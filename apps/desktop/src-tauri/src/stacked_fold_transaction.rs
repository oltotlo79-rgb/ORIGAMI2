use std::sync::{Mutex, MutexGuard};

use ori_collision::StackedFoldBoundedPathDiagnosticV1;
use ori_core::{PreparedStackedFoldRequestedPoseV1, StackedFoldNonFlatLayerOrderV1};
use ori_domain::ProjectId;
use tauri::State;

#[derive(Default)]
pub(super) struct StackedFoldTransactionState(Mutex<StackedFoldTransactionSlot>);

#[derive(Default)]
struct StackedFoldTransactionSlot {
    active_generation: Option<ProjectId>,
    pending: Option<PendingStackedFoldTransaction>,
    last_cancelled: Option<ProjectId>,
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
