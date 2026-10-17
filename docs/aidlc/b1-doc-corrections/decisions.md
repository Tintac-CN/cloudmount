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

2026-10-17 Gate#1 人类审批（AskUserQuestion 表单未获选择；对话逐字裁决）：
- 互审输入：reviews/gate1-ai.md @ 792caef（reviewer-pm 0/3/4 + reviewer-architect 0/1/6 + reviewer-qa 0/3/3 + reviewer-dev 0/1/4；合并 0 BLOCKER/8 MAJOR/17 MINOR，去重 6 MAJOR/10 MINOR。正面确认：蓝图 28 项行锚实测零实质漂移、23 处清单独立全仓扫描完备、AC↔TP 机械核对通过、断言面零联动复核成立）。
- 问：「B1 文档纠偏批 Gate#1 互审：0 BLOCKER / 8 MAJOR（去重 6）/ 17 MINOR。……6 MAJOR 全为工件间口径不一致（api.ts 触碰面/TP-07 计数锚/fmt-clippy 门口径/TP-01 锚 carve-out/ROADMAP:164 分类/命令主形态）。附带：M3 口径建议=要求 desktop fmt/clippy 单独跑；M6 主形态建议=build --release 为判据——是否随修订采纳？」
  答：「**打回修订，两项附带建议采纳**」
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception；第 1 轮/上限 3 轮）：修订点名 pm/architect/qa 三角色（dev 无 inception 工件——其发现 M1 归 pm/arch/qa 口径收敛面、行锚正面确认无需动作）。
  2. **附带建议①采纳（M3 口径）**：desktop fmt/clippy **单独跑为必达**（TP-05 豁免删除；沿 audit-roadmap-next-steps 终验修正口径）。
  3. **附带建议②采纳（M6 主形态）**：README:151 替换验证以 **`cargo build --release` 确定性可达为判据**；`cargo run` 仅「可拉起」超时冒烟语义（GUI 阻塞不适用退出 0 判据）。
  4. **M1 口径收敛（编排者按证据裁决，gate1 r2 过目）**：api.ts 触碰面收敛为「**:1-6 头注块**」，D7 搭车订正采纳（:5「当前为骨架」实测失实——八命令已全部实现）；prd 约束③/AC1 #11/TP-06 边界同步放宽至 ：1-6。
  5. 修订点名清单——**pm**：M1（约束③/AC1 #11 放宽 ：1-6+AC1 补 ：5 骨架句搭车项）/M2 关联（AC6b 排除清单补 .git/）/M4 关联（AC1 #5 示例改「M5+ 技术未定」与蓝图 #9 对齐）/M5（AC5(b) 维持必达+措辞域按现文「:164 括号改…其余字面不动」钉死，蓝图侧对齐）/m4（非目标补 README:143 测试计数 stale 点名）。**architect**：M5（蓝图 #25 自「联动候选」移入 B 段必达+措辞按 prd AC5(b) 域）/m1（E 节补 README:135 共享前端残留登记）/m3（三处证据引用精度订正）/M4 关联（蓝图 #9 维持 carve-out 形态——与 qa 侧 TP-01 修订联动）/M1 关联（蓝图 #15 维持 :1-6）。**qa**：M1（TP-06 diff 边界改 api.ts :1-6 头注块）/M2（TP-07 计数锚改 docs/aidlc/** 动态清点口径）/M3（TP-05 删豁免——desktop fmt/clippy 单独跑必达）/M4（TP-01 判据补 #9 carve-out+取值正确性核验[qa-m2]）/M6（TP-03 判据改 build --release 确定性 exit 0+cargo run 可拉起超时冒烟）/m1（AC6b 并列子串口径——__TAURI_INTERNALS__ 下划线邻接漏检）/m3（TP-06 与 TP-05 构造收尾同批执行序注记）。
  6. **decisions 补录（编排者，随本条）**：对齐拍范围扩裁记录（16→23 处，编排者 2026-10-16 裁决=意图「全仓 grep 清零」忠实落实——散落于 test-strategy 头注/prd Q3/architecture 开放问题③/state history，本条目为统一台账锚）。
  7. GATE-REJECTED 标记在后；修订完成后 gate1 复位 pending 复审（只核收敛点：6 MAJOR 消解+两项附带建议落实），须新回执。

GATE-REJECTED gate1 2026-10-17
---

2026-10-17 Gate#1 复审 r2 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate1-ai-r2.md @ 5dc93f5（reviewer-pm 0/0/2 + reviewer-architect 0/1/2 + reviewer-qa 0/1/2 + reviewer-dev 0/0/2；合并 0 BLOCKER/2 MAJOR/7 MINOR。上轮 6 MAJOR+10 MINOR 全部消解实证：行锚 15+ 处抽验零漂移[dev/pr/arch 三方独立实测]/43 条清点不适用本批[23 处清单完备]/AC1..AC6 机械终检通过）。
- 问：「B1 Gate#1 复审（r2，只核收敛点）：上轮 6 MAJOR+10 MINOR 全部消解实证（行锚 15+ 处抽验零漂移）；新发现 0 BLOCKER/2 MAJOR/7 MINOR（#25 替换字面三工件两套/TP-03 辅判据引注失实——均一行级收敛）。如何裁决？」
  答：「**通过+准出回填（推荐）**」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 最新 GATE-APPROVED gate1，满足 GATE-REJECTED gate1 2026-10-17 之后的新鲜度要求），phase → construction。
  2. **准出回填清单**（批准条件式一行级订正，编排者执行并核收留痕，不重走复审）：M-R2-1 #25 括号内替换字面三工件统一为单一形态（采 dev 建议扁平含锚「（TLS 可选——`--tls-cert/--tls-key`，见下条）」，同步蓝图 #25/prd AC5(b)/TP-06 引用判据）；M-R2-2 TP-03 辅判据改写（cargo run 冒烟=独立冒烟指令非回写行文本；回写行文本按蓝图 #4=build --release 形态）；MINOR：api.ts 证据区间 :44-80→:44-81 统一（architecture 两处）/TP-02 AC6a 标注补 config.rs+package.json（8→10 文件）/TP-06 执行序双列收敛（①初核⑤为准）/architecture「并行中」时态改完成时（三处）/TP-01 引 :11-31→:11-32。
  3. 追认项打包：本轮无新 §8 事件（三角色修订+四方复审全代理执行）；历次编排者代提交（inception 角色无 Bash）沿 GATE-APPROVED gate1（audit-roadmap-next-steps）回执先例追认。
  4. 后续：phase=construction，`/aidlc:construct` 进入 Sprint Planning（A1 接入 intent——A3 拆分确认在派发执行泳道前呈出）。

GATE-APPROVED gate1 2026-10-17
---

2026-10-17 Gate#1 准出回填核收（编排者内联执行——人类裁决「通过+准出回填」的条件式一行级订正）：
- **7 处全部落位**（机械 patch+复验零残留）：M-R2-1 #25 替换字面三工件统一扁平含锚形态「（TLS 可选——`--tls-cert/--tls-key`，见下条）」（蓝图 #25[弃嵌套括号长形态]/prd AC5(b)/TP-06 引用判据三处同步）；M-R2-2 TP-03 辅判据改写（cargo run=独立冒烟指令非回写行文本；回写行文本按蓝图 #4=build --release 形态）；MINOR：architecture :44-80→:44-81（#15 处）+「并行中」时态三处改完成时+TP-01 :11-31→:11-32+test-strategy TP-02 AC6a 标注 8→10 文件（补 config.rs/package.json）+TP-06 执行序双列收敛（①初核⑤为准）。
- gate1 裁决的全部准出条件闭合；phase=construction；下一步 `/aidlc:construct` 进入 Sprint Planning（A1 接入 intent——A3 拆分确认在派发执行泳道前呈出）。

---

2026-10-17 A3 拆分确认（AskUserQuestion 表单未获选择；对话逐字裁决——凭据=decisions.md A1 记录）：
- 呈批对象：plan.md @ d903b00（单 unit U1——蓝图 28+2 项照做：23 处 stale Tauri 回写+owner 元数据+.gitignore 死路径+TLS 注释+README:135 邻接；验证=TP-01..07 全套[双口径清零/build --release 主判据/fmt-clippy 双面含 desktop 单独跑/diff 边界零逻辑变更/动态清点]；影响面 low；TDD 锁 N/A[零测试文件]；基线 277/0+112/72/23/13/9 @ 08:59:45 已录）。
- 问：「A3 拆分确认（intent b1-doc-corrections，单 unit U1 只读转回写执行）：是否批准执行？」
  答：「**批准执行**」
- 裁决落点（编排者执行）：PLAN-CONFIRMED 回执在后；派发 U1（dev 泳道——回写执行+TP-01..07 验证一体）。

PLAN-CONFIRMED 2026-10-17
---

2026-10-18 Gate#2 人类审批（AskUserQuestion 表单未获选择；对话逐字裁决）：
- 互审输入：reviews/gate2-ai.md @ 0bf20ab（reviewer-pm 0/0/3 + reviewer-architect 0/2/3 + reviewer-qa 0/0/2 + reviewer-dev 0/1/7 + verifier 0/1/3；合并 0 BLOCKER/3 MAJOR 主题[去重 2]/13 MINOR。verifier 独立终检：蓝图 30/30 全落字、计划外文件=无、零逻辑变更成立、验证声称全部真实复现）。
- 问：「B1 Gate#2 交付门互审：0 BLOCKER / 3 MAJOR 主题（去重 2）/ 13 MINOR。……2 MAJOR 均为本批新写入文本的失实表述：M-1 api.ts「8 命令已全部实现」实为 4/8 命令 501 占位/M-2 web-ui README 引用不存在的 CLOUDMOUNT_WEB_UI 机制。如何裁决？（另：qa 报告既有 flaky 观察……建议登记跟踪）」
  答：「**打回修订+反馈**」
- 反馈内容（=互审发现清单，逐字锚定）：
  1. **M-1**：api.ts:6「8 命令已全部实现」为失实表述——唯一实现方 daemon 边缘模式 4/8 命令 501 占位（webdav.rs:938-942/:958-962）+get_status 硬编码占位，违反 prd 措辞约束②。收敛形态采 architect 建议措辞族。
  2. **M-2**：web-ui/README:20 引用不存在的 `CLOUDMOUNT_WEB_UI` 托管机制（daemon 实际 `--web-ui-dir` main.rs:116-118 无 env=；config 仅 APP_ID/SYNC_ROOT/CHANNEL；edge Dockerfile ENV 为死变量）——改可验证表述。
  3. **MINOR 处置指令**：prd 非目标「不做脱水 API 命名注记」vs #26 已按 A3 批准交付的矛盾→pm 修订 prd 注记；既有 flaky 观察（engine_works_with_native_watcher）→登记 backlog 跟踪；其余卫生 MINOR（双空行/微措辞簇）→入 backlog 不阻塞。
- 裁决落点（编排者执行）：
  1. gate2 打回（gate2=rejected，phase 回 construction；第 1 轮/上限 3 轮）：修订点名 dev（M-1/M-2 一行级×2）+pm（prd 非目标注记）。
  2. 编排者随手项：flaky 观察登记 backlog.md（台账新观察行）。
  3. GATE-REJECTED 标记在后；修订完成后 gate2 复位 pending 复审（只核收敛点：M-1/M-2 消解），须新回执。

GATE-REJECTED gate2 2026-10-18
---
