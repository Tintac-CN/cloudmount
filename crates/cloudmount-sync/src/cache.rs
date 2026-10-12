//! 内容块缓存（LRU + 字节限额 + 命中统计）。
//!
//! M1 的用途：**下载去重与小文件加速**——按远端 SHA1 缓存文件体，
//! 需要同一内容时（冲突保留两者、本地被删后恢复、重复下载）直接命中本地，
//! 省一次网络往返（对 1 r/s 限速的后端很值）。
//!
//! M2 的 VFS 层会把键从"整文件"细化为"(file_id, offset, len)"分块，
//! 本模块的 LRU/限额/统计接口保持不变。
//!
//! 实现说明：淘汰采用"扫描最小 last_used"的 O(n) 策略（n = 缓存条目数）。
//! 典型 n 在数百量级、淘汰只在超限时发生，故不引入链表式 O(1) LRU 的复杂度。

use cloudmount_core::{Error, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// 缓存命中统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheStats {
    /// 命中次数。
    pub hits: u64,
    /// 未命中次数。
    pub misses: u64,
    /// 写入次数。
    pub inserts: u64,
    /// 淘汰次数。
    pub evictions: u64,
    /// 当前占用字节。
    pub bytes: u64,
    /// 字节上限。
    pub limit_bytes: u64,
}

impl CacheStats {
    /// 命中率（无访问时为 0）。
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    size: u64,
    /// 逻辑时间戳（单调递增计数器），越大越新。
    last_used: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    entries: HashMap<String, IndexEntry>,
    /// 单调递增访问计数。
    clock: u64,
    /// 累计统计（持久化，便于长期观测）。
    hits: u64,
    misses: u64,
    inserts: u64,
    evictions: u64,
}

/// 磁盘内容缓存。
pub struct BlockCache {
    dir: PathBuf,
    limit_bytes: u64,
    index: Mutex<Index>,
}

/// 索引文件名。
const INDEX_FILE: &str = "index.json";

impl BlockCache {
    /// 打开（或创建）缓存目录并加载索引。
    pub fn open(dir: impl Into<PathBuf>, limit_bytes: u64) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let index = match std::fs::read(dir.join(INDEX_FILE)) {
            Ok(bytes) => serde_json::from_slice::<Index>(&bytes).unwrap_or_else(|e| {
                tracing::warn!("缓存索引损坏，重建: {e}");
                Index::default()
            }),
            Err(_) => Index::default(),
        };
        Ok(Self {
            dir,
            limit_bytes,
            index: Mutex::new(index),
        })
    }

    /// 条目在磁盘上的文件名（对键做 SHA1，避免路径穿越与非法字符）。
    fn blob_path(&self, key: &str) -> PathBuf {
        let digest = Sha1::digest(key.as_bytes());
        self.dir.join(format!("{}.blob", hex::encode(digest)))
    }

    /// 读取（命中则更新 LRU 并计数）。
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.blob_path(key);
        let mut idx = self.index.lock().map_err(lock_err)?;
        match std::fs::read(&path) {
            Ok(bytes) => {
                idx.clock += 1;
                let clock = idx.clock;
                if let Some(e) = idx.entries.get_mut(key) {
                    e.last_used = clock;
                }
                idx.hits += 1;
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 磁盘文件被外部清理：同步移除索引项
                idx.entries.remove(key);
                idx.misses += 1;
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// 是否已缓存（不更新 LRU，也不计数）。
    pub fn contains(&self, key: &str) -> bool {
        self.index
            .lock()
            .map(|i| i.entries.contains_key(key))
            .unwrap_or(false)
    }

    /// 写入并触发淘汰。
    pub fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        if data.len() as u64 > self.limit_bytes {
            // 单个对象超过总限额：不缓存（避免立刻被淘汰导致的抖动）
            tracing::debug!(key, size = data.len(), "对象超过缓存限额，跳过缓存");
            return Ok(());
        }
        let path = self.blob_path(key);
        // 先写临时文件再改名，避免半写文件进入索引
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &path)?;

        {
            let mut idx = self.index.lock().map_err(lock_err)?;
            idx.clock += 1;
            let clock = idx.clock;
            idx.entries.insert(
                key.to_string(),
                IndexEntry {
                    size: data.len() as u64,
                    last_used: clock,
                },
            );
            idx.inserts += 1;
        }
        self.evict_to_limit()
    }

    /// 淘汰直到满足字节上限。
    pub fn evict_to_limit(&self) -> Result<()> {
        let mut idx = self.index.lock().map_err(lock_err)?;
        loop {
            let total: u64 = idx.entries.values().map(|e| e.size).sum();
            if total <= self.limit_bytes {
                break;
            }
            // 找最久未使用的条目
            let victim = idx
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            idx.entries.remove(&victim);
            idx.evictions += 1;
            let path = self.blob_path(&victim);
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => tracing::warn!(error = %e, "删除缓存块失败"),
            }
        }
        Ok(())
    }

    /// 当前统计。
    pub fn stats(&self) -> Result<CacheStats> {
        let idx = self.index.lock().map_err(lock_err)?;
        Ok(CacheStats {
            hits: idx.hits,
            misses: idx.misses,
            inserts: idx.inserts,
            evictions: idx.evictions,
            bytes: idx.entries.values().map(|e| e.size).sum(),
            limit_bytes: self.limit_bytes,
        })
    }

    /// 清空缓存（索引 + 磁盘）。
    pub fn clear(&self) -> Result<()> {
        let mut idx = self.index.lock().map_err(lock_err)?;
        for key in idx.entries.keys() {
            let _ = std::fs::remove_file(self.blob_path(key));
        }
        idx.entries.clear();
        Ok(())
    }

    /// 持久化索引（引擎每轮结束时调用）。
    pub fn flush(&self) -> Result<()> {
        let idx = self.index.lock().map_err(lock_err)?;
        let bytes = serde_json::to_vec(&*idx)?;
        let tmp = self.dir.join(format!("{INDEX_FILE}.tmp"));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, self.dir.join(INDEX_FILE))?;
        Ok(())
    }
}

fn lock_err<T>(_: T) -> Error {
    Error::Other("缓存索引锁中毒".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cm-cache-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn put_get_round_trip_and_stats() {
        let dir = tmpdir("basic");
        let c = BlockCache::open(&dir, 1024).unwrap();
        assert!(c.get("k1").unwrap().is_none(), "未命中应返回 None");

        c.put("k1", b"hello").unwrap();
        assert_eq!(c.get("k1").unwrap().unwrap(), b"hello");
        assert!(c.contains("k1"));

        let s = c.stats().unwrap();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
        assert_eq!(s.inserts, 1);
        assert_eq!(s.bytes, 5);
        assert_eq!(s.limit_bytes, 1024);
        assert!((s.hit_ratio() - 0.5).abs() < 1e-9);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn evicts_least_recently_used_first() {
        let dir = tmpdir("lru");
        let c = BlockCache::open(&dir, 10).unwrap(); // 限额 10 字节
        c.put("a", b"1234").unwrap(); // 4
        c.put("b", b"1234").unwrap(); // 8
                                      // 访问 a，使 b 成为最久未使用
        assert!(c.get("a").unwrap().is_some());
        c.put("c", b"1234").unwrap(); // 12 > 10 → 淘汰 b

        assert!(c.contains("a"), "刚访问过的不应被淘汰");
        assert!(!c.contains("b"), "最久未使用的应被淘汰");
        assert!(c.contains("c"));

        let s = c.stats().unwrap();
        assert_eq!(s.evictions, 1);
        assert_eq!(s.bytes, 8);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oversized_object_is_not_cached() {
        let dir = tmpdir("oversize");
        let c = BlockCache::open(&dir, 4).unwrap();
        c.put("big", b"12345").unwrap(); // 5 > 4
        assert!(!c.contains("big"));
        assert_eq!(c.stats().unwrap().bytes, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn index_persists_across_reopen() {
        let dir = tmpdir("persist");
        {
            let c = BlockCache::open(&dir, 1024).unwrap();
            c.put("keep", b"persisted").unwrap();
            c.flush().unwrap();
        }
        let c2 = BlockCache::open(&dir, 1024).unwrap();
        assert!(c2.contains("keep"), "索引应跨进程恢复");
        assert_eq!(c2.get("keep").unwrap().unwrap(), b"persisted");
        assert_eq!(c2.stats().unwrap().inserts, 1, "统计也应持久化");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn external_blob_removal_is_tolerated() {
        let dir = tmpdir("external");
        let c = BlockCache::open(&dir, 1024).unwrap();
        c.put("gone", b"x").unwrap();
        // 模拟外部清理磁盘文件
        let blob = c.blob_path("gone");
        std::fs::remove_file(blob).unwrap();
        assert!(c.get("gone").unwrap().is_none());
        assert!(!c.contains("gone"), "索引项应被同步移除");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_removes_everything() {
        let dir = tmpdir("clear");
        let c = BlockCache::open(&dir, 1024).unwrap();
        c.put("a", b"1").unwrap();
        c.put("b", b"2").unwrap();
        c.clear().unwrap();
        assert_eq!(c.stats().unwrap().bytes, 0);
        assert!(!c.contains("a"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
