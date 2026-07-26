use super::*;

#[test]
fn edge_intersection_connection_is_atomic_ordered_and_exact_through_history() {
    let (_, mut original_pattern, original_paper, first, second) = crossing_edges_editor();
    let original_second_index = original_pattern
        .edges
        .iter()
        .position(|edge| edge.id == second.id)
        .expect("second crossing edge");
    let unrelated_edge = Edge {
        id: EdgeId::new(),
        start: first.start,
        end: second.start,
        kind: EdgeKind::Auxiliary,
    };
    original_pattern
        .edges
        .insert(original_second_index, unrelated_edge.clone());
    let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
    let first_index = original_pattern
        .edges
        .iter()
        .position(|edge| edge.id == first.id)
        .expect("first crossing edge");
    let second_index = original_pattern
        .edges
        .iter()
        .position(|edge| edge.id == second.id)
        .expect("second crossing edge");
    assert_eq!(second_index, first_index + 2);
    assert_eq!(original_pattern.edges[first_index + 1], unrelated_edge);
    assert!(!validate_crease_pattern(&original_pattern).is_valid());
    let new_vertex = VertexId::new();
    let first_new_edge = EdgeId::new();
    let second_new_edge = EdgeId::new();

    // Pass the targets in reverse vector order to verify that output order
    // remains tied to the document, while each generated ID remains tied
    // to its requested target edge.
    let result = editor
        .execute(
            0,
            Command::ConnectEdgeIntersection {
                first_edge: second.id,
                second_edge: first.id,
                new_vertex,
                first_new_edge,
                second_new_edge,
            },
        )
        .expect("connect proper edge intersection");

    assert_eq!(result.revision, 1);
    assert_eq!(
        result.changed_vertices,
        vec![new_vertex, first.start, first.end, second.start, second.end]
    );
    assert_eq!(
        result.changed_edges,
        vec![first.id, second_new_edge, second.id, first_new_edge]
    );
    assert!(!result.settings_changed);
    assert_eq!(editor.paper(), &original_paper);
    assert_eq!(
        editor.pattern().vertices.last(),
        Some(&Vertex {
            id: new_vertex,
            position: Point2::new(50.0, 50.0),
        })
    );
    assert_eq!(
        editor.pattern().edges[first_index],
        Edge {
            end: new_vertex,
            ..first.clone()
        }
    );
    assert_eq!(
        editor.pattern().edges[first_index + 1],
        Edge {
            id: second_new_edge,
            start: new_vertex,
            end: first.end,
            kind: first.kind,
        }
    );
    assert_eq!(
        editor.pattern().edges[second_index + 1],
        Edge {
            end: new_vertex,
            ..second.clone()
        }
    );
    assert_eq!(
        editor.pattern().edges[second_index + 2],
        Edge {
            id: first_new_edge,
            start: new_vertex,
            end: second.end,
            kind: second.kind,
        }
    );
    assert_eq!(editor.pattern().edges[first_index + 2], unrelated_edge);
    assert!(validate_crease_pattern(editor.pattern()).is_valid());
    assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    let connected_pattern = editor.pattern().clone();

    let undo = editor.undo(1).expect("undo intersection connection");
    assert_eq!(undo.revision, 2);
    assert_eq!(undo.changed_vertices, result.changed_vertices);
    assert_eq!(undo.changed_edges, result.changed_edges);
    assert!(!undo.settings_changed);
    assert_eq!(editor.pattern(), &original_pattern);
    assert_eq!(editor.paper(), &original_paper);

    let redo = editor.redo(2).expect("redo intersection connection");
    assert_eq!(redo.revision, 3);
    assert_eq!(redo.changed_vertices, result.changed_vertices);
    assert_eq!(redo.changed_edges, result.changed_edges);
    assert!(!redo.settings_changed);
    assert_eq!(editor.pattern(), &connected_pattern);
    assert_eq!(editor.paper(), &original_paper);
}

#[test]
fn edge_intersection_connection_handles_asymmetric_proper_fractions() {
    let (mut editor, horizontal, vertical) = two_edge_editor([
        Point2::new(0.0, 3.0),
        Point2::new(10.0, 3.0),
        Point2::new(2.0, 0.0),
        Point2::new(2.0, 10.0),
    ]);
    let new_vertex = VertexId::new();
    let horizontal_new = EdgeId::new();
    let vertical_new = EdgeId::new();

    editor
        .execute(
            0,
            Command::ConnectEdgeIntersection {
                first_edge: horizontal.id,
                second_edge: vertical.id,
                new_vertex,
                first_new_edge: horizontal_new,
                second_new_edge: vertical_new,
            },
        )
        .expect("connect asymmetric proper intersection");

    assert_eq!(
        editor.pattern().vertices.last(),
        Some(&Vertex {
            id: new_vertex,
            position: Point2::new(2.0, 3.0),
        })
    );
    assert_eq!(
        editor.pattern().edges,
        vec![
            Edge {
                end: new_vertex,
                ..horizontal.clone()
            },
            Edge {
                id: horizontal_new,
                start: new_vertex,
                end: horizontal.end,
                kind: EdgeKind::Mountain,
            },
            Edge {
                end: new_vertex,
                ..vertical.clone()
            },
            Edge {
                id: vertical_new,
                start: new_vertex,
                end: vertical.end,
                kind: EdgeKind::Valley,
            },
        ]
    );
    assert!(validate_crease_pattern(editor.pattern()).is_valid());
}

#[test]
fn edge_intersection_connection_preserves_reverse_cut_and_auxiliary_edges() {
    let (_, mut pattern, mut paper, first, second) = crossing_edges_editor();
    paper.cutting_allowed = true;
    let first_index = pattern
        .edges
        .iter()
        .position(|edge| edge.id == first.id)
        .expect("first edge");
    let second_index = pattern
        .edges
        .iter()
        .position(|edge| edge.id == second.id)
        .expect("second edge");
    pattern.edges[first_index] = Edge {
        start: first.end,
        end: first.start,
        kind: EdgeKind::Cut,
        ..first
    };
    pattern.edges[second_index] = Edge {
        start: second.end,
        end: second.start,
        kind: EdgeKind::Auxiliary,
        ..second
    };
    let original_first = pattern.edges[first_index].clone();
    let original_second = pattern.edges[second_index].clone();
    let new_vertex = VertexId::new();
    let first_new = EdgeId::new();
    let second_new = EdgeId::new();
    let mut editor = EditorState::with_paper(pattern, paper);

    editor
        .execute(
            0,
            Command::ConnectEdgeIntersection {
                first_edge: original_first.id,
                second_edge: original_second.id,
                new_vertex,
                first_new_edge: first_new,
                second_new_edge: second_new,
            },
        )
        .expect("connect reversed cut and auxiliary intersection");

    assert_eq!(
        editor.pattern().edges[first_index],
        Edge {
            end: new_vertex,
            ..original_first.clone()
        }
    );
    assert_eq!(
        editor.pattern().edges[first_index + 1],
        Edge {
            id: first_new,
            start: new_vertex,
            end: original_first.end,
            kind: EdgeKind::Cut,
        }
    );
    assert_eq!(
        editor.pattern().edges[second_index + 1],
        Edge {
            end: new_vertex,
            ..original_second.clone()
        }
    );
    assert_eq!(
        editor.pattern().edges[second_index + 2],
        Edge {
            id: second_new,
            start: new_vertex,
            end: original_second.end,
            kind: EdgeKind::Auxiliary,
        }
    );
}
