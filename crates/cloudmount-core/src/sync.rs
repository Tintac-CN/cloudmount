//! 同步引擎骨架。
//!
//! 职责：发现本地与远端的变更、维护每个文件的同步状态、调度上传/下载队列、
//! 解决冲突。当前为骨架，定义状态机与接口，具体实现见后续里程碑。
//!
//! 变更发现策略（因 115 官方 API 无事件接口）：
//! - **本地**：NTFS USN Journal（Windows）/ FSEvents（macOS）/ inotify（Linux）
//!   —— 平台相关，通过 `LocalWatcher` 抽象。
//! - **远端**：轮询 `usertime` 倒序头部 + 全量扫描兜底（默认 30–60s，指数退避）。

use crate::types::{RemoteEntry, RemoteId};
use std::collections::HashMap;

/// 单个文件的同步状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// 仅本地存在，待上传。
    LocalOnly,
    /// 仅远端存在，待下载（或作为占位符）。
    RemoteOnly,
    /// 两端一致。
    InSync,
    /// 冲突（两端都改）。
    Conflict,
    /// 上传中。
    Uploading,
    /// 下载/水合中。
    Downloading,
    /// 待处理（排队中）。
    Pending,
}

/// 本地缓存的条目记录（持久化到 SQLite）。
#[derive(Debug, Clone)]
pub struct CacheRecord {
    /// 相对路径（本地视角）。
    pub rel_path: String,
    /// 远端 ID（若已上传）。
    pub remote_id: Option<RemoteId>,
    /// 状态。
    pub state: SyncState,
    /// 本地修改时间（Unix 秒）。
    pub local_mtime: i64,
    /// 远端修改时间（Unix 秒）。
    pub remote_mtime: i64,
    /// 大小。
    pub size: u64,
    /// 已水合范围（稀疏追踪）；None 表示未水合（纯占位符）。
    pub hydrated: Option<Vec<(u64, u64)>>,
    /// pin 状态（始终保留 / 仅在线 / 排除）。
    pub pin: PinState,
}

/// 文件的"固定"策略，映射到各平台占位符语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinState {
    /// 始终保留在本设备（pinned）。
    Pinned,
    /// 仅在线（占位符，可被回收）。
    #[default]
    Online,
    /// 排除同步。
    Excluded,
    /// 继承父目录。
    Inherit,
}

/// 远端变更的来源（轮询或事件）。
#[derive(Debug, Clone)]
pub enum RemoteChange {
    /// 新增/更新。
    Upsert(RemoteEntry),
    /// 删除。
    Removed(RemoteId),
}

/// 一次同步周期的统计。
#[derive(Debug, Default, Clone)]
pub struct SyncStats {
    pub scanned_local: u64,
    pub scanned_remote: u64,
    pub uploaded: u64,
    pub downloaded: u64,
    pub conflicts: u64,
    pub errors: u64,
}

/// 同步引擎（骨架）。
///
/// 真实实现需持有 `Provider`、本地根路径、SQLite 连接、以及限速器/队列。
pub struct SyncEngine {
    /// 本地根目录（挂载点或同步文件夹）。
    pub local_root: std::path::PathBuf,
    /// 当前缓存记录（内存视图，权威在 SQLite）。
    pub records: HashMap<String, CacheRecord>,
    /// 统计。
    pub stats: SyncStats,
}

impl SyncEngine {
    /// 创建一个新的同步引擎。
    pub fn new(local_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            local_root: local_root.into(),
            records: HashMap::new(),
            stats: SyncStats::default(),
        }
    }

    /// 执行一轮同步。骨架：真实实现见里程碑 M1。
    ///
    /// 步骤（规划）：
    /// 1. 拉取远端目录树（增量：usertime 头部 + 缓存比对）。
    /// 2. 扫描本地变更（USN/inotify）。
    /// 3. 三向比对，生成操作集（上传/下载/删除/冲突）。
    /// 4. 按队列与限速调度执行。
    /// 5. 持久化状态。
    pub fn run_once(&mut self) -> crate::Result<&SyncStats> {
        // TODO(M1): 实现完整的双向同步循环。
        Err(crate::Error::Unimplemented("SyncEngine::run_once".into()))
    }
}
