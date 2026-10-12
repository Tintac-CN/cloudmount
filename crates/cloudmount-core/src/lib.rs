//! # cloudmount-core
//!
//! CloudMount 的核心抽象层：与平台无关的存储 [`Provider`] trait、通用数据类型、
//! 错误模型，以及同步引擎的骨架。
//!
//! 设计原则：
//! - **平台无关**：本 crate 不依赖任何 Windows / macOS / Linux 专有 API，
//!   可编译到桌面三端与 ARM 边缘设备。
//! - **驱动可插拔**：115 官方 OpenAPI、115 Cookie 通道、本地 FS 等都实现
//!   同一个 [`Provider`] trait，上层同步引擎与 VFS 无感知。
//! - **极简依赖**：只依赖 serde / thiserror / time / tracing，保持轻量。

pub mod envelope;
pub mod error;
pub mod paths;
pub mod provider;
pub mod serde_util;
pub mod types;

pub use envelope::{classify_code, ApiErrorKind, Envelope};
pub use error::{Error, Result};
pub use provider::{Provider, UploadResult, UploadSource};
pub use types::{ChangeProbe, EntryKind, Quota, RangeSpec, RemoteEntry, RemoteId};

/// 产品版本号（用于 UI 展示与 User-Agent）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 默认 User-Agent，遵循"像正常客户端"原则，避免触发风控。
pub const DEFAULT_USER_AGENT: &str = concat!(
    "CloudMount/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Tintac-CN/cloudmount)"
);
