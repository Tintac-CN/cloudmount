# CloudMount Windows 存储内核（自研 minifilter · 原生 Rust）

> 状态：**KM-0/KM-1 已合入；KM-2 只读视图已起步**（分支 `feat/cmflt-km2-readonly`，2026-10-27）。
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
├── cmflt-core/                # #![no_std] 纯逻辑：状态机 / 协议 / 范围记账 / 握手
├── cmflt-flt/                 # 手写 FltMgr ABI + safe 生命周期/端口/自旋锁封装
├── cmflt-driver/              # #![no_std] cdylib：DriverEntry / 端口回调 / unload
├── cmflt-sim/                 # std 用户态仿真器与属性测试（无 WDK 可跑）
├── cmflt-client/              # std 用户态握手冒烟客户端（FltLib）
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

## KM-1 已交付

- **通信端口**：`\CloudMountCmfltPort`，`FltCreateCommunicationPort` +
  `FltBuildDefaultSecurityDescriptor(FLT_PORT_ALL_ACCESS)`（系统/管理员可连）；
  connect/disconnect/message 三个回调；unload 按 client → server → unregister
  逆序关闭。
- **握手**：用户态 `Hello{proto,caps,engine_version}` → 内核 `HelloAck{status,
  proto,caps,kernel_version}`；`Health` 心跳仅在握手成功后接受；版本不匹配
  返回 `STATUS_REVISION_MISMATCH`，未握手 Health 返回 `STATUS_INVALID_PARAMETER`。
- **并发**：`SpinLock<EngineState>` 保护连接状态；协议解析与 ack 编码在锁内完成，
  临界区不含阻塞/分配。
- **版本/健康字段用原子标量 + 两个 `u64` 版本字**：避免 LLVM 把 16 字节
  拷贝/清零降成 SSE，驱动镜像继续通过 `xmm/ymm/zmm` 零命中门禁。
- **用户态冒烟工具**：`cmflt-client.exe` 连接端口、发 Hello/Health、
  校验 HelloAck。
- 宿主测试 52 项（core 35 + flt 5 + sim 5 + 集成 7）。

## KM-2 已起步（只读视图）

- **仅 NTFS 附着**：`InstanceSetupCallback` 对 `FLT_FSTYPE_NTFS` 返回
  `STATUS_SUCCESS`，其余返回 `STATUS_FLT_DO_NOT_ATTACH`。
- **CREATE 回调已接线**：`IRP_MJ_CREATE` pre-op 从真实 `FLT_CALLBACK_DATA` →
  `FLT_IO_PARAMETER_BLOCK` → `FLT_PARAMETERS.Create` 读取 `DesiredAccess` 和
  CreateOptions（低 24 位），按策略分类后仍 `FLT_PREOP_SUCCESS_NO_CALLBACK` 直通；
  create 路径不做任何数据 I/O。分类计数写入标量原子计数器（metadata/directory/
  data/unknown），为后续扫描抑制遥测留口。
- **create 分类策略**：`cmflt-core::policy` 把 `DesiredAccess/CreateOptions`
  分为 `MetadataOnly` / `DirectoryEnumeration` / `DataAccess`，宿主测试覆盖
  数据位、属性-only、目录枚举、reparse-point open；`should_hydrate_on_create`
  恒为 false。
- **占位符元数据模型**：`cmflt-core::placeholder` 定义非 Microsoft reparse tag、
  开发 GUID、`RECALL_ON_DATA_ACCESS|OFFLINE` 属性、`PlaceholderMeta` 32 字节
  payload 与状态不变量（Placeholder/Partial/Full/Pinned）；`Placeholder` 组合
  `RangeSet`，提供 `needs_recall`、`mark_hydrated`、`pin/unpin`、`dehydrate`
  的 partial→full→placeholder 转移，宿主测试覆盖边界与 OOB。新增固定 56 字节
  `encode_blob`/`reparse_blob`，可直接用于后续 `FSCTL_SET_REPARSE_POINT` 或
  `FltTagFileEx`。
- **扫描抑制配额**：`cmflt-core::scan::ScanQuota` 提供每会话请求数/字节数上限、
  原子式 acquire/release/reset 与 denied 计数，宿主测试覆盖请求/字节耗尽与
  释放下溢。
- **尚未完成**：实际 `FltTagFile`/属性写入、目录枚举投影、配额接入水合路径与
  真机 L2 场景；这些需要 testsigning/VM 与引擎侧配合。

## 未完成（需要测试机）

- **测试签名加载/卸载循环**：本机 `testsigning` 关闭；启用需
  `bcdedit /set testsigning on` + 重启，或在 VM/专用测试机执行。
- Driver Verifier、File System Filter Verification、200 次加载卸载循环（G1）。
- 真实引擎连接 `cmflt-client`（需 driver 已加载）；上下文与 operation 回调（KM-2）。

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

# 用户态握手客户端
cargo build -p cmflt-client
# 产物：drivers/win/target/debug/cmflt-client.exe
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

# 4. 握手/心跳冒烟（需要管理员权限以连接端口）
.\target\debug\cmflt-client.exe
# 期望：handshake ok / health ok

fltmc unload cloudmount
```

生产使用前必须申请正式 altitude 并过 HLK/WHCP；attestation 签名仅限测试用途。

## 验证记录（2026-10-28，KM-2 续）

```text
cargo test --workspace            76 passed / 0 failed
cargo clippy --workspace          -D warnings OK
cargo build -p cmflt-driver       OK（release）
cargo build -p cmflt-client       OK
cloudmount.sys                    6,144 bytes
entry point                       DriverEntry
subsystem                         Native (1)
imports                           FLTMGR.SYS + NTOSKRNL.EXE
FP/SSE/AVX in disassembly         none
writable+executable sections      none
cmflt-client（driver 未加载）      0x80070002（预期：端口不存在）
```

详细证据与复现命令见本文件"构建与测试"节；加载/卸载与真实握手仍待
testsigning 或 VM 执行。

## 里程碑

| 里程碑 | 内容 | 状态 |
|---|---|---|
| KM-0 | 构建链 + 空 filter 生命周期 + 宿主仿真/PE 门禁 | **已合入（PR #1）** |
| KM-1 | 通信端口 + connect/disconnect/message + Hello/Health/HelloAck + 版本协商 | **已合入（PR #2）** |
| KM-2 | 占位符创建 + 目录投影 + 扫描抑制 | **续：CREATE 已接线 + 占位符元数据模型；实际 tag 写入/投影待续** |
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
