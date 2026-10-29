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

2026-10-29 Gate#1 复审 r2 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate1-ai-r2.md @ d98ba2e（reviewer-architect 0/3/5 + reviewer-qa 0/0/4——合并 0 BLOCKER / 3 MAJOR / ~6 MINOR。上轮 2 BLOCKER+8 MAJOR 全部消解实证：qa 机械 grep -F 三工件同串逐字命中；arch 逐项实测 #21-#41 与 ci.yml 实文全部命中；B2 值修正+精确断言；OQ① 落实；qa 侧 6 项全 PASS）。
- 呈批两问（逐字）：
  问①：「条目 23 Gate#1 复审（r2，只核收敛点）：上轮 2 BLOCKER+8 MAJOR 全部消解实证（B1 三工件同串逐字命中/B2 rust-cache 值修正+精确断言/护栏与负断言锚集补齐/现状统一 clean/OQ① 落实）；新发现 0 BLOCKER/3 MAJOR/6 MINOR。如何裁决？」
  答①：「**通过+准出回填（推荐）**」
  问②：「M-R2-1（OQ④）当场裁决：断言脚本是否纳入本批范围？（intent 原文「ci.yml 唯一改动面」与 A1 拆分预览③「断言脚本守护」存在张力——需人类定）」
  答②：「**脚本入批（推荐）**」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 最新 GATE-APPROVED gate1，满足 GATE-REJECTED gate1 2026-10-29 之后的新鲜度要求），phase → construction。
  2. **OQ④ 裁决=断言脚本入批**：新增 `scripts/ci-desktop-yaml.test.sh` 属本批文件面（diff 白名单 = `.github/workflows/ci.yml` + 本脚本）；intent 原文「ci.yml 唯一改动面」由本裁决显式扩面（与 A1 拆分预览③一致）；扩展已完结脚本（desktop-installers-yaml.test.sh）与「排除」（AC3 作废）均否决。
  3. **准出回填清单**（批准条件式文本级订正，编排者执行并核收留痕，不重走复审）：
     a. M-R2-1（OQ④ 三工件状态统一）：prd OQ④ `[ ]`→`[x]` 引本裁决 + AC3/AC4 条件化措辞收敛；architecture OQ④ 关闭理由改引人类裁决（删单方关闭形态）+ 改动面/决策 9/头注同步；test-strategy OQ④ `[ ]`→`[x]` + TP-02 白名单定名单分支 + TP-04 载体单分支 + 分层策略同步。
     b. M-R2-2（AC3「断言不得弱化」定义统一）：architecture 脚本头注改为 prd AC3 三条（①负断言必须经由 grep_absent_ok ②正断言不得由精确字面放宽为子串 ③脚本自检+CRLF 剥除+降级计数断言必须存在）+ 保留「计数不降/新增允许/弱化须人类批准」；对齐表 AC3 行改引 prd 三条。
     c. M-R2-3（断言清单补 3 条 → M 44→47）：新增 #45（desktop 块内无 `working-directory: apps/desktop`）/#46（desktop 块内无 `RUSTFLAGS` 覆写）/#47（全文件三 OS matrix 精确行计数=2 + desktop 块内无 `strategy:`/`matrix:`）；architecture 计数四处同步（清单头/数据模型/开放问题/头注）；红阶段编号改 #1-#16+#45/#46；M 计数口径钉死=静态 report 调用点数 47，#42 actionlint 分支差异按 112/113 先例以运行末行为准且 N==M。
     d. MINOR：prd 引用架构旧编号 3 处修正（#7/#8→#8/#9、#10→#11、#6→#7）；architecture AC2 实测节清除 cd 形态残留（对比命令改 --manifest-path 形态）；#12 从严口径明示（if: always() 属强制运行非跳过；本批蓝图零 if:；未来收紧须 decisions 留痕）；OQ② 两侧状态注记统一（architecture 实质收敛/PRD 留 `[ ]` 待人类逐字确认）。
  4. 回填核收（编排者内联复验，2026-10-29 21:20）：三工件 grep 零残留（无实义「M 基线 44」/无「gate1 后落定」/无条件化「若④」分支）；prd 编号引用 #8/#9、#11、#7 命中；architecture #45-#47 行齐 + M=47 四处；test-strategy OQ④ 单分支。核收通过。
  5. 后续：phase=construction，`/aidlc:construct` 进入 Sprint Planning（A3 拆分确认在派发执行泳道前呈出）。

GATE-APPROVED gate1 2026-10-29
---

2026-10-29 Gate#2 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate2-ai.md @ 227646b（reviewer-pm 0/0/5 + reviewer-architect 0/0/4 + reviewer-qa 0/0/7 + reviewer-dev 0/0/8 + verifier 0/0/3；合并去重 0 BLOCKER / 0 MAJOR / 15 MINOR。五方独立复跑：断言脚本 47/47、既有五脚本 112/72/23/13/9、cargo test --workspace 277/0（26 目标）与 testBaseline 逐数一致零回退、产品面 diff 白名单守恒（ci.yml +33/−0；新脚本 +420/−0）、蓝图逐行 0 差异、TDD 红→锁→绿完整、工作区干净）。
- 呈批两问（逐字）：
  问①：「Gate#2 五方评审（reviewer-pm/architect/qa/dev + verifier）：0 BLOCKER / 0 MAJOR / 15 MINOR——交付与证据链自洽……MINOR 均为文档口径/后续硬化/ops 出口项。如何裁决？」
  答①：「**通过+准出回填（推荐）**」
  问②：「15 MINOR 之一：脚本头注「M 基线 = 47 个 report 调用点」措辞失准（实测静态调用点 49；运行期 M=47/48）。订正方式如何选择？」
  答②：「**保持锁定（推荐）**」
- 裁决落点（编排者执行）：
  1. gate2 = approved（本回执为 decisions.md 最新 GATE-APPROVED gate2；gate2 首轮审批，无既有 GATE-REJECTED gate2 作废问题），phase → operations。
  2. **准出回填清单**（批准条件式文本级订正，编排者执行并核收留痕，不重走复审）：
     a. **M 计数口径订正**（architecture r3 头注 + 断言清单头 + 计数口径段 + 数据模型 + 架构旧 OQ 关项）：47=断言编号数（#1-#47）；静态 `report` 调用点=49（#42 分支展开）；运行期 M=47（actionlint 不可用）/48（可用），以脚本运行末行为准且 N==M。
     b. **fmt `--all` 作用域事实订正**（verifier-m1 实测证伪）：`--all` 含 local path-based dependencies——实际检查 70 文件（desktop 4 + 根 workspace 66），非「不连带根 crate」；同步订正「覆盖增量/失败归因」叙述（无假绿/假红，仅归因稀释；`--all` 为 B1 裁决字面不得擅改）。
     c. **措辞收严**：`--locked` 未加（与既有 rust job 同纪律，lock 过期 cargo 可更新）；job 顺序无断言锁定（由 units/1.md 实现记录 + gate2 复核确认）。
     d. **OQ② 归属订正**：从「已关闭」小节移入「待人类裁决/待触发」（实质收敛、待人类逐字确认）；OQ③ 注明 2026-10-29 实测 clean 未触发。
     e. **TP-02 基线锚补记**：`<base>`=`2d5d695`（本 intent 开工基线=gate1 批准提交；TP-02 要求「Construction 记录于 decisions」，本条即载体；产品面 diff 白名单据此核验）。
     f. **backlog 登记**：条目 40（断言脚本硬化候选：负断言语法变体/job_block 注释截断/#43·#44 自检深度/头注 M 口径与 python 依赖措辞）；条目 41（首跑观察 + required checks）；条目 23 状态更新为「已交付（gate2 通过，待 ops 收口）」。
  3. **脚本头注 M 口径订正方式 = 保持锁定**（人类逐字裁决）：TDD 锁与脚本 hash 保持不动（锁后零改动审计链完整）；头注「M 基线 = 47 个 report 调用点」的准确口径由 architecture/decisions 本轮订正覆盖，脚本头注字面登记 backlog 条目 40 随下次解锁批（硬化批）订正。
  4. **Operations 出口条件**（AC3 后半句 + gate2 MINOR 收口）：ops-runbook/retro 必须收录「六脚本」硬门口径（112/72/23/13/9/47；本机 actionlint 不可用，desktop 脚本运行期 M=47）；记录 TP-06 首跑观察（desktop job 首跑绿、runner `rustc -V` 与本机对比、cache 体积/命中率与 10GB 配额）与 branch protection 未加入 required checks 的现状；M6 缓存体积 follow-up 保留。
  5. 后续：phase=operations，`/aidlc:ops`（route=classic——retro 硬性要求）。

GATE-APPROVED gate2 2026-10-29
---

2026-10-29 Operations 裁决收集（AskUserQuestion 三问，表单未获选择）：
- 问①「Operations-1：本批无部署面（唯一触发=CI 运行），「首跑观察窗口」如何定？」（首推当日核[推荐]／次日汇总核／不设时点）
- 问②「Operations-2：desktop job 的失败门控（是否加入 branch protection required checks）如何定？」（稳定后加 required[推荐]／首绿即加／不进 required）
- 问③「Operations-3：desktop 缓存体积的告警/处置阈值如何定？」（10GB+时长双阈值[推荐]／仅观察不设阈）
- **表单未获选择（非拒绝）**：如实记录「未获人类逐字裁决」；编排者按三问推荐项取**暂定缺省值**落 `ops-runbook.md` §5 并标注「暂定，待追认」；完结确认时一并追认或修订。本条不涉及 gate1/gate2 门控，无代答问题。
- 暂定值（逐项对应推荐项）：①首跑观察窗口=首推当日核验（无部署窗口=N/A）②required checks=首跑后累计 ≥3 次绿再由人类在仓库设置加入（本批不代操作仓库外设置）③缓存阈值=10GB + 既有四 job 时长双阈值，超限按 `shared-key: desktop` → `save-if`（仅 main 保存）→ `cache-targets: false` 顺序处置，变更 decisions 留痕。
- 工件落盘：`ops-runbook.md`（交付/触发面、监控告警、首跑观察清单含红态三分类、六脚本硬门口径 112/72/23/13/9/47、阈值与缓存处置顺序、回流与台账）+ `retro.md`（classic 硬性要求：什么有效/什么没用/学到的东西/下个 bolt）。
- 回流机制：生产事件/学习写新 intent 的 `intent.md` 或 `docs/aidlc/backlog.md`；已完结 intent 工件只读；本 intent 学习已全部入 `retro.md`。
- 完结确认呈出（待人类逐字裁决）：确认完结 → phase=done；否则继续观察或修订。
