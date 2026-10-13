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

use crate::m115;
use crate::ratelimit::RateLimiter;
use cloudmount_auth::cookie::{CookieAuth, CookiePolicy};
use cloudmount_core::serde_util::num_or_str_u64;
use cloudmount_core::{
    EntryKind, Envelope, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result,
    UploadResult,
};
use std::io::Read;

/// web 资源域。
pub const WEBAPI_BASE: &str = "https://webapi.115.com";
/// pro 资源域（下载直链，m115 加密）。
pub const PROAPI_BASE: &str = "https://proapi.115.com";

/// 列表分页大小（web 接口上限约 1150，保守取 1000）。
const PAGE_LIMIT: i64 = 1000;

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
        })
    }

    /// 注入资源域地址（集成测试 mock 服务端）。
    pub fn with_base_urls(mut self, webapi: impl Into<String>, proapi: impl Into<String>) -> Self {
        self.webapi_base = webapi.into().trim_end_matches('/').to_string();
        self.proapi_base = proapi.into().trim_end_matches('/').to_string();
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

    /// 只读探测（不踢下线）；失效返回 Ok(false)。
    pub fn probe(&self) -> Result<bool> {
        self.auth.probe_readonly()
    }
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
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::NotFound(format!("stat {} 失败", id.0)));
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
        // RemoteId 为 fid；下载需 pickcode——从 stat 取
        let entry = self.stat(id)?;
        let pick = entry
            .pick_code
            .clone()
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

    fn create_dir(&self, _parent: &RemoteId, _name: &str) -> Result<RemoteEntry> {
        Err(Error::Unimplemented("cookie create_dir（M3 v2）".into()))
    }

    fn upload(
        &self,
        _parent: &RemoteId,
        _name: &str,
        _source: cloudmount_core::provider::UploadSource<'_>,
        _size: u64,
    ) -> Result<UploadResult> {
        Err(Error::Unimplemented(
            "cookie upload（M3 v2：秒传+OSS 直传）".into(),
        ))
    }

    fn rename(&self, _id: &RemoteId, _new_name: &str) -> Result<()> {
        Err(Error::Unimplemented("cookie rename（M3 v2）".into()))
    }

    fn move_entry(&self, _id: &RemoteId, _new_parent: &RemoteId) -> Result<()> {
        Err(Error::Unimplemented("cookie move（M3 v2）".into()))
    }

    fn copy(&self, _id: &RemoteId, _new_parent: &RemoteId) -> Result<RemoteId> {
        Err(Error::Unimplemented("cookie copy（M3 v2）".into()))
    }

    fn delete(&self, _id: &RemoteId) -> Result<()> {
        Err(Error::Unimplemented("cookie delete（M3 v2）".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
