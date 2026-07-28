//! Strict wire contracts and pure admission helpers for stacked-fold reads.
//!
//! Native geometry analysis, proof construction, progress emission, and
//! transaction installation remain in the parent module.

use ori_collision::{
    StackedFoldFixedSideV1, StackedFoldReadSupportV1, StackedFoldRotationDirectionV1,
};
use ori_domain::{FaceId, ProjectId};
use serde::{Deserialize, Serialize};

use super::{
    CYCLE_PATH_RESOURCE_MESSAGE, CycleScheduleRequestV1, LiveGraphHingeAngleDto,
    MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1, MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1,
    MAX_STACKED_FOLD_REQUEST_HINGES_V1,
    stacked_fold_cycle_pose_wire::{CertifiedPathGraphRequestV1, LinearCandidateRequestV1},
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FixedSideRequest {
    Left,
    Right,
}

impl From<FixedSideRequest> for StackedFoldFixedSideV1 {
    fn from(value: FixedSideRequest) -> Self {
        match value {
            FixedSideRequest::Left => Self::Left,
            FixedSideRequest::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RotationDirectionRequest {
    Positive,
    Negative,
}

impl From<RotationDirectionRequest> for StackedFoldRotationDirectionV1 {
    fn from(value: RotationDirectionRequest) -> Self {
        match value {
            RotationDirectionRequest::Positive => Self::Positive,
            RotationDirectionRequest::Negative => Self::Negative,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StackedFoldReadRequest {
    #[serde(default)]
    pub(super) progress_request_id: Option<String>,
    pub(super) expected_project_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) first: [f64; 3],
    pub(super) second: [f64; 3],
    pub(super) fixed_side: FixedSideRequest,
    pub(super) rotation_direction: RotationDirectionRequest,
    pub(super) requested_angle_degrees: f64,
    #[serde(default)]
    pub(super) cycle_schedule_v1: Option<CycleScheduleRequestV1>,
    #[serde(default)]
    pub(super) linear_candidate_v1: Option<LinearCandidateRequestV1>,
    #[serde(default)]
    pub(super) certified_path_graph_v1: Option<CertifiedPathGraphRequestV1>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldReadProgressDtoV1 {
    pub(super) version: u32,
    pub(super) request_id: String,
    pub(super) explored_state_count: usize,
    pub(super) evaluated_transition_count: usize,
    pub(super) state_limit: usize,
    pub(super) transition_limit: usize,
    pub(super) authorizes_project_mutation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentCyclePoseProgressDtoV1 {
    pub(super) version: u32,
    pub(super) request_id: String,
    pub(super) status: &'static str,
    pub(super) completed_work: usize,
    pub(super) total_work: usize,
    pub(super) authorizes_project_mutation: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DyadicPoseGraphAngleDtoV1 {
    pub(super) edge: ori_domain::EdgeId,
    pub(super) angle_degrees: f64,
}

pub(super) fn validate_request_resource_shape_v1(
    request: &StackedFoldReadRequest,
) -> Result<(), &'static str> {
    if request
        .linear_candidate_v1
        .as_ref()
        .is_some_and(|candidate| {
            candidate.entries.is_empty()
                || candidate.entries.len() > MAX_STACKED_FOLD_REQUEST_HINGES_V1
        })
        || request
            .certified_path_graph_v1
            .as_ref()
            .is_some_and(|graph| {
                graph.states.is_empty()
                    || graph.states.len() > ori_collision::MAX_CERTIFIED_PATH_GRAPH_STATES_V1
                    || graph.transitions.is_empty()
                    || graph.transitions.len() > MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1
                    || graph.states.iter().any(|state| {
                        state.entries.is_empty()
                            || state.entries.len() > MAX_STACKED_FOLD_REQUEST_HINGES_V1
                    })
            })
        || request.cycle_schedule_v1.as_ref().is_some_and(|schedule| {
            schedule.entries.is_empty()
                || schedule.entries.len() > MAX_STACKED_FOLD_REQUEST_HINGES_V1
                || schedule.entries.iter().any(|entry| {
                    entry.numerator_power_coefficients.is_empty()
                        || entry.numerator_power_coefficients.len()
                            > MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1
                        || entry.denominator_power_coefficients.is_empty()
                        || entry.denominator_power_coefficients.len()
                            > MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1
                })
        })
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE);
    }
    Ok(())
}

pub(super) fn requires_graph_schedule_boundary_v1(
    topology_requires_closure: bool,
    has_explicit_cycle_schedule: bool,
) -> bool {
    topology_requires_closure || has_explicit_cycle_schedule
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StackedFoldReadSupportDto {
    NoHingeSingleFace,
    BitExactFlatEndpointTree,
}

impl From<StackedFoldReadSupportV1> for StackedFoldReadSupportDto {
    fn from(value: StackedFoldReadSupportV1) -> Self {
        match value {
            StackedFoldReadSupportV1::NoHingeSingleFace => Self::NoHingeSingleFace,
            StackedFoldReadSupportV1::BitExactFlatEndpointTree => Self::BitExactFlatEndpointTree,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldReadBindingDto {
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) source_revision: u64,
    pub(super) pose_generation: u64,
    pub(super) layer_order_generation: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldReadCellDto {
    pub(super) cell_key_sha256: String,
    pub(super) bottom_to_top_faces: Vec<FaceId>,
    pub(super) boundary_world: Vec<[f64; 3]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldMaterialSegmentDto {
    pub(super) face_id: FaceId,
    pub(super) start: [f64; 2],
    pub(super) end: [f64; 2],
    pub(super) fixed_side: &'static str,
    pub(super) assignment: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldReadWorkDto {
    pub(super) scanned_cells: usize,
    pub(super) total_boundary_vertices: usize,
    pub(super) total_layer_records: usize,
    pub(super) orientation_tests: usize,
    pub(super) exact_arithmetic_operations: usize,
    pub(super) maximum_exact_integer_bits: usize,
    pub(super) total_exact_integer_bits: usize,
    pub(super) retained_cells: usize,
    pub(super) retained_target_faces: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldTopologyProofDto {
    pub(super) target_fingerprint_sha256: String,
    pub(super) target_vertex_count: usize,
    pub(super) target_edge_count: usize,
    pub(super) target_boundary_vertex_count: usize,
    pub(super) lineage_record_count: usize,
    pub(super) source_edge_subdivision_count: usize,
    pub(super) expected_crease_subdivision_count: usize,
    pub(super) target_material_face_count: usize,
    pub(super) target_hinge_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldEndpointCollisionDto {
    pub(super) expected_pair_count: usize,
    pub(super) separated_pair_count: usize,
    pub(super) touching_pair_count: usize,
    pub(super) allowed_pair_count: usize,
    pub(super) penetrating_pair_count: usize,
    pub(super) indeterminate_pair_count: usize,
    pub(super) has_blocking_hold: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldContinuousPathDto {
    pub(super) model_id: &'static str,
    pub(super) continuous_certificate_model_id: Option<&'static str>,
    pub(super) sampled_pose_count: usize,
    pub(super) sampled_nonblocking_pose_count: usize,
    pub(super) interval_leaf_count: usize,
    pub(super) interval_pair_work: usize,
    pub(super) interval_candidate_limit: usize,
    pub(super) positive_endpoint_candidate_count: usize,
    pub(super) positive_endpoint_exact_pair_calls: usize,
    pub(super) positive_endpoint_candidate_limit: usize,
    pub(super) closure_required: bool,
    pub(super) closure_leaf_count: usize,
    pub(super) closure_pair_work: usize,
    pub(super) first_closure_failure_angle_degrees: Option<f64>,
    pub(super) first_sampled_blocking_angle_degrees: Option<f64>,
    pub(super) requested_angle_degrees: f64,
    pub(super) continuous_clearance_certified: bool,
    pub(super) safe_stop_angle_degrees: f64,
    pub(super) authorizes_project_mutation: bool,
    pub(super) paper_thickness_mm: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CertifiedPathGraphPreviewDto {
    pub(super) model_id: &'static str,
    pub(super) version: u32,
    pub(super) source_fingerprint_sha256: String,
    pub(super) target_fingerprint_sha256: String,
    pub(super) explored_state_count: usize,
    pub(super) evaluated_transition_count: usize,
    pub(super) edges: Vec<CertifiedPathGraphEdgeDto>,
    pub(super) authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CertifiedPathGraphEdgeDto {
    pub(super) source_fingerprint_sha256: String,
    pub(super) target_fingerprint_sha256: String,
    pub(super) schedule_certificate_sha256: String,
    pub(super) collision_certificate_sha256: String,
    pub(super) closure_certificate_sha256: String,
    pub(super) hinges: Vec<ori_domain::EdgeId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldFlatEndpointLayerOrderDto {
    pub(super) applicable: bool,
    pub(super) certified: bool,
    pub(super) material_face_count: usize,
    pub(super) overlap_cell_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StackedFoldTransactionFailureClassDto {
    ContinuousPathUncertified,
    TargetLayerOrderUnavailable,
}

pub(super) const STACKED_FOLD_APPLY_CONTRACT_VERSION_V1: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StackedFoldApplyModeDtoV1 {
    None,
    Certified,
    SpeculativeUnproven,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldTransactionProposalDto {
    pub(super) apply_contract_version: u8,
    pub(super) apply_mode: StackedFoldApplyModeDtoV1,
    pub(super) transaction_token: Option<ProjectId>,
    pub(super) speculative_unproven_available: bool,
    pub(super) source_project_id: ProjectId,
    pub(super) source_revision: u64,
    pub(super) target_revision: u64,
    pub(super) source_fingerprint_sha256: String,
    pub(super) target_fingerprint_sha256: String,
    pub(super) added_vertex_count: usize,
    pub(super) added_edge_count: usize,
    pub(super) mountain_crease_count: usize,
    pub(super) valley_crease_count: usize,
    pub(super) timeline_step_count: usize,
    pub(super) timeline_complete_hinge_angle_count: usize,
    pub(super) requested_angle_degrees: f64,
    pub(super) ready_for_atomic_apply: bool,
    pub(super) failure_classes: Vec<StackedFoldTransactionFailureClassDto>,
    pub(super) authorizes_project_mutation: bool,
}

impl StackedFoldTransactionProposalDto {
    pub(super) fn publish_certified_v1(&mut self, token: ProjectId) {
        self.apply_mode = StackedFoldApplyModeDtoV1::Certified;
        self.transaction_token = Some(token);
        self.speculative_unproven_available = false;
        self.ready_for_atomic_apply = true;
        self.authorizes_project_mutation = true;
        self.failure_classes.clear();
        debug_assert!(self.has_valid_apply_contract_v1(true, true));
    }

    pub(super) fn publish_speculative_unproven_v1(&mut self, token: ProjectId) {
        self.apply_mode = StackedFoldApplyModeDtoV1::SpeculativeUnproven;
        self.transaction_token = Some(token);
        self.speculative_unproven_available = true;
        self.ready_for_atomic_apply = false;
        self.authorizes_project_mutation = false;
        self.failure_classes =
            vec![StackedFoldTransactionFailureClassDto::ContinuousPathUncertified];
        debug_assert!(self.has_valid_apply_contract_v1(false, true));
    }

    #[must_use]
    pub(super) fn has_valid_apply_contract_v1(
        &self,
        continuous_path_certified: bool,
        target_layer_order_certified: bool,
    ) -> bool {
        self.apply_contract_version == STACKED_FOLD_APPLY_CONTRACT_VERSION_V1
            && self.failure_classes
                == transaction_failure_classes(
                    continuous_path_certified,
                    target_layer_order_certified,
                )
            && match self.apply_mode {
                StackedFoldApplyModeDtoV1::None => {
                    self.transaction_token.is_none()
                        && !self.speculative_unproven_available
                        && !self.ready_for_atomic_apply
                        && !self.authorizes_project_mutation
                }
                StackedFoldApplyModeDtoV1::Certified => {
                    self.transaction_token.is_some()
                        && !self.speculative_unproven_available
                        && self.ready_for_atomic_apply
                        && self.authorizes_project_mutation
                        && self.failure_classes.is_empty()
                }
                StackedFoldApplyModeDtoV1::SpeculativeUnproven => {
                    self.transaction_token.is_some()
                        && self.speculative_unproven_available
                        && !self.ready_for_atomic_apply
                        && !self.authorizes_project_mutation
                        && self.failure_classes.as_slice()
                            == [StackedFoldTransactionFailureClassDto::ContinuousPathUncertified]
                }
            }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackedFoldReadResponse {
    pub(super) guard_model_id: &'static str,
    pub(super) proposal_model_id: &'static str,
    pub(super) material_map_model_id: &'static str,
    pub(super) binding: StackedFoldReadBindingDto,
    pub(super) support: StackedFoldReadSupportDto,
    pub(super) crossed_cells: Vec<StackedFoldReadCellDto>,
    pub(super) target_faces: Vec<FaceId>,
    pub(super) material_segments: Vec<StackedFoldMaterialSegmentDto>,
    pub(super) topology_proof: StackedFoldTopologyProofDto,
    pub(super) live_graph_hinge_angles: Vec<LiveGraphHingeAngleDto>,
    pub(super) endpoint_collision: StackedFoldEndpointCollisionDto,
    pub(super) continuous_path: StackedFoldContinuousPathDto,
    pub(super) certified_path_graph: Option<CertifiedPathGraphPreviewDto>,
    pub(super) flat_endpoint_layer_order: StackedFoldFlatEndpointLayerOrderDto,
    pub(super) transaction_proposal: StackedFoldTransactionProposalDto,
    pub(super) work: StackedFoldReadWorkDto,
    pub(super) authorizes_project_mutation: bool,
    pub(super) authorizes_apply_stacked_fold: bool,
}

pub(super) fn transaction_failure_classes(
    continuous_path_certified: bool,
    target_layer_order_certified: bool,
) -> Vec<StackedFoldTransactionFailureClassDto> {
    let mut failures = Vec::new();
    if !continuous_path_certified {
        failures.push(StackedFoldTransactionFailureClassDto::ContinuousPathUncertified);
    }
    if !target_layer_order_certified {
        failures.push(StackedFoldTransactionFailureClassDto::TargetLayerOrderUnavailable);
    }
    failures
}

#[cfg(test)]
#[path = "stacked_fold_read_wire_tests.rs"]
mod tests;
