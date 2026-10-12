//! 端到端集成测试：用本地 mock 服务端验证驱动对 115 的**请求构造**与**响应解析**。
//!
//! 这些测试不需要 115 开放平台凭据，却能覆盖最容易出错的链路：
//! - Bearer 头是否带上、查询参数是否正确（cid/limit/offset）；
//! - 分页字段在 `data` 之外的列表响应能否正确解析；
//! - `rt_space_info` 嵌套结构 → 配额；
//! - **秒传命中**是否真的零传输（不请求 upload/get_token）；
//! - **二次认证**（sign_check 闭区间）计算的 SHA1 是否正确回传；
//! - 下载链路：file_id → pick_code → 直链 → 带 Range 的流式读取。

use cloudmount_auth::open::{OpenApiAuth, TokenSet};
use cloudmount_auth::store::MemoryStore;
use cloudmount_core::{EntryKind, Provider, RangeSpec, RemoteId, UploadSource};
use cloudmount_driver115::{hash::sha1_hex_upper, ApiClient, Open115Provider};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// 一次被记录的请求。
#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    /// 含查询串的路径，如 `/open/ufile/files?cid=0`。
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

/// 先绑定一个随机端口（便于响应体里引用自身地址），再启动服务线程。
fn bind_ephemeral() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("local_addr");
    (listener, addr)
}

/// 在已绑定的 listener 上服务请求。
fn serve_on<F>(listener: TcpListener, responder: F) -> Arc<Mutex<Vec<Recorded>>>
where
    F: Fn(&Recorded) -> (u16, String) + Send + Sync + 'static,
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

/// 便捷封装：绑定 + 服务，一次完成（响应体不需要自身地址时使用）。
fn spawn_mock<F>(responder: F) -> (SocketAddr, Arc<Mutex<Vec<Recorded>>>)
where
    F: Fn(&Recorded) -> (u16, String) + Send + Sync + 'static,
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
        body: String::from_utf8_lossy(&body).to_string(),
    };
    records.lock().expect("lock").push(recorded.clone());

    let (status, payload) = responder(&recorded);
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

/// 构造一个已登录的驱动，指向 mock 服务端。
fn provider_for(addr: SocketAddr) -> Open115Provider {
    let store = Arc::new(MemoryStore::default());
    let auth = Arc::new(OpenApiAuth::from_token_set(
        "test-app",
        store,
        TokenSet {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            // 远期过期，避免触发真实刷新请求
            expires_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + 3600,
        },
    ));
    // 提速限速器，避免测试等待
    let client =
        ApiClient::with_base_url(auth, format!("http://{addr}")).with_rate_limit(1000.0, 50.0);
    Open115Provider::with_client(client)
}

#[test]
fn list_sends_auth_and_paging_and_parses_entries() {
    let (addr, records) = spawn_mock(|_req| {
        (
            200,
            r#"{"state":true,"code":0,"message":"","data":[
                {"fid":"11","pid":"0","fc":"0","fn":"文档","fs":0,"sha1":"","pc":""},
                {"fid":"12","pid":"0","fc":"1","fn":"a.pdf","fs":999,"sha1":"ABC","pc":"pick1"}
            ],"count":2,"offset":0,"limit":1000,"cid":0}"#
                .to_string(),
        )
    });

    let provider = provider_for(addr);
    let entries = provider.list(&RemoteId("0".into())).expect("list");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].kind, EntryKind::Directory);
    assert_eq!(entries[0].name, "文档");
    assert_eq!(entries[1].kind, EntryKind::File);
    assert_eq!(entries[1].size, 999);
    assert_eq!(entries[1].pick_code.as_deref(), Some("pick1"));

    // 请求侧断言：Bearer 头与查询参数
    let reqs = records.lock().unwrap();
    assert_eq!(reqs.len(), 1, "count=2 一页即够，不应再请求");
    let req = &reqs[0];
    assert_eq!(req.method, "GET");
    assert_eq!(req.path_only(), "/open/ufile/files");
    assert_eq!(
        req.header("authorization").as_deref(),
        Some("Bearer test-token")
    );
    assert!(req.path.contains("cid=0"), "应带 cid: {}", req.path);
    assert!(req.path.contains("limit=1000"), "应带 limit: {}", req.path);
    assert!(req.path.contains("offset=0"), "应带 offset: {}", req.path);
}

#[test]
fn quota_parses_nested_space_info() {
    let (addr, _records) = spawn_mock(|_| {
        (
            200,
            r#"{"state":true,"code":0,"data":{
                "user_name":"tester",
                "rt_space_info":{
                  "all_total":{"size":1000,"size_format":"1KB"},
                  "all_use":{"size":"400"},
                  "all_remain":{"size":600}
                }}}"#
                .to_string(),
        )
    });

    let provider = provider_for(addr);
    let q = provider.quota().expect("quota");
    assert_eq!(q.total, 1000);
    assert_eq!(q.used, 400, "字符串数字也要解析");
    assert_eq!(q.remaining, 600);
}

#[test]
fn rapid_upload_hits_zero_transfer_and_does_not_request_oss_token() {
    let (addr, records) = spawn_mock(|req| {
        assert_eq!(req.path_only(), "/open/upload/init");
        (
            200,
            r#"{"state":true,"code":0,"data":{"status":2,"file_id":"777","pick_code":"pc777"}}"#
                .to_string(),
        )
    });

    let provider = provider_for(addr);
    let data = b"hello cloudmount";
    let result = provider
        .upload(
            &RemoteId("0".into()),
            "hello.txt",
            UploadSource::Bytes(data),
            data.len() as u64,
        )
        .expect("upload");

    assert!(result.rapid_upload, "status=2 应为秒传命中");
    assert_eq!(result.id.0, "777");

    let reqs = records.lock().unwrap();
    assert_eq!(reqs.len(), 1, "秒传命中不应再请求 get_token / OSS");
    // 表单编码：应带 fileid（大写 SHA1）与 target
    let expected_sha1 = sha1_hex_upper(data);
    assert!(
        reqs[0].body.contains(&format!("fileid={expected_sha1}")),
        "init 表单应含大写 SHA1 的 fileid；body={}",
        reqs[0].body
    );
    assert!(
        reqs[0].body.contains("target=U_1_0"),
        "根目录 target 应为 U_1_0；body={}",
        reqs[0].body
    );
}

#[test]
fn second_auth_sign_check_returns_correct_sha1_then_proceeds_to_oss_token() {
    // 文件 1000 字节，全部为 0x41('A')，便于计算区间 SHA1
    let data = vec![0x41u8; 1000];

    let (addr, records) = spawn_mock(move |req| {
        match req.path_only() {
            "/open/upload/init" => {
                if req.body.contains("sign_key=") {
                    // 第二次 init：已带二次认证签名 → 进入真实上传流程
                    (
                        200,
                        r#"{"state":true,"code":0,"data":{
                            "status":1,"target":"U_1_0","bucket":"bkt",
                            "object":"obj/path","file_id":"",
                            "callback":{"callback":"https://cb.example/x","callback_var":"{\"k\":1}"}}}"#
                            .to_string(),
                    )
                } else {
                    // 第一次 init：要求二次认证，区间 [10, 19]
                    (
                        200,
                        r#"{"state":true,"code":0,"data":{
                            "status":7,"sign_key":"KEY123","sign_check":"10-19"}}"#
                            .to_string(),
                    )
                }
            }
            "/open/upload/get_token" => (
                200,
                r#"{"state":true,"code":0,"data":{
                    "endpoint":"127.0.0.1:1","AccessKeyId":"AK",
                    "AccessKeySecret":"SK","SecurityToken":"ST"}}"#
                    .to_string(),
            ),
            other => panic!("mock 收到未预期路径: {other}"),
        }
    });

    let provider = provider_for(addr);
    // OSS 直传会指向 127.0.0.1:1（必然失败），这里只验证到"取到凭证"为止的行为
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "big.bin",
            UploadSource::Bytes(&data),
            data.len() as u64,
        )
        .expect_err("OSS 端点为不可达地址，应失败");

    let reqs = records.lock().unwrap();
    assert_eq!(
        reqs.len(),
        3,
        "应为 init → init(带签名) → get_token 三次请求"
    );
    assert_eq!(reqs[0].path_only(), "/open/upload/init");
    assert_eq!(reqs[1].path_only(), "/open/upload/init");
    assert_eq!(reqs[2].path_only(), "/open/upload/get_token");

    // 第一次不应带签名；第二次必须带正确的 sign_key 与区间 SHA1
    assert!(!reqs[0].body.contains("sign_key="), "首次 init 不该带签名");
    assert!(reqs[1].body.contains("sign_key=KEY123"), "应回传 sign_key");
    let expected = sha1_hex_upper(&data[10..=19]);
    assert!(
        reqs[1].body.contains(&format!("sign_val={expected}")),
        "sign_val 应为区间 [10,19] 的大写 SHA1；body={}",
        reqs[1].body
    );

    // 错误应是 OSS 传输失败（证明已走到直传这一步）
    let msg = format!("{err}");
    assert!(
        msg.contains("127.0.0.1:1") || msg.to_lowercase().contains("transport"),
        "应因 OSS 不可达而失败，实际: {msg}"
    );
}

#[test]
fn download_resolves_pick_code_then_streams_range_from_direct_url() {
    let direct_body = b"0123456789ABCDEF".to_vec();
    let direct_body_for_resp = direct_body.clone();

    let (listener, addr) = bind_ephemeral();
    let records = serve_on(listener, move |req| match req.path_only() {
        "/open/folder/get_info" => (
            200,
            r#"{"state":true,"code":0,"data":{
                "file_id":"42","file_name":"f.bin","size_byte":16,
                "sha1":"X","pick_code":"PC42","folder_count":null}}"#
                .to_string(),
        ),
        "/open/ufile/downurl" => {
            // 直链指向 mock 自己（http 明文），从而可端到端验证 Range 流式读取
            let payload = format!(
                r#"{{"state":true,"code":0,"data":{{"42":{{"file_name":"f.bin","file_size":16,
                    "pick_code":"PC42","url":{{"url":"http://{addr}/direct/42","client":"1"}}}}}}}}"#
            );
            (200, payload)
        }
        "/direct/42" => (
            206,
            String::from_utf8_lossy(&direct_body_for_resp).to_string(),
        ),
        other => panic!("mock 收到未预期路径: {other}"),
    });

    let provider = provider_for(addr);
    let mut reader = provider
        .open_range(
            &RemoteId("42".into()),
            RangeSpec {
                start: 0,
                end: Some(15),
            },
        )
        .expect("open_range");
    let mut got = Vec::new();
    reader.read_to_end(&mut got).expect("read stream");

    assert_eq!(got, direct_body, "直链内容应完整读出");

    let reqs = records.lock().unwrap();
    assert_eq!(
        reqs[0].path_only(),
        "/open/folder/get_info",
        "应先查详情拿 pick_code"
    );
    assert!(reqs[0].path.contains("file_id=42"));
    assert_eq!(reqs[1].path_only(), "/open/ufile/downurl");
    assert!(
        reqs[1].body.contains("pick_code=PC42"),
        "downurl 应带 pick_code；body={}",
        reqs[1].body
    );
    assert_eq!(reqs[2].path_only(), "/direct/42");
    assert_eq!(
        reqs[2].header("range").as_deref(),
        Some("bytes=0-15"),
        "直链请求应带 Range 头"
    );
    assert!(
        reqs[2].header("authorization").is_none(),
        "直链为 CDN 地址，不应带 Authorization"
    );
}
