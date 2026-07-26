use super::*;

#[test]
fn radial_array_quarter_turn_reuses_center_and_is_atomic() {
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
    let original = pattern.clone();
    let mut editor = EditorState::with_paper(pattern, paper);
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    with_radial_array_work_limit_for_test(18, || {
        assert!(
            editor
                .plan_radial_array(0, center, vertices.clone(), vec![edge], 1, 90_000_000)
                .is_ok()
        );
    });
    with_radial_array_work_limit_for_test(17, || {
        assert_eq!(
            editor.plan_radial_array(0, center, vertices.clone(), vec![edge], 1, 90_000_000),
            Err(CommandError::RadialArrayWorkLimitExceeded {
                observed: 18,
                maximum: 17
            })
        );
    });
    let command = editor
        .plan_radial_array(0, center, vertices.clone(), vec![edge], 3, 90_000_000)
        .unwrap();
    let assert_rejected = |tampered: Command| {
        let mut candidate = editor.clone();
        let before = editor_state_snapshot(&candidate);
        assert_eq!(
            candidate.execute(0, tampered),
            Err(CommandError::InvalidRadialArray)
        );
        assert_eq!(editor_state_snapshot(&candidate), before);
    };
    let mut empty = command.clone();
    if let Command::ApplyRadialArrayDocument(plan) = &mut empty {
        plan.source_vertices.clear();
        plan.source_edges.clear();
        plan.vertex_lineage.clear();
        plan.edge_seeds.clear();
    }
    assert_rejected(empty);
    let mut lineage_swap = command.clone();
    if let Command::ApplyRadialArrayDocument(plan) = &mut lineage_swap {
        plan.vertex_lineage.swap(0, 1);
    }
    assert_rejected(lineage_swap);
    let mut seed_swap = command.clone();
    if let Command::ApplyRadialArrayDocument(plan) = &mut seed_swap {
        plan.edge_seeds.swap(0, 1);
    }
    assert_rejected(seed_swap);
    let mut tampered = Vec::new();
    for mutate in 0..20 {
        let mut candidate = command.clone();
        let Command::ApplyRadialArrayDocument(plan) = &mut candidate else {
            unreachable!()
        };
        match mutate {
            0 => plan.before_fingerprint.push('0'),
            1 => plan.center = VertexId::new(),
            2 => plan.additional_copies = 4,
            3 => plan.angle_microdegrees = 45_000_000,
            4 => plan.new_vertices.push(VertexId::new()),
            5 => plan.new_edges.push(EdgeId::new()),
            6 => plan.removed_edges.push(edge),
            7 => plan.changed_edges.push(edge),
            8 => plan.vertex_lineage[0].0 = 0,
            9 => plan.vertex_lineage[0].1 = VertexId::new(),
            10 => plan.edge_seeds[0].0 = 0,
            11 => plan.edge_seeds[0].1 = EdgeId::new(),
            12 => plan.vertex_lineage[0].2 = VertexId::new(),
            13 => plan.edge_seeds[0].2 = EdgeId::new(),
            14 => plan.source_vertices[0] = VertexId::new(),
            15 => plan.source_edges[0] = EdgeId::new(),
            16 => plan.pattern.vertices.push(Vertex {
                id: VertexId::new(),
                position: Point2::new(70.0, 70.0),
            }),
            17 => plan.pattern.edges.push(Edge {
                id: EdgeId::new(),
                start: center,
                end: outer,
                kind: EdgeKind::Valley,
            }),
            18 => plan.project_layers.layers[0].name.push_str(" tampered"),
            _ => plan.before_project_layers.layers[0]
                .name
                .push_str(" tampered"),
        }
        tampered.push(candidate);
    }
    for candidate in tampered {
        assert_rejected(candidate);
    }
    editor.execute(0, command).unwrap();
    assert_eq!(
        editor
            .pattern
            .vertices
            .iter()
            .filter(|v| v.position == Point2::new(50.0, 50.0))
            .count(),
        1
    );
    for point in [
        Point2::new(50.0, 60.0),
        Point2::new(40.0, 50.0),
        Point2::new(50.0, 40.0),
    ] {
        assert!(editor.pattern.vertices.iter().any(|v| v.position == point));
    }
    editor.undo(1).unwrap();
    assert_eq!(editor.pattern(), &original);
    editor.redo(2).unwrap();
    let applied = editor.pattern.clone();
    let history = editor
        .export_history_v1(ori_domain::ProjectId::new())
        .unwrap();
    assert!(
        serde_json::to_string(&history)
            .unwrap()
            .contains("\"kind\":\"apply_radial_array_document\"")
    );
    let mut reopened = EditorState::with_document_parts_layers_and_history_v1(
        applied.clone(),
        editor.paper.clone(),
        editor.instruction_timeline.clone(),
        editor.geometric_constraints.clone(),
        editor.project_layers.clone(),
        history,
    )
    .unwrap();
    reopened.undo(0).unwrap();
    assert_eq!(reopened.pattern(), &original);
    reopened.redo(1).unwrap();
    assert_eq!(reopened.pattern(), &applied);
    assert_eq!(
        editor.plan_radial_array(3, center, vertices.clone(), vec![edge], 2, 180_000_000),
        Err(CommandError::InvalidRadialArray)
    );
    assert!(
        editor
            .plan_radial_array(3, center, vertices, vec![edge], 1, 180_000_000)
            .is_err(),
        "live copies collide and must fail closed"
    );
}

#[test]
fn radial_array_accepts_only_complete_nonrepeating_orthogonal_orbits() {
    for (angle, copies, expected) in [
        (90_000_000, 3, Point2::new(50.0, 40.0)),
        (180_000_000, 1, Point2::new(40.0, 50.0)),
        (270_000_000, 3, Point2::new(50.0, 60.0)),
    ] {
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
            kind: EdgeKind::Valley,
        });
        let mut vertices = vec![center, outer];
        vertices.sort_by_key(|id| id.canonical_bytes());
        let mut editor = EditorState::with_paper(pattern, paper);
        let command = editor
            .plan_radial_array(0, center, vertices.clone(), vec![edge], copies, angle)
            .unwrap();
        editor.execute(0, command).unwrap();
        assert!(
            editor
                .pattern
                .vertices
                .iter()
                .any(|v| v.position == expected)
        );
        assert_eq!(
            editor.plan_radial_array(
                1,
                center,
                vertices,
                vec![edge],
                if angle == 180_000_000 { 2 } else { 4 },
                angle
            ),
            Err(CommandError::InvalidRadialArray)
        );
    }
}
