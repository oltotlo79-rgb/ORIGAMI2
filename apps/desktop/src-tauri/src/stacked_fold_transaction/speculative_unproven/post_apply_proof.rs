use std::{
    collections::VecDeque,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use ori_collision::{
    STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
    STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
    StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticLimitsV1,
};
use ori_core::{
    PreparedStackedFoldRequestedPoseV1, SpeculativeApproximateBlockingObservationV1,
    SpeculativeUnprovenFoldBindingV1, SpeculativeUnprovenFoldProofOutcomeV1,
    SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldUnknownReasonV1,
    StackedFoldInitialLayerOrderV1,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1,
};
use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_foldability::fold_model_fingerprint_v1;
use tauri::State;

use super::super::super::StackedFoldTransactionState;
use super::resolution::{SpeculativeUnprovenFoldResolutionDtoV1, resolution_dto_v1};
use crate::{AppState, ProjectState, lock_project};

#[path = "post_apply_proof_atomic_revert.rs"]
mod atomic_revert;
pub(crate) use atomic_revert::{
    RevertPostApplyProofFailureRequestV1, revert_post_apply_proof_failure_v1,
};

const POST_APPLY_PROOF_PROTOCOL_VERSION_V1: u8 = 1;
const POST_APPLY_PROOF_SAMPLE_INTERVALS_V1: [usize; 3] = [16, 32, 64];
const POST_APPLY_PROOF_TOTAL_WORK_V1: usize = 16 + 32 + 64;
const MAX_POST_APPLY_PROOF_JOBS_V1: usize = 8;
const MAX_POST_APPLY_PROOF_RETAINED_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_POST_APPLY_PROOF_JOB_BYTES_V1: usize = 2 * 1024 * 1024;
const POST_APPLY_PROOF_START_RETENTION_V1: Duration = Duration::from_secs(5 * 60);
const POST_APPLY_PROOF_DEADLINE_V1: Duration = Duration::from_secs(30);

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

pub(in crate::stacked_fold_transaction) struct PostApplyProofRegistryV1 {
    jobs: VecDeque<PostApplyProofJobV1>,
    retained_bytes: usize,
    next_run_generation: u64,
}

impl Default for PostApplyProofRegistryV1 {
    fn default() -> Self {
        Self {
            jobs: VecDeque::new(),
            retained_bytes: 0,
            next_run_generation: 0,
        }
    }
}

pub(super) struct PostApplyProofPremiseV1 {
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
    proof_deadline: Option<Instant>,
    cumulative_work: usize,
    premise: Option<PostApplyProofPremiseV1>,
    resolution_report: Option<SpeculativeUnprovenFoldResolutionReportV1>,
    state: PostApplyProofJobStateV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostApplyProofJobStateV1 {
    Ready {
        next_stage: usize,
    },
    InFlight {
        run_generation: u64,
        stage: usize,
    },
    Resolving {
        run_generation: u64,
        terminal: PostApplyProofTerminalV1,
    },
    Terminal(PostApplyProofTerminalV1),
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

struct LivePostApplyProofBindingV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    target_fingerprint: [u8; 32],
    paper_thickness_bits: u64,
    pose_generation: u64,
    face_ids: Vec<FaceId>,
    hinge_ids: Vec<EdgeId>,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<(EdgeId, u64)>,
}

enum AttemptResultV1 {
    Continue,
    Terminal(PostApplyProofTerminalV1),
}

/// Retains the complete in-process path premise only after Apply committed.
///
/// Publication failure deliberately does not roll the edit back. The
/// speculative history mark remains unresolved and therefore fail-closed.
pub(super) fn publish_post_apply_proof_premise_v1(
    state: &StackedFoldTransactionState,
    premise: PostApplyProofPremiseV1,
) -> Result<(), ()> {
    if !premise_is_internally_bound_v1(&premise) {
        return Err(());
    }
    let retained_bytes = retained_premise_bytes_v1(&premise).ok_or(())?;
    if retained_bytes > MAX_POST_APPLY_PROOF_JOB_BYTES_V1 {
        return Err(());
    }
    let face_count = premise
        .requested
        .initial()
        .target()
        .model()
        .face_ids()
        .len();
    let total_pair_count = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|count| count / 2)
        .filter(|count| *count > 0)
        .ok_or(())?;
    let now = Instant::now();
    let retain_until = now
        .checked_add(POST_APPLY_PROOF_START_RETENTION_V1)
        .ok_or(())?;
    let mut registry = lock_registry_v1(state)?;
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
            .position(|job| matches!(job.state, PostApplyProofJobStateV1::Terminal(_)))
        else {
            return Err(());
        };
        remove_job_v1(&mut registry, index);
    }
    registry.retained_bytes = registry
        .retained_bytes
        .checked_add(retained_bytes)
        .ok_or(())?;
    registry.jobs.push_back(PostApplyProofJobV1 {
        job_token: ProjectId::new(),
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
        proof_deadline: None,
        cumulative_work: 0,
        premise: Some(premise),
        resolution_report: None,
        state: PostApplyProofJobStateV1::Ready { next_stage: 0 },
    });
    Ok(())
}

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
    let live = capture_live_binding_v1(&project)?;
    let now = Instant::now();
    let mut registry = lock_registry_v1(transaction_state).map_err(|_| unavailable_message_v1())?;
    let Some(job) = registry.jobs.iter_mut().find(|job| {
        job.binding.project_instance_id() == request.project_instance_id
            && job.binding.project_id() == request.project_id
            && job.target_revision == request.revision
    }) else {
        return Err(unavailable_message_v1());
    };
    if !job_matches_start_live_v1(job, &live) {
        job.state = PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Stale);
        job.premise = None;
        return Ok(progress_v1(job));
    }
    if now >= job.retain_until {
        resolve_locked_terminal_v1(
            &mut project,
            job,
            PostApplyProofTerminalV1::UnknownDeadlineReached,
        );
        return Ok(progress_v1(job));
    }
    if job.proof_deadline.is_none() {
        job.proof_deadline = now.checked_add(POST_APPLY_PROOF_DEADLINE_V1);
        if job.proof_deadline.is_none() {
            resolve_locked_terminal_v1(
                &mut project,
                job,
                PostApplyProofTerminalV1::UnknownResourceLimit,
            );
        }
    }
    Ok(progress_v1(job))
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
            mark_stale_v1(job);
            return Ok(progress_v1(job));
        }
        if matches!(job.state, PostApplyProofJobStateV1::Terminal(_)) {
            refresh_terminal_report_v1(&project, job);
            return Ok(progress_v1(job));
        }
        if deadline_reached_v1(job, now) {
            resolve_locked_terminal_v1(
                &mut project,
                job,
                PostApplyProofTerminalV1::UnknownDeadlineReached,
            );
            return Ok(progress_v1(job));
        }
        match job.state {
            PostApplyProofJobStateV1::InFlight { .. } => {
                return Ok(progress_v1(job));
            }
            PostApplyProofJobStateV1::Resolving { terminal, .. } => {
                resolve_locked_terminal_v1(&mut project, job, terminal);
                return Ok(progress_v1(job));
            }
            PostApplyProofJobStateV1::Ready { next_stage } => {
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
                job.state = PostApplyProofJobStateV1::InFlight {
                    run_generation,
                    stage: next_stage,
                };
                Some((worker_permit, run_generation, next_stage, premise))
            }
            PostApplyProofJobStateV1::Terminal(_) => unreachable!("handled above"),
        }
    };

    let Some((worker_permit, run_generation, stage, premise)) = work else {
        return Err(unavailable_message_v1());
    };
    let interval_count = POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[stage];
    let registry = Arc::clone(&transaction_state.3);
    let worker_request = request.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let diagnostic = catch_unwind(AssertUnwindSafe(|| {
            run_attempt_v1(&premise, interval_count)
        }))
        .unwrap_or(Err(()));
        complete_worker_attempt_v1(
            &registry,
            &worker_request,
            run_generation,
            stage,
            premise,
            diagnostic,
        );
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
        mark_stale_v1(job);
        return Ok(());
    }
    if matches!(job.state, PostApplyProofJobStateV1::Terminal(_)) {
        refresh_terminal_report_v1(&project, job);
        return Ok(());
    }
    resolve_locked_terminal_v1(
        &mut project,
        job,
        PostApplyProofTerminalV1::UnknownCancelled,
    );
    Ok(())
}

fn complete_worker_attempt_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
    request: &PostApplyProofJobRequestV1,
    run_generation: u64,
    stage: usize,
    premise: PostApplyProofPremiseV1,
    diagnostic: Result<StackedFoldBoundedPathDiagnosticV1, ()>,
) {
    let attempt = classify_attempt_v1(stage, diagnostic.as_ref().ok());
    let Ok(mut registry) = registry.lock() else {
        return;
    };
    let Some(index) = find_job_index_v1(&registry, request) else {
        return;
    };
    let job = &mut registry.jobs[index];
    if !run_result_is_current_v1(job.state, run_generation, stage) {
        return;
    }
    job.premise = Some(premise);
    let Some(cumulative_work) = job
        .cumulative_work
        .checked_add(POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[stage])
        .filter(|work| *work <= POST_APPLY_PROOF_TOTAL_WORK_V1)
    else {
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation,
            terminal: PostApplyProofTerminalV1::UnknownResourceLimit,
        };
        return;
    };
    job.cumulative_work = cumulative_work;
    if let Some(terminal) =
        terminal_after_attempt_v1(deadline_reached_v1(job, Instant::now()), attempt)
    {
        job.state = PostApplyProofJobStateV1::Resolving {
            run_generation,
            terminal,
        };
    } else {
        job.state = PostApplyProofJobStateV1::Ready {
            next_stage: stage + 1,
        };
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
        mark_stale_v1(job);
        return Ok(progress_v1(job));
    }
    if matches!(job.state, PostApplyProofJobStateV1::Terminal(_)) {
        refresh_terminal_report_v1(&project, job);
        return Ok(progress_v1(job));
    }
    if join_failed
        && matches!(
            job.state,
            PostApplyProofJobStateV1::InFlight { run_generation, .. }
                if run_generation == expected_run_generation
        )
    {
        resolve_locked_terminal_v1(
            &mut project,
            job,
            PostApplyProofTerminalV1::UnknownResourceLimit,
        );
        return Ok(progress_v1(job));
    }
    let requested_terminal = match job.state {
        PostApplyProofJobStateV1::Resolving {
            run_generation,
            terminal,
        } if run_generation == expected_run_generation => Some(terminal),
        PostApplyProofJobStateV1::Ready { .. } | PostApplyProofJobStateV1::InFlight { .. } => None,
        PostApplyProofJobStateV1::Resolving { .. } | PostApplyProofJobStateV1::Terminal(_) => {
            return Ok(progress_v1(job));
        }
    };
    if deadline_reached_v1(job, now) {
        resolve_locked_terminal_v1(
            &mut project,
            job,
            PostApplyProofTerminalV1::UnknownDeadlineReached,
        );
    } else if let Some(terminal) = requested_terminal {
        resolve_locked_terminal_v1(&mut project, job, terminal);
    }
    Ok(progress_v1(job))
}

fn resolve_locked_terminal_v1(
    project: &mut ProjectState,
    job: &mut PostApplyProofJobV1,
    terminal: PostApplyProofTerminalV1,
) {
    let Some(outcome) = terminal_outcome_v1(terminal) else {
        mark_stale_v1(job);
        return;
    };
    if let Ok(report) = project
        .editor
        .resolve_speculative_unproven_fold_v1(&job.binding, outcome)
    {
        job.state = PostApplyProofJobStateV1::Terminal(terminal);
        job.resolution_report = Some(report);
    } else {
        mark_stale_v1(job);
    }
    job.premise = None;
}

fn run_attempt_v1(
    premise: &PostApplyProofPremiseV1,
    sample_intervals: usize,
) -> Result<StackedFoldBoundedPathDiagnosticV1, ()> {
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1(
        &premise.requested,
        premise.paper_thickness_mm,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals,
            static_collision: Default::default(),
        },
        &premise.initial_layer_order,
    )
    .map_err(|_| ())
}

fn classify_attempt_v1(
    stage: usize,
    diagnostic: Option<&StackedFoldBoundedPathDiagnosticV1>,
) -> AttemptResultV1 {
    let Some(diagnostic) = diagnostic else {
        return AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownResourceLimit);
    };
    if diagnostic.first_sampled_blocking_angle_degrees().is_some() {
        return AttemptResultV1::Terminal(PostApplyProofTerminalV1::Blocked);
    }
    if diagnostic.continuous_clearance_certified()
        && diagnostic
            .continuous_certificate_model_id()
            .is_some_and(trusted_continuous_certificate_model_v1)
        && diagnostic.sampled_pose_count() > 0
        && diagnostic.sampled_nonblocking_pose_count() == diagnostic.sampled_pose_count()
    {
        return AttemptResultV1::Terminal(PostApplyProofTerminalV1::Certified);
    }
    if diagnostic.sampled_pose_count() == 0
        || diagnostic.sampled_nonblocking_pose_count() != diagnostic.sampled_pose_count()
        || stage + 1 >= POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.len()
    {
        AttemptResultV1::Terminal(PostApplyProofTerminalV1::UnknownEvidenceInsufficient)
    } else {
        AttemptResultV1::Continue
    }
}

fn trusted_continuous_certificate_model_v1(model_id: &str) -> bool {
    matches!(
        model_id,
        STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            | STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2
            | STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            | STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2
            | STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            | STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
    )
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
    state: PostApplyProofJobStateV1,
    run_generation: u64,
    stage: usize,
) -> bool {
    state
        == (PostApplyProofJobStateV1::InFlight {
            run_generation,
            stage,
        })
}

fn capture_live_binding_v1(project: &ProjectState) -> Result<LivePostApplyProofBindingV1, String> {
    let capability = project
        .applied_pose_authority
        .capture_capability(project)
        .map_err(|_| unavailable_message_v1())?
        .ok_or_else(unavailable_message_v1)?;
    let (model, pose) = capability.tree().ok_or_else(unavailable_message_v1)?;
    Ok(LivePostApplyProofBindingV1 {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_fingerprint: fold_model_fingerprint_v1(
            project.editor.pattern(),
            project.editor.paper(),
        )
        .0,
        paper_thickness_bits: project.editor.paper().thickness_mm.to_bits(),
        pose_generation: capability.generation(),
        face_ids: model.face_ids().to_vec(),
        hinge_ids: model.hinges().iter().map(|hinge| hinge.edge()).collect(),
        fixed_face: pose.fixed_face(),
        hinge_angles: pose
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
            .collect(),
    })
}

fn premise_is_internally_bound_v1(premise: &PostApplyProofPremiseV1) -> bool {
    let initial = premise.requested.initial();
    let lineage = initial.target().geometry().proof().lineage();
    let target_pose = premise.requested.pose();
    let expected_target_generation = premise.binding.pose_generation().checked_add(1);
    lineage.identity_namespace() == premise.binding.project_id()
        && lineage.source_revision() == premise.binding.source_revision()
        && lineage.target_revision() == premise.target_revision
        && lineage.source_fingerprint().to_hex()
            == premise.binding.source_geometry_fingerprint_sha256()
        && lineage.target_fingerprint().0 == premise.target_fingerprint
        && expected_target_generation == Some(premise.target_pose_generation)
        && premise.binding.paper_thickness_bits() == premise.paper_thickness_mm.to_bits()
        && matches!(
            premise.binding.approximate_blocking_observation(),
            SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
        )
        && initial.target().model().owns_pose(initial.pose())
        && initial.target().model().owns_pose(target_pose)
}

fn job_matches_start_live_v1(
    job: &PostApplyProofJobV1,
    live: &LivePostApplyProofBindingV1,
) -> bool {
    let retained_premise_matches = job.premise.as_ref().is_none_or(|premise| {
        premise_is_internally_bound_v1(premise) && job.binding == premise.binding
    });
    retained_premise_matches
        && job.binding.source_revision().checked_add(1) == Some(job.target_revision)
        && job.binding.project_instance_id() == live.project_instance_id
        && job.binding.project_id() == live.project_id
        && job.target_revision == live.revision
        && job.target_fingerprint == live.target_fingerprint
        && job.binding.paper_thickness_bits() == live.paper_thickness_bits
        && job.target_pose_generation == live.pose_generation
        && job.expected_face_ids == live.face_ids
        && job.expected_hinge_ids == live.hinge_ids
        && job.expected_fixed_face == live.fixed_face
        && job.expected_hinge_angles == live.hinge_angles
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
    match job.state {
        PostApplyProofJobStateV1::Ready { .. }
        | PostApplyProofJobStateV1::InFlight { .. }
        | PostApplyProofJobStateV1::Resolving { .. } => matches!(
            project
                .editor
                .inspect_speculative_unproven_fold_v1(&job.binding),
            Ok(None)
        ),
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified) => project
            .editor
            .inspect_speculative_unproven_fold_v1(&job.binding)
            .is_err(),
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Stale) => true,
        PostApplyProofJobStateV1::Terminal(terminal) => {
            let Some(expected) = terminal_outcome_v1(terminal) else {
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

fn refresh_terminal_report_v1(project: &ProjectState, job: &mut PostApplyProofJobV1) {
    let PostApplyProofJobStateV1::Terminal(terminal) = job.state else {
        return;
    };
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
    job.state = PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Stale);
    job.premise = None;
    job.resolution_report = None;
}

fn retained_premise_bytes_v1(premise: &PostApplyProofPremiseV1) -> Option<usize> {
    let initial = premise.requested.initial();
    let target = initial.target();
    let candidate = target.geometry().candidate();
    let model = target.model();
    let face_boundary_vertices = model.face_ids().iter().try_fold(0_usize, |sum, face| {
        sum.checked_add(model.face_boundary(*face)?.vertices().len())
    })?;
    let mut bytes = size_of::<PostApplyProofJobV1>()
        .checked_add(size_of::<PostApplyProofPremiseV1>())?
        .checked_add(
            premise
                .initial_layer_order
                .retained_bytes_upper_bound_v1()?,
        )?
        .checked_add(
            candidate
                .pattern
                .vertices
                .len()
                .checked_mul(size_of::<ori_domain::Vertex>())?,
        )?
        .checked_add(
            candidate
                .pattern
                .edges
                .len()
                .checked_mul(size_of::<ori_domain::Edge>())?,
        )?
        .checked_add(
            candidate
                .paper
                .boundary_vertices
                .len()
                .checked_mul(size_of::<ori_domain::VertexId>())?,
        )?
        .checked_add(face_boundary_vertices.checked_mul(64)?)?
        .checked_add(model.face_ids().len().checked_mul(256)?)?
        .checked_add(model.hinges().len().checked_mul(512)?)?
        .checked_add(
            initial
                .pose()
                .hinge_angles()
                .len()
                .checked_add(premise.requested.pose().hinge_angles().len())?
                .checked_mul(size_of::<ori_kinematics::HingeAngle>())?,
        )?;
    // Account for both retained binding strings plus allocator slack.
    bytes = bytes.checked_add(
        premise
            .binding
            .source_geometry_fingerprint_sha256()
            .len()
            .checked_mul(2)?,
    )?;
    bytes.checked_mul(2)
}

fn progress_v1(job: &PostApplyProofJobV1) -> PostApplyProofProgressV1 {
    let terminal = match job.state {
        PostApplyProofJobStateV1::Terminal(terminal) => Some(terminal),
        PostApplyProofJobStateV1::Resolving { terminal, .. } => Some(terminal),
        PostApplyProofJobStateV1::Ready { .. } | PostApplyProofJobStateV1::InFlight { .. } => None,
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
        PostApplyProofTerminalV1::Certified => {
            Some(SpeculativeUnprovenFoldProofOutcomeV1::Certified)
        }
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

fn deadline_reached_v1(job: &PostApplyProofJobV1, now: Instant) -> bool {
    now >= job.retain_until || job.proof_deadline.is_some_and(|deadline| now >= deadline)
}

fn lock_registry_v1(
    state: &StackedFoldTransactionState,
) -> Result<MutexGuard<'_, PostApplyProofRegistryV1>, ()> {
    state.3.lock().map_err(|_| ())
}

fn reclaim_jobs_v1(
    registry: &mut PostApplyProofRegistryV1,
    mut reclaim: impl FnMut(&PostApplyProofJobV1) -> bool,
) {
    let mut index = 0;
    while index < registry.jobs.len() {
        if reclaim(&registry.jobs[index]) {
            remove_job_v1(registry, index);
        } else {
            index += 1;
        }
    }
}

fn remove_job_v1(registry: &mut PostApplyProofRegistryV1, index: usize) {
    if let Some(job) = registry.jobs.remove(index) {
        registry.retained_bytes = registry.retained_bytes.saturating_sub(job.retained_bytes);
    }
}

fn unavailable_message_v1() -> String {
    "The post-Apply proof job is unavailable.".to_owned()
}

#[cfg(test)]
#[path = "post_apply_proof_tests.rs"]
mod tests;
