//! Pure hydration state machine.
//!
//! The module has no clock, no I/O and no heap. A caller feeds an [`Event`]
//! and receives the next [`State`] plus a static list of [`Action`]s to
//! execute. The kernel driver executes them with FltMgr; the host simulator
//! records them and checks invariants.
//!
//! The table is deliberately total and small. Events that are not meaningful
//! in a state return `None` and must be ignored by the caller; they never
//! mutate state.
//!
//! Key invariant: a transaction that has started emits exactly one
//! [`Action::CompleteIo`]. A cancel or timeout during `Switching` is deferred
//! into `Draining`, because `FsRtlChangeBackingFileObject` cannot be aborted
//! once started.

/// Completion status handed back to the original I/O.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoStatus {
    Success,
    Cancelled,
    TimedOut,
    ProviderNotRunning,
    DataError,
    InternalError,
}

/// Observable hydration state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No hydration transaction is active.
    Idle,
    /// Request is waiting in the cancel-safe queue.
    Queued,
    /// Request was sent to the user-mode engine and has a deadline.
    Waiting,
    /// Engine replied; the reply is being validated.
    Verifying,
    /// `FsRtlChangeBackingFileObject` is in progress.
    Switching,
    /// Switch is in progress and the original request already has a pending
    /// completion status (cancel or timeout).
    Draining(IoStatus),
    /// Data is fully present and the request succeeded.
    Hydrated,
    /// A transaction ended because the caller cancelled it.
    Canceled,
    /// A transaction ended in a definite error.
    Failed,
    /// The engine is considered unavailable; new recalls fail fast.
    Degraded,
}

impl State {
    /// States in which a transaction has not yet reached a terminal outcome.
    pub const fn is_waiting(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Waiting | Self::Verifying | Self::Switching | Self::Draining(_)
        )
    }

    /// Transaction terminal states. `Hydrated` and `Degraded` are settled but
    /// can still receive new events.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Canceled | Self::Failed)
    }

    /// The breaker is open and new recalls fail fast.
    pub const fn is_degraded(self) -> bool {
        matches!(self, Self::Degraded)
    }
}

/// Inputs to the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A read or section-sync request hit unhydrated data.
    DataAccess,
    /// The cancel-safe queue handed the request to the worker.
    Dequeued,
    /// The original caller cancelled.
    Cancel,
    /// The request deadline expired.
    Timeout,
    /// The engine replied with a syntactically valid result.
    ReplyAccepted,
    /// The engine replied with an error, or the reply was rejected.
    ReplyRejected,
    /// Reply validation passed.
    VerifyOk,
    /// Reply validation failed (length, hash, range, identity).
    VerifyFailed,
    /// The backing-object switch completed.
    SwitchOk,
    /// The backing-object switch failed.
    SwitchFailed,
    /// The file should return to placeholder state.
    Dehydrate,
    /// The user-mode engine is gone or the port is disconnected.
    EngineUnavailable,
    /// The engine completed a successful handshake/health check.
    EngineHealthy,
    /// Teardown/reset of the per-file transaction state.
    Reset,
    /// A transaction reached its settled outcome and the per-file state can
    /// return to Idle for the next recall.
    Settled,
}

/// Executable instruction produced by the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Enqueue,
    SendHydrate,
    ArmDeadline,
    DisarmDeadline,
    VerifyReply,
    SwitchBacking,
    FlushBuffers,
    MarkHydrated,
    MarkDehydrated,
    CompleteIo(IoStatus),
    FailFast(IoStatus),
    TripBreaker,
    ResetBreaker,
}

/// Result of one transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub next: State,
    pub actions: &'static [Action],
}

const NONE: &[Action] = &[];
const ENQUEUE: &[Action] = &[Action::Enqueue];
const SEND: &[Action] = &[Action::SendHydrate, Action::ArmDeadline];
const REPLY_OK: &[Action] = &[Action::DisarmDeadline, Action::VerifyReply];
const REPLY_REJECTED: &[Action] = &[
    Action::DisarmDeadline,
    Action::CompleteIo(IoStatus::DataError),
    Action::TripBreaker,
];
const WAIT_CANCEL: &[Action] = &[
    Action::DisarmDeadline,
    Action::CompleteIo(IoStatus::Cancelled),
];
const WAIT_TIMEOUT: &[Action] = &[
    Action::DisarmDeadline,
    Action::CompleteIo(IoStatus::TimedOut),
    Action::TripBreaker,
];
const WAIT_ENGINE_DOWN: &[Action] = &[
    Action::DisarmDeadline,
    Action::CompleteIo(IoStatus::ProviderNotRunning),
    Action::TripBreaker,
];
const VERIFY_FAILED: &[Action] = &[Action::CompleteIo(IoStatus::DataError), Action::TripBreaker];
const SWITCH_BACKING: &[Action] = &[Action::SwitchBacking];
const SWITCH_OK: &[Action] = &[
    Action::FlushBuffers,
    Action::MarkHydrated,
    Action::CompleteIo(IoStatus::Success),
];
const SWITCH_FAILED: &[Action] = &[
    Action::CompleteIo(IoStatus::InternalError),
    Action::TripBreaker,
];
const COMPLETE_CANCELLED: &[Action] = &[Action::CompleteIo(IoStatus::Cancelled)];
const COMPLETE_TIMEOUT: &[Action] = &[Action::CompleteIo(IoStatus::TimedOut), Action::TripBreaker];
const FAST_FAIL: &[Action] = &[Action::FailFast(IoStatus::ProviderNotRunning)];
const TRIP: &[Action] = &[Action::TripBreaker];
const RESET_BREAKER: &[Action] = &[Action::ResetBreaker];
const DEHYDRATE: &[Action] = &[Action::FlushBuffers, Action::MarkDehydrated];

/// Advance the state machine.
///
/// `None` means the event is not applicable in this state and the caller must
/// leave both state and outstanding I/O untouched.
pub fn step(state: State, event: Event) -> Option<Transition> {
    use Event::*;
    use State::*;

    let transition = match (state, event) {
        (Idle, DataAccess) => (Queued, ENQUEUE),
        (Idle, EngineUnavailable) => (Degraded, TRIP),
        (Idle, Reset) => (Idle, NONE),
        (Idle, Settled) => (Idle, NONE),

        (Queued, Dequeued) => (Waiting, SEND),
        (Queued, Cancel) => (Canceled, COMPLETE_CANCELLED),
        (Queued, Timeout) => (Canceled, COMPLETE_TIMEOUT),
        (Queued, EngineUnavailable) => (Canceled, WAIT_ENGINE_DOWN),
        (Queued, Reset) => (Canceled, COMPLETE_CANCELLED),

        (Waiting, ReplyAccepted) => (Verifying, REPLY_OK),
        (Waiting, ReplyRejected) => (Failed, REPLY_REJECTED),
        (Waiting, Cancel) => (Canceled, WAIT_CANCEL),
        (Waiting, Timeout) => (Degraded, WAIT_TIMEOUT),
        (Waiting, EngineUnavailable) => (Degraded, WAIT_ENGINE_DOWN),
        (Waiting, Reset) => (Canceled, WAIT_CANCEL),

        (Verifying, VerifyOk) => (Switching, SWITCH_BACKING),
        (Verifying, VerifyFailed) => (Failed, VERIFY_FAILED),
        (Verifying, Cancel) => (Canceled, COMPLETE_CANCELLED),
        (Verifying, Timeout) => (Degraded, COMPLETE_TIMEOUT),
        (Verifying, Reset) => (Canceled, COMPLETE_CANCELLED),

        (Switching, SwitchOk) => (Hydrated, SWITCH_OK),
        (Switching, SwitchFailed) => (Failed, SWITCH_FAILED),
        (Switching, Cancel) => (Draining(IoStatus::Cancelled), NONE),
        (Switching, Timeout) => (Draining(IoStatus::TimedOut), NONE),
        (Switching, Reset) => (Draining(IoStatus::Cancelled), NONE),

        (Draining(IoStatus::Cancelled), SwitchOk) => (Canceled, COMPLETE_CANCELLED),
        (Draining(IoStatus::TimedOut), SwitchOk) => (Degraded, COMPLETE_TIMEOUT),
        (Draining(_), SwitchFailed) => (Failed, SWITCH_FAILED),
        (Draining(_), Timeout) => (Degraded, COMPLETE_TIMEOUT),
        (Draining(status), Reset) => (Draining(status), NONE),

        (Hydrated, DataAccess) => (Hydrated, NONE),
        (Hydrated, Dehydrate) => (Idle, DEHYDRATE),
        (Hydrated, EngineUnavailable) => (Degraded, TRIP),
        (Hydrated, Reset) => (Hydrated, NONE),
        (Hydrated, Settled) => (Idle, NONE),

        (Canceled, Reset) | (Failed, Reset) => (Idle, NONE),
        (Canceled, Settled) | (Failed, Settled) => (Idle, NONE),

        (Degraded, DataAccess) => (Degraded, FAST_FAIL),
        (Degraded, EngineHealthy) => (Idle, RESET_BREAKER),
        (Degraded, Reset) => (Degraded, NONE),
        (Degraded, EngineUnavailable) => (Degraded, NONE),
        (Degraded, Settled) => (Degraded, NONE),

        _ => return None,
    };

    Some(Transition {
        next: transition.0,
        actions: transition.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT_STATES: [State; 4] = [
        State::Queued,
        State::Waiting,
        State::Verifying,
        State::Switching,
    ];

    const ALL_STATES: [State; 11] = [
        State::Idle,
        State::Queued,
        State::Waiting,
        State::Verifying,
        State::Switching,
        State::Draining(IoStatus::Cancelled),
        State::Draining(IoStatus::TimedOut),
        State::Hydrated,
        State::Canceled,
        State::Failed,
        State::Degraded,
    ];

    const ALL_EVENTS: [Event; 15] = [
        Event::DataAccess,
        Event::Dequeued,
        Event::Cancel,
        Event::Timeout,
        Event::ReplyAccepted,
        Event::ReplyRejected,
        Event::VerifyOk,
        Event::VerifyFailed,
        Event::SwitchOk,
        Event::SwitchFailed,
        Event::Dehydrate,
        Event::EngineUnavailable,
        Event::EngineHealthy,
        Event::Reset,
        Event::Settled,
    ];

    #[test]
    fn every_waiting_state_has_cancel_and_timeout_edges() {
        for state in WAIT_STATES {
            assert!(
                step(state, Event::Cancel).is_some(),
                "{} must accept Cancel",
                state_name(state)
            );
            assert!(
                step(state, Event::Timeout).is_some(),
                "{} must accept Timeout",
                state_name(state)
            );
        }

        // The switch critical section cannot be aborted, but a timeout must
        // still have an exit edge.
        for status in [IoStatus::Cancelled, IoStatus::TimedOut] {
            assert!(step(State::Draining(status), Event::Timeout).is_some());
        }
    }

    #[test]
    fn terminal_states_only_accept_reset() {
        for state in [State::Canceled, State::Failed] {
            for event in ALL_EVENTS {
                let handled = step(state, event).is_some();
                assert_eq!(
                    handled,
                    matches!(event, Event::Reset | Event::Settled),
                    "{} accepted unexpected {event:?}",
                    state_name(state)
                );
            }
        }
    }

    #[test]
    fn switch_cancel_is_deferred_and_completes_once() {
        let cancelled = step(State::Switching, Event::Cancel).expect("cancel is handled");
        assert_eq!(cancelled.next, State::Draining(IoStatus::Cancelled));
        assert!(cancelled.actions.is_empty());

        let done = step(cancelled.next, Event::SwitchOk).expect("switch completion is handled");
        assert_eq!(done.next, State::Canceled);
        assert_eq!(done.actions, COMPLETE_CANCELLED);

        let timed_out = step(State::Switching, Event::Timeout).expect("timeout is handled");
        assert_eq!(timed_out.next, State::Draining(IoStatus::TimedOut));
        let done = step(timed_out.next, Event::SwitchOk).expect("switch completion is handled");
        assert_eq!(done.next, State::Degraded);
        assert_eq!(done.actions, COMPLETE_TIMEOUT);
    }

    #[test]
    fn degraded_fails_fast_without_starting_a_transaction() {
        let transition = step(State::Degraded, Event::DataAccess).expect("degraded handles reads");
        assert_eq!(transition.next, State::Degraded);
        assert_eq!(transition.actions, FAST_FAIL);
        assert!(!transition
            .actions
            .iter()
            .any(|action| matches!(action, Action::Enqueue | Action::CompleteIo(_))));
    }

    #[test]
    fn a_transition_never_emits_more_than_one_completion() {
        for state in ALL_STATES {
            for event in ALL_EVENTS {
                if let Some(transition) = step(state, event) {
                    let completions = transition
                        .actions
                        .iter()
                        .filter(|action| matches!(action, Action::CompleteIo(_)))
                        .count();
                    assert!(
                        completions <= 1,
                        "{state:?} + {event:?} emitted {completions} completions"
                    );
                    assert!(transition.actions.len() <= 4);
                }
            }
        }
    }

    #[test]
    fn settled_returns_completed_state_to_idle() {
        for state in [State::Hydrated, State::Canceled, State::Failed] {
            let transition = step(state, Event::Settled).expect("settled");
            assert_eq!(transition.next, State::Idle);
            assert!(transition.actions.is_empty());
        }
        assert_eq!(
            step(State::Degraded, Event::Settled).expect("settled").next,
            State::Degraded
        );
    }

    #[test]
    fn reset_drains_an_inflight_transaction() {
        for state in WAIT_STATES {
            let transition = step(state, Event::Reset).expect("teardown is handled");
            let completions = |transition: &Transition| {
                transition
                    .actions
                    .iter()
                    .filter(|action| matches!(action, Action::CompleteIo(_)))
                    .count()
            };

            if let State::Draining(_) = transition.next {
                // The switch is not abortable; completion is deferred until
                // the in-flight switch finishes.
                assert_eq!(completions(&transition), 0);
                let finished = step(transition.next, Event::SwitchOk).expect("switch completes");
                assert_eq!(completions(&finished), 1);
            } else {
                assert!(
                    transition.next.is_terminal(),
                    "reset from {state:?} must drain, got {:?}",
                    transition.next
                );
                assert_eq!(
                    completions(&transition),
                    1,
                    "reset from {state:?} must complete the original I/O exactly once"
                );
            }
        }
    }

    #[test]
    fn state_helpers_are_consistent() {
        assert!(State::Draining(IoStatus::Cancelled).is_waiting());
        assert!(!State::Hydrated.is_waiting());
        assert!(State::Canceled.is_terminal());
        assert!(State::Failed.is_terminal());
        assert!(!State::Hydrated.is_terminal());
        assert!(State::Degraded.is_degraded());
    }

    fn state_name(state: State) -> &'static str {
        match state {
            State::Idle => "Idle",
            State::Queued => "Queued",
            State::Waiting => "Waiting",
            State::Verifying => "Verifying",
            State::Switching => "Switching",
            State::Draining(_) => "Draining",
            State::Hydrated => "Hydrated",
            State::Canceled => "Canceled",
            State::Failed => "Failed",
            State::Degraded => "Degraded",
        }
    }
}
