# Decisions: macos-release

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-15 A1 意图接入确认（AskUserQuestion，逐字转写）：
- 意图原话：`/aidlc mac环境在线上请使用GitHub的mac runner来实现打包与release。本地看看有模拟解决办法不`（逐字）。
- 识别的意图一句话：release.yml 补 macOS 打包腿（GitHub mac runner）接入 Release；本地建立编译面+脚本 dry-run 预检。
- 拆分预览（5 项）：①release.yml 增 binaries-macos job（macos runner 编译 daemon+cli → tar.gz，形态对齐 Linux 腿）②打包逻辑抽成 scripts/package-macos.sh（参数化产物目录，本地可 dry-run）③release job 依赖扩 macos、Release notes 产物表补 mac 行（含未签名/Gatekeeper 提示）④本地预检：双 target cargo check + dry-run 说明⑤（可选）desktop .app 打包是否纳入本期。
- 本地模拟结论（编排者核查，已呈用户）：真实 mac 链接/产物无法在本地 Windows 模拟（Apple SDK/链接器/codesigning 三重依赖；osxcross 法律灰且配置重不建议；nektos/act 仅 Linux 容器模拟不了 macos runner）——本地预检两道 = `rustup target add` 后 `cargo check --target aarch64/x86_64-apple-darwin`（check 不链接，抓 cfg/API 编译面）+ 打包脚本参数化 dry-run（Git Bash 占位产物走通 tar 布局，CI 调同一脚本）。仓库事实：release.yml 现有 linux 双架构/windows/docker 三类腿缺 mac；workspace 在 ci.yml macos-latest 腿已绿（mac 编译面健康）；desktop 为独立 cargo 项目（egui 跨平台，根 exclude）。
- 问：「意图「release.yml 补 mac 打包腿 + 本地预检」——是否进入 AIDLC 生命周期？」
  答：「进入生命周期（推荐）」
- 问：「规模分级路线（写入 intent 后 gate1 批准前仅可 express→classic 单向升级）：」
  答：「express（推荐）」
- 问：「mac 产物范围（若进入生命周期，此项作 inception 澄清输入）：」
  答：「daemon+cli 双架构（推荐）」
- 裁决落点（编排者执行）：建 intent `macos-release`（route=express）；desktop .app 不在本期（产物范围=daemon+cli 双架构）；Inception 不派 architect、不产 architecture.md，语义对齐并入 prd.md 内联小节；门控回执/decisions 逐字留痕/TDD 锁/hook 保护与 classic 完全一致。

---

2026-10-15 Inception 对齐拍记录（编排者）：
- express 两工件齐：prd.md（5 AC + 语义对齐内联小节 + 开放问题 3 项附建议默认值：Q1 runner 组合 macos-latest+macos-13 附退役风险与备选、Q2 命名 cloudmount-darwin-<arch>.tar.gz、Q3 edge.md 补最小 mac 小节）+ test-strategy.md（17 测试点，P0=10/P1=5/P2=2；负路径：缺文件 fail-fast/参数缺失/范围排除/预检阻断/语法拦截/Gatekeeper 预期拦截；E2E 真 tag 触发全部落人工核验清单）。
- **对齐拍检出映射错位**：qa 映射列按 A1 拆分预览锚定（预览 AC≠prd 实际 AC），pm 建议锚为预估编号——两角色各派回只做引用回填（测试点本体/场景/层级/优先级零改动）：T5→AC2、T6/T7/T8→AC3、T11→AC5、T12→AC4、T13→AC5、T14→AC1+AC5、T16→AC4；prd AC1..AC5 括注锚位改为实际 T 编号。
- 对齐后核对：AC 引用集合 = prd 实际集合 = {AC1..AC5}，无虚构编号；全部 AC 有 T 点落点（AC1=T9/T14/T15、AC2=T1/T2/T3/T4/T5/T10、AC3=T6/T7/T8、AC4=T12/T16、AC5=T11/T13/T14），无缺锚。
- 相位转移：两工件提交（@ 624e1a6）→ phase 置 gate1（express 评审集合 = prd + test-strategy 两件），提示 `/aidlc:gate`。

---

2026-10-15 Gate#1 人类审批（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 互审输入：`reviews/gate1-ai.md` @ 340ce70（reviewer-pm 1/1/6 + architect 0/1/6 + qa 0/4/6 + dev 1/2/4；合并 1 BLOCKER / 6 MAJOR / 16 MINOR）。
- 呈批四问（全部经 AskUserQuestion 呈出，用户对话一句话裁决）：
  问：「Gate#1 互审：1 BLOCKER（本地双 target cargo check 预检因 bundled C build scripts 不可行）+6 MAJOR（tar 布局字面矛盾/macos-13 退役/锚位不一致/断言可判定性缺口）+16 MINOR。如何裁决？」
  问：「BLOCKER B1 修复方向（本地 cargo check 双 darwin target 不可行）：」
  问：「开放问题 Q1（runner 组合，事实已刷新：macos-13 已完全退役、-latest 处迁移期）：」
  问：「开放问题 Q2（产物命名）与 Q3（edge.md mac 小节）——采建议默认还是调整？」
  答（对全部四问）：「打回，按全部推荐」
- 「全部推荐」映射（编排者如实展开，用户一句话采纳）：
  1. 门裁决 = **打回修订**（gate1=rejected，phase 回 inception；只重跑被点名角色 pm+qa，对齐拍后 gate1 复位 pending 复审，重点核对 B1/M1/M2 消解）。
  2. B1 修复方向 = **预检上移 CI**：新增 workflow_dispatch 触发的 mac runner check job（双 target 编译面验证，推 commit 即可手动触发不需 tag）；本地预检降为 dry-run 一道，AC3/T6-T8 判据同步改写。
  3. Q1 = **pin macos-15（arm64）单 runner 双 target**：rustup 装 x86_64-apple-darwin target 交叉编译（Apple clang 原生支持，arch/dev 双重复核无硬伤）；省 runner 额度，T9 断言随语义级改写。
  4. Q2 = **产物命名 `cloudmount-darwin-<arch>.tar.gz`（短 arch）**（如实描述：非 linux 腿 target-triple 同构）；Q3 = **docs/edge.md 补最小 mac 小节**（约 10 行），同步扩 intent.md 改动面声明（docs/edge.md）。
- 裁决落点（编排者执行）：
  1. gate1 打回：只重跑 pm+qa 修订——pm：B1（AC3① 判据改写为 CI workflow_dispatch mac check、本地预检降为 dry-run 一道）+ M1（AC2 布局字面统一为 tar 内根级 `README.md`/`edge.md`，与 Linux 腿同构）+ Q1/Q2/Q3 裁决值回填（开放问题转裁决记录）+ arch 措辞项随语义对齐小节顺带订正（rust-cache 表态、`-p` 同构、ad-hoc 签名措辞、needs 门控术语+matrix `fail-fast: false`、tag 规范非目标行、README 安装表 mac 行记非目标由 edge.md 小节承载）+ AC1 锚补 T10/T17；qa：M3（锚位双向一致）+ M4（T4 显式枚举完整期望清单+systemd 排除，SOP 同步）+ M5（T10 断言 `if-no-files-found: error` + glob=cloudmount-darwin-<arch> 写死）+ M6（T11 补"release job 不带 if:/status 覆盖"静态断言）+ T6/T7/T8 判据随 B1 改写（check job 静态结构断言+首次手动触发线上人工验证；预检说明文档提 P1）+ MINOR 落点（T16 真机前提、T14 资产集合不变、SOP file 子串判据+体积区间、T9 按 Q1 裁决语义级改写、dry-run 隔离并入 T1、缺文档负路径）。
  2. intent.md 同步扩改动面声明（docs/edge.md 入受影响系统）；Q1/Q2/Q3 从开放问题转已裁决。
  3. GATE-REJECTED 标记在后；复审前 gate1 复位 pending。

GATE-REJECTED gate1 2026-10-15

---

2026-10-15 Gate#1 修订轮记录（编排者）：
- 修订范围（只重跑被点名角色 pm+qa；其余工件零改动；修订提交 @ 1708fdb）：
- **prd.md**：AC3 整条改写（B1 裁决：编译面预检上移 CI——workflow_dispatch mac check job 双 target，本地预检降为 dry-run 一道+预检说明入仓，bundled C 因果注明）；AC2 布局字面定稿（M1：tar 内根级 `cloudmountd`/`cloudmount`/`README.md`/`edge.md`，扁平同构 linux 腿）；Q1/Q2/Q3 转已裁决回填；AC1 锚补 T10/T17（M3）；语义对齐小节订正（`-p` 同构、rust-cache 启用表态、"无 Developer ID 签名/公证"措辞、AC5 needs 门控术语+matrix `fail-fast: false`、缺文档硬失败）；非目标补两行（tag `v*` 规范、README 安装表 mac 行不在本期）；连带同步三处（US3 字面、非目标 edge.md 旧行移除、语义对齐 tar/本地模拟行随 B1/M1）。
- **test-strategy.md**：T6 改集成静态断言（workflow_dispatch mac check job 结构）+ T7 改线上人工 dispatch 触发 + T8 收窄 dry-run 一道+预检说明提 P1（B1）；T4 完整四项清单断言+systemd 排除（M4，SOP 同步）；T10 `if-no-files-found: error` + glob `cloudmount-darwin-*.tar.gz` 写死（M5）；T11 补 release job 无 `if:`/status 覆盖断言（M6）；T9 按 Q1 裁决语义级改写（同一 macos-15 job 双 target 产物存在）；T2 扩缺任一必含文件、T1 并入 dry-run 隔离断言、T14 补资产集合不变、T16 标真机前提、SOP `file` 子串判据+体积区间 5–50MB；计数 P0=10/P1=6/P2=1（T8 P2→P1）。
- 修订轮对齐拍：AC↔T 双向核对通过（AC1=T9/T10/T14/T15/T17 含 T10 双锚 AC1/AC2、T17 单锚 AC1；AC2=T1-T5/T10；AC3=T6/T7/T8；AC4=T12/T16；AC5=T11/T13/T14），计数一致；上轮 B1/M1/M2/M3/M4/M5/M6 修订点全部可见于字面。
- 相位转移：修订轮两工件提交（@ 1708fdb）→ gate1 复位 pending（重审以新一轮为准；GATE-REJECTED gate1 2026-10-15 之后须新回执），phase → gate1。复审重点核对 B1/M1/M2 消解。

---

2026-10-15 Gate#1 复审人类审批（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 互审输入：`reviews/gate1-ai-r2.md` @ 4b4d99e（reviewer-pm 0/0/3 + architect 0/1/3 + qa 0/1/2 + dev 0/1/2；合并 0 BLOCKER / 1 MAJOR / 8 MINOR；上轮 1 BLOCKER/6 MAJOR/16 MINOR 经四方字面+grounding 双验全部消解实证）。
- 呈批两问（AskUserQuestion 呈出，用户对话一句话裁决）：
  问：「Gate#1 复审（r2）：上轮 1 BLOCKER/6 MAJOR/16 MINOR 全部消解实证；新检出 1 MAJOR（check job 载体未钉死——release.yml 内联读法会致 GHCR :latest 覆盖/伪 Release，且与 T11 无-if: 断言互锁）+ 8 MINOR（措辞/断言收尾级）。如何裁决？」
  问：「R2-MAJOR 裁决：mac check job 的载体（workflow 文件）钉死为？（若上一问选打回，此项作为修订轮输入同样生效）」
  答（对两问）：「通过，独立 workflow 文件」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 中最新 GATE-APPROVED gate1，2026-10-15 GATE-REJECTED gate1 的新鲜度要求由本回执满足），phase → construction。
  2. **R2-MAJOR 钉死裁决**：mac check job 载体 = **独立 workflow 文件 `.github/workflows/check-macos.yml`**（仅 `workflow_dispatch` 触发 mac check job；`release.yml` 的 `on:` 保持仅 `tags: v*` 零副作用）；intent.md 改动面随批准扩入该文件；AC3/T6 落点字面随 Construction 首个 unit 顺带回填钉死。
  3. **8 MINOR 授权随 Construction 首个 unit 顺带回填**（gate1 r3 先例）：①AC5 括注收紧为"固定单 runner 顺序步（Q1 裁决），不采用 matrix"（qa 可改为 T9 补条件断言，二选一）②T12/T16/SOP"未签名"→"无 Developer ID 签名/公证（arm64 ad-hoc）"③T4 补 tar `./` 归一化半句（剥前缀/忽略目录项）④AC5 句尾"fail-fast 语义"→"needs 门控语义"⑤SOP 补发布前置第 0 步"tag 前手动触发 dispatch check 并确认绿"⑥T15 同步四项清单⑦SOP 体积判据改"超出 5–50 MB 带即人工复核"（消 50–200MB 灰区）⑧T9 单 runner 约束断言（与①合并落点）。
  4. 后续：phase = construction，`/aidlc:construct` 进入 Sprint Planning（express 路线：A3 拆分确认在派发 dev 泳道前呈出——本 intent 经 A1 接入，A3 表单适用）。

GATE-APPROVED gate1 2026-10-15

---

2026-10-15 A3 拆分确认（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 呈批对象：plan.md @ 68766e9（4 units 串行 TDD——U1 打包脚本/U2 release.yml mac 腿/U3 check-macos.yml 独立载体/U4 edge.md+回填收尾；验证命令=cargo test --workspace，基线 26 目标 277 passed 0 failed @ 2d1c501；串行派发沿 audit-p0p1-fixes 限流教训）。
- 问：「A3 拆分确认（intent macos-release，4 units 串行 TDD）：是否批准开始施工？」
  答：「批准」
- 裁决落点（编排者执行）：
  1. PLAN-CONFIRMED 回执在后；开始串行派发 U1→U2→U3→U4。
  2. 施工细节订正（编排者，plan.md 已同步）：U3 的 T6 断言落**独立新文件** `scripts/check-macos-yaml.test.sh`（原计划"追加至 release-yaml.test.sh"会与该文件 TDD 锁冲突——锁后追加即违约；独立文件各自置锁，免解锁仪式）。零范围变化，纯测试文件落点调整。

PLAN-CONFIRMED 2026-10-15

---

2026-10-15 Gate#2 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate2-ai.md` @ a694d3c（verifier 0/0/0+3INFO + architect 0/0/2 + qa 0/0/3 + dev 0/1/5 + pm 0/0/2；合并 0 BLOCKER / 1 MAJOR / 10 MINOR。verifier 独立复跑终态全部属实：脚本 9/9·22/22·13/13、cargo test 26 目标 277 passed 0 failed 零回归、fmt/clippy 净、TDD 红绿纪律 git 史核验合格、10 文件全部归属、testBaseline 逐字节未动）。
- 问：「Gate#2 互审：verifier 独立复跑终态全部属实（脚本 9/9·22/22·13/13、cargo 277/0 零回归、TDD 纪律合格、零归属外文件）；合并 0 BLOCKER / 1 MAJOR（--arch 零断言覆盖——CI 命名正确性依赖未断言的传参字面，实现在案且正确）/ 10 MINOR（记录级）。如何裁决？」
  答：「通过+准出回填（推荐）」
- 裁决落点（编排者执行）：
  1. gate2 = approved（本回执为 decisions.md 中最新 GATE-APPROVED gate2），phase → operations。
  2. **准出回填授权**（批准条件，编排者执行并留痕）：
     a. M1：解锁仪式（本条目即解锁留痕依据）——`scripts/release-yaml.test.sh` 临时自 tddLockPaths 移除 → 补 `--arch "${t%%-*}"` 结构断言（计入断言总数，CI 传参字面在位即绿）→ 复跑绿 → 重新并集置锁；
     b. MINOR 顺带：T3 字面与实现对齐（缺参→usage；路径非法→专用报错非零）/T16 补"/公证（arm64 ad-hoc）"括注/units/3.md 订正为独立断言文件/package-macos.sh 头注补 chmod +x 步骤/edge.md mac 小节补登录前置一句；
     c. 其余 MINOR 备案 P2：check-macos.yml permissions 显式化/断言脚本 YAML 解析精度/tar 半成品清理/行尾风格 .gitattributes。
  3. 追认项打包核查：本 intent Construction 无配额降级代行、无解锁记录（三 unit 严格红→锁→绿；本条 a 项解锁为准出回填授权行为，非契约错误解锁）——无历史追认项需呈。

GATE-APPROVED gate2 2026-10-15

---

2026-10-15 Operations 人类裁决（AskUserQuestion，逐字转写）：
- 问：「发布节奏与首次验证（部署窗口类；发布 SOP 第 0 步 dispatch check 前置已在案）：」
  答：「手动 tag+首tag prerelease（推荐）」
- 问：「监控与观察项档位（SLA 类；本 intent 无运行时面，监控对象=CI/发布链路）：」
  答：「轻量+两项观察（推荐）」
- 问：「retro.md 裁决（route=express 分支：retro 为人类可选，gate2 批准时未一并裁决，现补）：」
  答：「保留 retro（推荐）」
- 裁决落点（编排者执行）：
  1. `ops-runbook.md` 落盘（@ 4c3f07b）：发布 = 手动 tag（无固定窗口）+ **首个含 mac 腿的 tag 用 prerelease 标记预演 T14-T16**（两 tar 可下载/四项清单/file 架构子串/notes mac 行，核验通过后转正式——消解 test-strategy 缺口节"首次即生产"开放问题）；监控 = 轻量零新增基建（硬门=CI 双腿绿+dispatch check 绿+三脚本断言 9/23/13）+ 两项观察（macos-15 runner 可用性——退役/迁移时 runs-on 字面与 T9 断言同步改、actionlint 缺口——CI 无 actionlint 步骤，语法错由 GitHub Actions 解析失败兜底不误发布）；回退 = Release 页删资产/revert 提交；P2 backlog 五项（ci.yml actionlint/permissions 显式化/断言精度/tar 半成品/.gitattributes）。
  2. `retro.md` 落盘（@ 4c3f07b，express 分支人类裁决=保留）：什么有效（A1-A3 全流程首通/B1 设计门拦截/R2-MAJOR 钉死/三轮严格红锁绿/verifier 零失实证/准出回填解锁样例）；什么没用（AC3 初版技术硬伤靠互审救回/AskUserQuestion 三次未答停摆/基线首跑 tail 截断重跑/--arch 契约外扩展晚发现）；学习五条（bundled C 使跨 target check 不可行的前提验证/workflow_dispatch 触发全部 job 落独立文件/matrix 展开多 runner 须钉顺序步/新断言落独立文件免解锁/准出回填模式）；下个 bolt 三项（P2 backlog 优先 actionlint+行尾/首个 tag prerelease 预演/表单未答降级路径）。
  3. 回流机制：发布事件/CI 异常/新需求经 `/aidlc:inception` 写新 intent；本 intent 学习沉淀 retro.md。
  4. 完结确认呈出（retro 已落盘，phase=done 待人类确认）。

---

2026-10-15 Intent 完结确认（AskUserQuestion，逐字转写）：
- 问：「intent「macos-release」（mac runner 打包与 release + 本地预检）完结确认：4/4 units 交付、两门均过（含准出回填）、ops-runbook 与 retro 已落盘、线上人工项（T7 dispatch check/首 tag prerelease 预演）已入发布 SOP。确认 phase 置 done？」
  答：「确认完结（推荐）」
- 完结落点（编排者执行）：phase = done；本 intent 退出活跃位（session-context 不再注入、resume 不再选中）；工件链（intent.md / prd.md / test-strategy.md / plan.md / units/ / reviews/ / ops-runbook.md / retro.md）与 decisions.md 原样保留作审计；发布事件与后续学习经 `/aidlc:inception` 写新 intent（回流机制见 ops-runbook.md）。

INTENT-DONE macos-release 2026-10-15
