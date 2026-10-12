//! 占位符状态模型（跨平台统一语义）。
//!
//! 各平台用不同机制表达"文件在云端、本地只有元数据"：
//! - Windows CfAPI：`CF_PLACEHOLDER_STATE_*` + `CF_PIN_STATE_*`（reparse point）
//! - macOS File Provider：`NSFileProviderItem` 的 `isDownloaded`/`isMostRecentVersionDownloaded`
//! - Linux FUSE：无原生占位符，用稀疏文件 + 缓存元数据模拟
//! - 自研 Windows minifilter：自定义 reparse point + 本地 DB
//!
//! 本模块定义统一语义，由各后端映射。

/// 占位符的持久状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaceholderState {
    /// 纯占位符：本地无数据，读取时需水合。
    #[default]
    Placeholder,
    /// 部分水合：仅部分范围在本地（支持随机 seek 的渐进式下载）。
    Partial,
    /// 完全水合：全部数据在本地，但仍可被回收。
    Full,
    /// 已固定：用户显式"始终保留"，不可被系统回收。
    Pinned,
}

/// 水合进度（供 UI 展示下载进度）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HydrationState {
    /// 已水合字节数。
    pub hydrated_bytes: u64,
    /// 总字节数。
    pub total_bytes: u64,
    /// 是否正在水合。
    pub in_progress: bool,
    /// 是否与远端一致。
    pub in_sync: bool,
}

impl HydrationState {
    /// 水合完成度（0.0–1.0）。
    pub fn progress(&self) -> f32 {
        if self.total_bytes == 0 {
            1.0
        } else {
            self.hydrated_bytes as f32 / self.total_bytes as f32
        }
    }
}
