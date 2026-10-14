//! CfAPI 挂载后端的**真机集成测试**（Windows + cldflt.sys）。
//!
//! 金标准链路：注册同步根 → 创建占位符 → 连接回调 → 读文件触发 FETCH_DATA
//! → 水合函数取回内容 → 验证读到的内容与"云端"一致。
//!
//! 环境要求：本地 NTFS 卷 + cldflt 可用（Win10 1709+ 均内置）。
//!
//! **环境限制**：GitHub Actions 的 windows runner 虚拟机不支持 SyncRoot 注册
//! （`CfRegisterSyncRoot` 返回 `0x80070490 ELEMENT_NOT_FOUND`），这些测试在
//! 不支持的环境下自动跳过；已在真实 Windows 11 机器上验证通过（5/5）。
#![cfg(windows)]

use cloudmount_vfs::cfapi::{self, PlaceholderSpec};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

/// 每个测试独立的同步根目录。
fn sync_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cm-cfapi-{tag}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// 创建并注册同步根；当前环境不支持时返回 None（测试应跳过）。
fn setup_or_skip(tag: &str) -> Option<PathBuf> {
    let root = sync_root(tag);
    match cfapi::register(&root, "CloudMount-Test", "0.1") {
        Ok(_) => Some(root),
        Err(e) => {
            eprintln!("当前环境不支持 SyncRoot 注册（{e}），跳过真机测试");
            let _ = std::fs::remove_dir_all(&root);
            None
        }
    }
}

/// 测试结束清理（注销 + 删除目录）。
fn cleanup(root: &PathBuf) {
    let _ = cfapi::unregister(root);
    // 占位符目录直接删除可能被过滤器拒绝，重试几次
    for _ in 0..3 {
        if std::fs::remove_dir_all(root).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn platform_check_or_skip() {
    // 环境自检：cldflt 不可用时打印并跳过其余断言（CI 环境差异容错）
    if !cfapi::platform_available() {
        eprintln!("cldflt 不可用，跳过 CfAPI 真机测试");
    }
}

/// **万级目录投影基准**（真机）：10,000 文件 + 100 目录的投影耗时。
///
/// CfCreatePlaceholders 单次调用可带任意数量条目（内核批量建占位符），
/// 每父目录一次调用。基准断言 < 30s（CI 慢机器余量；本机实测见输出）。
#[test]
fn project_10k_entries_benchmark() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("bench-10k") else {
        return;
    };

    // 构造 100 目录 × 100 文件
    let mut specs: Vec<cfapi::PlaceholderSpec> = Vec::with_capacity(10_100);
    for d in 0..100 {
        specs.push(cfapi::PlaceholderSpec::dir(
            format!("d{d:03}"),
            1_700_000_000,
        ));
        for f in 0..100 {
            specs.push(cfapi::PlaceholderSpec::file(
                format!("d{d:03}/f{f:03}.dat"),
                1024,
                1_700_000_001,
            ));
        }
    }
    let start = std::time::Instant::now();
    let created = cfapi::create_placeholders_ordered(&root, &specs).expect("万级投影");
    let elapsed = start.elapsed();
    eprintln!(
        "万级投影基准：{} / {} 条目，耗时 {:?}（{:.0} 条/秒）",
        created,
        specs.len(),
        elapsed,
        specs.len() as f64 / elapsed.as_secs_f64()
    );
    assert_eq!(created, 10_100, "全部条目应创建");
    assert!(elapsed.as_secs() < 30, "万级投影应 < 30s，实际 {elapsed:?}");

    cleanup(&root);
}

#[test]
fn register_unregister_roundtrip() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("roundtrip") else {
        return;
    };
    let reg = cfapi::register(&root, "CloudMount-Test", "0.1").expect("重复注册应幂等");
    assert_eq!(reg.path, root);
    assert!(root.exists(), "注册应创建同步根目录");

    // 重复注册（幂等：先注销再注册）
    cfapi::register(&root, "CloudMount-Test", "0.1").expect("重复注册应幂等");

    cfapi::unregister(&root).expect("注销");
    cleanup(&root);
}

#[test]
fn create_placeholders_files_and_dirs() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("placeholders") else {
        return;
    };

    let created = cfapi::create_placeholders_ordered(
        &root,
        &[
            PlaceholderSpec::dir("docs", 1_700_000_000),
            PlaceholderSpec::file("docs/a.txt", 100, 1_700_000_001),
            PlaceholderSpec::file("root.md", 2048, 1_700_000_002),
        ],
    )
    .unwrap();

    assert_eq!(created, 3, "三个占位符应全部创建");

    // 元数据可见且不触发水合（metadata 不读数据）
    let meta = std::fs::metadata(root.join("docs/a.txt")).unwrap();
    assert_eq!(meta.len(), 100, "占位符大小应为远端大小");
    assert!(meta.is_file());
    assert!(std::fs::metadata(root.join("docs")).unwrap().is_dir());
    assert_eq!(std::fs::metadata(root.join("root.md")).unwrap().len(), 2048);

    cleanup(&root);
}

/// **金标准测试**：完整水合链路。
///
/// 占位符无本地数据 → 读文件触发 FETCH_DATA → 水合函数返回"云端"内容 →
/// 内容写入占位符 → RetrieveData 完成 → 读取返回真实内容。
#[test]
fn golden_hydration_flow() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("golden") else {
        return;
    };

    let cloud_content: &[u8] = b"CloudMount golden hydration test - 115 cloud data";
    cfapi::create_placeholders_ordered(
        &root,
        &[PlaceholderSpec::file(
            "golden.txt",
            cloud_content.len() as u64,
            1_700_000_000,
        )],
    )
    .unwrap();

    // 连接：水合函数模拟"云端"（按范围切片）
    let mut expected = Vec::new();
    expected.extend_from_slice(cloud_content);
    let hydrate_data = expected.clone();
    let calls_ref = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let calls_ref2 = calls_ref.clone();
    let conn = cfapi::connect(
        &root,
        Box::new(move |rel, offset, len| {
            calls_ref
                .lock()
                .unwrap()
                .push((rel.to_string(), offset, len));
            let end = (offset as usize + len as usize).min(hydrate_data.len());
            Ok(hydrate_data[offset as usize..end].to_vec())
        }),
    )
    .expect("连接同步根");

    // 等待过滤器完全建立连接（连接是异步生效的）
    std::thread::sleep(Duration::from_millis(300));

    // **读文件 → 触发 FETCH_DATA → 水合 → 拿到内容**
    let got = std::fs::read(root.join("golden.txt")).expect("读取占位符应触发水合并成功");

    assert_eq!(
        got,
        expected,
        "读到的内容必须与'云端'一致（读取长度 {}）",
        got.len()
    );

    let calls = calls_ref2.lock().unwrap();
    assert!(!calls.is_empty(), "读文件必须触发至少一次水合回调");
    assert_eq!(calls[0].0, "golden.txt", "水合回调应收到占位符的相对路径");
    assert_eq!(calls[0].1, 0, "首次读取从偏移 0 开始");

    // 连接仍持有：显式保留至测试结束
    let _ = conn;
    cleanup(&root);
}

#[test]
fn hydration_of_subdirectory_file() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("golden-sub") else {
        return;
    };

    let content = b"nested hydration works!";
    cfapi::create_placeholders_ordered(
        &root,
        &[
            PlaceholderSpec::dir("docs/2026", 1_700_000_000),
            PlaceholderSpec::file("docs/2026/report.txt", content.len() as u64, 1_700_000_001),
        ],
    )
    .unwrap();

    let conn = cfapi::connect(
        &root,
        Box::new(|rel, offset, len| {
            assert_eq!(rel, "docs/2026/report.txt");
            let content = b"nested hydration works!";
            let end = (offset as usize + len as usize).min(content.len());
            Ok(content[offset as usize..end].to_vec())
        }),
    )
    .unwrap();

    std::thread::sleep(Duration::from_millis(300));

    let got = std::fs::read(root.join("docs/2026/report.txt")).unwrap();
    assert_eq!(got, b"nested hydration works!");

    let _ = conn;
    cleanup(&root);
}

/// 确定性伪随机字节（xorshift64*，测试零依赖）。
fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut s = seed | 1;
    (0..len)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 21) as u8
        })
        .collect()
}

/// **U9 骨架（T-01 集成腿）**：多块规模占位符的 FETCH_DATA 分块水合拼接一致。
///
/// 2 MiB + 奇数尾：读取全程必然跨多个 FETCH_DATA 块；端到端内容逐字节一致。
/// 红测判别在单元腿（`pump_chunks` 读取序列，`cargo test -p cloudmount-vfs
/// --lib`）；本用例为真机端到端回归——实现阶段随 HydrateFn 契约切换
/// 机械适配 ChunkSink 形态（同 `golden_hydration_flow` 的调用点）。
#[test]
fn fetch_data_multiblock_hydration_content_identical() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("u9-multiblock") else {
        return;
    };

    let size = 2 * 1024 * 1024 + 12345;
    let expected = pseudo_random(size, 0xB10B);
    cfapi::create_placeholders_ordered(
        &root,
        &[PlaceholderSpec::file("big.bin", size as u64, 1_700_000_000)],
    )
    .unwrap();

    let hydrate_data = expected.clone();
    let conn = cfapi::connect(
        &root,
        Box::new(move |rel, offset, len| {
            assert_eq!(rel, "big.bin");
            let start = (offset as usize).min(hydrate_data.len());
            let end = (offset as usize + len as usize).min(hydrate_data.len());
            Ok(hydrate_data[start..end].to_vec())
        }),
    )
    .expect("连接同步根");

    // 等待过滤器完全建立连接（连接是异步生效的）
    std::thread::sleep(Duration::from_millis(300));

    let got = std::fs::read(root.join("big.bin")).expect("多块水合读取");
    assert_eq!(got.len(), expected.len(), "读取长度应等于远端大小");
    assert_eq!(got, expected, "多块 FETCH_DATA 拼接必须与云端逐字节一致");

    let _ = conn;
    cleanup(&root);
}

/// **U9 骨架（T-02 集成腿，占位——真机验证）**：水合中途取消后系统一致。
///
/// 真机语义：读取方提前关闭句柄 → 过滤器对挂起 FETCH_DATA 发 CANCEL
/// → 实现（ChunkSink 契约）逐块查 `cancelled` → Err 停止并丢弃 reader。
/// 当前口径：部分读取后取消，重新完整读取仍逐字节正确（无残留损坏）；
/// "逐块取消即停"的强断言由单元腿 T-02 守护，真机取消行为在实现阶段
/// 适配 ChunkSink 契约后于此补强。
#[test]
fn fetch_data_cancel_consistency_placeholder() {
    if !cfapi::platform_available() {
        return;
    }
    let Some(root) = setup_or_skip("u9-cancel") else {
        return;
    };

    let size = 8 * 1024 * 1024;
    let expected = pseudo_random(size, 0xCA4CE);
    cfapi::create_placeholders_ordered(
        &root,
        &[PlaceholderSpec::file("cancel.bin", size as u64, 1_700_000_000)],
    )
    .unwrap();

    let hydrate_data = expected.clone();
    let conn = cfapi::connect(
        &root,
        Box::new(move |rel, offset, len| {
            assert_eq!(rel, "cancel.bin");
            let start = (offset as usize).min(hydrate_data.len());
            let end = (offset as usize + len as usize).min(hydrate_data.len());
            Ok(hydrate_data[start..end].to_vec())
        }),
    )
    .expect("连接同步根");

    std::thread::sleep(Duration::from_millis(300));

    // 部分读取后提前关闭句柄（挂起传输被 CANCEL 的真机路径）
    {
        let mut f = std::fs::File::open(root.join("cancel.bin")).expect("打开占位符");
        let mut head = vec![0u8; 64 * 1024];
        f.read_exact(&mut head).expect("读取头部触发水合");
        assert_eq!(head, &expected[..head.len()], "部分读取内容一致");
    } // 句柄关闭 → 挂起 FETCH_DATA 取消

    std::thread::sleep(Duration::from_millis(500));

    // 取消后一致性：重新完整读取仍逐字节正确
    let got = std::fs::read(root.join("cancel.bin")).expect("取消后完整读取");
    assert_eq!(got, expected, "取消后重新读取必须逐字节一致（无残留损坏）");

    let _ = conn;
    cleanup(&root);
}
