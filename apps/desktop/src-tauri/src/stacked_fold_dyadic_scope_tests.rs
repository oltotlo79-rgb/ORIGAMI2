#[test]
fn open_cut_seam_strict_dyadic_preflight_is_unsupported_no_op() {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x76,
    ]);
    let positions = [
        (0.0, 0.0),
        (8.0, 0.0),
        (8.0, 8.0),
        (0.0, 8.0),
        (2.0, 4.0),
        (6.0, 4.0),
    ];
    let vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let mut edges = (0..4)
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x50, index as u8]),
            start: vertices[index].id,
            end: vertices[(index + 1) % 4].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let target_edge = edges[0].id;
    edges.push(Edge {
        id: EdgeId::derive_v5(namespace, b"open-cut-seam"),
        start: vertices[4].id,
        end: vertices[5].id,
        kind: EdgeKind::Cut,
    });
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: pattern.vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect(),
        cutting_allowed: true,
        ..Paper::default()
    };
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}

fn assert_out_of_scope_boundary_is_unsupported_no_op(
    pattern: ori_domain::CreasePattern,
    paper: ori_domain::Paper,
    target_edge: ori_domain::EdgeId,
) {
    let project = super::super::ProjectState::new_with_paper(pattern, paper);
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
    .expect("out-of-scope boundary returns a fail-closed observation")
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
    assert!(
        project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .is_none()
    );
}

fn boundary_preflight_fixture(
    positions: [(f64, f64); 3],
    omit_last_vertex: bool,
) -> (
    ori_domain::CreasePattern,
    ori_domain::Paper,
    ori_domain::EdgeId,
) {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x74,
    ]);
    let vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let target_edge = EdgeId::derive_v5(namespace, b"boundary-preflight-target");
    let edges = vec![Edge {
        id: target_edge,
        start: vertices[0].id,
        end: vertices[1].id,
        kind: EdgeKind::Mountain,
    }];
    let boundary_vertices = if omit_last_vertex {
        vec![
            vertices[0].id,
            vertices[1].id,
            VertexId::derive_v5(namespace, b"missing-boundary-vertex"),
        ]
    } else {
        vertices.iter().map(|vertex| vertex.id).collect()
    };
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            ..Paper::default()
        },
        target_edge,
    )
}

#[test]
fn nonfinite_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, target_edge) =
        boundary_preflight_fixture([(0.0, 0.0), (1.0, 0.0), (f64::NAN, 1.0)], false);
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}

#[test]
fn degenerate_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, target_edge) =
        boundary_preflight_fixture([(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], false);
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}

#[test]
fn missing_boundary_vertex_strict_dyadic_preflight_is_unsupported_no_op() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, target_edge) =
        boundary_preflight_fixture([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)], true);
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}

fn malformed_production_boundary_fixture(
    positions: [(f64, f64); 4],
    boundary_order: [usize; 4],
) -> (
    ori_domain::CreasePattern,
    ori_domain::Paper,
    ori_domain::EdgeId,
) {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
    let namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
        0x75,
    ]);
    let vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary_vertices = boundary_order.map(|index| vertices[index].id);
    let mut edges = (0..boundary_vertices.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x40, index as u8]),
            start: boundary_vertices[index],
            end: boundary_vertices[(index + 1) % boundary_vertices.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let target_edge = EdgeId::derive_v5(namespace, b"malformed-boundary-target");
    edges.push(Edge {
        id: target_edge,
        start: vertices[0].id,
        end: vertices[2].id,
        kind: EdgeKind::Mountain,
    });
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary_vertices.to_vec(),
            ..Paper::default()
        },
        target_edge,
    )
}

#[test]
fn duplicate_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, target_edge) = malformed_production_boundary_fixture(
        [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
        [0, 1, 2, 1],
    );
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}

#[test]
fn self_intersecting_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, target_edge) = malformed_production_boundary_fixture(
        [(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)],
        [0, 1, 2, 3],
    );
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}

#[test]
fn zero_length_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, target_edge) = malformed_production_boundary_fixture(
        [(0.0, 0.0), (0.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
        [0, 1, 2, 3],
    );
    assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
}
