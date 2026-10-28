//! Small safe layer over the raw FltMgr registration calls.
//!
//! The layer is intentionally narrow: it owns the registration lifetime and
//! turns NTSTATUS into `Result`. It does not hide IRQL or locking rules for
//! I/O callbacks yet; those wrappers arrive with the callbacks that need them.

use core::cell::UnsafeCell;
use core::ptr::{null, null_mut};
use core::sync::atomic::AtomicUsize;

use crate::raw::{
    ConnectNotifyCallback, DisconnectNotifyCallback, FilterUnloadCallback,
    FltBuildDefaultSecurityDescriptor, FltCloseClientPort, FltCloseCommunicationPort,
    FltCreateCommunicationPort, FltFreeSecurityDescriptor, FltInstanceSetupCallback,
    FltOperationRegistration, FltRegisterFilter, FltRegistration, FltStartFiltering,
    FltUnregisterFilter, MessageNotifyCallback, NtStatus, ObjectAttributes, PdriverObject,
    PfltFilter, PfltPort, SecurityDescriptor, UnicodeString, FLT_REGISTRATION_VERSION, KSPIN_LOCK,
    OBJ_CASE_INSENSITIVE, OBJ_KERNEL_HANDLE, STATUS_INSUFFICIENT_RESOURCES, STATUS_SUCCESS,
    STATUS_UNSUCCESSFUL,
};

use crate::raw::{KeAcquireSpinLockRaiseToDpc, KeInitializeSpinLock, KeReleaseSpinLock};

/// NT_SUCCESS: non-negative NTSTATUS values are success.
pub const fn nt_success(status: NtStatus) -> bool {
    status >= 0
}

/// Immutable registration object. Store it in a `static`; FltMgr keeps a
/// pointer to it while the filter is registered.
pub struct FilterRegistration {
    raw: FltRegistration,
}

impl FilterRegistration {
    /// Build a registration with only the unload callback populated. All
    /// other callbacks are `NULL`, which is the KM-0 empty-filter shape.
    pub const fn new(unload: FilterUnloadCallback) -> Self {
        Self {
            raw: FltRegistration {
                size: core::mem::size_of::<FltRegistration>() as u16,
                version: FLT_REGISTRATION_VERSION,
                flags: 0,
                context_registration: null(),
                operation_registration: null(),
                filter_unload_callback: Some(unload),
                instance_setup_callback: None,
                instance_query_teardown_callback: None,
                instance_teardown_start_callback: None,
                instance_teardown_complete_callback: None,
                generate_file_name_callback: None,
                normalize_name_component_callback: None,
                normalize_context_cleanup_callback: None,
                transaction_notification_callback: None,
                normalize_name_component_ex_callback: None,
                section_notification_callback: None,
            },
        }
    }

    /// Attach an instance-setup callback (KM-2: NTFS-only attachment).
    pub const fn with_instance_setup(mut self, callback: FltInstanceSetupCallback) -> Self {
        self.raw.instance_setup_callback = Some(callback);
        self
    }

    /// Attach a static `FLT_OPERATION_REGISTRATION` table. The table must be
    /// terminated by `{ IRP_MJ_OPERATION_END }` and outlive the filter.
    pub const fn with_operations(mut self, operations: *const FltOperationRegistration) -> Self {
        self.raw.operation_registration = operations;
        self
    }

    pub fn as_ptr(&self) -> *const FltRegistration {
        &self.raw
    }
}

/// A filter pointer returned by `FltRegisterFilter`.
///
/// The type has no `Drop`: unregistering is an explicit lifecycle step so a
/// driver cannot accidentally call FltMgr from a context where it is illegal.
/// `DriverEntry` converts it with [`RegisteredFilter::into_raw`] and the
/// unload callback calls [`unregister_raw`].
pub struct RegisteredFilter {
    raw: PfltFilter,
}

impl RegisteredFilter {
    /// # Safety
    /// `driver` must be the `PDRIVER_OBJECT` passed to `DriverEntry`, and
    /// `registration` must outlive the registered filter.
    pub unsafe fn register(
        driver: PdriverObject,
        registration: &'static FilterRegistration,
    ) -> Result<Self, NtStatus> {
        let mut filter: PfltFilter = null_mut();
        let status = unsafe { FltRegisterFilter(driver, registration.as_ptr(), &mut filter) };
        if nt_success(status) && !filter.is_null() {
            Ok(Self { raw: filter })
        } else if nt_success(status) {
            Err(STATUS_UNSUCCESSFUL)
        } else {
            Err(status)
        }
    }

    /// # Safety
    /// Must be called at IRQL <= APC_LEVEL, exactly once per registration.
    pub unsafe fn start(&self) -> Result<(), NtStatus> {
        let status = unsafe { FltStartFiltering(self.raw) };
        if nt_success(status) {
            Ok(())
        } else {
            Err(status)
        }
    }

    pub fn raw(&self) -> PfltFilter {
        self.raw
    }

    pub fn into_raw(self) -> PfltFilter {
        self.raw
    }

    /// # Safety
    /// Must be called at IRQL <= APC_LEVEL; no callback may use `self`
    /// afterwards.
    pub unsafe fn unregister(self) {
        unsafe { unregister_raw(self.raw) };
    }
}

/// Unregister a raw filter pointer. Kept separate so a file-scope atomic can
/// hold the pointer without keeping a Rust wrapper alive across callbacks.
///
/// # Safety
/// `filter` must be a non-null pointer returned by `FltRegisterFilter` and
/// must not be unregistered twice.
pub unsafe fn unregister_raw(filter: PfltFilter) {
    if !filter.is_null() {
        unsafe { FltUnregisterFilter(filter) };
    }
}

/// A kernel spinlock-protected value.
///
/// `init` must be called exactly once before the first [`SpinLock::with`].
/// The critical section runs at DISPATCH_LEVEL, so the closure must be short
/// and must not block, allocate, or call any PASSIVE-only API.
pub struct SpinLock<T> {
    lock: AtomicUsize,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            lock: AtomicUsize::new(0),
            value: UnsafeCell::new(value),
        }
    }

    /// # Safety
    /// Must be called exactly once before any `with` call.
    pub unsafe fn init(&self) {
        unsafe { KeInitializeSpinLock(self.lock.as_ptr().cast::<KSPIN_LOCK>()) };
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let irql = unsafe { KeAcquireSpinLockRaiseToDpc(self.lock.as_ptr().cast::<KSPIN_LOCK>()) };
        let result = f(unsafe { &mut *self.value.get() });
        unsafe { KeReleaseSpinLock(self.lock.as_ptr().cast::<KSPIN_LOCK>(), irql) };
        result
    }
}

impl UnicodeString {
    /// Borrow a static UTF-16 slice for a FltMgr call. The slice must not
    /// include a terminating NUL; `Length` is the exact byte length.
    fn from_wide(name: &'static [u16]) -> Self {
        let chars = name.len().min((u16::MAX as usize) / 2);
        let bytes = (chars * 2) as u16;
        Self {
            length: bytes,
            maximum_length: bytes,
            buffer: name.as_ptr() as *mut u16,
        }
    }
}

/// Server side of a Filter Manager communication port.
///
/// No `Drop`: closing is an explicit lifecycle step so unload can close the
/// client port first, then the server port, then unregister the filter.
pub struct CommunicationPort {
    raw: PfltPort,
}

impl CommunicationPort {
    /// # Safety
    /// `filter` must be a registered filter; callbacks must be valid for the
    /// lifetime of the port and must follow the FltMgr IRQL rules.
    pub unsafe fn create(
        filter: PfltFilter,
        name: &'static [u16],
        desired_access: u32,
        connect_notify: ConnectNotifyCallback,
        disconnect_notify: DisconnectNotifyCallback,
        message_notify: MessageNotifyCallback,
        max_connections: i32,
    ) -> Result<Self, NtStatus> {
        let mut security: SecurityDescriptor = null_mut();
        let status = unsafe { FltBuildDefaultSecurityDescriptor(&mut security, desired_access) };
        if !nt_success(status) {
            return Err(status);
        }
        if security.is_null() {
            return Err(STATUS_INSUFFICIENT_RESOURCES);
        }

        let mut unicode = UnicodeString::from_wide(name);
        let mut object_attributes = ObjectAttributes {
            length: core::mem::size_of::<ObjectAttributes>() as u32,
            root_directory: null_mut(),
            object_name: &mut unicode,
            attributes: OBJ_CASE_INSENSITIVE | OBJ_KERNEL_HANDLE,
            security_descriptor: security,
            security_quality_of_service: null_mut(),
        };
        let mut server: PfltPort = null_mut();
        let status = unsafe {
            FltCreateCommunicationPort(
                filter,
                &mut server,
                &mut object_attributes,
                null_mut(),
                Some(connect_notify),
                Some(disconnect_notify),
                Some(message_notify),
                max_connections,
            )
        };
        unsafe { FltFreeSecurityDescriptor(security) };

        if nt_success(status) && !server.is_null() {
            Ok(Self { raw: server })
        } else if nt_success(status) {
            Err(STATUS_UNSUCCESSFUL)
        } else {
            Err(status)
        }
    }

    pub fn raw(&self) -> PfltPort {
        self.raw
    }

    pub fn into_raw(self) -> PfltPort {
        self.raw
    }

    /// # Safety
    /// Must be called at IRQL <= PASSIVE_LEVEL, after the client port has
    /// been closed, and exactly once.
    pub unsafe fn close(self) {
        if !self.raw.is_null() {
            unsafe { FltCloseCommunicationPort(self.raw) };
        }
    }
}

/// Close a client port in the Filter Manager disconnect callback or unload.
///
/// # Safety
/// `filter` must still be registered; `client_port` must point to a live
/// client port variable. FltMgr clears the variable on success.
pub unsafe fn close_client_port(filter: PfltFilter, client_port: &mut PfltPort) {
    if !filter.is_null() && !client_port.is_null() {
        unsafe { FltCloseClientPort(filter, client_port) };
    }
}

const _: () = assert!(STATUS_SUCCESS == 0);

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    unsafe extern "system" fn unload(_flags: u32) -> NtStatus {
        STATUS_SUCCESS
    }

    #[test]
    fn registration_is_stable_on_x64() {
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<FltRegistration>(), 112);
            assert_eq!(
                size_of::<Option<FilterUnloadCallback>>(),
                size_of::<usize>()
            );
        }
    }

    #[test]
    fn registration_sets_documented_fields() {
        let registration = FilterRegistration::new(unload);
        let raw = unsafe { &*registration.as_ptr() };
        assert_eq!(raw.size as usize, size_of::<FltRegistration>());
        assert_eq!(raw.version, FLT_REGISTRATION_VERSION);
        assert!(raw.filter_unload_callback.is_some());
        assert!(raw.context_registration.is_null());
        assert!(raw.operation_registration.is_null());
    }

    #[test]
    fn nt_success_matches_ntstatus_semantics() {
        assert!(nt_success(STATUS_SUCCESS));
        assert!(nt_success(1));
        assert!(!nt_success(-1));
        assert!(!nt_success(STATUS_UNSUCCESSFUL));
    }

    #[test]
    fn unicode_string_and_object_attributes_layout() {
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<UnicodeString>(), 16);
            assert_eq!(size_of::<ObjectAttributes>(), 48);
        }

        let name: &'static [u16] = &[b'\\' as u16, b'C' as u16, b'm' as u16];
        let unicode = UnicodeString::from_wide(name);
        assert_eq!(unicode.length, 6);
        assert_eq!(unicode.maximum_length, 6);
        assert_eq!(unicode.buffer, name.as_ptr() as *mut u16);
    }

    #[test]
    fn spin_lock_is_constructible_without_kernel_calls() {
        let lock = SpinLock::new(0u32);
        assert_eq!(lock.lock.load(core::sync::atomic::Ordering::Relaxed), 0);
    }

    static TEST_OPS: [FltOperationRegistration; 1] = [FltOperationRegistration {
        major_function: crate::raw::IRP_MJ_OPERATION_END,
        flags: 0,
        pre_operation: None,
        post_operation: None,
        reserved1: core::ptr::null_mut(),
    }];

    unsafe extern "system" fn setup(
        _flt_objects: *mut core::ffi::c_void,
        _flags: u32,
        _device_type: u32,
        _filesystem_type: u32,
    ) -> NtStatus {
        STATUS_SUCCESS
    }

    #[test]
    fn registration_builders_set_km2_fields() {
        let registration = FilterRegistration::new(unload)
            .with_instance_setup(setup)
            .with_operations(TEST_OPS.as_ptr());
        let raw = unsafe { &*registration.as_ptr() };
        assert!(raw.instance_setup_callback.is_some());
        assert_eq!(raw.operation_registration, TEST_OPS.as_ptr());
    }
}
