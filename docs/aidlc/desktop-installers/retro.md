# Retro: desktop-installers

> 复盘日期：2026-10-16　参与：人类 + 编排者 + 各角色（pm/architect/qa/dev×多轮/reviewer×4/verifier）
> route=express；retro 保留裁决=Operations AskUserQuestion 2026-10-16 人类逐字「保留 retro（推荐）」。

## 什么有效

- **TDD 锁+断言脚本先行**：红→锁→绿全程可审计（58/53 → 73 → 87 → 111 → 112 轨迹，verifier 在 bbe706e 档案树独立复现了红态）。锁定断言作为契约，强制实现面「转绿而非弱化」。
- **红阶段钉死字面**（U1 时点钉死 job 名/产物名/构建形态/菜单 id）：避免了 U3/U4 实现期间的命名漂移，二期断言即有锚。
- **首日验证四项开工执行**（B1 类教训的制度化）：伪隐藏偏差正是四个探针实测抓出来的——「不断言未验证可行的 API」直接避免了把不可行方案写进自动化断言。
- **互审+独立 verifier 终检**：四方视角互补实证——dev 视角独立抓到 M1 状态机毒化 bug（其他三方未捕获）与 M3 fmt 口径盲区；verifier 全量复现了所有验证声称（构成交付可信度的独立锚）。
- **准出回填机制**（沿 audit-p0p1-fixes 先例）：「通过+条件式小修」在不重开门的情况下闭合 3 MAJOR，一次 AskUserQuestion 解决裁决+处置定义。

## 什么没用

- **`Visible(false)` 直觉方案被实测推翻**：真隐藏在 Windows 是物理形态敏感的（WM_PAINT 帧饥饿+ControlFlow::Poll 自旋）——两次探针+CPU 数据才说服切换方案；「一小时选直觉方案」的成本错觉被打破。
- **「终验全绿」措辞给了虚假覆盖感**：`cargo fmt --all`/`clippy --workspace` 根本不触及 workspace-exclude 的 apps/desktop（dev 评审实测 desktop 内 fmt dirty 394 行）——终验口径必须逐命令写明覆盖面，否则「全绿」是复合假象。
- **.wxs 的「直觉快捷方式写法」三级退化**：File 嵌套→CNDL0062，组件级→ICE43/57，最终独立组件+HKCU KeyPath——WiX per-user/per-machine 混装规则应最初就查证（ICE 全量零抑制口径放大了每一次试错的成本）。
- **AskUserQuestion 呈批两次未获选择**（A3/gate1 r2）：表单交互并非每次可达，对话兜底路径始终必要；「门禁永不自动」的铁律兜住了未获选择时的正确停等。

## 学到的东西（沉淀进 AGENTS.md / 角色约束候选）

- **workspace-exclude 子项目的验证命令必须显式覆盖**：根级 fmt/clippy/test 对 exclude 成员零效力；plan 的验证命令应按交付面逐根写（本期教训：终验组合加 `cd apps/desktop && cargo fmt/clippy`）。
- **Windows GUI「隐藏」是状态机问题不是样式问题**：任何「窗口消失」方案要连同「事件流是否继续喂」一起验证（伪隐藏/TOOLWINDOW 方案保留了 WM 消息面，才使托盘菜单可达）。
- **授权锁修订纪律在 hook 无机械拦截时仍应严格走**：guard-gates 的 deny 经查只命中 isTestFile 匹配的文件（.test.sh 不在其列），本 intent 两次解锁（T4/M2）全走授权→留痕→修订→重锁，零越界——纪律不因机制宽松而免除，这条写进了 decisions.md 并应延续。
- **桌面交付的"安装即验"场景 Sink**：msiexec 装卸冒烟+主进程探针（PowerShell PostMessage+GetWindowRect）是可 automated 的组合，值得作为桌面类 intent 的标准核验法沉淀。

## 下个 bolt 要改的

- backlog 11 项+沿用项见 ops-runbook.md「P2 backlog」（wxs ARP 元数据/TP-23 承接/dmg 架构校验/CI actionlint 步骤/Cargo.toml repository 元数据等）。
- CI 侧补 desktop fmt/clippy 步骤——把终验口径缺口在流水线上游补死（新 intent 候选，体积小）。
- mac 真机核验落地时：伪隐藏 mac 形态二选一（离屏保留 Dock 图标 vs LSUIElement）随 TP-20/21 实测裁决。
- gen_icons 若需扩尺寸集（256/512 png 组等），沿既有 stdlib 方案扩即可（幂等边界注记随行）。
