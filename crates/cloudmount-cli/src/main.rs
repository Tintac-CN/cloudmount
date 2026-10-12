//! CloudMount 命令行入口。
//!
//! 这是 M0 阶段的验证工具：用最短路径打通
//! **扫码登录 → 列目录 → 秒传上传 → Range 下载**。
//!
//! ```text
//! cloudmount login            # 扫码登录（官方 OpenAPI PKCE）
//! cloudmount ls /             # 列出根目录
//! cloudmount upload <本地文件> <远端目录>
//! cloudmount download <远端文件> <本地路径>
//! cloudmount quota            # 查询空间
//! ```

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cloudmount",
    version,
    about = "把 115 网盘变成你所有设备上的同步盘（开源、轻量）",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 使用 115 App 扫码登录（官方 OpenAPI，推荐）。
    Login {
        /// 额外启用 Cookie 补充通道（违反 115 服务协议，默认关闭）。
        #[arg(long)]
        with_cookie_channel: bool,
    },
    /// 列出远端目录。
    Ls {
        /// 远端目录 ID（根目录为 0）。
        #[arg(default_value = "0")]
        dir: String,
    },
    /// 上传本地文件（支持秒传判定）。
    Upload {
        /// 本地文件路径。
        path: std::path::PathBuf,
        /// 目标目录 ID。
        #[arg(long, default_value = "0")]
        to: String,
    },
    /// 下载远端文件。
    Download {
        /// 远端文件 ID。
        id: String,
        /// 本地输出路径。
        out: std::path::PathBuf,
    },
    /// 查询空间配额与账号状态。
    Quota,
    /// （边缘设备）以服务模式运行：WebDAV 出口 + 后台同步。
    Serve {
        /// 监听地址。
        #[arg(long, default_value = "127.0.0.1:8384")]
        listen: String,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Login {
            with_cookie_channel,
        } => {
            if with_cookie_channel {
                eprintln!(
                    "⚠️  Cookie 补充通道违反《115 生活用户服务协议》(3.2.3(4)/4.2(11)/5.4)，\
                     可能导致封号/限速/多端互踢。请确认你理解并接受这些风险。"
                );
            }
            // TODO(M0): OpenApiAuth::begin_login + 打印二维码 + poll_login
            anyhow::bail!("login: 待 M0 实现（官方 OpenAPI 设备码扫码）")
        }
        Commands::Ls { dir } => {
            let _ = dir;
            anyhow::bail!("ls: 待 M0 实现")
        }
        Commands::Upload { path, to } => {
            let _ = (path, to);
            anyhow::bail!("upload: 待 M0 实现（秒传 + OSS 分片直传）")
        }
        Commands::Download { id, out } => {
            let _ = (id, out);
            anyhow::bail!("download: 待 M0 实现（downurl + Range 并发）")
        }
        Commands::Quota => anyhow::bail!("quota: 待 M0 实现"),
        Commands::Serve { listen } => {
            let _ = listen;
            anyhow::bail!("serve: 待 M4 实现（dav-server-rs WebDAV 出口）")
        }
    }
}
