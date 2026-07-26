//! Adversarial tests for the process-local proof-cache runtime.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use ori_domain::FaceId;

use super::{tests::support::*, *};

#[path = "proof_cache_runtime_tests/edit_epoch.rs"]
mod edit_epoch;

fn model4_fixture(seed: u8) -> Fixture {
    let mut key = base_key();
    key.certificate_model = ProofCacheCertificateModelV1::TwoHingePositiveThickness;
    if seed != 0 {
        key.faces = [face(seed.wrapping_add(2)), face(seed.wrapping_add(3))];
        key.faces.sort_unstable_by_key(FaceId::canonical_bytes);
    }
    let source = fixture_for(key.clone(), seed);
    let dependencies = PairProofDependenciesV1::new_v1(
        &key,
        source.candidate.dependencies.footprints.clone(),
        source.candidate.dependencies.exact_poses.clone(),
        Vec::new(),
    )
    .expect("model-4 dependencies without a foreign memo");
    let candidate = PairProofCacheCandidateV1::new_v1(
        key.clone(),
        CachedPairProofConclusionV1::NonBlocking,
        source.candidate.work.clone(),
        dependencies,
    )
    .expect("sealed model-4 candidate");
    Fixture { key, candidate }
}

fn binding_for_key(key: &ProofCacheKeyV1) -> ProofCacheRuntimeBindingV1 {
    ProofCacheRuntimeBindingV1::new(
        key.project_instance_id,
        key.project_id,
        key.revision,
        key.geometry_fingerprint,
        key.pose_generation,
        f64::from_bits(key.paper_thickness_bits),
    )
    .expect("valid runtime binding")
}

fn runtime_with_work_limit(work_limit: usize) -> PersistentPairProofCacheRuntimeV1 {
    PersistentPairProofCacheRuntimeV1::new(cache_limits(
        MAX_PROOF_CACHE_ENTRIES_V1,
        MAX_PROOF_CACHE_STORAGE_BYTES_V1,
        work_limit,
    ))
    .expect("bounded runtime")
}

fn publish_fixture_to_runtime(
    runtime: &PersistentPairProofCacheRuntimeV1,
    fixture: &Fixture,
) -> ProofCacheRuntimeCaptureV1 {
    let capture = runtime
        .capture_v1(binding_for_key(&fixture.key))
        .expect("runtime capture");
    runtime
        .publish_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            vec![fixture.candidate.clone()],
            1,
            1,
            0,
            &fixture.candidate.work,
            operation_control(),
        )
        .expect("runtime publication");
    capture
}

fn current_snapshots(
    fixture: &Fixture,
) -> (
    Vec<FaceDependencyFootprintV1>,
    Vec<ExactFacePoseCacheWitnessV1>,
) {
    (
        fixture.candidate.dependencies.footprints.to_vec(),
        fixture.candidate.dependencies.exact_poses.to_vec(),
    )
}

fn normal_hit_runtime_work_v1(fixture: &Fixture) -> usize {
    let (footprints, exact_poses) = current_snapshots(fixture);
    let face_count = footprints.len();
    let levels =
        usize::try_from(usize::BITS - (face_count - 1).leading_zeros()).expect("small face count");
    let sort_work = face_count * (levels + 2) + exact_poses.len() * (levels + 2);
    let snapshot_lookup_work = 4 * (levels + 1);
    let cache_lookup_work = 3;
    let canonicalization_work = face_count + exact_poses.len();
    let footprint_work = fixture
        .candidate
        .dependencies
        .footprints
        .iter()
        .map(|item| 2 * item.vertices.len() + 2 * item.edges.len() + 2)
        .sum::<usize>();
    let exact_byte_work = fixture
        .candidate
        .dependencies
        .exact_poses
        .iter()
        .map(|item| 2 * item.canonical_exact_bytes.len())
        .sum::<usize>();
    sort_work
        + snapshot_lookup_work
        + cache_lookup_work
        + canonicalization_work
        + footprint_work
        + exact_byte_work
}

#[test]
fn runtime_binding_rejects_both_signed_zero_thicknesses_for_model4() {
    let fixture = model4_fixture(0);
    for zero in [0.0, -0.0] {
        assert_eq!(
            ProofCacheRuntimeBindingV1::new(
                fixture.key.project_instance_id,
                fixture.key.project_id,
                fixture.key.revision,
                fixture.key.geometry_fingerprint,
                fixture.key.pose_generation,
                zero,
            ),
            Err(ProofCacheRuntimeErrorV1::InvalidBinding)
        );
    }
}

#[test]
fn normal_hit_charges_all_footprint_ids_and_exact_bytes_one_short() {
    let fixture = model4_fixture(0);
    let exact_work = normal_hit_runtime_work_v1(&fixture);
    assert!(exact_work > 0);

    let runtime = runtime_with_work_limit(exact_work);
    let capture = publish_fixture_to_runtime(&runtime, &fixture);
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
        .expect("exact work limit admits a normal hit");
    assert_eq!(lookup.hits().len(), 1);

    let runtime = runtime_with_work_limit(exact_work - 1);
    let capture = publish_fixture_to_runtime(&runtime, &fixture);
    let (footprints, exact_poses) = current_snapshots(&fixture);
    assert_eq!(
        runtime.lookup_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&fixture.key),
            &generous_work_limits(),
            operation_control(),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded
        ))
    );
}

#[test]
fn normal_hit_honours_cancellation_deadline_and_snapshot_identity() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    let capture = publish_fixture_to_runtime(&runtime, &fixture);
    let cancelled = AtomicBool::new(true);
    let (footprints, exact_poses) = current_snapshots(&fixture);
    assert_eq!(
        runtime.lookup_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&fixture.key),
            &generous_work_limits(),
            ProofCacheOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(5)
            ),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::Cancelled
        ))
    );
    let (footprints, exact_poses) = current_snapshots(&fixture);
    assert_eq!(
        runtime.lookup_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&fixture.key),
            &generous_work_limits(),
            ProofCacheOperationControlV1::new(None, Instant::now()),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::DeadlineExceeded
        ))
    );
    let (footprints, mut exact_poses) = current_snapshots(&fixture);
    exact_poses[0].canonical_exact_bytes.push(0xFF);
    assert_eq!(
        runtime.lookup_two_hinge_positive_v1(
            &capture,
            fixture.key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&fixture.key),
            &generous_work_limits(),
            operation_control(),
        ),
        Err(ProofCacheRuntimeErrorV1::InvalidBinding)
    );
}

#[test]
fn cancelled_post_rebind_validation_never_authenticates_a_wrong_retry() {
    let fixture = model4_fixture(0);
    let runtime = runtime_with_work_limit(MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
    publish_fixture_to_runtime(&runtime, &fixture);
    let disjoint = disjoint_impact();
    runtime
        .begin_complete_edit_v1(
            SOURCE_REVISION,
            TARGET_REVISION,
            disjoint.vertices,
            disjoint.edges,
            disjoint.faces,
            operation_control(),
        )
        .expect("complete disjoint edit");
    runtime
        .advance_pose_authority_v1(TARGET_REVISION)
        .expect("target pose preserves pending impact");
    let mut target_key = fixture.key.clone();
    target_key.revision = TARGET_REVISION;
    target_key.geometry_fingerprint = TARGET_GEOMETRY;
    target_key.pose_generation = TARGET_POSE_GENERATION;
    let target_capture = runtime
        .capture_v1(binding_for_key(&target_key))
        .expect("target capture");
    let cancelled = AtomicBool::new(false);
    let (footprints, exact_poses) = current_snapshots(&fixture);
    assert_eq!(
        runtime.lookup_two_hinge_positive_after_cache_hook_v1(
            &target_capture,
            target_key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&target_key),
            &generous_work_limits(),
            ProofCacheOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(5)
            ),
            || cancelled.store(true, Ordering::Release),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::Cancelled
        ))
    );

    cancelled.store(false, Ordering::Release);
    let (footprints, mut wrong_poses) = current_snapshots(&fixture);
    wrong_poses[0].canonical_exact_bytes.push(1);
    assert_eq!(
        runtime.lookup_two_hinge_positive_v1(
            &target_capture,
            target_key.issuer_context,
            footprints,
            wrong_poses,
            std::slice::from_ref(&target_key),
            &generous_work_limits(),
            operation_control(),
        ),
        Err(ProofCacheRuntimeErrorV1::InvalidBinding)
    );
    let (footprints, exact_poses) = current_snapshots(&fixture);
    assert_eq!(
        runtime
            .lookup_two_hinge_positive_v1(
                &target_capture,
                target_key.issuer_context,
                footprints,
                exact_poses,
                std::slice::from_ref(&target_key),
                &generous_work_limits(),
                operation_control(),
            )
            .expect("valid retry")
            .hits()
            .len(),
        1
    );
}

#[test]
fn pending_rebind_and_lookup_share_one_total_work_cap_exactly() {
    fn prepare(
        fixture: &Fixture,
        work_limit: usize,
    ) -> (
        PersistentPairProofCacheRuntimeV1,
        ProofCacheRuntimeCaptureV1,
        ProofCacheKeyV1,
    ) {
        let runtime = runtime_with_work_limit(work_limit);
        publish_fixture_to_runtime(&runtime, fixture);
        let disjoint = disjoint_impact();
        runtime
            .begin_complete_edit_v1(
                SOURCE_REVISION,
                TARGET_REVISION,
                disjoint.vertices,
                disjoint.edges,
                disjoint.faces,
                operation_control(),
            )
            .expect("complete disjoint edit");
        runtime
            .advance_pose_authority_v1(TARGET_REVISION)
            .expect("preserve complete impact");
        let mut target_key = fixture.key.clone();
        target_key.revision = TARGET_REVISION;
        target_key.geometry_fingerprint = TARGET_GEOMETRY;
        target_key.pose_generation = TARGET_POSE_GENERATION;
        let capture = runtime
            .capture_v1(binding_for_key(&target_key))
            .expect("target capture");
        (runtime, capture, target_key)
    }

    let fixture = model4_fixture(0);
    let (runtime, capture, target_key) = prepare(&fixture, MAX_PROOF_CACHE_INVALIDATION_WORK_V1);
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
        .expect("bounded aggregate operation");
    assert_eq!(lookup.hits().len(), 1);
    let exact_total_work = lookup.runtime_operation_work();
    assert!((1..=MAX_PROOF_CACHE_INVALIDATION_WORK_V1).contains(&exact_total_work));

    let (runtime, capture, target_key) = prepare(&fixture, exact_total_work - 1);
    let (footprints, exact_poses) = current_snapshots(&fixture);
    assert_eq!(
        runtime.lookup_two_hinge_positive_v1(
            &capture,
            target_key.issuer_context,
            footprints,
            exact_poses,
            std::slice::from_ref(&target_key),
            &generous_work_limits(),
            operation_control(),
        ),
        Err(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded
        ))
    );
}
