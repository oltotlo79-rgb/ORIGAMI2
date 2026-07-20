//! Read-only desktop bridge for the first authenticated SIM-010 boundary.
//!
//! No value returned by this module authorizes project mutation. Heavy exact
//! analysis runs over detached immutable capabilities and is revalidated
//! against both live native slots before its bounded observation is returned.

use std::sync::atomic::{AtomicU64, Ordering};

use ori_collision::{
    FlatEndpointLayerOrderInputV1, StackedFoldFixedSideV1, StackedFoldLinearCandidateV1,
    StackedFoldMaterialMapLimitsV1, StackedFoldPathDiagnosticLimitsV1, StackedFoldReadBindingV1,
    StackedFoldReadLimitsV1, StackedFoldReadSupportV1, StackedFoldRotationDirectionV1,
    StaticCollisionLimits, capture_stacked_fold_read_guard_v1, diagnose_collective_hinge_path_v1,
    diagnose_scheduled_cycle_path_v1, diagnose_static_collision_geometry,
    propose_linear_stacked_fold_read_v1, reverse_map_linear_stacked_fold_material_v1,
};
use ori_core::{
    DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS, ExpectedStackedFoldCreaseV1, FaceLineageLimits,
    StackedFoldGeometryLimitsV1, StackedFoldTopologyBuildLimitsV1, analyze_global_flat_foldability,
    analyze_local_flat_foldability, prepare_stacked_fold_geometry_candidate_v1,
    prepare_stacked_fold_graph_non_flat_layer_order_v1, prepare_stacked_fold_initial_graph_pose_v1,
    prepare_stacked_fold_initial_pose_v1,
    prepare_stacked_fold_non_flat_layer_order_with_thickness_v1,
    prepare_stacked_fold_requested_pose_v1, prepare_stacked_fold_target_graph_audit_v1,
    prepare_stacked_fold_target_model_v1,
};
use ori_domain::{FaceId, ProjectId};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityOutcome,
};
use ori_kinematics::{
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, MultiHingePathCandidateLimitsV1, Point3,
    TreeKinematicsLimits, generate_linear_multi_hinge_path_candidate_v1,
};
use ori_topology::{FaceExtractionInput, TopologyIssueSeverity, analyze_faces};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use super::{
    AppState,
    global_flat_foldability::{
        GlobalFlatFoldabilityState, capture_current_layer_order_capability,
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
static STACKED_FOLD_READ_GENERATION: AtomicU64 = AtomicU64::new(0);
const STACKED_FOLD_READ_PROGRESS_EVENT_V1: &str = "stacked-fold-read-progress-v1";

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
struct LinearCandidateRequestV1 {
    version: u32,
    entries: Vec<LinearCandidateEntryRequestV1>,
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

fn validate_certified_path_graph_v1(
    request: &CertifiedPathGraphRequestV1,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<Vec<ori_kinematics::CanonicalHingeAngles>, &'static str> {
    if request.version != 1
        || request.states.is_empty()
        || request.states.len() > ori_collision::MAX_CERTIFIED_PATH_GRAPH_STATES_V1
        || request.transitions.len() > ori_collision::MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CycleScheduleRequestV1 {
    version: u32,
    entries: Vec<CycleScheduleEntryRequestV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
        if audited_target.requires_closure_certificate() {
            let initial = prepare_stacked_fold_initial_graph_pose_v1(
                audited_target,
                pose_capability.model(),
                pose_capability.pose(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let path_variant_count = usize::from(request.cycle_schedule_v1.is_some())
                + usize::from(request.linear_candidate_v1.is_some())
                + usize::from(request.certified_path_graph_v1.is_some());
            if path_variant_count != 1 || request.cycle_schedule_v1.is_some() {
                return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
            }
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
                    let progress_app = app.clone();
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
                            if let Some(request_id) = progress_request_id.as_ref() {
                                let _ = progress_app.emit(
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
                                );
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
                } else {
                    let linear = request
                        .linear_candidate_v1
                        .as_ref()
                        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
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
            let generated = generate_linear_multi_hinge_path_candidate_v1(
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
            })?;
            let cycle_limits = CycleScheduleLimitsV1::default();
            let interval_closure = initial
                .target()
                .hinge_geometry()
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
            let continuous = diagnose_scheduled_cycle_path_v1(
                initial.target().hinge_geometry(),
                initial.target().audit(),
                initial.pose().fixed_face(),
                &generated,
                &interval_closure,
                StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
            );
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
                        super::stacked_fold_transaction::PendingStackedFoldLayerProof::CertifiedFlat(
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
                        super::stacked_fold_transaction::PendingStackedFoldLayerProof::NonFlat(
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
                timeline_step_count: certified_path_certificate
                    .as_ref()
                    .map_or(1, |path| path.edges().len()),
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
mod tests {
    use super::*;

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
        assert!(serde_json::from_value::<StackedFoldReadRequest>(request()).is_ok());
        let mut unknown = request();
        unknown["cycleScheduleV1"]["entries"][0]["authority"] = serde_json::json!(true);
        assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());
        let mut overflow = request();
        overflow["cycleScheduleV1"]["entries"][0]["uDomain"][0]["denominator"] =
            serde_json::json!(-1);
        assert!(serde_json::from_value::<StackedFoldReadRequest>(overflow).is_err());
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
            entries: vec![LinearCandidateEntryRequestV1 {
                edge,
                initial_angle_degrees: f64::from_bits(20.0f64.to_bits() + 1),
                requested_angle_degrees: 40.0,
            }],
        };
        assert!(validate_linear_candidate_angles_v1(&mismatch, &live).is_err());
        let wrong_version = LinearCandidateRequestV1 {
            version: 2,
            entries: request.entries,
        };
        assert!(validate_linear_candidate_angles_v1(&wrong_version, &live).is_err());
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
    }

    #[test]
    fn stacked_fold_read_cancel_advances_the_process_wide_generation() {
        let before = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
        cancel_current_stacked_fold_read_v1().expect("generation has capacity");
        assert_eq!(
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
            before + 1
        );
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
}
