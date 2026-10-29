# Operations Runbook: ci-desktop-fmt-clippy

> 产出者：编排者（Operations）　日期：2026-10-29　前置：gate2 approved（GATE-APPROVED gate2 2026-10-29 @ 313826d）
> 本批性质：CI 配置变更（无运行时服务/无部署面）——运维面 = GitHub Actions 执行面与断言硬门。

## 1. 交付面与触发面

- **产品面改动**：`.github/workflows/ci.yml` 新增独立 `desktop` job（windows-latest 单腿；fmt/clippy 唯 `--manifest-path` 字面；`workspaces: apps/desktop -> target`；`permissions: contents: read`；`timeout-minutes: 60`）+ 新断言脚本 `scripts/ci-desktop-yaml.test.sh`（47 条断言）。
- **触发**：`ci.yml` 现有 `on:` = push(main) + pull_request；本批不新增触发器、不新增 secrets/网络端点、零新增第三方 action。
- **部署窗口**：**N/A**（本批无部署/发布面；release.yml 零触碰、无产物发布动作）。
- **回滚**：首选 fix-forward（CI 绿红由命令退出码决定，可快速再提交）；紧急回滚 = revert `d5818ed`（ci.yml desktop job）+ `810ad27`（脚本）两提交，并在 decisions/新 intent 留痕（回滚即恢复「desktop fmt/clippy 在 CI 从不执行」的缺口状态，非必要不采用）。

## 2. 监控与告警

| 监控项 | 信号源 | 告警/观察口径 |
|---|---|---|
| desktop job 红绿 | GitHub Actions run 页 / 通知 | job 红 = 门控生效（真实违规）或工具链漂移或基础设施故障——按 §3 三分类处置 |
| desktop job 时长 | Actions run 摘要 | 首跑冷编记录；warm 跑异常拉长（>2x 首跑 warm 值）时查 cache 命中与体积 |
| rust-cache 命中/体积 | Actions 缓存页 | entry 体积、命中率、既有四 job 时长本批前后对比（10GB 上限按 §6 处置） |
| 六脚本硬门 | 本机/开发机 Git Bash | 每次修改 ci.yml 或断言脚本前后全跑（§4） |
| 既有四 job 时长 | Actions 趋势 | 本批前后对比；显著变慢即按 §6 缓存处置顺序排查 |

- **告警渠道**：沿用 GitHub Actions 默认通知（无新增告警系统）；失败行以 job 名 `Desktop（fmt + clippy）` 为失败面标识。
- **required checks 现状（如实）**：`desktop` **尚未**加入 branch protection required checks——红可见但不阻塞合并。处置见 §5（暂定口径）。

## 3. 首跑观察清单（TP-06 / M6）

**时点**：本批下次 push/PR 即触发（暂定口径=首推当日核验，待人类追认，见 §5）。逐项记录：

1. run URL + 触发方式（pull_request 优先 / push main 兜底）；
2. `desktop` job 结论（预期绿——AC2 于 2026-10-29 实测三条全 clean）；
3. job 时长（冷编/暖跑分别记录）；
4. runner `rustc -V` 与本机版本对比（`@stable` 浮动漂移区分口径）；
5. rust-cache：首次 miss/后续 hit、entry 体积、`apps/desktop/target` 是否入缓存；
6. 既有四 job 时长与本批前对比。

**红态三分类**（不得混淆）：

- **真实违规**：日志含 fmt diff 或 clippy warning/error → 门控生效；修复归紧随的 desktop 代码批（本批约束不改 desktop 源码）。
- **工具链漂移**：新版 rustc/clippy/rustfmt 规则变化且本机复跑 clean → 记录 runner 与本机版本差；必要时另开工具链钉版批。
- **基础设施**：runner/网络/缓存故障 → 重跑；连续失败升级人类。

**观察记录载体**：生产事件/学习写入新 intent 的 `intent.md` 或 `docs/aidlc/backlog.md`；不修改已完结工件。首跑结果建议记入 backlog 条目 41 的状态备注。

## 4. 六脚本硬门口径（AC3 后半句落实）

本批起，断言硬门以「**六脚本**」口径执行（不修改 `desktop-installers/release-checklist.md` 等已完结工件的五脚本循环；后续 runbook/清单以此为准）：

```bash
bash scripts/desktop-installers-yaml.test.sh   # 112/112（actionlint 可用环境 113）
bash scripts/installers-yaml.test.sh           # 72/72
bash scripts/release-yaml.test.sh              # 23/23
bash scripts/check-macos-yaml.test.sh          # 13/13
bash scripts/package-macos.test.sh             # 9/9
bash scripts/ci-desktop-yaml.test.sh           # 47/47（本机 actionlint 不可用；可用环境运行期 M=48）
```

- 判据：六脚本全 exit 0；计数与上述基线一致（`desktop-installers` 与 `ci-desktop` 计数环境相关，以当次末行 `N/M` 自洽为准）。
- 断言不得弱化（prd AC3 三条）：负断言必须经 `grep_absent_ok`；正断言不得由精确字面放宽为子串；脚本自检/CRLF 剥除/降级计数断言必须存在。弱化须 decisions 留痕并经人类批准。

## 5. 门控与阈值（暂定，待人类追认）

> AskUserQuestion 三问未获选择（2026-10-29，见 decisions.md 同日条目）——以下取推荐缺省值，标注「暂定」；完结确认时一并追认或修订。

| 项 | 暂定口径 | 依据 |
|---|---|---|
| 首跑观察窗口 | **首推当日核验**（本批下次 push/PR 触发；无部署窗口=N/A） | 推荐项；TP-06/M6 需尽早暴露漂移与体积异常 |
| required checks | **稳定后加入**：首跑后累计 ≥3 次绿再由人类在 GitHub 仓库设置将 `desktop` 加入 branch protection；本批不代操作仓库外设置 | 推荐项；避免首跑波动误挡合并 |
| 缓存告警/处置阈值 | **10GB + 既有 job 时长双阈值**：entry 体积逼近 10GB 或既有四 job 显著变慢 → 按序 `shared-key: desktop` → `save-if`（仅 main 保存）→ `cache-targets: false` 降级，变更须 decisions 留痕 | 推荐项；architecture M6 风险节 |

## 6. 缓存体积处置顺序（M6 follow-up）

1. 首跑后量 entry 体积 / 命中率 / 既有四 job 时长（本批前后）；
2. 若挤占：优先 `shared-key: desktop`（键命名空间隔离，不增配额）；
3. 其次 `save-if` 仅 main 保存（降 entry churn，PR 只恢复）；
4. 最后 `cache-targets: false`（仅注册表缓存，记录为降级并保留回滚条件）；
5. 任何变更 decisions 留痕；缓存保存失败只影响时长、不影响红绿。

## 7. 回流与台账

- **回流机制**：生产事件/学习只写新 intent 的 `intent.md` 或 `backlog.md`（当前 intent 的学习已入 `retro.md`）；已完结 intent 工件只读。
- **台账现状**：条目 23（CI 补 desktop fmt/clippy）=「已交付（gate2 通过，待 ops 收口）」——本 runbook 落盘即收口；条目 40=断言硬化候选（待下次解锁批）；条目 41=首跑观察 + required checks（首推后触发）。
- **待人类事项**：OQ②（clippy `--all-targets` 收窄与否）仍留 PRD `[ ]` 待逐字确认（当前按含口径运行）；本 runbook §5 三项暂定口径待追认。
- **已知边界**：desktop job 未入 required checks；`@stable` 浮动工具链；`fmt --all` 实际连带根 workspace 66 文件（架构 r3 已订正，失败归因可能被稀释）；TP-06 与 M6 未在本地验证。
