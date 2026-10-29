# Intent: audit-roadmap-next-steps

> 创建：2026-10-16　状态：inception

## 一句话意图
请对现在的开发计划和功能实现做一次审核，开始计划下一步的实现和推进

## 问题与期望
- **要解决的问题**：
  - `docs/ROADMAP.md` 的 [x] 完成声明与实际实现现状可能漂移（初查已见苗头：路线图 M2/M5 写「Tauri 壳」而 `apps/desktop` 实为 egui/eframe 壳；`apps/shell-macos` 为占位 README；`.gitignore`/历史残留 `src-tauri` 死路径；`web-ui` 角色变迁未回写）——「勾了但没验」会长期腐蚀路线图的可信度。
  - 四个已完结 AIDLC intent（audit-p0p1-fixes / macos-release / installer-packages / desktop-installers）的 P2 backlog 分散在各 `ops-runbook.md`，合计 ~20 项，无统一归集与排序。
  - 路线图开放项（M0 AppID 实测／M1 真实凭据联调+RSS≤20MB／M2 MSIX／M3 Cookie 独有能力／M5 macOS File Provider／长期 minifilter）缺少按依赖/风险/价值的下一阶段推进排序。
- **期望的结果**：证据化的审核报告（每条发现带可复现证据与分级）+ 排序后的下一阶段路线图（可据此逐个开新 intent 推进）。

## 受影响系统/用户
- 系统面：全仓库——workspace 七 crate（core/vfs/driver115/auth/sync/cli/daemon）、`apps/desktop`（egui 壳）与 `apps/shell-macos`（占位）、`edge/`（WebDAV/systemd/Docker）、`tools/bin`、`drivers/win`（minifilter 设计定稿未实现）、`web-ui`、CI 三工作流（ci/check-macos/release）、五断言脚本。
- 用户：项目所有者（个人开发者）。

## 约束
- **本期为审核+规划类 intent**：以只读核验为主，不改产品代码（实现推进随后续 intent 逐个做）——范围取默认值待 gate1 人审过目（见 decisions.md A1 记录与开放问题）。
- 证据化：每条发现必须带可复现证据（文件/命令/输出），禁止空泛评价。
- 不发明需求：路线图未列的能力不擅自纳入下一阶段。
- 既有 AIDLC 纪律全量适用（hook/TDD 锁/门控回执）。
- route 取协议缺省 classic（表单未获选择，见 decisions.md；gate1 前可由人类复核）。

## 开放问题（需人类裁决）
- [ ] **本期范围**：审核+路线图（编排者默认）vs 审核+头部项实现 vs 审核+清全 backlog。
- [ ] **证据标准**：抽样验证（编排者默认：每里程碑 1-3 条关键声明）vs 全量逐项 vs 仅文档一致性。
- [ ] **下一阶段头号方向**：M0 AppID 实测（外部阻塞项，等审核现状后定）／macOS File Provider（M5 未勾）／Cookie 独有能力（M3 未勾）／backlog 批量清理——依赖审核结论与人类偏好。
- [ ] **route 缺省 classic 是否认可**（classic→express 降级被协议禁止，缺省取值须在 gate1 前复核；若偏好 express 应在 gate1 前以人类裁决留痕切换——注：协议仅允许 express→classic 单向，本期缺省取 classic 即锁定全量路径）。
