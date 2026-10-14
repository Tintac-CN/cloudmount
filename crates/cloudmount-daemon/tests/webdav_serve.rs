//! WebDAV 出口集成测试：MockRemote 云端 → ProviderFs → dav-server →
//! hyper HTTP → 客户端请求（PROPFIND/GET/PUT/MKCOL/DELETE/MOVE）。
//!
//! 这就是边缘设备挂载的真实链路（局域网设备经 WebDAV 读写 115），
//! MockRemote 换成 Open115Provider/Cookie115Provider 即真实云端。
//!
//! 测试不占固定端口（绑定 127.0.0.1:0 由 OS 分配）。
#![cfg(feature = "mock")]

use cloudmount_daemon::webdav;
use cloudmount_sync::testutil::MockRemote;
use std::path::PathBuf;
use std::sync::Arc;

// ==== U10 harness：web-ui fixture + 任意 Arc<dyn Provider> 注入口 ====

/// 金丝雀内容：位于 serve root（webui/）**之外**，路径遍历一旦成功即泄漏。
const CANARY_SECRET: &str = "CANARY-SECRET-7f3d9a2c-do-not-serve";
/// web-ui fixture：合法资产内容（正路径断言基准，逐字节一致）。
const UI_INDEX_HTML: &[u8] = b"<html>CloudMount UI fixture</html>";
const UI_APP_JS: &[u8] = b"// cloudmount web-ui fixture app.js";

/// web-ui fixture 布局（tempdir）：
///
/// ```text
/// <root>/canary-secret.txt      ← 金丝雀（webui 之外——遍历目标）
/// <root>/webui/index.html       ← serve 的 web_ui_dir 根
/// <root>/webui/assets/app.js
/// ```
struct UiFixture {
    /// tempdir 根（金丝雀所在——**不是** serve root）。
    root: PathBuf,
    /// serve 的 web_ui_dir（= `<root>/webui`）。
    ui_dir: PathBuf,
}

impl UiFixture {
    fn create(tag: &str) -> UiFixture {
        let root = std::env::temp_dir().join(format!(
            "cm-uitest-{}-{}-{}",
            std::process::id(),
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("webui/assets")).unwrap();
        std::fs::write(root.join("webui/index.html"), UI_INDEX_HTML).unwrap();
        std::fs::write(root.join("webui/assets/app.js"), UI_APP_JS).unwrap();
        std::fs::write(root.join("canary-secret.txt"), CANARY_SECRET).unwrap();
        UiFixture {
            ui_dir: root.join("webui"),
            root,
        }
    }
}

impl Drop for UiFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 启动服务并挂 web-ui fixture（serve 第 4 参传 `Some(ui_dir)`，使
/// serve_static 静态路由面可达——现状传 None 时非 /dav 路径直接 404）。
async fn spawn_with_ui(provider: Arc<dyn cloudmount_core::Provider>, ui_dir: PathBuf) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let listen = format!("127.0.0.1:{port}");
    tokio::spawn(async move {
        webdav::serve(provider, &listen, None, Some(ui_dir), None)
            .await
            .ok();
    });
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    port
}

/// 默认 MockRemote + web-ui fixture（T-05 用）。
/// 返回 (端口, MockRemote, fixture)。
async fn start_server_with_ui() -> (u16, Arc<MockRemote>, UiFixture) {
    let remote = Arc::new(MockRemote::new());
    let provider: Arc<dyn cloudmount_core::Provider> = remote.clone();
    let fixture = UiFixture::create("default");
    let port = spawn_with_ui(provider, fixture.ui_dir.clone()).await;
    (port, remote, fixture)
}

/// U11 注入口：任意 `Arc<dyn Provider>`（包装族 ErrorInjecting/Recording）
/// 携带 web-ui fixture，返回 (端口, fixture)。
///
/// 本 unit 先行铺设；U11 施工后由其测试消费（dead_code 抑制至彼时）。
#[allow(dead_code)]
pub(crate) async fn start_server_with_ui_provider(
    provider: Arc<dyn cloudmount_core::Provider>,
) -> (u16, UiFixture) {
    let fixture = UiFixture::create("provider");
    let port = spawn_with_ui(provider, fixture.ui_dir.clone()).await;
    (port, fixture)
}

/// 启动测试服务（随机端口），返回 (端口, MockRemote)。
async fn start_server() -> (u16, Arc<MockRemote>) {
    let remote = Arc::new(MockRemote::new());
    remote.put_file("welcome.txt", b"hello webdav edge gateway");
    remote.put_dir("docs");
    remote.put_file("docs/guide.txt", b"guide content");

    // 绑定临时端口：先绑定拿端口，再在 serve 里用该端口
    // （serve 接口收字符串地址——先探测一个空闲端口）
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let provider: Arc<dyn cloudmount_core::Provider> = remote.clone();
    let listen = format!("127.0.0.1:{port}");
    let server = tokio::spawn(async move {
        webdav::serve(provider, &listen, None, None, None)
            .await
            .ok();
    });
    // 等服务就绪
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    // 丢弃 JoinHandle = detach（server 存活至进程退出）
    std::mem::drop(server);
    (port, remote)
}

/// HTTP 客户端请求（hyper）。
async fn request(port: u16, method: &str, path: &str, body: Option<Vec<u8>>) -> (u16, Vec<u8>) {
    request_headers(port, method, path, body, &[]).await
}

/// 带自定义头的请求。
async fn request_headers(
    port: u16,
    method: &str,
    path: &str,
    body: Option<Vec<u8>>,
    headers: &[(&str, &str)],
) -> (u16, Vec<u8>) {
    use hyper::client::Client;
    use hyper::http::Request;
    let client: Client<hyper::client::HttpConnector, hyper::Body> = Client::new();
    let uri: hyper::Uri = format!("http://127.0.0.1:{port}{path}").parse().unwrap();
    let mut builder = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }
    if body.is_some() {
        builder = builder.header("Content-Type", "application/octet-stream");
    }
    let req = builder
        .body(hyper::Body::from(body.unwrap_or_default()))
        .unwrap();
    let resp = client.request(req).await.unwrap();
    let status = resp.status().as_u16();
    let data = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    (status, data.to_vec())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webdav_full_roundtrip() {
    let (port, remote) = start_server().await;

    // ---- PROPFIND 根目录（Depth: 1 由 dav-server 默认处理）----
    // 用 OPTIONS 先验证服务活着 + WebDAV 方法集
    let (status, _) = request(port, "OPTIONS", "/dav/", None).await;
    assert_eq!(status, 200, "OPTIONS 应 200");

    // ---- GET 文件 ----
    let (status, body) = request(port, "GET", "/dav/welcome.txt", None).await;
    assert_eq!(status, 200, "GET 应 200");
    assert_eq!(body, b"hello webdav edge gateway");

    // ---- GET 嵌套文件（路径解析：/docs/guide.txt）----
    let (status, body) = request(port, "GET", "/dav/docs/guide.txt", None).await;
    assert_eq!(status, 200, "嵌套 GET 应 200");
    assert_eq!(body, b"guide content");

    // ---- GET 不存在的文件 ----
    let (status, _) = request(port, "GET", "/dav/no-such.txt", None).await;
    assert_eq!(status, 404, "不存在应 404");

    // ---- PUT 新文件（spool → upload）----
    let (status, _) = request(
        port,
        "PUT",
        "/dav/uploaded.txt",
        Some(b"uploaded via webdav".to_vec()),
    )
    .await;
    assert_eq!(status, 201, "PUT 应 201 Created");
    // 远端可见
    assert_eq!(remote.read("uploaded.txt").unwrap(), b"uploaded via webdav");

    // ---- PUT 后 GET 回读（缓存失效 + 重解析）----
    let (status, body) = request(port, "GET", "/dav/uploaded.txt", None).await;
    assert_eq!(status, 200);
    assert_eq!(body, b"uploaded via webdav");

    // ---- MKCOL 新目录 ----
    let (status, _) = request(port, "MKCOL", "/dav/newdir", None).await;
    assert_eq!(status, 201, "MKCOL 应 201");

    // ---- PUT 进新目录（多级路径解析）----
    let (status, _) = request(
        port,
        "PUT",
        "/dav/newdir/nested.txt",
        Some(b"nested put".to_vec()),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(remote.read("newdir/nested.txt").unwrap(), b"nested put");

    // ---- DELETE 文件 ----
    let (status, _) = request(port, "DELETE", "/dav/uploaded.txt", None).await;
    assert_eq!(status, 204, "DELETE 应 204");
    let (status, _) = request(port, "GET", "/dav/uploaded.txt", None).await;
    assert_eq!(status, 404, "删除后应 404");

    // ---- MOVE（重命名）----
    let (status, _) = request(port, "MOVE", "/dav/newdir/nested.txt", None).await;
    assert_eq!(status, 400, "MOVE 缺 Destination 头应 400");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webdav_propfind_lists_entries() {
    let (port, _remote) = start_server().await;
    // PROPFIND（规范 body + Depth: 1）
    let propfind = br#"<?xml version="1.0"?><D:propfind xmlns:D="DAV:"><D:allprop/></D:propfind>"#;
    let (status, body) = request_headers(
        port,
        "PROPFIND",
        "/dav/",
        Some(propfind.to_vec()),
        &[("Depth", "1"), ("Content-Type", "application/xml")],
    )
    .await;
    assert!(
        status == 207 || status == 200,
        "PROPFIND 应 207，实际 {status}"
    );
    let text = String::from_utf8_lossy(&body);
    // 目录条目应出现在 multistatus 里
    assert!(text.contains("welcome.txt"), "应包含 welcome.txt: {text}");
    assert!(text.contains("docs"), "应包含 docs");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webdav_auth_rejects_missing_credentials() {
    // 带认证的服务
    let remote = Arc::new(MockRemote::new());
    remote.put_file("secret.txt", b"top secret");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let provider: Arc<dyn cloudmount_core::Provider> = remote.clone();
    let listen = format!("127.0.0.1:{port}");
    tokio::spawn(async move {
        webdav::serve(
            provider,
            &listen,
            Some(("user".into(), "pass".into())),
            None,
            None,
        )
        .await
        .ok();
    });
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // 无认证 → 401
    let (status, _) = request(port, "GET", "/dav/secret.txt", None).await;
    assert_eq!(status, 401, "无认证应 401");

    // 错误认证 → 401
    let (status, _) = request_auth(port, "GET", "/dav/secret.txt", None, "user:wrong").await;
    assert_eq!(status, 401, "错误认证应 401");

    // 正确认证 → 200
    let (status, body) = request_auth(port, "GET", "/dav/secret.txt", None, "user:pass").await;
    assert_eq!(status, 200, "正确认证应 200");
    assert_eq!(body, b"top secret");
}

/// 带 Basic 认证的请求。
async fn request_auth(
    port: u16,
    method: &str,
    path: &str,
    body: Option<Vec<u8>>,
    credentials: &str,
) -> (u16, Vec<u8>) {
    use base64::Engine;
    use hyper::client::Client;
    use hyper::http::Request;
    let client: Client<hyper::client::HttpConnector, hyper::Body> = Client::new();
    let uri: hyper::Uri = format!("http://127.0.0.1:{port}{path}").parse().unwrap();
    let auth = base64::engine::general_purpose::STANDARD.encode(credentials);
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Basic {auth}"))
        .body(hyper::Body::from(body.unwrap_or_default()))
        .unwrap();
    let resp = client.request(req).await.unwrap();
    let status = resp.status().as_u16();
    let data = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    (status, data.to_vec())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_endpoints_respond() {
    let remote = Arc::new(MockRemote::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let provider: Arc<dyn cloudmount_core::Provider> = remote.clone();
    let listen = format!("127.0.0.1:{port}");
    tokio::spawn(async move {
        webdav::serve(provider, &listen, None, None, None)
            .await
            .ok();
    });
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // get_status：JSON 契约（camelCase）
    let (status, body) = request(port, "POST", "/api/get_status", Some(Vec::new())).await;
    assert_eq!(status, 200);
    let text = String::from_utf8_lossy(&body).to_string();
    assert!(text.contains("\"version\""), "应含 version: {text}");
    assert!(text.contains("\"lastMessage\""), "camelCase 契约: {text}");
    assert!(text.contains("\"paused\":false"), "paused 字段: {text}");

    // get_quota：MockRemote 的配额
    let (status, body) = request(port, "POST", "/api/get_quota", Some(Vec::new())).await;
    assert_eq!(status, 200);
    let text = String::from_utf8_lossy(&body).to_string();
    assert!(text.contains("\"total\""), "配额字段: {text}");

    // toggle_pause：切换
    let (status, body) = request(port, "POST", "/api/toggle_pause", Some(Vec::new())).await;
    assert_eq!(status, 200);
    assert_eq!(String::from_utf8_lossy(&body), "true");

    // 未知 API：404
    let (status, _) = request(port, "POST", "/api/no_such", Some(Vec::new())).await;
    assert_eq!(status, 404);

    // export_logs：JSON 数组
    let (status, body) = request(port, "POST", "/api/export_logs", Some(Vec::new())).await;
    assert_eq!(status, 200);
    assert!(
        String::from_utf8_lossy(&body).starts_with("["),
        "日志应为数组"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_ui_static_serving() {
    // 临时 web-ui 目录
    let dir = std::env::temp_dir().join(format!("cm-webui-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), b"<html>CloudMount UI</html>").unwrap();

    let remote = Arc::new(MockRemote::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let provider: Arc<dyn cloudmount_core::Provider> = remote.clone();
    let listen = format!("127.0.0.1:{port}");
    let ui_dir = dir.clone();
    tokio::spawn(async move {
        webdav::serve(provider, &listen, None, Some(ui_dir), None)
            .await
            .ok();
    });
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // 根路径 → index.html
    let (status, body) = request(port, "GET", "/", None).await;
    assert_eq!(status, 200);
    assert!(String::from_utf8_lossy(&body).contains("CloudMount UI"));

    // 目录穿越防护
    let (status, _) = request(port, "GET", "/../etc/passwd", None).await;
    assert!(status == 403 || status == 404, "穿越应被拒: {status}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// T-05（AC3 / F-03）：路径遍历攻击向量全拦截 + 正路径不受影响。
///
/// 向量**不带 /dav/ 前缀**（/dav/* 经 webdav.rs 路由进 ProviderFs，
/// 到不了被修复的 serve_static），直打静态路由面。
///
/// 红测主力 = 裸冒号盘符主向量 `/C:/Windows/win.ini`：修复前 Windows 上
/// `root.join` 遇绝对成分直接替换基路径 → 200 泄漏真实 win.ini 内容。
/// 百分号编码向量（`/C%3A/foo` 等）= 修复后回归面（现状不解码）。
/// hyper Uri 对 `\` 等非 URI 合法字符要求百分号编码——向量一律编码形态发送。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_ui_traversal_vectors_rejected() {
    let (port, _remote, fixture) = start_server_with_ui().await;

    // fixture 自检：金丝雀确在 serve root 之外（防止 fixture 漂移导致测试空转）
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("canary-secret.txt")).unwrap(),
        CANARY_SECRET
    );

    // (向量, 说明)
    let vectors = [
        ("/C:/Windows/win.ini", "盘符绝对路径（红测主向量）"),
        ("/C%3A/foo", "盘符百分号编码（C:）"),
        ("/C%3A%5Cfoo", "盘符+反斜杠百分号编码（C:\\foo）"),
        (
            "/..%2Fcanary-secret.txt",
            "单层 .. 编码——解码后恰达 root 外金丝雀",
        ),
        ("//C:/Windows/win.ini", "空段+盘符"),
        ("//server/share/f", "UNC 前导斜杠形态（纵深防御）"),
        (
            "/%5C%5Cserver%5Cshare%5Cf",
            "UNC 反斜杠编码形态（纵深防御）",
        ),
    ];
    for (vector, why) in vectors {
        let (status, body) = request(port, "GET", vector, None).await;
        assert!(
            status == 403 || status == 404,
            "向量 {vector}（{why}）应 403/404，实际 {status}"
        );
        let text = String::from_utf8_lossy(&body);
        assert!(
            !text.contains("CANARY-SECRET"),
            "向量 {vector}（{why}）泄漏金丝雀: {text}"
        );
        // Windows 上裸盘符向量修复前返回真实 win.ini（92 字节，含以下标记）
        let lower = text.to_lowercase();
        assert!(
            !lower.contains("16-bit app support") && !lower.contains("[fonts]"),
            "向量 {vector}（{why}）疑似泄漏 win.ini 内容: {text}"
        );
    }

    // ---- 正路径（修复不得误伤合法资产）----
    let (status, body) = request(port, "GET", "/", None).await;
    assert_eq!(status, 200, "根路径应 200 → index.html");
    assert_eq!(body, UI_INDEX_HTML, "根路径内容应与 fixture 逐字节一致");

    let (status, body) = request(port, "GET", "/index.html", None).await;
    assert_eq!(status, 200, "/index.html 应 200");
    assert_eq!(body, UI_INDEX_HTML);

    let (status, body) = request(port, "GET", "/assets/app.js", None).await;
    assert_eq!(status, 200, "/assets/app.js 应 200");
    assert_eq!(body, UI_APP_JS);
}
