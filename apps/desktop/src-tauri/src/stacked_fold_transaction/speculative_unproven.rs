#[cfg(test)]
use std::cell::Cell;
use std::sync::MutexGuard;

use ori_collision::{StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticLimitsV1};
use ori_core::{
    AppliedPoseLimitsV1, PreparedStackedFoldRequestedPoseV1, SpeculativeUnprovenFoldTokenV1,
    StackedFoldInitialLayerOrderV1, StackedFoldNonFlatLayerOrderV1,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1,
    issue_speculative_unproven_fold_token_v1, prepare_applied_pose_v1,
};
use ori_domain::{
    InstructionHingeAngle, InstructionPose, InstructionPoseModel, InstructionStep,
    InstructionStepId, InstructionVisual, MIN_INSTRUCTION_DURATION_MS, ProjectId,
};
use ori_foldability::fold_model_fingerprint_v1;
use tauri::State;

#[cfg(test)]
use super::reissue_target_pose_or_rollback_with_v1;
use super::{
    AppState, CurrentLayerEvidence, GlobalFlatFoldabilityState,
    StackedFoldProjectRollbackSnapshotV1, StackedFoldTransactionState, lock_project,
    reissue_target_pose_or_rollback,
};
use crate::{
    applied_pose::{
        CurrentAppliedPoseCapability, CurrentAppliedPoseTransactionRollbackV1,
        lock_revalidated_current_applied_pose_for_commit,
    },
    global_flat_foldability::{
        CurrentLayerOrderCapability, lock_revalidated_current_layer_order_for_commit,
    },
};

mod post_apply_proof;
mod resolution;
pub(super) use post_apply_proof::PostApplyProofRegistryV1;
#[allow(unused_imports)]
pub(crate) use post_apply_proof::{
    PostApplyProofJobRequestV1, PostApplyProofProgressV1, RevertPostApplyProofFailureRequestV1,
    StartPostApplyProofJobRequestV1, cancel_post_apply_proof_job_v1, poll_post_apply_proof_job_v1,
    revert_post_apply_proof_failure_v1, start_post_apply_proof_job_v1,
};
#[allow(unused_imports)]
pub(crate) use resolution::{
    SpeculativeUnprovenFoldResolutionDtoV1, resolve_speculative_unproven_fold_native_v1,
};

#[derive(Default)]
pub(super) struct SpeculativeStackedFoldTransactionSlotV1 {
    pub(super) active_generation: Option<ProjectId>,
    pending: Option<PendingSpeculativeStackedFoldTransactionV1>,
    last_cancelled: Option<ProjectId>,
}

pub(crate) struct PendingSpeculativeStackedFoldPremisesV1 {
    pub expected_instance_id: ProjectId,
    pub expected_project_id: ProjectId,
    pub expected_revision: u64,
    pub expected_source_fingerprint: [u8; 32],
    pub expected_pose_generation: u64,
    pub expected_layer_generation: u64,
    pub requested: PreparedStackedFoldRequestedPoseV1,
    pub continuous: StackedFoldBoundedPathDiagnosticV1,
    pub diagnostic_paper_thickness_bits: u64,
    pub paper_thickness_mm: f64,
    pub initial_layer_order: StackedFoldInitialLayerOrderV1,
    pub layer_order: StackedFoldNonFlatLayerOrderV1,
    pub endpoint_has_blocking_hold: bool,
    pub endpoint_penetrating_pair_count: usize,
    pub endpoint_indeterminate_pair_count: usize,
}

struct PendingSpeculativeStackedFoldTransactionV1 {
    request_generation_id: ProjectId,
    token: SpeculativeUnprovenFoldTokenV1,
    post_apply_binding: ori_core::SpeculativeUnprovenFoldBindingV1,
    requested: PreparedStackedFoldRequestedPoseV1,
    initial_layer_order: StackedFoldInitialLayerOrderV1,
    layer_order: StackedFoldNonFlatLayerOrderV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
    expected_layer_generation: u64,
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_TARGET_POSE_REISSUE_V1: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApplySpeculativeStackedFoldRequestV1 {
    pub transaction_token: ProjectId,
    pub explicit_confirmation: bool,
}

#[must_use]
pub(crate) fn speculative_tree_diagnostic_is_issuable_v1(
    diagnostic: &StackedFoldBoundedPathDiagnosticV1,
) -> bool {
    diagnostic.continuous_certificate_model_id().is_none()
        && !diagnostic.continuous_clearance_certified()
        && diagnostic.first_sampled_blocking_angle_degrees().is_none()
        && diagnostic.sampled_pose_count() > 0
        && diagnostic.sampled_nonblocking_pose_count() == diagnostic.sampled_pose_count()
}

pub(crate) fn install_pending_speculative_stacked_fold_v1(
    state: &StackedFoldTransactionState,
    premises: PendingSpeculativeStackedFoldPremisesV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
    if !speculative_tree_diagnostic_is_issuable_v1(&premises.continuous)
        || premises.diagnostic_paper_thickness_bits != premises.paper_thickness_mm.to_bits()
        || premises.endpoint_has_blocking_hold
        || premises.endpoint_penetrating_pair_count != 0
        || premises.endpoint_indeterminate_pair_count != 0
        || pose_capability.generation() != premises.expected_pose_generation
        || layer_capability.generation() != premises.expected_layer_generation
        || !diagnostic_revalidates_exactly_v1(&premises)
        || !target_binding_is_consistent_v1(&premises)
    {
        return Err("The speculative stacked-fold premises are inconsistent.".to_owned());
    }
    let request_generation_id = ProjectId::new();
    let post_apply_binding = ori_core::SpeculativeUnprovenFoldBindingV1::new(
        premises.expected_instance_id,
        premises.expected_project_id,
        premises.expected_revision,
        lowercase_sha256_v1(premises.expected_source_fingerprint),
        premises.expected_pose_generation,
        request_generation_id,
        premises.paper_thickness_mm,
        ori_core::SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .map_err(|_| "The speculative stacked-fold binding is invalid.".to_owned())?;
    let token = issue_speculative_unproven_fold_token_v1(
        premises.expected_instance_id,
        &premises.requested,
        &premises.initial_layer_order,
        premises.expected_pose_generation,
        request_generation_id,
        premises.paper_thickness_mm,
    )
    .map_err(|_| "The speculative stacked-fold token could not be issued.".to_owned())?;
    let pending = PendingSpeculativeStackedFoldTransactionV1 {
        request_generation_id,
        token,
        post_apply_binding,
        requested: premises.requested,
        initial_layer_order: premises.initial_layer_order,
        layer_order: premises.layer_order,
        pose_capability,
        layer_capability,
        expected_layer_generation: premises.expected_layer_generation,
    };
    let _mode_guard = super::lock_transaction_mode_gate_v1(state)?;
    super::clear_pending_certified_stacked_fold_v1(state)?;
    let mut slot = lock_speculative_slot_v1(state)?;
    slot.active_generation = Some(request_generation_id);
    slot.pending = Some(pending);
    Ok(request_generation_id)
}

#[tauri::command]
pub(crate) fn apply_speculative_stacked_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    request: ApplySpeculativeStackedFoldRequestV1,
) -> Result<u64, String> {
    apply_speculative_stacked_fold_transaction_inner_v1(
        &app_state,
        &foldability_state,
        &transaction_state,
        request,
    )
}

pub(crate) fn apply_speculative_stacked_fold_transaction_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    request: ApplySpeculativeStackedFoldRequestV1,
) -> Result<u64, String> {
    if !request.explicit_confirmation {
        return Err("Explicit confirmation is required for speculative Apply.".to_owned());
    }
    let _mode_guard = super::lock_transaction_mode_gate_v1(transaction_state)?;
    let pending = take_pending_v1(transaction_state, request.transaction_token)?;
    let mut project =
        lock_project(app_state).map_err(|_| "The project is unavailable.".to_owned())?;
    let fingerprint = fold_model_fingerprint_v1(project.editor.pattern(), project.editor.paper()).0;
    if pending.expected_layer_generation != pending.layer_capability.generation()
        || !pending.token.reauthenticates_v1(
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            fingerprint,
            pending.pose_capability.generation(),
            request.transaction_token,
            project.editor.paper().thickness_mm.to_bits(),
        )
    {
        return Err("The speculative stacked-fold preview is stale.".to_owned());
    }
    let pose_guard =
        lock_revalidated_current_applied_pose_for_commit(&project, &pending.pose_capability)
            .map_err(|_| "The current pose authority is unavailable.".to_owned())?
            .ok_or_else(|| "The speculative stacked-fold preview is stale.".to_owned())?;
    let layer_guard = lock_revalidated_current_layer_order_for_commit(
        foldability_state,
        &project,
        &pending.layer_capability,
    )
    .map_err(|_| "The current layer-order authority is unavailable.".to_owned())?
    .ok_or_else(|| "The speculative stacked-fold preview is stale.".to_owned())?;

    let target = pending.requested.initial().target().geometry();
    let target_fingerprint = target.proof().lineage().target_fingerprint().0;
    let candidate_pattern = target.candidate().pattern.clone();
    let candidate_paper = target.candidate().paper.clone();
    let pose = pending.requested.pose();
    let applied_pose = prepare_applied_pose_v1(
        pose.face_ids(),
        &pose
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>(),
        pose.fixed_face(),
        &pose
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<Vec<_>>(),
        AppliedPoseLimitsV1::default(),
    )
    .map_err(|_| "The speculative target pose is inconsistent.".to_owned())?;
    let persisted_pose = InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint: target.proof().lineage().target_fingerprint().to_hex(),
        fixed_face: pose.fixed_face(),
        hinge_angles: pose
            .hinge_angles()
            .iter()
            .map(|angle| InstructionHingeAngle {
                edge: angle.edge(),
                angle_degrees: angle.angle_degrees(),
            })
            .collect(),
    };
    let mut timeline = project.editor.instruction_timeline().clone();
    timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "Stacked fold (awaiting proof)".to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: MIN_INSTRUCTION_DURATION_MS,
        visual: InstructionVisual::default(),
        pose: persisted_pose.clone(),
    });
    validate_target_layer_order_v1(&project, &pending, &persisted_pose)?;
    let layers = project.editor.project_layers().clone();
    let project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let expected_revision = project.editor.revision();
    let result = project
        .editor
        .execute_stacked_fold_document_with_unproven_mark_v1(
            expected_revision,
            candidate_pattern,
            candidate_paper,
            timeline,
            layers,
            applied_pose,
            pending.token,
        )
        .map_err(|_| "The speculative stacked fold could not be applied atomically.".to_owned())?;
    project.record_numeric_expression_edit();
    drop(pose_guard);
    let pose_rollback = reissue_speculative_target_pose_or_rollback_v1(
        &mut project,
        &persisted_pose,
        &project_before,
    )?;
    layer_guard.invalidate_after_project_mutation();
    project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(pending.layer_order));
    pose_rollback.disarm();
    if let Ok(Some(target_pose_capability)) =
        project.applied_pose_authority.capture_capability(&project)
    {
        let _ = post_apply_proof::publish_post_apply_proof_premise_v1(
            transaction_state,
            post_apply_proof::PostApplyProofPremiseV1 {
                binding: pending.post_apply_binding,
                requested: pending.requested,
                initial_layer_order: pending.initial_layer_order,
                target_revision: result.revision,
                target_fingerprint,
                target_pose_generation: target_pose_capability.generation(),
                paper_thickness_mm: project.editor.paper().thickness_mm,
            },
        );
    }
    Ok(result.revision)
}

fn lowercase_sha256_v1(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn reissue_speculative_target_pose_or_rollback_v1(
    project: &mut super::super::ProjectState,
    persisted_pose: &InstructionPose,
    project_before: &StackedFoldProjectRollbackSnapshotV1,
) -> Result<CurrentAppliedPoseTransactionRollbackV1, String> {
    #[cfg(test)]
    if FAIL_NEXT_TARGET_POSE_REISSUE_V1.with(|flag| flag.replace(false)) {
        return reissue_target_pose_or_rollback_with_v1(
            project,
            persisted_pose,
            project_before,
            crate::applied_pose::restore_persisted_current_pose_failing_after_prepare_for_test_v1,
        );
    }
    reissue_target_pose_or_rollback(project, persisted_pose, project_before)
}

#[cfg(test)]
pub(crate) fn fail_next_speculative_target_pose_reissue_for_test_v1() {
    FAIL_NEXT_TARGET_POSE_REISSUE_V1.with(|flag| {
        assert!(!flag.replace(true), "a failure injection is already armed");
    });
}

fn diagnostic_revalidates_exactly_v1(premises: &PendingSpeculativeStackedFoldPremisesV1) -> bool {
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1(
        &premises.requested,
        premises.paper_thickness_mm,
        StackedFoldPathDiagnosticLimitsV1::default(),
        &premises.initial_layer_order,
    )
    .is_ok_and(|actual| actual == premises.continuous)
}

fn target_binding_is_consistent_v1(premises: &PendingSpeculativeStackedFoldPremisesV1) -> bool {
    let lineage = premises
        .requested
        .initial()
        .target()
        .geometry()
        .proof()
        .lineage();
    let Some(expected_target_revision) = premises.expected_revision.checked_add(1) else {
        return false;
    };
    lineage.identity_namespace() == premises.expected_project_id
        && lineage.source_revision() == premises.expected_revision
        && lineage.target_revision() == expected_target_revision
        && lineage.source_fingerprint().0 == premises.expected_source_fingerprint
        && premises.layer_order.identity_namespace() == premises.expected_project_id
        && premises.layer_order.target_revision() == lineage.target_revision()
        && premises.layer_order.target_fingerprint() == lineage.target_fingerprint()
}

fn validate_target_layer_order_v1(
    project: &super::super::ProjectState,
    pending: &PendingSpeculativeStackedFoldTransactionV1,
    persisted_pose: &InstructionPose,
) -> Result<(), String> {
    let target = pending.requested.initial().target().geometry();
    let target_fingerprint =
        fold_model_fingerprint_v1(&target.candidate().pattern, &target.candidate().paper);
    let proof = &pending.layer_order;
    let pose_matches = proof.hinge_angles().len() == persisted_pose.hinge_angles.len()
        && proof
            .hinge_angles()
            .iter()
            .zip(&persisted_pose.hinge_angles)
            .all(|(sealed, persisted)| {
                sealed.edge() == persisted.edge
                    && sealed.angle_degrees().to_bits() == persisted.angle_degrees.to_bits()
            });
    let expected_target_revision = project
        .editor
        .revision()
        .checked_add(1)
        .ok_or_else(|| "The speculative target revision cannot advance.".to_owned())?;
    if proof.identity_namespace() != project.project_id
        || proof.target_revision() != expected_target_revision
        || proof.target_fingerprint() != target_fingerprint
        || proof.fixed_face() != persisted_pose.fixed_face
        || !pose_matches
    {
        return Err("The speculative target layer authority is stale or tampered.".to_owned());
    }
    Ok(())
}

fn take_pending_v1(
    state: &StackedFoldTransactionState,
    request_generation_id: ProjectId,
) -> Result<PendingSpeculativeStackedFoldTransactionV1, String> {
    let mut slot = lock_speculative_slot_v1(state)?;
    if slot.active_generation != Some(request_generation_id)
        || slot
            .pending
            .as_ref()
            .is_none_or(|pending| pending.request_generation_id != request_generation_id)
    {
        return Err("The speculative stacked-fold preview is stale.".to_owned());
    }
    slot.active_generation = None;
    slot.pending
        .take()
        .ok_or_else(|| "The speculative stacked-fold preview is stale.".to_owned())
}

fn lock_speculative_slot_v1(
    state: &StackedFoldTransactionState,
) -> Result<MutexGuard<'_, SpeculativeStackedFoldTransactionSlotV1>, String> {
    state
        .1
        .lock()
        .map_err(|_| "The speculative stacked-fold registry is unavailable.".to_owned())
}

pub(super) fn clear_pending_speculative_stacked_fold_v1(
    state: &StackedFoldTransactionState,
) -> Result<(), String> {
    let mut slot = lock_speculative_slot_v1(state)?;
    slot.active_generation = None;
    slot.pending = None;
    Ok(())
}

pub(super) fn try_cancel_pending_speculative_stacked_fold_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
) -> Result<bool, String> {
    let mut slot = lock_speculative_slot_v1(state)?;
    if slot.last_cancelled == Some(token) {
        return Ok(true);
    }
    if slot.active_generation != Some(token)
        || slot
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request_generation_id != token)
    {
        return Ok(false);
    }
    slot.pending = None;
    slot.active_generation = None;
    slot.last_cancelled = Some(token);
    Ok(true)
}

#[cfg(test)]
mod tests;
