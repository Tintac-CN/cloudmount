# Gate#2 AI 互审（2026-10-15）

> 评审对象：Construction 交付 diff（b1ae65b..HEAD @ 5d624e7）对照 plan.md/prd/test-strategy 与 gate1 裁决。
> 评审者：**verifier**（新鲜上下文终检，七条复跑+红态独立复现+TDD 纪律含授权锁修订合规核验，0/0/2+2INFO）、reviewer-architect（0/1/4）、reviewer-qa（0/0/5）、reviewer-dev（0/0/4）、reviewer-pm（0/0/3）——全部只读。
> 编排者去重合并：**0 BLOCKER / 1 MAJOR / 12 MINOR**。

## 复跑验证（verifier 亲自跑，全部属实）

- 四脚本：package-macos **9/9**、release-yaml **23/23**（actionlint 不可用 SKIP+WARN 降级，计数诚实）、check-macos-yaml **13/13**、installers-yaml **72/72**——全 exit 0。
- `cargo test --workspace`：**26 目标 / 277 passed / 0 failed**——与基线逐数一致，零回归；fmt/clippy 净。
- TDD 纪律（git 史+红态独立复现）：U1 红提交 b40eaf0 仅含测试文件，**git archive 提取 b40eaf0 全树实测 18/72+54 FAIL 与记录逐数一致**（红绿闭环属实、断言非恒真得证）；绿提交 f0a401f/087075d 零测试文件改动；**授权锁修订 2df10af 合规**（decisions.md 授权条目在案+解锁/重锁时序正确+修订范围恰为授权事项+非弱化等精度替换）；testBaseline 逐字节未动；6 文件全量归属零计划外；check-macos.yml 零 diff；YAML/JSON 合法；TODO/密钥零命中；MINOR⑫ 三项（Manufacturer/identifier/重编译+rust-cache/UTF-8 BOM）落位实证。
- T16 留痕链四件互证（decisions 条目/main.wxs 头注①②③/release.yml 版本注释/2df10af）；物理 msiexec 结果未独立复验（INFO，环境受限——留痕链一致）。

## MAJOR（1）

- **M1｜CI WiX 工具链版本软锁**［arch M1+qa MINOR+dev MINOR-2 三方同证］：release.yml WiX 安装为「预装探测+`choco install wixtoolset -y`」**未带 `--version`**，3.14.1.20250415 仅注释留痕——发布 MSI 的工具链版本随 runner 镜像漂移不可复现；且预装探测优先意味着 runner 自带 candle 时其版本被静默采用。prd「首日随安装方式一并定版本并锁定」只做到「定+留痕」。修复一行：`choco install wixtoolset --version 3.14.1.20250415`（并裁决预装探测 vs 强制安装优先序）。不阻断放行——失败模式是 job 显式失败非静默坏包。

## MINOR（12）

1. T3 断言 token 已按授权修订为 `ProgramFiles64Folder`（2df10af），但 test-strategy.md L16（T3 规格行）与 units/1.md L14 仍钉 `ProgramFilesFolder`——规格工件未随授权修订同步，未来实现者按字面会写回错误 token［qa+dev］。
2. pkg 侧版本无数值格式校验，与 MSI 侧不对称（MSI 有 `case *[!0-9.]*` 拒绝，pkg 脚本仅非空校验直传 pkgbuild）——rc tag 误推由 MSI 腿失败+needs 门控整体兜底，无发布面风险［dev+pm+arch］。
3. `MajorUpgrade` 元素实际启用向前升级自动卸旧装新，超出「升级语义不设计不保证」字面口径（行为无害：降级/同版本重装仍被拦；建议注释补留痕）［arch+pm］。
4. state.json history U2-done 条目（21:00:00）排在 U3-done（20:58:09）之前且时间戳为事后回填整点值——数组非时序排列，实质内容无失实［verifier］。
5. decisions「U2-wixfix 含 release.yml **移除** ICE80 抑制」措辞与提交史不符——ICE80 字样从未入任何提交（抑制仅存在于当时工作树实验），最终态与声称一致，纯措辞失实［verifier］。
6. state U2 条目「MSI 侧断言 27 条全绿」为 U2 时点口径，U3 定稿分解为 MSI 45+pkg 27——历史中间口径漂移，最终 72/72 已实证［qa］。
7. T15 拦截自校验正分支（actionlint 可用时的注入负路径）从未在真实环境实跑过一次——建议 actionlint 可用环境补跑全绿留痕［qa］。
8. 「72/72」隐含 actionlint 不可用降级前提（可用分支总数 73）——依赖本机无 actionlint 的事实未被记录，重放时 total 随工具可用性漂移［dev］。
9. 人工核验 SOP 缺「第 0 步 dispatch check」项（上 intent test-strategy 有此步，本 intent SOP 清单①-⑤未含；现有兜底=T13 静态守护无 workflow_dispatch）——operations runbook 固化时补「打 tag 前触发器复核」［qa］。
10. .wxs 无 UIRef——双击安装走 msiexec 缺省 basic UI（无向导）；US1「双击完成安装」仍满足，T22（P2）交互 UI 冒烟实际无向导可验，知会核验人［arch］。
11. release 下载步注释「只取 binaries」语义漂移（现亦收编 MSI/pkg）——AC3 要求该步零改，不改属正确执行，注释滞留纯文档问题［arch］。
12. edge.md 登录前置句（gate1 准出回填 b 项之一）未交付——经核验 gate1 授权清单（2.b）未列该项（上 intent 已交付的 edge.md 属 macos-release 工件），arch 确认「零触碰符合范围声明」；若需补录属 operations 裁量［arch 复核转正］。

## verifier INFO（2）

- actionlint 本机不可用：三脚本语法拦截自校验未执行（脚本内+缺口节在案）；T16 物理 msiexec 未独立复验（留痕链一致）。
- U2「48+/1-」、U3「41+/1-」、U1「566 行 72 断言」「18/72+54」等声称逐数核实无偏差。

## 结论

**0 BLOCKER / 1 MAJOR / 12 MINOR。** 交付三方一致（prd 语义对齐/代码/断言）、fail-fast 闭环完整、安全面干净、brownfield 纯追加纪律与 gate1 全部裁决（Q1-Q4/钉死五件/准出回填/T16 授权锁修订链）逐字落实；verifier 七条复跑+红态独立复现全部属实，TDD 纪律（含授权锁修订特殊形态）合规。唯一 MAJOR 为 CI 工具链版本软锁（一行 `--version` 修复，失败模式=显式失败非静默坏包）；MINOR 均为记录精度/对称性/枚举补全级。

**建议人类审批关注点**：①M1 处置——通过并附准出回填（`choco install --version` 硬锁一行 + 预装探测 vs 强制安装优先序按裁决收紧，MINOR 1/2/3/9 措辞与枚举随 Construction 后补、4/5 记录勘误随本条留痕、6/7/8/10/11 备案 P2）或打回轻量修订；②MINOR 12（edge.md 登录前置句）是否补录为 operations 裁量项。
