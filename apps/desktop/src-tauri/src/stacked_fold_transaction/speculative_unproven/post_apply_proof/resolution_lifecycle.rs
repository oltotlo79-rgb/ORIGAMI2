fn complete_worker_attempt_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
    request: &PostApplyProofJobRequestV1,
    run_generation: u64,
    stage: usize,
    worker_attempt: PostApplyProofWorkerAttemptV1,
) {
    let Ok(mut registry) = registry.lock() else {
        return;
    };
    let Some(index) = find_job_index_v1(&registry, request) else {
        return;
    };
    let job = &mut registry.jobs[index];
    if !run_result_is_current_v1(&job.state, run_generation, stage) {
        return;
    }
    let internal_resource_stop = job.resource_recovery_cancelled_run_generation
        == Some(run_generation)
        && worker_attempt.certificate.state() == PostApplyProofCertificateStateV1::Cancelled;
    if !internal_resource_stop {
        job.resource_recovery_cancelled_run_generation = None;
    }
    let worker_attempt = if internal_resource_stop {
        PostApplyProofWorkerAttemptV1 {
            diagnostic: worker_attempt.diagnostic,
            certificate: match worker_attempt.certificate {
                PostApplyProofWorkerCertificateV1::Cancelled(premise) => {
                    PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
                }
                _ => unreachable!("the internal resource stop was a cancelled certificate"),
            },
        }
    } else {
        worker_attempt
    };
    let attempt = if internal_resource_stop {
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit)
    } else {
        match worker_attempt.certificate.state() {
            PostApplyProofCertificateStateV1::Cancelled => {
                AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled)
            }
            PostApplyProofCertificateStateV1::DeadlineExceeded => {
                AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownDeadlineReached)
            }
            certificate => {
                classify_attempt_v1(stage, worker_attempt.diagnostic.as_ref().ok(), certificate)
            }
        }
    };
    let Some(stage_work) = POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[stage]
        .checked_mul(POST_APPLY_PROOF_MAX_DIAGNOSTIC_PASSES_PER_STAGE_V1)
    else {
        job.premise = worker_attempt.certificate.into_recoverable_premise();
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownResourceLimit,
            ),
        };
        return;
    };
    let Some(cumulative_work) = job
        .cumulative_work
        .checked_add(stage_work)
        .filter(|work| *work <= POST_APPLY_PROOF_TOTAL_WORK_V1)
    else {
        job.premise = worker_attempt.certificate.into_recoverable_premise();
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownResourceLimit,
            ),
        };
        return;
    };
    job.cumulative_work = cumulative_work;
    let terminal = match (internal_resource_stop, worker_attempt.certificate.state()) {
        (true, _) => Some(PostApplyProofTerminalV1::UnknownResourceLimit),
        (false, PostApplyProofCertificateStateV1::Cancelled) => {
            Some(PostApplyProofTerminalV1::UnknownCancelled)
        }
        (false, PostApplyProofCertificateStateV1::DeadlineExceeded) => {
            Some(PostApplyProofTerminalV1::UnknownDeadlineReached)
        }
        (false, _) => terminal_after_attempt_v1(deadline_reached_v1(job, Instant::now()), attempt),
    };
    match (terminal, worker_attempt.certificate) {
        (
            Some(PostApplyProofTerminalV1::Certified),
            PostApplyProofWorkerCertificateV1::Certified(proof),
        ) => {
            job.state = PostApplyProofJobStateV1::Resolving {
                run_generation,
                resolution: PostApplyProofResolutionV1::Certified(proof),
            };
        }
        (Some(terminal), certificate) => {
            job.premise = certificate.into_recoverable_premise();
            job.state = PostApplyProofJobStateV1::Resolving {
                run_generation,
                resolution: PostApplyProofResolutionV1::Failure(terminal),
            };
        }
        (None, certificate) => {
            let Some(premise) = certificate.into_recoverable_premise() else {
                job.state = PostApplyProofJobStateV1::Resolving {
                    run_generation,
                    resolution: PostApplyProofResolutionV1::Failure(
                        PostApplyProofTerminalV1::UnknownEvidenceInsufficient,
                    ),
                };
                return;
            };
            job.premise = Some(premise);
            job.state = PostApplyProofJobStateV1::Ready {
                next_stage: stage + 1,
            };
        }
    }
}

fn finish_worker_poll_v1(
    app_state: &AppState,
    transaction_state: &StackedFoldTransactionState,
    request: &PostApplyProofJobRequestV1,
    expected_run_generation: u64,
    join_failed: bool,
) -> Result<PostApplyProofProgressV1, String> {
    let mut project = lock_project(app_state).map_err(|_| unavailable_message_v1())?;
    let now = Instant::now();
    let mut registry = lock_registry_v1(transaction_state).map_err(|_| unavailable_message_v1())?;
    let Some(index) = find_job_index_v1(&registry, request) else {
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
    if join_failed
        && matches!(
            &job.state,
            PostApplyProofJobStateV1::InFlight { run_generation, .. }
                if *run_generation == expected_run_generation
        )
    {
        let _ = resolve_locked_terminal_v1(
            &mut project,
            job,
            PostApplyProofTerminalV1::UnknownResourceLimit,
        );
        return Ok(progress_v1(job));
    }
    let requested_terminal = match &job.state {
        PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution,
        } if *run_generation == expected_run_generation => Some(resolution.terminal()),
        PostApplyProofJobStateV1::Ready { .. } | PostApplyProofJobStateV1::InFlight { .. } => None,
        PostApplyProofJobStateV1::Resolving { .. } | PostApplyProofJobStateV1::Terminal(_) => {
            return Ok(progress_v1(job));
        }
    };
    if let Some(stop_terminal) = cancellation_or_deadline_terminal_v1(job, now) {
        let _ = resolve_locked_terminal_v1(&mut project, job, stop_terminal);
    } else if let Some(terminal) = requested_terminal {
        let _ = resolve_locked_terminal_v1(&mut project, job, terminal);
    }
    Ok(progress_v1(job))
}

fn resolve_locked_terminal_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
    terminal: PostApplyProofTerminalV1,
) -> Result<(), ()> {
    if certified_resolution_pending_v1(job) {
        // Once typed positive authority exists, its consumption/recovery must
        // finish before cancellation/deadline/resource policy may overwrite
        // the only indication that Certified can or could have committed.
        return resolve_locked_certified_terminal_v1(project, job);
    }
    if terminal == PostApplyProofTerminalV1::Certified {
        return resolve_locked_certified_terminal_v1(project, job);
    }
    // The worker may still own the premise while this generic terminal is
    // committed. Signal its unique InFlight token before replacing state so a
    // late proof cannot be published over cancellation, deadline, lifecycle,
    // or resource closure.
    signal_inflight_cancellation_v1(job);
    let terminal = if terminal == PostApplyProofTerminalV1::Stale {
        PostApplyProofTerminalV1::UnknownEvidenceInsufficient
    } else {
        terminal
    };
    let Some(outcome) = terminal_outcome_v1(terminal) else {
        return Err(());
    };
    if !matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Failure(actual),
            ..
        } if *actual == terminal
    ) {
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation: 0,
            resolution: PostApplyProofResolutionV1::Failure(terminal),
        };
    }
    let report = resolve_generic_binding_v1(project, &job.job_token, &job.binding, outcome)?;
    if report.outcome != outcome {
        mark_stale_v1(job);
        return Ok(());
    }
    job.state = PostApplyProofJobStateV1::Terminal(terminal);
    job.resource_recovery_cancelled_run_generation = None;
    job.premise = None;
    job.resolution_report = Some(report);
    wake_deadline_scheduler_v1();
    Ok(())
}

#[allow(clippy::result_large_err)]
fn resolve_locked_certified_terminal_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
) -> Result<(), ()> {
    let panic_position = take_post_apply_certified_resolution_panic_v1(&job.job_token);
    if panic_position == 1
        && matches!(
            &job.state,
            PostApplyProofJobStateV1::Resolving {
                resolution: PostApplyProofResolutionV1::Certified(_),
                ..
            }
        )
    {
        // Exercise the unwind boundary before moving the non-cloneable proof
        // out of its durable job owner. A panic at this boundary must leave the
        // exact authority available to the next owner-correct retry.
        let injected = catch_unwind(AssertUnwindSafe(|| {
            panic!("injected panic before certified post-Apply resolution");
        }));
        debug_assert!(injected.is_err());
        return Err(());
    }
    let state = std::mem::replace(
        &mut job.state,
        PostApplyProofJobStateV1::Resolving {
            run_generation: 0,
            resolution: PostApplyProofResolutionV1::CertifiedRecovery,
        },
    );
    match state {
        PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution: PostApplyProofResolutionV1::Certified(proof),
        } => {
            // The opaque proof is one-shot. Install a retryable recovery owner
            // before handing it to core so an unwind can never expose Stale or
            // abandon an exact Awaiting mark.
            job.state = PostApplyProofJobStateV1::Resolving {
                run_generation,
                resolution: PostApplyProofResolutionV1::CertifiedRecovery,
            };
            job.resolution_report = None;
            // The error path must return the same non-cloneable proof so the
            // durable job owner can restore it without allocation or loss.
            let attempted =
                catch_unwind(AssertUnwindSafe(
                    || match try_resolve_certified_authority_v1(project, proof) {
                        Ok(report) => {
                            if panic_position == 2 {
                                panic!("injected panic after certified post-Apply resolution");
                            }
                            Ok(report)
                        }
                        Err(rejected) => Err(rejected),
                    },
                ));
            match attempted {
                Ok(Ok(report))
                    if report.outcome == SpeculativeUnprovenFoldProofOutcomeV1::Certified =>
                {
                    job.state =
                        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified);
                    job.resource_recovery_cancelled_run_generation = None;
                    job.premise = None;
                    job.resolution_report = None;
                    wake_deadline_scheduler_v1();
                    return Ok(());
                }
                Ok(Err((_error, proof))) => {
                    // Every ordinary core rejection is non-consuming. Restore
                    // the same typed proof under the same generation owner;
                    // cancellation, deadline, lifecycle drift, and resource
                    // recovery may observe it but may not replace it.
                    job.state = PostApplyProofJobStateV1::Resolving {
                        run_generation,
                        resolution: PostApplyProofResolutionV1::Certified(proof),
                    };
                    job.resolution_report = None;
                    return Err(());
                }
                Ok(Ok(_)) | Err(_) => {}
            }
        }
        PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution: PostApplyProofResolutionV1::CertifiedRecovery,
        } => {
            job.state = PostApplyProofJobStateV1::Resolving {
                run_generation,
                resolution: PostApplyProofResolutionV1::CertifiedRecovery,
            };
        }
        other => {
            // Certified resolution is valid only for an explicitly retained
            // typed authority or its post-call recovery marker. Preserve any
            // unrelated owner verbatim instead of relabelling it Certified.
            job.state = other;
            return Err(());
        }
    }
    recover_consumed_certified_resolution_v1(project, job)
}

type RejectedPostApplyProofCertifiedAuthorityV1 = (
    SpeculativeUnprovenFoldResolutionErrorV1,
    PostApplyProofCertifiedAuthorityV1,
);

#[allow(clippy::result_large_err)]
fn try_resolve_certified_authority_v1(
    project: &mut ProjectState,
    proof: PostApplyProofCertifiedAuthorityV1,
) -> Result<SpeculativeUnprovenFoldResolutionReportV1, RejectedPostApplyProofCertifiedAuthorityV1> {
    match proof {
        PostApplyProofCertifiedAuthorityV1::Tree(proof) => project
            .editor
            .try_resolve_speculative_unproven_fold_certified_v1(proof)
            .map_err(|failure| {
                let (error, proof) = failure.into_parts();
                (error, PostApplyProofCertifiedAuthorityV1::Tree(proof))
            }),
        PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(proof) => project
            .editor
            .try_resolve_speculative_unproven_fold_layered_three_face_certified_v1(proof)
            .map_err(|failure| {
                let (error, proof) = failure.into_parts();
                (
                    error,
                    PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(proof),
                )
            }),
        PostApplyProofCertifiedAuthorityV1::LayeredFourFace(proof) => project
            .editor
            .try_resolve_speculative_unproven_fold_layered_four_face_certified_v1(proof)
            .map_err(|failure| {
                let (error, proof) = failure.into_parts();
                (
                    error,
                    PostApplyProofCertifiedAuthorityV1::LayeredFourFace(proof),
                )
            }),
    }
}

fn recover_consumed_certified_resolution_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
) -> Result<(), ()> {
    let inspected = catch_unwind(AssertUnwindSafe(|| {
        project
            .editor
            .inspect_speculative_unproven_fold_v1(&job.binding)
    }));
    match inspected {
        Ok(Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)) => {
            // Certified resolution removes the exact mark. Absence is
            // authoritative here because the caller held the project lock
            // continuously from the pre-call Awaiting inspection.
            job.state = PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified);
            job.resource_recovery_cancelled_run_generation = None;
            job.premise = None;
            job.resolution_report = None;
            wake_deadline_scheduler_v1();
            Ok(())
        }
        Ok(Ok(None)) => {
            // The resolver unwound after ownership crossed the call boundary,
            // but the exact mark is still Awaiting. Do not fabricate a
            // ResourceLimit terminal or allow a concurrent stop to overwrite
            // the positive recovery owner. A later owner retry inspects the
            // same mark under the project lock.
            job.resolution_report = None;
            Err(())
        }
        Ok(Ok(Some(report))) => {
            let terminal = terminal_for_observed_outcome_v1(report.outcome);
            job.state = PostApplyProofJobStateV1::Terminal(terminal);
            job.resource_recovery_cancelled_run_generation = None;
            job.premise = None;
            job.resolution_report =
                (terminal != PostApplyProofTerminalV1::Certified).then_some(report);
            wake_deadline_scheduler_v1();
            Ok(())
        }
        Ok(Err(_)) | Err(_) => Err(()),
    }
}

fn terminal_for_observed_outcome_v1(
    outcome: SpeculativeUnprovenFoldProofOutcomeV1,
) -> PostApplyProofTerminalV1 {
    match outcome {
        SpeculativeUnprovenFoldProofOutcomeV1::Certified => PostApplyProofTerminalV1::Certified,
        SpeculativeUnprovenFoldProofOutcomeV1::Blocked => PostApplyProofTerminalV1::Blocked,
        SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
            reason: SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient,
        } => PostApplyProofTerminalV1::UnknownEvidenceInsufficient,
        SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
            reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
        } => PostApplyProofTerminalV1::UnknownResourceLimit,
        SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
            reason: SpeculativeUnprovenFoldUnknownReasonV1::Cancelled,
        } => PostApplyProofTerminalV1::UnknownCancelled,
        SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
            reason: SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached,
        } => PostApplyProofTerminalV1::UnknownDeadlineReached,
    }
}

fn take_post_apply_certified_resolution_panic_v1(job_token: &ProjectId) -> usize {
    #[cfg(test)]
    {
        take_process_global_one_shot_fault_if_v1(
            &PANIC_NEXT_POST_APPLY_CERTIFIED_RESOLUTION_V1,
            |target| target.job_token == *job_token,
        )
        .map_or(0, |target| target.position)
    }
    #[cfg(not(test))]
    {
        let _ = job_token;
        0
    }
}

#[cfg(test)]
fn take_post_apply_generic_resolution_failure_for_test_v1(job_token: &ProjectId) -> bool {
    take_process_global_one_shot_fault_if_v1(
        &FAIL_NEXT_POST_APPLY_GENERIC_RESOLUTION_V1,
        |target| target == job_token,
    )
    .is_some()
}

fn resolve_generic_binding_v1(
    project: &mut ProjectState,
    job_token: &ProjectId,
    binding: &SpeculativeUnprovenFoldBindingV1,
    outcome: SpeculativeUnprovenFoldProofOutcomeV1,
) -> Result<SpeculativeUnprovenFoldResolutionReportV1, ()> {
    #[cfg(not(test))]
    let _ = job_token;
    #[cfg(test)]
    if take_post_apply_generic_resolution_failure_for_test_v1(job_token) {
        return Err(());
    }
    match catch_unwind(AssertUnwindSafe(|| {
        project.editor.inspect_speculative_unproven_fold_v1(binding)
    })) {
        Ok(Ok(Some(report))) if report.outcome == outcome => return Ok(report),
        Ok(Ok(None)) => {}
        _ => return Err(()),
    }
    catch_unwind(AssertUnwindSafe(|| {
        project
            .editor
            .resolve_speculative_unproven_fold_v1(binding, outcome)
    }))
    .map_err(|_| ())?
    .map_err(|_| ())
}
