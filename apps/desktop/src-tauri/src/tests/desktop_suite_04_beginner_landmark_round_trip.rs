#[test]
fn asymmetric_fish_live_estimate_configures_three_ordered_landmarks() {
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = [
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Fin, 2),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    let estimate =
        ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints).unwrap();
    assert_eq!(estimate.protrusion_count, 3);
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
        profile
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| (
                target.id,
                target.count,
                target.position_tenths_mm,
                target.direction_milli,
                target.symmetry,
                target.priority,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                1,
                1,
                [-4, 0, 0],
                [-1000, 200, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::None,
                80,
            ),
            (
                2,
                1,
                [5, 1, 0],
                [1000, -100, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::None,
                80,
            ),
            (
                3,
                1,
                [0, -5, 0],
                [100, -1000, 0],
                ori_domain::BeginnerProtrusionSymmetryV1::None,
                80,
            ),
        ]
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
        ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase
    );
    assert_eq!(plans[0].crease_pattern.edges.len(), 4);
}

#[test]
fn asymmetric_landmark_native_apply_undo_redo_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    for (plan_kind, target_kind, target_count, archive_name, semantic_binding_count) in [
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Wing,
            2,
            "asymmetric-bird.ori2",
            None,
        ),
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Leg,
            4,
            "asymmetric-four-leg.ori2",
            None,
        ),
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Tail,
            1,
            "asymmetric-insect.ori2",
            Some(10),
        ),
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Fin,
            2,
            "asymmetric-fish.ori2",
            Some(4),
        ),
    ] {
        let insect_landmarks = semantic_binding_count == Some(10);
        let fish_landmarks = semantic_binding_count == Some(4);
        let mut profile = ori_domain::BeginnerDesignProfileV1::default();
        profile.generation_constraints.target_category =
            Some(ori_domain::BeginnerTargetCategoryV1::Animal);
        profile.generation_constraints.target_category = Some(if insect_landmarks {
            ori_domain::BeginnerTargetCategoryV1::Insect
        } else {
            ori_domain::BeginnerTargetCategoryV1::Animal
        });
        profile.generation_constraints.target_parts = (if insect_landmarks {
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Head, 1),
                (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
                (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
                (ori_domain::BeginnerTargetPartKindV1::Leg, 6),
            ]
        } else if fish_landmarks {
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Head, 1),
                (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
                (ori_domain::BeginnerTargetPartKindV1::Fin, 2),
            ]
        } else {
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Head, 1),
                (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
                (target_kind, target_count),
            ]
        })
        .into_iter()
        .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
        .collect();
        configure_symmetric_profile(
            &mut profile,
            ori_domain::BeginnerSymmetricParameterEstimateV1 {
                protrusion_count: 2,
                scale_percent: 27,
                spacing_percent: 50,
            },
            27,
            50,
        );
        profile.generation_constraints.skeleton_segments.truncate(
            if target_count == 4 || insect_landmarks {
                3
            } else {
                2
            },
        );
        profile.generation_constraints.skeleton_segments[0]
            .start
            .x_tenths_mm = -10;
        profile.generation_constraints.skeleton_segments[0]
            .start
            .y_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[0]
            .end
            .x_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[0]
            .end
            .y_tenths_mm = 10;
        profile.generation_constraints.skeleton_segments[1]
            .start
            .x_tenths_mm = 10;
        profile.generation_constraints.skeleton_segments[1]
            .start
            .y_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[1]
            .end
            .x_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[1]
            .end
            .y_tenths_mm = 10;
        let mut left = profile.generation_constraints.protrusions[0].clone();
        left.count = 1;
        left.length_tenths_mm = 4;
        left.thickness_tenths_mm = 2;
        left.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::None;
        left.position_tenths_mm = [-4, 0, 0];
        left.direction_milli = [-1_000, 200, 0];
        let mut right = left.clone();
        right.id = 2;
        right.position_tenths_mm = [5, 1, 0];
        right.direction_milli = [1_000, -100, 0];
        profile.generation_constraints.protrusions = if insect_landmarks {
            let mut targets = vec![left.clone()];
            let leg_positions: [(i16, i16); 6] =
                [(-5, 4), (5, 4), (-6, 0), (6, 0), (-5, -4), (5, -4)];
            for (offset, (x, y)) in leg_positions.into_iter().enumerate() {
                let mut leg = left.clone();
                leg.id = u16::try_from(offset + 2).unwrap();
                leg.position_tenths_mm = [i32::from(x), i32::from(y), 0];
                leg.direction_milli = [x.signum() * 1_000, y * 50, 0];
                targets.push(leg);
            }
            targets
        } else if fish_landmarks {
            let mut tail = left.clone();
            tail.id = 3;
            tail.position_tenths_mm = [0, -5, 0];
            tail.direction_milli = [100, -1_000, 0];
            vec![left, right, tail]
        } else if target_count == 4 {
            let mut rear_left = left.clone();
            rear_left.id = 3;
            rear_left.position_tenths_mm = [-5, -4, 0];
            rear_left.direction_milli = [-900, -300, 0];
            let mut rear_right = right.clone();
            rear_right.id = 4;
            rear_right.position_tenths_mm = [4, -5, 0];
            rear_right.direction_milli = [900, -200, 0];
            vec![left, right, rear_left, rear_right]
        } else {
            vec![left, right]
        };

        let half_height = 86.602_540_378_443_86;
        let mut project = ProjectState::new(CreasePattern::empty());
        let geometry_namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x04, 0x97,
        ]);
        let boundary_positions = [
            Point2::new(100.0, 0.0),
            Point2::new(-50.0, half_height),
            Point2::new(-50.0, -half_height),
            Point2::new(50.0, -half_height),
        ];
        let vertices = boundary_positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| Vertex {
                id: VertexId::derive_v5(geometry_namespace, format!("vertex-{index}").as_bytes()),
                position,
            })
            .collect::<Vec<_>>();
        let edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::derive_v5(geometry_namespace, format!("boundary-{index}").as_bytes()),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect();
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.0,
            ..Paper::default()
        };
        project.editor = EditorState::with_paper(CreasePattern { vertices, edges }, paper);
        project.saved_document = Some(project.document());
        let project_id = project.project_id;
        let instance_id = project.instance_id;
        let revision = project.editor.revision();
        let saved = execute_command(
            &mut project,
            project_id,
            revision,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(profile.clone()),
            },
        )
        .unwrap();
        let plan = ori_domain::generate_beginner_plans_v1(
            project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            &profile.generation_constraints,
        )
        .unwrap()
        .into_iter()
        .find(|plan| plan.kind == plan_kind)
        .unwrap();
        let candidate_edge = plan.crease_pattern.edges[0].id;
        assert_eq!(
            plan.crease_pattern
                .edges
                .iter()
                .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
                .count(),
            4,
            "native-positive landmark DTO must remain a four-hinge tree candidate"
        );
        let preview = assess_beginner_generated_plan_with_deadline(
            project_id,
            project.editor.paper(),
            project.editor.pattern(),
            &plan,
            None,
            std::time::Instant::now() + std::time::Duration::from_millis(750),
        );
        assert!(
            preview.apply_allowed,
            "preview rejected: {}",
            preview.reason
        );
        assert_eq!(preview.proof_scope, "sufficient");
        assert!(matches!(
            preview.reason,
            "native_fold_path_certified" | "global_flat_foldability_proven"
        ));
        let expired = assess_beginner_generated_plan_with_deadline(
            project_id,
            project.editor.paper(),
            project.editor.pattern(),
            &plan,
            None,
            std::time::Instant::now(),
        );
        assert_eq!(
            (expired.apply_allowed, expired.proof_scope, expired.reason),
            (false, "indeterminate", "deadline_exceeded"),
            "an expired assessment deadline must not publish native fold-path authority"
        );
        let canonical_edge_ids = plan
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        for _ in 0..32 {
            let authority = ProjectId::new();
            let authority_plan = ori_domain::generate_beginner_plans_v1(
                authority,
                project.editor.pattern(),
                &project.editor.paper().boundary_vertices,
                &profile.generation_constraints,
            )
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.kind == plan_kind)
            .unwrap();
            assert_eq!(
                authority_plan
                    .crease_pattern
                    .edges
                    .iter()
                    .map(|edge| edge.id)
                    .collect::<Vec<_>>(),
                canonical_edge_ids
            );
            assert_eq!(
                authority_plan.crease_pattern, plan.crease_pattern,
                "schema-derived landmark geometry must not depend on the runtime project authority"
            );
        }
        let state = AppState::new(project);
        let before = {
            let project = lock_project(&state).unwrap();
            project_state_signature(&project)
        };
        let mut tampered = profile.clone();
        tampered.generation_constraints.protrusions[0].priority += 1;
        for (foreign_instance, foreign_project, stale_revision) in [
            (ProjectId::new(), project_id, saved.revision),
            (instance_id, ProjectId::new(), saved.revision),
            (instance_id, project_id, saved.revision.saturating_sub(1)),
        ] {
            assert!(
                apply_beginner_generated_plan_document(
                    &state,
                    foreign_instance,
                    foreign_project,
                    stale_revision,
                    profile.clone(),
                    plan_kind,
                    candidate_edge,
                )
                .is_err()
            );
            assert_eq!(
                {
                    let project = lock_project(&state).unwrap();
                    project_state_signature(&project)
                },
                before
            );
        }
        assert!(
            apply_beginner_generated_plan_document(
                &state,
                instance_id,
                project_id,
                saved.revision,
                tampered,
                plan_kind,
                candidate_edge,
            )
            .is_err()
        );
        assert_eq!(
            {
                let project = lock_project(&state).unwrap();
                project_state_signature(&project)
            },
            before
        );

        let applied = apply_beginner_generated_plan_document(
            &state,
            instance_id,
            project_id,
            saved.revision,
            profile.clone(),
            plan_kind,
            candidate_edge,
        )
        .unwrap();
        let after_apply = {
            let project = lock_project(&state).unwrap();
            project_state_signature(&project)
        };
        for (rejected_instance, rejected_project, rejected_revision, rejected_edge) in [
            (instance_id, project_id, saved.revision, candidate_edge),
            (
                ProjectId::new(),
                project_id,
                applied.revision,
                candidate_edge,
            ),
            (
                instance_id,
                ProjectId::new(),
                applied.revision,
                candidate_edge,
            ),
            (instance_id, project_id, applied.revision, EdgeId::new()),
        ] {
            assert!(
                apply_beginner_generated_plan_document(
                    &state,
                    rejected_instance,
                    rejected_project,
                    rejected_revision,
                    profile.clone(),
                    plan_kind,
                    rejected_edge,
                )
                .is_err()
            );
            assert_eq!(
                {
                    let project = lock_project(&state).unwrap();
                    project_state_signature(&project)
                },
                after_apply
            );
        }
        let mut project = lock_project(&state).unwrap();
        let provenance = project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .unwrap();
        assert!(provenance.fold_path_certificate_sha256.is_some());
        if let Some(expected_count) = semantic_binding_count {
            let semantic = provenance
                .semantic_landmark_provenance
                .as_ref()
                .expect("asymmetric semantic provenance");
            assert_eq!(semantic.ordered_bindings.len(), expected_count);
            assert_eq!(semantic.ordered_bindings[0].role, "head");
            assert_eq!(
                semantic.ordered_bindings.last().unwrap().role,
                if fish_landmarks {
                    "fin_right"
                } else {
                    "leg_rear_right"
                }
            );
            assert!(ori_domain::validate_beginner_generation_provenance_v1(
                provenance
            ));
        }
        let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
        assert!(
            project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .is_none()
        );
        execute_redo(&mut project, project_id, undone.revision).unwrap();
        let document = project.document();
        let bytes = write_project_ori2(&document).unwrap();
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
        let reopened = ProjectState::from_valid_document(restored, PathBuf::from(archive_name));
        assert_eq!(reopened.document(), document);
        assert!(
            reopened
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|value| value.fold_path_certificate_sha256)
                .is_some()
        );
        let instruction = reopened
            .editor
            .instruction_timeline()
            .steps
            .last()
            .expect("native-positive candidate instruction");
        let certificate = reopened
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|value| value.fold_path_certificate_sha256)
            .expect("archived native fold-path certificate");
        let certificate_hex = certificate
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(instruction.caution.contains(&certificate_hex));
        if let Some(expected_count) = semantic_binding_count {
            assert_eq!(
                reopened
                    .editor
                    .beginner_design_profile()
                    .generation_provenance
                    .as_ref()
                    .and_then(|value| value.semantic_landmark_provenance.as_ref())
                    .map(|semantic| semantic.ordered_bindings.len()),
                Some(expected_count)
            );
        }
    }
}
