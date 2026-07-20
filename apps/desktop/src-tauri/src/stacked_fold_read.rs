//! Read-only desktop bridge for the first authenticated SIM-010 boundary.
//!
//! No value returned by this module authorizes project mutation. Heavy exact
//! analysis runs over detached immutable capabilities and is revalidated
//! against both live native slots before its bounded observation is returned.

use ori_collision::{
    FlatEndpointLayerOrderInputV1, StackedFoldFixedSideV1, StackedFoldLinearCandidateV1,
    StackedFoldMaterialMapLimitsV1, StackedFoldPathDiagnosticLimitsV1, StackedFoldReadBindingV1,
    StackedFoldReadLimitsV1, StackedFoldReadSupportV1, StackedFoldRotationDirectionV1,
    StaticCollisionLimits, capture_stacked_fold_read_guard_v1, diagnose_collective_hinge_path_v1,
    diagnose_static_collision_geometry, propose_linear_stacked_fold_read_v1,
    reverse_map_linear_stacked_fold_material_v1,
};
use ori_core::{
    DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS, ExpectedStackedFoldCreaseV1, FaceLineageLimits,
    StackedFoldGeometryLimitsV1, StackedFoldTopologyBuildLimitsV1, analyze_global_flat_foldability,
    analyze_local_flat_foldability, prepare_stacked_fold_geometry_candidate_v1,
    prepare_stacked_fold_initial_graph_pose_v1, prepare_stacked_fold_initial_pose_v1,
    prepare_stacked_fold_non_flat_layer_order_with_thickness_v1,
    prepare_stacked_fold_requested_graph_pose_v1, prepare_stacked_fold_requested_pose_v1,
    prepare_stacked_fold_target_graph_audit_v1, prepare_stacked_fold_target_model_v1,
};
use ori_domain::{FaceId, ProjectId};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityOutcome,
};
use ori_kinematics::{Point3, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, TopologyIssueSeverity, analyze_faces};
use serde::{Deserialize, Serialize};
use tauri::State;

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
const BUSY_MESSAGE: &str = "Another native pose analysis is already running.";
const STALE_MESSAGE: &str =
    "The project, current pose, or certified layer order changed during analysis.";

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
    first_sampled_blocking_angle_degrees: Option<f64>,
    requested_angle_degrees: f64,
    continuous_clearance_certified: bool,
    safe_stop_angle_degrees: f64,
    authorizes_project_mutation: bool,
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
    endpoint_collision: StackedFoldEndpointCollisionDto,
    continuous_path: StackedFoldContinuousPathDto,
    flat_endpoint_layer_order: StackedFoldFlatEndpointLayerOrderDto,
    transaction_proposal: StackedFoldTransactionProposalDto,
    work: StackedFoldReadWorkDto,
    authorizes_project_mutation: bool,
    authorizes_apply_stacked_fold: bool,
}

#[tauri::command]
pub(super) async fn propose_current_stacked_fold_read(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    transaction_state: State<'_, super::stacked_fold_transaction::StackedFoldTransactionState>,
    request: StackedFoldReadRequest,
) -> Result<StackedFoldReadResponse, String> {
    let worker_permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| BUSY_MESSAGE.to_owned())?;
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
            let _closed_endpoint = prepare_stacked_fold_requested_graph_pose_v1(
                initial,
                candidate.requested_angle_degrees(),
            )
            .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
            // Collision and safe-stop diagnostics currently accept only an
            // issuer-bound material-tree pose. Never downgrade a proved graph
            // endpoint into that authority.
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
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
        let endpoint_collision = diagnose_static_collision_geometry(
            prepared_requested_pose.initial().target().model(),
            prepared_requested_pose.pose(),
            paper.thickness_mm,
            StaticCollisionLimits::default(),
        )
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        if endpoint_collision.has_prominent_blocking_hold() {
            return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
        }
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
        let native_transaction = transaction_layer_order.map(|layer_order| {
            super::stacked_fold_transaction::PendingStackedFoldPremises {
                expected_instance_id: binding.project_instance_id(),
                expected_project_id: binding.project_id(),
                expected_revision: binding.source_revision(),
                expected_source_fingerprint: source_fingerprint_bytes,
                expected_pose_generation: binding.pose_generation(),
                expected_layer_generation: binding.layer_order_generation(),
                requested: prepared_requested_pose,
                continuous: continuous_path,
                layer_order,
            }
        });
        let crossed_cells = proposal
            .crossed_cells()
            .iter()
            .map(|cell| StackedFoldReadCellDto {
                cell_key_sha256: lowercase_hex(cell.cell_key().canonical_bytes()),
                bottom_to_top_faces: cell.bottom_to_top_faces().to_vec(),
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
            work,
            endpoint_collision,
            continuous_path,
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
        work,
        endpoint_collision,
        continuous_path,
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
        let token = super::stacked_fold_transaction::install_pending_stacked_fold(
            &transaction_state,
            native_transaction,
            pose_capability,
            layer_capability,
        )?;
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
        endpoint_collision: StackedFoldEndpointCollisionDto {
            expected_pair_count: endpoint_collision.expected_unordered_face_pairs(),
            separated_pair_count: endpoint_collision.separated_pairs(),
            touching_pair_count: endpoint_collision.touching_pairs(),
            allowed_pair_count: endpoint_collision.allowed_pairs(),
            penetrating_pair_count: endpoint_collision.penetrating_pairs(),
            indeterminate_pair_count: endpoint_collision.indeterminate_pairs(),
            has_blocking_hold: endpoint_collision.has_prominent_blocking_hold(),
        },
        continuous_path: StackedFoldContinuousPathDto {
            model_id: continuous_path.model_id(),
            continuous_certificate_model_id: continuous_path.continuous_certificate_model_id(),
            sampled_pose_count: continuous_path.sampled_pose_count(),
            sampled_nonblocking_pose_count: continuous_path.sampled_nonblocking_pose_count(),
            first_sampled_blocking_angle_degrees: continuous_path
                .first_sampled_blocking_angle_degrees(),
            requested_angle_degrees: continuous_path.requested_angle_degrees(),
            continuous_clearance_certified: continuous_path.continuous_clearance_certified(),
            safe_stop_angle_degrees: continuous_path.safe_stop_angle_degrees(),
            authorizes_project_mutation: continuous_path.authorizes_project_mutation(),
        },
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
}
