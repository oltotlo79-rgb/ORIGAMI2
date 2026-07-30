use std::sync::{Arc, Mutex, MutexGuard};

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
use sha2::{Digest, Sha256};
use tauri::State;

mod speculative_unproven;

#[cfg(test)]
pub(crate) use crate::applied_pose::fail_next_transaction_rollback_for_test_v1;
#[cfg(test)]
pub(crate) use speculative_unproven::{
    ApplySpeculativeStackedFoldRequestV1, apply_speculative_stacked_fold_transaction_inner_v1,
    fail_next_speculative_target_pose_reissue_for_test_v1,
    install_pending_speculative_stacked_fold_v1,
};
pub(super) use speculative_unproven::{
    PendingSpeculativeStackedFoldPremisesV1, apply_speculative_stacked_fold_transaction,
    install_pending_speculative_stacked_fold_with_token_v1,
    speculative_tree_diagnostic_is_issuable_v1,
};
#[allow(unused_imports)]
pub(crate) use speculative_unproven::{
    SpeculativeUnprovenFoldResolutionDtoV1, resolve_speculative_unproven_fold_native_v1,
};
pub(super) use speculative_unproven::{
    cancel_post_apply_proof_job_v1, poll_post_apply_proof_job_v1,
    revert_post_apply_proof_failure_v1, start_post_apply_proof_job_v1,
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BasicFoldTimelinePreviewResponse {
    schema_version: u8,
    transaction_token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    source_model_fingerprint: String,
    fixed_face: ori_domain::FaceId,
    fold_edge: ori_domain::EdgeId,
    assignment: String,
    technique_kind: String,
    preview_binding_sha256: String,
    timeline: ori_domain::InstructionTimeline,
}

use super::{
    AppState,
    applied_pose::{
        CurrentAppliedPoseCapability, CurrentAppliedPoseTransactionRollbackV1,
        lock_revalidated_current_applied_pose_for_commit,
        restore_persisted_current_pose_transactional_v1,
    },
    global_flat_foldability::{
        CurrentLayerOrderCapability, CurrentLayerOrderCommitGuard,
        CurrentLayerOrderRollbackSnapshotV1, GlobalFlatFoldabilityState,
        lock_revalidated_current_layer_order_for_commit,
    },
    lock_project,
};

#[derive(Default)]
pub(super) struct StackedFoldTransactionState(
    Mutex<StackedFoldTransactionSlot>,
    Mutex<speculative_unproven::SpeculativeStackedFoldTransactionSlotV1>,
    Mutex<()>,
    Arc<Mutex<speculative_unproven::PostApplyProofRegistryV1>>,
);

#[cfg(test)]
impl StackedFoldTransactionState {
    pub(super) fn pending_token_for_test_v1(&self) -> Option<ProjectId> {
        self.0
            .lock()
            .expect("stacked-fold transaction test lock")
            .pending
            .as_ref()
            .map(|pending| pending.token)
    }

    pub(super) fn speculative_pending_token_for_test_v1(&self) -> Option<ProjectId> {
        self.1
            .lock()
            .expect("speculative stacked-fold transaction test lock")
            .active_generation
    }

    pub(super) fn set_installed_guard_tokens_for_test_v1(
        &self,
        certified: Option<ProjectId>,
        speculative: Option<ProjectId>,
    ) {
        let mut certified_slot = self.0.lock().expect("certified transaction test lock");
        certified_slot.active_generation = certified;
        certified_slot.pending = None;
        certified_slot.last_cancelled = None;
        drop(certified_slot);
        self.1
            .lock()
            .expect("speculative transaction test lock")
            .active_generation = speculative;
    }

    pub(super) fn installed_guard_tokens_for_test_v1(
        &self,
    ) -> (Option<ProjectId>, Option<ProjectId>) {
        (
            self.0
                .lock()
                .expect("certified transaction test lock")
                .active_generation,
            self.1
                .lock()
                .expect("speculative transaction test lock")
                .active_generation,
        )
    }
}

#[derive(Default)]
struct StackedFoldTransactionSlot {
    active_generation: Option<ProjectId>,
    pending: Option<PendingStackedFoldTransaction>,
    last_cancelled: Option<ProjectId>,
    applied_layer_order: Option<CurrentLayerEvidence>,
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
    layer_order: Option<CurrentLayerEvidence>,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: Option<CurrentLayerOrderCapability>,
}

pub(super) enum PendingStackedFoldRequestedPose {
    Tree {
        requested: PreparedStackedFoldRequestedPoseV1,
        continuous: StackedFoldBoundedPathDiagnosticV1,
        paper_thickness_mm: f64,
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
    BlockwiseCurrentCycle {
        geometry: ori_kinematics::MaterialHingeGraphGeometry,
        fixed_face: ori_domain::FaceId,
        authority: ori_collision::BlockwisePositiveLayerAuthorityV1,
        sources: [Box<LayerOrderSnapshot>; 2],
        articulation: ori_domain::FaceId,
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
        layer_order_pairs: Vec<(ori_domain::FaceId, ori_domain::FaceId)>,
        target_angles: Vec<(ori_domain::EdgeId, f64)>,
    },
    ThreeBlockCurrentCycle {
        geometry: ori_kinematics::MaterialHingeGraphGeometry,
        audit: ori_kinematics::MaterialHingeGraphAudit,
        pose: ori_kinematics::ClosedMaterialHingeGraphPose,
        decomposition: ori_kinematics::CanonicalMaterialEdgeBlockDecompositionV1,
        authority: ori_collision::CommonArticulationContinuousLayerPathAuthorityV1,
        common_pose_limits: ori_kinematics::CommonArticulationPoseLimitsV1,
        schedule: ori_kinematics::CanonicalCycleScheduleV1,
        schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
        closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
        clearance_limits: ori_collision::CommonArticulationClearanceLimitsV1,
        block_sources: Vec<LayerOrderSnapshot>,
        source: Box<LayerOrderSnapshot>,
        thickness: f64,
        issuer_context: [u8; 32],
        articulation_layer_fingerprint: [u8; 32],
        layer_order_pairs: Vec<(ori_domain::FaceId, ori_domain::FaceId)>,
        transition_count: usize,
        target_angles: Vec<(ori_domain::EdgeId, f64)>,
    },
}

pub(super) struct PendingCertifiedPathEdgeV1 {
    pub generated: ori_kinematics::GeneratedMultiHingePathCandidateV1,
    pub closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub expected: ori_collision::CertifiedPathTransitionEvidenceV1,
    pub target_angles: Vec<(ori_domain::EdgeId, f64)>,
}

fn canonical_three_block_layer_summary_v1(
    closure: &ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    source: &LayerOrderSnapshot,
) -> Option<(Vec<(ori_domain::FaceId, ori_domain::FaceId)>, usize)> {
    let transition_count = closure.leaves().len().checked_add(1)?;
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(source.face_pair_orders.len())
        .ok()?;
    pairs.extend(
        source
            .face_pair_orders
            .iter()
            .map(|pair| (pair.lower_face.face_id, pair.upper_face.face_id)),
    );
    pairs.sort_unstable_by_key(|(lower, upper)| (lower.canonical_bytes(), upper.canonical_bytes()));
    let source_pair_count = pairs.len();
    pairs.dedup();
    (pairs.len() == source_pair_count).then_some((pairs, transition_count))
}

fn three_block_layer_summary_matches_v1(
    closure: &ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    source: &LayerOrderSnapshot,
    layer_order_pairs: &[(ori_domain::FaceId, ori_domain::FaceId)],
    transition_count: usize,
) -> bool {
    canonical_three_block_layer_summary_v1(closure, source).is_some_and(
        |(expected_pairs, expected_transition_count)| {
            exact_three_block_summary_fields_match_v1(
                &expected_pairs,
                expected_transition_count,
                layer_order_pairs,
                transition_count,
            )
        },
    )
}

fn exact_three_block_summary_fields_match_v1(
    expected_pairs: &[(ori_domain::FaceId, ori_domain::FaceId)],
    expected_transition_count: usize,
    layer_order_pairs: &[(ori_domain::FaceId, ori_domain::FaceId)],
    transition_count: usize,
) -> bool {
    layer_order_pairs == expected_pairs && transition_count == expected_transition_count
}

#[allow(clippy::too_many_arguments)]
fn revalidate_three_block_current_cycle_authority_v1(
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    audit: &ori_kinematics::MaterialHingeGraphAudit,
    pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
    decomposition: &ori_kinematics::CanonicalMaterialEdgeBlockDecompositionV1,
    authority: &ori_collision::CommonArticulationContinuousLayerPathAuthorityV1,
    common_pose_limits: ori_kinematics::CommonArticulationPoseLimitsV1,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    closure: &ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    clearance_limits: ori_collision::CommonArticulationClearanceLimitsV1,
    block_sources: &[LayerOrderSnapshot],
    source: &LayerOrderSnapshot,
    thickness: f64,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
    target_angles: &[(ori_domain::EdgeId, f64)],
    layer_order_pairs: &[(ori_domain::FaceId, ori_domain::FaceId)],
    transition_count: usize,
) -> bool {
    let [first, second, third] = block_sources else {
        return false;
    };
    let block_sources = [first, second, third];
    authority.block_count_v1() == 3
        && decomposition.blocks().len() == 3
        && three_block_layer_summary_matches_v1(
            closure,
            source,
            layer_order_pairs,
            transition_count,
        )
        && authority
            .revalidate_v1(
                ori_collision::CommonArticulationContinuousLayerPathRevalidationInputV1 {
                    geometry,
                    audit,
                    pose,
                    decomposition,
                    common_pose_limits,
                    schedule,
                    schedule_limits,
                    closure,
                    paper_thickness_mm: thickness,
                    clearance_limits,
                    block_sources: &block_sources,
                    issuer_context,
                    articulation_layer_fingerprint,
                    target_angles,
                    source,
                },
            )
            .is_ok()
}

impl PendingStackedFoldRequestedPose {
    fn is_graph(&self) -> bool {
        matches!(
            self,
            Self::Graph { .. }
                | Self::CurrentCycle { .. }
                | Self::BlockwiseCurrentCycle { .. }
                | Self::ThreeBlockCurrentCycle { .. }
        )
    }
    fn geometry(&self) -> Option<&PreparedStackedFoldGeometryV1> {
        match self {
            Self::Tree { requested, .. } => Some(requested.initial().target().geometry()),
            Self::Graph { requested, .. } => Some(requested.initial().target().geometry()),
            Self::CurrentCycle { .. }
            | Self::BlockwiseCurrentCycle { .. }
            | Self::ThreeBlockCurrentCycle { .. } => None,
        }
    }
    fn continuous_certified(&self) -> bool {
        match self {
            Self::Tree {
                requested,
                continuous,
                paper_thickness_mm,
            } => {
                let source = requested.initial().pose().hinge_angles();
                let target = requested.pose().hinge_angles();
                ori_collision::diagnose_collective_hinge_path_from_pose_v1(
                    requested.initial().target().model(),
                    requested.initial().pose(),
                    source,
                    target,
                    *paper_thickness_mm,
                    ori_collision::StackedFoldPathDiagnosticLimitsV1::default(),
                )
                .is_ok_and(|revalidated| {
                    revalidated == *continuous
                        && revalidated.continuous_clearance_certified()
                        && !revalidated.authorizes_project_mutation()
                })
            }
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
            Self::BlockwiseCurrentCycle {
                authority,
                sources,
                articulation,
                thickness,
                issuer_context,
                articulation_layer_fingerprint,
                target_angles,
                ..
            } => {
                authority.target_angles_match_v1(target_angles)
                    && authority.revalidates_v1(
                        [&sources[0], &sources[1]],
                        *articulation,
                        *thickness,
                        *issuer_context,
                        *articulation_layer_fingerprint,
                    )
            }
            Self::ThreeBlockCurrentCycle {
                geometry,
                audit,
                pose,
                decomposition,
                authority,
                common_pose_limits,
                schedule,
                schedule_limits,
                closure,
                clearance_limits,
                block_sources,
                source,
                thickness,
                issuer_context,
                articulation_layer_fingerprint,
                target_angles,
                layer_order_pairs,
                transition_count,
                ..
            } => revalidate_three_block_current_cycle_authority_v1(
                geometry,
                audit,
                pose,
                decomposition,
                authority,
                *common_pose_limits,
                schedule,
                *schedule_limits,
                closure,
                *clearance_limits,
                block_sources,
                source,
                *thickness,
                *issuer_context,
                *articulation_layer_fingerprint,
                target_angles,
                layer_order_pairs,
                *transition_count,
            ),
        }
    }

    fn persisted_cycle_layer_order_proof(&self) -> Option<ori_domain::CycleLayerOrderProofV1> {
        let Self::CurrentCycle {
            layer_transport: Some(certificate),
            layer_order_pairs,
            ..
        } = self
        else {
            if let Self::BlockwiseCurrentCycle {
                authority,
                layer_order_pairs,
                ..
            } = self
            {
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
                return Some(ori_domain::CycleLayerOrderProofV1 {
                    version: 1,
                    model_id: ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1.to_owned(),
                    target_order_sha256: authority.target_order_hash_v1(),
                    transition_count: authority.transition_count_v1(),
                    pairs,
                });
            }
            if let Self::ThreeBlockCurrentCycle {
                authority,
                layer_order_pairs,
                transition_count,
                closure,
                source,
                ..
            } = self
            {
                let (canonical_pairs, expected_transition_count) =
                    canonical_three_block_layer_summary_v1(closure, source)?;
                if canonical_pairs.as_slice() != layer_order_pairs
                    || expected_transition_count != *transition_count
                {
                    return None;
                }
                let pairs = canonical_pairs
                    .iter()
                    .map(
                        |(lower_face, upper_face)| ori_domain::CycleLayerOrderPairV1 {
                            lower_face: *lower_face,
                            upper_face: *upper_face,
                        },
                    )
                    .collect::<Vec<_>>();
                return Some(ori_domain::CycleLayerOrderProofV1 {
                    version: 1,
                    model_id: ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1.to_owned(),
                    target_order_sha256: authority.whole_parent_target_order_hash_v1(),
                    transition_count: expected_transition_count,
                    pairs,
                });
            }
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
            Self::BlockwiseCurrentCycle {
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
            Self::ThreeBlockCurrentCycle {
                geometry,
                pose,
                target_angles,
                ..
            } => (
                geometry.face_ids().to_vec(),
                geometry.hinges().iter().map(|hinge| hinge.edge()).collect(),
                Some(pose.fixed_face()),
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
            Self::CurrentCycle { target_angles, .. }
            | Self::BlockwiseCurrentCycle { target_angles, .. }
            | Self::ThreeBlockCurrentCycle { target_angles, .. } => vec![target_angles.clone()],
            _ => vec![self.pose_components().3],
        }
    }

    fn certified_graph_path(&self) -> Option<&ori_collision::CertifiedPoseGraphPathCertificateV1> {
        match self {
            Self::Graph {
                certified_path: Some(path),
                ..
            } => Some(path),
            _ => None,
        }
    }

    fn certified_graph_source_angles(&self) -> Option<Vec<(ori_domain::EdgeId, f64)>> {
        let Self::Graph {
            certified_path: Some(_),
            certified_edges,
            ..
        } = self
        else {
            return None;
        };
        certified_edges
            .first()?
            .generated
            .schedule()
            .evaluate(0.0)
            .map(|angles| {
                angles
                    .as_slice()
                    .iter()
                    .map(|angle| (angle.edge(), angle.angle_degrees()))
                    .collect()
            })
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

fn lowercase_hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
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
    pub paper_thickness_mm: f64,
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
    pub layer_order: CurrentLayerEvidence,
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

pub(super) struct PendingBlockwiseCurrentCyclePremisesV1 {
    pub expected_instance_id: ProjectId,
    pub expected_project_id: ProjectId,
    pub expected_revision: u64,
    pub expected_source_fingerprint: [u8; 32],
    pub expected_pose_generation: u64,
    pub expected_layer_generation: u64,
    pub geometry: ori_kinematics::MaterialHingeGraphGeometry,
    pub fixed_face: ori_domain::FaceId,
    pub authority: ori_collision::BlockwisePositiveLayerAuthorityV1,
    pub sources: [Box<LayerOrderSnapshot>; 2],
    pub articulation: ori_domain::FaceId,
    pub thickness: f64,
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub layer_order_pairs: Vec<(ori_domain::FaceId, ori_domain::FaceId)>,
    pub target_angles: Vec<(ori_domain::EdgeId, f64)>,
}

pub(super) struct PendingThreeBlockCurrentCyclePremisesV1 {
    pub expected_instance_id: ProjectId,
    pub expected_project_id: ProjectId,
    pub expected_revision: u64,
    pub expected_source_fingerprint: [u8; 32],
    pub expected_pose_generation: u64,
    pub expected_layer_generation: u64,
    pub geometry: ori_kinematics::MaterialHingeGraphGeometry,
    pub audit: ori_kinematics::MaterialHingeGraphAudit,
    pub pose: ori_kinematics::ClosedMaterialHingeGraphPose,
    pub decomposition: ori_kinematics::CanonicalMaterialEdgeBlockDecompositionV1,
    pub authority: ori_collision::CommonArticulationContinuousLayerPathAuthorityV1,
    pub common_pose_limits: ori_kinematics::CommonArticulationPoseLimitsV1,
    pub schedule: ori_kinematics::CanonicalCycleScheduleV1,
    pub schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    pub closure: ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub clearance_limits: ori_collision::CommonArticulationClearanceLimitsV1,
    pub block_sources: Vec<LayerOrderSnapshot>,
    pub source: Box<LayerOrderSnapshot>,
    pub thickness: f64,
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub layer_order_pairs: Vec<(ori_domain::FaceId, ori_domain::FaceId)>,
    pub transition_count: usize,
    pub target_angles: Vec<(ori_domain::EdgeId, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum CurrentLayerEvidence {
    NonFlat(StackedFoldNonFlatLayerOrderV1),
    CertifiedFlat(LayerOrderSnapshot),
}

impl CurrentLayerEvidence {
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

#[must_use]
pub(super) const fn next_current_cycle_target_revision_v1(source_revision: u64) -> Option<u64> {
    match source_revision.checked_add(1) {
        Some(target_revision) if target_revision <= ori_core::MAX_REVISION => Some(target_revision),
        Some(_) | None => None,
    }
}

pub(super) fn install_pending_stacked_fold_with_token_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
    premises: PendingStackedFoldPremises,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
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
            paper_thickness_mm: premises.paper_thickness_mm,
        },
        layer_order: Some(CurrentLayerEvidence::NonFlat(premises.layer_order)),
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
    with_try_locked_transaction_install_slots_v1(state, |slot, speculative_slot| {
        speculative_unproven::clear_pending_speculative_slot_locked_v1(speculative_slot);
        slot.active_generation = Some(token);
        slot.pending = Some(pending);
        token
    })
}

pub(super) fn install_pending_stacked_fold_graph_with_token_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
    premises: PendingStackedFoldGraphPremises,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
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
    with_try_locked_transaction_install_slots_v1(state, |slot, speculative_slot| {
        speculative_unproven::clear_pending_speculative_slot_locked_v1(speculative_slot);
        slot.active_generation = Some(token);
        slot.pending = Some(pending);
        token
    })
}

pub(super) fn install_pending_current_cycle_pose_with_token_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
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
    with_project_guarded_current_cycle_install_slots_v1(
        state,
        pending.expected_revision,
        |slot, speculative_slot| {
            speculative_unproven::clear_pending_speculative_slot_locked_v1(speculative_slot);
            slot.active_generation = Some(token);
            slot.pending = Some(pending);
            token
        },
    )
}

pub(super) fn install_pending_blockwise_current_cycle_pose_with_token_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
    premises: PendingBlockwiseCurrentCyclePremisesV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
    if premises.fixed_face != premises.articulation
        || premises.issuer_context != premises.expected_source_fingerprint
        || !premises.authority.revalidates_v1(
            [&premises.sources[0], &premises.sources[1]],
            premises.articulation,
            premises.thickness,
            premises.issuer_context,
            premises.articulation_layer_fingerprint,
        )
    {
        return Err("The blockwise current-cycle premises are inconsistent.".to_owned());
    }
    let pending = PendingStackedFoldTransaction {
        token,
        expected_instance_id: premises.expected_instance_id,
        expected_project_id: premises.expected_project_id,
        expected_revision: premises.expected_revision,
        expected_source_fingerprint: premises.expected_source_fingerprint,
        expected_pose_generation: premises.expected_pose_generation,
        expected_layer_generation: premises.expected_layer_generation,
        requested: PendingStackedFoldRequestedPose::BlockwiseCurrentCycle {
            geometry: premises.geometry,
            fixed_face: premises.fixed_face,
            authority: premises.authority,
            sources: premises.sources,
            articulation: premises.articulation,
            thickness: premises.thickness,
            issuer_context: premises.issuer_context,
            articulation_layer_fingerprint: premises.articulation_layer_fingerprint,
            layer_order_pairs: premises.layer_order_pairs,
            target_angles: premises.target_angles,
        },
        layer_order: None,
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
        return Err("The blockwise current-cycle premises are inconsistent.".to_owned());
    }
    with_project_guarded_current_cycle_install_slots_v1(
        state,
        pending.expected_revision,
        |slot, speculative_slot| {
            speculative_unproven::clear_pending_speculative_slot_locked_v1(speculative_slot);
            slot.active_generation = Some(token);
            slot.pending = Some(pending);
            token
        },
    )
}

pub(super) fn install_pending_three_block_current_cycle_pose_with_token_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
    premises: PendingThreeBlockCurrentCyclePremisesV1,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
) -> Result<ProjectId, String> {
    if premises.issuer_context != premises.expected_source_fingerprint
        || premises.expected_pose_generation != pose_capability.generation()
        || premises.expected_layer_generation != layer_capability.generation()
        || premises.source.as_ref() != layer_capability.snapshot()
        || premises.transition_count == 0
        || !revalidate_three_block_current_cycle_authority_v1(
            &premises.geometry,
            &premises.audit,
            &premises.pose,
            &premises.decomposition,
            &premises.authority,
            premises.common_pose_limits,
            &premises.schedule,
            premises.schedule_limits,
            &premises.closure,
            premises.clearance_limits,
            &premises.block_sources,
            &premises.source,
            premises.thickness,
            premises.issuer_context,
            premises.articulation_layer_fingerprint,
            &premises.target_angles,
            &premises.layer_order_pairs,
            premises.transition_count,
        )
    {
        return Err("The three-block current-cycle premises are inconsistent.".to_owned());
    }
    let pending = PendingStackedFoldTransaction {
        token,
        expected_instance_id: premises.expected_instance_id,
        expected_project_id: premises.expected_project_id,
        expected_revision: premises.expected_revision,
        expected_source_fingerprint: premises.expected_source_fingerprint,
        expected_pose_generation: premises.expected_pose_generation,
        expected_layer_generation: premises.expected_layer_generation,
        requested: PendingStackedFoldRequestedPose::ThreeBlockCurrentCycle {
            geometry: premises.geometry,
            audit: premises.audit,
            pose: premises.pose,
            decomposition: premises.decomposition,
            authority: premises.authority,
            common_pose_limits: premises.common_pose_limits,
            schedule: premises.schedule,
            schedule_limits: premises.schedule_limits,
            closure: premises.closure,
            clearance_limits: premises.clearance_limits,
            block_sources: premises.block_sources,
            source: premises.source,
            thickness: premises.thickness,
            issuer_context: premises.issuer_context,
            articulation_layer_fingerprint: premises.articulation_layer_fingerprint,
            layer_order_pairs: premises.layer_order_pairs,
            transition_count: premises.transition_count,
            target_angles: premises.target_angles,
        },
        layer_order: None,
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
        return Err("The three-block current-cycle premises are inconsistent.".to_owned());
    }
    with_project_guarded_current_cycle_install_slots_v1(
        state,
        pending.expected_revision,
        |slot, speculative_slot| {
            speculative_unproven::clear_pending_speculative_slot_locked_v1(speculative_slot);
            slot.active_generation = Some(token);
            slot.pending = Some(pending);
            token
        },
    )
}

pub(super) fn cancel_pending_stacked_fold(
    state: &StackedFoldTransactionState,
    token: ProjectId,
) -> Result<(), String> {
    let _mode_guard = lock_transaction_mode_gate_v1(state)?;
    if try_cancel_pending_certified_stacked_fold_v1(state, token)?
        || speculative_unproven::try_cancel_pending_speculative_stacked_fold_v1(state, token)?
    {
        return Ok(());
    }
    Err("The stacked-fold transaction preview is stale.".to_owned())
}

fn try_cancel_pending_certified_stacked_fold_v1(
    state: &StackedFoldTransactionState,
    token: ProjectId,
) -> Result<bool, String> {
    let mut slot = lock_slot(state)?;
    if slot.last_cancelled == Some(token) {
        return Ok(true);
    }
    if slot.active_generation != Some(token)
        || slot
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token() != token)
    {
        return Ok(false);
    }
    slot.pending = None;
    slot.active_generation = None;
    slot.last_cancelled = Some(token);
    Ok(true)
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

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(super) fn preview_named_basic_fold_timeline(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_source_model_fingerprint: String,
    fold_edge: ori_domain::EdgeId,
    assignment: String,
    technique_kind: String,
    technique_document_json: String,
    technique_id: String,
) -> Result<BasicFoldTimelinePreviewResponse, String> {
    compile_named_basic_fold_preview(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        expected_source_model_fingerprint,
        fold_edge,
        assignment,
        technique_kind,
        technique_document_json,
        technique_id,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_named_basic_fold_preview(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_source_model_fingerprint: String,
    fold_edge: ori_domain::EdgeId,
    assignment: String,
    technique_kind: String,
    technique_document_json: String,
    technique_id: String,
) -> Result<BasicFoldTimelinePreviewResponse, String> {
    let basic_kind = match technique_kind.as_str() {
        "mountain" if assignment == "mountain" => Some(ori_instructions::BasicFoldKindV1::Mountain),
        "valley" if assignment == "valley" => Some(ori_instructions::BasicFoldKindV1::Valley),
        "squash" | "crimp" | "inside_reverse" | "outside_reverse" | "sink" | "accordion"
        | "layer_selective"
            if matches!(assignment.as_str(), "mountain" | "valley") =>
        {
            None
        }
        _ => return Err("The basic-fold assignment is unsupported.".to_owned()),
    };
    let technique =
        ori_instructions::read_fold_technique_file_v1(technique_document_json.as_bytes())
            .map_err(|_| "The named basic-fold document is invalid.".to_owned())?;
    let slot = lock_slot(&transaction_state)?;
    let pending = slot
        .pending
        .as_ref()
        .filter(|pending| pending.token() == token)
        .ok_or_else(|| "The basic-fold transaction preview is stale.".to_owned())?;
    let project = lock_project(&app_state).map_err(|_| "The project is unavailable.".to_owned())?;
    let fingerprint = fold_model_fingerprint_v1(project.editor.pattern(), project.editor.paper());
    if project.instance_id != expected_project_instance_id
        || project.project_id != expected_project_id
        || project.editor.revision() != expected_revision
        || fingerprint.to_hex() != expected_source_model_fingerprint
        || !pending.matches_live_binding(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            fingerprint.0,
            pending.expected_pose_generation,
            pending.expected_layer_generation,
        )
    {
        return Err("The basic-fold transaction preview is stale.".to_owned());
    }
    let _pose_guard =
        lock_revalidated_current_applied_pose_for_commit(&project, &pending.pose_capability)
            .map_err(|_| "The current pose authority is unavailable.".to_owned())?
            .ok_or_else(|| "The basic-fold transaction preview is stale.".to_owned())?;
    if let Some(capability) = pending.layer_capability.as_ref() {
        lock_revalidated_current_layer_order_for_commit(&foldability_state, &project, capability)
            .map_err(|_| "The current layer-order authority is unavailable.".to_owned())?
            .ok_or_else(|| "The basic-fold transaction preview is stale.".to_owned())?;
    }
    if technique_kind == "layer_selective" && pending.layer_capability.is_none() {
        return Err("Certified layer-selection authority is required.".to_owned());
    }
    let (_, hinge_ids, pending_fixed_face, _) = pending.requested.pose_components();
    let fixed_face = pending_fixed_face
        .ok_or_else(|| "The basic-fold fixed-face authority is unavailable.".to_owned())?;
    let source_angles = pending
        .requested
        .certified_graph_source_angles()
        .ok_or_else(|| "The basic-fold path certificate is unavailable.".to_owned())?;
    let targets = pending.requested.ordered_timeline_angles();
    let certificate = pending
        .requested
        .certified_graph_path()
        .ok_or_else(|| "The basic-fold path certificate is unavailable.".to_owned())?;
    if hinge_ids.first() != Some(&fold_edge)
        || targets.is_empty()
        || !project.editor.pattern().edges.iter().any(|edge| {
            edge.id == fold_edge
                && (basic_kind.is_some()
                    || matches!(
                        (assignment.as_str(), edge.kind),
                        ("mountain", ori_domain::EdgeKind::Mountain)
                            | ("valley", ori_domain::EdgeKind::Valley)
                    ))
                && matches!(
                    (basic_kind, edge.kind),
                    (
                        Some(ori_instructions::BasicFoldKindV1::Mountain),
                        ori_domain::EdgeKind::Mountain
                    ) | (
                        Some(ori_instructions::BasicFoldKindV1::Valley),
                        ori_domain::EdgeKind::Valley
                    ) | (
                        None,
                        ori_domain::EdgeKind::Mountain | ori_domain::EdgeKind::Valley
                    )
                )
        })
    {
        return Err("The basic-fold transaction binding was tampered with.".to_owned());
    }
    let source_hinge_angles = source_angles
        .into_iter()
        .map(|(edge, angle_degrees)| InstructionHingeAngle {
            edge,
            angle_degrees,
        })
        .collect::<Vec<_>>();
    let target_angle = targets[0]
        .iter()
        .find(|(edge, _)| *edge == fold_edge)
        .map(|(_, angle)| *angle)
        .filter(|angle| angle.is_finite() && (0.0..=180.0).contains(angle))
        .ok_or_else(|| "The basic-fold target angle is invalid.".to_owned())?;
    let straight_fold = || ori_instructions::BookFoldMotionRequestV1 {
        technique_file: &technique,
        technique_id: &technique_id,
        source_model_fingerprint: &expected_source_model_fingerprint,
        fixed_face,
        fold_edge,
        source_hinge_angles: &source_hinge_angles,
        target_angle_microdegrees: (target_angle * 1_000_000.0) as i64,
        path_certificate: certificate,
    };
    let mut timeline = if let Some(kind) = basic_kind {
        if certificate.edges().len() != 1 || targets.len() != 1 || hinge_ids.len() != 1 {
            return Err("The basic-fold path certificate is unavailable.".to_owned());
        }
        ori_instructions::compile_certified_basic_fold_timeline_v1(
            ori_instructions::BasicFoldMotionRequestV1 {
                kind,
                straight_fold: straight_fold(),
            },
        )
        .map_err(|_| "The named basic-fold compiler rejected the preview.".to_owned())?
    } else if technique_kind == "accordion" {
        let count = certificate.edges().len();
        if !(3..=31).contains(&count) || targets.len() != count || hinge_ids.len() != count {
            return Err(
                "Three to thirty-one continuous certified fold segments are required.".to_owned(),
            );
        }
        let assignments = hinge_ids
            .iter()
            .map(|hinge| {
                project
                    .editor
                    .pattern()
                    .edges
                    .iter()
                    .find(|edge| edge.id == *hinge)
                    .and_then(|edge| match edge.kind {
                        ori_domain::EdgeKind::Mountain => Some("mountain"),
                        ori_domain::EdgeKind::Valley => Some("valley"),
                        _ => None,
                    })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                "Every accordion segment must bind a mountain or valley crease.".to_owned()
            })?;
        if !accordion_assignments_alternate_v1(&assignments) {
            return Err("Accordion segment assignments must alternate.".to_owned());
        }
        let ordered_target_angles_microdegrees = hinge_ids
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                targets[index]
                    .iter()
                    .find(|(candidate, _)| candidate == edge)
                    .map(|(_, value)| *value)
                    .filter(|value| value.is_finite() && (0.0..=180.0).contains(value))
                    .map(|value| (value * 1_000_000.0) as i64)
                    .ok_or_else(|| "An accordion target angle is invalid.".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ordered_path_certificates = (0..count)
            .map(|index| {
                certificate
                    .segment_certificate_v1(index)
                    .ok_or_else(|| "An accordion path segment is unavailable.".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        ori_instructions::compile_certified_accordion_fold_timeline_v1(
            ori_instructions::AccordionFoldMotionRequestV1 {
                technique_file: &technique,
                technique_id: &technique_id,
                source_model_fingerprint: &expected_source_model_fingerprint,
                fixed_face,
                source_hinge_angles: &source_hinge_angles,
                ordered_edges: &hinge_ids,
                ordered_target_angles_microdegrees: &ordered_target_angles_microdegrees,
                ordered_path_certificates: &ordered_path_certificates,
            },
        )
        .map_err(|_| "The named accordion compiler rejected the preview.".to_owned())?
    } else {
        if certificate.edges().len() != 2 || targets.len() != 2 || hinge_ids.len() != 2 {
            return Err("Two continuous certified fold segments are required.".to_owned());
        }
        let first = certificate
            .segment_certificate_v1(0)
            .ok_or_else(|| "The first fold segment is unavailable.".to_owned())?;
        let second = certificate
            .segment_certificate_v1(1)
            .ok_or_else(|| "The second fold segment is unavailable.".to_owned())?;
        let angle = |index: usize, edge: ori_domain::EdgeId| {
            targets[index]
                .iter()
                .find(|(candidate, _)| *candidate == edge)
                .map(|(_, value)| *value)
                .filter(|value| value.is_finite() && (0.0..=180.0).contains(value))
                .map(|value| (value * 1_000_000.0) as i64)
                .ok_or_else(|| "A two-segment target angle is invalid.".to_owned())
        };
        let request = ori_instructions::SinkFoldMotionRequestV1 {
            technique_file: &technique,
            technique_id: &technique_id,
            source_model_fingerprint: &expected_source_model_fingerprint,
            fixed_face,
            first_edge: hinge_ids[0],
            second_edge: hinge_ids[1],
            source_hinge_angles: &source_hinge_angles,
            intermediate_angle_microdegrees: angle(0, hinge_ids[0])?,
            target_angle_microdegrees: angle(1, hinge_ids[1])?,
            first_path_certificate: &first,
            second_path_certificate: &second,
        };
        match technique_kind.as_str() {
            "squash" => ori_instructions::compile_certified_squash_fold_timeline_v1(request),
            "crimp" => ori_instructions::compile_certified_crimp_fold_timeline_v1(request),
            "sink" => ori_instructions::compile_certified_sink_fold_timeline_v1(request),
            "layer_selective" => {
                ori_instructions::compile_certified_layer_selective_timeline_v1(request)
            }
            "inside_reverse" | "outside_reverse" => {
                let reverse = ori_instructions::ReverseFoldMotionRequestV1 {
                    technique_file: request.technique_file,
                    technique_id: request.technique_id,
                    kind: if technique_kind == "inside_reverse" {
                        ori_instructions::ReverseFoldKindV1::Inside
                    } else {
                        ori_instructions::ReverseFoldKindV1::Outside
                    },
                    source_model_fingerprint: request.source_model_fingerprint,
                    fixed_face: request.fixed_face,
                    first_edge: request.first_edge,
                    second_edge: request.second_edge,
                    source_hinge_angles: request.source_hinge_angles,
                    intermediate_angle_microdegrees: request.intermediate_angle_microdegrees,
                    target_angle_microdegrees: request.target_angle_microdegrees,
                    first_path_certificate: request.first_path_certificate,
                    second_path_certificate: request.second_path_certificate,
                };
                ori_instructions::compile_certified_reverse_fold_timeline_v1(reverse)
            }
            _ => unreachable!(),
        }
        .map_err(|_| "The named two-segment compiler rejected the preview.".to_owned())?
    };
    bind_named_technique_compiler_metadata_v1(&mut timeline, &technique_kind)?;
    let preview_binding_sha256 = basic_fold_preview_binding_v1(
        token,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        &technique_kind,
        &timeline,
    )?;
    Ok(BasicFoldTimelinePreviewResponse {
        schema_version: 1,
        transaction_token: token,
        project_instance_id: expected_project_instance_id,
        project_id: expected_project_id,
        revision: expected_revision,
        source_model_fingerprint: expected_source_model_fingerprint,
        fixed_face,
        fold_edge,
        assignment,
        technique_kind,
        preview_binding_sha256,
        timeline,
    })
}

pub(super) fn bind_named_technique_compiler_metadata_v1(
    timeline: &mut ori_domain::InstructionTimeline,
    technique_kind: &str,
) -> Result<(), String> {
    let compiler_output_sha256 =
        ori_domain::named_technique_compiler_output_sha256_v1(&timeline.steps)
            .ok_or_else(|| "The compiler timeline metadata could not be bound.".to_owned())?;
    let segment_count = timeline.steps.len();
    for (segment_index, step) in timeline.steps.iter_mut().enumerate() {
        step.visual.named_technique_compiler_v1 =
            Some(ori_domain::NamedTechniqueCompilerMetadataV1 {
                version: 1,
                model_id: ori_domain::NAMED_TECHNIQUE_COMPILER_MODEL_ID_V1.to_owned(),
                technique_kind: technique_kind.to_owned(),
                segment_index,
                segment_count,
                compiler_output_sha256,
            });
    }
    ori_domain::validate_instruction_timeline(timeline)
        .map_err(|_| "The compiler timeline metadata is invalid.".to_owned())
}

pub(super) fn regular_quad_petal_face_v1(
    project: &super::ProjectState,
    hinges: &[ori_domain::EdgeId],
) -> bool {
    if hinges.len() != 3
        || hinges
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != 3
    {
        return false;
    }
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let Some(snapshot) = topology.simulation_snapshot() else {
        return false;
    };
    let pattern = project.editor.pattern();
    snapshot.faces.iter().any(|face| {
        if face.outer.half_edges.len() != 4
            || !face.holes.is_empty()
            || !face.seams.is_empty()
            || face
                .outer
                .half_edges
                .iter()
                .filter(|half| {
                    snapshot
                        .hinge_adjacency
                        .iter()
                        .any(|adjacency| adjacency.edge == half.edge)
                })
                .count()
                != 3
            || !hinges
                .iter()
                .all(|hinge| face.outer.half_edges.iter().any(|half| half.edge == *hinge))
        {
            return false;
        }
        let points = face
            .outer
            .half_edges
            .iter()
            .map(|half| {
                pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == half.origin)
                    .map(|vertex| vertex.position)
            })
            .collect::<Option<Vec<_>>>();
        let Some(points) = points else {
            return false;
        };
        let sides = (0..4)
            .map(|index| {
                (
                    points[(index + 1) % 4].x - points[index].x,
                    points[(index + 1) % 4].y - points[index].y,
                )
            })
            .collect::<Vec<_>>();
        let lengths = sides
            .iter()
            .map(|(x, y)| x.mul_add(*x, y * y))
            .collect::<Vec<_>>();
        let scale = lengths.iter().copied().fold(0.0_f64, f64::max);
        scale.is_finite()
            && scale > 0.0
            && lengths.iter().all(|length| *length == scale)
            && (0..4).all(|index| {
                let (ax, ay) = sides[index];
                let (bx, by) = sides[(index + 1) % 4];
                ax.mul_add(bx, ay * by) == 0.0
            })
    })
}

fn accordion_assignments_alternate_v1(assignments: &[&str]) -> bool {
    (3..=31).contains(&assignments.len()) && assignments.windows(2).all(|pair| pair[0] != pair[1])
}

fn basic_fold_preview_binding_v1(
    token: ProjectId,
    instance: ProjectId,
    project: ProjectId,
    revision: u64,
    technique_kind: &str,
    timeline: &ori_domain::InstructionTimeline,
) -> Result<String, String> {
    let timeline_sha256 = ori_domain::named_technique_preview_timeline_sha256_v1(timeline)
        .ok_or_else(|| "The basic-fold preview could not be bound.".to_owned())?;
    let mut hash = Sha256::new();
    hash.update(b"named_basic_fold_timeline_preview_binding_v1");
    hash.update(token.canonical_bytes());
    hash.update(instance.canonical_bytes());
    hash.update(project.canonical_bytes());
    hash.update(revision.to_be_bytes());
    hash.update((technique_kind.len() as u64).to_be_bytes());
    hash.update(technique_kind.as_bytes());
    hash.update(timeline_sha256);
    Ok(lowercase_hex(hash.finalize().into()))
}

#[tauri::command]
pub(super) fn apply_named_book_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_source_model_fingerprint: String,
    fold_edge: ori_domain::EdgeId,
    assignment: String,
    technique_kind: String,
    expected_preview_binding_sha256: String,
    technique_document_json: String,
    technique_id: String,
) -> Result<u64, String> {
    if technique_document_json.len() > ori_instructions::MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err("The named book-fold document exceeds the resource limit.".to_owned());
    }
    let compiled = compile_named_basic_fold_preview(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        expected_source_model_fingerprint,
        fold_edge,
        assignment,
        technique_kind,
        technique_document_json,
        technique_id,
    )?;
    if compiled.preview_binding_sha256 != expected_preview_binding_sha256 {
        return Err("The named basic-fold preview binding is stale or tampered.".to_owned());
    }
    apply_stacked_fold_transaction_with_title(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        None,
        Some(compiled.timeline),
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
        None,
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
    if !(3..=31).contains(&segment_count) {
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
        None,
    )
}

#[tauri::command]
pub(super) fn apply_named_sink_fold_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    technique_document_json: String,
    technique_id: String,
) -> Result<u64, String> {
    if technique_document_json.len() > ori_instructions::MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err("The sink-fold document exceeds the resource limit.".to_owned());
    }
    let document =
        ori_instructions::read_fold_technique_file_v1(technique_document_json.as_bytes())
            .map_err(|_| "The sink-fold document is invalid.".to_owned())?;
    let technique = document
        .document()
        .techniques
        .iter()
        .find(|value| value.id == technique_id)
        .ok_or_else(|| "The sink-fold technique is unavailable.".to_owned())?;
    if technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                ori_instructions::FoldTechniqueActionV1::SinkFold { .. }
            )
        })
        .count()
        != 1
    {
        return Err("Exactly one validated sink-fold operation is required.".to_owned());
    }
    {
        let slot = lock_slot(&transaction_state)?;
        let pending = slot
            .pending
            .as_ref()
            .filter(|pending| pending.token() == token)
            .ok_or_else(|| "The sink-fold preview is stale.".to_owned())?;
        if pending.requested.ordered_timeline_angles().len() != 2
            || !pending.requested.continuous_certified()
        {
            return Err("Two continuous certified sink-fold segments are required.".to_owned());
        }
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or_else(|| "The sink-fold title is unavailable.".to_owned())?;
    apply_stacked_fold_transaction_with_title(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        Some(&title),
        None,
    )
}

#[tauri::command]
pub(super) fn apply_named_layer_selective_transaction(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, StackedFoldTransactionState>,
    token: ProjectId,
    technique_document_json: String,
    technique_id: String,
) -> Result<u64, String> {
    let document =
        ori_instructions::read_fold_technique_file_v1(technique_document_json.as_bytes())
            .map_err(|_| "The layer-selective document is invalid.".to_owned())?;
    let technique = document
        .document()
        .techniques
        .iter()
        .find(|value| value.id == technique_id)
        .ok_or_else(|| "The layer-selective technique is unavailable.".to_owned())?;
    if technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                ori_instructions::FoldTechniqueActionV1::LayerSelectiveManipulation { .. }
            )
        })
        .count()
        != 1
    {
        return Err("Exactly one layer-selective operation is required.".to_owned());
    }
    {
        let slot = lock_slot(&transaction_state)?;
        let pending = slot
            .pending
            .as_ref()
            .filter(|pending| pending.token() == token)
            .ok_or_else(|| "The layer-selective preview is stale.".to_owned())?;
        if pending.requested.ordered_timeline_angles().len() != 2
            || !pending.requested.continuous_certified()
        {
            return Err("Two certified layer-selective segments are required.".to_owned());
        }
    }
    let title = technique
        .names
        .first()
        .map(|text| text.text.clone())
        .ok_or_else(|| "The layer-selective title is unavailable.".to_owned())?;
    apply_stacked_fold_transaction_with_title(
        &app_state,
        &foldability_state,
        &transaction_state,
        token,
        Some(&title),
        None,
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
        None,
    )
}

fn apply_stacked_fold_transaction_with_title(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &StackedFoldTransactionState,
    token: ProjectId,
    named_title: Option<&str>,
    compiled_timeline: Option<ori_domain::InstructionTimeline>,
) -> Result<u64, String> {
    let _mode_guard = lock_transaction_mode_gate_v1(transaction_state)?;
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
    if let PendingStackedFoldRequestedPose::ThreeBlockCurrentCycle {
        geometry,
        audit,
        pose,
        decomposition,
        authority,
        common_pose_limits,
        schedule,
        schedule_limits,
        closure,
        clearance_limits,
        block_sources,
        source,
        thickness,
        issuer_context,
        articulation_layer_fingerprint,
        target_angles,
        layer_order_pairs,
        transition_count,
        ..
    } = &pending.requested
    {
        let capability = pending.layer_capability.as_ref().ok_or_else(|| {
            "The three-block current-cycle layer-order authority is unavailable.".to_owned()
        })?;
        if layer_guard.is_none()
            || source.as_ref() != capability.snapshot()
            || !revalidate_three_block_current_cycle_authority_v1(
                geometry,
                audit,
                pose,
                decomposition,
                authority,
                *common_pose_limits,
                schedule,
                *schedule_limits,
                closure,
                *clearance_limits,
                block_sources,
                source,
                *thickness,
                *issuer_context,
                *articulation_layer_fingerprint,
                target_angles,
                layer_order_pairs,
                *transition_count,
            )
        {
            return Err(
                "The three-block current-cycle layer transport preview is stale.".to_owned(),
            );
        }
    }

    let requested = &pending.requested;
    let target = requested.geometry();
    if let (Some(target), Some(CurrentLayerEvidence::CertifiedFlat(snapshot))) =
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
    let existing_timeline_step_count = timeline.steps.len();
    let certified_path_angles = requested.ordered_timeline_angles();
    let persisted_layer_proof = requested.persisted_cycle_layer_order_proof();
    if certified_path_angles.is_empty()
        || (named_title.is_some() && certified_path_angles.len() > 31)
    {
        return Err("The certified path timeline is inconsistent.".to_owned());
    }
    // SIM-010 is one user operation and therefore exactly one timeline step.
    // Named multi-stage techniques retain their separately validated segments.
    let ordered_timeline_angles = if named_title.is_none() {
        vec![
            certified_path_angles
                .last()
                .expect("non-empty certified path")
                .clone(),
        ]
    } else {
        certified_path_angles
    };
    let source_model_fingerprint = target.map_or_else(
        || project.editor.fold_model_fingerprint_v1(),
        |target| target.proof().lineage().target_fingerprint().to_hex(),
    );
    let certified_graph_path = requested.certified_graph_path();
    if let (Some(title), Some(source_angles), Some(_)) = (
        named_title,
        requested.certified_graph_source_angles(),
        certified_graph_path,
    ) {
        timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: format!("「{title}」の開始姿勢"),
            description: "構造化証明の始点姿勢です。".to_owned(),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: source_model_fingerprint.clone(),
                fixed_face,
                hinge_angles: source_angles
                    .into_iter()
                    .map(|(edge, angle_degrees)| InstructionHingeAngle {
                        edge,
                        angle_degrees,
                    })
                    .collect(),
            },
        });
    }
    for (index, step_angles) in ordered_timeline_angles.into_iter().enumerate() {
        let path_reference = certified_graph_path.and_then(|path| {
            let edge = path.edges().get(index)?;
            let mut model_hash = Sha256::new();
            model_hash.update(b"path_certificate_source_model_binding_v1");
            model_hash.update(source_model_fingerprint.as_bytes());
            Some(ori_domain::PathCertificateReferenceV1 {
                version: 1,
                model_id: ori_domain::PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
                binding_sha256: path.binding_fingerprint_v1(),
                source_pose_sha256: edge.source(),
                target_pose_sha256: edge.target(),
                source_model_binding_sha256: model_hash.finalize().into(),
                transition_count: path.edges().len(),
            })
        });
        let description = match (named_title, path_reference.as_ref()) {
            (Some(title), Some(reference)) => format!(
                "認証済みの連続折り経路で名前付き技法「{title}」を適用します。経路証明 SHA-256: {} / 元モデル SHA-256: {source_model_fingerprint}",
                lowercase_hex(reference.binding_sha256),
            ),
            (Some(title), None) => format!(
                "証明参照のない名前付き技法「{title}」の姿勢です。連続折り経路は未証明です。"
            ),
            (None, _) => String::new(),
        };
        timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: if index == 0 {
                named_title.unwrap_or("Stacked fold").to_owned()
            } else {
                format!("{} {}", named_title.unwrap_or("Stacked fold"), index + 1)
            },
            description,
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual {
                cycle_layer_order_proof_v1: persisted_layer_proof.clone(),
                path_certificate_reference_v1: path_reference,
                ..InstructionVisual::default()
            },
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: source_model_fingerprint.clone(),
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
    if let Some(compiled_timeline) = compiled_timeline {
        timeline.steps.truncate(existing_timeline_step_count);
        timeline.steps.extend(compiled_timeline.steps);
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
    // Keep an exact rollback image until both target authorities have been
    // installed. This includes history/dirty state and every saved baseline;
    // pose authority and its pair-cache epoch use a separate one-shot guard.
    let mut project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let layer_before = layer_guard
        .as_ref()
        .map(CurrentLayerOrderCommitGuard::capture_rollback_snapshot_v1);
    let persisted_current_pose = timeline
        .steps
        .last()
        .map(|step| step.pose.clone())
        .ok_or_else(|| "The certified path timeline is inconsistent.".to_owned())?;
    if let Some(CurrentLayerEvidence::NonFlat(proof)) = &applied_layer_order {
        let expected_target_revision =
            pending.expected_revision.checked_add(1).ok_or_else(|| {
                "The non-flat target layer authority cannot advance its revision.".to_owned()
            })?;
        let target_fingerprint = fold_model_fingerprint_v1(&candidate_pattern, &candidate_paper);
        let pose_angles_match = proof.hinge_angles().len()
            == persisted_current_pose.hinge_angles.len()
            && proof
                .hinge_angles()
                .iter()
                .zip(&persisted_current_pose.hinge_angles)
                .all(|(sealed, persisted)| {
                    sealed.edge() == persisted.edge
                        && sealed.angle_degrees().to_bits() == persisted.angle_degrees.to_bits()
                });
        if proof.identity_namespace() != project.project_id
            || proof.target_revision() != expected_target_revision
            || proof.target_fingerprint() != target_fingerprint
            || proof.fixed_face() != persisted_current_pose.fixed_face
            || !pose_angles_match
        {
            return Err("The non-flat target layer authority is stale or tampered.".to_owned());
        }
    }
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
    project.record_numeric_expression_edit();
    drop(_pose_guard);
    let mut pose_rollback = reissue_target_pose_or_rollback(
        &mut project,
        &persisted_current_pose,
        &mut project_before,
    )?;
    match (&applied_layer_order, layer_guard) {
        (Some(CurrentLayerEvidence::NonFlat(_)) | None, layer_guard) => {
            if let Some(mut layer_guard) = layer_guard {
                layer_guard.invalidate_after_project_mutation();
            }
        }
        (Some(CurrentLayerEvidence::CertifiedFlat(snapshot)), layer_guard) => {
            let mut layer_guard = layer_guard;
            let layer_install_succeeded = layer_guard.as_mut().is_some_and(|guard| {
                guard
                    .install_certified_target_after_project_mutation(&project, snapshot.clone())
                    .is_ok()
            });
            if !layer_install_succeeded {
                rollback_stacked_fold_apply_v1(
                    &mut project,
                    &mut project_before,
                    &mut pose_rollback,
                    layer_guard.as_mut(),
                    layer_before.as_ref(),
                )?;
                return Err(
                    "The certified target layer order could not be installed atomically."
                        .to_owned(),
                );
            }
        }
    }
    project.current_layer_evidence = match applied_layer_order.as_ref() {
        Some(CurrentLayerEvidence::NonFlat(_)) => applied_layer_order.clone(),
        _ if target.is_none() => applied_layer_order.clone(),
        _ => None,
    };
    pose_rollback.disarm();
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

pub(crate) struct StackedFoldProjectRollbackSnapshotV1 {
    state: Option<StackedFoldProjectRollbackStateV1>,
}

struct StackedFoldProjectRollbackStateV1 {
    editor: ori_core::EditorState,
    current_layer_evidence: Option<CurrentLayerEvidence>,
    numeric_expressions: super::ProjectNumericExpressions,
    saved_revision: Option<u64>,
    saved_document: Option<super::ProjectDocument>,
    saved_speculative_unproven_state: Option<ori_core::SpeculativeUnprovenFoldStateMarkerV1>,
}

pub(super) fn rollback_stacked_fold_apply_v1(
    project: &mut super::ProjectState,
    project_before: &mut StackedFoldProjectRollbackSnapshotV1,
    pose_rollback: &mut CurrentAppliedPoseTransactionRollbackV1,
    layer_guard: Option<&mut CurrentLayerOrderCommitGuard<'_>>,
    layer_before: Option<&CurrentLayerOrderRollbackSnapshotV1>,
) -> Result<(), String> {
    // Restore every independently held image even when the pose image has
    // already been consumed or is otherwise unavailable. Leaving the editor
    // or layer slot at the target after reporting rollback failure would
    // expose a mixed transaction. The origin-bound pose/cache image itself
    // restores in one finite operation; never retry while these locks are
    // held.
    let project_result = project_before.restore(project);
    let pose_result = pose_rollback.rollback();
    if let (Some(layer_guard), Some(layer_before)) = (layer_guard, layer_before) {
        layer_guard.restore_rollback_snapshot_v1(layer_before);
    }
    project_result
        .map_err(|_| "The stacked-fold project rollback image was already consumed.".to_owned())?;
    pose_result.map_err(|_| "The stacked-fold rollback image was already consumed.".to_owned())
}

impl StackedFoldProjectRollbackSnapshotV1 {
    pub(crate) fn capture(project: &super::ProjectState) -> Self {
        Self {
            state: Some(StackedFoldProjectRollbackStateV1 {
                editor: project.editor.clone(),
                current_layer_evidence: project.current_layer_evidence.clone(),
                numeric_expressions: project.numeric_expressions.clone(),
                saved_revision: project.saved_revision,
                saved_document: project.saved_document.clone(),
                saved_speculative_unproven_state: project.saved_speculative_unproven_state.clone(),
            }),
        }
    }

    pub(crate) fn restore(&mut self, project: &mut super::ProjectState) -> Result<(), ()> {
        let before = self.state.take().ok_or(())?;
        project.editor = before.editor;
        project.current_layer_evidence = before.current_layer_evidence;
        project.numeric_expressions = before.numeric_expressions;
        project.saved_revision = before.saved_revision;
        project.saved_document = before.saved_document;
        project.saved_speculative_unproven_state = before.saved_speculative_unproven_state;
        Ok(())
    }
}

fn reissue_target_pose_or_rollback(
    project: &mut super::ProjectState,
    persisted_current_pose: &InstructionPose,
    project_before: &mut StackedFoldProjectRollbackSnapshotV1,
) -> Result<CurrentAppliedPoseTransactionRollbackV1, String> {
    reissue_target_pose_or_rollback_with_v1(
        project,
        persisted_current_pose,
        project_before,
        restore_persisted_current_pose_transactional_v1,
    )
}

fn reissue_target_pose_or_rollback_with_v1(
    project: &mut super::ProjectState,
    persisted_current_pose: &InstructionPose,
    project_before: &mut StackedFoldProjectRollbackSnapshotV1,
    restore: impl FnOnce(
        &mut super::ProjectState,
        &InstructionPose,
    ) -> Result<
        CurrentAppliedPoseTransactionRollbackV1,
        super::applied_pose::PoseAuthorityError,
    >,
) -> Result<CurrentAppliedPoseTransactionRollbackV1, String> {
    if let Ok(rollback) = restore(project, persisted_current_pose) {
        return Ok(rollback);
    }
    project_before
        .restore(project)
        .map_err(|_| "The stacked-fold project rollback image was already consumed.".to_owned())?;
    Err("The target pose authority could not be installed atomically.".to_owned())
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
fn lock_recovering_poison_for_drop_v1<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let guard = poisoned.into_inner();
            mutex.clear_poison();
            guard
        }
    }
}

fn lock_transaction_mode_gate_v1(
    state: &StackedFoldTransactionState,
) -> Result<MutexGuard<'_, ()>, String> {
    state
        .2
        .lock()
        .map_err(|_| "The stacked-fold transaction mode registry is unavailable.".to_owned())
}

/// Publishes a current-cycle preview while its caller already owns the project
/// mutex.
///
/// Apply and named-preview compilation acquire transaction registries before
/// the project mutex. A project-held installer must therefore never wait for
/// any of those registries. All three guards are acquired with `try_lock`
/// before the callback may mutate either pending slot.
fn with_project_guarded_current_cycle_install_slots_v1<R>(
    state: &StackedFoldTransactionState,
    source_revision: u64,
    action: impl FnOnce(
        &mut StackedFoldTransactionSlot,
        &mut speculative_unproven::SpeculativeStackedFoldTransactionSlotV1,
    ) -> R,
) -> Result<R, String> {
    if next_current_cycle_target_revision_v1(source_revision).is_none() {
        return Err("The current-cycle transaction cannot advance its revision.".to_owned());
    }
    with_try_locked_transaction_install_slots_v1(state, action)
}

fn with_try_locked_transaction_install_slots_v1<R>(
    state: &StackedFoldTransactionState,
    action: impl FnOnce(
        &mut StackedFoldTransactionSlot,
        &mut speculative_unproven::SpeculativeStackedFoldTransactionSlotV1,
    ) -> R,
) -> Result<R, String> {
    let _mode_guard = state
        .2
        .try_lock()
        .map_err(|_| "The stacked-fold transaction mode registry is unavailable.".to_owned())?;
    let mut certified_slot = state
        .0
        .try_lock()
        .map_err(|_| "The stacked-fold transaction registry is unavailable.".to_owned())?;
    let mut speculative_slot = state
        .1
        .try_lock()
        .map_err(|_| "The speculative stacked-fold registry is unavailable.".to_owned())?;
    Ok(action(&mut certified_slot, &mut speculative_slot))
}

fn clear_pending_certified_slot_locked_v1(slot: &mut StackedFoldTransactionSlot) {
    slot.active_generation = None;
    slot.pending = None;
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
    fn current_cycle_target_revision_stops_at_the_editor_limit() {
        assert_eq!(
            next_current_cycle_target_revision_v1(ori_core::MAX_REVISION - 1),
            Some(ori_core::MAX_REVISION)
        );
        assert_eq!(
            next_current_cycle_target_revision_v1(ori_core::MAX_REVISION),
            None
        );
        assert_eq!(
            next_current_cycle_target_revision_v1(ori_core::MAX_REVISION + 1),
            None
        );
        assert_eq!(next_current_cycle_target_revision_v1(u64::MAX), None);
    }

    #[test]
    fn three_block_persisted_summary_rejects_count_pair_and_empty_list_tampering() {
        let expected = [
            (ori_domain::FaceId::new(), ori_domain::FaceId::new()),
            (ori_domain::FaceId::new(), ori_domain::FaceId::new()),
        ];
        let transition_count = 9;
        assert!(exact_three_block_summary_fields_match_v1(
            &expected,
            transition_count,
            &expected,
            transition_count,
        ));
        assert!(!exact_three_block_summary_fields_match_v1(
            &expected,
            transition_count,
            &expected,
            transition_count + 1,
        ));

        let mut changed_pair = expected;
        changed_pair[0].1 = ori_domain::FaceId::new();
        assert!(!exact_three_block_summary_fields_match_v1(
            &expected,
            transition_count,
            &changed_pair,
            transition_count,
        ));
        assert!(!exact_three_block_summary_fields_match_v1(
            &expected,
            transition_count,
            &[],
            transition_count,
        ));
    }

    #[test]
    fn project_guarded_current_cycle_install_holds_all_three_registry_guards() {
        let state = StackedFoldTransactionState::default();
        let old_certified = ProjectId::new();
        let old_speculative = ProjectId::new();
        let replacement = ProjectId::new();
        state.set_installed_guard_tokens_for_test_v1(Some(old_certified), Some(old_speculative));

        let installed = with_project_guarded_current_cycle_install_slots_v1(
            &state,
            ori_core::MAX_REVISION - 1,
            |certified_slot, speculative_slot| {
                assert!(state.2.try_lock().is_err(), "mode gate was not retained");
                assert!(
                    state.0.try_lock().is_err(),
                    "certified slot was not retained"
                );
                assert!(
                    state.1.try_lock().is_err(),
                    "speculative slot was not retained"
                );
                assert_eq!(certified_slot.active_generation, Some(old_certified));
                assert_eq!(speculative_slot.active_generation, Some(old_speculative));

                speculative_unproven::clear_pending_speculative_slot_locked_v1(speculative_slot);
                certified_slot.active_generation = Some(replacement);
                replacement
            },
        )
        .expect("all project-held install locks are free");

        assert_eq!(installed, replacement);
        assert_eq!(
            state.installed_guard_tokens_for_test_v1(),
            (Some(replacement), None)
        );
    }

    #[test]
    fn project_guarded_current_cycle_install_contention_is_atomic() {
        fn fixture() -> (StackedFoldTransactionState, ProjectId, ProjectId) {
            let state = StackedFoldTransactionState::default();
            let certified = ProjectId::new();
            let speculative = ProjectId::new();
            state.set_installed_guard_tokens_for_test_v1(Some(certified), Some(speculative));
            (state, certified, speculative)
        }

        let exercise = |state: &StackedFoldTransactionState| {
            let callback_called = std::cell::Cell::new(false);
            assert!(
                with_project_guarded_current_cycle_install_slots_v1(state, 0, |_, _| {
                    callback_called.set(true)
                },)
                .is_err()
            );
            assert!(!callback_called.get());
        };

        let (state, certified, speculative) = fixture();
        let held = state.2.lock().expect("mode gate");
        exercise(&state);
        drop(held);
        assert_eq!(
            state.installed_guard_tokens_for_test_v1(),
            (Some(certified), Some(speculative))
        );

        let (state, certified, speculative) = fixture();
        let held = state.0.lock().expect("certified slot");
        exercise(&state);
        drop(held);
        assert_eq!(
            state.installed_guard_tokens_for_test_v1(),
            (Some(certified), Some(speculative))
        );

        let (state, certified, speculative) = fixture();
        let held = state.1.lock().expect("speculative slot");
        exercise(&state);
        drop(held);
        assert_eq!(
            state.installed_guard_tokens_for_test_v1(),
            (Some(certified), Some(speculative))
        );

        let (state, certified, speculative) = fixture();
        let callback_called = std::cell::Cell::new(false);
        assert!(
            with_project_guarded_current_cycle_install_slots_v1(
                &state,
                ori_core::MAX_REVISION,
                |_, _| callback_called.set(true),
            )
            .is_err()
        );
        assert!(!callback_called.get());
        assert_eq!(
            state.installed_guard_tokens_for_test_v1(),
            (Some(certified), Some(speculative))
        );
    }

    #[test]
    fn project_guarded_current_cycle_install_poison_is_atomic() {
        fn poison<T>(mutex: &Mutex<T>) {
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = mutex.lock().unwrap_or_else(|error| error.into_inner());
                panic!("poison current-cycle install registry");
            }));
            assert!(unwind.is_err());
            assert!(mutex.is_poisoned());
        }

        fn fixture() -> (StackedFoldTransactionState, ProjectId, ProjectId) {
            let state = StackedFoldTransactionState::default();
            let certified = ProjectId::new();
            let speculative = ProjectId::new();
            state.set_installed_guard_tokens_for_test_v1(Some(certified), Some(speculative));
            (state, certified, speculative)
        }

        fn recovered_tokens(
            state: &StackedFoldTransactionState,
        ) -> (Option<ProjectId>, Option<ProjectId>) {
            let certified = {
                let slot = lock_recovering_poison_for_drop_v1(&state.0);
                slot.active_generation
            };
            let speculative = {
                let slot = lock_recovering_poison_for_drop_v1(&state.1);
                slot.active_generation
            };
            (certified, speculative)
        }

        let exercise = |state: &StackedFoldTransactionState| {
            let callback_called = std::cell::Cell::new(false);
            assert!(
                with_project_guarded_current_cycle_install_slots_v1(state, 0, |_, _| {
                    callback_called.set(true)
                },)
                .is_err()
            );
            assert!(!callback_called.get());
        };

        let (state, certified, speculative) = fixture();
        poison(&state.2);
        exercise(&state);
        assert_eq!(
            recovered_tokens(&state),
            (Some(certified), Some(speculative))
        );

        let (state, certified, speculative) = fixture();
        poison(&state.0);
        exercise(&state);
        assert_eq!(
            recovered_tokens(&state),
            (Some(certified), Some(speculative))
        );

        let (state, certified, speculative) = fixture();
        poison(&state.1);
        exercise(&state);
        assert_eq!(
            recovered_tokens(&state),
            (Some(certified), Some(speculative))
        );
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

    #[test]
    fn basic_fold_preview_dto_never_serializes_raw_certificate_authority() {
        let response = BasicFoldTimelinePreviewResponse {
            schema_version: 1,
            transaction_token: ProjectId::new(),
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision: 7,
            source_model_fingerprint: "ab".repeat(32),
            fixed_face: ori_domain::FaceId::new(),
            fold_edge: ori_domain::EdgeId::new(),
            assignment: "mountain".to_owned(),
            technique_kind: "mountain".to_owned(),
            preview_binding_sha256: "cd".repeat(32),
            timeline: ori_domain::InstructionTimeline { steps: Vec::new() },
        };
        let value = serde_json::to_value(response).expect("serialize read-only preview");
        let object = value.as_object().expect("preview object");
        assert_eq!(
            object
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            [
                "assignment",
                "fixedFace",
                "foldEdge",
                "previewBindingSha256",
                "projectId",
                "projectInstanceId",
                "revision",
                "schemaVersion",
                "sourceModelFingerprint",
                "techniqueKind",
                "timeline",
                "transactionToken",
            ]
            .into_iter()
            .collect()
        );
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(!serialized.contains("certificate"));
        assert!(!serialized.contains("authorizesProjectMutation"));
    }

    #[test]
    fn basic_fold_preview_binding_covers_authority_and_timeline() {
        let token = ProjectId::new();
        let instance = ProjectId::new();
        let project = ProjectId::new();
        let timeline = ori_domain::InstructionTimeline { steps: Vec::new() };
        let expected =
            basic_fold_preview_binding_v1(token, instance, project, 7, "mountain", &timeline)
                .unwrap();
        assert_eq!(expected.len(), 64);
        assert_eq!(
            expected,
            basic_fold_preview_binding_v1(token, instance, project, 7, "mountain", &timeline)
                .unwrap()
        );
        assert_ne!(
            expected,
            basic_fold_preview_binding_v1(
                ProjectId::new(),
                instance,
                project,
                7,
                "mountain",
                &timeline
            )
            .unwrap()
        );
        assert_ne!(
            expected,
            basic_fold_preview_binding_v1(token, instance, project, 8, "mountain", &timeline)
                .unwrap()
        );
    }

    #[test]
    fn accordion_assignment_chain_is_bounded_and_strictly_alternating() {
        assert!(accordion_assignments_alternate_v1(&[
            "mountain", "valley", "mountain"
        ]));
        assert!(!accordion_assignments_alternate_v1(&[
            "mountain", "mountain", "valley"
        ]));
        assert!(!accordion_assignments_alternate_v1(&["mountain", "valley"]));
        assert!(!accordion_assignments_alternate_v1(&vec!["mountain"; 32]));
    }
}

#[cfg(test)]
mod rollback_tests;
