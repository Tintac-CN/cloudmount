# PRD: macos-release

> 产出者：pm　日期：2026-10-15　来源：intent.md（route=express，A1 裁决：产物范围 = daemon+cli 双架构，desktop 排除；2026-10-15 gate1 打回修订：按全部推荐，B1/M1/M3 落实，Q1-Q3 回填）

## 用户故事
| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| US1 | mac 用户 | 打 tag 后从 GitHub Release 下载与本机架构匹配（aarch64/x86_64）的 tar.gz，解压运行 `cloudmountd --serve` / CLI | mac 用户首次拥有官方产物，获得与 Linux 腿同级的 WebDAV 出口/CLI 能力 | 「一句话意图」「期望的结果」 |
| US2 | mac 用户 | 在 Release 页看到产物表 mac 行及签名状态 + 绕过 Gatekeeper 的可操作提示 | 首次打开不被 Gatekeeper 拦死，无证书约束下仍有可用引导 | 「约束」无 Apple 开发者证书条目 |
| US3 | 维护者 | 一键触发 CI 编译面预检（workflow_dispatch mac check job）+ 本地 Git Bash 打包脚本 dry-run | mac 编译面与 tar 布局可验证，线上试错成本降低、一次通过率提升 | 「问题与期望」本地缺预检手段 + 「期望的结果」 |
| US4 | 维护者 | CI 与本地 dry-run 调用同一份打包脚本 `scripts/package-macos.sh` | tar 布局单一事实来源，本地验证结果对 CI 有效，不会两套逻辑漂移 | 「期望的结果」打包逻辑脚本化（本地可 dry-run）+「约束」Git Bash 兼容 |

## 验收标准（可测）
- **AC1**：Given 仓库推送 `v*` tag，When release.yml 流水线运行，Then 新增的 `binaries-macos` job 在 GitHub macos runner 上对 workspace（daemon+cli）完成 `--release` 编译，产出 aarch64-apple-darwin 与 x86_64-apple-darwin 两个 tar.gz，且两者均出现在对应 GitHub Release 的资产列表中。（测试锚：T9/T10/T14/T15/T17）
- **AC2**：Given `scripts/package-macos.sh` 已参数化产物目录，When 本地以 dry-run 模式（占位二进制）执行与 CI 以真实编译产物执行，Then 两者生成的 tar.gz 内部布局一致——同一目录结构，均含**根级** `cloudmountd`、`cloudmount`、`README.md`、`edge.md`（edge.md 源自仓库 `docs/edge.md`；与 Linux 腿扁平布局同构，无 `docs/` 前缀），不含 `packaging/systemd/*`（mac 无 systemd，见语义对齐）；且 workflow 中 mac 腿只调用该脚本、不内联复制的打包逻辑。（测试锚：T1/T2/T3/T4/T5/T10）
- **AC3**（gate1 B1：workspace 依赖 bundled C 的 crate（libsqlite3-sys/ring）build scripts，本地 Windows 无 darwin C 工具链，本地双 target check 不可行，编译面预检上移 CI）：Given 仓库含 workflow_dispatch 触发的 mac runner check job（runner=`macos-15`，rustup 安装 aarch64-apple-darwin 与 x86_64-apple-darwin 两个 target），When 维护者手动触发该 job，Then job 对双 target 各执行 `cargo check --target <t>`（check 不链接）且整体绿（exit 0）；Given 维护者本地 Windows + Git Bash，When 执行打包脚本 dry-run，Then exit 0 且产物目录生成 tar.gz 并通过 AC2 同款布局断言；CI check job 与本地 dry-run 的预检命令与判据写入仓库内说明（脚本头注释或 README 小节）。（测试锚：T6/T7/T8）
- **AC4**：Given tag 触发的 release job 运行，When Release body（产物表）生成，Then 新增 aarch64 与 x86_64 两个 darwin 条目，且表格内或紧邻位置注明签名状态——无 Developer ID 签名/公证（arm64 产物为链接器 ad-hoc 签名，Apple Silicon 运行所需）——并给可操作绕过指引（`xattr -cr <解压后的二进制>` 或右键→打开），表述风格与现有产物表一致。（测试锚：T12/T16）
- **AC5**：Given release.yml 修改合入，When 完整 release 流水线被触发（或对 workflow 做静态校验），Then `binaries-linux`、`binaries-windows`、`docker`+`merge` 各 job 行为与改动前一致（成功状态与资产集合不变），`release` job 在 needs 门控语义下仅追加 macos job、不删除既有依赖；mac 双 target 在同一 `macos-15` job 内以顺序步产出（固定单 runner——Q1 裁决，不采用 matrix）；mac job 失败时 release job 不发布（既有 needs 门控语义不被削弱）。（测试锚：T11/T13/T14）

## 非目标（明确排除）
- desktop（apps/desktop，egui）.app 打包与 mac 产物纳入 —— A1 裁决明确排除（独立 cargo 项目，根 exclude）
- mac 本地挂载后端 —— mac 定位同 Linux 腿 = WebDAV 出口/CLI（`cloudmountd --serve`），不做 mac 文件系统挂载
- 代码签名与公证 —— 无 Apple 开发者证书（无 Developer ID 签名/公证），仅以文档提示（AC4）承载 Gatekeeper 引导
- 本地 Windows 产出真实可链接 mac 二进制 —— 不引入 osxcross、不使用 nektos/act 模拟 macos runner（Apple SDK/链接器/codesigning 三重依赖，无法本地模拟，见 decisions.md A1；B1 进一步明确本地连 `cargo check` 双 target 亦不可行，编译面预检由 CI 承载）
- 对既有 linux/windows/docker 腿的任何行为改动（AC5 仅约束零回归）
- homebrew formula、pkg installer、Sparkle 等其他 mac 分发形态
- tag 规范改造 —— 沿用既有 `v*` 约定（异形 tag 不触发属既有行为，不在本期范围）
- README 安装表 mac 行 —— 不在本期（mac 引导由 edge.md mac 小节承载，gate1 Q3 裁决）

## 语义对齐（express 内联小节）
关键动词 → 物理语义映射：
- **"GitHub mac runner 打包"** = release.yml 新增 `binaries-macos` job：pin `macos-15`（arm64）单 runner 覆盖双 target——aarch64-apple-darwin 原生编译 + x86_64-apple-darwin 经 rustup target 交叉编译（gate1 Q1）；构建面与 linux 腿同构：`cargo build --release --target <t> -p cloudmount-daemon -p cloudmount-cli`（desktop 因根 Cargo.toml exclude 天然不在构建面，与 A1 一致）；产物命名 `cloudmount-darwin-<arch>.tar.gz` 短 arch 形式（gate1 Q2，与 linux 腿 target-triple 命名不同构，为可读性取舍）；mac 腿启用 Swatinem/rust-cache（随 windows 腿先例；linux 腿不用 cache 的 cross 容器 glibc 理由不适用）；产物经 `scripts/package-macos.sh` 打成 tar.gz 后以 workflow artifact 上传。
- **"附于 Release"** = `release` job 在 needs 门控语义下追加 macos job，softprops/action-gh-release 将两个 tar.gz 加入 assets，同时产物表补 mac 行与签名状态/Gatekeeper 提示（AC1+AC4）。mac 腿定位对齐 Linux 腿：daemon `--serve` WebDAV 出口 / CLI，无挂载语义。
- **"本地模拟"** = 本地仅 dry-run 一道（占位二进制走通 tar 布局）；编译面预检已上移 CI：workflow_dispatch 触发 mac runner check job（`macos-15` 双 target `cargo check`，不链接）——gate1 B1：本地 Windows 无 darwin C 工具链，bundled C build scripts 不可行。本地无法验证真实链接、签名、Gatekeeper 行为——该部分只能由 CI 真跑验证（AC1 承载）。

错误处理与边界：
- `scripts/package-macos.sh` 头部 `set -euo pipefail`：脚本任一步失败即整体非零退出 → CI job 失败 → release job 因 needs 门控不发布，不得出现部分成功。
- 打 tar 前强制校验产物目录存在根级 `cloudmountd` 与 `cloudmount` 两个可执行文件（`-x` 判定）及 `README.md`、`edge.md` 两个文档；可执行缺失或**文档缺失均为硬失败**（立即非零退出，不得产出/上传 tar.gz）——文档硬校验与 linux 腿的静默容错不同，因 AC2 已将二者列为必有内容（qa 缺文档负路径落点）。
- dry-run 仅限本地：以占位文件替代二进制，产物写入独立 dry-run 目录（与 CI 上传路径隔离），占位产物不得进入任何上传路径；CI 与本地的唯一差异 = 二进制来源与产物目录，布局逻辑走同一脚本同一代码路径。
- mac tar.gz 内容 = **根级** `cloudmountd` + `cloudmount` + `README.md` + `edge.md`（edge.md 源自仓库 `docs/edge.md`；与 Linux 腿扁平布局同构、无 `docs/` 前缀；即 Linux 腿去掉 `packaging/systemd/*` 的子集——systemd 单元在 mac 无语义）。
- 签名状态：无 Developer ID 签名/公证（arm64 产物有链接器 ad-hoc 签名，Apple Silicon 运行所需）；不在构建/上传链路做额外签名处理；引导信息只落在 Release notes（AC4）。

## 已裁决（2026-10-15 gate1，decisions.md 留痕）
- [x] **Q1 runner 版本组合**：裁决 = pin `macos-15`（arm64）**单 runner 双 target**——aarch64-apple-darwin 原生编译，x86_64-apple-darwin 经 rustup target 交叉编译；不采用 macos-latest + macos-13 双 entry 方案。（溯源：intent.md 开放问题第 1 项）
- [x] **Q2 产物命名**：裁决 = `cloudmount-darwin-<arch>.tar.gz` 短 arch 形式（`cloudmount-darwin-aarch64.tar.gz` / `cloudmount-darwin-x86_64.tar.gz`）；如实注明：与 linux 腿的 target-triple 命名**不同构**，为可读性取舍。（溯源：intent.md 开放问题第 2 项）
- [x] **Q3 `docs/edge.md` mac 部署小节**：裁决 = 补最小 mac 小节（约 10 行：下载解压、`xattr -cr`/右键打开、`cloudmountd --serve` 启动与 WebDAV 出口验证）；intent.md 改动面已由编排者扩入 `docs/edge.md`。（溯源：intent.md 开放问题第 3 项）
