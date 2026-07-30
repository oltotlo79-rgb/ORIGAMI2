#[test]
fn eight_leaf_moving_cactus_preview_fails_closed_without_continuous_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, hinges) =
        super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern();
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
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let response = propose_current_cycle_pose_inner(
        None,
        &app_state,
        &transactions,
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
        "eight-leaf moving cactus must not mint continuous authority from closure and sampled \
         clearance alone",
    );
    let project = super::super::lock_project(&app_state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn sixteen_leaf_moving_cactus_preview_fails_closed_without_continuous_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, hinges) =
        super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern();
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
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let response = propose_current_cycle_pose_inner(
        None,
        &app_state,
        &transactions,
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
        "sixteen-leaf moving cactus must not mint continuous authority from closure and \
         sampled clearance alone",
    );
    let project = super::super::lock_project(&app_state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn current_graph_cycle_authenticates_or_fails_closed_three_times() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let mut authenticated = 0;
    let mut rejected = Vec::new();
    for iteration in 0..3 {
        let (mut project, hinges) =
            super::super::applied_pose::tests::flat_foldable_cross_cycle_project();
        set_zero_thickness_for_cycle_test_v1(&mut project);
        super::super::applied_pose::tests::install_flat_graph_pose_authority(
            &mut project,
            hinges.clone(),
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let transaction_state =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        assert_eq!(
            propose_current_cycle_pose_inner(
                None,
                &app_state,
                &transaction_state,
                CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision + 1,
                    cycle_schedule_v1: physical_four_vertex_cycle_schedule(&hinges),
                },
            )
            .unwrap_err(),
            STALE_MESSAGE
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
                cycle_schedule_v1: physical_four_vertex_cycle_schedule(&hinges),
            },
        );
        match response {
            Ok(mut response) => {
                authenticated += 1;
                assert!(response.closure_leaf_count > 0);
                assert!(response.closure_max_depth <= 16);
                assert_eq!(response.checked_hinge_count, response.total_hinge_count);
                assert_eq!(response.total_hinge_count, hinges.len());
                assert!(
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &app_state,
                        &GlobalFlatFoldabilityState::default(),
                        &transaction_state,
                        ProjectId::new(),
                    )
                    .is_err()
                );
                if iteration == 0 {
                    let cancelled = response.transaction_token;
                    super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
                        &transaction_state,
                        cancelled,
                    )
                    .unwrap();
                    assert!(
                        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                            &app_state,
                            &GlobalFlatFoldabilityState::default(),
                            &transaction_state,
                            cancelled,
                        )
                        .is_err()
                    );
                    response = propose_current_cycle_pose_inner(
                        None,
                        &app_state,
                        &transaction_state,
                        CurrentCyclePosePreviewRequestV1 {
                            progress_request_id: None,
                            expected_project_instance_id: instance,
                            expected_project_id: project_id,
                            expected_revision: revision,
                            cycle_schedule_v1: physical_four_vertex_cycle_schedule(&hinges),
                        },
                    )
                    .expect("replacement authenticated preview");
                    assert_ne!(response.transaction_token, cancelled);
                }
                assert!(!response.authorizes_project_mutation);
                assert!(response.continuous_path_certified);
                let applied =
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &app_state,
                        &GlobalFlatFoldabilityState::default(),
                        &transaction_state,
                        response.transaction_token,
                    )
                    .expect("authenticated atomic apply");
                let mut project = super::super::lock_project(&app_state).unwrap();
                assert_eq!(applied, revision + 1);
                assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
                project.editor.undo(applied).unwrap();
                let undo_revision = project.editor.revision();
                project.editor.redo(undo_revision).unwrap();
            }
            Err(error) => {
                rejected.push(error.clone());
                assert!(
                    error == CYCLE_NONCLOSING_MESSAGE || error == CYCLE_PATH_UNCERTIFIED_MESSAGE,
                    "unexpected fail-closed category: {error}"
                );
                let project = super::super::lock_project(&app_state).unwrap();
                assert_eq!(project.editor.revision(), revision);
                assert!(project.editor.instruction_timeline().steps.is_empty());
            }
        }
    }
    assert_eq!(
        authenticated, 3,
        "fixed native fixture must authenticate; rejected={rejected:?}"
    );
}

#[test]
fn current_cycle_generation_replacement_and_cancel_are_monotonic() {
    let _guard = STACKED_FOLD_READ_GENERATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let first = begin_stacked_fold_read_generation_v1().unwrap();
    let replacement = begin_stacked_fold_read_generation_v1().unwrap();
    assert!(replacement > first);
    assert_ne!(STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire), first);
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        replacement
    );
    cancel_current_stacked_fold_read_inner_v1().unwrap();
    assert_ne!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        replacement
    );
}
