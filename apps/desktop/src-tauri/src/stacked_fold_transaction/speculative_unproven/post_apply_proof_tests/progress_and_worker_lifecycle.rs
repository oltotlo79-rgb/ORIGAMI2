#[test]
fn progress_wire_is_coarse_and_contains_no_geometry() {
    let instance = ProjectId::new();
    let project = ProjectId::new();
    let token = ProjectId::new();
    let progress = PostApplyProofProgressV1 {
        version: 1,
        project_instance_id: instance,
        project_id: project,
        revision: 9,
        job_token: token,
        status: "proving",
        proven_pair_count: 0,
        total_pair_count: 3,
        proof_failure: None,
    };
    let value = serde_json::to_value(progress).expect("progress JSON");
    assert_eq!(
        value,
        serde_json::json!({
            "version": 1,
            "projectInstanceId": instance,
            "projectId": project,
            "revision": 9,
            "jobToken": token,
            "status": "proving",
            "provenPairCount": 0,
            "totalPairCount": 3,
            "proofFailure": null
        })
    );
    let keys = value
        .as_object()
        .expect("progress object")
        .keys()
        .collect::<Vec<_>>();
    for forbidden in [
        "geometry",
        "fingerprint",
        "coordinate",
        "vertex",
        "edge",
        "face",
        "angle",
    ] {
        assert!(
            keys.iter().all(|key| !key.contains(forbidden)),
            "leaked {forbidden}"
        );
    }
}

#[test]
fn only_typed_native_authority_can_choose_the_certified_terminal() {
    assert_eq!(
        classify_attempt_v1(0, None, PostApplyProofCertificateStateV1::Certified),
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::Certified)
    );
    assert_eq!(
        classify_attempt_v1(0, None, PostApplyProofCertificateStateV1::Uncertified),
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit)
    );
    assert_eq!(
        classify_attempt_v1(0, None, PostApplyProofCertificateStateV1::BindingRejected),
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownEvidenceInsufficient)
    );
    assert_eq!(
        classify_attempt_v1(
            0,
            None,
            PostApplyProofCertificateStateV1::ResourceUnavailable
        ),
        AttemptResultV1::Continue
    );
    assert_eq!(
        classify_attempt_v1(
            POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.len() - 1,
            None,
            PostApplyProofCertificateStateV1::ResourceUnavailable
        ),
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit)
    );
    assert_eq!(
        terminal_outcome_v1(PostApplyProofTerminalV1::Certified),
        None,
        "the generic resolver must never receive a Certified outcome"
    );
}

#[test]
fn a_blocking_witness_wins_over_typed_positive_authority() {
    assert_eq!(
        typed_authority_terminal_v1(true, PostApplyProofCertificateStateV1::Certified),
        Some(PostApplyProofTerminalV1::Blocked)
    );
    assert_eq!(
        typed_authority_terminal_v1(false, PostApplyProofCertificateStateV1::Certified),
        Some(PostApplyProofTerminalV1::Certified)
    );
    assert_eq!(
        typed_authority_terminal_v1(true, PostApplyProofCertificateStateV1::Uncertified),
        Some(PostApplyProofTerminalV1::Blocked)
    );
    assert_eq!(
        typed_authority_terminal_v1(true, PostApplyProofCertificateStateV1::BindingRejected),
        Some(PostApplyProofTerminalV1::Blocked)
    );
    assert_eq!(
        typed_authority_terminal_v1(true, PostApplyProofCertificateStateV1::ResourceUnavailable),
        Some(PostApplyProofTerminalV1::Blocked)
    );
}

#[test]
fn deadline_precedes_both_positive_and_negative_proof_results() {
    for completed in [
        PostApplyProofTerminalV1::Certified,
        PostApplyProofTerminalV1::Blocked,
    ] {
        assert_eq!(
            terminal_after_attempt_v1(true, AttemptResultV1::Terminal(completed)),
            Some(PostApplyProofTerminalV1::UnknownDeadlineReached)
        );
    }
}

#[test]
fn an_in_flight_result_is_ignored_after_cancel_terminal_wins() {
    assert!(run_result_is_current_v1(
        &PostApplyProofJobStateV1::InFlight {
            run_generation: 7,
            stage: 1,
            cancellation: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
        7,
        1,
    ));
    assert!(!run_result_is_current_v1(
        &PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled),
        7,
        1,
    ));
    assert!(!run_result_is_current_v1(
        &PostApplyProofJobStateV1::InFlight {
            run_generation: 8,
            stage: 1,
            cancellation: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
        7,
        1,
    ));
}

#[test]
fn each_in_flight_generation_owns_a_fresh_release_signalled_control_token() {
    let first = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let second = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    assert!(
        !std::sync::Arc::ptr_eq(&first, &second),
        "a later generation must never reuse an old cancellation token"
    );
    let state = PostApplyProofJobStateV1::InFlight {
        run_generation: 17,
        stage: 0,
        cancellation: std::sync::Arc::clone(&first),
    };
    signal_inflight_cancellation_state_v1(&state);
    assert!(first.load(std::sync::atomic::Ordering::Acquire));
    assert!(!second.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn reclaiming_an_in_flight_job_signals_its_worker_before_dropping_the_premise() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (_app_state, transaction_state, _request, _) = prepare_started_actual_job_v1();
    let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut registry = transaction_state.3.lock().expect("post-Apply registry");
    let job = registry.jobs.front_mut().expect("published job");
    let _worker_owned_premise = job.premise.take().expect("retained premise");
    job.state = PostApplyProofJobStateV1::InFlight {
        run_generation: 23,
        stage: 0,
        cancellation: std::sync::Arc::clone(&cancellation),
    };

    remove_job_v1(&mut registry, 0);

    assert!(registry.jobs.is_empty());
    assert!(
        cancellation.load(std::sync::atomic::Ordering::Acquire),
        "a detached worker must observe the lifecycle closure"
    );
}
