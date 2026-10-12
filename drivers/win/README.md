# CloudMount Windows 存储内核（自研 minifilter）

> 状态：**设计定稿 / 待实现**。M2 先用 CfAPI（微软原生占位符）发布可用产品，
> 本驱动作为后续的自研后端，通过 `cloudmount-vfs::MountBackend` 平滑替换。
>
> 📄 **完整实现方案见 [RUST-MINIFILTER-DESIGN.md](RUST-MINIFILTER-DESIGN.md)**
> —— 用 Rust 从零实现，含"更稳定"的 14 项机制、验证矩阵（S1–S6 门禁）与
> KM-0…KM-7 里程碑。本文档只保留动机与总体决策摘要。

## 目标与动机

把"类 OneDrive 占位符 + 按需水合"的挂载能力完全握在自己手里：

- 摆脱 **CfAPI 的限制**（仅 NTFS、SyncRoot 注册、受微软内核组件行为约束）；
- 摆脱 **WinFsp 的 GPLv3**（本项目全开源 AGPL，但希望内核组件许可自主）；
- 配合 Rust 核心做到极致轻量（目标整进程空闲 < 10MB）。

## 架构决策摘要（详细论证见设计文档 §3）

1. **Minifilter，不是 FSD。** 占位符 = "NTFS 上的云视图"，用 Filter Manager
   挂在 NTFS 卷上拦截 I/O 即可，ACL/缓存管理器/内存映射/oplock 全部由 NTFS
   承担。完整 FSD（WinFsp 路线）工作量大一个数量级，无必要。
2. **内核用 Rust（v1.0 方案更新）。** 早前判断"C 写内核"已被推翻：Rust 可用
   `crate-type=["cdylib"]` **直出 `.sys`，无需 C 桩**，已有真实 minifilter 先例
   （`apriorit/filesystem-kernel-filter-rs`）。缺的是 **FltMgr 绑定**（`wdk-sys`
   为零、`windows-drivers-rs` 不支持 minifilter）→ 自建绑定层，并保留"薄 C 壳"
   回退路径。通信仍走 **Filter Manager message port**（cldflt.sys 同款机制）。
3. **占位符 = 自有 GUID reparse point + HSM 组 + `RECALL_ON_DATA_ACCESS`。**
   filter 注册在 HSM Load Order Group（180000+），后备切换用官方原语
   `FsRtlChangeBackingFileObject`，**不自己重造 cache/section 一致性**。
   两条官方红线必须遵守：HSM 组内不得对带 recall 属性的文件做 targeted 缓存读写；
   必须处理 `IRP_MJ_ACQUIRE_FOR_SECTION_SYNCHRONIZATION`（否则 mmap 读到位图的 0）。
4. **Explorer 集成必须自研（最大隐性成本）。** 原生云状态图标绑定
   `IO_REPARSE_TAG_CLOUD_*` + CfAPI sync root，第三方 tag 不享受。需自研：
   Shell Namespace Extension（导航栏节点）、`IShellIconOverlayIdentifier`
   （注意 15 个上限）、`IExplorerCommand` 右键（"始终保留/释放空间"）、
   `IThumbnailProvider`。用户态 COM 组件，可用 Rust + `windows-rs` 实现。
5. **签名与分发（2026 政策已变）。** 开发期 `bcdedit /set testsigning on` + 自签；
   **attestation 签名现已限定测试用途**，零售分发必须过 **HLK/WHCP**，
   且 Windows Server 2016+ 不接受 filter 驱动的 attestation 提交。
   EV 证书不可避免。必须过 Driver Verifier，并在 HVCI 开启环境测试。

## 里程碑与工作量（详见设计文档 §11）

| 里程碑 | 内容 | 估算 |
|---|---|---|
| KM-0 | 构建链（cdylib 直出 .sys）+ 空 filter 可加载卸载 + CI 门禁 | 3–4 周 |
| KM-1 | FltMgr 安全绑定层 + 通信端口 + 握手（**决定是否走 C 回退**） | 4–6 周 |
| KM-2 | 只读视图：占位符创建 + 目录投影 + 扫描抑制 | 4–6 周 |
| KM-3 | 按需水合：状态机 + 取消安全 + 超时降级 + 后备切换 | 6–8 周 |
| KM-4 | mmap/section 钩子 + 写路径 + oplock 降级 | 4–6 周 |
| KM-5 | 崩溃一致对账 + 遥测/黑匣子 + triage 自动化 | 4–6 周 |
| KM-6 | Shell 集成（用户态 COM：NSE/overlay/右键/缩略图） | 4–8 周 |
| KM-7 | HLK/WHCP 认证 + 安装器 + 灰度 | 8–12 周（含等待） |

到"敢给陌生人装"的质量：**约 18–30 人月**，其中**验证与压测占一半以上**——
这正是"更稳定"的代价。改动幅度大，建议在 KM-1 结束后重新评估。

## 当前仓库文件

- [`RUST-MINIFILTER-DESIGN.md`](RUST-MINIFILTER-DESIGN.md) —— **完整实现方案（主文档）**
- `src/filter.c` —— 早期 C 骨架（**已废弃，仅作历史参考**；v1.0 方案改为纯 Rust）
- `include/cloudmount_proto.h` —— 内核/用户态协议定义（将被 Rust schema 取代，暂留参考）
- `cloudmount.inf` —— 驱动安装信息文件骨架

## 参考（借鉴架构，不抄代码）

- WinFsp `src/sys/`（借鉴：每设备过期定时器清理长 pending IRP、"加载后不可卸载"的取舍）
- Microsoft CfAPI 文档与 CloudMirror 示例（占位符状态机语义、progressive 的随机 seek 要求）
- ProjFS 文档（投影语义；官方推荐慢存储改用 CfAPI 的理由）
- WDK 样例：`cancelSafe`（Cbdq 正确用法）、`swapBuffers`、`SimRep`
- `apriorit/filesystem-kernel-filter-rs`（Rust minifilter 先例与 build.rs 链接做法）
- OSR/NTFSD 社区关于 minifilter 与 reparse point 的讨论
