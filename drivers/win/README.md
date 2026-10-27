# CloudMount Windows 存储内核（自研 minifilter · 原生 Rust）

> 状态：**KM-0 原生 Rust 骨架已落地**（分支 `feat/cmflt-native-rust`，2026-10-27）。
> M2 仍用 CfAPI 发布产品；`cmflt` 是长期自研后端，通过
> `cloudmount-vfs::MountBackend` 平滑替换，不阻塞任何产品里程碑。
>
> 📄 完整方案：[RUST-MINIFILTER-DESIGN.md](RUST-MINIFILTER-DESIGN.md)
> —— 含 14 项稳定机制、S1–S6 门禁、KM-0…KM-7 里程碑与验证矩阵。

## 原生 Rust 边界（与 C/C++ 解耦）

本目录以"原生 RUST 目标"为硬约束：

- **驱动镜像 100% Rust**：`cmflt-driver` 是 `#![no_std]` `cdylib`，由 rustc 直接
  产出 PE，`/ENTRY:DriverEntry`，无 C 启动桩、无 C 运行时、无 C++ EH。
- **FltMgr ABI 手写**：`cmflt-flt/src/raw.rs` 按 WDK 文档声明 `FLT_REGISTRATION`
  与三个生命周期 API；x64 结构体大小 112 字节用 `const assert` 锁定，
  `FLT_REGISTRATION_VERSION = 0x0203`。**不使用 bindgen/libclang**。
- **无 C/C++ 源文件**：旧 `src/filter.c` 与 `include/cloudmount_proto.h` 已删除
  （git 历史可查）；内核/引擎协议由 `cmflt-core::protocol` 定义，双方都是 Rust。
- **唯一非 Rust 构建输入**是 MSVC 链接器与 WDK 的 `.lib` 导入库——这是 Windows
  内核 ABI 的平台边界，不是仓库内编译的 C/C++ 组件。
- 设计文档 D1 的"薄 C 壳回退路径"**不再作为仓库内维护组件**；若未来绑定层出现
  无法隔离的 UB，回退方案重新立项，见设计文档文首 v1.1 修订。

## 目录结构

```
drivers/win/
├── Cargo.toml                 # 独立 workspace（与主产品 workspace 隔离）
├── cmflt-core/                # #![no_std] 纯逻辑：状态机 / 协议 / 范围记账
├── cmflt-flt/                 # 手写 FltMgr ABI + 最小 safe 生命周期封装
├── cmflt-driver/              # #![no_std] cdylib：DriverEntry / unload / panic 策略
├── cmflt-sim/                 # std 用户态仿真器与属性测试（无 WDK 可跑）
├── scripts/                   # host 测试 / 构建 / PE 验证 / test-sign
├── cloudmount.inf             # HSM 组安装信息（Class=HSM / Group=FSFilter HSM）
└── RUST-MINIFILTER-DESIGN.md  # 完整设计与里程碑
```

## KM-0 已交付

- `DriverEntry` → `FltRegisterFilter` → `FltStartFiltering` 的完整生命周期；
- unload 回调中 `FltUnregisterFilter`，支持 `fltmc unload`；
- 空 filter 形态：无 operation/context/port 回调，先把构建链与卸载路径钉死；
- 自定义 panic handler：任何 Rust panic 走 `KeBugCheckEx(0x52504E43, file, line, col, 0)`；
- 宿主侧纯逻辑：水合状态机（含取消/超时/熔断/后备切换排空）、显式小端协议、
  固定容量 hydrated-range 记账；
- 41 项宿主测试 + 64 seeds × 20,000 步随机游走（约 128 万事件）不变量检查；
- PE 静态门禁：x64 / Native / `DriverEntry` / 仅导入 `FLTMGR.SYS` / 无 RWX 段 /
  反汇编无 `xmm|ymm|zmm`。

## KM-0 未完成（需要下一环境）

- **测试签名加载/卸载循环**：本机 `testsigning` 关闭；启用需
  `bcdedit /set testsigning on` + 重启，或在 VM/专用测试机执行。
- Driver Verifier、File System Filter Verification、200 次加载卸载循环（G1）。
- 真实通信端口、上下文、operation 回调（KM-1/KM-2）。

## 构建前置

| 组件 | 要求 | 本机状态（2026-10-27） |
|---|---|---|
| Rust | stable，`x86_64-pc-windows-msvc` | 1.99.0 ✅ |
| MSVC | VS2019 BuildTools（C++ + Windows SDK） | 16.11 ✅ |
| WDK | `Microsoft.WindowsWDK.10.0.19041`（kernel headers + km libs） | 10.1.19041.685 ✅ |
| 权限 | 构建不需要；安装/签名需要管理员 | 提权 shell ✅ |

WDK 可用 winget 安装：

```powershell
winget install --id Microsoft.WindowsWDK.10.0.19041 --exact --silent `
  --accept-package-agreements --accept-source-agreements
```

`cargo check -p cmflt-driver` 不需要 WDK；链接 `.sys` 需要。构建脚本会通过
`CMFLT_WDK_LIB` / `CMFLT_WDK_VERSION` 覆盖自动探测。

## 构建与测试

```powershell
cd drivers/win

# 宿主测试 + 驱动类型检查（无需 WDK/管理员）
powershell -ExecutionPolicy Bypass -File scripts/test-host.ps1

# 构建 cloudmount.sys 并执行 PE 静态门禁
powershell -ExecutionPolicy Bypass -File scripts/build-driver.ps1
# 产物：drivers/win/out/cloudmount.sys
```

Git Bash 可直接 `bash scripts/test-host.sh`。

## 安装与加载（测试机）

```powershell
# 1. test-sign（首次会创建自签证书并导入 Root/TrustedPublisher，需要管理员）
powershell -ExecutionPolicy Bypass -File scripts/sign-test.ps1

# 2. 开启测试签名并重启
bcdedit /set testsigning on
Restart-Computer

# 3. 安装 INF（DefaultInstall）并加载
InfDefaultInstall.exe .\cloudmount.inf
fltmc load cloudmount
fltmc filters          # 应看到 cloudmount，Altitude=189900，Group=FSFilter HSM
fltmc unload cloudmount
```

生产使用前必须申请正式 altitude 并过 HLK/WHCP；attestation 签名仅限测试用途。

## 验证记录（2026-10-27）

```text
cargo test --workspace            41 passed / 0 failed
cargo check -p cmflt-driver       OK
cargo build -p cmflt-driver       OK（release）
cloudmount.sys                    4,608 bytes
entry point                       DriverEntry
subsystem                         Native (1)
imports                           FLTMGR.SYS only
FP/SSE/AVX in disassembly         none
writable+executable sections      none
```

详细证据与复现命令见本文件"构建与测试"节；加载/卸载仍待 testsigning 或 VM 执行。

## 里程碑

| 里程碑 | 内容 | 状态 |
|---|---|---|
| KM-0 | 构建链 + 空 filter 生命周期 + 宿主仿真/PE 门禁 | **骨架完成，加载待验** |
| KM-1 | 扩展 FltMgr safe 层（端口/上下文/回调）+ 握手/版本协商 | 待开始 |
| KM-2 | 占位符创建 + 目录投影 + 扫描抑制 | 待开始 |
| KM-3 | 按需水合 + 取消安全 + 超时降级 + 后备切换 | 待开始 |
| KM-4 | mmap/section 钩子 + 写路径 + oplock 降级 | 待开始 |
| KM-5 | 崩溃一致对账 + ETW/黑匣子 + triage | 待开始 |
| KM-6 | Shell 集成（NSE/overlay/右键/缩略图） | 待开始 |
| KM-7 | HLK/WHCP + 安装器 + 灰度 | 待开始 |

## 参考（借鉴架构，不抄代码）

- WinFsp `src/sys/`（长 pending 的过期定时器清理、不可卸载取舍）
- Microsoft CfAPI 文档与 CloudMirror 示例（占位符状态机语义、progressive 随机 seek）
- ProjFS 文档（投影语义）
- WDK 样例：`cancelSafe`（Cbdq 正确用法）、`swapBuffers`、`SimRep`
- `apriorit/filesystem-kernel-filter-rs`（Rust minifilter 先例）
- WDK 10.0.19041 `fltkernel.h` / Microsoft Learn DDI 页（ABI 事实来源）
