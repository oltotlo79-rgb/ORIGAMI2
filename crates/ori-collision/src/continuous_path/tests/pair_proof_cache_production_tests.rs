use ori_domain::ProjectId;

use super::*;

#[test]
fn model4_real_diagnostic_cold_and_hit_are_bit_exact_in_result_and_all_work() {
    use std::time::{Duration, Instant};

    let model = branched_triangle_model(6, false);
    let (moving, initial) = zero_tree_pose(&model);
    let requested = positive_tree_max_angle_degrees_v1(model.hinges().len()).unwrap();
    assert_eq!(requested.to_bits(), 30.0_f64.to_bits());
    let limits = StackedFoldPathDiagnosticLimitsV1 {
        sample_intervals: 1,
        ..StackedFoldPathDiagnosticLimitsV1::default()
    };
    let uncached =
        diagnose_collective_hinge_path_v1(&model, &initial, &moving, requested, 1.0, limits)
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
                1.0,
            )
            .expect("binding"),
        )
        .expect("capture");
    let cold = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &moving,
        requested,
        1.0,
        limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30)),
    )
    .expect("real cold model-4 diagnosis");
    assert_eq!(cold, uncached);
    assert!(cold.continuous_clearance_certified());
    assert_eq!(cold.positive_endpoint_exact_pair_calls(), 1);
    let cold_progress = runtime.progress_v1().expect("cold progress");
    assert_eq!(cold_progress.cold_proofs, 1);
    assert_eq!(cold_progress.cache_hits, 0);

    let hit = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &moving,
        requested,
        1.0,
        limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30)),
    )
    .expect("real cache-hit model-4 diagnosis");
    let hit_progress = runtime.progress_v1().expect("hit progress");
    assert_eq!(hit, cold);
    assert_eq!(hit_progress.cold_proofs, 0);
    assert_eq!(hit_progress.cache_hits, 1);
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

    let mut nondefault_limits = limits;
    nondefault_limits.static_collision.max_faces -= 1;
    let nondefault_uncached = diagnose_collective_hinge_path_v1(
        &model,
        &initial,
        &moving,
        requested,
        1.0,
        nondefault_limits,
    )
    .expect("non-default limits remain sufficient without cache");
    let nondefault_routed = diagnose_collective_hinge_path_with_pair_cache_v1(
        &model,
        &initial,
        &moving,
        requested,
        1.0,
        nondefault_limits,
        &runtime,
        &capture,
        crate::ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30)),
    )
    .expect("non-default limits bypass cache without changing the result");
    assert_eq!(nondefault_routed, nondefault_uncached);
    assert_eq!(
        runtime.progress_v1().expect("bypass progress"),
        hit_progress,
        "every non-default static limit must leave cache progress untouched"
    );

    let mut tight_limits = limits;
    tight_limits.static_collision.max_faces = model.face_ids().len() - 1;
    assert_eq!(
        diagnose_collective_hinge_path_v1(&model, &initial, &moving, requested, 1.0, tight_limits,),
        Err(StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)
    );
    assert_eq!(
        diagnose_collective_hinge_path_with_pair_cache_v1(
            &model,
            &initial,
            &moving,
            requested,
            1.0,
            tight_limits,
            &runtime,
            &capture,
            crate::ProofCacheOperationControlV1::new(
                None,
                Instant::now() + Duration::from_secs(30),
            ),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)
    );
    assert_eq!(
        runtime
            .progress_v1()
            .expect("unchanged tight-limit progress"),
        hit_progress,
        "a warm cache must not widen caller-supplied static limits"
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
        Some(STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
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
