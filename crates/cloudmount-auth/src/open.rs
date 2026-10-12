//! 115 官方 OpenAPI 认证（主通道）。
//!
//! 走 OAuth2 **PKCE 设备码**流程（无后端客户端无需 AppSecret）：
//!
//! ```text
//! 1. POST https://passportapi.115.com/open/authDeviceCode   → 取二维码/设备码
//! 2. 用户用 115 App 扫码
//! 3. POST https://passportapi.115.com/open/deviceCodeToToken → 换 access_token + refresh_token
//! 4. 后续请求带 Authorization: Bearer <access_token>
//! 5. 过期前 POST https://passportapi.115.com/open/refreshToken 刷新
//! ```
//!
//! ## 重要约束（来自官方文档与社区实测）
//! - `access_token` 约 2 小时有效；`refresh_token` 约 1 年，**刷新会返回新值且不延长有效期**。
//! - **请勿频繁刷新**，否则会被列入频控；本模块需做**单飞 + 节流**。
//! - 同一 AppID 存在并发登录数限制，新登录可能挤掉旧会话。
//!
//! 当前为骨架：常量与类型已就绪，网络调用见里程碑 M0/M3。

use crate::{Authenticator, LoginChallenge, LoginMethod, Session};
use cloudmount_core::{Error, Result};

/// 官方 OpenAPI 基础域名。
pub const PROAPI_BASE: &str = "https://proapi.115.com";
/// 通行证/认证域名。
pub const PASSPORT_BASE: &str = "https://passportapi.115.com";
/// 二维码状态轮询域名。
pub const QRCODE_BASE: &str = "https://qrcodeapi.115.com";

/// access_token 典型有效期（秒），用于提前刷新。
pub const ACCESS_TOKEN_TTL_SECS: i64 = 7200;

/// 官方 OpenAPI 认证器。
#[allow(dead_code)] // 骨架：字段将在 M0 实现刷新/请求逻辑时启用
pub struct OpenApiAuth {
    /// 开放平台 AppID（申请所得）。
    app_id: String,
    /// 当前 access_token。
    access_token: Option<String>,
    /// refresh_token。
    refresh_token: Option<String>,
    /// access_token 过期时间（Unix 秒）。
    expires_at: i64,
}

impl OpenApiAuth {
    /// 用 AppID 创建认证器。
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            access_token: None,
            refresh_token: None,
            expires_at: 0,
        }
    }

    /// 生成 PKCE `code_verifier`（随机 43–128 字符）。
    fn code_verifier() -> String {
        // TODO(M0): 使用 OS CSPRNG（getrandom）生成。
        unimplemented!("PKCE code_verifier generation")
    }

    /// 计算 PKCE `code_challenge` = BASE64URL(SHA256(verifier))。
    fn code_challenge(verifier: &str) -> String {
        use sha1::Digest;
        let mut h = sha1::Sha1::new();
        h.update(verifier.as_bytes());
        hex::encode(h.finalize())
    }
}

impl Authenticator for OpenApiAuth {
    fn begin_login(&self) -> Result<LoginChallenge> {
        // TODO(M0): POST {PASSPORT_BASE}/open/authDeviceCode
        //           参数含 app_id、code_challenge；返回二维码内容与轮询 token。
        let _ = (Self::code_verifier(), Self::code_challenge(""));
        Err(Error::Unimplemented("OpenApiAuth::begin_login".into()))
    }

    fn poll_login(&self, _challenge: &LoginChallenge) -> Result<Option<Session>> {
        // TODO(M0): 轮询 deviceCodeToToken，成功后持久化 token。
        Err(Error::Unimplemented("OpenApiAuth::poll_login".into()))
    }

    fn refresh(&self) -> Result<()> {
        // TODO(M0): POST {PASSPORT_BASE}/open/refreshToken（单飞 + 节流）。
        Err(Error::Unimplemented("OpenApiAuth::refresh".into()))
    }

    fn session(&self) -> Option<Session> {
        self.access_token.as_ref().map(|_| Session {
            method: LoginMethod::OpenApi,
            account: None,
            valid: true,
        })
    }
}
