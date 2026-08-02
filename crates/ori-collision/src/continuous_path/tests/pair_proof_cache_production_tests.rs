use ori_domain::ProjectId;

use super::*;

#[test]
fn model4_penetrating_exact_pair_is_fail_closed_and_never_published() {
    use std::time::{Duration, Instant};

    const THICKNESS_MM: f64 = 3.0;
    // This test validates cache equivalence, not deadline expiry.  The exact
    // model-4 kernel can exceed 30 seconds when the full suite loads the host.
    const NON_DEADLINE_TIMEOUT: Duration = Duration::from_secs(300);

    let model = branched_triangle_model(6, false);
    let (moving, initial) = zero_tree_pose(&model);
    let requested = positive_tree_max_angle_degrees_v1(model.hinges().len()).unwrap();
    assert_eq!(requested.to_bits(), 30.0_f64.to_bits());
    let limits = StackedFoldPathDiagnosticLimitsV1 {
        sample_intervals: 1,
        ..StackedFoldPathDiagnosticLimitsV1::default()
    };
    let uncached = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        requested,
        THICKNESS_MM,
        limits,
    )
    .expect("real no-cache model-4 diagnosis");
    let runtime =
        crate::PersistentPairProofCacheRuntimeV1::new(crate::ProofCacheLimitsV1::default())
            .expect("runtime");
    let capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                ProjectId::new(),
                ProjectId::new(),
                1,
                [0x73; 32],
                1,
                THICKNESS_MM,
            )
            .expect("binding"),
        )
        .expect("capture");
    let cold = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &moving,
        requested,
        THICKNESS_MM,
        limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + NON_DEADLINE_TIMEOUT),
    )
    .expect("real cold model-4 diagnosis");
    assert_eq!(cold, uncached);
    assert!(!cold.continuous_clearance_certified());
    assert_eq!(cold.positive_endpoint_exact_pair_calls(), 1);
    let cold_progress = runtime.progress_v1().expect("cold progress");
    assert_eq!(
        cold_progress,
        crate::ProofCacheProgressV1 {
            epoch: capture.epoch(),
            ..crate::ProofCacheProgressV1::default()
        },
        "a penetrating exact observation must not be published as non-blocking"
    );

    let repeated = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &moving,
        requested,
        THICKNESS_MM,
        limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + NON_DEADLINE_TIMEOUT),
    )
    .expect("repeated fail-closed model-4 diagnosis");
    assert_eq!(repeated, cold);
    assert_eq!(
        runtime.progress_v1().expect("repeated progress"),
        cold_progress
    );

    let mut nondefault_limits = limits;
    nondefault_limits.static_collision.max_faces -= 1;
    let nondefault_uncached = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        requested,
        THICKNESS_MM,
        nondefault_limits,
    )
    .expect("non-default limits remain sufficient without cache");
    let nondefault_routed = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &moving,
        requested,
        THICKNESS_MM,
        nondefault_limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + NON_DEADLINE_TIMEOUT),
    )
    .expect("non-default limits bypass cache without changing the result");
    assert_eq!(
        nondefault_uncached, uncached,
        "a sufficient non-default limit must not revive the retired broadphase false-positive"
    );
    assert_eq!(nondefault_routed, nondefault_uncached);
    assert_eq!(
        runtime.progress_v1().expect("bypass progress"),
        cold_progress,
        "every non-default static limit must leave cache progress untouched"
    );

    let mut tight_limits = limits;
    tight_limits.static_collision.max_faces = model.face_ids().len() - 1;
    assert_eq!(
        diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            THICKNESS_MM,
            tight_limits,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)
    );
    assert_eq!(
        diagnose_collective_hinge_path_with_pair_cache_v1(
            &model,
            &initial,
            &moving,
            requested,
            THICKNESS_MM,
            tight_limits,
            &runtime,
            &capture,
            crate::ProofCacheOperationControlV1::new(
                None,
                Instant::now() + NON_DEADLINE_TIMEOUT,
            ),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)
    );
    assert_eq!(
        runtime
            .progress_v1()
            .expect("unchanged tight-limit progress"),
        cold_progress,
        "an unpublished blocking observation must not widen caller-supplied static limits"
    );

    for (label, tight_static) in [
        (
            "shared-hinge diagnostic budget",
            crate::StaticCollisionLimits {
                max_shared_hinge_solid_diagnostics: model.hinges().len() - 1,
                ..crate::StaticCollisionLimits::default()
            },
        ),
        (
            "full-snapshot triangle budget",
            crate::StaticCollisionLimits {
                max_total_triangles: model.face_ids().len().checked_mul(2).unwrap() - 1,
                ..crate::StaticCollisionLimits::default()
            },
        ),
    ] {
        let active_limits = StackedFoldPathDiagnosticLimitsV1 {
            static_collision: tight_static,
            ..limits
        };
        let active_uncached = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            THICKNESS_MM,
            active_limits,
        )
        .expect("pair-kernel-specific limit returns a fail-closed diagnosis");
        assert!(
            !active_uncached.continuous_clearance_certified(),
            "{label} must remain active on the uncached exact-pair path"
        );
        let active_routed = diagnose_collective_hinge_path_with_pair_cache_v1(
            &model,
            &initial,
            &moving,
            requested,
            THICKNESS_MM,
            active_limits,
            &runtime,
            &capture,
            crate::ProofCacheOperationControlV1::new(
                None,
                Instant::now() + NON_DEADLINE_TIMEOUT,
            ),
        )
        .expect("non-default pair-kernel limit bypasses the cache");
        assert_eq!(active_routed, active_uncached, "{label}");
        assert_eq!(
            runtime
                .progress_v1()
                .expect("unchanged pair-kernel-limit progress"),
            cold_progress,
            "{label} must not read or publish cache state"
        );
    }
}

#[test]
fn exact_separated_pair_cold_and_hit_are_bit_exact_in_all_work() {
    use std::time::{Duration, Instant};

    const THICKNESS_MM: f64 = 1.0;

    let model = branched_triangle_model(6, false);
    let (moving, initial) = zero_tree_pose(&model);
    let requested = positive_tree_max_angle_degrees_v1(model.hinges().len()).unwrap();
    let moving_set = moving.iter().copied().collect::<HashSet<_>>();
    let endpoint =
        solve_collective_pose(&model, &initial, &moving_set, requested).expect("endpoint");
    let broadphase_candidates =
        positive_endpoint_candidates_v1(&model, &endpoint, THICKNESS_MM).expect("candidates");
    let exact_pair = model
        .face_ids()
        .iter()
        .enumerate()
        .find_map(|(index, first)| {
            model.face_ids().iter().skip(index + 1).find_map(|second| {
                let adjacent = model.hinges().iter().any(|hinge| {
                    (hinge.left_face() == *first && hinge.right_face() == *second)
                        || (hinge.left_face() == *second && hinge.right_face() == *first)
                });
                (!adjacent
                    && !faces_share_material_vertex_v1(&model, *first, *second)
                    && !broadphase_candidates.contains(&(*first, *second)))
                .then_some((*first, *second))
            })
        })
        .expect("fixture has a strictly broadphase-separated non-adjacent pair");
    let exact_pairs = [exact_pair];
    let expected_pairs = model.face_ids().len() * (model.face_ids().len() - 1) / 2;
    let runtime =
        crate::PersistentPairProofCacheRuntimeV1::new(crate::ProofCacheLimitsV1::default())
            .expect("runtime");
    let capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                ProjectId::new(),
                ProjectId::new(),
                1,
                [0x77; 32],
                1,
                THICKNESS_MM,
            )
            .expect("binding"),
        )
        .expect("capture");
    let issuer_context = [0x78; 32];
    let prove = || {
        let cache = super::super::pair_proof_cache::PositiveEndpointPairCacheUseV1 {
            runtime: &runtime,
            capture: &capture,
            issuer_context,
            control: crate::ProofCacheOperationControlV1::new(
                None,
                Instant::now() + Duration::from_secs(30),
            ),
        };
        super::super::pair_proof_cache::prove_positive_endpoint_pairs_with_cache_v1(
            model.bind_pose(&endpoint).expect("bound endpoint"),
            THICKNESS_MM,
            &exact_pairs,
            expected_pairs,
            &cache,
        )
    };

    assert_eq!(prove(), Ok(true));
    let cold_progress = runtime.progress_v1().expect("cold progress");
    assert_eq!(cold_progress.proven_pairs, expected_pairs);
    assert_eq!(cold_progress.cold_proofs, 1);
    assert_eq!(cold_progress.cache_hits, 0);
    assert_eq!(cold_progress.persistent_cached_pairs, 1);

    assert_eq!(prove(), Ok(true));
    let hit_progress = runtime.progress_v1().expect("hit progress");
    assert_eq!(hit_progress.proven_pairs, expected_pairs);
    assert_eq!(hit_progress.cold_proofs, 0);
    assert_eq!(hit_progress.cache_hits, 1);
    assert_eq!(hit_progress.persistent_cached_pairs, 1);
    assert_eq!(
        hit_progress.accounted_additive_work,
        cold_progress.accounted_additive_work
    );
    assert_eq!(
        hit_progress.accounted_maximum_work,
        cold_progress.accounted_maximum_work
    );
    assert!(
        hit_progress
            .accounted_additive_work
            .iter()
            .any(|counter| *counter > 0)
    );
}

#[test]
fn positive_non_model4_path_bypasses_pair_cache_exactly() {
    use std::time::{Duration, Instant};

    let model = one_hinge_model();
    let edge = model.hinges()[0].edge();
    let angles = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
    let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let limits = StackedFoldPathDiagnosticLimitsV1::default();
    let uncached = diagnose_collective_hinge_path_v1(&model, &initial, &[edge], 37.0, 0.1, limits)
        .expect("single-hinge no-cache result");
    assert_eq!(
        uncached.continuous_certificate_model_id(),
        Some(STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2)
    );

    let runtime =
        crate::PersistentPairProofCacheRuntimeV1::new(crate::ProofCacheLimitsV1::default())
            .expect("runtime");
    let capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                ProjectId::new(),
                ProjectId::new(),
                1,
                [0x74; 32],
                1,
                0.1,
            )
            .expect("binding"),
        )
        .expect("capture");
    let routed = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &[edge],
        37.0,
        0.1,
        limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30)),
    )
    .expect("single-hinge cache-aware result");

    assert_eq!(routed, uncached);
    assert_eq!(
        runtime.progress_v1().expect("unused cache progress"),
        crate::ProofCacheProgressV1 {
            epoch: capture.epoch(),
            ..crate::ProofCacheProgressV1::default()
        }
    );
}

#[test]
fn empty_exact_pair_set_skips_snapshot_encoding_and_cache_transaction() {
    use std::time::{Duration, Instant};

    use super::super::pair_proof_cache;

    const THICKNESS_MM: f64 = 0.001;

    let model = sparse_triangle_strip_model(3);
    let (_, pose) = zero_tree_pose(&model);
    let bound = model.bind_pose(&pose).expect("bound sparse model-4 pose");
    let expected_pairs = model.face_ids().len() * (model.face_ids().len() - 1) / 2;
    let runtime =
        crate::PersistentPairProofCacheRuntimeV1::new(crate::ProofCacheLimitsV1::default())
            .expect("runtime");
    let capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                ProjectId::new(),
                ProjectId::new(),
                1,
                [0x75; 32],
                1,
                THICKNESS_MM,
            )
            .expect("binding"),
        )
        .expect("capture");
    let cache = pair_proof_cache::PositiveEndpointPairCacheUseV1 {
        runtime: &runtime,
        capture: &capture,
        issuer_context: [0x76; 32],
        control: crate::ProofCacheOperationControlV1::new(
            None,
            Instant::now() + Duration::from_secs(30),
        ),
    };
    let initial_progress = runtime.progress_v1().expect("initial progress");

    assert_eq!(
        pair_proof_cache::prove_positive_endpoint_pairs_with_cache_v1(
            bound,
            THICKNESS_MM,
            &[],
            expected_pairs,
            &cache,
        ),
        Ok(true)
    );
    assert_eq!(
        runtime.progress_v1().expect("unchanged progress"),
        initial_progress,
        "an empty exact-pair theorem must not prepare/publish a cache batch"
    );
    assert_eq!(
        pair_proof_cache::prove_positive_endpoint_pairs_with_cache_v1(
            bound,
            THICKNESS_MM * 2.0,
            &[],
            expected_pairs,
            &cache,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable),
        "the empty fast path must retain capture/thickness consistency"
    );
}
