fn radial_corner_support_added_v1(plan: &ori_domain::BeginnerGeneratedPlanV1) -> usize {
    let support_codes = plan
        .instruction_codes
        .iter()
        .filter(|code| code.starts_with("bounded_radial_corner_support_v1:"))
        .collect::<Vec<_>>();
    let [support_code] = support_codes.as_slice() else {
        panic!("one canonical radial-corner support instruction is required");
    };
    let (added, covered) = support_code
        .strip_prefix("bounded_radial_corner_support_v1:added=")
        .and_then(|payload| payload.split_once(":covered="))
        .expect("canonical radial-corner support instruction");
    let added = added.parse::<usize>().expect("bounded support count");
    assert!(added <= 5);
    assert_eq!(covered, "4");
    added
}

#[test]
fn grid_apply_accepts_only_named_sufficient_proof_authorities() {
    let assessment = |proof_scope, apply_allowed, reason| BeginnerGeneratedPlanAssessment {
        kind: ori_domain::BeginnerGeneratedPlanKindV1::DiagonalFold,
        expected_candidate_edge_id: EdgeId::new(),
        proof_scope,
        apply_allowed,
        reason,
        shape_approximation_score: None,
        shape_difference_reason: None,
        component_shape_comparison: None,
    };
    assert!(beginner_assessment_has_sufficient_apply_authority_v1(
        &assessment("sufficient", true, "native_fold_path_certified")
    ));
    assert!(beginner_assessment_has_sufficient_apply_authority_v1(
        &assessment("sufficient", true, "global_flat_foldability_proven")
    ));
    for rejected in [
        assessment("necessary", true, "native_fold_path_certified"),
        assessment("sufficient", false, "native_fold_path_certified"),
        assessment("sufficient", true, "necessary_conditions_satisfied"),
    ] {
        assert!(!beginner_assessment_has_sufficient_apply_authority_v1(
            &rejected
        ));
    }
}

#[test]
fn grid_refined_point_domain_metadata_and_authority_are_fail_closed() {
    let grid = ori_domain::beginner_parameter_grid_v1();
    let seed = grid[13];
    let mut lower_bound = seed;
    lower_bound.scale_percent = 10;
    lower_bound.spacing_percent = 20;
    assert!(beginner_grid_refined_point_is_in_domain_v1(
        &grid,
        lower_bound
    ));
    let mut upper_bound = seed;
    upper_bound.scale_percent = 45;
    upper_bound.spacing_percent = 80;
    assert!(beginner_grid_refined_point_is_in_domain_v1(
        &grid,
        upper_bound
    ));
    for invalid in [
        ori_domain::BeginnerParameterGridPointV1 {
            scale_percent: 9,
            ..seed
        },
        ori_domain::BeginnerParameterGridPointV1 {
            scale_percent: 46,
            ..seed
        },
        ori_domain::BeginnerParameterGridPointV1 {
            spacing_percent: 19,
            ..seed
        },
        ori_domain::BeginnerParameterGridPointV1 {
            spacing_percent: 81,
            ..seed
        },
        ori_domain::BeginnerParameterGridPointV1 { id: 27, ..seed },
        ori_domain::BeginnerParameterGridPointV1 {
            detail_level: match seed.detail_level {
                ori_domain::BeginnerDetailLevelV1::Simple => {
                    ori_domain::BeginnerDetailLevelV1::Standard
                }
                _ => ori_domain::BeginnerDetailLevelV1::Simple,
            },
            ..seed
        },
    ] {
        assert!(!beginner_grid_refined_point_is_in_domain_v1(&grid, invalid));
    }
    let mut same_refined_tuple = seed;
    same_refined_tuple.id = seed.id.saturating_add(1);
    assert!(beginner_grid_refined_points_duplicate_v1(
        seed,
        same_refined_tuple
    ));
    let mut same_seed = seed;
    same_seed.scale_percent = if seed.scale_percent < 45 {
        seed.scale_percent + 1
    } else {
        seed.scale_percent - 1
    };
    assert!(beginner_grid_refined_points_duplicate_v1(seed, same_seed));
    let mut distinct = same_refined_tuple;
    distinct.spacing_percent = if seed.spacing_percent < 80 {
        seed.spacing_percent + 1
    } else {
        seed.spacing_percent - 1
    };
    assert!(!beginner_grid_refined_points_duplicate_v1(seed, distinct));

    assert!(beginner_grid_refinement_metadata_is_valid_v1(
        seed, seed, true, 1, 0, 5
    ));
    let mut improved = seed;
    improved.scale_percent = if seed.scale_percent <= 41 {
        seed.scale_percent + 4
    } else {
        seed.scale_percent - 4
    };
    assert!(beginner_grid_refinement_metadata_is_valid_v1(
        seed, improved, true, 0, 1, 5
    ));
    assert!(!beginner_grid_refinement_metadata_is_valid_v1(
        seed, improved, true, 0, 0, 5
    ));
    assert!(!beginner_grid_refinement_metadata_is_valid_v1(
        seed, improved, true, 0, 2, 5
    ));
    let mut unreachable_in_one_start = seed;
    unreachable_in_one_start.spacing_percent = if seed.spacing_percent <= 73 {
        seed.spacing_percent + 7
    } else {
        seed.spacing_percent - 7
    };
    assert!(!beginner_grid_refinement_metadata_is_valid_v1(
        seed,
        unreachable_in_one_start,
        true,
        0,
        1,
        5
    ));
    assert!(!beginner_grid_refinement_metadata_is_valid_v1(
        seed, seed, true, 9, 0, 5
    ));
    assert!(!beginner_grid_refinement_metadata_is_valid_v1(
        seed, seed, true, 1, 0, 4
    ));
    assert!(beginner_grid_refinement_metadata_is_valid_v1(
        seed, seed, false, 0, 0, 1
    ));
    assert!(!beginner_grid_refinement_metadata_is_valid_v1(
        seed, improved, false, 0, 0, 1
    ));

    let expected_edge = EdgeId::new();
    let plan = ori_domain::BeginnerGeneratedPlanV1 {
        schema_version: ori_domain::BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
        kind: ori_domain::BeginnerGeneratedPlanKindV1::DiagonalFold,
        crease_pattern: ori_domain::CreasePattern {
            vertices: Vec::new(),
            edges: Vec::new(),
        },
        instruction_codes: vec!["authority-fixture".to_owned()],
        target_parts: Vec::new(),
        skeleton_segments: Vec::new(),
        target_asset: None,
        semantic_landmark_provenance: None,
    };
    let assessment = BeginnerGeneratedPlanAssessment {
        kind: plan.kind,
        expected_candidate_edge_id: expected_edge,
        proof_scope: "sufficient",
        apply_allowed: true,
        reason: "native_fold_path_certified",
        shape_approximation_score: None,
        shape_difference_reason: None,
        component_shape_comparison: None,
    };
    let authority = BeginnerGridCandidateAuthorityV1 {
        point: seed,
        expected_candidate_edge_id: expected_edge,
        topology_authority_hash: [3; 32],
        plan_sha256: beginner_grid_plan_authority_sha256_v1(&plan).unwrap(),
        assessment_sha256: beginner_grid_assessment_authority_sha256_v1(&assessment).unwrap(),
        refinement_iterations: 0,
        strict_improvements: 0,
        refinement_starts: 1,
    };
    assert!(
        beginner_grid_candidate_authority_matches_result_v1(&authority, &plan, &assessment)
            .unwrap()
    );
    let mut tampered_plan_authority = authority.clone();
    tampered_plan_authority.plan_sha256[0] ^= 1;
    assert!(
        !beginner_grid_candidate_authority_matches_result_v1(
            &tampered_plan_authority,
            &plan,
            &assessment,
        )
        .unwrap()
    );
    let mut tampered_assessment_authority = authority;
    tampered_assessment_authority.assessment_sha256[0] ^= 1;
    assert!(
        !beginner_grid_candidate_authority_matches_result_v1(
            &tampered_assessment_authority,
            &plan,
            &assessment,
        )
        .unwrap()
    );
}

#[test]
fn complete_animal_grid_apply_replay_undo_redo_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 8,
            scale_percent: 25,
            spacing_percent: 50,
        },
        25,
        50,
    );
    assert!(ori_domain::animal_complete_bindings_v1(&profile.generation_constraints).is_some());

    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let apply_profile = profile.clone();
    for target in &mut profile.generation_constraints.protrusions {
        target.length_tenths_mm = 270 + u32::from(target.id) * 10;
        target.thickness_tenths_mm = 50 + target.id;
        target.direction_milli[0] = -target.direction_milli[0];
        target.direction_milli[1] = -target.direction_milli[1];
    }
    profile.generation_constraints.protrusions.reverse();
    let temporary = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    assert_eq!(
        temporary
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| target.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    for target in &temporary.generation_constraints.protrusions {
        assert_eq!(
            target.length_tenths_mm,
            ((270 + u32::from(target.id) * 10) * u32::from(point.scale_percent) / 27).max(1)
        );
        assert_eq!(
            target.thickness_tenths_mm,
            ((50 + target.id) * u16::from(point.spacing_percent) / 50).max(1)
        );
    }
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &apply_profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase)
    .unwrap();
    let mut mixed_project = initial_project_state();
    let mut mixed_assignment_plan = grid_template_plan(
        mixed_project.project_id,
        mixed_project.editor.pattern(),
        &mixed_project.editor.paper().boundary_vertices,
        &apply_profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|candidate| candidate.kind == plan.kind)
    .expect("mixed-assignment complete animal plan");
    let mixed_supports = radial_corner_support_added_v1(&mixed_assignment_plan);
    let mut physical_edges = mixed_assignment_plan
        .crease_pattern
        .edges
        .iter_mut()
        .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .collect::<Vec<_>>();
    assert_eq!(physical_edges.len(), 10 + mixed_supports);
    physical_edges[0].kind = EdgeKind::Mountain;
    assert!(
        physical_edges
            .iter()
            .skip(1)
            .any(|edge| edge.kind == EdgeKind::Valley)
    );
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let configured = temporary_symmetric_profile_for_grid(&apply_profile, point).unwrap();
    let mixed_project_id = mixed_project.project_id;
    let mixed_instance_id = mixed_project.instance_id;
    let mixed_profile = execute_command(
        &mut mixed_project,
        mixed_project_id,
        0,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(apply_profile.clone()),
        },
    )
    .unwrap();
    apply_grid_plan_document(
        &mut mixed_project,
        mixed_instance_id,
        mixed_project_id,
        mixed_profile.revision,
        mixed_assignment_plan,
        configured.clone(),
        None,
    )
    .expect(
        "mixed mountain/valley radial fan with explicit corner support keeps an exact certified endpoint path",
    );
    assert!(
        mixed_project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.fold_path_certificate_sha256)
            .is_some()
    );
    assert!(
        mixed_project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .is_some_and(|provenance| provenance
                .confidence_reasons
                .iter()
                .any(|reason| reason == "bounded_radial_corner_support_v1"))
    );
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(apply_profile),
        },
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan.clone(),
            configured.clone(),
            None,
        )
        .is_err()
    );
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
    let applied_document_authority = project
        .editor
        .beginner_design_profile()
        .generation_provenance
        .as_ref()
        .and_then(|provenance| provenance.document_authority_sha256)
        .expect("complete animal apply must bind its positive evidence to the final document");
    assert_eq!(
        ori_core::beginner_generation_document_authority_status_v1(
            project.editor.pattern(),
            project.editor.paper(),
            project.editor.beginner_design_profile(),
        ),
        ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
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
    let saved = project.document();
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("complete-animal.ori2"));
    assert_eq!(reopened.document(), saved);
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
    assert!(
        ori_domain::animal_complete_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints
        )
        .is_some()
    );
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
}

#[test]
fn complete_winged_animal_grid_apply_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
        (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    let estimate =
        ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints).unwrap();
    assert_eq!(estimate.protrusion_count, 10);
    configure_symmetric_profile(
        &mut profile,
        estimate,
        estimate.scale_percent,
        estimate.spacing_percent,
    );
    let binding = ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
        .expect("strict five-binding winged animal");
    assert_eq!(binding.wing_pair_protrusion_id, 5);
    let point = ori_domain::beginner_parameter_grid_v1()[13];
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
    .find(|plan| {
        plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
    })
    .expect("winged animal grid plan");
    let support_count = radial_corner_support_added_v1(&plan);
    assert_eq!(plan.crease_pattern.vertices.len(), 11 + support_count);
    assert_eq!(plan.crease_pattern.edges.len(), 10 + support_count);
    let semantic_edge_ids = plan
        .crease_pattern
        .edges
        .iter()
        .skip(support_count)
        .take(10)
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    assert_eq!(
        semantic_edge_ids,
        (0..10)
            .map(|index| {
                EdgeId::derive_v5(
                    project.project_id,
                    format!(
                        "beginner-plan-{:?}-e-{index}",
                        ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
                    )
                    .as_bytes(),
                )
            })
            .collect::<Vec<_>>(),
        "the exact ear/wing pairs must retain the canonical semantic edge slice after support"
    );
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let configured = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    let cancel_generation = ProjectId::new();
    let cancel_work = Arc::new(BeginnerGridWork::default());
    beginner_grid_work()
        .lock()
        .unwrap()
        .insert(cancel_generation, Arc::clone(&cancel_work));
    cancel_beginner_parameter_grid(cancel_generation).unwrap();
    assert!(cancel_work.cancelled.load(Ordering::Acquire));
    beginner_grid_work().lock().unwrap().clear();
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
    .unwrap();
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
    let applied_certificate = project
        .editor
        .beginner_design_profile()
        .generation_provenance
        .as_ref()
        .and_then(|provenance| provenance.fold_path_certificate_sha256)
        .expect("winged animal apply must persist its bounded graph path certificate");
    let applied_document_authority = project
        .editor
        .beginner_design_profile()
        .generation_provenance
        .as_ref()
        .and_then(|provenance| provenance.document_authority_sha256)
        .expect("winged animal apply must bind its positive evidence to the final document");
    assert_eq!(
        ori_core::beginner_generation_document_authority_status_v1(
            project.editor.pattern(),
            project.editor.paper(),
            project.editor.beginner_design_profile(),
        ),
        ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
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
    let reopened = ProjectState::from_valid_document(restored, PathBuf::from("winged-animal.ori2"));
    assert_eq!(
        reopened.editor.beginner_design_profile(),
        &saved.beginner_design_profile
    );
    assert!(
        ori_domain::animal_complete_winged_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints,
        )
        .is_some()
    );
    assert_eq!(
        reopened
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|provenance| provenance.fold_path_certificate_sha256),
        Some(applied_certificate),
        "the exact-pair certificate must survive undo/redo and archive reopen"
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

#[test]
fn standalone_six_leg_estimate_configures_three_strict_pairs() {
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Insect);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 6),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    let estimate =
        ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints).unwrap();
    assert_eq!(estimate.protrusion_count, 6);

    configure_symmetric_profile(
        &mut profile,
        estimate,
        estimate.scale_percent,
        estimate.spacing_percent,
    );
    let configured = profile.clone();
    configure_symmetric_profile(
        &mut profile,
        estimate,
        estimate.scale_percent,
        estimate.spacing_percent,
    );
    assert_eq!(profile, configured);
    assert_eq!(
        ori_domain::insect_three_pair_bindings_v1(&profile.generation_constraints)
            .unwrap()
            .map(|binding| (
                binding.pair_index,
                binding.protrusion_id,
                binding.center_y_tenths_mm,
            )),
        [(0, 1, -250), (1, 2, 0), (2, 3, 250)]
    );
    assert!(
        profile
            .generation_constraints
            .protrusions
            .iter()
            .all(|target| target.count == 2
                && target.symmetry == ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
                && target.direction_milli == [1000, 0, 0])
    );
    assert_eq!(
        ori_domain::beginner_target_approximation_score_v1(&profile.generation_constraints),
        92
    );

    let project = initial_project_state();
    let plans = ori_domain::generate_beginner_plans_v1(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile.generation_constraints,
    )
    .unwrap();
    assert_eq!(
        plans[0].kind,
        ori_domain::BeginnerGeneratedPlanKindV1::SymmetricSixLegBase
    );
    let support_count = radial_corner_support_added_v1(&plans[0]);
    assert_eq!(plans[0].crease_pattern.vertices.len(), 13 + support_count);
    assert_eq!(plans[0].crease_pattern.edges.len(), 12 + support_count);
}

#[test]
fn grid_apply_authority_binds_every_identity_dimension_and_single_consume() {
    let work = BeginnerGridWork::default();
    work.terminal.store(1, Ordering::Release);
    let token = ProjectId::new();
    work.authority_token.set(token).unwrap();
    let project_instance_id = ProjectId::new();
    let project_id = ProjectId::new();
    let revision = 7;
    let profile_sha256 = [3; 32];
    let grid = ori_domain::beginner_parameter_grid_v1();
    let grid_hash = ori_domain::beginner_parameter_grid_hash_v1(&grid);
    let point = grid[13];
    let candidate_edge_id = EdgeId::new();
    let topology_authority_hash = [5; 32];
    let authority = BeginnerGridApplyAuthorityV1 {
        authority_token: token,
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        grid_hash,
        candidates: vec![BeginnerGridCandidateAuthorityV1 {
            point,
            expected_candidate_edge_id: candidate_edge_id,
            topology_authority_hash,
            plan_sha256: [6; 32],
            assessment_sha256: [7; 32],
            refinement_iterations: 0,
            strict_improvements: 0,
            refinement_starts: 1,
        }],
    };
    let allows = |work: &BeginnerGridWork,
                  token,
                  instance,
                  project,
                  revision,
                  profile,
                  hash,
                  point,
                  edge,
                  topology| {
        beginner_grid_authority_allows_candidate_v1(
            work, &authority, token, instance, project, revision, profile, hash, point, edge,
            topology,
        )
    };
    assert!(allows(
        &work,
        token,
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        grid_hash,
        point,
        candidate_edge_id,
        topology_authority_hash,
    ));
    work.terminal.store(2, Ordering::Release);
    assert!(!allows(
        &work,
        token,
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        grid_hash,
        point,
        candidate_edge_id,
        topology_authority_hash,
    ));
    work.terminal.store(1, Ordering::Release);
    assert!(!allows(
        &work,
        ProjectId::new(),
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        grid_hash,
        point,
        candidate_edge_id,
        topology_authority_hash,
    ));
    for (instance, project, candidate_revision, profile, candidate_point, edge, topology) in [
        (
            ProjectId::new(),
            project_id,
            revision,
            profile_sha256,
            point,
            candidate_edge_id,
            topology_authority_hash,
        ),
        (
            project_instance_id,
            ProjectId::new(),
            revision,
            profile_sha256,
            point,
            candidate_edge_id,
            topology_authority_hash,
        ),
        (
            project_instance_id,
            project_id,
            revision + 1,
            profile_sha256,
            point,
            candidate_edge_id,
            topology_authority_hash,
        ),
        (
            project_instance_id,
            project_id,
            revision,
            [4; 32],
            point,
            candidate_edge_id,
            topology_authority_hash,
        ),
        (
            project_instance_id,
            project_id,
            revision,
            profile_sha256,
            grid[12],
            candidate_edge_id,
            topology_authority_hash,
        ),
        (
            project_instance_id,
            project_id,
            revision,
            profile_sha256,
            point,
            EdgeId::new(),
            topology_authority_hash,
        ),
        (
            project_instance_id,
            project_id,
            revision,
            profile_sha256,
            point,
            candidate_edge_id,
            [6; 32],
        ),
    ] {
        assert!(!allows(
            &work,
            token,
            instance,
            project,
            candidate_revision,
            profile,
            grid_hash,
            candidate_point,
            edge,
            topology,
        ));
    }
    let mut changed_grid = grid;
    changed_grid[0].scale_percent += 1;
    assert!(!allows(
        &work,
        token,
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        ori_domain::beginner_parameter_grid_hash_v1(&changed_grid),
        point,
        candidate_edge_id,
        topology_authority_hash,
    ));
    work.apply_consumed.store(true, Ordering::Release);
    assert!(!allows(
        &work,
        token,
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        grid_hash,
        point,
        candidate_edge_id,
        topology_authority_hash,
    ));

    let reused_work = BeginnerGridWork::default();
    let replacement_token = ProjectId::new();
    reused_work.authority_token.set(replacement_token).unwrap();
    let replacement_authority = BeginnerGridApplyAuthorityV1 {
        authority_token: replacement_token,
        ..authority
    };
    assert!(!beginner_grid_authority_allows_candidate_v1(
        &reused_work,
        &replacement_authority,
        token,
        project_instance_id,
        project_id,
        revision,
        profile_sha256,
        grid_hash,
        point,
        candidate_edge_id,
        topology_authority_hash,
    ));
}

#[test]
fn completed_grid_cancellation_is_persistent_idempotent_and_fail_closed() {
    let _serial = serial_beginner_grid_test();
    beginner_grid_work().lock().unwrap().clear();
    let request_generation_id = ProjectId::new();
    let work = Arc::new(BeginnerGridWork::default());
    run_registered_beginner_grid_work_v1(request_generation_id, &work, || Ok(())).unwrap();
    assert_eq!(work.terminal.load(Ordering::Acquire), 1);
    assert!(!work.cancelled.load(Ordering::Acquire));
    cancel_beginner_parameter_grid(request_generation_id).unwrap();
    assert!(work.cancelled.load(Ordering::Acquire));
    assert_eq!(work.terminal.load(Ordering::Acquire), 2);
    assert_eq!(
        get_beginner_parameter_grid_progress(request_generation_id)
            .unwrap()
            .terminal_state,
        "cancelled"
    );
    cancel_beginner_parameter_grid(request_generation_id).unwrap();
    assert!(work.cancelled.load(Ordering::Acquire));

    let consumed_generation_id = ProjectId::new();
    let consumed_work = Arc::new(BeginnerGridWork::default());
    run_registered_beginner_grid_work_v1(consumed_generation_id, &consumed_work, || Ok(()))
        .unwrap();
    consumed_work.apply_consumed.store(true, Ordering::Release);
    assert_eq!(
        cancel_beginner_parameter_grid(consumed_generation_id),
        Err("grid_generation_already_applied".to_owned())
    );
    assert!(!consumed_work.cancelled.load(Ordering::Acquire));
    assert_eq!(consumed_work.terminal.load(Ordering::Acquire), 1);

    let failed_generation_id = ProjectId::new();
    let failed_work = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        run_registered_beginner_grid_work_v1(failed_generation_id, &failed_work, || {
            Err::<(), _>("expected failure".to_owned())
        }),
        Err("expected failure".to_owned())
    );
    assert_eq!(failed_work.terminal.load(Ordering::Acquire), 3);
    assert_eq!(
        cancel_beginner_parameter_grid(failed_generation_id),
        Err("grid_generation_not_running".to_owned())
    );
    beginner_grid_work().lock().unwrap().clear();
}

#[test]
fn grid_terminal_history_is_bounded_and_generation_ids_remain_reserved_after_eviction() {
    let _serial = serial_beginner_grid_test();
    beginner_grid_work().lock().unwrap().clear();
    let mut generations =
        Vec::with_capacity(beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1);
    for _ in 0..beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1 {
        let generation = ProjectId::new();
        let work = Arc::new(BeginnerGridWork::default());
        run_registered_beginner_grid_work_v1(generation, &work, || Ok(())).unwrap();
        assert_eq!(work.terminal.load(Ordering::Acquire), 1);
        assert!(!work.registration_active.load(Ordering::Acquire));
        generations.push(generation);
    }
    assert_eq!(
        beginner_grid_work().lock().unwrap().len(),
        beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1
    );

    let still_reserved = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(generations[1], &still_reserved)
            .err()
            .as_deref(),
        Some("grid_generation_reused")
    );

    let replacement_generation = ProjectId::new();
    let replacement_work = Arc::new(BeginnerGridWork::default());
    run_registered_beginner_grid_work_v1(replacement_generation, &replacement_work, || Ok(()))
        .unwrap();
    let registry = beginner_grid_work().lock().unwrap();
    assert_eq!(
        registry.len(),
        beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1
    );
    assert!(
        !registry.contains_key(&generations[0]),
        "the oldest inactive terminal history entry is evicted first"
    );
    assert!(registry.contains_key(&generations[1]));
    assert!(registry.contains_key(&replacement_generation));
    drop(registry);

    let evicted_id_replacement = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(
            generations[0],
            &evicted_id_replacement,
        )
        .err()
        .as_deref(),
        Some("grid_generation_reused"),
        "an evicted terminal ID remains a process-lifetime cancellation tombstone"
    );
    assert_eq!(evicted_id_replacement.terminal.load(Ordering::Acquire), 0);
    assert!(
        !evicted_id_replacement
            .registration_active
            .load(Ordering::Acquire)
    );

    let distinct_generation = ProjectId::new();
    let distinct_work = Arc::new(BeginnerGridWork::default());
    run_registered_beginner_grid_work_v1(distinct_generation, &distinct_work, || Ok(())).unwrap();
    assert_eq!(distinct_work.terminal.load(Ordering::Acquire), 1);
    assert_eq!(
        beginner_grid_work().lock().unwrap().len(),
        beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1
    );
    beginner_grid_work().lock().unwrap().clear();
}

#[test]
fn six_leg_configuration_preserves_existing_four_eight_and_complete_insect_layouts() {
    let records = |parts: &[(ori_domain::BeginnerTargetPartKindV1, u8)]| {
        parts
            .iter()
            .copied()
            .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
            .collect()
    };
    let signature = |profile: &ori_domain::BeginnerDesignProfileV1| {
        profile
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| {
                (
                    target.id,
                    target.count,
                    target.position_tenths_mm,
                    target.direction_milli,
                    target.symmetry,
                    target.priority,
                )
            })
            .collect::<Vec<_>>()
    };

    let mut four = ori_domain::BeginnerDesignProfileV1::default();
    four.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    four.generation_constraints.target_parts = records(&[
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
    ]);
    let four_estimate =
        ori_domain::estimate_symmetric_parameters_v1(&four.generation_constraints).unwrap();
    configure_symmetric_profile(
        &mut four,
        four_estimate,
        four_estimate.scale_percent,
        four_estimate.spacing_percent,
    );
    assert_eq!(
        signature(&four),
        vec![(
            1,
            4,
            [0, 0, 0],
            [0, 1000, 0],
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
            50,
        )]
    );

    let mut complete_animal = ori_domain::BeginnerDesignProfileV1::default();
    complete_animal.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    complete_animal.generation_constraints.target_parts = records(&[
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
    ]);
    let complete_animal_estimate =
        ori_domain::estimate_symmetric_parameters_v1(&complete_animal.generation_constraints)
            .unwrap();
    assert_eq!(complete_animal_estimate.protrusion_count, 8);
    configure_symmetric_profile(
        &mut complete_animal,
        complete_animal_estimate,
        complete_animal_estimate.scale_percent,
        complete_animal_estimate.spacing_percent,
    );
    assert_eq!(
        signature(&complete_animal),
        vec![
            (
                1,
                1,
                [0, 0, 0],
                [0, -1000, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::None,
                50,
            ),
            (
                2,
                1,
                [0, 0, 0],
                [1000, 0, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::None,
                50,
            ),
            (
                3,
                2,
                [0, 0, 0],
                [1000, 0, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                50,
            ),
            (
                4,
                4,
                [0, 0, 0],
                [0, 1000, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                50,
            ),
        ]
    );

    let mut complete_insect = ori_domain::BeginnerDesignProfileV1::default();
    complete_insect.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Insect);
    complete_insect.generation_constraints.target_parts = records(&[
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
        (ori_domain::BeginnerTargetPartKindV1::Antenna, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 6),
    ]);
    let complete_insect_estimate =
        ori_domain::estimate_symmetric_parameters_v1(&complete_insect.generation_constraints)
            .unwrap();
    assert_eq!(complete_insect_estimate.protrusion_count, 10);
    configure_symmetric_profile(
        &mut complete_insect,
        complete_insect_estimate,
        complete_insect_estimate.scale_percent,
        complete_insect_estimate.spacing_percent,
    );
    assert_eq!(
        signature(&complete_insect),
        vec![
            (
                1,
                2,
                [0, 0, 0],
                [1000, 0, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                60,
            ),
            (
                2,
                2,
                [0, 0, 0],
                [0, -1000, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                60,
            ),
            (
                3,
                2,
                [0, -250, 0],
                [1000, 0, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                50,
            ),
            (
                4,
                2,
                [0, 0, 0],
                [1000, 0, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                50,
            ),
            (
                5,
                2,
                [0, 250, 0],
                [1000, 0, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                50,
            ),
        ]
    );
}

#[test]
fn standalone_animal_lateral_pairs_reach_all_four_plan_families() {
    let project = initial_project_state();
    for (part_kind, expected_plan_kind) in [
        (
            ori_domain::BeginnerTargetPartKindV1::Wing,
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricBirdBase,
        ),
        (
            ori_domain::BeginnerTargetPartKindV1::Fin,
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFishBase,
        ),
        (
            ori_domain::BeginnerTargetPartKindV1::Ear,
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricEarBase,
        ),
        (
            ori_domain::BeginnerTargetPartKindV1::Horn,
            ori_domain::BeginnerGeneratedPlanKindV1::SymmetricHornBase,
        ),
    ] {
        let mut profile = ori_domain::BeginnerDesignProfileV1::default();
        profile.generation_constraints.target_category =
            Some(ori_domain::BeginnerTargetCategoryV1::Animal);
        profile.generation_constraints.target_parts = [
            (ori_domain::BeginnerTargetPartKindV1::Head, 1),
            (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
            (part_kind, 2),
        ]
        .into_iter()
        .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
        .collect();
        let estimate =
            ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints).unwrap();
        assert_eq!(estimate.protrusion_count, 2);
        configure_symmetric_profile(
            &mut profile,
            estimate,
            estimate.scale_percent,
            estimate.spacing_percent,
        );
        let configured = profile.clone();
        configure_symmetric_profile(
            &mut profile,
            estimate,
            estimate.scale_percent,
            estimate.spacing_percent,
        );
        assert_eq!(profile, configured);
        let [target] = profile.generation_constraints.protrusions.as_slice() else {
            panic!("standalone lateral pair must configure one exact target");
        };
        assert_eq!(target.count, 2);
        assert_eq!(
            target.symmetry,
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
        );
        assert_eq!(target.direction_milli, [1000, 0, 0]);
        assert_eq!(target.priority, 80);
        assert_eq!(
            ori_domain::beginner_target_approximation_score_v1(&profile.generation_constraints),
            92
        );
        let plans = ori_domain::generate_beginner_plans_v1(
            project.project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            &profile.generation_constraints,
        )
        .unwrap();
        assert_eq!(plans[0].kind, expected_plan_kind);
        assert_eq!(plans[0].crease_pattern.edges.len(), 4);
    }
}

#[test]
fn lateral_pair_configuration_preserves_axis_composite_and_insect_signatures() {
    let configured = |category, parts: &[(ori_domain::BeginnerTargetPartKindV1, u8)]| {
        let mut profile = ori_domain::BeginnerDesignProfileV1::default();
        profile.generation_constraints.target_category = Some(category);
        profile.generation_constraints.target_parts = parts
            .iter()
            .copied()
            .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
            .collect();
        let estimate =
            ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints).unwrap();
        configure_symmetric_profile(
            &mut profile,
            estimate,
            estimate.scale_percent,
            estimate.spacing_percent,
        );
        profile
    };
    let signature = |profile: &ori_domain::BeginnerDesignProfileV1| {
        profile
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| {
                (
                    target.id,
                    target.count,
                    target.direction_milli,
                    target.symmetry,
                    target.priority,
                )
            })
            .collect::<Vec<_>>()
    };
    let base = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
    ];

    let horn = configured(
        ori_domain::BeginnerTargetCategoryV1::Animal,
        &[
            base[0],
            base[1],
            (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        ],
    );
    assert_eq!(
        signature(&horn),
        vec![(
            1,
            1,
            [0, -1000, 0],
            ori_domain::BeginnerProtrusionSymmetryV1::None,
            50,
        )]
    );

    let tail = configured(
        ori_domain::BeginnerTargetCategoryV1::Animal,
        &[
            base[0],
            base[1],
            (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        ],
    );
    assert_eq!(
        signature(&tail),
        vec![(
            1,
            1,
            [1000, 0, 0],
            ori_domain::BeginnerProtrusionSymmetryV1::None,
            50,
        )]
    );

    for (features, expected) in [
        (
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
            ],
            vec![
                (
                    1,
                    1,
                    [0, -1000, 0],
                    ori_domain::BeginnerProtrusionSymmetryV1::None,
                    50,
                ),
                (
                    2,
                    1,
                    [1000, 0, 0],
                    ori_domain::BeginnerProtrusionSymmetryV1::None,
                    50,
                ),
            ],
        ),
        (
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
                (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
            ],
            vec![
                (
                    1,
                    1,
                    [1000, 0, 0],
                    ori_domain::BeginnerProtrusionSymmetryV1::None,
                    50,
                ),
                (
                    2,
                    2,
                    [1000, 0, 0],
                    ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                    50,
                ),
            ],
        ),
        (
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
                (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
            ],
            vec![
                (
                    1,
                    1,
                    [0, -1000, 0],
                    ori_domain::BeginnerProtrusionSymmetryV1::None,
                    50,
                ),
                (
                    2,
                    2,
                    [1000, 0, 0],
                    ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                    50,
                ),
            ],
        ),
    ] {
        let parts = [base.as_slice(), features.as_slice()].concat();
        let profile = configured(ori_domain::BeginnerTargetCategoryV1::Animal, &parts);
        assert_eq!(signature(&profile), expected);
    }

    let insect = configured(
        ori_domain::BeginnerTargetCategoryV1::Insect,
        &[
            base[0],
            base[1],
            (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
        ],
    );
    assert_eq!(
        signature(&insect),
        vec![(
            1,
            2,
            [1000, 0, 0],
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
            50,
        )]
    );
}

#[test]
fn symmetry_transforms_are_exact_at_cardinal_angles() {
    assert_eq!(
        mirror_point_left_right(Point2::new(3.0, 4.0), 1.0),
        Point2::new(-1.0, 4.0)
    );
    let center = Point2::new(1.0, 2.0);
    let point = Point2::new(3.0, 4.0);
    for (angle, expected) in [
        (0.0, Point2::new(3.0, 4.0)),
        (90.0, Point2::new(-1.0, 4.0)),
        (180.0, Point2::new(-1.0, 0.0)),
        (270.0, Point2::new(3.0, 0.0)),
    ] {
        let (sin, cos) = symmetry_sin_cos(angle).expect("finite cardinal angle");
        assert_eq!(rotate_point_about(point, center, sin, cos), expected);
    }
    let (sin, cos) = symmetry_sin_cos(37.5).expect("finite non-cardinal angle");
    assert_eq!(sin.to_bits(), 0x3fe3_7af9_3f95_13ea);
    assert_eq!(cos.to_bits(), 0x3fe9_6326_8b57_2492);
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(symmetry_sin_cos(invalid), None);
    }
}

fn execute_command(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        command,
    )
}

fn execute_undo(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_undo(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_redo(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_redo(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_edge_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_edge_split(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_edge_intersection_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_edge_intersection_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_intersection_cluster_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_intersection_cluster_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        targets,
        junction_vertex_id,
    )
}

fn execute_t_junction_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_t_junction_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_boundary_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_boundary_split(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "origami2-native-file-tests-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated native-file test directory");
        Self { path }
    }

    #[cfg(target_os = "windows")]
    fn new_relative() -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let path = PathBuf::from(format!(
            ".origami2-relative-native-file-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated relative native-file test directory");
        Self { path }
    }

    fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
