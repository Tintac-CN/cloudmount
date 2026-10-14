# 审计发现归档：audit-p0p1-fixes

> 归档人：编排者　日期：2026-10-14
> 来源：双代理审计 —— `aidlc:reviewer-architect`（架构/安全视角）+ `aidlc:reviewer-dev`（实现质量视角），审计对象为 D:\OneDrive\Devops\cloudmount @ commit 920300e 前后（v0.1.0 后）。
> 编号说明：两位审计者各自编号有重叠（如 serve_static 遍历两方均报），此处去重后统一编为 F-01…F-14，并保留原始编号溯源。

## P0

### F-01（P0）CfAPI 水合全量读入内存
- 位置：`crates/cloudmount-vfs/src/backends/cfapi_mount.rs:113-139`（水合闭包 `read_to_end` 整个范围进 Vec）
- 关联：`crates/cloudmount-vfs/src/backends/cfapi.rs:729-756`（FETCH_DATA 使用完整 RequiredLength）；`:315`（CF_HYDRATION_POLICY_FULL —— 任意字节读触发整文件水合）
- 影响：读取 4GB 视频的任意字节 → 分配 4GB 内存；大文件直接 OOM。
- 修复方向：TRANSFER_DATA 分块传输（有界缓冲，如 1-4MB 块循环），配合 Provider `open_range` 流式读取。
- 溯源：dev-P0-1

## P1（代码缺陷，12 项）

### F-02（P1）ProviderRouter 未转发 change_probe
- 位置：`crates/cloudmount-driver115/src/router.rs:162-212`（`impl Provider for ProviderRouter` 缺 `change_probe`，走 trait 默认实现）
- 影响：`--channel auto`（默认）丢失快速路径，每次同步周期回退全量列表；100 目录 @1r/s → 每周期 100+ 秒。
- 修复方向：impl 内转发 `change_probe` 到活跃通道。
- 溯源：dev-P1-1

### F-03（P1）WebDAV serve_static 路径遍历（Windows 盘符逃逸）
- 位置：`crates/cloudmount-daemon/src/webdav.rs:922-932`
- 影响：仅 `rel.contains("..")` 检查；Windows 上 `/C:/foo` 这类绝对路径经 `root.join` 直接逃逸根目录，可读任意盘文件。
- 修复方向：成分级校验（split 后逐段拒绝空段/盘符/`..`），或 canonicalize 后要求 starts_with(root)。
- 溯源：dev-P1-3 ＝ arch-P1-1（双代理一致，安全级）

### F-04（P1）cookie.rs 调试日志泄漏凭据
- 位置：`crates/cloudmount-driver115/src/cookie.rs:401`（uploadinfo 原始响应）、`:492`（表单 debug，含 userkey）、`:560`（gettoken 原始响应，含 STS）
- 影响：tracing 开 debug 级即把 userkey / OSS STS 凭据写入日志产物。
- 修复方向：删除或脱敏（只记字段名与长度/前缀哈希）。
- 溯源：arch-P1-2

### F-05（P1）daemon 硬编码绕过 Cookie 通道开关
- 位置：`crates/cloudmount-daemon/src/main.rs:376-379`
- 影响：Cookie 通道"默认关闭、需知情同意"的产品边界在 daemon 组装处被硬编码绕过。
- 修复方向：回接配置开关与同意标志校验。
- 溯源：arch-P1-3

### F-06（P1）RdcWatcher Drop 空操作
- 位置：`crates/cloudmount-sync/src/rdc.rs:141-147`（`fn drop(&mut self) { drop(self.handle.clone()) }` —— 克隆后立刻 drop 副本，引用计数净变化为零）
- 影响：每次引擎重建泄漏一个线程 + 目录句柄。
- 修复方向：真正 join/取消线程并关闭目录句柄（信号 + join，或停止标志）。
- 溯源：arch-P1-4

### F-07（P1）ensure_remote_dir 持锁做网络 IO
- 位置：`crates/cloudmount-sync/src/exec.rs:454-499`
- 影响：目录创建期间整把引擎锁被网络往返占住，其它事件全部排队；弱网下放大为全局卡顿。
- 修复方向：锁内只做计划/去重，网络 IO 移出锁外或 per-entry 锁。
- 溯源：arch-P1-5

### F-08（P1）WebDAV spool flush 泄漏（Windows）
- 位置：`crates/cloudmount-daemon/src/webdav.rs:596-617`
- 影响：flush 用 `guard.as_ref()` 而非 `take()`，Drop 时二次 flush/泄漏；Windows 上文件仍被引用时 `remove_file` 因共享冲突失败 → spool 文件残留堆积。
- 修复方向：guard 内 `take()` 所有权；关闭句柄后再删，失败降级记录并重试。
- 溯源：arch-P1-6

### F-09（P1）WebDAV 写打开错误混淆
- 位置：`crates/cloudmount-daemon/src/webdav.rs:292-309`
- 影响：`Err(_)` 盲目把所有打开错误当"新文件"处理 —— 权限错误/IO 错误被掩盖成静默新建，可能造成数据误写。
- 修复方向：区分 NotFound 与其它错误，仅 NotFound 走新建路径。
- 溯源：arch-P1-7

### F-10（P1）OpenApiAuth 非单例刷新
- 位置：`crates/cloudmount-auth/src/open.rs:163-193`
- 影响：access_token 刷新在拿锁后、网络调用前把锁丢掉；并发线程各自刷新 → 115 侧 40140117（旧 token 立即失效互踩）。
- 修复方向：single-flight（持锁跨刷新 + 双检，或全局 OnceCell/Mutex 串行化刷新并共享结果）。
- 溯源：arch-P1-8

### F-11（P1）oss_put 吞 HTTP 状态
- 位置：`crates/cloudmount-driver115/src/cookie.rs:619-635`
- 影响：OSS PUT 非 2xx 时不报错，上层把失败上传当成功 → 数据丢失风险。
- 修复方向：非 2xx 返回 Err（带状态码与响应体摘要），并入重试队列。
- 溯源：arch-P1-9

### F-12（P1）SCM 服务路径忽略 placeholder_mount；service.rs 死代码
- 位置：`crates/cloudmount-daemon/src/main.rs:193-220`（SCM 运行路径从不检查 `cli.placeholder_mount`）；`crates/cloudmount-daemon/src/service.rs`（无调用方）
- 影响：以 Windows 服务方式安装后占位符挂载永不生效，行为与前台运行不一致。
- 修复方向：SCM 路径同样走 placeholder_mount 分支；service.rs 要么接线要么删除。
- 溯源：dev-P1-2

### F-13（P1）daemon 配置加载失败静默回退
- 位置：`crates/cloudmount-daemon/src/main.rs:162-165`
- 影响：config.toml 解析失败时无告警、静默用默认值 —— 用户以为配置生效实际没有。
- 修复方向：加载失败 fail-fast（或显式告警 + 明确日志 + 退出码）。
- 溯源：dev-P1-4

## P1（测试缺口，1 项）

### F-14（P1）Cookie 通道零 HTTP 级测试
- 位置：`crates/cloudmount-driver115/src/cookie.rs`（全文件，1127 行）
- 影响：m115/ec115/OSS 直传全链路无 HTTP 级回归保护；本次 F-04/F-11 的修复没有安全网。
- 修复方向：mock HTTP（本地测试服务器或可注入 transport）覆盖 uploadinfo/gettoken/oss_put/initupload 错误路径；随 F-04/F-11 一并补齐。
- 溯源：dev-P1-5

## P2（延后，不在本 intent 范围）

两位审计者合计报告约 26-37 项 P2（编号有重叠，待后续 intent 重新清点）。已知名样例（摘要，凭编排者记录，非全文）：
- webdav `/api/*` 边缘模式端点返回硬编码伪造数据（webdav.rs:875-916）
- Basic 认证非常数时间比较（webdav.rs:788）
- cookie `init_upload` 每次调用新建 `ureq::Agent`（cookie.rs:506-513）
- cookie 通道 0.5 r/s 限速器设置
- verify_upload sha1-OR-name 回退校验过宽

> 完整 P2 清单以审计轮原始回报为准；后续 intent 立项时重新逐条核对源码（行号可能随本 intent 修复漂移）。
