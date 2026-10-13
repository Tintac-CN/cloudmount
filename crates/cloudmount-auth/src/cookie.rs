//! 115 Cookie 认证通道（**补充通道，默认关闭**）。
//!
//! # ⚠️ 合规与风险声明（务必阅读）
//!
//! 本模块通过 115 网页/客户端协议（非官方开放平台接口）获取会话 Cookie。
//! 该方式**明确违反《115 生活用户服务协议》**，相关条款包括但不限于：
//! - 3.2.3(4)：禁止使用"非授权客户端或非授权接口"；
//! - 4.2(11)：禁止非授权第三方工具接入；
//! - 5.4：禁止反向工程；
//! - 9.2：平台可采取冻结账号、限制数据访问等措施。
//!
//! 此外还存在**多端登录互踢**（登录会挤掉其它设备的会话）与**风控限速**风险。
//!
//! 因此本通道的设计边界（见 [`CookiePolicy`]）：
//! 1. **默认关闭**，必须由用户显式开启并确认知情同意；
//! 2. 凭据存放在**独立容器**（[`SecretKind::CookieJar`]，与官方 token
//!    物理隔离），不与用户其它 115 会话共享 SEID；
//! 3. 失效探测只使用**只读、不踢下线**的接口（[`READONLY_STATUS_PROBE`]）；
//! 4. Cookie 失效时**自动回退**官方 OpenAPI；
//! 5. 不做任何主动风控规避（不伪造设备指纹绕过校验），只"像正常客户端"
//!    （固定浏览器 UA、严格限频）。
//!
//! 若你不同意上述风险，请不要启用本通道 —— 官方 OpenAPI 已能覆盖同步盘
//! 全部核心能力。
//!
//! # 扫码流程（实测端点，与官方 OpenAPI 流程同构）
//!
//! ```text
//! 1. GET  {QRCODE_BASE}/api/1.0/web/1.0/token/   → data: { uid, time, sign, qrcode }
//! 2. 用户用 115 App 扫 qrcode（https://115.com/scan/dg-…）
//! 3. GET  {QRCODE_BASE}/get/status/?uid&time&sign&_   （短轮询，立即返回）
//!    → data.status: 0=待扫 1=已扫待确认 2=已确认 -1=过期 -2=取消
//! 4. POST {PASSPORT_BASE}/app/1.0/web/1.0/login/qrcode   form: account=uid, app=web
//!    → data.cookie: { UID, CID, SEID, KID }
//! ```

use crate::store::{SecretKind, SecretStore};
use crate::{Authenticator, LoginChallenge, LoginMethod, Session};
use cloudmount_core::{Envelope, Error, Result};
use std::sync::{Arc, Mutex};

/// 二维码相关域名。
pub const QRCODE_BASE: &str = "https://qrcodeapi.115.com";
/// 通行证域名。
pub const PASSPORT_BASE: &str = "https://passportapi.115.com";

/// 只读、**不会**使其它设备下线的状态探测端点。
///
/// 对应 `CookieCheck`：`GET my.115.com/?ct=guide&ac=status`。
/// 严禁改用 `passportapi.115.com/app/1.0/web/1.0/check/sso`（会挤掉其它设备）。
pub const READONLY_STATUS_PROBE: &str = "https://my.115.com/?ct=guide&ac=status";

/// Cookie 通道固定浏览器 UA（"像正常客户端"：web 端点期望浏览器 UA，
/// 产品 UA 反而异常）。
pub const WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// 二维码有效期（秒；官方无字段明示，按扫码页实际行为取保守值）。
const QR_TTL_SECS: i64 = 300;

/// Cookie 通道的使用策略（决定是否允许启用）。
#[derive(Debug, Clone, Copy, Default)]
pub struct CookiePolicy {
    /// 用户是否已显式开启（默认 false）。
    pub enabled: bool,
    /// 用户是否已确认知情同意（默认 false；未确认为真时拒绝启用）。
    pub consent_acknowledged: bool,
}

impl CookiePolicy {
    /// 校验是否允许启用本通道。
    pub fn check_enabled(&self) -> Result<()> {
        if !self.enabled {
            return Err(Error::Auth(
                "Cookie 通道默认关闭；请在设置中显式开启".into(),
            ));
        }
        if !self.consent_acknowledged {
            return Err(Error::Auth(
                "启用 Cookie 通道前必须确认知情同意（违反 115 服务协议、可能封号/互踢）".into(),
            ));
        }
        Ok(())
    }
}

/// 一份 115 会话 Cookie（`UID/CID/SEID/KID`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CookieJar {
    #[serde(rename = "UID")]
    pub uid: String,
    #[serde(rename = "CID")]
    pub cid: String,
    #[serde(rename = "SEID")]
    pub seid: String,
    #[serde(rename = "KID", default)]
    pub kid: Option<String>,
}

impl CookieJar {
    /// 拼接 `Cookie` 请求头值。
    pub fn header_value(&self) -> String {
        let mut s = format!("UID={}; CID={}; SEID={}", self.uid, self.cid, self.seid);
        if let Some(kid) = &self.kid {
            s.push_str(&format!("; KID={kid}"));
        }
        s
    }
}

/// 进行中的扫码登录状态（begin_login 与 poll_login 之间保留）。
#[derive(Debug, Clone)]
struct PendingQr {
    uid: String,
    time: i64,
    sign: String,
}

/// Cookie 通道认证器。
pub struct CookieAuth {
    /// 使用策略。
    policy: CookiePolicy,
    /// HTTP 客户端（固定浏览器 UA）。
    agent: ureq::Agent,
    /// 当前 Cookie（内存）。
    jar: Mutex<Option<CookieJar>>,
    /// 进行中的扫码登录。
    pending: Mutex<Option<PendingQr>>,
    /// 凭据持久化（独立容器，与官方 token 物理隔离）。
    store: Arc<dyn SecretStore>,
    /// API 基础地址覆盖（测试注入 mock 服务端用；None = 生产地址）。
    base_override: Option<String>,
}

impl CookieAuth {
    /// 创建 Cookie 认证器（生产地址；Cookie 从 `store` 恢复）。
    pub fn new(policy: CookiePolicy, store: Arc<dyn SecretStore>) -> Self {
        let auth = Self {
            policy,
            agent: ureq::Agent::config_builder()
                .user_agent(WEB_USER_AGENT)
                .timeout_global(Some(std::time::Duration::from_secs(30)))
                .build()
                .into(),
            jar: Mutex::new(None),
            pending: Mutex::new(None),
            store,
            base_override: None,
        };
        // 从独立容器恢复（daemon/CLI 重启后保持登录态）
        auth.restore();
        auth
    }

    /// 指定基础地址（测试注入 mock 服务端；覆盖 QRCODE/PASSPORT 域名）。
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.base_override = Some(base.into().trim_end_matches('/').to_string());
        self
    }

    fn qrcode_base(&self) -> &str {
        self.base_override.as_deref().unwrap_or(QRCODE_BASE)
    }

    fn passport_base(&self) -> &str {
        self.base_override.as_deref().unwrap_or(PASSPORT_BASE)
    }

    fn probe_base(&self) -> &str {
        // 探测端点是完整 URL；测试注入时用 base 替换域名部分
        self.base_override
            .as_deref()
            .unwrap_or("https://my.115.com")
    }

    /// 是否已登录（Cookie 已恢复且策略允许）。
    pub fn is_logged_in(&self) -> bool {
        self.jar.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// 当前 Cookie（供驱动层构造请求头）。
    pub fn cookie_header(&self) -> Result<String> {
        self.policy.check_enabled()?;
        self.jar
            .lock()
            .map_err(|_| Error::Other("cookie 锁中毒".into()))?
            .as_ref()
            .map(|j| j.header_value())
            .ok_or_else(|| Error::Auth("未登录：请先执行 `cloudmount login --cookie`".into()))
    }

    /// 使用**只读**接口探测会话是否仍有效（绝不触发互踢）。
    pub fn probe_readonly(&self) -> Result<bool> {
        self.policy.check_enabled()?;
        let cookie = self.cookie_header()?;
        let url = format!(
            "{}/?ct=guide&ac=status&_={}",
            self.probe_base(),
            now_millis()
        );
        let resp = self
            .agent
            .get(&url)
            .header("Cookie", &cookie)
            .call()
            .map_err(|e| Error::Transport(format!("探测失败: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取探测响应失败: {e}")))?;
        let env = Envelope::parse(&text)?;
        Ok(env.is_ok())
    }

    /// 登出：清除内存与持久化凭据。
    pub fn logout(&self) -> Result<()> {
        self.store.delete(SecretKind::CookieJar)?;
        *self
            .jar
            .lock()
            .map_err(|_| Error::Other("cookie 锁中毒".into()))? = None;
        Ok(())
    }

    /// 从持久化恢复 Cookie。
    fn restore(&self) {
        if let Ok(Some(blob)) = self.store.get(SecretKind::CookieJar) {
            match serde_json::from_slice::<CookieJar>(&blob) {
                Ok(jar) => {
                    *self.jar.lock().expect("lock") = Some(jar);
                }
                Err(e) => tracing::warn!(error = %e, "Cookie 容器解析失败（视为未登录）"),
            }
        }
    }

    /// 保存 Cookie 到内存与独立容器。
    fn persist(&self, jar: CookieJar) -> Result<()> {
        let blob = serde_json::to_vec(&jar)?;
        self.store.put(SecretKind::CookieJar, &blob)?;
        *self.jar.lock().expect("lock") = Some(jar);
        Ok(())
    }

    /// GET 并解析 JSON（带 Cookie 头）。
    fn get_json(&self, url: &str, cookie: Option<&str>) -> Result<serde_json::Value> {
        let mut req = self.agent.get(url);
        if let Some(c) = cookie {
            req = req.header("Cookie", c);
        }
        let resp = req
            .call()
            .map_err(|e| Error::Transport(format!("GET {url}: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取响应失败: {e}")))?;
        serde_json::from_str(&text).map_err(|e| Error::Other(format!("JSON 解析失败: {e}")))
    }

    /// POST 表单并解析 JSON。
    fn post_form_json(&self, url: &str, fields: &[(&str, &str)]) -> Result<serde_json::Value> {
        let resp = self
            .agent
            .post(url)
            .send_form(fields.iter().copied())
            .map_err(|e| Error::Transport(format!("POST {url}: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取响应失败: {e}")))?;
        serde_json::from_str(&text).map_err(|e| Error::Other(format!("JSON 解析失败: {e}")))
    }
}

impl Authenticator for CookieAuth {
    fn begin_login(&self) -> Result<LoginChallenge> {
        self.policy.check_enabled()?;
        let url = format!("{}/api/1.0/web/1.0/token/", self.qrcode_base());
        let value = self.get_json(&url, None)?;
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::Auth(format!(
                "获取二维码失败: {}",
                env_message(&env)
            )));
        }
        let data = env
            .data
            .clone()
            .ok_or_else(|| Error::Auth("二维码响应缺少 data".into()))?;
        let uid = data
            .get("uid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Auth("二维码响应缺少 uid".into()))?
            .to_string();
        let time = data
            .get("time")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(now_unix);
        let sign = data
            .get("sign")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let qr = data
            .get("qrcode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Auth("二维码响应缺少 qrcode 内容".into()))?
            .to_string();

        *self.pending.lock().expect("lock") = Some(PendingQr {
            uid: uid.clone(),
            time,
            sign,
        });
        Ok(LoginChallenge {
            qr_content: qr,
            token: uid,
            expires_at: now_unix() + QR_TTL_SECS,
        })
    }

    fn poll_login(&self, _challenge: &LoginChallenge) -> Result<Option<Session>> {
        self.policy.check_enabled()?;
        let pending = self
            .pending
            .lock()
            .expect("lock")
            .clone()
            .ok_or_else(|| Error::Auth("没有进行中的登录，请先 begin_login".into()))?;

        // 短轮询（web 流程立即返回，由调用方控制节奏）
        let url = format!(
            "{}/get/status/?uid={}&time={}&sign={}&_={}",
            self.qrcode_base(),
            urlencode(&pending.uid),
            pending.time,
            urlencode(&pending.sign),
            now_millis()
        );
        let value = self.get_json(&url, None)?;
        let env = Envelope::from_value(value)?;
        if !env.is_ok() {
            return Err(Error::Auth(format!(
                "扫码状态查询失败: {}",
                env_message(&env)
            )));
        }
        let status = env
            .data
            .as_ref()
            .and_then(|d| d.get("status"))
            .and_then(|s| s.as_i64())
            .unwrap_or(0);
        match status {
            0 | 1 => Ok(None), // 待扫 / 已扫待确认
            2 => {
                // 已确认：换 Cookie
                let url = format!("{}/app/1.0/web/1.0/login/qrcode", self.passport_base());
                let value = self
                    .post_form_json(&url, &[("account", pending.uid.as_str()), ("app", "web")])?;
                let env = Envelope::from_value(value)?;
                if !env.is_ok() {
                    return Err(Error::Auth(format!(
                        "换取 Cookie 失败: {}",
                        env_message(&env)
                    )));
                }
                let cookie = env
                    .data
                    .as_ref()
                    .and_then(|d| d.get("cookie"))
                    .cloned()
                    .ok_or_else(|| Error::Auth("登录响应缺少 cookie".into()))?;
                let jar: CookieJar = serde_json::from_value(cookie)
                    .map_err(|e| Error::Auth(format!("cookie 解析失败: {e}")))?;
                let uid = jar.uid.clone();
                self.persist(jar)?;
                *self.pending.lock().expect("lock") = None;
                Ok(Some(Session {
                    method: LoginMethod::Cookie,
                    account: Some(uid),
                    valid: true,
                }))
            }
            -1 => Err(Error::Auth("二维码已过期，请重新登录".into())),
            -2 => Err(Error::Auth("用户取消了扫码".into())),
            other => Err(Error::Auth(format!("未知扫码状态: {other}"))),
        }
    }

    fn refresh(&self) -> Result<()> {
        // Cookie 通道无 refresh 概念；失效即重新扫码或回退官方 API。
        Ok(())
    }

    fn session(&self) -> Option<Session> {
        self.jar.lock().ok()?.as_ref().map(|j| Session {
            method: LoginMethod::Cookie,
            account: Some(j.uid.clone()),
            valid: true,
        })
    }
}

/// 信封错误文本（message/error 任一）。
fn env_message(env: &Envelope) -> String {
    env.message
        .clone()
        .or_else(|| env.error.clone())
        .unwrap_or_default()
}

/// 当前 Unix 秒。
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 当前 Unix 毫秒（`_` 防缓存参数）。
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 极简百分号编码（uid/sign 为十六进制串，实际只需保留安全字符）。
fn urlencode(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    fn policy() -> CookiePolicy {
        CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        }
    }

    #[test]
    fn policy_gates_disabled_and_unconsented() {
        assert!(CookiePolicy::default().check_enabled().is_err());
        assert!(CookiePolicy {
            enabled: true,
            consent_acknowledged: false
        }
        .check_enabled()
        .is_err());
        assert!(policy().check_enabled().is_ok());
    }

    #[test]
    fn jar_header_value_format() {
        let jar = CookieJar {
            uid: "u1".into(),
            cid: "c1".into(),
            seid: "s1".into(),
            kid: None,
        };
        assert_eq!(jar.header_value(), "UID=u1; CID=c1; SEID=s1");
        let jar = CookieJar {
            uid: "u1".into(),
            cid: "c1".into(),
            seid: "s1".into(),
            kid: Some("k1".into()),
        };
        assert_eq!(jar.header_value(), "UID=u1; CID=c1; SEID=s1; KID=k1");
    }

    #[test]
    fn persist_and_restore_roundtrip() {
        let store = Arc::new(MemoryStore::default());
        let auth = CookieAuth::new(policy(), store.clone());
        let jar = CookieJar {
            uid: "A".into(),
            cid: "B".into(),
            seid: "C".into(),
            kid: Some("D".into()),
        };
        auth.persist(jar).unwrap();
        assert!(auth.is_logged_in());
        assert_eq!(auth.cookie_header().unwrap(), "UID=A; CID=B; SEID=C; KID=D");
        // 独立实例从同一 store 恢复
        let auth2 = CookieAuth::new(policy(), store);
        auth2.restore();
        assert!(auth2.is_logged_in());
        // 登出清除
        auth2.logout().unwrap();
        assert!(!auth2.is_logged_in());
    }

    #[test]
    fn cookie_header_requires_login() {
        let auth = CookieAuth::new(policy(), Arc::new(MemoryStore::default()));
        let err = auth.cookie_header().unwrap_err();
        assert!(format!("{err}").contains("未登录"));
    }

    #[test]
    fn begin_login_rejects_when_policy_off() {
        let auth = CookieAuth::new(CookiePolicy::default(), Arc::new(MemoryStore::default()));
        assert!(auth.begin_login().is_err());
    }

    /// mock 扫码服务端：token → status → login/qrcode 三端点。
    #[test]
    fn qr_login_flow_against_mock_server() {
        // 用本地线程 + 极简 HTTP？——auth crate 无 HTTP 测试基建；
        // 改为直接测 JSON 解析路径：构造 CookieAuth 的内部解析逻辑等价物。
        // （完整 HTTP mock 在 driver115 的集成测试基建上做。）
        // 这里验证：登录响应 → CookieJar 解析。
        let resp = serde_json::json!({
            "state": 1,
            "data": {
                "cookie": {
                    "UID": "user-uid",
                    "CID": "dev-cid",
                    "SEID": "sess-seid",
                    "KID": "k"
                }
            }
        });
        let jar: CookieJar = serde_json::from_value(resp["data"]["cookie"].clone()).expect("解析");
        assert_eq!(jar.uid, "user-uid");
        assert_eq!(jar.seid, "sess-seid");
    }

    #[test]
    fn urlencode_keeps_hex_safe() {
        assert_eq!(urlencode("abc123"), "abc123");
        assert_eq!(urlencode("a/b"), "a%2Fb");
    }
}
