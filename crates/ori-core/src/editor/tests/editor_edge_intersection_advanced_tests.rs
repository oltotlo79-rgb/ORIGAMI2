use super::*;

#[test]
fn edge_intersection_connection_handles_extreme_finite_coordinates_exactly() {
    let (mut editor, first, second) = two_edge_editor([
        Point2::new(-f64::MAX, -1.0),
        Point2::new(f64::MAX, 1.0),
        Point2::new(-f64::MAX, 1.0),
        Point2::new(f64::MAX, -1.0),
    ]);
    let original = editor.pattern().clone();
    let junction = VertexId::new();

    editor
        .execute(
            0,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: junction,
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
        )
        .expect("exact predicates keep the finite crossing representable");

    assert_eq!(
        editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == junction)
            .expect("created extreme-coordinate junction")
            .position,
        Point2::new(0.0, 0.0)
    );
    assert!(validate_crease_pattern(editor.pattern()).is_valid());
    editor.undo(1).expect("undo extreme-coordinate crossing");
    assert_eq!(editor.pattern(), &original);
}

#[test]
fn stale_edge_intersection_connection_preserves_state_and_history() {
    let (mut editor, pattern, paper, first, second) = crossing_edges_editor();
    let error = editor
        .execute(
            9,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
        )
        .expect_err("stale intersection connection must fail");

    assert_eq!(
        error,
        CommandError::RevisionConflict {
            expected: 9,
            actual: 0,
        }
    );
    assert_eq!(editor.pattern(), &pattern);
    assert_eq!(editor.paper(), &paper);
    assert_eq!(editor.revision(), 0);
    assert!(!editor.can_undo());
    assert!(!editor.can_redo());
}

#[test]
fn create_intersection_cluster_is_canonical_atomic_and_exact_through_history() {
    let (original_pattern, original_paper, [horizontal, vertical, diagonal], unrelated) =
        three_way_create_cluster();
    assert!(!validate_crease_pattern(&original_pattern).is_valid());
    let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
    let junction = VertexId::new();
    let horizontal_new = EdgeId::new();
    let vertical_new = EdgeId::new();
    let diagonal_new = EdgeId::new();

    let result = editor
        .execute(
            0,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: junction },
                targets: vec![
                    IntersectionEdgeTarget {
                        edge: horizontal.id,
                        new_edge: Some(horizontal_new),
                    },
                    IntersectionEdgeTarget {
                        edge: diagonal.id,
                        new_edge: Some(diagonal_new),
                    },
                    IntersectionEdgeTarget {
                        edge: vertical.id,
                        new_edge: Some(vertical_new),
                    },
                ],
            },
        )
        .expect("connect three strict-interior edges");

    assert_eq!(result.revision, 1);
    assert_eq!(
        result.changed_vertices,
        vec![
            horizontal.start,
            vertical.end,
            diagonal.start,
            horizontal.end,
            vertical.start,
            diagonal.end,
            junction,
        ]
    );
    assert_eq!(
        result.changed_edges,
        vec![
            vertical.id,
            vertical_new,
            horizontal.id,
            horizontal_new,
            diagonal.id,
            diagonal_new,
        ]
    );
    assert!(!result.settings_changed);
    assert_eq!(
        editor.pattern().vertices.len(),
        original_pattern.vertices.len() + 1
    );
    assert_eq!(
        editor.pattern().vertices.last(),
        Some(&Vertex {
            id: junction,
            position: Point2::new(0.0, 0.0),
        })
    );
    assert_eq!(
        editor.pattern().edges,
        vec![
            Edge {
                end: junction,
                ..vertical.clone()
            },
            Edge {
                id: vertical_new,
                start: junction,
                end: vertical.end,
                kind: vertical.kind,
            },
            unrelated,
            Edge {
                end: junction,
                ..horizontal.clone()
            },
            Edge {
                id: horizontal_new,
                start: junction,
                end: horizontal.end,
                kind: horizontal.kind,
            },
            Edge {
                end: junction,
                ..diagonal.clone()
            },
            Edge {
                id: diagonal_new,
                start: junction,
                end: diagonal.end,
                kind: diagonal.kind,
            },
        ]
    );
    assert_eq!(editor.paper(), &original_paper);
    assert!(validate_crease_pattern(editor.pattern()).is_valid());
    let connected_pattern = editor.pattern().clone();

    let undo = editor.undo(1).expect("undo intersection cluster");
    assert_eq!(undo.revision, 2);
    assert_eq!(undo.changed_vertices, result.changed_vertices);
    assert_eq!(undo.changed_edges, result.changed_edges);
    assert!(!undo.settings_changed);
    assert_eq!(editor.pattern(), &original_pattern);
    assert_eq!(editor.paper(), &original_paper);

    let redo = editor.redo(2).expect("redo intersection cluster");
    assert_eq!(redo.revision, 3);
    assert_eq!(redo.changed_vertices, result.changed_vertices);
    assert_eq!(redo.changed_edges, result.changed_edges);
    assert!(!redo.settings_changed);
    assert_eq!(editor.pattern(), &connected_pattern);
    assert_eq!(editor.paper(), &original_paper);
}

#[test]
fn maximum_size_create_intersection_cluster_is_exact_through_history() {
    let (original_pattern, source_edges) = maximum_size_create_cluster();
    assert_eq!(source_edges.len(), MAX_INTERSECTION_CLUSTER_TARGETS);
    let original_paper = Paper::default();
    let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
    let junction = VertexId::new();
    let targets = source_edges
        .iter()
        .map(|edge| IntersectionEdgeTarget {
            edge: edge.id,
            new_edge: Some(EdgeId::new()),
        })
        .collect();

    let result = editor
        .execute(
            0,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: junction },
                targets,
            },
        )
        .expect("the inclusive 64-edge cluster limit must connect");

    assert_eq!(result.revision, 1);
    assert_eq!(
        result.changed_vertices.len(),
        MAX_INTERSECTION_CLUSTER_TARGETS * 2 + 1
    );
    assert_eq!(
        result.changed_edges.len(),
        MAX_INTERSECTION_CLUSTER_TARGETS * 2
    );
    assert!(!result.settings_changed);
    assert_eq!(
        editor.pattern().vertices.len(),
        original_pattern.vertices.len() + 1
    );
    assert_eq!(
        editor.pattern().edges.len(),
        original_pattern.edges.len() + MAX_INTERSECTION_CLUSTER_TARGETS
    );
    assert_eq!(
        editor.pattern().vertices.last(),
        Some(&Vertex {
            id: junction,
            position: Point2::new(0.0, 0.0),
        })
    );
    for source in &source_edges {
        let split_original = editor
            .pattern()
            .edges
            .iter()
            .find(|edge| edge.id == source.id)
            .expect("each original edge remains at the maximum cluster");
        assert_eq!(split_original.start, source.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, source.kind);
        let generated = editor
            .pattern()
            .edges
            .iter()
            .find(|edge| {
                !source_edges.iter().any(|source| source.id == edge.id)
                    && edge.start == junction
                    && edge.end == source.end
            })
            .expect("each maximum-cluster source gets one generated half");
        assert_eq!(generated.kind, source.kind);
    }
    assert!(validate_crease_pattern(editor.pattern()).is_valid());
    assert_eq!(editor.paper(), &original_paper);
    let connected_pattern = editor.pattern().clone();

    let undo = editor.undo(1).expect("undo maximum intersection cluster");
    assert_eq!(undo.revision, 2);
    assert_eq!(undo.changed_vertices, result.changed_vertices);
    assert_eq!(undo.changed_edges, result.changed_edges);
    assert_eq!(editor.pattern(), &original_pattern);
    assert_eq!(editor.paper(), &original_paper);

    let redo = editor.redo(2).expect("redo maximum intersection cluster");
    assert_eq!(redo.revision, 3);
    assert_eq!(redo.changed_vertices, result.changed_vertices);
    assert_eq!(redo.changed_edges, result.changed_edges);
    assert_eq!(editor.pattern(), &connected_pattern);
    assert_eq!(editor.paper(), &original_paper);
}
