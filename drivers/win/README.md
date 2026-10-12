# CloudMount Windows 存储内核（自研 minifilter）

> 状态：**长期目标 / 设计阶段**。M2 先用 CfAPI（微软原生占位符）发布可用产品，
> 本驱动作为后续的自研后端，通过 `cloudmount-vfs::MountBackend` 平滑替换。

## 目标与动机

把"类 OneDrive 占位符 + 按需水合"的挂载能力完全握在自己手里：

- 摆脱 **CfAPI 的限制**（仅 NTFS、SyncRoot 注册、受微软内核组件行为约束）；
- 摆脱 **WinFsp 的 GPLv3**（本项目全开源 AGPL，但希望内核组件许可自主）；
- 配合 Rust 核心做到极致轻量（目标整进程空闲 < 10MB）。

## 架构决策（来自 2026-10 调研）

1. **Minifilter，不是 FSD。** 占位符 = "NTFS 上的云视图"，用 Filter Manager
   挂在 NTFS 卷上拦截 I/O 即可，ACL/缓存管理器/内存映射/oplock 全部由 NTFS
   承担。完整 FSD（WinFsp 路线）工作量大一个数量级，无必要。
2. **内核用 C，用户态用 Rust。** `windows-drivers-rs` 至今不支持 minifilter
   （无 FltMgr/FsRtl 安全抽象，官方口径仍非生产就绪）。C 写 minifilter 骨架，
   Rust 引擎经 **Filter Manager message port**（`FltCreateCommunicationPort`）
   通信 —— 这与 cldflt.sys (CfAPI) 同款机制。
3. **占位符 = 自定义 reparse point。** `FSCTL_SET_REPARSE_POINT_EX` +
   `REPARSE_GUID_DATA_BUFFER`；tag 需向微软申请（rpid@microsoft.com，
   非 Microsoft tag 带 GUID）。水合流程：应用打开占位符 → 拦截
   `IRP_MJ_CREATE`/`STATUS_REPARSE` → message port 通知 Rust 引擎 →
   `downurl`+Range 拉取 → 回填数据 → 转正常文件。
4. **Explorer 集成必须自研（最大隐性成本）。** 原生云状态图标绑定
   `IO_REPARSE_TAG_CLOUD_*` + CfAPI sync root，第三方 tag 不享受。需自研：
   Shell Namespace Extension（导航栏节点）、`IShellIconOverlayIdentifier`
   （注意 15 个上限）、`IExplorerCommand` 右键（"始终保留/释放空间"）、
   `IThumbnailProvider`。用户态 COM 组件，可用 Rust + `windows-rs` 实现。
5. **签名与分发。** 生产需 EV 代码签名证书 + 微软 Attestation signing
   （Partner Center，分钟级）；开发期 `bcdedit /set testsigning on` +
   自签。必须过 Driver Verifier，并在 HVCI 开启环境测试。

## 工作量预期（诚实版）

| 里程碑 | 内容 | 估算 |
|---|---|---|
| KM-0 | WDK 环境 + minifilter 骨架（注册/加载/卸载 + message port 心跳） | 1–2 周 |
| KM-1 | 挂载视图：枚举远端目录 → 本地占位文件（reparse point） | 2–4 周 |
| KM-2 | 按需水合：CREATE/READ 拦截 + Rust 引擎回填 + 缓存一致性 | 4–8 周 |
| KM-3 | pin/释放空间/删除重命名同步、崩溃恢复、Driver Verifier 清零 | 4–8 周 |
| KM-4 | Shell 扩展（NSE + overlay + 右键） | 4–8 周 |
| KM-5 | 签名/安装器/自动更新、真实用户灰度 | 持续 |

到"敢给陌生人装"的质量：**约 12–24 人月**。蓝屏风险始终存在，
Driver Verifier + 双机调试是日常。

## 目录

- `src/filter.c` —— minifilter 入口与回调骨架（设计文档式代码，需 WDK 编译）
- `include/` —— 内核/用户态共享的协议定义（message port 消息结构）
- `cloudmount.inf` —— 驱动安装信息文件骨架

## 参考（借鉴架构，不抄代码）

- WinFsp `src/sys/`（FSD 路线参照其 IOCTL 事务模型；GPLv3，只学设计）
- Microsoft CfAPI 文档与 CloudMirror 示例（占位符状态机语义）
- ProjFS 文档（投影语义）
- OSR/NTFSD 社区关于 minifilter 与 reparse point 的讨论
