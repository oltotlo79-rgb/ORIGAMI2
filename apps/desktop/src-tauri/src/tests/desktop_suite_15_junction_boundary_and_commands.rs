#[test]
fn t_junction_connection_returns_reused_vertex_and_undoable_dirty_snapshot() {
    let (mut project, interior, stem, junction) = t_junction_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_count = original_document.crease_pattern.vertices.len();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();

    let response = execute_t_junction_connection(&mut project, project_id, 0, stem.id, interior.id)
        .expect("connect T-junction with reverse arguments");

    assert_eq!(response.vertex_id, junction);
    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_vertex_count
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices,
        original_document.crease_pattern.vertices
    );
    let split_original = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| edge.id == interior.id)
        .expect("split original edge");
    assert_eq!(split_original.start, interior.start);
    assert_eq!(split_original.end, junction);
    assert_eq!(split_original.kind, EdgeKind::Mountain);
    let generated = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| !original_edge_ids.contains(&edge.id))
        .expect("generated T-junction edge");
    assert_eq!(generated.start, junction);
    assert_eq!(generated.end, interior.end);
    assert_eq!(generated.kind, EdgeKind::Mountain);
    assert!(
        response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge == &stem)
    );
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project.editor.undo(1).expect("undo T-junction connection");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo T-junction connection");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn boundary_t_junction_api_splits_sheet_outline_with_reused_vertex_and_exact_history() {
    let (mut project, boundary, stem, junction) = boundary_t_junction_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_count = original_document.crease_pattern.vertices.len();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let original_boundary_vertices = original_document.paper.boundary_vertices.clone();

    let response = execute_t_junction_connection(&mut project, project_id, 0, stem.id, boundary.id)
        .expect("connect a crease endpoint to the strict interior of the sheet boundary");

    assert_eq!(response.vertex_id, junction);
    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_vertex_count
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices,
        original_document.crease_pattern.vertices
    );
    assert_eq!(
        response.snapshot.paper.boundary_vertices,
        vec![
            original_boundary_vertices[0],
            junction,
            original_boundary_vertices[1],
            original_boundary_vertices[2],
            original_boundary_vertices[3],
        ]
    );

    let split_original = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| edge.id == boundary.id)
        .expect("original boundary segment");
    assert_eq!(split_original.start, boundary.start);
    assert_eq!(split_original.end, junction);
    assert_eq!(split_original.kind, EdgeKind::Boundary);
    let generated = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| !original_edge_ids.contains(&edge.id))
        .expect("generated boundary segment");
    assert_eq!(generated.start, junction);
    assert_eq!(generated.end, boundary.end);
    assert_eq!(generated.kind, EdgeKind::Boundary);
    assert!(
        response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge == &stem)
    );
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo boundary T-junction connection");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo boundary T-junction connection");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn t_junction_api_conflicts_and_wrong_geometry_preserve_project_state() {
    let (mut project, interior, stem, _) = t_junction_project();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let wrong_project =
        execute_t_junction_connection(&mut project, ProjectId::new(), 0, interior.id, stem.id)
            .expect_err("wrong project must fail");
    assert!(wrong_project.contains("active project changed"));
    assert_eq!(project_state_signature(&project), before);

    let stale = execute_t_junction_connection(&mut project, project_id, 3, interior.id, stem.id)
        .expect_err("stale revision must fail");
    assert_eq!(stale, "expected revision 3, but the current revision is 0");
    assert_eq!(project_state_signature(&project), before);

    let boundary = project.editor.pattern().edges[0].id;
    let boundary_error =
        execute_t_junction_connection(&mut project, project_id, 0, boundary, interior.id)
            .expect_err("non-intersecting boundary target must fail");
    assert_eq!(
        boundary_error,
        "the selected edges do not form exactly one strict T-junction"
    );
    assert_eq!(project_state_signature(&project), before);

    let (mut crossing, first, second) = crossing_project();
    let crossing_project_id = crossing.project_id;
    let crossing_before = project_state_signature(&crossing);
    let proper_x =
        execute_t_junction_connection(&mut crossing, crossing_project_id, 0, first.id, second.id)
            .expect_err("proper X must not be accepted as T-junction");
    assert_eq!(
        proper_x,
        "the selected edges do not form exactly one strict T-junction"
    );
    assert_eq!(project_state_signature(&crossing), crossing_before);
}

#[test]
fn generated_id_boundary_split_handles_reverse_closing_edge_and_document_history() {
    let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
    let (mut pattern, paper) = sheet.into_parts();
    let forward_closing_edge = pattern.edges[3].clone();
    pattern.edges[3] = Edge {
        start: forward_closing_edge.end,
        end: forward_closing_edge.start,
        ..forward_closing_edge
    };
    let target_edge = pattern.edges[3].clone();
    let original_vertex_ids = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let project_id = project.project_id;
    let original_document = project.document();

    let response = execute_boundary_split(&mut project, project_id, 0, target_edge.id, 0.25)
        .expect("split reverse closing edge");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(response.paper.boundary_vertices.len(), 5);
    let new_vertex = response.paper.boundary_vertices[4];
    assert!(!original_vertex_ids.contains(&new_vertex));
    assert_eq!(response.crease_pattern.vertices.len(), 5);
    assert_eq!(
        response.crease_pattern.vertices[4],
        Vertex {
            id: new_vertex,
            position: Point2::new(0.0, 20.0),
        }
    );
    assert_eq!(response.crease_pattern.edges.len(), 5);
    assert_eq!(response.crease_pattern.edges[3].id, target_edge.id);
    assert_eq!(response.crease_pattern.edges[3].start, target_edge.start);
    assert_eq!(response.crease_pattern.edges[3].end, new_vertex);
    let generated_edge = &response.crease_pattern.edges[4];
    assert!(!original_edge_ids.contains(&generated_edge.id));
    assert_eq!(generated_edge.start, new_vertex);
    assert_eq!(generated_edge.end, target_edge.end);
    assert_eq!(generated_edge.kind, EdgeKind::Boundary);
    assert!(validation_snapshot(&project).is_valid);
    let split_document = project.document();
    assert_ne!(split_document, original_document);

    project.editor.undo(1).expect("undo boundary split");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo boundary split");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), split_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn boundary_split_conflict_and_invalid_fraction_preserve_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let edge = project.editor.pattern().edges[0].id;
    let before = project_state_signature(&project);

    let conflict = execute_boundary_split(&mut project, project_id, 1, edge, 0.5)
        .expect_err("stale split must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let invalid = execute_boundary_split(&mut project, project_id, 0, edge, f64::NAN)
        .expect_err("non-finite split must fail");
    assert_eq!(invalid, "boundary split fraction must be finite");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn boundary_vertex_removal_updates_document_dirty_state_and_history() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();
    let target = project.editor.paper().boundary_vertices[1];
    let previous = project.editor.paper().boundary_vertices[0];
    let next = project.editor.paper().boundary_vertices[2];
    let remaining = project.editor.paper().boundary_vertices[3];
    let kept_edge = project.editor.pattern().edges[0].clone();
    let removed_edge = project.editor.pattern().edges[1].clone();

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::RemoveBoundaryVertex { vertex: target },
    )
    .expect("remove boundary vertex");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(
        response.paper.boundary_vertices,
        vec![previous, next, remaining]
    );
    assert!(
        !response
            .crease_pattern
            .vertices
            .iter()
            .any(|vertex| vertex.id == target)
    );
    assert_eq!(response.crease_pattern.edges[0].id, kept_edge.id);
    assert_eq!(response.crease_pattern.edges[0].start, previous);
    assert_eq!(response.crease_pattern.edges[0].end, next);
    assert!(
        !response
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge.id == removed_edge.id)
    );
    assert!(validation_snapshot(&project).is_valid);
    let removed_document = project.document();
    assert_ne!(removed_document, original_document);

    project.editor.undo(1).expect("undo boundary removal");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo boundary removal");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), removed_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn boundary_vertex_removal_conflict_preserves_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let target = project.editor.paper().boundary_vertices[1];
    let before = project_state_signature(&project);

    let error = execute_command(
        &mut project,
        project_id,
        1,
        Command::RemoveBoundaryVertex { vertex: target },
    )
    .expect_err("stale boundary removal must fail");

    assert_eq!(error, "expected revision 1, but the current revision is 0");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn new_project_replaces_only_the_expected_unchanged_project() {
    let mut project = initial_project_state();
    let old_instance_id = project.instance_id;
    let old_project_id = project.project_id;

    let response = replace_with_new_project(
        &mut project,
        old_instance_id,
        old_project_id,
        0,
        new_project_parameters(),
    )
    .expect("replace current project");

    assert_ne!(response.project_id, old_project_id);
    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.name, "Test sheet");
    assert!(response.current_path.is_none());
    assert_eq!(response.revision, 0);
    assert!(response.saved_revision.is_none());
    assert!(response.is_dirty);
    assert!(!response.can_undo);
    assert!(!response.can_redo);
    assert!(project.saved_document.is_none());
}

#[test]
fn new_project_errors_leave_existing_state_untouched() {
    let mut project = initial_project_state();
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    assert!(
        replace_with_new_project(
            &mut project,
            instance_id,
            ProjectId::new(),
            0,
            new_project_parameters(),
        )
        .is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    assert!(
        replace_with_new_project(
            &mut project,
            instance_id,
            project_id,
            1,
            new_project_parameters(),
        )
        .is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    let mut invalid_name = new_project_parameters();
    invalid_name.name = " \0 ".to_owned();
    assert!(
        replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_name).is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    let mut invalid_dimensions = new_project_parameters();
    invalid_dimensions.width_mm = 0.0;
    assert!(
        replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_dimensions,)
            .is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    let mut invalid_thickness = new_project_parameters();
    invalid_thickness.thickness_mm = f64::NAN;
    assert!(
        replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_thickness,)
            .is_err()
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn delayed_new_project_rejects_same_document_revision_after_reopen_aba() {
    let mut project = initial_project_state();
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let document = project.document();
    project = ProjectState::from_valid_document(document, PathBuf::from("same-project.ori2"));
    assert_eq!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), expected_revision);
    assert_ne!(project.instance_id, stale_instance_id);
    let before = project_state_signature(&project);

    let error = replace_with_new_project(
        &mut project,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        new_project_parameters(),
    )
    .expect_err("reopened ABA instance must reject delayed new-project work");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn execute_command_rejects_same_document_revision_after_reopen_aba() {
    let project = initial_project_state();
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let mut reopened =
        ProjectState::from_valid_document(project.document(), PathBuf::from("same-project.ori2"));
    assert_eq!(reopened.project_id, expected_project_id);
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert_ne!(reopened.instance_id, stale_instance_id);
    let before = project_state_signature(&reopened);

    let error = super::execute_command(
        &mut reopened,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(25.0, 25.0),
        },
    )
    .expect_err("reopened ABA instance must reject a delayed edit command");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn execute_undo_rejects_same_project_and_revision_from_a_foreign_instance() {
    let mut stale_project = initial_project_state();
    let expected_project_id = stale_project.project_id;
    execute_command(
        &mut stale_project,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("advance the stale project to revision one");
    let stale_instance_id = stale_project.instance_id;
    let expected_revision = stale_project.editor.revision();

    let mut reopened = ProjectState::from_valid_document(
        stale_project.document(),
        PathBuf::from("same-project.ori2"),
    );
    execute_command(
        &mut reopened,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: false },
    )
    .expect("create undo history at the same revision");
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert!(reopened.editor.can_undo());
    assert_ne!(reopened.instance_id, stale_instance_id);
    let before = project_state_signature(&reopened);

    let error = super::execute_undo(
        &mut reopened,
        stale_instance_id,
        expected_project_id,
        expected_revision,
    )
    .expect_err("foreign project instance must not consume undo history");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn execute_redo_rejects_same_project_and_revision_from_a_foreign_instance() {
    let mut stale_project = initial_project_state();
    let expected_project_id = stale_project.project_id;
    execute_command(
        &mut stale_project,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("advance the stale project to revision one");
    execute_command(
        &mut stale_project,
        expected_project_id,
        1,
        Command::SetCuttingAllowed { allowed: false },
    )
    .expect("advance the stale project to revision two");
    let stale_instance_id = stale_project.instance_id;
    let expected_revision = stale_project.editor.revision();

    let mut reopened = ProjectState::from_valid_document(
        stale_project.document(),
        PathBuf::from("same-project.ori2"),
    );
    execute_command(
        &mut reopened,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("create current-instance undo history");
    execute_undo(&mut reopened, expected_project_id, 1)
        .expect("create redo history at revision two");
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert!(reopened.editor.can_redo());
    assert_ne!(reopened.instance_id, stale_instance_id);
    let before = project_state_signature(&reopened);

    let error = super::execute_redo(
        &mut reopened,
        stale_instance_id,
        expected_project_id,
        expected_revision,
    )
    .expect_err("foreign project instance must not consume redo history");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn move_vertex_returns_the_updated_revision_and_snapshot() {
    let id = VertexId::new();
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![Vertex {
            id,
            position: Point2::new(1.0, 2.0),
        }],
        edges: Vec::new(),
    });
    let project_id = project.project_id;
    assert!(!project.is_dirty());

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::MoveVertex {
            id,
            position: Point2::new(3.0, 5.0),
        },
    )
    .expect("move vertex");

    assert_eq!(response.revision, 1);
    assert_eq!(
        response.crease_pattern.vertices[0].position,
        Point2::new(3.0, 5.0)
    );
    assert!(response.can_undo);
    assert!(response.is_dirty);
}
