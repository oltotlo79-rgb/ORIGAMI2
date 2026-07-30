#[tauri::command]
pub(crate) fn start_post_apply_proof_job_v1(
    app_state: State<'_, AppState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    request: StartPostApplyProofJobRequestV1,
) -> Result<PostApplyProofProgressV1, String> {
    start_post_apply_proof_job_inner_v1(&app_state, &transaction_state, request)
}

fn start_post_apply_proof_job_inner_v1(
    app_state: &AppState,
    transaction_state: &StackedFoldTransactionState,
    request: StartPostApplyProofJobRequestV1,
) -> Result<PostApplyProofProgressV1, String> {
    validate_start_request_v1(&request)?;
    let mut project = lock_project(app_state).map_err(|_| unavailable_message_v1())?;
    let now = Instant::now();
    let mut registry = lock_registry_v1(transaction_state).map_err(|_| unavailable_message_v1())?;
    let Some(job) = registry.jobs.iter_mut().find(|job| {
        job.binding.project_instance_id() == request.project_instance_id
            && job.binding.project_id() == request.project_id
            && job.target_revision == request.revision
    }) else {
        return Err(unavailable_message_v1());
    };
    if matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
        if job_matches_continuing_project_v1(job, &project) {
            refresh_terminal_report_v1(&project, job);
        } else {
            close_noncontinuing_job_v1(&mut project, job);
        }
        return Ok(progress_v1(job));
    }
    if !job_matches_continuing_project_v1(job, &project) {
        // A replacement project owns neither this ticket nor its history mark.
        // Do not let an obsolete retry operate on the replacement.
        close_noncontinuing_job_v1(&mut project, job);
        return Ok(progress_v1(job));
    }
    if matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::CertifiedRecovery,
            ..
        }
    ) {
        // The typed resolver has already consumed its proof. Its exact
        // post-call inspection must take precedence over a concurrently
        // reached start deadline.
        settle_resolving_start_retry_v1(&mut project, job)?;
        return Ok(progress_v1(job));
    }
    if let Some(stop_terminal) = lifecycle_or_pending_stop_terminal_v1(job, &project, now) {
        try_resolve_start_terminal_v1(&mut project, job, stop_terminal)
            .map_err(|_| unavailable_message_v1())?;
        return Ok(progress_v1(job));
    }

    if job.frontend_started {
        // Start is an idempotent handshake. Once accepted, later document or
        // pose edits do not invalidate the self-contained historical premise.
        // A retry may also be the first observer of a worker result whose
        // original poll reply was lost, so commit that result before exposing
        // terminal progress.
        settle_resolving_start_retry_v1(&mut project, job)?;
        return Ok(progress_v1(job));
    }

    job.frontend_started = true;
    settle_resolving_start_retry_v1(&mut project, job)?;
    Ok(progress_v1(job))
}

fn settle_resolving_start_retry_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
) -> Result<(), String> {
    let PostApplyProofJobStateV1::Resolving { resolution, .. } = &job.state else {
        return Ok(());
    };
    let terminal = resolution.terminal();
    if terminal == PostApplyProofTerminalV1::Certified {
        resolve_locked_terminal_v1(project, job, terminal).map_err(|_| unavailable_message_v1())
    } else {
        try_resolve_start_terminal_v1(project, job, terminal).map_err(|_| unavailable_message_v1())
    }
}

fn try_resolve_start_terminal_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
    terminal: PostApplyProofTerminalV1,
) -> Result<(), ()> {
    #[cfg(test)]
    if take_post_apply_start_fail_closed_resolution_failure_for_test_v1() {
        return Err(());
    }
    // Install the retryable typed terminal before signalling an InFlight
    // worker. If exact publication fails, its cooperative Cancelled result is
    // stale and cannot relabel a deadline or infrastructure-owned stop.
    resolve_locked_terminal_v1(project, job, terminal)
}

#[tauri::command]
pub(crate) async fn poll_post_apply_proof_job_v1(
    app_state: State<'_, AppState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    request: PostApplyProofJobRequestV1,
) -> Result<PostApplyProofProgressV1, String> {
    poll_post_apply_proof_job_inner_v1(&app_state, &transaction_state, request).await
}

async fn poll_post_apply_proof_job_inner_v1(
    app_state: &AppState,
    transaction_state: &StackedFoldTransactionState,
    request: PostApplyProofJobRequestV1,
) -> Result<PostApplyProofProgressV1, String> {
    validate_job_request_v1(&request)?;
    let worker_permit = app_state.try_acquire_native_pose_worker();
    let work = {
        let now = Instant::now();
        let mut project = lock_project(app_state).map_err(|_| unavailable_message_v1())?;
        let mut registry =
            lock_registry_v1(transaction_state).map_err(|_| unavailable_message_v1())?;
        let Some(index) = find_job_index_v1(&registry, &request) else {
            return Err(unavailable_message_v1());
        };
        let job = &mut registry.jobs[index];
        if !job_matches_continuing_project_v1(job, &project) {
            close_noncontinuing_job_v1(&mut project, job);
            return Ok(progress_v1(job));
        }
        if matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
            refresh_terminal_report_v1(&project, job);
            return Ok(progress_v1(job));
        }
        if let Some(stop_terminal) = cancellation_or_deadline_terminal_v1(job, now) {
            let _ = resolve_locked_terminal_v1(&mut project, job, stop_terminal);
            return Ok(progress_v1(job));
        }
        match &job.state {
            PostApplyProofJobStateV1::InFlight { .. } => {
                return Ok(progress_v1(job));
            }
            PostApplyProofJobStateV1::Resolving { resolution, .. } => {
                let terminal = resolution.terminal();
                let _ = resolve_locked_terminal_v1(&mut project, job, terminal);
                return Ok(progress_v1(job));
            }
            PostApplyProofJobStateV1::Ready { next_stage } => {
                let next_stage = *next_stage;
                let Some(worker_permit) = worker_permit else {
                    return Ok(progress_v1(job));
                };
                let run_generation = registry
                    .next_run_generation
                    .checked_add(1)
                    .ok_or_else(unavailable_message_v1)?;
                registry.next_run_generation = run_generation;
                let job = &mut registry.jobs[index];
                let premise = job.premise.take().ok_or_else(unavailable_message_v1)?;
                let cancellation = Arc::new(AtomicBool::new(false));
                let proof_deadline = job.proof_deadline;
                job.resource_recovery_cancelled_run_generation = None;
                job.state = PostApplyProofJobStateV1::InFlight {
                    run_generation,
                    stage: next_stage,
                    cancellation: Arc::clone(&cancellation),
                };
                Some((
                    worker_permit,
                    run_generation,
                    next_stage,
                    premise,
                    cancellation,
                    proof_deadline,
                ))
            }
            PostApplyProofJobStateV1::Terminal(_) => unreachable!("handled above"),
        }
    };

    let Some((worker_permit, run_generation, stage, premise, cancellation, proof_deadline)) = work
    else {
        return Err(unavailable_message_v1());
    };
    let interval_count = POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[stage];
    let registry = Arc::clone(&transaction_state.3);
    let worker_request = request.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        inject_post_apply_worker_panic_for_test_v1(&worker_request.job_token);
        let attempt = run_attempt_v1(
            &worker_request.job_token,
            premise,
            interval_count,
            &cancellation,
            proof_deadline,
        );
        complete_worker_attempt_v1(&registry, &worker_request, run_generation, stage, attempt);
        drop(worker_permit);
    })
    .await;
    finish_worker_poll_v1(
        app_state,
        transaction_state,
        &request,
        run_generation,
        joined.is_err(),
    )
}

#[tauri::command]
pub(crate) fn cancel_post_apply_proof_job_v1(
    app_state: State<'_, AppState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    request: PostApplyProofJobRequestV1,
) -> Result<(), String> {
    cancel_post_apply_proof_job_inner_v1(&app_state, &transaction_state, request)
}

fn cancel_post_apply_proof_job_inner_v1(
    app_state: &AppState,
    transaction_state: &StackedFoldTransactionState,
    request: PostApplyProofJobRequestV1,
) -> Result<(), String> {
    validate_job_request_v1(&request)?;
    let mut project = lock_project(app_state).map_err(|_| unavailable_message_v1())?;
    let mut registry = lock_registry_v1(transaction_state).map_err(|_| unavailable_message_v1())?;
    let Some(index) = find_job_index_v1(&registry, &request) else {
        return Err(unavailable_message_v1());
    };
    let job = &mut registry.jobs[index];
    if !job_matches_continuing_project_v1(job, &project) {
        close_noncontinuing_job_v1(&mut project, job);
        return Ok(());
    }
    if matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
        refresh_terminal_report_v1(&project, job);
        return Ok(());
    }
    let _ = resolve_locked_terminal_v1(
        &mut project,
        job,
        PostApplyProofTerminalV1::UnknownCancelled,
    );
    Ok(())
}
