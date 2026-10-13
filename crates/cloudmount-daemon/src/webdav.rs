//! WebDAV 出口（M4 边缘网关）：把 [`Provider`] 暴露为 WebDAV 文件系统。
//!
//! 局域网设备（Windows 资源管理器 / macOS Finder / Linux davfs / rclone）
//! 经 WebDAV 挂载读写 115。这是"边缘设备挂载"的核心出口——不依赖
//! CfAPI 的平台（NAS/路由器/树莓派）都能用。
//!
//! ## 架构
//!
//! ```text
//! hyper HTTP 服务（--serve）
//!   ├── /dav/*  → dav-server DavHandler → ProviderFs（本模块）
//!   │                                    └── Provider（官方/Cookie/路由）
//!   └── 其余   → web-ui 静态托管（状态页/远程管理）
//! ```
//!
//! ## 路径 ↔ ID 解析
//!
//! 115 的文件系统是 **ID 寻址**（cid/fid），WebDAV 是**路径寻址**。
//! [`ProviderFs`] 维护路径缓存：解析时逐级 `list` 匹配名称；写操作后
//! 失效整个缓存（保守但正确，v2 可做子树级失效）。
//!
//! ## 阻塞桥接
//!
//! Provider 是同步实现（ureq）；DavFileSystem 是 async trait。所有
//! Provider 调用经 `spawn_blocking` 跑在阻塞线程池，不卡 async 运行时。
//!
//! ## v1 边界
//!
//! - HTTP（无 TLS）——LAN 场景；TLS（rustls + 证书管理）见 M4 后续；
//! - PUT 经临时文件中转（spool）再上传——WebDAV PUT 无流式语义；
//! - 锁系统用 FakeLs（Windows/macOS 挂载兼容所需的最小 LOCK 支持）。

use cloudmount_core::provider::UploadSource;
use cloudmount_core::{EntryKind, Error as CoreError, Provider, RangeSpec, RemoteEntry, RemoteId};
use dav_server::davpath::DavPath;
use dav_server::fs::*;
use dav_server::DavHandler;
use futures_util::future::FutureExt;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// 路径缓存条目。
#[derive(Debug, Clone)]
struct CacheEntry {
    entry: RemoteEntry,
    /// 缓存时间（TTL 失效）。
    at: SystemTime,
}

/// 路径缓存 TTL（秒）：远端变更经探测轮次反映，WebDAV 客户端
/// 重复 PROPFIND 走缓存避免逐级 list。
const CACHE_TTL: Duration = Duration::from_secs(30);

/// 基于 [`Provider`] 的 WebDAV 文件系统。
///
/// `Clone`（cache 经 Arc 共享）满足 dav-server 的 `BoxCloneFs` 约束。
#[derive(Clone)]
pub struct ProviderFs {
    provider: Arc<dyn Provider>,
    /// 路径（"/docs/a.txt"）→ 条目缓存。
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl ProviderFs {
    /// 创建（`provider` 为官方/Cookie/路由后的通道）。
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 路径归一化：URL 解码 + 折叠斜杠（"/a//b/%E6%96%87" → "/a/b/文"）。
    ///
    /// DavPath 给出的是百分号编码路径——不解码无法与远端条目名匹配。
    fn normalize(path: &DavPath) -> String {
        let s = path.as_url_string();
        let decoded = percent_decode(&s);
        let mut out = String::new();
        for comp in decoded.split('/') {
            if !comp.is_empty() {
                out.push('/');
                out.push_str(comp);
            }
        }
        if out.is_empty() {
            out.push('/');
        }
        out
    }

    /// 失效全部缓存（写操作后调用）。
    fn invalidate(&self) {
        if let Ok(mut c) = self.cache.write() {
            c.clear();
        }
    }
}

/// 路径解析（循环实现——async 递归需装箱且栈不友好）。
///
/// "/a/b/c" → 从根逐级 list 匹配；每级前查缓存（TTL 30s）。
async fn resolve_path(fs: &ProviderFs, path: &str) -> dav_server::fs::FsResult<RemoteEntry> {
    if path == "/" {
        return Ok(root_entry());
    }
    let comps: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
    // 找最深的有缓存前缀
    let mut start_idx = 0;
    let mut current = root_entry();
    for (i, _) in comps.iter().enumerate() {
        let prefix = format!("/{}", comps[..=i].join("/"));
        let hit = fs.cache.read().ok().and_then(|c| {
            c.get(&prefix)
                .filter(|e| e.at.elapsed().map(|t| t < CACHE_TTL).unwrap_or(true))
                .map(|e| e.entry.clone())
        });
        if let Some(entry) = hit {
            current = entry;
            start_idx = i + 1;
        }
    }
    // 从 start_idx 逐级下降
    for (i, comp) in comps.iter().enumerate().skip(start_idx) {
        if current.kind != EntryKind::Directory {
            return Err(FsError::NotFound);
        }
        let pid = current.id.clone();
        let entries = {
            let provider = fs.provider.clone();
            tokio::task::spawn_blocking(move || provider.list(&pid))
                .await
                .map_err(|_| FsError::GeneralFailure)?
                .map_err(map_err)?
        };
        let found = entries.into_iter().find(|e| &e.name == comp);
        match found {
            Some(entry) => {
                let prefix = format!("/{}", comps[..=i].join("/"));
                if let Ok(mut c) = fs.cache.write() {
                    c.insert(
                        prefix,
                        CacheEntry {
                            entry: entry.clone(),
                            at: SystemTime::now(),
                        },
                    );
                }
                current = entry;
            }
            None => return Err(FsError::NotFound),
        }
    }
    Ok(current)
}

/// 根目录条目（远端根 cid=0）。
fn root_entry() -> RemoteEntry {
    RemoteEntry {
        id: RemoteId("0".into()),
        parent_id: RemoteId(String::new()),
        name: String::new(),
        kind: EntryKind::Directory,
        size: 0,
        mtime: 0,
        sha1: None,
        pick_code: None,
        deleted: false,
    }
}

/// 核心错误 → FsError 映射（FsError 无载荷——细节走日志）。
fn map_err(e: CoreError) -> FsError {
    tracing::warn!(error = %e, "WebDAV Provider 调用失败");
    match e {
        CoreError::NotFound(_) => FsError::NotFound,
        CoreError::Auth(_) => FsError::GeneralFailure,
        CoreError::RateLimited { .. } => FsError::GeneralFailure,
        CoreError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => FsError::NotFound,
        _other => FsError::GeneralFailure,
    }
}

/// 远端条目的元数据适配。
#[derive(Debug, Clone)]
struct EntryMeta {
    entry: RemoteEntry,
}

impl DavMetaData for EntryMeta {
    fn len(&self) -> u64 {
        self.entry.size
    }
    fn modified(&self) -> dav_server::fs::FsResult<SystemTime> {
        Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(self.entry.mtime.max(0) as u64))
    }
    fn is_dir(&self) -> bool {
        self.entry.kind == EntryKind::Directory
    }
    fn etag(&self) -> Option<String> {
        // 用远端 ID + mtime（比默认的 len+time 更能反映远端变更）
        Some(format!("\"{}-{}\"", self.entry.id.0, self.entry.mtime))
    }
}

/// 目录条目适配。
struct DirEntry {
    entry: RemoteEntry,
}

impl DavDirEntry for DirEntry {
    fn name(&self) -> Vec<u8> {
        self.entry.name.clone().into_bytes()
    }
    fn metadata<'a>(&'a self) -> FsFuture<'a, Box<dyn DavMetaData>> {
        futures_util::future::ready(Ok(Box::new(EntryMeta {
            entry: self.entry.clone(),
        }) as Box<dyn DavMetaData>))
        .boxed()
    }
    fn is_dir<'a>(&'a self) -> FsFuture<'a, bool> {
        futures_util::future::ready(Ok(self.entry.kind == EntryKind::Directory)).boxed()
    }
}

/// 读文件句柄（open_range 的流包装）。
///
/// 状态经 `Arc<Mutex<..>>` 持有：async 方法不借用 `&mut self`（避免
/// future 生命周期逃逸），且满足 `DavFile: Send + Sync`。
struct ReadFile {
    meta: EntryMeta,
    reader: Arc<std::sync::Mutex<Option<Box<dyn Read + Send>>>>,
    pos: u64,
}

impl std::fmt::Debug for ReadFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadFile").field("pos", &self.pos).finish()
    }
}

impl DavFile for ReadFile {
    fn metadata<'a>(&'a mut self) -> FsFuture<'a, Box<dyn DavMetaData>> {
        futures_util::future::ready(Ok(Box::new(self.meta.clone()) as Box<dyn DavMetaData>)).boxed()
    }
    fn write_buf<'a>(&'a mut self, _buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'a, ()> {
        futures_util::future::ready(Err(FsError::NotImplemented)).boxed()
    }
    fn write_bytes<'a>(&'a mut self, _buf: bytes::Bytes) -> FsFuture<'a, ()> {
        futures_util::future::ready(Err(FsError::NotImplemented)).boxed()
    }
    fn read_bytes<'a>(&'a mut self, count: usize) -> FsFuture<'a, bytes::Bytes> {
        let reader = self.reader.clone();
        async move {
            let buf = tokio::task::spawn_blocking(move || {
                let mut buf = vec![0u8; count];
                let mut guard = reader.lock().map_err(|_| FsError::GeneralFailure)?;
                let r = guard.as_mut().ok_or(FsError::GeneralFailure)?;
                use std::io::Read as _;
                let n = r.read(&mut buf).map_err(|_| FsError::GeneralFailure)?;
                buf.truncate(n);
                Ok::<_, FsError>(buf)
            })
            .await
            .map_err(|_| FsError::GeneralFailure)??;
            Ok(bytes::Bytes::from(buf))
        }
        .boxed()
    }
    fn seek<'a>(&'a mut self, _pos: std::io::SeekFrom) -> FsFuture<'a, u64> {
        // v1 不支持服务端 seek（客户端 Range 由重新 open 满足）
        futures_util::future::ready(Err(FsError::NotImplemented)).boxed()
    }
    fn flush<'a>(&'a mut self) -> FsFuture<'a, ()> {
        futures_util::future::ready(Ok(())).boxed()
    }
}

impl DavFileSystem for ProviderFs {
    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: OpenOptions,
    ) -> FsFuture<'a, Box<dyn DavFile>> {
        let norm = Self::normalize(path);
        async move {
            // 写路径允许目标不存在（PUT 新文件）：解析失败时解析父目录构造合成条目
            let entry = if options.write || options.create || options.append {
                match resolve_path(self, &norm).await {
                    Ok(e) => e,
                    Err(_) => {
                        let (parent, name) = split_last(&norm)?;
                        let parent_entry = resolve_path(self, &parent).await?;
                        RemoteEntry {
                            id: RemoteId(String::new()),
                            parent_id: parent_entry.id.clone(),
                            name,
                            kind: EntryKind::File,
                            size: 0,
                            mtime: 0,
                            sha1: None,
                            pick_code: None,
                            deleted: false,
                        }
                    }
                }
            } else {
                resolve_path(self, &norm).await?
            };
            if entry.kind == EntryKind::Directory {
                return Err(FsError::GeneralFailure);
            }
            if options.write || options.create || options.append {
                // PUT：spool 临时文件，flush 上传
                let spool = std::env::temp_dir().join(format!(
                    "cm-dav-put-{}-{:?}",
                    std::process::id(),
                    SystemTime::now()
                ));
                let file = std::fs::File::create(&spool).map_err(|_| FsError::GeneralFailure)?;
                let meta = EntryMeta { entry };
                Ok(Box::new(WriteFileWithProvider {
                    meta: meta.clone(),
                    spool,
                    file: Arc::new(std::sync::Mutex::new(Some(file))),
                    written: 0,
                    provider: self.provider.clone(),
                }) as Box<dyn DavFile>)
            } else {
                // GET：open_range 全量流
                let id = entry.id.clone();
                let size = entry.size;
                let provider = self.provider.clone();
                let reader = tokio::task::spawn_blocking(move || {
                    provider.open_range(
                        &id,
                        RangeSpec {
                            start: 0,
                            end: None,
                        },
                    )
                })
                .await
                .map_err(|_| FsError::GeneralFailure)?
                .map_err(map_err)?;
                let _ = size;
                Ok(Box::new(ReadFile {
                    meta: EntryMeta { entry },
                    reader: Arc::new(std::sync::Mutex::new(Some(reader))),
                    pos: 0,
                }) as Box<dyn DavFile>)
            }
        }
        .boxed()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        let norm = Self::normalize(path);
        async move {
            let entry = resolve_path(self, &norm).await?;
            if entry.kind != EntryKind::Directory {
                return Err(FsError::GeneralFailure);
            }
            let id = entry.id.clone();
            let provider = self.provider.clone();
            let entries = tokio::task::spawn_blocking(move || provider.list(&id))
                .await
                .map_err(|_| FsError::GeneralFailure)?
                .map_err(map_err)?;
            // 缓存子项（后续 PROPFIND/GET 直达）
            if let Ok(mut c) = self.cache.write() {
                let now = SystemTime::now();
                for e in &entries {
                    let child = format!("{}/{}", norm.trim_end_matches('/'), e.name);
                    c.insert(
                        child,
                        CacheEntry {
                            entry: e.clone(),
                            at: now,
                        },
                    );
                }
            }
            let items: Vec<Box<dyn DavDirEntry>> = entries
                .into_iter()
                .map(|e| Box::new(DirEntry { entry: e }) as Box<dyn DavDirEntry>)
                .collect();
            let stream = futures_util::stream::iter(items);
            Ok(stream.boxed() as FsStream<Box<dyn DavDirEntry>>)
        }
        .boxed()
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, Box<dyn DavMetaData>> {
        let norm = Self::normalize(path);
        async move {
            let entry = resolve_path(self, &norm).await?;
            Ok(Box::new(EntryMeta { entry }) as Box<dyn DavMetaData>)
        }
        .boxed()
    }

    fn create_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        let norm = Self::normalize(path);
        async move {
            let (parent, name) = split_last(&norm)?;
            let parent_entry = resolve_path(self, &parent).await?;
            let pid = parent_entry.id.clone();
            let provider = self.provider.clone();
            tokio::task::spawn_blocking(move || provider.create_dir(&pid, &name))
                .await
                .map_err(|_| FsError::GeneralFailure)?
                .map_err(map_err)?;
            self.invalidate();
            Ok(())
        }
        .boxed()
    }

    fn remove_file<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        remove_entry(self, path).boxed()
    }

    fn remove_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        remove_entry(self, path).boxed()
    }

    fn rename<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        let from_norm = Self::normalize(from);
        let to_norm = Self::normalize(to);
        async move {
            let entry = resolve_path(self, &from_norm).await?;
            let (to_parent, to_name) = split_last(&to_norm)?;
            let to_parent_entry = resolve_path(self, &to_parent).await?;
            // 同父 → 纯重命名；跨父 → move + rename
            if entry.parent_id.0 != to_parent_entry.id.0 {
                let id = entry.id.clone();
                let new_parent = to_parent_entry.id.clone();
                let provider = self.provider.clone();
                tokio::task::spawn_blocking(move || provider.move_entry(&id, &new_parent))
                    .await
                    .map_err(|_| FsError::GeneralFailure)?
                    .map_err(map_err)?;
            }
            if entry.name != to_name {
                let id = entry.id.clone();
                let name = to_name.clone();
                let provider = self.provider.clone();
                tokio::task::spawn_blocking(move || provider.rename(&id, &name))
                    .await
                    .map_err(|_| FsError::GeneralFailure)?
                    .map_err(map_err)?;
            }
            self.invalidate();
            Ok(())
        }
        .boxed()
    }

    fn copy<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        let from_norm = Self::normalize(from);
        let to_norm = Self::normalize(to);
        async move {
            let entry = resolve_path(self, &from_norm).await?;
            let (to_parent, _name) = split_last(&to_norm)?;
            let to_parent_entry = resolve_path(self, &to_parent).await?;
            let id = entry.id.clone();
            let new_parent = to_parent_entry.id.clone();
            let provider = self.provider.clone();
            tokio::task::spawn_blocking(move || provider.copy(&id, &new_parent))
                .await
                .map_err(|_| FsError::GeneralFailure)?
                .map_err(map_err)?;
            self.invalidate();
            Ok(())
        }
        .boxed()
    }
}

/// 删除（文件与目录同路——115 的删除进回收站）。
async fn remove_entry(fs: &ProviderFs, path: &DavPath) -> dav_server::fs::FsResult<()> {
    let norm = ProviderFs::normalize(path);
    let entry = resolve_path(fs, &norm).await?;
    let id = entry.id.clone();
    let provider = fs.provider.clone();
    tokio::task::spawn_blocking(move || provider.delete(&id))
        .await
        .map_err(|_| FsError::GeneralFailure)?
        .map_err(map_err)?;
    fs.invalidate();
    Ok(())
}

/// 百分号解码（UTF-8；非法序列原样保留）。
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 && i + 2 <= bytes.len() - 1 + 1 {
            let hex = |b: u8| -> Option<u8> {
                match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    b'A'..=b'F' => Some(b - b'A' + 10),
                    _ => None,
                }
            };
            if i + 2 < bytes.len() {
                if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    out.push(hi * 16 + lo);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 拆路径末段（"/a/b" → ("/a", "b")）。
fn split_last(path: &str) -> dav_server::fs::FsResult<(String, String)> {
    match path.trim_end_matches('/').rsplit_once('/') {
        Some((p, n)) if !n.is_empty() => Ok((
            if p.is_empty() {
                "/".to_string()
            } else {
                p.to_string()
            },
            n.to_string(),
        )),
        _ => Err(FsError::GeneralFailure),
    }
}

/// 带 provider 的写句柄（flush 上传）。
struct WriteFileWithProvider {
    meta: EntryMeta,
    spool: PathBuf,
    file: Arc<std::sync::Mutex<Option<std::fs::File>>>,
    written: u64,
    provider: Arc<dyn Provider>,
}

impl std::fmt::Debug for WriteFileWithProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteFileWithProvider")
            .field("written", &self.written)
            .finish()
    }
}

impl DavFile for WriteFileWithProvider {
    fn metadata<'a>(&'a mut self) -> FsFuture<'a, Box<dyn DavMetaData>> {
        futures_util::future::ready(Ok(Box::new(self.meta.clone()) as Box<dyn DavMetaData>)).boxed()
    }
    fn write_buf<'a>(&'a mut self, mut buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'a, ()> {
        let bytes = buf.copy_to_bytes(buf.remaining());
        self.write_bytes(bytes)
    }
    fn write_bytes<'a>(&'a mut self, buf: bytes::Bytes) -> FsFuture<'a, ()> {
        let file = self.file.clone();
        async move {
            let b = buf.to_vec();
            tokio::task::spawn_blocking(move || {
                let mut guard = file.lock().map_err(|_| FsError::GeneralFailure)?;
                let f = guard.as_mut().ok_or(FsError::GeneralFailure)?;
                use std::io::Write as _;
                f.write_all(&b)
            })
            .await
            .map_err(|_| FsError::GeneralFailure)?
            .map_err(|_| FsError::GeneralFailure)?;
            Ok(())
        }
        .boxed()
    }
    fn read_bytes<'a>(&'a mut self, _count: usize) -> FsFuture<'a, bytes::Bytes> {
        futures_util::future::ready(Err(FsError::NotImplemented)).boxed()
    }
    fn seek<'a>(&'a mut self, _pos: std::io::SeekFrom) -> FsFuture<'a, u64> {
        futures_util::future::ready(Err(FsError::NotImplemented)).boxed()
    }
    fn flush<'a>(&'a mut self) -> FsFuture<'a, ()> {
        // flush = PUT 完成：上传 spool 文件（取出文件句柄关闭）
        let spool = self.spool.clone();
        let provider = self.provider.clone();
        let parent = self.meta.entry.parent_id.clone();
        let name = self.meta.entry.name.clone();
        let file = self.file.clone();
        async move {
            {
                let guard = file.lock().map_err(|_| FsError::GeneralFailure)?;
                if let Some(mut f) = guard.as_ref() {
                    let _ = f.flush();
                }
            }
            let size = std::fs::metadata(&spool).map(|m| m.len()).unwrap_or(0);
            let spool_for_upload = spool.clone();
            let r = tokio::task::spawn_blocking(move || {
                provider.upload(&parent, &name, UploadSource::Path(&spool_for_upload), size)
            })
            .await
            .map_err(|_| FsError::GeneralFailure)?
            .map_err(map_err);
            let _ = std::fs::remove_file(&spool);
            r.map(|_| ())
        }
        .boxed()
    }
}

/// TLS 配置（证书与私钥 PEM 路径）。
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

/// 启动 WebDAV + web-ui 静态托管服务（阻塞直到 Ctrl-C）。
///
/// - `/dav/*` → WebDAV（Provider 通道）
/// - 其余 → `web_ui_dir` 静态文件（None 则 404 提示）
/// - `auth`：`user:pass` 启用 HTTP Basic（None = 无认证，仅建议本机/内网）
/// - `tls`：Some 则 HTTPS（rustls；证书自备，可 `tailscale cert` 签发）
pub async fn serve(
    provider: Arc<dyn Provider>,
    listen: &str,
    auth: Option<(String, String)>,
    web_ui_dir: Option<PathBuf>,
    tls: Option<TlsConfig>,
) -> anyhow::Result<()> {
    // 网关状态（/api/* 数据源；与 Tauri 壳的 StatusSnapshot 同构）
    let gateway = Arc::new(GatewayState::new(provider.clone()));
    let addr: std::net::SocketAddr = listen
        .parse()
        .map_err(|e| anyhow::anyhow!("监听地址不合法 {listen}: {e}"))?;

    let dav = DavHandler::builder()
        .filesystem(Box::new(ProviderFs::new(provider)))
        .locksystem(dav_server::fakels::FakeLs::new())
        .strip_prefix("/dav")
        .build_handler();

    let dav = Arc::new(dav);
    let ui_dir = web_ui_dir.map(Arc::new);
    let scheme = if tls.is_some() { "https" } else { "http" };
    tracing::info!(%listen, scheme, "边缘网关启动：/dav/ = WebDAV 出口");

    // 请求处理服务工厂（TLS 与明文两条路径共用）
    let make_svc = move || {
        let dav = dav.clone();
        let ui_dir = ui_dir.clone();
        let auth = auth.clone();
        let gateway = gateway.clone();
        hyper::service::service_fn(move |req: hyper::Request<hyper::Body>| {
            let dav = dav.clone();
            let ui_dir = ui_dir.clone();
            let auth = auth.clone();
            let gateway = gateway.clone();
            async move {
                Ok::<_, std::convert::Infallible>(
                    handle_request(req, dav, ui_dir, auth, gateway).await,
                )
            }
        })
    };

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| anyhow::anyhow!("绑定 {addr} 失败: {e}"))?;

    match tls {
        None => {
            // 明文：逐连接服务
            loop {
                let (stream, _) = listener.accept().await?;
                let svc = make_svc();
                tokio::spawn(async move {
                    let _ = hyper::server::conn::Http::new()
                        .serve_connection(stream, svc)
                        .await;
                });
            }
        }
        Some(tls) => {
            // TLS：rustls 加载 PEM 后逐连接 accept
            let acceptor = load_tls_acceptor(&tls.cert_path, &tls.key_path)?;
            loop {
                let (stream, peer) = listener.accept().await?;
                let svc_factory = make_svc();
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let _ = hyper::server::conn::Http::new()
                                .serve_connection(tls_stream, svc_factory)
                                .await;
                        }
                        Err(e) => {
                            tracing::debug!(%peer, error = %e, "TLS 握手失败（客户端直连明文？）");
                        }
                    }
                });
            }
        }
    }
}

/// 加载 PEM 证书/私钥为 rustls acceptor。
fn load_tls_acceptor(
    cert_path: &PathBuf,
    key_path: &PathBuf,
) -> anyhow::Result<tokio_rustls::TlsAcceptor> {
    let certs: Vec<_> = {
        let f = std::fs::File::open(cert_path)
            .map_err(|e| anyhow::anyhow!("打开证书 {}: {e}", cert_path.display()))?;
        rustls_pemfile::certs(&mut std::io::BufReader::new(f))?
            .into_iter()
            .map(rustls::Certificate)
            .collect()
    };
    // rustls-pemfile 1.x 按格式逐个尝试（PKCS8 → RSA → EC）
    let key = {
        let f = std::fs::File::open(key_path)
            .map_err(|e| anyhow::anyhow!("打开私钥 {}: {e}", key_path.display()))?;
        let pkcs8 = rustls_pemfile::pkcs8_private_keys(&mut std::io::BufReader::new(f))?;
        if !pkcs8.is_empty() {
            rustls::PrivateKey(pkcs8[0].clone())
        } else {
            let f = std::fs::File::open(key_path)
                .map_err(|e| anyhow::anyhow!("打开私钥 {}: {e}", key_path.display()))?;
            let rsa = rustls_pemfile::rsa_private_keys(&mut std::io::BufReader::new(f))?;
            if !rsa.is_empty() {
                rustls::PrivateKey(rsa[0].clone())
            } else {
                let f = std::fs::File::open(key_path)
                    .map_err(|e| anyhow::anyhow!("打开私钥 {}: {e}", key_path.display()))?;
                let ec = rustls_pemfile::ec_private_keys(&mut std::io::BufReader::new(f))?;
                let ec = ec.into_iter().next().ok_or_else(|| {
                    anyhow::anyhow!("私钥文件无可用密钥（支持 PKCS8/RSA/EC PEM）")
                })?;
                rustls::PrivateKey(ec)
            }
        }
    };
    let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| anyhow::anyhow!("TLS 配置非法: {e}"))?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

/// 单请求路由：/dav/* → DavHandler；其余 → 静态 web-ui；认证前置。
async fn handle_request(
    req: hyper::Request<hyper::Body>,
    dav: Arc<DavHandler>,
    ui_dir: Option<Arc<PathBuf>>,
    auth: Option<(String, String)>,
    gateway: Arc<GatewayState>,
) -> hyper::Response<hyper::Body> {
    // Basic 认证
    if let Some((user, pass)) = &auth {
        let ok = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Basic "))
            .and_then(|b64| {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .ok()
                    .and_then(|raw| String::from_utf8(raw).ok())
            })
            .map(|decoded| decoded == format!("{user}:{pass}"))
            .unwrap_or(false);
        if !ok {
            return hyper::Response::builder()
                .status(401)
                .header("WWW-Authenticate", "Basic realm=\"CloudMount\"")
                .body(hyper::Body::from("Unauthorized"))
                .unwrap();
        }
    }

    let path = req.uri().path().to_string();
    // /api/*：web-ui 的 REST 面（与 Tauri 命令同名）
    if path == "/api" || path.starts_with("/api/") {
        return handle_api(req, gateway).await;
    }
    if path == "/dav" || path.starts_with("/dav/") {
        let resp = dav.handle(req).await;
        let (parts, body) = resp.into_parts();
        let hyper_body = hyper::Body::wrap_stream(body);
        return hyper::Response::from_parts(parts, hyper_body);
    }
    // web-ui 静态托管
    match ui_dir {
        Some(dir) => serve_static(dir, &path).await,
        None => hyper::Response::builder()
            .status(404)
            .body(hyper::Body::from(
                "CloudMount 边缘网关。WebDAV 出口: /dav/（web-ui 未配置）",
            ))
            .unwrap(),
    }
}

/// 网关状态（/api/* 数据源）。
///
/// 与 Tauri 壳的 AppState 同构：provider 直查（quota）、暂停标志、
/// 日志环形缓冲。边缘模式无本地引擎（WebDAV 直通 Provider），
/// 缓存统计返回"不可用"占位（v2 接入边缘缓存后填充）。
struct GatewayState {
    provider: Arc<dyn Provider>,
    paused: std::sync::atomic::AtomicBool,
    logs: Arc<std::sync::Mutex<Vec<String>>>,
}

impl GatewayState {
    fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            paused: std::sync::atomic::AtomicBool::new(false),
            logs: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

/// `/api/*` 处理（与 Tauri 命令同名同参；POST JSON）。
async fn handle_api(
    req: hyper::Request<hyper::Body>,
    gateway: Arc<GatewayState>,
) -> hyper::Response<hyper::Body> {
    use std::sync::atomic::Ordering;
    let path = req.uri().path().trim_start_matches("/api/").to_string();
    let method = req.method().as_str().to_string();
    let json = |v: serde_json::Value| {
        hyper::Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(v.to_string()))
            .unwrap()
    };
    let err = |msg: &str, code: u16| {
        hyper::Response::builder()
            .status(code)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(
                serde_json::json!({ "error": msg }).to_string(),
            ))
            .unwrap()
    };

    // 读请求体（set_sync_root 的 {path}）
    let body = hyper::body::to_bytes(req.into_body())
        .await
        .unwrap_or_default();
    let _args: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);

    match (method.as_str(), path.as_str()) {
        ("POST", "get_status") => json(serde_json::json!({
            "version": cloudmount_core::VERSION,
            "loggedIn": true,
            "syncing": true,
            "paused": gateway.paused.load(Ordering::SeqCst),
            "pendingUploads": 0,
            "pendingDownloads": 0,
            "conflicts": 0,
            "errors": 0,
            "lastMessage": "边缘网关模式（WebDAV 直通）",
            "totalUploaded": 0,
            "totalDownloads": 0,
        })),
        ("POST", "get_cache_stats") => {
            // 边缘模式无本地内容缓存（WebDAV 直通）——占位
            err("边缘网关模式无本地缓存统计", 501)
        }
        ("POST", "clear_cache") => err("边缘网关模式无本地缓存", 501),
        ("POST", "get_quota") => {
            let provider = gateway.provider.clone();
            match tokio::task::spawn_blocking(move || provider.quota()).await {
                Ok(Ok(q)) => json(serde_json::json!({
                    "total": q.total, "used": q.used, "remaining": q.remaining,
                })),
                Ok(Err(e)) => err(&format!("{e}"), 500),
                Err(e) => err(&format!("{e}"), 500),
            }
        }
        ("POST", "toggle_pause") => {
            // 边缘模式暂停 = 拒绝写操作（v2：WebDAV 只读切换）
            let was = gateway.paused.fetch_xor(true, Ordering::SeqCst);
            json(serde_json::json!(!was))
        }
        ("POST", "set_sync_root") => {
            // 边缘模式无本地同步根（WebDAV 直通云端）
            err("边缘网关模式无本地同步根", 501)
        }
        ("POST", "open_sync_folder") => err("边缘网关模式无本地目录", 501),
        ("POST", "export_logs") => {
            let logs = gateway.logs.lock().map(|v| v.clone()).unwrap_or_default();
            json(serde_json::json!(logs))
        }
        _ => err("未知 API", 404),
    }
}

/// 极简静态文件服务（web-ui：index.html + assets/*）。
async fn serve_static(root: Arc<PathBuf>, path: &str) -> hyper::Response<hyper::Body> {
    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    // 防目录穿越
    if rel.contains("..") {
        return hyper::Response::builder()
            .status(403)
            .body(hyper::Body::from("Forbidden"))
            .unwrap();
    }
    let full = root.join(rel);
    let mime = match full.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    };
    match tokio::fs::read(&full).await {
        Ok(data) => hyper::Response::builder()
            .status(200)
            .header("Content-Type", mime)
            .body(hyper::Body::from(data))
            .unwrap(),
        Err(_) => hyper::Response::builder()
            .status(404)
            .body(hyper::Body::from("Not Found"))
            .unwrap(),
    }
}
