//! Cookie 通道 mock HTTP 服务器（共享模块，非独立测试目标）。
//!
//! 扩展 `mock_api.rs` 的 `spawn_mock`/`Recorded` 模式（F-14 / T-23 / T-24 宿主）：
//! - method+path 路由、可编程状态码与 body；
//! - **二进制 body 变体**（initupload 的 ECDH 加密响应、OSS XML 错误体）；
//! - 请求录制（方法 / 含查询串路径 / 头 / 原始字节 body）；
//! - `MockResponse::abandon()`：不发响应直接断连（传输错误注入）。
//!
//! 仅服务本地明文 HTTP；所有测试零外部网络、零真实凭据。

#![allow(dead_code)] // 共享库：部分助手仅被个别用例使用

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// 一次被记录的请求（body 保留原始字节）。
#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    /// 含查询串的路径，如 `/4.0/initupload.php?k_ec=…`。
    pub path: String,
    /// 头名已小写。
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    /// 路径部分（不含查询串）。
    pub fn path_only(&self) -> &str {
        self.path.split('?').next().unwrap_or("")
    }

    /// 查询参数（原样字节；base64 值的 `+`/`/`/`=` 不做解码）。
    pub fn query(&self, name: &str) -> Option<String> {
        self.path.split_once('?').and_then(|(_, q)| {
            q.split('&').find_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                (k == name).then(|| v.to_string())
            })
        })
    }

    /// body 的 lossy UTF-8 形态。
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

/// 可编程响应（状态码 + 原始字节 body）。
pub struct MockResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: &'static str,
}

impl MockResponse {
    /// JSON 响应。
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into().into_bytes(),
            content_type: "application/json",
        }
    }

    /// 原始字节响应（ECDH 密文、OSS XML 等二进制形态）。
    pub fn bytes(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            body,
            content_type: "application/octet-stream",
        }
    }

    /// 不发任何响应、直接断连（模拟传输错误）。
    pub fn abandon() -> Self {
        Self {
            status: 0,
            body: Vec::new(),
            content_type: "",
        }
    }
}

type Responder = dyn Fn(&Recorded) -> MockResponse + Send + Sync;

/// 先绑定一个随机端口（便于响应里引用自身地址），再启动服务线程。
pub fn bind_ephemeral() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("local_addr");
    (listener, addr)
}

/// 在已绑定的 listener 上服务请求。
pub fn serve_on<F>(listener: TcpListener, responder: F) -> Arc<Mutex<Vec<Recorded>>>
where
    F: Fn(&Recorded) -> MockResponse + Send + Sync + 'static,
{
    let records = Arc::new(Mutex::new(Vec::new()));
    let records_for_thread = records.clone();
    let responder: Arc<Responder> = Arc::new(responder);

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let records = records_for_thread.clone();
            let responder = responder.clone();
            std::thread::spawn(move || {
                let _ = handle_connection(&mut stream, &records, &*responder);
            });
        }
    });

    records
}

/// 便捷封装：绑定 + 服务，一次完成。
pub fn spawn_mock<F>(responder: F) -> (SocketAddr, Arc<Mutex<Vec<Recorded>>>)
where
    F: Fn(&Recorded) -> MockResponse + Send + Sync + 'static,
{
    let (listener, addr) = bind_ephemeral();
    let records = serve_on(listener, responder);
    (addr, records)
}

fn handle_connection(
    stream: &mut TcpStream,
    records: &Mutex<Vec<Recorded>>,
    responder: &Responder,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            headers.push((k.trim().to_lowercase(), v.trim().to_string()));
        }
    }

    let content_length: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    let recorded = Recorded {
        method,
        path,
        headers,
        body,
    };
    records.lock().expect("lock").push(recorded.clone());

    let resp = responder(&recorded);
    if resp.status == 0 {
        // abandon：不发响应直接断连 → 客户端侧传输错误
        return Ok(());
    }
    let head = format!(
        "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        resp.status,
        resp.content_type,
        resp.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&resp.body)?;
    stream.flush()
}
