# Gate#2 AI 互审 r2（2026-10-15）

> 评审对象：修复轮 diff（c67c733..af7256e）对照上轮发现（reviews/gate2-ai.md）与打回裁决（decisions.md 2026-10-15 GATE-REJECTED gate2）。
> 评审者：**verifier**（新鲜上下文终检：亲自复跑四命令 + U6 逐行核对 + 计划外文件双查，回报 0/0/2）、reviewer-architect（0/0/3）、reviewer-qa（0/1/1）、reviewer-dev（0/0/3）、reviewer-pm（0/0/3）——全部只读。
> 编排者去重合并：**0 BLOCKER / 1 MAJOR / 5 MINOR**。

## 上轮发现消解核对（复审重点）

| 上轮 | 消解判定 | 证据 |
|---|---|---|
| B1（U6/F-02 未实施而记 done） | ✅ 消解 | 红提交 ccb365b（router.rs 测试 +192/-11：Stub 四态 ProbeBehavior + T-03/T-04/T-26 三红 + None 护栏；impl 层无覆写，红态机理=trait 默认 Ok(None) provider.rs:93-95）→ 绿提交 1d90f36（impl 增 change_probe 经 call() 转发）。HEAD router.rs:213-217 与 units/6.md 契约、architecture F-02、prd AC2 三方一致；T-26 断言 last_switch from/to/reason + 切换后路由 + 双向计数；driver115 全套 82/82（verifier 逐目标实测吻合）；state.json unitStatus U6=done 与 diff 一致 |
| M1（cargo fmt --check 失败 45 处/15 文件） | ✅ 消解 | verifier/architect/pm 三方独立复跑 `cargo fmt --all -- --check` exit 0；d88b95c 13 文件 + 1d90f36 router.rs = 14 文件纯布局（verifier token 多重集机械验证：字符串字面量/标识符/数字多重集红→绿全一致）；4 个锁定测试文件断言面零修改 |
| M2（默认腿间歇 flaky） | ✅ 处置合理（保留见 MINOR-2） | 修复轮 ~31 跑零复现 + verifier 本轮加跑 5 次零复现；处置="未复现、CI 观察项"；qa 全量 diff 核验无断言放宽（断言宏计数前后相等、无超时调大/重试包装） |
| M3（红提交纪律仅 Wave 1） | ✅ 留痕完成 | decisions.md 修复轮记录留痕流程偏差；U6 补做按严格红→绿纪律执行为修正样例（红提交含全部测试全文、绿提交断言语义零修改，git 史可独立复核） |
| M4（AC2 映射落空） | ✅ 随 B1 消解 | T-03 主格/T-03 None 护栏/T-04/T-26 四测试非恒真、断言具体值，AC2 三分支全锚定 |
| MINOR 1/2/5/6/9（点名同步项） | ✅ 全部落盘 | test-strategy.md 缺口节三处留痕（T-15 单元腿、T-27 措辞、T-28/T-30 desktop 降级）+ prd.md AC2 映射 U2→U6（grep 无残留）；m5/m7/m8/m9 如实记 P2 backlog/备案，未声称已修 |

## MAJOR

- **R2-MAJOR-1｜"绿阶段 diff 仅 impl 一方法"备案措辞与实际 diff 不符**［qa 记 MAJOR；architect/dev 同发现记 MINOR］
  decisions.md:222/:228 将"绿阶段 diff 仅 impl 增一行转发 + 测试断言零修改"设为本轮复审核验判据。实测 1d90f36 的 router.rs diff = 40+/12-：impl 侧仅 `ChangeProbe` import + `change_probe` 方法（契约内，实码 ~6 行）；但测试模块另含 7 处 rustfmt 布局重排（use 排序互换 / Stub::new 签名断行 / 5 处断言宏换行）。**断言语义零修改经三方独立核验为真**（逐断言比对：实参值、matcher、消息字符串字节级相同；qa 断言宏计数 51=51；verifier token 多重集一致，HEAD 与绿提交测试模块哈希一致）；1d90f36 提交信息尾部"本文件随 fmt"已自披露。判据后半句（断言面不变）实质通过，前半句（diff 仅 impl）字面失实——定性为记录准确性问题（实际优于记录），非交付缺陷。

## MINOR（5）

1. **fmt 计数口径失实**［verifier/qa/architect 三方一致］：decisions.md/d88b95c 称"45 处/15 文件归零"，实测 fmt 触及 13 文件（d88b95c）+ router.rs（1d90f36）= **14 文件**，第 15 个文件无法在 git 史归属；且 decisions.md "cookie_mock/mod.rs -w diff 为 0" 抽验口径空转（该文件不在 fmt 提交文件列表，-w=0 平凡为真）。`fmt --check` 现过的事实不受影响。
2. **M2 flaky 未定位 + 头号嫌疑未备案**［dev］：flake 本体测试名从未捕获，"CI 观察项"处置对不可复现 flake 属合理收口（pm 确认与裁决"定位 flaky"字面差距如实）；但静态排查给出头号嫌疑 `file_path_hashing_matches_bytes`（crates/cloudmount-driver115/src/hash.rs:157-170：固定共享 temp 路径 + 300KB 读写哈希比对，与并行进程/索引器存在截断中读竞态，重编译首跑高 IO 窗口最大——与"首跑重编译高负载现身、31 跑零复现"观测画像吻合），修复轮未排查未入 P2 backlog（per-run 唯一 temp 目录可根治）。
3. **prd AC2 标签编号空间混用**［pm］：prd.md:33 更正为 `(U6, F-02)` 后，"U6"在 AC 头是用户故事编号（U6=F-06），裁决所指实为施工单元编号（units/6=F-02）——AC2/AC6 两条 AC 均标 U6、两种编号空间并存。覆盖内容与测试面不受影响，建议措辞收敛（如 `(units/6, F-02)`）。
4. **architecture.md F-04 计数为第三个过时值**［pm］：architecture.md:157/:331 仍为"五处"（缺 ec115 两处升格），canonical=七处；裁决已明示 architecture 文本修订归 retro，非本轮违反，建议明确列入 retro 清单防再漂移。
5. **U6 Stub mock 漂移面**［dev］：Stub `clone_err` 兜底分支（router.rs:435）在 Error 枚举新增变体时会静默错映射；四态 Stub 无法表达"真实 provider 把非认证错误误包成 Auth → 路由层误切换"的通道侧盲区——固有 mock 漂移风险，建议随 m9 一并留痕。

## 复跑验证（verifier 亲自跑）

- `cargo fmt --all -- --check` exit 0；`cargo clippy --workspace --all-targets --features cloudmount-daemon/mock -- -D warnings` 零诊断。
- 默认腿 `cargo test --workspace`：**26 目标 / 277 passed / 0 failed**；mock 腿 `cargo test --workspace --features cloudmount-daemon/mock`：**26 目标 / 289 passed / 0 failed**——与 decisions.md 修复轮声称逐目标吻合；计数演进自洽（273→277 / 285→289，+4 = U6 四测试）；driver115 全套 82/82 吻合。M2 高危目标 driver115 --lib 另加跑 5 次零复现。
- 计划外文件双查：修复轮 18 文件全部可归属（router.rs=U6 红/绿、13 文件=fmt、4 文档=裁决留痕/MINOR 同步），零计划外改动、零新增 TODO/FIXME、Cargo.toml 零改动（零新依赖持续成立）。
- 验收抽查（子进程真实运行）：AC2/U6 router 11/11（含 U6 四测试）、AC10/U4 single_flight_refresh 8/8、AC7/U8 exec_remote_dir 4/4（testutil 腿）、AC8/AC9/U11 webdav_write 6/6（mock 腿）、desktop T-28/T-30 4/4（CI 不可见项人工兜底清单⑦口径）。

## 结论

**0 BLOCKER / 1 MAJOR（记录措辞级）/ 5 MINOR。** 上轮打回要点 B1/M1/M2/M3/M4 及点名 MINOR 全部消解且有提交级证据；两腿 277/289 全绿、`fmt --check` 过、clippy 净经三方独立复跑；需求覆盖 14/14 终态成立；零范围夹带、零追认项回归（rdc.rs/webdav.rs 修复轮零触及）。唯 MAJOR 为 decisions.md 备案句措辞与实际 diff 不符（断言面零修改实质为真、提交信息已自披露），其余 MINOR 均为记录/备案级。

**建议人类审批关注点**：①R2-MAJOR-1 备案措辞失实的处置（订正=本轮审批条目内订正留痕，decisions.md 只追加）；②M2 "CI 观察项"处置与 hash.rs 头号嫌疑补备案是否接受；③追认项打包（router.rs 内联测试模块锁例外 / fmt 触及 4 个锁定测试文件的布局调整）。
