fn set_zero_thickness_for_cycle_test_v1(project: &mut super::super::ProjectState) {
    let paper = project.editor.paper().clone();
    let revision = project.editor.revision();
    project
        .editor
        .execute(
            revision,
            ori_core::Command::UpdatePaperProperties {
                thickness_mm: 0.0,
                front_color: paper.front.color,
                back_color: paper.back.color,
                front_texture_asset: paper.front.texture_asset,
                back_texture_asset: paper.back.texture_asset,
                cutting_allowed: paper.cutting_allowed,
            },
        )
        .expect("set an explicit zero-thickness cycle fixture");
}

#[test]
fn exact_flat_endpoint_defers_until_zero_thickness_layer_order_diagnosis() {
    assert_eq!(
        endpoint_collision_plan_v1(180.0, false),
        EndpointCollisionPlanV1::DeferToFlatLayerOrder,
    );
    assert_eq!(
        endpoint_collision_plan_v1(180.0, true),
        EndpointCollisionPlanV1::DeferToFlatLayerOrder,
    );
    assert_eq!(
        FLAT_ENDPOINT_COLLISION_THICKNESS_MM_V1.to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn near_flat_endpoint_keeps_the_existing_static_collision_path() {
    assert_eq!(
        endpoint_collision_plan_v1(179.999, false),
        EndpointCollisionPlanV1::StaticGeometry,
    );
    assert_eq!(
        endpoint_collision_plan_v1(179.999, true),
        EndpointCollisionPlanV1::CertifiedPositiveThickness,
    );
}

#[test]
fn speculative_endpoint_gate_requires_all_three_nonblocking_observations() {
    let endpoint = |has_blocking_hold, penetrating_pair_count, indeterminate_pair_count| {
        StackedFoldEndpointCollisionDto {
            expected_pair_count: 1,
            separated_pair_count: 1,
            touching_pair_count: 0,
            allowed_pair_count: 0,
            penetrating_pair_count,
            indeterminate_pair_count,
            has_blocking_hold,
        }
    };
    assert!(endpoint_allows_speculative_apply_v1(&endpoint(false, 0, 0)));
    assert!(!endpoint_allows_speculative_apply_v1(&endpoint(true, 0, 0)));
    assert!(!endpoint_allows_speculative_apply_v1(&endpoint(
        false, 1, 0
    )));
    assert!(!endpoint_allows_speculative_apply_v1(&endpoint(
        false, 0, 1
    )));
}

#[test]
fn initial_layer_order_endpoint_admission_is_exact_and_fail_closed() {
    let endpoint = |expected_pair_count,
                    separated_pair_count,
                    touching_pair_count,
                    allowed_pair_count,
                    penetrating_pair_count,
                    indeterminate_pair_count,
                    has_blocking_hold| StackedFoldEndpointCollisionDto {
        expected_pair_count,
        separated_pair_count,
        touching_pair_count,
        allowed_pair_count,
        penetrating_pair_count,
        indeterminate_pair_count,
        has_blocking_hold,
    };

    let clear = admit_initial_layer_order_endpoint_v1(endpoint(4, 2, 1, 1, 0, 0, false), false)
        .expect("an exactly accounted clear endpoint remains clear");
    assert_eq!(clear.separated_pair_count, 2);
    assert_eq!(clear.touching_pair_count, 1);
    assert_eq!(clear.allowed_pair_count, 1);
    assert_eq!(clear.indeterminate_pair_count, 0);
    assert!(!clear.has_blocking_hold);

    let admitted = admit_initial_layer_order_endpoint_v1(endpoint(4, 1, 1, 0, 0, 2, true), true)
        .expect("the authenticated initial layer order admits its persistent flat pairs");
    assert_eq!(admitted.separated_pair_count, 1);
    assert_eq!(admitted.touching_pair_count, 1);
    assert_eq!(admitted.allowed_pair_count, 2);
    assert_eq!(admitted.indeterminate_pair_count, 0);
    assert!(!admitted.has_blocking_hold);

    assert!(
        admit_initial_layer_order_endpoint_v1(endpoint(1, 0, 0, 0, 0, 1, true), false,).is_none(),
        "a raw indeterminate endpoint requires the exact admitted path"
    );
    assert!(
        admit_initial_layer_order_endpoint_v1(endpoint(1, 0, 0, 0, 1, 0, true), true).is_none(),
        "penetration remains blocking under every layer-order admission"
    );
    assert!(
        admit_initial_layer_order_endpoint_v1(endpoint(2, 1, 0, 0, 0, 0, false), true).is_none(),
        "an unaccounted candidate-only pair must fail closed"
    );
    assert!(
        admit_initial_layer_order_endpoint_v1(endpoint(1, 0, 0, 0, 0, 1, false), true).is_none(),
        "the raw blocking flag must agree with the pair dispositions"
    );
    assert!(
        admit_initial_layer_order_endpoint_v1(
            endpoint(usize::MAX, usize::MAX, 0, 1, 0, 0, false),
            true,
        )
        .is_none(),
        "count overflow must fail closed"
    );
}

#[test]
fn native_positive_thickness_runtime_accepts_only_v2_model_ids() {
    assert!(is_positive_thickness_continuous_certificate_model_id_v2(
        Some(
            ori_collision::STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
        ),
    ));
    assert!(is_positive_thickness_continuous_certificate_model_id_v2(
        Some(
            ori_collision::STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
        ),
    ));
    for rejected in [
        None,
        Some("stacked_fold_single_hinge_positive_thickness_continuous_certificate_v1"),
        Some("stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1"),
        Some("forged_positive_thickness_continuous_certificate_v2"),
    ] {
        assert!(!is_positive_thickness_continuous_certificate_model_id_v2(
            rejected
        ));
    }
}

#[test]
fn certified_path_graph_thickness_preflight_allows_only_signed_zero() {
    for thickness in [0.0_f64, -0.0_f64] {
        assert_eq!(
            preflight_certified_path_graph_thickness_v1(thickness),
            Ok(()),
        );
    }
    for thickness in [f64::MIN_POSITIVE, 0.1, f64::MAX] {
        assert_eq!(
            preflight_certified_path_graph_thickness_v1(thickness),
            Err(ScheduledCycleThicknessDiagnosticErrorV1::PositiveThicknessUnsupported),
        );
    }
    for thickness in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1] {
        assert_eq!(
            preflight_certified_path_graph_thickness_v1(thickness),
            Err(ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness),
        );
    }
}

#[test]
fn blockwise_fallback_preserves_operational_errors_and_normalizes_proof_failures() {
    for preserved in [CANCELLED_MESSAGE, CYCLE_PATH_RESOURCE_MESSAGE] {
        assert_eq!(
            normalize_blockwise_current_cycle_fallback_error_v1(preserved.to_owned()),
            preserved,
        );
    }
    for proof_failure in [
        CYCLE_NONCLOSING_MESSAGE,
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
        CYCLE_PATH_UNSUPPORTED_MESSAGE,
    ] {
        assert_eq!(
            normalize_blockwise_current_cycle_fallback_error_v1(proof_failure.to_owned()),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
        );
    }
}

#[test]
fn scheduled_cycle_thickness_dispatch_preserves_signed_zero_and_exact_positive_model() {
    use std::cell::Cell;

    type Diagnostic = (Option<&'static str>, Option<u64>);
    for thickness in [0.0_f64, -0.0_f64] {
        let support_calls = Cell::new(0);
        let zero_calls = Cell::new(0);
        let positive_calls = Cell::new(0);
        let observed = diagnose_scheduled_cycle_path_for_thickness_v1(
            thickness,
            || {
                support_calls.set(support_calls.get() + 1);
                false
            },
            || {
                zero_calls.set(zero_calls.get() + 1);
                (
                    Some(
                        ori_collision::STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
                    ),
                    None,
                )
            },
            |_| {
                positive_calls.set(positive_calls.get() + 1);
                (None, None)
            },
            |diagnostic: &Diagnostic| *diagnostic,
        )
        .expect("signed zero keeps the established zero-thickness oracle");
        assert_eq!(
            observed,
            (
                Some(ori_collision::STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,),
                None,
            )
        );
        assert_eq!(support_calls.get(), 0);
        assert_eq!(zero_calls.get(), 1);
        assert_eq!(positive_calls.get(), 0);
    }
    for rejected in [
        (None, None),
        (
            Some(
                ori_collision::STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
            ),
            None,
        ),
        (
            Some(
                ori_collision::STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
            ),
            Some(0.0_f64.to_bits()),
        ),
    ] {
        assert_eq!(
            diagnose_scheduled_cycle_path_for_thickness_v1(
                0.0,
                || panic!("zero thickness must not query positive support"),
                || rejected,
                |_| panic!("zero thickness must not invoke the positive oracle"),
                |diagnostic: &Diagnostic| *diagnostic,
            ),
            Err(ScheduledCycleThicknessDiagnosticErrorV1::Uncertified),
        );
    }

    let thickness = f64::from_bits(0x3fb9_9999_9999_999a);
    let positive_calls = Cell::new(0);
    let observed = diagnose_scheduled_cycle_path_for_thickness_v1(
        thickness,
        || true,
        || panic!("positive thickness must not invoke the zero-thickness oracle"),
        |observed_thickness| {
            positive_calls.set(positive_calls.get() + 1);
            assert_eq!(observed_thickness.to_bits(), thickness.to_bits());
            (
                Some(
                    ori_collision::STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
                ),
                Some(observed_thickness.to_bits()),
            )
        },
        |diagnostic: &Diagnostic| *diagnostic,
    )
    .expect("the exact cycle positive-thickness model and binding are admitted");
    assert_eq!(observed.1, Some(thickness.to_bits()));
    assert_eq!(positive_calls.get(), 1);

    for rejected in [
        (
            Some(
                ori_collision::STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
            ),
            Some(thickness.to_bits()),
        ),
        (
            Some(
                ori_collision::STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
            ),
            Some(1.0_f64.to_bits()),
        ),
        (
            Some(
                ori_collision::STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
            ),
            Some(thickness.to_bits()),
        ),
        (None, Some(thickness.to_bits())),
    ] {
        assert_eq!(
            diagnose_scheduled_cycle_path_for_thickness_v1(
                thickness,
                || true,
                || panic!("positive thickness must not invoke the zero-thickness oracle"),
                |_| rejected,
                |diagnostic: &Diagnostic| *diagnostic,
            ),
            Err(ScheduledCycleThicknessDiagnosticErrorV1::Uncertified),
        );
    }
}

#[test]
fn unsupported_cycle_thickness_stops_before_work_cancel_and_publication_gates() {
    use std::cell::Cell;

    type Diagnostic = (Option<&'static str>, Option<u64>);
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let generation = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    for (thickness, expected_error, expected_support_calls) in [
        (
            0.1,
            ScheduledCycleThicknessDiagnosticErrorV1::PositiveThicknessUnsupported,
            1,
        ),
        (
            f64::NAN,
            ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness,
            0,
        ),
        (
            f64::INFINITY,
            ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness,
            0,
        ),
        (
            f64::NEG_INFINITY,
            ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness,
            0,
        ),
        (
            -0.1,
            ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness,
            0,
        ),
    ] {
        let support_calls = Cell::new(0);
        let diagnostic_work = Cell::new(0);
        let publication_reached = Cell::new(false);
        let result = diagnose_scheduled_cycle_path_for_thickness_v1(
            thickness,
            || {
                support_calls.set(support_calls.get() + 1);
                false
            },
            || {
                diagnostic_work.set(diagnostic_work.get() + 1);
                (None, None)
            },
            |_| {
                diagnostic_work.set(diagnostic_work.get() + 1);
                (None, None)
            },
            |diagnostic: &Diagnostic| *diagnostic,
        )
        .inspect(|_| {
            publication_reached.set(true);
        });
        assert_eq!(result, Err(expected_error));
        assert_eq!(support_calls.get(), expected_support_calls);
        assert_eq!(diagnostic_work.get(), 0);
        assert!(!publication_reached.get());
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            generation,
            "fail-closed thickness dispatch must not mutate cancellation state",
        );
    }
}

fn common_articulation_strip_fixture_v1(
    face_count: usize,
) -> (
    ori_domain::CreasePattern,
    ori_domain::Paper,
    Vec<ori_domain::EdgeId>,
) {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex, VertexId};

    assert!(face_count >= 2);
    let bottom = (0..=face_count)
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let top = (0..=face_count)
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let vertices = bottom
        .iter()
        .zip(&top)
        .enumerate()
        .flat_map(|(x, (&bottom, &top))| {
            let x = x as f64 * 10.0;
            [
                Vertex {
                    id: bottom,
                    position: Point2::new(x, 0.0),
                },
                Vertex {
                    id: top,
                    position: Point2::new(x, 10.0),
                },
            ]
        })
        .collect::<Vec<_>>();
    let boundary = bottom
        .iter()
        .copied()
        .chain(top.iter().rev().copied())
        .collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: ori_domain::EdgeId::new(),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let crease_edges = (1..face_count)
        .map(|x| Edge {
            id: ori_domain::EdgeId::new(),
            start: bottom[x],
            end: top[x],
            kind: EdgeKind::Mountain,
        })
        .collect::<Vec<_>>();
    let hinges = crease_edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    edges.extend(crease_edges);
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary,
            thickness_mm: 0.1,
            ..Paper::default()
        },
        hinges,
    )
}

#[test]
fn three_block_strip_flat_layer_anchor_mismatch_fails_closed() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let namespace = ProjectId::new();
    let face_count = 4usize;
    let (pattern, paper, hinges) = common_articulation_strip_fixture_v1(face_count);
    let document = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .unwrap();
    assert_eq!(document.faces.len(), 4);
    assert_eq!(document.hinge_adjacency.len(), 3);
    let geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &document,
        ori_kinematics::TreeKinematicsLimits::default(),
    )
    .expect("three-block strip geometry");
    let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
        &document,
        ori_kinematics::TreeKinematicsLimits::default(),
    )
    .expect("three-block strip audit");
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            &audit,
            ori_kinematics::CanonicalEdgeBlockLimitsV1 {
                max_blocks: 3,
                ..ori_kinematics::CanonicalEdgeBlockLimitsV1::default()
            },
        )
        .expect("three canonical strip blocks");
    assert_eq!(decomposition.blocks().len(), 3);
    assert!(
        decomposition
            .blocks()
            .iter()
            .all(|block| block.geometry().hinges().len() == 1),
        "every canonical block must retain its one live hinge"
    );
    let fixed_face = document.faces[0].id;
    let mut project = super::super::ProjectState::new_with_paper(pattern.clone(), paper.clone());
    project.project_id = namespace;
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        fixed_face,
    );
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let active = hinges.clone();
    assert_eq!(active.len(), 3);
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let schedule = dense_grid_schedule(&hinges, &active, 64);
    let request = || CurrentCyclePosePreviewRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        cycle_schedule_v1: schedule.clone(),
    };
    let app_state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();

    let resource_guard =
        super::stacked_fold_blockwise_cycle::
            override_bounded_multi_block_layer_peak_limit_for_test_v1(0);
    let resource_error = propose_current_cycle_pose_inner_with_layers(
        None,
        &app_state,
        Some(&layer_state),
        &transactions,
        request(),
    )
    .expect_err("the exact retained-layer peak cap must reject publication");
    assert_eq!(resource_error, CYCLE_PATH_RESOURCE_MESSAGE);
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    {
        let project =
            super::super::lock_project(&app_state).expect("project after resource rejection");
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }
    drop(resource_guard);

    let anchor_mismatch = propose_current_cycle_pose_inner_with_layers(
        None,
        &app_state,
        Some(&layer_state),
        &transactions,
        request(),
    )
    .expect_err("a 180-degree flat-layer certificate cannot anchor a zero-degree schedule");
    assert_eq!(anchor_mismatch, CYCLE_PATH_UNCERTIFIED_MESSAGE);
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    {
        let project =
            super::super::lock_project(&app_state).expect("project after anchor mismatch");
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    // Replacing the live capability with identical content cannot turn
    // an anchor mismatch into path authority or leave a partial token.
    {
        let project =
            super::super::lock_project(&app_state).expect("project before layer replacement");
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
    }
    let retry_error = propose_current_cycle_pose_inner_with_layers(
        None,
        &app_state,
        Some(&layer_state),
        &transactions,
        request(),
    )
    .expect_err("same-content layer replacement cannot supply anchor replay");
    assert_eq!(retry_error, CYCLE_PATH_UNCERTIFIED_MESSAGE);
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    let project =
        super::super::lock_project(&app_state).expect("project after anchor-mismatch retry");
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}
