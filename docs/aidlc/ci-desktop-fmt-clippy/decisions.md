# Decisions: ci-desktop-fmt-clippy

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-28 A1 意图接入确认（人类指令「开条目 23」——b2-115-qualify ops 裁决②「直接开下一批」的批号指定）：
- 意图原话（逐字）：`开条目 23`——backlog 台账条目 23（`docs/aidlc/backlog.md:41`：CI 补 desktop fmt/clippy 步骤[终验口径缺口上游补死]；来源=desktop-installers retro「唯一未沉淀进 runbook 的新可执行候选」；分类=CI；状态=待排[体积小]）。
- 识别的意图一句话：ci.yml 补 desktop（workspace exclude 独立 cargo 项目）的 fmt+clippy CI 执行面——上游补死终验口径缺口（根级命令对 exclude 成员零效力，desktop-installers gate2 M3 实证）。
- 粗粒度拆分预览（3 拆）：①ci.yml 新增 desktop job（fmt+clippy，沿既有 job 模式）②desktop 现状确认（本机 fmt/clippy 是否 clean——决定 CI 补死后首跑是否红）③断言脚本扩展（desktop-installers-yaml.test.sh 或新脚本守护新 job 字面——沿五脚本先例）。
- 呈批一问（AskUserQuestion）：Q「意图【CI 补 desktop fmt/clippy——条目 23】——是否进入 AIDLC 生命周期？路线选哪个？」（进入 express[体积小纯 CI 配置]／进入 classic[协议缺省]／调整拆分／不进流程直接答复）。
- **A1 表单未获选择 → 按协议缺省 route=classic**（未选 express 的 fail-safe 全量路径——classic→express 降级被禁止，缺省即锁定；人类若偏好 express 须在 gate1 前裁决留痕）。
- 裁决落点（编排者）：建 intent 骨架 → classic 扇出 pm/architect/qa 三工件 → 对齐拍 → phase=gate1。

---
