# Gate#1 AI 互审（2026-10-16）

> 评审对象：intent `audit-roadmap-next-steps`（route=classic）三件工件——prd.md @ be118d4 / architecture.md @ 3e3feb7+88bf9b1 / test-strategy.md @ c3276a1+b6cc344。
> 评审方：reviewer-pm（0/4/5）+ reviewer-architect（0/3/5）+ reviewer-qa（0/1/7）+ reviewer-dev（0/4/3）——合并 **0 BLOCKER / 12 MAJOR / 20 MINOR**（去重后 10 MAJOR / 16 MINOR，按主题归并）。
> 正面确认（四方交叉实证）：AC↔TP 映射 14/14 无虚构编号（编排者 §8 内联回填机械合格）；TP 引用路径/行号/符号抽验大面积命中（reconcile.rs:204、rdc.rs/inotify_watcher.rs、cfapi TRANSFER_DATA、api.ts 8 命令、五脚本名、三 CI job/触发、四 runbook 行号区间与条目数）；里程碑字面与 ROADMAP 逐字一致；非目标/开放问题质量合格。

## BLOCKER

无。

## MAJOR（去重后 10，按主题归并）

- **M1｜TP-01 证伪判据必然误触（假阴性设计缺陷）**[arch-M2+dev-M2+qa-m1]：`S256`/`device_code` 字面在 auth/cli **全仓零命中**（实现为 `code_challenge`/`code_challenge_method="sha256"`+扫码轮询，pkce.rs:3-4,61；115 非 RFC 8628 命名）——按 TP-01 自身证伪条款，诚实执行必把真声明判「站不住」；同族：TP-01 的 `grep '"login"'` 实跑 0 命中（clap derive 注册为 `Commands::Login`）。需重标定为实际符号（`code_challenge|code_challenge_method|qrcode`/`Login`），并把 ROADMAP「PKCE(S256)」措辞与实现的命名差异列为独立小发现。
- **M2｜AC2 事实错误：「M0 唯一未勾项」不成立**[pm-M1+qa-m2]：ROADMAP M0 有**两个**未勾项（L8 申请资质+L16 待 AppID 实测）；TP-23 自己就列了两项——PRD 与 QA 工件互相矛盾，照 AC2 字面执行会产出错误审核结论。
- **M3｜AC10 开放项归集缺 M0 L8**[pm-M2]：AC10 Given 声称「全部未勾项」但枚举缺 L8「申请 115 开发者资质」——该项是外部阻塞链源头（ROADMAP L3-4「唯一的外部阻塞项」），缺它则 AC14「外部阻塞优先」排序输入不完整。
- **M4｜非目标未钉「零写入」**[pm-M3]：非目标仅写「不改产品代码」，字面允许顺手改 ROADMAP/ARCHITECTURE/.gitignore——而本期核心产出恰是「回写建议」，执行诱惑真实；且文档改动污染审核自身 git 证据链。intent 的「以只读核验为主」同未覆盖文档面。
- **M5｜交付载体未钉**[pm-M4+dev-M4]：审核报告/归集表/路线图的输出路径、文件形态、完整性判据在任何 gate1 工件中均未定义——TP-17/21/22/25/26 全部断言于尚不存在且位置未定的产物；仓内有先例（audit-p0p1-fixes/audit-findings.md）但无一字绑定本 intent。gate2 将无法机械判定「报告完成」。
- **M6｜AS-IS 漏登 release.yml `merge` job**[arch-M1]：architecture CI 段列 8 job，实际 9（merge @ release.yml:355-384）——test-strategy 头注与 TP-10/16 均正确列 9，同一事实三方两口径，AS-IS 基线失真传染 release 面 TP。
- **M7｜安全节缺 CI/发布链权限面**[arch-M3]：release.yml 无 workflow 级默认 `permissions:`（仅 3 job 显式收权，六个 build job 继承默认 token 权限可能 read/write）；第三方 action 全部按可变 tag 引用（供应链面）；tag 触发无 environment 保护。architecture 自设「三元组表述」契约对唯一写权限 token 的发布面未兑现（macos-release runbook 已有「permissions 显式化」P2 项佐证）。
- **M8｜AC13 已知边界 7 项中 6 项无 TP 断言载体**[qa-M1]：仅 TP-06 覆盖第 1 项（同秒漏检）；签名/dmg aarch64-only/WiX 硬锁/macos-15/Cookie 条款/WebDAV TLS 口径 6 项无逐项核验方法——「14/14 覆盖」机械自检掩盖语义覆盖不足；附带 AC13⑦「无 TLS」与 AC6/M4「rustls 可选 TLS」两套表述需收敛。
- **M9｜TP-03 `--features testutil` 备注误导**[dev-M1]：`cargo test --workspace` **不需要**该 flag（testutil 经 dev-dependencies 自动启用，ci.yml:34 即裸命令）；照字面在 workspace 根追加该 flag 会 cargo 直接报错——P0 探测器动作按字面硬失败。应改写「workspace 裸跑；仅 `-p cloudmount-sync` 单包腿需要」。
- **M10｜TP-03/14 将真实执行 cldflt 集成测试，与只读约束矛盾未声明**[dev-M3]：本机是 M2 开发机（SyncRoot 注册会成功），`cargo test --workspace` 会真实执行 cfapi 三套件（SyncRoot 注册/占位符投影/水合/脱水机器状态写入）——与 test-strategy 自约束「不测真机交互/不启动真实挂载」矛盾。须显式裁决：接受瞬态测试夹具副作用，或限定命令（并说明与 277 基线可比性）。

## MINOR（去重后 16）

1. AC1「长期段全部 [x]」与 ROADMAP 实际形态不符（长期段无 checkbox）[pm-m1]
2. 「分级」口径双义未对齐（三态判定 vs P0/P1/P2 严重度分级）[pm-m2]
3. 「M2.5」段级引用与 ROADMAP 结构不符（实为 M2 段内条目）[pm-m3]
4. AC11 输入面含 retro.md 但 TP-21 仅对照 runbook——retro 观察项是否入归集不明确 [pm-m4]
5. U4 溯源标注不实（277/五断言出处是 decisions 而非 intent）[pm-m5]
6. gen_icons.py src-tauri 苗头前提已失效（desktop-installers 已修复该半边；D 表未对账）[arch-m1+qa-m3]
7. D2 证据目录清单不准（漏 Cargo.lock/.gitignore；D1 行号偏一）[arch-m2]
8. D4 低估 stale Tauri 引用面（api.ts 头注第三处）[arch-m3]
9. 数据模型 entries 列清单不全（漏 is_dir/remote_sha1/remote_mtime）[arch-m4]
10. edge//packaging/ 已实际分叉（非「并存待防」）[arch-m5]
11. TP-04 行号漂移一位（L28-29→27-28）[qa-m4+dev-MINOR1]
12. TP-16 的 AC8 半锚语义不匹配（CI job 名不属四苗头，正确锚 AC12/AC9）[qa-m5]
13. 自检行 TP-14 与表格矛盾（TP-14 仅 AC12；277 现值≠114 历史口径）[qa-m6]
14. AC9 新增偏离（D5 元数据/edge-packaging 布局）缺复核锚 [qa-m7]
15. 26 TP 无执行排序与工时注记（TP-03/14 共享复跑、TP-17 后序）[dev-MINOR2]
16. AC14 产出无表格模板（与 M5 同根）[dev-MINOR3]

## 结论

0 BLOCKER / 12 MAJOR（去重 10）/ 20 MINOR（去重 16）。三方在主轴上一致且证据可复现（映射 14/14、里程碑字面、偏离表主轴、CI 计数口径全对）；10 MAJOR 全部为**工件字面级修正**（事实订正/判据重标定/载体钉死/范围补钉），无结构性推翻——一轮修订可闭合。修订后按纪律复审只核收敛点。
**建议人类审批关注点**：①总裁决（打回修订 vs 带病通过——不推荐：M1/M2/M6/M9 属「照字面执行必产出错误结论」级）②顺带裁决两项待复核默认值：交付范围（审核+路线图）与证据标准（抽样验证）——裁决结果直接决定修订轮的范围口径。
