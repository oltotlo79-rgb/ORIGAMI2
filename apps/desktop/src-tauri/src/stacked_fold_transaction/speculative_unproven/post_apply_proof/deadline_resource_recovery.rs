fn fail_all_deadline_registrations_resource_v1(
    registrations: &mut Vec<DeadlineSchedulerRegistrationV1>,
) {
    let retained_count = registrations.len();
    for _ in 0..retained_count {
        if registrations.is_empty() {
            break;
        }
        // Retained registrations are appended below. Removing the oldest
        // original entry preserves the unprocessed suffix, so every
        // registration present at entry is recovered exactly once.
        let mut registration = registrations.remove(0);
        let retry_scope = deadline_resource_retry_scope_v1(&registration)
            .or_else(|| deadline_resource_retry_scope_snapshot_v1(&registration));
        match fail_deadline_registration_resource_isolated_v1(&registration, retry_scope) {
            Some(DeadlineSchedulerRecoveryDispositionV1::DropRegistration) => continue,
            Some(DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry) => {
                registration.resource_retry = None;
                registrations.push(registration);
            }
            Some(DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry) => {
                restart_exhausted_deadline_resource_retry_v1(&mut registration, Instant::now());
                registrations.push(registration);
            }
            None => {
                let now = Instant::now();
                if schedule_deadline_registration_resource_retry_v1(
                    &mut registration,
                    now,
                    retry_scope,
                ) {
                    registrations.push(registration);
                } else {
                    match retain_exhausted_deadline_resource_failure_v1(&registration) {
                        DeadlineSchedulerRecoveryDispositionV1::DropRegistration => {}
                        DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry => {
                            registration.resource_retry = None;
                            registrations.push(registration);
                        }
                        DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry => {
                            restart_exhausted_deadline_resource_retry_v1(&mut registration, now);
                            registrations.push(registration);
                        }
                    }
                }
            }
        }
    }
}

fn retry_failed_deadline_registrations_v1(
    registrations: &mut Vec<DeadlineSchedulerRegistrationV1>,
    now: Instant,
) {
    let mut index = 0;
    while index < registrations.len() {
        let retry_due = registrations[index]
            .resource_retry
            .is_some_and(|retry| now >= retry.not_before);
        if !retry_due {
            index += 1;
            continue;
        }
        let retry_scope = deadline_resource_retry_scope_v1(&registrations[index])
            .or_else(|| deadline_resource_retry_scope_snapshot_v1(&registrations[index]));
        match fail_deadline_registration_resource_isolated_v1(&registrations[index], retry_scope) {
            Some(DeadlineSchedulerRecoveryDispositionV1::DropRegistration) => {
                registrations.swap_remove(index);
            }
            Some(DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry) => {
                registrations[index].resource_retry = None;
                index += 1;
            }
            Some(DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry) => {
                restart_exhausted_deadline_resource_retry_v1(&mut registrations[index], now);
                index += 1;
            }
            None => {
                if schedule_deadline_registration_resource_retry_v1(
                    &mut registrations[index],
                    now,
                    retry_scope,
                ) {
                    index += 1;
                } else {
                    let disposition =
                        retain_exhausted_deadline_resource_failure_v1(&registrations[index]);
                    match disposition {
                        DeadlineSchedulerRecoveryDispositionV1::DropRegistration => {
                            registrations.swap_remove(index);
                        }
                        DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry => {
                            registrations[index].resource_retry = None;
                            index += 1;
                        }
                        DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry => {
                            restart_exhausted_deadline_resource_retry_v1(
                                &mut registrations[index],
                                now,
                            );
                            index += 1;
                        }
                    }
                }
            }
        }
    }
}

fn schedule_deadline_registration_resource_retry_v1(
    registration: &mut DeadlineSchedulerRegistrationV1,
    now: Instant,
    initial_scope: Option<u64>,
) -> bool {
    let (attempt, first_failure_at, through_scheduler_generation) =
        registration.resource_retry.map_or_else(
            || (1, now, initial_scope.unwrap_or(0)),
            |retry| {
                (
                    retry.attempt.saturating_add(1),
                    retry.first_failure_at,
                    retry
                        .through_scheduler_generation
                        .max(initial_scope.unwrap_or(retry.through_scheduler_generation)),
                )
            },
        );
    if !deadline_resource_retry_allowed_v1(attempt, first_failure_at, now) {
        if let Some(retry) = &mut registration.resource_retry {
            retry.through_scheduler_generation = through_scheduler_generation;
        }
        return false;
    }
    let shift = attempt.min(POST_APPLY_DEADLINE_RETRY_MAX_SHIFT_V1);
    let delay = Duration::from_millis(1_u64 << shift);
    registration.resource_retry = Some(DeadlineSchedulerResourceRetryV1 {
        attempt,
        first_failure_at,
        through_scheduler_generation,
        not_before: now.checked_add(delay).unwrap_or(now),
    });
    true
}

fn deadline_resource_retry_allowed_v1(
    attempt: u32,
    first_failure_at: Instant,
    now: Instant,
) -> bool {
    attempt <= POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_ATTEMPTS_V1
        && now.saturating_duration_since(first_failure_at)
            < POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_DURATION_V1
}

fn restart_exhausted_deadline_resource_retry_v1(
    registration: &mut DeadlineSchedulerRegistrationV1,
    now: Instant,
) {
    let through_scheduler_generation = deadline_resource_retry_scope_v1(registration)
        .or_else(|| deadline_resource_retry_scope_snapshot_v1(registration))
        .unwrap_or(0);
    registration.resource_retry = Some(DeadlineSchedulerResourceRetryV1 {
        attempt: 0,
        first_failure_at: now,
        through_scheduler_generation,
        not_before: now
            .checked_add(POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_DURATION_V1)
            .unwrap_or(now),
    });
}

fn deadline_resource_retry_scope_v1(registration: &DeadlineSchedulerRegistrationV1) -> Option<u64> {
    registration
        .resource_retry
        .map(|retry| retry.through_scheduler_generation)
}

fn deadline_resource_retry_scope_snapshot_v1(
    registration: &DeadlineSchedulerRegistrationV1,
) -> Option<u64> {
    registration
        .registry
        .upgrade()
        .map(|registry| lock_deadline_registry_recover_v1(&registry).next_scheduler_generation)
}

fn job_is_within_deadline_resource_retry_scope_v1(
    job: &PostApplyProofJobV1,
    through_scheduler_generation: Option<u64>,
) -> bool {
    through_scheduler_generation.is_none_or(|through| job.scheduler_generation <= through)
}

fn job_is_protected_by_deadline_resource_retry_v1(
    job: &PostApplyProofJobV1,
    through_scheduler_generation: Option<u64>,
) -> bool {
    !matches!(&job.state, PostApplyProofJobStateV1::Terminal(_))
        && through_scheduler_generation.is_some_and(|through| job.scheduler_generation <= through)
}

fn fail_deadline_registration_resource_isolated_v1(
    registration: &DeadlineSchedulerRegistrationV1,
    retry_scope: Option<u64>,
) -> Option<DeadlineSchedulerRecoveryDispositionV1> {
    for _ in 0..2 {
        if let Ok(Ok(disposition)) = catch_unwind(AssertUnwindSafe(|| {
            fail_deadline_registration_resource_v1(registration, retry_scope)
        })) {
            return Some(disposition);
        }
    }
    None
}

fn fail_deadline_registration_resource_v1(
    registration: &DeadlineSchedulerRegistrationV1,
    retry_scope: Option<u64>,
) -> Result<DeadlineSchedulerRecoveryDispositionV1, ()> {
    let Some(registry_handle) = registration.registry.upgrade() else {
        return Ok(DeadlineSchedulerRecoveryDispositionV1::DropRegistration);
    };
    let Some(project_handle) = registration.project.upgrade() else {
        let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
        clear_jobs_v1(&mut registry);
        registry.deadline_scheduler_registered = false;
        return Ok(DeadlineSchedulerRecoveryDispositionV1::DropRegistration);
    };
    let mut project = lock_deadline_project_recover_v1(&project_handle);
    let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
    signal_inflight_cancellation_within_deadline_resource_retry_scope_v1(
        &mut registry,
        retry_scope,
    );
    if force_post_apply_deadline_resource_failure_for_test_v1(&registry_handle) {
        return Err(());
    }
    let Some(resource_outcome) =
        terminal_outcome_v1(PostApplyProofTerminalV1::UnknownResourceLimit)
    else {
        return Err(());
    };
    let mut index = 0;
    while index < registry.jobs.len() {
        if !job_is_within_deadline_resource_retry_scope_v1(&registry.jobs[index], retry_scope) {
            index += 1;
            continue;
        }
        if matches!(
            &registry.jobs[index].state,
            PostApplyProofJobStateV1::Terminal(_)
        ) {
            index += 1;
            continue;
        }
        let remove_after_resolution = !registry.jobs[index].frontend_started;
        let job = &mut registry.jobs[index];
        if job.binding.project_instance_id() != project.instance_id
            || job.binding.project_id() != project.project_id
        {
            mark_stale_v1(job);
            if remove_after_resolution {
                remove_job_v1(&mut registry, index);
            } else {
                index += 1;
            }
            continue;
        }
        if cancellation_resolution_pending_v1(job) {
            resolve_locked_terminal_v1(
                &mut project,
                job,
                PostApplyProofTerminalV1::UnknownCancelled,
            )?;
            if remove_after_resolution {
                remove_job_v1(&mut registry, index);
            } else {
                index += 1;
            }
            continue;
        }
        if certified_resolution_pending_v1(job) {
            // A typed proof may be awaiting consumption or may already have
            // committed Certified. Recover that exact outcome before this
            // infrastructure-failure path may substitute ResourceLimit.
            resolve_locked_certified_terminal_v1(&mut project, job)?;
            if !matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)) {
                return Err(());
            }
            if remove_after_resolution {
                remove_job_v1(&mut registry, index);
            } else {
                index += 1;
            }
            continue;
        }
        let report = match catch_unwind(AssertUnwindSafe(|| {
            project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding)
        })) {
            Ok(Ok(Some(report))) if report.outcome == resource_outcome => report,
            Ok(Ok(None)) => match catch_unwind(AssertUnwindSafe(|| {
                project
                    .editor
                    .resolve_speculative_unproven_fold_v1(&job.binding, resource_outcome)
            })) {
                Ok(Ok(report)) if report.outcome == resource_outcome => report,
                _ => return Err(()),
            },
            _ => return Err(()),
        };
        signal_inflight_cancellation_v1(job);
        job.state =
            PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit);
        job.resource_recovery_cancelled_run_generation = None;
        job.premise = None;
        job.resolution_report = Some(report);
        wake_deadline_scheduler_v1();
        if remove_after_resolution {
            remove_job_v1(&mut registry, index);
        } else {
            index += 1;
        }
    }
    let retain_for_terminal_expiry = !registry.jobs.is_empty();
    registry.deadline_scheduler_registered = retain_for_terminal_expiry;
    Ok(if retain_for_terminal_expiry {
        DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry
    } else {
        DeadlineSchedulerRecoveryDispositionV1::DropRegistration
    })
}

// Exhausting one bounded retry window never permits the dispatcher to orphan
// an exact editor-side Awaiting mark. A failed final resolution keeps the
// bounded registration/job owner and starts another delayed retry window.
// Only an exact typed terminal report (or disappearance of the owning
// project) permits the owner and its one-shot premise to be reclaimed.
fn retain_exhausted_deadline_resource_failure_v1(
    registration: &DeadlineSchedulerRegistrationV1,
) -> DeadlineSchedulerRecoveryDispositionV1 {
    let Some(registry_handle) = registration.registry.upgrade() else {
        return DeadlineSchedulerRecoveryDispositionV1::DropRegistration;
    };
    let Some(project_handle) = registration.project.upgrade() else {
        let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
        clear_jobs_v1(&mut registry);
        registry.deadline_scheduler_registered = false;
        return DeadlineSchedulerRecoveryDispositionV1::DropRegistration;
    };
    let mut project = lock_deadline_project_recover_v1(&project_handle);
    let mut registry = lock_deadline_registry_recover_v1(&registry_handle);
    let retry_scope = deadline_resource_retry_scope_v1(registration);
    signal_inflight_cancellation_within_deadline_resource_retry_scope_v1(
        &mut registry,
        retry_scope,
    );
    let resource_terminal = PostApplyProofTerminalV1::UnknownResourceLimit;
    let Some(resource_outcome) = terminal_outcome_v1(resource_terminal) else {
        registry.deadline_scheduler_registered = true;
        return DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry;
    };
    let mut unresolved_owner = false;
    let mut index = 0;
    while index < registry.jobs.len() {
        if !job_is_within_deadline_resource_retry_scope_v1(&registry.jobs[index], retry_scope) {
            index += 1;
            continue;
        }
        if matches!(
            &registry.jobs[index].state,
            PostApplyProofJobStateV1::Terminal(_)
        ) {
            index += 1;
            continue;
        }
        let remove_after_exact_resolution = !registry.jobs[index].frontend_started;
        let job = &mut registry.jobs[index];
        signal_inflight_cancellation_v1(job);
        if job.binding.project_instance_id() != project.instance_id
            || job.binding.project_id() != project.project_id
        {
            mark_stale_v1(job);
            if remove_after_exact_resolution {
                remove_job_v1(&mut registry, index);
            } else {
                index += 1;
            }
            continue;
        }
        if cancellation_resolution_pending_v1(job) {
            if resolve_locked_terminal_v1(
                &mut project,
                job,
                PostApplyProofTerminalV1::UnknownCancelled,
            )
            .is_err()
            {
                unresolved_owner = true;
                index += 1;
                continue;
            }
            if remove_after_exact_resolution {
                remove_job_v1(&mut registry, index);
            } else {
                index += 1;
            }
            continue;
        }
        if certified_resolution_pending_v1(job) {
            if resolve_locked_certified_terminal_v1(&mut project, job).is_ok()
                && matches!(&job.state, PostApplyProofJobStateV1::Terminal(_))
            {
                if remove_after_exact_resolution {
                    remove_job_v1(&mut registry, index);
                } else {
                    index += 1;
                }
                continue;
            }
            unresolved_owner = true;
            index += 1;
            continue;
        }
        let exact_report =
            if force_post_apply_deadline_resource_failure_for_test_v1(&registry_handle) {
                None
            } else {
                exact_resource_resolution_report_v1(&mut project, job, resource_outcome)
            };
        if let Some(report) = exact_report {
            job.state = PostApplyProofJobStateV1::Terminal(resource_terminal);
            job.resource_recovery_cancelled_run_generation = None;
            job.premise = None;
            job.resolution_report = Some(report);
            wake_deadline_scheduler_v1();
            if remove_after_exact_resolution {
                remove_job_v1(&mut registry, index);
            } else {
                index += 1;
            }
        } else {
            unresolved_owner = true;
            index += 1;
        }
    }
    let retain_for_terminal_expiry = !registry.jobs.is_empty();
    registry.deadline_scheduler_registered = retain_for_terminal_expiry;
    if unresolved_owner {
        DeadlineSchedulerRecoveryDispositionV1::RetainForResourceRetry
    } else if retain_for_terminal_expiry {
        DeadlineSchedulerRecoveryDispositionV1::RetainForTerminalExpiry
    } else {
        DeadlineSchedulerRecoveryDispositionV1::DropRegistration
    }
}

fn exact_resource_resolution_report_v1(
    project: &mut ProjectState,
    job: &PostApplyProofJobV1,
    resource_outcome: SpeculativeUnprovenFoldProofOutcomeV1,
) -> Option<SpeculativeUnprovenFoldResolutionReportV1> {
    match catch_unwind(AssertUnwindSafe(|| {
        project
            .editor
            .inspect_speculative_unproven_fold_v1(&job.binding)
    })) {
        Ok(Ok(Some(report))) if report.outcome == resource_outcome => Some(report),
        Ok(Ok(None)) => match catch_unwind(AssertUnwindSafe(|| {
            project
                .editor
                .resolve_speculative_unproven_fold_v1(&job.binding, resource_outcome)
        })) {
            Ok(Ok(report)) if report.outcome == resource_outcome => Some(report),
            _ => None,
        },
        _ => None,
    }
}

fn signal_inflight_cancellation_within_deadline_resource_retry_scope_v1(
    registry: &mut PostApplyProofRegistryV1,
    through_scheduler_generation: Option<u64>,
) {
    for job in &mut registry.jobs {
        if job_is_within_deadline_resource_retry_scope_v1(job, through_scheduler_generation) {
            signal_inflight_resource_recovery_cancellation_v1(job);
        }
    }
}
