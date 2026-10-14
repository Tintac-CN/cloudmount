# PRD: audit-p0p1-fixes

> 产出者：pm　日期：2026-10-14　来源：intent.md + audit-findings.md

## 用户故事

| # | 角色 | 目标 | 价值 | 溯源（intent.md/audit-findings.md） |
|---|---|---|---|---|
| U1 | 桌面挂载用户 | 读取大文件占位符（如 4GB 视频）的任意字节时，水合按有界分块传输，进程不因请求范围大小分配等量内存 | 大文件读取不 OOM，空闲 RSS 5-15MB 目标不回退 | intent.md 问题清单"内存安全"；F-01 |
| U2 | `--channel auto` 同步用户 | change_probe 快速路径经 ProviderRouter 透传到活跃通道 | 同步周期不被全量列表拖慢（100 目录 @1r/s 不再退化为每周期 100+ 秒） | F-02 |
| U3 | WebDAV 边缘模式用户/安全方 | 静态文件服务拒绝一切逃逸文档根的路径（盘符、UNC、`..`、反斜杠变体） | 边缘暴露面不可读任意盘文件 | F-03（双代理一致，安全级） |
| U4 | 全体用户/安全方 | 任何日志级别下 userkey、OSS STS 凭据不落入日志产物 | 日志可安全归档/导出（export_logs 面收窄） | F-04 |
| U5 | Cookie 通道用户 | daemon 不再硬编码绕过"默认关闭、需知情同意"的产品开关 | 产品合规边界在所有组装路径一致生效 | F-05 |
| U6 | Windows 实时同步用户 | 引擎重建时旧 RdcWatcher 的线程与目录句柄被真正回收 | 长时间运行不泄漏线程/句柄 | F-06 |
| U7 | 弱网同步用户 | 远端目录创建不持有引擎全局锁做网络 IO | 单目录创建的网络往返不阻塞其余事件 | F-07 |
| U8 | WebDAV 写入用户 | PUT 完成后 spool 临时文件被清理，一次 PUT 恰好触发一次上传 | Windows 下不残留 spool 堆积、不重复上传 | F-08 |
| U9 | WebDAV 写入用户 | 写打开仅在 NotFound 时走新建路径，权限/IO 错误如实上抛 | 错误不被掩盖成静默新建，避免数据误写 | F-09 |
| U10 | OpenAPI 通道用户 | access_token 刷新单飞：并发调用只触发一次网络刷新并共享结果 | 不触发 115 侧 40140117 互踩失效 | F-10 |
| U11 | Cookie 通道上传用户 | OSS PUT 非 2xx 返回携带状态码的错误，失败上传不被当成功 | 消除静默数据丢失风险 | F-11 |
| U12 | Windows 服务用户 | SCM 服务运行路径与前台路径对 placeholder_mount 行为完全一致 | 服务方式安装后占位符挂载生效，行为可预期 | F-12 |
| U13 | daemon 用户 | 配置文件存在但解析失败时进程拒绝启动并指明原因 | "配置生效"与"实际生效"一致，不静默回退 | F-13 |
| U14 | 开发者/维护者 | cookie.rs 全链路具备 HTTP 级回归测试安全网 | F-04/F-11 修复有保护，后续改动不回退 | F-14 |
| U15 | 项目全体 | 全部修复后全量回归通过、CI 三平台绿 | v0.1.0 后质量基线恢复 | intent.md"期望的结果"与"约束" |

## 验收标准（可测）

- **AC1**（U1，F-01）：Given 一个远端大文件（测试桩可用 ≥8MB 模拟）已投影为占位符且 Provider 桩按 RangeSpec 返回数据，When 通过 CfAPI 触发该文件任意子范围的水合，Then 水合传输路径对 Provider 的单次读取请求均不超过声明的分块上限常量（默认 ≤4MB），返回字节与 Provider 提供内容逐字节一致，且全路径（含 FETCH_DATA 处理）不存在与请求总范围等大的单次连续缓冲分配（以桩记录的请求大小序列断言分块）。
- **AC2**（U2，F-02）：Given `--channel auto` 且活跃通道为记录调用的桩通道，When 经 ProviderRouter（以 trait 对象）调用 `change_probe`，Then 桩通道的 `change_probe` 恰被调用一次且返回值透传；Given 活跃通道对 `change_probe` 返回认证失效错误，Then 走与其它方法一致的通道切换处理。
- **AC3**（U3，F-03）：Given serve_static 以目录 `root` 为文档根，When 请求 `/C:/Windows/win.ini`（盘符绝对路径）、`//server/share/f`（UNC）、`/../../secret`、`/a/b/../../../secret` 及反斜杠变体中的任一项，Then 响应为 403/404 且响应体不含目标文件内容；Given `root` 下存在 `index.html` 与 `assets/app.js`，When 请求 `/`、`/index.html`、`/assets/app.js`，Then 返回 200 且内容正确；且对任一放行请求，最终读取路径的规范形式满足 `canonicalize().starts_with(root)`（可对路径解析函数做单元断言）。
- **AC4**（U4，F-04）：Given tracing 过滤开至 `trace` 级并捕获全部日志输出，且桩服务返回已知敏感值（userkey=`USERKEY-SECRET-123`、STS secret/security_token 为已知桩值），When 依次执行 uploadinfo、initupload、gettoken 全流程，Then 捕获的日志中不出现任一敏感桩值的明文或可还原片段（逐值断言缺席）；日志允许出现字段名、长度或前缀哈希等脱敏元数据。
- **AC5**（U5，F-05）：Given Cookie 通道策略为默认（enabled=false 或 consent_acknowledged=false）且用户以 `--channel cookie` 启动 daemon，When 组装 Provider，Then 进程以非零码退出，错误信息指明需显式开启并确认同意；Given enabled=true 且 consent_acknowledged=true，When 以 `--channel cookie` 启动，Then Cookie 通道可用且保留既有协议风险 warn 日志。
- **AC6**（U6，F-06）：Given 一个运行中的 RdcWatcher，When drop 该实例，Then 后台观察线程在有限超时（如 5s）内退出、目录句柄被关闭；循环"创建+drop"100 次后，进程线程计数相对基线无净增长（OS 级计数或代码内启动/退出插桩计数器，取测试稳定者）。
- **AC7**（U7，F-07）：Given 桩 Provider 的 `list`/`create_dir` 各延迟 500ms，When 目录 X（未缓存）触发 ensure_remote_dir，同时并发地对已缓存目录 Y 查询目录 ID，Then Y 的查询在 X 的网络往返完成前即返回（可观测 <500ms 完成），即 dir_ids 锁持有期间不发生 `provider.list`/`provider.create_dir` 调用（以桩调用与锁状态的时序记录断言）。
- **AC8**（U8，F-08）：Given 一次 WebDAV PUT 走完 flush 且上传成功，When flush 返回并释放文件句柄后，Then temp 目录中该 PUT 的 `cm-dav-put-*` spool 文件不存在；且该 PUT 完整生命周期内 `provider.upload` 恰被调用一次（桩计数断言，排除 Drop 二次 flush 重复上传）；Given 删除 spool 时因 Windows 共享冲突失败，Then 记录 warn 日志且不视为上传失败（延迟清理不阻塞响应）。
- **AC9**（U9，F-09）：Given 目标文件不存在且父目录可解析，When WebDAV 写打开，Then 走合成新条目路径且 PUT 成功；Given resolve 因权限或 IO 错误（非 NotFound）失败，When 写打开，Then 返回相应错误（非静默新建），且 `provider.upload` 未被调用（桩计数为 0）。
- **AC10**（U10，F-10）：Given access_token 已临近过期、可注入的桩 transport 对 refreshToken 端点计数，When 8 个线程并发调用 `access_token()`，Then 网络刷新请求恰好发出 1 次（桩计数为 1）且 8 个调用全部获得同一新 token；Given 桩 transport 对刷新返回失败，When 并发调用，Then 所有调用者收到一致的错误（错误不被吞掉），修复后的下次调用可重试。
- **AC11**（U11，F-11）：Given 桩 OSS 端点对 PUT 返回 403 与错误体，When oss_put 上传，Then 返回 Err 且错误信息包含 HTTP 状态码与响应体摘要，上层上传结果为失败（可进入既有重试处理）；Given 桩端点返回 2xx，Then oss_put 返回 Ok 且既有 callback 逻辑不回退。
- **AC12**（U12，F-12）：Given 以 Windows 服务 SCM Run 方式启动且 placeholder_mount=true，When 服务路径构造同步配置与挂载，Then 与前台运行 placeholder_mount=true 走同一构造分支（占位符挂载生效；以提取公共构造函数后的单元断言两路径产出的配置一致）；Given placeholder_mount=false，Then 两路径同为非占位符行为；且 `crates/cloudmount-daemon/src/service.rs` 不再存在零调用方的死代码（以编译器/clippy dead_code 无告警为准）。
- **AC13**（U13，F-13）：Given 配置文件存在但内容非法（非 TOML），When daemon 启动，Then 进程以非零退出码结束，输出包含"配置解析失败"与文件路径；Given 配置文件不存在，When daemon 启动，Then 以内置默认值正常启动（沿用 `config::load_from` 既有语义，首启不受影响）。
- **AC14**（U14，F-14）：Given 可注入 transport 或本地测试服务器，When 运行 `cargo test -p cloudmount-driver115` 的 cookie 通道 HTTP 级套件，Then 用例矩阵至少覆盖：uploadinfo 成功与 state=false、initupload 成功与错误返回（注：initupload 响应经 ec115 加密——P-224 ECDH + AES-CBC + LZ4，其"成功"用例依赖测试侧实现 ECDH 加密对端，见 architecture.md 开放问题 #5；未裁决前以 initupload 错误路径（传输错误/响应不可解密）为验收基线，成功回环待 Gate#1 裁决后生效或移除）、gettoken 成功与 status_code!=200、oss_put 2xx 与非 2xx（联动 AC11），每格至少 1 例且全绿。
- **AC15**（U15，溯源 intent.md 约束）：Given AC1–AC14 全部合入，When 在 Windows/Linux/macOS CI 运行全工作区构建与测试，Then 全绿；且既有内存基准（空闲 RSS 5-15MB 目标）不回退。

## 非目标（明确排除）

- **全部 P2 项（约 26-37 条）延后**到后续 intent，另行立项清点；已知名样例均不在本 intent 范围：webdav `/api/*` 硬编码伪造数据、Basic 认证非常数时间比较、`init_upload` 每次新建 `ureq::Agent`、cookie 通道 0.5 r/s 限速参数、verify_upload sha1-OR-name 回退校验过宽等（完整清单见 audit-findings.md P2 节）。
- **受阻项**：M5 macOS FileProvider、minifilter 驱动、边缘 48h 浸入测试——与缺陷修复无关，不进入本 intent。
- 不做审计之外的重构与特性：不调整 crate 边界、不更换日志/HTTP 框架、不改对外协议与 CLI 语义（缺陷本身即错误行为的除外，如 F-13 的退出码、F-05 的拒绝启动）。
- 修复过程中新发现的缺陷不顺手修：一律记录并交编排者裁决是否升格 P0/P1 或归入 P2。

## 开放问题（需人类裁决）

- [ ] **F-05 拒绝语义**：Cookie 通道"未启用/未同意"时，daemon 是硬拒绝（非零码退出，AC5 当前默认），还是 warn 后回退 open 通道继续运行？审计只要求"回接配置开关与同意标志校验"，两种语义均可成立。
- [ ] **F-12 service.rs 处置**：审计指出 service.rs 存在无调用方代码并给出"接线或删除"两方向，未指认具体函数面；请裁决删除（AC12 当前默认以 dead_code 无告警为验收）还是明确接线目标。
- [ ] **F-13 fail-fast 范围**：解析失败 fail-fast 已定；请确认配置文件**存在但不可读**（权限/IO 错误）是否同样 fail-fast（AC13 当前默认：是，只有"文件不存在"走默认值）。
- [ ] **范围确认（沿 intent 开放问题）**：本 intent 仅覆盖 P0+P1（F-01…F-14）；若 Gate#1 前审计方补充新的 P0/P1 条目，是并入本 intent 还是另立？P2 延后默认不变。
