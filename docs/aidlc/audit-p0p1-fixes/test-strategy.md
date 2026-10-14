# Test Strategy: audit-p0p1-fixes

> 产出者：qa　日期：2026-10-14　来源：intent.md + audit-findings.md
>
> 说明：PRD 正在并行编写、AC 编号未定，本版映射列以 audit-findings.md 的 **F-xx** 为锚；Gate 对齐时由对齐拍核对 F-xx ↔ AC。优先级与缺陷严重度对齐：F-01（P0 缺陷）及安全/数据丢失类（F-03/F-04/F-09/F-11）的测试点为 P0。
>
> 统一入口：`cargo test`（全工作区）。所有新测试零网络、零真实凭据，复用既有基建：
> - `cloudmount-sync/src/testutil.rs`：内存 `MockRemote`（完整 Provider + OpCounts 请求计数）
> - `cloudmount-driver115/tests/mock_api.rs`：手写本地 TCP mock HTTP 服务器（Recorded 请求 + 可编程响应）
> - `cloudmount-daemon/tests/webdav_serve.rs`：WebDAV 全链路（`#![cfg(feature = "mock")]`、随机端口、hyper 客户端）
> - `cloudmount-vfs/tests/cfapi_backend.rs`：windows 门控 + SyncRoot 注册失败（0x80070490）自动跳过
> - `cloudmount-core/src/config.rs` 的 `ENV_LOCK`：改环境变量的测试互斥模式

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| T-01 | F-01 | 正：分块水合内容正确——中等大小文件（如 8-16MB）随机偏移读取，FETCH_DATA 回调序列拼接结果与源一致，且**单次回调请求长度 ≤ 块上限**（不再出现完整 RequiredLength） | 单元（拷贝循环纯函数）+ 集成（cfapi_backend，windows 门控） | P0 |
| T-02 | F-01 | 负：Provider `open_range` 中途短读/返回 Err → 水合干净失败并传播错误，无无界缓冲累积、无 panic、无部分写入残留 | 单元（注入短读/错误 Reader）+ 集成 | P0 |
| T-03 | F-02 | 正：`ProviderRouter.change_probe` 转发活跃通道——MockRemote 作假通道，断言 `ops().change_probe` 计数 +1 且 `list` 计数**不增长**（快路径确实生效，非回退全量） | 集成（driver115 tests + MockRemote OpCounts） | P1 |
| T-04 | F-02 | 负：活跃通道 `change_probe` 返回 Err → 路由层原样透传错误，不吞成 `None`、不静默回退全量列表 | 单元 | P1 |
| T-05 | F-03 | 负（攻击输入）：webdav_serve 全链路发送 `/dav//C:/windows/win.ini`、`/dav/C:\foo`、`/dav/../secret`、`%2e%2e%2f`、`..%5c`、`//server/share` 风格、空段路径 → 一律 4xx，且响应体不含 root 外预置金丝雀文件内容 | 集成（webdav_serve hyper 客户端） | P0 |
| T-06 | F-03 | 正+负：成分级校验函数表驱动——盘符段、`..`、空段、反斜杠、绝对路径一律拒绝；合法名（中文、空格、`a.b`、`...file` 等易误伤名）必须通过（防修复过宽破坏正常文件） | 单元 | P0 |
| T-07 | F-04 | 负（失败注入）：本地 mock 服务器 + tracing 捕获订阅者，debug 级全开跑 uploadinfo/gettoken/表单提交，**含非 2xx 失败响应路径**——断言日志事件中不含金丝雀 userkey/STS 明文（只允许字段名/长度/哈希） | 集成（mock_api 模式 + tracing 测试层） | P0 |
| T-08 | F-05 | 正+负：daemon 组装决策三态——Cookie 通道默认关 → 不构造 Cookie115Provider；enabled=true 但缺知情同意标志 → 拒绝并报错（不静默放行）；两者齐备 → 构造成功 | 单元（main.rs 组装逻辑需提取为可测函数，见缺口） | P1 |
| T-09 | F-06 | 正：50-100 轮"创建 RdcWatcher → drop"循环后，线程存活计数回落基线（rdc 内 AtomicUsize 计数 hook）；Windows 上 drop 后可 rename/remove 被监视目录（句柄确已释放） | 单元/集成（cloudmount-sync） | P1 |
| T-10 | F-06 | 负：先灌入事件洪峰再 drop → 有界 join 完成、不 panic、不悬挂、计数仍回落 | 单元/集成 | P2 |
| T-11 | F-07 | 正：SlowMockRemote（create_dir 延迟 300-500ms）下并发两个事件（一个触发 ensure_remote_dir、一个普通上传），断言第二操作完成时间显著小于串行和（锁未被网络往返占住） | 集成（tokio::join / 双线程） | P1 |
| T-12 | F-07 | 负：create_dir 返回 Err → 仅该条目报错、锁释放、后续事件继续处理（无级联卡死） | 集成 | P1 |
| T-13 | F-08 | 正：WebDAV PUT 全链路后 spool 目录零残留；flush 内 `take()` 后缓冲为空、二次 flush 为 no-op（幂等） | 集成（webdav_serve）+ 单元 | P1 |
| T-14 | F-08 | 负（失败注入）：flush 期间保持第二个无共享句柄模拟 Windows 共享冲突（remove_file 失败）→ 不 panic、记录降级日志、句柄关闭后重试清理最终成功、无 spool 堆积 | 集成（`#[cfg(windows)]` 门控） | P1 |
| T-15 | F-09 | 负（失败注入）：目标文件存在但不可打开（外部 share-none 句柄 / 权限拒绝注入）→ PUT 返回错误（5xx）而非静默新建，**原文件内容不变**；同目录被文件占位等混淆场景一并覆盖 | 集成（webdav_serve）+ 单元（错误分类函数） | P0 |
| T-16 | F-09 | 正：NotFound → 正常新建；目标存在且可打开 → 正常覆盖（回归保护：修复不破坏创建/覆盖语义） | 集成 | P1 |
| T-17 | F-10 | 正：N=10 线程在 token 过期点会合后并发触发认证请求 → mock gettoken 服务端**恰收到 1 次**刷新、全体调用方拿到有效 token；循环 10 轮降 flaky | 集成（mock_api 模式，OpenApiAuth 已有先例） | P1 |
| T-18 | F-10 | 负：刷新端点返回 401/500/坏 JSON → 全体等待者收到 Err、无死锁、锁已释放（随后单线程重试成功） | 集成（mock_api 模式） | P1 |
| T-19 | F-11 | 负（非 2xx 响应）：mock OSS 端点返回 403 / 500 → `oss_put` 返回 Err 且错误含状态码，上层上传流程**不报成功**（数据丢失防护闸门） | 集成（mock，依赖 T-23 基建） | P0 |
| T-20 | F-11 | 正：OSS 200 + 合法响应 → Ok；顺带断言 PUT 的 URL/签名头/UA 构造正确（F-14 覆盖面） | 集成（mock） | P0 |
| T-21 | F-12 | 正+负：SCM 运行路径决策逻辑（提取为可测函数 + 注入挂载工厂捕获调用）——placeholder_mount=true → 占位符分支执行且与前台路径一致；false → 常规分支；service.rs 接线或删除以编译期引用检查兜底 | 单元（main.rs 重构出口） | P1 |
| T-22 | F-13 | 负+正：损坏 config.toml（语法错/未知字段/文件不可读）→ fail-fast Err、错误信息含文件路径、**默认值不生效**；合法配置 → 值生效。涉及环境变量时按 ENV_LOCK 模式互斥 | 单元 | P1 |
| T-23 | F-14 | 基建：cookie 通道 HTTP 级 mock transport（**方案：本地测试服务器**，见分层策略），url 注入扩展覆盖 uplb/OSS；作为 T-07/T-17/T-18/T-19/T-20 的宿主，覆盖 uploadinfo/gettoken/oss_put/initupload 四端点错误矩阵 | 集成（driver115 tests/） | P1 |
| T-24 | F-14 | 正：initupload 秒传命中（零传输、不请求 gettoken）与未命中（走直传）HTTP 级回归 | 集成（mock） | P2 |

**计数：P0 = 8（T-01/02/05/06/07/15/19/20），P1 = 14，P2 = 2。P0 已齐，可进 Construction。**

## 分层策略

- **单元**：各 crate 内 `#[cfg(test)]`，覆盖可在纯逻辑层判定的缺陷核心——F-03 路径校验函数（表驱动，含防过宽的合法名用例）、F-09 NotFound/其它错误分类函数、F-13 配置加载器（ENV_LOCK 模式）、F-06 Drop 的 take/停止标志语义、F-10 双检逻辑、F-01 分块拷贝循环（注入短读/错误 Reader，断言块长上界与错误传播）、F-05/F-12 的组装/分支决策函数（需先把 main.rs 内联逻辑提为可测函数——TDD 先写测试会倒逼该最小重构，边界见开放问题）。**不测**：ureq 真实网络行为、Windows SCM 生命周期。
- **集成**：跨 crate 复用四套既有基建——`webdav_serve.rs`（hyper 全链路，`feature="mock"`，随机端口）承载 F-03 攻击向量、F-08/F-09 的 HTTP 层验证；`mock_api.rs` 手写 TCP mock 服务器模式承载 F-10/F-11/F-14/F-04（OpenApiAuth 已有同款先例，直接扩展）；`testutil::MockRemote` + OpCounts 承载 F-02（路由转发计数断言）与 F-07（SlowMockRemote 委托包装注入 create_dir 延迟）；`cfapi_backend.rs`（windows 门控、SyncRoot 注册失败自动跳过、真机全跑）承载 F-01 端到端内容正确性。**不测**：真实 115 响应格式漂移（mock 固定响应，格式回归属后续 intent）。
- **E2E**：自动化上限 = `cargo test` 全工作区 + CI 三平台绿（intent 完成判据）。以下只能人工验证清单：① 真机 Windows CfAPI 挂载真实 115，读取 ≥4GB 文件任意字节、观察 RSS 有界（F-01 真实规模复现；CI 无法承受，注意 115 审核通道限制——用合规测试文件避免秒传/水合被审核拦截）；② SCM 服务真实安装/启动/卸载（需提权与服务控制管理器，F-12）；③ 真实 115 cookie 通道 debug 级日志产物人工抽查无凭据（F-04 纵深）；④ 弱网演练 ensure_remote_dir 不再全局卡顿（F-07 主观验证）；⑤ macOS 挂载回归（当前环境无 Mac，仅 CI runner 编译+测试覆盖，行为验证依赖 CI 矩阵与后续人工）。

### F-14 mock transport 方案抉择（本地测试服务器，二选一已定）

**选本地测试服务器**（扩展 `mock_api.rs` 的 `spawn_mock`/`Recorded` 模式），不选可注入 transport trait。理由：
1. 仓库已有完全相同的先例（mock_api.rs 即用本地 TCP 服务器测 Open 通道的请求构造与响应解析），零新生产抽象；
2. cookie 通道缺陷恰在真实 ureq 栈上（cookie jar、响应状态处理、头构造）——fake transport 会绕过这些机制，F-04/F-11 的回归价值大打折扣；本地服务器打的是真 HTTP；
3. 生产侧仅需小扩展：`with_base_urls`（cookie.rs:220，已覆盖 webapi/proapi）增加 uplb 基址与 OSS endpoint/bucket 参数（现为 const，cookie.rs:40/42/605），纯加法不改行为。
前提确认：本地服务器为明文 HTTP，需 ureq Agent 不强制 https——mock_api.rs 先例证明 Open 通道可行，cookie 通道同一 Agent 家族，风险低。

### 并发类缺陷（F-06/F-07/F-10）可重复测法与 flaky 缓解

- **F-06 泄漏**：不依赖系统线程枚举（std 无此 API），在 rdc.rs 内置 `AtomicUsize` 活跃线程/句柄计数（生产路径代价可忽略），drop 后**轮询至基线**（上限 5s）而非单次断言；50-100 轮循环把泄漏信号放大为 +N，即使单轮时序抖动也可稳定检出；Windows 句柄释放用"drop 后可删除被监视目录"二次验证。
- **F-07 锁内 IO**：SlowMockRemote 注入 300-500ms 延迟（远大于调度噪声），双事件并发完成时间阈值放宽为 < 串行和的 60%，重复 5-10 轮；避免纯墙钟单次断言。
- **F-10 并发刷新**：测试控制 token 过期（直接注入过期 TokenSet，不 sleep 真实时钟），用 channel/栅栏使 10 线程在过期点会合后并发；服务端计数断言 `==1` 是离散判定（非时序），天然低 flaky；外层循环 10 轮兜底。
- 通用：并发测试标记 `#[ignore]` 之外不设夜间专用集——数量少且上述设计已压低 flake；若 CI 出现偶发，优先加轮询与循环次数而非放宽语义断言。

## 缺口与开放问题

- [ ] **F-01 真实 4GB 规模无法 CI 自动化**：以"回调长度有界 + 分块计数 + 中等文件内容一致"作代理断言；真机大文件 RSS 观察进人工清单。
- [ ] **F-12 SCM 真实服务生命周期**（sc create/start、提权、服务账户）无法自动化：止步于决策函数单测，安装/启动/卸载进人工清单。
- [ ] **Windows 专属注入**（T-14 共享冲突、T-09 句柄释放验证）在 Linux CI 上不可运行：按 cfapi_backend.rs 先例 `#[cfg(windows)]` 门控，需在 CI 矩阵中明确哪些测试 windows-only，避免"三平台绿"误读为全量覆盖。
- [ ] **main.rs 可测性重构边界**：F-05/F-12 要求把 SCM 分支与驱动组装逻辑提为可测函数（注入工厂/出口 seam）——是否算"最小重构"需架构对齐确认；若不允许，这两项测试降级为集成级黑盒（通过进程退出码/日志断言），覆盖变弱。
- [ ] **ProviderRouter 测试注入口**：router 若只接受具体通道类型（Cookie115Provider/Open115Provider），需为测试提供 `Arc<dyn Provider>` 或泛型构造器；MockRemote 已实现完整 Provider 可直接作假通道，改动小但需 dev 配合。
- [ ] **F-04 日志捕获依赖**：假定 cookie.rs 全部为 tracing 宏（含 `tracing::debug!`）；若存在 log crate 桥接或 println 残留，需补 bridge 断言——Construction 时先核实。
- [ ] **OSS endpoint 参数化是否算改对外行为**：`with_base_urls` 扩展为纯加法 builder 参数，判定为不算，但需在 review 时确认无调用方依赖硬编码值。
- [ ] **无 Mac 本地环境**：macOS watcher/挂载行为仅 CI runner 覆盖，跨平台行为回归存在盲区，记录为环境性缺口。
- [ ] **P2 缺陷明确不设测试点**（webdav `/api/*` 伪造数据、非常数时间 Basic 比较、init_upload 重复建 Agent、限速、verify_upload 校验过宽）：随 P2 intent 另行立项，避免范围蔓延。
