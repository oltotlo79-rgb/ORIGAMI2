fn classify_attempt_v1(
    stage: usize,
    diagnostic: Option<&StackedFoldBoundedPathDiagnosticV1>,
    certificate: PostApplyProofCertificateStateV1,
) -> AttemptResultV1 {
    let has_blocking_witness = diagnostic
        .and_then(StackedFoldBoundedPathDiagnosticV1::first_sampled_blocking_angle_degrees)
        .is_some();
    if let Some(terminal) = typed_authority_terminal_v1(has_blocking_witness, certificate) {
        return AttemptResultV1::Terminal(terminal);
    }
    if certificate == PostApplyProofCertificateStateV1::ResourceUnavailable {
        return if stage + 1 >= POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.len() {
            AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit)
        } else {
            AttemptResultV1::Continue
        };
    }
    let Some(diagnostic) = diagnostic else {
        return AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit);
    };
    if diagnostic.sampled_pose_count() == 0
        || diagnostic.sampled_nonblocking_pose_count() != diagnostic.sampled_pose_count()
        || stage + 1 >= POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.len()
    {
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownEvidenceInsufficient)
    } else {
        AttemptResultV1::Continue
    }
}

fn typed_authority_terminal_v1(
    has_blocking_witness: bool,
    certificate: PostApplyProofCertificateStateV1,
) -> Option<PostApplyProofTerminalV1> {
    if has_blocking_witness {
        return Some(PostApplyProofTerminalV1::Blocked);
    }
    match certificate {
        PostApplyProofCertificateStateV1::BindingRejected => {
            Some(PostApplyProofTerminalV1::UnknownEvidenceInsufficient)
        }
        PostApplyProofCertificateStateV1::Certified => Some(PostApplyProofTerminalV1::Certified),
        PostApplyProofCertificateStateV1::Uncertified => None,
        PostApplyProofCertificateStateV1::ResourceUnavailable => None,
        PostApplyProofCertificateStateV1::Cancelled
        | PostApplyProofCertificateStateV1::DeadlineExceeded => None,
    }
}

fn terminal_after_attempt_v1(
    deadline_reached: bool,
    attempt: AttemptResultV1,
) -> Option<PostApplyProofTerminalV1> {
    if deadline_reached {
        Some(PostApplyProofTerminalV1::UnknownDeadlineReached)
    } else {
        match attempt {
            AttemptResultV1::Continue => None,
            AttemptResultV1::Terminal(terminal) => Some(terminal),
        }
    }
}

fn run_result_is_current_v1(
    state: &PostApplyProofJobStateV1,
    run_generation: u64,
    stage: usize,
) -> bool {
    matches!(
        state,
        PostApplyProofJobStateV1::InFlight {
            run_generation: actual_generation,
            stage: actual_stage,
            ..
        } if *actual_generation == run_generation && *actual_stage == stage
    )
}

fn job_matches_continuing_project_v1(job: &PostApplyProofJobV1, project: &ProjectState) -> bool {
    let retained_premise_matches = job.premise.as_ref().is_none_or(|premise| {
        premise_is_internally_bound_v1(premise) && job.binding == premise.binding
    });
    if !retained_premise_matches
        || job.binding.source_revision().checked_add(1) != Some(job.target_revision)
        || job.binding.project_instance_id() != project.instance_id
        || job.binding.project_id() != project.project_id
    {
        return false;
    }
    match &job.state {
        PostApplyProofJobStateV1::Ready { .. }
        | PostApplyProofJobStateV1::InFlight { .. }
        | PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Certified(_),
            ..
        } => matches!(
            project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding),
            Ok(None)
        ),
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Failure(terminal),
            ..
        } => {
            let Some(expected) = terminal_outcome_v1(*terminal) else {
                return false;
            };
            match project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding)
            {
                Ok(None) => true,
                Ok(Some(report)) => report.outcome == expected,
                Err(_) => false,
            }
        }
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::CertifiedRecovery,
            ..
        } => matches!(
            project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding),
            Ok(None) | Ok(Some(_)) | Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
        ),
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified) => matches!(
            project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding),
            Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
        ),
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Stale) => true,
        PostApplyProofJobStateV1::Terminal(terminal) => {
            let Some(expected) = terminal_outcome_v1(*terminal) else {
                return false;
            };
            matches!(
                project
                    .editor
                    .inspect_speculative_unproven_fold_v1(&job.binding),
                Ok(Some(report)) if report.outcome == expected
            )
        }
    }
}

fn close_noncontinuing_job_v1(project: &mut ProjectState, job: &mut PostApplyProofJobV1) {
    signal_inflight_cancellation_v1(job);
    let same_project = job.binding.project_instance_id() == project.instance_id
        && job.binding.project_id() == project.project_id;
    if same_project && certified_resolution_pending_v1(job) {
        let _ = resolve_locked_certified_terminal_v1(project, job);
        return;
    }
    if same_project
        && matches!(
            project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding),
            Ok(None)
        )
    {
        // Internal premise/state drift must not discard the only resolver for
        // an exact mark that is still awaiting proof.
        let _ = resolve_locked_terminal_v1(
            project,
            job,
            PostApplyProofTerminalV1::UnknownEvidenceInsufficient,
        );
    } else {
        // A replacement project or an already resolved/absent mark owns no
        // live authority that this job may complete.
        mark_stale_v1(job);
    }
}

fn refresh_terminal_report_v1(project: &ProjectState, job: &mut PostApplyProofJobV1) {
    let PostApplyProofJobStateV1::Terminal(terminal) = &job.state else {
        return;
    };
    let terminal = *terminal;
    if matches!(
        terminal,
        PostApplyProofTerminalV1::Certified | PostApplyProofTerminalV1::Stale
    ) {
        job.resolution_report = None;
        return;
    }
    let Some(expected) = terminal_outcome_v1(terminal) else {
        mark_stale_v1(job);
        return;
    };
    match project
        .editor
        .inspect_speculative_unproven_fold_v1(&job.binding)
    {
        Ok(Some(report)) if report.outcome == expected => {
            job.resolution_report = Some(report);
        }
        _ => mark_stale_v1(job),
    }
}

fn mark_stale_v1(job: &mut PostApplyProofJobV1) {
    signal_inflight_cancellation_v1(job);
    job.state = PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Stale);
    job.resource_recovery_cancelled_run_generation = None;
    job.premise = None;
    job.resolution_report = None;
    wake_deadline_scheduler_v1();
}

fn progress_v1(job: &PostApplyProofJobV1) -> PostApplyProofProgressV1 {
    let terminal = match &job.state {
        PostApplyProofJobStateV1::Terminal(terminal) => Some(*terminal),
        PostApplyProofJobStateV1::Ready { .. }
        | PostApplyProofJobStateV1::InFlight { .. }
        | PostApplyProofJobStateV1::Resolving { .. } => None,
    };
    let status = terminal.map_or("proving", terminal_status_v1);
    PostApplyProofProgressV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: job.binding.project_instance_id(),
        project_id: job.binding.project_id(),
        revision: job.target_revision,
        job_token: job.job_token,
        status,
        proven_pair_count: if terminal == Some(PostApplyProofTerminalV1::Certified) {
            job.total_pair_count
        } else {
            0
        },
        total_pair_count: job.total_pair_count,
        proof_failure: if terminal.is_some_and(terminal_is_failure_v1) {
            job.resolution_report.map(resolution_dto_v1)
        } else {
            None
        },
    }
}

fn terminal_is_failure_v1(terminal: PostApplyProofTerminalV1) -> bool {
    matches!(
        terminal,
        PostApplyProofTerminalV1::Blocked
            | PostApplyProofTerminalV1::UnknownEvidenceInsufficient
            | PostApplyProofTerminalV1::UnknownResourceLimit
            | PostApplyProofTerminalV1::UnknownCancelled
            | PostApplyProofTerminalV1::UnknownDeadlineReached
    )
}

fn terminal_status_v1(terminal: PostApplyProofTerminalV1) -> &'static str {
    match terminal {
        PostApplyProofTerminalV1::Certified => "certified",
        PostApplyProofTerminalV1::Blocked => "blocked",
        PostApplyProofTerminalV1::UnknownEvidenceInsufficient => "unknown_evidence_insufficient",
        PostApplyProofTerminalV1::UnknownResourceLimit => "unknown_resource_limit",
        PostApplyProofTerminalV1::UnknownCancelled => "unknown_cancelled",
        PostApplyProofTerminalV1::UnknownDeadlineReached => "unknown_deadline_reached",
        PostApplyProofTerminalV1::Stale => "stale",
    }
}

fn terminal_outcome_v1(
    terminal: PostApplyProofTerminalV1,
) -> Option<SpeculativeUnprovenFoldProofOutcomeV1> {
    match terminal {
        PostApplyProofTerminalV1::Certified => None,
        PostApplyProofTerminalV1::Blocked => Some(SpeculativeUnprovenFoldProofOutcomeV1::Blocked),
        PostApplyProofTerminalV1::UnknownEvidenceInsufficient => {
            Some(SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient,
            })
        }
        PostApplyProofTerminalV1::UnknownResourceLimit => {
            Some(SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
            })
        }
        PostApplyProofTerminalV1::UnknownCancelled => {
            Some(SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::Cancelled,
            })
        }
        PostApplyProofTerminalV1::UnknownDeadlineReached => {
            Some(SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached,
            })
        }
        PostApplyProofTerminalV1::Stale => None,
    }
}

fn validate_start_request_v1(request: &StartPostApplyProofJobRequestV1) -> Result<(), String> {
    if request.version != POST_APPLY_PROOF_PROTOCOL_VERSION_V1 {
        return Err(unavailable_message_v1());
    }
    Ok(())
}

fn validate_job_request_v1(request: &PostApplyProofJobRequestV1) -> Result<(), String> {
    if request.version != POST_APPLY_PROOF_PROTOCOL_VERSION_V1 {
        return Err(unavailable_message_v1());
    }
    Ok(())
}

fn find_job_index_v1(
    registry: &PostApplyProofRegistryV1,
    request: &PostApplyProofJobRequestV1,
) -> Option<usize> {
    registry.jobs.iter().position(|job| {
        job.job_token == request.job_token
            && job.binding.project_instance_id() == request.project_instance_id
            && job.binding.project_id() == request.project_id
            && job.target_revision == request.revision
    })
}
