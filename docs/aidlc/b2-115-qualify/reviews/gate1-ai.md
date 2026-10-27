# Gate#1 AI 互审（2026-10-18）

> 评审对象：intent `b2-115-qualify`（route=classic）三件工件——prd（AC1..AC4）/ architecture（申请材料技术规格 11 项+AppID 接入预备+受理状态机六态）/ test-strategy（5 TP）。
> 评审方：reviewer-pm（0/1/3）+ reviewer-architect（0/1/5）+ reviewer-qa（0/1/5）+ reviewer-dev（0/2/6）——合并 **0 BLOCKER / 8 MAJOR 报告条目（去重 2 主题）/ ~19 MINOR（去重 ~14）**。
> 正面确认（四方交叉实证）：规格 11 项+15 端点面代码锚抽验 **40+ 处全部属实**（pkce.rs RFC 7636 向量逐字/config 注入链/driver115 端点表 11 行/TODO(verify) 全仓恰 2 处）；勘误锚定（driver115/open.rs 非 auth/open.rs）复核正确；「零写入」边界 TP-05 机械判据成立；scope/redirect_uri 零 OAuth 参数语义成立；23 处…（本批无此面——43 条历史清点不涉及）。受理状态机六态+7+7/+14 口径三方自洽（+7/+14 明示「非平台承诺」）。

## BLOCKER

无。

## MAJOR（去重后 2 主题——均集中于受理跟踪一处，gate1 裁决可一并闭环）

- **M1｜受理跟踪簇：载体未钉+schema 缺键+TP 判据未单列（三工件连环缺口）**[dev-M1/M2+pm-M1+qa-M1+arch-m5]：
  - 载体未钉：prd AC2①「记录于**本批跟踪文档**」定指未决工件（开放问题悬置 tracking.md）——Construction 无从落 AC2/AC3 留痕（dev 判定「不裁决就放行必返工」）。
  - schema 缺键：prd AC3② 要求「受理状态的查看方式在案（在哪个页面/以什么凭据查看）」，蓝图数据模型键值（submit_date/receipt_no/status/expect_date/escalate_date/notes）**无承载键**——照建必漏项。
  - TP-03 判据未单列：四字段（日期/凭据/时点/超期路径）不含「查看方式」，证伪判据不覆盖其缺失（qa 自报但处置条件式）。
  - 附带纠偏：qa 称「decisions.md 只追加承不了多次更新」——实况 decisions 协议为「Write 全文+追加」，追加式日志**可以**承载（结论（建议 tracking.md）仍合理，论证失真须 gate1 呈准）。
  - 关联 MINOR：draft 态与数据模型矛盾（submit_date 不可空 vs draft 无提交——须补「draft=尚不建记录」）[dev-m6]。
- **M2｜AC4⑤ AppID 到位动作对 CLI `login` 不成立**[arch-MAJOR-1]：prd:34 与蓝图下游输入清单①均写「写 config.toml（app_id 键）或设 env 后执行 `login`」——实况 **CLI 二进制不读 config.toml 的 app_id**（仅 clap flag `--app-id` 或 env CLOUDMOUNT_APP_ID，main.rs:33-34/:143-150 皆缺即报错）；config.toml 路线仅对 daemon/desktop 生效。下游「AppID 实测」intent 照清单字面执行第一步即失败。修法（改文档零代码）：分面表述「CLI login 须 `--app-id` 或 `CLOUDMOUNT_APP_ID`；config.toml 路线仅 daemon/desktop 生效」。

## MINOR（去重后 ~14，按主题归并）

1. `client.rs:216-250`/`envelope.rs:71` 裸文件名锚（auth 侧无此文件——须补全 `driver115/`/`core/` crate 前缀）[pm-m1+arch-m5]
2. scope「grep 全 crates 零命中」表述不精确（实为 std::thread::scope 5 处无关命中——TP-01 抽验者按字面复跑会误触证伪）[pm-m3+arch-m2+dev-m2]
3. 7+7 口径引用锚 :8→:3-4 偏移（intent/test-strategy/蓝图三处两档）[qa-m1+arch-m3]
4. 🚀 emoji 错位（backlog A 节首行无 🚀，实为 B1 行）[pm-m2+arch-m3 同簇]
5. 设备码流程「四步」vs「五步」口径（登录四步+刷新第五步——两工件计数不同易假阳性）[arch-m4]
6. 排序参数「usertime」vs 代码 `user_utime`（ROADMAP 措辞转抄违反蓝图「从代码调用点枚举」自则）[dev-m1]
7. AC1⑧ copy/delete 待实测项超出 ROADMAP:16 出处（delete 不在其列）[dev-m3]
8. preid 128K 子事实缺代码锚（hash.rs:14）[dev-m4]
9. TP-05 批次基点占位未钉（应为 14a7eb4）[dev-m5]
10. AC2③ 白名单窄于 TP-05 判据（宜注一句以 TP-05 为准）[dev-m5b]
11. TP-02 机械执行自命中（核验对象含本 intent 自身工件的 grep 模式字面与否定性陈述——排除规则须补）[qa-m2]
12. TP-05 白名单括号措辞歧义 [qa-m3]
13. TP-05 P1 定级缺注记（零代码变更唯一机械判据，时点后置可解释但未落）[qa-m4]
14. multipart 表格锚指向与实现矛盾的 open.rs:5-17 文件内表格（实际 urlencoded——建议改锚 client.rs:137，该表格本身为未在案的文档漂移可收录）[arch-m1]

## 结论

0 BLOCKER / 8 MAJOR 报告条目（去重 2 主题）/ ~19 MINOR（去重 ~14）。事实基准质量高（40+ 锚零虚构、TODO(verify) 全仓恰 2 处、勘误锚定复核正确）；6 MAJOR 报告条目全部集中于**受理跟踪一处**（载体裁决+schema 补键+TP 判据单列——一个 gate1 裁决链可闭环）+**AC4⑤ 消费面失实**（分面表述一行修）。
**建议人类审批关注点**：①总裁决（打回修订 vs 带病通过——不推荐：M1 载体未钉=Construction 无从落留痕）②**载体裁决（gate1 当场可决）**：受理跟踪载体采纳 tracking.md（独立文档可变键值，蓝图为 dev 建议形态）——采纳即同时补 schema `check_method` 键+TP-03 第⑤字段。③M2 分面表述修订方向（CLI login 与 config.toml 分面）。
