use std::sync::{Mutex, MutexGuard};

use ori_collision::StackedFoldBoundedPathDiagnosticV1;
use ori_core::{
    AppliedPoseLimitsV1, PreparedStackedFoldGeometryV1, PreparedStackedFoldRequestedGraphPoseV1,
    PreparedStackedFoldRequestedPoseV1, StackedFoldNonFlatLayerOrderV1, prepare_applied_pose_v1,
};
use ori_domain::{
    InstructionHingeAngle, InstructionPose, InstructionPoseModel, InstructionStep,
    InstructionStepId, InstructionVisual, MIN_INSTRUCTION_DURATION_MS, ProjectId,
};
use ori_foldability::LayerOrderSnapshot;
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
    applied_layer_order: Option<PendingStackedFoldLayerProof>,
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
    requested: PendingStackedFoldRequestedPose,
    layer_order: PendingStackedFoldLayerProof,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
}

pub(super) enum PendingStackedFoldRequestedPose {
    Tree {
        requested: PreparedStackedFoldRequestedPoseV1,
        continuous: StackedFoldBoundedPathDiagnosticV1,
    },
    Graph {
        requested: PreparedStackedFoldRequestedGraphPoseV1,
        continuous: ori_collision::StackedFoldCyclePathDiagnosticV1,
        interval_closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
        certified_path: Option<ori_collision::CertifiedPoseGraphPathCertificateV1>,
        certified_edges: Vec<PendingCertifiedPathEdgeV1>,
    },
}

pub(super) struct PendingCertifiedPathEdgeV1 {
    pub generated: ori_kinematics::GeneratedMultiHingePathCandidateV1,
    pub closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub expected: ori_collision::CertifiedPathTransitionEvidenceV1,
    pub target_angles: Vec<(ori_domain::EdgeId, f64)>,
}

impl PendingStackedFoldRequestedPose {
    fn geometry(&self) -> &PreparedStackedFoldGeometryV1 {
        match self {
            Self::Tree { requested, .. } => requested.initial().target().geometry(),
            Self::Graph { requested, .. } => requested.initial().target().geometry(),
        }
    }
    fn continuous_certified(&self) -> bool {
        match self {
            Self::Tree { continuous, .. } => continuous.continuous_clearance_certified(),
            Self::Graph {
                continuous,
                interval_closure,
                certified_path,
                certified_edges,
                requested,
                ..
            } => {
                if continuous.continuous_certificate_model_id().is_none()
                    || interval_closure.leaves().is_empty()
                {
                    return false;
                }
                let Some(path) = certified_path else {
                    return true;
                };
                path.edges().len() == certified_edges.len()
                    && !path.authorizes_project_mutation()
                    && path
                        .edges()
                        .iter()
                        .zip(certified_edges)
                        .all(|(expected, edge)| {
                            if !certified_edge_target_matches_schedule(edge) {
                                return false;
                            }
                            ori_collision::certify_scheduled_cycle_transition_v1(
                                requested.initial().target().hinge_geometry(),
                                requested.initial().target().audit(),
                                requested.pose().fixed_face(),
                                &edge.generated,
                                &edge.closure,
                                8,
                                expected.source(),
                                expected.target(),
                            )
                            .is_some_and(|actual| actual == edge.expected && actual == *expected)
                        })
            }
        }
    }
    fn pose_components(
        &self,
    ) -> (
        Vec<ori_domain::FaceId>,
        Vec<ori_domain::EdgeId>,
        Option<ori_domain::FaceId>,
        Vec<(ori_domain::EdgeId, f64)>,
    ) {
        match self {
            Self::Tree { requested, .. } => {
                let pose = requested.pose();
                (
                    pose.face_ids().to_vec(),
                    pose.hinges().iter().map(|hinge| hinge.edge()).collect(),
                    pose.fixed_face(),
                    pose.hinge_angles()
                        .iter()
                        .map(|angle| (angle.edge(), angle.angle_degrees()))
                        .collect(),
                )
            }
            Self::Graph { requested, .. } => {
                let pose = requested.pose();
                (
                    requested
                        .initial()
                        .target()
                        .hinge_geometry()
                        .face_ids()
                        .to_vec(),
                    requested
                        .initial()
                        .target()
                        .hinge_geometry()
                        .hinges()
                        .iter()
                        .map(|hinge| hinge.edge())
                        .collect(),
                    Some(pose.fixed_face()),
                    pose.hinge_angles()
                        .as_slice()
                        .iter()
                        .map(|angle| (angle.edge(), angle.angle_degrees()))
                        .collect(),
                )
            }
        }
    }

    fn ordered_timeline_angles(&self) -> Vec<Vec<(ori_domain::EdgeId, f64)>> {
        match self {
            Self::Graph {
                certified_path: Some(_),
                certified_edges,
                ..
            } => certified_edges
                .iter()
                .map(|edge| edge.target_angles.clone())
                .collect(),
            _ => vec![self.pose_components().3],
        }
    }
}

fn bit_exact_canonical_angles_match(
    expected: &ori_kinematics::CanonicalHingeAngles,
    actual: &[(ori_domain::EdgeId, f64)],
) -> bool {
    expected.as_slice().len() == actual.len()
        && expected
            .as_slice()
            .iter()
            .zip(actual)
            .all(|(expected, actual)| {
                expected.edge() == actual.0
                    && expected.angle_degrees().to_bits() == actual.1.to_bits()
            })
}

fn certified_edge_target_matches_schedule(edge: &PendingCertifiedPathEdgeV1) -> bool {
    edge.generated
        .schedule()
        .evaluate(1.0)
        .is_some_and(|expected| bit_exact_canonical_angles_match(&expected, &edge.target_angles))
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

pub(super) struct PendingStackedFoldGraphPremises {
    pub expected_instance_id: ProjectId,
    pub expected_project_id: ProjectId,
    pub expected_revision: u64,
    pub expected_source_fingerprint: [u8; 32],
    pub expected_pose_generation: u64,
    pub expected_layer_generation: u64,
    pub requested: PreparedStackedFoldRequestedGraphPoseV1,
    pub continuous: ori_collision::StackedFoldCyclePathDiagnosticV1,
    pub interval_closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub layer_order: PendingStackedFoldLayerProof,
    pub certified_path: Option<ori_collision::CertifiedPoseGraphPathCertificateV1>,
    pub certified_edges: Vec<PendingCertifiedPathEdgeV1>,
}

#[derive(Clone)]
pub(super) enum PendingStackedFoldLayerProof {
    NonFlat(StackedFoldNonFlatLayerOrderV1),
    CertifiedFlat(LayerOrderSnapshot),
}

impl PendingStackedFoldLayerProof {
    fn target_revision(&self) -> u64 {
        match self {
            Self::NonFlat(value) => value.target_revision(),
            Self::CertifiedFlat(value) => value.provenance.source.source_revision,
        }
    }
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
        ) && self.requested.continuous_certified()
            && self.layer_order.target_revision()
                == self
                    .requested
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
        requested: PendingStackedFoldRequestedPose::Tree {
            requested: premises.requested,
            continuous: premises.continuous,
        },
        layer_order: PendingStackedFoldLayerProof::NonFlat(premises.layer_order),
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

pub(super) fn install_pending_stacked_fold_graph(
    state: &StackedFoldTransactionState,
    premises: PendingStackedFoldGraphPremises,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
    let token = ProjectId::new();
    let pending = PendingStackedFoldTransaction {
        token,
        expected_instance_id: premises.expected_instance_id,
        expected_project_id: premises.expected_project_id,
        expected_revision: premises.expected_revision,
        expected_source_fingerprint: premises.expected_source_fingerprint,
        expected_pose_generation: premises.expected_pose_generation,
        expected_layer_generation: premises.expected_layer_generation,
        requested: PendingStackedFoldRequestedPose::Graph {
            requested: premises.requested,
            continuous: premises.continuous,
            interval_closure: premises.interval_closure,
            certified_path: premises.certified_path,
            certified_edges: premises.certified_edges,
        },
        layer_order: premises.layer_order,
        pose_capability,
        layer_capability,
    };
    if !pending.matches_live_binding(
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
        pending.expected_source_fingerprint,
        pending.expected_pose_generation,
        pending.expected_layer_generation,
    ) {
        return Err("The stacked-fold graph transaction premises are inconsistent.".to_owned());
    }
    let mut slot = lock_slot(state)?;
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
    let layer_guard = lock_revalidated_current_layer_order_for_commit(
        &foldability_state,
        &project,
        &pending.layer_capability,
    )
    .map_err(|_| "The current layer-order authority is unavailable.".to_owned())?
    .ok_or_else(|| "The stacked-fold transaction preview is stale.".to_owned())?;

    let requested = &pending.requested;
    let target = requested.geometry();
    if let PendingStackedFoldLayerProof::CertifiedFlat(snapshot) = &pending.layer_order {
        let lineage = target.proof().lineage();
        if !layer_guard.preflight_certified_target(
            pending.expected_project_id,
            lineage.target_revision(),
            lineage.target_fingerprint().0,
            snapshot,
        ) {
            return Err(
                "The certified target layer order could not be prepared atomically.".to_owned(),
            );
        }
    }
    let (face_ids, hinge_ids, fixed_face, hinge_angles) = requested.pose_components();
    let applied_pose = prepare_applied_pose_v1(
        &face_ids,
        &hinge_ids,
        fixed_face,
        &hinge_angles,
        AppliedPoseLimitsV1::default(),
    )
    .map_err(|_| "The target pose is inconsistent.".to_owned())?;
    let candidate = target.candidate();
    let mut timeline = project.editor.instruction_timeline().clone();
    let ordered_timeline_angles = requested.ordered_timeline_angles();
    if ordered_timeline_angles.is_empty() || ordered_timeline_angles.len() > 31 {
        return Err("The certified path timeline is inconsistent.".to_owned());
    }
    for (index, step_angles) in ordered_timeline_angles.into_iter().enumerate() {
        timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: if index == 0 {
                "Stacked fold".to_owned()
            } else {
                format!("Stacked fold {}", index + 1)
            },
            description: String::new(),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: target.proof().lineage().target_fingerprint().to_hex(),
                fixed_face,
                hinge_angles: step_angles
                    .iter()
                    .map(|(edge, angle_degrees)| InstructionHingeAngle {
                        edge: *edge,
                        angle_degrees: *angle_degrees,
                    })
                    .collect(),
            },
        });
    }
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
    match &applied_layer_order {
        PendingStackedFoldLayerProof::NonFlat(_) => {
            layer_guard.invalidate_after_project_mutation();
        }
        PendingStackedFoldLayerProof::CertifiedFlat(snapshot) => {
            layer_guard
                .install_certified_target_after_project_mutation(&project, snapshot.clone())
                .map_err(|_| {
                    "The certified target layer order could not be installed atomically.".to_owned()
                })?;
        }
    }
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

    #[test]
    fn certified_timeline_targets_require_bit_exact_canonical_schedule_endpoints() {
        let mut edges = [ori_domain::EdgeId::new(), ori_domain::EdgeId::new()];
        edges.sort_unstable_by_key(ori_domain::EdgeId::canonical_bytes);
        let expected = ori_kinematics::CanonicalHingeAngles::new(vec![
            ori_kinematics::HingeAngle::new(edges[0], 45.0).unwrap(),
            ori_kinematics::HingeAngle::new(edges[1], 90.0).unwrap(),
        ])
        .unwrap();
        let exact = vec![(edges[0], 45.0), (edges[1], 90.0)];
        assert!(bit_exact_canonical_angles_match(&expected, &exact));

        let mut reordered = exact.clone();
        reordered.reverse();
        assert!(!bit_exact_canonical_angles_match(&expected, &reordered));
        assert!(!bit_exact_canonical_angles_match(
            &expected,
            &[
                (edges[0], 45.0),
                (edges[1], f64::from_bits(90.0_f64.to_bits() + 1))
            ],
        ));
        assert!(!bit_exact_canonical_angles_match(&expected, &exact[..1],));
    }
}
