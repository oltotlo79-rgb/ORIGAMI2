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
        (ori_domain::BeginnerTargetPartKindV1::Fin, 3),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 3,
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
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let temporary = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    assert_eq!(temporary.generation_constraints.protrusions.len(), 3);
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
    let witness = beginner_contour_placement_witness(&temporary.generation_constraints, &plan)
        .expect("generated contour geometry must provide a bounded witness");
    let mut semantic_endpoint_mismatch = temporary.generation_constraints.clone();
    semantic_endpoint_mismatch
        .target_parts
        .iter_mut()
        .find(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Fin)
        .unwrap()
        .count = 4;
    assert!(
        beginner_contour_placement_witness(&semantic_endpoint_mismatch, &plan).is_none(),
        "native response preflight must reject semantic/physical endpoint-count disagreement"
    );
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
    let mut reordered_constraints = temporary.generation_constraints.clone();
    reordered_constraints.protrusions.reverse();
    assert!(
        beginner_contour_placement_witness(&reordered_constraints, &plan).is_none(),
        "noncanonical protrusion storage order must fail closed"
    );
    reordered_constraints
        .protrusions
        .sort_unstable_by_key(|target| target.id);
    let reordered_witness = beginner_contour_placement_witness(&reordered_constraints, &plan)
        .expect("restoring canonical protrusion order must restore the feature binding");
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
    let graph_edge_count = profile.generation_constraints.skeleton_segments.len();
    let body_start = plan.crease_pattern.vertices.len()
        - graph_vertex_count
        - usize::from(witness.witnessed_vertices);
    let body_crease_start =
        plan.crease_pattern.edges.len() - graph_edge_count - usize::from(witness.witnessed_creases);
    let support_count = radial_corner_support_added_v1(&plan);
    assert_eq!(
        usize::from(witness.generic_feature_bindings[0].crease_start),
        support_count
    );
    assert_eq!(
        body_crease_start,
        support_count
            + witness
                .generic_feature_bindings
                .iter()
                .map(|binding| usize::from(binding.endpoint_count))
                .sum::<usize>()
    );
    assert_eq!(
        usize::from(witness.local_bindings[0].vertex_start),
        body_start + 4
    );
    assert_eq!(
        usize::from(witness.local_bindings[0].crease_start),
        body_crease_start + 4
    );
    assert_ne!(
        witness.local_bindings[0].vertex_start, witness.local_bindings[0].crease_start,
        "the radial star owns one more vertex than edge, so contour cursors are independent"
    );
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
    let mut aliased_local_cycle = archived_plan.clone();
    let local_crease_start = usize::from(witness.local_bindings[0].crease_start);
    aliased_local_cycle.crease_pattern.edges[local_crease_start].start =
        aliased_local_cycle.crease_pattern.vertices[body_start].id;
    assert!(
        beginner_contour_placement_witness(
            &archived_profile.generation_constraints,
            &aliased_local_cycle,
        )
        .is_none(),
        "a local contour edge cannot alias a body contour vertex"
    );

    let mut identical_local_profile = profile.clone();
    identical_local_profile.generation_constraints.protrusions[1].local_outline_tenths_mm =
        identical_local_profile.generation_constraints.protrusions[0]
            .local_outline_tenths_mm
            .clone();
    let identical_local_plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &identical_local_profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|candidate| {
        candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    })
    .unwrap();
    let identical_local_witness = beginner_contour_placement_witness(
        &temporary_symmetric_profile_for_grid(&identical_local_profile, point)
            .unwrap()
            .generation_constraints,
        &identical_local_plan,
    )
    .expect("identical local outlines retain separate generated ownership");
    assert_eq!(identical_local_witness.local_bindings.len(), 2);
    let first = &identical_local_witness.local_bindings[0];
    let second = &identical_local_witness.local_bindings[1];
    assert_ne!(first.vertex_start, second.vertex_start);
    assert_ne!(first.crease_start, second.crease_start);
    let first_vertices = identical_local_plan
        .crease_pattern
        .vertices
        .get(
            usize::from(first.vertex_start)
                ..usize::from(first.vertex_start) + usize::from(first.contour_points),
        )
        .unwrap()
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    let second_vertices = identical_local_plan
        .crease_pattern
        .vertices
        .get(
            usize::from(second.vertex_start)
                ..usize::from(second.vertex_start) + usize::from(second.contour_points),
        )
        .unwrap()
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    assert!(first_vertices.is_disjoint(&second_vertices));
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
    assert_eq!(witness.generic_feature_bindings.len(), 3);
    assert!(
        witness.generic_feature_bindings.len()
            <= ori_domain::MAX_BEGINNER_GENERIC_PROTRUSION_BINDINGS_V1
    );
    assert!(
        witness
            .generic_feature_bindings
            .iter()
            .map(|binding| usize::from(binding.endpoint_count))
            .sum::<usize>()
            <= 32
    );
    assert_eq!(witness.generic_feature_bindings[0].protrusion_id, 1);
    assert_eq!(witness.generic_feature_bindings[0].endpoint_count, 1);
    assert_eq!(witness.generic_feature_bindings[0].skeleton_segment_id, 3);
    assert_eq!(
        witness.generic_feature_bindings[0].skeleton_endpoint,
        "start"
    );
    assert_eq!(witness.generic_feature_bindings[1].protrusion_id, 2);
    assert_eq!(witness.generic_feature_bindings[1].endpoint_count, 1);
    assert_eq!(witness.generic_feature_bindings[2].protrusion_id, 3);
    assert_eq!(witness.generic_feature_bindings[2].endpoint_count, 1);
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
    assert_eq!(outline_free_witness.generic_feature_bindings.len(), 3);
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
        let canonical_configured =
            temporary_symmetric_profile_for_grid(&canonical_profile, point).unwrap();
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
            canonical_configured,
            None,
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
    let configured = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
        configured.clone(),
        None,
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
            configured,
            None,
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
fn generic_feature_binding_contract_keeps_u16_source_ids_separate_from_dense_generated_ids() {
    let target = |id: u16| ori_domain::BeginnerProtrusionTargetV1 {
        id,
        count: 1,
        length_tenths_mm: 100,
        thickness_tenths_mm: 10,
        root_width_tenths_mm: None,
        tip_width_tenths_mm: None,
        local_outline_tenths_mm: None,
        position_tenths_mm: [0, 0, 0],
        direction_milli: [1_000, 0, 0],
        symmetry: ori_domain::BeginnerProtrusionSymmetryV1::None,
        curvature_degrees: 0,
        joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
        motion_degrees: [0, 0],
        side: ori_domain::BeginnerProtrusionSideV1::Either,
        priority: 50,
    };
    let binding =
        |protrusion_id: u16, generated_feature_id: u8| BeginnerGenericFeatureBindingWitness {
            protrusion_id,
            generated_feature_id,
            endpoint_count: 1,
            crease_start: 0,
            crease_authority_sha256: [0; 32],
            skeleton_segment_id: 1,
            skeleton_endpoint: "start",
            mount_distance_squared_tenths_mm: 0,
        };
    let targets = [0, 255, 256, u16::MAX]
        .map(target)
        .into_iter()
        .collect::<Vec<_>>();
    let bindings = [0, 255, 256, u16::MAX]
        .into_iter()
        .enumerate()
        .map(|(index, id)| binding(id, u8::try_from(index + 1).unwrap()))
        .collect::<Vec<_>>();
    assert!(generic_feature_binding_contract_v1(&targets, &bindings));

    let mut duplicate_source = targets.clone();
    duplicate_source[1].id = duplicate_source[0].id;
    assert!(!generic_feature_binding_contract_v1(
        &duplicate_source,
        &bindings
    ));
    let mut generated_gap = bindings.clone();
    generated_gap[2].generated_feature_id = 4;
    assert!(!generic_feature_binding_contract_v1(
        &targets,
        &generated_gap
    ));

    let fourteen_targets = (0_u16..14).map(target).collect::<Vec<_>>();
    let fourteen_bindings = (0_u16..14)
        .map(|id| binding(id, u8::try_from(id + 1).unwrap()))
        .collect::<Vec<_>>();
    assert!(generic_feature_binding_contract_v1(
        &fourteen_targets,
        &fourteen_bindings
    ));
    let fifteen_targets = (0_u16..15).map(target).collect::<Vec<_>>();
    let fifteen_bindings = (0_u16..15)
        .map(|id| binding(id, u8::try_from(id + 1).unwrap()))
        .collect::<Vec<_>>();
    assert!(!generic_feature_binding_contract_v1(
        &fifteen_targets,
        &fifteen_bindings
    ));
}

#[test]
fn general_grid_configuration_preserves_boundary_u16_singleton_ids() {
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 4),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 4,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    );
    for (target, id) in
        profile
            .generation_constraints
            .protrusions
            .iter_mut()
            .zip([0, 255, 256, u16::MAX])
    {
        target.id = id;
    }
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let configured = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    assert_eq!(
        configured
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| target.id)
            .collect::<Vec<_>>(),
        [0, 255, 256, u16::MAX]
    );
    let project = initial_project_state();
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
    let witness =
        beginner_contour_placement_witness(&configured.generation_constraints, &plan).unwrap();
    assert_eq!(
        witness
            .generic_feature_bindings
            .iter()
            .map(|binding| (binding.protrusion_id, binding.generated_feature_id))
            .collect::<Vec<_>>(),
        [(0, 1), (255, 2), (256, 3), (u16::MAX, 4)]
    );
}

#[test]
fn generic_tree_provenance_rejects_instruction_ratio_topology_order_and_orientation_tampering() {
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 3),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 3,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    );
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| {
        plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
            && plan
                .instruction_codes
                .last()
                .is_some_and(|code| code.ends_with(":horizontal"))
    })
    .unwrap();
    let configured = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    let mut accepted = configured.clone();
    assert!(
        build_beginner_generic_tree_provenance_v1(&project, &mut accepted, &plan, true)
            .unwrap()
            .is_some()
    );

    let topology_index = if plan
        .instruction_codes
        .get(1)
        .is_some_and(|code| code.starts_with("bounded_radial_corner_support_v1:"))
    {
        2
    } else {
        1
    };
    for tamper in 0..5 {
        let mut invalid = plan.clone();
        match tamper {
            0 => invalid.instruction_codes[0].push_str(",1"),
            1 => invalid.instruction_codes[topology_index].push_str(":extra"),
            2 => invalid.instruction_codes.swap(0, topology_index),
            3 => invalid.instruction_codes[1].push_str(":extra"),
            _ => {
                *invalid.instruction_codes.last_mut().unwrap() =
                    "bounded_tree_paper_orientation_v1:diagonal".to_owned()
            }
        }
        let mut candidate = configured.clone();
        assert!(
            build_beginner_generic_tree_provenance_v1(&project, &mut candidate, &invalid, true,)
                .is_err()
        );
    }
}

#[test]
fn grid_apply_rejects_vertex_identity_collision_and_pre_cancel_without_mutation() {
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 3),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 3,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    );
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let configured = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
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
    let colliding = plan
        .crease_pattern
        .vertices
        .iter()
        .find(|vertex| {
            !project
                .editor
                .pattern()
                .vertices
                .iter()
                .any(|current| current.id == vertex.id)
        })
        .unwrap();
    let project_id = project.project_id;
    let collision_revision = project.editor.revision();
    let collision_snapshot = execute_command(
        &mut project,
        project_id,
        collision_revision,
        Command::AddVertex {
            id: colliding.id,
            position: Point2::new(colliding.position.x + 0.25, colliding.position.y + 0.25),
        },
    )
    .unwrap();
    let before_collision = project.document();
    let instance_id = project.instance_id;
    let collision_error = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        collision_snapshot.revision,
        plan.clone(),
        configured.clone(),
        None,
    )
    .unwrap_err();
    assert_eq!(
        collision_error,
        "grid_candidate_vertex_identity_stale".to_owned()
    );
    assert_eq!(project.document(), before_collision);

    let mut consensus_project = initial_project_state();
    let consensus_plan = grid_template_plan(
        consensus_project.project_id,
        consensus_project.editor.pattern(),
        &consensus_project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|candidate| {
        candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    })
    .unwrap();
    let mut stale_consensus_profile = configured.clone();
    stale_consensus_profile.reference_consensus_v1 =
        Some(ori_domain::BeginnerReferenceConsensusV1 {
            schema_version: 1,
            bindings: [AssetId::new(), AssetId::new()]
                .into_iter()
                .map(|asset_id| ori_domain::BeginnerReferenceBindingV1 {
                    kind: ori_domain::BeginnerReferenceBindingKindV1::ReferenceModel,
                    asset_id,
                    sha256: [9; 32],
                    quality: 100,
                })
                .collect(),
            excluded_asset_id: None,
        });
    let consensus_instance_id = consensus_project.instance_id;
    let consensus_project_id = consensus_project.project_id;
    let consensus_revision = consensus_project.editor.revision();
    let before_consensus = consensus_project.document();
    let consensus_error = apply_grid_plan_document(
        &mut consensus_project,
        consensus_instance_id,
        consensus_project_id,
        consensus_revision,
        consensus_plan,
        stale_consensus_profile,
        None,
    )
    .unwrap_err();
    assert_eq!(
        consensus_error,
        "reference_consensus_asset_binding_stale".to_owned()
    );
    assert_eq!(consensus_project.document(), before_consensus);

    let mut cancelled_project = initial_project_state();
    let before_cancel = cancelled_project.document();
    let cancelled = AtomicBool::new(true);
    let cancelled_plan = grid_template_plan(
        cancelled_project.project_id,
        cancelled_project.editor.pattern(),
        &cancelled_project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|candidate| {
        candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
    })
    .unwrap();
    let cancelled_instance_id = cancelled_project.instance_id;
    let cancelled_project_id = cancelled_project.project_id;
    let cancelled_revision = cancelled_project.editor.revision();
    assert!(
        apply_grid_plan_document(
            &mut cancelled_project,
            cancelled_instance_id,
            cancelled_project_id,
            cancelled_revision,
            cancelled_plan,
            configured,
            Some(&cancelled),
        )
        .is_err()
    );
    assert_eq!(cancelled_project.document(), before_cancel);
}

#[test]
fn aggregate_general_counts_two_through_fourteen_apply_undo_redo_and_reopen() {
    let _serial = serial_beginner_grid_test();
    for count in 2_u8..=ori_domain::MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1 {
        let mut profile = ori_domain::BeginnerDesignProfileV1::default();
        profile.generation_constraints.target_category =
            Some(ori_domain::BeginnerTargetCategoryV1::Animal);
        let mut target_parts = vec![
            (ori_domain::BeginnerTargetPartKindV1::Head, 1),
            (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        ];
        if count == 2 {
            target_parts.extend([
                (ori_domain::BeginnerTargetPartKindV1::Fin, 1),
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
            ]);
        } else {
            target_parts.push((
                ori_domain::BeginnerTargetPartKindV1::Fin,
                count.min(ori_domain::MAX_BEGINNER_TARGET_PART_COUNT_V1),
            ));
            if count > ori_domain::MAX_BEGINNER_TARGET_PART_COUNT_V1 {
                target_parts.push((
                    ori_domain::BeginnerTargetPartKindV1::Tail,
                    count - ori_domain::MAX_BEGINNER_TARGET_PART_COUNT_V1,
                ));
            }
        }
        profile.generation_constraints.target_parts = target_parts
            .into_iter()
            .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
            .collect();
        assert!(configure_symmetric_profile(
            &mut profile,
            ori_domain::BeginnerSymmetricParameterEstimateV1 {
                protrusion_count: count,
                scale_percent: 27,
                spacing_percent: 50,
            },
            27,
            50,
        ));
        if count == 6 {
            profile
                .generation_constraints
                .generic_body_outline_tenths_mm =
                Some(vec![[-120, -80], [-120, 80], [120, 80], [120, -80]]);
            profile.generation_constraints.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-20, 0], [20, 0], [0, 60]]);
        }
        let point = ori_domain::beginner_parameter_grid_v1()[13];
        let configured = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
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
        .find(|candidate| {
            candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
                && candidate
                    .instruction_codes
                    .last()
                    .is_some_and(|code| code.ends_with(":horizontal"))
        })
        .unwrap();
        let witness =
            beginner_contour_placement_witness(&configured.generation_constraints, &plan).unwrap();
        assert_eq!(witness.generic_feature_bindings.len(), usize::from(count));
        let support_count = radial_corner_support_added_v1(&plan);
        if matches!(count, 2 | 4) {
            assert_eq!(
                support_count, 4,
                "small even generic fans must add all four canonical paper corners"
            );
            assert_eq!(
                plan.crease_pattern
                    .edges
                    .iter()
                    .filter(|edge| {
                        matches!(
                            edge.kind,
                            ori_domain::EdgeKind::Mountain | ori_domain::EdgeKind::Valley
                        )
                    })
                    .count(),
                usize::from(count) + support_count,
                "small even generic fans must preserve all semantic and support rays"
            );
            assert!(
                usize::from(count) + support_count >= 6
                    && (usize::from(count) + support_count).is_multiple_of(2),
                "small even generic fans must enter the six-or-more radial bifold theorem"
            );
            assert!(beginner_plan_has_radial_corner_support_v1(&plan));
        }
        if count == 13 {
            assert_eq!(
                support_count, 5,
                "count thirteen must add one parity ray after all four paper corners"
            );
            assert_eq!(
                plan.crease_pattern
                    .edges
                    .iter()
                    .filter(|edge| {
                        matches!(
                            edge.kind,
                            ori_domain::EdgeKind::Mountain | ori_domain::EdgeKind::Valley
                        )
                    })
                    .count(),
                18,
                "count thirteen must enter the existing even radial theorem"
            );
        }
        if count == ori_domain::MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1 {
            assert_eq!(
                support_count, 4,
                "count fourteen must consume the four bounded corner-support slots"
            );
            assert_eq!(
                plan.crease_pattern
                    .edges
                    .iter()
                    .filter(|edge| {
                        matches!(
                            edge.kind,
                            ori_domain::EdgeKind::Mountain | ori_domain::EdgeKind::Valley
                        )
                    })
                    .count(),
                18,
                "count fourteen must remain below the native radial theorem boundary"
            );
            assert!(
                beginner_plan_has_radial_corner_support_v1(&plan),
                "the native certificate preflight must admit eighteen hinges below its twenty-four-sector boundary"
            );
        }
        assert_eq!(
            usize::from(witness.generic_feature_bindings[0].crease_start),
            support_count,
            "generic feature slices must begin after the physical paper-corner support prefix"
        );
        if count == 6 {
            let graph_edge_count = configured.generation_constraints.skeleton_segments.len();
            let contour_start = plan.crease_pattern.edges.len()
                - graph_edge_count
                - usize::from(witness.witnessed_creases);
            assert_eq!(
                usize::from(witness.local_bindings[0].crease_start),
                contour_start + 4,
                "support-prefixed radial features must not shift the body/local contour tail slices"
            );
        }
        let mut provenance_profile = configured.clone();
        assert!(
            build_beginner_generic_tree_provenance_v1(
                &project,
                &mut provenance_profile,
                &plan,
                true,
            )
            .unwrap()
            .is_some()
        );
        let (mut target_pattern, target_paper) = materialize_beginner_boundary_splits_v1(
            project.editor.pattern(),
            project.editor.paper(),
            &plan,
        )
        .unwrap();
        for vertex in &plan.crease_pattern.vertices {
            if !target_pattern
                .vertices
                .iter()
                .any(|current| current.id == vertex.id)
            {
                target_pattern.vertices.push(vertex.clone());
            }
        }
        target_pattern
            .edges
            .extend(plan.crease_pattern.edges.iter().cloned());
        let target_pattern_validation = validate_crease_pattern(&target_pattern);
        assert!(
            target_pattern_validation.is_valid(),
            "general count {count} target pattern invalid before apply: {:?}",
            target_pattern_validation.issues
        );
        let target_paper_validation = validate_paper(&target_paper, &target_pattern);
        assert!(
            target_paper_validation.is_valid(),
            "general count {count} target paper invalid before apply: {:?}",
            target_paper_validation.issues
        );
        if matches!(count, 2 | 4 | 13 | 14) {
            let certificate_topology =
                EditorState::with_paper(target_pattern.clone(), target_paper.clone())
                    .topology_analysis_input(project.project_id)
                    .analyze();
            let certificate_snapshot =
                certificate_topology
                    .simulation_snapshot()
                    .unwrap_or_else(|| {
                        panic!("general count {count} radial topology must be simulation-ready")
                    });
            assert!(
                certify_beginner_fold_path_v1(
                    &plan,
                    &target_paper,
                    &target_pattern,
                    certificate_snapshot,
                )
                .is_some(),
                "general count {count} must receive an issuer-bound positive-thickness path certificate"
            );

            let mut tampered_plan = plan.clone();
            let removed_edge = tampered_plan.crease_pattern.edges.remove(0);
            let mut tampered_pattern = target_pattern.clone();
            let edge_count_before = tampered_pattern.edges.len();
            tampered_pattern
                .edges
                .retain(|edge| edge.id != removed_edge.id);
            assert_eq!(tampered_pattern.edges.len() + 1, edge_count_before);
            let tampered_topology =
                EditorState::with_paper(tampered_pattern.clone(), target_paper.clone())
                    .topology_analysis_input(project.project_id)
                    .analyze();
            let tampered_certificate =
                tampered_topology
                    .simulation_snapshot()
                    .and_then(|snapshot| {
                        certify_beginner_fold_path_v1(
                            &tampered_plan,
                            &target_paper,
                            &tampered_pattern,
                            snapshot,
                        )
                    });
            assert_eq!(
                tampered_certificate, None,
                "removing one general count {count} radial edge must fail closed instead of certifying an odd-ray graph"
            );
        }
        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let applied = apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan,
            configured.clone(),
            None,
        )
        .unwrap_or_else(|error| panic!("general count {count} apply failed: {error}"));
        let applied_document_authority = project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.document_authority_sha256)
            .expect("general apply must bind its positive evidence to the final document");
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                project.editor.pattern(),
                project.editor.paper(),
                project.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
        );
        assert!(project.editor.paper().thickness_mm > 0.0);
        assert_eq!(
            project
                .editor
                .beginner_design_profile()
                .generation_constraints,
            configured.generation_constraints
        );
        assert!(
            project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|provenance| provenance.generic_tree.as_ref())
                .is_some()
        );
        assert!(
            project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|provenance| provenance.fold_path_certificate_sha256)
                .is_some(),
            "general count {count} must persist its positive-thickness fold-path certificate"
        );
        let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                project.editor.pattern(),
                project.editor.paper(),
                project.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance
        );
        execute_redo(&mut project, project_id, undone.revision).unwrap();
        assert_eq!(
            project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|provenance| provenance.document_authority_sha256),
            Some(applied_document_authority)
        );
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                project.editor.pattern(),
                project.editor.paper(),
                project.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
        );
        let mut saved = project.document();
        saved.thumbnail_svg = None;
        let bytes = write_project_ori2(&saved).unwrap();
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
        let reopened = ProjectState::from_valid_document(
            restored,
            PathBuf::from(format!("general-{count}.ori2")),
        );
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
                .and_then(|provenance| provenance.document_authority_sha256),
            Some(applied_document_authority)
        );
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                reopened.editor.pattern(),
                reopened.editor.paper(),
                reopened.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
        );
    }
    let mut rejected = ori_domain::BeginnerDesignProfileV1::default();
    rejected.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    rejected.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 8),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 7),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    let before = rejected.clone();
    assert!(!configure_symmetric_profile(
        &mut rejected,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 15,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    ));
    assert_eq!(
        rejected, before,
        "count fifteen must fail closed before mutating the editable profile"
    );
}

#[test]
fn general_count_four_two_supports_apply_tamper_undo_redo_and_reopen() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 4),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    assert!(configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 4,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    ));
    let skeleton_bounds = profile
        .generation_constraints
        .skeleton_segments
        .iter()
        .flat_map(|segment| {
            [
                (segment.start.x_tenths_mm, segment.start.y_tenths_mm),
                (segment.end.x_tenths_mm, segment.end.y_tenths_mm),
            ]
        })
        .fold(
            (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
            |(min_x, max_x, min_y, max_y), (x, y)| {
                (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
            },
        );
    assert_eq!(skeleton_bounds, (-500, 500, -500, 500));
    for (target, (y, x_direction)) in profile.generation_constraints.protrusions.iter_mut().zip([
        (-250, -1_000),
        (-250, 1_000),
        (250, 1_000),
        (250, -1_000),
    ]) {
        target.count = 1;
        target.length_tenths_mm = 250;
        target.root_width_tenths_mm = None;
        target.tip_width_tenths_mm = None;
        target.local_outline_tenths_mm = None;
        target.position_tenths_mm = [0, y, 0];
        target.direction_milli = [x_direction, 0, 0];
        target.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::None;
        target.priority = 100;
    }

    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let configured = temporary_symmetric_profile_for_grid(&profile, point)
        .expect("the exact-quarter singleton targets must remain a valid grid profile");
    assert_eq!(
        configured
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| (
                target.position_tenths_mm,
                target.direction_milli,
                target.length_tenths_mm,
                target.priority,
            ))
            .collect::<Vec<_>>(),
        [
            ([0, -250, 0], [-1_000, 0, 0], 250, 100),
            ([0, -250, 0], [1_000, 0, 0], 250, 100),
            ([0, 250, 0], [1_000, 0, 0], 250, 100),
            ([0, 250, 0], [-1_000, 0, 0], 250, 100),
        ]
    );

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
    .find(|candidate| {
        candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
            && candidate
                .instruction_codes
                .last()
                .is_some_and(|code| code.ends_with(":horizontal"))
    })
    .expect("the four exact-corner semantic rays must generate");
    assert_eq!(radial_corner_support_added_v1(&plan), 2);
    assert!(
        plan.instruction_codes
            .iter()
            .any(|code| code == "bounded_radial_corner_support_v1:added=2:covered=4")
    );
    assert!(beginner_plan_has_radial_corner_support_v1(&plan));
    assert_eq!(
        plan.crease_pattern
            .edges
            .iter()
            .filter(|edge| {
                matches!(
                    edge.kind,
                    ori_domain::EdgeKind::Mountain | ori_domain::EdgeKind::Valley
                )
            })
            .count(),
        6,
        "four semantic corner rays plus two deterministic parity supports enter the six-hinge theorem"
    );
    let witness =
        beginner_contour_placement_witness(&configured.generation_constraints, &plan).unwrap();
    assert_eq!(
        witness
            .generic_feature_bindings
            .iter()
            .map(|binding| (
                binding.generated_feature_id,
                binding.endpoint_count,
                binding.crease_start,
            ))
            .collect::<Vec<_>>(),
        [(1, 1, 2), (2, 1, 3), (3, 1, 4), (4, 1, 5)],
        "the two physical supports must remain outside all four semantic feature slices"
    );
    let semantic_edges = &plan.crease_pattern.edges[2..6];
    let semantic_boundary_ids = semantic_edges
        .iter()
        .flat_map(|edge| [edge.start, edge.end])
        .filter(|id| project.editor.paper().boundary_vertices.contains(id))
        .collect::<HashSet<_>>();
    assert_eq!(
        semantic_boundary_ids.len(),
        4,
        "the semantic rays themselves must cover all four paper corners"
    );

    let (mut target_pattern, target_paper) = materialize_beginner_boundary_splits_v1(
        project.editor.pattern(),
        project.editor.paper(),
        &plan,
    )
    .unwrap();
    for vertex in &plan.crease_pattern.vertices {
        if !target_pattern
            .vertices
            .iter()
            .any(|current| current.id == vertex.id)
        {
            target_pattern.vertices.push(vertex.clone());
        }
    }
    target_pattern
        .edges
        .extend(plan.crease_pattern.edges.iter().cloned());
    assert!(validate_crease_pattern(&target_pattern).is_valid());
    assert!(validate_paper(&target_paper, &target_pattern).is_valid());
    let certificate_topology =
        EditorState::with_paper(target_pattern.clone(), target_paper.clone())
            .topology_analysis_input(project.project_id)
            .analyze();
    let certificate_snapshot = certificate_topology
        .simulation_snapshot()
        .expect("the six-ray support-two graph must be simulation-ready");
    assert!(
        certify_beginner_fold_path_v1(&plan, &target_paper, &target_pattern, certificate_snapshot,)
            .is_some(),
        "the native issuer must certify the valid six-ray support-two graph"
    );

    let mut tampered_plan = plan.clone();
    let removed_support = tampered_plan.crease_pattern.edges.remove(0);
    let mut tampered_pattern = target_pattern;
    tampered_pattern
        .edges
        .retain(|edge| edge.id != removed_support.id);
    let tampered_topology = EditorState::with_paper(tampered_pattern.clone(), target_paper.clone())
        .topology_analysis_input(project.project_id)
        .analyze();
    assert_eq!(
        tampered_topology
            .simulation_snapshot()
            .and_then(|snapshot| {
                certify_beginner_fold_path_v1(
                    &tampered_plan,
                    &target_paper,
                    &tampered_pattern,
                    snapshot,
                )
            }),
        None,
        "removing one parity support must not certify the resulting five-ray graph"
    );
    let before_tampered_apply = project.document();
    let tampered_instance_id = project.instance_id;
    let tampered_project_id = project.project_id;
    let tampered_revision = project.editor.revision();
    let tampered_error = apply_grid_plan_document(
        &mut project,
        tampered_instance_id,
        tampered_project_id,
        tampered_revision,
        tampered_plan,
        configured.clone(),
        None,
    )
    .unwrap_err();
    assert_eq!(
        tampered_error,
        "grid_candidate_path_certificate_invalid".to_owned()
    );
    assert_eq!(project.document(), before_tampered_apply);

    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        revision,
        plan,
        configured.clone(),
        None,
    )
    .expect("the valid support-two plan must apply");
    let applied_pattern = project.editor.pattern().clone();
    let applied_paper = project.editor.paper().clone();
    let applied_profile = project.editor.beginner_design_profile().clone();
    let document_authority = applied_profile
        .generation_provenance
        .as_ref()
        .and_then(|provenance| provenance.document_authority_sha256)
        .expect("support-two apply must bind document authority");
    assert_eq!(
        applied_profile.generation_constraints,
        configured.generation_constraints
    );
    assert_eq!(
        ori_core::beginner_generation_document_authority_status_v1(
            &applied_pattern,
            &applied_paper,
            &applied_profile,
        ),
        ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
    );
    assert!(
        applied_profile
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.fold_path_certificate_sha256)
            .is_some()
    );

    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    assert_eq!(
        ori_core::beginner_generation_document_authority_status_v1(
            project.editor.pattern(),
            project.editor.paper(),
            project.editor.beginner_design_profile(),
        ),
        ori_core::BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance
    );
    execute_redo(&mut project, project_id, undone.revision).unwrap();
    assert_eq!(project.editor.pattern(), &applied_pattern);
    assert_eq!(project.editor.paper(), &applied_paper);
    assert_eq!(project.editor.beginner_design_profile(), &applied_profile);
    assert_eq!(
        project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.document_authority_sha256),
        Some(document_authority)
    );

    let mut saved = project.document();
    saved.thumbnail_svg = None;
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("general-four-support-two.ori2"));
    let mut reopened_document = reopened.document();
    reopened_document.thumbnail_svg = None;
    assert_eq!(reopened_document, saved);
    assert_eq!(reopened.editor.pattern(), &applied_pattern);
    assert_eq!(reopened.editor.paper(), &applied_paper);
    assert_eq!(
        ori_core::beginner_generation_document_authority_status_v1(
            reopened.editor.pattern(),
            reopened.editor.paper(),
            reopened.editor.beginner_design_profile(),
        ),
        ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
    );
}

#[test]
fn explicit_small_general_tree_fails_grid_configuration_without_replacement() {
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 3),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    assert!(configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 3,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    ));
    profile.generation_constraints.skeleton_segments =
        vec![ori_domain::BeginnerSkeletonSegmentV1 {
            id: 77,
            start: ori_domain::BeginnerSkeletonPointV1 {
                x_tenths_mm: 0,
                y_tenths_mm: 0,
            },
            end: ori_domain::BeginnerSkeletonPointV1 {
                x_tenths_mm: 8,
                y_tenths_mm: 8,
            },
            thickness_tenths_mm: 1,
        }];
    let before = profile.clone();
    assert_eq!(
        temporary_symmetric_profile_for_grid(
            &profile,
            ori_domain::beginner_parameter_grid_v1()[13],
        ),
        Err("beginner_parameter_grid_profile_invalid".to_owned())
    );
    assert_eq!(profile, before);
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
