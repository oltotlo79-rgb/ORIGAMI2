//! Read-only viewer boundary for the applied non-flat layer order.
//!
//! This module rejoins the current project, the current applied pose, and the
//! project-owned [`CurrentLayerEvidence::NonFlat`] on every call, then returns
//! a versioned snapshot that is valid only for that instant. It never issues
//! apply, transaction, or mutation authority: `readOnly` and
//! `authorizesProjectMutation` are literals, and the underlying proof must
//! keep reporting `authorizes_apply_stacked_fold() == false`.
//!
//! World XYZ face geometry and per-cell projection UV geometry are separate
//! wire types on purpose. A cell boundary is a rounded point in the projection
//! plane of its faces, never a world coordinate, and `source_to_plane` is the
//! exact affine into that plane, never a world transform.

use ori_collision::{
    NonFlatCellTransportLimitsV1, preflight_non_flat_cell_transport_v1,
    validate_non_flat_layer_order_structure_v1,
};
use ori_core::StackedFoldNonFlatLayerOrderV1;
use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_foldability::{ExactRationalValue, ExactSign};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use super::applied_pose::{
    CurrentAppliedPoseView, capture_current_applied_pose_capability,
    revalidate_current_applied_pose_capability,
};
use super::stacked_fold_transaction::CurrentLayerEvidence;
use super::{AppState, ProjectState, lock_project, wire_id};

/// Stable semantic identifier of this read-only viewer contract.
pub(super) const CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1: &str =
    "native_stacked_fold_non_flat_planar_order_v1";

/// The two admissible applied-pose model IDs, one per issuer kind.
const TREE_POSE_MODEL_ID_V1: &str = ori_core::APPLIED_POSE_MODEL_ID_V1;
const GRAPH_POSE_MODEL_ID_V1: &str = ori_core::CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1;

const FACE_DOMAIN_V1: &[u8] = b"origami2.non_flat_layer_view.v1.face";
const EXACT_BOUNDARY_DOMAIN_V1: &[u8] = b"origami2.non_flat_layer_view.v1.exact_boundary";
const CELL_DOMAIN_V1: &[u8] = b"origami2.non_flat_layer_view.v1.cell";

const MAX_FACES_V1: usize = 512;
const MAX_HINGES_V1: usize = 4_096;
const MAX_CELLS_V1: usize = 4_096;
const MAX_FACE_PAIR_ORDERS_V1: usize = 4_096;
const MAX_WORLD_POLYGON_POINTS_V1: usize = 4_096;
const MAX_CELL_POLYGON_POINTS_V1: usize = 4_096;
const MAX_TOTAL_WORLD_BOUNDARY_POINTS_V1: usize = 100_000;
const MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1: usize = 100_000;
const MAX_EXACT_MAGNITUDE_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_SERIALIZED_JSON_BYTES_V1: usize = 16 * 1024 * 1024;
/// `Number.MAX_SAFE_INTEGER`; every wire count must stay lossless in JSON.
const MAX_SAFE_WIRE_INTEGER_V1: usize = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CurrentNonFlatLayerOrderViewErrorCategoryV1 {
    StaleAuthority,
    InvalidEvidence,
    ResourceLimit,
    InternalFailure,
}

/// Data-free error payload.
///
/// Only the contract version and a fixed category cross the boundary; project
/// identifiers, revisions, fingerprints, entity IDs, coordinates, and proof
/// contents never appear here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderViewErrorV1 {
    version: u8,
    category: CurrentNonFlatLayerOrderViewErrorCategoryV1,
}

impl CurrentNonFlatLayerOrderViewErrorV1 {
    const fn new(category: CurrentNonFlatLayerOrderViewErrorCategoryV1) -> Self {
        Self {
            version: 1,
            category,
        }
    }

    const fn stale() -> Self {
        Self::new(CurrentNonFlatLayerOrderViewErrorCategoryV1::StaleAuthority)
    }

    const fn invalid() -> Self {
        Self::new(CurrentNonFlatLayerOrderViewErrorCategoryV1::InvalidEvidence)
    }

    const fn resource() -> Self {
        Self::new(CurrentNonFlatLayerOrderViewErrorCategoryV1::ResourceLimit)
    }

    const fn internal() -> Self {
        Self::new(CurrentNonFlatLayerOrderViewErrorCategoryV1::InternalFailure)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentNonFlatLayerOrderViewHingeAngleRequestV1 {
    edge_id: EdgeId,
    angle_degrees: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentNonFlatLayerOrderViewPoseRequestV1 {
    fixed_face_id: FaceId,
    hinge_angles: Vec<CurrentNonFlatLayerOrderViewHingeAngleRequestV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentNonFlatLayerOrderViewRequestV1 {
    version: u8,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint_sha256: String,
    expected_applied_pose: CurrentNonFlatLayerOrderViewPoseRequestV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExactRationalDtoV1 {
    sign: &'static str,
    numerator_magnitude_hex: String,
    denominator_magnitude_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExactPointDtoV1 {
    u: ExactRationalDtoV1,
    v: ExactRationalDtoV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExactAffineDtoV1 {
    m00: ExactRationalDtoV1,
    m01: ExactRationalDtoV1,
    m10: ExactRationalDtoV1,
    m11: ExactRationalDtoV1,
    tx: ExactRationalDtoV1,
    ty: ExactRationalDtoV1,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FaceProjectionDtoV1 {
    dropped_world_axis: &'static str,
    plane_axes: [&'static str; 2],
    source_to_plane_projection_exact: ExactAffineDtoV1,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderFaceDtoV1 {
    face_id: String,
    face_key_sha256: String,
    world_outer_boundary_xyz_mm: Vec<[f64; 3]>,
    projection: FaceProjectionDtoV1,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CellProjectionDtoV1 {
    dropped_world_axis: &'static str,
    plane_axes: [&'static str; 2],
    rounded_boundary_uv_mm: Vec<[f64; 2]>,
    exact_boundary_uv: Vec<ExactPointDtoV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderCellDtoV1 {
    cell_key_sha256: String,
    exact_boundary_sha256: String,
    lower_face_id: String,
    upper_face_id: String,
    projection: CellProjectionDtoV1,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderPoseDtoV1 {
    model_id: &'static str,
    generation: String,
    fixed_face_id: String,
    hinge_angles: Vec<CurrentNonFlatLayerOrderHingeAngleDtoV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderHingeAngleDtoV1 {
    edge_id: String,
    angle_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderWorkDtoV1 {
    tested_face_pairs: usize,
    material_face_count: usize,
    source_overlap_cells_authenticated: usize,
    overlap_cell_count: usize,
    face_pair_order_count: usize,
    world_boundary_point_count: usize,
    exact_boundary_point_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentNonFlatLayerOrderViewResponseV1 {
    version: u8,
    model_id: &'static str,
    project_instance_id: String,
    project_id: String,
    revision: u64,
    fold_model_fingerprint_sha256: String,
    pose: CurrentNonFlatLayerOrderPoseDtoV1,
    faces: Vec<CurrentNonFlatLayerOrderFaceDtoV1>,
    cells: Vec<CurrentNonFlatLayerOrderCellDtoV1>,
    work: CurrentNonFlatLayerOrderWorkDtoV1,
    read_only: bool,
    authorizes_project_mutation: bool,
}

/// Which issuer minted the live applied pose.
///
/// A revalidated view can expose both a tree and a closed-graph projection, so
/// the semantic pose model ID is the only authority on the issuer kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PoseIssuerKindV1 {
    Tree,
    Graph,
}

impl PoseIssuerKindV1 {
    const fn model_id(self) -> &'static str {
        match self {
            Self::Tree => TREE_POSE_MODEL_ID_V1,
            Self::Graph => GRAPH_POSE_MODEL_ID_V1,
        }
    }
}

fn issuer_kind(
    view: &CurrentAppliedPoseView<'_>,
) -> Result<PoseIssuerKindV1, CurrentNonFlatLayerOrderViewErrorV1> {
    let model_id = view.semantic_pose().model_id();
    if model_id == TREE_POSE_MODEL_ID_V1 && view.tree().is_some() {
        return Ok(PoseIssuerKindV1::Tree);
    }
    if model_id == GRAPH_POSE_MODEL_ID_V1 && view.graph().is_some() {
        return Ok(PoseIssuerKindV1::Graph);
    }
    Err(CurrentNonFlatLayerOrderViewErrorV1::internal())
}

/// Canonicalizes a wire/hash copy of one finite scalar.
///
/// Only the copy is normalized; the proof and the computation source keep
/// their original values.
fn canonical_finite(value: f64) -> Result<f64, CurrentNonFlatLayerOrderViewErrorV1> {
    if !value.is_finite() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn axis_tag(
    axis: u8,
) -> Result<(&'static str, [&'static str; 2]), CurrentNonFlatLayerOrderViewErrorV1> {
    match axis {
        0 => Ok(("x", ["y", "z"])),
        1 => Ok(("y", ["x", "z"])),
        2 => Ok(("z", ["x", "y"])),
        _ => Err(CurrentNonFlatLayerOrderViewErrorV1::invalid()),
    }
}

fn axis_index(axis: u8) -> Result<u8, CurrentNonFlatLayerOrderViewErrorV1> {
    if axis > 2 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok(axis)
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn frame_count(hasher: &mut Sha256, count: usize) {
    hasher.update((count as u64).to_be_bytes());
}

fn frame_f64(hasher: &mut Sha256, value: f64) {
    hasher.update(value.to_bits().to_be_bytes());
}

/// Converts one exact rational into its lossless wire form.
///
/// The canonical magnitude bytes are preserved; no decimal, scientific, or
/// two's-complement encoding is produced.
fn exact_rational_dto(
    value: &ExactRationalValue,
    magnitude_bytes: &mut usize,
) -> Result<ExactRationalDtoV1, CurrentNonFlatLayerOrderViewErrorV1> {
    let sign = match value.sign {
        ExactSign::Negative => "negative",
        ExactSign::Zero => "zero",
        ExactSign::Positive => "positive",
    };
    let numerator = &value.numerator_magnitude_be;
    let denominator = &value.denominator_be;
    let zero_sign = matches!(value.sign, ExactSign::Zero);
    // A zero rational has exactly one canonical form: an empty numerator over
    // the single denominator byte `0x01`.
    if denominator.is_empty()
        || denominator.iter().all(|byte| *byte == 0)
        || denominator[0] == 0
        || (zero_sign && denominator.as_slice() != [0x01])
        || zero_sign != numerator.is_empty()
        || numerator.first().is_some_and(|byte| *byte == 0)
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    let magnitude = numerator
        .len()
        .checked_add(denominator.len())
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
    *magnitude_bytes =
        accumulate_bounded_total_v1(*magnitude_bytes, magnitude, MAX_EXACT_MAGNITUDE_BYTES_V1)?;
    Ok(ExactRationalDtoV1 {
        sign,
        numerator_magnitude_hex: hex_lower(numerator),
        denominator_magnitude_hex: hex_lower(denominator),
    })
}

/// Frames one exact rational for hashing.
///
/// The preimage uses the canonical raw magnitude bytes, never their hex
/// rendering, so the digest is independent of the wire encoding.
fn frame_exact(hasher: &mut Sha256, value: &ExactRationalValue) {
    let sign_tag: u8 = match value.sign {
        ExactSign::Negative => 0,
        ExactSign::Zero => 1,
        ExactSign::Positive => 2,
    };
    hasher.update([sign_tag]);
    frame(hasher, &value.numerator_magnitude_be);
    frame(hasher, &value.denominator_be);
}

/// Rejoins the current project, applied pose, and non-flat evidence.
///
/// `Ok(None)` means the project genuinely owns no non-flat evidence. Every
/// other failure is a data-free error; a broken rejoin is never softened into
/// an absence.
#[tauri::command]
pub(super) fn get_current_non_flat_layer_order_view_v1(
    app_state: State<'_, AppState>,
    request: CurrentNonFlatLayerOrderViewRequestV1,
) -> Result<Option<CurrentNonFlatLayerOrderViewResponseV1>, CurrentNonFlatLayerOrderViewErrorV1> {
    if request.version != 1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    let project =
        lock_project(&app_state).map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
    build_current_non_flat_layer_order_view_v1(&project, &request)
}

fn build_current_non_flat_layer_order_view_v1(
    project: &ProjectState,
    request: &CurrentNonFlatLayerOrderViewRequestV1,
) -> Result<Option<CurrentNonFlatLayerOrderViewResponseV1>, CurrentNonFlatLayerOrderViewErrorV1> {
    let project_instance_id = wire_id(&project.instance_id)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
    let project_id = wire_id(&project.project_id)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
    let revision = project.editor.revision();
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || revision != request.expected_revision
        || fingerprint != request.expected_fold_model_fingerprint_sha256
        || request.expected_fold_model_fingerprint_sha256.len() != 64
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }

    let proof = match project.current_layer_evidence.as_ref() {
        None | Some(CurrentLayerEvidence::CertifiedFlat(_)) => return Ok(None),
        Some(CurrentLayerEvidence::NonFlat(proof)) => proof,
    };
    if proof.authorizes_apply_stacked_fold() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::internal());
    }
    // The proof must describe this exact project, revision, and fold model.
    if proof.model_id() != ori_core::STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if proof.identity_namespace() != project.project_id
        || proof.target_revision() != revision
        || proof.target_fingerprint().to_hex() != fingerprint
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    // Cheap, allocation-free count preflight runs before the shared structural
    // validator, which builds hash sets over every face, cell, and pair.
    preflight_view_resources(proof)?;

    let capability = capture_current_applied_pose_capability(project)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::stale())?
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::stale)?;
    let generation = capability.generation();
    if generation == 0 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    let view = revalidate_current_applied_pose_capability(project, &capability)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::stale())?
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::stale)?;

    let pose = build_pose_dto(proof, &view, generation, request)?;
    validate_non_flat_layer_order_structure_v1(proof)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::invalid())?;
    // One aggregate accumulator: the 8 MiB ceiling covers the whole response.
    let mut magnitude_bytes = 0usize;
    let (faces, world_points) = build_faces(proof, &view, &mut magnitude_bytes)?;
    let (cells, exact_points) = build_cells(proof, &faces, &mut magnitude_bytes)?;
    let response = CurrentNonFlatLayerOrderViewResponseV1 {
        version: 1,
        model_id: CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1,
        project_instance_id,
        project_id,
        revision,
        fold_model_fingerprint_sha256: fingerprint,
        pose,
        work: CurrentNonFlatLayerOrderWorkDtoV1 {
            tested_face_pairs: proof.tested_face_pairs(),
            material_face_count: faces.len(),
            source_overlap_cells_authenticated: proof.source_overlap_cells_authenticated(),
            overlap_cell_count: cells.len(),
            face_pair_order_count: cells.len(),
            world_boundary_point_count: world_points,
            exact_boundary_point_count: exact_points,
        },
        faces,
        cells,
        read_only: true,
        authorizes_project_mutation: false,
    };
    verify_response_invariants(proof, &response)?;
    let bytes = serde_json::to_vec(&response)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
    validate_serialized_json_bytes_v1(bytes.len())?;
    Ok(Some(response))
}

/// Numeric projection of one proof's bounded work.
///
/// The projection borrows nothing and owns no authority: it is a plain copy of
/// the counts the viewer must bound. Production builds it once from the proof
/// slices, and the validator below is the only place those numbers are judged,
/// so the boundary values can be exercised without forging a proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViewResourceCountsV1 {
    material_faces: usize,
    folded_faces: usize,
    hinges: usize,
    declared_cells: usize,
    actual_cells: usize,
    declared_pairs: usize,
    actual_pairs: usize,
    tested_face_pairs: usize,
    source_overlap_cells_authenticated: usize,
}

impl ViewResourceCountsV1 {
    /// Projects the bounded counts of one proof without allocating.
    fn from_proof(proof: &StackedFoldNonFlatLayerOrderV1) -> Self {
        Self {
            material_faces: proof.material_faces().len(),
            folded_faces: proof.folded_faces().len(),
            hinges: proof.hinge_angles().len(),
            declared_cells: proof.overlap_cell_count(),
            actual_cells: proof.overlap_cells().len(),
            declared_pairs: proof.face_pair_order_count(),
            actual_pairs: proof.face_pair_orders().len(),
            tested_face_pairs: proof.tested_face_pairs(),
            source_overlap_cells_authenticated: proof.source_overlap_cells_authenticated(),
        }
    }
}

/// Judges the projected counts.
///
/// A cap overrun or a checked overflow is a resource failure; a count that
/// contradicts another count is invalid evidence, never a resource limit.
fn validate_view_resource_counts_v1(
    counts: ViewResourceCountsV1,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if !(1..=MAX_FACES_V1).contains(&counts.material_faces) {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    if counts.folded_faces != counts.material_faces {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if !(1..=MAX_HINGES_V1).contains(&counts.hinges) {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    if counts.actual_cells > MAX_CELLS_V1 || counts.declared_cells > MAX_CELLS_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    if counts.actual_pairs > MAX_FACE_PAIR_ORDERS_V1
        || counts.declared_pairs > MAX_FACE_PAIR_ORDERS_V1
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    if counts.declared_cells != counts.actual_cells
        || counts.actual_pairs != counts.actual_cells
        || counts.declared_pairs != counts.actual_cells
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    for work in [
        counts.tested_face_pairs,
        counts.source_overlap_cells_authenticated,
    ] {
        validate_safe_wire_integer_v1(work)?;
    }
    Ok(())
}

/// Bounds one cell boundary: equal rounded/exact counts inside the cap.
fn validate_cell_boundary_counts_v1(
    rounded: usize,
    exact: usize,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if rounded != exact || exact < 3 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if exact > MAX_CELL_POLYGON_POINTS_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    Ok(())
}

/// Bounds one world polygon by its already known vertex count.
fn validate_world_polygon_count_v1(
    vertices: usize,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if vertices < 3 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if vertices > MAX_WORLD_POLYGON_POINTS_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    Ok(())
}

/// Adds one bounded term to a running total, refusing overflow and overrun.
fn accumulate_bounded_total_v1(
    total: usize,
    term: usize,
    maximum: usize,
) -> Result<usize, CurrentNonFlatLayerOrderViewErrorV1> {
    let sum = total
        .checked_add(term)
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
    if sum > maximum {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    Ok(sum)
}

/// Every wire count must stay lossless in both `u64` and JSON.
fn validate_safe_wire_integer_v1(value: usize) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if u64::try_from(value).is_err() || value > MAX_SAFE_WIRE_INTEGER_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    Ok(())
}

/// The serialized response must fit the transport ceiling.
fn validate_serialized_json_bytes_v1(
    bytes: usize,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if bytes > MAX_SERIALIZED_JSON_BYTES_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    Ok(())
}

/// Cheap viewer-owned count preflight.
///
/// Only borrowed slice lengths and checked sums are used; no hash set, clone,
/// or response buffer is built here, so an oversized proof is refused before
/// any heavy work runs.
fn preflight_view_resources(
    proof: &StackedFoldNonFlatLayerOrderV1,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    let counts = ViewResourceCountsV1::from_proof(proof);
    validate_view_resource_counts_v1(counts)?;
    let mut boundary_points = 0usize;
    for cell in proof.overlap_cells() {
        let exact = cell.exact_boundary().len();
        validate_cell_boundary_counts_v1(cell.boundary().len(), exact)?;
        boundary_points = accumulate_bounded_total_v1(
            boundary_points,
            exact,
            MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1,
        )?;
    }
    preflight_non_flat_cell_transport_v1(
        counts.material_faces,
        counts.actual_cells,
        counts.actual_pairs,
        boundary_points,
        NonFlatCellTransportLimitsV1 {
            max_faces: MAX_FACES_V1,
            max_cells: MAX_CELLS_V1,
            max_pairs: MAX_FACE_PAIR_ORDERS_V1,
            max_boundary_points: MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1,
        },
    )
    .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::resource())?;
    Ok(())
}

fn build_pose_dto(
    proof: &StackedFoldNonFlatLayerOrderV1,
    view: &CurrentAppliedPoseView<'_>,
    generation: u64,
    request: &CurrentNonFlatLayerOrderViewRequestV1,
) -> Result<CurrentNonFlatLayerOrderPoseDtoV1, CurrentNonFlatLayerOrderViewErrorV1> {
    let fixed = proof
        .fixed_face()
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
    let fixed_face_id =
        wire_id(&fixed).map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
    if fixed != request.expected_applied_pose.fixed_face_id {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    validate_request_hinge_vector(&request.expected_applied_pose.hinge_angles)?;
    let semantic = view.semantic_pose();
    if semantic.fixed_face() != Some(fixed) {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    // The response pose model ID follows the issuer kind; a graph pose never
    // carries the tree model ID.
    let pose_model_id = issuer_kind(view)?.model_id();
    if proof.hinge_angles().len() != request.expected_applied_pose.hinge_angles.len() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    // The request is compared against the proof by raw bits. A request `-0.0`
    // is never canonicalized into `+0.0` before the comparison.
    for (proof_angle, requested) in proof
        .hinge_angles()
        .iter()
        .zip(&request.expected_applied_pose.hinge_angles)
    {
        if proof_angle.edge() != requested.edge_id
            || proof_angle.angle_degrees().to_bits() != requested.angle_degrees.to_bits()
        {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
        }
    }
    let mut hinge_angles = Vec::new();
    hinge_angles
        .try_reserve_exact(proof.hinge_angles().len())
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::resource())?;
    for angle in proof.hinge_angles() {
        let edge_id =
            wire_id(&angle.edge()).map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
        hinge_angles.push(CurrentNonFlatLayerOrderHingeAngleDtoV1 {
            edge_id,
            angle_degrees: canonical_finite(angle.angle_degrees())?,
        });
    }
    if hinge_angles
        .windows(2)
        .any(|pair| pair[0].edge_id >= pair[1].edge_id)
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    // The revalidated semantic pose must carry the same hinge vector, compared
    // by edge ID, length, canonical order, and exact bits.
    let semantic_angles = semantic.hinge_angles();
    if semantic_angles.len() != proof.hinge_angles().len() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    for (proof_angle, applied) in proof.hinge_angles().iter().zip(semantic_angles) {
        if proof_angle.edge() != applied.edge()
            || proof_angle.angle_degrees().to_bits() != applied.angle_degrees().to_bits()
        {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
        }
    }
    if hinge_angles
        .iter()
        .all(|angle| angle.angle_degrees == 0.0 || angle.angle_degrees == 180.0)
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok(CurrentNonFlatLayerOrderPoseDtoV1 {
        model_id: pose_model_id,
        generation: generation.to_string(),
        fixed_face_id,
        hinge_angles,
    })
}

/// Validates the requested hinge vector on its own terms.
///
/// The request must already be canonical: bounded, sorted, duplicate-free,
/// finite, in range, free of negative zero, and not entirely flat.
fn validate_request_hinge_vector(
    angles: &[CurrentNonFlatLayerOrderViewHingeAngleRequestV1],
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if !(1..=MAX_HINGES_V1).contains(&angles.len()) {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    let mut previous: Option<[u8; 16]> = None;
    let mut non_flat = false;
    for angle in angles {
        let bits = angle.angle_degrees.to_bits();
        if !angle.angle_degrees.is_finite()
            || bits == (-0.0f64).to_bits()
            || angle.angle_degrees < 0.0
            || angle.angle_degrees > 180.0
        {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
        }
        if angle.angle_degrees != 0.0 && angle.angle_degrees != 180.0 {
            non_flat = true;
        }
        let key = angle.edge_id.canonical_bytes();
        if previous.is_some_and(|last| last >= key) {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
        }
        previous = Some(key);
    }
    if non_flat {
        Ok(())
    } else {
        Err(CurrentNonFlatLayerOrderViewErrorV1::stale())
    }
}

fn build_faces(
    proof: &StackedFoldNonFlatLayerOrderV1,
    view: &CurrentAppliedPoseView<'_>,
    magnitude_bytes: &mut usize,
) -> Result<(Vec<CurrentNonFlatLayerOrderFaceDtoV1>, usize), CurrentNonFlatLayerOrderViewErrorV1> {
    let mut face_ids = proof
        .material_faces()
        .iter()
        .map(|face| face.face_id)
        .collect::<Vec<_>>();
    face_ids.sort_unstable_by_key(FaceId::canonical_bytes);
    if face_ids.is_empty() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if face_ids.len() > MAX_FACES_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    if face_ids
        .windows(2)
        .any(|pair| pair[0].canonical_bytes() == pair[1].canonical_bytes())
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    // The proof material faces and the live model faces must be the same set.
    // The live registry is only borrowed until its length is known, and a
    // duplicate live face is refused instead of being collapsed away.
    let live: &[FaceId] = match issuer_kind(view)? {
        PoseIssuerKindV1::Tree => view
            .tree()
            .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::internal)?
            .0
            .face_ids(),
        PoseIssuerKindV1::Graph => view
            .graph()
            .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::internal)?
            .0
            .face_ids(),
    };
    validate_live_face_registry_v1(&face_ids, live)?;
    validate_live_face_registry_v1(&face_ids, view.semantic_pose().face_ids())?;
    let mut total_points = 0usize;
    let mut faces = Vec::with_capacity(face_ids.len());
    for face_id in face_ids {
        let folded = proof
            .folded_faces()
            .iter()
            .find(|folded| folded.face().face_id == face_id)
            .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
        let world = world_boundary(view, face_id)?;
        total_points = accumulate_bounded_total_v1(
            total_points,
            world.len(),
            MAX_TOTAL_WORLD_BOUNDARY_POINTS_V1,
        )?;
        let axis = axis_index(folded.dropped_world_axis())?;
        let (dropped, plane) = axis_tag(axis)?;
        let transform = folded.source_to_plane();
        let exact = ExactAffineDtoV1 {
            m00: exact_rational_dto(&transform.m00, magnitude_bytes)?,
            m01: exact_rational_dto(&transform.m01, magnitude_bytes)?,
            m10: exact_rational_dto(&transform.m10, magnitude_bytes)?,
            m11: exact_rational_dto(&transform.m11, magnitude_bytes)?,
            tx: exact_rational_dto(&transform.tx, magnitude_bytes)?,
            ty: exact_rational_dto(&transform.ty, magnitude_bytes)?,
        };
        let wire_face_id =
            wire_id(&face_id).map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
        // The raw domain separator opens the preimage; only variable-length
        // fields carry a `u64` big-endian length prefix.
        let mut hasher = Sha256::new();
        hasher.update(FACE_DOMAIN_V1);
        frame(&mut hasher, wire_face_id.as_bytes());
        frame_count(&mut hasher, world.len());
        for point in &world {
            for value in point {
                frame_f64(&mut hasher, *value);
            }
        }
        hasher.update([axis]);
        for tag in plane {
            frame(&mut hasher, tag.as_bytes());
        }
        for value in [
            &transform.m00,
            &transform.m01,
            &transform.m10,
            &transform.m11,
            &transform.tx,
            &transform.ty,
        ] {
            frame_exact(&mut hasher, value);
        }
        faces.push(CurrentNonFlatLayerOrderFaceDtoV1 {
            face_id: wire_face_id,
            face_key_sha256: hex_lower(&hasher.finalize()),
            world_outer_boundary_xyz_mm: world,
            projection: FaceProjectionDtoV1 {
                dropped_world_axis: dropped,
                plane_axes: plane,
                source_to_plane_projection_exact: exact,
            },
        });
    }
    Ok((faces, total_points))
}

/// Compares the proof material faces with the live model registry.
///
/// The live slice is only borrowed until its length is known, so an unbounded
/// copy never happens. A duplicate is refused instead of being collapsed away:
/// `dedup` would silently accept `[A, B, B]` against `[A, B]`.
fn validate_live_face_registry_v1(
    proof_faces: &[FaceId],
    live: &[FaceId],
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if live.len() != proof_faces.len() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    let mut live_face_ids: Vec<FaceId> = Vec::new();
    live_face_ids
        .try_reserve_exact(live.len())
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::resource())?;
    live_face_ids.extend_from_slice(live);
    live_face_ids.sort_unstable_by_key(FaceId::canonical_bytes);
    if live_face_ids
        .windows(2)
        .any(|pair| pair[0].canonical_bytes() == pair[1].canonical_bytes())
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if live_face_ids != proof_faces {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok(())
}

/// Resolves the directed face pair of one overlap cell.
///
/// An unknown face, an equal pair, and a pair whose two faces disagree on the
/// dropped world axis are all invalid evidence.
fn resolve_cell_face_pair_v1<'a>(
    faces: &'a [CurrentNonFlatLayerOrderFaceDtoV1],
    lower_face_id: &str,
    upper_face_id: &str,
) -> Result<
    (
        &'a CurrentNonFlatLayerOrderFaceDtoV1,
        &'a CurrentNonFlatLayerOrderFaceDtoV1,
    ),
    CurrentNonFlatLayerOrderViewErrorV1,
> {
    if lower_face_id == upper_face_id {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    let lower = faces
        .iter()
        .find(|face| face.face_id == lower_face_id)
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
    let upper = faces
        .iter()
        .find(|face| face.face_id == upper_face_id)
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
    if lower.projection.dropped_world_axis != upper.projection.dropped_world_axis {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok((lower, upper))
}

/// Re-derives the plane axes from a dropped-axis tag and refuses a mismatch.
fn validate_axis_derivation_v1(
    dropped: &str,
    plane: [&str; 2],
) -> Result<u8, CurrentNonFlatLayerOrderViewErrorV1> {
    let axis: u8 = match dropped {
        "x" => 0,
        "y" => 1,
        "z" => 2,
        _ => return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid()),
    };
    let (expected_dropped, expected_plane) = axis_tag(axis)?;
    if expected_dropped != dropped || expected_plane != plane {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok(axis)
}

fn world_boundary(
    view: &CurrentAppliedPoseView<'_>,
    face_id: FaceId,
) -> Result<Vec<[f64; 3]>, CurrentNonFlatLayerOrderViewErrorV1> {
    // The known vertex count is checked before anything is allocated, and the
    // output buffer is reserved fallibly. Canonical walk order is preserved:
    // no reverse, rotate, or dedup.
    match issuer_kind(view)? {
        PoseIssuerKindV1::Tree => {
            let (model, pose) = view
                .tree()
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::internal)?;
            let boundary = model
                .face_boundary(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            let vertices = boundary.vertices();
            let mut points = reserved_world_points(vertices.len())?;
            let transform = pose
                .face_transform(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            for vertex in vertices {
                let source = pose
                    .vertex_position(*vertex)
                    .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
                let world = transform
                    .apply_point(source)
                    .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::invalid())?;
                points.push([
                    canonical_finite(world.x())?,
                    canonical_finite(world.y())?,
                    canonical_finite(world.z())?,
                ]);
            }
            Ok(points)
        }
        PoseIssuerKindV1::Graph => {
            let (geometry, _audit, pose) = view
                .graph()
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::internal)?;
            let vertices = geometry
                .face_boundary_vertices(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            let mut points = reserved_world_points(vertices.len())?;
            let transform = pose
                .face_transform(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            for vertex in vertices {
                let source = geometry
                    .vertex_position(*vertex)
                    .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
                let world = transform
                    .apply_point(source)
                    .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::invalid())?;
                points.push([
                    canonical_finite(world.x())?,
                    canonical_finite(world.y())?,
                    canonical_finite(world.z())?,
                ]);
            }
            Ok(points)
        }
    }
}

/// Checks the per-polygon cap and reserves the output buffer fallibly.
fn reserved_world_points(
    vertices: usize,
) -> Result<Vec<[f64; 3]>, CurrentNonFlatLayerOrderViewErrorV1> {
    validate_world_polygon_count_v1(vertices)?;
    let mut points: Vec<[f64; 3]> = Vec::new();
    points
        .try_reserve_exact(vertices)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::resource())?;
    Ok(points)
}

fn build_cells(
    proof: &StackedFoldNonFlatLayerOrderV1,
    faces: &[CurrentNonFlatLayerOrderFaceDtoV1],
    magnitude_bytes: &mut usize,
) -> Result<(Vec<CurrentNonFlatLayerOrderCellDtoV1>, usize), CurrentNonFlatLayerOrderViewErrorV1> {
    let mut total_points = 0usize;
    let mut cells = Vec::with_capacity(proof.overlap_cells().len());
    for (cell, pair) in proof.overlap_cells().iter().zip(proof.face_pair_orders()) {
        if cell.lower_face() != pair.lower_face() || cell.upper_face() != pair.upper_face() {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
        }
        let lower_face_id = wire_id(&cell.lower_face())
            .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
        let upper_face_id = wire_id(&cell.upper_face())
            .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
        let (lower, _upper) = resolve_cell_face_pair_v1(faces, &lower_face_id, &upper_face_id)?;
        let dropped = lower.projection.dropped_world_axis;
        let plane = lower.projection.plane_axes;
        let axis_byte = validate_axis_derivation_v1(dropped, plane)?;
        validate_cell_boundary_counts_v1(cell.boundary().len(), cell.exact_boundary().len())?;
        total_points = accumulate_bounded_total_v1(
            total_points,
            cell.exact_boundary().len(),
            MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1,
        )?;
        let mut rounded = Vec::with_capacity(cell.boundary().len());
        let mut exact = Vec::with_capacity(cell.exact_boundary().len());
        for (point, value) in cell.boundary().iter().zip(cell.exact_boundary()) {
            let u = value
                .x
                .to_f64()
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            let v = value
                .y
                .to_f64()
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            if u.to_bits() != point.x.to_bits() || v.to_bits() != point.y.to_bits() {
                return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
            }
            rounded.push([canonical_finite(point.x)?, canonical_finite(point.y)?]);
            exact.push(ExactPointDtoV1 {
                u: exact_rational_dto(&value.x, magnitude_bytes)?,
                v: exact_rational_dto(&value.y, magnitude_bytes)?,
            });
        }
        let mut boundary_hasher = Sha256::new();
        boundary_hasher.update(EXACT_BOUNDARY_DOMAIN_V1);
        boundary_hasher.update([axis_byte]);
        for tag in plane {
            frame(&mut boundary_hasher, tag.as_bytes());
        }
        frame_count(&mut boundary_hasher, exact.len());
        for value in cell.exact_boundary() {
            frame_exact(&mut boundary_hasher, &value.x);
            frame_exact(&mut boundary_hasher, &value.y);
        }
        let exact_digest = boundary_hasher.finalize();
        let mut cell_hasher = Sha256::new();
        cell_hasher.update(CELL_DOMAIN_V1);
        frame(&mut cell_hasher, lower_face_id.as_bytes());
        frame(&mut cell_hasher, upper_face_id.as_bytes());
        // The nested digest is a fixed 32-byte field, so it needs no prefix.
        cell_hasher.update(exact_digest);
        cells.push(CurrentNonFlatLayerOrderCellDtoV1 {
            cell_key_sha256: hex_lower(&cell_hasher.finalize()),
            exact_boundary_sha256: hex_lower(&exact_digest),
            lower_face_id,
            upper_face_id,
            projection: CellProjectionDtoV1 {
                dropped_world_axis: dropped,
                plane_axes: plane,
                rounded_boundary_uv_mm: rounded,
                exact_boundary_uv: exact,
            },
        });
    }
    cells.sort_by(|left, right| left.cell_key_sha256.cmp(&right.cell_key_sha256));
    if cells
        .windows(2)
        .any(|pair| pair[0].cell_key_sha256 == pair[1].cell_key_sha256)
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    Ok((cells, total_points))
}

/// Final re-verification of every count, ordering, and authority literal.
fn verify_response_invariants(
    proof: &StackedFoldNonFlatLayerOrderV1,
    response: &CurrentNonFlatLayerOrderViewResponseV1,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    if !response.read_only || response.authorizes_project_mutation {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::internal());
    }
    if response.work.material_face_count != response.faces.len()
        || response.work.overlap_cell_count != response.cells.len()
        || response.work.face_pair_order_count != response.cells.len()
        || response.faces.len() != proof.material_faces().len()
        || response.cells.len() != proof.overlap_cell_count()
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    let world_points = response
        .faces
        .iter()
        .try_fold(0usize, |sum, face| {
            sum.checked_add(face.world_outer_boundary_xyz_mm.len())
        })
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
    let exact_points = response
        .cells
        .iter()
        .try_fold(0usize, |sum, cell| {
            sum.checked_add(cell.projection.exact_boundary_uv.len())
        })
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
    if world_points != response.work.world_boundary_point_count
        || exact_points != response.work.exact_boundary_point_count
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    if response
        .faces
        .windows(2)
        .any(|pair| pair[0].face_id >= pair[1].face_id)
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    for cell in &response.cells {
        if cell.projection.rounded_boundary_uv_mm.len() != cell.projection.exact_boundary_uv.len() {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
        }
    }
    if response.generation_is_canonical() {
        Ok(())
    } else {
        Err(CurrentNonFlatLayerOrderViewErrorV1::internal())
    }
}

impl CurrentNonFlatLayerOrderViewResponseV1 {
    fn generation_is_canonical(&self) -> bool {
        let value = &self.pose.generation;
        !value.is_empty()
            && value != "0"
            && !value.starts_with('0')
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && value.parse::<u64>().is_ok()
    }
}

/// The shared dense-grid cycle fixture, included for the graph issuer test.
///
/// Another module already includes the same file. Both inclusions are test-only
/// and neither can reference the other, so the duplicate is deliberate.
#[cfg(test)]
#[allow(clippy::duplicate_mod)]
#[path = "../../../../test-support/dense_grid_cycle.rs"]
mod viewer_dense_grid_cycle_test_support;

#[cfg(test)]
mod tests {
    use super::*;
    use ori_kinematics::{CanonicalHingeAngles, HingeAngle};

    /// Digest vectors computed outside this crate from the V1 byte contract.
    ///
    /// The expected values are literals, never produced by the production
    /// helper under test.
    const FACE_DIGEST_VECTOR_V1: &str =
        "309cb86e2e3f08c119aa03fcc6f237c701afdcd2ffe1e51eba2799fb7241d0d5";
    const EXACT_BOUNDARY_DIGEST_VECTOR_V1: &str =
        "818f5643b6e3078f0b902bbfc328ab77f4dc47902637e6bf2d85776a4ae7567c";
    const CELL_DIGEST_VECTOR_V1: &str =
        "25376bc0687be46f491d0d368a1c3a36e20b21d1cfcc60fa480e68fe2b64739f";

    /// A two-face material tree: one hexagonal sheet split by a single crease.
    fn centered_single_hinge_project() -> ProjectState {
        let positions = [
            (0.0, 0.0),
            (200.0, 0.0),
            (400.0, 0.0),
            (400.0, 400.0),
            (200.0, 400.0),
            (0.0, 400.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|(x, y)| ori_domain::Vertex {
                id: ori_domain::VertexId::new(),
                position: ori_domain::Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| ori_domain::Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: ori_domain::EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(ori_domain::Edge {
            id: EdgeId::new(),
            start: vertices[1].id,
            end: vertices[4].id,
            kind: ori_domain::EdgeKind::Mountain,
        });
        let paper = ori_domain::Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..ori_domain::Paper::default()
        };
        ProjectState::new_with_paper(ori_domain::CreasePattern { vertices, edges }, paper)
    }

    /// Builds a project whose current evidence is a freshly solved non-flat
    /// layer order after one authenticated stacked-fold document transition.
    fn non_flat_tree_project(angle_degrees: f64) -> ProjectState {
        let target = centered_single_hinge_project();
        let target_pattern = target.editor.pattern().clone();
        let target_paper = target.editor.paper().clone();
        let target_hinge = target_pattern
            .edges
            .iter()
            .find(|edge| edge.kind == ori_domain::EdgeKind::Mountain)
            .expect("one target hinge")
            .id;
        let mut source_pattern = target_pattern.clone();
        source_pattern.edges.retain(|edge| edge.id != target_hinge);
        let mut project = ProjectState::new_with_paper(source_pattern, target_paper.clone());
        let target_editor =
            ori_core::EditorState::with_paper(target_pattern.clone(), target_paper.clone());
        let topology = target_editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let simulation = topology
            .simulation_snapshot()
            .expect("a simulation snapshot");
        let mut faces = simulation
            .faces
            .iter()
            .map(|face| face.id)
            .collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let fixed = faces[0];
        let mut hinges = simulation
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = CanonicalHingeAngles::new(
            hinges
                .iter()
                .map(|edge| HingeAngle::new(*edge, angle_degrees).unwrap())
                .collect::<Vec<_>>(),
        )
        .expect("canonical hinge angles");
        let applied_pose = ori_core::prepare_applied_pose_v1(
            &faces,
            &hinges,
            Some(fixed),
            &angles
                .as_slice()
                .iter()
                .map(|angle| (angle.edge(), angle.angle_degrees()))
                .collect::<Vec<_>>(),
            ori_core::AppliedPoseLimitsV1::default(),
        )
        .expect("complete target semantic pose");
        let timeline = ori_domain::InstructionTimeline {
            steps: vec![ori_domain::InstructionStep {
                id: ori_domain::InstructionStepId::new(),
                title: "Stacked fold".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: ori_domain::MIN_INSTRUCTION_DURATION_MS,
                visual: ori_domain::InstructionVisual::default(),
                pose: ori_domain::InstructionPose {
                    model: ori_domain::InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: target_editor.fold_model_fingerprint_v1(),
                    fixed_face: Some(fixed),
                    hinge_angles: angles
                        .as_slice()
                        .iter()
                        .map(|angle| ori_domain::InstructionHingeAngle {
                            edge: angle.edge(),
                            angle_degrees: angle.angle_degrees(),
                        })
                        .collect(),
                },
            }],
        };
        project
            .editor
            .execute_stacked_fold_document(
                project.editor.revision(),
                target_pattern,
                target_paper,
                timeline,
                ori_domain::ProjectLayerDocumentV1::default(),
                applied_pose,
            )
            .expect("one authenticated stacked-fold history entry");
        crate::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
            &mut project,
            hinges,
            fixed,
            angle_degrees,
        );
        let flat = crate::global_flat_foldability::reanalyze_current_flat_layer_order(&project)
            .expect("the current flat layer order");
        let proof = ori_core::revalidate_current_non_flat_layer_order_v1(
            project.project_id,
            project.editor.revision(),
            project.editor.pattern(),
            project.editor.paper(),
            Some(fixed),
            &angles,
            &flat,
            ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
        )
        .expect("a freshly solved non-flat layer order");
        project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(proof));
        let capability = capture_current_applied_pose_capability(&project)
            .expect("capture native pose authority")
            .expect("native pose authority exists");
        let current = revalidate_current_applied_pose_capability(&project, &capability)
            .expect("revalidate native pose authority")
            .expect("native pose authority remains current");
        assert!(current.tree().is_some());
        let proof = match project.current_layer_evidence.as_ref() {
            Some(CurrentLayerEvidence::NonFlat(proof)) => proof,
            _ => unreachable!(),
        };
        assert_eq!(
            current.semantic_pose(),
            project.editor.current_applied_pose().unwrap()
        );
        assert_eq!(current.semantic_pose().fixed_face(), proof.fixed_face());
        assert!(
            current
                .semantic_pose()
                .hinge_angles()
                .iter()
                .zip(proof.hinge_angles())
                .all(|(pose, proof)| {
                    pose.edge() == proof.edge()
                        && pose.angle_degrees().to_bits() == proof.angle_degrees().to_bits()
                })
        );
        project
    }

    /// Builds the request that the current project and pose actually satisfy.
    fn canonical_request(project: &ProjectState) -> CurrentNonFlatLayerOrderViewRequestV1 {
        let proof = match project.current_layer_evidence.as_ref() {
            Some(CurrentLayerEvidence::NonFlat(proof)) => proof,
            _ => panic!("the fixture must own non-flat evidence"),
        };
        let mut hinge_angles = proof
            .hinge_angles()
            .iter()
            .map(|angle| CurrentNonFlatLayerOrderViewHingeAngleRequestV1 {
                edge_id: angle.edge(),
                angle_degrees: angle.angle_degrees(),
            })
            .collect::<Vec<_>>();
        hinge_angles.sort_by_key(|angle| angle.edge_id.canonical_bytes());
        CurrentNonFlatLayerOrderViewRequestV1 {
            version: 1,
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            expected_fold_model_fingerprint_sha256: project.editor.fold_model_fingerprint_v1(),
            expected_applied_pose: CurrentNonFlatLayerOrderViewPoseRequestV1 {
                fixed_face_id: proof.fixed_face().expect("the proof fixes one face"),
                hinge_angles,
            },
        }
    }

    fn category(
        error: CurrentNonFlatLayerOrderViewErrorV1,
    ) -> CurrentNonFlatLayerOrderViewErrorCategoryV1 {
        error.category
    }

    fn view(project: &ProjectState) -> CurrentNonFlatLayerOrderViewResponseV1 {
        let request = canonical_request(project);
        build_current_non_flat_layer_order_view_v1(project, &request)
            .expect("the bound viewer request succeeds")
            .expect("the project owns non-flat evidence")
    }

    #[test]
    fn applied_non_flat_evidence_yields_a_read_only_view() {
        let project = non_flat_tree_project(90.0);
        let response = view(&project);
        assert!(response.read_only);
        assert!(!response.authorizes_project_mutation);
        assert_eq!(response.version, 1);
        assert_eq!(
            response.model_id,
            CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1
        );
        assert!(
            response.pose.model_id == TREE_POSE_MODEL_ID_V1
                || response.pose.model_id == GRAPH_POSE_MODEL_ID_V1
        );
        assert_eq!(response.work.material_face_count, response.faces.len());
        assert_eq!(response.work.overlap_cell_count, response.cells.len());
        assert_eq!(response.work.face_pair_order_count, response.cells.len());
        assert!(!response.faces.is_empty());
        assert!(
            response
                .faces
                .windows(2)
                .all(|pair| pair[0].face_id < pair[1].face_id)
        );
        assert!(
            response
                .cells
                .windows(2)
                .all(|pair| pair[0].cell_key_sha256 < pair[1].cell_key_sha256)
        );
        for face in &response.faces {
            assert_eq!(face.face_key_sha256.len(), 64);
            assert!(face.world_outer_boundary_xyz_mm.len() >= 3);
            assert!(
                face.world_outer_boundary_xyz_mm
                    .iter()
                    .all(|point| point.iter().all(|value| value.is_finite()))
            );
            let axis = face.projection.dropped_world_axis;
            assert!(["x", "y", "z"].contains(&axis));
            let expected = match axis {
                "x" => ["y", "z"],
                "y" => ["x", "z"],
                _ => ["x", "y"],
            };
            assert_eq!(face.projection.plane_axes, expected);
        }
        for cell in &response.cells {
            assert_ne!(cell.lower_face_id, cell.upper_face_id);
            assert_eq!(
                cell.projection.rounded_boundary_uv_mm.len(),
                cell.projection.exact_boundary_uv.len()
            );
            assert!(cell.projection.rounded_boundary_uv_mm.len() >= 3);
        }
    }

    #[test]
    fn the_pose_model_id_follows_the_live_issuer_kind() {
        let project = non_flat_tree_project(90.0);
        let response = view(&project);
        let capability = capture_current_applied_pose_capability(&project)
            .unwrap()
            .unwrap();
        let live = revalidate_current_applied_pose_capability(&project, &capability)
            .unwrap()
            .unwrap();
        // A revalidated view can expose both projections, so the semantic pose
        // model ID is the only authority on the issuer kind.
        let expected = live.semantic_pose().model_id();
        assert!(expected == TREE_POSE_MODEL_ID_V1 || expected == GRAPH_POSE_MODEL_ID_V1);
        assert_eq!(response.pose.model_id, expected);
        assert_eq!(issuer_kind(&live).unwrap().model_id(), expected);
    }

    #[test]
    fn the_same_snapshot_is_byte_identical_on_every_call() {
        let project = non_flat_tree_project(90.0);
        let first = view(&project);
        let second = view(&project);
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
    }

    #[test]
    fn the_exact_rational_wire_form_is_canonical() {
        let project = non_flat_tree_project(90.0);
        let response = view(&project);
        let mut rationals = Vec::new();
        for face in &response.faces {
            let affine = &face.projection.source_to_plane_projection_exact;
            rationals.extend([
                &affine.m00,
                &affine.m01,
                &affine.m10,
                &affine.m11,
                &affine.tx,
                &affine.ty,
            ]);
        }
        for cell in &response.cells {
            for point in &cell.projection.exact_boundary_uv {
                rationals.push(&point.u);
                rationals.push(&point.v);
            }
        }
        assert!(!rationals.is_empty());
        let mut zeros = 0usize;
        for value in rationals {
            assert!(["negative", "zero", "positive"].contains(&value.sign));
            assert_eq!(value.numerator_magnitude_hex.len() % 2, 0);
            assert_eq!(value.denominator_magnitude_hex.len() % 2, 0);
            for hex in [
                &value.numerator_magnitude_hex,
                &value.denominator_magnitude_hex,
            ] {
                assert!(
                    hex.bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                );
            }
            if value.sign == "zero" {
                zeros += 1;
                assert_eq!(value.numerator_magnitude_hex, "");
                assert_eq!(value.denominator_magnitude_hex, "01");
            } else {
                assert!(!value.numerator_magnitude_hex.is_empty());
                assert!(!value.numerator_magnitude_hex.starts_with("00"));
                assert!(!value.denominator_magnitude_hex.starts_with("00"));
            }
        }
        assert!(zeros > 0, "the fixture must exercise a zero rational");
    }

    #[test]
    fn a_zero_rational_with_a_foreign_denominator_is_refused() {
        let mut bytes = 0usize;
        let canonical = ExactRationalValue {
            sign: ExactSign::Zero,
            numerator_magnitude_be: Vec::new(),
            denominator_be: vec![0x01],
        };
        let dto = exact_rational_dto(&canonical, &mut bytes).expect("the canonical zero");
        assert_eq!(dto.sign, "zero");
        assert_eq!(dto.numerator_magnitude_hex, "");
        assert_eq!(dto.denominator_magnitude_hex, "01");
        for denominator in [vec![0x02], vec![0x01, 0x00], vec![0x00], Vec::new()] {
            let mut bytes = 0usize;
            let value = ExactRationalValue {
                sign: ExactSign::Zero,
                numerator_magnitude_be: Vec::new(),
                denominator_be: denominator,
            };
            let error = exact_rational_dto(&value, &mut bytes)
                .expect_err("only `[0x01]` is a canonical zero denominator");
            assert_eq!(
                category(error),
                CurrentNonFlatLayerOrderViewErrorCategoryV1::InvalidEvidence
            );
        }
        let mut bytes = 0usize;
        let nonzero_numerator = ExactRationalValue {
            sign: ExactSign::Zero,
            numerator_magnitude_be: vec![0x01],
            denominator_be: vec![0x01],
        };
        assert!(exact_rational_dto(&nonzero_numerator, &mut bytes).is_err());
        let mut bytes = 0usize;
        let empty_numerator = ExactRationalValue {
            sign: ExactSign::Positive,
            numerator_magnitude_be: Vec::new(),
            denominator_be: vec![0x01],
        };
        assert!(exact_rational_dto(&empty_numerator, &mut bytes).is_err());
    }

    #[test]
    fn the_exact_magnitude_budget_is_shared_and_capped() {
        let mut bytes = MAX_EXACT_MAGNITUDE_BYTES_V1 - 2;
        let value = ExactRationalValue {
            sign: ExactSign::Positive,
            numerator_magnitude_be: vec![0x01],
            denominator_be: vec![0x01],
        };
        exact_rational_dto(&value, &mut bytes).expect("the boundary total is accepted");
        assert_eq!(bytes, MAX_EXACT_MAGNITUDE_BYTES_V1);
        let error = exact_rational_dto(&value, &mut bytes)
            .expect_err("one byte over the aggregate cap is refused");
        assert_eq!(
            category(error),
            CurrentNonFlatLayerOrderViewErrorCategoryV1::ResourceLimit
        );
    }

    #[test]
    fn the_face_digest_matches_an_independently_fixed_preimage() {
        let mut hasher = Sha256::new();
        hasher.update(FACE_DOMAIN_V1);
        frame(&mut hasher, b"11111111-1111-4111-8111-111111111111");
        frame_count(&mut hasher, 3);
        for value in [1.0_f64, 0.0, 0.0, -2.5, 4.0, 0.0, 0.0, 0.0, 8.125] {
            frame_f64(&mut hasher, value);
        }
        hasher.update([2u8]);
        for tag in ["x", "y"] {
            frame(&mut hasher, tag.as_bytes());
        }
        for value in [
            ExactRationalValue {
                sign: ExactSign::Positive,
                numerator_magnitude_be: vec![0x01],
                denominator_be: vec![0x01],
            },
            ExactRationalValue {
                sign: ExactSign::Zero,
                numerator_magnitude_be: Vec::new(),
                denominator_be: vec![0x01],
            },
            ExactRationalValue {
                sign: ExactSign::Negative,
                numerator_magnitude_be: vec![0x03],
                denominator_be: vec![0x02],
            },
            ExactRationalValue {
                sign: ExactSign::Positive,
                numerator_magnitude_be: vec![0x01],
                denominator_be: vec![0x01],
            },
            ExactRationalValue {
                sign: ExactSign::Zero,
                numerator_magnitude_be: Vec::new(),
                denominator_be: vec![0x01],
            },
            ExactRationalValue {
                sign: ExactSign::Negative,
                numerator_magnitude_be: vec![0x01, 0x00],
                denominator_be: vec![0x01],
            },
        ] {
            frame_exact(&mut hasher, &value);
        }
        assert_eq!(hex_lower(&hasher.finalize()), FACE_DIGEST_VECTOR_V1);
    }

    #[test]
    fn the_exact_boundary_and_cell_digests_match_independently_fixed_preimages() {
        let points = [
            (
                ExactRationalValue {
                    sign: ExactSign::Positive,
                    numerator_magnitude_be: vec![0x01],
                    denominator_be: vec![0x01],
                },
                ExactRationalValue {
                    sign: ExactSign::Zero,
                    numerator_magnitude_be: Vec::new(),
                    denominator_be: vec![0x01],
                },
            ),
            (
                ExactRationalValue {
                    sign: ExactSign::Negative,
                    numerator_magnitude_be: vec![0x02],
                    denominator_be: vec![0x03],
                },
                ExactRationalValue {
                    sign: ExactSign::Positive,
                    numerator_magnitude_be: vec![0x05],
                    denominator_be: vec![0x01],
                },
            ),
            (
                ExactRationalValue {
                    sign: ExactSign::Positive,
                    numerator_magnitude_be: vec![0x01, 0x00],
                    denominator_be: vec![0x01],
                },
                ExactRationalValue {
                    sign: ExactSign::Negative,
                    numerator_magnitude_be: vec![0x07],
                    denominator_be: vec![0x02],
                },
            ),
        ];
        let mut boundary_hasher = Sha256::new();
        boundary_hasher.update(EXACT_BOUNDARY_DOMAIN_V1);
        boundary_hasher.update([2u8]);
        for tag in ["x", "y"] {
            frame(&mut boundary_hasher, tag.as_bytes());
        }
        frame_count(&mut boundary_hasher, points.len());
        for (u, v) in &points {
            frame_exact(&mut boundary_hasher, u);
            frame_exact(&mut boundary_hasher, v);
        }
        let exact_digest = boundary_hasher.finalize();
        assert_eq!(hex_lower(&exact_digest), EXACT_BOUNDARY_DIGEST_VECTOR_V1);

        let lower = b"11111111-1111-4111-8111-111111111111";
        let upper = b"22222222-2222-4222-8222-222222222222";
        let mut cell_hasher = Sha256::new();
        cell_hasher.update(CELL_DOMAIN_V1);
        frame(&mut cell_hasher, lower);
        frame(&mut cell_hasher, upper);
        cell_hasher.update(exact_digest);
        assert_eq!(hex_lower(&cell_hasher.finalize()), CELL_DIGEST_VECTOR_V1);

        // Reversing the directed pair changes the cell digest.
        let mut reversed = Sha256::new();
        reversed.update(CELL_DOMAIN_V1);
        frame(&mut reversed, upper);
        frame(&mut reversed, lower);
        reversed.update(exact_digest);
        assert_ne!(hex_lower(&reversed.finalize()), CELL_DIGEST_VECTOR_V1);

        // A length-prefixed nested digest is a different preimage.
        let mut prefixed = Sha256::new();
        prefixed.update(CELL_DOMAIN_V1);
        frame(&mut prefixed, lower);
        frame(&mut prefixed, upper);
        frame(&mut prefixed, &exact_digest);
        assert_ne!(hex_lower(&prefixed.finalize()), CELL_DIGEST_VECTOR_V1);
    }

    #[test]
    fn the_domain_separator_is_not_length_framed() {
        let mut raw = Sha256::new();
        raw.update(FACE_DOMAIN_V1);
        let mut framed = Sha256::new();
        frame(&mut framed, FACE_DOMAIN_V1);
        assert_ne!(hex_lower(&raw.finalize()), hex_lower(&framed.finalize()));
    }

    #[test]
    fn the_exact_hash_uses_raw_magnitude_bytes_not_ascii_hex() {
        let value = ExactRationalValue {
            sign: ExactSign::Positive,
            numerator_magnitude_be: vec![0x01],
            denominator_be: vec![0x01],
        };
        let mut raw = Sha256::new();
        frame_exact(&mut raw, &value);
        let mut ascii = Sha256::new();
        ascii.update([2u8]);
        frame(&mut ascii, b"01");
        frame(&mut ascii, b"01");
        assert_ne!(hex_lower(&raw.finalize()), hex_lower(&ascii.finalize()));
    }

    #[test]
    fn one_bit_of_world_geometry_or_exact_magnitude_changes_its_digest() {
        let mut base = Sha256::new();
        base.update(FACE_DOMAIN_V1);
        frame_f64(&mut base, 4.0);
        let mut mutated = Sha256::new();
        mutated.update(FACE_DOMAIN_V1);
        frame_f64(&mut mutated, f64::from_bits(4.0_f64.to_bits() ^ 1));
        assert_ne!(hex_lower(&base.finalize()), hex_lower(&mutated.finalize()));

        let mut exact = Sha256::new();
        frame_exact(
            &mut exact,
            &ExactRationalValue {
                sign: ExactSign::Positive,
                numerator_magnitude_be: vec![0x01, 0x00],
                denominator_be: vec![0x01],
            },
        );
        let mut flipped = Sha256::new();
        frame_exact(
            &mut flipped,
            &ExactRationalValue {
                sign: ExactSign::Positive,
                numerator_magnitude_be: vec![0x01, 0x01],
                denominator_be: vec![0x01],
            },
        );
        assert_ne!(hex_lower(&exact.finalize()), hex_lower(&flipped.finalize()));
    }

    #[test]
    fn the_sign_and_axis_tags_are_frozen() {
        for (sign, tag) in [
            (ExactSign::Negative, 0u8),
            (ExactSign::Zero, 1),
            (ExactSign::Positive, 2),
        ] {
            let value = ExactRationalValue {
                sign,
                numerator_magnitude_be: if matches!(sign, ExactSign::Zero) {
                    Vec::new()
                } else {
                    vec![0x01]
                },
                denominator_be: vec![0x01],
            };
            let mut actual = Sha256::new();
            frame_exact(&mut actual, &value);
            let mut expected = Sha256::new();
            expected.update([tag]);
            frame(&mut expected, &value.numerator_magnitude_be);
            frame(&mut expected, &value.denominator_be);
            assert_eq!(
                hex_lower(&actual.finalize()),
                hex_lower(&expected.finalize())
            );
        }
        assert_eq!(axis_tag(0).unwrap(), ("x", ["y", "z"]));
        assert_eq!(axis_tag(1).unwrap(), ("y", ["x", "z"]));
        assert_eq!(axis_tag(2).unwrap(), ("z", ["x", "y"]));
        assert!(axis_tag(3).is_err());
    }

    #[test]
    fn a_foreign_instance_project_revision_or_fingerprint_is_stale() {
        let project = non_flat_tree_project(90.0);
        let base = canonical_request(&project);
        let mut foreign_instance = base.clone();
        foreign_instance.expected_project_instance_id = ProjectId::new();
        let mut foreign_project = base.clone();
        foreign_project.expected_project_id = ProjectId::new();
        let mut stale_revision = base.clone();
        stale_revision.expected_revision = base.expected_revision + 1;
        let mut stale_fingerprint = base.clone();
        stale_fingerprint.expected_fold_model_fingerprint_sha256 = "0".repeat(64);
        let mut short_fingerprint = base.clone();
        short_fingerprint.expected_fold_model_fingerprint_sha256 = String::new();
        for request in [
            foreign_instance,
            foreign_project,
            stale_revision,
            stale_fingerprint,
            short_fingerprint,
        ] {
            let error = build_current_non_flat_layer_order_view_v1(&project, &request)
                .expect_err("a foreign binding is refused");
            assert_eq!(
                category(error),
                CurrentNonFlatLayerOrderViewErrorCategoryV1::StaleAuthority
            );
        }
    }

    #[test]
    fn a_wrong_fixed_face_or_hinge_vector_is_refused() {
        let project = non_flat_tree_project(90.0);
        let base = canonical_request(&project);
        let mut wrong_face = base.clone();
        wrong_face.expected_applied_pose.fixed_face_id = FaceId::new();
        let mut extra_hinge = base.clone();
        let mut extra = extra_hinge.expected_applied_pose.hinge_angles[0].clone();
        extra.edge_id = EdgeId::new();
        extra_hinge.expected_applied_pose.hinge_angles.push(extra);
        extra_hinge
            .expected_applied_pose
            .hinge_angles
            .sort_by_key(|angle| angle.edge_id.canonical_bytes());
        let mut one_bit = base.clone();
        let angle = &mut one_bit.expected_applied_pose.hinge_angles[0];
        angle.angle_degrees = f64::from_bits(angle.angle_degrees.to_bits() ^ 1);
        let mut negative_zero = base.clone();
        negative_zero.expected_applied_pose.hinge_angles[0].angle_degrees = -0.0;
        let mut not_finite = base.clone();
        not_finite.expected_applied_pose.hinge_angles[0].angle_degrees = f64::NAN;
        let mut out_of_range = base.clone();
        out_of_range.expected_applied_pose.hinge_angles[0].angle_degrees = 181.0;
        let mut all_flat = base.clone();
        for angle in &mut all_flat.expected_applied_pose.hinge_angles {
            angle.angle_degrees = 180.0;
        }
        let mut requests = vec![
            wrong_face,
            extra_hinge,
            one_bit,
            negative_zero,
            not_finite,
            out_of_range,
            all_flat,
        ];
        if base.expected_applied_pose.hinge_angles.len() > 1 {
            let mut missing_hinge = base.clone();
            missing_hinge.expected_applied_pose.hinge_angles.pop();
            let mut duplicate_hinge = base.clone();
            let first = duplicate_hinge.expected_applied_pose.hinge_angles[0].clone();
            duplicate_hinge.expected_applied_pose.hinge_angles[1] = first;
            let mut reversed = base.clone();
            reversed.expected_applied_pose.hinge_angles.reverse();
            requests.push(missing_hinge);
            requests.push(duplicate_hinge);
            requests.push(reversed);
        }
        for request in requests {
            let error = build_current_non_flat_layer_order_view_v1(&project, &request)
                .expect_err("a mismatched pose is refused");
            assert_eq!(
                category(error),
                CurrentNonFlatLayerOrderViewErrorCategoryV1::StaleAuthority
            );
        }
    }

    #[test]
    fn an_empty_request_hinge_vector_is_a_resource_limit() {
        let project = non_flat_tree_project(90.0);
        let mut request = canonical_request(&project);
        request.expected_applied_pose.hinge_angles.clear();
        let error = build_current_non_flat_layer_order_view_v1(&project, &request)
            .expect_err("an empty request hinge vector is refused");
        assert_eq!(
            category(error),
            CurrentNonFlatLayerOrderViewErrorCategoryV1::ResourceLimit
        );
    }

    #[test]
    fn a_zero_cell_response_is_valid_and_never_claims_a_clearance_proof() {
        let project = non_flat_tree_project(90.0);
        let response = view(&project);
        assert!(response.cells.is_empty());
        assert_eq!(response.work.overlap_cell_count, 0);
        assert_eq!(response.work.face_pair_order_count, 0);
        assert_eq!(response.work.exact_boundary_point_count, 0);
        assert!(response.work.tested_face_pairs > 0);
    }

    #[test]
    fn a_project_without_non_flat_evidence_reports_absence() {
        let mut project = non_flat_tree_project(90.0);
        let request = canonical_request(&project);
        project.current_layer_evidence = None;
        assert_eq!(
            build_current_non_flat_layer_order_view_v1(&project, &request).unwrap(),
            None
        );
    }

    #[test]
    fn every_error_category_serializes_two_data_free_keys() {
        for (error, expected) in [
            (
                CurrentNonFlatLayerOrderViewErrorV1::stale(),
                "stale_authority",
            ),
            (
                CurrentNonFlatLayerOrderViewErrorV1::invalid(),
                "invalid_evidence",
            ),
            (
                CurrentNonFlatLayerOrderViewErrorV1::resource(),
                "resource_limit",
            ),
            (
                CurrentNonFlatLayerOrderViewErrorV1::internal(),
                "internal_failure",
            ),
        ] {
            let value = serde_json::to_value(error).unwrap();
            let object = value.as_object().expect("the payload is an object");
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            assert_eq!(keys, vec!["category".to_owned(), "version".to_owned()]);
            assert_eq!(object["version"], serde_json::json!(1));
            assert_eq!(object["category"], serde_json::json!(expected));
        }
    }

    #[test]
    fn a_reopened_project_needs_a_fresh_instance() {
        let project = non_flat_tree_project(90.0);
        let request = canonical_request(&project);
        let old_instance = project.instance_id;
        let archive = project.project_archive().unwrap();
        let reopened = crate::ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("non-flat-viewer-reopen.ori2"),
        )
        .expect("the archive reopens");
        assert_ne!(reopened.instance_id, old_instance);
        let error = build_current_non_flat_layer_order_view_v1(&reopened, &request)
            .expect_err("the old instance ID is refused after reopen");
        assert_eq!(
            category(error),
            CurrentNonFlatLayerOrderViewErrorCategoryV1::StaleAuthority
        );
    }

    // -- resource boundary matrix ------------------------------------------
    //
    // Every ceiling is fixed at `max - 1`, `max`, and `max + 1`. The counts are
    // given to the same validator production uses, so no proof is forged and no
    // oversized allocation happens.

    /// A consistent baseline projection; every field can be overridden.
    fn counts() -> ViewResourceCountsV1 {
        ViewResourceCountsV1 {
            material_faces: 2,
            folded_faces: 2,
            hinges: 1,
            declared_cells: 1,
            actual_cells: 1,
            declared_pairs: 1,
            actual_pairs: 1,
            tested_face_pairs: 1,
            source_overlap_cells_authenticated: 0,
        }
    }

    fn resource(error: CurrentNonFlatLayerOrderViewErrorV1) {
        assert_eq!(
            category(error),
            CurrentNonFlatLayerOrderViewErrorCategoryV1::ResourceLimit
        );
    }

    fn invalid(error: CurrentNonFlatLayerOrderViewErrorV1) {
        assert_eq!(
            category(error),
            CurrentNonFlatLayerOrderViewErrorCategoryV1::InvalidEvidence
        );
    }

    #[test]
    fn the_material_face_ceiling_is_inclusive() {
        for faces in [1, MAX_FACES_V1 - 1, MAX_FACES_V1] {
            let mut value = counts();
            value.material_faces = faces;
            value.folded_faces = faces;
            validate_view_resource_counts_v1(value).expect("the ceiling is inclusive");
        }
        for faces in [0, MAX_FACES_V1 + 1] {
            let mut value = counts();
            value.material_faces = faces;
            value.folded_faces = faces;
            resource(validate_view_resource_counts_v1(value).expect_err("out of range"));
        }
    }

    #[test]
    fn the_hinge_ceiling_is_inclusive() {
        for hinges in [1, MAX_HINGES_V1 - 1, MAX_HINGES_V1] {
            let mut value = counts();
            value.hinges = hinges;
            validate_view_resource_counts_v1(value).expect("the ceiling is inclusive");
        }
        for hinges in [0, MAX_HINGES_V1 + 1] {
            let mut value = counts();
            value.hinges = hinges;
            resource(validate_view_resource_counts_v1(value).expect_err("out of range"));
        }
    }

    #[test]
    fn the_overlap_cell_ceiling_is_inclusive() {
        for cells in [0, MAX_CELLS_V1 - 1, MAX_CELLS_V1] {
            let mut value = counts();
            value.declared_cells = cells;
            value.actual_cells = cells;
            value.declared_pairs = cells;
            value.actual_pairs = cells;
            validate_view_resource_counts_v1(value).expect("the ceiling is inclusive");
        }
        let mut over = counts();
        over.declared_cells = MAX_CELLS_V1 + 1;
        over.actual_cells = MAX_CELLS_V1 + 1;
        over.declared_pairs = MAX_CELLS_V1 + 1;
        over.actual_pairs = MAX_CELLS_V1 + 1;
        resource(validate_view_resource_counts_v1(over).expect_err("out of range"));
    }

    #[test]
    fn the_face_pair_order_ceiling_is_inclusive() {
        let mut boundary = counts();
        boundary.declared_cells = MAX_FACE_PAIR_ORDERS_V1;
        boundary.actual_cells = MAX_FACE_PAIR_ORDERS_V1;
        boundary.declared_pairs = MAX_FACE_PAIR_ORDERS_V1;
        boundary.actual_pairs = MAX_FACE_PAIR_ORDERS_V1;
        validate_view_resource_counts_v1(boundary).expect("the ceiling is inclusive");
        // The pair ceiling is judged before the pair/cell equality, so an
        // oversized pair count is a resource limit even with bounded cells.
        let mut over = counts();
        over.actual_pairs = MAX_FACE_PAIR_ORDERS_V1 + 1;
        resource(validate_view_resource_counts_v1(over).expect_err("out of range"));
        let mut declared_over = counts();
        declared_over.declared_pairs = MAX_FACE_PAIR_ORDERS_V1 + 1;
        resource(validate_view_resource_counts_v1(declared_over).expect_err("out of range"));
    }

    #[test]
    fn the_world_polygon_ceiling_is_inclusive() {
        for vertices in [
            3,
            MAX_WORLD_POLYGON_POINTS_V1 - 1,
            MAX_WORLD_POLYGON_POINTS_V1,
        ] {
            validate_world_polygon_count_v1(vertices).expect("the ceiling is inclusive");
        }
        resource(
            validate_world_polygon_count_v1(MAX_WORLD_POLYGON_POINTS_V1 + 1)
                .expect_err("out of range"),
        );
        for vertices in [0, 1, 2] {
            invalid(validate_world_polygon_count_v1(vertices).expect_err("degenerate"));
        }
    }

    #[test]
    fn the_cell_polygon_ceiling_is_inclusive() {
        for points in [
            3,
            MAX_CELL_POLYGON_POINTS_V1 - 1,
            MAX_CELL_POLYGON_POINTS_V1,
        ] {
            validate_cell_boundary_counts_v1(points, points).expect("the ceiling is inclusive");
        }
        let over = MAX_CELL_POLYGON_POINTS_V1 + 1;
        resource(validate_cell_boundary_counts_v1(over, over).expect_err("out of range"));
        for points in [0, 1, 2] {
            invalid(validate_cell_boundary_counts_v1(points, points).expect_err("degenerate"));
        }
    }

    #[test]
    fn a_rounded_and_exact_point_count_mismatch_is_invalid_evidence() {
        invalid(validate_cell_boundary_counts_v1(3, 4).expect_err("mismatch"));
        invalid(validate_cell_boundary_counts_v1(4, 3).expect_err("mismatch"));
    }

    #[test]
    fn the_aggregate_world_point_ceiling_is_inclusive() {
        let cap = MAX_TOTAL_WORLD_BOUNDARY_POINTS_V1;
        assert_eq!(
            accumulate_bounded_total_v1(cap - 2, 1, cap).unwrap(),
            cap - 1
        );
        assert_eq!(accumulate_bounded_total_v1(cap - 1, 1, cap).unwrap(), cap);
        resource(accumulate_bounded_total_v1(cap, 1, cap).expect_err("out of range"));
    }

    #[test]
    fn the_aggregate_exact_point_ceiling_is_inclusive() {
        let cap = MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1;
        assert_eq!(
            accumulate_bounded_total_v1(cap - 2, 1, cap).unwrap(),
            cap - 1
        );
        assert_eq!(accumulate_bounded_total_v1(cap - 1, 1, cap).unwrap(), cap);
        resource(accumulate_bounded_total_v1(cap, 1, cap).expect_err("out of range"));
    }

    #[test]
    fn the_aggregate_exact_magnitude_ceiling_is_inclusive() {
        let cap = MAX_EXACT_MAGNITUDE_BYTES_V1;
        assert_eq!(
            accumulate_bounded_total_v1(cap - 2, 1, cap).unwrap(),
            cap - 1
        );
        assert_eq!(accumulate_bounded_total_v1(cap - 1, 1, cap).unwrap(), cap);
        resource(accumulate_bounded_total_v1(cap, 1, cap).expect_err("out of range"));
    }

    #[test]
    fn a_bounded_accumulation_refuses_checked_add_overflow() {
        resource(
            accumulate_bounded_total_v1(usize::MAX, 1, usize::MAX)
                .expect_err("the checked add overflows"),
        );
        resource(
            accumulate_bounded_total_v1(usize::MAX - 1, 3, usize::MAX)
                .expect_err("the checked add overflows"),
        );
    }

    #[test]
    fn the_serialized_json_ceiling_is_inclusive() {
        let cap = MAX_SERIALIZED_JSON_BYTES_V1;
        validate_serialized_json_bytes_v1(cap - 1).expect("the ceiling is inclusive");
        validate_serialized_json_bytes_v1(cap).expect("the ceiling is inclusive");
        resource(validate_serialized_json_bytes_v1(cap + 1).expect_err("out of range"));
    }

    #[test]
    fn the_safe_wire_integer_ceiling_is_inclusive() {
        let cap = MAX_SAFE_WIRE_INTEGER_V1;
        validate_safe_wire_integer_v1(cap - 1).expect("the ceiling is inclusive");
        validate_safe_wire_integer_v1(cap).expect("the ceiling is inclusive");
        resource(validate_safe_wire_integer_v1(cap + 1).expect_err("out of range"));
        let mut work = counts();
        work.tested_face_pairs = cap + 1;
        resource(validate_view_resource_counts_v1(work).expect_err("out of range"));
        let mut authenticated = counts();
        authenticated.source_overlap_cells_authenticated = cap + 1;
        resource(validate_view_resource_counts_v1(authenticated).expect_err("out of range"));
    }

    // -- structural negative matrix ----------------------------------------

    #[test]
    fn a_material_and_folded_face_count_mismatch_is_invalid_evidence() {
        let mut fewer = counts();
        fewer.folded_faces = 1;
        invalid(validate_view_resource_counts_v1(fewer).expect_err("coverage mismatch"));
        let mut more = counts();
        more.folded_faces = 3;
        invalid(validate_view_resource_counts_v1(more).expect_err("coverage mismatch"));
    }

    #[test]
    fn a_declared_and_actual_count_mismatch_is_invalid_evidence() {
        let mut cells = counts();
        cells.declared_cells = 2;
        invalid(validate_view_resource_counts_v1(cells).expect_err("declared cell mismatch"));
        let mut pairs = counts();
        pairs.declared_pairs = 2;
        invalid(validate_view_resource_counts_v1(pairs).expect_err("declared pair mismatch"));
        let mut actual = counts();
        actual.actual_pairs = 2;
        invalid(validate_view_resource_counts_v1(actual).expect_err("actual pair mismatch"));
    }

    #[test]
    fn the_live_face_registry_must_match_the_proof_exactly() {
        let first = FaceId::new();
        let second = FaceId::new();
        let foreign = FaceId::new();
        let mut proof_faces = vec![first, second];
        proof_faces.sort_unstable_by_key(FaceId::canonical_bytes);

        validate_live_face_registry_v1(&proof_faces, &[second, first])
            .expect("the same set in any order is accepted");

        invalid(
            validate_live_face_registry_v1(&proof_faces, &[first])
                .expect_err("a missing live face is refused"),
        );
        invalid(
            validate_live_face_registry_v1(&proof_faces, &[first, second, foreign])
                .expect_err("an extra live face is refused"),
        );
        invalid(
            validate_live_face_registry_v1(&proof_faces, &[second, second])
                .expect_err("a duplicate live face is never collapsed away"),
        );
        invalid(
            validate_live_face_registry_v1(&proof_faces, &[first, foreign])
                .expect_err("a foreign live face of the same count is refused"),
        );
    }

    /// One face DTO with a chosen ID and dropped world axis.
    fn face_dto(face_id: &str, dropped: &'static str) -> CurrentNonFlatLayerOrderFaceDtoV1 {
        let (_, plane) = match dropped {
            "x" => ("x", ["y", "z"]),
            "y" => ("y", ["x", "z"]),
            _ => ("z", ["x", "y"]),
        };
        let zero = || ExactRationalDtoV1 {
            sign: "zero",
            numerator_magnitude_hex: String::new(),
            denominator_magnitude_hex: "01".to_owned(),
        };
        CurrentNonFlatLayerOrderFaceDtoV1 {
            face_id: face_id.to_owned(),
            face_key_sha256: "0".repeat(64),
            world_outer_boundary_xyz_mm: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            projection: FaceProjectionDtoV1 {
                dropped_world_axis: dropped,
                plane_axes: plane,
                source_to_plane_projection_exact: ExactAffineDtoV1 {
                    m00: zero(),
                    m01: zero(),
                    m10: zero(),
                    m11: zero(),
                    tx: zero(),
                    ty: zero(),
                },
            },
        }
    }

    #[test]
    fn an_unknown_equal_or_disagreeing_face_pair_is_invalid_evidence() {
        let lower = "11111111-1111-4111-8111-111111111111";
        let upper = "22222222-2222-4222-8222-222222222222";
        let faces = vec![face_dto(lower, "z"), face_dto(upper, "z")];
        let (resolved_lower, resolved_upper) =
            resolve_cell_face_pair_v1(&faces, lower, upper).expect("a known directed pair");
        assert_eq!(resolved_lower.face_id, lower);
        assert_eq!(resolved_upper.face_id, upper);
        // The reversed direction still resolves; the ordering itself is the
        // proof's own claim and is carried into the cell digest.
        let (reversed_lower, reversed_upper) =
            resolve_cell_face_pair_v1(&faces, upper, lower).expect("the reversed pair resolves");
        assert_eq!(reversed_lower.face_id, upper);
        assert_eq!(reversed_upper.face_id, lower);

        invalid(
            resolve_cell_face_pair_v1(&faces, lower, lower).expect_err("an equal pair is refused"),
        );
        invalid(
            resolve_cell_face_pair_v1(&faces, "33333333-3333-4333-8333-333333333333", upper)
                .expect_err("an unknown lower face is refused"),
        );
        invalid(
            resolve_cell_face_pair_v1(&faces, lower, "33333333-3333-4333-8333-333333333333")
                .expect_err("an unknown upper face is refused"),
        );

        let mixed = vec![face_dto(lower, "z"), face_dto(upper, "x")];
        invalid(
            resolve_cell_face_pair_v1(&mixed, lower, upper)
                .expect_err("two faces must agree on the dropped world axis"),
        );
    }

    #[test]
    fn the_plane_axes_must_be_derived_from_the_dropped_axis() {
        assert_eq!(validate_axis_derivation_v1("x", ["y", "z"]).unwrap(), 0);
        assert_eq!(validate_axis_derivation_v1("y", ["x", "z"]).unwrap(), 1);
        assert_eq!(validate_axis_derivation_v1("z", ["x", "y"]).unwrap(), 2);
        for (dropped, plane) in [
            ("z", ["y", "x"]),
            ("z", ["y", "z"]),
            ("x", ["x", "y"]),
            ("y", ["y", "z"]),
            ("w", ["x", "y"]),
            ("", ["x", "y"]),
        ] {
            invalid(
                validate_axis_derivation_v1(dropped, plane)
                    .expect_err("a derivation mismatch is refused"),
            );
        }
    }

    #[test]
    fn a_non_finite_world_point_is_invalid_evidence() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            invalid(canonical_finite(value).expect_err("a non-finite world point is refused"));
        }
        assert_eq!(canonical_finite(-0.0).unwrap().to_bits(), 0.0_f64.to_bits());
        assert_eq!(canonical_finite(2.5).unwrap().to_bits(), 2.5_f64.to_bits());
    }

    #[test]
    fn a_zero_face_or_hinge_count_is_a_resource_limit() {
        let mut faces = counts();
        faces.material_faces = 0;
        faces.folded_faces = 0;
        resource(validate_view_resource_counts_v1(faces).expect_err("zero faces"));
        let mut hinges = counts();
        hinges.hinges = 0;
        resource(validate_view_resource_counts_v1(hinges).expect_err("zero hinges"));
    }

    #[test]
    fn the_production_preflight_accepts_the_canonical_fixture() {
        let project = non_flat_tree_project(90.0);
        let proof = match project.current_layer_evidence.as_ref() {
            Some(CurrentLayerEvidence::NonFlat(proof)) => proof,
            _ => panic!("the fixture must own non-flat evidence"),
        };
        // The production projection and the validated projection are the same
        // values, so the boundary tests above cover the production path.
        let projected = ViewResourceCountsV1::from_proof(proof);
        assert_eq!(projected.material_faces, proof.material_faces().len());
        assert_eq!(projected.folded_faces, proof.folded_faces().len());
        assert_eq!(projected.hinges, proof.hinge_angles().len());
        assert_eq!(projected.actual_cells, proof.overlap_cells().len());
        assert_eq!(projected.declared_cells, proof.overlap_cell_count());
        assert_eq!(projected.actual_pairs, proof.face_pair_orders().len());
        assert_eq!(projected.declared_pairs, proof.face_pair_order_count());
        validate_view_resource_counts_v1(projected).expect("the canonical fixture is bounded");
        preflight_view_resources(proof).expect("the canonical fixture passes the preflight");
    }

    // -- issuer and dropped-axis positive matrix ---------------------------

    /// A two-face material tree whose single crease runs along world X.
    ///
    /// Folding about a crease parallel to world X carries one face into the
    /// world XY plane, which is the only way to observe a dropped Z axis from a
    /// canonical revalidation.
    fn horizontal_single_hinge_project() -> ProjectState {
        let positions = [
            (0.0, 0.0),
            (400.0, 0.0),
            (400.0, 200.0),
            (400.0, 400.0),
            (0.0, 400.0),
            (0.0, 200.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|(x, y)| ori_domain::Vertex {
                id: ori_domain::VertexId::new(),
                position: ori_domain::Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| ori_domain::Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: ori_domain::EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(ori_domain::Edge {
            id: EdgeId::new(),
            start: vertices[2].id,
            end: vertices[5].id,
            kind: ori_domain::EdgeKind::Mountain,
        });
        let paper = ori_domain::Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..ori_domain::Paper::default()
        };
        ProjectState::new_with_paper(ori_domain::CreasePattern { vertices, edges }, paper)
    }

    /// Installs a non-flat pose and freshly solved evidence on any project.
    fn install_non_flat_evidence(mut project: ProjectState, angle_degrees: f64) -> ProjectState {
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let simulation = topology
            .simulation_snapshot()
            .expect("a simulation snapshot");
        let fixed = simulation.faces[0].id;
        let hinges = simulation
            .hinge_adjacency
            .iter()
            .map(|hinge| hinge.edge)
            .collect::<Vec<_>>();
        crate::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
            &mut project,
            hinges.clone(),
            fixed,
            angle_degrees,
        );
        let angles = CanonicalHingeAngles::new(
            hinges
                .iter()
                .map(|edge| HingeAngle::new(*edge, angle_degrees).unwrap())
                .collect::<Vec<_>>(),
        )
        .expect("canonical hinge angles");
        let flat = crate::global_flat_foldability::reanalyze_current_flat_layer_order(&project)
            .expect("the current flat layer order");
        let proof = ori_core::revalidate_current_non_flat_layer_order_v1(
            project.project_id,
            project.editor.revision(),
            project.editor.pattern(),
            project.editor.paper(),
            Some(fixed),
            &angles,
            &flat,
            ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
        )
        .expect("a freshly solved non-flat layer order");
        project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(proof));
        project
    }

    /// The dropped world axes a fixture actually produces.
    fn dropped_axes(project: &ProjectState) -> Vec<&'static str> {
        let response = view(project);
        let mut axes = response
            .faces
            .iter()
            .map(|face| face.projection.dropped_world_axis)
            .collect::<Vec<_>>();
        axes.sort_unstable();
        axes.dedup();
        axes
    }

    #[test]
    fn the_vertical_crease_fixture_reaches_dropped_x_and_y() {
        let project = non_flat_tree_project(90.0);
        let axes = dropped_axes(&project);
        assert!(axes.contains(&"x"), "observed axes: {axes:?}");
        assert!(axes.contains(&"y"), "observed axes: {axes:?}");
        let response = view(&project);
        for face in &response.faces {
            let axis = validate_axis_derivation_v1(
                face.projection.dropped_world_axis,
                face.projection.plane_axes,
            )
            .expect("the plane axes are derived from the dropped axis");
            let (expected_dropped, expected_plane) = axis_tag(axis).unwrap();
            assert_eq!(face.projection.dropped_world_axis, expected_dropped);
            assert_eq!(face.projection.plane_axes, expected_plane);
        }
    }

    #[test]
    fn the_horizontal_crease_fixture_reaches_dropped_z() {
        let project = install_non_flat_evidence(horizontal_single_hinge_project(), 90.0);
        let axes = dropped_axes(&project);
        assert!(axes.contains(&"z"), "observed axes: {axes:?}");
        let response = view(&project);
        assert!(response.read_only);
        assert!(!response.authorizes_project_mutation);
        for face in &response.faces {
            validate_axis_derivation_v1(
                face.projection.dropped_world_axis,
                face.projection.plane_axes,
            )
            .expect("the plane axes are derived from the dropped axis");
        }
    }

    #[test]
    fn every_dropped_world_axis_is_reached_by_a_canonical_fixture() {
        let mut observed = dropped_axes(&non_flat_tree_project(90.0));
        observed.extend(dropped_axes(&install_non_flat_evidence(
            horizontal_single_hinge_project(),
            90.0,
        )));
        observed.sort_unstable();
        observed.dedup();
        assert_eq!(observed, vec!["x", "y", "z"], "observed axes: {observed:?}");
    }

    #[test]
    fn a_second_canonical_fixture_stays_read_only_and_deterministic() {
        let project = install_non_flat_evidence(horizontal_single_hinge_project(), 90.0);
        let first = view(&project);
        let second = view(&project);
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        let capability = capture_current_applied_pose_capability(&project)
            .unwrap()
            .unwrap();
        let live = revalidate_current_applied_pose_capability(&project, &capability)
            .unwrap()
            .unwrap();
        assert_eq!(first.pose.model_id, live.semantic_pose().model_id());
    }

    // -- graph issuer positive ---------------------------------------------

    /// The canonical dense-grid cycle fold angle, in degrees.
    ///
    /// The same magnitude the existing cycle regressions use, so the closed
    /// endpoint stays inside the certified neighbourhood.
    fn canonical_cycle_step_degrees() -> f64 {
        2.0 * (1.0_f64).atan2(100.0).to_degrees()
    }

    /// Builds a project whose applied pose is a closed non-flat graph pose.
    ///
    /// Only production constructors are used: the shared 3x3 Miura pattern, the
    /// canonical pose authority path, and the core graph revalidation entry
    /// point. No proof field is written directly.
    fn non_flat_graph_project() -> ProjectState {
        let step = canonical_cycle_step_degrees();
        let mut failures = Vec::new();
        for mask in 0..(1usize << 3) {
            // Every candidate starts from a fresh project; a rejected pending
            // state is never carried into the next candidate.
            let (pattern, mut paper, horizontal, _vertical) =
                super::viewer_dense_grid_cycle_test_support::miura_authority_pattern(3, 3);
            paper.thickness_mm = 0.1;
            let moving = horizontal.into_iter().take(3).collect::<Vec<_>>();
            let mut project = ProjectState::new_with_paper(pattern, paper);
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let snapshot = topology
                .simulation_snapshot()
                .expect("the shared fixture yields a simulation snapshot");
            // The fixed face is chosen canonically, never by storage order.
            let fixed = snapshot
                .faces
                .iter()
                .min_by_key(|face| (face.key.0, face.id.canonical_bytes()))
                .expect("at least one face")
                .id;
            let mut angles = snapshot
                .hinge_adjacency
                .iter()
                .map(|hinge| {
                    let Some(index) = moving.iter().position(|edge| *edge == hinge.edge) else {
                        // An inactive hinge is explicitly positive zero.
                        return (hinge.edge, 0.0_f64);
                    };
                    let mountain = hinge.assignment == ori_topology::FoldAssignment::Mountain;
                    let flip = mask & (1 << index) != 0;
                    (hinge.edge, if mountain ^ flip { -step } else { step })
                })
                .collect::<Vec<_>>();
            angles.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
            if let Err(error) = crate::applied_pose::tests::install_pose_authority_with_angles(
                &mut project,
                angles,
                fixed,
            ) {
                failures.push(format!("mask {mask}: pose {error:?}"));
                continue;
            }
            let Some(pose) = project.editor.current_applied_pose() else {
                failures.push(format!("mask {mask}: no applied pose"));
                continue;
            };
            let fixed_face = pose
                .fixed_face()
                .expect("the committed pose fixes one face");
            let committed = CanonicalHingeAngles::new(
                pose.hinge_angles()
                    .iter()
                    .map(|angle| {
                        HingeAngle::new(angle.edge(), angle.angle_degrees())
                            .expect("a committed hinge angle is representable")
                    })
                    .collect::<Vec<_>>(),
            )
            .expect("the committed hinge vector is canonical");
            let flat = match crate::global_flat_foldability::reanalyze_current_flat_layer_order(
                &project,
            ) {
                Ok(flat) => flat,
                Err(_) => {
                    failures.push(format!("mask {mask}: flat layer order"));
                    continue;
                }
            };
            // The graph entry point only; a tree fallback would not be a graph
            // issuer positive.
            let proof = match ori_core::revalidate_current_graph_non_flat_layer_order_v1(
                ori_core::RevalidateCurrentGraphNonFlatLayerOrderRequestV1 {
                    identity_namespace: project.project_id,
                    revision: project.editor.revision(),
                    pattern: project.editor.pattern(),
                    paper: project.editor.paper(),
                    fixed_face,
                    hinge_angles: &committed,
                    current_flat: &flat,
                    expected_archive: None,
                    max_face_pairs: ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
                },
            ) {
                Ok(proof) => proof,
                Err(error) => {
                    failures.push(format!("mask {mask}: graph revalidation {error:?}"));
                    continue;
                }
            };
            project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(proof));
            return project;
        }
        panic!("no closed graph candidate succeeded: {failures:?}");
    }

    #[test]
    fn a_closed_graph_issuer_yields_a_read_only_graph_view() {
        let project = non_flat_graph_project();

        // The live pose authority really is a closed graph issuer.
        let capability = capture_current_applied_pose_capability(&project)
            .expect("the capability is capturable")
            .expect("the project owns an applied pose");
        let live = revalidate_current_applied_pose_capability(&project, &capability)
            .expect("the capability revalidates")
            .expect("the capability is still current");
        assert!(
            live.graph().is_some(),
            "the live issuer must be a closed graph"
        );
        assert_eq!(live.semantic_pose().model_id(), GRAPH_POSE_MODEL_ID_V1);
        assert_eq!(issuer_kind(&live).unwrap(), PoseIssuerKindV1::Graph);

        let response = view(&project);
        assert_eq!(response.pose.model_id, GRAPH_POSE_MODEL_ID_V1);
        assert_eq!(response.pose.model_id, live.semantic_pose().model_id());
        assert!(response.read_only);
        assert!(!response.authorizes_project_mutation);

        let proof = match project.current_layer_evidence.as_ref() {
            Some(CurrentLayerEvidence::NonFlat(proof)) => proof,
            _ => panic!("the fixture must own non-flat evidence"),
        };
        // The response registry is exactly the proof registry.
        let mut proof_faces = proof
            .material_faces()
            .iter()
            .map(|face| face.face_id)
            .collect::<Vec<_>>();
        proof_faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let response_faces = response
            .faces
            .iter()
            .map(|face| face.face_id.clone())
            .collect::<Vec<_>>();
        let expected_faces = proof_faces
            .iter()
            .map(|face| wire_id(face).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(response_faces, expected_faces);
        assert_eq!(response.faces.len(), proof.material_faces().len());
        assert_eq!(response.pose.hinge_angles.len(), proof.hinge_angles().len());
        assert_eq!(response.cells.len(), proof.overlap_cell_count());
        assert_eq!(
            response.work.face_pair_order_count,
            proof.face_pair_order_count()
        );
        assert_eq!(response.work.material_face_count, response.faces.len());
        assert_eq!(response.work.overlap_cell_count, response.cells.len());

        for face in &response.faces {
            validate_axis_derivation_v1(
                face.projection.dropped_world_axis,
                face.projection.plane_axes,
            )
            .expect("the plane axes are derived from the dropped axis");
        }
        for cell in &response.cells {
            assert_ne!(cell.lower_face_id, cell.upper_face_id);
            assert_eq!(
                cell.projection.rounded_boundary_uv_mm.len(),
                cell.projection.exact_boundary_uv.len()
            );
        }

        // Two consecutive reads are identical in value and in bytes.
        let repeated = view(&project);
        assert_eq!(response, repeated);
        assert_eq!(
            serde_json::to_vec(&response).unwrap(),
            serde_json::to_vec(&repeated).unwrap()
        );
    }

    #[test]
    fn a_graph_issuer_view_still_refuses_every_stale_binding() {
        let project = non_flat_graph_project();
        let base = canonical_request(&project);
        let mut foreign_instance = base.clone();
        foreign_instance.expected_project_instance_id = ProjectId::new();
        let mut stale_revision = base.clone();
        stale_revision.expected_revision = base.expected_revision + 1;
        let mut wrong_face = base.clone();
        wrong_face.expected_applied_pose.fixed_face_id = FaceId::new();
        let mut one_ulp = base.clone();
        let angle = &mut one_ulp.expected_applied_pose.hinge_angles[0];
        angle.angle_degrees = f64::from_bits(angle.angle_degrees.to_bits() ^ 1);
        for request in [foreign_instance, stale_revision, wrong_face, one_ulp] {
            let error = build_current_non_flat_layer_order_view_v1(&project, &request)
                .expect_err("a stale binding is refused for a graph issuer too");
            assert_eq!(
                category(error),
                CurrentNonFlatLayerOrderViewErrorCategoryV1::StaleAuthority
            );
        }
    }
}
