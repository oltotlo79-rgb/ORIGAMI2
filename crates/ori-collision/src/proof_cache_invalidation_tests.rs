use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use ori_domain::VertexId;

use super::{super::*, support::*};

#[test]
fn model_neutral_rebind_context_accepts_and_distinguishes_signed_zero() {
    let positive = ProofCacheRebindContextV1::new(
        project(1),
        project(2),
        TARGET_REVISION,
        TARGET_GEOMETRY,
        TARGET_POSE_GENERATION,
        0.0,
        ISSUER_CONTEXT,
    )
    .expect("positive zero is valid for a zero-thickness model");
    let negative = ProofCacheRebindContextV1::new(
        project(1),
        project(2),
        TARGET_REVISION,
        TARGET_GEOMETRY,
        TARGET_POSE_GENERATION,
        -0.0,
        ISSUER_CONTEXT,
    )
    .expect("negative zero remains an exact, distinct generic binding");

    assert_ne!(positive, negative);
}

#[test]
fn proof_cache_invalidation_discards_pair_touching_impact_set() {
    let fixture = base_fixture();
    let touched_vertex = fixture.candidate.dependencies.footprints[0].vertices[0];
    let impact = AppliedEditImpactSetV1::from_complete_aggregate_v1(
        SOURCE_REVISION,
        TARGET_REVISION,
        vec![touched_vertex],
        Vec::new(),
        Vec::new(),
        &operation_control(),
    )
    .expect("complete touching impact");
    let healthy = fixture.candidate.dependencies.memo_dependencies.clone();
    let request = rebind_request(&fixture, impact, healthy);
    let rebound = rebound_key(&fixture.key);
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);

    let report = cache
        .rebind_after_complete_edit_v1(request, operation_control())
        .expect("bounded invalidation");
    assert_eq!(report.examined_entries, 1);
    assert_eq!(report.retained_entries, 0);
    assert_eq!(report.unproven_entries, 1);
    assert_eq!(report.total_entries, 0);
    assert!(
        cache
            .lookup_v1(&fixture.key, &generous_work_limits())
            .expect("old lookup")
            .is_none()
    );
    assert!(
        cache
            .lookup_v1(&rebound, &generous_work_limits())
            .expect("new lookup")
            .is_none()
    );
}

#[test]
fn proof_cache_invalidation_retains_pair_with_proven_disjointness() {
    let fixture = base_fixture();
    let healthy = fixture.candidate.dependencies.memo_dependencies.clone();
    let request = rebind_request(&fixture, disjoint_impact(), healthy);
    let rebound = rebound_key(&fixture.key);
    let cold_work = fixture.candidate.work.clone();
    let old_binding = fixture.candidate.result.binding();
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);

    let report = cache
        .rebind_after_complete_edit_v1(request, operation_control())
        .expect("complete disjointness proof");
    assert_eq!(report.examined_entries, 1);
    assert_eq!(report.retained_entries, 1);
    assert_eq!(report.unproven_entries, 0);
    assert_eq!(report.total_entries, 1);
    assert!(report.invalidation_work > 0);
    assert!(
        cache
            .lookup_v1(&fixture.key, &generous_work_limits())
            .expect("old lookup")
            .is_none()
    );
    let hit = cache
        .lookup_v1(&rebound, &generous_work_limits())
        .expect("new lookup")
        .expect("explicitly rebound proof");
    assert_eq!(hit.accounted_work(), &cold_work);
    assert_eq!(
        hit.result().conclusion(),
        CachedPairProofConclusionV1::NonBlocking
    );
    assert_ne!(hit.result().binding(), old_binding);
}

#[test]
fn proof_cache_invalidation_retains_nothing_when_shared_memo_invalidated() {
    let first = base_fixture();
    let mut second_key = first.key.clone();
    second_key.certificate_model = ProofCacheCertificateModelV1::TreeIntervalZeroThickness;
    let second = fixture_for(second_key, 0);
    assert_eq!(
        first.candidate.dependencies.memo_dependencies,
        second.candidate.dependencies.memo_dependencies
    );
    let invalidated = vec![memo_token(0x61, 10)];
    let request = rebind_request(&first, disjoint_impact(), invalidated);
    let mut cache = default_cache();
    cache
        .publish_batch_v1(
            vec![first.candidate.clone(), second.candidate.clone()],
            operation_control(),
        )
        .expect("two shared-memo candidates");

    let report = cache
        .rebind_after_complete_edit_v1(request, operation_control())
        .expect("memo invalidation");
    assert_eq!(report.examined_entries, 2);
    assert_eq!(report.retained_entries, 0);
    assert_eq!(report.unproven_entries, 2);
    assert_eq!(report.total_entries, 0);
}

#[test]
fn proof_cache_invalidation_fails_closed_on_footprint_or_pose_drift() {
    let fixture = base_fixture();
    let healthy = fixture.candidate.dependencies.memo_dependencies.clone();
    let context = ProofCacheRebindContextV1::new(
        fixture.key.project_instance_id,
        fixture.key.project_id,
        TARGET_REVISION,
        TARGET_GEOMETRY,
        TARGET_POSE_GENERATION,
        THICKNESS,
        fixture.key.issuer_context,
    )
    .expect("valid context");
    let mut changed_footprints = fixture.candidate.dependencies.footprints.to_vec();
    changed_footprints[0].vertices.push(vertex(0xD1));
    changed_footprints[0]
        .vertices
        .sort_unstable_by_key(VertexId::canonical_bytes);
    let request = ProofCacheRebindRequestV1::from_complete_revision_snapshot_v1(
        context,
        disjoint_impact(),
        changed_footprints,
        fixture.candidate.dependencies.exact_poses.to_vec(),
        healthy,
    )
    .expect("complete changed footprint snapshot");
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let report = cache
        .rebind_after_complete_edit_v1(request, operation_control())
        .expect("footprint drift is an unproven outcome");
    assert_eq!(report.retained_entries, 0);
    assert_eq!(report.unproven_entries, 1);

    let fixture = base_fixture();
    let context = ProofCacheRebindContextV1::new(
        fixture.key.project_instance_id,
        fixture.key.project_id,
        TARGET_REVISION,
        TARGET_GEOMETRY,
        TARGET_POSE_GENERATION,
        THICKNESS,
        fixture.key.issuer_context,
    )
    .expect("valid context");
    let mut changed_poses = fixture.candidate.dependencies.exact_poses.to_vec();
    changed_poses[0].canonical_exact_bytes.push(0xFF);
    let request = ProofCacheRebindRequestV1::from_complete_revision_snapshot_v1(
        context,
        disjoint_impact(),
        fixture.candidate.dependencies.footprints.to_vec(),
        changed_poses,
        fixture.candidate.dependencies.memo_dependencies.clone(),
    )
    .expect("complete changed pose snapshot");
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let report = cache
        .rebind_after_complete_edit_v1(request, operation_control())
        .expect("pose drift is an unproven outcome");
    assert_eq!(report.retained_entries, 0);
    assert_eq!(report.unproven_entries, 1);
}

#[test]
fn proof_cache_invalidation_cancel_rolls_back_atomically() {
    let fixture = base_fixture();
    let healthy = fixture.candidate.dependencies.memo_dependencies.clone();
    let request = rebind_request(&fixture, disjoint_impact(), healthy);
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let cancellation = AtomicBool::new(true);

    assert_eq!(
        cache.rebind_after_complete_edit_v1(
            request,
            ProofCacheOperationControlV1::new(
                Some(&cancellation),
                Instant::now() + Duration::from_secs(30),
            ),
        ),
        Err(ProofCacheErrorV1::Cancelled)
    );
    assert_eq!(cache.entry_count(), 1);
    assert!(
        cache
            .lookup_v1(&fixture.key, &generous_work_limits())
            .expect("old entry remains")
            .is_some()
    );
}

#[test]
fn proof_cache_invalidation_work_exhaustion_rolls_back_atomically() {
    let fixture = base_fixture();
    let healthy = fixture.candidate.dependencies.memo_dependencies.clone();
    let request = rebind_request(&fixture, disjoint_impact(), healthy);
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    // Impact preparation, snapshot canonicalization and entry authentication
    // all consume the same meter; exhaustion occurs before the commit phase.
    cache.limits.max_invalidation_work = 12;

    assert_eq!(
        cache.rebind_after_complete_edit_v1(request, operation_control()),
        Err(ProofCacheErrorV1::ResourceLimitExceeded)
    );
    assert_eq!(cache.entry_count(), 1);
    assert!(
        cache
            .lookup_v1(&fixture.key, &generous_work_limits())
            .expect("old entry remains after rollback")
            .is_some()
    );
}

#[test]
fn proof_cache_invalidation_rebind_collision_rolls_back_atomically() {
    let first = base_fixture();
    let mut alternate_key = first.key.clone();
    alternate_key.geometry_fingerprint[0] ^= 1;
    let alternate = fixture_for(alternate_key, 0);
    let request = rebind_request(
        &first,
        disjoint_impact(),
        first.candidate.dependencies.memo_dependencies.clone(),
    );
    let mut cache = default_cache();
    cache
        .publish_batch_v1(
            vec![first.candidate.clone(), alternate.candidate.clone()],
            operation_control(),
        )
        .expect("two source observations");

    assert_eq!(
        cache.rebind_after_complete_edit_v1(request, operation_control()),
        Err(ProofCacheErrorV1::ConflictingEvidence)
    );
    assert_eq!(cache.entry_count(), 2);
    for key in [&first.key, &alternate.key] {
        assert!(
            cache
                .lookup_v1(key, &generous_work_limits())
                .expect("source lookup after rollback")
                .is_some()
        );
    }
}

#[test]
fn proof_cache_invalidation_requires_strict_revision_and_pose_monotonicity() {
    let fixture = base_fixture();
    let context = ProofCacheRebindContextV1::new(
        fixture.key.project_instance_id,
        fixture.key.project_id,
        TARGET_REVISION,
        TARGET_GEOMETRY,
        fixture.key.pose_generation,
        THICKNESS,
        fixture.key.issuer_context,
    )
    .expect("otherwise valid context");
    let request = ProofCacheRebindRequestV1::from_complete_revision_snapshot_v1(
        context,
        disjoint_impact(),
        fixture.candidate.dependencies.footprints.to_vec(),
        fixture.candidate.dependencies.exact_poses.to_vec(),
        fixture.candidate.dependencies.memo_dependencies.clone(),
    )
    .expect("complete snapshot");
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let report = cache
        .rebind_after_complete_edit_v1(request, operation_control())
        .expect("nonmonotonic pose becomes unproven");
    assert_eq!(report.retained_entries, 0);
    assert_eq!(report.unproven_entries, 1);
}
