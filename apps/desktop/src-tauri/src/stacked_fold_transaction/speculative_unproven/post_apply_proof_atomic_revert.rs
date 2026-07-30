//! Explicit-confirmation, exact-suffix rollback for failed post-Apply proof jobs.

use ori_core::{
    MAX_EDITOR_HISTORY_ENTRIES, MAX_REVISION, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldHistoryLocationV1, SpeculativeUnprovenFoldProofOutcomeV1,
    SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldUnknownReasonV1,
};
use ori_domain::ProjectId;
use tauri::State;

use super::{
    POST_APPLY_PROOF_PROTOCOL_VERSION_V1, PostApplyProofJobStateV1, PostApplyProofRegistryV1,
    close_noncontinuing_job_v1, job_matches_continuing_project_v1, lock_registry_v1,
    refresh_terminal_report_v1, terminal_is_failure_v1,
};
use crate::{
    AppState, ProjectNumericExpressions, ProjectState, StackedFoldTransactionState,
    global_flat_foldability::{
        GlobalFlatFoldabilityState, lock_current_layer_order_for_history_mutation,
    },
    lock_project,
    proof_cache_edit_impact::commit_editor_pose_and_proof_invalidation_v1,
};

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RevertProofLocationV1 {
    AppliedTrimmedBase,
    AppliedRetainedUndo,
    UnappliedRedo,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RevertProofOutcomeV1 {
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RevertProofReasonV1 {
    EvidenceInsufficient,
    ResourceLimit,
    Cancelled,
    DeadlineReached,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RevertPostApplyProofFailureRequestV1 {
    pub(super) version: u8,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) job_token: ProjectId,
    pub(super) expected_location: RevertProofLocationV1,
    pub(super) expected_outcome: RevertProofOutcomeV1,
    pub(super) expected_reason: Option<RevertProofReasonV1>,
    pub(super) expected_subsequent_edit_count: u64,
    pub(super) expected_undo_steps_to_revert: Option<u32>,
    pub(super) explicit_confirmation: bool,
}

#[tauri::command]
pub(crate) async fn revert_post_apply_proof_failure_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    request: RevertPostApplyProofFailureRequestV1,
) -> Result<u64, String> {
    revert_post_apply_proof_failure_inner_v1(
        &app_state,
        &foldability_state,
        &transaction_state,
        request,
    )
    .await
}

pub(super) async fn revert_post_apply_proof_failure_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    request: RevertPostApplyProofFailureRequestV1,
) -> Result<u64, String> {
    revert_post_apply_proof_failure_with_interleave_v1(
        app_state,
        foldability_state,
        transaction_state,
        request,
        || {},
    )
    .await
}

#[cfg(test)]
pub(super) async fn revert_post_apply_proof_failure_with_interleave_for_test_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    request: RevertPostApplyProofFailureRequestV1,
    after_capture: impl FnOnce(),
) -> Result<u64, String> {
    revert_post_apply_proof_failure_with_interleave_v1(
        app_state,
        foldability_state,
        transaction_state,
        request,
        after_capture,
    )
    .await
}

async fn revert_post_apply_proof_failure_with_interleave_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    request: RevertPostApplyProofFailureRequestV1,
    after_capture: impl FnOnce(),
) -> Result<u64, String> {
    let expected_report = validate_revert_request_v1(&request)?;
    let (binding, editor, numeric_expressions, history_settings) = {
        let mut project = lock_project(app_state).map_err(|_| revert_unavailable_message_v1())?;
        ensure_revert_project_authority_v1(&project, &request)?;
        let mut registry =
            lock_registry_v1(transaction_state).map_err(|_| revert_unavailable_message_v1())?;
        let Some(index) = find_revert_job_index_v1(&registry, &request) else {
            return Err(revert_unavailable_message_v1());
        };
        let job = &mut registry.jobs[index];
        if !job_matches_continuing_project_v1(job, &project) {
            close_noncontinuing_job_v1(&mut project, job);
            return Err(revert_unavailable_message_v1());
        }
        refresh_terminal_report_v1(&project, job);
        if job.resolution_report != Some(expected_report)
            || !matches!(
                &job.state,
                PostApplyProofJobStateV1::Terminal(terminal)
                    if terminal_is_failure_v1(*terminal)
            )
        {
            return Err(revert_unavailable_message_v1());
        }
        (
            job.binding.clone(),
            project.editor.clone(),
            project.numeric_expressions.clone(),
            AtomicRevertHistorySettingsSealV1::capture(&project)
                .ok_or_else(revert_unavailable_message_v1)?,
        )
    };
    after_capture();

    let prepared = tauri::async_runtime::spawn_blocking(move || {
        prepare_atomic_revert_v1(editor, numeric_expressions, &binding, expected_report)
    })
    .await
    .map_err(|_| revert_unavailable_message_v1())?
    .map_err(|_| revert_unavailable_message_v1())?;

    let mut project = lock_project(app_state).map_err(|_| revert_unavailable_message_v1())?;
    ensure_revert_project_authority_v1(&project, &request)?;
    if !history_settings.matches(&project) {
        return Err(revert_unavailable_message_v1());
    }
    let mut registry =
        lock_registry_v1(transaction_state).map_err(|_| revert_unavailable_message_v1())?;
    let Some(index) = find_revert_job_index_v1(&registry, &request) else {
        return Err(revert_unavailable_message_v1());
    };
    let job = &mut registry.jobs[index];
    if !job_matches_continuing_project_v1(job, &project) {
        close_noncontinuing_job_v1(&mut project, job);
        return Err(revert_unavailable_message_v1());
    }
    refresh_terminal_report_v1(&project, job);
    if job.resolution_report != Some(expected_report) {
        return Err(revert_unavailable_message_v1());
    }

    let before_pattern = project.editor.pattern().clone();
    let before_paper = project.editor.paper().clone();
    let authority = project.applied_pose_authority.clone();
    let pose_invalidation = authority
        .begin_invalidation()
        .map_err(|_| revert_unavailable_message_v1())?;
    let mut layer_invalidation = lock_current_layer_order_for_history_mutation(foldability_state)
        .map_err(|_| revert_unavailable_message_v1())?;
    let source_revision = project.editor.revision();
    project.editor = prepared.editor;
    project.numeric_expressions = prepared.numeric_expressions;
    project.current_layer_evidence = None;
    commit_editor_pose_and_proof_invalidation_v1(
        pose_invalidation,
        source_revision,
        &before_pattern,
        &before_paper,
        &project,
    );
    layer_invalidation.invalidate_after_project_mutation();
    job.resolution_report = Some(prepared.report);
    Ok(project.editor.revision())
}

/// Seals the one live user setting that can change both cloned rollback inputs
/// without advancing the document revision.
///
/// Current-pose adoption is also revision-neutral, but history mutation
/// deliberately invalidates that runtime authority at commit. The remaining
/// revision-neutral `EditorState` restore APIs are used only while constructing
/// a replacement project, whose fresh instance identity is checked separately.
#[derive(Clone, Copy, PartialEq, Eq)]
struct AtomicRevertHistorySettingsSealV1 {
    history_entry_limit: usize,
    numeric_undo_entries: usize,
    numeric_redo_entries: usize,
    numeric_vertex_undo_entries: usize,
    numeric_vertex_redo_entries: usize,
}

impl AtomicRevertHistorySettingsSealV1 {
    fn capture(project: &ProjectState) -> Option<Self> {
        let history_entry_limit = project.editor.history_entry_limit();
        let seal = Self {
            history_entry_limit,
            numeric_undo_entries: project.numeric_expressions.undo_stack.len(),
            numeric_redo_entries: project.numeric_expressions.redo_stack.len(),
            numeric_vertex_undo_entries: project.numeric_expressions.vertex_undo_stack.len(),
            numeric_vertex_redo_entries: project.numeric_expressions.vertex_redo_stack.len(),
        };
        (seal.numeric_undo_entries == seal.numeric_vertex_undo_entries
            && seal.numeric_redo_entries == seal.numeric_vertex_redo_entries
            && seal.numeric_undo_entries <= history_entry_limit
            && seal.numeric_redo_entries <= history_entry_limit)
            .then_some(seal)
    }

    fn matches(self, project: &ProjectState) -> bool {
        Self::capture(project) == Some(self)
    }
}

struct PreparedAtomicRevertV1 {
    editor: ori_core::EditorState,
    numeric_expressions: ProjectNumericExpressions,
    report: SpeculativeUnprovenFoldResolutionReportV1,
}

fn prepare_atomic_revert_v1(
    mut editor: ori_core::EditorState,
    mut numeric_expressions: ProjectNumericExpressions,
    binding: &SpeculativeUnprovenFoldBindingV1,
    expected_report: SpeculativeUnprovenFoldResolutionReportV1,
) -> Result<PreparedAtomicRevertV1, ()> {
    let undo_steps = expected_report.undo_steps_to_revert.ok_or(())?;
    let undo_steps = usize::try_from(undo_steps).map_err(|_| ())?;
    if undo_steps == 0 || undo_steps > MAX_EDITOR_HISTORY_ENTRIES {
        return Err(());
    }
    let expected_target_revision = editor
        .revision()
        .checked_add(u64::try_from(undo_steps).map_err(|_| ())?)
        .filter(|revision| *revision <= MAX_REVISION)
        .ok_or(())?;
    for _ in 0..undo_steps {
        let revision = editor.revision();
        editor.undo(revision).map_err(|_| ())?;
        undo_numeric_expression_state_v1(&mut numeric_expressions)?;
    }
    if editor.revision() != expected_target_revision {
        return Err(());
    }
    let report = editor
        .inspect_speculative_unproven_fold_v1(binding)
        .map_err(|_| ())?
        .ok_or(())?;
    if report.location != SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo
        || report.outcome != expected_report.outcome
        || report.subsequent_edit_count != 0
        || report.undo_steps_to_revert.is_some()
    {
        return Err(());
    }
    Ok(PreparedAtomicRevertV1 {
        editor,
        numeric_expressions,
        report,
    })
}

fn undo_numeric_expression_state_v1(expressions: &mut ProjectNumericExpressions) -> Result<(), ()> {
    let previous = expressions.undo_stack.pop().ok_or(())?;
    let vertex_transition = expressions.vertex_undo_stack.pop().ok_or(())?;
    expressions
        .redo_stack
        .push(expressions.rectangular_paper_creation.take());
    expressions.rectangular_paper_creation = previous;
    if let Some(transition) = vertex_transition {
        for change in &transition.changes {
            crate::apply_vertex_expression_binding(
                &mut expressions.vertex_coordinates,
                change.vertex,
                change.before.clone(),
            );
        }
        expressions.vertex_redo_stack.push(Some(transition));
    } else {
        expressions.vertex_redo_stack.push(None);
    }
    Ok(())
}

pub(super) fn validate_revert_request_v1(
    request: &RevertPostApplyProofFailureRequestV1,
) -> Result<SpeculativeUnprovenFoldResolutionReportV1, String> {
    if request.version != POST_APPLY_PROOF_PROTOCOL_VERSION_V1
        || !request.explicit_confirmation
        || request.project_instance_id.canonical_bytes() == [0; 16]
        || request.project_id.canonical_bytes() == [0; 16]
        || request.job_token.canonical_bytes() == [0; 16]
        || request.expected_revision > MAX_REVISION
        || request.expected_subsequent_edit_count
            > u64::try_from(MAX_EDITOR_HISTORY_ENTRIES).expect("history limit fits u64")
    {
        return Err(revert_unavailable_message_v1());
    }

    let location = match request.expected_location {
        RevertProofLocationV1::AppliedTrimmedBase => {
            SpeculativeUnprovenFoldHistoryLocationV1::AppliedTrimmedBase
        }
        RevertProofLocationV1::AppliedRetainedUndo => {
            SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo
        }
        RevertProofLocationV1::UnappliedRedo => {
            SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo
        }
    };
    let outcome = match (request.expected_outcome, request.expected_reason) {
        (RevertProofOutcomeV1::Blocked, None) => SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        (RevertProofOutcomeV1::Unknown, Some(reason)) => {
            let reason = match reason {
                RevertProofReasonV1::EvidenceInsufficient => {
                    SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient
                }
                RevertProofReasonV1::ResourceLimit => {
                    SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit
                }
                RevertProofReasonV1::Cancelled => SpeculativeUnprovenFoldUnknownReasonV1::Cancelled,
                RevertProofReasonV1::DeadlineReached => {
                    SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached
                }
            };
            SpeculativeUnprovenFoldProofOutcomeV1::Unknown { reason }
        }
        _ => return Err(revert_unavailable_message_v1()),
    };

    let Some(undo_steps_to_revert) = request.expected_undo_steps_to_revert else {
        return Err(revert_unavailable_message_v1());
    };
    let expected_undo_steps = request
        .expected_subsequent_edit_count
        .checked_add(1)
        .and_then(|steps| u32::try_from(steps).ok())
        .filter(|steps| {
            usize::try_from(*steps).is_ok_and(|steps| steps <= MAX_EDITOR_HISTORY_ENTRIES)
        })
        .ok_or_else(revert_unavailable_message_v1)?;
    if location != SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo
        || undo_steps_to_revert != expected_undo_steps
    {
        return Err(revert_unavailable_message_v1());
    }

    Ok(SpeculativeUnprovenFoldResolutionReportV1 {
        location,
        outcome,
        subsequent_edit_count: request.expected_subsequent_edit_count,
        undo_steps_to_revert: Some(undo_steps_to_revert),
    })
}

fn ensure_revert_project_authority_v1(
    project: &ProjectState,
    request: &RevertPostApplyProofFailureRequestV1,
) -> Result<(), String> {
    if project.instance_id != request.project_instance_id
        || project.project_id != request.project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(revert_unavailable_message_v1());
    }
    Ok(())
}

fn find_revert_job_index_v1(
    registry: &PostApplyProofRegistryV1,
    request: &RevertPostApplyProofFailureRequestV1,
) -> Option<usize> {
    registry.jobs.iter().position(|job| {
        job.job_token == request.job_token
            && job.binding.project_instance_id() == request.project_instance_id
            && job.binding.project_id() == request.project_id
    })
}

pub(super) fn revert_unavailable_message_v1() -> String {
    "The failed post-Apply proof can no longer be reverted.".to_owned()
}
