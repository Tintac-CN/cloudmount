//! U4 / F-10 双入口单飞刷新 —— T-17 / T-18（TDD 红阶段测试，先于实现提交）。
//!
//! 契约来源：
//! - `docs/aidlc/audit-p0p1-fixes/units/4.md`
//! - `docs/aidlc/audit-p0p1-fixes/architecture.md`（F-10 节 + 关键决策 #7）
//! - `docs/aidlc/audit-p0p1-fixes/test-strategy.md`（T-17 / T-18）
//!
//! 数值口径（canonical）：10 线程、外层循环 10 轮；会合用栅栏（`Barrier`）；
//! 过期直接注入 `TokenSet`，不 sleep 真实时钟（mock responder 内的固定延迟是
//! 响应时延模拟，与 `tests/mock_api.rs` 既有先例同款）。
//!
//! 入口覆盖矩阵（T-17「双入口各测」）：
//! 1. `access_token()` —— 时间性过期入口，「恰 1 次」断言；
//! 2. `Authenticator::refresh()` —— 服务端判失效入口（client.rs:244 现状调用点），
//!    断言 single-flight 串行（无并发在途刷新）+ 全体成功 + 收敛一致；
//! 3. `refresh_stale(&stale)` —— U4 新增显式失效锚点入口（client.rs:244 修复后改调），
//!    「恰 1 次」断言；
//! 4. 经 `ApiClient::with_token_retry`（token 失效信封 40140123 触发）的生产路径端到端，
//!    「恰 1 次」断言。
//!
//! 为何入口 2 断言「串行」而非「恰 1 次」：该入口的双检锚点（观察到的旧 token）由各
//! 线程在调用内部自行获取，晚到的观察者可能拿到他人已刷新的新 token 而触发一次
//! **串行**的多余刷新——这正是架构引入显式 stale 锚点（入口 3，关键决策 #7）要关闭的
//! 窗口。锁本身可保证、且测试可确定性断言的是「每时刻至多一个在途刷新请求」
//! （single-flight 核心）；「恰 1 次」的确定性断言由入口 3/4（全体同锚）承载。

use cloudmount_auth::open::{OpenApiAuth, TokenSet};
use cloudmount_auth::store::MemoryStore;
use cloudmount_auth::Authenticator as _;
use cloudmount_driver115::ApiClient;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};

/// canonical 数值口径：并发线程数（test-strategy T-17）。
const THREADS: usize = 10;
/// canonical 数值口径：外层循环轮数（test-strategy T-17，降 flaky）。
const ROUNDS: usize = 10;

// ============ mock 服务端（沿 tests/mock_api.rs 既有先例 + 在途窗口记录） ============

/// 一次被记录的请求。
#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    /// 含查询串的路径，如 `/open/refreshToken`。
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    /// 路径部分（不含查询串）。
    fn path_only(&self) -> &str {
        self.path.split('?').next().unwrap_or("")
    }
}

type Responder = dyn Fn(&Recorded) -> (u16, String) + Send + Sync;

/// mock 服务端句柄：请求记录 + 每请求的 responder 在途窗口。
struct MockServer {
    addr: SocketAddr,
    records: Arc<Mutex<Vec<Recorded>>>,
    windows: Arc<Mutex<Vec<(String, Instant, Instant)>>>,
}

impl MockServer {
    /// 刷新端点（`/open/refreshToken`）收到的请求数（离散计数断言用）。
    fn refresh_count(&self) -> usize {
        self.records
            .lock()
            .expect("lock")
            .iter()
            .filter(|r| r.path_only() == "/open/refreshToken")
            .count()
    }

    /// 刷新端点每个请求的 responder 在途窗口 [进入, 离开]（串行断言用）。
    fn refresh_windows(&self) -> Vec<(Instant, Instant)> {
        self.windows
            .lock()
            .expect("lock")
            .iter()
            .filter(|(p, _, _)| p == "/open/refreshToken")
            .map(|(_, s, e)| (*s, *e))
            .collect()
    }
}

/// 绑定随机端口并启动 mock 服务线程（每连接一线程，逐请求记录并响应）。
fn spawn_mock<F>(responder: F) -> MockServer
where
    F: Fn(&Recorded) -> (u16, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("local_addr");
    let records = Arc::new(Mutex::new(Vec::new()));
    let windows = Arc::new(Mutex::new(Vec::new()));
    let responder: Arc<Responder> = Arc::new(responder);

    let records_t = records.clone();
    let windows_t = windows.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let records = records_t.clone();
            let windows = windows_t.clone();
            let responder = responder.clone();
            std::thread::spawn(move || {
                let _ = handle_connection(&mut stream, &records, &windows, &*responder);
            });
        }
    });

    MockServer {
        addr,
        records,
        windows,
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    records: &Mutex<Vec<Recorded>>,
    windows: &Mutex<Vec<(String, Instant, Instant)>>,
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
        body: String::from_utf8_lossy(&body).to_string(),
    };
    records.lock().expect("lock").push(recorded.clone());

    let start = Instant::now();
    let (status, payload) = responder(&recorded);
    let end = Instant::now();
    windows
        .lock()
        .expect("lock")
        .push((recorded.path_only().to_string(), start, end));

    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

// ============ 测试夹具 ============

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64
}

/// 已越过 5 分钟提前刷新余量的过期 TokenSet（直接注入，不 sleep 真实时钟）。
fn expired_tokens(access: &str, refresh: &str) -> TokenSet {
    TokenSet {
        access_token: access.into(),
        refresh_token: refresh.into(),
        expires_at: now_secs(),
    }
}

/// 远期有效的 TokenSet（refresh 类入口本身是强制刷新，不依赖时间性过期）。
fn fresh_tokens(access: &str, refresh: &str) -> TokenSet {
    TokenSet {
        access_token: access.into(),
        refresh_token: refresh.into(),
        expires_at: now_secs() + 3600,
    }
}

/// 指向 mock 通行证域的已登录认证器。
fn auth_with(addr: SocketAddr, ts: TokenSet) -> Arc<OpenApiAuth> {
    Arc::new(
        OpenApiAuth::from_token_set("test-app", Arc::new(MemoryStore::default()), ts)
            .with_passport_base(format!("http://{addr}")),
    )
}

/// refreshToken 成功响应（认证域信封，state 为数字 1）。
fn refresh_ok_body(access_token: &str) -> String {
    format!(
        r#"{{"state":1,"code":0,"message":"","data":{{"access_token":"{access_token}","refresh_token":"rt-new-{access_token}","expires_in":3600}}}}"#
    )
}

// ============ T-17：并发单飞（正路径，红：现状并发双刷计数 >1） ============

/// T-17 入口一：`access_token()` 时间性过期入口。
///
/// 10 线程在注入的过期 TokenSet 上栅栏会合并发取 token → mock 刷新端点**恰 1 次**、
/// 全体共享同一次刷新结果（同一新 token）。现状（快路径丢锁后各自 `do_refresh`）
/// 计数 >1 —— 红。
#[test]
fn t17_access_token_entry_concurrent_expiry_refreshes_exactly_once() {
    for round in 0..ROUNDS {
        let new_token = format!("new-at-r{round}");
        let issued = new_token.clone();
        let mock = spawn_mock(move |req| match req.path_only() {
            "/open/refreshToken" => {
                assert_eq!(req.method, "POST", "刷新应为 POST 表单请求");
                assert!(
                    req.body.contains("refresh_token=stale-rt"),
                    "刷新请求应携带注入的 refresh_token；body={}",
                    req.body
                );
                (200, refresh_ok_body(&issued))
            }
            other => panic!("mock 收到未预期路径: {other}"),
        });
        let auth = auth_with(mock.addr, expired_tokens("stale-at", "stale-rt"));

        let barrier = Arc::new(Barrier::new(THREADS));
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let auth = auth.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    auth.access_token()
                })
            })
            .collect();

        let mut tokens = Vec::new();
        for h in handles {
            tokens.push(h.join().expect("线程应完成").expect("access_token 应成功"));
        }

        assert_eq!(
            mock.refresh_count(),
            1,
            "第 {round} 轮：10 线程在过期点会合应只触发 1 次刷新请求（并发双刷 = 40140117 形态）"
        );
        assert!(
            tokens.iter().all(|t| t == &tokens[0]),
            "第 {round} 轮：全体应共享同一次刷新结果，实际 {tokens:?}"
        );
        assert_eq!(
            tokens[0], new_token,
            "第 {round} 轮：应持有 mock 签发的新 token"
        );
    }
}

/// T-17 入口二（U4 新增）：`refresh_stale(&stale)` 显式失效锚点入口。
///
/// 与生产同构（client.rs `with_token_retry`：先观察到同一 token，再并发强刷）：
/// 10 线程持同一 stale 锚点会合 → 身份双检合并为**恰 1 次**刷新。
/// 现状（骨架暂走旧的无条件刷新逻辑）计数 >1 —— 红。
#[test]
fn t17_refresh_stale_entry_concurrent_force_refreshes_exactly_once() {
    for round in 0..ROUNDS {
        let new_token = format!("new-at-r{round}");
        let issued = new_token.clone();
        let mock = spawn_mock(move |req| match req.path_only() {
            "/open/refreshToken" => (200, refresh_ok_body(&issued)),
            other => panic!("mock 收到未预期路径: {other}"),
        });
        let auth = auth_with(mock.addr, fresh_tokens("stale-at", "stale-rt"));

        // 与生产同构：先观察到同一 token（此刻仍有效，不触发时间性刷新）
        let stale = auth.access_token().expect("初始 token 可用");
        assert_eq!(stale, "stale-at");

        let barrier = Arc::new(Barrier::new(THREADS));
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let auth = auth.clone();
                let barrier = barrier.clone();
                let stale = stale.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    auth.refresh_stale(&stale)
                })
            })
            .collect();
        for h in handles {
            h.join().expect("线程应完成").expect("refresh_stale 应成功");
        }

        assert_eq!(
            mock.refresh_count(),
            1,
            "第 {round} 轮：同一 stale 锚点下的并发强刷应被身份双检合并为恰 1 次刷新"
        );
        assert_eq!(
            auth.access_token().expect("刷新后 token 可用"),
            new_token,
            "第 {round} 轮：强刷后应持有 mock 签发的新 token"
        );
    }
}

/// T-17 入口二（现状生产调用点）：`Authenticator::refresh()`。
///
/// 断言 single-flight 核心不变量：任意两个刷新请求的在途窗口**不得重叠**
/// （锁跨网络调用 ⇒ 严格串行）。现状（丢锁后各自 `do_refresh`）必然出现并发在途
/// 窗口 —— 红。不断言「恰 1 次」的理由见文件头说明；responder 内 50ms 固定延迟
/// 为在途窗口模拟（mock_api.rs 既有先例同款），非测试对真实时钟的等待。
#[test]
fn t17_trait_refresh_entry_never_has_overlapping_in_flight_refreshes() {
    for round in 0..ROUNDS {
        let new_token = format!("new-at-r{round}");
        let issued = new_token.clone();
        let mock = spawn_mock(move |req| match req.path_only() {
            "/open/refreshToken" => {
                std::thread::sleep(Duration::from_millis(50));
                (200, refresh_ok_body(&issued))
            }
            other => panic!("mock 收到未预期路径: {other}"),
        });
        let auth = auth_with(mock.addr, fresh_tokens("stale-at", "stale-rt"));

        let barrier = Arc::new(Barrier::new(THREADS));
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let auth = auth.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    auth.refresh()
                })
            })
            .collect();
        for h in handles {
            h.join().expect("线程应完成").expect("refresh 应成功");
        }

        assert!(mock.refresh_count() >= 1, "第 {round} 轮：至少应有一次刷新");
        let wins = mock.refresh_windows();
        for i in 0..wins.len() {
            for j in (i + 1)..wins.len() {
                let (s1, e1) = wins[i];
                let (s2, e2) = wins[j];
                assert!(
                    s1 >= e2 || s2 >= e1,
                    "第 {round} 轮：检测到并发在途刷新（窗口 {i:?} 与 {j:?}）——刷新未持锁跨网络（single-flight 破坏）"
                );
            }
        }
        assert_eq!(
            auth.access_token().expect("刷新后 token 可用"),
            new_token,
            "第 {round} 轮：风暴后应收敛到 mock 签发的新 token"
        );
    }
}

/// T-17 入口二的生产路径端到端：`ApiClient::with_token_retry`（token 失效信封
/// 40140123 触发强刷重试）。10 线程并发命中失效信封 → 刷新**恰 1 次**、全体重试成功。
/// 现状（client.rs:244 调 `refresh()`，无身份双检）计数 >1 —— 红。
#[test]
fn t17_with_token_retry_invalid_token_envelope_refreshes_exactly_once() {
    for round in 0..ROUNDS {
        let new_token = format!("new-at-r{round}");
        let mock = spawn_mock(move |req| match req.path_only() {
            "/open/ufile/files" => {
                if req.header("authorization").as_deref() == Some("Bearer stale-at") {
                    // 服务端判定 access_token 失效（官方码族 40140123–40140126）
                    (
                        200,
                        r#"{"state":false,"code":40140123,"message":"access_token 无效","data":[]}"#
                            .to_string(),
                    )
                } else {
                    (
                        200,
                        r#"{"state":true,"code":0,"data":{"ok":true}}"#.to_string(),
                    )
                }
            }
            "/open/refreshToken" => (200, refresh_ok_body(&new_token)),
            other => panic!("mock 收到未预期路径: {other}"),
        });
        let auth = auth_with(mock.addr, fresh_tokens("stale-at", "stale-rt"));
        let client = Arc::new(
            ApiClient::with_base_url(auth, format!("http://{}", mock.addr))
                .with_rate_limit(1000.0, 50.0),
        );

        let barrier = Arc::new(Barrier::new(THREADS));
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let client = client.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    client.get_json::<serde_json::Value>("/open/ufile/files", &[])
                })
            })
            .collect();
        for h in handles {
            h.join().expect("线程应完成").expect("失效重试后请求应成功");
        }

        assert_eq!(
            mock.refresh_count(),
            1,
            "第 {round} 轮：并发 token 失效重试应合并为恰 1 次刷新（并发双刷 = 40140117 形态）"
        );
    }
}

// ============ T-18：刷新失败注入（负路径护栏：一致 Err、无死锁、锁释放） ============

/// 刷新端点失败注入模式。
#[derive(Debug, Clone, Copy)]
enum FailMode {
    /// HTTP 401（传输层非 2xx）。
    Http401,
    /// HTTP 500。
    Http500,
    /// HTTP 200 + 非 JSON 信封体。
    BadJson,
}

fn fail_response(mode: FailMode) -> (u16, String) {
    match mode {
        FailMode::Http401 => (401, r#"{"error":"invalid credentials"}"#.to_string()),
        FailMode::Http500 => (500, r#"{"error":"internal server error"}"#.to_string()),
        FailMode::BadJson => (200, "}}}not-a-json-envelope{{{".to_string()),
    }
}

/// T-18（access_token 入口）：刷新端点 401/500/坏 JSON → 全体等待者一致 Err、
/// 无死锁（join 完成即证）、锁已释放（翻转 mock 后单线程重试成功，且成功后
/// 再取 token 不再触发额外刷新）。
fn access_token_entry_failure_all_consistent_then_retry(mode: FailMode) {
    let flipped = Arc::new(Mutex::new(false));
    let mock = {
        let flipped = flipped.clone();
        spawn_mock(move |req| match req.path_only() {
            "/open/refreshToken" => {
                if *flipped.lock().expect("lock") {
                    (200, refresh_ok_body("new-at-ok"))
                } else {
                    fail_response(mode)
                }
            }
            other => panic!("mock 收到未预期路径: {other}"),
        })
    };
    let auth = auth_with(mock.addr, expired_tokens("stale-at", "stale-rt"));

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let auth = auth.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                auth.access_token()
            })
        })
        .collect();

    // 全体一致 Err
    let errs: Vec<String> = handles
        .into_iter()
        .map(|h| {
            h.join()
                .expect("线程应完成（无死锁、无 panic）")
                .expect_err("刷新失败期间所有等待者都应收到 Err")
                .to_string()
        })
        .collect();
    assert_eq!(errs.len(), THREADS);
    for (i, e) in errs.iter().enumerate() {
        assert_eq!(e, &errs[0], "线程 {i} 的错误应与其它等待者一致");
    }

    // 锁已释放：翻转 mock 后，单线程重试成功（串行重试为合法形态）
    *flipped.lock().expect("lock") = true;
    assert_eq!(
        auth.access_token().expect("失败后单线程重试应成功"),
        "new-at-ok"
    );
    // 新 token 已缓存：再取不再触发刷新
    let after_retry = mock.refresh_count();
    assert_eq!(auth.access_token().expect("token 可用"), "new-at-ok");
    assert_eq!(
        mock.refresh_count(),
        after_retry,
        "成功后再次取 token 不应触发刷新"
    );
}

#[test]
fn t18_access_token_entry_refresh_401_all_consistent_err_then_retry_ok() {
    access_token_entry_failure_all_consistent_then_retry(FailMode::Http401);
}

#[test]
fn t18_access_token_entry_refresh_500_all_consistent_err_then_retry_ok() {
    access_token_entry_failure_all_consistent_then_retry(FailMode::Http500);
}

#[test]
fn t18_access_token_entry_refresh_bad_json_all_consistent_err_then_retry_ok() {
    access_token_entry_failure_all_consistent_then_retry(FailMode::BadJson);
}

/// T-18（refresh_stale 入口，500 注入为例）：同口径——一致 Err、无死锁、锁释放，
/// 随后单线程重试成功。
#[test]
fn t18_refresh_stale_entry_refresh_500_all_consistent_err_then_retry_ok() {
    let flipped = Arc::new(Mutex::new(false));
    let mock = {
        let flipped = flipped.clone();
        spawn_mock(move |req| match req.path_only() {
            "/open/refreshToken" => {
                if *flipped.lock().expect("lock") {
                    (200, refresh_ok_body("new-at-ok"))
                } else {
                    fail_response(FailMode::Http500)
                }
            }
            other => panic!("mock 收到未预期路径: {other}"),
        })
    };
    let auth = auth_with(mock.addr, fresh_tokens("stale-at", "stale-rt"));
    let stale = auth.access_token().expect("初始 token 可用");

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let auth = auth.clone();
            let barrier = barrier.clone();
            let stale = stale.clone();
            std::thread::spawn(move || {
                barrier.wait();
                auth.refresh_stale(&stale)
            })
        })
        .collect();

    let errs: Vec<String> = handles
        .into_iter()
        .map(|h| {
            h.join()
                .expect("线程应完成（无死锁、无 panic）")
                .expect_err("刷新失败期间所有调用方都应收到 Err")
                .to_string()
        })
        .collect();
    for (i, e) in errs.iter().enumerate() {
        assert_eq!(e, &errs[0], "线程 {i} 的错误应与其它调用方一致");
    }

    // 锁已释放：翻转 mock 后，单线程重试成功
    *flipped.lock().expect("lock") = true;
    auth.refresh_stale(&stale).expect("失败后单线程重试应成功");
    assert_eq!(auth.access_token().expect("token 可用"), "new-at-ok");
    let after_retry = mock.refresh_count();
    assert_eq!(auth.access_token().expect("token 可用"), "new-at-ok");
    assert_eq!(
        mock.refresh_count(),
        after_retry,
        "成功后再次取 token 不应触发刷新"
    );
}
