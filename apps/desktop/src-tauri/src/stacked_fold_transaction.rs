use std::sync::{Mutex, MutexGuard};

use ori_collision::StackedFoldBoundedPathDiagnosticV1;
use ori_core::{
    AppliedPoseLimitsV1, PreparedStackedFoldRequestedPoseV1, StackedFoldNonFlatLayerOrderV1,
    prepare_applied_pose_v1,
};
use ori_domain::{
    InstructionHingeAngle, InstructionPose, InstructionPoseModel, InstructionStep,
    InstructionStepId, InstructionVisual, MIN_INSTRUCTION_DURATION_MS, ProjectId,
};
use ori_foldability::fold_model_fingerprint_v1;
use tauri::State;

use super::{
    AppState,
    applied_pose::{
        CurrentAppliedPoseCapability, lock_revalidated_current_applied_pose_for_commit,
    },
    global_flat_foldability::{
        CurrentLayerOrderCapability, GlobalFlatFoldabilityState,
        lock_revalidated_current_layer_order_for_commit,
    },
    lock_project,
};

#[derive(Default)]
pub(super) struct StackedFoldTransactionState(Mutex<StackedFoldTransactionSlot>);

#[derive(Default)]
struct StackedFoldTransactionSlot {
    active_generation: Option<ProjectId>,
    pending: Option<PendingStackedFoldTransaction>,
    last_cancelled: Option<ProjectId>,
    applied_layer_order: Option<StackedFoldNonFlatLayerOrderV1>,
}

/// Native-only preview premises. None of the proof-bearing values is
/// serialized or reduced to a caller-replayable boolean.
pub(super) struct PendingStackedFoldTransaction {
    token: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_source_fingerprint: [u8; 32],
    expected_pose_generation: u64,
    expected_layer_generation: u64,
    requested: PreparedStackedFoldRequestedPoseV1,
    continuous: StackedFoldBoundedPathDiagnosticV1,
    layer_order: StackedFoldNonFlatLayerOrderV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
}

pub(super) struct PendingStackedFoldPremises {
    pub expected_instance_id: ProjectId,
    pub expected_project_id: ProjectId,
    pub expected_revision: u64,
    pub expected_source_fingerprint: [u8; 32],
    pub expected_pose_generation: u64,
    pub expected_layer_generation: u64,
    pub requested: PreparedStackedFoldRequestedPoseV1,
    pub continuous: StackedFoldBoundedPathDiagnosticV1,
    pub layer_order: StackedFoldNonFlatLayerOrderV1,
}

impl PendingStackedFoldTransaction {
    #[must_use]
    pub(super) const fn token(&self) -> ProjectId {
        self.token
    }

    #[must_use]
    pub(super) fn matches_live_binding(
        &self,
        instance_id: ProjectId,
        project_id: ProjectId,
        revision: u64,
        source_fingerprint: [u8; 32],
        pose_generation: u64,
        layer_generation: u64,
    ) -> bool {
        binding_matches(
            (
                self.expected_instance_id,
                self.expected_project_id,
                self.expected_revision,
                self.expected_source_fingerprint,
                self.expected_pose_generation,
                self.expected_layer_generation,
            ),
            (
                instance_id,
                project_id,
                revision,
                source_fingerprint,
                pose_generation,
                layer_generation,
            ),
        ) && self.continuous.continuous_clearance_certified()
            && self.continuous.requested_angle_degrees().to_bits()
                == self.requested.requested_angle_degrees().to_bits()
            && self.layer_order.target_revision()
                == self
                    .requested
                    .initial()
                    .target()
                    .geometry()
                    .proof()
                    .lineage()
                    .target_revision()
    }

    #[must_use]
    pub(super) const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

type LiveBinding = (ProjectId, ProjectId, u64, [u8; 32], u64, u64);

fn binding_matches(expected: LiveBinding, actual: LiveBinding) -> bool {
    expected == actual
}

pub(super) fn install_pending_stacked_fold(
    state: &StackedFoldTransactionState,
    premises: PendingStackedFoldPremises,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
    let token = ProjectId::new();
    let mut slot = lock_slot(state)?;
    let pending = PendingStackedFoldTransaction {
        token,
        expected_instance_id: premises.expected_instance_id,
        expected_project_id: premises.expected_project_id,
        expected_revision: premises.expected_revision,
        expected_source_fingerprint: premises.expected_source_fingerprint,
        expected_pose_generation: premises.expected_pose_generation,
        expected_layer_generation: premises.expected_layer_generation,
        requested: premises.requested,
        continuous: premises.continuous,
        layer_order: premises.layer_order,
        pose_capability,
        layer_capability,
    };
    if !pending.matches_live_binding(
        premises.expected_instance_id,
        premises.expected_project_id,
        premises.expected_revision,
        premises.expected_source_fingerprint,
        premises.expected_pose_generation,
        premises.expected_layer_generation,
    ) || pending.authorizes_project_mutation()
    {
        return Err("The stacked-fold transaction premises are inconsistent.".to_owned());
    }
    slot.active_generation = Some(token);
    slot.pending = Some(pending);
    Ok(token)
}

pub(super) fn cancel_pending_stacked_fold(
    state: &StackedFoldTransactionState,
    token: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_slot(state)?;
    if slot.last_cancelled == Some(token) {
        return Ok(());
    }
    if slot.active_generation != Some(token)
        || slot
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token() != token)
    {
        return Err("The stacked-fold transaction preview is stale.".to_owned());
    }
    slot.pending = None;
    slot.active_generation = None;
    slot.last_cancelled = Some(token);
    Ok(())
}

#[tauri::command]
pub(super) fn cancel_stacked_fold_transaction_preview(
    state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
) -> Result<(), String> {
    cancel_pending_stacked_fold(&state, token)
}

#[tauri::command]
pub(super) fn apply_stacked_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
) -> Result<u64, String> {
    let mut transaction_slot = lock_slot(&transaction_state)?;
    let pending = transaction_slot
        .pending
        .as_ref()
        .filter(|pending| pending.token() == token)
        .ok_or_else(|| "The stacked-fold transaction preview is stale.".to_owned())?;
    let mut project =
        lock_project(&app_state).map_err(|_| "The project is unavailable.".to_owned())?;
    let fingerprint = fold_model_fingerprint_v1(project.editor.pattern(), project.editor.paper()).0;
    if !pending.matches_live_binding(
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        fingerprint,
        pending.expected_pose_generation,
        pending.expected_layer_generation,
    ) {
        return Err("The stacked-fold transaction preview is stale.".to_owned());
    }
    let _pose_guard =
        lock_revalidated_current_applied_pose_for_commit(&project, &pending.pose_capability)
            .map_err(|_| "The current pose authority is unavailable.".to_owned())?
            .ok_or_else(|| "The stacked-fold transaction preview is stale.".to_owned())?;
    let _layer_guard = lock_revalidated_current_layer_order_for_commit(
        &foldability_state,
        &project,
        &pending.layer_capability,
    )
    .map_err(|_| "The current layer-order authority is unavailable.".to_owned())?
    .ok_or_else(|| "The stacked-fold transaction preview is stale.".to_owned())?;

    let requested = &pending.requested;
    let target = requested.initial().target();
    let target_pose = requested.pose();
    let applied_pose = prepare_applied_pose_v1(
        target_pose.face_ids(),
        &target_pose
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>(),
        target_pose.fixed_face(),
        &target_pose
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<Vec<_>>(),
        AppliedPoseLimitsV1::default(),
    )
    .map_err(|_| "The target pose is inconsistent.".to_owned())?;
    let candidate = target.geometry().candidate();
    let mut timeline = project.editor.instruction_timeline().clone();
    timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "Stacked fold".to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: MIN_INSTRUCTION_DURATION_MS,
        visual: InstructionVisual::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: target
                .geometry()
                .proof()
                .lineage()
                .target_fingerprint()
                .to_hex(),
            fixed_face: target_pose.fixed_face(),
            hinge_angles: target_pose
                .hinge_angles()
                .iter()
                .map(|angle| InstructionHingeAngle {
                    edge: angle.edge(),
                    angle_degrees: angle.angle_degrees(),
                })
                .collect(),
        },
    });
    let layers = project.editor.project_layers().clone();
    let applied_layer_order = pending.layer_order.clone();
    let result = project
        .editor
        .execute_stacked_fold_document(
            pending.expected_revision,
            candidate.pattern.clone(),
            candidate.paper.clone(),
            timeline,
            layers,
            applied_pose,
        )
        .map_err(|_| "The stacked-fold transaction could not be applied atomically.".to_owned())?;
    drop(_layer_guard);
    drop(_pose_guard);
    drop(project);
    transaction_slot.pending = None;
    transaction_slot.active_generation = None;
    transaction_slot.applied_layer_order = Some(applied_layer_order);
    debug_assert!(
        transaction_slot
            .applied_layer_order
            .as_ref()
            .is_some_and(|order| order.target_revision() == result.revision)
    );
    Ok(result.revision)
}

fn lock_slot(
    state: &StackedFoldTransactionState,
) -> Result<MutexGuard<'_, StackedFoldTransactionSlot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "The stacked-fold transaction registry is unavailable.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_idempotent_and_replacement_is_aba_safe() {
        let state = StackedFoldTransactionState::default();
        let first = ProjectId::new();
        let second = ProjectId::new();
        {
            let mut slot = lock_slot(&state).unwrap();
            slot.active_generation = Some(first);
            slot.last_cancelled = None;
        }
        cancel_pending_stacked_fold(&state, first).unwrap();
        cancel_pending_stacked_fold(&state, first).unwrap();
        {
            let mut slot = lock_slot(&state).unwrap();
            slot.active_generation = Some(second);
            slot.last_cancelled = None;
        }
        assert!(cancel_pending_stacked_fold(&state, first).is_err());
        assert_eq!(lock_slot(&state).unwrap().active_generation, Some(second));
    }

    #[test]
    fn every_live_binding_dimension_is_revalidated() {
        let instance = ProjectId::new();
        let project = ProjectId::new();
        let expected = (instance, project, 7, [0x5a; 32], 11, 13);
        assert!(binding_matches(expected, expected));
        assert!(!binding_matches(
            expected,
            (ProjectId::new(), project, 7, [0x5a; 32], 11, 13)
        ));
        assert!(!binding_matches(
            expected,
            (instance, ProjectId::new(), 7, [0x5a; 32], 11, 13)
        ));
        assert!(!binding_matches(
            expected,
            (instance, project, 8, [0x5a; 32], 11, 13)
        ));
        assert!(!binding_matches(
            expected,
            (instance, project, 7, [0xa5; 32], 11, 13)
        ));
        assert!(!binding_matches(
            expected,
            (instance, project, 7, [0x5a; 32], 12, 13)
        ));
        assert!(!binding_matches(
            expected,
            (instance, project, 7, [0x5a; 32], 11, 14)
        ));
    }
}
