#[test]
fn coupled_cactus_previews_fail_closed_without_continuous_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for cycle_count in [2, 3, 4, 8, 16, 32] {
        for thickness_mm in [10_000.0, 0.1, 1.0, 3.0] {
            let (pattern, mut paper, hinges) = if cycle_count == 2 {
                super::four_bay_cycle_test_support::two_bay_rational_cycle_pattern()
            } else if cycle_count == 3 {
                super::four_bay_cycle_test_support::three_bay_rational_cycle_pattern()
            } else if cycle_count == 4 {
                super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern()
            } else if cycle_count == 8 {
                super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern()
            } else if cycle_count == 32 {
                super::four_bay_cycle_test_support::thirty_two_bay_rational_cycle_pattern()
            } else {
                super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern()
            };
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let fixed = snapshot
                .faces
                .iter()
                .find(|face| {
                    snapshot
                        .hinge_adjacency
                        .iter()
                        .filter(|adjacency| {
                            adjacency.first == face.id || adjacency.second == face.id
                        })
                        .count()
                        == 2
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
            if thickness_mm < 10_000.0 && cycle_count < 32 {
                assert!(
                    crate::applied_pose::certify_current_static_collision(
                        &app_state,
                        ori_collision::StaticCollisionLimits::default(),
                    )
                    .expect("flat cactus current collision diagnosis")
                    .is_some(),
                    "rank-{cycle_count} cactus at thickness {thickness_mm} should retain its \
                     independent static collision certificate",
                );
            }
            if cycle_count == 32 {
                assert_eq!(
                    propose_current_cycle_pose_inner(
                        None,
                        &app_state,
                        &transactions,
                        CurrentCyclePosePreviewRequestV1 {
                            progress_request_id: Some("rank32:stale".to_owned()),
                            expected_project_instance_id: ProjectId::new(),
                            expected_project_id: project_id,
                            expected_revision: revision,
                            cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
                        },
                    )
                    .unwrap_err(),
                    STALE_MESSAGE,
                    "rank-{cycle_count} cactus at thickness {thickness_mm} must reject stale \
                     project identity before path diagnosis",
                );
            }
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
            let error = match response {
                Err(error) => error,
                Ok(_) => panic!(
                    "rank-{cycle_count} cactus at thickness {thickness_mm} must not receive \
                     positive-thickness continuous authority",
                ),
            };
            assert_eq!(
                error, CYCLE_PATH_UNCERTIFIED_MESSAGE,
                "rank-{cycle_count} cactus at thickness {thickness_mm} must fail closed \
                 without a continuous positive-thickness certificate",
            );
            let project = super::super::lock_project(&app_state).unwrap();
            assert!(
                project.editor.instruction_timeline().steps.is_empty(),
                "rank-{cycle_count} cactus at thickness {thickness_mm} must not mutate history",
            );
            assert_eq!(
                project.editor.revision(),
                revision,
                "rank-{cycle_count} cactus at thickness {thickness_mm} must not advance revision",
            );
            assert!(
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    ProjectId::new(),
                )
                .is_err(),
                "rank-{cycle_count} cactus at thickness {thickness_mm} must not leave an \
                 applicable transaction",
            );
        }
    }
}

#[test]
fn rank4_cycle_transports_layer_order_and_applies_atomically() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for (fixture_index, (columns, rows, thickness_mm, expected_cycle_rank)) in [
        (3, 3, 0.1, 4),
        (3, 3, 1.0, 4),
        (3, 3, 3.0, 4),
        (3, 3, 10_000.0, 4),
        (3, 5, 0.1, 8),
        (5, 5, 0.1, 16),
        (5, 9, 0.1, 32),
        (7, 7, 0.1, 36),
        (7, 9, 0.1, 48),
        (8, 9, 0.1, 56),
        (9, 9, 0.1, 64),
    ]
    .into_iter()
    .enumerate()
    {
        let (pattern, mut paper, horizontal, _) =
            super::dense_grid_cycle_test_support::miura_authority_pattern(columns, rows);
        let moving = horizontal.into_iter().take(columns).collect::<Vec<_>>();
        paper.thickness_mm = thickness_mm;
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        set_fixed_cycle_fixture_identity_v1(&mut project, 6, fixture_index as u16);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(
            snapshot.hinge_adjacency.len() + 1 - snapshot.faces.len(),
            expected_cycle_rank
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
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
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
                    .is_some_and(|hinge| {
                        hinge.assignment == ori_topology::FoldAssignment::Mountain
                    });
                if mountain ^ (mask & (1 << index) != 0) {
                    entry.numerator_power_coefficients[1].numerator *= -1;
                    entry.requested_angle_degrees *= -1.0;
                }
            }
            schedule
        };
        let request = |expected_project_instance_id, mask| CurrentCyclePosePreviewRequestV1 {
            progress_request_id: Some("rank4:layer".to_owned()),
            expected_project_instance_id,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: schedule_for(mask),
        };
        assert_eq!(
            propose_current_cycle_pose_inner_with_layers(
                None,
                &app_state,
                Some(&layer_state),
                &transactions,
                request(ProjectId::new(), 0),
            )
            .unwrap_err(),
            STALE_MESSAGE
        );
        if thickness_mm == 10_000.0 {
            assert!((0..(1usize << moving.len())).all(|mask| {
                propose_current_cycle_pose_inner_with_layers(
                    None,
                    &app_state,
                    Some(&layer_state),
                    &transactions,
                    request(instance, mask),
                )
                .is_err()
            }));
            let project = super::super::lock_project(&app_state).unwrap();
            assert_eq!(project.editor.revision(), revision);
            assert!(project.editor.instruction_timeline().steps.is_empty());
            continue;
        }
        let mut malformed = request(instance, 0);
        malformed.cycle_schedule_v1.entries[0].denominator_power_coefficients[0].numerator = 0;
        assert!(
            propose_current_cycle_pose_inner_with_layers(
                None,
                &app_state,
                Some(&layer_state),
                &transactions,
                malformed,
            )
            .is_err()
        );
        assert!(
            super::super::lock_project(&app_state)
                .unwrap()
                .editor
                .instruction_timeline()
                .steps
                .is_empty()
        );
        let Some((closing_mask, preview)) = (0..(1usize << moving.len())).find_map(|mask| {
            propose_current_cycle_pose_inner_with_layers(
                None,
                &app_state,
                Some(&layer_state),
                &transactions,
                request(instance, mask),
            )
            .ok()
            .map(|preview| (mask, preview))
        }) else {
            let project = super::super::lock_project(&app_state).unwrap();
            assert_eq!(project.editor.revision(), revision);
            assert!(project.editor.instruction_timeline().steps.is_empty());
            continue;
        };
        assert_eq!(
            preview.continuous_layer_transport_model_id,
            Some(ori_collision::GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1)
        );
        assert_eq!(preview.continuous_layer_transition_count, 2);
        assert_eq!(preview.source_layer_order, preview.target_layer_order);
        assert_eq!(
            preview.continuous_layer_pair_order_count,
            preview.source_layer_order.len()
        );
        assert!(!preview.authorizes_project_mutation);
        let cancelled = preview.transaction_token;
        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
            &transactions,
            cancelled,
        )
        .unwrap();
        assert!(
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &layer_state,
                &transactions,
                cancelled,
            )
            .is_err()
        );
        let stale_authority_preview = propose_current_cycle_pose_inner_with_layers(
            None,
            &app_state,
            Some(&layer_state),
            &transactions,
            request(instance, closing_mask),
        )
        .expect("rank4 layer authority ABA preview");
        {
            let project = super::super::lock_project(&app_state).unwrap();
            super::super::global_flat_foldability::tests::install_possible_layer_order(
                &layer_state,
                &project,
            );
        }
        assert!(
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &layer_state,
                &transactions,
                stale_authority_preview.transaction_token,
            )
            .is_err()
        );
        let preview = propose_current_cycle_pose_inner_with_layers(
            None,
            &app_state,
            Some(&layer_state),
            &transactions,
            request(instance, closing_mask),
        )
        .expect("rank4 layer transport retry");
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &layer_state,
            &transactions,
            preview.transaction_token,
        )
        .expect("rank4 layer transport apply");
        let mut project = super::super::lock_project(&app_state).unwrap();
        let persisted = project.editor.instruction_timeline().steps[0]
            .visual
            .cycle_layer_order_proof_v1
            .as_ref()
            .expect("applied transport proof is persisted in timeline history");
        assert_eq!(persisted.version, 1);
        assert_eq!(
            persisted.model_id,
            ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1
        );
        assert_eq!(persisted.target_order_sha256.len(), 32);
        if expected_cycle_rank == 16 {
            let pose = project.editor.current_applied_pose().unwrap();
            let fixed_face = pose.fixed_face();
            let angles = ori_kinematics::CanonicalHingeAngles::new(
                pose.hinge_angles()
                    .iter()
                    .map(|angle| {
                        ori_kinematics::HingeAngle::new(angle.edge(), angle.angle_degrees())
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let predecessor = project
                .editor
                .clone_predecessor_if_last_stacked_fold_v1()
                .expect("current-cycle apply has an authenticated predecessor");
            let proof = super::super::global_flat_foldability::
                revalidate_authenticated_non_refining_graph_layer_evidence(
                    project.project_id,
                    &predecessor,
                    &project.editor,
                    fixed_face,
                    &angles,
                    super::super::global_flat_foldability::archive_revalidation_deadline()
                        .unwrap(),
                )
                .expect("authenticated same-geometry graph evidence");
            project.current_layer_evidence =
                Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(proof));
            assert!(matches!(
                &project.current_layer_evidence,
                Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(_))
            ));
            let archive = project.project_archive().unwrap();
            assert!(matches!(
                &archive.layer_evidence,
                Some(ori_formats::LayerEvidenceArchiveV1 {
                    evidence: ori_formats::LayerEvidenceArchiveKindV1::NonFlat { .. },
                    ..
                })
            ));
            let mut reopened = super::super::ProjectState::from_project_archive(
                archive,
                std::path::PathBuf::from("rank16-graph-layer-evidence.ori2"),
            )
            .unwrap();
            assert!(matches!(
                &reopened.current_layer_evidence,
                Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(_))
            ));
            let revision = reopened.editor.revision();
            let reopened_instance = reopened.instance_id;
            let reopened_project = reopened.project_id;
            let vertex = reopened.editor.pattern().vertices[0].id;
            let position = reopened.editor.pattern().vertices[0].position;
            super::super::execute_command(
                &mut reopened,
                reopened_instance,
                reopened_project,
                revision,
                ori_core::Command::MoveVertex {
                    id: vertex,
                    position: ori_domain::Point2::new(position.x + 0.125, position.y),
                },
            )
            .unwrap();
            assert!(reopened.current_layer_evidence.is_none());
        }
        project.editor.undo(applied).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        assert!(
            project.editor.instruction_timeline().steps[0]
                .visual
                .cycle_layer_order_proof_v1
                .is_some()
        );
        let reopened = super::super::ProjectState::from_valid_document(
            project.document(),
            std::path::PathBuf::from("miura-cell-transport-reopened.ori2"),
        );
        let reopened_proof = reopened.editor.instruction_timeline().steps[0]
            .visual
            .cycle_layer_order_proof_v1
            .as_ref()
            .expect("persisted Miura cell proof survives reopen");
        assert_eq!(
            reopened_proof.model_id,
            ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1
        );
        assert_eq!(reopened_proof.target_order_sha256.len(), 32);
    }
}

#[test]
fn theta_positive_thickness_preview_applies_and_round_trips_history() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for (fixture_index, thickness_mm) in [0.1, 1.0, 3.0].into_iter().enumerate() {
        let (pattern, mut paper, hinges, moving) =
            super::theta_cycle_test_support::theta_shared_hinge_pattern();
        paper.thickness_mm = thickness_mm;
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        set_fixed_cycle_fixture_identity_v1(&mut project, 7, fixture_index as u16);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
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
        let request = || CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: theta_cycle_schedule(&hinges, &moving),
        };
        let mut broken = request();
        broken.cycle_schedule_v1.entries[0].requested_angle_degrees += 1.0;
        assert!(propose_current_cycle_pose_inner(None, &app_state, &transactions, broken).is_err());
        assert_eq!(
            super::super::lock_project(&app_state)
                .unwrap()
                .editor
                .revision(),
            revision
        );
        let replaced = propose_current_cycle_pose_inner(None, &app_state, &transactions, request())
            .expect("theta preview");
        let cancelled =
            propose_current_cycle_pose_inner(None, &app_state, &transactions, request())
                .expect("theta replacement preview");
        assert!(
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &GlobalFlatFoldabilityState::default(),
                &transactions,
                replaced.transaction_token,
            )
            .is_err()
        );
        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
            &transactions,
            cancelled.transaction_token,
        )
        .unwrap();
        assert!(
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &GlobalFlatFoldabilityState::default(),
                &transactions,
                cancelled.transaction_token,
            )
            .is_err()
        );
        let response = propose_current_cycle_pose_inner(None, &app_state, &transactions, request())
            .expect("theta retry");
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            response.transaction_token,
        )
        .unwrap();
        let mut project = super::super::lock_project(&app_state).unwrap();
        project.editor.undo(applied).unwrap();
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    }
}
