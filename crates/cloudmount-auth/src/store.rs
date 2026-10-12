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

impl SecretStore for FileStore {
    fn put(&self, kind: SecretKind, secret: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(kind.key().replace('/', "_"));
        std::fs::write(&path, secret)?;
        // TODO: 设置 0600 权限（Unix）；Windows 用 ACL。
        Ok(())
    }

    fn get(&self, kind: SecretKind) -> Result<Option<Vec<u8>>> {
        let path = self.dir.join(kind.key().replace('/', "_"));
        match std::fs::read(&path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, kind: SecretKind) -> Result<()> {
        let path = self.dir.join(kind.key().replace('/', "_"));
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
