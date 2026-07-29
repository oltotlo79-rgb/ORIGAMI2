//! Edit barriers, aggregate invalidation, and exact capacity progress.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Barrier, atomic::AtomicBool},
    time::{Duration, Instant},
};

use ori_domain::{EdgeId, FaceId, VertexId};

use super::*;

#[test]
fn one_shot_pose_transaction_rollback_restores_the_exact_cache_epoch() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    let capture = publish_fixture_to_runtime(&runtime, &fixture);
    let before = runtime.progress_v1().expect("initial progress");
    assert_eq!(before.persistent_cached_pairs, 1);
    let mut rollback = runtime
        .capture_rollback_snapshot_v1()
        .expect("opaque rollback snapshot");
    runtime
        .advance_pose_authority_v1(fixture.key.revision)
        .expect("one pose-authority transition");
    assert_eq!(
        runtime.progress_v1().expect("advanced progress").epoch,
        before.epoch.checked_add(1).expect("bounded test epoch")
    );
    rollback
        .restore_origin_exact_for_rollback_v1()
        .expect("exact originating runtime rollback");
    assert_eq!(runtime.progress_v1().expect("restored progress"), before);
    let (footprints, exact_poses) = current_snapshots(&fixture);
    let lookup = runtime
        .lookup_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&fixture.key),
            &generous_work_limits(),
            operation_control(),
        )
        .expect("restored proof-cache lookup");
    assert_eq!(lookup.hits().len(), 1);
    assert_eq!(lookup.missing_entries(), 0);
}

#[test]
fn origin_bound_rollback_restores_after_epoch_advances_beyond_normal_staleness() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let before = runtime.progress_v1().expect("initial progress");
    let mut rollback = runtime
        .capture_rollback_snapshot_v1()
        .expect("opaque rollback snapshot");
    runtime
        .advance_pose_authority_v1(fixture.key.revision)
        .expect("one pose-authority transition");
    runtime
        .advance_pose_authority_v1(fixture.key.revision)
        .expect("second pose-authority transition");
    rollback
        .restore_origin_exact_for_rollback_v1()
        .expect("the origin image bypasses normal stale-epoch policy");
    assert_eq!(runtime.progress_v1().expect("restored progress"), before);
}

#[test]
fn rollback_recovers_a_poisoned_cache_mutex_by_replacing_the_exact_image() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let before = runtime.progress_v1().expect("initial progress");
    let mut rollback = runtime
        .capture_rollback_snapshot_v1()
        .expect("opaque rollback snapshot");
    runtime
        .advance_pose_authority_v1(fixture.key.revision)
        .expect("one pose-authority transition");

    assert!(
        catch_unwind(AssertUnwindSafe(
            || runtime.poison_rollback_lock_for_test_v1()
        ))
        .is_err(),
        "the injected panic must poison the normal cache lock"
    );
    rollback
        .restore_origin_exact_for_rollback_v1()
        .expect("rollback recovers poison before replacing state");
    assert_eq!(runtime.progress_v1().expect("recovered progress"), before);
}

#[test]
fn complete_edit_panic_raii_recovers_poison_and_clears_the_in_progress_epoch() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let ticket = runtime.begin_edit_epoch_v1().expect("edit epoch");
    let epoch = runtime.progress_v1().expect("begun progress").epoch;
    PersistentPairProofCacheRuntimeV1::panic_next_complete_edit_while_locked_for_test_v1();
    let impact = disjoint_impact();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = runtime.complete_edit_epoch_v1(
                ticket,
                SOURCE_REVISION,
                TARGET_REVISION,
                impact.vertices,
                impact.edges,
                impact.faces,
                operation_control(),
            );
        }))
        .is_err(),
        "the deterministic fault must unwind while the runtime mutex is held"
    );

    assert_eq!(
        runtime.progress_v1().expect("RAII recovered runtime"),
        ProofCacheProgressV1 {
            epoch,
            ..ProofCacheProgressV1::default()
        }
    );
    let capture = runtime
        .capture_v1(binding_for_key(&fixture.key))
        .expect("the invalidation barrier was cleared");
    let (footprints, exact_poses) = current_snapshots(&fixture);
    let lookup = runtime
        .lookup_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&fixture.key),
            &generous_work_limits(),
            operation_control(),
        )
        .expect("fail-closed cache remains usable");
    assert_eq!(lookup.hits().len(), 0);
    assert_eq!(lookup.missing_entries(), 1);
}

#[test]
fn complete_edit_preserves_typed_preparation_errors_and_fails_closed() {
    let fixture = model4_fixture(0);

    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let cancelled = AtomicBool::new(true);
    let impact = disjoint_impact();
    assert_eq!(
        runtime.begin_complete_edit_v1(
            SOURCE_REVISION,
            TARGET_REVISION,
            impact.vertices,
            impact.edges,
            impact.faces,
            ProofCacheOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(5),
            ),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::Cancelled
        ))
    );
    assert_eq!(
        runtime
            .progress_v1()
            .expect("cancelled preparation recovered")
            .persistent_cached_pairs,
        0
    );

    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let impact = disjoint_impact();
    assert_eq!(
        runtime.begin_complete_edit_v1(
            SOURCE_REVISION,
            TARGET_REVISION,
            impact.vertices,
            impact.edges,
            impact.faces,
            ProofCacheOperationControlV1::new(None, Instant::now()),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::DeadlineExceeded
        ))
    );
    assert_eq!(
        runtime
            .progress_v1()
            .expect("deadlined preparation recovered")
            .persistent_cached_pairs,
        0
    );

    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let ticket = runtime.begin_edit_epoch_v1().expect("resource edit epoch");
    assert_eq!(
        runtime.complete_edit_epoch_with_upstream_work_v1(
            ticket,
            SOURCE_REVISION,
            TARGET_REVISION,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MAX_PROOF_CACHE_INVALIDATION_WORK_V1 + 1,
            operation_control(),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded
        ))
    );
    assert_eq!(
        runtime
            .progress_v1()
            .expect("resource preparation recovered")
            .persistent_cached_pairs,
        0
    );

    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let impact = disjoint_impact();
    assert_eq!(
        runtime.begin_complete_edit_v1(
            SOURCE_REVISION,
            SOURCE_REVISION,
            impact.vertices,
            impact.edges,
            impact.faces,
            operation_control(),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::InvalidCandidate
        ))
    );
    assert_eq!(
        runtime
            .progress_v1()
            .expect("invalid preparation recovered")
            .persistent_cached_pairs,
        0
    );
}

#[test]
fn consumed_origin_rollback_image_fails_once_without_retryability() {
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    let mut rollback = runtime
        .capture_rollback_snapshot_v1()
        .expect("opaque rollback snapshot");
    rollback
        .restore_origin_exact_for_rollback_v1()
        .expect("first origin restore");
    assert_eq!(
        rollback.restore_origin_exact_for_rollback_v1(),
        Err(ProofCacheRuntimeErrorV1::InvalidBinding),
        "a consumed rollback image is a finite invariant error"
    );
}

#[test]
fn consecutive_disjoint_edits_aggregate_from_original_revision_and_retain_proof() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let first = disjoint_impact();
    runtime
        .begin_complete_edit_v1(
            SOURCE_REVISION,
            TARGET_REVISION,
            first.vertices,
            first.edges,
            first.faces,
            operation_control(),
        )
        .expect("first complete edit");
    runtime
        .advance_pose_authority_v1(TARGET_REVISION)
        .expect("first target pose");
    let ticket = runtime.begin_edit_epoch_v1().expect("second edit epoch");
    runtime
        .complete_edit_epoch_v1(
            ticket,
            TARGET_REVISION,
            TARGET_REVISION + 1,
            vec![VertexId::derive_v5(project(0xF1), b"second-disjoint")],
            vec![EdgeId::derive_v5(project(0xF2), b"second-disjoint")],
            vec![FaceId::derive_v5(project(0xF3), b"second-disjoint")],
            operation_control(),
        )
        .expect("second complete edit");
    runtime
        .advance_pose_authority_v1(TARGET_REVISION + 1)
        .expect("second target pose");
    let mut target_key = fixture.key.clone();
    target_key.revision = TARGET_REVISION + 1;
    target_key.geometry_fingerprint = [0x34; 32];
    target_key.pose_generation = TARGET_POSE_GENERATION + 1;
    let capture = runtime
        .capture_v1(binding_for_key(&target_key))
        .expect("aggregate target capture");
    let (footprints, exact_poses) = current_snapshots(&fixture);
    let lookup = runtime
        .lookup_two_hinge_positive_v1(
            &capture,
            target_key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&target_key),
            &generous_work_limits(),
            operation_control(),
        )
        .expect("aggregate differential rebind");
    assert_eq!(lookup.hits().len(), 1);
    assert_eq!(lookup.missing_entries(), 0);
}

#[test]
fn cold_phase_barrier_allows_epoch_advance_and_rejects_stale_publish() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    let capture = runtime
        .capture_v1(binding_for_key(&fixture.key))
        .expect("source capture");
    let barrier = Arc::new(Barrier::new(2));
    let worker_runtime = runtime.clone();
    let worker_fixture = fixture.clone();
    let worker_capture = capture.clone();
    let worker_barrier = Arc::clone(&barrier);
    let worker = std::thread::spawn(move || {
        let (footprints, exact_poses) = current_snapshots(&worker_fixture);
        let miss = worker_runtime
            .lookup_two_hinge_positive_v1(
                &worker_capture,
                worker_fixture.key.issuer_context,
                footprints,
                exact_poses,
                std::slice::from_ref(&worker_fixture.key),
                &generous_work_limits(),
                operation_control(),
            )
            .expect("cold miss");
        assert_eq!(miss.missing_entries(), 1);
        worker_barrier.wait();
        worker_barrier.wait();
        worker_runtime.publish_two_hinge_positive_v1(
            &worker_capture,
            worker_fixture.key.issuer_context,
            vec![worker_fixture.candidate.clone()],
            1,
            1,
            0,
            &worker_fixture.candidate.work,
            operation_control(),
        )
    });

    barrier.wait();
    let ticket = runtime
        .begin_edit_epoch_v1()
        .expect("cache mutex is free during simulated cold proof");
    let disjoint = disjoint_impact();
    runtime
        .complete_edit_epoch_v1(
            ticket,
            SOURCE_REVISION,
            TARGET_REVISION,
            disjoint.vertices,
            disjoint.edges,
            disjoint.faces,
            operation_control(),
        )
        .expect("complete epoch advance before stale publication");
    barrier.wait();
    assert_eq!(
        worker.join().expect("cold worker"),
        Err(ProofCacheRuntimeErrorV1::StaleProof)
    );
    let progress = runtime.progress_v1().expect("progress");
    assert_eq!(progress.persistent_cached_pairs, 0);
    assert_eq!(progress.proven_pairs, 0);
}

#[test]
fn runtime_capacity_progress_is_explicit_at_exact_entry_boundary() {
    let first = model4_fixture(0);
    let second = model4_fixture(5);
    let runtime = PersistentPairProofCacheRuntimeV1::new(cache_limits(
        1,
        MAX_PROOF_CACHE_STORAGE_BYTES_V1,
        MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
    ))
    .expect("single-entry runtime");
    let capture = runtime
        .capture_v1(binding_for_key(&first.key))
        .expect("capture");
    let accounted = first
        .candidate
        .work
        .checked_merge(&second.candidate.work, &generous_work_limits())
        .expect("aggregate work");
    let report = runtime
        .publish_two_hinge_positive_v1(
            &capture,
            first.key.issuer_context,
            vec![first.candidate.clone(), second.candidate.clone()],
            2,
            2,
            0,
            &accounted,
            operation_control(),
        )
        .expect("capacity is explicit");
    assert_eq!(report.admitted_entries, 1);
    assert_eq!(report.unproven_due_to_capacity, 1);
    let progress = runtime.progress_v1().expect("progress");
    assert_eq!(progress.persistent_cached_pairs, 1);
    assert_eq!(progress.capacity_unproven_pairs, 1);
}
