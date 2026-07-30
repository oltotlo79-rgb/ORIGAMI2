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
    let project_id = project.project_id;
    let instance_id = project.instance_id;
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
        )
        .is_err()
    );
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
    )
    .unwrap();
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
    let _redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    let saved = project.document();
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("complete-animal.ori2"));
    assert_eq!(reopened.document(), saved);
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
    assert_eq!(plan.crease_pattern.vertices.len(), 15);
    assert_eq!(plan.crease_pattern.edges.len(), 14);
    let project_id = project.project_id;
    let instance_id = project.instance_id;
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
    )
    .unwrap();
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
    execute_redo(&mut project, project_id, undone.revision).unwrap();
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
    assert_eq!(plans[0].crease_pattern.vertices.len(), 13);
    assert_eq!(plans[0].crease_pattern.edges.len(), 12);
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
