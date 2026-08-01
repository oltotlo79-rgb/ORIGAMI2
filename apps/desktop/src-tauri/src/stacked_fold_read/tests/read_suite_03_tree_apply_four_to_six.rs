#[test]
fn four_hinge_tree_level_three_proof_applies_and_persists_atomically() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut project = four_hinge_tree_project();
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
    assert_eq!(hinges.len(), 4);
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
    let tree_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .unwrap()
        .unwrap();
    let mut tree_target_entries = target_angles
        .iter()
        .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    tree_target_entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
    let tree_target = ori_kinematics::CanonicalHingeAngles::new(tree_target_entries).unwrap();
    let (tree_model, tree_pose) = tree_capability.tree().expect("tree pose capability");
    let tree_diagnostic = ori_collision::diagnose_collective_hinge_path_from_pose_v1(
        tree_model,
        tree_pose,
        tree_pose.hinge_angles(),
        tree_target.as_slice(),
        project.editor.paper().thickness_mm,
        ori_collision::StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(
        tree_diagnostic.continuous_clearance_certified(),
        "four-hinge native Tree endpoint must issue positive evidence: {tree_diagnostic:?}"
    );
    let state = AppState::new(project);
    let request = |level_count, max_states, max_transitions| DyadicPoseGraphReadRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        target_angles: target_angles.clone(),
        max_states,
        max_transitions,
        level_count,
        cycle_schedule_v1: None,
    };
    for (levels, states, transitions) in [(5, 125, 600), (9, 128, 512)] {
        let limited = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(levels, states, transitions),
            None,
        )
        .unwrap()
        .into_test_view();
        assert_eq!(limited.status, "resource_limit");
        assert!(!limited.mutation_candidate_ready);
    }
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        &state,
        Some(&layer_state),
        request(3, 81, 432),
        None,
    )
    .unwrap()
    .into_test_view();
    assert_eq!((observed.state_count, observed.transition_count), (81, 432));
    assert_eq!(
        observed.status,
        "certified",
        "explored={} evaluated={} certified={} positive={}",
        observed.explored_state_count,
        observed.evaluated_transition_count,
        observed.certified_transition_count,
        observed.positive_thickness_transition_count,
    );
    assert!(observed.mutation_candidate_ready);
    assert!(!observed.authorizes_project_mutation);
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
            max_states: 81,
            max_transitions: 432,
            level_count: 3,
            cycle_schedule_v1: None,
            expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
            expected_positive_thickness_binding_sha256: observed
                .positive_thickness_binding_sha256
                .unwrap(),
            expected_layer_transport_binding_sha256: observed
                .layer_transport_binding_sha256
                .unwrap(),
        },
    )
    .expect("four-hinge certified graph mints a read-only token");
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
        "tampered Tree proof is an atomic no-op"
    );
    let registry_len_before = super::super::lock_project(&state)
        .unwrap()
        .trusted_path_certificates
        .len_v1();
    let _failure_guard =
        super::stacked_fold_dyadic_preview::fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .is_err(),
        "a post-pose failure must roll back before registry publication"
    );
    {
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert_eq!(
            project.trusted_path_certificates.len_v1(),
            registry_len_before,
            "failed dyadic Apply must retain the old registry image"
        );
    }
    let applied = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        apply_request(preview.path_binding_sha256.clone()),
    )
    .expect("issuer-bound four-hinge Tree proof applies atomically");
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .is_err(),
        "consumed Tree preview must be one-shot"
    );
    let mut project = super::super::lock_project(&state).unwrap();
    assert_eq!(applied, revision + 1);
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project.editor.instruction_timeline().steps[1..]
            .iter()
            .all(|step| step.visual.path_certificate_reference_v1.is_some())
    );
    assert!(
        project
            .trusted_path_certificates
            .export_attestation_v1(
                project.instance_id,
                project.project_id,
                project.editor.instruction_timeline(),
            )
            .expect("live dyadic path registry")
            .is_some(),
        "a successful dyadic Apply must be immediately export-attestable"
    );
    project.editor.undo(applied).unwrap();
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert!(
        project
            .trusted_path_certificates
            .export_attestation_v1(
                project.instance_id,
                project.project_id,
                project.editor.instruction_timeline(),
            )
            .expect("undone dyadic path registry")
            .is_none(),
        "Undo must not attest a path absent from the live timeline"
    );
    let undone = project.editor.revision();
    project.editor.redo(undone).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project
            .trusted_path_certificates
            .export_attestation_v1(
                project.instance_id,
                project.project_id,
                project.editor.instruction_timeline(),
            )
            .expect("redone dyadic path registry")
            .is_some(),
        "Redo must restore the exact live attestation binding"
    );
    let archive = project.project_archive().unwrap();
    let reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("four-hinge-tree-positive-thickness.ori2"),
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
fn five_hinge_tree_level_three_proof_applies_and_persists_atomically() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut project = five_hinge_tree_project();
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 6);
    let hinges = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    assert_eq!(hinges.len(), 5);
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
    let request =
        |level_count, max_states, max_transitions, angles: Vec<_>| DyadicPoseGraphReadRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: angles,
            max_states,
            max_transitions,
            level_count,
            cycle_schedule_v1: None,
        };
    for (levels, states, transitions) in [(5, 125, 600), (9, 128, 512)] {
        let limited = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(levels, states, transitions, target_angles.clone()),
            None,
        )
        .unwrap()
        .into_test_view();
        assert_eq!(limited.status, "resource_limit");
        assert!(!limited.mutation_candidate_ready);
    }
    let mut mismatched = target_angles.clone();
    mismatched[0].edge = ori_domain::EdgeId::new();
    assert_eq!(
        read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(3, 243, 1_620, mismatched),
            None,
        )
        .unwrap_err(),
        CYCLE_PATH_UNSUPPORTED_MESSAGE
    );
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        &state,
        Some(&layer_state),
        request(3, 243, 1_620, target_angles.clone()),
        None,
    )
    .unwrap()
    .into_test_view();
    assert_eq!(
        (observed.state_count, observed.transition_count),
        (243, 1_620)
    );
    assert_eq!(observed.status, "certified");
    assert!(observed.mutation_candidate_ready);
    assert!(observed.positive_thickness_certified);
    assert!(observed.layer_transport_certified);
    let preview_state = DyadicPathPreviewState::default();
    let preview_request = |expected_revision| DyadicPathPreviewRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision,
        target_angles: target_angles.clone(),
        max_states: 243,
        max_transitions: 1_620,
        level_count: 3,
        cycle_schedule_v1: None,
        expected_path_binding_sha256: observed.certificate_binding_sha256.clone().unwrap(),
        expected_positive_thickness_binding_sha256: observed
            .positive_thickness_binding_sha256
            .clone()
            .unwrap(),
        expected_layer_transport_binding_sha256: observed
            .layer_transport_binding_sha256
            .clone()
            .unwrap(),
    };
    assert_eq!(
        mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            preview_request(revision + 1),
        )
        .unwrap_err(),
        STALE_MESSAGE
    );
    let preview = mint_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        preview_request(revision),
    )
    .expect("five-hinge certified graph mints a read-only token");
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
        "tampered five-hinge proof is an atomic no-op"
    );
    let applied = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        apply_request(preview.path_binding_sha256.clone()),
    )
    .expect("issuer-bound five-hinge Tree proof applies atomically");
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .is_err(),
        "consumed five-hinge preview must be one-shot"
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
        std::path::PathBuf::from("five-hinge-tree-positive-thickness.ori2"),
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
fn six_hinge_tree_level_three_proof_applies_and_persists_atomically() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut project = six_hinge_tree_project();
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 7);
    let hinges = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    assert_eq!(hinges.len(), 6);
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
    let request = |level_count, max_states, max_transitions| DyadicPoseGraphReadRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        target_angles: target_angles.clone(),
        max_states,
        max_transitions,
        level_count,
        cycle_schedule_v1: None,
    };
    for (levels, states, transitions) in [(5, 125, 600), (9, 128, 512)] {
        let limited = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(levels, states, transitions),
            None,
        )
        .unwrap()
        .into_test_view();
        assert_eq!(limited.status, "resource_limit");
        assert_eq!((limited.state_count, limited.transition_count), (0, 0));
    }
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        &state,
        Some(&layer_state),
        request(3, 729, 5_832),
        None,
    )
    .unwrap()
    .into_test_view();
    assert_eq!(
        (observed.state_count, observed.transition_count),
        (729, 5_832)
    );
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
            max_states: 729,
            max_transitions: 5_832,
            level_count: 3,
            cycle_schedule_v1: None,
            expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
            expected_positive_thickness_binding_sha256: observed
                .positive_thickness_binding_sha256
                .unwrap(),
            expected_layer_transport_binding_sha256: observed
                .layer_transport_binding_sha256
                .unwrap(),
        },
    )
    .expect("six-hinge bounded proof mints a read-only token");
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
        "tampered six-hinge proof is an atomic no-op"
    );
    let applied = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &layer_state,
        &preview_state,
        apply_request(preview.path_binding_sha256.clone()),
    )
    .expect("issuer-bound six-hinge Tree proof applies atomically");
    assert!(
        apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .is_err(),
        "consumed six-hinge preview must be one-shot"
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
        std::path::PathBuf::from("six-hinge-tree-positive-thickness.ori2"),
    )
    .unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    assert!(
        reopened.editor.instruction_timeline().steps[1..]
            .iter()
            .all(|step| step.visual.path_certificate_reference_v1.is_some())
    );
}
