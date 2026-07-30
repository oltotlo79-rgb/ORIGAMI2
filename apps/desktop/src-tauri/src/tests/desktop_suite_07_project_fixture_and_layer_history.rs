fn new_project_parameters() -> NewProjectParameters {
    NewProjectParameters {
        name: "  Test sheet  ".to_owned(),
        width_expression: "210".to_owned(),
        height_expression: "297".to_owned(),
        width_mm: 210.0,
        height_mm: 297.0,
        thickness_mm: 0.2,
        cutting_allowed: true,
        front_color: RgbaColor {
            red: 10,
            green: 20,
            blue: 30,
            alpha: 240,
        },
        back_color: RgbaColor {
            red: 220,
            green: 210,
            blue: 200,
            alpha: 230,
        },
    }
}

fn cellular_multi_fold_project_state() -> ProjectState {
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(6.0, 0.0),
        Point2::new(8.0, 0.0),
        Point2::new(8.0, 6.0),
        Point2::new(6.0, 6.0),
        Point2::new(2.0, 6.0),
        Point2::new(0.0, 6.0),
    ];
    let vertices = positions
        .into_iter()
        .map(|position| Vertex {
            id: VertexId::new(),
            position,
        })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: EdgeId::new(),
            start: vertices[1].id,
            end: vertices[6].id,
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: EdgeId::new(),
            start: vertices[2].id,
            end: vertices[5].id,
            kind: EdgeKind::Valley,
        },
    ]);
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        ..Paper::default()
    };
    ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper)
}

fn four_ray_square_project_state(
    fold_endpoint_indices: [usize; 4],
    assignments: [EdgeKind; 4],
) -> (ProjectState, VertexId) {
    let boundary_positions = [
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(20.0, 0.0),
        Point2::new(20.0, 10.0),
        Point2::new(20.0, 20.0),
        Point2::new(10.0, 20.0),
        Point2::new(0.0, 20.0),
        Point2::new(0.0, 10.0),
    ];
    let mut vertices = boundary_positions
        .into_iter()
        .map(|position| Vertex {
            id: VertexId::new(),
            position,
        })
        .collect::<Vec<_>>();
    let center = Vertex {
        id: VertexId::new(),
        position: Point2::new(10.0, 10.0),
    };
    let center_id = center.id;
    vertices.push(center);

    let mut edges = (0..boundary_positions.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: vertices[index].id,
            end: vertices[(index + 1) % boundary_positions.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend(
        fold_endpoint_indices
            .into_iter()
            .zip(assignments)
            .map(|(endpoint, kind)| Edge {
                id: EdgeId::new(),
                start: center_id,
                end: vertices[endpoint].id,
                kind,
            }),
    );
    let paper = Paper {
        boundary_vertices: vertices[..boundary_positions.len()]
            .iter()
            .map(|vertex| vertex.id)
            .collect(),
        ..Paper::default()
    };
    (
        ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper),
        center_id,
    )
}

#[derive(Debug, PartialEq)]
struct ProjectStateSignature {
    instance_id: ProjectId,
    project_id: ProjectId,
    document: ProjectDocument,
    editor_debug: String,
    applied_pose_authority: applied_pose::CurrentAppliedPoseAuthoritySnapshot,
    current_path: Option<PathBuf>,
    saved_revision: Option<u64>,
    saved_document: Option<ProjectDocument>,
    revision: u64,
    can_undo: bool,
    can_redo: bool,
    is_dirty: bool,
}

fn project_state_signature(project: &ProjectState) -> ProjectStateSignature {
    ProjectStateSignature {
        instance_id: project.instance_id,
        project_id: project.project_id,
        document: project.document(),
        editor_debug: format!("{:?}", project.editor),
        applied_pose_authority: project
            .applied_pose_authority
            .test_snapshot()
            .expect("capture applied-pose authority"),
        current_path: project.current_path.clone(),
        saved_revision: project.saved_revision,
        saved_document: project.saved_document.clone(),
        revision: project.editor.revision(),
        can_undo: project.editor.can_undo(),
        can_redo: project.editor.can_redo(),
        is_dirty: project.is_dirty(),
    }
}

fn geometric_constraint_binding(state: &AppState) -> (ProjectId, ProjectId, u64) {
    let project = lock_project(state).expect("lock geometric-constraint project");
    (
        project.instance_id,
        project.project_id,
        project.editor.revision(),
    )
}

fn geometric_constraint_project_signature(state: &AppState) -> ProjectStateSignature {
    let project = lock_project(state).expect("lock geometric-constraint project");
    project_state_signature(&project)
}

fn run_default_geometric_constraint_analysis(
    state: &AppState,
    binding: (ProjectId, ProjectId, u64),
) -> Result<GeometricConstraintPreflightResponse, String> {
    tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        |pattern, document, _runtime| {
            Ok(analyze_geometric_constraint_document(&pattern, &document))
        },
    ))
}

fn wait_for_geometric_constraint_worker_idle(state: &Arc<AppState>) {
    let observer_state = Arc::clone(state);
    let (idle_tx, idle_rx) = mpsc::sync_channel(0);
    let observer = thread::spawn(move || {
        while observer_state.geometric_constraint_worker_is_busy() {
            thread::yield_now();
        }
        idle_tx.send(()).expect("announce idle worker gate");
    });
    idle_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker gate must become idle");
    observer
        .join()
        .expect("worker-gate observer must not panic");
}

#[test]
fn geometric_constraint_document_is_dirty_undoable_and_loadable() {
    let mut project = initial_project_state();
    let edge = project.editor.pattern().edges[0].id;
    let record = GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint: GeometricConstraintKindV1::Horizontal { edge },
    };
    let project_id = project.project_id;

    let added = execute_command(
        &mut project,
        project_id,
        0,
        Command::AddGeometricConstraint {
            record: record.clone(),
        },
    )
    .expect("add constraint through native project bridge");
    assert_eq!(
        added.geometric_constraints.constraints,
        vec![record.clone()]
    );
    assert!(added.is_dirty);
    assert_eq!(
        project.document().geometric_constraints.constraints,
        vec![record.clone()]
    );

    let undone = execute_undo(&mut project, project_id, 1).expect("undo constraint");
    assert!(undone.geometric_constraints.is_empty());
    assert!(!undone.is_dirty);
    let redone = execute_redo(&mut project, project_id, 2).expect("redo constraint");
    assert_eq!(
        redone.geometric_constraints.constraints,
        vec![record.clone()]
    );
    assert!(redone.is_dirty);

    let document = project.document();
    let loaded =
        ProjectState::from_valid_document(document.clone(), PathBuf::from("constraint.ori2"));
    assert_eq!(loaded.document(), document);
    assert_eq!(
        loaded.editor.geometric_constraints().constraints,
        vec![record]
    );
    assert!(!loaded.is_dirty());
    assert!(!loaded.editor.can_undo());
    assert!(!loaded.editor.can_redo());
}

#[test]
fn project_layers_are_snapshotted_dirty_tracked_saved_and_reopened_with_history() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let edge = project.editor.pattern().edges[0].id;
    let layer = LayerRecordV1 {
        id: ori_domain::LayerId::new(),
        name: "Details".to_owned(),
        content_kind: LayerContentKindV1::CreasePattern,
        visible: true,
        locked: false,
        opacity: 1.0,
    };

    let created = execute_command(
        &mut project,
        project_id,
        0,
        Command::CreateLayer {
            layer: layer.clone(),
            target_index: 1,
        },
    )
    .expect("create layer through native project bridge");
    assert_eq!(created.project_layers.layers[1], layer);
    assert!(created.project_layers.edge_assignments.is_empty());
    assert!(created.is_dirty);

    let assigned = execute_command(
        &mut project,
        project_id,
        1,
        Command::AssignEdgeToLayer {
            edge,
            layer: layer.id,
        },
    )
    .expect("assign edge through native project bridge");
    assert_eq!(assigned.project_layers.layer_for_edge(edge), layer.id);
    assert_eq!(project.document().layers, assigned.project_layers);
    assert!(project.is_dirty());

    let presented = execute_command(
        &mut project,
        project_id,
        2,
        Command::UpdateLayerPresentation {
            layer: layer.id,
            visible: false,
            locked: true,
            opacity: 0.25,
        },
    )
    .expect("update layer presentation through native project bridge");
    assert_eq!(project.document().layers, presented.project_layers);
    assert!(!presented.project_layers.layers[1].visible);
    assert!(presented.project_layers.layers[1].locked);
    assert_eq!(presented.project_layers.layers[1].opacity, 0.25);

    let document = project.document();
    let loaded_without_history =
        ProjectState::from_valid_document(document.clone(), PathBuf::from("layers.ori2"));
    assert_eq!(
        loaded_without_history.editor.project_layers(),
        &document.layers
    );
    assert!(!loaded_without_history.is_dirty());

    let directory = TestDirectory::new();
    let path = directory.join("layer-history.ori2");
    save_project_to_path(&mut project, path.clone()).expect("save layered archive");
    assert!(!project.is_dirty());

    let mut reopened = ProjectState::new(CreasePattern::empty());
    let replaced_instance_id = reopened.instance_id;
    let replaced_project_id = reopened.project_id;
    let loaded = load_project_file(path.clone()).expect("load layered archive");
    apply_loaded_project_file(
        &mut reopened,
        replaced_instance_id,
        replaced_project_id,
        0,
        loaded,
    )
    .expect("apply layered archive");
    assert_eq!(reopened.document(), document);
    assert_eq!(reopened.editor.project_layers(), &document.layers);
    assert_eq!(snapshot(&reopened).project_layers, document.layers);
    assert!(!reopened.is_dirty());

    reopened
        .editor
        .undo(0)
        .expect("undo reopened layer presentation");
    assert!(reopened.editor.project_layers().layers[1].visible);
    assert!(!reopened.editor.project_layers().layers[1].locked);
    assert_eq!(reopened.editor.project_layers().layers[1].opacity, 1.0);
    reopened.editor.undo(1).expect("undo reopened assignment");
    assert_eq!(
        reopened.editor.project_layers().layer_for_edge(edge),
        ori_domain::DEFAULT_PROJECT_LAYER_ID
    );
    assert!(reopened.is_dirty());
    reopened
        .editor
        .undo(2)
        .expect("undo reopened layer creation");
    assert_eq!(
        reopened.editor.project_layers(),
        &ProjectLayerDocumentV1::default()
    );
    reopened
        .editor
        .redo(3)
        .expect("redo reopened layer creation");
    reopened.editor.redo(4).expect("redo reopened assignment");
    reopened
        .editor
        .redo(5)
        .expect("redo reopened layer presentation");
    assert_eq!(reopened.document(), document);
    assert!(!reopened.is_dirty());
}

#[test]
fn project_layer_ipc_helpers_guard_binding_and_apply_every_supported_mutation() {
    let mut project = initial_project_state();
    let project_instance_id = project.instance_id;
    let project_id = project.project_id;
    let edge = project.editor.pattern().edges[0].id;
    let original_document = project.document();

    assert!(
        create_project_layer_in_project(
            &mut project,
            ProjectId::new(),
            project_id,
            0,
            "Foreign".to_owned(),
            LayerContentKindV1::CreasePattern,
        )
        .is_err()
    );
    assert_eq!(project.document(), original_document);
    assert_eq!(project.editor.revision(), 0);

    let created_crease = create_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        0,
        "Details".to_owned(),
        LayerContentKindV1::CreasePattern,
    )
    .expect("create crease-pattern layer");
    let crease_layer = created_crease.project_layers.layers[1].id;
    assert_eq!(created_crease.revision, 1);

    let created_annotation = create_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        1,
        "Notes".to_owned(),
        LayerContentKindV1::Annotation,
    )
    .expect("create empty annotation layer");
    let annotation_layer = created_annotation.project_layers.layers[2].id;
    assert_eq!(
        created_annotation.project_layers.layers[2].content_kind,
        LayerContentKindV1::Annotation
    );

    let renamed = rename_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        2,
        crease_layer,
        "Primary folds".to_owned(),
    )
    .expect("rename project layer");
    assert_eq!(renamed.project_layers.layers[1].name, "Primary folds");

    let presented = update_project_layer_presentation_in_project(
        &mut project,
        project_instance_id,
        project_id,
        3,
        crease_layer,
        ProjectLayerPresentationInput {
            visible: false,
            locked: true,
            opacity: 0.4,
        },
    )
    .expect("update project layer presentation");
    let presented_layer = presented
        .project_layers
        .layers
        .iter()
        .find(|layer| layer.id == crease_layer)
        .expect("presented layer");
    assert!(!presented_layer.visible);
    assert!(presented_layer.locked);
    assert_eq!(presented_layer.opacity, 0.4);

    let unlocked = update_project_layer_presentation_in_project(
        &mut project,
        project_instance_id,
        project_id,
        4,
        crease_layer,
        ProjectLayerPresentationInput {
            visible: true,
            locked: false,
            opacity: 0.4,
        },
    )
    .expect("unlock project layer");
    assert!(!unlocked.project_layers.layers[1].locked);

    let moved = move_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        5,
        annotation_layer,
        0,
    )
    .expect("move project layer");
    assert_eq!(moved.project_layers.layers[0].id, annotation_layer);

    let assigned = assign_edge_to_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        6,
        edge,
        crease_layer,
    )
    .expect("assign selected edge to crease-pattern layer");
    assert_eq!(assigned.project_layers.layer_for_edge(edge), crease_layer);

    let deleted = delete_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        7,
        crease_layer,
    )
    .expect("delete project layer");
    assert_eq!(
        deleted.project_layers.layer_for_edge(edge),
        ori_domain::DEFAULT_PROJECT_LAYER_ID
    );
    assert!(
        deleted
            .project_layers
            .layers
            .iter()
            .all(|layer| layer.id != crease_layer)
    );

    assert!(
        delete_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            8,
            ori_domain::DEFAULT_PROJECT_LAYER_ID,
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 8);
    assert_eq!(project.editor.project_layers(), &deleted.project_layers);
}

#[test]
fn project_layer_presentation_ipc_input_is_a_strict_nested_record() {
    let admitted = serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
        "visible": false,
        "locked": true,
        "opacity": 0.4
    }))
    .expect("strict presentation input");
    assert!(!admitted.visible);
    assert!(admitted.locked);
    assert_eq!(admitted.opacity, 0.4);
    assert!(
        serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
            "visible": false,
            "locked": true,
            "opacity": 0.4,
            "future": "rejected"
        }),)
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
            "visible": false,
            "opacity": 0.4
        }),)
        .is_err()
    );
}
