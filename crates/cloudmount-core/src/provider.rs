//! 存储 Provider 抽象。
//!
//! 所有后端（115 官方 OpenAPI、115 Cookie 通道、本地 FS、未来的 S3/WebDAV）
//! 都实现同一个 [`Provider`] trait。上层同步引擎与 VFS 只依赖此 trait，
//! 从而做到"驱动可插拔、引擎无感知"。
//!
//! 设计取舍：
//! - **同步 trait**：核心传输层基于 `ureq`（同步、依赖最浅、内存最省）。
//!   异步运行时（tokio）在 daemon 层用 `spawn_blocking` 调用本 trait。
//!   这样避免把 tokio 的 async 传染进整个核心，也便于单线程低内存运行。

use crate::error::Result;
use crate::types::{ChangeProbe, Quota, RangeSpec, RemoteEntry, RemoteId};
use std::io::Read;

/// 一次上传的结果。
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// 新文件的远端 ID。
    pub id: RemoteId,
    /// 是否命中秒传（未实际上传字节）。
    pub rapid_upload: bool,
}

/// 上传数据来源。小文件可直接给内存缓冲，大文件给文件路径以支持分片。
pub enum UploadSource<'a> {
    /// 内存缓冲。
    Bytes(&'a [u8]),
    /// 本地文件路径（用于分片/断点续传）。
    Path(&'a std::path::Path),
}

/// 存储后端统一接口。
///
/// 注意：这是**明文视角**的接口。加密资料库（v2）会在 Provider 之上、
/// 同步引擎之下插入一层透明的名称/内容转换，对引擎而言 vault 只是另一棵目录树。
pub trait Provider: Send + Sync {
    /// 后端名称（如 "115-open"、"115-cookie"、"local"）。
    fn name(&self) -> &'static str;

    /// 列出某目录下的直接子项。
    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>>;

    /// 获取单个条目元数据。
    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry>;

    /// 打开一个文件范围用于读取（按需水合 / 分块下载的底层）。
    ///
    /// 实现应尽量利用 HTTP Range 并发分块；返回的 reader 只覆盖请求范围。
    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>>;

    /// 新建目录。
    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry>;

    /// 上传文件到指定目录。
    ///
    /// 实现应先做秒传判定（SHA1 / 前 128K SHA1），命中则零传输完成。
    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult>;

    /// 重命名 / 更新属性。
    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()>;

    /// 移动。
    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()>;

    /// 复制。
    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId>;

    /// 删除（进回收站）。
    fn delete(&self, id: &RemoteId) -> Result<()>;

    /// 查询空间配额。
    fn quota(&self) -> Result<Quota>;

    /// 该后端是否支持增量变更事件（115 官方 API 不支持 → 默认 false）。
    ///
    /// 不支持时，同步引擎退化为 `usertime` 轮询 + 全量扫描兜底。
    fn supports_change_feed(&self) -> bool {
        false
    }

    /// 廉价的变更探测（可选能力）。
    ///
    /// 返回 `(条目数, 最大修改时间)`；引擎可以用它短路"远端未变"的整棵子树扫描。
    /// 默认返回 `None` 表示不支持，引擎走全量扫描。
    fn change_probe(&self, _dir: &RemoteId) -> Result<Option<ChangeProbe>> {
        Ok(None)
    }
}
