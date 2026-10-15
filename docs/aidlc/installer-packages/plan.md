# Plan: installer-packages

> 产出者：编排者（Sprint Planning）　日期：2026-10-15　前置：gate1 approved（@ 591a438，含准出回填 @ e7fc5c1）
>
> 依据：prd v3（AC1–AC6，gate1 修订轮+准出回填后）、test-strategy v3（22 点 P0=15/P1=6/P2=1，含 Construction 首日 MINOR 1-11 回填 @ ee3a0fc）、gate1 裁决（Q1-Q4/钉死项/T16 升 P0——decisions.md GATE-APPROVED gate1 2026-10-15）。
> TDD 铁律：每 unit 先提交失败测试（红）→ `/aidlc:lock-tests` 置锁 → 实现转绿。锁范围为失败测试文件并集（只增不减）；上 intent 三锁定脚本零触碰。

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | 安装包结构断言脚本（关键路径，红阶段） | `scripts/installers-yaml.test.sh`：T1-T14 断言全集（MSI/pkg job 结构、artifact name 前缀、精确路径、前置硬校验字面、needs 门控+护栏、收编链中段三字面、范围排除（desktop/服务注册/launchd/preinstall）、发布语义负断言、版本字面、.wxs token（ProgramFilesFolder/Environment/System="yes"）、actionlint 降级）——沿上 intent 三脚本先例（防空真/CRLF 剥除/计数诚实/job_block 提取复制注） | `scripts/installers-yaml.test.sh`（新增，TDD 红提交） | low（新文件零既有引用） | T1-T15 断言规格 | 红态：被测对象（两 job/wxs/pkg 脚本）不存在，mac/pkg 断言全红+护栏现状绿；红提交只含测试文件 |
| U2 | Windows MSI 腿（绿阶段） | `packaging/wix/main.wxs`（双 bin 铺放 Program Files\CloudMount+Machine PATH+ARP 条目；Manufacturer=`tintac-cn`（MINOR⑫ 落位：安装器元数据统一取 GitHub 实际 owner）；UTF-8 BOM 或英文 ARP 描述——首日验证定）+ release.yml 增 `installer-msi-windows` job（**job 内重编译+Swatinem/rust-cache**（MINOR⑫ 落位：不下载 artifact，失败传播=job 自身失败）；WiX 构建（安装方式与版本锁定首日定——v3 choco vs v4/v5 dotnet SDK）；前置 .exe 存在性校验；upload `cloudmount-windows-x64.msi`+`if-no-files-found: error`+artifact name `cloudmount-` 前缀）+ release needs 追加 + 产物表 MSI 行+签名/安装侧提示 | `packaging/wix/main.wxs`（新增）、`.github/workflows/release.yml`（仅追加） | med（release.yml 唯一存量文件——改动语义=仅追加新 job/needs 项/产物表行，T10/T11 护栏断言守护零删改） | T1-T4 断言转绿（pkg 断言仍红为预期） | U1 断言脚本 MSI 侧全 PASS；**T16 本地 WiX 冒烟**（P0：choco/winget 装 WiX→本地构建 MSI→msiexec 静默装/卸+PATH 验证；不可装则 SKIP 呈人工裁决留痕） |
| U3 | mac pkg 腿（绿阶段） | `scripts/package-pkg-macos.sh`（staging 归集+前置硬校验+dry-run 段本地可跑——**pkgbuild 段 mac 独占**（prd R2-MAJOR-1 边界）；identifier=`io.github.tintac-cn.cloudmount`（MINOR⑫ 落位）；`--version` 取 `github.ref_name`）+ release.yml 增 `installer-pkg-macos` job（macos-15 顺序步双 target（沿 binaries-macos 先例）；调 package-pkg-macos.sh；upload 双 arch pkg 精确路径+`if-no-files-found: error`+artifact name 前缀）+ needs 追加 + 产物表 pkg 2 行 | `scripts/package-pkg-macos.sh`（新增）、`.github/workflows/release.yml`（仅追加） | med（同 U2——仅追加语义） | T5-T8 断言转绿 | U1 断言脚本**全绿**（22/22 或含降级分支的诚实计数）；三锁定脚本回归仍绿 |
| U4 | 终验收尾 | 终验组合全绿+基线比对零回退；T16 冒烟结论留痕（执行或人工裁决 SKIP）；MINOR 12 三项落位核验 | 无新文件（核验+留痕） | low | 终验组合 | `cargo test --workspace` 与基线逐数一致+fmt/clippy 净+四脚本测试全绿（9/23/13/N）→ phase 置 gate2 |

## 验证命令（F2 规范来源）

- **基线/回归命令**：`cargo test --workspace`（第 0 步记录 `state.testBaseline`；本 intent 零 Rust 改动——改动面=.github/scripts/packaging/docs，终验比对预期零回退）
- **终验组合**：上命令 + `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`（四件套惯例）+ `bash scripts/package-macos.test.sh` + `bash scripts/release-yaml.test.sh` + `bash scripts/check-macos-yaml.test.sh` + `bash scripts/installers-yaml.test.sh`（U1 交付后加入）+ `actionlint`（可用时；不可用降级已内嵌于各脚本）

## 执行顺序与并行泳道

- 泳道 1（断言关键路径）：U1（红）→ 置锁
- 泳道 2（实现串行）：U2（MSI 腿）→ U3（pkg 腿）——共用 release.yml 与断言脚本绿态判定（逐断言 PASS/FAIL 可观测部分绿），串行避免冲突
- 泳道 3（收尾）：U4
- 波次：①U1 红+锁 → ②U2 → ③U3 → ④U4 终验 → gate2
- 影响面说明：U2/U3 的 release.yml 为唯一存量文件（med）——改动语义=仅追加（新 job/needs 项/产物表行），由 T10（needs 护栏）+T11（既有产物名+收编链锚点）+上 intent 三锁定脚本回归三重守护；无 high/unknown 项。串行派发（沿限流教训）。

## TDD 锁记录

- 待置位：U1 红提交后锁 `scripts/installers-yaml.test.sh`（与上 intent 三锁定文件并集——本 intent 契约集四文件）。契约错误走 `/aidlc:unlock-tests <理由>` 并留痕 decisions.md。

## gate1 授权落位清单（Construction 首日已执行/随 unit 落）

- **已执行（@ ee3a0fc）**：MINOR 1-11 回填（T12 三关键词/T7 版本锚/T11 收编链三字面/T9 消歧/SOP 管理员前置/T6 归因/T1-T5 口径/T8 preinstall/T3 token/T2 收窄/prd 首日验证补 WiX 版本+UTF-8）。
- **随 U2/U3 落（MINOR 12）**：安装器元数据 owner=`tintac-cn`（GitHub 实际 owner；Cargo.toml repository=zhonghaoxi 为历史不一致不流入安装器）；MSI Manufacturer=`tintac-cn`、pkg identifier=`io.github.tintac-cn.cloudmount`；MSI job 构建来源=job 内重编译+rust-cache。
- **T16（P0）**：U2 内执行本地 WiX 冒烟；不可装则 SKIP 呈人工裁决（AskUserQuestion）留痕 decisions.md。
