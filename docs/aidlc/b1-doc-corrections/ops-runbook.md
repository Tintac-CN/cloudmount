# Ops Runbook: b1-doc-corrections

> 产出者：编排者　日期：2026-10-18　前置：gate2 approved（f46e6f5，含 gate2 r1 修订 8f8200d/a295874）
>
> 人类裁决（AskUserQuestion 2026-10-18，逐字留痕 decisions.md）：**回写效果维护=时点归档**；**台账=唯一队列**；**下一步=直接开下一批（批号待收口时指定）**。
> 本 intent 为**文档纠偏批**（无运行时服务面、无部署面）——监控/告警/部署三节按运维实质适配为「回写消费/台账维护/回流」。

## 系统画像

文档纠偏批：交付物 = 23 处 stale Tauri 回写+3 项关联订正（commit c6dbcb8，10 文件+56/−47；经 gate2 修订 +8f8200d 两处一行事实订正）+prd/蓝图/TP 工件（时点归档）。**无运行时面**——监控对象=回写后文档作为事实表述的有效性（维护归产品面常态演进，裁决①无特殊机制）。

## 回写消费与归档口径（裁决①：时点归档）

- 本批回写（commit c6dbcb8+8f8200d）为 **2026-10-17 时点订正**；回写后的文档（README/ROADMAP/ARCHITECTURE/web-ui 面归产品常态维护，无特殊防回流机制——裁决①明示「无特殊口径」）。
- 溯源链：审核明细=`docs/aidlc/audit-roadmap-next-steps/audit-findings.md` §2（上游时点）→ 本批蓝图 §回写定位表（行锚实测）→ diff c6dbcb8/8f8200d。后续 intent 改动相关文档后引用行号需重新核验。
- 工件归档：prd/architecture/test-strategy/units 随本 intent 归档为时点记录（intent 目录只读——resume/续跑均拒）。

## 台账维护（裁决②：唯一队列）

- `docs/aidlc/backlog.md`（现 51 行：A 开放项 12+B backlog 39[工程 19/观察 5/记录 15]）为**唯一活跃队列**——后续 `/aidlc:inception` 前先查台账；A2 搁置项追加于此；完成标 `✅ 已消解（<slug>）`。
- 本批转出项：MINOR-1（prd/蓝图失实措辞保留为历史工件面）=台账条目 39（记录类）；flaky 观察=条目 38（观察类）。

## 回流机制（Operations 协议 §2）

- 生产事件/学习只写**新 intent 的 intent.md**；当前 intent 的学习沉淀于 `retro.md`（classic 硬性），已完结工件不再修改。
- 路线图本体的后续维护随产品演进正常进行（本批已消除「Tauri 壳」叙述性漂移；新增漂移的发现依赖后续审核 intent 按需开）。

## 监控与观察项

- **观察项 1（台账条目 38）**：engine_works_with_native_watcher 满载时序 flake 1 例（隔离复跑 5/5，非 B1 回归）——重试加固或隔离标注，随下个代码类 intent 顺带。
- **观察项 2（台账条目 39）**：prd/蓝图失实措辞保留为历史工件面——后续重放蓝图 #15 验收时以 api.ts:6 现字面为准，勿按旧形态逐字比对。
- 其余台账观察项（macos-15 退役/WiX 废弃/真机核验触发——条目 29/30/31）沿上一 intent runbook 持续有效。

## P2 backlog

见 `docs/aidlc/backlog.md`（唯一权威台账）。下一阶段启动序=审核报告 §5.1 批次表（B1 已完成→B2 资质申请[外部链源头，人类操作为主]→B3/B5/B7…，依赖图 §5.2）+台账工程条目（CI fmt-clippy 条目 23 等）。

## 下一步衔接（裁决③）

**收口后直接开下一批**（批号由人类收口时指定——台账待命：B2 资质申请/B3 Cookie 独有能力/条目 23 CI fmt-clippy/条目 31 真机核验批[条件已达]等）。
