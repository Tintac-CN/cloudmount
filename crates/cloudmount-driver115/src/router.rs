//! Provider 路由：官方 API 为主，Cookie 通道为兜底。
//!
//! 切换策略：
//! - 官方通道**认证失效**（token 失效且刷新失败）→ 若用户已开启 Cookie
//!   通道则自动切换，并在 UI 常驻提示"正在使用补充通道（有协议风险）"；
//! - Cookie 通道**认证失效**（只读探测失败）→ 自动回退官方通道，提示
//!   重新扫码；
//! - **只对认证类错误切换**：频控（退避重试）与网络错误（两通道同病）
//!   不切换；
//! - 每次切换**必须可见**（tracing::warn + [`ProviderRouter::last_switch`]
//!   供 UI 常驻角标查询）——永不静默。

use cloudmount_core::{
    provider::UploadSource, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result,
    UploadResult,
};
use std::io::Read;
use std::sync::Mutex;
use std::sync::RwLock;

/// 当前活动通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveChannel {
    /// 官方 OpenAPI。
    Open,
    /// Cookie 补充通道。
    Cookie,
}

impl ActiveChannel {
    /// 另一通道。
    fn other(self) -> Self {
        match self {
            ActiveChannel::Open => ActiveChannel::Cookie,
            ActiveChannel::Cookie => ActiveChannel::Open,
        }
    }
}

/// 一次通道切换的记录（UI 常驻提示用）。
#[derive(Debug, Clone)]
pub struct ChannelSwitch {
    pub from: ActiveChannel,
    pub to: ActiveChannel,
    pub reason: String,
    /// Unix 秒。
    pub at: i64,
}

/// 双通道路由器。
///
/// 持有 trait 对象（官方/Cookie 通道的具体类型由调用方装箱；测试可注入
/// 桩 Provider 验证切换逻辑）。
pub struct ProviderRouter {
    /// 官方通道（始终存在）。
    open: Box<dyn Provider>,
    /// Cookie 通道（可选）。
    cookie: Option<Box<dyn Provider>>,
    /// 当前活动通道（内部可变：Provider 方法都是 &self）。
    active: RwLock<ActiveChannel>,
    /// 最近一次切换（供 UI 常驻提示）。
    last_switch: Mutex<Option<ChannelSwitch>>,
}

impl ProviderRouter {
    /// 创建路由器（官方为主通道；`cookie` 存在即视为用户已显式开启）。
    pub fn new(open: Box<dyn Provider>, cookie: Option<Box<dyn Provider>>) -> Self {
        Self {
            open,
            cookie,
            active: RwLock::new(ActiveChannel::Open),
            last_switch: Mutex::new(None),
        }
    }

    /// 当前通道。
    pub fn active(&self) -> ActiveChannel {
        self.active
            .read()
            .map(|g| *g)
            .unwrap_or(ActiveChannel::Open)
    }

    /// 最近一次切换（UI 常驻"正在使用补充通道"角标的数据源）。
    pub fn last_switch(&self) -> Option<ChannelSwitch> {
        self.last_switch.lock().ok().and_then(|g| g.clone())
    }

    /// Cookie 通道是否可用。
    pub fn has_cookie(&self) -> bool {
        self.cookie.is_some()
    }

    /// 指定通道的 Provider 引用。
    fn channel(&self, c: ActiveChannel) -> Option<&dyn Provider> {
        match c {
            ActiveChannel::Open => Some(self.open.as_ref()),
            ActiveChannel::Cookie => self.cookie.as_deref(),
        }
    }

    /// 记录并应用切换（可见，不静默）。
    fn switch(&self, to: ActiveChannel, reason: &str) {
        let from = self.active();
        if from == to {
            return;
        }
        let record = ChannelSwitch {
            from,
            to,
            reason: reason.to_string(),
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        };
        if let Ok(mut g) = self.last_switch.lock() {
            *g = Some(record.clone());
        }
        if let Ok(mut a) = self.active.write() {
            *a = to;
        }
        let risk = if to == ActiveChannel::Cookie {
            "（补充通道有协议风险，UI 应常驻提示）"
        } else {
            ""
        };
        tracing::warn!(
            from = ?from,
            to = ?to,
            reason = %reason,
            "通道自动切换 {risk}"
        );
    }

    /// 带自动回退的调用：当前通道认证失效 → 切到另一通道重试一次。
    ///
    /// 只对 [`Error::Auth`] 切换（频控退避、网络错误不切换——两通道同病）。
    fn call<T>(&self, f: impl Fn(&dyn Provider) -> Result<T>) -> Result<T> {
        let current = self.active();
        let provider = self
            .channel(current)
            .ok_or_else(|| Error::Auth("当前通道不可用".into()))?;
        match f(provider) {
            Err(Error::Auth(reason)) => {
                // 认证失效：尝试另一通道（一次，不乒乓）
                let other = current.other();
                if let Some(other_provider) = self.channel(other) {
                    self.switch(other, &reason);
                    f(other_provider)
                } else {
                    Err(Error::Auth(reason))
                }
            }
            other => other,
        }
    }
}

/// 把 Provider 实现为"转发到当前通道 + 认证失效自动切换"，
/// 上层引擎只看到一个 Provider。
impl Provider for ProviderRouter {
    fn name(&self) -> &'static str {
        "115-router"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.call(|p| p.list(dir))
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        self.call(|p| p.stat(id))
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        self.call(|p| p.open_range(id, range))
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        self.call(|p| p.create_dir(parent, name))
    }

    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        self.call(|p| p.upload(parent, name, source.clone(), size))
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        self.call(|p| p.rename(id, new_name))
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        self.call(|p| p.move_entry(id, new_parent))
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        self.call(|p| p.copy(id, new_parent))
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        self.call(|p| p.delete(id))
    }

    fn quota(&self) -> Result<Quota> {
        self.call(|p| p.quota())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudmount_core::{ChangeProbe, EntryKind};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 桩的 `change_probe` 可编程行为（四态）。
    enum ProbeBehavior {
        /// 预置 `Some(ChangeProbe)`（支持探测）。
        Preset(ChangeProbe),
        /// 显式配置"不支持探测"（`Ok(None)`，与 trait 默认实现同形）。
        None,
        /// 非认证类错误（透传面，T-04）。
        NonAuthErr(Error),
        /// 认证失效错误（切换面，T-26）。
        AuthErr(String),
    }

    /// 桩调用计数（`Arc` 共享：`ProviderRouter` 拿走 `Box<dyn Provider>`
    /// 后，测试仍可经此句柄读取各方法调用次数）。
    #[derive(Clone, Default)]
    struct StubCounts {
        list: Arc<AtomicUsize>,
        probe: Arc<AtomicUsize>,
    }

    impl StubCounts {
        fn list_calls(&self) -> usize {
            self.list.load(Ordering::SeqCst)
        }
        fn probe_calls(&self) -> usize {
            self.probe.load(Ordering::SeqCst)
        }
    }

    /// 桩 Provider：可配置每个方法的返回。
    struct Stub {
        entries: Vec<RemoteEntry>,
        fail_with: Option<Error>,
        /// `change_probe` 可编程行为（默认 [`ProbeBehavior::None`]，
        /// 与 trait 默认实现 `Ok(None)` 观测等价，既有测试不受影响）。
        probe: ProbeBehavior,
        /// 调用计数（共享句柄见 [`StubCounts`]）。
        counts: StubCounts,
    }

    impl Stub {
        fn new(entries: Vec<RemoteEntry>, fail_with: Option<Error>, probe: ProbeBehavior) -> Box<Self> {
            Box::new(Self {
                entries,
                fail_with,
                probe,
                counts: StubCounts::default(),
            })
        }
        fn ok(entries: Vec<RemoteEntry>) -> Box<Self> {
            Self::new(entries, None, ProbeBehavior::None)
        }
        fn auth_fail(msg: &str) -> Box<Self> {
            Self::new(
                Vec::new(),
                Some(Error::Auth(msg.into())),
                ProbeBehavior::None,
            )
        }
        /// `change_probe` 返回预置探测值（其余方法同空目录 ok）。
        /// 返回共享计数句柄供测试断言。
        fn probe_some(p: ChangeProbe) -> (Box<Self>, StubCounts) {
            let s = Self::new(Vec::new(), None, ProbeBehavior::Preset(p));
            let counts = s.counts.clone();
            (s, counts)
        }
        /// `change_probe` 显式返回 `None`（不支持探测）。
        fn probe_none() -> (Box<Self>, StubCounts) {
            let s = Self::new(Vec::new(), None, ProbeBehavior::None);
            let counts = s.counts.clone();
            (s, counts)
        }
        /// `change_probe` 返回非认证类错误。
        fn probe_err(e: Error) -> (Box<Self>, StubCounts) {
            let s = Self::new(Vec::new(), None, ProbeBehavior::NonAuthErr(e));
            let counts = s.counts.clone();
            (s, counts)
        }
        /// `change_probe` 返回认证失效错误。
        fn probe_auth_err(msg: &str) -> (Box<Self>, StubCounts) {
            let s = Self::new(Vec::new(), None, ProbeBehavior::AuthErr(msg.into()));
            let counts = s.counts.clone();
            (s, counts)
        }
    }

    impl Provider for Stub {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn list(&self, _dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
            self.counts.list.fetch_add(1, Ordering::SeqCst);
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(self.entries.clone()),
            }
        }
        fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(RemoteEntry {
                    id: id.clone(),
                    parent_id: RemoteId("0".into()),
                    name: "stub".into(),
                    kind: EntryKind::File,
                    size: 1,
                    mtime: 0,
                    sha1: None,
                    pick_code: None,
                    deleted: false,
                }),
            }
        }
        fn open_range(&self, _: &RemoteId, _: RangeSpec) -> Result<Box<dyn Read + Send>> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(Box::new(std::io::Cursor::new(vec![1u8]))),
            }
        }
        fn create_dir(&self, _: &RemoteId, name: &str) -> Result<RemoteEntry> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(RemoteEntry {
                    id: RemoteId("d1".into()),
                    parent_id: RemoteId("0".into()),
                    name: name.into(),
                    kind: EntryKind::Directory,
                    size: 0,
                    mtime: 0,
                    sha1: None,
                    pick_code: None,
                    deleted: false,
                }),
            }
        }
        fn upload(
            &self,
            _: &RemoteId,
            name: &str,
            _: UploadSource<'_>,
            _: u64,
        ) -> Result<UploadResult> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(UploadResult {
                    id: RemoteId(format!("id-{name}")),
                    rapid_upload: true,
                }),
            }
        }
        fn rename(&self, _: &RemoteId, _: &str) -> Result<()> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(()),
            }
        }
        fn move_entry(&self, _: &RemoteId, _: &RemoteId) -> Result<()> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(()),
            }
        }
        fn copy(&self, _: &RemoteId, _: &RemoteId) -> Result<RemoteId> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(RemoteId("copy".into())),
            }
        }
        fn delete(&self, _: &RemoteId) -> Result<()> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(()),
            }
        }
        fn quota(&self) -> Result<Quota> {
            match &self.fail_with {
                Some(e) => Err(clone_err(e)),
                None => Ok(Quota {
                    total: 100,
                    used: 10,
                    remaining: 90,
                }),
            }
        }
        fn change_probe(&self, _dir: &RemoteId) -> Result<Option<ChangeProbe>> {
            self.counts.probe.fetch_add(1, Ordering::SeqCst);
            match &self.probe {
                ProbeBehavior::Preset(p) => Ok(Some(*p)),
                ProbeBehavior::None => Ok(None),
                ProbeBehavior::NonAuthErr(e) => Err(clone_err(e)),
                ProbeBehavior::AuthErr(m) => Err(Error::Auth(m.clone())),
            }
        }
    }

    /// Error 不实现 Clone——手工复制（测试专用）。
    fn clone_err(e: &Error) -> Error {
        match e {
            Error::Auth(m) => Error::Auth(m.clone()),
            Error::Other(m) => Error::Other(m.clone()),
            Error::Transport(m) => Error::Transport(m.clone()),
            Error::RateLimited { retry_after_ms } => Error::RateLimited {
                retry_after_ms: *retry_after_ms,
            },
            _ => Error::Other("测试桩错误类型".into()),
        }
    }

    fn entry(id: &str) -> RemoteEntry {
        RemoteEntry {
            id: RemoteId(id.into()),
            parent_id: RemoteId("0".into()),
            name: format!("n-{id}"),
            kind: EntryKind::File,
            size: 1,
            mtime: 0,
            sha1: None,
            pick_code: None,
            deleted: false,
        }
    }

    #[test]
    fn official_auth_failure_fails_over_to_cookie() {
        let router = ProviderRouter::new(
            Stub::auth_fail("token 失效"),
            Some(Stub::ok(vec![entry("c1")])),
        );
        // 官方认证失效 → 自动切 Cookie → 成功
        let list = router.list(&RemoteId("0".into())).expect("应切换后成功");
        assert_eq!(list.len(), 1);
        assert_eq!(router.active(), ActiveChannel::Cookie);
        // 切换可见：last_switch 有记录
        let sw = router.last_switch().expect("切换应被记录");
        assert_eq!(sw.from, ActiveChannel::Open);
        assert_eq!(sw.to, ActiveChannel::Cookie);
    }

    #[test]
    fn cookie_auth_failure_falls_back_to_open() {
        let router = ProviderRouter::new(
            Stub::ok(vec![entry("o1")]),
            Some(Stub::auth_fail("Cookie 失效")),
        );
        // 手动切到 cookie（模拟官方先失效过）
        router.switch(ActiveChannel::Cookie, "测试前置");
        let list = router.list(&RemoteId("0".into())).expect("应回退后成功");
        assert_eq!(list.len(), 1);
        assert_eq!(router.active(), ActiveChannel::Open);
    }

    #[test]
    fn no_cookie_channel_auth_error_surfaces() {
        let router = ProviderRouter::new(Stub::auth_fail("token 失效"), None);
        let err = router.list(&RemoteId("0".into())).unwrap_err();
        assert!(format!("{err}").contains("token 失效"));
        assert_eq!(router.active(), ActiveChannel::Open);
    }

    #[test]
    fn rate_limited_does_not_switch() {
        // 频控错误不切换（退避重试是正解）
        let open = Stub::new(
            Vec::new(),
            Some(Error::RateLimited {
                retry_after_ms: 1000,
            }),
            ProbeBehavior::None,
        );
        let router = ProviderRouter::new(open, Some(Stub::ok(vec![entry("c1")])));
        let err = router.list(&RemoteId("0".into())).unwrap_err();
        assert!(matches!(err, Error::RateLimited { .. }));
        assert_eq!(router.active(), ActiveChannel::Open, "频控不应切换通道");
    }

    #[test]
    fn both_channels_fail_returns_second_error() {
        let router = ProviderRouter::new(
            Stub::auth_fail("官方失效"),
            Some(Stub::auth_fail("Cookie 失效")),
        );
        let err = router.list(&RemoteId("0".into())).unwrap_err();
        assert!(format!("{err}").contains("Cookie 失效"));
    }

    #[test]
    fn normal_operation_no_switch() {
        let router = ProviderRouter::new(Stub::ok(vec![entry("o1")]), Some(Stub::ok(vec![])));
        let list = router.list(&RemoteId("0".into())).expect("官方正常");
        assert_eq!(list.len(), 1);
        assert_eq!(router.active(), ActiveChannel::Open);
        assert!(router.last_switch().is_none());
    }

    #[test]
    fn failover_persists_for_subsequent_calls() {
        let router = ProviderRouter::new(
            Stub::auth_fail("token 失效"),
            Some(Stub::ok(vec![entry("c1")])),
        );
        // 第一次调用触发切换
        router.list(&RemoteId("0".into())).expect("切换成功");
        // 后续调用直接走 Cookie（无感）
        assert_eq!(router.active(), ActiveChannel::Cookie);
        let q = router.quota().expect("后续调用走 Cookie");
        assert_eq!(q.total, 100);
    }

    // ---- F-02：change_probe 转发（T-03 / T-04 / T-26）----

    /// T-03（正）：`ProviderRouter::change_probe` 转发到**活跃通道**——
    /// 预置 Some 探测值原样透传、活跃通道计数 +1、另一通道不触达、
    /// `list` 计数不增长（快路径确实生效，非回退全量）。
    #[test]
    fn provider_router_change_probe_forwards_to_active_channel() {
        let open_probe = ChangeProbe {
            count: 11,
            max_mtime: 111,
        };
        let cookie_probe = ChangeProbe {
            count: 22,
            max_mtime: 222,
        };
        let (open, open_counts) = Stub::probe_some(open_probe);
        let (cookie, cookie_counts) = Stub::probe_some(cookie_probe);
        let router = ProviderRouter::new(open, Some(cookie));

        let dir = RemoteId("0".into());
        let probe = router
            .change_probe(&dir)
            .expect("探测不应报错")
            .expect("活跃通道预置探测值应原样透传，而非默认实现吞成 None");

        assert_eq!(probe, open_probe, "应返回活跃通道（Open）的探测值");
        assert_eq!(open_counts.probe_calls(), 1, "活跃通道 change_probe 应恰好转发一次");
        assert_eq!(cookie_counts.probe_calls(), 0, "非活跃通道不应被触达");
        assert_eq!(open_counts.list_calls(), 0, "探测路径不得回退全量 list");
        assert_eq!(cookie_counts.list_calls(), 0);
    }

    /// T-03（None 分支，r2 pm MIN-6）：Stub 配置 `change_probe` 返回 `None`
    /// → 路由层**原样透传 `None`**（引擎侧回退全量列表为既有行为，本点仅断言透传）。
    #[test]
    fn provider_router_change_probe_none_passthrough() {
        let (open, _open_counts) = Stub::probe_none();
        let (cookie, _cookie_counts) = Stub::probe_some(ChangeProbe {
            count: 7,
            max_mtime: 7,
        });
        let router = ProviderRouter::new(open, Some(cookie));

        let probe = router
            .change_probe(&RemoteId("0".into()))
            .expect("探测不应报错");
        assert_eq!(probe, None, "配置 None 应原样透传 None");
    }

    /// T-04（负）：活跃通道 `change_probe` 返回**非认证类** Err → 路由层
    /// 原样透传错误（不吞成 `None`、不触发切换、不回退全量 list）。
    #[test]
    fn change_probe_non_auth_error_passes_through() {
        let (open, open_counts) = Stub::probe_err(Error::Transport("网络中断".into()));
        let router = ProviderRouter::new(open, Some(Stub::ok(vec![entry("c1")])));

        let err = router
            .change_probe(&RemoteId("0".into()))
            .expect_err("非认证类错误应原样透传，而非被默认实现吞成 None");
        assert!(
            matches!(err, Error::Transport(ref m) if m.contains("网络中断")),
            "错误应原样透传（不吞成 None、不换形）：{err:?}"
        );
        assert_eq!(router.active(), ActiveChannel::Open, "非认证类错误不应切换通道");
        assert!(router.last_switch().is_none(), "非认证类错误不应记录切换");
        assert_eq!(open_counts.probe_calls(), 1, "应确经活跃通道透传（仅一次）");
        assert_eq!(open_counts.list_calls(), 0, "不得静默回退全量 list");
    }

    /// T-26（负）：活跃通道 `change_probe` 返回**认证失效**（Error::Auth）
    /// → 走与其它方法一致的通道切换：`last_switch()` 记录（from/to/reason）、
    /// 后续调用路由到切换后通道、错误不静默吞成 `None`。
    #[test]
    fn change_probe_auth_error_switches_channel() {
        let preset = ChangeProbe {
            count: 42,
            max_mtime: 777,
        };
        let (open, open_counts) = Stub::probe_auth_err("token 失效");
        let (cookie, cookie_counts) = Stub::probe_some(preset);
        let router = ProviderRouter::new(open, Some(cookie));

        let dir = RemoteId("0".into());
        let first = router
            .change_probe(&dir)
            .expect("Auth 失效切换后应成功，而非吞成 None");
        assert_eq!(first, Some(preset), "应返回切换后通道（Cookie）的探测值，不吞成 None");

        let sw = router.last_switch().expect("Auth 失效应记录切换");
        assert_eq!(sw.from, ActiveChannel::Open);
        assert_eq!(sw.to, ActiveChannel::Cookie);
        assert!(sw.reason.contains("token 失效"), "切换原因应携带认证失效信息：{}", sw.reason);
        assert_eq!(router.active(), ActiveChannel::Cookie);

        // 后续调用路由到切换后通道（无感）
        let second = router
            .change_probe(&dir)
            .expect("后续调用应走切换后通道");
        assert_eq!(second, Some(preset));
        assert_eq!(open_counts.probe_calls(), 1, "官方通道不应再被探测");
        assert_eq!(cookie_counts.probe_calls(), 2, "首次切换重试 + 后续调用都应命中 Cookie");
    }
}
