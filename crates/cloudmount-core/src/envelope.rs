//! 115 API 响应信封解析（认证域与资源域共用）。
//!
//! 实测两种形态：
//! - 资源域（proapi）：`{"state":true,"code":0,"message":"","data":[...]}`
//! - 认证域（passport）：`{"state":1,"code":0,"message":"","data":{...},"error":"","errno":0}`
//!
//! 注意 `state` 在资源域是**布尔**、认证域是**数字 0/1**，必须兼容解析。
//! 错误时 `data` 可能是 `[]`（与成功时的数组/对象同名），因此先按
//! [`serde_json::Value`] 收下、校验通过后再反序列化为目标类型，避免
//! "错误响应被当成反序列化失败"这种误导性报错。

use crate::{Error, Result};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// `state` 字段：兼容 bool 与 0/1 数字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateFlag(bool);

impl StateFlag {
    /// 是否为成功态。
    pub fn is_ok(self) -> bool {
        self.0
    }
}

impl<'de> Deserialize<'de> for StateFlag {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StateVisitor;
        impl Visitor<'_> for StateVisitor {
            type Value = StateFlag;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("布尔或 0/1 整数")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<StateFlag, E> {
                Ok(StateFlag(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<StateFlag, E> {
                Ok(StateFlag(v != 0))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<StateFlag, E> {
                Ok(StateFlag(v != 0))
            }
        }
        deserializer.deserialize_any(StateVisitor)
    }
}

/// 115 业务错误码分类（依据官方错误码文档与 restic-115 生产实现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// access_token 失效（40140123–40140126）→ 应刷新后重试。
    TokenExpired,
    /// 刷新过于频繁（40140117）→ 应退避，不可立即重试。
    RefreshTooFrequent,
    /// 触发频控（406）→ 应指数退避。
    RateLimited,
    /// 上传需二次认证（700/701/702）。
    UploadSecondAuth,
    /// 其它业务错误。
    Other,
}

/// 分类 115 业务错误码。
pub fn classify_code(code: i64) -> ApiErrorKind {
    match code {
        40140123..=40140126 => ApiErrorKind::TokenExpired,
        40140117 => ApiErrorKind::RefreshTooFrequent,
        406 => ApiErrorKind::RateLimited,
        700..=702 => ApiErrorKind::UploadSecondAuth,
        _ => ApiErrorKind::Other,
    }
}

/// 通用响应信封。
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    /// 成功标志（可能缺失，缺失时以 code 判定）。
    #[serde(default)]
    pub state: Option<StateFlag>,
    /// 业务码（0 为成功）。
    #[serde(default)]
    pub code: Option<i64>,
    /// 提示信息。
    #[serde(default)]
    pub message: Option<String>,
    /// 认证域的错误文本。
    #[serde(default)]
    pub error: Option<String>,
    /// 认证域的错误码（与 code 同值）。
    #[serde(default)]
    pub errno: Option<i64>,
    /// 数据体（成功时的数组/对象/空）。
    #[serde(default)]
    pub data: Option<Value>,
}

impl Envelope {
    /// 解析 JSON 文本为信封。
    pub fn parse(body: &str) -> Result<Self> {
        serde_json::from_str::<Envelope>(body).map_err(|e| {
            Error::Other(format!(
                "响应不是合法 JSON 信封: {e}; 原文前 200 字符: {}",
                body.chars().take(200).collect::<String>()
            ))
        })
    }

    /// 从已解析的 JSON 值构造信封（用于需要先取顶层字段的接口）。
    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        serde_json::from_value::<Envelope>(value)
            .map_err(|e| Error::Other(format!("响应不是合法 JSON 信封: {e}")))
    }

    /// 业务码（code 优先，回退 errno，默认 0）。
    pub fn code(&self) -> i64 {
        self.code.or(self.errno).unwrap_or(0)
    }

    /// 是否成功。
    pub fn is_ok(&self) -> bool {
        self.code() == 0 && self.state.map(|s| s.is_ok()).unwrap_or(true)
    }

    /// 错误描述文本。
    fn message_text(&self) -> String {
        self.message
            .clone()
            .or_else(|| self.error.clone())
            .unwrap_or_default()
    }

    /// 转为 [`Error`]（仅在该信封为错误时调用）。
    pub fn into_error(self) -> Error {
        let code = self.code();
        let msg = self.message_text();
        match classify_code(code) {
            ApiErrorKind::TokenExpired => Error::Auth(format!("token 失效（{code}）: {msg}")),
            ApiErrorKind::RefreshTooFrequent => Error::RateLimited {
                retry_after_ms: 60_000,
            },
            ApiErrorKind::RateLimited => Error::RateLimited {
                retry_after_ms: 5_000,
            },
            ApiErrorKind::UploadSecondAuth => Error::SecondFactorRequired(msg),
            ApiErrorKind::Other => Error::Remote { code, message: msg },
        }
    }

    /// 取数据体并反序列化为 `T`。
    pub fn into_data<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        if !self.is_ok() {
            return Err(self.into_error());
        }
        let value = self.data.unwrap_or(Value::Null);
        serde_json::from_value::<T>(value)
            .map_err(|e| Error::Other(format!("data 反序列化失败: {e}")))
    }

    /// 成功时返回原始 `Value`。
    pub fn into_value(self) -> Result<Value> {
        if !self.is_ok() {
            return Err(self.into_error());
        }
        Ok(self.data.unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_resource_domain_success() {
        let env = Envelope::parse(r#"{"state":true,"code":0,"message":"","data":[{"fid":"1"}]}"#)
            .unwrap();
        assert!(env.is_ok());
        let v: Vec<serde_json::Value> = env.into_data().unwrap();
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn parses_auth_domain_number_state() {
        let env = Envelope::parse(
            r#"{"state":1,"code":0,"message":"","data":{"uid":"abc"},"error":"","errno":0}"#,
        )
        .unwrap();
        assert!(env.is_ok(), "数字 state=1 应视为成功");
        let v: serde_json::Value = env.into_data().unwrap();
        assert_eq!(v["uid"], "abc");
    }

    #[test]
    fn error_with_empty_array_data_is_mapped_not_serde_error() {
        // 错误响应里 data 是 []，不能因为与目标类型不符而报"反序列化失败"
        let env = Envelope::parse(
            r#"{"state":false,"message":"access_token 格式错误","code":40140123,"data":[]}"#,
        )
        .unwrap();
        let err = env.into_data::<Vec<String>>().unwrap_err();
        assert!(matches!(err, Error::Auth(_)), "应映射为 Auth 错误: {err:?}");
    }

    #[test]
    fn classifies_codes() {
        assert_eq!(classify_code(40140123), ApiErrorKind::TokenExpired);
        assert_eq!(classify_code(40140126), ApiErrorKind::TokenExpired);
        assert_eq!(classify_code(40140117), ApiErrorKind::RefreshTooFrequent);
        assert_eq!(classify_code(406), ApiErrorKind::RateLimited);
        assert_eq!(classify_code(701), ApiErrorKind::UploadSecondAuth);
        assert_eq!(classify_code(12345), ApiErrorKind::Other);
    }

    #[test]
    fn auth_domain_error_uses_errno_when_code_absent() {
        let env = Envelope::parse(
            r#"{"state":0,"message":"code_verifier 验证失败","data":{},"error":"x","errno":40140113}"#,
        )
        .unwrap();
        assert!(!env.is_ok());
        assert_eq!(env.code(), 40140113);
    }
}
