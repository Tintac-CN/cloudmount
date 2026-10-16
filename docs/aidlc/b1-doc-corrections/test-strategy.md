# Test Strategy: b1-doc-corrections

> 产出者：qa　日期：2026-10-16　来源：intent.md + prd.md
>
> **AC 锚位说明（prd 未落盘）**：映射列按 decisions.md A1 四拆骨架预映射（拆①16 处回写／拆②owner 修正／拆③死路径+TLS 注释／拆④回写后验证），标注「待对齐拍核对」——**禁止虚构编号**，prd.md 落盘后由对齐拍回填实际 AC 编号。
> **本批定位＝回写验证策略**（文档批 qa）：TP 四要素沿 audit-roadmap-next-steps 先例（核验对象/执行动作/通过判据/证伪判据）；回归语义沿用 plan.md 特化——**终验重跑同一命令，基线计数不变＝零影响证明，任何漂移＝回写过程污染工作区＝BLOCKER**。
> **qa 只读实测基线（2026-10-16，写本策略前核验）**：
> - 16 处清单行锚逐一实读在位：README.md:89/:96/:133/:151、ROADMAP.md:111/:124/:176、ARCHITECTURE.md:22/:48/:86、web-ui/README.md:7/:10/:18/:23、web-ui/src/api.ts:4、根 Cargo.toml:12。
> - **清单外残留实测 7 处**：`crates/cloudmount-core/src/config.rs:3`（「Tauri 壳」注释）、`crates/cloudmount-daemon/src/webdav.rs:694/:850/:874/:893`（四条「与 Tauri 壳/命令同构/同名」注释）、`web-ui/package.json:6`（description「Tauri 桌面壳」）、`.gitignore:12`（「# Tauri」节注——:13-14 删行若不连同删 :12，TP-02 清零必失败）。见缺口节第 1 条裁决请求。
> - 干净面实测：`apps/desktop/src`+`apps/desktop/Cargo.toml` 零 tauri（egui/eframe 0.31+wgpu @ Cargo.toml:3,12-13，repository=Tintac-CN @ :7）；`.github/`、`packaging/`、`edge/`、`scripts/`、`drivers/`、`apps/shell-macos/` 零 tauri；`web-ui/src/platform.ts` 零 tauri（:2-5 已单宿主化，hostKind 恒 "web"）——`__TAURI_INTERNALS__` 字面在产品面仅存 `web-ui/README.md:10`（=16 处清单内）；coordinator 问句实测：webdav.rs:937 邻近＝`/api/*` match 臂（:924-963），**零 tauri 字面**。
> - 五断言脚本在位（count 实测）：`scripts/{desktop-installers-yaml(695 行),installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh`；基线 112/72/23/13/9+277/0（audit 批 plan.md 2026-10-16T21:26 录制）。**脚本文档字面断言专项核对（实测完成）**：唯一触及本批对象族者＝desktop-installers-yaml.test.sh:697-698（`gen_icons.py` 无 src-tauri 字面——对象是 scripts/gen_icons.py，**不在回写面**且现已零命中）；package-macos.test.sh 的 README.md 断言＝归档清单**存在性**非内容（:145,:213,:225）——README 内容回写不影响计数。
> - owner 三方实测：根 Cargo.toml:20 `repository=zhonghaoxi` + **:21 `homepage=zhonghaoxi`（audit NEW-5 未列的第二处错侧）** + :22 `authors=["zhonghaoxi"]`（个人署名）vs apps/desktop/Cargo.toml:7 `Tintac-CN` vs .git/config:12 `git@github.com:Tintac-CN/cloudmount.git`。

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| TP-01 | 拆①（待对齐拍核对） | **16 处回写逐处验证（正路径+证伪）**：核验对象＝intent 受影响面 8 文件的 16 处行锚（qa 实读清点见头部）。动作：逐文件 `grep -in tauri <file>` 判零；新表述事实锚 grep（`egui\|eframe\|纯 Rust\|单宿主`）逐处落字；逐处对照 audit-findings §2.1①/§2.2 NEW-1 行锚不漂移。内容事实锚（防「换词不换实」）：README:89「桌面壳与边缘网关**共用同一个 web-ui**」与 :133「托盘 + web-ui」须随回写订正为 egui 原生 UI 事实（desktop 不嵌 web-ui）；README:96 架构图同步；ROADMAP L111/124 `[x]` 条目按 intent 开放问题①以「事实订正」形态保留完成语义（egui 壳完成同等功能）；web-ui/README.md 宿主表与 :10「运行时探测 `__TAURI_INTERNALS__`」订正为单宿主事实（platform.ts:9-11 已实测佐证）；api.ts:4 头注改「边缘单宿主＝cloudmountd serve」。通过：16 处逐一零 stale 字面+新表述含事实锚。**证伪：任一行锚 tauri 残留，或新表述仍声称 WebView/双宿主/内嵌 frontendDist → 回写假完成** | 单元 | P0 |
| TP-02 | 拆④（待对齐拍核对） | **全仓 `grep -ri tauri` 清零验证（负路径·本 TP 关键设计＝排除清单）**：核验对象＝排除清单外全仓文件面。动作：**分片扫描**（实测证据：本仓树含 target/node_modules 时全仓单命令 30s 超时×2——必须排除，禁全仓裸跑）；分片清单＝根散文件（README/Cargo.toml/.gitignore）+docs/（非 aidlc）+crates/+apps/desktop/{src,Cargo.toml}+apps/shell-macos/+web-ui/{src,README.md,package.json}+.github/+packaging/+edge/+scripts/+drivers/；主口径子串 `grep -rin tauri`，**词界复核**（实测：`DataURI`/`datauri` 含子串 "tauri"——node_modules 命中全为此类误报，产品面若现同形标识符须人工判别）。**排除清单（合法保留，逐一列明）**：①`docs/aidlc/**`＝历史 AIDLC 工件审计链（intent 约束不改；含 backlog.md 台账、audit-findings.md 偏离记录本身、b1-doc-corrections 本批工件——tauri 字面属发现转述）；②`target/`（workspace 根+apps/desktop/target/）构建产物；③`web-ui/node_modules/`（第三方依赖，命中=DataURI 误报实测）；④`web-ui/dist/` 构建产物；⑤`.git/`。通过：排除清单外全仓零命中（以缺口节第 1 条裁决后的回写面为准——现清单外残留 7 处：config.rs:3、webdav.rs:694/850/874/893、package.json:6、.gitignore:12）。**证伪：排除清单外任一命中未回写且未记为发现 → 清零声明假完成；把合法保留侧计入命中 → 假阳性阻塞** | 集成 | P0 |
| TP-03 | 拆①（待对齐拍核对） | **死命令替换实跑可达（正路径+证伪）**：核验对象＝README:151 替换后命令（现值 `cd apps/desktop && cargo tauri dev`——apps/desktop 无 tauri 依赖无 tauri.conf.json，实跑必败）。动作：在 apps/desktop 实跑替换命令（预期形态 `cd apps/desktop && cargo run --release`——desktop 为 workspace exclude 独立 cargo 项目，目录内裸 cargo 可用；构建可达即证，不需 GUI 交互）；回写后该行与「开发者」节其余命令并存自洽。通过：替换命令构建退出 0（或 run 起动可达）。**证伪：替换后命令仍不可执行（引用不存在的 target/flag/路径）→ 死命令换死命令，首屏 P1 未消** | 集成 | P0 |
| TP-04 | 拆②（待对齐拍核对） | **owner 元数据三处一致+字段边界（正路径+边界裁）**：核验对象＝根 Cargo.toml:20-22、apps/desktop/Cargo.toml:7、.git/config remote。动作：三方 grep 对照 repository 字面须同为 Tintac-CN；**边界项（qa 实测扩展）**：根 Cargo.toml 错侧实为 :20 repository+:21 homepage 两字段（audit NEW-5 仅列 :20），:22 `authors=["zhonghaoxi"]`＝个人署名非 owner——homepage 随批改、authors 是否保留，以 prd/对齐拍裁决为准（缺口节第 3 条）。通过：repository 三方一致；homepage 按裁决落字。**证伪：任一 repository 字面仍 zhonghaoxi 或三方不一致 → owner 修正假完成** | 单元 | P1 |
| TP-05 | 拆④（待对齐拍核对） | **回归保护·基线不变（负路径·终验）**：核验对象＝五断言脚本+workspace 基线+lint 双面。动作：回写完成后逐个 `bash scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh`，末行计数对照基线 **112/72/23/13/9**（desktop 在 actionlint 可用环境为 113——环境相关口径沿 audit 批如实注记）；`cargo test --workspace` **277/0**；`cargo fmt --all -- --check`+`cargo clippy --workspace --all-targets -- -D warnings`（**webdav.rs 注释行改动后必跑**——注释行在 workspace fmt/clippy 覆盖内）；desktop 侧 fmt/clippy：本批 desktop 零改动（qa 实测干净面），**显式声明豁免**并留豁免理由（沿 audit 批 TP-14 双面口径的反向应用）。通过：五计数与基线一致+277/0+workspace fmt/clippy clean。**证伪：任一脚本计数漂移或退出非零、failed>0、lint 报错 → 回写过程污染工作区＝BLOCKER（回归语义特化沿用）**。注：脚本面已专项核对（见头部实测）——回写不触任何断言输入，计数不变为强预期 | 集成 | P0 |
| TP-06 | 拆③+全局约束（待对齐拍核对） | **「仅注释/头注/字段」零逻辑变更边界+关联订正落字（负路径·diff 面核验）**：核验对象＝代码邻接文件 diff（webdav.rs/api.ts/根 Cargo.toml）+两处关联订正（webdav.rs:29 TLS 注释、.gitignore:12-14）。动作：`git diff` 逐文件核 HUNK 类别：webdav.rs 仅注释行（**:14 清单外张力**：另实测 ：694/850/874/893 四条 Tauri 注释——若经缺口节裁决扩入回写，diff 仍须 100% 注释行）；api.ts 仅 :2-4 头注；根 Cargo.toml 仅 :12 注释+:20(/:21) 字段；.gitignore 仅 :12-14 删行（**须连同 :12「# Tauri」节注三行一起**——只删 :13-14 会在 TP-02 留残留）；任何文件不得出现代码/依赖/行为分支 HUNK。webdav.rs:29 新表述事实锚＝`--tls-cert/--tls-key` 字面在位（对齐 NEW-2 建议与 audit 批 AC13⑦ TLS 收敛口径：**可选非默认**，非「无 TLS」）。通过：diff 面全部落在注释/头注/字段/删行四类+订正落字含事实锚。**证伪：diff 出现任何逻辑 HUNK → 违反「零产品逻辑变更」intent 约束＝BLOCKER；webdav.rs:29 仍写「TLS 见 M4 后续」→ 订正假完成** | 单元 | P1 |
| TP-07 | 拆④（待对齐拍核对） | **历史工件合法保留清点（正路径盘点·排除面可审计性）**：核验对象＝`docs/aidlc/**` 的 tauri 字面全量（qa 实测 docs/ 下 tauri 命中 28 文件＝aidlc 26+ROADMAP+ARCHITECTURE）。动作：分片 grep docs/aidlc 清点命中文件与条目数，登入验证记录为「合法保留清单」（逐条溯源：哪一 intent 的哪份工件、字面属何种转述——audit-findings §2 偏离记录/backlog 台账🚀行/本批 intent 引文）；与 TP-02 排除清单互证一致。通过：保留侧全量在案可溯源。**证伪：保留侧未清点（清零声明的排除面不可审计）或误把产品文档归入保留侧 → 清零验证不可信** | 单元 | P2 |

**负路径覆盖核对**：四拆每条主流程 ≥1 负路径/证伪——拆①：TP-01 证伪（残留/换词不换实）+TP-03 证伪（死命令换死命令）；拆②：TP-04 证伪（三方不一致）；拆③：TP-06 双证伪（逻辑 HUNK=BLOCKER/订正假完成）；拆④：TP-02 清零残留探测器+TP-05 回归漂移=BLOCKER+TP-07 排除面不可审计。满足常驻约束。

**执行序注记**：①静态 diff/grep 面（TP-01/06/07——零副作用）→②全仓分片清零扫描（TP-02）→③实跑（TP-03 死命令可达）→④TP-04 三方对照→⑤TP-05 终验（五脚本+277/0+fmt/clippy——**最后执行**，零影响证明以回写全停为前提，沿 audit 批回归语义）。TP-02 判据口径以缺口节第 1 条对齐拍裁决为准。

## 分层策略

- **单元**：静态行锚与 diff 面核验（TP-01/04/06/07）——测：字面存在性/清零、行锚与 audit 明细对应、新表述事实锚在位、diff HUNK 类别边界、合法保留面可溯源。不测：运行时行为、渲染效果、GitHub 侧元数据实际生效（git remote 以 .git/config 本地读为准）。
- **集成**：实跑与基线回归（TP-02/03/05）——全仓分片清零交叉验证、替换命令真实构建、五断言脚本计数+workspace 277/0+fmt/clippy 复跑（终验＝零影响证明）。执行面声明：`cargo test --workspace` 复跑沿用基线口径（本机 cfapi 集成腿瞬态夹具副作用的接受声明沿 audit 批 plan.md 留痕，不在本批重复展开）；TP-03 构建产物落 apps/desktop/target/＝既有 ignore 面，非工作区污染。
- **E2E**：**本轮不适用**——纯文档/元数据批无端到端运行面。唯一端到端等价物＝「全仓清零（TP-02）+基线不变（TP-05）」组合终验：回写完整性与零影响的联合证明；真实 GUI 手测（egui 壳起动观感）、真实发布链路不在本期。

## 缺口与开放问题

- [ ] **【清单-实测差·需对齐拍裁决，影响 TP-02 判据】16 处清单外实测残留 7 处**：`config.rs:3`、`webdav.rs:694/:850/:874/:893`、`web-ui/package.json:6`、`.gitignore:12`（# Tauri 节注）。intent「期望结果＝全仓 grep 清零（排除历史 AIDLC 工件）」与「16 处清单」当前**不自洽**：不扩清单则 TP-02 清零判据必失败（假完成）；选项 A＝回写面扩为 16+7（webdav.rs 四条仍守「仅注释行」边界）；选项 B＝清零口径收窄（产品代码注释列白名单）。**由 pm/architect 在对齐拍裁决并落 prd**。
- [ ] **【prd 未落盘】映射列为四拆骨架预映射**（拆①..④，均标「待对齐拍核对」）——prd.md 落盘后回填实际 AC 编号；本策略不虚构编号。
- [ ] **【owner 字段边界】根 Cargo.toml 错侧实为 ：20 repository + :21 homepage 两字段**（audit NEW-5 仅列 :20）：homepage 随批修正建议纳入；:22 `authors=["zhonghaoxi"]` 为个人署名非 owner，是否保留待 pm/architect 裁（TP-04 判据随裁法定）。
- [ ] **【README 邻接 stale（清单外）】README:143「214 个测试（Windows）/185（Linux）」vs 基线现值 277**：不属 16 处清单，但与「首屏准确性」意图同域——是否随批订正待 pm 裁；若纳入须扩 prd 回写面（守「范围=审核明细钉死」约束的显式扩围，不得顺手段落）。
- [ ] **【ROADMAP 内容性过时面】L124-127 条目正文「web-ui 集成（内嵌 frontendDist）」「custom-protocol feature」**＝Tauri 时代语义（无 tauri 字面但与 egui 事实矛盾）：intent 开放问题①「事实订正形态」须覆盖内容面而非仅字面替换，回写深度（只改字面 vs 重写条目正文）待 gate1 过目。
- [ ] **【合法保留侧边界】docs/aidlc/** tauri 字面＝审计链合法保留**（intent 约束不改历史工件）；web-ui 回写后措辞按 intent 开放问题②＝单宿主**现状事实化**，不写「永不恢复双宿主」（未来裁决不属本批）——TP-01 事实锚按此口径判。
- [ ] **【执行环境实测】全仓单命令 grep 在本仓树超时**（qa 只读工具 30s×2 实证；target/node_modules 体量）——TP-02 执行动作已定为分片扫描，编排者代跑时禁全仓裸跑。
