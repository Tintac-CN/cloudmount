//! CloudMount minifilter — KM-1 image.
//!
//! KM-1 adds the Filter Manager communication port and the engine handshake:
//!
//! - `DriverEntry` registers the filter, creates the server port, then starts
//!   filtering; any failure unwinds in reverse order.
//! - `connect`/`disconnect` callbacks own the single client connection.
//! - `message` accepts `Hello`/`Health` frames from `cmflt-core::protocol`,
//!   runs them through the same pure `cmflt-core::handshake::EngineState` used
//!   by host tests, and returns a `HelloAck` in the reply buffer.
//! - unload closes the client port, closes the server port, then unregisters
//!   the filter, so every reference is released before unload completes.
//!
//! There is no C stub, no C runtime startup, and no bindgen output. The only
//! non-Rust inputs at link time are Microsoft's WDK import libraries.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use cmflt_core::handshake::{EngineState, HandshakeError};
use cmflt_core::placeholder::{
    CMFLT_REPARSE_TAG, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
};
use cmflt_core::policy::{classify_create, CreateIntent};
#[cfg(not(test))]
use cmflt_flt::raw::KeBugCheckEx;
use cmflt_flt::raw::{
    DeviceType, FltCallbackData, FltCloseCommunicationPort, FltDoCompletionProcessingWhenSafe,
    FltFilesystemType, FltInstanceSetupFlags, FltOperationRegistration, FltPostOperationFlags,
    FltPostopCallbackStatus, FltPreopCallbackStatus, FltQueryInformationFile,
    FltSetInformationFile, FltUnregisterFilter, NtStatus, PdriverObject, PfltFilter, PfltPort,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_BASIC_INFORMATION_CLASS,
    FILE_REPARSE_POINT_INFORMATION_CLASS, FLT_FSTYPE_NTFS, FLT_PORT_ALL_ACCESS,
    FLT_POSTOP_FINISHED_PROCESSING, FLT_PREOP_SUCCESS_NO_CALLBACK, IRP_MJ_CREATE,
    IRP_MJ_OPERATION_END, STATUS_BUFFER_TOO_SMALL, STATUS_FLT_DO_NOT_ATTACH,
    STATUS_INVALID_PARAMETER, STATUS_NOT_IMPLEMENTED, STATUS_PORT_DISCONNECTED,
    STATUS_REVISION_MISMATCH, STATUS_SUCCESS,
};
use cmflt_flt::safe::{
    close_client_port, CommunicationPort, FilterRegistration, RegisteredFilter, SpinLock,
};

/// Custom bugcheck code used for a Rust panic. The four parameters carry the
/// source location (file pointer, line, column, 0). This turns every panic
/// into a deterministic, triageable bugcheck instead of an unwind through
/// kernel frames.
#[cfg(not(test))]
const CMFLT_BUGCHECK_PANIC: u32 = 0x5250_4E43;

/// Port name shared with `cmflt-client` (`\CloudMountCmfltPort`).
const PORT_NAME: &[u16] = &[
    0x5C, // '\'
    b'C' as u16,
    b'l' as u16,
    b'o' as u16,
    b'u' as u16,
    b'd' as u16,
    b'M' as u16,
    b'o' as u16,
    b'u' as u16,
    b'n' as u16,
    b't' as u16,
    b'C' as u16,
    b'm' as u16,
    b'f' as u16,
    b'l' as u16,
    b't' as u16,
    b'P' as u16,
    b'o' as u16,
    b'r' as u16,
    b't' as u16,
];

static FILTER: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static SERVER_PORT: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static CLIENT_PORT: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static ENGINE: SpinLock<EngineState> = SpinLock::new(EngineState::new());

/// Create-path telemetry counters. They make the policy classification
/// observable without changing create behavior; hydration is still KM-3.
static CREATE_METADATA: AtomicU32 = AtomicU32::new(0);
static CREATE_DIRECTORY: AtomicU32 = AtomicU32::new(0);
static CREATE_DATA: AtomicU32 = AtomicU32::new(0);
static CREATE_UNKNOWN: AtomicU32 = AtomicU32::new(0);

/// KM-2 operation table. The create pre-op classifies the request and always
/// passes through: create itself never hydrates.
static OPERATIONS: [FltOperationRegistration; 2] = [
    FltOperationRegistration {
        major_function: IRP_MJ_CREATE,
        flags: 0,
        pre_operation: Some(pre_create),
        post_operation: Some(post_create),
        reserved1: null_mut(),
    },
    FltOperationRegistration {
        major_function: IRP_MJ_OPERATION_END,
        flags: 0,
        pre_operation: None,
        post_operation: None,
        reserved1: null_mut(),
    },
];

static REGISTRATION: FilterRegistration = FilterRegistration::new(filter_unload)
    .with_instance_setup(instance_setup)
    .with_operations(OPERATIONS.as_ptr());

/// Minifilter entry point. `/ENTRY:DriverEntry` is passed by `build.rs`.
///
/// # Safety
///
/// Called exactly once by the Windows I/O manager with a valid
/// `DRIVER_OBJECT`. It must never be called from user mode or with a null
/// driver object.
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DriverEntry(
    driver: PdriverObject,
    _registry_path: *mut c_void,
) -> NtStatus {
    let filter = match unsafe { RegisteredFilter::register(driver, &REGISTRATION) } {
        Ok(filter) => filter,
        Err(status) => return status,
    };
    let raw_filter = filter.raw();
    FILTER.store(raw_filter, Ordering::Release);

    unsafe { ENGINE.init() };
    let port = match unsafe {
        CommunicationPort::create(
            raw_filter,
            PORT_NAME,
            FLT_PORT_ALL_ACCESS,
            port_connect,
            port_disconnect,
            port_message,
            1,
        )
    } {
        Ok(port) => port,
        Err(status) => {
            FILTER.store(null_mut(), Ordering::Release);
            unsafe { filter.unregister() };
            return status;
        }
    };

    if let Err(status) = unsafe { filter.start() } {
        FILTER.store(null_mut(), Ordering::Release);
        let mut client = CLIENT_PORT.swap(null_mut(), Ordering::AcqRel);
        ENGINE.with(|state| state.on_disconnect());
        unsafe { close_client_port(raw_filter, &mut client) };
        unsafe { port.close() };
        unsafe { filter.unregister() };
        return status;
    }

    SERVER_PORT.store(port.into_raw(), Ordering::Release);
    filter.into_raw();
    STATUS_SUCCESS
}

/// Client connected: there is at most one connection (MaxConnections = 1).
unsafe extern "system" fn port_connect(
    client_port: PfltPort,
    _server_port_cookie: *mut c_void,
    _connection_context: *mut c_void,
    _size_of_context: u32,
    connection_port_cookie: *mut *mut c_void,
) -> NtStatus {
    if client_port.is_null() || connection_port_cookie.is_null() {
        return STATUS_INVALID_PARAMETER;
    }
    unsafe { *connection_port_cookie = null_mut() };
    CLIENT_PORT.store(client_port, Ordering::Release);
    ENGINE.with(|state| state.on_connect());
    STATUS_SUCCESS
}

/// Client disconnected: clear the engine state and close its port.
unsafe extern "system" fn port_disconnect(_connection_cookie: *mut c_void) {
    let mut client = CLIENT_PORT.swap(null_mut(), Ordering::AcqRel);
    ENGINE.with(|state| state.on_disconnect());
    let filter = FILTER.load(Ordering::Acquire);
    unsafe { close_client_port(filter, &mut client) };
}

/// Message callback. The reply buffer is filled directly; no kernel allocation
/// or blocking call is made while the engine state is locked.
unsafe extern "system" fn port_message(
    _port_cookie: *mut c_void,
    input_buffer: *mut c_void,
    input_buffer_length: u32,
    output_buffer: *mut c_void,
    output_buffer_length: u32,
    return_output_buffer_length: *mut u32,
) -> NtStatus {
    if return_output_buffer_length.is_null() {
        return STATUS_INVALID_PARAMETER;
    }
    unsafe { *return_output_buffer_length = 0 };
    if input_buffer.is_null() || input_buffer_length == 0 {
        return STATUS_INVALID_PARAMETER;
    }

    let input = unsafe {
        core::slice::from_raw_parts(input_buffer as *const u8, input_buffer_length as usize)
    };
    let output: &mut [u8] = if output_buffer.is_null() || output_buffer_length == 0 {
        &mut []
    } else {
        unsafe {
            core::slice::from_raw_parts_mut(output_buffer as *mut u8, output_buffer_length as usize)
        }
    };

    match ENGINE.with(|state| state.handle(input, output)) {
        Ok(length) => {
            if length > 0 {
                unsafe { *return_output_buffer_length = length as u32 };
            }
            STATUS_SUCCESS
        }
        Err(error) => handshake_status(error),
    }
}

const fn handshake_status(error: HandshakeError) -> NtStatus {
    match error {
        HandshakeError::NotConnected => STATUS_PORT_DISCONNECTED,
        HandshakeError::NotHandshaken | HandshakeError::Malformed => STATUS_INVALID_PARAMETER,
        HandshakeError::UnsupportedMessage => STATUS_NOT_IMPLEMENTED,
        HandshakeError::VersionMismatch => STATUS_REVISION_MISMATCH,
        HandshakeError::OutputTooSmall => STATUS_BUFFER_TOO_SMALL,
    }
}

/// Attach only to NTFS volumes: placeholders require NTFS reparse points and
/// the HSM semantics of NTFS.
unsafe extern "system" fn instance_setup(
    _flt_objects: *mut c_void,
    _flags: FltInstanceSetupFlags,
    _volume_device_type: DeviceType,
    volume_filesystem_type: FltFilesystemType,
) -> NtStatus {
    if volume_filesystem_type == FLT_FSTYPE_NTFS {
        STATUS_SUCCESS
    } else {
        STATUS_FLT_DO_NOT_ATTACH
    }
}

/// KM-2 create pre-op. Attribute-only opens, directory enumeration and data
/// opens all pass through; hydration is triggered only from READ or section
/// synchronization, never from CREATE. The classification is recorded in
/// scalar counters for telemetry and later scan-suppression policy.
unsafe extern "system" fn pre_create(
    data: *mut c_void,
    _flt_objects: *mut c_void,
    _completion_context: *mut *mut c_void,
) -> FltPreopCallbackStatus {
    match unsafe { read_create_parameters(data) } {
        Some((desired_access, create_options)) => {
            match classify_create(desired_access, create_options) {
                CreateIntent::MetadataOnly => {
                    CREATE_METADATA.fetch_add(1, Ordering::Relaxed);
                }
                CreateIntent::DirectoryEnumeration => {
                    CREATE_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                }
                CreateIntent::DataAccess => {
                    CREATE_DATA.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        None => {
            CREATE_UNKNOWN.fetch_add(1, Ordering::Relaxed);
        }
    }
    FLT_PREOP_SUCCESS_NO_CALLBACK
}

/// Read `DesiredAccess` and the low 24 CreateOptions bits from the documented
/// `FLT_CALLBACK_DATA` / `FLT_IO_PARAMETER_BLOCK` / `FLT_PARAMETERS.Create`
/// prefix layouts. A malformed or unexpected callback is treated as unknown
/// and passed through, never as a data request.
unsafe fn read_create_parameters(data: *mut c_void) -> Option<(u32, u32)> {
    if data.is_null() {
        return None;
    }
    let callback = unsafe { &*(data as *const FltCallbackData) };
    if callback.iopb.is_null() {
        return None;
    }
    let iopb = unsafe { &*callback.iopb };
    if iopb.parameters.security_context.is_null() {
        return None;
    }
    let security = unsafe { &*iopb.parameters.security_context };
    Some((
        security.desired_access,
        iopb.parameters.options & 0x00FF_FFFF,
    ))
}

/// Post-create entry. It may run at DISPATCH_LEVEL, so all file-information
/// work is deferred through `FltDoCompletionProcessingWhenSafe`.
unsafe extern "system" fn post_create(
    data: *mut c_void,
    flt_objects: *mut c_void,
    completion_context: *mut c_void,
    flags: FltPostOperationFlags,
) -> FltPostopCallbackStatus {
    let mut status = FLT_POSTOP_FINISHED_PROCESSING;
    let safe = unsafe {
        FltDoCompletionProcessingWhenSafe(
            data,
            flt_objects,
            completion_context,
            flags,
            safe_post_create,
            &mut status,
        )
    };
    if safe == 0 {
        FLT_POSTOP_FINISHED_PROCESSING
    } else {
        status
    }
}

unsafe extern "system" fn safe_post_create(
    data: *mut c_void,
    _flt_objects: *mut c_void,
    _completion_context: *mut c_void,
    _flags: FltPostOperationFlags,
) -> FltPostopCallbackStatus {
    unsafe { finalize_placeholder(data) };
    FLT_POSTOP_FINISHED_PROCESSING
}

/// A user-mode-created placeholder carries our GUID reparse tag and OFFLINE,
/// but Windows only lets a kernel component set `RECALL_ON_DATA_ACCESS`.
/// Detect the tag and add the recall attribute in one PASSIVE-level pass.
unsafe fn finalize_placeholder(data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let callback = unsafe { &*(data as *const FltCallbackData) };
    if (callback.io_status.status_or_pointer as i32) < 0 {
        return;
    }
    if callback.iopb.is_null() {
        return;
    }
    let iopb = unsafe { &*callback.iopb };
    let instance = iopb.target_instance;
    let file_object = iopb.target_file_object;
    if instance.is_null() || file_object.is_null() {
        return;
    }

    let mut basic = core::mem::MaybeUninit::<[u8; 40]>::uninit();
    let mut returned = 0u32;
    let status = unsafe {
        FltQueryInformationFile(
            instance,
            file_object,
            basic.as_mut_ptr() as *mut c_void,
            40,
            FILE_BASIC_INFORMATION_CLASS,
            &mut returned,
        )
    };
    if status < 0 {
        return;
    }
    let attributes =
        unsafe { core::ptr::read_unaligned((basic.as_ptr() as *const u8).add(32) as *const u32) };
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
        || attributes & FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS != 0
    {
        return;
    }

    let mut reparse = core::mem::MaybeUninit::<[u8; 16]>::uninit();
    let status = unsafe {
        FltQueryInformationFile(
            instance,
            file_object,
            reparse.as_mut_ptr() as *mut c_void,
            16,
            FILE_REPARSE_POINT_INFORMATION_CLASS,
            &mut returned,
        )
    };
    if status < 0 {
        return;
    }
    let tag =
        unsafe { core::ptr::read_unaligned((reparse.as_ptr() as *const u8).add(8) as *const u32) };
    if tag != CMFLT_REPARSE_TAG {
        return;
    }

    let new_attributes = attributes | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_OFFLINE;
    unsafe {
        core::ptr::write_unaligned(
            (basic.as_mut_ptr() as *mut u8).add(32) as *mut u32,
            new_attributes,
        )
    };
    let _ = unsafe {
        FltSetInformationFile(
            instance,
            file_object,
            basic.as_mut_ptr() as *mut c_void,
            40,
            FILE_BASIC_INFORMATION_CLASS,
        )
    };
}

/// Filter unload callback. Close the client port first, then the server port,
/// then unregister the filter. This is the reverse of creation order.
unsafe extern "system" fn filter_unload(_flags: u32) -> NtStatus {
    let filter: PfltFilter = FILTER.swap(null_mut(), Ordering::AcqRel);
    let mut client = CLIENT_PORT.swap(null_mut(), Ordering::AcqRel);
    ENGINE.with(|state| state.on_disconnect());
    unsafe { close_client_port(filter, &mut client) };

    let server = SERVER_PORT.swap(null_mut(), Ordering::AcqRel);
    if !server.is_null() {
        unsafe { FltCloseCommunicationPort(server) };
    }
    if !filter.is_null() {
        unsafe { FltUnregisterFilter(filter) };
    }
    STATUS_SUCCESS
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let (file, line, column) = match info.location() {
        Some(location) => (
            location.file().as_ptr() as usize,
            location.line() as usize,
            location.column() as usize,
        ),
        None => (0, 0, 0),
    };
    unsafe { KeBugCheckEx(CMFLT_BUGCHECK_PANIC, file, line, column, 0) }
}

/// The WDK/LLVM toolchain expects the `_fltused` marker in kernel images.
#[no_mangle]
pub static _fltused: i32 = 0;
