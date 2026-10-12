//! 115 官方 OpenAPI 驱动（主通道）—— 完整实现。
//!
//! 端点与要点（依据官方文档 + 实测报文）：
//!
//! | 功能 | 方法 | 路径 | 编码 |
//! |---|---|---|---|
//! | 文件列表 | GET | `/open/ufile/files` | - |
//! | 详情 | GET | `/open/folder/get_info` | - |
//! | 下载直链 | POST | `/open/ufile/downurl` | multipart |
//! | 新建目录 | POST | `/open/folder/add` | multipart |
//! | 重命名 | POST | `/open/ufile/update` | multipart |
//! | 移动 | POST | `/open/ufile/move` | multipart |
//! | 复制 | POST | `/open/ufile/copy` | multipart |
//! | 删除 | POST | `/open/ufile/delete` | multipart |
//! | 上传初始化 | POST | `/open/upload/init` | multipart |
//! | 上传凭证 | GET | `/open/upload/get_token` | - |
//! | 配额 | GET | `/open/user/info` | - |
//!
//! 秒传流程：`init`（fileid/preid 均为**大写 SHA1**）→ `status=2` 零传输完成；
//! `status=6/7/8` 需按 `sign_check` 闭区间回传 `sign_key`+`sign_val` 重试；
//! 否则取 OSS 临时凭证直传，并带 `x-oss-callback` 头拿最终 `file_id`。

use crate::client::{ApiClient, PutBody};
use crate::hash::{hashes_for, read_source_range, sha1_hex_upper};
use crate::models::{
    DownUrlMap, FolderInfo, ListPage, OssToken, UploadInit, UploadResultData, UserInfo,
};
use cloudmount_auth::open::OpenApiAuth;
use cloudmount_core::{
    Envelope, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result, UploadResult,
    UploadSource,
};
use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};

/// 列表分页每页条数（官方文档上限 1150，取 1000 留余量）。
const PAGE_LIMIT: u32 = 1000;
/// 单目录最多遍历页数（防死循环）。
const MAX_PAGES: u32 = 100;
/// 二次认证最多重试次数。
const MAX_SIGN_ATTEMPTS: u32 = 3;

/// 官方 OpenAPI 驱动。
pub struct Open115Provider {
    /// 资源域客户端。
    client: ApiClient,
    /// `file_id -> pick_code` 缓存（downurl 需要 pick_code，而调用方只给 file_id）。
    pick_cache: Mutex<HashMap<String, String>>,
}

impl Open115Provider {
    /// 用认证器创建驱动。
    pub fn new(auth: Arc<OpenApiAuth>) -> Self {
        Self {
            client: ApiClient::new(auth),
            pick_cache: Mutex::new(HashMap::new()),
        }
    }

    /// 用已有客户端创建（便于复用连接与限速器）。
    pub fn with_client(client: ApiClient) -> Self {
        Self {
            client,
            pick_cache: Mutex::new(HashMap::new()),
        }
    }

    /// 上传目标标识：`U_1_<cid>`（根目录为 `U_1_0`）。
    fn target_of(parent: &RemoteId) -> String {
        format!("U_1_{}", parent.0)
    }

    /// 记住 file_id → pick_code 映射。
    fn remember_pick(&self, id: &str, pick: &str) {
        if id.is_empty() || pick.is_empty() {
            return;
        }
        if let Ok(mut cache) = self.pick_cache.lock() {
            cache.insert(id.to_string(), pick.to_string());
        }
    }

    /// 取缓存的 pick_code。
    fn cached_pick(&self, id: &str) -> Option<String> {
        self.pick_cache.lock().ok().and_then(|c| c.get(id).cloned())
    }

    /// 解析下载所需的 pick_code（缓存优先，缺失则查详情）。
    fn resolve_pick_code(&self, id: &RemoteId) -> Result<String> {
        if let Some(pc) = self.cached_pick(&id.0) {
            return Ok(pc);
        }
        let entry = self.stat(id)?;
        entry
            .pick_code
            .or_else(|| self.cached_pick(&id.0))
            .ok_or_else(|| Error::NotFound(format!("{} 没有 pick_code（目录或无法下载）", id.0)))
    }

    /// 从 `data` 里宽松提取一个 ID（不同接口返回 `file_id`/`cid`/`fid` 不一）。
    fn extract_id(value: &serde_json::Value) -> Option<String> {
        let obj = value.as_object()?;
        for key in ["file_id", "fileId", "cid", "fid", "fid_list"] {
            if let Some(v) = obj.get(key) {
                match v {
                    serde_json::Value::String(s) if !s.is_empty() => return Some(s.clone()),
                    serde_json::Value::Number(n) => return Some(n.to_string()),
                    serde_json::Value::Array(a) => {
                        if let Some(first) = a.first() {
                            if let Some(s) = first.as_str() {
                                return Some(s.to_string());
                            }
                            if let Some(n) = first.as_i64() {
                                return Some(n.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// 直传 OSS 并解析回执。
    fn upload_to_oss(
        &self,
        init: &UploadInit,
        source: &UploadSource<'_>,
        name: &str,
    ) -> Result<UploadResult> {
        let bucket = init
            .bucket
            .clone()
            .ok_or_else(|| Error::Other("init 响应缺少 bucket".into()))?;
        let object = init
            .object
            .clone()
            .ok_or_else(|| Error::Other("init 响应缺少 object".into()))?;
        let callback = init.callback_info().ok_or_else(|| {
            Error::Other("init 响应缺少 callback（缺少直传回执会拿不到 file_id）".into())
        })?;

        let token = {
            // get_token 无参数；data 形状在不同 SDK 中为对象或数组，这里统一宽松解析
            let value: serde_json::Value = self.client.get_json("/open/upload/get_token", &[])?;
            OssToken::parse(&value)?
        };
        let host = token
            .endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        let url = format!("https://{bucket}.{host}/{object}");

        use base64::Engine as _;
        let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
        let headers = vec![
            ("x-oss-security-token", token.security_token.clone()),
            ("x-oss-callback", b64(&callback.callback)),
            ("x-oss-callback-var", b64(&callback.callback_var)),
            ("Content-Type", "application/octet-stream".to_string()),
        ];

        tracing::info!(target = %url, name, "开始直传 OSS");
        let text = match source {
            UploadSource::Bytes(b) => {
                self.client
                    .put_with_headers(&url, &headers, PutBody::Bytes(b))?
            }
            UploadSource::Path(p) => {
                let f = std::fs::File::open(p).map_err(|e| {
                    Error::Io(std::io::Error::new(
                        e.kind(),
                        format!("打开 {} 失败: {e}", p.display()),
                    ))
                })?;
                self.client
                    .put_with_headers(&url, &headers, PutBody::File(&f))?
            }
        };

        let env = Envelope::parse(&text)?;
        let data: UploadResultData = env.into_data()?;
        let id = data
            .file_id
            .clone()
            .or_else(|| init.file_id.clone())
            .ok_or_else(|| Error::Other(format!("上传回执缺少 file_id: {name}")))?;
        if let Some(pc) = &data.pick_code {
            self.remember_pick(&id, pc);
        }
        Ok(UploadResult {
            id: RemoteId(id),
            rapid_upload: false,
        })
    }
}

impl Provider for Open115Provider {
    fn name(&self) -> &'static str {
        "115-open"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        let mut out: Vec<RemoteEntry> = Vec::new();
        let mut offset: u32 = 0;
        let mut page_no: u32 = 0;
        loop {
            page_no += 1;
            if page_no > MAX_PAGES {
                return Err(Error::Other(format!(
                    "目录 {} 分页超过 {MAX_PAGES} 页，疑似异常",
                    dir.0
                )));
            }
            let raw = self.client.get_raw(
                "/open/ufile/files",
                &[
                    ("cid", dir.0.clone()),
                    ("limit", PAGE_LIMIT.to_string()),
                    ("offset", offset.to_string()),
                    ("show_dir", "1".to_string()),
                    ("o", "user_utime".to_string()),
                    ("asc", "0".to_string()),
                ],
            )?;
            let page: ListPage = serde_json::from_value(raw)
                .map_err(|e| Error::Other(format!("文件列表响应解析失败: {e}")))?;
            let (files, count) = page.check()?;
            let n = files.len();
            for f in &files {
                let entry = f.to_entry();
                if let Some(pc) = entry.pick_code.as_deref() {
                    self.remember_pick(&entry.id.0, pc);
                }
                out.push(entry);
            }
            offset += n as u32;
            if n == 0 || out.len() as u64 >= count {
                break;
            }
        }
        Ok(out)
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        let raw = self
            .client
            .get_raw("/open/folder/get_info", &[("file_id", id.0.clone())])?;
        let info: FolderInfo = Envelope::from_value(raw)?.into_data()?;
        let entry = info.to_entry();
        if let Some(pc) = entry.pick_code.as_deref() {
            self.remember_pick(&entry.id.0, pc);
        }
        Ok(entry)
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        let pick_code = self.resolve_pick_code(id)?;
        let map: DownUrlMap = self
            .client
            .post_form_json("/open/ufile/downurl", &[("pick_code", pick_code)])?;
        // 返回以 file_id 为键；优先取请求的那一项，其次取第一项
        let entry = map
            .get(&id.0)
            .or_else(|| map.values().next())
            .ok_or_else(|| Error::NotFound(format!("downurl 未返回 {} 的直链", id.0)))?;
        self.client.get_range_stream(&entry.url.url, range)
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        // TODO(verify): 参数名按官方文档推断（pid/file_name），M0 实测时确认。
        let value: serde_json::Value = self.client.post_form_json(
            "/open/folder/add",
            &[("pid", parent.0.clone()), ("file_name", name.to_string())],
        )?;
        let id =
            Self::extract_id(&value).ok_or_else(|| Error::Other("新建目录响应未返回 ID".into()))?;
        Ok(RemoteEntry {
            id: RemoteId(id),
            parent_id: parent.clone(),
            name: name.to_string(),
            kind: cloudmount_core::EntryKind::Directory,
            size: 0,
            mtime: 0,
            sha1: None,
            pick_code: None,
            deleted: false,
        })
    }

    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        let hashes = hashes_for(&source)?;
        if hashes.size != size {
            tracing::warn!(
                expected = size,
                actual = hashes.size,
                "调用方给出的大小与实际读取不一致，以实际为准"
            );
        }

        let mut sign: Option<(String, String)> = None;
        for attempt in 1..=MAX_SIGN_ATTEMPTS {
            let mut fields = vec![
                ("file_name", name.to_string()),
                ("file_size", hashes.size.to_string()),
                ("target", Self::target_of(parent)),
                ("fileid", hashes.fileid.clone()),
                ("preid", hashes.preid.clone()),
            ];
            if let Some((key, val)) = &sign {
                fields.push(("sign_key", key.clone()));
                fields.push(("sign_val", val.clone()));
            }

            let init: UploadInit = self.client.post_form_json("/open/upload/init", &fields)?;
            match init.status {
                // 秒传命中：零传输完成
                2 => {
                    let id = init
                        .file_id
                        .clone()
                        .or_else(|| init.pick_code.clone())
                        .ok_or_else(|| Error::Other("秒传成功但未返回 file_id".into()))?;
                    if let Some(pc) = &init.pick_code {
                        self.remember_pick(&id, pc);
                    }
                    tracing::info!(%name, "秒传命中（零传输）");
                    return Ok(UploadResult {
                        id: RemoteId(id),
                        rapid_upload: true,
                    });
                }
                // 需要二次认证：按 sign_check 区间回传 SHA1 后重试
                6..=8 => {
                    let (start, end) = init.sign_check_range().ok_or_else(|| {
                        Error::Other("服务端要求二次认证但缺少 sign_check 区间".into())
                    })?;
                    let key = init
                        .sign_key
                        .clone()
                        .ok_or_else(|| Error::Other("服务端要求二次认证但缺少 sign_key".into()))?;
                    let bytes = read_source_range(&source, start, end)?;
                    tracing::info!(
                        attempt,
                        start,
                        end,
                        "115 要求上传二次认证，计算区间 SHA1 后重试"
                    );
                    sign = Some((key, sha1_hex_upper(&bytes)));
                    continue;
                }
                // 需要真实上传
                _ => return self.upload_to_oss(&init, &source, name),
            }
        }
        Err(Error::Other(format!(
            "二次认证重试 {MAX_SIGN_ATTEMPTS} 次仍未通过：{name}"
        )))
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        // TODO(verify): 参数名（file_id/file_name）待 M0 实测确认。
        let _: serde_json::Value = self.client.post_form_json(
            "/open/ufile/update",
            &[
                ("file_id", id.0.clone()),
                ("file_name", new_name.to_string()),
            ],
        )?;
        Ok(())
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        // 参数名已确认：file_ids（逗号分隔）+ to_cid（目标目录，根为 0）
        let _: serde_json::Value = self.client.post_form_json(
            "/open/ufile/move",
            &[("file_ids", id.0.clone()), ("to_cid", new_parent.0.clone())],
        )?;
        Ok(())
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        // 参数名已确认：注意 copy 用的是**单数** `file_id` + 目标目录 `pid`
        // （与 move 的 file_ids/to_cid 不同）；`nodupli` 可选（1=不允许重复）。
        let value: serde_json::Value = self.client.post_form_json(
            "/open/ufile/copy",
            &[("file_id", id.0.clone()), ("pid", new_parent.0.clone())],
        )?;
        // 官方文档只写 data: array，未给出字段；三方可参考实现也都不解析。
        // 这里宽松提取，取不到就明确报错（M0 实测后补精确字段）。
        Self::extract_id(&value)
            .map(RemoteId)
            .ok_or_else(|| Error::Other("复制成功但响应未返回新文件 ID（待实测确认）".into()))
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        // 参数名已确认：`file_ids` 为**单个字段**，多个 ID 用英文逗号分隔
        let _: serde_json::Value = self
            .client
            .post_form_json("/open/ufile/delete", &[("file_ids", id.0.clone())])?;
        Ok(())
    }

    fn quota(&self) -> Result<Quota> {
        let info: UserInfo = self.client.get_json("/open/user/info", &[])?;
        info.to_quota()
    }
}
