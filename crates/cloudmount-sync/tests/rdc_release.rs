//! F-06 RdcWatcher 真释放（T-09/T-10）——windows 门控集成测试。
//!
//! 测试位置说明（架构 MA6 seam）：`ACTIVE_WATCHERS` 活跃计数是**进程级**
//! 全局量——本文件是独立测试二进制（独立进程），避免与 lib 单元测试中
//! 并行存活的 watcher（如 factory 用例可达 20s）互踩回落基线；文件内以
//! [`test_lock`] 串行化本文件的用例，保证计数断言期间无并发 watcher。
//!
//! 映射：test-strategy T-09（P1，50-100 轮断言下限 50）/ T-10（P2 容错）。
//! 超时/detach 分支无确定性造红手段（本地目录 CloseHandle 实践必醒，r2
//! qa min-5）：自动化侧仅做 warn 日志捕获断言（正常路径应无 detach
//! warn），分支本身归人工验证兜底。

#![cfg(windows)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use cloudmount_sync::rdc::{active_watcher_count, RdcWatcher};

/// 串行化本文件用例：活跃计数断言期间不容并发 watcher 干扰基线。
/// 毒化容忍：一个用例失败不级联出误导性 PoisonError。
fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_test_mutex() -> std::sync::MutexGuard<'static, ()> {
    test_lock().lock().unwrap_or_else(|e| e.into_inner())
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "cm-rdc-release-{tag}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 有界轮询：条件满足即返回 true（轮询而非单次断言，上限 5s——T-09 口径）。
fn wait_until(f: impl Fn() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

/// T-09：50 轮"创建→drop"后活跃计数回落基线；drop 后被监视目录可 rename/remove。
///
/// 红测判据：现状 `drop(self.handle.clone())` 净变化为零、线程永不退出，
/// 计数净增长 +50 且不回落（泄漏信号放大，单轮时序抖动不误报）。
#[test]
fn t09_drop_releases_thread_and_directory_handle() {
    let _g = lock_test_mutex();

    const ROUNDS: usize = 50;
    let root = tmpdir("t09");
    let baseline = active_watcher_count();

    for i in 0..ROUNDS {
        let w =
            RdcWatcher::new(&root).unwrap_or_else(|e| panic!("第 {i} 轮创建 watcher 失败: {e}"));
        drop(w);
    }

    let settled = wait_until(
        || active_watcher_count() <= baseline,
        Duration::from_secs(5),
    );
    assert!(
        settled,
        "50 轮创建→drop 后活跃计数未回落基线（泄漏）: baseline={baseline}, now={}",
        active_watcher_count()
    );

    // windows 腿（句柄确已释放）：drop 后可 rename/remove 被监视目录。
    let nested = root.join("sub");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("f.txt"), b"x").unwrap();
    drop(RdcWatcher::new(&root).unwrap());
    assert!(
        wait_until(
            || active_watcher_count() <= baseline,
            Duration::from_secs(5)
        ),
        "第二次 drop 后活跃计数未回落基线: baseline={baseline}, now={}",
        active_watcher_count()
    );
    let renamed = root.join("sub-renamed");
    std::fs::rename(&nested, &renamed).expect("drop 后应可 rename 被监视目录内的子目录");
    std::fs::remove_dir_all(&root).expect("drop 后应可 remove 被监视的根目录");

    let _ = std::fs::remove_dir_all(&root);
}

/// 手写捕获型 tracing Subscriber（仅 tracing 本体，零新依赖——F-04 同款模式）。
struct CapturingSubscriber {
    events: Arc<Mutex<Vec<String>>>,
}

impl tracing::Subscriber for CapturingSubscriber {
    fn enabled(&self, _meta: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {
        // 本文件不使用 span 字段
    }

    fn record_follows_from(&self, _id: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        struct MsgVisitor(Vec<String>);
        impl tracing::field::Visit for MsgVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.push(format!("{}={:?}", field.name(), value));
            }
        }
        let mut v = MsgVisitor(Vec::new());
        event.record(&mut v);
        self.events.lock().unwrap().push(format!(
            "[{}] {} {}",
            event.metadata().level(),
            event.metadata().target(),
            v.0.join(" ")
        ));
    }

    fn enter(&self, _id: &tracing::span::Id) {}
    fn exit(&self, _id: &tracing::span::Id) {}
}

/// T-10（P2 容错）：事件洪峰后 drop → 不 panic、不悬挂（悬挂即测试超时
/// 失败）、计数回落基线；正常有界收割不应出现超时/detach warn（捕获型
/// Subscriber 断言）。超时/detach 分支本身无确定性造红手段，人工验证兜底。
#[test]
fn t10_event_flood_then_drop_is_bounded_and_count_settles() {
    let _g = lock_test_mutex();

    let root = tmpdir("t10");
    let baseline = active_watcher_count();
    let w = RdcWatcher::new(&root).unwrap();

    // 灌入事件洪峰：批量创建 + 改名（不要求全部送达，只要求在途压力）
    for i in 0..300 {
        let p = root.join(format!("flood-{i}.txt"));
        std::fs::write(&p, b"flood").unwrap();
        let _ = std::fs::rename(&p, root.join(format!("flood-{i}-r.txt")));
    }
    // 给线程一点时间消费洪峰
    std::thread::sleep(Duration::from_millis(300));

    // 捕获 drop 期间（测试线程上）的日志：detach 分支会发 warn。
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CapturingSubscriber {
        events: events.clone(),
    };
    tracing::subscriber::with_default(subscriber, || {
        drop(w); // 若收割悬挂 → 本测试永不返回（悬挂即失败）
    });

    assert!(
        wait_until(
            || active_watcher_count() <= baseline,
            Duration::from_secs(5)
        ),
        "洪峰后 drop 活跃计数未回落基线: baseline={baseline}, now={}",
        active_watcher_count()
    );
    let warns: Vec<String> = events
        .lock()
        .unwrap()
        .iter()
        .filter(|l| l.contains("WARN"))
        .cloned()
        .collect();
    assert!(
        warns.is_empty(),
        "本地目录 drop 应有界收割成功，不应出现超时/detach warn: {warns:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
