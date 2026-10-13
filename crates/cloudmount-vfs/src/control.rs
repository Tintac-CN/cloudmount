//! daemon 控制通道：本地回环 TCP + 端口文件。
//!
//! **为什么需要它**：`CfDehydratePlaceholder` 等 API 是"同步提供方专属"——
//! 只有持有 `CfConnectSyncRoot` 连接的进程（daemon）才能调用；CLI 等
//! 独立进程直接调用会被 `0x8007016A`（provider not running）拒绝。
//! 控制通道把这些请求转发给 daemon 代为执行。
//!
//! **协议**：行式、UTF-8、`\n` 结尾，一连接一命令。
//! - 请求：`PING` / `DEHYDRATE <绝对路径>`
//! - 响应：`OK [详情]` / `ERR <原因>`
//!
//! **端口文件**：一行 `端口 PID 同步根`，daemon 启动时写入、退出时删除，
//! 位于数据目录（用户级权限）。CLI 据此发现 daemon。
//!
//! 传输用 std TCP（跨平台，Linux/macOS 的边缘模式将来复用同一通道）。

use cloudmount_core::{Error, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 默认端口文件位置（数据目录下；daemon 与 CLI 的会合点）。
pub fn default_port_file() -> Result<PathBuf> {
    Ok(cloudmount_core::paths::ensure_data_dir()?.join("control.port"))
}

/// 运行中的控制服务。
///
/// 专用线程上 accept（非阻塞轮询，便于及时停止）。`stop()` 会 join 线程
/// 并删除端口文件——**必须在卸载挂载之前**调用：handler 可能持有挂载
/// 句柄的克隆（`Arc::try_unwrap` 需要它先释放）。
pub struct ControlServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ControlServer {
    /// 监听端口（日志展示用）。
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 启动控制服务。
    ///
    /// `handler(cmd, arg)` 返回单行响应（`OK ...` / `ERR ...`，不含换行）。
    /// `port_file` 的父目录必须已存在（[`default_port_file`] 已保证）。
    pub fn start<F>(port_file: &Path, sync_root: &Path, handler: F) -> Result<Self>
    where
        F: Fn(&str, &str) -> String + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| Error::Other(format!("控制通道绑定失败: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| Error::Other(format!("控制通道端口解析失败: {e}")))?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|e| Error::Other(format!("控制通道非阻塞设置失败: {e}")))?;

        // 端口文件：CLI 的发现机制（端口 + PID + 同步根，便于诊断）
        std::fs::write(
            port_file,
            format!("{port} {} {}\n", std::process::id(), sync_root.display()),
        )?;

        let handler = Arc::new(handler);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let port_file = port_file.to_path_buf();

        let thread = std::thread::Builder::new()
            .name("cm-control".into())
            .spawn(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let h = handler.clone();
                            if let Err(e) = serve(stream, &*h) {
                                tracing::debug!(error = %e, "控制连接处理失败");
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "控制通道 accept 失败，退出");
                            break;
                        }
                    }
                }
                let _ = std::fs::remove_file(&port_file);
            })
            .map_err(|e| Error::Other(format!("控制线程启动失败: {e}")))?;

        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }

    /// 停止服务：置停止标志、join 线程、删除端口文件。
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// 处理一条连接：读一行命令 → handler → 写回一行响应。
fn serve(mut stream: TcpStream, handler: &dyn Fn(&str, &str) -> String) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    stream.set_nodelay(true)?;

    let mut line = String::new();
    {
        let mut reader = BufReader::new(&mut stream);
        reader.read_line(&mut line)?;
    }
    let line = line.trim_end();
    let (cmd, arg) = match line.split_once(' ') {
        Some((c, a)) => (c, a),
        None => (line, ""),
    };
    let resp = if cmd.is_empty() {
        "ERR empty-command".to_string()
    } else {
        handler(cmd, arg)
    };
    writeln!(stream, "{resp}")
}

/// 向端口文件指向的 daemon 发送一条命令，返回响应行（去掉换行）。
pub fn send(port_file: &Path, cmd: &str, arg: &str) -> Result<String> {
    let content = std::fs::read_to_string(port_file)
        .map_err(|_| Error::Other("daemon 未运行（控制端口文件不存在）".into()))?;
    let port: u16 = content
        .split_whitespace()
        .next()
        .and_then(|p| p.parse().ok())
        .ok_or_else(|| Error::Other("端口文件损坏（无法解析端口）".into()))?;

    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(3),
    )
    .map_err(|_| {
        Error::Other("daemon 未运行（连接被拒绝；可能异常退出留下陈旧端口文件）".into())
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|e| Error::Other(format!("控制连接设置超时失败: {e}")))?;
    stream.set_nodelay(true).ok();

    let req = if arg.is_empty() {
        format!("{cmd}\n")
    } else {
        format!("{cmd} {arg}\n")
    };
    stream
        .write_all(req.as_bytes())
        .map_err(|e| Error::Other(format!("控制命令发送失败: {e}")))?;

    let mut resp = String::new();
    let mut reader = BufReader::new(stream);
    reader
        .read_line(&mut resp)
        .map_err(|e| Error::Other(format!("控制响应读取失败: {e}")))?;
    Ok(resp.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试独立的端口文件（测试并行，不能用全局默认位置）。
    fn temp_port_file(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cm-control-{tag}-{}.port", std::process::id()))
    }

    #[test]
    fn roundtrip_ping_and_unknown_command() {
        let pf = temp_port_file("rt");
        let _ = std::fs::remove_file(&pf);
        let srv = ControlServer::start(&pf, Path::new("/tmp/root"), |cmd, arg| match cmd {
            "PING" => format!("OK pong:{arg}"),
            _ => "ERR unknown-command".into(),
        })
        .expect("启动控制服务");

        let resp = send(&pf, "PING", "hello").expect("PING 应有响应");
        assert_eq!(resp, "OK pong:hello");

        let resp = send(&pf, "NOPE", "").expect("未知命令也应有响应");
        assert_eq!(resp, "ERR unknown-command");

        srv.stop();
        assert!(!pf.exists(), "停止后端口文件应删除");
    }

    #[test]
    fn port_file_records_port_pid_and_root() {
        let pf = temp_port_file("meta");
        let _ = std::fs::remove_file(&pf);
        let srv = ControlServer::start(&pf, Path::new("D:/Cloud"), |_, _| "OK".into())
            .expect("启动控制服务");

        let content = std::fs::read_to_string(&pf).expect("端口文件应写入");
        let parts: Vec<&str> = content.split_whitespace().collect();
        assert_eq!(parts.len(), 3, "端口文件应为 端口/PID/同步根 三段");
        assert_eq!(parts[0].parse::<u16>().unwrap(), srv.port());
        assert_eq!(parts[1].parse::<u32>().unwrap(), std::process::id());
        assert_eq!(parts[2], "D:/Cloud");

        srv.stop();
    }

    #[test]
    fn send_fails_cleanly_without_port_file() {
        let pf = temp_port_file("absent");
        let _ = std::fs::remove_file(&pf);
        let err = send(&pf, "PING", "").expect_err("无端口文件应报错");
        assert!(format!("{err}").contains("daemon 未运行"));
    }
}
