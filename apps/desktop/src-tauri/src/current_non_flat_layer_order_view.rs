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
use ori_domain::FaceId;
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
    edge_id: String,
    angle_degrees: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentNonFlatLayerOrderViewPoseRequestV1 {
    fixed_face_id: String,
    hinge_angles: Vec<CurrentNonFlatLayerOrderViewHingeAngleRequestV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentNonFlatLayerOrderViewRequestV1 {
    version: u8,
    expected_project_instance_id: String,
    expected_project_id: String,
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
    if denominator.is_empty()
        || denominator.iter().all(|byte| *byte == 0)
        || denominator[0] == 0
        || zero_sign != numerator.is_empty()
        || numerator.first().is_some_and(|byte| *byte == 0)
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    *magnitude_bytes = magnitude_bytes
        .checked_add(numerator.len())
        .and_then(|sum| sum.checked_add(denominator.len()))
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
    if *magnitude_bytes > MAX_EXACT_MAGNITUDE_BYTES_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
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
    if project_instance_id != request.expected_project_instance_id
        || project_id != request.expected_project_id
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
    if wire_id(&proof.identity_namespace())
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?
        != project_id
        || proof.target_revision() != revision
        || proof.target_fingerprint().to_hex() != fingerprint
    {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    validate_non_flat_layer_order_structure_v1(proof)
        .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::invalid())?;
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
    if bytes.len() > MAX_SERIALIZED_JSON_BYTES_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    Ok(Some(response))
}

fn preflight_view_resources(
    proof: &StackedFoldNonFlatLayerOrderV1,
) -> Result<(), CurrentNonFlatLayerOrderViewErrorV1> {
    let boundary_points = proof
        .overlap_cells()
        .iter()
        .try_fold(0usize, |sum, cell| {
            sum.checked_add(cell.exact_boundary().len())
        })
        .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
    preflight_non_flat_cell_transport_v1(
        proof.material_faces().len(),
        proof.overlap_cell_count(),
        proof.face_pair_order_count(),
        boundary_points,
        NonFlatCellTransportLimitsV1 {
            max_faces: MAX_FACES_V1,
            max_cells: MAX_CELLS_V1,
            max_pairs: MAX_FACE_PAIR_ORDERS_V1,
            max_boundary_points: MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1,
        },
    )
    .map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::resource())?;
    if proof.hinge_angles().len() > MAX_HINGES_V1 || proof.hinge_angles().is_empty() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
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
    if fixed_face_id != request.expected_applied_pose.fixed_face_id {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    let semantic = view.semantic_pose();
    if semantic.fixed_face() != Some(fixed) {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    // The response pose model ID follows the issuer kind; a graph pose never
    // carries the tree model ID.
    let pose_model_id = match (view.tree(), view.graph()) {
        (Some(_), None) => TREE_POSE_MODEL_ID_V1,
        (None, Some(_)) => GRAPH_POSE_MODEL_ID_V1,
        _ => return Err(CurrentNonFlatLayerOrderViewErrorV1::internal()),
    };
    if semantic.model_id() != pose_model_id {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    let mut hinge_angles = Vec::with_capacity(proof.hinge_angles().len());
    for angle in proof.hinge_angles() {
        let edge_id =
            wire_id(&angle.edge()).map_err(|_| CurrentNonFlatLayerOrderViewErrorV1::internal())?;
        hinge_angles.push(CurrentNonFlatLayerOrderHingeAngleDtoV1 {
            edge_id,
            angle_degrees: canonical_finite(angle.angle_degrees())?,
        });
    }
    if hinge_angles.len() != request.expected_applied_pose.hinge_angles.len() {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
    }
    for (proof_angle, requested) in hinge_angles
        .iter()
        .zip(&request.expected_applied_pose.hinge_angles)
    {
        if proof_angle.edge_id != requested.edge_id
            || proof_angle.angle_degrees.to_bits()
                != canonical_finite(requested.angle_degrees)?.to_bits()
        {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::stale());
        }
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
    if face_ids.is_empty() || face_ids.len() > MAX_FACES_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    // The proof material faces and the live model faces must be the same set:
    // a missing face and an extra live face are both refused.
    let mut live_face_ids = match (view.tree(), view.graph()) {
        (Some((model, _)), None) => model.face_ids().to_vec(),
        (None, Some((geometry, _, _))) => geometry.face_ids().to_vec(),
        _ => return Err(CurrentNonFlatLayerOrderViewErrorV1::internal()),
    };
    live_face_ids.sort_unstable_by_key(FaceId::canonical_bytes);
    live_face_ids.dedup();
    if live_face_ids != face_ids {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::invalid());
    }
    let mut total_points = 0usize;
    let mut faces = Vec::with_capacity(face_ids.len());
    for face_id in face_ids {
        let folded = proof
            .folded_faces()
            .iter()
            .find(|folded| folded.face().face_id == face_id)
            .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
        let world = world_boundary(view, face_id)?;
        total_points = total_points
            .checked_add(world.len())
            .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
        if total_points > MAX_TOTAL_WORLD_BOUNDARY_POINTS_V1 {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
        }
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
        let mut hasher = Sha256::new();
        frame(&mut hasher, FACE_DOMAIN_V1);
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

fn world_boundary(
    view: &CurrentAppliedPoseView<'_>,
    face_id: FaceId,
) -> Result<Vec<[f64; 3]>, CurrentNonFlatLayerOrderViewErrorV1> {
    let points = match (view.tree(), view.graph()) {
        (Some((model, pose)), None) => {
            let boundary = model
                .face_boundary(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            let transform = pose
                .face_transform(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            boundary
                .vertices()
                .iter()
                .map(|vertex| {
                    let source = pose.vertex_position(*vertex)?;
                    transform.apply_point(source).ok()
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?
        }
        (None, Some((geometry, _audit, pose))) => {
            let vertices = geometry
                .face_boundary_vertices(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            let transform = pose
                .face_transform(face_id)
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?;
            vertices
                .iter()
                .map(|vertex| {
                    let source = geometry.vertex_position(*vertex)?;
                    transform.apply_point(source).ok()
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::invalid)?
        }
        _ => return Err(CurrentNonFlatLayerOrderViewErrorV1::internal()),
    };
    if points.len() < 3 || points.len() > MAX_WORLD_POLYGON_POINTS_V1 {
        return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
    }
    points
        .into_iter()
        .map(|point| {
            Ok([
                canonical_finite(point.x())?,
                canonical_finite(point.y())?,
                canonical_finite(point.z())?,
            ])
        })
        .collect()
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
        let dropped = lower.projection.dropped_world_axis;
        let plane = lower.projection.plane_axes;
        let axis_byte: u8 = match dropped {
            "x" => 0,
            "y" => 1,
            _ => 2,
        };
        if cell.boundary().len() < 3
            || cell.boundary().len() > MAX_CELL_POLYGON_POINTS_V1
            || cell.exact_boundary().len() != cell.boundary().len()
        {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
        }
        total_points = total_points
            .checked_add(cell.exact_boundary().len())
            .ok_or_else(CurrentNonFlatLayerOrderViewErrorV1::resource)?;
        if total_points > MAX_TOTAL_EXACT_BOUNDARY_POINTS_V1 {
            return Err(CurrentNonFlatLayerOrderViewErrorV1::resource());
        }
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
        frame(&mut boundary_hasher, EXACT_BOUNDARY_DOMAIN_V1);
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
        frame(&mut cell_hasher, CELL_DOMAIN_V1);
        frame(&mut cell_hasher, lower_face_id.as_bytes());
        frame(&mut cell_hasher, upper_face_id.as_bytes());
        frame(&mut cell_hasher, &exact_digest);
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
