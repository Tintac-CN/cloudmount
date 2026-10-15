# Test Strategy: macos-release

> 产出者：qa　日期：2026-10-15　来源：intent.md + prd.md
>
> AC 映射说明：映射列与 prd.md 实际 AC1..AC5 对齐（对齐拍回填；gate1 打回修订仅动被点名项）——AC1=tag 触发双架构产物附 Release；AC2=打包脚本参数化+CI/本地同脚本同布局；AC3=本地预检（B1 收窄为 dry-run 一道）+ mac check job（workflow_dispatch 双 target check）+ 预检说明；AC4=Release notes 产物表 mac 行+签名状态提示；AC5=既有腿零回归+needs 仅追加+门控语义不削弱。AC1 锚=T9/T10/T14/T15/T17（T10=AC1/AC2、T17=AC1，与 prd 括注双向一致）。
> 开放问题已裁决（gate1）：Q1 runner=pin `macos-15`（arm64）单 runner 双 target（x86_64 经 rustup target 交叉编译）；Q2 产物命名=`cloudmount-darwin-<arch>.tar.gz`；Q3=docs/edge.md 补最小 mac 小节。断言字面值已写死。
> 已知事实（编排者核查）：release.yml 真发布仍仅 push tag `v*` 触发；workspace 在 ci.yml `macos-latest` 腿已绿；本地为 Windows/Git Bash，无 Apple SDK/链接器/签名能力。

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| T1 | AC2 | 正：Git Bash 本地执行 `scripts/package-macos.sh` dry-run（占位产物、参数化产物目录）→ 退出码 0，产出 tar.gz；隔离断言：占位产物只写 dry-run 专属目录，不进任何上传路径 | 单元 | P0 |
| T2 | AC2 | 负：暂存目录缺任一必含文件（daemon/cli 二进制或 README.md/edge.md 文档）→ 脚本整体非零失败且不产出 tar（fail-fast，杜绝残缺 tar 进入上传步） | 单元 | P0 |
| T3 | AC2 | 负：必填参数缺失 → usage 报错、非零退出（产物目录路径非法 → 专用报错非零退出，不带 usage——与实现对齐）；无半成品目录或空 tar 残留（路径非法负用例未入自动化断言，gate2 互审 MINOR 备案） | 单元 | P1 |
| T4 | AC2 | 正：`tar -tzf` 断言完整期望清单 = 根级 `cloudmountd`+`cloudmount`+`README.md`+`edge.md` 四项（不多不少；断言前归一化——剥 `./` 前缀、忽略目录条目，对齐 linux 腿 `tar … .` 打包法的清单形态），且不含 `packaging/systemd/*`；CI 调同一脚本 | 单元 | P0 |
| T5 | AC2 | 负（范围排除）：dry-run 产物清单断言不含 desktop/.app 任何条目（A1：desktop 不入本期 mac 产物；A1 裁决守护） | 单元 | P1 |
| T6 | AC3 | 正（集成静态断言）：workflow 含 workflow_dispatch 触发的 mac check job——断言 runner=`macos-15`、rustup targets 含 aarch64+x86_64-apple-darwin、`cargo check --target` 双 target 步骤存在 | 集成 | P0 |
| T7 | AC3 | 正（线上人工）：手动触发 workflow_dispatch → mac check job 绿（判据=双 target check 退出码 0，真 mac runner 上验证编译面） | E2E | P0 |
| T8 | AC3 | 正：本地预检收窄为 dry-run 一道（判据退出码 0）；预检说明文档存在、步骤可照做（AC3 硬交付物） | 单元（预检）+文档 | P1 |
| T9 | AC1 | 正：release.yml 结构断言——存在 binaries-macos job（runner=`macos-15`），同一 job 顺序步产出 aarch64 + x86_64 双 target 产物（固定单 runner，不采用 matrix——断言无 strategy:/matrix: 且双 target 字面在位） | 集成 | P0 |
| T10 | AC1/AC2 | 正：binaries-macos 步骤调用 `scripts/package-macos.sh`（CI 与本地同脚本，dry-run 结论才对 CI 有效）；mac upload 步含 `if-no-files-found: error`（glob 落空即整体失败，杜绝静默零文件），glob `cloudmount-darwin-*.tar.gz` 与脚本输出命名严格匹配 | 集成 | P0 |
| T11 | AC5 | 正+负：release job `needs` 含 binaries-macos（断言缺失即测试失败）→ mac job 失败则 release job 不执行、Release 不产出；静态断言 release job 不带 `if:`/status 函数覆盖（防 Construction 加 `if: always()` 破坏 needs 门控语义） | 集成 | P0 |
| T12 | AC4 | 正：Release notes 产物表含 mac 行，注明无 Developer ID 签名/公证（arm64 为链接器 ad-hoc 签名）+ Gatekeeper 提示（`xattr -cr` 或右键打开）——grep/人工断言 | 集成（文档断言） | P1 |
| T13 | AC5 | 负：`actionlint` 对 release.yml 通过（整文件合法性=零回归静态校验）；另对注入语法错误的副本文件运行 actionlint 确认能拦截（校验拦截手段本身有效）。actionlint 本地不可用时降级为 YAML 解析 + T9-T11 结构断言，并记缺口 | 集成 | P1 |
| T14 | AC1+AC5 | 正（线上人工）：push 真 tag `v*` → workflow 全绿（含 binaries-macos），两架构 tar.gz 附于 GitHub Release；既有 linux 双架构/windows 资产仍在 Release（资产集合不变，AC5） | E2E | P0 |
| T15 | AC1 | 正（线上人工）：下载两 tar，解包后 `file` 验证分别为 Mach-O arm64 / x86_64，文件清单齐（四项：`cloudmountd`+`cloudmount`+`README.md`+`edge.md`） | E2E | P0 |
| T16 | AC4 | 正+负（线上人工，需真 mac 机）：Release 页 mac 行可见、两 tar 可下载；负路径 = 无 Developer ID 签名/公证（arm64 ad-hoc）的二进制首启被 Gatekeeper 拦截为预期行为，notes 提示（`xattr -cr`/右键打开）可解除 | E2E | P1 |
| T17 | AC1 | （线上人工，可选）真 mac 机冒烟：`cloudmountd --serve` / CLI 基本命令可运行（x86_64 二进制在 arm 机器可经 Rosetta 验证） | E2E | P2 |

负路径覆盖核对：打包主流程 T2/T3；范围排除 T5；workflow 语法/结构拦截 T13/T11（含 `if:` 覆盖防护）；upload glob 落空由 T10 `if-no-files-found: error` 拦截；Gatekeeper 权限拦截 T16。每条主流程 ≥1 负路径，满足。

## 分层策略

- **单元**：打包脚本 dry-run（Git Bash 本地可执行：退出码 + `tar -tzf` 完整清单断言，T1-T5、T8）。不测：真实链接、真实 mac 运行行为、desktop 项目（本期排除）。
- **集成**：actionlint + release.yml 静态结构断言（T6 mac check job 定义、T9 双 target 产物存在、T10 同脚本 + `if-no-files-found: error` + glob 匹配、T11 needs 门控 + 无 `if:` 覆盖、T12 notes mac 行、T13 拦截手段自校验）。不测：runner 真实执行。
- **E2E**：线上人工——T7（workflow_dispatch 手动触发 mac check，tag 前预演编译面）+ T14-T17（真 tag 全链路：产物、Release 面、Gatekeeper、冒烟）。真发布无法全自动覆盖（仅 tag 触发、macos runner 无法本地模拟——nektos/act 仅 Linux 容器），落人工核验清单，见缺口节。

## 缺口与开放问题

- [ ] **【缺口·E2E 不可自动化】mac 真实链接、产物与 Gatekeeper 行为无法本地验证**（无 Apple SDK/链接器/codesign 三重依赖；osxcross 法律灰不建议；act 模拟不了 macos runner）。T7 可经 workflow_dispatch 在 tag 前线上验证编译面，但产物/Release 面只能真 tag 触发后人工核验。**人工核验清单**（发布 SOP 固化）：
  0. **发布前置第 0 步**：tag 前手动触发 `.github/workflows/check-macos.yml`（workflow_dispatch）并确认绿——编译面预检通过才打 tag（gate1 B1 裁决的编译面预检唯一路径，补回 B1 改写后消失的"check 失败不得打 tag"过程纪律）；
  1. 两架构 tar（aarch64 + x86_64）均可从 Release 页下载；单个体积超出 5–50 MB 带即人工复核（对照 linux 腿 musl tar 量级）；
  2. 解包文件清单齐 = `cloudmountd` + `cloudmount` + `README.md` + `edge.md` 四项，不含 `packaging/systemd/*`；
  3. `file` 输出含子串 `Mach-O` 且分别含 `arm64` / `x86_64`（子串匹配，防 file/libmagic 版本措辞漂移）；
  4. Release notes 产物表含 mac 行，含签名状态（无 Developer ID 签名/公证，arm64 ad-hoc）与 Gatekeeper 提示。
- [ ] **【缺口·拦截手段依赖】actionlint 在本地 Windows/Git Bash 未必可用**；不可用时降级为 yq/python YAML 解析 + 结构断言（T6、T9-T11），工具可用性在 Construction 首日确认并回写。
- [ ] **【缺口·首次即生产】真发布仍仅 tag 触发：B1 的 dispatch mac check（T7）可预演编译面，但 Release 资产/notes 面（T14-T16）无 tag 前预演，首个含 mac 腿的 tag 即真实发布**。开放问题（需人类裁决）：首次验证是否允许用 prerelease 标记/预发布 tag 走一遍 T14-T16，降低对正式 Release 列表的污染。
- [ ] **【已裁决·gate1】Q1=pin `macos-15`（arm64）单 runner 双 target（x86_64 经 rustup target 交叉编译）；Q2=产物命名 `cloudmount-darwin-<arch>.tar.gz`；Q3=docs/edge.md 补最小 mac 小节。**T4/T6/T9/T10 断言字面值已按裁决写死；edge.md 的 mac 小节内容由 pm/写手落，其存在性由 T4 清单断言覆盖。
- [ ] 映射列已经对齐拍回填为 prd 实际 AC；AC1..AC5 全部有 T 点落点，无缺锚。
