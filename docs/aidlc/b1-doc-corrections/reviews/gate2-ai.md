# Gate#2 AI 互审（2026-10-17）

> 评审对象：intent `b1-doc-corrections`（route=classic）Construction 交付——回写 diff `d41af14..c6dbcb8`（10 文件 +56/−47，蓝图 28+2 项）。
> 评审方：reviewer-pm（0/0/3）+ reviewer-architect（0/2/3）+ reviewer-qa（0/0/2）+ reviewer-dev（0/1/7）+ **verifier（0/1/3）**——合并 **0 BLOCKER / 3 MAJOR 主题（去重 2）/ ~13 MINOR**。

## verifier 独立终检结论（先置）

蓝图 30 项**全落字（0 漏项 0 越项）**；**计划外文件=无**（diff 恰为蓝图 10 文件；`git status` 干净）；**零逻辑变更成立**（逐 hunk 分类：注释/头注/字段/文档/删行，无任何代码逻辑 hunk）；四方对账一致；验证声称全部真实复跑（277/0+五脚本 112/72/23/13/9+fmt/clippy 双面+TP-02 双口径+AC6a 十文件+owner 四源+AC4 净删）。终检结论=**报告/回写完成，无阻断项**。

## 正面确认（四方交叉实证）

- 23 处清零：双口径独立复跑零命中（产品面）；AC6a 十文件逐一 grep 全零。
- 零越界：diff 文件清单恰为蓝图 10 文件；webdav.rs 五处全注释行；api.ts 仅头注块；Cargo.toml 仅 :12+:20（homepage/authors 未动=约束守住）；README:143 stale **正确地未顺手改**。
- #26 命名注记事实核验属实（cfapi.rs:179-193 两步路径逐点吻合）；「8 命令」函数计数属实（api.ts :45-81 恰 8 个）；排版零回归（README 架构图显示宽度 65 列改前后逐位一致）。
- 蓝图措辞适配（#6 历史注记去 Tauri 字面）判定合规且必要（清零验收所需），已报备。

## BLOCKER

无。

## MAJOR（3 报告条目，去重 2 主题——均为本批新写入文本的失实表述，一行级收敛）

- **M-1｜api.ts:6「8 命令已全部实现」为失实表述，违反 prd 措辞约束②**[arch-M1+dev-M1 同源]：唯一实现方 daemon 边缘模式实况=**4/8 命令 501 占位**（webdav.rs:938-942 get_cache_stats「占位」/clear_cache/:958-962 set_sync_root/open_sync_folder）+get_status 硬编码占位（loggedIn/syncing=true、计数全 0）；与蓝图自身 F3「边缘四命令 501=文档化边界」直接矛盾。根因=蓝图 #15/D7 引注锚（api.ts:44-81 客户端函数）而非服务端实现，dev 照单执行。一行收敛（arch 建议）：改「8 命令契约已定形；其中 4 命令在边缘模式为文档化 501 边界」类限定表述，并同步蓝图 #15 措辞族。
- **M-2｜web-ui/README:20 引用不存在的托管机制 `CLOUDMOUNT_WEB_UI`**[arch-M2]：全仓 crates/** 无消费点；daemon 实际用 `--web-ui-dir`（main.rs:116-118 无 env= 属性）；config 环境覆盖仅 APP_ID/SYNC_ROOT/CHANNEL；权威记载=docs/edge.md:17,56（edge/Dockerfile:23 的 ENV 为死变量，属既有 edge/ 分叉，本批不收敛）。蓝图 F3 唯一无锚事实未经实测落入产品文档——「Docker COPY」半句为真、机制名半句失实。一行收敛：改「由 cloudmountd `--web-ui-dir` 指向静态托管（Docker 镜像 COPY 至 /web-ui）」类可验证表述。

## MINOR（~13，按主题归并）

1. **prd 三工件 sync 债**：非目标「不做脱水 API 命名注记」vs plan/A3 批准纳入 #26 且已交付（arch-m1+qa-m1+dev-m6 同源）——gate2 准出回填补裁决注记。
2. ROADMAP:114-115「UI 直连引擎」漏列 sync-root 等价能力（desktop 设置页实有选择目录写回；保守方向漏列）[arch-m2]
3. web-ui 非 tauri 旧语境残留（api.ts:44「托盘」/web-ui README:27「托盘设置页」）[arch-m3]
4. ROADMAP:128-135 混纪元自洽（bug① 历史注记限定词建议）[dev-m1]
5. ROADMAP:116-119 列表流打断（承载句独立成句建议）[dev-m2]
6. Cargo.toml:21-22 homepage/authors 残留 zhonghaoxi（intent 最小域钉死；防丢失跟踪=owner 统一 1/3）[dev-m3]
7. README:144 测试计数 stale 信任度升高（登记勿漏）[dev-m4]
8. .gitignore:9-10 双空行（AC4 字面达成的自然结果，卫生批）[pm-m1+dev-m5]
9. prd AC1 证据载体「workbook 文件」未物化（证据以 state history+commit message 摘要形态在案，可独立复验）[pm-m2]
10. prd↔蓝图括号风格微差（#1 桌面壳标注形态）[pm-m3]
11. webdav.rs:850「web-ui 的 REST 面」web-ui 重复等微措辞簇 [dev-m7]
12. ROADMAP:133「契约/序列化」双重复 [dev-m7]
13. api.ts「命名与参数保持一致」约束对象悬空（web-ui README:26 已补「以 api.ts 为准」，api.ts 本体未补）[dev-m7]

### verifier 单列

- MINOR：state.json history 时间戳簿记失真（数组内非单调——生命周期顺序与决策台账全部正确，仅时钟值失真；事后审计勿依赖 state 时间戳，以 git 提交时序为准）。

### INFO（不计发现）

- api.ts 头注块 :1-6→:1-7 自然漂移（搭车句 +1 行，契约「仅头注块」实质完好）；蓝图 E 节 README:143 实为 :144（未动=正确）；#6 措辞适配合规已报备。

## 结论

0 BLOCKER / 3 MAJOR 报告条目（去重 2 主题）/ ~13 MINOR。**回写机械执行质量高**（verifier：30/30 落字+零计划外+零逻辑变更；qa：双口径+基线+边界独立复现；dev：8 处发现正确性抽检零翻车）。2 MAJOR 均为本批新写入文本的失实表述（讽刺性复发，但被 gate2 拦截——正是互审价值），均一行级收敛且 arch 已给出建议措辞。
**建议人类审批关注点**：①总裁决（通过+准出回填 vs 打回修订——2 MAJOR 为一行级事实订正，随批准闭合不重走复审）②既有 flaky 观察（engine_works_with_native_watcher 满载时序 flake，隔离复跑 5/5——非本批回归，建议登记跟踪）。
