//! 115 Cookie 通道驱动（**补充通道，默认关闭**）。
//!
//! ⚠️ 风险与合规：见 [`cloudmount_auth::cookie`] 模块头部声明。
//! 本驱动仅实现"像正常客户端"的行为（固定浏览器 UA、严格限频），
//! **不做**任何主动风控规避。
//!
//! # 能力范围（M3 v1：只读）
//!
//! - `list` / `stat`：`GET webapi.115.com/files`（分页）；
//! - `open_range`（下载直链）：`POST proapi.115.com/app/chrome/downurl`
//!   （m115 加密，见 [`crate::m115`]）；
//! - `quota`：`GET webapi.115.com/files/index_info`（已用量；总量该接口
//!   不返回，置 0）；
//! - 写操作（create/upload/rename/move/copy/delete）：**v1 未实现**
//!   （上传需秒传 + OSS 直传链路，M3 v2）；引擎在 Cookie 通道下
//!   挂载浏览/读取可用，本地修改上传会得到明确错误。

use crate::ec115::{self, EcdhCipher};
use crate::hash::{hashes_for, read_source_range, sha1_hex_upper};
use crate::m115;
use crate::ratelimit::RateLimiter;
use cloudmount_auth::cookie::{CookieAuth, CookiePolicy};
use cloudmount_core::serde_util::num_or_str_u64;
use cloudmount_core::{
    EntryKind, Envelope, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result,
    UploadResult,
};
use std::collections::HashMap;
use std::io::Read;

/// web 资源域。
pub const WEBAPI_BASE: &str = "https://webapi.115.com";
/// pro 资源域（下载直链，m115 加密）。
pub const PROAPI_BASE: &str = "https://proapi.115.com";

/// 列表分页大小（web 接口上限约 1150，保守取 1000）。
const PAGE_LIMIT: i64 = 1000;

/// 上传基址（**完整基址含 scheme**；默认 = 生产 uplb 域名，测试可注入本地 mock）。
///
/// F-14 注入面：initupload/gettoken 渲染为 `{upload_base}{路径}` 拼接，
/// 默认渲染与拆分前常量（`https://uplb.115.com/4.0/initupload.php`、
/// `https://uplb.115.com/3.0/gettoken.php`）逐字节一致（等价回归单测锁定）。
const UPLOAD_BASE_DEFAULT: &str = "https://uplb.115.com";
/// initupload 端点路径（拼接在上传基址后）。
const INITUPLOAD_PATH: &str = "/4.0/initupload.php";
/// gettoken 端点路径（拼接在上传基址后）。
const GETTOKEN_PATH: &str = "/3.0/gettoken.php";
/// OSS 直传 URL 模板（**完整模板含 scheme**，含 `{bucket}`/`{object}` 占位）。
///
/// 默认渲染与原 `format!("https://{bucket}.{OSS_ENDPOINT}/{object}")` 逐字节一致；
/// 模板形态使本地明文 mock（无 DNS 子域能力）可用路径段承载 bucket。
const OSS_BASE_TEMPLATE_DEFAULT: &str =
    "https://{bucket}.cn-shenzhen.oss.aliyuncs.com/{object}";
/// 上传 app 版本（p115client 现行值；27.0.5.7/30.5.1/36.0.1 均被拒）。
const APP_VER: &str = "36.2.28";
/// sign_check 重试上限。
const MAX_SIGN_ATTEMPTS: u32 = 3;

/// 分页保护上限。
const MAX_PAGES: u32 = 200;

/// 单次限速等待上限。
const MAX_RATE_WAIT_MS: u64 = 5_000;

/// web 文件列表的单个条目（字段名对齐 webapi 报文）。
///
/// ID 语义（对照 115driver File.from）：
/// - 文件：ID=`fid`，父=`cid`（文件所属目录）；
/// - 目录：ID=`cid`（自身），父=`pid`；`fid` 为空即目录。
#[derive(Debug, Clone, serde::Deserialize)]
struct WebFile {
    /// 文件 ID（目录为空串）。
    #[serde(default)]
    fid: String,
    /// 文件=父目录 ID；目录=自身 ID。
    #[serde(default, deserialize_with = "num_or_str")]
    cid: String,
    /// 目录的父目录 ID。
    #[serde(default)]
    pid: String,
    /// 名称。
    #[serde(default, rename = "n")]
    name: String,
    /// 大小（字节；字符串或数字）。
    #[serde(default, rename = "s", deserialize_with = "num_or_str_u64")]
    size: u64,
    /// 下载凭证 pickcode。
    #[serde(default, rename = "pc")]
    pick_code: String,
    /// SHA1。
    #[serde(default, rename = "sha")]
    sha1: String,
    /// 更新时间 "2026-10-13 15:04:05"（东八区字符串）。
    #[serde(default, rename = "t")]
    updated: String,
    /// 创建时间（Unix 秒，字符串或数字）。
    #[serde(default, rename = "tp", deserialize_with = "num_or_str_u64")]
    created: u64,
}

impl WebFile {
    fn to_entry(&self) -> RemoteEntry {
        let is_dir = self.fid.is_empty();
        RemoteEntry {
            id: RemoteId(if is_dir {
                self.cid.clone()
            } else {
                self.fid.clone()
            }),
            parent_id: RemoteId(if is_dir {
                self.pid.clone()
            } else {
                self.cid.clone()
            }),
            name: self.name.clone(),
            kind: if is_dir {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size: self.size,
            mtime: web_mtime(&self.updated, self.created),
            sha1: empty_to_none(&self.sha1),
            pick_code: empty_to_none(&self.pick_code),
            deleted: false,
        }
    }
}

/// web 时间解析（对照 115driver File.from 的不对称语义）：
/// - 目录的 `t` 是 **Unix 秒**（数字串）；
/// - 文件的 `t` 是东八区日期串 `"2026-10-13 15:04"`（**分钟精度**，
///   可能带秒）；解析失败回退 `created`。
fn web_mtime(updated: &str, created: u64) -> i64 {
    if !updated.is_empty() {
        // 目录：纯数字串 → Unix 秒
        if let Ok(secs) = updated.parse::<i64>() {
            return secs;
        }
        // 文件：日期串（"Y-M-D H:M" 或 "Y-M-D H:M:S"）
        let parts: Vec<&str> = updated.split(['-', ' ', ':']).collect();
        if parts.len() >= 5 {
            let nums: Option<Vec<i64>> = parts.iter().map(|p| p.parse::<i64>().ok()).collect();
            if let Some(n) = nums {
                let (y, mo, d, h, mi) = (n[0], n[1], n[2], n[3], n[4]);
                let sec = n.get(5).copied().unwrap_or(0);
                if let Ok(date) = time::Date::from_calendar_date(
                    y as i32,
                    time::Month::try_from(mo as u8).unwrap_or(time::Month::January),
                    d as u8,
                ) {
                    if let Ok(dt) = date.with_hms(h as u8, mi as u8, sec as u8) {
                        // 按东八区解释 → Unix 秒
                        let cst = dt
                            .assume_utc()
                            .replace_offset(time::UtcOffset::from_hms(8, 0, 0).unwrap());
                        return cst.unix_timestamp();
                    }
                }
            }
        }
    }
    created as i64
}

/// cid 字段可能是数字或字符串（Go IntString 语义）。
fn num_or_str<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    })
}

fn empty_to_none(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Cookie 通道驱动。
pub struct Cookie115Provider {
    /// 认证器（含默认关闭策略校验）。
    auth: CookieAuth,
    /// HTTP 客户端（固定浏览器 UA + Cookie 头 + 限速）。
    agent: ureq::Agent,
    /// 全局限速器（比官方通道更保守，默认 0.5 r/s）。
    limiter: RateLimiter,
    /// web 资源域（测试注入）。
    webapi_base: String,
    /// pro 资源域（测试注入）。
    proapi_base: String,
    /// 上传基址（完整基址含 scheme；默认 = [`UPLOAD_BASE_DEFAULT`]）。
    upload_base: String,
    /// OSS 直传 URL 模板（含 `{bucket}`/`{object}` 占位；默认 =
    /// [`OSS_BASE_TEMPLATE_DEFAULT`]）。
    oss_base: String,
    /// ECDH 远端公钥替换（仅测试；`None` = 115 固定公钥，生产路径不变）。
    ecdh_remote_pub: Option<[u8; 56]>,
    /// fid → pickcode 缓存（list 时填充；open_range 下载凭证的来源——
    /// get_info 端点实测返回 990002 参数错误，不可依赖）。
    picks: std::sync::Mutex<HashMap<String, String>>,
}

impl Cookie115Provider {
    /// 创建 Cookie 驱动；`policy` 必须为显式开启且知情同意，否则构造即失败。
    pub fn try_new(policy: CookiePolicy, auth: CookieAuth) -> Result<Self> {
        policy.check_enabled()?;
        if !auth.is_logged_in() {
            return Err(Error::Auth(
                "Cookie 通道未登录：请先执行 `cloudmount login --cookie`".into(),
            ));
        }
        Ok(Self {
            agent: ureq::Agent::config_builder()
                .user_agent(cloudmount_auth::cookie::WEB_USER_AGENT)
                .http_status_as_error(false)
                .timeout_global(Some(std::time::Duration::from_secs(120)))
                .build()
                .into(),
            limiter: RateLimiter::new(0.5, 1.0),
            auth,
            webapi_base: WEBAPI_BASE.into(),
            proapi_base: PROAPI_BASE.into(),
            upload_base: UPLOAD_BASE_DEFAULT.into(),
            oss_base: OSS_BASE_TEMPLATE_DEFAULT.into(),
            ecdh_remote_pub: None,
            picks: std::sync::Mutex::new(HashMap::new()),
        })
    }

    /// 注入资源域地址（集成测试 mock 服务端）。
    pub fn with_base_urls(mut self, webapi: impl Into<String>, proapi: impl Into<String>) -> Self {
        self.webapi_base = webapi.into().trim_end_matches('/').to_string();
        self.proapi_base = proapi.into().trim_end_matches('/').to_string();
        self
    }

    /// 注入上传基址（**完整基址含 scheme**；集成测试本地 mock 服务端）。
    ///
    /// initupload/gettoken 渲染为 `{base}{INITUPLOAD_PATH}`、`{base}{GETTOKEN_PATH}`；
    /// 默认值与生产常量逐字节等价。
    pub fn with_upload_base(mut self, base: impl Into<String>) -> Self {
        self.upload_base = base.into().trim_end_matches('/').to_string();
        self
    }

    /// 注入 OSS 直传 URL 模板（须含 scheme 与 `{bucket}`/`{object}` 占位）。
    ///
    /// 本地明文 mock 无 DNS 子域能力，用路径段承载 bucket
    /// （如 `http://127.0.0.1:{port}/{bucket}/{object}`）。
    pub fn with_oss_base(mut self, template: impl Into<String>) -> Self {
        self.oss_base = template.into();
        self
    }

    /// 注入限速参数（测试提速；镜像 `ApiClient::with_rate_limit` 既有先例）。
    pub fn with_rate_limit(mut self, rate_per_sec: f64, capacity: f64) -> Self {
        self.limiter = RateLimiter::new(rate_per_sec, capacity);
        self
    }

    /// 替换 ECDH 远端公钥（仅测试；与 [`crate::ec115::test_peer`] 测试对端配对）。
    #[doc(hidden)]
    pub fn with_ecdh_remote_pub(mut self, remote: [u8; 56]) -> Self {
        self.ecdh_remote_pub = Some(remote);
        self
    }

    /// 尊重限速的等待。
    fn pace(&self) -> Result<()> {
        loop {
            match self.limiter.try_acquire() {
                Ok(()) => return Ok(()),
                Err(Error::RateLimited { retry_after_ms }) => {
                    let wait = retry_after_ms.min(MAX_RATE_WAIT_MS);
                    std::thread::sleep(std::time::Duration::from_millis(wait));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// 带 Cookie 的 GET，返回原始 JSON。
    fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let cookie = self.auth.cookie_header()?;
        self.pace()?;
        let resp = self
            .agent
            .get(url)
            .header("Cookie", &cookie)
            .call()
            .map_err(|e| Error::Transport(format!("GET {url}: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取响应失败: {e}")))?;
        serde_json::from_str(&text).map_err(|e| Error::Other(format!("JSON 解析失败: {e}")))
    }

    /// 带 Cookie 的 POST 表单，返回原始 JSON。
    fn post_form_json(&self, url: &str, fields: &[(&str, &str)]) -> Result<serde_json::Value> {
        let cookie = self.auth.cookie_header()?;
        self.pace()?;
        let resp = self
            .agent
            .post(url)
            .header("Cookie", &cookie)
            .send_form(fields.iter().copied())
            .map_err(|e| Error::Transport(format!("POST {url}: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取响应失败: {e}")))?;
        serde_json::from_str(&text).map_err(|e| Error::Other(format!("JSON 解析失败: {e}")))
    }

    /// 拉取一页文件列表。
    fn list_page(&self, cid: &str, offset: i64) -> Result<(Vec<WebFile>, i64)> {
        let url = format!(
            "{}/files?aid=1&cid={}&o=user_ptime&asc=0&offset={}&show_dir=1&limit={}&snap=0&natsort=0&record_open_time=1&format=json&fc_mix=0",
            self.webapi_base,
            cid,
            offset,
            PAGE_LIMIT
        );
        let value = self.get_json(&url)?;
        let env = Envelope::from_value(value.clone())?;
        if !env.is_ok() {
            // Cookie 失效：web 接口返回 state=false + 未登录提示
            return Err(Error::Auth(format!(
                "Cookie 通道列表失败（可能已失效，请重新扫码）: {}",
                env.message.clone().unwrap_or_default()
            )));
        }
        let count = value.get("count").and_then(|c| c.as_i64()).unwrap_or(0);
        let files: Vec<WebFile> = env
            .into_value()?
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        // 记忆 fid → pickcode（open_range 下载凭证）
        if let Ok(mut picks) = self.picks.lock() {
            for f in &files {
                if !f.pick_code.is_empty() && !f.fid.is_empty() {
                    picks.insert(f.fid.clone(), f.pick_code.clone());
                }
            }
        }
        Ok((files, count))
    }

    /// m115 加密的下载直链请求。
    fn download_url(&self, pick_code: &str) -> Result<String> {
        let payload = format!(r#"{{"pickcode":"{pick_code}"}}"#);
        let (encoded, key) = m115::encode(payload.as_bytes());
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let url = format!("{}/app/chrome/downurl?t={}", self.proapi_base, now_ms);
        let value = self.post_form_json(&url, &[("data", encoded.as_str())])?;
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::Auth(format!(
                "获取下载直链失败: {}",
                env.message.clone().unwrap_or_default()
            )));
        }
        // data 是 m115 加密字符串
        let encoded_resp = env
            .into_value()?
            .as_str()
            .ok_or_else(|| Error::Other("downurl 响应 data 不是字符串".into()))?
            .to_string();
        let plain = m115::decode(&encoded_resp, &key)?;
        // JSON: map[pickcode] → {url: {url: "..."}}
        let decoded: serde_json::Value = serde_json::from_slice(&plain)
            .map_err(|e| Error::Other(format!("downurl 解码后 JSON 解析失败: {e}")))?;
        let entry = decoded
            .get(pick_code)
            .or_else(|| decoded.as_object().and_then(|m| m.values().next()))
            .ok_or_else(|| Error::NotFound(format!("downurl 未返回 {pick_code} 的直链")))?;
        let url = entry
            .pointer("/url/url")
            .and_then(|u| u.as_str())
            .or_else(|| entry.get("url").and_then(|u| u.as_str()))
            .ok_or_else(|| Error::Other("downurl 响应缺少 url".into()))?
            .to_string();
        Ok(url)
    }

    /// 上传后验证：列父目录按 SHA1（或名称）找到新文件，返回其 ID。
    fn verify_upload(
        &self,
        dir_id: &str,
        sha1: &str,
        name: &str,
        rapid: bool,
    ) -> Result<UploadResult> {
        let entries = self.list(&RemoteId(dir_id.to_string()))?;
        let found = entries.iter().find(|e| {
            e.sha1
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case(sha1))
                .unwrap_or(false)
                || e.name == name
        });
        match found {
            Some(e) => {
                tracing::info!(%name, rapid, "上传完成（已验证）");
                Ok(UploadResult {
                    id: e.id.clone(),
                    rapid_upload: rapid,
                })
            }
            None => Err(Error::Other(format!(
                "上传回执验证失败：目录 {dir_id} 未找到 {name}"
            ))),
        }
    }

    /// 只读探测（不踢下线）；失效返回 Ok(false)。
    pub fn probe(&self) -> Result<bool> {
        self.auth.probe_readonly()
    }

    // ---------- 上传（M3 v2） ----------

    /// 上传通道版本（openlist/115driver 验证过的组合：UA 与 appversion
    /// 都用 115Browser 27.0.5.7；动态取 appversion 端点的 win 版本
    /// 36.0.1 反而 token invalid——那是桌面客户端版本号）。
    fn current_app_version(&self) -> String {
        APP_VER.into()
    }

    /// 上传上下文（uploadinfo 一次性获取，随上传缓存）。
    fn upload_info(&self) -> Result<UploadCtx> {
        let url = format!("{}/app/uploadinfo", self.proapi_base);
        let value = self.post_form_json(&url, &[])?;
        tracing::debug!(target: "cm_cookie_uploadinfo", raw = %value, "uploadinfo 原始响应");
        // 响应为**扁平结构**（user_id/userkey/size_limit 在顶层，无 data 包裹）
        // ——直接读顶层（Envelope.into_value 只回 data 字段，会拿到 null）
        if value.get("state").and_then(|v| v.as_bool()) == Some(false) {
            return Err(Error::Remote {
                code: value.get("errno").and_then(|v| v.as_i64()).unwrap_or(0),
                message: "uploadinfo 失败".into(),
            });
        }
        let data = value;
        let user_id = data
            .get("user_id")
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(0);
        let userkey = data
            .get("userkey")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if user_id == 0 || userkey.is_empty() {
            return Err(Error::Auth(
                "uploadinfo 缺少 user_id/userkey（Cookie 可能失效）".into(),
            ));
        }
        Ok(UploadCtx { user_id, userkey })
    }

    /// 构造 ECDH 会话（生产 = 115 固定公钥；注入替换公钥时用测试对端配对构造）。
    fn ecdh_cipher(&self) -> Result<EcdhCipher> {
        match self.ecdh_remote_pub {
            Some(remote) => EcdhCipher::with_remote_pub(remote),
            None => EcdhCipher::new(),
        }
    }

    /// ECDH 加密的 initupload 请求（含 sign_check 区间的自动计算）。
    #[allow(clippy::too_many_arguments)]
    fn init_upload(
        &self,
        ctx: &UploadCtx,
        dir_id: &str,
        name: &str,
        size: u64,
        fileid: &str,
        sign: &Option<(String, String)>,
        source: &cloudmount_core::provider::UploadSource<'_>,
    ) -> Result<InitUploadResp> {
        let cipher = self.ecdh_cipher()?;
        let target = format!("U_1_{dir_id}");
        let app_ver = self.current_app_version();
        // **秒级时间戳**（fake115uploader 同款；毫秒会 token invalid）
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (sign_key, sign_val) = match sign {
            Some((k, v)) => (k.clone(), v.clone()),
            None => (String::new(), String::new()),
        };

        // 表单（p115client 现行协议）：**含 userkey 字段**，键排序，
        // 空值过滤（sign_key/sign_val 为空时不传）
        let mut form: Vec<(String, String)> = vec![
            ("appid".into(), "0".into()),
            ("appversion".into(), app_ver.to_string()),
            ("fileid".into(), fileid.to_string()),
            ("filename".into(), name.to_string()),
            ("filesize".into(), size.to_string()),
            (
                "sig".into(),
                ec115::generate_signature(&ctx.user_id.to_string(), &ctx.userkey, fileid, &target),
            ),
            ("t".into(), t.to_string()),
            ("target".into(), target.clone()),
            (
                "token".into(),
                ec115::generate_token(
                    &ctx.user_id.to_string(),
                    fileid,
                    &t.to_string(),
                    &size.to_string(),
                    &sign_key,
                    &sign_val,
                    &app_ver,
                ),
            ),
            ("topupload".into(), "true".into()),
            ("userid".into(), ctx.user_id.to_string()),
            ("userkey".into(), ctx.userkey.clone()),
        ];
        if !sign_key.is_empty() && !sign_val.is_empty() {
            form.push(("sign_key".into(), sign_key));
            form.push(("sign_val".into(), sign_val));
        }
        form.sort_by(|a, b| a.0.cmp(&b.0));

        tracing::debug!(target: "cm_initupload", form = ?form, "initupload 表单");
        // ECDH 加密表单体
        let body = form
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencode_component(v)))
            .collect::<Vec<_>>()
            .join("&");
        let encrypted = cipher.encrypt(body.as_bytes());
        let k_ec = cipher.encode_token(t as i64);

        let cookie = self.auth.cookie_header()?;
        self.pace()?;
        let url = format!("{}{INITUPLOAD_PATH}?k_ec={k_ec}", self.upload_base);
        // initupload UA：三客户端家族合一（p115client 现行）
        let disk_ua_agent: ureq::Agent = ureq::Agent::config_builder()
            .user_agent(format!(
                "Mozilla/5.0 115disk/{app_ver} 115Browser/{app_ver} 115wangpan_android/{app_ver}"
            ))
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(120)))
            .build()
            .into();
        let resp = disk_ua_agent
            .post(&url)
            .header("Cookie", &cookie)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(&encrypted)
            .map_err(|e| Error::Transport(format!("initupload 失败: {e}")))?;
        let raw = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| Error::Transport(format!("读取 initupload 响应失败: {e}")))?;
        let plain = cipher.decrypt(&raw)?;
        tracing::debug!(target: "cm_ec115", resp = %String::from_utf8_lossy(&plain), "initupload 解密响应");
        let mut resp: InitUploadResp = serde_json::from_slice(&plain)
            .map_err(|e| Error::Other(format!("initupload 响应解析失败: {e}")))?;

        // sign_check（status 7）：按区间计算 SHA1 填入 sign_val（调用方重试）
        if resp.status == 7 {
            if let Some(check) = resp.sign_check.clone() {
                if let Some((start, end)) = parse_range(&check) {
                    let bytes = read_source_range(source, start, end)?;
                    resp.sign_val = Some(sha1_hex_upper(&bytes));
                }
            }
        }
        Ok(resp)
    }

    /// OSS 直传（STS token + V1 签名 + 回调头）。
    fn oss_put(
        &self,
        init: &InitUploadResp,
        source: &cloudmount_core::provider::UploadSource<'_>,
    ) -> Result<()> {
        // STS token
        let cookie = self.auth.cookie_header()?;
        self.pace()?;
        let token_url = format!("{}{GETTOKEN_PATH}", self.upload_base);
        let resp = self
            .agent
            .get(&token_url)
            .header("Cookie", &cookie)
            .call()
            .map_err(|e| Error::Transport(format!("gettoken 失败: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取 token 响应失败: {e}")))?;
        tracing::debug!(target: "cm_oss_token", raw = %text, "gettoken 响应");
        let token: OssStsToken = serde_json::from_str(&text)
            .map_err(|e| Error::Other(format!("token 响应解析失败: {e}")))?;
        if token.status_code != "200" {
            return Err(Error::Auth(format!(
                "OSS token 获取失败: {}",
                token.status_code
            )));
        }

        let bucket = init
            .bucket
            .as_deref()
            .ok_or_else(|| Error::Other("initupload 缺少 bucket".into()))?;
        let object = init
            .object
            .as_deref()
            .ok_or_else(|| Error::Other("initupload 缺少 object".into()))?;
        let callback = init
            .callback
            .as_ref()
            .ok_or_else(|| Error::Other("initupload 缺少 callback".into()))?;
        let callback_str = callback.callback.as_str();
        let callback_var = callback.callback_var.as_str();

        use base64::Engine;
        let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
        let cb_b64 = b64(callback_str);
        let cbv_b64 = b64(callback_var);

        // OSS V1 签名（CanonicalizedOSSHeaders 按名称排序）
        let date = http_date_now();
        let string_to_sign = format!(
            "PUT\n\n\n{date}\nx-oss-callback:{cb_b64}\nx-oss-callback-var:{cbv_b64}\nx-oss-security-token:{}\n/{bucket}/{object}",
            token.security_token
        );
        let signature = {
            use hmac::{Hmac, Mac};
            use sha1::Sha1;
            let mut mac = Hmac::<Sha1>::new_from_slice(token.access_key_secret.as_bytes())
                .expect("HMAC 接受任意长度密钥");
            mac.update(string_to_sign.as_bytes());
            base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
        };

        let url = render_oss_url(&self.oss_base, bucket, object);
        self.pace()?;
        let req = self
            .agent
            .put(&url)
            .header("Date", &date)
            .header("x-oss-security-token", &token.security_token)
            .header("x-oss-callback", &cb_b64)
            .header("x-oss-callback-var", &cbv_b64)
            .header(
                "Authorization",
                &format!("OSS {}:{}", token.access_key_id, signature),
            )
            .header("User-Agent", "aliyun-sdk-android/2.9.1");
        let resp = match source {
            cloudmount_core::provider::UploadSource::Bytes(b) => req
                .send(*b)
                .map_err(|e| Error::Transport(format!("OSS 直传失败: {e}")))?,
            cloudmount_core::provider::UploadSource::Path(p) => {
                let f = std::fs::File::open(p).map_err(|e| {
                    Error::Io(std::io::Error::new(e.kind(), format!("打开失败: {e}")))
                })?;
                req.send(f)
                    .map_err(|e| Error::Transport(format!("OSS 直传失败: {e}")))?
            }
        };
        // F-11：OSS PUT 非 2xx 必须显式失败（数据丢失防护闸门）——
        // agent 以 http_status_as_error(false) 构建，状态码只能在此显式检查；
        // 5xx 走 is_retryable 轮内重试，4xx 快速失败。
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            // 体为 OSS XML 错误（不含凭据）；io::Read::take 限长读前 2KB 作摘要
            let mut raw = Vec::new();
            let _ = resp
                .into_body()
                .into_reader()
                .take(2048)
                .read_to_end(&mut raw);
            return Err(Error::Http {
                status,
                body: format!("OSS 直传失败: {status} {}", String::from_utf8_lossy(&raw)),
            });
        }
        let _ = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取上传回执失败: {e}")))?;
        Ok(())
    }
}

/// 上传上下文。
struct UploadCtx {
    user_id: i64,
    userkey: String,
}

/// initupload 响应（ECDH 解密后的明文 JSON）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct InitUploadResp {
    /// 2=秒传命中 1=未命中（走 OSS） 7=需 sign_check。
    #[serde(default)]
    status: i64,
    /// 服务端错误码（0 或 701 为正常）。
    #[serde(default, rename = "statuscode")]
    status_code: i64,
    #[serde(default, rename = "statusmsg")]
    status_msg: String,
    /// 诊断字段（验证走列表，不读 pickcode）。
    #[serde(default)]
    #[allow(dead_code)]
    pickcode: Option<String>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    object: Option<String>,
    #[serde(default)]
    callback: Option<CallbackInfo>,
    #[serde(default, rename = "sign_key")]
    sign_key: Option<String>,
    /// sign_check 区间（"start-end"）。
    #[serde(default, rename = "sign_check")]
    sign_check: Option<String>,
    /// 计算出的 sign_val（本端填充）。
    #[serde(skip)]
    sign_val: Option<String>,
}

/// initupload 的回调信息（嵌套对象）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct CallbackInfo {
    #[serde(default)]
    callback: String,
    #[serde(default, rename = "callback_var")]
    callback_var: String,
}

/// OSS STS token 响应。
#[derive(Debug, serde::Deserialize)]
struct OssStsToken {
    #[serde(rename = "AccessKeyId")]
    access_key_id: String,
    #[serde(rename = "AccessKeySecret")]
    access_key_secret: String,
    #[serde(rename = "SecurityToken")]
    security_token: String,
    #[serde(rename = "StatusCode")]
    status_code: String,
}

/// 解析 sign_check 区间（"start-end"）。
fn parse_range(spec: &str) -> Option<(u64, u64)> {
    let (s, e) = spec.split_once('-')?;
    Some((s.parse().ok()?, e.parse().ok()?))
}

/// OSS 直传 URL 渲染：对模板占位 `{bucket}`/`{object}` 一次性分段替换。
///
/// 值原样替换（与原 `format!` 语义一致，不做百分号编码）；
/// 默认模板渲染与拆分前 `format!("https://{bucket}.{OSS_ENDPOINT}/{object}")`
/// 逐字节一致（等价回归单测锁定）。
fn render_oss_url(template: &str, bucket: &str, object: &str) -> String {
    let mut out = String::with_capacity(template.len() + bucket.len() + object.len());
    let mut rest = template;
    for (placeholder, value) in [("{bucket}", bucket), ("{object}", object)] {
        if let Some((head, tail)) = rest.split_once(placeholder) {
            out.push_str(head);
            out.push_str(value);
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// RFC 1123 GMT 时间（OSS 签名用）。
fn http_date_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    // 手工格式化（避免 time crate 的格式描述符构造）
    let days = now.div_euclid(86400);
    let secs_of_day = now.rem_euclid(86400);
    let (h, m, s) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    // 1970-01-01 是周四
    const WD: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let wd = WD[(days % 7) as usize];
    // 儒略日算法（civil_from_days）
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    const MON: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{wd}, {d:02} {mon} {y:04} {h:02}:{m:02}:{s:02} GMT",
        mon = MON[(mth - 1) as usize]
    )
}

/// 校验 web 写操作响应信封。
fn check_ok(value: serde_json::Value, what: &str) -> Result<()> {
    let env = Envelope::from_value(value)?;
    if !env.is_ok() {
        return Err(Error::Remote {
            code: env.code(),
            message: format!("{what}: {}", env.message.clone().unwrap_or_default()),
        });
    }
    Ok(())
}

/// 表单值百分号编码（initupload 的 ECDH 明文体）。
fn urlencode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl Provider for Cookie115Provider {
    fn name(&self) -> &'static str {
        "115-cookie"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        let cid = if dir.0.is_empty() { "0" } else { &dir.0 };
        let mut out: Vec<RemoteEntry> = Vec::new();
        let mut offset: i64 = 0;
        for page in 1..=MAX_PAGES {
            let (files, count) = self.list_page(cid, offset)?;
            if files.is_empty() {
                break;
            }
            offset += files.len() as i64;
            out.extend(files.iter().map(|f| f.to_entry()));
            if count > 0 && offset >= count {
                break;
            }
            let _ = page;
        }
        Ok(out)
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        // files/get_info?fid=…（文件）；目录走父列表查找不可行（无父信息），
        // v1 用 get_info 尽力而为
        let url = format!("{}/files/get_info?fid={}", self.webapi_base, id.0);
        let value = self.get_json(&url)?;
        tracing::debug!(target: "cm_cookie_stat", raw = %value, "get_info 响应");
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::NotFound(format!(
                "stat {} 失败: {}",
                id.0,
                env.message.clone().unwrap_or_default()
            )));
        }
        let data = env.into_value()?;
        // data 可能是数组（get_info 批量语义）
        let item = if data.is_array() {
            data.as_array()
                .and_then(|a| a.first())
                .cloned()
                .ok_or_else(|| Error::NotFound(format!("stat {} 无数据", id.0)))?
        } else {
            data
        };
        let f: WebFile = serde_json::from_value(item)
            .map_err(|e| Error::Other(format!("stat 响应解析失败: {e}")))?;
        Ok(f.to_entry())
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        // pickcode 优先取缓存（list 填充）；未命中才走 get_info（实测常返回
        // 990002 参数错误，故缓存是主路径）
        let pick = self
            .picks
            .lock()
            .ok()
            .and_then(|p| p.get(&id.0).cloned())
            .or_else(|| self.stat(id).ok().and_then(|e| e.pick_code))
            .ok_or_else(|| Error::Invalid(format!("{} 无 pickcode（目录不可下载）", id.0)))?;
        let url = self.download_url(&pick)?;
        // 直链请求需带浏览器 UA（CDN 校验）
        let resp = self
            .agent
            .get(&url)
            .header(
                "Range",
                &format!("bytes={}-{}", range.start, range.end.unwrap_or(u64::MAX)),
            )
            .call()
            .map_err(|e| Error::Transport(format!("下载失败: {e}")))?;
        Ok(Box::new(resp.into_body().into_reader()))
    }

    fn quota(&self) -> Result<Quota> {
        // index_info：已用量（count/size）；总量该接口不提供
        let url = format!("{}/files/index_info", self.webapi_base);
        let value = self.get_json(&url)?;
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::Remote {
                code: env.code(),
                message: "quota 查询失败".into(),
            });
        }
        let used = env
            .into_value()?
            .get("size")
            .and_then(|s| {
                s.as_str()
                    .and_then(|v| v.parse::<u64>().ok())
                    .or_else(|| s.as_u64())
            })
            .unwrap_or(0);
        Ok(Quota {
            total: 0,
            used,
            remaining: 0,
        })
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        let pid = if parent.0.is_empty() { "0" } else { &parent.0 };
        let url = format!("{}/files/add", self.webapi_base);
        let value = self.post_form_json(&url, &[("pid", pid), ("cname", name)])?;
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::Remote {
                code: env.code(),
                message: format!("创建目录失败: {}", env.message.clone().unwrap_or_default()),
            });
        }
        let cid = env
            .into_value()?
            .get("cid")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .ok_or_else(|| Error::Other("创建目录响应缺少 cid".into()))?;
        Ok(RemoteEntry {
            id: RemoteId(cid),
            parent_id: parent.clone(),
            name: name.to_string(),
            kind: EntryKind::Directory,
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
        source: cloudmount_core::provider::UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        let hashes = hashes_for(&source)?;
        if hashes.size != size {
            tracing::warn!(
                expected = size,
                actual = hashes.size,
                "大小不一致，以实际为准"
            );
        }
        let ctx = self.upload_info()?;
        let dir_id = if parent.0.is_empty() { "0" } else { &parent.0 };

        let mut sign: Option<(String, String)> = None;
        for _ in 1..=MAX_SIGN_ATTEMPTS {
            let init = self.init_upload(
                &ctx,
                dir_id,
                name,
                hashes.size,
                &hashes.fileid,
                &sign,
                &source,
            )?;
            match init.status {
                // 秒传命中：零传输
                2 => return self.verify_upload(dir_id, &hashes.fileid, name, true),
                // sign_check 二次认证：回传区间 SHA1 后重试
                7 => {
                    let key = init
                        .sign_key
                        .clone()
                        .filter(|k| !k.is_empty())
                        .ok_or_else(|| Error::Other("sign_check 缺少 sign_key".into()))?;
                    let val = init
                        .sign_val
                        .clone()
                        .ok_or_else(|| Error::Other("sign_check 区间计算失败".into()))?;
                    tracing::info!(%name, "115 要求上传二次认证，回传区间 SHA1 后重试");
                    sign = Some((key, val));
                }
                // 未命中：OSS 直传
                1 => {
                    self.oss_put(&init, &source)?;
                    return self.verify_upload(dir_id, &hashes.fileid, name, false);
                }
                other => {
                    return Err(Error::Remote {
                        code: init.status_code,
                        message: format!("initupload status={other}: {}", init.status_msg),
                    })
                }
            }
        }
        Err(Error::Other(format!(
            "sign_check 重试超过 {MAX_SIGN_ATTEMPTS} 次"
        )))
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        let url = format!("{}/files/batch_rename", self.webapi_base);
        let key = format!("files_new_name[{}]", id.0);
        let value = self.post_form_json(
            &url,
            &[
                ("fid", id.0.as_str()),
                ("file_name", new_name),
                (key.as_str(), new_name),
            ],
        )?;
        check_ok(value, "重命名失败")
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        let pid = if new_parent.0.is_empty() {
            "0"
        } else {
            &new_parent.0
        };
        let url = format!("{}/files/move", self.webapi_base);
        let value = self.post_form_json(&url, &[("pid", pid), ("fid[0]", id.0.as_str())])?;
        check_ok(value, "移动失败")
    }

    fn copy(&self, _id: &RemoteId, _new_parent: &RemoteId) -> Result<RemoteId> {
        let pid = if _new_parent.0.is_empty() {
            "0"
        } else {
            &_new_parent.0
        };
        let url = format!("{}/files/copy", self.webapi_base);
        let value = self.post_form_json(&url, &[("pid", pid), ("fid[0]", _id.0.as_str())])?;
        check_ok(value, "复制失败")?;
        // 复制是异步的：列表查找新副本（同名）
        let entries = self.list(_new_parent)?;
        let name = self.stat(_id).ok().map(|e| e.name);
        if let Some(name) = name {
            if let Some(found) = entries.iter().find(|e| e.name == name && e.id.0 != _id.0) {
                return Ok(found.id.clone());
            }
        }
        Err(Error::Other(
            "复制已提交但未找到新副本（异步生效中）".into(),
        ))
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        // 进回收站（与官方通道语义一致：可恢复）
        let url = format!("{}/rb/delete", self.webapi_base);
        let value = self.post_form_json(&url, &[("fid[0]", id.0.as_str())])?;
        check_ok(value, "删除失败")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudmount_auth::cookie::CookieJar;
    use cloudmount_auth::store::{MemoryStore, SecretKind, SecretStore};

    #[test]
    fn default_upload_rendering_is_byte_equivalent() {
        // F-14 等价回归锚：默认渲染与拆分前常量（:40/:42）及 :605 format! 逐字节一致
        assert_eq!(
            format!("{UPLOAD_BASE_DEFAULT}{INITUPLOAD_PATH}"),
            "https://uplb.115.com/4.0/initupload.php"
        );
        assert_eq!(
            format!("{UPLOAD_BASE_DEFAULT}{GETTOKEN_PATH}"),
            "https://uplb.115.com/3.0/gettoken.php"
        );
        assert_eq!(
            render_oss_url(OSS_BASE_TEMPLATE_DEFAULT, "bkt", "obj/path"),
            "https://bkt.cn-shenzhen.oss.aliyuncs.com/obj/path"
        );
    }

    #[test]
    fn provider_defaults_use_production_injection_values() {
        let store = std::sync::Arc::new(MemoryStore::default());
        let jar = CookieJar {
            uid: "u".into(),
            cid: "c".into(),
            seid: "s".into(),
            kid: None,
        };
        store
            .put(SecretKind::CookieJar, &serde_json::to_vec(&jar).unwrap())
            .expect("预置 CookieJar");
        let policy = CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        };
        let p = Cookie115Provider::try_new(policy, CookieAuth::new(policy, store))
            .expect("构造");
        assert_eq!(p.upload_base, "https://uplb.115.com");
        assert_eq!(
            p.oss_base,
            "https://{bucket}.cn-shenzhen.oss.aliyuncs.com/{object}"
        );
        assert_eq!(p.ecdh_remote_pub, None, "默认必须用 115 固定公钥");
    }

    #[test]
    fn web_mtime_parses_shanghai_time() {
        // 文件：日期串（分钟精度 + 带秒两种）
        assert_eq!(web_mtime("2026-10-13 15:04", 0), 1791875040);
        assert_eq!(web_mtime("2026-10-13 15:04:05", 0), 1791875045);
        // 目录：Unix 秒数字串
        assert_eq!(web_mtime("1791901445", 0), 1791901445);
    }

    #[test]
    fn web_mtime_falls_back_to_created() {
        assert_eq!(web_mtime("", 12345), 12345);
        assert_eq!(web_mtime("bad", 12345), 12345);
        assert_eq!(web_mtime("2026-10-13T15:04:05", 99), 99);
    }

    #[test]
    fn http_date_now_format() {
        let d = http_date_now();
        // RFC 1123 形态："Thu, 02 Jan 2026 03:04:05 GMT"
        assert!(d.ends_with(" GMT"), "应以 GMT 结尾: {d}");
        let parts: Vec<&str> = d.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert!(parts[0].ends_with(','));
        assert_eq!(parts[5], "GMT");
        assert!(["Mon,", "Tue,", "Wed,", "Thu,", "Fri,", "Sat,", "Sun,"].contains(&parts[0]));
        assert!(parts[1].len() == 2 && parts[1].chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn parse_range_spec() {
        assert_eq!(parse_range("0-131071"), Some((0, 131071)));
        assert_eq!(parse_range("bad"), None);
        assert_eq!(parse_range("1-"), None);
    }

    #[test]
    fn urlencode_component_safe() {
        assert_eq!(urlencode_component("abc-1_2.3~"), "abc-1_2.3~");
        assert_eq!(urlencode_component("a b/c"), "a%20b%2Fc");
        // 中文文件名
        assert_eq!(urlencode_component("文件"), "%E6%96%87%E4%BB%B6");
    }

    #[test]
    fn init_upload_resp_parsing() {
        let r: InitUploadResp = serde_json::from_str(
            r#"{"status":2,"statuscode":0,"pickcode":"PC1","bucket":"b","object":"o",
                "callback":{"callback":"cb","callback_var":"cv"},
                "sign_key":"k","sign_check":"0-99"}"#,
        )
        .unwrap();
        assert_eq!(r.status, 2);
        assert_eq!(r.bucket.as_deref(), Some("b"));
        assert_eq!(r.callback.as_ref().unwrap().callback, "cb");
        assert_eq!(r.callback.as_ref().unwrap().callback_var, "cv");
        assert_eq!(r.sign_check.as_deref(), Some("0-99"));
    }

    #[test]
    fn oss_sts_token_parsing() {
        let t: OssStsToken = serde_json::from_str(
            r#"{"AccessKeyId":"id","AccessKeySecret":"sec","SecurityToken":"tok","StatusCode":"200"}"#,
        )
        .unwrap();
        assert_eq!(t.access_key_id, "id");
        assert_eq!(t.status_code, "200");
    }

    #[test]
    fn webfile_dir_uses_cid_as_id() {
        // 目录：fid 空，ID=cid，父=pid，t=Unix 秒
        let f = serde_json::from_value::<WebFile>(serde_json::json!({
            "cid": 123, "pid": "0", "n": "docs", "s": "0", "t": "1791901445"
        }))
        .unwrap();
        let e = f.to_entry();
        assert_eq!(e.kind, EntryKind::Directory);
        assert_eq!(e.id.0, "123");
        assert_eq!(e.parent_id.0, "0");
        assert_eq!(e.mtime, 1791901445);
    }

    #[test]
    fn webfile_file_uses_fid_with_cid_parent() {
        // 文件：ID=fid，父=cid，t=日期串（分钟精度）
        let f = serde_json::from_value::<WebFile>(serde_json::json!({
            "fid": "456", "cid": "123", "n": "a.txt", "s": "1024", "pc": "PC1", "sha": "ABC",
            "t": "2026-10-13 15:04"
        }))
        .unwrap();
        let e = f.to_entry();
        assert_eq!(e.kind, EntryKind::File);
        assert_eq!(e.id.0, "456");
        assert_eq!(e.parent_id.0, "123");
        assert_eq!(e.size, 1024);
        assert_eq!(e.pick_code.as_deref(), Some("PC1"));
        // 2026-10-13 15:04 +08:00 → 1791875040
        assert_eq!(e.mtime, 1791875040);
    }
}
