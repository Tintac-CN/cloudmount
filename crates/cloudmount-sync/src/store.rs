//! SQLite 元数据存储：上次同步快照、状态机、pin 策略、引擎水位。
//!
//! 设计取舍：
//! - 使用 `rusqlite` 的 **bundled** 特性（内嵌 SQLite），Windows/Linux/ARM 交叉编译
//!   无外部依赖。
//! - 表结构以 **相对路径为主键**（同步引擎的天然寻址方式），`remote_id` 建索引
//!   以便反查。
//! - 引擎每轮只做"读快照 → 计算 → 写回"三段式，写回在一个事务里完成，
//!   避免半更新状态。

use crate::state::{EntryRecord, PinState, SyncState};
use cloudmount_core::{Error, RemoteId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

/// 引擎水位键名（用于远端廉价探测的短路判定）。
pub const META_PROBE_ROOT: &str = "probe.root";
/// 上次完整扫描完成的 Unix 秒。
pub const META_LAST_SCAN: &str = "last_scan_unix";

/// 元数据存储。
pub struct MetaStore {
    conn: Connection,
}

impl MetaStore {
    /// 打开（或创建）磁盘数据库。
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(db_err)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// 内存数据库（测试用）。
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// 建表（幂等）。
    fn migrate(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS meta (
                    k TEXT PRIMARY KEY,
                    v TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS entries (
                    rel_path     TEXT PRIMARY KEY,
                    is_dir       INTEGER NOT NULL,
                    remote_id    TEXT,
                    remote_sha1  TEXT,
                    remote_mtime INTEGER NOT NULL DEFAULT 0,
                    local_mtime  INTEGER NOT NULL DEFAULT 0,
                    size         INTEGER NOT NULL DEFAULT 0,
                    state        TEXT NOT NULL,
                    pin          TEXT NOT NULL DEFAULT 'online',
                    updated_at   INTEGER NOT NULL DEFAULT 0
                );

                CREATE INDEX IF NOT EXISTS idx_entries_remote_id ON entries(remote_id);
                CREATE INDEX IF NOT EXISTS idx_entries_state     ON entries(state);
                "#,
            )
            .map_err(db_err)
    }

    /// 读水位。
    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT v FROM meta WHERE k = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(db_err)
    }

    /// 写水位。
    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO meta(k, v) VALUES(?1, ?2)
                 ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                params![key, value],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// 插入或更新一条快照记录。
    pub fn upsert_entry(&self, rec: &EntryRecord) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT INTO entries
                     (rel_path, is_dir, remote_id, remote_sha1, remote_mtime, local_mtime,
                      size, state, pin, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                   ON CONFLICT(rel_path) DO UPDATE SET
                     is_dir       = excluded.is_dir,
                     remote_id    = excluded.remote_id,
                     remote_sha1  = excluded.remote_sha1,
                     remote_mtime = excluded.remote_mtime,
                     local_mtime  = excluded.local_mtime,
                     size         = excluded.size,
                     state        = excluded.state,
                     pin          = excluded.pin,
                     updated_at   = excluded.updated_at"#,
                params![
                    rec.rel_path,
                    rec.is_dir as i64,
                    rec.remote_id.as_ref().map(|r| r.0.as_str()),
                    rec.remote_sha1,
                    rec.remote_mtime,
                    rec.local_mtime,
                    rec.size as i64,
                    rec.state.as_str(),
                    rec.pin.as_str(),
                    now_unix(),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// 批量写入（单事务，供每轮结束时一次性提交）。
    pub fn upsert_entries(&mut self, recs: &[EntryRecord]) -> Result<()> {
        let tx = self.conn.transaction().map_err(db_err)?;
        {
            let mut stmt = tx
                .prepare(
                    r#"INSERT INTO entries
                         (rel_path, is_dir, remote_id, remote_sha1, remote_mtime, local_mtime,
                          size, state, pin, updated_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                       ON CONFLICT(rel_path) DO UPDATE SET
                         is_dir       = excluded.is_dir,
                         remote_id    = excluded.remote_id,
                         remote_sha1  = excluded.remote_sha1,
                         remote_mtime = excluded.remote_mtime,
                         local_mtime  = excluded.local_mtime,
                         size         = excluded.size,
                         state        = excluded.state,
                         pin          = excluded.pin,
                         updated_at   = excluded.updated_at"#,
                )
                .map_err(db_err)?;
            let ts = now_unix();
            for rec in recs {
                stmt.execute(params![
                    rec.rel_path,
                    rec.is_dir as i64,
                    rec.remote_id.as_ref().map(|r| r.0.as_str()),
                    rec.remote_sha1,
                    rec.remote_mtime,
                    rec.local_mtime,
                    rec.size as i64,
                    rec.state.as_str(),
                    rec.pin.as_str(),
                    ts,
                ])
                .map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// 读取单个记录。
    pub fn get_entry(&self, rel_path: &str) -> Result<Option<EntryRecord>> {
        self.conn
            .query_row(
                r#"SELECT rel_path, is_dir, remote_id, remote_sha1, remote_mtime,
                          local_mtime, size, state, pin
                   FROM entries WHERE rel_path = ?1"#,
                params![rel_path],
                map_row,
            )
            .optional()
            .map_err(db_err)
    }

    /// 加载全部快照（三向对比的基准集）。
    pub fn load_all(&self) -> Result<HashMap<String, EntryRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                r#"SELECT rel_path, is_dir, remote_id, remote_sha1, remote_mtime,
                          local_mtime, size, state, pin
                   FROM entries"#,
            )
            .map_err(db_err)?;
        let rows = stmt.query_map([], map_row).map_err(db_err)?;
        let mut out = HashMap::new();
        for row in rows {
            let rec = row.map_err(db_err)?;
            out.insert(rec.rel_path.clone(), rec);
        }
        Ok(out)
    }

    /// 删除记录（条目已从两端消失）。
    pub fn remove_entry(&self, rel_path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM entries WHERE rel_path = ?1", params![rel_path])
            .map_err(db_err)?;
        Ok(())
    }

    /// 按状态统计条目数（供 UI/诊断）。
    pub fn count_by_state(&self) -> Result<HashMap<String, u64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT state, COUNT(*) FROM entries GROUP BY state")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })
            .map_err(db_err)?;
        let mut out = HashMap::new();
        for row in rows {
            let (k, v) = row.map_err(db_err)?;
            out.insert(k, v);
        }
        Ok(out)
    }

    /// 清空快照（用于"强制全量重同步"）。
    pub fn clear(&self) -> Result<()> {
        self.conn
            .execute_batch("DELETE FROM entries;")
            .map_err(db_err)
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EntryRecord> {
    let state_str: String = row.get(7)?;
    let pin_str: String = row.get(8)?;
    Ok(EntryRecord {
        rel_path: row.get(0)?,
        is_dir: row.get::<_, i64>(1)? != 0,
        remote_id: row.get::<_, Option<String>>(2)?.map(RemoteId),
        remote_sha1: row.get(3)?,
        remote_mtime: row.get(4)?,
        local_mtime: row.get(5)?,
        size: row.get::<_, i64>(6)? as u64,
        state: SyncState::from_str_lossy(&state_str),
        pin: PinState::from_str_lossy(&pin_str),
    })
}

fn db_err(e: rusqlite::Error) -> Error {
    Error::Db(e.to_string())
}

/// 当前 Unix 秒。
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(path: &str, state: SyncState) -> EntryRecord {
        EntryRecord {
            rel_path: path.to_string(),
            is_dir: false,
            remote_id: Some(RemoteId(format!("id-{path}"))),
            remote_sha1: Some("SHA1".into()),
            remote_mtime: 100,
            local_mtime: 100,
            size: 42,
            state,
            pin: PinState::Online,
        }
    }

    #[test]
    fn upsert_and_get_round_trip() {
        let store = MetaStore::open_in_memory().unwrap();
        store
            .upsert_entry(&rec("a/b.txt", SyncState::InSync))
            .unwrap();
        let got = store.get_entry("a/b.txt").unwrap().expect("应存在");
        assert_eq!(got.rel_path, "a/b.txt");
        assert_eq!(got.size, 42);
        assert_eq!(got.state, SyncState::InSync);
        assert_eq!(got.remote_id.unwrap().0, "id-a/b.txt");
    }

    #[test]
    fn upsert_overwrites_state_and_times() {
        let store = MetaStore::open_in_memory().unwrap();
        store.upsert_entry(&rec("f", SyncState::Pending)).unwrap();
        let mut updated = rec("f", SyncState::InSync);
        updated.size = 100;
        updated.local_mtime = 200;
        store.upsert_entry(&updated).unwrap();

        let got = store.get_entry("f").unwrap().unwrap();
        assert_eq!(got.state, SyncState::InSync);
        assert_eq!(got.size, 100);
        assert_eq!(got.local_mtime, 200);
    }

    #[test]
    fn load_all_and_remove() {
        let mut store = MetaStore::open_in_memory().unwrap();
        store
            .upsert_entries(&[rec("a", SyncState::InSync), rec("b", SyncState::LocalOnly)])
            .unwrap();
        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all["b"].state, SyncState::LocalOnly);

        store.remove_entry("a").unwrap();
        assert_eq!(store.load_all().unwrap().len(), 1);
    }

    #[test]
    fn meta_kv_round_trip() {
        let store = MetaStore::open_in_memory().unwrap();
        assert_eq!(store.get_meta(META_PROBE_ROOT).unwrap(), None);
        store.set_meta(META_PROBE_ROOT, "3|1700000000").unwrap();
        assert_eq!(
            store.get_meta(META_PROBE_ROOT).unwrap().as_deref(),
            Some("3|1700000000")
        );
        // 覆盖写
        store.set_meta(META_PROBE_ROOT, "4|1700000001").unwrap();
        assert_eq!(
            store.get_meta(META_PROBE_ROOT).unwrap().as_deref(),
            Some("4|1700000001")
        );
    }

    #[test]
    fn count_by_state_groups() {
        let mut store = MetaStore::open_in_memory().unwrap();
        store
            .upsert_entries(&[
                rec("a", SyncState::InSync),
                rec("b", SyncState::InSync),
                rec("c", SyncState::Conflict),
            ])
            .unwrap();
        let counts = store.count_by_state().unwrap();
        assert_eq!(counts["in_sync"], 2);
        assert_eq!(counts["conflict"], 1);
    }

    #[test]
    fn pin_state_persists() {
        let store = MetaStore::open_in_memory().unwrap();
        let mut r = rec("pinned.txt", SyncState::InSync);
        r.pin = PinState::Pinned;
        store.upsert_entry(&r).unwrap();
        assert_eq!(
            store.get_entry("pinned.txt").unwrap().unwrap().pin,
            PinState::Pinned
        );
    }

    #[test]
    fn clear_empties_snapshot() {
        let mut store = MetaStore::open_in_memory().unwrap();
        store
            .upsert_entries(&[rec("a", SyncState::InSync)])
            .unwrap();
        store.clear().unwrap();
        assert!(store.load_all().unwrap().is_empty());
    }
}
