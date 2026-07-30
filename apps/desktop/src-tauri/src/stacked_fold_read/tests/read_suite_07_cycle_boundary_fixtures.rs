#[test]
fn parametric_oblique_dense_static_authority_is_available_on_desktop() {
    for (angle_index, angle_degrees) in [30.0, 45.0, 120.0].into_iter().enumerate() {
        for (thickness_index, thickness_mm) in [0.1, 1.0, 3.0].into_iter().enumerate() {
            let (pattern, mut paper, _, _) =
                super::dense_grid_cycle_test_support::angled_dense_cycle_pattern(
                    3,
                    3,
                    angle_degrees,
                );
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            set_fixed_cycle_fixture_identity_v1(
                &mut project,
                1,
                (angle_index * 3 + thickness_index) as u16,
            );
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges,
                fixed,
            );
            let state = AppState::new(project);
            assert!(
                crate::applied_pose::certify_current_static_collision(
                    &state,
                    ori_collision::StaticCollisionLimits::default(),
                )
                .expect("parametric oblique static diagnosis")
                .is_some()
            );
        }
    }
}

fn balloon_six_sector_cycle_pattern() -> (
    ori_domain::CreasePattern,
    ori_domain::Paper,
    Vec<ori_domain::EdgeId>,
) {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x41,
    ]);
    let center = Vertex {
        id: VertexId::derive_v5(namespace, b"balloon-center"),
        position: Point2::new(0.0, 0.0),
    };
    let boundary = [
        (100.0, 0.0),
        (50.0, 100.0),
        (-50.0, 100.0),
        (-100.0, 0.0),
        (-50.0, -100.0),
        (50.0, -100.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (x, y))| Vertex {
        id: VertexId::derive_v5(namespace, format!("balloon-{index}").as_bytes()),
        position: Point2::new(x, y),
    })
    .collect::<Vec<_>>();
    let mut edges = (0..6)
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, format!("boundary-{index}").as_bytes()),
            start: boundary[index].id,
            end: boundary[(index + 1) % 6].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = (0..6)
        .map(|index| EdgeId::derive_v5(namespace, format!("spoke-{index}").as_bytes()))
        .collect::<Vec<_>>();
    edges.extend((0..6).map(|index| Edge {
        id: hinges[index],
        start: center.id,
        end: boundary[index].id,
        kind: if matches!(index, 0 | 1 | 3 | 4) {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let mut vertices = vec![center];
    vertices.extend(boundary.iter().cloned());
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.0,
            ..Paper::default()
        },
        vec![hinges[0], hinges[3]],
    )
}

fn octagonal_eight_sector_cycle_pattern() -> (
    ori_domain::CreasePattern,
    ori_domain::Paper,
    Vec<ori_domain::EdgeId>,
) {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x42,
    ]);
    let center = Vertex {
        id: VertexId::derive_v5(namespace, b"octagonal-center"),
        position: Point2::new(0.0, 0.0),
    };
    let boundary = [
        (100.0, 0.0),
        (70.0, 70.0),
        (0.0, 100.0),
        (-70.0, 70.0),
        (-100.0, 0.0),
        (-70.0, -70.0),
        (0.0, -100.0),
        (70.0, -70.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (x, y))| Vertex {
        id: VertexId::derive_v5(namespace, format!("octagonal-{index}").as_bytes()),
        position: Point2::new(x, y),
    })
    .collect::<Vec<_>>();
    let mut edges = (0..8)
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, format!("boundary-{index}").as_bytes()),
            start: boundary[index].id,
            end: boundary[(index + 1) % 8].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = (0..8)
        .map(|index| EdgeId::derive_v5(namespace, format!("spoke-{index}").as_bytes()))
        .collect::<Vec<_>>();
    edges.extend((0..8).map(|index| Edge {
        id: hinges[index],
        start: center.id,
        end: boundary[index].id,
        kind: if matches!(index, 0 | 1 | 2 | 4 | 6) {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let mut vertices = vec![center];
    vertices.extend(boundary.iter().cloned());
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.0,
            ..Paper::default()
        },
        vec![hinges[0], hinges[2], hinges[4], hinges[6]],
    )
}

fn sixteen_sector_cycle_pattern(
    moving_second: usize,
) -> (
    ori_domain::CreasePattern,
    ori_domain::Paper,
    Vec<ori_domain::EdgeId>,
) {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x43,
    ]);
    let center = Vertex {
        id: VertexId::derive_v5(namespace, b"sixteen-center"),
        position: Point2::new(0.0, 0.0),
    };
    let half = [
        (100.0, 0.0),
        (92.0, 38.0),
        (71.0, 71.0),
        (38.0, 92.0),
        (0.0, 100.0),
        (-38.0, 92.0),
        (-71.0, 71.0),
        (-92.0, 38.0),
    ];
    let coordinates = half
        .into_iter()
        .chain(half.into_iter().map(|(x, y)| (-x, -y)))
        .collect::<Vec<_>>();
    let boundary = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, format!("sixteen-{index}").as_bytes()),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let mut edges = (0..16)
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, format!("boundary-{index}").as_bytes()),
            start: boundary[index].id,
            end: boundary[(index + 1) % 16].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = (0..16)
        .map(|index| EdgeId::derive_v5(namespace, format!("spoke-{index}").as_bytes()))
        .collect::<Vec<_>>();
    edges.extend((0..16).map(|index| Edge {
        id: hinges[index],
        start: center.id,
        end: boundary[index].id,
        kind: if index <= 8 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let mut vertices = vec![center];
    vertices.extend(boundary.iter().cloned());
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.0,
            ..Paper::default()
        },
        vec![hinges[0], hinges[moving_second]],
    )
}

#[test]
fn balloon_six_sector_straight_line_cycle_previews_applies_and_round_trips_history() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, moving) = balloon_six_sector_cycle_pattern();
    assert_eq!(
        pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
            .count(),
        4
    );
    assert_eq!(
        pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == ori_domain::EdgeKind::Valley)
            .count(),
        2
    );
    assert!(
        pattern
            .edges
            .iter()
            .filter(|edge| moving.contains(&edge.id))
            .all(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
    );
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    set_fixed_cycle_fixture_identity_v1(&mut project, 2, 0);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 6);
    assert_eq!(snapshot.hinge_adjacency.len(), 6);
    let discovered = automatic_opposite_pairs(&project, &snapshot);
    assert!(
        discovered
            .iter()
            .any(|pair| pair.iter().all(|edge| moving.contains(edge)))
    );
    let mut reordered_pattern = project.editor.pattern().clone();
    reordered_pattern.edges.reverse();
    let mut reordered = super::super::ProjectState::new_with_paper(
        reordered_pattern,
        project.editor.paper().clone(),
    );
    set_fixed_cycle_fixture_identity_v1(&mut reordered, 2, 0);
    let reordered_analysis = reordered
        .editor
        .topology_analysis_input(reordered.project_id)
        .analyze();
    let reordered_snapshot = reordered_analysis.simulation_snapshot().unwrap();
    assert_eq!(
        automatic_opposite_pairs(&reordered, &reordered_snapshot),
        discovered
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
    .expect("balloon straight-line cycle must certify");
    let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &transactions,
        preview.transaction_token,
    )
    .expect("balloon straight-line cycle apply");
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
    .expect("the rebound current pose must authorize a second preview");
    let second_applied =
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            second_preview.transaction_token,
        )
        .expect("second balloon operation applies atomically");
    let mut project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .is_some()
    );
    project.editor.undo(second_applied).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    assert!(
        project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .is_none()
    );
    let first_undone = project.editor.revision();
    project.editor.undo(first_undone).unwrap();
    assert!(project.editor.instruction_timeline().steps.is_empty());
    let undone = project.editor.revision();
    project.editor.redo(undone).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    let first_redone = project.editor.revision();
    project.editor.redo(first_redone).unwrap();
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    let mut nonclosing_document = project.document();
    let tampered = nonclosing_document.instruction_timeline.steps[0]
        .pose
        .hinge_angles
        .iter_mut()
        .find(|hinge| hinge.edge == moving[0])
        .expect("moving balloon hinge is persisted");
    tampered.angle_degrees += 0.01;
    assert!(
        super::super::validate_document_instruction_poses(&nonclosing_document)
            .expect_err("a nonclosing cyclic persisted pose must fail closed")
            .contains("is not cycle-closing")
    );
    let archive = project
        .project_archive()
        .expect("serialize applied balloon cycle with history");
    super::super::restore_archive_editor(&archive).expect("restore applied balloon editor history");
    let mut reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from("balloon-cycle.ori2"),
    )
    .expect("reopen applied balloon cycle");
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    assert!(
        reopened
            .applied_pose_authority
            .capture_capability(&reopened)
            .unwrap()
            .is_some()
    );
    let reopened_revision = reopened.editor.revision();
    reopened.editor.undo(reopened_revision).unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
    let reopened_undone = reopened.editor.revision();
    reopened.editor.undo(reopened_undone).unwrap();
    assert!(reopened.editor.instruction_timeline().steps.is_empty());
    let reopened_first_redo = reopened.editor.revision();
    reopened.editor.redo(reopened_first_redo).unwrap();
    let reopened_second_redo = reopened.editor.revision();
    reopened.editor.redo(reopened_second_redo).unwrap();
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
}

#[test]
fn concave_boundary_strict_dyadic_read_fails_closed_without_mutation_authority() {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x71,
    ]);
    let coordinates = [
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 1.0),
        (1.0, 1.0),
        (1.0, 3.0),
        (0.0, 3.0),
    ];
    let vertices = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let hinge = EdgeId::derive_v5(namespace, b"concave-hinge");
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x20, index as u8]),
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: hinge,
        start: vertices[0].id,
        end: vertices[3].id,
        kind: EdgeKind::Mountain,
    });
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        thickness_mm: 0.1,
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
    .expect("concave read returns a fail-closed observation")
    .into_test_view();
    assert_eq!(observed.reason, "unsupported_geometry");
    assert!(!observed.mutation_candidate_ready);
    assert!(!observed.authorizes_project_mutation);
    let project = super::super::lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn cut_boundary_strict_dyadic_read_fails_closed_without_mutation_authority() {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x72,
    ]);
    let coordinates = [
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 0.0),
        (3.0, 2.0),
        (2.0, 2.0),
        (1.0, 2.0),
        (0.0, 2.0),
    ];
    let vertices = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let hinge = EdgeId::derive_v5(namespace, b"cut-fixture-hinge");
    let cut = EdgeId::derive_v5(namespace, b"cut-fixture-cut");
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x20, index as u8]),
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: hinge,
            start: vertices[1].id,
            end: vertices[6].id,
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: cut,
            start: vertices[2].id,
            end: vertices[5].id,
            kind: EdgeKind::Cut,
        },
    ]);
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        thickness_mm: 0.1,
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
    .expect("cut read returns a fail-closed observation")
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
