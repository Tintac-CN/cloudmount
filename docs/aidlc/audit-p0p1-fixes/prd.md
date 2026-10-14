# PRD: audit-p0p1-fixes

> 产出者：pm　日期：2026-10-14（Gate#1 打回修订轮）　来源：intent.md + audit-findings.md
> 修订依据：reviews/gate1-ai.md（互审点名：B1/MA2/MA3/MA9/MA10/MA11/m4/m7/m8/m13/m16 + 开放问题补全）+ decisions.md（Gate#1 人类裁决与编排者代行裁决）。
> 修订规则：AC1–AC15 编号保持稳定（内容修订不重排），新增项自 U16/AC16 续编；新增断言溯源标注"Gate#1 裁决/互审发现"。

## 用户故事

| # | 角色 | 目标 | 价值 | 溯源（intent.md/audit-findings.md） |
|---|---|---|---|---|
| U1 | 桌面挂载用户 | 读取大文件占位符（如 4GB 视频）的任意字节时，水合按有界分块传输，进程不因请求范围大小分配等量内存 | 大文件读取不 OOM，空闲 RSS 5-15MB 目标不回退 | intent.md 问题清单"内存安全"；F-01 |
| U2 | `--channel auto` 同步用户 | change_probe 快速路径经 ProviderRouter 透传到活跃通道 | 同步周期不被全量列表拖慢（100 目录 @1r/s 不再退化为每周期 100+ 秒） | F-02 |
| U3 | WebDAV 边缘模式用户/安全方 | 静态文件服务拒绝一切逃逸文档根的路径（盘符、UNC、`..`、反斜杠变体） | 边缘暴露面不可读任意盘文件 | F-03（双代理一致，安全级） |
| U4 | 全体用户/安全方 | 任何日志级别与错误输出中，userkey、OSS STS 凭据、Cookie 字段值不落入日志/错误产物 | 日志与错误消息可安全归档/导出（export_logs 面收窄） | F-04（含 Gate#1 裁决扩围，见 AC4 范围注） |
| U5 | Cookie 通道用户 | daemon 不再硬编码绕过"默认关闭、需知情同意"的产品开关 | 产品合规边界在所有组装路径一致生效 | F-05 |
| U6 | Windows 实时同步用户 | 引擎重建时旧 RdcWatcher 的线程与目录句柄被真正回收 | 长时间运行不泄漏线程/句柄 | F-06 |
| U7 | 弱网同步用户 | 远端目录创建不持有引擎全局锁做网络 IO | 单目录创建的网络往返不阻塞其余事件 | F-07 |
| U8 | WebDAV 写入用户 | PUT 完成后 spool 临时文件被清理，一次 PUT 恰好触发一次上传 | Windows 下不残留 spool 堆积、不重复上传 | F-08 |
| U9 | WebDAV 写入用户 | 写打开仅在 NotFound 时走新建路径，权限/IO 错误如实上抛 | 错误不被掩盖成静默新建，避免数据误写 | F-09 |
| U10 | OpenAPI 通道用户 | access_token 刷新单飞：并发调用只触发一次网络刷新并共享结果 | 不触发 115 侧 40140117 互踩失效 | F-10 |
| U11 | Cookie 通道上传用户 | OSS PUT 非 2xx 返回携带状态码的错误，失败上传不被当成功 | 消除静默数据丢失风险 | F-11 |
| U12 | Windows 服务用户 | SCM 服务运行路径与前台路径对 placeholder_mount 及启动参数（--interval）行为一致 | 服务方式安装后占位符挂载生效、同步周期如安装时所设，行为可预期 | F-12（--interval 落点：互审 MA9） |
| U13 | daemon 用户 | 配置文件存在但解析失败时进程拒绝启动并指明原因 | "配置生效"与"实际生效"一致，不静默回退 | F-13 |
| U14 | 开发者/维护者 | cookie.rs 全链路具备 HTTP 级回归测试安全网（含 initupload 成功回环） | F-04/F-11 修复有保护，后续改动不回退 | F-14（ECDH 回环：Gate#1 人类裁决纳入） |
| U15 | 项目全体 | 全部修复后全量回归通过、CI 三平台绿（含 mock feature 门控与 windows-only 清单语义） | v0.1.0 后质量基线恢复 | intent.md"期望的结果"与"约束" |
| U16 | desktop 壳用户 | 扫码登录成功后把同意标志（enabled+consent_acknowledged 双真）写回 config.toml `[cookie]` 段；点击登录时构造 CookiePolicy（enabled=true 恒真、consent_acknowledged=勾选状态；勾选框初值取配置），不再硬编码双真 | desktop 与 daemon/CLI 组装口径一致，"默认关闭+显式开启+知情同意"开关在所有组装路径一致生效（U5 语义完整成立） | F-05 扩围；Gate#1 人类裁决纳入（decisions.md 2026-10-14；互审 MA2） |

## 验收标准（可测）

> **T 点映射纪律（互审 B1/MA5，本修订轮新增）**：以下每条 AC（含 AC15、AC16 及各 AC 内新增分支）均须在 test-strategy.md 测试点清单中有 T 点映射落点（映射列回填为 AC-x ↔ F-xx 双锚，qa 侧回填）；文中标注"须有 T 点映射"处为本次修订新增的断言分支，映射由 qa 侧落位。无 T 点落点的 AC 不得进入 Construction。

- **AC1**（U1，F-01）：Given 一个远端大文件（测试桩可用 ≥8MB 模拟）已投影为占位符且 Provider 桩按 RangeSpec 返回数据，When 通过 CfAPI 触发该文件任意子范围的水合，Then 水合传输路径在 Provider 返回数据流上的**单次读取长度**均不超过分块上限常量（实现值 1 MiB——architecture 关键决策 #2；本 AC 上限 ≤4MB），返回字节与 Provider 提供内容逐字节一致，且全路径（含 FETCH_DATA 处理）不存在与请求总范围等大的单次连续缓冲分配（**断言锚 = reader 级读取序列**：桩 Reader 记录每次 `read` 返回的长度序列、断言呈分块上限等长循环；勿按 open_range 的 RangeSpec 断言——修复前后都只调用一次，无判别力。互审 m1 断言口径统一）。
- **AC2**（U2，F-02）：Given `--channel auto` 且活跃通道为记录调用的桩通道，When 经 ProviderRouter（以 trait 对象）调用 `change_probe`，Then 桩通道的 `change_probe` 恰被调用一次且返回值透传（`Some` 探测结果与 `None` 均原样透传，`None` 由引擎回退全量扫描）；Given 活跃通道对 `change_probe` 返回认证失效错误（`Error::Auth`），Then 走与 `call()` 其它方法一致的通道切换处理（切至另一可用通道并以该通道重试一次，不原样透传认证错误、不静默吞成 `None`）；Given 活跃通道返回非认证类 Err，Then 错误原样透传、不触发切换（互审 m7：认证失效与非认证类两分支均须有 T 点映射，qa 侧补）。
- **AC3**（U3，F-03）：Given serve_static 以目录 `root` 为文档根，When 请求 `/C:/Windows/win.ini`（盘符绝对路径）、`//server/share/f`（UNC）、`/../../secret`、`/a/b/../../../secret` 及反斜杠变体中的任一项，Then 响应为 403/404 且响应体不含目标文件内容；Given `root` 下存在 `index.html` 与 `assets/app.js`，When 请求 `/`、`/index.html`、`/assets/app.js`，Then 返回 200 且内容正确；且对任一放行请求，最终读取路径的规范形式满足 `canonicalize().starts_with(root)`（可对路径解析函数做单元断言）。
- **AC4**（U4，F-04）：Given tracing 过滤开至 `trace` 级（全开，含 debug/info 事件）并捕获全部日志输出，且桩服务返回已知敏感值（userkey=`USERKEY-SECRET-123`、STS secret/security_token 为已知桩值），When 依次执行 uploadinfo、initupload、gettoken 全流程，Then 捕获的日志中不出现任一敏感桩值的明文或可还原片段（逐值断言缺席）；日志允许出现字段名、长度或前缀哈希等脱敏元数据。**范围注（Gate#1 编排者代行裁决扩围，同安全不变量 #2；互审 m13/m18）**：F-04 施工面在 cookie.rs 三处（:401/:492/:560）之外并入两处同类脱敏——① `cloudmount-cli/src/main.rs:233` 登录失败错误输出对原始 cookie 串的回显：Given 桩 cookie 含金丝雀 UID/CID/SEID 值且缺少必需字段触发报错，Then 错误消息不含任一金丝雀值（允许字段名/长度）；② cookie.rs get_info 处原始 JSON 的 debug 日志脱敏（评审基线行号 cookie.rs:796——架构决策 #15 已核实更正文件归属，互审原记 webdav.rs:796 属笔误；非凭据、同类原样打印）。两处均须有 T 点映射（qa 侧落位）。
- **AC5**（U5，F-05）：Given Cookie 通道策略为默认（enabled=false 或 consent_acknowledged=false）且用户以 `--channel cookie` 启动 daemon，When 组装 Provider，Then 组装返回 Err 且错误信息指明需显式开启并确认同意（单元代理：提取 main 组装边界函数断言 Err 链，须有 T 点映射——互审 m16；进程级非零退出码黑盒验证列入人工清单）；Given enabled=true 且 consent_acknowledged=true，When 以 `--channel cookie` 启动，Then Cookie 通道可用且保留既有协议风险 warn 日志；Given `--channel auto` 且 `[cookie]` 未双真，When 组装 Provider，Then 静默跳过 Cookie 兜底、不构造 Cookie115Provider、以纯官方（Open）通道组装运行（auto 通道断言——互审 MA11，须有 T 点映射，qa 侧落位）。拒绝语义（硬拒绝 vs warn 后回退 open 通道）仍为开放问题，本 AC 以硬拒绝为当前默认。
- **AC6**（U6，F-06）：Given 一个运行中的 RdcWatcher，When drop 该实例，Then 后台观察线程在有限超时（如 5s）内退出、目录句柄被关闭（Windows 上以 drop 后可删除被监视目录二次验证句柄释放）；循环"创建+drop"50–100 轮（断言下限 50 轮——互审 m4 数值口径统一）后，进程活跃线程/句柄计数相对基线无净增长（以代码内启动/退出插桩计数 hook 轮询至基线断言，而非单次读取；OS 级计数或插桩计数器，取测试稳定者）。
- **AC7**（U7，F-07）：Given 桩 Provider 的 `list`/`create_dir` 各延迟 500ms，When 目录 X（未缓存）触发 ensure_remote_dir，同时并发地（a）对另一未缓存目录 Z（不同前缀）触发 ensure_remote_dir（主判据场景：时间窗交叠）；（b）对已缓存目录 Y 查询目录 ID（副判据场景：提前返回），Then 主判据（确定性）：桩侧记录每次 provider.list/create_dir 调用的起止时间窗，断言存在时间窗**交叠**（不同前缀两目录并发 ensure_remote_dir；交叠 ⇒ dir_ids 锁未跨网络持有；现状整段持锁则严格串行无交叠，红测判别成立）；副判据：Y 的查询在 X 的网络往返完成前即返回（完成时间 < 两操作串行和的 60%，重复多轮降 flaky——互审 m4 数值口径统一）。
- **AC8**（U8，F-08）：Given 一次 WebDAV PUT 走完 flush 且上传成功，When flush 返回并释放文件句柄后，Then temp 目录中该 PUT 的 `cm-dav-put-*` spool 文件不存在；且该 PUT 完整生命周期内 `provider.upload` 恰被调用一次（桩计数断言，排除 Drop 二次 flush 重复上传）；Given 删除 spool 时因 Windows 共享冲突失败，Then 记录 warn 日志且不视为上传失败（删除失败 = warn + **接受残留**、无重试机制——与 architecture F-08/T-14 统一口径）。
- **AC9**（U9，F-09）：Given 目标文件不存在且父目录可解析，When WebDAV 写打开，Then 走合成新条目路径且 PUT 成功；Given resolve 因权限或 IO 错误（非 NotFound）失败，When 写打开，Then 返回相应错误（非静默新建），且 `provider.upload` 未被调用（桩计数为 0）。
- **AC10**（U10，F-10）：Given access_token 已临近过期、可注入的桩 transport 对 refreshToken 端点计数，When 10 个线程在过期点会合后并发调用 `access_token()`（数值口径统一为 10 线程——互审 m4），Then 网络刷新请求恰好发出 1 次（桩计数为 1）且 10 个调用全部获得同一新 token；Given 桩 transport 对刷新返回失败，When 并发调用，Then 所有调用者收到一致的错误（错误不被吞掉），修复后的下次调用可重试。
- **AC11**（U11，F-11）：Given 桩 OSS 端点对 PUT 返回 403 与错误体，When oss_put 上传，Then 返回 Err 且错误信息包含 HTTP 状态码与响应体摘要，上层上传结果为失败（可进入既有重试处理）；Given 桩端点返回 2xx，Then oss_put 返回 Ok 且既有 callback 逻辑不回退。
- **AC12**（U12，F-12）：Given 以 Windows 服务 SCM Run 方式启动且 placeholder_mount=true，When 服务路径构造同步配置与挂载，Then 与前台运行 placeholder_mount=true 走同一构造分支（占位符挂载生效；以提取公共构造函数后的单元断言两路径产出的配置一致）；Given placeholder_mount=false，Then 两路径同为非占位符行为；Given install 时指定 `--interval N`（N 为非默认值），When 构造服务启动参数（`build_launch_args`），Then 产出参数包含 `--interval` 且值为 N（单元快照断言——修复"install 不烘焙 --interval、服务模式恒默认 30s"的潜伏 bug，互审 MA9；须有 T 点映射，qa 侧落位）；且 `crates/cloudmount-daemon/src/service.rs` 源码中 `define_windows_service!` 宏调用与 `service_main` 函数不复存在（结构性断言：源码符号检索零命中；废弃原"dead_code 无告警"判据——现状被 `#[allow(dead_code)]` 压制、判据空转，互审 MA10；处置采架构决策 #3"保留在用部分+删除死 FFI 入口"，Gate#1 已裁决，见开放问题已裁决节）。
- **AC13**（U13，F-13）：Given 配置文件存在但内容非法（非 TOML），When daemon 启动加载配置，Then 加载以 Err 上抛且错误链包含子串「解析失败」与该文件路径（对齐 `config::load_from` 实际消息"配置文件解析失败 {path}"，config.rs:106——互审 m8 断言子串统一；单元代理：提取 main Result 边界断言 Err 链，须有 T 点映射——互审 m16，进程级非零退出码/stderr 黑盒验证列入人工清单）；Given 配置文件不存在，When daemon 启动，Then 以内置默认值正常启动（沿用 `config::load_from` 既有语义，首启不受影响）。文件存在但不可读（权限/IO 错误）分支语义见开放问题（未裁决，当前默认同样 fail-fast）。
- **AC14**（U14，F-14）：Given 本地测试服务器（测试位于 tests/ 集成层）与经 `with_base_urls` 纯加法扩展注入的上传端点基址（uplb/OSS 注入字段存完整基址、含 scheme——互审 m14/MA3④，Gate#1 裁决统一注入面），When 运行 `cargo test -p cloudmount-driver115` 的 cookie 通道 HTTP 级套件，Then 用例矩阵至少覆盖：uploadinfo 成功与 state=false、initupload 成功回环（测试侧实现 P-224 ECDH 加密对端——AES-CBC + LZ4，p224/aes/lz4_flex 已是正式依赖、无新供应链；ec115 测试辅助形态由架构定；Gate#1 人类裁决纳入本期）与 initupload 错误返回（传输错误/响应不可解密）、gettoken 成功与 status_code!=200、oss_put 2xx 与非 2xx（联动 AC11），每格至少 1 例且全绿（initupload 成功回环须有 T 点映射，qa 侧落位）。
- **AC15**（U15，溯源 intent.md 约束）：Given AC1–AC14 与 AC16（其余全部验收标准）合入，When 在 Windows/Linux/macOS CI 运行全工作区构建与测试，Then 全绿；且 CI 矩阵须含带 mock feature 的测试 job，并明示 windows-only 用例清单与自动跳过语义（"三平台绿"不得误读为全量覆盖——互审 B1/MA8；CI 矩阵 + features + skip 语义须有 T 点映射，qa 侧落位）。内存纪律验收拆分（互审 B1）：①自动化部分锚定 AC1 的分块上界断言（水合内存峰值与请求总范围大小无关，由 AC1 的块长上界与请求大小序列断言代理，不另设内存基准自动化判据）；②空闲 RSS 部分显式列为**人工清单判据**：release 构建的 daemon 启动并静置 30 秒后，进程 RSS ≤ 15MB（Windows 与 Linux 各执行一次，以任务管理器/ps 读取；CI 无 RSS 测量基建，不设自动化断言）。
- **AC16**（U16，F-05 扩围，Gate#1 人类裁决纳入；复审 B-R2-1 后按人类裁决"点击即同意"统一口径整条重写）：**分支 1（写回）**——Given desktop 登录页勾选同意并点击登录、扫码成功（登录挑战轮询返回成功会话），Then 登录成功回调经 cloudmount-core 写回函数将 `[cookie]` 双真（enabled=true 且 consent_acknowledged=true）写入 config.toml、供 daemon/CLI 消费（"点击即同意即持久化"闭环；写回函数为 core 纯函数——如 `set_cookie_consent(&mut Config)`，配合既有 load_from/save_to 路径注入变体——以路径注入做单元断言，T-27 落点；desktop 侧接线（登录成功回调调用该函数）显式归人工清单⑦，本 AC 不要求 desktop crate 内自动化测试——M-R2-2 收敛；GUI 交互验证列入人工清单）；**分支 2（构造）**——CookiePolicy 在**点击登录时构造**：enabled=true 恒真（显式点击即开启）、consent_acknowledged=勾选框状态，勾选框初值取配置（配置 `[cookie]` 双真则预勾选），首次登录零障碍；不再采"CookiePolicy 从该配置构造/配置缺省或未双真则通道不启用"旧口径（该口径与 `check_enabled` 门控组合会使全新安装无法完成首次登录、"登录成功写回"闭环不可达——复审 B-R2-1 死锁指认）；**分支 3（组装路径豁免口径）**——desktop 引擎/组装路径（build_channel_provider/try_cookie_provider/engine_loop 同构面）不构造 CookiePolicy 双真：硬编码 `CookiePolicy { enabled: true, consent_acknowledged: true }` 的结构性断言**收窄为组装路径**（grep 式断言：apps/desktop 源码中该字面量构造仅允许出现在登录会话构造点、且该处有注释说明"点击即同意"），登录会话路径豁免（互审 MA2 指认的 apps/desktop/src/main.rs:143-149 属组装路径硬编码，仍在断言打击面内）；desktop 引擎/组装路径口径与 daemon/CLI 一致，U5"所有组装路径一致生效"完整成立。desktop 登录交互（扫码）是同意的产生点、config.toml `[cookie]` 段是同意的唯一持久化位置（安全不变量 #4）；未同意状态下登录入口的具体交互设计由架构修订定，本 PRD 不规定 UX。本条（含各分支）须有 T 点映射，qa 侧落位（结构性断言的 T 点落点随 B-R2-1 收敛——复审 min-17：T-28 或新增 T-30 grep 式断言）。

## 非目标（明确排除）

- **全部 P2 项（约 26-37 条）延后**到后续 intent，另行立项清点；已知名样例均不在本 intent 范围：webdav `/api/*` 硬编码伪造数据、Basic 认证非常数时间比较、`init_upload` 每次新建 `ureq::Agent`、cookie 通道 0.5 r/s 限速参数、verify_upload sha1-OR-name 回退校验过宽等（完整清单见 audit-findings.md P2 节）。
- **受阻项**：M5 macOS FileProvider、minifilter 驱动、边缘 48h 浸入测试——与缺陷修复无关，不进入本 intent。
- 不做审计之外的重构与特性：不调整 crate 边界、不更换日志/HTTP 框架、不改对外协议与 CLI 语义（缺陷本身即错误行为的除外，如 F-13 的退出码、F-05 的拒绝启动；另含 Gate#1 裁决纳入的两处扩围——desktop 壳 Cookie 开关回接（U16/AC16）与编排者代行裁决并入 F-04 的两处同类脱敏（AC4 范围注），均为裁决落点而非新需求）。
- 修复过程中新发现的缺陷不顺手修：一律记录并交编排者裁决是否升格 P0/P1 或归入 P2（互审新发现 m13/m18 即按此流程经编排者裁决并入 F-04 施工面，为已裁决例外）。
- desktop 壳仅纳入 Cookie 开关回接（U16/AC16）这一裁决落点：不改 desktop 的引擎通道选择、GUI 框架及其它行为（apps/desktop 其余部分仍不在触及面）。

## 开放问题（需人类裁决）

### 已裁决（Gate#1，2026-10-14，逐字记录于 decisions.md）

- [x] **F-14 initupload 成功回环（ECDH 测试对端）**：裁决**纳入本期**。AC14 验收范围含 initupload 成功回环（测试侧 P-224 ECDH 加密对端 + ec115 测试辅助；p224/aes/lz4_flex 已是正式依赖，无新供应链）；测试注入面统一为 tests/ 集成层 + `with_base_urls` 纯加法扩展（含完整基址含 scheme）。原"未裁决前以错误路径为基线"的加注已删除。
- [x] **desktop 壳范围**（apps/desktop 硬编码 CookiePolicy 双真、GUI 登录不写 `[cookie]` 段，互审 MA2）：裁决**纳入本 intent**。新增 U16/AC16（登录成功写回 `[cookie]` 段 + 从配置构造 CookiePolicy）；U5"所有组装路径一致生效"完整成立。
- [x] **F-12 service.rs 处置**：按互审确认采架构决策 #3——**保留在用部分 + 删除死 FFI 入口**（`define_windows_service!`/`service_main`）；AC12 验收改为结构性断言（原"dead_code 无告警"判据废弃）。
- [x] **范围确认（沿 intent 开放问题）**：维持 P0+P1（F-01…F-14）+ Gate#1 裁决纳入的 desktop 壳扩围；互审新发现 m13/m18 经编排者代行裁决并入 F-04 施工面（同安全不变量 #2，Gate#1 复审追认项）；P2 延后不变。

### 未裁决（保留，需人类/后续 Gate 裁决）

- [ ] **F-05 拒绝语义**：Cookie 通道"未启用/未同意"时，daemon 是硬拒绝（组装 Err → 进程非零码退出，AC5 当前默认），还是 warn 后回退 open 通道继续运行？审计只要求"回接配置开关与同意标志校验"，两种语义均可成立。
- [ ] **F-13 不可读分支**：解析失败 fail-fast 已定；请确认配置文件**存在但不可读**（权限/IO 错误）是否同样 fail-fast（AC13 当前默认：是，只有"文件不存在"走默认值）。
- [ ] **F-05 升级行为/同意存储位置 A/B**：已登录 Cookie 但配置无 `[cookie]` 段的用户升级后通道回到默认关闭（需重新执行登录或手动补配置）——是否接受该可感知行为变化；同意标志落 config.toml（方案 A，当前默认）还是独立 marker 文件（方案 B）。另记互审 m12：新版本 CLI/desktop 写入 `[cookie]` 段后，**旧版** daemon/desktop（deny_unknown_fields）读同份配置将解析失败（旧 daemon 叠加 F-13 静默回退），降级/回滚窗口行为请一并确认。
