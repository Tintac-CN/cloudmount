//! Provider 路由：官方 API 为主，Cookie 通道为兜底。
//!
//! 切换策略：
//! - 官方通道连续失败（认证失效且刷新失败）→ 若用户已开启 Cookie 通道则自动
//!   切换，并在 UI 常驻提示"正在使用补充通道（有协议风险）"。
//! - Cookie 通道失效（只读探测失败）→ 自动回退官方通道，提示重新扫码。
//! - 永不静默升级风险：任何降级/回退都必须对用户可见。

use crate::{cookie::Cookie115Provider, open::Open115Provider};
use cloudmount_core::{
    provider::UploadSource, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result,
    UploadResult,
};
use std::io::Read;

/// 当前活动通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveChannel {
    /// 官方 OpenAPI。
    Open,
    /// Cookie 补充通道。
    Cookie,
}

/// 双通道路由器。
pub struct ProviderRouter {
    /// 官方通道（始终存在）。
    open: Open115Provider,
    /// Cookie 通道（可选）。
    cookie: Option<Cookie115Provider>,
    /// 当前活动通道。
    active: ActiveChannel,
}

impl ProviderRouter {
    /// 创建路由器。
    pub fn new(open: Open115Provider, cookie: Option<Cookie115Provider>) -> Self {
        Self {
            open,
            cookie,
            active: ActiveChannel::Open,
        }
    }

    /// 当前通道。
    pub fn active(&self) -> ActiveChannel {
        self.active
    }

    /// 切换到 Cookie 通道（若未启用则报错）。
    pub fn switch_to_cookie(&mut self) -> Result<()> {
        if self.cookie.is_some() {
            self.active = ActiveChannel::Cookie;
            Ok(())
        } else {
            Err(Error::Auth(
                "Cookie 通道未启用；请在设置中显式开启并知情同意".into(),
            ))
        }
    }

    /// 回退到官方通道。
    pub fn switch_to_open(&mut self) {
        self.active = ActiveChannel::Open;
    }

    /// 活动通道的 [`Provider`] 引用。
    fn current(&self) -> &dyn Provider {
        match self.active {
            ActiveChannel::Open => &self.open,
            ActiveChannel::Cookie => self
                .cookie
                .as_ref()
                .map(|c| c as &dyn Provider)
                .unwrap_or(&self.open),
        }
    }
}

// 把 Provider 实现为"转发到当前通道"，让上层引擎只看到一个 Provider。
impl Provider for ProviderRouter {
    fn name(&self) -> &'static str {
        "115-router"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.current().list(dir)
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        self.current().stat(id)
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        self.current().open_range(id, range)
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        self.current().create_dir(parent, name)
    }

    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        self.current().upload(parent, name, source, size)
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        self.current().rename(id, new_name)
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        self.current().move_entry(id, new_parent)
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        self.current().copy(id, new_parent)
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        self.current().delete(id)
    }

    fn quota(&self) -> Result<Quota> {
        self.current().quota()
    }
}
