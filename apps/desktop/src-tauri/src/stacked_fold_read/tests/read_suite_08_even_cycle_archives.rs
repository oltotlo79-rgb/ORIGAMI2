#[test]
fn hole_boundary_strict_dyadic_read_fails_closed_without_mutation_authority() {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x73,
    ]);
    let coordinates = [
        (0.0, 0.0),
        (1.0, 0.0),
        (8.0, 0.0),
        (8.0, 8.0),
        (1.0, 8.0),
        (0.0, 8.0),
        (2.0, 2.0),
        (6.0, 2.0),
        (4.0, 6.0),
    ];
    let vertices = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let hinge = EdgeId::derive_v5(namespace, b"hole-fixture-hinge");
    let mut edges = (0..6)
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x20, index as u8]),
            start: vertices[index].id,
            end: vertices[(index + 1) % 6].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: hinge,
        start: vertices[1].id,
        end: vertices[4].id,
        kind: EdgeKind::Mountain,
    });
    for (index, (start, end)) in [(6, 7), (7, 8), (8, 6)].into_iter().enumerate() {
        edges.push(Edge {
            id: EdgeId::derive_v5(namespace, &[0x30, index as u8]),
            start: vertices[start].id,
            end: vertices[end].id,
            kind: EdgeKind::Cut,
        });
    }
    let paper = Paper {
        boundary_vertices: vertices[..6].iter().map(|vertex| vertex.id).collect(),
        thickness_mm: 0.1,
        cutting_allowed: true,
        ..Paper::default()
    };
    let project =
        super::super::ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper);
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
                edge: hinge,
                angle_degrees: 1.0,
            }],
            max_states: 32,
            max_transitions: 64,
            level_count: 3,
            cycle_schedule_v1: None,
        },
        None,
    )
    .expect("hole read returns a fail-closed observation")
    .into_test_view();
    assert_eq!(observed.reason, "unsupported_geometry");
    assert!(!observed.mutation_candidate_ready);
    assert!(!observed.authorizes_project_mutation);
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert!(
        project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .is_none()
    );
}

#[test]
fn even_cycle_exact_schedules_are_admitted_by_strict_dyadic_read() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (c8_pattern, c8_paper, c8_cardinal) = octagonal_eight_sector_cycle_pattern();
    let c8_opposite = vec![c8_cardinal[0], c8_cardinal[2]];
    for (fixture_index, (fixture_name, (pattern, mut paper, moving))) in [
        ("balloon-c6", balloon_six_sector_cycle_pattern()),
        ("octagonal-c8", (c8_pattern, c8_paper, c8_opposite)),
    ]
    .into_iter()
    .enumerate()
    {
        paper.thickness_mm = 0.1;
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        set_fixed_cycle_fixture_identity_v1(&mut project, 3, fixture_index as u16);
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
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let endpoint_ratio = match hinges.len() {
            6 => (4, 3),
            8 => (7, 3),
            _ => unreachable!("bounded opposite-pair fixture"),
        };
        let schedule =
            dense_grid_schedule_ratio(&hinges, &moving, endpoint_ratio.0, endpoint_ratio.1);
        let target = {
            let project = super::super::lock_project(&state).unwrap();
            let capability = project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .unwrap();
            let (geometry, audit, pose) = capability.graph().unwrap();
            prepare_requested_cycle_schedule_v1(
                &schedule,
                geometry,
                audit,
                pose.fixed_face(),
                pose.hinge_angles(),
            )
            .unwrap()
            .evaluate(1.0)
            .unwrap()
        };
        let target_angles = target
            .as_slice()
            .iter()
            .map(|angle| DyadicPoseGraphAngleDtoV1 {
                edge: angle.edge(),
                angle_degrees: angle.angle_degrees(),
            })
            .collect::<Vec<_>>();
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            DyadicPoseGraphReadRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: target_angles
                    .iter()
                    .map(|angle| DyadicPoseGraphAngleDtoV1 {
                        edge: angle.edge,
                        angle_degrees: angle.angle_degrees,
                    })
                    .collect(),
                max_states: 32,
                max_transitions: 128,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        )
        .unwrap_or_else(|error| panic!("{fixture_name} exact schedule dyadic read: {error}"))
        .into_test_view();
        assert_eq!(observed.status, "certified");
        assert_eq!(observed.state_count, 3);
        assert_eq!(observed.transition_count, 4);
        assert!(observed.certified_transition_count > 0);
        assert!(observed.positive_thickness_certified);
        assert!(observed.layer_transport_certified);
        assert!(observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);

        let expected_steps = observed.certified_transition_count + 1;
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
                max_states: 32,
                max_transitions: 128,
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
        .unwrap_or_else(|error| panic!("{fixture_name} proof families mint preview: {error}"));
        let apply_request =
            |expected_revision: u64, path: String| ApplyDyadicPathPreviewRequestV1 {
                preview_token: preview.preview_token,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision,
                expected_target_binding_sha256: preview.target_binding_sha256.clone(),
                expected_path_binding_sha256: path,
                expected_positive_thickness_binding_sha256: preview
                    .positive_thickness_binding_sha256
                    .clone(),
                expected_layer_transport_binding_sha256: preview
                    .layer_transport_binding_sha256
                    .clone(),
            };
        for rejected in [
            apply_request(revision, "00".repeat(32)),
            apply_request(revision + 1, preview.path_binding_sha256.clone()),
        ] {
            assert!(
                apply_dyadic_pose_path_preview_inner_v1(
                    &state,
                    &layer_state,
                    &preview_state,
                    rejected,
                )
                .is_err()
            );
            assert_eq!(
                super::super::lock_project(&state)
                    .unwrap()
                    .editor
                    .revision(),
                revision,
                "tamper and stale attempts are atomic no-ops"
            );
        }
        let applied = apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(revision, preview.path_binding_sha256.clone()),
        )
        .unwrap_or_else(|error| panic!("{fixture_name} path applies atomically: {error}"));
        assert!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request(revision, preview.path_binding_sha256),
            )
            .is_err()
        );
        let mut project = super::super::lock_project(&state).unwrap();
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
        assert!(project.editor.instruction_timeline().steps.is_empty());
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(
            project.editor.instruction_timeline().steps.len(),
            expected_steps
        );
        let archive = project.project_archive().unwrap();
        drop(project);
        let reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from(format!("{fixture_name}-dyadic-authority.ori2")),
        )
        .expect("reopen proof-bearing degree-six balloon path");
        assert_eq!(
            reopened.editor.instruction_timeline().steps.len(),
            expected_steps
        );
        assert!(
            reopened.editor.instruction_timeline().steps[1..]
                .iter()
                .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
        );
        // Instruction rendering has its own bounded topology contract;
        // this regression authenticates exact cycle read/apply/history.
    }
}

#[test]
fn automatic_kawasaki_archive_reopens_with_native_pose_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (mut project, hinges) = super::super::applied_pose::tests::four_vertex_cycle_project();
    set_zero_thickness_for_cycle_test_v1(&mut project);
    super::super::applied_pose::tests::install_flat_graph_pose_authority(&mut project, hinges);
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
            cycle_schedule_v1: CycleScheduleRequestV1 {
                version: 2,
                entries: Vec::new(),
                endpoint_denominator: None,
            },
        },
    )
    .expect("automatic exact Kawasaki preview");
    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &transactions,
        preview.transaction_token,
    )
    .expect("apply automatic exact Kawasaki pose");
    let project = super::super::lock_project(&state).unwrap();
    let original_pose = project.editor.instruction_timeline().steps[0].pose.clone();
    let archive = project.project_archive().unwrap();
    let mut tampered = project.document();
    tampered.instruction_timeline.steps[0].pose.hinge_angles[0].angle_degrees += 0.01;
    assert!(super::super::validate_document_instruction_poses(&tampered).is_err());
    drop(project);
    let reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("automatic-kawasaki.ori2"),
    )
    .expect("reopen automatic exact Kawasaki archive");
    assert_eq!(
        reopened.editor.instruction_timeline().steps[0].pose,
        original_pose
    );
    assert!(
        reopened
            .applied_pose_authority
            .capture_capability(&reopened)
            .unwrap()
            .is_some()
    );
}

#[test]
fn uncertified_rational_kawasaki_endpoints_are_atomic_no_ops() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for (numerator, denominator, complement) in [(5.0, 13.0, 12.0), (7.0, 25.0, 24.0)] {
        let (mut project, hinges) =
            uncertified_rational_kawasaki_project(numerator, denominator, complement);
        super::super::applied_pose::tests::install_flat_graph_pose_authority(&mut project, hinges);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let endpoint_read = read_even_cycle_candidates_inner_v1(
            &state,
            EvenCycleCandidatesRequestV1::for_test(instance, project_id, revision, 6),
        )
        .unwrap();
        let endpoint_outcomes = endpoint_read.kawasaki_endpoint_outcomes_for_test();
        assert_eq!(endpoint_outcomes.len(), 5);
        assert!(endpoint_outcomes.iter().all(
            |(closure_status, collision_status, authorizes_apply)| {
                *closure_status == "certified"
                    && *collision_status == "uncertified"
                    && !*authorizes_apply
            }
        ));
        let result = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: CycleScheduleRequestV1 {
                    version: 2,
                    entries: Vec::new(),
                    endpoint_denominator: Some(16),
                },
            },
        );
        assert!(matches!(
            result,
            Err(reason) if reason == CYCLE_PATH_UNCERTIFIED_MESSAGE
        ));
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_some(),
            "a rejected preview must not consume source pose authority"
        );
    }
}

#[test]
fn octagonal_eight_sector_cycle_previews_applies_and_reopens_history() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, moving) = octagonal_eight_sector_cycle_pattern();
    assert_eq!(
        pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
            .count(),
        5
    );
    assert_eq!(
        pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == ori_domain::EdgeKind::Valley)
            .count(),
        3
    );
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    set_fixed_cycle_fixture_identity_v1(&mut project, 4, 0);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 8);
    assert_eq!(snapshot.hinge_adjacency.len(), 8);
    assert!(
        automatic_opposite_pairs(&project, &snapshot)
            .iter()
            .any(|pair| pair.iter().all(|edge| moving.contains(edge)))
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
    let state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let opposite_pair = vec![moving[0], moving[2]];
    assert_eq!(
        propose_current_cycle_pose_inner(
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
        .unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
        "the all-cardinal octagonal schedule has no continuous authority",
    );
    let preview = propose_current_cycle_pose_inner(
        None,
        &state,
        &transactions,
        CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: dense_grid_schedule(&hinges, &opposite_pair, 100),
        },
    )
    .expect("octagonal straight-line cycle must certify");
    assert_eq!(preview.checked_hinge_count, 8);
    assert_eq!(preview.total_hinge_count, 8);
    let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &transactions,
        preview.transaction_token,
    )
    .expect("octagonal straight-line cycle apply");
    let mut project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    project.editor.undo(applied).unwrap();
    assert!(project.editor.instruction_timeline().steps.is_empty());
    let undone = project.editor.revision();
    project.editor.redo(undone).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    let archive = project
        .project_archive()
        .expect("serialize applied octagonal cycle");
    let mut reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("octagonal-cycle.ori2"),
    )
    .expect("reopen applied octagonal cycle");
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
    let reopened_revision = reopened.editor.revision();
    reopened.editor.undo(reopened_revision).unwrap();
    assert!(reopened.editor.instruction_timeline().steps.is_empty());
    let reopened_undone = reopened.editor.revision();
    reopened.editor.redo(reopened_undone).unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
}
