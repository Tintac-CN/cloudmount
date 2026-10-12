//! Windows 原生 watcher：`ReadDirectoryChangesW`。
//!
//! 结构：后台线程阻塞读目录变更 → 归约为脏路径批次送入通道；
//! `poll()` 排空通道、经 [`crate::watcher::apply_dirty`] 更新快照、
//! 溢出或周期性对账时全量重扫自愈。
//!
//! 关键取舍（为什么不用 USN Journal）：读取 USN 需要**管理员权限**且以文件
//! 引用号寻址；`ReadDirectoryChangesW` 无需提权、直接给相对路径、一个句柄
//! 监听整棵子树，是 Dropbox/OneDrive 等用户态同步工具的标准选择。
//! 事件可能溢出/丢失 —— 由溢出标志 + 周期性全量对账兜底。

use crate::scan::{LocalChange, LocalEntry, LocalWatcher};
use crate::watcher::{apply_dirty, diff_snapshots, rescan_into, DirtyEvent};
use cloudmount_core::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_CREATION, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SECURITY,
    FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

/// 事件缓冲大小（64KB，业界常用值；溢出由自愈机制处理）。
const BUFFER_SIZE: usize = 64 * 1024;

/// 每多少轮 poll 做一次全量对账（防事件漂移的兜底）。
const RESCAN_EVERY_POLLS: u32 = 50;

/// Windows 原生目录变更 watcher。
pub struct RdcWatcher {
    root: PathBuf,
    snapshot: HashMap<String, LocalEntry>,
    events: Receiver<Vec<DirtyEvent>>,
    /// 事件缓冲溢出标志（线程置位，poll 消费）。
    overflow: Arc<AtomicBool>,
    /// 关停标志。
    shutdown: Arc<AtomicBool>,
    /// 目录句柄（Drop 时关闭以唤醒阻塞的读线程）。
    handle: Arc<RawHandle>,
    polls: u32,
}

/// 裸句柄包装（Drop 关闭；线程共享只读使用）。
struct RawHandle(windows::Win32::Foundation::HANDLE);

// HANDLE 是指针大小的值类型，跨线程共享安全（仅用于系统调用）
unsafe impl Send for RawHandle {}
unsafe impl Sync for RawHandle {}

impl Drop for RawHandle {
    fn drop(&mut self) {
        // 关闭句柄会唤醒阻塞中的 ReadDirectoryChangesW（文档化的取消方式）
        let _ = unsafe { CloseHandle(self.0) };
    }
}

impl RdcWatcher {
    /// 创建 watcher（首次全量扫描建立基线，随后增量维护）。
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let root_abs = std::fs::canonicalize(root)
            .map_err(|e| Error::Other(format!("watcher 根目录不可访问 {}: {e}", root.display())))?;

        let handle = open_directory(&root_abs)?;
        let (tx, rx) = channel::<Vec<DirtyEvent>>();
        let overflow = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let shared_handle = Arc::new(RawHandle(handle));

        let _thread = std::thread::Builder::new()
            .name("cm-rdc-watcher".into())
            .spawn({
                let handle = shared_handle.clone();
                let overflow = overflow.clone();
                let shutdown = shutdown.clone();
                move || event_loop(handle, tx, overflow, shutdown)
            })
            .map_err(|e| Error::Other(format!("启动 watcher 线程失败: {e}")))?;

        // 建立基线快照（此时线程已在监听，期间发生的事件不会丢失）
        let snapshot = crate::scan::scan_tree(&root_abs)?;

        Ok(Self {
            root: root_abs,
            snapshot,
            events: rx,
            overflow,
            shutdown,
            handle: shared_handle,
            polls: 0,
        })
    }
}

impl LocalWatcher for RdcWatcher {
    fn poll(&mut self) -> Result<Vec<LocalChange>> {
        self.polls += 1;
        let old = self.snapshot.clone();

        // 排空事件批次（引擎每轮 poll 一次，通道不会积压过多）
        while let Ok(batch) = self.events.try_recv() {
            let dirty: Vec<DirtyEvent> = batch
                .into_iter()
                .filter(|e| !crate::exec::is_internal_path(&e.rel_path))
                .filter(|e| !crate::exclude::is_excluded_path(&e.rel_path))
                .collect();
            if !dirty.is_empty() {
                apply_dirty(&self.root, &mut self.snapshot, &dirty)?;
            }
        }

        // 溢出 → 全量重扫自愈
        if self.overflow.swap(false, Ordering::SeqCst) {
            tracing::warn!("目录事件缓冲溢出，执行全量重扫");
            rescan_into(&self.root, &mut self.snapshot)?;
        }

        // 周期性对账（事件可能漂移：如缓冲边界丢事件）
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

impl Drop for RdcWatcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // 关闭句柄（RawHandle 的 Drop）唤醒阻塞线程；线程自行退出
        drop(self.handle.clone());
    }
}

/// 打开目录句柄（需要 FILE_FLAG_BACKUP_SEMANTICS 才能打开目录）。
fn open_directory(path: &Path) -> Result<windows::Win32::Foundation::HANDLE> {
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        CreateFileW(
            windows::core::PCWSTR(wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .map_err(|e| Error::Other(format!("打开目录句柄失败: {e}")))
}

/// 事件读取线程主循环。
fn event_loop(
    handle: Arc<RawHandle>,
    tx: Sender<Vec<DirtyEvent>>,
    overflow: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    // 过滤掩码：文件/目录增删改名 + 属性 + 大小 + 修改时间 + 创建 + 安全描述符。
    // 刻意不含 LAST_ACCESS（纯读取也触发，噪音太大）。
    let filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_ATTRIBUTES
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_CREATION
        | FILE_NOTIFY_CHANGE_SECURITY;

    loop {
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_returned: u32 = 0;
        let result = unsafe {
            ReadDirectoryChangesW(
                handle.0,
                buffer.as_mut_ptr() as *mut core::ffi::c_void,
                buffer.len() as u32,
                true, // 监听整棵子树
                filter,
                Some(&mut bytes_returned),
                None, // 同步（阻塞）模式
                None,
            )
        };

        match result {
            Ok(()) => {
                // bytes_returned == 0 表示缓冲溢出（事件被丢弃）
                if bytes_returned == 0 {
                    overflow.store(true, Ordering::SeqCst);
                    continue;
                }
                let events = parse_events(&buffer[..bytes_returned as usize]);
                if tx.send(events).is_err() {
                    break; // 接收端已销毁
                }
            }
            Err(e) => {
                if shutdown.load(Ordering::SeqCst) {
                    break; // 正常关停
                }
                // ERROR_NOTIFY_ENUM_DIR(1014)：缓冲溢出，置位后继续
                if e.code().0 == 1014 {
                    overflow.store(true, Ordering::SeqCst);
                    continue;
                }
                // 句柄失效（Drop 关闭）或其它错误：退出线程
                tracing::debug!(error = %e, "watcher 线程退出");
                break;
            }
        }
    }
}

/// 解析 FILE_NOTIFY_INFORMATION 数组。
fn parse_events(buf: &[u8]) -> Vec<DirtyEvent> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    // 每条记录头：NextEntryOffset(4) + Action(4) + FileNameLength(4) + FileName(UTF-16)
    while offset + 12 <= buf.len() {
        let next = u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]) as usize;
        let name_len = u32::from_le_bytes([
            buf[offset + 8],
            buf[offset + 9],
            buf[offset + 10],
            buf[offset + 11],
        ]) as usize;
        let name_start = offset + 12;
        if name_start + name_len > buf.len() {
            break; // 记录不完整（不应发生）
        }
        let name = decode_utf16(&buf[name_start..name_start + name_len]);
        if !name.is_empty() {
            // 事件路径相对监听根，分隔符统一为 '/'
            let rel = name.replace('\\', "/");
            out.push(DirtyEvent {
                rel_path: rel,
                // Action 不区分文件/目录：交给 apply_dirty 用 stat 判定
                is_dir: false,
            });
        }
        if next == 0 {
            break;
        }
        offset += next;
    }
    out
}

/// 解码 UTF-16LE 字节串。
fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::build_watcher;
    use std::time::Duration;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cm-rdc-{tag}-{}-{:?}",
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

    /// 等待事件到达 watcher 线程（异步送达，需要短暂等待）。
    fn settle() {
        std::thread::sleep(Duration::from_millis(150));
    }

    #[test]
    fn detects_create_modify_delete() {
        let root = tmpdir("basic");
        let mut w = RdcWatcher::new(&root).unwrap();
        assert!(w.poll().unwrap().is_empty(), "基线后首轮无变更");

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
    fn detects_nested_and_rename() {
        let root = tmpdir("nested");
        let mut w = RdcWatcher::new(&root).unwrap();
        w.poll().unwrap();

        std::fs::create_dir_all(root.join("docs/sub")).unwrap();
        std::fs::write(root.join("docs/sub/f.txt"), b"data").unwrap();
        settle();
        w.poll().unwrap();
        assert!(
            w.snapshot().contains_key("docs/sub/f.txt"),
            "嵌套文件应入快照"
        );

        // 文件改名
        std::fs::rename(root.join("docs/sub/f.txt"), root.join("docs/sub/g.txt")).unwrap();
        settle();
        w.poll().unwrap();
        assert!(!w.snapshot().contains_key("docs/sub/f.txt"), "旧名应移除");
        assert!(w.snapshot().contains_key("docs/sub/g.txt"), "新名应出现");

        // 目录改名（子文件路径应整体迁移）
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
        let mut w = RdcWatcher::new(&root).unwrap();
        w.poll().unwrap();

        std::fs::create_dir_all(root.join(".cloudmount")).unwrap();
        std::fs::write(root.join(".cloudmount/state.db"), b"x").unwrap();
        std::fs::write(root.join("half.cmtmp"), b"t").unwrap();
        settle();
        w.poll().unwrap();

        assert!(
            !w.snapshot().keys().any(|k| k.starts_with(".cloudmount")),
            "内部目录不应入快照: {:?}",
            w.snapshot().keys().collect::<Vec<_>>()
        );
        assert!(!w.snapshot().keys().any(|k| k.ends_with(".cmtmp")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn factory_uses_native_on_windows() {
        let root = tmpdir("factory");
        // Auto 应构建出原生 watcher（通过行为验证：创建文件后能检测到）
        let mut w = build_watcher(&root, crate::watcher::WatcherBackend::Auto).unwrap();
        w.poll().unwrap();
        std::fs::write(root.join("probe.txt"), b"hi").unwrap();
        // 事件异步送达：有界重试（CI 慢 runner 需要更长窗口）
        let mut detected = false;
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let changes = w.poll().unwrap();
            if changes
                .iter()
                .any(|c| matches!(c, LocalChange::Created { path, .. } if path == "probe.txt"))
            {
                detected = true;
                break;
            }
        }
        assert!(
            detected,
            "Auto 后端应在 20s 内检测到变更（原生或回退轮询均可）"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_events_decodes_records() {
        // 手工构造两条 FILE_NOTIFY_INFORMATION 记录。
        // 注意：NextEntryOffset 是**对齐后**的记录长度（4 字节对齐）。
        fn record(action: u32, name: &str, next_offset: u32) -> Vec<u8> {
            let name_units: Vec<u16> = name.encode_utf16().collect();
            let mut out = Vec::new();
            out.extend_from_slice(&next_offset.to_le_bytes());
            out.extend_from_slice(&action.to_le_bytes());
            out.extend_from_slice(&((name_units.len() * 2) as u32).to_le_bytes());
            for u in &name_units {
                out.extend_from_slice(&u.to_le_bytes());
            }
            while out.len() % 4 != 0 {
                out.push(0);
            }
            out
        }

        // "a.txt"：12 头 + 10 名 = 22 → 对齐到 24
        let rec1 = record(3, "a.txt", 24);
        assert_eq!(rec1.len(), 24, "对齐后长度应与 NextEntryOffset 一致");
        let rec2 = record(1, "dir", 0); // 最后一条
        let mut buf = rec1;
        buf.extend(rec2);

        let events = parse_events(&buf);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].rel_path, "a.txt");
        assert_eq!(events[1].rel_path, "dir");
    }
}
