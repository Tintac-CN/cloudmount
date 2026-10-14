# Gate#1 AI 互审 · 复审轮 r2（2026-10-14）

> 评审对象：三工件 v2（prd @ b142982 / architecture @ bab7f7e / test-strategy @ 4b60534）
> 评审者：reviewer-pm（0/2/6）、reviewer-architect（1/1/4）、reviewer-qa（0/2/9）、reviewer-dev（0/3/3）——只读交叉评审，全部对照仓库源码逐项核验。
> 编排者去重合并：**1 BLOCKER / 3 MAJOR / 15 MINOR**。
> **总体结论：第 1 轮全部发现（2 BLOCKER / 11 MAJOR / 18 MINOR）经源码级核验确认真实消解**（B2 红测向量链路、MA1 双入口单飞、MA3 ECDH test_peer 协议逆运算、MA4 退出链归一均逐一验证成立）；本轮新发现全部为措辞级/声明级收敛问题，无方向性缺陷。reviewer-architect 与 reviewer-dev 均明确："修完即可放行 Construction，无需第三轮全量互审"。

## 第 1 轮消解核对（摘要）

- **B1 消解**：AC15 拆分（自动化锚 AC1 + RSS 人工判据四要素齐备：release 静置 30s ≤15MB、Win/Linux、任务管理器/ps）；T-25 三要素（mock-feature job / windows-only 清单 / cfapi skip 面）；AC1–AC16 每条有 T 点（逐条走查通过，计数 P0=9/P1=19/P2=1 准确）。
- **B2 消解**：T-05 向量链路源码逐点验证（/dav/* → ProviderFs webdav.rs:805-812；web_ui_dir=None → 404 :819-823；盘符主向量修复前必红——join 绝对成分替换基路径；start_server_with_ui 第 4 参扩展可行；金丝雀位置正确）。
- **MA1 消解**：refresh_locked 无重入（do_refresh 只用 self.agent、persist 规避）；client.rs:244 改调 refresh_stale 可行（Arc<OpenApiAuth> 具体类型）；身份双检语义正确。
- **MA2 主体消解**（遗留口径问题见 B-R2-1）；**MA3 消解**（with_base_urls 零调用方、:605 默认渲染逐字节等价、test_peer 四条理由对源码逐一成立、协议逆运算可实现）；**MA4 消解**（三个退出出口可归一置 exited；超时 detach + warn = 可观测泄漏）；**MA5–MA11 消解**；18 MINOR 全部落位（含 m18 文件归属修正经源码核实架构方正确：cookie.rs:796 确为 get_info 日志，互审原记 webdav.rs:796 属笔误）。

## BLOCKER

- **B-R2-1**［arch BLOCKER-1 + dev MA-B］**desktop CookiePolicy 构造口径三方矛盾，字面实现致功能死锁**。架构设计节正文：点击登录时构造 `{enabled:true, consent_acknowledged:true}`（点击即同意），配置只驱动勾选框初值；决策 #14 括号却写"从配置构造 policy"；prd AC16 分支 2/3 写"CookiePolicy 从该配置构造 / 配置缺省则通道不启用"；T-28 写"policy 构造改读 config、缺段 → {false,false}"。**功能后果（源码证实）**：全新安装无 [cookie] 段 → policy {false,false} → `CookieAuth::begin_login` 首行 `check_enabled()?` 即拒绝（auth/cookie.rs:279-280，:519 既有测试佐证）→ desktop 永远无法扫码登录 → AC16"登录成功写回 [cookie]"闭环不可达（鸡生蛋死锁）。且 AC16 末句结构性断言（"不复存在硬编码 CookiePolicy{true,true} 构造"）会红掉按架构正文合规书写的实现。**修复方向**：统一为"配置仅驱动勾选框初值与写回；policy 在显式点击时构造（consent_acknowledged 取勾选状态）"；相应修订决策 #14 括号、AC16 分支 2/3、T-28；结构性断言收窄为"引擎/组装路径（build_channel_provider/try_cookie_provider/engine_loop）不复存在硬编码，登录会话路径豁免（点击即同意）"。附：人类裁决原文"从配置构造 CookiePolicy"本身有歧义，复审时向人类确认统一口径（见 AskUserQuestion）。

## MAJOR

- **M-R2-1**［pm MAJOR-2 + qa MA-A + dev MA-A］**T-11/AC7 主判据的"锁观测 seam"未在架构声明**。T-11 写"采样 exec.rs 锁持有观测 seam（架构修订声明）"、AC7 写"锁状态的时序记录"，但 architecture.md F-07 节无任何锁观测 hook 设计（MA6 落实摘要只含 pump_chunks 与 ACTIVE_WATCHERS）；exec.rs:46 `dir_ids` 为私有字段，集成测试无法 try_lock 观测。属第 1 轮 MA6 同类残留 + "strategy 引用不存在的架构声明"。**修复**：架构补 seam 声明（如 Executor 内测试可见持锁标志/计数，对照 ACTIVE_WATCHERS 先例），或 qa 改纯测试侧判据（双操作时间戳交叠证明），二者取一并使两文档一致。
- **M-R2-2**［arch MAJOR-1 + qa MA-B + pm MAJOR-1 + dev MA-C］**desktop 写回自动化测试归宿三文档互斥**。prd AC16："写回逻辑提取为可测函数、config 路径注入单元断言"；architecture desktop 节："对已测 pub API 的三行调用——不新增自动化测试"；T-27 按"写回函数"书写单元测。且 dev 指出更深一层：desktop 被 workspace exclude（根 Cargo.toml:13），CI `cargo test --workspace` 永不触达 desktop 测试——测试放 desktop crate 则 CI 不可见（B1/MA8 同类"空绿"在新裁决面重演），放 cloudmount-core 则只复测 core 往返、对"desktop 调用了写回"零判别力。**修复**：三选一收敛并三文档同步——①提取 helper 落 cloudmount-core（`load_from`/`save_to` 路径注入变体已存在）+ core 单测（守护 `[cookie]` 写回语义）+ AC16 措辞改"helper 单测 + 接线归人工清单⑦"；②T-27 重定位为 core serde 往返 + AC16 写回分支全归人工清单；③CI 为 desktop 增设 test job。
- **M-R2-3**［dev MA-C + qa min-1 + dev m-B］**windows-only 编译面与清单缺口**。`pub mod rdc` 整体 `#[cfg(windows)]`（cloudmount-sync/lib.rs:43-44）→ T-09 主腿本身 Linux 不可编译，T-25② 清单只列了句柄子腿；`build_launch_args` 为 `#[cfg(windows)]`（daemon main.rs:293-294）→ T-29（及 T-21 提取物）在 Linux CI **编译失败**（比 skip 更糟）。**修复**：build_launch_args 提取为跨平台纯函数（参数组装与 windows 执行分离）或测试门控；T-25② 清单补全（T-09 全腿、T-29/T-21 门控语义）。

## MINOR

1. test-strategy.md:7 头部"prd/architecture 磁盘版本为修订前快照"注记已失效（三 v2 已对齐），删除防 Construction 误用基线。［pm MIN-1 + arch MIN-4］
2. architecture.md 开放问题"F-12 service.rs 处置"仍标未决 `[ ]`——prd/decisions 已按裁决 #3 收敛，同步为已裁决口径。［pm MIN-2］
3. prd AC8 失败分支"延迟清理不阻塞响应"措辞残留——统一为"warn + 接受残留、无重试机制"（architecture/T-14 已统一）。［pm MIN-3 + arch MIN-2］
4. T-29 负分支"不烘焙或烘焙默认，快照固化"与架构 F-12"无条件烘焙"不一致，钉死"烘焙默认值"。［pm MIN-4］
5. T-01"断言呈 1 MiB 等长循环"末块短读会假红——改"除末块外每次读取恒为块上限（全部 ≤ 上限）"。［pm MIN-5］
6. AC2 的 None 透传分支无显式 T 断言——T-03 Stub 返回值配置一行覆盖。［pm MIN-6］
7. prd AC4 范围注②仍写"webdav.rs:796"——架构决策 #15/T-07 已更正为 cookie.rs:796（源码核实），PRD 同步。［arch MIN-1］
8. desktop 写回 `if let Ok(mut cfg) = config::load()` 在 load 失败时静默跳过（沿设置页既有模式）——补一条 warn 或人工清单注明"同意记录丢失无信号"。［arch MIN-3］
9. AC15 RSS 人工判据要求 Linux 各一次，E2E 人工清单①只覆盖 Windows 场景——补 Linux 静置 RSS 清单行。［qa min-2］
10. T-05 金丝雀向量过冲：`/..%2F..%2Fcanary-secret.txt` 解码后落在 tempdir 之上、够不到金丝雀（修复前后均拦，无判别力）——改单层 `..%2F` 或删；并注明"编码向量（/C%3A/...）= 修复后回归面（现状不解码、修复前后均 404），红测力集中在裸冒号主向量"。［qa min-3 + dev m-C］
11. T-14 注入规格歧义："upload 回调内打开无共享句柄"须**持有到 upload 返回之后** remove_file 才失败——写明句柄保持时长。［qa min-4］
12. T-10 超时分支无确定性造红手段（本地目录 CloseHandle 实践必醒）——注明该分支为容错断言/人工验证（P2 点）。［qa min-5］
13. T-11 延迟值"300-500ms"与 AC7"各延迟 500ms"漂移——钉死一致。［qa min-6］
14. T-08 保留缺口误引"架构修订声明"（F-05 节无 main 边界提取声明）——锚改 `CookiePolicy::check_enabled`（pub，cookie.rs:72，拒绝消息已核实）或架构补 F-05 提取声明。［qa min-7］
15. sign_check（status=7）二次区间分支：架构 F-14 覆盖矩阵承诺了但 T-24/AC14 未安排用例（cookie.rs:928-950 循环真实存在）——T-24 补用例或架构声明延后。［qa min-8］
16. ACTIVE_WATCHERS 下溢隐患：scan_tree 失败时 new() 返回 Err 但线程退出仍会 -1（无对应 +1）→ usize 下溢，恰污染 T-09 基线——写明"spawn 后立即 +1"或减一用 saturating_sub。［dev m-A］
17. AC16 结构性断言无 T 点落点（对照 AC12 同类断言落 T-21 的先例）——随 B-R2-1/M-R2-2 收敛一并落点。［qa min-9，并入 BLOCKER/MAJOR 修复］

## 结论

**1 BLOCKER / 3 MAJOR / 15 MINOR**（17 条去重后）。上轮全部发现真实消解、源码引用零失实；本轮新发现均为一句话级对齐问题（desktop 口径统一、seam 声明、测试归宿收敛、清单补全），不动设计主体。两个评审角色明确建议修完即可放行、无需第三轮全量互审。决策点：①Gate 审批（打回微调 vs 通过+前置任务）；②desktop CookiePolicy 统一口径需人类确认（裁决原文歧义）。
