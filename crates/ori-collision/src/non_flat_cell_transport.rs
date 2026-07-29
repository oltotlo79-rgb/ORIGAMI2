use std::collections::{HashMap, HashSet};

use ori_domain::{FaceId, Point2};
use ori_foldability::{ExactAffineTransform, ExactPointValue};
use thiserror::Error;

use crate::{CooperativeOperationControlV1, CooperativeOperationStopV1};

pub const NON_FLAT_CELL_TRANSPORT_MODEL_ID_V1: &str = "native_non_flat_exact_cell_transport_v1";

/// Proofs are invariant in their authenticated source type. Implementing the
/// public generic source traits can only mint a proof for that exact
/// implementation; it cannot manufacture core-owned layer evidence.
///
/// ```compile_fail
/// use ori_collision::NonFlatCellTransportProofV1;
///
/// struct FirstSource;
/// struct SecondSource;
/// fn substitute_source(
///     first: NonFlatCellTransportProofV1<FirstSource>,
/// ) -> NonFlatCellTransportProofV1<SecondSource> {
///     first
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct NonFlatCellTransportProofV1<T> {
    source: T,
    target: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonFlatCellTransportLimitsV1 {
    pub max_faces: usize,
    pub max_cells: usize,
    pub max_pairs: usize,
    pub max_boundary_points: usize,
}

/// Process-wide hard caps. Caller-configured limits may narrow these values,
/// but can never expand certification work beyond them.
pub const NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1: NonFlatCellTransportLimitsV1 =
    NonFlatCellTransportLimitsV1 {
        max_faces: 2_048,
        max_cells: 2_000_000,
        max_pairs: 2_000_000,
        max_boundary_points: 8_000_000,
    };

impl Default for NonFlatCellTransportLimitsV1 {
    fn default() -> Self {
        NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
    }
}

impl<T: PartialEq> NonFlatCellTransportProofV1<T> {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        NON_FLAT_CELL_TRANSPORT_MODEL_ID_V1
    }
    #[must_use]
    pub const fn target(&self) -> &T {
        &self.target
    }
    #[must_use]
    pub fn is_for(&self, source: &T, target: &T) -> bool {
        self.source == *source && self.target == *target
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NonFlatCellTransportErrorV1 {
    #[error("non-flat layer validation was cancelled")]
    Cancelled,
    #[error("non-flat layer validation absolute deadline elapsed")]
    DeadlineExceeded,
    #[error("non-flat layer evidence is stale or belongs to another project")]
    BindingMismatch,
    #[error("non-flat exact face or cell coverage is incomplete")]
    IncompleteCoverage,
    #[error("non-flat cell order crosses or contradicts itself")]
    Crossing,
    #[error("non-flat cell transport exceeds its configured work bound")]
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NonFlatLayerOrderCountsV1 {
    material_faces: usize,
    folded_faces: usize,
    overlap_cells: usize,
    pair_orders: usize,
}

pub fn certify_non_flat_cell_transport_v1<T>(
    source: &T,
    target: &T,
) -> Result<NonFlatCellTransportProofV1<T>, NonFlatCellTransportErrorV1>
where
    T: NonFlatLayerOrderTransportSourceV1,
{
    certify_non_flat_cell_transport_with_limits_v1(
        source,
        target,
        NonFlatCellTransportLimitsV1::default(),
    )
}

pub fn certify_non_flat_cell_transport_with_limits_v1<T>(
    source: &T,
    target: &T,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<NonFlatCellTransportProofV1<T>, NonFlatCellTransportErrorV1>
where
    T: NonFlatLayerOrderTransportSourceV1,
{
    certify_non_flat_cell_transport_with_control_v1(
        source,
        target,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Controlled transport certification.
///
/// Count-only bounds are checked before reading any cell. Boundary accounting,
/// structural validation, and proof retention all observe the same cooperative
/// control, so a stopped operation never returns partially checked authority.
/// `Clone` itself is not cooperatively interruptible; both retained values pass
/// the process-wide count and boundary hard caps and complete structural
/// validation before either clone begins, with checkpoints between retention
/// steps and immediately before issuance.
pub fn certify_non_flat_cell_transport_with_control_v1<T>(
    source: &T,
    target: &T,
    limits: NonFlatCellTransportLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<NonFlatCellTransportProofV1<T>, NonFlatCellTransportErrorV1>
where
    T: NonFlatLayerOrderTransportSourceV1,
{
    validate_non_flat_configured_limits_v1(limits)?;
    non_flat_layer_checkpoint_v1(control)?;
    let source_counts = capture_non_flat_layer_order_counts_v1(source, control)?;
    preflight_non_flat_layer_order_counts_v1(source_counts, 0, limits)?;
    let target_counts = capture_non_flat_layer_order_counts_v1(target, control)?;
    preflight_non_flat_layer_order_counts_v1(target_counts, 0, limits)?;

    let independently_readmitted_same_model = source.same_target_model_v1(target);
    non_flat_layer_checkpoint_v1(control)?;
    if source.identity_namespace_v1() != target.identity_namespace_v1()
        || source.target_revision_v1().checked_add(1) != Some(target.target_revision_v1())
        || (target.source_overlap_cells_authenticated_v1() != source_counts.overlap_cells
            && !independently_readmitted_same_model)
    {
        return Err(NonFlatCellTransportErrorV1::BindingMismatch);
    }
    non_flat_layer_checkpoint_v1(control)?;
    let source_boundary_points = count_non_flat_boundary_points_with_control_v1(
        source,
        source_counts.overlap_cells,
        control,
    )?;
    preflight_non_flat_layer_order_counts_v1(source_counts, source_boundary_points, limits)?;
    let target_boundary_points = count_non_flat_boundary_points_with_control_v1(
        target,
        target_counts.overlap_cells,
        control,
    )?;
    preflight_non_flat_layer_order_counts_v1(target_counts, target_boundary_points, limits)?;

    validate_non_flat_layer_order_structural_source_with_expected_counts_v1(
        source,
        source_counts,
        NonFlatCellTransportErrorV1::BindingMismatch,
        control,
    )?;
    validate_non_flat_layer_order_structural_source_with_expected_counts_v1(
        target,
        target_counts,
        NonFlatCellTransportErrorV1::BindingMismatch,
        control,
    )?;
    non_flat_layer_checkpoint_v1(control)?;
    let retained_source = source.clone();
    non_flat_layer_checkpoint_v1(control)?;
    let retained_target = target.clone();
    non_flat_layer_checkpoint_v1(control)?;
    Ok(NonFlatCellTransportProofV1 {
        source: retained_source,
        target: retained_target,
    })
}

pub fn preflight_non_flat_cell_transport_v1(
    faces: usize,
    cells: usize,
    pairs: usize,
    boundary_points: usize,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    if !non_flat_configured_limits_within_hard_caps_v1(limits)
        || faces == 0
        || faces > limits.max_faces
        || cells > limits.max_cells
        || pairs > limits.max_pairs
        || boundary_points > limits.max_boundary_points
        || pairs > cells
    {
        return Err(NonFlatCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

fn non_flat_configured_limits_within_hard_caps_v1(limits: NonFlatCellTransportLimitsV1) -> bool {
    limits.max_faces <= NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_faces
        && limits.max_cells <= NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_cells
        && limits.max_pairs <= NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_pairs
        && limits.max_boundary_points <= NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_boundary_points
}

fn validate_non_flat_configured_limits_v1(
    limits: NonFlatCellTransportLimitsV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    if !non_flat_configured_limits_within_hard_caps_v1(limits) {
        return Err(NonFlatCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

fn preflight_non_flat_layer_order_counts_v1(
    counts: NonFlatLayerOrderCountsV1,
    boundary_points: usize,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    preflight_non_flat_cell_transport_v1(
        counts.material_faces,
        counts.overlap_cells,
        counts.pair_orders,
        boundary_points,
        limits,
    )?;
    if counts.folded_faces > limits.max_faces {
        return Err(NonFlatCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

fn preflight_non_flat_structural_hard_counts_v1(
    counts: NonFlatLayerOrderCountsV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    if counts.material_faces > NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_faces
        || counts.folded_faces > NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_faces
        || counts.overlap_cells > NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_cells
        || counts.pair_orders > NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1.max_pairs
    {
        return Err(NonFlatCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

fn capture_non_flat_layer_order_counts_v1<S>(
    source: &S,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<NonFlatLayerOrderCountsV1, NonFlatCellTransportErrorV1>
where
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
{
    non_flat_layer_checkpoint_v1(control)?;
    let material_faces = source.material_face_count();
    non_flat_layer_checkpoint_v1(control)?;
    let folded_faces = source.folded_face_count();
    non_flat_layer_checkpoint_v1(control)?;
    let overlap_cells = source.overlap_cell_count();
    non_flat_layer_checkpoint_v1(control)?;
    let pair_orders = source.face_pair_order_count();
    non_flat_layer_checkpoint_v1(control)?;
    Ok(NonFlatLayerOrderCountsV1 {
        material_faces,
        folded_faces,
        overlap_cells,
        pair_orders,
    })
}

fn count_non_flat_boundary_points_with_control_v1<S>(
    source: &S,
    overlap_cells: usize,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<usize, NonFlatCellTransportErrorV1>
where
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
{
    let mut boundary_points = 0_usize;
    for index in 0..overlap_cells {
        non_flat_layer_checkpoint_v1(control)?;
        let cell = source
            .overlap_cell(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        non_flat_layer_checkpoint_v1(control)?;
        boundary_points = boundary_points
            .checked_add(cell.exact_boundary.len())
            .ok_or(NonFlatCellTransportErrorV1::ResourceLimit)?;
    }
    non_flat_layer_checkpoint_v1(control)?;
    Ok(boundary_points)
}

/// Validates the structural completeness of one non-flat layer order.
///
/// This is the single definition consumed both by
/// [`certify_non_flat_cell_transport_with_limits_v1`] and by read-only viewers
/// that must not receive transport binding or transition authority. It performs
/// validation only: it never issues a proof, a capability, or any mutation
/// authority, and it does not compare two revisions.
pub fn validate_non_flat_layer_order_structure_v1<T>(
    value: &T,
) -> Result<(), NonFlatCellTransportErrorV1>
where
    T: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
{
    validate_non_flat_layer_order_structure_with_control_v1(
        value,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Controlled structural validation. A stop fails closed before the caller can
/// treat an incompletely checked source as valid layer evidence.
pub fn validate_non_flat_layer_order_structure_with_control_v1<T>(
    value: &T,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), NonFlatCellTransportErrorV1>
where
    T: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
{
    validate_non_flat_layer_order_structural_source_with_control_v1(value, control)
}

/// Borrowed structural view of one folded face.
pub struct NonFlatFoldedFaceStructuralRefV1<'a> {
    pub face_id: FaceId,
    pub dropped_world_axis: u8,
    pub source_to_plane: &'a ExactAffineTransform,
}

/// Borrowed structural view of one overlap cell.
pub struct NonFlatOverlapCellStructuralRefV1<'a> {
    pub boundary: &'a [Point2],
    pub exact_boundary: &'a [ExactPointValue],
    pub lower_face: FaceId,
    pub upper_face: FaceId,
}

/// Structural view of one directed face-pair order.
///
/// Pair orders form an idempotent registry of directed face relations. A
/// source may either emit one normalized entry or repeat the same-direction
/// entry alongside each supporting cell. Multiple overlap cells may therefore
/// reference the same normalized relation; the opposite direction still
/// contradicts it.
#[derive(Debug, Clone, Copy)]
pub struct NonFlatFacePairOrderStructuralV1 {
    pub lower_face: FaceId,
    pub upper_face: FaceId,
}

/// Read-only structural source consumed by the completeness validator.
///
/// Implementations borrow their data and grant no proof, capability, or
/// mutation authority. A transport proof remains parameterized by its source
/// type, so an unrelated implementation cannot impersonate authenticated core
/// evidence.
pub trait NonFlatLayerOrderStructuralSourceV1 {
    fn material_face_count(&self) -> usize;
    fn material_face_id(&self, index: usize) -> Option<FaceId>;
    fn folded_face_count(&self) -> usize;
    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>>;
    fn overlap_cell_count(&self) -> usize;
    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>>;
    fn face_pair_order_count(&self) -> usize;
    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1>;
}

/// Authenticated binding needed in addition to structural completeness.
pub trait NonFlatLayerOrderTransportSourceV1:
    NonFlatLayerOrderStructuralSourceV1 + Clone + PartialEq
{
    fn identity_namespace_v1(&self) -> ori_domain::ProjectId;
    fn target_revision_v1(&self) -> u64;
    fn source_overlap_cells_authenticated_v1(&self) -> usize;
    fn same_target_model_v1(&self, other: &Self) -> bool;
}

/// Structural completeness of one non-flat layer order.
///
/// This is the single definition of the check. It compares no revisions,
/// normalizes idempotent same-direction pair declarations before matching
/// every overlap cell to the resulting registry, and fails closed to
/// [`NonFlatCellTransportErrorV1::IncompleteCoverage`] whenever a declared
/// count and the readable data disagree.
#[cfg(test)]
fn validate_non_flat_layer_order_structural_source_v1<
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
>(
    source: &S,
) -> Result<(), NonFlatCellTransportErrorV1> {
    validate_non_flat_layer_order_structural_source_with_control_v1(
        source,
        &CooperativeOperationControlV1::unbounded(),
    )
}

fn validate_non_flat_layer_order_structural_source_with_control_v1<
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
>(
    source: &S,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), NonFlatCellTransportErrorV1> {
    let expected = capture_non_flat_layer_order_counts_v1(source, control)?;
    preflight_non_flat_structural_hard_counts_v1(expected)?;
    validate_non_flat_layer_order_structural_source_with_expected_counts_v1(
        source,
        expected,
        NonFlatCellTransportErrorV1::IncompleteCoverage,
        control,
    )
}

fn validate_non_flat_layer_order_structural_source_with_expected_counts_v1<
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
>(
    source: &S,
    expected: NonFlatLayerOrderCountsV1,
    count_mismatch: NonFlatCellTransportErrorV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), NonFlatCellTransportErrorV1> {
    let observed = capture_non_flat_layer_order_counts_v1(source, control)?;
    if observed != expected {
        return Err(count_mismatch);
    }
    let material_count = expected.material_faces;
    let folded_count = expected.folded_faces;
    let overlap_count = expected.overlap_cells;
    let pair_count = expected.pair_orders;
    non_flat_layer_checkpoint_v1(control)?;
    let mut faces = HashSet::new();
    non_flat_layer_checkpoint_v1(control)?;
    faces
        .try_reserve(material_count)
        .map_err(|_| NonFlatCellTransportErrorV1::ResourceLimit)?;
    non_flat_layer_checkpoint_v1(control)?;
    for index in 0..material_count {
        non_flat_layer_checkpoint_v1(control)?;
        let face = source
            .material_face_id(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if !faces.insert(face) {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    let mut folded_ids = HashSet::new();
    non_flat_layer_checkpoint_v1(control)?;
    folded_ids
        .try_reserve(folded_count)
        .map_err(|_| NonFlatCellTransportErrorV1::ResourceLimit)?;
    non_flat_layer_checkpoint_v1(control)?;
    for index in 0..folded_count {
        non_flat_layer_checkpoint_v1(control)?;
        let folded = source
            .folded_face(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if !folded_ids.insert(folded.face_id) {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    non_flat_layer_checkpoint_v1(control)?;
    if faces.is_empty()
        || faces.len() != material_count
        || folded_count != faces.len()
        || folded_ids != faces
    {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    non_flat_layer_checkpoint_v1(control)?;
    for index in 0..folded_count {
        non_flat_layer_checkpoint_v1(control)?;
        let folded = source
            .folded_face(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        let transform = folded.source_to_plane;
        let values = [
            &transform.m00,
            &transform.m01,
            &transform.m10,
            &transform.m11,
            &transform.tx,
            &transform.ty,
        ];
        if folded.dropped_world_axis > 2 {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
        for value in values {
            non_flat_layer_checkpoint_v1(control)?;
            if value.to_f64().is_none_or(|value| !value.is_finite()) {
                return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
            }
        }
    }
    non_flat_layer_checkpoint_v1(control)?;
    if pair_count > overlap_count {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    let mut directed = HashMap::<(FaceId, FaceId), bool>::new();
    non_flat_layer_checkpoint_v1(control)?;
    directed
        .try_reserve(pair_count)
        .map_err(|_| NonFlatCellTransportErrorV1::ResourceLimit)?;
    non_flat_layer_checkpoint_v1(control)?;
    for index in 0..pair_count {
        non_flat_layer_checkpoint_v1(control)?;
        let pair = source
            .face_pair_order(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if !faces.contains(&pair.lower_face)
            || !faces.contains(&pair.upper_face)
            || pair.lower_face == pair.upper_face
        {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
        if directed.contains_key(&(pair.upper_face, pair.lower_face)) {
            return Err(NonFlatCellTransportErrorV1::Crossing);
        }
        directed
            .entry((pair.lower_face, pair.upper_face))
            .or_insert(false);
    }
    for index in 0..overlap_count {
        non_flat_layer_checkpoint_v1(control)?;
        let cell = source
            .overlap_cell(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if cell.boundary.len() < 3
            || cell.exact_boundary.len() != cell.boundary.len()
            || !faces.contains(&cell.lower_face)
            || !faces.contains(&cell.upper_face)
            || cell.lower_face == cell.upper_face
        {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
        for (point, exact) in cell.boundary.iter().zip(cell.exact_boundary) {
            non_flat_layer_checkpoint_v1(control)?;
            if exact
                .x
                .to_f64()
                .is_none_or(|x| x.to_bits() != point.x.to_bits())
                || exact
                    .y
                    .to_f64()
                    .is_none_or(|y| y.to_bits() != point.y.to_bits())
            {
                return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
            }
        }
        non_flat_layer_checkpoint_v1(control)?;
        let order = (cell.lower_face, cell.upper_face);
        let Some(covered) = directed.get_mut(&order) else {
            if directed.contains_key(&(cell.upper_face, cell.lower_face)) {
                return Err(NonFlatCellTransportErrorV1::Crossing);
            }
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        };
        *covered = true;
    }
    non_flat_layer_checkpoint_v1(control)?;
    for covered in directed.values() {
        non_flat_layer_checkpoint_v1(control)?;
        if !*covered {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    non_flat_layer_checkpoint_v1(control)?;
    Ok(())
}

fn non_flat_layer_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), NonFlatCellTransportErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => NonFlatCellTransportErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            NonFlatCellTransportErrorV1::DeadlineExceeded
        }
    })
}

#[cfg(test)]
mod tests;
