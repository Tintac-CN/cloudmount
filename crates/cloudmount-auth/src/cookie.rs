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
//! 2. 凭据存放在**独立容器**，不与用户其它 115 会话共享 SEID；
//! 3. 失效探测只使用**只读、不踢下线**的接口；
//! 4. Cookie 失效时**自动回退**官方 OpenAPI；
//! 5. 不做任何主动风控规避（不伪造设备指纹绕过校验），只"像正常客户端"。
//!
//! 若你不同意上述风险，请不要启用本通道 —— 官方 OpenAPI 已能覆盖同步盘全部核心能力。

use crate::{Authenticator, LoginChallenge, LoginMethod, Session};
use cloudmount_core::{Error, Result};

/// 二维码相关域名。
pub const QRCODE_BASE: &str = "https://qrcodeapi.115.com";
/// 通行证域名。
pub const PASSPORT_BASE: &str = "https://passportapi.115.com";

/// 只读、**不会**使其它设备下线的状态探测端点。
///
/// 对应 `CookieCheck`：`GET my.115.com/?ct=guide&ac=status`。
/// 严禁改用 `passportapi.115.com/app/1.0/web/1.0/check/sso`（会挤掉其它设备）。
pub const READONLY_STATUS_PROBE: &str = "https://my.115.com/?ct=guide&ac=status";

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

/// Cookie 通道认证器。
pub struct CookieAuth {
    /// 使用策略。
    policy: CookiePolicy,
    /// 当前会话 Cookie（`UID/CID/SEID/KID`）。
    cookies: Option<String>,
}

impl CookieAuth {
    /// 创建 Cookie 认证器。
    pub fn new(policy: CookiePolicy) -> Self {
        Self {
            policy,
            cookies: None,
        }
    }

    /// 使用**只读**接口探测会话是否仍有效（绝不触发互踢）。
    pub fn probe_readonly(&self) -> Result<bool> {
        self.policy.check_enabled()?;
        // TODO(M3): GET READONLY_STATUS_PROBE，解析登录态；不调用 check/sso。
        Err(Error::Unimplemented("CookieAuth::probe_readonly".into()))
    }
}

impl Authenticator for CookieAuth {
    fn begin_login(&self) -> Result<LoginChallenge> {
        self.policy.check_enabled()?;
        // TODO(M3):
        //   1. GET {QRCODE_BASE}/api/1.0/web/1.0/token/  → uid/time/sign
        //   2. 展示 {QRCODE_BASE}/api/1.0/web/1.0/qrcode?uid=...
        Err(Error::Unimplemented("CookieAuth::begin_login".into()))
    }

    fn poll_login(&self, _challenge: &LoginChallenge) -> Result<Option<Session>> {
        self.policy.check_enabled()?;
        // TODO(M3):
        //   1. 轮询 {QRCODE_BASE}/get/status/（0待扫/1已扫/2已确认/-1过期/-2取消）
        //   2. POST {PASSPORT_BASE}/app/1.0/web/1.0/login/qrcode 换 Cookie
        Err(Error::Unimplemented("CookieAuth::poll_login".into()))
    }

    fn refresh(&self) -> Result<()> {
        // Cookie 通道无 refresh 概念；失效即重新扫码或回退官方 API。
        Ok(())
    }

    fn session(&self) -> Option<Session> {
        self.cookies.as_ref().map(|_| Session {
            method: LoginMethod::Cookie,
            account: None,
            valid: true,
        })
    }
}
