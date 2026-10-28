//! Integration checks for the pure core. These run with `cargo test` on the
//! host and need no WDK, kernel, or privileged access.

use cmflt_core::hydration::{step, Action, Event, IoStatus, State};
use cmflt_core::protocol::{
    Header, Health, Hello, HydrateReply, HydrateRequest, MessageKind, ProtocolError,
    HYDRATE_REPLY_PAYLOAD_LEN, HYDRATE_REQUEST_PAYLOAD_LEN, PROTOCOL_VERSION,
};
use cmflt_core::range::{ByteRange, RangeError, RangeSet, MAX_RANGES};

const EVENTS: [Event; 15] = [
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

const STATES: [State; 11] = [
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

#[test]
fn exhaustive_state_event_matrix_is_total_and_bounded() {
    for state in STATES {
        for event in EVENTS {
            if let Some(transition) = step(state, event) {
                assert!(transition.actions.len() <= 4, "{state:?} + {event:?}");
                let completions = transition
                    .actions
                    .iter()
                    .filter(|action| matches!(action, Action::CompleteIo(_)))
                    .count();
                assert!(completions <= 1, "{state:?} + {event:?}");
            }
        }
    }
}

#[test]
fn hydrate_request_wire_size_is_exact() {
    let mut buf = [0u8; 128];
    let message = HydrateRequest {
        tx_id: 7,
        file_id: 9,
        offset: 0,
        length: 4096,
        requestor_pid: 4,
        deadline_ms: 15_000,
        flags: 0,
    };
    let len = message.encode(&mut buf).expect("encode");
    assert_eq!(len, 24 + HYDRATE_REQUEST_PAYLOAD_LEN);
    assert_eq!(HydrateRequest::decode(&buf[..len]), Ok(message));
}

#[test]
fn hydrate_reply_wire_size_is_exact() {
    let mut buf = [0u8; 128];
    let message = HydrateReply {
        tx_id: 7,
        status: 0,
        bytes_written: 4096,
        sha1: [1u8; 20],
        flags: 0,
    };
    let len = message.encode(&mut buf).expect("encode");
    assert_eq!(len, 24 + HYDRATE_REPLY_PAYLOAD_LEN);
    assert_eq!(HydrateReply::decode(&buf[..len]), Ok(message));
}

#[test]
fn control_messages_have_zero_transaction_id() {
    let mut buf = [0u8; 128];

    let hello = Hello {
        proto_version: PROTOCOL_VERSION,
        capabilities: 0,
        engine_version: [0u64; 2],
    };
    let len = hello.encode(&mut buf).expect("hello");
    let header = Header::decode(&buf[..len]).expect("header");
    assert_eq!(header.kind, MessageKind::Hello);
    assert_eq!(header.tx_id, 0);

    let health = Health {
        uptime_ms: 1,
        inflight: 0,
        completed: 0,
        failed: 0,
        breaker_trips: 0,
    };
    let len = health.encode(&mut buf).expect("health");
    let header = Header::decode(&buf[..len]).expect("header");
    assert_eq!(header.kind, MessageKind::Health);
    assert_eq!(header.tx_id, 0);
}

#[test]
fn wrong_message_kind_is_rejected() {
    let mut buf = [0u8; 128];
    let request = HydrateRequest {
        tx_id: 1,
        file_id: 1,
        offset: 0,
        length: 1,
        requestor_pid: 1,
        deadline_ms: 1,
        flags: 0,
    };
    let len = request.encode(&mut buf).expect("encode");
    assert_eq!(
        HydrateReply::decode(&buf[..len]),
        Err(ProtocolError::WrongKind)
    );
}

#[test]
fn range_set_capacity_is_a_hard_bound() {
    let mut set = RangeSet::new();
    for index in 0..MAX_RANGES {
        let offset = (index as u64) * 16;
        set.insert(ByteRange::new(offset, 8)).expect("insert");
    }
    assert_eq!(
        set.insert(ByteRange::new(10_000, 8)),
        Err(RangeError::Capacity)
    );
    assert_eq!(set.len(), MAX_RANGES);
}

#[test]
fn every_started_transaction_has_a_completion_edge() {
    // From any waiting state, teardown must be able to complete the original
    // I/O (possibly after a deferred switch).
    for state in [
        State::Queued,
        State::Waiting,
        State::Verifying,
        State::Switching,
        State::Draining(IoStatus::Cancelled),
        State::Draining(IoStatus::TimedOut),
    ] {
        let transition = step(state, Event::Reset).expect("reset handled");
        let completes = transition
            .actions
            .iter()
            .any(|action| matches!(action, Action::CompleteIo(_)));
        assert!(
            completes || matches!(transition.next, State::Draining(_)),
            "{state:?} must complete or remain draining"
        );
    }
}
