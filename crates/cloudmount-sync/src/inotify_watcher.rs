//! Linux 原生 watcher：inotify。
//!
//! 结构与 Windows [`crate::rdc`] 对称：后台线程阻塞读事件 → 归约为脏路径批次；
//! `poll()` 排空并经 [`crate::watcher::apply_dirty`] 更新快照。
//!
//! ## inotify 特有的处理
//!
//! - **递归监听**：inotify 不支持子树监听，需对每个目录加 watch；
//!   新目录出现（`CREATE|ISDIR` / `MOVED_TO|ISDIR`）时递归补挂（覆盖移入的整棵树）。
//! - **唤醒退出**：阻塞的 `read` 无法中断；Drop 时 touch 根目录下的
//!   `.cm-wake.cmtmp`（该名字会被扫描过滤，不污染同步）产生一个事件唤醒线程，
//!   线程检查关停标志后退出。
//! - **溢出**：`Q_OVERFLOW`（内核队列满，事件丢失）→ 置位溢出标志，poll 时全量重扫。
//! - **未知 watch 描述符**（初始化窗口期的事件）→ 保守置位溢出标志自愈。

use crate::scan::{LocalChange, LocalEntry, LocalWatcher};
use crate::watcher::{apply_dirty, diff_snapshots, rescan_into, DirtyEvent};
use cloudmount_core::{Error, Result};
use inotify::{EventMask, Inotify, WatchMask};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// 事件缓冲大小。
const BUFFER_SIZE: usize = 64 * 1024;

/// 每多少轮 poll 做一次全量对账。
const RESCAN_EVERY_POLLS: u32 = 50;

/// 唤醒文件名（`is_internal` 会过滤 `.cmtmp`，不进入同步快照）。
const WAKE_FILE: &str = ".cm-wake.cmtmp";

/// Linux inotify watcher。
pub struct InotifyWatcher {
    root: PathBuf,
    snapshot: HashMap<String, LocalEntry>,
    events: Receiver<Vec<DirtyEvent>>,
    overflow: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    polls: u32,
}

impl InotifyWatcher {
    /// 创建 watcher（基线快照 + 后台事件线程）。
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let root_abs = std::fs::canonicalize(root)
            .map_err(|e| Error::Other(format!("watcher 根目录不可访问 {}: {e}", root.display())))?;

        // 基线快照先于线程建立（初始挂 watch 期间的事件由溢出自愈兜底）
        let snapshot = crate::scan::scan_tree(&root_abs)?;

        let (tx, rx) = channel::<Vec<DirtyEvent>>();
        let overflow = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));

        let thread_root = root_abs.clone();
        let thread_overflow = overflow.clone();
        let thread_shutdown = shutdown.clone();
        std::thread::Builder::new()
            .name("cm-inotify-watcher".into())
            .spawn(move || event_loop(thread_root, tx, thread_overflow, thread_shutdown))
            .map_err(|e| Error::Other(format!("启动 watcher 线程失败: {e}")))?;

        Ok(Self {
            root: root_abs,
            snapshot,
            events: rx,
            overflow,
            shutdown,
            polls: 0,
        })
    }
}

impl LocalWatcher for InotifyWatcher {
    fn poll(&mut self) -> Result<Vec<LocalChange>> {
        self.polls += 1;
        let old = self.snapshot.clone();

        while let Ok(batch) = self.events.try_recv() {
            let dirty: Vec<DirtyEvent> = batch
                .into_iter()
                .filter(|e| !crate::exec::is_internal_path(&e.rel_path))
                .collect();
            if !dirty.is_empty() {
                apply_dirty(&self.root, &mut self.snapshot, &dirty)?;
            }
        }

        if self.overflow.swap(false, Ordering::SeqCst) {
            tracing::warn!("inotify 事件溢出，执行全量重扫");
            rescan_into(&self.root, &mut self.snapshot)?;
        }
        if self.polls.is_multiple_of(RESCAN_EVERY_POLLS) {
            rescan_into(&self.root, &mut self.snapshot)?;
        }

        Ok(diff_snapshots(&old, &self.snapshot))
    }

    fn snapshot(&self) -> &HashMap<String, LocalEntry> {
        &self.snapshot
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for InotifyWatcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // touch 唤醒文件 → 产生一个事件唤醒阻塞的 read → 线程看到关停标志后退出
        let wake = self.root.join(WAKE_FILE);
        if let Err(e) = std::fs::write(&wake, b"w") {
            tracing::debug!(error = %e, "写入唤醒文件失败（watcher 线程将随进程退出）");
        }
    }
}

/// 事件线程主循环。
fn event_loop(
    root: PathBuf,
    tx: Sender<Vec<DirtyEvent>>,
    overflow: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let mut inotify = match Inotify::init() {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = %e, "inotify 初始化失败");
            return;
        }
    };

    // watch 掩码：创建/删除/移入移出/写关闭/属性变化/自身删除与移动。
    // 不含 MODIFY（写过程中反复触发，CLOSE_WRITE 已覆盖落盘）。
    let mask = WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::MOVED_FROM
        | WatchMask::MOVED_TO
        | WatchMask::CLOSE_WRITE
        | WatchMask::ATTRIB
        | WatchMask::DELETE_SELF
        | WatchMask::MOVE_SELF;

    // wd → 相对路径
    let mut wd_map: HashMap<inotify::WatchDescriptor, String> = HashMap::new();
    add_watch_recursive(&mut inotify, &mut wd_map, &root, "", mask);

    loop {
        let mut buffer = [0u8; BUFFER_SIZE];
        match inotify.read_events(&mut buffer) {
            Ok(events) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                let mut dirty = Vec::new();
                for event in events {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    handle_event(
                        &event,
                        &mut inotify,
                        &mut wd_map,
                        &root,
                        mask,
                        &mut dirty,
                        &overflow,
                    );
                }
                if !dirty.is_empty() && tx.send(dirty).is_err() {
                    break; // 接收端已销毁
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                tracing::debug!(error = %e, "inotify 线程退出");
                break;
            }
        }
    }
}

/// 处理单个事件。
fn handle_event(
    event: &inotify::Event<&std::ffi::OsStr>,
    inotify: &mut Inotify,
    wd_map: &mut HashMap<inotify::WatchDescriptor, String>,
    root: &Path,
    mask: WatchMask,
    dirty: &mut Vec<DirtyEvent>,
    overflow: &AtomicBool,
) {
    if event.mask.contains(EventMask::Q_OVERFLOW) {
        overflow.store(true, Ordering::SeqCst);
        return;
    }
    if event.mask.contains(EventMask::IGNORED) {
        // 内核移除了该 watch（目录被删除）
        wd_map.remove(&event.wd);
        return;
    }

    let Some(dir_rel) = wd_map.get(&event.wd).cloned() else {
        // 未知 watch（初始化窗口期）：保守触发自愈
        overflow.store(true, Ordering::SeqCst);
        return;
    };

    let Some(name) = event.name.map(|n| n.to_string_lossy().to_string()) else {
        // 事件关于被监听目录自身（DELETE_SELF / MOVE_SELF）
        if event
            .mask
            .intersects(EventMask::DELETE_SELF | EventMask::MOVE_SELF)
        {
            if dir_rel.is_empty() {
                // 根目录自身被删/移：全量自愈
                overflow.store(true, Ordering::SeqCst);
            } else {
                dirty.push(DirtyEvent {
                    rel_path: dir_rel,
                    is_dir: true,
                });
            }
        }
        return;
    };

    let rel_path = if dir_rel.is_empty() {
        name
    } else {
        format!("{dir_rel}/{name}")
    };

    // 内部路径（含唤醒文件）：不产生脏事件
    if crate::exec::is_internal_path(&rel_path) {
        return;
    }

    let is_dir = event.mask.contains(EventMask::ISDIR);

    // 新目录 → 递归补挂 watch（覆盖移入的整棵子树）
    if is_dir
        && event
            .mask
            .intersects(EventMask::CREATE | EventMask::MOVED_TO)
    {
        add_watch_recursive(inotify, wd_map, root, &rel_path, mask);
    }

    dirty.push(DirtyEvent { rel_path, is_dir });
}

/// 递归挂 watch（跳过内部目录）。
fn add_watch_recursive(
    inotify: &mut Inotify,
    wd_map: &mut HashMap<inotify::WatchDescriptor, String>,
    root: &Path,
    rel: &str,
    mask: WatchMask,
) {
    let abs = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    let mut stack = vec![(abs, rel.to_string())];
    while let Some((dir, dir_rel)) = stack.pop() {
        match inotify.watches().add(&dir, mask) {
            Ok(wd) => {
                wd_map.insert(wd, dir_rel.clone());
            }
            Err(e) => {
                tracing::debug!(dir = %dir.display(), error = %e, "添加 watch 失败（跳过）");
                continue;
            }
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if crate::scan::is_internal(&name) {
                continue;
            }
            let path = ent.path();
            if path.is_dir()
                && !std::fs::symlink_metadata(&path)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
            {
                let child_rel = if dir_rel.is_empty() {
                    name
                } else {
                    format!("{dir_rel}/{name}")
                };
                stack.push((path, child_rel));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::build_watcher;
    use std::time::Duration;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cm-inotify-{tag}-{}-{:?}",
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

    fn settle() {
        std::thread::sleep(Duration::from_millis(150));
    }

    #[test]
    fn detects_create_modify_delete() {
        let root = tmpdir("basic");
        let mut w = InotifyWatcher::new(&root).unwrap();
        assert!(w.poll().unwrap().is_empty());

        std::fs::write(root.join("a.txt"), b"1").unwrap();
        settle();
        let changes = w.poll().unwrap();
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, LocalChange::Created { path, .. } if path == "a.txt")),
            "应检测到创建: {changes:?}"
        );

        std::fs::write(root.join("a.txt"), b"12345678").unwrap();
        settle();
        let changes = w.poll().unwrap();
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, LocalChange::Modified { path, .. } if path == "a.txt")),
            "应检测到修改: {changes:?}"
        );
        assert_eq!(w.snapshot()["a.txt"].size, 8);

        std::fs::remove_file(root.join("a.txt")).unwrap();
        settle();
        let changes = w.poll().unwrap();
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, LocalChange::Removed { path, .. } if path == "a.txt")),
            "应检测到删除: {changes:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_nested_new_dirs_and_rename() {
        let root = tmpdir("nested");
        let mut w = InotifyWatcher::new(&root).unwrap();
        w.poll().unwrap();

        // 新目录（watcher 应递归补挂 watch）
        std::fs::create_dir_all(root.join("docs/sub")).unwrap();
        std::fs::write(root.join("docs/sub/f.txt"), b"data").unwrap();
        settle();
        w.poll().unwrap();
        assert!(w.snapshot().contains_key("docs/sub/f.txt"));

        // 文件改名
        std::fs::rename(root.join("docs/sub/f.txt"), root.join("docs/sub/g.txt")).unwrap();
        settle();
        w.poll().unwrap();
        assert!(!w.snapshot().contains_key("docs/sub/f.txt"));
        assert!(w.snapshot().contains_key("docs/sub/g.txt"));

        // 目录改名（子树路径整体迁移）
        std::fs::rename(root.join("docs/sub"), root.join("docs/renamed")).unwrap();
        settle();
        w.poll().unwrap();
        assert!(!w.snapshot().contains_key("docs/sub/g.txt"));
        assert!(w.snapshot().contains_key("docs/renamed/g.txt"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn internal_paths_are_ignored() {
        let root = tmpdir("internal");
        let mut w = InotifyWatcher::new(&root).unwrap();
        w.poll().unwrap();

        std::fs::create_dir_all(root.join(".cloudmount")).unwrap();
        std::fs::write(root.join(".cloudmount/state.db"), b"x").unwrap();
        std::fs::write(root.join("half.cmtmp"), b"t").unwrap();
        settle();
        w.poll().unwrap();

        assert!(!w.snapshot().keys().any(|k| k.starts_with(".cloudmount")));
        assert!(!w.snapshot().keys().any(|k| k.ends_with(".cmtmp")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn factory_uses_native_on_linux() {
        let root = tmpdir("factory");
        let mut w = build_watcher(&root, crate::watcher::WatcherBackend::Auto).unwrap();
        w.poll().unwrap();
        std::fs::write(root.join("probe.txt"), b"hi").unwrap();
        settle();
        let changes = w.poll().unwrap();
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, LocalChange::Created { path, .. } if path == "probe.txt")),
            "Auto 后端应能检测到变更: {changes:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
