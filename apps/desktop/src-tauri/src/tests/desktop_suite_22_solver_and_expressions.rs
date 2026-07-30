fn solver_stage_fixture() -> (
    ProjectState,
    GeometricConstraintSolveStage,
    VertexId,
    Point2,
) {
    let start = VertexId::new();
    let end = VertexId::new();
    let original = Point2::new(0.0, 0.0);
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![
            ori_domain::Vertex {
                id: start,
                position: original,
            },
            ori_domain::Vertex {
                id: end,
                position: Point2::new(5.0, 0.0),
            },
        ],
        edges: vec![ori_domain::Edge {
            id: EdgeId::new(),
            start,
            end,
            kind: EdgeKind::Auxiliary,
        }],
    });
    project.saved_revision = Some(0);
    let stage = GeometricConstraintSolveStage {
        token: ProjectId::new(),
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: 0,
        positions: vec![(start, Point2::new(2.0, 3.0))],
        expression_bindings: None,
        exact_satisfaction: None,
    };
    (project, stage, start, original)
}

fn solver_vertex_position(project: &ProjectState, id: VertexId) -> Point2 {
    project
        .editor
        .pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == id)
        .unwrap()
        .position
}

#[test]
fn constraint_solver_stale_token_is_atomic() {
    let (mut project, stage, vertex, original) = solver_stage_fixture();
    assert!(
        apply_geometric_constraint_solve_stage(
            &mut project,
            &stage,
            stage.project_instance_id,
            stage.project_id,
            0,
            ProjectId::new(),
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 0);
    assert_eq!(solver_vertex_position(&project, vertex), original);
}

#[test]
fn constraint_solver_layer_lock_is_atomic() {
    let (mut project, mut stage, vertex, original) = solver_stage_fixture();
    let layer = project.editor.project_layers().layers[0].id;
    execute_command(
        &mut project,
        stage.project_id,
        0,
        Command::UpdateLayerPresentation {
            layer,
            visible: true,
            locked: true,
            opacity: 1.0,
        },
    )
    .unwrap();
    stage.revision = 1;
    assert!(
        apply_geometric_constraint_solve_stage(
            &mut project,
            &stage,
            stage.project_instance_id,
            stage.project_id,
            1,
            stage.token,
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 1);
    assert_eq!(solver_vertex_position(&project, vertex), original);
}

#[test]
fn constraint_solver_apply_is_one_history_entry() {
    let (mut project, stage, _, _) = solver_stage_fixture();
    let snapshot = apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        stage.project_instance_id,
        stage.project_id,
        0,
        stage.token,
    )
    .unwrap();
    assert_eq!(snapshot.revision, 1);
    assert!(snapshot.can_undo);
    assert!(!snapshot.can_redo);
}

#[test]
fn constraint_solver_undo_redo_restores_exact_positions() {
    let (mut project, stage, vertex, original) = solver_stage_fixture();
    let target = stage.positions[0].1;
    apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        stage.project_instance_id,
        stage.project_id,
        0,
        stage.token,
    )
    .unwrap();
    execute_undo(&mut project, stage.project_id, 1).unwrap();
    assert_eq!(solver_vertex_position(&project, vertex), original);
    execute_redo(&mut project, stage.project_id, 2).unwrap();
    assert_eq!(solver_vertex_position(&project, vertex), target);
}

#[test]
fn saved_vertex_expressions_are_recomputed_as_multi_drivers() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex, "1+2", "sqrt(16)", 0.0, 0.0,
    )];
    assert_eq!(
        reevaluate_saved_vertex_expressions(&project).unwrap(),
        vec![(vertex, Point2::new(3.0, 4.0))]
    );
}

#[test]
fn saved_expression_duplicates_and_nonfinite_results_fail_closed() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    let valid = VertexCoordinateExpressions::new(vertex, "1", "2", 0.0, 0.0);
    project.numeric_expressions.vertex_coordinates = vec![valid.clone(), valid];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex, "1/0", "2", 0.0, 0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn saved_expression_dependency_names_and_shared_vertex_cycles_fail_closed() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex,
        "vertex_x+1",
        "2",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    let binding = VertexCoordinateExpressions::new(vertex, "1", "2", 0.0, 0.0);
    project.numeric_expressions.vertex_coordinates = vec![binding.clone(), binding];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

fn vertex_reference(id: VertexId, axis: char) -> String {
    let id = serde_json::to_value(id)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    format!("v.{id}.{axis}")
}

fn edge_reference(id: EdgeId, field: &str) -> String {
    let id = serde_json::to_value(id)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    format!("e.{id}.{field}")
}

static VERTEX_REFERENCE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn saved_vertex_reference_dag_is_evaluated_topologically() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "2", "3", 0.0, 0.0),
        VertexCoordinateExpressions::new(
            second,
            format!("{}+4", vertex_reference(first, 'x')),
            format!("{}*2", vertex_reference(first, 'y')),
            0.0,
            0.0,
        ),
    ];
    assert_eq!(
        reevaluate_saved_vertex_expressions(&project).unwrap(),
        vec![
            (first, Point2::new(2.0, 3.0)),
            (second, Point2::new(6.0, 6.0)),
        ]
    );
}

#[test]
fn saved_vertex_reference_self_cycle_and_dangling_fail_closed() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex,
        vertex_reference(vertex, 'x'),
        "0",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex,
        vertex_reference(VertexId::new(), 'x'),
        "0",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn vertex_reference_requires_lowercase_canonical_uuid_and_allows_equal_values() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "2", "2", 2.0, 2.0),
        VertexCoordinateExpressions::new(second, "2", "2", 2.0, 2.0),
    ];
    reevaluate_saved_vertex_expressions(&project).expect("distinct bindings may share values");
    project.numeric_expressions.vertex_coordinates[1].x_source =
        vertex_reference(first, 'x').to_uppercase();
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

fn dependency_chain(project: &mut ProjectState, count: usize) {
    let ids = (1..=count)
        .map(|index| {
            serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
                .expect("fixed dependency vertex ID")
        })
        .collect::<Vec<_>>();
    project.numeric_expressions.vertex_coordinates = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let source = ids
                .get(index + 1)
                .map_or_else(|| "1".to_owned(), |next| vertex_reference(*next, 'x'));
            VertexCoordinateExpressions::new(*id, source, "0", 0.0, 0.0)
        })
        .collect();
}

#[test]
fn vertex_reference_depth_64_is_allowed_and_65_is_rejected() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, _, _) = solver_stage_fixture();
    dependency_chain(&mut project, 65);
    assert!(reevaluate_saved_vertex_expressions(&project).is_ok());
    dependency_chain(&mut project, 66);
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn vertex_reference_4096_boundary_is_bounded_and_4097_is_rejected() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, vertex, _) = solver_stage_fixture();
    let reference = vertex_reference(vertex, 'x');
    let source = std::iter::repeat_n(reference.as_str(), 4_096)
        .collect::<Vec<_>>()
        .join("+");
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let mut work = 0;
    let started = std::time::Instant::now();
    assert!(
        expand_saved_vertex_references(&project, &source, &mut memo, &mut visiting, &mut work, 0,)
            .is_ok()
    );
    assert_eq!(work, 4_096);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the maximum-size reference graph must remain bounded on loaded CI hosts"
    );
    let too_many = format!("{source}+{reference}");
    assert!(
        expand_saved_vertex_references(
            &project,
            &too_many,
            &mut HashMap::new(),
            &mut HashSet::new(),
            &mut 0,
            0,
        )
        .is_err()
    );
}

#[test]
fn referenced_expression_still_obeys_numeric_operation_limit() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    let oversized = std::iter::repeat_n("1", 20_000)
        .collect::<Vec<_>>()
        .join("+");
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, oversized, "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(second, vertex_reference(first, 'x'), "0", 0.0, 0.0),
    ];
    let started = std::time::Instant::now();
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn saved_edge_length_and_angle_follow_endpoint_dag() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, start, _) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    let derived = VertexId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: derived,
        position: Point2::new(0.0, 0.0),
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(edge.end, "3", "4", 3.0, 4.0),
        VertexCoordinateExpressions::new(
            derived,
            edge_reference(edge.id, "length"),
            edge_reference(edge.id, "angle"),
            5.0,
            53.13010235415598,
        ),
    ];
    let values =
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).unwrap();
    let point = values
        .iter()
        .find(|(vertex, _)| *vertex == derived)
        .unwrap()
        .1;
    assert_eq!(point.x, 5.0);
    assert!((point.y - 53.13010235415598).abs() <= 1e-12);
}

#[test]
fn saved_edge_reference_cycle_and_dangling_fail_closed() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, _, _) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        edge.end,
        edge_reference(edge.id, "length"),
        "0",
        0.0,
        0.0,
    )];
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).is_err()
    );
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        edge.end,
        edge_reference(EdgeId::new(), "length"),
        "0",
        0.0,
        0.0,
    )];
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).is_err()
    );
}

#[test]
fn edge_angle_reversal_and_zero_boundary_are_canonical() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, start, _) = solver_stage_fixture();
    let original = project.editor.pattern().edges[0].clone();
    let reverse = EdgeId::new();
    let derived = VertexId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.edges.push(ori_domain::Edge {
        id: reverse,
        start: original.end,
        end: original.start,
        kind: EdgeKind::Auxiliary,
    });
    pattern.vertices.push(ori_domain::Vertex {
        id: derived,
        position: Point2::new(0.0, 0.0),
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(original.end, "5", "0", 5.0, 0.0),
        VertexCoordinateExpressions::new(
            derived,
            edge_reference(original.id, "angle"),
            edge_reference(reverse, "angle"),
            0.0,
            180.0,
        ),
    ];
    let values =
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).unwrap();
    let angle = values.iter().find(|(id, _)| *id == derived).unwrap().1;
    assert_eq!(angle, Point2::new(0.0, 180.0));
}

#[test]
fn zero_length_edge_reference_fails_closed() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, start, _) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    let derived = VertexId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: derived,
        position: Point2::new(0.0, 0.0),
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "1", "1", 0.0, 0.0),
        VertexCoordinateExpressions::new(edge.end, "1", "1", 0.0, 0.0),
        VertexCoordinateExpressions::new(derived, edge_reference(edge.id, "length"), "0", 0.0, 0.0),
    ];
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).is_err()
    );
}

#[test]
fn shared_edge_chain_is_memoized_and_indirect_cycle_is_rejected() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, first, _) = solver_stage_fixture();
    let first_edge = project.editor.pattern().edges[0].clone();
    let third = VertexId::new();
    let second_edge = EdgeId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: third,
        position: Point2::new(0.0, 0.0),
    });
    pattern.edges.push(ori_domain::Edge {
        id: second_edge,
        start: first_edge.end,
        end: third,
        kind: EdgeKind::Auxiliary,
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(first_edge.end, "3", "0", 3.0, 0.0),
        VertexCoordinateExpressions::new(
            third,
            format!("{}+4", edge_reference(first_edge.id, "length")),
            "0",
            7.0,
            0.0,
        ),
    ];
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).is_ok()
    );
    project.numeric_expressions.vertex_coordinates[1].x_source =
        edge_reference(second_edge, "length");
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, true).is_err()
    );
}

#[test]
fn referenced_expression_round_trip_detects_saved_value_tampering() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "2", "3", 2.0, 3.0),
        VertexCoordinateExpressions::new(
            second,
            vertex_reference(first, 'x'),
            vertex_reference(first, 'y'),
            2.0,
            3.0,
        ),
    ];
    let mut document = project.document();
    for binding in &document.numeric_expressions.vertex_coordinates {
        let vertex = document
            .crease_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == binding.vertex)
            .unwrap();
        vertex.position = Point2::new(binding.adopted_x_mm, binding.adopted_y_mm);
    }
    assert!(validate_loaded_numeric_expression_bindings(&document).is_ok());
    document.numeric_expressions.vertex_coordinates[1].adopted_x_mm = 9.0;
    assert!(validate_loaded_numeric_expression_bindings(&document).is_err());
}

#[test]
fn ten_thousand_saved_expressions_are_rejected_before_evaluation_within_bound() {
    let (mut project, _, _, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = (0..10_000)
        .map(|_| VertexCoordinateExpressions::new(VertexId::new(), "1", "2", 1.0, 2.0))
        .collect();
    let started = std::time::Instant::now();
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    assert!(started.elapsed() < std::time::Duration::from_millis(100));
}

#[test]
fn expression_reexecution_after_undo_redo_uses_the_restored_binding() {
    let (mut project, mut stage, vertex, _) = solver_stage_fixture();
    let dependent = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(vertex, "2", "3", 0.0, 0.0),
        VertexCoordinateExpressions::new(
            dependent,
            format!("{}+1", vertex_reference(vertex, 'x')),
            format!("{}+1", vertex_reference(vertex, 'y')),
            0.0,
            0.0,
        ),
    ];
    stage.positions.push((dependent, Point2::new(3.0, 4.0)));
    stage.expression_bindings = Some(
        project
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .cloned()
            .zip([Point2::new(2.0, 3.0), Point2::new(3.0, 4.0)])
            .map(|(mut binding, point)| {
                binding.adopted_x_mm = point.x;
                binding.adopted_y_mm = point.y;
                binding
            })
            .collect(),
    );
    apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        stage.project_instance_id,
        stage.project_id,
        0,
        stage.token,
    )
    .unwrap();
    execute_undo(&mut project, stage.project_id, 1).unwrap();
    execute_redo(&mut project, stage.project_id, 2).unwrap();
    let mut actual = reevaluate_saved_vertex_expressions(&project).unwrap();
    actual.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
    let mut expected = vec![
        (vertex, Point2::new(2.0, 3.0)),
        (dependent, Point2::new(3.0, 4.0)),
    ];
    expected.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
    assert_eq!(actual, expected);
}

#[test]
fn expression_reexecution_survives_project_document_round_trip() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex, "6/2", "sqrt(16)", 3.0, 4.0,
    )];
    let reopened = ProjectState::from_valid_document(
        project.document(),
        PathBuf::from("expression-round-trip.ori2"),
    );
    assert_eq!(
        reevaluate_saved_vertex_expressions(&reopened).unwrap(),
        vec![(vertex, Point2::new(3.0, 4.0))]
    );
}

#[test]
fn saved_expression_constraint_conflict_does_not_mutate_project() {
    let (mut project, _, start, original) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddGeometricConstraint {
            record: GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: edge.id,
                    length_mm: 1.0,
                },
            },
        },
    )
    .unwrap();
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(edge.end, "2", "0", 2.0, 0.0),
    ];
    let drivers = reevaluate_saved_vertex_expressions(&project).unwrap();
    assert!(
        solve_geometric_constraints_with_drivers_v1(
            project.editor.pattern(),
            project.editor.geometric_constraints(),
            &drivers,
            ConstraintSolveLimitsV1::default(),
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 1);
    assert_eq!(solver_vertex_position(&project, start), original);
}
