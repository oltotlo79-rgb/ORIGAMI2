use std::collections::HashSet;

use ori_domain::{FaceId, Point2};
use ori_foldability::{ExactAffineTransform, ExactPointValue};
use thiserror::Error;

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

impl Default for NonFlatCellTransportLimitsV1 {
    fn default() -> Self {
        Self {
            max_faces: 2_048,
            max_cells: 2_000_000,
            max_pairs: 2_000_000,
            max_boundary_points: 8_000_000,
        }
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
    #[error("non-flat layer evidence is stale or belongs to another project")]
    BindingMismatch,
    #[error("non-flat exact face or cell coverage is incomplete")]
    IncompleteCoverage,
    #[error("non-flat cell order crosses or contradicts itself")]
    Crossing,
    #[error("non-flat cell transport exceeds its configured work bound")]
    ResourceLimit,
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
    let independently_readmitted_same_model = source.same_target_model_v1(target);
    if source.identity_namespace_v1() != target.identity_namespace_v1()
        || source.target_revision_v1().checked_add(1) != Some(target.target_revision_v1())
        || (target.source_overlap_cells_authenticated_v1() != source.overlap_cell_count()
            && !independently_readmitted_same_model)
    {
        return Err(NonFlatCellTransportErrorV1::BindingMismatch);
    }
    let boundary_points = (0..target.overlap_cell_count()).try_fold(0usize, |sum, index| {
        let cell = target
            .overlap_cell(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        sum.checked_add(cell.exact_boundary.len())
            .ok_or(NonFlatCellTransportErrorV1::ResourceLimit)
    })?;
    preflight_non_flat_cell_transport_v1(
        target.material_face_count(),
        target.overlap_cell_count(),
        target.face_pair_order_count(),
        boundary_points,
        limits,
    )?;
    validate_non_flat_layer_order_structure_v1(target)?;
    Ok(NonFlatCellTransportProofV1 {
        source: source.clone(),
        target: target.clone(),
    })
}

pub fn preflight_non_flat_cell_transport_v1(
    faces: usize,
    cells: usize,
    pairs: usize,
    boundary_points: usize,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    if faces == 0
        || faces > limits.max_faces
        || cells > limits.max_cells
        || pairs > limits.max_pairs
        || boundary_points > limits.max_boundary_points
        || pairs != cells
    {
        return Err(NonFlatCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

#[cfg(test)]
macro_rules! non_flat_cell_transport_tests {
    () => {
        mod tests {
            use super::*;
            use ori_domain::ProjectId;

            #[test]
            fn work_preflight_is_inclusive_and_fail_closed() {
                let limits = NonFlatCellTransportLimitsV1 {
                    max_faces: 16,
                    max_cells: 32,
                    max_pairs: 32,
                    max_boundary_points: 128,
                };
                assert_eq!(
                    preflight_non_flat_cell_transport_v1(16, 32, 32, 128, limits),
                    Ok(())
                );
                for rejected in [
                    (0, 0, 0, 0),
                    (17, 32, 32, 128),
                    (16, 33, 33, 128),
                    (16, 32, 31, 128),
                    (16, 32, 32, 129),
                ] {
                    assert_eq!(
                        preflight_non_flat_cell_transport_v1(
                            rejected.0, rejected.1, rejected.2, rejected.3, limits
                        ),
                        Err(NonFlatCellTransportErrorV1::ResourceLimit)
                    );
                }
            }

            /// Owned adversarial fixture. It never builds a public
            /// `StackedFoldNonFlatLayerOrderV1`; it only implements the generic
            /// structural trait so that one condition at a time can be broken.
            #[derive(Clone, Debug, PartialEq)]
            struct StructuralFixture {
                material_faces: Vec<FaceId>,
                folded: Vec<(FaceId, u8, ExactAffineTransform)>,
                cells: Vec<(Vec<Point2>, Vec<ExactPointValue>, FaceId, FaceId)>,
                pairs: Vec<(FaceId, FaceId)>,
            }

            impl NonFlatLayerOrderStructuralSourceV1 for StructuralFixture {
                fn material_face_count(&self) -> usize {
                    self.material_faces.len()
                }
                fn material_face_id(&self, index: usize) -> Option<FaceId> {
                    self.material_faces.get(index).copied()
                }
                fn folded_face_count(&self) -> usize {
                    self.folded.len()
                }
                fn folded_face(
                    &self,
                    index: usize,
                ) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
                    self.folded.get(index).map(|(face_id, axis, transform)| {
                        NonFlatFoldedFaceStructuralRefV1 {
                            face_id: *face_id,
                            dropped_world_axis: *axis,
                            source_to_plane: transform,
                        }
                    })
                }
                fn overlap_cell_count(&self) -> usize {
                    self.cells.len()
                }
                fn overlap_cell(
                    &self,
                    index: usize,
                ) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
                    self.cells
                        .get(index)
                        .map(
                            |(boundary, exact, lower, upper)| NonFlatOverlapCellStructuralRefV1 {
                                boundary,
                                exact_boundary: exact,
                                lower_face: *lower,
                                upper_face: *upper,
                            },
                        )
                }
                fn face_pair_order_count(&self) -> usize {
                    self.pairs.len()
                }
                fn face_pair_order(
                    &self,
                    index: usize,
                ) -> Option<NonFlatFacePairOrderStructuralV1> {
                    self.pairs
                        .get(index)
                        .map(|(lower, upper)| NonFlatFacePairOrderStructuralV1 {
                            lower_face: *lower,
                            upper_face: *upper,
                        })
                }
            }

            struct DeclaredCapacityOverflowFixture;

            impl NonFlatLayerOrderStructuralSourceV1 for DeclaredCapacityOverflowFixture {
                fn material_face_count(&self) -> usize {
                    usize::MAX
                }
                fn material_face_id(&self, _index: usize) -> Option<FaceId> {
                    None
                }
                fn folded_face_count(&self) -> usize {
                    0
                }
                fn folded_face(
                    &self,
                    _index: usize,
                ) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
                    None
                }
                fn overlap_cell_count(&self) -> usize {
                    0
                }
                fn overlap_cell(
                    &self,
                    _index: usize,
                ) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
                    None
                }
                fn face_pair_order_count(&self) -> usize {
                    0
                }
                fn face_pair_order(
                    &self,
                    _index: usize,
                ) -> Option<NonFlatFacePairOrderStructuralV1> {
                    None
                }
            }

            fn exact_integer(value: i8) -> ori_foldability::ExactRationalValue {
                ori_foldability::ExactRationalValue {
                    sign: match value.cmp(&0) {
                        std::cmp::Ordering::Less => ori_foldability::ExactSign::Negative,
                        std::cmp::Ordering::Equal => ori_foldability::ExactSign::Zero,
                        std::cmp::Ordering::Greater => ori_foldability::ExactSign::Positive,
                    },
                    numerator_magnitude_be: if value == 0 {
                        Vec::new()
                    } else {
                        vec![value.unsigned_abs()]
                    },
                    denominator_be: vec![1],
                }
            }

            fn identity_transform() -> ExactAffineTransform {
                ExactAffineTransform {
                    m00: exact_integer(1),
                    m01: exact_integer(0),
                    m10: exact_integer(0),
                    m11: exact_integer(1),
                    tx: exact_integer(0),
                    ty: exact_integer(0),
                }
            }

            fn triangle() -> (Vec<Point2>, Vec<ExactPointValue>) {
                let points = [(0_i8, 0_i8), (1, 0), (0, 1)];
                (
                    points
                        .iter()
                        .map(|(x, y)| Point2::new(f64::from(*x), f64::from(*y)))
                        .collect(),
                    points
                        .iter()
                        .map(|(x, y)| ExactPointValue {
                            x: exact_integer(*x),
                            y: exact_integer(*y),
                        })
                        .collect(),
                )
            }

            fn base_fixture() -> (StructuralFixture, FaceId, FaceId) {
                let a = FaceId::new();
                let b = FaceId::new();
                let (boundary, exact) = triangle();
                (
                    StructuralFixture {
                        material_faces: vec![a, b],
                        folded: vec![(a, 0, identity_transform()), (b, 2, identity_transform())],
                        cells: vec![(boundary, exact, a, b)],
                        pairs: vec![(a, b)],
                    },
                    a,
                    b,
                )
            }

            #[derive(Clone, Debug, PartialEq)]
            struct AuthenticatedFixture {
                structural: StructuralFixture,
                namespace: ProjectId,
                revision: u64,
                authenticated_source_cells: usize,
                target_model_tag: u8,
            }

            impl NonFlatLayerOrderStructuralSourceV1 for AuthenticatedFixture {
                fn material_face_count(&self) -> usize {
                    self.structural.material_face_count()
                }
                fn material_face_id(&self, index: usize) -> Option<FaceId> {
                    self.structural.material_face_id(index)
                }
                fn folded_face_count(&self) -> usize {
                    self.structural.folded_face_count()
                }
                fn folded_face(
                    &self,
                    index: usize,
                ) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
                    self.structural.folded_face(index)
                }
                fn overlap_cell_count(&self) -> usize {
                    self.structural.overlap_cell_count()
                }
                fn overlap_cell(
                    &self,
                    index: usize,
                ) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
                    self.structural.overlap_cell(index)
                }
                fn face_pair_order_count(&self) -> usize {
                    self.structural.face_pair_order_count()
                }
                fn face_pair_order(
                    &self,
                    index: usize,
                ) -> Option<NonFlatFacePairOrderStructuralV1> {
                    self.structural.face_pair_order(index)
                }
            }

            impl NonFlatLayerOrderTransportSourceV1 for AuthenticatedFixture {
                fn identity_namespace_v1(&self) -> ProjectId {
                    self.namespace
                }
                fn target_revision_v1(&self) -> u64 {
                    self.revision
                }
                fn source_overlap_cells_authenticated_v1(&self) -> usize {
                    self.authenticated_source_cells
                }
                fn same_target_model_v1(&self, other: &Self) -> bool {
                    self.target_model_tag == other.target_model_tag
                }
            }

            #[test]
            fn a_generic_source_can_mint_only_its_own_typed_bounded_proof() {
                let (structural, _, _) = base_fixture();
                let namespace = ProjectId::new();
                let source = AuthenticatedFixture {
                    structural: structural.clone(),
                    namespace,
                    revision: 9,
                    authenticated_source_cells: 0,
                    target_model_tag: 1,
                };
                let target = AuthenticatedFixture {
                    structural,
                    namespace,
                    revision: 10,
                    authenticated_source_cells: source.overlap_cell_count(),
                    target_model_tag: 2,
                };
                let proof: NonFlatCellTransportProofV1<AuthenticatedFixture> =
                    certify_non_flat_cell_transport_v1(&source, &target)
                        .expect("the exact generic fixture is admissible");
                assert!(proof.is_for(&source, &target));
                assert_eq!(proof.target(), &target);
                assert_eq!(
                    certify_non_flat_cell_transport_with_limits_v1(
                        &source,
                        &target,
                        NonFlatCellTransportLimitsV1 {
                            max_faces: 1,
                            ..NonFlatCellTransportLimitsV1::default()
                        },
                    ),
                    Err(NonFlatCellTransportErrorV1::ResourceLimit)
                );
            }

            #[test]
            fn the_structural_fixture_baseline_is_complete() {
                let (fixture, _, _) = base_fixture();
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Ok(())
                );
            }

            #[test]
            fn a_missing_folded_face_is_incomplete() {
                let (mut fixture, _, _) = base_fixture();
                fixture.folded.pop();
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn a_duplicate_material_face_is_incomplete() {
                let (mut fixture, a, _) = base_fixture();
                fixture.material_faces = vec![a, a];
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn structural_validation_declared_capacity_overflow_is_resource_limit() {
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(
                        &DeclaredCapacityOverflowFixture,
                    ),
                    Err(NonFlatCellTransportErrorV1::ResourceLimit)
                );
            }

            #[test]
            fn an_out_of_range_dropped_world_axis_is_incomplete() {
                let (mut fixture, _, _) = base_fixture();
                fixture.folded[0].1 = 3;
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn negative_zero_rounded_provenance_is_incomplete() {
                let (mut fixture, _, _) = base_fixture();
                // -0.0 and +0.0 compare equal numerically, so only the to_bits()
                // comparison rejects this rounded/exact provenance mismatch.
                fixture.cells[0].0[0] = Point2::new(-0.0, 0.0);
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn a_pair_that_disagrees_with_its_cell_is_incomplete() {
                let (mut fixture, a, _) = base_fixture();
                let c = FaceId::new();
                fixture.pairs[0] = (a, c);
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn an_unknown_face_in_a_cell_is_incomplete() {
                let (mut fixture, a, _) = base_fixture();
                let c = FaceId::new();
                fixture.cells[0].3 = c;
                fixture.pairs[0] = (a, c);
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn a_self_paired_cell_is_incomplete() {
                let (mut fixture, a, _) = base_fixture();
                fixture.cells[0].3 = a;
                fixture.pairs[0] = (a, a);
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn an_opposite_direction_cell_crosses() {
                let (mut fixture, a, b) = base_fixture();
                let (boundary, exact) = triangle();
                fixture.cells.push((boundary, exact, b, a));
                fixture.pairs.push((b, a));
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::Crossing)
                );
            }

            #[test]
            fn a_cell_and_pair_count_mismatch_is_incomplete() {
                let (mut fixture, _, _) = base_fixture();
                fixture.pairs.clear();
                assert_eq!(
                    validate_non_flat_layer_order_structural_source_v1(&fixture),
                    Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
                );
            }

            #[test]
            fn the_public_structure_validator_grants_no_transport_authority() {
                // Structural validation alone must never behave like certification:
                // it takes one value, returns no proof, and cannot compare revisions.
                let accepts: fn(&StructuralFixture) -> Result<(), NonFlatCellTransportErrorV1> =
                    validate_non_flat_layer_order_structure_v1;
                let _ = accepts;
            }
        }
    };
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
    validate_non_flat_layer_order_structural_source_v1(value)
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
#[derive(Clone, Copy)]
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
/// This is the single definition of the check. It compares no revisions, keeps
/// the original decision order, and fails closed to
/// [`NonFlatCellTransportErrorV1::IncompleteCoverage`] whenever a declared
/// count and the readable data disagree.
fn validate_non_flat_layer_order_structural_source_v1<
    S: NonFlatLayerOrderStructuralSourceV1 + ?Sized,
>(
    source: &S,
) -> Result<(), NonFlatCellTransportErrorV1> {
    let material_count = source.material_face_count();
    let mut faces = HashSet::new();
    faces
        .try_reserve(material_count)
        .map_err(|_| NonFlatCellTransportErrorV1::ResourceLimit)?;
    for index in 0..material_count {
        let face = source
            .material_face_id(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if !faces.insert(face) {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    let folded_count = source.folded_face_count();
    let mut folded_ids = HashSet::new();
    folded_ids
        .try_reserve(folded_count)
        .map_err(|_| NonFlatCellTransportErrorV1::ResourceLimit)?;
    for index in 0..folded_count {
        let folded = source
            .folded_face(index)
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if !folded_ids.insert(folded.face_id) {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    if faces.is_empty()
        || faces.len() != material_count
        || folded_count != faces.len()
        || folded_ids != faces
    {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    for index in 0..source.folded_face_count() {
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
        if folded.dropped_world_axis > 2
            || values
                .into_iter()
                .any(|value| value.to_f64().is_none_or(|value| !value.is_finite()))
        {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    let overlap_count = source.overlap_cell_count();
    if overlap_count != source.face_pair_order_count() {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    let mut directed = HashSet::<(FaceId, FaceId)>::new();
    directed
        .try_reserve(overlap_count)
        .map_err(|_| NonFlatCellTransportErrorV1::ResourceLimit)?;
    for index in 0..overlap_count {
        let (cell, pair) = source
            .overlap_cell(index)
            .zip(source.face_pair_order(index))
            .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if cell.boundary.len() < 3
            || cell.exact_boundary.len() != cell.boundary.len()
            || cell.lower_face != pair.lower_face
            || cell.upper_face != pair.upper_face
            || !faces.contains(&cell.lower_face)
            || !faces.contains(&cell.upper_face)
            || cell.lower_face == cell.upper_face
        {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
        for (point, exact) in cell.boundary.iter().zip(cell.exact_boundary) {
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
        if directed.contains(&(cell.upper_face, cell.lower_face)) {
            return Err(NonFlatCellTransportErrorV1::Crossing);
        }
        if !directed.insert((cell.lower_face, cell.upper_face)) {
            return Err(NonFlatCellTransportErrorV1::Crossing);
        }
    }
    Ok(())
}

#[cfg(test)]
non_flat_cell_transport_tests!();
