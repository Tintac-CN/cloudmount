//! Raw FltMgr ABI.
//!
//! Sources: WDK 10.0.19041 `fltkernel.h` and Microsoft Learn DDI pages
//! (`FLT_REGISTRATION`, `FltRegisterFilter`, `FltUnregisterFilter`,
//! `PFLT_FILTER_UNLOAD_CALLBACK`). `FLTAPI` is `NTAPI`; on x64 all of these
//! are the platform C calling convention, which Rust spells `extern "system"`.
//!
//! Do not add a field here without a size assertion and a matching update to
//! the documented layout below.

#![allow(non_camel_case_types)]

use core::ffi::c_void;
use core::mem::{offset_of, size_of};

pub type NtStatus = i32;
pub type PdriverObject = *mut c_void;
pub type PfltFilter = *mut c_void;
pub type PfltPort = *mut c_void;
pub type SecurityDescriptor = *mut c_void;
pub type KIRQL = u8;
pub type KSPIN_LOCK = usize;

pub const STATUS_SUCCESS: NtStatus = 0;
pub const STATUS_UNSUCCESSFUL: NtStatus = 0xC000_0001u32 as NtStatus;
pub const STATUS_NOT_IMPLEMENTED: NtStatus = 0xC000_0002u32 as NtStatus;
pub const STATUS_INVALID_PARAMETER: NtStatus = 0xC000_000Du32 as NtStatus;
pub const STATUS_BUFFER_TOO_SMALL: NtStatus = 0xC000_0023u32 as NtStatus;
pub const STATUS_PORT_DISCONNECTED: NtStatus = 0xC000_0037u32 as NtStatus;
pub const STATUS_REVISION_MISMATCH: NtStatus = 0xC000_0059u32 as NtStatus;
pub const STATUS_INSUFFICIENT_RESOURCES: NtStatus = 0xC000_009Au32 as NtStatus;
pub const STATUS_FLT_DO_NOT_ATTACH: NtStatus = 0xC01C_000Fu32 as NtStatus;

pub type FltInstanceSetupFlags = u32;
pub type DeviceType = u32;
pub type FltFilesystemType = u32;
/// `FLT_FSTYPE_NTFS` (the enum starts at UNKNOWN=0, RAW=1, NTFS=2).
pub const FLT_FSTYPE_NTFS: FltFilesystemType = 2;

pub const FLT_PORT_CONNECT: u32 = 0x0000_0001;
pub const STANDARD_RIGHTS_ALL: u32 = 0x001F_0000;
pub const FLT_PORT_ALL_ACCESS: u32 = FLT_PORT_CONNECT | STANDARD_RIGHTS_ALL;

pub const OBJ_CASE_INSENSITIVE: u32 = 0x0000_0040;
pub const OBJ_KERNEL_HANDLE: u32 = 0x0000_0200;

/// `FLT_REGISTRATION_VERSION_0203` — current value for Windows 8 and later.
pub const FLT_REGISTRATION_VERSION: u16 = 0x0203;

pub type FltFilterUnloadFlags = u32;
pub const FLTFL_FILTER_UNLOAD_MANDATORY: FltFilterUnloadFlags = 0x0000_0001;

/// `PFLT_FILTER_UNLOAD_CALLBACK`:
/// `NTSTATUS (*)(FLT_FILTER_UNLOAD_FLAGS Flags)`.
pub type FilterUnloadCallback = unsafe extern "system" fn(FltFilterUnloadFlags) -> NtStatus;

/// `PFLT_CONNECT_NOTIFY`.
pub type ConnectNotifyCallback = unsafe extern "system" fn(
    client_port: PfltPort,
    server_port_cookie: *mut c_void,
    connection_context: *mut c_void,
    size_of_context: u32,
    connection_port_cookie: *mut *mut c_void,
) -> NtStatus;

/// `PFLT_DISCONNECT_NOTIFY`.
pub type DisconnectNotifyCallback = unsafe extern "system" fn(connection_cookie: *mut c_void);

/// `PFLT_MESSAGE_NOTIFY`.
pub type MessageNotifyCallback = unsafe extern "system" fn(
    port_cookie: *mut c_void,
    input_buffer: *mut c_void,
    input_buffer_length: u32,
    output_buffer: *mut c_void,
    output_buffer_length: u32,
    return_output_buffer_length: *mut u32,
) -> NtStatus;

/// `PFLT_INSTANCE_SETUP_CALLBACK`.
pub type FltInstanceSetupCallback = unsafe extern "system" fn(
    flt_objects: *mut c_void,
    flags: FltInstanceSetupFlags,
    volume_device_type: DeviceType,
    volume_filesystem_type: FltFilesystemType,
) -> NtStatus;

/// `FLT_PREOP_CALLBACK_STATUS` values used by KM-2.
pub type FltPreopCallbackStatus = i32;
pub const FLT_PREOP_SUCCESS_WITH_CALLBACK: FltPreopCallbackStatus = 0;
pub const FLT_PREOP_SUCCESS_NO_CALLBACK: FltPreopCallbackStatus = 1;

/// `PFLT_PRE_OPERATION_CALLBACK`. `FLT_CALLBACK_DATA` stays opaque until the
/// data path needs its parameters.
pub type FltPreOperationCallback = unsafe extern "system" fn(
    data: *mut c_void,
    flt_objects: *mut c_void,
    completion_context: *mut *mut c_void,
) -> FltPreopCallbackStatus;

pub type FltOperationRegistrationFlags = u32;
pub const FLTFL_OPERATION_REGISTRATION_SKIP_PAGING_IO: FltOperationRegistrationFlags = 0x0000_0001;

pub const IRP_MJ_CREATE: u8 = 0x00;
pub const IRP_MJ_OPERATION_END: u8 = 0x80;

/// `FLT_OPERATION_REGISTRATION` (x64 size 32).
#[repr(C)]
pub struct FltOperationRegistration {
    pub major_function: u8,
    pub flags: FltOperationRegistrationFlags,
    pub pre_operation: Option<FltPreOperationCallback>,
    pub post_operation: UnusedCallback,
    pub reserved1: *mut c_void,
}

// The operation table is an immutable static read by FltMgr.
unsafe impl Sync for FltOperationRegistration {}

/// `UNICODE_STRING`; the buffer is borrowed for the duration of the FltMgr
/// call and must remain alive until that call returns.
#[repr(C)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: *mut u16,
}

/// `OBJECT_ATTRIBUTES` (x64).
#[repr(C)]
pub struct ObjectAttributes {
    pub length: u32,
    pub root_directory: *mut c_void,
    pub object_name: *mut UnicodeString,
    pub attributes: u32,
    pub security_descriptor: SecurityDescriptor,
    pub security_quality_of_service: *mut c_void,
}

/// Prefix of `FLT_CALLBACK_DATA` needed by the create callback.
///
/// The real structure has more fields after `Iopb`; because this is a
/// `#[repr(C)]` prefix, reading the first three fields from a real callback
/// data pointer is layout-compatible. The offsets are asserted below.
#[repr(C)]
pub struct FltCallbackData {
    pub flags: u32,
    pub _padding: u32,
    pub thread: *mut c_void,
    pub iopb: *mut FltIoParameterBlock,
}

/// Prefix of `FLT_IO_PARAMETER_BLOCK`; the `Parameters` union begins at
/// offset 24 and its `Create` member begins at offset 0.
#[repr(C)]
pub struct FltIoParameterBlock {
    pub irp_flags: u32,
    pub major_function: u8,
    pub minor_function: u8,
    pub operation_flags: u8,
    pub reserved: u8,
    pub target_file_object: *mut c_void,
    pub target_instance: *mut c_void,
    pub parameters: FltCreateParameters,
}

/// `FLT_PARAMETERS.Create` (x64 size 40).
#[repr(C)]
pub struct FltCreateParameters {
    pub security_context: *mut IoSecurityContext,
    pub options: u32,
    pub file_attributes: u16,
    pub share_access: u16,
    pub ea_length: u32,
    pub ea_buffer: *mut c_void,
    pub allocation_size: i64,
}

/// `IO_SECURITY_CONTEXT` (x64 size 24).
#[repr(C)]
pub struct IoSecurityContext {
    pub security_qos: *mut c_void,
    pub access_state: *mut c_void,
    pub desired_access: u32,
    pub full_create_options: u32,
}

/// Placeholder for the ten callbacks this milestone leaves NULL.
///
/// `Option<extern "system" fn()>` has the same representation as a function
/// pointer (null pointer optimization); the exact signature is irrelevant
/// while the field is `None`.
pub type UnusedCallback = Option<unsafe extern "system" fn()>;

/// `FLT_REGISTRATION` from `fltkernel.h`.
///
/// x64 layout:
/// ```text
/// Size(2) Version(2) Flags(4)                    -> 8
/// ContextRegistration, OperationRegistration      -> 16
/// 11 callback pointers                            -> 88
/// total                                           -> 112
/// ```
#[repr(C)]
pub struct FltRegistration {
    pub size: u16,
    pub version: u16,
    pub flags: u32,
    pub context_registration: *const c_void,
    pub operation_registration: *const FltOperationRegistration,
    pub filter_unload_callback: Option<FilterUnloadCallback>,
    pub instance_setup_callback: Option<FltInstanceSetupCallback>,
    pub instance_query_teardown_callback: UnusedCallback,
    pub instance_teardown_start_callback: UnusedCallback,
    pub instance_teardown_complete_callback: UnusedCallback,
    pub generate_file_name_callback: UnusedCallback,
    pub normalize_name_component_callback: UnusedCallback,
    pub normalize_context_cleanup_callback: UnusedCallback,
    pub transaction_notification_callback: UnusedCallback,
    pub normalize_name_component_ex_callback: UnusedCallback,
    pub section_notification_callback: UnusedCallback,
}

// The registration is immutable after `DriverEntry`; FltMgr may read it from
// arbitrary contexts while the filter is loaded.
unsafe impl Sync for FltRegistration {}

const _: () = assert!(size_of::<Option<FilterUnloadCallback>>() == size_of::<usize>());
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<FltRegistration>() == 112);
    assert!(size_of::<FltOperationRegistration>() == 32);
    assert!(size_of::<UnicodeString>() == 16);
    assert!(size_of::<ObjectAttributes>() == 48);
    assert!(size_of::<FltCallbackData>() == 24);
    assert!(offset_of!(FltCallbackData, iopb) == 16);
    assert!(offset_of!(FltIoParameterBlock, parameters) == 24);
    assert!(size_of::<FltCreateParameters>() == 40);
    assert!(offset_of!(FltCreateParameters, options) == 8);
    assert!(offset_of!(FltCreateParameters, ea_buffer) == 24);
    assert!(size_of::<IoSecurityContext>() == 24);
    assert!(offset_of!(IoSecurityContext, desired_access) == 16);
};

extern "system" {
    /// `NTSTATUS FltRegisterFilter(PDRIVER_OBJECT, const FLT_REGISTRATION *, PFLT_FILTER *)`
    pub fn FltRegisterFilter(
        driver: PdriverObject,
        registration: *const FltRegistration,
        ret_filter: *mut PfltFilter,
    ) -> NtStatus;

    /// `NTSTATUS FltStartFiltering(PFLT_FILTER)`
    pub fn FltStartFiltering(filter: PfltFilter) -> NtStatus;

    /// `VOID FltUnregisterFilter(PFLT_FILTER)`
    pub fn FltUnregisterFilter(filter: PfltFilter);
}

extern "system" {
    /// `NTSTATUS FltCreateCommunicationPort(PFLT_FILTER, PFLT_PORT *,
    /// OBJECT_ATTRIBUTES *, PVOID, PFLT_CONNECT_NOTIFY,
    /// PFLT_DISCONNECT_NOTIFY, PFLT_MESSAGE_NOTIFY, LONG)`
    pub fn FltCreateCommunicationPort(
        filter: PfltFilter,
        server_port: *mut PfltPort,
        object_attributes: *mut ObjectAttributes,
        server_port_cookie: *mut c_void,
        connect_notify: Option<ConnectNotifyCallback>,
        disconnect_notify: Option<DisconnectNotifyCallback>,
        message_notify: Option<MessageNotifyCallback>,
        max_connections: i32,
    ) -> NtStatus;

    /// `VOID FltCloseCommunicationPort(PFLT_PORT)`
    pub fn FltCloseCommunicationPort(server_port: PfltPort);

    /// `VOID FltCloseClientPort(PFLT_FILTER, PFLT_PORT *)`
    pub fn FltCloseClientPort(filter: PfltFilter, client_port: *mut PfltPort);

    /// `NTSTATUS FltBuildDefaultSecurityDescriptor(PSECURITY_DESCRIPTOR *, ACCESS_MASK)`
    pub fn FltBuildDefaultSecurityDescriptor(
        security_descriptor: *mut SecurityDescriptor,
        desired_access: u32,
    ) -> NtStatus;

    /// `VOID FltFreeSecurityDescriptor(PSECURITY_DESCRIPTOR)`
    pub fn FltFreeSecurityDescriptor(security_descriptor: SecurityDescriptor);
}

extern "system" {
    /// `VOID KeBugCheckEx(ULONG BugCheckCode, ULONG_PTR P1..P4)`.
    /// Declared diverging because the documented behavior never returns.
    pub fn KeBugCheckEx(
        bug_check_code: u32,
        parameter1: usize,
        parameter2: usize,
        parameter3: usize,
        parameter4: usize,
    ) -> !;

    /// `VOID KeInitializeSpinLock(PKSPIN_LOCK)`
    pub fn KeInitializeSpinLock(spin_lock: *mut KSPIN_LOCK);

    /// `KIRQL KeAcquireSpinLockRaiseToDpc(PKSPIN_LOCK)`
    pub fn KeAcquireSpinLockRaiseToDpc(spin_lock: *mut KSPIN_LOCK) -> KIRQL;

    /// `VOID KeReleaseSpinLock(PKSPIN_LOCK, KIRQL)`
    pub fn KeReleaseSpinLock(spin_lock: *mut KSPIN_LOCK, new_irql: KIRQL);
}
