#[test]
fn edge_split_conflicts_invalid_fractions_and_boundary_targets_preserve_project_state() {
    let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
    let (mut pattern, paper) = sheet.into_parts();
    let boundary_edge = pattern.edges[0].id;
    let crease = Edge {
        id: EdgeId::new(),
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    };
    pattern.edges.push(crease.clone());
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let conflict = execute_edge_split(&mut project, project_id, 1, crease.id, 0.5)
        .expect_err("stale split must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let invalid = execute_edge_split(&mut project, project_id, 0, crease.id, f64::NAN)
        .expect_err("non-finite split must fail");
    assert_eq!(invalid, "edge split fraction must be finite");
    assert_eq!(project_state_signature(&project), before);

    let boundary = execute_edge_split(&mut project, project_id, 0, boundary_edge, 0.5)
        .expect_err("boundary split must use the sheet command");
    assert!(boundary.contains("must be changed through a sheet-boundary operation"));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn edge_intersection_connection_returns_vertex_and_exact_undoable_snapshot() {
    let (mut project, first, second) = crossing_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = original_document
        .crease_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();

    let response =
        execute_edge_intersection_connection(&mut project, project_id, 0, second.id, first.id)
            .expect("connect crossing edges");

    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    let created_vertex = response
        .snapshot
        .crease_pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == response.vertex_id)
        .expect("explicitly returned generated vertex");
    assert_eq!(created_vertex.position, Point2::new(50.0, 50.0));
    assert!(!original_vertex_ids.contains(&response.vertex_id));
    let generated_edges = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .filter(|edge| !original_edge_ids.contains(&edge.id))
        .collect::<Vec<_>>();
    assert_eq!(generated_edges.len(), 2);
    assert!(
        generated_edges
            .iter()
            .all(|edge| edge.start == response.vertex_id)
    );
    assert_eq!(
        generated_edges
            .iter()
            .map(|edge| edge.kind)
            .collect::<Vec<_>>(),
        vec![EdgeKind::Mountain, EdgeKind::Valley]
    );
    assert_eq!(
        response.snapshot.crease_pattern,
        project.editor.pattern().clone()
    );
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo intersection connection");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo intersection connection");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn edge_intersection_api_rejections_preserve_entire_project_state() {
    let (mut project, first, second) = crossing_project();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let wrong_project = execute_edge_intersection_connection(
        &mut project,
        ProjectId::new(),
        0,
        first.id,
        second.id,
    )
    .expect_err("wrong project must fail");
    assert!(wrong_project.contains("active project changed"));
    assert_eq!(project_state_signature(&project), before);

    let stale =
        execute_edge_intersection_connection(&mut project, project_id, 4, first.id, second.id)
            .expect_err("stale revision must fail");
    assert_eq!(stale, "expected revision 4, but the current revision is 0");
    assert_eq!(project_state_signature(&project), before);

    let same_edge =
        execute_edge_intersection_connection(&mut project, project_id, 0, first.id, first.id)
            .expect_err("same target edge must fail");
    assert_eq!(same_edge, "the two intersection edge IDs must be different");
    assert_eq!(project_state_signature(&project), before);

    let boundary = project.editor.pattern().edges[0].id;
    let boundary_error =
        execute_edge_intersection_connection(&mut project, project_id, 0, boundary, first.id)
            .expect_err("boundary target must fail");
    assert!(boundary_error.contains("must not be a boundary edge"));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn edge_intersection_api_rejects_t_junction_without_mutation() {
    let (project, first, second) = crossing_project();
    let mut document = project.document();
    document
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == second.start)
        .expect("second start")
        .position = Point2::new(50.0, 50.0);
    let mut project = ProjectState::new_with_paper(document.crease_pattern, document.paper);
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let error =
        execute_edge_intersection_connection(&mut project, project_id, 0, first.id, second.id)
            .expect_err("T-junction must fail");

    assert_eq!(
        error,
        "the selected edges must intersect strictly inside both edges"
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn intersection_cluster_api_creates_three_way_junction_with_one_step_history() {
    let (mut project, edges) = create_cluster_project(false);
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = original_document
        .crease_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let targets = edges
        .iter()
        .map(|edge| IntersectionClusterTargetRequest {
            edge_id: edge.id,
            relation: IntersectionClusterRelation::Interior,
        })
        .collect();

    let response =
        execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
            .expect("connect a newly created three-edge intersection cluster");

    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(response.snapshot.paper, original_document.paper);
    assert!(!original_vertex_ids.contains(&response.vertex_id));
    assert_eq!(
        response
            .snapshot
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == response.vertex_id)
            .expect("created cluster junction")
            .position,
        Point2::new(50.0, 50.0)
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_document.crease_pattern.vertices.len() + 1
    );
    assert_eq!(
        response.snapshot.crease_pattern.edges.len(),
        original_document.crease_pattern.edges.len() + edges.len()
    );
    for edge in &edges {
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| candidate.id == edge.id)
            .expect("split original cluster edge");
        assert_eq!(split_original.start, edge.start);
        assert_eq!(split_original.end, response.vertex_id);
        assert_eq!(split_original.kind, edge.kind);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| {
                !original_edge_ids.contains(&candidate.id)
                    && candidate.start == response.vertex_id
                    && candidate.end == edge.end
            })
            .expect("generated cluster edge");
        assert_eq!(generated.kind, edge.kind);
    }
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo created intersection cluster");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo created intersection cluster");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn intersection_cluster_api_accepts_64_targets_and_returns_the_created_junction() {
    let (mut project, edges) = maximum_cluster_project();
    assert_eq!(edges.len(), MAX_INTERSECTION_CLUSTER_TARGETS);
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = original_document
        .crease_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let targets = edges
        .iter()
        .map(|edge| IntersectionClusterTargetRequest {
            edge_id: edge.id,
            relation: IntersectionClusterRelation::Interior,
        })
        .collect();

    let response =
        execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
            .expect("the inclusive 64-target API limit must connect");

    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert!(!original_vertex_ids.contains(&response.vertex_id));
    assert_eq!(
        response
            .snapshot
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == response.vertex_id),
        Some(&Vertex {
            id: response.vertex_id,
            position: Point2::new(50.0, 50.0),
        })
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_document.crease_pattern.vertices.len() + 1
    );
    assert_eq!(
        response.snapshot.crease_pattern.edges.len(),
        original_document.crease_pattern.edges.len() + MAX_INTERSECTION_CLUSTER_TARGETS
    );
    for source in &edges {
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| edge.id == source.id)
            .expect("each maximum-cluster source edge remains");
        assert_eq!(split_original.start, source.start);
        assert_eq!(split_original.end, response.vertex_id);
        assert_eq!(split_original.kind, source.kind);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| {
                !edges.iter().any(|source| source.id == edge.id)
                    && edge.start == response.vertex_id
                    && edge.end == source.end
            })
            .expect("each maximum-cluster source gets one generated half");
        assert_eq!(generated.kind, source.kind);
    }
    assert!(validation_snapshot(&project).is_valid);

    let (mut rejected_project, rejected_edges) = maximum_cluster_project();
    let rejected_project_id = rejected_project.project_id;
    let rejected_before = project_state_signature(&rejected_project);
    let error = execute_intersection_cluster_connection(
        &mut rejected_project,
        rejected_project_id,
        0,
        (0..=MAX_INTERSECTION_CLUSTER_TARGETS)
            .map(|index| IntersectionClusterTargetRequest {
                edge_id: rejected_edges[index % rejected_edges.len()].id,
                relation: IntersectionClusterRelation::Interior,
            })
            .collect(),
        None,
    )
    .expect_err("65 targets must be rejected at the API boundary");
    assert_eq!(
        error,
        "an intersection cluster supports at most 64 target edges, found 65"
    );
    assert_eq!(project_state_signature(&rejected_project), rejected_before);
}

#[test]
fn intersection_cluster_api_reuses_junction_with_interior_and_endpoint_targets() {
    let (mut project, [horizontal, vertical, stem], junction) = reuse_cluster_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let targets = vec![
        IntersectionClusterTargetRequest {
            edge_id: stem.id,
            relation: IntersectionClusterRelation::Endpoint,
        },
        IntersectionClusterTargetRequest {
            edge_id: vertical.id,
            relation: IntersectionClusterRelation::Interior,
        },
        IntersectionClusterTargetRequest {
            edge_id: horizontal.id,
            relation: IntersectionClusterRelation::Interior,
        },
    ];

    let response = execute_intersection_cluster_connection(
        &mut project,
        project_id,
        0,
        targets,
        Some(junction),
    )
    .expect("connect a mixed interior/endpoint cluster at an existing vertex");

    assert_eq!(response.vertex_id, junction);
    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(
        response.snapshot.crease_pattern.vertices,
        original_document.crease_pattern.vertices
    );
    assert_eq!(
        response.snapshot.crease_pattern.edges.len(),
        original_document.crease_pattern.edges.len() + 2
    );
    assert!(
        response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge == &stem)
    );
    for edge in [&horizontal, &vertical] {
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| candidate.id == edge.id)
            .expect("split original cluster edge");
        assert_eq!(split_original.start, edge.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, edge.kind);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| {
                !original_edge_ids.contains(&candidate.id)
                    && candidate.start == junction
                    && candidate.end == edge.end
            })
            .expect("generated cluster edge");
        assert_eq!(generated.kind, edge.kind);
    }
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo reused intersection cluster");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo reused intersection cluster");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn intersection_cluster_api_rejections_are_atomic_and_boundary_remains_unsupported() {
    let interior_target = |edge: &Edge| IntersectionClusterTargetRequest {
        edge_id: edge.id,
        relation: IntersectionClusterRelation::Interior,
    };

    let (mut bounded_project, bounded_edges) = create_cluster_project(false);
    let bounded_project_id = bounded_project.project_id;
    let bounded_before = project_state_signature(&bounded_project);
    let too_few_error = execute_intersection_cluster_connection(
        &mut bounded_project,
        bounded_project_id,
        0,
        bounded_edges[..2].iter().map(interior_target).collect(),
        None,
    )
    .expect_err("fewer than three request targets must fail before ID allocation");
    assert_eq!(
        too_few_error,
        "an intersection cluster requires at least three target edges, found 2"
    );
    let too_many_error = execute_intersection_cluster_connection(
        &mut bounded_project,
        bounded_project_id,
        0,
        (0..65)
            .map(|_| interior_target(&bounded_edges[0]))
            .collect(),
        None,
    )
    .expect_err("more than 64 request targets must fail before ID allocation");
    assert_eq!(
        too_many_error,
        "an intersection cluster supports at most 64 target edges, found 65"
    );
    assert_eq!(project_state_signature(&bounded_project), bounded_before);

    let (mut stale_project, stale_edges) = create_cluster_project(false);
    let stale_project_id = stale_project.project_id;
    let stale_before = project_state_signature(&stale_project);
    let stale_error = execute_intersection_cluster_connection(
        &mut stale_project,
        stale_project_id,
        1,
        stale_edges.iter().map(interior_target).collect(),
        None,
    )
    .expect_err("stale cluster command must fail");
    assert_eq!(
        stale_error,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&stale_project), stale_before);

    let (mut incomplete_project, incomplete_edges) = create_cluster_project(true);
    let incomplete_project_id = incomplete_project.project_id;
    let incomplete_before = project_state_signature(&incomplete_project);
    let incomplete_error = execute_intersection_cluster_connection(
        &mut incomplete_project,
        incomplete_project_id,
        0,
        incomplete_edges[..3].iter().map(interior_target).collect(),
        None,
    )
    .expect_err("an omitted intersecting edge must reject the whole cluster");
    assert!(incomplete_error.contains("also passes through the intersection cluster"));
    assert!(incomplete_error.contains(&format!("{:?}", incomplete_edges[3].id)));
    assert_eq!(
        project_state_signature(&incomplete_project),
        incomplete_before
    );

    let (mut boundary_project, boundary_edges) = create_cluster_project(false);
    let boundary_project_id = boundary_project.project_id;
    let boundary_before = project_state_signature(&boundary_project);
    let boundary = boundary_project.editor.pattern().edges[0].clone();
    let boundary_error = execute_intersection_cluster_connection(
        &mut boundary_project,
        boundary_project_id,
        0,
        vec![
            interior_target(&boundary),
            interior_target(&boundary_edges[1]),
            interior_target(&boundary_edges[2]),
        ],
        None,
    )
    .expect_err("boundary clusters remain unsupported in the first core increment");
    assert!(boundary_error.contains("does not yet support boundary edge"));
    assert_eq!(project_state_signature(&boundary_project), boundary_before);
}
