//! 115 Cookie 通道驱动（**补充通道，默认关闭**）。
//!
//! ⚠️ 风险与合规：见 [`cloudmount_auth::cookie`] 模块头部声明。
//! 本驱动仅实现"像正常客户端"的行为（固定 UA、复用 device_id、严格限频），
//! **不做**任何主动风控规避。
//!
//! 相比官方 API 的能力补充：分享/转存、批量操作、app 端私有接口。
//! 仅在用户显式开启 Cookie 通道后可用。

use crate::ratelimit::RateLimiter;
use cloudmount_auth::cookie::{CookieAuth, CookiePolicy};
use cloudmount_core::{
    Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result, UploadResult,
};
use std::io::Read;

/// Cookie 通道驱动。
#[allow(dead_code)] // 骨架：字段将在 M3 实现请求逻辑时启用
pub struct Cookie115Provider {
    /// 认证器（含默认关闭策略校验）。
    auth: CookieAuth,
    /// 限速器（比官方通道更保守，默认 0.5 r/s）。
    limiter: RateLimiter,
}

impl Cookie115Provider {
    /// 创建 Cookie 驱动；`policy` 必须为显式开启且知情同意，否则构造即失败。
    pub fn try_new(policy: CookiePolicy) -> Result<Self> {
        policy.check_enabled()?;
        Ok(Self {
            auth: CookieAuth::new(policy),
            limiter: RateLimiter::new(0.5, 1.0),
        })
    }
}

impl Provider for Cookie115Provider {
    fn name(&self) -> &'static str {
        "115-cookie"
    }

    fn list(&self, _dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        // TODO(M3)：webapi.115.com/files，携带 Cookie；失效时 probe_readonly 判定。
        Err(Error::Unimplemented("cookie list".into()))
    }

    fn stat(&self, _id: &RemoteId) -> Result<RemoteEntry> {
        Err(Error::Unimplemented("cookie stat".into()))
    }

    fn open_range(&self, _id: &RemoteId, _range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        // TODO(M3)：proapi.115.com/app/chrome/downurl 取直链。
        Err(Error::Unimplemented("cookie open_range".into()))
    }

    fn create_dir(&self, _parent: &RemoteId, _name: &str) -> Result<RemoteEntry> {
        Err(Error::Unimplemented("cookie create_dir".into()))
    }

    fn upload(
        &self,
        _parent: &RemoteId,
        _name: &str,
        _source: cloudmount_core::provider::UploadSource<'_>,
        _size: u64,
    ) -> Result<UploadResult> {
        Err(Error::Unimplemented("cookie upload".into()))
    }

    fn rename(&self, _id: &RemoteId, _new_name: &str) -> Result<()> {
        Err(Error::Unimplemented("cookie rename".into()))
    }

    fn move_entry(&self, _id: &RemoteId, _new_parent: &RemoteId) -> Result<()> {
        Err(Error::Unimplemented("cookie move".into()))
    }

    fn copy(&self, _id: &RemoteId, _new_parent: &RemoteId) -> Result<RemoteId> {
        Err(Error::Unimplemented("cookie copy".into()))
    }

    fn delete(&self, _id: &RemoteId) -> Result<()> {
        Err(Error::Unimplemented("cookie delete".into()))
    }

    fn quota(&self) -> Result<Quota> {
        Err(Error::Unimplemented("cookie quota".into()))
    }
}
