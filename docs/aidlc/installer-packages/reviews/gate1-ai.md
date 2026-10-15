# Gate#1 AI 互审（2026-10-15）

> 评审对象：express 两件（prd.md + test-strategy.md @ d9dbb41）对照 intent.md/decisions.md 与 brownfield（release.yml 六 job/三锁定脚本/daemon service 面）。
> 评审者：reviewer-pm（0/4/5）、reviewer-architect（0/4/8）、reviewer-qa（1/3/8）、reviewer-dev（0/3/7）——全部只读。
> 编排者去重合并：**1 BLOCKER / 8 MAJOR / 15 MINOR**。

## BLOCKER

- **B1｜AC1/AC2「artifact 名以 `cloudmount-` 开头」零测试锚——pattern 收编机制链静态 P0 层失守**［qa 定 BLOCKER；arch M2/pm 隐含同证］
  - 证据：prd AC1/AC2 明列「artifact 名以 `cloudmount-` 开头」为验收子句（收编前提，prd L17/L21/L56）；test-strategy T2/T6 仅断言「打包步输出名/upload path 严格匹配」+`if-no-files-found`——upload-artifact 的 `name:`（artifact 名）与 `path:`（文件路径）是两个字段，path 严格匹配不约束 name；全文无一处锚定 name 字段。
  - 失败模式**静默**：artifact 名落 glob 外 → release job download `pattern: cloudmount-*` 不收编 → Release 照常创建但**缺 MSI/pkg**，所有静态测试全绿；叠加 A1「首个含安装包的 tag 即真实发布」（无 prerelease 预演），只能 T17/T19 发布后人工发现。
  - 先例矛盾：上 intent 对 mac 腿有同型断言（release-yaml.test.sh L176「upload artifact name 以 cloudmount-darwin 开头」）——属遗漏非取舍。
  - 修复：T2/T6 各补一条「upload 步 artifact name 以 `cloudmount-` 开头（pattern 收编前提）」断言，成本极低。

## MAJOR（8）

1. **AC3 可测性拆解与 T11 口径直接冲突，且既有 job/产物名机器断言面实际为零**［pm M2+arch M1+qa m3 同根］：prd L25 说「新断言脚本保证关键锚点（既有 job 名、step、产物名）不丢失」，T11 说「新脚本不重复断言既有 job 内部」——互斥；且实测锁定脚本只守护 binaries-macos/needs 三护栏/on:/body mac 行，`cloudmount-linux-*.tar.gz`、`cloudmount-windows-x64.zip`、`digests-*` 等既有产物名与 binaries-linux/windows/docker/merge 的 step **零静态断言**，「零删改」机器可测面仅 needs 护栏+mac 锚点。需 gate1 消歧（建议：新脚本承接既有产物名锚点断言，与锁定脚本重复无害）。
2. **前置硬校验层无测试锚 + T6 双 arch 只出一 arch 的兜底不成立**［arch M3+qa M1 同根］：prd L61 声明两层防护（前置硬校验+`if-no-files-found` 兜底），但 T2/T6 只测兜底层；`if-no-files-found: error` 仅在 glob 完全落空时触发——双 arch 只产出一个时 glob 命中 1 文件、job 成功、Release 静默缺一个 arch 的 pkg。修复：T6 钉死 upload path 为双字面精确路径（非 glob）或补「打包步前置校验存在」断言（镜像 package-macos.sh 硬校验先例）。
3. **MSI 升级/重装边界披露不实 + 安装器版本取值来源未定义**［pm M3+dev M3 同根］：WiX 语境无良性「缺省升级行为」（无 UpgradeCode 时重复安装被 MSI 引擎阻断或 ARP 双条目）；非目标「不做专门文档」使 v0.1→v0.2 升级即撞墙无引导。且 MSI ProductVersion/pkg `--version` 取值面（Cargo.toml vs `github.ref_name`）全文未提——不 bump Cargo.toml 则安装器版本恒 0.1.0。修复：非目标措辞改「升级不设计不保证，Release notes 提示先卸载再装」+版本取值钉 `github.ref_name`。
4. **双 SOP 并存 + 人工核验清单固化载体未定**［pm M4+arch M4+qa m6 同根］：macos-release ops-runbook 仍载「首 tag prerelease 预演」（该预演从未执行——v0.1.0 tag 在 mac 腿合入之前），其 L23 硬门「三脚本（9/23/13）」也将因第四脚本过时；test-strategy L47 要求清单「固化进发布 SOP 且打 tag 前就绪」但载体未定（本 intent 无 runbook 交付物）。修复：operations 阶段产出本 intent runbook 固化清单并显式声明取代 macos-release runbook 发布流程节。
5. **T16 P1 定级与地位不匹配 + SKIP 判据主观**［qa M2］：T16 是 MSI 安装行为唯一 tag 前验证路径；P1 下 SKIP（「工具链本地不可装」）无客观证据要求、无裁决留痕。修复：升 P0 或 SKIP 需显式人工裁决记 decisions.md。
6. **T20 降级口径不完整**［qa M3］：「无 mac 机则降级为资产完整性+体积核验」——资产完整性无判据（XAR 头 magic/digest？）、降级后零验证无补救闭环。修复：降级判据钉死（如 pkg 文件头 `xar!` magic + 体积带）+真机补验触发条件入 SOP。
7. **断言对象路径未钉死 + 新断言脚本名两处冲突**［dev M1+qa m7+pm MINOR-1 同根］：T3/T4/T8 断言对象「安装器定义文件（路径 Construction 定）」悬空——gate1 是 express 最后裁决点，不钉死则 TDD 红态断言无法落笔；prd AC6 写 `installer-yaml.test.sh`（单数）vs test-strategy 三处 `installers-yaml.test.sh`（复数）——T14 断言字面即文件名本身，必须单一事实来源。路径惯例冲突：cargo-wix 惯例 `crates/<pkg>/wix/main.wxs` vs 仓库惯例集中 `packaging/`。修复：gate1 钉死 `packaging/wix/main.wxs`、`scripts/package-pkg-macos.sh`（沿 package-macos.sh 先例）、脚本名统一（复数）。
8. **WiX v3 EOL + cargo-wix 维护态 + CI 安装锁定策略全缺**［dev M2+pm MINOR-4 同根］：cargo-wix 封装 WiX v3（已停止维护，v4 语法不兼容）；cargo-wix 缺省模板按单 package 单 bin 生成，双 bin+PATH 几乎必然手改 .wxs——「手写 .wxs+直接 WiX」名义降级路径实为大概率主路径；CI 侧安装方式与版本锁定（`cargo install --version X --locked` vs 预编译下载）未定义——不锁版本=不可复现构建。修复：Q2 裁决把 WiX v3 EOL 计入权衡（推荐倾向手写 .wxs+CI 直接 WiX）+ CI 安装锁定策略入 plan。

## MINOR（15）

1. AC2「不含 launchd 服务注册」子句零测试锚（T8 只断言 desktop/.app——MSI 侧 T4 有服务注册负断言，mac 侧缺等价物）。
2. AC1「产出**且仅**产出」的「仅」无负断言（T2 只测存在性）。
3. PATH 刷新「文档边界」悬空（prd L53 声明已开终端不刷新，但 AC4/T12 断言不含「新开终端生效」提示——用户会当缺陷上报）。
4. AC4「产物表新增两行」与 T12 双 arch 字面断言不匹配（按先例每 arch 一行则新增三行；prd 占位写法 `<arch>` 单行与 T12 必红）。
5. AC4 无负路径（可补：签名说明须双平台覆盖，漏 Windows 即缺陷；body 不含「已签名/已公证」误导措辞负断言）。
6. T21 场景含 AC4 验证内容但映射列仅 AC5——双向映射未体现（建议补 AC4）。
7. T9 护栏不对称（安装包 job 无 `if:`/status 覆盖未镜像 T10；`continue-on-error` 白名单偏宽可收窄至 build/package/upload 步）。
8. T10/T13 与锁定脚本既有断言完全重复、与 T11「不重复」自设约束矛盾（冗余无害但文档自相矛盾——建议明确「允许对被修改面做包含式护栏」）。
9. Q1-Q4 仍未裁决而 AC 已按建议默认值写成定论（gate1 通过即隐式批准——建议显式拍板）。
10. UAC 预期未披露（Machine 级 PATH+Program Files 必提权；T16/T18 的 msiexec /qn 需管理员 shell——判据需注明）+ T18「新 shell」需钉死「OS 新开终端」（Git Bash 子 shell 继承旧环境会假阴性）。
11. rc 形态 tag（如 v1.0.0-rc1）的「正式 Release」语义未定义（softprops 是否自动置 prerelease 未验证）。
12. pkg 未显式排除 preinstall/postinstall 脚本（T7/T8 无对应负断言——安全面）。
13. 元数据 owner 不一致将流入安装器（Cargo.toml repository=zhonghaoxi vs release.yml owner=tintac-cn；pkg identifier 反向 DNS 未定）。
14. MSI job 构建来源未指明（重编译+rust-cache vs 下载 binaries-windows artifact——失败传播语义不同，建议明示重编译）。
15. AC3③「仅两类追加」与 AC4 落点歧义（既有签名段是独立段落非表行——MSI 说明追加新行还是改既有段，diff「仅追加」口径需明确）。

## 结论

**1 BLOCKER / 8 MAJOR / 15 MINOR。** 正向确认：语义主干（glob 收编链/tag 即发布链/安装面语义/fail-fast 门控/可行性披露诚实——cargo-wix 标注 Construction 首日验证沿 B1 教训）三方自洽且 brownfield 实证兼容；AC↔T 映射无缺锚；范围纪律与 A1 裁决一致。但 B1（pattern 收编链静态层失守，静默失败叠加首个 tag 即真实发布）+ M1/M2（守护口径冲突与兜底不成立）使 P0 静态层有实质缺口，M7/M8 把关键裁决（断言对象路径/工具链选型含 WiX v3 EOL）转嫁 Construction。

**建议：打回修订（phase 回 inception）**——只重跑 pm+qa 修订被点名项（B1+M1/M2/M3/M5/M6 断言与措辞、M7 路径钉死后回填字面、prd 非目标措辞与 AC4 行数口径），gate1 一并裁决：Q1-Q4 显式拍板（Q2 计入 WiX v3 EOL 权衡）+ 新钉死项（packaging/wix/main.wxs、scripts/package-pkg-macos.sh、断言脚本名统一复数、安装器版本取 github.ref_name、SOP 载体=operations 新 runbook 取代声明）+ T16 升 P0/SKIP 留痕。修订后复审重点核对 B1/M1/M2/M7 消解。
