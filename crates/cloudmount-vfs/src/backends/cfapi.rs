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
    CfConnectSyncRoot, CfCreatePlaceholders, CfDisconnectSyncRoot, CfExecute, CfRegisterSyncRoot,
    CfSetInSyncState, CfUnregisterSyncRoot, CfUpdatePlaceholder, CF_CALLBACK_INFO,
    CF_CALLBACK_PARAMETERS, CF_CALLBACK_REGISTRATION, CF_CALLBACK_TYPE_CANCEL_FETCH_DATA,
    CF_CALLBACK_TYPE_FETCH_DATA, CF_CALLBACK_TYPE_NONE, CF_CONNECTION_KEY, CF_CONNECT_FLAG_NONE,
    CF_CREATE_FLAGS, CF_CREATE_FLAG_NONE, CF_FS_METADATA, CF_HARDLINK_POLICY_NONE,
    CF_HYDRATION_POLICY, CF_HYDRATION_POLICY_FULL, CF_HYDRATION_POLICY_MODIFIER,
    CF_INSYNC_POLICY_NONE, CF_IN_SYNC_STATE_IN_SYNC, CF_OPERATION_INFO, CF_OPERATION_PARAMETERS,
    CF_OPERATION_PARAMETERS_0_0, CF_OPERATION_TYPE_TRANSFER_DATA, CF_PLACEHOLDER_CREATE_FLAGS,
    CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION,
    CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC, CF_PLACEHOLDER_CREATE_INFO,
    CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT, CF_POPULATION_POLICY,
    CF_POPULATION_POLICY_ALWAYS_FULL, CF_POPULATION_POLICY_MODIFIER, CF_REGISTER_FLAG_NONE,
    CF_SET_IN_SYNC_FLAG_NONE, CF_SYNC_POLICIES, CF_SYNC_REGISTRATION, CF_UPDATE_FLAG_DEHYDRATE,
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

    static CALLBACK_TABLE: [CF_CALLBACK_REGISTRATION; 3] = [
        CF_CALLBACK_REGISTRATION {
            Type: CF_CALLBACK_TYPE_FETCH_DATA,
            Callback: Some(fetch_data_callback),
        },
        CF_CALLBACK_REGISTRATION {
            Type: CF_CALLBACK_TYPE_CANCEL_FETCH_DATA,
            Callback: Some(cancel_fetch_data_callback),
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
