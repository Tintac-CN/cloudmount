//! # cloudmount-auth
//!
//! 认证与会话管理，两条通道：
//!
//! 1. **官方 OpenAPI（主通道）** —— OAuth2 PKCE 设备码扫码登录，见 [`open`]。
//!    需要 115 开放平台审核的 AppID；无后端客户端用 PKCE，无需 AppSecret。
//! 2. **Cookie 通道（补充通道，默认关闭）** —— 见 [`cookie`]。技术上完整可用，
//!    但**违反 115 用户服务协议**（3.2.3(4)/4.2(11)/5.4），且登录会挤掉其它设备
//!    会话。仅作为官方 API 不可用时的兜底，必须由用户显式开启并知情同意。
//!
//! 凭据持久化见 [`store`]，实现按平台封装系统安全存储
//! （Windows DPAPI / macOS Keychain / Linux Secret Service）。

pub mod cookie;
pub mod open;
pub mod store;

use cloudmount_core::Result;

/// 登录方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMethod {
    /// 官方 OpenAPI（推荐）。
    OpenApi,
    /// Cookie 通道（补充，默认关闭）。
    Cookie,
}

/// 一个可用的会话凭据（脱敏后可供 UI 展示状态）。
#[derive(Debug, Clone)]
pub struct Session {
    /// 登录方式。
    pub method: LoginMethod,
    /// 显示名 / 账号标识。
    pub account: Option<String>,
    /// 是否已登录且未过期。
    pub valid: bool,
}

/// 认证通道统一接口。
pub trait Authenticator: Send + Sync {
    /// 发起登录（返回一个待用户完成的挑战，如二维码）。
    fn begin_login(&self) -> Result<LoginChallenge>;

    /// 轮询登录状态，完成后返回会话。
    fn poll_login(&self, challenge: &LoginChallenge) -> Result<Option<Session>>;

    /// 刷新 token（官方通道）。Cookie 通道通常为 no-op。
    fn refresh(&self) -> Result<()>;

    /// 当前会话。
    fn session(&self) -> Option<Session>;
}

/// 登录挑战（二维码或设备码）。
#[derive(Debug, Clone)]
pub struct LoginChallenge {
    /// 二维码内容（otpauth 风格 URL 或 115 扫码 uid）。
    pub qr_content: String,
    /// 轮询所需的临时凭据（uid/time/sign 等）。
    pub token: String,
    /// 过期时间（Unix 秒）。
    pub expires_at: i64,
}
