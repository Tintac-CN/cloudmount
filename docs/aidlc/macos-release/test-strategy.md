# Test Strategy: macos-release

> 产出者：qa　日期：2026-10-15　来源：intent.md + prd.md
>
> AC 映射说明：映射列已对齐 prd.md 实际 AC1..AC5（对齐拍回填）——AC1=tag 触发双架构产物附 Release；AC2=打包脚本参数化+CI/本地同脚本同布局；AC3=本地预检两道（check 双 target+dry-run）+说明；AC4=Release notes 产物表 mac 行+未签名提示；AC5=既有腿零回归+needs 仅追加+fail-fast 不削弱。测试点本体、场景、层级、优先级零改动，仅映射列回填。
> 已知事实（编排者核查）：release.yml 仅 push tag `v*` 触发（PR 不跑）→ mac 产物真实验证只能线上；workspace 在 ci.yml `macos-latest` 腿已绿；本地为 Windows/Git Bash，无 Apple SDK/链接器/签名能力。

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| T1 | AC2 | 正：Git Bash 本地执行 `scripts/package-macos.sh` dry-run（占位产物、参数化产物目录）→ 退出码 0，产出 tar.gz | 单元 | P0 |
| T2 | AC2 | 负：暂存目录缺 daemon 或 cli 二进制 → 脚本整体非零失败且不产出 tar（fail-fast，杜绝残缺 tar 进入上传步） | 单元 | P0 |
| T3 | AC2 | 负：必填参数缺失 / 产物目录路径非法 → usage 报错、非零退出，无半成品目录或空 tar 残留 | 单元 | P1 |
| T4 | AC2 | 正：`tar -tzf` 断言 tar 内文件清单 = daemon+cli 两二进制且路径布局符合预期（与 Linux 腿形态对齐），CI 调同一脚本 | 单元 | P0 |
| T5 | AC2 | 负（范围排除）：dry-run 产物清单断言不含 desktop/.app 任何条目（A1：desktop 不入本期 mac 产物；A1 裁决守护） | 单元 | P1 |
| T6 | AC3 | 正：`rustup target add aarch64-apple-darwin` 后 `cargo check --target aarch64-apple-darwin`（workspace 根，desktop 已 exclude）→ 判据：退出码 0 | 单元（预检） | P0 |
| T7 | AC3 | 正：`cargo check --target x86_64-apple-darwin` → 判据：退出码 0 | 单元（预检） | P0 |
| T8 | AC3 | 负：target 未安装或 check 失败 → 非零退出即预检阻断信号（不得继续打 tag）；双 target check + dry-run 预检说明文档存在、步骤可照做 | 单元（预检/文档） | P2 |
| T9 | AC1 | 正：release.yml 结构断言——存在 binaries-macos job；matrix 恰含 aarch64-apple-darwin + x86_64-apple-darwin 两个 entry；runs-on 与双 entry 的 runner 组合匹配（runner 版本按 intent 开放问题裁决后校正字面值） | 集成 | P0 |
| T10 | AC1/AC2 | 正：binaries-macos 步骤调用 `scripts/package-macos.sh`（CI 与本地同脚本，dry-run 结论才对 CI 有效）；upload 产物 glob 与脚本输出命名（`cloudmount-darwin-<target>.tar.gz`，命名待裁决）严格匹配，不得出现 glob 落空静默零文件 | 集成 | P0 |
| T11 | AC5 | 正+负：release job `needs` 含 binaries-macos（断言缺失即测试失败）→ 语义即负路径拦截：mac job 失败则 release job 不执行、Release 不产出 | 集成 | P0 |
| T12 | AC4 | 正：Release notes 产物表含 mac 行，注明未签名 + Gatekeeper 提示（`xattr -cr` 或右键打开）——grep/人工断言 | 集成（文档断言） | P1 |
| T13 | AC5 | 负：`actionlint` 对 release.yml 通过（整文件合法性=零回归静态校验）；另对注入语法错误的副本文件运行 actionlint 确认能拦截（校验拦截手段本身有效）。actionlint 本地不可用时降级为 YAML 解析 + T9-T11 结构断言，并记缺口 | 集成 | P1 |
| T14 | AC1+AC5 | 正（线上人工）：push 真 tag `v*` → workflow 全绿（含 binaries-macos），两架构 tar.gz 附于 GitHub Release | E2E | P0 |
| T15 | AC1 | 正（线上人工）：下载两 tar，解包后 `file` 验证分别为 Mach-O arm64 / x86_64，文件清单齐（daemon+cli） | E2E | P0 |
| T16 | AC4 | 正+负（线上人工）：Release 页 mac 行可见、两 tar 可下载；负路径 = 未签名二进制首启被 Gatekeeper 拦截为预期行为，notes 提示（`xattr -cr`/右键打开）可解除 | E2E | P1 |
| T17 | AC1 | （线上人工，可选）真 mac 机冒烟：`cloudmountd --serve` / CLI 基本命令可运行（x86_64 二进制在 arm 机器可经 Rosetta 验证） | E2E | P2 |

负路径覆盖核对：打包主流程 T2/T3；范围排除 T5；本地预检阻断 T8；workflow 语法/结构拦截 T13/T11；Gatekeeper 权限拦截 T16。每条主流程 ≥1 负路径，满足。

## 分层策略

- **单元**：打包脚本 dry-run（Git Bash 本地可执行：退出码 + `tar -tzf` 清单断言，T1-T5）+ 双 darwin target `cargo check` 本地预检（T6-T8，判据=退出码 0）。不测：真实链接、真实 mac 运行行为、desktop 项目（本期排除）。
- **集成**：actionlint（可用时）+ release.yml 静态结构断言（grep 式：T9 job 存在且 matrix 双 target、T10 调同一脚本且 upload glob 匹配、T11 release needs 含 binaries-macos、T12 notes mac 行、T13 拦截手段自校验）。不测：runner 真实执行（release.yml 不跑 PR，无法 PR 预演）。
- **E2E**：真 tag 触发的线上人工验证（T14-T17）。CI 不可自动覆盖（workflow 仅 tag 触发，且 macos runner 无法本地模拟——nektos/act 仅 Linux 容器），全部落人工核验清单，见缺口节。

## 缺口与开放问题

- [ ] **【缺口·E2E 不可自动化】mac 真实链接、产物与 Gatekeeper 行为无法本地验证**（无 Apple SDK/链接器/codesign 三重依赖；osxcross 法律灰不建议；act 模拟不了 macos runner）。只能线上 tag 触发后人工核验。**人工核验清单**（发布 SOP 固化）：
  1. 两架构 tar（aarch64 + x86_64）均可从 Release 页下载、体积合理；
  2. 解包文件清单齐：daemon + cli 两二进制；
  3. `file` 输出架构正确：`Mach-O 64-bit executable arm64` / `Mach-O 64-bit executable x86_64`；
  4. Release notes 产物表含 mac 行，含未签名/Gatekeeper 提示。
- [ ] **【缺口·拦截手段依赖】actionlint 在本地 Windows/Git Bash 未必可用**；不可用时降级为 yq/python YAML 解析 + 结构断言（T9-T11），工具可用性在 Construction 首日确认并回写。
- [ ] **【缺口·首次即生产】release.yml 仅 tag 触发、无 PR 预演，首个含 mac 腿的 tag 即真实发布**。开放问题（需人类裁决）：首次验证是否允许用 prerelease 标记/预发布 tag 走一遍 T14-T16，降低对正式 Release 列表的污染。
- [ ] runner 版本组合、产物命名（intent 开放问题未裁决）影响 T9/T10 断言字面值，对齐拍后校正；本策略测试点结构不受影响。
- [ ] 映射列已经对齐拍回填为 prd 实际 AC；AC1..AC5 全部有 T 点落点，无缺锚。
