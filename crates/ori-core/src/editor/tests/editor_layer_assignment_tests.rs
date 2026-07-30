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

#[test]
fn explicit_unlocked_crease_layer_authors_atomically_when_default_is_locked() {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    let layer = test_layer("Writable");
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(10.0, 0.0),
            },
        ],
        edges: Vec::new(),
    };
    let mut editor =
        editor_with_test_layers(pattern, Paper::default(), vec![layer.clone()], Vec::new());
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

    assert!(matches!(
        editor.plan_add_edge_with_intersections(
            1,
            edge,
            start,
            end,
            EdgeKind::Mountain,
        ),
        Err(CommandError::LayerLocked(id)) if id == DEFAULT_PROJECT_LAYER_ID
    ));
    let command = editor
        .plan_add_edge_with_intersections_for_layer(
            1,
            edge,
            start,
            end,
            EdgeKind::Mountain,
            layer.id,
        )
        .expect("plan on writable crease layer");
    editor.execute(1, command).expect("apply one atomic edit");
    assert_eq!(editor.revision(), 2);
    assert_eq!(editor.project_layers().layer_for_edge(edge), layer.id);

    editor.undo(2).expect("undo normalized authoring");
    assert!(
        editor
            .pattern()
            .edges
            .iter()
            .all(|record| record.id != edge)
    );
    editor.redo(3).expect("redo normalized authoring");
    assert_eq!(editor.project_layers().layer_for_edge(edge), layer.id);
}

#[test]
fn explicit_crease_authoring_rejects_locked_missing_wrong_kind_and_locked_crossing() {
    let horizontal_start = VertexId::new();
    let horizontal_end = VertexId::new();
    let vertical_start = VertexId::new();
    let vertical_end = VertexId::new();
    let horizontal = EdgeId::new();
    let layer = test_layer("Writable");
    let annotation = LayerRecordV1 {
        id: LayerId::new(),
        name: "Notes".to_owned(),
        content_kind: ori_domain::LayerContentKindV1::Annotation,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: horizontal_start,
                position: Point2::new(-10.0, 0.0),
            },
            Vertex {
                id: horizontal_end,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: vertical_start,
                position: Point2::new(0.0, -10.0),
            },
            Vertex {
                id: vertical_end,
                position: Point2::new(0.0, 10.0),
            },
        ],
        edges: vec![Edge {
            id: horizontal,
            start: horizontal_start,
            end: horizontal_end,
            kind: EdgeKind::Valley,
        }],
    };
    let mut editor = editor_with_test_layers(
        pattern,
        Paper::default(),
        vec![layer.clone(), annotation.clone()],
        Vec::new(),
    );
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

    assert!(matches!(
        editor.plan_add_edge_with_intersections_for_layer(
            1,
            EdgeId::new(),
            vertical_start,
            vertical_end,
            EdgeKind::Mountain,
            annotation.id,
        ),
        Err(CommandError::ProjectLayerDocumentInvalid(
            ProjectLayerDocumentValidationErrorV1::AssignmentLayerWrongContentKind { .. }
        ))
    ));
    assert!(matches!(
        editor.plan_add_edge_with_intersections_for_layer(
            1,
            EdgeId::new(),
            vertical_start,
            vertical_end,
            EdgeKind::Mountain,
            LayerId::new(),
        ),
        Err(CommandError::LayerNotFound(_))
    ));
    assert_eq!(editor_state_snapshot(&editor), before);
    let before_pattern = editor.pattern().clone();
    let before_layers = editor.project_layers().clone();

    editor
        .execute(
            1,
            Command::UpdateLayerPresentation {
                layer: layer.id,
                visible: true,
                locked: true,
                opacity: 1.0,
            },
        )
        .expect("lock authored layer");
    assert!(matches!(
        editor.plan_add_edge_with_intersections_for_layer(
            2,
            EdgeId::new(),
            vertical_start,
            vertical_end,
            EdgeKind::Mountain,
            layer.id,
        ),
        Err(CommandError::LayerLocked(id)) if id == layer.id
    ));
    editor.undo(2).expect("unlock authored layer");
    assert_eq!(editor.pattern(), &before_pattern);
    assert_eq!(editor.project_layers(), &before_layers);

    assert!(matches!(
        editor.plan_add_edge_with_intersections_for_layer(
            3,
            EdgeId::new(),
            vertical_start,
            vertical_end,
            EdgeKind::Mountain,
            layer.id,
        ),
        Err(CommandError::LayerLocked(id)) if id == DEFAULT_PROJECT_LAYER_ID
    ));
    assert_eq!(editor.pattern(), &before_pattern);
    assert_eq!(editor.project_layers(), &before_layers);
}

#[test]
fn connected_vertex_and_ray_can_target_an_unlocked_non_default_crease_layer() {
    let sheet = crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid sheet");
    let connected_start = vertex_at(10.0, 25.0);
    let ray_target = vertex_at(20.0, 25.0);
    let layer = test_layer("Writable");
    let mut pattern = sheet.pattern().clone();
    pattern
        .vertices
        .extend([connected_start.clone(), ray_target.clone()]);
    let mut connected = editor_with_test_layers(
        pattern.clone(),
        sheet.paper().clone(),
        vec![layer.clone()],
        Vec::new(),
    );
    connected
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
    let connected_edge = EdgeId::new();
    let command = connected
        .plan_add_connected_vertex_for_layer(
            1,
            VertexId::new(),
            Point2::new(10.0, 30.0),
            connected_edge,
            connected_start.id,
            EdgeKind::Auxiliary,
            layer.id,
        )
        .expect("plan connected vertex on writable layer");
    connected
        .execute(1, command)
        .expect("apply connected vertex");
    assert_eq!(
        connected.project_layers().layer_for_edge(connected_edge),
        layer.id,
    );

    let mut ray = editor_with_test_layers(
        pattern,
        sheet.paper().clone(),
        vec![layer.clone()],
        Vec::new(),
    );
    ray.execute(
        0,
        Command::UpdateLayerPresentation {
            layer: DEFAULT_PROJECT_LAYER_ID,
            visible: true,
            locked: true,
            opacity: 1.0,
        },
    )
    .expect("lock default layer");
    let command = ray
        .plan_add_ray_to_first_target_for_layer(
            1,
            connected_start.id,
            0,
            EdgeKind::Auxiliary,
            layer.id,
        )
        .expect("plan ray on writable layer");
    ray.execute(1, command).expect("apply ray");
    let authored = ray
        .pattern()
        .edges
        .iter()
        .find(|edge| {
            edge.start == connected_start.id
                && edge.end == ray_target.id
                && edge.kind == EdgeKind::Auxiliary
        })
        .expect("ray-authored edge");
    assert_eq!(ray.project_layers().layer_for_edge(authored.id), layer.id,);
}

fn normalized_layer_guard_editor(
    source_layer: LayerRecordV1,
    additional_layers: Vec<LayerRecordV1>,
) -> (EditorState, Edge) {
    let start = vertex_at(0.0, 0.0);
    let end = vertex_at(10.0, 0.0);
    let edge = Edge {
        id: EdgeId::new(),
        start: start.id,
        end: end.id,
        kind: EdgeKind::Mountain,
    };
    let pattern = CreasePattern {
        vertices: vec![start, end],
        edges: vec![edge.clone()],
    };
    let source_layer_id = source_layer.id;
    let mut layers = vec![source_layer];
    layers.extend(additional_layers);
    (
        editor_with_test_layers(
            pattern,
            Paper::default(),
            layers,
            vec![(edge.id, source_layer_id)],
        ),
        edge,
    )
}

#[test]
fn normalized_document_rejects_locked_edge_removal_atomically() {
    let mut locked = test_layer("Locked source");
    locked.locked = true;
    let locked_id = locked.id;
    let (mut editor, _) = normalized_layer_guard_editor(locked, Vec::new());
    let before = editor_state_snapshot(&editor);
    let mut pattern = editor.pattern().clone();
    pattern.edges.clear();
    let mut project_layers = editor.project_layers().clone();
    project_layers.edge_assignments.clear();

    assert_eq!(
        editor.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern,
                project_layers,
            },
        ),
        Err(CommandError::LayerLocked(locked_id)),
    );
    assert_eq!(editor_state_snapshot(&editor), before);
}

#[test]
fn normalized_document_rejects_locked_edge_record_or_endpoint_change_atomically() {
    let mut locked = test_layer("Locked source");
    locked.locked = true;
    let locked_id = locked.id;
    let (mut editor, _) = normalized_layer_guard_editor(locked, Vec::new());
    let before = editor_state_snapshot(&editor);
    let mut changed_edge = editor.pattern().clone();
    changed_edge.edges[0].kind = EdgeKind::Valley;

    assert_eq!(
        editor.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern: changed_edge,
                project_layers: editor.project_layers().clone(),
            },
        ),
        Err(CommandError::LayerLocked(locked_id)),
    );
    assert_eq!(editor_state_snapshot(&editor), before);

    let mut moved_endpoint = editor.pattern().clone();
    moved_endpoint.vertices[0].position = Point2::new(1.0, 0.0);
    assert_eq!(
        editor.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern: moved_endpoint,
                project_layers: editor.project_layers().clone(),
            },
        ),
        Err(CommandError::LayerLocked(locked_id)),
    );
    assert_eq!(editor_state_snapshot(&editor), before);
}

#[test]
fn normalized_document_reassignment_requires_both_old_and_new_layers_unlocked() {
    let mut locked_source = test_layer("Locked source");
    locked_source.locked = true;
    let locked_source_id = locked_source.id;
    let unlocked_target = test_layer("Unlocked target");
    let unlocked_target_id = unlocked_target.id;
    let (mut old_locked, _) = normalized_layer_guard_editor(locked_source, vec![unlocked_target]);
    let before = editor_state_snapshot(&old_locked);
    let mut project_layers = old_locked.project_layers().clone();
    project_layers.edge_assignments[0].layer = unlocked_target_id;

    assert_eq!(
        old_locked.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern: old_locked.pattern().clone(),
                project_layers,
            },
        ),
        Err(CommandError::LayerLocked(locked_source_id)),
    );
    assert_eq!(editor_state_snapshot(&old_locked), before);

    let unlocked_source = test_layer("Unlocked source");
    let mut locked_target = test_layer("Locked target");
    locked_target.locked = true;
    let locked_target_id = locked_target.id;
    let (mut new_locked, edge) =
        normalized_layer_guard_editor(unlocked_source, vec![locked_target]);
    let before = editor_state_snapshot(&new_locked);
    let mut project_layers = new_locked.project_layers().clone();
    let assignment = project_layers
        .edge_assignments
        .iter_mut()
        .find(|assignment| assignment.edge == edge.id)
        .expect("source assignment");
    assignment.layer = locked_target_id;

    assert_eq!(
        new_locked.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern: new_locked.pattern().clone(),
                project_layers,
            },
        ),
        Err(CommandError::LayerLocked(locked_target_id)),
    );
    assert_eq!(editor_state_snapshot(&new_locked), before);
}

#[test]
fn normalized_document_new_edge_rejects_locked_target() {
    fn editor_without_edges(layer: LayerRecordV1) -> (EditorState, Edge) {
        let start = vertex_at(0.0, 0.0);
        let end = vertex_at(10.0, 0.0);
        let edge = Edge {
            id: EdgeId::new(),
            start: start.id,
            end: end.id,
            kind: EdgeKind::Mountain,
        };
        (
            editor_with_test_layers(
                CreasePattern {
                    vertices: vec![start, end],
                    edges: Vec::new(),
                },
                Paper::default(),
                vec![layer],
                Vec::new(),
            ),
            edge,
        )
    }

    let mut target = test_layer("Target");
    target.locked = true;
    let target_id = target.id;
    let (mut target_locked, edge) = editor_without_edges(target);
    let before = editor_state_snapshot(&target_locked);
    let mut pattern = target_locked.pattern().clone();
    pattern.edges.push(edge.clone());
    let mut project_layers = target_locked.project_layers().clone();
    project_layers.edge_assignments = vec![EdgeLayerAssignmentV1 {
        edge: edge.id,
        layer: target_id,
    }];

    assert_eq!(
        target_locked.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern,
                project_layers,
            },
        ),
        Err(CommandError::LayerLocked(target_id)),
    );
    assert_eq!(editor_state_snapshot(&target_locked), before);
}

#[test]
fn normalized_document_rejects_layer_metadata_forgery_atomically() {
    let source = test_layer("Source");
    let source_id = source.id;
    let other = test_layer("Other");
    let (mut editor, edge) = normalized_layer_guard_editor(source, vec![other]);
    let before = editor_state_snapshot(&editor);

    let mut candidates = Vec::new();
    let mut schema = editor.project_layers().clone();
    schema.schema_version = schema.schema_version.saturating_add(1);
    candidates.push((
        schema,
        CommandError::ProjectLayerDocumentInvalid(
            ProjectLayerDocumentValidationErrorV1::UnsupportedSchemaVersion {
                actual: 2,
                expected: 1,
            },
        ),
    ));

    let mut renamed = editor.project_layers().clone();
    renamed.layers[1].name.push_str(" forged");
    candidates.push((renamed, CommandError::InvalidStackedFoldDocument));

    let mut reordered = editor.project_layers().clone();
    reordered.layers.swap(1, 2);
    candidates.push((reordered, CommandError::InvalidStackedFoldDocument));

    let mut lock_toggled = editor.project_layers().clone();
    lock_toggled.layers[1].locked = !lock_toggled.layers[1].locked;
    candidates.push((lock_toggled, CommandError::InvalidStackedFoldDocument));

    let mut content_kind = editor.project_layers().clone();
    content_kind.layers[1].content_kind = ori_domain::LayerContentKindV1::Annotation;
    candidates.push((
        content_kind,
        CommandError::ProjectLayerDocumentInvalid(
            ProjectLayerDocumentValidationErrorV1::AssignmentLayerWrongContentKind {
                edge: edge.id,
                layer: source_id,
            },
        ),
    ));

    for (project_layers, expected) in candidates {
        assert_eq!(
            editor.execute(
                0,
                Command::ApplyNormalizedEdgeDocument {
                    pattern: editor.pattern().clone(),
                    project_layers,
                },
            ),
            Err(expected),
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }
}

#[test]
fn normalized_document_rejects_locked_default_isolated_vertex_changes() {
    let existing = vertex_at(0.0, 0.0);
    let mut editor = EditorState::new(CreasePattern {
        vertices: vec![existing.clone()],
        edges: Vec::new(),
    });
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

    let mut moved = editor.pattern().clone();
    moved.vertices[0].position = Point2::new(1.0, 0.0);
    let mut removed = editor.pattern().clone();
    removed.vertices.clear();
    let mut added = editor.pattern().clone();
    added.vertices.push(vertex_at(2.0, 0.0));

    for pattern in [moved, removed, added] {
        assert_eq!(
            editor.execute(
                1,
                Command::ApplyNormalizedEdgeDocument {
                    pattern,
                    project_layers: editor.project_layers().clone(),
                },
            ),
            Err(CommandError::LayerLocked(DEFAULT_PROJECT_LAYER_ID)),
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }
}

#[test]
fn sealed_document_variants_route_through_the_layer_diff_guard_atomically() {
    let sheet = crate::create_rectangular_sheet(80.0, 60.0, false).expect("valid sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let ray_source = vertex_at(10.0, 10.0);
    let ray_target = vertex_at(20.0, 10.0);
    let linear_source = vertex_at(10.0, 20.0);
    let radial_center = vertex_at(50.0, 30.0);
    let radial_source = vertex_at(60.0, 30.0);
    pattern.vertices.extend([
        ray_source.clone(),
        ray_target,
        linear_source.clone(),
        radial_center.clone(),
        radial_source.clone(),
    ]);
    let mut editor = EditorState::with_paper(pattern, paper.clone());
    let mut ray = editor
        .plan_add_ray_to_first_target(0, ray_source.id, 0, EdgeKind::Mountain)
        .expect("valid ray plan");
    let mut linear = editor
        .plan_linear_array(
            0,
            vec![linear_source.id],
            Vec::new(),
            1,
            Point2::new(10.0, 0.0),
        )
        .expect("valid vertex-only linear plan");
    let mut radial = editor
        .plan_radial_array(
            0,
            radial_center.id,
            vec![radial_source.id],
            Vec::new(),
            1,
            90_000_000,
        )
        .expect("valid vertex-only radial plan");
    editor
        .project_layers
        .layers
        .iter_mut()
        .find(|layer| layer.id == DEFAULT_PROJECT_LAYER_ID)
        .expect("default layer")
        .locked = true;
    let project_layers = editor.project_layers().clone();
    let Command::ApplyRayToTargetDocument(ray_plan) = &mut ray else {
        panic!("ray plan")
    };
    ray_plan.before_project_layers = project_layers.clone();
    ray_plan.project_layers.layers = project_layers.layers.clone();
    let Command::ApplyLinearArrayDocument(linear_plan) = &mut linear else {
        panic!("linear plan")
    };
    linear_plan.before_project_layers = project_layers.clone();
    linear_plan.project_layers.layers = project_layers.layers.clone();
    let Command::ApplyRadialArrayDocument(radial_plan) = &mut radial else {
        panic!("radial plan")
    };
    radial_plan.before_project_layers = project_layers.clone();
    radial_plan.project_layers.layers = project_layers.layers.clone();

    let mut target = editor.pattern().clone();
    let new_edge = EdgeId::new();
    target.edges.push(Edge {
        id: new_edge,
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let timeline = InstructionTimeline {
        steps: vec![instruction_step(
            InstructionStepId::new(),
            "Locked replacement",
            crate::fold_model_fingerprint::fold_model_fingerprint_v1(&target, &paper),
        )],
    };
    let beginner_design_profile = Box::new(editor.beginner_design_profile().clone());
    let commands = vec![
        ray,
        linear,
        radial,
        Command::ApplyStackedFoldDocument(StackedFoldDocumentCommandV1::new(
            target.clone(),
            paper.clone(),
            timeline.clone(),
            project_layers.clone(),
            beginner_design_profile.clone(),
        )),
        Command::ApplyBeginnerGeneratedDocument {
            pattern: target,
            paper,
            instruction_timeline: timeline,
            project_layers,
            beginner_design_profile,
        },
    ];

    for command in commands {
        let mut candidate = editor.clone();
        let before = editor_state_snapshot(&candidate);
        assert_eq!(
            candidate.execute(0, command),
            Err(CommandError::LayerLocked(DEFAULT_PROJECT_LAYER_ID)),
        );
        assert_eq!(editor_state_snapshot(&candidate), before);
    }
}

#[test]
fn replacement_rejects_target_locked_default_isolated_addition_atomically() {
    let sheet = crate::create_rectangular_sheet(80.0, 60.0, false).expect("valid sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut editor = EditorState::with_paper(pattern, paper.clone());
    let mut target = editor.pattern().clone();
    target.vertices.push(vertex_at(20.0, 20.0));
    let mut project_layers = editor.project_layers().clone();
    project_layers
        .layers
        .iter_mut()
        .find(|layer| layer.id == DEFAULT_PROJECT_LAYER_ID)
        .expect("target default layer")
        .locked = true;
    let timeline = InstructionTimeline {
        steps: vec![instruction_step(
            InstructionStepId::new(),
            "Target-locked replacement",
            crate::fold_model_fingerprint::fold_model_fingerprint_v1(&target, &paper),
        )],
    };
    let command = Command::ApplyBeginnerGeneratedDocument {
        pattern: target,
        paper,
        instruction_timeline: timeline,
        project_layers,
        beginner_design_profile: Box::new(editor.beginner_design_profile().clone()),
    };
    let before = editor_state_snapshot(&editor);

    assert_eq!(
        editor.execute(0, command),
        Err(CommandError::LayerLocked(DEFAULT_PROJECT_LAYER_ID)),
    );
    assert_eq!(editor_state_snapshot(&editor), before);
}

#[test]
fn mirror_move_and_duplicate_recheck_the_default_layer_lock_atomically() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).expect("valid sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let source = vertex_at(20.0, 20.0);
    pattern.vertices.push(source.clone());
    let mut editor = EditorState::with_paper(pattern, paper);
    editor
        .project_layers
        .layers
        .iter_mut()
        .find(|layer| layer.id == DEFAULT_PROJECT_LAYER_ID)
        .expect("default layer")
        .locked = true;
    let axis = MirrorAxisV1 {
        start: Point2::new(50.0, 0.0),
        end: Point2::new(50.0, 100.0),
    };
    let commands = [
        Command::MirrorSelection {
            vertices: vec![source.id],
            edges: Vec::new(),
            axis,
            mode: MirrorSelectionModeV1::Move,
            new_vertices: Vec::new(),
            new_edges: Vec::new(),
        },
        Command::MirrorSelection {
            vertices: vec![source.id],
            edges: Vec::new(),
            axis,
            mode: MirrorSelectionModeV1::Duplicate,
            new_vertices: vec![VertexId::new()],
            new_edges: Vec::new(),
        },
    ];

    for command in commands {
        let mut candidate = editor.clone();
        let before = editor_state_snapshot(&candidate);
        assert_eq!(
            candidate.execute(0, command),
            Err(CommandError::LayerLocked(DEFAULT_PROJECT_LAYER_ID)),
        );
        assert_eq!(editor_state_snapshot(&candidate), before);
    }
}
