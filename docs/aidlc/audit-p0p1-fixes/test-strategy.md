# Test Strategy: audit-p0p1-fixes

> 产出者：qa　日期：2026-10-14（Gate#1 打回修订轮 v2）　来源：intent.md + audit-findings.md + reviews/gate1-ai.md + Gate#1 人类/编排者裁决
>
> 修订说明：本版按互审（B2/MA3/MA5/MA7/MA8/MA9/MA10/MA1/m1/m2/m3/m5/m6/m7/m8/m14/m15/m16）与 Gate#1 裁决逐条修订：T-05 攻击向量重打静态路由面并扩 harness、映射列回填 AC-x 双锚、T-15 注入机制改为 error-injectable Provider 包装、新增 T-25（CI 矩阵语义）/T-26（通道切换）/T-27-T-28（desktop 壳）/T-29（build_launch_args --interval）、T-24 扩展 ECDH 成功回环并升格 P0、F-08 spool 语义与数值口径按 canonical 统一。分层策略结构保留。
>
> 映射口径（MA5 回填）：**AC1↔F-01 … AC14↔F-14 按序 1:1**；AC15 为 intent 约束项（映射 T-25）；desktop 壳新增 AC（AC16 等）与 AC12 的 --interval 子句由 pm 并行修订回填——本稿按 Gate#1 裁决已知项锚定，编号以 Gate 对齐拍为准。prd.md/architecture.md 磁盘版本为修订前快照，冲突处以 reviews/gate1-ai.md 及 canonical 裁决为准。
>
> 优先级口径：F-01（P0 缺陷）及安全/数据丢失类（F-03/F-04/F-09/F-11）测试点为 P0；T-24 因 Gate#1 裁决扩围（ECDH 成功回环）且为 T-19/T-20（F-11 数据丢失闸门，P0）经公开 upload() 到达 oss_put 的可达性前置，升格 P0。
>
> 统一入口：`cargo test --workspace` + `cargo test --workspace --features cloudmount-daemon/mock`（后者为 T-25 要求 CI 增补的腿，本地等价命令）。所有新测试零网络、零真实凭据，复用既有基建：
> - `cloudmount-sync/src/testutil.rs`：内存 MockRemote（完整 Provider + OpCounts 请求计数）
> - **测试侧 Provider 包装族**（各 tests/ 文件内以 `Arc<dyn Provider>` 委托实现，零生产改动）：ErrorInjectingProvider（T-15，可编程对指定 list 返回非 NotFound 错误）、RecordingProvider（T-13/T-14，捕获 flush 上传传入的 `UploadSource::Path` 精确 spool 路径）、SlowMockRemote（T-11，list 与 create_dir 双延迟）
> - `cloudmount-driver115/tests/mock_api.rs`：手写本地 TCP mock HTTP 服务器（Recorded 请求 + 可编程响应）——cookie 通道复用同款模式
> - `cloudmount-daemon/tests/webdav_serve.rs`：WebDAV 全链路（`#![cfg(feature = "mock")]`、随机端口、hyper 客户端）——**本版扩展**：`start_server_with_ui()`（传 web_ui_dir fixture，使 serve_static 可达）与任意 `Arc<dyn Provider>` 注入口（供包装族进入）
> - `cloudmount-vfs/tests/cfapi_backend.rs`：windows 门控 + SyncRoot 注册失败（0x80070490）自动跳过
> - `cloudmount-core/src/config.rs` 的 `ENV_LOCK`：改环境变量的测试互斥模式

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| T-01 | AC1（F-01） | 正：分块水合内容正确——中等大小文件（8-16MB）随机偏移读取，内容逐字节一致；**断言口径为 reader 级读取序列**（m1）：桩 Reader 记录每次 `read` 返回的长度序列，断言呈 1 MiB 等长循环（修复后）；修复前 `read_to_end` 一次性填满整个容量缓冲（几何增长形态），两形态可区分、可造红测；**勿按 open_range 的 RangeSpec 断言**（修复前后都只调用一次，无判别力） | 单元（分块拷贝循环提为 crate 内可测函数，seam 见架构 MA6 声明）+ 集成（cfapi_backend，windows 门控） | P0 |
| T-02 | AC1（F-01） | 负：Provider Reader 中途短读/返回 Err → 水合干净失败并传播错误，无无界缓冲累积、无 panic、无部分写入残留；取消路径（CANCEL_FETCH_DATA 置位后）以桩 Reader 的 drop 计数断言下游 reader 被关闭（不悬挂） | 单元（注入短读/错误 Reader）+ 集成 | P0 |
| T-03 | AC2（F-02） | 正：`ProviderRouter.change_probe` 转发活跃通道——**在 router.rs 内扩展现成 Stub**（零 dev-dep，m10：`ProviderRouter::new` 本就收 `Box<dyn Provider>`，router.rs:54-67；不引 cloudmount-sync dev-dep），断言活跃通道 change_probe 计数 +1 且 `list` 计数**不增长**（快路径确实生效，非回退全量） | 单元（router.rs `#[cfg(test)]` Stub） | P1 |
| T-04 | AC2（F-02） | 负：活跃通道 `change_probe` 返回**非认证类** Err → 路由层原样透传错误（不吞成 `None`、不静默回退全量列表）；认证类 Err 的切换语义由 T-26 单独覆盖（m7：call() 对 Auth 失效走通道切换，不属"原样透传"面） | 单元（router.rs Stub） | P1 |
| T-05 | AC3（F-03） | 负（攻击输入）+ 正（m6 并入）：**harness 前置扩展**——webdav_serve 新增 `start_server_with_ui()`：向 webdav::serve 第 4 参传 web_ui_dir fixture（tempdir 布局：`webui/index.html`、`webui/assets/app.js` + **root 外金丝雀** `tempdir/canary-secret.txt`；现状 webdav_serve.rs:30 传 None → 非 /dav 路径直接 404（webdav.rs:813-818），被测函数不可达）。攻击向量**不带 `/dav/` 前缀**（`/dav/*` 经 webdav.rs:804-812 路由进 ProviderFs，到不了被修复的 serve_static :922-932），直接打静态路由面：`/C:/Windows/win.ini`（盘符，红测主向量——修复前 Windows 上 `root.join` 遇绝对成分替换基路径、返回 200 真实内容）、`/C%3A/foo`、`/C%3A%5Cfoo`（`C:\foo` 百分号编码形态）、`/..%2F..%2Fcanary-secret.txt`、`/a/..%2F..%2Fcanary-secret.txt`、`//C:/Windows/win.ini`（空段+盘符）、`//server/share/f` 与 `/%5C%5Cserver%5Cshare%5Cf`（UNC——现状经 trim 前导斜杠不逃逸，保留为纵深防御，断言不泄漏即可）→ 一律 403/404 且响应体不含金丝雀内容（Windows 上另断言不含 win.ini 真实内容）。正路径（m6）：`/`、`/index.html`、`/assets/app.js` → 200 且内容与 fixture 逐字节一致。注：hyper `Uri` 对 `\` 等非 URI 合法字符需百分号编码（否则客户端 parse 失败），向量一律以编码形态发送 | 集成（webdav_serve 扩展 harness） | P0 |
| T-06 | AC3（F-03） | 正+负：成分级校验函数表驱动——盘符段（`C:`）、`..`、`.`、反斜杠、NTFS 备用数据流（`:`）、Windows 保留设备名（CON/PRN/AUX/NUL/COM1-9/LPT1-9）一律拒绝；合法名（中文、空格、`a.b`、`...file` 等易误伤名）必须通过（防修复过宽破坏正常文件）；放行路径断言解析结果 canonicalize 后 `starts_with(root)`（AC3 单元锚） | 单元 | P0 |
| T-07 | AC4（F-04） | 负（失败注入）：本地 mock 服务器 + **手写 tracing Subscriber**（约 40 行，仅 tracing 本体——driver115 零 dev-deps，MakeWriter 属 tracing-subscriber 不可用，m2），**trace 级全开**（canonical 口径）捕获 uploadinfo/initupload/gettoken/get_info 全流程（含非 2xx 失败响应路径）——断言日志事件中不含金丝雀 userkey/STS 明文（逐值断言缺席，只允许字段名/长度/哈希）。**断言面扩**（canonical 裁决并入 F-04）：① cookie.rs:796 get_info 原始 JSON debug 日志脱敏（stat 流程一并跑通并断言）；② cli main.rs:233 登录校验失败的错误消息不含原始 cookie 串（校验函数提取后单元断言，仅含缺失字段名） | 集成（driver115/tests/ mock + 手写 Subscriber）+ 单元（cli 校验函数） | P0 |
| T-08 | AC5（F-05） | 正+负：daemon 组装决策三态——Cookie 通道默认关（enabled=false 或 consent_acknowledged=false）且 `--channel cookie` → 拒绝并报错（不静默放行）；双真 → 构造成功（保留既有协议风险 warn）；auto 时缺配置 → 静默跳过 Cookie 兜底（纯官方，MA11 断言面）。**单元代理**（m16/canonical）：main Result 边界提取后断言 Err 链（错误信息指明需显式开启并确认同意）；进程级黑盒（非零退出码/输出）进人工清单 | 单元（main.rs 组装逻辑提取为可测函数） | P1 |
| T-09 | AC6（F-06） | 正：**50-100 轮循环（断言下限 50，canonical 口径）**"创建 RdcWatcher → drop"后，rdc.rs 内置 AtomicUsize 活跃计数（seam 见架构 MA6 声明）轮询回落基线（上限 5s，非单次断言）；Windows 上 drop 后可 rename/remove 被监视目录（句柄确已释放，windows-only——CI skip 面见 T-25） | 单元/集成（cloudmount-sync） | P1 |
| T-10 | AC6（F-06） | 负：先灌入事件洪峰再 drop → 有界 join 完成、不 panic、不悬挂、计数仍回落；join 超时/detach 降级语义按架构修订版（MA4）断言（超时路径 warn、不永久挂起） | 单元/集成 | P2 |
| T-11 | AC7（F-07） | 正：SlowMockRemote **同时延迟 list 与 create_dir**（各 300-500ms——m5：AC7 要求两者都不在锁内）；**主判据 = 锁时序记录（确定性，canonical 口径）**：桩在 list/create_dir 被调时采样 exec.rs 锁持有观测 seam（架构修订声明，如 AtomicBool/时间戳记录），断言全部网络调用样本均"锁未持有"；**副判据 = 第二操作完成时间 < 串行和 60%**（时序噪声大，仅辅助；重复 5-10 轮） | 集成（cloudmount-sync） | P1 |
| T-12 | AC7（F-07） | 负：create_dir 返回 Err → 仅该条目报错、锁释放、后续事件继续处理（无级联卡死）；并发 N 线程对同一深层目录 ensure_remote_dir → create_dir 恰一次、全体返回同一 ID（per-前缀锁+双检回归） | 集成 | P1 |
| T-13 | AC8（F-08） | 正：PUT 全链路后 spool 清理——**断言针对本次 PUT 的具体 `cm-dav-put-*` 路径**（m15：RecordingProvider 包装捕获 flush 上传时传入的 `UploadSource::Path` 精确路径（webdav.rs:612），对该路径断言不存在；勿全目录 glob——cargo 并行测试互踩假红/假绿）；flush 幂等（二次 flush no-op）与 `provider.upload` 恰被调用一次（OpCounts，排除 Drop 二次 flush——flushed 在上传确认成功后置位语义的固化断言，m3）。红测判别为 Windows 腿（现状 `as_ref()` 半泄漏句柄致 remove 失败残留；Linux unlink 语义两态皆过） | 集成（webdav_serve + RecordingProvider）+ 单元 | P1 |
| T-14 | AC8（F-08） | 负（失败注入，windows-only）：flush 期间 RecordingProvider 在 upload 回调内对**本次 spool 精确路径**打开无共享句柄（模拟 Windows 共享冲突，remove_file 失败）→ 不 panic、warn 日志记录路径（捕获型 Subscriber 断言）、**接受残留**（canonical 裁决：删除失败 = warn + 接受残留，**不做"重试最终成功"断言**——残留文件存在即为预期终态，m3）、上传结果不受影响（成功如实成功，不掩盖） | 集成（`#[cfg(windows)]`） | P1 |
| T-15 | AC9（F-09） | 负（失败注入）：**ErrorInjectingProvider**（`Arc<dyn Provider>` 委托包装，测试文件内实现，可编程对指定目录 id 的 `list` 返回非 NotFound 错误——MA7：F-09 的 Err 产生于 resolve_path→**远端 Provider** 调用（webdav.rs:291-309），本地 share-none 句柄/权限注入压不住该路径，harness 须换注入口）——注入 list(目标父目录) 返回权限/IO 类错误后 PUT `/dav/dir/file.txt`（dir 内预置既有文件）→ 返回 5xx 而非静默新建、**远端原内容不变**（MockRemote 读回断言）、`upload` OpCounts == 0；同目录被文件占位等混淆场景一并覆盖 | 集成（webdav_serve + 注入包装）+ 单元（错误分类函数：仅 NotFound 走合成新条目） | P0 |
| T-16 | AC9（F-09） | 正：NotFound → 正常新建；目标存在且可打开 → 正常覆盖（回归保护：修复不破坏创建/覆盖语义） | 集成 | P1 |
| T-17 | AC10（F-10） | 正：**10 线程**（canonical 口径）在 token 过期点会合（channel/栅栏；直接注入过期 TokenSet，不 sleep 真实时钟）后并发触发认证请求 → mock 刷新端点**恰收到 1 次**请求、全体调用方拿到同一有效 token；循环 10 轮降 flaky（服务端计数 ==1 为离散断言，天然低 flaky）。**会合入口覆盖 access_token() 与 refresh() 两个入口**（MA1：refresh() 复用同一持锁刷新助手后纳入——client.rs:244 并发重试路径同一单飞语义） | 集成（mock_api 模式，OpenApiAuth 已有先例） | P1 |
| T-18 | AC10（F-10） | 负：刷新端点返回 401/500/坏 JSON → 全体等待者收到一致 Err、无死锁、锁已释放（随后单线程重试成功——串行重试为合法形态）；refresh() 入口同口径 | 集成（mock_api 模式） | P1 |
| T-19 | AC11（F-11） | 负：mock OSS 端点返回 403/500 → 经公开 upload() 全链路（initupload 成功回环后到达 oss_put）返回 Err 且错误含 HTTP 状态码与响应体摘要，上层上传结果**不报成功**（数据丢失防护闸门） | 集成（driver115/tests/，依赖 T-23/T-24 基建） | P0 |
| T-20 | AC11（F-11） | 正：OSS 200 + 合法响应 → Ok；顺带断言 PUT 的 URL 构造（完整基址含 scheme 注入后指向 mock）/签名头（Authorization OSS 签名）/UA 正确（F-14 覆盖面） | 集成（driver115/tests/） | P0 |
| T-21 | AC12（F-12） | 正+负：SCM 运行路径决策逻辑（提取为可测函数 + 注入挂载工厂捕获调用）——placeholder_mount=true → 占位符分支执行且与前台路径一致（两路径产出配置一致）；false → 常规分支。死入口删除判据用**结构性断言**（`define_windows_service!`/`service_main` 符号不复存在——MA10：现状 service.rs:148 `#[allow(dead_code)]` 压制告警，"无告警"判据今天就是绿的、无判别力；辅以删除 allow 属性后 clippy -D warnings 绿） | 单元（main.rs 重构出口） | P1 |
| T-22 | AC13（F-13） | 负+正：损坏 config.toml（语法错/未知字段/路径指向目录=存在但不可读的跨平台构造）→ fail-fast Err、错误链含**"解析失败"统一子串与文件路径**（m8：与 pm 修订后 AC13 措辞对齐）、默认值不生效；文件不存在 → 内置默认值正常启动（回归，load_from 既有语义）。**单元代理**（m16/canonical）：main Result 边界提取后断言 Err 链；进程级黑盒（非零退出码/输出）进人工清单。涉及环境变量时按 ENV_LOCK 模式互斥 | 单元 | P1 |
| T-23 | AC14（F-14） | 基建：cookie 通道 HTTP 级 mock（**driver115/tests/ 集成层**——canonical：测试注入面统一，非 `#[cfg(test)]` 私有字段注入）；生产侧 `with_base_urls` **纯加法扩展**：uplb 基址与 OSS endpoint 存**完整基址含 scheme**（cookie.rs:605 `format!("https://{bucket}.{OSS_ENDPOINT}/…")` 硬编码 scheme——存完整基址本地明文 mock 才打得通，m14）；ECDH 对端形态由架构定（本表测试点按"集成层可达"书写）。作为 T-07/T-17/T-18/T-19/T-20/T-24 的宿主，覆盖 uploadinfo/gettoken/oss_put/initupload 四端点错误矩阵 | 集成（driver115/tests/） | P1 |
| T-24 | AC14（F-14） | 正：initupload **成功回环**（Gate#1 裁决纳入本期：测试侧实现 ECDH 加密对端——p224/aes/lz4_flex 均已是正式依赖，mock 服务器以对端身份用 ECDH 会话加密 initupload 成功响应，驱动解密后进入直传分支）+ 秒传命中（零传输、不请求 gettoken——请求计数断言）与未命中（走直传）HTTP 级回归。**可达性前置**：T-19/T-20（P0）经公开 upload() 到达 oss_put 依赖本回环（initupload 须先成功），Construction 须与 T-23 同批最先落地。与既有 Open 通道秒传测试（mock_api.rs:225-263 `rapid_upload_hits_zero_transfer_…`）的关系：**并行收编非替代**——Open 通道回归由既有用例守护（表单 fileid/target 断言），cookie 通道新格式（ECDH 加密表单体）由本点守护，断言面不同 | 集成（driver115/tests/） | P0（原 P2，裁决扩围后升格） |
| T-25 | AC15（intent 约束项） | CI 矩阵语义（MA8/B1）：①CI 增加 `cargo test --workspace --features cloudmount-daemon/mock` job（现状 ci.yml:34 只跑 `cargo test --workspace`，webdav_serve.rs 整文件 `#![cfg(feature = "mock")]` 门控从不执行——F-03/F-08/F-09 集成测试在 CI 空绿）；②windows-only 清单显式化：T-14 共享冲突注入、T-09 句柄释放验证（drop 后 rename/remove）——Linux 跳过；③cfapi 集成腿（T-01/T-02）在 GH windows runner 因 SyncRoot 注册失败（0x80070490）自动跳过 → CI 实际守护只剩单元腿——"三平台绿"的 skip 面语义由此写明，防误读为全量覆盖。断言方式：CI yml 变更评审 + 本地 `cargo test -p cloudmount-daemon --features mock` 全绿可复现。AC15 的 RSS 子句判据落人工清单（静置 RSS 观察，见 E2E），CI 矩阵语义由本点守护 | 集成（CI 基建） | P1 |
| T-26 | AC2（F-02） | 负（m7）：活跃通道 `change_probe` 返回**认证失效错误**（Error::Auth）→ 走与其它方法一致的通道切换处理：断言 `last_switch()` 记录（from/to/reason）、后续调用路由到切换后通道、错误不静默吞成 None | 单元（router.rs Stub） | P1 |
| T-27 | AC16（desktop 壳，pm 修订新增；编号以对齐拍为准） | 正+负：desktop 登录写回 [cookie]（Gate#1 裁决纳入）——config 层单元测：写回函数作用于含/缺 `[cookie]` 段的 config.toml（tempdir）→ save → reload 往返断言 `enabled=true, consent_acknowledged=true`；负：未写回时缺段 = 双 false（通道关，serde default）。GUI 登录交互流程进人工清单 | 单元（config 层） | P1 |
| T-28 | AC16（desktop 壳） | 正：desktop 组装从配置构造 CookiePolicy——现状 apps/desktop/src/main.rs:143-149 硬编码 {true,true}；修复后 login_auth 的 policy 构造改读 config。若提取为纯函数（如 `cookie_policy_from_config(&Config) -> CookiePolicy`）则单元测（双真 → {true,true}；缺段 → {false,false}），否则降级人工清单（缺口已记） | 单元（可提取时；否则人工清单） | P1 |
| T-29 | AC12（F-12，--interval 子句——pm 修订回填） | 正+负：`build_launch_args` 快照测试（MA9：--interval 潜伏 bug——install 不烘焙（main.rs:294-315 现状无 interval）、服务模式恒默认 30s）：placeholder_mount/sync_root/remote_root/propagate_* 各组合 + `--interval 120` → 参数向量含 `--interval 120`（快照断言整向量）；负：未显式给 interval → 按架构修订语义（不烘焙或烘焙默认，快照固化） | 单元 | P1 |

**计数：P0 = 9（T-01/02/05/06/07/15/19/20/24），P1 = 19，P2 = 1（T-10）。共 29 个测试点；AC1-AC16 全部有映射落点（AC15 经 T-25 + 人工清单 RSS 判据）；每条主流程（F-01…F-14 + desktop + --interval）至少一个负路径。P0 已齐，可进 Construction。**

## 分层策略

- **单元**：各 crate 内 `#[cfg(test)]`，覆盖可在纯逻辑层判定的缺陷核心——F-03 路径校验函数（表驱动，含防过宽合法名 + canonicalize 锚）、F-09 错误分类（仅 NotFound 走合成新条目）、F-13 配置加载器（ENV_LOCK 模式）、F-06 Drop 的 take/停止标志/close 幂等语义、F-10 双检与锁语义（并发入口见集成）、F-01 分块拷贝循环（注入短读/错误 Reader，断言 reader 级读取序列与错误传播）、F-02 路由转发/透传/切换（router.rs 内 Stub，零 dev-dep）、F-05/F-12/F-13 的组装/分支/参数构造函数（main Result 边界提取，单元代理口径见 m16）、desktop `[cookie]` 写回与 CookiePolicy 构造（config 层）、cli cookie 校验错误脱敏。**不测**：ureq 真实网络行为、Windows SCM 生命周期、GUI 交互。
- **集成**：跨 crate 复用四套既有基建 + 测试侧 Provider 包装族——`webdav_serve.rs`（hyper 全链路，`feature="mock"`，随机端口；**本版扩展**：`start_server_with_ui` 传 web_ui_dir fixture 使 serve_static 可达 + 任意 `Arc<dyn Provider>` 注入口供 ErrorInjecting/Recording 包装进入）承载 F-03 攻击向量与 AC3 正路径、F-08 spool 生命周期、F-09 错误上抛；`mock_api.rs` 手写 TCP mock 服务器模式承载 F-10/F-11/F-14/F-04（cookie 通道经 with_base_urls 完整基址注入 + ECDH 对端）；`testutil::MockRemote` + OpCounts + SlowMockRemote（list/create_dir 双延迟）承载 F-07 锁时序与 F-02 计数断言；`cfapi_backend.rs`（windows 门控、SyncRoot 注册失败自动跳过、真机全跑）承载 F-01 端到端内容正确性。**不测**：真实 115 响应格式漂移（mock 固定响应，格式回归属后续 intent）。
- **E2E**：自动化上限 = `cargo test --workspace` + `--features cloudmount-daemon/mock` 腿（T-25）+ CI 三平台绿（skip 面语义见 T-25，intent 完成判据）。人工验证清单：① 真机 Windows CfAPI 挂载真实 115，读取 ≥4GB 文件任意字节、观察 RSS 有界与静置 RSS（F-01 真实规模复现 + AC15 RSS 子句判据；CI 无法承受，注意 115 审核通道限制——用合规测试文件避免秒传/水合被审核拦截）；② SCM 服务真实安装/启动/卸载（需提权与服务控制管理器，F-12）；③ 真实 115 cookie 通道 trace 级日志产物人工抽查无凭据（F-04 纵深）；④ 弱网演练 ensure_remote_dir 不再全局卡顿（F-07 主观验证）；⑤ macOS 挂载回归（当前环境无 Mac，仅 CI runner 编译+测试覆盖）；⑥ **AC5/AC13 进程级黑盒**（m16：坏配置/未同意 cookie 启动 daemon 二进制 → 非零退出码 + 输出含统一子串与路径）；⑦ **desktop GUI 登录流程**（Gate#1 裁决：同意交互 → [cookie] 写回 → 组装行为，GUI 面无法自动化；T-28 不可提取时组装行为亦在此验证）。

### F-14 mock transport 方案（已定，本版按 canonical 修订）

**选本地测试服务器**（扩展 `mock_api.rs` 的 `spawn_mock`/`Recorded` 模式），测试位于 `driver115/tests/` 集成层；生产侧注入为 `with_base_urls` 纯加法扩展（uplb 基址与 OSS endpoint 存**完整基址含 scheme**，解决 cookie.rs:605 硬编码 format! 的可达性）。理由：
1. 仓库已有完全相同的先例（mock_api.rs 即用本地 TCP 服务器测 Open 通道的请求构造与响应解析），零新生产抽象、零新依赖（含 dev-deps）；
2. cookie 通道缺陷恰在真实 ureq 栈上（cookie jar、响应状态处理、头构造）——fake transport 会绕过这些机制，F-04/F-11 的回归价值大打折扣；本地服务器打的是真 HTTP；
3. ECDH 回环（Gate#1 裁决纳入本期）：测试侧实现 P-224 ECDH 加密对端（p224/aes/lz4_flex 已是正式依赖，无新供应链），对端形态由架构定——测试点按"集成层可达"书写（T-24）；
4. 前提确认：本地服务器为明文 HTTP，需 ureq Agent 不强制 https——mock_api.rs 先例证明 Open 通道可行，cookie 通道同一 Agent 家族；OSS 注入存完整基址含 scheme 后明文 mock 打得通（m14）。

### 并发类缺陷（F-06/F-07/F-10）可重复测法与 flaky 缓解（数值口径按 canonical 统一）

- **F-06 泄漏**：不依赖系统线程枚举（std 无此 API），rdc.rs 内置 `AtomicUsize` 活跃线程/句柄计数（seam 见架构 MA6 声明，生产路径代价可忽略），drop 后**轮询至基线**（上限 5s）而非单次断言；**50-100 轮循环（断言下限 50）**把泄漏信号放大为 +N，即使单轮时序抖动也可稳定检出；Windows 句柄释放用"drop 后可删除被监视目录"二次验证（windows-only，skip 面见 T-25）。
- **F-07 锁内 IO**：**主判据 = 锁时序记录**（桩调用时采样锁持有观测 seam——确定性断言，非墙钟）；副判据 = SlowMockRemote（list 与 create_dir 各延迟 300-500ms，远大于调度噪声）双事件并发完成时间 < 串行和 60%，重复 5-10 轮；避免纯墙钟单次断言。
- **F-10 并发刷新**：测试控制 token 过期（直接注入过期 TokenSet，不 sleep 真实时钟），channel/栅栏使 **10 线程**在过期点会合后并发；服务端计数断言 `==1` 是离散判定（非时序），天然低 flaky；外层循环 10 轮兜底；入口覆盖 access_token() 与 refresh()。
- 通用：并发测试不设夜间专用集——数量少且上述设计已压低 flake；若 CI 出现偶发，优先加轮询与循环次数而非放宽语义断言。

## 修订落点对照（互审编号 → 本稿落点）

- B2 → T-05（向量去 `/dav/` 前缀 + `start_server_with_ui` harness + 百分号编码注 + 盘符主向量/UNC 纵深）；B1 → T-25 + 人工清单 RSS 判据
- MA5 → 全表映射列回填 `AC-x（F-xx）` 双锚；MA7 → T-15（ErrorInjectingProvider）；MA8 → T-25；MA9 → T-29；MA1 → T-17/T-18（refresh() 入口）；MA10 → T-21（结构性断言）；MA11 → T-08（auto 断言）
- MA3/m14 → T-19/T-20/T-23/T-24（driver115/tests/ 集成层 + 完整基址含 scheme + ECDH 回环 + 与 Open 通道秒传测试关系）
- m1 → T-01；m2 → T-07；m3 → T-13/T-14（warn+接受残留）；m5 → T-11；m6 → T-05 正路径；m7 → T-04/T-26；m8 → T-22；m15 → T-13/T-14；m16 → T-08/T-22 + 人工清单⑥
- desktop 裁决 → T-27/T-28 + 人工清单⑦；ECDH 裁决 → T-24（升格 P0）；数值口径 canonical → T-09/T-11/T-17/T-07

## 缺口与开放问题

**已关闭（本修订轮）**：
- ~~ProviderRouter 测试注入口~~（m10）：router.rs:54-67 `new` 本就收 `Box<dyn Provider>`——测试在 router.rs 内扩展现成 Stub，零 dev-dep，无需任何生产改动。
- ~~ECDH 回环是否纳入~~（Gate#1 裁决）：纳入本期，T-24 扩展为 initupload 成功回环并升格 P0。
- ~~OSS endpoint 参数化是否算改对外行为~~（canonical）：`with_base_urls` 纯加法扩展、存完整基址含 scheme，判定为不算；review 时确认无调用方依赖硬编码值。

**保留缺口**：
- [ ] **F-01 真实 4GB 规模无法 CI 自动化**：以"reader 级读取序列有界 + 内容一致"作代理断言；真机大文件 RSS 观察进人工清单（AC15 RSS 子句判据同落人工清单——B1）。
- [ ] **F-12 SCM 真实服务生命周期**（sc create/start、提权、服务账户）无法自动化：止步于决策函数单测 + build_launch_args 快照（T-29），安装/启动/卸载进人工清单。
- [ ] **cfapi 集成腿 CI skip 面**：GH windows runner SyncRoot 注册失败（0x80070490）自动跳过 → CI 实际守护只剩单元腿（T-25③ 已写明；真机全跑属本地/人工）。
- [ ] **Windows 专属注入**（T-14 共享冲突、T-09 句柄释放验证）在 Linux CI 上不可运行：`#[cfg(windows)]` 门控，skip 面由 T-25 显式化，避免"三平台绿"误读为全量覆盖。
- [ ] **main.rs 可测性重构边界**：F-05/F-12/F-13 的单元代理依赖 main Result 边界提取（架构修订声明）；若不允许，降级为进程级黑盒进人工清单（覆盖变弱）。
- [ ] **desktop 组装可提取性**（T-28）：login_auth 的 policy 构造若不能提取为纯函数，则该点降级人工清单（GUI 面）。
- [ ] **F-04 日志捕获依赖**：假定 cookie.rs 全部为 tracing 宏（含 `tracing::debug!`）；若存在 log crate 桥接或 println 残留，需补 bridge 断言——Construction 时先核实。
- [ ] **无 Mac 本地环境**：macOS watcher/挂载行为仅 CI runner 覆盖，跨平台行为回归存在盲区，记录为环境性缺口。
- [ ] **P2 缺陷明确不设测试点**（webdav `/api/*` 伪造数据、非常数时间 Basic 比较、init_upload 重复建 Agent、限速、verify_upload 校验过宽等）：随 P2 intent 另行立项，避免范围蔓延。
