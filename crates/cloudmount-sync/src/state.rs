//! 同步状态、快照记录与统计。

use cloudmount_core::RemoteId;
use serde::{Deserialize, Serialize};

/// 单个路径的同步状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// 仅本地存在，待上传。
    LocalOnly,
    /// 仅远端存在，待下载。
    RemoteOnly,
    /// 两端一致。
    InSync,
    /// 冲突（两端都改），需人工或策略解决。
    Conflict,
    /// 正在上传。
    Uploading,
    /// 正在下载。
    Downloading,
    /// 待处理（已入队）。
    Pending,
}

impl SyncState {
    /// 持久化用字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            SyncState::LocalOnly => "local_only",
            SyncState::RemoteOnly => "remote_only",
            SyncState::InSync => "in_sync",
            SyncState::Conflict => "conflict",
            SyncState::Uploading => "uploading",
            SyncState::Downloading => "downloading",
            SyncState::Pending => "pending",
        }
    }

    /// 从持久化字符串恢复（未知值回退 `Pending` 并告警）。
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "local_only" => SyncState::LocalOnly,
            "remote_only" => SyncState::RemoteOnly,
            "in_sync" => SyncState::InSync,
            "conflict" => SyncState::Conflict,
            "uploading" => SyncState::Uploading,
            "downloading" => SyncState::Downloading,
            "pending" => SyncState::Pending,
            other => {
                tracing::warn!(value = other, "未知同步状态，回退为 pending");
                SyncState::Pending
            }
        }
    }
}

/// 文件的"固定"策略（映射到各平台占位符语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinState {
    /// 始终保留在本设备。
    Pinned,
    /// 仅在线（占位符，可被回收）。
    #[default]
    Online,
    /// 排除同步。
    Excluded,
    /// 继承父目录。
    Inherit,
}

impl PinState {
    /// 持久化用字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            PinState::Pinned => "pinned",
            PinState::Online => "online",
            PinState::Excluded => "excluded",
            PinState::Inherit => "inherit",
        }
    }

    /// 从持久化字符串恢复。
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "pinned" => PinState::Pinned,
            "excluded" => PinState::Excluded,
            "inherit" => PinState::Inherit,
            _ => PinState::Online,
        }
    }
}

/// 上次同步快照中的一条记录（SQLite 表 `entries` 的映射）。
///
/// 三向对比的"基准"：记录了上次同步完成时两端的 (size, mtime, remote_id)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRecord {
    /// 相对根目录的路径（以 `/` 分隔，不含根）。
    pub rel_path: String,
    /// 是否目录。
    pub is_dir: bool,
    /// 远端 ID（目录亦然）。
    pub remote_id: Option<RemoteId>,
    /// 远端 SHA1（目录为 None）。
    pub remote_sha1: Option<String>,
    /// 上次同步时的远端修改时间（Unix 秒）。
    pub remote_mtime: i64,
    /// 上次同步时的本地修改时间（Unix 秒）。
    pub local_mtime: i64,
    /// 上次同步时的文件大小。
    pub size: u64,
    /// 状态。
    pub state: SyncState,
    /// pin 策略。
    pub pin: PinState,
}

/// 一次同步周期的统计。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncStats {
    /// 本地扫描到的条目数。
    pub scanned_local: u64,
    /// 远端扫描到的条目数。
    pub scanned_remote: u64,
    /// 远端探测是否命中"未变化"短路。
    pub remote_unchanged: bool,
    /// 上传成功数。
    pub uploaded: u64,
    /// 下载成功数。
    pub downloaded: u64,
    /// 冲突数。
    pub conflicts: u64,
    /// 失败的操作数。
    pub errors: u64,
    /// 本地删除数（远端删除传播）。
    pub deleted_local: u64,
    /// 远端删除数（本地删除传播）。
    pub deleted_remote: u64,
    /// 创建的目录数（本地或远端）。
    pub dirs_created: u64,
}

impl SyncStats {
    /// 是否有任何变更发生。
    pub fn has_changes(&self) -> bool {
        self.uploaded > 0
            || self.downloaded > 0
            || self.conflicts > 0
            || self.deleted_local > 0
            || self.deleted_remote > 0
            || self.dirs_created > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_state_round_trip() {
        for st in [
            SyncState::LocalOnly,
            SyncState::RemoteOnly,
            SyncState::InSync,
            SyncState::Conflict,
            SyncState::Uploading,
            SyncState::Downloading,
            SyncState::Pending,
        ] {
            assert_eq!(SyncState::from_str_lossy(st.as_str()), st);
        }
        assert_eq!(SyncState::from_str_lossy("garbage"), SyncState::Pending);
    }

    #[test]
    fn pin_state_round_trip() {
        for p in [
            PinState::Pinned,
            PinState::Online,
            PinState::Excluded,
            PinState::Inherit,
        ] {
            assert_eq!(PinState::from_str_lossy(p.as_str()), p);
        }
        assert_eq!(PinState::from_str_lossy("?"), PinState::Online);
    }

    #[test]
    fn stats_has_changes() {
        let mut s = SyncStats::default();
        assert!(!s.has_changes());
        s.uploaded = 1;
        assert!(s.has_changes());
    }
}
