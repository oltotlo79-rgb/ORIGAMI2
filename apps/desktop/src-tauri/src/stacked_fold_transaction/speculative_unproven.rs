#[cfg(test)]
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::MutexGuard,
};

use ori_collision::{StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticLimitsV1};
use ori_core::{
    PreparedStackedFoldRequestedPoseV1, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldProofOutcomeV1, SpeculativeUnprovenFoldUnknownReasonV1,
    StackedFoldInitialLayerOrderV1, StackedFoldNonFlatLayerOrderV1,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1,
};
use ori_domain::{InstructionHingeAngle, InstructionPose, InstructionPoseModel, ProjectId};
use ori_foldability::fold_model_fingerprint_v1;
use tauri::State;

#[cfg(test)]
use super::reissue_target_pose_or_rollback_with_v1;
use super::{
    AppState, CurrentLayerEvidence, GlobalFlatFoldabilityState,
    StackedFoldProjectRollbackSnapshotV1, StackedFoldTransactionState, lock_project,
    reissue_target_pose_or_rollback, rollback_stacked_fold_apply_v1,
};
use crate::{
    applied_pose::{
        CurrentAppliedPoseCapability, CurrentAppliedPoseTransactionRollbackV1,
        lock_revalidated_current_applied_pose_for_commit,
    },
    global_flat_foldability::{
        CurrentLayerOrderCapability, CurrentLayerOrderCommitGuard,
        lock_revalidated_current_layer_order_for_commit,
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
    post_apply_binding: ori_core::SpeculativeUnprovenFoldBindingV1,
    requested: PreparedStackedFoldRequestedPoseV1,
    initial_layer_order: StackedFoldInitialLayerOrderV1,
    layer_order: StackedFoldNonFlatLayerOrderV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
    expected_layer_generation: u64,
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct PostApplyPublicationResolutionFaultV1 {
    token: u64,
    fault: u8,
}

#[cfg(test)]
#[must_use = "the target-pose fault remains armed only while this guard is held"]
pub(crate) struct ArmedTargetPoseReissueFailureGuardV1 {
    token: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for ArmedTargetPoseReissueFailureGuardV1 {
    fn drop(&mut self) {
        FAIL_NEXT_TARGET_POSE_REISSUE_V1.with(|slot| {
            if slot.get() == Some(self.token) {
                slot.set(None);
            }
        });
    }
}

#[cfg(test)]
thread_local! {
    static NEXT_TARGET_POSE_REISSUE_FAILURE_TOKEN_V1: Cell<u64> = const { Cell::new(0) };
    static FAIL_NEXT_TARGET_POSE_REISSUE_V1: Cell<Option<u64>> = const { Cell::new(None) };
    static NEXT_POST_APPLY_PUBLICATION_RESOLUTION_FAULT_TOKEN_V1: Cell<u64> =
        const { Cell::new(0) };
    static NEXT_POST_APPLY_PUBLICATION_RESOLUTION_FAULT_V1:
        Cell<Option<PostApplyPublicationResolutionFaultV1>> = const { Cell::new(None) };
}

#[cfg(test)]
#[must_use = "the publication-resolution fault remains armed only while this guard is held"]
pub(crate) struct ArmedPostApplyPublicationResolutionFaultGuardV1 {
    token: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for ArmedPostApplyPublicationResolutionFaultGuardV1 {
    fn drop(&mut self) {
        NEXT_POST_APPLY_PUBLICATION_RESOLUTION_FAULT_V1.with(|slot| {
            if slot.get().is_some_and(|armed| armed.token == self.token) {
                slot.set(None);
            }
        });
    }
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

#[cfg(test)]
pub(crate) fn install_pending_speculative_stacked_fold_v1(
    state: &StackedFoldTransactionState,
    premises: PendingSpeculativeStackedFoldPremisesV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
    let request_generation_id = ProjectId::new();
    install_pending_speculative_stacked_fold_with_token_v1(
        state,
        request_generation_id,
        premises,
        pose_capability,
        layer_capability,
    )
}

pub(crate) fn install_pending_speculative_stacked_fold_with_token_v1(
    state: &StackedFoldTransactionState,
    request_generation_id: ProjectId,
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
    let pending = PendingSpeculativeStackedFoldTransactionV1 {
        request_generation_id,
        post_apply_binding,
        requested: premises.requested,
        initial_layer_order: premises.initial_layer_order,
        layer_order: premises.layer_order,
        pose_capability,
        layer_capability,
        expected_layer_generation: premises.expected_layer_generation,
    };
    super::with_try_locked_transaction_install_slots_v1(
        state,
        |certified_slot, speculative_slot| {
            super::clear_pending_certified_slot_locked_v1(certified_slot);
            speculative_slot.active_generation = Some(request_generation_id);
            speculative_slot.pending = Some(pending);
            request_generation_id
        },
    )
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
        || pending.post_apply_binding.project_instance_id() != project.instance_id
        || pending.post_apply_binding.project_id() != project.project_id
        || pending.post_apply_binding.source_revision() != project.editor.revision()
        || pending
            .post_apply_binding
            .source_geometry_fingerprint_sha256()
            != lowercase_sha256_v1(fingerprint)
        || pending.post_apply_binding.pose_generation() != pending.pose_capability.generation()
        || pending.post_apply_binding.request_generation_id() != request.transaction_token
        || pending.post_apply_binding.paper_thickness_bits()
            != project.editor.paper().thickness_mm.to_bits()
    {
        return Err("The speculative stacked-fold preview is stale.".to_owned());
    }
    let pose_guard =
        lock_revalidated_current_applied_pose_for_commit(&project, &pending.pose_capability)
            .map_err(|_| "The current pose authority is unavailable.".to_owned())?
            .ok_or_else(|| "The speculative stacked-fold preview is stale.".to_owned())?;
    let mut layer_guard = lock_revalidated_current_layer_order_for_commit(
        foldability_state,
        &project,
        &pending.layer_capability,
    )
    .map_err(|_| "The current layer-order authority is unavailable.".to_owned())?
    .ok_or_else(|| "The speculative stacked-fold preview is stale.".to_owned())?;

    let target = pending.requested.initial().target().geometry();
    let target_fingerprint = target.proof().lineage().target_fingerprint().0;
    let pose = pending.requested.pose();
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
    validate_target_layer_order_v1(&project, &pending, &persisted_pose)?;
    let token = project
        .editor
        .issue_speculative_unproven_fold_token_v1(
            project.instance_id,
            &pending.requested,
            &pending.initial_layer_order,
            pending.pose_capability.generation(),
            request.transaction_token,
            project.editor.paper().thickness_mm,
        )
        .map_err(|_| "The speculative stacked-fold token could not be issued.".to_owned())?;
    let mut project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let layer_before = layer_guard.capture_rollback_snapshot_v1();
    let (result, resolution_ticket) = project
        .editor
        .execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1(token)
        .map_err(|_| "The speculative stacked fold could not be applied atomically.".to_owned())?;
    project.record_numeric_expression_edit();
    drop(pose_guard);
    let mut pose_rollback = reissue_speculative_target_pose_or_rollback_v1(
        &mut project,
        &persisted_pose,
        &mut project_before,
    )?;
    let target_pose_capability = match project.applied_pose_authority.capture_capability(&project) {
        Ok(Some(capability)) => capability,
        _ => {
            rollback_failed_post_apply_publication_v1(
                &mut project,
                &mut project_before,
                &mut pose_rollback,
                &mut layer_guard,
                &layer_before,
            )?;
            return Err(
                "The post-Apply proof premise could not capture its target pose authority."
                    .to_owned(),
            );
        }
    };
    let premise = post_apply_proof::PostApplyProofPremiseV1 {
        resolution_ticket,
        binding: pending.post_apply_binding,
        requested: pending.requested,
        initial_layer_order: pending.initial_layer_order,
        target_revision: result.revision,
        target_fingerprint,
        target_pose_generation: target_pose_capability.generation(),
        paper_thickness_mm: project.editor.paper().thickness_mm,
    };
    if let Err(premise) =
        post_apply_proof::publish_post_apply_proof_premise_v1(app_state, transaction_state, premise)
    {
        if let Err(error) =
            resolve_post_apply_publication_failure_v1(&mut project, &premise.binding)
        {
            rollback_failed_post_apply_publication_v1(
                &mut project,
                &mut project_before,
                &mut pose_rollback,
                &mut layer_guard,
                &layer_before,
            )?;
            return Err(error);
        }
        // The exact mark is now observably fail-closed, so its one-shot
        // positive-proof ticket is no longer live authority.
        drop(premise);
    }
    layer_guard.invalidate_after_project_mutation();
    project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(pending.layer_order));
    pose_rollback.disarm();
    Ok(result.revision)
}

fn rollback_failed_post_apply_publication_v1(
    project: &mut super::super::ProjectState,
    project_before: &mut StackedFoldProjectRollbackSnapshotV1,
    pose_rollback: &mut CurrentAppliedPoseTransactionRollbackV1,
    layer_guard: &mut CurrentLayerOrderCommitGuard<'_>,
    layer_before: &crate::global_flat_foldability::CurrentLayerOrderRollbackSnapshotV1,
) -> Result<(), String> {
    rollback_stacked_fold_apply_v1(
        project,
        project_before,
        pose_rollback,
        Some(layer_guard),
        Some(layer_before),
    )
}

fn resolve_post_apply_publication_failure_v1(
    project: &mut super::super::ProjectState,
    binding: &SpeculativeUnprovenFoldBindingV1,
) -> Result<(), String> {
    let expected = SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
        reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
    };
    let fault = take_post_apply_publication_resolution_fault_v1();
    let attempted = catch_unwind(AssertUnwindSafe(|| {
        if fault == 2 {
            panic!("injected pre-resolution post-Apply publication fallback panic");
        }
        if fault == 1 {
            return Err(());
        }
        let report = project
            .editor
            .resolve_speculative_unproven_fold_v1(binding, expected)
            .map_err(|_| ())?;
        if fault == 3 {
            panic!("injected post-resolution post-Apply publication fallback panic");
        }
        Ok(report)
    }));
    if matches!(attempted, Ok(Ok(report)) if report.outcome == expected) {
        return Ok(());
    }
    // An Err or unwind may occur after the editor committed the explicit
    // outcome. Observe the exact binding before deciding that Apply must be
    // rolled back; the one-shot premise remains owned by the caller meanwhile.
    if matches!(
        catch_unwind(AssertUnwindSafe(|| {
            project
                .editor
                .inspect_speculative_unproven_fold_v1(binding)
        })),
        Ok(Ok(Some(report))) if report.outcome == expected
    ) {
        return Ok(());
    }
    Err("The post-Apply proof premise could not be retained or resolved fail-closed.".to_owned())
}

fn take_post_apply_publication_resolution_fault_v1() -> u8 {
    #[cfg(test)]
    {
        NEXT_POST_APPLY_PUBLICATION_RESOLUTION_FAULT_V1
            .with(Cell::take)
            .map_or(0, |armed| armed.fault)
    }
    #[cfg(not(test))]
    0
}

#[cfg(test)]
pub(crate) fn fail_next_post_apply_publication_resolution_for_test_v1()
-> ArmedPostApplyPublicationResolutionFaultGuardV1 {
    arm_post_apply_publication_resolution_fault_v1(1)
}

#[cfg(test)]
pub(crate) fn panic_next_post_apply_publication_resolution_before_for_test_v1()
-> ArmedPostApplyPublicationResolutionFaultGuardV1 {
    arm_post_apply_publication_resolution_fault_v1(2)
}

#[cfg(test)]
pub(crate) fn panic_next_post_apply_publication_resolution_after_for_test_v1()
-> ArmedPostApplyPublicationResolutionFaultGuardV1 {
    arm_post_apply_publication_resolution_fault_v1(3)
}

#[cfg(test)]
fn arm_post_apply_publication_resolution_fault_v1(
    fault: u8,
) -> ArmedPostApplyPublicationResolutionFaultGuardV1 {
    assert!((1..=3).contains(&fault), "known resolution fault");
    let token = NEXT_POST_APPLY_PUBLICATION_RESOLUTION_FAULT_TOKEN_V1.with(|next| {
        let token = next
            .get()
            .checked_add(1)
            .expect("publication resolution fault token overflow");
        next.set(token);
        token
    });
    NEXT_POST_APPLY_PUBLICATION_RESOLUTION_FAULT_V1.with(|slot| {
        assert!(
            slot.get().is_none(),
            "one post-Apply publication resolution fault may be armed"
        );
        slot.set(Some(PostApplyPublicationResolutionFaultV1 { token, fault }));
    });
    ArmedPostApplyPublicationResolutionFaultGuardV1 {
        token,
        _not_send_or_sync: PhantomData,
    }
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
    project_before: &mut StackedFoldProjectRollbackSnapshotV1,
) -> Result<CurrentAppliedPoseTransactionRollbackV1, String> {
    #[cfg(test)]
    if take_target_pose_reissue_failure_for_test_v1() {
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
pub(crate) fn fail_next_speculative_target_pose_reissue_for_test_v1()
-> ArmedTargetPoseReissueFailureGuardV1 {
    let token = NEXT_TARGET_POSE_REISSUE_FAILURE_TOKEN_V1.with(|next| {
        let token = next
            .get()
            .checked_add(1)
            .expect("target pose reissue failure token overflow");
        next.set(token);
        token
    });
    FAIL_NEXT_TARGET_POSE_REISSUE_V1.with(|slot| {
        assert!(slot.get().is_none(), "a failure injection is already armed");
        slot.set(Some(token));
    });
    ArmedTargetPoseReissueFailureGuardV1 {
        token,
        _not_send_or_sync: PhantomData,
    }
}

#[cfg(test)]
fn take_target_pose_reissue_failure_for_test_v1() -> bool {
    FAIL_NEXT_TARGET_POSE_REISSUE_V1.with(Cell::take).is_some()
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

#[cfg(test)]
pub(super) fn clear_pending_speculative_stacked_fold_v1(
    state: &StackedFoldTransactionState,
) -> Result<(), String> {
    let mut slot = lock_speculative_slot_v1(state)?;
    clear_pending_speculative_slot_locked_v1(&mut slot);
    Ok(())
}

pub(super) fn clear_pending_speculative_slot_locked_v1(
    slot: &mut SpeculativeStackedFoldTransactionSlotV1,
) {
    slot.active_generation = None;
    slot.pending = None;
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
