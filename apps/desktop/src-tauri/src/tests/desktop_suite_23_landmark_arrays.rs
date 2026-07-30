#[test]
fn folded_landmark_ranking_rejects_collision_and_resource_one_over_without_mutation() {
    let reference = BeginnerReferenceModelSuggestionV1 {
        asset_id: AssetId::new(),
        bbox_min_tenths_mm: [0, 0, 0],
        bbox_max_tenths_mm: [100, 80, 40],
        dominant_normal_milli: [0, 0, 1000],
        surface_area_milli: 8_000,
        surface_landmarks_tenths_mm: vec![[0, 0, 0], [100, 80, 40]],
        surface_ranges: Vec::new(),
        protrusions: Vec::new(),
        general_protrusion_candidates: Vec::new(),
        stick_bars: Vec::new(),
        component_count: 1,
        inferred_component_bridges: false,
        principal_axis_extents_tenths_mm: [100, 80, 40],
        quality_score: 0,
        quality_reasons: vec!["strict_glb_vertex_index_bounds".to_owned()],
        insufficiency_reasons: vec!["insufficient_distinct_vertices".to_owned()],
        pair_bindings: Vec::new(),
        method: "test".to_owned(),
        suggested_part_kind: None,
    };
    let plan = |vertices: Vec<Vertex>, edges: Vec<Edge>| ori_domain::BeginnerGeneratedPlanV1 {
        schema_version: 1,
        kind: ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
        crease_pattern: CreasePattern { vertices, edges },
        instruction_codes: Vec::new(),
        target_parts: Vec::new(),
        skeleton_segments: Vec::new(),
        target_asset: None,
        semantic_landmark_provenance: None,
    };
    let a = VertexId::new();
    let b = VertexId::new();
    let c = VertexId::new();
    let collision = plan(
        vec![
            Vertex {
                id: a,
                position: Point2::new(1.0, 1.0),
            },
            Vertex {
                id: b,
                position: Point2::new(1.0, 1.0),
            },
            Vertex {
                id: c,
                position: Point2::new(2.0, 1.0),
            },
        ],
        vec![Edge {
            id: EdgeId::new(),
            start: a,
            end: c,
            kind: EdgeKind::Mountain,
        }],
    );
    assert_eq!(
        bounded_folded_pose_landmark_score_v1(&collision, &reference),
        None
    );
    let tiny_end = VertexId::new();
    let tiny = CreasePattern {
        vertices: vec![
            Vertex {
                id: a,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: tiny_end,
                position: Point2::new(1.0e-12, 0.0),
            },
        ],
        edges: vec![Edge {
            id: EdgeId::new(),
            start: a,
            end: tiny_end,
            kind: EdgeKind::Valley,
        }],
    };
    assert_eq!(
        validate_beginner_manufacturability_v1(&tiny, &Paper::default()),
        Err("manufacturability_minimum_crease_spacing")
    );
    let p0 = VertexId::new();
    let p1 = VertexId::new();
    let p2 = VertexId::new();
    let _manufacturable_sequence = plan(
        vec![
            Vertex {
                id: p0,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: p1,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: p2,
                position: Point2::new(0.0, 10.0),
            },
        ],
        vec![
            Edge {
                id: EdgeId::new(),
                start: p0,
                end: p1,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: p1,
                end: p2,
                kind: EdgeKind::Valley,
            },
        ],
    );
    let boundary_vertices = [
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (5.0, 10.0),
        (0.0, 10.0),
    ]
    .into_iter()
    .map(|(x, y)| Vertex {
        id: VertexId::new(),
        position: Point2::new(x, y),
    })
    .collect::<Vec<_>>();
    let boundary_ids = boundary_vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let certified_edge = Edge {
        id: EdgeId::new(),
        start: boundary_ids[1],
        end: boundary_ids[4],
        kind: EdgeKind::Mountain,
    };
    let certified = plan(
        vec![boundary_vertices[1].clone(), boundary_vertices[4].clone()],
        vec![certified_edge.clone()],
    );
    let mut certified_edges = (0..boundary_ids.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: boundary_ids[index],
            end: boundary_ids[(index + 1) % boundary_ids.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    certified_edges.push(certified_edge);
    let certified_pattern = CreasePattern {
        vertices: boundary_vertices,
        edges: certified_edges,
    };
    let certified_paper = Paper {
        boundary_vertices: boundary_ids,
        thickness_mm: 0.0,
        ..Paper::default()
    };
    let certified_editor =
        EditorState::with_paper(certified_pattern.clone(), certified_paper.clone());
    let certified_topology = certified_editor
        .topology_analysis_input(ProjectId::new())
        .analyze();
    let certified_topology = certified_topology
        .simulation_snapshot()
        .expect("certificate fixture topology");
    assert_eq!(
        certify_beginner_fold_path_v1(
            &certified,
            &certified_paper,
            &certified_pattern,
            certified_topology,
        ),
        certify_beginner_fold_path_v1(
            &certified,
            &certified_paper,
            &certified_pattern,
            certified_topology,
        )
    );
    let ranked = plan(
        vec![
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(10.0, 8.0),
            },
        ],
        Vec::new(),
    );
    let shape_profile = ori_domain::BeginnerDesignProfileV1 {
        preset: ori_domain::BeginnerDesignPresetV1::ShapePriority,
        shape_fidelity_weight: 60,
        foldability_weight: 20,
        step_count_weight: 10,
        paper_efficiency_weight: 10,
        ..ori_domain::BeginnerDesignProfileV1::default()
    };
    let fold_profile = ori_domain::BeginnerDesignProfileV1 {
        preset: ori_domain::BeginnerDesignPresetV1::FoldabilityPriority,
        shape_fidelity_weight: 20,
        foldability_weight: 60,
        step_count_weight: 10,
        paper_efficiency_weight: 10,
        ..ori_domain::BeginnerDesignProfileV1::default()
    };
    assert_ne!(
        preset_weighted_refinement_score_v1(&ranked, &reference, &shape_profile),
        preset_weighted_refinement_score_v1(&ranked, &reference, &fold_profile),
    );

    let oversized = plan(
        (0..=MAX_BEGINNER_FOLDED_LANDMARKS_V1)
            .map(|index| Vertex {
                id: VertexId::new(),
                position: Point2::new(index as f64, 0.0),
            })
            .collect(),
        Vec::new(),
    );
    assert_eq!(
        bounded_folded_pose_landmark_score_v1(&oversized, &reference),
        None
    );
    let project = initial_project_state();
    let before = project.document();
    let _ = bounded_folded_pose_landmark_score_v1(&oversized, &reference);
    assert_eq!(project.document(), before);
}

#[test]
fn linear_array_preview_and_confirm_are_bound_read_only_and_atomic() {
    let sheet = create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let mut vertices = [VertexId::new(), VertexId::new()];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: vertices[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: vertices[1],
            position: Point2::new(30.0, 20.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: vertices[0],
        end: vertices[1],
        kind: EdgeKind::Mountain,
    });
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let instance = project.instance_id;
    let id = project.project_id;
    let request = LinearArrayRequestV1 {
        vertices: vertices.to_vec(),
        edges: vec![edge],
        additional_copies: 1,
        delta: Point2::new(0.0, 10.0),
    };
    let before = project.document();
    let preview = preview_linear_array_inner(&project, instance, id, 0, request.clone()).unwrap();
    assert_eq!(project.document(), before);
    assert!(!preview.authorizes_project_mutation);
    for (bad_instance, bad_id, bad_revision) in [
        (ProjectId::new(), id, 0),
        (instance, ProjectId::new(), 0),
        (instance, id, 1),
    ] {
        assert!(
            preview_linear_array_inner(
                &project,
                bad_instance,
                bad_id,
                bad_revision,
                request.clone()
            )
            .is_err()
        );
        assert_eq!(project.document(), before);
    }
    let mut bad_digest = preview.request_sha256.clone();
    bad_digest.replace_range(0..1, if &bad_digest[0..1] == "0" { "1" } else { "0" });
    assert!(
        confirm_linear_array_inner(&mut project, instance, id, 0, request.clone(), bad_digest)
            .is_err()
    );
    assert_eq!(project.document(), before);
    let mut changed_request = request.clone();
    changed_request.delta.y = 11.0;
    assert!(
        confirm_linear_array_inner(
            &mut project,
            instance,
            id,
            0,
            changed_request,
            preview.request_sha256.clone(),
        )
        .is_err()
    );
    assert_eq!(project.document(), before);
    let snapshot = confirm_linear_array_inner(
        &mut project,
        instance,
        id,
        0,
        request.clone(),
        preview.request_sha256,
    )
    .unwrap();
    assert_eq!(snapshot.revision, 1);
    assert!(
        confirm_linear_array_inner(&mut project, instance, id, 0, request, String::new()).is_err()
    );
    project.editor.undo(1).unwrap();
    assert_eq!(project.editor.pattern(), &before.crease_pattern);
    let restored = project.document();
    let invalid = LinearArrayRequestV1 {
        vertices: vertices.to_vec(),
        edges: vec![edge],
        additional_copies: 0,
        delta: Point2::new(0.0, 10.0),
    };
    assert!(preview_linear_array_inner(&project, instance, id, 2, invalid).is_err());
    assert_eq!(project.document(), restored);
}

#[test]
fn radial_array_preview_and_confirm_are_domain_bound_read_only_and_atomic() {
    let sheet = create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(50.0, 50.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(60.0, 50.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Mountain,
    });
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let instance = project.instance_id;
    let id = project.project_id;
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let request = RadialArrayRequestV1 {
        center,
        vertices,
        edges: vec![edge],
        additional_copies: 1,
        angle_microdegrees: 90_000_000,
    };
    let before = project.document();
    let preview = preview_radial_array_inner(&project, instance, id, 0, request.clone()).unwrap();
    assert_eq!(project.document(), before);
    assert!(!preview.authorizes_project_mutation);
    assert!(
        preview_radial_array_inner(&project, ProjectId::new(), id, 0, request.clone()).is_err()
    );
    assert!(
        preview_radial_array_inner(&project, instance, ProjectId::new(), 0, request.clone())
            .is_err()
    );
    assert!(preview_radial_array_inner(&project, instance, id, 1, request.clone()).is_err());
    assert_eq!(project.document(), before);
    let linear = LinearArrayRequestV1 {
        vertices: request.vertices.clone(),
        edges: request.edges.clone(),
        additional_copies: 1,
        delta: Point2::new(0.0, 10.0),
    };
    assert_ne!(
        preview.request_sha256,
        linear_array_request_sha256(instance, id, 0, &linear).unwrap()
    );
    let mut changed = request.clone();
    changed.angle_microdegrees = 180_000_000;
    assert!(
        confirm_radial_array_inner(
            &mut project,
            instance,
            id,
            0,
            changed,
            preview.request_sha256.clone()
        )
        .is_err()
    );
    assert_eq!(project.document(), before);
    assert!(
        confirm_radial_array_inner(
            &mut project,
            instance,
            id,
            0,
            request.clone(),
            "0".repeat(64)
        )
        .is_err()
    );
    assert_eq!(project.document(), before);
    let snapshot = confirm_radial_array_inner(
        &mut project,
        instance,
        id,
        0,
        request,
        preview.request_sha256,
    )
    .unwrap();
    assert_eq!(snapshot.revision, 1);
    project.editor.undo(1).unwrap();
    assert_eq!(project.editor.pattern(), &before.crease_pattern);
}
