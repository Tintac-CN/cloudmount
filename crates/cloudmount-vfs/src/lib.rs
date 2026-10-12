//! # cloudmount-vfs
//!
//! 挂载层抽象。把 [`cloudmount_core::Provider`] 投影为"本地文件系统视图"，
//! 并定义**可插拔的挂载后端**接口 —— 这是本项目"一套核心、多平台挂载"的关键。
//!
//! ## 挂载后端矩阵
//!
//! | 平台 | 后端 | 占位符/按需 | 状态图标 | 状态 |
//! |---|---|---|---|---|
//! | Windows | `CfapiBackend`（Cloud Files API） | ✅ | ✅ | 计划 |
//! | Windows | `WinFspBackend` | ⚠️ 挂载盘 | ❌ | 可选/降级（注意 GPLv3） |
//! | Windows | `CloudMountFilterBackend`（自研 minifilter） | ✅ | 需自研 Shell 扩展 | 长期目标 |
//! | macOS | `FileProviderBackend`（NSFileProviderExtension） | ✅ | ✅ | 计划 |
//! | Linux/边缘 | `FuseBackend`（fuser） | ⚠️ 挂载盘+缓存 | ❌ | 计划 |
//! | 全平台 | `WebDavBackend`（边缘出口） | ⚠️ | ❌ | 计划 |
//!
//! 所有后端实现同一个 [`MountBackend`] trait，`cloudmount-daemon` 按平台选择。

pub mod placeholder;

/// 平台原生挂载后端。
pub mod backends;

#[cfg(windows)]
pub use backends::cfapi;

pub use placeholder::{HydrationState, PlaceholderState};

use cloudmount_core::{RangeSpec, RemoteId, Result};
use std::path::Path;

/// 挂载点句柄：一个已挂载的云盘视图。
pub trait Mount: Send + Sync {
    /// 挂载点路径（Windows 盘符 / Unix 目录 / 边缘网关虚拟根）。
    fn mount_point(&self) -> &Path;

    /// 卸载。
    fn unmount(self: Box<Self>) -> Result<()>;
}

/// 可插拔的挂载后端。
///
/// 各平台实现负责：注册/连接、把文件系统回调翻译为对 VFS 层的调用、
/// 管理占位符状态与 Shell 集成。
pub trait MountBackend: Send + Sync {
    /// 后端名称。
    fn name(&self) -> &'static str;

    /// 本平台是否支持"占位符 + 按需水合"。
    fn supports_placeholders(&self) -> bool;

    /// 本平台是否支持资源管理器/Finder 原生状态图标。
    fn supports_status_icons(&self) -> bool;

    /// 挂载到指定位置，返回句柄。
    fn mount(&self, point: &Path) -> Result<Box<dyn Mount>>;

    /// 触发一次按需水合（读取指定范围），供文件系统回调调用。
    fn hydrate(&self, id: &RemoteId, range: RangeSpec) -> Result<Vec<u8>>;

    /// 释放本地缓存（转为纯占位符）。
    fn dehydrate(&self, id: &RemoteId) -> Result<()>;
}
