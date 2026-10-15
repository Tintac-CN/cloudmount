# Decisions: macos-release

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-15 A1 意图接入确认（AskUserQuestion，逐字转写）：
- 意图原话：`/aidlc mac环境在线上请使用GitHub的mac runner来实现打包与release。本地看看有模拟解决办法不`（逐字）。
- 识别的意图一句话：release.yml 补 macOS 打包腿（GitHub mac runner）接入 Release；本地建立编译面+脚本 dry-run 预检。
- 拆分预览（5 项）：①release.yml 增 binaries-macos job（macos runner 编译 daemon+cli → tar.gz，形态对齐 Linux 腿）②打包逻辑抽成 scripts/package-macos.sh（参数化产物目录，本地可 dry-run）③release job 依赖扩 macos、Release notes 产物表补 mac 行（含未签名/Gatekeeper 提示）④本地预检：双 target cargo check + dry-run 说明⑤（可选）desktop .app 打包是否纳入本期。
- 本地模拟结论（编排者核查，已呈用户）：真实 mac 链接/产物无法在本地 Windows 模拟（Apple SDK/链接器/codesigning 三重依赖；osxcross 法律灰且配置重不建议；nektos/act 仅 Linux 容器模拟不了 macos runner）——本地预检两道 = `rustup target add` 后 `cargo check --target aarch64/x86_64-apple-darwin`（check 不链接，抓 cfg/API 编译面）+ 打包脚本参数化 dry-run（Git Bash 占位产物走通 tar 布局，CI 调同一脚本）。仓库事实：release.yml 现有 linux 双架构/windows/docker 三类腿缺 mac；workspace 在 ci.yml macos-latest 腿已绿（mac 编译面健康）；desktop 为独立 cargo 项目（egui 跨平台，根 exclude）。
- 问：「意图「release.yml 补 mac 打包腿 + 本地预检」——是否进入 AIDLC 生命周期？」
  答：「进入生命周期（推荐）」
- 问：「规模分级路线（写入 intent 后 gate1 批准前仅可 express→classic 单向升级）：」
  答：「express（推荐）」
- 问：「mac 产物范围（若进入生命周期，此项作 inception 澄清输入）：」
  答：「daemon+cli 双架构（推荐）」
- 裁决落点（编排者执行）：建 intent `macos-release`（route=express）；desktop .app 不在本期（产物范围=daemon+cli 双架构）；Inception 不派 architect、不产 architecture.md，语义对齐并入 prd.md 内联小节；门控回执/decisions 逐字留痕/TDD 锁/hook 保护与 classic 完全一致。
