# Retro: installer-packages

> 复盘日期：2026-10-15　参与：人类 + 各角色（pm / qa / dev / verifier / 编排者）　route=express

## 什么有效

- **gate1 一轮修订即全消解**：上 intent gate1 走了三轮，本 intent 一轮打回后复审 r2 即 0/0/5 全过——预检设计教训（B1 系：工具链前提先行验证）与断言可判定性要求在 gate1 呈批时就随裁决钉死（Q1-Q4/钉死五件/T16 升 P0），修订轮一轮收敛。
- **T16 冒烟实抓实修的锁修订闭环**：T16 冒烟实测抓出 WiX v3 x64 包 `ProgramFilesFolder` 落 x86 目录（ICE80 非误报）——正是 P0「MSI 安装行为唯一 tag 前验证路径」的价值兑现；「冒烟实证→停下报告→人类授权→解锁修订→重锁」闭环全程留痕，是 TDD 授权修订的样例。
- **verifier 红态独立复现**：git archive 提取 b40eaf0 全树实测 18/72+54 FAIL 与记录逐数一致——红态真实性首次由独立复现证明（此前仅靠提交时点推断）。
- **结构断言脚本先例复制**：installers-yaml.test.sh 复制 release-yaml.test.sh 机制（job_block/防空真/CRLF 剥除/降级计数）并在头注注明来源与同步义务——TDD 红态断言一次落笔即 54 项精准全红（护栏与被测断言分明）。
- **「实现超出契约即红旗」自查提前**：`--arch` 契约外扩展在上 intent 是 gate2 才抓出的 MAJOR，本 intent dev 在 U2 即主动报告 T16 工具链不可用而非静默跳过——角色约束内化有进步。
- **配额降级定时重派**：gate2 评审遇 5 小时限额全灭，后台定时器 55 分钟后自动重派全过——无需人工盯守，串行限流教训之外新增「限额窗口定时重派」实践。

## 什么没用

- 初版 AC 按建议默认值写成定论而 Q1-Q4 仍开放（MINOR⑨）——「未裁决项写成事实」被四方一致点名，裁决应在工件落定前显式化。
- 断言对象路径（.wxs/打包脚本/断言文件名）两边都没列入开放问题、且两工件各执一词（单复数冲突）——「断言一个还不存在的路径约定」使 TDD 红态断言一度无法落笔，靠 gate1 钉死才解。
- WiX v3 EOL 事实在 gate1 互审才由 dev 角色披露——Q2 建议默认（cargo-wix）在编排者核查阶段就该检出 EOL 与双 bin 模板缺陷，减少一轮裁决往返。
- state.json history 出现时序倒置条目（U2-done 整点回填排在 U3-done 前）+「移除 ICE80」措辞与提交史不符——verifier 抓出的两处记录精度问题均为事后补记时未核对 git 时序。
- AskUserQuestion 未获选择的停摆模式本 intent 仍出现 2 次（gate1 复审、gate2 审批）——对话式一句话裁决兜底有效但依赖用户在场。

## 学到的东西（沉淀）

1. 安装器工具链选型先查 EOL 与模板适配面：WiX v3 已 EOL、cargo-wix 模板按单 package 单 bin 生成——「Rust 生态标准」不等于适配当前仓库形态（双 bin+PATH 必手改 .wxs）。
2. WiX v3 x64 包须用 `ProgramFiles64Folder`（`ProgramFilesFolder` 运行时解析 x86 目录，ICE80 是正确告警不可抑制）——环境元素类断言 token 应在冒烟实测后钉死，而非按「惯用字面」预设。
3. workflow_dispatch 新触发面默认落独立 workflow 文件；给存量 workflow 加触发器会连带触发全部 job（GHCR `:latest` 覆盖风险沿上 intent 学习再次成立）。
4. 授权锁修订的完整闭环（冒烟实证→停下→人类授权→解锁留痕→修订→重锁→授权依据随提交）可复制——测试契约变更的合法路径不破坏 TDD 纪律。
5. express 路线 + 串行派发 + 三重前置发布 SOP 的组合在 CI/打包基建类 intent 上成本收益比良好（4 units 一轮修订收敛，总时长约 8 小时）。

## 下个 bolt 要改的

1. P2 backlog 六项（见 ops-runbook.md）：优先 ci.yml 补 actionlint 步骤（消 T15 降级缺口）与 Cargo.toml repository 元数据修正。
2. 首个含安装包 tag 的发布执行：三重前置 → tag → T17-T21 人工核验（MSI 本机静默装/卸、pkg 真机/降级判据）——发布 SOP 已固化于 ops-runbook.md。
3. AskUserQuestion 未答时的自动降级：超时转对话式一句话裁决提示（沿本 intent 实践固化为编排习惯）。
