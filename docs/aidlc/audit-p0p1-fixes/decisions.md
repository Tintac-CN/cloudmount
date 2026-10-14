# Decisions: audit-p0p1-fixes

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-14 编排者开场记录：
- 意图来源：`/aidlc:inception 请按照审计报告开始修复`（逐字）。
- 审计输入由编排者将双代理（reviewer-architect / reviewer-dev）回报逐字归档为 `audit-findings.md`。
- 范围默认裁决（编排者代行，Gate#1 人类可推翻）：本 intent 只修 P0+P1；P2 延后。
- 角色调度说明：pm/architect/qa 角色工具集不含 Bash，工件提交由编排者按 §1 工件链代行（落盘→git add→commit→回报）。

---

2026-10-14 对齐拍记录（编排者）：
- 三工件齐后比对：F-01…F-14 ↔ prd AC1–AC14 一一对应（AC15 为 intent 约束项）；qa 测试点 T-01…T-24 全部以 F-xx 锚定、无虚构编号；architect 语义对齐表 14 行与 PRD 动词语义一致。
- 发现错位 1 处：AC14 要求"initupload 成功"用例，而 architect 核实 initupload 响应经 ec115 加密、mock 成功需测试侧 ECDH 对端（architecture.md 开放问题 #5）。已派回 pm 只做引用回填：AC14 加注，未裁决前以 initupload 错误路径（传输错误/响应不可解密）为验收基线，成功回环待 Gate#1 裁决后生效或移除。
- 编排者代行裁决备案（Gate#1 人类可推翻）：
  1. architect 对 F-12 的审计修正（service.rs 仅 FFI 入口为死代码、在用部分保留）与 AC12"dead_code 无告警"验收兼容，采 architecture.md 关键决策 #3。
  2. F-13 以 AC13 断言措辞（输出含"配置解析失败"+文件路径）为准绳；architecture.md 中的示例文案非验收字符串。
  3. 范围默认 P0+P1（F-01…F-14）、P2 延后；交付方式单 intent 连续施工（沿 intent 开放问题，亦见 prd 开放问题 #4）。
- 需人类裁决的开放问题汇入 Gate#1：prd 4 项 + architecture 6 项（其中 F-13 fail-fast 语义、F-05 升级行为、service.rs 处置三项两工件重叠，合并裁决）。

---

2026-10-14 Gate#1 人类审批（AskUserQuestion，逐字转写）：
- 问：「Gate#1 互审发现 2 BLOCKER / 11 MAJOR / 18 MINOR（基线可信，问题集中在测试规格与验收口径）。如何裁决？」
  答：「打回修订（推荐）」
- 问：「desktop 壳（apps/desktop/src/main.rs:143-149）同样硬编码 CookiePolicy 双真且 GUI 登录不写 [cookie] 段，与 U5"所有组装路径一致生效"矛盾。如何处置？」
  答：「纳入本 intent（推荐）」
- 问：「initupload 响应经 ec115 加密（P-224 ECDH + AES-CBC + LZ4），mock 一个"成功"响应需测试侧实现对端加密（p224/aes/lz4_flex 已是正式依赖，无新供应链）。是否纳入本期？」
  答：「纳入本期」
- 互审输入：`reviews/gate1-ai.md`（2 BLOCKER / 11 MAJOR / 18 MINOR，四角色只读交叉评审，源码抽验零失实）。
- 裁决落点（编排者执行）：
  1. gate1 打回，phase 回 inception；只重跑 pm/architect/qa 修订被点名发现（B1/B2 必修，MAJOR 按下列落点）；复审重点核对 B1/B2 消解。
  2. desktop 壳纳入本 intent：登录成功后写回 `[cookie]` 段 + 从配置构造 CookiePolicy（U5"所有组装路径一致生效"语义完整成立）。
  3. F-14 ECDH 加密对端纳入本期：initupload 成功回环 + T-24 保留；测试注入面统一为 tests/ 集成层 + `with_base_urls` 纯加法扩展（含完整基址含 scheme）+ ec115 测试辅助（形态由 architect 定）。
- 修订轮编排者代行裁决（复审追认项，Gate#1 复审时人类可推翻）：
  1. 互审新发现 cli main.rs:233 原始 cookie 回显进错误输出（m13）——与安全不变量 #2（凭据不出现在日志/错误面）同源，**升格纳入 F-04 范围**（一行脱敏，非新需求）。
  2. 互审新发现 webdav.rs:796 get_info 原始 JSON debug 日志（m18）——同类原样打印（非凭据），**并入 F-04 施工面**顺带脱敏。（architect 修订版经源码核实修正落点归属为 cookie.rs:796，互审记 webdav.rs:796 属文件归属笔误，发现本身属实。）
  3. 前次代行裁决 #2（AC13 断言"配置解析失败"）按互审 m8 修正：断言子串统一为「解析失败」+ 文件路径（load_from 实际消息为"配置文件解析失败 {path}"，config.rs:106）。
  4. AC/测试点数值口径统一（消 m4）：AC10/T-17 = 10 线程会合；AC6/T-09 = 50-100 轮循环（断言下限 50）；AC7 主判据 = 锁时序记录（确定性），副判据 = 完成时间 < 串行和 60%；AC4/T-07 = trace 级全开捕获。
  5. m16 进程级断言：接受单元代理（提取 main Result 边界断言 Err 链），进程级黑盒（退出码/输出）进人工清单。

GATE-REJECTED gate1 2026-10-14

---

2026-10-14 修订轮对齐拍记录（编排者）：
- 三个角色完成修订（pm/prd-v2、architect/architecture-v2、qa/test-strategy-v2；首轮派发被中断但工件已完整落盘，经核对无截断）。逐工件提交：prd-v2 @ b142982、architecture-v2 @ bab7f7e、test-strategy-v2 @ 4b60534。
- **B1/B2 消解核对**：B1 → AC15 拆分（自动化锚 AC1 + 空闲 RSS 显式人工判据：release 启动静置 30s 后 RSS ≤15MB，Windows/Linux 各一次）+ T-25（CI mock-feature job / windows-only 清单 / cfapi skip 面）+ 每 AC 有 T 点落点（映射纪律写入 prd）；B2 → T-05 重写（向量去 /dav 前缀、盘符主向量、harness 扩 start_server_with_ui + root 外金丝雀、百分号编码注、UNC 保留纵深）。
- **11 项 MAJOR 全部落位**：MA1 → refresh()/refresh_stale() 双入口同一持锁刷新助手（决策 #7，token 身份双检）；MA2 → desktop 设计节 + 决策 #14（零依赖调整）+ U16/AC16 + T-27/T-28；MA3 → F-14 统一（tests/ 集成 + with_base_urls 完整基址含 scheme + ec115.rs test_peer ECDH 对端，决策 #12）+ T-24 升格 P0；MA4 → join 5s 超时 + detach + warn（决策 #13）；MA5 → 映射列 AC-x（F-xx）双锚；MA6 → ACTIVE_WATCHERS AtomicUsize + pump_chunks seam；MA7 → T-15 ErrorInjectingProvider；MA8 → T-25 + CI job 设计（决策 #16）；MA9 → AC12 --interval 子句 + T-29；MA10 → AC12 结构性断言 + T-21；MA11 → AC5 auto 断言 + T-08。
- **18 项 MINOR 落位**（qa"修订落点对照"节逐条可查）；编排者遗漏派发 pm 的 m1（AC1 断言锚）在对齐拍检出，已派回 pm 最小回填（仅 AC1 一行改 reader 级锚 + 1 MiB 实现值标注），其余未动。
- 新计数：AC1–AC16 ↔ T-01…T-29 全部有映射落点；P0 = 9、P1 = 19、P2 = 1。
- 残留未裁决开放问题收敛为 3 项（prd：F-05 拒绝语义、F-13 不可读分支、F-05 升级行为/存储位置 A/B 含 m12 反向窗口）+ architecture 6 项（desktop 同意交互形态为新增）——Gate#1 复审时合并呈报。
- 相位转移：gate1 复位 pending（重审前复位，decisions.md 2026-10-14 GATE-REJECTED gate1 之后须新回执），phase → gate1。

---

2026-10-14 Gate#1 复审（r2）与人类裁决（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate1-ai-r2.md`（复审轮：上轮全部发现经源码级核验确认真实消解；新发现 1 BLOCKER / 3 MAJOR / 15 MINOR，均为措辞级收敛；reviewer-architect 与 reviewer-dev 明确"修完即可放行、无需第三轮全量互审"）。
- 问：「Gate#1 复审：上轮全部发现确认消解，新发现 1 BLOCKER / 3 MAJOR / 15 MINOR（均为措辞级收敛，四方评审建议修完即可放行）。如何裁决？」
  答：「打回微调（推荐）」
- 问：「BLOCKER 的根因是裁决原文"从配置构造 CookiePolicy"有歧义。desktop 侧统一口径采哪种？」
  答：「点击即同意（推荐）」
- 裁决落点（编排者执行）：
  1. gate1 第 2 次打回（gate1=rejected，phase 回 inception）；只重跑被点名收敛项（B-R2-1 口径统一 + 3 MAJOR + MINOR 清单），复审只核对新收敛点（轻量，不重跑全量四方互审）。
  2. **desktop 统一口径 = 点击即同意**：勾选框初值取配置；点击登录时构造 CookiePolicy（enabled=true 恒真、consent_acknowledged=勾选状态）→ 首次登录零障碍；登录成功写回 [cookie] 段供 daemon/CLI 消费。结构性断言收窄为"引擎/组装路径（build_channel_provider/try_cookie_provider/engine_loop）不复存在硬编码，登录会话路径豁免（勾选即同意）"。
- 收敛轮编排者代行裁决（Gate#1 第三次复审追认项）：
  1. M-R2-1（T-11/AC7 锁观测 seam）：采**纯测试侧判据**（零生产 seam）——SlowMockRemote 桩侧记录每次 list/create_dir 调用的起止时间窗，断言存在时间窗交叠（不同前缀的两目录并发 ensure_remote_dir；交叠 ⇒ 锁未跨网络持有；现状整段持锁则严格串行、无交叠，红测判别成立）；AC7/T-11 措辞同步，不再引用"架构修订声明的 seam"。
  2. M-R2-2（desktop 写回测试归宿）：采方案①——写回逻辑提取为 cloudmount-core 纯函数（如 `set_cookie_consent(&mut Config)`，配合既有 load_from/save_to 路径注入变体），core 层单元测（CI 可见，守护 [cookie] 写回语义）；**desktop 接线（登录成功回调调用该函数）显式归人工清单⑦**；AC16 措辞同步（不要求 desktop crate 内自动化测试）。
  3. M-R2-3（windows 编译面）：build_launch_args 的**参数向量组装提取为跨平台纯函数**（纯 Vec<String> 组装，无 windows 专属 API），T-29/T-21 测试无门控；windows 专属执行面（SCM 注册等）留原处；T-25② 清单补全（T-09 全腿 windows-only、T-29/T-21 无门控说明）。
  4. sign_check（status=7）二次区间用例：**延后**——architecture F-14 覆盖矩阵注明延后，qa 缺口节记录（不扩 T-24 复杂度）。
  5. desktop 写回在 config::load 失败时静默跳过（arch MINOR-3）：写回分支补 warn（同意记录丢失可观测）。
  6. MINOR 清单第 17 条（AC16 结构性断言 T 点）随 B-R2-1 收敛一并落点（T-28 或新增 T-30 grep 式断言）。

GATE-REJECTED gate1 2026-10-14

---

2026-10-14 收敛轮对齐拍记录（编排者）：
- 三个角色完成收敛修订（仅动被点名条目）：prd-v3 @ ba295c9、architecture-v3 @ a0e1a03、test-strategy-v3 @ d45af6a。pm 顺带标出两处超出点名范围的残留（U16 目标列旧口径、AC7 When 场景与新主判据并存），经编排者核实为"点击即同意"裁决的直接推论，已派回 pm 一并回填。
- **收敛点三方一致性自查（关键锚点全部通过）**：
  1. **点击即同意口径**：prd U16/AC16（点击登录时构造、enabled=true 恒真、consent_acknowledged=勾选状态、初值取配置、死锁理由注）↔ arch desktop 设计节 + 决策 #14（"配置不参与 policy 构造"）↔ qa T-28/T-30（点击口径 + 结构性断言收窄为组装路径、登录会话豁免带注释）。
  2. **set_cookie_consent**：prd AC16 分支 1（core 纯函数 + T-27 + desktop 接线归人工清单⑦）↔ arch desktop 节/数据模型/F-05 配套（CLI 与 desktop 双写入方均经同一函数）↔ qa T-27（三断言 + 负基线 + workspace exclude 理由）。
  3. **时间窗交叠判据**：prd AC7（双场景 When：主判据=不同前缀双目录交叠、副判据=缓存 Y 提前返回）↔ arch F-07（零生产 seam、不新增观测 hook）↔ qa T-11（500ms 钉死、交叠主判据、60% 副判据）。
  4. **windows 编译面**：arch F-12 跨平台提取注 ↔ qa T-29（无门控 + 烘焙默认值钉死）/T-25② 清单补全（T-09 全腿 windows-only）。
  5. **其余 MINOR**：T-01 除末块口径、T-03 None 透传、T-05 单层向量+编码向量回归面注、T-14 句柄保持时长、T-10 容错断言、T-08 锚 check_enabled、ACTIVE_WATCHERS saturating_sub、F-12 开放问题已裁决、sign_check 延后（arch+qa 双侧）、人工清单⑧ Linux RSS——全部落位（qa"修订落点对照"节可查）。
- 新计数：AC1–AC16 ↔ T-01…T-30 全部有映射落点；P0 = 9、P1 = 20、P2 = 1，共 30 点。
- 相位转移：gate1 复位 pending（重审前复位），phase → gate1；第三次复审按打回裁决**只核对新收敛点**（B-R2-1/三 MAJOR/MINOR 清单），不重跑全量四方互审。


---

2026-10-14 Gate#1 第三次复审（r3 轻量）与人类终审（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate1-ai-r3.md`（轻量复审，按 r2 打回裁决只核对收敛点：B-R2-1 + 3 MAJOR + 15 MINOR 全部经源码级核验实质消解；残留 0 BLOCKER / 0 MAJOR / 8 MINOR 一句话级措辞残句，两评审角色均认定不构成 Construction 闸门障碍）。
- 问：「Gate#1 第三次复审：收敛点全部实质消解，残留 0 BLOCKER / 0 MAJOR / 8 MINOR（一句话级措辞，两评审角色均认定不阻塞 Construction）。如何裁决？」
  答：「通过进 Construction（推荐）」
- 问：「追认项打包：两轮修订中编排者代行裁决共 11 条（m13/m18 升格 F-04 五处施工面、数值口径统一、T-11 纯测试侧判据、set_cookie_consent 方案①、build_launch_args 跨平台提取、sign_check 延后、desktop 写回补 warn 等，逐字在 decisions.md）。是否全部追认？」
  答：「全部追认（推荐）」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 中最新 GATE-APPROVED gate1，此前两条 GATE-REJECTED gate1 2026-10-14 均已被本轮新回执覆盖新鲜度），phase → construction。
  2. 11 条代行裁决全部追认入档；Construction 中如有异议可随时推翻并记入本文件。
  3. 残留 8 条 MINOR（reviews/gate1-ai-r3.md 清单）作为 Construction 前置微调落 plan.md 首个 unit（或随首个相关 unit 顺带回填），不阻塞。

GATE-APPROVED gate1 2026-10-14

---

2026-10-14 Construction/Sprint Planning 记录（编排者）：
- gate1 approved（@ 671de97）后拆分 12 个 unit（plan.md + units/1-12.md）：U1=F-14 基建（关键路径）、U2=F-04 五处脱敏、U3=F-11 oss_put、U4=F-10 双入口单飞、U5=F-05+desktop、U6=F-02 probe 转发、U7=F-06 RdcWatcher、U8=F-07 锁重构、U9=F-01 水合分块（P0）、U10=F-03+CI mock 腿、U11=F-08+F-09 写路径、U12=F-12+F-13。
- 泳道/波次：①U1+U4+U9（三 crate 并行）→ ②U2+U5+U7+U10 → ③U3+U6+U8+U11 → ④U12 → ⑤全量验证（cargo test --workspace 两腿）→ gate2。同 crate 文件冲突面已串行化（U10→U11 同 webdav.rs）。
- r3 残留 8 条 MINOR 按终审决议由编排者顺带回填（prd 3 处：AC1 末块口径/AC16 分支 3 括注更正/已裁决节溯源标注；test-strategy 5 处：头部版本标签 v3/映射口径 v3/T-05 向量几何更正/T-08 锚统一/T-11 副判据场景归属/T-30 注释断言成分）——属终审批准的"随首个提交顺带回填"授权，措辞级、零语义变更。
- unitStatus 全部 pending；TDD 锁未置位（各 unit 失败测试提交后按序置锁，tddLockPaths 并集只增不减）。

---

2026-10-14 TDD 锁置位记录（编排者，Wave 1 红阶段提交后）：
- 红测提交：U1 @ 9daacd0（cookie_channel.rs + cookie_mock/mod.rs，5红4绿护栏）、U4 @ a375829（single_flight_refresh.rs，4红4绿护栏）、U9 @ 59c3315（cfapi.rs 内联 pump_chunks_tests + cfapi_hydration.rs 集成骨架，5红）。
- tddLock 置位：true；tddLockPaths（并集）= crates/cloudmount-driver115/tests/cookie_channel.rs、crates/cloudmount-driver115/tests/cookie_mock/mod.rs、crates/cloudmount-driver115/tests/single_flight_refresh.rs、crates/cloudmount-vfs/tests/cfapi_hydration.rs。
- **例外留痕**：U9 的 5 个单元红测为 cfapi.rs 内联 `#[cfg(test)] mod`（Rust 同文件测试模块），路径锁会阻断同文件实现——该模块不列入锁清单，以"绿阶段测试断言零修改 + gate2 verifier 对照红测提交 diff"兜底（U4 的 open.rs 骨架同理，其测试在独立 tests/ 文件已锁定）。
- U4 遗留口径备注（供 gate2 verifier/qa 核验，非阻塞）：trait `refresh()` 直连并发入口的断言为"无并发在途窗口（串行化）"而非"恰 1 次"——晚到观察者会触发一次串行多余刷新，"恰 1 次"契约由 `refresh_stale` 显式同锚入口与 with_token_retry 生产路径承载（架构关键决策 #7 的动机）；理由已写入测试文件头注释。

---

2026-10-14 U9 实现阶段临时解锁留痕（编排者）：
- 解锁路径：crates/cloudmount-vfs/tests/cfapi_hydration.rs（自 tddLockPaths 临时移除）。
- 理由：U9 绿阶段将把 HydrateFn 契约切换为 ChunkSink（F-01 核心修复），该文件 :190/:246 一带的既有 HydrateFn 闭包调用点必须做**机械签名适配**（断言面不变，仅闭包签名/包装方式跟随新契约）——路径锁会阻断这一合法性改动。非"契约错误"情形，系 Rust 同文件/契约切换与路径锁的结构性冲突；本文件红阶段的 2 个新集成骨架（fetch_data_multiblock/cancel）断言不得削弱。
- 适配完成并验证绿后，该路径重新纳入 tddLockPaths（并集），本条作为 /aidlc:unlock-tests 等价留痕。
