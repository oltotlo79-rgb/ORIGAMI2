use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use num_bigint::BigInt;
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_foldability::{
    FoldModelFingerprintV1, GlobalFlatFoldabilityProvenance, LAYER_ORDER_MODEL_ID, LayerFace,
    LayerOrderSnapshot, fold_model_fingerprint_v1,
};
use ori_geometry::{
    GeometryError, Orientation, PointPolygonRelation, PointSegmentRelation, SegmentIntersection,
    exact_orientation, point_polygon_relation, point_segment_relation, segment_intersection,
};
use ori_kinematics::{
    CandidateFaceTransform, CanonicalHingeAngles, ClosedMaterialHingeGraphPose, HingeAngle,
    KinematicsError, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    MaterialTreeKinematicsModel, MaterialTreePose, Point3, TreeKinematicsLimits,
};
use ori_topology::{
    Face, FaceExtractionInput, TopologyIssueSeverity, TopologySnapshot, analyze_faces,
};
use thiserror::Error;

use crate::{MAX_REVISION, Revision};

pub const DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES: usize = 2_048;
pub const DEFAULT_MAX_FACE_LINEAGE_TARGET_FACES: usize = 2_048;
pub const DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES: usize = 100_000;
pub const DEFAULT_MAX_FACE_LINEAGE_FACE_PAIRS: usize = 500_000;
pub const DEFAULT_MAX_FACE_LINEAGE_EXACT_CONTAINMENT_TESTS: usize = 100_000_000;

/// Deterministic limits for proving one crease-addition face lineage.
///
/// Equality is admitted. A caller must use the same limits when retrying the
/// same immutable input if it needs bit-for-bit repeatability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceLineageLimits {
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_source_paper_boundary_vertices: usize,
    pub max_target_vertices: usize,
    pub max_target_edges: usize,
    pub max_target_paper_boundary_vertices: usize,
    pub max_source_faces: usize,
    pub max_target_faces: usize,
    pub max_source_boundary_half_edges: usize,
    pub max_target_boundary_half_edges: usize,
    pub max_face_pairs: usize,
    pub max_exact_containment_tests: usize,
}

impl Default for FaceLineageLimits {
    fn default() -> Self {
        Self {
            max_source_vertices: crate::DEFAULT_MAX_SOURCE_VERTICES,
            max_source_edges: crate::DEFAULT_MAX_SOURCE_EDGES,
            max_source_paper_boundary_vertices: crate::DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_target_vertices: crate::DEFAULT_MAX_SOURCE_VERTICES,
            max_target_edges: crate::DEFAULT_MAX_SOURCE_EDGES,
            max_target_paper_boundary_vertices: crate::DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_source_faces: DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES,
            max_target_faces: DEFAULT_MAX_FACE_LINEAGE_TARGET_FACES,
            max_source_boundary_half_edges: DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES,
            max_target_boundary_half_edges: DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES,
            max_face_pairs: DEFAULT_MAX_FACE_LINEAGE_FACE_PAIRS,
            max_exact_containment_tests: DEFAULT_MAX_FACE_LINEAGE_EXACT_CONTAINMENT_TESTS,
        }
    }
}

/// Immutable source and candidate geometry for one future stacked-fold
/// transaction.
///
/// This input only prepares face lineage. It does not authorize a project
/// mutation: reverse mapping, per-layer assignments, collision-stop evidence,
/// the updated layer order, and timeline migration still have to succeed in
/// the eventual atomic `ApplyStackedFold` command. In particular, this module
/// neither proves that the target delta is one straight crease nor re-proves
/// overlap-cell stack ordering. `LayerOrderSnapshot` is public transport data,
/// so matching its provenance and material registry here is not authentication
/// that the solver minted it. That command must separately authenticate the
/// native current-layer-order slot, its immutable binding, and its complete
/// certificate immediately before commit.
#[derive(Debug, Clone, Copy)]
pub struct FaceLineageInput<'a> {
    pub identity_namespace: ProjectId,
    pub source_revision: Revision,
    pub source_paper: &'a Paper,
    pub source_pattern: &'a CreasePattern,
    pub source_layer_order: &'a LayerOrderSnapshot,
    pub target_revision: Revision,
    pub target_paper: &'a Paper,
    pub target_pattern: &'a CreasePattern,
}

/// One complete source-face to descendant-faces relation.
///
/// Descendants are ordered by canonical `FaceKey`, then by the face ID's RFC
/// bytes. At least one record in a valid lineage has two or more descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceLineageRecord {
    source: LayerFace,
    descendants: Vec<LayerFace>,
}

impl FaceLineageRecord {
    #[must_use]
    pub const fn source(&self) -> LayerFace {
        self.source
    }

    #[must_use]
    pub fn descendants(&self) -> &[LayerFace] {
        &self.descendants
    }
}

/// Canonical, revision-bound proof that candidate faces refine source faces.
///
/// Fields remain private so callers cannot forge an accepted mapping by
/// constructing this type directly. The proof is deliberately not a project
/// command, does not confer authority for any layer stack, and carries no
/// authority to mutate an [`crate::EditorState`].
///
/// ```compile_fail
/// use ori_core::FaceLineageV1;
///
/// fn discard_records(proof: FaceLineageV1) -> FaceLineageV1 {
///     FaceLineageV1 {
///         records: Vec::new(),
///         ..proof
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceLineageV1 {
    identity_namespace: ProjectId,
    source_revision: Revision,
    target_revision: Revision,
    source_fingerprint: FoldModelFingerprintV1,
    target_fingerprint: FoldModelFingerprintV1,
    records: Vec<FaceLineageRecord>,
}

impl FaceLineageV1 {
    #[must_use]
    pub const fn identity_namespace(&self) -> ProjectId {
        self.identity_namespace
    }

    #[must_use]
    pub const fn source_revision(&self) -> Revision {
        self.source_revision
    }

    #[must_use]
    pub const fn target_revision(&self) -> Revision {
        self.target_revision
    }

    #[must_use]
    pub const fn source_fingerprint(&self) -> FoldModelFingerprintV1 {
        self.source_fingerprint
    }

    #[must_use]
    pub const fn target_fingerprint(&self) -> FoldModelFingerprintV1 {
        self.target_fingerprint
    }

    #[must_use]
    pub fn records(&self) -> &[FaceLineageRecord] {
        &self.records
    }
}

pub const DEFAULT_MAX_STACKED_FOLD_EXPECTED_CREASES: usize = 10_000;
pub const DEFAULT_MAX_STACKED_FOLD_EDGE_CARRIER_TESTS: usize = 10_000_000;
pub const DEFAULT_MAX_STACKED_FOLD_CARRIER_OVERLAP_TESTS: usize = 10_000_000;
pub const DEFAULT_MAX_STACKED_FOLD_LINEAGE_RECORDS: usize = DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES;
pub const DEFAULT_MAX_STACKED_FOLD_LINEAGE_DESCENDANTS: usize =
    DEFAULT_MAX_FACE_LINEAGE_TARGET_FACES;
pub const DEFAULT_MAX_STACKED_FOLD_BUILD_CARRIERS: usize = 12_048;
pub const DEFAULT_MAX_STACKED_FOLD_BUILD_INTERSECTIONS: usize = 2_000_000;
pub const DEFAULT_MAX_STACKED_FOLD_BUILD_VERTICES: usize = 100_000;
pub const DEFAULT_MAX_STACKED_FOLD_BUILD_EDGES: usize = 200_000;
pub const STACKED_FOLD_TARGET_GRAPH_AUDIT_MODEL_ID_V1: &str =
    "native_stacked_fold_target_graph_audit_v1";
pub const STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1: f64 = 1.0e-9;
pub const DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS: usize = 2_000_000;
pub const STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1: &str =
    "native_stacked_fold_non_flat_planar_order_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackedFoldTopologyBuildLimitsV1 {
    pub max_carriers: usize,
    pub max_pair_tests: usize,
    pub max_vertices: usize,
    pub max_edges: usize,
}

impl Default for StackedFoldTopologyBuildLimitsV1 {
    fn default() -> Self {
        Self {
            max_carriers: DEFAULT_MAX_STACKED_FOLD_BUILD_CARRIERS,
            max_pair_tests: DEFAULT_MAX_STACKED_FOLD_BUILD_INTERSECTIONS,
            max_vertices: DEFAULT_MAX_STACKED_FOLD_BUILD_VERTICES,
            max_edges: DEFAULT_MAX_STACKED_FOLD_BUILD_EDGES,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackedFoldTopologyCandidateV1 {
    pub pattern: CreasePattern,
    pub paper: Paper,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedStackedFoldGeometryV1 {
    candidate: StackedFoldTopologyCandidateV1,
    proof: StackedFoldGeometryProofV1,
}

pub struct PreparedStackedFoldTargetModelV1 {
    geometry: PreparedStackedFoldGeometryV1,
    model: MaterialTreeKinematicsModel,
}

/// Read-only, topology-bound transport for a proved target hinge graph.
///
/// Unlike [`PreparedStackedFoldTargetModelV1`], this package also admits
/// cyclic graphs.  Its audit can subsequently be supplied to
/// `MaterialHingeClosureCertificate::observe` once a caller has candidate
/// transforms for every target face.  Merely preparing or transporting this
/// package grants no pose, closure, or mutation authority.
pub struct PreparedStackedFoldTargetGraphAuditV1 {
    geometry: PreparedStackedFoldGeometryV1,
    audit: MaterialHingeGraphAudit,
    hinge_geometry: MaterialHingeGraphGeometry,
}

pub struct PreparedStackedFoldInitialPoseV1 {
    target: PreparedStackedFoldTargetModelV1,
    pose: MaterialTreePose,
}

pub struct PreparedStackedFoldRequestedPoseV1 {
    initial: PreparedStackedFoldInitialPoseV1,
    pose: MaterialTreePose,
    requested_angle_degrees: f64,
}

pub struct PreparedStackedFoldInitialGraphPoseV1 {
    target: PreparedStackedFoldTargetGraphAuditV1,
    pose: ClosedMaterialHingeGraphPose,
}

pub struct PreparedStackedFoldRequestedGraphPoseV1 {
    initial: PreparedStackedFoldInitialGraphPoseV1,
    pose: ClosedMaterialHingeGraphPose,
    requested_angle_degrees: f64,
}

/// Bounded proof that a non-flat endpoint has no pair of faces sharing one
/// planar support, hence has no planar overlap cell requiring a ply order.
///
/// This is read-only transport evidence. It grants no collision, timeline, or
/// mutation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackedFoldNonFlatLayerOrderV1 {
    target_revision: Revision,
    material_faces: Vec<LayerFace>,
    tested_face_pairs: usize,
    source_overlap_cells_authenticated: usize,
}

impl StackedFoldNonFlatLayerOrderV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1
    }

    #[must_use]
    pub const fn target_revision(&self) -> Revision {
        self.target_revision
    }

    #[must_use]
    pub fn material_faces(&self) -> &[LayerFace] {
        &self.material_faces
    }

    #[must_use]
    pub const fn tested_face_pairs(&self) -> usize {
        self.tested_face_pairs
    }

    #[must_use]
    pub const fn overlap_cell_count(&self) -> usize {
        0
    }

    #[must_use]
    pub const fn face_pair_order_count(&self) -> usize {
        0
    }

    #[must_use]
    pub const fn source_overlap_cells_authenticated(&self) -> usize {
        self.source_overlap_cells_authenticated
    }

    #[must_use]
    pub const fn authorizes_apply_stacked_fold(&self) -> bool {
        false
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum PrepareStackedFoldNonFlatLayerOrderErrorV1 {
    #[error("the requested endpoint is flat or invalid")]
    NotNonFlatEndpoint,
    #[error("the source layer snapshot is stale or belongs to another model")]
    SourceLayerOrderMismatch,
    #[error("target face-pair work exceeds its configured limit")]
    ResourceLimit,
    #[error("target pose issuer or material registry is inconsistent")]
    TargetPoseMismatch,
    #[error("a target face has no finitely representable planar support")]
    UnrepresentableFacePlane,
    #[error("target faces {first:?} and {second:?} may share one planar support")]
    CoincidentPlanarSupports { first: FaceId, second: FaceId },
    #[error("target pose geometry failed: {0}")]
    Kinematics(#[from] KinematicsError),
}

impl PreparedStackedFoldInitialGraphPoseV1 {
    #[must_use]
    pub const fn target(&self) -> &PreparedStackedFoldTargetGraphAuditV1 {
        &self.target
    }

    #[must_use]
    pub const fn pose(&self) -> &ClosedMaterialHingeGraphPose {
        &self.pose
    }
}

impl PreparedStackedFoldRequestedGraphPoseV1 {
    #[must_use]
    pub const fn initial(&self) -> &PreparedStackedFoldInitialGraphPoseV1 {
        &self.initial
    }

    #[must_use]
    pub const fn pose(&self) -> &ClosedMaterialHingeGraphPose {
        &self.pose
    }

    #[must_use]
    pub const fn requested_angle_degrees(&self) -> f64 {
        self.requested_angle_degrees
    }
}

impl PreparedStackedFoldRequestedPoseV1 {
    #[must_use]
    pub const fn initial(&self) -> &PreparedStackedFoldInitialPoseV1 {
        &self.initial
    }

    #[must_use]
    pub const fn pose(&self) -> &MaterialTreePose {
        &self.pose
    }

    #[must_use]
    pub const fn requested_angle_degrees(&self) -> f64 {
        self.requested_angle_degrees
    }
}

impl PreparedStackedFoldInitialPoseV1 {
    #[must_use]
    pub const fn target(&self) -> &PreparedStackedFoldTargetModelV1 {
        &self.target
    }

    #[must_use]
    pub const fn pose(&self) -> &MaterialTreePose {
        &self.pose
    }
}

impl PreparedStackedFoldTargetModelV1 {
    #[must_use]
    pub const fn geometry(&self) -> &PreparedStackedFoldGeometryV1 {
        &self.geometry
    }

    #[must_use]
    pub const fn model(&self) -> &MaterialTreeKinematicsModel {
        &self.model
    }
}

impl PreparedStackedFoldTargetGraphAuditV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        STACKED_FOLD_TARGET_GRAPH_AUDIT_MODEL_ID_V1
    }

    #[must_use]
    pub const fn geometry(&self) -> &PreparedStackedFoldGeometryV1 {
        &self.geometry
    }

    /// Canonical face, spanning-hinge, and closure-hinge identities needed by
    /// a later observation-only closure certificate.
    #[must_use]
    pub const fn audit(&self) -> &MaterialHingeGraphAudit {
        &self.audit
    }

    #[must_use]
    pub const fn hinge_geometry(&self) -> &MaterialHingeGraphGeometry {
        &self.hinge_geometry
    }

    #[must_use]
    pub const fn requires_closure_certificate(&self) -> bool {
        !self.audit.is_tree()
    }

    #[must_use]
    pub const fn authorizes_pose(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply_stacked_fold(&self) -> bool {
        false
    }

    /// Returns the proved geometry for the existing tree-only pipeline after
    /// the caller has inspected the graph audit.
    #[must_use]
    pub fn into_geometry(self) -> PreparedStackedFoldGeometryV1 {
        self.geometry
    }
}

impl PreparedStackedFoldGeometryV1 {
    #[must_use]
    pub const fn candidate(&self) -> &StackedFoldTopologyCandidateV1 {
        &self.candidate
    }

    #[must_use]
    pub const fn proof(&self) -> &StackedFoldGeometryProofV1 {
        &self.proof
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldTopologyBuildResourceV1 {
    Carriers,
    PairTests,
    Vertices,
    Edges,
}

#[derive(Debug, Error, PartialEq)]
pub enum StackedFoldTopologyBuildErrorV1 {
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimit {
        resource: StackedFoldTopologyBuildResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("source edge {edge:?} has a missing endpoint")]
    SourceEdgeEndpointMissing { edge: EdgeId },
    #[error("source geometry contains duplicate vertex coordinates")]
    DuplicateSourceVertexPosition,
    #[error("source geometry contains duplicate vertex ID {vertex:?}")]
    DuplicateSourceVertexId { vertex: VertexId },
    #[error("a derived target identity collides with an existing identity")]
    DerivedIdentityCollision,
    #[error("an expected crease is non-finite, degenerate, or not mountain/valley")]
    InvalidExpectedCrease,
    #[error("carriers {first} and {second} overlap collinearly")]
    CarrierOverlap { first: usize, second: usize },
    #[error("the paper boundary references a missing boundary carrier")]
    PaperBoundaryCarrierMissing,
    #[error("the paper boundary references missing vertex {vertex:?}")]
    PaperBoundaryVertexMissing { vertex: VertexId },
    #[error("exact geometry predicate failed: {0}")]
    Geometry(#[from] GeometryError),
}

#[derive(Debug, Error, PartialEq)]
pub enum PrepareStackedFoldGeometryErrorV1 {
    #[error("source revision cannot advance")]
    SourceRevisionCannotAdvance,
    #[error("target topology construction failed: {0}")]
    Topology(#[from] StackedFoldTopologyBuildErrorV1),
    #[error("target face lineage failed: {0}")]
    Lineage(#[from] FaceLineageError),
    #[error("target geometry proof failed: {0}")]
    Geometry(#[from] StackedFoldGeometryErrorV1),
}

#[derive(Debug, Error, PartialEq)]
pub enum PrepareStackedFoldTargetModelErrorV1 {
    #[error("proved target topology could not be reconstructed: {0}")]
    Topology(#[from] FaceLineageError),
    #[error("target hinge graph has {closure_hinge_count} loop-closure constraint(s)")]
    CyclicTargetUnsupported { closure_hinge_count: usize },
    #[error("proved target topology is not supported by material tree kinematics: {0}")]
    Kinematics(#[from] KinematicsError),
}

/// Strict failure classes for the read-only target graph audit stage.
#[derive(Debug, Error, PartialEq)]
pub enum PrepareStackedFoldTargetGraphAuditErrorV1 {
    #[error("proved target topology could not be reconstructed: {0}")]
    Topology(#[from] FaceLineageError),
    #[error("target hinge graph exceeds the configured resource limit")]
    ResourceLimit,
    #[error("target hinge graph is disconnected, duplicate, or otherwise unsupported")]
    UnsupportedTopology,
    #[error("target hinge graph geometry is not finitely representable")]
    UnrepresentableGeometry,
}
#[derive(Debug, Error, PartialEq)]
pub enum PrepareStackedFoldInitialPoseErrorV1 {
    #[error("source pose was not issued by the supplied source model")]
    SourcePoseIssuerMismatch,
    #[error("source pose is missing lineage face {face:?}")]
    SourcePoseFaceMissing { face: FaceId },
    #[error("source pose is missing hinge angle for source edge {edge:?}")]
    SourceHingeAngleMissing { edge: EdgeId },
    #[error("target hinge {edge:?} is not covered by the proved geometry delta")]
    TargetHingeWithoutCarrier { edge: EdgeId },
    #[error("source fixed face is absent from proved lineage")]
    SourceFixedFaceMissing,
    #[error("target descendant transform does not preserve source face {face:?}")]
    DescendantTransformMismatch { face: FaceId },
    #[error("target pose preparation failed: {0}")]
    Kinematics(#[from] KinematicsError),
}

#[derive(Debug, Error, PartialEq)]
pub enum PrepareStackedFoldRequestedPoseErrorV1 {
    #[error("requested angle must be finite and in (0, 180]")]
    InvalidRequestedAngle,
    #[error("an expected crease subdivision is absent from target hinges")]
    ExpectedCreaseHingeMissing,
    #[error("requested target pose preparation failed: {0}")]
    Kinematics(#[from] KinematicsError),
}

/// Deterministic count limits for one stacked-fold geometry-delta proof.
///
/// The edge-carrier limit counts the complete Cartesian product of target
/// edges and source/expected carriers. The overlap limit counts every
/// source/expected and expected/expected carrier pair. Equality is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackedFoldGeometryLimitsV1 {
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_source_paper_boundary_vertices: usize,
    pub max_target_vertices: usize,
    pub max_target_edges: usize,
    pub max_target_paper_boundary_vertices: usize,
    pub max_expected_creases: usize,
    pub max_lineage_records: usize,
    pub max_lineage_descendants: usize,
    pub max_edge_carrier_tests: usize,
    pub max_carrier_overlap_tests: usize,
}

impl Default for StackedFoldGeometryLimitsV1 {
    fn default() -> Self {
        Self {
            max_source_vertices: crate::DEFAULT_MAX_SOURCE_VERTICES,
            max_source_edges: crate::DEFAULT_MAX_SOURCE_EDGES,
            max_source_paper_boundary_vertices: crate::DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_target_vertices: crate::DEFAULT_MAX_SOURCE_VERTICES,
            max_target_edges: crate::DEFAULT_MAX_SOURCE_EDGES,
            max_target_paper_boundary_vertices: crate::DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_expected_creases: DEFAULT_MAX_STACKED_FOLD_EXPECTED_CREASES,
            max_lineage_records: DEFAULT_MAX_STACKED_FOLD_LINEAGE_RECORDS,
            max_lineage_descendants: DEFAULT_MAX_STACKED_FOLD_LINEAGE_DESCENDANTS,
            max_edge_carrier_tests: DEFAULT_MAX_STACKED_FOLD_EDGE_CARRIER_TESTS,
            max_carrier_overlap_tests: DEFAULT_MAX_STACKED_FOLD_CARRIER_OVERLAP_TESTS,
        }
    }
}

/// One untrusted, reverse-mapped crease that the target is expected to add.
///
/// This type deliberately does not certify where the segment came from.
/// Version 1 only admits mountain and valley assignments. A future native
/// `ApplyStackedFold` command must authenticate this set against its selected
/// world-space fold operation and current layer authority before committing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpectedStackedFoldCreaseV1 {
    pub start: Point2,
    pub end: Point2,
    pub kind: EdgeKind,
}

/// Immutable inputs for proving one narrow source-to-target geometry delta.
///
/// [`FaceLineageV1`] is a required premise and is rebound to all identity,
/// revision, and fold-model fingerprints. This input and its result make no
/// claim that the expected segments came from one world-space straight line,
/// that every required layer was selected, or that any layer-order snapshot
/// remains authoritative. Those checks belong to a future native command.
#[derive(Debug, Clone, Copy)]
pub struct StackedFoldGeometryInputV1<'a> {
    pub identity_namespace: ProjectId,
    pub source_revision: Revision,
    pub source_paper: &'a Paper,
    pub source_pattern: &'a CreasePattern,
    pub target_revision: Revision,
    pub target_paper: &'a Paper,
    pub target_pattern: &'a CreasePattern,
    pub face_lineage: &'a FaceLineageV1,
    pub expected_creases: &'a [ExpectedStackedFoldCreaseV1],
}

/// Canonical target subdivision of one source edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEdgeSubdivisionV1 {
    source_edge: EdgeId,
    target_edges: Vec<EdgeId>,
}

impl SourceEdgeSubdivisionV1 {
    #[must_use]
    pub const fn source_edge(&self) -> EdgeId {
        self.source_edge
    }

    /// Target edge IDs are ordered by canonical RFC bytes.
    #[must_use]
    pub fn target_edges(&self) -> &[EdgeId] {
        &self.target_edges
    }
}

/// Canonical target subdivision of one explicitly expected new crease.
///
/// Records are published in canonical `(start, end, kind)` order, independent
/// of caller slice order and segment direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedCreaseSubdivisionV1 {
    start_x_bits: u64,
    start_y_bits: u64,
    end_x_bits: u64,
    end_y_bits: u64,
    kind: EdgeKind,
    target_edges: Vec<EdgeId>,
}

impl ExpectedCreaseSubdivisionV1 {
    #[must_use]
    pub fn start(&self) -> Point2 {
        Point2::new(
            f64::from_bits(self.start_x_bits),
            f64::from_bits(self.start_y_bits),
        )
    }

    #[must_use]
    pub fn end(&self) -> Point2 {
        Point2::new(
            f64::from_bits(self.end_x_bits),
            f64::from_bits(self.end_y_bits),
        )
    }

    #[must_use]
    pub const fn kind(&self) -> EdgeKind {
        self.kind
    }

    /// Target edge IDs are ordered by canonical RFC bytes.
    #[must_use]
    pub fn target_edges(&self) -> &[EdgeId] {
        &self.target_edges
    }
}

/// Unforgeable proof that the target changes only admissible edge
/// subdivisions and the explicitly expected M/V crease set.
///
/// This proof owns its [`FaceLineageV1`] premise, so identity, revisions,
/// source/target fingerprints, face refinement, and this geometry delta travel
/// together. It is still read-only evidence, not layer authority and not a
/// project command.
///
/// ```compile_fail
/// use ori_core::StackedFoldGeometryProofV1;
///
/// fn discard_delta(proof: StackedFoldGeometryProofV1) -> StackedFoldGeometryProofV1 {
///     StackedFoldGeometryProofV1 {
///         source_edges: Vec::new(),
///         ..proof
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackedFoldGeometryProofV1 {
    lineage: FaceLineageV1,
    source_edges: Vec<SourceEdgeSubdivisionV1>,
    expected_creases: Vec<ExpectedCreaseSubdivisionV1>,
}

impl StackedFoldGeometryProofV1 {
    #[must_use]
    pub const fn lineage(&self) -> &FaceLineageV1 {
        &self.lineage
    }

    #[must_use]
    pub fn source_edges(&self) -> &[SourceEdgeSubdivisionV1] {
        &self.source_edges
    }

    #[must_use]
    pub fn expected_creases(&self) -> &[ExpectedCreaseSubdivisionV1] {
        &self.expected_creases
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldGeometryResourceV1 {
    SourceVertices,
    SourceEdges,
    SourcePaperBoundaryVertices,
    TargetVertices,
    TargetEdges,
    TargetPaperBoundaryVertices,
    ExpectedCreases,
    LineageRecords,
    LineageDescendants,
    EdgeCarrierTests,
    CarrierOverlapTests,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldGeometryCarrierV1 {
    SourceEdge(EdgeId),
    /// Index in the proof's canonical expected-crease order, not caller order.
    ExpectedCrease(usize),
}

/// Deterministic failure of the narrow geometry-delta proof.
///
/// Every `expected_index`, `first`, and `second` value addresses the canonical
/// expected-crease order described by [`ExpectedCreaseSubdivisionV1`].
#[derive(Debug, Error, PartialEq)]
pub enum StackedFoldGeometryErrorV1 {
    #[error("face lineage belongs to another project identity")]
    LineageIdentityMismatch,
    #[error("face lineage revisions do not match the immutable geometry input")]
    LineageRevisionMismatch,
    #[error("face lineage source fingerprint does not match the immutable source geometry")]
    LineageSourceFingerprintMismatch,
    #[error("face lineage target fingerprint does not match the immutable target geometry")]
    LineageTargetFingerprintMismatch,
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimit {
        resource: StackedFoldGeometryResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("at least one explicit expected crease is required")]
    ExpectedCreaseSetEmpty,
    #[error("expected creases must contain finite coordinates")]
    ExpectedCreaseNonFinite,
    #[error("expected creases must have positive geometric length")]
    ExpectedCreaseDegenerate,
    #[error("expected creases must use only mountain or valley assignments")]
    ExpectedCreaseKindUnsupported,
    #[error("expected crease {expected_index} overlaps source edge {source_edge:?}")]
    ExpectedCreaseOverlapsSourceEdge {
        expected_index: usize,
        source_edge: EdgeId,
    },
    #[error("expected creases {first} and {second} overlap")]
    ExpectedCreasesOverlap { first: usize, second: usize },
    #[error("{topology:?} contains duplicate vertex ID {vertex:?}")]
    DuplicateVertex {
        topology: FaceLineageTopology,
        vertex: VertexId,
    },
    #[error("{topology:?} vertex {vertex:?} is non-finite")]
    NonFiniteVertex {
        topology: FaceLineageTopology,
        vertex: VertexId,
    },
    #[error("target geometry removed source vertex {vertex:?}")]
    SourceVertexMissing { vertex: VertexId },
    #[error("target geometry moved source vertex {vertex:?}")]
    SourceVertexMoved { vertex: VertexId },
    #[error("new target vertex {vertex:?} is unrelated to every target edge")]
    NewTargetVertexIsolated { vertex: VertexId },
    #[error("{topology:?} contains duplicate edge ID {edge:?}")]
    DuplicateEdge {
        topology: FaceLineageTopology,
        edge: EdgeId,
    },
    #[error("{topology:?} edge {edge:?} has a missing endpoint")]
    EdgeEndpointMissing {
        topology: FaceLineageTopology,
        edge: EdgeId,
    },
    #[error("{topology:?} edge {edge:?} has zero geometric length")]
    DegenerateEdge {
        topology: FaceLineageTopology,
        edge: EdgeId,
    },
    #[error("target geometry removed source edge identity {edge:?}")]
    SourceEdgeIdentityMissing { edge: EdgeId },
    #[error("target geometry changed the kind of source edge {edge:?}")]
    SourceEdgeKindChanged { edge: EdgeId },
    #[error("target geometry moved source edge identity {edge:?} off its original segment")]
    SourceEdgeGeometryChanged { edge: EdgeId },
    #[error("target edge {edge:?} does not belong to a source or expected carrier")]
    TargetEdgeWithoutCarrier { edge: EdgeId },
    #[error("target edge {edge:?} belongs to more than one carrier")]
    TargetEdgeWithMultipleCarriers { edge: EdgeId },
    #[error("target subdivisions do not exactly cover {carrier:?}")]
    CarrierCoverageMismatch {
        carrier: StackedFoldGeometryCarrierV1,
    },
    #[error("exact geometry predicate failed: {0}")]
    Geometry(#[from] GeometryError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceLineageTopology {
    Source,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceLineageResource {
    SourceVertices,
    SourceEdges,
    SourcePaperBoundaryVertices,
    TargetVertices,
    TargetEdges,
    TargetPaperBoundaryVertices,
    SourceFaces,
    TargetFaces,
    SourceBoundaryHalfEdges,
    TargetBoundaryHalfEdges,
    FacePairs,
    ExactContainmentTests,
}

#[derive(Debug, Error, PartialEq)]
pub enum FaceLineageError {
    #[error("source revision cannot advance")]
    SourceRevisionCannotAdvance,
    #[error("target revision {actual} is not the required next revision {expected}")]
    TargetRevisionNotNext {
        expected: Revision,
        actual: Revision,
    },
    #[error("{topology:?} topology is not safe and complete ({issue_count} blocking issue(s))")]
    TopologyNotSimulationReady {
        topology: FaceLineageTopology,
        issue_count: usize,
    },
    #[error("the supplied layer order is not current for the source geometry")]
    LayerOrderNotCurrent,
    #[error("the supplied layer order does not use the required model")]
    LayerOrderModelMismatch,
    #[error("the supplied layer-order material registry does not match source topology")]
    LayerOrderMaterialRegistryMismatch,
    #[error("a face-lineage candidate cannot change non-boundary paper properties")]
    PaperPropertiesChanged,
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimit {
        resource: FaceLineageResource,
        actual: usize,
        maximum: usize,
    },
    #[error("source face {face:?} is not convex and cannot use this proof model")]
    SourceFaceNotConvex { face: FaceId },
    #[error("target face {face:?} is not contained by any source face")]
    TargetFaceWithoutSource { face: FaceId },
    #[error("target face {face:?} is contained by more than one source face")]
    TargetFaceWithMultipleSources { face: FaceId },
    #[error("source face {face:?} has no target descendant")]
    SourceFaceWithoutDescendants { face: FaceId },
    #[error("descendants do not conserve the exact area of source face {face:?}")]
    SourceFaceAreaMismatch { face: FaceId },
    #[error("candidate geometry does not split any source face")]
    NoSourceFaceSplit,
    #[error("validated topology lost an internal vertex or face invariant")]
    ValidatedTopologyInvariantLost,
    #[error("exact containment predicate failed: {0}")]
    Geometry(#[from] GeometryError),
}

#[derive(Debug)]
struct PolygonRecord {
    face: LayerFace,
    points: Vec<Point2>,
    exact_double_area: BigInt,
}

/// Proves a complete, deterministic source-face to target-face lineage.
///
/// Both topologies are rebuilt from immutable geometry. Source faces must be
/// convex, matching the current `facewise_layer_order_v1` target class. Every
/// target vertex is then classified exactly against every source polygon. A
/// target is accepted only inside one source, and exact dyadic area
/// conservation is checked independently for every source face.
///
/// The function is read-only. Every error therefore leaves project geometry,
/// revision, timeline, and Undo/Redo history unchanged.
///
/// This foundation has deterministic count limits but no deadline or
/// cancellation channel. A future UI command must run it away from the
/// project-state lock, support cooperative cancellation around this bounded
/// phase, and revalidate the immutable project/layer-order binding before an
/// atomic commit.
pub fn prepare_face_lineage_v1(
    input: FaceLineageInput<'_>,
    limits: FaceLineageLimits,
) -> Result<FaceLineageV1, FaceLineageError> {
    check_limit(
        FaceLineageResource::SourceVertices,
        input.source_pattern.vertices.len(),
        limits.max_source_vertices,
    )?;
    check_limit(
        FaceLineageResource::SourceEdges,
        input.source_pattern.edges.len(),
        limits.max_source_edges,
    )?;
    check_limit(
        FaceLineageResource::SourcePaperBoundaryVertices,
        input.source_paper.boundary_vertices.len(),
        limits.max_source_paper_boundary_vertices,
    )?;
    check_limit(
        FaceLineageResource::TargetVertices,
        input.target_pattern.vertices.len(),
        limits.max_target_vertices,
    )?;
    check_limit(
        FaceLineageResource::TargetEdges,
        input.target_pattern.edges.len(),
        limits.max_target_edges,
    )?;
    check_limit(
        FaceLineageResource::TargetPaperBoundaryVertices,
        input.target_paper.boundary_vertices.len(),
        limits.max_target_paper_boundary_vertices,
    )?;

    if input.source_revision >= MAX_REVISION {
        return Err(FaceLineageError::SourceRevisionCannotAdvance);
    }
    let expected_target_revision = input.source_revision + 1;
    if input.target_revision != expected_target_revision {
        return Err(FaceLineageError::TargetRevisionNotNext {
            expected: expected_target_revision,
            actual: input.target_revision,
        });
    }
    if input.source_paper.thickness_mm.to_bits() != input.target_paper.thickness_mm.to_bits()
        || input.source_paper.cutting_allowed != input.target_paper.cutting_allowed
        || input.source_paper.front != input.target_paper.front
        || input.source_paper.back != input.target_paper.back
    {
        return Err(FaceLineageError::PaperPropertiesChanged);
    }

    let source_topology = simulation_snapshot(
        input.identity_namespace,
        input.source_revision,
        input.source_paper,
        input.source_pattern,
        FaceLineageTopology::Source,
    )?;
    check_topology_limits(&source_topology, FaceLineageTopology::Source, limits)?;

    let source_fingerprint = fold_model_fingerprint_v1(input.source_pattern, input.source_paper);
    let source_provenance = GlobalFlatFoldabilityProvenance::for_geometry(
        input.identity_namespace,
        input.source_revision,
        input.source_paper,
        input.source_pattern,
    );
    if !input.source_layer_order.is_current_for(&source_provenance) {
        return Err(FaceLineageError::LayerOrderNotCurrent);
    }
    if input.source_layer_order.model_id != LAYER_ORDER_MODEL_ID {
        return Err(FaceLineageError::LayerOrderModelMismatch);
    }

    let source_registry = canonical_registry(&source_topology);
    if input.source_layer_order.material_faces.len() != source_registry.len()
        || input.source_layer_order.material_faces != source_registry
    {
        return Err(FaceLineageError::LayerOrderMaterialRegistryMismatch);
    }

    let target_topology = simulation_snapshot(
        input.identity_namespace,
        input.target_revision,
        input.target_paper,
        input.target_pattern,
        FaceLineageTopology::Target,
    )?;
    check_topology_limits(&target_topology, FaceLineageTopology::Target, limits)?;

    let source_polygons = polygon_records(input.source_pattern, &source_topology)?;
    let target_polygons = polygon_records(input.target_pattern, &target_topology)?;
    for source in &source_polygons {
        ensure_convex(source)?;
    }

    let pair_count = source_polygons
        .len()
        .checked_mul(target_polygons.len())
        .ok_or(FaceLineageError::ResourceLimit {
            resource: FaceLineageResource::FacePairs,
            actual: usize::MAX,
            maximum: limits.max_face_pairs,
        })?;
    check_limit(
        FaceLineageResource::FacePairs,
        pair_count,
        limits.max_face_pairs,
    )?;

    let mut exact_tests = 0_usize;
    let mut descendants = vec![Vec::<usize>::new(); source_polygons.len()];
    for (target_index, target) in target_polygons.iter().enumerate() {
        let mut matching_source = None;
        for (source_index, source) in source_polygons.iter().enumerate() {
            let pair_tests = source
                .points
                .len()
                .checked_mul(target.points.len())
                .and_then(|value| value.checked_mul(2))
                .ok_or(FaceLineageError::ResourceLimit {
                    resource: FaceLineageResource::ExactContainmentTests,
                    actual: usize::MAX,
                    maximum: limits.max_exact_containment_tests,
                })?;
            exact_tests =
                exact_tests
                    .checked_add(pair_tests)
                    .ok_or(FaceLineageError::ResourceLimit {
                        resource: FaceLineageResource::ExactContainmentTests,
                        actual: usize::MAX,
                        maximum: limits.max_exact_containment_tests,
                    })?;
            check_limit(
                FaceLineageResource::ExactContainmentTests,
                exact_tests,
                limits.max_exact_containment_tests,
            )?;

            if polygon_is_within_convex_source(&target.points, &source.points)?
                && matching_source.replace(source_index).is_some()
            {
                return Err(FaceLineageError::TargetFaceWithMultipleSources {
                    face: target.face.face_id,
                });
            }
        }
        let source_index = matching_source.ok_or(FaceLineageError::TargetFaceWithoutSource {
            face: target.face.face_id,
        })?;
        descendants[source_index].push(target_index);
    }

    let mut records = Vec::with_capacity(source_polygons.len());
    let mut split_found = false;
    for (source_index, source) in source_polygons.iter().enumerate() {
        let target_indices = &descendants[source_index];
        if target_indices.is_empty() {
            return Err(FaceLineageError::SourceFaceWithoutDescendants {
                face: source.face.face_id,
            });
        }
        split_found |= target_indices.len() > 1;

        let descendant_area = target_indices
            .iter()
            .fold(BigInt::from(0_u8), |area, index| {
                area + &target_polygons[*index].exact_double_area
            });
        if descendant_area != source.exact_double_area {
            return Err(FaceLineageError::SourceFaceAreaMismatch {
                face: source.face.face_id,
            });
        }

        let mut canonical_descendants = target_indices
            .iter()
            .map(|index| target_polygons[*index].face)
            .collect::<Vec<_>>();
        canonical_descendants.sort_unstable_by(compare_layer_faces);
        records.push(FaceLineageRecord {
            source: source.face,
            descendants: canonical_descendants,
        });
    }
    if !split_found {
        return Err(FaceLineageError::NoSourceFaceSplit);
    }
    records.sort_unstable_by(|left, right| compare_layer_faces(&left.source, &right.source));

    Ok(FaceLineageV1 {
        identity_namespace: input.identity_namespace,
        source_revision: input.source_revision,
        target_revision: input.target_revision,
        source_fingerprint,
        target_fingerprint: fold_model_fingerprint_v1(input.target_pattern, input.target_paper),
        records,
    })
}

#[derive(Debug, Clone, Copy)]
struct GeometryVertexRecord {
    id: VertexId,
    position: Point2,
}

#[derive(Debug, Clone, Copy)]
struct GeometryEdgeRecord {
    id: EdgeId,
    start_vertex: VertexId,
    end_vertex: VertexId,
    start: Point2,
    end: Point2,
    kind: EdgeKind,
}

#[derive(Debug, Clone, Copy)]
struct CanonicalExpectedCrease {
    start: Point2,
    end: Point2,
    kind: EdgeKind,
}

#[derive(Debug, Clone, Copy)]
struct GeometryCarrier {
    public: StackedFoldGeometryCarrierV1,
    start: Point2,
    end: Point2,
    kind: EdgeKind,
}

#[derive(Debug, Clone, Copy)]
struct BuildCarrier {
    source_edge: Option<EdgeId>,
    start: Point2,
    end: Point2,
    kind: EdgeKind,
}

/// Builds the complete planar arrangement required by a stacked-fold
/// candidate. All source edges and expected creases are split at their mutual
/// point intersections. Source vertices and one subdivision identity per
/// source edge are preserved; boundary split vertices are inserted into the
/// paper cycle.
///
/// The result is detached data and grants no mutation authority. Callers must
/// still run face-lineage and stacked-fold geometry proofs and reauthenticate
/// all live capabilities before an atomic commit.
pub fn build_stacked_fold_topology_v1(
    identity_namespace: ProjectId,
    source_revision: Revision,
    source_pattern: &CreasePattern,
    source_paper: &Paper,
    expected_creases: &[ExpectedStackedFoldCreaseV1],
    limits: StackedFoldTopologyBuildLimitsV1,
) -> Result<StackedFoldTopologyCandidateV1, StackedFoldTopologyBuildErrorV1> {
    let mut source_positions = HashMap::with_capacity(source_pattern.vertices.len());
    let mut vertex_ids = HashMap::with_capacity(source_pattern.vertices.len());
    for vertex in &source_pattern.vertices {
        let key = point_bits(vertex.position);
        if vertex_ids.insert(key, vertex.id).is_some() {
            return Err(StackedFoldTopologyBuildErrorV1::DuplicateSourceVertexPosition);
        }
        if source_positions
            .insert(vertex.id, vertex.position)
            .is_some()
        {
            return Err(StackedFoldTopologyBuildErrorV1::DuplicateSourceVertexId {
                vertex: vertex.id,
            });
        }
    }

    let carrier_count = source_pattern
        .edges
        .len()
        .checked_add(expected_creases.len())
        .unwrap_or(usize::MAX);
    build_limit(
        StackedFoldTopologyBuildResourceV1::Carriers,
        carrier_count,
        limits.max_carriers,
    )?;
    let pair_tests = carrier_count
        .checked_mul(carrier_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .unwrap_or(usize::MAX);
    build_limit(
        StackedFoldTopologyBuildResourceV1::PairTests,
        pair_tests,
        limits.max_pair_tests,
    )?;

    let mut carriers = Vec::with_capacity(carrier_count);
    let mut canonical_source_edges = source_pattern.edges.iter().collect::<Vec<_>>();
    canonical_source_edges.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    for edge in canonical_source_edges {
        let start = source_positions
            .get(&edge.start)
            .copied()
            .ok_or(StackedFoldTopologyBuildErrorV1::SourceEdgeEndpointMissing { edge: edge.id })?;
        let end = source_positions
            .get(&edge.end)
            .copied()
            .ok_or(StackedFoldTopologyBuildErrorV1::SourceEdgeEndpointMissing { edge: edge.id })?;
        carriers.push(BuildCarrier {
            source_edge: Some(edge.id),
            start,
            end,
            kind: edge.kind,
        });
    }
    let mut canonical_expected = expected_creases.to_vec();
    canonical_expected.sort_unstable_by(compare_expected_crease);
    for crease in &canonical_expected {
        if !crease.start.x.is_finite()
            || !crease.start.y.is_finite()
            || !crease.end.x.is_finite()
            || !crease.end.y.is_finite()
            || crease.start == crease.end
            || !matches!(crease.kind, EdgeKind::Mountain | EdgeKind::Valley)
        {
            return Err(StackedFoldTopologyBuildErrorV1::InvalidExpectedCrease);
        }
        let (start, end) = if point_bits(crease.start) <= point_bits(crease.end) {
            (crease.start, crease.end)
        } else {
            (crease.end, crease.start)
        };
        carriers.push(BuildCarrier {
            source_edge: None,
            start,
            end,
            kind: crease.kind,
        });
    }

    let mut carrier_points = carriers
        .iter()
        .map(|carrier| vec![carrier.start, carrier.end])
        .collect::<Vec<_>>();
    for first in 0..carriers.len() {
        for second in first + 1..carriers.len() {
            match segment_intersection(
                carriers[first].start,
                carriers[first].end,
                carriers[second].start,
                carriers[second].end,
            )? {
                SegmentIntersection::None => {}
                SegmentIntersection::Point(point) => {
                    push_unique_point(&mut carrier_points[first], point);
                    push_unique_point(&mut carrier_points[second], point);
                }
                SegmentIntersection::CollinearOverlap => {
                    return Err(StackedFoldTopologyBuildErrorV1::CarrierOverlap { first, second });
                }
            }
        }
    }

    for (carrier, points) in carriers.iter().zip(&mut carrier_points) {
        points.sort_unstable_by(|left, right| compare_along(*carrier, *left, *right));
        points.dedup_by(|left, right| point_bits(*left) == point_bits(*right));
        for point in points.iter().copied() {
            if !vertex_ids.contains_key(&point_bits(point)) {
                let id = derived_vertex_id(identity_namespace, source_revision, point);
                if source_positions.contains_key(&id)
                    || vertex_ids.values().any(|value| *value == id)
                {
                    return Err(StackedFoldTopologyBuildErrorV1::DerivedIdentityCollision);
                }
                vertex_ids.insert(point_bits(point), id);
            }
        }
    }
    build_limit(
        StackedFoldTopologyBuildResourceV1::Vertices,
        vertex_ids.len(),
        limits.max_vertices,
    )?;

    let mut vertices = source_pattern.vertices.clone();
    let source_vertex_ids = vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    let mut new_vertices = vertex_ids
        .iter()
        .filter(|(_, id)| !source_vertex_ids.contains(id))
        .map(|(&(x, y), &id)| Vertex {
            id,
            position: Point2::new(f64::from_bits(x), f64::from_bits(y)),
        })
        .collect::<Vec<_>>();
    new_vertices.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    vertices.extend(new_vertices);

    let estimated_edges = carrier_points
        .iter()
        .try_fold(0_usize, |total, points| {
            total.checked_add(points.len().saturating_sub(1))
        })
        .unwrap_or(usize::MAX);
    build_limit(
        StackedFoldTopologyBuildResourceV1::Edges,
        estimated_edges,
        limits.max_edges,
    )?;
    let mut edges = Vec::with_capacity(estimated_edges);
    let source_edge_ids = source_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<HashSet<_>>();
    let mut emitted_edge_ids = HashSet::with_capacity(estimated_edges);
    for (carrier_index, (carrier, points)) in carriers.iter().zip(&carrier_points).enumerate() {
        for (index, pair) in points.windows(2).enumerate() {
            let id = if index == 0 {
                carrier.source_edge.unwrap_or_else(|| {
                    derived_edge_id(
                        identity_namespace,
                        source_revision,
                        carrier_index,
                        pair[0],
                        pair[1],
                        carrier.kind,
                    )
                })
            } else {
                derived_edge_id(
                    identity_namespace,
                    source_revision,
                    carrier_index,
                    pair[0],
                    pair[1],
                    carrier.kind,
                )
            };
            if !emitted_edge_ids.insert(id)
                || (source_edge_ids.contains(&id) && carrier.source_edge != Some(id))
            {
                return Err(StackedFoldTopologyBuildErrorV1::DerivedIdentityCollision);
            }
            edges.push(Edge {
                id,
                start: vertex_ids[&point_bits(pair[0])],
                end: vertex_ids[&point_bits(pair[1])],
                kind: carrier.kind,
            });
        }
    }

    let mut paper = source_paper.clone();
    let mut boundary = Vec::new();
    for index in 0..source_paper.boundary_vertices.len() {
        let start_id = source_paper.boundary_vertices[index];
        let end_id =
            source_paper.boundary_vertices[(index + 1) % source_paper.boundary_vertices.len()];
        let start_position = source_positions.get(&start_id).copied().ok_or(
            StackedFoldTopologyBuildErrorV1::PaperBoundaryVertexMissing { vertex: start_id },
        )?;
        let end_position = source_positions.get(&end_id).copied().ok_or(
            StackedFoldTopologyBuildErrorV1::PaperBoundaryVertexMissing { vertex: end_id },
        )?;
        let carrier_index = carriers.iter().position(|carrier| {
            carrier.source_edge.is_some()
                && ((point_bits(carrier.start) == point_bits(start_position)
                    && point_bits(carrier.end) == point_bits(end_position))
                    || (point_bits(carrier.end) == point_bits(start_position)
                        && point_bits(carrier.start) == point_bits(end_position)))
        });
        let Some(carrier_index) = carrier_index else {
            return Err(StackedFoldTopologyBuildErrorV1::PaperBoundaryCarrierMissing);
        };
        let carrier = BuildCarrier {
            start: start_position,
            end: end_position,
            ..carriers[carrier_index]
        };
        let mut points = carrier_points[carrier_index].clone();
        points.sort_unstable_by(|left, right| compare_along(carrier, *left, *right));
        boundary.extend(
            points[..points.len() - 1]
                .iter()
                .map(|point| vertex_ids[&point_bits(*point)]),
        );
    }
    paper.boundary_vertices = boundary;

    Ok(StackedFoldTopologyCandidateV1 {
        pattern: CreasePattern { vertices, edges },
        paper,
    })
}

/// Builds and proves one detached stacked-fold geometry candidate as a single
/// fail-closed operation.
///
/// Keeping the candidate and its owning proof together prevents callers from
/// accidentally mixing geometry, lineage, or revisions between preparation
/// phases. The package remains read-only and must be rebound to live
/// pose/layer/collision authority by the eventual commit command.
pub fn prepare_stacked_fold_geometry_candidate_v1(
    identity_namespace: ProjectId,
    source_revision: Revision,
    source_pattern: &CreasePattern,
    source_paper: &Paper,
    source_layer_order: &LayerOrderSnapshot,
    expected_creases: &[ExpectedStackedFoldCreaseV1],
    topology_limits: StackedFoldTopologyBuildLimitsV1,
    lineage_limits: FaceLineageLimits,
    geometry_limits: StackedFoldGeometryLimitsV1,
) -> Result<PreparedStackedFoldGeometryV1, PrepareStackedFoldGeometryErrorV1> {
    let target_revision = source_revision
        .checked_add(1)
        .filter(|revision| *revision <= MAX_REVISION)
        .ok_or(PrepareStackedFoldGeometryErrorV1::SourceRevisionCannotAdvance)?;
    let candidate = build_stacked_fold_topology_v1(
        identity_namespace,
        source_revision,
        source_pattern,
        source_paper,
        expected_creases,
        topology_limits,
    )?;
    let lineage = prepare_face_lineage_v1(
        FaceLineageInput {
            identity_namespace,
            source_revision,
            source_paper,
            source_pattern,
            source_layer_order,
            target_revision,
            target_paper: &candidate.paper,
            target_pattern: &candidate.pattern,
        },
        lineage_limits,
    )?;
    let proof = prepare_stacked_fold_geometry_v1(
        StackedFoldGeometryInputV1 {
            identity_namespace,
            source_revision,
            source_paper,
            source_pattern,
            target_revision,
            target_paper: &candidate.paper,
            target_pattern: &candidate.pattern,
            face_lineage: &lineage,
            expected_creases,
        },
        geometry_limits,
    )?;
    Ok(PreparedStackedFoldGeometryV1 { candidate, proof })
}

/// Reconstructs and audits the proved target hinge graph without attempting
/// to solve it.
///
/// Cycles are retained as explicit closure hinges rather than rejected or
/// silently reduced to a tree. The returned package is the opaque transport
/// premise for a later [`ori_kinematics::MaterialHingeClosureCertificate`].
pub fn prepare_stacked_fold_target_graph_audit_v1(
    geometry: PreparedStackedFoldGeometryV1,
    limits: TreeKinematicsLimits,
) -> Result<PreparedStackedFoldTargetGraphAuditV1, PrepareStackedFoldTargetGraphAuditErrorV1> {
    let lineage = geometry.proof.lineage();
    let topology = simulation_snapshot(
        lineage.identity_namespace(),
        lineage.target_revision(),
        &geometry.candidate.paper,
        &geometry.candidate.pattern,
        FaceLineageTopology::Target,
    )?;
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, limits).map_err(|error| match error {
            KinematicsError::ResourceLimitExceeded => {
                PrepareStackedFoldTargetGraphAuditErrorV1::ResourceLimit
            }
            KinematicsError::UnsupportedTopology => {
                PrepareStackedFoldTargetGraphAuditErrorV1::UnsupportedTopology
            }
            KinematicsError::UnrepresentableGeometry => {
                PrepareStackedFoldTargetGraphAuditErrorV1::UnrepresentableGeometry
            }
            _ => PrepareStackedFoldTargetGraphAuditErrorV1::UnsupportedTopology,
        })?;
    let hinge_geometry = MaterialHingeGraphGeometry::prepare(
        &geometry.candidate.pattern,
        &geometry.candidate.paper,
        &topology,
        limits,
    )
    .map_err(|error| match error {
        KinematicsError::ResourceLimitExceeded => {
            PrepareStackedFoldTargetGraphAuditErrorV1::ResourceLimit
        }
        KinematicsError::UnrepresentableGeometry => {
            PrepareStackedFoldTargetGraphAuditErrorV1::UnrepresentableGeometry
        }
        _ => PrepareStackedFoldTargetGraphAuditErrorV1::UnsupportedTopology,
    })?;
    Ok(PreparedStackedFoldTargetGraphAuditV1 {
        geometry,
        audit,
        hinge_geometry,
    })
}

/// Reconstructs and admits the proved target as a native material-tree model.
///
/// This rejects planar arrangements whose face adjacency cannot be represented
/// by the current tree-kinematics target class. It does not select a fixed
/// descendant, assign target hinge angles, solve a pose, or authorize commit.
pub fn prepare_stacked_fold_target_model_v1(
    geometry: PreparedStackedFoldGeometryV1,
    limits: TreeKinematicsLimits,
) -> Result<PreparedStackedFoldTargetModelV1, PrepareStackedFoldTargetModelErrorV1> {
    let audited = prepare_stacked_fold_target_graph_audit_v1(geometry, limits).map_err(
        |error| match error {
            PrepareStackedFoldTargetGraphAuditErrorV1::Topology(error) => {
                PrepareStackedFoldTargetModelErrorV1::Topology(error)
            }
            PrepareStackedFoldTargetGraphAuditErrorV1::ResourceLimit => {
                PrepareStackedFoldTargetModelErrorV1::Kinematics(
                    KinematicsError::ResourceLimitExceeded,
                )
            }
            PrepareStackedFoldTargetGraphAuditErrorV1::UnsupportedTopology => {
                PrepareStackedFoldTargetModelErrorV1::Kinematics(
                    KinematicsError::UnsupportedTopology,
                )
            }
            PrepareStackedFoldTargetGraphAuditErrorV1::UnrepresentableGeometry => {
                PrepareStackedFoldTargetModelErrorV1::Kinematics(
                    KinematicsError::UnrepresentableGeometry,
                )
            }
        },
    )?;
    if !audited.audit.is_tree() {
        return Err(
            PrepareStackedFoldTargetModelErrorV1::CyclicTargetUnsupported {
                closure_hinge_count: audited.audit.closure_hinges().len(),
            },
        );
    }
    let geometry = audited.geometry;
    let lineage = geometry.proof.lineage();
    let topology = simulation_snapshot(
        lineage.identity_namespace(),
        lineage.target_revision(),
        &geometry.candidate.paper,
        &geometry.candidate.pattern,
        FaceLineageTopology::Target,
    )?;
    let model = MaterialTreeKinematicsModel::prepare(
        &geometry.candidate.pattern,
        &geometry.candidate.paper,
        &topology,
        limits,
    )?;
    Ok(PreparedStackedFoldTargetModelV1 { geometry, model })
}

/// Lifts the current source pose onto the proved target topology before the
/// new collective hinge moves. Source hinge subdivisions inherit their source
/// angles and every newly expected crease starts at zero.
pub fn prepare_stacked_fold_initial_pose_v1(
    target: PreparedStackedFoldTargetModelV1,
    source_model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
) -> Result<PreparedStackedFoldInitialPoseV1, PrepareStackedFoldInitialPoseErrorV1> {
    if !source_model.owns_pose(source_pose) {
        return Err(PrepareStackedFoldInitialPoseErrorV1::SourcePoseIssuerMismatch);
    }
    let source_angles = source_pose
        .hinge_angles()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<HashMap<_, _>>();
    let proof = target.geometry.proof();
    let mut target_angle_values = HashMap::<EdgeId, f64>::new();
    for subdivision in proof.source_edges() {
        let source_edge = subdivision.source_edge();
        if let Some(angle) = source_angles.get(&source_edge).copied() {
            for edge in subdivision.target_edges() {
                target_angle_values.insert(*edge, angle);
            }
        }
    }
    for subdivision in proof.expected_creases() {
        for edge in subdivision.target_edges() {
            target_angle_values.insert(*edge, 0.0);
        }
    }
    let mut angles = target
        .model
        .hinges()
        .iter()
        .map(|hinge| {
            let edge = hinge.edge();
            let angle = target_angle_values
                .get(&edge)
                .copied()
                .ok_or(PrepareStackedFoldInitialPoseErrorV1::TargetHingeWithoutCarrier { edge })?;
            HingeAngle::new(edge, angle).map_err(PrepareStackedFoldInitialPoseErrorV1::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
    let angles = CanonicalHingeAngles::new(angles)?;

    let lineage = proof.lineage();
    let fixed_face = match source_pose.fixed_face() {
        None if target.model.hinges().is_empty() => None,
        None => Some(
            lineage
                .records()
                .first()
                .and_then(|record| record.descendants().first())
                .ok_or(PrepareStackedFoldInitialPoseErrorV1::SourceFixedFaceMissing)?
                .face_id,
        ),
        Some(source_fixed) => Some(
            lineage
                .records()
                .iter()
                .find(|record| record.source().face_id == source_fixed)
                .and_then(|record| record.descendants().first())
                .ok_or(PrepareStackedFoldInitialPoseErrorV1::SourceFixedFaceMissing)?
                .face_id,
        ),
    };
    let pose = target.model.solve(fixed_face, &angles)?;
    for record in lineage.records() {
        let source_face = record.source().face_id;
        let source_transform = source_pose.face_transform(source_face).ok_or(
            PrepareStackedFoldInitialPoseErrorV1::SourcePoseFaceMissing { face: source_face },
        )?;
        for descendant in record.descendants() {
            if pose.face_transform(descendant.face_id) != Some(source_transform) {
                return Err(
                    PrepareStackedFoldInitialPoseErrorV1::DescendantTransformMismatch {
                        face: source_face,
                    },
                );
            }
        }
    }
    Ok(PreparedStackedFoldInitialPoseV1 { target, pose })
}

/// Lifts the authenticated source embedding onto a proved target graph and
/// admits it only after every spanning and closure hinge has been observed.
pub fn prepare_stacked_fold_initial_graph_pose_v1(
    target: PreparedStackedFoldTargetGraphAuditV1,
    source_model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
) -> Result<PreparedStackedFoldInitialGraphPoseV1, PrepareStackedFoldInitialPoseErrorV1> {
    if !source_model.owns_pose(source_pose) {
        return Err(PrepareStackedFoldInitialPoseErrorV1::SourcePoseIssuerMismatch);
    }
    let source_angles = source_pose
        .hinge_angles()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<HashMap<_, _>>();
    let proof = target.geometry.proof();
    let mut target_angle_values = HashMap::<EdgeId, f64>::new();
    for subdivision in proof.source_edges() {
        if let Some(angle) = source_angles.get(&subdivision.source_edge()).copied() {
            for edge in subdivision.target_edges() {
                target_angle_values.insert(*edge, angle);
            }
        }
    }
    for subdivision in proof.expected_creases() {
        for edge in subdivision.target_edges() {
            target_angle_values.insert(*edge, 0.0);
        }
    }
    let mut angles = target
        .hinge_geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let edge = hinge.edge();
            let angle = target_angle_values
                .get(&edge)
                .copied()
                .ok_or(PrepareStackedFoldInitialPoseErrorV1::TargetHingeWithoutCarrier { edge })?;
            HingeAngle::new(edge, angle).map_err(PrepareStackedFoldInitialPoseErrorV1::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
    let angles = CanonicalHingeAngles::new(angles)?;
    let lineage = proof.lineage();
    let fixed_face = match source_pose.fixed_face() {
        None => {
            lineage
                .records()
                .first()
                .and_then(|record| record.descendants().first())
                .ok_or(PrepareStackedFoldInitialPoseErrorV1::SourceFixedFaceMissing)?
                .face_id
        }
        Some(source_fixed) => {
            lineage
                .records()
                .iter()
                .find(|record| record.source().face_id == source_fixed)
                .and_then(|record| record.descendants().first())
                .ok_or(PrepareStackedFoldInitialPoseErrorV1::SourceFixedFaceMissing)?
                .face_id
        }
    };
    let mut candidate = Vec::new();
    candidate
        .try_reserve_exact(target.hinge_geometry.face_ids().len())
        .map_err(|_| {
            PrepareStackedFoldInitialPoseErrorV1::Kinematics(KinematicsError::ResourceLimitExceeded)
        })?;
    for record in lineage.records() {
        let source_face = record.source().face_id;
        let transform = source_pose.face_transform(source_face).ok_or(
            PrepareStackedFoldInitialPoseErrorV1::SourcePoseFaceMissing { face: source_face },
        )?;
        for descendant in record.descendants() {
            candidate.push(CandidateFaceTransform::new(descendant.face_id, transform));
        }
    }
    let pose = target.hinge_geometry.observe_closed(
        &target.audit,
        fixed_face,
        &angles,
        &candidate,
        STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
    )?;
    Ok(PreparedStackedFoldInitialGraphPoseV1 { target, pose })
}

/// Solves the requested endpoint by moving every proved new crease
/// subdivision as one collective hinge angle.
///
/// This is endpoint geometry only. It deliberately carries no continuous
/// collision, safe-stop, layer-order, timeline, or mutation authority.
pub fn prepare_stacked_fold_requested_pose_v1(
    initial: PreparedStackedFoldInitialPoseV1,
    requested_angle_degrees: f64,
) -> Result<PreparedStackedFoldRequestedPoseV1, PrepareStackedFoldRequestedPoseErrorV1> {
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || requested_angle_degrees > 180.0
        || requested_angle_degrees.to_bits() == (-0.0_f64).to_bits()
    {
        return Err(PrepareStackedFoldRequestedPoseErrorV1::InvalidRequestedAngle);
    }
    let expected_edges = initial
        .target
        .geometry
        .proof()
        .expected_creases()
        .iter()
        .flat_map(|subdivision| subdivision.target_edges().iter().copied())
        .collect::<HashSet<_>>();
    let target_hinges = initial
        .target
        .model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    if expected_edges.is_empty() || !expected_edges.is_subset(&target_hinges) {
        return Err(PrepareStackedFoldRequestedPoseErrorV1::ExpectedCreaseHingeMissing);
    }
    let mut angles = initial
        .pose
        .hinge_angles()
        .iter()
        .map(|angle| {
            HingeAngle::new(
                angle.edge(),
                if expected_edges.contains(&angle.edge()) {
                    requested_angle_degrees
                } else {
                    angle.angle_degrees()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
    let angles = CanonicalHingeAngles::new(angles)?;
    let pose = initial
        .target
        .model
        .solve(initial.pose.fixed_face(), &angles)?;
    Ok(PreparedStackedFoldRequestedPoseV1 {
        initial,
        pose,
        requested_angle_degrees,
    })
}

/// Re-authenticates the source layer snapshot and recomputes the narrow
/// non-flat target class whose face supports are pairwise non-coincident.
///
/// Pairwise intersecting or merely near-coincident planes are not silently
/// ordered: the former remains collision analysis' responsibility and the
/// latter is rejected here because a local ply order could be required.
pub fn prepare_stacked_fold_non_flat_layer_order_v1(
    requested: &PreparedStackedFoldRequestedPoseV1,
    source_layer_order: &LayerOrderSnapshot,
    max_face_pairs: usize,
) -> Result<StackedFoldNonFlatLayerOrderV1, PrepareStackedFoldNonFlatLayerOrderErrorV1> {
    let angle = requested.requested_angle_degrees();
    if !angle.is_finite() || angle <= 0.0 || angle >= 180.0 {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::NotNonFlatEndpoint);
    }
    let lineage = requested.initial.target.geometry.proof.lineage();
    let provenance = &source_layer_order.provenance.source;
    if source_layer_order.model_id != LAYER_ORDER_MODEL_ID
        || provenance.identity_namespace != Some(lineage.identity_namespace())
        || provenance.source_revision != lineage.source_revision()
        || provenance.source_fingerprint != Some(lineage.source_fingerprint())
    {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::SourceLayerOrderMismatch);
    }
    let mut source_faces = lineage
        .records()
        .iter()
        .map(FaceLineageRecord::source)
        .collect::<Vec<_>>();
    source_faces.sort_unstable_by(|first, second| {
        first.face_key.cmp(&second.face_key).then_with(|| {
            first
                .face_id
                .canonical_bytes()
                .cmp(&second.face_id.canonical_bytes())
        })
    });
    if source_layer_order.material_faces != source_faces {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::SourceLayerOrderMismatch);
    }
    let pose = requested.pose();
    if !requested.initial.target.model.owns_pose(pose) {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::TargetPoseMismatch);
    }
    let mut material_faces = lineage
        .records()
        .iter()
        .flat_map(|record| record.descendants().iter().copied())
        .collect::<Vec<_>>();
    material_faces.sort_unstable_by(|first, second| {
        first.face_key.cmp(&second.face_key).then_with(|| {
            first
                .face_id
                .canonical_bytes()
                .cmp(&second.face_id.canonical_bytes())
        })
    });
    let mut material_face_ids = material_faces
        .iter()
        .map(|face| face.face_id)
        .collect::<Vec<_>>();
    material_face_ids.sort_unstable_by_key(FaceId::canonical_bytes);
    if material_face_ids != pose.face_ids() {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::TargetPoseMismatch);
    }
    let tested_face_pairs = material_faces
        .len()
        .checked_mul(material_faces.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(PrepareStackedFoldNonFlatLayerOrderErrorV1::ResourceLimit)?;
    if tested_face_pairs > max_face_pairs {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::ResourceLimit);
    }
    let planes = material_faces
        .iter()
        .map(|face| target_face_plane(pose, face.face_id))
        .collect::<Result<Vec<_>, _>>()?;
    for first in 0..planes.len() {
        for second in (first + 1)..planes.len() {
            if planar_supports_may_coincide(planes[first], planes[second])? {
                return Err(
                    PrepareStackedFoldNonFlatLayerOrderErrorV1::CoincidentPlanarSupports {
                        first: material_faces[first].face_id,
                        second: material_faces[second].face_id,
                    },
                );
            }
        }
    }
    Ok(StackedFoldNonFlatLayerOrderV1 {
        target_revision: lineage.target_revision(),
        material_faces,
        tested_face_pairs,
        source_overlap_cells_authenticated: source_layer_order.overlap_cells.len(),
    })
}

fn target_face_plane(
    pose: &MaterialTreePose,
    face: FaceId,
) -> Result<(Point3, Point3), PrepareStackedFoldNonFlatLayerOrderErrorV1> {
    let boundary = pose
        .face_boundary(face)
        .ok_or(PrepareStackedFoldNonFlatLayerOrderErrorV1::TargetPoseMismatch)?;
    let transform = pose
        .face_transform(face)
        .ok_or(PrepareStackedFoldNonFlatLayerOrderErrorV1::TargetPoseMismatch)?;
    let points = boundary
        .vertices()
        .iter()
        .map(|vertex| {
            pose.vertex_position(*vertex)
                .ok_or(PrepareStackedFoldNonFlatLayerOrderErrorV1::TargetPoseMismatch)
                .and_then(|point| transform.apply_point(point).map_err(Into::into))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let origin = *points
        .first()
        .ok_or(PrepareStackedFoldNonFlatLayerOrderErrorV1::UnrepresentableFacePlane)?;
    for index in 1..points.len().saturating_sub(1) {
        let first = point_delta(points[index], origin);
        let second = point_delta(points[index + 1], origin);
        let cross = point_cross(first, second);
        let length = point_length(cross);
        if length.is_finite() && length > 1.0e-12 {
            return Ok((origin, point_scale(cross, 1.0 / length)?));
        }
    }
    Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::UnrepresentableFacePlane)
}

fn planar_supports_may_coincide(
    first: (Point3, Point3),
    second: (Point3, Point3),
) -> Result<bool, PrepareStackedFoldNonFlatLayerOrderErrorV1> {
    let normal_cross = point_length(point_cross(point_values(first.1), point_values(second.1)));
    let separation = point_dot(point_delta(second.0, first.0), first.1).abs();
    if !normal_cross.is_finite() || !separation.is_finite() {
        return Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::UnrepresentableFacePlane);
    }
    Ok(normal_cross <= 1.0e-9 && separation <= 1.0e-9)
}

fn point_delta(first: Point3, second: Point3) -> [f64; 3] {
    [
        first.x() - second.x(),
        first.y() - second.y(),
        first.z() - second.z(),
    ]
}

fn point_values(point: Point3) -> [f64; 3] {
    [point.x(), point.y(), point.z()]
}

fn point_cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

fn point_dot(first: [f64; 3], second: Point3) -> f64 {
    first[0] * second.x() + first[1] * second.y() + first[2] * second.z()
}

fn point_length(point: [f64; 3]) -> f64 {
    (point[0].powi(2) + point[1].powi(2) + point[2].powi(2)).sqrt()
}

fn point_scale(
    point: [f64; 3],
    scale: f64,
) -> Result<Point3, PrepareStackedFoldNonFlatLayerOrderErrorV1> {
    Point3::new(point[0] * scale, point[1] * scale, point[2] * scale).map_err(Into::into)
}

/// Solves a deterministic spanning candidate for a cyclic or acyclic target
/// and fails closed unless every retained loop constraint closes.
pub fn prepare_stacked_fold_requested_graph_pose_v1(
    initial: PreparedStackedFoldInitialGraphPoseV1,
    requested_angle_degrees: f64,
) -> Result<PreparedStackedFoldRequestedGraphPoseV1, PrepareStackedFoldRequestedPoseErrorV1> {
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || requested_angle_degrees > 180.0
        || requested_angle_degrees.to_bits() == (-0.0_f64).to_bits()
    {
        return Err(PrepareStackedFoldRequestedPoseErrorV1::InvalidRequestedAngle);
    }
    let expected_edges = initial
        .target
        .geometry
        .proof()
        .expected_creases()
        .iter()
        .flat_map(|subdivision| subdivision.target_edges().iter().copied())
        .collect::<HashSet<_>>();
    let target_edges = initial
        .target
        .hinge_geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    if expected_edges.is_empty() || !expected_edges.is_subset(&target_edges) {
        return Err(PrepareStackedFoldRequestedPoseErrorV1::ExpectedCreaseHingeMissing);
    }
    let mut angles = initial
        .pose
        .hinge_angles()
        .as_slice()
        .iter()
        .map(|angle| {
            HingeAngle::new(
                angle.edge(),
                if expected_edges.contains(&angle.edge()) {
                    requested_angle_degrees
                } else {
                    angle.angle_degrees()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
    let angles = CanonicalHingeAngles::new(angles)?;
    let pose = initial.target.hinge_geometry.solve_closed(
        &initial.target.audit,
        initial.pose.fixed_face(),
        &angles,
        STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
    )?;
    Ok(PreparedStackedFoldRequestedGraphPoseV1 {
        initial,
        pose,
        requested_angle_degrees,
    })
}

fn compare_expected_crease(
    left: &ExpectedStackedFoldCreaseV1,
    right: &ExpectedStackedFoldCreaseV1,
) -> Ordering {
    let canonical = |crease: &ExpectedStackedFoldCreaseV1| {
        let mut endpoints = [point_bits(crease.start), point_bits(crease.end)];
        endpoints.sort_unstable();
        (endpoints, crease.kind as u8)
    };
    canonical(left).cmp(&canonical(right))
}

fn derived_vertex_id(
    identity_namespace: ProjectId,
    source_revision: Revision,
    point: Point2,
) -> VertexId {
    let mut name = b"stacked-fold-target-v1\0vertex\0".to_vec();
    let (x, y) = point_bits(point);
    name.extend_from_slice(&source_revision.to_be_bytes());
    name.extend_from_slice(&x.to_be_bytes());
    name.extend_from_slice(&y.to_be_bytes());
    VertexId::derive_v5(identity_namespace, &name)
}

fn derived_edge_id(
    identity_namespace: ProjectId,
    source_revision: Revision,
    carrier_index: usize,
    start: Point2,
    end: Point2,
    kind: EdgeKind,
) -> EdgeId {
    let mut name = b"stacked-fold-target-v1\0edge\0".to_vec();
    let (start_x, start_y) = point_bits(start);
    let (end_x, end_y) = point_bits(end);
    name.extend_from_slice(&source_revision.to_be_bytes());
    name.extend_from_slice(&(carrier_index as u64).to_be_bytes());
    name.extend_from_slice(&start_x.to_be_bytes());
    name.extend_from_slice(&start_y.to_be_bytes());
    name.extend_from_slice(&end_x.to_be_bytes());
    name.extend_from_slice(&end_y.to_be_bytes());
    name.push(kind as u8);
    EdgeId::derive_v5(identity_namespace, &name)
}

fn point_bits(point: Point2) -> (u64, u64) {
    (
        canonical_coordinate_bits(point.x),
        canonical_coordinate_bits(point.y),
    )
}

fn push_unique_point(points: &mut Vec<Point2>, point: Point2) {
    if !points
        .iter()
        .any(|candidate| point_bits(*candidate) == point_bits(point))
    {
        points.push(point);
    }
}

fn compare_along(carrier: BuildCarrier, left: Point2, right: Point2) -> Ordering {
    let dx = (carrier.end.x - carrier.start.x).abs();
    let dy = (carrier.end.y - carrier.start.y).abs();
    let ordering = if dx >= dy {
        if carrier.start.x <= carrier.end.x {
            left.x.total_cmp(&right.x)
        } else {
            right.x.total_cmp(&left.x)
        }
    } else if carrier.start.y <= carrier.end.y {
        left.y.total_cmp(&right.y)
    } else {
        right.y.total_cmp(&left.y)
    };
    ordering
        .then_with(|| left.x.total_cmp(&right.x))
        .then_with(|| left.y.total_cmp(&right.y))
}

fn build_limit(
    resource: StackedFoldTopologyBuildResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), StackedFoldTopologyBuildErrorV1> {
    if actual > maximum {
        Err(StackedFoldTopologyBuildErrorV1::ResourceLimit {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

/// Proves the exact, narrow geometry-delta class admitted by stacked-fold v1.
///
/// Every source vertex ID and its binary64 coordinate bits are preserved.
/// Every source edge ID survives as one member of a same-kind subdivision, and
/// target subdivisions have an exact, gap-free, overlap-free union equal to
/// that source segment. Every remaining target edge must form the corresponding
/// exact union of one explicitly expected M/V crease. New isolated vertices,
/// unrelated edges, extra creases, coincident expected carriers, and any
/// unpreserved source material are rejected.
///
/// The function is pure over immutable inputs. It does not create an editor
/// command and cannot mutate project geometry, revision, history, or timeline.
/// Its expected-crease slice is comparison data only; the proof does not
/// authenticate reverse mapping, world-line straightness, selected layers, or
/// current layer authority.
pub fn prepare_stacked_fold_geometry_v1(
    input: StackedFoldGeometryInputV1<'_>,
    limits: StackedFoldGeometryLimitsV1,
) -> Result<StackedFoldGeometryProofV1, StackedFoldGeometryErrorV1> {
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::SourceVertices,
        input.source_pattern.vertices.len(),
        limits.max_source_vertices,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::SourceEdges,
        input.source_pattern.edges.len(),
        limits.max_source_edges,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::SourcePaperBoundaryVertices,
        input.source_paper.boundary_vertices.len(),
        limits.max_source_paper_boundary_vertices,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::TargetVertices,
        input.target_pattern.vertices.len(),
        limits.max_target_vertices,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::TargetEdges,
        input.target_pattern.edges.len(),
        limits.max_target_edges,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::TargetPaperBoundaryVertices,
        input.target_paper.boundary_vertices.len(),
        limits.max_target_paper_boundary_vertices,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::ExpectedCreases,
        input.expected_creases.len(),
        limits.max_expected_creases,
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::LineageRecords,
        input.face_lineage.records.len(),
        limits.max_lineage_records,
    )?;
    let lineage_descendants = input
        .face_lineage
        .records
        .iter()
        .try_fold(0_usize, |total, record| {
            total.checked_add(record.descendants.len())
        })
        .ok_or(StackedFoldGeometryErrorV1::ResourceLimit {
            resource: StackedFoldGeometryResourceV1::LineageDescendants,
            actual: usize::MAX,
            maximum: limits.max_lineage_descendants,
        })?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::LineageDescendants,
        lineage_descendants,
        limits.max_lineage_descendants,
    )?;

    if input.face_lineage.identity_namespace != input.identity_namespace {
        return Err(StackedFoldGeometryErrorV1::LineageIdentityMismatch);
    }
    if input.face_lineage.source_revision != input.source_revision
        || input.face_lineage.target_revision != input.target_revision
    {
        return Err(StackedFoldGeometryErrorV1::LineageRevisionMismatch);
    }
    let source_fingerprint = fold_model_fingerprint_v1(input.source_pattern, input.source_paper);
    if input.face_lineage.source_fingerprint != source_fingerprint {
        return Err(StackedFoldGeometryErrorV1::LineageSourceFingerprintMismatch);
    }
    let target_fingerprint = fold_model_fingerprint_v1(input.target_pattern, input.target_paper);
    if input.face_lineage.target_fingerprint != target_fingerprint {
        return Err(StackedFoldGeometryErrorV1::LineageTargetFingerprintMismatch);
    }

    let expected_creases = canonical_expected_creases(input.expected_creases)?;
    if expected_creases.is_empty() {
        return Err(StackedFoldGeometryErrorV1::ExpectedCreaseSetEmpty);
    }

    let source_vertices =
        geometry_vertex_records(input.source_pattern, FaceLineageTopology::Source)?;
    let target_vertices =
        geometry_vertex_records(input.target_pattern, FaceLineageTopology::Target)?;
    let source_positions = vertex_position_map(&source_vertices);
    let target_positions = vertex_position_map(&target_vertices);
    for source in &source_vertices {
        let Some(target_position) = target_positions.get(&source.id) else {
            return Err(StackedFoldGeometryErrorV1::SourceVertexMissing { vertex: source.id });
        };
        if !point_bits_equal(source.position, *target_position) {
            return Err(StackedFoldGeometryErrorV1::SourceVertexMoved { vertex: source.id });
        }
    }

    let source_edges = geometry_edge_records(
        input.source_pattern,
        &source_positions,
        FaceLineageTopology::Source,
    )?;
    let target_edges = geometry_edge_records(
        input.target_pattern,
        &target_positions,
        FaceLineageTopology::Target,
    )?;
    let target_incident_vertices = target_edges
        .iter()
        .flat_map(|edge| [edge.start_vertex, edge.end_vertex])
        .collect::<HashSet<_>>();
    for target in &target_vertices {
        if !source_positions.contains_key(&target.id)
            && !target_incident_vertices.contains(&target.id)
        {
            return Err(StackedFoldGeometryErrorV1::NewTargetVertexIsolated { vertex: target.id });
        }
    }

    let overlap_tests = checked_overlap_test_count(source_edges.len(), expected_creases.len())
        .ok_or(StackedFoldGeometryErrorV1::ResourceLimit {
            resource: StackedFoldGeometryResourceV1::CarrierOverlapTests,
            actual: usize::MAX,
            maximum: limits.max_carrier_overlap_tests,
        })?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::CarrierOverlapTests,
        overlap_tests,
        limits.max_carrier_overlap_tests,
    )?;
    for (expected_index, expected) in expected_creases.iter().enumerate() {
        for source in &source_edges {
            if segments_share_positive_collinear_interval(
                expected.start,
                expected.end,
                source.start,
                source.end,
            )? {
                return Err(
                    StackedFoldGeometryErrorV1::ExpectedCreaseOverlapsSourceEdge {
                        expected_index,
                        source_edge: source.id,
                    },
                );
            }
        }
    }
    for first in 0..expected_creases.len() {
        for second in (first + 1)..expected_creases.len() {
            if segments_share_positive_collinear_interval(
                expected_creases[first].start,
                expected_creases[first].end,
                expected_creases[second].start,
                expected_creases[second].end,
            )? {
                return Err(StackedFoldGeometryErrorV1::ExpectedCreasesOverlap { first, second });
            }
        }
    }

    let mut carriers = source_edges
        .iter()
        .map(|edge| GeometryCarrier {
            public: StackedFoldGeometryCarrierV1::SourceEdge(edge.id),
            start: edge.start,
            end: edge.end,
            kind: edge.kind,
        })
        .collect::<Vec<_>>();
    carriers.extend(
        expected_creases
            .iter()
            .enumerate()
            .map(|(index, crease)| GeometryCarrier {
                public: StackedFoldGeometryCarrierV1::ExpectedCrease(index),
                start: crease.start,
                end: crease.end,
                kind: crease.kind,
            }),
    );
    let carrier_tests = target_edges.len().checked_mul(carriers.len()).ok_or(
        StackedFoldGeometryErrorV1::ResourceLimit {
            resource: StackedFoldGeometryResourceV1::EdgeCarrierTests,
            actual: usize::MAX,
            maximum: limits.max_edge_carrier_tests,
        },
    )?;
    check_stacked_fold_limit(
        StackedFoldGeometryResourceV1::EdgeCarrierTests,
        carrier_tests,
        limits.max_edge_carrier_tests,
    )?;

    let source_edge_indices = source_edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.id, index))
        .collect::<HashMap<_, _>>();
    let mut source_identities_seen = vec![false; source_edges.len()];
    let mut assignments = vec![Vec::<GeometryEdgeRecord>::new(); carriers.len()];
    for target in &target_edges {
        if let Some(source_index) = source_edge_indices.get(&target.id).copied() {
            let source = source_edges[source_index];
            source_identities_seen[source_index] = true;
            if target.kind != source.kind {
                return Err(StackedFoldGeometryErrorV1::SourceEdgeKindChanged { edge: target.id });
            }
            if !segment_is_within_carrier(*target, source.start, source.end)? {
                return Err(StackedFoldGeometryErrorV1::SourceEdgeGeometryChanged {
                    edge: target.id,
                });
            }
        }

        let mut matching_carrier = None;
        for (carrier_index, carrier) in carriers.iter().enumerate() {
            if target.kind == carrier.kind
                && segment_is_within_carrier(*target, carrier.start, carrier.end)?
                && matching_carrier.replace(carrier_index).is_some()
            {
                return Err(StackedFoldGeometryErrorV1::TargetEdgeWithMultipleCarriers {
                    edge: target.id,
                });
            }
        }
        let carrier_index = matching_carrier
            .ok_or(StackedFoldGeometryErrorV1::TargetEdgeWithoutCarrier { edge: target.id })?;
        if let Some(source_index) = source_edge_indices.get(&target.id)
            && *source_index != carrier_index
        {
            return Err(StackedFoldGeometryErrorV1::SourceEdgeGeometryChanged { edge: target.id });
        }
        assignments[carrier_index].push(*target);
    }
    for (source_index, source) in source_edges.iter().enumerate() {
        if !source_identities_seen[source_index] {
            return Err(StackedFoldGeometryErrorV1::SourceEdgeIdentityMissing { edge: source.id });
        }
    }
    for (carrier, assigned_edges) in carriers.iter().zip(&assignments) {
        if !carrier_has_exact_coverage(*carrier, assigned_edges) {
            return Err(StackedFoldGeometryErrorV1::CarrierCoverageMismatch {
                carrier: carrier.public,
            });
        }
    }

    let source_subdivisions = source_edges
        .iter()
        .enumerate()
        .map(|(index, source)| SourceEdgeSubdivisionV1 {
            source_edge: source.id,
            target_edges: canonical_edge_ids(&assignments[index]),
        })
        .collect();
    let expected_subdivisions = expected_creases
        .iter()
        .enumerate()
        .map(|(expected_index, expected)| {
            let target_edges =
                canonical_edge_ids(&assignments[source_edges.len() + expected_index]);
            ExpectedCreaseSubdivisionV1 {
                start_x_bits: canonical_coordinate_bits(expected.start.x),
                start_y_bits: canonical_coordinate_bits(expected.start.y),
                end_x_bits: canonical_coordinate_bits(expected.end.x),
                end_y_bits: canonical_coordinate_bits(expected.end.y),
                kind: expected.kind,
                target_edges,
            }
        })
        .collect();

    Ok(StackedFoldGeometryProofV1 {
        lineage: input.face_lineage.clone(),
        source_edges: source_subdivisions,
        expected_creases: expected_subdivisions,
    })
}

fn check_stacked_fold_limit(
    resource: StackedFoldGeometryResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), StackedFoldGeometryErrorV1> {
    if actual > maximum {
        Err(StackedFoldGeometryErrorV1::ResourceLimit {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn canonical_expected_creases(
    expected: &[ExpectedStackedFoldCreaseV1],
) -> Result<Vec<CanonicalExpectedCrease>, StackedFoldGeometryErrorV1> {
    if expected.iter().any(|crease| {
        !crease.start.x.is_finite()
            || !crease.start.y.is_finite()
            || !crease.end.x.is_finite()
            || !crease.end.y.is_finite()
    }) {
        return Err(StackedFoldGeometryErrorV1::ExpectedCreaseNonFinite);
    }
    if expected
        .iter()
        .any(|crease| !matches!(crease.kind, EdgeKind::Mountain | EdgeKind::Valley))
    {
        return Err(StackedFoldGeometryErrorV1::ExpectedCreaseKindUnsupported);
    }
    if expected.iter().any(|crease| crease.start == crease.end) {
        return Err(StackedFoldGeometryErrorV1::ExpectedCreaseDegenerate);
    }

    let mut canonical = expected
        .iter()
        .map(|crease| {
            let first = canonical_point(crease.start);
            let second = canonical_point(crease.end);
            let (start, end) = if compare_points(first, second) == Ordering::Greater {
                (second, first)
            } else {
                (first, second)
            };
            CanonicalExpectedCrease {
                start,
                end,
                kind: crease.kind,
            }
        })
        .collect::<Vec<_>>();
    canonical.sort_unstable_by(compare_expected_creases);
    Ok(canonical)
}

fn geometry_vertex_records(
    pattern: &CreasePattern,
    topology: FaceLineageTopology,
) -> Result<Vec<GeometryVertexRecord>, StackedFoldGeometryErrorV1> {
    let mut vertices = pattern.vertices.iter().collect::<Vec<_>>();
    vertices.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    for pair in vertices.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(StackedFoldGeometryErrorV1::DuplicateVertex {
                topology,
                vertex: pair[0].id,
            });
        }
    }
    vertices
        .into_iter()
        .map(|vertex| {
            if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
                Err(StackedFoldGeometryErrorV1::NonFiniteVertex {
                    topology,
                    vertex: vertex.id,
                })
            } else {
                Ok(GeometryVertexRecord {
                    id: vertex.id,
                    position: vertex.position,
                })
            }
        })
        .collect()
}

fn vertex_position_map(vertices: &[GeometryVertexRecord]) -> HashMap<VertexId, Point2> {
    vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect()
}

fn geometry_edge_records(
    pattern: &CreasePattern,
    positions: &HashMap<VertexId, Point2>,
    topology: FaceLineageTopology,
) -> Result<Vec<GeometryEdgeRecord>, StackedFoldGeometryErrorV1> {
    let mut edges = pattern.edges.iter().collect::<Vec<_>>();
    edges.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    for pair in edges.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(StackedFoldGeometryErrorV1::DuplicateEdge {
                topology,
                edge: pair[0].id,
            });
        }
    }
    edges
        .into_iter()
        .map(|edge| {
            let start = positions.get(&edge.start).copied().ok_or(
                StackedFoldGeometryErrorV1::EdgeEndpointMissing {
                    topology,
                    edge: edge.id,
                },
            )?;
            let end = positions.get(&edge.end).copied().ok_or(
                StackedFoldGeometryErrorV1::EdgeEndpointMissing {
                    topology,
                    edge: edge.id,
                },
            )?;
            if start == end {
                return Err(StackedFoldGeometryErrorV1::DegenerateEdge {
                    topology,
                    edge: edge.id,
                });
            }
            Ok(GeometryEdgeRecord {
                id: edge.id,
                start_vertex: edge.start,
                end_vertex: edge.end,
                start,
                end,
                kind: edge.kind,
            })
        })
        .collect()
}

fn checked_overlap_test_count(source_edges: usize, expected_creases: usize) -> Option<usize> {
    let source_expected = source_edges.checked_mul(expected_creases)?;
    let expected_pairs = expected_creases.checked_mul(expected_creases.saturating_sub(1))? / 2;
    source_expected.checked_add(expected_pairs)
}

fn segments_share_positive_collinear_interval(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> Result<bool, GeometryError> {
    if exact_orientation(first_start, first_end, second_start)? != Orientation::Collinear
        || exact_orientation(first_start, first_end, second_end)? != Orientation::Collinear
    {
        return Ok(false);
    }
    let use_x = first_start.x != first_end.x;
    let first_low = carrier_scalar(first_start, use_x).min(carrier_scalar(first_end, use_x));
    let first_high = carrier_scalar(first_start, use_x).max(carrier_scalar(first_end, use_x));
    let second_low = carrier_scalar(second_start, use_x).min(carrier_scalar(second_end, use_x));
    let second_high = carrier_scalar(second_start, use_x).max(carrier_scalar(second_end, use_x));
    Ok(first_low.max(second_low) < first_high.min(second_high))
}

fn segment_is_within_carrier(
    edge: GeometryEdgeRecord,
    carrier_start: Point2,
    carrier_end: Point2,
) -> Result<bool, GeometryError> {
    Ok(
        point_segment_relation(edge.start, carrier_start, carrier_end)?
            != PointSegmentRelation::Outside
            && point_segment_relation(edge.end, carrier_start, carrier_end)?
                != PointSegmentRelation::Outside,
    )
}

fn carrier_has_exact_coverage(carrier: GeometryCarrier, edges: &[GeometryEdgeRecord]) -> bool {
    if edges.is_empty() {
        return false;
    }
    let use_x = carrier.start.x != carrier.end.x;
    let carrier_start = carrier_scalar(carrier.start, use_x);
    let carrier_end = carrier_scalar(carrier.end, use_x);
    let carrier_low = carrier_start.min(carrier_end);
    let carrier_high = carrier_start.max(carrier_end);
    let mut intervals = edges
        .iter()
        .map(|edge| {
            let start = carrier_scalar(edge.start, use_x);
            let end = carrier_scalar(edge.end, use_x);
            (start.min(end), start.max(end), edge.id)
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable_by(|left, right| {
        compare_finite_coordinates(left.0, right.0)
            .then_with(|| compare_finite_coordinates(left.1, right.1))
            .then_with(|| left.2.canonical_bytes().cmp(&right.2.canonical_bytes()))
    });
    let mut covered_until = carrier_low;
    for (low, high, _) in intervals {
        if low != covered_until || high <= low {
            return false;
        }
        covered_until = high;
    }
    covered_until == carrier_high
}

fn canonical_edge_ids(edges: &[GeometryEdgeRecord]) -> Vec<EdgeId> {
    let mut ids = edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(EdgeId::canonical_bytes);
    ids
}

fn canonical_point(point: Point2) -> Point2 {
    Point2::new(canonical_coordinate(point.x), canonical_coordinate(point.y))
}

fn canonical_coordinate(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn canonical_coordinate_bits(value: f64) -> u64 {
    canonical_coordinate(value).to_bits()
}

fn compare_points(left: Point2, right: Point2) -> Ordering {
    compare_finite_coordinates(left.x, right.x)
        .then_with(|| compare_finite_coordinates(left.y, right.y))
}

fn compare_expected_creases(
    left: &CanonicalExpectedCrease,
    right: &CanonicalExpectedCrease,
) -> Ordering {
    compare_points(left.start, right.start)
        .then_with(|| compare_points(left.end, right.end))
        .then_with(|| edge_kind_rank(left.kind).cmp(&edge_kind_rank(right.kind)))
}

fn compare_finite_coordinates(left: f64, right: f64) -> Ordering {
    debug_assert!(left.is_finite() && right.is_finite());
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

const fn edge_kind_rank(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Mountain => 0,
        EdgeKind::Valley => 1,
        EdgeKind::Auxiliary => 2,
        EdgeKind::Boundary => 3,
        EdgeKind::Cut => 4,
    }
}

const fn carrier_scalar(point: Point2, use_x: bool) -> f64 {
    if use_x { point.x } else { point.y }
}

fn point_bits_equal(first: Point2, second: Point2) -> bool {
    first.x.to_bits() == second.x.to_bits() && first.y.to_bits() == second.y.to_bits()
}

fn check_limit(
    resource: FaceLineageResource,
    actual: usize,
    maximum: usize,
) -> Result<(), FaceLineageError> {
    if actual > maximum {
        Err(FaceLineageError::ResourceLimit {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn simulation_snapshot(
    identity_namespace: ProjectId,
    source_revision: Revision,
    paper: &Paper,
    pattern: &CreasePattern,
    topology: FaceLineageTopology,
) -> Result<TopologySnapshot, FaceLineageError> {
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace,
        source_revision,
        paper,
        pattern,
    });
    let blocking_issue_count = report
        .issues
        .iter()
        .filter(|issue| issue.severity != TopologyIssueSeverity::Warning)
        .count();
    match (blocking_issue_count, report.snapshot) {
        (0, Some(snapshot)) => Ok(snapshot),
        _ => Err(FaceLineageError::TopologyNotSimulationReady {
            topology,
            issue_count: blocking_issue_count,
        }),
    }
}

fn check_topology_limits(
    topology: &TopologySnapshot,
    side: FaceLineageTopology,
    limits: FaceLineageLimits,
) -> Result<(), FaceLineageError> {
    let face_resource = match side {
        FaceLineageTopology::Source => FaceLineageResource::SourceFaces,
        FaceLineageTopology::Target => FaceLineageResource::TargetFaces,
    };
    let face_limit = match side {
        FaceLineageTopology::Source => limits.max_source_faces,
        FaceLineageTopology::Target => limits.max_target_faces,
    };
    check_limit(face_resource, topology.faces.len(), face_limit)?;

    let boundary_half_edges = topology
        .faces
        .iter()
        .try_fold(0_usize, |total, face| {
            total.checked_add(face.outer.half_edges.len())
        })
        .ok_or(FaceLineageError::ResourceLimit {
            resource: match side {
                FaceLineageTopology::Source => FaceLineageResource::SourceBoundaryHalfEdges,
                FaceLineageTopology::Target => FaceLineageResource::TargetBoundaryHalfEdges,
            },
            actual: usize::MAX,
            maximum: match side {
                FaceLineageTopology::Source => limits.max_source_boundary_half_edges,
                FaceLineageTopology::Target => limits.max_target_boundary_half_edges,
            },
        })?;
    let (resource, maximum) = match side {
        FaceLineageTopology::Source => (
            FaceLineageResource::SourceBoundaryHalfEdges,
            limits.max_source_boundary_half_edges,
        ),
        FaceLineageTopology::Target => (
            FaceLineageResource::TargetBoundaryHalfEdges,
            limits.max_target_boundary_half_edges,
        ),
    };
    check_limit(resource, boundary_half_edges, maximum)
}

fn canonical_registry(topology: &TopologySnapshot) -> Vec<LayerFace> {
    let mut registry = topology
        .faces
        .iter()
        .map(|face| LayerFace {
            face_id: face.id,
            face_key: face.key,
        })
        .collect::<Vec<_>>();
    registry.sort_unstable_by(compare_layer_faces);
    registry
}

fn compare_layer_faces(left: &LayerFace, right: &LayerFace) -> Ordering {
    left.face_key.cmp(&right.face_key).then_with(|| {
        left.face_id
            .canonical_bytes()
            .cmp(&right.face_id.canonical_bytes())
    })
}

fn polygon_records(
    pattern: &CreasePattern,
    topology: &TopologySnapshot,
) -> Result<Vec<PolygonRecord>, FaceLineageError> {
    let positions = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<VertexId, Point2>>();
    let mut records = topology
        .faces
        .iter()
        .map(|face| polygon_record(face, &positions))
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_unstable_by(|left, right| compare_layer_faces(&left.face, &right.face));
    Ok(records)
}

fn polygon_record(
    face: &Face,
    positions: &HashMap<VertexId, Point2>,
) -> Result<PolygonRecord, FaceLineageError> {
    let points = face
        .outer
        .half_edges
        .iter()
        .map(|half_edge| {
            positions
                .get(&half_edge.origin)
                .copied()
                .ok_or(FaceLineageError::ValidatedTopologyInvariantLost)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if points.len() < 3 {
        return Err(FaceLineageError::ValidatedTopologyInvariantLost);
    }
    let exact_double_area = exact_polygon_double_area(&points);
    if exact_double_area <= BigInt::from(0_u8) {
        return Err(FaceLineageError::ValidatedTopologyInvariantLost);
    }
    Ok(PolygonRecord {
        face: LayerFace {
            face_id: face.id,
            face_key: face.key,
        },
        points,
        exact_double_area,
    })
}

fn ensure_convex(source: &PolygonRecord) -> Result<(), FaceLineageError> {
    for index in 0..source.points.len() {
        let previous = source.points[(index + source.points.len() - 1) % source.points.len()];
        let current = source.points[index];
        let next = source.points[(index + 1) % source.points.len()];
        if exact_orientation(previous, current, next)? == Orientation::Clockwise {
            return Err(FaceLineageError::SourceFaceNotConvex {
                face: source.face.face_id,
            });
        }
    }
    Ok(())
}

fn polygon_is_within_convex_source(
    target: &[Point2],
    source: &[Point2],
) -> Result<bool, GeometryError> {
    for point in target {
        if point_polygon_relation(*point, source)? == PointPolygonRelation::Outside {
            return Ok(false);
        }
    }

    // The source is independently proven convex and the target topology is a
    // simple material face. Every segment between two accepted target
    // vertices, and therefore the entire target polygon, is inside the closed
    // convex source. Rechecking each midpoint would add no proof strength and
    // would repeatedly allocate an exact copy of the source polygon.
    Ok(true)
}

/// Returns the exact signed double area at the common `2^-2148` scale.
///
/// Every finite binary64 coordinate is an integer multiple of `2^-1074`.
/// Products therefore share this fixed scale, so equality remains exact
/// without an epsilon or an independently rounded `f64` area.
fn exact_polygon_double_area(points: &[Point2]) -> BigInt {
    let mut area = BigInt::from(0_u8);
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        area += exact_f64_at_minimum_scale(current.x) * exact_f64_at_minimum_scale(next.y);
        area -= exact_f64_at_minimum_scale(current.y) * exact_f64_at_minimum_scale(next.x);
    }
    area
}

fn exact_f64_at_minimum_scale(value: f64) -> BigInt {
    debug_assert!(value.is_finite());
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, shift) = if exponent == 0 {
        (fraction, 0)
    } else {
        ((1_u64 << 52) | fraction, exponent - 1)
    };
    let integer = BigInt::from(significand) << shift;
    if negative { -integer } else { integer }
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, EdgeId, EdgeKind, Vertex};
    use ori_foldability::{
        GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, analyze_global_flat_foldability,
    };
    use ori_topology::{analyze_faces, analyze_local_flat_foldability};

    use super::*;
    use crate::{EditorState, create_rectangular_sheet};

    struct Fixture {
        identity: ProjectId,
        source_pattern: CreasePattern,
        source_paper: Paper,
        source_layer_order: LayerOrderSnapshot,
        target_pattern: CreasePattern,
        target_paper: Paper,
    }

    impl Fixture {
        fn input(&self) -> FaceLineageInput<'_> {
            FaceLineageInput {
                identity_namespace: self.identity,
                source_revision: 7,
                source_paper: &self.source_paper,
                source_pattern: &self.source_pattern,
                source_layer_order: &self.source_layer_order,
                target_revision: 8,
                target_paper: &self.target_paper,
                target_pattern: &self.target_pattern,
            }
        }
    }

    fn fixture() -> Fixture {
        let identity = ProjectId::new();
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, source_paper) = sheet.into_parts();
        let source_layer_order = proven_layer_order(identity, 7, &source_pattern, &source_paper);

        let mut target_pattern = source_pattern.clone();
        target_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: source_paper.boundary_vertices[0],
            end: source_paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });

        Fixture {
            identity,
            source_pattern,
            source_paper: source_paper.clone(),
            source_layer_order,
            target_pattern,
            target_paper: source_paper,
        }
    }

    #[derive(Clone)]
    struct GeometryFixture {
        identity: ProjectId,
        source_revision: Revision,
        target_revision: Revision,
        source_pattern: CreasePattern,
        source_paper: Paper,
        source_layer_order: LayerOrderSnapshot,
        target_pattern: CreasePattern,
        target_paper: Paper,
        expected_creases: Vec<ExpectedStackedFoldCreaseV1>,
    }

    impl GeometryFixture {
        fn lineage_input(&self) -> FaceLineageInput<'_> {
            FaceLineageInput {
                identity_namespace: self.identity,
                source_revision: self.source_revision,
                source_paper: &self.source_paper,
                source_pattern: &self.source_pattern,
                source_layer_order: &self.source_layer_order,
                target_revision: self.target_revision,
                target_paper: &self.target_paper,
                target_pattern: &self.target_pattern,
            }
        }

        fn lineage(&self) -> FaceLineageV1 {
            prepare_face_lineage_v1(self.lineage_input(), FaceLineageLimits::default())
                .expect("prepare geometry fixture lineage")
        }

        fn geometry_input<'a>(
            &'a self,
            lineage: &'a FaceLineageV1,
        ) -> StackedFoldGeometryInputV1<'a> {
            StackedFoldGeometryInputV1 {
                identity_namespace: self.identity,
                source_revision: self.source_revision,
                source_paper: &self.source_paper,
                source_pattern: &self.source_pattern,
                target_revision: self.target_revision,
                target_paper: &self.target_paper,
                target_pattern: &self.target_pattern,
                face_lineage: lineage,
                expected_creases: &self.expected_creases,
            }
        }
    }

    fn simple_geometry_fixture() -> GeometryFixture {
        let fixture = fixture();
        let expected_creases = vec![ExpectedStackedFoldCreaseV1 {
            start: vertex_position(
                &fixture.source_pattern,
                fixture.source_paper.boundary_vertices[0],
            ),
            end: vertex_position(
                &fixture.source_pattern,
                fixture.source_paper.boundary_vertices[2],
            ),
            kind: EdgeKind::Mountain,
        }];
        GeometryFixture {
            identity: fixture.identity,
            source_revision: 7,
            target_revision: 8,
            source_pattern: fixture.source_pattern,
            source_paper: fixture.source_paper,
            source_layer_order: fixture.source_layer_order,
            target_pattern: fixture.target_pattern,
            target_paper: fixture.target_paper,
            expected_creases,
        }
    }

    #[test]
    fn topology_builder_creates_provable_cross_arrangement() {
        let identity = ProjectId::new();
        let source_revision = 31;
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, source_paper) = sheet.into_parts();
        let corners = source_paper
            .boundary_vertices
            .iter()
            .map(|id| vertex_position(&source_pattern, *id))
            .collect::<Vec<_>>();
        let expected = [
            ExpectedStackedFoldCreaseV1 {
                start: corners[0],
                end: corners[2],
                kind: EdgeKind::Mountain,
            },
            ExpectedStackedFoldCreaseV1 {
                start: corners[1],
                end: corners[3],
                kind: EdgeKind::Valley,
            },
        ];

        let candidate = build_stacked_fold_topology_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &expected,
            StackedFoldTopologyBuildLimitsV1::default(),
        )
        .expect("build crossing crease arrangement");
        assert_eq!(candidate.pattern.vertices.len(), 5);
        assert_eq!(candidate.pattern.edges.len(), 8);
        assert_eq!(
            candidate.paper.boundary_vertices,
            source_paper.boundary_vertices
        );
        let mut reversed_expected = expected;
        reversed_expected.reverse();
        for crease in &mut reversed_expected {
            std::mem::swap(&mut crease.start, &mut crease.end);
        }
        let repeated = build_stacked_fold_topology_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &reversed_expected,
            StackedFoldTopologyBuildLimitsV1::default(),
        )
        .expect("repeat with reversed caller order and direction");
        assert_eq!(candidate, repeated);
        let mut signed_zero_expected = expected;
        for crease in &mut signed_zero_expected {
            for coordinate in [
                &mut crease.start.x,
                &mut crease.start.y,
                &mut crease.end.x,
                &mut crease.end.y,
            ] {
                if *coordinate == 0.0 {
                    *coordinate = -0.0;
                }
            }
        }
        let signed_zero = build_stacked_fold_topology_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &signed_zero_expected,
            StackedFoldTopologyBuildLimitsV1::default(),
        )
        .expect("canonicalize signed zero");
        assert_eq!(candidate, signed_zero);

        let source_layer_order =
            proven_layer_order(identity, source_revision, &source_pattern, &source_paper);
        let lineage = prepare_face_lineage_v1(
            FaceLineageInput {
                identity_namespace: identity,
                source_revision,
                source_paper: &source_paper,
                source_pattern: &source_pattern,
                source_layer_order: &source_layer_order,
                target_revision: source_revision + 1,
                target_paper: &candidate.paper,
                target_pattern: &candidate.pattern,
            },
            FaceLineageLimits::default(),
        )
        .expect("prove generated face lineage");
        let proof = prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                identity_namespace: identity,
                source_revision,
                source_paper: &source_paper,
                source_pattern: &source_pattern,
                target_revision: source_revision + 1,
                target_paper: &candidate.paper,
                target_pattern: &candidate.pattern,
                face_lineage: &lineage,
                expected_creases: &expected,
            },
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prove generated geometry");
        assert_eq!(proof.expected_creases().len(), 2);
        assert_eq!(
            proof
                .expected_creases()
                .iter()
                .map(|crease| crease.target_edges().len())
                .collect::<Vec<_>>(),
            vec![2, 2]
        );
        let prepared = prepare_stacked_fold_geometry_candidate_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &source_layer_order,
            &expected,
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("build and prove one owning package");
        assert_eq!(prepared.candidate(), &candidate);
        assert_eq!(prepared.proof(), &proof);
        assert!(matches!(
            prepare_stacked_fold_target_model_v1(prepared, TreeKinematicsLimits::default()),
            Err(
                PrepareStackedFoldTargetModelErrorV1::CyclicTargetUnsupported {
                    closure_hinge_count: 1
                }
            )
        ));
    }

    #[test]
    fn topology_builder_splits_paper_boundary_at_crease_endpoint() {
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, source_paper) = sheet.into_parts();
        let corner = vertex_position(&source_pattern, source_paper.boundary_vertices[0]);
        let opposite = vertex_position(&source_pattern, source_paper.boundary_vertices[2]);
        let expected = [ExpectedStackedFoldCreaseV1 {
            start: Point2::new((corner.x + opposite.x) * 0.5, corner.y),
            end: Point2::new((corner.x + opposite.x) * 0.5, opposite.y),
            kind: EdgeKind::Mountain,
        }];

        let candidate = build_stacked_fold_topology_v1(
            ProjectId::new(),
            0,
            &source_pattern,
            &source_paper,
            &expected,
            StackedFoldTopologyBuildLimitsV1::default(),
        )
        .expect("build boundary-to-boundary crease");
        assert_eq!(candidate.pattern.vertices.len(), 6);
        assert_eq!(candidate.pattern.edges.len(), 7);
        assert_eq!(candidate.paper.boundary_vertices.len(), 6);
    }

    #[test]
    fn target_graph_audit_transports_cycle_constraints_without_authority() {
        let identity = ProjectId::new();
        let source_revision = 41;
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, source_paper) = sheet.into_parts();
        let corners = source_paper
            .boundary_vertices
            .iter()
            .map(|id| vertex_position(&source_pattern, *id))
            .collect::<Vec<_>>();
        let expected = [
            ExpectedStackedFoldCreaseV1 {
                start: corners[0],
                end: corners[2],
                kind: EdgeKind::Mountain,
            },
            ExpectedStackedFoldCreaseV1 {
                start: corners[1],
                end: corners[3],
                kind: EdgeKind::Valley,
            },
        ];
        let source_layer_order =
            proven_layer_order(identity, source_revision, &source_pattern, &source_paper);
        let prepare_geometry = || {
            prepare_stacked_fold_geometry_candidate_v1(
                identity,
                source_revision,
                &source_pattern,
                &source_paper,
                &source_layer_order,
                &expected,
                StackedFoldTopologyBuildLimitsV1::default(),
                FaceLineageLimits::default(),
                StackedFoldGeometryLimitsV1::default(),
            )
            .expect("prepare cyclic geometry")
        };

        let package = prepare_stacked_fold_target_graph_audit_v1(
            prepare_geometry(),
            TreeKinematicsLimits::default(),
        )
        .expect("retain cycle audit");
        assert_eq!(
            package.model_id(),
            STACKED_FOLD_TARGET_GRAPH_AUDIT_MODEL_ID_V1
        );
        assert_eq!(package.audit().faces().len(), 4);
        assert_eq!(package.audit().spanning_hinges().len(), 3);
        assert_eq!(package.audit().closure_hinges().len(), 1);
        assert_eq!(package.hinge_geometry().face_ids().len(), 4);
        assert_eq!(package.hinge_geometry().hinges().len(), 4);
        assert!(package.requires_closure_certificate());
        assert!(!package.authorizes_pose());
        assert!(!package.authorizes_apply_stacked_fold());
        assert_eq!(package.geometry().proof().expected_creases().len(), 2);
        let source_topology = simulation_snapshot(
            identity,
            source_revision,
            &source_paper,
            &source_pattern,
            FaceLineageTopology::Source,
        )
        .expect("source topology");
        let source_model = MaterialTreeKinematicsModel::prepare(
            &source_pattern,
            &source_paper,
            &source_topology,
            TreeKinematicsLimits::default(),
        )
        .expect("source model");
        let source_pose = source_model
            .solve(
                None,
                &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
            )
            .expect("source pose");
        let initial =
            prepare_stacked_fold_initial_graph_pose_v1(package, &source_model, &source_pose)
                .expect("cycle initial embedding closes");
        assert_eq!(
            initial.pose().closure_certificate().checked_hinges().len(),
            4
        );
        assert_eq!(initial.pose().transforms().len(), 4);
        assert!(matches!(
            prepare_stacked_fold_requested_graph_pose_v1(initial, 90.0),
            Err(PrepareStackedFoldRequestedPoseErrorV1::Kinematics(
                KinematicsError::UnsupportedTopology
            ))
        ));

        let limited = TreeKinematicsLimits {
            max_faces: 3,
            ..TreeKinematicsLimits::default()
        };
        assert!(matches!(
            prepare_stacked_fold_target_graph_audit_v1(prepare_geometry(), limited),
            Err(PrepareStackedFoldTargetGraphAuditErrorV1::ResourceLimit)
        ));
    }

    #[test]
    fn prepared_target_is_admitted_by_native_tree_kinematics() {
        let fixture = simple_geometry_fixture();
        let geometry = prepare_stacked_fold_geometry_candidate_v1(
            fixture.identity,
            fixture.source_revision,
            &fixture.source_pattern,
            &fixture.source_paper,
            &fixture.source_layer_order,
            &fixture.expected_creases,
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prepare geometry");
        let audited =
            prepare_stacked_fold_target_graph_audit_v1(geometry, TreeKinematicsLimits::default())
                .expect("audit tree target");
        assert!(!audited.requires_closure_certificate());
        assert_eq!(audited.audit().closure_hinges(), &[]);
        let geometry = prepare_stacked_fold_geometry_candidate_v1(
            fixture.identity,
            fixture.source_revision,
            &fixture.source_pattern,
            &fixture.source_paper,
            &fixture.source_layer_order,
            &fixture.expected_creases,
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prepare geometry after audit");
        let target =
            prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
                .expect("prepare target material tree");
        assert_eq!(target.model().face_ids().len(), 2);
        assert_eq!(target.model().hinges().len(), 1);
        assert_eq!(target.geometry().proof().expected_creases().len(), 1);
        let source_topology = simulation_snapshot(
            fixture.identity,
            fixture.source_revision,
            &fixture.source_paper,
            &fixture.source_pattern,
            FaceLineageTopology::Source,
        )
        .expect("source topology");
        let source_model = MaterialTreeKinematicsModel::prepare(
            &fixture.source_pattern,
            &fixture.source_paper,
            &source_topology,
            TreeKinematicsLimits::default(),
        )
        .expect("source model");
        let source_pose = source_model
            .solve(
                None,
                &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
            )
            .expect("source pose");
        let initial = prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
            .expect("lift source pose");
        assert!(initial.target().model().owns_pose(initial.pose()));
        assert_eq!(initial.pose().hinge_angles().len(), 1);
        assert_eq!(initial.pose().hinge_angles()[0].angle_degrees(), 0.0);
        let requested =
            prepare_stacked_fold_requested_pose_v1(initial, 90.0).expect("solve requested pose");
        assert!(
            requested
                .initial()
                .target()
                .model()
                .owns_pose(requested.pose())
        );
        assert_eq!(requested.requested_angle_degrees(), 90.0);
        assert_eq!(requested.pose().hinge_angles()[0].angle_degrees(), 90.0);
        let non_flat_order = prepare_stacked_fold_non_flat_layer_order_v1(
            &requested,
            &fixture.source_layer_order,
            1,
        )
        .expect("pairwise non-coincident target supports");
        assert_eq!(
            non_flat_order.model_id(),
            STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1
        );
        assert_eq!(non_flat_order.material_faces().len(), 2);
        assert_eq!(non_flat_order.tested_face_pairs(), 1);
        assert_eq!(non_flat_order.overlap_cell_count(), 0);
        assert_eq!(non_flat_order.face_pair_order_count(), 0);
        assert!(!non_flat_order.authorizes_apply_stacked_fold());

        let rebuild = || {
            let geometry = prepare_stacked_fold_geometry_candidate_v1(
                fixture.identity,
                fixture.source_revision,
                &fixture.source_pattern,
                &fixture.source_paper,
                &fixture.source_layer_order,
                &fixture.expected_creases,
                StackedFoldTopologyBuildLimitsV1::default(),
                FaceLineageLimits::default(),
                StackedFoldGeometryLimitsV1::default(),
            )
            .expect("rebuild geometry");
            let target =
                prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
                    .expect("rebuild target");
            prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
                .expect("rebuild initial pose")
        };
        for invalid in [0.0, -0.0, -1.0, 180.000_000_1, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                prepare_stacked_fold_requested_pose_v1(rebuild(), invalid),
                Err(PrepareStackedFoldRequestedPoseErrorV1::InvalidRequestedAngle)
            ));
        }
        let flat =
            prepare_stacked_fold_requested_pose_v1(rebuild(), 180.0).expect("solve flat endpoint");
        assert_eq!(
            prepare_stacked_fold_non_flat_layer_order_v1(
                &flat,
                &fixture.source_layer_order,
                usize::MAX,
            ),
            Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::NotNonFlatEndpoint)
        );
        let bounded = prepare_stacked_fold_requested_pose_v1(rebuild(), 90.0)
            .expect("solve bounded endpoint");
        assert_eq!(
            prepare_stacked_fold_non_flat_layer_order_v1(&bounded, &fixture.source_layer_order, 0,),
            Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::ResourceLimit)
        );
        let authenticated = prepare_stacked_fold_requested_pose_v1(rebuild(), 90.0)
            .expect("solve authenticated endpoint");
        let mut stale_layer_order = fixture.source_layer_order.clone();
        stale_layer_order.provenance.source.source_revision += 1;
        assert_eq!(
            prepare_stacked_fold_non_flat_layer_order_v1(
                &authenticated,
                &stale_layer_order,
                usize::MAX,
            ),
            Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::SourceLayerOrderMismatch)
        );
    }

    #[test]
    fn topology_builder_rejects_overlapping_carriers_and_exact_limits() {
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, source_paper) = sheet.into_parts();
        let start = vertex_position(&source_pattern, source_paper.boundary_vertices[0]);
        let end = vertex_position(&source_pattern, source_paper.boundary_vertices[1]);
        let expected = [ExpectedStackedFoldCreaseV1 {
            start,
            end,
            kind: EdgeKind::Mountain,
        }];
        let carriers = source_pattern.edges.len() + expected.len();
        let pair_tests = carriers * (carriers - 1) / 2;
        let inclusive = StackedFoldTopologyBuildLimitsV1 {
            max_carriers: carriers,
            max_pair_tests: pair_tests,
            ..StackedFoldTopologyBuildLimitsV1::default()
        };
        assert!(matches!(
            build_stacked_fold_topology_v1(
                ProjectId::new(),
                0,
                &source_pattern,
                &source_paper,
                &expected,
                inclusive
            ),
            Err(StackedFoldTopologyBuildErrorV1::CarrierOverlap { .. })
        ));
        assert_eq!(
            build_stacked_fold_topology_v1(
                ProjectId::new(),
                0,
                &source_pattern,
                &source_paper,
                &[],
                StackedFoldTopologyBuildLimitsV1 {
                    max_carriers: source_pattern.edges.len() - 1,
                    ..StackedFoldTopologyBuildLimitsV1::default()
                }
            ),
            Err(StackedFoldTopologyBuildErrorV1::ResourceLimit {
                resource: StackedFoldTopologyBuildResourceV1::Carriers,
                actual: source_pattern.edges.len(),
                maximum: source_pattern.edges.len() - 1,
            })
        );
        let mut missing_boundary_vertex = source_paper.clone();
        let missing = VertexId::new();
        missing_boundary_vertex.boundary_vertices[0] = missing;
        assert_eq!(
            build_stacked_fold_topology_v1(
                ProjectId::new(),
                0,
                &source_pattern,
                &missing_boundary_vertex,
                &[],
                StackedFoldTopologyBuildLimitsV1::default(),
            ),
            Err(StackedFoldTopologyBuildErrorV1::PaperBoundaryVertexMissing { vertex: missing })
        );
    }

    fn subdivided_cross_geometry_fixture() -> GeometryFixture {
        let identity = ProjectId::new();
        let source_revision = 12;
        let target_revision = 13;
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (mut source_pattern, paper) = sheet.into_parts();
        let source_hinge = EdgeId::new();
        source_pattern.edges.push(Edge {
            id: source_hinge,
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let source_layer_order =
            proven_layer_order(identity, source_revision, &source_pattern, &paper);

        let mut target_pattern = source_pattern.clone();
        let center = VertexId::new();
        target_pattern.vertices.push(Vertex {
            id: center,
            position: Point2::new(200.0, 200.0),
        });
        target_pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == source_hinge)
            .expect("source hinge")
            .end = center;
        target_pattern.edges.extend([
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[2],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: paper.boundary_vertices[1],
                end: center,
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[3],
                kind: EdgeKind::Valley,
            },
        ]);
        let expected_creases = vec![ExpectedStackedFoldCreaseV1 {
            start: vertex_position(&source_pattern, paper.boundary_vertices[1]),
            end: vertex_position(&source_pattern, paper.boundary_vertices[3]),
            kind: EdgeKind::Valley,
        }];

        GeometryFixture {
            identity,
            source_revision,
            target_revision,
            source_pattern,
            source_paper: paper.clone(),
            source_layer_order,
            target_pattern,
            target_paper: paper,
            expected_creases,
        }
    }

    fn two_new_crossing_creases_fixture() -> GeometryFixture {
        let identity = ProjectId::new();
        let source_revision = 20;
        let target_revision = 21;
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, paper) = sheet.into_parts();
        let source_layer_order =
            proven_layer_order(identity, source_revision, &source_pattern, &paper);
        let center = VertexId::new();
        let mut target_pattern = source_pattern.clone();
        target_pattern.vertices.push(Vertex {
            id: center,
            position: Point2::new(200.0, 200.0),
        });
        target_pattern.edges.extend([
            Edge {
                id: EdgeId::new(),
                start: paper.boundary_vertices[0],
                end: center,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[2],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: paper.boundary_vertices[1],
                end: center,
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[3],
                kind: EdgeKind::Valley,
            },
        ]);
        let expected_creases = vec![
            ExpectedStackedFoldCreaseV1 {
                start: vertex_position(&source_pattern, paper.boundary_vertices[0]),
                end: vertex_position(&source_pattern, paper.boundary_vertices[2]),
                kind: EdgeKind::Mountain,
            },
            ExpectedStackedFoldCreaseV1 {
                start: vertex_position(&source_pattern, paper.boundary_vertices[1]),
                end: vertex_position(&source_pattern, paper.boundary_vertices[3]),
                kind: EdgeKind::Valley,
            },
        ];

        GeometryFixture {
            identity,
            source_revision,
            target_revision,
            source_pattern,
            source_paper: paper.clone(),
            source_layer_order,
            target_pattern,
            target_paper: paper,
            expected_creases,
        }
    }

    fn vertex_position(pattern: &CreasePattern, id: VertexId) -> Point2 {
        pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == id)
            .expect("fixture vertex")
            .position
    }

    fn proven_layer_order(
        identity: ProjectId,
        revision: Revision,
        pattern: &CreasePattern,
        paper: &Paper,
    ) -> LayerOrderSnapshot {
        let source_topology = analyze_faces(FaceExtractionInput {
            identity_namespace: identity,
            source_revision: revision,
            paper,
            pattern,
        })
        .snapshot
        .expect("source topology");
        let local = analyze_local_flat_foldability(paper, pattern);
        let report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                identity,
                paper,
                pattern,
                &source_topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("global analysis");
        report.layer_order().expect("possible layer order").clone()
    }

    #[test]
    fn proves_one_source_face_split_into_two_canonical_descendants() {
        let fixture = fixture();
        let lineage = prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default())
            .expect("prove lineage");

        assert_eq!(lineage.identity_namespace(), fixture.identity);
        assert_eq!(lineage.source_revision(), 7);
        assert_eq!(lineage.target_revision(), 8);
        assert_eq!(
            lineage.source_fingerprint(),
            fold_model_fingerprint_v1(&fixture.source_pattern, &fixture.source_paper)
        );
        assert_eq!(
            lineage.target_fingerprint(),
            fold_model_fingerprint_v1(&fixture.target_pattern, &fixture.target_paper)
        );
        assert_eq!(lineage.records().len(), 1);
        assert_eq!(lineage.records()[0].descendants().len(), 2);
        assert!(
            lineage.records()[0]
                .descendants()
                .windows(2)
                .all(|faces| compare_layer_faces(&faces[0], &faces[1]) == Ordering::Less)
        );
    }

    #[test]
    fn lineage_is_invariant_to_storage_order_and_new_edge_direction() {
        let fixture = fixture();
        let expected = prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default())
            .expect("baseline lineage");

        let mut reordered = fixture.target_pattern.clone();
        reordered.vertices.reverse();
        reordered.edges.reverse();
        let mut reordered_paper = fixture.target_paper.clone();
        reordered_paper.boundary_vertices.rotate_left(1);
        reordered_paper.boundary_vertices.reverse();
        let fold = reordered
            .edges
            .iter_mut()
            .find(|edge| matches!(edge.kind, EdgeKind::Mountain))
            .expect("new fold");
        std::mem::swap(&mut fold.start, &mut fold.end);
        let input = FaceLineageInput {
            target_pattern: &reordered,
            target_paper: &reordered_paper,
            ..fixture.input()
        };

        assert_eq!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Ok(expected)
        );
    }

    #[test]
    fn proves_two_source_faces_each_split_after_shared_hinge_subdivision() {
        let identity = ProjectId::new();
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (mut source_pattern, paper) = sheet.into_parts();
        let source_hinge = EdgeId::new();
        source_pattern.edges.push(Edge {
            id: source_hinge,
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let source_layer_order = proven_layer_order(identity, 12, &source_pattern, &paper);

        let mut target_pattern = source_pattern.clone();
        let center = VertexId::new();
        target_pattern.vertices.push(Vertex {
            id: center,
            position: Point2::new(200.0, 200.0),
        });
        target_pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == source_hinge)
            .expect("source hinge")
            .end = center;
        target_pattern.edges.extend([
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[2],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: paper.boundary_vertices[1],
                end: center,
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[3],
                kind: EdgeKind::Valley,
            },
        ]);

        let lineage = prepare_face_lineage_v1(
            FaceLineageInput {
                identity_namespace: identity,
                source_revision: 12,
                source_paper: &paper,
                source_pattern: &source_pattern,
                source_layer_order: &source_layer_order,
                target_revision: 13,
                target_paper: &paper,
                target_pattern: &target_pattern,
            },
            FaceLineageLimits::default(),
        )
        .expect("prove two-face lineage");

        assert_eq!(lineage.records().len(), 2);
        assert!(
            lineage
                .records()
                .iter()
                .all(|record| record.descendants().len() == 2)
        );
        let descendant_ids = lineage
            .records()
            .iter()
            .flat_map(FaceLineageRecord::descendants)
            .map(|face| face.face_id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(descendant_ids.len(), 4);
    }

    #[test]
    fn geometry_proof_accepts_only_the_explicit_mountain_delta() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let proof = prepare_stacked_fold_geometry_v1(
            fixture.geometry_input(&lineage),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prove simple stacked-fold geometry");

        assert_eq!(proof.lineage(), &lineage);
        assert_eq!(
            proof.source_edges().len(),
            fixture.source_pattern.edges.len()
        );
        assert!(
            proof
                .source_edges()
                .iter()
                .all(|subdivision| subdivision.target_edges().len() == 1)
        );
        assert_eq!(proof.expected_creases().len(), 1);
        assert_eq!(proof.expected_creases()[0].start(), Point2::new(0.0, 0.0));
        assert_eq!(proof.expected_creases()[0].end(), Point2::new(400.0, 400.0));
        assert_eq!(proof.expected_creases()[0].kind(), EdgeKind::Mountain);
        assert_eq!(proof.expected_creases()[0].target_edges().len(), 1);
    }

    #[test]
    fn geometry_proof_accepts_exact_source_and_expected_subdivisions() {
        let fixture = subdivided_cross_geometry_fixture();
        let source_hinge = fixture
            .source_pattern
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::Mountain)
            .expect("source hinge")
            .id;
        let lineage = fixture.lineage();
        let proof = prepare_stacked_fold_geometry_v1(
            fixture.geometry_input(&lineage),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prove subdivided source and expected crease");

        let source_subdivision = proof
            .source_edges()
            .iter()
            .find(|subdivision| subdivision.source_edge() == source_hinge)
            .expect("source hinge subdivision");
        assert_eq!(source_subdivision.target_edges().len(), 2);
        assert_eq!(proof.expected_creases().len(), 1);
        assert_eq!(proof.expected_creases()[0].kind(), EdgeKind::Valley);
        assert_eq!(proof.expected_creases()[0].target_edges().len(), 2);
    }

    #[test]
    fn geometry_proof_is_invariant_to_storage_expected_order_and_edge_direction() {
        let fixture = two_new_crossing_creases_fixture();
        let lineage = fixture.lineage();
        let expected = prepare_stacked_fold_geometry_v1(
            fixture.geometry_input(&lineage),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("baseline geometry proof");

        let mut source_pattern = fixture.source_pattern.clone();
        source_pattern.vertices.reverse();
        source_pattern.edges.reverse();
        for edge in &mut source_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut target_pattern = fixture.target_pattern.clone();
        target_pattern.vertices.reverse();
        target_pattern.edges.reverse();
        for edge in &mut target_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut source_paper = fixture.source_paper.clone();
        source_paper.boundary_vertices.rotate_left(1);
        source_paper.boundary_vertices.reverse();
        let mut target_paper = fixture.target_paper.clone();
        target_paper.boundary_vertices.rotate_right(1);
        target_paper.boundary_vertices.reverse();
        let mut expected_creases = fixture.expected_creases.clone();
        expected_creases.reverse();
        for crease in &mut expected_creases {
            std::mem::swap(&mut crease.start, &mut crease.end);
        }
        let reordered_input = StackedFoldGeometryInputV1 {
            identity_namespace: fixture.identity,
            source_revision: fixture.source_revision,
            source_paper: &source_paper,
            source_pattern: &source_pattern,
            target_revision: fixture.target_revision,
            target_paper: &target_paper,
            target_pattern: &target_pattern,
            face_lineage: &lineage,
            expected_creases: &expected_creases,
        };

        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                reordered_input,
                StackedFoldGeometryLimitsV1::default()
            ),
            Ok(expected)
        );
    }

    #[test]
    fn geometry_proof_rebinds_identity_revisions_and_both_fingerprints() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let input = fixture.geometry_input(&lineage);

        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    identity_namespace: ProjectId::new(),
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::LineageIdentityMismatch)
        );
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    target_revision: fixture.target_revision + 1,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::LineageRevisionMismatch)
        );

        let mut changed_source = fixture.source_pattern.clone();
        changed_source.vertices[0].position.x = 1.0;
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    source_pattern: &changed_source,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::LineageSourceFingerprintMismatch)
        );

        let mut changed_target = fixture.target_pattern.clone();
        changed_target.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(123.0, 234.0),
        });
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    target_pattern: &changed_target,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::LineageTargetFingerprintMismatch)
        );
    }

    #[test]
    fn expected_crease_input_is_nonempty_finite_nondegenerate_and_mv_only() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let input = fixture.geometry_input(&lineage);

        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &[],
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::ExpectedCreaseSetEmpty)
        );

        let non_finite = [ExpectedStackedFoldCreaseV1 {
            start: Point2::new(f64::NAN, 0.0),
            ..fixture.expected_creases[0]
        }];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &non_finite,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::ExpectedCreaseNonFinite)
        );

        let degenerate = [ExpectedStackedFoldCreaseV1 {
            end: fixture.expected_creases[0].start,
            ..fixture.expected_creases[0]
        }];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &degenerate,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::ExpectedCreaseDegenerate)
        );

        for kind in [EdgeKind::Auxiliary, EdgeKind::Boundary, EdgeKind::Cut] {
            let unsupported = [ExpectedStackedFoldCreaseV1 {
                kind,
                ..fixture.expected_creases[0]
            }];
            assert_eq!(
                prepare_stacked_fold_geometry_v1(
                    StackedFoldGeometryInputV1 {
                        expected_creases: &unsupported,
                        ..input
                    },
                    StackedFoldGeometryLimitsV1::default(),
                ),
                Err(StackedFoldGeometryErrorV1::ExpectedCreaseKindUnsupported)
            );
        }
    }

    #[test]
    fn coincident_expected_and_source_carriers_are_rejected() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let input = fixture.geometry_input(&lineage);
        let duplicate = [
            fixture.expected_creases[0],
            ExpectedStackedFoldCreaseV1 {
                start: fixture.expected_creases[0].end,
                end: fixture.expected_creases[0].start,
                kind: EdgeKind::Mountain,
            },
        ];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &duplicate,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::ExpectedCreasesOverlap {
                first: 0,
                second: 1,
            })
        );

        let boundary_start = fixture.source_paper.boundary_vertices[0];
        let boundary_end = fixture.source_paper.boundary_vertices[1];
        let boundary_edge = fixture
            .source_pattern
            .edges
            .iter()
            .find(|edge| {
                (edge.start == boundary_start && edge.end == boundary_end)
                    || (edge.start == boundary_end && edge.end == boundary_start)
            })
            .expect("source boundary edge")
            .id;
        let overlaps_source = [ExpectedStackedFoldCreaseV1 {
            start: vertex_position(&fixture.source_pattern, boundary_start),
            end: vertex_position(&fixture.source_pattern, boundary_end),
            kind: EdgeKind::Mountain,
        }];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &overlaps_source,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(
                StackedFoldGeometryErrorV1::ExpectedCreaseOverlapsSourceEdge {
                    expected_index: 0,
                    source_edge: boundary_edge,
                }
            )
        );
    }

    #[test]
    fn wrong_missing_and_extra_expected_creases_are_rejected_exactly() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let input = fixture.geometry_input(&lineage);
        let target_fold = fixture
            .target_pattern
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::Mountain)
            .expect("target fold")
            .id;
        let wrong_kind = [ExpectedStackedFoldCreaseV1 {
            kind: EdgeKind::Valley,
            ..fixture.expected_creases[0]
        }];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &wrong_kind,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::TargetEdgeWithoutCarrier { edge: target_fold })
        );

        let missing_target = [
            fixture.expected_creases[0],
            ExpectedStackedFoldCreaseV1 {
                start: Point2::new(0.0, 400.0),
                end: Point2::new(400.0, 0.0),
                kind: EdgeKind::Valley,
            },
        ];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &missing_target,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::CarrierCoverageMismatch {
                carrier: StackedFoldGeometryCarrierV1::ExpectedCrease(1),
            })
        );

        let two_creases = two_new_crossing_creases_fixture();
        let two_lineage = two_creases.lineage();
        let only_mountain = [two_creases.expected_creases[0]];
        assert!(matches!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &only_mountain,
                    ..two_creases.geometry_input(&two_lineage)
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::TargetEdgeWithoutCarrier { edge })
                if two_creases
                    .target_pattern
                    .edges
                    .iter()
                    .any(|candidate| candidate.id == edge && candidate.kind == EdgeKind::Valley)
        ));
    }

    #[test]
    fn source_edge_identity_and_kind_cannot_change_during_subdivision() {
        let fixture = subdivided_cross_geometry_fixture();
        let source_hinge = fixture
            .source_pattern
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::Mountain)
            .expect("source hinge")
            .id;

        let mut changed_kind = fixture.clone();
        changed_kind
            .target_pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == source_hinge)
            .expect("target source edge")
            .kind = EdgeKind::Valley;
        let changed_kind_lineage = changed_kind.lineage();
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                changed_kind.geometry_input(&changed_kind_lineage),
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::SourceEdgeKindChanged { edge: source_hinge })
        );

        let mut changed_identity = fixture;
        changed_identity
            .target_pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == source_hinge)
            .expect("target source edge")
            .id = EdgeId::new();
        let changed_identity_lineage = changed_identity.lineage();
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                changed_identity.geometry_input(&changed_identity_lineage),
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::SourceEdgeIdentityMissing { edge: source_hinge })
        );
    }

    #[test]
    fn new_unrelated_target_vertices_are_rejected_even_with_valid_lineage() {
        let mut fixture = simple_geometry_fixture();
        let isolated = VertexId::new();
        fixture.target_pattern.vertices.push(Vertex {
            id: isolated,
            position: Point2::new(123.0, 234.0),
        });
        let lineage = fixture.lineage();

        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                fixture.geometry_input(&lineage),
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::NewTargetVertexIsolated { vertex: isolated })
        );
    }

    #[test]
    fn moving_an_existing_unrelated_vertex_is_rejected_even_with_valid_lineage() {
        let identity = ProjectId::new();
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (mut source_pattern, paper) = sheet.into_parts();
        let isolated = VertexId::new();
        source_pattern.vertices.push(Vertex {
            id: isolated,
            position: Point2::new(500.0, 500.0),
        });
        let source_layer_order = proven_layer_order(identity, 30, &source_pattern, &paper);
        let mut target_pattern = source_pattern.clone();
        target_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == isolated)
            .expect("isolated source vertex")
            .position = Point2::new(501.0, 500.0);
        target_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let expected_creases = [ExpectedStackedFoldCreaseV1 {
            start: vertex_position(&source_pattern, paper.boundary_vertices[0]),
            end: vertex_position(&source_pattern, paper.boundary_vertices[2]),
            kind: EdgeKind::Mountain,
        }];
        let lineage = prepare_face_lineage_v1(
            FaceLineageInput {
                identity_namespace: identity,
                source_revision: 30,
                source_paper: &paper,
                source_pattern: &source_pattern,
                source_layer_order: &source_layer_order,
                target_revision: 31,
                target_paper: &paper,
                target_pattern: &target_pattern,
            },
            FaceLineageLimits::default(),
        )
        .expect("lineage intentionally ignores isolated draft movement");

        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    identity_namespace: identity,
                    source_revision: 30,
                    source_paper: &paper,
                    source_pattern: &source_pattern,
                    target_revision: 31,
                    target_paper: &paper,
                    target_pattern: &target_pattern,
                    face_lineage: &lineage,
                    expected_creases: &expected_creases,
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::SourceVertexMoved { vertex: isolated })
        );
    }

    #[test]
    fn geometry_resource_limits_admit_equality_and_reject_one_less() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let exact = StackedFoldGeometryLimitsV1 {
            max_source_vertices: 4,
            max_source_edges: 4,
            max_source_paper_boundary_vertices: 4,
            max_target_vertices: 4,
            max_target_edges: 5,
            max_target_paper_boundary_vertices: 4,
            max_expected_creases: 1,
            max_lineage_records: 1,
            max_lineage_descendants: 2,
            max_edge_carrier_tests: 25,
            max_carrier_overlap_tests: 4,
        };
        prepare_stacked_fold_geometry_v1(fixture.geometry_input(&lineage), exact)
            .expect("all documented limits admit equality");

        for (limits, resource, actual, maximum) in [
            (
                StackedFoldGeometryLimitsV1 {
                    max_edge_carrier_tests: 24,
                    ..exact
                },
                StackedFoldGeometryResourceV1::EdgeCarrierTests,
                25,
                24,
            ),
            (
                StackedFoldGeometryLimitsV1 {
                    max_carrier_overlap_tests: 3,
                    ..exact
                },
                StackedFoldGeometryResourceV1::CarrierOverlapTests,
                4,
                3,
            ),
            (
                StackedFoldGeometryLimitsV1 {
                    max_lineage_descendants: 1,
                    ..exact
                },
                StackedFoldGeometryResourceV1::LineageDescendants,
                2,
                1,
            ),
            (
                StackedFoldGeometryLimitsV1 {
                    max_expected_creases: 0,
                    ..exact
                },
                StackedFoldGeometryResourceV1::ExpectedCreases,
                1,
                0,
            ),
        ] {
            assert_eq!(
                prepare_stacked_fold_geometry_v1(fixture.geometry_input(&lineage), limits),
                Err(StackedFoldGeometryErrorV1::ResourceLimit {
                    resource,
                    actual,
                    maximum,
                })
            );
        }
    }

    #[test]
    fn geometry_failure_is_pure_and_leaves_editor_state_unchanged() {
        let fixture = simple_geometry_fixture();
        let lineage = fixture.lineage();
        let editor =
            EditorState::with_paper(fixture.source_pattern.clone(), fixture.source_paper.clone());
        let before_pattern = editor.pattern().clone();
        let before_paper = editor.paper().clone();
        let before_timeline = editor.instruction_timeline().clone();
        let before_revision = editor.revision();
        let before_undo = editor.can_undo();
        let before_redo = editor.can_redo();
        let wrong_kind = [ExpectedStackedFoldCreaseV1 {
            kind: EdgeKind::Valley,
            ..fixture.expected_creases[0]
        }];

        assert!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &wrong_kind,
                    ..fixture.geometry_input(&lineage)
                },
                StackedFoldGeometryLimitsV1::default(),
            )
            .is_err()
        );
        assert_eq!(editor.pattern(), &before_pattern);
        assert_eq!(editor.paper(), &before_paper);
        assert_eq!(editor.instruction_timeline(), &before_timeline);
        assert_eq!(editor.revision(), before_revision);
        assert_eq!(editor.can_undo(), before_undo);
        assert_eq!(editor.can_redo(), before_redo);
    }

    #[test]
    fn exact_carrier_coverage_distinguishes_adjacency_gap_and_overlap() {
        let carrier = GeometryCarrier {
            public: StackedFoldGeometryCarrierV1::ExpectedCrease(0),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(3.0, 0.0),
            kind: EdgeKind::Mountain,
        };
        let edge = |start: f64, end: f64| GeometryEdgeRecord {
            id: EdgeId::new(),
            start_vertex: VertexId::new(),
            end_vertex: VertexId::new(),
            start: Point2::new(start, 0.0),
            end: Point2::new(end, 0.0),
            kind: EdgeKind::Mountain,
        };

        assert!(carrier_has_exact_coverage(
            carrier,
            &[edge(3.0, 2.0), edge(0.0, 1.0), edge(2.0, 1.0)]
        ));
        assert!(!carrier_has_exact_coverage(
            carrier,
            &[edge(0.0, 1.0), edge(2.0, 3.0)]
        ));
        assert!(!carrier_has_exact_coverage(
            carrier,
            &[edge(0.0, 2.0), edge(1.0, 3.0)]
        ));
    }

    #[test]
    fn exact_overlap_rejects_positive_interval_but_allows_point_contact_and_crossing() {
        let horizontal_start = Point2::new(0.0, 0.0);
        let horizontal_end = Point2::new(2.0, 0.0);
        assert!(
            segments_share_positive_collinear_interval(
                horizontal_start,
                horizontal_end,
                Point2::new(3.0, 0.0),
                Point2::new(1.0, 0.0),
            )
            .unwrap()
        );
        assert!(
            !segments_share_positive_collinear_interval(
                horizontal_start,
                horizontal_end,
                Point2::new(2.0, 0.0),
                Point2::new(3.0, 0.0),
            )
            .unwrap()
        );
        assert!(
            !segments_share_positive_collinear_interval(
                horizontal_start,
                horizontal_end,
                Point2::new(1.0, -1.0),
                Point2::new(1.0, 1.0),
            )
            .unwrap()
        );
    }

    #[test]
    fn stale_layer_order_is_rejected_before_any_lineage_is_published() {
        let mut fixture = fixture();
        fixture.source_layer_order.provenance.source.source_revision = 6;

        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default()),
            Err(FaceLineageError::LayerOrderNotCurrent)
        );
    }

    #[test]
    fn oversized_layer_registry_is_rejected_without_clone_or_target_work() {
        let mut fixture = fixture();
        let repeated_face = fixture.source_layer_order.material_faces[0];
        fixture
            .source_layer_order
            .material_faces
            .resize(DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES + 1, repeated_face);
        fixture.target_pattern.edges[0].start = VertexId::new();

        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default()),
            Err(FaceLineageError::LayerOrderMaterialRegistryMismatch)
        );
    }

    #[test]
    fn revision_gap_and_unrelated_paper_changes_are_rejected() {
        let fixture = fixture();
        let revision_gap = FaceLineageInput {
            target_revision: 9,
            ..fixture.input()
        };
        assert_eq!(
            prepare_face_lineage_v1(revision_gap, FaceLineageLimits::default()),
            Err(FaceLineageError::TargetRevisionNotNext {
                expected: 8,
                actual: 9,
            })
        );

        let mut changed_paper = fixture.target_paper.clone();
        changed_paper.front.color.red ^= 1;
        let paper_change = FaceLineageInput {
            target_paper: &changed_paper,
            ..fixture.input()
        };
        assert_eq!(
            prepare_face_lineage_v1(paper_change, FaceLineageLimits::default()),
            Err(FaceLineageError::PaperPropertiesChanged)
        );
    }

    #[test]
    fn exact_per_source_area_rejects_material_loss() {
        let fixture = fixture();
        let smaller = create_rectangular_sheet(200.0, 200.0, false).expect("smaller rectangle");
        let (mut target_pattern, target_paper) = smaller.into_parts();
        target_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: target_paper.boundary_vertices[0],
            end: target_paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let input = FaceLineageInput {
            target_pattern: &target_pattern,
            target_paper: &target_paper,
            ..fixture.input()
        };

        assert!(matches!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Err(FaceLineageError::SourceFaceAreaMismatch { .. })
        ));
    }

    #[test]
    fn no_geometry_split_is_not_a_stacked_fold_lineage() {
        let fixture = fixture();
        let input = FaceLineageInput {
            target_pattern: &fixture.source_pattern,
            target_paper: &fixture.source_paper,
            ..fixture.input()
        };

        assert_eq!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Err(FaceLineageError::NoSourceFaceSplit)
        );
    }

    #[test]
    fn stale_revision_and_resource_failure_leave_editor_state_unchanged() {
        let fixture = fixture();
        let editor =
            EditorState::with_paper(fixture.source_pattern.clone(), fixture.source_paper.clone());
        let before_pattern = editor.pattern().clone();
        let before_paper = editor.paper().clone();
        let before_timeline = editor.instruction_timeline().clone();
        let before_revision = editor.revision();
        let before_undo = editor.can_undo();
        let before_redo = editor.can_redo();

        let stale = FaceLineageInput {
            source_revision: u64::MAX,
            target_revision: 0,
            ..fixture.input()
        };
        assert_eq!(
            prepare_face_lineage_v1(stale, FaceLineageLimits::default()),
            Err(FaceLineageError::SourceRevisionCannotAdvance)
        );

        let limits = FaceLineageLimits {
            max_target_edges: fixture.target_pattern.edges.len() - 1,
            ..FaceLineageLimits::default()
        };
        assert!(matches!(
            prepare_face_lineage_v1(fixture.input(), limits),
            Err(FaceLineageError::ResourceLimit {
                resource: FaceLineageResource::TargetEdges,
                ..
            })
        ));

        assert_eq!(editor.pattern(), &before_pattern);
        assert_eq!(editor.paper(), &before_paper);
        assert_eq!(editor.instruction_timeline(), &before_timeline);
        assert_eq!(editor.revision(), before_revision);
        assert_eq!(editor.can_undo(), before_undo);
        assert_eq!(editor.can_redo(), before_redo);
    }

    #[test]
    fn face_lineage_rejects_json_revision_ceiling_and_larger_source_revisions() {
        let fixture = fixture();
        let final_source_revision = crate::MAX_REVISION - 1;
        let final_source_layer_order = proven_layer_order(
            fixture.identity,
            final_source_revision,
            &fixture.source_pattern,
            &fixture.source_paper,
        );
        let final_valid_input = FaceLineageInput {
            source_revision: final_source_revision,
            source_layer_order: &final_source_layer_order,
            target_revision: crate::MAX_REVISION,
            ..fixture.input()
        };
        let final_lineage =
            prepare_face_lineage_v1(final_valid_input, FaceLineageLimits::default())
                .expect("the final JSON-safe target revision must remain admissible");
        assert_eq!(final_lineage.source_revision(), final_source_revision);
        assert_eq!(final_lineage.target_revision(), crate::MAX_REVISION);

        for source_revision in [crate::MAX_REVISION, crate::MAX_REVISION + 1, u64::MAX] {
            let input = FaceLineageInput {
                source_revision,
                target_revision: source_revision.saturating_add(1),
                ..fixture.input()
            };

            assert_eq!(
                prepare_face_lineage_v1(input, FaceLineageLimits::default()),
                Err(FaceLineageError::SourceRevisionCannotAdvance),
                "source revision {source_revision} must not produce a lineage"
            );
        }
    }

    #[test]
    fn exact_work_limits_admit_equality_and_reject_the_next_operation() {
        let fixture = fixture();
        let exact_limit = 2 * 4 * 3 * 2;
        let inclusive = FaceLineageLimits {
            max_face_pairs: 2,
            max_exact_containment_tests: exact_limit,
            ..FaceLineageLimits::default()
        };
        prepare_face_lineage_v1(fixture.input(), inclusive)
            .expect("the documented resource limits admit equality");

        let pair_limited = FaceLineageLimits {
            max_face_pairs: 1,
            ..inclusive
        };
        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), pair_limited),
            Err(FaceLineageError::ResourceLimit {
                resource: FaceLineageResource::FacePairs,
                actual: 2,
                maximum: 1,
            })
        );

        let predicate_limited = FaceLineageLimits {
            max_exact_containment_tests: exact_limit - 1,
            ..inclusive
        };
        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), predicate_limited),
            Err(FaceLineageError::ResourceLimit {
                resource: FaceLineageResource::ExactContainmentTests,
                actual: exact_limit,
                maximum: exact_limit - 1,
            })
        );
    }

    #[test]
    fn convex_vertex_certificate_contains_whole_target_edges() {
        let source = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let boundary_chord = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let concave_target = [
            Point2::new(1.0, 1.0),
            Point2::new(9.0, 1.0),
            Point2::new(5.0, 5.0),
            Point2::new(9.0, 9.0),
            Point2::new(1.0, 9.0),
        ];
        let outside = [
            Point2::new(1.0, 1.0),
            Point2::new(11.0, 5.0),
            Point2::new(1.0, 9.0),
        ];

        assert!(polygon_is_within_convex_source(&boundary_chord, &source).unwrap());
        assert!(polygon_is_within_convex_source(&concave_target, &source).unwrap());
        assert!(!polygon_is_within_convex_source(&outside, &source).unwrap());
    }

    #[test]
    fn exact_binary64_units_cover_subnormal_normal_and_maximum_values() {
        let minimum_subnormal = f64::from_bits(1);
        assert_eq!(
            exact_f64_at_minimum_scale(minimum_subnormal),
            BigInt::from(1_u8)
        );
        assert_eq!(
            exact_f64_at_minimum_scale(-minimum_subnormal),
            BigInt::from(-1_i8)
        );
        assert_eq!(
            exact_f64_at_minimum_scale(f64::MIN_POSITIVE),
            BigInt::from(1_u8) << 52_usize
        );
        assert_eq!(
            exact_f64_at_minimum_scale(1.0),
            BigInt::from(1_u8) << 1074_usize
        );
        assert_eq!(
            exact_f64_at_minimum_scale(f64::MAX),
            BigInt::from((1_u64 << 53) - 1) << 2045_usize
        );
        assert_eq!(exact_f64_at_minimum_scale(-0.0), BigInt::from(0_u8));
    }

    #[test]
    fn exact_area_uses_binary64_values_without_rounding_the_sum() {
        let huge = f64::from_bits(0x7fe0_0000_0000_0000);
        let tiny = f64::from_bits(1);
        let polygon = [
            Point2::new(0.0, 0.0),
            Point2::new(huge, 0.0),
            Point2::new(huge, tiny),
            Point2::new(0.0, tiny),
        ];
        assert!(exact_polygon_double_area(&polygon) > BigInt::from(0_u8));
        let mut reversed = polygon;
        reversed.reverse();
        assert_eq!(
            exact_polygon_double_area(&reversed),
            -exact_polygon_double_area(&polygon)
        );
    }
}
