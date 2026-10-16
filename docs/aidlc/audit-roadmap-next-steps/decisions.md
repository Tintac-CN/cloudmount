# Decisions: audit-roadmap-next-steps

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-16 A1 意图接入确认（AskUserQuestion 呈出未获选择；按协议缺省取值，编排者留痕并标注来源）：
- 意图原话（逐字，命令实参）：`请对现在的开发计划和功能实现做一次审核，开始计划下一步的实现和推进`
- 识别的意图一句话：对 `docs/ROADMAP.md`（开发计划）+ 现有功能实现做一次证据化审核，并据此规划下一阶段实现与推进。
- 粗粒度拆分预览（5 拆，逐字）：
  1. **路线图断言核验**——M0–M5 各 [x] 声明抽证（代码/测试/CI 证据对照），揪出「勾了但没验」项
  2. **计划-实现偏离扫描**——已见苗头：路线图写「Tauri 壳」而 desktop 实为 egui/eframe；`apps/shell-macos` 为占位 README；`.gitignore`/gen_icons 残留 src-tauri 死路径；web-ui 角色变迁
  3. **开放项+backlog 归集**——路线图未勾项（AppID 实测／真实凭据联调／RSS≤20MB／MSIX／macOS File Provider／Cookie 独有能力）+ 四个已完结 intent 的 P2 backlog ~20 项
  4. **质量与交付面体检**——277 测试+五断言脚本（112/72/23/13/9）+ 三 CI 工作流覆盖 + 已知边界（同秒漏检已关闭／未签名／aarch64-only）现状盘点
  5. **下一阶段路线图**——按依赖/风险/价值排序的推进计划（含是否启动 macOS File Provider、Cookie 独有能力、backlog 清理批次）
- 呈批三问（AskUserQuestion，逐字）：Q1「意图【审核开发计划+功能实现并规划下一步】——是否进入 AIDLC 生命周期？路线选哪个？」（进入 express（推荐）／进入 classic／调整拆分后再进／不进流程直接答复）；Q2「本期交付范围（影响 units 拆分规模）：」（审核+路线图（推荐）／审核+头部项实现／审核+清全 backlog）；Q3「审核的证据标准（决定发现可信度）：」（抽样验证（推荐）／全量验证／仅文档审）。**表单未获选择（user did not provide answers）。**
- 裁决落点（编排者，按协议缺省+意图字面取默认，来源逐项标注；**未推断任何人类偏好**）：
  1. **接入=是**：用户以显式 `/aidlc:inception` 命令 + 明确审核指令进入（命令即入口信号；非 auto-route 自然语言入口）。
  2. **route=classic（协议缺省）**：表单未选 express → 按 skill §3.0「用户未选 express 或未确认 → **缺省 classic**」取缺省（fail-safe 全量路径：三工件含 architecture.md、三方互审）。**后果注记**：协议禁止 classic→express 降级，缺省取值即锁定全量路径——人类若偏好 express 须在 gate1 前以裁决留痕复核（已列 intent.md 开放问题）。
  3. **交付范围=审核+路线图（编排者按意图字面取默认）**：意图原话「做一次审核，开始计划下一步的实现和推进」= 审核报告+下阶段计划；实现推进随后续 intent 逐个做。**待 gate1 人审过目**（intent.md 开放问题第 1 项）。
  4. **证据标准=抽样验证（编排者取默认）**：每里程碑 1-3 条关键声明跑证据 + 抽跑测试/CI 现状；**待 gate1 人审过目**（intent.md 开放问题第 2 项）。
- 后续：建 intent 骨架（intent.md/state.json/decisions.md）→ 按 classic 扇出 pm/architect/qa 三工件 → 对齐拍 → phase=gate1。

---

2026-10-16 Inception 对齐拍记录（编排者；含 §8 降级代行）：
- 三工件：prd @ be118d4（AC1..AC14 连续无空号）/ architecture @ 3e3feb7+align 88bf9b1 / test-strategy @ c3276a1+align（本条）。
- **architect 回填**（88bf9b1）：语义对齐表锚 AC1..AC14+补 AC12 缺行（原无对应动词行——由 §逻辑架构 CI 段+§安全与权限已知边界节承载）+prd 落盘事实订正；表与 prd 动词双向一致核过。
- **qa 回填（§8 内联代行）**：qa 代理停滞（对齐派发后 >20 分钟零产出，两次 600s 阻塞等待无果）→ 停单（TaskStop）→ 编排者内联代行映射回填（机械替换：26 TP 映射列+头注说明+缺口项勾销）；映射关系经 prd.md 正文逐条核对；**AC 覆盖机械自检**：26/26 行匹配，AC1..AC14 全覆盖无空锚（AC1←TP-12/17、AC2←TP-01/02、AC3←TP-03..06、AC4←TP-07/08、AC5←TP-09、AC6←TP-10、AC7←TP-11、AC8←TP-15/16/18/19/20、AC9←TP-19/20/24、AC10←TP-23、AC11←TP-21/22、AC12←TP-03/13/14/16、AC13←TP-06/24、AC14←TP-25/26）。
- 对齐拍结论：**工件齐且对齐**（classic 三件全落盘并提交；qa 引用 AC 编号 ↔ prd 实际编号一致[14/14]；architect 语义对齐表 ↔ prd 动词一致[含 AC12 补行]）。
- 相位转移：phase → gate1（gate1=pending），提示 `/aidlc:gate`（party-review classic 三件互审）。

---
