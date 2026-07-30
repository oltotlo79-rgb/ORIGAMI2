use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use ori_collision::{
    CanonicalPositiveThicknessCyclePathControlErrorV1, CooperativeOperationControlV1,
};

#[test]
fn controlled_cycle_authority_read_rejects_stops_and_preserves_the_current_generation() {
    let _serial = super::STACKED_FOLD_READ_GENERATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let original = super::STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    super::STACKED_FOLD_READ_GENERATION.store(701, Ordering::Release);

    let active = AtomicBool::new(false);
    let deadline = CooperativeOperationControlV1::new(Some(&active), Instant::now());
    assert_eq!(
        super::super::controlled_cycle_authority_read_v1(701, &deadline, |_| {
            Err::<Option<u8>, _>(
                CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded,
            )
        }),
        Err(super::super::ControlledCycleAuthorityReadErrorV1::DeadlineExceeded)
    );
    let cancelled = AtomicBool::new(true);
    let cancelled_control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(
        super::super::controlled_cycle_authority_read_v1(701, &cancelled_control, |_| {
            Err::<Option<u8>, _>(CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
        }),
        Err(super::super::ControlledCycleAuthorityReadErrorV1::Cancelled)
    );

    let old = CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &super::STACKED_FOLD_READ_GENERATION,
        701,
        Instant::now() + Duration::from_secs(1),
    );
    super::STACKED_FOLD_READ_GENERATION.store(702, Ordering::Release);
    assert_eq!(
        super::super::controlled_cycle_authority_read_v1(701, &old, |_| Ok(Some(1_u8))),
        Err(super::super::ControlledCycleAuthorityReadErrorV1::Cancelled)
    );
    let current = CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &super::STACKED_FOLD_READ_GENERATION,
        702,
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(
        super::super::controlled_cycle_authority_read_v1(702, &current, |_| Ok(Some(2_u8))),
        Ok(Some(2_u8)),
        "a current generation may publish only its own authority"
    );
    assert_eq!(
        super::STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        702,
        "read-only authority checks do not apply a scene or alter history"
    );
    super::STACKED_FOLD_READ_GENERATION.store(original, Ordering::Release);
}

#[test]
fn blockwise_fallback_preserves_control_stop_messages() {
    for message in [
        super::super::CANCELLED_MESSAGE,
        super::super::CYCLE_PATH_DEADLINE_MESSAGE,
        super::super::CYCLE_PATH_RESOURCE_MESSAGE,
    ] {
        assert_eq!(
            super::super::normalize_blockwise_current_cycle_fallback_error_v1(message.to_owned()),
            message
        );
    }
}

#[test]
fn current_cycle_publication_is_atomic_with_generation_replacement_and_cancel() {
    let _serial = super::STACKED_FOLD_READ_GENERATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let original = super::STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    super::STACKED_FOLD_READ_GENERATION.store(731, Ordering::Release);

    let stale_publish_called = Cell::new(false);
    assert_eq!(
        super::super::with_current_cycle_publication_v1(730, || {
            stale_publish_called.set(true);
            Ok(1_u8)
        }),
        Err(super::super::CANCELLED_MESSAGE.to_owned())
    );
    assert!(!stale_publish_called.get());

    let current_publish_called = Cell::new(false);
    assert_eq!(
        super::super::with_current_cycle_publication_v1(731, || {
            current_publish_called.set(true);
            Ok(2_u8)
        }),
        Ok(2_u8)
    );
    assert!(current_publish_called.get());
    assert_eq!(
        super::STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        731,
        "publication does not alter the cancellation generation"
    );

    super::STACKED_FOLD_READ_GENERATION.store(original, Ordering::Release);
}
