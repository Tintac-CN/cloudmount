//! Cookie 通道日志凭据脱敏回归（F-04 / T-07：AC4 安全不变量 #2）。
//!
//! 本地 mock（复用 `cookie_mock` 的 pub 项）+ **手写 tracing Subscriber**
//! （仅 `tracing` 本体——driver115 零 dev-deps，m2 裁决：不用
//! MakeWriter/tracing-subscriber），`with_default` 挂载、trace 级全开捕获
//! uploadinfo / initupload / gettoken / get_info 全流程（含非 2xx 失败路径）。
//!
//! 断言口径（逐值缺席）：捕获日志中不得出现金丝雀敏感值
//! （userkey / STS 三元组 / Cookie UID/CID/SEID / pickcode 明文）；
//! 字段名与长度（`名:长度`）允许出现。测试侧零外部网络、零真实凭据。

mod cookie_mock;

use cloudmount_auth::cookie::{CookieAuth, CookieJar, CookiePolicy};
use cloudmount_auth::store::{MemoryStore, SecretKind, SecretStore};
use cloudmount_core::{Error, Provider, RemoteId, UploadSource};
use cloudmount_driver115::ec115::test_peer::TestEcdhPeer;
use cloudmount_driver115::{hash::sha1_hex_upper, Cookie115Provider};
use cookie_mock::{spawn_mock, MockResponse, Recorded};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

// ---------- 金丝雀敏感值（桩值，逐值缺席断言的靶子） ----------

/// uploadinfo 下发的 userkey（金丝雀）。
const USERKEY: &str = "USERKEY-CANARY-123";
/// STS 三元组（金丝雀）。
const AKID: &str = "AKID-CANARY-123";
const AKSECRET: &str = "AKSECRET-CANARY-123";
const SECTOKEN: &str = "SECTOKEN-CANARY-123";
/// get_info 元数据中的 pickcode（金丝雀；非凭据但同类原样打印，一并收口）。
const PICKCODE: &str = "PC-CANARY-123";
/// 注入 Cookie 的三字段（金丝雀；HTTP 头不属日志面，纵深防御一并断言）。
const UID_CANARY: &str = "UID-CANARY-123";
const CID_CANARY: &str = "CID-CANARY-123";
const SEID_CANARY: &str = "SEID-CANARY-123";

const ALL_CANARIES: [&str; 8] = [
    USERKEY,
    AKID,
    AKSECRET,
    SECTOKEN,
    PICKCODE,
    UID_CANARY,
    CID_CANARY,
    SEID_CANARY,
];

// ---------- 桩响应 ----------

/// uploadinfo 成功（扁平结构；userkey 为金丝雀）。
const OK_UPLOADINFO: &str = r#"{"state":true,"errno":0,"user_id":123,"userkey":"USERKEY-CANARY-123","size_limit":1153433600}"#;

/// uploadinfo 业务失败 + **HTTP 500 非 2xx**（体仍回显 userkey 金丝雀——
/// 失败路径的原始响应同样不得落日志）。
const FAIL_UPLOADINFO: &str =
    r#"{"state":false,"errno":990001,"errcode":"990001","userkey":"USERKEY-CANARY-123"}"#;

/// gettoken 成功（OSS STS；三元组全金丝雀）。
const OK_GETTOKEN: &str = r#"{"AccessKeyId":"AKID-CANARY-123","AccessKeySecret":"AKSECRET-CANARY-123","SecurityToken":"SECTOKEN-CANARY-123","StatusCode":"200"}"#;

/// gettoken 业务失败（StatusCode!=200；体仍含 STS 金丝雀）。
const FAIL_GETTOKEN: &str = r#"{"AccessKeyId":"AKID-CANARY-123","AccessKeySecret":"AKSECRET-CANARY-123","SecurityToken":"SECTOKEN-CANARY-123","StatusCode":"403"}"#;

/// initupload 状态 1（未命中，走 OSS 直传）的解密后明文。
/// 注：该响应不含凭据（bucket/object/callback），其解密日志不在本 unit 施工面。
const INIT_MISS: &str = r#"{"status":1,"statuscode":0,"bucket":"bkt","object":"obj/f.bin","callback":{"callback":"https://cb.example/n","callback_var":"{\"x\":1}"}}"#;

/// get_info 成功（文件元数据；pickcode 为金丝雀）。
const OK_GET_INFO: &str = r#"{"state":true,"code":0,"message":"","data":[{"fid":"321","cid":"0","pid":"0","n":"f.bin","s":"3","pc":"PC-CANARY-123","sha":"ABCDEF","t":"2026-10-13 15:04","tp":"0"}]}"#;

// ---------- 手写 capturing Subscriber（tracing 本体，约 40 行） ----------

/// 捕获全部事件文本的 Subscriber：`enabled` 全级别放行（trace 全开，
/// T-07 canonical 口径），`on_event`（[`tracing::Subscriber::event`]）经
/// [`tracing::field::Visit`] 把字段渲染进行文本。
struct CapturingSubscriber {
    events: Arc<Mutex<Vec<String>>>,
}

impl tracing::Subscriber for CapturingSubscriber {
    fn enabled(&self, _meta: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut line = format!(
            "[{}] {}",
            event.metadata().level(),
            event.metadata().target()
        );
        event.record(&mut FieldVisitor { out: &mut line });
        self.events.lock().expect("capture lock").push(line);
    }

    fn enter(&self, _id: &tracing::span::Id) {}

    fn exit(&self, _id: &tracing::span::Id) {}
}

/// 字段访问器：` name=value` 追加到事件行尾。
struct FieldVisitor<'a> {
    out: &'a mut String,
}

impl tracing::field::Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.out.push_str(&format!(" {}={}", field.name(), value));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.out.push_str(&format!(" {}={:?}", field.name(), value));
    }
}

/// 在捕获 Subscriber 下运行 `f`，返回（结果, 事件文本列表）。
fn capture_events<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CapturingSubscriber {
        events: events.clone(),
    };
    let out = tracing::subscriber::with_default(subscriber, f);
    let captured = std::mem::take(&mut *events.lock().expect("capture lock"));
    (out, captured)
}

// ---------- 断言助手 ----------

/// 逐值缺席：任一敏感金丝雀出现在任一事件中即失败（附泄漏事件全文）。
fn assert_no_secrets(events: &[String], secrets: &[&str]) {
    for secret in secrets {
        for line in events {
            assert!(
                !line.contains(secret),
                "安全不变量 #2 违例：敏感值 `{secret}` 落入日志——事件: {line}"
            );
        }
    }
}

/// 捕获完备性守卫：指定 target 必须有事件（防 subscriber 失效/日志整体缺失的假绿）。
fn assert_target_logged(events: &[String], target: &str) {
    assert!(
        !events.is_empty(),
        "应捕获到日志事件（subscriber 失效会假绿）"
    );
    assert!(
        events.iter().any(|l| l.contains(target)),
        "应有 {target} 日志事件；实际捕获: {events:?}"
    );
}

// ---------- 被测构造 ----------

/// 已登录（policy 双真 + 独立容器预置**金丝雀 Cookie**）的认证器。
fn logged_in_auth() -> CookieAuth {
    let store = Arc::new(MemoryStore::default());
    let jar = CookieJar {
        uid: UID_CANARY.into(),
        cid: CID_CANARY.into(),
        seid: SEID_CANARY.into(),
        kid: None,
    };
    store
        .put(SecretKind::CookieJar, &serde_json::to_vec(&jar).unwrap())
        .expect("预置 CookieJar");
    CookieAuth::new(
        CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        },
        store,
    )
}

/// 指向 mock 的 Cookie 驱动（四类基址全指向本地 mock；快速限速绕开 0.5 r/s）。
fn cookie_provider(addr: SocketAddr, peer_pub: Option<[u8; 56]>) -> Cookie115Provider {
    let base = format!("http://{addr}");
    let provider = Cookie115Provider::try_new(
        CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        },
        logged_in_auth(),
    )
    .expect("Cookie 通道构造")
    .with_base_urls(base.clone(), base.clone())
    .with_upload_base(base.clone())
    .with_oss_base(format!("{base}/{{bucket}}/{{object}}"))
    .with_rate_limit(1000.0, 50.0);
    match peer_pub {
        Some(key) => provider.with_ecdh_remote_pub(key),
        None => provider,
    }
}

/// initupload 的 ECDH 加密响应（对端镜像加密；同 cookie_channel 既有模式）。
fn encrypted_init_response(peer: &TestEcdhPeer, req: &Recorded, plain: &str) -> MockResponse {
    let k_ec = req.query("k_ec").expect("initupload 应带 k_ec 查询参数");
    let session = peer.session(&k_ec).expect("对端会话");
    let body = session
        .encrypt_response(plain.as_bytes())
        .expect("对端加密响应");
    MockResponse::bytes(200, body)
}

/// webapi 列表成功响应（验证命中用；count=1 单页终止）。
fn list_ok(fid: &str, name: &str, sha1: &str, size: u64) -> String {
    format!(
        r#"{{"state":true,"code":0,"message":"","count":1,"data":[{{"fid":"{fid}","cid":"0","pid":"0","n":"{name}","s":"{size}","pc":"PC-LIST","sha":"{sha1}","t":"2026-10-13 15:04","tp":"0"}}]}}"#
    )
}

// ============ T-07：全流程金丝雀逐值缺席 ============

/// 未命中直传全流程（uploadinfo → initupload → gettoken → OSS PUT → 验证列表，
/// 成功路径）：四处施工面全部经过，捕获日志逐值缺席全部金丝雀。
#[test]
fn upload_miss_flow_logs_never_contain_secret_values() {
    let data = b"canary-flow-payload";
    let sha1 = sha1_hex_upper(data);
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, _records) = spawn_mock(move |req| {
        match (req.method.as_str(), req.path_only()) {
            ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
            ("POST", "/4.0/initupload.php") => encrypted_init_response(&peer, req, INIT_MISS),
            ("GET", "/3.0/gettoken.php") => MockResponse::json(200, OK_GETTOKEN),
            ("PUT", "/bkt/obj/f.bin") => MockResponse::bytes(200, Vec::new()),
            ("GET", "/files") => {
                MockResponse::json(200, list_ok("999", "f.bin", &sha1, data.len() as u64))
            }
            other => panic!("mock 收到未预期请求: {other:?}"),
        }
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let (result, events) = capture_events(|| {
        provider.upload(
            &RemoteId("0".into()),
            "f.bin",
            UploadSource::Bytes(data),
            data.len() as u64,
        )
    });
    result.expect("直传成功路径应成功");

    // 捕获完备性守卫：三处上传侧施工面都必须有事件
    for target in [
        "cm_cookie_uploadinfo",
        "cm_initupload",
        "cm_oss_token",
    ] {
        assert_target_logged(&events, target);
    }
    // 核心断言：逐值缺席（红 = 现状原始 JSON/表单落日志含金丝雀明文）
    assert_no_secrets(&events, &ALL_CANARIES);
    // 脱敏仍保留诊断线索：字段名 + 长度形态（`userkey:` 前缀）可出现
    assert!(
        events
            .iter()
            .any(|l| l.contains("cm_cookie_uploadinfo") && l.contains("userkey")),
        "uploadinfo 脱敏日志应保留字段名（名:长度形态）: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|l| l.contains("cm_initupload") && l.contains("userkey")),
        "initupload 脱敏日志应保留字段名（名:长度形态）: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|l| l.contains("cm_oss_token") && l.contains("AccessKeySecret")),
        "gettoken 脱敏日志应保留字段名（名:长度形态）: {events:?}"
    );
}

/// uploadinfo 失败路径（**HTTP 500 非 2xx** + 体含 userkey 金丝雀）：
/// 失败响应的原始体同样不得落日志。
#[test]
fn uploadinfo_non2xx_failure_logs_never_contain_secret_values() {
    let (addr, records) = spawn_mock(|req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => MockResponse::json(500, FAIL_UPLOADINFO),
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = cookie_provider(addr, None);
    let (result, events) = capture_events(|| {
        provider.upload(
            &RemoteId("0".into()),
            "a.txt",
            UploadSource::Bytes(b"abc"),
            3,
        )
    });
    assert!(
        matches!(result, Err(Error::Remote { .. })),
        "state=false 必须 → Error::Remote，实际: {:?}",
        result
    );
    assert_eq!(
        records.lock().unwrap().len(),
        1,
        "uploadinfo 失败后不应再发任何请求"
    );
    assert_target_logged(&events, "cm_cookie_uploadinfo");
    assert_no_secrets(&events, &ALL_CANARIES);
}

/// gettoken 失败路径（StatusCode!=200，体含 STS 三元组金丝雀）：
/// 业务失败响应同样不得落日志，且错误上抛前不得继续 OSS PUT。
#[test]
fn gettoken_failure_logs_never_contain_secret_values() {
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, records) = spawn_mock(move |req| {
        match (req.method.as_str(), req.path_only()) {
            ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
            ("POST", "/4.0/initupload.php") => encrypted_init_response(&peer, req, INIT_MISS),
            ("GET", "/3.0/gettoken.php") => MockResponse::json(200, FAIL_GETTOKEN),
            other => panic!("mock 收到未预期请求: {other:?}"),
        }
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let (result, events) = capture_events(|| {
        provider.upload(
            &RemoteId("0".into()),
            "f.bin",
            UploadSource::Bytes(b"payload"),
            7,
        )
    });
    assert!(
        matches!(result, Err(Error::Auth(_))),
        "StatusCode!=200 必须 → Error::Auth，实际: {:?}",
        result
    );
    assert!(
        !records
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.method == "PUT"),
        "token 获取失败后不得继续 OSS PUT"
    );
    assert_target_logged(&events, "cm_oss_token");
    assert_no_secrets(&events, &ALL_CANARIES);
}

/// stat → get_info：文件元数据原样 JSON 落日志一并收口（pickcode 金丝雀缺席），
/// 且脱敏后仍保留字段名（pc 名:长度形态）。
#[test]
fn stat_logs_never_contain_secret_values() {
    let (addr, _records) = spawn_mock(|req| match (req.method.as_str(), req.path_only()) {
        ("GET", "/files/get_info") => MockResponse::json(200, OK_GET_INFO),
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = cookie_provider(addr, None);
    let (entry, events) = capture_events(|| provider.stat(&RemoteId("321".into())));
    let entry = entry.expect("stat 应成功");
    assert_eq!(entry.pick_code.as_deref(), Some(PICKCODE), "桩 pickcode 应解析");

    assert_target_logged(&events, "cm_cookie_stat");
    assert_no_secrets(&events, &ALL_CANARIES);
    // 脱敏保留**顶层** JSON 字段名（名:长度形态；pickcode 嵌套于 data，
    // 按架构只记 data 的长度——金丝雀缺席由 assert_no_secrets 守护）
    assert!(
        events
            .iter()
            .any(|l| l.contains("cm_cookie_stat") && l.contains("data:")),
        "get_info 脱敏日志应保留顶层字段名（名:长度形态）: {events:?}"
    );
}
