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
