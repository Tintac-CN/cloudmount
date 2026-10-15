# Decisions: installer-packages

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-15 A1 意图接入确认（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 意图原话：`windows需要提供msi安装包，mac也需要。请继续推进自动化发布和打包工作，tag即发布`（逐字）。
- 识别的意图一句话：Windows 提供 MSI 安装包、mac 提供安装包（pkg/dmg），扩展 release.yml 自动化发布——tag 即发布。
- 拆分预览（5 项）：①Windows MSI——windows runner 上 cargo-wix（WiX 封装）生成 `.wxs` 并打包 cloudmountd.exe+cloudmount.exe → `cloudmount-windows-x64.msi`（Program Files+PATH；SCM 服务注册留 `cloudmountd service install` 用户命令）②mac 安装包——macos runner 上 pkgbuild/productbuild（原生零新依赖）打包 daemon+cli → `cloudmount-darwin-<arch>.pkg`（/usr/local/bin；.dmg 备选 gate1 裁决）③release.yml 扩展——两腿各增安装包 job、产物上传 + Release 产物表补 MSI/pkg 行（无证书签名状态说明）④结构断言测试（TDD）——新断言脚本守护 MSI/pkg job 结构与产物命名⑤发布语义——「tag 即发布」= push tag 直接正式发布（是否覆盖上 intent「首 tag prerelease 预演」运维裁决——呈请裁决）。
- 仓库事实（编排者核查，已呈用户）：全仓零 wix/msi/pkg/dmg 痕迹（安装包打包全新领域）；daemon 已有 `service install/start/stop/uninstall` 子命令（SCM 注册应用自管，安装器只需铺文件+PATH）；desktop 为独立 cargo 项目（egui，上 intent A1 已排除出 mac 产物）；上 intent（macos-release）已交付 release.yml mac 腿/check-macos.yml/package-macos.sh 与三断言脚本（TDD 锁定）。
- 呈批四问（AskUserQuestion 呈出，用户对话一句话裁决）：
  问：「意图「MSI/pkg 安装包 + tag 即发布自动化」——是否进入 AIDLC 生命周期？」
  问：「规模分级路线：」
  问：「安装包产物范围（与上 intent 同口径为 daemon+cli）：」
  问：「「tag 即发布」语义（是否覆盖上 intent的首 tag prerelease 预演裁决）：」
  答（对全部四问）：「进入，express，daemon+cli，直接正式发布」
- 裁决落点（编排者执行）：建 intent `installer-packages`（route=express）；产物范围=daemon+cli（MSI/pkg 只装 cloudmountd+cloudmount；SCM 服务注册留用户命令；desktop 不入本期）；「tag 即发布」= push `v*` tag 直接触发全流水线并发布正式 Release——**覆盖 macos-release intent ops-runbook 的「首 tag prerelease 预演」裁决**（该 runbook 为已完结 intent 工件不改，以本条为准）；Inception 不派 architect、不产 architecture.md，语义对齐并入 prd.md 内联小节；门控回执/decisions 逐字留痕/TDD 锁/hook 保护与 classic 完全一致。

---

2026-10-15 Inception 对齐拍记录（编排者）：
- express 两工件齐：prd.md（6 AC + 语义对齐内联小节 + 开放问题 4 项附建议默认值：Q1 mac 形态 .pkg（推荐）/Q2 Windows 工具链 cargo-wix（推荐，供应链面仅 CI 侧，workspace bin 支持形态 Construction 首日验证）/Q3 MSI 写 PATH（推荐）/Q4 安装位置 Program Files\CloudMount + /usr/local/bin（推荐））+ test-strategy.md（22 测试点，P0=14/P1=7/P2=1；负路径：构建失败→needs 门控不发布（含无 continue-on-error 旁路防护）/产物缺文件不产出/既有腿零删改护栏（needs 护栏+三锁定脚本回归+既有资产核验）/desktop 与服务注册排除/发布语义负断言（无 prerelease:true/draft:true+on: 守护）/SmartScreen/Gatekeeper 预期拦截；MSI 静默装/卸=线上产物人工核验但执行环境=本机 Windows（如实标注）；mac pkg 只能真机）。
- **对齐拍检出双向错位**：qa 映射列按 A1 拆分预览 5 项锚定（prd 实际 6 条 AC），pm 建议锚为预估编号——两角色各派回只做引用回填（测试点本体零改动）：T11→AC3/AC6、T12→AC4、T14→AC6；prd AC1..AC6 括注锚位改为实际 T 编号（AC1=T1/T2/T3/T4/T16/T17/T18/T22、AC2=T5-T8/T19/T20、AC3=T9/T10/T11/T15、AC4=T12、AC5=T13/T17/T19/T21、AC6=T11/T14）。
- 对齐后核对：AC 引用集合 = prd 实际集合 = {AC1..AC6}，无虚构编号；全部 AC 有 T 点落点，无缺锚（T15 为整文件校验的 AC1/AC2/AC3 多锚、T17/T19 双锚——集合层一致）。
- 相位转移：两工件提交（@ d9dbb41）→ phase 置 gate1（express 评审集合 = prd + test-strategy 两件），提示 `/aidlc:gate`。

---

2026-10-15 Gate#1 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate1-ai.md` @ c8fb644（reviewer-pm 0/4/5 + architect 0/4/8 + qa 1/3/8 + dev 0/3/7；合并 1 BLOCKER / 8 MAJOR / 15 MINOR）。
- 呈批四问与用户裁决：
  问：「Gate#1 互审：1 BLOCKER（B1 artifact 名零锚——错名将静默产出缺安装包的正式 Release）+8 MAJOR（守护口径冲突/前置硬校验无锚/升级披露不实/双 SOP/路径悬空/WiX v3 EOL 等）+15 MINOR。如何裁决？」
  答：「打回修订（推荐）」
  问：「开放问题 Q1-Q4（互审建议 gate1 显式拍板而非默认通过；Q2 已计入 WiX v3 EOL 权衡）：」
  答：「pkg+直接WiX+PATH（推荐）」
  问：「互审新钉死项（M3/M4/M7 落点，修订轮回填字面）：」
  答：「全部采推荐（推荐）」
  问：「T16 定级与 SKIP 判据（qa M2）：」
  答：「升 P0+SKIP留痕（推荐）」
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception）：只重跑 pm+qa 修订被点名项，对齐拍后 gate1 复位 pending 复审（重点核对 B1/M1/M2/M7 消解）。
  2. **Q1-Q4 裁决**：Q1 = mac 形态 `.pkg`；Q2 = **手写 `.wxs` + CI 直接 WiX**（计入 WiX v3 EOL 与双 bin 必手改 .wxs 的权衡——cargo-wix 不采用，仅 CI 侧装 WiX 工具链，安装方式与版本锁定 Construction 落）；Q3 = MSI 写 Machine 级 PATH；Q4 = 安装位置 `Program Files\CloudMount`（Windows）/ `/usr/local/bin`（mac）。
  3. **钉死项裁决（M3/M4/M7 落点）**：安装器定义 = `packaging/wix/main.wxs`（仓库集中惯例）；mac 打包脚本 = `scripts/package-pkg-macos.sh`（沿 package-macos.sh 先例）；新断言脚本名统一 = `scripts/installers-yaml.test.sh`（复数）；安装器版本取值 = `github.ref_name`（不依赖 Cargo.toml bump）；SOP 载体 = operations 阶段产出本 intent runbook 显式声明取代 macos-release runbook 的发布流程节。
  4. **T16 裁决**：升 P0；SKIP 需显式人工裁决记 decisions.md（含安装尝试与失败输出留痕）。
  5. 修订范围点名——pm：AC3 可测性拆解改口径（新脚本承接既有产物名锚点，与锁定脚本重复无害）/非目标升级措辞改「不设计不保证，Release notes 提示先卸载再装」+版本取值 github.ref_name 入语义对齐/SOP 载体声明/AC6 脚本名统一复数/AC4 行数口径（MSI 1 行+pkg 每 arch 一行共 3 行）+PATH 刷新提示+UAC 预期+rc tag 语义+pkg 无 install 脚本声明+AC3③ 追加口径（签名段追加新行不改既有段）/开放问题 Q1-Q4 转已裁决；qa：B1（T2/T6 补 artifact name 断言）/M1（T11 措辞消歧）/M2（T6 upload path 钉双字面精确路径+前置硬校验断言）/M6（T20 降级判据 xar! magic+体积带+真机补验触发）/M7（断言对象路径钉死+脚本名统一）/T16 升 P0+SKIP 留痕/T8 补 launchd 负断言/T12 双 arch 字面+AC4 负路径/T21 映射补 AC4/T9 补无 if:/status/T10-T11 矛盾消解/T18 判据操作化（OS 新开终端+管理员 shell+command -v）/头部与缺口节同步。
  6. GATE-REJECTED 标记在后；修订轮完成后 gate1 复位 pending 复审。

GATE-REJECTED gate1 2026-10-15

---

2026-10-15 Gate#1 修订轮记录（编排者）：
- 修订范围（只重跑被点名角色 pm+qa；其余工件零改动；修订提交 @ 68a58d2）：
- **prd.md**：M1（AC3 拆解改「新脚本承接既有产物名锚点——双保险」）/M3（非目标升级措辞改「不设计不保证，Release notes 提示先卸载再装」+语义对齐补版本取值 github.ref_name）/M4（AC5+语义对齐补 SOP 载体声明——operations 新 runbook 取代 macos-release 发布流程节）/M7（AC6 脚本名钉死 installers-yaml.test.sh 复数+语义对齐钉死 packaging/wix/main.wxs 与 scripts/package-pkg-macos.sh）/M8-Q2（开放问题四项转已裁决+Windows 工具链段删 cargo-wix 表述、保留 WiX 安装锁定 Construction 落）/MINOR（AC4 行数 3 行+PATH 新开终端+UAC+先卸载再装提示、AC5 rc tag 语义、AC2 pkg 无 install 脚本、AC3③ 追加口径、AC1 仅产出负断言口径）；连带一致性三处（AC1 拆解/非目标 .dmg 行/首日验证条目随 Q2 裁决同步）。
- **test-strategy.md**：B1（T2/T6 补 upload artifact name 以 cloudmount- 开头断言——镜像上 intent 先例）/M2（T6 upload path 钉双字面精确路径+T2/T6 补前置硬校验断言）/M1（T11 改双保险口径+允许被修改面包含式护栏——消 T10/T11/T13 自相矛盾）/M3（T3 补版本取 github.ref_name 断言）/M5（T16 升 P0+SKIP=显式人工裁决留痕；随 Q2 裁决改 WiX 冒烟）/M6（T20 降级判据 xar! magic+体积带+真机补验触发入 SOP）/M7（T3/T4/T8 断言对象钉死=release.yml job 块+packaging/wix/main.wxs / scripts/package-pkg-macos.sh）/MINOR（T8 launchd 负断言、T12 行数口径+双平台签名覆盖负路径+误导措辞负断言、T21 映射补 AC4、T9 无 if:/status+continue-on-error 白名单收窄、T18 判据操作化）；头部/缺口节同步（Q1-Q4 已裁决、钉死项写死、SOP 载体）；计数 **P0=15/P1=6/P2=1**（T16 升 P0）。
- 修订轮对齐拍：AC 引用集合 = {AC1..AC6} 无幻影；prd 锚位与映射列双向核对通过（T21 映射列补 AC4 为 qa 侧超集——集合层一致）；钉死字面（installers-yaml.test.sh、packaging/wix/main.wxs、scripts/package-pkg-macos.sh、github.ref_name）双文件在案。
- 相位转移：修订轮两工件提交（@ 68a58d2）→ gate1 复位 pending（重审以新一轮为准；GATE-REJECTED gate1 2026-10-15 之后须新回执），phase → gate1。复审重点核对 B1/M1/M2/M7 消解。

---

2026-10-15 Gate#1 复审人类审批（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 互审输入：`reviews/gate1-ai-r2.md` @ f944ee0（reviewer-pm 0/0/5 + architect 0/0/5 + qa 0/0/5 + dev 0/1/8；合并 0 BLOCKER / 1 MAJOR / 12 MINOR；上轮 1 BLOCKER/8 MAJOR 经四方逐项实证全部消解——含先例字面验证与三锁定脚本基线实跑）。
- 呈批一问（AskUserQuestion 呈出，用户对话一句话裁决）：
  问：「Gate#1 复审（r2）：上轮 1 BLOCKER/8 MAJOR 全部消解实证（含先例字面与基线脚本实跑）；新检出 1 MAJOR（修订新引入的 dry-run 语义边界措辞——一句话可修，无需设计决策）+ 12 MINOR（断言枚举/措辞级）。如何裁决？」
  答：「通过」
- 裁决落点（编排者执行；「通过」按呈批推荐选项语义=通过+准出回填）：
  1. gate1 = approved（本回执为 decisions.md 中最新 GATE-APPROVED gate1，2026-10-15 GATE-REJECTED gate1 的新鲜度要求由本回执满足），phase → construction。
  2. **准出回填授权（批准条件，编排者执行并留痕）**：
     a. R2-MAJOR-1：prd「安装（pkg）」条补 dry-run 边界声明——「dry-run 仅覆盖 staging+硬校验段，pkgbuild 调用段 mac 独占、本地不覆盖」（纯事实边界，无需设计决策）；
     b. MINOR 1-11 授权随 Construction 首日回填：①AC4 三提示关键词入 T12 断言枚举②pkg 版本断言补 AC2 侧③T11 承接清单补收编链中段三字面（pattern/merge-multiple/files）④T9 白名单措辞消歧⑤SOP 清单②补管理员 shell 前置⑥T6 归因措辞如实化⑦T1/T5 与头部 runner pin 口径统一⑧T8 补 preinstall/postinstall 负断言⑨WiX 大版本选择入首日验证⑩T3 .wxs 断言 token 钉（ProgramFilesFolder/Environment/System="yes"）⑪T2 前置硬校验 OR 逃生口收窄为具体字面；
     c. MINOR 12 三项遗留（元数据 owner/pkg identifier 钉死、MSI job 构建来源明示、UTF-8 中文 description 风险）随 Construction 首日验证一并落。
  3. 后续：phase = construction，`/aidlc:construct` 进入 Sprint Planning（A1 接入 intent——A3 拆分确认表单在派发 dev 泳道前呈出）。

GATE-APPROVED gate1 2026-10-15

---

2026-10-15 A3 拆分确认（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 呈批对象：plan.md @ 7f6a550（4 units 串行 TDD——U1 安装包结构断言脚本红/U2 Windows MSI 腿含 T16 冒烟/U3 mac pkg 腿/U4 终验收尾；验证命令=cargo test --workspace，基线 26 目标 277 passed 0 failed（@ 03f85f1 记录）；MINOR 12 落位值钉死进 plan：owner=tintac-cn、identifier=io.github.tintac-cn.cloudmount、MSI job 构建来源=重编译+rust-cache；串行派发沿限流教训）。
- 问：「A3 拆分确认（intent installer-packages，4 units 串行 TDD）：是否批准开始施工？」
  答：「批准」
- 裁决落点（编排者执行）：PLAN-CONFIRMED 回执在后；开始串行派发 U1→U2→U3→U4。

PLAN-CONFIRMED 2026-10-15

---

2026-10-15 T16 冒烟实证与锁修订授权（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- T16 执行事实（dev 实测，编排者核收）：本机安装 WiX v3（choco wixtoolset，解析 **v3.14.1.20250415**，candle/light 3.14.1.8722）；本地构建实测两留痕——LGHT0368（双 File 组件自动 GUID 不合法→拆一文件一组件，WiX 最佳实践）、ICE80×3（`ProgramFilesFolder` 被 ICE 归 32 位标准目录与 x64 组件冲突）；MINOR⑫ 验证：UTF-8 BOM+中文注释 candle 零告警。
- **冒烟判据冲突实证**：变体 A（按锁定 T3 字面 `ProgramFilesFolder`+`-sice:ICE80` 抑制构建）msiexec 机制 PASS（安装退出码 0/ARP 正常/机器 PATH 写入/卸载全清）但落位 **FAIL**——两 exe 落 `C:\Program Files (x86)\CloudMount`，**不满足 prd Q4/AC1/T18 判据「Program Files\CloudMount」**（WiX v3 的 x64 包下 ProgramFilesFolder 运行时仍解析 32 位 Program Files）；变体 B（`ProgramFiles64Folder`，无抑制）对照冒烟**全项 PASS**（构建零告警/`C:\Program Files\CloudMount` 双 exe 在位/机器 PATH 正确/新进程 `where cloudmount` 命中/卸载退出码 0 目录+PATH 全清）。
- 问：「T16 冒烟暴露契约冲突：WiX v3 x64 包下 ProgramFilesFolder 实测落 C:\Program Files (x86)（不满足 prd Q4/AC1/T18），正确修复需改锁定 T3 断言字面。如何裁决？」
  答：「授权锁修订」
- 裁决落点（编排者执行）：
  1. **解锁仪式**（授权依据=本条人类裁决）：`scripts/installers-yaml.test.sh` 临时自 tddLockPaths 移除，用于 T3 断言字面修订（`ProgramFilesFolder` → `ProgramFiles64Folder`——MINOR⑩ 本意即 Program Files 安装语义，64 变体为 x64 正确形式）；断言修订+实现修正在案后重新并集置锁。
  2. dev 提交 U2-wixfix：①`packaging/wix/main.wxs`（组件拆分一文件一组件+`ProgramFiles64Folder`+头注三留痕：LGHT0368/ICE80/x64 落位实测）②`.github/workflows/release.yml`（移除 `-sice:ICE80` 抑制及注释、版本锁定回写 3.14.1）③`scripts/installers-yaml.test.sh`（T3 断言字面 64Folder）——授权依据随提交信息留痕。
  3. 变体 A 的 ICE80 抑制路线废弃（实测为错误方向——抑制掩盖 x64 落位错误）。

---

2026-10-15 Gate#2 人类审批（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 互审输入：`reviews/gate2-ai.md` @ 8998b01（verifier 0/0/2+2INFO + architect 0/1/4 + qa 0/0/5 + dev 0/0/4 + pm 0/0/3；合并 0 BLOCKER / 1 MAJOR / 12 MINOR。verifier 七条复跑+红态独立复现（b40eaf0 全树实测 18/72+54 FAIL 与记录逐数一致）+TDD 纪律含授权锁修订合规核验+6 文件归属+testBaseline 逐字节未动——全部属实）。
- 问：「Gate#2 互审：verifier 七条复跑+红态独立复现全部属实（脚本 9/9·23/23·13/13·72/72、cargo 277/0 零回归、TDD 纪律合格、零归属外文件）；合并 0 BLOCKER / 1 MAJOR（CI WiX 版本软锁——一行 choco --version 硬锁可修，失败模式=显式失败非静默坏包）/ 12 MINOR（记录精度/对称性/枚举级）。如何裁决？」
  答：「通过+准出回填（推荐）」
- 裁决落点（编排者执行）：
  1. gate2 = approved（本回执为 decisions.md 中最新 GATE-APPROVED gate2），phase → operations。
  2. **准出回填授权（批准条件，编排者执行并留痕）**：
     a. M1：release.yml WiX 安装硬锁——废弃预装探测，强制 `choco install wixtoolset --version 3.14.1.20250415`；
     b. MINOR 1：test-strategy.md T3 规格行与 units/1.md 契约 token 同步 `ProgramFiles64Folder`（授权修订留痕）；
     c. MINOR 2：`scripts/package-pkg-macos.sh` 补 --version 数值格式校验（与 MSI 侧 case 对称）；
     d. MINOR 3：main.wxs MajorUpgrade 注释补留痕（向前升级自动卸旧装新，降级/同版本仍拦）；
     e. MINOR 9：operations runbook 固化时 SOP 补「第 0 步 dispatch check」（operations 阶段执行）；
     f. MINOR 12：docs/edge.md mac 小节补登录前置句；
     g. MINOR 4/5 记录勘误随本条留痕（state history 时序整点回填事实；「移除 ICE80」措辞与提交史不符——ICE80 从未入提交）；MINOR 6/7/8/10/11 备案 P2。
  3. 追认项打包核查：本 intent Construction 无配额降级代行；T16 授权锁修订为人类裁决行为已留痕——无历史追认项需呈。

GATE-APPROVED gate2 2026-10-15

---

2026-10-15 Operations 人类裁决（AskUserQuestion，逐字转写）：
- 问：「发布执行前置（发布 SOP 固化项；「tag 即发布」直接正式已裁决）：」
  答：「三重前置固定（推荐）」
- 问：「监控与观察项档位（本 intent 无运行时面，监控对象=CI/发布链路+安装包面）：」
  答：「轻量+三项观察（推荐）」
- 问：「retro.md 裁决（route=express 分支：retro 为人类可选，gate2 批准时未一并裁决，现补）：」
  答：「保留 retro（推荐）」
- 裁决落点（编排者执行）：
  1. `ops-runbook.md` 落盘（@ 99c977f，含取代声明——取代 macos-release ops-runbook 的发布流程节）：发布 = 三重前置固定（dispatch check+本地预检四脚本+CI 双腿绿）→ push `v*` tag → 直接正式 Release → T17-T21 人工核验（T18 本机静默装/卸、T20 真机或降级 `xar!` magic 判据）；监控 = 轻量零新增基建（硬门=CI 双腿绿+dispatch 绿+四脚本断言 9/23/13/72）+ 三项观察（WiX 版本硬锁 3.14.1 源公告、macos-15 runner 可用性、安装包真机核验触发）；回退 = 删资产/revert；P2 backlog 七项。
  2. `retro.md` 落盘（@ 99c977f，express 分支人类裁决=保留）：什么有效（gate1 一轮修订全消解/T16 冒烟实抓实修锁修订闭环/verifier 红态独立复现/断言脚本先例复制/配额降级定时重派）；什么没用（AC 写死未裁决默认值/断言对象路径双工件悬空/WiX v3 EOL 晚披露/state 时序倒置与措辞失实/AskUserQuestion 停摆 2 次）；学习五条（安装器工具链先查 EOL 与模板适配面/WiX v3 x64 须 64Folder 且 ICE80 不可抑制/workflow_dispatch 落独立文件/TDD 授权锁修订闭环可复制/express+串行+三重前置组合成本收益良好）。
  3. 回流机制：发布事件/CI 异常/新需求经 `/aidlc:inception` 写新 intent；本 intent 学习沉淀 retro.md。
  4. 完结确认呈出（retro 已落盘，phase=done 待人类确认）。
