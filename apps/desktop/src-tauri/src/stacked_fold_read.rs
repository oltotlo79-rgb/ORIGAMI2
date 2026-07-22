//! Read-only desktop bridge for the first authenticated SIM-010 boundary.
//!
//! Read responses never authorize mutation. The dyadic one-shot apply boundary
//! retains private native capabilities and revalidates them under both live
//! authority guards before one atomic Editor command is committed.

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

use ori_collision::{
    FlatEndpointLayerOrderInputV1, GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
    StackedFoldFixedSideV1, StackedFoldLinearCandidateV1, StackedFoldMaterialMapLimitsV1,
    StackedFoldPathDiagnosticLimitsV1, StackedFoldReadBindingV1, StackedFoldReadLimitsV1,
    StackedFoldReadSupportV1, StackedFoldRotationDirectionV1, StaticCollisionLimits,
    capture_stacked_fold_read_guard_v1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
    certify_general_multi_face_cell_transport_v1, diagnose_collective_hinge_path_v1,
    diagnose_scheduled_cycle_path_v1, diagnose_scheduled_positive_thickness_cycle_path_v1,
    diagnose_static_collision_geometry, propose_linear_stacked_fold_read_v1,
    reverse_map_linear_stacked_fold_material_v1, supports_scheduled_positive_thickness_path_v1,
};
use ori_core::{
    AppliedPoseLimitsV1, DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS, ExpectedStackedFoldCreaseV1,
    FaceLineageLimits, StackedFoldGeometryLimitsV1, StackedFoldTopologyBuildLimitsV1,
    analyze_global_flat_foldability, analyze_local_flat_foldability,
    prepare_closed_graph_applied_pose_v1, prepare_stacked_fold_geometry_candidate_v1,
    prepare_stacked_fold_graph_non_flat_layer_order_v1, prepare_stacked_fold_initial_graph_pose_v1,
    prepare_stacked_fold_initial_pose_v1,
    prepare_stacked_fold_non_flat_layer_order_with_thickness_v1,
    prepare_stacked_fold_requested_pose_v1, prepare_stacked_fold_target_graph_audit_v1,
    prepare_stacked_fold_target_model_v1,
};
use ori_domain::{FaceId, InstructionHingeAngle, ProjectId};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityOutcome,
};
use ori_kinematics::{
    CycleBasisLimitsV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    MultiHingePathCandidateLimitsV1, Point3, TreeKinematicsLimits,
    generate_linear_multi_hinge_path_candidate_v1,
};
use ori_topology::{FaceExtractionInput, TopologyIssueSeverity, analyze_faces};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use super::{
    AppState,
    applied_pose::{
        CurrentAppliedPoseCapability, lock_revalidated_current_applied_pose_for_commit,
        restore_persisted_current_pose,
    },
    global_flat_foldability::{
        CurrentLayerOrderCapability, GlobalFlatFoldabilityState,
        capture_current_layer_order_capability, lock_revalidated_current_layer_order_for_commit,
        revalidate_current_layer_order_capability,
    },
    lock_project,
};

const UNAVAILABLE_MESSAGE: &str =
    "The current pose and certified layer order cannot prepare a stacked-fold proposal.";
const INVALID_REQUEST_MESSAGE: &str = "The stacked-fold line request is invalid.";
const ANALYSIS_FAILED_MESSAGE: &str =
    "The stacked-fold proposal is unsupported or could not be certified.";
const CYCLE_NONCLOSING_MESSAGE: &str = "stacked_fold_cycle_nonclosing";
const CYCLE_PATH_UNCERTIFIED_MESSAGE: &str = "stacked_fold_cycle_path_uncertified";
const CYCLE_PATH_UNSUPPORTED_MESSAGE: &str = "stacked_fold_cycle_path_unsupported";
const CYCLE_PATH_RESOURCE_MESSAGE: &str = "stacked_fold_cycle_path_resource_limit";
const CYCLE_PATH_NO_CERTIFIED_PATH_MESSAGE: &str = "stacked_fold_cycle_path_no_certified_path";
const BUSY_MESSAGE: &str = "Another native pose analysis is already running.";
const STALE_MESSAGE: &str =
    "The project, current pose, or certified layer order changed during analysis.";
const CANCELLED_MESSAGE: &str = "stacked_fold_cycle_path_cancelled";
const MAX_STACKED_FOLD_REQUEST_HINGES_V1: usize = 64;
const MAX_DYADIC_GRAPH_STATES_V1: usize = 2_187;
const MAX_DYADIC_GRAPH_TRANSITIONS_V1: usize = 20_412;

fn dyadic_request_hinge_counts_are_bounded_v1(
    target_angle_count: usize,
    cycle_schedule_entry_count: Option<usize>,
) -> bool {
    target_angle_count > 0
        && target_angle_count <= MAX_STACKED_FOLD_REQUEST_HINGES_V1
        && cycle_schedule_entry_count
            .is_none_or(|count| count > 0 && count <= MAX_STACKED_FOLD_REQUEST_HINGES_V1)
}
const MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1: usize = 9;
// A certified path is committed as one editor transaction. Keep the request
// boundary aligned with the editor's bounded multi-step transaction admission.
const MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1: usize = 31;
static STACKED_FOLD_READ_GENERATION: AtomicU64 = AtomicU64::new(0);
const STACKED_FOLD_READ_PROGRESS_EVENT_V1: &str = "stacked-fold-read-progress-v1";
const CURRENT_CYCLE_POSE_PROGRESS_EVENT_V1: &str = "current-cycle-pose-progress-v1";

#[tauri::command]
pub(super) fn cancel_current_stacked_fold_read_v1() -> Result<(), String> {
    STACKED_FOLD_READ_GENERATION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
            generation.checked_add(1)
        })
        .map(|_| ())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixedSideRequest {
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
enum RotationDirectionRequest {
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
pub(super) struct StackedFoldReadRequest {
    #[serde(default)]
    progress_request_id: Option<String>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first: [f64; 3],
    second: [f64; 3],
    fixed_side: FixedSideRequest,
    rotation_direction: RotationDirectionRequest,
    requested_angle_degrees: f64,
    #[serde(default)]
    cycle_schedule_v1: Option<CycleScheduleRequestV1>,
    #[serde(default)]
    linear_candidate_v1: Option<LinearCandidateRequestV1>,
    #[serde(default)]
    certified_path_graph_v1: Option<CertifiedPathGraphRequestV1>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadProgressDtoV1 {
    version: u32,
    request_id: String,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    state_limit: usize,
    transition_limit: usize,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentCyclePoseProgressDtoV1 {
    version: u32,
    request_id: String,
    status: &'static str,
    completed_work: usize,
    total_work: usize,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LiveHingeRegistryRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first: [f64; 3],
    second: [f64; 3],
    fixed_side: FixedSideRequest,
    rotation_direction: RotationDirectionRequest,
    requested_angle_degrees: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveHingeRegistryResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    pose_generation: u64,
    graph_fingerprint_sha256: String,
    entries: Vec<LiveGraphHingeAngleDto>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EvenCycleCandidatesRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    max_pair_tests: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EvenCycleCandidatesResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    status: &'static str,
    reason: &'static str,
    candidates: Vec<EvenCycleCandidateDtoV1>,
    kawasaki_endpoints: Vec<KawasakiEndpointCandidateDtoV1>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KawasakiEndpointCandidateDtoV1 {
    version: u32,
    endpoint_denominator: u64,
    closure_status: &'static str,
    collision_status: &'static str,
    authorizes_apply: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvenCycleCandidateDtoV1 {
    version: u32,
    edges: [ori_domain::EdgeId; 2],
    reason: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DyadicPoseGraphReadRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    target_angles: Vec<DyadicPoseGraphAngleDtoV1>,
    max_states: usize,
    max_transitions: usize,
    #[serde(default = "default_dyadic_level_count_v1")]
    level_count: usize,
    #[serde(default)]
    cycle_schedule_v1: Option<CycleScheduleRequestV1>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DyadicPoseGraphAngleDtoV1 {
    edge: ori_domain::EdgeId,
    angle_degrees: f64,
}

#[derive(Default)]
pub(super) struct DyadicPathPreviewState(Mutex<Option<DyadicPathPreviewRecordV1>>);

struct DyadicPathPreviewRecordV1 {
    token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    target_binding: [u8; 32],
    path_binding: String,
    positive_binding: String,
    layer_binding: String,
    authority: Option<DyadicPathNativeAuthorityV1>,
}

struct DyadicPathNativeAuthorityV1 {
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
    path: ori_collision::CertifiedPoseGraphPathCertificateV1,
    edges: Vec<DyadicPathEdgeAuthorityV1>,
    paper_thickness_mm: f64,
    target_angles: ori_kinematics::CanonicalHingeAngles,
}

struct DyadicPathEdgeAuthorityV1 {
    source: ori_collision::PoseFingerprintV1,
    target: ori_collision::PoseFingerprintV1,
    schedule: ori_kinematics::CanonicalCycleScheduleV1,
    closure: Option<ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1>,
    auxiliary: DyadicAuxiliaryProofV1,
}

enum DyadicAuxiliaryProofV1 {
    Graph {
        positive: ori_collision::PositiveThicknessContinuousCertificateV1,
        layer: ori_collision::GeneralMultiFaceCellTransportProofV1,
    },
    Tree {
        positive: ori_collision::PositiveThicknessTreeContinuousCertificateV1,
        layer: ori_collision::SharedVertexTreeLayerTransportProofV1,
    },
}

struct DyadicAuxiliaryEdgeV1 {
    positive_binding: Option<[u8; 32]>,
    layer_binding: Option<[u8; 32]>,
    schedule: ori_kinematics::CanonicalCycleScheduleV1,
    closure: Option<ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1>,
    proof: Option<DyadicAuxiliaryProofV1>,
}

impl DyadicPathNativeAuthorityV1 {
    fn revalidates_private_proofs_v1(
        &self,
        record_target: [u8; 32],
        record_path_binding: &str,
    ) -> bool {
        let Some((geometry, _audit, _pose)) = self.pose_capability.graph() else {
            return false;
        };
        let path_binding = self
            .path
            .binding_fingerprint_v1()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if self.path.target() != record_target
            || pose_state_fingerprint_v1(&self.target_angles) != record_target
            || path_binding != record_path_binding
            || self.path.edges().len() != self.edges.len()
        {
            return false;
        }
        self.path
            .edges()
            .iter()
            .zip(&self.edges)
            .all(|(path_edge, edge)| {
                path_edge.source() == edge.source
                    && path_edge.target() == edge.target
                    && match &edge.auxiliary {
                        DyadicAuxiliaryProofV1::Graph { positive, layer } => {
                            let Some(closure) = edge.closure.as_ref() else {
                                return false;
                            };
                            positive.is_for(
                                geometry,
                                closure.fixed_face(),
                                &edge.schedule,
                                closure,
                                self.paper_thickness_mm,
                            ) && layer.is_for(
                                geometry,
                                self.layer_capability.snapshot(),
                                &edge.schedule,
                                closure,
                                self.paper_thickness_mm,
                            )
                        }
                        DyadicAuxiliaryProofV1::Tree { positive, layer } => self
                            .pose_capability
                            .tree()
                            .and_then(|(model, native_source_pose)| {
                                let source = edge.schedule.evaluate(0.0)?;
                                let target = edge.schedule.evaluate(1.0)?;
                                let source_pose =
                                    model.solve(native_source_pose.fixed_face(), &source).ok()?;
                                Some(
                                    positive.is_for(
                                        model,
                                        &source_pose,
                                        &target,
                                        self.paper_thickness_mm,
                                    ) && layer.is_for(
                                        model,
                                        &source_pose,
                                        self.layer_capability.snapshot(),
                                        &target,
                                        self.paper_thickness_mm,
                                        positive,
                                    ),
                                )
                            })
                            .unwrap_or(false),
                    }
            })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DyadicPathPreviewRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    target_angles: Vec<DyadicPoseGraphAngleDtoV1>,
    max_states: usize,
    max_transitions: usize,
    #[serde(default = "default_dyadic_level_count_v1")]
    level_count: usize,
    #[serde(default)]
    cycle_schedule_v1: Option<CycleScheduleRequestV1>,
    expected_path_binding_sha256: String,
    expected_positive_thickness_binding_sha256: String,
    expected_layer_transport_binding_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DyadicPathPreviewResponseV1 {
    version: u32,
    preview_token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    target_binding_sha256: String,
    path_binding_sha256: String,
    positive_thickness_binding_sha256: String,
    layer_transport_binding_sha256: String,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ApplyDyadicPathPreviewRequestV1 {
    preview_token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_target_binding_sha256: String,
    expected_path_binding_sha256: String,
    expected_positive_thickness_binding_sha256: String,
    expected_layer_transport_binding_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CancelDyadicPathPreviewRequestV1 {
    preview_token: ProjectId,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DyadicPoseGraphReadResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    status: &'static str,
    reason: &'static str,
    state_count: usize,
    transition_count: usize,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    certified_transition_count: usize,
    certificate_binding_sha256: Option<String>,
    positive_thickness_transition_count: usize,
    positive_thickness_certified: bool,
    positive_thickness_binding_sha256: Option<String>,
    layer_transport_transition_count: usize,
    layer_transport_certified: bool,
    layer_transport_binding_sha256: Option<String>,
    mutation_candidate_ready: bool,
    authorizes_project_mutation: bool,
}

#[tauri::command]
pub(super) fn read_bounded_dyadic_pose_graph_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    request: DyadicPoseGraphReadRequestV1,
) -> Result<DyadicPoseGraphReadResponseV1, String> {
    read_bounded_dyadic_pose_graph_inner_v1(&app_state, Some(&foldability_state), request, None)
}

#[tauri::command]
pub(super) fn mint_dyadic_pose_path_preview_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    preview_state: State<'_, DyadicPathPreviewState>,
    request: DyadicPathPreviewRequestV1,
) -> Result<DyadicPathPreviewResponseV1, String> {
    mint_dyadic_pose_path_preview_inner_v1(&app_state, &foldability_state, &preview_state, request)
}

fn mint_dyadic_pose_path_preview_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    preview_state: &DyadicPathPreviewState,
    request: DyadicPathPreviewRequestV1,
) -> Result<DyadicPathPreviewResponseV1, String> {
    let valid_hash = |value: &str| {
        value.len() == 64
            && value
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    };
    if !valid_hash(&request.expected_path_binding_sha256)
        || !valid_hash(&request.expected_positive_thickness_binding_sha256)
        || !valid_hash(&request.expected_layer_transport_binding_sha256)
    {
        return Err(INVALID_REQUEST_MESSAGE.to_owned());
    }
    if !dyadic_request_hinge_counts_are_bounded_v1(
        request.target_angles.len(),
        request
            .cycle_schedule_v1
            .as_ref()
            .map(|schedule| schedule.entries.len()),
    ) {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    if request.max_states > MAX_DYADIC_GRAPH_STATES_V1
        || request.max_transitions > MAX_DYADIC_GRAPH_TRANSITIONS_V1
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let mut target_entries = request
        .target_angles
        .iter()
        .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    target_entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
    let target = ori_kinematics::CanonicalHingeAngles::new(target_entries)
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let target_binding = pose_state_fingerprint_v1(&target);
    let mut native_authority = None;
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        app_state,
        Some(foldability_state),
        DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: request.expected_project_instance_id,
            expected_project_id: request.expected_project_id,
            expected_revision: request.expected_revision,
            target_angles: request.target_angles,
            max_states: request.max_states,
            max_transitions: request.max_transitions,
            level_count: request.level_count,
            cycle_schedule_v1: request.cycle_schedule_v1,
        },
        Some(&mut native_authority),
    )?;
    if !observed.mutation_candidate_ready
        || observed.certificate_binding_sha256.as_deref()
            != Some(request.expected_path_binding_sha256.as_str())
        || observed.positive_thickness_binding_sha256.as_deref()
            != Some(request.expected_positive_thickness_binding_sha256.as_str())
        || observed.layer_transport_binding_sha256.as_deref()
            != Some(request.expected_layer_transport_binding_sha256.as_str())
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let token = ProjectId::new();
    let record = DyadicPathPreviewRecordV1 {
        token,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_binding,
        path_binding: request.expected_path_binding_sha256.clone(),
        positive_binding: request.expected_positive_thickness_binding_sha256.clone(),
        layer_binding: request.expected_layer_transport_binding_sha256.clone(),
        authority: Some(native_authority.ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?),
    };
    *preview_state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())? = Some(record);
    Ok(DyadicPathPreviewResponseV1 {
        version: 1,
        preview_token: token,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_binding_sha256: target_binding
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        path_binding_sha256: request.expected_path_binding_sha256,
        positive_thickness_binding_sha256: request.expected_positive_thickness_binding_sha256,
        layer_transport_binding_sha256: request.expected_layer_transport_binding_sha256,
        authorizes_project_mutation: false,
    })
}

/// Consumes a fully bound dyadic preview only after its private proof objects
/// and both live authority slots have been revalidated atomically.
#[tauri::command]
pub(super) fn apply_dyadic_pose_path_preview_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    preview_state: State<'_, DyadicPathPreviewState>,
    request: ApplyDyadicPathPreviewRequestV1,
) -> Result<u64, String> {
    apply_dyadic_pose_path_preview_inner_v1(&app_state, &foldability_state, &preview_state, request)
}

fn apply_dyadic_pose_path_preview_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    preview_state: &DyadicPathPreviewState,
    request: ApplyDyadicPathPreviewRequestV1,
) -> Result<u64, String> {
    let mut project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let target_binding = request
        .expected_target_binding_sha256
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|value| u8::from_str_radix(value, 16).ok())
        })
        .collect::<Option<Vec<_>>>()
        .filter(|value| value.len() == 32)
        .ok_or_else(|| INVALID_REQUEST_MESSAGE.to_owned())?;
    let mut slot = preview_state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    let record = slot
        .as_ref()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    if record.token != request.preview_token
        || record.project_instance_id != request.expected_project_instance_id
        || record.project_id != request.expected_project_id
        || record.revision != request.expected_revision
        || record.target_binding.as_slice() != target_binding
        || record.path_binding != request.expected_path_binding_sha256
        || record.positive_binding != request.expected_positive_thickness_binding_sha256
        || record.layer_binding != request.expected_layer_transport_binding_sha256
        || !record.authority.as_ref().is_some_and(|authority| {
            authority.revalidates_private_proofs_v1(record.target_binding, &record.path_binding)
        })
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let authority = record
        .authority
        .as_ref()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let (geometry, _audit, pose) = authority
        .pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let pose_guard =
        lock_revalidated_current_applied_pose_for_commit(&project, &authority.pose_capability)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| STALE_MESSAGE.to_owned())?;
    let layer_guard = lock_revalidated_current_layer_order_for_commit(
        // The capability retains the exact source snapshot used by every proof.
        // Holding this guard through the document commit closes replacement races.
        foldability_state,
        &project,
        &authority.layer_capability,
    );
    let layer_guard = layer_guard
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
        .ok_or_else(|| STALE_MESSAGE.to_owned())?;
    let face_ids = geometry.face_ids().to_vec();
    let hinge_ids = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let hinge_angles = authority
        .target_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let applied_pose = prepare_closed_graph_applied_pose_v1(
        &face_ids,
        &hinge_ids,
        pose.fixed_face(),
        &hinge_angles,
        AppliedPoseLimitsV1::default(),
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let source_model_fingerprint = project.editor.fold_model_fingerprint_v1();
    let source_angles = pose
        .hinge_angles()
        .as_slice()
        .iter()
        .map(|angle| InstructionHingeAngle {
            edge: angle.edge(),
            angle_degrees: angle.angle_degrees(),
        })
        .collect::<Vec<_>>();
    let transition_targets = authority
        .edges
        .iter()
        .map(|edge| {
            edge.schedule
                .evaluate(1.0)
                .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
                .map(|angles| {
                    angles
                        .as_slice()
                        .iter()
                        .map(|angle| InstructionHingeAngle {
                            edge: angle.edge(),
                            angle_degrees: angle.angle_degrees(),
                        })
                        .collect::<Vec<_>>()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let timeline = ori_instructions::append_certified_dyadic_path_timeline_v1(
        project.editor.instruction_timeline(),
        "Certified dyadic pose path",
        &source_model_fingerprint,
        pose.fixed_face(),
        &source_angles,
        &transition_targets,
        &authority.path,
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let persisted_pose = timeline
        .steps
        .last()
        .map(|step| step.pose.clone())
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let editor_before = project.editor.clone();
    let pattern = project.editor.pattern().clone();
    let paper = project.editor.paper().clone();
    let layers = project.editor.project_layers().clone();
    let result = project
        .editor
        .execute_stacked_fold_document(
            record.revision,
            pattern,
            paper,
            timeline,
            layers,
            applied_pose,
        )
        .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    drop(pose_guard);
    if restore_persisted_current_pose(&mut project, &persisted_pose).is_err() {
        project.editor = editor_before;
        return Err(UNAVAILABLE_MESSAGE.to_owned());
    }
    layer_guard.invalidate_after_project_mutation();
    slot.take();
    Ok(result.revision)
}

#[tauri::command]
pub(super) fn cancel_dyadic_pose_path_preview_v1(
    preview_state: State<'_, DyadicPathPreviewState>,
    request: CancelDyadicPathPreviewRequestV1,
) -> Result<(), String> {
    cancel_dyadic_pose_path_preview_inner_v1(&preview_state, request.preview_token)
}

fn cancel_dyadic_pose_path_preview_inner_v1(
    preview_state: &DyadicPathPreviewState,
    token: ProjectId,
) -> Result<(), String> {
    let mut slot = preview_state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if slot.as_ref().is_none_or(|record| record.token != token) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    slot.take();
    Ok(())
}

const fn default_dyadic_level_count_v1() -> usize {
    3
}

fn read_bounded_dyadic_pose_graph_inner_v1(
    app_state: &AppState,
    foldability_state: Option<&GlobalFlatFoldabilityState>,
    request: DyadicPoseGraphReadRequestV1,
    authority_out: Option<&mut Option<DyadicPathNativeAuthorityV1>>,
) -> Result<DyadicPoseGraphReadResponseV1, String> {
    let generation = begin_stacked_fold_read_generation_v1()?;
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    if !dyadic_request_hinge_counts_are_bounded_v1(
        request.target_angles.len(),
        request
            .cycle_schedule_v1
            .as_ref()
            .map(|schedule| schedule.entries.len()),
    ) {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    if !matches!(request.level_count, 3 | 5 | 9) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
    }
    if !strict_dyadic_geometry_is_in_scope_v1(&project) {
        return Ok(unsupported_dyadic_graph_response_v1(&project));
    }
    let layer_capability = foldability_state
        .map(|state| capture_current_layer_order_capability(state, &project))
        .transpose()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
        .flatten();
    let paper_thickness_mm = project.editor.paper().thickness_mm;
    let Some(capability) = project
        .applied_pose_authority
        .capture_capability(&project)
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
    else {
        return Ok(unsupported_dyadic_graph_response_v1(&project));
    };
    let Some((geometry, audit, pose)) = capability.graph() else {
        return Ok(unsupported_dyadic_graph_response_v1(&project));
    };
    let mut target_entries = request
        .target_angles
        .iter()
        .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    target_entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
    let target = ori_kinematics::CanonicalHingeAngles::new(target_entries)
        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let collective_schedule = request
        .cycle_schedule_v1
        .as_ref()
        .map(|schedule| {
            prepare_requested_cycle_schedule_v1(
                schedule,
                geometry,
                audit,
                pose.fixed_face(),
                pose.hinge_angles(),
            )
        })
        .transpose()
        .map_err(str::to_owned)?
        .or_else(|| {
            generate_even_opposite_pair_schedule_v1(
                geometry,
                audit,
                pose.fixed_face(),
                pose.hinge_angles(),
                &target,
            )
            .ok()
        });
    if collective_schedule
        .as_ref()
        .is_some_and(|schedule| schedule.evaluate(1.0).as_ref() != Some(&target))
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let generated_graph = if let Some(schedule) = collective_schedule.as_ref() {
        schedule
            .evaluate(0.5)
            .and_then(|midpoint| {
                ori_kinematics::generate_bounded_collective_pose_graph_v1(
                    pose.hinge_angles(),
                    &midpoint,
                    &target,
                )
                .ok()
            })
            .ok_or(ori_kinematics::DyadicPoseGraphGenerationErrorV1::BindingMismatch)
    } else if audit.closure_hinges().len() >= 2 || target.as_slice().len() >= 32 {
        let midpoint = ori_kinematics::CanonicalHingeAngles::new(
            pose.hinge_angles()
                .as_slice()
                .iter()
                .zip(target.as_slice())
                .map(|(source, target)| {
                    ori_kinematics::HingeAngle::new(
                        source.edge(),
                        (source.angle_degrees() + target.angle_degrees()) * 0.5,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
        )
        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
        ori_kinematics::generate_bounded_collective_pose_graph_v1(
            pose.hinge_angles(),
            &midpoint,
            &target,
        )
    } else {
        ori_kinematics::generate_bounded_dyadic_pose_graph_at_levels_v1(
            pose.hinge_angles(),
            &target,
            request.level_count,
            ori_kinematics::DyadicPoseGraphLimitsV1 {
                max_states: request.max_states,
                max_transitions: request.max_transitions,
            },
            || STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == generation,
        )
    };
    let graph = match generated_graph {
        Ok(value) => value,
        Err(ori_kinematics::DyadicPoseGraphGenerationErrorV1::ResourceLimit) => {
            return Ok(dyadic_graph_response(
                &project,
                "resource_limit",
                0,
                0,
                0,
                0,
                0,
                None,
                0,
                None,
                0,
                None,
            ));
        }
        Err(ori_kinematics::DyadicPoseGraphGenerationErrorV1::Cancelled) => {
            return Ok(dyadic_graph_response(
                &project,
                "cancelled",
                0,
                0,
                0,
                0,
                0,
                None,
                0,
                None,
                0,
                None,
            ));
        }
        Err(_) => return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned()),
    };
    let fingerprints = graph
        .states()
        .iter()
        .map(pose_state_fingerprint_v1)
        .collect::<Vec<_>>();
    let mut candidates = graph
        .transitions()
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let mut key = [0; 32];
            key[24..].copy_from_slice(&(index as u64).to_be_bytes());
            ori_collision::CertifiedPathTransitionCandidateV1 {
                source: fingerprints[edge.source_state],
                target: fingerprints[edge.target_state],
                candidate_key: key,
            }
        })
        .collect::<Vec<_>>();
    let midpoint = ori_kinematics::CanonicalHingeAngles::new(
        pose.hinge_angles()
            .as_slice()
            .iter()
            .zip(target.as_slice())
            .map(|(source, target)| {
                ori_kinematics::HingeAngle::new(
                    source.edge(),
                    (source.angle_degrees() + target.angle_degrees()) * 0.5,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
    )
    .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    if let Some(midpoint_state) = graph.states().iter().position(|state| state == &midpoint) {
        for (index, (source_state, target_state)) in [
            (graph.source_state(), midpoint_state),
            (midpoint_state, graph.source_state()),
            (midpoint_state, graph.target_state()),
            (graph.target_state(), midpoint_state),
        ]
        .into_iter()
        .enumerate()
        {
            if source_state != target_state
                && !candidates.iter().any(|edge| {
                    edge.source == fingerprints[source_state]
                        && edge.target == fingerprints[target_state]
                })
            {
                let mut key = [0xff; 32];
                key[24..].copy_from_slice(&(index as u64).to_be_bytes());
                candidates.push(ori_collision::CertifiedPathTransitionCandidateV1 {
                    source: fingerprints[source_state],
                    target: fingerprints[target_state],
                    candidate_key: key,
                });
            }
        }
    }
    if graph.source_state() != graph.target_state()
        && !candidates.iter().any(|edge| {
            edge.source == fingerprints[graph.source_state()]
                && edge.target == fingerprints[graph.target_state()]
        })
    {
        candidates.push(ori_collision::CertifiedPathTransitionCandidateV1 {
            source: fingerprints[graph.source_state()],
            target: fingerprints[graph.target_state()],
            candidate_key: [0xfe; 32],
        });
    }
    if candidates.len()
        > graph
            .transitions()
            .len()
            .checked_add(ori_collision::MAX_CERTIFIED_PATH_GRAPH_OVERLAY_EDGES_V1)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let mut auxiliary_certificates = std::collections::HashMap::new();
    let searched = ori_collision::search_certified_pose_graph_with_checkpoint_v1(
        &fingerprints,
        &candidates,
        fingerprints[graph.source_state()],
        fingerprints[graph.target_state()],
        || STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == generation,
        |edge| {
            let source = fingerprints
                .iter()
                .position(|value| value == &edge.source)?;
            let target = fingerprints
                .iter()
                .position(|value| value == &edge.target)?;
            let generated = if source == graph.source_state() && target == graph.target_state() {
                collective_schedule.as_ref().and_then(|schedule| {
                    ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
                        schedule.clone(), pose.hinge_angles(), &graph.states()[target]).ok()
                }).or_else(||
                [1, 2, 4, 8, 16].into_iter().find_map(|denominator| {
                    let generated = ori_kinematics::generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
                        geometry, audit, pose.fixed_face(), denominator, production_cycle_schedule_limits_v1()).ok()?;
                    (generated.schedule().evaluate(1.0).as_ref() == Some(&graph.states()[target]))
                        .then_some(generated)
                }))
            } else {
                None
            }
            .or_else(|| generate_linear_multi_hinge_path_candidate_v1(
                geometry, audit, pose.fixed_face(), &graph.states()[source],
                &graph.states()[target], MultiHingePathCandidateLimitsV1::default()).ok())?;
            let closure = geometry
                .prove_dyadic_schedule_closure_v1(
                    audit,
                    pose.fixed_face(),
                    generated.schedule(),
                    ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 8,
                        max_leaves: 256,
                        max_work: 1_152,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .ok();
            let cycle_evidence = closure.as_ref().and_then(|closure| {
                ori_collision::certify_scheduled_cycle_transition_v1(
                    geometry,
                    audit,
                    pose.fixed_face(),
                    &generated,
                    &closure,
                    32,
                    edge.source,
                    edge.target,
                )
            });
            let mut tree_positive_seed = None;
            let evidence = cycle_evidence.or_else(|| {
                if edge.source != pose_state_fingerprint_v1(pose.hinge_angles()) {
                    return None;
                }
                let (tree_model, tree_source_pose) = capability.tree()?;
                let target = generated.schedule().evaluate(1.0)?;
                let positive = ori_collision::certify_positive_thickness_tree_continuous_path_v1(
                    tree_model,
                    tree_source_pose,
                    &target,
                    paper_thickness_mm,
                )?;
                let evidence = ori_collision::CertifiedPathTransitionEvidenceV1::from_native_oracle(
                    edge.source,
                    edge.target,
                    generated.schedule().certificate_binding_fingerprint_v1(),
                    positive.binding_fingerprint_v1(),
                    generated.schedule().graph_binding_fingerprint_v1(),
                );
                tree_positive_seed = Some(positive);
                Some(evidence)
            })?;
            let positive = closure.as_ref().and_then(|closure| {
                certify_canonical_positive_thickness_cycle_schedule_path_v1(
                    geometry,
                    audit,
                    pose.fixed_face(),
                    generated.schedule(),
                    &closure,
                    paper_thickness_mm,
                    32,
                )
            });
            let mut positive_binding = positive.as_ref().map(|certificate| {
                let mut hash = Sha256::new();
                hash.update(b"dyadic_positive_thickness_transition_v1");
                hash.update(edge.source);
                hash.update(edge.target);
                hash.update(evidence.schedule_certificate());
                hash.update(evidence.closure_certificate());
                hash.update(certificate.thickness_bits().to_be_bytes());
                <[u8; 32]>::from(hash.finalize())
            });
            let layer = positive.as_ref().and_then(|positive| {
                let closure = closure.as_ref()?;
                let source = layer_capability.as_ref()?.snapshot();
                let transition_count = closure.leaves().len().checked_add(1)?;
                let layer_records = source.overlap_cells.iter().try_fold(0usize, |sum, cell| {
                    sum.checked_add(cell.bottom_to_top_faces.len())
                })?;
                let boundary_samples = source
                    .overlap_cells
                    .iter()
                    .try_fold(0usize, |sum, cell| {
                        cell.exact_boundary
                            .len()
                            .checked_mul(cell.bottom_to_top_faces.len())
                            .and_then(|work| sum.checked_add(work))
                    })?
                    .checked_mul(transition_count)?;
                let proof =
                    certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                        geometry,
                        audit,
                        source,
                        schedule: generated.schedule(),
                        closure: &closure,
                        positive_continuous: positive,
                        paper_thickness_mm,
                        tolerance: ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                        limits: GeneralCellTransportLimitsV1 {
                            max_transitions: transition_count,
                            max_cells: source.overlap_cells.len(),
                            max_layer_records: layer_records,
                            max_boundary_samples: boundary_samples,
                        },
                    })
                    .ok()?;
                let mut hash = Sha256::new();
                hash.update(b"dyadic_layer_transport_transition_v1");
                hash.update(edge.source);
                hash.update(edge.target);
                hash.update(proof.target_order_hash());
                for checkpoint in proof.transition_hashes() {
                    hash.update(checkpoint);
                }
                Some((<[u8; 32]>::from(hash.finalize()), proof))
            });
            let mut layer_binding = layer.as_ref().map(|value| value.0);
            let mut proof = positive.zip(layer.as_ref()).map(|(positive, (_, layer))| {
                DyadicAuxiliaryProofV1::Graph {
                    positive,
                    layer: layer.clone(),
                }
            });
            if proof.is_none() && edge.source == pose_state_fingerprint_v1(pose.hinge_angles()) {
                proof = capability
                    .tree()
                    .and_then(|(tree_model, tree_source_pose)| {
                        let target = generated.schedule().evaluate(1.0)?;
                        let positive = tree_positive_seed.take().or_else(|| {
                            ori_collision::certify_positive_thickness_tree_continuous_path_v1(
                                tree_model,
                                tree_source_pose,
                                &target,
                                paper_thickness_mm,
                            )
                        })?;
                        let layer = ori_collision::prepare_shared_vertex_tree_layer_transport_v1(
                            tree_model,
                            tree_source_pose,
                            layer_capability.as_ref()?.snapshot(),
                            &target,
                            paper_thickness_mm,
                            &positive,
                        )?;
                        let mut positive_hash = Sha256::new();
                        positive_hash.update(b"dyadic_tree_positive_thickness_transition_v1");
                        positive_hash.update(edge.source);
                        positive_hash.update(edge.target);
                        positive_hash.update(paper_thickness_mm.to_bits().to_be_bytes());
                        positive_binding = Some(positive_hash.finalize().into());
                        let mut layer_hash = Sha256::new();
                        layer_hash.update(b"dyadic_tree_shared_vertex_layer_transition_v1");
                        layer_hash.update(edge.source);
                        layer_hash.update(edge.target);
                        layer_hash.update(paper_thickness_mm.to_bits().to_be_bytes());
                        layer_binding = Some(layer_hash.finalize().into());
                        Some(DyadicAuxiliaryProofV1::Tree { positive, layer })
                    });
            }
            auxiliary_certificates.insert(
                (edge.source, edge.target),
                DyadicAuxiliaryEdgeV1 {
                    positive_binding,
                    layer_binding,
                    schedule: generated.schedule().clone(),
                    closure,
                    proof,
                },
            );
            Some(evidence)
        },
    );
    let mut authority_parts = None;
    let (
        status,
        explored,
        evaluated,
        certified,
        binding,
        positive_count,
        positive_binding,
        layer_count,
        layer_binding,
    ) = match searched {
        ori_collision::CertifiedPathGraphSearchResultV1::Certified(value) => {
            if !value.edges().iter().all(|edge| {
                edge.source() != edge.target()
                    && edge.schedule_certificate() != [0; 32]
                    && edge.collision_certificate() != [0; 32]
                    && edge.closure_certificate() != [0; 32]
            }) {
                return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
            }
            let binding = value
                .binding_fingerprint_v1()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let aggregate = |select_layer: bool| {
                let mut count = 0usize;
                let mut hash = Sha256::new();
                hash.update(if select_layer {
                    b"dyadic_layer_transport_path_v1".as_slice()
                } else {
                    b"dyadic_positive_thickness_path_v1".as_slice()
                });
                for edge in value.edges() {
                    let pair = auxiliary_certificates.get(&(edge.source(), edge.target()))?;
                    let certificate = if select_layer {
                        pair.layer_binding.as_ref()?
                    } else {
                        pair.positive_binding.as_ref()?
                    };
                    hash.update(edge.source());
                    hash.update(edge.target());
                    hash.update(certificate);
                    count += 1;
                }
                Some((
                    count,
                    hash.finalize()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                ))
            };
            let positive = aggregate(false);
            let layer = aggregate(true);
            let explored = value.explored_state_count();
            let evaluated = value.evaluated_transition_count();
            let certified = value.edges().len();
            if positive.is_some() && layer.is_some() && authority_out.is_some() {
                let mut edges = Vec::with_capacity(certified);
                for certified_edge in value.edges() {
                    let mut edge = auxiliary_certificates
                        .remove(&(certified_edge.source(), certified_edge.target()))
                        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
                    edges.push(DyadicPathEdgeAuthorityV1 {
                        source: certified_edge.source(),
                        target: certified_edge.target(),
                        schedule: edge.schedule,
                        closure: edge.closure,
                        auxiliary: edge
                            .proof
                            .take()
                            .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?,
                    });
                }
                authority_parts = Some((value, edges));
            }
            (
                "certified",
                explored,
                evaluated,
                certified,
                Some(binding),
                positive.as_ref().map_or(0, |value| value.0),
                positive.map(|value| value.1),
                layer.as_ref().map_or(0, |value| value.0),
                layer.map(|value| value.1),
            )
        }
        ori_collision::CertifiedPathGraphSearchResultV1::Indeterminate {
            reason,
            explored_state_count,
            evaluated_transition_count,
        } => (
            match reason {
                ori_collision::CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath => {
                    "no_path"
                }
                ori_collision::CertifiedPathGraphIndeterminateReasonV1::ResourceLimit => {
                    "resource_limit"
                }
                ori_collision::CertifiedPathGraphIndeterminateReasonV1::Cancelled => "cancelled",
            },
            explored_state_count,
            evaluated_transition_count,
            0,
            None,
            0,
            None,
            0,
            None,
        ),
    };
    if let Some(out) = authority_out {
        *out = match (authority_parts, layer_capability) {
            (Some((path, edges)), Some(layer_capability)) => Some(DyadicPathNativeAuthorityV1 {
                pose_capability: capability,
                layer_capability,
                path,
                edges,
                paper_thickness_mm,
                target_angles: target,
            }),
            _ => None,
        };
    }
    Ok(dyadic_graph_response(
        &project,
        status,
        graph.states().len(),
        graph.transitions().len(),
        explored,
        evaluated,
        certified,
        binding,
        positive_count,
        positive_binding,
        layer_count,
        layer_binding,
    ))
}

fn unsupported_dyadic_graph_response_v1(
    project: &super::ProjectState,
) -> DyadicPoseGraphReadResponseV1 {
    dyadic_graph_response(
        project,
        "unsupported",
        0,
        0,
        0,
        0,
        0,
        None,
        0,
        None,
        0,
        None,
    )
}

fn strict_dyadic_geometry_is_in_scope_v1(project: &super::ProjectState) -> bool {
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let Some(snapshot) = topology.simulation_snapshot() else {
        return false;
    };
    if !strict_dyadic_topology_snapshot_is_in_scope_v1(snapshot) {
        return false;
    }
    let pattern = project.editor.pattern();
    if pattern
        .edges
        .iter()
        .any(|edge| edge.kind == ori_domain::EdgeKind::Cut)
    {
        return false;
    }
    let boundary = &project.editor.paper().boundary_vertices;
    if boundary.len() < 3
        || boundary
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != boundary.len()
    {
        return false;
    }
    let point = |id| {
        pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == id)
            .map(|vertex| vertex.position)
            .filter(|point| point.x.is_finite() && point.y.is_finite())
    };
    let Some(points) = boundary
        .iter()
        .copied()
        .map(point)
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if points.iter().enumerate().any(|(index, first)| {
        let second = points[(index + 1) % points.len()];
        first.x.to_bits() == second.x.to_bits() && first.y.to_bits() == second.y.to_bits()
    }) {
        return false;
    }
    let cross =
        |first: ori_domain::Point2, second: ori_domain::Point2, third: ori_domain::Point2| {
            (second.x - first.x) * (third.y - first.y) - (second.y - first.y) * (third.x - first.x)
        };
    let on_segment =
        |first: ori_domain::Point2, second: ori_domain::Point2, point: ori_domain::Point2| {
            cross(first, second, point) == 0.0
                && point.x >= first.x.min(second.x)
                && point.x <= first.x.max(second.x)
                && point.y >= first.y.min(second.y)
                && point.y <= first.y.max(second.y)
        };
    let intersects = |first: ori_domain::Point2,
                      second: ori_domain::Point2,
                      third: ori_domain::Point2,
                      fourth: ori_domain::Point2| {
        let values = [
            cross(first, second, third),
            cross(first, second, fourth),
            cross(third, fourth, first),
            cross(third, fourth, second),
        ];
        (values[0].is_sign_positive() != values[1].is_sign_positive()
            && values[2].is_sign_positive() != values[3].is_sign_positive()
            && values.iter().all(|value| *value != 0.0))
            || on_segment(first, second, third)
            || on_segment(first, second, fourth)
            || on_segment(third, fourth, first)
            || on_segment(third, fourth, second)
    };
    for first in 0..points.len() {
        for second in (first + 1)..points.len() {
            if second == first + 1 || (first == 0 && second + 1 == points.len()) {
                continue;
            }
            if intersects(
                points[first],
                points[(first + 1) % points.len()],
                points[second],
                points[(second + 1) % points.len()],
            ) {
                return false;
            }
        }
    }
    let mut orientation = 0_i8;
    for index in 0..boundary.len() {
        let turn = cross(
            points[index],
            points[(index + 1) % points.len()],
            points[(index + 2) % points.len()],
        );
        if turn == 0.0 {
            continue;
        }
        let sign = if turn.is_sign_positive() { 1 } else { -1 };
        if orientation == 0 {
            orientation = sign;
        } else if orientation != sign {
            return false;
        }
    }
    orientation != 0
}

fn strict_dyadic_topology_snapshot_is_in_scope_v1(
    snapshot: &ori_topology::TopologySnapshot,
) -> bool {
    snapshot.material_components.len() == 1
        && snapshot
            .faces
            .iter()
            .all(|face| face.holes.is_empty() && face.seams.is_empty())
}

fn dyadic_graph_response(
    project: &super::ProjectState,
    status: &'static str,
    state_count: usize,
    transition_count: usize,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    certified_transition_count: usize,
    certificate_binding_sha256: Option<String>,
    positive_thickness_transition_count: usize,
    positive_thickness_binding_sha256: Option<String>,
    layer_transport_transition_count: usize,
    layer_transport_binding_sha256: Option<String>,
) -> DyadicPoseGraphReadResponseV1 {
    let positive_thickness_certified = certified_transition_count > 0
        && positive_thickness_transition_count == certified_transition_count
        && positive_thickness_binding_sha256.is_some();
    let layer_transport_certified = certified_transition_count > 0
        && layer_transport_transition_count == certified_transition_count
        && layer_transport_binding_sha256.is_some();
    DyadicPoseGraphReadResponseV1 {
        version: 1,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        status,
        reason: match status {
            "certified" if positive_thickness_certified && layer_transport_certified => {
                "proof_complete"
            }
            "certified" => "no_certified_path",
            "no_path" => "no_certified_path",
            "resource_limit" => "bounded_resource_limit",
            "cancelled" => "cancelled",
            _ => "unsupported_geometry",
        },
        state_count,
        transition_count,
        explored_state_count,
        evaluated_transition_count,
        certified_transition_count,
        certificate_binding_sha256,
        positive_thickness_transition_count,
        positive_thickness_certified,
        positive_thickness_binding_sha256,
        layer_transport_transition_count,
        layer_transport_certified,
        layer_transport_binding_sha256,
        mutation_candidate_ready: positive_thickness_certified && layer_transport_certified,
        authorizes_project_mutation: false,
    }
}

#[tauri::command]
pub(super) fn read_even_cycle_candidates_v1(
    app_state: State<'_, AppState>,
    request: EvenCycleCandidatesRequestV1,
) -> Result<EvenCycleCandidatesResponseV1, String> {
    read_even_cycle_candidates_inner_v1(&app_state, request)
}

fn read_even_cycle_candidates_inner_v1(
    app_state: &AppState,
    request: EvenCycleCandidatesRequestV1,
) -> Result<EvenCycleCandidatesResponseV1, String> {
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .ok()
        .flatten();
    let graph = pose_capability
        .as_ref()
        .and_then(|capability| capability.graph());
    let (status, reason, candidates) = match graph {
        None => (
            "unsupported",
            "current_pose_is_not_a_material_hinge_graph",
            Vec::new(),
        ),
        Some((geometry, audit, _)) => {
            match ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
                geometry,
                audit,
                request.max_pair_tests,
            ) {
                Ok(pairs) if pairs.is_empty() => {
                    ("none", "no_same_assignment_opposite_pair", Vec::new())
                }
                Ok(pairs) => ("ready", "same_assignment_geometrically_opposite", pairs),
                Err(ori_kinematics::KinematicsError::ResourceLimitExceeded) => {
                    ("resource_limit", "pair_test_limit_exceeded", Vec::new())
                }
                Err(_) => (
                    "unsupported",
                    "not_a_bounded_even_single_vertex_cycle",
                    Vec::new(),
                ),
            }
        }
    };
    let kawasaki_endpoints = graph.map_or_else(Vec::new, |(geometry, audit, pose)| {
        [1_u64, 2, 4, 8, 16]
            .into_iter()
            .filter_map(|endpoint_denominator| {
                let generated = ori_kinematics::generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
                    geometry, audit, pose.fixed_face(), endpoint_denominator,
                    production_cycle_schedule_limits_v1(),
                ).ok()?;
                let closure = geometry.prove_simultaneous_cycle_basis_schedule_closure_v1(
                    audit, pose.fixed_face(), generated.schedule(),
                    ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                    CycleBasisLimitsV1::default(),
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 16, max_leaves: 65_536, max_work: 1_048_576,
                        schedule_limits: production_cycle_schedule_limits_v1(),
                    },
                ).ok()?;
                let continuous = diagnose_scheduled_cycle_path_v1(
                    geometry, audit, pose.fixed_face(), &generated, closure.closure(), 32,
                );
                let certified = continuous.continuous_certificate_model_id().is_some();
                Some(KawasakiEndpointCandidateDtoV1 {
                    version: 1,
                    endpoint_denominator,
                    closure_status: "certified",
                    collision_status: if certified { "certified" } else { "uncertified" },
                    authorizes_apply: false,
                })
            })
            .collect()
    });
    Ok(EvenCycleCandidatesResponseV1 {
        version: 1,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        status,
        reason,
        candidates: candidates
            .into_iter()
            .map(|edges| EvenCycleCandidateDtoV1 {
                version: 1,
                edges,
                reason: "same_assignment_geometrically_opposite",
            })
            .collect(),
        kawasaki_endpoints,
        authorizes_project_mutation: false,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LinearCandidateRequestV1 {
    version: u32,
    entries: Vec<LinearCandidateEntryRequestV1>,
    #[serde(default)]
    exact_dyadic_path_v1: Option<ExactDyadicPathRequestV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactDyadicPathRequestV1 {
    version: u32,
    segments: Vec<ExactDyadicSegmentRequestV1>,
    max_pair_tests: usize,
    max_denominator_power: u32,
    max_integer_bits: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactDyadicSegmentRequestV1 {
    start: ExactDyadicPointRequestV1,
    end: ExactDyadicPointRequestV1,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactDyadicPointRequestV1 {
    x_numerator: i128,
    y_numerator: i128,
    denominator_power: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LinearCandidateEntryRequestV1 {
    edge: ori_domain::EdgeId,
    initial_angle_degrees: f64,
    requested_angle_degrees: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CertifiedPathGraphRequestV1 {
    version: u32,
    states: Vec<CertifiedPathGraphStateRequestV1>,
    transitions: Vec<CertifiedPathGraphTransitionRequestV1>,
    source_state: usize,
    target_state: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CertifiedPathGraphStateRequestV1 {
    entries: Vec<CertifiedPathGraphAngleRequestV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CertifiedPathGraphAngleRequestV1 {
    edge: ori_domain::EdgeId,
    angle_degrees: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CertifiedPathGraphTransitionRequestV1 {
    source_state: usize,
    target_state: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentCyclePosePreviewRequestV1 {
    #[serde(default)]
    progress_request_id: Option<String>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    cycle_schedule_v1: CycleScheduleRequestV1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentCyclePosePreviewResponseV1 {
    version: u32,
    transaction_token: ProjectId,
    source_revision: u64,
    target_revision: u64,
    closure_leaf_count: usize,
    closure_max_depth: u32,
    checked_hinge_count: usize,
    total_hinge_count: usize,
    continuous_path_certified: bool,
    continuous_layer_transport_model_id: Option<&'static str>,
    continuous_layer_transition_count: usize,
    continuous_layer_pair_order_count: usize,
    continuous_layer_target_order_sha256: Option<String>,
    source_layer_order: Vec<LayerOrderPairDtoV1>,
    target_layer_order: Vec<LayerOrderPairDtoV1>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayerOrderPairDtoV1 {
    lower_face: FaceId,
    upper_face: FaceId,
}

#[tauri::command]
pub(super) fn propose_current_cycle_pose_v1(
    app: AppHandle,
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, super::stacked_fold_transaction::StackedFoldTransactionState>,
    request: CurrentCyclePosePreviewRequestV1,
) -> Result<CurrentCyclePosePreviewResponseV1, String> {
    let request_id = request.progress_request_id.clone();
    let result = propose_current_cycle_pose_inner_with_layers(
        Some(&app),
        &app_state,
        Some(&foldability_state),
        &transaction_state,
        request,
    );
    if let Some(request_id) = request_id.as_deref() {
        emit_current_cycle_terminal_v1(
            &app,
            request_id,
            match &result {
                Ok(_) => "certified",
                Err(error) if error == CANCELLED_MESSAGE => "cancelled",
                Err(_) => "failed",
            },
        );
    }
    result
}

#[cfg(test)]
fn propose_current_cycle_pose_inner(
    app: Option<&AppHandle>,
    app_state: &AppState,
    transaction_state: &super::stacked_fold_transaction::StackedFoldTransactionState,
    request: CurrentCyclePosePreviewRequestV1,
) -> Result<CurrentCyclePosePreviewResponseV1, String> {
    propose_current_cycle_pose_inner_with_layers(app, app_state, None, transaction_state, request)
}

fn propose_current_cycle_pose_inner_with_layers(
    app: Option<&AppHandle>,
    app_state: &AppState,
    foldability_state: Option<&GlobalFlatFoldabilityState>,
    transaction_state: &super::stacked_fold_transaction::StackedFoldTransactionState,
    request: CurrentCyclePosePreviewRequestV1,
) -> Result<CurrentCyclePosePreviewResponseV1, String> {
    let generation = begin_stacked_fold_read_generation_v1()?;
    let progress_request_id =
        validate_progress_request_id_v1(request.progress_request_id.as_deref())?;
    emit_current_cycle_progress_v1(app, progress_request_id, 0, 0);
    emit_current_cycle_status_v1(app, progress_request_id, "running", 0);
    let project = lock_project(&app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let layer_capability = foldability_state
        .map(|state| capture_current_layer_order_capability(state, &project))
        .transpose()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
        .flatten();
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
        .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
    let (geometry, audit, pose) = pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let automatic_kawasaki =
        request.cycle_schedule_v1.version == 2 && request.cycle_schedule_v1.entries.is_empty();
    let schedule = if automatic_kawasaki {
        ori_kinematics::generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
            geometry,
            audit,
            pose.fixed_face(),
            request.cycle_schedule_v1.endpoint_denominator.unwrap_or(1),
            production_cycle_schedule_limits_v1(),
        )
        .map_err(|error| match error {
            ori_kinematics::MultiHingePathCandidateErrorV1::ResourceLimit => {
                CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
            }
            _ => CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned(),
        })?
        .schedule()
        .clone()
    } else {
        prepare_requested_cycle_schedule_v1(
            &request.cycle_schedule_v1,
            geometry,
            audit,
            pose.fixed_face(),
            pose.hinge_angles(),
        )
        .map_err(str::to_owned)?
    };
    let requested = schedule
        .evaluate(1.0)
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let generated = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
        schedule,
        pose.hinge_angles(),
        &requested,
    )
    .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let basis_closure = geometry
        .prove_simultaneous_cycle_basis_schedule_closure_v1(
            audit,
            pose.fixed_face(),
            generated.schedule(),
            ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            CycleBasisLimitsV1::default(),
            DyadicIntervalClosureLimitsV1 {
                max_depth: 16,
                max_leaves: 65_536,
                max_work: 1_048_576,
                schedule_limits: production_cycle_schedule_limits_v1(),
            },
        )
        .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
    let closure = basis_closure.closure().clone();
    let source = pose_state_fingerprint_v1(pose.hinge_angles());
    let target = pose_state_fingerprint_v1(&requested);
    let paper_thickness_mm = project.editor.paper().thickness_mm;
    let continuous = if paper_thickness_mm > 0.0
        && supports_scheduled_positive_thickness_path_v1(
            geometry,
            audit,
            pose.fixed_face(),
            generated.schedule(),
        ) {
        diagnose_scheduled_positive_thickness_cycle_path_v1(
            geometry,
            audit,
            pose.fixed_face(),
            &generated,
            &closure,
            paper_thickness_mm,
            32,
        )
    } else {
        diagnose_scheduled_cycle_path_v1(
            geometry,
            audit,
            pose.fixed_face(),
            &generated,
            &closure,
            32,
        )
    };
    if continuous.continuous_certificate_model_id().is_none() {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let expected = ori_collision::certify_scheduled_cycle_transition_v1(
        geometry,
        audit,
        pose.fixed_face(),
        &generated,
        &closure,
        32,
        source,
        target,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let closure_leaf_count = closure.leaves().len();
    let closure_max_depth = closure
        .leaves()
        .iter()
        .map(|(depth, _, _)| *depth)
        .max()
        .unwrap_or(0);
    let total_hinge_count = geometry.hinges().len();
    let checked_hinge_count = closure
        .leaves()
        .first()
        .map_or(0, |(_, _, leaf)| leaf.checked_hinges().len());
    let source_layer_order = layer_capability.as_ref().map(|capability| {
        capability
            .snapshot()
            .face_pair_orders
            .iter()
            .map(|order| LayerOrderPairDtoV1 {
                lower_face: order.lower_face.face_id,
                upper_face: order.upper_face.face_id,
            })
            .collect::<Vec<_>>()
    });
    let layer_transport = if let (Some(capability), Some(_source_orders)) =
        (layer_capability.as_ref(), source_layer_order.as_ref())
    {
        let source = capability.snapshot();
        let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            geometry,
            audit,
            pose.fixed_face(),
            generated.schedule(),
            &closure,
            paper_thickness_mm,
            32,
        )
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        let layer_records = source
            .overlap_cells
            .iter()
            .try_fold(0usize, |sum, cell| {
                sum.checked_add(cell.bottom_to_top_faces.len())
            })
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let boundary_samples = source
            .overlap_cells
            .iter()
            .try_fold(0usize, |sum, cell| {
                cell.exact_boundary
                    .len()
                    .checked_mul(cell.bottom_to_top_faces.len())
                    .and_then(|work| sum.checked_add(work))
            })
            .and_then(|work| work.checked_mul(closure_leaf_count + 1))
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        Some(
            certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                geometry,
                audit,
                source,
                schedule: generated.schedule(),
                closure: &closure,
                positive_continuous: &positive,
                paper_thickness_mm,
                tolerance: ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                limits: GeneralCellTransportLimitsV1 {
                    max_transitions: closure_leaf_count + 1,
                    max_cells: source.overlap_cells.len(),
                    max_layer_records: layer_records,
                    max_boundary_samples: boundary_samples,
                },
            })
            .map_err(|error| match error {
                ori_collision::GeneralCellTransportErrorV1::ResourceLimit => {
                    CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
                }
                _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
            })?,
        )
    } else {
        None
    };
    let layer_transport_metadata = layer_transport.as_ref().map(|certificate| {
        (
            certificate.model_id(),
            certificate.transition_hashes().len(),
            certificate.pair_order_count(),
            certificate
                .target_order_hash()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        )
    });
    let persisted_layer_order_pairs = source_layer_order
        .as_ref()
        .map(|orders| {
            orders
                .iter()
                .map(|order| (order.lower_face, order.upper_face))
                .collect()
        })
        .unwrap_or_default();
    emit_current_cycle_progress_v1(app, progress_request_id, 1, 1);
    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != generation {
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    let target_angles = requested
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect();
    let token = super::stacked_fold_transaction::install_pending_current_cycle_pose_v1(
        &transaction_state,
        super::stacked_fold_transaction::PendingCurrentCyclePosePremisesV1 {
            expected_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            expected_source_fingerprint: ori_foldability::fold_model_fingerprint_v1(
                project.editor.pattern(),
                project.editor.paper(),
            )
            .0,
            expected_pose_generation: pose_capability.generation(),
            expected_layer_generation: 0,
            geometry: geometry.clone(),
            audit: audit.clone(),
            fixed_face: pose.fixed_face(),
            generated,
            closure,
            expected,
            continuous,
            layer_transport,
            layer_order_pairs: persisted_layer_order_pairs,
            target_angles,
        },
        pose_capability,
        layer_capability,
    )?;
    let source_layer_order = source_layer_order.unwrap_or_default();
    let (layer_model_id, layer_transition_count, layer_pair_count, layer_target_hash) =
        layer_transport_metadata.map_or((None, 0, 0, None), |value| {
            (Some(value.0), value.1, value.2, Some(value.3))
        });
    Ok(CurrentCyclePosePreviewResponseV1 {
        version: 1,
        transaction_token: token,
        source_revision: request.expected_revision,
        target_revision: request.expected_revision + 1,
        closure_leaf_count,
        closure_max_depth,
        checked_hinge_count,
        total_hinge_count,
        continuous_path_certified: true,
        continuous_layer_transport_model_id: layer_model_id,
        continuous_layer_transition_count: layer_transition_count,
        continuous_layer_pair_order_count: layer_pair_count,
        continuous_layer_target_order_sha256: layer_target_hash,
        target_layer_order: source_layer_order.clone(),
        source_layer_order,
        authorizes_project_mutation: false,
    })
}

fn emit_current_cycle_status_v1(
    app: Option<&AppHandle>,
    request_id: Option<&str>,
    status: &'static str,
    completed_work: usize,
) {
    let (Some(app), Some(request_id)) = (app, request_id) else {
        return;
    };
    let _ = app.emit(
        CURRENT_CYCLE_POSE_PROGRESS_EVENT_V1,
        CurrentCyclePoseProgressDtoV1 {
            version: 1,
            request_id: request_id.to_owned(),
            status,
            completed_work,
            total_work: 2,
            authorizes_project_mutation: false,
        },
    );
}

fn emit_current_cycle_terminal_v1(app: &AppHandle, request_id: &str, status: &'static str) {
    emit_current_cycle_status_v1(Some(app), Some(request_id), status, 2);
}

fn begin_stacked_fold_read_generation_v1() -> Result<u64, String> {
    STACKED_FOLD_READ_GENERATION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?
        .checked_add(1)
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
}

fn validate_progress_request_id_v1(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        Some(value) if value.is_empty() || value.len() > 128 || !value.is_ascii() => {
            Err(INVALID_REQUEST_MESSAGE.to_owned())
        }
        value => Ok(value),
    }
}

fn emit_current_cycle_progress_v1(
    app: Option<&AppHandle>,
    request_id: Option<&str>,
    explored_state_count: usize,
    evaluated_transition_count: usize,
) {
    let (Some(app), Some(request_id)) = (app, request_id) else {
        return;
    };
    let _ = app.emit(
        STACKED_FOLD_READ_PROGRESS_EVENT_V1,
        StackedFoldReadProgressDtoV1 {
            version: 1,
            request_id: request_id.to_owned(),
            explored_state_count,
            evaluated_transition_count,
            state_limit: 32,
            transition_limit: 64,
            authorizes_project_mutation: false,
        },
    );
}

fn validate_certified_path_graph_v1(
    request: &CertifiedPathGraphRequestV1,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<Vec<ori_kinematics::CanonicalHingeAngles>, &'static str> {
    if request.version != 1
        || request.states.is_empty()
        || request.states.len() > ori_collision::MAX_CERTIFIED_PATH_GRAPH_STATES_V1
        || request.transitions.is_empty()
        || request.transitions.len() > MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1
        || request.states.iter().any(|state| {
            state.entries.is_empty() || state.entries.len() > MAX_STACKED_FOLD_REQUEST_HINGES_V1
        })
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE);
    }
    if request.source_state != 0
        || request.target_state >= request.states.len()
        || request.target_state == request.source_state
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let mut states = Vec::with_capacity(request.states.len());
    for state in &request.states {
        let angles = ori_kinematics::CanonicalHingeAngles::new(
            state
                .entries
                .iter()
                .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?,
        )
        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?;
        if angles.as_slice().len() != live.as_slice().len() {
            return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
        }
        states.push(angles);
    }
    if states.first() != Some(live)
        || states.iter().enumerate().any(|(index, state)| {
            states[..index]
                .iter()
                .any(|previous| previous.as_slice() == state.as_slice())
        })
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let mut canonical_edges = request
        .transitions
        .iter()
        .map(|edge| (edge.source_state, edge.target_state))
        .collect::<Vec<_>>();
    if canonical_edges.iter().any(|(source, target)| {
        *source >= states.len() || *target >= states.len() || source == target
    }) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    canonical_edges.sort_unstable();
    if canonical_edges.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    Ok(states)
}

fn validate_linear_candidate_angles_v1(
    request: &LinearCandidateRequestV1,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<
    (
        ori_kinematics::CanonicalHingeAngles,
        ori_kinematics::CanonicalHingeAngles,
    ),
    (),
> {
    if request.version != 1 {
        return Err(());
    }
    let collect = |requested: bool| {
        ori_kinematics::CanonicalHingeAngles::new(
            request
                .entries
                .iter()
                .map(|entry| {
                    ori_kinematics::HingeAngle::new(
                        entry.edge,
                        if requested {
                            entry.requested_angle_degrees
                        } else {
                            entry.initial_angle_degrees
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ())?,
        )
        .map_err(|_| ())
    };
    let initial = collect(false)?;
    if initial != *live {
        return Err(());
    }
    Ok((initial, collect(true)?))
}

fn prepare_requested_cycle_schedule_v1(
    request: &CycleScheduleRequestV1,
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    audit: &ori_kinematics::MaterialHingeGraphAudit,
    fixed_face: FaceId,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<ori_kinematics::CanonicalCycleScheduleV1, &'static str> {
    if request.version != 1 || request.entries.len() != live.as_slice().len() {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let rational = |value: RationalCoefficientRequestV1| {
        (value.denominator != 0)
            .then_some(ori_kinematics::RationalCoefficientV1 {
                numerator: value.numerator,
                denominator: value.denominator,
            })
            .ok_or(CYCLE_PATH_UNSUPPORTED_MESSAGE)
    };
    let inputs = request
        .entries
        .iter()
        .map(|entry| {
            Ok(ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge: entry.edge,
                u_domain: [rational(entry.u_domain[0])?, rational(entry.u_domain[1])?],
                numerator_power_coefficients: entry
                    .numerator_power_coefficients
                    .iter()
                    .copied()
                    .map(rational)
                    .collect::<Result<Vec<_>, _>>()?,
                denominator_power_coefficients: entry
                    .denominator_power_coefficients
                    .iter()
                    .copied()
                    .map(rational)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let limits = production_cycle_schedule_limits_v1();
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        geometry, audit, fixed_face, inputs, limits,
    )
    .map_err(|error| match error {
        ori_kinematics::CycleSchedulePrepareErrorV1::ResourceLimit => CYCLE_PATH_RESOURCE_MESSAGE,
        _ => CYCLE_PATH_UNSUPPORTED_MESSAGE,
    })?;
    for (upper, expected) in [false, true].into_iter().zip([
        live.as_slice()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<Vec<_>>(),
        request
            .entries
            .iter()
            .map(|entry| (entry.edge, entry.requested_angle_degrees))
            .collect(),
    ]) {
        let endpoint = schedule
            .evaluate_endpoint_angle_box(upper, limits)
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?;
        if endpoint.len() != expected.len()
            || endpoint
                .iter()
                .zip(expected)
                .any(|((edge, interval), expected)| {
                    *edge != expected.0
                        || !expected.1.is_finite()
                        || expected.1 < interval.lower()
                        || expected.1 > interval.upper()
                        || interval.upper() - interval.lower()
                            > ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1
                })
        {
            return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
        }
    }
    Ok(schedule)
}

fn generate_even_opposite_pair_schedule_v1(
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    audit: &ori_kinematics::MaterialHingeGraphAudit,
    fixed_face: FaceId,
    live: &ori_kinematics::CanonicalHingeAngles,
    target: &ori_kinematics::CanonicalHingeAngles,
) -> Result<ori_kinematics::CanonicalCycleScheduleV1, &'static str> {
    let changed = live
        .as_slice()
        .iter()
        .zip(target.as_slice())
        .filter_map(|(source, target)| {
            (source.angle_degrees().to_bits() != target.angle_degrees().to_bits())
                .then_some(target.edge())
        })
        .collect::<Vec<_>>();
    if changed.len() != 2
        || !ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(geometry, audit, 128)
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?
            .iter()
            .any(|pair| pair.iter().all(|edge| changed.contains(edge)))
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let requested = target
        .as_slice()
        .iter()
        .find(|entry| entry.edge() == changed[0])
        .map(|entry| entry.angle_degrees())
        .ok_or(CYCLE_PATH_UNSUPPORTED_MESSAGE)?;
    let (endpoint_numerator, endpoint_denominator) =
        bounded_primitive_endpoint_ratio_for_angle_v1(requested)?;
    let entries = live
        .as_slice()
        .iter()
        .map(|source| {
            let active = changed.contains(&source.edge());
            CycleScheduleEntryRequestV1 {
                edge: source.edge(),
                u_domain: [
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: if active {
                    vec![
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: endpoint_numerator,
                            denominator: 1,
                        },
                    ]
                } else {
                    vec![RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                    numerator: if active {
                        endpoint_denominator as i64
                    } else {
                        1
                    },
                    denominator: 1,
                }],
                requested_angle_degrees: if active {
                    requested
                } else {
                    source.angle_degrees()
                },
            }
        })
        .collect();
    prepare_requested_cycle_schedule_v1(
        &CycleScheduleRequestV1 {
            version: 1,
            entries,
            endpoint_denominator: None,
        },
        geometry,
        audit,
        fixed_face,
        live,
    )
}

fn bounded_primitive_endpoint_ratio_v1(
    numerator: i64,
    denominator: u64,
) -> Result<(i64, u64), &'static str> {
    if numerator == 0 || denominator == 0 || denominator > 64 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let magnitude = numerator.unsigned_abs();
    if magnitude > 64 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let mut left = magnitude;
    let mut right = denominator;
    while right != 0 {
        (left, right) = (right, left % right);
    }
    if left != 1 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    Ok((numerator, denominator))
}

fn bounded_primitive_endpoint_ratio_for_angle_v1(
    requested_angle_degrees: f64,
) -> Result<(i64, u64), &'static str> {
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees == 0.0
        || requested_angle_degrees.abs() >= 180.0
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let sign = if requested_angle_degrees.is_sign_positive() {
        1_i64
    } else {
        -1_i64
    };
    (1_u64..=64)
        .flat_map(|denominator| (1_i64..=64).map(move |n| (n * sign, denominator)))
        .filter_map(|ratio| bounded_primitive_endpoint_ratio_v1(ratio.0, ratio.1).ok())
        .find(|(numerator, denominator)| {
            (requested_angle_degrees
                - 2.0 * (*numerator as f64).atan2(*denominator as f64).to_degrees())
            .abs()
                <= 1.0e-12
        })
        .ok_or(CYCLE_PATH_UNSUPPORTED_MESSAGE)
}

fn production_cycle_schedule_limits_v1() -> CycleScheduleLimitsV1 {
    let defaults = CycleScheduleLimitsV1::default();
    CycleScheduleLimitsV1 {
        max_hinges: 256,
        max_work: 1_152,
        ..defaults
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CycleScheduleRequestV1 {
    version: u32,
    entries: Vec<CycleScheduleEntryRequestV1>,
    #[serde(default)]
    endpoint_denominator: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(Clone)]
struct CycleScheduleEntryRequestV1 {
    edge: ori_domain::EdgeId,
    u_domain: [RationalCoefficientRequestV1; 2],
    numerator_power_coefficients: Vec<RationalCoefficientRequestV1>,
    denominator_power_coefficients: Vec<RationalCoefficientRequestV1>,
    requested_angle_degrees: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RationalCoefficientRequestV1 {
    numerator: i64,
    denominator: u64,
}

fn validate_request_resource_shape_v1(
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

fn validate_exact_dyadic_candidate_path_v1(
    request: &ExactDyadicPathRequestV1,
) -> Result<(), &'static str> {
    if request.version != 1 || request.segments.is_empty() {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE);
    }
    let segments = request
        .segments
        .iter()
        .map(|segment| {
            let point = |value: ExactDyadicPointRequestV1| ori_collision::DyadicPointV1 {
                x_numerator: value.x_numerator,
                y_numerator: value.y_numerator,
                denominator_power: value.denominator_power,
            };
            ori_collision::DyadicSegmentV1 {
                start: point(segment.start),
                end: point(segment.end),
            }
        })
        .collect::<Vec<_>>();
    match ori_collision::classify_exact_dyadic_path_self_intersection_v1(
        &segments,
        ori_collision::ExactDyadicIntersectionLimitsV1 {
            max_denominator_power: request.max_denominator_power,
            max_integer_bits: request.max_integer_bits,
        },
        request.max_pair_tests,
    ) {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(CYCLE_PATH_UNCERTIFIED_MESSAGE),
        Err(ori_collision::ExactDyadicPathIntersectionErrorV1::ResourceLimit) => {
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        }
        Err(ori_collision::ExactDyadicPathIntersectionErrorV1::Cancelled) => Err(CANCELLED_MESSAGE),
        Err(ori_collision::ExactDyadicPathIntersectionErrorV1::InvalidSegment) => {
            Err(CYCLE_PATH_UNCERTIFIED_MESSAGE)
        }
    }
}

fn requires_graph_schedule_boundary_v1(
    topology_requires_closure: bool,
    has_explicit_cycle_schedule: bool,
) -> bool {
    topology_requires_closure || has_explicit_cycle_schedule
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum StackedFoldReadSupportDto {
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
struct StackedFoldReadBindingDto {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: u64,
    pose_generation: u64,
    layer_order_generation: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadCellDto {
    cell_key_sha256: String,
    bottom_to_top_faces: Vec<FaceId>,
    boundary_world: Vec<[f64; 3]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldMaterialSegmentDto {
    face_id: FaceId,
    start: [f64; 2],
    end: [f64; 2],
    fixed_side: &'static str,
    assignment: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadWorkDto {
    scanned_cells: usize,
    total_boundary_vertices: usize,
    total_layer_records: usize,
    orientation_tests: usize,
    exact_arithmetic_operations: usize,
    maximum_exact_integer_bits: usize,
    total_exact_integer_bits: usize,
    retained_cells: usize,
    retained_target_faces: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldTopologyProofDto {
    target_fingerprint_sha256: String,
    target_vertex_count: usize,
    target_edge_count: usize,
    target_boundary_vertex_count: usize,
    lineage_record_count: usize,
    source_edge_subdivision_count: usize,
    expected_crease_subdivision_count: usize,
    target_material_face_count: usize,
    target_hinge_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldEndpointCollisionDto {
    expected_pair_count: usize,
    separated_pair_count: usize,
    touching_pair_count: usize,
    allowed_pair_count: usize,
    penetrating_pair_count: usize,
    indeterminate_pair_count: usize,
    has_blocking_hold: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldContinuousPathDto {
    model_id: &'static str,
    continuous_certificate_model_id: Option<&'static str>,
    sampled_pose_count: usize,
    sampled_nonblocking_pose_count: usize,
    interval_leaf_count: usize,
    interval_pair_work: usize,
    interval_candidate_limit: usize,
    positive_endpoint_candidate_count: usize,
    positive_endpoint_exact_pair_calls: usize,
    positive_endpoint_candidate_limit: usize,
    closure_required: bool,
    closure_leaf_count: usize,
    closure_pair_work: usize,
    first_closure_failure_angle_degrees: Option<f64>,
    first_sampled_blocking_angle_degrees: Option<f64>,
    requested_angle_degrees: f64,
    continuous_clearance_certified: bool,
    safe_stop_angle_degrees: f64,
    authorizes_project_mutation: bool,
    paper_thickness_mm: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CertifiedPathGraphPreviewDto {
    model_id: &'static str,
    version: u32,
    source_fingerprint_sha256: String,
    target_fingerprint_sha256: String,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    edges: Vec<CertifiedPathGraphEdgeDto>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CertifiedPathGraphEdgeDto {
    source_fingerprint_sha256: String,
    target_fingerprint_sha256: String,
    schedule_certificate_sha256: String,
    collision_certificate_sha256: String,
    closure_certificate_sha256: String,
    hinges: Vec<ori_domain::EdgeId>,
}

enum StackedFoldPathAnalysis {
    Tree(ori_collision::StackedFoldBoundedPathDiagnosticV1),
    Graph {
        diagnostic: ori_collision::StackedFoldCyclePathDiagnosticV1,
        requested_angle_degrees: f64,
    },
}

enum NativeStackedFoldPremises {
    Tree(super::stacked_fold_transaction::PendingStackedFoldPremises),
    Graph(super::stacked_fold_transaction::PendingStackedFoldGraphPremises),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldFlatEndpointLayerOrderDto {
    applicable: bool,
    certified: bool,
    material_face_count: usize,
    overlap_cell_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum StackedFoldTransactionFailureClassDto {
    ContinuousPathUncertified,
    TargetLayerOrderUnavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldTransactionProposalDto {
    transaction_token: Option<ProjectId>,
    source_project_id: ProjectId,
    source_revision: u64,
    target_revision: u64,
    source_fingerprint_sha256: String,
    target_fingerprint_sha256: String,
    added_vertex_count: usize,
    added_edge_count: usize,
    mountain_crease_count: usize,
    valley_crease_count: usize,
    timeline_step_count: usize,
    timeline_complete_hinge_angle_count: usize,
    requested_angle_degrees: f64,
    ready_for_atomic_apply: bool,
    failure_classes: Vec<StackedFoldTransactionFailureClassDto>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveGraphHingeAngleDto {
    edge: ori_domain::EdgeId,
    initial_angle_degrees: f64,
}

fn live_hinge_registry(angles: &[ori_kinematics::HingeAngle]) -> Vec<LiveGraphHingeAngleDto> {
    angles
        .iter()
        .map(|angle| LiveGraphHingeAngleDto {
            edge: angle.edge(),
            initial_angle_degrees: angle.angle_degrees(),
        })
        .collect()
}

#[tauri::command]
pub(super) async fn read_live_hinge_registry_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    request: LiveHingeRegistryRequestV1,
) -> Result<LiveHingeRegistryResponseV1, String> {
    read_live_hinge_registry_inner(&app_state, &foldability_state, request).await
}

async fn read_live_hinge_registry_inner(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    request: LiveHingeRegistryRequestV1,
) -> Result<LiveHingeRegistryResponseV1, String> {
    let worker_permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| BUSY_MESSAGE.to_owned())?;
    let (paper, pattern, capability, layer_capability, source_fingerprint) = {
        let project = lock_project(&app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
        {
            return Err(STALE_MESSAGE.to_owned());
        }
        let capability = project
            .applied_pose_authority
            .capture_capability(&project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        (
            project.editor.paper().clone(),
            project.editor.pattern().clone(),
            capability,
            capture_current_layer_order_capability(&foldability_state, &project)
                .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
                .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?,
            project.editor.fold_model_fingerprint_v1(),
        )
    };
    let expected_instance_id = request.expected_project_instance_id;
    let expected_project_id = request.expected_project_id;
    let expected_revision = request.expected_revision;
    let (capability, layer_capability, source_fingerprint, fingerprint, entries) =
        tauri::async_runtime::spawn_blocking(move || {
            let first = Point3::new(request.first[0], request.first[1], request.first[2])
                .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
            let second = Point3::new(request.second[0], request.second[1], request.second[2])
                .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
            let candidate = StackedFoldLinearCandidateV1::new(
                first,
                second,
                request.fixed_side.into(),
                request.rotation_direction.into(),
                request.requested_angle_degrees,
            )
            .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
            let binding = StackedFoldReadBindingV1::new(
                expected_instance_id,
                expected_project_id,
                expected_revision,
                capability.generation(),
                layer_capability.generation(),
            );
            let input = FlatEndpointLayerOrderInputV1 {
                identity_namespace: binding.project_id(),
                source_revision: binding.source_revision(),
                paper: &paper,
                pattern: &pattern,
                model: capability.model(),
                pose: capability.pose(),
                layer_order: layer_capability.snapshot(),
            };
            let limits = StackedFoldReadLimitsV1::default();
            let guard = capture_stacked_fold_read_guard_v1(binding, input, limits)
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let proposal =
                propose_linear_stacked_fold_read_v1(&guard, binding, input, candidate, limits)
                    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let material_map = reverse_map_linear_stacked_fold_material_v1(
                &proposal,
                &guard,
                binding,
                input,
                limits,
                StackedFoldMaterialMapLimitsV1::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let expected_creases = material_map
                .segments()
                .iter()
                .map(|segment| ExpectedStackedFoldCreaseV1 {
                    start: segment.start(),
                    end: segment.end(),
                    kind: segment.assignment(),
                })
                .collect::<Vec<_>>();
            let prepared = prepare_stacked_fold_geometry_candidate_v1(
                binding.project_id(),
                binding.source_revision(),
                &pattern,
                &paper,
                layer_capability.snapshot(),
                &expected_creases,
                StackedFoldTopologyBuildLimitsV1::default(),
                FaceLineageLimits::default(),
                StackedFoldGeometryLimitsV1::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let fingerprint = prepared.proof().lineage().target_fingerprint().to_hex();
            let audited = prepare_stacked_fold_target_graph_audit_v1(
                prepared,
                TreeKinematicsLimits::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let initial = prepare_stacked_fold_initial_graph_pose_v1(
                audited,
                capability.model(),
                capability.pose(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let entries = live_hinge_registry(initial.pose().hinge_angles().as_slice());
            if entries.len() > 64 {
                return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
            }
            drop(worker_permit);
            Ok::<_, String>((
                capability,
                layer_capability,
                source_fingerprint,
                fingerprint,
                entries,
            ))
        })
        .await
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())??;
    {
        let project = lock_project(&app_state).map_err(|_| STALE_MESSAGE.to_owned())?;
        if project.editor.fold_model_fingerprint_v1() != source_fingerprint
            || project
                .applied_pose_authority
                .revalidate_capability(&project, &capability)
                .map_err(|_| STALE_MESSAGE.to_owned())?
                .is_none()
            || revalidate_current_layer_order_capability(
                &foldability_state,
                &project,
                &layer_capability,
            )
            .map_err(|_| STALE_MESSAGE.to_owned())?
            .is_none()
        {
            return Err(STALE_MESSAGE.to_owned());
        }
    }
    Ok(LiveHingeRegistryResponseV1 {
        version: 1,
        project_instance_id: expected_instance_id,
        project_id: expected_project_id,
        revision: expected_revision,
        pose_generation: capability.generation(),
        graph_fingerprint_sha256: fingerprint,
        entries,
        authorizes_project_mutation: false,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldReadResponse {
    guard_model_id: &'static str,
    proposal_model_id: &'static str,
    material_map_model_id: &'static str,
    binding: StackedFoldReadBindingDto,
    support: StackedFoldReadSupportDto,
    crossed_cells: Vec<StackedFoldReadCellDto>,
    target_faces: Vec<FaceId>,
    material_segments: Vec<StackedFoldMaterialSegmentDto>,
    topology_proof: StackedFoldTopologyProofDto,
    live_graph_hinge_angles: Vec<LiveGraphHingeAngleDto>,
    endpoint_collision: StackedFoldEndpointCollisionDto,
    continuous_path: StackedFoldContinuousPathDto,
    certified_path_graph: Option<CertifiedPathGraphPreviewDto>,
    flat_endpoint_layer_order: StackedFoldFlatEndpointLayerOrderDto,
    transaction_proposal: StackedFoldTransactionProposalDto,
    work: StackedFoldReadWorkDto,
    authorizes_project_mutation: bool,
    authorizes_apply_stacked_fold: bool,
}

#[tauri::command]
pub(super) async fn propose_current_stacked_fold_read(
    app: AppHandle,
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, super::stacked_fold_transaction::StackedFoldTransactionState>,
    request: StackedFoldReadRequest,
) -> Result<StackedFoldReadResponse, String> {
    propose_current_stacked_fold_read_inner(
        Some(&app),
        &app_state,
        &foldability_state,
        &transaction_state,
        request,
    )
    .await
}

async fn propose_current_stacked_fold_read_inner(
    app: Option<&AppHandle>,
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    transaction_state: &super::stacked_fold_transaction::StackedFoldTransactionState,
    request: StackedFoldReadRequest,
) -> Result<StackedFoldReadResponse, String> {
    validate_request_resource_shape_v1(&request).map_err(str::to_owned)?;
    let worker_permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| BUSY_MESSAGE.to_owned())?;
    // A rejected busy request must not cancel the permit owner.
    let analysis_generation = STACKED_FOLD_READ_GENERATION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
            generation.checked_add(1)
        })
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?
        .checked_add(1)
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    let progress_request_id = request.progress_request_id.clone().filter(|value| {
        !value.is_empty() && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_graphic())
    });
    let (paper, pattern, pose_capability, layer_capability, binding) = {
        let project = lock_project(&app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
        {
            return Err(STALE_MESSAGE.to_owned());
        }
        let pose_capability = project
            .applied_pose_authority
            .capture_capability(&project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        let layer_capability = capture_current_layer_order_capability(&foldability_state, &project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        let binding = StackedFoldReadBindingV1::new(
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            pose_capability.generation(),
            layer_capability.generation(),
        );
        (
            project.editor.paper().clone(),
            project.editor.pattern().clone(),
            pose_capability,
            layer_capability,
            binding,
        )
    };

    let first = Point3::new(request.first[0], request.first[1], request.first[2])
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let second = Point3::new(request.second[0], request.second[1], request.second[2])
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let candidate = StackedFoldLinearCandidateV1::new(
        first,
        second,
        request.fixed_side.into(),
        request.rotation_direction.into(),
        request.requested_angle_degrees,
    )
    .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let paper_thickness_mm = paper.thickness_mm;
    let progress_app = app.cloned();
    let analysis = tauri::async_runtime::spawn_blocking(move || {
        let input = FlatEndpointLayerOrderInputV1 {
            identity_namespace: binding.project_id(),
            source_revision: binding.source_revision(),
            paper: &paper,
            pattern: &pattern,
            model: pose_capability.model(),
            pose: pose_capability.pose(),
            layer_order: layer_capability.snapshot(),
        };
        let limits = StackedFoldReadLimitsV1::default();
        let guard = capture_stacked_fold_read_guard_v1(binding, input, limits)
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let proposal =
            propose_linear_stacked_fold_read_v1(&guard, binding, input, candidate, limits)
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let material_map = reverse_map_linear_stacked_fold_material_v1(
            &proposal,
            &guard,
            binding,
            input,
            limits,
            StackedFoldMaterialMapLimitsV1::default(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let expected_creases = material_map
            .segments()
            .iter()
            .map(|segment| ExpectedStackedFoldCreaseV1 {
                start: segment.start(),
                end: segment.end(),
                kind: segment.assignment(),
            })
            .collect::<Vec<_>>();
        let prepared_geometry = prepare_stacked_fold_geometry_candidate_v1(
            binding.project_id(),
            binding.source_revision(),
            &pattern,
            &paper,
            layer_capability.snapshot(),
            &expected_creases,
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let audited_target = prepare_stacked_fold_target_graph_audit_v1(
            prepared_geometry,
            TreeKinematicsLimits::default(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        if requires_graph_schedule_boundary_v1(
            audited_target.requires_closure_certificate(),
            request.cycle_schedule_v1.is_some(),
        ) {
            let initial = prepare_stacked_fold_initial_graph_pose_v1(
                audited_target,
                pose_capability.model(),
                pose_capability.pose(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let path_variant_count = usize::from(request.cycle_schedule_v1.is_some())
                + usize::from(request.linear_candidate_v1.is_some())
                + usize::from(request.certified_path_graph_v1.is_some());
            if path_variant_count != 1 {
                return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
            }
            let supplied_cycle_candidate = if let Some(cycle) = request.cycle_schedule_v1.as_ref() {
                let schedule = prepare_requested_cycle_schedule_v1(
                    cycle,
                    initial.target().hinge_geometry(),
                    initial.target().audit(),
                    initial.pose().fixed_face(),
                    initial.pose().hinge_angles(),
                )
                .map_err(str::to_owned)?;
                let requested = ori_kinematics::CanonicalHingeAngles::new(
                    cycle.entries.iter().map(|entry| {
                        ori_kinematics::HingeAngle::new(entry.edge, entry.requested_angle_degrees)
                    }).collect::<Result<Vec<_>, _>>()
                        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
                ).map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
                Some((
                    ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
                        schedule,
                        initial.pose().hinge_angles(),
                        &requested,
                    ).map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
                    requested,
                ))
            } else {
                None
            };
            let (
                initial_angles,
                requested_angles,
                all_requested_flat,
                certified_path_graph,
                certified_path_certificate,
                certified_path_edges,
            ) =
                if let Some(graph) = request.certified_path_graph_v1.as_ref() {
                    let states =
                        validate_certified_path_graph_v1(graph, initial.pose().hinge_angles())
                            .map_err(str::to_owned)?;
                    if states[graph.target_state]
                        .as_slice()
                        .iter()
                        .zip(states[graph.source_state].as_slice())
                        .any(|(target, source)| {
                            target.angle_degrees().to_bits()
                                != source.angle_degrees().to_bits()
                                && target.angle_degrees().to_bits()
                                    != candidate.requested_angle_degrees().to_bits()
                        })
                    {
                        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
                    }
                    let fingerprints = states
                        .iter()
                        .map(pose_state_fingerprint_v1)
                        .collect::<Vec<_>>();
                    let candidates = graph
                        .transitions
                        .iter()
                        .enumerate()
                        .map(|(index, edge)| {
                            let mut key = [0_u8; 32];
                            key[24..].copy_from_slice(&(index as u64).to_be_bytes());
                            ori_collision::CertifiedPathTransitionCandidateV1 {
                                source: fingerprints[edge.source_state],
                                target: fingerprints[edge.target_state],
                                candidate_key: key,
                            }
                        })
                        .collect::<Vec<_>>();
                    let index_by_fingerprint = fingerprints
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(index, fingerprint)| (fingerprint, index))
                        .collect::<std::collections::BTreeMap<_, _>>();
                    let mut resource_exhausted = false;
                    let mut oracle_edges = std::collections::BTreeMap::new();
                    let progress_app = progress_app.clone();
                    let progress_request_id = progress_request_id.clone();
                    let searched =
                        ori_collision::search_certified_pose_graph_with_progress_v1(
                        &fingerprints,
                        &candidates,
                        fingerprints[graph.source_state],
                        fingerprints[graph.target_state],
                        || {
                            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire)
                                == analysis_generation
                        },
                        |progress| {
                            #[cfg(test)]
                            if progress_request_id
                                .as_deref()
                                .and_then(|value| value.strip_prefix("test-cancel-after-"))
                                .and_then(|value| value.parse::<usize>().ok())
                                .is_some_and(|limit| {
                                    progress.evaluated_transition_count >= limit
                                })
                            {
                                let _ = cancel_current_stacked_fold_read_v1();
                            }
                            if let Some(request_id) = progress_request_id.as_ref() {
                                if let Some(progress_app) = progress_app.as_ref() { let _ = progress_app.emit(
                                    STACKED_FOLD_READ_PROGRESS_EVENT_V1,
                                    StackedFoldReadProgressDtoV1 {
                                        version: 1,
                                        request_id: request_id.clone(),
                                        explored_state_count: progress.explored_state_count,
                                        evaluated_transition_count:
                                            progress.evaluated_transition_count,
                                        state_limit: progress.state_limit,
                                        transition_limit: progress.transition_limit,
                                        authorizes_project_mutation: false,
                                    },
                                ); }
                            }
                        },
                        |edge| {
                            let source_index = *index_by_fingerprint.get(&edge.source)?;
                            let target_index = *index_by_fingerprint.get(&edge.target)?;
                            let generated = match generate_linear_multi_hinge_path_candidate_v1(
                                initial.target().hinge_geometry(),
                                initial.target().audit(),
                                initial.pose().fixed_face(),
                                &states[source_index],
                                &states[target_index],
                                MultiHingePathCandidateLimitsV1::default(),
                            ) {
                                Ok(value) => value,
                                Err(ori_kinematics::MultiHingePathCandidateErrorV1::ResourceLimit) => {
                                    resource_exhausted = true;
                                    return None;
                                }
                                Err(_) => return None,
                            };
                            let cycle_limits = CycleScheduleLimitsV1::default();
                            let closure = match initial.target().hinge_geometry()
                                .prove_dyadic_schedule_closure_v1(
                                    initial.target().audit(),
                                    initial.pose().fixed_face(),
                                    generated.schedule(),
                                    ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                                    DyadicIntervalClosureLimitsV1 {
                                        max_depth: 8,
                                        max_leaves: 256,
                                        max_work: cycle_limits.max_work,
                                        schedule_limits: CycleScheduleLimitsV1 {
                                            max_degree: 1,
                                            ..cycle_limits
                                        },
                                    },
                                ) {
                                    Ok(value) => value,
                                    Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit) => {
                                        resource_exhausted = true;
                                        return None;
                                    }
                                    Err(_) => return None,
                                };
                            let expected = ori_collision::certify_scheduled_cycle_transition_v1(
                                initial.target().hinge_geometry(),
                                initial.target().audit(),
                                initial.pose().fixed_face(),
                                &generated,
                                &closure,
                                StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
                                edge.source,
                                edge.target,
                            )?;
                            oracle_edges.insert(
                                (edge.source, edge.target),
                                super::stacked_fold_transaction::PendingCertifiedPathEdgeV1 {
                                    generated,
                                    closure,
                                    expected,
                                    target_angles: states[target_index]
                                        .as_slice()
                                        .iter()
                                        .map(|angle| (angle.edge(), angle.angle_degrees()))
                                        .collect(),
                                },
                            );
                            Some(expected)
                        },
                    );
                    let certificate = match searched {
                        ori_collision::CertifiedPathGraphSearchResultV1::Certified(value) => value,
                        ori_collision::CertifiedPathGraphSearchResultV1::Indeterminate {
                            reason: ori_collision::CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
                            ..
                        } => return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned()),
                        ori_collision::CertifiedPathGraphSearchResultV1::Indeterminate {
                            reason: ori_collision::CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                            ..
                        } => return Err(CANCELLED_MESSAGE.to_owned()),
                        ori_collision::CertifiedPathGraphSearchResultV1::Indeterminate { .. }
                            if resource_exhausted =>
                        {
                            return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
                        }
                        ori_collision::CertifiedPathGraphSearchResultV1::Indeterminate { .. } => {
                            return Err(CYCLE_PATH_NO_CERTIFIED_PATH_MESSAGE.to_owned());
                        }
                    };
                    let registry_edges = certificate
                        .edges()
                        .iter()
                        .map(|edge| {
                            oracle_edges
                                .remove(&(edge.source(), edge.target()))
                                .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let edges = certificate
                        .edges()
                        .iter()
                        .map(|edge| {
                            let source_index = index_by_fingerprint[&edge.source()];
                            let target_index = index_by_fingerprint[&edge.target()];
                            let hinges = states[source_index]
                                .as_slice()
                                .iter()
                                .zip(states[target_index].as_slice())
                                .filter_map(|(source, target)| {
                                    (source.angle_degrees().to_bits()
                                        != target.angle_degrees().to_bits())
                                        .then_some(source.edge())
                                })
                                .collect();
                            CertifiedPathGraphEdgeDto {
                                source_fingerprint_sha256: lowercase_hex(edge.source()),
                                target_fingerprint_sha256: lowercase_hex(edge.target()),
                                schedule_certificate_sha256: lowercase_hex(
                                    edge.schedule_certificate(),
                                ),
                                collision_certificate_sha256: lowercase_hex(
                                    edge.collision_certificate(),
                                ),
                                closure_certificate_sha256: lowercase_hex(
                                    edge.closure_certificate(),
                                ),
                                hinges,
                            }
                        })
                        .collect();
                    let preview = CertifiedPathGraphPreviewDto {
                        model_id: certificate.model_id(),
                        version: u32::from(certificate.version()),
                        source_fingerprint_sha256: lowercase_hex(certificate.source()),
                        target_fingerprint_sha256: lowercase_hex(certificate.target()),
                        explored_state_count: certificate.explored_state_count(),
                        evaluated_transition_count: certificate.evaluated_transition_count(),
                        edges,
                        authorizes_project_mutation: false,
                    };
                    let requested = states[graph.target_state].clone();
                    let all_flat = requested.as_slice().iter().all(|entry| {
                        entry.angle_degrees().to_bits() == 180.0_f64.to_bits()
                    });
                    (
                        states[0].clone(),
                        requested,
                        all_flat,
                        Some(preview),
                        Some(certificate),
                        registry_edges,
                    )
                } else if let Some((_, requested)) = supplied_cycle_candidate.as_ref() {
                    (
                        initial.pose().hinge_angles().clone(),
                        requested.clone(),
                        requested.as_slice().iter().all(|entry| {
                            entry.angle_degrees().to_bits() == 180.0_f64.to_bits()
                        }),
                        None,
                        None,
                        Vec::new(),
                    )
                } else {
                    let linear = request
                        .linear_candidate_v1
                        .as_ref()
                        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
                    if let Some(exact_path) = linear.exact_dyadic_path_v1.as_ref() {
                        validate_exact_dyadic_candidate_path_v1(exact_path)
                            .map_err(str::to_owned)?;
                    }
                    let (initial_angles, requested_angles) =
                        validate_linear_candidate_angles_v1(linear, initial.pose().hinge_angles())
                            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
                    let all_flat = linear.entries.iter().all(|entry| {
                        entry.requested_angle_degrees.to_bits() == 180.0_f64.to_bits()
                    });
                    (
                        initial_angles,
                        requested_angles,
                        all_flat,
                        None,
                        None,
                        Vec::new(),
                    )
                };
            let generated = if let Some((generated, _)) = supplied_cycle_candidate {
                generated
            } else { generate_linear_multi_hinge_path_candidate_v1(
                initial.target().hinge_geometry(),
                initial.target().audit(),
                initial.pose().fixed_face(),
                &initial_angles,
                &requested_angles,
                MultiHingePathCandidateLimitsV1::default(),
            )
            .map_err(|error| match error {
                ori_kinematics::MultiHingePathCandidateErrorV1::ResourceLimit => {
                    CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
                }
                _ => CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned(),
            })? };
            let cycle_limits = CycleScheduleLimitsV1::default();
            let closure_schedule_limits = CycleScheduleLimitsV1 {
                max_degree: 1,
                max_work: 1_048_576,
                ..cycle_limits
            };
            let interval_closure = initial
                .target()
                .hinge_geometry()
                .prove_dyadic_schedule_closure_v1(
                    initial.target().audit(),
                    initial.pose().fixed_face(),
                    generated.schedule(),
                    ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 16,
                        max_leaves: 65_536,
                        max_work: closure_schedule_limits.max_work,
                        schedule_limits: closure_schedule_limits,
                    },
                )
                .map_err(|error| match error {
                    ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit => {
                        CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
                    }
                    ori_kinematics::DyadicIntervalClosureErrorV1::UnprovenClosure { .. } => {
                        CYCLE_NONCLOSING_MESSAGE.to_owned()
                    }
                    ori_kinematics::DyadicIntervalClosureErrorV1::InvalidInput => {
                        CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned()
                    }
                })?;
            let continuous = if paper_thickness_mm > 0.0
                && supports_scheduled_positive_thickness_path_v1(
                    initial.target().hinge_geometry(),
                    initial.target().audit(),
                    initial.pose().fixed_face(),
                    generated.schedule(),
                )
            {
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    initial.target().hinge_geometry(),
                    initial.target().audit(),
                    initial.pose().fixed_face(),
                    &generated,
                    &interval_closure,
                    paper_thickness_mm,
                    StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
                )
            } else {
                diagnose_scheduled_cycle_path_v1(
                    initial.target().hinge_geometry(),
                    initial.target().audit(),
                    initial.pose().fixed_face(),
                    &generated,
                    &interval_closure,
                    StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
                )
            };
            if continuous.continuous_certificate_model_id().is_none() {
                // The bounded CCD diagnostic intentionally does not distinguish
                // an actual collision from an enclosure that stayed unresolved
                // at its subdivision limit. Do not overstate either outcome.
                return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
            }
            let closed_endpoint = ori_core::prepare_stacked_fold_requested_scheduled_graph_pose_v1(
                initial,
                generated.schedule(),
                &interval_closure,
                requested_angles,
                candidate.requested_angle_degrees(),
            )
            .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
            let geometry_proof = closed_endpoint.initial().target().geometry().proof();
            let topology = closed_endpoint
                .initial()
                .target()
                .geometry()
                .candidate();
            let lineage = geometry_proof.lineage();
            let (layer_proof, layer_material_face_count, layer_overlap_cell_count) =
                if all_requested_flat {
                    let report = analyze_faces(FaceExtractionInput {
                        identity_namespace: binding.project_id(),
                        source_revision: lineage.target_revision(),
                        paper: &topology.paper,
                        pattern: &topology.pattern,
                    });
                    if report
                        .issues
                        .iter()
                        .any(|issue| issue.severity != TopologyIssueSeverity::Warning)
                    {
                        return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
                    }
                    let target_topology = report
                        .snapshot
                        .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                    let local =
                        analyze_local_flat_foldability(&topology.paper, &topology.pattern);
                    let global = analyze_global_flat_foldability(
                        GlobalFlatFoldabilityInput::current_with_geometry(
                            binding.project_id(),
                            &topology.paper,
                            &topology.pattern,
                            &target_topology,
                            &local,
                        ),
                        GlobalFlatFoldabilityLimits::default(),
                    )
                    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                    let GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } = global.outcome
                    else {
                        return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
                    };
                    let material_count = layer_order.material_faces.len();
                    let overlap_count = layer_order.overlap_cells.len();
                    (
                        super::stacked_fold_transaction::CurrentLayerEvidence::CertifiedFlat(
                            *layer_order,
                        ),
                        material_count,
                        overlap_count,
                    )
                } else {
                    let layer_order = prepare_stacked_fold_graph_non_flat_layer_order_v1(
                        &closed_endpoint,
                        layer_capability.snapshot(),
                        DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
                    )
                    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                    let material_count = layer_order.material_faces().len();
                    let overlap_count = layer_order.overlap_cell_count();
                    (
                        super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(
                            layer_order,
                        ),
                        material_count,
                        overlap_count,
                    )
                };
            let face_count = closed_endpoint
                .initial()
                .target()
                .hinge_geometry()
                .face_ids()
                .len();
            let expected_pair_count = face_count * face_count.saturating_sub(1) / 2;
            let adjacent_pair_count = closed_endpoint
                .initial()
                .target()
                .hinge_geometry()
                .hinges()
                .iter()
                .map(|hinge| {
                    let mut pair = [hinge.left_face(), hinge.right_face()];
                    pair.sort_unstable_by_key(FaceId::canonical_bytes);
                    pair
                })
                .collect::<std::collections::HashSet<_>>()
                .len();
            let endpoint_collision = StackedFoldEndpointCollisionDto {
                expected_pair_count,
                separated_pair_count: expected_pair_count.saturating_sub(adjacent_pair_count),
                touching_pair_count: 0,
                allowed_pair_count: adjacent_pair_count,
                penetrating_pair_count: 0,
                indeterminate_pair_count: 0,
                has_blocking_hold: false,
            };
            let topology_proof = StackedFoldTopologyProofDto {
                target_fingerprint_sha256: lineage.target_fingerprint().to_hex(),
                target_vertex_count: topology.pattern.vertices.len(),
                target_edge_count: topology.pattern.edges.len(),
                target_boundary_vertex_count: topology.paper.boundary_vertices.len(),
                lineage_record_count: lineage.records().len(),
                source_edge_subdivision_count: geometry_proof.source_edges().len(),
                expected_crease_subdivision_count: geometry_proof.expected_creases().len(),
                target_material_face_count: face_count,
                target_hinge_count: closed_endpoint
                    .initial()
                    .target()
                    .hinge_geometry()
                    .hinges()
                    .len(),
            };
            let added_vertex_count = topology
                .pattern
                .vertices
                .len()
                .checked_sub(pattern.vertices.len())
                .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let added_edge_count = topology
                .pattern
                .edges
                .len()
                .checked_sub(pattern.edges.len())
                .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let mountain_crease_count = expected_creases
                .iter()
                .filter(|crease| crease.kind == ori_domain::EdgeKind::Mountain)
                .count();
            let valley_crease_count = expected_creases.len() - mountain_crease_count;
            let transaction_proposal = StackedFoldTransactionProposalDto {
                transaction_token: None,
                source_project_id: binding.project_id(),
                source_revision: binding.source_revision(),
                target_revision: lineage.target_revision(),
                source_fingerprint_sha256: lineage.source_fingerprint().to_hex(),
                target_fingerprint_sha256: lineage.target_fingerprint().to_hex(),
                added_vertex_count,
                added_edge_count,
                mountain_crease_count,
                valley_crease_count,
                timeline_step_count: 1,
                timeline_complete_hinge_angle_count: closed_endpoint
                    .pose()
                    .hinge_angles()
                    .as_slice()
                    .len(),
                requested_angle_degrees: candidate.requested_angle_degrees(),
                ready_for_atomic_apply: false,
                failure_classes: Vec::new(),
                authorizes_project_mutation: false,
            };
            let live_graph_hinge_angles =
                live_hinge_registry(closed_endpoint.initial().pose().hinge_angles().as_slice());
            let transaction_source_fingerprint = lineage.source_fingerprint().0;
            let native_transaction = Some(NativeStackedFoldPremises::Graph(
                super::stacked_fold_transaction::PendingStackedFoldGraphPremises {
                    expected_instance_id: binding.project_instance_id(),
                    expected_project_id: binding.project_id(),
                    expected_revision: binding.source_revision(),
                    expected_source_fingerprint: transaction_source_fingerprint,
                    expected_pose_generation: binding.pose_generation(),
                    expected_layer_generation: binding.layer_order_generation(),
                    requested: closed_endpoint,
                    continuous,
                    interval_closure,
                    layer_order: layer_proof,
                    certified_path: certified_path_certificate,
                    certified_edges: certified_path_edges,
                },
            ));
            let crossed_cells = proposal
                .crossed_cells()
                .iter()
                .map(|cell| StackedFoldReadCellDto {
                    cell_key_sha256: lowercase_hex(cell.cell_key().canonical_bytes()),
                    bottom_to_top_faces: cell.bottom_to_top_faces().to_vec(),
                    boundary_world: cell.boundary_world().to_vec(),
                })
                .collect();
            let work = proposal.work();
            let support = proposal.support();
            let target_faces = proposal.target_faces().to_vec();
            let material_segments = material_map
                .segments()
                .iter()
                .map(|segment| StackedFoldMaterialSegmentDto {
                    face_id: segment.face(),
                    start: [segment.start().x, segment.start().y],
                    end: [segment.end().x, segment.end().y],
                    fixed_side: match segment.fixed_side() {
                        StackedFoldFixedSideV1::Left => "left",
                        StackedFoldFixedSideV1::Right => "right",
                    },
                    assignment: match segment.assignment() {
                        ori_domain::EdgeKind::Mountain => "mountain",
                        ori_domain::EdgeKind::Valley => "valley",
                        _ => unreachable!("material map emits only mountain or valley"),
                    },
                })
                .collect();
            drop(material_map);
            drop(proposal);
            drop(guard);
            return Ok::<_, String>((
                worker_permit,
                pose_capability,
                layer_capability,
                support,
                crossed_cells,
                target_faces,
                material_segments,
                topology_proof,
                live_graph_hinge_angles,
                work,
                endpoint_collision,
                StackedFoldPathAnalysis::Graph {
                    diagnostic: continuous,
                    requested_angle_degrees: candidate.requested_angle_degrees(),
                },
                certified_path_graph,
                StackedFoldFlatEndpointLayerOrderDto {
                    applicable: true,
                    certified: true,
                    material_face_count: layer_material_face_count,
                    overlap_cell_count: layer_overlap_cell_count,
                },
                transaction_proposal,
                native_transaction,
            ));
        }
        let prepared_target = prepare_stacked_fold_target_model_v1(
            audited_target.into_geometry(),
            TreeKinematicsLimits::default(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let prepared_initial_pose = prepare_stacked_fold_initial_pose_v1(
            prepared_target,
            pose_capability.model(),
            pose_capability.pose(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let moving_hinges = prepared_initial_pose
            .target()
            .geometry()
            .proof()
            .expected_creases()
            .iter()
            .flat_map(|subdivision| subdivision.target_edges().iter().copied())
            .collect::<Vec<_>>();
        let continuous_path = diagnose_collective_hinge_path_v1(
            prepared_initial_pose.target().model(),
            prepared_initial_pose.pose(),
            &moving_hinges,
            candidate.requested_angle_degrees(),
            paper.thickness_mm,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let prepared_requested_pose = prepare_stacked_fold_requested_pose_v1(
            prepared_initial_pose,
            candidate.requested_angle_degrees(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let topology = prepared_requested_pose
            .initial()
            .target()
            .geometry()
            .candidate();
        let geometry_proof = prepared_requested_pose
            .initial()
            .target()
            .geometry()
            .proof();
        let positive_thickness_certificate = matches!(
            continuous_path.continuous_certificate_model_id(),
            Some(
                ori_collision::STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
                    | ori_collision::STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            )
        );
        let endpoint_collision = if positive_thickness_certificate {
            let face_count = prepared_requested_pose
                .initial()
                .target()
                .model()
                .face_ids()
                .len();
            let expected_pair_count = face_count * face_count.saturating_sub(1) / 2;
            StackedFoldEndpointCollisionDto {
                expected_pair_count,
                separated_pair_count: 0,
                touching_pair_count: 0,
                allowed_pair_count: expected_pair_count,
                penetrating_pair_count: 0,
                indeterminate_pair_count: 0,
                has_blocking_hold: false,
            }
        } else {
            let endpoint = diagnose_static_collision_geometry(
                prepared_requested_pose.initial().target().model(),
                prepared_requested_pose.pose(),
                paper.thickness_mm,
                StaticCollisionLimits::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            if endpoint.has_prominent_blocking_hold() {
                return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
            }
            StackedFoldEndpointCollisionDto {
                expected_pair_count: endpoint.expected_unordered_face_pairs(),
                separated_pair_count: endpoint.separated_pairs(),
                touching_pair_count: endpoint.touching_pairs(),
                allowed_pair_count: endpoint.allowed_pairs(),
                penetrating_pair_count: endpoint.penetrating_pairs(),
                indeterminate_pair_count: endpoint.indeterminate_pairs(),
                has_blocking_hold: false,
            }
        };
        let (flat_endpoint_layer_order, transaction_layer_order) =
            if candidate.requested_angle_degrees().to_bits() == 180.0_f64.to_bits() {
                let target_revision = geometry_proof.lineage().target_revision();
                let topology_report = analyze_faces(FaceExtractionInput {
                    identity_namespace: binding.project_id(),
                    source_revision: target_revision,
                    paper: &topology.paper,
                    pattern: &topology.pattern,
                });
                if topology_report
                    .issues
                    .iter()
                    .any(|issue| issue.severity != TopologyIssueSeverity::Warning)
                {
                    return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
                }
                let target_topology = topology_report
                    .snapshot
                    .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                let local = analyze_local_flat_foldability(&topology.paper, &topology.pattern);
                let report = analyze_global_flat_foldability(
                    GlobalFlatFoldabilityInput::current_with_geometry(
                        binding.project_id(),
                        &topology.paper,
                        &topology.pattern,
                        &target_topology,
                        &local,
                    ),
                    GlobalFlatFoldabilityLimits::default(),
                )
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                match report.outcome {
                    GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => (
                        StackedFoldFlatEndpointLayerOrderDto {
                            applicable: true,
                            certified: true,
                            material_face_count: layer_order.material_faces.len(),
                            overlap_cell_count: layer_order.overlap_cells.len(),
                        },
                        None,
                    ),
                    GlobalFlatFoldabilityOutcome::Impossible { .. }
                    | GlobalFlatFoldabilityOutcome::Unknown { .. } => (
                        StackedFoldFlatEndpointLayerOrderDto {
                            applicable: true,
                            certified: false,
                            material_face_count: 0,
                            overlap_cell_count: 0,
                        },
                        None,
                    ),
                }
            } else {
                let non_flat = prepare_stacked_fold_non_flat_layer_order_with_thickness_v1(
                    &prepared_requested_pose,
                    layer_capability.snapshot(),
                    paper.thickness_mm,
                    DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
                )
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                (
                    StackedFoldFlatEndpointLayerOrderDto {
                        applicable: true,
                        certified: true,
                        material_face_count: non_flat.material_faces().len(),
                        overlap_cell_count: non_flat.overlap_cell_count(),
                    },
                    Some(non_flat),
                )
            };
        let lineage = geometry_proof.lineage();
        let topology_proof = StackedFoldTopologyProofDto {
            target_fingerprint_sha256: lineage.target_fingerprint().to_hex(),
            target_vertex_count: topology.pattern.vertices.len(),
            target_edge_count: topology.pattern.edges.len(),
            target_boundary_vertex_count: topology.paper.boundary_vertices.len(),
            lineage_record_count: lineage.records().len(),
            source_edge_subdivision_count: geometry_proof.source_edges().len(),
            expected_crease_subdivision_count: geometry_proof.expected_creases().len(),
            target_material_face_count: prepared_requested_pose
                .initial()
                .target()
                .model()
                .face_ids()
                .len(),
            target_hinge_count: prepared_requested_pose
                .initial()
                .target()
                .model()
                .hinges()
                .len(),
        };
        let source_fingerprint_sha256 = geometry_proof.lineage().source_fingerprint().to_hex();
        let target_fingerprint_sha256 = geometry_proof.lineage().target_fingerprint().to_hex();
        let added_vertex_count = topology
            .pattern
            .vertices
            .len()
            .checked_sub(pattern.vertices.len())
            .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let added_edge_count = topology
            .pattern
            .edges
            .len()
            .checked_sub(pattern.edges.len())
            .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let mountain_crease_count = expected_creases
            .iter()
            .filter(|crease| crease.kind == ori_domain::EdgeKind::Mountain)
            .count();
        let valley_crease_count = expected_creases
            .iter()
            .filter(|crease| crease.kind == ori_domain::EdgeKind::Valley)
            .count();
        if mountain_crease_count + valley_crease_count != expected_creases.len() {
            return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
        }
        let transaction_failures = transaction_failure_classes(
            continuous_path.continuous_clearance_certified(),
            flat_endpoint_layer_order.certified,
        );
        let transaction_proposal = StackedFoldTransactionProposalDto {
            transaction_token: None,
            source_project_id: binding.project_id(),
            source_revision: binding.source_revision(),
            target_revision: geometry_proof.lineage().target_revision(),
            source_fingerprint_sha256,
            target_fingerprint_sha256,
            added_vertex_count,
            added_edge_count,
            mountain_crease_count,
            valley_crease_count,
            timeline_step_count: 1,
            timeline_complete_hinge_angle_count: prepared_requested_pose
                .pose()
                .hinge_angles()
                .len(),
            requested_angle_degrees: candidate.requested_angle_degrees(),
            ready_for_atomic_apply: false,
            failure_classes: transaction_failures,
            authorizes_project_mutation: false,
        };
        let source_fingerprint_bytes = geometry_proof.lineage().source_fingerprint().0;
        let live_graph_hinge_angles =
            live_hinge_registry(prepared_requested_pose.initial().pose().hinge_angles());
        let native_transaction = transaction_layer_order.map(|layer_order| {
            NativeStackedFoldPremises::Tree(super::stacked_fold_transaction::PendingStackedFoldPremises {
                expected_instance_id: binding.project_instance_id(),
                expected_project_id: binding.project_id(),
                expected_revision: binding.source_revision(),
                expected_source_fingerprint: source_fingerprint_bytes,
                expected_pose_generation: binding.pose_generation(),
                expected_layer_generation: binding.layer_order_generation(),
                requested: prepared_requested_pose,
                continuous: continuous_path,
                paper_thickness_mm,
                layer_order,
            })
        });
        let crossed_cells = proposal
            .crossed_cells()
            .iter()
            .map(|cell| StackedFoldReadCellDto {
                cell_key_sha256: lowercase_hex(cell.cell_key().canonical_bytes()),
                bottom_to_top_faces: cell.bottom_to_top_faces().to_vec(),
                boundary_world: cell.boundary_world().to_vec(),
            })
            .collect();
        let work = proposal.work();
        let support = proposal.support();
        let target_faces = proposal.target_faces().to_vec();
        let material_segments = material_map
            .segments()
            .iter()
            .map(|segment| StackedFoldMaterialSegmentDto {
                face_id: segment.face(),
                start: [segment.start().x, segment.start().y],
                end: [segment.end().x, segment.end().y],
                fixed_side: match segment.fixed_side() {
                    StackedFoldFixedSideV1::Left => "left",
                    StackedFoldFixedSideV1::Right => "right",
                },
                assignment: match segment.assignment() {
                    ori_domain::EdgeKind::Mountain => "mountain",
                    ori_domain::EdgeKind::Valley => "valley",
                    _ => unreachable!("material map emits only mountain or valley"),
                },
            })
            .collect();
        drop(material_map);
        drop(proposal);
        drop(guard);
        Ok::<_, String>((
            worker_permit,
            pose_capability,
            layer_capability,
            support,
            crossed_cells,
            target_faces,
            material_segments,
            topology_proof,
            live_graph_hinge_angles,
            work,
            endpoint_collision,
            StackedFoldPathAnalysis::Tree(continuous_path),
            None,
            flat_endpoint_layer_order,
            transaction_proposal,
            native_transaction,
        ))
    })
    .await
    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())??;
    let (
        worker_permit,
        pose_capability,
        layer_capability,
        support,
        crossed_cells,
        target_faces,
        material_segments,
        topology_proof,
        live_graph_hinge_angles,
        work,
        endpoint_collision,
        continuous_path,
        certified_path_graph,
        flat_endpoint_layer_order,
        mut transaction_proposal,
        native_transaction,
    ) = analysis;

    {
        let project = lock_project(&app_state).map_err(|_| STALE_MESSAGE.to_owned())?;
        let pose_is_current = project
            .applied_pose_authority
            .revalidate_capability(&project, &pose_capability)
            .map_err(|_| STALE_MESSAGE.to_owned())?
            .is_some();
        let layer_is_current = revalidate_current_layer_order_capability(
            &foldability_state,
            &project,
            &layer_capability,
        )
        .map_err(|_| STALE_MESSAGE.to_owned())?
        .is_some();
        if !pose_is_current || !layer_is_current {
            return Err(STALE_MESSAGE.to_owned());
        }
    }
    if let Some(native_transaction) = native_transaction {
        let token = match native_transaction {
            NativeStackedFoldPremises::Tree(premises) => {
                super::stacked_fold_transaction::install_pending_stacked_fold(
                    &transaction_state,
                    premises,
                    pose_capability,
                    layer_capability,
                )?
            }
            NativeStackedFoldPremises::Graph(premises) => {
                super::stacked_fold_transaction::install_pending_stacked_fold_graph(
                    &transaction_state,
                    premises,
                    pose_capability,
                    layer_capability,
                )?
            }
        };
        transaction_proposal.transaction_token = Some(token);
        transaction_proposal.ready_for_atomic_apply = true;
        transaction_proposal.authorizes_project_mutation = true;
    }
    drop(worker_permit);

    Ok(StackedFoldReadResponse {
        guard_model_id: ori_collision::STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
        proposal_model_id: ori_collision::STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
        material_map_model_id: ori_collision::STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1,
        binding: StackedFoldReadBindingDto {
            project_instance_id: binding.project_instance_id(),
            project_id: binding.project_id(),
            source_revision: binding.source_revision(),
            pose_generation: binding.pose_generation(),
            layer_order_generation: binding.layer_order_generation(),
        },
        support: support.into(),
        crossed_cells,
        target_faces,
        material_segments,
        topology_proof,
        live_graph_hinge_angles,
        endpoint_collision,
        continuous_path: match continuous_path {
            StackedFoldPathAnalysis::Tree(value) => StackedFoldContinuousPathDto {
                model_id: value.model_id(),
                continuous_certificate_model_id: value.continuous_certificate_model_id(),
                sampled_pose_count: value.sampled_pose_count(),
                sampled_nonblocking_pose_count: value.sampled_nonblocking_pose_count(),
                interval_leaf_count: value.interval_leaf_count(),
                interval_pair_work: value.interval_pair_work(),
                interval_candidate_limit: value.interval_candidate_limit(),
                positive_endpoint_candidate_count: value.positive_endpoint_memo_pair_entries(),
                positive_endpoint_exact_pair_calls: value.positive_endpoint_exact_pair_calls(),
                positive_endpoint_candidate_limit: value.positive_endpoint_candidate_limit(),
                closure_required: false,
                closure_leaf_count: 0,
                closure_pair_work: 0,
                first_closure_failure_angle_degrees: None,
                first_sampled_blocking_angle_degrees: value.first_sampled_blocking_angle_degrees(),
                requested_angle_degrees: value.requested_angle_degrees(),
                continuous_clearance_certified: value.continuous_clearance_certified(),
                safe_stop_angle_degrees: value.safe_stop_angle_degrees(),
                authorizes_project_mutation: value.authorizes_project_mutation(),
                paper_thickness_mm,
            },
            StackedFoldPathAnalysis::Graph {
                diagnostic,
                requested_angle_degrees,
            } => StackedFoldContinuousPathDto {
                model_id: ori_collision::STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1,
                continuous_certificate_model_id: diagnostic.continuous_certificate_model_id(),
                sampled_pose_count: diagnostic.leaf_count().saturating_add(1),
                sampled_nonblocking_pose_count: if diagnostic
                    .continuous_certificate_model_id()
                    .is_some()
                {
                    diagnostic.leaf_count().saturating_add(1)
                } else {
                    0
                },
                interval_leaf_count: 0,
                interval_pair_work: 0,
                interval_candidate_limit: 0,
                positive_endpoint_candidate_count: 0,
                positive_endpoint_exact_pair_calls: 0,
                positive_endpoint_candidate_limit: 0,
                closure_required: true,
                closure_leaf_count: diagnostic.leaf_count(),
                closure_pair_work: diagnostic.pair_work(),
                first_closure_failure_angle_degrees: diagnostic
                    .first_closure_failure_angle_degrees(),
                first_sampled_blocking_angle_degrees: None,
                requested_angle_degrees,
                continuous_clearance_certified: diagnostic
                    .continuous_certificate_model_id()
                    .is_some(),
                safe_stop_angle_degrees: if diagnostic.continuous_certificate_model_id().is_some() {
                    requested_angle_degrees
                } else {
                    0.0
                },
                authorizes_project_mutation: false,
                paper_thickness_mm,
            },
        },
        certified_path_graph,
        flat_endpoint_layer_order,
        transaction_proposal,
        work: StackedFoldReadWorkDto {
            scanned_cells: work.scanned_cells,
            total_boundary_vertices: work.total_boundary_vertices,
            total_layer_records: work.total_layer_records,
            orientation_tests: work.orientation_tests,
            exact_arithmetic_operations: work.exact_arithmetic_operations,
            maximum_exact_integer_bits: work.maximum_exact_integer_bits,
            total_exact_integer_bits: work.total_exact_integer_bits,
            retained_cells: work.retained_cells,
            retained_target_faces: work.retained_target_faces,
        },
        authorizes_project_mutation: false,
        authorizes_apply_stacked_fold: false,
    })
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

fn pose_state_fingerprint_v1(angles: &ori_kinematics::CanonicalHingeAngles) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"stacked_fold_certified_path_graph_state_v1");
    hash.update((angles.as_slice().len() as u64).to_be_bytes());
    for angle in angles.as_slice() {
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_be_bytes());
    }
    hash.finalize().into()
}

fn transaction_failure_classes(
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
#[path = "../../../../test-support/dense_grid_cycle.rs"]
mod dense_grid_cycle_test_support;
#[cfg(test)]
#[path = "../../../../test-support/four_bay_cycle.rs"]
mod four_bay_cycle_test_support;
#[cfg(test)]
#[path = "../../../../test-support/theta_cycle.rs"]
mod theta_cycle_test_support;

#[cfg(test)]
mod tests {
    use super::*;

    // The production cancellation generation is intentionally process-wide.
    // Serialize tests that advance it so parallel test scheduling cannot make
    // an unrelated preview observe a foreign cancellation.
    static STACKED_FOLD_READ_GENERATION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_stacked_fold_read_generation_test() -> std::sync::MutexGuard<'static, ()> {
        STACKED_FOLD_READ_GENERATION_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn fixed_id<T: serde::de::DeserializeOwned>(group: &str, index: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-4000-{group}-{index:012x}\"")).unwrap()
    }

    fn automatic_opposite_pairs(
        project: &super::super::ProjectState,
        snapshot: &ori_topology::TopologySnapshot,
    ) -> Vec<[ori_domain::EdgeId; 2]> {
        let geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
            project.editor.pattern(),
            project.editor.paper(),
            snapshot,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
            snapshot,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .unwrap();
        let count = geometry.hinges().len();
        ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
            &geometry,
            &audit,
            count * (count - 1) / 2,
        )
        .unwrap()
    }

    fn uncertified_rational_kawasaki_project(
        numerator: f64,
        denominator: f64,
        complement: f64,
    ) -> (super::super::ProjectState, Vec<ori_domain::EdgeId>) {
        use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
        let ratio = numerator / denominator;
        let sine = complement / denominator;
        let points = [
            (1.0, 0.0),
            (-ratio, sine),
            (2.0 * ratio * ratio - 1.0, -2.0 * ratio * sine),
            (ratio, -sine),
            (0.0, 0.0),
        ];
        let vertices = points
            .into_iter()
            .map(|(x, y)| Vertex {
                id: ori_domain::VertexId::new(),
                position: Point2::new(x * 100.0, y * 100.0),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let center = vertices[4].id;
        let mut edges = (0..4)
            .map(|index| Edge {
                id: ori_domain::EdgeId::new(),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = (0..4)
            .map(|_| ori_domain::EdgeId::new())
            .collect::<Vec<_>>();
        edges.extend((0..4).map(|index| Edge {
            id: hinges[index],
            start: boundary[index],
            end: center,
            kind: if index == 3 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        (
            super::super::ProjectState::new_with_paper(
                CreasePattern { vertices, edges },
                Paper {
                    boundary_vertices: boundary,
                    ..Paper::default()
                },
            ),
            hinges,
        )
    }

    fn two_hinge_tree_project() -> super::super::ProjectState {
        use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
        let points = [
            (0.0, 0.0),
            (33.0, 0.0),
            (66.0, 0.0),
            (100.0, 0.0),
            (100.0, 100.0),
            (66.0, 100.0),
            (33.0, 100.0),
            (0.0, 100.0),
        ];
        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: fixed_id("7100", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("7200", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([
            Edge {
                id: fixed_id("7200", 20),
                start: boundary[1],
                end: boundary[6],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: fixed_id("7200", 21),
                start: boundary[2],
                end: boundary[5],
                kind: EdgeKind::Valley,
            },
        ]);
        let mut project = super::super::ProjectState::new_with_paper(
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        );
        project.instance_id = fixed_id("7300", 1);
        project.project_id = fixed_id("7300", 2);
        project
    }

    fn four_hinge_tree_project() -> super::super::ProjectState {
        use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (520.0, 90.0),
            (680.0, 280.0),
            (650.0, 500.0),
            (450.0, 680.0),
            (180.0, 700.0),
            (0.0, 340.0),
        ];
        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: fixed_id("7400", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("7500", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (index, end) in [2, 3, 4, 5, 6].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("7500", index as u64 + 20),
                start: boundary[0],
                end: boundary[end],
                kind: if index % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        super::super::ProjectState::new_with_paper(
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        )
    }

    fn five_hinge_tree_project() -> super::super::ProjectState {
        positive_tree_project(5)
    }

    fn six_hinge_tree_project() -> super::super::ProjectState {
        positive_tree_project(6)
    }

    fn seven_hinge_tree_project() -> super::super::ProjectState {
        positive_tree_project(7)
    }

    fn positive_tree_project(hinge_count: usize) -> super::super::ProjectState {
        use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
        let points: &[(f64, f64)] = match hinge_count {
            5 => &[
                (0.0, 0.0),
                (300.0, 0.0),
                (520.0, 90.0),
                (680.0, 280.0),
                (650.0, 500.0),
                (450.0, 680.0),
                (180.0, 700.0),
                (0.0, 340.0),
            ],
            6 => &[
                (0.0, 0.0),
                (300.0, 0.0),
                (530.0, 70.0),
                (700.0, 220.0),
                (760.0, 430.0),
                (620.0, 640.0),
                (380.0, 760.0),
                (140.0, 720.0),
                (0.0, 360.0),
            ],
            7 => &[
                (0.0, 0.0),
                (300.0, 0.0),
                (540.0, 60.0),
                (730.0, 190.0),
                (840.0, 380.0),
                (810.0, 580.0),
                (650.0, 760.0),
                (410.0, 850.0),
                (150.0, 780.0),
                (0.0, 390.0),
            ],
            _ => unreachable!("positive Tree fixture only covers 5..=7 hinges"),
        };
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("7600", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("7700", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (index, end) in (2..=hinge_count + 1).enumerate() {
            edges.push(Edge {
                id: fixed_id("7700", index as u64 + 20),
                start: boundary[0],
                end: boundary[end],
                kind: if index % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        super::super::ProjectState::new_with_paper(
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        )
    }

    fn eight_hinge_tree_project() -> super::super::ProjectState {
        use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
        let bottom = (0..=9).map(|index| (index as f64 * 20.0, 0.0));
        let top = (0..=9).rev().map(|index| (index as f64 * 20.0, 100.0));
        let vertices = bottom
            .chain(top)
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: fixed_id("7c00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("7d00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for index in 1..=8 {
            edges.push(Edge {
                id: fixed_id("7d00", index as u64 + 20),
                start: boundary[index],
                end: boundary[19 - index],
                kind: if index % 2 == 0 {
                    EdgeKind::Valley
                } else {
                    EdgeKind::Mountain
                },
            });
        }
        super::super::ProjectState::new_with_paper(
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        )
    }

    #[test]
    fn dyadic_pose_graph_read_is_strict_bounded_and_observation_only() {
        let (mut project, hinges) = super::super::applied_pose::tests::four_vertex_cycle_project();
        super::super::applied_pose::tests::install_flat_graph_pose_authority(
            &mut project,
            hinges.clone(),
        );
        let live_edges = project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .unwrap()
            .graph()
            .unwrap()
            .2
            .hinge_angles()
            .as_slice()
            .iter()
            .map(|angle| angle.edge())
            .collect::<Vec<_>>();
        let request = |max_states| DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            target_angles: live_edges
                .iter()
                .copied()
                .enumerate()
                .map(|(index, edge)| DyadicPoseGraphAngleDtoV1 {
                    edge,
                    angle_degrees: if index < 2 { 30.0 } else { 0.0 },
                })
                .collect(),
            max_states,
            max_transitions: 64,
            level_count: 3,
            cycle_schedule_v1: None,
        };
        let limited_request = request(8);
        let live_request = request(32);
        let state = AppState::new(project);
        let limited =
            read_bounded_dyadic_pose_graph_inner_v1(&state, None, limited_request, None).unwrap();
        assert_eq!(limited.status, "resource_limit");
        assert!(!limited.authorizes_project_mutation);
        let observed =
            read_bounded_dyadic_pose_graph_inner_v1(&state, None, live_request, None).unwrap();
        assert_eq!(observed.state_count, 9);
        assert_eq!(observed.transition_count, 24);
        assert_eq!(observed.status, "no_path");
        assert_eq!(observed.reason, "no_certified_path");
        assert_eq!(observed.certified_transition_count, 0);
        assert!(observed.certificate_binding_sha256.is_none());
        assert_eq!(observed.positive_thickness_transition_count, 0);
        assert!(!observed.positive_thickness_certified);
        assert!(observed.positive_thickness_binding_sha256.is_none());
        assert_eq!(observed.layer_transport_transition_count, 0);
        assert!(!observed.layer_transport_certified);
        assert!(observed.layer_transport_binding_sha256.is_none());
        assert!(!observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let preview_state = DyadicPathPreviewState::default();
        let rejected = mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &preview_state,
            DyadicPathPreviewRequestV1 {
                expected_project_instance_id: observed.project_instance_id,
                expected_project_id: observed.project_id,
                expected_revision: observed.revision,
                target_angles: live_edges
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, edge)| DyadicPoseGraphAngleDtoV1 {
                        edge,
                        angle_degrees: if index < 2 { 30.0 } else { 0.0 },
                    })
                    .collect(),
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
                expected_path_binding_sha256: "00".repeat(32),
                expected_positive_thickness_binding_sha256: "11".repeat(32),
                expected_layer_transport_binding_sha256: "22".repeat(32),
            },
        );
        assert_eq!(rejected.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
        assert!(preview_state.0.lock().unwrap().is_none());
        let token = ProjectId::new();
        let target_binding = [0x33; 32];
        *preview_state.0.lock().unwrap() = Some(DyadicPathPreviewRecordV1 {
            token,
            project_instance_id: observed.project_instance_id,
            project_id: observed.project_id,
            revision: observed.revision,
            target_binding,
            path_binding: "44".repeat(32),
            positive_binding: "55".repeat(32),
            layer_binding: "66".repeat(32),
            authority: None,
        });
        let apply_request = |path: String| ApplyDyadicPathPreviewRequestV1 {
            preview_token: token,
            expected_project_instance_id: observed.project_instance_id,
            expected_project_id: observed.project_id,
            expected_revision: observed.revision,
            expected_target_binding_sha256: "33".repeat(32),
            expected_path_binding_sha256: path,
            expected_positive_thickness_binding_sha256: "55".repeat(32),
            expected_layer_transport_binding_sha256: "66".repeat(32),
        };
        let apply_layer_state = GlobalFlatFoldabilityState::default();
        assert_eq!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &apply_layer_state,
                &preview_state,
                apply_request("77".repeat(32)),
            )
            .unwrap_err(),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
        );
        assert!(preview_state.0.lock().unwrap().is_some());
        assert_eq!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &apply_layer_state,
                &preview_state,
                apply_request("44".repeat(32)),
            )
            .unwrap_err(),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
        );
        assert!(preview_state.0.lock().unwrap().is_some());
        cancel_dyadic_pose_path_preview_inner_v1(&preview_state, token).unwrap();
        assert!(preview_state.0.lock().unwrap().is_none());
        assert_eq!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &apply_layer_state,
                &preview_state,
                apply_request("44".repeat(32)),
            )
            .unwrap_err(),
            CYCLE_PATH_UNCERTIFIED_MESSAGE,
        );
        assert!(
            super::super::lock_project(&state)
                .unwrap()
                .editor
                .instruction_timeline()
                .steps
                .is_empty()
        );
    }

    #[test]
    fn four_hinge_tree_level_three_read_and_preview_are_bounded_read_only() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut project = four_hinge_tree_project();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        assert_eq!(hinges.len(), 4);
        super::super::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
            &mut project,
            hinges.clone(),
            snapshot.faces[0].id,
            1.0,
        );
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let target_angles = hinges
            .iter()
            .copied()
            .map(|edge| DyadicPoseGraphAngleDtoV1 {
                edge,
                angle_degrees: 1.0,
            })
            .collect::<Vec<_>>();
        let tree_capability = project
            .applied_pose_authority
            .capture_capability(&project)
            .unwrap()
            .unwrap();
        let mut tree_target_entries = target_angles
            .iter()
            .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        tree_target_entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
        let tree_target = ori_kinematics::CanonicalHingeAngles::new(tree_target_entries).unwrap();
        let tree_diagnostic = ori_collision::diagnose_collective_hinge_path_from_pose_v1(
            tree_capability.model(),
            tree_capability.pose(),
            tree_capability.pose().hinge_angles(),
            tree_target.as_slice(),
            project.editor.paper().thickness_mm,
            ori_collision::StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(
            tree_diagnostic.continuous_clearance_certified(),
            "four-hinge native Tree endpoint must issue positive evidence: {tree_diagnostic:?}"
        );
        let state = AppState::new(project);
        let request = |level_count, max_states, max_transitions| DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target_angles.clone(),
            max_states,
            max_transitions,
            level_count,
            cycle_schedule_v1: None,
        };
        for (levels, states, transitions) in [(5, 125, 600), (9, 128, 512)] {
            let limited = read_bounded_dyadic_pose_graph_inner_v1(
                &state,
                Some(&layer_state),
                request(levels, states, transitions),
                None,
            )
            .unwrap();
            assert_eq!(limited.status, "resource_limit");
            assert!(!limited.mutation_candidate_ready);
        }
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(3, 81, 432),
            None,
        )
        .unwrap();
        assert_eq!((observed.state_count, observed.transition_count), (81, 432));
        assert_eq!(
            observed.status,
            "certified",
            "explored={} evaluated={} certified={} positive={}",
            observed.explored_state_count,
            observed.evaluated_transition_count,
            observed.certified_transition_count,
            observed.positive_thickness_transition_count,
        );
        assert!(observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let preview_state = DyadicPathPreviewState::default();
        let preview = mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            DyadicPathPreviewRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles,
                max_states: 81,
                max_transitions: 432,
                level_count: 3,
                cycle_schedule_v1: None,
                expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
                expected_positive_thickness_binding_sha256: observed
                    .positive_thickness_binding_sha256
                    .unwrap(),
                expected_layer_transport_binding_sha256: observed
                    .layer_transport_binding_sha256
                    .unwrap(),
            },
        )
        .expect("four-hinge certified graph mints a read-only token");
        assert!(!preview.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        drop(project);
        let apply_request = |path_binding: String| ApplyDyadicPathPreviewRequestV1 {
            preview_token: preview.preview_token,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            expected_target_binding_sha256: preview.target_binding_sha256.clone(),
            expected_path_binding_sha256: path_binding,
            expected_positive_thickness_binding_sha256: preview
                .positive_thickness_binding_sha256
                .clone(),
            expected_layer_transport_binding_sha256: preview.layer_transport_binding_sha256.clone(),
        };
        assert!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request("00".repeat(32)),
            )
            .is_err()
        );
        assert_eq!(
            super::super::lock_project(&state)
                .unwrap()
                .editor
                .revision(),
            revision,
            "tampered Tree proof is an atomic no-op"
        );
        let applied = apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .expect("issuer-bound four-hinge Tree proof applies atomically");
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(applied, revision + 1);
        assert!(!project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn five_hinge_tree_level_three_mints_only_a_certified_read_only_preview() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut project = five_hinge_tree_project();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 6);
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        assert_eq!(hinges.len(), 5);
        super::super::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
            &mut project,
            hinges.clone(),
            snapshot.faces[0].id,
            1.0,
        );
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let target_angles = hinges
            .iter()
            .copied()
            .map(|edge| DyadicPoseGraphAngleDtoV1 {
                edge,
                angle_degrees: 2.0,
            })
            .collect::<Vec<_>>();
        let state = AppState::new(project);
        let request = |level_count, max_states, max_transitions, angles: Vec<_>| {
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: angles,
                max_states,
                max_transitions,
                level_count,
                cycle_schedule_v1: None,
            }
        };
        for (levels, states, transitions) in [(5, 125, 600), (9, 128, 512)] {
            let limited = read_bounded_dyadic_pose_graph_inner_v1(
                &state,
                Some(&layer_state),
                request(levels, states, transitions, target_angles.clone()),
                None,
            )
            .unwrap();
            assert_eq!(limited.status, "resource_limit");
            assert!(!limited.mutation_candidate_ready);
        }
        let mut mismatched = target_angles.clone();
        mismatched[0].edge = ori_domain::EdgeId::new();
        assert_eq!(
            read_bounded_dyadic_pose_graph_inner_v1(
                &state,
                Some(&layer_state),
                request(3, 243, 1_620, mismatched),
                None,
            )
            .unwrap_err(),
            CYCLE_PATH_UNSUPPORTED_MESSAGE
        );
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(3, 243, 1_620, target_angles.clone()),
            None,
        )
        .unwrap();
        assert_eq!(
            (observed.state_count, observed.transition_count),
            (243, 1_620)
        );
        assert_eq!(observed.status, "certified");
        assert!(observed.mutation_candidate_ready);
        let preview_state = DyadicPathPreviewState::default();
        let preview_request = |expected_revision| DyadicPathPreviewRequestV1 {
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision,
            target_angles: target_angles.clone(),
            max_states: 243,
            max_transitions: 1_620,
            level_count: 3,
            cycle_schedule_v1: None,
            expected_path_binding_sha256: observed.certificate_binding_sha256.clone().unwrap(),
            expected_positive_thickness_binding_sha256: observed
                .positive_thickness_binding_sha256
                .clone()
                .unwrap(),
            expected_layer_transport_binding_sha256: observed
                .layer_transport_binding_sha256
                .clone()
                .unwrap(),
        };
        assert_eq!(
            mint_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                preview_request(revision + 1),
            )
            .unwrap_err(),
            STALE_MESSAGE
        );
        let preview = mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            preview_request(revision),
        )
        .expect("five-hinge certified graph mints a read-only token");
        assert!(!preview.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn six_hinge_tree_level_three_is_bounded_and_mints_read_only_preview() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut project = six_hinge_tree_project();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 7);
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        assert_eq!(hinges.len(), 6);
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            snapshot.faces[0].id,
        );
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let target_angles = hinges
            .iter()
            .copied()
            .map(|edge| DyadicPoseGraphAngleDtoV1 {
                edge,
                angle_degrees: 2.0,
            })
            .collect::<Vec<_>>();
        let state = AppState::new(project);
        let request = |level_count, max_states, max_transitions| DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target_angles.clone(),
            max_states,
            max_transitions,
            level_count,
            cycle_schedule_v1: None,
        };
        for (levels, states, transitions) in [(5, 125, 600), (9, 128, 512)] {
            let limited = read_bounded_dyadic_pose_graph_inner_v1(
                &state,
                Some(&layer_state),
                request(levels, states, transitions),
                None,
            )
            .unwrap();
            assert_eq!(limited.status, "resource_limit");
            assert_eq!((limited.state_count, limited.transition_count), (0, 0));
        }
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(3, 729, 5_832),
            None,
        )
        .unwrap();
        assert_eq!(
            (observed.state_count, observed.transition_count),
            (729, 5_832)
        );
        assert_eq!(observed.status, "certified");
        assert!(observed.mutation_candidate_ready);
        let preview_state = DyadicPathPreviewState::default();
        let preview = mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            DyadicPathPreviewRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles,
                max_states: 729,
                max_transitions: 5_832,
                level_count: 3,
                cycle_schedule_v1: None,
                expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
                expected_positive_thickness_binding_sha256: observed
                    .positive_thickness_binding_sha256
                    .unwrap(),
                expected_layer_transport_binding_sha256: observed
                    .layer_transport_binding_sha256
                    .unwrap(),
            },
        )
        .expect("six-hinge bounded proof mints a read-only token");
        assert!(!preview.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn seven_hinge_generic_grid_is_bounded_and_mints_read_only_preview() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut project = seven_hinge_tree_project();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 8);
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        assert_eq!(hinges.len(), 7);
        super::super::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
            &mut project,
            hinges.clone(),
            snapshot.faces[0].id,
            1.0,
        );
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let target_angles = hinges
            .iter()
            .copied()
            .map(|edge| DyadicPoseGraphAngleDtoV1 {
                edge,
                angle_degrees: 2.0,
            })
            .collect::<Vec<_>>();
        let state = AppState::new(project);
        let request = |schedule| DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target_angles.clone(),
            max_states: 2_187,
            max_transitions: 20_412,
            level_count: 3,
            cycle_schedule_v1: schedule,
        };
        let generic = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(None),
            None,
        )
        .unwrap();
        assert_eq!(generic.status, "certified");
        assert_eq!(
            (generic.state_count, generic.transition_count),
            (2_187, 20_412)
        );
        assert!(generic.mutation_candidate_ready);
        let preview_state = DyadicPathPreviewState::default();
        let preview = mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            DyadicPathPreviewRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles,
                max_states: 2_187,
                max_transitions: 20_412,
                level_count: 3,
                cycle_schedule_v1: None,
                expected_path_binding_sha256: generic.certificate_binding_sha256.unwrap(),
                expected_positive_thickness_binding_sha256: generic
                    .positive_thickness_binding_sha256
                    .unwrap(),
                expected_layer_transport_binding_sha256: generic
                    .layer_transport_binding_sha256
                    .unwrap(),
            },
        )
        .expect("seven-hinge generic proof mints a bounded read-only token");
        assert!(!preview.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn eight_hinge_generic_grid_fails_before_allocation_and_collective_route_mints() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut project = eight_hinge_tree_project();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 9);
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        assert_eq!(hinges.len(), 8);
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            snapshot.faces[0].id,
        );
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let angle = 2.0 * 1.0_f64.atan2(4.0).to_degrees();
        let target_angles = hinges
            .iter()
            .copied()
            .map(|edge| DyadicPoseGraphAngleDtoV1 {
                edge,
                angle_degrees: angle,
            })
            .collect::<Vec<_>>();
        let state = AppState::new(project);
        let request = |schedule| DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target_angles.clone(),
            max_states: 2_187,
            max_transitions: 20_412,
            level_count: 3,
            cycle_schedule_v1: schedule,
        };
        let generic = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(None),
            None,
        )
        .unwrap();
        assert_eq!(generic.status, "resource_limit");
        assert_eq!((generic.state_count, generic.transition_count), (0, 0));
        assert!(!generic.mutation_candidate_ready);

        let schedule = dense_grid_schedule(&hinges, &hinges, 4);
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            Some(&layer_state),
            request(Some(schedule.clone())),
            None,
        )
        .unwrap();
        assert_eq!((observed.state_count, observed.transition_count), (3, 4));
        assert_eq!(observed.status, "certified");
        assert!(observed.mutation_candidate_ready);
        let preview_state = DyadicPathPreviewState::default();
        let preview = mint_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            DyadicPathPreviewRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles,
                max_states: 2_187,
                max_transitions: 20_412,
                level_count: 3,
                cycle_schedule_v1: Some(schedule),
                expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
                expected_positive_thickness_binding_sha256: observed
                    .positive_thickness_binding_sha256
                    .unwrap(),
                expected_layer_transport_binding_sha256: observed
                    .layer_transport_binding_sha256
                    .unwrap(),
            },
        )
        .expect("eight-hinge collective proof mints a bounded read-only token");
        assert!(!preview.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn two_hinge_e2e_fixture_issues_pose_and_layer_authorities() {
        let mut project = two_hinge_tree_project();
        super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_some()
        );
        assert!(
            capture_current_layer_order_capability(&layer_state, &project)
                .unwrap()
                .is_some()
        );
    }

    fn assert_non_graph_capability_returns_unsupported_dto(
        project: super::super::ProjectState,
        target_edge: ori_domain::EdgeId,
        authority_expected: bool,
    ) {
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            None,
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                    edge: target_edge,
                    angle_degrees: 1.0,
                }],
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        )
        .expect("non-graph capability returns a read-only DTO");
        assert_eq!(observed.status, "unsupported");
        assert_eq!(observed.reason, "unsupported_geometry");
        assert_eq!(observed.state_count, 0);
        assert_eq!(observed.transition_count, 0);
        assert_eq!(observed.explored_state_count, 0);
        assert_eq!(observed.evaluated_transition_count, 0);
        assert_eq!(observed.certified_transition_count, 0);
        assert!(!observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert_eq!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_some(),
            authority_expected
        );
    }

    #[test]
    fn missing_pose_capability_strict_dyadic_read_returns_unsupported_dto() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let project = two_hinge_tree_project();
        let target_edge = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze()
            .simulation_snapshot()
            .unwrap()
            .hinge_adjacency[0]
            .edge;
        assert_non_graph_capability_returns_unsupported_dto(project, target_edge, false);
    }

    #[test]
    fn tree_pose_capability_rejects_incomplete_target_without_mutation() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut project = two_hinge_tree_project();
        let target_edge = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze()
            .simulation_snapshot()
            .unwrap()
            .hinge_adjacency[0]
            .edge;
        super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let result = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            None,
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                    edge: target_edge,
                    angle_degrees: 1.0,
                }],
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        );
        assert_eq!(result.unwrap_err(), CYCLE_PATH_UNSUPPORTED_MESSAGE);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    fn assert_two_hinge_projective_schedule_round_trip(
        first: [f64; 3],
        second: [f64; 3],
        certified_path_steps: usize,
        cancel_after_transition: Option<usize>,
    ) -> Vec<(String, String, String)> {
        let _generation_guard = STACKED_FOLD_READ_GENERATION_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut project = two_hinge_tree_project();
        super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let layer_state = GlobalFlatFoldabilityState::default();
        {
            let project = super::super::lock_project(&app_state).unwrap();
            super::super::global_flat_foldability::tests::install_possible_layer_order(
                &layer_state,
                &project,
            );
        }
        let certified_path = certified_path_steps > 0;
        let angle = if certified_path {
            certified_path_steps as f64
        } else {
            2.0 * 1.0_f64.atan2(5.0).to_degrees()
        };
        let registry = tauri::async_runtime::block_on(read_live_hinge_registry_inner(
            &app_state,
            &layer_state,
            LiveHingeRegistryRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                first,
                second,
                fixed_side: FixedSideRequest::Left,
                rotation_direction: RotationDirectionRequest::Positive,
                requested_angle_degrees: angle,
            },
        ))
        .expect("live target hinge registry");
        assert!(registry.entries.len() >= 2);
        let cycle_schedule_v1 = CycleScheduleRequestV1 {
            version: 1,
            endpoint_denominator: None,
            entries: registry
                .entries
                .iter()
                .map(|entry| {
                    let is_source_hinge =
                        entry.initial_angle_degrees.to_bits() == 180.0_f64.to_bits();
                    CycleScheduleEntryRequestV1 {
                        edge: entry.edge,
                        u_domain: [
                            RationalCoefficientRequestV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientRequestV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ],
                        numerator_power_coefficients: if is_source_hinge {
                            vec![RationalCoefficientRequestV1 {
                                numerator: 1,
                                denominator: 1,
                            }]
                        } else {
                            vec![
                                RationalCoefficientRequestV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientRequestV1 {
                                    numerator: 1,
                                    denominator: 1,
                                },
                            ]
                        },
                        denominator_power_coefficients: if is_source_hinge {
                            vec![RationalCoefficientRequestV1 {
                                numerator: 0,
                                denominator: 1,
                            }]
                        } else {
                            vec![RationalCoefficientRequestV1 {
                                numerator: 5,
                                denominator: 1,
                            }]
                        },
                        requested_angle_degrees: if is_source_hinge { 180.0 } else { angle },
                    }
                })
                .collect(),
        };
        let certified_path_graph_v1 = certified_path.then(|| CertifiedPathGraphRequestV1 {
            version: 1,
            states: (0..=certified_path_steps)
                .map(|step| step as f64 / certified_path_steps as f64)
                .map(|progress| CertifiedPathGraphStateRequestV1 {
                    entries: registry
                        .entries
                        .iter()
                        .map(|entry| CertifiedPathGraphAngleRequestV1 {
                            edge: entry.edge,
                            angle_degrees: if entry.initial_angle_degrees.to_bits()
                                == 180.0_f64.to_bits()
                            {
                                180.0
                            } else {
                                angle * progress
                            },
                        })
                        .collect(),
                })
                .collect(),
            transitions: (0..certified_path_steps)
                .map(|step| CertifiedPathGraphTransitionRequestV1 {
                    source_state: step,
                    target_state: step + 1,
                })
                .collect(),
            source_state: 0,
            target_state: certified_path_steps,
        });
        let transaction_state =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let response = tauri::async_runtime::block_on(propose_current_stacked_fold_read_inner(
            None,
            &app_state,
            &layer_state,
            &transaction_state,
            StackedFoldReadRequest {
                progress_request_id: cancel_after_transition
                    .map(|step| format!("test-cancel-after-{step}")),
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                first,
                second,
                fixed_side: FixedSideRequest::Left,
                rotation_direction: RotationDirectionRequest::Positive,
                requested_angle_degrees: angle,
                cycle_schedule_v1: (!certified_path).then_some(cycle_schedule_v1),
                linear_candidate_v1: None,
                certified_path_graph_v1,
            },
        ));
        if cancel_after_transition.is_some() {
            assert_eq!(response.unwrap_err(), CANCELLED_MESSAGE);
            let project = super::super::lock_project(&app_state).unwrap();
            assert_eq!(project.editor.revision(), revision);
            assert!(project.editor.instruction_timeline().steps.is_empty());
            return Vec::new();
        }
        let response = response.expect("genuine ready preview");
        let certificate_hashes = if certified_path {
            let graph = response
                .certified_path_graph
                .as_ref()
                .expect("certified path graph preview");
            assert_eq!(graph.explored_state_count, certified_path_steps);
            assert_eq!(graph.evaluated_transition_count, certified_path_steps);
            assert_eq!(graph.edges.len(), certified_path_steps);
            assert!(graph.edges.iter().all(|edge| {
                edge.schedule_certificate_sha256.len() == 64
                    && edge.collision_certificate_sha256.len() == 64
                    && edge.closure_certificate_sha256.len() == 64
            }));
            assert!(!graph.authorizes_project_mutation);
            graph
                .edges
                .iter()
                .map(|edge| {
                    (
                        edge.schedule_certificate_sha256.clone(),
                        edge.collision_certificate_sha256.clone(),
                        edge.closure_certificate_sha256.clone(),
                    )
                })
                .collect()
        } else {
            assert!(response.certified_path_graph.is_none());
            Vec::new()
        };
        assert!(response.transaction_proposal.ready_for_atomic_apply);
        let token = response
            .transaction_proposal
            .transaction_token
            .expect("ready token");
        let before = {
            let project = super::super::lock_project(&app_state).unwrap();
            project.editor.clone()
        };
        let applied_revision =
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &layer_state,
                &transaction_state,
                token,
            )
            .expect("atomic apply");
        let mut project = super::super::lock_project(&app_state).unwrap();
        assert_eq!(project.editor.revision(), applied_revision);
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        assert_eq!(response.transaction_proposal.timeline_step_count, 1);
        let after = project.editor.clone();
        let source_vertices = before
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<std::collections::HashSet<_>>();
        let inserted = after
            .pattern()
            .vertices
            .iter()
            .filter(|vertex| !source_vertices.contains(&vertex.id))
            .collect::<Vec<_>>();
        assert!(
            !inserted.is_empty(),
            "the new straight line must atomically materialize its source-hinge intersections"
        );
        let line_start = ori_domain::Point2::new(first[0], -first[2]);
        let line_end = ori_domain::Point2::new(second[0], -second[2]);
        let line_length =
            ((line_end.x - line_start.x).powi(2) + (line_end.y - line_start.y).powi(2)).sqrt();
        let position_tolerance = 1.0e-9_f64;
        let inserted_on_requested_line = inserted
            .iter()
            .filter(|vertex| {
                let cross = (line_end.x - line_start.x) * (vertex.position.y - line_start.y)
                    - (line_end.y - line_start.y) * (vertex.position.x - line_start.x);
                cross.abs() <= position_tolerance * line_length.max(1.0)
                    && vertex.position.x + position_tolerance >= line_start.x.min(line_end.x)
                    && vertex.position.x - position_tolerance <= line_start.x.max(line_end.x)
                    && vertex.position.y + position_tolerance >= line_start.y.min(line_end.y)
                    && vertex.position.y - position_tolerance <= line_start.y.max(line_end.y)
            })
            .count();
        assert!(
            inserted_on_requested_line >= 2,
            "both source hinges must gain a materialized intersection on the requested line"
        );
        assert!(after.pattern().edges.len() > before.pattern().edges.len());
        project.editor.undo(applied_revision).unwrap();
        assert_eq!(project.editor.pattern(), before.pattern());
        assert_eq!(
            project.editor.current_applied_pose(),
            before.current_applied_pose()
        );
        let undo_revision = project.editor.revision();
        project.editor.redo(undo_revision).unwrap();
        assert_eq!(project.editor.pattern(), after.pattern());
        assert_eq!(
            project.editor.current_applied_pose(),
            after.current_applied_pose()
        );
        let archive = project
            .project_archive()
            .expect("serialize split-hinge cycle operation");
        let mut reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("split-hinge-cycle.ori2"),
        )
        .expect("reopen split-hinge cycle operation");
        assert_eq!(reopened.editor.pattern(), after.pattern());
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
        let reopened_revision = reopened.editor.revision();
        reopened.editor.undo(reopened_revision).unwrap();
        assert_eq!(reopened.editor.pattern(), before.pattern());
        let reopened_redo_revision = reopened.editor.revision();
        reopened.editor.redo(reopened_redo_revision).unwrap();
        assert_eq!(reopened.editor.pattern(), after.pattern());
        assert_eq!(
            reopened.editor.current_applied_pose(),
            after.current_applied_pose()
        );
        certificate_hashes
    }

    #[test]
    fn genuine_two_hinge_projective_schedule_previews_applies_and_round_trips_history() {
        let _ = assert_two_hinge_projective_schedule_round_trip(
            [50.0, 0.0, 0.0],
            [50.0, 0.0, -100.0],
            0,
            None,
        );
    }

    #[test]
    fn genuine_common_axis_cycle_previews_applies_and_round_trips_history() {
        let _ = assert_two_hinge_projective_schedule_round_trip(
            [0.0, 0.0, -50.0],
            [100.0, 0.0, -50.0],
            0,
            None,
        );
    }

    #[test]
    fn genuine_common_axis_cycle_certified_path_applies_and_round_trips_history() {
        let _ = assert_two_hinge_projective_schedule_round_trip(
            [0.0, 0.0, -50.0],
            [100.0, 0.0, -50.0],
            2,
            None,
        );
    }

    #[test]
    fn genuine_common_axis_cycle_four_edge_certified_path_applies_and_round_trips_history() {
        let _ = assert_two_hinge_projective_schedule_round_trip(
            [0.0, 0.0, -50.0],
            [100.0, 0.0, -50.0],
            4,
            None,
        );
    }

    #[test]
    fn genuine_common_axis_cycle_sixteen_edge_certified_path_applies_and_round_trips_history() {
        let _ = assert_two_hinge_projective_schedule_round_trip(
            [0.0, 0.0, -50.0],
            [100.0, 0.0, -50.0],
            16,
            None,
        );
    }

    #[test]
    fn genuine_common_axis_cycle_maximum_atomic_path_cancels_cleanly_and_retries() {
        let first = [0.0, 0.0, -50.0];
        let second = [100.0, 0.0, -50.0];
        assert!(
            assert_two_hinge_projective_schedule_round_trip(first, second, 31, Some(8)).is_empty()
        );
        let first_retry = assert_two_hinge_projective_schedule_round_trip(first, second, 31, None);
        let second_retry = assert_two_hinge_projective_schedule_round_trip(first, second, 31, None);
        assert_eq!(first_retry, second_retry);
    }

    #[test]
    fn request_schema_is_closed_and_rejects_non_finite_points() {
        let project_instance_id = ProjectId::new();
        let project_id = ProjectId::new();
        let json = serde_json::json!({
            "expectedProjectInstanceId": project_instance_id,
            "expectedProjectId": project_id,
            "expectedRevision": 7,
            "first": [10.0, 0.0, 0.0],
            "second": [10.0, 0.0, -20.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 90.0
        });
        let request: StackedFoldReadRequest =
            serde_json::from_value(json.clone()).expect("valid request");
        assert_eq!(request.expected_revision, 7);
        assert!(
            StackedFoldLinearCandidateV1::new(
                Point3::new(request.first[0], request.first[1], request.first[2]).unwrap(),
                Point3::new(request.second[0], request.second[1], request.second[2]).unwrap(),
                request.fixed_side.into(),
                request.rotation_direction.into(),
                request.requested_angle_degrees,
            )
            .is_ok()
        );

        let mut unknown = json.clone();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("future".to_owned(), serde_json::Value::Bool(true));
        assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());

        let mut non_finite = json;
        non_finite["first"][0] = serde_json::json!(f64::INFINITY);
        assert!(
            serde_json::from_value::<StackedFoldReadRequest>(non_finite)
                .ok()
                .and_then(|request| {
                    Point3::new(request.first[0], request.first[1], request.first[2]).ok()
                })
                .is_none()
        );
    }

    #[test]
    fn cell_keys_use_fixed_lowercase_sha256_hex() {
        let mut bytes = [0_u8; 32];
        bytes[0] = 0xab;
        bytes[31] = 0xef;
        let encoded = lowercase_hex(bytes);
        assert_eq!(encoded.len(), 64);
        assert!(encoded.starts_with("ab00"));
        assert!(encoded.ends_with("00ef"));
        assert!(encoded.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(!encoded.bytes().any(|byte| byte.is_ascii_uppercase()));
    }

    #[test]
    fn request_schema_rejects_missing_malformed_and_open_enum_values() {
        let valid = serde_json::json!({
            "expectedProjectInstanceId": ProjectId::new(),
            "expectedProjectId": ProjectId::new(),
            "expectedRevision": 7,
            "first": [10.0, 0.0, 0.0],
            "second": [10.0, 0.0, -20.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 90.0
        });

        for field in [
            "expectedProjectInstanceId",
            "expectedProjectId",
            "expectedRevision",
            "first",
            "second",
            "fixedSide",
            "rotationDirection",
            "requestedAngleDegrees",
        ] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<StackedFoldReadRequest>(missing).is_err(),
                "missing field {field} must be rejected"
            );
        }

        for malformed in [
            ("first", serde_json::json!([10.0, 0.0])),
            ("second", serde_json::json!([10.0, 0.0, -20.0, 1.0])),
            ("fixedSide", serde_json::json!("center")),
            ("fixedSide", serde_json::json!("Left")),
            ("rotationDirection", serde_json::json!("clockwise")),
            ("rotationDirection", serde_json::json!("Positive")),
        ] {
            let mut request = valid.clone();
            request[malformed.0] = malformed.1;
            assert!(
                serde_json::from_value::<StackedFoldReadRequest>(request).is_err(),
                "malformed field {} must be rejected",
                malformed.0
            );
        }
    }

    #[test]
    fn candidate_validation_rejects_degenerate_line_and_invalid_angles() {
        let point = Point3::new(1.0, 2.0, 3.0).unwrap();
        assert!(
            StackedFoldLinearCandidateV1::new(
                point,
                point,
                StackedFoldFixedSideV1::Left,
                StackedFoldRotationDirectionV1::Positive,
                90.0,
            )
            .is_err()
        );

        let other = Point3::new(2.0, 2.0, 3.0).unwrap();
        for angle in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -90.0,
            180.1,
        ] {
            assert!(
                StackedFoldLinearCandidateV1::new(
                    point,
                    other,
                    StackedFoldFixedSideV1::Right,
                    StackedFoldRotationDirectionV1::Negative,
                    angle,
                )
                .is_err(),
                "invalid angle {angle:?} must be rejected"
            );
        }
    }

    #[test]
    fn transaction_proposal_failure_classes_are_explicit_and_fail_closed() {
        let missing_all = serde_json::to_value(transaction_failure_classes(false, false)).unwrap();
        assert_eq!(
            missing_all,
            serde_json::json!([
                "continuous_path_uncertified",
                "target_layer_order_unavailable"
            ])
        );
        let ready = serde_json::to_value(transaction_failure_classes(true, true)).unwrap();
        assert_eq!(ready, serde_json::json!([]));
    }

    #[test]
    fn cycle_schedule_wire_rejects_unknown_fields_and_numeric_overflow() {
        let request = || {
            serde_json::json!({
                "expectedProjectInstanceId": "018f47a2-4b7a-7cc1-8abc-112233445566",
                "expectedProjectId": "018f47a2-4b7a-7cc1-8abc-665544332211",
                "expectedRevision": 3,
                "first": [0.0, 0.0, 0.0],
                "second": [1.0, 0.0, 0.0],
                "fixedSide": "left",
                "rotationDirection": "positive",
                "requestedAngleDegrees": 90.0,
                "cycleScheduleV1": {
                    "version": 1,
                    "entries": [{
                        "edge": "018f47a2-4b7a-7cc1-8abc-778899aabbcc",
                        "uDomain": [
                            {"numerator": 0, "denominator": 1},
                            {"numerator": 1, "denominator": 1}
                        ],
                        "numeratorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                        "denominatorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                        "requestedAngleDegrees": 90.0
                    }]
                }
            })
        };
        let admitted = serde_json::from_value::<StackedFoldReadRequest>(request()).unwrap();
        assert_eq!(validate_request_resource_shape_v1(&admitted), Ok(()));
        let mut unknown = request();
        unknown["cycleScheduleV1"]["entries"][0]["authority"] = serde_json::json!(true);
        assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());
        let mut overflow = request();
        overflow["cycleScheduleV1"]["entries"][0]["uDomain"][0]["denominator"] =
            serde_json::json!(-1);
        assert!(serde_json::from_value::<StackedFoldReadRequest>(overflow).is_err());

        let mut coefficient_exhaustion = request();
        coefficient_exhaustion["cycleScheduleV1"]["entries"][0]["numeratorPowerCoefficients"] = serde_json::json!(
            (0..=MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1)
                .map(|_| serde_json::json!({"numerator": 1, "denominator": 1}))
                .collect::<Vec<_>>()
        );
        let coefficient_exhaustion =
            serde_json::from_value::<StackedFoldReadRequest>(coefficient_exhaustion).unwrap();
        assert_eq!(
            validate_request_resource_shape_v1(&coefficient_exhaustion),
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        );
    }

    #[test]
    fn rank64_cycle_request_rejects_resource_before_work_and_keeps_progress_cancel_dtos_bounded() {
        let entry = || {
            serde_json::json!({
                "edge": ori_domain::EdgeId::new(),
                "uDomain": [{"numerator": 0, "denominator": 1}, {"numerator": 1, "denominator": 1}],
                "numeratorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                "denominatorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                "requestedAngleDegrees": 90.0
            })
        };
        let request = serde_json::from_value::<StackedFoldReadRequest>(serde_json::json!({
            "progressRequestId": "rank64:resource",
            "expectedProjectInstanceId": ori_domain::ProjectId::new(),
            "expectedProjectId": ori_domain::ProjectId::new(),
            "expectedRevision": 0,
            "first": [0.0, 0.0, 0.0],
            "second": [1.0, 0.0, 0.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 90.0,
            "cycleScheduleV1": {"version": 1, "entries": (0..256).map(|_| entry()).collect::<Vec<_>>()}
        })).unwrap();
        assert_eq!(
            validate_progress_request_id_v1(request.progress_request_id.as_deref()),
            Ok(Some("rank64:resource"))
        );
        assert_eq!(
            validate_request_resource_shape_v1(&request),
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        );

        let _generation_guard = lock_stacked_fold_read_generation_test();
        let before = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
        cancel_current_stacked_fold_read_v1().expect("rank64 cancel dto remains available");
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            before + 1
        );
    }

    #[test]
    fn linear_candidate_requires_bit_exact_live_initial_angles() {
        let edge = serde_json::from_value::<ori_domain::EdgeId>(serde_json::json!(
            "018f47a2-4b7a-7cc1-8abc-778899aabbcc"
        ))
        .unwrap();
        let live = ori_kinematics::CanonicalHingeAngles::new(vec![
            ori_kinematics::HingeAngle::new(edge, 20.0).unwrap(),
        ])
        .unwrap();
        let request = LinearCandidateRequestV1 {
            version: 1,
            exact_dyadic_path_v1: None,
            entries: vec![LinearCandidateEntryRequestV1 {
                edge,
                initial_angle_degrees: 20.0,
                requested_angle_degrees: 40.0,
            }],
        };
        let (initial, requested) = validate_linear_candidate_angles_v1(&request, &live).unwrap();
        assert_eq!(initial, live);
        assert_ne!(requested, live);

        let mismatch = LinearCandidateRequestV1 {
            version: 1,
            exact_dyadic_path_v1: None,
            entries: vec![LinearCandidateEntryRequestV1 {
                edge,
                initial_angle_degrees: f64::from_bits(20.0f64.to_bits() + 1),
                requested_angle_degrees: 40.0,
            }],
        };
        assert!(validate_linear_candidate_angles_v1(&mismatch, &live).is_err());
        let wrong_version = LinearCandidateRequestV1 {
            version: 2,
            exact_dyadic_path_v1: None,
            entries: request.entries,
        };
        assert!(validate_linear_candidate_angles_v1(&wrong_version, &live).is_err());
    }

    #[test]
    fn exact_dyadic_candidate_preflight_rejects_crossing_and_allows_endpoint_touch() {
        let point = |x, y, power| ExactDyadicPointRequestV1 {
            x_numerator: x,
            y_numerator: y,
            denominator_power: power,
        };
        let path = |second| ExactDyadicPathRequestV1 {
            version: 1,
            segments: vec![
                ExactDyadicSegmentRequestV1 {
                    start: point(0, 0, 0),
                    end: point(2, 0, 0),
                },
                second,
            ],
            max_pair_tests: 1,
            max_denominator_power: 80,
            max_integer_bits: 256,
        };
        assert_eq!(
            validate_exact_dyadic_candidate_path_v1(&path(ExactDyadicSegmentRequestV1 {
                start: point(1, -1, 80),
                end: point(1, 1, 80),
            })),
            Err(CYCLE_PATH_UNCERTIFIED_MESSAGE)
        );
        assert_eq!(
            validate_exact_dyadic_candidate_path_v1(&path(ExactDyadicSegmentRequestV1 {
                start: point(2, 0, 0),
                end: point(3, 1, 0),
            })),
            Ok(())
        );
        let mut bounded = path(ExactDyadicSegmentRequestV1 {
            start: point(1, 1, 80),
            end: point(1, 2, 80),
        });
        bounded.max_pair_tests = 0;
        assert_eq!(
            validate_exact_dyadic_candidate_path_v1(&bounded),
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        );
    }

    #[test]
    fn certified_path_graph_admission_is_live_bound_canonical_and_bounded() {
        let edge = ori_domain::EdgeId::new();
        let live = ori_kinematics::CanonicalHingeAngles::new(vec![
            ori_kinematics::HingeAngle::new(edge, 0.0).unwrap(),
        ])
        .unwrap();
        let state = |angle_degrees| CertifiedPathGraphStateRequestV1 {
            entries: vec![CertifiedPathGraphAngleRequestV1 {
                edge,
                angle_degrees,
            }],
        };
        let valid = CertifiedPathGraphRequestV1 {
            version: 1,
            states: vec![state(0.0), state(45.0), state(90.0)],
            transitions: vec![
                CertifiedPathGraphTransitionRequestV1 {
                    source_state: 0,
                    target_state: 1,
                },
                CertifiedPathGraphTransitionRequestV1 {
                    source_state: 1,
                    target_state: 2,
                },
            ],
            source_state: 0,
            target_state: 2,
        };
        assert_eq!(
            validate_certified_path_graph_v1(&valid, &live)
                .unwrap()
                .len(),
            3
        );

        let stale = CertifiedPathGraphRequestV1 {
            states: vec![state(1.0), state(45.0)],
            target_state: 1,
            transitions: vec![CertifiedPathGraphTransitionRequestV1 {
                source_state: 0,
                target_state: 1,
            }],
            ..valid
        };
        assert_eq!(
            validate_certified_path_graph_v1(&stale, &live),
            Err(CYCLE_PATH_UNSUPPORTED_MESSAGE)
        );
        let over_limit = CertifiedPathGraphRequestV1 {
            version: 1,
            states: (0..=ori_collision::MAX_CERTIFIED_PATH_GRAPH_STATES_V1)
                .map(|index| state(index as f64))
                .collect(),
            target_state: 1,
            transitions: Vec::new(),
            source_state: 0,
        };
        assert_eq!(
            validate_certified_path_graph_v1(&over_limit, &live),
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        );
        let transition_over_limit = CertifiedPathGraphRequestV1 {
            version: 1,
            states: vec![state(0.0), state(90.0)],
            transitions: (0..=MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1)
                .map(|_| CertifiedPathGraphTransitionRequestV1 {
                    source_state: 0,
                    target_state: 1,
                })
                .collect(),
            source_state: 0,
            target_state: 1,
        };
        assert_eq!(
            validate_certified_path_graph_v1(&transition_over_limit, &live),
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        );
        let oversized_state = CertifiedPathGraphRequestV1 {
            version: 1,
            states: vec![
                CertifiedPathGraphStateRequestV1 {
                    entries: (0..=MAX_STACKED_FOLD_REQUEST_HINGES_V1)
                        .map(|_| CertifiedPathGraphAngleRequestV1 {
                            edge: ori_domain::EdgeId::new(),
                            angle_degrees: 0.0,
                        })
                        .collect(),
                },
                state(90.0),
            ],
            transitions: vec![CertifiedPathGraphTransitionRequestV1 {
                source_state: 0,
                target_state: 1,
            }],
            source_state: 0,
            target_state: 1,
        };
        assert_eq!(
            validate_certified_path_graph_v1(&oversized_state, &live),
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        );
    }

    #[test]
    fn stacked_fold_read_cancel_advances_the_process_wide_generation() {
        let _generation_guard = STACKED_FOLD_READ_GENERATION_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
        cancel_current_stacked_fold_read_v1().expect("generation has capacity");
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            before + 1
        );
    }

    #[test]
    fn explicit_half_angle_schedule_uses_graph_proof_boundary_for_tree_topology() {
        assert!(requires_graph_schedule_boundary_v1(false, true));
        assert!(requires_graph_schedule_boundary_v1(true, false));
        assert!(!requires_graph_schedule_boundary_v1(false, false));
    }

    #[test]
    fn live_registry_round_trips_into_the_same_bit_exact_linear_request() {
        let first = serde_json::from_value::<ori_domain::EdgeId>(serde_json::json!(
            "018f47a2-4b7a-7cc1-8abc-665544332211"
        ))
        .unwrap();
        let second = serde_json::from_value::<ori_domain::EdgeId>(serde_json::json!(
            "018f47a2-4b7a-7cc1-8abc-778899aabbcc"
        ))
        .unwrap();
        let live = ori_kinematics::CanonicalHingeAngles::new(vec![
            ori_kinematics::HingeAngle::new(first, 10.0).unwrap(),
            ori_kinematics::HingeAngle::new(second, 20.0).unwrap(),
        ])
        .unwrap();
        let registry = live_hinge_registry(live.as_slice());
        assert_eq!(
            registry.iter().map(|entry| entry.edge).collect::<Vec<_>>(),
            vec![first, second]
        );
        let request = LinearCandidateRequestV1 {
            version: 1,
            exact_dyadic_path_v1: None,
            entries: registry
                .iter()
                .map(|entry| LinearCandidateEntryRequestV1 {
                    edge: entry.edge,
                    initial_angle_degrees: entry.initial_angle_degrees,
                    requested_angle_degrees: entry.initial_angle_degrees + 5.0,
                })
                .collect(),
        };
        let (round_tripped, requested) =
            validate_linear_candidate_angles_v1(&request, &live).unwrap();
        assert_eq!(round_tripped, live);
        assert!(
            requested
                .as_slice()
                .iter()
                .zip(live.as_slice())
                .all(|(next, initial)| {
                    next.edge() == initial.edge()
                        && next.angle_degrees() == initial.angle_degrees() + 5.0
                })
        );
    }

    fn physical_four_vertex_cycle_schedule(
        _hinges: &[ori_domain::EdgeId],
    ) -> CycleScheduleRequestV1 {
        CycleScheduleRequestV1 {
            version: 2,
            entries: Vec::new(),
            endpoint_denominator: Some(1),
        }
    }

    fn dense_grid_schedule(
        hinges: &[ori_domain::EdgeId],
        moving: &[ori_domain::EdgeId],
        denominator: i64,
    ) -> CycleScheduleRequestV1 {
        dense_grid_schedule_ratio(hinges, moving, 1, denominator)
    }

    fn dense_grid_schedule_ratio(
        hinges: &[ori_domain::EdgeId],
        moving: &[ori_domain::EdgeId],
        numerator: i64,
        denominator: i64,
    ) -> CycleScheduleRequestV1 {
        let moving = moving
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut entries = hinges
            .iter()
            .copied()
            .map(|edge| {
                let active = moving.contains(&edge);
                CycleScheduleEntryRequestV1 {
                    edge,
                    u_domain: [
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: if active {
                        vec![
                            RationalCoefficientRequestV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientRequestV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ]
                    } else {
                        vec![RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                    denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                        numerator: if active { denominator } else { 1 },
                        denominator: 1,
                    }],
                    requested_angle_degrees: if active {
                        2.0 * (numerator as f64).atan2(denominator as f64).to_degrees()
                    } else {
                        0.0
                    },
                }
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        CycleScheduleRequestV1 {
            version: 1,
            entries,
            endpoint_denominator: None,
        }
    }

    fn advance_collective_schedule(
        hinges: &[ori_domain::EdgeId],
        moving: &[ori_domain::EdgeId],
        denominator: i64,
    ) -> CycleScheduleRequestV1 {
        let moving = moving
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut entries = hinges
            .iter()
            .copied()
            .map(|edge| {
                let active = moving.contains(&edge);
                CycleScheduleEntryRequestV1 {
                    edge,
                    u_domain: [
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: if active {
                        vec![
                            RationalCoefficientRequestV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                            RationalCoefficientRequestV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ]
                    } else {
                        vec![RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                    denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                        numerator: if active { denominator } else { 1 },
                        denominator: 1,
                    }],
                    requested_angle_degrees: if active {
                        2.0 * 2.0_f64.atan2(denominator as f64).to_degrees()
                    } else {
                        0.0
                    },
                }
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        CycleScheduleRequestV1 {
            version: 1,
            entries,
            endpoint_denominator: None,
        }
    }

    #[test]
    fn dense_rank_four_grid_previews_applies_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
            let (pattern, mut paper, moving) =
                super::dense_grid_cycle_test_support::three_by_three_dense_cycle_pattern();
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            assert_eq!(
                (snapshot.faces.len(), snapshot.hinge_adjacency.len()),
                (9, 12)
            );
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect::<Vec<_>>();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let app_state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            let request = |expected_instance_id| CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: expected_instance_id,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &moving, 4),
            };
            assert_eq!(
                propose_current_cycle_pose_inner(
                    None,
                    &app_state,
                    &transactions,
                    request(ProjectId::new())
                )
                .unwrap_err(),
                STALE_MESSAGE
            );
            let preview = propose_current_cycle_pose_inner(
                None,
                &app_state,
                &transactions,
                request(instance),
            );
            if thickness_mm == 10_000.0 {
                assert_eq!(preview.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
                assert!(
                    super::super::lock_project(&app_state)
                        .unwrap()
                        .editor
                        .instruction_timeline()
                        .steps
                        .is_empty()
                );
                continue;
            }
            let preview = preview.expect("dense rank-four preview");
            assert_eq!(
                (
                    preview.closure_leaf_count,
                    preview.checked_hinge_count,
                    preview.total_hinge_count
                ),
                (1, 12, 12)
            );
            if thickness_mm == 1.0 {
                super::super::lock_project(&app_state).unwrap().instance_id = ProjectId::new();
                assert!(
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &app_state,
                        &GlobalFlatFoldabilityState::default(),
                        &transactions,
                        preview.transaction_token,
                    )
                    .is_err()
                );
                assert!(
                    super::super::lock_project(&app_state)
                        .unwrap()
                        .editor
                        .instruction_timeline()
                        .steps
                        .is_empty()
                );
                continue;
            }
            super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
                &transactions,
                preview.transaction_token,
            )
            .unwrap();
            assert!(
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    preview.transaction_token,
                )
                .is_err()
            );
            let preview = propose_current_cycle_pose_inner(
                None,
                &app_state,
                &transactions,
                request(instance),
            )
            .expect("dense retry");
            let applied =
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    preview.transaction_token,
                )
                .expect("dense atomic apply");
            let mut project = super::super::lock_project(&app_state).unwrap();
            project.editor.undo(applied).unwrap();
            let undone = project.editor.revision();
            project.editor.redo(undone).unwrap();
            assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        }
    }

    #[test]
    fn dense_square_and_rectangular_grids_preview_and_apply_atomically() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for (columns, rows) in [
            (4usize, 4usize),
            (5, 5),
            (6, 6),
            (7, 7),
            (3, 7),
            (5, 7),
            (6, 7),
        ] {
            for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
                let (pattern, mut paper, moving) =
                    super::dense_grid_cycle_test_support::rectangular_dense_cycle_pattern(
                        columns, rows,
                    );
                paper.thickness_mm = thickness_mm;
                let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
                let topology = project
                    .editor
                    .topology_analysis_input(project.project_id)
                    .analyze();
                let snapshot = topology.simulation_snapshot().unwrap();
                let expected_hinges = 2 * columns * rows - columns - rows;
                assert_eq!(
                    (snapshot.faces.len(), snapshot.hinge_adjacency.len()),
                    (columns * rows, expected_hinges)
                );
                let hinges = snapshot
                    .hinge_adjacency
                    .iter()
                    .map(|hinge| hinge.edge)
                    .collect::<Vec<_>>();
                let fixed = snapshot.faces[0].id;
                super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                    &mut project,
                    hinges.clone(),
                    fixed,
                );
                let request = CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: project.instance_id,
                    expected_project_id: project.project_id,
                    expected_revision: project.editor.revision(),
                    cycle_schedule_v1: dense_grid_schedule(
                        &hinges,
                        &moving,
                        if columns == 4 && rows == 4 { 4 } else { 100 },
                    ),
                };
                let state = AppState::new(project);
                let transactions =
                    super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
                let preview =
                    propose_current_cycle_pose_inner(None, &state, &transactions, request);
                if thickness_mm == 10_000.0 {
                    assert_eq!(preview.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
                    assert!(
                        super::super::lock_project(&state)
                            .unwrap()
                            .editor
                            .instruction_timeline()
                            .steps
                            .is_empty()
                    );
                    continue;
                }
                let preview = preview.unwrap_or_else(|error| {
                    panic!("{columns}x{rows} dense preview at {thickness_mm}mm: {error}")
                });
                assert_eq!(
                    (preview.closure_leaf_count, preview.checked_hinge_count),
                    (1, expected_hinges)
                );
                let applied =
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &state,
                        &GlobalFlatFoldabilityState::default(),
                        &transactions,
                        preview.transaction_token,
                    )
                    .expect("rank-nine dense apply");
                let mut project = super::super::lock_project(&state).unwrap();
                project.editor.undo(applied).unwrap();
                let undone = project.editor.revision();
                project.editor.redo(undone).unwrap();
            }
        }
    }

    #[test]
    fn orthogonal_dense_rank_four_horizontal_axis_previews_and_applies() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
            let (pattern, mut paper, horizontal, _) =
                super::dense_grid_cycle_test_support::orthogonal_dense_cycle_pattern(3, 3);
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect::<Vec<_>>();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let request = |expected_project_instance_id| CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &horizontal, 4),
            };
            let state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            assert_eq!(
                propose_current_cycle_pose_inner(
                    None,
                    &state,
                    &transactions,
                    request(ProjectId::new())
                )
                .unwrap_err(),
                STALE_MESSAGE
            );
            let preview =
                propose_current_cycle_pose_inner(None, &state, &transactions, request(instance));
            if thickness_mm == 10_000.0 {
                assert_eq!(preview.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
                continue;
            }
            let preview = preview.expect("orthogonal horizontal dense preview");
            let applied =
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    preview.transaction_token,
                )
                .expect("orthogonal horizontal dense apply");
            let mut project = super::super::lock_project(&state).unwrap();
            project.editor.undo(applied).unwrap();
            let undone = project.editor.revision();
            project.editor.redo(undone).unwrap();
        }
    }

    #[test]
    fn oblique_dense_rank_four_collision_fails_closed_before_preview() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for thickness_mm in [0.1, 1.0, 3.0, 10_000.0] {
            let (pattern, mut paper, horizontal, _) =
                super::dense_grid_cycle_test_support::oblique_dense_cycle_pattern(3, 3);
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect::<Vec<_>>();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let request = |expected_project_instance_id| CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &horizontal, 100),
            };
            let state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            assert_eq!(
                propose_current_cycle_pose_inner(
                    None,
                    &state,
                    &transactions,
                    request(ProjectId::new())
                )
                .unwrap_err(),
                STALE_MESSAGE
            );
            let preview =
                propose_current_cycle_pose_inner(None, &state, &transactions, request(instance));
            if thickness_mm == 3.0 {
                let preview = preview.expect("3mm oblique prism separation");
                let applied =
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &state,
                        &GlobalFlatFoldabilityState::default(),
                        &transactions,
                        preview.transaction_token,
                    )
                    .expect("3mm oblique apply");
                let mut project = super::super::lock_project(&state).unwrap();
                project.editor.undo(applied).unwrap();
                let undone = project.editor.revision();
                project.editor.redo(undone).unwrap();
            } else {
                assert_eq!(preview.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
            }
            assert_eq!(
                super::super::lock_project(&state)
                    .unwrap()
                    .editor
                    .instruction_timeline()
                    .steps
                    .len(),
                usize::from(thickness_mm == 3.0)
            );
        }
    }

    #[test]
    fn parametric_oblique_dense_static_authority_is_available_on_desktop() {
        for angle_degrees in [30.0, 45.0, 120.0] {
            for thickness_mm in [0.1, 1.0, 3.0] {
                let (pattern, mut paper, _, _) =
                    super::dense_grid_cycle_test_support::angled_dense_cycle_pattern(
                        3,
                        3,
                        angle_degrees,
                    );
                paper.thickness_mm = thickness_mm;
                let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
                let topology = project
                    .editor
                    .topology_analysis_input(project.project_id)
                    .analyze();
                let snapshot = topology.simulation_snapshot().unwrap();
                let hinges = snapshot
                    .hinge_adjacency
                    .iter()
                    .map(|hinge| hinge.edge)
                    .collect();
                let fixed = snapshot.faces[0].id;
                super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                    &mut project,
                    hinges,
                    fixed,
                );
                let state = AppState::new(project);
                assert!(
                    crate::applied_pose::certify_current_static_collision(
                        &state,
                        ori_collision::StaticCollisionLimits::default(),
                    )
                    .expect("parametric oblique static diagnosis")
                    .is_some()
                );
            }
        }
    }

    fn four_bay_cycle_schedule(hinges: &[ori_domain::EdgeId]) -> CycleScheduleRequestV1 {
        let triples = [
            (3, 5),
            (5, 13),
            (8, 17),
            (7, 25),
            (3, 5),
            (5, 13),
            (8, 17),
            (7, 25),
            (3, 5),
            (5, 13),
            (8, 17),
            (7, 25),
            (3, 5),
            (5, 13),
            (8, 17),
            (7, 25),
        ];
        let mut entries = hinges
            .iter()
            .copied()
            .enumerate()
            .map(|(index, edge)| {
                let (p, q) = triples[(index / 4) % triples.len()];
                CycleScheduleEntryRequestV1 {
                    edge,
                    u_domain: [
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: vec![
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: if index % 2 == 0 { 1 } else { p },
                            denominator: 1,
                        },
                    ],
                    denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                        numerator: if index % 2 == 0 { 1 } else { q },
                        denominator: 1,
                    }],
                    requested_angle_degrees: 2.0
                        * (if index % 2 == 0 { 1.0 } else { p as f64 })
                            .atan2(if index % 2 == 0 { 1.0 } else { q as f64 })
                            .to_degrees(),
                }
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        CycleScheduleRequestV1 {
            version: 1,
            entries,
            endpoint_denominator: None,
        }
    }

    fn theta_cycle_schedule(
        hinges: &[ori_domain::EdgeId],
        moving: &[ori_domain::EdgeId],
    ) -> CycleScheduleRequestV1 {
        let moving = moving
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut entries = hinges
            .iter()
            .copied()
            .map(|edge| {
                let moves = moving.contains(&edge);
                CycleScheduleEntryRequestV1 {
                    edge,
                    u_domain: [
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: if moves {
                        vec![
                            RationalCoefficientRequestV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientRequestV1 {
                                numerator: 2,
                                denominator: 15,
                            },
                        ]
                    } else {
                        vec![RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                    denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                        numerator: 1,
                        denominator: 1,
                    }],
                    requested_angle_degrees: if moves {
                        2.0 * (2.0_f64 / 15.0).atan().to_degrees()
                    } else {
                        0.0
                    },
                }
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        CycleScheduleRequestV1 {
            version: 1,
            entries,
            endpoint_denominator: None,
        }
    }

    fn balloon_six_sector_cycle_pattern() -> (
        ori_domain::CreasePattern,
        ori_domain::Paper,
        Vec<ori_domain::EdgeId>,
    ) {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x41,
        ]);
        let center = Vertex {
            id: VertexId::derive_v5(namespace, b"balloon-center"),
            position: Point2::new(0.0, 0.0),
        };
        let boundary = [
            (100.0, 0.0),
            (50.0, 100.0),
            (-50.0, 100.0),
            (-100.0, 0.0),
            (-50.0, -100.0),
            (50.0, -100.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, format!("balloon-{index}").as_bytes()),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
        let mut edges = (0..6)
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, format!("boundary-{index}").as_bytes()),
                start: boundary[index].id,
                end: boundary[(index + 1) % 6].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = (0..6)
            .map(|index| EdgeId::derive_v5(namespace, format!("spoke-{index}").as_bytes()))
            .collect::<Vec<_>>();
        edges.extend((0..6).map(|index| Edge {
            id: hinges[index],
            start: center.id,
            end: boundary[index].id,
            kind: if matches!(index, 0 | 1 | 3 | 4) {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let mut vertices = vec![center];
        vertices.extend(boundary.iter().cloned());
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary.iter().map(|vertex| vertex.id).collect(),
                thickness_mm: 0.0,
                ..Paper::default()
            },
            vec![hinges[0], hinges[3]],
        )
    }

    fn octagonal_eight_sector_cycle_pattern() -> (
        ori_domain::CreasePattern,
        ori_domain::Paper,
        Vec<ori_domain::EdgeId>,
    ) {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x42,
        ]);
        let center = Vertex {
            id: VertexId::derive_v5(namespace, b"octagonal-center"),
            position: Point2::new(0.0, 0.0),
        };
        let boundary = [
            (100.0, 0.0),
            (70.0, 70.0),
            (0.0, 100.0),
            (-70.0, 70.0),
            (-100.0, 0.0),
            (-70.0, -70.0),
            (0.0, -100.0),
            (70.0, -70.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, format!("octagonal-{index}").as_bytes()),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
        let mut edges = (0..8)
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, format!("boundary-{index}").as_bytes()),
                start: boundary[index].id,
                end: boundary[(index + 1) % 8].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = (0..8)
            .map(|index| EdgeId::derive_v5(namespace, format!("spoke-{index}").as_bytes()))
            .collect::<Vec<_>>();
        edges.extend((0..8).map(|index| Edge {
            id: hinges[index],
            start: center.id,
            end: boundary[index].id,
            kind: if matches!(index, 0 | 1 | 2 | 4 | 6) {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let mut vertices = vec![center];
        vertices.extend(boundary.iter().cloned());
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary.iter().map(|vertex| vertex.id).collect(),
                thickness_mm: 0.0,
                ..Paper::default()
            },
            vec![hinges[0], hinges[2], hinges[4], hinges[6]],
        )
    }

    fn sixteen_sector_cycle_pattern(
        moving_second: usize,
    ) -> (
        ori_domain::CreasePattern,
        ori_domain::Paper,
        Vec<ori_domain::EdgeId>,
    ) {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x43,
        ]);
        let center = Vertex {
            id: VertexId::derive_v5(namespace, b"sixteen-center"),
            position: Point2::new(0.0, 0.0),
        };
        let half = [
            (100.0, 0.0),
            (92.0, 38.0),
            (71.0, 71.0),
            (38.0, 92.0),
            (0.0, 100.0),
            (-38.0, 92.0),
            (-71.0, 71.0),
            (-92.0, 38.0),
        ];
        let coordinates = half
            .into_iter()
            .chain(half.into_iter().map(|(x, y)| (-x, -y)))
            .collect::<Vec<_>>();
        let boundary = coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, format!("sixteen-{index}").as_bytes()),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let mut edges = (0..16)
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, format!("boundary-{index}").as_bytes()),
                start: boundary[index].id,
                end: boundary[(index + 1) % 16].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = (0..16)
            .map(|index| EdgeId::derive_v5(namespace, format!("spoke-{index}").as_bytes()))
            .collect::<Vec<_>>();
        edges.extend((0..16).map(|index| Edge {
            id: hinges[index],
            start: center.id,
            end: boundary[index].id,
            kind: if index <= 8 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let mut vertices = vec![center];
        vertices.extend(boundary.iter().cloned());
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary.iter().map(|vertex| vertex.id).collect(),
                thickness_mm: 0.0,
                ..Paper::default()
            },
            vec![hinges[0], hinges[moving_second]],
        )
    }

    #[test]
    fn balloon_six_sector_straight_line_cycle_previews_applies_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, moving) = balloon_six_sector_cycle_pattern();
        assert_eq!(
            pattern
                .edges
                .iter()
                .filter(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
                .count(),
            4
        );
        assert_eq!(
            pattern
                .edges
                .iter()
                .filter(|edge| edge.kind == ori_domain::EdgeKind::Valley)
                .count(),
            2
        );
        assert!(
            pattern
                .edges
                .iter()
                .filter(|edge| moving.contains(&edge.id))
                .all(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
        );
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 6);
        assert_eq!(snapshot.hinge_adjacency.len(), 6);
        let discovered = automatic_opposite_pairs(&project, &snapshot);
        assert!(
            discovered
                .iter()
                .any(|pair| pair.iter().all(|edge| moving.contains(edge)))
        );
        let mut reordered_pattern = project.editor.pattern().clone();
        reordered_pattern.edges.reverse();
        let reordered = super::super::ProjectState::new_with_paper(
            reordered_pattern,
            project.editor.paper().clone(),
        );
        let reordered_analysis = reordered
            .editor
            .topology_analysis_input(reordered.project_id)
            .analyze();
        let reordered_snapshot = reordered_analysis.simulation_snapshot().unwrap();
        assert_eq!(
            automatic_opposite_pairs(&reordered, &reordered_snapshot),
            discovered
        );
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        let fixed = snapshot.faces[0].id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let preview = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &moving, 100),
            },
        )
        .expect("balloon straight-line cycle must certify");
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            preview.transaction_token,
        )
        .expect("balloon straight-line cycle apply");
        let second_preview = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: applied,
                cycle_schedule_v1: advance_collective_schedule(&hinges, &moving, 100),
            },
        )
        .expect("the rebound current pose must authorize a second preview");
        let second_applied =
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &state,
                &GlobalFlatFoldabilityState::default(),
                &transactions,
                second_preview.transaction_token,
            )
            .expect("second balloon operation applies atomically");
        let mut project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_some()
        );
        project.editor.undo(second_applied).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_none()
        );
        let first_undone = project.editor.revision();
        project.editor.undo(first_undone).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        let first_redone = project.editor.revision();
        project.editor.redo(first_redone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
        let mut nonclosing_document = project.document();
        let tampered = nonclosing_document.instruction_timeline.steps[0]
            .pose
            .hinge_angles
            .iter_mut()
            .find(|hinge| hinge.edge == moving[0])
            .expect("moving balloon hinge is persisted");
        tampered.angle_degrees += 0.01;
        assert!(
            super::super::validate_document_instruction_poses(&nonclosing_document)
                .expect_err("a nonclosing cyclic persisted pose must fail closed")
                .contains("is not cycle-closing")
        );
        let archive = project
            .project_archive()
            .expect("serialize applied balloon cycle with history");
        super::super::restore_archive_editor(&archive)
            .expect("restore applied balloon editor history");
        let mut reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("balloon-cycle.ori2"),
        )
        .expect("reopen applied balloon cycle");
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
        assert!(
            reopened
                .applied_pose_authority
                .capture_capability(&reopened)
                .unwrap()
                .is_some()
        );
        let reopened_revision = reopened.editor.revision();
        reopened.editor.undo(reopened_revision).unwrap();
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
        let reopened_undone = reopened.editor.revision();
        reopened.editor.undo(reopened_undone).unwrap();
        assert!(reopened.editor.instruction_timeline().steps.is_empty());
        let reopened_first_redo = reopened.editor.revision();
        reopened.editor.redo(reopened_first_redo).unwrap();
        let reopened_second_redo = reopened.editor.revision();
        reopened.editor.redo(reopened_second_redo).unwrap();
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    }

    #[test]
    fn concave_boundary_strict_dyadic_read_fails_closed_without_mutation_authority() {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x71,
        ]);
        let coordinates = [
            (0.0, 0.0),
            (3.0, 0.0),
            (3.0, 1.0),
            (1.0, 1.0),
            (1.0, 3.0),
            (0.0, 3.0),
        ];
        let vertices = coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let hinge = EdgeId::derive_v5(namespace, b"concave-hinge");
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, &[0x20, index as u8]),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: hinge,
            start: vertices[0].id,
            end: vertices[3].id,
            kind: EdgeKind::Mountain,
        });
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.1,
            ..Paper::default()
        };
        let project =
            super::super::ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            None,
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                    edge: hinge,
                    angle_degrees: 1.0,
                }],
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        )
        .expect("concave read returns a fail-closed observation");
        assert_eq!(observed.reason, "unsupported_geometry");
        assert!(!observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn cut_boundary_strict_dyadic_read_fails_closed_without_mutation_authority() {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x72,
        ]);
        let coordinates = [
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (3.0, 0.0),
            (3.0, 2.0),
            (2.0, 2.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ];
        let vertices = coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let hinge = EdgeId::derive_v5(namespace, b"cut-fixture-hinge");
        let cut = EdgeId::derive_v5(namespace, b"cut-fixture-cut");
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, &[0x20, index as u8]),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([
            Edge {
                id: hinge,
                start: vertices[1].id,
                end: vertices[6].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: cut,
                start: vertices[2].id,
                end: vertices[5].id,
                kind: EdgeKind::Cut,
            },
        ]);
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.1,
            ..Paper::default()
        };
        let project =
            super::super::ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            None,
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                    edge: hinge,
                    angle_degrees: 1.0,
                }],
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        )
        .expect("cut read returns a fail-closed observation");
        assert_eq!(observed.reason, "unsupported_geometry");
        assert!(!observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn hole_boundary_strict_dyadic_read_fails_closed_without_mutation_authority() {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x73,
        ]);
        let coordinates = [
            (0.0, 0.0),
            (1.0, 0.0),
            (8.0, 0.0),
            (8.0, 8.0),
            (1.0, 8.0),
            (0.0, 8.0),
            (2.0, 2.0),
            (6.0, 2.0),
            (4.0, 6.0),
        ];
        let vertices = coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let hinge = EdgeId::derive_v5(namespace, b"hole-fixture-hinge");
        let mut edges = (0..6)
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, &[0x20, index as u8]),
                start: vertices[index].id,
                end: vertices[(index + 1) % 6].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: hinge,
            start: vertices[1].id,
            end: vertices[4].id,
            kind: EdgeKind::Mountain,
        });
        for (index, (start, end)) in [(6, 7), (7, 8), (8, 6)].into_iter().enumerate() {
            edges.push(Edge {
                id: EdgeId::derive_v5(namespace, &[0x30, index as u8]),
                start: vertices[start].id,
                end: vertices[end].id,
                kind: EdgeKind::Cut,
            });
        }
        let paper = Paper {
            boundary_vertices: vertices[..6].iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.1,
            cutting_allowed: true,
            ..Paper::default()
        };
        let project =
            super::super::ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            None,
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                    edge: hinge,
                    angle_degrees: 1.0,
                }],
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        )
        .expect("hole read returns a fail-closed observation");
        assert_eq!(observed.reason, "unsupported_geometry");
        assert!(!observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn open_cut_seam_strict_dyadic_preflight_is_unsupported_no_op() {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x76,
        ]);
        let positions = [
            (0.0, 0.0),
            (8.0, 0.0),
            (8.0, 8.0),
            (0.0, 8.0),
            (2.0, 4.0),
            (6.0, 4.0),
        ];
        let vertices = positions
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let mut edges = (0..4)
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, &[0x50, index as u8]),
                start: vertices[index].id,
                end: vertices[(index + 1) % 4].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let target_edge = edges[0].id;
        edges.push(Edge {
            id: EdgeId::derive_v5(namespace, b"open-cut-seam"),
            start: vertices[4].id,
            end: vertices[5].id,
            kind: EdgeKind::Cut,
        });
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: pattern.vertices[..4]
                .iter()
                .map(|vertex| vertex.id)
                .collect(),
            cutting_allowed: true,
            ..Paper::default()
        };
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    fn assert_out_of_scope_boundary_is_unsupported_no_op(
        pattern: ori_domain::CreasePattern,
        paper: ori_domain::Paper,
        target_edge: ori_domain::EdgeId,
    ) {
        let project = super::super::ProjectState::new_with_paper(pattern, paper);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let observed = read_bounded_dyadic_pose_graph_inner_v1(
            &state,
            None,
            DyadicPoseGraphReadRequestV1 {
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                target_angles: vec![DyadicPoseGraphAngleDtoV1 {
                    edge: target_edge,
                    angle_degrees: 1.0,
                }],
                max_states: 32,
                max_transitions: 64,
                level_count: 3,
                cycle_schedule_v1: None,
            },
            None,
        )
        .expect("out-of-scope boundary returns a fail-closed observation");
        assert_eq!(observed.status, "unsupported");
        assert_eq!(observed.reason, "unsupported_geometry");
        assert_eq!(observed.state_count, 0);
        assert_eq!(observed.transition_count, 0);
        assert_eq!(observed.explored_state_count, 0);
        assert_eq!(observed.evaluated_transition_count, 0);
        assert_eq!(observed.certified_transition_count, 0);
        assert!(!observed.mutation_candidate_ready);
        assert!(!observed.authorizes_project_mutation);
        let project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .is_none()
        );
    }

    fn boundary_preflight_fixture(
        positions: [(f64, f64); 3],
        omit_last_vertex: bool,
    ) -> (
        ori_domain::CreasePattern,
        ori_domain::Paper,
        ori_domain::EdgeId,
    ) {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x74,
        ]);
        let vertices = positions
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let target_edge = EdgeId::derive_v5(namespace, b"boundary-preflight-target");
        let edges = vec![Edge {
            id: target_edge,
            start: vertices[0].id,
            end: vertices[1].id,
            kind: EdgeKind::Mountain,
        }];
        let boundary_vertices = if omit_last_vertex {
            vec![
                vertices[0].id,
                vertices[1].id,
                VertexId::derive_v5(namespace, b"missing-boundary-vertex"),
            ]
        } else {
            vertices.iter().map(|vertex| vertex.id).collect()
        };
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices,
                ..Paper::default()
            },
            target_edge,
        )
    }

    #[test]
    fn nonfinite_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, target_edge) =
            boundary_preflight_fixture([(0.0, 0.0), (1.0, 0.0), (f64::NAN, 1.0)], false);
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    #[test]
    fn degenerate_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, target_edge) =
            boundary_preflight_fixture([(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], false);
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    #[test]
    fn missing_boundary_vertex_strict_dyadic_preflight_is_unsupported_no_op() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, target_edge) =
            boundary_preflight_fixture([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)], true);
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    fn malformed_production_boundary_fixture(
        positions: [(f64, f64); 4],
        boundary_order: [usize; 4],
    ) -> (
        ori_domain::CreasePattern,
        ori_domain::Paper,
        ori_domain::EdgeId,
    ) {
        use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x75,
        ]);
        let vertices = positions
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary_vertices = boundary_order.map(|index| vertices[index].id);
        let mut edges = (0..boundary_vertices.len())
            .map(|index| Edge {
                id: EdgeId::derive_v5(namespace, &[0x40, index as u8]),
                start: boundary_vertices[index],
                end: boundary_vertices[(index + 1) % boundary_vertices.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let target_edge = EdgeId::derive_v5(namespace, b"malformed-boundary-target");
        edges.push(Edge {
            id: target_edge,
            start: vertices[0].id,
            end: vertices[2].id,
            kind: EdgeKind::Mountain,
        });
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary_vertices.to_vec(),
                ..Paper::default()
            },
            target_edge,
        )
    }

    #[test]
    fn duplicate_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, target_edge) = malformed_production_boundary_fixture(
            [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
            [0, 1, 2, 1],
        );
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    #[test]
    fn self_intersecting_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, target_edge) = malformed_production_boundary_fixture(
            [(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)],
            [0, 1, 2, 3],
        );
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    #[test]
    fn zero_length_boundary_strict_dyadic_preflight_is_unsupported_no_op() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, target_edge) = malformed_production_boundary_fixture(
            [(0.0, 0.0), (0.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
            [0, 1, 2, 3],
        );
        assert_out_of_scope_boundary_is_unsupported_no_op(pattern, paper, target_edge);
    }

    #[test]
    fn even_cycle_exact_schedules_are_admitted_by_strict_dyadic_read() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for denominator in 1..=64 {
            assert_eq!(
                bounded_primitive_endpoint_ratio_v1(1, denominator),
                Ok((1, denominator))
            );
        }
        for ratio in [(2, 3), (3, 7), (63, 64), (-2, 3), (4, 3), (7, 3), (64, 1)] {
            assert_eq!(
                bounded_primitive_endpoint_ratio_v1(ratio.0, ratio.1),
                Ok(ratio)
            );
        }
        for ratio in [(2, 3), (3, 7), (63, 64), (4, 3), (7, 3), (64, 1), (-4, 3)] {
            let angle = 2.0 * (ratio.0 as f64).atan2(ratio.1 as f64).to_degrees();
            assert_eq!(
                bounded_primitive_endpoint_ratio_for_angle_v1(angle),
                Ok(ratio)
            );
        }
        for rejected_angle in [0.0, 180.0, -180.0, f64::INFINITY] {
            assert_eq!(
                bounded_primitive_endpoint_ratio_for_angle_v1(rejected_angle),
                Err(CYCLE_PATH_UNSUPPORTED_MESSAGE)
            );
        }
        for rejected in [(2, 4), (i64::MIN, 1), (1, 0), (1, 65), (65, 64)] {
            assert_eq!(
                bounded_primitive_endpoint_ratio_v1(rejected.0, rejected.1),
                Err(CYCLE_PATH_UNSUPPORTED_MESSAGE)
            );
        }
        assert!(dyadic_request_hinge_counts_are_bounded_v1(64, Some(64)));
        assert!(!dyadic_request_hinge_counts_are_bounded_v1(65, Some(64)));
        assert!(!dyadic_request_hinge_counts_are_bounded_v1(64, Some(65)));
        let (c8_pattern, c8_paper, c8_cardinal) = octagonal_eight_sector_cycle_pattern();
        let c8_opposite = vec![c8_cardinal[0], c8_cardinal[2]];
        let (mut mixed_pattern, mixed_paper, mut mixed_hinges) =
            super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern();
        let mut gateways = mixed_pattern
            .vertices
            .iter()
            .filter(|vertex| vertex.position.x.to_bits() == 4.0_f64.to_bits())
            .collect::<Vec<_>>();
        gateways.sort_by(|first, second| first.position.y.total_cmp(&second.position.y));
        let branch_endpoints = [gateways[0].id, gateways[1].id];
        let branch = ori_domain::EdgeId::derive_v5(
            ProjectId::schema_namespace([
                0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x05, 0x61,
            ]),
            b"mixed-cactus-tree-branch",
        );
        mixed_pattern.edges.push(ori_domain::Edge {
            id: branch,
            start: branch_endpoints[0],
            end: branch_endpoints[1],
            kind: ori_domain::EdgeKind::Mountain,
        });
        mixed_hinges.push(branch);
        for (fixture_name, (pattern, mut paper, moving), kind) in [
            ("balloon-c6", balloon_six_sector_cycle_pattern(), 0),
            ("octagonal-c8", (c8_pattern, c8_paper, c8_opposite), 0),
            (
                "mixed-cactus-branch",
                (mixed_pattern, mixed_paper, mixed_hinges),
                1,
            ),
        ] {
            paper.thickness_mm = 0.1;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect::<Vec<_>>();
            let fixed = if kind == 1 {
                snapshot
                    .faces
                    .iter()
                    .find(|face| {
                        snapshot
                            .hinge_adjacency
                            .iter()
                            .filter(|hinge| hinge.first == face.id || hinge.second == face.id)
                            .count()
                            == 2
                    })
                    .unwrap()
                    .id
            } else {
                snapshot.faces[0].id
            };
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let layer_state = GlobalFlatFoldabilityState::default();
            super::super::global_flat_foldability::tests::install_possible_layer_order(
                &layer_state,
                &project,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let state = AppState::new(project);
            let schedule = if kind == 1 {
                four_bay_cycle_schedule(&moving)
            } else {
                let endpoint_ratio = match hinges.len() {
                    6 => (4, 3),
                    8 => (7, 3),
                    16 => (64, 1),
                    _ => unreachable!("bounded opposite-pair fixture"),
                };
                dense_grid_schedule_ratio(&hinges, &moving, endpoint_ratio.0, endpoint_ratio.1)
            };
            let target = {
                let project = super::super::lock_project(&state).unwrap();
                let capability = project
                    .applied_pose_authority
                    .capture_capability(&project)
                    .unwrap()
                    .unwrap();
                let (geometry, audit, pose) = capability.graph().unwrap();
                if fixture_name == "mixed-cactus-branch" {
                    assert!(audit.closure_hinges().len() >= 4);
                    assert!(audit.spanning_hinges().contains(&branch));
                }
                prepare_requested_cycle_schedule_v1(
                    &schedule,
                    geometry,
                    audit,
                    pose.fixed_face(),
                    pose.hinge_angles(),
                )
                .unwrap()
                .evaluate(1.0)
                .unwrap()
            };
            if fixture_name == "mixed-cactus-branch" {
                assert!(
                    target
                        .as_slice()
                        .iter()
                        .any(|angle| { angle.edge() == branch && angle.angle_degrees() > 0.0 })
                );
            }
            let target_angles = target
                .as_slice()
                .iter()
                .map(|angle| DyadicPoseGraphAngleDtoV1 {
                    edge: angle.edge(),
                    angle_degrees: angle.angle_degrees(),
                })
                .collect::<Vec<_>>();
            let observed = read_bounded_dyadic_pose_graph_inner_v1(
                &state,
                Some(&layer_state),
                DyadicPoseGraphReadRequestV1 {
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    target_angles: target_angles
                        .iter()
                        .map(|angle| DyadicPoseGraphAngleDtoV1 {
                            edge: angle.edge,
                            angle_degrees: angle.angle_degrees,
                        })
                        .collect(),
                    max_states: 32,
                    max_transitions: 128,
                    level_count: 3,
                    cycle_schedule_v1: None,
                },
                None,
            )
            .unwrap_or_else(|error| panic!("{fixture_name} exact schedule dyadic read: {error}"));
            assert_eq!(observed.status, "certified");
            assert_eq!(observed.state_count, 3);
            assert_eq!(observed.transition_count, 4);
            assert!(observed.certified_transition_count > 0);
            assert!(observed.positive_thickness_certified);
            assert!(observed.layer_transport_certified);
            assert!(observed.mutation_candidate_ready);
            assert!(!observed.authorizes_project_mutation);

            let expected_steps = observed.certified_transition_count + 1;
            let preview_state = DyadicPathPreviewState::default();
            let preview = mint_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                DyadicPathPreviewRequestV1 {
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    target_angles,
                    max_states: 32,
                    max_transitions: 128,
                    level_count: 3,
                    cycle_schedule_v1: None,
                    expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
                    expected_positive_thickness_binding_sha256: observed
                        .positive_thickness_binding_sha256
                        .unwrap(),
                    expected_layer_transport_binding_sha256: observed
                        .layer_transport_binding_sha256
                        .unwrap(),
                },
            )
            .unwrap_or_else(|error| panic!("{fixture_name} proof families mint preview: {error}"));
            let apply_request =
                |expected_revision: u64, path: String| ApplyDyadicPathPreviewRequestV1 {
                    preview_token: preview.preview_token,
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision,
                    expected_target_binding_sha256: preview.target_binding_sha256.clone(),
                    expected_path_binding_sha256: path,
                    expected_positive_thickness_binding_sha256: preview
                        .positive_thickness_binding_sha256
                        .clone(),
                    expected_layer_transport_binding_sha256: preview
                        .layer_transport_binding_sha256
                        .clone(),
                };
            for rejected in [
                apply_request(revision, "00".repeat(32)),
                apply_request(revision + 1, preview.path_binding_sha256.clone()),
            ] {
                assert!(
                    apply_dyadic_pose_path_preview_inner_v1(
                        &state,
                        &layer_state,
                        &preview_state,
                        rejected,
                    )
                    .is_err()
                );
                assert_eq!(
                    super::super::lock_project(&state)
                        .unwrap()
                        .editor
                        .revision(),
                    revision,
                    "tamper and stale attempts are atomic no-ops"
                );
            }
            let applied = apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request(revision, preview.path_binding_sha256.clone()),
            )
            .unwrap_or_else(|error| panic!("{fixture_name} path applies atomically: {error}"));
            assert!(
                apply_dyadic_pose_path_preview_inner_v1(
                    &state,
                    &layer_state,
                    &preview_state,
                    apply_request(revision, preview.path_binding_sha256),
                )
                .is_err()
            );
            let mut project = super::super::lock_project(&state).unwrap();
            assert_eq!(applied, revision + 1);
            assert_eq!(
                project.editor.instruction_timeline().steps.len(),
                expected_steps
            );
            assert!(
                project.editor.instruction_timeline().steps[1..]
                    .iter()
                    .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
            );
            project.editor.undo(applied).unwrap();
            assert!(project.editor.instruction_timeline().steps.is_empty());
            let undone = project.editor.revision();
            project.editor.redo(undone).unwrap();
            assert_eq!(
                project.editor.instruction_timeline().steps.len(),
                expected_steps
            );
            let archive = project.project_archive().unwrap();
            drop(project);
            let reopened = super::super::ProjectState::from_project_archive(
                archive,
                std::path::PathBuf::from(format!("{fixture_name}-dyadic-authority.ori2")),
            )
            .expect("reopen proof-bearing degree-six balloon path");
            assert_eq!(
                reopened.editor.instruction_timeline().steps.len(),
                expected_steps
            );
            assert!(
                reopened.editor.instruction_timeline().steps[1..]
                    .iter()
                    .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
            );
            // Instruction rendering has its own bounded topology contract;
            // this regression authenticates exact cycle read/apply/history.
        }
    }

    #[test]
    fn automatic_kawasaki_archive_reopens_with_native_pose_authority() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (mut project, hinges) = super::super::applied_pose::tests::four_vertex_cycle_project();
        super::super::applied_pose::tests::install_flat_graph_pose_authority(&mut project, hinges);
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let preview = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: CycleScheduleRequestV1 {
                    version: 2,
                    entries: Vec::new(),
                    endpoint_denominator: None,
                },
            },
        )
        .expect("automatic exact Kawasaki preview");
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            preview.transaction_token,
        )
        .expect("apply automatic exact Kawasaki pose");
        let project = super::super::lock_project(&state).unwrap();
        let original_pose = project.editor.instruction_timeline().steps[0].pose.clone();
        let archive = project.project_archive().unwrap();
        let mut tampered = project.document();
        tampered.instruction_timeline.steps[0].pose.hinge_angles[0].angle_degrees += 0.01;
        assert!(super::super::validate_document_instruction_poses(&tampered).is_err());
        drop(project);
        let reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("automatic-kawasaki.ori2"),
        )
        .expect("reopen automatic exact Kawasaki archive");
        assert_eq!(
            reopened.editor.instruction_timeline().steps[0].pose,
            original_pose
        );
        assert!(
            reopened
                .applied_pose_authority
                .capture_capability(&reopened)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn uncertified_rational_kawasaki_endpoints_are_atomic_no_ops() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for (numerator, denominator, complement) in [(5.0, 13.0, 12.0), (7.0, 25.0, 24.0)] {
            let (mut project, hinges) =
                uncertified_rational_kawasaki_project(numerator, denominator, complement);
            super::super::applied_pose::tests::install_flat_graph_pose_authority(
                &mut project,
                hinges,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            let endpoint_read = read_even_cycle_candidates_inner_v1(
                &state,
                EvenCycleCandidatesRequestV1 {
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    max_pair_tests: 6,
                },
            )
            .unwrap();
            assert_eq!(endpoint_read.kawasaki_endpoints.len(), 5);
            assert!(endpoint_read.kawasaki_endpoints.iter().all(|candidate| {
                candidate.closure_status == "certified"
                    && candidate.collision_status == "uncertified"
                    && !candidate.authorizes_apply
            }));
            let result = propose_current_cycle_pose_inner(
                None,
                &state,
                &transactions,
                CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    cycle_schedule_v1: CycleScheduleRequestV1 {
                        version: 2,
                        entries: Vec::new(),
                        endpoint_denominator: Some(16),
                    },
                },
            );
            assert!(matches!(
                result,
                Err(reason) if reason == CYCLE_PATH_UNCERTIFIED_MESSAGE
            ));
            let project = super::super::lock_project(&state).unwrap();
            assert_eq!(project.editor.revision(), revision);
            assert!(project.editor.instruction_timeline().steps.is_empty());
            assert!(
                project
                    .applied_pose_authority
                    .capture_capability(&project)
                    .unwrap()
                    .is_some(),
                "a rejected preview must not consume source pose authority"
            );
        }
    }

    #[test]
    fn octagonal_eight_sector_cycle_previews_applies_and_reopens_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, moving) = octagonal_eight_sector_cycle_pattern();
        assert_eq!(
            pattern
                .edges
                .iter()
                .filter(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
                .count(),
            5
        );
        assert_eq!(
            pattern
                .edges
                .iter()
                .filter(|edge| edge.kind == ori_domain::EdgeKind::Valley)
                .count(),
            3
        );
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 8);
        assert_eq!(snapshot.hinge_adjacency.len(), 8);
        assert!(
            automatic_opposite_pairs(&project, &snapshot)
                .iter()
                .any(|pair| pair.iter().all(|edge| moving.contains(edge)))
        );
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        let fixed = snapshot.faces[0].id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let opposite_pair = vec![moving[0], moving[2]];
        assert_eq!(
            propose_current_cycle_pose_inner(
                None,
                &state,
                &transactions,
                CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    cycle_schedule_v1: dense_grid_schedule(&hinges, &moving, 100),
                },
            )
            .unwrap_err(),
            CYCLE_NONCLOSING_MESSAGE
        );
        let preview = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &opposite_pair, 100),
            },
        )
        .expect("octagonal straight-line cycle must certify");
        assert_eq!(preview.checked_hinge_count, 8);
        assert_eq!(preview.total_hinge_count, 8);
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            preview.transaction_token,
        )
        .expect("octagonal straight-line cycle apply");
        let mut project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        project.editor.undo(applied).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        let archive = project
            .project_archive()
            .expect("serialize applied octagonal cycle");
        let mut reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("octagonal-cycle.ori2"),
        )
        .expect("reopen applied octagonal cycle");
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
        let reopened_revision = reopened.editor.revision();
        reopened.editor.undo(reopened_revision).unwrap();
        assert!(reopened.editor.instruction_timeline().steps.is_empty());
        let reopened_undone = reopened.editor.revision();
        reopened.editor.redo(reopened_undone).unwrap();
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
    }

    #[test]
    fn sixteen_sector_upper_bound_previews_applies_reopens_and_rejects_nonopposite_pair() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, moving) = sixteen_sector_cycle_pattern(8);
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        assert_eq!(snapshot.faces.len(), 16);
        assert_eq!(snapshot.hinge_adjacency.len(), 16);
        let graph_geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
            project.editor.pattern(),
            project.editor.paper(),
            &snapshot,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .unwrap();
        let graph_audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
            &snapshot,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .unwrap();
        let automatic_pairs = ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
            &graph_geometry,
            &graph_audit,
            120,
        )
        .expect("bounded C16 opposite-pair discovery");
        assert!(
            automatic_pairs
                .iter()
                .any(|pair| { pair.iter().all(|edge| moving.contains(edge)) })
        );
        assert!(matches!(
            ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
                &graph_geometry,
                &graph_audit,
                119,
            ),
            Err(ori_kinematics::KinematicsError::ResourceLimitExceeded)
        ));
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        let fixed = snapshot.faces[0].id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let preview = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &moving, 100),
            },
        )
        .expect("sixteen-sector opposite pair must certify");
        assert_eq!(preview.checked_hinge_count, 16);
        assert_eq!(preview.total_hinge_count, 16);
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            preview.transaction_token,
        )
        .expect("sixteen-sector opposite pair apply");
        let second_preview = propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: applied,
                cycle_schedule_v1: advance_collective_schedule(&hinges, &moving, 100),
            },
        )
        .expect("C16 rebound authority must authorize the second preview");
        assert_eq!(second_preview.checked_hinge_count, 16);
        assert_eq!(second_preview.total_hinge_count, 16);
        let second_applied =
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &state,
                &GlobalFlatFoldabilityState::default(),
                &transactions,
                second_preview.transaction_token,
            )
            .expect("second C16 operation applies atomically");
        let mut project = super::super::lock_project(&state).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
        project.editor.undo(second_applied).unwrap();
        let first_undone = project.editor.revision();
        project.editor.undo(first_undone).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        let first_redo = project.editor.revision();
        project.editor.redo(first_redo).unwrap();
        let second_redo = project.editor.revision();
        project.editor.redo(second_redo).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
        let archive = project.project_archive().expect("serialize C16 cycle");
        let mut reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("sixteen-cycle.ori2"),
        )
        .expect("reopen C16 cycle");
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
        let reopened_revision = reopened.editor.revision();
        reopened.editor.undo(reopened_revision).unwrap();
        let reopened_first_undone = reopened.editor.revision();
        reopened.editor.undo(reopened_first_undone).unwrap();
        assert!(reopened.editor.instruction_timeline().steps.is_empty());
        let reopened_first_redo = reopened.editor.revision();
        reopened.editor.redo(reopened_first_redo).unwrap();
        let reopened_second_redo = reopened.editor.revision();
        reopened.editor.redo(reopened_second_redo).unwrap();
        assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);

        let (pattern, paper, nonopposite) = sixteen_sector_cycle_pattern(7);
        let mut rejected = super::super::ProjectState::new_with_paper(pattern, paper);
        let rejected_topology = rejected
            .editor
            .topology_analysis_input(rejected.project_id)
            .analyze();
        let rejected_snapshot = rejected_topology.simulation_snapshot().unwrap();
        let rejected_hinges = rejected_snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut rejected,
            rejected_hinges.clone(),
            rejected_snapshot.faces[0].id,
        );
        let rejected_instance = rejected.instance_id;
        let rejected_project_id = rejected.project_id;
        let rejected_revision = rejected.editor.revision();
        let rejected_state = AppState::new(rejected);
        assert_eq!(
            propose_current_cycle_pose_inner(
                None,
                &rejected_state,
                &super::super::stacked_fold_transaction::StackedFoldTransactionState::default(),
                CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: rejected_instance,
                    expected_project_id: rejected_project_id,
                    expected_revision: rejected_revision,
                    cycle_schedule_v1: dense_grid_schedule(&rejected_hinges, &nonopposite, 100,),
                },
            )
            .unwrap_err(),
            CYCLE_NONCLOSING_MESSAGE
        );
    }

    #[test]
    fn four_leaf_cycle_preview_applies_atomically_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, hinges) =
            super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern();
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let fixed = snapshot
            .faces
            .iter()
            .max_by_key(|face| {
                snapshot
                    .hinge_adjacency
                    .iter()
                    .filter(|adjacency| adjacency.first == face.id || adjacency.second == face.id)
                    .count()
            })
            .unwrap()
            .id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        {
            let capability = project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .unwrap();
            let (geometry, audit, _) = capability.graph().unwrap();
            let basis = geometry
                .extract_canonical_cycle_basis_v1(audit, CycleBasisLimitsV1::default())
                .expect("four-cycle canonical basis");
            assert_eq!(basis.cycles().len(), 4);
            assert!(
                geometry
                    .extract_canonical_cycle_basis_v1(
                        audit,
                        CycleBasisLimitsV1 {
                            max_cycles: 3,
                            ..CycleBasisLimitsV1::default()
                        },
                    )
                    .is_err()
            );
        }
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let transaction_state =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let mut corrupted = four_bay_cycle_schedule(&hinges);
        corrupted
            .entries
            .iter_mut()
            .find(|entry| entry.edge == hinges[12])
            .unwrap()
            .numerator_power_coefficients[1]
            .numerator += 1;
        assert!(
            propose_current_cycle_pose_inner(
                None,
                &app_state,
                &transaction_state,
                CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    cycle_schedule_v1: corrupted,
                },
            )
            .is_err()
        );
        assert!(
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &GlobalFlatFoldabilityState::default(),
                &transaction_state,
                ProjectId::new(),
            )
            .is_err()
        );
        let response = propose_current_cycle_pose_inner(
            None,
            &app_state,
            &transaction_state,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
            },
        )
        .expect("four-leaf authenticated preview");
        assert_eq!(response.closure_leaf_count, 4);
        assert_eq!(response.closure_max_depth, 2);
        assert_eq!(response.checked_hinge_count, 16);
        assert_eq!(response.total_hinge_count, 16);
        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
            &transaction_state,
            response.transaction_token,
        )
        .unwrap();
        assert!(
            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                &app_state,
                &GlobalFlatFoldabilityState::default(),
                &transaction_state,
                response.transaction_token,
            )
            .is_err()
        );
        let response = propose_current_cycle_pose_inner(
            None,
            &app_state,
            &transaction_state,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
            },
        )
        .expect("four-leaf retry preview");
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &GlobalFlatFoldabilityState::default(),
            &transaction_state,
            response.transaction_token,
        )
        .expect("four-leaf atomic apply");
        let mut project = super::super::lock_project(&app_state).unwrap();
        assert_eq!(applied, revision + 1);
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        project.editor.undo(applied).unwrap();
        assert!(project.editor.instruction_timeline().steps.is_empty());
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    }

    #[test]
    fn coupled_cactus_previews_apply_and_round_trip_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for cycle_count in [2, 3, 4, 8, 16, 32] {
            for thickness_mm in [10_000.0, 0.1, 1.0, 3.0] {
                let (pattern, mut paper, hinges) = if cycle_count == 2 {
                    super::four_bay_cycle_test_support::two_bay_rational_cycle_pattern()
                } else if cycle_count == 3 {
                    super::four_bay_cycle_test_support::three_bay_rational_cycle_pattern()
                } else if cycle_count == 4 {
                    super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern()
                } else if cycle_count == 8 {
                    super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern()
                } else if cycle_count == 32 {
                    super::four_bay_cycle_test_support::thirty_two_bay_rational_cycle_pattern()
                } else {
                    super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern()
                };
                paper.thickness_mm = thickness_mm;
                let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
                let topology = project
                    .editor
                    .topology_analysis_input(project.project_id)
                    .analyze();
                let snapshot = topology.simulation_snapshot().unwrap();
                let fixed = snapshot
                    .faces
                    .iter()
                    .find(|face| {
                        snapshot
                            .hinge_adjacency
                            .iter()
                            .filter(|adjacency| {
                                adjacency.first == face.id || adjacency.second == face.id
                            })
                            .count()
                            == 2
                    })
                    .unwrap()
                    .id;
                super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                    &mut project,
                    hinges.clone(),
                    fixed,
                );
                let instance = project.instance_id;
                let project_id = project.project_id;
                let revision = project.editor.revision();
                let app_state = AppState::new(project);
                let transactions =
                    super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
                if thickness_mm < 10_000.0 && cycle_count < 32 {
                    assert!(
                        crate::applied_pose::certify_current_static_collision(
                            &app_state,
                            ori_collision::StaticCollisionLimits::default(),
                        )
                        .expect("flat cactus current collision diagnosis")
                        .is_some()
                    );
                }
                if cycle_count == 32 {
                    assert_eq!(
                        propose_current_cycle_pose_inner(
                            None,
                            &app_state,
                            &transactions,
                            CurrentCyclePosePreviewRequestV1 {
                                progress_request_id: Some("rank32:stale".to_owned()),
                                expected_project_instance_id: ProjectId::new(),
                                expected_project_id: project_id,
                                expected_revision: revision,
                                cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
                            },
                        )
                        .unwrap_err(),
                        STALE_MESSAGE
                    );
                }
                let response = propose_current_cycle_pose_inner(
                    None,
                    &app_state,
                    &transactions,
                    CurrentCyclePosePreviewRequestV1 {
                        progress_request_id: None,
                        expected_project_instance_id: instance,
                        expected_project_id: project_id,
                        expected_revision: revision,
                        cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
                    },
                );
                if thickness_mm == 10_000.0 {
                    assert_eq!(response.unwrap_err(), CYCLE_PATH_UNCERTIFIED_MESSAGE);
                    let project = super::super::lock_project(&app_state).unwrap();
                    assert!(project.editor.instruction_timeline().steps.is_empty());
                    assert_eq!(project.editor.revision(), revision);
                    assert!(
                        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                            &app_state,
                            &GlobalFlatFoldabilityState::default(),
                            &transactions,
                            ProjectId::new(),
                        )
                        .is_err()
                    );
                    continue;
                }
                let response = response.expect("coupled cactus preview");
                assert_eq!(response.closure_leaf_count, cycle_count);
                assert_eq!(response.checked_hinge_count, cycle_count * 4);
                if cycle_count == 32 && thickness_mm == 1.0 {
                    super::super::lock_project(&app_state).unwrap().instance_id = ProjectId::new();
                    assert!(
                        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                            &app_state,
                            &GlobalFlatFoldabilityState::default(),
                            &transactions,
                            response.transaction_token,
                        )
                        .is_err()
                    );
                    assert!(
                        super::super::lock_project(&app_state)
                            .unwrap()
                            .editor
                            .instruction_timeline()
                            .steps
                            .is_empty()
                    );
                    continue;
                }
                super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
                    &transactions,
                    response.transaction_token,
                )
                .unwrap();
                assert!(
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &app_state,
                        &GlobalFlatFoldabilityState::default(),
                        &transactions,
                        response.transaction_token,
                    )
                    .is_err()
                );
                let response = propose_current_cycle_pose_inner(
                    None,
                    &app_state,
                    &transactions,
                    CurrentCyclePosePreviewRequestV1 {
                        progress_request_id: None,
                        expected_project_instance_id: instance,
                        expected_project_id: project_id,
                        expected_revision: revision,
                        cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
                    },
                )
                .expect("coupled cactus retry");
                let applied =
                    super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &app_state,
                        &GlobalFlatFoldabilityState::default(),
                        &transactions,
                        response.transaction_token,
                    )
                    .unwrap();
                let mut project = super::super::lock_project(&app_state).unwrap();
                project.editor.undo(applied).unwrap();
                let undone = project.editor.revision();
                project.editor.redo(undone).unwrap();
                assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
                if cycle_count == 16 {
                    let archive = project
                        .project_archive()
                        .expect("serialize positive-thickness C16 cycle");
                    let mut reopened = super::super::ProjectState::from_project_archive(
                        archive,
                        std::path::PathBuf::from(format!("positive-c16-{thickness_mm}.ori2")),
                    )
                    .expect("reopen positive-thickness C16 cycle");
                    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
                    let reopened_revision = reopened.editor.revision();
                    reopened.editor.undo(reopened_revision).unwrap();
                    assert!(reopened.editor.instruction_timeline().steps.is_empty());
                    let reopened_redo = reopened.editor.revision();
                    reopened.editor.redo(reopened_redo).unwrap();
                    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 1);
                }
            }
        }
    }

    #[test]
    fn rank4_cycle_transports_layer_order_and_applies_atomically() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for (columns, rows, thickness_mm, expected_cycle_rank) in [
            (3, 3, 0.1, 4),
            (3, 3, 1.0, 4),
            (3, 3, 3.0, 4),
            (3, 3, 10_000.0, 4),
            (3, 5, 0.1, 8),
            (5, 5, 0.1, 16),
            (5, 9, 0.1, 32),
            (7, 7, 0.1, 36),
            (7, 9, 0.1, 48),
            (8, 9, 0.1, 56),
            (9, 9, 0.1, 64),
        ] {
            let (pattern, mut paper, horizontal, _) =
                super::dense_grid_cycle_test_support::miura_authority_pattern(columns, rows);
            let moving = horizontal.into_iter().take(columns).collect::<Vec<_>>();
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            assert_eq!(
                snapshot.hinge_adjacency.len() + 1 - snapshot.faces.len(),
                expected_cycle_rank
            );
            let hinges = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| hinge.edge)
                .collect::<Vec<_>>();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let layer_state = GlobalFlatFoldabilityState::default();
            super::super::global_flat_foldability::tests::install_possible_layer_order(
                &layer_state,
                &project,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let app_state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            let schedule_for = |mask: usize| {
                let mut schedule = dense_grid_schedule(&hinges, &moving, 100);
                for (index, entry) in schedule
                    .entries
                    .iter_mut()
                    .filter(|entry| moving.contains(&entry.edge))
                    .enumerate()
                {
                    let mountain = snapshot
                        .hinge_adjacency
                        .iter()
                        .find(|hinge| hinge.edge == entry.edge)
                        .is_some_and(|hinge| {
                            hinge.assignment == ori_topology::FoldAssignment::Mountain
                        });
                    if mountain ^ (mask & (1 << index) != 0) {
                        entry.numerator_power_coefficients[1].numerator *= -1;
                        entry.requested_angle_degrees *= -1.0;
                    }
                }
                schedule
            };
            let request = |expected_project_instance_id, mask| CurrentCyclePosePreviewRequestV1 {
                progress_request_id: Some("rank4:layer".to_owned()),
                expected_project_instance_id,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: schedule_for(mask),
            };
            assert_eq!(
                propose_current_cycle_pose_inner_with_layers(
                    None,
                    &app_state,
                    Some(&layer_state),
                    &transactions,
                    request(ProjectId::new(), 0),
                )
                .unwrap_err(),
                STALE_MESSAGE
            );
            if thickness_mm == 10_000.0 {
                assert!((0..(1usize << moving.len())).all(|mask| {
                    propose_current_cycle_pose_inner_with_layers(
                        None,
                        &app_state,
                        Some(&layer_state),
                        &transactions,
                        request(instance, mask),
                    )
                    .is_err()
                }));
                let project = super::super::lock_project(&app_state).unwrap();
                assert_eq!(project.editor.revision(), revision);
                assert!(project.editor.instruction_timeline().steps.is_empty());
                continue;
            }
            let mut malformed = request(instance, 0);
            malformed.cycle_schedule_v1.entries[0].denominator_power_coefficients[0].numerator = 0;
            assert!(
                propose_current_cycle_pose_inner_with_layers(
                    None,
                    &app_state,
                    Some(&layer_state),
                    &transactions,
                    malformed,
                )
                .is_err()
            );
            assert!(
                super::super::lock_project(&app_state)
                    .unwrap()
                    .editor
                    .instruction_timeline()
                    .steps
                    .is_empty()
            );
            let Some((closing_mask, preview)) = (0..(1usize << moving.len())).find_map(|mask| {
                propose_current_cycle_pose_inner_with_layers(
                    None,
                    &app_state,
                    Some(&layer_state),
                    &transactions,
                    request(instance, mask),
                )
                .ok()
                .map(|preview| (mask, preview))
            }) else {
                let project = super::super::lock_project(&app_state).unwrap();
                assert_eq!(project.editor.revision(), revision);
                assert!(project.editor.instruction_timeline().steps.is_empty());
                continue;
            };
            assert_eq!(
                preview.continuous_layer_transport_model_id,
                Some(ori_collision::GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1)
            );
            assert_eq!(preview.continuous_layer_transition_count, 2);
            assert_eq!(preview.source_layer_order, preview.target_layer_order);
            assert_eq!(
                preview.continuous_layer_pair_order_count,
                preview.source_layer_order.len()
            );
            assert!(!preview.authorizes_project_mutation);
            let cancelled = preview.transaction_token;
            super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
                &transactions,
                cancelled,
            )
            .unwrap();
            assert!(
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &layer_state,
                    &transactions,
                    cancelled,
                )
                .is_err()
            );
            let stale_authority_preview = propose_current_cycle_pose_inner_with_layers(
                None,
                &app_state,
                Some(&layer_state),
                &transactions,
                request(instance, closing_mask),
            )
            .expect("rank4 layer authority ABA preview");
            {
                let project = super::super::lock_project(&app_state).unwrap();
                super::super::global_flat_foldability::tests::install_possible_layer_order(
                    &layer_state,
                    &project,
                );
            }
            assert!(
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &layer_state,
                    &transactions,
                    stale_authority_preview.transaction_token,
                )
                .is_err()
            );
            let preview = propose_current_cycle_pose_inner_with_layers(
                None,
                &app_state,
                Some(&layer_state),
                &transactions,
                request(instance, closing_mask),
            )
            .expect("rank4 layer transport retry");
            let applied =
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &layer_state,
                    &transactions,
                    preview.transaction_token,
                )
                .expect("rank4 layer transport apply");
            let mut project = super::super::lock_project(&app_state).unwrap();
            let persisted = project.editor.instruction_timeline().steps[0]
                .visual
                .cycle_layer_order_proof_v1
                .as_ref()
                .expect("applied transport proof is persisted in timeline history");
            assert_eq!(persisted.version, 1);
            assert_eq!(
                persisted.model_id,
                ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1
            );
            assert_eq!(persisted.target_order_sha256.len(), 32);
            if expected_cycle_rank == 16 {
                let pose = project.editor.current_applied_pose().unwrap();
                let fixed_face = pose.fixed_face().unwrap();
                let angles = ori_kinematics::CanonicalHingeAngles::new(
                    pose.hinge_angles()
                        .iter()
                        .map(|angle| {
                            ori_kinematics::HingeAngle::new(angle.edge(), angle.angle_degrees())
                                .unwrap()
                        })
                        .collect(),
                )
                .unwrap();
                let flat =
                    super::super::global_flat_foldability::reanalyze_current_flat_layer_order(
                        &project,
                    )
                    .unwrap();
                let proof = ori_core::revalidate_current_graph_non_flat_layer_order_v1(
                    ori_core::RevalidateCurrentGraphNonFlatLayerOrderRequestV1 {
                        identity_namespace: project.project_id,
                        revision: project.editor.revision(),
                        pattern: project.editor.pattern(),
                        paper: project.editor.paper(),
                        fixed_face,
                        hinge_angles: &angles,
                        current_flat: &flat,
                        expected_archive: None,
                        max_face_pairs: ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
                    },
                )
                .unwrap();
                project.current_layer_evidence = Some(
                    super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(proof),
                );
                let archive = project.project_archive().unwrap();
                assert!(matches!(
                    &archive.layer_evidence,
                    Some(ori_formats::LayerEvidenceArchiveV1 {
                        evidence: ori_formats::LayerEvidenceArchiveKindV1::NonFlat { .. },
                        ..
                    })
                ));
                let mut reopened = super::super::ProjectState::from_project_archive(
                    archive,
                    std::path::PathBuf::from("rank16-graph-layer-evidence.ori2"),
                )
                .unwrap();
                assert!(matches!(
                    &reopened.current_layer_evidence,
                    Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(_))
                ));
                let revision = reopened.editor.revision();
                let reopened_instance = reopened.instance_id;
                let reopened_project = reopened.project_id;
                let vertex = reopened.editor.pattern().vertices[0].id;
                let position = reopened.editor.pattern().vertices[0].position;
                super::super::execute_command(
                    &mut reopened,
                    reopened_instance,
                    reopened_project,
                    revision,
                    ori_core::Command::MoveVertex {
                        id: vertex,
                        position: ori_domain::Point2::new(position.x + 0.125, position.y),
                    },
                )
                .unwrap();
                assert!(reopened.current_layer_evidence.is_none());
            }
            project.editor.undo(applied).unwrap();
            assert!(project.editor.instruction_timeline().steps.is_empty());
            let undone = project.editor.revision();
            project.editor.redo(undone).unwrap();
            assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
            assert!(
                project.editor.instruction_timeline().steps[0]
                    .visual
                    .cycle_layer_order_proof_v1
                    .is_some()
            );
            let reopened = super::super::ProjectState::from_document(
                project.document(),
                std::path::PathBuf::from("miura-cell-transport-reopened.ori2"),
            );
            let reopened_proof = reopened.editor.instruction_timeline().steps[0]
                .visual
                .cycle_layer_order_proof_v1
                .as_ref()
                .expect("persisted Miura cell proof survives reopen");
            assert_eq!(
                reopened_proof.model_id,
                ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1
            );
            assert_eq!(reopened_proof.target_order_sha256.len(), 32);
        }
    }

    #[test]
    fn dyadic_private_authority_applies_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, mut paper, horizontal, _) =
            super::dense_grid_cycle_test_support::miura_authority_pattern(3, 3);
        paper.thickness_mm = 0.1;
        let moving = horizontal.into_iter().take(3).collect::<Vec<_>>();
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let hinges = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            snapshot.faces[0].id,
        );
        let layer_state = GlobalFlatFoldabilityState::default();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);
        let preview_state = DyadicPathPreviewState::default();
        let schedule_for = |mask: usize| {
            let mut schedule = dense_grid_schedule(&hinges, &moving, 100);
            for (index, entry) in schedule
                .entries
                .iter_mut()
                .filter(|entry| moving.contains(&entry.edge))
                .enumerate()
            {
                let mountain = snapshot
                    .hinge_adjacency
                    .iter()
                    .find(|hinge| hinge.edge == entry.edge)
                    .is_some_and(|hinge| {
                        hinge.assignment == ori_topology::FoldAssignment::Mountain
                    });
                if mountain ^ (mask & (1 << index) != 0) {
                    entry.numerator_power_coefficients[1].numerator *= -1;
                    entry.requested_angle_degrees *= -1.0;
                }
            }
            schedule
        };
        let (schedule, target, observed) = (0..8)
            .find_map(|mask| {
                let schedule = schedule_for(mask);
                let target = {
                    let project = super::super::lock_project(&state).unwrap();
                    let capability = project
                        .applied_pose_authority
                        .capture_capability(&project)
                        .ok()??;
                    let (geometry, audit, pose) = capability.graph()?;
                    prepare_requested_cycle_schedule_v1(
                        &schedule,
                        geometry,
                        audit,
                        pose.fixed_face(),
                        pose.hinge_angles(),
                    )
                    .ok()?
                    .evaluate(1.0)?
                };
                let request = DyadicPoseGraphReadRequestV1 {
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    target_angles: target
                        .as_slice()
                        .iter()
                        .map(|angle| DyadicPoseGraphAngleDtoV1 {
                            edge: angle.edge(),
                            angle_degrees: angle.angle_degrees(),
                        })
                        .collect(),
                    max_states: 32,
                    max_transitions: 128,
                    level_count: 3,
                    cycle_schedule_v1: Some(schedule.clone()),
                };
                let value = read_bounded_dyadic_pose_graph_inner_v1(
                    &state,
                    Some(&layer_state),
                    request,
                    None,
                )
                .ok()?;
                value
                    .mutation_candidate_ready
                    .then_some((schedule, target, value))
            })
            .expect("real Miura schedule yields a certified dyadic candidate");
        let expected_steps = observed.certified_transition_count + 1;
        let request = DyadicPathPreviewRequestV1 {
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            target_angles: target
                .as_slice()
                .iter()
                .map(|angle| DyadicPoseGraphAngleDtoV1 {
                    edge: angle.edge(),
                    angle_degrees: angle.angle_degrees(),
                })
                .collect(),
            max_states: 32,
            max_transitions: 128,
            level_count: 3,
            cycle_schedule_v1: Some(schedule),
            expected_path_binding_sha256: observed.certificate_binding_sha256.unwrap(),
            expected_positive_thickness_binding_sha256: observed
                .positive_thickness_binding_sha256
                .unwrap(),
            expected_layer_transport_binding_sha256: observed
                .layer_transport_binding_sha256
                .unwrap(),
        };
        let preview =
            mint_dyadic_pose_path_preview_inner_v1(&state, &layer_state, &preview_state, request)
                .unwrap();
        let apply_request = |path: String| ApplyDyadicPathPreviewRequestV1 {
            preview_token: preview.preview_token,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            expected_target_binding_sha256: preview.target_binding_sha256.clone(),
            expected_path_binding_sha256: path,
            expected_positive_thickness_binding_sha256: preview
                .positive_thickness_binding_sha256
                .clone(),
            expected_layer_transport_binding_sha256: preview.layer_transport_binding_sha256.clone(),
        };
        assert!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request("00".repeat(32))
            )
            .is_err()
        );
        assert_eq!(
            super::super::lock_project(&state)
                .unwrap()
                .editor
                .revision(),
            revision
        );
        let applied = apply_dyadic_pose_path_preview_inner_v1(
            &state,
            &layer_state,
            &preview_state,
            apply_request(preview.path_binding_sha256.clone()),
        )
        .unwrap();
        assert!(
            apply_dyadic_pose_path_preview_inner_v1(
                &state,
                &layer_state,
                &preview_state,
                apply_request(preview.path_binding_sha256.clone())
            )
            .is_err()
        );
        let mut project = super::super::lock_project(&state).unwrap();
        assert_eq!(applied, revision + 1);
        assert_eq!(
            project.editor.instruction_timeline().steps.len(),
            expected_steps
        );
        assert!(
            project.editor.instruction_timeline().steps[1..]
                .iter()
                .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
        );
        project.editor.undo(applied).unwrap();
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        let archive = project.project_archive().unwrap();
        let reopened = super::super::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("dyadic-authority.ori2"),
        )
        .unwrap();
        assert_eq!(
            reopened.editor.instruction_timeline().steps.len(),
            expected_steps
        );
        assert!(
            reopened.editor.instruction_timeline().steps[1..]
                .iter()
                .all(|step| { step.visual.path_certificate_reference_v1.is_some() })
        );
    }

    #[test]
    fn theta_positive_thickness_preview_applies_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        for thickness_mm in [0.1, 1.0, 3.0] {
            let (pattern, mut paper, hinges, moving) =
                super::theta_cycle_test_support::theta_shared_hinge_pattern();
            paper.thickness_mm = thickness_mm;
            let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology.simulation_snapshot().unwrap();
            let fixed = snapshot.faces[0].id;
            super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
                &mut project,
                hinges.clone(),
                fixed,
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let app_state = AppState::new(project);
            let transactions =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            let request = || CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: theta_cycle_schedule(&hinges, &moving),
            };
            let mut broken = request();
            broken.cycle_schedule_v1.entries[0].requested_angle_degrees += 1.0;
            assert!(
                propose_current_cycle_pose_inner(None, &app_state, &transactions, broken).is_err()
            );
            assert_eq!(
                super::super::lock_project(&app_state)
                    .unwrap()
                    .editor
                    .revision(),
                revision
            );
            let replaced =
                propose_current_cycle_pose_inner(None, &app_state, &transactions, request())
                    .expect("theta preview");
            let cancelled =
                propose_current_cycle_pose_inner(None, &app_state, &transactions, request())
                    .expect("theta replacement preview");
            assert!(
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    replaced.transaction_token,
                )
                .is_err()
            );
            super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
                &transactions,
                cancelled.transaction_token,
            )
            .unwrap();
            assert!(
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    cancelled.transaction_token,
                )
                .is_err()
            );
            let response =
                propose_current_cycle_pose_inner(None, &app_state, &transactions, request())
                    .expect("theta retry");
            let applied =
                super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                    &app_state,
                    &GlobalFlatFoldabilityState::default(),
                    &transactions,
                    response.transaction_token,
                )
                .unwrap();
            let mut project = super::super::lock_project(&app_state).unwrap();
            project.editor.undo(applied).unwrap();
            let undone = project.editor.revision();
            project.editor.redo(undone).unwrap();
            assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
        }
    }

    #[test]
    fn eight_leaf_cycle_preview_applies_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, hinges) =
            super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern();
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let fixed = snapshot
            .faces
            .iter()
            .max_by_key(|face| {
                snapshot
                    .hinge_adjacency
                    .iter()
                    .filter(|adjacency| adjacency.first == face.id || adjacency.second == face.id)
                    .count()
            })
            .unwrap()
            .id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let response = propose_current_cycle_pose_inner(
            None,
            &app_state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
            },
        )
        .expect("eight-leaf preview");
        assert_eq!(response.closure_leaf_count, 8);
        assert_eq!(response.closure_max_depth, 3);
        assert_eq!(response.checked_hinge_count, 32);
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            response.transaction_token,
        )
        .unwrap();
        let mut project = super::super::lock_project(&app_state).unwrap();
        project.editor.undo(applied).unwrap();
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    }

    #[test]
    fn sixteen_leaf_cycle_preview_applies_and_round_trips_history() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let (pattern, paper, hinges) =
            super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern();
        let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let fixed = snapshot
            .faces
            .iter()
            .max_by_key(|face| {
                snapshot
                    .hinge_adjacency
                    .iter()
                    .filter(|adjacency| adjacency.first == face.id || adjacency.second == face.id)
                    .count()
            })
            .unwrap()
            .id;
        super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
            &mut project,
            hinges.clone(),
            fixed,
        );
        let instance = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let app_state = AppState::new(project);
        let transactions =
            super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
        let response = propose_current_cycle_pose_inner(
            None,
            &app_state,
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: four_bay_cycle_schedule(&hinges),
            },
        )
        .expect("sixteen-leaf preview");
        assert_eq!(response.closure_leaf_count, 16);
        assert_eq!(response.closure_max_depth, 4);
        assert_eq!(response.checked_hinge_count, 64);
        let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &GlobalFlatFoldabilityState::default(),
            &transactions,
            response.transaction_token,
        )
        .unwrap();
        let mut project = super::super::lock_project(&app_state).unwrap();
        project.editor.undo(applied).unwrap();
        let undone = project.editor.revision();
        project.editor.redo(undone).unwrap();
        assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    }

    #[test]
    fn current_graph_cycle_authenticates_or_fails_closed_three_times() {
        let _generation_guard = lock_stacked_fold_read_generation_test();
        let mut authenticated = 0;
        let mut rejected = Vec::new();
        for iteration in 0..3 {
            let (mut project, hinges) =
                super::super::applied_pose::tests::flat_foldable_cross_cycle_project();
            super::super::applied_pose::tests::install_flat_graph_pose_authority(
                &mut project,
                hinges.clone(),
            );
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            let app_state = AppState::new(project);
            let transaction_state =
                super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
            assert_eq!(
                propose_current_cycle_pose_inner(
                    None,
                    &app_state,
                    &transaction_state,
                    CurrentCyclePosePreviewRequestV1 {
                        progress_request_id: None,
                        expected_project_instance_id: instance,
                        expected_project_id: project_id,
                        expected_revision: revision + 1,
                        cycle_schedule_v1: physical_four_vertex_cycle_schedule(&hinges),
                    },
                )
                .unwrap_err(),
                STALE_MESSAGE
            );
            let response = propose_current_cycle_pose_inner(
                None,
                &app_state,
                &transaction_state,
                CurrentCyclePosePreviewRequestV1 {
                    progress_request_id: None,
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: revision,
                    cycle_schedule_v1: physical_four_vertex_cycle_schedule(&hinges),
                },
            );
            match response {
                Ok(mut response) => {
                    authenticated += 1;
                    assert!(response.closure_leaf_count > 0);
                    assert!(response.closure_max_depth <= 16);
                    assert_eq!(response.checked_hinge_count, response.total_hinge_count);
                    assert_eq!(response.total_hinge_count, hinges.len());
                    assert!(
                        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                            &app_state,
                            &GlobalFlatFoldabilityState::default(),
                            &transaction_state,
                            ProjectId::new(),
                        )
                        .is_err()
                    );
                    if iteration == 0 {
                        let cancelled = response.transaction_token;
                        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
                            &transaction_state,
                            cancelled,
                        )
                        .unwrap();
                        assert!(
                            super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                                &app_state,
                                &GlobalFlatFoldabilityState::default(),
                                &transaction_state,
                                cancelled,
                            )
                            .is_err()
                        );
                        response = propose_current_cycle_pose_inner(
                            None,
                            &app_state,
                            &transaction_state,
                            CurrentCyclePosePreviewRequestV1 {
                                progress_request_id: None,
                                expected_project_instance_id: instance,
                                expected_project_id: project_id,
                                expected_revision: revision,
                                cycle_schedule_v1: physical_four_vertex_cycle_schedule(&hinges),
                            },
                        )
                        .expect("replacement authenticated preview");
                        assert_ne!(response.transaction_token, cancelled);
                    }
                    assert!(!response.authorizes_project_mutation);
                    assert!(response.continuous_path_certified);
                    let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
                        &app_state,
                        &GlobalFlatFoldabilityState::default(),
                        &transaction_state,
                        response.transaction_token,
                    )
                    .expect("authenticated atomic apply");
                    let mut project = super::super::lock_project(&app_state).unwrap();
                    assert_eq!(applied, revision + 1);
                    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
                    project.editor.undo(applied).unwrap();
                    let undo_revision = project.editor.revision();
                    project.editor.redo(undo_revision).unwrap();
                }
                Err(error) => {
                    rejected.push(error.clone());
                    assert!(
                        error == CYCLE_NONCLOSING_MESSAGE
                            || error == CYCLE_PATH_UNCERTIFIED_MESSAGE,
                        "unexpected fail-closed category: {error}"
                    );
                    let project = super::super::lock_project(&app_state).unwrap();
                    assert_eq!(project.editor.revision(), revision);
                    assert!(project.editor.instruction_timeline().steps.is_empty());
                }
            }
        }
        assert_eq!(
            authenticated, 3,
            "fixed native fixture must authenticate; rejected={rejected:?}"
        );
    }

    #[test]
    fn current_cycle_preview_request_rejects_unknown_dto_fields() {
        let id = ProjectId::new();
        let value = serde_json::json!({
            "expectedProjectInstanceId": id,
            "expectedProjectId": id,
            "expectedRevision": 0,
            "cycleScheduleV1": { "version": 1, "entries": [] },
            "unexpected": true
        });
        assert!(serde_json::from_value::<CurrentCyclePosePreviewRequestV1>(value).is_err());
    }

    #[test]
    fn current_cycle_generation_replacement_and_cancel_are_monotonic() {
        let _guard = STACKED_FOLD_READ_GENERATION_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let first = begin_stacked_fold_read_generation_v1().unwrap();
        let replacement = begin_stacked_fold_read_generation_v1().unwrap();
        assert!(replacement > first);
        assert_ne!(STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire), first);
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            replacement
        );
        cancel_current_stacked_fold_read_v1().unwrap();
        assert_ne!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            replacement
        );
    }

    #[test]
    fn current_cycle_progress_id_is_strict_and_bounded() {
        assert_eq!(validate_progress_request_id_v1(None).unwrap(), None);
        assert_eq!(
            validate_progress_request_id_v1(Some("cycle:1")).unwrap(),
            Some("cycle:1")
        );
        assert!(validate_progress_request_id_v1(Some("")).is_err());
        assert!(validate_progress_request_id_v1(Some(&"x".repeat(129))).is_err());
        assert!(validate_progress_request_id_v1(Some("循環")).is_err());
    }
}
