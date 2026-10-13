//! 控制通道 + 脱水真机集成测试（Windows + cldflt）。
//!
//! 链路：挂载（MockRemote 云端）→ 读文件水合 → 控制通道 DEHYDRATE
//! （CfSetPinState UNPINNED → NOTIFY_DEHYDRATE 回调 ACK → 平台异步脱水）
//! → 占位符回到"仅元数据"（RECALL_ON_DATA_ACCESS 属性）→ 再读重新水合。
//!
//! 这就是 CLI `cloudmount dehydrate` 的真实路径：CLI 独立进程经控制通道
//! 转发给持有 CfAPI 连接的 daemon（本测试进程即 provider，与 daemon 同构）。
//!
//! CI 的 windows runner 不支持 SyncRoot 注册（见 cfapi_hydration.rs 说明），
//! 注册失败自动跳过；真机完整运行。
#![cfg(windows)]

use cloudmount_sync::testutil::MockRemote;
use cloudmount_vfs::control::{self, ControlServer};
use cloudmount_vfs::{CfapiBackend, Mount};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS（未水合占位符的标志；读取该属性
/// 本身不触发水合——文档保证）。
const RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

fn file_attrs(path: &std::path::Path) -> u32 {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(path).unwrap().file_attributes()
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

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "cm-ctl-{tag}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// 远端条目视图（rel → entry）：逐级 stat MockRemote 构造。
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

/// 与 daemon 相同形态的控制 handler（main.rs::dehydrate_via_mount 的镜像）。
fn dehydrate_handler(
    mount: &Arc<cloudmount_vfs::CfapiMount>,
    root_canon: &std::path::Path,
    cmd: &str,
    arg: &str,
) -> String {
    match cmd {
        "PING" => "OK".into(),
        "DEHYDRATE" => {
            let canon = PathBuf::from(arg)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(arg));
            match canon.strip_prefix(root_canon) {
                Ok(rel) => match mount.dehydrate_rel(&rel.to_string_lossy()) {
                    Ok(()) => "OK".into(),
                    Err(e) => format!("ERR {e}"),
                },
                Err(_) => format!("ERR 路径不在挂载根内: {arg}"),
            }
        }
        _ => "ERR unknown-command".into(),
    }
}

#[test]
fn control_channel_dehydrates_and_rehydrates() {
    let content: &[u8] = b"hydrated then dehydrated content";
    let remote = MockRemote::new();
    remote.put_file("big.bin", content);
    let view = view_of(&remote, &["big.bin"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("e2e");
    let mount = Arc::new(match backend.mount_concrete(&root, &view) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x80070490") {
                eprintln!("当前环境不支持 SyncRoot 注册，跳过：{msg}");
                return;
            }
            panic!("挂载失败（非环境限制）: {msg}");
        }
    });
    std::thread::sleep(Duration::from_millis(200));

    let file = root.join("big.bin");

    // 读 → 水合 → RECALL 属性清除（水合完成的证据）
    assert_eq!(std::fs::read(&file).unwrap(), content);
    assert_eq!(
        file_attrs(&file) & RECALL_ON_DATA_ACCESS,
        0,
        "水合后 RECALL_ON_DATA_ACCESS 应清除"
    );
    let downloads_after_hydrate = remote
        .ops()
        .download
        .load(std::sync::atomic::Ordering::SeqCst);
    assert!(downloads_after_hydrate > 0, "首次读取应触发云端拉取");

    // 控制通道启动（端口文件在独立临时目录，避免与真实 daemon 冲突）
    let port_dir = tmpdir("ctl");
    std::fs::create_dir_all(&port_dir).unwrap();
    let port_file = port_dir.join("control.port");
    let mount_for_ctl = mount.clone();
    let root_canon = root.canonicalize().unwrap();
    let server = ControlServer::start(&port_file, &root, move |cmd, arg| {
        dehydrate_handler(&mount_for_ctl, &root_canon, cmd, arg)
    })
    .expect("控制服务启动");

    // PING 健康检查
    assert!(control::send(&port_file, "PING", "")
        .unwrap()
        .starts_with("OK"));

    // 根外路径被拒绝（校验防御）
    let outside = std::env::temp_dir().join(format!("cm-ctl-outside-{}.txt", std::process::id()));
    std::fs::write(&outside, b"x").unwrap();
    let resp = control::send(&port_file, "DEHYDRATE", &outside.to_string_lossy()).unwrap();
    assert!(resp.starts_with("ERR"), "根外路径应被拒绝: {resp}");

    // 正常脱水：CfSetPinState(UNPINNED) → 平台异步脱水（NOTIFY_DEHYDRATE
    // → 本进程回调 ACK → 文件空闲时释放数据）。
    // 实测（Win11 26300）语义（勿用错误断言）：
    // ① API 返回成功 ≠ 脱水完成（官方文档明示）——须有界等待；
    // ② 脱水完成后属性回到新占位符形态：RECALL_ON_DATA_ACCESS 置位；
    // ③ 脱水不清系统文件缓存：再读可能命中缓存而不触发 FETCH_DATA
    //    （OneDrive 同此行为；数据正确性与磁盘释放均不受影响）。
    //    GetCompressedFileSizeW 在占位符上不可用（过滤器拒绝其内部打开）。
    let resp = control::send(&port_file, "DEHYDRATE", &file.to_string_lossy()).unwrap();
    assert_eq!(resp, "OK", "脱水请求应被接受: {resp}");
    assert_eq!(
        std::fs::metadata(&file).unwrap().len(),
        content.len() as u64,
        "元数据（逻辑大小）应保留"
    );
    let dehydrated = wait_for(|| file_attrs(&file) & RECALL_ON_DATA_ACCESS != 0, 8000);
    assert!(
        dehydrated,
        "脱水应在数秒内完成（RECALL_ON_DATA_ACCESS 回到置位态）: attrs={:#x}",
        file_attrs(&file)
    );

    // 再读 → 内容一致（缓存命中或重新水合，两者都是正确语义）
    assert_eq!(std::fs::read(&file).unwrap(), content);
    assert_eq!(
        file_attrs(&file) & RECALL_ON_DATA_ACCESS,
        0,
        "读取后不应处于未水合状态"
    );

    // 停止 → 端口文件删除 → 卸载
    server.stop();
    assert!(!port_file.exists(), "停止后端口文件应删除");
    if let Ok(m) = Arc::try_unwrap(mount) {
        Mount::unmount(Box::new(m)).ok();
    }
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&root);
}

/// 用户创建的文件（普通文件，非占位符）→ 上传 → 脱水（转换占位符）→
/// 再读重新水合。这是"释放空间"对本地新建文件的完整语义。
#[test]
fn dehydrate_user_created_file_after_upload() {
    use cloudmount_vfs::LocalChangeUploader;

    let remote = MockRemote::new();
    remote.put_file("seed.txt", b"seed");
    let view = view_of(&remote, &["seed.txt"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("user-file");
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

    // 用户创建文件 → 等待上传（同步水位记录后脱水才被放行）
    let content: &[u8] = b"user created this locally";
    std::fs::write(root.join("user.bin"), content).unwrap();
    let uploaded = wait_for(
        || {
            uploader
                .poll_and_upload()
                .map(|s| s.uploaded > 0)
                .unwrap_or(false)
        },
        4000,
    );
    assert!(uploaded, "用户文件应被上传");
    assert_eq!(remote.read("user.bin").unwrap(), content);

    // 脱水：普通文件路径（CfConvertToPlaceholder + DEHYDRATE）
    mount.dehydrate_rel("user.bin").expect("脱水（转换路径）");

    // 转换后是脱水的占位符（provider 视角 RECALL 置位）
    let dehydrated = wait_for(
        || file_attrs(&root.join("user.bin")) & RECALL_ON_DATA_ACCESS != 0,
        8000,
    );
    assert!(
        dehydrated,
        "转换+脱水应在数秒内完成: attrs={:#x}",
        file_attrs(&root.join("user.bin"))
    );

    // 再读 → 内容一致（重新水合或缓存命中）
    assert_eq!(std::fs::read(root.join("user.bin")).unwrap(), content);

    if let Ok(m) = Arc::try_unwrap(mount) {
        Mount::unmount(Box::new(m)).ok();
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dehydrate_rel_rejects_directory() {
    let remote = MockRemote::new();
    remote.put_file("seed.txt", b"seed");
    remote.put_dir("adir");
    let view = view_of(&remote, &["seed.txt", "adir"]);

    let backend = CfapiBackend::new(Arc::new(remote.clone()));
    let root = tmpdir("dir-reject");
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

    // 目录脱水应被拒绝（与 CLI 行为一致：仅支持文件）
    let err = mount.dehydrate_rel("adir").expect_err("目录应报错");
    assert!(format!("{err}").contains("仅支持文件"), "错误信息: {err}");

    if let Ok(m) = Arc::try_unwrap(mount) {
        Mount::unmount(Box::new(m)).ok();
    }
    let _ = std::fs::remove_dir_all(&root);
}
