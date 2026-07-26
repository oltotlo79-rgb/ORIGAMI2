use super::*;

#[test]
fn layer_crud_assignment_and_complete_history_are_atomic_and_fingerprint_neutral() {
    let first = VertexId::new();
    let second = VertexId::new();
    let edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: first,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: second,
                position: Point2::new(10.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: edge,
            start: first,
            end: second,
            kind: EdgeKind::Mountain,
        }],
    };
    let mut editor = EditorState::new(pattern);
    let initial_layers = editor.project_layers().clone();
    let fingerprint = editor.fold_model_fingerprint_v1();
    let crease = test_layer("Details");
    let annotation = LayerRecordV1 {
        id: LayerId::new(),
        name: "Notes".to_owned(),
        content_kind: ori_domain::LayerContentKindV1::Annotation,
        visible: true,
        locked: false,
        opacity: 1.0,
    };

    editor
        .execute(
            0,
            Command::CreateLayer {
                layer: crease.clone(),
                target_index: 1,
            },
        )
        .expect("create crease layer");
    editor
        .execute(
            1,
            Command::RenameLayer {
                layer: crease.id,
                name: "Fine details".to_owned(),
            },
        )
        .expect("rename layer");
    editor
        .execute(
            2,
            Command::CreateLayer {
                layer: annotation.clone(),
                target_index: 1,
            },
        )
        .expect("create annotation layer");
    editor
        .execute(
            3,
            Command::MoveLayer {
                layer: crease.id,
                target_index: 0,
            },
        )
        .expect("reorder layer");
    editor
        .execute(
            4,
            Command::AssignEdgeToLayer {
                edge,
                layer: crease.id,
            },
        )
        .expect("assign edge");
    assert_eq!(editor.project_layers().layer_for_edge(edge), crease.id);
    assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);

    let before_failure = editor_state_snapshot(&editor);
    assert!(matches!(
        editor.execute(
            5,
            Command::AssignEdgeToLayer {
                edge,
                layer: annotation.id,
            },
        ),
        Err(CommandError::ProjectLayerDocumentInvalid(
            ProjectLayerDocumentValidationErrorV1::AssignmentLayerWrongContentKind { .. }
        ))
    ));
    assert_eq!(editor_state_snapshot(&editor), before_failure);
    assert_eq!(
        editor.execute(
            5,
            Command::DeleteLayer {
                layer: DEFAULT_PROJECT_LAYER_ID,
            },
        ),
        Err(CommandError::DefaultLayerDeletionForbidden)
    );
    assert_eq!(editor_state_snapshot(&editor), before_failure);

    editor
        .execute(5, Command::DeleteLayer { layer: crease.id })
        .expect("delete assigned layer");
    assert_eq!(
        editor.project_layers().layer_for_edge(edge),
        DEFAULT_PROJECT_LAYER_ID
    );
    let final_layers = editor.project_layers().clone();

    for revision in 6..12 {
        editor.undo(revision).expect("undo complete layer history");
    }
    assert_eq!(editor.project_layers(), &initial_layers);
    assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);

    for revision in 12..18 {
        editor.redo(revision).expect("redo complete layer history");
    }
    assert_eq!(editor.project_layers(), &final_layers);
    assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);
}

#[test]
fn remove_and_split_edge_preserve_explicit_layer_assignments_exactly() {
    let first = VertexId::new();
    let second = VertexId::new();
    let source = Edge {
        id: EdgeId::new(),
        start: first,
        end: second,
        kind: EdgeKind::Valley,
    };
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: first,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: second,
                position: Point2::new(10.0, 0.0),
            },
        ],
        edges: vec![source.clone()],
    };
    let layer = test_layer("Fold");

    let mut remove_editor = editor_with_test_layers(
        pattern.clone(),
        Paper::default(),
        vec![layer.clone()],
        vec![(source.id, layer.id)],
    );
    let original_layers = remove_editor.project_layers().clone();
    remove_editor
        .execute(0, Command::RemoveEdge { id: source.id })
        .expect("remove assigned edge");
    assert!(remove_editor.project_layers().edge_assignments.is_empty());
    remove_editor.undo(1).expect("undo assigned removal");
    assert_eq!(remove_editor.project_layers(), &original_layers);
    remove_editor.redo(2).expect("redo assigned removal");
    assert!(remove_editor.project_layers().edge_assignments.is_empty());

    let mut split_editor = editor_with_test_layers(
        pattern,
        Paper::default(),
        vec![layer.clone()],
        vec![(source.id, layer.id)],
    );
    let new_vertex = VertexId::new();
    let new_edge = EdgeId::new();
    let original_layers = split_editor.project_layers().clone();
    split_editor
        .execute(
            0,
            Command::SplitEdge {
                edge: source.id,
                new_vertex,
                new_edge,
                fraction: 0.5,
            },
        )
        .expect("split assigned edge");
    assert_eq!(
        split_editor.project_layers().layer_for_edge(source.id),
        layer.id
    );
    assert_eq!(
        split_editor.project_layers().layer_for_edge(new_edge),
        layer.id
    );
    split_editor.undo(1).expect("undo assigned split");
    assert_eq!(split_editor.project_layers(), &original_layers);
    split_editor.redo(2).expect("redo assigned split");
    assert_eq!(
        split_editor.project_layers().layer_for_edge(new_edge),
        layer.id
    );

    let third = VertexId::new();
    let fourth = VertexId::new();
    let added_edge = EdgeId::new();
    let add_pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: first,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: second,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: third,
                position: Point2::new(0.0, 10.0),
            },
            Vertex {
                id: fourth,
                position: Point2::new(10.0, 10.0),
            },
        ],
        edges: vec![source.clone()],
    };
    let mut add_editor = editor_with_test_layers(
        add_pattern,
        Paper::default(),
        vec![layer.clone()],
        vec![(source.id, layer.id)],
    );
    let original_layers = add_editor.project_layers().clone();
    add_editor
        .execute(
            0,
            Command::AddEdge {
                id: added_edge,
                start: third,
                end: fourth,
                kind: EdgeKind::Mountain,
            },
        )
        .expect("add an independently authored edge");
    assert_eq!(
        add_editor.project_layers().layer_for_edge(added_edge),
        DEFAULT_PROJECT_LAYER_ID
    );
    assert_eq!(add_editor.project_layers(), &original_layers);
    add_editor.undo(1).expect("undo default-layer edge");
    assert_eq!(add_editor.project_layers(), &original_layers);
    add_editor.redo(2).expect("redo default-layer edge");
    assert_eq!(
        add_editor.project_layers().layer_for_edge(added_edge),
        DEFAULT_PROJECT_LAYER_ID
    );
}
