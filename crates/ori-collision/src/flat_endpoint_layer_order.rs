//! Observation-only transport of a certified flat layer order to one exact
//! native material pose. Version 1 admits either the no-hinge single-face
//! identity pose or a material tree whose every hinge is bit-exact 180 degrees.
//!
//! This module deliberately does not authenticate a public
//! [`LayerOrderSnapshot`]. The desktop's private current-layer-order guard must
//! supply the exact snapshot object while it remains current. The opaque anchor
//! binds that object by address, the exact kinematics issuer, and the exact
//! pose instance; it cannot turn copied transport data into solver authority.
//!
//! Multi-face [`crate::NativeStaticCollisionGeometryProof`] is not currently
//! issuable. Consequently this anchor contains no static-collision, positive
//! thickness, continuous-motion, project-mutation, revision-generation, or
//! shared-hinge admission authority. This first transport slice also fails
//! closed unless every material face and overlap cell is convex; extending
//! exact coverage verification to concave faces is separate future work.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use ori_domain::{CreasePattern, EdgeId, FaceId, Paper, Point2, ProjectId, VertexId};
use ori_foldability::{
    ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign, FoldModelFingerprintV1,
    FoldedFaceOrientation, GLOBAL_FLAT_FOLDABILITY_MODEL_ID, LAYER_ORDER_MODEL_ID, LayerFace,
    LayerOrderDerivation, LayerOrderSnapshot, OverlapCellKey, fold_model_fingerprint_v1,
};
use ori_kinematics::{MaterialTreeKinematicsModel, MaterialTreePose, Point3};
use ori_topology::{
    FaceExtractionInput, HalfEdgeRef, TopologySnapshot, analyze_faces, canonical_face_key,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const FLAT_ENDPOINT_LAYER_ORDER_ANCHOR_MODEL_ID_V1: &str =
    "flat_endpoint_layer_order_anchor_v1";

const CELL_KEY_DOMAIN: &[u8] = b"ORIGAMI2\0overlap-cell\0v1\0";

#[derive(Debug, Clone, Copy)]
pub struct FlatEndpointLayerOrderInputV1<'source, 'snapshot> {
    pub identity_namespace: ProjectId,
    pub source_revision: u64,
    pub paper: &'source Paper,
    pub pattern: &'source CreasePattern,
    pub model: &'source MaterialTreeKinematicsModel,
    pub pose: &'source MaterialTreePose,
    /// This is a premise, not an authenticated certificate. Callers must
    /// obtain it from their private current-layer-order guard.
    pub layer_order: &'snapshot LayerOrderSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatEndpointLayerOrderLimitsV1 {
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_paper_boundary_vertices: usize,
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_cells: usize,
    pub max_boundary_vertices_per_cell: usize,
    pub max_total_boundary_vertices: usize,
    pub max_total_layer_records: usize,
    pub max_face_pair_orders: usize,
    pub max_total_supporting_cells: usize,
    pub max_exact_payload_bytes: usize,
    pub max_exact_integer_bits: usize,
    pub max_containment_orientation_tests: usize,
    pub max_cell_separation_orientation_tests: usize,
}

impl Default for FlatEndpointLayerOrderLimitsV1 {
    fn default() -> Self {
        Self {
            max_source_vertices: 100_000,
            max_source_edges: 100_000,
            max_paper_boundary_vertices: 100_000,
            max_faces: 10_001,
            max_hinges: 10_000,
            max_cells: 100_000,
            max_boundary_vertices_per_cell: 4_096,
            max_total_boundary_vertices: 500_000,
            max_total_layer_records: 1_000_000,
            max_face_pair_orders: 500_000,
            max_total_supporting_cells: 1_000_000,
            max_exact_payload_bytes: 128 * 1024 * 1024,
            max_exact_integer_bits: 65_536,
            max_containment_orientation_tests: 100_000_000,
            max_cell_separation_orientation_tests: 100_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatEndpointLayerOrderResourceV1 {
    SourceVertices,
    SourceEdges,
    PaperBoundaryVertices,
    Faces,
    Hinges,
    Cells,
    BoundaryVerticesPerCell,
    TotalBoundaryVertices,
    LayerRecords,
    FacePairOrders,
    SupportingCells,
    ExactPayloadBytes,
    ExactIntegerBits,
    ContainmentOrientationTests,
    CellSeparationOrientationTests,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlatEndpointLayerOrderWorkV1 {
    pub faces: usize,
    pub hinges: usize,
    pub cells: usize,
    pub total_boundary_vertices: usize,
    pub total_layer_records: usize,
    pub face_pair_orders: usize,
    pub total_supporting_cells: usize,
    pub exact_payload_bytes: usize,
    pub maximum_exact_integer_bits: usize,
    pub containment_orientation_tests: usize,
    pub cell_separation_orientation_tests: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FlatEndpointLayerOrderAnchorErrorV1 {
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimitExceeded {
        resource: FlatEndpointLayerOrderResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("flat-endpoint layer-order resource counting overflowed")]
    ResourceCountOverflow,
    #[error("flat-endpoint layer-order storage allocation failed")]
    AllocationFailed,
    #[error("the source identity or revision does not match the layer-order premise")]
    SourceIdentityMismatch,
    #[error("the source fold-model fingerprint does not match the layer-order premise")]
    SourceFingerprintMismatch,
    #[error("the immutable source geometry could not be reconstructed safely")]
    SourceGeometryUnavailable,
    #[error("the kinematics model does not match the immutable source face registry")]
    ModelSourceMismatch,
    #[error("the material pose was issued by a different model instance")]
    PoseIssuerMismatch,
    #[error(
        "the flat-endpoint anchor requires either one face without hinges or a multi-face tree ({faces} faces, {hinges} hinges)"
    )]
    UnsupportedPoseClass { faces: usize, hinges: usize },
    #[error("hinge {edge:?} is not at the bit-exact 180-degree endpoint")]
    NotBitExactFlatEndpoint { edge: EdgeId },
    #[error("the pose root is not compatible with the layer-order reference face")]
    ReferenceFaceMismatch,
    #[error("the supplied layer-order model or derivation is not the admitted flat model")]
    LayerOrderModelMismatch,
    #[error("the layer-order material face registry is incomplete or foreign")]
    MaterialFaceRegistryMismatch,
    #[error("the exact layer-order payload is noncanonical, malformed, or unrepresentable")]
    InvalidExactPayload,
    #[error("folded face {face:?} does not match the exact current flat endpoint")]
    FoldedFaceTransformMismatch { face: FaceId },
    #[error("the overlap-cell registry, geometry, or coverage is incomplete")]
    CellCompletenessMismatch,
    #[error("the cell-local or face-pair layer order is inconsistent")]
    CellOrderMismatch,
    #[error("the optional whole-model order contradicts a cell-local order")]
    GlobalOrderMismatch,
    #[error("the anchor is not bound to the exact supplied model, pose, and snapshot objects")]
    AuthorityBindingMismatch,
    #[error("immutable flat-endpoint layer-order revalidation failed")]
    AnchorReverificationFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlatEndpointCellKeyV1([u8; 32]);

impl FlatEndpointCellKeyV1 {
    #[must_use]
    pub const fn canonical_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlatEndpointLayerCellV1 {
    cell_key: FlatEndpointCellKeyV1,
    world_boundary: Vec<Point3>,
    covering_faces: Vec<FaceId>,
    bottom_to_top_faces: Vec<FaceId>,
}

impl FlatEndpointLayerCellV1 {
    #[must_use]
    pub const fn cell_key(&self) -> FlatEndpointCellKeyV1 {
        self.cell_key
    }

    #[must_use]
    pub fn world_boundary(&self) -> &[Point3] {
        &self.world_boundary
    }

    #[must_use]
    pub fn covering_faces(&self) -> &[FaceId] {
        &self.covering_faces
    }

    #[must_use]
    pub fn bottom_to_top_faces(&self) -> &[FaceId] {
        &self.bottom_to_top_faces
    }
}

#[derive(Debug)]
struct FlatEndpointLayerOrderAnchorProofV1 {
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
    identity_namespace: ProjectId,
    source_revision: u64,
    source_fingerprint: FoldModelFingerprintV1,
    material_faces: Vec<LayerFace>,
    exact_cells: Vec<CellGeometry>,
    cells: Vec<FlatEndpointLayerCellV1>,
    work: FlatEndpointLayerOrderWorkV1,
}

/// Opaque, observation-only binding of one exact facewise snapshot object to
/// one exact native flat pose.
///
/// Cloning preserves anchor identity. Re-solving the same 180-degree vector or
/// re-running flat-foldability produces a different authority object and is
/// rejected by [`Self::is_for_authorities`].
///
/// This type does not implement serialization.
///
/// ```compile_fail
/// use ori_collision::NativeFlatEndpointLayerOrderAnchorV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<NativeFlatEndpointLayerOrderAnchorV1<'static>>();
/// ```
#[derive(Debug, Clone)]
pub struct NativeFlatEndpointLayerOrderAnchorV1<'snapshot> {
    proof: Arc<FlatEndpointLayerOrderAnchorProofV1>,
    snapshot: &'snapshot LayerOrderSnapshot,
}

impl PartialEq for NativeFlatEndpointLayerOrderAnchorV1<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.same_anchor(other)
    }
}

impl Eq for NativeFlatEndpointLayerOrderAnchorV1<'_> {}

impl<'snapshot> NativeFlatEndpointLayerOrderAnchorV1<'snapshot> {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        FLAT_ENDPOINT_LAYER_ORDER_ANCHOR_MODEL_ID_V1
    }

    #[must_use]
    pub fn same_anchor(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.proof, &other.proof)
    }

    #[must_use]
    pub fn source_fingerprint(&self) -> FoldModelFingerprintV1 {
        self.proof.source_fingerprint
    }

    #[must_use]
    pub fn material_faces(&self) -> &[LayerFace] {
        &self.proof.material_faces
    }

    #[must_use]
    pub fn cells(&self) -> &[FlatEndpointLayerCellV1] {
        &self.proof.cells
    }

    pub(crate) fn exact_cells(&self) -> &[CellGeometry] {
        &self.proof.exact_cells
    }

    pub(crate) fn identity_namespace(&self) -> ProjectId {
        self.proof.identity_namespace
    }

    pub(crate) fn source_revision(&self) -> u64 {
        self.proof.source_revision
    }

    pub(crate) const fn snapshot(&self) -> &'snapshot LayerOrderSnapshot {
        self.snapshot
    }

    #[must_use]
    pub fn work(&self) -> FlatEndpointLayerOrderWorkV1 {
        self.proof.work
    }

    #[must_use]
    pub fn is_for_authorities(
        &self,
        model: &MaterialTreeKinematicsModel,
        pose: &MaterialTreePose,
        snapshot: &LayerOrderSnapshot,
    ) -> bool {
        self.proof.model == *model
            && self.proof.pose.same_instance(pose)
            && std::ptr::eq(self.snapshot, snapshot)
    }
}

struct Analysis {
    identity_namespace: ProjectId,
    source_revision: u64,
    source_fingerprint: FoldModelFingerprintV1,
    material_faces: Vec<LayerFace>,
    exact_cells: Vec<CellGeometry>,
    cells: Vec<FlatEndpointLayerCellV1>,
    work: FlatEndpointLayerOrderWorkV1,
}

pub fn anchor_flat_endpoint_layer_order_v1<'snapshot>(
    input: FlatEndpointLayerOrderInputV1<'_, 'snapshot>,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<NativeFlatEndpointLayerOrderAnchorV1<'snapshot>, FlatEndpointLayerOrderAnchorErrorV1> {
    let analysis = analyze(input, limits)?;
    Ok(NativeFlatEndpointLayerOrderAnchorV1 {
        proof: Arc::new(FlatEndpointLayerOrderAnchorProofV1 {
            model: input.model.clone(),
            pose: input.pose.clone(),
            identity_namespace: analysis.identity_namespace,
            source_revision: analysis.source_revision,
            source_fingerprint: analysis.source_fingerprint,
            material_faces: analysis.material_faces,
            exact_cells: analysis.exact_cells,
            cells: analysis.cells,
            work: analysis.work,
        }),
        snapshot: input.layer_order,
    })
}

pub fn revalidate_flat_endpoint_layer_order_anchor_v1(
    anchor: &NativeFlatEndpointLayerOrderAnchorV1<'_>,
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    if !anchor.is_for_authorities(input.model, input.pose, input.layer_order) {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch);
    }
    let analysis = analyze(input, limits)?;
    if anchor.proof.identity_namespace != analysis.identity_namespace
        || anchor.proof.source_revision != analysis.source_revision
        || anchor.proof.source_fingerprint != analysis.source_fingerprint
        || anchor.proof.material_faces != analysis.material_faces
        || anchor.proof.exact_cells != analysis.exact_cells
        || anchor.proof.work != analysis.work
        || !cells_bit_exact_equal(&anchor.proof.cells, &analysis.cells)
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::AnchorReverificationFailed);
    }
    Ok(())
}

#[derive(Default)]
struct WorkTracker {
    work: FlatEndpointLayerOrderWorkV1,
}

impl WorkTracker {
    fn charge_exact(
        &mut self,
        value: &ExactRationalValue,
        limits: FlatEndpointLayerOrderLimitsV1,
    ) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
        let bytes = value
            .numerator_magnitude_be
            .len()
            .checked_add(value.denominator_be.len())
            .and_then(|value| value.checked_add(17))
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        self.work.exact_payload_bytes = self
            .work
            .exact_payload_bytes
            .checked_add(bytes)
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        check_limit(
            FlatEndpointLayerOrderResourceV1::ExactPayloadBytes,
            self.work.exact_payload_bytes,
            limits.max_exact_payload_bytes,
        )?;
        let bits = value
            .numerator_magnitude_be
            .len()
            .max(value.denominator_be.len())
            .checked_mul(8)
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        self.work.maximum_exact_integer_bits = self.work.maximum_exact_integer_bits.max(bits);
        check_limit(
            FlatEndpointLayerOrderResourceV1::ExactIntegerBits,
            self.work.maximum_exact_integer_bits,
            limits.max_exact_integer_bits,
        )
    }

    fn charge_containment(
        &mut self,
        amount: usize,
        limits: FlatEndpointLayerOrderLimitsV1,
    ) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
        self.work.containment_orientation_tests = self
            .work
            .containment_orientation_tests
            .checked_add(amount)
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        check_limit(
            FlatEndpointLayerOrderResourceV1::ContainmentOrientationTests,
            self.work.containment_orientation_tests,
            limits.max_containment_orientation_tests,
        )
    }

    fn charge_separation(
        &mut self,
        amount: usize,
        limits: FlatEndpointLayerOrderLimitsV1,
    ) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
        self.work.cell_separation_orientation_tests = self
            .work
            .cell_separation_orientation_tests
            .checked_add(amount)
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        check_limit(
            FlatEndpointLayerOrderResourceV1::CellSeparationOrientationTests,
            self.work.cell_separation_orientation_tests,
            limits.max_cell_separation_orientation_tests,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RationalPoint {
    pub(crate) x: BigRational,
    pub(crate) y: BigRational,
}

#[derive(Debug, Clone)]
struct RationalTransform {
    m00: BigRational,
    m01: BigRational,
    m10: BigRational,
    m11: BigRational,
    tx: BigRational,
    ty: BigRational,
}

impl RationalTransform {
    fn apply(&self, point: &RationalPoint) -> RationalPoint {
        RationalPoint {
            x: &self.m00 * &point.x + &self.m01 * &point.y + &self.tx,
            y: &self.m10 * &point.x + &self.m11 * &point.y + &self.ty,
        }
    }

    fn determinant(&self) -> BigRational {
        &self.m00 * &self.m11 - &self.m01 * &self.m10
    }

    fn is_isometry(&self) -> bool {
        let one = BigRational::one();
        let zero = BigRational::zero();
        &self.m00 * &self.m00 + &self.m10 * &self.m10 == one
            && &self.m01 * &self.m01 + &self.m11 * &self.m11 == one
            && &self.m00 * &self.m01 + &self.m10 * &self.m11 == zero
            && self.determinant().abs() == one
    }
}

struct FaceGeometry {
    polygon: Vec<RationalPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellGeometry {
    pub(crate) key: OverlapCellKey,
    pub(crate) polygon: Vec<RationalPoint>,
    pub(crate) covering_indices: Vec<usize>,
    pub(crate) bottom_indices: Vec<usize>,
    pub(crate) exact_area: BigRational,
}

fn analyze(
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<Analysis, FlatEndpointLayerOrderAnchorErrorV1> {
    check_limit(
        FlatEndpointLayerOrderResourceV1::SourceVertices,
        input.pattern.vertices.len(),
        limits.max_source_vertices,
    )?;
    check_limit(
        FlatEndpointLayerOrderResourceV1::SourceEdges,
        input.pattern.edges.len(),
        limits.max_source_edges,
    )?;
    check_limit(
        FlatEndpointLayerOrderResourceV1::PaperBoundaryVertices,
        input.paper.boundary_vertices.len(),
        limits.max_paper_boundary_vertices,
    )?;
    input
        .model
        .bind_pose(input.pose)
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::PoseIssuerMismatch)?;

    let face_count = input.pose.face_ids().len();
    let hinge_count = input.pose.hinges().len();
    check_limit(
        FlatEndpointLayerOrderResourceV1::Faces,
        face_count,
        limits.max_faces,
    )?;
    check_limit(
        FlatEndpointLayerOrderResourceV1::Hinges,
        hinge_count,
        limits.max_hinges,
    )?;
    let single_face = face_count == 1 && hinge_count == 0;
    let flat_tree = face_count >= 2 && hinge_count == face_count.saturating_sub(1);
    if !single_face && !flat_tree {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::UnsupportedPoseClass {
            faces: face_count,
            hinges: hinge_count,
        });
    }
    if input.pose.face_ids() != input.model.face_ids()
        || input.pose.hinge_angles().len() != hinge_count
        || input.pose.hinges() != input.model.hinges()
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch);
    }
    for (hinge, angle) in input.pose.hinges().iter().zip(input.pose.hinge_angles()) {
        if hinge.edge() != angle.edge() || angle.angle_degrees().to_bits() != 180.0_f64.to_bits() {
            return Err(
                FlatEndpointLayerOrderAnchorErrorV1::NotBitExactFlatEndpoint { edge: hinge.edge() },
            );
        }
    }

    let source = input.layer_order.provenance.source;
    if input.identity_namespace.canonical_bytes() == [0; 16]
        || source.identity_namespace != Some(input.identity_namespace)
        || source.source_revision != input.source_revision
        || source.model_id != GLOBAL_FLAT_FOLDABILITY_MODEL_ID
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::SourceIdentityMismatch);
    }
    let fingerprint = fold_model_fingerprint_v1(input.pattern, input.paper);
    if source.source_fingerprint != Some(fingerprint) {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::SourceFingerprintMismatch);
    }
    if input.layer_order.model_id != LAYER_ORDER_MODEL_ID {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch);
    }

    let topology = reconstruct_topology(input)?;
    let registry = canonical_material_registry(&topology)?;
    if registry.len() != face_count
        || input.layer_order.material_faces != registry
        || input.layer_order.folded_faces.len() != face_count
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::MaterialFaceRegistryMismatch);
    }
    validate_model_source(input, &topology, &registry)?;

    let reference = input
        .layer_order
        .reference_face
        .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ReferenceFaceMismatch)?;
    let expected_root = if single_face {
        None
    } else {
        Some(reference.face_id)
    };
    if registry.first().copied() != Some(reference) || input.pose.fixed_face() != expected_root {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::ReferenceFaceMismatch);
    }
    validate_derivation(input, &topology, &registry, reference)?;

    let vertices = source_vertex_positions(input.pattern)?;
    let registry_indices = registry
        .iter()
        .enumerate()
        .map(|(index, face)| (face.face_id.canonical_bytes(), index))
        .collect::<BTreeMap<_, _>>();
    let mut tracker = WorkTracker::default();
    tracker.work.faces = face_count;
    tracker.work.hinges = hinge_count;
    let face_geometry = validate_folded_faces(
        input,
        &topology,
        &registry,
        &registry_indices,
        &vertices,
        &mut tracker,
        limits,
    )?;
    let (cell_geometry, cells) = validate_cells(
        input.layer_order,
        &registry,
        &registry_indices,
        &face_geometry,
        &mut tracker,
        limits,
    )?;
    validate_cell_coverage(&face_geometry, &cell_geometry, &mut tracker, limits)?;
    validate_orders(
        input.layer_order,
        &registry,
        &registry_indices,
        &cell_geometry,
        &mut tracker,
        limits,
    )?;

    Ok(Analysis {
        identity_namespace: input.identity_namespace,
        source_revision: input.source_revision,
        source_fingerprint: fingerprint,
        material_faces: registry,
        exact_cells: cell_geometry,
        cells,
        work: tracker.work,
    })
}

fn reconstruct_topology(
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
) -> Result<TopologySnapshot, FlatEndpointLayerOrderAnchorErrorV1> {
    analyze_faces(FaceExtractionInput {
        identity_namespace: input.identity_namespace,
        source_revision: input.source_revision,
        paper: input.paper,
        pattern: input.pattern,
    })
    .snapshot
    .ok_or(FlatEndpointLayerOrderAnchorErrorV1::SourceGeometryUnavailable)
}

fn canonical_material_registry(
    topology: &TopologySnapshot,
) -> Result<Vec<LayerFace>, FlatEndpointLayerOrderAnchorErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(topology.faces.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    result.extend(topology.faces.iter().map(|face| LayerFace {
        face_id: face.id,
        face_key: face.key,
    }));
    result.sort_unstable_by_key(|face| (face.face_key, face.face_id.canonical_bytes()));
    Ok(result)
}

fn source_vertex_positions(
    pattern: &CreasePattern,
) -> Result<HashMap<VertexId, Point2>, FlatEndpointLayerOrderAnchorErrorV1> {
    let mut result = HashMap::new();
    result
        .try_reserve(pattern.vertices.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    for vertex in &pattern.vertices {
        if !vertex.position.x.is_finite()
            || !vertex.position.y.is_finite()
            || vertex.position.x.to_bits() == (-0.0_f64).to_bits()
            || vertex.position.y.to_bits() == (-0.0_f64).to_bits()
            || result.insert(vertex.id, vertex.position).is_some()
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::SourceGeometryUnavailable);
        }
    }
    Ok(result)
}

fn validate_model_source(
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    topology: &TopologySnapshot,
    registry: &[LayerFace],
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    let topology_faces = topology
        .faces
        .iter()
        .map(|face| (face.id.canonical_bytes(), face))
        .collect::<BTreeMap<_, _>>();
    let vertices = source_vertex_positions(input.pattern)?;
    let model_ids = input
        .model
        .face_ids()
        .iter()
        .map(FaceId::canonical_bytes)
        .collect::<BTreeSet<_>>();
    let registry_ids = registry
        .iter()
        .map(|face| face.face_id.canonical_bytes())
        .collect::<BTreeSet<_>>();
    if model_ids != registry_ids {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch);
    }
    for layer in registry {
        let boundary = input
            .model
            .face_boundary(layer.face_id)
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch)?;
        let half_edges = boundary_half_edges(boundary.vertices(), boundary.edges())?;
        if canonical_face_key(&half_edges).ok() != Some(layer.face_key)
            || topology_faces
                .get(&layer.face_id.canonical_bytes())
                .is_none_or(|face| face.key != layer.face_key)
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch);
        }
        for vertex in boundary.vertices() {
            let source = vertices
                .get(vertex)
                .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch)?;
            let actual = input
                .model
                .vertex_position(*vertex)
                .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch)?;
            if point3_bits(actual)
                != [
                    source.x.to_bits(),
                    0.0_f64.to_bits(),
                    canonical_zero(-source.y).to_bits(),
                ]
            {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch);
            }
        }
    }

    let expected_hinges = topology
        .hinge_adjacency
        .iter()
        .map(|hinge| {
            (
                hinge.edge.canonical_bytes(),
                hinge.assignment,
                unordered_face_pair(hinge.first, hinge.second),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual_hinges = input
        .model
        .hinges()
        .iter()
        .map(|hinge| {
            (
                hinge.edge().canonical_bytes(),
                hinge.assignment(),
                unordered_face_pair(hinge.left_face(), hinge.right_face()),
            )
        })
        .collect::<BTreeSet<_>>();
    if expected_hinges != actual_hinges || actual_hinges.len() != input.model.hinges().len() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch);
    }
    Ok(())
}

fn boundary_half_edges(
    vertices: &[VertexId],
    edges: &[EdgeId],
) -> Result<Vec<HalfEdgeRef>, FlatEndpointLayerOrderAnchorErrorV1> {
    if vertices.len() < 3 || vertices.len() != edges.len() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::ModelSourceMismatch);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(vertices.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    for index in 0..vertices.len() {
        result.push(HalfEdgeRef {
            edge: edges[index],
            origin: vertices[index],
            destination: vertices[(index + 1) % vertices.len()],
        });
    }
    Ok(result)
}

fn validate_derivation(
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    topology: &TopologySnapshot,
    registry: &[LayerFace],
    reference: LayerFace,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    match input.layer_order.provenance.derivation {
        LayerOrderDerivation::SingleFace { face }
            if registry.len() == 1 && topology.hinge_adjacency.is_empty() =>
        {
            if face != reference || registry.first().copied() != Some(face) {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch);
            }
        }
        LayerOrderDerivation::SingleHinge {
            hinge_edge,
            assignment,
            canonical_first,
            canonical_second,
        } if registry.len() == 2 && topology.hinge_adjacency.len() == 1 => {
            let hinge = topology.hinge_adjacency[0];
            if hinge_edge != hinge.edge
                || assignment != hinge.assignment
                || canonical_first != registry[0]
                || canonical_second != registry[1]
            {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch);
            }
        }
        LayerOrderDerivation::FacewiseCertificate {
            reference_face,
            overlap_cell_count,
            constraint_count,
        } if registry.len() > 2 => {
            if reference_face != reference
                || overlap_cell_count != input.layer_order.overlap_cells.len()
                || input
                    .layer_order
                    .proof_summary
                    .is_none_or(|summary| summary.constraints != constraint_count)
            {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch);
            }
        }
        _ => return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch),
    }
    Ok(())
}

fn validate_folded_faces(
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    topology: &TopologySnapshot,
    registry: &[LayerFace],
    registry_indices: &BTreeMap<[u8; 16], usize>,
    vertices: &HashMap<VertexId, Point2>,
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<Vec<FaceGeometry>, FlatEndpointLayerOrderAnchorErrorV1> {
    let topology_faces = topology
        .faces
        .iter()
        .map(|face| (face.id.canonical_bytes(), face))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    result
        .try_reserve_exact(registry.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    for (expected_index, folded) in input.layer_order.folded_faces.iter().enumerate() {
        let Some(&index) = registry_indices.get(&folded.face.face_id.canonical_bytes()) else {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::MaterialFaceRegistryMismatch);
        };
        if index != expected_index || folded.face != registry[index] || !seen.insert(index) {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::MaterialFaceRegistryMismatch);
        }
        let transform = parse_transform(&folded.source_to_flat, tracker, limits)?;
        if !transform.is_isometry() {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload);
        }
        let determinant = transform.determinant();
        let expected_orientation = if determinant.is_positive() {
            FoldedFaceOrientation::FrontUp
        } else {
            FoldedFaceOrientation::BackUp
        };
        if folded.orientation != expected_orientation {
            return Err(
                FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch {
                    face: folded.face.face_id,
                },
            );
        }
        let source_face = topology_faces[&folded.face.face_id.canonical_bytes()];
        let mut polygon = Vec::new();
        polygon
            .try_reserve_exact(source_face.outer.half_edges.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        let pose_transform = input.pose.face_transform(folded.face.face_id).ok_or(
            FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch {
                face: folded.face.face_id,
            },
        )?;
        validate_planar_pose_transform(pose_transform, folded.orientation, folded.face.face_id)?;
        for half_edge in &source_face.outer.half_edges {
            let source = vertices.get(&half_edge.origin).ok_or(
                FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch {
                    face: folded.face.face_id,
                },
            )?;
            let source_exact = rational_point_from_binary64(*source)?;
            let flat = transform.apply(&source_exact);
            let material = input.model.vertex_position(half_edge.origin).ok_or(
                FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch {
                    face: folded.face.face_id,
                },
            )?;
            let world = pose_transform.apply_point(material).map_err(|_| {
                FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch {
                    face: folded.face.face_id,
                }
            })?;
            let flat_world = rational_point_to_world(&flat)?;
            if point3_bits(world) != point3_bits(flat_world) {
                return Err(
                    FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch {
                        face: folded.face.face_id,
                    },
                );
            }
            polygon.push(flat);
        }
        normalize_convex_ccw(&mut polygon)?;
        result.push(FaceGeometry { polygon });
    }
    Ok(result)
}

fn validate_planar_pose_transform(
    transform: ori_kinematics::RigidTransform,
    orientation: FoldedFaceOrientation,
    face: FaceId,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    let rows = transform.rotation_rows();
    let translation = transform.translation();
    let expected_normal: f64 = match orientation {
        FoldedFaceOrientation::FrontUp => 1.0,
        FoldedFaceOrientation::BackUp => -1.0,
    };
    // Composing multiple bit-exact half turns may retain IEEE-754 signed
    // zero in an otherwise exact planar transform. Normalize zero only at
    // this geometry comparison boundary; every nonzero bit pattern remains
    // subject to the exact folded-face checks below.
    if canonical_zero(rows[0][1]).to_bits() != 0.0_f64.to_bits()
        || canonical_zero(rows[1][0]).to_bits() != 0.0_f64.to_bits()
        || rows[1][1].to_bits() != expected_normal.to_bits()
        || canonical_zero(rows[1][2]).to_bits() != 0.0_f64.to_bits()
        || canonical_zero(rows[2][1]).to_bits() != 0.0_f64.to_bits()
        || canonical_zero(translation.y()).to_bits() != 0.0_f64.to_bits()
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch { face });
    }
    Ok(())
}

fn validate_cells(
    layer_order: &LayerOrderSnapshot,
    registry: &[LayerFace],
    registry_indices: &BTreeMap<[u8; 16], usize>,
    faces: &[FaceGeometry],
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<(Vec<CellGeometry>, Vec<FlatEndpointLayerCellV1>), FlatEndpointLayerOrderAnchorErrorV1>
{
    check_limit(
        FlatEndpointLayerOrderResourceV1::Cells,
        layer_order.overlap_cells.len(),
        limits.max_cells,
    )?;
    tracker.work.cells = layer_order.overlap_cells.len();
    let mut prior_key = None;
    let mut exact_cells = Vec::new();
    let mut output = Vec::new();
    exact_cells
        .try_reserve_exact(layer_order.overlap_cells.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    output
        .try_reserve_exact(layer_order.overlap_cells.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    let mut covered_faces = BTreeSet::new();
    for cell in &layer_order.overlap_cells {
        if prior_key.is_some_and(|prior: [u8; 32]| prior >= cell.cell_key.0) {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
        prior_key = Some(cell.cell_key.0);
        if cell.exact_boundary.len() < 3 {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
        check_limit(
            FlatEndpointLayerOrderResourceV1::BoundaryVerticesPerCell,
            cell.exact_boundary.len(),
            limits.max_boundary_vertices_per_cell,
        )?;
        tracker.work.total_boundary_vertices = tracker
            .work
            .total_boundary_vertices
            .checked_add(cell.exact_boundary.len())
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        check_limit(
            FlatEndpointLayerOrderResourceV1::TotalBoundaryVertices,
            tracker.work.total_boundary_vertices,
            limits.max_total_boundary_vertices,
        )?;
        if cell.covering_faces.is_empty()
            || cell.covering_faces.len() != cell.bottom_to_top_faces.len()
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        }
        tracker.work.total_layer_records = tracker
            .work
            .total_layer_records
            .checked_add(cell.covering_faces.len())
            .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
        check_limit(
            FlatEndpointLayerOrderResourceV1::LayerRecords,
            tracker.work.total_layer_records,
            limits.max_total_layer_records,
        )?;
        let mut polygon = Vec::new();
        let mut world_boundary = Vec::new();
        polygon
            .try_reserve_exact(cell.exact_boundary.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        world_boundary
            .try_reserve_exact(cell.exact_boundary.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        for point in &cell.exact_boundary {
            let exact = parse_point(point, tracker, limits)?;
            world_boundary.push(rational_point_to_world(&exact)?);
            polygon.push(exact);
        }
        if world_boundary
            .iter()
            .map(|point| point3_bits(*point))
            .collect::<BTreeSet<_>>()
            .len()
            != world_boundary.len()
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
        if polygon
            .iter()
            .enumerate()
            .any(|(index, point)| point == &polygon[(index + 1) % polygon.len()])
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
        normalize_convex_ccw(&mut polygon)?;
        let exact_area = polygon_double_area(&polygon).abs() / BigInt::from(2_u8);
        let mut covering_indices = Vec::new();
        covering_indices
            .try_reserve_exact(cell.covering_faces.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        let mut previous = None;
        for face in &cell.covering_faces {
            let Some(&index) = registry_indices.get(&face.face_id.canonical_bytes()) else {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
            };
            if *face != registry[index] || previous.is_some_and(|prior| prior >= index) {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
            }
            previous = Some(index);
            covered_faces.insert(index);
            covering_indices.push(index);
        }
        if recompute_cell_key(&cell.exact_boundary, &cell.covering_faces)? != cell.cell_key {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
        let mut bottom_indices = Vec::new();
        bottom_indices
            .try_reserve_exact(cell.bottom_to_top_faces.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        let mut bottom_set = BTreeSet::new();
        for face in &cell.bottom_to_top_faces {
            let Some(&index) = registry_indices.get(&face.canonical_bytes()) else {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
            };
            if !bottom_set.insert(index) {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
            }
            bottom_indices.push(index);
        }
        if bottom_set != covering_indices.iter().copied().collect() {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        }
        for index in &covering_indices {
            validate_polygon_contained(&polygon, &faces[*index].polygon, tracker, limits)?;
        }
        exact_cells.push(CellGeometry {
            key: cell.cell_key,
            polygon,
            covering_indices,
            bottom_indices,
            exact_area,
        });
        let mut covering_faces = Vec::new();
        covering_faces
            .try_reserve_exact(cell.covering_faces.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        covering_faces.extend(cell.covering_faces.iter().map(|face| face.face_id));
        let mut bottom_to_top_faces = Vec::new();
        bottom_to_top_faces
            .try_reserve_exact(cell.bottom_to_top_faces.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
        bottom_to_top_faces.extend_from_slice(&cell.bottom_to_top_faces);
        output.push(FlatEndpointLayerCellV1 {
            cell_key: FlatEndpointCellKeyV1(cell.cell_key.0),
            world_boundary,
            covering_faces,
            bottom_to_top_faces,
        });
    }
    if covered_faces != (0..registry.len()).collect() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
    }
    Ok((exact_cells, output))
}

fn validate_cell_coverage(
    faces: &[FaceGeometry],
    cells: &[CellGeometry],
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    for first in 0..cells.len() {
        for second in (first + 1)..cells.len() {
            let tests = cells[first]
                .polygon
                .len()
                .checked_mul(cells[second].polygon.len())
                .and_then(|value| value.checked_mul(2))
                .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
            tracker.charge_separation(tests, limits)?;
            if convex_interiors_overlap(&cells[first].polygon, &cells[second].polygon) {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
            }
        }
    }
    for (face_index, face) in faces.iter().enumerate() {
        let expected = polygon_double_area(&face.polygon).abs() / BigInt::from(2_u8);
        let observed = cells
            .iter()
            .filter(|cell| cell.covering_indices.contains(&face_index))
            .fold(BigRational::zero(), |sum, cell| sum + &cell.exact_area);
        if observed != expected {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
    }
    Ok(())
}

fn validate_orders(
    layer_order: &LayerOrderSnapshot,
    registry: &[LayerFace],
    registry_indices: &BTreeMap<[u8; 16], usize>,
    cells: &[CellGeometry],
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    type PairRecord = (usize, usize, Vec<OverlapCellKey>);
    check_limit(
        FlatEndpointLayerOrderResourceV1::FacePairOrders,
        layer_order.face_pair_orders.len(),
        limits.max_face_pair_orders,
    )?;
    tracker.work.face_pair_orders = layer_order.face_pair_orders.len();
    tracker.work.total_supporting_cells = layer_order
        .face_pair_orders
        .iter()
        .try_fold(0_usize, |total, order| {
            total.checked_add(order.supporting_cells.len())
        })
        .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
    check_limit(
        FlatEndpointLayerOrderResourceV1::SupportingCells,
        tracker.work.total_supporting_cells,
        limits.max_total_supporting_cells,
    )?;
    let derived_pair_occurrences = cells.iter().try_fold(0_usize, |total, cell| {
        let pairs = cell
            .bottom_indices
            .len()
            .checked_mul(cell.bottom_indices.len().saturating_sub(1))
            .and_then(|value| value.checked_div(2))?;
        total.checked_add(pairs)
    });
    if derived_pair_occurrences != Some(tracker.work.total_supporting_cells) {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
    }
    let mut derived = BTreeMap::<(usize, usize), PairRecord>::new();
    for cell in cells {
        for lower_position in 0..cell.bottom_indices.len() {
            for upper_position in (lower_position + 1)..cell.bottom_indices.len() {
                let lower = cell.bottom_indices[lower_position];
                let upper = cell.bottom_indices[upper_position];
                let key = if lower < upper {
                    (lower, upper)
                } else {
                    (upper, lower)
                };
                let entry = derived
                    .entry(key)
                    .or_insert_with(|| (lower, upper, Vec::new()));
                if entry.0 != lower || entry.1 != upper {
                    return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
                }
                entry.2.push(cell.key);
            }
        }
    }
    for entry in derived.values_mut() {
        entry.2.sort_unstable_by_key(|key| key.0);
        entry.2.dedup();
    }
    if layer_order.face_pair_orders.len() != derived.len() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
    }
    let mut observed_pairs = BTreeSet::new();
    let mut prior_order_key = None;
    for order in &layer_order.face_pair_orders {
        let Some(&lower) = registry_indices.get(&order.lower_face.face_id.canonical_bytes()) else {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        };
        let Some(&upper) = registry_indices.get(&order.upper_face.face_id.canonical_bytes()) else {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        };
        if order.lower_face != registry[lower]
            || order.upper_face != registry[upper]
            || lower == upper
            || !observed_pairs.insert((lower.min(upper), lower.max(upper)))
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        }
        let order_key = (
            order.lower_face.face_key,
            order.upper_face.face_key,
            order.lower_face.face_id.canonical_bytes(),
            order.upper_face.face_id.canonical_bytes(),
        );
        if prior_order_key.is_some_and(|prior| prior >= order_key) {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        }
        prior_order_key = Some(order_key);
        let Some(expected) = derived.get(&(lower.min(upper), lower.max(upper))) else {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        };
        if expected.0 != lower
            || expected.1 != upper
            || order.supporting_cells != expected.2
            || order
                .supporting_cells
                .windows(2)
                .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch);
        }
    }
    if let Some(global) = &layer_order.global_bottom_to_top {
        if global.len() != registry.len() {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::GlobalOrderMismatch);
        }
        let mut positions = BTreeMap::new();
        for (position, face) in global.iter().enumerate() {
            let Some(&index) = registry_indices.get(&face.face_id.canonical_bytes()) else {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::GlobalOrderMismatch);
            };
            if *face != registry[index] || positions.insert(index, position).is_some() {
                return Err(FlatEndpointLayerOrderAnchorErrorV1::GlobalOrderMismatch);
            }
        }
        if positions.len() != registry.len()
            || derived
                .values()
                .any(|(lower, upper, _)| positions[lower] >= positions[upper])
        {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::GlobalOrderMismatch);
        }
    }
    let Some(summary) = layer_order.proof_summary else {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch);
    };
    let maximum_ply = cells
        .iter()
        .map(|cell| cell.covering_indices.len())
        .max()
        .unwrap_or(0);
    if summary.material_faces != registry.len()
        || summary.overlap_face_pairs != derived.len()
        || summary.overlap_cells != cells.len()
        || summary.maximum_ply != maximum_ply
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch);
    }
    Ok(())
}

fn parse_transform(
    value: &ExactAffineTransform,
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<RationalTransform, FlatEndpointLayerOrderAnchorErrorV1> {
    Ok(RationalTransform {
        m00: parse_rational(&value.m00, tracker, limits)?,
        m01: parse_rational(&value.m01, tracker, limits)?,
        m10: parse_rational(&value.m10, tracker, limits)?,
        m11: parse_rational(&value.m11, tracker, limits)?,
        tx: parse_rational(&value.tx, tracker, limits)?,
        ty: parse_rational(&value.ty, tracker, limits)?,
    })
}

fn parse_point(
    value: &ExactPointValue,
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<RationalPoint, FlatEndpointLayerOrderAnchorErrorV1> {
    Ok(RationalPoint {
        x: parse_rational(&value.x, tracker, limits)?,
        y: parse_rational(&value.y, tracker, limits)?,
    })
}

fn parse_rational(
    value: &ExactRationalValue,
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<BigRational, FlatEndpointLayerOrderAnchorErrorV1> {
    tracker.charge_exact(value, limits)?;
    if value.numerator_magnitude_be.is_empty()
        || value.denominator_be.is_empty()
        || (value.numerator_magnitude_be.len() > 1 && value.numerator_magnitude_be[0] == 0)
        || (value.denominator_be.len() > 1 && value.denominator_be[0] == 0)
    {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload);
    }
    let magnitude = BigInt::from_bytes_be(Sign::Plus, &value.numerator_magnitude_be);
    let denominator = BigInt::from_bytes_be(Sign::Plus, &value.denominator_be);
    if denominator <= BigInt::zero() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload);
    }
    let numerator = match value.sign {
        ExactSign::Negative if !magnitude.is_zero() => -magnitude,
        ExactSign::Zero if magnitude.is_zero() => magnitude,
        ExactSign::Positive if !magnitude.is_zero() => magnitude,
        _ => return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload),
    };
    if !numerator.abs().gcd(&denominator).is_one() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload);
    }
    let rational = BigRational::new(numerator, denominator);
    if encode_rational(&rational) != *value {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload);
    }
    Ok(rational)
}

fn encode_rational(value: &BigRational) -> ExactRationalValue {
    let (sign, numerator_magnitude_be) = value.numer().to_bytes_be();
    let (_, denominator_be) = value.denom().to_bytes_be();
    ExactRationalValue {
        sign: if value.is_zero() {
            ExactSign::Zero
        } else if sign == Sign::Minus {
            ExactSign::Negative
        } else {
            ExactSign::Positive
        },
        numerator_magnitude_be,
        denominator_be,
    }
}

fn rational_point_from_binary64(
    point: Point2,
) -> Result<RationalPoint, FlatEndpointLayerOrderAnchorErrorV1> {
    Ok(RationalPoint {
        x: rational_from_binary64(point.x)?,
        y: rational_from_binary64(point.y)?,
    })
}

fn rational_from_binary64(value: f64) -> Result<BigRational, FlatEndpointLayerOrderAnchorErrorV1> {
    if !value.is_finite() || value.to_bits() == (-0.0_f64).to_bits() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload);
    }
    BigRational::from_float(value).ok_or(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload)
}

pub(crate) fn rational_point_to_world(
    point: &RationalPoint,
) -> Result<Point3, FlatEndpointLayerOrderAnchorErrorV1> {
    let x = point
        .x
        .to_f64()
        .filter(|value| value.is_finite())
        .ok_or(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload)?;
    let y = point
        .y
        .to_f64()
        .filter(|value| value.is_finite())
        .ok_or(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload)?;
    Point3::new(canonical_zero(x), 0.0, canonical_zero(-y))
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload)
}

fn normalize_convex_ccw(
    polygon: &mut [RationalPoint],
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    if polygon.len() < 3 {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
    }
    let area = polygon_double_area(polygon);
    if area.is_zero() {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
    }
    if area.is_negative() {
        polygon.reverse();
    }
    let mut observed_positive = false;
    for index in 0..polygon.len() {
        let cross = orientation(
            &polygon[index],
            &polygon[(index + 1) % polygon.len()],
            &polygon[(index + 2) % polygon.len()],
        );
        if cross.is_negative() {
            return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
        }
        observed_positive |= cross.is_positive();
    }
    if !observed_positive {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
    }
    Ok(())
}

fn polygon_double_area(polygon: &[RationalPoint]) -> BigRational {
    (0..polygon.len()).fold(BigRational::zero(), |sum, index| {
        let next = (index + 1) % polygon.len();
        sum + &polygon[index].x * &polygon[next].y - &polygon[index].y * &polygon[next].x
    })
}

fn orientation(
    first: &RationalPoint,
    second: &RationalPoint,
    third: &RationalPoint,
) -> BigRational {
    (&second.x - &first.x) * (&third.y - &first.y) - (&second.y - &first.y) * (&third.x - &first.x)
}

fn validate_polygon_contained(
    inner: &[RationalPoint],
    outer: &[RationalPoint],
    tracker: &mut WorkTracker,
    limits: FlatEndpointLayerOrderLimitsV1,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    let tests = inner
        .len()
        .checked_mul(outer.len())
        .ok_or(FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?;
    tracker.charge_containment(tests, limits)?;
    if outer.iter().enumerate().any(|(edge, first)| {
        let second = &outer[(edge + 1) % outer.len()];
        inner
            .iter()
            .any(|point| orientation(first, second, point).is_negative())
    }) {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
    }
    Ok(())
}

fn convex_interiors_overlap(first: &[RationalPoint], second: &[RationalPoint]) -> bool {
    !has_nonpositive_separating_axis(first, second)
        && !has_nonpositive_separating_axis(second, first)
}

fn has_nonpositive_separating_axis(
    axis_polygon: &[RationalPoint],
    other: &[RationalPoint],
) -> bool {
    axis_polygon.iter().enumerate().any(|(edge, first)| {
        let second = &axis_polygon[(edge + 1) % axis_polygon.len()];
        other
            .iter()
            .map(|point| orientation(first, second, point))
            .max()
            .is_none_or(|maximum| !maximum.is_positive())
    })
}

fn recompute_cell_key(
    boundary: &[ExactPointValue],
    covering_faces: &[LayerFace],
) -> Result<OverlapCellKey, FlatEndpointLayerOrderAnchorErrorV1> {
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(boundary.len())
        .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::AllocationFailed)?;
    for point in boundary {
        let mut bytes = encode_exact_value(&point.x)?;
        bytes.extend_from_slice(&encode_exact_value(&point.y)?);
        encoded.push(bytes);
    }
    let Some((mut start, first)) = encoded.iter().enumerate().next() else {
        return Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch);
    };
    let mut minimum = first;
    for (index, candidate) in encoded.iter().enumerate().skip(1) {
        if candidate < minimum {
            start = index;
            minimum = candidate;
        }
    }
    let len = encoded.len();
    let mut direction = Ordering::Equal;
    for offset in 0..len {
        direction = encoded[(start + offset) % len].cmp(&encoded[(start + len - offset) % len]);
        if direction != Ordering::Equal {
            break;
        }
    }
    let reverse = direction == Ordering::Greater;
    let mut hasher = Sha256::new();
    hasher.update(CELL_KEY_DOMAIN);
    hasher.update(
        u64::try_from(boundary.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?
            .to_be_bytes(),
    );
    for offset in 0..len {
        let index = if reverse {
            (start + len - offset) % len
        } else {
            (start + offset) % len
        };
        hasher.update(
            u64::try_from(encoded[index].len())
                .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?
                .to_be_bytes(),
        );
        hasher.update(&encoded[index]);
    }
    for face in covering_faces {
        hasher.update(face.face_key.0);
    }
    Ok(OverlapCellKey(hasher.finalize().into()))
}

fn encode_exact_value(
    value: &ExactRationalValue,
) -> Result<Vec<u8>, FlatEndpointLayerOrderAnchorErrorV1> {
    let mut bytes = Vec::new();
    bytes.push(match value.sign {
        ExactSign::Negative => 0,
        ExactSign::Zero => 1,
        ExactSign::Positive => 2,
    });
    append_len_prefixed(&mut bytes, &value.numerator_magnitude_be)?;
    append_len_prefixed(&mut bytes, &value.denominator_be)?;
    Ok(bytes)
}

fn append_len_prefixed(
    target: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    target.extend_from_slice(
        &u64::try_from(value.len())
            .map_err(|_| FlatEndpointLayerOrderAnchorErrorV1::ResourceCountOverflow)?
            .to_be_bytes(),
    );
    target.extend_from_slice(value);
    Ok(())
}

fn check_limit(
    resource: FlatEndpointLayerOrderResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), FlatEndpointLayerOrderAnchorErrorV1> {
    if actual > maximum {
        Err(FlatEndpointLayerOrderAnchorErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn unordered_face_pair(first: FaceId, second: FaceId) -> ([u8; 16], [u8; 16]) {
    let first = first.canonical_bytes();
    let second = second.canonical_bytes();
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn point3_bits(point: Point3) -> [u64; 3] {
    [
        canonical_zero(point.x()).to_bits(),
        canonical_zero(point.y()).to_bits(),
        canonical_zero(point.z()).to_bits(),
    ]
}

fn cells_bit_exact_equal(
    first: &[FlatEndpointLayerCellV1],
    second: &[FlatEndpointLayerCellV1],
) -> bool {
    first.len() == second.len()
        && first.iter().zip(second).all(|(first, second)| {
            first.cell_key == second.cell_key
                && first.covering_faces == second.covering_faces
                && first.bottom_to_top_faces == second.bottom_to_top_faces
                && first.world_boundary.len() == second.world_boundary.len()
                && first
                    .world_boundary
                    .iter()
                    .zip(&second.world_boundary)
                    .all(|(first, second)| point3_bits(*first) == point3_bits(*second))
        })
}
