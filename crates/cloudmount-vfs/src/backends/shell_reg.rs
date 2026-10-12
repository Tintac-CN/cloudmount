//! Shell 侧同步根注册（WinRT `StorageProviderSyncRootManager`）。
//!
//! ## 为什么 CfRegisterSyncRoot 之外还要这个
//!
//! `CfRegisterSyncRoot`（cfapi.rs）只做**内核过滤层**注册——占位符、水合、
//! 按需下载全部工作，但资源管理器的**导航栏节点、云状态图标、品牌显示**
//! 依赖 Shell 侧的同步根身份（DisplayName/Icon/RecycleBin 等）。
//! 官方 CloudMirror 样例即用 WinRT `StorageProviderSyncRootManager::Register`。
//!
//! ## 使用约束
//!
//! - `Id` 格式：`"ProviderName!SyncRootName"`（叹号分隔）；
//! - 与 `CfRegisterSyncRoot` 的 `ProviderName` 一致性由过滤器校验（路径匹配）；
//! - 注册是持久的（跨重启），卸载时调 [`unregister_sync_root_shell`]。

use cloudmount_core::Result;
use std::path::Path;
use windows::core::HSTRING;
use windows::Storage::Provider::{
    StorageProviderHardlinkPolicy, StorageProviderHydrationPolicy,
    StorageProviderHydrationPolicyModifier, StorageProviderInSyncPolicy,
    StorageProviderPopulationPolicy, StorageProviderSyncRootInfo, StorageProviderSyncRootManager,
};
use windows::Storage::StorageFolder;

/// Shell 侧注册参数。
pub struct ShellRegistration<'a> {
    /// 同步根的绝对路径。
    pub sync_root: &'a Path,
    /// 提供程序名（与 CfRegisterSyncRoot 的 ProviderName 一致）。
    pub provider_name: &'a str,
    /// 同步根显示名（Explorer 导航栏）。
    pub display_name: &'a str,
    /// 版本字符串。
    pub version: &'a str,
    /// 图标资源（如 `%SystemRoot%\system32\charmap.exe,0` 或品牌 ico 路径）。
    pub icon_resource: &'a str,
    /// 同步根身份（任意稳定串；样例用 "server->client"）。
    pub identity: &'a str,
}

/// Shell 侧注册同步根（Explorer 导航栏/状态图标/品牌显示）。
///
/// 幂等：先注销同名 Id 再注册。
pub fn register_shell(reg: &ShellRegistration<'_>) -> Result<()> {
    let _ = unregister_shell_id(&shell_id(reg.provider_name, reg.display_name));

    let info = StorageProviderSyncRootInfo::new()
        .map_err(|e| cloudmount_core::Error::Other(format!("SyncRootInfo 创建失败: {e}")))?;

    let id = HSTRING::from(shell_id(reg.provider_name, reg.display_name));
    info.SetId(&id)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetId 失败: {e}")))?;

    // Path 需要真实存在的目录
    if !reg.sync_root.exists() {
        std::fs::create_dir_all(reg.sync_root)?;
    }
    let folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(
        reg.sync_root.to_string_lossy().as_ref(),
    ))
    .map_err(|e| cloudmount_core::Error::Other(format!("GetFolderFromPath 失败: {e}")))?
    .join()
    .map_err(|e| cloudmount_core::Error::Other(format!("GetFolderFromPath 等待失败: {e}")))?;
    info.SetPath(&folder)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetPath 失败: {e}")))?;

    info.SetDisplayNameResource(&HSTRING::from(reg.display_name))
        .map_err(|e| cloudmount_core::Error::Other(format!("SetDisplayName 失败: {e}")))?;
    info.SetIconResource(&HSTRING::from(reg.icon_resource))
        .map_err(|e| cloudmount_core::Error::Other(format!("SetIcon 失败: {e}")))?;
    info.SetVersion(&HSTRING::from(reg.version))
        .map_err(|e| cloudmount_core::Error::Other(format!("SetVersion 失败: {e}")))?;

    // 策略与 CfRegisterSyncRoot 的 CF_SYNC_POLICIES 对齐
    info.SetHydrationPolicy(StorageProviderHydrationPolicy::Full)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetHydration 失败: {e}")))?;
    info.SetHydrationPolicyModifier(StorageProviderHydrationPolicyModifier::None)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetHydrationModifier 失败: {e}")))?;
    info.SetPopulationPolicy(StorageProviderPopulationPolicy::AlwaysFull)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetPopulation 失败: {e}")))?;
    // InSync：创建时间 + 最后写时间（与 CloudMirror 一致的保守集合）
    info.SetInSyncPolicy(
        StorageProviderInSyncPolicy::FileCreationTime
            | StorageProviderInSyncPolicy::DirectoryCreationTime
            | StorageProviderInSyncPolicy::FileLastWriteTime
            | StorageProviderInSyncPolicy::DirectoryLastWriteTime
            | StorageProviderInSyncPolicy::PreserveInsyncForSyncEngine,
    )
    .map_err(|e| cloudmount_core::Error::Other(format!("SetInSync 失败: {e}")))?;
    info.SetHardlinkPolicy(StorageProviderHardlinkPolicy::None)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetHardlink 失败: {e}")))?;
    info.SetShowSiblingsAsGroup(false)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetShowSiblings 失败: {e}")))?;

    // Context：同步根身份（UTF-8 字节）
    let context = windows::Storage::Streams::DataWriter::new()
        .map_err(|e| cloudmount_core::Error::Other(format!("DataWriter 创建失败: {e}")))?;
    context
        .WriteBytes(reg.identity.as_bytes())
        .map_err(|e| cloudmount_core::Error::Other(format!("Context 写入失败: {e}")))?;
    let buf = context
        .DetachBuffer()
        .map_err(|e| cloudmount_core::Error::Other(format!("Context 分离失败: {e}")))?;
    info.SetContext(&buf)
        .map_err(|e| cloudmount_core::Error::Other(format!("SetContext 失败: {e}")))?;

    StorageProviderSyncRootManager::Register(&info).map_err(|e| {
        cloudmount_core::Error::Other(format!(
            "StorageProviderSyncRootManager::Register 失败: {e}"
        ))
    })?;
    tracing::info!(id = %id, "Shell 侧同步根注册完成");
    Ok(())
}

/// 注销 Shell 侧同步根。
pub fn unregister_shell(provider_name: &str, display_name: &str) -> Result<()> {
    unregister_shell_id(&shell_id(provider_name, display_name))
}

fn shell_id(provider_name: &str, display_name: &str) -> String {
    format!("{provider_name}!{display_name}")
}

fn unregister_shell_id(id: &str) -> Result<()> {
    StorageProviderSyncRootManager::Unregister(&HSTRING::from(id))
        .map_err(|e| cloudmount_core::Error::Other(format!("Shell 同步根注销失败: {e}")))
}
