//! Deterministic, fail-closed global flat-foldability proofs.
//!
//! The first model proves deterministic layer orders for connected convex
//! material faces using exact flat embeddings, overlap cells, and facewise
//! constraints. Local necessary-condition violations can also disprove an
//! input. Unsupported, stale, over-limit, or incomplete inputs remain
//! `Unknown`.

use std::collections::{HashMap, HashSet};

use ori_domain::{CreasePattern, EdgeId, FaceId, Paper, ProjectId, VertexId};
use ori_topology::{
    CooperativeAnalysisAbort, CooperativeAnalysisCheckpoint, EdgeIncidence, FaceExtractionInput,
    FaceKey, FoldAssignment, LocalFlatFoldabilityModel, LocalFlatFoldabilityReport,
    LocalFlatFoldabilityReportStatus, LocalFoldabilityConditionStatus, LocalFoldabilityReason,
    LocalVertexFoldabilityVerdict, MAX_EXACT_FOLD_DEGREE, TopologyIssueSeverity, TopologySnapshot,
    analyze_faces_with_checkpoint, analyze_local_flat_foldability_with_checkpoint,
};
use serde::Serialize;
use thiserror::Error;

mod constraints;
mod exact;
mod facewise;
mod fingerprint;

pub use exact::{ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign};
use fingerprint::fold_model_fingerprint_v1_with_checkpoint;
pub use fingerprint::{FoldModelFingerprintV1, fold_model_fingerprint_v1};

pub const DEFAULT_MAX_FACES: usize = 2_048;
pub const DEFAULT_MAX_FACE_BOUNDARY_HALF_EDGES: usize = 100_000;
pub const DEFAULT_MAX_HINGES: usize = 100_000;
pub const DEFAULT_MAX_EDGE_INCIDENCE_RECORDS: usize = 500_000;
pub const DEFAULT_MAX_LOCAL_VERTICES: usize = 100_000;
pub const DEFAULT_MAX_TOTAL_RECORDS: usize = 2_000_000;
pub const DEFAULT_MAX_OVERLAP_FACE_PAIRS: usize = 500_000;
pub const DEFAULT_MAX_ARRANGEMENT_SEGMENTS: usize = 1_000_000;
pub const DEFAULT_MAX_OVERLAP_CELLS: usize = 500_000;
pub const DEFAULT_MAX_CONSTRAINTS: usize = 5_000_000;
pub const DEFAULT_MAX_SEARCH_NODES: usize = 10_000_000;
pub const DEFAULT_MAX_EXACT_INTEGER_BITS: usize = 65_536;
pub const DEFAULT_MAX_EXACT_OPERATIONS: usize = 100_000_000;
pub const DEFAULT_MAX_CERTIFICATE_BYTES: usize = 128 * 1024 * 1024;
/// Bounds each immutable source collection before canonical fingerprint,
/// topology, or local-report reconstruction allocates derived indexes.
///
/// Canonical sort calls are checkpointed immediately before and after; this
/// finite cap bounds the one non-interruptible library sort interval.
pub const DEFAULT_MAX_SOURCE_VERTICES: usize = 100_000;
pub const DEFAULT_MAX_SOURCE_EDGES: usize = 100_000;
pub const DEFAULT_MAX_PAPER_BOUNDARY_VERTICES: usize = 100_000;

/// Versioned proof model. New proof classes require a new closed variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityModelId {
    ConvexFacesFacewiseV1,
}

pub const GLOBAL_FLAT_FOLDABILITY_MODEL_ID: GlobalFlatFoldabilityModelId =
    GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1;

/// Versioned representation consumed by later layer-aware operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerOrderModelId {
    FacewiseLayerOrderV1,
}

pub const LAYER_ORDER_MODEL_ID: LayerOrderModelId = LayerOrderModelId::FacewiseLayerOrderV1;

/// Complete source binding shared by the verdict and any derived layer order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct GlobalFlatFoldabilityProvenance {
    pub identity_namespace: Option<ProjectId>,
    pub source_revision: u64,
    pub source_fingerprint: Option<FoldModelFingerprintV1>,
    pub model_id: GlobalFlatFoldabilityModelId,
}

impl GlobalFlatFoldabilityProvenance {
    /// Builds the complete provenance expected for one immutable geometry.
    #[must_use]
    pub fn for_geometry(
        identity_namespace: ProjectId,
        source_revision: u64,
        paper: &Paper,
        crease_pattern: &CreasePattern,
    ) -> Self {
        Self {
            identity_namespace: Some(identity_namespace),
            source_revision,
            source_fingerprint: Some(fold_model_fingerprint_v1(crease_pattern, paper)),
            model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
        }
    }
}

/// Identity- and revision-labelled immutable inputs.
///
/// Geometry-backed analysis independently reconstructs topology and local
/// evidence from `identity_namespace`, paper, and pattern. The supplied
/// artifacts must match those reconstructions exactly. `current()` remains a
/// compatibility capture route but, without geometry and identity, can only
/// return `Unknown`.
#[derive(Debug, Clone, Copy)]
pub struct GlobalFlatFoldabilityInput<'a> {
    pub identity_namespace: Option<ProjectId>,
    pub source_revision: u64,
    pub local_report_source_revision: u64,
    pub paper: Option<&'a Paper>,
    pub crease_pattern: Option<&'a CreasePattern>,
    pub topology: &'a TopologySnapshot,
    pub local_flat_foldability: &'a LocalFlatFoldabilityReport,
}

impl<'a> GlobalFlatFoldabilityInput<'a> {
    /// Binds a report produced beside this topology to the topology revision.
    #[must_use]
    pub const fn current(
        topology: &'a TopologySnapshot,
        local_flat_foldability: &'a LocalFlatFoldabilityReport,
    ) -> Self {
        Self {
            identity_namespace: None,
            source_revision: topology.source_revision,
            local_report_source_revision: topology.source_revision,
            paper: None,
            crease_pattern: None,
            topology,
            local_flat_foldability,
        }
    }

    /// Includes immutable source coordinates for the full facewise model.
    #[must_use]
    pub const fn current_with_geometry(
        identity_namespace: ProjectId,
        paper: &'a Paper,
        crease_pattern: &'a CreasePattern,
        topology: &'a TopologySnapshot,
        local_flat_foldability: &'a LocalFlatFoldabilityReport,
    ) -> Self {
        Self {
            identity_namespace: Some(identity_namespace),
            source_revision: topology.source_revision,
            local_report_source_revision: topology.source_revision,
            paper: Some(paper),
            crease_pattern: Some(crease_pattern),
            topology,
            local_flat_foldability,
        }
    }

    /// Adds source geometry without changing either revision binding.
    #[must_use]
    pub const fn with_geometry(
        mut self,
        identity_namespace: ProjectId,
        paper: &'a Paper,
        crease_pattern: &'a CreasePattern,
    ) -> Self {
        self.identity_namespace = Some(identity_namespace);
        self.paper = Some(paper);
        self.crease_pattern = Some(crease_pattern);
        self
    }
}

/// Deterministic record-count limits. Equality is admitted; only `limit + 1`
/// is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalFlatFoldabilityLimits {
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_paper_boundary_vertices: usize,
    pub max_faces: usize,
    pub max_face_boundary_half_edges: usize,
    pub max_hinges: usize,
    pub max_edge_incidence_records: usize,
    pub max_local_vertices: usize,
    pub max_total_records: usize,
    pub max_overlap_face_pairs: usize,
    pub max_arrangement_segments: usize,
    pub max_overlap_cells: usize,
    pub max_constraints: usize,
    pub max_search_nodes: usize,
    pub max_exact_integer_bits: usize,
    pub max_exact_operations: usize,
    /// Logical proof-storage budget shared by the supported 64-bit Windows and
    /// macOS targets. This is not an operating-system heap or resident-set-size
    /// limit; structural record limits and explicitly fallible allocations are
    /// separate safeguards.
    pub max_certificate_bytes: usize,
}

impl Default for GlobalFlatFoldabilityLimits {
    fn default() -> Self {
        Self {
            max_source_vertices: DEFAULT_MAX_SOURCE_VERTICES,
            max_source_edges: DEFAULT_MAX_SOURCE_EDGES,
            max_paper_boundary_vertices: DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_faces: DEFAULT_MAX_FACES,
            max_face_boundary_half_edges: DEFAULT_MAX_FACE_BOUNDARY_HALF_EDGES,
            max_hinges: DEFAULT_MAX_HINGES,
            max_edge_incidence_records: DEFAULT_MAX_EDGE_INCIDENCE_RECORDS,
            max_local_vertices: DEFAULT_MAX_LOCAL_VERTICES,
            max_total_records: DEFAULT_MAX_TOTAL_RECORDS,
            max_overlap_face_pairs: DEFAULT_MAX_OVERLAP_FACE_PAIRS,
            max_arrangement_segments: DEFAULT_MAX_ARRANGEMENT_SEGMENTS,
            max_overlap_cells: DEFAULT_MAX_OVERLAP_CELLS,
            max_constraints: DEFAULT_MAX_CONSTRAINTS,
            max_search_nodes: DEFAULT_MAX_SEARCH_NODES,
            max_exact_integer_bits: DEFAULT_MAX_EXACT_INTEGER_BITS,
            max_exact_operations: DEFAULT_MAX_EXACT_OPERATIONS,
            max_certificate_bytes: DEFAULT_MAX_CERTIFICATE_BYTES,
        }
    }
}

/// Execution state is deliberately outside the three mathematical outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalFlatFoldabilityExecutionControl {
    Continue,
    Cancelled,
}

/// Monotonic-clock ownership remains with the caller; the solver only consumes
/// this closed checkpoint result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalFlatFoldabilityCheckpoint {
    Continue,
    DeadlineReached,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityPhase {
    Capturing,
    ValidatingLocalConditions,
    BuildingFlatEmbedding,
    BuildingOverlapArrangement,
    BuildingConstraints,
    Propagating,
    Searching,
    VerifyingCertificate,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GlobalFlatFoldabilityProgress {
    pub phase: GlobalFlatFoldabilityPhase,
    pub completed_work: usize,
    pub total_work: Option<usize>,
    pub exact_operations: usize,
    pub overlap_face_pairs: usize,
    pub overlap_cells: usize,
    pub constraints: usize,
    pub search_nodes: usize,
}

/// Thread-confined callback boundary for deadline, cancellation, and progress.
/// Implementations must not mutate the analyzed project snapshot.
pub trait GlobalFlatFoldabilityObserver {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        GlobalFlatFoldabilityCheckpoint::Continue
    }

    fn on_progress(&mut self, _progress: GlobalFlatFoldabilityProgress) {}
}

#[derive(Debug, Default)]
pub struct NoopGlobalFlatFoldabilityObserver;

impl GlobalFlatFoldabilityObserver for NoopGlobalFlatFoldabilityObserver {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityInternalError {
    WorkCountOverflow,
    ValidatedTopologyInvariantLost,
}

/// Cancellation and implementation failure cannot be confused with
/// Possible/Impossible/Unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GlobalFlatFoldabilityExecutionError {
    #[error("global flat-foldability analysis was cancelled")]
    Cancelled,
    #[error("global flat-foldability analysis failed internally: {reason:?}")]
    Internal {
        reason: GlobalFlatFoldabilityInternalError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GlobalFlatFoldabilityWorkCounts {
    pub source_vertex_records: usize,
    pub source_edge_records: usize,
    pub paper_boundary_vertex_records: usize,
    pub face_records: usize,
    pub face_boundary_half_edges: usize,
    pub hinge_records: usize,
    pub edge_incidence_records: usize,
    pub local_vertex_records: usize,
    pub total_records: usize,
    pub overlap_face_pairs: usize,
    pub arrangement_segments: usize,
    pub overlap_cells: usize,
    pub constraints: usize,
    pub search_nodes: usize,
    pub exact_operations: usize,
    pub exact_values: usize,
    pub certificate_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LayerFace {
    pub face_id: FaceId,
    pub face_key: FaceKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct OverlapCellKey(pub [u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldedFaceOrientation {
    FrontUp,
    BackUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoldedFaceSnapshot {
    pub face: LayerFace,
    pub source_to_flat: ExactAffineTransform,
    pub orientation: FoldedFaceOrientation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverlapCellSnapshot {
    pub cell_key: OverlapCellKey,
    pub exact_boundary: Vec<ExactPointValue>,
    pub covering_faces: Vec<LayerFace>,
    pub bottom_to_top_faces: Vec<FaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FacePairOrderSnapshot {
    pub lower_face: LayerFace,
    pub upper_face: LayerFace,
    pub supporting_cells: Vec<OverlapCellKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FacewiseProofSummary {
    pub material_faces: usize,
    pub overlap_face_pairs: usize,
    pub overlap_cells: usize,
    pub constraints: usize,
    pub search_nodes: usize,
    pub maximum_ply: usize,
    pub certificate_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayerOrderDerivation {
    SingleFace {
        face: LayerFace,
    },
    SingleHinge {
        hinge_edge: EdgeId,
        assignment: FoldAssignment,
        canonical_first: LayerFace,
        canonical_second: LayerFace,
    },
    FacewiseCertificate {
        reference_face: LayerFace,
        overlap_cell_count: usize,
        constraint_count: usize,
    },
}

/// Proof provenance retained with a layer order rather than inferred later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LayerOrderProvenance {
    pub source: GlobalFlatFoldabilityProvenance,
    pub derivation: LayerOrderDerivation,
}

/// Facewise layer-order certificate. `overlap_cells` and `face_pair_orders`
/// are authoritative; the whole-model list is a deterministic presentation
/// order because valid orders in disjoint cells need not share one global DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayerOrderSnapshot {
    pub model_id: LayerOrderModelId,
    /// Canonical material-face registry, ordered by `FaceKey`.
    pub material_faces: Vec<LayerFace>,
    /// A whole-model linear extension when one exists. Location-dependent
    /// cell orders remain valid when this is `None`.
    pub global_bottom_to_top: Option<Vec<LayerFace>>,
    pub provenance: LayerOrderProvenance,
    pub reference_face: Option<LayerFace>,
    pub folded_faces: Vec<FoldedFaceSnapshot>,
    pub overlap_cells: Vec<OverlapCellSnapshot>,
    pub face_pair_orders: Vec<FacePairOrderSnapshot>,
    pub proof_summary: Option<FacewiseProofSummary>,
}

impl LayerOrderSnapshot {
    /// Rejects stale, differently identified, or differently modelled layer
    /// state at a later boundary.
    #[must_use]
    pub fn is_current_for(&self, provenance: &GlobalFlatFoldabilityProvenance) -> bool {
        self.provenance.source == *provenance
            && self.provenance.source.identity_namespace.is_some()
            && self.provenance.source.source_fingerprint.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityVerdict {
    Possible,
    Impossible,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityPossibleReason {
    TrivialSingleFace {
        face: LayerFace,
    },
    AssignedSingleHinge {
        hinge_edge: EdgeId,
        assignment: FoldAssignment,
    },
    FacewiseConstraintCertificate {
        reference_face: LayerFace,
        overlap_cell_count: usize,
        constraint_count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FacewiseConstraintKind {
    Antisymmetry,
    Transitivity,
    TacoTaco,
    TacoTortilla,
    TortillaTortilla,
    MountainValley,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LocalNecessaryConditionViolation {
    pub vertex: VertexId,
    pub kawasaki_violated: bool,
    pub maekawa_violated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityImpossibleReason {
    LocalNecessaryConditionViolated {
        violations: Vec<LocalNecessaryConditionViolation>,
    },
    InconsistentFlatEmbedding {
        face: LayerFace,
        conflicting_hinge: EdgeId,
        conflicting_vertex: VertexId,
    },
    FacewiseConstraintContradiction {
        constraint_kind: FacewiseConstraintKind,
        faces: Vec<LayerFace>,
        supporting_cell: Option<OverlapCellKey>,
    },
    FacewiseSearchExhausted {
        variable_count: usize,
        constraint_count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlatFoldabilityInputArtifact {
    TopologySnapshot,
    LocalFlatFoldabilityReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlatFoldabilityResource {
    SourceVertices,
    SourceEdges,
    PaperBoundaryVertices,
    Faces,
    FaceBoundaryHalfEdges,
    Hinges,
    EdgeIncidenceRecords,
    LocalVertices,
    TotalRecords,
    OverlapFacePairs,
    ArrangementSegments,
    OverlapCells,
    Constraints,
    SearchNodes,
    ExactOperations,
    CertificateBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlatFoldabilityProofIncompleteReason {
    NoMaterialFaces,
    DisconnectedFacesWithoutHinge,
    SingleHingeDoesNotCoverExactlyTwoFaces,
    LocalNecessaryConditionsBlocked,
    LocalNecessaryConditionsIndeterminate,
    GeometryInputUnavailable,
    CertificateReverificationFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedFlatFoldabilityTopology {
    CutEdge,
    MissingSourceVertex,
    MissingSourceEdge,
    DuplicateSourceVertex,
    DuplicateSourceEdge,
    DisconnectedMaterial,
    NonSimpleFace,
    UnassignedHinge,
    InconsistentSourceBoundary,
    InvalidBinary64Coordinate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlatFoldabilityInputConsistencyIssue {
    TopologyGeometryMismatch,
    LocalReportGeometryMismatch,
    DuplicateFaceId { face: FaceId },
    DuplicateFaceKey { face_key: FaceKey },
    DuplicateIncidenceEdge { edge: EdgeId },
    DuplicateHingeEdge { edge: EdgeId },
    UnknownIncidenceFace { edge: EdgeId, face: FaceId },
    UnknownHingeFace { edge: EdgeId, face: FaceId },
    SelfHinge { edge: EdgeId, face: FaceId },
    NonCanonicalHingeFaces { edge: EdgeId },
    HingeIncidenceMissing { edge: EdgeId },
    HingeAdjacencyMissing { edge: EdgeId },
    HingeAssignmentMismatch { edge: EdgeId },
    HingeFacesMismatch { edge: EdgeId },
    UnexpectedLocalFoldDegreeLimit { expected: usize, actual: usize },
    DuplicateLocalVertex { vertex: VertexId },
    LocalVertexCountsMismatch { vertex: VertexId },
    LocalVertexVerdictMismatch { vertex: VertexId },
    LocalReportCountsMismatch,
    LocalReportStatusMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityUnknownReason {
    StaleProvenance {
        artifact: FlatFoldabilityInputArtifact,
        expected_revision: u64,
        actual_revision: u64,
    },
    ResourceLimitReached {
        resource: FlatFoldabilityResource,
        limit: usize,
        observed: usize,
    },
    UnsupportedTargetClass {
        hinge_count: usize,
    },
    UnsupportedTopology {
        reason: UnsupportedFlatFoldabilityTopology,
    },
    NonConvexFace {
        face: LayerFace,
    },
    TimeLimitReached {
        phase: GlobalFlatFoldabilityPhase,
    },
    ExactNumberLimitReached {
        limit_bits: usize,
        observed_bits: usize,
    },
    OverlapArrangementLimitReached {
        resource: FlatFoldabilityResource,
        limit: usize,
        observed: usize,
    },
    ConstraintLimitReached {
        limit: usize,
        observed: usize,
    },
    InconsistentInput {
        issue: FlatFoldabilityInputConsistencyIssue,
    },
    ProofIncomplete {
        reason: FlatFoldabilityProofIncompleteReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum GlobalFlatFoldabilityOutcome {
    Possible {
        reason: GlobalFlatFoldabilityPossibleReason,
        layer_order: Box<LayerOrderSnapshot>,
    },
    Impossible {
        reason: GlobalFlatFoldabilityImpossibleReason,
    },
    Unknown {
        reason: GlobalFlatFoldabilityUnknownReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlobalFlatFoldabilityReport {
    pub provenance: GlobalFlatFoldabilityProvenance,
    pub work_counts: GlobalFlatFoldabilityWorkCounts,
    pub outcome: GlobalFlatFoldabilityOutcome,
}

impl GlobalFlatFoldabilityReport {
    #[must_use]
    pub const fn verdict(&self) -> GlobalFlatFoldabilityVerdict {
        match self.outcome {
            GlobalFlatFoldabilityOutcome::Possible { .. } => GlobalFlatFoldabilityVerdict::Possible,
            GlobalFlatFoldabilityOutcome::Impossible { .. } => {
                GlobalFlatFoldabilityVerdict::Impossible
            }
            GlobalFlatFoldabilityOutcome::Unknown { .. } => GlobalFlatFoldabilityVerdict::Unknown,
        }
    }

    #[must_use]
    pub const fn layer_order(&self) -> Option<&LayerOrderSnapshot> {
        match &self.outcome {
            GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => Some(layer_order),
            GlobalFlatFoldabilityOutcome::Impossible { .. }
            | GlobalFlatFoldabilityOutcome::Unknown { .. } => None,
        }
    }
}

/// Runs the deterministic first proof model.
pub fn analyze_global_flat_foldability(
    input: GlobalFlatFoldabilityInput<'_>,
    limits: GlobalFlatFoldabilityLimits,
) -> Result<GlobalFlatFoldabilityReport, GlobalFlatFoldabilityExecutionError> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    analyze_global_flat_foldability_with_observer(input, limits, &mut observer)
}

/// Runs with an explicit deterministic cancellation checkpoint.
pub fn analyze_global_flat_foldability_with_control(
    input: GlobalFlatFoldabilityInput<'_>,
    limits: GlobalFlatFoldabilityLimits,
    control: GlobalFlatFoldabilityExecutionControl,
) -> Result<GlobalFlatFoldabilityReport, GlobalFlatFoldabilityExecutionError> {
    let mut observer = FixedGlobalFlatFoldabilityObserver { control };
    analyze_global_flat_foldability_with_observer(input, limits, &mut observer)
}

/// Runs with repeated deadline/cancellation checkpoints and monotonic progress.
pub fn analyze_global_flat_foldability_with_observer<O: GlobalFlatFoldabilityObserver + ?Sized>(
    input: GlobalFlatFoldabilityInput<'_>,
    limits: GlobalFlatFoldabilityLimits,
    observer: &mut O,
) -> Result<GlobalFlatFoldabilityReport, GlobalFlatFoldabilityExecutionError> {
    let mut provenance = GlobalFlatFoldabilityProvenance {
        identity_namespace: input.identity_namespace,
        source_revision: input.source_revision,
        source_fingerprint: None,
        model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    };
    let work_counts = count_work(&input)?;
    if let Some(reason) = phase_checkpoint(
        observer,
        GlobalFlatFoldabilityPhase::Capturing,
        work_counts,
        Some(work_counts.total_records),
    )? {
        return Ok(unknown(provenance, work_counts, reason));
    }

    if input.topology.source_revision != input.source_revision {
        return Ok(unknown(
            provenance,
            work_counts,
            GlobalFlatFoldabilityUnknownReason::StaleProvenance {
                artifact: FlatFoldabilityInputArtifact::TopologySnapshot,
                expected_revision: input.source_revision,
                actual_revision: input.topology.source_revision,
            },
        ));
    }
    if input.local_report_source_revision != input.source_revision {
        return Ok(unknown(
            provenance,
            work_counts,
            GlobalFlatFoldabilityUnknownReason::StaleProvenance {
                artifact: FlatFoldabilityInputArtifact::LocalFlatFoldabilityReport,
                expected_revision: input.source_revision,
                actual_revision: input.local_report_source_revision,
            },
        ));
    }
    if let Some(reason) = first_limit_failure(work_counts, limits) {
        return Ok(unknown(provenance, work_counts, reason));
    }

    let (Some(identity_namespace), Some(paper), Some(crease_pattern)) =
        (input.identity_namespace, input.paper, input.crease_pattern)
    else {
        return Ok(unknown(
            provenance,
            work_counts,
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable,
            },
        ));
    };

    if let Some(reason) = phase_checkpoint(
        observer,
        GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
        work_counts,
        Some(work_counts.local_vertex_records),
    )? {
        return Ok(unknown(provenance, work_counts, reason));
    }
    let fingerprint = {
        let mut checkpoint = || observer_reverification_checkpoint(observer);
        fold_model_fingerprint_v1_with_checkpoint(crease_pattern, paper, &mut checkpoint)
    };
    match fingerprint {
        Ok(fingerprint) => provenance.source_fingerprint = Some(fingerprint),
        Err(SourceReverificationAbort::Unknown(reason)) => {
            return Ok(unknown(provenance, work_counts, reason));
        }
        Err(SourceReverificationAbort::Execution(error)) => return Err(error),
    }
    match reverify_source_artifacts(
        identity_namespace,
        input.source_revision,
        paper,
        crease_pattern,
        input.topology,
        input.local_flat_foldability,
        observer,
    ) {
        Ok(()) => {}
        Err(SourceReverificationAbort::Unknown(reason)) => {
            return Ok(unknown(provenance, work_counts, reason));
        }
        Err(SourceReverificationAbort::Execution(error)) => return Err(error),
    }

    let canonical_faces = match validate_topology(input.topology) {
        Ok(faces) => faces,
        Err(reason) => return Ok(unknown(provenance, work_counts, reason)),
    };
    if canonical_faces.is_empty() {
        return Ok(unknown(
            provenance,
            work_counts,
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::NoMaterialFaces,
            },
        ));
    }

    let local = match validate_local_report(input.local_flat_foldability) {
        Ok(local) => local,
        Err(reason) => return Ok(unknown(provenance, work_counts, reason)),
    };
    match local {
        LocalReportEvidence::Blocked => {
            return Ok(unknown(
                provenance,
                work_counts,
                GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::LocalNecessaryConditionsBlocked,
                },
            ));
        }
        LocalReportEvidence::Indeterminate => {
            return Ok(unknown(
                provenance,
                work_counts,
                GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason:
                        FlatFoldabilityProofIncompleteReason::LocalNecessaryConditionsIndeterminate,
                },
            ));
        }
        LocalReportEvidence::Violated(violations) => {
            return Ok(GlobalFlatFoldabilityReport {
                provenance,
                work_counts,
                outcome: GlobalFlatFoldabilityOutcome::Impossible {
                    reason:
                        GlobalFlatFoldabilityImpossibleReason::LocalNecessaryConditionViolated {
                            violations,
                        },
                },
            });
        }
        LocalReportEvidence::NoViolation => {}
    }

    facewise::analyze_facewise(
        facewise::FacewiseAnalysisInput {
            paper,
            crease_pattern,
            topology: input.topology,
            canonical_faces: &canonical_faces,
            provenance,
            work_counts,
            limits,
        },
        observer,
    )
}

enum SourceReverificationAbort {
    Unknown(GlobalFlatFoldabilityUnknownReason),
    Execution(GlobalFlatFoldabilityExecutionError),
}

fn observer_reverification_checkpoint<O: GlobalFlatFoldabilityObserver + ?Sized>(
    observer: &mut O,
) -> Result<(), SourceReverificationAbort> {
    match observer.checkpoint() {
        GlobalFlatFoldabilityCheckpoint::Continue => Ok(()),
        GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
            Err(SourceReverificationAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
                },
            ))
        }
        GlobalFlatFoldabilityCheckpoint::Cancelled => Err(SourceReverificationAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled,
        )),
    }
}

fn reverify_source_artifacts<O: GlobalFlatFoldabilityObserver + ?Sized>(
    identity_namespace: ProjectId,
    source_revision: u64,
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    local: &LocalFlatFoldabilityReport,
    observer: &mut O,
) -> Result<(), SourceReverificationAbort> {
    let mut checkpoint = || match observer.checkpoint() {
        GlobalFlatFoldabilityCheckpoint::Continue => CooperativeAnalysisCheckpoint::Continue,
        GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
            CooperativeAnalysisCheckpoint::DeadlineReached
        }
        GlobalFlatFoldabilityCheckpoint::Cancelled => CooperativeAnalysisCheckpoint::Cancelled,
    };
    let topology_report = analyze_faces_with_checkpoint(
        FaceExtractionInput {
            identity_namespace,
            source_revision,
            paper,
            pattern: crease_pattern,
        },
        &mut checkpoint,
    )
    .map_err(source_reverification_abort)?;
    let topology_matches = topology_report.snapshot.as_ref() == Some(topology)
        && topology_report
            .issues
            .iter()
            .all(|issue| issue.severity == TopologyIssueSeverity::Warning);
    if !topology_matches {
        return Err(SourceReverificationAbort::Unknown(inconsistent(
            FlatFoldabilityInputConsistencyIssue::TopologyGeometryMismatch,
        )));
    }

    let verified_local =
        analyze_local_flat_foldability_with_checkpoint(paper, crease_pattern, &mut checkpoint)
            .map_err(source_reverification_abort)?;
    if &verified_local != local {
        return Err(SourceReverificationAbort::Unknown(inconsistent(
            FlatFoldabilityInputConsistencyIssue::LocalReportGeometryMismatch,
        )));
    }
    match checkpoint() {
        CooperativeAnalysisCheckpoint::Continue => Ok(()),
        CooperativeAnalysisCheckpoint::DeadlineReached => Err(SourceReverificationAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
            },
        )),
        CooperativeAnalysisCheckpoint::Cancelled => Err(SourceReverificationAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled,
        )),
    }
}

const fn source_reverification_abort(abort: CooperativeAnalysisAbort) -> SourceReverificationAbort {
    match abort {
        CooperativeAnalysisAbort::Cancelled => {
            SourceReverificationAbort::Execution(GlobalFlatFoldabilityExecutionError::Cancelled)
        }
        CooperativeAnalysisAbort::DeadlineReached => SourceReverificationAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
            },
        ),
    }
}

struct FixedGlobalFlatFoldabilityObserver {
    control: GlobalFlatFoldabilityExecutionControl,
}

impl GlobalFlatFoldabilityObserver for FixedGlobalFlatFoldabilityObserver {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        match self.control {
            GlobalFlatFoldabilityExecutionControl::Continue => {
                GlobalFlatFoldabilityCheckpoint::Continue
            }
            GlobalFlatFoldabilityExecutionControl::Cancelled => {
                GlobalFlatFoldabilityCheckpoint::Cancelled
            }
        }
    }
}

fn phase_checkpoint<O: GlobalFlatFoldabilityObserver + ?Sized>(
    observer: &mut O,
    phase: GlobalFlatFoldabilityPhase,
    work: GlobalFlatFoldabilityWorkCounts,
    total_work: Option<usize>,
) -> Result<Option<GlobalFlatFoldabilityUnknownReason>, GlobalFlatFoldabilityExecutionError> {
    observer.on_progress(GlobalFlatFoldabilityProgress {
        phase,
        completed_work: completed_work_count(work),
        total_work,
        exact_operations: work.exact_operations,
        overlap_face_pairs: work.overlap_face_pairs,
        overlap_cells: work.overlap_cells,
        constraints: work.constraints,
        search_nodes: work.search_nodes,
    });
    match observer.checkpoint() {
        GlobalFlatFoldabilityCheckpoint::Continue => Ok(None),
        GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
            Ok(Some(GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                phase,
            }))
        }
        GlobalFlatFoldabilityCheckpoint::Cancelled => {
            Err(GlobalFlatFoldabilityExecutionError::Cancelled)
        }
    }
}

fn complete_progress<O: GlobalFlatFoldabilityObserver + ?Sized>(
    observer: &mut O,
    work: GlobalFlatFoldabilityWorkCounts,
) {
    observer.on_progress(GlobalFlatFoldabilityProgress {
        phase: GlobalFlatFoldabilityPhase::Completed,
        completed_work: completed_work_count(work),
        total_work: None,
        exact_operations: work.exact_operations,
        overlap_face_pairs: work.overlap_face_pairs,
        overlap_cells: work.overlap_cells,
        constraints: work.constraints,
        search_nodes: work.search_nodes,
    });
}

const fn completed_work_count(work: GlobalFlatFoldabilityWorkCounts) -> usize {
    work.total_records
        .saturating_add(work.arrangement_segments)
        .saturating_add(work.constraints)
        .saturating_add(work.search_nodes)
}

fn count_work(
    input: &GlobalFlatFoldabilityInput<'_>,
) -> Result<GlobalFlatFoldabilityWorkCounts, GlobalFlatFoldabilityExecutionError> {
    let topology = input.topology;
    let local = input.local_flat_foldability;
    let source_vertex_records = input
        .crease_pattern
        .map_or(0, |pattern| pattern.vertices.len());
    let source_edge_records = input
        .crease_pattern
        .map_or(0, |pattern| pattern.edges.len());
    let paper_boundary_vertex_records =
        input.paper.map_or(0, |paper| paper.boundary_vertices.len());
    let face_boundary_half_edges = topology.faces.iter().try_fold(0_usize, |total, face| {
        total.checked_add(face.outer.half_edges.len()).ok_or(
            GlobalFlatFoldabilityExecutionError::Internal {
                reason: GlobalFlatFoldabilityInternalError::WorkCountOverflow,
            },
        )
    })?;
    let counts = [
        source_vertex_records,
        source_edge_records,
        paper_boundary_vertex_records,
        topology.faces.len(),
        face_boundary_half_edges,
        topology.hinge_adjacency.len(),
        topology.edge_incidence.len(),
        local.vertices.len(),
    ];
    let total_records = counts.into_iter().try_fold(0_usize, |total, count| {
        total
            .checked_add(count)
            .ok_or(GlobalFlatFoldabilityExecutionError::Internal {
                reason: GlobalFlatFoldabilityInternalError::WorkCountOverflow,
            })
    })?;
    Ok(GlobalFlatFoldabilityWorkCounts {
        source_vertex_records,
        source_edge_records,
        paper_boundary_vertex_records,
        face_records: topology.faces.len(),
        face_boundary_half_edges,
        hinge_records: topology.hinge_adjacency.len(),
        edge_incidence_records: topology.edge_incidence.len(),
        local_vertex_records: local.vertices.len(),
        total_records,
        overlap_face_pairs: 0,
        arrangement_segments: 0,
        overlap_cells: 0,
        constraints: 0,
        search_nodes: 0,
        exact_operations: 0,
        exact_values: 0,
        certificate_bytes: 0,
    })
}

fn first_limit_failure(
    work: GlobalFlatFoldabilityWorkCounts,
    limits: GlobalFlatFoldabilityLimits,
) -> Option<GlobalFlatFoldabilityUnknownReason> {
    let candidates = [
        (
            FlatFoldabilityResource::SourceVertices,
            limits.max_source_vertices,
            work.source_vertex_records,
        ),
        (
            FlatFoldabilityResource::SourceEdges,
            limits.max_source_edges,
            work.source_edge_records,
        ),
        (
            FlatFoldabilityResource::PaperBoundaryVertices,
            limits.max_paper_boundary_vertices,
            work.paper_boundary_vertex_records,
        ),
        (
            FlatFoldabilityResource::Faces,
            limits.max_faces,
            work.face_records,
        ),
        (
            FlatFoldabilityResource::FaceBoundaryHalfEdges,
            limits.max_face_boundary_half_edges,
            work.face_boundary_half_edges,
        ),
        (
            FlatFoldabilityResource::Hinges,
            limits.max_hinges,
            work.hinge_records,
        ),
        (
            FlatFoldabilityResource::EdgeIncidenceRecords,
            limits.max_edge_incidence_records,
            work.edge_incidence_records,
        ),
        (
            FlatFoldabilityResource::LocalVertices,
            limits.max_local_vertices,
            work.local_vertex_records,
        ),
        (
            FlatFoldabilityResource::TotalRecords,
            limits.max_total_records,
            work.total_records,
        ),
    ];
    candidates
        .into_iter()
        .find(|(_, limit, observed)| observed > limit)
        .map(|(resource, limit, observed)| {
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource,
                limit,
                observed,
            }
        })
}

fn validate_topology(
    topology: &TopologySnapshot,
) -> Result<Vec<LayerFace>, GlobalFlatFoldabilityUnknownReason> {
    let mut face_ids = HashSet::with_capacity(topology.faces.len());
    let mut face_keys = HashSet::with_capacity(topology.faces.len());
    let mut keys_by_id = HashMap::with_capacity(topology.faces.len());
    let mut canonical_faces = Vec::with_capacity(topology.faces.len());
    let mut face_records = topology.faces.iter().collect::<Vec<_>>();
    face_records.sort_by_key(|face| (face.id.canonical_bytes(), face.key));
    for face in face_records {
        if !face_ids.insert(face.id) {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::DuplicateFaceId { face: face.id },
            ));
        }
        if !face_keys.insert(face.key) {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::DuplicateFaceKey { face_key: face.key },
            ));
        }
        keys_by_id.insert(face.id, face.key);
        canonical_faces.push(LayerFace {
            face_id: face.id,
            face_key: face.key,
        });
    }
    canonical_faces.sort_by_key(|face| (face.face_key, face.face_id.canonical_bytes()));

    let mut incidence_edges = HashSet::with_capacity(topology.edge_incidence.len());
    let mut incidence_hinges = HashMap::new();
    let mut incidence_records = topology.edge_incidence.iter().collect::<Vec<_>>();
    incidence_records.sort_by_key(|(edge, _)| edge.canonical_bytes());
    for (edge, incidence) in incidence_records {
        if !incidence_edges.insert(*edge) {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::DuplicateIncidenceEdge { edge: *edge },
            ));
        }
        match *incidence {
            EdgeIncidence::Boundary { material } => {
                ensure_face_exists(&keys_by_id, *edge, material, false)?;
            }
            EdgeIncidence::Hinge {
                left,
                right,
                assignment,
            } => {
                ensure_face_exists(&keys_by_id, *edge, left, false)?;
                ensure_face_exists(&keys_by_id, *edge, right, false)?;
                if left == right {
                    return Err(inconsistent(
                        FlatFoldabilityInputConsistencyIssue::SelfHinge {
                            edge: *edge,
                            face: left,
                        },
                    ));
                }
                incidence_hinges.insert(*edge, (left, right, assignment));
            }
            EdgeIncidence::Cut { left, right } => {
                ensure_face_exists(&keys_by_id, *edge, left, false)?;
                ensure_face_exists(&keys_by_id, *edge, right, false)?;
            }
            EdgeIncidence::AuxiliaryIgnored => {}
        }
    }

    let mut adjacency_edges = HashSet::with_capacity(topology.hinge_adjacency.len());
    let mut hinge_records = topology.hinge_adjacency.iter().collect::<Vec<_>>();
    hinge_records.sort_by_key(|hinge| hinge.edge.canonical_bytes());
    for hinge in hinge_records {
        if !adjacency_edges.insert(hinge.edge) {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::DuplicateHingeEdge { edge: hinge.edge },
            ));
        }
        ensure_face_exists(&keys_by_id, hinge.edge, hinge.first, true)?;
        ensure_face_exists(&keys_by_id, hinge.edge, hinge.second, true)?;
        if hinge.first == hinge.second {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::SelfHinge {
                    edge: hinge.edge,
                    face: hinge.first,
                },
            ));
        }
        let first_key = keys_by_id.get(&hinge.first).copied().ok_or_else(|| {
            inconsistent(FlatFoldabilityInputConsistencyIssue::UnknownHingeFace {
                edge: hinge.edge,
                face: hinge.first,
            })
        })?;
        let second_key = keys_by_id.get(&hinge.second).copied().ok_or_else(|| {
            inconsistent(FlatFoldabilityInputConsistencyIssue::UnknownHingeFace {
                edge: hinge.edge,
                face: hinge.second,
            })
        })?;
        if first_key >= second_key {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::NonCanonicalHingeFaces { edge: hinge.edge },
            ));
        }
        let Some((left, right, assignment)) = incidence_hinges.get(&hinge.edge).copied() else {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::HingeIncidenceMissing { edge: hinge.edge },
            ));
        };
        if assignment != hinge.assignment {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::HingeAssignmentMismatch { edge: hinge.edge },
            ));
        }
        let same_faces = (left == hinge.first && right == hinge.second)
            || (left == hinge.second && right == hinge.first);
        if !same_faces {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::HingeFacesMismatch { edge: hinge.edge },
            ));
        }
    }
    if let Some(edge) = incidence_hinges
        .keys()
        .filter(|edge| !adjacency_edges.contains(edge))
        .min_by_key(|edge| edge.canonical_bytes())
        .copied()
    {
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::HingeAdjacencyMissing { edge },
        ));
    }
    Ok(canonical_faces)
}

fn ensure_face_exists(
    keys_by_id: &HashMap<FaceId, FaceKey>,
    edge: EdgeId,
    face: FaceId,
    hinge: bool,
) -> Result<(), GlobalFlatFoldabilityUnknownReason> {
    if keys_by_id.contains_key(&face) {
        return Ok(());
    }
    let issue = if hinge {
        FlatFoldabilityInputConsistencyIssue::UnknownHingeFace { edge, face }
    } else {
        FlatFoldabilityInputConsistencyIssue::UnknownIncidenceFace { edge, face }
    };
    Err(inconsistent(issue))
}

enum LocalReportEvidence {
    Blocked,
    NoViolation,
    Violated(Vec<LocalNecessaryConditionViolation>),
    Indeterminate,
}

fn validate_local_report(
    report: &LocalFlatFoldabilityReport,
) -> Result<LocalReportEvidence, GlobalFlatFoldabilityUnknownReason> {
    if report.model != LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1 {
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::LocalReportStatusMismatch,
        ));
    }
    if report.max_exact_fold_degree != MAX_EXACT_FOLD_DEGREE {
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::UnexpectedLocalFoldDegreeLimit {
                expected: MAX_EXACT_FOLD_DEGREE,
                actual: report.max_exact_fold_degree,
            },
        ));
    }

    let mut vertices = HashSet::with_capacity(report.vertices.len());
    let mut satisfied = 0_usize;
    let mut violated = 0_usize;
    let mut not_applicable = 0_usize;
    let mut indeterminate = 0_usize;
    let mut violations = Vec::new();
    let mut vertex_records = report.vertices.iter().collect::<Vec<_>>();
    vertex_records.sort_by_key(|vertex| vertex.vertex.canonical_bytes());
    for vertex in vertex_records {
        if !vertices.insert(vertex.vertex) {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::DuplicateLocalVertex {
                    vertex: vertex.vertex,
                },
            ));
        }
        if vertex
            .mountain_count
            .checked_add(vertex.valley_count)
            .is_none_or(|count| count != vertex.fold_degree)
        {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::LocalVertexCountsMismatch {
                    vertex: vertex.vertex,
                },
            ));
        }
        let valid_verdict = match vertex.verdict {
            LocalVertexFoldabilityVerdict::NotApplicable => {
                not_applicable += 1;
                matches!(
                    vertex.reason,
                    Some(
                        LocalFoldabilityReason::PaperBoundary
                            | LocalFoldabilityReason::CutIncident
                            | LocalFoldabilityReason::NoIncidentFoldEdges
                    )
                ) && vertex.kawasaki == LocalFoldabilityConditionStatus::NotApplicable
                    && vertex.maekawa == LocalFoldabilityConditionStatus::NotApplicable
            }
            LocalVertexFoldabilityVerdict::Satisfied => {
                satisfied += 1;
                vertex.reason.is_none()
                    && vertex.kawasaki == LocalFoldabilityConditionStatus::Satisfied
                    && vertex.maekawa == LocalFoldabilityConditionStatus::Satisfied
            }
            LocalVertexFoldabilityVerdict::Violated => {
                violated += 1;
                let kawasaki_violated =
                    vertex.kawasaki == LocalFoldabilityConditionStatus::Violated;
                let maekawa_violated = vertex.maekawa == LocalFoldabilityConditionStatus::Violated;
                if kawasaki_violated || maekawa_violated {
                    violations.push(LocalNecessaryConditionViolation {
                        vertex: vertex.vertex,
                        kawasaki_violated,
                        maekawa_violated,
                    });
                }
                vertex.reason.is_none() && (kawasaki_violated || maekawa_violated)
            }
            LocalVertexFoldabilityVerdict::Indeterminate => {
                indeterminate += 1;
                vertex.reason == Some(LocalFoldabilityReason::FoldDegreeLimit)
                    && vertex.kawasaki == LocalFoldabilityConditionStatus::Indeterminate
                    && vertex.maekawa == LocalFoldabilityConditionStatus::Satisfied
            }
        };
        if !valid_verdict {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::LocalVertexVerdictMismatch {
                    vertex: vertex.vertex,
                },
            ));
        }
    }

    let applicable = satisfied
        .checked_add(violated)
        .and_then(|count| count.checked_add(indeterminate))
        .ok_or_else(|| {
            inconsistent(FlatFoldabilityInputConsistencyIssue::LocalReportCountsMismatch)
        })?;
    if report.total_vertices != report.vertices.len()
        || report.applicable_vertices != applicable
        || report.satisfied_vertices != satisfied
        || report.violated_vertices != violated
        || report.not_applicable_vertices != not_applicable
        || report.indeterminate_vertices != indeterminate
    {
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::LocalReportCountsMismatch,
        ));
    }

    if report.status == LocalFlatFoldabilityReportStatus::Blocked {
        if report.total_vertices == 0
            && applicable == 0
            && not_applicable == 0
            && violations.is_empty()
        {
            return Ok(LocalReportEvidence::Blocked);
        }
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::LocalReportStatusMismatch,
        ));
    }
    let expected_status = if violated != 0 {
        LocalFlatFoldabilityReportStatus::Violated
    } else if indeterminate != 0 {
        LocalFlatFoldabilityReportStatus::Indeterminate
    } else if satisfied != 0 {
        LocalFlatFoldabilityReportStatus::NecessaryConditionsSatisfied
    } else {
        LocalFlatFoldabilityReportStatus::NotApplicable
    };
    if report.status != expected_status {
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::LocalReportStatusMismatch,
        ));
    }
    violations.sort_by_key(|violation| violation.vertex.canonical_bytes());
    if !violations.is_empty() {
        Ok(LocalReportEvidence::Violated(violations))
    } else if indeterminate != 0 {
        Ok(LocalReportEvidence::Indeterminate)
    } else {
        Ok(LocalReportEvidence::NoViolation)
    }
}

const fn inconsistent(
    issue: FlatFoldabilityInputConsistencyIssue,
) -> GlobalFlatFoldabilityUnknownReason {
    GlobalFlatFoldabilityUnknownReason::InconsistentInput { issue }
}

const fn unknown(
    provenance: GlobalFlatFoldabilityProvenance,
    work_counts: GlobalFlatFoldabilityWorkCounts,
    reason: GlobalFlatFoldabilityUnknownReason,
) -> GlobalFlatFoldabilityReport {
    GlobalFlatFoldabilityReport {
        provenance,
        work_counts,
        outcome: GlobalFlatFoldabilityOutcome::Unknown { reason },
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, EdgeKind, FaceId, Point2, ProjectId, Vertex, VertexId};
    use ori_topology::{
        BoundaryWalk, Face, FaceAdjacency, FaceExtractionInput, HalfEdgeRef,
        LocalVertexFoldability, TopologySnapshot, analyze_local_flat_foldability,
        extract_faces_strict,
    };
    use serde::de::DeserializeOwned;

    use super::*;

    const REVISION: u64 = 41;

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID fixture")
    }

    fn face(id_suffix: u64, key: u8) -> Face {
        Face {
            id: fixed_id::<FaceId>(id_suffix),
            key: FaceKey([key; 32]),
            outer: BoundaryWalk {
                half_edges: Vec::new(),
                signed_double_area: 2.0,
            },
            holes: Vec::new(),
            seams: Vec::new(),
            area: 1.0,
        }
    }

    fn local_not_applicable(vertex_count: usize) -> LocalFlatFoldabilityReport {
        let vertices = (0..vertex_count)
            .map(|index| LocalVertexFoldability {
                vertex: fixed_id(0x800 + index as u64),
                fold_degree: 0,
                mountain_count: 0,
                valley_count: 0,
                verdict: LocalVertexFoldabilityVerdict::NotApplicable,
                reason: Some(LocalFoldabilityReason::NoIncidentFoldEdges),
                kawasaki: LocalFoldabilityConditionStatus::NotApplicable,
                maekawa: LocalFoldabilityConditionStatus::NotApplicable,
            })
            .collect::<Vec<_>>();
        LocalFlatFoldabilityReport {
            model: LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1,
            max_exact_fold_degree: MAX_EXACT_FOLD_DEGREE,
            status: LocalFlatFoldabilityReportStatus::NotApplicable,
            total_vertices: vertices.len(),
            applicable_vertices: 0,
            satisfied_vertices: 0,
            violated_vertices: 0,
            not_applicable_vertices: vertices.len(),
            indeterminate_vertices: 0,
            vertices,
        }
    }

    fn local_violated() -> LocalFlatFoldabilityReport {
        let vertex = LocalVertexFoldability {
            vertex: fixed_id(0x901),
            fold_degree: 4,
            mountain_count: 2,
            valley_count: 2,
            verdict: LocalVertexFoldabilityVerdict::Violated,
            reason: None,
            kawasaki: LocalFoldabilityConditionStatus::Violated,
            maekawa: LocalFoldabilityConditionStatus::Satisfied,
        };
        LocalFlatFoldabilityReport {
            model: LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1,
            max_exact_fold_degree: MAX_EXACT_FOLD_DEGREE,
            status: LocalFlatFoldabilityReportStatus::Violated,
            total_vertices: 1,
            applicable_vertices: 1,
            satisfied_vertices: 0,
            violated_vertices: 1,
            not_applicable_vertices: 0,
            indeterminate_vertices: 0,
            vertices: vec![vertex],
        }
    }

    fn local_blocked() -> LocalFlatFoldabilityReport {
        LocalFlatFoldabilityReport {
            model: LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1,
            max_exact_fold_degree: MAX_EXACT_FOLD_DEGREE,
            status: LocalFlatFoldabilityReportStatus::Blocked,
            total_vertices: 0,
            applicable_vertices: 0,
            satisfied_vertices: 0,
            violated_vertices: 0,
            not_applicable_vertices: 0,
            indeterminate_vertices: 0,
            vertices: Vec::new(),
        }
    }

    fn local_indeterminate() -> LocalFlatFoldabilityReport {
        let vertex = LocalVertexFoldability {
            vertex: fixed_id(0x902),
            fold_degree: MAX_EXACT_FOLD_DEGREE + 2,
            mountain_count: 130,
            valley_count: 128,
            verdict: LocalVertexFoldabilityVerdict::Indeterminate,
            reason: Some(LocalFoldabilityReason::FoldDegreeLimit),
            kawasaki: LocalFoldabilityConditionStatus::Indeterminate,
            maekawa: LocalFoldabilityConditionStatus::Satisfied,
        };
        LocalFlatFoldabilityReport {
            model: LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1,
            max_exact_fold_degree: MAX_EXACT_FOLD_DEGREE,
            status: LocalFlatFoldabilityReportStatus::Indeterminate,
            total_vertices: 1,
            applicable_vertices: 1,
            satisfied_vertices: 0,
            violated_vertices: 0,
            not_applicable_vertices: 0,
            indeterminate_vertices: 1,
            vertices: vec![vertex],
        }
    }

    fn zero_hinge() -> TopologySnapshot {
        TopologySnapshot {
            material_components: Vec::new(),
            source_revision: REVISION,
            faces: vec![face(0x101, 0x10)],
            edge_incidence: Vec::new(),
            hinge_adjacency: Vec::new(),
        }
    }

    fn one_hinge(assignment: FoldAssignment) -> TopologySnapshot {
        let first = face(0x101, 0x10);
        let second = face(0x102, 0x20);
        let edge = fixed_id(0x301);
        TopologySnapshot {
            material_components: Vec::new(),
            source_revision: REVISION,
            faces: vec![second.clone(), first.clone()],
            edge_incidence: vec![(
                edge,
                EdgeIncidence::Hinge {
                    left: second.id,
                    right: first.id,
                    assignment,
                },
            )],
            hinge_adjacency: vec![FaceAdjacency {
                edge,
                first: first.id,
                second: second.id,
                assignment,
            }],
        }
    }

    fn multiple_hinges() -> TopologySnapshot {
        let first = face(0x101, 0x10);
        let second = face(0x102, 0x20);
        let third = face(0x103, 0x30);
        let first_edge = fixed_id(0x301);
        let second_edge = fixed_id(0x302);
        TopologySnapshot {
            material_components: Vec::new(),
            source_revision: REVISION,
            faces: vec![third.clone(), first.clone(), second.clone()],
            edge_incidence: vec![
                (
                    second_edge,
                    EdgeIncidence::Hinge {
                        left: second.id,
                        right: third.id,
                        assignment: FoldAssignment::Valley,
                    },
                ),
                (
                    first_edge,
                    EdgeIncidence::Hinge {
                        left: first.id,
                        right: second.id,
                        assignment: FoldAssignment::Mountain,
                    },
                ),
            ],
            hinge_adjacency: vec![
                FaceAdjacency {
                    edge: second_edge,
                    first: second.id,
                    second: third.id,
                    assignment: FoldAssignment::Valley,
                },
                FaceAdjacency {
                    edge: first_edge,
                    first: first.id,
                    second: second.id,
                    assignment: FoldAssignment::Mountain,
                },
            ],
        }
    }

    fn three_panel_accordion() -> (Paper, CreasePattern, TopologySnapshot) {
        let vertices = (0..8)
            .map(|index| fixed_id::<VertexId>(0x100 + index))
            .collect::<Vec<_>>();
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(6.0, 2.0),
            Point2::new(4.0, 2.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 2.0),
        ];
        let vertex_records = vertices
            .iter()
            .copied()
            .zip(positions)
            .map(|(id, position)| Vertex { id, position })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: fixed_id(0x200 + index as u64),
                start: vertices[index],
                end: vertices[(index + 1) % vertices.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: fixed_id(0x301),
            start: vertices[1],
            end: vertices[6],
            kind: EdgeKind::Mountain,
        });
        edges.push(Edge {
            id: fixed_id(0x302),
            start: vertices[2],
            end: vertices[5],
            kind: EdgeKind::Valley,
        });
        let paper = Paper {
            boundary_vertices: vertices,
            ..Paper::default()
        };
        let pattern = CreasePattern {
            vertices: vertex_records,
            edges,
        };
        let topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(1),
            source_revision: REVISION,
            paper: &paper,
            pattern: &pattern,
        })
        .expect("three-panel accordion topology");
        (paper, pattern, topology)
    }

    fn centered_single_hinge_square() -> (
        Paper,
        CreasePattern,
        TopologySnapshot,
        LocalFlatFoldabilityReport,
    ) {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(200.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 400.0),
            Point2::new(200.0, 400.0),
            Point2::new(0.0, 400.0),
        ];
        let vertices = positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| Vertex {
                id: fixed_id(0xa00 + index as u64),
                position,
            })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: fixed_id(0xb00 + index as u64),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: fixed_id(0xc00),
            start: vertices[1].id,
            end: vertices[4].id,
            kind: EdgeKind::Mountain,
        });
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        let pattern = CreasePattern { vertices, edges };
        let topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(2),
            source_revision: REVISION,
            paper: &paper,
            pattern: &pattern,
        })
        .expect("centered single-hinge square topology");
        let local = analyze_local_flat_foldability(&paper, &pattern);
        (paper, pattern, topology, local)
    }

    struct DeadlineAtFacewise {
        phase: GlobalFlatFoldabilityPhase,
    }

    impl GlobalFlatFoldabilityObserver for DeadlineAtFacewise {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            if self.phase >= GlobalFlatFoldabilityPhase::BuildingFlatEmbedding {
                GlobalFlatFoldabilityCheckpoint::DeadlineReached
            } else {
                GlobalFlatFoldabilityCheckpoint::Continue
            }
        }

        fn on_progress(&mut self, progress: GlobalFlatFoldabilityProgress) {
            self.phase = progress.phase;
        }
    }

    fn analyze(
        topology: &TopologySnapshot,
        local: &LocalFlatFoldabilityReport,
        limits: GlobalFlatFoldabilityLimits,
    ) -> GlobalFlatFoldabilityReport {
        analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current(topology, local),
            limits,
        )
        .expect("fixture analysis executes")
    }

    #[test]
    fn versioned_model_ids_have_stable_serialized_names() {
        assert_eq!(
            serde_json::to_string(&GLOBAL_FLAT_FOLDABILITY_MODEL_ID).expect("model ID serializes"),
            "\"convex_faces_facewise_v1\""
        );
        assert_eq!(
            serde_json::to_string(&LAYER_ORDER_MODEL_ID).expect("layer model ID serializes"),
            "\"facewise_layer_order_v1\""
        );
    }

    #[test]
    fn no_geometry_fast_path_is_unknown_and_never_returns_a_layer_order() {
        let topology = zero_hinge();
        let local = local_not_applicable(0);
        let report = analyze(&topology, &local, GlobalFlatFoldabilityLimits::default());

        assert_eq!(report.verdict(), GlobalFlatFoldabilityVerdict::Unknown);
        assert!(report.layer_order().is_none());
        assert!(matches!(
            report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));
    }

    #[test]
    fn no_geometry_single_hinge_is_unknown_and_never_returns_partial_layer_state() {
        let local = local_not_applicable(0);
        let mountain = one_hinge(FoldAssignment::Mountain);
        let mountain_report = analyze(&mountain, &local, GlobalFlatFoldabilityLimits::default());
        assert_eq!(
            mountain_report.verdict(),
            GlobalFlatFoldabilityVerdict::Unknown
        );
        assert!(mountain_report.layer_order().is_none());
        assert!(matches!(
            mountain_report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));
    }

    #[test]
    fn input_storage_order_does_not_change_the_report() {
        let mut first_topology = one_hinge(FoldAssignment::Mountain);
        let mut first_local = local_not_applicable(2);
        let expected = analyze(
            &first_topology,
            &first_local,
            GlobalFlatFoldabilityLimits::default(),
        );

        first_topology.faces.reverse();
        first_topology.edge_incidence.reverse();
        first_topology.hinge_adjacency.reverse();
        first_local.vertices.reverse();
        let actual = analyze(
            &first_topology,
            &first_local,
            GlobalFlatFoldabilityLimits::default(),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn public_geometry_api_proves_a_three_panel_accordion() {
        let (paper, pattern, topology) = three_panel_accordion();
        let local = analyze_local_flat_foldability(&paper, &pattern);
        let report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(1),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("facewise analysis executes");

        assert_eq!(report.verdict(), GlobalFlatFoldabilityVerdict::Possible);
        let layer_order = report.layer_order().expect("possible has layer order");
        assert_eq!(layer_order.material_faces.len(), 3);
        assert_eq!(layer_order.face_pair_orders.len(), 3);
        assert!(layer_order.proof_summary.is_some());
    }

    #[test]
    fn centered_single_hinge_geometry_certificate_has_two_ply_overlap() {
        let (paper, pattern, topology, local) = centered_single_hinge_square();
        let report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("single-hinge geometry analysis executes");

        assert_eq!(report.verdict(), GlobalFlatFoldabilityVerdict::Possible);
        let layer_order = report.layer_order().expect("possible has layer order");
        assert_eq!(layer_order.material_faces.len(), 2);
        assert_eq!(layer_order.folded_faces.len(), 2);
        assert_eq!(
            layer_order
                .proof_summary
                .expect("geometry certificate summary")
                .maximum_ply,
            2
        );
    }

    #[test]
    fn public_geometry_api_keeps_deadline_limit_and_cancel_distinct() {
        let (paper, pattern, topology) = three_panel_accordion();
        let local = analyze_local_flat_foldability(&paper, &pattern);
        let input = || {
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(1),
                &paper,
                &pattern,
                &topology,
                &local,
            )
        };

        let mut deadline = DeadlineAtFacewise {
            phase: GlobalFlatFoldabilityPhase::Capturing,
        };
        let timed_out = analyze_global_flat_foldability_with_observer(
            input(),
            GlobalFlatFoldabilityLimits::default(),
            &mut deadline,
        )
        .expect("deadline is a mathematical unknown");
        assert!(matches!(
            timed_out.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::BuildingFlatEmbedding
                }
            }
        ));

        let limited = analyze_global_flat_foldability(
            input(),
            GlobalFlatFoldabilityLimits {
                max_overlap_face_pairs: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("resource limit is a mathematical unknown");
        assert!(matches!(
            limited.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::OverlapFacePairs,
                    limit: 0,
                    observed: 1,
                }
            }
        ));

        let cancelled = analyze_global_flat_foldability_with_control(
            input(),
            GlobalFlatFoldabilityLimits::default(),
            GlobalFlatFoldabilityExecutionControl::Cancelled,
        );
        assert_eq!(
            cancelled,
            Err(GlobalFlatFoldabilityExecutionError::Cancelled)
        );
    }

    #[test]
    fn geometry_reverification_rejects_forged_local_reports_in_both_directions() {
        let (paper, pattern, topology, actual_local) = centered_single_hinge_square();
        let forged_violation = local_violated();
        let forged = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &forged_violation,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("mismatched local evidence is a mathematical unknown");
        assert!(matches!(
            forged.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::InconsistentInput {
                    issue: FlatFoldabilityInputConsistencyIssue::LocalReportGeometryMismatch
                }
            }
        ));

        let empty = local_not_applicable(0);
        assert_ne!(
            empty, actual_local,
            "fixture must exercise the inverse mismatch"
        );
        let missing = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &empty,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("missing local evidence is a mathematical unknown");
        assert!(matches!(
            missing.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::InconsistentInput {
                    issue: FlatFoldabilityInputConsistencyIssue::LocalReportGeometryMismatch
                }
            }
        ));
    }

    #[test]
    fn geometry_reverification_rejects_stale_topology_and_wrong_identity() {
        let (paper, mut pattern, topology, original_local) = centered_single_hinge_square();
        let wrong_identity = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(3),
                &paper,
                &pattern,
                &topology,
                &original_local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("wrong identity is a mathematical unknown");
        assert!(matches!(
            wrong_identity.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::InconsistentInput {
                    issue: FlatFoldabilityInputConsistencyIssue::TopologyGeometryMismatch
                }
            }
        ));

        let mut wrong_paper = paper.clone();
        wrong_paper.boundary_vertices.swap(1, 2);
        let wrong_paper_local = analyze_local_flat_foldability(&wrong_paper, &pattern);
        let wrong_boundary = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &wrong_paper,
                &pattern,
                &topology,
                &wrong_paper_local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("wrong paper boundary is a mathematical unknown");
        assert!(matches!(
            wrong_boundary.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::InconsistentInput {
                    issue: FlatFoldabilityInputConsistencyIssue::TopologyGeometryMismatch
                }
            }
        ));

        pattern.edges.push(Edge {
            id: fixed_id(0xc01),
            start: pattern.vertices[0].id,
            end: pattern.vertices[3].id,
            kind: EdgeKind::Valley,
        });
        let local = analyze_local_flat_foldability(&paper, &pattern);

        let report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("stale topology is a mathematical unknown");
        assert!(matches!(
            report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::InconsistentInput {
                    issue: FlatFoldabilityInputConsistencyIssue::TopologyGeometryMismatch
                }
            }
        ));
        assert!(report.layer_order().is_none());
    }

    #[test]
    fn every_source_record_is_counted_before_facewise_allocation() {
        let (paper, pattern, topology, local) = centered_single_hinge_square();
        let base = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("baseline analysis");
        let counts = base.work_counts;
        assert_eq!(counts.source_vertex_records, pattern.vertices.len());
        assert_eq!(counts.source_edge_records, pattern.edges.len());
        assert_eq!(
            counts.paper_boundary_vertex_records,
            paper.boundary_vertices.len()
        );

        let exact = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_source_vertices: pattern.vertices.len(),
                max_source_edges: pattern.edges.len(),
                max_paper_boundary_vertices: paper.boundary_vertices.len(),
                max_total_records: counts.total_records,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("source limit equality is admitted");
        assert_eq!(exact.verdict(), GlobalFlatFoldabilityVerdict::Possible);

        let limited = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_source_vertices: pattern.vertices.len() - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("source limit is a mathematical unknown");
        assert!(matches!(
            limited.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SourceVertices,
                    limit,
                    observed,
                }
            } if limit + 1 == observed && observed == pattern.vertices.len()
        ));

        let edge_limited = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_source_edges: pattern.edges.len() - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("source-edge limit is a mathematical unknown");
        assert!(matches!(
            edge_limited.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SourceEdges,
                    limit,
                    observed,
                }
            } if limit + 1 == observed && observed == pattern.edges.len()
        ));

        let boundary_limited = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_paper_boundary_vertices: paper.boundary_vertices.len() - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("paper-boundary limit is a mathematical unknown");
        assert!(matches!(
            boundary_limited.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::PaperBoundaryVertices,
                    limit,
                    observed,
                }
            } if limit + 1 == observed && observed == paper.boundary_vertices.len()
        ));

        let total_limited = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_total_records: counts.total_records - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("total source-inclusive limit is a mathematical unknown");
        assert!(matches!(
            total_limited.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::TotalRecords,
                    limit,
                    observed,
                }
            } if limit + 1 == observed && observed == counts.total_records
        ));
    }

    #[test]
    fn isolated_vertices_and_auxiliary_edges_are_not_hidden_from_source_limits() {
        let (paper, pattern, baseline_topology, baseline_local) = centered_single_hinge_square();
        let baseline = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &baseline_topology,
                &baseline_local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("baseline analysis");

        let mut extended = pattern.clone();
        let first = Vertex {
            id: fixed_id(0xd01),
            position: Point2::new(100.0, 100.0),
        };
        let second = Vertex {
            id: fixed_id(0xd02),
            position: Point2::new(101.0, 100.0),
        };
        extended.vertices.extend([first.clone(), second.clone()]);
        extended.edges.push(Edge {
            id: fixed_id(0xd03),
            start: first.id,
            end: second.id,
            kind: EdgeKind::Auxiliary,
        });
        let topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(2),
            source_revision: REVISION,
            paper: &paper,
            pattern: &extended,
        })
        .expect("auxiliary draft geometry remains topology-safe");
        let local = analyze_local_flat_foldability(&paper, &extended);
        let extended_report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &extended,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("extended analysis");
        assert_eq!(
            extended_report.work_counts.source_vertex_records,
            baseline.work_counts.source_vertex_records + 2
        );
        assert_eq!(
            extended_report.work_counts.source_edge_records,
            baseline.work_counts.source_edge_records + 1
        );

        let limited = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &extended,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_total_records: baseline.work_counts.total_records,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("source-inclusive total limit is a mathematical unknown");
        assert!(matches!(
            limited.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::TotalRecords,
                    limit,
                    observed,
                }
            } if limit == baseline.work_counts.total_records
                && observed == extended_report.work_counts.total_records
                && observed > limit
        ));
    }

    #[test]
    fn layer_order_provenance_rejects_identity_and_same_revision_content_aba() {
        let (paper, pattern, topology, local) = centered_single_hinge_square();
        let report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("baseline analysis");
        let order = report.layer_order().expect("geometry-backed layer order");
        assert!(order.is_current_for(&report.provenance));

        let mut wrong_identity = report.provenance;
        wrong_identity.identity_namespace = Some(fixed_id::<ProjectId>(3));
        assert!(!order.is_current_for(&wrong_identity));

        let mut wrong_revision = report.provenance;
        wrong_revision.source_revision += 1;
        assert!(!order.is_current_for(&wrong_revision));

        let mut changed_pattern = pattern.clone();
        changed_pattern.vertices[0].position.x = -0.0;
        let changed_content = GlobalFlatFoldabilityProvenance::for_geometry(
            fixed_id::<ProjectId>(2),
            REVISION,
            &paper,
            &changed_pattern,
        );
        assert_ne!(
            report.provenance.source_fingerprint,
            changed_content.source_fingerprint
        );
        assert!(!order.is_current_for(&changed_content));
    }

    #[test]
    fn cancellation_and_deadline_are_observed_during_source_reverification() {
        struct AbortDuringReverification {
            calls: usize,
            checkpoint: GlobalFlatFoldabilityCheckpoint,
            phases: Vec<GlobalFlatFoldabilityPhase>,
        }
        impl GlobalFlatFoldabilityObserver for AbortDuringReverification {
            fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
                self.calls += 1;
                if self.calls >= 15 {
                    self.checkpoint
                } else {
                    GlobalFlatFoldabilityCheckpoint::Continue
                }
            }

            fn on_progress(&mut self, progress: GlobalFlatFoldabilityProgress) {
                assert!(
                    self.phases
                        .last()
                        .is_none_or(|previous| *previous <= progress.phase),
                    "progress phases must remain monotonic during reverification"
                );
                self.phases.push(progress.phase);
            }
        }

        let (paper, pattern, topology, local) = centered_single_hinge_square();
        let input = || {
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(2),
                &paper,
                &pattern,
                &topology,
                &local,
            )
        };
        let mut cancel = AbortDuringReverification {
            calls: 0,
            checkpoint: GlobalFlatFoldabilityCheckpoint::Cancelled,
            phases: Vec::new(),
        };
        assert_eq!(
            analyze_global_flat_foldability_with_observer(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &mut cancel,
            ),
            Err(GlobalFlatFoldabilityExecutionError::Cancelled)
        );
        assert!(cancel.calls >= 15);
        assert_eq!(
            cancel.phases,
            vec![
                GlobalFlatFoldabilityPhase::Capturing,
                GlobalFlatFoldabilityPhase::ValidatingLocalConditions
            ]
        );

        let mut deadline = AbortDuringReverification {
            calls: 0,
            checkpoint: GlobalFlatFoldabilityCheckpoint::DeadlineReached,
            phases: Vec::new(),
        };
        let timed_out = analyze_global_flat_foldability_with_observer(
            input(),
            GlobalFlatFoldabilityLimits::default(),
            &mut deadline,
        )
        .expect("deadline remains an unknown verdict");
        assert!(matches!(
            timed_out.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions
                }
            }
        ));
        assert!(deadline.calls >= 15);
        assert_eq!(
            deadline.phases,
            vec![
                GlobalFlatFoldabilityPhase::Capturing,
                GlobalFlatFoldabilityPhase::ValidatingLocalConditions
            ]
        );
    }

    #[test]
    fn no_geometry_never_trusts_an_explicit_local_violation() {
        let topology = one_hinge(FoldAssignment::Mountain);
        let report = analyze(
            &topology,
            &local_violated(),
            GlobalFlatFoldabilityLimits::default(),
        );

        assert_eq!(report.verdict(), GlobalFlatFoldabilityVerdict::Unknown);
        assert!(report.layer_order().is_none());
        assert!(matches!(
            report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));
    }

    #[test]
    fn no_geometry_precedes_topology_class_and_local_evidence() {
        let topology = multiple_hinges();
        let report = analyze(
            &topology,
            &local_not_applicable(0),
            GlobalFlatFoldabilityLimits {
                max_hinges: 2,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );

        assert_eq!(report.verdict(), GlobalFlatFoldabilityVerdict::Unknown);
        assert!(matches!(
            report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));

        let report_with_local_counterexample = analyze(
            &topology,
            &local_violated(),
            GlobalFlatFoldabilityLimits {
                max_hinges: 2,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            report_with_local_counterexample.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));
    }

    #[test]
    fn no_geometry_does_not_expose_unverified_local_report_details() {
        let topology = one_hinge(FoldAssignment::Mountain);
        let blocked = analyze(
            &topology,
            &local_blocked(),
            GlobalFlatFoldabilityLimits::default(),
        );
        assert!(matches!(
            blocked.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));

        let indeterminate = analyze(
            &topology,
            &local_indeterminate(),
            GlobalFlatFoldabilityLimits::default(),
        );
        assert!(matches!(
            indeterminate.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));
    }

    #[test]
    fn every_limit_admits_exactly_the_limit_and_rejects_plus_one() {
        let zero = zero_hinge();
        let empty_local = local_not_applicable(0);
        let exact_zero = analyze(
            &zero,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_faces: 1,
                max_face_boundary_half_edges: 0,
                max_hinges: 0,
                max_edge_incidence_records: 0,
                max_local_vertices: 0,
                max_total_records: 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            exact_zero.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));

        let one = one_hinge(FoldAssignment::Mountain);
        let exact_one = analyze(
            &one,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_faces: 2,
                max_face_boundary_half_edges: 0,
                max_hinges: 1,
                max_edge_incidence_records: 1,
                max_local_vertices: 0,
                max_total_records: 4,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            exact_one.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));

        let over_faces = analyze(
            &zero,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_faces: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_faces.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::Faces,
                    limit: 0,
                    observed: 1,
                }
            }
        ));

        let mut boundary_work = zero.clone();
        boundary_work.faces[0].outer.half_edges.push(HalfEdgeRef {
            edge: fixed_id(0x401),
            origin: fixed_id(0x501),
            destination: fixed_id(0x502),
        });
        let over_boundary = analyze(
            &boundary_work,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_face_boundary_half_edges: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_boundary.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::FaceBoundaryHalfEdges,
                    limit: 0,
                    observed: 1,
                }
            }
        ));

        let over_hinge = analyze(
            &one,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_hinges: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_hinge.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::Hinges,
                    limit: 0,
                    observed: 1,
                }
            }
        ));

        let over_incidence = analyze(
            &one,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_edge_incidence_records: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_incidence.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::EdgeIncidenceRecords,
                    limit: 0,
                    observed: 1,
                }
            }
        ));

        let one_local_vertex = local_not_applicable(1);
        let over_local = analyze(
            &zero,
            &one_local_vertex,
            GlobalFlatFoldabilityLimits {
                max_local_vertices: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_local.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::LocalVertices,
                    limit: 0,
                    observed: 1,
                }
            }
        ));

        let over_total = analyze(
            &one,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_total_records: 3,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_total.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::TotalRecords,
                    limit: 3,
                    observed: 4,
                }
            }
        ));
    }

    #[test]
    fn stale_topology_or_local_provenance_is_unknown() {
        let topology = zero_hinge();
        let local = local_not_applicable(0);
        let stale_topology = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput {
                identity_namespace: None,
                source_revision: REVISION + 1,
                local_report_source_revision: REVISION + 1,
                topology: &topology,
                local_flat_foldability: &local,
                paper: None,
                crease_pattern: None,
            },
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("stale input is a mathematical unknown");
        assert!(matches!(
            stale_topology.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::StaleProvenance {
                    artifact: FlatFoldabilityInputArtifact::TopologySnapshot,
                    ..
                }
            }
        ));

        let stale_local = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput {
                identity_namespace: None,
                source_revision: REVISION,
                local_report_source_revision: REVISION - 1,
                topology: &topology,
                local_flat_foldability: &local,
                paper: None,
                crease_pattern: None,
            },
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("stale input is a mathematical unknown");
        assert!(matches!(
            stale_local.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::StaleProvenance {
                    artifact: FlatFoldabilityInputArtifact::LocalFlatFoldabilityReport,
                    ..
                }
            }
        ));
    }

    #[test]
    fn cancellation_is_an_execution_error_not_a_three_value_verdict() {
        let topology = zero_hinge();
        let local = local_not_applicable(0);
        let result = analyze_global_flat_foldability_with_control(
            GlobalFlatFoldabilityInput::current(&topology, &local),
            GlobalFlatFoldabilityLimits::default(),
            GlobalFlatFoldabilityExecutionControl::Cancelled,
        );

        assert_eq!(result, Err(GlobalFlatFoldabilityExecutionError::Cancelled));
    }

    #[test]
    fn malformed_proof_inputs_fail_closed_as_unknown() {
        let mut topology = one_hinge(FoldAssignment::Mountain);
        topology.hinge_adjacency[0].first = topology.hinge_adjacency[0].second;
        let report = analyze(
            &topology,
            &local_not_applicable(0),
            GlobalFlatFoldabilityLimits::default(),
        );

        assert_eq!(report.verdict(), GlobalFlatFoldabilityVerdict::Unknown);
        assert!(report.layer_order().is_none());
    }
}
