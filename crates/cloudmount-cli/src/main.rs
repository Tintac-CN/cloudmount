//! CloudMount 命令行入口（M0 验证工具）。
//!
//! 最短路径验证官方 OpenAPI 全链路：
//! **扫码登录 → 列目录 → 秒传上传 → Range 下载**。
//!
//! ```text
//! export CLOUDMOUNT_APP_ID=<开放平台 AppID>     # 或 --app-id
//! cloudmount login                # 终端二维码 + 轮询
//! cloudmount ls 0                 # 列出根目录
//! cloudmount quota                # 空间配额
//! cloudmount upload ./report.pdf --to 0
//! cloudmount download <file_id> ./report.pdf
//! ```

use clap::{Parser, Subcommand};
use cloudmount_auth::open::OpenApiAuth;
use cloudmount_auth::store::default_store;
use cloudmount_auth::Authenticator as _;
use cloudmount_core::{EntryKind, Provider, RangeSpec, RemoteId, UploadSource};
use cloudmount_driver115::Open115Provider;
use std::io::Write;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "cloudmount",
    version,
    about = "把 115 网盘变成你所有设备上的同步盘（开源、轻量）",
    long_about = None
)]
struct Cli {
    /// 115 开放平台 AppID（也可用环境变量 CLOUDMOUNT_APP_ID）。
    #[arg(long, global = true, env = "CLOUDMOUNT_APP_ID")]
    app_id: Option<String>,

    /// 日志级别（error/warn/info/debug/trace）。
    #[arg(long, global = true, env = "CLOUDMOUNT_LOG", default_value = "info")]
    log: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 用 115 App 扫码登录（官方 OpenAPI，推荐）。
    Login {
        /// 额外启用 Cookie 补充通道（违反 115 服务协议，默认关闭）。
        #[arg(long)]
        with_cookie_channel: bool,
    },
    /// 显示当前登录状态。
    Status,
    /// 登出并删除本地凭据。
    Logout,
    /// 列出远端目录。
    Ls {
        /// 目录 ID（根目录为 0）。
        #[arg(default_value = "0")]
        dir: String,
    },
    /// 查询空间配额。
    Quota,
    /// 上传本地文件（自动尝试秒传）。
    Upload {
        /// 本地文件路径。
        path: std::path::PathBuf,
        /// 目标目录 ID。
        #[arg(long, default_value = "0")]
        to: String,
        /// 远端文件名（默认与本地同名）。
        #[arg(long)]
        name: Option<String>,
    },
    /// 下载远端文件。
    Download {
        /// 远端文件 ID（fid）。
        id: String,
        /// 本地输出路径。
        out: std::path::PathBuf,
    },
    /// [仅 Windows] 释放空间：把已水合的占位符文件脱水（保留元数据）。
    #[cfg(windows)]
    Dehydrate {
        /// 本地文件路径（须在已挂载的同步根下）。
        path: std::path::PathBuf,
    },

    /// （边缘设备）以服务模式运行：WebDAV 出口 + 后台同步。
    Serve {
        /// 监听地址。
        #[arg(long, default_value = "127.0.0.1:8384")]
        listen: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log.clone().into()),
        )
        .init();

    match cli.command {
        Commands::Login {
            with_cookie_channel,
        } => {
            if with_cookie_channel {
                eprintln!(
                    "⚠️  Cookie 补充通道违反《115 生活用户服务协议》(3.2.3(4)/4.2(11)/5.4)，\n\
                     可能导致封号/限速/多端互踢。Cookie 通道将在 M3 实现。"
                );
            }
            cmd_login(&cli.app_id)
        }
        Commands::Status => cmd_status(&cli.app_id),
        Commands::Logout => cmd_logout(),
        Commands::Ls { dir } => cmd_ls(&cli.app_id, &dir),
        Commands::Quota => cmd_quota(&cli.app_id),
        Commands::Upload { path, to, name } => cmd_upload(&cli.app_id, path, to, name),
        Commands::Download { id, out } => cmd_download(&cli.app_id, id, out),
        #[cfg(windows)]
        Commands::Dehydrate { path } => cmd_dehydrate(path),
        Commands::Serve { listen } => {
            let _ = listen;
            anyhow::bail!("serve: 待 M4 实现（dav-server-rs WebDAV 出口）")
        }
    }
}

/// 构造认证器 + 驱动（需要 AppID 的命令共用）。
fn build(app_id: &Option<String>) -> anyhow::Result<(Arc<OpenApiAuth>, Open115Provider)> {
    let app_id = app_id.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "缺少 AppID：请设置环境变量 CLOUDMOUNT_APP_ID 或传 --app-id。\n\
             申请入口: https://open.115.com/ （个人开发者可申请，审核约 7+7 个工作日）"
        )
    })?;
    let store = default_store()?;
    let auth = Arc::new(OpenApiAuth::new(app_id, store));
    let provider = Open115Provider::new(auth.clone());
    Ok((auth, provider))
}

/// 只构造认证器（status/logout 不需要驱动）。
fn build_auth(app_id: &Option<String>) -> anyhow::Result<Arc<OpenApiAuth>> {
    let app_id = app_id.clone().unwrap_or_default();
    let store = default_store()?;
    Ok(Arc::new(OpenApiAuth::new(app_id, store)))
}

fn cmd_login(app_id: &Option<String>) -> anyhow::Result<()> {
    let (auth, _) = build(app_id)?;
    if auth.is_logged_in() {
        println!("已登录（如需重新登录请先执行 `cloudmount logout`）。");
        return Ok(());
    }

    let challenge = auth.begin_login()?;
    println!("请用 115 App 扫描下面的二维码：\n");
    println!("{}\n", challenge.qr_content);
    match render_qr(&challenge.qr_content) {
        Ok(art) => println!("{art}"),
        Err(e) => tracing::debug!("终端二维码渲染失败（可用上面的链接）: {e}"),
    }

    let deadline = challenge.expires_at;
    loop {
        if now_unix() > deadline {
            anyhow::bail!("二维码已过期，请重新执行 login");
        }
        match auth.poll_login(&challenge)? {
            Some(_) => {
                println!("\n✅ 登录成功，凭据已保存到系统凭据库。");
                return Ok(());
            }
            None => {
                // 状态接口为长轮询，本身会阻塞；短暂 sleep 防止空转
                std::thread::sleep(std::time::Duration::from_millis(500));
                print!(".");
                std::io::stdout().flush().ok();
            }
        }
    }
}

fn cmd_status(app_id: &Option<String>) -> anyhow::Result<()> {
    let auth = build_auth(app_id)?;
    match auth.session() {
        Some(s) => println!(
            "登录方式: {:?}\n状态: {}",
            s.method,
            if s.valid {
                "有效"
            } else {
                "临近过期，下次请求会自动刷新"
            }
        ),
        None => println!("未登录"),
    }
    Ok(())
}

fn cmd_logout() -> anyhow::Result<()> {
    let auth = build_auth(&None)?;
    auth.logout()?;
    println!("已清除本地凭据。");
    Ok(())
}

fn cmd_ls(app_id: &Option<String>, dir: &str) -> anyhow::Result<()> {
    let (_, provider) = build(app_id)?;
    let entries = provider
        .list(&RemoteId(dir.to_string()))
        .map_err(|e| anyhow::anyhow!("列目录失败: {e}"))?;

    println!("{:<6} {:>12}  {:<19}  名称", "类型", "大小", "修改时间");
    println!("{}", "-".repeat(72));
    for e in &entries {
        let kind = match e.kind {
            EntryKind::Directory => "目录",
            EntryKind::File => "文件",
        };
        let size = if e.kind == EntryKind::Directory {
            "-".to_string()
        } else {
            human_size(e.size)
        };
        println!(
            "{:<6} {:>12}  {:<19}  {}  [{}]",
            kind,
            size,
            format_time(e.mtime),
            e.name,
            e.id
        );
    }
    println!("\n共 {} 项", entries.len());
    Ok(())
}

fn cmd_quota(app_id: &Option<String>) -> anyhow::Result<()> {
    let (_, provider) = build(app_id)?;
    let q = provider
        .quota()
        .map_err(|e| anyhow::anyhow!("查询配额失败: {e}"))?;
    println!("总空间: {}", human_size(q.total));
    println!("已使用: {}", human_size(q.used));
    println!("剩余:   {}", human_size(q.remaining));
    if q.total > 0 {
        println!("使用率: {:.1}%", q.used as f64 / q.total as f64 * 100.0);
    }
    Ok(())
}

fn cmd_upload(
    app_id: &Option<String>,
    path: std::path::PathBuf,
    to: String,
    name: Option<String>,
) -> anyhow::Result<()> {
    let (_, provider) = build(app_id)?;
    let meta = std::fs::metadata(&path)
        .map_err(|e| anyhow::anyhow!("读取 {} 失败: {e}", path.display()))?;
    if !meta.is_file() {
        anyhow::bail!("{} 不是普通文件", path.display());
    }
    let file_name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    println!(
        "上传 {} ({}) → 目录 {} ...",
        file_name,
        human_size(meta.len()),
        to
    );
    let source = UploadSource::Path(&path);
    let result = provider
        .upload(&RemoteId(to), &file_name, source, meta.len())
        .map_err(|e| anyhow::anyhow!("上传失败: {e}"))?;

    if result.rapid_upload {
        println!("⚡ 秒传命中（零传输完成）");
    } else {
        println!("✅ 上传完成");
    }
    println!("file_id: {}", result.id.0);
    Ok(())
}

fn cmd_download(
    app_id: &Option<String>,
    id: String,
    out: std::path::PathBuf,
) -> anyhow::Result<()> {
    let (_, provider) = build(app_id)?;
    let entry = provider
        .stat(&RemoteId(id.clone()))
        .map_err(|e| anyhow::anyhow!("查询文件信息失败: {e}"))?;
    println!(
        "下载 {} ({}) → {}",
        entry.name,
        human_size(entry.size),
        out.display()
    );

    let mut reader = provider
        .open_range(
            &RemoteId(id),
            RangeSpec {
                start: 0,
                end: None,
            },
        )
        .map_err(|e| anyhow::anyhow!("获取下载流失败: {e}"))?;

    let mut file = std::fs::File::create(&out)
        .map_err(|e| anyhow::anyhow!("创建 {} 失败: {e}", out.display()))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut written: u64 = 0;
    let mut last_report: u64 = 0;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        written += n as u64;
        // 每 16 MiB 汇报一次进度
        if written - last_report >= 16 * 1024 * 1024 {
            last_report = written;
            if entry.size > 0 {
                print!(
                    "\r进度: {} / {} ({:.1}%)",
                    human_size(written),
                    human_size(entry.size),
                    written as f64 / entry.size as f64 * 100.0
                );
            } else {
                print!("\r已下载: {}", human_size(written));
            }
            std::io::stdout().flush().ok();
        }
    }
    println!("\n✅ 下载完成: {} ({})", out.display(), human_size(written));
    Ok(())
}

/// 终端二维码渲染（unicode 半块字符，两行合一行高度）。
fn render_qr(content: &str) -> anyhow::Result<String> {
    use qrcode::render::unicode;
    let code = qrcode::QrCode::new(content.as_bytes())?;
    Ok(code.render::<unicode::Dense1x2>().quiet_zone(true).build())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 人类可读大小。
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", UNITS[i])
    }
}

/// Unix 秒 → `YYYY-MM-DD HH:MM:SS`（UTC）。
fn format_time(secs: i64) -> String {
    if secs <= 0 {
        return "-".to_string();
    }
    match time::OffsetDateTime::from_unix_timestamp(secs) {
        Ok(dt) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.year(),
            dt.month() as u8,
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        ),
        Err(_) => "-".to_string(),
    }
}

#[cfg(windows)]
fn cmd_dehydrate(path: std::path::PathBuf) -> anyhow::Result<()> {
    use cloudmount_vfs::cfapi;

    let path = path
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("文件不存在或不可访问 {}: {e}", path.display()))?;
    let meta = std::fs::metadata(&path)?;
    if !meta.is_file() {
        anyhow::bail!("仅支持文件（目录请逐个处理）: {}", path.display());
    }

    println!("释放空间: {} ({})", path.display(), human_size(meta.len()));
    match cfapi::dehydrate(&path) {
        Ok(()) => {
            println!("✅ 已脱水（本地数据已丢弃，元数据保留；下次读取时重新水合）");
            Ok(())
        }
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("0x8007016A") {
                anyhow::bail!(
                    "脱水需要由同步提供方（daemon）执行——CLI 独立进程无法调用此 API。
                     替代方案：
  ① 资源管理器右键 →「释放空间」（Shell 原生支持）
  ② cloudmountd 内置命令（待 M3）"
                );
            }
            Err(anyhow::anyhow!("脱水失败: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.00 KiB");
        assert_eq!(human_size(1024 * 1024 * 3 / 2), "1.50 MiB");
    }

    #[test]
    fn time_formats_epoch() {
        assert_eq!(format_time(0), "-", "0 视为无效时间");
        assert_eq!(format_time(1), "1970-01-01 00:00:01");
    }
}
