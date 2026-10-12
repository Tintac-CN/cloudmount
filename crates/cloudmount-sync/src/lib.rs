//! # cloudmount-sync
//!
//! CloudMount 的双向同步引擎（M1）。职责：
//!
//! 1. **变更发现**：本地扫描（[`scan`]，后续接 USN/inotify/FSEvents）+
//!    远端廉价探测（[`cloudmount_core::Provider::change_probe`]，不支持时全量扫描）。
//! 2. **三向对比**：以"上次同步快照"为基准，判定新增/更新/冲突/删除传播
//!    （[`reconcile`]，纯函数，可穷举测试）。
//! 3. **调度执行**：上传/下载分列、重试退避、暂停、并发
//!    （[`queue`] + [`exec`]）。
//! 4. **本地状态**：SQLite 元数据（[`store`]）与内容块缓存（[`cache`]）。
//!
//! ## 一致性策略（重要设计取舍）
//!
//! - **删除不传播是默认值**：远端删了本地保留（重新上传）、本地删了远端保留（重新下载）。
//!   数据同步工具误删用户文件的代价不可逆，故默认保守；需要 OneDrive 式行为时
//!   显式打开 [`SyncPolicy::propagate_remote_deletes`] / [`propagate_local_deletes`]。
//! - **冲突默认"保留两者"**：本地副本改名 `<名称> (conflict-<设备>-<时间>).<扩展名>`
//!   后上传为新文件，远端原文件下载回本地原路径。
//! - **不做内容哈希的全量校验**：靠 (size, mtime) 判定变更（快）。同一秒内
//!   等大小修改会漏检——这是已知取舍，内容校验留待 M1.5 的"慢速校验"通道。
//! - **下载先写临时文件再原子改名**，且把本地 mtime 设为远端 mtime（`filetime`），
//!   避免下轮被误判为"本地已改"。

pub mod cache;
pub mod engine;
pub mod exec;
pub mod queue;
pub mod reconcile;
pub mod remote;
pub mod scan;
pub mod state;
pub mod store;
pub mod watcher;

/// Windows 原生 watcher（ReadDirectoryChangesW）。
#[cfg(windows)]
pub mod rdc;

/// Linux 原生 watcher（inotify）。
#[cfg(target_os = "linux")]
pub mod inotify_watcher;

pub use engine::{SyncEngine, SyncEngineConfig};
pub use reconcile::{Action, ConflictPolicy, SyncPolicy};
pub use state::{EntryRecord, PinState, SyncState, SyncStats};
pub use store::MetaStore;
