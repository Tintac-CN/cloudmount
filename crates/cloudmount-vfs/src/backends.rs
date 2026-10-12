/// 平台原生挂载后端实现（按平台 cfg 编译）。
#[cfg(windows)]
pub mod cfapi;

/// CfAPI 高层后端（MountBackend 实现）。
#[cfg(windows)]
pub mod cfapi_mount;

/// Shell 侧同步根注册（WinRT StorageProviderSyncRootManager，仅 Windows）。
#[cfg(windows)]
pub mod shell_reg;
