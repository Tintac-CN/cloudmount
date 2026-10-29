# Intent: ci-desktop-fmt-clippy

> 创建：2026-10-28　状态：inception

## 一句话意图
CI 补 desktop fmt/clippy 步骤（backlog 台账条目 23——终验口径缺口流水线上游补死）。

## 问题与期望
- **要解决的问题**：`apps/desktop` 为 workspace exclude 独立 cargo 项目——根级 `cargo fmt --all -- --check`/`cargo clippy --workspace` 对其**零效力**（desktop-installers gate2 互审 M3 实证：desktop 内 fmt dirty 394 行未被根级命令拦截）；当前 ci.yml 的 `rust` job 只跑根级命令，desktop 的 fmt/clippy 面**在 CI 从不执行**——终验口径缺口（audit-roadmap §4.1 缺口 1/2）。
- **期望的结果**：ci.yml 新增（或扩展）步骤使 desktop 的 fmt+clippy 在 CI 执行——desktop 侧格式/静态检查违规即 CI 红，上游补死终验缺口。

## 受影响系统/用户
- 文件面：`.github/workflows/ci.yml`（唯一改动面——新增 job 或步骤）。
- 用户：CI 消费者（PR/push 门控）；desktop 开发者（违规即早知）。

## 约束
- **仅 CI 配置变更**：不改产品代码/不改 desktop 源码（若 desktop 当前 fmt/clippy 有违规，须先确认现状——若 dirty 则本 intent 范围内只补 CI 步骤+如实报告违规，修复归 desktop 代码批）。
- desktop 构建依赖重（eframe/wgpu——CI 缓存 Swatinem/rust-cache 已在用，desktop 独立 target 目录需 cache key 覆盖）。
- 沿既有 ci.yml 模式（job 命名/step 风格/注释密度）——brownfield 一致性。
- 既有 job 零删改（四 job 结构不动——新增 job 或在既有 job 内追加 step）。

## 开放问题（需人类裁决）
- [ ] desktop fmt/clippy 的 CI 形态：新增独立 job（如 `desktop` job）vs 在既有 `rust` job 内追加 step（matrix 适配）——影响 CI 时长与缓存策略。
- [ ] desktop clippy 是否含 `--all-targets`（终验口径=含；CI 时长考量）。
- [ ] 若 desktop 当前 fmt/clippy 有违规：CI 补死后首跑即红——修复归本 intent 还是分批？
