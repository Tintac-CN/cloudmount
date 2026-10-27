//! CloudMount minifilter — KM-0 image.
//!
//! This milestone intentionally registers an empty filter: no operation
//! callbacks, no contexts, no communication port. It proves the native-Rust
//! build and lifecycle path (`DriverEntry` -> `FltRegisterFilter` ->
//! `FltStartFiltering` -> unload callback -> `FltUnregisterFilter`) before any
//! data-path logic is added. All hydration logic lives in `cmflt-core` and is
//! tested on the host.
//!
//! There is no C stub, no C runtime startup, and no bindgen output. The only
//! non-Rust inputs at link time are Microsoft's WDK import libraries.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, Ordering};

#[cfg(not(test))]
use cmflt_flt::raw::KeBugCheckEx;
use cmflt_flt::raw::{FltUnregisterFilter, NtStatus, PdriverObject, PfltFilter, STATUS_SUCCESS};
use cmflt_flt::safe::{FilterRegistration, RegisteredFilter};

/// Custom bugcheck code used for a Rust panic. The four parameters carry the
/// source location (file pointer, line, column, 0). This turns every panic
/// into a deterministic, triageable bugcheck instead of an unwind through
/// kernel frames.
#[cfg(not(test))]
const CMFLT_BUGCHECK_PANIC: u32 = 0x5250_4E43;

static FILTER: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

static REGISTRATION: FilterRegistration = FilterRegistration::new(filter_unload);

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

    if let Err(status) = unsafe { filter.start() } {
        unsafe { filter.unregister() };
        return status;
    }

    FILTER.store(filter.into_raw(), Ordering::Release);
    STATUS_SUCCESS
}

/// Filter unload callback. FltMgr calls this before unloading the driver; it
/// must unregister the filter so all instances and contexts are torn down.
unsafe extern "system" fn filter_unload(_flags: u32) -> NtStatus {
    let filter: PfltFilter = FILTER.swap(null_mut(), Ordering::AcqRel);
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
