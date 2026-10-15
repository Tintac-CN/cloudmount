# Gate#1 AI 互审（2026-10-15）

> 评审对象：express 两件（prd.md @ 624e1a6 + test-strategy.md @ 624e1a6）对照 intent.md/decisions.md 与 brownfield 事实（release.yml/ci.yml/Cargo.toml/Cargo.lock）。
> 评审者：reviewer-pm（1/1/6）、reviewer-architect（0/1/6）、reviewer-qa（0/4/6）、reviewer-dev（1/2/4）——全部只读。
> 编排者去重合并：**1 BLOCKER / 6 MAJOR / 16 MINOR**。

## BLOCKER

- **B1｜本地双 darwin target `cargo check` 预检（AC3①/T6/T7，P0）在声明的环境（Windows+Git Bash+rustup）不可达成**［pm+dev 独立同证］
  - 证据链：`cargo check` 不链接但**仍为目标编译并执行依赖的 build scripts**——rusqlite bundled（Cargo.toml:38 → libsqlite3-sys，经 cc 为 target 编译 sqlite3.c；引入链 daemon→cloudmount-sync→rusqlite，无 feature 门控）与 ring 0.17.14（rustls 链）build.rs 同样编译 C/asm；Windows 宿主无 darwin C 交叉工具链/Apple SDK（PRD 非目标明确不引入 osxcross）→ cc fatal → exit 0 无通路。
  - prd.md 语义对齐小节"check 不链接，验证 cfg/API 编译面"的论证遗漏 build-script C 编译一环；T6/T7 按现判据永远红，AC3 不可满足。
  - 修复方向（供修订，裁决见人审）：①预检上移 CI——workflow_dispatch 触发的 mac runner check job（无需 tag 即可验证编译面，服务"线上一次通过率"），本地预检降为 dry-run 一道；②或如实改写 AC3——本地仅 dry-run+结构校验，check 判据撤销并披露原因。

## MAJOR

- **M1｜AC2 tar 布局/文件名字面三方矛盾**［arch+dev+pm 同根］：linux 腿实际把文件拍平进 dist/ 根再打 tar（release.yml:36-38）——tar 内为根级 `./README.md`、`./edge.md`（无 `docs/` 前缀）；AC2 写 "`README`"（错文件名）+ "`docs/edge.md`"（错路径层级），照字面实现即违反自身"与 Linux 腿对齐"声明，T4 断言无处落笔。修订：统一为"tar 内根级 `README.md` 与 `edge.md`（源文件取自仓库 docs/edge.md，与 Linux 腿同构）"。
- **M2｜Q1 裁决输入事实过时（2026-10-15 实测 actions/runner-images 官方 README）**［dev］：`macos-13` 已完全退役（可用与 deprecated 列表均无）；可用 = `macos-15`（arm64）、`macos-15-intel`/`macos-15-large`（x64）；macos-14 已标 deprecated；`-latest` 处于 14→15 迁移期官方明言行为可能变。按现建议默认（macos-latest+macos-13）照抄得到永不调度的 job。建议 pin `macos-15`（arm64），x86_64 用单 runner 双 target（Apple clang 原生交叉链接 arm→x86_64，技术面 arch 复核无硬伤）或 `macos-15-intel` 双 runner。
- **M3｜AC 锚双向不一致 + decisions 核对记录与工件不符**［qa］：prd AC1 锚位=T9/T14/T15，但 test-strategy T10 映射 "AC1/AC2"、T17 映射 AC1——反向多出两锚；decisions.md 对齐拍核对行"AC1=T9/T14/T15…无缺锚"与工件实际不符。修订：prd AC1 括注补全、decisions 以新条目订正（append-only）。
- **M4｜AC2 布局元素全测试集无显式断言**［qa］：AC2 要求含 README/edge.md、不含 packaging/systemd/*，但 T4 仅断言"daemon+cli 两二进制"、SOP 第 2 条同样只写两二进制——漏 README/edge.md 或误带 systemd 无任何测试点失败。修订：T4 显式枚举完整期望清单+systemd 排除（SOP 同步）。
- **M5｜T10 "glob 落空静默零文件"无可判定断言手段**［qa］：静态断言未写明判据。现成手段：断言 mac upload 步含 `if-no-files-found: error`（同 linux 腿 release.yml:44）+ glob 字面与脚本输出命名一致。
- **M6｜T11 断言语义超载**［qa］："needs 含 binaries-macos → 失败不发布"仅在 release job 无 `if:`/status 覆盖时成立；当前 release.yml:72-78 确无 `if:`，但 T11 未断言"不得引入 if 覆盖"——Construction 若加 `if: always()` 则 T11 过而 AC5 fail-fast 语义已破。修订：补"release job 不带 if:/status 函数覆盖"静态断言。

## MINOR（16）

1. rust-cache 取舍未表态（arch+dev 同根）：mac 腿原生编译与 windows 腿同类可安全启用；建议语义对齐小节一句话表态，避免 Construction 自由发挥。
2. "fail-fast"术语撞名（arch）：AC5 实指 needs 门控，与 Actions matrix `strategy.fail-fast` 是两机制；建议 AC5 改"needs 门控语义"措辞并注明 mac matrix 沿用 `fail-fast: false`。
3. Q2 命名"对齐 linux 腿命名法"表述失真（arch+dev+pm 同根）：linux 实际用完整 target triple（`cloudmount-linux-x86_64-unknown-linux-musl.tar.gz`），Q2 建议是短 arch 简写；T10 行本体未标 blocked-on-Q2。
4. 构建面建议与 linux 腿同构用 `-p cloudmount-daemon -p cloudmount-cli`（arch）：语义等价但缩小构建面、与产物表一致。
5. "未签名"表述精确化（arch）：arm64 产物会被链接器自动 ad-hoc 签名（Apple Silicon 硬性要求），实义是"无 Developer ID 签名/公证"；措辞精确化以免 T16 预期误写。
6. edge.md/README 缺失容错语义未定（dev）：linux 腿对文档 `2>/dev/null || true` 静默容错，AC2 却列为必有；脚本应明确硬校验或容错，且无"缺文档"负路径测试点。
7. AC3 硬交付物（预检说明入仓）挂 T8 整体 P2（qa）：建议文档存在性拆出提 P1。
8. T16 未标"真 mac 机"前提（qa）：Gatekeeper 拦截/解除只能真 mac 观察，前提只标在 T17。
9. T14 未覆盖 AC5"资产集合不变"半句（qa）：建议补"既有 linux 双架构/windows zip 资产仍在 Release"。
10. SOP `file` 判据过刚（qa）：精确串随 file/libmagic 版本漂移，改按子串 "Mach-O"+"arm64"/"x86_64"；"体积合理"无阈值，给粗略区间。
11. T9 对 Q1 备选不成立（qa）：若落"单 macos-latest 双 target"，T9"matrix 恰含两个 entry"需语义级改写而非仅字面校正（与 M2 修订联动）。
12. dry-run 隔离目录断言缺失（qa）：prd"占位产物不得进入任何上传路径"无测试点，可并入 T1。
13. tag 规范边界未声明（pm）：非目标补一行"tag 规范沿用既有 v* 约定，异形 tag 不触发属既有行为"。
14. README 安装表无 mac 行（pm）：README 随 mac tar 分发但安装表仅 Linux/Windows/Docker；补行或记非目标，须明确。
15. Q3 与 intent 改动面一致性（pm）：intent.md 改动面不含 docs/edge.md，Q3 若裁决补小节需同步扩 intent 备注。
16. macos-15-intel 第三备选（pm）：Q1 核查时一并评估其 2026-10 可用性（dev 实测已确认可用）。

## 结论

**1 BLOCKER / 6 MAJOR / 16 MINOR。** 方案骨架（binaries-macos job+单一脚本+needs 追加）与 brownfield 同构性经 arch/dev 双向核实成立，需求覆盖完整、缺口披露诚实；但 B1（本地 check 预检技术不可行）与 M1（tar 布局字面矛盾）使 AC2/AC3 两条 AC 按现文不可验收，M2（macos-13 已退役）使 Q1 按现建议裁决即错。建议：**打回修订（phase 回 inception）**——点名 pm（B1 修复方向+AC2/AC3 字面+Q1-Q3 回填）、qa（T4/T10/T11 断言可判定化+锚位双向一致+M4-M6/MINOR 落点）修订，arch 语义对齐小节随 pm 顺带订正；对齐拍后 gate1 复位 pending 复审（重点核对 B1/M1/M2 消解）。
