//! 115 资源域数据模型（字段名严格对齐官方文档与实测报文）。
//!
//! 字段命名注意：
//! - 文件名在 JSON 里是 `fn`（Rust 关键字，需 `rename`）；
//! - 大小是 `fs`，目录/文件标志是 `fc`（`"0"`=目录、`"1"`=文件，**字符串**）；
//! - 列表接口的分页字段（`count`/`offset`/`limit`）与 `data` **平级**，不在 `data` 内。

use cloudmount_core::serde_util::{num_or_str_u64, opt_num_or_str_u64};
use cloudmount_core::{EntryKind, Envelope, Error, Quota, RemoteEntry, RemoteId, Result};
use serde::{Deserialize, Serialize};

/// `ufile/files` 返回的单个条目。
#[derive(Debug, Clone, Deserialize)]
pub struct ApiFile {
    /// 文件/目录 ID。
    pub fid: String,
    /// 父目录 ID。
    #[serde(default)]
    pub pid: String,
    /// 类别："0"=目录，"1"=文件（字符串）。
    #[serde(default)]
    pub fc: String,
    /// 名称（JSON 字段名 `fn`）。
    #[serde(rename = "fn", default)]
    pub name: String,
    /// 大小（字节）。
    #[serde(default, deserialize_with = "num_or_str_u64")]
    pub fs: u64,
    /// 文件 SHA1（目录为空）。
    #[serde(default)]
    pub sha1: Option<String>,
    /// 提取码 / 下载凭据 pick_code。
    #[serde(default)]
    pub pc: Option<String>,
    /// 更新时间（Unix 秒）。
    #[serde(default, deserialize_with = "opt_num_or_str_u64")]
    pub uet: Option<u64>,
    /// 创建时间（Unix 秒）。
    #[serde(default, deserialize_with = "opt_num_or_str_u64")]
    pub upt: Option<u64>,
}

impl ApiFile {
    /// 转为统一条目模型。
    pub fn to_entry(&self) -> RemoteEntry {
        RemoteEntry {
            id: RemoteId(self.fid.clone()),
            parent_id: RemoteId(self.pid.clone()),
            name: self.name.clone(),
            kind: if self.fc == "0" {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size: self.fs,
            mtime: self.uet.or(self.upt).unwrap_or(0) as i64,
            sha1: self.sha1.clone().filter(|s| !s.is_empty()),
            pick_code: self.pc.clone().filter(|s| !s.is_empty()),
            deleted: false,
        }
    }
}

/// `ufile/files` 的整页响应（含顶层分页字段）。
#[derive(Debug, Clone, Deserialize)]
pub struct ListPage {
    #[serde(default)]
    pub state: Option<cloudmount_core::envelope::StateFlag>,
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub errno: Option<i64>,
    /// 本页条目。
    #[serde(default)]
    pub data: Vec<ApiFile>,
    /// 该目录下的总条目数。
    #[serde(default, deserialize_with = "num_or_str_u64")]
    pub count: u64,
}

impl ListPage {
    /// 校验信封并取出（条目, 总数）。
    pub fn check(self) -> Result<(Vec<ApiFile>, u64)> {
        let env = Envelope {
            state: self.state,
            code: self.code,
            message: self.message,
            error: self.error,
            errno: self.errno,
            data: None,
        };
        if !env.is_ok() {
            return Err(env.into_error());
        }
        Ok((self.data, self.count))
    }
}

/// `folder/get_info` 的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct FolderInfo {
    /// 文件/目录 ID。
    #[serde(default)]
    pub file_id: String,
    /// 名称。
    #[serde(default)]
    pub file_name: String,
    /// 大小（字节，可能是字符串）。
    #[serde(default, deserialize_with = "num_or_str_u64")]
    pub size_byte: u64,
    /// SHA1。
    #[serde(default)]
    pub sha1: Option<String>,
    /// pick_code。
    #[serde(default)]
    pub pick_code: Option<String>,
    /// 更新时间。
    #[serde(default, deserialize_with = "opt_num_or_str_u64")]
    pub utime: Option<u64>,
    /// 创建时间。
    #[serde(default, deserialize_with = "opt_num_or_str_u64")]
    pub ptime: Option<u64>,
    /// 子目录数量（**仅目录有此字段**，据此区分目录/文件）。
    #[serde(default, deserialize_with = "opt_num_or_str_u64")]
    pub folder_count: Option<u64>,
    /// 父目录 ID（来自 paths 末项，可选）。
    #[serde(default)]
    pub paths: Option<Vec<FolderPath>>,
}

/// `folder/get_info` 的路径项。
#[derive(Debug, Clone, Deserialize)]
pub struct FolderPath {
    /// 路径节点 ID。
    #[serde(default)]
    pub file_id: String,
    /// 路径节点名称。
    #[serde(default)]
    pub file_name: String,
}

impl FolderInfo {
    /// 转为统一条目模型。
    pub fn to_entry(&self) -> RemoteEntry {
        let parent = self
            .paths
            .as_ref()
            .and_then(|p| p.iter().rev().nth(1)) // 倒数第二个是父目录
            .map(|p| p.file_id.clone())
            .unwrap_or_else(|| "0".to_string());
        RemoteEntry {
            id: RemoteId(self.file_id.clone()),
            parent_id: RemoteId(parent),
            name: self.file_name.clone(),
            kind: if self.folder_count.is_some() {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size: self.size_byte,
            mtime: self.utime.or(self.ptime).unwrap_or(0) as i64,
            sha1: self.sha1.clone().filter(|s| !s.is_empty()),
            pick_code: self.pick_code.clone().filter(|s| !s.is_empty()),
            deleted: false,
        }
    }
}

/// `ufile/downurl` 的 `data`：以 file_id 为键的对象。
pub type DownUrlMap = std::collections::HashMap<String, DownUrlEntry>;

/// `ufile/downurl` 的单个条目。
#[derive(Debug, Clone, Deserialize)]
pub struct DownUrlEntry {
    /// 文件名。
    #[serde(default)]
    pub file_name: String,
    /// 文件大小。
    #[serde(default, deserialize_with = "num_or_str_u64")]
    pub file_size: u64,
    /// pick_code。
    #[serde(default)]
    pub pick_code: String,
    /// SHA1。
    #[serde(default)]
    pub sha1: Option<String>,
    /// 直链信息。
    pub url: DownUrlInfo,
}

/// 直链信息（实测路径：`data[fileId].url.url`）。
#[derive(Debug, Clone, Deserialize)]
pub struct DownUrlInfo {
    /// 真正的下载直链。
    pub url: String,
    /// 客户端类型。
    #[serde(default)]
    pub client: Option<String>,
    /// 说明。
    #[serde(default)]
    pub desc: Option<String>,
    /// ISP 标识。
    #[serde(default)]
    pub isp: Option<String>,
}

/// `user/info` 的 `data`（空间字段嵌套在 `rt_space_info` 里，无平铺 size/used/remain）。
#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    /// 用户 ID。
    #[serde(default)]
    pub user_id: Option<serde_json::Value>,
    /// 用户名。
    #[serde(default)]
    pub user_name: Option<String>,
    /// 空间信息。
    #[serde(default)]
    pub rt_space_info: Option<SpaceInfo>,
    /// 会员信息。
    #[serde(default)]
    pub vip_info: Option<VipInfo>,
}

/// 空间信息。
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceInfo {
    /// 总空间。
    #[serde(default)]
    pub all_total: Option<SpaceSize>,
    /// 已用空间。
    #[serde(default)]
    pub all_use: Option<SpaceSize>,
    /// 剩余空间。
    #[serde(default)]
    pub all_remain: Option<SpaceSize>,
}

/// 空间尺寸（`size` 为字节）。
#[derive(Debug, Clone, Deserialize)]
pub struct SpaceSize {
    /// 字节数（可能是字符串）。
    #[serde(default, deserialize_with = "num_or_str_u64")]
    pub size: u64,
}

/// 会员信息。
#[derive(Debug, Clone, Deserialize)]
pub struct VipInfo {
    /// 等级名。
    #[serde(default)]
    pub level_name: Option<String>,
    /// 到期时间。
    #[serde(default)]
    pub expire: Option<serde_json::Value>,
}

impl UserInfo {
    /// 转为统一配额模型。
    pub fn to_quota(&self) -> Result<Quota> {
        let space = self
            .rt_space_info
            .as_ref()
            .ok_or_else(|| Error::Other("user/info 缺少 rt_space_info".into()))?;
        let total = space.all_total.as_ref().map(|s| s.size).unwrap_or(0);
        let used = space.all_use.as_ref().map(|s| s.size).unwrap_or(0);
        let remaining = space
            .all_remain
            .as_ref()
            .map(|s| s.size)
            .unwrap_or_else(|| total.saturating_sub(used));
        Ok(Quota {
            total,
            used,
            remaining,
        })
    }
}

/// OSS 直传回调信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CallbackInfo {
    /// 回调 URL。
    pub callback: String,
    /// 回调变量（JSON 文本）。
    pub callback_var: String,
}

impl CallbackInfo {
    /// 从多种已知形态解析回调信息：
    /// 1) `{"callback":..,"callback_var":..}`
    /// 2) `[{"callback":..,"callback_var":..}]`
    /// 3) `{"value":{"callback":..,"callback_var":..}}`
    pub fn parse(v: Option<&serde_json::Value>) -> Option<Self> {
        fn from_obj(o: &serde_json::Map<String, serde_json::Value>) -> Option<CallbackInfo> {
            let callback = o.get("callback")?.as_str()?.to_string();
            // callback_var 可能是字符串，也可能是对象
            let callback_var = match o.get("callback_var") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(v @ serde_json::Value::Object(_)) => v.to_string(),
                _ => String::new(),
            };
            Some(CallbackInfo {
                callback,
                callback_var,
            })
        }
        match v? {
            serde_json::Value::Object(o) => {
                if o.contains_key("callback") {
                    from_obj(o)
                } else {
                    o.get("value").and_then(|inner| match inner {
                        serde_json::Value::Object(io) => from_obj(io),
                        serde_json::Value::Array(items) => {
                            items.first().and_then(|i| i.as_object()).and_then(from_obj)
                        }
                        _ => None,
                    })
                }
            }
            serde_json::Value::Array(items) => {
                items.first().and_then(|i| i.as_object()).and_then(from_obj)
            }
            _ => None,
        }
    }
}

/// `upload/init` 与 `upload/resume` 的 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct UploadInit {
    /// 状态：1=需上传，2=秒传成功，6/7/8=需二次认证。
    #[serde(default, deserialize_with = "num_or_str_u64")]
    pub status: u64,
    /// pick_code（秒传成功时即为最终文件）。
    #[serde(default)]
    pub pick_code: Option<String>,
    /// 二次认证用的 key。
    #[serde(default)]
    pub sign_key: Option<String>,
    /// 待校验字节区间，形如 `"2392148-2392298"`（闭区间）。
    #[serde(default)]
    pub sign_check: Option<String>,
    /// 秒传成功时的文件 ID。
    #[serde(default)]
    pub file_id: Option<String>,
    /// 目标目录标识 `U_1_<cid>`。
    #[serde(default)]
    pub target: Option<String>,
    /// 直传 bucket。
    #[serde(default)]
    pub bucket: Option<String>,
    /// 直传 object 路径。
    #[serde(default)]
    pub object: Option<String>,
    /// 直传回调（形态多样，用 [`CallbackInfo::parse`] 解析）。
    #[serde(default)]
    pub callback: Option<serde_json::Value>,
}

impl UploadInit {
    /// 解析回调。
    pub fn callback_info(&self) -> Option<CallbackInfo> {
        CallbackInfo::parse(self.callback.as_ref())
    }

    /// 解析 `sign_check` 闭区间为 (start, end)。
    pub fn sign_check_range(&self) -> Option<(u64, u64)> {
        parse_sign_check(self.sign_check.as_deref()?)
    }
}

/// 解析 `"start-end"` 形式的闭区间。
pub fn parse_sign_check(s: &str) -> Option<(u64, u64)> {
    let (a, b) = s.trim().split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// `upload/get_token` 的 `data`（字段大小写在不同 SDK 中不一致，全部兼容）。
#[derive(Debug, Clone)]
pub struct OssToken {
    /// 直传 endpoint 主机名。
    pub endpoint: String,
    /// 临时 AccessKeyId。
    pub access_key_id: String,
    /// 临时 AccessKeySecret。
    pub access_key_secret: String,
    /// 安全令牌。
    pub security_token: String,
}

impl OssToken {
    /// 从 `data`（对象或数组形态）解析。
    pub fn parse(v: &serde_json::Value) -> Result<Self> {
        let obj =
            match v {
                serde_json::Value::Object(o) => o.clone(),
                serde_json::Value::Array(items) => items
                    .first()
                    .and_then(|i| i.as_object())
                    .cloned()
                    .ok_or_else(|| Error::Other("get_token 返回空数组".into()))?,
                _ => return Err(Error::Other("get_token 返回格式未知".into())),
            };
        let get = |keys: &[&str]| -> Option<String> {
            keys.iter().find_map(|k| {
                obj.get(*k).map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                })
            })
        };
        Ok(Self {
            endpoint: get(&["endpoint", "Endpoint"])
                .ok_or_else(|| Error::Other("get_token 缺少 endpoint".into()))?,
            access_key_id: get(&["AccessKeyId", "access_key_id", "accessKeyId"])
                .ok_or_else(|| Error::Other("get_token 缺少 AccessKeyId".into()))?,
            // 官方文档有 `AccessKeySecrett` 笔误，线上实际用 `AccessKeySecret`，两者都试
            access_key_secret: get(&[
                "AccessKeySecret",
                "AccessKeySecrett",
                "access_key_secret",
                "accessKeySecret",
            ])
            .ok_or_else(|| Error::Other("get_token 缺少 AccessKeySecret".into()))?,
            security_token: get(&["SecurityToken", "security_token", "securityToken"])
                .ok_or_else(|| Error::Other("get_token 缺少 SecurityToken".into()))?,
        })
    }
}

/// 上传完成（OSS 回调）后的响应 `data`。
#[derive(Debug, Clone, Deserialize)]
pub struct UploadResultData {
    /// 最终文件 ID。
    #[serde(default)]
    pub file_id: Option<String>,
    /// pick_code。
    #[serde(default)]
    pub pick_code: Option<String>,
    /// 文件名。
    #[serde(default)]
    pub file_name: Option<String>,
    /// 大小。
    #[serde(default, deserialize_with = "opt_num_or_str_u64")]
    pub file_size: Option<u64>,
    /// SHA1。
    #[serde(default)]
    pub sha1: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用调研拿到的真实报文样本验证解析。
    #[test]
    fn parses_real_list_page() {
        let raw = r#"{
          "state": true, "code": 0, "message": "",
          "data": [
            {"fid":"2742473097771062416","aid":"1","pid":"0","fc":"0","fn":"我的文档","pc":"",
             "upt":1770622000,"uet":1770622001,"sha1":"","fs":0},
            {"fid":"2742473097771062417","aid":"1","pid":"0","fc":"1","fn":"报告.pdf","pc":"abc123",
             "upt":1770622000,"uet":1770622002,"sha1":"DA39A3EE5E6B4B0D3255BFEF95601890AFD80709","fs":12345}
          ],
          "count": 2, "sys_count": 0, "offset": 0, "limit": 20, "cid": 0
        }"#;
        let page: ListPage = serde_json::from_str(raw).unwrap();
        let (files, count) = page.check().unwrap();
        assert_eq!(count, 2);
        assert_eq!(files.len(), 2);

        let dir = files[0].to_entry();
        assert_eq!(dir.kind, EntryKind::Directory);
        assert_eq!(dir.name, "我的文档");
        assert_eq!(dir.size, 0);
        assert_eq!(dir.sha1, None, "空 SHA1 应归一为 None");

        let file = files[1].to_entry();
        assert_eq!(file.kind, EntryKind::File);
        assert_eq!(file.name, "报告.pdf");
        assert_eq!(file.size, 12345);
        assert_eq!(file.pick_code.as_deref(), Some("abc123"));
        assert_eq!(file.mtime, 1770622002);
    }

    #[test]
    fn list_page_error_maps_to_auth() {
        let raw = r#"{"state":false,"message":"access_token 格式错误","code":40140123,"data":[],"count":0}"#;
        let page: ListPage = serde_json::from_str(raw).unwrap();
        assert!(matches!(page.check().unwrap_err(), Error::Auth(_)));
    }

    #[test]
    fn parses_user_info_quota() {
        let raw = r#"{
          "user_id": 123, "user_name": "tester",
          "rt_space_info": {
            "all_total": {"size": 1099511627776, "size_format": "1TB"},
            "all_use": {"size": 1073741824},
            "all_remain": {"size": "1098437885952"}
          },
          "vip_info": {"level_name": "长期VIP", "expire": 1800000000}
        }"#;
        let info: UserInfo = serde_json::from_str(raw).unwrap();
        let q = info.to_quota().unwrap();
        assert_eq!(q.total, 1099511627776);
        assert_eq!(q.used, 1073741824);
        assert_eq!(q.remaining, 1098437885952, "字符串数字也要能解析");
    }

    #[test]
    fn parses_sign_check_range() {
        assert_eq!(
            parse_sign_check("2392148-2392298"),
            Some((2392148, 2392298))
        );
        assert_eq!(parse_sign_check(" 10 - 20 "), Some((10, 20)));
        assert_eq!(parse_sign_check("garbage"), None);
    }

    #[test]
    fn parses_callback_shapes() {
        let a = serde_json::json!({"callback":"https://cb","callback_var":"{\"x\":1}"});
        let b = serde_json::json!([{"callback":"https://cb","callback_var":"v"}]);
        let c = serde_json::json!({"value":{"callback":"https://cb","callback_var":{"k":"v"}}});
        for v in [a, b, c] {
            let info = CallbackInfo::parse(Some(&v)).expect("应能解析");
            assert_eq!(info.callback, "https://cb");
            assert!(!info.callback_var.is_empty());
        }
        assert!(CallbackInfo::parse(None).is_none());
    }

    #[test]
    fn parses_get_token_with_typo_key_and_array_shape() {
        // 官方文档的 `AccessKeySecrett` 笔误形态
        let quirky = serde_json::json!([{
            "endpoint": "oss-cn-shenzhen.aliyuncs.com",
            "AccessKeyId": "AK",
            "AccessKeySecrett": "SK",
            "SecurityToken": "ST"
        }]);
        let t = OssToken::parse(&quirky).unwrap();
        assert_eq!(t.endpoint, "oss-cn-shenzhen.aliyuncs.com");
        assert_eq!(t.access_key_secret, "SK");

        // 正常对象形态
        let normal = serde_json::json!({
            "Endpoint":"e","AccessKeyId":"a","AccessKeySecret":"s","SecurityToken":"t"
        });
        assert_eq!(OssToken::parse(&normal).unwrap().access_key_id, "a");
    }
}
