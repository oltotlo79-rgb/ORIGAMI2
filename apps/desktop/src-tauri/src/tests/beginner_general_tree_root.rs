use super::*;

fn id_root_general_tree_profile() -> ori_domain::BeginnerDesignProfileV1 {
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
    for segment in &mut profile.generation_constraints.skeleton_segments {
        let start = (segment.start.x_tenths_mm, segment.start.y_tenths_mm);
        let end = (segment.end.x_tenths_mm, segment.end.y_tenths_mm);
        if end < start {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
    }
    let first_id = profile.generation_constraints.skeleton_segments[0].id;
    profile.generation_constraints.skeleton_segments[0].id =
        profile.generation_constraints.skeleton_segments[1].id;
    profile.generation_constraints.skeleton_segments[1].id = first_id;
    profile
        .generation_constraints
        .skeleton_segments
        .sort_unstable_by_key(|segment| segment.id);
    profile
}

#[test]
fn generic_tree_uses_lowest_id_canonical_start_as_compatible_root() {
    let profile = id_root_general_tree_profile();
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let mut project = initial_project_state();
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
    let canonical_root = (
        plan.skeleton_segments[0].start.x_tenths_mm,
        plan.skeleton_segments[0].start.y_tenths_mm,
    );
    let global_minimum = plan
        .skeleton_segments
        .iter()
        .flat_map(|segment| {
            [
                (segment.start.x_tenths_mm, segment.start.y_tenths_mm),
                (segment.end.x_tenths_mm, segment.end.y_tenths_mm),
            ]
        })
        .min()
        .unwrap();
    assert_eq!(plan.skeleton_segments[0].id, 1);
    assert_eq!(canonical_root, (0, 500));
    assert_eq!(global_minimum, (-500, 0));
    assert_ne!(canonical_root, global_minimum);

    let baseline_witness =
        beginner_contour_placement_witness(&profile.generation_constraints, &plan).unwrap();
    assert_eq!(baseline_witness.skeleton_branch_bindings[0].segment_id, 1);
    assert_eq!(
        baseline_witness.skeleton_tree_authority_sha256,
        [
            0xc0, 0x53, 0xda, 0x21, 0xc1, 0x09, 0xb8, 0xe0, 0x4f, 0x85, 0xa2, 0x16, 0xa8, 0x1f,
            0x94, 0xd3, 0x99, 0xbc, 0x17, 0x9e, 0x2b, 0xf9, 0xda, 0xd5, 0x55, 0x3b, 0xcd, 0x73,
            0xc7, 0xf7, 0x9f, 0xa8,
        ],
        "the canonical semantic tree authority excludes project-scoped generated UUIDs"
    );
    assert_eq!(
        baseline_witness
            .skeleton_branch_bindings
            .iter()
            .map(|binding| (
                binding.segment_id,
                binding.parent_segment_id,
                binding.parent_endpoint,
                binding.child_endpoint,
                binding.generated_feature_ids.as_slice(),
            ))
            .collect::<Vec<_>>(),
        [
            (1, None, None, None, [2, 3].as_slice()),
            (2, Some(1), Some("start"), Some("end"), [].as_slice()),
            (3, Some(1), Some("start"), Some("end"), [1].as_slice()),
        ],
    );
    let baseline_witness_json = serde_json::to_vec(&baseline_witness).unwrap();
    let mut all_reversed = profile.clone();
    for segment in &mut all_reversed.generation_constraints.skeleton_segments {
        std::mem::swap(&mut segment.start, &mut segment.end);
    }
    let mut shuffled_reversed = all_reversed.clone();
    let reversed_segments = all_reversed
        .generation_constraints
        .skeleton_segments
        .clone();
    shuffled_reversed.generation_constraints.skeleton_segments =
        [2, 0, 1].map(|index| reversed_segments[index]).to_vec();
    for variant in [&all_reversed, &shuffled_reversed] {
        let variant_plans = grid_template_plan(
            project.project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            variant,
            point,
        )
        .unwrap();
        assert_eq!(variant_plans, baseline_plans);
        let variant_plan = variant_plans
            .iter()
            .find(|candidate| {
                candidate.kind
                    == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
            })
            .unwrap();
        let witness =
            beginner_contour_placement_witness(&variant.generation_constraints, variant_plan)
                .unwrap();
        assert_eq!(serde_json::to_vec(&witness).unwrap(), baseline_witness_json);
    }

    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let revision = project.editor.revision();
    let saved = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(shuffled_reversed),
        },
    )
    .unwrap();
    let configured =
        temporary_symmetric_profile_for_grid(project.editor.beginner_design_profile(), point)
            .unwrap();
    apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved.revision,
        plan.clone(),
        configured,
        None,
    )
    .unwrap();
    let generic_tree = project
        .editor
        .beginner_design_profile()
        .generation_provenance
        .as_ref()
        .and_then(|provenance| provenance.generic_tree.as_ref())
        .unwrap();
    assert_eq!(
        generic_tree.tree_topology_sha256,
        <[u8; 32]>::from(sha2::Sha256::digest(
            serde_json::to_vec(&plan.skeleton_segments).unwrap()
        ))
    );
    assert_eq!(
        generic_tree.normalized_length_ratios,
        [1_000_000, 1_000_000, 2_000_000]
    );
    assert_eq!(
        generic_tree
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
            .collect::<Vec<_>>(),
        [
            ("tree-river-0001", 0, "valley"),
            ("tree-river-0002", 0, "mountain"),
            ("tree-river-0003", 0, "valley"),
        ],
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
    let proposal = generic_tree.instruction_proposal.as_ref().unwrap();
    assert_eq!(proposal.topology_sha256, generic_tree.tree_topology_sha256);
    assert!(!proposal.authorizes_apply);
    assert!(!proposal.physical_motion_proof);
}
