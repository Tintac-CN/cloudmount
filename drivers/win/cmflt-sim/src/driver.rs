//! Host-side IRP lifecycle simulator.
//!
//! This models the kernel read path before FltMgr is involved: a pending read
//! is enqueued, the engine reply is stored, and the request is completed,
//! cancelled or timed out exactly once. The real driver maps these operations
//! to Cbdq + FltSendMessage; the simulator catches state-machine/quota bugs
//! without a testsigned kernel.

use cmflt_core::coordinator::{HydrationCoordinator, ReadOutcome};
use cmflt_core::hydration::{Action, State};
use cmflt_core::placeholder::{Placeholder, PlaceholderError};
use cmflt_core::protocol::{HydrateReply, HydrateRequest, STATUS_SUCCESS};
use cmflt_core::scan::{QuotaError, ScanQuota};

use crate::engine::MockEngine;

const DEADLINE_MS: u64 = 15_000;
const REQUESTOR_PID: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    Busy { tx_id: u64 },
    Degraded,
    Quota(QuotaError),
    Invalid(PlaceholderError),
    BadLength,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadResult {
    Pass,
    Pending { tx_id: u64 },
    Failed(ReadFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Success {
        tx_id: u64,
        offset: u64,
        length: u64,
    },
    Cancelled {
        tx_id: u64,
    },
    TimedOut {
        tx_id: u64,
    },
    Failed {
        tx_id: u64,
        status: i32,
    },
    LocalFailure {
        tx_id: u64,
    },
}

#[derive(Clone, Copy, Debug)]
struct PendingRead {
    tx_id: u64,
    offset: u64,
    length: u64,
    reply: HydrateReply,
}

pub struct SimDriver {
    coordinator: HydrationCoordinator,
    engine: MockEngine,
    pending: Vec<PendingRead>,
    completed: Vec<Completion>,
}

impl SimDriver {
    pub fn new(file_id: u64, size: u64, engine: MockEngine) -> Self {
        Self {
            coordinator: HydrationCoordinator::new(
                Placeholder::new(file_id, size),
                ScanQuota::defaults(),
            ),
            engine,
            pending: Vec::new(),
            completed: Vec::new(),
        }
    }

    pub const fn coordinator(&self) -> &HydrationCoordinator {
        &self.coordinator
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn completed(&self) -> &[Completion] {
        &self.completed
    }

    /// Submit a read. A `Pending` result means Cbdq/IRP would be held until
    /// `complete`, `cancel` or `timeout` is called.
    pub fn read(&mut self, offset: u64, length: u64) -> ReadResult {
        let file_id = self.coordinator.placeholder().meta().file_id;
        let outcome = self.coordinator.begin_read(offset, length, DEADLINE_MS);
        let (tx_id, range) = match outcome {
            ReadOutcome::Pass => return ReadResult::Pass,
            ReadOutcome::Busy { tx_id } => return ReadResult::Failed(ReadFailure::Busy { tx_id }),
            ReadOutcome::Degraded => return ReadResult::Failed(ReadFailure::Degraded),
            ReadOutcome::Quota(error) => return ReadResult::Failed(ReadFailure::Quota(error)),
            ReadOutcome::Invalid(error) => return ReadResult::Failed(ReadFailure::Invalid(error)),
            ReadOutcome::Recall { tx_id, range, .. } => (tx_id, range),
        };

        let actions = match self.coordinator.on_dequeued() {
            Ok(actions) => actions,
            Err(_) => return ReadResult::Failed(ReadFailure::Degraded),
        };
        if !actions.contains(&Action::SendHydrate) {
            return ReadResult::Failed(ReadFailure::Degraded);
        }

        let length_u32 = match u32::try_from(range.length) {
            Ok(length) => length,
            Err(_) => {
                let _ = self.coordinator.on_reset();
                return ReadResult::Failed(ReadFailure::BadLength);
            }
        };
        let request = HydrateRequest {
            tx_id,
            file_id,
            offset: range.offset,
            length: length_u32,
            requestor_pid: REQUESTOR_PID,
            deadline_ms: DEADLINE_MS as u32,
            flags: 0,
        };
        let mut input = [0u8; 128];
        let request_len = match request.encode(&mut input) {
            Ok(length) => length,
            Err(_) => {
                let _ = self.coordinator.on_reset();
                return ReadResult::Failed(ReadFailure::BadLength);
            }
        };
        let mut output = [0u8; 128];
        let reply_len = match self.engine.handle(&input[..request_len], &mut output) {
            Ok(length) => length,
            Err(_) => {
                let _ = self.coordinator.on_reset();
                return ReadResult::Failed(ReadFailure::Degraded);
            }
        };
        let reply = match HydrateReply::decode(&output[..reply_len]) {
            Ok(reply) => reply,
            Err(_) => {
                let _ = self.coordinator.on_reset();
                return ReadResult::Failed(ReadFailure::Degraded);
            }
        };
        self.pending.push(PendingRead {
            tx_id,
            offset: range.offset,
            length: range.length,
            reply,
        });
        ReadResult::Pending { tx_id }
    }

    /// Engine reply is available: verify it, mark the range hydrated and
    /// complete the read.
    pub fn complete(&mut self, tx_id: u64) -> Option<Completion> {
        let index = self
            .pending
            .iter()
            .position(|pending| pending.tx_id == tx_id)?;
        let pending = self.pending.remove(index);
        if pending.reply.status != STATUS_SUCCESS {
            let _ = self.coordinator.on_reply_rejected();
            let completion = Completion::Failed {
                tx_id,
                status: pending.reply.status,
            };
            self.completed.push(completion);
            return Some(completion);
        }
        if self.coordinator.on_reply_accepted().is_err()
            || self.coordinator.on_verify_ok().is_err()
            || self.coordinator.on_switch_ok().is_err()
        {
            let completion = Completion::LocalFailure { tx_id };
            self.completed.push(completion);
            return Some(completion);
        }
        let completion = Completion::Success {
            tx_id,
            offset: pending.offset,
            length: pending.length,
        };
        self.completed.push(completion);
        Some(completion)
    }

    pub fn cancel(&mut self, tx_id: u64) -> Option<Completion> {
        let index = self
            .pending
            .iter()
            .position(|pending| pending.tx_id == tx_id)?;
        let _pending = self.pending.remove(index);
        let _ = self.coordinator.on_cancel();
        if matches!(self.coordinator.state(), State::Draining(_)) {
            let _ = self.coordinator.on_switch_ok();
        }
        let completion = Completion::Cancelled { tx_id };
        self.completed.push(completion);
        Some(completion)
    }

    pub fn timeout(&mut self, tx_id: u64) -> Option<Completion> {
        let index = self
            .pending
            .iter()
            .position(|pending| pending.tx_id == tx_id)?;
        let _pending = self.pending.remove(index);
        let _ = self.coordinator.on_timeout();
        if matches!(self.coordinator.state(), State::Draining(_)) {
            let _ = self.coordinator.on_switch_ok();
        }
        let completion = Completion::TimedOut { tx_id };
        self.completed.push(completion);
        Some(completion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(file_id: u64, size: usize) -> MockEngine {
        let mut engine = MockEngine::new();
        engine.add_file(file_id, vec![0xAB; size]);
        engine
    }

    #[test]
    fn pending_read_completes_and_next_read_passes() {
        let mut driver = SimDriver::new(7, 4096, engine(7, 4096));
        let tx_id = match driver.read(0, 1024) {
            ReadResult::Pending { tx_id } => tx_id,
            other => panic!("expected pending, got {other:?}"),
        };
        assert_eq!(driver.pending_len(), 1);
        assert_eq!(
            driver.complete(tx_id),
            Some(Completion::Success {
                tx_id,
                offset: 0,
                length: 1024
            })
        );
        assert_eq!(driver.read(0, 1024), ReadResult::Pass);
        assert_eq!(driver.coordinator().quota().requests(), 0);
    }

    #[test]
    fn cancel_completes_once_and_releases_quota() {
        let mut driver = SimDriver::new(7, 4096, engine(7, 4096));
        let tx_id = match driver.read(0, 1024) {
            ReadResult::Pending { tx_id } => tx_id,
            other => panic!("expected pending, got {other:?}"),
        };
        assert_eq!(driver.cancel(tx_id), Some(Completion::Cancelled { tx_id }));
        assert_eq!(driver.cancel(tx_id), None);
        assert_eq!(driver.complete(tx_id), None);
        assert_eq!(driver.coordinator().quota().requests(), 0);
        assert_eq!(driver.completed().len(), 1);
    }

    #[test]
    fn timeout_opens_breaker_and_rejects_the_next_read() {
        let mut driver = SimDriver::new(7, 4096, engine(7, 4096));
        let tx_id = match driver.read(0, 1024) {
            ReadResult::Pending { tx_id } => tx_id,
            other => panic!("expected pending, got {other:?}"),
        };
        assert_eq!(driver.timeout(tx_id), Some(Completion::TimedOut { tx_id }));
        assert_eq!(
            driver.read(0, 1024),
            ReadResult::Failed(ReadFailure::Degraded)
        );
        assert_eq!(driver.coordinator().quota().requests(), 0);
    }

    #[test]
    fn engine_not_found_fails_the_read() {
        let mut driver = SimDriver::new(7, 4096, MockEngine::new());
        let tx_id = match driver.read(0, 1024) {
            ReadResult::Pending { tx_id } => tx_id,
            other => panic!("expected pending, got {other:?}"),
        };
        assert_eq!(
            driver.complete(tx_id),
            Some(Completion::Failed {
                tx_id,
                status: crate::engine::STATUS_OBJECT_NAME_NOT_FOUND
            })
        );
        assert_eq!(driver.coordinator().quota().requests(), 0);
        assert!(matches!(driver.read(0, 1024), ReadResult::Pending { .. }));
    }

    #[test]
    fn second_read_is_busy_while_pending() {
        let mut driver = SimDriver::new(7, 4096, engine(7, 4096));
        let tx_id = match driver.read(0, 1024) {
            ReadResult::Pending { tx_id } => tx_id,
            other => panic!("expected pending, got {other:?}"),
        };
        assert_eq!(
            driver.read(0, 1024),
            ReadResult::Failed(ReadFailure::Busy { tx_id })
        );
    }

    #[test]
    fn every_completion_is_recorded_exactly_once() {
        let mut driver = SimDriver::new(7, 4096, engine(7, 4096));
        let mut tx_ids = Vec::new();
        for _ in 0..8 {
            if let ReadResult::Pending { tx_id } = driver.read(0, 512) {
                tx_ids.push(tx_id);
            }
        }
        assert_eq!(tx_ids.len(), 1);
        let tx = tx_ids[0];
        driver.complete(tx);
        driver.complete(tx);
        driver.cancel(tx);
        assert_eq!(driver.completed().len(), 1);
        assert_eq!(driver.pending_len(), 0);
    }
}
