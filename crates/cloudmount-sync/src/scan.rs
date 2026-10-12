//! 本地变更发现：目录扫描 + 变更监控抽象。
//!
//! M1 提供**可移植的轮询扫描**（`PollingWatcher`）作为基线实现；
//! 平台高效实现通过 [`LocalWatcher`] 后续接入：
//! - Windows：USN Journal（增量、无需遍历）
//! - macOS：FSEvents
//! - Linux：inotify
//!
//! 统一约束：路径一律用 `/` 分隔的相对路径（与远端寻址一致）。

use cloudmount_core::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 扫描到的本地条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEntry {
    /// 是否目录。
    pub is_dir: bool,
    /// 字节大小（目录为 0）。
    pub size: u64,
    /// 修改时间（Unix 秒）。
    pub mtime: i64,
}

/// 本地变更类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalChange {
    /// 新增。
    Created { path: String, entry: LocalEntry },
    /// 内容或时间戳变化。
    Modified { path: String, entry: LocalEntry },
    /// 删除。
    Removed { path: String, was_dir: bool },
}

/// 本地变更监控抽象。
///
/// 实现可以是轮询（[`PollingWatcher`]）或平台原生（USN/inotify/FSEvents）。
pub trait LocalWatcher: Send {
    /// 拉取自上次调用以来的变更集。
    fn poll(&mut self) -> Result<Vec<LocalChange>>;

    /// 当前全量视图（用于三向对比）。
    fn snapshot(&self) -> &HashMap<String, LocalEntry>;

    /// 根目录。
    fn root(&self) -> &Path;
}

/// 递归扫描目录树（不跟随符号链接）。
///
/// 跳过 `.cloudmount` 内部目录与临时文件（`*.cmtmp`），避免把引擎自身的
/// 中间产物当成用户数据同步出去。
pub fn scan_tree(root: &Path) -> Result<HashMap<String, LocalEntry>> {
    scan_subtree(root, "")
}

/// 扫描 `root` 下某个子目录（`rel_prefix` 为相对根的路径前缀），
/// 返回以**完整相对路径**为键的条目。
///
/// 原生 watcher 收到"某子目录有变化"的事件时，只需重扫该子树而不是整棵树。
pub fn scan_subtree(root: &Path, rel_prefix: &str) -> Result<HashMap<String, LocalEntry>> {
    let mut out = HashMap::new();
    let abs = if rel_prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel_prefix.replace('/', std::path::MAIN_SEPARATOR_STR))
    };
    if !abs.exists() {
        return Ok(out);
    }
    let mut stack: Vec<PathBuf> = vec![abs];
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            // 目录在扫描过程中消失（并发删除）不是错误
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        for ent in rd {
            let ent = match ent {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = ent.path();
            let name = ent.file_name().to_string_lossy().to_string();
            if is_internal(&name) || crate::exclude::is_excluded_name(&name) {
                continue;
            }
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue, // 竞态删除
            };
            if meta.file_type().is_symlink() {
                // 不同步符号链接（避免目录环与跨设备语义差异）
                continue;
            }
            let dir_rel = rel_path(root, &path)?;
            let is_dir = meta.is_dir();
            out.insert(
                dir_rel.clone(),
                LocalEntry {
                    is_dir,
                    size: if is_dir { 0 } else { meta.len() },
                    mtime: mtime_secs(&meta),
                },
            );
            if is_dir {
                stack.push(path);
            }
        }
    }
    Ok(out)
}

/// 是否引擎内部文件/目录。
pub fn is_internal(name: &str) -> bool {
    name == ".cloudmount" || name.ends_with(".cmtmp")
}

/// 计算相对路径（统一 `/` 分隔）。
pub fn rel_path(root: &Path, path: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| cloudmount_core::Error::Invalid(format!("{path:?} 不在根 {root:?} 下")))?;
    Ok(rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

/// 取 mtime（Unix 秒）。
pub fn mtime_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 轮询式监控（可移植基线实现）。
///
/// 每次 `poll()` 重新扫描并与上次快照对比。大目录下成本与文件数成正比，
/// 这是官方 API 无事件接口时的必然取舍；平台原生实现（USN/inotify）后续替换。
pub struct PollingWatcher {
    root: PathBuf,
    snapshot: HashMap<String, LocalEntry>,
}

impl PollingWatcher {
    /// 创建并做首次扫描（首次 `poll()` 返回空变更集，因为还没基准）。
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let snapshot = scan_tree(&root)?;
        Ok(Self { root, snapshot })
    }
}

impl LocalWatcher for PollingWatcher {
    fn poll(&mut self) -> Result<Vec<LocalChange>> {
        let current = scan_tree(&self.root)?;
        let changes = crate::watcher::diff_snapshots(&self.snapshot, &current);
        self.snapshot = current;
        Ok(changes)
    }

    fn snapshot(&self) -> &HashMap<String, LocalEntry> {
        &self.snapshot
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cm-scan-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn scans_nested_tree_with_slash_paths() {
        let root = tmpdir("nested");
        std::fs::create_dir_all(root.join("docs/sub")).unwrap();
        std::fs::write(root.join("docs/a.txt"), b"hello").unwrap();
        std::fs::write(root.join("docs/sub/b.txt"), b"world!").unwrap();

        let tree = scan_tree(&root).unwrap();
        assert!(tree["docs"].is_dir);
        assert!(tree["docs/sub"].is_dir);
        assert_eq!(tree["docs/a.txt"].size, 5);
        assert_eq!(tree["docs/sub/b.txt"].size, 6);
        assert!(!tree["docs/a.txt"].is_dir);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_internal_and_temp_files() {
        let root = tmpdir("internal");
        std::fs::create_dir_all(root.join(".cloudmount")).unwrap();
        std::fs::write(root.join(".cloudmount/state.db"), b"x").unwrap();
        std::fs::write(root.join("keep.txt"), b"k").unwrap();
        std::fs::write(root.join("half.cmtmp"), b"t").unwrap();

        let tree = scan_tree(&root).unwrap();
        assert!(tree.contains_key("keep.txt"));
        assert!(!tree.keys().any(|k| k.starts_with(".cloudmount")));
        assert!(!tree.keys().any(|k| k.ends_with(".cmtmp")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn watcher_reports_create_modify_remove() {
        let root = tmpdir("watch");
        std::fs::write(root.join("a.txt"), b"1").unwrap();
        let mut w = PollingWatcher::new(&root).unwrap();
        assert!(w.poll().unwrap().is_empty(), "首次 poll 无基准，应无变更");

        // 注意：这里改变**大小**（1→3 字节）而不是同大小覆盖——
        // 见 `same_second_same_size_edit_is_not_detected` 记录的盲区。
        std::fs::write(root.join("a.txt"), b"333").unwrap();
        std::fs::write(root.join("b.txt"), b"new").unwrap();
        std::fs::remove_file(root.join("b.txt")).unwrap();

        let changes = w.poll().unwrap();
        assert_eq!(changes.len(), 1, "a.txt 被修改；b.txt 创建后又删除应不算");
        assert!(matches!(&changes[0], LocalChange::Modified { path, .. } if path == "a.txt"));

        std::fs::remove_file(root.join("a.txt")).unwrap();
        let changes = w.poll().unwrap();
        assert!(matches!(&changes[0], LocalChange::Removed { path, .. } if path == "a.txt"));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// **已知盲区**（诚实的边界记录）：mtime 精度为秒，(大小, mtime) 判定的变更检测
    /// 无法识别"同一秒内、等大小"的内容修改。
    ///
    /// 影响：极小概率漏检一次修改（下次该文件因其它原因变化时仍会被同步）。
    /// 缓解：M1.5 计划引入可选的"慢速内容校验"通道（抽样哈希）；
    /// 或让平台 watcher（USN/FSEvents/inotify）提供精确事件。
    #[test]
    fn same_second_same_size_edit_is_not_detected() {
        let root = tmpdir("blindspot");
        std::fs::write(root.join("x.txt"), b"A").unwrap();
        let mut w = PollingWatcher::new(&root).unwrap();
        w.poll().unwrap();

        // 同秒内等大小覆盖（1 字节 → 1 字节）
        std::fs::write(root.join("x.txt"), b"B").unwrap();
        let changes = w.poll().unwrap();
        assert!(
            changes.is_empty(),
            "该盲区是已知设计取舍；若此处失败说明检测能力被改动，请同步更新文档"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_root_scans_empty() {
        let root = std::env::temp_dir().join("cm-scan-does-not-exist-xyz");
        let _ = std::fs::remove_dir_all(&root);
        assert!(scan_tree(&root).unwrap().is_empty());
    }
}

#[cfg(test)]
mod git_exclusion_tests {
    use super::*;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cm-scan-{tag}-{}-{:?}",
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
    fn git_dir_is_invisible_to_scan() {
        let root = tmpdir("git-scan");
        std::fs::create_dir_all(root.join("project/.git/objects")).unwrap();
        std::fs::write(root.join("project/.git/index"), b"git index").unwrap();
        std::fs::write(root.join("project/.git/index.lock"), b"lock").unwrap();
        std::fs::write(root.join("project/main.rs"), b"fn main() {}").unwrap();

        let tree = scan_tree(&root).unwrap();
        assert!(tree.contains_key("project"), "普通目录应可见");
        assert!(tree.contains_key("project/main.rs"), "worktree 文件应可见");
        assert!(
            !tree.keys().any(|k| k.contains(".git")),
            ".git 全树应不可见: {:?}",
            tree.keys().collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn junk_files_are_invisible() {
        let root = tmpdir("junk-scan");
        std::fs::write(root.join(".DS_Store"), b"junk").unwrap();
        std::fs::write(root.join("~$doc.docx"), b"office temp").unwrap();
        std::fs::write(root.join("real.txt"), b"real").unwrap();
        let tree = scan_tree(&root).unwrap();
        assert!(tree.contains_key("real.txt"));
        assert_eq!(tree.len(), 1, "垃圾文件应全部过滤: {tree:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
