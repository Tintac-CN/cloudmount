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
use core::mem::size_of;

pub type NtStatus = i32;
pub type PdriverObject = *mut c_void;
pub type PfltFilter = *mut c_void;

pub const STATUS_SUCCESS: NtStatus = 0;
pub const STATUS_UNSUCCESSFUL: NtStatus = 0xC000_0001u32 as NtStatus;
pub const STATUS_INVALID_PARAMETER: NtStatus = 0xC000_000Du32 as NtStatus;
pub const STATUS_INSUFFICIENT_RESOURCES: NtStatus = 0xC000_009Au32 as NtStatus;

/// `FLT_REGISTRATION_VERSION_0203` — current value for Windows 8 and later.
pub const FLT_REGISTRATION_VERSION: u16 = 0x0203;

pub type FltFilterUnloadFlags = u32;
pub const FLTFL_FILTER_UNLOAD_MANDATORY: FltFilterUnloadFlags = 0x0000_0001;

/// `PFLT_FILTER_UNLOAD_CALLBACK`:
/// `NTSTATUS (*)(FLT_FILTER_UNLOAD_FLAGS Flags)`.
pub type FilterUnloadCallback = unsafe extern "system" fn(FltFilterUnloadFlags) -> NtStatus;

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
    pub operation_registration: *const c_void,
    pub filter_unload_callback: Option<FilterUnloadCallback>,
    pub instance_setup_callback: UnusedCallback,
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
const _: () = assert!(size_of::<FltRegistration>() == 112);

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
    /// `VOID KeBugCheckEx(ULONG BugCheckCode, ULONG_PTR P1..P4)`.
    /// Declared diverging because the documented behavior never returns.
    pub fn KeBugCheckEx(
        bug_check_code: u32,
        parameter1: usize,
        parameter2: usize,
        parameter3: usize,
        parameter4: usize,
    ) -> !;
}
