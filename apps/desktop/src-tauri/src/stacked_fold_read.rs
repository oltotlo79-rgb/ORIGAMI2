//! Read-only desktop bridge for the first authenticated SIM-010 boundary.
//!
//! Read responses never authorize mutation. The dyadic one-shot apply boundary
//! retains private native capabilities and revalidates them under both live
//! authority guards before one atomic Editor command is committed.

#[path = "stacked_fold_blockwise_cycle.rs"]
mod stacked_fold_blockwise_cycle;
pub(super) const fn bounded_multi_block_current_cycle_arity_supported_v1(
    block_count: usize,
) -> bool {
    stacked_fold_blockwise_cycle::bounded_multi_block_current_cycle_arity_supported_v1(block_count)
}
#[cfg(test)]
#[path = "stacked_fold_blockwise_cycle_tests.rs"]
mod stacked_fold_blockwise_cycle_tests;
#[path = "stacked_fold_cycle_pose_wire.rs"]
mod stacked_fold_cycle_pose_wire;
#[path = "stacked_fold_cycle_schedule.rs"]
mod stacked_fold_cycle_schedule;
#[path = "stacked_fold_dyadic_graph_wire.rs"]
mod stacked_fold_dyadic_graph_wire;
#[path = "stacked_fold_dyadic_preview.rs"]
pub(super) mod stacked_fold_dyadic_preview;
#[path = "stacked_fold_dyadic_scope.rs"]
mod stacked_fold_dyadic_scope;
#[path = "stacked_fold_non_flat_continuation.rs"]
pub(super) mod stacked_fold_non_flat_continuation;
#[path = "stacked_fold_read_wire.rs"]
mod stacked_fold_read_wire;

use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ori_collision::{
    CanonicalPositiveThicknessCyclePathControlErrorV1, CooperativeOperationControlV1,
    FlatEndpointLayerOrderInputV1, GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
    ProofCacheOperationControlV1, ProofCacheRuntimeBindingV1, ProofCacheRuntimeErrorV1,
    StackedFoldFixedSideV1, StackedFoldLinearCandidateV1, StackedFoldMaterialMapLimitsV1,
    StackedFoldPathDiagnosticLimitsV1, StackedFoldReadBindingV1, StackedFoldReadLimitsV1,
    StaticCollisionLimits, anchor_flat_endpoint_layer_order_v1, capture_stacked_fold_read_guard_v1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
    certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1,
    certify_general_multi_face_cell_transport_v1, diagnose_collective_hinge_path_v1,
    diagnose_collective_hinge_path_with_pair_cache_v1, diagnose_scheduled_cycle_path_v1,
    diagnose_scheduled_positive_thickness_cycle_path_v1, diagnose_static_collision_geometry,
    diagnose_static_collision_geometry_with_flat_layer_order_v1,
    propose_linear_stacked_fold_read_v1, reverse_map_linear_stacked_fold_material_v1,
    supports_scheduled_positive_thickness_path_v1,
};
use ori_core::{
    DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS, ExpectedStackedFoldCreaseV1, FaceLineageLimits,
    StackedFoldGeometryLimitsV1, StackedFoldTopologyBuildLimitsV1, analyze_global_flat_foldability,
    analyze_local_flat_foldability,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1,
    prepare_stacked_fold_geometry_candidate_v1, prepare_stacked_fold_graph_non_flat_layer_order_v1,
    prepare_stacked_fold_initial_graph_pose_v1, prepare_stacked_fold_initial_layer_order_v1,
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
    CycleBasisLimitsV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    MultiHingePathCandidateLimitsV1, Point3, TreeKinematicsLimits,
    generate_linear_multi_hinge_path_candidate_v1,
};
use ori_topology::{FaceExtractionInput, TopologyIssueSeverity, analyze_faces};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use self::stacked_fold_blockwise_cycle::prepare_blockwise_current_cycle_fallback_v1;
#[cfg(test)]
use self::stacked_fold_cycle_pose_wire::{
    CertifiedPathGraphAngleRequestV1, CertifiedPathGraphRequestV1,
    CertifiedPathGraphStateRequestV1, CertifiedPathGraphTransitionRequestV1,
    LinearCandidateEntryRequestV1, LinearCandidateRequestV1,
};
use self::stacked_fold_cycle_pose_wire::{
    CurrentCyclePosePreviewRequestV1, CurrentCyclePosePreviewResponseV1, LayerOrderPairDtoV1,
    validate_certified_path_graph_v1, validate_exact_dyadic_candidate_path_v1,
    validate_linear_candidate_angles_v1, validate_progress_request_id_v1,
};
pub(super) use self::stacked_fold_cycle_schedule::production_cycle_schedule_limits_v1;
#[cfg(test)]
use self::stacked_fold_cycle_schedule::{
    CycleScheduleEntryRequestV1, RationalCoefficientRequestV1, advance_collective_schedule,
    dense_grid_schedule, dense_grid_schedule_ratio, four_bay_cycle_schedule,
    physical_four_vertex_cycle_schedule, theta_cycle_schedule,
};
use self::stacked_fold_cycle_schedule::{
    CycleScheduleRequestV1, generate_even_opposite_pair_schedule_v1,
    prepare_requested_cycle_schedule_v1,
};
use self::stacked_fold_dyadic_graph_wire::{
    DyadicPoseGraphReadRequestV1, DyadicPoseGraphReadResponseV1, dyadic_graph_response,
    unsupported_dyadic_graph_response_v1,
};
#[cfg(test)]
use self::stacked_fold_dyadic_preview::{
    ApplyDyadicPathPreviewRequestV1, DyadicPathPreviewRequestV1, DyadicPathPreviewState,
    apply_dyadic_pose_path_preview_inner_v1, cancel_dyadic_pose_path_preview_inner_v1,
    mint_dyadic_pose_path_preview_inner_v1,
};
use self::stacked_fold_dyadic_scope::strict_dyadic_geometry_is_in_scope_v1;
#[cfg(test)]
use self::stacked_fold_read_wire::StackedFoldTransactionFailureClassDto;
use self::stacked_fold_read_wire::{
    CertifiedPathGraphEdgeDto, CertifiedPathGraphPreviewDto, CurrentCyclePoseProgressDtoV1,
    DyadicPoseGraphAngleDtoV1, STACKED_FOLD_APPLY_CONTRACT_VERSION_V1, StackedFoldApplyModeDtoV1,
    StackedFoldContinuousPathDto, StackedFoldEndpointCollisionDto,
    StackedFoldFlatEndpointLayerOrderDto, StackedFoldMaterialSegmentDto, StackedFoldReadBindingDto,
    StackedFoldReadCellDto, StackedFoldReadProgressDtoV1, StackedFoldReadWorkDto,
    StackedFoldTopologyProofDto, StackedFoldTransactionProposalDto,
    requires_graph_schedule_boundary_v1, transaction_failure_classes,
    validate_request_resource_shape_v1,
};
pub(super) use self::stacked_fold_read_wire::{
    FixedSideRequest, RotationDirectionRequest, StackedFoldReadRequest, StackedFoldReadResponse,
};
#[cfg(test)]
use super::stacked_fold_even_cycle_candidates::{
    EvenCycleCandidatesRequestV1, read_even_cycle_candidates_inner_v1,
};
use super::stacked_fold_live_hinge_registry::{LiveGraphHingeAngleDto, live_hinge_registry};
#[cfg(test)]
use super::stacked_fold_live_hinge_registry::{
    LiveHingeRegistryRequestV1, read_live_hinge_registry_inner,
};
use super::{
    AppState, ProjectState,
    applied_pose::CurrentAppliedPoseCapability,
    global_flat_foldability::{
        CurrentLayerOrderCapability, GlobalFlatFoldabilityState,
        capture_current_layer_order_capability, revalidate_current_layer_order_capability,
    },
    lock_project, stacked_fold_transaction,
};

pub(super) const UNAVAILABLE_MESSAGE: &str =
    "The current pose and certified layer order cannot prepare a stacked-fold proposal.";
pub(super) const INVALID_REQUEST_MESSAGE: &str = "The stacked-fold line request is invalid.";
pub(super) const ANALYSIS_FAILED_MESSAGE: &str =
    "The stacked-fold proposal is unsupported or could not be certified.";
const CYCLE_NONCLOSING_MESSAGE: &str = "stacked_fold_cycle_nonclosing";
const CYCLE_PATH_UNCERTIFIED_MESSAGE: &str = "stacked_fold_cycle_path_uncertified";
const CYCLE_PATH_UNSUPPORTED_MESSAGE: &str = "stacked_fold_cycle_path_unsupported";
const CYCLE_PATH_RESOURCE_MESSAGE: &str = "stacked_fold_cycle_path_resource_limit";
const CYCLE_PATH_NO_CERTIFIED_PATH_MESSAGE: &str = "stacked_fold_cycle_path_no_certified_path";
const CYCLE_PATH_DEADLINE_MESSAGE: &str = "stacked_fold_cycle_path_deadline_exceeded";
pub(super) const BUSY_MESSAGE: &str = "Another native pose analysis is already running.";
pub(super) const STALE_MESSAGE: &str =
    "The project, current pose, or certified layer order changed during analysis.";
const CANCELLED_MESSAGE: &str = "stacked_fold_cycle_path_cancelled";
const FLAT_ENDPOINT_COLLISION_THICKNESS_MM_V1: f64 = 0.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EndpointCollisionPlanV1 {
    DeferToFlatLayerOrder,
    CertifiedPositiveThickness,
    StaticGeometry,
}

fn endpoint_collision_plan_v1(
    requested_angle_degrees: f64,
    positive_thickness_certificate: bool,
) -> EndpointCollisionPlanV1 {
    if requested_angle_degrees.to_bits() == 180.0_f64.to_bits() {
        EndpointCollisionPlanV1::DeferToFlatLayerOrder
    } else if positive_thickness_certificate {
        EndpointCollisionPlanV1::CertifiedPositiveThickness
    } else {
        EndpointCollisionPlanV1::StaticGeometry
    }
}

#[must_use]
fn endpoint_allows_speculative_apply_v1(endpoint: &StackedFoldEndpointCollisionDto) -> bool {
    !endpoint.has_blocking_hold
        && endpoint.penetrating_pair_count == 0
        && endpoint.indeterminate_pair_count == 0
}

fn admit_initial_layer_order_endpoint_v1(
    mut endpoint: StackedFoldEndpointCollisionDto,
    layer_admitted_speculative_path: bool,
) -> Option<StackedFoldEndpointCollisionDto> {
    let accounted_pair_count = endpoint
        .separated_pair_count
        .checked_add(endpoint.touching_pair_count)?
        .checked_add(endpoint.allowed_pair_count)?
        .checked_add(endpoint.penetrating_pair_count)?
        .checked_add(endpoint.indeterminate_pair_count)?;
    if accounted_pair_count != endpoint.expected_pair_count
        || endpoint.has_blocking_hold
            != (endpoint.penetrating_pair_count > 0 || endpoint.indeterminate_pair_count > 0)
        || endpoint.penetrating_pair_count > 0
    {
        return None;
    }
    if endpoint.indeterminate_pair_count == 0 {
        return Some(endpoint);
    }
    if !layer_admitted_speculative_path {
        return None;
    }
    endpoint.allowed_pair_count = endpoint
        .allowed_pair_count
        .checked_add(endpoint.indeterminate_pair_count)?;
    endpoint.indeterminate_pair_count = 0;
    endpoint.has_blocking_hold = false;
    Some(endpoint)
}

fn is_positive_thickness_continuous_certificate_model_id_v2(model_id: Option<&str>) -> bool {
    matches!(
        model_id,
        Some(
            ori_collision::STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2
                | ori_collision::STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2
        )
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduledCycleThicknessAuthorityV1 {
    ZeroThickness,
    PositiveThickness { thickness_bits: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduledCycleThicknessDiagnosticErrorV1 {
    InvalidThickness,
    PositiveThicknessUnsupported,
    Uncertified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ControlledCycleAuthorityReadErrorV1 {
    Cancelled,
    DeadlineExceeded,
}

fn controlled_cycle_authority_read_v1<T>(
    generation: u64,
    control: &CooperativeOperationControlV1<'_>,
    issue: impl FnOnce(
        &CooperativeOperationControlV1<'_>,
    ) -> Result<Option<T>, CanonicalPositiveThicknessCyclePathControlErrorV1>,
) -> Result<Option<T>, ControlledCycleAuthorityReadErrorV1> {
    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != generation {
        return Err(ControlledCycleAuthorityReadErrorV1::Cancelled);
    }
    let authority = issue(control).map_err(|error| match error {
        CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled => {
            ControlledCycleAuthorityReadErrorV1::Cancelled
        }
        CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded => {
            ControlledCycleAuthorityReadErrorV1::DeadlineExceeded
        }
    })?;
    control.checkpoint().map_err(|stop| match stop {
        ori_collision::CooperativeOperationStopV1::Cancelled => {
            ControlledCycleAuthorityReadErrorV1::Cancelled
        }
        ori_collision::CooperativeOperationStopV1::DeadlineExceeded => {
            ControlledCycleAuthorityReadErrorV1::DeadlineExceeded
        }
    })?;
    (STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == generation)
        .then_some(authority)
        .ok_or(ControlledCycleAuthorityReadErrorV1::Cancelled)
}

const fn controlled_cycle_authority_read_message_v1(
    error: ControlledCycleAuthorityReadErrorV1,
) -> &'static str {
    match error {
        ControlledCycleAuthorityReadErrorV1::Cancelled => CANCELLED_MESSAGE,
        ControlledCycleAuthorityReadErrorV1::DeadlineExceeded => CYCLE_PATH_DEADLINE_MESSAGE,
    }
}

fn normalize_blockwise_current_cycle_fallback_error_v1(error: String) -> String {
    if error == CANCELLED_MESSAGE
        || error == CYCLE_PATH_DEADLINE_MESSAGE
        || error == CYCLE_PATH_RESOURCE_MESSAGE
    {
        error
    } else {
        CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned()
    }
}

fn preflight_certified_path_graph_thickness_v1(
    paper_thickness_mm: f64,
) -> Result<(), ScheduledCycleThicknessDiagnosticErrorV1> {
    if paper_thickness_mm == 0.0 {
        // Preserve the established +0/-0 zero-thickness graph issuer.
        Ok(())
    } else if !paper_thickness_mm.is_finite() || paper_thickness_mm < 0.0 {
        Err(ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness)
    } else {
        // CertifiedPathTransitionEvidenceV1 currently carries only the
        // zero-thickness cycle collision issuer. Until every graph edge and
        // its pending revalidation bind the exact positive thickness, do not
        // let a detached positive direct path upgrade those edge certificates.
        Err(ScheduledCycleThicknessDiagnosticErrorV1::PositiveThicknessUnsupported)
    }
}

fn scheduled_cycle_diagnostic_matches_thickness_authority_v1(
    authority: ScheduledCycleThicknessAuthorityV1,
    model_id: Option<&str>,
    positive_thickness_bits: Option<u64>,
) -> bool {
    match authority {
        ScheduledCycleThicknessAuthorityV1::ZeroThickness => {
            model_id
                == Some(
                    ori_collision::STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
                )
                && positive_thickness_bits.is_none()
        }
        ScheduledCycleThicknessAuthorityV1::PositiveThickness { thickness_bits } => {
            model_id
                == Some(
                    ori_collision::STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
                )
                && positive_thickness_bits == Some(thickness_bits)
        }
    }
}

fn diagnose_scheduled_cycle_path_for_thickness_v1<T>(
    paper_thickness_mm: f64,
    supports_positive_thickness: impl FnOnce() -> bool,
    diagnose_zero_thickness: impl FnOnce() -> T,
    diagnose_positive_thickness: impl FnOnce(f64) -> T,
    certificate_metadata: impl FnOnce(&T) -> (Option<&'static str>, Option<u64>),
) -> Result<T, ScheduledCycleThicknessDiagnosticErrorV1> {
    let authority = if paper_thickness_mm == 0.0 {
        // IEEE equality intentionally preserves the existing +0/-0 behavior.
        ScheduledCycleThicknessAuthorityV1::ZeroThickness
    } else if !paper_thickness_mm.is_finite() || paper_thickness_mm < 0.0 {
        return Err(ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness);
    } else {
        if !supports_positive_thickness() {
            // A positive-thickness request must never fall through to the
            // zero-thickness oracle merely because the positive theorem does
            // not cover this graph/schedule.
            return Err(ScheduledCycleThicknessDiagnosticErrorV1::PositiveThicknessUnsupported);
        }
        ScheduledCycleThicknessAuthorityV1::PositiveThickness {
            thickness_bits: paper_thickness_mm.to_bits(),
        }
    };
    let diagnostic = match authority {
        ScheduledCycleThicknessAuthorityV1::ZeroThickness => diagnose_zero_thickness(),
        ScheduledCycleThicknessAuthorityV1::PositiveThickness { .. } => {
            diagnose_positive_thickness(paper_thickness_mm)
        }
    };
    let (model_id, positive_thickness_bits) = certificate_metadata(&diagnostic);
    if !scheduled_cycle_diagnostic_matches_thickness_authority_v1(
        authority,
        model_id,
        positive_thickness_bits,
    ) {
        return Err(ScheduledCycleThicknessDiagnosticErrorV1::Uncertified);
    }
    Ok(diagnostic)
}

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
const MAX_PRE_CANCELLED_STACKED_FOLD_READ_REQUESTS_V1: usize = 256;
const MAX_STACKED_FOLD_RENDERED_CELLS_V1: usize = 2_048;
const MAX_STACKED_FOLD_RENDERED_CELL_LAYERS_V1: usize = 2_048;
const MAX_STACKED_FOLD_RENDERED_BOUNDARY_POINTS_V1: usize = 4_096;
const MAX_STACKED_FOLD_RENDER_VERTEX_INSTANCES_V1: usize = 32_768;

fn validate_stacked_fold_layer_view_cells_v1(
    cells: &[StackedFoldReadCellDto],
) -> Result<(), String> {
    if cells.is_empty() || cells.len() > MAX_STACKED_FOLD_RENDERED_CELLS_V1 {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let mut total_render_vertex_instances = 0_usize;
    let mut cell_keys = HashSet::with_capacity(cells.len());
    for cell in cells {
        if !cell_keys.insert(cell.cell_key_sha256.as_str())
            || cell.bottom_to_top_faces.is_empty()
            || cell.bottom_to_top_faces.len() > MAX_STACKED_FOLD_RENDERED_CELL_LAYERS_V1
            || !(3..=MAX_STACKED_FOLD_RENDERED_BOUNDARY_POINTS_V1)
                .contains(&cell.boundary_world.len())
            || !cell
                .boundary_world
                .iter()
                .flatten()
                .all(|coordinate| coordinate.is_finite())
        {
            return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
        }
        let unique_faces = cell
            .bottom_to_top_faces
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if unique_faces.len() != cell.bottom_to_top_faces.len() {
            return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
        }
        let render_vertex_instances = cell
            .bottom_to_top_faces
            .len()
            .checked_mul(cell.boundary_world.len())
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        total_render_vertex_instances = total_render_vertex_instances
            .checked_add(render_vertex_instances)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        if total_render_vertex_instances > MAX_STACKED_FOLD_RENDER_VERTEX_INSTANCES_V1 {
            return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
        }
    }
    Ok(())
}
static STACKED_FOLD_READ_GENERATION: AtomicU64 = AtomicU64::new(0);
struct StackedFoldReadPublicationStateV1 {
    active_request_id: Option<String>,
    pre_cancelled_request_ids: VecDeque<String>,
}

static STACKED_FOLD_READ_PUBLICATION_GATE_V1: Mutex<StackedFoldReadPublicationStateV1> =
    Mutex::new(StackedFoldReadPublicationStateV1 {
        active_request_id: None,
        pre_cancelled_request_ids: VecDeque::new(),
    });
#[cfg(test)]
static STACKED_FOLD_PREPUBLICATION_ACTION_V1: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);
const STACKED_FOLD_READ_PROGRESS_EVENT_V1: &str = "stacked-fold-read-progress-v1";
const CURRENT_CYCLE_POSE_PROGRESS_EVENT_V1: &str = "current-cycle-pose-progress-v1";

#[tauri::command]
pub(super) fn cancel_current_stacked_fold_read_v1(
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    let result = cancel_current_stacked_fold_read_inner_v1();
    if result.is_ok() {
        app_state.1.notify_waiters();
    }
    result
}

fn cancel_current_stacked_fold_read_inner_v1() -> Result<(), String> {
    let publication = STACKED_FOLD_READ_PUBLICATION_GATE_V1
        .lock()
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    if publication.active_request_id.is_some() {
        return Ok(());
    }
    advance_stacked_fold_read_generation_v1()
}

#[tauri::command]
pub(super) fn cancel_current_stacked_fold_read_request_v1(
    app_state: State<'_, AppState>,
    request_id: String,
) -> Result<(), String> {
    let result = cancel_current_stacked_fold_read_request_inner_v1(request_id);
    if result.is_ok() {
        app_state.1.notify_waiters();
    }
    result
}

fn cancel_current_stacked_fold_read_request_inner_v1(request_id: String) -> Result<(), String> {
    validate_progress_request_id_v1(Some(&request_id))?;
    let mut publication = STACKED_FOLD_READ_PUBLICATION_GATE_V1
        .lock()
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    if publication.active_request_id.as_deref() == Some(request_id.as_str()) {
        advance_stacked_fold_read_generation_v1()?;
        publication.active_request_id = None;
    }
    remember_pre_cancelled_request_id_v1(&mut publication, request_id);
    Ok(())
}

fn remember_pre_cancelled_request_id_v1(
    publication: &mut StackedFoldReadPublicationStateV1,
    request_id: String,
) {
    if publication
        .pre_cancelled_request_ids
        .iter()
        .any(|cancelled| cancelled == &request_id)
    {
        return;
    }
    if publication.pre_cancelled_request_ids.len()
        == MAX_PRE_CANCELLED_STACKED_FOLD_READ_REQUESTS_V1
    {
        publication.pre_cancelled_request_ids.pop_front();
    }
    publication.pre_cancelled_request_ids.push_back(request_id);
}

fn advance_stacked_fold_read_generation_v1() -> Result<(), String> {
    STACKED_FOLD_READ_GENERATION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
            generation.checked_add(1)
        })
        .map(|_| ())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
}

fn with_current_cycle_publication_v1<R>(
    generation: u64,
    publish: impl FnOnce() -> Result<R, String>,
) -> Result<R, String> {
    let _publication_guard = STACKED_FOLD_READ_PUBLICATION_GATE_V1
        .lock()
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != generation {
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    publish()
}

fn stacked_fold_pair_cache_control_v1(
    expected_generation: u64,
    absolute_deadline: Instant,
) -> ProofCacheOperationControlV1<'static> {
    ProofCacheOperationControlV1::new_with_generation(
        None,
        &STACKED_FOLD_READ_GENERATION,
        expected_generation,
        absolute_deadline,
    )
}

#[allow(clippy::too_many_arguments)]
fn positive_pair_proof_cache_binding_v1(
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    geometry_fingerprint: [u8; 32],
    pose_generation: u64,
    paper_thickness_mm: f64,
) -> Result<Option<ProofCacheRuntimeBindingV1>, ProofCacheRuntimeErrorV1> {
    if !paper_thickness_mm.is_finite() || paper_thickness_mm <= 0.0 {
        // Model 4 requires strictly positive finite thickness. Preserve the
        // established uncached diagnostic (including its original failure
        // meaning) for signed zero and every other non-positive/non-finite
        // input instead of letting cache binding validation replace it.
        return Ok(None);
    }
    ProofCacheRuntimeBindingV1::new(
        project_instance_id,
        project_id,
        revision,
        geometry_fingerprint,
        pose_generation,
        paper_thickness_mm,
    )
    .map(Some)
}

const fn map_cached_tree_path_error_v1(
    error: ori_collision::StackedFoldPathDiagnosticErrorV1,
) -> &'static str {
    match error {
        ori_collision::StackedFoldPathDiagnosticErrorV1::Cancelled => CANCELLED_MESSAGE,
        _ => ANALYSIS_FAILED_MESSAGE,
    }
}

/// Native-only holder for the future regular-quad petal transaction.
///
/// This deliberately has no command/state registration yet.  Keeping the
/// authority private prevents a DTO from becoming mutation authority before
/// the dedicated issuer and atomic commit path are complete.
#[allow(dead_code)]
struct RegularQuadPetalPreviewRecordV1 {
    token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    target_binding: [u8; 32],
    path_binding: String,
    authority: DyadicPathNativeAuthorityV1,
}

#[derive(Default)]
#[allow(dead_code)]
struct RegularQuadPetalPrivatePreviewStateV1(Mutex<Option<RegularQuadPetalPreviewRecordV1>>);

#[cfg(test)]
pub(super) struct RegularQuadPetalCertificatePreviewStateV1(
    Mutex<
        Option<(
            ProjectId,
            [u8; 32],
            ori_collision::CertifiedPoseGraphPathCertificateV1,
        )>,
    >,
);

#[cfg(test)]
impl RegularQuadPetalCertificatePreviewStateV1 {
    pub(super) fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub(super) fn mint_once_v1(
        &self,
        token: ProjectId,
        binding: [u8; 32],
        parent: ori_collision::CertifiedPoseGraphPathCertificateV1,
    ) -> Result<(), String> {
        if parent.edges().len() != 3 || parent.binding_fingerprint_v1() != binding {
            return Err("invalid petal parent".to_owned());
        }
        let mut slot = self.0.lock().map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if slot.is_some() {
            return Err("petal preview occupied".to_owned());
        }
        *slot = Some((token, binding, parent));
        Ok(())
    }

    pub(super) fn consume_v1(
        &self,
        token: ProjectId,
        binding: [u8; 32],
    ) -> Result<ori_collision::CertifiedPoseGraphPathCertificateV1, String> {
        let mut slot = self.0.lock().map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if !slot
            .as_ref()
            .is_some_and(|(stored_token, stored_binding, parent)| {
                *stored_token == token
                    && *stored_binding == binding
                    && parent.binding_fingerprint_v1() == binding
            })
        {
            return Err("petal preview mismatch".to_owned());
        }
        Ok(slot.take().expect("validated occupied petal slot").2)
    }
}

#[allow(dead_code)]
impl RegularQuadPetalPrivatePreviewStateV1 {
    fn mint_once_v1(&self, record: RegularQuadPetalPreviewRecordV1) -> Result<(), String> {
        let mut slot = self.0.lock().map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if slot.is_some() {
            return Err("a regular-quad petal preview is already active".to_owned());
        }
        *slot = Some(record);
        Ok(())
    }

    fn consume_for_apply_v1(
        &self,
        token: ProjectId,
        project_instance_id: ProjectId,
        project_id: ProjectId,
        revision: u64,
        target_binding: [u8; 32],
        path_binding: &str,
    ) -> Result<RegularQuadPetalPreviewRecordV1, String> {
        let mut slot = self.0.lock().map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if !slot.as_ref().is_some_and(|record| {
            record.revalidates_for_apply_v1(
                token,
                project_instance_id,
                project_id,
                revision,
                target_binding,
                path_binding,
            )
        }) {
            return Err("regular-quad petal preview revalidation failed".to_owned());
        }
        slot.take()
            .ok_or_else(|| "regular-quad petal preview was consumed".to_owned())
    }
}

/// Captures both live authorities while the same project instance is locked,
/// derives the fixed face's canonical authored M/V hinges, and only publishes
/// the fully certified record after every stage has succeeded.
#[allow(dead_code)]
fn capture_and_mint_regular_quad_petal_preview_v1(
    project: &super::ProjectState,
    foldability_state: &GlobalFlatFoldabilityState,
    previews: &RegularQuadPetalPrivatePreviewStateV1,
) -> Result<(ProjectId, [u8; 32], String), String> {
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(project)
        .map_err(|_| "regular-quad petal pose authority is unavailable".to_owned())?
        .ok_or_else(|| "regular-quad petal pose authority is unavailable".to_owned())?;
    let layer_capability = capture_current_layer_order_capability(foldability_state, project)
        .map_err(|_| "regular-quad petal layer authority is unavailable".to_owned())?
        .ok_or_else(|| "regular-quad petal layer authority is unavailable".to_owned())?;
    let fixed_face = pose_capability
        .graph()
        .map(|(_, _, pose)| pose.fixed_face())
        .ok_or_else(|| "regular-quad petal requires graph pose authority".to_owned())?;
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology
        .simulation_snapshot()
        .ok_or_else(|| "regular-quad petal topology is unavailable".to_owned())?;
    let face = snapshot
        .faces
        .iter()
        .find(|face| face.id == fixed_face)
        .ok_or_else(|| "regular-quad petal fixed face is unavailable".to_owned())?;
    let pattern = project.editor.pattern();
    let mut hinges = face
        .outer
        .half_edges
        .iter()
        .filter_map(|half| {
            let edge = pattern.edges.iter().find(|edge| edge.id == half.edge)?;
            let assignment = match edge.kind {
                ori_domain::EdgeKind::Mountain => ori_topology::FoldAssignment::Mountain,
                ori_domain::EdgeKind::Valley => ori_topology::FoldAssignment::Valley,
                _ => return None,
            };
            Some((edge.id, assignment))
        })
        .collect::<Vec<_>>();
    hinges.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    let hinges: [(ori_domain::EdgeId, ori_topology::FoldAssignment); 3] = hinges
        .try_into()
        .map_err(|_| "regular-quad petal requires exactly three authored hinges".to_owned())?;
    let token = ProjectId::new();
    let record = issue_regular_quad_petal_preview_record_v1(
        project,
        token,
        pose_capability,
        layer_capability,
        hinges,
    )?;
    let target = record.target_binding;
    let path = record.path_binding.clone();
    previews.mint_once_v1(record)?;
    Ok((token, target, path))
}

#[allow(dead_code)]
impl RegularQuadPetalPreviewRecordV1 {
    fn issue_v1(
        project: &super::ProjectState,
        hinges: &[ori_domain::EdgeId],
        token: ProjectId,
        revision: u64,
        target_binding: [u8; 32],
        path_binding: String,
        authority: DyadicPathNativeAuthorityV1,
    ) -> Result<Self, String> {
        if project.editor.revision() != revision
            || !super::stacked_fold_transaction::regular_quad_petal_face_v1(project, hinges)
            || !authority.revalidates_exact_three_graph_segments_v1(target_binding, &path_binding)
        {
            return Err("regular-quad petal authority is unavailable".to_owned());
        }
        Ok(Self {
            token,
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision,
            target_binding,
            path_binding,
            authority,
        })
    }

    #[cfg(test)]
    fn compile_timeline_v1(
        &self,
        technique_file: &ori_instructions::FoldTechniqueFileV1,
        technique_id: &str,
        source_model_fingerprint: &str,
        fixed_face: ori_domain::FaceId,
        source_hinge_angles: &[ori_domain::InstructionHingeAngle],
        ordered_edges: &[ori_domain::EdgeId; 3],
        ordered_target_angles_microdegrees: &[i64; 3],
    ) -> Result<ori_domain::InstructionTimeline, String> {
        let certificates = self
            .authority
            .regular_quad_petal_segment_certificates_v1(self.target_binding, &self.path_binding)
            .ok_or_else(|| "regular-quad petal segment authority is unavailable".to_owned())?;
        ori_instructions::compile_certified_regular_quad_petal_fold_timeline_v1(
            ori_instructions::RegularQuadPetalFoldMotionRequestV1 {
                technique_file,
                technique_id,
                source_model_fingerprint,
                fixed_face,
                source_hinge_angles,
                ordered_edges,
                ordered_target_angles_microdegrees,
                ordered_path_certificates: &certificates,
            },
        )
        .map_err(|error| error.to_string())
    }

    /// The apply boundary rechecks both the immutable preview envelope and all
    /// issuer-bound per-segment proofs.  A caller-provided binding can never
    /// substitute for the native certificate retained in this record.
    fn revalidates_for_apply_v1(
        &self,
        token: ProjectId,
        project_instance_id: ProjectId,
        project_id: ProjectId,
        revision: u64,
        target_binding: [u8; 32],
        path_binding: &str,
    ) -> bool {
        self.token == token
            && self.project_instance_id == project_instance_id
            && self.project_id == project_id
            && self.revision == revision
            && self.target_binding == target_binding
            && self.path_binding == path_binding
            && self
                .authority
                .revalidates_exact_three_graph_segments_v1(target_binding, path_binding)
    }
}

#[allow(dead_code)]
fn issue_regular_quad_petal_preview_record_v1(
    project: &super::ProjectState,
    token: ProjectId,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
    hinges: [(ori_domain::EdgeId, ori_topology::FoldAssignment); 3],
) -> Result<RegularQuadPetalPreviewRecordV1, String> {
    let hinge_ids = hinges.map(|(edge, _)| edge);
    if !super::stacked_fold_transaction::regular_quad_petal_face_v1(project, &hinge_ids) {
        return Err("regular-quad petal face is unavailable".to_owned());
    }
    let paper_thickness_mm = project.editor.paper().thickness_mm;
    let (path, edges, target_angles, transport) = {
        let (geometry, audit, pose) = pose_capability
            .graph()
            .ok_or_else(|| "regular-quad petal requires graph pose authority".to_owned())?;
        let fixed_face = pose.fixed_face();
        let issued = ori_collision::issue_regular_quad_petal_chained_authority_v1(
            geometry,
            audit,
            layer_capability.snapshot(),
            fixed_face,
            hinges.map(|(edge, assignment)| {
                (edge, assignment == ori_topology::FoldAssignment::Mountain)
            }),
            paper_thickness_mm,
            1.0e-9,
            production_cycle_schedule_limits_v1(),
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_000_000,
                schedule_limits: production_cycle_schedule_limits_v1(),
            },
        )
        .ok_or_else(|| "regular-quad petal candidates are uncertified".to_owned())?;
        let (_, schedules, closures, positives, transport) = issued.into_parts();
        let mut path_segments = Vec::with_capacity(3);
        let mut native_edges = Vec::with_capacity(3);
        for (layer_index, ((schedule, closure), positive)) in schedules
            .into_iter()
            .zip(closures)
            .zip(positives)
            .enumerate()
        {
            let source_angles = schedule
                .evaluate(0.0)
                .ok_or_else(|| "invalid petal source".to_owned())?;
            let target = schedule
                .evaluate(1.0)
                .ok_or_else(|| "invalid petal target".to_owned())?;
            let source = pose_state_fingerprint_v1(&source_angles);
            let target_fingerprint = pose_state_fingerprint_v1(&target);
            let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
                schedule.clone(),
                &source_angles,
                &target,
            )
            .map_err(|_| "invalid petal schedule".to_owned())?;
            let evidence = ori_collision::certify_scheduled_cycle_transition_v1(
                geometry, audit, fixed_face, &candidate, &closure, 1,
            )
            .ok_or_else(|| "petal transition is uncertified".to_owned())?;
            let segment = ori_collision::search_certified_pose_graph_v1(
                &[source, target_fingerprint],
                &[ori_collision::CertifiedPathTransitionCandidateV1 {
                    source,
                    target: target_fingerprint,
                    candidate_key: schedule.certificate_binding_fingerprint_v2(),
                }],
                source,
                target_fingerprint,
                |_| Some(evidence),
            );
            let ori_collision::CertifiedPathGraphSearchResultV1::Certified(certificate) = segment
            else {
                return Err("petal segment path is uncertified".to_owned());
            };
            path_segments.push(certificate);
            native_edges.push(DyadicPathEdgeAuthorityV1 {
                source,
                target: target_fingerprint,
                schedule,
                closure: Some(closure),
                auxiliary: DyadicAuxiliaryProofV1::ChainedGraph {
                    positive,
                    layer_index,
                },
            });
        }
        let path = ori_collision::issue_private_three_segment_path_v1(
            path_segments
                .try_into()
                .map_err(|_| "petal segment count mismatch".to_owned())?,
        )
        .ok_or_else(|| "petal parent path is uncertified".to_owned())?;
        let target_angles = native_edges
            .last()
            .and_then(|edge| edge.schedule.evaluate(1.0))
            .ok_or_else(|| "petal target is unavailable".to_owned())?;
        (path, native_edges, target_angles, transport)
    };
    let target_binding = pose_state_fingerprint_v1(&target_angles);
    let path_binding = path
        .binding_fingerprint_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let authority = DyadicPathNativeAuthorityV1 {
        read_scope: None,
        pose_capability,
        layer_capability,
        path,
        edges,
        chained_graph_transport: Some(transport),
        paper_thickness_mm,
        target_angles,
    };
    RegularQuadPetalPreviewRecordV1::issue_v1(
        project,
        &hinge_ids,
        token,
        project.editor.revision(),
        target_binding,
        path_binding,
        authority,
    )
}

struct DyadicPathNativeAuthorityV1 {
    read_scope: Option<StackedFoldReadGenerationLeaseV1>,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
    path: ori_collision::CertifiedPoseGraphPathCertificateV1,
    edges: Vec<DyadicPathEdgeAuthorityV1>,
    chained_graph_transport: Option<ori_collision::ChainedGeneralCellTransportAuthorityV1>,
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
    ChainedGraph {
        positive: ori_collision::PositiveThicknessContinuousCertificateV1,
        layer_index: usize,
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
    #[cfg(test)]
    fn regular_quad_petal_segment_certificates_v1(
        &self,
        record_target: [u8; 32],
        record_path_binding: &str,
    ) -> Option<[ori_collision::CertifiedPoseGraphPathCertificateV1; 3]> {
        if !self.revalidates_exact_three_graph_segments_v1(record_target, record_path_binding) {
            return None;
        }
        Some([
            self.path.segment_certificate_v1(0)?,
            self.path.segment_certificate_v1(1)?,
            self.path.segment_certificate_v1(2)?,
        ])
    }

    /// Stronger authority boundary for the dormant regular-quad petal path.
    ///
    /// A petal preview must never accept the Tree fallback or infer continuous
    /// layer transport from the endpoint capability.  Every one of its three
    /// segments must carry the graph positive-thickness and cell-transport
    /// certificates minted for that exact schedule.
    fn revalidates_exact_three_graph_segments_v1(
        &self,
        record_target: [u8; 32],
        record_path_binding: &str,
    ) -> bool {
        let Some(chained) = self.chained_graph_transport.as_ref() else {
            return false;
        };
        exact_three_graph_segment_shape_v1(
            self.edges.len(),
            self.edges
                .iter()
                .enumerate()
                .filter(|(index, edge)| {
                    matches!(
                        &edge.auxiliary,
                        DyadicAuxiliaryProofV1::ChainedGraph { layer_index, .. }
                            if *layer_index == *index
                    )
                })
                .count(),
        ) && chained.proofs().len() == 3
            && self.revalidates_private_proofs_v1(record_target, record_path_binding)
    }

    fn revalidates_private_proofs_v1(
        &self,
        record_target: [u8; 32],
        record_path_binding: &str,
    ) -> bool {
        let Some((geometry, audit, _pose)) = self.pose_capability.graph() else {
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
                                audit,
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
                        DyadicAuxiliaryProofV1::ChainedGraph {
                            positive,
                            layer_index,
                        } => {
                            let Some(closure) = edge.closure.as_ref() else {
                                return false;
                            };
                            let Some(layer) = self
                                .chained_graph_transport
                                .as_ref()
                                .and_then(|authority| authority.proofs().get(*layer_index))
                            else {
                                return false;
                            };
                            positive.is_for(
                                geometry,
                                audit,
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

const fn exact_three_graph_segment_shape_v1(
    segment_count: usize,
    graph_segment_count: usize,
) -> bool {
    segment_count == 3 && graph_segment_count == 3
}

#[tauri::command]
pub(super) fn read_bounded_dyadic_pose_graph_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    request: DyadicPoseGraphReadRequestV1,
) -> Result<DyadicPoseGraphReadResponseV1, String> {
    read_bounded_dyadic_pose_graph_inner_v1(&app_state, Some(&foldability_state), request, None)
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
    if !dyadic_request_hinge_counts_are_bounded_v1(
        request.target_angles.len(),
        request
            .cycle_schedule_v1
            .as_ref()
            .map(|schedule| schedule.entries.len()),
    ) {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    if !(1..=MAX_DYADIC_GRAPH_STATES_V1).contains(&request.max_states)
        || !(1..=MAX_DYADIC_GRAPH_TRANSITIONS_V1).contains(&request.max_transitions)
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    if !matches!(request.level_count, 3 | 5 | 9) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
    }
    validate_progress_request_id_v1(request.progress_request_id.as_deref())?;
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
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
    let uses_collective_graph = collective_schedule.is_some()
        || audit.closure_hinges().len() >= 2
        || target.as_slice().len() >= 32;
    if uses_collective_graph && (request.max_states < 3 || request.max_transitions < 4) {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    // Only a request that has passed every bounded, binding, and schedule
    // admission check may replace the process-wide read generation. The
    // project guard remains held, preserving the global project ->
    // publication lock order used by the final publication path.
    let read_scope = begin_stacked_fold_read_scope_v1(request.progress_request_id.clone())?;
    let generation = read_scope.generation();
    let mut read_scope = Some(read_scope);
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
    } else if uses_collective_graph {
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
                )
            });
            let mut tree_positive_seed = None;
            let evidence = cycle_evidence.or_else(|| {
                if edge.source != pose_state_fingerprint_v1(pose.hinge_angles()) {
                    return None;
                }
                let closure = closure.as_ref()?;
                let (tree_model, tree_source_pose) = capability.tree()?;
                let target = generated.schedule().evaluate(1.0)?;
                let positive = ori_collision::certify_positive_thickness_tree_continuous_path_v1(
                    tree_model,
                    tree_source_pose,
                    &target,
                    paper_thickness_mm,
                )?;
                let evidence =
                    ori_collision::certify_positive_thickness_tree_scheduled_transition_v1(
                        geometry,
                        audit,
                        pose.fixed_face(),
                        &generated,
                        closure,
                        tree_model,
                        tree_source_pose,
                        &target,
                        paper_thickness_mm,
                        &positive,
                    )?;
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
            let mut proof = positive
                .zip(layer)
                .map(|(positive, (_, layer))| DyadicAuxiliaryProofV1::Graph { positive, layer });
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
                .collect::<String>();
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
                read_scope: read_scope.take(),
                pose_capability: capability,
                layer_capability,
                path,
                edges,
                chained_graph_transport: None,
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
    if let Some(request_id) = request_id {
        emit_current_cycle_terminal_owned_v1(
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
    let progress_request_id_owned = request.progress_request_id.clone();
    let progress_request_id =
        validate_progress_request_id_v1(progress_request_id_owned.as_deref())?;
    let target_revision = super::stacked_fold_transaction::next_current_cycle_target_revision_v1(
        request.expected_revision,
    )
    .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
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
    if !automatic_kawasaki
        && (request.cycle_schedule_v1.entries.is_empty()
            || request.cycle_schedule_v1.entries.iter().any(|entry| {
                entry.numerator_power_coefficients.is_empty()
                    || entry.numerator_power_coefficients.len()
                        > MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1 + 1
                    || entry.denominator_power_coefficients.is_empty()
                    || entry.denominator_power_coefficients.len()
                        > MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1 + 1
            }))
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
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
    // Invalid, stale, and non-admissible requests must leave an older valid
    // analysis running. Advance the generation only after all cheap
    // admission checks have succeeded, while retaining project ->
    // publication lock order.
    let generation_scope = begin_stacked_fold_read_scope_v1(progress_request_id_owned.clone())?;
    let generation = generation_scope.generation();
    let paper_thickness_mm = project.editor.paper().thickness_mm;
    let basis_closure = match geometry.prove_simultaneous_cycle_basis_schedule_closure_v1(
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
    ) {
        Ok(value) => value,
        Err(_) => {
            return prepare_blockwise_current_cycle_fallback_v1(
                app,
                transaction_state,
                &project,
                foldability_state,
                pose_capability,
                layer_capability,
                &generated,
                &requested,
                paper_thickness_mm,
                generation,
                progress_request_id,
                request.expected_revision,
                target_revision,
            )
            .map_err(normalize_blockwise_current_cycle_fallback_error_v1);
        }
    };
    let closure = basis_closure.closure().clone();
    let continuous = match diagnose_scheduled_cycle_path_for_thickness_v1(
        paper_thickness_mm,
        || {
            supports_scheduled_positive_thickness_path_v1(
                geometry,
                audit,
                pose.fixed_face(),
                generated.schedule(),
            )
        },
        || {
            diagnose_scheduled_cycle_path_v1(
                geometry,
                audit,
                pose.fixed_face(),
                &generated,
                &closure,
                32,
            )
        },
        |thickness| {
            diagnose_scheduled_positive_thickness_cycle_path_v1(
                geometry,
                audit,
                pose.fixed_face(),
                &generated,
                &closure,
                thickness,
                32,
            )
        },
        |diagnostic| {
            (
                diagnostic.continuous_certificate_model_id(),
                diagnostic.positive_thickness_bits(),
            )
        },
    ) {
        Ok(continuous) => continuous,
        Err(ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness) => {
            return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
        }
        Err(
            ScheduledCycleThicknessDiagnosticErrorV1::PositiveThicknessUnsupported
            | ScheduledCycleThicknessDiagnosticErrorV1::Uncertified,
        ) => {
            return prepare_blockwise_current_cycle_fallback_v1(
                app,
                transaction_state,
                &project,
                foldability_state,
                pose_capability,
                layer_capability,
                &generated,
                &requested,
                paper_thickness_mm,
                generation,
                progress_request_id,
                request.expected_revision,
                target_revision,
            )
            .map_err(normalize_blockwise_current_cycle_fallback_error_v1);
        }
    };
    let expected = ori_collision::certify_scheduled_cycle_transition_v1(
        geometry,
        audit,
        pose.fixed_face(),
        &generated,
        &closure,
        32,
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
        let positive = controlled_cycle_authority_read_v1(
            generation,
            &CooperativeOperationControlV1::unbounded(),
            |control| {
                certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                    geometry,
                    audit,
                    pose.fixed_face(),
                    generated.schedule(),
                    &closure,
                    paper_thickness_mm,
                    32,
                    control,
                )
            },
        )
        .map_err(|error| controlled_cycle_authority_read_message_v1(error).to_owned())?
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        let layer_records = source
            .overlap_cells
            .iter()
            .try_fold(0usize, |sum, cell| {
                sum.checked_add(cell.bottom_to_top_faces.len())
            })
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let transition_count = closure_leaf_count
            .checked_add(1)
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
            .and_then(|work| work.checked_mul(transition_count))
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
                    max_transitions: transition_count,
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
    let target_angles = requested
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect();
    let pose_is_current = project
        .applied_pose_authority
        .revalidate_capability(&project, &pose_capability)
        .map_err(|_| STALE_MESSAGE.to_owned())?
        .is_some();
    let layer_is_current = match (foldability_state, layer_capability.as_ref()) {
        (Some(state), Some(capability)) => {
            revalidate_current_layer_order_capability(state, &project, capability)
                .map_err(|_| STALE_MESSAGE.to_owned())?
                .is_some()
        }
        (None, None) => true,
        _ => false,
    };
    if !pose_is_current || !layer_is_current {
        return Err(STALE_MESSAGE.to_owned());
    }
    let pending = super::stacked_fold_transaction::PendingCurrentCyclePosePremisesV1 {
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
    };
    let source_layer_order = source_layer_order.unwrap_or_default();
    let target_layer_order = source_layer_order.clone();
    let (layer_model_id, layer_transition_count, layer_pair_count, layer_target_hash) =
        layer_transport_metadata.map_or((None, 0, 0, None), |value| {
            (Some(value.0), value.1, value.2, Some(value.3))
        });
    let token = ProjectId::new();
    let response = CurrentCyclePosePreviewResponseV1 {
        version: 1,
        transaction_token: token,
        source_revision: request.expected_revision,
        target_revision,
        closure_leaf_count,
        closure_max_depth,
        checked_hinge_count,
        total_hinge_count,
        continuous_path_certified: true,
        continuous_layer_transport_model_id: layer_model_id,
        continuous_layer_transition_count: layer_transition_count,
        continuous_layer_pair_order_count: layer_pair_count,
        continuous_layer_target_order_sha256: layer_target_hash,
        target_layer_order,
        source_layer_order,
        authorizes_project_mutation: false,
    };
    with_current_cycle_publication_v1(generation, || {
        super::stacked_fold_transaction::install_pending_current_cycle_pose_with_token_v1(
            &transaction_state,
            token,
            pending,
            pose_capability,
            layer_capability,
        )
    })?;
    Ok(response)
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

fn emit_current_cycle_terminal_owned_v1(app: &AppHandle, request_id: String, status: &'static str) {
    // A presentation-only event must never hide an already issued native
    // transaction token. The request ID is owned before publication and
    // emitter failures or unwinds are isolated from the command result.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.emit(
            CURRENT_CYCLE_POSE_PROGRESS_EVENT_V1,
            CurrentCyclePoseProgressDtoV1 {
                version: 1,
                request_id,
                status,
                completed_work: 2,
                total_work: 2,
                authorizes_project_mutation: false,
            },
        );
    }));
}

#[cfg(test)]
fn begin_stacked_fold_read_generation_v1() -> Result<u64, String> {
    begin_stacked_fold_read_generation_for_request_v1(None)
}

#[derive(Debug)]
struct StackedFoldReadGenerationLeaseV1 {
    generation: u64,
}

impl StackedFoldReadGenerationLeaseV1 {
    const fn generation(&self) -> u64 {
        self.generation
    }
}

impl Drop for StackedFoldReadGenerationLeaseV1 {
    fn drop(&mut self) {
        let mut publication = STACKED_FOLD_READ_PUBLICATION_GATE_V1
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == self.generation {
            publication.active_request_id = None;
        }
    }
}

fn begin_stacked_fold_read_scope_v1(
    request_id: Option<String>,
) -> Result<StackedFoldReadGenerationLeaseV1, String> {
    begin_stacked_fold_read_generation_for_request_v1(request_id)
        .map(|generation| StackedFoldReadGenerationLeaseV1 { generation })
}

fn begin_stacked_fold_read_generation_for_request_v1(
    request_id: Option<String>,
) -> Result<u64, String> {
    let mut publication = STACKED_FOLD_READ_PUBLICATION_GATE_V1
        .lock()
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    if let Some(request_id) = request_id.as_ref()
        && let Some(index) = publication
            .pre_cancelled_request_ids
            .iter()
            .position(|cancelled| cancelled == request_id)
    {
        publication.pre_cancelled_request_ids.remove(index);
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    let generation = STACKED_FOLD_READ_GENERATION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        })
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?
        .checked_add(1)
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    publication.active_request_id = request_id;
    Ok(generation)
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

enum StackedFoldPathAnalysis {
    Tree(ori_collision::StackedFoldBoundedPathDiagnosticV1),
    Graph {
        diagnostic: ori_collision::StackedFoldCyclePathDiagnosticV1,
        requested_angle_degrees: f64,
    },
}

enum NativeStackedFoldPremises {
    Tree(super::stacked_fold_transaction::PendingStackedFoldPremises),
    SpeculativeTree(super::stacked_fold_transaction::PendingSpeculativeStackedFoldPremisesV1),
    Graph(super::stacked_fold_transaction::PendingStackedFoldGraphPremises),
}

fn stacked_fold_read_binding_is_current_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    binding: StackedFoldReadBindingV1,
    expected_source_fingerprint_sha256: &str,
) -> Result<bool, String> {
    let project = lock_project(app_state).map_err(|_| STALE_MESSAGE.to_owned())?;
    if project.instance_id != binding.project_instance_id()
        || project.project_id != binding.project_id()
        || project.editor.revision() != binding.source_revision()
        || project.editor.fold_model_fingerprint_v1() != expected_source_fingerprint_sha256
    {
        return Ok(false);
    }
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .map_err(|_| STALE_MESSAGE.to_owned())?;
    let layer_capability = capture_current_layer_order_capability(foldability_state, &project)
        .map_err(|_| STALE_MESSAGE.to_owned())?;
    Ok(pose_capability
        .as_ref()
        .is_some_and(|capability| capability.generation() == binding.pose_generation())
        && layer_capability
            .as_ref()
            .is_some_and(|capability| capability.generation() == binding.layer_order_generation()))
}

fn stacked_fold_read_capabilities_match_project_v1(
    project: &ProjectState,
    foldability_state: &GlobalFlatFoldabilityState,
    binding: StackedFoldReadBindingV1,
    expected_source_fingerprint_sha256: &str,
    pose_capability: &CurrentAppliedPoseCapability,
    layer_capability: &CurrentLayerOrderCapability,
) -> Result<bool, String> {
    if project.instance_id != binding.project_instance_id()
        || project.project_id != binding.project_id()
        || project.editor.revision() != binding.source_revision()
        || project.editor.fold_model_fingerprint_v1() != expected_source_fingerprint_sha256
        || pose_capability.generation() != binding.pose_generation()
        || layer_capability.generation() != binding.layer_order_generation()
    {
        return Ok(false);
    }
    let pose_is_current = project
        .applied_pose_authority
        .revalidate_capability(project, pose_capability)
        .map_err(|_| STALE_MESSAGE.to_owned())?
        .is_some();
    let layer_is_current =
        revalidate_current_layer_order_capability(foldability_state, project, layer_capability)
            .map_err(|_| STALE_MESSAGE.to_owned())?
            .is_some();
    Ok(pose_is_current && layer_is_current)
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
    validate_progress_request_id_v1(request.progress_request_id.as_deref())?;
    let path_variant_count = usize::from(request.cycle_schedule_v1.is_some())
        + usize::from(request.linear_candidate_v1.is_some())
        + usize::from(request.certified_path_graph_v1.is_some());
    if path_variant_count > 1 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
    }
    if let Some(exact_path) = request
        .linear_candidate_v1
        .as_ref()
        .and_then(|linear| linear.exact_dyadic_path_v1.as_ref())
    {
        validate_exact_dyadic_candidate_path_v1(exact_path).map_err(str::to_owned)?;
    }
    let progress_request_id = request.progress_request_id.clone();
    let (
        paper,
        pattern,
        pose_capability,
        layer_capability,
        binding,
        pair_proof_cache,
        pair_proof_cache_capture,
        _analysis_scope,
        analysis_generation,
    ) = {
        let project = lock_project(&app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
        {
            return Err(STALE_MESSAGE.to_owned());
        }
        if request.certified_path_graph_v1.is_some() {
            preflight_certified_path_graph_thickness_v1(project.editor.paper().thickness_mm)
                .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
        }
        let pose_authority = project.applied_pose_authority.clone();
        let pose_capability = pose_authority
            .capture_capability(&project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        if pose_capability.tree().is_none() {
            return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
        }
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
        let pair_proof_cache = pose_authority.pair_proof_cache_runtime_v1();
        let pair_proof_cache_binding = positive_pair_proof_cache_binding_v1(
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            ori_foldability::fold_model_fingerprint_v1(
                project.editor.pattern(),
                project.editor.paper(),
            )
            .0,
            pose_capability.generation(),
            project.editor.paper().thickness_mm,
        )
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        // Lock order is project -> pose (capture_capability above) -> cache.
        let pair_proof_cache_capture = pair_proof_cache_binding
            .map(|binding| pair_proof_cache.capture_v1(binding))
            .transpose()
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        let paper = project.editor.paper().clone();
        let pattern = project.editor.pattern().clone();
        // Malformed, stale, unsupported-thickness, and unavailable authority
        // requests must not cancel the current permit owner. This fully
        // admitted request linearizes replacement while the project binding is
        // still locked, preserving project -> publication lock order. Worker
        // capacity is awaited only after this project guard is released.
        let analysis_scope = begin_stacked_fold_read_scope_v1(progress_request_id.clone())?;
        let analysis_generation = analysis_scope.generation();
        (
            paper,
            pattern,
            pose_capability,
            layer_capability,
            binding,
            pair_proof_cache,
            pair_proof_cache_capture,
            analysis_scope,
            analysis_generation,
        )
    };
    let worker_permit = app_state
        .1
        .acquire_notified_while(move || {
            STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == analysis_generation
        })
        .await
        .ok_or_else(|| CANCELLED_MESSAGE.to_owned())?;
    let paper_thickness_mm = paper.thickness_mm;
    let progress_app = app.cloned();
    #[cfg(test)]
    let prepublication_request_id = progress_request_id.clone();
    let analysis = tauri::async_runtime::spawn_blocking(move || {
        let (model, pose) = pose_capability
            .tree()
            .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let input = FlatEndpointLayerOrderInputV1 {
            identity_namespace: binding.project_id(),
            source_revision: binding.source_revision(),
            paper: &paper,
            pattern: &pattern,
            model,
            pose,
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
            let initial = prepare_stacked_fold_initial_graph_pose_v1(audited_target, model, pose)
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
                    cycle
                        .entries
                        .iter()
                        .map(|entry| {
                            ori_kinematics::HingeAngle::new(
                                entry.edge,
                                entry.requested_angle_degrees,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
                )
                .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
                Some((
                    ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
                        schedule,
                        initial.pose().hinge_angles(),
                        &requested,
                    )
                    .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
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
            ) = if let Some(graph) = request.certified_path_graph_v1.as_ref() {
                let states = validate_certified_path_graph_v1(graph, initial.pose().hinge_angles())
                    .map_err(str::to_owned)?;
                if states[graph.target_state]
                    .as_slice()
                    .iter()
                    .zip(states[graph.source_state].as_slice())
                    .any(|(target, source)| {
                        target.angle_degrees().to_bits() != source.angle_degrees().to_bits()
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
                let searched = ori_collision::search_certified_pose_graph_with_progress_v1(
                    &fingerprints,
                    &candidates,
                    fingerprints[graph.source_state],
                    fingerprints[graph.target_state],
                    || STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == analysis_generation,
                    |progress| {
                        #[cfg(test)]
                        if progress_request_id
                            .as_deref()
                            .and_then(|value| value.strip_prefix("test-cancel-after-"))
                            .and_then(|value| value.parse::<usize>().ok())
                            .is_some_and(|limit| progress.evaluated_transition_count >= limit)
                        {
                            if let Some(request_id) = progress_request_id.as_ref() {
                                let _ = cancel_current_stacked_fold_read_request_inner_v1(
                                    request_id.clone(),
                                );
                            } else {
                                let _ = cancel_current_stacked_fold_read_inner_v1();
                            }
                        }
                        if let Some(request_id) = progress_request_id.as_ref() {
                            if let Some(progress_app) = progress_app.as_ref() {
                                let _ = progress_app.emit(
                                    STACKED_FOLD_READ_PROGRESS_EVENT_V1,
                                    StackedFoldReadProgressDtoV1 {
                                        version: 1,
                                        request_id: request_id.clone(),
                                        explored_state_count: progress.explored_state_count,
                                        evaluated_transition_count: progress
                                            .evaluated_transition_count,
                                        state_limit: progress.state_limit,
                                        transition_limit: progress.transition_limit,
                                        authorizes_project_mutation: false,
                                    },
                                );
                            }
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
                        let closure = match initial
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
                        reason:
                            ori_collision::CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
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
                            schedule_certificate_sha256: lowercase_hex(edge.schedule_certificate()),
                            collision_certificate_sha256: lowercase_hex(
                                edge.collision_certificate(),
                            ),
                            closure_certificate_sha256: lowercase_hex(edge.closure_certificate()),
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
                let all_flat = requested
                    .as_slice()
                    .iter()
                    .all(|entry| entry.angle_degrees().to_bits() == 180.0_f64.to_bits());
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
                    requested
                        .as_slice()
                        .iter()
                        .all(|entry| entry.angle_degrees().to_bits() == 180.0_f64.to_bits()),
                    None,
                    None,
                    Vec::new(),
                )
            } else {
                let linear = request
                    .linear_candidate_v1
                    .as_ref()
                    .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
                let (initial_angles, requested_angles) =
                    validate_linear_candidate_angles_v1(linear, initial.pose().hinge_angles())
                        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
                let all_flat = linear
                    .entries
                    .iter()
                    .all(|entry| entry.requested_angle_degrees.to_bits() == 180.0_f64.to_bits());
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
            } else {
                generate_linear_multi_hinge_path_candidate_v1(
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
                })?
            };
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
            let continuous = diagnose_scheduled_cycle_path_for_thickness_v1(
                paper_thickness_mm,
                || {
                    supports_scheduled_positive_thickness_path_v1(
                        initial.target().hinge_geometry(),
                        initial.target().audit(),
                        initial.pose().fixed_face(),
                        generated.schedule(),
                    )
                },
                || {
                    diagnose_scheduled_cycle_path_v1(
                        initial.target().hinge_geometry(),
                        initial.target().audit(),
                        initial.pose().fixed_face(),
                        &generated,
                        &interval_closure,
                        StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
                    )
                },
                |thickness| {
                    diagnose_scheduled_positive_thickness_cycle_path_v1(
                        initial.target().hinge_geometry(),
                        initial.target().audit(),
                        initial.pose().fixed_face(),
                        &generated,
                        &interval_closure,
                        thickness,
                        StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
                    )
                },
                |diagnostic| {
                    (
                        diagnostic.continuous_certificate_model_id(),
                        diagnostic.positive_thickness_bits(),
                    )
                },
            )
            .map_err(|error| match error {
                ScheduledCycleThicknessDiagnosticErrorV1::InvalidThickness
                | ScheduledCycleThicknessDiagnosticErrorV1::PositiveThicknessUnsupported => {
                    CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned()
                }
                ScheduledCycleThicknessDiagnosticErrorV1::Uncertified => {
                    // The bounded CCD diagnostic intentionally does not
                    // distinguish an actual collision from an enclosure that
                    // stayed unresolved at its subdivision limit.
                    CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned()
                }
            })?;
            let closed_endpoint = ori_core::prepare_stacked_fold_requested_scheduled_graph_pose_v1(
                initial,
                generated.schedule(),
                &interval_closure,
                requested_angles,
                candidate.requested_angle_degrees(),
            )
            .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
            let geometry_proof = closed_endpoint.initial().target().geometry().proof();
            let topology = closed_endpoint.initial().target().geometry().candidate();
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
                    let local = analyze_local_flat_foldability(&topology.paper, &topology.pattern);
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
                        super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(layer_order),
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
            let expected_pair_count = face_count
                .checked_sub(1)
                .and_then(|prior| face_count.checked_mul(prior))
                .map(|ordered| ordered / 2)
                .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
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
            let separated_pair_count = expected_pair_count
                .checked_sub(adjacent_pair_count)
                .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let endpoint_collision = StackedFoldEndpointCollisionDto {
                expected_pair_count,
                separated_pair_count,
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
                apply_contract_version: STACKED_FOLD_APPLY_CONTRACT_VERSION_V1,
                apply_mode: StackedFoldApplyModeDtoV1::None,
                transaction_token: None,
                speculative_unproven_available: false,
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
                .collect::<Vec<_>>();
            validate_stacked_fold_layer_view_cells_v1(&crossed_cells)?;
            let work = proposal.work();
            let support = proposal.support();
            let target_faces = proposal.target_faces().to_vec();
            let material_segments = material_map
                .segments()
                .iter()
                .map(|segment| {
                    Ok(StackedFoldMaterialSegmentDto {
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
                            _ => return Err(ANALYSIS_FAILED_MESSAGE.to_owned()),
                        },
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
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
        let prepared_initial_pose =
            prepare_stacked_fold_initial_pose_v1(prepared_target, model, pose)
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let moving_hinges = prepared_initial_pose
            .target()
            .geometry()
            .proof()
            .expected_creases()
            .iter()
            .flat_map(|subdivision| subdivision.target_edges().iter().copied())
            .collect::<Vec<_>>();
        let path_limits = StackedFoldPathDiagnosticLimitsV1::default();
        let mut continuous_path =
            if let Some(pair_proof_cache_capture) = pair_proof_cache_capture.as_ref() {
                diagnose_collective_hinge_path_with_pair_cache_v1(
                    prepared_initial_pose.target().model(),
                    prepared_initial_pose.pose(),
                    &moving_hinges,
                    candidate.requested_angle_degrees(),
                    paper.thickness_mm,
                    path_limits,
                    &pair_proof_cache,
                    pair_proof_cache_capture,
                    stacked_fold_pair_cache_control_v1(
                        analysis_generation,
                        Instant::now() + Duration::from_secs(30),
                    ),
                )
            } else {
                // Zero-thickness and every non-model-4 branch keep the established
                // uncached diagnostic. The library cache wrapper itself admits
                // only model 4 when a positive-thickness capture is present.
                diagnose_collective_hinge_path_v1(
                    prepared_initial_pose.target().model(),
                    prepared_initial_pose.pose(),
                    &moving_hinges,
                    candidate.requested_angle_degrees(),
                    paper.thickness_mm,
                    path_limits,
                )
            }
            .map_err(|error| map_cached_tree_path_error_v1(error).to_owned())?;
        let prepared_requested_pose = prepare_stacked_fold_requested_pose_v1(
            prepared_initial_pose,
            candidate.requested_angle_degrees(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let mut initial_layer_order = None;
        if paper.thickness_mm.to_bits() == 0.0_f64.to_bits()
            && !continuous_path.continuous_clearance_certified()
            && continuous_path.continuous_certificate_model_id().is_none()
            && continuous_path
                .first_sampled_blocking_angle_degrees()
                .is_some_and(|angle| angle.to_bits() == 0.0_f64.to_bits())
            && let Ok(order) = prepare_stacked_fold_initial_layer_order_v1(
                prepared_requested_pose.initial(),
                layer_capability.snapshot(),
                DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
            )
            && let Ok(admitted) = diagnose_stacked_fold_requested_path_with_initial_layer_order_v1(
                &prepared_requested_pose,
                paper.thickness_mm,
                path_limits,
                &order,
            )
        {
            continuous_path = admitted;
            initial_layer_order = Some(order);
        }
        let layer_admitted_speculative_path = initial_layer_order.is_some()
            && super::stacked_fold_transaction::speculative_tree_diagnostic_is_issuable_v1(
                &continuous_path,
            );
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
        let positive_thickness_certificate =
            is_positive_thickness_continuous_certificate_model_id_v2(
                continuous_path.continuous_certificate_model_id(),
            );
        let endpoint_collision_plan = endpoint_collision_plan_v1(
            candidate.requested_angle_degrees(),
            positive_thickness_certificate,
        );
        let exact_flat_endpoint =
            endpoint_collision_plan == EndpointCollisionPlanV1::DeferToFlatLayerOrder;
        let mut endpoint_collision =
            if endpoint_collision_plan != EndpointCollisionPlanV1::StaticGeometry {
                let face_count = prepared_requested_pose
                    .initial()
                    .target()
                    .model()
                    .face_ids()
                    .len();
                let expected_pair_count = face_count
                    .checked_sub(1)
                    .and_then(|prior| face_count.checked_mul(prior))
                    .map(|ordered| ordered / 2)
                    .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
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
                admit_initial_layer_order_endpoint_v1(
                    StackedFoldEndpointCollisionDto {
                        expected_pair_count: endpoint.expected_unordered_face_pairs(),
                        separated_pair_count: endpoint.separated_pairs(),
                        touching_pair_count: endpoint.touching_pairs(),
                        allowed_pair_count: endpoint.allowed_pairs(),
                        penetrating_pair_count: endpoint.penetrating_pairs(),
                        indeterminate_pair_count: endpoint.indeterminate_pairs(),
                        has_blocking_hold: endpoint.has_prominent_blocking_hold(),
                    },
                    layer_admitted_speculative_path,
                )
                .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?
            };
        let (flat_endpoint_layer_order, transaction_layer_order) = if exact_flat_endpoint {
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
                GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => {
                    let model = prepared_requested_pose.initial().target().model();
                    let pose = prepared_requested_pose.pose();
                    let anchor = anchor_flat_endpoint_layer_order_v1(
                        FlatEndpointLayerOrderInputV1 {
                            identity_namespace: binding.project_id(),
                            source_revision: target_revision,
                            paper: &topology.paper,
                            pattern: &topology.pattern,
                            model,
                            pose,
                            layer_order: &layer_order,
                        },
                        ori_collision::FlatEndpointLayerOrderLimitsV1::default(),
                    )
                    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                    let endpoint = diagnose_static_collision_geometry_with_flat_layer_order_v1(
                        model,
                        pose,
                        FLAT_ENDPOINT_COLLISION_THICKNESS_MM_V1,
                        StaticCollisionLimits::default(),
                        &anchor,
                    )
                    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                    if endpoint.has_prominent_blocking_hold() {
                        return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
                    }
                    endpoint_collision = StackedFoldEndpointCollisionDto {
                        expected_pair_count: endpoint.expected_unordered_face_pairs(),
                        separated_pair_count: endpoint.separated_pairs(),
                        touching_pair_count: endpoint.touching_pairs(),
                        allowed_pair_count: endpoint.allowed_pairs(),
                        penetrating_pair_count: endpoint.penetrating_pairs(),
                        indeterminate_pair_count: endpoint.indeterminate_pairs(),
                        has_blocking_hold: false,
                    };
                    (
                        StackedFoldFlatEndpointLayerOrderDto {
                            applicable: true,
                            certified: true,
                            material_face_count: layer_order.material_faces.len(),
                            overlap_cell_count: layer_order.overlap_cells.len(),
                        },
                        None,
                    )
                }
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
        if mountain_crease_count
            .checked_add(valley_crease_count)
            .is_none_or(|count| count != expected_creases.len())
        {
            return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
        }
        let transaction_failures = transaction_failure_classes(
            continuous_path.continuous_clearance_certified(),
            flat_endpoint_layer_order.certified,
        );
        let transaction_proposal = StackedFoldTransactionProposalDto {
            apply_contract_version: STACKED_FOLD_APPLY_CONTRACT_VERSION_V1,
            apply_mode: StackedFoldApplyModeDtoV1::None,
            transaction_token: None,
            speculative_unproven_available: false,
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
        let native_transaction = transaction_layer_order.and_then(|layer_order| {
            if continuous_path.continuous_clearance_certified() {
                Some(NativeStackedFoldPremises::Tree(
                    super::stacked_fold_transaction::PendingStackedFoldPremises {
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
                    },
                ))
            } else if layer_admitted_speculative_path
                && endpoint_allows_speculative_apply_v1(&endpoint_collision)
            {
                let initial_layer_order = initial_layer_order?;
                Some(NativeStackedFoldPremises::SpeculativeTree(
                    super::stacked_fold_transaction::PendingSpeculativeStackedFoldPremisesV1 {
                        expected_instance_id: binding.project_instance_id(),
                        expected_project_id: binding.project_id(),
                        expected_revision: binding.source_revision(),
                        expected_source_fingerprint: source_fingerprint_bytes,
                        expected_pose_generation: binding.pose_generation(),
                        expected_layer_generation: binding.layer_order_generation(),
                        requested: prepared_requested_pose,
                        continuous: continuous_path,
                        diagnostic_paper_thickness_bits: paper_thickness_mm.to_bits(),
                        paper_thickness_mm,
                        initial_layer_order,
                        layer_order,
                        endpoint_has_blocking_hold: endpoint_collision.has_blocking_hold,
                        endpoint_penetrating_pair_count: endpoint_collision.penetrating_pair_count,
                        endpoint_indeterminate_pair_count: endpoint_collision
                            .indeterminate_pair_count,
                    },
                ))
            } else {
                None
            }
        });
        let crossed_cells = proposal
            .crossed_cells()
            .iter()
            .map(|cell| StackedFoldReadCellDto {
                cell_key_sha256: lowercase_hex(cell.cell_key().canonical_bytes()),
                bottom_to_top_faces: cell.bottom_to_top_faces().to_vec(),
                boundary_world: cell.boundary_world().to_vec(),
            })
            .collect::<Vec<_>>();
        validate_stacked_fold_layer_view_cells_v1(&crossed_cells)?;
        let work = proposal.work();
        let support = proposal.support();
        let target_faces = proposal.target_faces().to_vec();
        let material_segments = material_map
            .segments()
            .iter()
            .map(|segment| {
                Ok(StackedFoldMaterialSegmentDto {
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
                        _ => return Err(ANALYSIS_FAILED_MESSAGE.to_owned()),
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
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

    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != analysis_generation {
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    let continuous_path_certified = match &continuous_path {
        StackedFoldPathAnalysis::Tree(value) => value.continuous_clearance_certified(),
        StackedFoldPathAnalysis::Graph { diagnostic, .. } => {
            diagnostic.continuous_certificate_model_id().is_some()
        }
    };
    let target_layer_order_certified = flat_endpoint_layer_order.certified;
    if !transaction_proposal
        .has_valid_apply_contract_v1(continuous_path_certified, target_layer_order_certified)
    {
        return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
    }
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
    let publication_token = native_transaction.as_ref().map(|transaction| {
        let token = ProjectId::new();
        match transaction {
            NativeStackedFoldPremises::Tree(_) | NativeStackedFoldPremises::Graph(_) => {
                transaction_proposal.publish_certified_v1(token);
            }
            NativeStackedFoldPremises::SpeculativeTree(_) => {
                transaction_proposal.publish_speculative_unproven_v1(token);
            }
        }
        token
    });
    #[cfg(test)]
    let prepublication_action = STACKED_FOLD_PREPUBLICATION_ACTION_V1.swap(0, Ordering::AcqRel);
    #[cfg(not(test))]
    let prepublication_action = 0_u8;
    #[cfg(test)]
    if prepublication_action == 1 {
        if let Some(request_id) = prepublication_request_id {
            let _ = cancel_current_stacked_fold_read_request_inner_v1(request_id);
        } else {
            let _ = cancel_current_stacked_fold_read_inner_v1();
        }
    }
    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != analysis_generation {
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    if prepublication_action == 2
        || !stacked_fold_read_binding_is_current_v1(
            app_state,
            foldability_state,
            binding,
            &transaction_proposal.source_fingerprint_sha256,
        )?
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    #[cfg(test)]
    if prepublication_action == 3 {
        transaction_proposal.apply_contract_version = 0;
    }
    if !transaction_proposal
        .has_valid_apply_contract_v1(continuous_path_certified, target_layer_order_certified)
    {
        return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
    }
    drop(worker_permit);

    let response = StackedFoldReadResponse {
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
            } => {
                let sampled_pose_count = diagnostic
                    .leaf_count()
                    .checked_add(1)
                    .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
                StackedFoldContinuousPathDto {
                    model_id: ori_collision::STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1,
                    continuous_certificate_model_id: diagnostic.continuous_certificate_model_id(),
                    sampled_pose_count,
                    sampled_nonblocking_pose_count: if diagnostic
                        .continuous_certificate_model_id()
                        .is_some()
                    {
                        sampled_pose_count
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
                    safe_stop_angle_degrees: if diagnostic
                        .continuous_certificate_model_id()
                        .is_some()
                    {
                        requested_angle_degrees
                    } else {
                        0.0
                    },
                    authorizes_project_mutation: false,
                    paper_thickness_mm,
                }
            }
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
    };
    let native_publication = match (native_transaction, publication_token) {
        (Some(transaction), Some(token)) => Some((transaction, token)),
        (None, None) => None,
        _ => return Err(ANALYSIS_FAILED_MESSAGE.to_owned()),
    };
    // Every fallible response construction and contract check is complete
    // before a pending token can displace an older valid proposal.
    let project = lock_project(app_state).map_err(|_| STALE_MESSAGE.to_owned())?;
    if !stacked_fold_read_capabilities_match_project_v1(
        &project,
        foldability_state,
        binding,
        &response.transaction_proposal.source_fingerprint_sha256,
        &pose_capability,
        &layer_capability,
    )? {
        return Err(STALE_MESSAGE.to_owned());
    }
    let _publication_guard = STACKED_FOLD_READ_PUBLICATION_GATE_V1
        .lock()
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != analysis_generation {
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    // Retain project -> publication order through the non-blocking registry
    // admission. The installers acquire every transaction registry with
    // try_lock before mutating either slot, so an apply path that already owns
    // a registry causes a closed error instead of a lock-order cycle.
    if let Some((native_transaction, token)) = native_publication {
        match native_transaction {
            NativeStackedFoldPremises::Tree(premises) => {
                super::stacked_fold_transaction::install_pending_stacked_fold_with_token_v1(
                    transaction_state,
                    token,
                    premises,
                    pose_capability,
                    layer_capability,
                )?
            }
            NativeStackedFoldPremises::SpeculativeTree(premises) => {
                super::stacked_fold_transaction::
                    install_pending_speculative_stacked_fold_with_token_v1(
                    transaction_state,
                    token,
                    premises,
                    pose_capability,
                    layer_capability,
                )?
            }
            NativeStackedFoldPremises::Graph(premises) => {
                super::stacked_fold_transaction::install_pending_stacked_fold_graph_with_token_v1(
                    transaction_state,
                    token,
                    premises,
                    pose_capability,
                    layer_capability,
                )?
            }
        };
    }
    drop(project);
    Ok(response)
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

#[cfg(test)]
#[path = "../../../../test-support/dense_grid_cycle.rs"]
mod dense_grid_cycle_test_support;
#[cfg(test)]
#[path = "../../../../test-support/four_bay_cycle.rs"]
mod four_bay_cycle_test_support;
#[cfg(test)]
#[path = "../../../../test-support/miura_cactus.rs"]
mod miura_cactus_test_support;
#[cfg(test)]
#[path = "../../../../test-support/theta_cycle.rs"]
mod theta_cycle_test_support;

#[cfg(test)]
#[path = "stacked_fold_read/tests.rs"]
pub(crate) mod tests;
