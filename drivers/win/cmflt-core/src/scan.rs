//! Per-session scan-suppression quota.
//!
//! Backup/search/dedup scanners can walk a cloud tree and trigger thousands of
//! recalls. The engine and the future hydration path share this bounded quota:
//! a recall request must acquire bytes/request budget, and a completed or
//! failed request releases it. The quota is plain data so the policy is
//! host-tested before it is wired into KM-3.

pub const DEFAULT_MAX_REQUESTS: u32 = 256;
pub const DEFAULT_MAX_BYTES: u64 = 1 << 30;

/// Reservation returned by [`ScanQuota::try_acquire`]. Release it exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotaGrant {
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaError {
    RequestsExhausted,
    BytesExhausted,
}

/// Fixed-capacity per-session quota. No allocation, saturating counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanQuota {
    max_requests: u32,
    max_bytes: u64,
    requests: u32,
    bytes: u64,
    denied: u32,
}

impl ScanQuota {
    pub const fn new(max_requests: u32, max_bytes: u64) -> Self {
        Self {
            max_requests,
            max_bytes,
            requests: 0,
            bytes: 0,
            denied: 0,
        }
    }

    pub const fn defaults() -> Self {
        Self::new(DEFAULT_MAX_REQUESTS, DEFAULT_MAX_BYTES)
    }

    /// Reserve one request and `bytes` of hydration budget.
    ///
    /// A zero-byte request still consumes one request slot; this keeps the
    /// request-rate limit meaningful for metadata-only probes.
    pub fn try_acquire(&mut self, bytes: u64) -> Result<QuotaGrant, QuotaError> {
        if self.requests >= self.max_requests {
            self.denied = self.denied.saturating_add(1);
            return Err(QuotaError::RequestsExhausted);
        }
        let remaining = self.max_bytes.saturating_sub(self.bytes);
        if bytes > remaining {
            self.denied = self.denied.saturating_add(1);
            return Err(QuotaError::BytesExhausted);
        }
        self.requests = self.requests.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        Ok(QuotaGrant { bytes })
    }

    pub fn release(&mut self, grant: QuotaGrant) {
        self.requests = self.requests.saturating_sub(1);
        self.bytes = self.bytes.saturating_sub(grant.bytes);
    }

    pub fn reset(&mut self) {
        self.requests = 0;
        self.bytes = 0;
        self.denied = 0;
    }

    pub const fn requests(&self) -> u32 {
        self.requests
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn denied(&self) -> u32 {
        self.denied
    }

    pub const fn max_requests(&self) -> u32 {
        self.max_requests
    }

    pub const fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
}

impl Default for ScanQuota {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_limit_is_enforced_and_released() {
        let mut quota = ScanQuota::new(2, 100);
        let first = quota.try_acquire(10).expect("first");
        let second = quota.try_acquire(20).expect("second");
        assert_eq!(quota.requests(), 2);
        assert_eq!(quota.bytes(), 30);
        assert_eq!(quota.try_acquire(0), Err(QuotaError::RequestsExhausted));
        assert_eq!(quota.denied(), 1);

        quota.release(first);
        assert_eq!(quota.requests(), 1);
        assert_eq!(quota.bytes(), 20);
        assert!(quota.try_acquire(5).is_ok());
        quota.release(second);
        quota.release(QuotaGrant { bytes: 5 });
        assert_eq!(quota.requests(), 0);
        assert_eq!(quota.bytes(), 0);
    }

    #[test]
    fn byte_limit_is_enforced_without_partial_reservation() {
        let mut quota = ScanQuota::new(10, 100);
        let grant = quota.try_acquire(80).expect("first");
        assert_eq!(quota.try_acquire(21), Err(QuotaError::BytesExhausted));
        assert_eq!(quota.requests(), 1);
        assert_eq!(quota.bytes(), 80);
        assert_eq!(quota.denied(), 1);
        assert_eq!(quota.try_acquire(20), Ok(QuotaGrant { bytes: 20 }));
        quota.release(grant);
        assert_eq!(quota.bytes(), 20);
    }

    #[test]
    fn zero_byte_requests_consume_request_slots() {
        let mut quota = ScanQuota::new(1, 0);
        assert_eq!(quota.try_acquire(0), Ok(QuotaGrant { bytes: 0 }));
        assert_eq!(quota.try_acquire(0), Err(QuotaError::RequestsExhausted));
    }

    #[test]
    fn reset_clears_all_counters() {
        let mut quota = ScanQuota::defaults();
        quota.try_acquire(1024).expect("acquire");
        assert_eq!(
            quota.try_acquire(DEFAULT_MAX_BYTES),
            Err(QuotaError::BytesExhausted)
        );
        quota.reset();
        assert_eq!(quota.requests(), 0);
        assert_eq!(quota.bytes(), 0);
        assert_eq!(quota.denied(), 0);
    }

    #[test]
    fn saturating_release_never_underflows() {
        let mut quota = ScanQuota::new(1, 10);
        quota.release(QuotaGrant { bytes: 5 });
        assert_eq!(quota.requests(), 0);
        assert_eq!(quota.bytes(), 0);
    }
}
