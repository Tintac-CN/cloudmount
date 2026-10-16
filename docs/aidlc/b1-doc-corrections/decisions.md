# Decisions: b1-doc-corrections

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-16 A1 意图接入确认（AskUserQuestion 表单未获选择；按协议缺省取值，编排者留痕并标注来源）：
- 意图原话（逐字，接入信号=ops 裁决③预先指令+显式 /aidlc:inception）：`B1 文档纠偏批：stale Tauri 全量面回写（16 处）+owner 元数据修正+.gitignore 死路径清理+TLS 注释过时订正——审核报告 §2 偏离清单的文档面回写（路线图批次 B1）`
- 识别的意图一句话：审核报告（audit-roadmap-next-steps）§2 偏离清单的文档面回写——16 处 stale Tauri 改 egui 事实表述+owner 统一+死路径清理+TLS 注释订正。
- 粗粒度拆分预览（4 拆，逐字）：①stale Tauri 16 处回写（README/ROADMAP/ARCHITECTURE/web-ui README/api.ts/根 Cargo.toml 注释——egui 事实表述，README:151 死命令改实际构建/运行方式）②owner 元数据修正（根 Cargo.toml zhonghaoxi→Tintac-CN）③死路径清理+注释订正（.gitignore L13-14/webdav.rs:29）④回写后验证（全量 grep tauri 清零[排除历史 AIDLC 工件]+五脚本不变+workspace 277/0 基线不变）。
- 呈批一问（AskUserQuestion，逐字）：Q「意图【B1 文档纠偏批：…】——是否进入 AIDLC 生命周期？路线选哪个？」（进入 express（推荐）／进入 classic／调整范围／不进流程）。**表单未获选择。**
- 裁决落点（编排者，按协议缺省取值+来源标注）：
  1. **接入=是**：人类 ops 裁决③预先指令「收口后直接开 B1 文档纠偏批」（audit-roadmap-next-steps decisions.md Operations 条目，2026-10-16）——接入信号明确。
  2. **route=classic（协议缺省）**：表单未选 express → 按 skill §3.0 缺省 classic（fail-safe 全量路径：三工件含 architecture.md、三方互审）；classic→express 降级被禁止，缺省即锁定——人类若偏好 express 须在 gate1 前裁决（列 intent.md 开放问题）。
  3. **范围=审核报告 §2.1① 明细钉死**（16 处+3 项关联订正，零产品逻辑变更）；开放问题：ROADMAP 条目回写形态（事实订正非删除，编排者默认）/web-ui 单宿主表述事实化（不裁决未来双宿主）——待 gate1 过目。
- 后续：建 intent 骨架 → 按 classic 扇出 pm/architect/qa 三工件 → 对齐拍 → phase=gate1。

---
