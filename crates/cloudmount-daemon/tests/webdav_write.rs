//! WebDAV 写路径集成测试（U11：F-08 spool 收口 + F-09 写打开错误混淆）。
//!
//! T-13（AC8/F-08）：PUT 全链路后 spool 清理——断言 RecordingProvider 捕获的
//!   **本次 PUT 精确 spool 路径**（`UploadSource::Path` 实参）不存在；flush 幂等
//!   （二次 flush no-op）；`provider.upload` 恰一次（OpCounts，排除 Drop 二次上传）。
//! T-14（AC8/F-08，windows-only）：删除失败注入——upload 回调内对本次 spool 精确
//!   路径开无共享句柄（`share_mode(0)`）并持有到 PUT 响应之后 → 不 panic、
//!   warn 日志含路径、接受残留、上传结果不受影响。
//! T-15（AC9/F-09）：ErrorInjectingProvider 对目标父目录 list 注入权限类非 NotFound
//!   错误 → PUT 返回 5xx 而非静默新建、远端原内容不变、upload 计数 == 0。
//! T-16（AC9/F-09 回归）：NotFound 正常新建；存在且可打开正常覆盖。
//!
//! flush 幂等 / Drop 自愈的直接驱动（dav-server 每 PUT 只 flush 一次，HTTP 面无法
//! 触发第二次 flush）经公开 API `ProviderFs::new` + `DavFileSystem::open` 黑盒完成。
#![cfg(feature = "mock")]

use cloudmount_core::{
    Error as CoreError, Provider, RangeSpec, RemoteEntry, RemoteId, Result, UploadResult,
    UploadSource,
};
use cloudmount_daemon::webdav::{serve, ProviderFs};
use cloudmount_sync::testutil::MockRemote;
use dav_server::davpath::DavPath;
use dav_server::fs::{DavFile, DavFileSystem};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ==== 测试侧 Provider 包装族（Arc<dyn Provider> 委托，零生产改动）====

/// 委托宏：`list`/`upload` 由各包装显式提供（extra 项），其余透传 `inner()`。
macro_rules! provider_wrapper {
    ($t:ty { $($extra:item)* }) => {
        impl Provider for $t {
            fn name(&self) -> &'static str {
                "u11-test-wrapper"
            }
            fn stat(&self, id: &RemoteId) -> Result<RemoteEntry> {
                self.inner().stat(id)
            }
            fn open_range(
                &self,
                id: &RemoteId,
                range: RangeSpec,
            ) -> Result<Box<dyn std::io::Read + Send>> {
                self.inner().open_range(id, range)
            }
            fn create_dir(&self, parent: &RemoteId, name: &str) -> Result<RemoteEntry> {
                self.inner().create_dir(parent, name)
            }
            fn rename(&self, id: &RemoteId, new_name: &str) -> Result<()> {
                self.inner().rename(id, new_name)
            }
            fn move_entry(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<()> {
                self.inner().move_entry(id, new_parent)
            }
            fn copy(&self, id: &RemoteId, new_parent: &RemoteId) -> Result<RemoteId> {
                self.inner().copy(id, new_parent)
            }
            fn delete(&self, id: &RemoteId) -> Result<()> {
                self.inner().delete(id)
            }
            fn quota(&self) -> Result<cloudmount_core::Quota> {
                self.inner().quota()
            }
            $($extra)*
        }
    };
}

/// T-13：捕获 `upload` 收到的 `UploadSource::Path` 精确 spool 路径（按调用序）。
struct RecordingProvider {
    inner: Arc<MockRemote>,
    uploads: Arc<Mutex<Vec<PathBuf>>>,
}

impl RecordingProvider {
    fn new(inner: Arc<MockRemote>) -> Self {
        Self {
            inner,
            uploads: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn inner(&self) -> &Arc<MockRemote> {
        &self.inner
    }
    fn uploads(&self) -> Vec<PathBuf> {
        self.uploads.lock().unwrap().clone()
    }
}

provider_wrapper!(RecordingProvider {
    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.inner().list(dir)
    }
    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        if let UploadSource::Path(p) = &source {
            self.uploads.lock().unwrap().push(p.to_path_buf());
        }
        self.inner.upload(parent, name, source, size)
    }
});

/// T-13 Drop 自愈腿：`upload` 一律失败（可编程失败注入的最小形态）。
struct FailingUploadProvider {
    inner: Arc<MockRemote>,
    uploads: Arc<Mutex<Vec<PathBuf>>>,
}

impl FailingUploadProvider {
    fn new(inner: Arc<MockRemote>) -> Self {
        Self {
            inner,
            uploads: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn inner(&self) -> &Arc<MockRemote> {
        &self.inner
    }
    fn uploads(&self) -> Vec<PathBuf> {
        self.uploads.lock().unwrap().clone()
    }
}

provider_wrapper!(FailingUploadProvider {
    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.inner().list(dir)
    }
    fn upload(
        &self,
        _parent: &RemoteId,
        _name: &str,
        source: UploadSource<'_>,
        _size: u64,
    ) -> Result<UploadResult> {
        if let UploadSource::Path(p) = &source {
            self.uploads.lock().unwrap().push(p.to_path_buf());
        }
        Err(CoreError::Http {
            status: 500,
            body: "injected: upload 失败（Drop 自愈腿）".into(),
        })
    }
});

/// T-15：可编程对指定目录的 `list` 返回权限/IO 类**非 NotFound** 错误。
struct ErrorInjectingProvider {
    inner: Arc<MockRemote>,
    /// Some(id) → `list(该 id)` 返回注入错误。
    fail_list_for: Mutex<Option<String>>,
}

impl ErrorInjectingProvider {
    fn new(inner: Arc<MockRemote>) -> Self {
        Self {
            inner,
            fail_list_for: Mutex::new(None),
        }
    }
    fn inner(&self) -> &Arc<MockRemote> {
        &self.inner
    }
    fn arm(&self, dir_id: &RemoteId) {
        *self.fail_list_for.lock().unwrap() = Some(dir_id.0.clone());
    }
}

provider_wrapper!(ErrorInjectingProvider {
    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        if self.fail_list_for.lock().unwrap().as_deref() == Some(dir.0.as_str()) {
            return Err(CoreError::Other(
                "injected: 目录 list 被拒（权限类，非 NotFound）".into(),
            ));
        }
        self.inner.list(dir)
    }
    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        self.inner.upload(parent, name, source, size)
    }
});

/// T-14（windows-only）：`upload` 完成后对本次 spool 精确路径开**无共享句柄**
/// （`share_mode(0)`）并持有，模拟外部进程独占——flush 随后的 `remove_file`
/// 将因共享冲突失败。
#[cfg(windows)]
struct HoldExclusiveProvider {
    inner: Arc<MockRemote>,
    uploads: Arc<Mutex<Vec<PathBuf>>>,
    /// 持有的无共享句柄（测试断言完成后经 `release` 释放）。
    held: Arc<Mutex<Option<std::fs::File>>>,
    open_failed: Arc<AtomicBool>,
}

#[cfg(windows)]
impl HoldExclusiveProvider {
    fn new(inner: Arc<MockRemote>) -> Self {
        Self {
            inner,
            uploads: Arc::new(Mutex::new(Vec::new())),
            held: Arc::new(Mutex::new(None)),
            open_failed: Arc::new(AtomicBool::new(false)),
        }
    }
    fn inner(&self) -> &Arc<MockRemote> {
        &self.inner
    }
    fn uploads(&self) -> Vec<PathBuf> {
        self.uploads.lock().unwrap().clone()
    }
    fn open_failed(&self) -> bool {
        self.open_failed.load(Ordering::SeqCst)
    }
    fn release(&self) {
        *self.held.lock().unwrap() = None;
    }
}

#[cfg(windows)]
provider_wrapper!(HoldExclusiveProvider {
    fn list(&self, dir: &RemoteId) -> Result<Vec<RemoteEntry>> {
        self.inner().list(dir)
    }
    fn upload(
        &self,
        parent: &RemoteId,
        name: &str,
        source: UploadSource<'_>,
        size: u64,
    ) -> Result<UploadResult> {
        let mut path: Option<PathBuf> = None;
        if let UploadSource::Path(p) = &source {
            path = Some(p.to_path_buf());
            self.uploads.lock().unwrap().push(p.to_path_buf());
        }
        // 先完成上传（内容读入远端），再持有独占句柄
        let r = self.inner.upload(parent, name, source, size);
        if let Some(p) = path {
            use std::os::windows::fs::OpenOptionsExt;
            match std::fs::OpenOptions::new().read(true).share_mode(0).open(&p) {
                Ok(f) => *self.held.lock().unwrap() = Some(f),
                Err(_) => self.open_failed.store(true, Ordering::SeqCst),
            }
        }
        r
    }
});

// ==== 日志捕获（手写 tracing Subscriber，仅 tracing 本体；windows-only T-14 断言面）====

#[cfg(windows)]
fn log_buf() -> &'static Arc<Mutex<Vec<String>>> {
    static BUF: std::sync::OnceLock<Arc<Mutex<Vec<String>>>> = std::sync::OnceLock::new();
    BUF.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

#[cfg(windows)]
fn install_log_capture() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = tracing::subscriber::set_global_default(log_capture::CapturingSubscriber {
            buf: log_buf().clone(),
        });
    });
}

/// 轮询捕获缓冲直到出现含 `needle` 的行（全局缓冲多测试共享、按唯一路径过滤）。
#[cfg(windows)]
fn wait_for_log_containing(needle: &str, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if log_buf().lock().unwrap().iter().any(|l| l.contains(needle)) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(windows)]
mod log_capture {
    use std::fmt;
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Id, Metadata, Subscriber};

    pub(crate) struct CapturingSubscriber {
        pub(crate) buf: Arc<Mutex<Vec<String>>>,
    }

    struct EventVisitor {
        message: String,
        fields: Vec<String>,
    }

    impl Visit for EventVisitor {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            if field.name() == "message" {
                self.message = format!("{value:?}");
            } else {
                self.fields.push(format!("{}={:?}", field.name(), value));
            }
        }
        fn record_str(&mut self, field: &Field, value: &str) {
            if field.name() == "message" {
                self.message = value.to_string();
            } else {
                self.fields.push(format!("{}={}", field.name(), value));
            }
        }
    }

    impl Subscriber for CapturingSubscriber {
        fn enabled(&self, _meta: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _attrs: &tracing::span::Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _span: &Id, _values: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _from: &Id, _to: &Id) {}
        fn event(&self, event: &Event<'_>) {
            let mut v = EventVisitor {
                message: String::new(),
                fields: Vec::new(),
            };
            event.record(&mut v);
            let mut line = format!(
                "{:?} {}",
                event.metadata().level(),
                event.metadata().target()
            );
            if !v.message.is_empty() {
                line.push(' ');
                line.push_str(&v.message);
            }
            for f in &v.fields {
                line.push(' ');
                line.push_str(f);
            }
            self.buf.lock().unwrap().push(line);
        }
        fn enter(&self, _span: &Id) {}
        fn exit(&self, _span: &Id) {}
    }
}

// ==== harness（随机端口 + hyper 客户端；/dav 面不涉 web_ui_dir，传 None）====

/// 启动服务（任意 `Arc<dyn Provider>` 注入口，与 U10 注入机制同源）。
async fn spawn_server(provider: Arc<dyn Provider>) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let listen = format!("127.0.0.1:{port}");
    tokio::spawn(async move {
        serve(provider, &listen, None, None, None).await.ok();
    });
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    port
}

/// HTTP 客户端请求（hyper）。
async fn request(port: u16, method: &str, path: &str, body: Option<Vec<u8>>) -> (u16, Vec<u8>) {
    use hyper::client::Client;
    use hyper::http::Request;
    let client: Client<hyper::client::HttpConnector, hyper::Body> = Client::new();
    let uri: hyper::Uri = format!("http://127.0.0.1:{port}{path}").parse().unwrap();
    let mut builder = Request::builder().method(method).uri(uri);
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

// ==== 直接驱动 DavFile 写路径的黑盒出口（公开 API：ProviderFs::new + DavFileSystem::open）====

/// 与 dav-server handle_put 一致的写打开选项。
fn write_open_options() -> dav_server::fs::OpenOptions {
    dav_server::fs::OpenOptions {
        read: false,
        write: true,
        append: false,
        truncate: true,
        create: true,
        create_new: false,
        size: None,
        checksum: None,
    }
}

async fn open_write(fs: &ProviderFs, path: &str) -> Box<dyn DavFile> {
    let dav_path = DavPath::new(path).expect("测试路径合法");
    fs.open(&dav_path, write_open_options())
        .await
        .expect("写 open 应成功")
}

// ==== 单元腿：flush 幂等 + Drop 自愈（HTTP 面无法触发第二次 flush / 无 flush Drop）====

/// T-13：flush 幂等——二次 flush no-op；`provider.upload` 恰一次；Drop 不二次上传；
/// spool 精确路径（RecordingProvider 捕获）最终不存在；远端内容正确。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flush_is_idempotent_and_upload_exactly_once() {
    let remote = Arc::new(MockRemote::new());
    let recorder = Arc::new(RecordingProvider::new(remote.clone()));
    let fs = ProviderFs::new(recorder.clone() as Arc<dyn Provider>);

    let mut file = open_write(&fs, "/t13-unit.txt").await;
    file.write_bytes(bytes::Bytes::from_static(b"t13-unit-body"))
        .await
        .expect("写 spool 应成功");

    file.flush().await.expect("首次 flush 应成功");
    assert_eq!(
        remote.ops().upload.load(Ordering::SeqCst),
        1,
        "首次 flush 应恰一次上传"
    );
    let uploads = recorder.uploads();
    assert_eq!(uploads.len(), 1, "应捕获恰好一条 spool 路径: {uploads:?}");
    let spool = uploads[0].clone();
    assert!(
        spool
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("cm-dav-put-")),
        "捕获路径应为 spool 形态: {spool:?}"
    );

    // 二次 flush：幂等 no-op，不得重复上传
    file.flush().await.expect("二次 flush 应 no-op 成功");
    assert_eq!(
        remote.ops().upload.load(Ordering::SeqCst),
        1,
        "二次 flush 不得重复上传（flush 幂等护栏）"
    );

    // Drop：不得再次上传；spool 已清理
    drop(file);
    assert_eq!(
        remote.ops().upload.load(Ordering::SeqCst),
        1,
        "Drop 不得二次上传"
    );
    assert!(!spool.exists(), "spool {spool:?} 应已清理");
    assert_eq!(remote.read("t13-unit.txt").unwrap(), b"t13-unit-body");
}

/// T-13 Drop 自愈腿：上传失败 → flush 如实上抛、spool 暂留（重试窗口）→
/// Drop 清理未成功上传的 spool 残留且不再尝试上传。
/// （"写一半断连未 flush 即 Drop"与"上传失败后 Drop"同走 Drop 的 `!flushed` 分支。）
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_upload_drop_self_heals_spool() {
    let remote = Arc::new(MockRemote::new());
    let failing = Arc::new(FailingUploadProvider::new(remote.clone()));
    let fs = ProviderFs::new(failing.clone() as Arc<dyn Provider>);

    let mut file = open_write(&fs, "/t13-heal.txt").await;
    file.write_bytes(bytes::Bytes::from_static(b"heal"))
        .await
        .expect("写 spool 应成功");

    let r = file.flush().await;
    assert!(r.is_err(), "上传失败应如实上抛，不得假成功");
    assert_eq!(failing.uploads().len(), 1, "上传应被尝试恰一次");
    let spool = failing.uploads()[0].clone();
    assert!(
        spool.exists(),
        "上传失败后 spool 应暂留（Drop 兜底清理的重试窗口）: {spool:?}"
    );

    drop(file);
    assert!(
        !spool.exists(),
        "Drop 应自愈清理未上传的 spool 残留: {spool:?}"
    );
    assert_eq!(failing.uploads().len(), 1, "Drop 不得再次尝试上传");
    assert_eq!(
        remote.ops().upload.load(Ordering::SeqCst),
        0,
        "不得有成功上传"
    );
}

// ==== T-13 集成腿：PUT 全链路（dav-server → ProviderFs → flush → upload）====

/// PUT 全链路后：对 RecordingProvider 捕获的**本次精确 spool 路径**断言不存在；
/// upload 恰一次（OpCounts，排除 Drop 二次上传）；远端内容一致。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_full_chain_spool_exact_path_removed_upload_once() {
    let remote = Arc::new(MockRemote::new());
    remote.put_dir("dir");
    let recorder = Arc::new(RecordingProvider::new(remote.clone()));
    let port = spawn_server(recorder.clone() as Arc<dyn Provider>).await;

    let (status, _) = request(
        port,
        "PUT",
        "/dav/dir/t13.txt",
        Some(b"t13-payload".to_vec()),
    )
    .await;
    assert_eq!(status, 201, "PUT 新文件应 201 Created");

    let uploads = recorder.uploads();
    assert_eq!(uploads.len(), 1, "provider.upload 应恰一次: {uploads:?}");
    let spool = uploads[0].clone();
    assert!(
        spool
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("cm-dav-put-")),
        "捕获的应是本次 PUT 的 spool 路径: {spool:?}"
    );
    assert_eq!(
        remote.read("dir/t13.txt").unwrap(),
        b"t13-payload",
        "远端内容应与 PUT 一致"
    );

    // 留出句柄收尾/Drop 的竞态窗口后复核：无二次上传
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        remote.ops().upload.load(Ordering::SeqCst),
        1,
        "PUT 生命周期内 upload 恰一次（排除 Drop 二次上传）"
    );
    assert!(
        !spool.exists(),
        "PUT 完成后本次 spool 精确路径应不存在（勿 glob——防并行互踩）: {spool:?}"
    );
}

// ==== T-14（windows-only）：删除失败注入 → warn + 接受残留，上传结果不受影响 ====

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_delete_conflict_warns_and_accepts_residue() {
    install_log_capture();
    let remote = Arc::new(MockRemote::new());
    remote.put_dir("dir");
    let holder = Arc::new(HoldExclusiveProvider::new(remote.clone()));
    let port = spawn_server(holder.clone() as Arc<dyn Provider>).await;

    let (status, _) = request(
        port,
        "PUT",
        "/dav/dir/t14.txt",
        Some(b"t14-payload".to_vec()),
    )
    .await;
    // 上传成功如实成功——删除失败不得掩盖上传结果
    assert!(
        (200..300).contains(&status),
        "上传成功不得被删除失败掩盖，实际 {status}"
    );
    assert_eq!(
        remote.read("dir/t14.txt").unwrap(),
        b"t14-payload",
        "上传结果应如实生效"
    );

    let uploads = holder.uploads();
    assert_eq!(uploads.len(), 1, "upload 应恰一次");
    let spool = uploads[0].clone();
    assert!(
        !holder.open_failed(),
        "upload 期间应能独占打开 spool——flush 必须先关闭自身句柄\
         （现状 as_ref 半泄漏句柄会挡住 share_mode(0) 的独占打开）"
    );

    // dav-server 在 flush 完成后才响应 PUT——remove_file 冲突失败已发生，warn 已落
    assert!(
        wait_for_log_containing(&spool.display().to_string(), Duration::from_secs(5)),
        "warn 日志应含 spool 路径（删除失败可观测）: {spool:?}"
    );
    // 接受残留：残留存在即预期终态（不做"重试最终成功"断言）
    assert!(spool.exists(), "删除失败应接受残留（预期终态）: {spool:?}");

    holder.release(); // 断言完成后释放冲突句柄
    let _ = std::fs::remove_file(&spool); // 测试自身收尾（非断言面）
}

// ==== T-15：写打开错误混淆（Err(_) 盲当新文件）====

/// 注入目标父目录 list 的权限类非 NotFound 错误 → PUT 既有文件应 5xx
/// （非静默新建）、远端原内容不变、upload 计数 == 0。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_open_error_not_masked_as_new_file() {
    let remote = Arc::new(MockRemote::new());
    remote.put_dir("dir");
    remote.put_file("dir/file.txt", b"ORIGINAL-CONTENT");
    let injector = Arc::new(ErrorInjectingProvider::new(remote.clone()));
    let provider: Arc<dyn Provider> = injector.clone();

    // 解析 dir 的远端 id（未布防时 list 透传），随后布防
    let root = provider.list(&RemoteId("0".into())).expect("list root");
    let dir_id = root
        .iter()
        .find(|e| e.name == "dir")
        .expect("dir 应存在")
        .id
        .clone();
    injector.arm(&dir_id);

    let port = spawn_server(provider).await;
    let (status, body) = request(
        port,
        "PUT",
        "/dav/dir/file.txt",
        Some(b"NEW-CONTENT".to_vec()),
    )
    .await;
    assert!(
        status >= 500,
        "打开错误应上抛为 5xx 而非静默新建，实际 {status} body={}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(
        remote.read("dir/file.txt").unwrap(),
        b"ORIGINAL-CONTENT",
        "远端原内容不得被静默覆盖"
    );
    assert_eq!(
        remote.ops().upload.load(Ordering::SeqCst),
        0,
        "打开失败时 upload 不得被调用"
    );
}

// ==== T-16：回归护栏——NotFound 正常新建；存在且可打开正常覆盖 ====

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn put_notfound_creates_and_existing_overwrites() {
    let remote = Arc::new(MockRemote::new());
    let port = spawn_server(remote.clone() as Arc<dyn Provider>).await;

    // NotFound → 合成新条目正常新建
    let (status, _) = request(port, "PUT", "/dav/t16-new.txt", Some(b"v1".to_vec())).await;
    assert_eq!(status, 201, "PUT 新文件应 201 Created");
    assert_eq!(remote.read("t16-new.txt").unwrap(), b"v1");

    // 存在且可打开 → 正常覆盖
    let (status, _) = request(port, "PUT", "/dav/t16-new.txt", Some(b"v2-longer".to_vec())).await;
    assert_eq!(status, 204, "PUT 覆盖既有文件应 204 No Content");
    assert_eq!(remote.read("t16-new.txt").unwrap(), b"v2-longer");

    // 父目录缺失 → 仍 NotFound（4xx），收窄后不得误新建
    let (status, _) = request(port, "PUT", "/dav/missing-dir/t16.txt", Some(b"x".to_vec())).await;
    assert!(
        (400..500).contains(&status),
        "父目录缺失应 4xx（非新建），实际 {status}"
    );
}
