fn assert_two_hinge_projective_schedule_round_trip(
    first: [f64; 3],
    second: [f64; 3],
    paper_thickness_mm: f64,
    certified_path_steps: usize,
    cancel_after_transition: Option<usize>,
    expected_non_flat_pose_model_id: Option<&'static str>,
) -> Vec<(String, String, String)> {
    let _generation_guard = STACKED_FOLD_READ_GENERATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut project = two_hinge_tree_project(paper_thickness_mm);
    super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let layer_state = GlobalFlatFoldabilityState::default();
    {
        let project = super::super::lock_project(&app_state).unwrap();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
    }
    let certified_path = certified_path_steps > 0;
    let angle = if certified_path {
        certified_path_steps as f64
    } else {
        ori_kinematics::deterministic_half_angle_ratio_degrees_v1(1.0, 5.0)
            .expect("the canonical half-angle fixture is finite")
    };
    let registry = tauri::async_runtime::block_on(read_live_hinge_registry_inner(
        &app_state,
        &layer_state,
        LiveHingeRegistryRequestV1::for_test(
            instance,
            project_id,
            revision,
            first,
            second,
            FixedSideRequest::Left,
            RotationDirectionRequest::Positive,
            angle,
        ),
    ))
    .expect("live target hinge registry");
    let registry_entries = registry.entries_for_test();
    assert!(registry_entries.len() >= 2);
    let cycle_schedule_v1 = CycleScheduleRequestV1 {
        version: 1,
        endpoint_denominator: None,
        entries: registry_entries
            .iter()
            .map(|entry| {
                let is_source_hinge =
                    entry.initial_angle_degrees_for_test().to_bits() == 180.0_f64.to_bits();
                CycleScheduleEntryRequestV1 {
                    edge: entry.edge_for_test(),
                    u_domain: [
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: if is_source_hinge {
                        vec![RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        }]
                    } else {
                        vec![
                            RationalCoefficientRequestV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientRequestV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ]
                    },
                    denominator_power_coefficients: if is_source_hinge {
                        vec![RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    } else {
                        vec![RationalCoefficientRequestV1 {
                            numerator: 5,
                            denominator: 1,
                        }]
                    },
                    requested_angle_degrees: if is_source_hinge { 180.0 } else { angle },
                }
            })
            .collect(),
    };
    let certified_path_graph_v1 = certified_path.then(|| CertifiedPathGraphRequestV1 {
        version: 1,
        states: (0..=certified_path_steps)
            .map(|step| step as f64 / certified_path_steps as f64)
            .map(|progress| CertifiedPathGraphStateRequestV1 {
                entries: registry_entries
                    .iter()
                    .map(|entry| CertifiedPathGraphAngleRequestV1 {
                        edge: entry.edge_for_test(),
                        angle_degrees: if entry.initial_angle_degrees_for_test().to_bits()
                            == 180.0_f64.to_bits()
                        {
                            180.0
                        } else {
                            angle * progress
                        },
                    })
                    .collect(),
            })
            .collect(),
        transitions: (0..certified_path_steps)
            .map(|step| CertifiedPathGraphTransitionRequestV1 {
                source_state: step,
                target_state: step + 1,
            })
            .collect(),
        source_state: 0,
        target_state: certified_path_steps,
    });
    let transaction_state =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let generation_before_proposal = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    let response = tauri::async_runtime::block_on(propose_current_stacked_fold_read_inner(
        None,
        &app_state,
        &layer_state,
        &transaction_state,
        StackedFoldReadRequest {
            progress_request_id: cancel_after_transition
                .map(|step| format!("test-cancel-after-{step}")),
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            first,
            second,
            fixed_side: FixedSideRequest::Left,
            rotation_direction: RotationDirectionRequest::Positive,
            requested_angle_degrees: angle,
            cycle_schedule_v1: (!certified_path).then_some(cycle_schedule_v1),
            linear_candidate_v1: None,
            certified_path_graph_v1,
        },
    ));
    if certified_path && paper_thickness_mm != 0.0 {
        assert_eq!(response.unwrap_err(), CYCLE_PATH_UNSUPPORTED_MESSAGE);
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            generation_before_proposal,
            "positive-thickness Graph rejection must precede generation and cancellation",
        );
        assert_eq!(transaction_state.pending_token_for_test_v1(), None);
        let project = super::super::lock_project(&app_state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        return Vec::new();
    }
    if cancel_after_transition.is_some() {
        assert_eq!(response.unwrap_err(), CANCELLED_MESSAGE);
        let project = super::super::lock_project(&app_state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        return Vec::new();
    }
    let response = response.expect("genuine ready preview");
    let certificate_hashes = if certified_path {
        let graph = response
            .certified_path_graph
            .as_ref()
            .expect("certified path graph preview");
        assert_eq!(graph.explored_state_count, certified_path_steps);
        assert_eq!(graph.evaluated_transition_count, certified_path_steps);
        assert_eq!(graph.edges.len(), certified_path_steps);
        assert!(graph.edges.iter().all(|edge| {
            edge.schedule_certificate_sha256.len() == 64
                && edge.collision_certificate_sha256.len() == 64
                && edge.closure_certificate_sha256.len() == 64
        }));
        assert!(!graph.authorizes_project_mutation);
        graph
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.schedule_certificate_sha256.clone(),
                    edge.collision_certificate_sha256.clone(),
                    edge.closure_certificate_sha256.clone(),
                )
            })
            .collect()
    } else {
        assert!(response.certified_path_graph.is_none());
        Vec::new()
    };
    assert!(response.transaction_proposal.ready_for_atomic_apply);
    let token = response
        .transaction_proposal
        .transaction_token
        .expect("ready token");
    let before = {
        let project = super::super::lock_project(&app_state).unwrap();
        project.editor.clone()
    };
    let applied_revision =
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &layer_state,
            &transaction_state,
            token,
        )
        .expect("atomic apply");
    let mut project = super::super::lock_project(&app_state).unwrap();
    assert_eq!(project.editor.revision(), applied_revision);
    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    assert_eq!(response.transaction_proposal.timeline_step_count, 1);
    let after = project.editor.clone();
    if let Some(expected_pose_model_id) = expected_non_flat_pose_model_id {
        include!("read_suite_05_non_flat_archive_body.rs");
    }
    let source_vertices = before
        .pattern()
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<std::collections::HashSet<_>>();
    let inserted = after
        .pattern()
        .vertices
        .iter()
        .filter(|vertex| !source_vertices.contains(&vertex.id))
        .collect::<Vec<_>>();
    assert!(
        !inserted.is_empty(),
        "the new straight line must atomically materialize its source-hinge intersections"
    );
    let line_start = ori_domain::Point2::new(first[0], -first[2]);
    let line_end = ori_domain::Point2::new(second[0], -second[2]);
    let line_length =
        ((line_end.x - line_start.x).powi(2) + (line_end.y - line_start.y).powi(2)).sqrt();
    let position_tolerance = 1.0e-9_f64;
    let inserted_on_requested_line = inserted
        .iter()
        .filter(|vertex| {
            let cross = (line_end.x - line_start.x) * (vertex.position.y - line_start.y)
                - (line_end.y - line_start.y) * (vertex.position.x - line_start.x);
            cross.abs() <= position_tolerance * line_length.max(1.0)
                && vertex.position.x + position_tolerance >= line_start.x.min(line_end.x)
                && vertex.position.x - position_tolerance <= line_start.x.max(line_end.x)
                && vertex.position.y + position_tolerance >= line_start.y.min(line_end.y)
                && vertex.position.y - position_tolerance <= line_start.y.max(line_end.y)
        })
        .count();
    assert!(
        inserted_on_requested_line >= 2,
        "both source hinges must gain a materialized intersection on the requested line"
    );
    assert!(after.pattern().edges.len() > before.pattern().edges.len());
    super::super::execute_undo(&mut project, instance, project_id, applied_revision)
        .expect("undo atomically applied stacked fold");
    assert_eq!(project.editor.pattern(), before.pattern());
    assert_eq!(
        project.editor.current_applied_pose(),
        before.current_applied_pose()
    );
    assert!(project.current_layer_evidence.is_none());
    let undo_revision = project.editor.revision();
    super::super::execute_redo(&mut project, instance, project_id, undo_revision)
        .expect("redo atomically applied stacked fold");
    assert_eq!(project.editor.pattern(), after.pattern());
    assert_eq!(
        project.editor.current_applied_pose(),
        after.current_applied_pose()
    );
    assert!(project.current_layer_evidence.is_none());
    let archive = project
        .project_archive()
        .expect("serialize split-hinge cycle operation");
    assert!(
        archive.layer_evidence.is_none(),
        "redo must not resurrect invalidated layer-order evidence"
    );
    let mut reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("split-hinge-cycle.ori2"),
    )
    .expect("reopen split-hinge cycle operation");
    assert_eq!(reopened.editor.pattern(), after.pattern());
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
    assert!(reopened.current_layer_evidence.is_none());
    let reopened_instance = reopened.instance_id;
    let reopened_project_id = reopened.project_id;
    let reopened_revision = reopened.editor.revision();
    super::super::execute_undo(
        &mut reopened,
        reopened_instance,
        reopened_project_id,
        reopened_revision,
    )
    .expect("undo reopened stacked fold");
    assert_eq!(reopened.editor.pattern(), before.pattern());
    assert!(reopened.current_layer_evidence.is_none());
    let reopened_redo_revision = reopened.editor.revision();
    super::super::execute_redo(
        &mut reopened,
        reopened_instance,
        reopened_project_id,
        reopened_redo_revision,
    )
    .expect("redo reopened stacked fold");
    assert_eq!(reopened.editor.pattern(), after.pattern());
    assert_eq!(
        reopened.editor.current_applied_pose(),
        after.current_applied_pose()
    );
    assert!(reopened.current_layer_evidence.is_none());
    certificate_hashes
}
