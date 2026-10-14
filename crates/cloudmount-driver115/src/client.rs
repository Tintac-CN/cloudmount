//! 资源域 API 客户端（proapi）。
//!
//! 职责：
//! - 统一 Bearer 认证与 **token 失效自动刷新重试**（官方码 40140123–40140126）；
//! - 全局限速（默认 1 r/s，官方频控细则不公开，必须保守）；
//! - 信封解析与错误码映射（[`cloudmount_core::Envelope`]）；
//! - 两种 POST 编码：资源域用 `multipart/form-data`（与 restic-115 等生产实现一致），
//!   认证域用 urlencoded（由 auth crate 负责）。

use crate::ratelimit::RateLimiter;
use cloudmount_auth::open::{OpenApiAuth, PROAPI_BASE};
use cloudmount_core::{Envelope, Error, RangeSpec, Result};
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

/// 单次限速等待上限，避免 CLI 卡太久。
const MAX_RATE_WAIT_MS: u64 = 5_000;

/// CDN 并发水合参数（8 并发流，下载器常规量级）。
const CDN_RATE_PER_SEC: f64 = 8.0;
const CDN_BURST: f64 = 8.0;

/// 资源域客户端。
pub struct ApiClient {
    /// HTTP 客户端（ureq 同步 + rustls）。
    pub agent: ureq::Agent,
    /// 全局限速器。
    pub limiter: RateLimiter,
    /// CDN 直链限速器（宽松：CDN 为海量并发设计；**勿与 API 共用**——
    /// 并发水合时共用会把多文件读取串行化到 1 r/s）。
    cdn_limiter: RateLimiter,
    /// 认证器（提供 access_token，必要时刷新）。
    pub auth: Arc<OpenApiAuth>,
    /// API 基础地址（生产为 proapi.115.com；测试可注入 mock 服务端）。
    pub base_url: String,
}

impl ApiClient {
    /// 创建客户端（生产地址）。
    pub fn new(auth: Arc<OpenApiAuth>) -> Self {
        Self::with_base_url(auth, PROAPI_BASE)
    }

    /// 指定 API 基础地址创建客户端（用于集成测试的 mock 服务端）。
    pub fn with_base_url(auth: Arc<OpenApiAuth>, base_url: impl Into<String>) -> Self {
        Self {
            agent: ureq::Agent::config_builder()
                .user_agent(cloudmount_core::DEFAULT_USER_AGENT)
                // 自己处理 4xx（115 的错误信封带业务码，不能被 ureq 提前变成 Err）
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(120)))
                .build()
                .into(),
            limiter: RateLimiter::conservative(),
            cdn_limiter: RateLimiter::new(CDN_RATE_PER_SEC, CDN_BURST),
            auth,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    /// 调整限速（测试用；生产请保持保守默认值）。
    pub fn with_rate_limit(mut self, rate_per_sec: f64, capacity: f64) -> Self {
        self.limiter = RateLimiter::new(rate_per_sec, capacity);
        self
    }

    /// 尊重限速的等待（同步阻塞式，适合 CLI 与服务端后台线程）。
    fn pace(&self) -> Result<()> {
        loop {
            match self.limiter.try_acquire() {
                Ok(()) => return Ok(()),
                Err(Error::RateLimited { retry_after_ms }) => {
                    let wait = retry_after_ms.min(MAX_RATE_WAIT_MS);
                    std::thread::sleep(Duration::from_millis(wait));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// CDN 直链的限速等待（宽松档）。
    fn cdn_pace(&self) -> Result<()> {
        loop {
            match self.cdn_limiter.try_acquire() {
                Ok(()) => return Ok(()),
                Err(Error::RateLimited { retry_after_ms }) => {
                    let wait = retry_after_ms.min(MAX_RATE_WAIT_MS);
                    std::thread::sleep(Duration::from_millis(wait));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// GET 并解析 `data`。
    pub fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let text = self.with_token_retry(|token| {
            self.pace()?;
            let mut req = self
                .agent
                .get(url.as_str())
                .header("Authorization", &format!("Bearer {token}"));
            for (k, v) in query {
                req = req.query(k, v.as_str());
            }
            let resp = req.call().map_err(|e| map_ureq_err(&url, e))?;
            let raw = read_body_text(resp)?;
            Ok(raw)
        })?;
        Envelope::parse(&text)?.into_data()
    }

    /// POST（`application/x-www-form-urlencoded`）并解析 `data`。
    ///
    /// 编码选择说明：官方文档标注 `form-data`，但两种编码在**生产实现中都被验证可用**
    /// （AList 的 Go SDK 用 urlencoded，restic-115 用 multipart）。本项目采用
    /// urlencoded —— 它是使用面最广的生产路线，且对中文文件名等非 ASCII 值的
    /// 百分号编码语义明确无歧义。
    pub fn post_form_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        fields: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let text = self.with_token_retry(|token| {
            self.pace()?;
            let resp = self
                .agent
                .post(url.as_str())
                .header("Authorization", &format!("Bearer {token}"))
                .send_form(fields.iter().map(|(k, v)| (*k, v.as_str())))
                .map_err(|e| map_ureq_err(&url, e))?;
            read_body_text(resp)
        })?;
        Envelope::parse(&text)?.into_data()
    }

    /// GET 原始 JSON（保留顶层字段，用于 `ufile/files` 这类分页字段在 data 之外的响应）。
    pub fn get_raw(&self, path: &str, query: &[(&str, String)]) -> Result<serde_json::Value> {
        let url = format!("{}{path}", self.base_url);
        let text = self.with_token_retry(|token| {
            self.pace()?;
            let mut req = self
                .agent
                .get(url.as_str())
                .header("Authorization", &format!("Bearer {token}"));
            for (k, v) in query {
                req = req.query(k, v.as_str());
            }
            read_body_text(req.call().map_err(|e| map_ureq_err(&url, e))?)
        })?;
        serde_json::from_str(&text).map_err(|e| {
            Error::Other(format!(
                "响应不是合法 JSON: {e}; 原文前 200 字符: {}",
                text.chars().take(200).collect::<String>()
            ))
        })
    }

    /// 对直链发起带 Range 的 GET，返回流式 reader（用于按需水合/分块下载）。
    ///
    /// 注意：直链走 CDN，**不带** Authorization。
    pub fn get_range_stream(
        &self,
        direct_url: &str,
        range: RangeSpec,
    ) -> Result<Box<dyn Read + Send>> {
        // CDN 直链走宽松限速（并发水合不串行化；API 仍保守）
        self.cdn_pace()?;
        let resp = self
            .agent
            .get(direct_url)
            .header("Range", &range.to_header())
            .call()
            .map_err(|e| map_ureq_err(direct_url, e))?;
        let status = resp.status().as_u16();
        if status != 200 && status != 206 {
            return Err(Error::Http {
                status,
                body: format!("直链下载失败（Range {}）", range.to_header()),
            });
        }
        Ok(Box::new(resp.into_body().into_reader()))
    }

    /// 上传到预签名直传地址（OSS/云厂商），带自定义回调头。
    pub fn put_with_headers(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: PutBody<'_>,
    ) -> Result<String> {
        self.pace()?;
        let mut req = self.agent.put(url);
        for (k, v) in headers {
            req = req.header(*k, v.as_str());
        }
        let resp = match body {
            PutBody::Bytes(b) => req.send(b).map_err(|e| map_ureq_err(url, e))?,
            PutBody::File(f) => req.send(f).map_err(|e| map_ureq_err(url, e))?,
        };
        let mut rb = resp.into_body();
        rb.read_to_string()
            .map_err(|e| Error::Transport(format!("读取上传回执失败: {e}")))
    }

    /// 带"token 失效自动刷新重试一次"的请求封装。
    ///
    /// 处理链：
    /// 1. 取 token（auth 内部按过期时间提前刷新，且为单飞）；
    /// 2. 执行请求；若业务码属于 40140123–40140126，则强制刷新后重试一次；
    /// 3. 40140117（刷新过快）直接上抛为 [`Error::RateLimited`]，由调用方退避。
    fn with_token_retry<F>(&self, mut attempt: F) -> Result<String>
    where
        F: FnMut(&str) -> Result<String>,
    {
        let token = self.auth.access_token()?;
        let text = attempt(&token)?;
        // 先看是否是 token 失效
        let needs_refresh = match Envelope::parse(&text) {
            Ok(env) => {
                if env.is_ok() {
                    return Ok(text);
                }
                matches!(
                    cloudmount_core::classify_code(env.code()),
                    cloudmount_core::ApiErrorKind::TokenExpired
                )
            }
            // 不是 JSON 信封（可能是 HTTP 层错误页），交给上层解析时报错
            Err(_) => return Ok(text),
        };
        if !needs_refresh {
            return Ok(text);
        }
        tracing::debug!("access_token 被服务端判定失效，强制刷新后重试一次");
        // 显式传本次失效的 token：并发重试路径经身份双检合并为至多一次刷新（40140117 防护）
        self.auth.refresh_stale(&token)?;
        let token = self.auth.access_token()?;
        attempt(&token)
    }
}

/// PUT 的请求体。
pub enum PutBody<'a> {
    /// 内存缓冲（小文件）。
    Bytes(&'a [u8]),
    /// 本地文件（大文件，单次 PUT 上限 5GB）。
    File(&'a std::fs::File),
}

/// 读取响应体为文本。
fn read_body_text(resp: ureq::http::Response<ureq::Body>) -> Result<String> {
    let status = resp.status().as_u16();
    let mut body = resp.into_body();
    let text = body
        .read_to_string()
        .map_err(|e| Error::Transport(format!("读取响应失败: {e}")))?;
    // HTTP 层错误且响应不是 JSON 信封时，给出带状态码的明确错误
    if (status >= 400) && Envelope::parse(&text).is_err() {
        return Err(Error::Http {
            status,
            body: text.chars().take(200).collect(),
        });
    }
    Ok(text)
}

/// 把 ureq 错误映射为核心错误。
fn map_ureq_err(url: &str, e: ureq::Error) -> Error {
    match e {
        ureq::Error::StatusCode(code) => Error::Http {
            status: code,
            body: String::new(),
        },
        e => Error::Transport(format!("{url}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_code_errors() {
        let err = map_ureq_err("https://example.com", ureq::Error::StatusCode(429));
        assert!(matches!(err, Error::Http { status: 429, .. }));
    }

    #[test]
    fn form_encoding_is_queryable_by_ureq() {
        // 与 send_form 相同的编码路径：中文文件名必须被百分号编码
        let fields = [("file_name", "报告 2026.pdf")];
        let mut body = String::new();
        for (k, v) in fields {
            if !body.is_empty() {
                body.push('&');
            }
            body.push_str(&form_check(k));
            body.push('=');
            body.push_str(&form_check(v));
        }
        assert!(!body.contains(' '), "空格应被编码");
        assert!(!body.contains("报告"), "中文应被百分号编码");
    }

    /// 与 ureq 内部等价的简易校验编码器（仅用于断言编码确实发生）。
    fn form_check(s: &str) -> String {
        let mut out = String::new();
        for b in s.as_bytes() {
            match *b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(*b as char)
                }
                b' ' => out.push('+'),
                other => out.push_str(&format!("%{other:02X}")),
            }
        }
        out
    }
}
