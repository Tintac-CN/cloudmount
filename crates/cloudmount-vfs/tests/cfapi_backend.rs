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
