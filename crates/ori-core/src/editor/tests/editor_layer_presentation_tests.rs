use super::*;

#[test]
fn layer_presentation_is_atomic_undoable_and_can_unlock_itself() {
    let mut editor = EditorState::new(CreasePattern::empty());
    let original = editor.project_layers().clone();

    let result = editor
        .execute(
            0,
            Command::UpdateLayerPresentation {
                layer: DEFAULT_PROJECT_LAYER_ID,
                visible: false,
                locked: true,
                opacity: 0.25,
            },
        )
        .expect("lock and hide default layer");
    assert!(result.settings_changed);
    assert_eq!(
        editor.project_layers().layers[0],
        LayerRecordV1 {
            visible: false,
            locked: true,
            opacity: 0.25,
            ..original.layers[0].clone()
        }
    );

    editor.undo(1).expect("undo presentation");
    assert_eq!(editor.project_layers(), &original);
    editor.redo(2).expect("redo presentation");
    assert!(editor.project_layers().layers[0].locked);

    editor
        .execute(
            3,
            Command::UpdateLayerPresentation {
                layer: DEFAULT_PROJECT_LAYER_ID,
                visible: true,
                locked: false,
                opacity: 1.0,
            },
        )
        .expect("a locked layer must be able to unlock itself");
    assert_eq!(editor.project_layers(), &original);
}

#[test]
fn invalid_layer_opacity_is_rejected_without_partial_state_or_history() {
    for opacity in [f64::NAN, f64::INFINITY, -0.0, -0.1, 1.1] {
        let mut editor = EditorState::new(CreasePattern::empty());
        let before = editor_state_snapshot(&editor);
        assert!(matches!(
            editor.execute(
                0,
                Command::UpdateLayerPresentation {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                    visible: false,
                    locked: true,
                    opacity,
                },
            ),
            Err(CommandError::ProjectLayerDocumentInvalid(_))
        ));
        assert_eq!(editor_state_snapshot(&editor), before);
    }
}

#[test]
fn locked_layer_routes_every_edge_and_shared_vertex_mutation_to_one_guard() {
    let shared = VertexId::new();
    let locked_end = VertexId::new();
    let unlocked_end = VertexId::new();
    let locked_edge = EdgeId::new();
    let unlocked_edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: shared,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: locked_end,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: unlocked_end,
                position: Point2::new(0.0, 10.0),
            },
        ],
        edges: vec![
            Edge {
                id: locked_edge,
                start: shared,
                end: locked_end,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: unlocked_edge,
                start: shared,
                end: unlocked_end,
                kind: EdgeKind::Valley,
            },
        ],
    };
    let mut locked_layer = test_layer("Locked fold");
    locked_layer.locked = true;
    let locked_layer_id = locked_layer.id;
    let mut editor = editor_with_test_layers(
        pattern,
        Paper::default(),
        vec![locked_layer],
        vec![(locked_edge, locked_layer_id)],
    );
    let before = editor_state_snapshot(&editor);

    let commands = vec![
        Command::MoveVertex {
            id: shared,
            position: Point2::new(1.0, 1.0),
        },
        Command::RemoveVertex { id: shared },
        Command::RemoveEdge { id: locked_edge },
        Command::SplitEdge {
            edge: locked_edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        Command::SplitBoundaryEdge {
            edge: locked_edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction: 0.5,
        },
        Command::ConnectEdgeIntersection {
            first_edge: unlocked_edge,
            second_edge: locked_edge,
            new_vertex: VertexId::new(),
            first_new_edge: EdgeId::new(),
            second_new_edge: EdgeId::new(),
        },
        Command::ConnectTJunction {
            first_edge: unlocked_edge,
            second_edge: locked_edge,
            new_edge: EdgeId::new(),
        },
        Command::ConnectIntersectionCluster {
            junction: JunctionVertexIntent::Create {
                id: VertexId::new(),
            },
            targets: vec![
                IntersectionEdgeTarget {
                    edge: unlocked_edge,
                    new_edge: Some(EdgeId::new()),
                },
                IntersectionEdgeTarget {
                    edge: locked_edge,
                    new_edge: Some(EdgeId::new()),
                },
            ],
        },
        Command::RemoveBoundaryVertex { vertex: shared },
        Command::ResizeRectangularPaper {
            width_mm: 200.0,
            height_mm: 200.0,
        },
        Command::AssignEdgeToLayer {
            edge: locked_edge,
            layer: DEFAULT_PROJECT_LAYER_ID,
        },
        Command::AssignEdgeToLayer {
            edge: unlocked_edge,
            layer: locked_layer_id,
        },
        Command::DeleteLayer {
            layer: locked_layer_id,
        },
    ];

    for command in commands {
        assert_eq!(
            editor.execute(0, command),
            Err(CommandError::LayerLocked(locked_layer_id))
        );
        assert_eq!(
            editor_state_snapshot(&editor),
            before,
            "a rejected locked-layer edit must be fully atomic",
        );
    }
}

#[test]
fn locked_default_layer_blocks_new_geometry_but_not_unlocking() {
    let mut editor = EditorState::new(CreasePattern::empty());
    editor
        .execute(
            0,
            Command::UpdateLayerPresentation {
                layer: DEFAULT_PROJECT_LAYER_ID,
                visible: true,
                locked: true,
                opacity: 1.0,
            },
        )
        .expect("lock default layer");
    let before = editor_state_snapshot(&editor);

    for command in [
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 1.0),
        },
        Command::AddEdge {
            id: EdgeId::new(),
            start: VertexId::new(),
            end: VertexId::new(),
            kind: EdgeKind::Mountain,
        },
    ] {
        assert_eq!(
            editor.execute(1, command),
            Err(CommandError::LayerLocked(DEFAULT_PROJECT_LAYER_ID))
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    editor
        .execute(
            1,
            Command::UpdateLayerPresentation {
                layer: DEFAULT_PROJECT_LAYER_ID,
                visible: true,
                locked: false,
                opacity: 1.0,
            },
        )
        .expect("unlock remains available");
}
