//! 三向对比（three-way merge）：以"上次同步快照"为基准，产出本轮操作集。
//!
//! **这是同步引擎的核心，且刻意做成纯函数**：无 I/O、无状态、无环境依赖，
//! 因此可以在单元测试里穷举所有组合（本地/远端的存在性 × 是否变化 × 策略开关）。
//! 所有需要 I/O 的判断（内容哈希比对）下沉为 [`Action::Verify`] 交给执行器。
//!
//! ## 判定矩阵（`L`=本地有, `R`=远端有, `K`=上次同步记录）
//!
//! | 情形 | 判定 | 说明 |
//! |---|---|---|
//! | L R K，两端都没变 | 无操作 | 幂等 |
//! | L R K，仅本地变 | Upload | 本地为准 |
//! | L R K，仅远端变 | Download | 远端为准 |
//! | L R K，两端都变 | **Conflict** | 默认保留两者 |
//! | L R（无 K） | Verify | 比对内容哈希，相同则入账，不同则冲突 |
//! | L，无 R，无 K | Upload | 本地新建 |
//! | L，无 R，有 K | 远端已删 | 默认**重新上传**（保守）；可配置为删除本地 |
//! | 无 L，R，无 K | Download | 远端新建 |
//! | 无 L，R，有 K | 本地已删 | 默认**重新下载**（保守）；可配置为删除远端 |
//! | 类型不一致（一边文件一边目录） | **Conflict** | 交由策略解决 |

use crate::scan::LocalEntry;
use crate::state::EntryRecord;
use cloudmount_core::{EntryKind, RemoteEntry};
use std::collections::HashMap;

/// 冲突解决策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictPolicy {
    /// 保留两者：本地副本改名后上传为新文件，远端原文件下载回原路径。
    #[default]
    KeepBoth,
    /// 本地优先：覆盖远端（上传本地版本）。
    PreferLocal,
    /// 远端优先：覆盖本地（下载远端版本）。
    PreferRemote,
}

/// 同步策略（安全默认值见各字段说明）。
#[derive(Debug, Clone)]
pub struct SyncPolicy {
    /// 远端删除是否传播到本地。**默认 false**（保留本地文件并重新上传）。
    ///
    /// 数据同步工具的误删不可逆，故默认保守；需要 OneDrive 式镜像行为时打开。
    pub propagate_remote_deletes: bool,
    /// 本地删除是否传播到远端。**默认 false**（保留远端文件并重新下载）。
    pub propagate_local_deletes: bool,
    /// 冲突策略。
    pub conflict: ConflictPolicy,
    /// 冲突副本文件名里的设备标识（多设备区分）。
    pub conflict_tag: String,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self {
            propagate_remote_deletes: false,
            propagate_local_deletes: false,
            conflict: ConflictPolicy::KeepBoth,
            conflict_tag: "device".to_string(),
        }
    }
}

impl SyncPolicy {
    /// 生成冲突副本的相对路径：`dir/name (conflict-<tag>-<yyyymmdd-HHMMSS>).ext`。
    pub fn conflict_path(&self, path: &str, ts: i64) -> String {
        conflict_path(path, &self.conflict_tag, ts)
    }
}

/// 生成冲突副本路径（纯函数，便于测试）。
pub fn conflict_path(path: &str, tag: &str, ts: i64) -> String {
    let (dir, file) = match path.rsplit_once('/') {
        Some((d, f)) => (format!("{d}/"), f.to_string()),
        None => (String::new(), path.to_string()),
    };
    let (stem, ext) = match file.rsplit_once('.') {
        // 隐藏文件（以 . 开头且无其它点）视为无扩展名
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (file, String::new()),
    };
    let stamp = match time::OffsetDateTime::from_unix_timestamp(ts) {
        Ok(dt) => format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            dt.year(),
            dt.month() as u8,
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        ),
        Err(_) => ts.to_string(),
    };
    format!("{dir}{stem} (conflict-{tag}-{stamp}){ext}")
}

/// 本轮的单个操作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 上传本地文件（新增或更新）。
    Upload {
        /// 相对路径。
        path: String,
    },
    /// 下载远端文件到本地路径。
    Download {
        /// 相对路径。
        path: String,
        /// 远端条目（含 id/size/mtime/sha1）。
        remote: RemoteEntry,
    },
    /// 本地缺少目录（远端有），创建之。
    MkdirLocal {
        /// 相对路径。
        path: String,
    },
    /// 远端缺少目录（本地有），创建之（保留空目录语义）。
    MkdirRemote {
        /// 相对路径。
        path: String,
    },
    /// 删除本地文件/目录（远端删除传播）。
    DeleteLocal {
        /// 相对路径。
        path: String,
    },
    /// 删除远端条目（本地删除传播）。
    DeleteRemote {
        /// 相对路径。
        path: String,
        /// 远端 ID。
        remote_id: String,
    },
    /// 两端都有但从未同步过：比对内容哈希后再决定（相同→入账，不同→冲突）。
    Verify {
        /// 相对路径。
        path: String,
        /// 远端条目。
        remote: RemoteEntry,
    },
    /// 冲突，按策略处理。
    Conflict {
        /// 相对路径。
        path: String,
        /// 远端条目。
        remote: RemoteEntry,
        /// 冲突类型（用于日志与统计）。
        kind: ConflictKind,
    },
}

/// 冲突成因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// 两端都修改。
    BothModified,
    /// 两端都新建且内容不同。
    BothCreated,
    /// 类型不一致（文件 vs 目录）。
    TypeMismatch,
}

/// 三向对比的输入。
pub struct PlanInput<'a> {
    /// 远端树（相对路径 → 条目）。
    pub remote: &'a HashMap<String, RemoteEntry>,
    /// 本地扫描（相对路径 → 条目）。
    pub local: &'a HashMap<String, LocalEntry>,
    /// 上次同步快照。
    pub known: &'a HashMap<String, EntryRecord>,
    /// 策略。
    pub policy: &'a SyncPolicy,
}

/// 计算本轮操作集（**确定性输出**：按路径排序）。
pub fn plan(input: &PlanInput<'_>) -> Vec<Action> {
    let mut actions = Vec::new();

    // 收集全部路径（并集），排序保证确定性
    let mut paths: Vec<&String> = input
        .remote
        .keys()
        .chain(input.local.keys())
        .chain(input.known.keys())
        .collect();
    paths.sort_unstable();
    paths.dedup();

    for path in paths {
        let r = input.remote.get(path);
        let l = input.local.get(path);
        let k = input.known.get(path);
        let policy = input.policy;

        // 类型不一致：直接冲突（无论是否有快照）
        if let (Some(re), Some(le)) = (r, l) {
            let r_is_dir = re.kind == EntryKind::Directory;
            if r_is_dir != le.is_dir {
                actions.push(Action::Conflict {
                    path: path.clone(),
                    remote: re.clone(),
                    kind: ConflictKind::TypeMismatch,
                });
                continue;
            }
        }

        match (l, r, k) {
            // ---- 两端都有 ----
            (Some(le), Some(re), Some(rec)) => {
                let local_changed = le.size != rec.size || le.mtime != rec.local_mtime;
                let remote_changed = re.size != rec.size || re.mtime != rec.remote_mtime;
                match (local_changed, remote_changed) {
                    (true, true) => actions.push(Action::Conflict {
                        path: path.clone(),
                        remote: re.clone(),
                        kind: ConflictKind::BothModified,
                    }),
                    (true, false) => {
                        if le.is_dir {
                            // 目录本身无需传输
                        } else {
                            actions.push(Action::Upload { path: path.clone() });
                        }
                    }
                    (false, true) => {
                        if le.is_dir {
                            // 目录：创建/更新时间戳无需处理
                        } else {
                            actions.push(Action::Download {
                                path: path.clone(),
                                remote: re.clone(),
                            });
                        }
                    }
                    (false, false) => {}
                }
            }
            (Some(_), Some(re), None) => {
                // 从未同步过但两端都有：先验证内容
                actions.push(Action::Verify {
                    path: path.clone(),
                    remote: re.clone(),
                });
            }
            // ---- 仅本地 ----
            (Some(_), None, Some(_)) => {
                // 远端曾同步、现已删除
                if policy.propagate_remote_deletes {
                    actions.push(Action::DeleteLocal { path: path.clone() });
                } else {
                    actions.push(Action::Upload { path: path.clone() });
                }
            }
            (Some(le), None, None) => {
                if le.is_dir {
                    // 本地新建目录 → 在远端建出来（保留空目录）
                    actions.push(Action::MkdirRemote { path: path.clone() });
                } else {
                    actions.push(Action::Upload { path: path.clone() });
                }
            }
            // ---- 仅远端 ----
            (None, Some(re), Some(_)) => {
                // 本地曾同步、现已删除
                if policy.propagate_local_deletes {
                    actions.push(Action::DeleteRemote {
                        path: path.clone(),
                        remote_id: re.id.0.clone(),
                    });
                } else {
                    actions.push(Action::Download {
                        path: path.clone(),
                        remote: re.clone(),
                    });
                }
            }
            (None, Some(re), None) => {
                if re.kind == EntryKind::Directory {
                    actions.push(Action::MkdirLocal { path: path.clone() });
                } else {
                    actions.push(Action::Download {
                        path: path.clone(),
                        remote: re.clone(),
                    });
                }
            }
            // ---- 仅快照有（两端都没了）：清账 ----
            (None, None, Some(_)) => {}
            // 并集保证至少一边存在；防御式跳过而非 panic
            (None, None, None) => continue,
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudmount_core::RemoteId;

    fn remote_file(id: &str, path_name: &str, size: u64, mtime: i64) -> RemoteEntry {
        RemoteEntry {
            id: RemoteId(id.into()),
            parent_id: RemoteId("0".into()),
            name: path_name.into(),
            kind: EntryKind::File,
            size,
            mtime,
            sha1: Some("SHA1".into()),
            pick_code: None,
            deleted: false,
        }
    }

    fn remote_dir(id: &str, name: &str) -> RemoteEntry {
        RemoteEntry {
            kind: EntryKind::Directory,
            size: 0,
            ..remote_file(id, name, 0, 0)
        }
    }

    fn local_file(size: u64, mtime: i64) -> LocalEntry {
        LocalEntry {
            is_dir: false,
            size,
            mtime,
        }
    }

    fn local_dir() -> LocalEntry {
        LocalEntry {
            is_dir: true,
            size: 0,
            mtime: 0,
        }
    }

    fn known(path: &str, size: u64, local_mtime: i64, remote_mtime: i64) -> EntryRecord {
        EntryRecord {
            rel_path: path.into(),
            is_dir: false,
            remote_id: Some(RemoteId("rid".into())),
            remote_sha1: Some("SHA1".into()),
            remote_mtime,
            local_mtime,
            size,
            state: crate::state::SyncState::InSync,
            pin: crate::state::PinState::Online,
        }
    }

    fn run(
        remote: Vec<(&str, RemoteEntry)>,
        local: Vec<(&str, LocalEntry)>,
        known: Vec<EntryRecord>,
        policy: SyncPolicy,
    ) -> Vec<Action> {
        let remote: HashMap<String, RemoteEntry> = remote
            .into_iter()
            .map(|(p, e)| (p.to_string(), e))
            .collect();
        let local: HashMap<String, LocalEntry> =
            local.into_iter().map(|(p, e)| (p.to_string(), e)).collect();
        let known: HashMap<String, EntryRecord> =
            known.into_iter().map(|r| (r.rel_path.clone(), r)).collect();
        plan(&PlanInput {
            remote: &remote,
            local: &local,
            known: &known,
            policy: &policy,
        })
    }

    #[test]
    fn new_local_file_uploads() {
        let a = run(
            vec![],
            vec![("a.txt", local_file(5, 100))],
            vec![],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::Upload {
                path: "a.txt".into()
            }]
        );
    }

    #[test]
    fn new_remote_file_downloads() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 5, 100))],
            vec![],
            vec![],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::Download {
                path: "a.txt".into(),
                remote: remote_file("1", "a.txt", 5, 100)
            }]
        );
    }

    #[test]
    fn unchanged_is_idempotent() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 5, 100))],
            vec![("a.txt", local_file(5, 100))],
            vec![known("a.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert!(a.is_empty(), "无变化不应产生操作: {a:?}");
    }

    #[test]
    fn local_modified_uploads() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 5, 100))],
            vec![("a.txt", local_file(9, 200))], // 本地变了
            vec![known("a.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::Upload {
                path: "a.txt".into()
            }]
        );
    }

    #[test]
    fn remote_modified_downloads() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 9, 300))], // 远端变了
            vec![("a.txt", local_file(5, 100))],
            vec![known("a.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::Download {
                path: "a.txt".into(),
                remote: remote_file("1", "a.txt", 9, 300)
            }]
        );
    }

    #[test]
    fn both_modified_conflicts() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 9, 300))],
            vec![("a.txt", local_file(7, 250))],
            vec![known("a.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert_eq!(a.len(), 1);
        assert!(matches!(
            &a[0],
            Action::Conflict {
                path,
                kind: ConflictKind::BothModified,
                ..
            } if path == "a.txt"
        ));
    }

    #[test]
    fn both_exist_without_snapshot_verifies_first() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 5, 100))],
            vec![("a.txt", local_file(5, 100))],
            vec![],
            SyncPolicy::default(),
        );
        assert!(
            matches!(&a[0], Action::Verify { path, .. } if path == "a.txt"),
            "应先用哈希验证而不是直接冲突: {a:?}"
        );
    }

    #[test]
    fn remote_delete_not_propagated_by_default_resurrects() {
        let a = run(
            vec![], // 远端没了
            vec![("a.txt", local_file(5, 100))],
            vec![known("a.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::Upload {
                path: "a.txt".into()
            }]
        );
    }

    #[test]
    fn remote_delete_propagates_when_enabled() {
        let policy = SyncPolicy {
            propagate_remote_deletes: true,
            ..SyncPolicy::default()
        };
        let a = run(
            vec![],
            vec![("a.txt", local_file(5, 100))],
            vec![known("a.txt", 5, 100, 100)],
            policy,
        );
        assert_eq!(
            a,
            vec![Action::DeleteLocal {
                path: "a.txt".into()
            }]
        );
    }

    #[test]
    fn local_delete_not_propagated_by_default_restores() {
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 5, 100))],
            vec![],
            vec![known("a.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert!(matches!(&a[0], Action::Download { path, .. } if path == "a.txt"));
    }

    #[test]
    fn local_delete_propagates_when_enabled() {
        let policy = SyncPolicy {
            propagate_local_deletes: true,
            ..SyncPolicy::default()
        };
        let a = run(
            vec![("a.txt", remote_file("1", "a.txt", 5, 100))],
            vec![],
            vec![known("a.txt", 5, 100, 100)],
            policy,
        );
        assert_eq!(
            a,
            vec![Action::DeleteRemote {
                path: "a.txt".into(),
                remote_id: "1".into()
            }]
        );
    }

    #[test]
    fn nested_dirs_create_on_missing_side() {
        // 本地有目录、远端没有 → 创建远端目录（由执行器解析父链）
        let a = run(
            vec![],
            vec![("docs", local_dir())],
            vec![],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::MkdirRemote {
                path: "docs".into()
            }]
        );

        // 远端有目录、本地没有 → 创建本地目录
        let a = run(
            vec![("docs", remote_dir("d1", "docs"))],
            vec![],
            vec![],
            SyncPolicy::default(),
        );
        assert_eq!(
            a,
            vec![Action::MkdirLocal {
                path: "docs".into()
            }]
        );
    }

    #[test]
    fn type_mismatch_conflicts() {
        let a = run(
            vec![("x", remote_dir("d1", "x"))],
            vec![("x", local_file(10, 50))],
            vec![known("x", 10, 50, 50)],
            SyncPolicy::default(),
        );
        assert!(matches!(
            &a[0],
            Action::Conflict {
                kind: ConflictKind::TypeMismatch,
                ..
            }
        ));
    }

    #[test]
    fn vanished_both_sides_yields_nothing() {
        let a = run(
            vec![],
            vec![],
            vec![known("gone.txt", 5, 100, 100)],
            SyncPolicy::default(),
        );
        assert!(a.is_empty(), "两端都无的旧记录应静默清账: {a:?}");
    }

    #[test]
    fn output_is_sorted_deterministically() {
        let a = run(
            vec![
                ("z.txt", remote_file("3", "z.txt", 1, 1)),
                ("a.txt", remote_file("1", "a.txt", 1, 1)),
            ],
            vec![("m.txt", local_file(1, 1))],
            vec![],
            SyncPolicy::default(),
        );
        let paths: Vec<String> = a
            .iter()
            .map(|act| match act {
                Action::Upload { path } => path.clone(),
                Action::Download { path, .. } => path.clone(),
                Action::Verify { path, .. } => path.clone(),
                Action::Conflict { path, .. } => path.clone(),
                Action::MkdirLocal { path } => path.clone(),
                Action::MkdirRemote { path } => path.clone(),
                Action::DeleteLocal { path } => path.clone(),
                Action::DeleteRemote { path, .. } => path.clone(),
            })
            .collect();
        assert_eq!(paths, vec!["a.txt", "m.txt", "z.txt"]);
    }

    #[test]
    fn conflict_path_shapes() {
        assert_eq!(
            conflict_path("a.txt", "pc1", 0),
            "a (conflict-pc1-19700101-000000).txt"
        );
        assert_eq!(
            conflict_path("docs/a.tar.gz", "pc1", 0),
            "docs/a.tar (conflict-pc1-19700101-000000).gz"
        );
        assert_eq!(
            conflict_path("README", "pc1", 0),
            "README (conflict-pc1-19700101-000000)"
        );
        assert_eq!(
            conflict_path(".gitignore", "pc1", 0),
            ".gitignore (conflict-pc1-19700101-000000)",
            "隐藏文件不应被当成扩展名切分"
        );
    }
}
