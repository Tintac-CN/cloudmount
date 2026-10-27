//! Small safe layer over the raw FltMgr registration calls.
//!
//! The layer is intentionally narrow: it owns the registration lifetime and
//! turns NTSTATUS into `Result`. It does not hide IRQL or locking rules for
//! I/O callbacks yet; those wrappers arrive with the callbacks that need them.

use core::ptr::{null, null_mut};

use crate::raw::{
    FilterUnloadCallback, FltRegisterFilter, FltRegistration, FltStartFiltering,
    FltUnregisterFilter, NtStatus, PdriverObject, PfltFilter, FLT_REGISTRATION_VERSION,
    STATUS_SUCCESS, STATUS_UNSUCCESSFUL,
};

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
}
