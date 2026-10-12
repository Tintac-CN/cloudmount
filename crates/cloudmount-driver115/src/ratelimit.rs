//! 限速与退避。
//!
//! 115 官方明确"频控细则不公开"，超限会被限流甚至冻结。因此**默认保守限速**
//! （社区经验约 1 r/s），并对 429/频控码做指数退避 + 熔断。

use cloudmount_core::{Error, Result};
use std::time::{Duration, Instant};

/// 简单令牌桶限速器（内部互斥，可安全地被 `&self` 方法调用与并发使用）。
pub struct RateLimiter {
    /// 内部状态。
    inner: std::sync::Mutex<TokenState>,
}

struct TokenState {
    /// 每秒补充的令牌数（速率）。
    rate_per_sec: f64,
    /// 桶容量（允许的突发）。
    capacity: f64,
    /// 当前令牌数。
    tokens: f64,
    /// 上次补充时间。
    last: Instant,
}

impl RateLimiter {
    /// 构造限速器。默认速率 1 r/s、容量 2（允许小突发）。
    pub fn new(rate_per_sec: f64, capacity: f64) -> Self {
        Self {
            inner: std::sync::Mutex::new(TokenState {
                rate_per_sec,
                capacity,
                tokens: capacity,
                last: Instant::now(),
            }),
        }
    }

    /// 官方驱动默认限速（保守）。
    pub fn conservative() -> Self {
        Self::new(1.0, 2.0)
    }

    /// 获取一个令牌；不足则返回需要等待的时长（`Error::RateLimited`）。
    pub fn try_acquire(&self) -> Result<()> {
        let mut st = self
            .inner
            .lock()
            .map_err(|_| Error::Other("rate limiter lock poisoned".into()))?;
        let now = Instant::now();
        let elapsed = now.duration_since(st.last).as_secs_f64();
        st.tokens = (st.tokens + elapsed * st.rate_per_sec).min(st.capacity);
        st.last = now;
        if st.tokens >= 1.0 {
            st.tokens -= 1.0;
            Ok(())
        } else {
            let wait = (1.0 - st.tokens) / st.rate_per_sec;
            Err(Error::RateLimited {
                retry_after_ms: (wait * 1000.0) as u64,
            })
        }
    }
}

/// 指数退避策略。
#[derive(Debug, Clone)]
pub struct Backoff {
    /// 当前退避时长。
    pub current: Duration,
    /// 基准。
    pub base: Duration,
    /// 上限。
    pub max: Duration,
    /// 连续失败次数。
    pub attempts: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            current: Duration::from_millis(500),
            base: Duration::from_millis(500),
            max: Duration::from_secs(60),
            attempts: 0,
        }
    }
}

impl Backoff {
    /// 记录一次失败，退避翻倍（带上限）。
    pub fn on_failure(&mut self) -> Duration {
        self.attempts += 1;
        self.current = (self.current * 2).min(self.max);
        self.current
    }

    /// 成功后重置。
    pub fn on_success(&mut self) {
        self.attempts = 0;
        self.current = self.base;
    }
}
