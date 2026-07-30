#[test]
fn seven_hinge_generic_grid_proof_applies_and_persists_atomically() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut project = seven_hinge_tree_project();
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 8);
    let hinges = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    assert_eq!(hinges.len(), 7);
    super::super::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
        &mut project,
        hinges.clone(),
        snapshot.faces[0].id,
        1.0,
    );
    assert!(project.editor.paper().thickness_mm > 0.0);
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let target_angles = hinges
        .iter()
        .copied()
        .map(|edge| DyadicPoseGraphAngleDtoV1 {
            edge,
            angle_degrees: 2.0,
        })
        .collect::<Vec<_>>();
    let state = AppState::new(project);
    let request = |schedule| DyadicPoseGraphReadRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        target_angles: target_angles.clone(),
        max_states: 2_187,
        max_transitions: 20_412,
        level_count: 3,
        cycle_schedule_v1: schedule,
    };
    let generic =
        read_bounded_dyadic_pose_graph_inner_v1(&state, Some(&layer_state), request(None), None)
            .unwrap()
            .into_test_view();
    assert_eq!(generic.status, "certified");
    assert_eq!(
        (generic.state_count, generic.transition_count),
        (2_187, 20_412)
    );
    assert!(generic.mutation_candidate_ready);
    assert!(generic.positive_thickness_certified);
    assert!(generic.layer_transport_certified);
    let preview_state = DyadicPathPreviewState::default();
    let preview = mint_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        DyadicPathPreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles,
            max_states: 2_187,
            max_transitions: 20_412,
            level_count: 3,
            cycle_schedule_v1: None,
            expected_path_binding_sha256: generic.certificate_binding_sha256.unwrap(),
            expected_positive_thickness_binding_sha256: generic
                .positive_thickness_binding_sha256
                .unwrap(),
            expected_layer_transport_binding_sha256: generic
                .layer_transport_binding_sha256
                .unwrap(),
        },
    )
    .expect("seven-hinge generic proof mints a bounded read-only token");
    assert!(!preview.authorizes_project_mutation);
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
    drop(project);
    let apply_request = |path_binding: String| ApplyDyadicPathPreviewRequestV1 {
        preview_token: preview.preview_token,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        expected_target_binding_sha256: preview.target_binding_sha256.clone(),
        expected_path_binding_sha256: path_binding,
        expected_positive_thickness_binding_sha256: preview
            .positive_thickness_binding_sha256
            .clone(),
        expected_layer_transport_binding_sha256: preview.layer_transport_binding_sha256.clone(),
    };
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request("00".repeat(32)),
        )
        .is_err()
    );
    assert_eq!(
        super::super::lock_project(&state)
            .unwrap()
            .editor
            .revision(),
        revision,
        "tampered seven-hinge proof is an atomic no-op"
    );
    let applied = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        apply_request(preview.path_binding_sha256.clone()),
    )
    .expect("issuer-bound seven-hinge Tree proof applies atomically");
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .is_err(),
        "consumed seven-hinge preview must be one-shot"
    );
    let mut project = super::super::lock_project(&state).unwrap();
    assert_eq!(applied, revision + 1);
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project.editor.instruction_timeline().steps[1..]
            .iter()
            .all(|step| step.visual.path_certificate_reference_v1.is_some())
    );
    project.editor.undo(applied).unwrap();
    assert!(project.editor.instruction_timeline().steps.is_empty());
    let undone = project.editor.revision();
    project.editor.redo(undone).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    let archive = project.project_archive().unwrap();
    let reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("seven-hinge-tree-positive-thickness.ori2"),
    )
    .unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    assert!(
        reopened.editor.instruction_timeline().steps[1..]
            .iter()
            .all(|step| step.visual.path_certificate_reference_v1.is_some())
    );
}

#[test]
fn eight_hinge_collective_proof_applies_and_persists_atomically() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut project = eight_hinge_tree_project();
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 9);
    let hinges = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    assert_eq!(hinges.len(), 8);
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        snapshot.faces[0].id,
    );
    assert!(project.editor.paper().thickness_mm > 0.0);
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let angle = 2.0 * 1.0_f64.atan2(64.0).to_degrees();
    let target_angles = hinges
        .iter()
        .copied()
        .map(|edge| DyadicPoseGraphAngleDtoV1 {
            edge,
            angle_degrees: angle,
        })
        .collect::<Vec<_>>();
    let state = AppState::new(project);
    let request = |schedule| DyadicPoseGraphReadRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        target_angles: target_angles.clone(),
        max_states: 2_187,
        max_transitions: 20_412,
        level_count: 3,
        cycle_schedule_v1: schedule,
    };
    let generic =
        read_bounded_dyadic_pose_graph_inner_v1(&state, Some(&layer_state), request(None), None)
            .unwrap()
            .into_test_view();
    assert_eq!(generic.status, "resource_limit");
    assert_eq!((generic.state_count, generic.transition_count), (0, 0));
    assert!(!generic.mutation_candidate_ready);

    let schedule = dense_grid_schedule_ratio(&hinges, &hinges, 1, 64);
    let generation_before_limit_rejections = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    for (max_states, max_transitions) in [
        (0, 4),
        (MAX_DYADIC_GRAPH_STATES_V1 + 1, 4),
        (3, 0),
        (3, MAX_DYADIC_GRAPH_TRANSITIONS_V1 + 1),
        (2, 4),
        (3, 3),
    ] {
        let mut rejected = request(Some(schedule.clone()));
        rejected.max_states = max_states;
        rejected.max_transitions = max_transitions;
        let error =
            read_bounded_dyadic_pose_graph_inner_v1(&state, Some(&layer_state), rejected, None)
                .expect_err("under-provisioned collective request must fail");
        assert_eq!(error, CYCLE_PATH_RESOURCE_MESSAGE);
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            generation_before_limit_rejections,
            "dyadic graph resource admission must precede generation replacement",
        );
    }
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        &state,
        Some(&layer_state),
        request(Some(schedule.clone())),
        None,
    )
    .unwrap()
    .into_test_view();
    assert_eq!((observed.state_count, observed.transition_count), (3, 4));
    assert_eq!(observed.status, "certified");
    assert!(observed.mutation_candidate_ready);
    assert!(observed.positive_thickness_certified);
    assert!(observed.layer_transport_certified);
    let preview_state = DyadicPathPreviewState::default();
    let preview = mint_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        DyadicPathPreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles,
            max_states: 2_187,
            max_transitions: 20_412,
            level_count: 3,
            cycle_schedule_v1: Some(schedule),
            expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
            expected_positive_thickness_binding_sha256: observed
                .positive_thickness_binding_sha256
                .unwrap(),
            expected_layer_transport_binding_sha256: observed
                .layer_transport_binding_sha256
                .unwrap(),
        },
    )
    .expect("eight-hinge collective proof mints a bounded read-only token");
    assert!(!preview.authorizes_project_mutation);
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
    drop(project);
    let apply_request = |path_binding: String| ApplyDyadicPathPreviewRequestV1 {
        preview_token: preview.preview_token,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        expected_target_binding_sha256: preview.target_binding_sha256.clone(),
        expected_path_binding_sha256: path_binding,
        expected_positive_thickness_binding_sha256: preview
            .positive_thickness_binding_sha256
            .clone(),
        expected_layer_transport_binding_sha256: preview.layer_transport_binding_sha256.clone(),
    };
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request("00".repeat(32)),
        )
        .is_err()
    );
    assert_eq!(
        super::super::lock_project(&state)
            .unwrap()
            .editor
            .revision(),
        revision,
        "tampered eight-hinge proof is an atomic no-op"
    );
    let applied = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        apply_request(preview.path_binding_sha256.clone()),
    )
    .expect("issuer-bound eight-hinge collective proof applies atomically");
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .is_err(),
        "consumed eight-hinge preview must be one-shot"
    );
    let mut project = super::super::lock_project(&state).unwrap();
    assert_eq!(applied, revision + 1);
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project.editor.instruction_timeline().steps[1..]
            .iter()
            .all(|step| step.visual.path_certificate_reference_v1.is_some())
    );
    project.editor.undo(applied).unwrap();
    assert!(project.editor.instruction_timeline().steps.is_empty());
    let undone = project.editor.revision();
    project.editor.redo(undone).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    let archive = project.project_archive().unwrap();
    let reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("eight-hinge-collective-positive-thickness.ori2"),
    )
    .unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    assert!(
        reopened.editor.instruction_timeline().steps[1..]
            .iter()
            .all(|step| step.visual.path_certificate_reference_v1.is_some())
    );
}

#[test]
fn two_hinge_e2e_fixture_issues_pose_and_layer_authorities() {
    let mut project = two_hinge_tree_project(0.0);
    super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    assert!(
        project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .is_some()
    );
    assert!(
        capture_current_layer_order_capability(&layer_state, &project)
            .unwrap()
            .is_some()
    );
}

fn assert_non_graph_capability_returns_unsupported_dto(
    project: super::super::ProjectState,
    target_edge: ori_domain::EdgeId,
    authority_expected: bool,
) {
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let state = AppState::new(project);
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        &state,
        None,
        DyadicPoseGraphReadRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                edge: target_edge,
                angle_degrees: 1.0,
            }],
            max_states: 32,
            max_transitions: 64,
            level_count: 3,
            cycle_schedule_v1: None,
        },
        None,
    )
    .expect("non-graph capability returns a read-only DTO")
    .into_test_view();
    assert_eq!(observed.status, "unsupported");
    assert_eq!(observed.reason, "unsupported_geometry");
    assert_eq!(observed.state_count, 0);
    assert_eq!(observed.transition_count, 0);
    assert_eq!(observed.explored_state_count, 0);
    assert_eq!(observed.evaluated_transition_count, 0);
    assert_eq!(observed.certified_transition_count, 0);
    assert!(!observed.mutation_candidate_ready);
    assert!(!observed.authorizes_project_mutation);
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert_eq!(
        project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .is_some(),
        authority_expected
    );
}

#[test]
fn missing_pose_capability_strict_dyadic_read_returns_unsupported_dto() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let project = two_hinge_tree_project(0.0);
    let target_edge = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze()
        .simulation_snapshot()
        .unwrap()
        .hinge_adjacency[0]
        .edge;
    assert_non_graph_capability_returns_unsupported_dto(project, target_edge, false);
}

#[test]
fn tree_pose_capability_rejects_incomplete_target_without_mutation() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut project = two_hinge_tree_project(0.0);
    let target_edge = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze()
        .simulation_snapshot()
        .unwrap()
        .hinge_adjacency[0]
        .edge;
    super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let state = AppState::new(project);
    let result = read_bounded_dyadic_pose_graph_inner_v1(
        &state,
        None,
        DyadicPoseGraphReadRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                edge: target_edge,
                angle_degrees: 1.0,
            }],
            max_states: 32,
            max_transitions: 64,
            level_count: 3,
            cycle_schedule_v1: None,
        },
        None,
    );
    assert_eq!(result.unwrap_err(), CYCLE_PATH_UNSUPPORTED_MESSAGE);
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}
