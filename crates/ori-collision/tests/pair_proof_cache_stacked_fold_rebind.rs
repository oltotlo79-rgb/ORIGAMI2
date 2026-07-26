use std::collections::HashSet;

use ori_collision::{
    PersistentPairProofCacheRuntimeV1, ProofCacheLimitsV1, ProofCacheRuntimeBindingV1,
    STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    StackedFoldPathDiagnosticErrorV1,
};
use ori_domain::ProjectId;

#[path = "support/pair_proof_cache_driver.rs"]
mod cache_support;
#[path = "support/pair_proof_cache_stacked_fold.rs"]
mod support;

use cache_support::{binding, cached, uncached};
use support::{
    BASELINE_TARGET_REVISION, CHANGED_TARGET_REVISION, FACE_COUNT, PAPER_THICKNESS_MM,
    REQUESTED_ANGLE_DEGREES, footprint_map, production_fixture,
};

// This is the production-pipeline truth test. Its deliberately small exact
// pair set keeps CI bounded and does not claim a retention percentage. The
// separate legal 15-face acceptance test
// `stable_identity_fifteen_face_fixture_reproves_fourteen_of_one_hundred_five_pairs`
// measures the 91-hit/14-cold (13.33%) differential-retention requirement.
#[test]
fn production_stacked_fold_edit_rebind_is_exact_and_fails_closed_across_aba() {
    let identity = ProjectId::new();
    let instance = ProjectId::new();
    let fixture = production_fixture(identity);
    let baseline = &fixture.baseline;
    let changed = &fixture.changed;

    assert_eq!(baseline.revision, BASELINE_TARGET_REVISION);
    assert_eq!(changed.revision, CHANGED_TARGET_REVISION);
    assert_eq!(baseline.model().face_ids().len(), FACE_COUNT);
    assert_eq!(baseline.model().hinges().len(), FACE_COUNT - 1);
    assert_ne!(baseline.fingerprint, changed.fingerprint);
    assert_eq!(baseline.moving_hinges(), changed.moving_hinges());
    assert_eq!(baseline.pose().fixed_face(), changed.pose().fixed_face());

    let baseline_position = baseline
        .candidate_pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == fixture.changed_vertex)
        .expect("baseline production vertex")
        .position;
    let changed_position = changed
        .candidate_pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == fixture.changed_vertex)
        .expect("regenerated production vertex")
        .position;
    assert_eq!(
        changed_position.y.to_bits(),
        (baseline_position.y + 0.25).to_bits(),
        "the one-vertex source edit must reach the regenerated target geometry"
    );

    let baseline_footprints = footprint_map(&baseline.topology);
    let changed_footprints = footprint_map(&changed.topology);
    let impacted_faces = baseline_footprints
        .iter()
        .filter_map(|(face, (vertices, edges))| {
            (vertices.contains(&fixture.changed_vertex) || edges.contains(&fixture.changed_edge))
                .then_some(*face)
        })
        .collect::<HashSet<_>>();
    assert!(!impacted_faces.is_empty());
    let unrelated_faces = baseline_footprints
        .keys()
        .filter(|face| !impacted_faces.contains(face))
        .copied()
        .collect::<HashSet<_>>();
    assert!(unrelated_faces.len() >= FACE_COUNT - 3);
    for face in unrelated_faces {
        assert_eq!(
            baseline_footprints.get(&face),
            changed_footprints.get(&face),
            "unrelated production FaceId and complete ID footprint must remain stable"
        );
    }

    let baseline_uncached =
        uncached(baseline, REQUESTED_ANGLE_DEGREES).expect("baseline no-cache diagnosis");
    assert!(baseline_uncached.continuous_clearance_certified());
    assert_eq!(
        baseline_uncached.continuous_certificate_model_id(),
        Some(STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
    );
    assert_eq!(baseline_uncached.positive_endpoint_exact_pair_calls(), 1);

    let runtime =
        PersistentPairProofCacheRuntimeV1::new(ProofCacheLimitsV1::default()).expect("runtime");
    let baseline_capture = runtime
        .capture_v1(binding(instance, identity, baseline, 1))
        .expect("baseline capture");
    let baseline_cold = cached(
        baseline,
        REQUESTED_ANGLE_DEGREES,
        PAPER_THICKNESS_MM,
        &runtime,
        &baseline_capture,
    )
    .expect("baseline cold diagnosis");
    assert_eq!(baseline_cold, baseline_uncached);
    let baseline_cold_progress = runtime.progress_v1().expect("baseline cold progress");
    assert_eq!(baseline_cold_progress.cache_hits, 0);
    assert_eq!(baseline_cold_progress.cold_proofs, 1);
    let baseline_warm = cached(
        baseline,
        REQUESTED_ANGLE_DEGREES,
        PAPER_THICKNESS_MM,
        &runtime,
        &baseline_capture,
    )
    .expect("baseline warm diagnosis");
    assert_eq!(baseline_warm, baseline_uncached);
    let baseline_warm_progress = runtime.progress_v1().expect("baseline warm progress");
    assert_eq!(baseline_warm_progress.cache_hits, 1);
    assert_eq!(baseline_warm_progress.cold_proofs, 0);
    assert_eq!(
        baseline_warm_progress.accounted_additive_work,
        baseline_cold_progress.accounted_additive_work
    );
    assert_eq!(
        baseline_warm_progress.accounted_maximum_work,
        baseline_cold_progress.accounted_maximum_work
    );

    let invalidation = runtime
        .begin_complete_edit_v1(
            BASELINE_TARGET_REVISION,
            CHANGED_TARGET_REVISION,
            vec![fixture.changed_vertex],
            vec![fixture.changed_edge],
            Vec::new(),
            cache_support::live_control(),
        )
        .expect("complete one-vertex/one-crease edit impact");
    assert!(invalidation.differential_retention_possible);
    assert!(
        runtime
            .advance_pose_authority_v1(CHANGED_TARGET_REVISION)
            .expect("regenerated pose authority")
            .differential_retention_possible
    );
    assert_eq!(
        cached(
            baseline,
            REQUESTED_ANGLE_DEGREES,
            PAPER_THICKNESS_MM,
            &runtime,
            &baseline_capture,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult)
    );

    let changed_uncached =
        uncached(changed, REQUESTED_ANGLE_DEGREES).expect("changed no-cache diagnosis");
    let changed_capture = runtime
        .capture_v1(binding(instance, identity, changed, 2))
        .expect("changed capture");
    let changed_rebound = cached(
        changed,
        REQUESTED_ANGLE_DEGREES,
        PAPER_THICKNESS_MM,
        &runtime,
        &changed_capture,
    )
    .expect("changed differential diagnosis");
    assert_eq!(changed_rebound, changed_uncached);
    let rebound_progress = runtime.progress_v1().expect("rebind progress");
    assert_eq!(rebound_progress.cache_hits, 0);
    assert_eq!(
        rebound_progress.cold_proofs,
        changed_uncached.positive_endpoint_exact_pair_calls()
    );
    assert_eq!(rebound_progress.cold_proofs, 1);
    let changed_warm = cached(
        changed,
        REQUESTED_ANGLE_DEGREES,
        PAPER_THICKNESS_MM,
        &runtime,
        &changed_capture,
    )
    .expect("changed warm diagnosis");
    assert_eq!(changed_warm, changed_uncached);
    assert_eq!(
        runtime
            .progress_v1()
            .expect("changed warm progress")
            .cache_hits,
        1
    );

    let one_ulp_thicker = f64::from_bits(PAPER_THICKNESS_MM.to_bits() + 1);
    assert_eq!(
        cached(
            changed,
            REQUESTED_ANGLE_DEGREES,
            one_ulp_thicker,
            &runtime,
            &changed_capture,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable),
        "exact paper-thickness bits must be rebound before lookup"
    );

    let alternate_request = REQUESTED_ANGLE_DEGREES / 2.0;
    let alternate_cold = cached(
        changed,
        alternate_request,
        PAPER_THICKNESS_MM,
        &runtime,
        &changed_capture,
    )
    .expect("alternate issuer cold diagnosis");
    let alternate_cold_progress = runtime.progress_v1().expect("alternate issuer progress");
    assert_eq!(alternate_cold_progress.cache_hits, 0);
    assert_eq!(alternate_cold_progress.cold_proofs, 1);
    let alternate_warm = cached(
        changed,
        alternate_request,
        PAPER_THICKNESS_MM,
        &runtime,
        &changed_capture,
    )
    .expect("alternate issuer warm diagnosis");
    assert_eq!(alternate_warm, alternate_cold);
    assert_eq!(
        runtime
            .progress_v1()
            .expect("alternate warm progress")
            .cache_hits,
        1
    );
    assert_eq!(
        cached(
            changed,
            REQUESTED_ANGLE_DEGREES,
            PAPER_THICKNESS_MM,
            &runtime,
            &changed_capture,
        )
        .expect("original issuer after ABA"),
        changed_uncached,
        "issuer A -> B -> A must recover only A's exact proof"
    );

    let mut other_fingerprint = changed.fingerprint;
    other_fingerprint[0] ^= 1;
    let other_geometry_capture = runtime
        .capture_v1(
            ProofCacheRuntimeBindingV1::new(
                instance,
                identity,
                changed.revision,
                other_fingerprint,
                2,
                PAPER_THICKNESS_MM,
            )
            .expect("other geometry binding"),
        )
        .expect("other geometry capture");
    assert_eq!(
        runtime
            .progress_v1()
            .expect("geometry transition progress")
            .persistent_cached_pairs,
        0
    );
    assert_eq!(
        cached(
            changed,
            REQUESTED_ANGLE_DEGREES,
            PAPER_THICKNESS_MM,
            &runtime,
            &changed_capture,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult)
    );
    let restored_capture = runtime
        .capture_v1(binding(instance, identity, changed, 2))
        .expect("restored geometry capture");
    assert_eq!(
        cached(
            changed,
            REQUESTED_ANGLE_DEGREES,
            PAPER_THICKNESS_MM,
            &runtime,
            &other_geometry_capture,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult)
    );
    let restored = cached(
        changed,
        REQUESTED_ANGLE_DEGREES,
        PAPER_THICKNESS_MM,
        &runtime,
        &restored_capture,
    )
    .expect("geometry ABA cold diagnosis");
    assert_eq!(restored, changed_uncached);
    let restored_progress = runtime.progress_v1().expect("geometry ABA progress");
    assert_eq!(restored_progress.cache_hits, 0);
    assert_eq!(restored_progress.cold_proofs, 1);
}
