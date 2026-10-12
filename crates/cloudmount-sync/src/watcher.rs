//! 本地 watcher 工厂与共享的"脏路径应用"逻辑。
//!
//! ## 架构
//!
//! 平台原生 watcher（Windows [`rdc`] / Linux [`inotify_watcher`]）不直接维护
//! 完整快照的增删逻辑，而是把事件归约为**脏路径集**（`DirtyEvent`），再由
//! [`apply_dirty`] 统一应用到快照上：
//!
//! ```text
//! 原生事件（增/删/改/改名） ──▶ 脏路径集 ──▶ apply_dirty：
//!     ├─ 路径是目录且存在   → 重扫该子树（scan_subtree）合并
//!     ├─ 路径是目录且不存在 → 前缀匹配整棵移除
//!     ├─ 路径是文件         → stat 更新或移除
//!     └─ 事件缓冲溢出       → 全量重扫自愈
//! ```
//!
//! 这样把"事件语义"（各平台不同）与"快照维护"（平台无关）解耦：
//! - 目录改名只产生两个目录事件，子文件路径全部由子树重扫自动修正；
//! - 事件丢失/乱序由溢出自愈与周期性全量对账兜底。
//!
//! ## 为什么 Windows 用 ReadDirectoryChangesW 而不是 USN Journal
//!
//! USN Journal 读取需要**管理员权限**（打开卷句柄），且以文件引用号寻址、
//! 需要自建 FRN→路径映射。`ReadDirectoryChangesW` 是用户态同步工具的行业标准
//! （Dropbox/OneDrive 同款）：无需提权、直接给相对路径、一个句柄监听整棵子树。
//! 代价是事件可能溢出/丢失——由上述自愈机制兜底。

use crate::scan::{scan_tree, LocalChange, LocalEntry, LocalWatcher, PollingWatcher};
use cloudmount_core::Result;
use std::collections::HashMap;
use std::path::Path;

/// watcher 后端选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatcherBackend {
    /// 按平台自动选择（原生优先，失败回退轮询）。
    #[default]
    Auto,
    /// 强制轮询（可移植基线，用于诊断/对照）。
    Polling,
    /// 强制平台原生实现（Windows: RDC / Linux: inotify / 其它: 轮询）。
    /// 注意：必须无条件定义——用 cfg 按平台拆分会导致某些平台（如 macOS）
    /// 上该变体不存在，而 match 分支仍然引用它，编译失败。
    Native,
}

/// 一条"脏路径"事件（原生 watcher 归约后的产物）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyEvent {
    /// 相对根的路径（`/` 分隔）。
    pub rel_path: String,
    /// 事件指向目录（不确定时按文件处理，stat 会纠正）。
    pub is_dir: bool,
}

/// 按平台构建 watcher。
///
/// `Auto`：Windows/Linux 用原生实现，macOS 暂用轮询（FSEvents 待 M2+）；
/// 原生初始化失败（如网络盘/权限）自动回退轮询并记录告警。
pub fn build_watcher(
    root: impl AsRef<Path>,
    backend: WatcherBackend,
) -> Result<Box<dyn LocalWatcher>> {
    let root = root.as_ref();
    match backend {
        WatcherBackend::Polling => Ok(Box::new(PollingWatcher::new(root)?)),
        WatcherBackend::Auto | WatcherBackend::Native => build_native(root),
    }
}

#[cfg(windows)]
fn build_native(root: &Path) -> Result<Box<dyn LocalWatcher>> {
    match crate::rdc::RdcWatcher::new(root) {
        Ok(w) => Ok(Box::new(w)),
        Err(e) => {
            tracing::warn!(error = %e, "原生 watcher 不可用，回退轮询扫描");
            Ok(Box::new(PollingWatcher::new(root)?))
        }
    }
}

#[cfg(target_os = "linux")]
fn build_native(root: &Path) -> Result<Box<dyn LocalWatcher>> {
    match crate::inotify_watcher::InotifyWatcher::new(root) {
        Ok(w) => Ok(Box::new(w)),
        Err(e) => {
            tracing::warn!(error = %e, "原生 watcher 不可用，回退轮询扫描");
            Ok(Box::new(PollingWatcher::new(root)?))
        }
    }
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn build_native(root: &Path) -> Result<Box<dyn LocalWatcher>> {
    // macOS：FSEvents 绑定待 M2+，当前用轮询（行为正确，只是大目录下更费 I/O）
    tracing::debug!("当前平台无原生 watcher，使用轮询扫描");
    Ok(Box::new(PollingWatcher::new(root)?))
}

/// 把脏路径集应用到快照（**原地修改**）。
///
/// 返回是否有路径被实际改动（供调用方决定是否需要计算变更集）。
pub fn apply_dirty(
    root: &Path,
    snapshot: &mut HashMap<String, LocalEntry>,
    dirty: &[DirtyEvent],
) -> Result<()> {
    for event in dirty {
        let abs = join_rel(root, &event.rel_path);
        let meta = std::fs::symlink_metadata(&abs);
        match meta {
            Ok(m) if m.is_dir() => {
                // 目录存在 → 重扫子树并合并（新增/改名目录的子文件由此自动修正）
                let subtree = crate::scan::scan_subtree(root, &event.rel_path)?;
                // 先移除旧前缀（目录可能被改名后事件路径是旧名）
                remove_prefix(snapshot, &event.rel_path);
                for (path, entry) in subtree {
                    snapshot.insert(path, entry);
                }
                // 目录自身
                snapshot.insert(
                    event.rel_path.clone(),
                    LocalEntry {
                        is_dir: true,
                        size: 0,
                        mtime: crate::scan::mtime_secs(&m),
                    },
                );
            }
            Ok(m) => {
                // 文件存在 → 更新
                snapshot.insert(
                    event.rel_path.clone(),
                    LocalEntry {
                        is_dir: false,
                        size: m.len(),
                        mtime: crate::scan::mtime_secs(&m),
                    },
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 不存在 → 移除该路径及其子树
                snapshot.remove(&event.rel_path);
                remove_prefix(snapshot, &event.rel_path);
            }
            Err(_) => {
                // 其它错误（权限等）：保守起见标记为移除（下轮全量对账会纠正）
                snapshot.remove(&event.rel_path);
                remove_prefix(snapshot, &event.rel_path);
            }
        }
    }
    Ok(())
}

/// 全量重扫并替换快照（溢出自愈用）。
pub fn rescan_into(root: &Path, snapshot: &mut HashMap<String, LocalEntry>) -> Result<()> {
    let fresh = scan_tree(root)?;
    snapshot.clear();
    snapshot.extend(fresh);
    Ok(())
}

/// 移除以 `prefix/` 开头的所有子路径。
fn remove_prefix(snapshot: &mut HashMap<String, LocalEntry>, prefix: &str) {
    if prefix.is_empty() {
        snapshot.clear();
        return;
    }
    let p = format!("{prefix}/");
    snapshot.retain(|k, _| !k.starts_with(&p));
}

/// 对比新旧快照，产出变更集（轮询与原生 watcher 共用的差异计算）。
pub fn diff_snapshots(
    old: &HashMap<String, LocalEntry>,
    new: &HashMap<String, LocalEntry>,
) -> Vec<LocalChange> {
    let mut changes = Vec::new();
    for (path, entry) in new {
        match old.get(path) {
            None => changes.push(LocalChange::Created {
                path: path.clone(),
                entry: entry.clone(),
            }),
            Some(prev) if prev != entry => changes.push(LocalChange::Modified {
                path: path.clone(),
                entry: entry.clone(),
            }),
            Some(_) => {}
        }
    }
    for (path, prev) in old {
        if !new.contains_key(path) {
            changes.push(LocalChange::Removed {
                path: path.clone(),
                was_dir: prev.is_dir,
            });
        }
    }
    changes
}

/// 相对路径 → 绝对路径。
fn join_rel(root: &Path, rel: &str) -> std::path::PathBuf {
    if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cm-watcher-{tag}-{}-{:?}",
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

    #[test]
    fn dirty_file_add_update_remove() {
        let root = tmpdir("file");
        let mut snap = HashMap::new();

        std::fs::write(root.join("a.txt"), b"1").unwrap();
        apply_dirty(
            &root,
            &mut snap,
            &[DirtyEvent {
                rel_path: "a.txt".into(),
                is_dir: false,
            }],
        )
        .unwrap();
        assert_eq!(snap["a.txt"].size, 1);

        std::fs::write(root.join("a.txt"), b"12345").unwrap();
        apply_dirty(
            &root,
            &mut snap,
            &[DirtyEvent {
                rel_path: "a.txt".into(),
                is_dir: false,
            }],
        )
        .unwrap();
        assert_eq!(snap["a.txt"].size, 5);

        std::fs::remove_file(root.join("a.txt")).unwrap();
        apply_dirty(
            &root,
            &mut snap,
            &[DirtyEvent {
                rel_path: "a.txt".into(),
                is_dir: false,
            }],
        )
        .unwrap();
        assert!(!snap.contains_key("a.txt"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dirty_dir_rescans_subtree_and_removal_cleans_children() {
        let root = tmpdir("dir");
        let mut snap = HashMap::new();

        std::fs::create_dir_all(root.join("docs/sub")).unwrap();
        std::fs::write(root.join("docs/a.txt"), b"x").unwrap();
        std::fs::write(root.join("docs/sub/b.txt"), b"yy").unwrap();

        apply_dirty(
            &root,
            &mut snap,
            &[DirtyEvent {
                rel_path: "docs".into(),
                is_dir: true,
            }],
        )
        .unwrap();
        assert!(snap["docs"].is_dir);
        assert!(snap.contains_key("docs/a.txt"));
        assert_eq!(snap["docs/sub/b.txt"].size, 2);

        // 整个目录消失 → 子路径一并清除
        std::fs::remove_dir_all(root.join("docs")).unwrap();
        apply_dirty(
            &root,
            &mut snap,
            &[DirtyEvent {
                rel_path: "docs".into(),
                is_dir: true,
            }],
        )
        .unwrap();
        assert!(snap.is_empty(), "目录删除应清掉全部子路径: {snap:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dir_rename_is_handled_by_subtree_rescan() {
        // 目录改名：事件只报目录本身（旧名/新名各一条），
        // 子文件路径由"旧前缀清除 + 新子树重扫"自动修正。
        let root = tmpdir("rename");
        let mut snap = HashMap::new();

        std::fs::create_dir_all(root.join("old")).unwrap();
        std::fs::write(root.join("old/f.txt"), b"data").unwrap();
        apply_dirty(
            &root,
            &mut snap,
            &[DirtyEvent {
                rel_path: "old".into(),
                is_dir: true,
            }],
        )
        .unwrap();
        assert!(snap.contains_key("old/f.txt"));

        // 改名
        std::fs::rename(root.join("old"), root.join("new")).unwrap();
        apply_dirty(
            &root,
            &mut snap,
            &[
                DirtyEvent {
                    rel_path: "old".into(),
                    is_dir: true,
                },
                DirtyEvent {
                    rel_path: "new".into(),
                    is_dir: true,
                },
            ],
        )
        .unwrap();
        assert!(!snap.contains_key("old/f.txt"), "旧路径应消失");
        assert!(snap.contains_key("new/f.txt"), "新路径应出现");
        assert_eq!(snap["new/f.txt"].size, 4);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rescan_replaces_whole_snapshot() {
        let root = tmpdir("rescan");
        let mut snap = HashMap::new();
        std::fs::write(root.join("only.txt"), b"1").unwrap();
        rescan_into(&root, &mut snap).unwrap();
        assert_eq!(snap.len(), 1);
        assert!(snap.contains_key("only.txt"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn polling_backend_builds() {
        let root = tmpdir("factory");
        let w = build_watcher(&root, WatcherBackend::Polling).unwrap();
        assert!(w.snapshot().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_backend_falls_back_gracefully_on_missing_root() {
        // 根不存在：原生初始化失败 → 回退轮询（轮询对不存在的根返回空快照）
        let root = std::env::temp_dir().join("cm-watcher-no-such-root-xyz");
        let _ = std::fs::remove_dir_all(&root);
        let w = build_watcher(&root, WatcherBackend::Auto).unwrap();
        assert!(w.snapshot().is_empty());
    }
}
