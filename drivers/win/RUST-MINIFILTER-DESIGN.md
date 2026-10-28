# CloudMount Windows 存储内核 · Rust 重写完整方案

> 代号 `cmflt`（CloudMount Filter） ｜ 版本 v1.0 ｜ 2026-10-12
> 前置：[README.md](README.md)（C 骨架与路线）、[../../docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md)
>
> **本文回答一个问题**：如何用 Rust 从零实现一个云盘占位符 minifilter，
> 且**在稳定性上超越**现有实现（CfAPI 托管方案、WinFsp/Dokan 的 FSD、以及本文档早期
> 的 C 骨架）——而不只是把 C 翻译成 Rust。

---

> **v1.1 原生 Rust 实施修订（2026-10-27）**
>
> - 目标改为"原生 RUST"：驱动镜像不含任何 C/C++ 源文件、C 启动桩、bindgen/libclang
>   产物或 C++ 运行时。旧 `src/filter.c` 与 `include/cloudmount_proto.h` 已从工作树移除
>   （git 历史可查）；协议由 `cmflt-core::protocol` 的显式小端编码定义。
> - §3 D1 的"薄 C 壳回退路径"不再作为仓库内维护组件。若未来绑定层出现无法隔离的 UB，
>   回退方案重新立项评估，不把 C 组件放回主路径。
> - FltMgr ABI 手写于 `cmflt-flt/src/raw.rs`：`FLT_REGISTRATION_VERSION = 0x0203`，
>   x64 `FLT_REGISTRATION` 112 字节由 `const assert` 锁定；不使用 `cargo-wdk` 或 bindgen。
> - 工具链：stable Rust + VS2019 BuildTools + WDK 10.0.19041；`cargo check` 不需要 WDK，
>   链接 `.sys` 需要 WDK 的 km 导入库。
> - KM-0 已实际产出空 filter 镜像：Native 子系统、入口 `DriverEntry`、仅导入
>   `FLTMGR.SYS`、无 RWX 段、反汇编无 `xmm/ymm/zmm`；宿主侧 41 项测试通过。
> - 未完成：testsigning 加载/卸载循环与 Driver Verifier（本机 testsigning 关闭）。

> **v1.2 KM-1 实施（2026-10-27）**
>
> - 通信端口 `\CloudMountCmfltPort`：`FltCreateCommunicationPort` +
>   `FltBuildDefaultSecurityDescriptor(FLT_PORT_ALL_ACCESS)`（系统/管理员可连），
>   connect/disconnect/message 回调；unload 按 client → server → unregister 逆序关闭。
> - 握手：`Hello` → `HelloAck`（status/proto/caps/kernel_version）；`Health` 仅在
>   Hello 成功后接受；版本不匹配返回 `STATUS_REVISION_MISMATCH`。新增用户态
>   `cmflt-client` 冒烟工具。
> - 连接状态在 `SpinLock<EngineState>` 内；版本/健康字段用原子标量 + 两个 `u64`
>   版本字，避免 LLVM 生成 16 字节 SSE 拷贝/清零。
> - 实测：宿主 52 项测试通过；`cloudmount.sys` 6,144 字节，Native，导入
>   `FLTMGR.SYS` + `NTOSKRNL.EXE`，无 RWX 段，反汇编无 FP/SSE。
> - 未完成：testsigning 下 `fltmc load/unload`、`cmflt-client` 真机握手、G1。

> **v1.3 KM-2 起步（2026-10-27）**
>
> - `InstanceSetupCallback` 仅附着 NTFS（`FLT_FSTYPE_NTFS`），其余卷返回
>   `STATUS_FLT_DO_NOT_ATTACH`。
> - 注册 `IRP_MJ_CREATE` pre-op，并从真实 `FLT_CALLBACK_DATA` →
>   `FLT_IO_PARAMETER_BLOCK` → `FLT_PARAMETERS.Create` 读取
>   `DesiredAccess/CreateOptions`；分类后仍直通、无数据 I/O。所有结构偏移由
>   `const assert` 锁定。
> - `cmflt-core::policy` 对 `DesiredAccess/CreateOptions` 分类
>   `MetadataOnly/DirectoryEnumeration/DataAccess`，明确 create 路径永不水合；
>   分类计数进入标量原子计数器。
> - `cmflt-core::placeholder` 定义非 Microsoft reparse tag、开发 GUID、
>   `RECALL_ON_DATA_ACCESS|OFFLINE` 属性、32 字节 `PlaceholderMeta` 与
>   Placeholder/Partial/Full/Pinned 状态不变量；`Placeholder` 组合 `RangeSet`，
>   覆盖 `needs_recall` / `mark_hydrated` / `pin` / `dehydrate` 转移，并提供
>   固定 56 字节 reparse blob。
> - `cmflt-core::scan::ScanQuota` 提供每会话请求/字节配额与 denied 计数，
>   作为扫描抑制的纯逻辑基础。
> - 占位符创建路径：`cmflt-client create-placeholder` 建 sparse 文件并写
>   GUID reparse + `OFFLINE`；driver CREATE post-op 在
>   `FltDoCompletionProcessingWhenSafe` 的 PASSIVE 回调中查询
>   `FileBasicInformation`/`FileReparsePointInformation`，对自有 tag 用
>   `FltSetInformationFile` 补 `RECALL_ON_DATA_ACCESS`（user-mode 实测无法
>   单独设置该位）。
> - 实测：宿主 76 项测试通过；driver 7,168 字节、导入
>   `FLTMGR.SYS` + `NTOSKRNL.EXE`、无 RWX/FP/SSE。
> - 未完成：实际 `FltTagFile`/属性写入、目录枚举投影、扫描配额与真机 L2。

> **v1.4 KM-2/KM-3 实施（2026-10-28）**
>
> - 占位符创建：`cmflt-client create-placeholder` 建 sparse 文件 + GUID
>   reparse + `OFFLINE`；driver CREATE post-op 在
>   `FltDoCompletionProcessingWhenSafe` 的 PASSIVE 回调中识别自有 tag，并用
>   `FltSetInformationFile` 补 `RECALL_ON_DATA_ACCESS`（本机实测 user-mode 无法
>   单独设置该位）。
> - `cmflt-core::coordinator::HydrationCoordinator`：把 Placeholder 范围、
>   ScanQuota、hydration 状态机与 tx_id 组合为纯逻辑协调器；覆盖
>   Pass/Recall/Busy/Degraded/Quota/Invalid、取消/超时/switch 失败与
>   switch 中取消/排空、partial→full 范围标记。
> - 协议补齐 `Dehydrate`/`PinSet`/`Invalidate`；`cmflt-sim::engine::MockEngine`
>   消费 `HydrateRequest` 生成 `HydrateReply`，端到端集成测试跑通
>   coordinator → request → engine → reply → coordinator。
> - 实测：宿主 96 项测试通过；driver 7,168 字节、导入
>   `FLTMGR.SYS` + `NTOSKRNL.EXE`、无 RWX/FP/SSE。
> - 未完成：内核 READ/Cbdq/FltSendMessage 接线、目录投影、真机 L2/G1。

---

## 0. 结论摘要

| 维度 | 结论 |
|---|---|
| 语言可行性 | **可行**。Rust 可直出 `.sys`（`crate-type=["cdylib"]` + MSVC link.exe），**不需要 C 桩**。已有真实 minifilter 先例（`apriorit/filesystem-kernel-filter-rs`，2026-09 活跃）。 |
| 最大工程缺口 | **FltMgr 零绑定**。`wdk-sys` 只有 `STATUS_FLT_*` 常量；`windows-drivers-rs` 至今不支持 minifilter（相关 PR 自 2025 起 open 未合并）。→ **自建 `flt` 绑定层**（本方案 §5）。 |
| "更稳定"的抓手 | 不是"换了语言就安全"，而是四条可验证的机制：**① 编译期消灭缺陷类；② 内核逻辑可在用户态确定性仿真测试；③ 一切外部依赖（用户态引擎）视为不可信 + 看门狗降级；④ 崩溃可恢复（对账日志）。** 且 panic 源被结构化消除——这是 Rust 在内核里最需要小心的地方（§7.4）。 |
| 官方红线（必须遵守） | ① HSM altitude 组内 filter **不得**对 `RECALL_ON_DATA_ACCESS` 文件做 targeted 缓存读写（数据损坏风险）；② **必须**处理 section 同步钩子，否则 mmap 读到占位符的 0；③ 后备存储切换用官方原语 `FsRtlChangeBackingFileObject`，**不要**自己重造 section/cache 一致性。 |
| 排期硬约束 | **2026 年签名政策已变**：attestation 签名限定测试用途，零售分发需 **HLK/WHCP**，且 Windows Server 2016+ 不接受 filter 驱动的 attestation 提交。开发期靠 test-signing。 |
| 预估工作量 | KM-0…KM-7 共 **18–30 人月**（含验证），其中 FltMgr 绑定层 1–2 人月、水合状态机 3–5 人月、验证与压测 6–10 人月。**验证占一半以上**——这正是"更稳定"的代价所在。 |

---

## 1. "更稳定"如何定义与度量

把"稳定"变成可验收的门禁，否则只是一句口号。`cmflt` 的稳定性按六项指标定义：

| # | 指标 | 目标 | 度量方式 |
|---|---|---|---|
| S1 | **卸载零泄漏** | 任意时刻 `fltmc unload cmflt` 在 5s 内成功 | Filter Verifier 在卸载时对 context/name-info/callback-data/work-item 引用计数断点，零断点才算过（§10 门禁 G1） |
| S2 | **取消零丢单** | 随机 `CancelIoEx` × 水合 × 卸载交叉矩阵下无双完成/无悬挂 | 双完成触发 `0x44`（MULTIPLE_IRP_COMPLETE_REQUESTS）；矩阵全跑 + Verifier 无举报（G2） |
| S3 | **IRQL 零违约** | 分页代码与阻塞调用不出现在 DISPATCH_LEVEL 及以上 | 编译期类型 + Debug 运行时断言 + `verifier /flags` Force IRQL Checking + 反汇编审计（§7.1） |
| S4 | **无超时挂死** | 任何内核→用户态请求都有超时与降级路径，用户态引擎挂死不影响系统可用性 | 混沌注入：kill/暂停引擎进程，观察文件系统行为有界（G4） |
| S5 | **崩溃可恢复** | BSOD/断电重启后，占位符与远端状态一致，无"幽灵半水合"文件 | 恢复对账（§6.6）+ 掉电注入测试（G5） |
| S6 | **兼容性矩阵通过** | 与 cldflt(OneDrive/CfAPI)、主流杀软、备份软件、Windows Search、robocopy /b 共存无数据损坏 | §10 L2 场景矩阵逐项通过（G3） |

> **设计含义**：本文档后续的每条机制都映射到 S1–S6 中的至少一项。做不到映射的"最佳实践"不写入方案。

---

## 2. 事实基线（2026-10 调研）

### 2.1 Rust 内核生态现状（决定了必须自建什么）

- `windows-drivers-rs`：crates `wdk 0.4.1 / wdk-sys 0.5.1 / wdk-build 0.5.1`（2025-11 发布，仓库 2026-10 仍活跃）。支持 WDM/KMDF/UMDF，README 仍写明 **"not yet recommended for production use"**。**不支持 minifilter**；`fltkernel.h` 绑定相关 PR（#307/#337/#359）自 2025 年起 open 未合并。
- `wdk-sys` 0.5.1 **无任何 FltMgr 绑定**（仅 `STATUS_FLT_*` 常量与 `_fltused` 符号）。FltMgr 绑定必须自建。
- 先例：`SubconsciousCompute/poc-windows-rust-filter`（2023，PoC）、**`apriorit/filesystem-kernel-filter-rs`**（2026-09 活跃，真实 minifilter，用 fork 的 windows-drivers-rs 加 `minifilter` feature，在 `build.rs` 手写 `cargo:rustc-link-lib=static=fltMgr`）。
- 微软自身：Win11 内核已有 Rust 组件（`win32kbase_rs.sys`），2025-05 修过一个 panic 缺陷；官方 2025 年发过两篇驱动 Rust 博客，但无 minifilter 支持承诺。

### 2.2 构建链事实（可直接采用）

| 事项 | 结论 |
|---|---|
| 产物形态 | `crate-type = ["cdylib"]`，rustc 调 MSVC `link.exe` 直接产出 DLL，由构建脚本改名为 `.sys`。**无需 C 桩、无需 staticlib**。 |
| 必须链接 | `FltMgr.lib`（自加）、`BufferOverflowFastFailK.lib`（提供 `memcpy/memset`）、`ntoskrnl.lib`、`hal.lib`、`wmilib.lib` |
| 链接参数 | `/DRIVER /NODEFAULTLIB /SUBSYSTEM:NATIVE /KERNEL /ENTRY:DriverEntry /IGNORE:4257,4216` |
| 硬性编译要求 | `-C target-feature=+crt-static`；`panic = "abort"`；**必须禁用 rust-lld**（否则 `sc start` 报 998） |
| 需要提供的符号 | `_fltused`（wdk-sys 已给）、`__CxxFrameHandler3/4`、`__GSHandlerCheck_EH4`（rust-lang#101134 相关，wdk-sys 已给）；`__chkstk` 由 WDK 库解析 |
| 已知上游缺口 | ① `wdk-alloc` 的 `GlobalAlloc` **未实现对齐**（issue #303/#353 open）→ 我们自己写；② **无 PAGE 段一等支持**（issue #507）→ 用 `#[link_section = "PAGE"]` + `PAGED_CODE()` 宏，注意泛型/内联/LTO 会搬移代码；③ rustc 无 MSVC `/kernel` 等价物 → **`/IGNORE:4257` 掩盖了"未用 /kernel 编译"的告警**，浮点/SSE 需自己守（§7.2）。 |
| panic 行为 | `panic=abort` 时 `wdk-panic` 调 `KeBugCheckEx(0x52504E43, file, line, col, 0)`。**这意味着任何 Rust panic 都是 BSOD** —— 本方案 §7.4 专门处理。 |

### 2.3 占位符语义事实（决定了"哪些绝不能自己做"）

- **官方存在非 CfAPI 的分层存储通道**：占位符 = "拥有它的 minifilter 的 reparse point"，该 filter 需在 **HSM Load Order Group（180000–189999）** 且对文件设 `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`。
- **官方红线**：HSM 组之下的 filter **不得**对带 `RECALL_ON_DATA_ACCESS` 的文件做 targeted 缓存读写 —— 否则**缓存污染 + 数据损坏**。所有 filter 都不应为它做无谓读写。
- **后备存储切换原语**：`FsRtlChangeBackingFileObject`（Vista+，PASSIVE_LEVEL）可把某 stream 的 image/data control area 与 shared cache map 的 file object 换成新 file object，异步生效、旧对象延迟释放。**这是"占位符↔实体数据"切换的官方机制，不要自己重造。**
- `FSCTL_RECALL_FILE` 仅在系统装了 Remote Storage 时有效，是微软自家 HSM 入口，**不是**第三方回调通道。
- 属性语义：`OFFLINE(0x1000)` = 数据已迁至离线存储（应用不应随意改）；`RECALL_ON_OPEN(0x40000)` = open 即昂贵（仅出现在目录枚举）；`RECALL_ON_DATA_ACCESS(0x400000)` = 读数据才昂贵，**仅内核可设**。
- **`SetFileValidData` 不能用于造占位符**：只能增大 VAL、要求 `SE_MANAGE_VOLUME_NAME`、不可用于稀疏/压缩/网络文件、会把未写簇的旧数据暴露给用户（隐私风险）且不可减小。占位符的"读作 0"应通过 **文件大小与 VDL 之差** 表达。
- CfAPI 的公开保证（我们需显式复制）：仅 NTFS；progressive 不保证从 0 顺序回调 → provider 必须支持**随机 seek**；`PARTIAL` 策略未实现；hydration policy 在 open 时确定且不可变；云 reparse 默认对所有进程隐藏（provider 与 `%systemroot%` 下进程除外），可用 `RtlSetProcess/ThreadPlaceholderCompatibilityMode`（`PHCM_DISGUISE/EXPOSE`）切换。

### 2.4 缺陷类事实（决定的防线设计）

| 编号 | 缺陷类 | 关键事实（直接影响实现） |
|---|---|---|
| D1 | IRP 取消 | 挂起**只能**用 `FltCbdq`（cancel-safe queue），且仅限 IRP 操作（`FLT_IS_IRP_OPERATION`），fast I/O 不可挂。FM 在调用取消例程**前已自动出队**；`FltCbdqRemoveIo` 返回非 NULL 后取消即失效，此时才可安全转 work item 延后完成。文档承认"插入与取消存在竞态"。 |
| D2 | 引用泄漏 | `FltGetFileNameInformation` 必须配对释放；泄漏 → **卸载永久挂起**（`InstanceTeardownComplete` 永不调用）。Filter Verifier 在卸载时对引用计数断点。该 API 在分页 I/O、`TopLevelIrp≠NULL`、cleanup 之后、APC 全禁用、section-sync 回调内均返回 `STATUS_FLT_INVALID_NAME_REQUEST`。 |
| D3 | IRQL 误用 | `FltReadFile/FltWriteFile/FltFlushBuffers = PASSIVE_LEVEL`；`FltSendMessage ≤ APC_LEVEL`（Timeout=NULL 可无限等待）。高 IRQL 等待 → `0xA`；DISPATCH 触分页代码 → `0xD1`。 |
| D4 | 递归/重入 | `FltReadFile/FltWriteFile` 只下发到"发起实例之下"（天然防自递归），但 FSD 内部 I/O（分页/EA/set-info）仍重入回调，此时 `TopLevelIrp≠NULL`。严禁向栈顶重发（官方 guidance：会死锁）。 |
| D5 | Cache/VDL 一致性 | 绕过 cache 的写与已建 section 不一致；`FltWriteFile` 对中间缓冲打开的文件走缓存路径天然一致；完成/去水合前应 `FltFlushBuffers`。 |
| D6 | Section 同步 | `IRP_MJ_ACQUIRE_FOR_SECTION_SYNCHRONIZATION` 必须处理：`SyncTypeOther` 不允许失败；`CreateSection` 仅可因内存不足失败；fast I/O 版返回 `FLT_PREOP_DISALLOW_FASTIO` 迫其转 IRP（I/O manager 会改发等价 IRP）。**不处理 → mmap 直接读到底层稀疏数据的 0。** |
| D7 | Oplock/占用 | 后台水合写会 break oplock；**用与用户句柄 share/access 冲突的第二句柄回填写 = `ERROR_SHARING_VIOLATION`**（"另一个程序正在使用此文件"的机制来源）。 |
| D8 | 卸载 | `TeardownStart` 必须完成所有 pending、`FltCbdqDisable`、停 work item、可 `FltCancelIo`；`Complete` 只在所有挂起与自发起 I/O 完成后调用，且必须关闭自开文件。 |
| D9 | 第三方扫描 | 真实事故：`tree`、去重扫描器、AI agent 遍历云目录触发**批量水合**；属性-only open 若被转成数据访问会**死锁**。 |

---

## 3. 关键架构决策（ADR）

### D1 · 语言边界：原生 RUST，无 C/C++ 组件
**决定**：内核驱动 100% Rust；FltMgr ABI 手写在 `cmflt-flt`，不使用 bindgen/libclang，不编译任何仓库内 C/C++ 源；仓库内不维护 C 薄壳回退路径。
**理由**：无 C 桩的直出路径已被 apriorit 项目验证；单语言消除 FFI 语义漂移。2026-10-27 的 KM-0 产物已实际链接为 Native PE（仅导入 `FLTMGR.SYS`），验证了该边界。
**被否方案**：C 驱动 + Rust 逻辑库（稳妥但失去"编译期消灭缺陷类"的主要收益）。
**回退备案**：若 KM-1 后绑定层出现无法用 safe 封装隔离的 UB，回退方案重新立项（可在独立分支评估），不把 C 组件放回本仓库主路径（见文首 v1.1 修订、§9.4）。

### D2 · 薄内核原则（最重要的稳定性决策）
**内核只拥有三件事**：① 水合状态机；② 后备对象切换（调 `FsRtlChangeBackingFileObject`）；③ 与用户态引擎的协议。
其余全部下沉用户态：网络、解密、目录树、缓存、冲突解决、日志。
**理由**：内核每多一行代码，BSOD 概率与验证成本都线性增长；且用户态代码可以用标准 Rust 工具链测（§10 L0）。目标是内核逻辑 **≤ 4000 行 Rust**（不含绑定）。

### D3 · 占位符语义：自研 GUID reparse tag + HSM 组 + `RECALL_ON_DATA_ACCESS`
**决定**：自带非微软 reparse tag（高位必须为 0），filter 注册在 HSM Load Order Group 内（180000+），对占位符设 `RECALL_ON_DATA_ACCESS`（对旧软件兼容再加 `OFFLINE`）。**只拦截"上方"请求**，绝不对自己拥有的占位符做 targeted 缓存读写（§2.3 红线）。
**被否**：复用 `IO_REPARSE_TAG_CLOUD_*`（那是 cldflt 的地盘，会与 OneDrive 冲突且不受支持）。

### D4 · 不重写 section/oplock/cache 语义
**决定**：后备切换一律走 `FsRtlChangeBackingFileObject` + 缓存路径写 + `FltFlushBuffers`；section 同步钩子内"同步水合或明确失败"。
**理由**：这是微软官方提供的原语；自行 union/替换 control area 是数据损坏的头号来源。
**参考**：ProjFS/VFS-for-Git 的工程哲学——把 section/oplock/缓存语义交给平台，自己只做数据回调。

### D5 · 用户态引擎视为不可信
**决定**：协议层强制：消息长度上限、结构体对齐与版本校验、请求超时（默认 15s，可配）、失败熔断（连续失败 N 次进入"只读降级"）、引擎身份校验（端口 SDDL 限管理员/SYSTEM）。
**理由**：引擎可能崩溃、被挂起、被恶意替换；内核不能因此挂死或损坏（S4）。

### D6 · 可卸载（与 WinFsp 的取舍相反）
**决定**：驱动**必须支持干净卸载**（S1 作为 CI 门禁）。
**理由**：我们要频繁升级（云盘客户端迭代快），"加载后不可卸载"会把每次升级变成重启。
**代价**：所有引用/工作项/pending 都必须可排空——这也是 S1 门禁存在的理由。

### D7 · 遥测与黑匣子（一等公民）
**决定**：ETW provider + 固定大小内核环形缓冲（掉电不丢最近 N 条事件的能力不做承诺，但 BSOD 转储里可读）+ 用户态"事件包"落盘。
**理由**：内核 bug 的定位成本占 80% 时间；没有黑匣子就只能靠复现。

### D8 · 与 CfAPI 共存而非对抗
**决定**：`cmflt` 与 cldflt 可同时装载。两者 tag 不同、altitude 不同（我们在 HSM 组，cldflt 在其自有 altitude）；`cmflt` 只处理**自己的 tag**，遇到 `IO_REPARSE_TAG_CLOUD_*` 一律放行。
**理由**：用户很可能同时用 OneDrive。

---

## 4. 模块与 crate 布局

```
drivers/win/
├── Cargo.toml                    # workspace: cmflt-* crates（独立于主 workspace）
├── .cargo/config.toml            # +crt-static（x64 MSVC）
├── cmflt-flt/                    # 【unsafe 边界】手写 FltMgr ABI + 最小 safe 封装
│   ├── src/raw.rs                # 手写 ABI（const 布局断言；无 bindgen 产物）
│   ├── src/safe.rs               # 注册生命周期、NTSTATUS -> Result
│   └── src/lib.rs
├── cmflt-core/                   # 【no_std 纯逻辑】状态机、协议编解码、范围记账
│   ├── src/hydration.rs          # 水合状态机（用户态可测）
│   ├── src/protocol.rs           # 显式小端 schema + 编解码 + 版本/长度校验
│   ├── src/range.rs              # 固定容量 hydrated-range 记账
│   └── src/lib.rs                # #![no_std]；测试时启用 std
├── cmflt-sim/                    # 【std】用户态仿真器 + 属性/负路径测试
├── cmflt-driver/                 # 【cdylib】DriverEntry / unload / panic 策略
│   └── build.rs                  # WDK 导入库探测 + Native 链接参数
├── scripts/                      # host 测试 / 构建 / PE 验证 / test-sign
├── cloudmount.inf                # HSM 组安装信息
└── RUST-MINIFILTER-DESIGN.md
```

**关键点**：`cmflt-core` 是 `#![no_std]` 但无内核依赖，配 `std` feature 后可在用户态编译运行 → **同一份状态机代码，既跑在内核，也在 CI 里被 fuzz/模型检查/并发压测**（§10 L0）。这是"更稳定"最有力的来源。

---

## 5. FltMgr 安全绑定层（`cmflt-flt`）

### 5.1 绑定策略：手写 ABI，禁止 bindgen
- 依据 WDK 10.0.19041 `fltkernel.h` 与 Microsoft Learn DDI 页手写 `cmflt-flt/src/raw.rs`；**不引入 bindgen/libclang/头文件扫描**，也不依赖 `cargo-wdk`。
- 只声明当前里程碑实际使用的符号；`FLT_REGISTRATION` x64 大小 112 字节、`FLT_REGISTRATION_VERSION = 0x0203` 由 `const assert` 锁定。
- `raw` 模块公开但仅限驱动 crate 使用；注册生命周期必须经 `safe` 层的 `FilterRegistration`/`RegisteredFilter`，避免裸指针外泄。
- 新增 ABI 必须同时提供：① 文档链接；② x64 布局断言；③ 至少一个宿主测试或真机验证记录。

### 5.2 引用类型：RAII 化（消灭 D2）
```rust
// 概念示意：每种"获取型 API"对应一个 Drop 即释放的类型
pub struct FileNameInfo(NonNull<FLT_FILE_NAME_INFORMATION>);
impl Drop for FileNameInfo { fn drop(&mut self) { unsafe { FltReleaseFileNameInformation(self.0.as_ptr()) } } }
impl FileNameInfo {
    /// 失败返回 None 而不是 panic；调用方必须"失败即放行"（见状态机 §6.2）
    pub fn try_get(cbd: &CallbackData, fmt: NameFormat, flags: u32) -> Option<Self> { /* unsafe 仅在 raw 内 */ }
}
```
**规则**：内核里禁止 `unwrap/expect`（clippy 自定义 lint 强制），获取失败一律 `Option/Result` → 放行或返回明确 NTSTATUS。
**收益**：把"忘记释放 → 卸载挂起"从"需要 Verifier 才能发现"变成"编译期不可能写出"（S1）。

### 5.3 IRQL 类型状态（消灭 D3）
```rust
pub struct Passive; pub struct Apc; pub struct Dispatch;
pub struct Irql<L>(PhantomData<L>, u8);          // 运行时带真实 KIRQL 值

impl<L> Irql<L> {
    /// 只能"升"不能"降"（降级需显式 unsafe 并说明）
    pub fn raise<L2: AtLeast<L>>(self) -> Irql<L2>;
}
/// 需要 PASSIVE 的 API 只接受 Irql<Passive>
pub fn flush_buffers(irql: Irql<Passive>, ...) -> Result<()>;
/// 分页代码标注：Debug 下断言 IRQL<=APC，Release 下为空
#[macro_export] macro_rules! paged_code { () => { /* PAGED_CODE() */ } }
```
**关键约束**：`#[paged]`（`#[link_section = "PAGE"]`）函数**只能**接受 `Irql<Passive|Apc>`。
已知坑：泛型函数不能加该属性、`#[inline]`/LTO 会搬移代码 → 规则：**分页函数一律 `#[inline(never)]` 且非泛型**，并在 CI 里用反汇编校验段归属（§9.3）。

### 5.4 锁序检查（消灭死锁类）
- 所有锁（`ERESOURCE`/push lock/spinlock）在类型上带 **rank 常量**；Debug 构建维护"当前线程持有的最小 rank"，逆序获取即断言失败并写入黑匣子。
- 生产构建保留一个廉价版本：只在 rank 违反时累加计数器上报 ETW（不上蓝屏）——**降级为可观测事件而不是崩溃**。

### 5.5 需要绑定的 API 清单（分七类，约 70 个）
| 类别 | 代表 API | 用途 |
|---|---|---|
| 注册/生命周期 | `FltRegisterFilter` `FltStartFiltering` `FltUnregisterFilter` `FltGetFilterFromName` | KM-0 |
| 端口通信 | `FltCreateCommunicationPort` `FltCloseCommunicationPort` `FltSendMessage` `FltCloseClientPort` | 与引擎 |
| 回调与数据 | `FLT_OPERATION_REGISTRATION` `FltGetRequestorProcess(Id)` `FltGetFileNameInformation` `FltReleaseFileNameInformation` `FltGetFileObjectFrom*` `FltSetCallbackDataDirty` | 回调 |
| Cancel-safe 队列 | `FltCbdqInitialize` `FltCbdqInsertIo` `FltCbdqRemoveIo` `FltCbdqRemoveNextIo` `FltCbdqDisable` `FltCompletePendedPreOperation` | 水合挂起 |
| 自发起 I/O | `FltReadFile` `FltWriteFile` `FltFlushBuffers` `FltAllocateCallbackData` `FltPerformSynchronousIo` `FltCancelIo` | 水合回填 |
| 上下文与对象 | `FltAllocateContext` `FltSetInstanceContext` `FltGetInstanceContext` `FltReferenceContext` `FltReleaseContext` `FltObjectReference` `FltObjectDereference` | 流/文件上下文 |
| 文件信息与后备 | `FltQueryInformationFile` `FltSetInformationFile` `FsRtlChangeBackingFileObject` `FltTagFile`/`FltUntagFile`（reparse） | 占位符 |
| 辅助 | `FltGetVolumeProperties` `FltGetDiskDeviceObject` `FltIsOperationSynchronous` `FltQueueDeferredIoWorkItem` `FltAllocateDeferredIoWorkItem` `IoQueueWorkItem` | 调度 |

**审计要求**：每个绑定函数在 `safe` 层必须有：① 文档注释引用 Learn 链接；② 前置条件（IRQL/锁/引用）说明；③ 至少一个仿真或真实环境测试覆盖。

---

## 6. 水合状态机（`cmflt-core`）

### 6.1 状态图（显式、不可表示非法态）

```
                    ┌─────────────┐
   CREATE(占位符) → │  Passthrough│  ← 属性-only open / 目录枚举 / 备份读
                    └─────────────┘     （绝不触发水合，D9）
                    
   READ/RANGE 命中未水合数据
        │
        ▼
   ┌──────────┐  投递请求    ┌───────────┐  引擎应答    ┌──────────────┐
   │ Enqueue  │────────────▶│  Waiting  │─────────────▶│  Verifying   │
   │ (Cbdq)   │             │(可取消/超时)│              │ (长度/范围/校验)│
   └──────────┘             └───────────┘              └──────┬───────┘
        │ 取消                    │ 超时                        │ 通过
        ▼                         ▼                             ▼
   ┌──────────┐             ┌──────────┐                ┌──────────────┐
   │ Canceled │             │ Degraded │                │  Switching   │
   │ Complete │             │ (降级策略)│                │(ChangeBacking)│
   └──────────┘             └──────────┘                └──────┬───────┘
                                                                │ 完成
                                                                ▼
                                                         ┌──────────────┐
                                                         │ Hydrated +   │
                                                         │ InSync 状态   │
                                                         └──────────────┘
```

**实现要点**：
- 状态是 `enum`，**转移函数是纯函数**（`(State, Event) -> (State, Vec<Action>)`），所以在用户态仿真器里可以穷举/随机测试。
- 每个"等待"状态必须有 `Cancel` 与 `Timeout` 两条出边——**没有无出边状态**（这条规则由仿真器在 CI 里强制：随机游走不得进入僵局）。
- `Actions` 是可执行指令（`CompleteIrp(status)` / `SendMessage(msg)` / `SwitchBacking(fo)` / `ArmTimer(ms)`），由驱动器（内核）或仿真器（用户态）分别实现。**同一状态机，两种执行器。**

### 6.2 取消安全（D1 的正确用法）
1. pre-op 中判断：`FLT_IS_IRP_OPERATION` 才可挂起；fast I/O 路径返回 `FLT_PREOP_DISALLOW_FASTIO`（D6 同理）。
2. 挂起走 `FltCbdqInsertIo`；**插入前后各检查一次取消**（文档承认竞态，插入后若已取消须立即调用队列的取消例程）。
3. `FltCbdqRemoveIo` 返回非 NULL ⇒ 取消窗口已关闭 ⇒ **此时才允许**转 work item 延后完成。
4. 取消例程内**只做**一件事：`FltCompletePendedPreOperation(Data, FLT_PREOP_COMPLETE, NULL)`。
5. **禁止**出队后仍等待取消；**禁止**双完成（`0x44`）。
6. `TeardownStart` → `FltCbdqDisable` → 排空 → 等自发起 I/O 收敛 → `TeardownComplete`（D8）。

### 6.3 超时与看门狗（S4）
- 每个请求带 `TxId` + 截止时间；内核侧用 `KeSetTimer` 单次定时器轮询（**不用** `FltSendMessage` 的 NULL Timeout 无限等待——D3）。
- 超时策略（按操作类型，可配）：
  - **读数据水合超时** → 完成用户 I/O 为 `STATUS_IO_TIMEOUT`（明确失败，不静默返回 0——**静默 0 是最危险的降级**，上层会把它当真实数据）。
  - **属性/元数据类** → 回退"按占位符元数据回答"（不返回错误）。
- 连续超时 N 次 ⇒ **熔断**：进入 `Degraded` 态，对新水合请求快速失败（`STATUS_CLOUD_FILE_PROVIDER_NOT_RUNNING` 语义），同时通知 UI。恢复条件：引擎重新握手成功且健康检查通过。

### 6.4 幂等与重放（S5）
- 引擎应答丢失（超时后又收到）用 `TxId` 去重；同一 `TxId` 的重放必须是安全的（写入前先校验数据范围与哈希）。
- 引擎侧写入采用"先写临时流 + 原子切换"：数据落到独立文件对象后，用 `FsRtlChangeBackingFileObject` 切换，避免"写一半失败"的可见状态。

### 6.5 崩溃一致性（S5）
- 内核不写持久化日志（避免在关键路径上引入自己的 I/O 与失败模式）；**对账由用户态完成**：
  - 引擎持有 `(file_id, TxId, 已水合范围, 期望大小/校验)` 的持久化记录。
  - 驱动重启后：对每个占位符查询实际 VDL/大小 → 与引擎记录比对 → 不一致的占位符标记为"需重新水合"（幂等重做），而不是信任半成品。
- 内核侧只维护**内存态**：流上下文（占位符标志、水合范围位图、pin 状态）。丢失内存态是安全的，因为可以从属性+引擎记录恢复。

### 6.6 关键陷阱的显式处理
| 陷阱 | 处理 |
|---|---|
| 属性-only open 被转成数据访问 → 死锁（D9） | 在 `IRP_MJ_CREATE`/`QUERY_INFORMATION`/目录枚举路径**绝不下发数据 I/O**；水合只在 `IRP_MJ_READ`、`ACQUIRE_FOR_SECTION_SYNCHRONIZATION`、以及显式 RECALL 路径触发 |
| `TopLevelIrp≠NULL` / 分页 I/O 重入（D4） | 回调入口先判定：若为重入或分页 I/O，**一律放行**（不查名、不水合）——这也是 `FltGetFileNameInformation` 的合法边界 |
| mmap 读到 0（D6） | `IRP_MJ_ACQUIRE_FOR_SECTION_SYNCHRONIZATION` 中**同步水合或明确失败**；`SyncTypeOther` 不得失败；fast I/O 版 disallow；`IRP_MJ_ACQUIRE_FOR_MOD_WRITE` 对称处理 |
| 回填触发 oplock break / 共享冲突（D7） | 回填复用**发起 I/O 的同一 file object 与线程**；后台补全前检查句柄集合与 oplock 状态；冲突则**降级为 partial 水合 + 返回当前可用范围**，绝不静默截断 |
| VDL/稀疏语义错误（D5） | 占位符用"文件大小 > VDL"表达"未水合"；**不使用 `SetFileValidData`**；水合完成后按需 `FltFlushBuffers`；一律走缓存路径写 |
| 数据写入绕过 cache | 只用 `FltWriteFile`（缓存路径）；去水合/隐藏前 `FltFlushBuffers` |
| 第三方扫描器批量水合（D9） | 扫描抑制：属性-only open 与枚举不触发；提供"每次会话水合配额"与 UI toast（对齐 OneDrive 的 Automatic file downloads 设计） |

---

## 7. 十四项稳定机制（对照缺陷类与门禁）

| # | 机制 | 消除的缺陷类 | 验证方式（对应门禁） |
|---|---|---|---|
| 1 | **薄内核**（≤4000 行逻辑） | 全部（减少攻击面） | 代码行数 CI 门禁 + 评审 |
| 2 | **RAII 引用类型** | D2 引用泄漏 → 卸载挂起 | 编译期（不可写出泄漏）+ Verifier（S1/G1） |
| 3 | **IRQL 类型状态 + `#[paged]` 约束** | D3 IRQL 误用 | 编译期 + Force IRQL + 反汇编审计（S3/G6） |
| 4 | **锁序 rank 校验**（Debug 断言 / Release 计数上报） | 死锁 | 仿真器锁序随机测试 + Verifier Deadlock Detection（G2） |
| 5 | **状态机 enum + 纯函数转移** | 逻辑死锁/漏分支/非法态 | 仿真器穷举 + 随机游走无僵局断言（G7） |
| 6 | **取消安全单一状态机**（Cbdq 规则固化） | D1 取消竞态、双完成 | 取消×水合×卸载矩阵（S2/G2） |
| 7 | **看门狗 + 分级降级**（明确失败，不静默 0） | D3 无限等待、S4 挂死 | 混沌注入（kill/暂停引擎）（G4） |
| 8 | **熔断 + 恢复** | 引擎长期不可用时的连锁故障 | 混沌注入 + 恢复测试 |
| 9 | **幂等 TxId + 原子后备切换** | 重放/半写导致的损坏（S5） | 仿真器重放测试 + 掉电注入（G5） |
| 10 | **用户态不可信假设**（长度/版本/校验/超时/身份） | 恶意或损坏的引擎消息 | 协议 fuzz（G7） |
| 11 | **有界资源**（内存/队列/并发按句柄与总额记账 + 背压） | 池耗尽、DoS | Verifier Low Resource Simulation（G4） |
| 12 | **崩溃一致对账**（内核无持久态，引擎侧记录） | S5 幽灵半水合文件 | 掉电注入 + 重启对账测试 |
| 13 | **黑匣子遥测**（ETW + 环形缓冲 + 事件包） | 不可定位（间接提升稳定性） | 每次 BSOD 必须能定位到具体状态与 TxId（G8） |
| 14 | **兼容性护栏**（只处理自有 tag、只拦上方、扫描抑制、reparse 兼容模式） | D9/B9 红线、与 cldflt 冲突 | 共存矩阵（S6/G3） |

### 7.1 关于"分页代码"的额外防线（D3 深化）
- 规则：`#[paged]` 只允许出现在"仅在设计上会在 PASSIVE 执行"的函数上；由类型系统保证（参数含 `Irql<Passive|Apc>`）。
- CI 反汇编审计：对产出 `.sys` 用 `dumpbin /disasm` 扫描 PAGE 段函数的"高 IRQL 可达路径"（保守近似：检查 PAGE 段函数是否调用了非分页函数之外的等待/阻塞原语）。
- 已知上游缺口（`#[link_section="PAGE"]` 与泛型/inline/LTO 的冲突）→ 用"分页函数非泛型 + `#[inline(never)]` + LTO 后段布局校验"三条规则守住。

### 7.2 关于浮点/SSE 的防线
- 事实：rustc 无 MSVC `/kernel` 等价物，`/IGNORE:4257` 掩盖了该差异 → **浮点与 AVX 指令有被编译器引入的风险**。
- 规则：`cmflt-*` 全部 `#![no_std]` 且**禁止 `f32/f64` 类型**（编译期禁止：自定义 clippy lint / `#![deny(illegal_floating_point_literal_pattern)]` 不够，故用 CI 反汇编扫描 + 类型白名单检查）。
- CI：`llvm-objdump -d` 扫描产物，出现 `xmm/ymm/zmm` 寄存器或 `movaps/vaddsd` 等即失败（G6）。
- 附带：`_fltused` 符号必须存在（wdk-sys 提供），否则链接器会因 LLVM 置位而报错。

### 7.3 关于 HVCI / PatchGuard / CET
- 驱动必须**静态代码、W^X**（Rust 天然满足，无 JIT/动态代码）。
- 建议加 `/CETCOMPAT`（内核 Shadow Stack，Win11）—— 当前 wdk-build 不自动加，我们自己在链接参数里补。
- 不注册任何可被滥用的 IOCTL（只暴露通信端口，且 SDDL 限管理员/SYSTEM）。

### 7.4 panic 策略：把"Rust panic = BSOD"变成受控设计（重点）
事实：`panic=abort` 下任何 panic 都是 `KeBugCheckEx`。业界争议（Check Point 对 Windows 自带 Rust GDI 组件越界 BSOD 的分析）集中在"把 C 的静默越界换成确定性 BSOD 是否更好"。

**本方案的态度**：不靠"少 panic"的口号，而是**穷举 panic 源并结构化消除**：

| panic 源 | 消除手段 |
|---|---|
| 索引越界（`v[i]`） | 禁用索引（lint）；一律 `get()/get_mut()` + `Option` |
| `unwrap/expect` | 全 crate `#![deny(clippy::unwrap_used, clippy::expect_used)]` |
| 整数溢出 | `overflow-checks = false`（Release 内核语义与 C 一致）+ 关键运算用 `checked_*/saturating_*` 并显式处理 |
| `slice::split_at` 等前置条件 | 用 `split_at_checked`（Rust 1.80+） |
| 分配失败 | **禁用 `alloc` 的 infallible 接口**：`Vec::push` 等改为 `try_reserve` 包装的自有容器；分配失败返回 `STATUS_INSUFFICIENT_RESOURCES` |
| `unreachable!()` | 用 `debug_unreachable` 风格：Debug 断言 + Release 返回错误码（并在黑匣子记一条 violation） |
| 除零 | 禁用 `/` 对非常量除数；用 `checked_div` |
| 类型转换（as） | `#![deny(clippy::cast_possible_truncation)]` 等 |

**分级处理**：定义"**可恢复违约**"（逻辑前置条件不满足、超时、资源不足 → 记黑匣子 + 返回错误码 + 计数上报，**不 BSOD**）与"**内存安全违约**"（不可恢复 → 立即 bugcheck 并带上下文）。这条分界必须写进代码评审清单：凡是"可能内存安全违约"的地方，panic 是唯一正确选择；其余的必须走错误码。

---

## 8. 协议设计（内核 ↔ 用户态引擎）

### 8.1 单一真源
- 用一份 **消息 schema**（自定义 IDL 或 `#[repr(C)]` + 静态断言）生成两侧代码：内核侧 `cmflt-core::protocol`，引擎侧 Rust（同一 crate 的 std 构建）。
- 每个结构体在编译期断言 `size_of` / `align_of` 与协议常量一致（`const _: () = assert!(...)`）。

### 8.2 消息集（最小化）
| 方向 | 消息 | 说明 |
|---|---|---|
| K→U | `Hydrate{tx_id, file_id, path, offset, len, requestor_pid, deadline_ms}` | 请求水合 |
| K→U | `Dehydrate{tx_id, file_id}` | 释放空间 |
| K→U | `PinSet{tx_id, file_id, pin}` | 始终保留/仅在线 |
| K→U | `Invalidate{file_id}` | 远端变更 |
| U→K | `HydrateReply{tx_id, status, bytes_written, sha1?}` | **只回报结果，不回传数据**（数据由引擎直接写文件流，避免大块内存穿越内核） |
| U→K | `Hello{proto_ver, caps, engine_ver}` | 握手与版本协商 |
| K→U | `HelloAck{status, proto_ver, caps, kernel_ver}` | 握手响应（KM-1 已实现） |
| U→K | `Health{stats}` | 心跳/统计（仅握手成功后） |

### 8.3 协议硬化
- 消息上限（如 64 KiB；路径压缩为 file_id 引用，路径仅在必要消息中出现且长度受限）。
- 字段级校验：`proto_ver` 不匹配 → 拒绝连接；偏移/长度必须落在文件大小内；`file_id` 与回调上下文一致（**防混淆攻击**）。
- 引擎视为不可信（D5）：所有输入按"外部输入"处理，解析器是 fuzz 目标（G7）。
- 背压：内核侧对每个 file object 限并发 1 个水合事务；全局在途事务上限（可配）+ 超限 `STATUS_INSUFFICIENT_RESOURCES`。

---

## 9. 构建与工具链

### 9.1 编译配置（KM-0 实际值）
```toml
# drivers/win/Cargo.toml（独立 workspace）
[profile.dev]    opt-level = 1, panic = "abort"
[profile.release]
opt-level = "z"        # 体积优先（也降低指令缓存压力）
lto = "fat"
codegen-units = 1
panic = "abort"
overflow-checks = false
strip = "debuginfo"    # 保留符号表供 !analyze（如体积允许）
```
- `.cargo/config.toml` 设置 `-C target-feature=+crt-static`（x64 MSVC）。
- `[lib] crate-type = ["cdylib"]`（驱动器 crate）；`cmflt-core` 为 `rlib`（同时给 std 仿真器）。
- `cmflt-driver/build.rs` 探测 WDK 的 `Lib/<ver>/km/x64`，链接 `fltMgr`、`ntoskrnl`、`hal`、`bufferoverflowfastfailk`、`wmilib`；传 `/DRIVER /SUBSYSTEM:NATIVE /ENTRY:DriverEntry /NODEFAULTLIB /IGNORE:4257,4216 /MACHINE:X64 /NXCOMPAT /DYNAMICBASE`。
- 使用 rustc 默认的 MSVC `link.exe`，不启用 rust-lld；`/CETCOMPAT` 暂未加入（VS2019 link 兼容性优先，KM-1 再评估）。
- KM-0 产物实测：4.5 KB，`Native` 子系统，仅导入 `FLTMGR.SYS`；KM-1 产物实测：
  6,144 字节，导入 `FLTMGR.SYS` + `NTOSKRNL.EXE`（自旋锁），无 RWX 段，反汇编无 FP/SSE。

### 9.2 自有分配器（补上游缺口）
- 实现 `GlobalAlloc`：`ExAllocatePool2(POOL_FLAG_NON_PAGED, size, TAG)`，其中 `TAG = 'CmfL'`。
- **必须实现对齐**（上游 `wdk-alloc` 未实现，issue #303/#353）：对齐 > 16 时按 `size + align` 申请并保存原始指针。
- 记账：每次分配/释放更新原子计数器（字节数与块数），暴露给 `!poolused` 之外的自有遥测（便于定位泄漏）。
- 分配失败语义：返回 null → 调用方用 `try_reserve` 路径返回 `STATUS_INSUFFICIENT_RESOURCES`（§7.4）。

### 9.3 CI 门禁（自动检查）
| 编号 | 检查 | 失败即阻断 |
|---|---|---|
| B1 | `cargo clippy -- -D warnings` + 自定义 lint（禁 `unwrap/expect/f32/f64/索引/`as` 截断） | ✅ |
| B2 | 段布局校验：PAGE 段函数清单与"非泛型 + `#[inline(never)]`"规则一致（解析 `dumpbin /headers`） | ✅ |
| B3 | 反汇编审计：无 FP/SSE/AVX 指令（§7.2）；无 RWX 段（HVCI 前提） | ✅ |
| B4 | ABI 审查：`cmflt-flt/src/raw.rs` 与 WDK 头文件逐项核对；`FLT_REGISTRATION` 布局 `const assert` | ✅ |
| B5 | 仿真器测试套件（§10 L0）全绿，含随机游走无僵局 | ✅ |
| B6 | 协议编解码 fuzz 语料回归 | ✅ |
| B7 | 结构体布局静态断言（协议） | ✅ |

### 9.4 构建/分发路径
- **开发期**：`bcdedit /set testsigning on` + 自签证书；VM 快照回滚脚本化。
- **内测分发**：**attestation 签名**（注意 2026 政策：**仅测试用途**，不可零售发布；Windows Server 2016+ 不接受 filter 驱动的 attestation）。
- **零售分发**：必须走 **HLK/WHCP**（提交 CAB → Partner Center → 重建 `.cat`）。需采购 EV 证书。**排期上应把 HLK 认证当作一个独立里程碑（KM-7）**。
- 回退备案（D1 v1.1）：内核回调层与 `cmflt-core` 之间保持"纯数据 + 纯状态"接口，便于未来独立评估替代实现；**仓库内不保留 C 薄壳组件**，也不为其提供构建路径。

---

## 10. 测试与验证矩阵

### L0 · 用户态仿真（最高性价比，先建）
- **`cmflt-sim`**：mock FltMgr —— 实现 `cmflt-core` 需要的全部能力（回调投递、Cbdq 语义、取消注入、IRQL 断言、定时器虚拟时钟、随机故障注入）。同一状态机代码在用户态编译运行。
- 覆盖：
  - 状态机**穷举**：所有 `(State, Event)` 组合，断言无非法态/无僵局/取消与超时均有出边。
  - **随机游走**：数千次随机事件序列（含取消×完成×超时×卸载交错），断言不变量（引用计数归零、无双完成、TxId 去重）。
  - **Loom**（需 std）：对协议队列与事务表的并发实现做**全交错**验证（Loom 不能在 no_std 里跑，所以把并发结构留在 std 可测的 crate 中，见 §4 布局）。
  - **Kani**（bounded model check，no_std 起支持）：对协议解码器与范围计算做有界验证（越界/溢出/长度校验）。
  - **Miri**（`MIRI_NO_STD`）：对 `unsafe` 边界之外的纯逻辑查 UB（注意 Windows host 支持弱，用 Linux target 跑）。
  - **cargo-fuzz**：协议解码器（把 no_std 逻辑抽到 std 宿主）。
- **产出**：状态机覆盖率报告 + 不变量清单。

### L1 · VM 单机（内核真机）
- Driver Verifier：`Special Pool / Pool Tracking / Force IRQL Checking / Deadlock Detection / Randomized Low Resource Simulation / Enhanced I/O Verification` + **File System Filter Verification**（Win11：`verifier /ruleclasses 5 37 /driver cmflt.sys`）。
- `!fltkd`（`!filters / !filter / !instance / !volume / !cbd / !frames`）做引用与帧审查；`!poolused`/PoolMon 看池泄漏；`fltmc` 做加载/卸载/列举。
- **门禁 G1**：200 次"加载 → 混合负载 → 卸载"循环，Verifier 零举报、卸载均在 5s 内完成。

### L2 · 场景矩阵（真实世界负载）
| 场景 | 工具/方法 | 关注点 |
|---|---|---|
| 大文件顺序读 | `diskspd` | 吞吐、水合粒度、range 合并 |
| 随机读（含 mmap） | `diskspd -r`、自研 mmap 压测 | section 钩子正确性（D6） |
| 备份读路径 | `robocopy /b`、`wbadmin` | 属性-only open 不触发水合（D9） |
| 杀软共存 | Defender 全量扫描 + 第三方 AV | 扫描抑制、无死锁 |
| 搜索索引 | Windows Search 重建索引 | 枚举路径零 I/O |
| 备份/去重工具 | 文件去重扫描器 | 批量水合抑制（配额） |
| OneDrive/CfAPI 共存 | 同机跑 OneDrive | 各自 tag 互不干扰（D8/护栏） |
| 多进程竞争 | 8 进程同时读写同一占位符 | 单文件并发水合去重 |

### L3 · 混沌与故障注入
- 引擎侧：kill -9 / 挂起（暂停进程）/ 延迟应答 / 返回损坏数据 / 返回错误长度 / 版本不符 → 断言内核行为有界、降级路径生效、无静默 0（**门禁 G4**）。
- 内核侧：Low Resource Simulation（分配失败注入）、`FltCancelIo` 随机取消、随机 `CancelIoEx`。
- 存储侧：低空间、拔盘（可移动卷）、卷被强制卸载。
- 掉电注入：VM 强制断电 → 重启 → 对账（**门禁 G5**）。

### L4 · Soak 与事后分析
- 72h+ 混合负载 soak（Verifier 全开），内存/句柄/引用零增长。
- BSOD 自动 triage 脚本：`!analyze -v` + `!fltkd.cbd` + 黑匣子事件包 → 生成"状态 + TxId + 调用栈"报告（**门禁 G8**：任何 BSOD 都必须能定位到具体状态）。

---

## 11. 里程碑与退出判据

| 阶段 | 内容 | 退出判据 | 估算 |
|---|---|---|---|
| **KM-0** 骨架与构建 | cdylib 直出 `.sys`；`.inf`；段布局与链接参数；空 filter（无回调）可加载卸载 | 构建/PE 门禁绿（**2026-10-27 已达成**）；`sc start/stop` 与 `fltmc` 可见（**待 testsigning/VM**） | 3–4 周 |
| **KM-1** 绑定与骨架回调 | `cmflt-flt` safe 层（§5.5 清单）；通信端口 + 默认安全描述符；握手/版本协商；`Hello/HelloAck/Health` | **实现完成（2026-10-27）**：端口/握手/客户端 + 52 宿主测试；引擎真机连接与 **G1 首版**待 testsigning/VM；绑定层无 C/C++ 组件 | 4–6 周 |
| **KM-2** 只读视图 | 自有 tag 占位符创建；目录枚举投影（元数据）；**属性-only open 零 I/O**；扫描抑制 | **续（2026-10-28）**：NTFS-only attach + 真实 CREATE 分类 + 占位符元数据/范围转移 + reparse blob + ScanQuota + 占位符创建（user-mode reparse/offline + driver post-op recall）（76 tests）；目录投影待续 | 4–6 周 |
| **KM-3** 按需水合 | 状态机全量（§6）+ Cbdq 取消安全 + 超时/降级/熔断 + `FsRtlChangeBackingFileObject` 切换 + `FltFlushBuffers` | **协调器 + 协议 + MockEngine（2026-10-28，96 tests）**：Pass/Recall/Busy/Quota/Degraded、取消/超时/Draining、partial→full、Dehydrate/Pin/Invalidate；内核 READ/Cbdq 接线与 **G2/G4** 待 testsigning/VM | 6–8 周 |
| **KM-4** mmap 与写路径 | section 同步钩子（读/写/ModWrite）+ fast I/O disallow + oplock/共享冲突降级 + 写回与去水合 | mmap 压测零错读；无 `ERROR_SHARING_VIOLATION` 误报 | 4–6 周 |
| **KM-5** 崩溃一致与遥测 | 对账协议 + TxId 幂等 + 原子后备切换 + ETW/黑匣子 + triage 脚本 | **G5**（掉电对账）；**G8**（BSOD 可定位） | 4–6 周 |
| **KM-6** Shell 集成（用户态） | NSE（导航栏）+ `IShellIconOverlayIdentifier`（注意 15 个上限）+ `IExplorerCommand`（始终保留/释放空间）+ `IThumbnailProvider` | Explorer 体验与 OneDrive 对齐；不改内核 | 4–8 周 |
| **KM-7** 认证与发布 | HLK/WHCP 测试与认证、安装器、静默升级、兼容性白皮书 | **S6 全矩阵通过**；认证取得 | 8–12 周（含等待） |

> 与 CfAPI 路线的并行关系：M2 用 CfAPI 发布产品；`cmflt` 完成后通过 `cloudmount-vfs::MountBackend` 切换，用户侧无感。**`cmflt` 不阻塞任何产品里程碑。**

---

## 12. 风险登记与决策点

| # | 风险 | 影响 | 缓解 | 触发回退 |
|---|---|---|---|---|
| R1 | FltMgr 绑定层出现无法用 safe 隔离的 UB | 内存安全 | 审计 + fuzz + Kani；回退方案重新立项（不引入树内 C 组件，D1 v1.1） | KM-1 结束仍不达标 |
| R2 | 上游（windows-drivers-rs/wdk-*) 行为变更 | 构建断裂 | 锁定版本；绑定与构建参数自持；B4 diff 门禁 | — |
| R3 | `panic=abort` 导致 BSOD（业界争议点） | 可用性 | §7.4 全量 panic 源消除 + 分级处理 + 黑匣子 | 若实测 BSOD 率不可接受 → 扩大"用户态处理"范围（D2 更薄） |
| R4 | HLK/WHCP 认证周期与要求（2026 政策收紧） | 发布延后 | KM-7 独立排期；开发期 testsigning；提前读 HLK 要求 | 认证不通过 → 内部/企业分发（attestation 不适用于零售） |
| R5 | 第三方软件把占位符当普通文件 | 数据损坏/误水合 | 属性语义 + 扫描抑制 + 配额 + 文档；不支持场景明确报错 | — |
| R6 | 与 cldflt/杀软 altitude 冲突 | 加载失败/行为异常 | HSM 组注册 + 只处理自有 tag + 共存矩阵 | — |
| R7 | 验证成本超预期（占总量 >60%） | 排期 | L0 仿真器前置（最大化自动化）；门禁自动化 | — |
| R8 | 缺少 Rust 内核人才/审查者 | 质量 | 绑定层窄化 + 评审清单 + 外部内核专家评审（建议） | — |

**待决策点（需你确认）**：
1. **是否接受"C 薄壳回退路径"**作为第一类公民（我认为必须接受，否则 R1 一旦发生会返工）？
2. **KM-6 Shell 集成是否在本方案范围**（它全是用户态 COM，可用 Rust + windows-rs，但不属于内核）？
3. **是否走 HLK/WHCP 零售路线**（影响是否投入 EV 证书与认证排期）；若只自用/内部分发，可停在 testsigning/attestation。
4. **是否引入外部内核专家做绑定层与状态机评审**（这是我最强烈建议的一项投入）。

---

## 13. 与"C 版复刻"的差异一览

| 维度 | C 骨架复刻 | 本方案（Rust 重写 · 更稳定） |
|---|---|---|
| 缺陷类 | 靠 Verifier 事后发现 | 编译期消除（引用/IRQL/锁序/索引/panic 源） |
| 状态机 | 隐式（散落在回调里） | 显式 enum + 纯函数转移，**可被 CI 穷举** |
| 测试 | 只能真机试 | **同一逻辑代码在用户态确定性仿真** + Loom/Kani/Miri/fuzz |
| 引擎故障 | 未定义 | 超时/熔断/降级（明确失败，绝不静默 0） |
| 崩溃 | 状态丢失，需人工排查 | 对账恢复 + TxId 幂等 + 黑匣子 triable |
| 后备切换 | 需自研 cache/section 一致 | 用官方 `FsRtlChangeBackingFileObject` |
| 红线遵守 | 未明确 | HSM 组 + `RECALL_ON_DATA_ACCESS` + 只拦上方 + 扫描抑制 |
| 卸载 | 可能挂起 | S1 门禁：200 次循环零泄漏、5s 内卸载 |
| panic 策略 | N/A | 穷举 panic 源 + 可恢复/内存安全两级处理 |
| 浮点风险 | N/A | 反汇编门禁禁 SSE/AVX |
| 分发 | testsigning | testsigning → attestation（测试）→ **HLK/WHCP（零售）** |

---

## 附：参考资料（关键事实来源）

- 微软 Learn：`fltkernel` 各 API（IRQL/要求）、`wd/ifs/placeholders`、`wd/ifs/placeholders_guidance`、`wd/ifs/flt-parameters-for-irp-mj-acquire-for-section-synchronization`、`wd/kernel/cancel-safe-irp-queues`、`wd/devtest/file-system-filter-verification`、`wd/devtest/randomized-low-resources-simulation`、`nf-ntifs-fsrtlchangebackingfileobject`、`file-attribute-constants`、`ni-winioctl-fsctl_recall_file`、`cfapi/build-a-cloud-file-sync-engine`
- 构建：`github.com/microsoft/windows-drivers-rs`（issues #307/#337/#359/#507/#303/#353/#182）、`docs.rs/wdk-sys`、`github.com/rust-lang/rust/issues/101134`
- 先例：`github.com/apriorit/filesystem-kernel-filter-rs`、`github.com/SubconsciousCompute/poc-windows-rust-filter`
- panic 争议：`research.checkpoint.com/2025/denial-of-fuzzing-rust-in-the-windows-kernel/`
- 签名政策：`learn.microsoft.com/.../dashboard/driver-signing-offerings`
- 验证工具：Miri（`MIRI_NO_STD`）、Kani（no_std 支持）、Loom、cargo-fuzz
- 学习对象：`winfsp` 的 sys 层取舍（长 pending 的过期定时器清理、不可卸载取舍）、WDK 样例 `cancelSafe` / `swapBuffers` / `SimRep`
