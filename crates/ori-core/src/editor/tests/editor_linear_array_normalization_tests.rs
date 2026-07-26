use super::*;

#[test]
fn linear_array_normalizes_a_proper_crossing_atomically() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let mut selected = [VertexId::new(), VertexId::new()];
    selected.sort_by_key(|id| id.canonical_bytes());
    let target_vertices = [VertexId::new(), VertexId::new()];
    let source_edge = EdgeId::new();
    let target_edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: selected[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: selected[1],
            position: Point2::new(40.0, 20.0),
        },
        Vertex {
            id: target_vertices[0],
            position: Point2::new(30.0, 40.0),
        },
        Vertex {
            id: target_vertices[1],
            position: Point2::new(30.0, 60.0),
        },
    ]);
    pattern.edges.extend([
        Edge {
            id: source_edge,
            start: selected[0],
            end: selected[1],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: target_edge,
            start: target_vertices[0],
            end: target_vertices[1],
            kind: EdgeKind::Valley,
        },
    ]);
    let original = pattern.clone();
    let mut editor = EditorState::with_paper(pattern, paper);
    let source_layer = test_layer("Array source");
    editor.project_layers.layers.push(source_layer.clone());
    editor
        .project_layers
        .edge_assignments
        .push(EdgeLayerAssignmentV1 {
            edge: source_edge,
            layer: source_layer.id,
        });
    let command = editor
        .plan_linear_array(
            0,
            selected.to_vec(),
            vec![source_edge],
            1,
            Point2::new(0.0, 30.0),
        )
        .unwrap();
    editor.execute(0, command).unwrap();
    let junction = editor
        .pattern
        .vertices
        .iter()
        .find(|vertex| vertex.position == Point2::new(30.0, 50.0))
        .unwrap();
    assert_eq!(
        editor
            .pattern
            .edges
            .iter()
            .filter(|edge| { edge.start == junction.id || edge.end == junction.id })
            .count(),
        4
    );
    assert!(
        editor
            .pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .all(|edge| editor.project_layers.layer_for_edge(edge.id) == source_layer.id)
    );
    assert_eq!(
        editor.project_layers.layer_for_edge(target_edge),
        DEFAULT_PROJECT_LAYER_ID
    );
    editor.undo(1).unwrap();
    assert_eq!(editor.pattern(), &original);
    editor.redo(2).unwrap();
    assert!(
        editor
            .pattern
            .vertices
            .iter()
            .any(|vertex| { vertex.position == Point2::new(30.0, 50.0) })
    );
}

#[test]
fn linear_array_rejects_source_selection_over_the_hard_cap() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let mut ids = (0..257).map(|_| VertexId::new()).collect::<Vec<_>>();
    ids.sort_by_key(|id| id.canonical_bytes());
    pattern
        .vertices
        .extend(ids.iter().enumerate().map(|(index, id)| Vertex {
            id: *id,
            position: Point2::new(10.0 + (index % 20) as f64, 10.0 + (index / 20) as f64),
        }));
    let editor = EditorState::with_paper(pattern, paper);
    assert_eq!(
        editor.plan_linear_array(0, ids, Vec::new(), 1, Point2::new(0.125, 0.0)),
        Err(CommandError::InvalidLinearArray),
    );
}

#[test]
fn linear_array_normalizes_a_t_junction_without_an_extra_vertex() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let mut source = [VertexId::new(), VertexId::new()];
    source.sort_by_key(|id| id.canonical_bytes());
    let target = [VertexId::new(), VertexId::new()];
    let source_edge = EdgeId::new();
    let target_edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: source[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: source[1],
            position: Point2::new(30.0, 20.0),
        },
        Vertex {
            id: target[0],
            position: Point2::new(30.0, 40.0),
        },
        Vertex {
            id: target[1],
            position: Point2::new(30.0, 70.0),
        },
    ]);
    pattern.edges.extend([
        Edge {
            id: source_edge,
            start: source[0],
            end: source[1],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: target_edge,
            start: target[0],
            end: target[1],
            kind: EdgeKind::Valley,
        },
    ]);
    let mut editor = EditorState::with_paper(pattern, paper);
    let command = editor
        .plan_linear_array(
            0,
            source.to_vec(),
            vec![source_edge],
            1,
            Point2::new(0.0, 35.0),
        )
        .unwrap();
    editor.execute(0, command).unwrap();
    let junctions = editor
        .pattern
        .vertices
        .iter()
        .filter(|vertex| vertex.position == Point2::new(30.0, 55.0))
        .collect::<Vec<_>>();
    assert_eq!(junctions.len(), 1, "the target endpoint must be reused");
    let junction = junctions[0].id;
    assert_eq!(
        editor
            .pattern
            .edges
            .iter()
            .filter(|edge| edge.start == junction || edge.end == junction)
            .count(),
        3
    );
    assert!(
        editor
            .pattern
            .edges
            .iter()
            .any(|edge| edge.id == target_edge)
    );
}

#[test]
fn linear_array_rejects_overlap_and_locked_crossing_atomically() {
    let make_editor = |target_layer: Option<LayerRecordV1>| {
        let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
        let (mut pattern, paper) = sheet.into_parts();
        let mut source = [VertexId::new(), VertexId::new()];
        source.sort_by_key(|id| id.canonical_bytes());
        let target = [VertexId::new(), VertexId::new()];
        let source_edge = EdgeId::new();
        let target_edge = EdgeId::new();
        pattern.vertices.extend([
            Vertex {
                id: source[0],
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: source[1],
                position: Point2::new(40.0, 20.0),
            },
            Vertex {
                id: target[0],
                position: Point2::new(30.0, 40.0),
            },
            Vertex {
                id: target[1],
                position: Point2::new(30.0, 60.0),
            },
        ]);
        pattern.edges.extend([
            Edge {
                id: source_edge,
                start: source[0],
                end: source[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: target_edge,
                start: target[0],
                end: target[1],
                kind: EdgeKind::Valley,
            },
        ]);
        let mut editor = EditorState::with_paper(pattern, paper);
        if let Some(layer) = target_layer {
            editor.project_layers.layers.push(layer.clone());
            editor
                .project_layers
                .edge_assignments
                .push(EdgeLayerAssignmentV1 {
                    edge: target_edge,
                    layer: layer.id,
                });
        }
        (editor, source, source_edge)
    };

    let (mut overlap, source, source_edge) = make_editor(None);
    let target_edge = overlap
        .pattern
        .edges
        .iter()
        .find(|edge| edge.id != source_edge)
        .unwrap()
        .id;
    let endpoints = overlap
        .pattern
        .edges
        .iter()
        .find(|edge| edge.id == target_edge)
        .map(|edge| (edge.start, edge.end))
        .unwrap();
    for (id, position) in [
        (endpoints.0, Point2::new(25.0, 40.0)),
        (endpoints.1, Point2::new(35.0, 40.0)),
    ] {
        overlap
            .pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == id)
            .unwrap()
            .position = position;
    }
    let snapshot = overlap.pattern.clone();
    assert!(
        overlap
            .plan_linear_array(
                0,
                source.to_vec(),
                vec![source_edge],
                1,
                Point2::new(0.0, 20.0)
            )
            .is_err()
    );
    assert_eq!(overlap.pattern, snapshot);

    let mut locked = test_layer("Locked crossing target");
    locked.locked = true;
    let (target_locked, source, source_edge) = make_editor(Some(locked.clone()));
    assert_eq!(
        target_locked.plan_linear_array(
            0,
            source.to_vec(),
            vec![source_edge],
            1,
            Point2::new(0.0, 30.0)
        ),
        Err(CommandError::LayerLocked(locked.id))
    );
    assert_eq!(target_locked.revision(), 0);
}

#[test]
fn linear_array_work_limit_is_exact_and_one_short_is_atomic() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let mut vertices = [VertexId::new(), VertexId::new()];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: vertices[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: vertices[1],
            position: Point2::new(30.0, 20.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: vertices[0],
        end: vertices[1],
        kind: EdgeKind::Mountain,
    });
    let editor = EditorState::with_paper(pattern, paper);
    // selected=3, generated=3, future edges=8: 3*(8+1)+3*3 = 36.
    with_linear_array_work_limit_for_test(36, || {
        assert!(
            editor
                .plan_linear_array(0, vertices.to_vec(), vec![edge], 1, Point2::new(0.0, 10.0))
                .is_ok()
        );
    });
    with_linear_array_work_limit_for_test(35, || {
        assert_eq!(
            editor.plan_linear_array(0, vertices.to_vec(), vec![edge], 1, Point2::new(0.0, 10.0)),
            Err(CommandError::LinearArrayWorkLimitExceeded {
                observed: 36,
                maximum: 35
            })
        );
    });
    assert_eq!(editor.revision(), 0);
    assert!(!editor.can_undo());
}
