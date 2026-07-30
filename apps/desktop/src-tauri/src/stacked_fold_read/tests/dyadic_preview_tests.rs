use super::*;

#[test]
fn dyadic_pose_graph_read_is_strict_bounded_and_observation_only() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (mut project, hinges) =
        super::super::super::applied_pose::tests::four_vertex_cycle_project();
    super::super::super::applied_pose::tests::install_flat_graph_pose_authority(
        &mut project,
        hinges.clone(),
    );
    let live_edges = project
        .applied_pose_authority
        .capture_capability(&project)
        .unwrap()
        .unwrap()
        .graph()
        .unwrap()
        .2
        .hinge_angles()
        .as_slice()
        .iter()
        .map(|angle| angle.edge())
        .collect::<Vec<_>>();
    let request = |max_states| DyadicPoseGraphReadRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        target_angles: live_edges
            .iter()
            .copied()
            .enumerate()
            .map(|(index, edge)| DyadicPoseGraphAngleDtoV1 {
                edge,
                angle_degrees: if index < 2 { 30.0 } else { 0.0 },
            })
            .collect(),
        max_states,
        max_transitions: 64,
        level_count: 3,
        cycle_schedule_v1: None,
    };
    let limited_request = request(8);
    let live_request = request(32);
    let state = AppState::new(project);
    let limited = read_bounded_dyadic_pose_graph_inner_v1(&state, None, limited_request, None)
        .unwrap()
        .into_test_view();
    assert_eq!(limited.status, "resource_limit");
    assert!(!limited.authorizes_project_mutation);
    let observed = read_bounded_dyadic_pose_graph_inner_v1(&state, None, live_request, None)
        .unwrap()
        .into_test_view();
    assert_eq!(observed.state_count, 9);
    assert_eq!(observed.transition_count, 24);
    assert_eq!(observed.status, "no_path");
    assert_eq!(observed.reason, "no_certified_path");
    assert_eq!(observed.certified_transition_count, 0);
    assert!(observed.certificate_binding_sha256.is_none());
    assert_eq!(observed.positive_thickness_transition_count, 0);
    assert!(!observed.positive_thickness_certified);
    assert!(observed.positive_thickness_binding_sha256.is_none());
    assert_eq!(observed.layer_transport_transition_count, 0);
    assert!(!observed.layer_transport_certified);
    assert!(observed.layer_transport_binding_sha256.is_none());
    assert!(!observed.mutation_candidate_ready);
    assert!(!observed.authorizes_project_mutation);
    let preview_state = DyadicPathPreviewState::default();
    let rejected = mint_dyadic_pose_path_preview_inner_v1(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &preview_state,
        DyadicPathPreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: observed.project_instance_id,
            expected_project_id: observed.project_id,
            expected_revision: observed.revision,
            target_angles: live_edges
                .iter()
                .copied()
                .enumerate()
                .map(|(index, edge)| DyadicPoseGraphAngleDtoV1 {
                    edge,
                    angle_degrees: if index < 2 { 30.0 } else { 0.0 },
                })
                .collect(),
            max_states: 32,
            max_transitions: 64,
            level_count: 3,
            cycle_schedule_v1: None,
            expected_path_binding_sha256: "00".repeat(32),
            expected_positive_thickness_binding_sha256: "11".repeat(32),
            expected_layer_transport_binding_sha256: "22".repeat(32),
        },
    );
    assert_eq!(rejected.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
    assert!(preview_state.is_empty_for_test());
    let token = ProjectId::new();
    let target_binding = [0x33; 32];
    preview_state.install_record_for_test(
        token,
        observed.project_instance_id,
        observed.project_id,
        observed.revision,
        target_binding,
        "44".repeat(32),
        "55".repeat(32),
        "66".repeat(32),
        None,
    );
    let apply_request = |path: String| ApplyDyadicPathPreviewRequestV1 {
        preview_token: token,
        expected_project_instance_id: observed.project_instance_id,
        expected_project_id: observed.project_id,
        expected_revision: observed.revision,
        expected_target_binding_sha256: "33".repeat(32),
        expected_path_binding_sha256: path,
        expected_positive_thickness_binding_sha256: "55".repeat(32),
        expected_layer_transport_binding_sha256: "66".repeat(32),
    };
    let apply_layer_state = GlobalFlatFoldabilityState::default();
    assert_eq!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &apply_layer_state,
            &preview_state,
            apply_request("77".repeat(32)),
        )
        .unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
    );
    assert!(!preview_state.is_empty_for_test());
    assert_eq!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &apply_layer_state,
            &preview_state,
            apply_request("44".repeat(32)),
        )
        .unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
    );
    assert!(!preview_state.is_empty_for_test());
    cancel_dyadic_pose_path_preview_inner_v1(&preview_state, token).unwrap();
    assert!(preview_state.is_empty_for_test());
    assert_eq!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &apply_layer_state,
            &preview_state,
            apply_request("44".repeat(32)),
        )
        .unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
    );
    assert!(
        super::super::super::lock_project(&state)
            .unwrap()
            .editor
            .instruction_timeline()
            .steps
            .is_empty()
    );
}

#[test]
fn moving_dense_dyadic_path_does_not_mint_private_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, mut paper, horizontal, _) =
        super::super::dense_grid_cycle_test_support::miura_authority_pattern(3, 3);
    paper.thickness_mm = 0.1;
    let moving = horizontal.into_iter().take(3).collect::<Vec<_>>();
    let mut project = super::super::super::ProjectState::new_with_paper(pattern, paper);
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
    super::super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        snapshot.faces[0].id,
    );
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let state = AppState::new(project);
    let preview_state = DyadicPathPreviewState::default();
    let schedule_for = |mask: usize| {
        let mut schedule = dense_grid_schedule(&hinges, &moving, 100);
        for (index, entry) in schedule
            .entries
            .iter_mut()
            .filter(|entry| moving.contains(&entry.edge))
            .enumerate()
        {
            let mountain = snapshot
                .hinge_adjacency
                .iter()
                .find(|hinge| hinge.edge == entry.edge)
                .is_some_and(|hinge| hinge.assignment == ori_topology::FoldAssignment::Mountain);
            if mountain ^ (mask & (1 << index) != 0) {
                entry.numerator_power_coefficients[1].numerator *= -1;
                entry.requested_angle_degrees *= -1.0;
            }
        }
        schedule
    };
    let candidate = (0..8).find_map(|mask| {
        let schedule = schedule_for(mask);
        let target = {
            let project = super::super::super::lock_project(&state).unwrap();
            let capability = project
                .applied_pose_authority
                .capture_capability(&project)
                .ok()??;
            let (geometry, audit, pose) = capability.graph()?;
            prepare_requested_cycle_schedule_v1(
                &schedule,
                geometry,
                audit,
                pose.fixed_face(),
                pose.hinge_angles(),
            )
            .ok()?
            .evaluate(1.0)?
        };
        let request = DyadicPoseGraphReadRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target
                .as_slice()
                .iter()
                .map(|angle| DyadicPoseGraphAngleDtoV1 {
                    edge: angle.edge(),
                    angle_degrees: angle.angle_degrees(),
                })
                .collect(),
            max_states: 32,
            max_transitions: 128,
            level_count: 3,
            cycle_schedule_v1: Some(schedule.clone()),
        };
        let value =
            read_bounded_dyadic_pose_graph_inner_v1(&state, Some(&layer_state), request, None)
                .ok()?
                .into_test_view();
        value
            .mutation_candidate_ready
            .then_some((schedule, target, value))
    });
    assert!(
        candidate.is_none(),
        "finite moving Miura path samples must not mint positive-thickness and layer authority",
    );
    assert!(
        super::super::super::lock_project(&state)
            .unwrap()
            .editor
            .instruction_timeline()
            .steps
            .is_empty(),
    );
    if let Some((schedule, target, observed)) = candidate {
        let expected_steps = observed.certified_transition_count + 1;
        let request = DyadicPathPreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target
                .as_slice()
                .iter()
                .map(|angle| DyadicPoseGraphAngleDtoV1 {
                    edge: angle.edge(),
                    angle_degrees: angle.angle_degrees(),
                })
                .collect(),
            max_states: 32,
            max_transitions: 128,
            level_count: 3,
            cycle_schedule_v1: Some(schedule),
            expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
            expected_positive_thickness_binding_sha256: observed
                .positive_thickness_binding_sha256
                .unwrap(),
            expected_layer_transport_binding_sha256: observed
                .layer_transport_binding_sha256
                .unwrap(),
        };
        let preview =
            mint_dyadic_pose_path_preview_inner_v1(&state, &layer_state, &preview_state, request)
                .unwrap();
        let apply_request = |path: String| ApplyDyadicPathPreviewRequestV1 {
            preview_token: preview.preview_token,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            expected_target_binding_sha256: preview.target_binding_sha256.clone(),
            expected_path_binding_sha256: path,
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
                apply_request("00".repeat(32))
            )
            .is_err()
        );
        assert_eq!(
            super::super::super::lock_project(&state)
                .unwrap()
                .editor
                .revision(),
            revision
        );
        let (pose_before_rollback, layer_before_rollback) = {
            let project = super::super::super::lock_project(&state).unwrap();
            (
            project
                .applied_pose_authority
                .capture_capability(&project)
                .expect("source pose authority")
                .expect("source pose capability"),
            super::super::super::global_flat_foldability::capture_current_layer_order_capability(
                &layer_state,
                &project,
            )
            .expect("source layer authority")
            .expect("source layer capability"),
        )
        };
        let _dyadic_apply_failure_guard =
        super::super::stacked_fold_dyadic_preview::fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
        assert!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request(preview.path_binding_sha256.clone())
            )
            .is_err(),
            "injected post-pose failure must restore the complete dyadic Apply image"
        );
        assert!(!preview_state.is_empty_for_test());
        {
            let project = super::super::super::lock_project(&state).unwrap();
            assert_eq!(project.editor.revision(), revision);
            assert!(
                project
                    .applied_pose_authority
                    .revalidate_capability(&project, &pose_before_rollback)
                    .expect("pose rollback revalidation")
                    .is_some()
            );
            assert!(
            super::super::super::global_flat_foldability::revalidate_current_layer_order_capability(
                &layer_state,
                &project,
                &layer_before_rollback,
            )
            .expect("layer rollback revalidation")
            .is_some()
        );
        }
        let applied = apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .unwrap();
        assert!(
            preview_state.is_empty_for_test(),
            "a successful dyadic Apply consumes its one-shot preview slot"
        );
        assert!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request(preview.path_binding_sha256.clone())
            )
            .is_err()
        );
        let mut project = super::super::super::lock_project(&state).unwrap();
        assert_eq!(applied, revision + 1);
        assert_eq!(
            project.editor.instruction_timeline().steps.len(),
            expected_steps
        );
        assert!(
            project.editor.instruction_timeline().steps[1..]
                .iter()
                .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
        );
        project.editor.undo(applied).unwrap();
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        let archive = project.project_archive().unwrap();
        let reopened = super::super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("dyadic-authority.ori2"),
        )
        .unwrap();
        assert_eq!(
            reopened.editor.instruction_timeline().steps.len(),
            expected_steps
        );
        assert!(
            reopened.editor.instruction_timeline().steps[1..]
                .iter()
                .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
        );
    }
}
