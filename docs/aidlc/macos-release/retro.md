# Retro: macos-release

> 复盘日期：2026-10-15　参与：人类 + 各角色（pm / qa / dev / verifier / 编排者）　route=express

## 什么有效

- **A1→A3 全流程首次走通**：自然语言接入 → 意图/路线/范围一次表单裁决 → PLAN-CONFIRMED 拆分确认——人类把关点前置且无遗漏，express 裁剪下门控回执/留痕/锁保护一项未少。
- **B1 技术发现链**：pm/dev 独立同证「本地双 target `cargo check` 因 bundled C build scripts 不可行」——互审在**设计门**就拦下不可验收的 AC，避免施工期返工；打回后按裁决一句话（"按全部推荐"）高效收敛。
- **R2-MAJOR 三方同证 + 人类钉死**：check job 载体欠定与 T11 断言互锁在复审被抓出，独立 workflow 文件裁决一句话落地——「欠定自由度在 gate 前收敛」模式有效。
- **三轮严格红→锁→绿**（U1/U2/U3）：红提交只含测试文件、绿提交零测试改动、锁时序严格红后绿前——verifier git 史逐提交核验全过；dev 红阶段用仓库外参考实现自证断言可绿非恒红，是红态真实性的加强证据。
- **verifier 零失实证**：六命令独立复跑全部属实、`testBaseline` 逐字节未动核验、10 文件逐归属——上 intent「终态逐 unit 对照 diff」的学习在本 intent 直接落地，全程无记录失实。
- **准出回填解锁仪式**：gate2 批准条目授权 → 解锁留痕 → 补 `--arch` 断言（22→23）→ 重锁——「覆盖缺口、代码正确」型 MAJOR 不需整轮打回，一句话级闭环。

## 什么没用

- 初版 AC3 的「check 不链接」论证有技术硬伤（漏 build scripts C 编译一环）——编排者核查结论未经实证就进了 prd，靠互审救回；技术可行性断言应先验证再落工件。
- AskUserQuestion 三次未获选择（gate1 打回、gate1 复审、A3）——表单未答时流程停摆等对话裁决，往返成本高。
- 基线首跑命令带 `tail -60` 截断导致统计不可用、重跑一轮——验证命令的输出捕获必须全量。
- `--arch` 契约外扩展：实现加了参数但契约与测试都没跟上，直到 gate2 互审才抓出覆盖缺口——「实现超出契约即红旗」应更早自查。

## 学到的东西（沉淀）

1. 「`cargo check` 不链接」≠「不编译原生代码」：bundled C（libsqlite3-sys/ring）build scripts 使跨 target check 在无交叉工具链宿主不可行——预检设计先验证工具链前提，再写判据。
2. 给存量 workflow 加 `workflow_dispatch` 会触发其**全部** job（GHCR `:latest` 覆盖/伪 Release 风险）——新触发面默认落独立 workflow 文件。
3. GHA matrix 会展开为多 job 多 runner——「单 runner 双 target」须显式钉死顺序步形态并配无-matrix 断言，否则 Construction 可自由漂移。
4. TDD 锁与多 unit 测试追加的结构性冲突：新断言落**独立测试文件**免解锁仪式（U3 样例可复制）。
5. 准出回填（批准条件式小修 + 解锁仪式留痕）比整轮打回更适合「覆盖缺口、代码正确」型发现。

## 下个 bolt 要改的

1. P2 backlog 五项（见 ops-runbook.md）：优先 ci.yml actionlint 步骤与 `.gitattributes` 行尾统一。
2. 首个 mac tag 的 **prerelease 预演**（T14-T16 人工核验）——发布 SOP 第 0 步 dispatch check 先行。
3. AskUserQuestion 未答时的降级路径：表单超时自动转对话式一句话裁决提示（减少停摆往返）。
