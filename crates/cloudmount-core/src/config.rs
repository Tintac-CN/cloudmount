//! 用户配置文件（TOML）。
//!
//! 三个二进制（cloudmountd / cloudmount / egui 桌面壳）共用同一份配置，
//! 位于[数据目录](crate::paths)`config.toml`：
//!
//! | 平台 | 路径 |
//! |---|---|
//! | Windows | `%APPDATA%\cloudmount\config.toml` |
//! | macOS | `~/Library/Application Support/cloudmount/config.toml` |
//! | Linux | `$XDG_DATA_HOME/cloudmount/config.toml`（默认 `~/.local/share/...`） |
//!
//! 可用 `CLOUDMOUNT_DATA_DIR` 重定向（容器/边缘部署）。
//!
//! # 优先级
//!
//! ```text
//! CLI flag > 环境变量 > 配置文件 > 内置默认
//! ```
//!
//! 环境变量（`CLOUDMOUNT_APP_ID` / `CLOUDMOUNT_SYNC_ROOT` / `CLOUDMOUNT_CHANNEL`）
//! 保持向后兼容——配置文件是它们的持久化形态，不是替代。

use crate::{Error, Result};
use std::path::PathBuf;

/// 根配置。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// 115 开放平台 AppID（官方通道；申请 https://open.115.com/ ）。
    pub app_id: Option<String>,
    /// 本地同步根目录（占位符挂载 / 双向同步）。
    pub sync_root: Option<String>,
    /// 数据通道：`auto`（官方为主 Cookie 兜底，默认）/ `open` / `cookie`。
    pub channel: String,
    /// [serve] 监听地址。
    pub listen: String,
    /// 远端根目录 ID（0 = 账号根）。
    pub remote_root: String,
    /// 同步轮次间隔（秒）。
    pub interval: u64,
    /// serve 模式子配置。
    pub serve: ServeConfig,
    /// 同步策略子配置。
    pub sync: SyncConfig,
    /// Cookie 补充通道子配置（默认双 false = 通道关闭）。
    pub cookie: CookieChannelConfig,
}

/// Cookie 补充通道子配置（F-05：**默认关闭 + 显式开启 + 知情同意**）。
///
/// 缺段 = 双 false = 通道关闭（serde(default)，旧配置文件完全兼容）；
/// 仅 `enabled=true` 而未同意仍拒绝启用。同意仅由登录交互产生
/// （CLI `login --with-cookie-channel` / desktop 登录勾选）并经
/// [`set_cookie_consent`] 持久化于此；daemon/desktop 引擎是消费者、
/// 永不自产同意。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CookieChannelConfig {
    /// 用户是否已显式开启（默认 false）。
    pub enabled: bool,
    /// 用户是否已确认知情同意（默认 false）。
    pub consent_acknowledged: bool,
}

/// Cookie 同意写回纯函数（F-05）：`[cookie]` 双字段置真。
///
/// 写回方仅两处——CLI `cmd_login_cookie` 与 desktop 登录成功回调：
/// `load` → 本函数 → `save`/`save_to`（原子写）。
pub fn set_cookie_consent(cfg: &mut Config) {
    cfg.cookie.enabled = true;
    cfg.cookie.consent_acknowledged = true;
}

/// serve（边缘网关）子配置。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServeConfig {
    /// WebDAV Basic 认证（`user:pass`；空 = 无认证，仅建议内网）。
    pub auth: Option<String>,
    /// web-ui 静态目录（空 = 不托管 UI）。
    pub web_ui_dir: Option<String>,
    /// TLS 证书 PEM（与 tls_key 成对启用 HTTPS）。
    pub tls_cert: Option<String>,
    /// TLS 私钥 PEM。
    pub tls_key: Option<String>,
}

/// 同步策略子配置。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyncConfig {
    /// 传播远端删除（默认 false：本地保留并重新上传）。
    pub propagate_remote_deletes: bool,
    /// 传播本地删除（默认 false：远端保留并重新下载）。
    pub propagate_local_deletes: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_id: None,
            sync_root: None,
            channel: "auto".into(),
            listen: "127.0.0.1:8384".into(),
            remote_root: "0".into(),
            interval: 30,
            serve: ServeConfig::default(),
            sync: SyncConfig::default(),
            cookie: CookieChannelConfig::default(),
        }
    }
}

/// 配置文件路径（数据目录下 `config.toml`）。
pub fn config_path() -> Result<PathBuf> {
    Ok(crate::paths::ensure_data_dir()?.join("config.toml"))
}

/// 加载配置（默认位置）：文件不存在 → 默认值（首次运行是正常路径）。
pub fn load() -> Result<Config> {
    load_from(&config_path()?)
}

/// [`load`] 的路径注入变体（测试用；文件不存在 → 默认值）。
///
/// 文件存在但解析失败 → 报错（坏配置不应静默吞掉）。
pub fn load_from(path: &std::path::Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text)
        .map_err(|e| Error::Other(format!("配置文件解析失败 {}: {e}", path.display())))
}

/// 保存配置（默认位置；原子写：先写临时文件再改名，避免半写状态）。
pub fn save(cfg: &Config) -> Result<()> {
    save_to(&config_path()?, cfg)
}

/// [`save`] 的路径注入变体（测试用）。
pub fn save_to(path: &std::path::Path, cfg: &Config) -> Result<()> {
    let text =
        toml::to_string_pretty(cfg).map_err(|e| Error::Other(format!("配置序列化失败: {e}")))?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, &text)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// 环境变量覆盖（优先级：env > 文件）。
///
/// 返回覆盖后的副本；`CLOUDMOUNT_APP_ID` / `CLOUDMOUNT_SYNC_ROOT` /
/// `CLOUDMOUNT_CHANNEL` 三个变量保持向后兼容。
pub fn apply_env_overrides(mut cfg: Config) -> Config {
    if let Ok(v) = std::env::var("CLOUDMOUNT_APP_ID") {
        if !v.is_empty() {
            cfg.app_id = Some(v);
        }
    }
    if let Ok(v) = std::env::var("CLOUDMOUNT_SYNC_ROOT") {
        if !v.is_empty() {
            cfg.sync_root = Some(v);
        }
    }
    if let Ok(v) = std::env::var("CLOUDMOUNT_CHANNEL") {
        if !v.is_empty() {
            cfg.channel = v;
        }
    }
    cfg
}

/// 便捷入口：加载 + 环境变量覆盖。
pub fn load_effective() -> Result<Config> {
    Ok(apply_env_overrides(load()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cm-cfg-{tag}-{}-{:?}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ))
    }

    #[test]
    fn default_roundtrip() {
        let path = temp_config("rt");
        let cfg = Config {
            app_id: Some("app-123".into()),
            sync_root: Some("D:/Cloud".into()),
            channel: "cookie".into(),
            listen: "0.0.0.0:8384".into(),
            ..Default::default()
        };
        save_to(&path, &cfg).expect("保存");
        let loaded = load_from(&path).expect("加载");
        assert_eq!(loaded, cfg, "往返应无损");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_returns_default() {
        let path = temp_config("absent");
        let cfg = load_from(&path).expect("无文件应回默认");
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.channel, "auto");
        assert_eq!(cfg.listen, "127.0.0.1:8384");
    }

    #[test]
    fn malformed_file_errors() {
        let path = temp_config("bad");
        std::fs::write(&path, "not = [valid").unwrap();
        let err = load_from(&path).unwrap_err();
        assert!(format!("{err}").contains("解析失败"), "错误信息: {err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_field_rejected() {
        let path = temp_config("unknown");
        std::fs::write(&path, "no_such_field = 1\n").unwrap();
        assert!(
            load_from(&path).is_err(),
            "未知字段应报错（防拼写错误静默失效）"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// env var 是进程级的——两个 env 测试必须互斥（并行 set/remove 同一
    /// 变量会竞态：CI Windows 上实际复现）。
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn env_overrides_file() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = temp_config("env");
        save_to(
            &path,
            &Config {
                app_id: Some("from-file".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let loaded = load_from(&path).unwrap();
        std::env::set_var("CLOUDMOUNT_APP_ID", "from-env");
        let effective = apply_env_overrides(loaded);
        std::env::remove_var("CLOUDMOUNT_APP_ID");
        assert_eq!(
            effective.app_id.as_deref(),
            Some("from-env"),
            "env 应覆盖文件"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_env_keeps_file_value() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = temp_config("empty-env");
        save_to(
            &path,
            &Config {
                app_id: Some("from-file".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let loaded = load_from(&path).unwrap();
        std::env::set_var("CLOUDMOUNT_APP_ID", "");
        let effective = apply_env_overrides(loaded);
        std::env::remove_var("CLOUDMOUNT_APP_ID");
        assert_eq!(
            effective.app_id.as_deref(),
            Some("from-file"),
            "空 env 不应覆盖（CI 常见空变量）"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn atomic_write_no_tmp_residue() {
        let path = temp_config("atomic");
        save_to(&path, &Config::default()).unwrap();
        // 只查本路径对应的临时文件（临时目录里其它测试的 .tmp 并行存在是正常的）
        let tmp = path.with_extension("toml.tmp");
        assert!(!tmp.exists(), "原子写不应留自己的临时文件");
        assert!(path.exists(), "正式文件应在");
        let _ = std::fs::remove_file(&path);
    }

    // ---- F-05 / T-27：Cookie 通道同意写回（set_cookie_consent 纯函数）----

    /// 含 [cookie] 段的配置 → 调用后双字段覆写为真、其余段不受影响。
    #[test]
    fn set_cookie_consent_overwrites_existing_section() {
        let path = temp_config("f05-overwrite");
        save_to(
            &path,
            &Config {
                app_id: Some("app-keep".into()),
                cookie: CookieChannelConfig {
                    enabled: false,
                    consent_acknowledged: false,
                },
                ..Default::default()
            },
        )
        .unwrap();
        let mut cfg = load_from(&path).expect("加载");
        assert!(!cfg.cookie.enabled && !cfg.cookie.consent_acknowledged);
        set_cookie_consent(&mut cfg);
        assert!(cfg.cookie.enabled, "enabled 必须置真");
        assert!(
            cfg.cookie.consent_acknowledged,
            "consent_acknowledged 必须置真"
        );
        assert_eq!(cfg.app_id.as_deref(), Some("app-keep"), "其余段不受影响");
        let _ = std::fs::remove_file(&path);
    }

    /// 缺段的旧版配置文件 → 负基线缺段双 false（通道关，serde default）；
    /// 调用后写入 [cookie] 段双真，save → reload 往返保持、其余字段无损。
    #[test]
    fn set_cookie_consent_missing_section_write_and_roundtrip() {
        let path = temp_config("f05-missing");
        // 旧版文件形态：无 [cookie] 段（deny_unknown_fields 兼容）
        std::fs::write(&path, "app_id = \"legacy-app\"\nchannel = \"auto\"\n").unwrap();
        let mut cfg = load_from(&path).expect("旧版文件必须可解析");
        assert!(
            !cfg.cookie.enabled && !cfg.cookie.consent_acknowledged,
            "负基线：缺段 = 双 false（通道关，serde default）"
        );
        set_cookie_consent(&mut cfg);
        assert_eq!(
            cfg.cookie,
            CookieChannelConfig {
                enabled: true,
                consent_acknowledged: true,
            },
            "缺段写入后应双真"
        );
        save_to(&path, &cfg).expect("保存");
        let reloaded = load_from(&path).expect("重载");
        assert_eq!(
            reloaded.cookie,
            CookieChannelConfig {
                enabled: true,
                consent_acknowledged: true,
            },
            "reload 往返必须保持双真"
        );
        assert_eq!(reloaded.app_id.as_deref(), Some("legacy-app"), "往返无损");
        assert_eq!(reloaded.channel, "auto", "往返无损");
        let _ = std::fs::remove_file(&path);
    }
}
