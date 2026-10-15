# Retro: audit-p0p1-fixes

> 复盘日期：2026-10-15　参与：人类 + 各角色（pm / architect / qa / dev / verifier / 编排者）

## 什么有效

- **verifier 新鲜上下文终检**：两轮都抓到流程盲区——r1 抓出 B1（U6 漏派发 + 终态批量置位失实，编排者自审不可见），r2 以 token 多重集机械验证 fmt 断言面、独立复核红→绿链。「终态置位必须逐 unit 对照 diff」由此立规。
- **TDD 锁 + 红→绿提交链**：4 个锁定测试文件全程断言面零修改（两轮四方独立核验）；U6 补做按严格红提交纪律执行，成为 M3 流程偏差的修正样例。
- **互审四方 + 只读工具集**：r1 产出 1B/4M/10m、r2 产出 0B/1M/5m，逐条证据引用（文件+行/commit）使人类审批只需裁决语义，不必复读代码。
- **decisions.md 逐字留痕**：打回裁决 → 修复轮 → 复审 → 批准的审计链完整；GATE-REJECTED 后旧回执作废的新鲜度机制两次实际生效（gate1 两轮、gate2 一轮）。
- **偏差备案文化**：U7 事件唤醒（CloseHandle 阻塞实证）、U11 std 语义（FILE_SHARE_DELETE）、U11 自包含 harness 三项实证级备案全部经 gate2 追认——实现发现设计前提不成立时，「备案 → 独立核验 → 人类追认」路径畅通，没有为守契约而扭曲实现。

## 什么没用

- **编排失误**：U6 漏派发（Wave 3 只派 U8+U11）+ 终态批量 replace 置位 done——「12/12 done」声称失实，被 gate2 抓出后整轮返工。
- **红提交纪律 Wave 2-3 缺失**：仅 Wave 1 严格执行；tddLockPaths 恒为 4 文件未随各 unit 红测扩并集，红→绿一度只剩提交信息转述背书。
- **fmt 缺位于施工验证命令**：验证命令只含两腿 test + clippy，不含 `fmt --check`——CI 步骤的 45 处格式违规拖到 gate2 才暴露（M1）。
- **文档计数漂移**：F-04 计数在工件间出现 五/七/八 三个值；AC2 标签混用用户故事编号与施工单元编号两种编号空间——记录性声称也会被互审逐条核对。
- **间歇 flaky 无名可循**：verifier 首跑现身 1 次、测试名未捕获，其后 31+ 跑零复现——只能如实挂「CI 观察项」，定位循环无产出。

## 学到的东西（沉淀进 AGENTS.md / 角色约束）

1. 终态 unitStatus 置位必须**逐 unit 对照 diff**，禁止批量 replace（gate2 人类裁决立规）。
2. 施工验证命令模板固化为四件套：**两腿 test + `fmt --check` + clippy -D warnings**——与 CI 全量步骤对齐，「全绿」才覆盖 CI。
3. 内联 `#[cfg(test)]` 测试模块与路径锁的结构性冲突是 Rust 项目常态（U9 cfapi.rs / U6 router.rs 两次前例）：锁例外三件套「红提交全在测试模块 + 绿提交断言零修改 + gate2 复核」可复用。
4. 修复面扩围（升格并入）走「同一不变量同一性质」判据 + 独立核验 + 人类追认，避免范围蔓延与漏修两极（m13/m18/ec115 三次同模式）。
5. 工件中的可验证量词（「仅 impl 一方法」「15 文件」）宁可实测后写，不估算——记录与实际的偏差在互审中必然被逐条抓出，且比代码缺陷更伤审计链可信度。

## 下个 bolt 要改的

1. **P2 backlog 五项**（见 ops-runbook.md 清单），优先：hash.rs per-run 唯一 temp 目录（根治 flaky 头号嫌疑）、cookie.rs:584 initupload 解密明文日志收口为 status/pickcode。
2. **CI release workflow**：tag 触发三平台 `cargo build --release` + 打包三产物，把发布检查清单自动化一半。
3. F-04 计数三值收敛与 AC 标签编号空间收敛**以本 retro + decisions.md 记录为准**，不回改已完结 intent 的原始工件；如需文档修订另起 intent。
4. 新 intent 施工启用四件套验证命令模板与逐 unit 置位纪律（本 retro 第 1/2 条学习直接落地）。
