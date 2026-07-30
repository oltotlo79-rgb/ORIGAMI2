use super::*;

#[test]
fn layered_fallback_gate_is_exact_and_keeps_all_other_paths_uncertified_v1() {
    let exact = [(true, 0.0, 90.0), (true, 180.0, 180.0)];
    assert_eq!(
        layered_three_face_fallback_decision_v1(0.0, 3, 2, exact, true),
        LayeredThreeFaceFallbackDecisionV1::LayeredAttempt
    );

    let ordinary = LayeredThreeFaceFallbackDecisionV1::OrdinaryUncertified;
    let cases: &[(&str, f64, usize, usize, &[(bool, f64, f64)], bool)] = &[
        ("positive thickness", 0.01, 3, 2, &exact, true),
        ("not three faces", 0.0, 4, 2, &exact, true),
        ("not two hinges", 0.0, 3, 3, &exact, true),
        (
            "moving source is not exact positive zero",
            0.0,
            3,
            2,
            &[(true, -0.0, 90.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "moving target zero",
            0.0,
            3,
            2,
            &[(true, 0.0, 0.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "moving target 180",
            0.0,
            3,
            2,
            &[(true, 0.0, 180.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "moving target NaN",
            0.0,
            3,
            2,
            &[(true, 0.0, f64::NAN), (true, 180.0, 180.0)],
            true,
        ),
        (
            "moving target outside range",
            0.0,
            3,
            2,
            &[(true, 0.0, 181.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "stationary source is not 180",
            0.0,
            3,
            2,
            &[(true, 0.0, 90.0), (true, 179.0, 180.0)],
            true,
        ),
        (
            "stationary target is not exact 180",
            0.0,
            3,
            2,
            &[(true, 0.0, 90.0), (true, 180.0, 179.0)],
            true,
        ),
        (
            "both hinges moving",
            0.0,
            3,
            2,
            &[(true, 0.0, 90.0), (true, 0.0, 90.0)],
            true,
        ),
        (
            "edge ID mismatch",
            0.0,
            3,
            2,
            &[(false, 0.0, 90.0), (true, 180.0, 180.0)],
            true,
        ),
        (
            "edge order mismatch",
            0.0,
            3,
            2,
            &[(false, 0.0, 90.0), (false, 180.0, 180.0)],
            true,
        ),
        ("schedule length mismatch", 0.0, 3, 2, &exact, false),
    ];
    for (name, thickness, faces, hinges, schedule, same_schedule_length) in cases {
        assert_eq!(
            layered_three_face_fallback_decision_v1(
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
}

#[test]
fn production_three_face_strip_fallback_resolves_the_awaiting_mark_as_certified_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let _deadline_override_guard =
        set_next_post_apply_proof_deadline_v1(Duration::from_secs(5 * 60));
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    {
        let registry = transaction_state
            .3
            .lock()
            .expect("published production job");
        let premise = registry
            .jobs
            .front()
            .and_then(|job| job.premise.as_ref())
            .expect("retained production premise");
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
            "the production strip must reach the layered fallback rather than mint tree authority"
        );
    }
    let certified = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("production three-face post-Apply proof");
    assert_eq!(certified.status, "certified");
    assert!(certified.proof_failure.is_none());
    let project = crate::lock_project(&app_state).expect("certified production project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.total(), 0);
}
