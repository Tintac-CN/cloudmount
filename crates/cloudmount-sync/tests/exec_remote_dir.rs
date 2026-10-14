//! F-07（T-11/T-12）：`ensure_remote_dir` 锁内计划 / 锁外执行。
//!
//! 判据（test-strategy T-11/T-12，主判据 = 纯测试侧时间窗交叠，零生产 seam）：
//! - **T-11 主判据**：SlowMockRemote（list/create_dir 各延迟 500ms）在桩侧记录每次
//!   网络调用的起止时间窗；不同前缀两目录（`a/b` 与 `c/d`）并发 ensure → 存在
//!   时间窗**交叠**。现状整段持 `dir_ids` 锁 ⇒ 严格串行、无交叠（红测判别成立）。
//! - **T-11 副判据（AC7(b)）**：已缓存目录 Y 的查询在 X 的网络往返完成前返回
//!   （Y 完成时间 < X 串行耗时 60%，重复 5 轮）。现状 Y 也要排队等整段锁 ⇒ 红。
//! - **T-12**：create_dir Err 仅该条目报错、锁释放、后续事件继续；并发 N 线程
//!   同一深层目录 → 每组件 create_dir 恰一次、远端无重复节点（全体同一 ID）。

mod common;

use common::MockRemote;
use cloudmount_core::{
    ChangeProbe, Error, Provider, Quota, RangeSpec, RemoteEntry, RemoteId, Result, UploadResult,
    UploadSource,
};
use cloudmount_sync::cache::BlockCache;
use cloudmount_sync::exec::Executor;
use cloudmount_sync::reconcile::{Action, SyncPolicy};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};

/// 单次网络调用的起止时间窗（桩侧记录，T-11 主判据）。
#[derive(Clone, Copy, Debug)]
struct CallWindow {
    op: &'static str,
    start: Instant,
    end: Instant,
}

/// 慢速远端：以 `Arc<dyn Provider>` 委托包装内层 Provider（testutil MockRemote
/// 作内层），list/create_dir 各延迟 `delay`，并记录每次调用的起止时间窗。
struct SlowMockRemote {
    inner: Arc<dyn Provider>,
    delay: Duration,
    windows: Mutex<Vec<CallWindow>>,
}

impl SlowMockRemote {
    /// 包装内层 Provider（MockRemote 经 `Arc` 转为 `dyn Provider`）。
    fn wrapping(inner: Arc<dyn Provider>, delay: Duration) -> Self {
        Self {
            inner,
            delay,
            windows: Mutex::new(Vec::new()),
        }
    }

    fn windows(&self) -> Vec<CallWindow> {
        self.windows.lock().unwrap().clone()
    }
}

/// 时间窗是否存在两两交叠（交叠 ⇒ 网络调用未被全局锁串行化）。
fn has_overlap(windows: &[CallWindow]) -> bool {
    for i in 0..windows.len() {
        for j in (i + 1)..windows.len() {
            let (a, b) = (windows[i], windows[j]);
            if a.start < b.end && b.start < a.end {
                return true;
            }
        }
    }
    false
}

/// 时间窗序列的可读渲染（断言诊断用）。
fn render(windows: &[CallWindow]) -> String {
    windows
        .iter()
        .map(|w| format!("{}[{:?}..{:?}]", w.op, w.start, w.end))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Provider for SlowMockRemote {
    fn name(&self) -> &'static str {
        "slow-mock"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        let start = Instant::now();
        std::thread::sleep(self.delay);
        let out = self.inner.list(dir);
        self.windows.lock().unwrap().push(CallWindow {
            op: "list",
            start,
            end: Instant::now(),
        });
        out
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        self.inner.stat(id)
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        self.inner.open_range(id, range)
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        let start = Instant::now();
        std::thread::sleep(self.delay);
        let out = self.inner.create_dir(parent, name);
        self.windows.lock().unwrap().push(CallWindow {
            op: "create_dir",
            start,
            end: Instant::now(),
        });
        out
    }

    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        self.inner.upload(parent, name, source, size)
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        self.inner.rename(id, new_name)
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        self.inner.move_entry(id, new_parent)
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        self.inner.copy(id, new_parent)
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        self.inner.delete(id)
    }

    fn quota(&self) -> Result<Quota> {
        self.inner.quota()
    }

    fn change_probe(&self, dir: &RemoteId) -> Result<Option<ChangeProbe>> {
        self.inner.change_probe(dir)
    }
}

/// 注入 create_dir 失败的包装（T-12：对指定目录名返回 Err，其余全部委托）。
struct FailCreateDirNamed {
    inner: Arc<dyn Provider>,
    fail_name: &'static str,
}

impl Provider for FailCreateDirNamed {
    fn name(&self) -> &'static str {
        "fail-create-mock"
    }

    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.inner.list(dir)
    }

    fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
        self.inner.stat(id)
    }

    fn open_range(&self, id: &RemoteId, range: RangeSpec) -> Result<Box<dyn Read + Send>> {
        self.inner.open_range(id, range)
    }

    fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
        if name == self.fail_name {
            return Err(Error::Other(format!("注入的 create_dir 失败: {name}")));
        }
        self.inner.create_dir(parent, name)
    }

    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        self.inner.upload(parent, name, source, size)
    }

    fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
        self.inner.rename(id, new_name)
    }

    fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
        self.inner.move_entry(id, new_parent)
    }

    fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
        self.inner.copy(id, new_parent)
    }

    fn delete(&self, id: &RemoteId) -> Result<()> {
        self.inner.delete(id)
    }

    fn quota(&self) -> Result<Quota> {
        self.inner.quota()
    }

    fn change_probe(&self, dir: &RemoteId) -> Result<Option<ChangeProbe>> {
        self.inner.change_probe(dir)
    }
}

/// 每个测试独立的临时工作区（本地根 + 缓存目录），Drop 时清理。
struct Workspace {
    root: PathBuf,
    cache_dir: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "cm-exec-dir-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("root");
        let cache_dir = base.join("cache");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&cache_dir).unwrap();
        Self { root, cache_dir }
    }

    fn write(&self, rel: &str, data: &[u8]) {
        let p = self.root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, data).unwrap();
    }

    fn cache(&self) -> BlockCache {
        BlockCache::open(&self.cache_dir, 1_000_000).unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if let Some(base) = self.root.parent() {
            let _ = std::fs::remove_dir_all(base);
        }
    }
}

fn as_provider(p: &impl Provider) -> &dyn Provider {
    p
}

/// ---------- T-11 主判据：不同前缀并发建目录，网络调用时间窗必须交叠 ----------

#[test]
fn t11_concurrent_different_prefix_dirs_overlap_network_windows() {
    let mock = MockRemote::new();
    let slow = SlowMockRemote::wrapping(Arc::new(mock.clone()), Duration::from_millis(500));
    let ws = Workspace::new("t11-overlap");
    let policy = SyncPolicy::default();
    let cache = ws.cache();

    // 初始缓存为空：a/b 与 c/d 两条链完全缺失，各自需要 list+create_dir 网络 IO。
    let exec = Arc::new(Executor::new(
        as_provider(&slow),
        &ws.root,
        &policy,
        &cache,
        HashMap::new(),
    ));

    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|s| {
        let ex1 = exec.clone();
        let ex2 = exec.clone();
        let b1 = barrier.clone();
        let b2 = barrier.clone();
        let h1 = s.spawn(move || {
            b1.wait();
            ex1.execute(&Action::MkdirRemote {
                path: "a/b".to_string(),
            })
        });
        let h2 = s.spawn(move || {
            b2.wait();
            ex2.execute(&Action::MkdirRemote {
                path: "c/d".to_string(),
            })
        });
        (h1.join().unwrap(), h2.join().unwrap())
    });
    assert!(results.0.is_ok(), "a/b 建目录失败: {results:?}");
    assert!(results.1.is_ok(), "c/d 建目录失败: {results:?}");

    // 远端语义正确：两条链都建成、无重复节点
    let mut paths = mock.all_paths();
    paths.sort();
    paths.dedup();
    assert_eq!(paths, vec!["a", "a/b", "c", "c/d"]);

    // 主判据：桩侧时间窗存在交叠（现状整段持锁 ⇒ 严格串行、无交叠 ⇒ 此处红）
    let windows = slow.windows();
    assert!(
        windows.len() >= 4,
        "应有至少 4 次慢速网络调用（两链 × list+create_dir）: {}",
        render(&windows)
    );
    assert!(
        has_overlap(&windows),
        "不同前缀并发建目录的网络调用时间窗必须交叠（dir_ids 锁不得跨网络持有）: {}",
        render(&windows)
    );
}

/// ---------- T-11 副判据（AC7(b)）：已缓存 Y 在 X 网络往返完成前返回 ----------

#[test]
fn t11_cached_dir_y_returns_before_uncached_x_network_roundtrip() {
    let rounds = 5;
    for round in 0..rounds {
        let mock = MockRemote::new();
        let y_id = mock.put_dir("y");
        let slow = SlowMockRemote::wrapping(Arc::new(mock.clone()), Duration::from_millis(500));
        let ws = Workspace::new(&format!("t11-cached-{round}"));
        let policy = SyncPolicy::default();
        let cache = ws.cache();

        // 预置缓存：y 已解析（零网络）；x 缺失（需 list + create_dir，共 ~1s）。
        let dir_ids = HashMap::from([
            (String::new(), RemoteId("0".to_string())),
            ("y".to_string(), RemoteId(y_id)),
        ]);
        let exec = Arc::new(Executor::new(
            as_provider(&slow),
            &ws.root,
            &policy,
            &cache,
            dir_ids,
        ));

        let t0 = Instant::now();
        let (x_elapsed, y_elapsed) = std::thread::scope(|s| {
            let ex1 = exec.clone();
            let ex2 = exec.clone();
            // X 先起（争取先持锁，保证红相位的串行形态可复现）
            let hx = s.spawn(move || {
                ex1.execute(&Action::MkdirRemote {
                    path: "x".to_string(),
                })
                .unwrap();
                Instant::now().duration_since(t0)
            });
            let hy = s.spawn(move || {
                ex2.execute(&Action::MkdirRemote {
                    path: "y".to_string(),
                })
                .unwrap();
                Instant::now().duration_since(t0)
            });
            (hx.join().unwrap(), hy.join().unwrap())
        });

        // 副判据一：Y 在 X 仍在网络往返中时已返回（Y 完成时间 < X 串行耗时 60%）。
        // 现状 Y 排队等整段锁 ⇒ y ≈ x ⇒ 红修复后 y 为纯缓存命中 ⇒ 稳定绿。
        assert!(
            y_elapsed.as_millis() * 5 < x_elapsed.as_millis() * 3,
            "第 {round} 轮：已缓存 Y 应在 X 网络往返完成前返回（y={y_elapsed:?}, x={x_elapsed:?}）"
        );

        // 副判据二：Y 完成时刻早于 X 最后一次网络调用的窗口结束时刻
        // （Y 的查询不做任何网络调用 ⇒ 全部窗口都属于 X）。
        let y_done_abs = t0 + y_elapsed;
        let windows = slow.windows();
        assert!(
            !windows.is_empty(),
            "第 {round} 轮：X 应产生网络调用（list+create_dir）"
        );
        let x_last_end = windows.iter().map(|w| w.end).max().unwrap();
        assert!(
            y_done_abs < x_last_end,
            "第 {round} 轮：Y 应在 X 的网络往返结束前返回（y_done={y_done_abs:?}, x 最后窗口结束={x_last_end:?}）"
        );
    }
}

/// ---------- T-12：create_dir Err 不卡死、后续事件继续 ----------

#[test]
fn t12_create_dir_error_releases_lock_and_later_events_continue() {
    let mock = MockRemote::new();
    let provider = FailCreateDirNamed {
        inner: Arc::new(mock.clone()),
        fail_name: "jinx",
    };
    let ws = Workspace::new("t12-error");
    let policy = SyncPolicy::default();
    let cache = ws.cache();
    let exec = Executor::new(&provider, &ws.root, &policy, &cache, HashMap::new());

    // 失败条目：仅该条目报错
    let err = exec
        .execute(&Action::MkdirRemote {
            path: "jinx".to_string(),
        })
        .expect_err("注入的 create_dir 失败必须上抛");
    assert!(err.to_string().contains("jinx"), "错误应指明失败条目: {err}");

    // 锁已释放（未被毒化/持有）：后续事件继续处理
    exec.execute(&Action::MkdirRemote {
        path: "alpha".to_string(),
    })
    .expect("后续事件应正常继续");

    // 远端终态：成功条目建成，失败条目无半成品
    assert!(mock.exists("alpha"), "成功条目应已建到远端");
    assert!(!mock.exists("jinx"), "失败条目不应在远端留下半成品");

    let outcome = exec.finish();
    assert_eq!(outcome.stats.dirs_created, 1, "仅成功条目计入统计");
}

/// ---------- T-12：并发同深层目录 → 每组件 create_dir 恰一次、全体同一 ID ----------

#[test]
fn t12_concurrent_same_deep_dir_creates_each_component_exactly_once() {
    let mock = MockRemote::new();
    let slow = SlowMockRemote::wrapping(Arc::new(mock.clone()), Duration::from_millis(500));
    let ws = Workspace::new("t12-concurrent");
    let policy = SyncPolicy::default();
    let cache = ws.cache();

    const N: usize = 8;
    // 8 线程并发上传同一深层目录下的不同文件 → 内部并发 ensure_remote_dir("p/q/r")
    for i in 0..N {
        ws.write(&format!("p/q/r/f{i}.txt"), format!("content-{i}").as_bytes());
    }

    let exec = Arc::new(Executor::new(
        as_provider(&slow),
        &ws.root,
        &policy,
        &cache,
        HashMap::new(),
    ));

    let barrier = Arc::new(Barrier::new(N));
    std::thread::scope(|s| {
        for i in 0..N {
            let ex = exec.clone();
            let bar = barrier.clone();
            s.spawn(move || {
                bar.wait();
                ex.execute(&Action::Upload {
                    path: format!("p/q/r/f{i}.txt"),
                })
                .unwrap();
            });
        }
    });

    // create_dir 恰好每组件一次：p、p/q、p/q/r（不因并发而重复）
    let create_dirs = mock.ops().create_dir.load(Ordering::SeqCst);
    assert_eq!(create_dirs, 3, "三个缺失组件各恰建一次，实际 {create_dirs} 次");

    // 全体同一 ID：远端树无重复节点——8 个文件全部落在同一个 p/q/r 节点下
    let mut paths = mock.all_paths();
    let mut expected: Vec<String> = vec!["p".into(), "p/q".into(), "p/q/r".into()];
    expected.extend((0..N).map(|i| format!("p/q/r/f{i}.txt")));
    expected.sort();
    paths.sort();
    assert_eq!(paths, expected, "远端应恰为 3 个目录 + {N} 个文件（无重复节点）");

    for i in 0..N {
        assert_eq!(
            mock.read(&format!("p/q/r/f{i}.txt")).unwrap(),
            format!("content-{i}").into_bytes(),
            "文件 {i} 应落在统一的 p/q/r 目录下"
        );
    }
}
