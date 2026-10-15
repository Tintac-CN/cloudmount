# Intent: macos-release

> 创建：2026-10-15　状态：inception

## 一句话意图
mac环境在线上请使用GitHub的mac runner来实现打包与release。本地看看有模拟解决办法不

## 问题与期望
- **要解决的问题**：`.github/workflows/release.yml` 产物矩阵缺 macOS 腿（现有 linux 双架构 musl / windows x64 / GHCR docker 多架构），mac 用户无官方产物；且 mac 链接/产物只能在 macos runner 上真验证，本地（Windows）缺预检手段，线上试错成本高。
- **期望的结果**：打 tag `v*` 后 GitHub mac runner 自动产出 mac 双架构（aarch64 + x86_64）daemon+cli tar.gz 并附于 GitHub Release；打包逻辑脚本化（本地可 dry-run），编译面预检上移 CI（workflow_dispatch 触发的 mac runner 双 target `cargo check`——gate1 裁决：本地 check 因 bundled C build scripts 不可行）。

## 受影响系统/用户
- 改动面：`.github/workflows/release.yml`、`scripts/`（新增打包脚本）、Release notes 产物表、`docs/edge.md`（最小 mac 部署小节，gate1 Q3 裁决扩入）
- 用户：mac 用户（下载 tar.gz 使用 `cloudmountd --serve` / CLI）；维护者（本地预检、线上一次通过率）

## 约束
- 无 Apple 开发者证书：产物未签名，Release 需 Gatekeeper 提示（`xattr -cr` 或右键打开），不做公证
- mac 定位同 Linux 腿 = WebDAV 出口/CLI（无 mac 挂载后端；挂载为非目标）
- 打包脚本需 Git Bash 兼容（本地 dry-run 与 CI 同一脚本）
- workspace（daemon/cli）在 ci.yml `macos-latest` 腿已绿——mac 编译面健康为前提
- desktop 为独立 cargo 项目（根 Cargo.toml exclude），本期不入 mac 产物（A1 裁决）

## 开放问题（需人类裁决）
- [x] runner 版本组合 → **已裁决**（2026-10-15 gate1）：pin `macos-15`（arm64）单 runner 双 target，x86_64-apple-darwin 经 rustup target 交叉编译
- [x] 产物命名 → **已裁决**（2026-10-15 gate1）：`cloudmount-darwin-<arch>.tar.gz`（短 arch）
- [x] `docs/edge.md` 是否补 mac 部署小节 → **已裁决**（2026-10-15 gate1）：补最小 mac 小节（约 10 行）
