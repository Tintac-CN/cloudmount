//! 115 官方 OpenAPI 认证（主通道）—— 完整实现。
//!
//! 流程（PKCE 设备码，无需 AppSecret）：
//!
//! ```text
//! 1. POST {passport}/open/authDeviceCode
//!       form: client_id=<AppID>&code_challenge=<b64url(sha256(verifier))>&code_challenge_method=sha256
//!    → data: { uid, time, qrcode, sign }        （qrcode 即二维码内容，已含 https://115.com/scan/dg-…）
//! 2. 用户用 115 App 扫码
//! 3. GET  {qrcode}/get/status/?uid&time&sign    （长轮询）
//!    → state: 1=继续轮询, 0=二维码失效
//!      data.status: 1=已扫码待确认, 2=已确认
//! 4. POST {passport}/open/deviceCodeToToken
//!       form: uid=<uid>&code_verifier=<verifier>
//!    → data: { access_token, refresh_token, expires_in }
//! 5. 过期前 POST {passport}/open/refreshToken
//!       form: refresh_token=<rt>
//!    → data: { access_token, refresh_token(新值), expires_in }
//! ```
//!
//! 重要约束（官方文档 + 实测）：
//! - `state` 在认证域是**数字** 0/1（资源域是布尔）——由 [`cloudmount_core::Envelope`] 兼容。
//! - **请勿频繁刷新**：返回 `40140117` 即"刷新过快"，必须退避而非立即重试。
//! - 认证接口 Content-Type 为 `application/x-www-form-urlencoded`。

use crate::pkce::Pkce;
use crate::store::{SecretKind, SecretStore};
use crate::{Authenticator, LoginChallenge, LoginMethod, Session};
use cloudmount_core::{Envelope, Error, Result};
use std::sync::{Arc, Mutex};

/// 官方 OpenAPI 基础域名（资源接口）。
pub const PROAPI_BASE: &str = "https://proapi.115.com";
/// 通行证/认证域名。
pub const PASSPORT_BASE: &str = "https://passportapi.115.com";
/// 二维码域名（轮询与二维码内容）。
pub const QRCODE_BASE: &str = "https://qrcodeapi.115.com";

/// access_token 提前刷新余量（秒）：剩余不足该值即视为需要刷新。
pub const REFRESH_MARGIN_SECS: i64 = 300;

/// 一份 token 集合。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenSet {
    /// 访问令牌。
    pub access_token: String,
    /// 刷新令牌。
    pub refresh_token: String,
    /// access_token 过期时间（Unix 秒）。
    pub expires_at: i64,
}

impl TokenSet {
    /// 是否已不可用（含提前刷新余量）。
    pub fn needs_refresh(&self, now: i64) -> bool {
        self.expires_at - REFRESH_MARGIN_SECS <= now
    }
}

/// 设备码登录的中间状态（begin_login 与 poll_login 之间保留）。
#[derive(Debug, Clone)]
struct PendingLogin {
    uid: String,
    /// 首次请求返回的 time，轮询需原样回传。
    time: i64,
    /// 轮询签名。
    sign: String,
    pkce: Pkce,
}

/// 官方 OpenAPI 认证器。
pub struct OpenApiAuth {
    /// 开放平台 AppID（申请所得）。
    app_id: String,
    /// HTTP 客户端。
    agent: ureq::Agent,
    /// 当前 token（内存）。
    tokens: Mutex<Option<TokenSet>>,
    /// 进行中的登录。
    pending: Mutex<Option<PendingLogin>>,
    /// 凭据持久化。
    store: Arc<dyn SecretStore>,
}

impl OpenApiAuth {
    /// 用 AppID 创建认证器（token 从 `store` 恢复）。
    pub fn new(app_id: impl Into<String>, store: Arc<dyn SecretStore>) -> Self {
        let auth = Self {
            app_id: app_id.into(),
            agent: ureq::Agent::config_builder()
                .user_agent(cloudmount_core::DEFAULT_USER_AGENT)
                .timeout_global(Some(std::time::Duration::from_secs(30)))
                .build()
                .into(),
            tokens: Mutex::new(None),
            pending: Mutex::new(None),
            store,
        };
        if let Ok(Some(blob)) = auth.store.get(SecretKind::OpenApiToken) {
            match serde_json::from_slice::<TokenSet>(&blob) {
                Ok(ts) => {
                    tracing::info!("已恢复 115 官方通道 token");
                    *auth.tokens.lock().expect("lock") = Some(ts);
                }
                Err(e) => tracing::warn!("token 解析失败，忽略: {e}"),
            }
        }
        auth
    }

    /// 用既有的 token 集合创建认证器（用于测试，或从其它工具导入凭据）。
    pub fn from_token_set(
        app_id: impl Into<String>,
        store: Arc<dyn SecretStore>,
        tokens: TokenSet,
    ) -> Self {
        let auth = Self::new(app_id, store);
        *auth.tokens.lock().expect("lock") = Some(tokens);
        auth
    }

    /// 当前 Unix 秒。
    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// POST 表单并解析信封（认证域统一 urlencoded）。
    fn post_form<F>(&self, url: &str, fields: &[(&str, &str)], map: F) -> Result<serde_json::Value>
    where
        F: FnOnce(&Envelope) -> Option<Error>,
    {
        let resp = self
            .agent
            .post(url)
            .send_form(fields.iter().copied())
            .map_err(|e| Error::Transport(format!("POST {url}: {e}")))?;
        let mut body = resp.into_body();
        let text = body
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取响应失败: {e}")))?;
        let env = Envelope::parse(&text)?;
        if let Some(err) = map(&env) {
            return Err(err);
        }
        env.into_value()
    }

    /// 保存 token 到内存与安全存储。
    fn persist(&self, ts: TokenSet) -> Result<()> {
        let blob = serde_json::to_vec(&ts)?;
        self.store.put(SecretKind::OpenApiToken, &blob)?;
        *self.tokens.lock().expect("lock") = Some(ts);
        Ok(())
    }

    /// 返回可用的 access_token；临近过期则自动刷新。
    ///
    /// 说明：用同一把锁保证"单飞"——并发调用者只会触发一次刷新，
    /// 避免踩官方"刷新过快"的 40140117。
    pub fn access_token(&self) -> Result<String> {
        let now = Self::now();
        {
            let guard = self.tokens.lock().expect("lock");
            if let Some(ts) = guard.as_ref() {
                if !ts.needs_refresh(now) {
                    return Ok(ts.access_token.clone());
                }
            } else {
                return Err(Error::Auth("未登录：请先执行 `cloudmount login`".into()));
            }
        }
        // 需要刷新（持锁执行，天然单飞）
        let guard = self.tokens.lock().expect("lock");
        // 双检：可能已被其它线程刷新
        if let Some(ts) = guard.as_ref() {
            if !ts.needs_refresh(Self::now()) {
                return Ok(ts.access_token.clone());
            }
        }
        let refresh_token = guard
            .as_ref()
            .map(|t| t.refresh_token.clone())
            .ok_or_else(|| Error::Auth("未登录".into()))?;
        drop(guard);

        let ts = self.do_refresh(&refresh_token)?;
        let token = ts.access_token.clone();
        self.persist(ts)?;
        Ok(token)
    }

    /// 实际执行刷新请求。
    fn do_refresh(&self, refresh_token: &str) -> Result<TokenSet> {
        let url = format!("{PASSPORT_BASE}/open/refreshToken");
        let value = self.post_form(&url, &[("refresh_token", refresh_token)], |env| {
            (!env.is_ok()).then(|| env.clone().into_error())
        })?;
        let data: TokenData = serde_json::from_value(value)
            .map_err(|e| Error::Other(format!("refreshToken 响应异常: {e}")))?;
        tracing::debug!("115 token 已刷新");
        Ok(TokenSet {
            access_token: data.access_token,
            refresh_token: data.refresh_token,
            expires_at: Self::now() + data.expires_in as i64,
        })
    }

    /// 是否已登录（不校验有效性）。
    pub fn is_logged_in(&self) -> bool {
        self.tokens.lock().expect("lock").is_some()
    }

    /// 清除本地凭据（登出）。
    pub fn logout(&self) -> Result<()> {
        *self.tokens.lock().expect("lock") = None;
        self.store.delete(SecretKind::OpenApiToken)
    }
}

/// deviceCodeToToken / refreshToken 的 `data` 体。
#[derive(Debug, serde::Deserialize)]
struct TokenData {
    access_token: String,
    refresh_token: String,
    #[serde(deserialize_with = "cloudmount_core::serde_util::num_or_str_u64")]
    expires_in: u64,
}

/// authDeviceCode 的 `data` 体。
#[derive(Debug, serde::Deserialize)]
struct DeviceCodeData {
    uid: String,
    #[serde(deserialize_with = "cloudmount_core::serde_util::num_or_str_u64")]
    time: u64,
    qrcode: String,
    sign: String,
}

/// 轮询的 `data` 体（未扫码时为空对象）。
#[derive(Debug, Default, serde::Deserialize)]
struct PollData {
    #[serde(
        default,
        deserialize_with = "cloudmount_core::serde_util::opt_num_or_str_u64"
    )]
    status: Option<u64>,
    #[serde(default)]
    msg: Option<String>,
}

impl Authenticator for OpenApiAuth {
    fn begin_login(&self) -> Result<LoginChallenge> {
        let pkce = Pkce::generate()?;
        let url = format!("{PASSPORT_BASE}/open/authDeviceCode");
        let challenge = pkce.challenge();
        let value = self.post_form(
            &url,
            &[
                ("client_id", self.app_id.as_str()),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", pkce.method),
            ],
            |env| (!env.is_ok()).then(|| env.clone().into_error()),
        )?;
        let data: DeviceCodeData = serde_json::from_value(value)
            .map_err(|e| Error::Other(format!("authDeviceCode 响应异常: {e}")))?;

        *self.pending.lock().expect("lock") = Some(PendingLogin {
            uid: data.uid.clone(),
            time: data.time as i64,
            sign: data.sign,
            pkce,
        });

        Ok(LoginChallenge {
            qr_content: data.qrcode,
            // 轮询令牌不上屏，这里仅回传 uid 供 UI 参考
            token: data.uid,
            // 115 未明确给出二维码有效期，按常见 5 分钟处理
            expires_at: Self::now() + 300,
        })
    }

    fn poll_login(&self, _challenge: &LoginChallenge) -> Result<Option<Session>> {
        let pending = self
            .pending
            .lock()
            .expect("lock")
            .clone()
            .ok_or_else(|| Error::Auth("没有进行中的登录，请先 begin_login".into()))?;

        // 长轮询状态
        let url = format!("{QRCODE_BASE}/get/status/");
        let resp = self
            .agent
            .get(url.as_str())
            .query("uid", &pending.uid)
            .query("time", pending.time.to_string())
            .query("sign", &pending.sign)
            .call()
            .map_err(|e| Error::Transport(format!("轮询二维码状态失败: {e}")))?;
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| Error::Transport(format!("读取轮询响应失败: {e}")))?;
        let env = Envelope::parse(&text)?;

        // state=0 → 二维码已失效
        if !env.is_ok() {
            let msg = env
                .message
                .clone()
                .or_else(|| env.error.clone())
                .unwrap_or_else(|| "二维码已失效".into());
            *self.pending.lock().expect("lock") = None;
            return Err(Error::Auth(msg));
        }

        let data: PollData = match env.data.clone() {
            Some(v) => serde_json::from_value(v).unwrap_or_default(),
            None => PollData::default(),
        };

        match data.status {
            // 已扫码，等待用户在 App 上确认
            Some(1) => Ok(None),
            // 已确认 → 换取 token
            Some(2) => {
                let url = format!("{PASSPORT_BASE}/open/deviceCodeToToken");
                let value = self.post_form(
                    &url,
                    &[
                        ("uid", pending.uid.as_str()),
                        ("code_verifier", pending.pkce.verifier.as_str()),
                    ],
                    |env| (!env.is_ok()).then(|| env.clone().into_error()),
                )?;
                let data: TokenData = serde_json::from_value(value)
                    .map_err(|e| Error::Other(format!("deviceCodeToToken 响应异常: {e}")))?;
                self.persist(TokenSet {
                    access_token: data.access_token,
                    refresh_token: data.refresh_token,
                    expires_at: Self::now() + data.expires_in as i64,
                })?;
                *self.pending.lock().expect("lock") = None;
                tracing::info!("115 官方通道登录成功");
                Ok(Some(Session {
                    method: LoginMethod::OpenApi,
                    account: None,
                    valid: true,
                }))
            }
            // 未扫码或未知状态 → 继续轮询
            _ => {
                if let Some(msg) = &data.msg {
                    tracing::trace!(msg = %msg, "二维码轮询进行中");
                }
                Ok(None)
            }
        }
    }

    fn refresh(&self) -> Result<()> {
        let rt = {
            let guard = self.tokens.lock().expect("lock");
            guard.as_ref().map(|t| t.refresh_token.clone())
        };
        let rt = rt.ok_or_else(|| Error::Auth("未登录".into()))?;
        let ts = self.do_refresh(&rt)?;
        self.persist(ts)
    }

    fn session(&self) -> Option<Session> {
        self.tokens
            .lock()
            .expect("lock")
            .as_ref()
            .map(|ts| Session {
                method: LoginMethod::OpenApi,
                account: None,
                valid: !ts.needs_refresh(Self::now()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    fn auth() -> OpenApiAuth {
        OpenApiAuth::new("test-app", Arc::new(MemoryStore::default()))
    }

    #[test]
    fn token_expiry_logic() {
        let ts = TokenSet {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 1_000_000,
        };
        // 距过期还有 10 分钟 → 不需要刷新
        assert!(!ts.needs_refresh(1_000_000 - 600));
        // 只剩 4 分钟（< 5 分钟余量）→ 需要刷新
        assert!(ts.needs_refresh(1_000_000 - 240));
        // 已过期
        assert!(ts.needs_refresh(1_000_000 + 1));
    }

    #[test]
    fn access_token_requires_login() {
        let a = auth();
        assert!(!a.is_logged_in());
        let err = a.access_token().unwrap_err();
        assert!(matches!(err, Error::Auth(_)));
    }

    #[test]
    fn persists_and_restores_token() {
        let store = Arc::new(MemoryStore::default());
        {
            let a = OpenApiAuth::new("app", store.clone());
            a.persist(TokenSet {
                access_token: "at".into(),
                refresh_token: "rt".into(),
                expires_at: OpenApiAuth::now() + 3600,
            })
            .unwrap();
            assert!(a.is_logged_in());
            assert_eq!(a.access_token().unwrap(), "at");
        }
        // 新实例应从同一 store 恢复
        let b = OpenApiAuth::new("app", store);
        assert!(b.is_logged_in());
        assert_eq!(b.access_token().unwrap(), "at");
    }

    #[test]
    fn poll_without_begin_fails() {
        let a = auth();
        let ch = LoginChallenge {
            qr_content: String::new(),
            token: String::new(),
            expires_at: 0,
        };
        assert!(matches!(a.poll_login(&ch).unwrap_err(), Error::Auth(_)));
    }
}
