use super::*;

#[test]
fn radial_array_rejects_missing_boundary_and_extreme_centers_without_state_change() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(50.0, 50.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(60.0, 50.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Auxiliary,
    });
    let editor = EditorState::with_paper(pattern, paper);
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    for bad_center in [VertexId::new(), editor.paper.boundary_vertices[0]] {
        assert_eq!(
            editor.plan_radial_array(0, bad_center, vertices.clone(), vec![edge], 1, 90_000_000),
            Err(CommandError::InvalidRadialArray)
        );
    }
    let mut extreme = editor.clone();
    extreme
        .pattern
        .vertices
        .iter_mut()
        .find(|v| v.id == center)
        .unwrap()
        .position = Point2::new(-f64::MAX, 50.0);
    extreme
        .pattern
        .vertices
        .iter_mut()
        .find(|v| v.id == outer)
        .unwrap()
        .position = Point2::new(f64::MAX, 50.0);
    assert_eq!(
        extreme.plan_radial_array(0, center, vertices, vec![edge], 1, 90_000_000),
        Err(CommandError::InvalidRadialArray)
    );
    assert_eq!(editor.revision(), 0);
    assert!(!editor.can_undo());
}

#[test]
fn radial_array_rejects_noncanonical_and_oversized_sources() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    pattern.vertices.push(Vertex {
        id: center,
        position: Point2::new(50.0, 50.0),
    });
    let mut ids = (0..256)
        .map(|index| {
            let id = VertexId::new();
            pattern.vertices.push(Vertex {
                id,
                position: Point2::new(10.0 + (index % 20) as f64, 10.0 + (index / 20) as f64),
            });
            id
        })
        .collect::<Vec<_>>();
    ids.push(center);
    ids.sort_by_key(|id| id.canonical_bytes());
    let editor = EditorState::with_paper(pattern, paper);
    assert_eq!(
        editor.plan_radial_array(0, center, ids.clone(), Vec::new(), 1, 90_000_000),
        Err(CommandError::InvalidRadialArray)
    );
    let mut two = ids[..2].to_vec();
    two.reverse();
    assert_eq!(
        editor.plan_radial_array(0, center, two, Vec::new(), 1, 90_000_000),
        Err(CommandError::InvalidRadialArray)
    );
}

#[test]
fn radial_array_normalizes_signed_zero_and_handles_tiny_representable_offsets() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    for vertex in &mut pattern.vertices {
        vertex.position.x -= 50.0;
        vertex.position.y -= 50.0;
    }
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(1.0, -0.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Auxiliary,
    });
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let mut editor = EditorState::with_paper(pattern, paper);
    let command = editor
        .plan_radial_array(0, center, vertices, vec![edge], 1, 90_000_000)
        .unwrap();
    editor.execute(0, command).unwrap();
    let rotated = editor
        .pattern
        .vertices
        .iter()
        .find(|v| v.position.y == 1.0 && v.id != outer)
        .unwrap();
    assert_eq!(rotated.position.x.to_bits(), 0);
    let sheet = crate::create_rectangular_sheet(10.0, 10.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(1.0, 1.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(1.0 + f64::EPSILON, 1.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Auxiliary,
    });
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let tiny = EditorState::with_paper(pattern, paper);
    assert!(
        tiny.plan_radial_array(0, center, vertices, vec![edge], 1, 90_000_000)
            .is_ok()
    );
}

#[test]
fn radial_array_rejects_paper_escape_and_uses_one_external_revision() {
    let sheet = crate::create_rectangular_sheet(100.0, 10.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(50.0, 5.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(60.0, 5.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Mountain,
    });
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let outside = EditorState::with_paper(pattern, paper);
    let before = editor_state_snapshot(&outside);
    assert_eq!(
        outside.plan_radial_array(0, center, vertices, vec![edge], 1, 90_000_000),
        Err(CommandError::InvalidRadialArray)
    );
    assert_eq!(editor_state_snapshot(&outside), before);
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(50.0, 50.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(60.0, 50.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Mountain,
    });
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let mut editor = EditorState::with_paper(pattern, paper);
    editor.revision = MAX_REVISION - 1;
    let command = editor
        .plan_radial_array(
            MAX_REVISION - 1,
            center,
            vertices,
            vec![edge],
            1,
            90_000_000,
        )
        .unwrap();
    let stale = command.clone();
    assert_eq!(
        editor.execute(MAX_REVISION - 1, command).unwrap().revision,
        MAX_REVISION
    );
    assert_eq!(
        editor.execute(MAX_REVISION, stale),
        Err(CommandError::RevisionExhausted {
            revision: MAX_REVISION
        })
    );
}
