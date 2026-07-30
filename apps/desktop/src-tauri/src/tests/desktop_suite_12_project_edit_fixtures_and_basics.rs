fn file_document(name: &str, x: f64) -> ProjectDocument {
    ProjectDocument::new(
        name,
        CreasePattern {
            vertices: vec![Vertex {
                id: VertexId::new(),
                position: Point2::new(x, 5.0),
            }],
            edges: Vec::new(),
        },
    )
}

fn crossing_project() -> (ProjectState, Edge, Edge) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let ids = [
        VertexId::new(),
        VertexId::new(),
        VertexId::new(),
        VertexId::new(),
    ];
    pattern.vertices.extend([
        Vertex {
            id: ids[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: ids[1],
            position: Point2::new(80.0, 80.0),
        },
        Vertex {
            id: ids[2],
            position: Point2::new(20.0, 80.0),
        },
        Vertex {
            id: ids[3],
            position: Point2::new(80.0, 20.0),
        },
    ]);
    let first = Edge {
        id: EdgeId::new(),
        start: ids[0],
        end: ids[1],
        kind: EdgeKind::Mountain,
    };
    let second = Edge {
        id: EdgeId::new(),
        start: ids[2],
        end: ids[3],
        kind: EdgeKind::Valley,
    };
    pattern.edges.extend([first.clone(), second.clone()]);
    (ProjectState::new_with_paper(pattern, paper), first, second)
}

fn t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let interior_start = VertexId::new();
    let interior_end = VertexId::new();
    let stem_other = VertexId::new();
    let junction = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: interior_start,
            position: Point2::new(10.0, 40.0),
        },
        Vertex {
            id: interior_end,
            position: Point2::new(90.0, 40.0),
        },
        Vertex {
            id: stem_other,
            position: Point2::new(34.0, 10.0),
        },
        Vertex {
            id: junction,
            position: Point2::new(34.0, 40.0),
        },
    ]);
    let interior = Edge {
        id: EdgeId::new(),
        start: interior_start,
        end: interior_end,
        kind: EdgeKind::Mountain,
    };
    let stem = Edge {
        id: EdgeId::new(),
        start: stem_other,
        end: junction,
        kind: EdgeKind::Valley,
    };
    pattern.edges.extend([interior.clone(), stem.clone()]);
    (
        ProjectState::new_with_paper(pattern, paper),
        interior,
        stem,
        junction,
    )
}

fn boundary_t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let boundary = pattern.edges[0].clone();
    let junction = VertexId::new();
    let stem_other = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: junction,
            position: Point2::new(40.0, 0.0),
        },
        Vertex {
            id: stem_other,
            position: Point2::new(40.0, 30.0),
        },
    ]);
    let stem = Edge {
        id: EdgeId::new(),
        start: stem_other,
        end: junction,
        kind: EdgeKind::Mountain,
    };
    pattern.edges.push(stem.clone());
    (
        ProjectState::new_with_paper(pattern, paper),
        boundary,
        stem,
        junction,
    )
}

fn append_cluster_test_edge(
    pattern: &mut CreasePattern,
    start_position: Point2,
    end_position: Point2,
    kind: EdgeKind,
) -> Edge {
    let start = VertexId::new();
    let end = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: start,
            position: start_position,
        },
        Vertex {
            id: end,
            position: end_position,
        },
    ]);
    let edge = Edge {
        id: EdgeId::new(),
        start,
        end,
        kind,
    };
    pattern.edges.push(edge.clone());
    edge
}

fn create_cluster_project(include_omitted_edge: bool) -> (ProjectState, Vec<Edge>) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let mut edges = vec![
        append_cluster_test_edge(
            &mut pattern,
            Point2::new(10.0, 50.0),
            Point2::new(90.0, 50.0),
            EdgeKind::Mountain,
        ),
        append_cluster_test_edge(
            &mut pattern,
            Point2::new(50.0, 10.0),
            Point2::new(50.0, 90.0),
            EdgeKind::Valley,
        ),
        append_cluster_test_edge(
            &mut pattern,
            Point2::new(20.0, 20.0),
            Point2::new(80.0, 80.0),
            EdgeKind::Auxiliary,
        ),
    ];
    if include_omitted_edge {
        edges.push(append_cluster_test_edge(
            &mut pattern,
            Point2::new(20.0, 80.0),
            Point2::new(80.0, 20.0),
            EdgeKind::Mountain,
        ));
    }
    (ProjectState::new_with_paper(pattern, paper), edges)
}

fn maximum_cluster_project() -> (ProjectState, Vec<Edge>) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let mut edges = Vec::with_capacity(MAX_INTERSECTION_CLUSTER_TARGETS);
    for index in 0..MAX_INTERSECTION_CLUSTER_TARGETS {
        let offset = index as f64 - 32.0;
        let edge = append_cluster_test_edge(
            &mut pattern,
            Point2::new(10.0, 50.0 - offset),
            Point2::new(90.0, 50.0 + offset),
            match index % 4 {
                0 => EdgeKind::Mountain,
                1 => EdgeKind::Valley,
                2 => EdgeKind::Auxiliary,
                _ => EdgeKind::Cut,
            },
        );
        edges.push(edge);
    }
    (ProjectState::new_with_paper(pattern, paper), edges)
}

fn reuse_cluster_project() -> (ProjectState, [Edge; 3], VertexId) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let horizontal = append_cluster_test_edge(
        &mut pattern,
        Point2::new(10.0, 50.0),
        Point2::new(90.0, 50.0),
        EdgeKind::Mountain,
    );
    let vertical = append_cluster_test_edge(
        &mut pattern,
        Point2::new(50.0, 10.0),
        Point2::new(50.0, 90.0),
        EdgeKind::Valley,
    );
    let junction = VertexId::new();
    let stem_start = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: stem_start,
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: junction,
            position: Point2::new(50.0, 50.0),
        },
    ]);
    let stem = Edge {
        id: EdgeId::new(),
        start: stem_start,
        end: junction,
        kind: EdgeKind::Auxiliary,
    };
    pattern.edges.push(stem.clone());
    (
        ProjectState::new_with_paper(pattern, paper),
        [horizontal, vertical, stem],
        junction,
    )
}

#[test]
fn benchmark_pattern_response_contains_stable_renderable_geometry() {
    let response = generate_benchmark_pattern(4);

    assert_eq!(response.requested_edge_count, 4);
    assert_eq!(response.vertex_count, 4);
    assert_eq!(response.edge_count, 4);
    assert_eq!(
        response.vertices,
        vec![
            BenchmarkVertex {
                id: "benchmark-v-0".to_owned(),
                position: Point2::new(0.0, 0.0),
            },
            BenchmarkVertex {
                id: "benchmark-v-1".to_owned(),
                position: Point2::new(1.0, 0.0),
            },
            BenchmarkVertex {
                id: "benchmark-v-2".to_owned(),
                position: Point2::new(0.0, 1.0),
            },
            BenchmarkVertex {
                id: "benchmark-v-3".to_owned(),
                position: Point2::new(1.0, 1.0),
            },
        ]
    );
    assert_eq!(
        response.edges,
        vec![
            BenchmarkEdge {
                id: "benchmark-e-0".to_owned(),
                start: "benchmark-v-0".to_owned(),
                end: "benchmark-v-1".to_owned(),
                kind: EdgeKind::Mountain,
            },
            BenchmarkEdge {
                id: "benchmark-e-1".to_owned(),
                start: "benchmark-v-0".to_owned(),
                end: "benchmark-v-2".to_owned(),
                kind: EdgeKind::Valley,
            },
            BenchmarkEdge {
                id: "benchmark-e-2".to_owned(),
                start: "benchmark-v-1".to_owned(),
                end: "benchmark-v-3".to_owned(),
                kind: EdgeKind::Mountain,
            },
            BenchmarkEdge {
                id: "benchmark-e-3".to_owned(),
                start: "benchmark-v-2".to_owned(),
                end: "benchmark-v-3".to_owned(),
                kind: EdgeKind::Valley,
            },
        ]
    );
    assert_eq!(generate_benchmark_pattern(4), response);
}

#[test]
fn benchmark_pattern_response_has_all_ten_thousand_edges_and_valid_references() {
    let response = generate_benchmark_pattern(10_000);

    assert_eq!(response.requested_edge_count, 10_000);
    assert_eq!(response.vertex_count, 5_184);
    assert_eq!(response.edge_count, 10_000);
    let vertex_ids = response
        .vertices
        .iter()
        .map(|vertex| vertex.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(response.edges.iter().all(|edge| {
        vertex_ids.contains(edge.start.as_str()) && vertex_ids.contains(edge.end.as_str())
    }));
}

#[test]
fn benchmark_pattern_response_is_empty_for_zero_edges() {
    let response = generate_benchmark_pattern(0);

    assert_eq!(response.requested_edge_count, 0);
    assert_eq!(response.vertex_count, 0);
    assert_eq!(response.edge_count, 0);
    assert!(response.vertices.is_empty());
    assert!(response.edges.is_empty());
}

#[test]
fn project_name_is_trimmed_and_validated_by_unicode_character_count() {
    assert_eq!(normalize_project_name("  Crane  "), Ok("Crane".to_owned()));
    assert_eq!(
        normalize_project_name("\n  Crane  \t"),
        Ok("Crane".to_owned())
    );
    assert!(normalize_project_name("").is_err());
    assert!(normalize_project_name(" \t\n ").is_err());
    assert!(normalize_project_name("Crane\0draft").is_err());

    let maximum = "鶴".repeat(MAX_PROJECT_NAME_CHARS);
    assert_eq!(normalize_project_name(&maximum), Ok(maximum.clone()));
    assert!(normalize_project_name(&format!("{maximum}鶴")).is_err());
}

#[test]
fn paper_thickness_accepts_zero_and_rejects_negative_or_non_finite_values() {
    assert_eq!(validate_paper_thickness(0.0), Ok(()));
    assert_eq!(validate_paper_thickness(-0.0), Ok(()));
    for invalid in [-f64::MIN_POSITIVE, -1.0, f64::NAN, f64::INFINITY] {
        assert!(validate_paper_thickness(invalid).is_err());
    }
}

#[test]
fn new_project_state_has_requested_paper_and_no_saved_baseline() {
    let parameters = new_project_parameters();
    let expected_front = parameters.front_color;
    let expected_back = parameters.back_color;

    let project = create_new_project_state(parameters).expect("valid new project");
    let response = snapshot(&project);

    assert_eq!(project.name, "Test sheet");
    assert!(project.current_path.is_none());
    assert!(project.saved_revision.is_none());
    assert!(project.saved_document.is_none());
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(project.editor.cutting_allowed());
    assert!(project.is_dirty());
    assert_eq!(project.editor.paper().thickness_mm, 0.2);
    assert_eq!(project.editor.paper().front.color, expected_front);
    assert_eq!(project.editor.paper().back.color, expected_back);
    assert_eq!(project.editor.paper().front.texture_asset, None);
    assert_eq!(project.editor.paper().back.texture_asset, None);
    let creation_expressions = project
        .numeric_expressions
        .rectangular_paper_creation
        .as_ref()
        .expect("new project keeps both creation expressions");
    assert_eq!(creation_expressions.schema_version, 1);
    assert_eq!(creation_expressions.width_source, "210");
    assert_eq!(creation_expressions.height_source, "297");
    assert_eq!(creation_expressions.adopted_width_mm, 210.0);
    assert_eq!(creation_expressions.adopted_height_mm, 297.0);
    assert_eq!(
        response.numeric_expressions, project.numeric_expressions,
        "snapshot and persisted document share the same bounded metadata"
    );
    assert_eq!(
        project.document().numeric_expressions,
        project.numeric_expressions
    );
    assert_eq!(
        project.editor.pattern().vertices[2].position,
        Point2::new(210.0, 297.0)
    );
    assert!(validate_paper(project.editor.paper(), project.editor.pattern()).is_valid());

    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.name, "Test sheet");
    assert!(response.current_path.is_none());
    assert_eq!(response.revision, 0);
    assert!(response.saved_revision.is_none());
    assert!(response.is_dirty);
    assert_eq!(&response.paper, project.editor.paper());
    assert!(response.cutting_allowed);
    assert!(!response.can_undo);
    assert!(!response.can_redo);
}

#[test]
fn loaded_numeric_expressions_are_re_evaluated_against_saved_adopted_values() {
    assert_eq!(
        map_loaded_numeric_expression_error(PositiveMillimetrePairError::WorkerBusy),
        PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE
    );
    let project = create_new_project_state(new_project_parameters()).expect("valid new project");
    let document = project.document();
    validate_loaded_numeric_expression_bindings(&document)
        .expect("untampered expressions remain loadable");

    let mut changed_source = document.clone();
    changed_source
        .numeric_expressions
        .rectangular_paper_creation
        .as_mut()
        .expect("creation expressions")
        .width_source = "211".to_owned();
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&changed_source),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut changed_value = document.clone();
    changed_value
        .numeric_expressions
        .rectangular_paper_creation
        .as_mut()
        .expect("creation expressions")
        .adopted_height_mm = 298.0;
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&changed_value),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut legacy = document;
    legacy.numeric_expressions = ProjectNumericExpressions::default();
    validate_loaded_numeric_expression_bindings(&legacy)
        .expect("legacy projects without expressions migrate safely");
}

#[test]
fn polar_expression_loading_is_legacy_compatible_and_v2_bit_exact() {
    let mut legacy = initial_project_state().document();
    let start = legacy.crease_pattern.vertices[0].clone();
    let target = legacy.crease_pattern.vertices[1].id;
    let legacy_endpoint = Point2::new(12.25, -7.5);
    legacy
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == target)
        .expect("target vertex")
        .position = legacy_endpoint;
    let mut legacy_binding = VertexCoordinateExpressions::new(
        target,
        legacy_endpoint.x.to_string(),
        legacy_endpoint.y.to_string(),
        legacy_endpoint.x,
        legacy_endpoint.y,
    );
    legacy_binding.polar_construction = Some(PolarVertexConstructionExpressions {
        schema_version: ori_formats::PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION,
        start_vertex: start.id,
        adopted_start_x_mm: start.position.x,
        adopted_start_y_mm: start.position.y,
        length_source: "8".to_owned(),
        angle_degrees_source: "37.5".to_owned(),
        adopted_length_mm: 8.0,
        adopted_angle_degrees: 37.5,
    });
    legacy.numeric_expressions.vertex_coordinates = vec![legacy_binding];
    validate_loaded_numeric_expression_bindings(&legacy)
        .expect("legacy creator-runtime coordinates remain loadable without trig replay");

    let mut forged_legacy_start = legacy.clone();
    forged_legacy_start.numeric_expressions.vertex_coordinates[0]
        .polar_construction
        .as_mut()
        .expect("legacy polar metadata")
        .adopted_start_x_mm += 1.0;
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&forged_legacy_start),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut deterministic = legacy;
    let endpoint =
        ori_numeric::deterministic_polar_endpoint_v2(start.position.x, start.position.y, 8.0, 37.5)
            .map(|(x, y)| Point2::new(x, y))
            .expect("deterministic endpoint");
    assert_eq!(endpoint.x.to_bits(), 4_618_831_910_042_805_394);
    deterministic
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == target)
        .expect("target vertex")
        .position = endpoint;
    let binding = &mut deterministic.numeric_expressions.vertex_coordinates[0];
    binding.x_source =
        super::numeric_expression::canonical_binary64_expression_literal_v1(endpoint.x)
            .expect("finite deterministic x");
    binding.y_source =
        super::numeric_expression::canonical_binary64_expression_literal_v1(endpoint.y)
            .expect("finite deterministic y");
    binding.adopted_x_mm = endpoint.x;
    binding.adopted_y_mm = endpoint.y;
    binding.schema_version =
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2;
    binding.transcendental_model_id =
        Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.to_owned());
    let staged = ProjectState::from_document(deterministic.clone(), PathBuf::new())
        .expect("the deterministic fixture is a valid project");
    let reevaluated =
        reevaluate_saved_vertex_expressions_for_archive_load_with_model_support(&staged, true)
            .expect("the deterministic coordinate expressions re-evaluate");
    assert_eq!(reevaluated.len(), 1);
    assert_eq!(reevaluated[0].1.x.to_bits(), endpoint.x.to_bits());
    assert_eq!(reevaluated[0].1.y.to_bits(), endpoint.y.to_bits());
    validate_loaded_numeric_expression_bindings_with_model_support(&deterministic, true)
        .expect("the frozen V2 deterministic endpoint is bit-exact");
    if !ori_numeric::deterministic_transcendental_model_supported_v1() {
        assert_eq!(
            validate_loaded_numeric_expression_bindings(&deterministic),
            Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned()),
            "V2 deterministic archives fail closed on targets outside the frozen replay matrix"
        );
        return;
    }
    validate_loaded_numeric_expression_bindings(&deterministic)
        .expect("the supported target accepts the V2 deterministic endpoint");

    let mut forged_endpoint = deterministic.clone();
    let forged_x = {
        let binding = &mut forged_endpoint.numeric_expressions.vertex_coordinates[0];
        binding.adopted_x_mm = f64::from_bits(binding.adopted_x_mm.to_bits() + 1);
        binding.x_source = super::numeric_expression::canonical_binary64_expression_literal_v1(
            binding.adopted_x_mm,
        )
        .expect("finite forged endpoint");
        binding.adopted_x_mm
    };
    forged_endpoint
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == target)
        .expect("target vertex")
        .position
        .x = forged_x;
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&forged_endpoint),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut forged_model = deterministic;
    forged_model.numeric_expressions.vertex_coordinates[0].transcendental_model_id =
        Some("forged_model".to_owned());
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&forged_model),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
}
