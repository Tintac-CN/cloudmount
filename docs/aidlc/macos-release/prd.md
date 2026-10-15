# PRD: macos-release

> 产出者：pm　日期：2026-10-15　来源：intent.md（route=express，A1 裁决：产物范围 = daemon+cli 双架构，desktop 排除）

## 用户故事
| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| US1 | mac 用户 | 打 tag 后从 GitHub Release 下载与本机架构匹配（aarch64/x86_64）的 tar.gz，解压运行 `cloudmountd --serve` / CLI | mac 用户首次拥有官方产物，获得与 Linux 腿同级的 WebDAV 出口/CLI 能力 | 「一句话意图」「期望的结果」 |
| US2 | mac 用户 | 在 Release 页看到产物表 mac 行及"未签名 + 绕过 Gatekeeper"的可操作提示 | 首次打开不被 Gatekeeper 拦死，无证书约束下仍有可用引导 | 「约束」无 Apple 开发者证书条目 |
| US3 | 维护者 | 在本地 Windows（Git Bash）执行两道预检：双 darwin target `cargo check` + 打包脚本 dry-run | mac 编译面与 tar 布局本地可验证，线上试错成本降低、一次通过率提升 | 「问题与期望」本地缺预检手段 + 「期望的结果」 |
| US4 | 维护者 | CI 与本地 dry-run 调用同一份打包脚本 `scripts/package-macos.sh` | tar 布局单一事实来源，本地验证结果对 CI 有效，不会两套逻辑漂移 | 「期望的结果」打包逻辑脚本化（本地可 dry-run）+「约束」Git Bash 兼容 |

## 验收标准（可测）
- **AC1**：Given 仓库推送 `v*` tag，When release.yml 流水线运行，Then 新增的 `binaries-macos` job 在 GitHub macos runner 上对 workspace（daemon+cli）完成 `--release` 编译，产出 aarch64-apple-darwin 与 x86_64-apple-darwin 两个 tar.gz，且两者均出现在对应 GitHub Release 的资产列表中。（测试锚：T9/T14/T15）
- **AC2**：Given `scripts/package-macos.sh` 已参数化产物目录，When 本地以 dry-run 模式（占位二进制）执行与 CI 以真实编译产物执行，Then 两者生成的 tar.gz 内部布局一致——同一目录结构，均含 `cloudmountd`、`cloudmount`、`README`、`docs/edge.md`，不含 `packaging/systemd/*`（mac 无 systemd，见语义对齐）；且 workflow 中 mac 腿只调用该脚本、不内联复制的打包逻辑。（测试锚：T1/T2/T3/T4/T5/T10）
- **AC3**：Given 维护者本地 Windows + Git Bash + rustup，When 依次执行 ① `rustup target add aarch64-apple-darwin x86_64-apple-darwin` 后对根 workspace 各跑 `cargo check --target aarch64-apple-darwin` 与 `cargo check --target x86_64-apple-darwin`（check 不链接），② 打包脚本 dry-run，Then ① 两个 target 均 exit 0，② dry-run exit 0 且产物目录生成 tar.gz 并通过 AC2 同款布局断言；两道预检的命令与判据写入仓库内说明（脚本头注释或 README 小节）。（测试锚：T6/T7/T8）
- **AC4**：Given tag 触发的 release job 运行，When Release body（产物表）生成，Then 新增 aarch64 与 x86_64 两个 darwin 条目，且表格内或紧邻位置含未签名说明与可操作绕过指引（`xattr -cr <解压后的二进制>` 或右键→打开），表述风格与现有产物表一致。（测试锚：T12/T16）
- **AC5**：Given release.yml 修改合入，When 完整 release 流水线被触发（或对 workflow 做静态校验），Then `binaries-linux`、`binaries-windows`、`docker`+`merge` 各 job 行为与改动前一致（成功状态与资产集合不变），`release` job 的 needs 在原依赖上仅追加 macos 腿、不删除既有依赖；mac 腿失败时 release job 不发布（既有 fail-fast 语义不被削弱）。（测试锚：T11/T13/T14）

## 非目标（明确排除）
- desktop（apps/desktop，egui）.app 打包与 mac 产物纳入 —— A1 裁决明确排除（独立 cargo 项目，根 exclude）
- mac 本地挂载后端 —— mac 定位同 Linux 腿 = WebDAV 出口/CLI（`cloudmountd --serve`），不做 mac 文件系统挂载
- 代码签名与公证 —— 无 Apple 开发者证书，仅以文档提示（AC4）承载 Gatekeeper 引导
- 本地 Windows 产出真实可链接 mac 二进制 —— 不引入 osxcross、不使用 nektos/act 模拟 macos runner（Apple SDK/链接器/codesigning 三重依赖，无法本地模拟，见 decisions.md A1）
- 对既有 linux/windows/docker 腿的任何行为改动（AC5 仅约束零回归）
- homebrew formula、pkg installer、Sparkle 等其他 mac 分发形态
- `docs/edge.md` mac 部署小节 —— 是否补为开放问题 Q3，未裁决前不写入范围

## 语义对齐（express 内联小节）
关键动词 → 物理语义映射：
- **"GitHub mac runner 打包"** = release.yml 新增 `binaries-macos` job：matrix 覆盖 aarch64-apple-darwin 与 x86_64-apple-darwin 两个 target；在根 workspace 执行 `cargo build --release --target <target>`（desktop 因根 Cargo.toml exclude 天然不在构建面，与 A1 一致）；产物经 `scripts/package-macos.sh` 打成 tar.gz 后以 workflow artifact 上传。
- **"附于 Release"** = `release` job 的 needs 追加 macos 腿，softprops/action-gh-release 将两个 tar.gz 加入 assets，同时产物表补 mac 行与未签名提示（AC1+AC4）。mac 腿定位对齐 Linux 腿：daemon `--serve` WebDAV 出口 / CLI，无挂载语义。
- **"本地模拟"** = 仅两道预检，且边界明确：① `cargo check --target <darwin target>`（不链接，验证 cfg/API 编译面，workspace 在 ci.yml macos-latest 已绿为前提）；② 打包脚本 dry-run（占位二进制走通 tar 布局）。本地无法验证真实链接、签名、Gatekeeper 行为——该部分只能由 CI 真跑验证（AC1 承载）。

错误处理与边界：
- `scripts/package-macos.sh` 头部 `set -euo pipefail`：脚本任一步失败即整体非零退出 → CI job 失败 → release job 因 needs 不发布，不得出现部分成功。
- 打 tar 前强制校验产物目录同时存在 `cloudmountd` 与 `cloudmount` 两个可执行文件（`-x` 判定），缺任一立即失败退出，不得产出/上传 tar.gz——半成品不得成为 Release 资产。
- dry-run 仅限本地：以占位文件替代二进制，产物写入独立 dry-run 目录（与 CI 上传路径隔离），占位产物不得进入任何上传路径；CI 与本地的唯一差异 = 二进制来源与产物目录，布局逻辑走同一脚本同一代码路径。
- mac tar.gz 内容 = `cloudmountd` + `cloudmount` + `README` + `docs/edge.md`（即 Linux 腿去掉 `packaging/systemd/*` 的子集；systemd 单元在 mac 无语义）。若 gate1 另有裁决，改动面仅限脚本文件清单一行。
- 未签名产物：不在构建/上传链路做任何签名处理；引导信息只落在 Release notes（AC4）。

## 开放问题（需人类裁决）
- [ ] **Q1 runner 版本组合**：建议默认 = `macos-latest`（arm64 → aarch64-apple-darwin）+ `macos-13`（x64 → x86_64-apple-darwin）双 matrix entry。风险提示：macos-13 属 GitHub 官方退役时间表内镜像，2026-10 时点需核查可用性；若已退役，备选 = 单 `macos-latest` runner 跑双 target（x86_64-apple-darwin 交叉编译，链接步需 CI 首跑验证）。→ gate1 裁决（溯源：intent.md 开放问题第 1 项）
- [ ] **Q2 产物命名**：建议默认 = `cloudmount-darwin-aarch64.tar.gz` / `cloudmount-darwin-x86_64.tar.gz`（对齐 linux 腿 `<名>-<os>-<arch>` 命名法，intent.md 预填方案）。（溯源：intent.md 开放问题第 2 项）
- [ ] **Q3 `docs/edge.md` 是否补 mac 部署小节**：建议默认 = 本期补最小小节（约 10 行：下载解压、`xattr -cr`/右键打开、`cloudmountd --serve` 启动与 WebDAV 出口验证）——理由：tar 内含 edge.md，mac 用户读到的现状内容是 systemd 部署（不适用）；若裁决不补，则 edge.md 按现状原样入包、mac 引导仅落在 Release notes（AC4 已覆盖），此小节转入非目标。（溯源：intent.md 开放问题第 3 项）
