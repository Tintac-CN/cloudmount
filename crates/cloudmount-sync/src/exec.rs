//! 操作执行器：把 [`crate::reconcile::Action`] 落成真实 I/O。
//!
//! 关键安全性设计：
//! - **删除进回收站**：即使开启了删除传播，也是移到 `<根>/.cloudmount/trash/<时间戳>/`
//!   而不是真删；用户可从本地恢复。
//! - **下载先写临时文件再原子改名**，并设置 mtime = 远端 mtime（避免下轮误判"本地已改"）。
//! - **冲突"保留两者"**：本地副本改名 + 上传为新远端文件，远端原文件下载回原路径，
//!   两条记录都入账，用户不会丢任何一版。
//! - 目录创建走**两阶段**：`dir_ids` 锁内只做查表计划，网络 IO（`list`/`create_dir`）
//!   在锁外、per-前缀创建锁 + 双检之下执行——弱网下单个目录的往返不再阻塞全部
//!   并发事件，同时保住"并发上传不会重复创建远端目录"的确定性。

use crate::cache::BlockCache;
use crate::reconcile::{Action, ConflictKind, ConflictPolicy, SyncPolicy};
use crate::scan::{is_internal, mtime_secs};
use crate::state::{EntryRecord, PinState, SyncState, SyncStats};
use cloudmount_core::{EntryKind, Provider, RemoteId, Result, UploadSource};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 可缓存进块缓存的单文件大小上限（超过则只下载不缓存）。
pub const CACHEABLE_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// 下载/上传的流式缓冲大小。
const STREAM_BUF: usize = 256 * 1024;

/// 执行结果汇总。
#[derive(Debug, Default, Clone)]
pub struct ExecOutcome {
    /// 需要写入快照的记录。
    pub records: Vec<EntryRecord>,
    /// 需要从快照移除的路径。
    pub removed: Vec<String>,
    /// 统计。
    pub stats: SyncStats,
}

/// 操作执行器（`&self` 可并发调用，内部状态有锁）。
pub struct Executor<'a> {
    provider: &'a dyn Provider,
    local_root: PathBuf,
    policy: &'a SyncPolicy,
    cache: &'a BlockCache,
    /// 目录路径 → 远端 ID（初始由远端树播种）。
    dir_ids: Mutex<HashMap<String, RemoteId>>,
    /// 正在创建中的目录前缀 → per-前缀创建锁（F-07）。
    ///
    /// 网络 IO 移出 `dir_ids` 锁后，"同一目录不重复建"的确定性由这把
    /// per-前缀锁 + `dir_ids` 双检保持：只有串行化**同一前缀**的创建，
    /// 不同前缀彼此并行。条目在对应前缀写回 `dir_ids` 后移除（防无界增长；
    /// 等待者克隆的 `Arc` 仍有效，双检兜底，不产生正确性问题）。
    dir_creating: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// 累积结果。
    outcome: Mutex<ExecOutcome>,
}

/// 目录链解析计划（锁内阶段的产物，F-07）。
struct DirPlan {
    /// 最深已解析前缀对应的远端 ID（缺失链的起始父目录）。
    current: RemoteId,
    /// 缺失前缀链（父前缀在前）：`(prefix, 组件名)`。
    missing: Vec<(String, String)>,
}

impl<'a> Executor<'a> {
    /// 创建执行器。
    pub fn new(
        provider: &'a dyn Provider,
        local_root: impl Into<PathBuf>,
        policy: &'a SyncPolicy,
        cache: &'a BlockCache,
        dir_ids: HashMap<String, RemoteId>,
    ) -> Self {
        Self {
            provider,
            local_root: local_root.into(),
            policy,
            cache,
            dir_ids: Mutex::new(dir_ids),
            dir_creating: Mutex::new(HashMap::new()),
            outcome: Mutex::new(ExecOutcome::default()),
        }
    }

    /// 消费执行器，取出累积结果。
    pub fn finish(self) -> ExecOutcome {
        self.outcome.into_inner().expect("lock")
    }

    /// 执行一个动作。
    pub fn execute(&self, action: &Action) -> Result<()> {
        match action {
            Action::MkdirRemote { path } => {
                self.ensure_remote_dir(path)?;
                self.bump(|s| s.dirs_created += 1);
                Ok(())
            }
            Action::MkdirLocal { path } => {
                let abs = self.abs(path);
                std::fs::create_dir_all(&abs)?;
                self.bump(|s| s.dirs_created += 1);
                // 目录也入账（便于下轮判定两端一致）
                let (mtime, _) = self.local_meta(path);
                self.push_record(EntryRecord {
                    rel_path: path.clone(),
                    is_dir: true,
                    remote_id: None,
                    remote_sha1: None,
                    remote_mtime: 0,
                    local_mtime: mtime,
                    size: 0,
                    state: SyncState::InSync,
                    content_check: 0,
                    pin: PinState::Online,
                });
                Ok(())
            }
            Action::Upload { path } => self.upload(path),
            Action::Download { path, remote } => self.download(path, remote),
            Action::DeleteLocal { path } => {
                self.trash_local(path)?;
                self.mark_removed(path);
                self.bump(|s| s.deleted_local += 1);
                Ok(())
            }
            Action::DeleteRemote { path, remote_id } => {
                self.provider.delete(&RemoteId(remote_id.clone()))?;
                self.mark_removed(path);
                self.bump(|s| s.deleted_remote += 1);
                Ok(())
            }
            Action::Verify { path, remote } => self.verify(path, remote),
            Action::Conflict { path, remote, kind } => self.conflict(path, remote, *kind),
            Action::Nothing => Ok(()),
            Action::MoveRemote {
                from,
                to,
                from_remote_id,
                expected_sha1,
            } => self.move_remote(from, to, from_remote_id, expected_sha1.as_deref()),
            Action::RenameLocal { from, to, remote } => self.rename_local(from, to, remote),
        }
    }

    // ---------- 上传 ----------

    /// 上传本地文件（必要时先建远端父目录链）。
    fn upload(&self, path: &str) -> Result<()> {
        let abs = self.abs(path);
        let meta = std::fs::metadata(&abs)?;
        if !meta.is_file() {
            return Ok(()); // 目录不在此处理
        }
        let (parent_rel, name) = split_parent(path);
        let parent_id = self.ensure_remote_dir(&parent_rel)?;
        let result =
            self.provider
                .upload(&parent_id, &name, UploadSource::Path(&abs), meta.len())?;

        // 取权威远端信息（大小/时间/SHA1 以服务端为准，避免下轮误判"远端已改"）
        let remote = self.provider.stat(&result.id)?;
        let local_mtime = mtime_secs(&meta);
        self.push_record(EntryRecord {
            rel_path: path.to_string(),
            is_dir: false,
            remote_id: Some(remote.id.clone()),
            remote_sha1: remote.sha1.clone(),
            remote_mtime: remote.mtime,
            local_mtime,
            size: meta.len(),
            state: SyncState::InSync,
            content_check: 0,
            pin: PinState::Online,
        });
        self.bump(|s| s.uploaded += 1);
        tracing::info!(path, rapid = result.rapid_upload, "上传完成");
        Ok(())
    }

    // ---------- 下载 ----------

    /// 下载远端文件到本地（先临时文件 + 设 mtime + 原子改名）。
    fn download(&self, path: &str, remote: &cloudmount_core::RemoteEntry) -> Result<()> {
        let abs = self.abs(path);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 缓存命中：按远端 SHA1
        let cache_key = remote.sha1.as_ref().map(|s| format!("sha1:{s}"));
        let mut bytes_from_cache = None;
        if let Some(key) = &cache_key {
            if let Some(data) = self.cache.get(key)? {
                bytes_from_cache = Some(data);
            }
        }

        let tmp = self.tmp_path(&abs);
        if let Some(data) = bytes_from_cache {
            std::fs::write(&tmp, &data)?;
        } else {
            let mut reader = self.provider.open_range(
                &remote.id,
                cloudmount_core::RangeSpec {
                    start: 0,
                    end: None,
                },
            )?;
            let mut out = std::fs::File::create(&tmp)?;
            let mut buf = vec![0u8; STREAM_BUF];
            let mut written: u64 = 0;
            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                written += n as u64;
            }
            out.flush()?;
            drop(out);
            tracing::debug!(path, written, "远端内容已落盘");

            // 小文件入缓存（大文件只落盘）
            if remote.size <= CACHEABLE_MAX_BYTES {
                if let Some(key) = &cache_key {
                    if let Ok(data) = std::fs::read(&tmp) {
                        if let Err(e) = self.cache.put(key, &data) {
                            tracing::warn!(error = %e, "写入缓存失败（不影响同步）");
                        }
                    }
                }
            }
        }

        // 保持远端时间戳：避免下轮把"本地 mtime 是下载时间"误判为本地已改
        if remote.mtime > 0 {
            let ft = filetime::FileTime::from_unix_time(remote.mtime, 0);
            if let Err(e) = filetime::set_file_mtime(&tmp, ft) {
                tracing::warn!(path, error = %e, "设置 mtime 失败（不影响内容）");
            }
        }
        atomic_replace(&tmp, &abs)?;

        let size = std::fs::metadata(&abs)
            .map(|m| m.len())
            .unwrap_or(remote.size);
        let local_mtime = if remote.mtime > 0 {
            remote.mtime
        } else {
            self.local_meta(path).0
        };
        self.push_record(EntryRecord {
            rel_path: path.to_string(),
            is_dir: false,
            remote_id: Some(remote.id.clone()),
            remote_sha1: remote.sha1.clone(),
            remote_mtime: remote.mtime,
            local_mtime,
            size,
            state: SyncState::InSync,
            content_check: 0,
            pin: PinState::Online,
        });
        self.bump(|s| s.downloaded += 1);
        tracing::info!(path, size, "下载完成");
        Ok(())
    }

    // ---------- 校验（两端都有、无快照） ----------

    /// 比对本地内容与远端 SHA1：相同则入账，不同则按冲突处理。
    fn verify(&self, path: &str, remote: &cloudmount_core::RemoteEntry) -> Result<()> {
        let abs = self.abs(path);
        let local_sha1 = match hash_file(&abs) {
            Ok(h) => h,
            Err(_) => {
                // 本地读不到（权限/竞态）：以远端为准下载
                return self.download(path, remote);
            }
        };
        let same = match &remote.sha1 {
            Some(rs) => rs.eq_ignore_ascii_case(&local_sha1),
            None => {
                // 远端无 SHA1（如目录）→ 用大小判断
                std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0) == remote.size
            }
        };
        if same {
            let (mtime, size) = self.local_meta(path);
            self.push_record(EntryRecord {
                rel_path: path.to_string(),
                is_dir: remote.kind == EntryKind::Directory,
                remote_id: Some(remote.id.clone()),
                remote_sha1: remote.sha1.clone(),
                remote_mtime: remote.mtime,
                local_mtime: mtime,
                size,
                state: SyncState::InSync,
                content_check: 0,
                pin: PinState::Online,
            });
            tracing::info!(path, "内容一致，直接入账");
            Ok(())
        } else {
            tracing::warn!(path, "两端内容不一致且无同步记录，按冲突处理");
            self.conflict(path, remote, ConflictKind::BothCreated)
        }
    }

    // ---------- 重命名（零传输路径） ----------

    /// 本地重命名 → 远端同步改名。
    ///
    /// 先用哈希二次确认"新路径内容 == 旧远端内容"（配对只看了大小）；
    /// 不一致则回退为"上传新路径 + 删除旧远端"——内容已在本地保留，无数据丢失。
    fn move_remote(
        &self,
        from: &str,
        to: &str,
        from_remote_id: &str,
        expected_sha1: Option<&str>,
    ) -> Result<()> {
        let abs_to = self.abs(to);
        let content_matches = match expected_sha1 {
            Some(sha) => match hash_file(&abs_to) {
                Ok(local_sha) => sha.eq_ignore_ascii_case(&local_sha),
                Err(_) => false,
            },
            // 无 SHA1 可校验：仅凭大小配对，直接执行（后端无哈希能力的场景）
            None => true,
        };

        if content_matches {
            let (parent_rel, name) = split_parent(to);
            let parent_id = self.ensure_remote_dir(&parent_rel)?;
            let id = RemoteId(from_remote_id.to_string());
            self.provider.move_entry(&id, &parent_id)?;
            self.provider.rename(&id, &name)?;
            let remote = self.provider.stat(&id)?;
            let (local_mtime, size) = self.local_meta(to);
            self.mark_removed(from);
            self.push_record(EntryRecord {
                rel_path: to.to_string(),
                is_dir: false,
                remote_id: Some(remote.id.clone()),
                remote_sha1: remote.sha1.clone(),
                remote_mtime: remote.mtime,
                local_mtime,
                size,
                state: SyncState::InSync,
                content_check: 0,
                pin: PinState::Online,
            });
            tracing::info!(from, to, "远端已同步重命名（零传输）");
            Ok(())
        } else {
            tracing::warn!(from, to, "重命名配对的内容校验失败，回退为上传+删除");
            self.upload(to)?;
            self.provider
                .delete(&RemoteId(from_remote_id.to_string()))?;
            self.mark_removed(from);
            Ok(())
        }
    }

    /// 远端重命名 → 本地同步改名。
    ///
    /// 先校验"本地旧路径内容 == 新远端内容"；不一致则回退为
    /// "旧路径进回收站 + 下载新路径"。
    fn rename_local(
        &self,
        from: &str,
        to: &str,
        remote: &cloudmount_core::RemoteEntry,
    ) -> Result<()> {
        let abs_from = self.abs(from);
        let content_matches = if abs_from.exists() {
            match &remote.sha1 {
                Some(sha) => match hash_file(&abs_from) {
                    Ok(local_sha) => sha.eq_ignore_ascii_case(&local_sha),
                    Err(_) => false,
                },
                None => std::fs::metadata(&abs_from).map(|m| m.len()).unwrap_or(0) == remote.size,
            }
        } else {
            false
        };

        if content_matches {
            let abs_to = self.abs(to);
            if let Some(parent) = abs_to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&abs_from, &abs_to)?;
            let (local_mtime, size) = self.local_meta(to);
            self.mark_removed(from);
            self.push_record(EntryRecord {
                rel_path: to.to_string(),
                is_dir: false,
                remote_id: Some(remote.id.clone()),
                remote_sha1: remote.sha1.clone(),
                remote_mtime: remote.mtime,
                local_mtime,
                size,
                state: SyncState::InSync,
                content_check: 0,
                pin: PinState::Online,
            });
            tracing::info!(from, to, "本地已同步重命名（零传输）");
            Ok(())
        } else {
            tracing::warn!(from, to, "重命名配对的内容校验失败，回退为回收站+下载");
            if abs_from.exists() {
                self.trash_local(from)?;
            }
            self.download(to, remote)
        }
    }

    // ---------- 冲突 ----------

    fn conflict(
        &self,
        path: &str,
        remote: &cloudmount_core::RemoteEntry,
        kind: ConflictKind,
    ) -> Result<()> {
        self.bump(|s| s.conflicts += 1);
        tracing::warn!(path, ?kind, policy = ?self.policy.conflict, "冲突");

        match self.policy.conflict {
            ConflictPolicy::KeepBoth => {
                // 1) 本地版本改名保留
                let conflict_rel = self.policy.conflict_path(path, now_unix());
                let conflict_rel = self.unique_local_path(&conflict_rel);
                let from = self.abs(path);
                let to = self.abs(&conflict_rel);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&from, &to)?;
                tracing::warn!(from = path, to = %conflict_rel, "本地版本已改名为冲突副本");

                // 2) 本地副本上传为新远端文件（保住本地这一版）
                self.upload(&conflict_rel)?;

                // 3) 远端原文件落回原路径
                self.download(path, remote)
            }
            ConflictPolicy::PreferLocal => {
                // 远端以本地为准（注意：部分后端同名上传会生成副本而非覆盖，
                // 如需严格覆盖应改为 delete + upload，见 M1.5 TODO）
                self.upload(path)
            }
            ConflictPolicy::PreferRemote => {
                let abs = self.abs(path);
                if abs.exists() {
                    self.trash_local(path)?;
                }
                self.download(path, remote)
            }
        }
    }

    // ---------- 目录与辅助 ----------

    /// 确保远端目录链存在，返回最末端目录的 ID。
    ///
    /// 两阶段（F-07）：
    /// 1. **锁内只做计划**——拿 `dir_ids` 锁沿 `rel` 组件走 map，得出已解析 ID
    ///    与缺失前缀链，立即放锁（见 [`Self::plan_dir_chain`]，持锁期间零网络 IO）；
    /// 2. **锁外做网络**——对每个缺失前缀，在 per-前缀创建锁（[`Self::dir_creating`]）
    ///    之下**双检** `dir_ids`（其它线程可能已完成），仍缺失才 `provider.list` 查重 /
    ///    `provider.create_dir`，结果写回 `dir_ids`。
    ///
    /// "并发上传不会重复建目录"的确定性由 per-前缀锁 + 双检保持；锁序固定为
    /// 父前缀先于子前缀（与遍历顺序一致，逐个获取、从不嵌套持有两把创建锁），
    /// 无环，无死锁。
    fn ensure_remote_dir(&self, rel: &str) -> Result<RemoteId> {
        let plan = self.plan_dir_chain(rel)?;
        let mut current = plan.current;
        for (prefix, comp) in plan.missing {
            // 取 per-前缀锁：只克隆 Arc，立刻释放锁表锁（不在锁表锁下做任何等待）
            let gate = {
                let mut creating = self
                    .dir_creating
                    .lock()
                    .map_err(|_| cloudmount_core::Error::Other("目录创建锁表中毒".into()))?;
                creating
                    .entry(prefix.clone())
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone()
            };
            let _gate = gate
                .lock()
                .map_err(|_| cloudmount_core::Error::Other("目录创建锁中毒".into()))?;

            // 双检：拿锁期间其它线程可能已完成同一前缀
            let hit = {
                self.dir_ids
                    .lock()
                    .map_err(|_| cloudmount_core::Error::Other("目录缓存锁中毒".into()))?
                    .get(&prefix)
                    .cloned()
            };
            if let Some(id) = hit {
                current = id;
            } else {
                // 先看远端是否已存在同名目录（避免重复创建）
                let existing = self
                    .provider
                    .list(&current)?
                    .into_iter()
                    .find(|e| e.kind == EntryKind::Directory && e.name == comp);
                let id = match existing {
                    Some(e) => e.id,
                    None => {
                        tracing::info!(dir = %prefix, "创建远端目录");
                        self.provider.create_dir(&current, &comp)?.id
                    }
                };
                // **先写回缓存，后移除创建锁条目**（顺序不可换：条目只在对应前缀
                // 已确定进入 dir_ids 后才允许消失，否则后续线程双检不命中会重复建目录）
                self.dir_ids
                    .lock()
                    .map_err(|_| cloudmount_core::Error::Other("目录缓存锁中毒".into()))?
                    .insert(prefix.clone(), id.clone());
                current = id;
            }
            drop(_gate);

            // 顺手移除已完成的创建锁条目，防锁表无界增长。带 `Arc::ptr_eq` 判定：
            // 只清理"自己这把"门锁——若条目已被移除并被更晚的规划者重建（新 Arc），
            // 说明有新的在途创建，不得误删其门锁。
            if let Ok(mut creating) = self.dir_creating.lock() {
                if creating
                    .get(&prefix)
                    .is_some_and(|g| Arc::ptr_eq(g, &gate))
                {
                    creating.remove(&prefix);
                }
            }
        }
        Ok(current)
    }

    /// [`ensure_remote_dir`] 的锁内计划阶段：沿 `rel` 组件走 `dir_ids` map，
    /// 返回最深已解析 ID 与缺失前缀链。持锁期间**只做查表**（零网络 IO）。
    ///
    /// 注意：持锁期间不得再调用会加同一把锁的方法（std Mutex 不可重入）——
    /// 根 ID 直接从已持有的 map 读，而不是调会再加锁的入口。
    fn plan_dir_chain(&self, rel: &str) -> Result<DirPlan> {
        let map = self
            .dir_ids
            .lock()
            .map_err(|_| cloudmount_core::Error::Other("目录缓存锁中毒".into()))?;
        let root = map.get("").cloned().unwrap_or_else(|| RemoteId("0".into()));
        if rel.is_empty() {
            return Ok(DirPlan {
                current: root,
                missing: Vec::new(),
            });
        }
        if let Some(id) = map.get(rel) {
            return Ok(DirPlan {
                current: id.clone(),
                missing: Vec::new(),
            });
        }
        // 自顶向下：已解析的最深前缀 + 缺失前缀链（父前缀在前）
        let mut prefix = String::new();
        let mut current = root;
        let mut missing = Vec::new();
        for comp in rel.split('/').filter(|c| !c.is_empty()) {
            prefix = if prefix.is_empty() {
                comp.to_string()
            } else {
                format!("{prefix}/{comp}")
            };
            match map.get(&prefix) {
                Some(id) => current = id.clone(),
                None => missing.push((prefix.clone(), comp.to_string())),
            }
        }
        Ok(DirPlan { current, missing })
    }

    /// 本地绝对路径。
    fn abs(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.local_root.clone()
        } else {
            self.local_root
                .join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
        }
    }

    /// 临时文件路径（与目标同目录，保证 rename 原子性）。
    fn tmp_path(&self, abs: &Path) -> PathBuf {
        let name = abs
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_string();
        abs.with_file_name(format!("{name}.cmtmp"))
    }

    /// 删除本地条目：**移动进回收站**而不是真删。
    fn trash_local(&self, rel: &str) -> Result<()> {
        let abs = self.abs(rel);
        if !abs.exists() {
            return Ok(());
        }
        let trash = self
            .local_root
            .join(".cloudmount")
            .join("trash")
            .join(now_unix().to_string());
        let dest = trash.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&abs, &dest)?;
        tracing::warn!(path = rel, trash = %dest.display(), "已移入回收站（非永久删除）");
        Ok(())
    }

    /// 本地条目元数据（mtime 秒, size）。
    fn local_meta(&self, rel: &str) -> (i64, u64) {
        std::fs::metadata(self.abs(rel))
            .map(|m| (mtime_secs(&m), m.len()))
            .unwrap_or((0, 0))
    }

    /// 生成不冲突的本地路径（若已存在则追加序号）。
    fn unique_local_path(&self, rel: &str) -> String {
        let mut candidate = rel.to_string();
        let mut n = 1;
        while self.abs(&candidate).exists() {
            n += 1;
            candidate = match rel.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() => format!("{stem}-{n}.{ext}"),
                _ => format!("{rel}-{n}"),
            };
        }
        candidate
    }

    fn push_record(&self, rec: EntryRecord) {
        if let Ok(mut o) = self.outcome.lock() {
            // 同路径后写覆盖先写（并发场景下以最后完成的为准）
            o.records.retain(|r| r.rel_path != rec.rel_path);
            o.records.push(rec);
        }
    }

    fn mark_removed(&self, path: &str) {
        if let Ok(mut o) = self.outcome.lock() {
            o.removed.push(path.to_string());
        }
    }

    fn bump(&self, f: impl FnOnce(&mut SyncStats)) {
        if let Ok(mut o) = self.outcome.lock() {
            f(&mut o.stats);
        }
    }
}

/// 拆分父目录与文件名。
fn split_parent(rel: &str) -> (String, String) {
    match rel.rsplit_once('/') {
        Some((p, n)) => (p.to_string(), n.to_string()),
        None => (String::new(), rel.to_string()),
    }
}

/// 流式计算文件 SHA1（大写十六进制）。
fn hash_file(path: &Path) -> Result<String> {
    hash_file_impl(path)
}

/// [`hash_file`] 的公开入口（engine 的内容校验通道复用）。
pub fn hash_file_pub(path: &Path) -> Result<String> {
    hash_file_impl(path)
}

fn hash_file_impl(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; STREAM_BUF];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode_upper(h.finalize()))
}

/// 原子替换：把 `tmp` 改成 `dest`（同目录 rename；目标存在则先删）。
fn atomic_replace(tmp: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    std::fs::rename(tmp, dest)?;
    Ok(())
}

/// 当前 Unix 秒。
fn now_unix() -> i64 {
    crate::store::now_unix()
}

/// 判断路径是否属于引擎内部（供外部过滤）。
pub fn is_internal_path(rel: &str) -> bool {
    rel.split('/').any(is_internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_parent_handles_nested_and_top_level() {
        assert_eq!(split_parent("a.txt"), (String::new(), "a.txt".into()));
        assert_eq!(
            split_parent("docs/sub/a.txt"),
            ("docs/sub".to_string(), "a.txt".into())
        );
    }

    #[test]
    fn internal_path_detection() {
        assert!(is_internal_path(".cloudmount/trash/1/a.txt"));
        assert!(is_internal_path("docs/x.cmtmp"));
        assert!(!is_internal_path("docs/a.txt"));
    }

    #[test]
    fn hash_file_matches_known_vector() {
        let dir = std::env::temp_dir().join(format!("cm-exec-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("abc.bin");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            hash_file(&p).unwrap(),
            "A9993E364706816ABA3E25717850C26C9CD0D89D"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_replace_overwrites_existing() {
        let dir = std::env::temp_dir().join(format!("cm-exec-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("f.txt");
        let tmp = dir.join("f.txt.cmtmp");
        std::fs::write(&dest, b"old").unwrap();
        std::fs::write(&tmp, b"new").unwrap();
        atomic_replace(&tmp, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
