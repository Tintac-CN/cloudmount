# Ops Runbook: macos-release

> 产出者：编排者　日期：2026-10-15　前置：gate2 approved（含准出回填 @ 893ed4e）
>
> 人类裁决（AskUserQuestion 2026-10-15，逐字留痕 decisions.md）：**发布 = 手动 tag + 首 tag prerelease 预演**；**监控 = 轻量 + 两项观察**；**retro = 保留**。

## 系统画像

CI/发布基建 intent，无运行时面——监控对象 = CI/发布链路。交付物：`release.yml` mac 腿（tag `v*` 触发）、`.github/workflows/check-macos.yml`（仅 `workflow_dispatch` 触发的编译面预检）、`scripts/package-macos.sh`（mac 双架构打包单一事实来源，CI 与本地 dry-run 同脚本）、三结构断言脚本（9/23/13 用例，TDD 锁定）。

## 发布流程（裁决：手动 tag + 首 tag prerelease）

1. **发布前检查清单**（SOP，test-strategy 缺口节固化）：
   - 第 0 步：手动触发 `check-macos.yml`（workflow_dispatch）确认绿——编译面预检（本地不可行，B1 裁决唯一路径）；
   - 本地 dry-run 一道：`bash scripts/package-macos.test.sh` 9/9 绿；
   - push 后 ci.yml 三平台双腿绿。
2. **首个含 mac 腿的 tag：prerelease 预演**——tag 触发 release.yml 全绿后，按人工核验清单核 T14-T16：两架构 tar 可下载、解包四项清单齐（`cloudmountd`/`cloudmount`/`README.md`/`edge.md`）、`file` 输出含 `Mach-O`+`arm64`/`x86_64` 子串、Release notes mac 行含签名状态与 Gatekeeper 提示、单 tar 体积超出 5–50 MB 带即复核。核验通过后转正式发布（去 prerelease 标记或重打正式 tag）。
3. **后续 tag**：直接正式发布，发布后按清单抽查（至少 T15 架构子串一项）。
4. **回退**：Release 页删除错误资产/删 prerelease；workflow 层回退 = `git revert` 对应提交（既有腿零删改断言守护 revert 安全性）。

## 监控与观察项（裁决：轻量 + 两项观察，零新增基建）

- **硬门**：CI 三平台双腿绿 + `check-macos.yml` dispatch 绿 + 三脚本断言绿（9/23/13）。
- **观察项 1（macos-15 runner 可用性）**：GitHub 官方 runner-images 退役/迁移公告时——`runs-on: macos-15` 字面与 T9 断言需同步改（改断言走解锁仪式或随下个 intent；`-latest` 迁移期不追新，pin 到退役再迁）。
- **观察项 2（actionlint 缺口）**：本地降级在案（YAML 解析+结构断言+记缺口）；CI 无 actionlint 步骤，workflow 语法错由 GitHub Actions 解析失败兜底（job 直接红、needs 门控保证不误发布）。补 CI actionlint 步骤走新 intent（P2 backlog）。

## P2 backlog（下个 intent 候选）

- ci.yml 增 actionlint 步骤（语法拦截前移，消观察项 2 缺口）；
- check-macos.yml `permissions:` 显式化（最小权限加固）；
- 断言脚本 YAML 解析精度（job_block 行启发式 → 真解析）；
- package-macos.sh tar 半成品清理（临时名成功后 mv）；
- `.gitattributes` 行尾统一（release.yml CRLF / check-macos.yml LF）。

## 回流机制（Operations 协议 §2）

- 发布事件/CI 异常/新需求 → 经 `/aidlc:inception` 写**新 intent**（不改本 intent 已完结工件）；过程学习沉淀 `retro.md`。
