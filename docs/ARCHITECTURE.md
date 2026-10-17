# CloudMount 架构（v0.3）

> 2026-10 定稿。前置探索见 [design/2026-10-12-exploration-v0.2.md](design/2026-10-12-exploration-v0.2.md)。

## 设计原则

1. **一套核心，三端外壳。** 同步引擎/驱动/缓存是平台无关的 Rust；
   挂载与 UI 是各平台薄壳。所有外壳通过 `cloudmount-core` 的同一套抽象说话。
2. **驱动可插拔。** 115 官方 OpenAPI、Cookie 通道（默认关）、本地 FS 都实现
   `Provider` trait；上层无感知。`ProviderRouter` 负责通道降级与回退。
3. **挂载可插拔。** CfAPI / 自研 minifilter / FUSE / FileProvider / WebDAV
   都实现 `MountBackend` trait，按平台与配置选择。
4. **极致轻量是硬约束。** 每个依赖都要过"空闲 RSS 账"：
   tokio current_thread（省 ~8MB）、ureq+rustls（砍 async HTTP 栈）、
   SQLite bundled、零框架前端。目标空闲 5–15MB、二进制 1–3MB。

## 进程与数据流

### Windows（当前规划：M2）

```
[CloudMount.exe]  桌面壳（egui/eframe 纯 Rust：原生 UI + 托盘 + 平台集成）
      │ 链接
      ▼
[cloudmount-core 等 crates]  同步引擎/驱动/缓存（同进程库）
      │            ▲
      │            │ MountBackend trait
      ▼            │
[115 OpenAPI]   [CfAPI (cldflt.sys)]  ← M2；长期替换为 [自研 minifilter]
                     ▲                         ▲
                     │ CF 回调                  │ FilterManager message port
                Windows Shell              [cloudmountd（Rust 引擎服务）]
```

### 边缘设备（M4）

```
[cloudmountd]（musl 静态单二进制）
  ├── WebDAV 出口（dav-server-rs + rustls） ← 局域网/Tailscale 设备挂载
  ├── FUSE 挂载（fuser，可选）              ← 设备本地盘符
  ├── web-ui 静态托管（:8384）              ← 浏览器管理
  └── 同步引擎（usertime 轮询 + inotify）
```

### macOS（M5+）

```
[宿主 App（M5+，技术未定）] ←→ [NSFileProviderReplicatedExtension（Swift 薄桥）]
                                   │ C ABI / staticlib
                              [cloudmount-core（Rust）]
```

## 关键机制

### 变更发现（115 无事件接口的必然设计）

- **本地**：NTFS USN Journal（Windows）/ FSEvents（macOS）/ inotify（Linux）
  —— 增量、高效、不扫描。
- **远端**：`ufile/files` 按 `usertime` 倒序取头部探测 + 周期性全量兜底。
  默认间隔 30–60s，指数退避，接受"远端变更最多延迟 1 分钟"并写进产品文档。

### 传输（效率关键）

- **上传秒传**：`upload/init` 携带 SHA1 + 前 128K SHA1，命中零传输完成；
  未命中走 OSS STS 直传（分片并发 3–4，断点续传 `upload/resume`）。
- **下载**：`downurl` 直链缓存（带过期）+ HTTP Range 并发分块 +
  磁盘块缓存（LRU、限额、托盘可视化）。
- **限速**：全局令牌桶默认 1 r/s（官方频控细则不公开，必须保守），
  429/频控码 → 指数退避 → 熔断。

### 通道降级（ProviderRouter）

```
官方 OpenAPI 连续失败（刷新也失败）
  → Cookie 通道已启用？ ── 是 → 切换，UI 常驻"补充通道"风险提示
                        └─ 否 → 引导用户等待/重新扫码
Cookie 只读探测失败（my.115.com/?ct=guide&ac=status，不踢下线）
  → 自动回退官方通道，提示重新扫码
```

凭据隔离：官方 token 与 Cookie 分开存放（`SecretKind`），Cookie 通道
独立 UA/独立限速（0.5 r/s）。

## web-ui 管理页（边缘单宿主）

唯一宿主 = `cloudmountd --serve`（浏览器 → `POST /api/{cmd}`）。契约在
`web-ui/src/api.ts`，daemon 实现于 `/api/*`，两端**同名同参**。桌面端为
egui 原生 UI（apps/desktop），不使用 web-ui——`platform.ts` 已单宿主化
（`hostKind()` 恒 `"web"`）。前端零框架（vanilla TS），复杂文件列表出现
后再评估 Svelte。

## 自研 Windows minifilter（长期，drivers/win）

调研结论（详见 drivers/win/README.md）：
- **minifilter 而非 FSD**：占位符本质是"NTFS 上的云视图"，Filter Manager
  挂 NTFS 卷 + 自定义 reparse point（GUID tag 需向微软申请）+ 拦截
  CREATE/READ 触发水合，ACL/CC/Mm/oplock 由 NTFS 承担。
- **内核也用 Rust（v1.0 方案更新）**：`windows-drivers-rs` 至今不支持 minifilter，
  但 Rust 可用 `crate-type=["cdylib"]` **直出 `.sys`（无需 C 桩）**，已有真实
  minifilter 先例（apriorit）。缺口在 **FltMgr 绑定**（`wdk-sys` 为零）→ 自建绑定层，
  并保留"薄 C 壳"回退路径。**完整方案见 [../drivers/win/RUST-MINIFILTER-DESIGN.md](../drivers/win/RUST-MINIFILTER-DESIGN.md)。**
- **"更稳定"的四条抓手**：编译期消灭缺陷类（RAII 引用/IRQL 类型/锁序/panic 源穷举）、
  内核逻辑在用户态确定性仿真测试、引擎故障走超时+熔断+明确降级（绝不静默返回 0）、
  崩溃后可对账恢复 + 黑匣子 triage。
- 工作量：到敢发布 **18–30 人月**（验证占一半以上）；作为长期旗舰，不阻塞产品先走 CfAPI。

## 加密资料库（v2 可选，已降级）

设计存档于探索文档 §6.5：Cryptomator 式格式（scrypt+AES-KW 主密钥、
AES-SIV 文件名、AES-GCM 分块、长名缩短）；**本地 TOTP 对静态数据无效**
（security theater），真 2FA 需服务端密钥份额（可自托管在边缘节点）。
当前不实现，`Provider` trait 的分层已为其预留位置（加密层夹在 Provider
与同步引擎之间）。
