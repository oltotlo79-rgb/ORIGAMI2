#![allow(clippy::items_after_test_module)]

use std::collections::HashSet;

use ori_core::StackedFoldNonFlatLayerOrderV1;
use ori_domain::{FaceId, Point2};
use ori_foldability::{ExactAffineTransform, ExactPointValue};
use thiserror::Error;

pub const NON_FLAT_CELL_TRANSPORT_MODEL_ID_V1: &str = "native_non_flat_exact_cell_transport_v1";

#[derive(Debug, Clone, PartialEq)]
pub struct NonFlatCellTransportProofV1 {
    source: StackedFoldNonFlatLayerOrderV1,
    target: StackedFoldNonFlatLayerOrderV1,
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

impl NonFlatCellTransportProofV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        NON_FLAT_CELL_TRANSPORT_MODEL_ID_V1
    }
    #[must_use]
    pub fn target(&self) -> &StackedFoldNonFlatLayerOrderV1 {
        &self.target
    }
    #[must_use]
    pub fn is_for(
        &self,
        source: &StackedFoldNonFlatLayerOrderV1,
        target: &StackedFoldNonFlatLayerOrderV1,
    ) -> bool {
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

pub fn certify_non_flat_cell_transport_v1(
    source: &StackedFoldNonFlatLayerOrderV1,
    target: &StackedFoldNonFlatLayerOrderV1,
) -> Result<NonFlatCellTransportProofV1, NonFlatCellTransportErrorV1> {
    certify_non_flat_cell_transport_with_limits_v1(
        source,
        target,
        NonFlatCellTransportLimitsV1::default(),
    )
}

pub fn certify_non_flat_cell_transport_with_limits_v1(
    source: &StackedFoldNonFlatLayerOrderV1,
    target: &StackedFoldNonFlatLayerOrderV1,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<NonFlatCellTransportProofV1, NonFlatCellTransportErrorV1> {
    let independently_readmitted_same_model = source.target_fingerprint()
        == target.target_fingerprint()
        && source.material_faces() == target.material_faces();
    if source.identity_namespace() != target.identity_namespace()
        || source.target_revision().checked_add(1) != Some(target.target_revision())
        || (target.source_overlap_cells_authenticated() != source.overlap_cell_count()
            && !independently_readmitted_same_model)
    {
        return Err(NonFlatCellTransportErrorV1::BindingMismatch);
    }
    let boundary_points = target
        .overlap_cells()
        .iter()
        .try_fold(0usize, |sum, cell| {
            sum.checked_add(cell.exact_boundary().len())
        })
        .ok_or(NonFlatCellTransportErrorV1::ResourceLimit)?;
    preflight_non_flat_cell_transport_v1(
        target.material_faces().len(),
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
mod tests {
    use super::*;
    use ori_core::revalidate_current_non_flat_layer_order_v1;
    use ori_domain::{Edge, EdgeId, EdgeKind, ProjectId};
    use ori_kinematics::{CanonicalHingeAngles, HingeAngle};
    use ori_topology::{FaceExtractionInput, analyze_faces};

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

    #[test]
    fn current_tree_admission_transports_exact_non_flat_evidence() {
        let project = ProjectId::new();
        let sheet = ori_core::create_rectangular_sheet(100.0, 100.0, false).unwrap();
        let (mut pattern, paper) = sheet.into_parts();
        let hinge = EdgeId::new();
        pattern.edges.push(Edge {
            id: hinge,
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let flat = |revision| {
            let topology = analyze_faces(FaceExtractionInput {
                identity_namespace: project,
                source_revision: revision,
                paper: &paper,
                pattern: &pattern,
            })
            .snapshot
            .unwrap();
            let local = ori_topology::analyze_local_flat_foldability(&paper, &pattern);
            ori_foldability::analyze_global_flat_foldability(
                ori_foldability::GlobalFlatFoldabilityInput::current_with_geometry(
                    project, &paper, &pattern, &topology, &local,
                ),
                ori_foldability::GlobalFlatFoldabilityLimits::default(),
            )
            .unwrap()
            .layer_order()
            .unwrap()
            .clone()
        };
        let angles =
            CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 90.0).unwrap()]).unwrap();
        let source_flat = flat(1);
        let fixed = source_flat.material_faces[0].face_id;
        let source = revalidate_current_non_flat_layer_order_v1(
            project,
            1,
            &pattern,
            &paper,
            Some(fixed),
            &angles,
            &source_flat,
            1,
        )
        .unwrap();
        let target_flat = flat(2);
        let target = revalidate_current_non_flat_layer_order_v1(
            project,
            2,
            &pattern,
            &paper,
            Some(fixed),
            &angles,
            &target_flat,
            1,
        )
        .unwrap();
        let proof = certify_non_flat_cell_transport_v1(&source, &target).unwrap();
        assert!(proof.is_for(&source, &target));
        assert_eq!(proof.target().folded_faces().len(), 2);
        assert!(matches!(
            certify_non_flat_cell_transport_v1(&source, &source),
            Err(NonFlatCellTransportErrorV1::BindingMismatch)
        ));
        assert!(matches!(
            certify_non_flat_cell_transport_with_limits_v1(
                &source,
                &target,
                NonFlatCellTransportLimitsV1 {
                    max_faces: 1,
                    ..NonFlatCellTransportLimitsV1::default()
                },
            ),
            Err(NonFlatCellTransportErrorV1::ResourceLimit)
        ));
        let different = revalidate_current_non_flat_layer_order_v1(
            project,
            2,
            &pattern,
            &paper,
            Some(fixed),
            &CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 80.0).unwrap()]).unwrap(),
            &target_flat,
            1,
        )
        .unwrap();
        assert!(!proof.is_for(&source, &different));

        // The certification path and read-only callers must share one
        // definition of structural completeness.
        assert_eq!(validate_non_flat_layer_order_structure_v1(&source), Ok(()));
        assert_eq!(validate_non_flat_layer_order_structure_v1(&target), Ok(()));
        assert_eq!(
            validate_non_flat_layer_order_structure_v1(proof.target()),
            Ok(())
        );
    }

    /// Owned adversarial fixture. It never builds a public
    /// `StackedFoldNonFlatLayerOrderV1`; it only implements the private
    /// structural trait so that one condition at a time can be broken.
    #[derive(Clone)]
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
        fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
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
        fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
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
        fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
            self.pairs
                .get(index)
                .map(|(lower, upper)| NonFlatFacePairOrderStructuralV1 {
                    lower_face: *lower,
                    upper_face: *upper,
                })
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
        let accepts: fn(
            &StackedFoldNonFlatLayerOrderV1,
        ) -> Result<(), NonFlatCellTransportErrorV1> = validate_non_flat_layer_order_structure_v1;
        let _ = accepts;
    }
}

/// Validates the structural completeness of one non-flat layer order.
///
/// This is the single definition consumed both by
/// [`certify_non_flat_cell_transport_with_limits_v1`] and by read-only viewers
/// that must not receive transport binding or transition authority. It performs
/// validation only: it never issues a proof, a capability, or any mutation
/// authority, and it does not compare two revisions.
pub fn validate_non_flat_layer_order_structure_v1(
    value: &StackedFoldNonFlatLayerOrderV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    validate_non_flat_layer_order_structural_source_v1(&CoreNonFlatLayerOrderStructuralViewV1(
        value,
    ))
}

/// Borrowed structural view of one folded face.
struct NonFlatFoldedFaceStructuralRefV1<'a> {
    face_id: FaceId,
    dropped_world_axis: u8,
    source_to_plane: &'a ExactAffineTransform,
}

/// Borrowed structural view of one overlap cell.
struct NonFlatOverlapCellStructuralRefV1<'a> {
    boundary: &'a [Point2],
    exact_boundary: &'a [ExactPointValue],
    lower_face: FaceId,
    upper_face: FaceId,
}

/// Structural view of one directed face-pair order.
#[derive(Clone, Copy)]
struct NonFlatFacePairOrderStructuralV1 {
    lower_face: FaceId,
    upper_face: FaceId,
}

/// Read-only structural source consumed by the completeness validator.
///
/// The trait is private to this module and is never re-exported. It exists so
/// that the one validator body can be exercised against adversarial fixtures
/// without adding any constructor, setter, or serialization surface to the
/// core evidence types. Implementations borrow their data and grant no proof,
/// capability, or mutation authority.
trait NonFlatLayerOrderStructuralSourceV1 {
    fn material_face_count(&self) -> usize;
    fn material_face_id(&self, index: usize) -> Option<FaceId>;
    fn folded_face_count(&self) -> usize;
    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>>;
    fn overlap_cell_count(&self) -> usize;
    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>>;
    fn face_pair_order_count(&self) -> usize;
    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1>;
}

/// Borrowing adapter over authenticated core evidence.
struct CoreNonFlatLayerOrderStructuralViewV1<'a>(&'a StackedFoldNonFlatLayerOrderV1);

impl NonFlatLayerOrderStructuralSourceV1 for CoreNonFlatLayerOrderStructuralViewV1<'_> {
    fn material_face_count(&self) -> usize {
        self.0.material_faces().len()
    }

    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        self.0.material_faces().get(index).map(|face| face.face_id)
    }

    fn folded_face_count(&self) -> usize {
        self.0.folded_faces().len()
    }

    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        self.0
            .folded_faces()
            .get(index)
            .map(|folded| NonFlatFoldedFaceStructuralRefV1 {
                face_id: folded.face().face_id,
                dropped_world_axis: folded.dropped_world_axis(),
                source_to_plane: folded.source_to_plane(),
            })
    }

    fn overlap_cell_count(&self) -> usize {
        self.0.overlap_cells().len()
    }

    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        self.0
            .overlap_cells()
            .get(index)
            .map(|cell| NonFlatOverlapCellStructuralRefV1 {
                boundary: cell.boundary(),
                exact_boundary: cell.exact_boundary(),
                lower_face: cell.lower_face(),
                upper_face: cell.upper_face(),
            })
    }

    fn face_pair_order_count(&self) -> usize {
        self.0.face_pair_orders().len()
    }

    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        self.0
            .face_pair_orders()
            .get(index)
            .map(|pair| NonFlatFacePairOrderStructuralV1 {
                lower_face: pair.lower_face(),
                upper_face: pair.upper_face(),
            })
    }
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
    let faces = (0..material_count)
        .map(|index| source.material_face_id(index))
        .collect::<Option<HashSet<_>>>()
        .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
    let folded_ids = (0..source.folded_face_count())
        .map(|index| source.folded_face(index).map(|folded| folded.face_id))
        .collect::<Option<HashSet<_>>>()
        .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
    if faces.is_empty()
        || faces.len() != material_count
        || source.folded_face_count() != faces.len()
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
        ]
        .into_iter()
        .map(|value| value.to_f64())
        .collect::<Option<Vec<_>>>()
        .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if folded.dropped_world_axis > 2 || values.iter().any(|value| !value.is_finite()) {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    if source.overlap_cell_count() != source.face_pair_order_count() {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    let mut directed = HashSet::<(FaceId, FaceId)>::new();
    for index in 0..source.overlap_cell_count() {
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
        directed.insert((cell.lower_face, cell.upper_face));
    }
    Ok(())
}
