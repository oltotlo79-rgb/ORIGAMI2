#[test]
fn vertex_coordinate_expressions_follow_native_history_and_archive_round_trip() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let vertex = VertexId::new();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddVertex {
            id: vertex,
            position: Point2::new(0.5, -2.0),
        },
    )
    .expect("add expression-backed vertex");
    project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
        vertex, "1 / 2", "-sqrt(4)", 0.5, -2.0,
    ));
    let binding = project.numeric_expressions.vertex_coordinates[0].clone();
    assert_eq!(binding.x_source, "1 / 2");
    assert_eq!(binding.y_source, "-sqrt(4)");
    validate_loaded_numeric_expression_bindings(
        &project
            .project_archive()
            .expect("serialize expression history")
            .document,
    )
    .expect("re-evaluate every persisted expression");

    execute_undo(&mut project, project_id, 1).expect("undo vertex");
    assert!(project.numeric_expressions.vertex_coordinates.is_empty());
    execute_redo(&mut project, project_id, 2).expect("redo vertex");
    assert_eq!(
        project.numeric_expressions.vertex_coordinates,
        vec![binding]
    );
}

#[test]
fn creation_expressions_follow_document_dirty_state_without_entering_editor_undo_history() {
    let mut project =
        create_new_project_state(new_project_parameters()).expect("valid new project");
    let project_id = project.project_id;
    let saved_document = project.document();
    let saved_expressions = project.numeric_expressions.clone();
    project.saved_document = Some(saved_document.clone());
    project.saved_revision = Some(project.editor.revision());
    assert!(!project.is_dirty());

    let resized = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: 420.0,
            height_mm: 594.0,
        },
    )
    .expect("resize paper");
    assert!(resized.is_dirty);
    assert_eq!(
        project.numeric_expressions.rectangular_paper_creation,
        saved_expressions.rectangular_paper_creation
    );

    project.editor.undo(1).expect("undo resize");
    assert_eq!(project.document(), saved_document);
    assert_eq!(
        project.numeric_expressions.rectangular_paper_creation,
        saved_expressions.rectangular_paper_creation
    );
    assert!(!project.is_dirty());

    project
        .numeric_expressions
        .rectangular_paper_creation
        .as_mut()
        .expect("creation expressions")
        .width_source = "210 + 0".to_owned();
    assert!(project.is_dirty());
}

#[test]
fn snapshot_paper_uses_the_current_editor_cutting_setting() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    assert!(!project.editor.paper().cutting_allowed);

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("enable cutting");

    assert!(response.cutting_allowed);
    assert!(response.paper.cutting_allowed);
    assert!(project.document().paper.cutting_allowed);
}

#[test]
fn paper_properties_follow_undo_redo_dirty_save_and_validation() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original = project.editor.paper().clone();
    let front_color = RgbaColor::opaque(15, 35, 55);
    let back_color = RgbaColor::opaque(205, 185, 165);

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::UpdatePaperProperties {
            thickness_mm: 0.0,
            front_color,
            back_color,
            front_texture_asset: None,
            back_texture_asset: None,
            cutting_allowed: true,
        },
    )
    .expect("update paper properties");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert_eq!(response.paper.thickness_mm, 0.0);
    assert_eq!(response.paper.front.color, front_color);
    assert_eq!(response.paper.back.color, back_color);
    assert!(response.paper.cutting_allowed);
    assert!(validation_snapshot(&project).is_valid);

    project.editor.undo(1).expect("undo properties");
    assert_eq!(project.editor.paper(), &original);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo properties");
    assert!(project.is_dirty());
    let saved_document = project.document();
    project.saved_revision = Some(project.editor.revision());
    project.saved_document = Some(saved_document.clone());
    assert!(!project.is_dirty());
    assert_eq!(project.document(), saved_document);

    project.editor.undo(3).expect("undo after save");
    assert!(project.is_dirty());
    project.editor.redo(4).expect("redo to saved content");
    assert!(!project.is_dirty());
}

#[test]
fn imported_front_textures_remain_live_across_undo_redo() {
    let mut project = initial_project_state();
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let png = |tag| {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.push(tag);
        bytes
    };

    register_front_texture(
        &mut project,
        instance_id,
        project_id,
        0,
        ProjectTextureMediaTypeV1::Png,
        png(1),
    )
    .expect("first texture");
    let first = project.editor.paper().front.texture_asset.unwrap();
    register_front_texture(
        &mut project,
        instance_id,
        project_id,
        1,
        ProjectTextureMediaTypeV1::Png,
        png(2),
    )
    .expect("replacement texture");
    let second = project.editor.paper().front.texture_asset.unwrap();
    assert_ne!(first, second);
    assert_eq!(project.texture_assets.len(), 2);

    project.editor.undo(2).expect("undo texture replacement");
    assert_eq!(project.editor.paper().front.texture_asset, Some(first));
    ori_formats::write_project_json(&project.document()).expect("undo document");
    project.editor.redo(3).expect("redo texture replacement");
    assert_eq!(project.editor.paper().front.texture_asset, Some(second));
    ori_formats::write_project_json(&project.document()).expect("redo document");
}

#[test]
fn imported_back_textures_remain_live_across_undo_redo() {
    let mut project = initial_project_state();
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let png = |tag| {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.push(tag);
        bytes
    };
    register_back_texture(
        &mut project,
        instance_id,
        project_id,
        0,
        ProjectTextureMediaTypeV1::Png,
        png(1),
    )
    .expect("first back texture");
    let first = project.editor.paper().back.texture_asset.unwrap();
    register_back_texture(
        &mut project,
        instance_id,
        project_id,
        1,
        ProjectTextureMediaTypeV1::Png,
        png(2),
    )
    .expect("replacement back texture");
    let second = project.editor.paper().back.texture_asset.unwrap();
    assert_ne!(first, second);
    project.editor.undo(2).expect("undo back texture");
    assert_eq!(project.editor.paper().back.texture_asset, Some(first));
    ori_formats::write_project_json(&project.document()).expect("undo document");
    project.editor.redo(3).expect("redo back texture");
    assert_eq!(project.editor.paper().back.texture_asset, Some(second));
    ori_formats::write_project_json(&project.document()).expect("redo document");
}

#[test]
fn length_display_unit_follows_snapshot_dirty_history_and_fingerprint_contracts() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    let reference_edge = project.editor.pattern().edges[0].id;

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::SetLengthDisplayUnit {
            unit: LengthDisplayUnit::PaperEdgeRatio { reference_edge },
        },
    )
    .expect("set native length display unit");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(
        response.paper.length_display_unit,
        LengthDisplayUnit::PaperEdgeRatio { reference_edge }
    );
    assert_eq!(response.fold_model_fingerprint, fingerprint);
    assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);
    assert_eq!(
        project.document().paper.length_display_unit,
        LengthDisplayUnit::PaperEdgeRatio { reference_edge }
    );

    project.editor.undo(1).expect("undo display unit");
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());
    assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);

    project.editor.redo(2).expect("redo display unit");
    assert!(project.is_dirty());
    assert_eq!(
        project.editor.paper().length_display_unit,
        LengthDisplayUnit::PaperEdgeRatio { reference_edge }
    );
    assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);
}

#[test]
fn invalid_paper_property_command_preserves_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let conflict = execute_command(
        &mut project,
        project_id,
        1,
        Command::UpdatePaperProperties {
            thickness_mm: 0.3,
            front_color: RgbaColor::opaque(1, 2, 3),
            back_color: RgbaColor::opaque(4, 5, 6),
            front_texture_asset: None,
            back_texture_asset: None,
            cutting_allowed: true,
        },
    )
    .expect_err("stale property update must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let error = execute_command(
        &mut project,
        project_id,
        0,
        Command::UpdatePaperProperties {
            thickness_mm: f64::NAN,
            front_color: RgbaColor::opaque(1, 2, 3),
            back_color: RgbaColor::opaque(4, 5, 6),
            front_texture_asset: None,
            back_texture_asset: None,
            cutting_allowed: true,
        },
    )
    .expect_err("invalid thickness must fail");

    assert_eq!(error, "paper thickness must be finite");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn rectangular_resize_updates_document_dirty_state_and_undo_redo() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = project
        .editor
        .pattern()
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edges = project.editor.pattern().edges.clone();
    let original_paper = project.editor.paper().clone();

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: 210.0,
            height_mm: 297.0,
        },
    )
    .expect("resize paper");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(response.paper, original_paper);
    assert_eq!(
        response
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        original_vertex_ids
    );
    assert_eq!(response.crease_pattern.edges, original_edges);
    assert!(
        response
            .crease_pattern
            .vertices
            .iter()
            .any(|vertex| vertex.position == Point2::new(210.0, 297.0))
    );
    assert!(validation_snapshot(&project).is_valid);
    let resized_document = project.document();
    assert_ne!(resized_document, original_document);
    assert_eq!(resized_document.paper, original_paper);

    project.editor.undo(1).expect("undo resize");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo resize");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), resized_document);
    assert!(project.is_dirty());
}

#[test]
fn same_size_resize_has_history_without_making_the_document_dirty() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: DEFAULT_SHEET_SIZE_MM,
            height_mm: DEFAULT_SHEET_SIZE_MM,
        },
    )
    .expect("same-size resize");

    assert_eq!(response.revision, 1);
    assert!(response.can_undo);
    assert!(!response.is_dirty);
    assert_eq!(project.document(), original_document);
}

#[test]
fn resize_conflicts_invalid_dimensions_and_overflow_preserve_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let conflict = execute_command(
        &mut project,
        project_id,
        1,
        Command::ResizeRectangularPaper {
            width_mm: 210.0,
            height_mm: 297.0,
        },
    )
    .expect_err("stale resize must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let invalid = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: 0.0,
            height_mm: 297.0,
        },
    )
    .expect_err("zero width must fail");
    assert_eq!(invalid, "paper width must be greater than zero");
    assert_eq!(project_state_signature(&project), before);

    let overflow = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: f64::MAX,
            height_mm: 2.0,
        },
    )
    .expect_err("unrepresentable area must fail");
    assert_eq!(
        overflow,
        "target paper area is too large to represent safely"
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn generated_id_edge_split_updates_snapshot_document_and_history() {
    let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
    let (mut pattern, paper) = sheet.into_parts();
    let crease = Edge {
        id: EdgeId::new(),
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Valley,
    };
    pattern.edges.push(crease.clone());
    let original_vertex_ids = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    let original_edge_index = pattern.edges.len() - 1;
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let project_id = project.project_id;
    let original_document = project.document();

    let response =
        execute_edge_split(&mut project, project_id, 0, crease.id, 0.5).expect("split crease edge");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(response.paper, original_document.paper);
    assert_eq!(response.crease_pattern.vertices.len(), 5);
    let generated_vertices = response
        .crease_pattern
        .vertices
        .iter()
        .filter(|vertex| !original_vertex_ids.contains(&vertex.id))
        .collect::<Vec<_>>();
    assert_eq!(generated_vertices.len(), 1);
    let generated_vertex = generated_vertices[0];
    assert_eq!(generated_vertex.position, Point2::new(50.0, 40.0));
    assert_eq!(response.crease_pattern.edges.len(), 6);
    assert_eq!(
        response.crease_pattern.edges[original_edge_index],
        Edge {
            end: generated_vertex.id,
            ..crease.clone()
        }
    );
    let generated_edge = &response.crease_pattern.edges[original_edge_index + 1];
    assert!(!original_edge_ids.contains(&generated_edge.id));
    assert_eq!(generated_edge.start, generated_vertex.id);
    assert_eq!(generated_edge.end, crease.end);
    assert_eq!(generated_edge.kind, EdgeKind::Valley);
    assert!(validation_snapshot(&project).is_valid);
    let split_document = project.document();
    assert_ne!(split_document, original_document);

    project.editor.undo(1).expect("undo edge split");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo edge split");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), split_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}
