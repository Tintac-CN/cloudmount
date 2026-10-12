//! 凭据安全存储。
//!
//! 抽象一层 [`SecretStore`]，各平台实现：
//! - Windows：DPAPI（`CryptProtectData`）或凭据管理器
//! - macOS：Keychain Services
//! - Linux：Secret Service（libsecret）/ 回退到文件 + 0600 权限
//!
//! 官方 OpenAPI 的 token 与 Cookie 通道的凭据**物理隔离**存放，降低互踢面。

use cloudmount_core::{Error, Result};

/// 凭据类别（隔离存放）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// 官方 OpenAPI 的 access/refresh token。
    OpenApiToken,
    /// Cookie 通道的 `UID/CID/SEID/KID`。
    CookieJar,
}

impl SecretKind {
    /// 存储键名。
    pub fn key(self) -> &'static str {
        match self {
            SecretKind::OpenApiToken => "cloudmount/openapi-token",
            SecretKind::CookieJar => "cloudmount/cookie-jar",
        }
    }
}

/// 凭据存储接口。
pub trait SecretStore: Send + Sync {
    /// 写入凭据（加密）。
    fn put(&self, kind: SecretKind, secret: &[u8]) -> Result<()>;
    /// 读取凭据。
    fn get(&self, kind: SecretKind) -> Result<Option<Vec<u8>>>;
    /// 删除凭据。
    fn delete(&self, kind: SecretKind) -> Result<()>;
}

/// 内存实现（测试 / 无安全存储的环境；**不用于生产**）。
#[derive(Default)]
pub struct MemoryStore {
    items: std::sync::Mutex<std::collections::HashMap<&'static str, Vec<u8>>>,
}

impl SecretStore for MemoryStore {
    fn put(&self, kind: SecretKind, secret: &[u8]) -> Result<()> {
        self.items
            .lock()
            .map_err(|_| Error::Other("lock poisoned".into()))?
            .insert(kind.key(), secret.to_vec());
        Ok(())
    }

    fn get(&self, kind: SecretKind) -> Result<Option<Vec<u8>>> {
        Ok(self
            .items
            .lock()
            .map_err(|_| Error::Other("lock poisoned".into()))?
            .get(kind.key())
            .cloned())
    }

    fn delete(&self, kind: SecretKind) -> Result<()> {
        self.items
            .lock()
            .map_err(|_| Error::Other("lock poisoned".into()))?
            .remove(kind.key());
        Ok(())
    }
}

/// 基于文件的回退实现（权限 0600；内容应已由上层加密）。
pub struct FileStore {
    /// 存储目录。
    pub dir: std::path::PathBuf,
}

impl FileStore {
    /// 在给定目录创建。
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// 默认位置：`<data_dir>/secrets`。
    pub fn default_location() -> Result<Self> {
        Ok(Self::new(
            cloudmount_core::paths::ensure_data_dir()?.join("secrets"),
        ))
    }

    fn path_of(&self, kind: SecretKind) -> std::path::PathBuf {
        self.dir.join(kind.key().replace('/', "_"))
    }
}

impl SecretStore for FileStore {
    fn put(&self, kind: SecretKind, secret: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path_of(kind);
        std::fs::write(&path, secret)?;
        restrict_permissions(&path)?;
        Ok(())
    }

    fn get(&self, kind: SecretKind) -> Result<Option<Vec<u8>>> {
        match std::fs::read(self.path_of(kind)) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, kind: SecretKind) -> Result<()> {
        match std::fs::remove_file(self.path_of(kind)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// 收紧文件权限到 0600（仅 Unix；Windows 依赖用户目录 ACL）。
fn restrict_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// 系统凭据存储（桌面端原生；Linux/边缘回退到 0600 文件）。
#[cfg(any(windows, target_os = "macos"))]
pub struct SystemStore;

#[cfg(any(windows, target_os = "macos"))]
impl SystemStore {
    const SERVICE: &'static str = "io.github.tintac-cn.cloudmount";

    fn entry(kind: SecretKind) -> Result<keyring::Entry> {
        keyring::Entry::new(Self::SERVICE, kind.key())
            .map_err(|e| Error::Other(format!("系统凭据库不可用: {e}")))
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl SecretStore for SystemStore {
    fn put(&self, kind: SecretKind, secret: &[u8]) -> Result<()> {
        // 系统凭据库以字符串存储，二进制用 base64 包装
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(secret);
        Self::entry(kind)?
            .set_password(&encoded)
            .map_err(|e| Error::Other(format!("写入系统凭据库失败: {e}")))
    }

    fn get(&self, kind: SecretKind) -> Result<Option<Vec<u8>>> {
        use base64::Engine as _;
        match Self::entry(kind)?.get_password() {
            Ok(encoded) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded.as_bytes())
                    .map_err(|e| Error::Other(format!("凭据解码失败: {e}")))?;
                Ok(Some(bytes))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Other(format!("读取系统凭据库失败: {e}"))),
        }
    }

    fn delete(&self, kind: SecretKind) -> Result<()> {
        match Self::entry(kind)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Other(format!("删除系统凭据失败: {e}"))),
        }
    }
}

/// 平台默认凭据存储。
///
/// - Windows：凭据管理器（DPAPI 保护）
/// - macOS：Keychain
/// - Linux/边缘：`<data_dir>/secrets` 下的 0600 文件（无头设备通常无 Secret Service）
pub fn default_store() -> Result<std::sync::Arc<dyn SecretStore>> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        Ok(std::sync::Arc::new(SystemStore))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Ok(std::sync::Arc::new(FileStore::default_location()?))
    }
}
