fn expire_due_deadline_registrations_v1(
    registrations: &mut Vec<DeadlineSchedulerRegistrationV1>,
    now: Instant,
) {
    let mut index = 0;
    while index < registrations.len() {
        let Some(registry_handle) = registrations[index].registry.upgrade() else {
            registrations.swap_remove(index);
            continue;
        };
        let protected_through = deadline_resource_retry_scope_v1(&registrations[index]);
        let due = {
            let registry = lock_deadline_registry_recover_v1(&registry_handle);
            registry
                .jobs
                .iter()
                .filter(|job| {
                    !job_is_protected_by_deadline_resource_retry_v1(job, protected_through)
                })
                .any(|job| {
                    if matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
                        now >= job.retain_until
                    } else {
                        deadline_reached_v1(job, now)
                    }
                })
        };
        if due {
            let Some(project_handle) = registrations[index].project.upgrade() else {
                let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
                clear_jobs_v1(&mut registry);
                registry.deadline_scheduler_registered = false;
                registrations.swap_remove(index);
                continue;
            };
            // The scheduler uses the same global order as command handlers:
            // canonical project first, then the post-Apply registry.
            let mut project = lock_deadline_project_recover_v1(&project_handle);
            let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
            let retry_scope = Some(registry.next_scheduler_generation);
            let expired = expire_due_jobs_outside_resource_retry_scope_locked_v1(
                &mut project,
                &mut registry,
                now,
                protected_through,
            );
            if expired.is_err() {
                signal_inflight_cancellation_within_deadline_resource_retry_scope_v1(
                    &mut registry,
                    retry_scope,
                );
            }
            drop(registry);
            drop(project);
            if expired.is_err() {
                if !schedule_deadline_registration_resource_retry_v1(
                    &mut registrations[index],
                    now,
                    retry_scope,
                ) {
                    let disposition =
                        retain_exhausted_deadline_resource_failure_v1(&registrations[index]);
                    match disposition {
                        DeadlineSchedulerRecoveryDispositionV1::DropRegistration => {
                            registrations.swap_remove(index);
                            continue;
                        }
                        DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry => {
                            registrations[index].resource_retry = None;
                        }
                        DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry => {
                            restart_exhausted_deadline_resource_retry_v1(
                                &mut registrations[index],
                                now,
                            );
                        }
                    }
                }
            }
        }
        index += 1;
    }
}

fn prune_and_next_deadline_v1(
    registrations: &mut Vec<DeadlineSchedulerRegistrationV1>,
) -> Option<Instant> {
    let mut next_deadline: Option<Instant> = None;
    let mut index = 0;
    while index < registrations.len() {
        let Some(registry_handle) = registrations[index].registry.upgrade() else {
            registrations.swap_remove(index);
            continue;
        };
        let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
        let registration_deadline =
            next_deadline_for_registration_v1(&registry, registrations[index].resource_retry);
        let Some(registration_deadline) = registration_deadline else {
            registry.deadline_scheduler_registered = false;
            drop(registry);
            registrations.swap_remove(index);
            continue;
        };
        next_deadline = Some(next_deadline.map_or(registration_deadline, |current| {
            current.min(registration_deadline)
        }));
        index += 1;
    }
    next_deadline
}

fn next_deadline_for_registration_v1(
    registry: &PostApplyProofRegistryV1,
    resource_retry: Option<DeadlineSchedulerResourceRetryV1>,
) -> Option<Instant> {
    let protected_through = resource_retry.map(|retry| retry.through_scheduler_generation);
    let job_deadline = registry
        .jobs
        .iter()
        .filter(|job| !job_is_protected_by_deadline_resource_retry_v1(job, protected_through))
        .map(|job| {
            if matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
                job.retain_until
            } else {
                job.proof_deadline
            }
        })
        .min();
    match resource_retry {
        Some(retry) => {
            Some(job_deadline.map_or(retry.not_before, |deadline| deadline.min(retry.not_before)))
        }
        None => job_deadline,
    }
}

#[cfg(test)]
fn expire_due_jobs_locked_v1(
    project: &mut ProjectState,
    registry: &mut PostApplyProofRegistryV1,
    now: Instant,
) -> Result<(), ()> {
    expire_due_jobs_outside_resource_retry_scope_locked_v1(project, registry, now, None)
}

fn expire_due_jobs_outside_resource_retry_scope_locked_v1(
    project: &mut ProjectState,
    registry: &mut PostApplyProofRegistryV1,
    now: Instant,
    protected_through_scheduler_generation: Option<u64>,
) -> Result<(), ()> {
    let mut index = 0;
    while index < registry.jobs.len() {
        if job_is_protected_by_deadline_resource_retry_v1(
            &registry.jobs[index],
            protected_through_scheduler_generation,
        ) {
            index += 1;
            continue;
        }
        if matches!(
            &registry.jobs[index].state,
            PostApplyProofJobStateV1::Terminal(_)
        ) {
            if now >= registry.jobs[index].retain_until {
                remove_job_v1(registry, index);
            } else {
                index += 1;
            }
            continue;
        }
        if !deadline_reached_v1(&registry.jobs[index], now) {
            index += 1;
            continue;
        }
        let remove_after_resolution = !registry.jobs[index].frontend_started;
        let job = &mut registry.jobs[index];
        if !job_matches_continuing_project_v1(job, project) {
            close_noncontinuing_job_v1(project, job);
            if !matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
                return Err(());
            }
        } else {
            let stop_terminal = lifecycle_or_pending_stop_terminal_v1(job, project, now)
                .unwrap_or(PostApplyProofTerminalV1::UnknownDeadlineReached);
            match catch_unwind(AssertUnwindSafe(|| {
                inject_deadline_resolution_panic_for_test_v1(job);
                resolve_locked_terminal_v1(project, job, stop_terminal)
            })) {
                Ok(Ok(())) => {}
                Ok(Err(())) => return Err(()),
                Err(_) => {
                    recover_deadline_resolution_panic_v1(project, job, stop_terminal)?;
                }
            }
        }
        if remove_after_resolution && matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
            remove_job_v1(registry, index);
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn recover_deadline_resolution_panic_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
    requested_terminal: PostApplyProofTerminalV1,
) -> Result<(), ()> {
    if requested_terminal == PostApplyProofTerminalV1::UnknownCancelled {
        signal_inflight_cancellation_v1(job);
    } else {
        signal_inflight_resource_recovery_cancellation_v1(job);
    }
    if fail_deadline_resolution_recovery_for_test_v1(job) {
        return Err(());
    }
    let Some(requested_outcome) = terminal_outcome_v1(requested_terminal) else {
        return Err(());
    };
    let resource_terminal = PostApplyProofTerminalV1::UnknownResourceLimit;
    let Some(resource_outcome) = terminal_outcome_v1(resource_terminal) else {
        return Err(());
    };
    let inspected = catch_unwind(AssertUnwindSafe(|| {
        project
            .editor
            .inspect_speculative_unproven_fold_v1(&job.binding)
    }));
    if let Ok(Ok(Some(report))) = inspected {
        let terminal = if report.outcome == requested_outcome {
            requested_terminal
        } else if report.outcome == resource_outcome {
            resource_terminal
        } else {
            return Err(());
        };
        job.state = PostApplyProofJobStateV1::Terminal(terminal);
        job.resource_recovery_cancelled_run_generation = None;
        job.premise = None;
        job.resolution_report = Some(report);
        wake_deadline_scheduler_v1();
        return Ok(());
    }

    let (fallback_terminal, fallback_outcome) =
        if requested_terminal == PostApplyProofTerminalV1::UnknownCancelled {
            (requested_terminal, requested_outcome)
        } else {
            (resource_terminal, resource_outcome)
        };
    match catch_unwind(AssertUnwindSafe(|| {
        project
            .editor
            .resolve_speculative_unproven_fold_v1(&job.binding, fallback_outcome)
    })) {
        Ok(Ok(report)) if report.outcome == fallback_outcome => {
            job.state = PostApplyProofJobStateV1::Terminal(fallback_terminal);
            job.resource_recovery_cancelled_run_generation = None;
            job.premise = None;
            job.resolution_report = Some(report);
            wake_deadline_scheduler_v1();
            Ok(())
        }
        _ => Err(()),
    }
}

fn lock_deadline_project_recover_v1(
    project: &Arc<Mutex<ProjectState>>,
) -> MutexGuard<'_, ProjectState> {
    project
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn lock_deadline_registry_recover_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> MutexGuard<'_, PostApplyProofRegistryV1> {
    registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn deadline_reached_v1(job: &PostApplyProofJobV1, now: Instant) -> bool {
    now >= job.proof_deadline
}

fn cancellation_resolution_pending_v1(job: &PostApplyProofJobV1) -> bool {
    matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownCancelled
            ),
            ..
        }
    )
}

fn certified_resolution_pending_v1(job: &PostApplyProofJobV1) -> bool {
    matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Certified(_)
                | PostApplyProofResolutionV1::CertifiedRecovery,
            ..
        }
    )
}

fn resource_recovery_resolution_pending_v1(job: &PostApplyProofJobV1) -> bool {
    matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            run_generation,
            resolution: PostApplyProofResolutionV1::Failure(
                PostApplyProofTerminalV1::UnknownResourceLimit
            ),
        } if job.resource_recovery_cancelled_run_generation == Some(*run_generation)
    )
}

fn lifecycle_or_pending_stop_terminal_v1(
    job: &PostApplyProofJobV1,
    project: &ProjectState,
    now: Instant,
) -> Option<PostApplyProofTerminalV1> {
    if resource_recovery_resolution_pending_v1(job) {
        // Once an internally stopped worker has returned its premise, later
        // lifecycle drift cannot relabel the scheduler's resource failure.
        Some(PostApplyProofTerminalV1::UnknownResourceLimit)
    } else if !unstarted_job_matches_live_binding_v1(job, project) {
        // Lifecycle cancellation wins over a deadline first observed at the
        // same start/scheduler boundary.
        Some(PostApplyProofTerminalV1::UnknownCancelled)
    } else {
        cancellation_or_deadline_terminal_v1(job, now)
    }
}

fn cancellation_or_deadline_terminal_v1(
    job: &PostApplyProofJobV1,
    now: Instant,
) -> Option<PostApplyProofTerminalV1> {
    // Keep command, worker-join, and scheduler publication consistent with
    // CooperativeOperationControlV1: explicit cancellation wins when both
    // stop conditions are observable at the same boundary.
    if cancellation_resolution_pending_v1(job) {
        Some(PostApplyProofTerminalV1::UnknownCancelled)
    } else if resource_recovery_resolution_pending_v1(job) {
        // A scheduler-owned cancellation only releases native work after its
        // exact resource-resolution path failed. Keep that infrastructure
        // origin through the joining command boundary; an already elapsed
        // proof deadline must not relabel it as a user-visible timeout.
        Some(PostApplyProofTerminalV1::UnknownResourceLimit)
    } else if deadline_reached_v1(job, now) {
        Some(PostApplyProofTerminalV1::UnknownDeadlineReached)
    } else {
        None
    }
}

fn signal_inflight_cancellation_v1(job: &PostApplyProofJobV1) {
    signal_inflight_cancellation_state_v1(&job.state);
}

fn signal_inflight_resource_recovery_cancellation_v1(job: &mut PostApplyProofJobV1) {
    if let PostApplyProofJobStateV1::InFlight { run_generation, .. } = &job.state {
        job.resource_recovery_cancelled_run_generation = Some(*run_generation);
    }
    signal_inflight_cancellation_v1(job);
}

fn signal_inflight_cancellation_state_v1(state: &PostApplyProofJobStateV1) {
    if let PostApplyProofJobStateV1::InFlight { cancellation, .. } = state {
        cancellation.store(true, Ordering::Release);
    }
}
