//! 同步排除规则：应用"内部多文件一致性"目录与系统垃圾文件。
//!
//! ## 为什么 `.git` 必须默认排除（OneDrive 的结构性翻车）
//!
//! Git 的写入模式对文件同步引擎有四重杀伤：
//! 1. **内部一致性**：`index` 引用 objects 的 SHA、objects 引用 refs——必须
//!    **作为整体**到达另一台机器。逐文件同步 → 对端看到"半同步"的 `.git`
//!    → `index file corrupt` / loose object 损坏。
//! 2. **原子改名被拆散**：git 大量使用"写临时 → rename 到最终名"
//!    （`tmp_pack_XXX → pack-XXX.pack`）。同步引擎把 rename 拆成"删+建"且
//!    两步不同时到达 → 对端两个都存在或都不存在。
//! 3. **锁文件被同步**：`.git/index.lock` 传到云端再传回 → 假冲突。
//! 4. **冲突副本写进 `.git`**：git 看到未知文件 → 仓库损坏。**在 `.git` 内
//!    创建冲突副本是绝对禁止的**，通用同步引擎却不知道。
//!
//! **架构结论**：git 自带同步协议（push/pull）——文件同步层同步 `.git` 是
//! 两个同步系统抢同一份数据。因此默认排除，`.git` 纯本地，worktree 正常同步。
//!
//! ## 过滤点（四处，缺一不可）
//!
//! 1. [`crate::scan`]：本地扫描跳过（快照/基线不含排除项）
//! 2. RDC / inotify 事件：排除路径的事件不进入变更集（防误上传）
//! 3. [`crate::remote`]：远端树拉取跳过（远端已有 `.git` 也不拉取——本地权威）
//! 4. 投影（PlaceholderSpec）：远端视图过滤后自然不投影
//!
//! ## 强制同步排除项的规则（设计存档，M2.5）
//!
//! 高级用户关闭排除时：`.git` 内冲突**永不创建副本**（last-writer-wins+告警）；
//! 静默期门控（存在 `*.lock` 或近 N 秒有写入 → 推迟整轮 `.git` 上传）。

/// 默认排除的**目录名**（任意层级，精确匹配）。
pub const EXCLUDED_DIRS: &[&str] = &[
    // VCS 内部目录（见模块文档：同步它们 = 仓库损坏）
    ".git", ".hg", ".svn", // NAS 系统目录
    "@eaDir", "#recycle",
];

/// 默认排除的**文件名**（任意层级，精确匹配）。
pub const EXCLUDED_FILES: &[&str] = &[
    // OS 元数据垃圾
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
];

/// 默认排除的**文件名模式**（前缀/后缀）。
const EXCLUDED_PREFIXES: &[&str] = &[
    "~$",      // Office 临时文件（~$xxx.docx）
    ".~lock#", // LibreOffice 锁
];
const EXCLUDED_SUFFIXES: &[&str] = &[".tmp", ".swp", ".swo"];

/// 名称是否被默认排除（目录或文件通用）。
pub fn is_excluded_name(name: &str) -> bool {
    if EXCLUDED_DIRS.contains(&name) || EXCLUDED_FILES.contains(&name) {
        return true;
    }
    EXCLUDED_PREFIXES.iter().any(|p| name.starts_with(p))
        || EXCLUDED_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// 相对路径（`/` 分隔）是否被默认排除（任一层级命中即排除）。
pub fn is_excluded_path(rel: &str) -> bool {
    rel.split('/').any(is_excluded_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vcs_dirs_excluded() {
        for d in [".git", ".hg", ".svn"] {
            assert!(is_excluded_name(d), "{d} 应被排除");
        }
    }

    #[test]
    fn path_any_level() {
        assert!(is_excluded_path(".git"));
        assert!(is_excluded_path("project/.git"));
        assert!(is_excluded_path("a/b/.git/objects"));
        assert!(!is_excluded_path("gitignore.txt"), "名字含 git 但不是 .git");
        assert!(!is_excluded_path("docs/README.md"));
    }

    #[test]
    fn junk_files() {
        assert!(is_excluded_name(".DS_Store"));
        assert!(is_excluded_name("Thumbs.db"));
        assert!(is_excluded_name("~$report.docx"));
        assert!(is_excluded_name(".~lock#report.docx#"));
        assert!(is_excluded_name("a.tmp"));
        assert!(is_excluded_name(".swp"));
        assert!(!is_excluded_name("important.tmp.txt"), "后缀须在末尾");
    }

    #[test]
    fn normal_files_pass() {
        for n in ["main.rs", "report.pdf", ".gitignore", ".env", "index.lock"] {
            assert!(!is_excluded_name(n), "{n} 不应被排除");
        }
    }
}
