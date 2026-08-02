//! Canonical, interruptible `LayerOrderSnapshot` binding for V2.

use ori_domain::FaceId;
use ori_foldability::{
    ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign, FacePairOrderSnapshot,
    FoldedFaceSnapshot, GlobalFlatFoldabilityModelId, LayerFace, LayerOrderDerivation,
    LayerOrderModelId, LayerOrderProvenance, LayerOrderSnapshot,
    LayerOrderSnapshotRetainedByteLimitV2, OverlapCellKey, OverlapCellSnapshot,
};
use sha2::{Digest, Sha256};

use super::*;
mod equality;
mod membership;
pub(crate) use equality::source_equal_with_checkpoint_v2;
use membership::*;

/// All source counts needed by the explicit V2 transport equation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceMetricsV2 {
    pub(super) material_faces: usize,
    pub(super) folded_faces: usize,
    pub(super) overlap_cells: usize,
    pub(super) face_pair_orders: usize,
    pub(super) global_order_faces: usize,
    pub(super) layer_records: usize,
    pub(super) boundary_vertices: usize,
    pub(super) boundary_layer_products: usize,
    /// Semantic length-based source size observed during validation. Unlike
    /// retained capacity, this is allocator independent and is bound on
    /// replay as the live overlap-source metric required by the profile.
    pub(super) projected_source_bytes: usize,
    /// Deterministic V2 charge for the bounded source passes. This is the
    /// caller's admission cap, never allocator-observed vector capacity.
    pub(super) charged_source_bytes: usize,
    pub(super) traversal_work: usize,
}

/// Linearly measured source shape used to reject an over-budget source before
/// any of the later membership scans become quadratic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceLayoutMetricsV2 {
    global_order_faces: usize,
    layer_records: usize,
    boundary_vertices: usize,
    boundary_layer_products: usize,
    exact_value_bytes: usize,
    supporting_cells: usize,
    projected_source_bytes: usize,
}

// The issue path performs six whole-source passes whose cost scales with the
// caller-admitted source bound: pre-validation deep-size, digest, clone
// preflight, clone copy, clone remeasurement, and the issue-side
// remeasurement. The explicit exact-value walk is charged apart.
const RETAINED_SOURCE_FULL_PASSES_V2: usize = 6;

/// Validates the sealed source's transport shape and returns its
/// domain-separated digest.
///
/// This boundary never accepts a caller-constructed `LayerOrderSnapshot`: the
/// borrowed snapshot arrives only through the opaque V2 authority, whose
/// solver or no-search revalidation has already regenerated the full exact
/// geometric certificate. The checks below therefore defend transport-owned
/// resource accounting and record shape; they do not repeat arbitrary-size
/// rational GCD reduction or the solver's complete pair/cell reconstruction.
pub(super) fn source_digest_and_metrics_v2(
    source: &LayerOrderSnapshot,
    geometry: &MaterialHingeGraphGeometry,
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    profile: &CommonArticulationResourceProfileV2,
    expected_provenance: ori_foldability::GlobalFlatFoldabilityProvenance,
    limits: CommonArticulationGeneralCellTransportLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<([u8; 32], SourceMetricsV2), CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint_v2(checkpoint)?;
    if source.model_id != LayerOrderModelId::FacewiseLayerOrderV1
        || !source.is_current_for(&expected_provenance)
        || source.provenance.source.model_id != GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1
        || source.provenance.source.identity_namespace.is_none()
        || source.provenance.source.source_fingerprint.is_none()
        || source.material_faces.len() != geometry.face_ids().len()
        || source.material_faces.len() > limits.max_material_faces
        || source.folded_faces.len() != source.material_faces.len()
        || source.folded_faces.len() > limits.max_folded_faces
        || source.overlap_cells.len() > limits.max_overlap_cells
        || source.face_pair_orders.len() > limits.max_face_pair_orders
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
    }

    validate_source_retained_cap_v2(source, limits.max_source_retained_bytes, checkpoint)?;

    ensure_source_logical_work_within_limit_v2(
        retained_source_work_v2(limits.max_source_retained_bytes)?,
        limits.max_logical_work,
    )?;
    let layout = scan_source_layout_metrics_v2(source, checkpoint)?;
    let traversal_work =
        source_traversal_work_v2(source, layout, limits.max_source_retained_bytes, checkpoint)?;
    // Perform this deterministic bound check before the membership/prior
    // scans below. A tiny logical budget must not buy quadratic validation.
    ensure_source_logical_work_within_limit_v2(traversal_work, limits.max_logical_work)?;

    validate_material_faces_v2(source, geometry, decomposition, profile, checkpoint)?;
    validate_global_order_v2(source, limits, checkpoint)?;
    validate_cells_v2(source, checkpoint)?;
    validate_folded_faces_v2(source, checkpoint)?;
    validate_pair_orders_v2(source, checkpoint)?;

    let metrics = SourceMetricsV2 {
        material_faces: source.material_faces.len(),
        folded_faces: source.folded_faces.len(),
        overlap_cells: source.overlap_cells.len(),
        face_pair_orders: source.face_pair_orders.len(),
        global_order_faces: layout.global_order_faces,
        layer_records: layout.layer_records,
        boundary_vertices: layout.boundary_vertices,
        boundary_layer_products: layout.boundary_layer_products,
        projected_source_bytes: layout.projected_source_bytes,
        charged_source_bytes: limits.max_source_retained_bytes,
        traversal_work,
    };
    let digest = source_digest_v2(source, checkpoint)?;
    checkpoint_v2(checkpoint)?;
    Ok((digest, metrics))
}

fn validate_material_faces_v2(
    source: &LayerOrderSnapshot,
    geometry: &MaterialHingeGraphGeometry,
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    profile: &CommonArticulationResourceProfileV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    for (index, face) in source.material_faces.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        if index > 0 && source.material_faces[index - 1].face_key.0 >= face.face_key.0 {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
        if !contains_geometry_face_v2(geometry, face.face_id, checkpoint)?
            || contains_prior_face_id_v2(&source.material_faces[..index], face.face_id, checkpoint)?
        {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
    }
    if profile.actual_v2().face_count_v2() != source.material_faces.len()
        || decomposition.actual_block_count_v2() != profile.actual_block_count_v2()
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
    }
    for block in decomposition.blocks() {
        for face in block.geometry().face_ids().iter().copied() {
            if !contains_material_face_id_v2(source, face, checkpoint)? {
                return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
            }
        }
    }
    // Equal cardinality plus unique membership in the geometry registry means
    // every material face is represented, without assuming an ID sort order.
    checkpoint_v2(checkpoint)
}

fn validate_global_order_v2(
    source: &LayerOrderSnapshot,
    limits: CommonArticulationGeneralCellTransportLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    let Some(order) = source.global_bottom_to_top.as_deref() else {
        return Ok(0);
    };
    if order.len() != source.material_faces.len() || order.len() > limits.max_global_order_faces {
        return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
    }
    for (index, face) in order.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        if !contains_layer_face_v2(&source.material_faces, face, checkpoint)?
            || contains_prior_face_id_v2(&order[..index], face.face_id, checkpoint)?
        {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
    }
    Ok(order.len())
}

fn validate_folded_faces_v2(
    source: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    for (index, folded) in source.folded_faces.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        if !contains_layer_face_v2(&source.material_faces, &folded.face, checkpoint)?
            || contains_prior_folded_face_id_v2(
                &source.folded_faces[..index],
                folded.face.face_id,
                checkpoint,
            )?
        {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
        validate_transform_v2(&folded.source_to_flat, checkpoint)?;
    }
    checkpoint_v2(checkpoint)
}

pub(super) fn validate_cells_v2(
    source: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(usize, usize, usize), CommonArticulationGeneralCellTransportErrorV2> {
    let mut layer_records = 0usize;
    let mut boundary_vertices = 0usize;
    let mut boundary_layer_products = 0usize;
    for (index, cell) in source.overlap_cells.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        if index > 0 && source.overlap_cells[index - 1].cell_key.0 >= cell.cell_key.0
            || cell.exact_boundary.len() < 3
            || cell.covering_faces.len() != cell.bottom_to_top_faces.len()
            || cell.bottom_to_top_faces.is_empty()
        {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
        for point in &cell.exact_boundary {
            validate_point_v2(point, checkpoint)?;
        }
        // `covering_faces` is a canonical material registry subset; it is
        // intentionally not in solved layer order. Validate its registry
        // identity and uniqueness separately from the bottom-to-top order.
        for (index, face) in cell.covering_faces.iter().enumerate() {
            checkpoint_v2(checkpoint)?;
            if !contains_layer_face_v2(&source.material_faces, face, checkpoint)?
                || contains_prior_face_id_v2(
                    &cell.covering_faces[..index],
                    face.face_id,
                    checkpoint,
                )?
            {
                return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
            }
        }
        for (rank, face) in cell.bottom_to_top_faces.iter().enumerate() {
            checkpoint_v2(checkpoint)?;
            if !contains_material_face_id_v2(source, *face, checkpoint)?
                || contains_face_id_v2(&cell.bottom_to_top_faces[..rank], *face, checkpoint)?
                || !contains_prior_face_id_v2(&cell.covering_faces, *face, checkpoint)?
            {
                return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
            }
        }
        layer_records = layer_records
            .checked_add(cell.bottom_to_top_faces.len())
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
        boundary_vertices = boundary_vertices
            .checked_add(cell.exact_boundary.len())
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
        boundary_layer_products = cell
            .exact_boundary
            .len()
            .checked_mul(cell.bottom_to_top_faces.len())
            .and_then(|work| boundary_layer_products.checked_add(work))
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    }
    Ok((layer_records, boundary_vertices, boundary_layer_products))
}

pub(super) fn validate_pair_orders_v2(
    source: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    // The sealed authority already binds every pair to its complete set of
    // supporting cells: foldability revalidation regenerates that set by
    // scanning all overlap cells and compares the complete ordered records.
    // Repeating P*C*layer work here would add a second solver-scale pass with
    // no attacker-controlled raw snapshot to defend. Transport still checks
    // endpoint registry membership, support-key shape, canonical issuer order,
    // and the one-record per unordered-pair invariant needed by its own
    // resource accounting. Canonical order makes the opposite-direction check
    // logarithmic and allocation-free instead of scanning all prior pairs.
    for adjacent in source.face_pair_orders.windows(2) {
        checkpoint_v2(checkpoint)?;
        if directed_pair_order_key_v2(&adjacent[0]) >= directed_pair_order_key_v2(&adjacent[1]) {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
    }
    for pair in &source.face_pair_orders {
        checkpoint_v2(checkpoint)?;
        if pair.lower_face == pair.upper_face
            || !contains_layer_face_v2(&source.material_faces, &pair.lower_face, checkpoint)?
            || !contains_layer_face_v2(&source.material_faces, &pair.upper_face, checkpoint)?
            || contains_reversed_pair_order_v2(&source.face_pair_orders, pair, checkpoint)?
            || pair.supporting_cells.is_empty()
        {
            return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
        }
        for (index, cell) in pair.supporting_cells.iter().enumerate() {
            checkpoint_v2(checkpoint)?;
            if !contains_overlap_cell_key_v2(&source.overlap_cells, *cell, checkpoint)?
                || contains_cell_key_v2(&pair.supporting_cells[..index], *cell, checkpoint)?
            {
                return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
            }
        }
    }
    checkpoint_v2(checkpoint)
}

fn validate_point_v2(
    point: &ExactPointValue,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    validate_rational_v2(&point.x, checkpoint)?;
    validate_rational_v2(&point.y, checkpoint)
}

fn validate_transform_v2(
    transform: &ExactAffineTransform,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    for value in [
        &transform.m00,
        &transform.m01,
        &transform.m10,
        &transform.m11,
        &transform.tx,
        &transform.ty,
    ] {
        validate_rational_v2(value, checkpoint)?;
    }
    Ok(())
}

pub(super) fn validate_rational_v2(
    value: &ExactRationalValue,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    let numerator_is_canonical = match value.sign {
        ExactSign::Zero => {
            value.numerator_magnitude_be.as_slice() == [0] && value.denominator_be.as_slice() == [1]
        }
        ExactSign::Negative | ExactSign::Positive => value
            .numerator_magnitude_be
            .first()
            .is_some_and(|first| *first != 0),
    };
    if value.denominator_be.first().is_none_or(|first| *first == 0) || !numerator_is_canonical {
        return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
    }
    for byte in value
        .numerator_magnitude_be
        .iter()
        .chain(value.denominator_be.iter())
    {
        checkpoint_v2(checkpoint)?;
        let _ = byte;
    }
    Ok(())
}

/// Completes the linear, checkpointed source pass required before structural
/// membership validation. It deliberately does not trust structural contents:
/// malformed records still consume their bounded scan cost before later
/// fail-closed validation rejects them.
pub(super) fn scan_source_layout_metrics_v2(
    source: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<SourceLayoutMetricsV2, CommonArticulationGeneralCellTransportErrorV2> {
    let mut layer_records = 0usize;
    let mut boundary_vertices = 0usize;
    let mut boundary_layer_products = 0usize;
    let mut exact_value_bytes = 0usize;
    let mut projected_source_bytes = std::mem::size_of::<LayerOrderSnapshot>();
    checked_add_projected_vec_v2::<LayerFace>(
        &mut projected_source_bytes,
        source.material_faces.len(),
    )?;
    if let Some(global_order) = &source.global_bottom_to_top {
        checked_add_projected_vec_v2::<LayerFace>(&mut projected_source_bytes, global_order.len())?;
    }
    checked_add_projected_vec_v2::<FoldedFaceSnapshot>(
        &mut projected_source_bytes,
        source.folded_faces.len(),
    )?;
    for folded in &source.folded_faces {
        exact_value_bytes = exact_value_bytes
            .checked_add(transform_declared_byte_len_v2(
                &folded.source_to_flat,
                &mut projected_source_bytes,
                checkpoint,
            )?)
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    }
    checked_add_projected_vec_v2::<OverlapCellSnapshot>(
        &mut projected_source_bytes,
        source.overlap_cells.len(),
    )?;
    for cell in &source.overlap_cells {
        checkpoint_v2(checkpoint)?;
        let cell_layers = cell.bottom_to_top_faces.len();
        let cell_vertices = cell.exact_boundary.len();
        checked_add_projected_vec_v2::<ExactPointValue>(
            &mut projected_source_bytes,
            cell_vertices,
        )?;
        layer_records = layer_records
            .checked_add(cell_layers)
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
        boundary_vertices = boundary_vertices
            .checked_add(cell_vertices)
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
        boundary_layer_products = cell_vertices
            .checked_mul(cell_layers)
            .and_then(|work| boundary_layer_products.checked_add(work))
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
        for point in &cell.exact_boundary {
            exact_value_bytes = exact_value_bytes
                .checked_add(point_declared_byte_len_v2(
                    point,
                    &mut projected_source_bytes,
                    checkpoint,
                )?)
                .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
        }
        checked_add_projected_vec_v2::<LayerFace>(
            &mut projected_source_bytes,
            cell.covering_faces.len(),
        )?;
        checked_add_projected_vec_v2::<FaceId>(&mut projected_source_bytes, cell_layers)?;
    }
    checked_add_projected_vec_v2::<FacePairOrderSnapshot>(
        &mut projected_source_bytes,
        source.face_pair_orders.len(),
    )?;
    let supporting_cells = source
        .face_pair_orders
        .iter()
        .try_fold(0usize, |sum, pair| {
            checkpoint_v2(checkpoint)?;
            checked_add_projected_vec_v2::<OverlapCellKey>(
                &mut projected_source_bytes,
                pair.supporting_cells.len(),
            )?;
            sum.checked_add(pair.supporting_cells.len())
                .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
        })?;
    Ok(SourceLayoutMetricsV2 {
        global_order_faces: source.global_bottom_to_top.as_ref().map_or(0, Vec::len),
        layer_records,
        boundary_vertices,
        boundary_layer_products,
        exact_value_bytes,
        supporting_cells,
        projected_source_bytes,
    })
}

/// Computes the deterministic source traversal charge from the linear layout
/// pass. `charged_source_bytes` is always the admitted input bound rather
/// than allocator-observed capacity, so equivalent snapshots replay across
/// runtimes and spare allocation differences.
pub(super) fn source_traversal_work_v2(
    source: &LayerOrderSnapshot,
    layout: SourceLayoutMetricsV2,
    charged_source_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint_v2(checkpoint)?;
    let material = source.material_faces.len();
    let cells = source.overlap_cells.len();
    let pair_orders = source.face_pair_orders.len();
    // Every explicit membership/registry search below is checkpointed. The
    // bound is deliberately stated in terms of their deterministic ceilings,
    // rather than hiding work inside `any`/`contains`: material+block registry 3m^2,
    // global-order 2m^2, folded registry 2m^2, cell registry/membership/prior
    // scans 2Lm + 3L^2, pair endpoints 2Pm, canonical pair order P,
    // reverse-direction binary lookups P*(floor(log2(P))+1), and source/prior
    // supporting-cell checks 2SC.
    let pair_registry_work = checked_pair_registry_work_v2(pair_orders)?;
    let membership_work = material
        .checked_mul(material)
        .and_then(|value| value.checked_mul(7))
        .and_then(|value| {
            value.checked_add(layout.layer_records.checked_mul(material)?.checked_mul(2)?)
        })
        .and_then(|value| {
            value.checked_add(
                layout
                    .layer_records
                    .checked_mul(layout.layer_records)?
                    .checked_mul(3)?,
            )
        })
        .and_then(|value| value.checked_add(pair_orders.checked_mul(material)?.checked_mul(2)?))
        .and_then(|value| value.checked_add(pair_registry_work))
        .and_then(|value| {
            value.checked_add(layout.supporting_cells.checked_mul(cells)?.checked_mul(2)?)
        })
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    // Charge every retained-capacity pass in the actual issue path with the
    // finite caller-admitted bound: input deep-size, digest, clone
    // projected-size, clone copy, clone remeasurement, and issue-side
    // remeasurement. The explicit exact-value walk is charged apart.
    let retention_work = retained_source_work_v2(charged_source_bytes)?;
    source
        .material_faces
        .len()
        .checked_add(source.folded_faces.len())
        .and_then(|value| value.checked_add(source.overlap_cells.len()))
        .and_then(|value| value.checked_add(source.face_pair_orders.len()))
        .and_then(|value| value.checked_add(layout.global_order_faces))
        .and_then(|value| value.checked_add(layout.layer_records))
        .and_then(|value| value.checked_add(layout.boundary_vertices))
        .and_then(|value| value.checked_add(layout.exact_value_bytes))
        .and_then(|value| value.checked_add(layout.supporting_cells))
        .and_then(|value| value.checked_add(membership_work))
        .and_then(|value| value.checked_add(retention_work))
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
}

pub(super) fn checked_pair_registry_work_v2(
    pair_orders: usize,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    let pair_search_depth = if pair_orders == 0 {
        0
    } else {
        (usize::BITS - pair_orders.leading_zeros()) as usize
    };
    pair_orders
        .checked_add(
            pair_orders
                .checked_mul(pair_search_depth)
                .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?,
        )
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
}

fn retained_source_work_v2(
    charged_source_bytes: usize,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    charged_source_bytes
        .checked_mul(RETAINED_SOURCE_FULL_PASSES_V2)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
}

pub(super) fn ensure_source_logical_work_within_limit_v2(
    observed: usize,
    maximum: usize,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    if observed > maximum {
        return Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit);
    }
    Ok(())
}

/// Bounds allocator-observed live capacity before any source-layout work.
/// Capacity is admission-only and deliberately never enters source identity.
pub(super) fn validate_source_retained_cap_v2(
    source: &LayerOrderSnapshot,
    maximum: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    let mut size_checkpoint = || checkpoint_v2(checkpoint);
    match source
        .checked_deep_retained_bytes_with_limit_and_checkpoint_v2(maximum, &mut size_checkpoint)?
    {
        LayerOrderSnapshotRetainedByteLimitV2::WithinLimit { .. } => Ok(()),
        LayerOrderSnapshotRetainedByteLimitV2::Exceeded { .. } => {
            Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
        }
    }
}

/// Computes the exact-payload work bound without touching payload bytes. The
/// actual byte scans occur only after the logical cap has admitted the source,
/// in `validate_rational_v2` and the digest encoder.
fn point_declared_byte_len_v2(
    point: &ExactPointValue,
    projected_source_bytes: &mut usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    rational_declared_byte_len_v2(&point.x, projected_source_bytes, checkpoint)?
        .checked_add(rational_declared_byte_len_v2(
            &point.y,
            projected_source_bytes,
            checkpoint,
        )?)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
}

fn transform_declared_byte_len_v2(
    transform: &ExactAffineTransform,
    projected_source_bytes: &mut usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    let mut bytes = 0usize;
    for value in [
        &transform.m00,
        &transform.m01,
        &transform.m10,
        &transform.m11,
        &transform.tx,
        &transform.ty,
    ] {
        bytes = bytes
            .checked_add(rational_declared_byte_len_v2(
                value,
                projected_source_bytes,
                checkpoint,
            )?)
            .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    }
    Ok(bytes)
}

fn rational_declared_byte_len_v2(
    value: &ExactRationalValue,
    projected_source_bytes: &mut usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<usize, CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint_v2(checkpoint)?;
    checked_add_projected_vec_v2::<u8>(projected_source_bytes, value.numerator_magnitude_be.len())?;
    checked_add_projected_vec_v2::<u8>(projected_source_bytes, value.denominator_be.len())?;
    value
        .numerator_magnitude_be
        .len()
        .checked_add(value.denominator_be.len())
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
}

fn checked_add_projected_vec_v2<T>(
    total: &mut usize,
    length: usize,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    let bytes = std::mem::size_of::<T>()
        .checked_mul(length)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    *total = total
        .checked_add(bytes)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    Ok(())
}

pub(super) fn source_digest_v2(
    source: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<[u8; 32], CommonArticulationGeneralCellTransportErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_MODEL_ID_V2.as_bytes());
    hash.update([1]);
    hash_provenance_v2(&mut hash, &source.provenance, checkpoint)?;
    hash_layer_faces_v2(&mut hash, &source.material_faces, checkpoint)?;
    match source.global_bottom_to_top.as_deref() {
        Some(faces) => {
            hash.update([1]);
            hash_layer_faces_v2(&mut hash, faces, checkpoint)?;
        }
        None => hash.update([0]),
    }
    hash_optional_layer_face_v2(&mut hash, source.reference_face, checkpoint)?;
    hash_folded_faces_v2(&mut hash, &source.folded_faces, checkpoint)?;
    hash_cells_v2(&mut hash, &source.overlap_cells, checkpoint)?;
    hash_pair_orders_v2(&mut hash, &source.face_pair_orders, checkpoint)?;
    match source.proof_summary {
        Some(summary) => {
            hash.update([1]);
            for value in [
                summary.material_faces,
                summary.overlap_face_pairs,
                summary.overlap_cells,
                summary.constraints,
                summary.maximum_ply,
                summary.certificate_bytes,
            ] {
                hash_usize_v2(&mut hash, value)?;
            }
        }
        None => hash.update([0]),
    }
    // All semantic fields and their lengths are encoded above. Deliberately
    // exclude allocator capacity and runtime resource observations: they are
    // admission checks, not certificate identity.
    Ok(hash.finalize().into())
}

fn hash_provenance_v2(
    hash: &mut Sha256,
    provenance: &LayerOrderProvenance,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash.update([match provenance.source.model_id {
        GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1 => 1,
    }]);
    match provenance.source.identity_namespace {
        Some(id) => {
            hash.update([1]);
            hash.update(id.canonical_bytes());
        }
        None => hash.update([0]),
    }
    hash.update(provenance.source.source_revision.to_le_bytes());
    match provenance.source.source_fingerprint {
        Some(value) => {
            hash.update([1]);
            hash.update(value.0);
        }
        None => hash.update([0]),
    }
    checkpoint_v2(checkpoint)?;
    match provenance.derivation {
        LayerOrderDerivation::SingleFace { face } => {
            hash.update([1]);
            hash_layer_face_v2(hash, &face, checkpoint)?;
        }
        LayerOrderDerivation::SingleHinge {
            hinge_edge,
            assignment,
            canonical_first,
            canonical_second,
        } => {
            hash.update([2]);
            hash.update(hinge_edge.canonical_bytes());
            hash.update([match assignment {
                ori_topology::FoldAssignment::Mountain => 1,
                ori_topology::FoldAssignment::Valley => 2,
            }]);
            hash_layer_face_v2(hash, &canonical_first, checkpoint)?;
            hash_layer_face_v2(hash, &canonical_second, checkpoint)?;
        }
        LayerOrderDerivation::FacewiseCertificate {
            reference_face,
            overlap_cell_count,
            constraint_count,
        } => {
            hash.update([3]);
            hash_layer_face_v2(hash, &reference_face, checkpoint)?;
            hash_usize_v2(hash, overlap_cell_count)?;
            hash_usize_v2(hash, constraint_count)?;
        }
    }
    Ok(())
}

fn hash_layer_faces_v2(
    hash: &mut Sha256,
    faces: &[ori_foldability::LayerFace],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash_usize_v2(hash, faces.len())?;
    for face in faces {
        hash_layer_face_v2(hash, face, checkpoint)?;
    }
    Ok(())
}
fn hash_layer_face_v2(
    hash: &mut Sha256,
    face: &ori_foldability::LayerFace,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint_v2(checkpoint)?;
    hash.update(face.face_id.canonical_bytes());
    hash.update(face.face_key.0);
    Ok(())
}
fn hash_optional_layer_face_v2(
    hash: &mut Sha256,
    face: Option<ori_foldability::LayerFace>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    match face {
        Some(face) => {
            hash.update([1]);
            hash_layer_face_v2(hash, &face, checkpoint)
        }
        None => {
            hash.update([0]);
            Ok(())
        }
    }
}

fn hash_folded_faces_v2(
    hash: &mut Sha256,
    faces: &[FoldedFaceSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash_usize_v2(hash, faces.len())?;
    for face in faces {
        hash_layer_face_v2(hash, &face.face, checkpoint)?;
        hash.update([match face.orientation {
            ori_foldability::FoldedFaceOrientation::FrontUp => 1,
            ori_foldability::FoldedFaceOrientation::BackUp => 2,
        }]);
        hash_transform_v2(hash, &face.source_to_flat, checkpoint)?;
    }
    Ok(())
}
fn hash_cells_v2(
    hash: &mut Sha256,
    cells: &[OverlapCellSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash_usize_v2(hash, cells.len())?;
    for cell in cells {
        checkpoint_v2(checkpoint)?;
        hash.update(cell.cell_key.0);
        hash_usize_v2(hash, cell.exact_boundary.len())?;
        for point in &cell.exact_boundary {
            hash_point_v2(hash, point, checkpoint)?;
        }
        hash_layer_faces_v2(hash, &cell.covering_faces, checkpoint)?;
        hash_usize_v2(hash, cell.bottom_to_top_faces.len())?;
        for face in &cell.bottom_to_top_faces {
            checkpoint_v2(checkpoint)?;
            hash.update(face.canonical_bytes());
        }
    }
    Ok(())
}
fn hash_pair_orders_v2(
    hash: &mut Sha256,
    pairs: &[FacePairOrderSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash_usize_v2(hash, pairs.len())?;
    for pair in pairs {
        hash_layer_face_v2(hash, &pair.lower_face, checkpoint)?;
        hash_layer_face_v2(hash, &pair.upper_face, checkpoint)?;
        hash_usize_v2(hash, pair.supporting_cells.len())?;
        for cell in &pair.supporting_cells {
            checkpoint_v2(checkpoint)?;
            hash.update(cell.0);
        }
    }
    Ok(())
}
fn hash_point_v2(
    hash: &mut Sha256,
    point: &ExactPointValue,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash_rational_v2(hash, &point.x, checkpoint)?;
    hash_rational_v2(hash, &point.y, checkpoint)
}
fn hash_transform_v2(
    hash: &mut Sha256,
    transform: &ExactAffineTransform,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    for value in [
        &transform.m00,
        &transform.m01,
        &transform.m10,
        &transform.m11,
        &transform.tx,
        &transform.ty,
    ] {
        hash_rational_v2(hash, value, checkpoint)?;
    }
    Ok(())
}
fn hash_rational_v2(
    hash: &mut Sha256,
    value: &ExactRationalValue,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash.update([match value.sign {
        ExactSign::Negative => 0,
        ExactSign::Zero => 1,
        ExactSign::Positive => 2,
    }]);
    hash_bytes_v2(hash, &value.numerator_magnitude_be, checkpoint)?;
    hash_bytes_v2(hash, &value.denominator_be, checkpoint)
}
fn hash_bytes_v2(
    hash: &mut Sha256,
    bytes: &[u8],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash_usize_v2(hash, bytes.len())?;
    for byte in bytes {
        checkpoint_v2(checkpoint)?;
        hash.update([*byte]);
    }
    Ok(())
}
fn hash_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
