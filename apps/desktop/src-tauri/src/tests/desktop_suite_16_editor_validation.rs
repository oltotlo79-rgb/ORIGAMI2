#[test]
fn face_vertex_batch_is_one_persisted_undo_redo_entry() {
    let first = VertexId::new();
    let second = VertexId::new();
    let edge = EdgeId::new();
    let mut project = ProjectState::new_unsaved(
        "face batch".to_owned(),
        CreasePattern {
            vertices: vec![
                ori_domain::Vertex {
                    id: first,
                    position: Point2::new(1.0, 2.0),
                },
                ori_domain::Vertex {
                    id: second,
                    position: Point2::new(3.0, 4.0),
                },
            ],
            edges: vec![ori_domain::Edge {
                id: edge,
                start: first,
                end: second,
                kind: EdgeKind::Mountain,
            }],
        },
        Paper::default(),
    );
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::MoveVertices {
            updates: vec![
                VertexPositionUpdate {
                    vertex: first,
                    position: Point2::new(11.0, 12.0),
                },
                VertexPositionUpdate {
                    vertex: second,
                    position: Point2::new(13.0, 14.0),
                },
            ],
        },
    )
    .expect("move face vertices");
    let archive = project
        .project_archive()
        .expect("persist face move history");
    let mut reopened =
        ProjectState::from_project_archive(archive, PathBuf::from("face-batch.ori2"))
            .expect("restore face move history");
    assert_eq!(
        reopened.editor.pattern().vertices[0].position,
        Point2::new(11.0, 12.0)
    );
    assert_eq!(
        reopened.editor.pattern().vertices[1].position,
        Point2::new(13.0, 14.0)
    );
    let reopened_project_id = reopened.project_id;
    let undo_revision = reopened.editor.revision();
    execute_undo(&mut reopened, reopened_project_id, undo_revision)
        .expect("undo the face move as one entry");
    assert_eq!(
        reopened.editor.pattern().vertices[0].position,
        Point2::new(1.0, 2.0)
    );
    assert_eq!(
        reopened.editor.pattern().vertices[1].position,
        Point2::new(3.0, 4.0)
    );
    let redo_revision = reopened.editor.revision();
    execute_redo(&mut reopened, reopened_project_id, redo_revision)
        .expect("redo the face move as one entry");
    assert_eq!(
        reopened.editor.pattern().vertices[0].position,
        Point2::new(11.0, 12.0)
    );
    assert_eq!(
        reopened.editor.pattern().vertices[1].position,
        Point2::new(13.0, 14.0)
    );
}

#[test]
fn initial_project_is_a_clean_square_sheet() {
    let project = initial_project_state();
    let snapshot = snapshot(&project);

    assert!(!snapshot.is_dirty);
    assert_eq!(snapshot.revision, 0);
    assert_eq!(project.editor.paper().boundary_vertices.len(), 4);
    assert_eq!(snapshot.crease_pattern.vertices.len(), 4);
    assert_eq!(snapshot.crease_pattern.edges.len(), 4);
    assert!(
        snapshot
            .crease_pattern
            .edges
            .iter()
            .all(|edge| edge.kind == EdgeKind::Boundary)
    );
}

#[test]
fn remove_edge_then_vertex_returns_each_current_snapshot() {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![
            Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(1.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Mountain,
        }],
    });
    let project_id = project.project_id;

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::RemoveEdge { id: edge },
    )
    .expect("remove edge");
    assert_eq!(response.revision, 1);
    assert!(response.crease_pattern.edges.is_empty());

    let response = execute_command(
        &mut project,
        project_id,
        1,
        Command::RemoveVertex { id: start },
    )
    .expect("remove vertex");
    assert_eq!(response.revision, 2);
    assert_eq!(response.crease_pattern.vertices.len(), 1);
    assert_eq!(response.crease_pattern.vertices[0].id, end);
}

#[test]
fn edit_commands_preserve_revision_conflict_errors() {
    let id = VertexId::new();
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![Vertex {
            id,
            position: Point2::new(0.0, 0.0),
        }],
        edges: Vec::new(),
    });
    let project_id = project.project_id;

    let error = execute_command(&mut project, project_id, 4, Command::RemoveVertex { id })
        .expect_err("stale command must fail");

    assert_eq!(error, "expected revision 4, but the current revision is 0");
    assert_eq!(project.editor.pattern().vertices.len(), 1);
}

#[test]
fn validation_snapshot_identifies_both_crossing_edges() {
    let vertices = [
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 0.0),
        },
    ];
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    let project = ProjectState::new(CreasePattern {
        vertices: vertices.to_vec(),
        edges: vec![
            Edge {
                id: first_edge,
                start: vertices[0].id,
                end: vertices[1].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: second_edge,
                start: vertices[2].id,
                end: vertices[3].id,
                kind: EdgeKind::Valley,
            },
        ],
    });

    let response = validation_snapshot(&project);

    assert!(!response.is_valid);
    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.revision, 0);
    assert_eq!(response.issues.len(), 2);
    let crossing = response
        .issues
        .iter()
        .find(|issue| issue.code == "unsplit_intersection")
        .expect("crease-pattern issue");
    assert_eq!(crossing.edges, vec![first_edge, second_edge]);
    assert!(
        response
            .issues
            .iter()
            .any(|issue| issue.code == "too_few_boundary_vertices")
    );
}

#[test]
fn valid_initial_sheet_has_no_combined_validation_issues() {
    let project = initial_project_state();

    let response = validation_snapshot(&project);

    assert!(response.is_valid);
    assert!(response.issues.is_empty());
}

#[test]
fn initial_sheet_reports_boundary_vertices_as_locally_not_applicable() {
    let project = initial_project_state();

    let response = validation_snapshot(&project);
    let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
    let local = &encoded["local_flat_foldability"];

    assert_eq!(local["model"], "interior_single_vertex_zero_thickness_v1");
    assert_eq!(local["status"], "not_applicable");
    assert_eq!(local["total_vertices"], 4);
    assert_eq!(local["applicable_vertices"], 0);
    assert_eq!(local["not_applicable_vertices"], 4);
    for vertex in local["vertices"].as_array().expect("vertex reports") {
        assert_eq!(vertex["verdict"], "not_applicable");
        assert_eq!(vertex["reason"], "paper_boundary");
        assert_eq!(vertex["kawasaki"], "not_applicable");
        assert_eq!(vertex["maekawa"], "not_applicable");
    }
}

#[test]
fn cardinal_mmmv_vertex_reports_both_local_conditions_satisfied() {
    let (project, center) = four_ray_square_project_state(
        [3, 5, 7, 1],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
        ],
    );

    let response = validation_snapshot(&project);
    let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
    let center = serde_json::to_value(center).expect("serialize center vertex ID");
    let local = encoded["local_flat_foldability"]
        .as_object()
        .expect("local report object");
    let center_report = local["vertices"]
        .as_array()
        .expect("vertex reports")
        .iter()
        .find(|report| report["vertex"] == center)
        .expect("center report");

    assert_eq!(local["status"], "necessary_conditions_satisfied");
    assert_eq!(local["applicable_vertices"], 1);
    assert_eq!(local["satisfied_vertices"], 1);
    assert_eq!(center_report["fold_degree"], 4);
    assert_eq!(center_report["mountain_count"], 3);
    assert_eq!(center_report["valley_count"], 1);
    assert_eq!(center_report["verdict"], "satisfied");
    assert_eq!(center_report["reason"], serde_json::Value::Null);
    assert_eq!(center_report["kawasaki"], "satisfied");
    assert_eq!(center_report["maekawa"], "satisfied");
}

#[test]
fn local_report_keeps_kawasaki_and_maekawa_violations_independent() {
    let (kawasaki_project, kawasaki_center) = four_ray_square_project_state(
        [3, 5, 7, 0],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
        ],
    );
    let (maekawa_project, maekawa_center) = four_ray_square_project_state(
        [3, 5, 7, 1],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Valley,
        ],
    );

    let kawasaki = validation_snapshot(&kawasaki_project);
    let kawasaki_json = serde_json::to_value(&kawasaki).expect("serialize Kawasaki counterexample");
    let kawasaki_center =
        serde_json::to_value(kawasaki_center).expect("serialize Kawasaki center vertex ID");
    let kawasaki_center_report = kawasaki_json["local_flat_foldability"]["vertices"]
        .as_array()
        .expect("Kawasaki vertex reports")
        .iter()
        .find(|report| report["vertex"] == kawasaki_center)
        .expect("Kawasaki center report");
    assert_eq!(kawasaki_center_report["kawasaki"], "violated");
    assert_eq!(kawasaki_center_report["maekawa"], "satisfied");
    assert_eq!(kawasaki_center_report["verdict"], "violated");

    let maekawa = validation_snapshot(&maekawa_project);
    let maekawa_json = serde_json::to_value(&maekawa).expect("serialize Maekawa counterexample");
    let maekawa_center =
        serde_json::to_value(maekawa_center).expect("serialize Maekawa center vertex ID");
    let maekawa_center_report = maekawa_json["local_flat_foldability"]["vertices"]
        .as_array()
        .expect("Maekawa vertex reports")
        .iter()
        .find(|report| report["vertex"] == maekawa_center)
        .expect("Maekawa center report");
    assert_eq!(maekawa_center_report["kawasaki"], "satisfied");
    assert_eq!(maekawa_center_report["maekawa"], "violated");
    assert_eq!(maekawa_center_report["verdict"], "violated");
}

#[test]
fn local_flat_foldability_json_contract_is_exact_and_does_not_change_geometry_validity() {
    let (project, center) = four_ray_square_project_state(
        [3, 5, 7, 1],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Valley,
        ],
    );

    let response = validation_snapshot(&project);
    assert!(response.is_valid);
    assert!(response.issues.is_empty());
    let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
    let center = serde_json::to_value(center).expect("serialize center vertex ID");
    let root_keys = encoded
        .as_object()
        .expect("validation object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let local = encoded["local_flat_foldability"]
        .as_object()
        .expect("local report object");
    let local_keys = local.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let center_report = local["vertices"]
        .as_array()
        .expect("vertex reports")
        .iter()
        .find(|report| report["vertex"] == center)
        .expect("center report")
        .as_object()
        .expect("center report object");
    let center_keys = center_report
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        root_keys,
        [
            "project_id",
            "revision",
            "is_valid",
            "issues",
            "local_flat_foldability"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        local_keys,
        [
            "model",
            "max_exact_fold_degree",
            "status",
            "total_vertices",
            "applicable_vertices",
            "satisfied_vertices",
            "violated_vertices",
            "not_applicable_vertices",
            "indeterminate_vertices",
            "vertices",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        center_keys,
        [
            "vertex",
            "fold_degree",
            "mountain_count",
            "valley_count",
            "verdict",
            "reason",
            "kawasaki",
            "maekawa",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(local["status"], "violated");
    assert_eq!(center_report["kawasaki"], "satisfied");
    assert_eq!(center_report["maekawa"], "violated");
}

#[test]
fn paper_thickness_issues_are_included_without_highlight_targets() {
    let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
    let (pattern, mut paper) = sheet.into_parts();
    paper.thickness_mm = -0.01;
    let project = ProjectState::new_with_paper(pattern.clone(), paper);

    let response = validation_snapshot(&project);

    assert!(!response.is_valid);
    assert_eq!(response.issues.len(), 1);
    assert_eq!(response.issues[0].code, "negative_thickness");
    assert!(response.issues[0].vertices.is_empty());
    assert!(response.issues[0].edges.is_empty());

    let mut zero_paper = project.editor.paper().clone();
    zero_paper.thickness_mm = 0.0;
    let zero_project = ProjectState::new_with_paper(pattern, zero_paper);
    let zero_thickness = validation_snapshot(&zero_project);
    assert!(zero_thickness.is_valid);
    assert!(zero_thickness.issues.is_empty());
}

#[test]
fn paper_intersection_maps_boundary_references_to_domain_edges() {
    let vertices = [
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 0.0),
        },
    ];
    let boundary_edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
    let pattern = CreasePattern {
        vertices: vertices.to_vec(),
        edges: vec![
            Edge {
                id: boundary_edges[0],
                start: vertices[0].id,
                end: vertices[1].id,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: boundary_edges[1],
                start: vertices[1].id,
                end: vertices[2].id,
                kind: EdgeKind::Boundary,
            },
            // Domain edges are undirected for boundary highlighting, so
            // mapping also accepts the reverse of the paper's order.
            Edge {
                id: boundary_edges[2],
                start: vertices[3].id,
                end: vertices[2].id,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: boundary_edges[3],
                start: vertices[3].id,
                end: vertices[0].id,
                kind: EdgeKind::Boundary,
            },
        ],
    };
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        ..Paper::default()
    };
    let project = ProjectState::new_with_paper(pattern, paper);

    let response = validation_snapshot(&project);
    let intersection = response
        .issues
        .iter()
        .find(|issue| issue.code == "boundary_self_intersection")
        .expect("paper self-intersection issue");

    assert_eq!(
        intersection.vertices,
        vec![
            vertices[0].id,
            vertices[1].id,
            vertices[2].id,
            vertices[3].id
        ]
    );
    assert_eq!(
        intersection.edges,
        vec![boundary_edges[0], boundary_edges[2]]
    );
}

#[test]
fn paper_boundary_topology_issues_include_actionable_targets() {
    let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
    let (mut pattern, paper) = sheet.into_parts();
    let boundary = paper.boundary_vertices.clone();

    pattern.edges[0].kind = EdgeKind::Mountain;
    let first_duplicate = pattern.edges[1].id;
    let duplicate_edge = Edge {
        id: EdgeId::new(),
        start: pattern.edges[1].end,
        end: pattern.edges[1].start,
        kind: EdgeKind::Boundary,
    };
    let duplicate = duplicate_edge.id;
    pattern.edges.push(duplicate_edge);
    let unexpected_edge = Edge {
        id: EdgeId::new(),
        start: boundary[0],
        end: boundary[2],
        kind: EdgeKind::Boundary,
    };
    let unexpected = unexpected_edge.id;
    pattern.edges.push(unexpected_edge);
    let project = ProjectState::new_with_paper(pattern, paper);

    let response = validation_snapshot(&project);
    let missing = response
        .issues
        .iter()
        .find(|issue| issue.code == "missing_boundary_edge")
        .expect("wrong-kind edge is missing from the Boundary set");
    assert_eq!(missing.vertices, vec![boundary[0], boundary[1]]);
    assert!(missing.edges.is_empty());

    let duplicate_issue = response
        .issues
        .iter()
        .find(|issue| issue.code == "duplicate_boundary_edge")
        .expect("duplicate Boundary record");
    assert_eq!(duplicate_issue.vertices, vec![boundary[1], boundary[2]]);
    assert_eq!(duplicate_issue.edges, vec![first_duplicate, duplicate]);

    let unexpected_issue = response
        .issues
        .iter()
        .find(|issue| issue.code == "unexpected_boundary_edge")
        .expect("unexpected Boundary chord");
    assert_eq!(unexpected_issue.vertices, vec![boundary[0], boundary[2]]);
    assert_eq!(unexpected_issue.edges, vec![unexpected]);
}
