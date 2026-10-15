# Plan: macos-release

> 产出者：编排者（Sprint Planning）　日期：2026-10-15　前置：gate1 approved（@ 9ec8fd9）
>
> 依据：prd v2（AC1–AC5，gate1 修订轮后）、test-strategy v2（17 点，P0=10/P1=6/P2=1）、gate1 复审 R2-MAJOR 裁决（check job 载体=独立 workflow 文件 `.github/workflows/check-macos.yml`）与 8 MINOR 回填授权（decisions.md GATE-APPROVED gate1 2026-10-15）。
> TDD 铁律：每 unit 先提交失败测试（红）→ `/aidlc:lock-tests` 置锁 → 实现转绿。锁范围为失败测试文件并集（只增不减）。

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | mac 打包脚本（关键路径） | `scripts/package-macos.sh`：参数化产物目录/--dry-run（占位二进制、隔离目录）、`set -euo pipefail`、四项必含文件硬校验（cloudmountd/cloudmount 可执行+README.md/edge.md）、tar 根级扁平布局（对齐 linux 腿）、输出 `cloudmount-darwin-<arch>.tar.gz`；脚本头注释含本地预检说明 | `scripts/package-macos.sh`（新增）、`scripts/package-macos.test.sh`（新增，TDD） | low（新文件无引用方；release.yml 于 U2 才引用） | T1/T2/T3/T4/T5 | AC2：dry-run exit 0+布局四项不多不少（`./` 归一化）；缺任一必含文件/参数缺失→非零无残缺 tar；desktop/systemd 排除 |
| U2 | release.yml mac 腿 | binaries-macos job（runner=`macos-15` 单机、rustup 双 darwin target、`cargo build --release --target <t> -p cloudmount-daemon -p cloudmount-cli` 顺序步双 target **不采用 matrix**、Swatinem/rust-cache、调用 `scripts/package-macos.sh`、upload glob `cloudmount-darwin-*.tar.gz` + `if-no-files-found: error`）+ release job `needs` 仅追加（不带 `if:`/status 覆盖）+ 产物表 mac 行（无 Developer ID 签名/公证（arm64 ad-hoc）+ Gatekeeper 指引）；`on:` 保持仅 `tags: v*` | `.github/workflows/release.yml`、`scripts/release-yaml.test.sh`（新增，TDD） | med（发布主 workflow——tag 触发全流水线的唯一交付路径；改动语义=仅追加，AC5/T11 静态断言守护零删改） | T9/T10/T11/T12/T13 | AC1/AC5 静态面：结构断言全绿+actionlint 过（可用时） |
| U3 | mac check job 独立载体 | 新建 `.github/workflows/check-macos.yml`：`on: workflow_dispatch` 唯一触发；runner=`macos-15`；rustup 装 aarch64+x86_64-apple-darwin；`cargo check --target <t> -p cloudmount-daemon -p cloudmount-cli` 双步（顺序步） | `.github/workflows/check-macos.yml`（新增）、`scripts/release-yaml.test.sh`（追加 T6 断言） | low（新文件零既有引用；无自动触发零副作用） | T6 | AC3①：结构断言绿（dispatch/runner/双 target check 步骤） |
| U4 | edge.md mac 小节 + 回填收尾 | `docs/edge.md` 追加最小 mac 部署小节（约 10 行：下载解压→`xattr -cr`/右键打开→`cloudmountd --serve` 启动与 WebDAV 出口验证——Q3 裁决）；gate1 授权 8 MINOR 工件回填终检（prd/test-strategy 措辞逐项核对补漏）；终验组合全绿+基线比对 | `docs/edge.md`、`docs/aidlc/macos-release/{prd.md,test-strategy.md}`（回填） | low（文档追加小节，不改既有结构；release.yml linux 腿打包被动携带） | T8（说明文档存在性） | AC3③：预检说明入仓；Q3 mac 小节落盘；终验全绿+基线零回退 → phase 置 gate2 |

## 验证命令（F2 规范来源）

- **基线/回归命令**：`cargo test --workspace`（第 0 步记录 `state.testBaseline`；本 intent 零 Rust 改动——`.github/`+`scripts/`+`docs/` 面，终验比对预期零回退）
- **终验组合**：上命令 + `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`（四件套惯例，audit-p0p1-fixes retro 学习沉淀）+ `bash scripts/package-macos.test.sh` + `bash scripts/release-yaml.test.sh`（U1/U2 交付后加入）+ `actionlint`（可用时，覆盖 release.yml 与 check-macos.yml；不可用时降级为两脚本内 YAML 结构断言并记缺口）

## 执行顺序与并行泳道

- 泳道 1（脚本关键路径）：U1
- 泳道 2（workflow 串行）：U2 → U3（共用 `scripts/release-yaml.test.sh` 结构断言脚本，U3 在其上追加断言）
- 泳道 3（文档收尾）：U4（依赖 U1/U2 定稿）
- 波次：①U1 → ②U2 → ③U3 → ④U4 → ⑤终验（四件套+脚本测试+基线比对）→ gate2
- 影响面说明：全部 unit 无跨层引用方（U2 的 release.yml 为唯一存量文件，改动语义=仅追加 mac 腿与产物表行；无 high/unknown 项）。限流约束（上 intent 三次 1302）：默认串行派发，不并行扇出。

## TDD 锁记录

- 待置位：U1 红提交后锁 `scripts/package-macos.test.sh`；U2 红提交后并集锁 `scripts/release-yaml.test.sh`。契约错误走 `/aidlc:unlock-tests <理由>` 并留痕 decisions.md。

## gate1 授权回填清单（8 MINOR——U4 落点，decisions.md GATE-APPROVED gate1 2026-10-15）

①AC5 括注收紧"固定单 runner 顺序步（Q1），不采用 matrix"（U2 形态已按此定稿，prd/test-strategy 字面随 U4 回填）②T12/T16/SOP"未签名"→"无 Developer ID 签名/公证（arm64 ad-hoc）"③T4 补 tar `./` 归一化半句④AC5 句尾"fail-fast 语义"→"needs 门控语义"⑤SOP 补发布前置第 0 步"tag 前手动触发 dispatch check 并确认绿"⑥T15 同步四项清单⑦SOP 体积判据"超出 5–50 MB 带即人工复核"⑧T9 单 runner 约束断言（与①合并落点）——编排者按授权微调 prd/test-strategy 措辞。
