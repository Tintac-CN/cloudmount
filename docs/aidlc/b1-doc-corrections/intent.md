# Intent: b1-doc-corrections

> 创建：2026-10-16　状态：inception

## 一句话意图
B1 文档纠偏批：stale Tauri 全量面回写（16 处）+owner 元数据修正+.gitignore 死路径清理+TLS 注释过时订正——审核报告 `docs/aidlc/audit-roadmap-next-steps/audit-findings.md` §2 偏离清单的文档面回写（路线图 §5.1 批次表 B1，backlog 台账🚀行）。

## 问题与期望
- **要解决的问题**：审核（audit-roadmap-next-steps）确认的文档-实现偏离集中单根因——「Tauri 壳」叙事已在仓库 16 处（README×4[含 :151 不可执行死命令 `cargo tauri dev`]/ROADMAP×3/ARCHITECTURE×3/web-ui README×4/api.ts×1/根 Cargo.toml 注释×1）与实现（apps/desktop=egui/eframe）脱节；另有 owner 元数据错侧（根 Cargo.toml zhonghaoxi vs 实际 Tintac-CN）、.gitignore:13-14 src-tauri 死路径、webdav.rs:29 过时 TLS 注释。首屏文档漂移直接误导新读者（P1×3）。
- **期望的结果**：16 处回写为 egui/eframe 事实表述（死命令改为实际构建/运行方式）+owner 统一 Tintac-CN+死路径清理+TLS 注释订正；全量 `grep -ri tauri`（排除历史 AIDLC 工件）清零。

## 受影响系统/用户
- 文件面（审核报告 §2.1① 明细钉死）：根 `README.md`、`docs/ROADMAP.md`、`docs/ARCHITECTURE.md`、`web-ui/README.md`、`web-ui/src/api.ts`（头注）、根 `Cargo.toml`（注释+owner）、`.gitignore`、`crates/cloudmount-daemon/src/webdav.rs:29`（注释）。
- 用户：新读者/贡献者（首屏准确性）；发布/镜像面（owner 一致性）。

## 约束
- **纯文档/元数据/注释回写**：零产品逻辑变更（webdav.rs 仅注释行；api.ts 仅头注；Cargo.toml 仅注释与 repository 字段——不动依赖与代码）。
- 回写事实基准=审核报告 §2.1① 明细+architecture AS-IS（egui/eframe 0.31+wgpu+tray-icon 0.21；web-ui 单宿主=daemon serve；desktop 为独立 cargo 项目 workspace exclude）。
- 五断言脚本与 workspace 基线（277/0）不得受影响；desktop-installers-yaml.test.sh 若有相关断言须保持绿（其 TP-18/24 面引用文档字面——回写后复核）。
- 不改历史 AIDLC 工件（docs/aidlc/ 内其它 intent 的工件为审计记录，其中的 tauri 字面属历史留痕——回写范围仅限上列产品/文档文件）。

## 开放问题（需人类裁决）
- [ ] ROADMAP.md 的「Tauri 壳」条目回写后，其 [x] 完成语义保留（egui 壳已完成同等功能）——回写以「事实订正」形态而非删除条目（编排者默认，待 gate1 过目）。
- [ ] web-ui 的角色定位表述（单宿主=daemon serve）随回写同步——architecture 开放问题「是否正式降级为边缘单宿主」在本批以文档事实化方式处理（不裁决未来是否恢复双宿主）。
