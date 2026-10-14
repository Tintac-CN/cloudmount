# Gate#2 AI 互审（2026-10-15）

> 评审对象：Construction 全量代码 diff（671de97..HEAD @ 33517e5）对照 plan.md/architecture.md/test-strategy.md
> 评审者：**verifier**（新鲜上下文终检，FAIL 4 发现）、reviewer-architect（1/0/4）、reviewer-qa（1/1/3）、reviewer-dev（1/1/4）——全部只读，verifier 亲自复跑验证命令。
> 编排者去重合并：**1 BLOCKER / 4 MAJOR / 10 MINOR**。

## BLOCKER（四方一致）

- **B1｜U6/F-02（ProviderRouter change_probe 转发）未实施，但状态记录为 done**。
  - 代码事实：`router.rs` 在 671de97..HEAD 的 diff 为空（最后触及为 intent 之前的 022460d）；`impl Provider for ProviderRouter` 无 change_probe 覆写，走 trait 默认 `Ok(None)`（core/provider.rs:93）→ `--channel auto` 恒 None → 每周期回退全量扫描——F-02 要修的病灶（100 目录 @1r/s → 100+ 秒）仍在成品中。units/6.md 承诺的 T-03/T-04/T-26 三测试全仓零命中。
  - 记录失实：state.json `unitStatus.U6 = "done"`；decisions.md "12/12 units 全部 done"；且其自列提交链中无任何 U6 提交（自相矛盾）。
  - **根因（编排者自认）**：Wave 3 只派发了 U8+U11，plan.md 波次③的 U6 被编排者遗漏派发；终态更新时又以批量 replace 将全部 unitStatus 置 done，未逐 unit 核对。verifier 新鲜上下文终检正是为此设计——抓到。
  - **修复方向**：补 U6（一行转发经 call() + router.rs Stub 三分支测试，契约已在 units/6.md 定死）+ state.json/decisions.md 记录更正。无任何人类裁决支持出范围，不可"补备案"消解。

## MAJOR

- **M1**［dev］**cargo fmt --check 失败**：45 处违规/15 文件（open.rs/daemon main.rs/webdav_write.rs/cookie.rs/ec115.rs/cookie_channel.rs/cookie_logging.rs/single_flight_refresh.rs/exec.rs/exec_remote_dir.rs/rdc_release.rs/cfapi.rs/cfapi_hydration.rs 等）→ ci.yml:28 的 fmt 步骤将直接红。施工验证命令只含两腿 test + clippy，未含 fmt——"全绿"结论不覆盖 CI 第 28 步。修复机械（cargo fmt），零语义风险。
- **M2**［verifier］**默认腿间歇失败**：`cargo test --workspace` 首跑 1/5 出红（driver115 --lib 49/1，exit 101，测试名未捕获）；随后 4 次整腿 + 26 次隔离跑未复现——疑重编译后首跑高负载的时序敏感断言。编排者"两腿 0 失败"在其时点为真，但 gate 依赖该命令须先定位此 flake。
- **M3**［verifier］**TDD 红阶段提交/锁并集纪律仅 Wave 1 执行**：tddLockPaths 恒为 4 文件从未扩并集；U2/U7/U8/U10/U11 的测试文件只在各自 -green 提交出现，无独立红提交，红→绿仅由提交信息与 decisions.md 转述背书（git 史不可独立复核）。缓解：qa 抽验测试非恒真、无测试削弱实证；属流程偏差记录而非交付缺陷。
- **M4**［qa］**AC2 映射纪律落空**：AC2 三分支的 T-03/T-04/T-26 无对应测试——prd 自设"无 T 点落点的 AC 不得进入 Construction"在交付面失守（B1 同根因，补 U6 自动消解）。

## MINOR（10）

1. T-15 单元腿缺失（错误分类函数仅集成覆盖，缺口节未留痕）。［qa］
2. T-28/T-30 落 desktop crate 内 #[cfg(test)]（CI 不可见，实测 4 绿需手动跑）——decisions 已留痕人工清单⑦兜底，但 test-strategy 缺口节未同步降级，且 T-27 行"不设 desktop crate 内自动化测试"与实际落位矛盾。［qa］
3. mock 腿计数备案 281 vs 实测 285（doctest 口径尾差，不影响 0 failed）。［verifier/qa］
4. cfapi_hydration.rs 解锁窗口内 2 处 doc 注释顺带改写（断言面未动，"补强"措辞漂移为"人工清单"）。［verifier］
5. F-04 计数失实："八处"实为**七处**（cookie.rs 4 + cli 1 + ec115 2）；且 cookie.rs:584 的 initupload 解密响应全文日志（架构豁免保留项）与 ec115 同类材料"只记长度"的新口径矛盾，建议后续收口为 status/pickcode。［arch/dev］
6. prd.md AC2 映射列标"（U2，F-02）"应为 U6。［arch］
7. exec.rs 错误路径（list/create_dir Err 经 ? 提前返回）跳过 dir_creating 条目清理——条目残留（增长上界=失败过的不同前缀数；正确性无害，内存尾债）。［dev］
8. webdav.rs:513 percent_decode 边界条件写两遍（冗余费解，行为正确）。［dev/arch］
9. tests/cookie_mock/mod.rs 与 tests/mock_api.rs 约 60 行平行实现未备案（两套 responder 漂移风险；U11 自包含 harness 有备案，此处无）。［dev］
10. architecture F-10 并发测试要求"refresh() 入口恰 1 次"与实际断言"无并发在途窗口（串行化）"措辞级差异（decisions.md:122 已留痕语义理由）。［arch］

## 复跑验证（verifier/qa 各自亲自跑）

- mock 腿：285 passed / 0 failed（webdav_serve 6 + webdav_write 6 真实执行——T-25 空绿消除）；默认腿 273 passed / 0 failed（除 M2 的一次间歇）。desktop 本地 4 绿（CI 不可见）。`cargo test -p cloudmount-sync` 无 features 编译失败为既有状态（workspace 腿经特性统一不受影响）。零 #[ignore]、零残留 todo!()。

## 架构符合性与偏差裁决建议（reviewer-architect）

- **13/14 项修复源码级核实符合设计**（F-01/03/04/05/06/07/08/09/10/11/12/13/14 全过；唯一缺失即 F-02）；安全不变量 5 条逐条判定成立；范围内 Cargo.toml 零改动（零新依赖成立）；计划外文件双查零无归属。
- **三项偏差备案建议全部追认**：①U7 事件唤醒（探针证伪 CloseHandle 唤醒前提；契约面——幂等 close/Option/exited/5s 有界/detach+warn——原样保留；wake 文件 .tmp 后缀经 EXCLUDED_SUFFIXES 双滤除不污染变更集；mem::forget 语义必要且正确）；②U11 T-13 std 语义（FILE_SHARE_DELETE 使原红判据不成立；T-14 独占打开判别力等价或更强）；③U11 自包含 harness（结构约束下合理取舍，代价是两份 mock 需同步维护）。

## 结论

**1 BLOCKER / 4 MAJOR / 10 MINOR。** F-02 缺失为编排失误（漏派发 + 终态批量置位未核对），修复量小且契约定死；fmt 为机械修复；flaky 需定位；其余为记录与措辞级。建议：**打回（phase 回 construction）**——点名修复 B1（补 U6）+ M1（cargo fmt）+ M2（定位 flaky）+ M3（流程偏差记录留痕）+ MINOR 1/2/5/6/9（记录与测试缺口节同步），订正 state.json 记录后重进 gate2 复审（复审重点核对 B1/M1/M2 消解）。
