//! 通用数据类型：远端条目、配额、范围请求等。
//!
//! 这些类型是平台无关的，被所有驱动（115 官方 / Cookie / 本地）与上层 VFS 共用。

use serde::{Deserialize, Serialize};

/// 远端条目的稳定标识。115 使用 `file_id`（数字）或 `pick_code`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteId(pub String);

impl std::fmt::Display for RemoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// 条目类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    /// 普通文件。
    File,
    /// 目录。
    Directory,
}

/// 一个远端文件/目录条目（驱动列表结果的统一形态）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEntry {
    /// 稳定 ID。
    pub id: RemoteId,
    /// 父目录 ID（根为 "0"）。
    pub parent_id: RemoteId,
    /// 名称（明文；加密层在其上做名称转换）。
    pub name: String,
    /// 类型。
    pub kind: EntryKind,
    /// 字节大小（目录为 0）。
    pub size: u64,
    /// 远端修改时间（Unix 秒）。
    pub mtime: i64,
    /// 115 的 SHA1（用于秒传判定）；目录为 None。
    pub sha1: Option<String>,
    /// 115 的 `pick_code`（下载直链所需）。
    pub pick_code: Option<String>,
    /// 是否已从远端删除（软删除标记）。
    #[serde(default)]
    pub deleted: bool,
}

/// 空间配额。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Quota {
    /// 总空间（字节）。
    pub total: u64,
    /// 已用空间（字节）。
    pub used: u64,
    /// 剩余空间（字节）。
    pub remaining: u64,
}

/// 廉价的远端变更探测结果。
///
/// 115 官方 API 没有事件/增量接口，全量扫描一棵大目录代价高。
/// 支持探测的后端可返回"条目数 + 最大修改时间"，引擎据此判断
/// "远端未变"从而短路整棵子树的扫描。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeProbe {
    /// 该目录下的条目总数。
    pub count: u64,
    /// 该目录下最新的修改时间（Unix 秒）。
    pub max_mtime: i64,
}

/// 一个 HTTP Range 请求（用于分块/并发下载与按需水合）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeSpec {
    /// 起始偏移（含）。
    pub start: u64,
    /// 结束偏移（含）。None 表示到文件末尾。
    pub end: Option<u64>,
}

impl RangeSpec {
    /// 构造 `bytes=start-end` 形式的 Range 头值。
    pub fn to_header(&self) -> String {
        match self.end {
            Some(end) => format!("bytes={}-{}", self.start, end),
            None => format!("bytes={}-", self.start),
        }
    }

    /// 请求的字节数（end 为 None 时返回 None）。
    pub fn len(&self) -> Option<u64> {
        self.end.map(|e| e.saturating_sub(self.start) + 1)
    }

    /// 是否为空范围。
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }
}
