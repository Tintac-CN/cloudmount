//! 同步引擎端到端测试（Mock 远端 + 临时本地目录）。
//!
//! 覆盖验收路径：**双向同步、冲突保留两者、删除策略、幂等、短路优化**。

mod common;

use cloudmount_core::RemoteId;
use cloudmount_sync::reconcile::{ConflictPolicy, SyncPolicy};
use cloudmount_sync::{SyncEngine, SyncEngineConfig};
use common::MockRemote;
use std::path::PathBuf;
use std::sync::Arc;

/// 每个测试独立的本地工作区。
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cm-sync-e2e-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root
            .join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
    }

    fn write(&self, rel: &str, data: &[u8]) {
        let p = self.path(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, data).unwrap();
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.path(rel)).unwrap()
    }

    fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    fn engine(&self, remote: &MockRemote, policy: SyncPolicy) -> SyncEngine {
        let cfg = SyncEngineConfig {
            policy,
            scheduler: cloudmount_sync::queue::SchedulerConfig {
                concurrency: 1, // 串行保证断言确定性
                ..Default::default()
            },
            // 测试固定轮询后端：原生 watcher 的事件是异步送达的，
            // "写文件→立即断言"会引入竞态；原生路径由专门测试覆盖。
            watcher_backend: cloudmount_sync::watcher::WatcherBackend::Polling,
            ..SyncEngineConfig::new(&self.root)
        };
        SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn local_new_file_is_uploaded_and_second_run_is_idempotent() {
    let ws = Workspace::new("upload");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("hello.txt", b"hello world");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.uploaded, 1, "本地新文件应被上传: {stats:?}");
    assert_eq!(remote.read("hello.txt").unwrap(), b"hello world");

    // 第二轮：无变化 → 短路，且不应再上传
    let before = remote
        .ops()
        .upload
        .load(std::sync::atomic::Ordering::SeqCst);
    let stats2 = engine.run_once().unwrap();
    assert_eq!(stats2.uploaded, 0);
    assert_eq!(stats2.errors, 0);
    assert_eq!(
        remote
            .ops()
            .upload
            .load(std::sync::atomic::Ordering::SeqCst),
        before,
        "幂等轮不应再次上传"
    );
}

#[test]
fn remote_new_file_is_downloaded() {
    let ws = Workspace::new("download");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    remote.put_file("from-cloud.txt", b"remote content");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.downloaded, 1, "远端新文件应被下载: {stats:?}");
    assert_eq!(ws.read("from-cloud.txt"), b"remote content");
}

#[test]
fn nested_remote_tree_creates_local_dirs() {
    let ws = Workspace::new("nested-dl");
    let remote = MockRemote::new();
    remote.put_dir("docs/2026");
    remote.put_file("docs/2026/report.md", b"# report");
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    let stats = engine.run_once().unwrap();
    assert!(ws.exists("docs/2026/report.md"));
    assert_eq!(ws.read("docs/2026/report.md"), b"# report");
    assert!(stats.dirs_created >= 1, "应创建本地目录: {stats:?}");
}

#[test]
fn nested_local_tree_creates_remote_dirs() {
    let ws = Workspace::new("nested-ul");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("projects/a/src/main.rs", b"fn main() {}");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.uploaded, 1);
    assert!(stats.dirs_created >= 1, "应创建远端目录: {stats:?}");
    assert_eq!(
        remote.read("projects/a/src/main.rs").unwrap(),
        b"fn main() {}"
    );
    // 目录也在远端建出来了
    assert!(remote.exists("projects/a/src"));
}

#[test]
fn local_modification_uploads_new_content() {
    let ws = Workspace::new("modify-local");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("doc.txt", b"v1");
    engine.run_once().unwrap();
    assert_eq!(remote.read("doc.txt").unwrap(), b"v1");

    // 改变大小以确保 (size, mtime) 检测生效（秒级粒度见 scan 模块的盲区说明）
    ws.write("doc.txt", b"v2-longer");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.uploaded, 1, "本地修改应触发上传: {stats:?}");
    assert_eq!(remote.read("doc.txt").unwrap(), b"v2-longer");
}

#[test]
fn remote_modification_downloads_new_content() {
    let ws = Workspace::new("modify-remote");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    remote.put_file("doc.txt", b"v1");
    engine.run_once().unwrap();
    assert_eq!(ws.read("doc.txt"), b"v1");

    remote.put_file("doc.txt", b"v2-remote");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.downloaded, 1, "远端修改应触发下载: {stats:?}");
    assert_eq!(ws.read("doc.txt"), b"v2-remote");
}

#[test]
fn both_modified_keeps_both_versions() {
    let ws = Workspace::new("conflict");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("shared.txt", b"base");
    engine.run_once().unwrap();

    // 两端各自修改
    ws.write("shared.txt", b"local-version");
    remote.put_file("shared.txt", b"remote-version");

    let stats = engine.run_once().unwrap();
    assert_eq!(stats.conflicts, 1, "应检测到冲突: {stats:?}");

    // 原路径 = 远端版本
    assert_eq!(
        ws.read("shared.txt"),
        b"remote-version",
        "原路径应为远端版本"
    );
    // 本地版本以冲突副本形式保留，并被上传到远端
    let conflict_paths: Vec<String> = remote
        .all_paths()
        .into_iter()
        .filter(|p| p.contains("conflict-"))
        .collect();
    assert_eq!(conflict_paths.len(), 1, "冲突副本应上传为远端新文件");
    let local_conflicts: Vec<String> = std::fs::read_dir(&ws.root)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains("conflict-"))
        .collect();
    assert_eq!(local_conflicts.len(), 1, "本地冲突副本应存在");
    assert_eq!(ws.read(&local_conflicts[0]), b"local-version");
}

#[test]
fn identical_files_without_snapshot_are_reconciled_not_conflicted() {
    let ws = Workspace::new("identical");
    let remote = MockRemote::new();

    // 两端独立放入完全相同的内容，且从未同步过
    ws.write("same.txt", b"identical-bytes");
    remote.put_file("same.txt", b"identical-bytes");

    let mut engine = ws.engine(&remote, SyncPolicy::default());
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.conflicts, 0, "内容一致不应报冲突: {stats:?}");
    assert_eq!(stats.uploaded, 0);
    assert_eq!(stats.downloaded, 0);

    // 已入账：再跑一轮应短路且无操作
    let stats2 = engine.run_once().unwrap();
    assert!(!stats2.has_changes(), "验证入账后应稳定: {stats2:?}");
}

#[test]
fn remote_delete_is_not_propagated_by_default() {
    let ws = Workspace::new("del-conservative");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("keepme.txt", b"important");
    engine.run_once().unwrap();

    remote.remove("keepme.txt");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.deleted_local, 0, "默认不应删除本地文件: {stats:?}");
    assert!(ws.exists("keepme.txt"), "本地文件必须保留");
    assert_eq!(stats.uploaded, 1, "保守策略下应把本地版本重新上传回去");
    assert_eq!(remote.read("keepme.txt").unwrap(), b"important");
}

#[test]
fn remote_delete_propagates_when_enabled_and_goes_to_trash() {
    let ws = Workspace::new("del-propagate");
    let remote = MockRemote::new();
    let policy = SyncPolicy {
        propagate_remote_deletes: true,
        ..SyncPolicy::default()
    };
    let mut engine = ws.engine(&remote, policy);

    ws.write("trashme.txt", b"bye");
    engine.run_once().unwrap();

    remote.remove("trashme.txt");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.deleted_local, 1, "开启传播后应删除本地: {stats:?}");
    assert!(!ws.exists("trashme.txt"), "原路径应消失");
    // 但进的是回收站，仍可恢复
    let trash_root = ws.root.join(".cloudmount/trash");
    let mut found = false;
    for entry in walk(&trash_root) {
        if entry.ends_with("trashme.txt") {
            found = true;
            assert_eq!(std::fs::read(&entry).unwrap(), b"bye", "回收站内容应完整");
        }
    }
    assert!(found, "被删文件应可在回收站找回: {trash_root:?}");
}

#[test]
fn local_delete_not_propagated_by_default_restores_file() {
    let ws = Workspace::new("local-del");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("restore-me.txt", b"content");
    engine.run_once().unwrap();

    std::fs::remove_file(ws.path("restore-me.txt")).unwrap();
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.deleted_remote, 0, "默认不应删远端: {stats:?}");
    assert!(ws.exists("restore-me.txt"), "默认策略应把文件恢复回来");
    assert_eq!(ws.read("restore-me.txt"), b"content");
}

#[test]
fn change_probe_short_circuit_avoids_remote_requests() {
    let ws = Workspace::new("shortcircuit");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("a.txt", b"data");
    engine.run_once().unwrap(); // 轮 1：上传（产生变更）
    engine.run_once().unwrap(); // 轮 2：全量拉取并建立水位

    let list_before = remote.ops().list.load(std::sync::atomic::Ordering::SeqCst);
    let stats = engine.run_once().unwrap(); // 轮 3：应短路
    assert!(stats.remote_unchanged, "远端未变时应短路: {stats:?}");
    assert_eq!(
        remote.ops().list.load(std::sync::atomic::Ordering::SeqCst),
        list_before,
        "短路轮不应再 list 远端"
    );
    assert!(
        remote
            .ops()
            .change_probe
            .load(std::sync::atomic::Ordering::SeqCst)
            >= 3,
        "每轮仍应做一次廉价探测"
    );
}

#[test]
fn remote_change_after_short_circuit_is_detected() {
    let ws = Workspace::new("detect-after-sc");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("a.txt", b"1");
    engine.run_once().unwrap();
    engine.run_once().unwrap(); // 建立水位
    assert!(engine.run_once().unwrap().remote_unchanged, "此处应短路");

    // 远端出现新文件：探测值必然变化 → 必须被检测到
    remote.put_file("new-remote.txt", b"surprise");
    let stats = engine.run_once().unwrap();
    assert!(!stats.remote_unchanged, "远端变化后不应继续短路");
    assert_eq!(stats.downloaded, 1);
    assert_eq!(ws.read("new-remote.txt"), b"surprise");
}

#[test]
fn empty_dirs_are_mirrored_both_ways() {
    let ws = Workspace::new("emptydirs");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    // 本地空目录 → 远端建出
    std::fs::create_dir_all(ws.path("empty-local")).unwrap();
    engine.run_once().unwrap();
    assert!(remote.exists("empty-local"), "本地空目录应同步到远端");

    // 远端空目录 → 本地建出
    remote.put_dir("empty-remote");
    engine.run_once().unwrap();
    assert!(ws.exists("empty-remote"), "远端空目录应同步到本地");
}

#[test]
fn pause_prevents_any_action() {
    let ws = Workspace::new("paused");
    let remote = MockRemote::new();
    let cfg = SyncEngineConfig {
        watcher_backend: cloudmount_sync::watcher::WatcherBackend::Polling,
        ..SyncEngineConfig::new(&ws.root)
    };
    let paused = cfg.scheduler.paused.clone();
    paused.store(true, std::sync::atomic::Ordering::Relaxed);

    let mut engine = SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap();
    ws.write("never-uploaded.txt", b"x");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.uploaded, 0, "暂停时不应执行任何操作: {stats:?}");
    assert!(remote.read("never-uploaded.txt").is_none());

    // 恢复后正常工作
    paused.store(false, std::sync::atomic::Ordering::Relaxed);
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.uploaded, 1, "恢复后应正常上传: {stats:?}");
}

#[test]
fn download_uses_cache_on_second_copy() {
    let ws = Workspace::new("cache");
    let remote = MockRemote::new();
    remote.put_file("shared.bin", b"cached-body");
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    // 第一次下载：无缓存命中
    engine.run_once().unwrap();
    assert_eq!(ws.read("shared.bin"), b"cached-body");

    // 本地删除后（保守策略会重新下载）：应命中缓存，不再走 open_range
    std::fs::remove_file(ws.path("shared.bin")).unwrap();
    let downloads_before = remote
        .ops()
        .download
        .load(std::sync::atomic::Ordering::SeqCst);
    engine.run_once().unwrap();
    let downloads_after = remote
        .ops()
        .download
        .load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        downloads_before, downloads_after,
        "第二次获取同一内容应命中块缓存（远端 SHA1 相同）"
    );
    assert_eq!(ws.read("shared.bin"), b"cached-body");
}

#[test]
fn snapshot_survives_engine_restart() {
    let ws = Workspace::new("restart");
    let remote = MockRemote::new();
    ws.write("persisted.txt", b"data");
    {
        let mut engine = ws.engine(&remote, SyncPolicy::default());
        engine.run_once().unwrap();
    }
    // 新引擎实例（同一 db）→ 不应重复上传
    let uploads_before = remote
        .ops()
        .upload
        .load(std::sync::atomic::Ordering::SeqCst);
    let mut engine2 = ws.engine(&remote, SyncPolicy::default());
    let stats = engine2.run_once().unwrap();
    assert_eq!(stats.uploaded, 0, "重启后应复用快照: {stats:?}");
    assert_eq!(
        remote
            .ops()
            .upload
            .load(std::sync::atomic::Ordering::SeqCst),
        uploads_before
    );
}

#[test]
fn conflict_prefer_local_strategy_overwrites_remote() {
    let ws = Workspace::new("prefer-local");
    let remote = MockRemote::new();
    let policy = SyncPolicy {
        conflict: ConflictPolicy::PreferLocal,
        ..SyncPolicy::default()
    };
    let mut engine = ws.engine(&remote, policy);

    ws.write("f.txt", b"base");
    engine.run_once().unwrap();
    ws.write("f.txt", b"local-wins");
    remote.put_file("f.txt", b"remote-loses");

    let stats = engine.run_once().unwrap();
    assert_eq!(stats.conflicts, 1);
    assert_eq!(
        remote.read("f.txt").unwrap(),
        b"local-wins",
        "本地优先应把本地版本推到远端"
    );
}

#[test]
fn conflict_prefer_remote_strategy_overwrites_local() {
    let ws = Workspace::new("prefer-remote");
    let remote = MockRemote::new();
    let policy = SyncPolicy {
        conflict: ConflictPolicy::PreferRemote,
        ..SyncPolicy::default()
    };
    let mut engine = ws.engine(&remote, policy);

    ws.write("f.txt", b"base");
    engine.run_once().unwrap();
    ws.write("f.txt", b"local-loses");
    remote.put_file("f.txt", b"remote-wins");

    let stats = engine.run_once().unwrap();
    assert_eq!(stats.conflicts, 1);
    assert_eq!(ws.read("f.txt"), b"remote-wins", "远端优先应覆盖本地");
}

#[test]
fn remote_change_during_pause_is_never_skipped_after_resume() {
    // 回归测试：暂停期间远端发生变更，恢复后**必须**被处理。
    // 曾经的缺陷：暂停轮仍然推进了远端水位，恢复后首轮因"水位一致"而短路，
    // 那次远端变更会被永久跳过。
    let ws = Workspace::new("pause-watermark");
    let remote = MockRemote::new();
    let cfg = SyncEngineConfig {
        watcher_backend: cloudmount_sync::watcher::WatcherBackend::Polling,
        ..SyncEngineConfig::new(&ws.root)
    };
    let paused = cfg.scheduler.paused.clone();
    let mut engine = SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap();

    // 建立基线（此时无变更，水位可正常推进）
    engine.run_once().unwrap();
    engine.run_once().unwrap();

    // 暂停 → 远端发生变更 → 跑一轮（应什么都不做）
    paused.store(true, std::sync::atomic::Ordering::Relaxed);
    remote.put_file("while-paused.txt", b"must arrive");
    let paused_stats = engine.run_once().unwrap();
    assert_eq!(paused_stats.downloaded, 0, "暂停期间不应下载");
    assert!(!ws.exists("while-paused.txt"));

    // 恢复：必须把暂停期间发生的远端变更补上
    paused.store(false, std::sync::atomic::Ordering::Relaxed);
    let stats = engine.run_once().unwrap();
    assert!(
        ws.exists("while-paused.txt"),
        "恢复后必须处理暂停期间的远端变更（水位不得被暂停轮推进）: {stats:?}"
    );
    assert_eq!(ws.read("while-paused.txt"), b"must arrive");
}

// ============ M1.5：重命名检测（零传输路径） ============

#[test]
fn local_rename_moves_remote_without_duplication() {
    let ws = Workspace::new("rename-local");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("orig.txt", b"rename-me-content");
    engine.run_once().unwrap();
    assert!(remote.exists("orig.txt"));
    let uploads_before = remote
        .ops()
        .upload
        .load(std::sync::atomic::Ordering::SeqCst);

    // 本地改名
    std::fs::rename(ws.path("orig.txt"), ws.path("renamed.txt")).unwrap();
    let stats = engine.run_once().unwrap();

    // 远端应改名而非"恢复旧文件 + 上传新文件"
    assert!(
        !remote.exists("orig.txt"),
        "旧远端路径应消失: {:?}",
        remote.all_paths()
    );
    assert_eq!(
        remote.read("renamed.txt").unwrap(),
        b"rename-me-content",
        "内容应原样保留"
    );
    assert_eq!(
        remote
            .ops()
            .upload
            .load(std::sync::atomic::Ordering::SeqCst),
        uploads_before,
        "零传输改名：不应重新上传内容（stats: {stats:?}）"
    );

    // 本地也只有一个文件
    assert!(!ws.exists("orig.txt"));
    assert!(ws.exists("renamed.txt"));

    // 下一轮应稳定（改名已入账）
    let stats2 = engine.run_once().unwrap();
    assert!(!stats2.has_changes(), "改名入账后应稳定: {stats2:?}");
}

#[test]
fn remote_rename_renames_local_without_duplication() {
    let ws = Workspace::new("rename-remote");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    remote.put_file("old.txt", b"remote-rename-data");
    engine.run_once().unwrap();
    assert!(ws.exists("old.txt"));
    let downloads_before = remote
        .ops()
        .download
        .load(std::sync::atomic::Ordering::SeqCst);

    // 远端"改名"（服务端 rename 表现为：旧路径删除 + 新路径出现，内容不变）
    remote.remove("old.txt");
    remote.put_file("new.txt", b"remote-rename-data");

    engine.run_once().unwrap();

    // 本地应改名而非"恢复旧文件 + 下载新文件"
    assert!(!ws.exists("old.txt"), "旧本地路径应消失");
    assert_eq!(ws.read("new.txt"), b"remote-rename-data");
    assert_eq!(
        remote
            .ops()
            .download
            .load(std::sync::atomic::Ordering::SeqCst),
        downloads_before,
        "零传输改名：不应重新下载内容"
    );
}

#[test]
fn rename_with_content_change_falls_back_safely() {
    // 改名的同时内容也变了：哈希二次确认失败 → 回退为"上传新路径 + 删旧远端"
    let ws = Workspace::new("rename-modify");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    ws.write("a.txt", b"original");
    engine.run_once().unwrap();

    // 改名 + 修改内容（大小恰好相同，触发配对但哈希不匹配）
    std::fs::rename(ws.path("a.txt"), ws.path("b.txt")).unwrap();
    std::fs::write(ws.path("b.txt"), b"modified").unwrap();

    engine.run_once().unwrap();

    assert!(!remote.exists("a.txt"), "旧远端应被删除（回退路径）");
    assert_eq!(remote.read("b.txt").unwrap(), b"modified", "新内容应上传");
    assert_eq!(ws.read("b.txt"), b"modified");
}

// ============ M1.5：内容校验通道（关闭"同秒同大小"盲区） ============

#[test]
fn content_verification_catches_same_second_same_size_edit() {
    let ws = Workspace::new("verify-blindspot");
    let remote = MockRemote::new();
    // 每轮都校验、批量足够大
    let cfg = SyncEngineConfig {
        watcher_backend: cloudmount_sync::watcher::WatcherBackend::Polling,
        verify_every_cycles: 1,
        verify_batch_size: 100,
        scheduler: cloudmount_sync::queue::SchedulerConfig {
            concurrency: 1,
            ..Default::default()
        },
        ..SyncEngineConfig::new(&ws.root)
    };
    let mut engine = SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap();

    // 固定 mtime，制造"同秒同大小"盲区
    let fixed_mtime = filetime::FileTime::from_unix_time(1_000_000, 0);
    ws.write("blind.txt", b"AAAA");
    filetime::set_file_mtime(ws.path("blind.txt"), fixed_mtime).unwrap();
    engine.run_once().unwrap();
    assert_eq!(remote.read("blind.txt").unwrap(), b"AAAA");

    // 同秒内等大小覆盖（常规 (size, mtime) 检测必然漏检）
    ws.write("blind.txt", b"BBBB");
    filetime::set_file_mtime(ws.path("blind.txt"), fixed_mtime).unwrap();

    let stats = engine.run_once().unwrap();
    assert_eq!(
        stats.uploaded, 1,
        "内容校验通道应发现盲区修改并上传: {stats:?}"
    );
    assert_eq!(
        remote.read("blind.txt").unwrap(),
        b"BBBB",
        "远端应更新为新内容"
    );

    // 校验通过后入账，再跑一轮应稳定
    let stats2 = engine.run_once().unwrap();
    assert!(!stats2.has_changes(), "校验入账后应稳定: {stats2:?}");
}

#[test]
fn content_verification_can_be_disabled() {
    let ws = Workspace::new("verify-off");
    let remote = MockRemote::new();
    let cfg = SyncEngineConfig {
        watcher_backend: cloudmount_sync::watcher::WatcherBackend::Polling,
        verify_every_cycles: 0, // 关闭
        verify_batch_size: 0,
        scheduler: cloudmount_sync::queue::SchedulerConfig {
            concurrency: 1,
            ..Default::default()
        },
        ..SyncEngineConfig::new(&ws.root)
    };
    let mut engine = SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap();

    let fixed_mtime = filetime::FileTime::from_unix_time(1_000_000, 0);
    ws.write("blind.txt", b"AAAA");
    filetime::set_file_mtime(ws.path("blind.txt"), fixed_mtime).unwrap();
    engine.run_once().unwrap();

    ws.write("blind.txt", b"BBBB");
    filetime::set_file_mtime(ws.path("blind.txt"), fixed_mtime).unwrap();

    // 关闭校验 → 盲区修改不会被检测（这是文档化的行为边界）
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.uploaded, 0, "关闭校验后盲区修改不被检测: {stats:?}");
    assert_eq!(remote.read("blind.txt").unwrap(), b"AAAA");
}

// ============ M1.5：原生 watcher 与引擎协同（Windows/Linux） ============

#[test]
fn engine_works_with_native_watcher() {
    let ws = Workspace::new("native-watcher");
    let remote = MockRemote::new();
    // Auto：Windows 用 RdcWatcher / Linux 用 inotify / 其它回退轮询
    let cfg = SyncEngineConfig {
        scheduler: cloudmount_sync::queue::SchedulerConfig {
            concurrency: 1,
            ..Default::default()
        },
        ..SyncEngineConfig::new(&ws.root)
    };
    let mut engine = SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap();

    ws.write("native.txt", b"via-native-watcher");
    // 原生事件是异步送达的：有界重试等待（固定 sleep 在慢机器/CI 上会闪烁）
    let mut uploaded = 0;
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let stats = engine.run_once().unwrap();
        if stats.uploaded > 0 {
            uploaded = stats.uploaded;
            break;
        }
    }
    assert_eq!(uploaded, 1, "原生 watcher 应在 2s 内检测到新文件并上传");
    assert_eq!(remote.read("native.txt").unwrap(), b"via-native-watcher");

    let _ = std::fs::remove_dir_all(&ws.root);
}

/// 递归列出目录下的文件路径。
fn walk(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn remote_id_root_is_used_for_parent_resolution() {
    // 远端根不是 "0" 时也应正确工作（引擎允许配置 remote_root）
    let ws = Workspace::new("custom-root");
    let remote = MockRemote::new();
    let sub = remote.put_dir("cloudmount-root");
    let cfg = SyncEngineConfig {
        remote_root: RemoteId(sub),
        watcher_backend: cloudmount_sync::watcher::WatcherBackend::Polling,
        ..SyncEngineConfig::new(&ws.root)
    };
    let mut engine = SyncEngine::new(cfg, Arc::new(remote.clone())).unwrap();

    ws.write("in-root.txt", b"x");
    engine.run_once().unwrap();
    assert_eq!(
        remote.read("cloudmount-root/in-root.txt").unwrap(),
        b"x",
        "文件应上传到配置的远端根目录下"
    );
}

// ============ .git 防护：同步引擎视角 ============

#[test]
fn git_dir_is_fully_ignored_by_sync_engine() {
    let ws = Workspace::new("git-protect");
    let remote = MockRemote::new();
    let mut engine = ws.engine(&remote, SyncPolicy::default());

    // 本地建 .git（模拟用户 git init）+ worktree 文件
    ws.write("project/.git/index", b"git internal");
    ws.write("project/.git/index.lock", b"lock");
    ws.write("project/main.rs", b"fn main() {}");
    let stats = engine.run_once().unwrap();

    // .git 不可见：只有 main.rs 被上传
    assert_eq!(stats.uploaded, 1, "只应上传 worktree 文件: {stats:?}");
    assert!(remote.read("project/main.rs").is_some());
    assert!(
        remote.all_paths().iter().all(|p| !p.contains(".git")),
        "远端绝不应出现 .git: {:?}",
        remote.all_paths()
    );

    // 远端预置一个 .git（模拟其它工具传上去的）→ 引擎也不拉取
    remote.put_dir("evil/.git");
    remote.put_file("evil/.git/config", b"[core]");
    remote.put_file("evil/README.md", b"readme");
    let stats = engine.run_once().unwrap();
    assert_eq!(stats.downloaded, 1, "只应下载 README: {stats:?}");
    assert!(ws.exists("evil/README.md"));
    assert!(
        !ws.exists("evil/.git/config"),
        "远端的 .git 不应被拉取到本地"
    );
    let _ = std::fs::remove_dir_all(&ws.root);
}
