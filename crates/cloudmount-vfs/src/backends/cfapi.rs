//! Windows CfAPI（Cloud Files API）挂载后端 —— 仅 Windows。
//!
//! 本模块是 M2 的核心：把远端目录树投影为 NTFS 占位符（placeholder），
//! 文件被访问时由 cldflt.sys 触发 `FETCH_DATA` 回调，经水合函数取回内容
//! 并完成按需下载 —— 与 OneDrive Files On-Demand 同机制。
//!
//! ## 流程
//!
//! ```text
//! register(sync_root)                CfRegisterSyncRoot（幂等：先注销再注册）
//! create_placeholders(sync_root, …)  CfCreatePlaceholders（目录树全量投影，
//!                                    FileIdentity 携带相对路径）
//! connect(sync_root, hydrate)        CfConnectSyncRoot + FETCH_DATA 回调
//!     用户读文件 → filter 回调 → hydrate(rel, offset, len)
//!     → 内容写入占位符文件（普通 I/O）→ CfExecute(RETRIEVE_DATA) 完成水合
//! ```
//!
//! ## 设计说明
//!
//! - **水合回调签名** `Fn(&str rel_path, u64 offset, u64 len) -> Result<Vec<u8>>`：
//!   与具体后端（115/本地/测试）解耦，由调用方提供。
//! - **写占位符 + RetrieveData** 模式（微软 CloudMirror 样例同款）：内容用普通
//!   文件 I/O 写入占位符文件，再以 `CfExecute(RETRIEVE_DATA)`（Buffer=null）告知
//!   过滤器数据已就绪 —— 避免.Provider 缓冲区的特殊分配要求。
//! - **FileIdentity** 存相对路径（UTF-8 字节）；回调里优先使用 identity，
//!   归一化路径仅作日志。
//! - **确定性**：测试在本机真实 cldflt 上运行（Win10 1709+ 均内置）。
//!
//! ## 已知边界（M2 骨架）
//!
//! - 目录枚举依赖"全量投影"：创建占位符前需先拉取远端树（Population=FULL，
//!   不触发 FETCH_PLACEHOLDERS 回调）。
//! - 水合失败时不完成 RetrieveData，用户读取将超时失败（骨架取舍；
//!   正式版应经 `CfExecute` 上报错误状态并重试）。

use cloudmount_core::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::CloudFilters::{
    CfConnectSyncRoot, CfConvertToPlaceholder, CfCreatePlaceholders, CfDisconnectSyncRoot,
    CfExecute, CfRegisterSyncRoot, CfSetInSyncState, CfSetPinState, CfUnregisterSyncRoot,
    CfUpdatePlaceholder, CF_CALLBACK_INFO, CF_CALLBACK_PARAMETERS, CF_CALLBACK_REGISTRATION,
    CF_CALLBACK_TYPE_CANCEL_FETCH_DATA, CF_CALLBACK_TYPE_FETCH_DATA, CF_CALLBACK_TYPE_NONE,
    CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE, CF_CONNECTION_KEY, CF_CONNECT_FLAG_NONE, CF_CONVERT_FLAGS,
    CF_CONVERT_FLAG_DEHYDRATE, CF_CONVERT_FLAG_MARK_IN_SYNC, CF_CREATE_FLAGS, CF_CREATE_FLAG_NONE,
    CF_FS_METADATA, CF_HARDLINK_POLICY_NONE, CF_HYDRATION_POLICY, CF_HYDRATION_POLICY_FULL,
    CF_HYDRATION_POLICY_MODIFIER, CF_INSYNC_POLICY_NONE, CF_IN_SYNC_STATE_IN_SYNC,
    CF_OPERATION_ACK_DEHYDRATE_FLAGS, CF_OPERATION_INFO, CF_OPERATION_PARAMETERS,
    CF_OPERATION_PARAMETERS_0, CF_OPERATION_PARAMETERS_0_0, CF_OPERATION_PARAMETERS_0_5,
    CF_OPERATION_TYPE_ACK_DEHYDRATE, CF_OPERATION_TYPE_TRANSFER_DATA, CF_PIN_STATE_UNPINNED,
    CF_PLACEHOLDER_CREATE_FLAGS, CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION,
    CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC, CF_PLACEHOLDER_CREATE_INFO,
    CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT, CF_POPULATION_POLICY,
    CF_POPULATION_POLICY_ALWAYS_FULL, CF_POPULATION_POLICY_MODIFIER, CF_REGISTER_FLAG_NONE,
    CF_SET_IN_SYNC_FLAG_NONE, CF_SET_PIN_FLAGS, CF_SYNC_POLICIES, CF_SYNC_REGISTRATION,
    CF_UPDATE_FLAG_DEHYDRATE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_BASIC_INFO,
    FILE_FLAGS_AND_ATTRIBUTES, OPEN_EXISTING,
};

/// 环境自检：cldflt（Cloud Files 过滤器）是否可用。
///
/// Win10 1709+ 均内置；返回 false 时 CfAPI 真机测试应跳过。
pub fn platform_available() -> bool {
    unsafe { windows::Win32::Storage::CloudFilters::CfGetPlatformInfo() }.is_ok()
}

/// Unix 秒 → Windows FILETIME（100ns，自 1601-01-01）。
fn filetime_from_unix(secs: i64) -> i64 {
    (secs + 11_644_473_600) * 10_000_000
}

/// 文件 mtime（Unix 秒）；不存在返回 0。
fn file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 把文件标记为"已同步"（Explorer 显示绿勾）。
///
/// 上传成功后调用；文件须已存在（水合态）。
pub fn set_in_sync(abs_path: &Path) -> Result<()> {
    let wide = to_wide(&abs_path.to_string_lossy());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("打开文件失败: {e}")))?;
    let result = unsafe {
        CfSetInSyncState(
            handle,
            CF_IN_SYNC_STATE_IN_SYNC,
            CF_SET_IN_SYNC_FLAG_NONE,
            None,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("CfSetInSyncState 失败: {e}")));
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(handle);
    }
    result
}

/// 更新占位符（远端内容变化的重投影）。
///
/// - 新元数据（大小/mtime）+ 新 FileIdentity 生效；
/// - `CF_UPDATE_FLAG_DEHYDRATE`：丢弃本地陈旧数据（若已水合），
///   下次读取重新水合**新**内容。
///
/// **调用方必须先确认本地未被用户修改**——DEHYDRATE 会销毁未上传的修改。
pub fn update_placeholder(abs_path: &Path, size: u64, mtime: i64, identity: &[u8]) -> Result<()> {
    let wide = to_wide(&abs_path.to_string_lossy());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("打开占位符失败: {e}")))?;

    let ft = filetime_from_unix(mtime);
    let metadata = CF_FS_METADATA {
        BasicInfo: FILE_BASIC_INFO {
            CreationTime: ft,
            LastAccessTime: ft,
            LastWriteTime: ft,
            ChangeTime: ft,
            FileAttributes: FILE_ATTRIBUTE_NORMAL.0,
        },
        FileSize: size as i64,
    };
    let result = unsafe {
        CfUpdatePlaceholder(
            handle,
            Some(&metadata),
            Some(identity.as_ptr() as *const core::ffi::c_void),
            identity.len() as u32,
            None,
            windows::Win32::Storage::CloudFilters::CF_UPDATE_FLAGS(CF_UPDATE_FLAG_DEHYDRATE.0),
            None,
            None,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("CfUpdatePlaceholder 失败: {e}")));
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(handle);
    }
    result
}

/// 释放空间：把已水合的占位符文件脱水（本地数据丢弃，保留元数据）。
///
/// **文档化路径**（两步）：
/// ① `CfSetPinState(UNPINNED)`——用户意图"在线-only"（Explorer 云图标
///    语义；unpin 本身不脱水，实测平台不会主动跟进）；
/// ② `CfUpdatePlaceholder` + `CF_UPDATE_FLAG_DEHYDRATE`——文档明示
///    "更新成功后平台脱水文件"，同步生效。
///
/// **勿用 `CfDehydratePlaceholder`**：它是 cldapi.dll 的未文档化导出
/// （不在 cfapi.h 文档函数列表中）——实测（Win11 26300）返回成功但
/// 数据不释放：文件停留在中间态（稀疏标志已置、数据仍在）。
///
/// `identity` 须传该占位符的现有身份（本项目 = rel_path 字节）——
/// CfUpdatePlaceholder 会回写身份，传错会破坏水合寻址。
///
/// **约束**（官方文档）：文件必须无其它写入句柄（独占语义由调用方
/// 保证，平台不校验）；若文件被独占打开会失败（共享冲突）。
pub fn dehydrate(abs_path: &Path, identity: &[u8]) -> Result<()> {
    // ① 用户意图：UNPINNED
    let wide = to_wide(&abs_path.to_string_lossy());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("打开占位符失败: {e}")))?;
    let pin = unsafe {
        CfSetPinState(handle, CF_PIN_STATE_UNPINNED, CF_SET_PIN_FLAGS(0), None).map_err(|e| {
            cloudmount_core::Error::Other(format!("CfSetPinState(UNPINNED) 失败: {e}"))
        })
    };
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(handle);
    }
    pin?;

    // ② 实际脱水：元数据传当前值（原样保留），身份回写原值
    let meta = std::fs::metadata(abs_path)?;
    update_placeholder(abs_path, meta.len(), file_mtime(abs_path), identity)
}

/// 普通文件 → 脱水的占位符（一步完成：`CfConvertToPlaceholder` +
/// `CF_CONVERT_FLAG_DEHYDRATE`）。
///
/// 用于"用户创建并已上传"的文件释放空间：数据已在云端（调用方须先
/// 校验同步水位），转换即丢弃本地数据。`CF_CONVERT_FLAG_MARK_IN_SYNC`
/// 标记同步（Explorer 绿勾）；转换后设 UNPINNED（在线-only 语义）。
pub fn convert_and_dehydrate(abs_path: &Path, identity: &[u8]) -> Result<()> {
    let wide = to_wide(&abs_path.to_string_lossy());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("打开文件失败: {e}")))?;
    let result = unsafe {
        CfConvertToPlaceholder(
            handle,
            Some(identity.as_ptr() as *const core::ffi::c_void),
            identity.len() as u32,
            CF_CONVERT_FLAGS(CF_CONVERT_FLAG_DEHYDRATE.0 | CF_CONVERT_FLAG_MARK_IN_SYNC.0),
            None,
            None,
        )
        .map_err(|e| cloudmount_core::Error::Other(format!("CfConvertToPlaceholder 失败: {e}")))
        .and_then(|_| {
            // 转换后设 UNPINNED（在线-only；失败不回滚转换）
            CfSetPinState(handle, CF_PIN_STATE_UNPINNED, CF_SET_PIN_FLAGS(0), None).map_err(|e| {
                cloudmount_core::Error::Other(format!("CfSetPinState(UNPINNED) 失败: {e}"))
            })
        })
    };
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(handle);
    }
    result
}

/// 宽字符串 → 以 NUL 结尾的 UTF-16 缓冲。
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 一个已注册的同步根。
///
/// 注册是进程级/系统级持久状态（重启后仍有效，直到注销或同名覆盖），
/// 因此 `Drop` **不**自动注销 —— 由调用方显式调用 [`unregister`]。
#[derive(Debug, Clone)]
pub struct RegisteredSyncRoot {
    pub path: PathBuf,
}

/// 注册同步根（目录必须已存在且位于 NTFS 卷）。
///
/// 幂等：若同名同步根已注册，先注销再注册（更新身份/策略）。
pub fn register(
    sync_root: &Path,
    provider_name: &str,
    provider_version: &str,
) -> Result<RegisteredSyncRoot> {
    if !sync_root.exists() {
        std::fs::create_dir_all(sync_root)?;
    }
    // 幂等：先注销可能存在的旧注册
    let _ = unregister(sync_root);

    let sync_root_identity = to_wide(provider_name);
    let file_identity = to_wide("cloudmount-sync-root");
    let provider_name_w = to_wide(provider_name);
    let provider_version_w = to_wide(provider_version);
    let path_w = to_wide(&sync_root.to_string_lossy());

    let registration = CF_SYNC_REGISTRATION {
        StructSize: std::mem::size_of::<CF_SYNC_REGISTRATION>() as u32,
        ProviderName: PCWSTR(provider_name_w.as_ptr()),
        ProviderVersion: PCWSTR(provider_version_w.as_ptr()),
        SyncRootIdentity: sync_root_identity.as_ptr() as *const core::ffi::c_void,
        SyncRootIdentityLength: (sync_root_identity.len() * 2) as u32,
        FileIdentity: file_identity.as_ptr() as *const core::ffi::c_void,
        FileIdentityLength: (file_identity.len() * 2) as u32,
        ProviderId: windows::core::GUID::zeroed(),
    };

    let policies = CF_SYNC_POLICIES {
        StructSize: std::mem::size_of::<CF_SYNC_POLICIES>() as u32,
        Hydration: CF_HYDRATION_POLICY {
            Primary: CF_HYDRATION_POLICY_FULL,
            Modifier: CF_HYDRATION_POLICY_MODIFIER(0),
        },
        // ALWAYS_FULL(3)：目录视为始终全量填充（占位符创建即对所有进程可见）。
        // 与 FULL(2) 的区别：FULL 下目录条目需提供程序信号"填充完成"才对
        // 其它进程可见（表现为 dir 看不到文件 + 跨进程读取报"提供程序未运行"）。
        // 官方 CloudMirror 样例即用 AlwaysFull。
        Population: CF_POPULATION_POLICY {
            Primary: CF_POPULATION_POLICY_ALWAYS_FULL,
            Modifier: CF_POPULATION_POLICY_MODIFIER(0),
        },
        InSync: CF_INSYNC_POLICY_NONE,
        HardLink: CF_HARDLINK_POLICY_NONE,
        PlaceholderManagement: CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT,
    };

    unsafe {
        CfRegisterSyncRoot(
            PCWSTR(path_w.as_ptr()),
            &registration,
            &policies,
            CF_REGISTER_FLAG_NONE,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("CfRegisterSyncRoot 失败: {e}")))?;

    Ok(RegisteredSyncRoot {
        path: sync_root.to_path_buf(),
    })
}

/// 注销同步根（占位符文件被还原为普通文件或删除）。
pub fn unregister(sync_root: &Path) -> Result<()> {
    let path_w = to_wide(&sync_root.to_string_lossy());
    unsafe { CfUnregisterSyncRoot(PCWSTR(path_w.as_ptr())) }
        .map_err(|e| cloudmount_core::Error::Other(format!("CfUnregisterSyncRoot 失败: {e}")))
}

/// 占位符创建规格。
#[derive(Debug, Clone)]
pub struct PlaceholderSpec {
    /// 相对同步根的路径（`/` 分隔，可为多级：`docs/2026/a.txt`）。
    pub rel_path: String,
    /// 文件大小（目录为 0）。
    pub size: u64,
    /// 是否目录。
    pub is_dir: bool,
    /// 修改时间（Unix 秒）。
    pub mtime: i64,
    /// FileIdentity（将原样出现在 FETCH_DATA 回调中）。空则用 rel_path。
    pub identity: Vec<u8>,
}

impl PlaceholderSpec {
    /// 以相对路径作为身份的便捷构造。
    pub fn file(rel_path: impl Into<String>, size: u64, mtime: i64) -> Self {
        let rel_path = rel_path.into();
        Self {
            identity: rel_path.as_bytes().to_vec(),
            rel_path,
            size,
            is_dir: false,
            mtime,
        }
    }

    /// 目录占位符。
    pub fn dir(rel_path: impl Into<String>, mtime: i64) -> Self {
        let rel_path = rel_path.into();
        Self {
            identity: rel_path.as_bytes().to_vec(),
            rel_path,
            size: 0,
            is_dir: true,
            mtime,
        }
    }
}

/// 有序创建占位符（完整树投影）。
///
/// 关键约束：`CfCreatePlaceholders` 的 `RelativeFileName` **不允许包含路径分隔符**
/// （含分隔符会得到 `ERROR_CLOUD_FILE_INVALID_REQUEST`）。因此本函数：
/// 1. 按父目录把规格分组；
/// 2. 目录按路径深度升序逐个创建（父目录占位符先提交，子目录的 base 才存在）；
/// 3. 文件按其父目录分批创建（同一父目录内的文件可以单批）。
///
/// 返回成功创建的条目数。
pub fn create_placeholders_ordered(sync_root: &Path, specs: &[PlaceholderSpec]) -> Result<u32> {
    use std::collections::HashMap;

    // 父目录（相对路径，""=根）→ 条目
    let mut by_parent: HashMap<String, Vec<&PlaceholderSpec>> = HashMap::new();
    for spec in specs {
        let (parent, _name) = split_parent_rel(&spec.rel_path);
        by_parent.entry(parent).or_default().push(spec);
    }

    // 自动补全中间目录（如 specs 只有 "docs/2026/a.txt"，则补 "docs" 与 "docs/2026"）
    let mut all_dir_paths: std::collections::BTreeSet<String> = specs
        .iter()
        .filter(|s| s.is_dir)
        .map(|s| s.rel_path.clone())
        .collect();
    for spec in specs {
        let mut acc = String::new();
        for comp in spec.rel_path.split('/').filter(|c| !c.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(comp);
            if spec.rel_path == acc {
                break; // 条目自身（文件或末级目录）不算中间目录
            }
            all_dir_paths.insert(acc.clone());
        }
    }

    // 目录按深度升序逐个创建
    let mut dirs: Vec<String> = all_dir_paths.into_iter().collect();
    dirs.sort_by_key(|d| d.split('/').count());

    let mut total = 0u32;
    for dir_path in dirs {
        let (parent, name) = split_parent_rel(&dir_path);
        let base = join_rel(sync_root, &parent);
        let mtime = specs
            .iter()
            .find(|s| s.rel_path == dir_path)
            .map(|s| s.mtime)
            .unwrap_or(0);
        let identity = specs
            .iter()
            .find(|s| s.rel_path == dir_path)
            .map(|s| s.identity.clone())
            .unwrap_or_else(|| dir_path.as_bytes().to_vec());
        let single = PlaceholderSpec {
            rel_path: name,
            identity,
            size: 0,
            is_dir: true,
            mtime,
        };
        total += create_placeholders(&base, &[single])?;
    }

    // 文件按父目录分批
    for (parent, items) in &by_parent {
        let file_items: Vec<&PlaceholderSpec> =
            items.iter().copied().filter(|s| !s.is_dir).collect();
        if file_items.is_empty() {
            continue;
        }
        let base = join_rel(sync_root, parent);
        let owned: Vec<PlaceholderSpec> = file_items
            .into_iter()
            .map(|s| PlaceholderSpec {
                rel_path: s
                    .rel_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&s.rel_path)
                    .to_string(),
                identity: s.identity.clone(),
                size: s.size,
                is_dir: false,
                mtime: s.mtime,
            })
            .collect();
        total += create_placeholders(&base, &owned)?;
    }
    Ok(total)
}

/// 拆分相对路径的父目录与末段。
fn split_parent_rel(rel: &str) -> (String, String) {
    match rel.rsplit_once('/') {
        Some((p, n)) => (p.to_string(), n.to_string()),
        None => (String::new(), rel.to_string()),
    }
}

/// 相对路径 → 绝对路径。
fn join_rel(root: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    }
}

/// 在同步根下创建占位符（幂等：已存在的相对路径会失败但其余继续）。
///
/// 返回成功创建的条目数。
pub fn create_placeholders(sync_root: &Path, specs: &[PlaceholderSpec]) -> Result<u32> {
    if specs.is_empty() {
        return Ok(0);
    }
    let base_w = to_wide(&sync_root.to_string_lossy());

    // 预构造宽字符串（CF_PLACEHOLDER_CREATE_INFO 持有指针，须活到调用结束）
    let mut wide_paths: Vec<Vec<u16>> = Vec::with_capacity(specs.len());
    let mut identities: Vec<Vec<u8>> = Vec::with_capacity(specs.len());
    let mut infos: Vec<CF_PLACEHOLDER_CREATE_INFO> = Vec::with_capacity(specs.len());

    for spec in specs {
        // RelativeFileName：Windows 路径分隔符
        let rel = spec.rel_path.replace('/', "\\");
        wide_paths.push(to_wide(&rel));
        identities.push(if spec.identity.is_empty() {
            spec.rel_path.as_bytes().to_vec()
        } else {
            spec.identity.clone()
        });

        let attr = if spec.is_dir {
            FILE_ATTRIBUTE_DIRECTORY
        } else {
            FILE_ATTRIBUTE_NORMAL
        };
        let ft = filetime_from_unix(spec.mtime);
        infos.push(CF_PLACEHOLDER_CREATE_INFO {
            RelativeFileName: PCWSTR(wide_paths.last().unwrap().as_ptr()),
            FsMetadata: CF_FS_METADATA {
                BasicInfo: FILE_BASIC_INFO {
                    CreationTime: ft,
                    LastAccessTime: ft,
                    LastWriteTime: ft,
                    ChangeTime: ft,
                    FileAttributes: attr.0,
                },
                FileSize: spec.size as i64,
            },
            FileIdentity: identities.last().unwrap().as_ptr() as *const core::ffi::c_void,
            FileIdentityLength: identities.last().unwrap().len() as u32,
            // 目录加 DISABLE_ON_DEMAND_POPULATION：内容全量投影，过滤器不再
            // 发起 FETCH_PLACEHOLDERS（缺此标志时在目录下创建子项会失败）。
            Flags: CF_PLACEHOLDER_CREATE_FLAGS(if spec.is_dir {
                CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC.0
                    | CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION.0
            } else {
                CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC.0
            }),
            Result: Default::default(),
            CreateUsn: 0,
        });
    }

    let mut processed: u32 = 0;
    unsafe {
        CfCreatePlaceholders(
            PCWSTR(base_w.as_ptr()),
            &mut infos,
            CF_CREATE_FLAGS(CF_CREATE_FLAG_NONE.0),
            Some(&mut processed),
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("CfCreatePlaceholders 失败: {e}")))?;

    // 部分失败（如路径已存在）只告警：调用方可用 processed 判断
    if (processed as usize) < specs.len() {
        tracing::warn!(
            created = processed,
            total = specs.len(),
            "部分占位符创建失败（多为已存在）"
        );
    }
    Ok(processed)
}

/// 水合函数：给定相对路径与请求范围，返回内容。
pub type HydrateFn = Box<dyn Fn(&str, u64, u64) -> Result<Vec<u8>> + Send + Sync>;

/// 分块推送 sink（F-01 水合分块契约的回调侧接口）。
///
/// U9 后水合方不再一次性返回整块 `Vec<u8>`，而是逐块调用 [`ChunkSink::transfer`]
/// 回传内容；回调侧每块完成一次 `TRANSFER_DATA`（同步返回 → 天然背压，
/// 任一时刻至多一个块在途，内存峰值 = 1 块）。`offset` 为该块在文件内的
/// 绝对偏移；实现方返回 `Err`（含取消：每块发送前查 `cancelled` 集合）时，
/// 分块泵送立即中止并丢弃 reader（关闭下游连接）。
pub trait ChunkSink {
    fn transfer(&mut self, offset: u64, data: &[u8]) -> Result<()>;
}

/// F-01 分块泵送核心循环（crate 内可测 seam：T-01/T-02 单元腿的注入点，
/// 短读/错误/取消语义直接压循环体；水合闭包经它循环读单个复用块缓冲）。
///
/// 契约（T-01 断言口径 = **reader 级读取序列**）：
/// - 除末块外每次 `read` 请求恒为 `chunk_size`，全部 ≤ `chunk_size`；
/// - `transfer` 序列自 `offset` 起连续覆盖已读内容，拼接逐字节一致；
/// - reader 或 sink 的任一错误原样传播并立即停止（不再发起后续读取，
///   失败的读取不产生伪造 transfer）。
///
/// 返回成功泵送的总字节数。
#[allow(dead_code)] // TDD 骨架期占位：实现阶段由水合闭包接入后移除
pub(crate) fn pump_chunks(
    _reader: &mut dyn std::io::Read,
    _sink: &mut dyn ChunkSink,
    _offset: u64,
    _chunk_size: usize,
) -> Result<u64> {
    todo!("U9（F-01）红阶段骨架：分块泵送循环于实现阶段落地")
}

/// 水合记录（rel_path → 完成后 mtime），连接回调与挂载层共享。
///
/// 双向同步的关键：过滤器的按需水合会写文件（触发 RDC 修改事件），
/// 但那不是用户修改、不应上传。上传协调器据此区分。
pub type HydrationLog = Arc<Mutex<HashMap<String, i64>>>;

/// 连接上下文（经 `CfConnectSyncRoot` 的 callbackcontext 传入回调）。
struct HydrationContext {
    hydrate: HydrateFn,
    /// 同步根（水合完成后 stat 文件 mtime 用）。
    sync_root: PathBuf,
    /// 已取消的传输（TransferKey）。
    cancelled: Mutex<HashSet<i64>>,
    /// 最近水合完成的（rel_path → 完成后的 mtime）。
    ///
    /// **双向同步的关键**：过滤器的按需水合会写文件（触发 RDC 修改事件），
    /// 但那不是用户修改、不应上传。上传协调器据此区分：
    /// RDC 报告的 mtime == 水合记录的 mtime → 过滤器写入，跳过；
    /// 不等（用户在水合后又改过）→ 上传。
    hydrated: HydrationLog,
}

unsafe impl Send for HydrationContext {}
unsafe impl Sync for HydrationContext {}

/// 一个已连接的同步根。
///
/// `Drop` 断开连接（停止回调），随后释放上下文。
pub struct CloudConnection {
    key: CF_CONNECTION_KEY,
    sync_root: PathBuf,
    // 持有上下文：Drop 顺序保证回调停止后才释放
    _ctx: Arc<HydrationContext>,
}

impl CloudConnection {
    /// 同步根路径。
    pub fn sync_root(&self) -> &Path {
        &self.sync_root
    }
}

impl Drop for CloudConnection {
    fn drop(&mut self) {
        unsafe {
            let _ = CfDisconnectSyncRoot(self.key);
        }
    }
}

/// 连接同步根并开始接收水合回调。
///
/// `hydrate` 在过滤器的回调线程上被调用（可能并发），必须对同一文件的
/// 相同范围返回稳定内容。
pub fn connect(sync_root: &Path, hydrate: HydrateFn) -> Result<CloudConnection> {
    connect_with_log(sync_root, hydrate, Arc::new(Mutex::new(HashMap::new())))
}

/// [`connect`] 的变体：水合记录写入调用方提供的共享日志
/// （挂载层据此区分"过滤器水合写入"与"用户修改"）。
pub fn connect_with_log(
    sync_root: &Path,
    hydrate: HydrateFn,
    hydration_log: HydrationLog,
) -> Result<CloudConnection> {
    let ctx = Arc::new(HydrationContext {
        hydrate,
        sync_root: sync_root.to_path_buf(),
        cancelled: Mutex::new(HashSet::new()),
        hydrated: hydration_log,
    });

    static CALLBACK_TABLE: [CF_CALLBACK_REGISTRATION; 4] = [
        CF_CALLBACK_REGISTRATION {
            Type: CF_CALLBACK_TYPE_FETCH_DATA,
            Callback: Some(fetch_data_callback),
        },
        CF_CALLBACK_REGISTRATION {
            Type: CF_CALLBACK_TYPE_CANCEL_FETCH_DATA,
            Callback: Some(cancel_fetch_data_callback),
        },
        // 脱水确认：平台脱水前询问 provider（用户应用被阻塞，期望响应）。
        // 不注册则脱水请求无人 ACK 而挂起（60s 超时后放弃，数据不释放）。
        CF_CALLBACK_REGISTRATION {
            Type: CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE,
            Callback: Some(notify_dehydrate_callback),
        },
        CF_CALLBACK_REGISTRATION {
            Type: CF_CALLBACK_TYPE_NONE,
            Callback: None,
        },
    ];

    let path_w = to_wide(&sync_root.to_string_lossy());
    let key = unsafe {
        // windows-rs 绑定省略了 callbacktablesize（过滤器以 END 标记界定表）
        CfConnectSyncRoot(
            PCWSTR(path_w.as_ptr()),
            CALLBACK_TABLE.as_ptr(),
            Some(Arc::as_ptr(&ctx) as *const core::ffi::c_void),
            CF_CONNECT_FLAG_NONE,
        )
    }
    .map_err(|e| cloudmount_core::Error::Other(format!("CfConnectSyncRoot 失败: {e}")))?;

    Ok(CloudConnection {
        key,
        sync_root: sync_root.to_path_buf(),
        _ctx: ctx,
    })
}

/// 从回调信息提取相对路径（FileIdentity 为 UTF-8 的相对路径）。
unsafe fn identity_to_rel(info: &CF_CALLBACK_INFO) -> Option<String> {
    if info.FileIdentity.is_null() || info.FileIdentityLength == 0 {
        return None;
    }
    let bytes = std::slice::from_raw_parts(
        info.FileIdentity as *const u8,
        info.FileIdentityLength as usize,
    );
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// FETCH_DATA 回调：按需水合。
unsafe extern "system" fn fetch_data_callback(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if info.is_null() {
        return;
    }
    let info = &*info;
    let Some(ctx) = (info.CallbackContext as *const HydrationContext).as_ref() else {
        return;
    };
    let Some(rel_path) = identity_to_rel(info) else {
        return;
    };

    // 请求范围
    let (offset, length) = if params.is_null() {
        (0i64, info.FileSize)
    } else {
        let p = &(*params).Anonymous.FetchData;
        (p.RequiredFileOffset, p.RequiredLength)
    };

    tracing::debug!(rel = %rel_path, offset, length, "FETCH_DATA");

    // 已取消的传输：不再回填
    {
        let cancelled = ctx.cancelled.lock().expect("lock");
        if cancelled.contains(&info.TransferKey) {
            tracing::debug!(rel = %rel_path, "传输已取消，跳过水合");
            return;
        }
    }

    // 水合
    let data = (ctx.hydrate)(&rel_path, offset as u64, length as u64);

    match data {
        Ok(bytes) => {
            // **关键：直接把缓冲交给过滤器（RetrieveData.Buffer）。**
            // 不要在这里用普通 I/O 打开占位符——对已脱水的占位符申请写访问
            // 会触发过滤器的隐式水合 → 递归等待当前 FETCH_DATA → 死锁
            // （表现为用户读取超时后 CANCEL）。
            if let Err(e) = transfer_data(info, offset, Some(&bytes), 0) {
                tracing::error!(rel = %rel_path, error = %e, "TransferData 失败");
            } else {
                // 记录水合后的 mtime：供上传协调器区分"过滤器写入"与"用户修改"
                if let Ok(mut hydrated) = ctx.hydrated.lock() {
                    let abs = ctx.sync_root.join(rel_path.replace('/', "\\"));
                    hydrated.insert(rel_path.to_string(), file_mtime(&abs));
                }
                tracing::debug!(rel = %rel_path, len = bytes.len(), "水合完成");
            }
        }
        Err(e) => {
            // 用失败的 TRANSFER_DATA 结束挂起的读取（明确错误而非 60s 超时）
            let _ = transfer_data(info, offset, None, STATUS_UNSUCCESSFUL);
            tracing::error!(rel = %rel_path, error = %e, "水合失败（读取已以错误结束）");
        }
    }
}

/// CANCEL_FETCH_DATA 回调：记录取消的传输。
unsafe extern "system" fn cancel_fetch_data_callback(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if info.is_null() || params.is_null() {
        return;
    }
    let info = &*info;
    let cancel = &(*params).Anonymous.Cancel;
    let _ = cancel;
    let Some(ctx) = (info.CallbackContext as *const HydrationContext).as_ref() else {
        return;
    };
    if let Ok(mut set) = ctx.cancelled.lock() {
        set.insert(info.TransferKey);
    }
    tracing::debug!(transfer = info.TransferKey, "水合被取消");
}

/// NOTIFY_DEHYDRATE 回调：平台即将脱水占位符（用户应用被阻塞），
/// ACK 放行。脱水来源：`CfSetPinState(UNPINNED)`（我们的 dehydrate 命令）
/// 或 Explorer 右键"释放空间"（Shell 同样走 unpin）。
unsafe extern "system" fn notify_dehydrate_callback(
    info: *const CF_CALLBACK_INFO,
    _params: *const CF_CALLBACK_PARAMETERS,
) {
    if info.is_null() {
        return;
    }
    let info = &*info;
    let rel = identity_to_rel(info).unwrap_or_default();
    tracing::debug!(rel = %rel, "NOTIFY_DEHYDRATE（平台请求脱水，ACK 放行）");
    if let Err(e) = ack_dehydrate(info) {
        tracing::error!(rel = %rel, error = %e, "ACK_DEHYDRATE 失败（脱水被挂起）");
    }
}

/// 确认脱水（CF_OPERATION_TYPE_ACK_DEHYDRATE）。
///
/// FileIdentity 原样回传（不借脱水变更身份）；CompletionStatus=0 放行。
unsafe fn ack_dehydrate(info: &CF_CALLBACK_INFO) -> Result<()> {
    let op_info = CF_OPERATION_INFO {
        StructSize: std::mem::size_of::<CF_OPERATION_INFO>() as u32,
        Type: CF_OPERATION_TYPE_ACK_DEHYDRATE,
        ConnectionKey: info.ConnectionKey,
        TransferKey: info.TransferKey,
        CorrelationVector: std::ptr::null(),
        SyncStatus: std::ptr::null(),
        RequestKey: info.RequestKey,
    };
    let ack = CF_OPERATION_PARAMETERS_0_5 {
        Flags: CF_OPERATION_ACK_DEHYDRATE_FLAGS(0),
        CompletionStatus: windows::Win32::Foundation::NTSTATUS(0),
        FileIdentity: info.FileIdentity,
        FileIdentityLength: info.FileIdentityLength,
    };
    // ParamSize = union 成员偏移 + 成员大小（与 C 的 CF_SIZE_OF_OP_PARAM 宏等价）
    let param_size = (std::mem::offset_of!(CF_OPERATION_PARAMETERS, Anonymous)
        + std::mem::size_of::<CF_OPERATION_PARAMETERS_0_5>()) as u32;
    let mut params = CF_OPERATION_PARAMETERS {
        ParamSize: param_size,
        Anonymous: CF_OPERATION_PARAMETERS_0 { AckDehydrate: ack },
    };
    unsafe { CfExecute(&op_info, &mut params) }
        .map_err(|e| cloudmount_core::Error::Other(format!("CfExecute(ACK_DEHYDRATE) 失败: {e}")))
}

/// 完成 FETCH_DATA：用 **TRANSFER_DATA** 操作把缓冲交给过滤器。
///
/// 官方 CloudMirror 样例（FileCopierWithProgress.cpp::TransferData）即此模式：
/// `CF_OPERATION_TYPE_TRANSFER_DATA` + `CompletionStatus`（NTSTATUS，0=成功）。
/// `RETRIEVE_DATA` 不用于完成 FETCH_DATA，过滤器会以
/// `ERROR_CLOUD_FILE_INVALID_REQUEST` 拒绝。
unsafe fn transfer_data(
    info: &CF_CALLBACK_INFO,
    offset: i64,
    data: Option<&[u8]>,
    status: i32,
) -> Result<()> {
    let op_info = CF_OPERATION_INFO {
        StructSize: std::mem::size_of::<CF_OPERATION_INFO>() as u32,
        Type: CF_OPERATION_TYPE_TRANSFER_DATA,
        ConnectionKey: info.ConnectionKey,
        TransferKey: info.TransferKey,
        CorrelationVector: std::ptr::null(),
        SyncStatus: std::ptr::null(),
        RequestKey: info.RequestKey,
    };
    let (buffer, length) = match data {
        Some(bytes) => (
            bytes.as_ptr() as *const core::ffi::c_void,
            bytes.len() as i64,
        ),
        None => (std::ptr::null(), 0),
    };
    let transfer = CF_OPERATION_PARAMETERS_0_0 {
        Flags: Default::default(),
        CompletionStatus: windows::Win32::Foundation::NTSTATUS(status),
        Buffer: buffer,
        Offset: offset,
        Length: length,
    };
    // ParamSize = union 成员偏移 + 成员大小（与 C 的 CF_SIZE_OF_OP_PARAM 宏等价）
    let param_size = (std::mem::offset_of!(CF_OPERATION_PARAMETERS, Anonymous)
        + std::mem::size_of::<CF_OPERATION_PARAMETERS_0_0>()) as u32;
    let mut params = CF_OPERATION_PARAMETERS {
        ParamSize: param_size,
        Anonymous: windows::Win32::Storage::CloudFilters::CF_OPERATION_PARAMETERS_0 {
            TransferData: transfer,
        },
    };
    unsafe { CfExecute(&op_info, &mut params) }
        .map_err(|e| cloudmount_core::Error::Other(format!("CfExecute(TRANSFER_DATA) 失败: {e}")))
}

/// STATUS_UNSUCCESSFUL（NTSTATUS）。
const STATUS_UNSUCCESSFUL: i32 = 0xC000_0001u32 as i32;

/// F-01（U9）TDD 失败测试 —— T-01/T-02 单元腿。
///
/// 断言口径（test-strategy m1）：**reader 级读取序列** —— 桩 Reader 记录每次
/// `read()` 的请求长度：修复前 `read_to_end` 呈几何增长直至总范围等大；
/// 修复后除末块外恒为块上限。勿以 `open_range` 的 RangeSpec 断言
/// （修复前后都只调用一次 open_range，无判别力）。
#[cfg(test)]
mod pump_chunks_tests {
    use super::{pump_chunks, ChunkSink};
    use cloudmount_core::{Error, Result};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 单元腿用小块上限（64 KiB）：与 1 MiB 同一"几何增长 vs 定长"判别口径，
    /// 测试跑得快；1 MiB 实现值口径另见
    /// [`pump_chunks_1mib_cap_medium_body_from_random_offset`]。
    const CHUNK: usize = 64 * 1024;

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

    /// 桩 Reader：记录每次 `read` 的（请求长度，返回长度）序列。
    ///
    /// `short`：每次至多返回请求长度的一半（短读形态，T-02）；
    /// `fail_at`：第 N 次调用（0 起）注入 io 错误（T-02 错误传播）。
    /// drop 时向计数器 +1（T-02 取消路径的 drop 计数断言）。
    struct RecordingReader {
        data: Vec<u8>,
        pos: usize,
        short: bool,
        fail_at: Option<usize>,
        calls: usize,
        requests: Vec<usize>,
        returned: Vec<usize>,
        drops: Arc<AtomicUsize>,
    }

    impl RecordingReader {
        fn with(data: Vec<u8>) -> Self {
            Self {
                data,
                pos: 0,
                short: false,
                fail_at: None,
                calls: 0,
                requests: Vec::new(),
                returned: Vec::new(),
                drops: Arc::new(AtomicUsize::new(0)),
            }
        }

        /// 短读形态：每次至多返回请求长度的一半。
        fn short(mut self) -> Self {
            self.short = true;
            self
        }

        /// 第 `call` 次（0 起）读取注入 io 错误。
        fn failing_at(mut self, call: usize) -> Self {
            self.fail_at = Some(call);
            self
        }

        fn drop_counter(&self) -> Arc<AtomicUsize> {
            self.drops.clone()
        }
    }

    impl std::io::Read for RecordingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            self.requests.push(buf.len());
            if self.fail_at == Some(self.calls - 1) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "reader 中途故障（T-02 注入）",
                ));
            }
            if self.pos >= self.data.len() || buf.is_empty() {
                self.returned.push(0);
                return Ok(0);
            }
            let want = if self.short {
                (buf.len() / 2).max(1)
            } else {
                buf.len()
            };
            let n = want.min(self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            self.returned.push(n);
            Ok(n)
        }
    }

    impl Drop for RecordingReader {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// 桩 Sink：收集 (offset, data) 序列；第 N 次 `transfer`（0 起）可注入
    /// 错误（模拟 FETCH_DATA 侧取消：每块发送前查 `cancelled` → Err）。
    struct RecordingSink {
        transfers: Vec<(u64, Vec<u8>)>,
        fail_at: Option<usize>,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self {
                transfers: Vec::new(),
                fail_at: None,
            }
        }

        fn failing_at(n: usize) -> Self {
            Self {
                transfers: Vec::new(),
                fail_at: Some(n),
            }
        }

        fn concatenated(&self) -> Vec<u8> {
            self.transfers
                .iter()
                .flat_map(|(_, d)| d.iter().copied())
                .collect()
        }
    }

    impl ChunkSink for RecordingSink {
        fn transfer(&mut self, offset: u64, data: &[u8]) -> Result<()> {
            if self.fail_at == Some(self.transfers.len()) {
                return Err(Error::Other("sink 中止（模拟取消/TRANSFER_DATA 失败）".into()));
            }
            self.transfers.push((offset, data.to_vec()));
            Ok(())
        }
    }

    /// T-01：除末块外每次读取请求恒为块上限、全部 ≤ 上限；transfer 序列自 0
    /// 连续覆盖、拼接逐字节一致；以 0 长度读取探测 EOF 干净收尾。
    ///
    /// 红测判别：现状水合闭包 `read_to_end` 一次性填满整个容量缓冲（几何增长）。
    #[test]
    fn pump_chunks_requests_bounded_at_chunk_cap_and_content_identical() {
        let body = pseudo_random(CHUNK * 5 + 12345, 0xDA7A_115);
        let mut reader = RecordingReader::with(body.clone());
        let mut sink = RecordingSink::new();

        let total = pump_chunks(&mut reader, &mut sink, 0, CHUNK).expect("泵送成功");

        assert_eq!(total as usize, body.len(), "应返回泵送总字节数");
        // reader 级读取序列：除末次外恒为块上限，全部 ≤ 上限
        let (last, head) = reader.requests.split_last().expect("至少一次读取");
        assert!(
            head.iter().all(|&r| r == CHUNK),
            "除末次外读取请求恒为块上限（几何增长即红），实际 {head:?}"
        );
        assert!(*last <= CHUNK, "末次读取请求 ≤ 块上限");
        assert_eq!(
            reader.returned.iter().sum::<usize>(),
            body.len(),
            "读出字节总数 = 数据长度"
        );
        assert_eq!(
            reader.returned.last(),
            Some(&0),
            "以 0 长度读取探测 EOF 干净收尾"
        );
        // transfer 序列：自 0 连续覆盖、无零长度块、末块短读不假红
        let mut expect_off = 0u64;
        for (off, data) in &sink.transfers {
            assert_eq!(*off, expect_off, "transfer 偏移必须连续");
            assert!(!data.is_empty(), "不产生零长度 transfer");
            assert!(data.len() <= CHUNK, "单块 ≤ 块上限");
            expect_off += data.len() as u64;
        }
        assert_eq!(sink.concatenated(), body, "拼接内容逐字节一致");
        assert!(
            sink.transfers.last().unwrap().1.len() < CHUNK,
            "非对齐尾 → 末块为短块（末块短读不假红）"
        );
    }

    /// T-01（1 MiB 实现值口径，关键决策 #2）：12 MiB + 奇数尾、自非零随机
    /// 偏移起泵送，内容逐字节一致，除末次外读取请求恒为 1 MiB。
    #[test]
    fn pump_chunks_1mib_cap_medium_body_from_random_offset() {
        const MIB: usize = 1024 * 1024;
        let base: u64 = 4_210_197; // 随机偏移（文件内绝对偏移由调用方给定）
        let body = pseudo_random(MIB * 12 + 3777, 0x115_5A7);
        let mut reader = RecordingReader::with(body.clone());
        let mut sink = RecordingSink::new();

        let total = pump_chunks(&mut reader, &mut sink, base, MIB).expect("泵送成功");

        assert_eq!(total as usize, body.len());
        let (last, head) = reader.requests.split_last().unwrap();
        assert!(head.iter().all(|&r| r == MIB), "除末次外恒为 1 MiB 读取");
        assert!(*last <= MIB);
        let mut expect_off = base;
        for (off, data) in &sink.transfers {
            assert_eq!(*off, expect_off, "transfer 偏移自给定起点连续推进");
            expect_off += data.len() as u64;
        }
        assert_eq!(sink.concatenated(), body, "拼接内容逐字节一致");
    }

    /// T-02：短读 Reader（每次至多返回半个块）——读取请求仍只按块上限发起
    /// （不放大缓冲补偿短读），内容拼接不受短读影响，无 panic。
    #[test]
    fn pump_chunks_short_reads_still_request_within_cap() {
        let body = pseudo_random(CHUNK * 3 + 7, 0x510D_0A7);
        let mut reader = RecordingReader::with(body.clone()).short();
        let mut sink = RecordingSink::new();

        let total = pump_chunks(&mut reader, &mut sink, 0, CHUNK).expect("短读下仍应泵送成功");

        assert_eq!(total as usize, body.len());
        // 只按块上限断言读取请求（返回值序列允许半块）
        assert!(
            reader.requests.iter().all(|&r| r <= CHUNK),
            "短读下读取请求仍恒 ≤ 块上限（放大缓冲补偿即红），实际 {:?}",
            reader.requests
        );
        assert_eq!(sink.concatenated(), body, "短读不影响内容一致性");
    }

    /// T-02：Reader 中途 Err → 以 `Error::Io` 原样传播（保留原始错误消息）、
    /// 立即停止读取、失败的读取不产生伪造 transfer（无部分写入残留）、无 panic。
    #[test]
    fn pump_chunks_reader_error_propagates_without_partial_writes() {
        let body = pseudo_random(CHUNK * 2, 0xE115);
        let mut reader = RecordingReader::with(body.clone()).failing_at(2); // 第 3 次读取注入错误
        let mut sink = RecordingSink::new();

        let err = pump_chunks(&mut reader, &mut sink, 0, CHUNK).expect_err("错误必须传播");

        match &err {
            Error::Io(e) => assert!(
                e.to_string().contains("T-02 注入"),
                "应保留原始 io 错误，实际 {err:?}"
            ),
            other => panic!("io 错误应映射为 Error::Io 传播，实际 {other:?}"),
        }
        assert_eq!(reader.calls, 3, "错误后立即停止，不再发起后续读取");
        assert_eq!(sink.transfers.len(), 2, "仅错误前完整读出的块进入 sink");
        assert!(sink.transfers.iter().all(|(_, d)| d.len() == CHUNK));
        assert_eq!(
            sink.concatenated(),
            body,
            "已成功读出的内容完整推送，无丢失/重复/伪造"
        );
    }

    /// T-02：取消位（FETCH_DATA 回调的 sink 每块发送前查 `cancelled` → Err）
    /// → 泵送立即以该错误中止：不再向下游发起读取（不吸干 reader）、
    /// 被取消的块不进入 sink、reader 随调用方丢弃（drop 计数 = 下游连接关闭）。
    #[test]
    fn pump_chunks_sink_cancel_stops_reading_and_drops_reader() {
        let body = pseudo_random(CHUNK * 8, 0xCA4CE);
        let mut reader = RecordingReader::with(body);
        let drops = reader.drop_counter();
        let mut sink = RecordingSink::failing_at(3); // 第 4 块前取消

        let result = pump_chunks(&mut reader, &mut sink, 0, CHUNK);
        let err = result.expect_err("取消必须以 Err 中止泵送");
        assert!(err.to_string().contains("取消"), "取消错误原样传播，实际 {err:?}");

        assert!(
            reader.requests.len() <= 4,
            "取消后立即停止读取（继续吸干 reader 即红），实际 {} 次",
            reader.requests.len()
        );
        assert_eq!(sink.transfers.len(), 3, "被取消的块不得进入 sink");

        // 实现形态：取消 → Err 返回 → 调用方丢弃 reader（关闭下游 HTTP 连接）
        drop(reader);
        assert_eq!(drops.load(Ordering::SeqCst), 1, "桩 Reader 应被 drop（下游关闭）");
    }
}
