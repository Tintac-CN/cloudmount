//! End-to-end host test of the KM-3 contract: coordinator → HydrateRequest →
//! mock engine → HydrateReply → coordinator.

use cmflt_core::coordinator::{HydrationCoordinator, ReadOutcome};
use cmflt_core::hydration::Action;
use cmflt_core::placeholder::{Placeholder, PlaceholderState};
use cmflt_core::protocol::{HydrateReply, HydrateRequest, STATUS_SUCCESS};
use cmflt_core::scan::ScanQuota;
use cmflt_sim::engine::MockEngine;

fn recall(coordinator: &mut HydrationCoordinator, offset: u64, length: u64) -> (u64, u64, u64) {
    let outcome = coordinator.begin_read(offset, length, 15_000);
    match outcome {
        ReadOutcome::Recall { tx_id, range, .. } => (tx_id, range.offset, range.length),
        other => panic!("expected recall, got {other:?}"),
    }
}

#[test]
fn hydrate_request_and_reply_complete_a_partial_range() {
    let mut coordinator =
        HydrationCoordinator::new(Placeholder::new(7, 4096), ScanQuota::new(8, 4096));
    let (tx_id, offset, length) = recall(&mut coordinator, 0, 1024);
    let actions = coordinator.on_dequeued().expect("dequeue");
    assert!(actions.contains(&Action::SendHydrate));
    assert!(actions.contains(&Action::ArmDeadline));

    let mut engine = MockEngine::new();
    engine.add_file(7, vec![0xAB; 4096]);
    let request = HydrateRequest {
        tx_id,
        file_id: 7,
        offset,
        length: length as u32,
        requestor_pid: 123,
        deadline_ms: 15_000,
        flags: 0,
    };
    let mut input = [0u8; 128];
    let request_len = request.encode(&mut input).expect("encode request");
    let mut output = [0u8; 128];
    let reply_len = engine
        .handle(&input[..request_len], &mut output)
        .expect("engine reply");
    let reply = HydrateReply::decode(&output[..reply_len]).expect("decode reply");
    assert_eq!(reply.status, STATUS_SUCCESS);
    assert_eq!(reply.bytes_written as u64, length);
    assert_eq!(reply.tx_id, tx_id);

    coordinator.on_reply_accepted().expect("reply accepted");
    coordinator.on_verify_ok().expect("verify");
    coordinator.on_switch_ok().expect("switch");
    assert_eq!(coordinator.state(), cmflt_core::hydration::State::Hydrated);
    assert_eq!(
        coordinator.placeholder().meta().state,
        PlaceholderState::Partial
    );
    assert_eq!(
        coordinator.placeholder().needs_recall(offset, length),
        Ok(false)
    );
    coordinator.settle().expect("settle");
    assert_eq!(coordinator.state(), cmflt_core::hydration::State::Idle);
    assert_eq!(
        coordinator.begin_read(offset, length, 20_000),
        ReadOutcome::Pass
    );
}

#[test]
fn engine_not_found_reply_fails_the_transaction_and_releases_quota() {
    let mut coordinator =
        HydrationCoordinator::new(Placeholder::new(7, 4096), ScanQuota::new(8, 4096));
    let (tx_id, offset, length) = recall(&mut coordinator, 0, 1024);
    coordinator.on_dequeued().expect("dequeue");

    let mut engine = MockEngine::new();
    let request = HydrateRequest {
        tx_id,
        file_id: 7,
        offset,
        length: length as u32,
        requestor_pid: 123,
        deadline_ms: 15_000,
        flags: 0,
    };
    let mut input = [0u8; 128];
    let request_len = request.encode(&mut input).expect("encode request");
    let mut output = [0u8; 128];
    let reply_len = engine
        .handle(&input[..request_len], &mut output)
        .expect("engine reply");
    let reply = HydrateReply::decode(&output[..reply_len]).expect("decode reply");
    assert_ne!(reply.status, STATUS_SUCCESS);

    coordinator.on_reply_rejected().expect("reject");
    assert_eq!(coordinator.state(), cmflt_core::hydration::State::Failed);
    assert_eq!(coordinator.quota().requests(), 0);
    assert!(matches!(
        coordinator.begin_read(0, 1024, 20_000),
        ReadOutcome::Recall { tx_id: 2, .. }
    ));
}
