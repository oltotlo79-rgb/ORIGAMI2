use std::sync::{Mutex, MutexGuard};

use ori_collision::StackedFoldBoundedPathDiagnosticV1;
use ori_core::{
    AppliedPoseLimitsV1, PreparedStackedFoldGeometryV1, PreparedStackedFoldRequestedGraphPoseV1,
    PreparedStackedFoldRequestedPoseV1, StackedFoldNonFlatLayerOrderV1, prepare_applied_pose_v1,
    prepare_closed_graph_applied_pose_v1,
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
    layer_order: Option<PendingStackedFoldLayerProof>,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: Option<CurrentLayerOrderCapability>,
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
    CurrentCycle {
        geometry: ori_kinematics::MaterialHingeGraphGeometry,
        audit: ori_kinematics::MaterialHingeGraphAudit,
        fixed_face: ori_domain::FaceId,
        generated: ori_kinematics::GeneratedMultiHingePathCandidateV1,
        closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
        expected: ori_collision::CertifiedPathTransitionEvidenceV1,
        continuous: ori_collision::StackedFoldCyclePathDiagnosticV1,
        layer_transport: Option<ori_collision::GeneralMultiFaceCellTransportProofV1>,
        layer_order_pairs: Vec<(ori_domain::FaceId, ori_domain::FaceId)>,
        target_angles: Vec<(ori_domain::EdgeId, f64)>,
    },
}

pub(super) struct PendingCertifiedPathEdgeV1 {
    pub generated: ori_kinematics::GeneratedMultiHingePathCandidateV1,
    pub closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub expected: ori_collision::CertifiedPathTransitionEvidenceV1,
    pub target_angles: Vec<(ori_domain::EdgeId, f64)>,
}

impl PendingStackedFoldRequestedPose {
    fn is_graph(&self) -> bool {
        matches!(self, Self::Graph { .. } | Self::CurrentCycle { .. })
    }
    fn geometry(&self) -> Option<&PreparedStackedFoldGeometryV1> {
        match self {
            Self::Tree { requested, .. } => Some(requested.initial().target().geometry()),
            Self::Graph { requested, .. } => Some(requested.initial().target().geometry()),
            Self::CurrentCycle { .. } => None,
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
            Self::CurrentCycle {
                geometry,
                audit,
                fixed_face,
                generated,
                closure,
                expected,
                continuous,
                target_angles,
                ..
            } => {
                certified_edge_target_matches_schedule(&PendingCertifiedPathEdgeV1 {
                    generated: generated.clone(),
                    closure: closure.clone(),
                    expected: expected.clone(),
                    target_angles: target_angles.clone(),
                }) && continuous.continuous_certificate_model_id().is_some()
                    && match continuous.positive_thickness_bits() {
                        Some(bits) => {
                            ori_collision::diagnose_scheduled_positive_thickness_cycle_path_v1(
                                geometry,
                                audit,
                                *fixed_face,
                                generated,
                                closure,
                                f64::from_bits(bits),
                                32,
                            ) == *continuous
                        }
                        None => {
                            ori_collision::diagnose_scheduled_cycle_path_v1(
                                geometry,
                                audit,
                                *fixed_face,
                                generated,
                                closure,
                                32,
                            ) == *continuous
                        }
                    }
                    && ori_collision::certify_scheduled_cycle_transition_v1(
                        geometry,
                        audit,
                        *fixed_face,
                        generated,
                        closure,
                        32,
                        expected.source(),
                        expected.target(),
                    )
                    .is_some_and(|actual| actual == *expected)
            }
        }
    }

    fn persisted_cycle_layer_order_proof(&self) -> Option<ori_domain::CycleLayerOrderProofV1> {
        let Self::CurrentCycle {
            layer_transport: Some(certificate),
            layer_order_pairs,
            ..
        } = self
        else {
            return None;
        };
        let mut pairs = layer_order_pairs
            .iter()
            .map(
                |(lower_face, upper_face)| ori_domain::CycleLayerOrderPairV1 {
                    lower_face: *lower_face,
                    upper_face: *upper_face,
                },
            )
            .collect::<Vec<_>>();
        pairs.sort_unstable_by_key(|pair| {
            (
                pair.lower_face.canonical_bytes(),
                pair.upper_face.canonical_bytes(),
            )
        });
        Some(ori_domain::CycleLayerOrderProofV1 {
            version: 1,
            model_id: ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1.to_owned(),
            target_order_sha256: certificate.target_order_hash(),
            transition_count: certificate.transition_hashes().len(),
            pairs,
        })
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
            Self::CurrentCycle {
                geometry,
                fixed_face,
                target_angles,
                ..
            } => (
                geometry.face_ids().to_vec(),
                geometry.hinges().iter().map(|hinge| hinge.edge()).collect(),
                Some(*fixed_face),
                target_angles.clone(),
            ),
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
            Self::CurrentCycle { target_angles, .. } => vec![target_angles.clone()],
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

pub(super) struct PendingCurrentCyclePosePremisesV1 {
    pub expected_instance_id: ProjectId,
    pub expected_project_id: ProjectId,
    pub expected_revision: u64,
    pub expected_source_fingerprint: [u8; 32],
    pub expected_pose_generation: u64,
    pub expected_layer_generation: u64,
    pub geometry: ori_kinematics::MaterialHingeGraphGeometry,
    pub audit: ori_kinematics::MaterialHingeGraphAudit,
    pub fixed_face: ori_domain::FaceId,
    pub generated: ori_kinematics::GeneratedMultiHingePathCandidateV1,
    pub closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub expected: ori_collision::CertifiedPathTransitionEvidenceV1,
    pub continuous: ori_collision::StackedFoldCyclePathDiagnosticV1,
    pub layer_transport: Option<ori_collision::GeneralMultiFaceCellTransportProofV1>,
    pub layer_order_pairs: Vec<(ori_domain::FaceId, ori_domain::FaceId)>,
    pub target_angles: Vec<(ori_domain::EdgeId, f64)>,
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
            && match (&self.layer_order, self.requested.geometry()) {
                (Some(layer_order), Some(geometry)) => {
                    layer_order.target_revision() == geometry.proof().lineage().target_revision()
                }
                (None, None) => true,
                _ => false,
            }
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
        layer_order: Some(PendingStackedFoldLayerProof::NonFlat(premises.layer_order)),
        pose_capability,
        layer_capability: Some(layer_capability),
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
        layer_order: Some(premises.layer_order),
        pose_capability,
        layer_capability: Some(layer_capability),
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

pub(super) fn install_pending_current_cycle_pose_v1(
    state: &StackedFoldTransactionState,
    premises: PendingCurrentCyclePosePremisesV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: Option<CurrentLayerOrderCapability>,
) -> Result<ProjectId, String> {
    let mut graph_hinges = premises
        .geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    graph_hinges.sort_unstable_by_key(ori_domain::EdgeId::canonical_bytes);
    if !premises.closure.has_canonical_complete_partition_v1()
        || premises.closure.fixed_face() != premises.fixed_face
        || premises.closure.leaves().iter().any(|(_, _, leaf)| {
            leaf.fixed_face() != premises.fixed_face || leaf.checked_hinges() != graph_hinges
        })
    {
        return Err("The current-cycle pose premises are inconsistent.".to_owned());
    }
    if premises
        .layer_transport
        .as_ref()
        .is_some_and(|certificate| {
            !layer_capability.as_ref().is_some_and(|capability| {
                certificate.is_for(
                    &premises.geometry,
                    capability.snapshot(),
                    premises.generated.schedule(),
                    &premises.closure,
                    certificate.paper_thickness_mm(),
                )
            })
        })
    {
        return Err("The current-cycle layer transport premises are inconsistent.".to_owned());
    }
    let token = ProjectId::new();
    let pending = PendingStackedFoldTransaction {
        token,
        expected_instance_id: premises.expected_instance_id,
        expected_project_id: premises.expected_project_id,
        expected_revision: premises.expected_revision,
        expected_source_fingerprint: premises.expected_source_fingerprint,
        expected_pose_generation: premises.expected_pose_generation,
        expected_layer_generation: premises.expected_layer_generation,
        requested: PendingStackedFoldRequestedPose::CurrentCycle {
            geometry: premises.geometry,
            audit: premises.audit,
            fixed_face: premises.fixed_face,
            generated: premises.generated,
            closure: premises.closure,
            expected: premises.expected,
            continuous: premises.continuous,
            layer_transport: premises.layer_transport,
            layer_order_pairs: premises.layer_order_pairs,
            target_angles: premises.target_angles,
        },
        layer_order: None,
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
    ) || !pending.requested.continuous_certified()
    {
        return Err("The current-cycle pose premises are inconsistent.".to_owned());
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
    apply_stacked_fold_transaction_inner(&app_state, &foldability_state, &transaction_state, token)
}

#[tauri::command]
pub(super) fn apply_named_book_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    technique_document_json: String,
    technique_id: String,
) -> Result<u64, String> {
    if technique_document_json.len() > ori_instructions::MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err("The named book-fold document exceeds the resource limit.".to_owned());
    }
    let document =
        ori_instructions::read_fold_technique_file_v1(technique_document_json.as_bytes())
            .map_err(|_| "The named book-fold document is invalid.".to_owned())?;
    let technique = document
        .document()
        .techniques
        .iter()
        .find(|candidate| candidate.id == technique_id)
        .ok_or_else(|| "The named book-fold technique is unavailable.".to_owned())?;
    let physical = technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                ori_instructions::FoldTechniqueActionV1::StraightLineStackedFold
            )
        })
        .collect::<Vec<_>>();
    if physical.len() != 1 || technique.operations.iter().any(|operation| {
        matches!(
            operation.execution_support,
            ori_instructions::FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation { .. }
        )
    }) {
        return Err("Only one proven straight-line book fold can be applied.".to_owned());
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or_else(|| "The named book-fold title is unavailable.".to_owned())?;
    apply_stacked_fold_transaction_with_title(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        Some(&title),
    )
}

#[tauri::command]
pub(super) fn apply_named_reverse_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    technique_document_json: String,
    technique_id: String,
) -> Result<u64, String> {
    if technique_document_json.len() > ori_instructions::MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err("The named reverse-fold document exceeds the resource limit.".to_owned());
    }
    let document =
        ori_instructions::read_fold_technique_file_v1(technique_document_json.as_bytes())
            .map_err(|_| "The named reverse-fold document is invalid.".to_owned())?;
    let technique = document
        .document()
        .techniques
        .iter()
        .find(|candidate| candidate.id == technique_id)
        .ok_or_else(|| "The named reverse-fold technique is unavailable.".to_owned())?;
    let reverse_count = technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                ori_instructions::FoldTechniqueActionV1::InsideReverseFold
                    | ori_instructions::FoldTechniqueActionV1::OutsideReverseFold
            )
        })
        .count();
    if reverse_count != 1 {
        return Err("Exactly one reverse-fold operation is required.".to_owned());
    }
    {
        let slot = lock_slot(&transaction_state)?;
        let pending = slot
            .pending
            .as_ref()
            .filter(|pending| pending.token() == token)
            .ok_or_else(|| "The reverse-fold transaction preview is stale.".to_owned())?;
        if pending.requested.ordered_timeline_angles().len() < 2
            || !pending.requested.continuous_certified()
        {
            return Err("Two continuous certified reverse-fold segments are required.".to_owned());
        }
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or_else(|| "The named reverse-fold title is unavailable.".to_owned())?;
    apply_stacked_fold_transaction_with_title(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        Some(&title),
    )
}

#[tauri::command]
pub(super) fn apply_named_accordion_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    technique_document_json: String,
    technique_id: String,
) -> Result<u64, String> {
    if technique_document_json.len() > ori_instructions::MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err("The accordion-fold document exceeds the resource limit.".to_owned());
    }
    let document =
        ori_instructions::read_fold_technique_file_v1(technique_document_json.as_bytes())
            .map_err(|_| "The accordion-fold document is invalid.".to_owned())?;
    let technique = document
        .document()
        .techniques
        .iter()
        .find(|candidate| candidate.id == technique_id)
        .ok_or_else(|| "The accordion-fold technique is unavailable.".to_owned())?;
    let segment_count = technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                ori_instructions::FoldTechniqueActionV1::StraightLineStackedFold
            )
        })
        .count();
    if segment_count < 3 || segment_count > 31 {
        return Err("At least three bounded accordion segments are required.".to_owned());
    }
    {
        let slot = lock_slot(&transaction_state)?;
        let pending = slot
            .pending
            .as_ref()
            .filter(|pending| pending.token() == token)
            .ok_or_else(|| "The accordion-fold transaction preview is stale.".to_owned())?;
        if pending.requested.ordered_timeline_angles().len() != segment_count
            || !pending.requested.continuous_certified()
        {
            return Err("Every accordion segment requires continuous native authority.".to_owned());
        }
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or_else(|| "The accordion-fold title is unavailable.".to_owned())?;
    apply_stacked_fold_transaction_with_title(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        Some(&title),
    )
}

pub(crate) fn apply_stacked_fold_transaction_inner(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    token: ProjectId,
) -> Result<u64, String> {
    apply_stacked_fold_transaction_with_title(
        app_state,
        foldability_state,
        transaction_state,
        token,
        None,
    )
}

fn apply_stacked_fold_transaction_with_title(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    token: ProjectId,
    named_title: Option<&str>,
) -> Result<u64, String> {
    let mut transaction_slot = lock_slot(transaction_state)?;
    let pending = transaction_slot
        .pending
        .as_ref()
        .filter(|pending| pending.token() == token)
        .ok_or_else(|| "The stacked-fold transaction preview is stale.".to_owned())?;
    let mut project =
        lock_project(app_state).map_err(|_| "The project is unavailable.".to_owned())?;
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
    let layer_guard = pending
        .layer_capability
        .as_ref()
        .map(|capability| {
            lock_revalidated_current_layer_order_for_commit(foldability_state, &project, capability)
                .map_err(|_| "The current layer-order authority is unavailable.".to_owned())?
                .ok_or_else(|| "The stacked-fold transaction preview is stale.".to_owned())
        })
        .transpose()?;

    if let PendingStackedFoldRequestedPose::CurrentCycle {
        geometry,
        generated,
        closure,
        layer_transport: Some(certificate),
        ..
    } = &pending.requested
    {
        let capability = pending
            .layer_capability
            .as_ref()
            .ok_or_else(|| "The current-cycle layer-order authority is unavailable.".to_owned())?;
        if layer_guard.is_none()
            || !certificate.is_for(
                geometry,
                capability.snapshot(),
                generated.schedule(),
                closure,
                certificate.paper_thickness_mm(),
            )
            || !certificate.matches_source_content_v1(capability.snapshot())
        {
            return Err("The current-cycle layer transport preview is stale.".to_owned());
        }
    }

    let requested = &pending.requested;
    let target = requested.geometry();
    if let (Some(target), Some(PendingStackedFoldLayerProof::CertifiedFlat(snapshot))) =
        (target, &pending.layer_order)
    {
        let lineage = target.proof().lineage();
        if !layer_guard.as_ref().is_some_and(|guard| {
            guard.preflight_certified_target(
                pending.expected_project_id,
                lineage.target_revision(),
                lineage.target_fingerprint().0,
                snapshot,
            )
        }) {
            return Err(
                "The certified target layer order could not be prepared atomically.".to_owned(),
            );
        }
    }
    let (face_ids, hinge_ids, fixed_face, hinge_angles) = requested.pose_components();
    let applied_pose = if requested.is_graph() {
        prepare_closed_graph_applied_pose_v1(
            &face_ids,
            &hinge_ids,
            fixed_face.ok_or_else(|| "The target pose is inconsistent.".to_owned())?,
            &hinge_angles,
            AppliedPoseLimitsV1::default(),
        )
    } else {
        prepare_applied_pose_v1(
            &face_ids,
            &hinge_ids,
            fixed_face,
            &hinge_angles,
            AppliedPoseLimitsV1::default(),
        )
    }
    .map_err(|_| "The target pose is inconsistent.".to_owned())?;
    let mut timeline = project.editor.instruction_timeline().clone();
    let ordered_timeline_angles = requested.ordered_timeline_angles();
    let persisted_layer_proof = requested.persisted_cycle_layer_order_proof();
    if ordered_timeline_angles.is_empty() || ordered_timeline_angles.len() > 31 {
        return Err("The certified path timeline is inconsistent.".to_owned());
    }
    for (index, step_angles) in ordered_timeline_angles.into_iter().enumerate() {
        timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: if index == 0 {
                named_title.unwrap_or("Stacked fold").to_owned()
            } else {
                format!("{} {}", named_title.unwrap_or("Stacked fold"), index + 1)
            },
            description: named_title.map_or_else(String::new, |title| {
                format!("認証済みの連続折り経路で名前付き技法「{title}」を適用します。")
            }),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual {
                cycle_layer_order_proof_v1: persisted_layer_proof.clone(),
                ..InstructionVisual::default()
            },
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: target.map_or_else(
                    || project.editor.fold_model_fingerprint_v1(),
                    |target| target.proof().lineage().target_fingerprint().to_hex(),
                ),
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
    let (candidate_pattern, candidate_paper) = target.map_or_else(
        || {
            (
                project.editor.pattern().clone(),
                project.editor.paper().clone(),
            )
        },
        |target| {
            (
                target.candidate().pattern.clone(),
                target.candidate().paper.clone(),
            )
        },
    );
    let result = project
        .editor
        .execute_stacked_fold_document(
            pending.expected_revision,
            candidate_pattern,
            candidate_paper,
            timeline,
            layers,
            applied_pose,
        )
        .map_err(|_| "The stacked-fold transaction could not be applied atomically.".to_owned())?;
    match (&applied_layer_order, layer_guard) {
        (Some(PendingStackedFoldLayerProof::NonFlat(_)) | None, layer_guard) => {
            if let Some(layer_guard) = layer_guard {
                layer_guard.invalidate_after_project_mutation();
            }
        }
        (Some(PendingStackedFoldLayerProof::CertifiedFlat(snapshot)), layer_guard) => {
            layer_guard
                .ok_or_else(|| "The certified target layer order is unavailable.".to_owned())?
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
    transaction_slot.applied_layer_order = applied_layer_order;
    debug_assert!(
        transaction_slot
            .applied_layer_order
            .as_ref()
            .as_ref()
            .is_none_or(|order| order.target_revision() == result.revision)
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
