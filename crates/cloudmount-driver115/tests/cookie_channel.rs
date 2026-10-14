//! Cookie 通道 HTTP 级集成测试（F-14：T-23 错误矩阵 + T-24 成功回环 + 渲染回归）。
//!
//! 全部请求落在本地明文 mock 服务端（`cookie_mock` 模块，扩展 mock_api.rs 模式），
//! 经 `with_base_urls`/`with_upload_base`/`with_oss_base` 完整基址（含 scheme）注入，
//! 零外部网络、零真实凭据。
//!
//! TDD 红阶段（U1）状态说明：
//! - `ec115::test_peer` 与 `Cookie115Provider::with_ecdh_remote_pub` 的消费端
//!   （`ecdh_cipher`）为 `todo!()` 骨架——依赖 ECDH 回环的用例（gettoken /
//!   oss_put / 秒传 / 直传）以 todo panic 呈红；
//! - oss_put 500/403 用例锚定 F-11 修复后契约（`Error::Http{status}`）；
//! - uploadinfo / 传输错误 / 不可解密 / 渲染回归为已可达路径的基线守护。

mod cookie_mock;

use cloudmount_auth::cookie::{CookieAuth, CookieJar, CookiePolicy};
use cloudmount_auth::store::{MemoryStore, SecretKind, SecretStore};
use cloudmount_core::{Error, Provider, RemoteId, UploadSource};
use cloudmount_driver115::ec115::test_peer::TestEcdhPeer;
use cloudmount_driver115::{hash::sha1_hex_upper, Cookie115Provider};
use cookie_mock::{spawn_mock, MockResponse, Recorded};
use std::net::SocketAddr;
use std::sync::Arc;

/// uploadinfo 成功响应（扁平结构；userkey 为金丝雀值——凭据不得落日志，T-07 另行断言）。
const OK_UPLOADINFO: &str =
    r#"{"state":true,"errno":0,"user_id":123,"userkey":"USERKEY-CANARY","size_limit":1153433600}"#;

/// gettoken 成功响应（OSS STS；值均为金丝雀）。
const OK_GETTOKEN: &str = r#"{"AccessKeyId":"AKID-CANARY","AccessKeySecret":"AKSECRET-CANARY","SecurityToken":"SECTOKEN-CANARY","StatusCode":"200"}"#;

/// initupload 状态 1（未命中，走 OSS 直传）的解密后明文。
const INIT_MISS: &str = r#"{"status":1,"statuscode":0,"bucket":"bkt","object":"obj/f.bin","callback":{"callback":"https://cb.example/n","callback_var":"{\"x\":1}"}}"#;

/// 已登录（policy 双真 + 独立容器预置 CookieJar）的认证器。
fn logged_in_auth() -> CookieAuth {
    let store = Arc::new(MemoryStore::default());
    let jar = CookieJar {
        uid: "u1".into(),
        cid: "c1".into(),
        seid: "s1".into(),
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

/// 构造指向 mock 的 Cookie 驱动：四类基址（webapi/proapi/upload/oss）全部指向
/// 同一本地 mock（OSS 以路径段承载 bucket），快速限速绕开 0.5 r/s。
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

fn provider_for(addr: SocketAddr) -> Cookie115Provider {
    cookie_provider(addr, None)
}

/// initupload 的 ECDH 加密成功响应（对端镜像加密）。
fn encrypted_init_response(peer: &TestEcdhPeer, req: &Recorded, plain: &str) -> MockResponse {
    let k_ec = req.query("k_ec").expect("initupload 应带 k_ec 查询参数");
    let session = peer.session(&k_ec).expect("对端会话");
    let body = session
        .encrypt_response(plain.as_bytes())
        .expect("对端加密响应");
    MockResponse::bytes(200, body)
}

/// webapi 列表成功响应（单条文件；sha/名称供 verify_upload 命中；count=1 单页终止）。
fn list_ok(fid: &str, name: &str, sha1: &str, size: u64) -> String {
    format!(
        r#"{{"state":true,"code":0,"message":"","count":1,"data":[{{"fid":"{fid}","cid":"0","pid":"0","n":"{name}","s":"{size}","pc":"PC-LIST","sha":"{sha1}","t":"2026-10-13 15:04","tp":"0"}}]}}"#
    )
}

// ============ T-23 错误矩阵 ============

/// uploadinfo `state=false` → `Error::Remote`（业务信封码 → Remote，
/// 架构 F-11 分类边界；HTTP 层 200 不掩盖业务失败）。
#[test]
fn uploadinfo_state_false_is_remote_error() {
    let (addr, records) = spawn_mock(|req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => {
            MockResponse::json(200, r#"{"state":false,"errno":990001,"errcode":"990001"}"#)
        }
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = provider_for(addr);
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "a.txt",
            UploadSource::Bytes(b"abc"),
            3,
        )
        .expect_err("uploadinfo state=false 必须失败");
    match err {
        Error::Remote { code, message } => {
            assert_eq!(code, 990001);
            assert!(
                message.contains("uploadinfo"),
                "错误应指明 uploadinfo: {message}"
            );
        }
        other => panic!("应为 Remote 业务错误，实际: {other}"),
    }
    assert_eq!(
        records.lock().unwrap().len(),
        1,
        "uploadinfo 失败后不应再发任何请求"
    );
}

/// initupload 传输错误（服务端不发响应直接断连）→ `Error::Transport`。
/// 同时证明 uploadinfo 成功路径解析正确、流程推进到 initupload。
#[test]
fn initupload_transport_error_is_reported_verbatim() {
    let (addr, records) = spawn_mock(|req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
        ("POST", "/4.0/initupload.php") => MockResponse::abandon(),
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = provider_for(addr);
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "a.bin",
            UploadSource::Bytes(b"xyz"),
            3,
        )
        .expect_err("initupload 传输错误必须上抛");
    assert!(
        matches!(err, Error::Transport(_)),
        "应为 Transport 传输错误，实际: {err}"
    );
    let reqs = records.lock().unwrap();
    assert_eq!(reqs.len(), 2, "uploadinfo + initupload");
    assert_eq!(reqs[1].path_only(), "/4.0/initupload.php");
}

/// initupload 响应不可解密（非 ECDH 密文）→ Err 且错误链含 ec115 标识。
#[test]
fn initupload_undecryptable_response_is_err() {
    let (addr, _records) = spawn_mock(|req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
        ("POST", "/4.0/initupload.php") => {
            // 非法密文：定长整块垃圾字节
            MockResponse::bytes(200, vec![0xFF; 32])
        }
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = provider_for(addr);
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "a.bin",
            UploadSource::Bytes(b"xyz"),
            3,
        )
        .expect_err("不可解密响应必须失败");
    let msg = format!("{err}");
    assert!(msg.contains("ec115"), "错误链应含 ec115 标识: {msg}");
}

/// gettoken `StatusCode != 200` → `Error::Auth`，且不得继续 OSS PUT。
/// （需经 ECDH 回环到达 gettoken——红阶段以 todo panic 呈红。）
#[test]
fn gettoken_non_200_status_is_auth_error() {
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, records) = spawn_mock(move |req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
        ("POST", "/4.0/initupload.php") => encrypted_init_response(&peer, req, INIT_MISS),
        ("GET", "/3.0/gettoken.php") => MockResponse::json(
            200,
            r#"{"AccessKeyId":"A","AccessKeySecret":"S","SecurityToken":"T","StatusCode":"403"}"#,
        ),
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "f.bin",
            UploadSource::Bytes(b"payload"),
            7,
        )
        .expect_err("gettoken StatusCode!=200 必须失败");
    match err {
        Error::Auth(msg) => assert!(
            msg.contains("OSS token"),
            "错误应指明 OSS token 获取失败: {msg}"
        ),
        other => panic!("应为 Auth 错误，实际: {other}"),
    }
    let reqs = records.lock().unwrap();
    assert!(
        !reqs.iter().any(|r| r.method == "PUT"),
        "token 获取失败后不得继续 OSS PUT"
    );
}

/// OSS PUT 500 → `Error::Http{500}` 含状态码与响应体摘要。
/// （F-11：现状吞掉非 2xx 假成功——红测锚定修复后契约。）
#[test]
fn oss_put_500_is_reported_as_http_error() {
    let data = b"oss-500-payload";
    let sha1 = sha1_hex_upper(data);
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, _records) = spawn_mock(move |req| {
        match (req.method.as_str(), req.path_only()) {
            ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
            ("POST", "/4.0/initupload.php") => encrypted_init_response(&peer, req, INIT_MISS),
            ("GET", "/3.0/gettoken.php") => MockResponse::json(200, OK_GETTOKEN),
            ("PUT", "/bkt/obj/f.bin") => MockResponse::bytes(
                500,
                br#"<?xml version="1.0"?><Error><Code>InternalError</Code></Error>"#.to_vec(),
            ),
            // 现状吞掉 500 后会走到验证列表——列表也返回命中，
            // 让"假成功"在 expect_err 处显式暴露
            ("GET", "/files") => {
                MockResponse::json(200, list_ok("500", "f.bin", &sha1, data.len() as u64))
            }
            other => panic!("mock 收到未预期请求: {other:?}"),
        }
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "f.bin",
            UploadSource::Bytes(data),
            data.len() as u64,
        )
        .expect_err("OSS 500 不得被吞成上传成功");
    match err {
        Error::Http { status, body } => {
            assert_eq!(status, 500);
            assert!(
                body.contains("500") || body.contains("InternalError"),
                "错误应含状态码或响应体摘要: {body}"
            );
        }
        other => panic!("应为 Error::Http{{500}}（F-11），实际: {other}"),
    }
}

/// OSS PUT 403 → `Error::Http{403}` 且不可重试（4xx 永久错误快速失败，
/// 不进入轮内重试——m9 硬要求）。
#[test]
fn oss_put_403_fails_fast_without_retry() {
    let data = b"oss-403-payload";
    let sha1 = sha1_hex_upper(data);
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, records) = spawn_mock(move |req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
        ("POST", "/4.0/initupload.php") => encrypted_init_response(&peer, req, INIT_MISS),
        ("GET", "/3.0/gettoken.php") => MockResponse::json(200, OK_GETTOKEN),
        ("PUT", "/bkt/obj/f.bin") => MockResponse::bytes(
            403,
            br#"<?xml version="1.0"?><Error><Code>AccessDenied</Code></Error>"#.to_vec(),
        ),
        ("GET", "/files") => {
            MockResponse::json(200, list_ok("403", "f.bin", &sha1, data.len() as u64))
        }
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let err = provider
        .upload(
            &RemoteId("0".into()),
            "f.bin",
            UploadSource::Bytes(data),
            data.len() as u64,
        )
        .expect_err("OSS 403 不得被吞成上传成功");
    assert!(!err.is_retryable(), "403 永久错误不得进入重试分类");
    match err {
        Error::Http { status, .. } => assert_eq!(status, 403),
        other => panic!("应为 Error::Http{{403}}（F-11），实际: {other}"),
    }
    let reqs = records.lock().unwrap();
    assert_eq!(
        reqs.iter().filter(|r| r.method == "PUT").count(),
        1,
        "OSS PUT 应恰一次（快速失败，无轮内重试）"
    );
}

// ============ T-24 成功回环 ============

/// initupload status=2 秒传命中：零传输、不请求 gettoken（请求计数断言），
/// 且对端可解密请求表单（target/filename/fileid 契约）。
#[test]
fn rapid_upload_hits_zero_transfer_and_skips_gettoken() {
    let data = b"hello cloudmount";
    let sha1 = sha1_hex_upper(data);
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, records) = spawn_mock(move |req| {
        match (req.method.as_str(), req.path_only()) {
            ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
            ("POST", "/4.0/initupload.php") => {
                let resp = encrypted_init_response(
                    &peer,
                    req,
                    r#"{"status":2,"statuscode":0,"pickcode":"PC2"}"#,
                );
                // 对端解密请求体（请求方向无 LZ4 帧）：锁定表单契约
                let k_ec = req.query("k_ec").expect("k_ec");
                let session = peer.session(&k_ec).expect("对端会话");
                let form =
                    String::from_utf8(session.decrypt_request(&req.body)).expect("表单 UTF-8");
                assert!(
                    form.contains("target=U_1_0"),
                    "秒传 target 应为 U_1_0: {form}"
                );
                assert!(form.contains("filename=hello.txt"), "应含文件名: {form}");
                assert!(
                    form.contains(&format!("fileid={sha1}")),
                    "fileid 应为大写 SHA1: {form}"
                );
                assert!(form.contains("userid=123"), "应含 user_id: {form}");
                resp
            }
            ("GET", "/files") => {
                MockResponse::json(200, list_ok("777", "hello.txt", &sha1, data.len() as u64))
            }
            other => panic!("mock 收到未预期请求: {other:?}"),
        }
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let result = provider
        .upload(
            &RemoteId("0".into()),
            "hello.txt",
            UploadSource::Bytes(data),
            data.len() as u64,
        )
        .expect("秒传命中应成功");
    assert!(result.rapid_upload, "status=2 应为秒传命中");
    assert_eq!(result.id.0, "777");

    let reqs = records.lock().unwrap();
    assert_eq!(
        reqs.len(),
        3,
        "秒传命中 = uploadinfo + initupload + 验证列表，恰三次请求"
    );
    assert!(
        !reqs.iter().any(|r| r.path_only().contains("gettoken")),
        "秒传命中不得请求 gettoken"
    );
    assert!(
        !reqs.iter().any(|r| r.method == "PUT"),
        "秒传命中零传输（无 OSS PUT）"
    );
}

/// initupload status=1 未命中：gettoken → OSS 直传（体为原文件字节、OSS V1
/// 签名与回调头齐全）→ 验证列表，rapid=false。
#[test]
fn upload_miss_goes_through_gettoken_and_oss_direct_put() {
    let data = b"direct-upload-payload";
    let sha1 = sha1_hex_upper(data);
    let (peer, peer_pub) = TestEcdhPeer::generate();
    let (addr, records) = spawn_mock(move |req| match (req.method.as_str(), req.path_only()) {
        ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
        ("POST", "/4.0/initupload.php") => encrypted_init_response(&peer, req, INIT_MISS),
        ("GET", "/3.0/gettoken.php") => MockResponse::json(200, OK_GETTOKEN),
        ("PUT", "/bkt/obj/f.bin") => {
            assert_eq!(req.body, data.to_vec(), "OSS PUT 体应为原文件字节");
            let auth = req.header("authorization").expect("应带 Authorization");
            assert!(auth.starts_with("OSS "), "应为 OSS V1 签名: {auth}");
            assert!(
                req.header("x-oss-security-token").is_some(),
                "应带 STS SecurityToken 头"
            );
            assert!(
                req.header("x-oss-callback").is_some(),
                "应带回调头（initupload 下发的 callback base64）"
            );
            MockResponse::bytes(200, Vec::new())
        }
        ("GET", "/files") => {
            MockResponse::json(200, list_ok("888", "f.bin", &sha1, data.len() as u64))
        }
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = cookie_provider(addr, Some(peer_pub));
    let result = provider
        .upload(
            &RemoteId("0".into()),
            "f.bin",
            UploadSource::Bytes(data),
            data.len() as u64,
        )
        .expect("OSS 直传成功应返回 Ok");
    assert!(!result.rapid_upload, "status=1 非秒传");
    assert_eq!(result.id.0, "888");

    let reqs = records.lock().unwrap();
    let paths: Vec<&str> = reqs.iter().map(|r| r.path_only()).collect();
    assert_eq!(
        paths,
        vec![
            "/app/uploadinfo",
            "/4.0/initupload.php",
            "/3.0/gettoken.php",
            "/bkt/obj/f.bin",
            "/files",
        ],
        "未命中请求序列：uploadinfo → initupload → gettoken → OSS PUT → 验证列表"
    );
}

// ============ 渲染回归（注入基址后的请求构造逐项锁定） ============

/// 注入基址后：list 完整查询串逐字节、uploadinfo/initupload 路径与头、
/// Cookie 头、initupload 专用 UA 与密文体。
#[test]
fn injected_bases_render_exact_requests() {
    let (addr, records) = spawn_mock(|req| match (req.method.as_str(), req.path_only()) {
        ("GET", "/files") => {
            MockResponse::json(200, r#"{"state":true,"code":0,"count":0,"data":[]}"#)
        }
        ("POST", "/app/uploadinfo") => MockResponse::json(200, OK_UPLOADINFO),
        // initupload 只验证请求构造；响应给非法密文（本用例不关心结果）
        ("POST", "/4.0/initupload.php") => MockResponse::bytes(200, vec![0u8; 16]),
        other => panic!("mock 收到未预期请求: {other:?}"),
    });

    let provider = provider_for(addr);
    let _ = provider.list(&RemoteId("0".into()));
    let _ = provider.upload(
        &RemoteId("0".into()),
        "r.txt",
        UploadSource::Bytes(b"abc"),
        3,
    );

    let reqs = records.lock().unwrap();
    assert_eq!(reqs.len(), 3, "list + uploadinfo + initupload");
    // 列表：完整查询串逐字节锁定（默认渲染等价性的可达面）
    assert_eq!(
        reqs[0].path,
        "/files?aid=1&cid=0&o=user_ptime&asc=0&offset=0&show_dir=1&limit=1000&snap=0&natsort=0&record_open_time=1&format=json&fc_mix=0"
    );
    assert_eq!(
        reqs[0].header("cookie").as_deref(),
        Some("UID=u1; CID=c1; SEID=s1"),
        "web 请求应带浏览器 Cookie 头"
    );
    // uploadinfo：POST + Cookie
    assert_eq!(reqs[1].method, "POST");
    assert_eq!(reqs[1].path_only(), "/app/uploadinfo");
    assert!(reqs[1].header("cookie").is_some());
    // initupload：路径 + k_ec + 三客户端家族 UA + 表单 Content-Type + 非空密文体
    assert!(
        reqs[2].path.starts_with("/4.0/initupload.php?k_ec="),
        "initupload 应渲染到注入基址下: {}",
        reqs[2].path
    );
    assert_eq!(
        reqs[2].header("content-type").as_deref(),
        Some("application/x-www-form-urlencoded")
    );
    assert!(
        reqs[2]
            .header("user-agent")
            .expect("initupload 应带专用 UA")
            .contains("115disk"),
        "initupload 应用 115disk 家族 UA"
    );
    assert!(!reqs[2].body.is_empty(), "initupload 应发送 ECDH 密文体");
    assert!(reqs[2].header("cookie").is_some());
}
