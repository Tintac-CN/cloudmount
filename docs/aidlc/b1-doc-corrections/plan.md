# Plan: b1-doc-corrections

> 产出者：编排者（Sprint Planning）　日期：2026-10-17　前置：gate1 approved（065434d，含准出回填 a4d9474）

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | 23 处 stale Tauri 回写+3 项关联订正（蓝图 28+2 项照做） | 按回写蓝图 architecture.md A/B/C/D/E 节逐项执行：A 16 处+B 3 处（#17-19/#25）+C 5 处（#20-24）+D 1 处（#26）+E 邻接（README:135 建议项）+关联 3 项（owner:20/.gitignore:13-14/webdav.rs:29）；**E+ 段 README:135 按「邻接调整纳入」执行**（gate1 r2 确认）；#25 按统一形态「（TLS 可选——`--tls-cert/--tls-key`，见下条）」 | 根 `README.md`、`docs/ROADMAP.md`、`docs/ARCHITECTURE.md`、`web-ui/README.md`、`web-ui/src/api.ts`（头注 :1-6）、`web-ui/package.json`（description）、根 `Cargo.toml`（注释 :12-13+repository :20）、`.gitignore`（:12-14）、`crates/cloudmount-core/src/config.rs:3`、`crates/cloudmount-daemon/src/webdav.rs`（:29+四注释行） | low（纯文档/元数据/注释回写，零产品逻辑——F3 扫描：改动文件均为文档/注释面，无代码引用方） | TP-01..07 全执行（蓝图照做+三态判定+AC15 式完整性自检） | 23 处全回写+全仓词边界/子串双口径清零+五脚本 112/72/23/13/9 与 workspace 277/0 不变+「仅注释/头注/字段」边界（TP-06 diff 面核验） |

## 验证命令（F2 规范来源）

- 命令：`cargo test --workspace`
- 辅助现状锚：五断言脚本 `bash scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh`（计数 112/72/23/13/9）+ fmt/clippy 双面（workspace 与 `cd apps/desktop` 各自——TP-05 必达口径）
- **基线已录**（第 0 步，2026-10-17T08:59:45+08:00）：workspace **277 passed/0 failed**；五脚本 **112/72/23/13/9** 全 exit 0
- 回归语义（本 intent 特化）：文档批零产品逻辑——终验重跑同一命令 **277/0 保持+五脚本计数不变=零影响证明**；webdav.rs 仅注释行改动，任何测试失败=越界=BLOCKER

## 执行顺序与并行泳道

- **单 unit 串行**（23 处回写为一次机械遍历，蓝图 28+2 项即执行清单——拆多 unit 反增交接成本）；无并行泳道（纯文档批，无 IaC/E2E 面）
- 执行序沿 test-strategy 注记：①静态回写（蓝图逐项）→②TP-01/02 双口径清零验证→③TP-03 build 验证→④TP-04 三方对照→⑤TP-06+TP-05 收尾同批

## TDD 锁记录

- **N/A（如实记录）**：本 intent 为文档回写批，零测试文件产出，TDD 锁无锁定对象（`tddLock=false`）；质量守护=蓝图照做+TP-01..07 验证+gate2 复审+verifier 独立终检

## 缺口与开放问题

- [ ] 无（蓝图 28+2 项行锚全部实测；唯一环境项=TP-03 cargo run 冒烟为「可拉起」超时语义，非退出码判据）
