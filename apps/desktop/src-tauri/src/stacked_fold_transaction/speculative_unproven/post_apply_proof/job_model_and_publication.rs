#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartPostApplyProofJobRequestV1 {
    version: u8,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PostApplyProofJobRequestV1 {
    version: u8,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    job_token: ProjectId,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PostApplyProofProgressV1 {
    version: u8,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    job_token: ProjectId,
    status: &'static str,
    proven_pair_count: usize,
    total_pair_count: usize,
    proof_failure: Option<SpeculativeUnprovenFoldResolutionDtoV1>,
}

#[derive(Default)]
pub(in crate::stacked_fold_transaction) struct PostApplyProofRegistryV1 {
    jobs: VecDeque<PostApplyProofJobV1>,
    retained_bytes: usize,
    next_run_generation: u64,
    next_scheduler_generation: u64,
    deadline_scheduler_registered: bool,
}

pub(super) struct PostApplyProofPremiseV1 {
    pub(super) resolution_ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    pub(super) binding: SpeculativeUnprovenFoldBindingV1,
    pub(super) requested: PreparedStackedFoldRequestedPoseV1,
    pub(super) initial_layer_order: StackedFoldInitialLayerOrderV1,
    pub(super) target_revision: u64,
    pub(super) target_fingerprint: [u8; 32],
    pub(super) target_pose_generation: u64,
    pub(super) paper_thickness_mm: f64,
}

struct PostApplyProofJobV1 {
    job_token: ProjectId,
    scheduler_generation: u64,
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: u64,
    target_fingerprint: [u8; 32],
    target_pose_generation: u64,
    expected_face_ids: Vec<FaceId>,
    expected_hinge_ids: Vec<EdgeId>,
    expected_fixed_face: Option<FaceId>,
    expected_hinge_angles: Vec<(EdgeId, u64)>,
    total_pair_count: usize,
    retained_bytes: usize,
    retain_until: Instant,
    proof_deadline: Instant,
    frontend_started: bool,
    cumulative_work: usize,
    premise: Option<PostApplyProofPremiseV1>,
    resolution_report: Option<SpeculativeUnprovenFoldResolutionReportV1>,
    // Identifies a worker generation stopped only to release native work
    // during scheduler resource recovery. Its Cancelled result is
    // infrastructure-owned and must not impersonate an explicit user cancel.
    resource_recovery_cancelled_run_generation: Option<u64>,
    state: PostApplyProofJobStateV1,
}

struct DeadlineSchedulerRegistrationV1 {
    project: Weak<Mutex<ProjectState>>,
    registry: Weak<Mutex<PostApplyProofRegistryV1>>,
    resource_retry: Option<DeadlineSchedulerResourceRetryV1>,
    _lease: DeadlineSchedulerRegistrationLeaseV1,
}

#[derive(Clone, Copy)]
struct DeadlineSchedulerResourceRetryV1 {
    attempt: u32,
    first_failure_at: Instant,
    // Snapshot at the failed scheduler pass. A later publication shares the
    // registration but must not inherit the old pass's resource failure.
    through_scheduler_generation: u64,
    not_before: Instant,
}

enum DeadlineSchedulerCommandV1 {
    Register(DeadlineSchedulerRegistrationV1),
    Wake,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeadlineSchedulerRecoveryDispositionV1 {
    DropRegistration,
    RetainForTerminalExpiry,
    RetainForResourceRetry,
}

struct DeadlineSchedulerRegistrationLeaseV1;

impl DeadlineSchedulerRegistrationLeaseV1 {
    fn try_acquire_v1() -> Result<Self, ()> {
        ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_POST_APPLY_DEADLINE_REGISTRATIONS_V1)
                    .then(|| active.saturating_add(1))
            })
            .map(|_| Self)
            .map_err(|_| ())
    }
}

impl Drop for DeadlineSchedulerRegistrationLeaseV1 {
    fn drop(&mut self) {
        let _ = ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |active| active.checked_sub(1),
        );
    }
}

#[derive(Debug)]
enum PostApplyProofJobStateV1 {
    Ready {
        next_stage: usize,
    },
    InFlight {
        run_generation: u64,
        stage: usize,
        cancellation: Arc<AtomicBool>,
    },
    Resolving {
        run_generation: u64,
        resolution: PostApplyProofResolutionV1,
    },
    Terminal(PostApplyProofTerminalV1),
}

#[derive(Debug)]
enum PostApplyProofResolutionV1 {
    Certified(PostApplyProofCertifiedAuthorityV1),
    CertifiedRecovery,
    Failure(PostApplyProofTerminalV1),
}

#[derive(Debug)]
enum PostApplyProofCertifiedAuthorityV1 {
    Tree(SpeculativeUnprovenFoldCertifiedProofV1),
    LayeredThreeFace(SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1),
    LayeredFourFace(SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostApplyProofTerminalV1 {
    Certified,
    Blocked,
    UnknownEvidenceInsufficient,
    UnknownResourceLimit,
    UnknownCancelled,
    UnknownDeadlineReached,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttemptResultV1 {
    Continue,
    Terminal(PostApplyProofTerminalV1),
}

struct PostApplyProofWorkerAttemptV1 {
    diagnostic: Result<StackedFoldBoundedPathDiagnosticV1, ()>,
    certificate: PostApplyProofWorkerCertificateV1,
}

enum PostApplyProofWorkerCertificateV1 {
    Uncertified(PostApplyProofPremiseV1),
    Certified(PostApplyProofCertifiedAuthorityV1),
    BindingRejected(PostApplyProofPremiseV1),
    ResourceUnavailable(PostApplyProofPremiseV1),
    Cancelled(PostApplyProofPremiseV1),
    DeadlineExceeded(PostApplyProofPremiseV1),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostApplyProofCertificateStateV1 {
    Uncertified,
    Certified,
    BindingRejected,
    ResourceUnavailable,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostApplyProofDirectCertificateErrorV1 {
    BindingRejected,
    ResourceUnavailable,
    Cancelled,
    DeadlineExceeded,
}

impl PostApplyProofWorkerCertificateV1 {
    const fn state(&self) -> PostApplyProofCertificateStateV1 {
        match self {
            Self::Uncertified(_) => PostApplyProofCertificateStateV1::Uncertified,
            Self::Certified(_) => PostApplyProofCertificateStateV1::Certified,
            Self::BindingRejected(_) => PostApplyProofCertificateStateV1::BindingRejected,
            Self::ResourceUnavailable(_) => PostApplyProofCertificateStateV1::ResourceUnavailable,
            Self::Cancelled(_) => PostApplyProofCertificateStateV1::Cancelled,
            Self::DeadlineExceeded(_) => PostApplyProofCertificateStateV1::DeadlineExceeded,
        }
    }

    fn into_recoverable_premise(self) -> Option<PostApplyProofPremiseV1> {
        match self {
            Self::Uncertified(premise)
            | Self::BindingRejected(premise)
            | Self::ResourceUnavailable(premise)
            | Self::Cancelled(premise)
            | Self::DeadlineExceeded(premise) => Some(premise),
            Self::Certified(_) => None,
        }
    }
}

impl PostApplyProofResolutionV1 {
    const fn terminal(&self) -> PostApplyProofTerminalV1 {
        match self {
            Self::Certified(_) | Self::CertifiedRecovery => PostApplyProofTerminalV1::Certified,
            Self::Failure(terminal) => *terminal,
        }
    }
}

/// Retains the complete in-process path premise only after Apply committed.
///
/// The caller must immediately resolve the exact speculative history mark to
/// an explicit fail-closed `Unknown::ResourceLimit` outcome if publication
/// fails. Every ordinary failure returns the complete original premise, so
/// silently abandoning the one-shot resolution ticket is impossible.
// Boxing the failure would require a fresh allocation on the resource-failure
// path and could itself destroy the only recoverable one-shot premise.
#[allow(clippy::result_large_err)]
pub(super) fn publish_post_apply_proof_premise_v1(
    app_state: &AppState,
    state: &StackedFoldTransactionState,
    premise: PostApplyProofPremiseV1,
) -> Result<(), PostApplyProofPremiseV1> {
    #[cfg(test)]
    if take_post_apply_proof_publication_failure_for_test_v1() {
        return Err(premise);
    }
    if !premise_is_internally_bound_v1(&premise) {
        return Err(premise);
    }
    let Some(retained_bytes) = retained_premise_bytes_v1(&premise) else {
        return Err(premise);
    };
    if retained_bytes > MAX_POST_APPLY_PROOF_JOB_BYTES_V1 {
        return Err(premise);
    }
    let face_count = premise
        .requested
        .initial()
        .target()
        .model()
        .face_ids()
        .len();
    let Some(total_pair_count) = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|count| count / 2)
        .filter(|count| *count > 0)
    else {
        return Err(premise);
    };
    let now = Instant::now();
    let Some(retain_until) = now.checked_add(POST_APPLY_PROOF_START_RETENTION_V1) else {
        return Err(premise);
    };
    let Some(proof_deadline) = now.checked_add(next_post_apply_proof_deadline_v1()) else {
        return Err(premise);
    };
    let Ok(mut registry) = lock_registry_v1(state) else {
        return Err(premise);
    };
    let expected_face_ids = premise
        .requested
        .initial()
        .target()
        .model()
        .face_ids()
        .to_vec();
    let expected_hinge_ids = premise
        .requested
        .initial()
        .target()
        .model()
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect();
    let expected_fixed_face = premise.requested.pose().fixed_face();
    let expected_hinge_angles = premise
        .requested
        .pose()
        .hinge_angles()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect();

    // A reopened/replaced project has a different instance identity. Its
    // inaccessible premises cannot authorize work and are reclaimed first.
    reclaim_jobs_v1(&mut registry, |job| {
        job.binding.project_instance_id() != premise.binding.project_instance_id()
    });
    while registry.jobs.len() >= MAX_POST_APPLY_PROOF_JOBS_V1
        || registry
            .retained_bytes
            .checked_add(retained_bytes)
            .is_none_or(|bytes| bytes > MAX_POST_APPLY_PROOF_RETAINED_BYTES_V1)
    {
        let Some(index) = registry
            .jobs
            .iter()
            .position(|job| matches!(&job.state, PostApplyProofJobStateV1::Terminal(_)))
        else {
            return Err(premise);
        };
        remove_job_v1(&mut registry, index);
    }
    let Some(next_retained_bytes) = registry.retained_bytes.checked_add(retained_bytes) else {
        return Err(premise);
    };
    let Some(scheduler_generation) = registry.next_scheduler_generation.checked_add(1) else {
        return Err(premise);
    };
    // Reserve before registering the deadline owner. This keeps a successful
    // registration from observing an empty registry after a fallible queue
    // growth and preserves the complete premise for the caller on failure.
    if registry.jobs.try_reserve(1).is_err() {
        return Err(premise);
    }
    let register_scheduler = !registry.deadline_scheduler_registered;
    if register_scheduler
        && register_deadline_scheduler_v1(app_state.project_handle_v1(), Arc::clone(&state.3))
            .is_err()
    {
        return Err(premise);
    }
    if register_scheduler {
        registry.deadline_scheduler_registered = true;
    }
    registry.next_scheduler_generation = scheduler_generation;
    registry.retained_bytes = next_retained_bytes;
    let job_token = ProjectId::new();
    registry.jobs.push_back(PostApplyProofJobV1 {
        job_token,
        scheduler_generation,
        binding: premise.binding.clone(),
        target_revision: premise.target_revision,
        target_fingerprint: premise.target_fingerprint,
        target_pose_generation: premise.target_pose_generation,
        expected_face_ids,
        expected_hinge_ids,
        expected_fixed_face,
        expected_hinge_angles,
        total_pair_count,
        retained_bytes,
        retain_until,
        proof_deadline,
        frontend_started: false,
        cumulative_work: 0,
        premise: Some(premise),
        resolution_report: None,
        resource_recovery_cancelled_run_generation: None,
        state: PostApplyProofJobStateV1::Ready { next_stage: 0 },
    });
    drop(registry);
    // A previously registered dispatcher may currently be sleeping until a
    // terminal job's long retention deadline. Every newly published active
    // job must therefore force it to recompute the earlier proof deadline.
    wake_deadline_scheduler_v1();
    Ok(())
}
