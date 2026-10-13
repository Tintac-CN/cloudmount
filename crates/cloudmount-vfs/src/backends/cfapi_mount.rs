//! CfAPI 后端的高层封装：`CfapiBackend` 实现 [`crate::MountBackend`]。
//!
//! 组合低层原语（[`super::cfapi`]）为完整能力：
//!
//! ```text
//! mount(point, remote_view)
//!   1. 注册同步根（幂等）
//!   2. 投影：远端树 → 占位符（create_placeholders_ordered）
//!   3. 连接：水合回调 = rel_path → 远端 ID → provider.open_range（Range 拉取）
//!      ↑ 这正是 115 的 downurl + Range 模式；测试用 MockRemote 验证同一链路
//! project(remote)  增量投影新增条目（内容更新的重投影见 TODO）
//! ```
//!
//! ## 与同步引擎的关系（M2 后续接线）
//!
//! 同步引擎每轮产出远端视图（快照合成或全量拉取）→ `project()` 增量投影；
//! 用户读文件 → 过滤器回调 → Provider 直接按需拉取（不经过同步引擎的下载
//! 队列 —— 按需读取天然绕过全量下载）。

use super::cfapi::{self, PlaceholderSpec};
use crate::{Mount, MountBackend};
use cloudmount_core::{EntryKind, Error, RangeSpec, RemoteEntry, RemoteId, Result, UploadSource};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// CfAPI 挂载后端。
pub struct CfapiBackend {
    provider: Arc<dyn Provider>,
    /// 是否做 Shell 侧注册（Explorer 导航栏/图标；持久跨重启）。
    shell_integration: bool,
}

/// 挂载句柄内部共享状态（回调与控制面共享）。
struct MountState {
    /// 相对路径 → 远端条目（投影与水合的寻址表）。
    remote: RwLock<HashMap<String, RemoteEntry>>,
}

/// CfAPI 挂载句柄。
pub struct CfapiMount {
    root: PathBuf,
    connection: cfapi::CloudConnection,
    state: Arc<MountState>,
    /// 是否启用了 Shell 侧注册（决定卸载时是否同步注销 Shell 身份）。
    shell_integration: bool,
    /// 已投影的相对路径（增量投影用）。
    projected: Mutex<HashSet<String>>,
    /// 水合记录（与连接回调共享；区分"过滤器写入"与"用户修改"）。
    hydration_log: cfapi::HydrationLog,
}

impl CfapiBackend {
    /// 创建后端（`provider` 提供水合数据源）。
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            shell_integration: false,
        }
    }

    /// 启用 Shell 侧注册（Explorer 导航栏/状态图标；持久跨重启）。
    pub fn with_shell_integration(mut self) -> Self {
        self.shell_integration = true;
        self
    }

    /// 挂载并返回具体句柄（暴露 [`CfapiMount::project`] 增量投影）。
    pub fn mount_concrete(
        &self,
        point: &Path,
        remote: &HashMap<String, RemoteEntry>,
    ) -> Result<CfapiMount> {
        // 1. 注册（幂等）：内核过滤层 + 可选 Shell 层（导航栏/图标）
        cfapi::register(point, "CloudMount", cloudmount_core::VERSION)?;
        if self.shell_integration {
            use super::shell_reg;
            let reg = shell_reg::ShellRegistration {
                sync_root: point,
                provider_name: "CloudMount",
                display_name: "CloudMount 云挂",
                version: cloudmount_core::VERSION,
                icon_resource: r"%SystemRoot%\system32\charmap.exe,0",
                identity: "cloudmount-sync-root",
            };
            shell_reg::register_shell(&reg)?;
        }

        // 2. 共享状态（水合回调按 rel_path 寻址）
        let state = Arc::new(MountState {
            remote: RwLock::new(remote.clone()),
        });

        // 3. 投影
        let specs: Vec<PlaceholderSpec> = remote
            .iter()
            .map(|(rel, e)| {
                let mut spec = if e.kind == EntryKind::Directory {
                    PlaceholderSpec::dir(rel, e.mtime)
                } else {
                    PlaceholderSpec::file(rel, e.size, e.mtime)
                };
                spec.identity = rel.as_bytes().to_vec();
                spec
            })
            .collect();
        cfapi::create_placeholders_ordered(point, &specs)?;

        // 4. 连接：水合 = rel_path → ID → Provider Range 拉取
        let provider = self.provider.clone();
        let state_for_cb = state.clone();
        let hydrate: cfapi::HydrateFn = Box::new(move |rel, offset, len| {
            let entry = state_for_cb
                .remote
                .read()
                .map_err(|_| Error::Other("远端视图锁中毒".into()))?
                .get(rel)
                .cloned()
                .ok_or_else(|| Error::NotFound(rel.to_string()))?;
            // 请求范围按远端大小钳制（过滤器可能按块请求超出 EOF 的范围）
            let start = offset.min(entry.size);
            let end = start
                .saturating_add(len.saturating_sub(1))
                .min(entry.size.saturating_sub(1));
            if start > end {
                return Ok(Vec::new());
            }
            let mut reader = provider.open_range(
                &entry.id,
                RangeSpec {
                    start,
                    end: Some(end),
                },
            )?;
            let mut buf = Vec::with_capacity((end - start + 1) as usize);
            reader.read_to_end(&mut buf)?;
            Ok(buf)
        });
        let hydration_log: cfapi::HydrationLog = Arc::new(Mutex::new(HashMap::new()));
        let connection = cfapi::connect_with_log(point, hydrate, hydration_log.clone())?;

        let projected = remote.keys().cloned().collect();
        Ok(CfapiMount {
            root: point.to_path_buf(),
            connection,
            state,
            shell_integration: self.shell_integration,
            projected: Mutex::new(projected),
            hydration_log,
        })
    }
}

impl MountBackend for CfapiBackend {
    fn name(&self) -> &'static str {
        "cfapi"
    }

    fn supports_placeholders(&self) -> bool {
        true
    }

    fn supports_status_icons(&self) -> bool {
        true
    }

    /// 注册 + 投影 + 连接。
    ///
    /// 注意：同步根注册失败（如 CI 虚拟机不支持）返回 Err，调用方可据此降级。
    fn mount(&self, point: &Path, remote: &HashMap<String, RemoteEntry>) -> Result<Box<dyn Mount>> {
        Ok(Box::new(self.mount_concrete(point, remote)?))
    }

    /// 直接按 ID 从 Provider 水合指定范围。
    fn hydrate(&self, id: &RemoteId, range: RangeSpec) -> Result<Vec<u8>> {
        let mut reader = self.provider.open_range(id, range)?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// 释放空间（转为纯占位符）—— M2.5 实现（需 CfDehydratePlaceholder 语义）。
    fn dehydrate(&self, _id: &RemoteId) -> Result<()> {
        Err(Error::Unimplemented("dehydrate（M2.5）".into()))
    }
}

impl CfapiMount {
    /// 同步根路径。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 增量投影：远端视图中新增的路径创建为占位符（已有路径跳过）。
    ///
    /// 同时刷新水合寻址表（新增文件的 ID 对水合回调可见）。
    /// **已知边界**：已有占位符的**内容更新**不会重投影——那需要
    /// `CfUpdatePlaceholders` + 失效通知，属 M2.5。
    pub fn project(&self, remote: &HashMap<String, RemoteEntry>) -> Result<u32> {
        {
            let mut view = self
                .state
                .remote
                .write()
                .map_err(|_| Error::Other("远端视图锁中毒".into()))?;
            for (rel, e) in remote {
                view.insert(rel.clone(), e.clone());
            }
        }

        let mut specs = Vec::new();
        {
            let mut projected = self.projected.lock().map_err(lock_poisoned)?;
            for (rel, e) in remote {
                if projected.contains(rel) {
                    continue;
                }
                let mut spec = if e.kind == EntryKind::Directory {
                    PlaceholderSpec::dir(rel, e.mtime)
                } else {
                    PlaceholderSpec::file(rel, e.size, e.mtime)
                };
                spec.identity = rel.as_bytes().to_vec();
                specs.push(spec);
                projected.insert(rel.clone());
            }
        }
        if specs.is_empty() {
            return Ok(0);
        }
        cfapi::create_placeholders_ordered(&self.root, &specs)
    }

    /// 删除占位符（远端删除传播）。
    ///
    /// 目录递归删除其下全部占位符。
    pub fn remove(&self, rel_path: &str) -> Result<()> {
        let abs = self.abs_of(rel_path);
        if !abs.exists() {
            // 已不存在：顺带清账
            self.unproject(rel_path);
            return Ok(());
        }
        let meta = std::fs::symlink_metadata(&abs)?;
        if meta.is_dir() {
            // 目录：先清子项的投影记录（remove_dir_all 会连带删除）
            let prefix = format!("{rel_path}/");
            if let Ok(view) = self.state.remote.read() {
                for key in view.keys() {
                    if key.starts_with(&prefix) {
                        self.unproject(key);
                    }
                }
            }
            std::fs::remove_dir_all(&abs)?;
        } else {
            std::fs::remove_file(&abs)?;
        }
        self.unproject(rel_path);
        Ok(())
    }

    fn abs_of(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.root.clone()
        } else {
            self.root.join(rel)
        }
    }

    fn unproject(&self, rel: &str) {
        if let Ok(mut p) = self.projected.lock() {
            p.remove(rel);
        }
        if let Ok(mut v) = self.state.remote.write() {
            v.remove(rel);
        }
    }

    // ---------- 双向同步支撑（供 LocalChangeUploader 使用） ----------

    /// 远端视图条目查询（水合寻址与上传父目录解析）。
    pub fn remote_entry(&self, rel: &str) -> Option<RemoteEntry> {
        self.state
            .remote
            .read()
            .ok()
            .and_then(|v| v.get(rel).cloned())
    }

    /// 路径是否已投影（区分"我们的投影"与"用户新建"）。
    pub fn is_projected(&self, rel: &str) -> bool {
        self.projected
            .lock()
            .map(|p| p.contains(rel))
            .unwrap_or(false)
    }

    /// 上传成功后更新视图条目并标记已投影
    /// （防止 PlaceholderSync 把本地新文件重复投影为占位符）。
    pub fn upsert_view_entry(&self, rel: &str, entry: RemoteEntry) {
        if let Ok(mut v) = self.state.remote.write() {
            v.insert(rel.to_string(), entry);
        }
        if let Ok(mut p) = self.projected.lock() {
            p.insert(rel.to_string());
        }
    }

    /// 记录"最近同步时的本地 mtime"（上传成功后调用）。
    ///
    /// 水合记录复用为同步水位：脱水安全检查据此区分"已同步"与
    /// "未上传的本地修改"（用户创建的文件本地 mtime=写入时刻，
    /// 远端 mtime=上传时刻，天然不等，须以本地水位为准）。
    pub fn record_synced_mtime(&self, rel: &str, mtime: i64) {
        if let Ok(mut log) = self.hydration_log.lock() {
            log.insert(rel.to_string(), mtime);
        }
    }

    /// 某路径最近一次水合完成后的 mtime（None = 从未水合）。
    pub fn hydration_mtime(&self, rel: &str) -> Option<i64> {
        self.hydration_log
            .lock()
            .ok()
            .and_then(|m| m.get(rel).copied())
    }

    /// 释放空间：把挂载根内已水合的文件脱水（本地数据丢弃，元数据保留）。
    ///
    /// **provider 专属能力**：脱水经 `CfUpdatePlaceholder(DEHYDRATE)`，
    /// 由持有连接的进程调用——CLI 等外部进程经控制通道
    /// （[`crate::control`]）转发到 daemon，由 daemon 调到这里执行。
    ///
    /// **安全检查**：本地 mtime 既不匹配水合记录也不匹配远端条目
    /// （= 用户修改未上传）时拒绝——脱水会销毁未上传的修改。
    pub fn dehydrate_rel(&self, rel: &str) -> Result<()> {
        // 统一分隔符为 '/'：视图/水合记录/占位符身份均用正斜杠
        // （调用方 strip_prefix 产出的是 Windows 反斜杠）
        let rel = rel.replace('\\', "/");
        let abs = self.root.join(&rel);
        let meta = std::fs::metadata(&abs)?;
        if !meta.is_file() {
            return Err(Error::Invalid(format!(
                "仅支持文件（目录请逐个处理）: {rel}"
            )));
        }
        let local_mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let matches_hydration = self.hydration_mtime(&rel).is_some_and(|m| m == local_mtime);
        let matches_remote = self
            .remote_entry(rel.as_str())
            .is_some_and(|e| e.mtime == local_mtime);
        if !matches_hydration && !matches_remote {
            return Err(Error::Invalid(format!(
                "本地有未上传的修改，拒绝脱水（等待同步完成后重试）: {rel}"
            )));
        }
        // 占位符 → pin+update 脱水；普通文件（用户创建已上传）→ 转换+脱水。
        // REPARSE_POINT 判定在 provider 进程内可靠（本方法只在 daemon/测试
        // 即 provider 进程中调用；过滤器对非 provider 进程掩码占位符属性）。
        let is_placeholder = {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes()
                & windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT.0
                != 0
        };
        if is_placeholder {
            cfapi::dehydrate(&abs, rel.as_bytes())
        } else {
            cfapi::convert_and_dehydrate(&abs, rel.as_bytes())
        }
    }

    /// 内容变化重投影：更新占位符元数据并丢弃陈旧本地数据。
    ///
    /// **安全检查**：本地 mtime 与旧远端条目 mtime 一致（未被用户修改）
    /// 才执行——`DEHYDRATE` 会销毁未上传的用户修改。
    /// 返回 true = 已更新；false = 本地已修改（冲突，跳过）。
    pub fn update_entry(&self, rel: &str, new_entry: &RemoteEntry) -> Result<bool> {
        let old_mtime = self
            .remote_entry(rel)
            .map(|e| e.mtime)
            .unwrap_or(new_entry.mtime);
        let abs = self.root.join(rel);
        let local_mtime = std::fs::metadata(&abs)
            .map(|m| {
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        // 本地 mtime ≠ 旧远端 mtime → 用户修改过（未上传）→ 冲突，跳过
        if local_mtime != 0 && local_mtime != old_mtime {
            tracing::warn!(
                rel,
                local_mtime,
                old_mtime,
                "本地已修改，跳过远端重投影（冲突）"
            );
            return Ok(false);
        }

        let identity = rel.as_bytes().to_vec();
        cfapi::update_placeholder(&abs, new_entry.size, new_entry.mtime, &identity)?;
        // 更新水合寻址表（下次水合服务新内容）
        self.upsert_view_entry(rel, new_entry.clone());
        // 清水合记录（旧记录的 mtime 已失效）
        if let Ok(mut log) = self.hydration_log.lock() {
            log.remove(rel);
        }
        tracing::info!(rel, size = new_entry.size, "占位符已重投影（内容更新）");
        Ok(true)
    }
}

/// 远端视图同步的轮次结果。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ApplyStats {
    /// 新增投影的条目数。
    pub added: u32,
    /// 删除的占位符数。
    pub removed: u32,
    /// 内容变化已重投影的条目数（占位符更新 + 陈旧数据丢弃）。
    pub content_updated: u32,
    /// 内容变化但**未**处理的条目数（本地已修改的冲突，或重投影失败）。
    pub content_changes_pending: u32,
}

/// **占位符协调器**：把远端视图的变化持续应用到挂载视图上。
///
/// 与同步引擎的分工：
/// - 本协调器只管"远端 → 视图"（新增投影 / 删除传播）；
/// - 读取水合由过滤器回调直接走 Provider（不经协调器）；
/// - "本地 → 远端"（用户修改上传）由同步引擎的 RDC/inotify 监视负责。
///
/// ## 内容变化（M2.5 边界）
///
/// 已投影占位符的 size/mtime/sha1 变化需要 `CfUpdatePlaceholders` + 失效通知，
/// 当前版本**跳过并计数**（`content_changes_pending`），视图记录仍更新，
/// 下一轮不会再报。
pub struct PlaceholderSync {
    mount: Arc<CfapiMount>,
    view: HashMap<String, RemoteEntry>,
}

impl PlaceholderSync {
    /// 以初始远端视图创建协调器（视图应与挂载时一致）。
    pub fn new(mount: Arc<CfapiMount>, initial: &HashMap<String, RemoteEntry>) -> Self {
        Self {
            mount,
            view: initial.clone(),
        }
    }

    /// 应用新的远端视图：新增投影、删除传播、内容变化计数。
    pub fn apply_remote_view(
        &mut self,
        new_view: &HashMap<String, RemoteEntry>,
    ) -> Result<ApplyStats> {
        let mut stats = ApplyStats::default();

        // 1. 删除传播：旧视图有、新视图没有。
        //    子路径先删（倒序），避免父目录删除连带后重复删除报 NotFound。
        let removed: Vec<String> = self
            .view
            .keys()
            .filter(|k| !new_view.contains_key(*k))
            .cloned()
            .collect();
        let mut removed_sorted = removed.clone();
        removed_sorted.sort();
        removed_sorted.reverse();
        for rel in &removed_sorted {
            self.mount.remove(rel)?;
            stats.removed += 1;
        }

        // 2. 新增投影：新视图有、旧视图没有
        let added_view: HashMap<String, RemoteEntry> = new_view
            .iter()
            .filter(|(k, _)| !self.view.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !added_view.is_empty() {
            stats.added = self.mount.project(&added_view)?;
        }

        // 3. 内容变化重投影（两边都有但 size/mtime 变化）
        for (k, new_entry) in new_view {
            if let Some(old) = self.view.get(k) {
                if old.size != new_entry.size || old.mtime != new_entry.mtime {
                    // 本地已修改 → 冲突计数（不覆盖用户数据）；
                    // 否则 CfUpdatePlaceholder + DEHYDRATE（下次读取水合新内容）
                    match self.mount.update_entry(k, new_entry) {
                        Ok(true) => stats.content_updated += 1,
                        Ok(false) => stats.content_changes_pending += 1,
                        Err(e) => {
                            tracing::error!(rel = %k, error = %e, "重投影失败（下一轮重试）");
                            stats.content_changes_pending += 1;
                        }
                    }
                }
            }
        }

        self.view = new_view.clone();
        Ok(stats)
    }

    /// 当前已同步的视图。
    pub fn view(&self) -> &HashMap<String, RemoteEntry> {
        &self.view
    }
}

impl Mount for CfapiMount {
    fn mount_point(&self) -> &Path {
        &self.root
    }

    /// 卸载：断开连接（停止回调）+ 双注销（内核 + Shell 侧）。
    fn unmount(self: Box<Self>) -> Result<()> {
        // connection 先 Drop（断开回调），再注销
        drop(self.connection);
        let kernel = cfapi::unregister(&self.root);
        // Shell 侧同步注销（Explorer 导航栏/图标不残留）
        if self.shell_integration {
            use super::shell_reg;
            if let Err(e) = shell_reg::unregister_shell("CloudMount", "CloudMount 云挂") {
                tracing::warn!(error = %e, "Shell 侧同步根注销失败（不阻塞卸载）");
            }
        }
        kernel
    }
}

fn lock_poisoned<T>(_: T) -> Error {
    Error::Other("投影集合锁中毒".into())
}

// 引用 Provider trait（供 open_range 方法解析）
use cloudmount_core::Provider;

// ============ 本地→远端：上传协调器 ============

/// 本地变更上传的轮次结果。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UploadStats {
    /// 成功上传的文件数。
    pub uploaded: u32,
    /// 为用户新建目录创建的远端目录数。
    pub dirs_uploaded: u32,
    /// 因"过滤器水合写入"跳过的修改数（非用户修改，正确行为）。
    pub skipped_hydration: u32,
    /// 因"我们自己的投影"跳过的创建数（投影不是用户新建）。
    pub skipped_projected: u32,
    /// 本地删除（保守策略：不上传，远端保留）。
    pub removed_logged: u32,
    /// 跳过的目录事件。
    pub skipped_dirs: u32,
    /// 失败数。
    pub errors: u32,
}

/// **本地变更上传协调器**：把挂载视图里的用户修改上传到远端。
///
/// ## 三类事件的判定（防误上传的关键）
///
/// | RDC 事件 | 判定 | 动作 |
/// |---|---|---|
/// | Created（文件） | `is_projected`？是 → 我们投影的 | 跳过 |
/// | Created（文件） | 未投影 → 用户新建 | 上传 |
/// | Modified（文件） | mtime == 水合记录 → **过滤器水合写入** | 跳过（否则读一次文件就上传一次） |
/// | Modified（文件） | mtime ≠ 水合记录 → 用户在水合后修改 | 上传 |
/// | Removed | 保守策略（与同步引擎一致） | 不上传，远端保留 |
/// | 目录事件 | 新本地目录上传需建远端目录链（TODO） | 跳过 |
///
/// 上传成功后：更新挂载视图（防止 PlaceholderSync 重复投影）+ `CfSetInSyncState`
/// （Explorer 绿勾）。
pub struct LocalChangeUploader {
    mount: Arc<CfapiMount>,
    provider: Arc<dyn Provider>,
    /// 远端根 ID（上传到根目录用）。
    remote_root: RemoteId,
    watcher: Box<dyn cloudmount_sync::scan::LocalWatcher>,
}

impl LocalChangeUploader {
    /// 创建协调器（在挂载完成后、进入循环前调用）。
    pub fn new(
        mount: Arc<CfapiMount>,
        provider: Arc<dyn Provider>,
        remote_root: RemoteId,
    ) -> Result<Self> {
        let watcher = cloudmount_sync::watcher::build_watcher(
            mount.root(),
            cloudmount_sync::watcher::WatcherBackend::Auto,
        )?;
        Ok(Self {
            mount,
            provider,
            remote_root,
            watcher,
        })
    }

    /// 拉取本地变更并上传。每轮调用一次。
    pub fn poll_and_upload(&mut self) -> Result<UploadStats> {
        use cloudmount_sync::scan::LocalChange;
        let changes = self.watcher.poll()?;
        let mut stats = UploadStats::default();
        for change in changes {
            match change {
                LocalChange::Created { path, entry } => {
                    self.handle_created(&path, &entry, &mut stats)
                }
                LocalChange::Modified { path, entry } => {
                    self.handle_modified(&path, &entry, &mut stats)
                }
                LocalChange::Removed { path, .. } => {
                    // 保守策略：本地删除不上传（远端保留；与同步引擎默认一致）
                    tracing::debug!(path = %path, "本地删除（保守策略：不上传）");
                    stats.removed_logged += 1;
                }
            }
        }
        Ok(stats)
    }

    fn handle_created(&self, path: &str, entry: &LocalEntryView, stats: &mut UploadStats) {
        if entry.is_dir {
            // 用户新建目录 → 远端建目录链（保留空目录语义）
            match self.ensure_remote_dir(path) {
                Ok(_) => stats.dirs_uploaded += 1,
                Err(e) => {
                    tracing::error!(path, error = %e, "创建远端目录失败");
                    stats.errors += 1;
                }
            }
            return;
        }
        if self.mount.is_projected(path) {
            // 我们自己的投影（PlaceholderSync.project 创建的占位符）
            stats.skipped_projected += 1;
            return;
        }
        self.dispatch_upload(path, stats);
    }

    fn handle_modified(&self, path: &str, entry: &LocalEntryView, stats: &mut UploadStats) {
        if entry.is_dir {
            stats.skipped_dirs += 1;
            return;
        }
        // 水合写入判定：mtime == 水合记录 → 过滤器写入（读文件触发的按需水合）
        if let Some(hydrated_mtime) = self.mount.hydration_mtime(path) {
            if hydrated_mtime == entry.mtime {
                stats.skipped_hydration += 1;
                return;
            }
        }
        self.dispatch_upload(path, stats);
    }

    fn dispatch_upload(&self, rel: &str, stats: &mut UploadStats) {
        match self.upload_inner(rel) {
            Ok(()) => stats.uploaded += 1,
            Err(e) => {
                tracing::error!(rel, error = %e, "本地变更上传失败");
                stats.errors += 1;
            }
        }
    }

    fn upload_inner(&self, rel: &str) -> Result<()> {
        let abs = self.mount.root().join(rel.replace('/', "\\"));
        let meta = std::fs::metadata(&abs)?;
        if !meta.is_file() {
            return Ok(()); // 竞态：已消失
        }
        let (parent_rel, name) = match rel.rsplit_once('/') {
            Some((p, n)) => (p, n),
            None => ("", rel),
        };

        // 解析父目录远端 ID（用户新建的目录链自动在远端创建）
        let parent_id = self.ensure_remote_dir(parent_rel)?;

        let result =
            self.provider
                .upload(&parent_id, name, UploadSource::Path(&abs), meta.len())?;
        // 取权威远端信息并更新挂载视图（防 PlaceholderSync 重复投影）
        let entry = self.provider.stat(&result.id)?;
        self.mount.upsert_view_entry(rel, entry);
        // 记录同步水位（脱水安全检查据此放行已上传文件）
        self.mount.record_synced_mtime(
            rel,
            meta.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        );
        // 标记同步（Explorer 绿勾）。失败仅告警不回滚：上传已成功，
        // 且新建的普通文件（非占位符）本就可能不支持该标记。
        if let Err(e) = cfapi::set_in_sync(&abs) {
            tracing::debug!(rel, error = %e, "标记同步状态失败（不影响上传结果）");
        }
        tracing::info!(rel, "本地变更已上传");
        Ok(())
    }
}

impl LocalChangeUploader {
    /// 确保远端目录链存在，返回最末端目录 ID。
    ///
    /// 逐级检查视图（已投影的目录直接复用），缺失的经 Provider 创建并
    /// 写入视图/投影集合（防 PlaceholderSync 重复投影）。
    fn ensure_remote_dir(&self, parent_rel: &str) -> Result<RemoteId> {
        if parent_rel.is_empty() {
            return Ok(self.remote_root.clone());
        }
        if let Some(e) = self.mount.remote_entry(parent_rel) {
            if e.kind == EntryKind::Directory {
                return Ok(e.id);
            }
        }
        let mut prefix = String::new();
        let mut current = self.remote_root.clone();
        for comp in parent_rel.split('/').filter(|c| !c.is_empty()) {
            prefix = if prefix.is_empty() {
                comp.to_string()
            } else {
                format!("{prefix}/{comp}")
            };
            if let Some(e) = self.mount.remote_entry(&prefix) {
                if e.kind == EntryKind::Directory {
                    current = e.id.clone();
                    continue;
                }
            }
            let created = self.provider.create_dir(&current, comp)?;
            let entry = self.provider.stat(&created.id)?;
            self.mount.upsert_view_entry(&prefix, entry);
            current = created.id;
        }
        Ok(current)
    }
}

/// 本地条目视图（避免直接依赖 sync 的 LocalEntry 类型名冲突时的别名）。
type LocalEntryView = cloudmount_sync::scan::LocalEntry;
