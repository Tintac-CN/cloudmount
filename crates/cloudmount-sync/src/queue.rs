//! 任务队列与调度：并发、重试退避、暂停。
//!
//! 设计取舍：
//! - `concurrency == 1` 时**严格按入队顺序串行执行**（测试与可复现性优先）；
//!   `> 1` 时由多个工作线程从共享队列取任务，path 之间天然独立。
//! - 重试采用指数退避（上限封顶），**只重试可重试错误**（由 `cloudmount_core::Error::is_retryable`
//!   判定：网络抖动 / 频控 / 5xx），认证与参数错误立即失败。
//! - 暂停语义：本轮不再取新任务，未处理的任务留待下一轮（队列状态由调用方重建）。

use cloudmount_core::{Error, Result};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 重试策略。
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// 最大尝试次数（含首次）。
    pub max_attempts: u32,
    /// 首次退避。
    pub base_delay: Duration,
    /// 退避上限。
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// 第 `attempt` 次失败后的退避时长（attempt 从 1 开始）。
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(16);
        let d = self.base_delay.saturating_mul(1u32 << shift);
        d.min(self.max_delay)
    }
}

/// 调度配置。
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// 并发度（1 = 串行、确定性）。
    pub concurrency: usize,
    /// 重试策略。
    pub retry: RetryPolicy,
    /// 外部暂停开关（UI/托盘可切换）。
    pub paused: Arc<AtomicBool>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            concurrency: 1,
            retry: RetryPolicy::default(),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// 单个任务的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    /// 任务标识（路径等，供统计与日志）。
    pub label: String,
    /// 是否成功。
    pub ok: bool,
    /// 实际尝试次数。
    pub attempts: u32,
    /// 失败原因。
    pub error: Option<String>,
}

/// 串行/并发执行任务集，带重试退避与暂停。
///
/// `worker` 必须对同一任务可重复调用（幂等或至少可安全重试）。
pub fn run_jobs<J, F>(
    jobs: Vec<J>,
    worker: F,
    cfg: &SchedulerConfig,
    label_of: impl Fn(&J) -> String + Send + Sync,
) -> Vec<JobOutcome>
where
    J: Send + Sync,
    F: Fn(&J) -> Result<()> + Send + Sync,
{
    let outcomes: Mutex<Vec<JobOutcome>> = Mutex::new(Vec::new());
    let cursor = AtomicUsize::new(0);
    let jobs = &jobs;
    let worker = &worker;
    let label_of = &label_of;

    let workers = cfg.concurrency.max(1);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    // 暂停：本轮到此为止，未处理任务留待下一轮
                    if cfg.paused.load(Ordering::Relaxed) {
                        break;
                    }
                    let idx = cursor.fetch_add(1, Ordering::SeqCst);
                    if idx >= jobs.len() {
                        break;
                    }
                    let job = &jobs[idx];
                    let mut attempts = 0u32;
                    let mut last_err: Option<Error> = None;

                    while attempts < cfg.retry.max_attempts.max(1) {
                        attempts += 1;
                        match worker(job) {
                            Ok(()) => {
                                last_err = None;
                                break;
                            }
                            Err(e) => {
                                let retryable = e.is_retryable();
                                tracing::warn!(
                                    attempt = attempts,
                                    retryable,
                                    error = %e,
                                    "任务失败"
                                );
                                last_err = Some(e);
                                if !retryable {
                                    break;
                                }
                                if attempts < cfg.retry.max_attempts.max(1) {
                                    let mut d = cfg.retry.delay_for(attempts);
                                    // 尊重频控建议的退避（取较大者）
                                    if let Some(Error::RateLimited { retry_after_ms }) =
                                        last_err.as_ref()
                                    {
                                        d = d.max(Duration::from_millis(*retry_after_ms));
                                    }
                                    std::thread::sleep(d);
                                }
                            }
                        }
                    }

                    outcomes.lock().expect("lock").push(JobOutcome {
                        label: label_of(job),
                        ok: last_err.is_none(),
                        attempts,
                        error: last_err.map(|e| e.to_string()),
                    });
                }
            });
        }
    });

    outcomes.into_inner().expect("lock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn backoff_grows_and_caps() {
        let p = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(400),
        };
        assert_eq!(p.delay_for(1), Duration::from_millis(100));
        assert_eq!(p.delay_for(2), Duration::from_millis(200));
        assert_eq!(p.delay_for(3), Duration::from_millis(400));
        assert_eq!(p.delay_for(9), Duration::from_millis(400), "应封顶");
    }

    fn fast_cfg(concurrency: usize) -> SchedulerConfig {
        SchedulerConfig {
            concurrency,
            retry: RetryPolicy {
                max_attempts: 3,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(2),
            },
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn serial_run_keeps_order() {
        let jobs: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let seen = Mutex::new(Vec::new());
        let out = run_jobs(
            jobs,
            |j| {
                seen.lock().unwrap().push(j.clone());
                Ok(())
            },
            &fast_cfg(1),
            |j| j.clone(),
        );
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|o| o.ok && o.attempts == 1));
        assert_eq!(seen.into_inner().unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn retries_retryable_errors_then_succeeds() {
        let jobs = vec!["flaky".to_string()];
        let calls = AtomicU32::new(0);
        let out = run_jobs(
            jobs,
            |_| {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(Error::RateLimited { retry_after_ms: 1 })
                } else {
                    Ok(())
                }
            },
            &fast_cfg(1),
            |j| j.clone(),
        );
        assert_eq!(out[0].attempts, 3);
        assert!(out[0].ok, "第 3 次应成功");
    }

    #[test]
    fn does_not_retry_non_retryable() {
        let jobs = vec!["fatal".to_string()];
        let calls = AtomicU32::new(0);
        let out = run_jobs(
            jobs,
            |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(Error::Invalid("参数错误".into()))
            },
            &fast_cfg(1),
            |j| j.clone(),
        );
        assert_eq!(out[0].attempts, 1, "不可重试错误应只尝试一次");
        assert!(!out[0].ok);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn pause_skips_all_work() {
        let cfg = fast_cfg(1);
        cfg.paused.store(true, Ordering::Relaxed);
        let calls = AtomicU32::new(0);
        let out = run_jobs(
            vec!["x".to_string(), "y".to_string()],
            |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            &cfg,
            |j| j.clone(),
        );
        assert!(out.is_empty(), "暂停时不应执行任何任务");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn concurrent_processes_every_job_exactly_once() {
        let jobs: Vec<u32> = (0..50).collect();
        let counter = Mutex::new(Vec::new());
        let out = run_jobs(
            jobs.clone(),
            |j| {
                counter.lock().unwrap().push(*j);
                Ok(())
            },
            &fast_cfg(4),
            |j| j.to_string(),
        );
        assert_eq!(out.len(), 50);
        assert!(out.iter().all(|o| o.ok));
        let mut got = counter.into_inner().unwrap();
        got.sort_unstable();
        assert_eq!(got, jobs, "每个任务应恰好执行一次");
    }
}
