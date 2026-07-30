#[test]
fn sixteen_sector_upper_bound_previews_applies_reopens_and_rejects_nonopposite_pair() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, moving) = sixteen_sector_cycle_pattern(8);
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    set_fixed_cycle_fixture_identity_v1(&mut project, 5, 0);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 16);
    assert_eq!(snapshot.hinge_adjacency.len(), 16);
    let graph_geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
        project.editor.pattern(),
        project.editor.paper(),
        &snapshot,
        ori_kinematics::TreeKinematicsLimits::default(),
    )
    .unwrap();
    let graph_audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
        &snapshot,
        ori_kinematics::TreeKinematicsLimits::default(),
    )
    .unwrap();
    let automatic_pairs = ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
        &graph_geometry,
        &graph_audit,
        120,
    )
    .expect("bounded C16 opposite-pair discovery");
    assert!(
        automatic_pairs
            .iter()
            .any(|pair| { pair.iter().all(|edge| moving.contains(edge)) })
    );
    assert!(matches!(
        ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
            &graph_geometry,
            &graph_audit,
            119,
        ),
        Err(ori_kinematics::KinematicsError::ResourceLimitExceeded)
    ));
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
    )
    .expect("sixteen-sector opposite pair must certify");
    assert_eq!(preview.checked_hinge_count, 16);
    assert_eq!(preview.total_hinge_count, 16);
    let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &transactions,
        preview.transaction_token,
    )
    .expect("sixteen-sector opposite pair apply");
    let second_preview = propose_current_cycle_pose_inner(
        None,
        &state,
        &transactions,
        CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: applied,
            cycle_schedule_v1: advance_collective_schedule(&hinges, &moving, 100),
        },
    )
    .expect("C16 rebound authority must authorize the second preview");
    assert_eq!(second_preview.checked_hinge_count, 16);
    assert_eq!(second_preview.total_hinge_count, 16);
    let second_applied =
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            second_preview.transaction_token,
        )
        .expect("second C16 operation applies atomically");
    let mut project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    project.editor.undo(second_applied).unwrap();
    let first_undone = project.editor.revision();
    project.editor.undo(first_undone).unwrap();
    assert!(project.editor.instruction_timeline().steps.is_empty());
    let first_redo = project.editor.revision();
    project.editor.redo(first_redo).unwrap();
    let second_redo = project.editor.revision();
    project.editor.redo(second_redo).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    let archive = project.project_archive().expect("serialize C16 cycle");
    let mut reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("sixteen-cycle.ori2"),
    )
    .expect("reopen C16 cycle");
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    let reopened_revision = reopened.editor.revision();
    reopened.editor.undo(reopened_revision).unwrap();
    let reopened_first_undone = reopened.editor.revision();
    reopened.editor.undo(reopened_first_undone).unwrap();
    assert!(reopened.editor.instruction_timeline().steps.is_empty());
    let reopened_first_redo = reopened.editor.revision();
    reopened.editor.redo(reopened_first_redo).unwrap();
    let reopened_second_redo = reopened.editor.revision();
    reopened.editor.redo(reopened_second_redo).unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);

    let (pattern, paper, nonopposite) = sixteen_sector_cycle_pattern(7);
    let mut rejected = super::super::ProjectState::new_with_paper(pattern, paper);
    let rejected_topology = rejected
        .editor
        .topology_analysis_input(rejected.project_id)
        .analyze();
    let rejected_snapshot = rejected_topology.simulation_snapshot().unwrap();
    let rejected_hinges = rejected_snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut rejected,
        rejected_hinges.clone(),
        rejected_snapshot.faces[0].id,
    );
    let rejected_instance = rejected.instance_id;
    let rejected_project_id = rejected.project_id;
    let rejected_revision = rejected.editor.revision();
    let rejected_state = AppState::new(rejected);
    assert_eq!(
        propose_current_cycle_pose_inner(
            None,
            &rejected_state,
            &super::super::stacked_fold_transaction::StackedFoldTransactionState::default(),
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: rejected_instance,
                expected_project_id: rejected_project_id,
                expected_revision: rejected_revision,
                cycle_schedule_v1: dense_grid_schedule(&rejected_hinges, &nonopposite, 100,),
            },
        )
        .unwrap_err(),
        CYCLE_NONCLOSING_MESSAGE
    );
}

#[test]
fn four_leaf_moving_cactus_preview_fails_closed_without_continuous_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, hinges) =
        super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern();
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    let fixed = snapshot
        .faces
        .iter()
        .max_by_key(|face| {
            snapshot
                .hinge_adjacency
                .iter()
                .filter(|adjacency| adjacency.first == face.id || adjacency.second == face.id)
                .count()
        })
        .unwrap()
        .id;
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        fixed,
    );
    {
        let capability = project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .unwrap();
        let (geometry, audit, _) = capability.graph().unwrap();
        let basis = geometry
            .extract_canonical_cycle_basis_v1(audit, CycleBasisLimitsV1::default())
            .expect("four-cycle canonical basis");
        assert_eq!(basis.cycles().len(), 4);
        assert!(
            geometry
                .extract_canonical_cycle_basis_v1(
                    audit,
                    CycleBasisLimitsV1 {
                        max_cycles: 3,
                        ..CycleBasisLimitsV1::default()
                    },
                )
                .is_err()
        );
    }
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let transaction_state =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let mut corrupted = four_bay_cycle_schedule(&hinges);
    corrupted
        .entries
        .iter_mut()
        .find(|entry| entry.edge == hinges[12])
        .unwrap()
        .numerator_power_coefficients[1]
        .numerator += 1;
    assert!(
        propose_current_cycle_pose_inner(
            None,
            &app_state,
            &transaction_state,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: corrupted,
            },
        )
        .is_err()
    );
    assert!(
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &GlobalFlatFoldabilityState::default(),
            &transaction_state,
            ProjectId::new(),
        )
        .is_err()
    );
    let response = propose_current_cycle_pose_inner(
        None,
        &app_state,
        &transaction_state,
        CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
        },
    );
    assert_eq!(
        response.unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
        "four-leaf moving cactus must not mint continuous authority from closure and sampled \
         clearance alone",
    );
    let project = super::super::lock_project(&app_state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}
