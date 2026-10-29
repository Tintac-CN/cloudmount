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
