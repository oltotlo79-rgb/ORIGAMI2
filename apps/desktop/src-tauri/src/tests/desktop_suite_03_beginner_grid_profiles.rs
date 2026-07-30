#[test]
fn grid_profile_is_temporary_canonical_and_does_not_change_free_parameters() {
    let _serial = serial_beginner_grid_test();
    let mut source = ori_domain::BeginnerDesignProfileV1::default();
    source.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    source.generation_constraints.target_parts = vec![
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Head,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Torso,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 4,
        },
    ];
    let before = source.clone();
    let point = ori_domain::beginner_parameter_grid_v1()[26];
    let temporary = temporary_symmetric_profile_for_grid(&source, point).unwrap();

    assert_eq!(source, before);
    assert_eq!(
        temporary.generation_constraints.detail_level,
        ori_domain::BeginnerDetailLevelV1::Detailed
    );
    assert_eq!(temporary.generation_constraints.protrusions.len(), 1);
    assert_eq!(
        temporary.generation_constraints.protrusions[0].length_tenths_mm,
        450
    );
    assert_eq!(
        temporary.generation_constraints.protrusions[0].thickness_tenths_mm,
        160
    );
    let mut forged = point;
    forged.scale_percent = 44;
    assert_eq!(
        temporary_symmetric_profile_for_grid(&source, forged),
        Err("beginner_parameter_grid_point_invalid".to_owned())
    );
    let mut model_source = source.clone();
    configure_symmetric_profile(
        &mut model_source,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 4,
            scale_percent: 25,
            spacing_percent: 35,
        },
        27,
        50,
    );
    model_source.generation_constraints.protrusions[0].length_tenths_mm = 270;
    model_source.generation_constraints.protrusions[0].thickness_tenths_mm = 100;
    model_source.generation_constraints.target_asset =
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel {
            asset_id: AssetId::new(),
        });
    let model_candidate = temporary_symmetric_profile_for_grid(
        &model_source,
        ori_domain::beginner_parameter_grid_v1()[0],
    )
    .unwrap();
    assert_eq!(
        model_candidate.generation_constraints.protrusions[0].length_tenths_mm,
        100
    );
    assert_eq!(
        model_candidate.generation_constraints.protrusions[0].thickness_tenths_mm,
        40
    );

    let mut project = initial_project_state();
    for point in ori_domain::beginner_parameter_grid_v1() {
        let plans = grid_template_plan(
            project.project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            &source,
            point,
        )
        .unwrap();
        assert!(!plans.is_empty());
        assert!(plans.len() <= ori_domain::MAX_BEGINNER_GENERATED_CANDIDATES_V1);
    }
    let point = ori_domain::beginner_parameter_grid_v1()[26];
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &source,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase)
    .unwrap();
    assert_eq!(
        plan.crease_pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Valley)
            .count(),
        8,
        "the default valley-first choice must cover four semantic rays and four support creases"
    );
    assert!(
        plan.crease_pattern
            .edges
            .iter()
            .all(|edge| edge.kind != EdgeKind::Mountain),
        "corner support must not introduce a fold assignment outside the valley-first choice"
    );
    assert!(
        plan.instruction_codes
            .iter()
            .any(|code| code == "bounded_radial_corner_support_v1:added=4:covered=4"),
        "the support/semantic distinction must be explicitly bound"
    );
    assert!(
        plan.crease_pattern.edges[..4].iter().all(|edge| project
            .editor
            .paper()
            .boundary_vertices
            .contains(&edge.end)),
        "the deterministic four-corner support prefix must use the live paper corners"
    );
    assert_eq!(
        plan.crease_pattern.edges[4..]
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>(),
        (0..4)
            .map(|index| EdgeId::derive_v5(
                project.project_id,
                format!(
                    "beginner-plan-{:?}-e-{index}",
                    ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase
                )
                .as_bytes(),
            ))
            .collect::<Vec<_>>(),
        "the exact four semantic leg bindings must remain the suffix after support insertion"
    );
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let revision = project.editor.revision();
    let snapshot = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        revision,
        plan.clone(),
        temporary.clone(),
        None,
    )
    .unwrap();
    assert_eq!(snapshot.revision, revision + 1);
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan,
            temporary,
            None,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, snapshot.revision).unwrap();
    assert_eq!(undone.revision, snapshot.revision + 1);
    let redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    assert_eq!(redone.revision, undone.revision + 1);
}

#[test]
fn complete_insect_grid_preserves_all_five_pair_dimensions_and_bindings() {
    let _serial = serial_beginner_grid_test();
    let mut source = ori_domain::BeginnerDesignProfileV1::default();
    source.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Insect);
    source.generation_constraints.target_parts = vec![
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Head,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Torso,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Wing,
            count: 2,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Antenna,
            count: 2,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 6,
        },
    ];
    configure_symmetric_profile(
        &mut source,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 10,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    );
    for (index, target) in source
        .generation_constraints
        .protrusions
        .iter_mut()
        .enumerate()
    {
        target.length_tenths_mm = if index == 0 {
            1
        } else {
            270 + index as u32 * 27
        };
        target.thickness_tenths_mm = if index == 0 {
            1
        } else {
            50 + index as u16 * 10
        };
        target.direction_milli[0] = -target.direction_milli[0];
        target.direction_milli[1] = -target.direction_milli[1];
    }
    source.generation_constraints.protrusions.reverse();
    let point = ori_domain::beginner_parameter_grid_v1()[26];
    let temporary = temporary_symmetric_profile_for_grid(&source, point).unwrap();

    assert_eq!(temporary.generation_constraints.protrusions.len(), 5);
    assert!(ori_domain::insect_complete_bindings_v1(&temporary.generation_constraints).is_some());
    for (index, target) in temporary
        .generation_constraints
        .protrusions
        .iter()
        .enumerate()
    {
        assert_eq!(target.id, index as u16 + 1);
        let source_length = if index == 0 {
            1
        } else {
            270 + index as u32 * 27
        };
        let source_thickness = if index == 0 {
            1
        } else {
            50 + index as u16 * 10
        };
        assert_eq!(target.length_tenths_mm, (source_length * 45 / 27).max(1));
        assert_eq!(
            target.thickness_tenths_mm,
            (source_thickness * 80 / 50).max(1)
        );
    }

    let mut generatable = source.clone();
    for target in &mut generatable.generation_constraints.protrusions {
        target.length_tenths_mm = 270;
    }
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &generatable,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase)
    .unwrap();
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let configured = temporary_symmetric_profile_for_grid(&generatable, point).unwrap();
    let profile_revision = project.editor.revision();
    let profile_saved = execute_command(
        &mut project,
        project_id,
        profile_revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(generatable),
        },
    )
    .unwrap();
    let revision = profile_saved.revision;
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        revision,
        plan.clone(),
        configured.clone(),
        None,
    )
    .unwrap();
    let generated_steps = &project.editor.instruction_timeline().steps;
    assert_eq!(generated_steps.len(), 1);
    assert_eq!(
        generated_steps[0].title,
        "Complete composite insect grid candidate"
    );
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan,
            configured,
            None,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    let redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    assert_eq!(redone.revision, undone.revision + 1);
    let saved = project.document();
    let bytes = write_project_ori2(&saved).expect("persist complete insect grid apply");
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default())
        .expect("restore complete insect grid apply");
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("complete-insect-grid.ori2"));
    assert_eq!(reopened.document(), saved);
    assert!(
        ori_domain::insect_complete_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints
        )
        .is_some()
    );
    let score_input = ori_domain::BeginnerCandidateInputV1 {
        vertex_count: project.editor.pattern().vertices.len(),
        edge_count: project.editor.pattern().edges.len(),
        crease_count: project
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
            .count(),
        target_approximation_score: ori_domain::beginner_target_approximation_score_v1(
            &project
                .editor
                .beginner_design_profile()
                .generation_constraints,
        ),
    };
    assert_eq!(
        ori_domain::score_beginner_candidates_v1(
            score_input,
            project.editor.beginner_design_profile()
        ),
        ori_domain::score_beginner_candidates_v1(
            score_input,
            reopened.editor.beginner_design_profile()
        )
    );
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
}
