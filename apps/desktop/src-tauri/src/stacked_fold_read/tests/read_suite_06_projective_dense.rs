#[test]
fn genuine_two_hinge_projective_schedule_previews_applies_and_round_trips_history() {
    let _ = assert_two_hinge_projective_schedule_round_trip(
        [50.0, 0.0, 0.0],
        [50.0, 0.0, -100.0],
        0.0,
        0,
        None,
        Some(ori_core::APPLIED_POSE_MODEL_ID_V1),
    );
}

#[test]
fn genuine_common_axis_cycle_previews_applies_and_round_trips_history() {
    let _ = assert_two_hinge_projective_schedule_round_trip(
        [0.0, 0.0, -50.0],
        [100.0, 0.0, -50.0],
        0.0,
        0,
        None,
        Some(ori_core::CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1),
    );
}

#[test]
fn genuine_common_axis_cycle_certified_path_applies_and_round_trips_history() {
    let _ = assert_two_hinge_projective_schedule_round_trip(
        [0.0, 0.0, -50.0],
        [100.0, 0.0, -50.0],
        0.0,
        2,
        None,
        None,
    );
}

#[test]
fn positive_thickness_certified_path_graph_is_rejected_before_generation_or_authority() {
    assert!(
        assert_two_hinge_projective_schedule_round_trip(
            [0.0, 0.0, -50.0],
            [100.0, 0.0, -50.0],
            0.1,
            2,
            None,
            None,
        )
        .is_empty()
    );
}

#[test]
fn negative_zero_certified_path_graph_keeps_legacy_atomic_apply() {
    assert_eq!(
        assert_two_hinge_projective_schedule_round_trip(
            [0.0, 0.0, -50.0],
            [100.0, 0.0, -50.0],
            -0.0,
            2,
            None,
            None,
        )
        .len(),
        2,
    );
}

#[test]
fn genuine_common_axis_cycle_four_edge_certified_path_applies_and_round_trips_history() {
    let _ = assert_two_hinge_projective_schedule_round_trip(
        [0.0, 0.0, -50.0],
        [100.0, 0.0, -50.0],
        0.0,
        4,
        None,
        None,
    );
}

#[test]
fn genuine_common_axis_cycle_sixteen_edge_certified_path_applies_and_round_trips_history() {
    let _ = assert_two_hinge_projective_schedule_round_trip(
        [0.0, 0.0, -50.0],
        [100.0, 0.0, -50.0],
        0.0,
        16,
        None,
        None,
    );
}

#[test]
fn genuine_common_axis_cycle_maximum_atomic_path_cancels_cleanly_and_retries() {
    let first = [0.0, 0.0, -50.0];
    let second = [100.0, 0.0, -50.0];
    assert!(
        assert_two_hinge_projective_schedule_round_trip(first, second, 0.0, 31, Some(8), None,)
            .is_empty()
    );
    let first_retry =
        assert_two_hinge_projective_schedule_round_trip(first, second, 0.0, 31, None, None);
    let second_retry =
        assert_two_hinge_projective_schedule_round_trip(first, second, 0.0, 31, None, None);
    assert_eq!(first_retry, second_retry);
}

#[test]
fn cell_keys_use_fixed_lowercase_sha256_hex() {
    let mut bytes = [0_u8; 32];
    bytes[0] = 0xab;
    bytes[31] = 0xef;
    let encoded = lowercase_hex(bytes);
    assert_eq!(encoded.len(), 64);
    assert!(encoded.starts_with("ab00"));
    assert!(encoded.ends_with("00ef"));
    assert!(encoded.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(!encoded.bytes().any(|byte| byte.is_ascii_uppercase()));
}

#[test]
fn rank64_cycle_request_rejects_resource_before_work_and_keeps_progress_cancel_dtos_bounded() {
    let entry = || {
        serde_json::json!({
            "edge": ori_domain::EdgeId::new(),
            "uDomain": [{"numerator": 0, "denominator": 1}, {"numerator": 1, "denominator": 1}],
            "numeratorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
            "denominatorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
            "requestedAngleDegrees": 90.0
        })
    };
    let request = serde_json::from_value::<StackedFoldReadRequest>(serde_json::json!({
        "progressRequestId": "rank64:resource",
        "expectedProjectInstanceId": ori_domain::ProjectId::new(),
        "expectedProjectId": ori_domain::ProjectId::new(),
        "expectedRevision": 0,
        "first": [0.0, 0.0, 0.0],
        "second": [1.0, 0.0, 0.0],
        "fixedSide": "left",
        "rotationDirection": "positive",
        "requestedAngleDegrees": 90.0,
        "cycleScheduleV1": {"version": 1, "entries": (0..256).map(|_| entry()).collect::<Vec<_>>()}
    }))
    .unwrap();
    assert_eq!(
        validate_progress_request_id_v1(request.progress_request_id.as_deref()),
        Ok(Some("rank64:resource"))
    );
    assert_eq!(
        validate_request_resource_shape_v1(&request),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );

    let _generation_guard = lock_stacked_fold_read_generation_test();
    let before = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    cancel_current_stacked_fold_read_inner_v1().expect("rank64 cancel dto remains available");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        before + 1
    );
}

#[test]
fn stacked_fold_read_cancel_advances_the_process_wide_generation() {
    let _generation_guard = STACKED_FOLD_READ_GENERATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let before = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    cancel_current_stacked_fold_read_inner_v1().expect("generation has capacity");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        before + 1
    );
}

#[test]
fn live_registry_round_trips_into_the_same_bit_exact_linear_request() {
    let first = serde_json::from_value::<ori_domain::EdgeId>(serde_json::json!(
        "018f47a2-4b7a-7cc1-8abc-665544332211"
    ))
    .unwrap();
    let second = serde_json::from_value::<ori_domain::EdgeId>(serde_json::json!(
        "018f47a2-4b7a-7cc1-8abc-778899aabbcc"
    ))
    .unwrap();
    let live = ori_kinematics::CanonicalHingeAngles::new(vec![
        ori_kinematics::HingeAngle::new(first, 10.0).unwrap(),
        ori_kinematics::HingeAngle::new(second, 20.0).unwrap(),
    ])
    .unwrap();
    let registry = live_hinge_registry(live.as_slice());
    assert_eq!(
        registry
            .iter()
            .map(LiveGraphHingeAngleDto::edge_for_test)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    let request = LinearCandidateRequestV1 {
        version: 1,
        exact_dyadic_path_v1: None,
        entries: registry
            .iter()
            .map(|entry| LinearCandidateEntryRequestV1 {
                edge: entry.edge_for_test(),
                initial_angle_degrees: entry.initial_angle_degrees_for_test(),
                requested_angle_degrees: entry.initial_angle_degrees_for_test() + 5.0,
            })
            .collect(),
    };
    let (round_tripped, requested) = validate_linear_candidate_angles_v1(&request, &live).unwrap();
    assert_eq!(round_tripped, live);
    assert!(
        requested
            .as_slice()
            .iter()
            .zip(live.as_slice())
            .all(|(next, initial)| {
                next.edge() == initial.edge()
                    && next.angle_degrees() == initial.angle_degrees() + 5.0
            })
    );
}

#[test]
fn moving_dense_rank_four_grid_previews_fail_closed_without_layer_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
        let (pattern, mut paper, moving) =
            super::dense_grid_cycle_test_support::three_by_three_dense_cycle_pattern();
        paper.thickness_mm = thickness_mm;
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(
            (snapshot.faces.len(), snapshot.hinge_adjacency.len()),
            (9, 12)
        );
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        let fixed = snapshot.faces[0].id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let request = |expected_instance_id| CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: expected_instance_id,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: dense_grid_schedule(&hinges, &moving, 4),
        };
        assert_eq!(
            propose_current_cycle_pose_inner(
                None,
                &app_state,
                &transactions,
                request(ProjectId::new())
            )
            .unwrap_err(),
            STALE_MESSAGE
        );
        let preview =
            propose_current_cycle_pose_inner(None, &app_state, &transactions, request(instance));
        assert_eq!(
            preview.unwrap_err(),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
            "moving rank-four dense grid at thickness {thickness_mm} must fail closed \
             without continuous all-pair and layer authority",
        );
        let project = super::super::lock_project(&app_state).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert_eq!(project.editor.revision(), revision);
    }
}

#[test]
fn regular_quad_petal_gate_accepts_only_three_hinges_on_one_square_boundary_face() {
    let (pattern, paper, _, _) =
        super::dense_grid_cycle_test_support::miura_authority_pattern(3, 3);
    let project = super::super::ProjectState::new_with_paper(pattern, paper);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    let hinges = snapshot
        .faces
        .iter()
        .find_map(|face| {
            let hinges = face
                .outer
                .half_edges
                .iter()
                .filter(|half| {
                    snapshot
                        .hinge_adjacency
                        .iter()
                        .any(|adjacency| adjacency.edge == half.edge)
                })
                .map(|half| half.edge)
                .collect::<Vec<_>>();
            (face.outer.half_edges.len() == 4 && hinges.len() == 3).then_some(hinges)
        })
        .expect("3x3 edge cell has exactly three hinge sides");
    assert!(super::super::stacked_fold_transaction::regular_quad_petal_face_v1(&project, &hinges));
    assert!(
        !super::super::stacked_fold_transaction::regular_quad_petal_face_v1(&project, &hinges[..2],)
    );
    let mut duplicate = hinges.clone();
    duplicate[2] = duplicate[0];
    assert!(
        !super::super::stacked_fold_transaction::regular_quad_petal_face_v1(&project, &duplicate,)
    );
}

#[test]
fn regular_quad_petal_private_capture_rejects_without_publishing_or_mutating() {
    let (pattern, paper, _, _) =
        super::dense_grid_cycle_test_support::miura_authority_pattern(3, 3);
    let project = super::super::ProjectState::new_with_paper(pattern, paper);
    let revision = project.editor.revision();
    let previews = RegularQuadPetalPrivatePreviewStateV1::default();
    assert!(
        capture_and_mint_regular_quad_petal_preview_v1(
            &project,
            &GlobalFlatFoldabilityState::default(),
            &previews,
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert!(previews.0.lock().unwrap().is_none());
}

#[test]
fn moving_dense_square_and_rectangular_grids_fail_closed_without_layer_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for (columns, rows) in [
        (4usize, 4usize),
        (5, 5),
        (6, 6),
        (7, 7),
        (3, 7),
        (5, 7),
        (6, 7),
    ] {
        for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
            let (pattern, mut paper, moving) =
                super::dense_grid_cycle_test_support::rectangular_dense_cycle_pattern(
                    columns, rows,
                );
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let expected_hinges = 2 * columns * rows - columns - rows;
            assert_eq!(
                (snapshot.faces.len(), snapshot.hinge_adjacency.len()),
                (columns * rows, expected_hinges)
            );
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect::<Vec<_>>();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let request = CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
                cycle_schedule_v1: dense_grid_schedule(
                    &hinges,
                    &moving,
                    if columns == 4 && rows == 4 { 4 } else { 100 },
                ),
            };
            let state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            let preview = propose_current_cycle_pose_inner(None, &state, &transactions, request);
            assert_eq!(
                preview.unwrap_err(),
                CYCLE_PATH_UNCERTIFIED_MESSAGE,
                "{columns}x{rows} moving dense grid at thickness {thickness_mm} must fail \
                 closed without continuous all-pair and layer authority",
            );
            let project = super::super::lock_project(&state).unwrap();
            assert!(project.editor.instruction_timeline().steps.is_empty());
        }
    }
}

#[test]
fn eighty_four_hinge_moving_path_without_layer_transport_fails_closed() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, mut paper, moving) =
        super::dense_grid_cycle_test_support::rectangular_dense_cycle_pattern(7, 7);
    paper.thickness_mm = 0.1;
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    let hinges = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    assert_eq!((snapshot.faces.len(), hinges.len()), (49, 84));
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        snapshot.faces[0].id,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let preview = propose_current_cycle_pose_inner(
        None,
        &state,
        &transactions,
        CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: dense_grid_schedule(&hinges, &moving, 100),
        },
    );
    assert_eq!(
        preview.unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
        "84-hinge moving path must not mint authority from flat-start samples alone",
    );
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn orthogonal_dense_rank_four_horizontal_axis_fails_closed_without_layer_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
        let (pattern, mut paper, horizontal, _) =
            super::dense_grid_cycle_test_support::orthogonal_dense_cycle_pattern(3, 3);
        paper.thickness_mm = thickness_mm;
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        let fixed = snapshot.faces[0].id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let request = |expected_project_instance_id| CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: dense_grid_schedule(&hinges, &horizontal, 4),
        };
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        assert_eq!(
            propose_current_cycle_pose_inner(
                None,
                &state,
                &transactions,
                request(ProjectId::new())
            )
            .unwrap_err(),
            STALE_MESSAGE
        );
        let preview =
            propose_current_cycle_pose_inner(None, &state, &transactions, request(instance));
        assert_eq!(
            preview.unwrap_err(),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
            "orthogonal moving dense grid at thickness {thickness_mm} must fail closed \
             without continuous all-pair and layer authority",
        );
        let project = super::super::lock_project(&state).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert_eq!(project.editor.revision(), revision);
    }
}

#[test]
fn oblique_dense_rank_four_collision_fails_closed_before_preview() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
        let (pattern, mut paper, horizontal, _) =
            super::dense_grid_cycle_test_support::oblique_dense_cycle_pattern(3, 3);
        paper.thickness_mm = thickness_mm;
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        let fixed = snapshot.faces[0].id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let request = |expected_project_instance_id| CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: dense_grid_schedule(&hinges, &horizontal, 100),
        };
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        assert_eq!(
            propose_current_cycle_pose_inner(
                None,
                &state,
                &transactions,
                request(ProjectId::new())
            )
            .unwrap_err(),
            STALE_MESSAGE
        );
        let preview =
            propose_current_cycle_pose_inner(None, &state, &transactions, request(instance));
        assert_eq!(
            preview.unwrap_err(),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
            "oblique moving dense grid at thickness {thickness_mm} must fail closed without \
             continuous all-pair and layer authority",
        );
        assert_eq!(
            super::super::lock_project(&state)
                .unwrap()
                .editor
                .instruction_timeline()
                .steps
                .len(),
            0,
        );
    }
}
