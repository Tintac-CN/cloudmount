# Gate#1 AI 互审 r2（2026-10-15）

> 评审对象：修订轮两工件（prd.md + test-strategy.md @ 1708fdb；修订 diff = 340ce70..1708fdb）对照上轮发现（reviews/gate1-ai.md）与打回裁决（decisions.md 2026-10-15 GATE-REJECTED gate1「打回，按全部推荐」）。
> 评审者：reviewer-pm（0/0/3）、reviewer-architect（0/1/3）、reviewer-qa（0/1/2）、reviewer-dev（0/1/2）——全部只读。
> 编排者去重合并：**0 BLOCKER / 1 MAJOR / 8 MINOR**。

## 上轮发现消解核对（复审重点 B1/M1/M2 —— 四方逐项字面+grounding 双验，全部消解）

| 上轮 | 判定 | 证据 |
|---|---|---|
| B1（本地双 target check 不可行） | ✅ 消解 | AC3 整条改写：编译面预检上移 CI（workflow_dispatch mac check job，runner=macos-15，rustup 双 target，判据 exit 0）+ 本地降为 dry-run 一道 + 预检说明入仓；因果（libsqlite3-sys/ring bundled C build scripts）与 dev 上轮证据链逐环吻合；旧"两道预检"表述零残留（US3/语义对齐/非目标/intent 四处连带同步到位） |
| M1（tar 布局字面矛盾） | ✅ 消解 | AC2/语义对齐/T4/SOP 四处统一为根级 `cloudmountd`/`cloudmount`/`README.md`/`edge.md`（无 docs/ 前缀），与 release.yml:36-38 linux 腿拍平事实同构；无字面残留 |
| M2（Q1 事实过时） | ✅ 消解 | Q1/Q2/Q3 转「已裁决」四处逐字一致（macos-15 单机双 target / cloudmount-darwin-<arch> / edge.md 最小小节）；macos-13 残留仅在否定句与历史条目（合法留痕） |
| M3-M6 + 16 MINOR | ✅ 全部落盘 | 锚双向一致（AC1=T9/T10/T14/T15/T17）；T4 四项清单+systemd 排除；T10 `if-no-files-found: error`+glob 写死；T11 无 `if:` 断言；rust-cache/`-p`/ad-hoc 措辞/needs 门控术语/tag 规范/README 行非目标/T16 真机/T14 资产集合/子串判据+体积区间等逐项在字面；计数 P0=10/P1=6/P2=1 复核无误 |

## MAJOR（1，三方同证）

- **R2-MAJOR-1｜check job 载体（workflow 文件）未钉死，release.yml 内联读法带生产隐患且与 T11 互锁**［arch/qa/dev 一致］
  - 证据：AC3（prd.md:16）只写"仓库含"、T6（test-strategy.md:18）只写"workflow 含"，未限定文件；全仓 workflows 现无任何 `workflow_dispatch`（grep 实测）。若按 intent 主语境落 release.yml：手动 dispatch 触发**全部既有 job**——docker/merge job 以分支名推 GHCR（`:latest` 被覆盖，release.yml:213-216）、release job 以分支名建伪 Release（:87-90）；唯一防护（release job 加 `if: startsWith(github.ref,'refs/tags/')`）恰被 T11 新断言"release job 不带 `if:`/status 覆盖"（test-strategy.md:23）静态禁止——就地方案下 Construction 只能违约或带副作用上线。合规落点只剩独立 workflow 文件或 ci.yml，但 intent.md:13 改动面未列（qa 指出漏报）。
  - 修复：一句话钉死载体（推荐独立 workflow 文件，release.yml `on:` 保持仅 `tags: v*`）+ intent 改动面同步；落 ci.yml 需注意继承顶层 `RUSTFLAGS: -D warnings`（dev 提示）。

## MINOR（8）

1. AC5 括注"matrix 或顺序步"与 Q1"单 runner 双 target"存在张力（matrix 会展开为多 job 多 runner），且"若用 matrix 沿用 `fail-fast: false`"子句无任何 T 点断言（T9 明示不判形态）［pm+qa 同根］——建议收紧为"固定单 runner 顺序步"或 T9 补条件断言。
2. "未签名"措辞在 T12/T16/SOP 第 4 条残留（prd 侧已精确化为"无 Developer ID 签名/公证+ad-hoc"，qa 侧未随）——字面 grep 可能假阴［arch］。
3. T4"四项（不多不少）"未写归一化：同构 `cd dist && tar czf … .` 的清单含 `./` 目录项，字面断言会假红；补"剥 `./` 前缀/忽略目录项"半句［arch+dev 同根］。
4. AC5 句尾"（既有 fail-fast 语义不被削弱）"残留旧术语（同句已引入 matrix fail-fast），建议改"既有 needs 门控语义"［arch］。
5. check 前置纪律消失：旧"check 失败不得打 tag"随 B1 撤销后，SOP 未把"tag 前跑一次 dispatch check 并确认绿"列为发布前置步骤；AC3 check 腿（T6/T7）无负路径点无豁免说明［dev］。
6. T15 未随 M4 同步为四项清单（仍"daemon+cli"两二进制，靠 SOP 兜底）［pm］。
7. SOP 体积判据"5–50 MB 量级"与复核触发"<2 MB / >200 MB"之间存在 50–200 MB 灰区无处置指引［qa］。
8. （并入 1 的另一面）T9 断言拦不住 Construction 改走 matrix 双 runner——"单 runner"约束无对应可测断言［pm］。

## 结论

**0 BLOCKER / 1 MAJOR / 8 MINOR。** 上轮打回要点全部消解实证（字面+grounding 双验，含 brownfield 兼容终验：`cloudmount-*` glob 天然收编、needs 仅追加同构、权限沿用）；新 MAJOR 为 B1 改写引入的落点欠定，一句话可钉死；8 MINOR 均为措辞/断言收尾级。方案骨架不动摇。

**建议人类审批关注点**：①check job 载体裁决（推荐独立 workflow 文件，intent 改动面随批准扩入）——若采纳，可与 MINOR 1/4/5/6/7/8 一并作为"随 Construction 首个 unit 顺带回填"授权（gate1 r3 先例）；②或打回第三轮轻量修订（只钉落点+MINOR 措辞，复审只核收敛点）。
