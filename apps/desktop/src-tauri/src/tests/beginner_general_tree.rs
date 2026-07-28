use super::*;

#[test]
fn generic_mixed_target_grid_apply_undo_redo_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 2),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 1,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    );
    profile
        .generation_constraints
        .generic_body_outline_tenths_mm =
        Some(vec![[-120, -80], [-120, 80], [120, 80], [120, -80]]);
    profile.generation_constraints.protrusions[0].local_outline_tenths_mm =
        Some(vec![[-20, 0], [20, 0], [0, 60]]);
    let mut fin = profile.generation_constraints.protrusions[0].clone();
    fin.id = 2;
    fin.local_outline_tenths_mm = None;
    fin.count = 2;
    fin.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
    fin.direction_milli = [1000, 0, 0];
    fin.priority = 60;
    profile.generation_constraints.protrusions.push(fin);
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let temporary = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    assert_eq!(temporary.generation_constraints.protrusions.len(), 2);
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
    .unwrap();
    let witness = beginner_contour_placement_witness(&profile.generation_constraints, &plan)
        .expect("generated contour geometry must provide a bounded witness");
    let graph_edge = plan.crease_pattern.edges.last().unwrap();
    let graph_start = plan
        .crease_pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == graph_edge.start)
        .unwrap()
        .position;
    let graph_end = plan
        .crease_pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == graph_edge.end)
        .unwrap()
        .position;
    let midpoint = ori_domain::Point2::new(
        (graph_start.x + graph_end.x) / 2.0,
        (graph_start.y + graph_end.y) / 2.0,
    );
    let crossing_start = ori_domain::VertexId::new();
    let crossing_end = ori_domain::VertexId::new();
    let mut crossing_pattern = project.editor.pattern().clone();
    crossing_pattern.vertices.extend([
        ori_domain::Vertex {
            id: crossing_start,
            position: ori_domain::Point2::new(
                midpoint.x - (graph_end.y - graph_start.y) * 0.1,
                midpoint.y + (graph_end.x - graph_start.x) * 0.1,
            ),
        },
        ori_domain::Vertex {
            id: crossing_end,
            position: ori_domain::Point2::new(
                midpoint.x + (graph_end.y - graph_start.y) * 0.1,
                midpoint.y - (graph_end.x - graph_start.x) * 0.1,
            ),
        },
    ]);
    crossing_pattern.edges.push(ori_domain::Edge {
        id: ori_domain::EdgeId::new(),
        start: crossing_start,
        end: crossing_end,
        kind: ori_domain::EdgeKind::Mountain,
    });
    let crossing_assessment = assess_beginner_generated_plan(
        project.project_id,
        project.editor.paper(),
        &crossing_pattern,
        &plan,
        None,
    );
    assert!(!crossing_assessment.apply_allowed);
    assert_eq!(crossing_assessment.reason, "geometry_invalid");
    let exported_rejection = serde_json::to_value(&crossing_assessment)
        .expect("export rejected generic candidate assessment");
    assert_eq!(exported_rejection["reason"], "geometry_invalid");
    assert_eq!(exported_rejection["apply_allowed"], false);
    assert_eq!(exported_rejection["proof_scope"], "necessary");
    let mut duplicate_skeleton = profile.generation_constraints.clone();
    let mut duplicate = duplicate_skeleton.skeleton_segments[0].clone();
    duplicate.id = 99;
    duplicate_skeleton.skeleton_segments.push(duplicate);
    assert!(beginner_contour_placement_witness(&duplicate_skeleton, &plan).is_none());
    let mut cyclic_skeleton = profile.generation_constraints.clone();
    cyclic_skeleton
        .skeleton_segments
        .push(ori_domain::BeginnerSkeletonSegmentV1 {
            id: 99,
            start: cyclic_skeleton.skeleton_segments[0].start,
            end: cyclic_skeleton.skeleton_segments[1].start,
            thickness_tenths_mm: 50,
        });
    assert!(beginner_contour_placement_witness(&cyclic_skeleton, &plan).is_none());
    let mut reordered_constraints = profile.generation_constraints.clone();
    reordered_constraints.protrusions.reverse();
    let reordered_witness = beginner_contour_placement_witness(&reordered_constraints, &plan)
        .expect("feature ID mapping must not depend on protrusion storage order");
    assert_eq!(
        reordered_witness
            .generic_feature_bindings
            .iter()
            .map(|binding| (
                binding.generated_feature_id,
                binding.protrusion_id,
                binding.crease_authority_sha256,
            ))
            .collect::<Vec<_>>(),
        witness
            .generic_feature_bindings
            .iter()
            .map(|binding| (
                binding.generated_feature_id,
                binding.protrusion_id,
                binding.crease_authority_sha256,
            ))
            .collect::<Vec<_>>(),
    );
    let graph_vertex_count = profile
        .generation_constraints
        .skeleton_segments
        .iter()
        .flat_map(|segment| {
            [
                (segment.start.x_tenths_mm, segment.start.y_tenths_mm),
                (segment.end.x_tenths_mm, segment.end.y_tenths_mm),
            ]
        })
        .collect::<HashSet<_>>()
        .len();
    let body_start = plan.crease_pattern.vertices.len()
        - graph_vertex_count
        - usize::from(witness.witnessed_vertices);
    let mut cyclic_body = plan.crease_pattern.vertices[body_start..body_start + 4].to_vec();
    cyclic_body.rotate_left(2);
    cyclic_body.reverse();
    assert_eq!(
        normalized_contour_error_millionths(
            profile
                .generation_constraints
                .generic_body_outline_tenths_mm
                .as_deref()
                .unwrap(),
            &cyclic_body,
        ),
        Some(0)
    );
    cyclic_body.pop();
    assert!(
        normalized_contour_error_millionths(
            profile
                .generation_constraints
                .generic_body_outline_tenths_mm
                .as_deref()
                .unwrap(),
            &cyclic_body,
        )
        .is_some_and(|error| error > 1)
    );
    let archived_profile: ori_domain::BeginnerDesignProfileV1 =
        serde_json::from_slice(&serde_json::to_vec(&profile).unwrap()).unwrap();
    let archived_plan: ori_domain::BeginnerGeneratedPlanV1 =
        serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
    assert_eq!(
        beginner_contour_placement_witness(
            &archived_profile.generation_constraints,
            &archived_plan,
        )
        .unwrap()
        .topology_authority_hash,
        witness.topology_authority_hash,
    );
    let mut tampered_plan = archived_plan.clone();
    let contour_start = tampered_plan.crease_pattern.vertices.len() - 7;
    tampered_plan.crease_pattern.vertices[contour_start]
        .position
        .x += 0.001;
    assert!(
        beginner_contour_placement_witness(
            &archived_profile.generation_constraints,
            &tampered_plan,
        )
        .is_none()
    );
    let alternate_point = ori_domain::beginner_parameter_grid_v1()[14];
    let alternate_plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile,
        alternate_point,
    )
    .unwrap()
    .into_iter()
    .find(|candidate| {
        candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    })
    .unwrap();
    assert_eq!(
        beginner_contour_placement_witness(&profile.generation_constraints, &alternate_plan,)
            .unwrap()
            .topology_authority_hash,
        witness.topology_authority_hash,
    );
    assert_eq!(witness.body_contour_points, 4);
    assert_eq!(witness.local_bindings.len(), 1);
    assert_eq!(witness.local_bindings[0].protrusion_id, 1);
    assert_eq!(witness.local_bindings[0].generated_face_id, 1);
    assert_eq!(witness.generic_feature_bindings.len(), 2);
    assert!(witness.generic_feature_bindings.len() <= 16);
    assert!(
        witness
            .generic_feature_bindings
            .iter()
            .map(|binding| usize::from(binding.endpoint_count))
            .sum::<usize>()
            <= 16
    );
    assert_eq!(witness.generic_feature_bindings[0].protrusion_id, 1);
    assert_eq!(witness.generic_feature_bindings[0].endpoint_count, 1);
    assert_eq!(witness.generic_feature_bindings[0].skeleton_segment_id, 1);
    assert_eq!(
        witness.generic_feature_bindings[0].skeleton_endpoint,
        "start"
    );
    assert_eq!(witness.generic_feature_bindings[1].protrusion_id, 2);
    assert_eq!(witness.generic_feature_bindings[1].endpoint_count, 2);
    let mut remapped_profile = profile.clone();
    remapped_profile.generation_constraints.skeleton_segments[0]
        .start
        .x_tenths_mm += 1;
    assert_ne!(
        beginner_contour_placement_witness(&remapped_profile.generation_constraints, &plan,)
            .unwrap()
            .topology_authority_hash,
        witness.topology_authority_hash,
    );
    assert_eq!(witness.witnessed_vertices, 7);
    assert_eq!(witness.witnessed_creases, 7);
    let mut one_short = plan.clone();
    one_short.crease_pattern.edges.truncate(6);
    assert!(
        beginner_contour_placement_witness(&profile.generation_constraints, &one_short,).is_none()
    );
    profile
        .generation_constraints
        .generic_body_outline_tenths_mm = None;
    profile.generation_constraints.protrusions[0].local_outline_tenths_mm = None;
    let baseline_plans = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap();
    let plan = baseline_plans
        .iter()
        .find(|candidate| {
            candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        })
        .cloned()
        .unwrap();
    let outline_free_witness =
        beginner_contour_placement_witness(&profile.generation_constraints, &plan).unwrap();
    assert!(outline_free_witness.local_bindings.is_empty());
    assert_eq!(outline_free_witness.generic_feature_bindings.len(), 2);
    let baseline_assessment = assess_beginner_generated_plan_with_deadline(
        project.project_id,
        project.editor.paper(),
        project.editor.pattern(),
        &plan,
        None,
        std::time::Instant::now() + std::time::Duration::from_millis(750),
    );
    let sorted_segments = profile.generation_constraints.skeleton_segments.clone();
    let mut storage_shuffled_profile = profile.clone();
    storage_shuffled_profile
        .generation_constraints
        .skeleton_segments = [2, 0, 1].map(|index| sorted_segments[index]).to_vec();
    let mut all_endpoints_reversed_profile = profile.clone();
    for segment in &mut all_endpoints_reversed_profile
        .generation_constraints
        .skeleton_segments
    {
        std::mem::swap(&mut segment.start, &mut segment.end);
    }
    let mut one_endpoint_reversed_profile = profile.clone();
    let segment = &mut one_endpoint_reversed_profile
        .generation_constraints
        .skeleton_segments[0];
    std::mem::swap(&mut segment.start, &mut segment.end);
    let mut shuffled_reversed_profile = storage_shuffled_profile.clone();
    for segment in &mut shuffled_reversed_profile
        .generation_constraints
        .skeleton_segments
    {
        std::mem::swap(&mut segment.start, &mut segment.end);
    }
    let baseline_witness_json = serde_json::to_vec(&outline_free_witness).unwrap();
    let baseline_assessment_json = serde_json::to_vec(&baseline_assessment).unwrap();
    for (label, variant_profile) in [
        ("storage shuffle", &storage_shuffled_profile),
        ("all endpoint directions", &all_endpoints_reversed_profile),
        ("one endpoint direction", &one_endpoint_reversed_profile),
        (
            "shuffle and endpoint directions",
            &shuffled_reversed_profile,
        ),
    ] {
        let variant_plans = grid_template_plan(
            project.project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            variant_profile,
            point,
        )
        .unwrap();
        assert_eq!(
            variant_plans, baseline_plans,
            "{label} must preserve the complete generic candidate Vec bit-exactly"
        );
        let variant_plan = variant_plans
            .iter()
            .find(|candidate| {
                candidate.kind
                    == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
            })
            .unwrap();
        let variant_witness = beginner_contour_placement_witness(
            &variant_profile.generation_constraints,
            variant_plan,
        )
        .expect("canonical graph-edge tail must remain consumable by the native witness");
        assert_eq!(
            serde_json::to_vec(&variant_witness).unwrap(),
            baseline_witness_json,
            "{label} must preserve the complete native witness bit-exactly"
        );
        let variant_assessment = assess_beginner_generated_plan_with_deadline(
            project.project_id,
            project.editor.paper(),
            project.editor.pattern(),
            variant_plan,
            None,
            std::time::Instant::now() + std::time::Duration::from_millis(750),
        );
        assert_eq!(
            serde_json::to_vec(&variant_assessment).unwrap(),
            baseline_assessment_json,
            "{label} must preserve the complete assessment bit-exactly"
        );
    }
    let baseline_generic_tree = {
        let mut canonical_profile = profile.clone();
        let mut canonical_segments = sorted_segments.clone();
        canonical_segments.sort_unstable_by_key(|segment| segment.id);
        for segment in &mut canonical_segments {
            let start = (segment.start.x_tenths_mm, segment.start.y_tenths_mm);
            let end = (segment.end.x_tenths_mm, segment.end.y_tenths_mm);
            if end < start {
                std::mem::swap(&mut segment.start, &mut segment.end);
            }
        }
        canonical_profile.generation_constraints.skeleton_segments = canonical_segments;
        let mut baseline_project = initial_project_state();
        let canonical_plan = grid_template_plan(
            baseline_project.project_id,
            baseline_project.editor.pattern(),
            &baseline_project.editor.paper().boundary_vertices,
            &canonical_profile,
            point,
        )
        .unwrap()
        .into_iter()
        .find(|candidate| {
            candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        })
        .unwrap();
        let baseline_project_id = baseline_project.project_id;
        let baseline_instance_id = baseline_project.instance_id;
        let baseline_revision = baseline_project.editor.revision();
        let saved = execute_command(
            &mut baseline_project,
            baseline_project_id,
            baseline_revision,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(canonical_profile),
            },
        )
        .unwrap();
        apply_grid_plan_document(
            &mut baseline_project,
            baseline_instance_id,
            baseline_project_id,
            saved.revision,
            canonical_plan,
        )
        .unwrap();
        baseline_project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.generic_tree.clone())
            .expect("canonical baseline generic tree provenance")
    };
    profile = shuffled_reversed_profile;
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile.clone()),
        },
    )
    .unwrap();
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
    )
    .unwrap();
    assert!(
        project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .is_some()
    );
    let generic_tree = project
        .editor
        .beginner_design_profile()
        .generation_provenance
        .as_ref()
        .and_then(|provenance| provenance.generic_tree.as_ref())
        .expect("generic apply must persist read-only canonical tree provenance");
    assert_eq!(
        generic_tree, &baseline_generic_tree,
        "canonical and shuffled/reversed apply must persist identical generic tree provenance"
    );
    let mut canonical_segments = profile.generation_constraints.skeleton_segments.clone();
    canonical_segments.sort_unstable_by_key(|segment| segment.id);
    for segment in &mut canonical_segments {
        let start = (segment.start.x_tenths_mm, segment.start.y_tenths_mm);
        let end = (segment.end.x_tenths_mm, segment.end.y_tenths_mm);
        if end < start {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
    }
    assert_eq!(
        project
            .editor
            .beginner_design_profile()
            .generation_constraints
            .skeleton_segments,
        canonical_segments,
        "apply must persist canonical bar storage for archive equality"
    );
    assert_eq!(
        generic_tree.tree_topology_sha256,
        <[u8; 32]>::from(sha2::Sha256::digest(
            serde_json::to_vec(&canonical_segments).unwrap()
        ))
    );
    assert_eq!(
        generic_tree.normalized_length_ratios,
        [1_000_000, 1_000_000, 2_000_000]
    );
    assert_eq!(
        generic_tree.source,
        ori_domain::BeginnerGenericTreeSourceV1::ManualSkeleton
    );
    assert_eq!(
        generic_tree.orientation,
        ori_domain::BeginnerGenericTreeOrientationV1::Horizontal
    );
    assert!(!generic_tree.authorizes_apply);
    let proposal = generic_tree
        .instruction_proposal
        .as_ref()
        .expect("generic tree instruction proposal");
    assert_eq!(proposal.topology_sha256, generic_tree.tree_topology_sha256);
    assert!(!proposal.authorizes_apply);
    assert!(!proposal.physical_motion_proof);
    let mut proposal_assignments = generic_tree
        .instruction_proposal
        .as_ref()
        .unwrap()
        .steps
        .iter()
        .map(|step| {
            (
                step.canonical_crease_id.as_str(),
                step.tree_depth,
                step.assignment.as_str(),
            )
        })
        .collect::<Vec<_>>();
    proposal_assignments.sort_unstable();
    assert_eq!(
        proposal_assignments,
        [
            ("tree-river-0001", 0, "valley"),
            ("tree-river-0002", 1, "mountain"),
            ("tree-river-0003", 1, "valley"),
        ]
    );
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            saved_profile.revision,
            plan,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    assert!(
        project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
    execute_redo(&mut project, project_id, undone.revision).unwrap();
    assert!(
        project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .is_some()
    );
    let mut saved = project.document();
    saved.thumbnail_svg = None;
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("generic-target.ori2"));
    assert_eq!(
        reopened.editor.beginner_design_profile(),
        &saved.beginner_design_profile
    );
    assert_eq!(
        reopened
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.generic_tree.as_ref()),
        Some(&baseline_generic_tree),
        "archive round-trip must preserve canonical generic tree provenance"
    );
    assert!(
        reopened
            .editor
            .instruction_timeline()
            .steps
            .last()
            .is_some_and(|step| step.caution.contains("topology authority SHA-256:"))
    );
}

#[test]
fn native_generic_tree_canonicalizer_reuses_domain_bar_ceiling() {
    let bar = |id: u16| ori_domain::BeginnerSkeletonSegmentV1 {
        id,
        start: ori_domain::BeginnerSkeletonPointV1 {
            x_tenths_mm: i32::from(id) + 1,
            y_tenths_mm: 1,
        },
        end: ori_domain::BeginnerSkeletonPointV1 {
            x_tenths_mm: i32::from(id),
            y_tenths_mm: 0,
        },
        thickness_tenths_mm: 1,
    };
    let mut exact = (0..u16::try_from(ori_domain::MAX_BEGINNER_GENERIC_TREE_BARS_V1).unwrap())
        .rev()
        .map(bar)
        .collect::<Vec<_>>();
    let canonical =
        crate::beginner_design_commands::canonical_generic_tree_segments_v1(&exact).unwrap();
    assert_eq!(
        canonical.len(),
        ori_domain::MAX_BEGINNER_GENERIC_TREE_BARS_V1
    );
    assert!(canonical.windows(2).all(|pair| pair[0].id < pair[1].id));
    assert!(canonical.iter().all(|segment| {
        (segment.start.x_tenths_mm, segment.start.y_tenths_mm)
            < (segment.end.x_tenths_mm, segment.end.y_tenths_mm)
    }));
    exact.push(bar(u16::try_from(
        ori_domain::MAX_BEGINNER_GENERIC_TREE_BARS_V1,
    )
    .unwrap()));
    assert!(crate::beginner_design_commands::canonical_generic_tree_segments_v1(&exact).is_none());
}

#[test]
fn beginner_manufacturability_is_deterministic_at_adjacent_spacing_bits() {
    let threshold = 1.0e-6_f64;
    let below = f64::from_bits(threshold.to_bits() - 1);
    let above = f64::from_bits(threshold.to_bits() + 1);
    let pattern = |length| {
        let start = VertexId::new();
        let end = VertexId::new();
        CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(length, 0.0),
                },
            ],
            edges: vec![Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Valley,
            }],
        }
    };

    assert_eq!(
        validate_beginner_manufacturability_v1(&pattern(below), &Paper::default()),
        Err("manufacturability_minimum_crease_spacing")
    );
    for length in [threshold, above] {
        assert_eq!(
            validate_beginner_manufacturability_v1(&pattern(length), &Paper::default()),
            Ok(())
        );
    }
    for length in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            validate_beginner_manufacturability_v1(&pattern(length), &Paper::default()),
            Err("manufacturability_non_finite_geometry")
        );
    }
}

#[test]
fn beginner_contour_authority_rejects_non_finite_distance_inputs() {
    let target = [[0, 0], [10, 0], [10, 10], [0, 10]];
    let generated = |invalid| {
        [Point2::new(0.0, 0.0), Point2::new(1.0, 0.0), invalid]
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        normalized_contour_error_millionths(&target, &generated(Point2::new(f64::NAN, 1.0)),),
        None
    );
    assert_eq!(
        normalized_contour_error_millionths(&target, &generated(Point2::new(f64::INFINITY, 1.0)),),
        None
    );
}

#[test]
fn beginner_contour_authority_ignores_duplicate_finite_segments() {
    let target = [[0, 0], [10, 0], [10, 10], [0, 10]];
    let generated = [
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 0.0),
        Point2::new(1.0, 0.0),
        Point2::new(1.0, 1.0),
        Point2::new(0.0, 1.0),
    ]
    .into_iter()
    .map(|position| Vertex {
        id: VertexId::new(),
        position,
    })
    .collect::<Vec<_>>();

    assert_eq!(
        normalized_contour_error_millionths(&target, &generated),
        Some(0)
    );
}
