use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ori_kinematics::{MaterialTreeDyadicIntervalErrorV1, MaterialTreeDyadicIntervalLimitsV1};

use super::*;

#[test]
fn three_face_leaf_count_cannot_exceed_the_global_dyadic_hard_cap() {
    let maximum = super::super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1;
    assert!(maximum.is_power_of_two());
    let maximum_depth =
        u8::try_from(maximum.ilog2()).expect("the global leaf cap has a bounded depth");
    assert_eq!(
        super::super::leaf_count_v1(maximum_depth, maximum),
        Some(maximum)
    );
    assert_eq!(
        super::super::leaf_count_v1(
            maximum_depth,
            maximum
                .checked_add(1)
                .expect("the hard cap has a successor"),
        ),
        None
    );
    assert_eq!(
        super::super::leaf_count_v1(
            maximum_depth
                .checked_add(1)
                .expect("the bounded depth has a successor"),
            maximum,
        ),
        None
    );
}

#[test]
fn missing_three_face_registry_pair_yields_to_typed_stop_only_when_stopped() {
    let LayeredThreeFaceFixtureV1 {
        model,
        source_pose,
        target_angles,
        limits,
        ..
    } = layered_three_face_fixture_v1();
    let registry = model
        .prepare_dyadic_face_vertex_intervals_v1(
            &source_pose,
            &target_angles,
            limits.dyadic_depth,
            0,
            limits.interval_limits,
        )
        .expect("bounded three-face registry");
    let missing_pair = [FaceId::new(), FaceId::new()];
    assert_eq!(
        super::super::strictly_separated_registry_pair_with_control_v1(
            &registry,
            missing_pair,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(false),
        "an unstopped missing registry pair remains unavailable"
    );

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        super::super::strictly_separated_registry_pair_with_control_v1(
            &registry,
            missing_pair,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    );
    assert_eq!(
        super::super::strictly_separated_registry_pair_with_control_v1(
            &registry,
            missing_pair,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::DeadlineExceeded)
    );
}

#[test]
fn layered_resource_preflight_accepts_exact_caps_and_rejects_every_one_over() {
    let interval = MaterialTreeDyadicIntervalLimitsV1 {
        max_faces: 3,
        max_hinges: 2,
        max_vertices: super::super::MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1,
        max_interval_work: super::super::MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1,
        max_total_interval_work: super::super::MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1,
    };
    let hard = super::super::LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1;
    assert_eq!(hard, StaticCollisionLimits::default());
    assert_eq!(
        interval.max_vertices,
        MaterialTreeDyadicIntervalLimitsV1::default().max_vertices
    );
    assert_eq!(
        interval.max_interval_work,
        MaterialTreeDyadicIntervalLimitsV1::default().max_interval_work
    );
    assert_eq!(
        interval.max_total_interval_work,
        MaterialTreeDyadicIntervalLimitsV1::default().max_total_interval_work
    );
    assert!(
        super::super::layered_continuous_resource_limits_within_hard_caps_v1(interval, 3, 2, hard,)
    );

    for one_over in [
        MaterialTreeDyadicIntervalLimitsV1 {
            max_faces: 4,
            ..interval
        },
        MaterialTreeDyadicIntervalLimitsV1 {
            max_hinges: 3,
            ..interval
        },
        MaterialTreeDyadicIntervalLimitsV1 {
            max_vertices: interval.max_vertices + 1,
            ..interval
        },
        MaterialTreeDyadicIntervalLimitsV1 {
            max_interval_work: interval.max_interval_work + 1,
            ..interval
        },
        MaterialTreeDyadicIntervalLimitsV1 {
            max_total_interval_work: interval.max_total_interval_work + 1,
            ..interval
        },
    ] {
        assert!(
            !super::super::layered_continuous_resource_limits_within_hard_caps_v1(
                one_over, 3, 2, hard,
            )
        );
    }

    macro_rules! assert_static_one_over_rejected {
        ($($field:ident),+ $(,)?) => {
            $(
                let one_over = StaticCollisionLimits {
                    $field: hard
                        .$field
                        .checked_add(1)
                        .expect("the layered static hard cap has a successor"),
                    ..hard
                };
                assert!(
                    !super::super::layered_continuous_resource_limits_within_hard_caps_v1(
                        interval,
                        3,
                        2,
                        one_over,
                    ),
                    "oversized {} must be rejected",
                    stringify!($field),
                );
            )+
        };
    }
    assert_static_one_over_rejected!(
        max_faces,
        max_unordered_face_pairs,
        max_boundary_vertices_per_face,
        max_total_boundary_vertices,
        max_triangles_per_face,
        max_total_triangles,
        max_triangulation_work_per_face,
        max_total_triangulation_work,
        max_registry_authentication_work,
        max_triangle_pairs_per_face_pair,
        max_total_triangle_pairs,
        max_boundary_relation_work_per_face_pair,
        max_total_boundary_relation_work,
        max_rational_input_bits,
        max_total_rational_input_storage_bits,
        max_total_rational_retained_clone_bits,
        max_rational_operations,
        max_rational_intermediate_bits,
        max_rational_gcd_fallback_calls,
        max_rational_gcd_fallback_input_bits,
        max_rational_allocations,
        max_rational_allocation_bits,
        max_total_rational_allocation_bits,
        max_rational_output_bits,
        max_total_rational_output_bits,
        max_shared_hinge_boundary_diagnostics,
        max_shared_hinge_solid_diagnostics,
    );
}

#[test]
fn three_face_issuer_and_revalidation_enforce_layered_hard_caps() {
    let LayeredThreeFaceFixtureV1 {
        model,
        source_pose,
        target_angles,
        admission,
        limits,
        ..
    } = layered_three_face_fixture_v1();
    let exact_caps = LayeredThreeFaceContinuousLimitsV1 {
        max_leaves: super::super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1,
        interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
            max_faces: 3,
            max_hinges: 2,
            max_vertices: super::super::MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1,
            max_interval_work: super::super::MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1,
            max_total_interval_work: super::super::MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1,
        },
        static_limits: super::super::LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1,
        ..limits
    };
    let certificate = certify_layered_three_face_continuous_path_with_control_v1(
        &model,
        &source_pose,
        &target_angles,
        &admission,
        exact_caps,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("the exact layered hard caps must issue");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            exact_caps,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(true)
    );

    for one_over in [
        LayeredThreeFaceContinuousLimitsV1 {
            max_leaves: exact_caps.max_leaves + 1,
            ..exact_caps
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_faces: exact_caps.interval_limits.max_faces + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_hinges: exact_caps.interval_limits.max_hinges + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_vertices: exact_caps.interval_limits.max_vertices + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_interval_work: exact_caps.interval_limits.max_interval_work + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_total_interval_work: exact_caps.interval_limits.max_total_interval_work + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredThreeFaceContinuousLimitsV1 {
            static_limits: StaticCollisionLimits {
                max_total_rational_allocation_bits: exact_caps
                    .static_limits
                    .max_total_rational_allocation_bits
                    + 1,
                ..exact_caps.static_limits
            },
            ..exact_caps
        },
    ] {
        assert!(matches!(
            certify_layered_three_face_continuous_path_with_control_v1(
                &model,
                &source_pose,
                &target_angles,
                &admission,
                one_over,
                &CooperativeOperationControlV1::unbounded(),
            ),
            Err(LayeredThreeFaceContinuousErrorV1::ResourceLimit)
        ));
        assert_eq!(
            certificate.is_for_with_control_v1(
                &model,
                &source_pose,
                &target_angles,
                &admission,
                one_over,
                &CooperativeOperationControlV1::unbounded(),
            ),
            Ok(false)
        );
    }
}

#[test]
fn three_face_certificate_has_exact_and_one_short_resource_limits() {
    let LayeredThreeFaceFixtureV1 {
        model,
        source_pose,
        target_angles,
        admission,
        limits,
        ..
    } = layered_three_face_fixture_v1();
    let interval_limit_template = MaterialTreeDyadicIntervalLimitsV1 {
        max_faces: 3,
        max_hinges: 2,
        max_vertices: 8,
        max_interval_work: limits.interval_limits.max_interval_work,
        max_total_interval_work: 18,
    };
    let mut lowest_work = 0_usize;
    let mut highest_work = interval_limit_template.max_interval_work;
    assert!(
        model
            .prepare_dyadic_face_vertex_intervals_v1(
                &source_pose,
                &target_angles,
                0,
                0,
                MaterialTreeDyadicIntervalLimitsV1 {
                    max_interval_work: highest_work,
                    ..interval_limit_template
                },
            )
            .is_ok(),
        "fixture interval work must stay within the bounded exact-work probe"
    );
    while lowest_work < highest_work {
        let candidate_work = lowest_work + (highest_work - lowest_work) / 2;
        let candidate_limits = MaterialTreeDyadicIntervalLimitsV1 {
            max_interval_work: candidate_work,
            ..interval_limit_template
        };
        match model.prepare_dyadic_face_vertex_intervals_v1(
            &source_pose,
            &target_angles,
            0,
            0,
            candidate_limits,
        ) {
            Ok(_) => highest_work = candidate_work,
            Err(MaterialTreeDyadicIntervalErrorV1::ResourceLimit) => {
                lowest_work = candidate_work + 1;
            }
            Err(error) => panic!("unexpected interval-work probe error: {error:?}"),
        }
    }
    assert!(lowest_work > 0, "the fixture must require interval work");
    let exact_interval_limits = MaterialTreeDyadicIntervalLimitsV1 {
        max_interval_work: lowest_work,
        ..interval_limit_template
    };
    let exact_limits = LayeredThreeFaceContinuousLimitsV1 {
        dyadic_depth: 0,
        max_leaves: 1,
        interval_limits: exact_interval_limits,
        static_limits: StaticCollisionLimits {
            max_faces: 3,
            ..limits.static_limits
        },
        ..limits
    };
    assert!(
        certify_layered_three_face_continuous_path_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            exact_limits,
            &CooperativeOperationControlV1::unbounded(),
        )
        .is_ok()
    );
    for one_short in [
        LayeredThreeFaceContinuousLimitsV1 {
            max_leaves: 0,
            ..exact_limits
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_faces: 2,
                ..exact_interval_limits
            },
            ..exact_limits
        },
        LayeredThreeFaceContinuousLimitsV1 {
            static_limits: StaticCollisionLimits {
                max_faces: 2,
                ..exact_limits.static_limits
            },
            ..exact_limits
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_hinges: 1,
                ..exact_interval_limits
            },
            ..exact_limits
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_vertices: 7,
                ..exact_interval_limits
            },
            ..exact_limits
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_interval_work: lowest_work - 1,
                ..exact_interval_limits
            },
            ..exact_limits
        },
        LayeredThreeFaceContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_total_interval_work: 17,
                ..exact_interval_limits
            },
            ..exact_limits
        },
    ] {
        assert!(matches!(
            certify_layered_three_face_continuous_path_with_control_v1(
                &model,
                &source_pose,
                &target_angles,
                &admission,
                one_short,
                &CooperativeOperationControlV1::unbounded(),
            ),
            Err(LayeredThreeFaceContinuousErrorV1::ResourceLimit)
        ));
    }
}

#[test]
fn three_face_certificate_stops_mid_operation_and_rejects_stale_generation() {
    let pre_admission_model = three_face_two_hinge_model_v1();
    let (pre_admission_angles, _) = schedule_v1(&pre_admission_model, 0.0, 45.0);
    let pre_admission_pose = pre_admission_model
        .solve(
            Some(pre_admission_model.face_ids()[0]),
            &pre_admission_angles,
        )
        .expect("fixture source pose");
    let pre_cancel_source = admission_source_v1(&pre_admission_model, &pre_admission_pose);
    let pre_cancel = AtomicBool::new(true);
    assert!(matches!(
        prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
            &pre_admission_model,
            &pre_admission_pose,
            0.0,
            StaticCollisionLimits::default(),
            &pre_cancel_source,
            &CooperativeOperationControlV1::new(
                Some(&pre_cancel),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::Cancelled)
    ));
    assert_eq!(pre_cancel_source.observed.get(), 0);
    let deadline_source = admission_source_v1(&pre_admission_model, &pre_admission_pose);
    assert!(matches!(
        prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
            &pre_admission_model,
            &pre_admission_pose,
            0.0,
            StaticCollisionLimits::default(),
            &deadline_source,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::DeadlineExceeded)
    ));
    assert_eq!(deadline_source.observed.get(), 0);

    let LayeredThreeFaceFixtureV1 {
        model,
        source_pose,
        target_angles,
        admission,
        limits,
        ..
    } = layered_three_face_fixture_v1();
    let issue_leaf_cancelled = Arc::new(AtomicBool::new(false));
    let issue_hook_cleanup = arm_layered_three_face_test_checkpoint_hook_v1(
        LayeredThreeFaceTestCheckpointHookV1::Cancel {
            phase: LayeredThreeFaceTestCheckpointPhaseV1::IssueLeaf,
            signal: Arc::clone(&issue_leaf_cancelled),
        },
    );
    assert!(matches!(
        certify_layered_three_face_continuous_path_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(
                Some(&issue_leaf_cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    ));
    drop(issue_hook_cleanup);
    let issue_cancelled = AtomicBool::new(true);
    assert!(matches!(
        certify_layered_three_face_continuous_path_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(
                Some(&issue_cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    ));
    assert!(matches!(
        certify_layered_three_face_continuous_path_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::DeadlineExceeded)
    ));
    let partition = matches_three_face_schedule_v1(&model, &source_pose, &target_angles)
        .expect("fixture pair partition");
    let registry = model
        .prepare_dyadic_face_vertex_intervals_with_checkpoint_v1(
            &source_pose,
            &target_angles,
            limits.dyadic_depth,
            0,
            limits.interval_limits,
            || true,
        )
        .expect("controlled fixture dyadic interval registry");
    assert!(strictly_separated_registry_pair_v1(
        &registry,
        partition.nonadjacent_pair
    ));
    let interval_checkpoints = Cell::new(0_usize);
    assert!(matches!(
        model.prepare_dyadic_face_vertex_intervals_with_checkpoint_v1(
            &source_pose,
            &target_angles,
            limits.dyadic_depth,
            0,
            limits.interval_limits,
            || {
                let next = interval_checkpoints.get() + 1;
                interval_checkpoints.set(next);
                next < 12
            },
        ),
        Err(MaterialTreeDyadicIntervalErrorV1::Cancelled)
    ));
    assert_eq!(
        interval_checkpoints.get(),
        12,
        "the twelfth native interval checkpoint is the first emitted vertex"
    );
    let certificate = certify_layered_three_face_continuous_path_with_control_v1(
        &model,
        &source_pose,
        &target_angles,
        &admission,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("controlled dyadic three-face certificate");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(true)
    );

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::DeadlineExceeded)
    );

    let leaf_cancelled = Arc::new(AtomicBool::new(false));
    let leaf_hook_cleanup = arm_layered_three_face_test_checkpoint_hook_v1(
        LayeredThreeFaceTestCheckpointHookV1::Cancel {
            phase: LayeredThreeFaceTestCheckpointPhaseV1::RevalidationLeaf,
            signal: Arc::clone(&leaf_cancelled),
        },
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(
                Some(&leaf_cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    );
    drop(leaf_hook_cleanup);

    let axis_entered = Arc::new(AtomicBool::new(false));
    let axis_hook_cleanup = arm_layered_three_face_test_checkpoint_hook_v1(
        LayeredThreeFaceTestCheckpointHookV1::Deadline {
            phase: LayeredThreeFaceTestCheckpointPhaseV1::NonadjacentAxis,
            entered: Arc::clone(&axis_entered),
        },
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(None, Instant::now() + Duration::from_secs(1),),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::DeadlineExceeded)
    );
    assert!(axis_entered.load(Ordering::Acquire));
    drop(axis_hook_cleanup);

    let vertex_cancelled = Arc::new(AtomicBool::new(false));
    let vertex_hook_cleanup = arm_layered_three_face_test_checkpoint_hook_v1(
        LayeredThreeFaceTestCheckpointHookV1::Cancel {
            phase: LayeredThreeFaceTestCheckpointPhaseV1::NonadjacentVertex,
            signal: Arc::clone(&vertex_cancelled),
        },
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(
                Some(&vertex_cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    );
    drop(vertex_hook_cleanup);

    let final_cancelled = Arc::new(AtomicBool::new(false));
    let final_hook_cleanup = arm_layered_three_face_test_checkpoint_hook_v1(
        LayeredThreeFaceTestCheckpointHookV1::Cancel {
            phase: LayeredThreeFaceTestCheckpointPhaseV1::FinalRevalidation,
            signal: Arc::clone(&final_cancelled),
        },
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new(
                Some(&final_cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    );
    drop(final_hook_cleanup);

    let generation = AtomicU64::new(9);
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new_with_generation(
                None,
                &generation,
                8,
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredThreeFaceContinuousErrorV1::Cancelled)
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::new_with_generation(
                None,
                &generation,
                9,
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Ok(true)
    );
}
