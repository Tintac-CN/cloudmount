//! Deterministic user-mode simulator for the kernel hydration state machine.
//!
//! The simulator deliberately drives the same `cmflt_core::hydration::step`
//! table the driver uses. It checks the transaction-level invariants that are
//! expensive to reproduce with a real kernel:
//!
//! - a transaction emits exactly one `CompleteIo`;
//! - no completion is emitted without an active transaction;
//! - a second transaction cannot start before the first completes;
//! - every random walk can be drained to a settled state.
//!
//! It also carries a small xorshift PRNG so no external crate is needed.

pub mod driver;
pub mod engine;

use cmflt_core::hydration::{step, Action, Event, IoStatus, State};

/// Deterministic xorshift64* generator.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, upper: u32) -> u32 {
        if upper == 0 {
            0
        } else {
            (self.next_u64() % upper as u64) as u32
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub transitions: u64,
    pub ignored: u64,
    pub started: u64,
    pub completions: u64,
    pub completed_success: u64,
    pub completed_cancelled: u64,
    pub completed_timeout: u64,
    pub completed_error: u64,
    pub fast_fails: u64,
    pub breaker_trips: u64,
    pub last_completion: Option<IoStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimError {
    DoubleCompletion,
    CompletionWithoutTransaction,
    TransactionAlreadyActive,
    Stuck,
}

impl core::fmt::Display for SimError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DoubleCompletion => formatter.write_str("more than one completion emitted"),
            Self::CompletionWithoutTransaction => {
                formatter.write_str("completion emitted without an active transaction")
            }
            Self::TransactionAlreadyActive => {
                formatter.write_str("a second transaction started before the first completed")
            }
            Self::Stuck => formatter.write_str("state machine could not be drained"),
        }
    }
}

impl std::error::Error for SimError {}

pub struct Sim {
    pub state: State,
    rng: Rng,
    tx_active: bool,
    pub stats: Stats,
}

impl Sim {
    pub fn new(seed: u64) -> Self {
        Self {
            state: State::Idle,
            rng: Rng::new(seed),
            tx_active: false,
            stats: Stats::default(),
        }
    }

    pub fn apply(&mut self, event: Event) -> Result<(), SimError> {
        let Some(transition) = step(self.state, event) else {
            self.stats.ignored = self.stats.ignored.saturating_add(1);
            return Ok(());
        };

        let completions = transition
            .actions
            .iter()
            .filter(|action| matches!(action, Action::CompleteIo(_)))
            .count();
        if completions > 1 {
            return Err(SimError::DoubleCompletion);
        }
        if completions == 1 {
            if !self.tx_active {
                return Err(SimError::CompletionWithoutTransaction);
            }
            self.tx_active = false;
            self.stats.completions = self.stats.completions.saturating_add(1);
            if let Some(status) = transition.actions.iter().find_map(|action| match action {
                Action::CompleteIo(status) => Some(*status),
                _ => None,
            }) {
                self.stats.last_completion = Some(status);
                match status {
                    IoStatus::Success => {
                        self.stats.completed_success =
                            self.stats.completed_success.saturating_add(1)
                    }
                    IoStatus::Cancelled => {
                        self.stats.completed_cancelled =
                            self.stats.completed_cancelled.saturating_add(1)
                    }
                    IoStatus::TimedOut => {
                        self.stats.completed_timeout =
                            self.stats.completed_timeout.saturating_add(1)
                    }
                    IoStatus::ProviderNotRunning
                    | IoStatus::DataError
                    | IoStatus::InternalError => {
                        self.stats.completed_error = self.stats.completed_error.saturating_add(1)
                    }
                }
            }
        }

        for action in transition.actions {
            match action {
                Action::Enqueue => {
                    if self.tx_active {
                        return Err(SimError::TransactionAlreadyActive);
                    }
                    self.tx_active = true;
                    self.stats.started = self.stats.started.saturating_add(1);
                }
                Action::FailFast(_) => {
                    self.stats.fast_fails = self.stats.fast_fails.saturating_add(1)
                }
                Action::TripBreaker => {
                    self.stats.breaker_trips = self.stats.breaker_trips.saturating_add(1)
                }
                _ => {}
            }
        }

        self.state = transition.next;
        self.stats.transitions = self.stats.transitions.saturating_add(1);
        Ok(())
    }

    pub fn run_random(&mut self, steps: usize) -> Result<(), SimError> {
        for _ in 0..steps {
            let event = self.next_event();
            self.apply(event)?;
        }
        Ok(())
    }

    /// Drive the current transaction to a settled state using legal events.
    pub fn drain(&mut self) -> Result<(), SimError> {
        for _ in 0..64 {
            if !self.tx_active
                && matches!(self.state, State::Idle | State::Hydrated | State::Degraded)
            {
                return Ok(());
            }
            let event = match self.state {
                State::Queued => Event::Cancel,
                State::Waiting => Event::Timeout,
                State::Verifying => Event::Cancel,
                State::Switching => Event::Cancel,
                State::Draining(_) => Event::SwitchOk,
                State::Canceled | State::Failed => Event::Reset,
                State::Idle | State::Hydrated | State::Degraded => {
                    if self.tx_active {
                        return Err(SimError::Stuck);
                    }
                    return Ok(());
                }
            };
            self.apply(event)?;
        }
        Err(SimError::Stuck)
    }

    fn next_event(&mut self) -> Event {
        let choices: &[Event] = match self.state {
            State::Idle => &[
                Event::DataAccess,
                Event::DataAccess,
                Event::DataAccess,
                Event::EngineUnavailable,
                Event::Reset,
            ],
            State::Queued => &[
                Event::Dequeued,
                Event::Dequeued,
                Event::Dequeued,
                Event::Cancel,
                Event::Timeout,
                Event::EngineUnavailable,
                Event::Reset,
            ],
            State::Waiting => &[
                Event::ReplyAccepted,
                Event::ReplyAccepted,
                Event::ReplyAccepted,
                Event::ReplyRejected,
                Event::Cancel,
                Event::Timeout,
                Event::EngineUnavailable,
                Event::Reset,
            ],
            State::Verifying => &[
                Event::VerifyOk,
                Event::VerifyOk,
                Event::VerifyOk,
                Event::VerifyFailed,
                Event::Cancel,
                Event::Timeout,
                Event::Reset,
            ],
            State::Switching => &[
                Event::SwitchOk,
                Event::SwitchOk,
                Event::SwitchOk,
                Event::SwitchFailed,
                Event::Cancel,
                Event::Timeout,
                Event::Reset,
            ],
            State::Draining(_) => &[
                Event::SwitchOk,
                Event::SwitchOk,
                Event::SwitchOk,
                Event::SwitchFailed,
                Event::Timeout,
            ],
            State::Hydrated => &[
                Event::Dehydrate,
                Event::Dehydrate,
                Event::DataAccess,
                Event::EngineUnavailable,
                Event::Reset,
            ],
            State::Canceled | State::Failed => &[Event::Reset],
            State::Degraded => &[
                Event::EngineHealthy,
                Event::EngineHealthy,
                Event::DataAccess,
                Event::DataAccess,
                Event::EngineUnavailable,
                Event::Reset,
            ],
        };
        let index = self.rng.below(choices.len() as u32) as usize;
        choices.get(index).copied().unwrap_or(Event::Reset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmflt_core::range::{ByteRange, RangeSet};

    #[test]
    fn random_walks_preserve_transaction_invariants() {
        for seed in 1..=64 {
            let mut sim = Sim::new(seed);
            sim.run_random(20_000).expect("random walk");
            sim.drain().expect("drain");
            assert_eq!(
                sim.stats.completions, sim.stats.started,
                "seed {seed}: every started transaction must complete exactly once"
            );
        }
    }

    #[test]
    fn cancel_during_switch_completes_after_the_switch() {
        let mut sim = Sim::new(11);
        sim.apply(Event::DataAccess).expect("start");
        sim.apply(Event::Dequeued).expect("dequeue");
        sim.apply(Event::ReplyAccepted).expect("reply");
        sim.apply(Event::VerifyOk).expect("verify");
        assert_eq!(sim.state, State::Switching);
        sim.apply(Event::Cancel).expect("cancel");
        assert_eq!(sim.state, State::Draining(IoStatus::Cancelled));
        assert_eq!(sim.stats.completions, 0, "must not complete before switch");
        sim.apply(Event::SwitchOk).expect("switch completion");
        assert_eq!(sim.state, State::Canceled);
        assert_eq!(sim.stats.completions, 1);
        assert_eq!(sim.stats.completed_cancelled, 1);
    }

    #[test]
    fn timeout_trips_breaker_and_fails_the_original_io() {
        let mut sim = Sim::new(12);
        sim.apply(Event::DataAccess).expect("start");
        sim.apply(Event::Dequeued).expect("dequeue");
        sim.apply(Event::Timeout).expect("timeout");
        assert_eq!(sim.state, State::Degraded);
        assert_eq!(sim.stats.completions, 1);
        assert_eq!(sim.stats.completed_timeout, 1);
        assert!(sim.stats.breaker_trips >= 1);
        sim.apply(Event::DataAccess).expect("fast fail");
        assert_eq!(sim.stats.fast_fails, 1);
        sim.apply(Event::EngineHealthy).expect("recover");
        assert_eq!(sim.state, State::Idle);
    }

    #[test]
    fn range_set_matches_bruteforce() {
        let mut rng = Rng::new(7);
        let mut set = RangeSet::new();
        let mut reference = [false; 4096];

        for _ in 0..200 {
            let offset = rng.below(4096) as u64;
            let length = rng.below(64) as u64;
            let range = ByteRange::new(offset, length);
            if set.insert(range).is_ok() {
                let end = offset.saturating_add(length).min(4096);
                for byte in offset..end {
                    if let Some(slot) = reference.get_mut(byte as usize) {
                        *slot = true;
                    }
                }
            }
        }

        for _ in 0..1000 {
            let offset = rng.below(4096) as u64;
            let available = 4096u64 - offset;
            let length = (rng.below(128) as u64).min(available);
            let expected = (offset..offset + length)
                .all(|byte| reference.get(byte as usize).copied().unwrap_or(false));
            assert_eq!(set.covers(offset, length), expected);
        }
    }

    #[test]
    fn rng_is_deterministic_and_nonzero() {
        let mut a = Rng::new(99);
        let mut b = Rng::new(99);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut zero = Rng::new(0);
        assert_ne!(zero.next_u64(), 0);
    }
}
