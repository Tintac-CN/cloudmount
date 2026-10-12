/*++
 * cloudmount_proto.h —— 内核 minifilter 与用户态 Rust 引擎的共享协议定义。
 *
 * Rust 侧用 #[repr(C)] 镜像这些结构（cloudmount-core::winproto，KM-1 时添加）。
 * 约定：
 *   - 所有结构固定布局、显式 padding、4/8 字节对齐，两侧 sizeof 断言一致；
 *   - 消息经 Filter Manager message port 传递（FilterSendMessage /
 *     FltSendMessage），单消息上限约 65535 字节 —— 大块数据不走这里，
 *     引擎直接写文件流，内核只收"完成/失败"回执。
 */

#ifndef CLOUDMOUNT_PROTO_H
#define CLOUDMOUNT_PROTO_H

#include <ntdef.h>

#ifdef _KERNEL_MODE
#include <ntifs.h>
#endif

/* 消息类型（用户态 → 内核方向复用同一端口） */
typedef enum _CLOUDMOUNT_MSG_TYPE {
    CmMsgHandshake     = 1,  /* 引擎上线，交换版本/能力位 */
    CmMsgHydrateDone   = 2,  /* 水合事务完成（file_id, status） */
    CmMsgPinSet        = 3,  /* 设置 pin 状态（始终保留/仅在线） */
    CmMsgInvalidate    = 4,  /* 远端变更 → 使本地占位符元数据失效 */
} CLOUDMOUNT_MSG_TYPE;

/* 内核 → 引擎方向 */
typedef enum _CLOUDMOUNT_KMSG_TYPE {
    CmKMsgHydrate      = 1,  /* 请求水合：file_id + 范围 */
    CmKMsgCreateDone   = 2,  /* 占位符创建回执 */
    CmKMsgEnumerate    = 3,  /* 目录枚举（KM-1） */
} CLOUDMOUNT_KMSG_TYPE;

/* 通用消息头 */
typedef struct _CLOUDMOUNT_MSG_HEADER {
    UINT32 Type;      /* CLOUDMOUNT_MSG_TYPE 或 KMSG_TYPE，按方向 */
    UINT32 Length;    /* 含头的总长 */
    UINT64 TxId;      /* 事务 ID，请求/回执配对 */
} CLOUDMOUNT_MSG_HEADER;

/* 水合请求（内核 → 引擎） */
typedef struct _CLOUDMOUNT_HYDRATE_REQ {
    CLOUDMOUNT_MSG_HEADER Header;
    WCHAR FilePath[520];   /* 绝对路径，UTF-16，预留 520 = 260*2 */
    UINT64 Offset;
    UINT64 Length;         /* 请求范围；0 表示整个文件 */
} CLOUDMOUNT_HYDRATE_REQ;

/* 握手（引擎 → 内核） */
typedef struct _CLOUDMOUNT_HANDSHAKE {
    CLOUDMOUNT_MSG_HEADER Header;
    UINT32 ProtoVersion;   /* 协议版本，不匹配则拒绝连接 */
    UINT32 Capabilities;   /* 能力位：占位符/pin/目录投影 */
} CLOUDMOUNT_HANDSHAKE;

#endif /* CLOUDMOUNT_PROTO_H */
