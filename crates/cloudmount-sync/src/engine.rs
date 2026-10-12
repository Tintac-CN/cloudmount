//! 同步引擎：把变更发现、三向对比、执行、持久化串成一个周期。
//!
//! ## 每轮流程
//!
//! ```text
//! 1. 本地扫描（PollingWatcher，后续替换为 USN/inotify）
//! 2. 远端廉价探测 change_probe（1 次请求：条目数 + 最新 mtime）
//!     ├─ 未变化 → 【用快照合成远端视图】← 零远端请求（关键优化）
//!     └─ 变化/不支持 → 全量拉取远端树
//! 3. 三向对比（reconcile::plan）
//! 4. 执行（Executor + 队列：重试/退避/暂停/并发）
//! 5. 持久化（记录 upsert、删除项 remove、水位 meta）+ 缓存索引落盘
//! ```
//!
//! ## 短路条件（全部满足才跳过本轮 I/O）
//!
//! - 远端探测与水位一致；
//! - 本地扫描与快照一致（无新增/修改/删除）；
//! - 快照中没有遗留的 `Pending/Uploading/Downloading/Conflict` 状态；
//! - 未到强制全量扫描周期。
//!
//! ## 已知边界
//!
//! 根级探测只反映根目录直接子项的变化。**依赖后端在子项变更时更新父目录时间戳**
//! （115 如此：目录 `uet` 随子项变化）；其它后端需验证，或把
//! [`SyncEngineConfig::full_scan_every_cycles`] 调小作为安全网。

use crate::cache::{BlockCache, CacheStats};
use crate::exec::Executor;
use crate::queue::{run_jobs, SchedulerConfig};
use crate::reconcile::{plan, Action, PlanInput, SyncPolicy};
use crate::remote::{self, RemoteTree};
use crate::scan::{scan_tree, LocalWatcher};
use crate::state::{EntryRecord, SyncState, SyncStats};
use crate::store::{MetaStore, META_LAST_SCAN, META_PROBE_ROOT};
use crate::watcher::{build_watcher, WatcherBackend};
use cloudmount_core::{ChangeProbe, EntryKind, Error, Provider, RemoteEntry, RemoteId, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// 引擎配置。
#[derive(Debug, Clone)]
pub struct SyncEngineConfig {
    /// 本地同步根目录。
    pub local_root: PathBuf,
    /// 远端根目录 ID（115 根为 "0"）。
    pub remote_root: RemoteId,
    /// SQLite 数据库路径。
    pub db_path: PathBuf,
    /// 块缓存目录。
    pub cache_dir: PathBuf,
    /// 缓存字节上限。
    pub cache_limit_bytes: u64,
    /// 同步策略（删除传播/冲突策略等）。
    pub policy: SyncPolicy,
    /// 调度配置（并发/重试/暂停）。
    pub scheduler: SchedulerConfig,
    /// 每 N 轮强制一次全量远端扫描（0 = 不强制）。安全网，默认 20。
    pub full_scan_every_cycles: u32,
    /// 距上次全量扫描超过该秒数则强制全量（0 = 不按时间强制）。默认 1 小时。
    pub full_scan_max_age_secs: i64,
    /// 本地 watcher 后端（Auto = 平台原生优先，失败回退轮询）。
    pub watcher_backend: WatcherBackend,
    /// 慢速内容校验：每 N 轮执行一次（0 = 关闭）。默认 5。
    ///
    /// 这是 (size, mtime) 秒级粒度检测的兜底：抽样比对本地内容哈希与远端 SHA1，
    /// 关闭"同秒内等大小修改"的漏检盲区。
    pub verify_every_cycles: u32,
    /// 每轮校验的文件数（轮转选择，0 = 关闭）。默认 32。
    pub verify_batch_size: usize,
}

impl SyncEngineConfig {
    /// 基于本地根目录给出合理默认值。
    pub fn new(local_root: impl Into<PathBuf>) -> Self {
        let local_root = local_root.into();
        let internal = local_root.join(".cloudmount");
        Self {
            local_root,
            remote_root: RemoteId("0".into()),
            db_path: internal.join("sync.db"),
            cache_dir: internal.join("cache"),
            cache_limit_bytes: 512 * 1024 * 1024,
            policy: SyncPolicy::default(),
            scheduler: SchedulerConfig::default(),
            full_scan_every_cycles: 20,
            full_scan_max_age_secs: 3600,
            watcher_backend: WatcherBackend::Auto,
            verify_every_cycles: 5,
            verify_batch_size: 32,
        }
    }
}

/// 同步引擎。
pub struct SyncEngine {
    cfg: SyncEngineConfig,
    provider: Arc<dyn Provider>,
    store: MetaStore,
    cache: BlockCache,
    watcher: Box<dyn LocalWatcher>,
    /// 距上次全量远端扫描的轮数。
    cycles_since_full: u32,
    /// 总轮数（内容校验的调度依据）。
    cycles: u32,
    last_stats: SyncStats,
}

impl SyncEngine {
    /// 创建引擎（会建立本地内部目录、打开数据库与缓存）。
    pub fn new(cfg: SyncEngineConfig, provider: Arc<dyn Provider>) -> Result<Self> {
        std::fs::create_dir_all(&cfg.local_root)?;
        std::fs::create_dir_all(cfg.local_root.join(".cloudmount"))?;
        let store = MetaStore::open(&cfg.db_path)?;
        let cache = BlockCache::open(&cfg.cache_dir, cfg.cache_limit_bytes)?;
        let watcher = build_watcher(&cfg.local_root, cfg.watcher_backend)?;
        Ok(Self {
            cfg,
            provider,
            store,
            cache,
            watcher,
            cycles_since_full: u32::MAX / 2, // 首轮强制全量，建立基线
            cycles: 0,
            last_stats: SyncStats::default(),
        })
    }

    /// 最近一轮统计。
    pub fn last_stats(&self) -> &SyncStats {
        &self.last_stats
    }

    /// 缓存统计。
    pub fn cache_stats(&self) -> Result<CacheStats> {
        self.cache.stats()
    }

    /// 清空快照（下次全量重同步）。
    pub fn reset_snapshot(&self) -> Result<()> {
        self.store.clear()?;
        self.store.set_meta(META_PROBE_ROOT, "")?;
        Ok(())
    }

    /// 强制下一次执行全量扫描。
    pub fn request_full_scan(&mut self) {
        self.cycles_since_full = u32::MAX / 2;
    }

    /// 执行一轮同步。
    pub fn run_once(&mut self) -> Result<SyncStats> {
        let mut stats = SyncStats::default();
        self.cycles += 1;

        // 1. 本地扫描
        self.watcher.poll()?;
        let local = self.watcher.snapshot().clone();
        stats.scanned_local = local.len() as u64;

        // 2. 快照
        let known = self.store.load_all()?;

        // 3. 判定是否需要远端网络 I/O
        let probe = remote::probe(self.provider.as_ref(), &self.cfg.remote_root)?;
        let stored_probe = self
            .store
            .get_meta(META_PROBE_ROOT)?
            .filter(|s| !s.is_empty())
            .and_then(|s| remote::decode_probe(&s));
        let remote_unchanged = match (probe, stored_probe) {
            (Some(p), Some(old)) => p == old,
            _ => false,
        };
        // 3.5 慢速内容校验（短路判定之前：发现不一致即视为本地有变化）
        let (verify_mismatches, verified_records) = if self.verify_due() {
            self.verify_batch(&local, &known)
        } else {
            (Vec::new(), Vec::new())
        };
        let local_changed =
            local_differs_from_snapshot(&local, &known) || !verify_mismatches.is_empty();
        let has_pending = known.values().any(|r| {
            matches!(
                r.state,
                SyncState::Pending | SyncState::Uploading | SyncState::Downloading
            )
        });
        let full_scan_due = self.full_scan_due();

        // 4. 短路：远端未变 + 本地未变 + 无遗留任务 + 未到强制全量
        if remote_unchanged && !local_changed && !has_pending && !full_scan_due {
            // 校验通过的记录仍需落账（推进 content_check 轮转水位）
            if !verified_records.is_empty() {
                self.store.upsert_entries(&verified_records)?;
            }
            stats.remote_unchanged = true;
            self.last_stats = stats;
            tracing::debug!("远端与本地均无变化，跳过本轮");
            return Ok(self.last_stats.clone());
        }

        // 5. 远端视图：探测未变则用快照合成（零请求），否则全量拉取
        let (remote_map, dir_ids, probed_ok) = if remote_unchanged && !full_scan_due {
            let map = synthesize_remote_from_snapshot(&known);
            let dir_ids = collect_dir_ids(&map, &self.cfg.remote_root);
            tracing::debug!(entries = map.len(), "远端未变：用快照合成远端视图");
            (map, dir_ids, true)
        } else {
            let tree: RemoteTree =
                remote::fetch_tree(self.provider.as_ref(), &self.cfg.remote_root)?;
            stats.scanned_remote = tree.entries.len() as u64;
            let dir_ids = collect_dir_ids(&tree.entries, &self.cfg.remote_root);
            (tree.entries, dir_ids, false)
        };

        // 6. 三向对比
        let mut actions = plan(&PlanInput {
            remote: &remote_map,
            local: &local,
            known: &known,
            policy: &self.cfg.policy,
        });
        // 注入内容校验发现的不一致（plan 只看 size/mtime，同秒同大小修改会漏检）
        for path in &verify_mismatches {
            let already = actions.iter().any(|a| match a {
                Action::Upload { path: p } => p == path,
                Action::Conflict { path: p, .. } => p == path,
                _ => false,
            });
            if !already {
                tracing::warn!(path = %path, "内容校验发现不一致（同秒等大小修改），补入上传");
                actions.push(Action::Upload { path: path.clone() });
            }
        }
        tracing::debug!(count = actions.len(), "本轮计划操作数");

        // 7. 执行
        let executor = Executor::new(
            self.provider.as_ref(),
            &self.cfg.local_root,
            &self.cfg.policy,
            &self.cache,
            dir_ids,
        );
        // 计划动作数（用于水位可信性判定：暂停/失败时不得推进水位）
        let planned = actions.len();
        let outcomes = run_jobs(
            actions,
            |a| executor.execute(a),
            &self.cfg.scheduler,
            action_label,
        );
        let outcome = executor.finish();

        // 8. 汇总
        stats.uploaded = outcome.stats.uploaded;
        stats.downloaded = outcome.stats.downloaded;
        stats.conflicts = outcome.stats.conflicts;
        stats.deleted_local = outcome.stats.deleted_local;
        stats.deleted_remote = outcome.stats.deleted_remote;
        stats.dirs_created = outcome.stats.dirs_created;
        stats.errors = outcomes.iter().filter(|o| !o.ok).count() as u64;

        // 9. 注意：暂停或任务失败时 outcomes 会少于 planned，这是预期行为
        //    （未完成的工作由下一轮重新计划）。

        // 10. 持久化
        let mut records_to_write = outcome.records.clone();
        for verified in &verified_records {
            // 执行器本轮已重写该路径的记录时以执行器为准
            if !records_to_write
                .iter()
                .any(|r| r.rel_path == verified.rel_path)
            {
                records_to_write.push(verified.clone());
            }
        }
        if !records_to_write.is_empty() {
            self.store.upsert_entries(&records_to_write)?;
        }
        for path in &outcome.removed {
            remove_with_children(&self.store, path)?;
        }
        // **水位可信性规则**：只有在"本轮没有任何计划动作、也没有任何变更发生"
        // 时才推进水位。否则（有变更、被暂停、或任务失败）一律清空水位，强制
        // 下一轮做全量拉取并重新建立水位。
        //
        // 这条规则防的是两个真实故障：
        //   ① 用陈旧水位误判"远端未变"；
        //   ② 暂停期间远端发生变更却推进了水位 ⇒ 恢复后该变更被短路永久跳过。
        match probe {
            Some(p) if planned == 0 && !stats.has_changes() => self
                .store
                .set_meta(META_PROBE_ROOT, &remote::encode_probe(p))?,
            _ => self.store.set_meta(META_PROBE_ROOT, "")?,
        }
        self.store
            .set_meta(META_LAST_SCAN, &crate::store::now_unix().to_string())?;
        self.cache.flush()?;

        // 11. 轮次计数
        self.cycles_since_full = if probed_ok {
            self.cycles_since_full.saturating_add(1)
        } else {
            0
        };

        self.last_stats = stats;
        Ok(self.last_stats.clone())
    }

    /// 是否该做全量远端扫描。
    fn full_scan_due(&self) -> bool {
        if self.cfg.full_scan_every_cycles > 0
            && self.cycles_since_full >= self.cfg.full_scan_every_cycles
        {
            return true;
        }
        if self.cfg.full_scan_max_age_secs > 0 {
            if let Ok(Some(s)) = self.store.get_meta(META_LAST_SCAN) {
                if let Ok(ts) = s.parse::<i64>() {
                    let age = crate::store::now_unix() - ts;
                    // 只有曾经扫描过（ts>0）才按时间强制
                    if ts > 0 && age > self.cfg.full_scan_max_age_secs {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 是否到了内容校验轮。
    fn verify_due(&self) -> bool {
        self.cfg.verify_batch_size > 0
            && self.cfg.verify_every_cycles > 0
            && self.cycles.is_multiple_of(self.cfg.verify_every_cycles)
    }

    /// 慢速内容校验批次：按 content_check 升序取 N 个文件，
    /// 哈希本地内容并与快照中的远端 SHA1 比对。
    ///
    /// 返回 (不一致路径列表, 校验通过的记录（content_check 已推进)）。
    fn verify_batch(
        &self,
        local: &HashMap<String, crate::scan::LocalEntry>,
        known: &HashMap<String, EntryRecord>,
    ) -> (Vec<String>, Vec<EntryRecord>) {
        let now = crate::store::now_unix();
        let mut candidates: Vec<&EntryRecord> = known
            .values()
            .filter(|r| !r.is_dir && r.state == SyncState::InSync)
            .filter(|r| r.remote_sha1.is_some())
            .filter(|r| local.contains_key(&r.rel_path))
            .collect();
        candidates.sort_by_key(|r| r.content_check);

        let mut mismatches = Vec::new();
        let mut verified = Vec::new();
        for rec in candidates.into_iter().take(self.cfg.verify_batch_size) {
            let abs = self
                .cfg
                .local_root
                .join(rec.rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            let sha = match crate::exec::hash_file_pub(&abs) {
                Ok(sha) => sha,
                Err(_) => continue, // 读取失败（权限/竞态）：跳过，下轮再试
            };
            let remote_sha = rec.remote_sha1.clone().unwrap_or_default();
            if sha.eq_ignore_ascii_case(&remote_sha) {
                let mut updated = rec.clone();
                updated.content_check = now;
                verified.push(updated);
            } else {
                tracing::warn!(
                    path = %rec.rel_path,
                    "内容校验不一致：本地哈希与远端 SHA1 不匹配"
                );
                mismatches.push(rec.rel_path.clone());
            }
        }
        (mismatches, verified)
    }

    /// 一次性扫描（不含网络的最小本地视图，供 UI/诊断）。
    pub fn scan_local(&self) -> Result<HashMap<String, crate::scan::LocalEntry>> {
        scan_tree(&self.cfg.local_root)
    }
}

/// 本地扫描是否与快照不一致（新增/修改/删除）。
fn local_differs_from_snapshot(
    local: &HashMap<String, crate::scan::LocalEntry>,
    known: &HashMap<String, EntryRecord>,
) -> bool {
    for (path, entry) in local {
        match known.get(path) {
            None => return true,
            Some(rec) => {
                if entry.is_dir != rec.is_dir
                    || entry.size != rec.size
                    || entry.mtime != rec.local_mtime
                {
                    return true;
                }
            }
        }
    }
    for path in known.keys() {
        if !local.contains_key(path) {
            return true;
        }
    }
    false
}

/// 用快照合成远端视图（远端探测未变时使用，零请求）。
fn synthesize_remote_from_snapshot(
    known: &HashMap<String, EntryRecord>,
) -> HashMap<String, RemoteEntry> {
    known
        .iter()
        .filter_map(|(path, rec)| {
            let id = rec.remote_id.clone()?;
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            Some((
                path.clone(),
                RemoteEntry {
                    id,
                    parent_id: RemoteId("0".into()),
                    name,
                    kind: if rec.is_dir {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    },
                    size: rec.size,
                    mtime: rec.remote_mtime,
                    sha1: rec.remote_sha1.clone(),
                    pick_code: None,
                    deleted: false,
                },
            ))
        })
        .collect()
}

/// 收集"目录路径 → 远端 ID"，并注入根目录。
fn collect_dir_ids(
    entries: &HashMap<String, RemoteEntry>,
    root: &RemoteId,
) -> HashMap<String, RemoteId> {
    let mut map = HashMap::new();
    map.insert(String::new(), root.clone());
    for (path, e) in entries {
        if e.kind == EntryKind::Directory {
            map.insert(path.clone(), e.id.clone());
        }
    }
    map
}

/// 删除记录时连同其子路径一起清账。
fn remove_with_children(store: &MetaStore, path: &str) -> Result<()> {
    store.remove_entry(path)?;
    let prefix = format!("{path}/");
    let all = store.load_all()?;
    for key in all.keys() {
        if key.starts_with(&prefix) {
            store.remove_entry(key)?;
        }
    }
    Ok(())
}

/// 动作的展示标签（用于队列统计与日志）。
fn action_label(action: &Action) -> String {
    match action {
        Action::Upload { path } => format!("upload:{path}"),
        Action::Download { path, .. } => format!("download:{path}"),
        Action::MkdirLocal { path } => format!("mkdir-local:{path}"),
        Action::MkdirRemote { path } => format!("mkdir-remote:{path}"),
        Action::DeleteLocal { path } => format!("delete-local:{path}"),
        Action::DeleteRemote { path, .. } => format!("delete-remote:{path}"),
        Action::Verify { path, .. } => format!("verify:{path}"),
        Action::Conflict { path, .. } => format!("conflict:{path}"),
        Action::MoveRemote { from, to, .. } => format!("move-remote:{from}->{to}"),
        Action::RenameLocal { from, to, .. } => format!("rename-local:{from}->{to}"),
        Action::Nothing => String::new(),
    }
}

/// 便利：把探测值写入水位（测试与诊断用）。
pub fn probe_watermark(p: ChangeProbe) -> String {
    remote::encode_probe(p)
}

/// 便利：判断某错误是否值得重试（透出给调用方）。
pub fn is_retryable(e: &Error) -> bool {
    e.is_retryable()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::LocalEntry;

    fn rec(
        path: &str,
        size: u64,
        local_mtime: i64,
        remote_mtime: i64,
        is_dir: bool,
    ) -> EntryRecord {
        EntryRecord {
            rel_path: path.into(),
            is_dir,
            remote_id: Some(RemoteId(format!("id:{path}"))),
            remote_sha1: Some("SHA".into()),
            remote_mtime,
            local_mtime,
            size,
            state: SyncState::InSync,
            content_check: 0,
            pin: crate::state::PinState::Online,
        }
    }

    #[test]
    fn local_diff_detects_new_modified_removed() {
        let known: HashMap<String, EntryRecord> = [("a".to_string(), rec("a", 1, 10, 20, false))]
            .into_iter()
            .collect();

        // 一致
        let local: HashMap<String, LocalEntry> = [(
            "a".to_string(),
            LocalEntry {
                is_dir: false,
                size: 1,
                mtime: 10,
            },
        )]
        .into_iter()
        .collect();
        assert!(!local_differs_from_snapshot(&local, &known));

        // 修改
        let local2: HashMap<String, LocalEntry> = [(
            "a".to_string(),
            LocalEntry {
                is_dir: false,
                size: 2,
                mtime: 10,
            },
        )]
        .into_iter()
        .collect();
        assert!(local_differs_from_snapshot(&local2, &known));

        // 新增
        let mut local3 = local.clone();
        local3.insert(
            "b".to_string(),
            LocalEntry {
                is_dir: false,
                size: 1,
                mtime: 1,
            },
        );
        assert!(local_differs_from_snapshot(&local3, &known));

        // 删除
        assert!(local_differs_from_snapshot(&HashMap::new(), &known));
    }

    #[test]
    fn synthesize_remote_uses_snapshot() {
        let known: HashMap<String, EntryRecord> = [
            ("docs".to_string(), rec("docs", 0, 0, 5, true)),
            ("docs/a.txt".to_string(), rec("docs/a.txt", 9, 1, 7, false)),
        ]
        .into_iter()
        .collect();
        let map = synthesize_remote_from_snapshot(&known);
        assert_eq!(map.len(), 2);
        assert_eq!(map["docs"].kind, EntryKind::Directory);
        assert_eq!(map["docs/a.txt"].kind, EntryKind::File);
        assert_eq!(map["docs/a.txt"].size, 9);
        assert_eq!(map["docs/a.txt"].mtime, 7);
        assert_eq!(map["docs/a.txt"].name, "a.txt", "名称应从路径末段推得");
        assert_eq!(map["docs/a.txt"].id.0, "id:docs/a.txt");
    }

    #[test]
    fn collect_dir_ids_includes_root_and_dirs_only() {
        let mut entries = HashMap::new();
        entries.insert("docs".to_string(), {
            let mut e = rec("docs", 0, 0, 0, true);
            e.remote_id = Some(RemoteId("d1".into()));
            RemoteEntry {
                id: RemoteId("d1".into()),
                parent_id: RemoteId("0".into()),
                name: "docs".into(),
                kind: EntryKind::Directory,
                size: 0,
                mtime: 0,
                sha1: None,
                pick_code: None,
                deleted: false,
            }
        });
        entries.insert(
            "f.txt".to_string(),
            RemoteEntry {
                id: RemoteId("f1".into()),
                parent_id: RemoteId("0".into()),
                name: "f.txt".into(),
                kind: EntryKind::File,
                size: 1,
                mtime: 0,
                sha1: None,
                pick_code: None,
                deleted: false,
            },
        );
        let map = collect_dir_ids(&entries, &RemoteId("ROOT".into()));
        assert_eq!(map[""].0, "ROOT");
        assert_eq!(map["docs"].0, "d1");
        assert!(!map.contains_key("f.txt"), "文件不应作为目录");
    }

    #[test]
    fn action_labels_are_prefixed() {
        assert_eq!(
            action_label(&Action::Upload { path: "a".into() }),
            "upload:a"
        );
        assert_eq!(
            action_label(&Action::DeleteRemote {
                path: "a".into(),
                remote_id: "1".into()
            }),
            "delete-remote:a"
        );
    }
}
