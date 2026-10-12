/*++ BUILD: 需要 WDK（Windows Driver Kit）。此文件为设计骨架，不能脱离 WDK 单独编译。
 *
 * CloudMount Windows 存储内核 —— minifilter 骨架（设计文档式代码）
 *
 * 路线（见 README.md）：
 *   - Filter Manager minifilter，挂在 NTFS 卷上；
 *   - 占位符 = 自定义 reparse point（GUID tag，需向微软申请）；
 *   - 水合：拦截 IRP_MJ_CREATE/READ，经 Filter Manager message port
 *     通知用户态 Rust 引擎（cloudmountd），回填数据后放行；
 *   - 本文件只落骨架与注释，真实实现按 KM-0..KM-5 推进。
 *
 * 借鉴对象（只学设计，不复制代码）：
 *   - cldflt.sys 的占位符状态机语义（CfAPI 文档公开部分）
 *   - WinFsp 的用户态/内核事务模型（FSD 路线，参考其 IOCTL 设计）
 */

#include <fltKernel.h>
#include <dontuse.h>
#include "include/cloudmount_proto.h"

// ---------------------------------------------------------------------------
// 全局
// ---------------------------------------------------------------------------

static PFLT_FILTER g_Filter = NULL;
static PFLT_PORT g_ServerPort = NULL;   // 用户态连接入口（Rust 引擎）
static PFLT_PORT g_ClientPort = NULL;   // 已连接的引擎端口

// TODO(KM-0): 通过 INF 注册 reparse tag GUID；正式 tag 向 rpid@microsoft.com 申请。
// DEFINE_GUID(CLOUDMOUNT_REPARSE_TAG_GUID, ...);

// ---------------------------------------------------------------------------
// 回调：卷/实例
// ---------------------------------------------------------------------------

// InstanceSetup：只在 NTFS 卷上附着（占位符依赖 NTFS reparse point）。
static NTSTATUS
CmInstanceSetup(_In_ PCFLT related_object_unused,
               _In_ PCFLT_INSTANCE instance,
               _In_ DEVICE_TYPE volume_device_type,
               _In_ FS_FILTER_SECTION_SYNC_TYPE unused,
               _In_ ULONG flags)
{
    UNREFERENCED_PARAMETER(related_object_unused);
    UNREFERENCED_PARAMETER(unused);
    UNREFERENCED_PARAMETER(flags);

    if (volume_device_type != FILE_DEVICE_DISK_FILE_SYSTEM) {
        return STATUS_FLT_DO_NOT_ATTACH;
    }
    // TODO(KM-0): FltGetVolumeProperties → 判定 NTFS（FILE_FS_ATTRIBUTE_INFORMATION）
    UNREFERENCED_PARAMETER(instance);
    return STATUS_SUCCESS;
}

// ---------------------------------------------------------------------------
// 回调：创建（水合触发点）
// ---------------------------------------------------------------------------

// Pre-create：命中本驱动 reparse point 的文件 → 发起水合事务。
//
// 语义对齐 CfAPI（FETCH_DATA 回调）：
//   1. FltGetFileInformation 查 reparse tag；
//   2. 命中 → 经 message port 发 CLOUDMOUNT_MSG_HYDRATE{file_id, offset, len}；
//   3. 等待引擎回包（可中断）；失败则返回 IO_ERROR，让上层得到明确错误；
//   4. 成功 → 引擎已把数据写回文件流，放行 create。
static FLT_PREOP_CALLBACK_STATUS
CmPreCreate(_Inout_ PFLT_CALLBACK_DATA data,
            _In_ PCFLT_RELATED_OBJECTS flt_objects,
            _Flt_CompletionContext_Outptr_ PVOID *completion_context)
{
    UNREFERENCED_PARAMETER(data);
    UNREFERENCED_PARAMETER(flt_objects);
    UNREFERENCED_PARAMETER(completion_context);
    // TODO(KM-2): 水合事务。注意：
    //   - 不得在 DISPATCH_LEVEL 等待；
    //   - 事务需可取消（IRP 取消 → 通知引擎中止）；
    //   - 与 CC/Mm 缓存一致性：回填前 FltFlushBuffers，避免脏页覆盖。
    return FLT_PREOP_SUCCESS_WITH_CALLBACK;
}

// ---------------------------------------------------------------------------
// 用户态通信端口（对齐 cldflt ↔ CfConnectSyncRoot 模型）
// ---------------------------------------------------------------------------

// 引擎连接/断开回调：Rust 侧 cloudmountd 通过 FilterConnectCommunicationPort 接入。
static NTSTATUS
CmDefaultConnect(_In_ PFLT_PORT client_port, _In_opt_ PVOID server_port_cookie)
{
    UNREFERENCED_PARAMETER(server_port_cookie);
    g_ClientPort = client_port;
    return STATUS_SUCCESS;
}

static VOID
CmDefaultDisconnect(_In_opt_ PVOID cookie)
{
    UNREFERENCED_PARAMETER(cookie);
    g_ClientPort = NULL; // 引擎断开 → 进入"仅占位符"降级模式：拒绝对未水合文件的访问
}

// 消息分发：内核 → 引擎的请求复用此端口（HYDRATE/PIN/DEHYDRATE/INVALIDATE）。
static NTSTATUS
CmMessageNotify(_In_opt_ PVOID port_cookie,
                _In_ PVOID input_buffer,
                _In_ ULONG input_len,
                _Out_opt_ PVOID output_buffer,
                _In_ ULONG output_len,
                _Out_ PULONG return_len)
{
    UNREFERENCED_PARAMETER(port_cookie);
    UNREFERENCED_PARAMETER(input_len);
    UNREFERENCED_PARAMETER(output_len);
    UNREFERENCED_PARAMETER(return_len);
    // TODO(KM-1): 按 CLOUDMOUNT_MSG_TYPE 分发；结构体见 include/cloudmount_proto.h
    //（内核/用户态共享头，Rust 侧用 repr(C) 镜像）。
    if (input_buffer == NULL || output_buffer == NULL) {
        return STATUS_INVALID_PARAMETER;
    }
    return STATUS_NOT_IMPLEMENTED;
}

// ---------------------------------------------------------------------------
// DriverEntry /Unload
// ---------------------------------------------------------------------------

static const FLT_OPERATION_REGISTRATION g_Callbacks[] = {
    { IRP_MJ_CREATE, 0, CmPreCreate, NULL },
    // TODO(KM-2): IRP_MJ_READ / IRP_MJ_WRITE / IRP_MJ_DIRECTORY_CONTROL /
    //             IRP_MJ_SET_INFORMATION（重命名/删除同步）
    { FLT_OPERATION_END }
};

static const FLT_CONTEXT_REGISTRATION g_Contexts[] = { FLT_CONTEXT_END };

static const FLT_REGISTRATION g_FilterRegistration = {
    sizeof(FLT_REGISTRATION),
    FLT_REGISTRATION_VERSION,
    0,                  // Flags
    g_Contexts,
    g_Callbacks,
    CmDefaultUnloadStub,   // FilterUnloadCallback
    CmInstanceSetup,       // InstanceSetup
    NULL,                  // InstanceQueryTeardown
    NULL,                  // InstanceTeardownStart
    NULL,                  // InstanceTeardownComplete
    NULL, NULL, NULL       // NameGeneration callbacks
};

static NTSTATUS
CmDefaultUnloadStub(_In_ FLT_FILTER_UNLOAD_FLAGS flags)
{
    UNREFERENCED_PARAMETER(flags);
    // 占位符未水合文件存在时拒绝强制卸载，避免数据视图悬空。
    return STATUS_FLT_DO_NOT_DETACH_ON_UNRELEASED;
}

NTSTATUS
DriverEntry(_In_ PDRIVER_OBJECT driver_object, _In_ PUNICODE_STRING registry_path)
{
    NTSTATUS status;
    OBJECT_ATTRIBUTES oa;
    UNICODE_STRING port_name;
    PSECURITY_DESCRIPTOR sd = NULL;

    UNREFERENCED_PARAMETER(registry_path);

    status = FltRegisterFilter(driver_object, &g_FilterRegistration, &g_Filter);
    if (!NT_SUCCESS(status)) {
        return status;
    }

    // 通信端口：仅允许管理员与 SYSTEM 连接（对齐 cldflt 的安全模型）。
    // TODO(KM-0): FltBuildDefaultSecurityDescriptor + SDDL 收紧。
    RtlInitUnicodeString(&port_name, L"\\CloudMountPort");
    InitializeObjectAttributes(&oa, &port_name, OBJ_KERNEL_HANDLE | OBJ_CASE_INSENSITIVE, NULL, sd);

    status = FltCreateCommunicationPort(g_Filter, &g_ServerPort, &oa,
                                        NULL, CmDefaultConnect,
                                        CmDefaultDisconnect, CmMessageNotify, 1);
    if (!NT_SUCCESS(status)) {
        FltUnregisterFilter(g_Filter);
        return status;
    }

    return STATUS_SUCCESS;
}
