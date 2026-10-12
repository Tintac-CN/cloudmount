//! 统一错误模型。

use thiserror::Error;

/// CloudMount 核心错误类型。
///
/// 设计上区分"可重试的传输类错误"与"不可重试的逻辑/认证类错误"，
/// 供同步引擎与限速退避器据此决策。
#[derive(Debug, Error)]
pub enum Error {
    /// 网络/传输层错误（可重试）。
    #[error("transport error: {0}")]
    Transport(String),

    /// HTTP 非 2xx 响应。
    #[error("http error {status}: {body}")]
    Http { status: u16, body: String },

    /// 触发远端频控（HTTP 429 或服务端频控码）。`retry_after_ms` 为建议退避。
    #[error("rate limited (retry after {retry_after_ms}ms)")]
    RateLimited { retry_after_ms: u64 },

    /// 认证失败 / token 失效（需重新登录）。
    #[error("auth error: {0}")]
    Auth(String),

    /// 需要二次验证（如 115 上传的 sign_check 二次校验）。
    #[error("second factor required: {0}")]
    SecondFactorRequired(String),

    /// 远端返回业务错误码。
    #[error("remote error {code}: {message}")]
    Remote { code: i64, message: String },

    /// 本地 IO 错误。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 元数据 DB 错误。
    #[error("database error: {0}")]
    Db(String),

    /// 序列化错误。
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// 资源不存在。
    #[error("not found: {0}")]
    NotFound(String),

    /// 参数非法。
    #[error("invalid argument: {0}")]
    Invalid(String),

    /// 功能未实现（占位）。
    #[error("not implemented: {0}")]
    Unimplemented(String),

    /// 其它。
    #[error("{0}")]
    Other(String),
}

/// 核心 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// 该错误是否值得自动重试（网络抖动 / 频控 / 5xx）。
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Transport(_) | Error::RateLimited { .. } => true,
            Error::Http { status, .. } => *status >= 500,
            _ => false,
        }
    }
}
