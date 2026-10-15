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
