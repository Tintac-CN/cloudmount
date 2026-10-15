# Intent: installer-packages

> 创建：2026-10-15　状态：inception

## 一句话意图
windows需要提供msi安装包，mac也需要。请继续推进自动化发布和打包工作，tag即发布

## 问题与期望
- **要解决的问题**：release 产物面只有 tar/zip（用户需手动解压、自配 PATH）；缺 Windows MSI 与 mac 安装包形态；发布语义需收敛为「tag 即发布」（直接正式）。
- **期望的结果**：push `v*` tag 后 release.yml 自动产出 `cloudmount-windows-x64.msi` 与 mac 安装包（`cloudmount-darwin-<arch>.pkg`，形态 gate1 裁决）并附于正式 GitHub Release；安装包只装 `cloudmountd`+`cloudmount`（daemon+cli 范围裁决）；结构断言守护新增 job。

## 受影响系统/用户
- 改动面：`.github/workflows/release.yml`（安装包 job + 产物表）、安装器定义/脚本（WiX `.wxs` 或 cargo-wix 配置；mac pkgbuild 脚本）、`scripts/`（断言脚本）、Release notes
- 用户：Windows/mac 安装用户（双击/命令行安装，免手动解压配 PATH）

## 约束
- 无代码签名证书：MSI 无 Authenticode（SmartScreen 提示）、mac 包无 Developer ID 签名/公证（Gatekeeper 提示）——文档承载引导
- 产物范围 = daemon+cli（A1 裁决）：SCM 服务注册留用户命令 `cloudmountd service install`，不入安装器；desktop 不入本期
- mac 安装形态默认 `.pkg`（pkgbuild/productbuild 原生工具零新依赖；`.dmg` 备选——gate1 裁决）
- Windows 工具链：cargo-wix（WiX 封装，Rust 生态标准）或直接 WiX——选型 gate1/Construction 裁决，注意供应链（零新 crate 依赖优先，工具链仅 CI 侧）
- 「tag 即发布」= push tag 直接触发全流水线并发布**正式** Release（A1 裁决：覆盖上 intent「首 tag prerelease 预演」运维裁决）
- 既有产物（linux tar/windows zip/mac tar）与 job 零删改（同 AC5 口径守护）
- 上 intent 交付的断言脚本（release-yaml.test.sh 等）已 TDD 锁定——新断言落独立文件（上 intent retro 学习）

## 开放问题（需人类裁决）
- [ ] mac 安装形态：`.pkg`（推荐，CLI 工具惯例）还是 `.dmg`？
- [ ] Windows 工具链：cargo-wix 还是直接 WiX Toolset（CI 装 wix）？
- [ ] MSI 是否写 PATH 环境变量（推荐：写，免用户手动配）？
- [ ] 安装位置：`Program Files\CloudMount`（Windows）/ `/usr/local/bin`（mac，推荐）？
