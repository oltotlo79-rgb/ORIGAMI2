#[test]
fn publication_failure_resolves_the_exact_mark_unknown_instead_of_abandoning_it() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        crate::stacked_fold_read::tests::prepare_speculative_tree_preview_v1();
    let response_wire = serde_json::to_value(response).expect("preview response wire");
    let token: ProjectId =
        serde_json::from_value(response_wire["transactionProposal"]["transactionToken"].clone())
            .expect("speculative transaction token");
    let _publication_failure_guard = fail_next_post_apply_proof_publication_v1();
    super::super::apply_speculative_stacked_fold_transaction_inner_v1(
        &app_state,
        &layer_state,
        &transaction_state,
        super::super::ApplySpeculativeStackedFoldRequestV1 {
            transaction_token: token,
            explicit_confirmation: true,
        },
    )
    .expect("Apply succeeds while proof retention fails closed");

    let project = crate::lock_project(&app_state).expect("project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_resource_limit, 1);
    assert_eq!(summary.applied.total(), 1);
}

fn assert_publication_resolution_fault_rolls_back_complete_apply_v1<
    PublicationFaultGuard,
    ResolutionFaultGuard,
    PoseRollbackFaultGuard,
>(
    arm_publication_fault: impl FnOnce(&StackedFoldTransactionState) -> PublicationFaultGuard,
    arm_resolution_fault: impl FnOnce() -> ResolutionFaultGuard,
    arm_pose_rollback_fault: impl FnOnce() -> PoseRollbackFaultGuard,
) {
    let (app_state, layer_state, transaction_state, response) =
        crate::stacked_fold_read::tests::prepare_speculative_tree_preview_v1();
    let response_wire = serde_json::to_value(response).expect("preview response wire");
    let token: ProjectId =
        serde_json::from_value(response_wire["transactionProposal"]["transactionToken"].clone())
            .expect("speculative transaction token");
    let (
        editor_before,
        document_before,
        numeric_before,
        layer_evidence_before,
        pose_before,
        layer_capability_before,
    ) = {
        let project = crate::lock_project(&app_state).expect("pre-Apply project");
        (
            format!("{:?}", project.editor),
            project.document(),
            project.numeric_expressions.clone(),
            project.current_layer_evidence.clone(),
            project
                .applied_pose_authority
                .test_snapshot()
                .expect("pre-Apply pose authority"),
            crate::global_flat_foldability::capture_current_layer_order_capability(
                &layer_state,
                &project,
            )
            .expect("pre-Apply layer capture")
            .expect("pre-Apply layer authority"),
        )
    };

    let _publication_fault_guard = arm_publication_fault(&transaction_state);
    let _resolution_fault_guard = arm_resolution_fault();
    let _pose_rollback_fault_guard = arm_pose_rollback_fault();
    assert!(
        super::super::apply_speculative_stacked_fold_transaction_inner_v1(
            &app_state,
            &layer_state,
            &transaction_state,
            super::super::ApplySpeculativeStackedFoldRequestV1 {
                transaction_token: token,
                explicit_confirmation: true,
            },
        )
        .is_err(),
        "Apply must fail instead of exposing a mark without a recovery owner"
    );

    let project = crate::lock_project(&app_state).expect("rolled-back project");
    assert_eq!(format!("{:?}", project.editor), editor_before);
    assert_eq!(project.document(), document_before);
    assert_eq!(project.numeric_expressions, numeric_before);
    assert_eq!(project.current_layer_evidence, layer_evidence_before);
    assert_eq!(
        project
            .applied_pose_authority
            .test_snapshot()
            .expect("rolled-back pose authority"),
        pose_before
    );
    assert!(
        crate::global_flat_foldability::revalidate_current_layer_order_capability(
            &layer_state,
            &project,
            &layer_capability_before,
        )
        .expect("rolled-back layer validation")
        .is_some(),
        "the unconsumed layer guard must preserve the exact source authority"
    );
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .total(),
        0,
        "the pre-Apply editor must contain neither Awaiting nor a partial fallback outcome"
    );
    let registry = transaction_state.3.lock().expect("post-Apply registry");
    assert!(registry.jobs.is_empty());
    assert_eq!(registry.retained_bytes, 0);
    assert!(!registry.deadline_scheduler_registered);
}

#[test]
fn publication_fallback_error_and_panic_restore_the_complete_pre_apply_state() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    assert_publication_resolution_fault_rolls_back_complete_apply_v1(
        |_| fail_next_post_apply_proof_publication_v1(),
        super::super::fail_next_post_apply_publication_resolution_for_test_v1,
        || {},
    );
    assert_publication_resolution_fault_rolls_back_complete_apply_v1(
        |_| fail_next_post_apply_proof_publication_v1(),
        super::super::panic_next_post_apply_publication_resolution_before_for_test_v1,
        || {},
    );
    assert_publication_resolution_fault_rolls_back_complete_apply_v1(
        |transaction_state| fail_next_post_apply_deadline_registration_v1(&transaction_state.3),
        super::super::fail_next_post_apply_publication_resolution_for_test_v1,
        || {},
    );
}

#[test]
fn publication_rollback_restores_a_stale_origin_cache_snapshot_without_retrying() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    assert_publication_resolution_fault_rolls_back_complete_apply_v1(
        |_| fail_next_post_apply_proof_publication_v1(),
        super::super::fail_next_post_apply_publication_resolution_for_test_v1,
        crate::stacked_fold_transaction::fail_next_transaction_rollback_for_test_v1,
    );
}

#[test]
fn publication_fallback_panic_after_resolution_is_confirmed_as_committed() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        crate::stacked_fold_read::tests::prepare_speculative_tree_preview_v1();
    let response_wire = serde_json::to_value(response).expect("preview response wire");
    let token: ProjectId =
        serde_json::from_value(response_wire["transactionProposal"]["transactionToken"].clone())
            .expect("speculative transaction token");
    let _publication_failure_guard = fail_next_post_apply_proof_publication_v1();
    let _resolution_fault_guard =
        super::super::panic_next_post_apply_publication_resolution_after_for_test_v1();
    super::super::apply_speculative_stacked_fold_transaction_inner_v1(
        &app_state,
        &layer_state,
        &transaction_state,
        super::super::ApplySpeculativeStackedFoldRequestV1 {
            transaction_token: token,
            explicit_confirmation: true,
        },
    )
    .expect("the exact committed fallback is recovered after its reply panics");
    let project = crate::lock_project(&app_state).expect("fail-closed project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_resource_limit, 1);
    assert_eq!(summary.applied.total(), 1);
}

#[test]
fn first_start_after_document_or_pose_drift_resolves_the_exact_mark_cancelled() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();

    let (app_state, transaction_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    assert!(execute_memo_edit_v1(&app_state, "edit before the first proof start") > revision);
    let document_drift = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("document drift resolves the retained mark fail-closed");
    assert_eq!(document_drift.status, "unknown_cancelled");
    assert_eq!(
        proof_failure_json_v1(document_drift),
        serde_json::json!({
            "location": "applied_retained_undo",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 1,
            "undoStepsToRevert": 2
        })
    );
    {
        let project = crate::lock_project(&app_state).expect("document-drift project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        assert_eq!(summary.applied.awaiting_proof, 0);
        assert_eq!(summary.applied.unknown_cancelled, 1);
        assert_eq!(summary.applied.unknown_deadline_reached, 0);
        let registry = transaction_state.3.lock().expect("document-drift registry");
        let job = registry.jobs.front().expect("retained terminal");
        assert!(job.premise.is_none());
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled)
        ));
    }

    let (app_state, transaction_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    {
        let project = crate::lock_project(&app_state).expect("pose-drift project");
        let authority = project.applied_pose_authority.clone();
        authority
            .begin_invalidation()
            .expect("pose invalidation preflight")
            .commit();
        assert_eq!(project.editor.revision(), revision);
    }
    let pose_drift = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("pose drift resolves the retained mark fail-closed");
    assert_eq!(pose_drift.status, "unknown_cancelled");
    assert_eq!(
        proof_failure_json_v1(pose_drift),
        serde_json::json!({
            "location": "applied_retained_undo",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 0,
            "undoStepsToRevert": 1
        })
    );
    let project = crate::lock_project(&app_state).expect("pose-drift project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
    assert_eq!(summary.applied.unknown_deadline_reached, 0);
}

#[test]
fn failed_first_start_resolution_keeps_the_ticket_and_retry_resolves_it() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    execute_memo_edit_v1(&app_state, "force first-start drift");
    let job_token = transaction_state
        .3
        .lock()
        .expect("post-Apply registry")
        .jobs
        .front()
        .expect("published job")
        .job_token;
    let request = StartPostApplyProofJobRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: instance_id,
        project_id,
        revision,
    };

    let _start_resolution_failure_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
    assert_eq!(
        start_post_apply_proof_job_inner_v1(&app_state, &transaction_state, request),
        Err(unavailable_message_v1())
    );
    {
        let project = crate::lock_project(&app_state).expect("unresolved project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        assert_eq!(summary.applied.awaiting_proof, 1);
        assert_eq!(summary.applied.unknown_cancelled, 0);
        let registry = transaction_state.3.lock().expect("retained registry");
        let job = registry.jobs.front().expect("retryable job");
        assert_eq!(job.job_token, job_token);
        assert!(!job.frontend_started);
        assert!(job.premise.is_some());
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Ready { next_stage: 0 }
        ));
    }

    let retried = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("retry resolves the same retained ticket");
    assert_eq!(retried.job_token, job_token);
    assert_eq!(retried.status, "unknown_cancelled");
    let project = crate::lock_project(&app_state).expect("resolved retry project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
}

#[test]
fn accepted_start_retry_ignores_later_live_target_edits() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    execute_memo_edit_v1(&app_state, "edit after the accepted start");

    let retried = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
        },
    )
    .expect("an accepted start is idempotent after later edits");
    assert_eq!(retried.job_token, request.job_token);
    assert_eq!(retried.status, "proving");
    {
        let project = crate::lock_project(&app_state).expect("continuing project");
        assert_eq!(
            project
                .editor
                .speculative_unproven_fold_summary_v1()
                .applied
                .awaiting_proof,
            1
        );
        let registry = transaction_state.3.lock().expect("continuing registry");
        let job = registry.jobs.front().expect("continuing job");
        assert!(job.frontend_started);
        assert!(job.premise.is_some());
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Ready { next_stage: 0 }
        ));
    }

    let terminal = cancel_and_poll_v1(&app_state, &transaction_state, &request);
    assert_eq!(terminal.status, "unknown_cancelled");
    let project = crate::lock_project(&app_state).expect("cancelled continuing project");
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        0
    );
}

#[test]
fn start_retry_commits_resolving_failure_before_exposing_terminal_progress() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    execute_memo_edit_v1(&app_state, "edit while the proof result is resolving");
    {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("started job");
        job.premise = None;
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation: 1,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownEvidenceInsufficient,
            ),
        };
        let uncommitted = progress_v1(job);
        assert_eq!(uncommitted.status, "proving");
        assert!(uncommitted.proof_failure.is_none());
    }

    let settled = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
        },
    )
    .expect("start retry commits a pending worker resolution");
    assert_eq!(settled.job_token, request.job_token);
    assert_eq!(settled.status, "unknown_evidence_insufficient");
    assert_eq!(
        proof_failure_json_v1(settled),
        serde_json::json!({
            "location": "applied_retained_undo",
            "outcome": "unknown",
            "reason": "evidence_insufficient",
            "subsequentEditCount": 1,
            "undoStepsToRevert": 2
        })
    );
    let project = crate::lock_project(&app_state).expect("settled project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_evidence_insufficient, 1);
}

#[test]
fn generic_resolution_failure_keeps_the_exact_ticket_until_poll_retry() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let _resolution_failure_guard = fail_next_post_apply_generic_resolution_v1(&request.job_token);
    cancel_post_apply_proof_job_inner_v1(&app_state, &transaction_state, request.clone())
        .expect("cancel queues a retryable exact resolution");

    {
        let project = crate::lock_project(&app_state).expect("awaiting project");
        let summary = project.editor.speculative_unproven_fold_summary_v1();
        assert_eq!(summary.applied.awaiting_proof, 1);
        assert_eq!(summary.applied.unknown_cancelled, 0);
        let registry = transaction_state.3.lock().expect("retryable registry");
        let job = registry.jobs.front().expect("retryable job");
        assert!(
            job.premise.is_some(),
            "the one-shot ticket must remain owned"
        );
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Resolving {
                resolution: PostApplyProofResolutionV1::Failure(
                    PostApplyProofTerminalV1::UnknownCancelled
                ),
                ..
            }
        ));
        assert_eq!(progress_v1(job).status, "proving");
    }

    let retried = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("poll retries the exact generic resolution");
    assert_eq!(retried.status, "unknown_cancelled");
    let project = crate::lock_project(&app_state).expect("resolved project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
}

#[test]
fn generic_retry_recovers_a_matching_report_that_was_already_committed() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let cancelled = SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
        reason: SpeculativeUnprovenFoldUnknownReasonV1::Cancelled,
    };
    {
        let mut project = crate::lock_project(&app_state).expect("project");
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("started job");
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation: 59,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownCancelled,
            ),
        };
        let report = project
            .editor
            .resolve_speculative_unproven_fold_v1(&job.binding, cancelled)
            .expect("simulate a committed exact report whose reply was lost");
        assert_eq!(report.outcome, cancelled);
        assert!(job_matches_continuing_project_v1(job, &project));
    }

    let recovered = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("poll recovers the already committed matching report");
    assert_eq!(recovered.status, "unknown_cancelled");
    let project = crate::lock_project(&app_state).expect("recovered project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
}

#[test]
fn failed_idempotent_start_keeps_deadline_origin_after_its_worker_joins() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_premise = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("started job");
        let premise = job.premise.take().expect("worker premise");
        job.proof_deadline = Instant::now();
        job.state = PostApplyProofJobStateV1::InFlight {
            run_generation: 60,
            stage: 0,
            cancellation: std::sync::Arc::clone(&cancellation),
        };
        premise
    };
    let start_request = StartPostApplyProofJobRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: request.project_instance_id,
        project_id: request.project_id,
        revision: request.revision,
    };

    let _resolution_failure_guard = fail_next_post_apply_generic_resolution_v1(&request.job_token);
    assert_eq!(
        start_post_apply_proof_job_inner_v1(
            &app_state,
            &transaction_state,
            StartPostApplyProofJobRequestV1 {
                version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
                project_instance_id: request.project_instance_id,
                project_id: request.project_id,
                revision: request.revision,
            },
        ),
        Err(unavailable_message_v1())
    );
    assert!(cancellation.load(Ordering::Acquire));
    {
        let registry = transaction_state.3.lock().expect("retryable registry");
        let job = registry.jobs.front().expect("deadline-owned job");
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Resolving {
                run_generation: 0,
                resolution: PostApplyProofResolutionV1::Failure(
                    PostApplyProofTerminalV1::UnknownDeadlineReached
                ),
            }
        ));
    }

    complete_worker_attempt_v1(
        &transaction_state.3,
        &request,
        60,
        0,
        PostApplyProofWorkerAttemptV1 {
            diagnostic: Err(()),
            certificate: PostApplyProofWorkerCertificateV1::Cancelled(worker_premise),
        },
    );
    let resolved =
        start_post_apply_proof_job_inner_v1(&app_state, &transaction_state, start_request)
            .expect("idempotent start retries the original exact deadline");
    assert_eq!(resolved.status, "unknown_deadline_reached");
    let project = crate::lock_project(&app_state).expect("deadline project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 0);
    assert_eq!(summary.applied.unknown_deadline_reached, 1);
}

#[test]
fn worker_join_panic_keeps_a_retryable_exact_resource_resolution() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let _worker_panic_guard = panic_next_post_apply_worker_v1(&request.job_token);
    let _resolution_failure_guard = fail_next_post_apply_generic_resolution_v1(&request.job_token);

    let first = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request.clone(),
    ))
    .expect("worker panic is converted to a retryable resource resolution");
    assert_eq!(first.status, "proving");
    {
        let project = crate::lock_project(&app_state).expect("awaiting project");
        assert_eq!(
            project
                .editor
                .speculative_unproven_fold_summary_v1()
                .applied
                .awaiting_proof,
            1
        );
        let registry = transaction_state.3.lock().expect("retryable registry");
        let job = registry.jobs.front().expect("retryable job");
        assert!(job.premise.is_none(), "the unwound worker owned the ticket");
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Resolving {
                resolution: PostApplyProofResolutionV1::Failure(
                    PostApplyProofTerminalV1::UnknownResourceLimit
                ),
                ..
            }
        ));
    }

    let retried = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("poll retries the exact resource resolution");
    assert_eq!(retried.status, "unknown_resource_limit");
    let project = crate::lock_project(&app_state).expect("resource-resolved project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_resource_limit, 1);
}

fn assert_transient_binder_fault_retains_ticket_for_next_stage_v1(
    fault: InjectedPostApplyBinderFaultV1,
) {
    let _deadline_override_guard =
        set_next_post_apply_proof_deadline_v1(Duration::from_secs(5 * 60));
    let (app_state, transaction_state, request, total_pair_count) = prepare_started_actual_job_v1();
    let _binder_fault_guard = inject_next_post_apply_binder_fault_v1(&request.job_token, fault);
    let first = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request.clone(),
    ))
    .expect("transient binder failure returns nonterminal progress");
    assert_eq!(first.status, "proving");
    {
        let project = crate::lock_project(&app_state).expect("awaiting project");
        assert_eq!(
            project
                .editor
                .speculative_unproven_fold_summary_v1()
                .applied
                .awaiting_proof,
            1
        );
        let registry = transaction_state.3.lock().expect("retryable registry");
        let job = registry.jobs.front().expect("retryable job");
        assert!(
            job.premise.is_some(),
            "the original ticket must be retained"
        );
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Ready { next_stage: 1 }
        ));
    }

    // The one-shot fault is consumed by the first worker generation. The
    // exact premise then belongs to stage 1, where the current narrow
    // three-face authority may finish the same ticket.
    let second = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request.clone(),
    ))
    .expect("the same retained ticket recovers on the next bounded stage");
    assert_eq!(second.status, "certified");
    assert_eq!(second.proven_pair_count, total_pair_count);
    assert!(second.proof_failure.is_none());
    {
        let registry = transaction_state.3.lock().expect("recovered registry");
        let job = registry.jobs.front().expect("recovered job");
        assert!(
            job.premise.is_none(),
            "terminal authority must consume the retained premise"
        );
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
        ));
    }
    let project = crate::lock_project(&app_state).expect("certified project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.total(), 0);
}

#[test]
fn transient_binder_allocation_fault_retains_the_ticket_for_the_next_stage() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let observed_before = observed_post_apply_binder_validation_panics_for_test_v1();
    assert_transient_binder_fault_retains_ticket_for_next_stage_v1(
        InjectedPostApplyBinderFaultV1::Allocation,
    );
    assert_eq!(
        observed_post_apply_binder_validation_panics_for_test_v1(),
        observed_before,
        "an allocation fault must not use the validation-panic boundary"
    );
}

#[test]
fn transient_binder_validation_panic_is_caught_and_retains_the_ticket_for_the_next_stage() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let observed_before = observed_post_apply_binder_validation_panics_for_test_v1();
    assert_transient_binder_fault_retains_ticket_for_next_stage_v1(
        InjectedPostApplyBinderFaultV1::ValidationPanic,
    );
    assert_eq!(
        observed_post_apply_binder_validation_panics_for_test_v1(),
        observed_before
            .checked_add(1)
            .expect("binder validation panic observation count"),
        "the validation fault must execute and catch one real binder panic"
    );
}

#[test]
fn binding_rejection_resolves_unknown_instead_of_abandoning_the_mark_as_stale() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let premise = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("started job");
        let premise = job.premise.take().expect("retained premise");
        job.state = PostApplyProofJobStateV1::InFlight {
            run_generation: 41,
            stage: 0,
            cancellation: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        premise
    };
    complete_worker_attempt_v1(
        &transaction_state.3,
        &request,
        41,
        0,
        PostApplyProofWorkerAttemptV1 {
            diagnostic: Err(()),
            certificate: PostApplyProofWorkerCertificateV1::BindingRejected(premise),
        },
    );
    {
        let registry = transaction_state.3.lock().expect("resolving registry");
        let job = registry.jobs.front().expect("resolving job");
        assert!(job.premise.is_some());
        assert!(matches!(
            &job.state,
            PostApplyProofJobStateV1::Resolving {
                resolution: PostApplyProofResolutionV1::Failure(
                    PostApplyProofTerminalV1::UnknownEvidenceInsufficient
                ),
                ..
            }
        ));
    }

    let settled = finish_worker_poll_v1(&app_state, &transaction_state, &request, 41, false)
        .expect("binding mismatch resolves an explicit fail-closed outcome");
    assert_eq!(settled.status, "unknown_evidence_insufficient");
    let project = crate::lock_project(&app_state).expect("resolved mismatch project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_evidence_insufficient, 1);
}
