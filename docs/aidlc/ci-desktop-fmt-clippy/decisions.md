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

2026-10-29 Gate#1 人类审批（AskUserQuestion 表单未获选择；对话逐字裁决）：
- 互审输入：reviews/gate1-ai.md @ 8576a45（reviewer-pm 1/4/8 + reviewer-architect 1/7/8 + reviewer-qa 0/4/7 + reviewer-dev 1/3/7；合并 2 BLOCKER/8 MAJOR 主题/~20 MINOR。正面实证：desktop fmt 实测 clean/四 job 结构/断言脚本可写性/「CI 从不执行 desktop lint」前提成立）。
- 呈批三问（AskUserQuestion，逐字）：
  问：「条目 23 Gate#1 互审：2 BLOCKER / 8 MAJOR 主题 / ~20 MINOR。B1=fmt/clippy 命令字面三工件互斥……B2=rust-cache workspaces 值语义错误……如何裁决？」
  答：「**打回修订，两项推荐采纳**」
  （两问的推荐项随此裁决一并采纳：①B1 字面源=`--manifest-path apps/desktop/Cargo.toml` 完整字面 ②OQ① 形态=独立 `desktop` job+windows-latest 单腿）
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception；第 1 轮/上限 3 轮）：修订点名 pm/architect/qa 三角色（dev 无 inception 工件——其发现归三角色修订面）。
  2. **B1 字面源裁决=--manifest-path 完整字面**：prd AC1①/TP-01 字面改为 `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check` 与 `cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`；删除 working-directory/cd 形态锚（架构断言 #7/#8/#10 维持——已与裁决一致）；三工件同一字面源。
  3. **OQ① 形态裁决=独立 desktop job+windows-latest 单腿**：prd AC1 增「形态与 runs-on 按裁决落字面」子项（job id=`desktop`/runs-on=`windows-latest`）；TP-01 同步断言；M7 冲突（追加形态改 rust-cache vs 零删改）随裁决消解。
  4. 修订点名清单——**architect**：B2（rust-cache 值改 `apps/desktop -> target`+断言 #6 精确形态+「workspaces 给出后不连带根 workspace」叙述修正）/M5（按 prd/test-strategy 定稿实文回填语义对齐表+同步断言清单）/M1（断言清单补齐 AC4 锚集：rust 四命令[含 check]+三 OS matrix+components+另三 job 命令+env[双击]+on:；#12 修正 `--` 拼写与三命令→四命令）/M2（负断言补 status 函数[always|success|failure|cancelled()]+shell 级静默绿[|| true/set +e/if: false]——沿 desktop-installers-yaml.test.sh:355-372 regex）/M4（现状统一「证据链预期 clean，Construction 当日实测确认」+时间线修正[533e456 早于 2a02050]）/M6（缓存体积/10GB 淘汰风险节+timeout 建议+共享键隔离备选）/OQ 清理（关闭 OQ⑤ 冗余；cargo test 标注「默认排除，选入需 PRD 修订」；runs-on/cache key/脚本名按裁决与先例落字面=ci-desktop-yaml.test.sh）。**pm**：B1（AC1① 字面改 --manifest-path 完整形态）→OQ① 裁决回填（AC1 增形态/runs-on 子项）→AC1⑤ 负断言枚举补全（status 函数/shell 级/if: false）→AC3「断言不得弱化」操作定义→AC2 dirty 分支准出条件（OQ③ 裁决记录+修复批号/backlog 登记）→AC4 diff 面守卫声明→非目标补 cargo test 默认排除+脚本名同步 ci-desktop-yaml.test.sh+六脚本口径进 ops 声明→关闭 OQ⑤。**qa**：TP-01 字面改 --manifest-path+承接缓存精确断言（apps/desktop -> target）/TP-01 负断言枚举补全（status 函数/shell 级）→TP-02 补 diff 文件集断言+matrix/on:/env 护栏→TP-03 改「预期 clean；dirty 为偏差」+RUSTFLAGS 形态→TP-05 注入机制钉法（临时副本/清理）→TP-06 首跑渠道改 PR 优先+push 兜底→TP-04 进门同跑或升 P0→断言不得弱化定义。
  5. GATE-REJECTED 标记在后；修订完成后 gate1 复位 pending 复审（只核收敛点：2 BLOCKER 消解+两项裁决落实+MAJOR 修订），须新回执。

GATE-REJECTED gate1 2026-10-29
---
