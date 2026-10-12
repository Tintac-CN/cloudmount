//! 平台数据目录解析（零依赖，按 OS 惯例）。
//!
//! | 平台 | 数据目录 |
//! |---|---|
//! | Windows | `%APPDATA%\cloudmount` |
//! | macOS | `~/Library/Application Support/cloudmount` |
//! | Linux/边缘 | `$XDG_DATA_HOME/cloudmount` 或 `~/.local/share/cloudmount` |
//!
//! 可用环境变量 `CLOUDMOUNT_DATA_DIR` 覆盖（容器/边缘部署常用）。

use crate::{Error, Result};
use std::path::PathBuf;

/// 应用数据目录（token、缓存索引、日志等）。
pub fn data_dir() -> Result<PathBuf> {
    data_dir_with(std::env::var_os("CLOUDMOUNT_DATA_DIR"))
}

/// 数据目录解析的实际实现（`override_dir` 便于测试注入，避免改全局环境变量）。
fn data_dir_with(override_dir: Option<std::ffi::OsString>) -> Result<PathBuf> {
    if let Some(dir) = override_dir {
        return Ok(PathBuf::from(dir));
    }

    #[cfg(windows)]
    {
        let base =
            std::env::var_os("APPDATA").ok_or_else(|| Error::Other("APPDATA 未设置".into()))?;
        Ok(PathBuf::from(base).join("cloudmount"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or_else(|| Error::Other("HOME 未设置".into()))?;
        Ok(PathBuf::from(home)
            .join("Library/Application Support")
            .join("cloudmount"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            Ok(PathBuf::from(xdg).join("cloudmount"))
        } else {
            let home =
                std::env::var_os("HOME").ok_or_else(|| Error::Other("HOME 未设置".into()))?;
            Ok(PathBuf::from(home).join(".local/share/cloudmount"))
        }
    }
}

/// 确保目录存在并返回。
pub fn ensure_data_dir() -> Result<PathBuf> {
    let dir = data_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 本地缓存目录（内容块缓存）。
pub fn cache_dir() -> Result<PathBuf> {
    Ok(ensure_data_dir()?.join("cache"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dir_ends_with_app_name() {
        // 不依赖具体平台：末尾必须是 cloudmount
        let d = data_dir().expect("应能确定数据目录");
        assert!(
            d.ends_with("cloudmount"),
            "数据目录应以 cloudmount 结尾，实际: {}",
            d.display()
        );
    }

    #[test]
    fn override_dir_takes_precedence() {
        let d = data_dir_with(Some("/custom/cloudmount-data".into())).unwrap();
        assert_eq!(d, PathBuf::from("/custom/cloudmount-data"));
    }

    #[test]
    fn no_override_falls_back_to_platform_default() {
        let d = data_dir_with(None).unwrap();
        assert!(d.ends_with("cloudmount"));
    }
}
