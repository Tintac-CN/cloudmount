# 115 云同步盘 · 探索与架构设计

> 代号：**CloudMount（云挂）** — 把 115 网盘变成 Windows 上的"类 OneDrive"同步盘，支持边缘设备挂载，含 Cookie 补充登录通道；加密资料库为可选后续模块。
> 文档版本：v0.2（Rust 后端定稿 + Cookie 通道 + 加密降级） ｜ 日期：2026-10-12

---

## 0. 结论摘要（TL;DR）

1. **可行，且有一条清晰的差异化路线。** 115 官方开放平台 OpenAPI 已能覆盖同步盘所需的全部能力（列目录、增删改、秒传/断点上传、Range 下载、配额、回收站、离线下载），个人开发者可申请。**走官方 API 而非 Cookie 逆向是主路线**，也是相对 CD2/OpenList 的稳定性护城河。
2. **Cookie 登录作为补充通道，明确边界。** 调研确认 Cookie 通道完整可用（扫码即得 Cookie），但**明确违反 115 用户服务协议**（3.2.3/4.2/5.4 条），且受多端互踢与风控制约。设计为：**默认关闭、用户显式开启、开启前知情同意、只读探测、独立凭据容器、可一键回退**。仅覆盖官方 API 挂掉时的兜底场景。
3. **后端采用 Rust，极致省资源。** 目标空闲 RSS **5–15MB**（Go 同类 15–40MB），二进制 1–3MB，ARM musl 静态交叉编译。技术栈：**tokio `current_thread` runtime + ureq/reqwest + rustls + rusqlite(bundled)**；边缘 FUSE 用 `fuser`，WebDAV 出口用 `dav-server-rs`（SMB 出口放弃，Rust 无成熟 SMB server）。
4. **Windows 采用"C# 外壳 + Rust 核心"混合架构。** 调研结论很明确：Rust 可以经 `windows` crate 的 `Win32_Storage_CloudFilters` 直连 CldApi.dll 做 CfAPI，但**真正缺的生态是 Shell/COM 集成（导航栏节点、右键菜单、缩略图、MSIX 打包）——那是 C#/.NET 的主场**。因此：外壳 C#（CfAPI + Shell + MSIX），核心 Rust（写成 cdylib，P/Invoke 调用）。这样拿到两边各自的最优。**Go 在此架构中没有位置**。
5. **重要修正：CfAPI 不强制 MSIX。** 微软文档对 `CfRegisterSyncRoot` 只要求 **NTFS + Win10 1709+**；MSIX/Desktop Bridge 仅在需要"资源管理器导航栏品牌节点 + 完整 Shell 集成"时才是产品级建议。这降低了 Windows 版的工程门槛。
6. **加密资料库降级为可选模块（v2）。** 调研发现"本地 TOTP 保护静态数据"是 security theater，真正方案需要密钥份额服务。既然加密需求放缓，先不做；设计已预留接口，后续可作为独立模块接入（见 §6.5 附录）。

---

## 1. 需求解读

| 你的原话 | 解读 | 设计含义 |
|---|---|---|
| "像 OneDrive 挂在 Windows，作为数据同步盘" | 要的是**双向同步 + 盘符体验**，不是只读挂载 | 本地缓存、冲突处理、按需占位 |
| "方便同步资料" | 单人/小团队文档同步为主 | 不做多人协作/共享权限 |
| "Cookie 模仿登录作为补充" | 官方 API 不可用时能兜底 | Cookie 通道：默认关、显式开、有边界（§6.1.3） |
| "Rust 后端极致节约资源" | 常驻进程必须轻 | 目标 5–15MB 空闲 RSS；tokio current_thread；musl 静态链接 |
| "效率要求很高" | 性能敏感、不卡资源管理器 | 占位符 + USN 增量 + 并发分块传输 |
| "支持边缘设备挂载" | ARM Linux/NAS/小主机 | Rust 核心交叉编译 + WebDAV/FUSE 出口 |
| "类似 CD2 但没这么复杂" | 不要多云、ACL、完整文件系统语义 | 砍掉自研内核驱动与多云聚合 |
| "比 OpenList 易用" | 一体化、开箱即用 | 单安装包、扫码登录、零配置 |

**一句话定位**：一个专注、轻量、零配置的 115 同步盘 —— OneDrive 的体验 + Rust 核心的极致轻量 + 边缘节点（NAS/树莓派）也能跑。

---

## 2. 调研关键结论

### 2.1 115 官方 OpenAPI（主路线，可行性：高）

- **接入**：个人开发者可申请，实名 → 入驻审核（约 7 工作日）→ 创建应用（再 7 工作日）→ 发放 AppID/AppKey/AppSecret。无后端客户端走 **PKCE，无需 AppSecret**。
- **认证**：`passportapi.115.com/open/authDeviceCode` 生成二维码 → 115 App 扫码 → `deviceCodeToToken` 换 `access_token`(约 2h) + `refresh_token`(1 年)。**refresh 返回新值且不延期，"频繁刷新会被频控"** —— 需持久化 + 节流。注意官方 OpenAPI 也有"同一 AppID 并发登录数"限制（新登录踢旧）。
- **文件端点**（`proapi.115.com`）：`/open/ufile/files`、`/open/ufile/search`、`/open/folder/add`、`/open/ufile/update`、`/open/ufile/move`、`/open/ufile/copy`、`/open/ufile/delete`、`/open/ufile/downurl`、`/open/ufile/rb/*`、`/open/user/info`。
- **上传**：`/open/upload/init`（`fileid=SHA1`、`preid=前128K SHA1` 触发秒传）→ 命中直接完成；未命中 `/open/upload/get_token` 拿阿里云 OSS STS 直传；`/open/upload/resume` 断点续传。**秒传是效率关键**。
- **下载**：`/open/ufile/downurl` 拿直链，支持 HTTP Range 并发分块，直链需缓存 + 过期刷新。
- **增量**：**无变更事件/cursor 接口**，只能轮询 `usertime` + 全量扫描兜底。
- **频控**：细则不公开，社区经验默认 **≤1 r/s**，超限限流/冻结。

### 2.2 Cookie 登录通道（补充路线）

- **认证**：关键 Cookie 四个 —— `UID`（用户 ID）、`CID`（设备 ID）、`SEID`（会话凭证，核心）、`KID`（签名），域 `.115.com`，随 `Cookie` 头发送，`Set-Cookie` 自动续期。
- **扫码流程**（实测端点）：`GET qrcodeapi.115.com/api/1.0/web/1.0/token/` 取 `uid/time/sign` → 轮询 `qrcodeapi.115.com/get/status/`（0 待扫 / 1 已扫 / 2 已确认 / -1 过期 / -2 取消）→ `POST passportapi.115.com/app/1.0/web/1.0/login/qrcode` 换 Cookie。
- **接口能力**：Cookie/web 接口比官方 OpenAPI **更广** —— 文件全操作、OSS 秒传/分片上传、下载直链、离线下载、**分享/转存、批量操作、app 私有接口**（p115client 封装约 1600 个方法）。这是 Cookie 通道的真实价值：不止兜底，还能补官方 API 缺的能力。
- **风险（必须如实呈现给用户）**：
  - **条款**：115 用户服务协议 3.2.3(4) 禁止"非授权客户端或非授权接口"，4.2(11)/5.4 同类；违规可冻结账号/限制数据访问。
  - **多端互踢**：登录会挤掉现有会话（alist 文档明确警告）；旧版桌面客户端 Cookie 已失效。
  - **风控**：请求带 `sign` 签名、ECDH-AES 加密、`device_id`、UA 校验；应对只能"像正常客户端"（固定 UA、复用 device_id、限速退避），**不鼓励主动规避**。
  - 公开渠道**无权威封号统计**（[不确定]）。

### 2.3 竞品（差异化空间大）

| 产品 | 挂载技术 | 同步 | 易用性 | 我们的取舍 |
|---|---|---|---|---|
| **CloudDrive2** | 疑似自研驱动（支持 ACL） | 多云聚合、跨云复制、加密FS | 6.5/10 | 借鉴 WebDAV 出口；**砍掉**多云与 ACL |
| **OpenList** | 本体无盘符，靠 WebDAV + 外部 rclone/WinFsp | 无 | 4.5/10 | **反面教材**：不搞多工具串联 |
| **rclone** | WinFsp | 单向为主；115 无官方 backend | 3.5/10 | 借鉴 VFS 分层缓存 |

**空白点**：占位符 + 按需 + 资源管理器深度集成，CD2/rclone/OpenList 都做不到。

### 2.4 Windows 挂载技术（选型关键）

- **Cloud Files API (CfAPI)**：Win10 1709+，`cldflt.sys` minifilter。占位符、水合、pin/unpin、**资源管理器原生状态图标与右键项**。OneDrive Files On-Demand 即基于它。
  - **仅 NTFS**；`CfRegisterSyncRoot` **不要求 MSIX**（文档 Requirements 只有 NTFS + 版本）；MSIX 仅在做导航栏品牌节点/完整 Shell 集成时是产品级建议。
  - 核心调用：`CfRegisterSyncRoot` / `CfConnectSyncRoot` / `CfCreatePlaceholders` / `CfHydratePlaceholder` / `CfDehydratePlaceholder` / `CfSetPinState` / `CfSetInSyncState`。
  - **Rust 路径**：`windows` crate feature `Win32_Storage_CloudFilters` 已含全部 `Cf*` 绑定（微软官方生成，100% 覆盖）；`ok-nick/wincs` 是孤本薄封装（31★，示例全是 TODO，不建议依赖）。**CfAPI 本身 Rust 可行**。
  - **真正的坑**：Shell/COM 集成（导航栏节点、右键菜单、缩略图提供者、MSIX 打包）Rust 生态近乎空白；`cargo-msix` 停更。→ 这部分归 C#。
- **WinFsp**：用户态文件系统。**没有占位符/状态列**。Rust 绑定 `winfsp-rs` v0.13（活跃，69★），覆盖全回调，**但 GPLv3 许可**（传染，商业分发需购买 WinFsp 商业授权或开源）。Go 的 `cgofuse` 同理 GPLv3。
- **Dokany**：维护慢，不推荐。
- **ProjFS**：无在线/离线状态与回收，远程存储微软明确让用 CfAPI。

### 2.5 Rust 低资源技术栈（调研定稿）

| 关注点 | 结论 |
|---|---|
| **async 运行时** | **tokio `current_thread`**：比多线程省约 8MB（HN 实测，≈50%）且性能无损。`async-std` 已废弃转 smol（趋冷），不用 |
| **HTTP 客户端** | 省内存排序：`ureq`（同步，依赖最浅）< hyper < reqwest（最重）。核心传输层用 **ureq + rustls**，可砍掉 tokio 整棵子树；边缘 WebDAV 服务用 axum 时保留 tokio |
| **TLS** | **rustls 0.23** + `rustls-platform-verifier`（Windows 用系统根证书）：纯 Rust、静态链接、ARM 友好。openssl 仅 FIPS 必需时用 |
| **数据库** | **rusqlite 0.40 + `bundled`**：内嵌 SQLite 3.53，cc 交叉编译无障碍，免外部依赖 |
| **Linux FUSE** | **`fuser` 0.18**：纯 Rust 实现，关 `libfuse` feature 去 C 依赖，利于 musl 静态交叉编译。注意其治理变化（0.18 后由 coding agent 维护、不收 PR）——锁定版本 |
| **WebDAV 出口** | **`dav-server-rs` 0.11**（成熟，243★）。**SMB 出口放弃**：Rust 无成熟 SMB server，边缘需要 SMB 时建议直接调系统 smbd |
| **ARM 交叉编译** | `aarch64/armv7-unknown-linux-musl` + rustls + rusqlite(bundled)，全程静态、无 C 依赖地狱 |
| **体积优化** | `opt-level="z"` + `lto="fat"` + `codegen-units=1"` + `strip` + `panic="abort"`：10MB → 1–3MB |
| **内存预期** | tokio current_thread + ureq + rusqlite + panic=abort → 空闲 RSS **5–15MB**（[推断，无权威基准]，Go 同类 15–40MB） |

### 2.6 加密 / 2FA（已降级，设计存档）

- 若将来做：参考 Cryptomator 格式（scrypt KEK + AES Key Wrap 主密钥、AES-SIV 文件名、AES-GCM 分块、`.c9s` 长名缩短）。
- **必须诚实**：本地 TOTP 对静态数据无效（security theater）。真 2FA 需服务端密钥份额（可自托管在边缘节点）或 WebAuthn PRF / YubiKey hmac-secret。
- 存档于 §6.5，当前版本不做。

---

## 3. 产品定位与差异化

**三条护城河**：
1. **官方 API 主路线 + Cookie 兜底** —— 稳定性优先，能力上比纯官方 API 产品多一层冗余。
2. **真 OneDrive 体验** —— CfAPI 占位符 + 状态图标 + 按需下载，竞品做不到。
3. **Rust 极致轻量** —— 空闲 5–15MB vs CD2/Go 系的几十 MB；同一二进制跑边缘设备。

**明确不做**：多云聚合、POSIX ACL/自研内核驱动、多人协作、SMB 服务器（Rust 不成熟）、移动端 App（用边缘 WebDAV 兼容现有客户端）。

---

## 4. 整体架构

### 4.1 分层

```
┌─────────────────────────────────────────────────────────────┐
│                      接入层 (Access)                          │
│  Windows 外壳 C#(CfAPI/Shell/MSIX) │ WebDAV/FUSE 出口 │ Tray  │
├─────────────────────────────────────────────────────────────┤
│                    挂载/投影层 (VFS)                          │
│  占位符管理 · 按需水合 · pin状态 · 路径映射 · 属性缓存         │
├─────────────────────────────────────────────────────────────┤
│                   同步引擎 (Sync Engine)        [Rust]        │
│  本地变更监控(USN) · 远端轮询(usertime) · 冲突解决 · 队列调度  │
├─────────────────────────────────────────────────────────────┤
│     认证/会话层        ┆          传输层 (Transfer) [Rust]    │
│  OpenAPI OAuth(PKCE)  ┆  秒传 · 分片 · Range · 直链缓存 ·限速 │
│  Cookie 通道(补充)    ┆                                      │
├─────────────────────────────────────────────────────────────┤
│                  存储抽象层 (Provider)           [Rust]       │
│      115 Open Driver  │  115 Cookie Driver  │  Local FS      │
├─────────────────────────────────────────────────────────────┤
│              本地缓存 (Cache Store)              [Rust]       │
│      内容块缓存(LRU) · 元数据SQLite(WAL) · 状态持久化          │
└─────────────────────────────────────────────────────────────┘
         ┌──────────────────────────────────────────┐
         │  边缘节点 (Edge Node) — 可选、自托管 [Rust] │
         │  WebDAV 服务 · 可选 FUSE 挂载 · Tailscale  │
         └──────────────────────────────────────────┘
```

### 4.2 进程模型（Windows）

```
[CloudMount.Shell.exe]  C#/.NET  ── CfAPI 回调、Shell 集成、托盘 UI、MSIX 打包
        │  P/Invoke → cloudmount_core.dll (cdylib, Rust)
        ▼
[cloudmount_core.dll + cloudmountd.exe]  Rust ── 同步引擎/115驱动/缓存/传输
        │  ureq/rustls → HTTPS
        ▼
    115 OpenAPI  │  115 Cookie/web API（补充通道，默认关）
```

- **Rust 核心导出 C ABI**（`extern "C"` + `#[repr(C)]` 结构），C# 经 P/Invoke 调用；回调用函数指针 + 用户上下文。不用 gRPC——同一进程内 P/Invoke 开销更低、部署更简单；gRPC 留给"边缘网关远程核心"场景。
- 托盘/状态页由 C# 外壳承载（WinUI/WPF）。
- 边缘节点：`cloudmountd`（同一 Rust 核心）以服务模式跑 WebDAV + 可选 FUSE。

### 4.3 关键数据流

**读取（按需水合）**：资源管理器打开 → CfAPI `FETCH_DATA` 回调 → C# 调 Rust `vfs_read(path, offset, len)` → 查块缓存，未命中则 `downurl` + Range 拉取 → 回填 → `CfSetInSyncState`。
**写入**：保存 → 回调 → Rust 写本地缓存立即返回 → 入上传队列 → `init` 秒传判定 → 命中即完成 / 未命中 OSS 分片直传。
**本地变更**：**USN Journal** 增量捕获。**远端变更**：`usertime` 轮询（30–60s + 退避）。

---

## 5. 关键技术选型（决策表）

| 维度 | 方案 | 理由 / 备注 |
|---|---|---|
| **核心语言** | **Rust** | 空闲 5–15MB、1–3MB 二进制、musl 静态 ARM；无 GC 停顿 |
| **Windows 外壳** | **C#/.NET + CfAPI** | Shell/COM/MSIX 是 C# 主场；CfAPI 官方示例即 C# |
| **核心↔外壳** | **P/Invoke（cdylib + C ABI）** | 同进程零部署；gRPC 仅留作边缘远程模式 |
| **Windows 挂载** | **CfAPI（cldflt）** 为主；WinFsp-rs 仅作非 NTFS 降级 | CfAPI 无 MSIX 硬要求；WinFsp-rs 注意 **GPLv3** |
| **同步发现** | USN（本地）+ usertime 轮询（远端） | 115 无事件接口 |
| **元数据** | SQLite WAL（rusqlite bundled） | 事务、成熟、静态编译 |
| **内容缓存** | 分块文件 + LRU 索引，限额可视化 | 直击 CD2 占盘不透明痛点 |
| **HTTP/TLS** | ureq（核心）/ axum（边缘 WebDAV）+ rustls + platform-verifier | 依赖最浅、ARM 友好 |
| **async** | tokio `current_thread` | 省 ~8MB；禁用不必要 feature |
| **边缘出口** | **WebDAV（dav-server-rs）**；FUSE（fuser）可选 | SMB 放弃（Rust 无成熟实现） |
| **边缘远程访问** | Tailscale 集成（可选）/ 文档化 WireGuard | 免公网暴露 |
| **Cookie 通道** | 默认关，显式开启，独立容器，只读探测 | 条款风险与互踢，见 §6.1.3 |
| **加密资料库** | **v2 可选模块**（Cryptomator 式） | 需求放缓，预留接口 |

---

## 6. 模块设计

### 6.1 认证与 Provider 层

**6.1.1 官方 Open Driver（主）**
- PKCE 扫码登录；token 存 Windows 凭据管理器 / DPAPI 加密；refresh **单飞 + 节流**（防频控）。
- 全局令牌桶 **≤1 r/s**（可配），429/频控 → 指数退避 + 熔断。
- 统一 `Provider` trait：`list / stat / open_range / create / update / move / copy / delete / upload / quota`。

**6.1.2 Cookie Driver（补充，默认关）**
- 实现 §2.2 的扫码流程，Cookie 存独立加密容器（与官方 token 物理隔离）。
- 复用同一 `Provider` trait，引擎层无感知切换。
- 能力差异：Cookie 驱动额外暴露分享/转存/批量操作（官方 API 没有的），作为"高级功能"仅在 Cookie 通道开启时可用。

**6.1.3 Cookie 通道的边界设计（重要）**
- **默认关闭**；开启前展示知情同意：违反服务协议 3.2.3(4)/4.2(11)/5.4 条、可能封号/限速/数据访问受限、无稳定性承诺。
- **凭据隔离**：独立 Cookie 容器，不与用户其他 115 会话共享 SEID，降低互踢面。
- **只读探测**：失效检测用 `GET my.115.com/?ct=guide&ac=status`（不踢下线），**禁用** `check/sso` 类会挤掉其他设备的探测。
- **失效即回退**：Cookie 失效自动切回官方 API，提示用户重新扫码；一键彻底关闭并清除凭据。
- **行为约束**：固定 UA、严格限频、退避重试、日志审计；不做任何主动风控规避（不伪造设备指纹绕过校验，只"像正常客户端"）。
- UI 常驻风险提示角标，明确"补充通道"属性。

### 6.2 同步引擎（Rust）

- **状态机**：`LocalOnly / RemoteOnly / InSync / Conflict / Uploading / Downloading / Pending`，持久化 SQLite。
- **冲突解决**：默认保留两者（`name (device-conflict).ext`），可配本地/远端优先。
- **队列**：上传/下载分队列，全局并发 + 每文件分片并发（3–4）；支持暂停/限速。
- **选择性同步**：目录级"始终保留/仅在线/排除"，映射 CfAPI pin state。

### 6.3 本地缓存 / VFS（Rust）

- 元数据全量缓存（SQLite WAL）+ 内容块 LRU 缓存（默认上限 10GB，**托盘可视化 + 一键清理**）。
- 占位符状态持久化（pin state、已水合范围、稀疏文件追踪），重启恢复。
- 缓存命中率/占用在托盘展示。

### 6.4 Windows 挂载与外壳（C# + Rust cdylib）

- **阶段一（快速可用）**：C# 外壳经 WinFsp-rs 挂**本地盘**（全实体文件，无占位符）——仅供开发联调与非 NTFS 降级，注意 GPLv3 对产品分发的约束。
- **阶段二（正式版）**：**CfAPI 占位符模式**。
  - C#：SyncRoot 注册、`CfConnectSyncRoot`、回调调度、Shell 集成（状态图标由平台原生提供；右键"始终保留/释放空间"为平台动词）、托盘与状态页、MSIX 打包（导航栏品牌节点）。
  - Rust cdylib：`vfs_read/vfs_write/vfs_readdir/...` + `hydrate/dehydrate/pin`。
  - Hydration policy：`PROGRESSIVE`（支持随机 seek 的部分水合）。
- 降级检测：安装/启动时检查 NTFS + 系统版本，不满足则提示并切 WinFsp 模式。

### 6.5 附录：加密资料库（v2 可选，设计存档）

> 当前版本不做。若将来启用：

- **格式**：参考 Cryptomator —— 主密钥（512b）由密码经 scrypt 派生 KEK + AES Key Wrap 包裹；文件名 AES-SIV（确定性，利同步去重）；内容 AES-GCM 32KiB 分块；`dir.c9r` 目录混淆；`.c9s` 长名缩短（阈值 220）。
- **必须诚实**：本地 TOTP 是 security theater（种子在本地，攻击者可绕过）。真 2FA 方案：
  - **A（推荐）**：服务端持有第二密钥份额，验证 TOTP（微软 Authenticator 扫码的标准 otpauth）后释放 —— 份额服务可自托管在边缘节点；
  - **B**：WebAuthn PRF / YubiKey hmac-secret 注入 KDF（无服务端，需硬件，注册端兼容有限）。
- **同步集成**：加密层在 Provider 之上、同步引擎之下；vault 对引擎是"另一棵目录树"，但冲突/重命名按密文名处理，避免 `file (1).enc` 污染。

### 6.6 边缘网关（Edge Node，Rust）

- `cloudmountd`：同一核心编译 `aarch64/armv7-unknown-linux-musl`，静态链接。
- **出口**：WebDAV（dav-server-rs，含 TLS via rustls）；FUSE 挂载（fuser）可选。
- **低内存预设**：tokio current_thread、buffer 4M、chunk 4M/limit 32M、限并发、systemd `MemoryMax`；目标 512MB 设备可跑。
- **远程访问**：可选 Tailscale 集成（`tailscale serve` 暴露 WebDAV）或文档化 WireGuard。
- 容器化：多架构 Docker 镜像（scratch/dist less，单二进制）。

---

## 7. 路线图

| 阶段 | 目标 | 交付 |
|---|---|---|
| **M0 验证（1–2周）** | 打通官方 API + 申请开发者资质 | Rust CLI：PKCE 登录、列目录、秒传、Range 下载 |
| **M1 Rust 核心（3–5周）** | 同步引擎 + 缓存 + SQLite 元数据 | 双向同步（CLI/无 UI）；USN+轮询；冲突处理；目标空闲 ≤20MB |
| **M2 Windows 外壳（4–6周）** | WinFsp 联调盘 → CfAPI 占位符 | 阶段一快速可挂；阶段二占位符 + 状态图标 + 托盘；MSIX |
| **M3 Cookie 通道（2–3周）** | 补充登录 + 能力补全 | 扫码登录、知情同意、只读探测、一键回退；分享/转存等高级能力 |
| **M4 边缘网关（2–3周）** | ARM 构建 + WebDAV/FUSE | cloudmountd、低内存预设、Tailscale、Docker 镜像 |
| **M5 打磨** | 安装包、自动更新、缓存可视化 | MSIX 自动更新、缓存一键清理、性能调优 |
| **M6（可选，v2）** | 加密资料库 | §6.5 存档设计，含真 2FA 份额服务 |

> M1 是所有后续的基础。M2 阶段二（CfAPI）是最大不确定项，阶段一先用 WinFsp 保证"能用"。

---

## 8. 风险与开放问题

| 风险 | 影响 | 缓解 |
|---|---|---|
| 开发者资质审核不通过/周期长 | 阻塞 | M0 尽早申请；开发期测试账号 |
| 频控细则不公开 | 同步不稳 | 保守限速 + 退避 + 熔断；本地缓存扛读 |
| 无增量事件接口 | 远端变更 30–60s 延迟 | 接受并文档化；轮询参数可调 |
| **Cookie 通道条款风险** | 封号/限速 | 默认关 + 知情同意 + 只读探测 + 一键回退；只作兜底，不作卖点宣传 |
| Cookie 多端互踢 | 用户其他端被登出 | 独立容器 + 不共享 SEID；提示用户 |
| Rust Shell/MSIX 生态空白 | 外壳开发成本 | 外壳本就用 C#；Rust 只做 cdylib |
| winfsp-rs GPLv3 | 商业分发受限 | 仅作开发联调/降级；正式体验靠 CfAPI；或评估 WinFsp 商业授权 |
| CfAPI 仅 NTFS | 兼容性 | 检测降级；文档明示 |
| fuser 治理变化 | 维护风险 | 锁定版本；必要时 fork 或评估 fuse3 |
| 115 条款变更 | 合规 | 如实申报、遵守频控、不共享账号 |

---

## 9. 待你决策的关键点

1. **M2 顺序**：先 WinFsp 联调盘再迭代 CfAPI（推荐，风险低），还是直接 CfAPI（体验优先）？
2. **Cookie 通道的定位**：仅作"官方 API 挂掉时的兜底"，还是也开放"分享/转存/批量操作"等官方没有的高级能力（增强吸引力但提高条款风险敞口）？→ 建议：兜底为主，高级能力放在二级菜单并常驻风险提示。
3. **边缘网关出口**：WebDAV 够用，还是必须要 FUSE 本地挂盘？（SMB 需借助系统 smbd，Rust 无成熟实现）
4. **产品分发形态**：开源（GPLv3 的 WinFsp 无压力）还是闭源商业（需规避 WinFsp，正式体验全押 CfAPI）？这影响 M2 阶段一的取舍。
5. **加密资料库**：确认 v2 再做？还是接口预留但永远不承诺？

---

## 附录：主要信息来源

- 115 开放平台：https://www.yuque.com/115yun/open ｜ https://open.115.com/
- Cookie 通道：https://github.com/ChenyangGao/p115client ｜ https://github.com/SheltonZhu/115driver ｜ https://alistgo.com/guide/drivers/115.html ｜ 115 用户服务协议 https://115.com
- 微软 CfAPI：https://learn.microsoft.com/windows/win32/cfapi/build-a-cloud-file-sync-engine ｜ ProjFS：https://learn.microsoft.com/windows/win32/projfs/projected-file-system
- windows crate：`Win32_Storage_CloudFilters` feature（docs.rs/crate/windows）｜ wincs：https://github.com/ok-nick/wincs
- WinFsp：https://winfsp.dev/ ｜ winfsp-rs：https://github.com/SnowflakePowered/winfsp-rs
- fuser：https://crates.io/crates/fuser ｜ dav-server-rs：https://github.com/messense/dav-server-rs
- tokio/ureq/rustls/rusqlite：crates.io ｜ Cryptomator 架构：https://docs.cryptomator.org/en/latest/security/architecture/
- CloudDrive2：https://www.clouddrive2.com/ ｜ OpenList：https://github.com/OpenListTeam/OpenList ｜ rclone：https://rclone.org/
