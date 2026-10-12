//! # cloudmount-driver115
//!
//! 115 网盘驱动，实现 [`cloudmount_core::Provider`]。两条通道：
//!
//! - [`open::Open115Provider`]：**官方 OpenAPI（主通道）**，稳定、合规。
//! - [`cookie::Cookie115Provider`]：**Cookie 通道（补充）**，能力更广但违规且有互踢风险。
//!
//! 上层通过 [`router::ProviderRouter`] 在两条通道间切换/降级。
//!
//! 内部模块：
//! - [`client::ApiClient`]：资源域 HTTP 客户端（Bearer、限速、token 失效自动重试）
//! - [`models`]：115 报文数据模型（字段名严格对齐实测）
//! - [`hash`]：秒传所需的 SHA1 / 前 128KiB preid 计算
//! - [`ratelimit`]：令牌桶与指数退避

pub mod client;
pub mod cookie;
pub mod hash;
pub mod models;
pub mod open;
pub mod ratelimit;
pub mod router;

pub use client::ApiClient;
pub use open::Open115Provider;
pub use router::ProviderRouter;

/// 115 的根目录 ID 固定为 "0"。
pub const ROOT_ID: &str = "0";
