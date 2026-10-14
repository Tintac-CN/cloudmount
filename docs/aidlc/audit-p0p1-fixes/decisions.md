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
