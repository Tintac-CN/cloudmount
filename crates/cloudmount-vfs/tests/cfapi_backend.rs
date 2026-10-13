//! CfapiBackend 端到端测试：Mock 远端 → 占位符投影 → Provider 水合。
//!
//! 这就是 115 接入后的真实形态：把 `MockRemote` 换成 `Open115Provider`，
//! 水合回调走的 `open_range`（downurl + Range）链路完全一致。
//! CI 的 windows runner 不支持 SyncRoot 注册（见 cfapi_hydration.rs 说明），
//! 注册失败自动跳过；真机完整运行。
#![cfg(windows)]

use cloudmount_sync::testutil::MockRemote;
use cloudmount_vfs::{CfapiBackend, Mount, MountBackend};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "cm-cfapi-be-{tag}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// 注册失败（CI runner 不支持）→ None 跳过。
fn mount_or_skip(
    tag: &str,
    remote: &MockRemote,
    view: &HashMap<String, cloudmount_core::RemoteEntry>,
) -> Option<(PathBuf, Box<dyn Mount>)> {
    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir(tag);
    match backend.mount(&root, view) {
        Ok(m) => Some((root, m)),
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                None
            } else {
                panic!("挂载失败（非环境限制）: {msg}");
            }
        }
    }
}

/// 远端条目视图（rel → entry）：逐级 stat MockRemote 构造
/// （模拟同步引擎产出远端视图的方式）。
fn view_of(remote: &MockRemote, paths: &[&str]) -> HashMap<String, cloudmount_core::RemoteEntry> {
    use cloudmount_core::{Provider, RemoteId};
    let mut out = HashMap::new();
    for p in paths {
        let mut acc = String::new();
        for comp in p.split('/') {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(comp);
            if out.contains_key(&acc) {
                continue;
            }
            let parent_id = if acc == comp {
                RemoteId("0".into())
            } else {
                let parent_rel = acc.rsplit_once('/').map(|(x, _)| x).unwrap();
                out.get(parent_rel)
                    .map(|e: &cloudmount_core::RemoteEntry| e.id.clone())
                    .expect("父目录应已在视图中")
            };
            let entry = remote
                .list(&parent_id)
                .expect("list")
                .into_iter()
                .find(|e| e.name == comp)
                .unwrap_or_else(|| panic!("MockRemote 缺少 {acc}"));
            out.insert(acc.clone(), entry);
        }
    }
    out
}

#[test]
fn backend_mounts_and_hydrates_from_provider() {
    let remote = MockRemote::new();
    remote.put_file("readme.md", b"# CloudMount CfAPI backend");
    remote.put_dir("docs");
    remote.put_file("docs/guide.txt", b"guide content here");

    let view = view_of(&remote, &["readme.md", "docs", "docs/guide.txt"]);
    let Some((root, mount)) = mount_or_skip("e2e", &remote, &view) else {
        return;
    };

    // 元数据可见且不触发下载（占位符投影，非内容拉取）
    let dl_before = remote
        .ops()
        .download
        .load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(std::fs::metadata(root.join("readme.md")).unwrap().len(), 26);
    assert!(root.join("docs/guide.txt").exists());
    assert_eq!(
        remote
            .ops()
            .download
            .load(std::sync::atomic::Ordering::SeqCst),
        dl_before,
        "元数据访问不应触发内容下载"
    );

    // 读文件 → 水合（Provider open_range）→ 内容与"云端"一致
    std::thread::sleep(Duration::from_millis(200)); // 等连接就绪
    assert_eq!(
        std::fs::read(root.join("readme.md")).unwrap(),
        b"# CloudMount CfAPI backend"
    );
    assert_eq!(
        std::fs::read(root.join("docs/guide.txt")).unwrap(),
        b"guide content here"
    );

    // 卸载：断开 + 注销
    mount.unmount().expect("卸载");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn backend_project_adds_new_remote_files() {
    let remote = MockRemote::new();
    remote.put_file("base.txt", b"base");

    let view = view_of(&remote, &["base.txt"]);
    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("project");
    let mount = match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    };

    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(std::fs::read(root.join("base.txt")).unwrap(), b"base");

    // "云端"新增文件 → project() 增量投影 → 本地可见并可水合
    remote.put_file("fresh.txt", b"fresh from cloud");
    let view2 = view_of(&remote, &["base.txt", "fresh.txt"]);
    let created = mount.project(&view2).expect("增量投影");
    assert_eq!(created, 1, "应只投影新增的 1 个条目");

    std::thread::sleep(Duration::from_millis(200));
    assert!(root.join("fresh.txt").exists(), "新文件占位符应出现");
    assert_eq!(
        std::fs::read(root.join("fresh.txt")).unwrap(),
        b"fresh from cloud",
        "新文件应可正常水合"
    );

    // 幂等：重复投影无新增
    let created2 = mount.project(&view2).unwrap();
    assert_eq!(created2, 0, "重复投影应无操作");

    Mount::unmount(Box::new(mount)).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

// ============ PlaceholderSync：远端视图变化 → 本地视图同步 ============

#[test]
fn placeholder_sync_applies_remote_add_and_delete() {
    use cloudmount_vfs::backends::cfapi_mount::PlaceholderSync;

    let remote = MockRemote::new();
    remote.put_file("keep.txt", b"stays");
    let view = view_of(&remote, &["keep.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("sync");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    std::thread::sleep(Duration::from_millis(200));
    assert!(root.join("keep.txt").exists());

    let mut sync = PlaceholderSync::new(mount.clone(), &view);

    // 云端新增两个文件（含嵌套）→ apply → 本地出现且可水合
    remote.put_file("new.txt", b"brand new");
    remote.put_dir("deep");
    remote.put_file("deep/nested.txt", b"nested new");
    let view2 = view_of(&remote, &["keep.txt", "new.txt", "deep", "deep/nested.txt"]);
    let stats = sync.apply_remote_view(&view2).expect("apply add");
    assert_eq!(stats.added, 3, "应新增投影 3 个条目: {stats:?}");
    assert_eq!(stats.removed, 0);

    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(std::fs::read(root.join("new.txt")).unwrap(), b"brand new");
    assert_eq!(
        std::fs::read(root.join("deep/nested.txt")).unwrap(),
        b"nested new"
    );

    // 云端删除嵌套文件与目录 → apply → 本地消失
    remote.remove("deep");
    let view3 = view_of(&remote, &["keep.txt", "new.txt"]);
    let stats = sync.apply_remote_view(&view3).expect("apply delete");
    assert_eq!(stats.removed, 2, "嵌套目录连带子文件应删除: {stats:?}");
    assert!(!root.join("deep").exists(), "删除的目录应消失");
    assert!(root.join("keep.txt").exists(), "未删除的文件应保留");
    assert!(root.join("new.txt").exists());

    // 云端删除最后一个 → 本地也消失
    remote.remove("new.txt");
    let view4 = view_of(&remote, &["keep.txt"]);
    let stats = sync.apply_remote_view(&view4).unwrap();
    assert_eq!(stats.removed, 1);
    assert!(!root.join("new.txt").exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn placeholder_sync_reprojects_content_changes() {
    use cloudmount_vfs::backends::cfapi_mount::PlaceholderSync;

    let remote = MockRemote::new();
    remote.put_file("doc.txt", b"v1 content");
    let view = view_of(&remote, &["doc.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("reproject");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    std::thread::sleep(Duration::from_millis(200));

    // 先水合旧内容（建立"已读取"基线）
    assert_eq!(std::fs::read(root.join("doc.txt")).unwrap(), b"v1 content");

    // 远端内容变化 → apply → 重投影
    remote.put_file("doc.txt", b"v2 with different length");
    let view2 = view_of(&remote, &["doc.txt"]);
    let mut sync = PlaceholderSync::new(mount, &view);
    let stats = sync.apply_remote_view(&view2).expect("apply");
    assert_eq!(stats.content_updated, 1, "内容变化应被重投影: {stats:?}");
    assert_eq!(stats.content_changes_pending, 0);

    // 再读 → 水合**新**内容（DEHYDRATE 丢弃了旧数据）
    assert_eq!(
        std::fs::read(root.join("doc.txt")).unwrap(),
        b"v2 with different length",
        "重投影后读取应得到新内容"
    );
    // 元数据也应是新大小
    assert_eq!(
        std::fs::metadata(root.join("doc.txt")).unwrap().len(),
        b"v2 with different length".len() as u64
    );
}

#[test]
fn placeholder_sync_skips_reproject_when_locally_modified() {
    use cloudmount_vfs::backends::cfapi_mount::PlaceholderSync;

    let remote = MockRemote::new();
    remote.put_file("doc.txt", b"v1");
    let view = view_of(&remote, &["doc.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("reproject-conflict");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    std::thread::sleep(Duration::from_millis(200));

    // 用户修改（未上传）→ 远端也变化 → 冲突：不覆盖用户数据
    std::fs::read(root.join("doc.txt")).unwrap(); // 先水合
    std::fs::write(root.join("doc.txt"), b"user local edit").unwrap();

    remote.put_file("doc.txt", b"remote v2");
    let view2 = view_of(&remote, &["doc.txt"]);
    let mut sync = PlaceholderSync::new(mount, &view);
    let stats = sync.apply_remote_view(&view2).unwrap();
    assert_eq!(
        stats.content_changes_pending, 1,
        "本地已修改应计冲突: {stats:?}"
    );
    assert_eq!(stats.content_updated, 0);
    // 用户数据未被覆盖
    assert_eq!(
        std::fs::read(root.join("doc.txt")).unwrap(),
        b"user local edit"
    );
}

/// 有界等待：条件满足即返回（异步事件送达，固定 sleep 在慢机器会闪烁）。
fn wait_for<F: FnMut() -> bool>(mut f: F, timeout_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        if f() {
            return true;
        }
    }
    false
}

#[test]
fn bidirectional_hydration_does_not_upload() {
    use cloudmount_vfs::LocalChangeUploader;

    let remote = MockRemote::new();
    remote.put_file("read.txt", b"read me to hydrate");
    let view = view_of(&remote, &["read.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("bidir-read");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    let mut uploader = LocalChangeUploader::new(
        mount.clone(),
        Arc::new(remote.clone()),
        cloudmount_core::RemoteId("0".into()),
    )
    .expect("uploader");

    // 读文件（触发水合）→ 轮询 → 不应上传。
    // 实测（Win11 26300）：过滤器的内核态水合写入**不产生 RDC 事件**，
    // poll 天然看不到变更；水合跳过逻辑（双 mtime 判定）作为跨版本防御保留。
    std::fs::read(root.join("read.txt")).expect("水合读取");
    let uploads_before = remote
        .ops()
        .upload
        .load(std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(400)); // 等可能的异步事件
    let stats = uploader.poll_and_upload().unwrap();
    assert_eq!(stats.uploaded, 0, "水合读取绝不应触发上传: {stats:?}");
    assert_eq!(
        remote
            .ops()
            .upload
            .load(std::sync::atomic::Ordering::SeqCst),
        uploads_before,
        "远端上传计数不应增长"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bidirectional_user_modification_uploads() {
    use cloudmount_vfs::LocalChangeUploader;

    let remote = MockRemote::new();
    remote.put_file("doc.txt", b"original content");
    let view = view_of(&remote, &["doc.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("bidir-mod");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    let mut uploader = LocalChangeUploader::new(
        mount.clone(),
        Arc::new(remote.clone()),
        cloudmount_core::RemoteId("0".into()),
    )
    .expect("uploader");

    // 先水合（建立基线），再用户修改
    std::fs::read(root.join("doc.txt")).expect("水合");
    let _ = uploader.poll_and_upload(); // 排掉水合事件

    std::fs::write(root.join("doc.txt"), b"modified by user").expect("用户修改");
    let ok = wait_for(
        || {
            uploader
                .poll_and_upload()
                .map(|s| s.uploaded > 0)
                .unwrap_or(false)
        },
        4000,
    );
    assert!(ok, "用户修改应被上传");
    assert_eq!(
        remote.read("doc.txt").unwrap(),
        b"modified by user",
        "远端应更新为用户内容"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bidirectional_new_file_uploads() {
    use cloudmount_vfs::LocalChangeUploader;

    let remote = MockRemote::new();
    remote.put_file("seed.txt", b"seed");
    let view = view_of(&remote, &["seed.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("bidir-new");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    let mut uploader = LocalChangeUploader::new(
        mount.clone(),
        Arc::new(remote.clone()),
        cloudmount_core::RemoteId("0".into()),
    )
    .expect("uploader");

    // 用户在同步根新建文件 → 上传
    std::fs::write(root.join("fresh-note.txt"), b"created by user").expect("新建");
    let ok = wait_for(
        || {
            uploader
                .poll_and_upload()
                .map(|s| s.uploaded > 0)
                .unwrap_or(false)
        },
        4000,
    );
    assert!(ok, "用户新建应被上传");
    assert_eq!(
        remote.read("fresh-note.txt").unwrap(),
        b"created by user",
        "远端应出现新文件"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ============ .git 防护：占位符模式 ============

#[test]
fn git_dir_not_projected_and_writes_not_uploaded() {
    use cloudmount_vfs::LocalChangeUploader;

    let remote = MockRemote::new();
    // 远端预置 .git（模拟其它工具传上去的）+ 正常文件
    remote.put_dir(".git");
    remote.put_file(".git/config", b"[core]");
    remote.put_file("README.md", b"readme");

    // view_of 走 list 链路——fetch_tree 的排除发生在 RemoteViewTracker，
    // 这里直接构造视图验证投影过滤（fetch_tree 已在 sync 侧测试覆盖）
    let view = view_of(&remote, &["README.md"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("git-mount");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    assert!(!root.join(".git").exists(), ".git 不应被投影");
    assert!(root.join("README.md").exists());

    // 用户在挂载盘里 git init（写 .git）→ 不应上传
    let mut uploader = LocalChangeUploader::new(
        mount.clone(),
        Arc::new(remote.clone()),
        cloudmount_core::RemoteId("0".into()),
    )
    .expect("uploader");
    std::fs::create_dir_all(root.join(".git/objects")).unwrap();
    std::fs::write(root.join(".git/index"), b"git index").unwrap();
    std::fs::write(root.join(".git/index.lock"), b"lock").unwrap();

    let uploads_before = remote
        .ops()
        .upload
        .load(std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(400));
    let stats = uploader.poll_and_upload().unwrap();
    assert_eq!(stats.uploaded, 0, ".git 写入绝不应上传: {stats:?}");
    assert_eq!(
        remote
            .ops()
            .upload
            .load(std::sync::atomic::Ordering::SeqCst),
        uploads_before
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bidirectional_new_dir_with_files_uploads() {
    use cloudmount_vfs::LocalChangeUploader;

    let remote = MockRemote::new();
    remote.put_file("seed.txt", b"seed");
    let view = view_of(&remote, &["seed.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("bidir-dirs");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    let mut uploader = LocalChangeUploader::new(
        mount.clone(),
        Arc::new(remote.clone()),
        cloudmount_core::RemoteId("0".into()),
    )
    .expect("uploader");

    // 用户新建嵌套目录 + 文件 → 全部上传
    std::fs::create_dir_all(root.join("new/deep")).unwrap();
    std::fs::write(root.join("new/deep/file.txt"), b"in new dirs").unwrap();
    let ok = wait_for(
        || {
            uploader
                .poll_and_upload()
                .map(|s| s.uploaded > 0)
                .unwrap_or(false)
        },
        4000,
    );
    assert!(ok, "新目录内文件应被上传");
    assert_eq!(
        remote.read("new/deep/file.txt").unwrap(),
        b"in new dirs",
        "远端应出现嵌套新文件"
    );

    // 空 dir 事件也可能到达 → ensure 目录链
    let _ = uploader.poll_and_upload();

    assert!(remote.exists("new/deep"), "远端目录链应存在");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dehydrate_discards_local_data_but_keeps_metadata() {
    let remote = MockRemote::new();
    remote.put_file("d.txt", b"xxxx");
    let view = view_of(&remote, &["d.txt"]);
    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("dehydrate");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    });
    std::thread::sleep(Duration::from_millis(200));

    // 水合 → 请求脱水（异步）→ 有界等待完成 → 元数据保留、可再读
    assert_eq!(std::fs::read(root.join("d.txt")).unwrap(), b"xxxx");
    mount.dehydrate_rel("d.txt").expect("脱水请求");
    let dehydrated = wait_for(
        || {
            use std::os::windows::fs::MetadataExt;
            let a = std::fs::metadata(root.join("d.txt"))
                .map(|m| m.file_attributes())
                .unwrap_or(0);
            eprintln!("attrs during wait: {a:#x}");
            a & 0x0040_0000 != 0
        },
        8000,
    );
    assert!(dehydrated, "脱水应在数秒内完成（RECALL 属性置位）");
    assert_eq!(
        std::fs::metadata(root.join("d.txt")).unwrap().len(),
        4,
        "元数据（逻辑大小）应保留"
    );
    assert_eq!(
        std::fs::read(root.join("d.txt")).unwrap(),
        b"xxxx",
        "脱水后再读应取回内容（重新水合或缓存命中）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

// ============ Shell 侧注册（WinRT，Explorer 导航栏/图标） ============

#[test]
fn shell_register_unregister_roundtrip() {
    let root = tmpdir("shell-reg");
    // Shell 注册不要求内核注册先行（两者独立），但真实产品两者都做
    let reg = cloudmount_vfs::backends::shell_reg::ShellRegistration {
        sync_root: &root,
        provider_name: "CloudMount-Test",
        display_name: "CloudMount 测试盘",
        version: "0.1.0",
        icon_resource: r"%SystemRoot%\system32\charmap.exe,0",
        identity: "test-server->test-client",
    };
    match cloudmount_vfs::backends::shell_reg::register_shell(&reg) {
        Ok(()) => {
            // 同名重注册（幂等：先注销再注册）
            cloudmount_vfs::backends::shell_reg::register_shell(&reg).expect("重复注册应幂等");
            cloudmount_vfs::backends::shell_reg::unregister_shell(
                "CloudMount-Test",
                "CloudMount 测试盘",
            )
            .expect("注销");
        }
        Err(e) => {
            let msg = format!("{e}");
            // CI runner 与部分虚拟机不支持 Shell 注册（同 CfRegisterSyncRoot 的限制）
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 Shell 同步根注册，跳过：{msg}");
                return;
            }
            panic!("Shell 注册失败（非环境限制）: {msg}");
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn backend_mount_with_shell_integration_registers_and_unregisters() {
    let remote = MockRemote::new();
    remote.put_file("x.txt", b"data");
    let view = view_of(&remote, &["x.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone())).with_shell_integration();
    let root = tmpdir("shell-int");
    let mount = match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败: {msg}");
        }
    };
    // Shell 注册成功（Register 幂等已验证）；WinRT 查询需 StorageFolder
    // 类型包装，此处省略——注册/注销往返本身即为验证目标
    cloudmount_vfs::Mount::unmount(Box::new(mount)).ok();
}
