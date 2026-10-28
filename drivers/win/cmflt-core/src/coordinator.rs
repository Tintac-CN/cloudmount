//! Pure KM-3 hydration coordinator.
//!
//! The kernel read/section path will own one coordinator per placeholder
//! stream. It combines:
//! - the placeholder's hydrated-range set and state,
//! - the cancel/timeout/switch state machine in [`crate::hydration`],
//! - the per-session [`ScanQuota`],
//! - stable transaction ids for the kernel↔engine protocol.
//!
//! Everything here is host-testable and allocation-free. The kernel executor
//! maps [`Action`] lists to FltMgr calls (queue, send, timer, switch, flush).

use crate::hydration::{step, Action, Event, State};
use crate::placeholder::{Placeholder, PlaceholderError};
use crate::range::ByteRange;
use crate::scan::{QuotaError, QuotaGrant, ScanQuota};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    /// The requested range is fully local; no recall is needed.
    Pass,
    /// The kernel must enqueue this transaction and send the hydrate request.
    Recall {
        tx_id: u64,
        range: ByteRange,
        deadline_ms: u64,
    },
    /// A transaction is already in flight for this stream.
    Busy { tx_id: u64 },
    /// The engine breaker is open; fail fast.
    Degraded,
    /// The scan-suppression quota rejected the request.
    Quota(QuotaError),
    /// The requested range is invalid for this placeholder.
    Invalid(PlaceholderError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinatorError {
    InvalidTransition,
    Placeholder(PlaceholderError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveTransaction {
    tx_id: u64,
    range: ByteRange,
    deadline_ms: u64,
    grant: QuotaGrant,
}

/// One in-flight hydration transaction per placeholder stream.
#[derive(Debug)]
pub struct HydrationCoordinator {
    placeholder: Placeholder,
    quota: ScanQuota,
    state: State,
    active: Option<ActiveTransaction>,
    next_tx: u64,
}

impl HydrationCoordinator {
    pub const fn new(placeholder: Placeholder, quota: ScanQuota) -> Self {
        Self {
            placeholder,
            quota,
            state: State::Idle,
            active: None,
            next_tx: 1,
        }
    }

    pub const fn placeholder(&self) -> &Placeholder {
        &self.placeholder
    }

    pub const fn state(&self) -> State {
        self.state
    }

    pub const fn active_tx(&self) -> Option<u64> {
        match self.active {
            Some(active) => Some(active.tx_id),
            None => None,
        }
    }

    pub const fn deadline_ms(&self) -> Option<u64> {
        match self.active {
            Some(active) => Some(active.deadline_ms),
            None => None,
        }
    }

    pub const fn quota(&self) -> &ScanQuota {
        &self.quota
    }

    /// Start a read/section recall. On `Recall`, the caller must enqueue the
    /// request and then call [`Self::on_dequeued`] when the worker runs.
    pub fn begin_read(&mut self, offset: u64, length: u64, deadline_ms: u64) -> ReadOutcome {
        match self.state {
            State::Hydrated | State::Canceled | State::Failed => {
                let _ = self.settle();
            }
            State::Idle => {}
            State::Degraded => return ReadOutcome::Degraded,
            _ => {
                return ReadOutcome::Busy {
                    tx_id: self.active.map(|active| active.tx_id).unwrap_or(0),
                }
            }
        }

        match self.placeholder.needs_recall(offset, length) {
            Ok(false) => return ReadOutcome::Pass,
            Err(error) => return ReadOutcome::Invalid(error),
            Ok(true) => {}
        }

        let grant = match self.quota.try_acquire(length) {
            Ok(grant) => grant,
            Err(error) => return ReadOutcome::Quota(error),
        };
        let tx_id = self.next_tx.max(1);
        self.next_tx = if tx_id == u64::MAX { 1 } else { tx_id + 1 };
        let range = ByteRange::new(offset, length);
        self.active = Some(ActiveTransaction {
            tx_id,
            range,
            deadline_ms,
            grant,
        });

        let transition = match step(self.state, Event::DataAccess) {
            Some(transition) => transition,
            None => {
                self.finish();
                return ReadOutcome::Degraded;
            }
        };
        self.state = transition.next;
        ReadOutcome::Recall {
            tx_id,
            range,
            deadline_ms,
        }
    }

    pub fn on_dequeued(&mut self) -> Result<&'static [Action], CoordinatorError> {
        self.transition(State::Queued, Event::Dequeued)
    }

    pub fn on_reply_accepted(&mut self) -> Result<&'static [Action], CoordinatorError> {
        self.transition(State::Waiting, Event::ReplyAccepted)
    }

    pub fn on_reply_rejected(&mut self) -> Result<&'static [Action], CoordinatorError> {
        self.transition(State::Waiting, Event::ReplyRejected)
    }

    pub fn on_verify_ok(&mut self) -> Result<&'static [Action], CoordinatorError> {
        self.transition(State::Verifying, Event::VerifyOk)
    }

    pub fn on_verify_failed(&mut self) -> Result<&'static [Action], CoordinatorError> {
        self.transition(State::Verifying, Event::VerifyFailed)
    }

    /// Switch completed. The range is marked hydrated before the terminal
    /// state is recorded, even when the original I/O was cancelled/timed out.
    pub fn on_switch_ok(&mut self) -> Result<&'static [Action], CoordinatorError> {
        if !matches!(self.state, State::Switching | State::Draining(_)) {
            return Err(CoordinatorError::InvalidTransition);
        }
        let active = self.active.ok_or(CoordinatorError::InvalidTransition)?;
        if let Err(error) = self.placeholder.mark_hydrated(active.range) {
            let _ = step(self.state, Event::SwitchFailed).map(|transition| {
                self.state = transition.next;
            });
            self.finish();
            return Err(CoordinatorError::Placeholder(error));
        }
        let transition =
            step(self.state, Event::SwitchOk).ok_or(CoordinatorError::InvalidTransition)?;
        self.state = transition.next;
        self.finish();
        Ok(transition.actions)
    }

    pub fn on_switch_failed(&mut self) -> Result<&'static [Action], CoordinatorError> {
        if !matches!(self.state, State::Switching | State::Draining(_)) {
            return Err(CoordinatorError::InvalidTransition);
        }
        let transition =
            step(self.state, Event::SwitchFailed).ok_or(CoordinatorError::InvalidTransition)?;
        self.state = transition.next;
        self.finish();
        Ok(transition.actions)
    }

    pub fn on_cancel(&mut self) -> Result<&'static [Action], CoordinatorError> {
        match self.state {
            State::Queued | State::Waiting | State::Verifying => {
                let transition = self.transition_inner(Event::Cancel)?;
                self.finish();
                Ok(transition)
            }
            State::Switching => {
                let transition = self.transition_inner(Event::Cancel)?;
                // Keep the quota reservation until the in-flight switch ends.
                Ok(transition)
            }
            State::Draining(_) => Ok(&[]),
            _ => Err(CoordinatorError::InvalidTransition),
        }
    }

    pub fn on_timeout(&mut self) -> Result<&'static [Action], CoordinatorError> {
        match self.state {
            State::Queued | State::Waiting | State::Verifying => {
                let transition = self.transition_inner(Event::Timeout)?;
                self.finish();
                Ok(transition)
            }
            State::Switching => {
                let transition = self.transition_inner(Event::Timeout)?;
                // Drain the switch; the reservation is released on switch end.
                Ok(transition)
            }
            State::Draining(_) => {
                let transition = self.transition_inner(Event::Timeout)?;
                self.finish();
                Ok(transition)
            }
            _ => Err(CoordinatorError::InvalidTransition),
        }
    }

    /// Teardown: drain any in-flight transaction. A switch in progress stays
    /// in `Draining` until its completion event.
    pub fn on_reset(&mut self) -> Result<&'static [Action], CoordinatorError> {
        let transition =
            step(self.state, Event::Reset).ok_or(CoordinatorError::InvalidTransition)?;
        self.state = transition.next;
        if !matches!(self.state, State::Draining(_)) {
            self.finish();
        }
        Ok(transition.actions)
    }

    /// Move a completed transaction back to Idle so the next recall can start.
    pub fn settle(&mut self) -> Result<(), CoordinatorError> {
        match self.state {
            State::Idle | State::Degraded => Ok(()),
            State::Hydrated | State::Canceled | State::Failed => {
                let transition =
                    step(self.state, Event::Settled).ok_or(CoordinatorError::InvalidTransition)?;
                self.state = transition.next;
                self.finish();
                Ok(())
            }
            _ => Err(CoordinatorError::InvalidTransition),
        }
    }

    /// Engine recovered from the breaker state.
    pub fn on_engine_healthy(&mut self) -> Result<(), CoordinatorError> {
        if self.state != State::Degraded {
            return Err(CoordinatorError::InvalidTransition);
        }
        let transition =
            step(self.state, Event::EngineHealthy).ok_or(CoordinatorError::InvalidTransition)?;
        self.state = transition.next;
        Ok(())
    }

    fn transition(
        &mut self,
        expected: State,
        event: Event,
    ) -> Result<&'static [Action], CoordinatorError> {
        if self.state != expected {
            return Err(CoordinatorError::InvalidTransition);
        }
        let actions = self.transition_inner(event)?;
        if matches!(self.state, State::Canceled | State::Failed) {
            self.finish();
        }
        Ok(actions)
    }

    fn transition_inner(&mut self, event: Event) -> Result<&'static [Action], CoordinatorError> {
        let transition = step(self.state, event).ok_or(CoordinatorError::InvalidTransition)?;
        self.state = transition.next;
        Ok(transition.actions)
    }

    fn finish(&mut self) {
        if let Some(active) = self.active.take() {
            self.quota.release(active.grant);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placeholder::Placeholder;
    use crate::range::ByteRange;

    fn coordinator() -> HydrationCoordinator {
        HydrationCoordinator::new(Placeholder::new(7, 4096), ScanQuota::new(8, 4096))
    }

    fn complete_flow(coordinator: &mut HydrationCoordinator) {
        coordinator.on_dequeued().expect("dequeue");
        coordinator.on_reply_accepted().expect("reply");
        coordinator.on_verify_ok().expect("verify");
        coordinator.on_switch_ok().expect("switch");
    }

    #[test]
    fn full_hydration_flow_marks_the_whole_file() {
        let mut manager = coordinator();
        let outcome = manager.begin_read(0, 4096, 1_000);
        assert!(matches!(outcome, ReadOutcome::Recall { tx_id: 1, .. }));
        assert_eq!(manager.state(), State::Queued);
        assert_eq!(manager.quota().requests(), 1);

        complete_flow(&mut manager);
        assert_eq!(manager.state(), State::Hydrated);
        assert_eq!(
            manager.placeholder().meta().state,
            crate::placeholder::PlaceholderState::Full
        );
        assert_eq!(manager.quota().requests(), 0);
        assert_eq!(manager.quota().bytes(), 0);
        assert_eq!(manager.begin_read(0, 1, 2_000), ReadOutcome::Pass);
    }

    #[test]
    fn partial_hydration_can_recall_the_remaining_range() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(0, 1024, 1_000),
            ReadOutcome::Recall { tx_id: 1, .. }
        ));
        complete_flow(&mut manager);
        assert_eq!(
            manager.placeholder().meta().state,
            crate::placeholder::PlaceholderState::Partial
        );
        assert_eq!(manager.begin_read(0, 1024, 2_000), ReadOutcome::Pass);
        assert!(matches!(
            manager.begin_read(1024, 1024, 2_000),
            ReadOutcome::Recall { tx_id: 2, .. }
        ));
        assert_eq!(manager.state(), State::Queued);
    }

    #[test]
    fn active_transaction_reports_busy() {
        let mut manager = coordinator();
        let first = manager.begin_read(0, 512, 1_000);
        let tx_id = match first {
            ReadOutcome::Recall { tx_id, .. } => tx_id,
            other => panic!("expected recall, got {other:?}"),
        };
        assert_eq!(
            manager.begin_read(512, 512, 1_000),
            ReadOutcome::Busy { tx_id }
        );
    }

    #[test]
    fn cancel_releases_quota_and_allows_retry() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(0, 512, 1_000),
            ReadOutcome::Recall { .. }
        ));
        assert_eq!(manager.quota().requests(), 1);
        manager.on_cancel().expect("cancel");
        assert_eq!(manager.state(), State::Canceled);
        assert_eq!(manager.quota().requests(), 0);
        assert!(matches!(
            manager.begin_read(0, 512, 2_000),
            ReadOutcome::Recall { tx_id: 2, .. }
        ));
    }

    #[test]
    fn timeout_opens_breaker_and_fast_fails() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(0, 512, 1_000),
            ReadOutcome::Recall { .. }
        ));
        manager.on_dequeued().expect("dequeue");
        manager.on_timeout().expect("timeout");
        assert_eq!(manager.state(), State::Degraded);
        assert_eq!(manager.quota().requests(), 0);
        assert_eq!(manager.begin_read(0, 512, 2_000), ReadOutcome::Degraded);
        manager.on_engine_healthy().expect("recover");
        assert_eq!(manager.state(), State::Idle);
    }

    #[test]
    fn switch_failure_releases_quota_and_allows_retry() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(0, 512, 1_000),
            ReadOutcome::Recall { .. }
        ));
        manager.on_dequeued().expect("dequeue");
        manager.on_reply_accepted().expect("reply");
        manager.on_verify_ok().expect("verify");
        manager.on_switch_failed().expect("switch failure");
        assert_eq!(manager.state(), State::Failed);
        assert_eq!(manager.quota().requests(), 0);
        assert!(matches!(
            manager.begin_read(0, 512, 2_000),
            ReadOutcome::Recall { tx_id: 2, .. }
        ));
    }

    #[test]
    fn cancel_during_switch_keeps_quota_until_switch_completes() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(0, 512, 1_000),
            ReadOutcome::Recall { .. }
        ));
        manager.on_dequeued().expect("dequeue");
        manager.on_reply_accepted().expect("reply");
        manager.on_verify_ok().expect("verify");
        manager.on_cancel().expect("cancel");
        assert!(matches!(manager.state(), State::Draining(_)));
        assert_eq!(manager.quota().requests(), 1);
        manager.on_switch_ok().expect("switch");
        assert_eq!(manager.state(), State::Canceled);
        assert_eq!(manager.quota().requests(), 0);
        assert_eq!(
            manager.placeholder().meta().state,
            crate::placeholder::PlaceholderState::Partial
        );
    }

    #[test]
    fn quota_exhaustion_is_reported_without_starting_a_transaction() {
        let mut manager =
            HydrationCoordinator::new(Placeholder::new(7, 4096), ScanQuota::new(1, 0));
        assert_eq!(
            manager.begin_read(0, 1, 1_000),
            ReadOutcome::Quota(QuotaError::BytesExhausted)
        );
        assert_eq!(manager.state(), State::Idle);
        assert_eq!(manager.quota().requests(), 0);

        let mut requests =
            HydrationCoordinator::new(Placeholder::new(7, 4096), ScanQuota::new(0, 4096));
        assert_eq!(
            requests.begin_read(0, 1, 1_000),
            ReadOutcome::Quota(QuotaError::RequestsExhausted)
        );
        assert_eq!(requests.state(), State::Idle);
    }

    #[test]
    fn invalid_range_is_rejected_without_quota_use() {
        let mut manager = coordinator();
        assert_eq!(
            manager.begin_read(4096, 1, 1_000),
            ReadOutcome::Invalid(PlaceholderError::OutOfBounds)
        );
        assert_eq!(manager.quota().requests(), 0);
    }

    #[test]
    fn reset_drains_and_releases() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(0, 512, 1_000),
            ReadOutcome::Recall { .. }
        ));
        manager.on_reset().expect("reset");
        assert_eq!(manager.state(), State::Canceled);
        assert_eq!(manager.quota().requests(), 0);
        manager.settle().expect("settle");
        assert_eq!(manager.state(), State::Idle);
    }

    #[test]
    fn switch_completion_marks_the_requested_range_only() {
        let mut manager = coordinator();
        assert!(matches!(
            manager.begin_read(1024, 1024, 1_000),
            ReadOutcome::Recall { .. }
        ));
        complete_flow(&mut manager);
        assert_eq!(
            manager.placeholder().ranges().as_slice(),
            &[ByteRange::new(1024, 1024)]
        );
        assert_eq!(manager.placeholder().needs_recall(0, 1024), Ok(true));
        assert_eq!(manager.placeholder().needs_recall(1024, 1024), Ok(false));
    }
}
