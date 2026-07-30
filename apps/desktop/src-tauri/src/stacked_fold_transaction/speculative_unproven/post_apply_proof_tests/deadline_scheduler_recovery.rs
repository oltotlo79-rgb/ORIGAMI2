#[test]
fn backend_deadline_cleanup_is_aba_safe_for_a_replaced_project_instance() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        crate::stacked_fold_read::tests::prepare_speculative_tree_preview_v1();
    let response_wire = serde_json::to_value(response).expect("preview response wire");
    let token: ProjectId =
        serde_json::from_value(response_wire["transactionProposal"]["transactionToken"].clone())
            .expect("speculative transaction token");
    super::super::apply_speculative_stacked_fold_transaction_inner_v1(
        &app_state,
        &layer_state,
        &transaction_state,
        super::super::ApplySpeculativeStackedFoldRequestV1 {
            transaction_token: token,
            explicit_confirmation: true,
        },
    )
    .expect("Apply publishes a backend-owned deadline");

    let mut project = crate::lock_project(&app_state).expect("project");
    project.instance_id = ProjectId::new();
    let mut registry = transaction_state.3.lock().expect("post-Apply registry");
    let now = Instant::now();
    registry
        .jobs
        .front_mut()
        .expect("published job")
        .proof_deadline = now;
    expire_due_jobs_locked_v1(&mut project, &mut registry, now)
        .expect("stale cleanup does not need recovery");

    assert!(
        registry.jobs.is_empty(),
        "the obsolete ticket must be reclaimed"
    );
    assert_eq!(registry.retained_bytes, 0);
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 1);
    assert_eq!(summary.applied.unknown_deadline_reached, 0);
    assert_eq!(summary.applied.total(), 1);
}

#[test]
fn a_new_active_job_wakes_a_scheduler_waiting_on_terminal_retention() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (first_app, first_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let first_started = start_post_apply_proof_job_inner_v1(
        &first_app,
        &first_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("start first job");
    let first_request = PostApplyProofJobRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: instance_id,
        project_id,
        revision,
        job_token: first_started.job_token,
    };
    cancel_post_apply_proof_job_inner_v1(&first_app, &first_state, first_request)
        .expect("retain first terminal until its long retention deadline");

    let (_second_app, second_state, _, _, _) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let mut newly_active = {
        let mut registry = second_state.3.lock().expect("second registry");
        let job = registry.jobs.pop_front().expect("second active job");
        registry.retained_bytes = registry.retained_bytes.saturating_sub(job.retained_bytes);
        job
    };
    let newly_active_token = newly_active.job_token;
    newly_active.proof_deadline = Instant::now();
    {
        let mut registry = first_state.3.lock().expect("first registry");
        assert!(registry.deadline_scheduler_registered);
        registry.retained_bytes = registry
            .retained_bytes
            .checked_add(newly_active.retained_bytes)
            .expect("bounded retained bytes");
        registry.jobs.push_back(newly_active);
    }

    // This is the same notification issued after every successful
    // publication, including when the registry already has a scheduler.
    wake_deadline_scheduler_v1();
    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded test wait");
    loop {
        let registry = first_state.3.lock().expect("first registry");
        let new_job_reclaimed = registry
            .jobs
            .iter()
            .all(|job| job.job_token != newly_active_token);
        let retained_terminal_unchanged = registry.jobs.len() == 1
            && matches!(
                &registry.jobs[0].state,
                PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled)
            );
        if new_job_reclaimed && retained_terminal_unchanged {
            break;
        }
        drop(registry);
        assert!(
            Instant::now() < wait_until,
            "the new proof deadline must preempt the five-minute terminal retention sleep"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_scheduler_panic_fails_closed_releases_its_lease_and_accepts_the_next_job() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    wake_deadline_scheduler_v1();
    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded stale-registration drain");
    while ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) != 0 {
        assert!(
            Instant::now() < wait_until,
            "dropped test registrations must release their RAII leases"
        );
        thread::sleep(Duration::from_millis(1));
    }
    let active_before = 0;
    let (failed_app, failed_state, failed_instance, failed_project, failed_revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let failed_started = start_post_apply_proof_job_inner_v1(
        &failed_app,
        &failed_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: failed_instance,
            project_id: failed_project,
            revision: failed_revision,
        },
    )
    .expect("start the job whose scheduler will panic");
    assert_eq!(
        ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire),
        active_before + 1
    );
    let _scheduler_panic_guard = panic_next_deadline_scheduler_iteration_v1(&failed_state);
    wake_deadline_scheduler_v1();

    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded panic recovery wait");
    loop {
        let project = crate::lock_project(&failed_app).expect("failed project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        let registry = failed_state.3.lock().expect("failed registry");
        let recovered = summary.applied.awaiting_proof == 0
            && summary.applied.unknown_resource_limit == 1
            && registry.jobs.len() == 1
            && registry.retained_bytes > 0
            && registry.deadline_scheduler_registered
            && matches!(
                &registry.jobs[0].state,
                PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit)
            )
            && ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire)
                == active_before + 1;
        if recovered {
            break;
        }
        drop(registry);
        drop(project);
        assert!(
            Instant::now() < wait_until,
            "a scheduler panic must fail all retained work closed and release every lease"
        );
        thread::sleep(Duration::from_millis(1));
    }

    let retained = start_post_apply_proof_job_inner_v1(
        &failed_app,
        &failed_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: failed_instance,
            project_id: failed_project,
            revision: failed_revision,
        },
    )
    .expect("resource terminal remains observable");
    assert_eq!(retained.job_token, failed_started.job_token);
    assert_eq!(retained.status, "unknown_resource_limit");
    {
        let mut registry = failed_state.3.lock().expect("failed registry");
        registry.jobs[0].retain_until = Instant::now();
    }
    wake_deadline_scheduler_v1();
    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded terminal retention cleanup");
    loop {
        let registry = failed_state.3.lock().expect("failed registry");
        let purged = registry.jobs.is_empty()
            && registry.retained_bytes == 0
            && !registry.deadline_scheduler_registered
            && ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) == active_before;
        if purged {
            break;
        }
        drop(registry);
        assert!(
            Instant::now() < wait_until,
            "terminal retention expiry must purge bytes and release its lease"
        );
        thread::sleep(Duration::from_millis(1));
    }

    let _deadline_override_guard = set_next_post_apply_proof_deadline_v1(Duration::ZERO);
    let (next_app, next_state, _, _, _) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded post-panic deadline wait");
    loop {
        let project = crate::lock_project(&next_app).expect("next project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        let registry = next_state.3.lock().expect("next registry");
        let completed = summary.applied.awaiting_proof == 0
            && summary.applied.unknown_deadline_reached == 1
            && registry.jobs.is_empty()
            && !registry.deadline_scheduler_registered
            && ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) == active_before;
        if completed {
            break;
        }
        drop(registry);
        drop(project);
        assert!(
            Instant::now() < wait_until,
            "the supervised scheduler must keep using the connected OnceLock sender"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_scheduler_panic_recovers_each_of_three_retained_registrations_once() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    wake_deadline_scheduler_v1();
    let drain_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded stale-registration drain");
    while ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) != 0 {
        assert!(
            Instant::now() < drain_until,
            "stale scheduler registrations must release their RAII leases"
        );
        thread::sleep(Duration::from_millis(1));
    }

    let mut retained = Vec::new();
    for _ in 0..3 {
        let (app_state, transaction_state, instance_id, project_id, revision) =
            crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1(
            );
        let started = start_post_apply_proof_job_inner_v1(
            &app_state,
            &transaction_state,
            StartPostApplyProofJobRequestV1 {
                version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
                project_instance_id: instance_id,
                project_id,
                revision,
            },
        )
        .expect("start retained scheduler-panic job");
        retained.push((app_state, transaction_state, started.job_token));
    }
    assert_eq!(
        ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire),
        retained.len()
    );

    // Target the last enqueued registration. The panic cannot fire until all
    // three earlier FIFO commands have entered the scheduler-owned vector.
    let _scheduler_panic_guard = panic_next_deadline_scheduler_iteration_v1(&retained[2].1);
    wake_deadline_scheduler_v1();
    let recovered_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded multi-registration panic recovery");
    loop {
        let recovered = retained
            .iter()
            .all(|(app_state, transaction_state, job_token)| {
                let project = crate::lock_project(app_state).expect("retained project");
                let summary = project.editor.speculative_unproven_fold_summary_v1();
                let registry = transaction_state.3.lock().expect("retained registry");
                summary.applied.awaiting_proof == 0
                    && summary.applied.unknown_resource_limit == 1
                    && registry.jobs.len() == 1
                    && registry.jobs[0].job_token == *job_token
                    && registry.deadline_scheduler_registered
                    && matches!(
                        &registry.jobs[0].state,
                        PostApplyProofJobStateV1::Terminal(
                            PostApplyProofTerminalV1::UnknownResourceLimit
                        )
                    )
            });
        if recovered {
            break;
        }
        assert!(
            Instant::now() < recovered_until,
            "one scheduler panic must recover every original retained registration exactly once"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire),
        retained.len()
    );

    for (_, transaction_state, _) in &retained {
        let mut registry = transaction_state.3.lock().expect("retained registry");
        registry.jobs[0].retain_until = Instant::now();
    }
    wake_deadline_scheduler_v1();
    let purge_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded retained-registration purge");
    loop {
        let purged = retained.iter().all(|(_, transaction_state, _)| {
            let registry = transaction_state.3.lock().expect("purged registry");
            registry.jobs.is_empty()
                && registry.retained_bytes == 0
                && !registry.deadline_scheduler_registered
        }) && ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) == 0;
        if purged {
            break;
        }
        assert!(
            Instant::now() < purge_until,
            "all retained terminals must purge and release their RAII leases"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_deadline_resolution_panic_keeps_its_ticket_until_resource_retry_succeeds() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    wake_deadline_scheduler_v1();
    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded stale-registration drain");
    while ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) != 0 {
        assert!(
            Instant::now() < wait_until,
            "stale scheduler registrations must drain within two seconds"
        );
        thread::sleep(Duration::from_millis(1));
    }

    let (app_state, transaction_state, _, _, _) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let job_token = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("published job");
        job.proof_deadline = Instant::now();
        job.job_token
    };
    let _deadline_resolution_panic_guard =
        panic_next_deadline_resolution_and_recovery_v1(job_token);
    wake_deadline_scheduler_v1();

    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded resource retry");
    loop {
        let project = crate::lock_project(&app_state).expect("project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        let registry = transaction_state.3.lock().expect("post-Apply registry");
        let recovered = summary.applied.awaiting_proof == 0
            && summary.applied.unknown_resource_limit == 1
            && registry.jobs.is_empty()
            && registry.retained_bytes == 0
            && !registry.deadline_scheduler_registered
            && ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) == 0;
        if recovered {
            break;
        }
        drop(registry);
        drop(project);
        assert!(
            Instant::now() < wait_until,
            "panic recovery failure must retain the ticket for resource retry"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn bounded_scheduler_resource_retries_keep_ownership_until_the_exact_mark_resolves() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    wake_deadline_scheduler_v1();
    let drain_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded stale-registration drain");
    while ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) != 0 {
        assert!(
            Instant::now() < drain_until,
            "stale scheduler registrations must drain before poison recovery"
        );
        thread::sleep(Duration::from_millis(1));
    }

    let (app_state, transaction_state, _, _, _) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let scheduler_recovery_barrier =
        crate::lock_project(&app_state).expect("scheduler recovery project barrier");

    // First force the registered dispatcher into its isolated recovery path
    // while the still-future proof deadline cannot acquire the project. The
    // consumed panic target is the handshake proving that no ordinary expiry
    // iteration can race the due-deadline mutation below.
    let scheduler_panic_guard = panic_next_deadline_scheduler_iteration_v1(&transaction_state);
    wake_deadline_scheduler_v1();
    let panic_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded scheduler panic handshake");
    while deadline_scheduler_panic_targets_registry_for_test_v1(&transaction_state.3) {
        assert!(
            Instant::now() < panic_until,
            "the registered scheduler must acknowledge its armed recovery panic"
        );
        wake_deadline_scheduler_v1();
        thread::sleep(Duration::from_millis(1));
    }
    drop(scheduler_panic_guard);

    // Exhaust one complete retry window plus its final exact-resolution
    // attempt, then let the retained owner succeed in the next window. The
    // guard clears this process-global test fault even if a later assertion
    // unwinds before the explicit normal-path drop.
    let forced_resource_failures =
        force_next_post_apply_deadline_resource_failures_v1(&transaction_state.3, 12);
    let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (request, worker_owned_premise) = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry
            .jobs
            .front_mut()
            .expect("published never-started job");
        let premise = job.premise.take().expect("worker premise");
        job.proof_deadline = Instant::now();
        job.state = PostApplyProofJobStateV1::InFlight {
            run_generation: 61,
            stage: 0,
            cancellation: std::sync::Arc::clone(&cancellation),
        };
        (
            PostApplyProofJobRequestV1 {
                version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
                project_instance_id: job.binding.project_instance_id(),
                project_id: job.binding.project_id(),
                revision: job.target_revision,
                job_token: job.job_token,
            },
            premise,
        )
    };

    // Command handlers deliberately reject this poisoned registry; the
    // backend scheduler owns the recovery lock and must still finish its
    // bounded retry sequence without retaining a busy registration forever.
    let poisoned = std::sync::Arc::clone(&transaction_state.3);
    let _ = std::panic::catch_unwind(move || {
        let _guard = poisoned.lock().expect("registry before poison");
        panic!("inject post-Apply registry poison");
    });
    drop(scheduler_recovery_barrier);
    wake_deadline_scheduler_v1();

    let resolved_until = Instant::now()
        .checked_add(Duration::from_secs(3))
        .expect("bounded repeated-retry wait");
    loop {
        let project = crate::lock_project(&app_state).expect("project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        let registry = transaction_state
            .3
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let resolved = summary.applied.awaiting_proof == 0
            && summary.applied.unknown_resource_limit == 1
            && registry.jobs.is_empty()
            && registry.retained_bytes == 0
            && !registry.deadline_scheduler_registered
            && cancellation.load(std::sync::atomic::Ordering::Acquire)
            && ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire) == 0;
        if resolved {
            break;
        }
        if Instant::now() >= resolved_until {
            let job = registry.jobs.front().map(|job| {
                format!(
                    "state={:?}, scheduler_generation={}, frontend_started={}, premise={}, \
                     recovery_cancelled_generation={:?}",
                    job.state,
                    job.scheduler_generation,
                    job.frontend_started,
                    job.premise.is_some(),
                    job.resource_recovery_cancelled_run_generation,
                )
            });
            panic!(
                "each bounded retry window must retain ownership until an exact ResourceLimit \
                 report succeeds: awaiting={}, resource={}, cancelled={}, deadline={}, jobs={}, \
                 retained_bytes={}, registered={}, cancellation={}, active={}, forced_remaining={}, \
                 job={job:?}",
                summary.applied.awaiting_proof,
                summary.applied.unknown_resource_limit,
                summary.applied.unknown_cancelled,
                summary.applied.unknown_deadline_reached,
                registry.jobs.len(),
                registry.retained_bytes,
                registry.deadline_scheduler_registered,
                cancellation.load(std::sync::atomic::Ordering::Acquire),
                ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.load(Ordering::Acquire),
                forced_post_apply_deadline_resource_failures_remaining_for_test_v1(
                    &transaction_state.3
                ),
            );
        }
        drop(registry);
        drop(project);
        thread::sleep(Duration::from_millis(1));
    }

    drop(forced_resource_failures);

    // The exact editor mark is already terminal. A late worker still owns its
    // one-shot premise, but no local job remains that it could revive.
    complete_worker_attempt_v1(
        &transaction_state.3,
        &request,
        61,
        0,
        PostApplyProofWorkerAttemptV1 {
            diagnostic: Err(()),
            certificate: PostApplyProofWorkerCertificateV1::Uncertified(worker_owned_premise),
        },
    );
    let project = crate::lock_project(&app_state).expect("resolved project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_resource_limit, 1);
}

#[test]
fn backend_deadline_resolves_when_the_start_reply_and_all_polls_are_lost() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    // Preserve only the identity needed to observe the backend later. The
    // successful response itself is intentionally lost before the deadline.
    let lost_job_token = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("backend accepts start before its reply is lost")
    .job_token;
    {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        registry
            .jobs
            .front_mut()
            .expect("started job")
            .proof_deadline = Instant::now();
    }
    wake_deadline_scheduler_v1();

    let wait_until = Instant::now()
        .checked_add(Duration::from_secs(2))
        .expect("bounded test wait");
    loop {
        let project = crate::lock_project(&app_state).expect("project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        let registry = transaction_state.3.lock().expect("post-Apply registry");
        let completed = summary.applied.awaiting_proof == 0
            && summary.applied.unknown_deadline_reached == 1
            && registry.jobs.len() == 1
            && matches!(
                &registry.jobs[0].state,
                PostApplyProofJobStateV1::Terminal(
                    PostApplyProofTerminalV1::UnknownDeadlineReached
                )
            )
            && registry.jobs[0].premise.is_none();
        if completed {
            assert_eq!(project.editor.revision(), revision);
            assert_eq!(summary.applied.total(), 1);
            break;
        }
        drop(registry);
        drop(project);
        assert!(
            Instant::now() < wait_until,
            "the backend deadline must not depend on delivery of start or any poll"
        );
        thread::sleep(Duration::from_millis(1));
    }

    execute_memo_edit_v1(&app_state, "edit after a lost deadline reply");
    let retry = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("the identical start retry reads the retained terminal");
    assert_eq!(retry.job_token, lost_job_token);
    assert_eq!(retry.status, "unknown_deadline_reached");
    assert_eq!(
        proof_failure_json_v1(retry),
        serde_json::json!({
            "location": "applied_retained_undo",
            "outcome": "unknown",
            "reason": "deadline_reached",
            "subsequentEditCount": 1,
            "undoStepsToRevert": 2
        })
    );
}
