use super::*;

#[test]
fn backend_deadline_resolves_and_reclaims_a_never_started_or_polled_job() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        crate::stacked_fold_read::tests::prepare_speculative_tree_preview_v1();
    let response_wire = serde_json::to_value(response).expect("preview response wire");
    let token: ProjectId =
        serde_json::from_value(response_wire["transactionProposal"]["transactionToken"].clone())
            .expect("speculative transaction token");
    let _deadline_override_guard = set_next_post_apply_proof_deadline_v1(Duration::ZERO);
    let revision = super::super::super::apply_speculative_stacked_fold_transaction_inner_v1(
        &app_state,
        &layer_state,
        &transaction_state,
        super::super::super::ApplySpeculativeStackedFoldRequestV1 {
            transaction_token: token,
            explicit_confirmation: true,
        },
    )
    .expect("Apply publishes a backend-owned deadline");
    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded test wait");
    loop {
        let project = crate::lock_project(&app_state).expect("project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        let registry = transaction_state.3.lock().expect("post-Apply registry");
        let completed = summary.applied.awaiting_proof == 0
            && summary.applied.unknown_deadline_reached == 1
            && registry.jobs.is_empty()
            && registry.retained_bytes == 0
            && !registry.deadline_scheduler_registered;
        if completed {
            assert_eq!(project.editor.revision(), revision);
            assert_eq!(summary.applied.total(), 1);
            break;
        }
        drop(registry);
        drop(project);
        assert!(
            Instant::now() < wait_until,
            "the backend deadline scheduler must not require start, poll, a job token, or a reply"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn each_deadline_resource_retry_window_is_bounded_by_attempts_and_elapsed_time() {
    let first_failure_at = Instant::now();
    assert!(deadline_resource_retry_allowed_v1(
        POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_ATTEMPTS_V1,
        first_failure_at,
        first_failure_at,
    ));
    assert!(!deadline_resource_retry_allowed_v1(
        POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_ATTEMPTS_V1.saturating_add(1),
        first_failure_at,
        first_failure_at,
    ));
    assert!(!deadline_resource_retry_allowed_v1(
        1,
        first_failure_at,
        first_failure_at + POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_DURATION_V1,
    ));
}

#[test]
fn resource_retry_scope_does_not_cancel_a_newer_published_in_flight_job() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (first_app, first_state, first_request, _) = prepare_started_actual_job_v1();
    let (_second_app, second_state, _second_request, _) = prepare_started_actual_job_v1();
    let old_cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let new_cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (mut newer_job, new_worker_premise) = {
        let mut second_registry = second_state.3.lock().expect("second registry");
        let mut job = second_registry
            .jobs
            .pop_front()
            .expect("second published job");
        second_registry.retained_bytes = second_registry
            .retained_bytes
            .saturating_sub(job.retained_bytes);
        let premise = job.premise.take().expect("new worker premise");
        job.state = PostApplyProofJobStateV1::InFlight {
            run_generation: 72,
            stage: 0,
            cancellation: std::sync::Arc::clone(&new_cancellation),
        };
        (job, premise)
    };
    let mut first_registry = first_state.3.lock().expect("first registry");
    let old_job = first_registry.jobs.front_mut().expect("old published job");
    let old_generation = old_job.scheduler_generation;
    let old_worker_premise = old_job.premise.take().expect("old worker premise");
    old_job.state = PostApplyProofJobStateV1::InFlight {
        run_generation: 71,
        stage: 0,
        cancellation: std::sync::Arc::clone(&old_cancellation),
    };
    newer_job.scheduler_generation = old_generation
        .checked_add(1)
        .expect("distinct bounded scheduler generation");
    let newer_token = newer_job.job_token;
    let old_retained_bytes = old_job.retained_bytes;
    first_registry.next_scheduler_generation = newer_job.scheduler_generation;
    first_registry.retained_bytes = first_registry
        .retained_bytes
        .checked_add(newer_job.retained_bytes)
        .expect("bounded retained bytes");
    first_registry.jobs.push_back(newer_job);
    signal_inflight_cancellation_within_deadline_resource_retry_scope_v1(
        &mut first_registry,
        Some(old_generation),
    );
    assert!(old_cancellation.load(std::sync::atomic::Ordering::Acquire));
    assert!(
        !new_cancellation.load(std::sync::atomic::Ordering::Acquire),
        "an old scheduler retry must not stop a job published after its scope"
    );
    drop(first_registry);

    complete_worker_attempt_v1(
        &first_state.3,
        &first_request,
        71,
        0,
        PostApplyProofWorkerAttemptV1 {
            diagnostic: Err(()),
            certificate: PostApplyProofWorkerCertificateV1::Cancelled(old_worker_premise),
        },
    );
    let newer_retain_until = {
        let mut registry = first_state.3.lock().expect("resource-stopped registry");
        let old_job = registry
            .jobs
            .iter_mut()
            .find(|job| job.job_token == first_request.job_token)
            .expect("resource-stopped old job");
        assert!(old_job.premise.is_some());
        assert_eq!(old_job.resource_recovery_cancelled_run_generation, Some(71));
        assert!(matches!(
            &old_job.state,
            PostApplyProofJobStateV1::Resolving {
                resolution: PostApplyProofResolutionV1::Failure(
                    PostApplyProofTerminalV1::UnknownResourceLimit
                ),
                ..
            }
        ));
        assert_eq!(
            cancellation_or_deadline_terminal_v1(old_job, old_job.proof_deadline),
            Some(PostApplyProofTerminalV1::UnknownResourceLimit),
            "joining an internally stopped worker must preserve resource origin at the deadline"
        );
        old_job.frontend_started = false;

        let newer_job = registry
            .jobs
            .iter_mut()
            .find(|job| job.job_token == newer_token)
            .expect("newer scoped job");
        newer_job.state =
            PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled);
        let retain_until = Instant::now();
        newer_job.retain_until = retain_until;
        retain_until
    };
    drop(new_worker_premise);
    execute_memo_edit_v1(&first_app, "drift after internal resource stop");

    let now = Instant::now();
    let mut project = crate::lock_project(&first_app).expect("first scoped project");
    let mut registry = first_state.3.lock().expect("scoped expiry registry");
    let old_job = registry
        .jobs
        .iter()
        .find(|job| job.job_token == first_request.job_token)
        .expect("resource-origin job after lifecycle drift");
    assert!(!unstarted_job_matches_live_binding_v1(old_job, &project));
    assert_eq!(
        lifecycle_or_pending_stop_terminal_v1(old_job, &project, now),
        Some(PostApplyProofTerminalV1::UnknownResourceLimit),
        "later unstarted lifecycle drift must not relabel an internal resource stop"
    );
    let retry_not_before = now
        .checked_add(POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_DURATION_V1)
        .expect("bounded retry deadline");
    assert_eq!(
        next_deadline_for_registration_v1(
            &registry,
            Some(DeadlineSchedulerResourceRetryV1 {
                attempt: 1,
                first_failure_at: now,
                through_scheduler_generation: old_generation,
                not_before: retry_not_before,
            }),
        ),
        Some(newer_retain_until),
        "a newer terminal retention deadline preempts an older scope's retry delay"
    );
    expire_due_jobs_outside_resource_retry_scope_locked_v1(
        &mut project,
        &mut registry,
        now,
        Some(old_generation),
    )
    .expect("newer terminal retention proceeds during an old retry");
    assert_eq!(registry.jobs.len(), 1);
    assert_eq!(registry.jobs[0].job_token, first_request.job_token);
    assert_eq!(registry.retained_bytes, old_retained_bytes);
}

#[test]
fn cancelled_worker_resolution_wins_over_a_simultaneously_elapsed_deadline() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let run_generation = 73;
    {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("started production job");
        job.proof_deadline = Instant::now();
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownCancelled,
            ),
        };
        assert_eq!(
            cancellation_or_deadline_terminal_v1(job, Instant::now()),
            Some(PostApplyProofTerminalV1::UnknownCancelled),
            "the shared start/poll/join/scheduler selector must preserve explicit cancellation"
        );
    }

    let progress = finish_worker_poll_v1(
        &app_state,
        &transaction_state,
        &request,
        run_generation,
        false,
    )
    .expect("the joined cancellation resolves despite the elapsed deadline");
    assert_eq!(progress.status, "unknown_cancelled");
    let project = crate::lock_project(&app_state).expect("cancelled project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
    assert_eq!(summary.applied.unknown_deadline_reached, 0);
    drop(project);

    let (scheduler_app, scheduler_state, _request, _) = prepare_started_actual_job_v1();
    let now = Instant::now();
    {
        let mut registry = scheduler_state.3.lock().expect("scheduler registry");
        let job = registry.jobs.front_mut().expect("scheduler job");
        job.proof_deadline = now;
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation: 74,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownCancelled,
            ),
        };
    }
    let mut scheduler_project =
        crate::lock_project(&scheduler_app).expect("scheduler cancellation project");
    let mut registry = scheduler_state.3.lock().expect("scheduler registry");
    expire_due_jobs_locked_v1(&mut scheduler_project, &mut registry, now)
        .expect("scheduler preserves cancellation precedence");
    assert!(matches!(
        &registry.jobs.front().expect("retained terminal").state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled)
    ));
    let summary = scheduler_project
        .editor
        .speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
    assert_eq!(summary.applied.unknown_deadline_reached, 0);
}

#[test]
fn scheduler_resolves_unstarted_live_drift_as_cancelled_before_deadline() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, _, _, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    assert!(execute_memo_edit_v1(&app_state, "drift before scheduler expiry") > revision);

    let now = Instant::now();
    let mut project = crate::lock_project(&app_state).expect("drifted project");
    let mut registry = transaction_state.3.lock().expect("post-Apply registry");
    let job = registry.jobs.front_mut().expect("unstarted job");
    job.proof_deadline = now;
    assert!(!unstarted_job_matches_live_binding_v1(job, &project));
    expire_due_jobs_locked_v1(&mut project, &mut registry, now)
        .expect("scheduler resolves lifecycle cancellation");

    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
    assert_eq!(summary.applied.unknown_deadline_reached, 0);
    assert!(registry.jobs.is_empty());
    assert_eq!(registry.retained_bytes, 0);
}
