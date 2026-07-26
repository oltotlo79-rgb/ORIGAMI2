use super::*;

#[test]
fn edge_split_rejects_boundary_missing_and_ambiguous_targets_atomically() {
    let (_, pattern, paper) = rectangular_editor();
    let boundary_edge = pattern.edges[0].clone();
    let crease_edge = pattern.edges[4].clone();

    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: boundary_edge.id,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::BoundaryEdgeRequiresSheetOperation(boundary_edge.id),
    );

    let missing_edge = EdgeId::new();
    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: missing_edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::EdgeNotFound(missing_edge),
    );

    let mut duplicate_pattern = pattern.clone();
    duplicate_pattern.edges.push(crease_edge.clone());
    let mut editor = EditorState::with_paper(duplicate_pattern, paper);
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::EdgeSplitTargetEdgeIdAmbiguous {
            edge: crease_edge.id,
        },
    );
}

#[test]
fn edge_split_rejects_non_unique_generated_ids_globally() {
    let (_, pattern, paper) = rectangular_editor();
    let crease_edge = pattern.edges[4].clone();
    let existing_vertex = pattern.vertices[0].id;
    let existing_edge = pattern.edges[1].id;

    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: existing_vertex,
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::VertexAlreadyExists(existing_vertex),
    );

    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: VertexId::new(),
            new_edge: existing_edge,
            fraction: 0.5,
        },
        CommandError::EdgeAlreadyExists(existing_edge),
    );

    let boundary_only_id = VertexId::new();
    let mut malformed_paper = paper.clone();
    malformed_paper.boundary_vertices.push(boundary_only_id);
    let mut editor = EditorState::with_paper(pattern.clone(), malformed_paper);
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: boundary_only_id,
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::VertexAlreadyExists(boundary_only_id),
    );

    let endpoint_only_id = VertexId::new();
    let mut malformed_pattern = pattern;
    malformed_pattern.edges.push(Edge {
        id: EdgeId::new(),
        start: endpoint_only_id,
        end: crease_edge.start,
        kind: EdgeKind::Auxiliary,
    });
    let mut editor = EditorState::with_paper(malformed_pattern, paper);
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: endpoint_only_id,
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::VertexAlreadyExists(endpoint_only_id),
    );
}

#[test]
fn edge_split_rejects_invalid_fractions_positions_and_revisions_atomically() {
    let (_, pattern, paper) = rectangular_editor();
    let crease_edge = pattern.edges[4].clone();
    for (fraction, expected) in [
        (f64::NAN, CommandError::EdgeSplitFractionNotFinite),
        (f64::INFINITY, CommandError::EdgeSplitFractionNotFinite),
        (0.0, CommandError::EdgeSplitFractionOutOfRange),
        (-0.5, CommandError::EdgeSplitFractionOutOfRange),
        (1.0, CommandError::EdgeSplitFractionOutOfRange),
    ] {
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction,
            },
            expected,
        );
    }

    let occupied_by = VertexId::new();
    let start = pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == crease_edge.start)
        .expect("crease start")
        .position;
    let end = pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == crease_edge.end)
        .expect("crease end")
        .position;
    let mut occupied_pattern = pattern.clone();
    occupied_pattern.vertices.push(Vertex {
        id: occupied_by,
        position: Point2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0),
    });
    let mut editor = EditorState::with_paper(occupied_pattern, paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::EdgeSplitPositionOccupied {
            vertex: occupied_by,
        },
    );

    let mut non_finite_pattern = pattern.clone();
    non_finite_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == crease_edge.end)
        .expect("crease end")
        .position
        .y = f64::NEG_INFINITY;
    let mut editor = EditorState::with_paper(non_finite_pattern, paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::EdgeSplitEndpointPositionNotFinite {
            edge: crease_edge.id,
            vertex: crease_edge.end,
        },
    );

    for endpoint in [crease_edge.start, crease_edge.end] {
        let mut ambiguous_pattern = pattern.clone();
        ambiguous_pattern.vertices.push(Vertex {
            id: endpoint,
            position: Point2::new(-123.0, 456.0),
        });
        let mut editor = EditorState::with_paper(ambiguous_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeSplitEndpointVertexRecordAmbiguous {
                edge: crease_edge.id,
                vertex: endpoint,
            },
        );
    }

    let mut missing_endpoint_pattern = pattern.clone();
    missing_endpoint_pattern
        .vertices
        .retain(|vertex| vertex.id != crease_edge.start);
    let mut editor = EditorState::with_paper(missing_endpoint_pattern, paper.clone());
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: crease_edge.id,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        CommandError::VertexNotFound(crease_edge.start),
    );

    let close_ids = [VertexId::new(), VertexId::new()];
    let close_edge = Edge {
        id: EdgeId::new(),
        start: close_ids[0],
        end: close_ids[1],
        kind: EdgeKind::Valley,
    };
    let close_pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: close_ids[0],
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: close_ids[1],
                position: Point2::new(2.0, 0.0),
            },
        ],
        edges: vec![close_edge.clone()],
    };
    let mut editor = EditorState::new(close_pattern);
    assert_split_rejected(
        &mut editor,
        Command::SplitEdge {
            edge: close_edge.id,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: f64::MIN_POSITIVE,
        },
        CommandError::EdgeSplitPositionNotDistinct,
    );

    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    let original_pattern = editor.pattern().clone();
    let original_paper = editor.paper().clone();
    let error = editor
        .execute(
            7,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
        )
        .expect_err("stale split must fail");
    assert_eq!(
        error,
        CommandError::RevisionConflict {
            expected: 7,
            actual: 0,
        }
    );
    assert_eq!(editor.pattern(), &original_pattern);
    assert_eq!(editor.paper(), &original_paper);
    assert_eq!(editor.revision(), 0);
    assert!(!editor.can_undo());
}

#[test]
fn edge_split_uses_stable_interpolation_for_extreme_finite_endpoints() {
    let ids = [VertexId::new(), VertexId::new()];
    let edge = Edge {
        id: EdgeId::new(),
        start: ids[0],
        end: ids[1],
        kind: EdgeKind::Mountain,
    };
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: ids[0],
                position: Point2::new(-f64::MAX, 0.0),
            },
            Vertex {
                id: ids[1],
                position: Point2::new(f64::MAX, 2.0),
            },
        ],
        edges: vec![edge.clone()],
    };
    let new_vertex = VertexId::new();
    let mut editor = EditorState::new(pattern);

    editor
        .execute(
            0,
            Command::SplitEdge {
                edge: edge.id,
                new_vertex,
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
        )
        .expect("split extreme finite edge");

    assert_eq!(
        editor.pattern().vertices.last(),
        Some(&Vertex {
            id: new_vertex,
            position: Point2::new(0.0, 1.0),
        })
    );
}
