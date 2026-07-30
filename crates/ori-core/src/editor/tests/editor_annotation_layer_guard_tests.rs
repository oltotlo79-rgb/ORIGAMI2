use super::*;

fn auxiliary_layer(
    name: &str,
    content_kind: ori_domain::LayerContentKindV1,
    locked: bool,
) -> LayerRecordV1 {
    LayerRecordV1 {
        id: LayerId::new(),
        name: name.to_owned(),
        content_kind,
        visible: true,
        locked,
        opacity: 1.0,
    }
}

fn test_annotation(
    id: AnnotationId,
    anchor: ori_domain::AnnotationAnchorV1,
    layer: LayerId,
) -> AnnotationRecordV1 {
    AnnotationRecordV1 {
        id,
        text: "Anchor".to_owned(),
        anchor,
        style: ori_domain::AnnotationStyleV1 {
            color: RgbaColor::opaque(32, 64, 96),
            font_size_mm: 4.0,
            bold: false,
            italic: false,
        },
        layer,
    }
}

fn test_underlay(id: UnderlayId, layer: LayerId) -> UnderlayRecordV1 {
    UnderlayRecordV1 {
        id,
        asset: AssetId::new(),
        transform: ori_domain::UnderlayTransformV1 {
            position: Point2::new(10.0, 20.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
        },
        opacity: 0.5,
        layer,
    }
}

#[test]
fn annotation_and_underlay_updates_require_unlocked_source_and_target_layers() {
    let annotation_source = auxiliary_layer(
        "Locked annotation source",
        ori_domain::LayerContentKindV1::Annotation,
        true,
    );
    let annotation_target = auxiliary_layer(
        "Annotation target",
        ori_domain::LayerContentKindV1::Annotation,
        false,
    );
    let underlay_source = auxiliary_layer(
        "Locked underlay source",
        ori_domain::LayerContentKindV1::Underlay,
        true,
    );
    let underlay_target = auxiliary_layer(
        "Underlay target",
        ori_domain::LayerContentKindV1::Underlay,
        false,
    );
    let annotation = test_annotation(
        AnnotationId::new(),
        ori_domain::AnnotationAnchorV1::Absolute {
            position: Point2::new(1.0, 2.0),
        },
        annotation_source.id,
    );
    let underlay = test_underlay(UnderlayId::new(), underlay_source.id);
    let mut editor = editor_with_test_layers(
        CreasePattern::empty(),
        Paper::default(),
        vec![
            annotation_source.clone(),
            annotation_target.clone(),
            underlay_source.clone(),
            underlay_target.clone(),
        ],
        Vec::new(),
    );
    editor.annotations.annotations.push(annotation.clone());
    editor.underlays.underlays.push(underlay.clone());

    let mut annotation_update = annotation.clone();
    annotation_update.text = "Moved annotation".to_owned();
    annotation_update.layer = annotation_target.id;
    let mut underlay_update = underlay.clone();
    underlay_update.opacity = 0.75;
    underlay_update.layer = underlay_target.id;
    let before_state = editor_state_snapshot(&editor);
    let before_annotations = editor.annotations().clone();
    let before_underlays = editor.underlays().clone();

    assert_eq!(
        editor.execute(
            0,
            Command::UpdateAnnotation {
                record: annotation_update.clone(),
            },
        ),
        Err(CommandError::LayerLocked(annotation_source.id)),
    );
    assert_eq!(editor_state_snapshot(&editor), before_state);
    assert_eq!(editor.annotations(), &before_annotations);
    assert_eq!(editor.underlays(), &before_underlays);

    assert_eq!(
        editor.execute(
            0,
            Command::UpdateUnderlay {
                record: underlay_update.clone(),
            },
        ),
        Err(CommandError::LayerLocked(underlay_source.id)),
    );
    assert_eq!(editor_state_snapshot(&editor), before_state);
    assert_eq!(editor.annotations(), &before_annotations);
    assert_eq!(editor.underlays(), &before_underlays);

    let missing_annotation = AnnotationRecordV1 {
        id: AnnotationId::new(),
        ..annotation_update.clone()
    };
    assert_eq!(
        editor.execute(
            0,
            Command::UpdateAnnotation {
                record: missing_annotation.clone(),
            },
        ),
        Err(CommandError::AnnotationNotFound(missing_annotation.id)),
    );
    let missing_underlay = UnderlayRecordV1 {
        id: UnderlayId::new(),
        ..underlay_update.clone()
    };
    assert_eq!(
        editor.execute(
            0,
            Command::UpdateUnderlay {
                record: missing_underlay.clone(),
            },
        ),
        Err(CommandError::UnderlayNotFound(missing_underlay.id)),
    );
    assert_eq!(editor_state_snapshot(&editor), before_state);
    assert_eq!(editor.annotations(), &before_annotations);
    assert_eq!(editor.underlays(), &before_underlays);

    let mut unlocked = editor;
    for layer in &mut unlocked.project_layers.layers {
        if layer.id == annotation_source.id || layer.id == underlay_source.id {
            layer.locked = false;
        }
    }
    unlocked
        .execute(
            0,
            Command::UpdateAnnotation {
                record: annotation_update.clone(),
            },
        )
        .expect("move annotation between unlocked layers");
    unlocked
        .execute(
            1,
            Command::UpdateUnderlay {
                record: underlay_update.clone(),
            },
        )
        .expect("move underlay between unlocked layers");
    assert_eq!(
        unlocked.annotations().annotations,
        vec![annotation_update.clone()]
    );
    assert_eq!(
        unlocked.underlays().underlays,
        vec![underlay_update.clone()]
    );

    unlocked.undo(2).expect("undo underlay transfer");
    assert_eq!(unlocked.underlays().underlays, vec![underlay.clone()]);
    unlocked.undo(3).expect("undo annotation transfer");
    assert_eq!(unlocked.annotations().annotations, vec![annotation.clone()]);
    unlocked.redo(4).expect("redo annotation transfer");
    assert_eq!(unlocked.annotations().annotations, vec![annotation_update]);
    unlocked.redo(5).expect("redo underlay transfer");
    assert_eq!(unlocked.underlays().underlays, vec![underlay_update]);
}

#[test]
fn connected_vertex_removal_rejects_dangling_annotation_and_round_trips_when_unanchored() {
    let start = vertex_at(0.0, 0.0);
    let terminal = vertex_at(10.0, 0.0);
    let edge = Edge {
        id: EdgeId::new(),
        start: start.id,
        end: terminal.id,
        kind: EdgeKind::Mountain,
    };
    let annotation_layer = auxiliary_layer(
        "Connected vertex annotation",
        ori_domain::LayerContentKindV1::Annotation,
        false,
    );
    let mut editor = editor_with_test_layers(
        CreasePattern {
            vertices: vec![start, terminal.clone()],
            edges: vec![edge.clone()],
        },
        Paper::default(),
        vec![annotation_layer.clone()],
        Vec::new(),
    );
    editor.annotations.annotations.push(test_annotation(
        AnnotationId::new(),
        ori_domain::AnnotationAnchorV1::Vertex {
            vertex: terminal.id,
            offset: Point2::new(0.0, 0.0),
        },
        annotation_layer.id,
    ));
    let before = editor_state_snapshot(&editor);
    let annotations = editor.annotations().clone();

    assert_eq!(
        editor.execute(
            0,
            Command::RemoveConnectedVertex {
                vertex_id: terminal.id,
                edge_id: edge.id,
            },
        ),
        Err(CommandError::InvalidAnnotation),
    );
    assert_eq!(editor_state_snapshot(&editor), before);
    assert_eq!(editor.annotations(), &annotations);

    let mut unanchored = editor;
    unanchored.annotations.annotations.clear();
    unanchored
        .execute(
            0,
            Command::RemoveConnectedVertex {
                vertex_id: terminal.id,
                edge_id: edge.id,
            },
        )
        .expect("remove unanchored connected vertex");
    assert!(unanchored.vertex_index(terminal.id).is_none());
    unanchored.undo(1).expect("undo connected vertex removal");
    assert!(unanchored.vertex_index(terminal.id).is_some());
    unanchored.redo(2).expect("redo connected vertex removal");
    assert!(unanchored.vertex_index(terminal.id).is_none());
}

#[test]
fn boundary_vertex_removal_rejects_dangling_annotation_and_round_trips_when_unanchored() {
    let (_, pattern, paper) = simple_rectangular_editor();
    let target = paper.boundary_vertices[1];
    let annotation_layer = auxiliary_layer(
        "Boundary vertex annotation",
        ori_domain::LayerContentKindV1::Annotation,
        false,
    );
    let mut editor =
        editor_with_test_layers(pattern, paper, vec![annotation_layer.clone()], Vec::new());
    editor.annotations.annotations.push(test_annotation(
        AnnotationId::new(),
        ori_domain::AnnotationAnchorV1::Vertex {
            vertex: target,
            offset: Point2::new(0.0, 0.0),
        },
        annotation_layer.id,
    ));
    let before = editor_state_snapshot(&editor);
    let annotations = editor.annotations().clone();

    assert_eq!(
        editor.execute(0, Command::RemoveBoundaryVertex { vertex: target }),
        Err(CommandError::InvalidAnnotation),
    );
    assert_eq!(editor_state_snapshot(&editor), before);
    assert_eq!(editor.annotations(), &annotations);

    let mut unanchored = editor;
    unanchored.annotations.annotations.clear();
    unanchored
        .execute(0, Command::RemoveBoundaryVertex { vertex: target })
        .expect("remove unanchored boundary vertex");
    assert!(unanchored.vertex_index(target).is_none());
    unanchored.undo(1).expect("undo boundary vertex removal");
    assert!(unanchored.vertex_index(target).is_some());
    unanchored.redo(2).expect("redo boundary vertex removal");
    assert!(unanchored.vertex_index(target).is_none());
}

#[test]
fn normalized_document_rejects_dangling_annotation_and_round_trips_when_unanchored() {
    let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).expect("valid sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let isolated = vertex_at(50.0, 50.0);
    pattern.vertices.push(isolated.clone());
    let annotation_layer = auxiliary_layer(
        "Normalized vertex annotation",
        ori_domain::LayerContentKindV1::Annotation,
        false,
    );
    let mut editor =
        editor_with_test_layers(pattern, paper, vec![annotation_layer.clone()], Vec::new());
    editor.annotations.annotations.push(test_annotation(
        AnnotationId::new(),
        ori_domain::AnnotationAnchorV1::Vertex {
            vertex: isolated.id,
            offset: Point2::new(0.0, 0.0),
        },
        annotation_layer.id,
    ));
    let mut target = editor.pattern().clone();
    target.vertices.retain(|vertex| vertex.id != isolated.id);
    let project_layers = editor.project_layers().clone();
    let before = editor_state_snapshot(&editor);
    let annotations = editor.annotations().clone();

    assert_eq!(
        editor.execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern: target.clone(),
                project_layers: project_layers.clone(),
            },
        ),
        Err(CommandError::InvalidAnnotation),
    );
    assert_eq!(editor_state_snapshot(&editor), before);
    assert_eq!(editor.annotations(), &annotations);

    let mut unanchored = editor;
    unanchored.annotations.annotations.clear();
    unanchored
        .execute(
            0,
            Command::ApplyNormalizedEdgeDocument {
                pattern: target,
                project_layers,
            },
        )
        .expect("apply unanchored normalized document");
    assert!(unanchored.vertex_index(isolated.id).is_none());
    unanchored.undo(1).expect("undo normalized document");
    assert!(unanchored.vertex_index(isolated.id).is_some());
    unanchored.redo(2).expect("redo normalized document");
    assert!(unanchored.vertex_index(isolated.id).is_none());
}
