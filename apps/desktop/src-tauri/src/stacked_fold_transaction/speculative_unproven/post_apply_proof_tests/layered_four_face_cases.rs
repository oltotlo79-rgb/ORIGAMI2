#[test]
fn four_face_geometric_rank_is_input_order_and_id_independent_v1() {
    let boundaries = [
        vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        vec![(10.0, 0.0), (11.0, 0.0), (11.0, 1.0), (10.0, 1.0)],
        vec![(20.0, 0.0), (21.0, 0.0), (21.0, 1.0), (20.0, 1.0)],
    ];
    let first_ids = [FaceId::new(), FaceId::new(), FaceId::new()];
    let second_ids = [FaceId::new(), FaceId::new(), FaceId::new()];
    let make_faces = |ids: [FaceId; 3], order: [usize; 3], reverse_even_slots: bool| {
        order
            .into_iter()
            .enumerate()
            .map(|(slot, index)| {
                let mut boundary = boundaries[index].clone();
                let length = boundary.len();
                boundary.rotate_left((slot + index) % length);
                if reverse_even_slots && slot % 2 == 0 {
                    boundary.reverse();
                }
                (ids[index], boundary)
            })
            .collect::<Vec<_>>()
    };

    for rank in 0..3 {
        assert_eq!(
            source_face_by_geometric_rank_v1(make_faces(first_ids, [2, 0, 1], false), 3, rank),
            Some(first_ids[rank])
        );
        assert_eq!(
            source_face_by_geometric_rank_v1(make_faces(second_ids, [1, 0, 2], true), 3, rank),
            Some(second_ids[rank])
        );
    }
    assert!(
        source_face_by_geometric_rank_v1(make_faces(first_ids, [0, 1, 2], false), 3, 3).is_none()
    );
    assert!(
        source_face_by_geometric_rank_v1(make_faces(first_ids, [0, 1, 2], false), 0, 0).is_none()
    );
    assert!(
        source_face_by_geometric_rank_v1(make_faces(first_ids, [0, 1, 2], false), 4, 0).is_none()
    );

    let mut duplicate_geometry = make_faces(first_ids, [0, 1, 2], false);
    duplicate_geometry[2].1 = duplicate_geometry[1].1.clone();
    assert!(source_face_by_geometric_rank_v1(duplicate_geometry, 3, 0).is_none());

    let mut non_finite = make_faces(first_ids, [0, 1, 2], false);
    non_finite[0].1[0].0 = f64::NAN;
    assert!(source_face_by_geometric_rank_v1(non_finite, 3, 0).is_none());
}

#[test]
fn layered_four_face_fallback_gate_is_exact_and_does_not_widen_general_fallback_v1() {
    let exact = [
        (true, 0.0, 90.0),
        (true, 180.0, 180.0),
        (true, 180.0, 180.0),
    ];
    assert_eq!(
        layered_four_face_fallback_decision_v1(0.0, 4, 3, exact, true),
        LayeredFourFaceFallbackDecisionV1::LayeredAttempt
    );

    let ordinary = LayeredFourFaceFallbackDecisionV1::OrdinaryUncertified;
    let stationary_below_flat = f64::from_bits(180.0_f64.to_bits() - 1);
    let stationary_above_flat = f64::from_bits(180.0_f64.to_bits() + 1);
    let moving_above_zero = f64::from_bits(0.0_f64.to_bits() + 1);
    let cases: &[(&str, f64, usize, usize, &[(bool, f64, f64)], bool)] = &[
        ("positive thickness", 0.01, 4, 3, &exact, true),
        ("not four faces", 0.0, 3, 3, &exact, true),
        ("not three hinges", 0.0, 4, 2, &exact, true),
        (
            "moving source is negative zero",
            0.0,
            4,
            3,
            &[
                (true, -0.0, 90.0),
                (true, 180.0, 180.0),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        (
            "moving source is one ulp above zero",
            0.0,
            4,
            3,
            &[
                (true, moving_above_zero, 90.0),
                (true, 180.0, 180.0),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        (
            "stationary target is one ulp below flat",
            0.0,
            4,
            3,
            &[
                (true, 0.0, 90.0),
                (true, 180.0, stationary_below_flat),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        (
            "stationary target is one ulp above flat",
            0.0,
            4,
            3,
            &[
                (true, 0.0, 90.0),
                (true, 180.0, stationary_above_flat),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        (
            "both non-flat hinges move",
            0.0,
            4,
            3,
            &[(true, 0.0, 90.0), (true, 0.0, 45.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "moving endpoint is zero",
            0.0,
            4,
            3,
            &[(true, 0.0, 0.0), (true, 180.0, 180.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "moving endpoint is flat",
            0.0,
            4,
            3,
            &[
                (true, 0.0, 180.0),
                (true, 180.0, 180.0),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        (
            "moving endpoint is not finite",
            0.0,
            4,
            3,
            &[
                (true, 0.0, f64::NAN),
                (true, 180.0, 180.0),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        (
            "one canonical edge differs",
            0.0,
            4,
            3,
            &[
                (true, 0.0, 90.0),
                (false, 180.0, 180.0),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        ("schedule length differs", 0.0, 4, 3, &exact, false),
    ];
    for (name, thickness, faces, hinges, schedule, same_schedule_length) in cases {
        assert_eq!(
            layered_four_face_fallback_decision_v1(
                *thickness,
                *faces,
                *hinges,
                schedule.iter().copied(),
                *same_schedule_length,
            ),
            ordinary,
            "{name} must retain ordinary Uncertified handling"
        );
    }

    let one_ulp_target = f64::from_bits(90.0_f64.to_bits() + 1);
    assert_eq!(
        layered_four_face_fallback_decision_v1(
            0.0,
            4,
            3,
            [
                (true, 0.0, one_ulp_target),
                (true, 180.0, 180.0),
                (true, 180.0, 180.0),
            ],
            true,
        ),
        LayeredFourFaceFallbackDecisionV1::LayeredAttempt,
        "the gate must preserve an exact requested non-flat target rather than rounding it"
    );
}

#[test]
fn layered_four_face_foreign_failure_then_owner_retry_preserves_positive_precedence_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, mut job) = prepare_four_face_certified_resolution_job_v1();
    let mut foreign = {
        let project = crate::lock_project(&app_state).expect("authority-owning project");
        ProjectState::new_with_paper(
            project.editor.pattern().clone(),
            project.editor.paper().clone(),
        )
    };
    assert_eq!(
        resolve_locked_terminal_v1(
            &mut foreign,
            &mut job,
            PostApplyProofTerminalV1::UnknownCancelled,
        ),
        Err(())
    );
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Certified(
                PostApplyProofCertifiedAuthorityV1::LayeredFourFace(_)
            ),
            ..
        }
    ));

    job.proof_deadline = Instant::now();
    let mut project = crate::lock_project(&app_state).expect("authority-owning project");
    resolve_locked_terminal_v1(
        &mut project,
        &mut job,
        PostApplyProofTerminalV1::UnknownDeadlineReached,
    )
    .expect("the owner retry consumes positive authority before stop policy");
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
    ));
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .total(),
        0
    );
    assert_eq!(
        resolve_locked_certified_terminal_v1(&mut project, &mut job),
        Err(()),
        "a duplicate direct resolver call cannot consume the proof twice"
    );
}

#[test]
fn production_four_face_chain_one_ulp_target_resolves_only_the_awaiting_mark_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let _deadline_override_guard =
        set_next_post_apply_proof_deadline_v1(Duration::from_secs(5 * 60));
    let target = f64::from_bits(90.0_f64.to_bits() + 1);
    let (app_state, transaction_state, request, selection) =
        prepare_started_four_face_job_v1(target);
    let (document_before, revision_before) = {
        let project = crate::lock_project(&app_state).expect("applied four-face project");
        (project.document(), project.editor.revision())
    };
    let (initial_orders, hinge_schedule) = {
        let registry = transaction_state.3.lock().expect("post-Apply registry");
        let premise = registry
            .jobs
            .front()
            .and_then(|job| job.premise.as_ref())
            .expect("retained four-face premise");
        assert!(is_layered_four_face_fallback_candidate_v1(premise));
        assert_eq!(
            premise.requested.requested_angle_degrees().to_bits(),
            target.to_bits()
        );
        assert!(
            run_direct_certificate_v1(
                premise,
                StackedFoldPathDiagnosticLimitsV1 {
                    sample_intervals: POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[0],
                    static_collision: Default::default(),
                },
                &CooperativeOperationControlV1::unbounded(),
            )
            .expect("ordinary tree issuer")
            .is_none(),
            "the production chain must reach only the distinct layered-four-face theorem"
        );
        let initial_orders = (0..premise.initial_layer_order.face_pair_order_count())
            .filter_map(|index| premise.initial_layer_order.face_pair_order(index))
            .map(|order| (order.lower_face, order.upper_face))
            .collect::<Vec<_>>();
        let hinge_schedule = premise
            .requested
            .initial()
            .pose()
            .hinge_angles()
            .iter()
            .zip(premise.requested.pose().hinge_angles())
            .map(|(source, target)| {
                (
                    source.edge(),
                    source.angle_degrees().to_bits(),
                    target.angle_degrees().to_bits(),
                )
            })
            .collect::<Vec<_>>();
        (initial_orders, hinge_schedule)
    };

    let job_diagnostic = || {
        let registry = transaction_state
            .3
            .lock()
            .expect("diagnostic four-face registry");
        registry
            .jobs
            .iter()
            .find(|job| job.job_token == request.job_token)
            .map(|job| {
                format!(
                    "state={:?}, cumulative_work={}, premise={}, \
                     recovery_cancelled_generation={:?}",
                    job.state,
                    job.cumulative_work,
                    job.premise.is_some(),
                    job.resource_recovery_cancelled_run_generation,
                )
            })
            .unwrap_or_else(|| "job=<missing>".to_owned())
    };
    let mut certified = None;
    for poll_index in 0..=POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.len() {
        let progress = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
            &app_state,
            &transaction_state,
            request.clone(),
        ))
        .expect("production four-face post-Apply proof");
        match progress.status {
            "certified" => {
                certified = Some(progress);
                break;
            }
            "proving" if poll_index < POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.len() => {}
            "proving" => panic!(
                "the bounded progressive poll budget must reach a terminal four-face result: {}, \
                 selection={selection:?}, initial_orders={initial_orders:?}, \
                 hinge_schedule={hinge_schedule:?}",
                job_diagnostic(),
            ),
            terminal => panic!(
                "the four-face theorem must certify instead of terminating as {terminal}: \
                 proof_failure={:?}, {}, selection={selection:?}, \
                 initial_orders={initial_orders:?}, hinge_schedule={hinge_schedule:?}",
                progress.proof_failure,
                job_diagnostic(),
            ),
        }
    }
    let certified = certified.expect("bounded four-face polling must publish Certified");
    assert!(certified.proof_failure.is_none());
    let project = crate::lock_project(&app_state).expect("certified four-face project");
    assert_eq!(project.document(), document_before);
    assert_eq!(project.editor.revision(), revision_before);
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.total(), 0);
}

#[test]
fn four_face_fallback_preserves_stop_and_foreign_evidence_without_document_mutation_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let _deadline_override_guard =
        set_next_post_apply_proof_deadline_v1(Duration::from_secs(5 * 60));
    let (app_state, transaction_state, _, _) = prepare_started_four_face_job_v1(90.0);
    let (_, foreign_state, _, _) = prepare_started_actual_job_v1();
    let document_before = crate::lock_project(&app_state)
        .expect("four-face project")
        .document();
    let mut premise = take_retained_premise_v1(&transaction_state);
    let mut foreign = take_retained_premise_v1(&foreign_state);

    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let cancelled_result = run_layered_four_face_fallback_v1(
        premise,
        &CooperativeOperationControlV1::new(
            Some(&cancelled),
            std::time::Instant::now() + Duration::from_secs(60),
        ),
    );
    assert_eq!(
        cancelled_result.state(),
        PostApplyProofCertificateStateV1::Cancelled
    );
    premise = cancelled_result
        .into_recoverable_premise()
        .expect("cancel retains the exact premise");

    let deadline_result = run_layered_four_face_fallback_v1(
        premise,
        &CooperativeOperationControlV1::new(None, std::time::Instant::now()),
    );
    assert_eq!(
        deadline_result.state(),
        PostApplyProofCertificateStateV1::DeadlineExceeded
    );
    premise = deadline_result
        .into_recoverable_premise()
        .expect("deadline retains the exact premise");

    let original_admission = premise.initial_layer_order.clone();
    premise.initial_layer_order = foreign.initial_layer_order.clone();
    let foreign_admission_result =
        run_layered_four_face_fallback_v1(premise, &CooperativeOperationControlV1::unbounded());
    assert_ne!(
        foreign_admission_result.state(),
        PostApplyProofCertificateStateV1::Certified
    );
    premise = foreign_admission_result
        .into_recoverable_premise()
        .expect("foreign admission cannot consume the ticket");
    premise.initial_layer_order = original_admission;

    std::mem::swap(&mut premise.requested, &mut foreign.requested);
    let foreign_pose_result =
        run_layered_four_face_fallback_v1(premise, &CooperativeOperationControlV1::unbounded());
    assert_ne!(
        foreign_pose_result.state(),
        PostApplyProofCertificateStateV1::Certified
    );
    assert_eq!(
        crate::lock_project(&app_state)
            .expect("unchanged four-face project")
            .document(),
        document_before
    );
}
