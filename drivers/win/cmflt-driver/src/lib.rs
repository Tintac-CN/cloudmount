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
use core::sync::atomic::{AtomicPtr, Ordering};

use cmflt_core::handshake::{EngineState, HandshakeError};
#[cfg(not(test))]
use cmflt_flt::raw::KeBugCheckEx;
use cmflt_flt::raw::{
    DeviceType, FltCloseCommunicationPort, FltFilesystemType, FltInstanceSetupFlags,
    FltOperationRegistration, FltPreopCallbackStatus, FltUnregisterFilter, NtStatus, PdriverObject,
    PfltFilter, PfltPort, FLT_FSTYPE_NTFS, FLT_PORT_ALL_ACCESS, FLT_PREOP_SUCCESS_NO_CALLBACK,
    IRP_MJ_CREATE, IRP_MJ_OPERATION_END, STATUS_BUFFER_TOO_SMALL, STATUS_FLT_DO_NOT_ATTACH,
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

/// KM-2 operation table. The create pre-op is pass-through for now; the
/// host-tested `cmflt_core::policy` classification will be wired to
/// `FLT_CALLBACK_DATA` when the data path lands.
static OPERATIONS: [FltOperationRegistration; 2] = [
    FltOperationRegistration {
        major_function: IRP_MJ_CREATE,
        flags: 0,
        pre_operation: Some(pre_create),
        post_operation: None,
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
/// synchronization, never from CREATE. The host-tested `policy` module will
/// classify `FLT_CALLBACK_DATA` here once the data path lands.
unsafe extern "system" fn pre_create(
    _data: *mut c_void,
    _flt_objects: *mut c_void,
    _completion_context: *mut *mut c_void,
) -> FltPreopCallbackStatus {
    FLT_PREOP_SUCCESS_NO_CALLBACK
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
