#![forbid(unsafe_code)]

//! Deterministic, fail-closed global flat-foldability proofs.
//!
//! The first model proves deterministic layer orders for connected convex
//! material faces using exact flat embeddings, overlap cells, and facewise
//! constraints. Local necessary-condition violations can also disprove an
//! input. Unsupported, stale, over-limit, or incomplete inputs remain
//! `Unknown`.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

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
    AllocationFailed,
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

/// One caller-required order between two trusted material faces.
///
/// The constrained archive-admission API treats every value as untrusted and
/// rebinds both complete [`LayerFace`] records to the freshly reconstructed
/// canonical material registry before using the relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredLayerOrderPair {
    pub lower_face: LayerFace,
    pub upper_face: LayerFace,
}

/// Fail-closed result categories for constrained layer-order reanalysis.
///
/// A required-order contradiction is deliberately not a global
/// flat-foldability verdict: it only means that the supplied requirements
/// cannot authenticate an archived certificate for this project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RequiredLayerOrderError {
    #[error("the project is globally impossible before archived pair requirements are considered")]
    BaseAnalysisImpossible,
    #[error("the constrained analysis remained inconclusive: {reason:?}")]
    Inconclusive {
        reason: GlobalFlatFoldabilityUnknownReason,
    },
    #[error("a required layer-order face is not in the trusted material registry: {face:?}")]
    UnknownFace { face: FaceId },
    #[error("a required layer-order pair orders one face against itself: {face:?}")]
    EqualFace { face: FaceId },
    #[error(
        "a required layer-order pair does not overlap in the trusted flat arrangement: {lower:?} -> {upper:?}"
    )]
    NonOverlappingPair { lower: FaceId, upper: FaceId },
    #[error("a required layer-order pair is duplicated: {lower:?} -> {upper:?}")]
    DuplicatePair { lower: FaceId, upper: FaceId },
    #[error("required layer-order pairs conflict: {first:?} <-> {second:?}")]
    ConflictingPair { first: FaceId, second: FaceId },
    #[error(
        "a required layer-order pair contradicts a trusted fixed assignment: {lower:?} -> {upper:?}"
    )]
    ContradictsTrustedFixedOrder { lower: FaceId, upper: FaceId },
    #[error("the trusted flat constraints cannot satisfy the required layer orders")]
    Unsatisfied,
    #[error("the constrained layer-order certificate did not reverify")]
    CertificateReverificationFailed,
    #[error(transparent)]
    Execution(#[from] GlobalFlatFoldabilityExecutionError),
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

/// Recoverable failures while constructing a retained layer-order snapshot.
///
/// The byte limit covers the snapshot value itself plus every Rust-owned
/// `Vec` allocation reachable from it. Allocator metadata and transient stack
/// storage are outside this deterministic accounting boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LayerOrderSnapshotCloneErrorV1 {
    #[error("layer-order retained-byte accounting overflowed")]
    SizeOverflow,
    #[error(
        "layer-order snapshot would retain {observed} bytes, exceeding the {maximum}-byte limit"
    )]
    ByteLimitExceeded { observed: usize, maximum: usize },
    #[error("layer-order snapshot allocation failed")]
    AllocationFailed,
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

    /// Returns the bytes retained by this value and all of its owned vectors.
    ///
    /// Vector capacity, rather than length, is measured so spare allocation is
    /// never hidden from a retention budget.
    #[must_use]
    pub fn checked_deep_retained_bytes_v1(&self) -> Option<usize> {
        checked_layer_order_snapshot_actual_bytes_v1(self)
    }

    /// Fallibly clones this snapshot without an unbounded intermediate clone.
    ///
    /// The projected length-based size is checked before allocation. The
    /// capacity returned by every fallible reserve is charged immediately, so
    /// allocator over-capacity cannot accumulate across nested regions. The
    /// constructed value is also remeasured before it is returned.
    pub fn try_clone_with_retained_byte_limit_v1(
        &self,
        maximum: usize,
    ) -> Result<Self, LayerOrderSnapshotCloneErrorV1> {
        self.try_clone_filtered_with_retained_byte_limit_v1(None, maximum)
    }

    /// Returns the projected retained bytes of the face-restricted clone.
    ///
    /// This is the exact length-based preflight used by
    /// [`Self::try_restrict_to_faces_with_retained_byte_limit_v1`]. The final
    /// clone is still remeasured because an allocator may expose a capacity
    /// larger than the requested length.
    #[must_use]
    pub fn checked_restricted_deep_retained_bytes_v1(&self, faces: &[FaceId]) -> Option<usize> {
        checked_layer_order_snapshot_projected_bytes_v1(self, Some(faces))
    }

    /// Fallibly constructs the portion of this certificate retained by
    /// `faces`, without first cloning the complete source.
    ///
    /// Cell geometry is retained only for cells with at least one selected
    /// bottom-to-top record. Pair records require both endpoint faces.
    /// Reference-face fallback matches the former clone-then-retain behavior.
    pub fn try_restrict_to_faces_with_retained_byte_limit_v1(
        &self,
        faces: &[FaceId],
        maximum: usize,
    ) -> Result<Self, LayerOrderSnapshotCloneErrorV1> {
        self.try_clone_filtered_with_retained_byte_limit_v1(Some(faces), maximum)
    }

    fn try_clone_filtered_with_retained_byte_limit_v1(
        &self,
        faces: Option<&[FaceId]>,
        maximum: usize,
    ) -> Result<Self, LayerOrderSnapshotCloneErrorV1> {
        let projected = checked_layer_order_snapshot_projected_bytes_v1(self, faces)
            .ok_or(LayerOrderSnapshotCloneErrorV1::SizeOverflow)?;
        check_layer_order_snapshot_byte_limit_v1(projected, maximum)?;

        let mut budget = LayerOrderSnapshotCloneBudgetV1::new(maximum)?;
        let cloned = try_clone_layer_order_snapshot_filtered_v1(self, faces, &mut budget)?;
        let observed = cloned
            .checked_deep_retained_bytes_v1()
            .ok_or(LayerOrderSnapshotCloneErrorV1::SizeOverflow)?;
        if observed != budget.observed {
            return Err(LayerOrderSnapshotCloneErrorV1::SizeOverflow);
        }
        check_layer_order_snapshot_byte_limit_v1(observed, maximum)?;
        Ok(cloned)
    }
}

fn check_layer_order_snapshot_byte_limit_v1(
    observed: usize,
    maximum: usize,
) -> Result<(), LayerOrderSnapshotCloneErrorV1> {
    if observed > maximum {
        return Err(LayerOrderSnapshotCloneErrorV1::ByteLimitExceeded { observed, maximum });
    }
    Ok(())
}

fn checked_add_vec_allocation_v1<T>(total: &mut usize, elements: usize) -> Option<()> {
    let bytes = std::mem::size_of::<T>().checked_mul(elements)?;
    *total = total.checked_add(bytes)?;
    Some(())
}

fn checked_add_exact_rational_actual_bytes_v1(
    total: &mut usize,
    value: &ExactRationalValue,
) -> Option<()> {
    checked_add_vec_allocation_v1::<u8>(total, value.numerator_magnitude_be.capacity())?;
    checked_add_vec_allocation_v1::<u8>(total, value.denominator_be.capacity())
}

fn checked_add_exact_rational_projected_bytes_v1(
    total: &mut usize,
    value: &ExactRationalValue,
) -> Option<()> {
    checked_add_vec_allocation_v1::<u8>(total, value.numerator_magnitude_be.len())?;
    checked_add_vec_allocation_v1::<u8>(total, value.denominator_be.len())
}

fn checked_add_exact_point_actual_bytes_v1(
    total: &mut usize,
    value: &ExactPointValue,
) -> Option<()> {
    checked_add_exact_rational_actual_bytes_v1(total, &value.x)?;
    checked_add_exact_rational_actual_bytes_v1(total, &value.y)
}

fn checked_add_exact_point_projected_bytes_v1(
    total: &mut usize,
    value: &ExactPointValue,
) -> Option<()> {
    checked_add_exact_rational_projected_bytes_v1(total, &value.x)?;
    checked_add_exact_rational_projected_bytes_v1(total, &value.y)
}

fn checked_add_exact_transform_actual_bytes_v1(
    total: &mut usize,
    value: &ExactAffineTransform,
) -> Option<()> {
    for coefficient in [
        &value.m00, &value.m01, &value.m10, &value.m11, &value.tx, &value.ty,
    ] {
        checked_add_exact_rational_actual_bytes_v1(total, coefficient)?;
    }
    Some(())
}

fn checked_add_exact_transform_projected_bytes_v1(
    total: &mut usize,
    value: &ExactAffineTransform,
) -> Option<()> {
    for coefficient in [
        &value.m00, &value.m01, &value.m10, &value.m11, &value.tx, &value.ty,
    ] {
        checked_add_exact_rational_projected_bytes_v1(total, coefficient)?;
    }
    Some(())
}

fn checked_layer_order_snapshot_actual_bytes_v1(snapshot: &LayerOrderSnapshot) -> Option<usize> {
    let mut total = std::mem::size_of::<LayerOrderSnapshot>();
    checked_add_vec_allocation_v1::<LayerFace>(&mut total, snapshot.material_faces.capacity())?;
    if let Some(global) = &snapshot.global_bottom_to_top {
        checked_add_vec_allocation_v1::<LayerFace>(&mut total, global.capacity())?;
    }
    checked_add_vec_allocation_v1::<FoldedFaceSnapshot>(
        &mut total,
        snapshot.folded_faces.capacity(),
    )?;
    for folded in &snapshot.folded_faces {
        checked_add_exact_transform_actual_bytes_v1(&mut total, &folded.source_to_flat)?;
    }
    checked_add_vec_allocation_v1::<OverlapCellSnapshot>(
        &mut total,
        snapshot.overlap_cells.capacity(),
    )?;
    for cell in &snapshot.overlap_cells {
        checked_add_vec_allocation_v1::<ExactPointValue>(
            &mut total,
            cell.exact_boundary.capacity(),
        )?;
        for point in &cell.exact_boundary {
            checked_add_exact_point_actual_bytes_v1(&mut total, point)?;
        }
        checked_add_vec_allocation_v1::<LayerFace>(&mut total, cell.covering_faces.capacity())?;
        checked_add_vec_allocation_v1::<FaceId>(&mut total, cell.bottom_to_top_faces.capacity())?;
    }
    checked_add_vec_allocation_v1::<FacePairOrderSnapshot>(
        &mut total,
        snapshot.face_pair_orders.capacity(),
    )?;
    for pair in &snapshot.face_pair_orders {
        checked_add_vec_allocation_v1::<OverlapCellKey>(
            &mut total,
            pair.supporting_cells.capacity(),
        )?;
    }
    Some(total)
}

fn face_is_retained_v1(faces: Option<&[FaceId]>, face: FaceId) -> bool {
    faces.is_none_or(|selected| selected.contains(&face))
}

fn cell_is_retained_v1(faces: Option<&[FaceId]>, cell: &OverlapCellSnapshot) -> bool {
    faces.is_none()
        || cell
            .bottom_to_top_faces
            .iter()
            .copied()
            .any(|face| face_is_retained_v1(faces, face))
}

fn checked_layer_order_snapshot_projected_bytes_v1(
    snapshot: &LayerOrderSnapshot,
    faces: Option<&[FaceId]>,
) -> Option<usize> {
    let mut total = std::mem::size_of::<LayerOrderSnapshot>();

    let material_face_count = snapshot
        .material_faces
        .iter()
        .filter(|face| face_is_retained_v1(faces, face.face_id))
        .count();
    checked_add_vec_allocation_v1::<LayerFace>(&mut total, material_face_count)?;

    if let Some(global) = &snapshot.global_bottom_to_top {
        let global_count = global
            .iter()
            .filter(|face| face_is_retained_v1(faces, face.face_id))
            .count();
        checked_add_vec_allocation_v1::<LayerFace>(&mut total, global_count)?;
    }

    let folded_face_count = snapshot
        .folded_faces
        .iter()
        .filter(|face| face_is_retained_v1(faces, face.face.face_id))
        .count();
    checked_add_vec_allocation_v1::<FoldedFaceSnapshot>(&mut total, folded_face_count)?;
    for folded in snapshot
        .folded_faces
        .iter()
        .filter(|face| face_is_retained_v1(faces, face.face.face_id))
    {
        checked_add_exact_transform_projected_bytes_v1(&mut total, &folded.source_to_flat)?;
    }

    let overlap_cell_count = snapshot
        .overlap_cells
        .iter()
        .filter(|cell| cell_is_retained_v1(faces, cell))
        .count();
    checked_add_vec_allocation_v1::<OverlapCellSnapshot>(&mut total, overlap_cell_count)?;
    for cell in snapshot
        .overlap_cells
        .iter()
        .filter(|cell| cell_is_retained_v1(faces, cell))
    {
        checked_add_vec_allocation_v1::<ExactPointValue>(&mut total, cell.exact_boundary.len())?;
        for point in &cell.exact_boundary {
            checked_add_exact_point_projected_bytes_v1(&mut total, point)?;
        }
        let covering_face_count = cell
            .covering_faces
            .iter()
            .filter(|face| face_is_retained_v1(faces, face.face_id))
            .count();
        checked_add_vec_allocation_v1::<LayerFace>(&mut total, covering_face_count)?;
        let layer_count = cell
            .bottom_to_top_faces
            .iter()
            .filter(|face| face_is_retained_v1(faces, **face))
            .count();
        checked_add_vec_allocation_v1::<FaceId>(&mut total, layer_count)?;
    }

    let pair_count = snapshot
        .face_pair_orders
        .iter()
        .filter(|pair| {
            face_is_retained_v1(faces, pair.lower_face.face_id)
                && face_is_retained_v1(faces, pair.upper_face.face_id)
        })
        .count();
    checked_add_vec_allocation_v1::<FacePairOrderSnapshot>(&mut total, pair_count)?;
    for pair in snapshot.face_pair_orders.iter().filter(|pair| {
        face_is_retained_v1(faces, pair.lower_face.face_id)
            && face_is_retained_v1(faces, pair.upper_face.face_id)
    }) {
        checked_add_vec_allocation_v1::<OverlapCellKey>(&mut total, pair.supporting_cells.len())?;
    }

    Some(total)
}

struct LayerOrderSnapshotCloneBudgetV1 {
    observed: usize,
    maximum: usize,
}

impl LayerOrderSnapshotCloneBudgetV1 {
    fn new(maximum: usize) -> Result<Self, LayerOrderSnapshotCloneErrorV1> {
        let observed = std::mem::size_of::<LayerOrderSnapshot>();
        check_layer_order_snapshot_byte_limit_v1(observed, maximum)?;
        Ok(Self { observed, maximum })
    }

    fn try_vec_with_exact_capacity<T>(
        &mut self,
        requested_capacity: usize,
    ) -> Result<Vec<T>, LayerOrderSnapshotCloneErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(requested_capacity)
            .map_err(|_| LayerOrderSnapshotCloneErrorV1::AllocationFailed)?;
        let allocation_bytes = std::mem::size_of::<T>()
            .checked_mul(values.capacity())
            .ok_or(LayerOrderSnapshotCloneErrorV1::SizeOverflow)?;
        let observed = self
            .observed
            .checked_add(allocation_bytes)
            .ok_or(LayerOrderSnapshotCloneErrorV1::SizeOverflow)?;
        check_layer_order_snapshot_byte_limit_v1(observed, self.maximum)?;
        self.observed = observed;
        Ok(values)
    }
}

fn try_clone_exact_bytes_v1(
    source: &[u8],
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
) -> Result<Vec<u8>, LayerOrderSnapshotCloneErrorV1> {
    let mut cloned = budget.try_vec_with_exact_capacity(source.len())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn try_clone_exact_rational_v1(
    source: &ExactRationalValue,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
) -> Result<ExactRationalValue, LayerOrderSnapshotCloneErrorV1> {
    Ok(ExactRationalValue {
        sign: source.sign,
        numerator_magnitude_be: try_clone_exact_bytes_v1(&source.numerator_magnitude_be, budget)?,
        denominator_be: try_clone_exact_bytes_v1(&source.denominator_be, budget)?,
    })
}

fn try_clone_exact_point_v1(
    source: &ExactPointValue,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
) -> Result<ExactPointValue, LayerOrderSnapshotCloneErrorV1> {
    Ok(ExactPointValue {
        x: try_clone_exact_rational_v1(&source.x, budget)?,
        y: try_clone_exact_rational_v1(&source.y, budget)?,
    })
}

fn try_clone_exact_transform_v1(
    source: &ExactAffineTransform,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
) -> Result<ExactAffineTransform, LayerOrderSnapshotCloneErrorV1> {
    Ok(ExactAffineTransform {
        m00: try_clone_exact_rational_v1(&source.m00, budget)?,
        m01: try_clone_exact_rational_v1(&source.m01, budget)?,
        m10: try_clone_exact_rational_v1(&source.m10, budget)?,
        m11: try_clone_exact_rational_v1(&source.m11, budget)?,
        tx: try_clone_exact_rational_v1(&source.tx, budget)?,
        ty: try_clone_exact_rational_v1(&source.ty, budget)?,
    })
}

fn try_clone_layer_order_snapshot_filtered_v1(
    source: &LayerOrderSnapshot,
    faces: Option<&[FaceId]>,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
) -> Result<LayerOrderSnapshot, LayerOrderSnapshotCloneErrorV1> {
    let material_face_count = source
        .material_faces
        .iter()
        .filter(|face| face_is_retained_v1(faces, face.face_id))
        .count();
    let mut material_faces = budget.try_vec_with_exact_capacity(material_face_count)?;
    for face in &source.material_faces {
        if face_is_retained_v1(faces, face.face_id) {
            material_faces.push(*face);
        }
    }

    let global_bottom_to_top = if let Some(global) = &source.global_bottom_to_top {
        let retained_count = global
            .iter()
            .filter(|face| face_is_retained_v1(faces, face.face_id))
            .count();
        let mut retained = budget.try_vec_with_exact_capacity(retained_count)?;
        for face in global {
            if face_is_retained_v1(faces, face.face_id) {
                retained.push(*face);
            }
        }
        Some(retained)
    } else {
        None
    };

    let folded_face_count = source
        .folded_faces
        .iter()
        .filter(|face| face_is_retained_v1(faces, face.face.face_id))
        .count();
    let mut folded_faces = budget.try_vec_with_exact_capacity(folded_face_count)?;
    for folded in &source.folded_faces {
        if face_is_retained_v1(faces, folded.face.face_id) {
            folded_faces.push(FoldedFaceSnapshot {
                face: folded.face,
                source_to_flat: try_clone_exact_transform_v1(&folded.source_to_flat, budget)?,
                orientation: folded.orientation,
            });
        }
    }

    let overlap_cell_count = source
        .overlap_cells
        .iter()
        .filter(|cell| cell_is_retained_v1(faces, cell))
        .count();
    let mut overlap_cells = budget.try_vec_with_exact_capacity(overlap_cell_count)?;
    for cell in &source.overlap_cells {
        if !cell_is_retained_v1(faces, cell) {
            continue;
        }

        let mut exact_boundary = budget.try_vec_with_exact_capacity(cell.exact_boundary.len())?;
        for point in &cell.exact_boundary {
            exact_boundary.push(try_clone_exact_point_v1(point, budget)?);
        }

        let covering_face_count = cell
            .covering_faces
            .iter()
            .filter(|face| face_is_retained_v1(faces, face.face_id))
            .count();
        let mut covering_faces = budget.try_vec_with_exact_capacity(covering_face_count)?;
        for face in &cell.covering_faces {
            if face_is_retained_v1(faces, face.face_id) {
                covering_faces.push(*face);
            }
        }

        let layer_count = cell
            .bottom_to_top_faces
            .iter()
            .filter(|face| face_is_retained_v1(faces, **face))
            .count();
        let mut bottom_to_top_faces = budget.try_vec_with_exact_capacity(layer_count)?;
        for face in &cell.bottom_to_top_faces {
            if face_is_retained_v1(faces, *face) {
                bottom_to_top_faces.push(*face);
            }
        }

        overlap_cells.push(OverlapCellSnapshot {
            cell_key: cell.cell_key,
            exact_boundary,
            covering_faces,
            bottom_to_top_faces,
        });
    }

    let pair_count = source
        .face_pair_orders
        .iter()
        .filter(|pair| {
            face_is_retained_v1(faces, pair.lower_face.face_id)
                && face_is_retained_v1(faces, pair.upper_face.face_id)
        })
        .count();
    let mut face_pair_orders = budget.try_vec_with_exact_capacity(pair_count)?;
    for pair in &source.face_pair_orders {
        if !face_is_retained_v1(faces, pair.lower_face.face_id)
            || !face_is_retained_v1(faces, pair.upper_face.face_id)
        {
            continue;
        }
        let mut supporting_cells =
            budget.try_vec_with_exact_capacity(pair.supporting_cells.len())?;
        supporting_cells.extend_from_slice(&pair.supporting_cells);
        face_pair_orders.push(FacePairOrderSnapshot {
            lower_face: pair.lower_face,
            upper_face: pair.upper_face,
            supporting_cells,
        });
    }

    let reference_face = if faces.is_some() {
        source
            .reference_face
            .filter(|face| face_is_retained_v1(faces, face.face_id))
            .or_else(|| material_faces.first().copied())
    } else {
        source.reference_face
    };

    Ok(LayerOrderSnapshot {
        model_id: source.model_id,
        material_faces,
        global_bottom_to_top,
        provenance: source.provenance,
        reference_face,
        folded_faces,
        overlap_cells,
        face_pair_orders,
        proof_summary: source.proof_summary,
    })
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

/// Solves and verifies one flat layer-order certificate while requiring
/// selected pair directions.
///
/// Source revalidation, exact embedding, constraint construction, the
/// constrained solve, and certificate verification share one analysis pass
/// and one set of operation/search/storage limits. Invalid, contradictory,
/// inconclusive, or unsatisfied requirements never become a public
/// `Impossible` verdict for the project.
pub fn analyze_global_flat_foldability_with_required_pair_orders(
    input: GlobalFlatFoldabilityInput<'_>,
    limits: GlobalFlatFoldabilityLimits,
    required_pair_orders: &[RequiredLayerOrderPair],
) -> Result<LayerOrderSnapshot, RequiredLayerOrderError> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    analyze_global_flat_foldability_with_required_pair_orders_and_observer(
        input,
        limits,
        required_pair_orders,
        &mut observer,
    )
}

fn required_pair_preflight_failure(
    required_pair_count: usize,
    limits: GlobalFlatFoldabilityLimits,
) -> Option<GlobalFlatFoldabilityUnknownReason> {
    if required_pair_count > limits.max_overlap_face_pairs {
        return Some(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource: FlatFoldabilityResource::OverlapFacePairs,
            limit: limits.max_overlap_face_pairs,
            observed: required_pair_count,
        });
    }
    let required_storage =
        match required_pair_count.checked_mul(std::mem::size_of::<(usize, bool)>()) {
            Some(required_storage) => required_storage,
            None => {
                return Some(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: limits.max_certificate_bytes,
                    observed: usize::MAX,
                });
            }
        };
    if required_storage > limits.max_certificate_bytes {
        return Some(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource: FlatFoldabilityResource::CertificateBytes,
            limit: limits.max_certificate_bytes,
            observed: required_storage,
        });
    }
    None
}

struct ValidatedGlobalFlatSource<'a> {
    paper: &'a Paper,
    crease_pattern: &'a CreasePattern,
    topology: &'a TopologySnapshot,
    canonical_faces: Vec<LayerFace>,
    provenance: GlobalFlatFoldabilityProvenance,
    work_counts: GlobalFlatFoldabilityWorkCounts,
}

enum GlobalFlatSourceValidationFailure {
    Unknown {
        provenance: GlobalFlatFoldabilityProvenance,
        work_counts: GlobalFlatFoldabilityWorkCounts,
        reason: GlobalFlatFoldabilityUnknownReason,
    },
    Impossible {
        provenance: GlobalFlatFoldabilityProvenance,
        work_counts: GlobalFlatFoldabilityWorkCounts,
        violations: Vec<LocalNecessaryConditionViolation>,
    },
    Execution(GlobalFlatFoldabilityExecutionError),
}

fn validate_global_flat_source_with_observer<'a, O: GlobalFlatFoldabilityObserver + ?Sized>(
    input: GlobalFlatFoldabilityInput<'a>,
    limits: GlobalFlatFoldabilityLimits,
    required_pair_count: Option<usize>,
    observer: &mut O,
) -> Result<ValidatedGlobalFlatSource<'a>, Box<GlobalFlatSourceValidationFailure>> {
    let mut provenance = GlobalFlatFoldabilityProvenance {
        identity_namespace: input.identity_namespace,
        source_revision: input.source_revision,
        source_fingerprint: None,
        model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    };
    let work_counts = count_work(&input)
        .map_err(|error| Box::new(GlobalFlatSourceValidationFailure::Execution(error)))?;
    match phase_checkpoint(
        observer,
        GlobalFlatFoldabilityPhase::Capturing,
        work_counts,
        Some(work_counts.total_records),
    ) {
        Ok(None) => {}
        Ok(Some(reason)) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason,
            }));
        }
        Err(error) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Execution(
                error,
            )));
        }
    }
    if let Some(reason) =
        required_pair_count.and_then(|count| required_pair_preflight_failure(count, limits))
    {
        return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
            provenance,
            work_counts,
            reason,
        }));
    }
    if input.topology.source_revision != input.source_revision {
        return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
            provenance,
            work_counts,
            reason: GlobalFlatFoldabilityUnknownReason::StaleProvenance {
                artifact: FlatFoldabilityInputArtifact::TopologySnapshot,
                expected_revision: input.source_revision,
                actual_revision: input.topology.source_revision,
            },
        }));
    }
    if input.local_report_source_revision != input.source_revision {
        return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
            provenance,
            work_counts,
            reason: GlobalFlatFoldabilityUnknownReason::StaleProvenance {
                artifact: FlatFoldabilityInputArtifact::LocalFlatFoldabilityReport,
                expected_revision: input.source_revision,
                actual_revision: input.local_report_source_revision,
            },
        }));
    }
    if let Some(reason) = first_limit_failure(work_counts, limits) {
        return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
            provenance,
            work_counts,
            reason,
        }));
    }
    let (Some(identity_namespace), Some(paper), Some(crease_pattern)) =
        (input.identity_namespace, input.paper, input.crease_pattern)
    else {
        return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
            provenance,
            work_counts,
            reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable,
            },
        }));
    };
    match phase_checkpoint(
        observer,
        GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
        work_counts,
        Some(work_counts.local_vertex_records),
    ) {
        Ok(None) => {}
        Ok(Some(reason)) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason,
            }));
        }
        Err(error) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Execution(
                error,
            )));
        }
    }
    let fingerprint = {
        let mut checkpoint = || observer_reverification_checkpoint(observer);
        fold_model_fingerprint_v1_with_checkpoint(crease_pattern, paper, &mut checkpoint)
    };
    match fingerprint {
        Ok(fingerprint) => provenance.source_fingerprint = Some(fingerprint),
        Err(SourceReverificationAbort::Unknown(reason)) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason,
            }));
        }
        Err(SourceReverificationAbort::Execution(error)) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Execution(
                error,
            )));
        }
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
            return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason,
            }));
        }
        Err(SourceReverificationAbort::Execution(error)) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Execution(
                error,
            )));
        }
    }
    let canonical_faces = validate_topology(input.topology).map_err(|failure| {
        global_source_failure_from_input_structure(failure, provenance, work_counts)
    })?;
    if canonical_faces.is_empty() {
        return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
            provenance,
            work_counts,
            reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::NoMaterialFaces,
            },
        }));
    }
    match validate_local_report(input.local_flat_foldability).map_err(|failure| {
        global_source_failure_from_input_structure(failure, provenance, work_counts)
    })? {
        LocalReportEvidence::Blocked => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::LocalNecessaryConditionsBlocked,
                },
            }));
        }
        LocalReportEvidence::Indeterminate => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason:
                        FlatFoldabilityProofIncompleteReason::LocalNecessaryConditionsIndeterminate,
                },
            }));
        }
        LocalReportEvidence::Violated(violations) => {
            return Err(Box::new(GlobalFlatSourceValidationFailure::Impossible {
                provenance,
                work_counts,
                violations,
            }));
        }
        LocalReportEvidence::NoViolation => {}
    }
    Ok(ValidatedGlobalFlatSource {
        paper,
        crease_pattern,
        topology: input.topology,
        canonical_faces,
        provenance,
        work_counts,
    })
}

/// Observer-enabled form of
/// [`analyze_global_flat_foldability_with_required_pair_orders`].
pub fn analyze_global_flat_foldability_with_required_pair_orders_and_observer<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    input: GlobalFlatFoldabilityInput<'_>,
    limits: GlobalFlatFoldabilityLimits,
    required_pair_orders: &[RequiredLayerOrderPair],
    observer: &mut O,
) -> Result<LayerOrderSnapshot, RequiredLayerOrderError> {
    let validated = match validate_global_flat_source_with_observer(
        input,
        limits,
        Some(required_pair_orders.len()),
        observer,
    ) {
        Ok(validated) => validated,
        Err(failure) => match *failure {
            GlobalFlatSourceValidationFailure::Unknown { reason, .. } => {
                return Err(RequiredLayerOrderError::Inconclusive { reason });
            }
            GlobalFlatSourceValidationFailure::Impossible { .. } => {
                return Err(RequiredLayerOrderError::BaseAnalysisImpossible);
            }
            GlobalFlatSourceValidationFailure::Execution(error) => {
                return Err(RequiredLayerOrderError::Execution(error));
            }
        },
    };
    facewise::analyze_facewise_with_required_pair_orders(
        facewise::FacewiseAnalysisInput {
            paper: validated.paper,
            crease_pattern: validated.crease_pattern,
            topology: validated.topology,
            canonical_faces: &validated.canonical_faces,
            provenance: validated.provenance,
            work_counts: validated.work_counts,
            limits,
        },
        required_pair_orders,
        observer,
    )
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
    let validated = match validate_global_flat_source_with_observer(input, limits, None, observer) {
        Ok(validated) => validated,
        Err(failure) => match *failure {
            GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason,
            } => return Ok(unknown(provenance, work_counts, reason)),
            GlobalFlatSourceValidationFailure::Impossible {
                provenance,
                work_counts,
                violations,
            } => {
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
            GlobalFlatSourceValidationFailure::Execution(error) => return Err(error),
        },
    };
    facewise::analyze_facewise(
        facewise::FacewiseAnalysisInput {
            paper: validated.paper,
            crease_pattern: validated.crease_pattern,
            topology: validated.topology,
            canonical_faces: &validated.canonical_faces,
            provenance: validated.provenance,
            work_counts: validated.work_counts,
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
        let overflow = || GlobalFlatFoldabilityExecutionError::Internal {
            reason: GlobalFlatFoldabilityInternalError::WorkCountOverflow,
        };
        let mut total = total
            .checked_add(face.outer.half_edges.len())
            .ok_or_else(overflow)?;
        for boundary in face.holes.iter().chain(&face.seams) {
            total = total
                .checked_add(boundary.half_edges.len())
                .ok_or_else(overflow)?;
        }
        Ok::<_, GlobalFlatFoldabilityExecutionError>(total)
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

enum InputStructureValidationFailure {
    Unknown(GlobalFlatFoldabilityUnknownReason),
    Execution(GlobalFlatFoldabilityExecutionError),
}

fn global_source_failure_from_input_structure(
    failure: InputStructureValidationFailure,
    provenance: GlobalFlatFoldabilityProvenance,
    work_counts: GlobalFlatFoldabilityWorkCounts,
) -> Box<GlobalFlatSourceValidationFailure> {
    Box::new(match failure {
        InputStructureValidationFailure::Unknown(reason) => {
            GlobalFlatSourceValidationFailure::Unknown {
                provenance,
                work_counts,
                reason,
            }
        }
        InputStructureValidationFailure::Execution(error) => {
            GlobalFlatSourceValidationFailure::Execution(error)
        }
    })
}

impl From<GlobalFlatFoldabilityUnknownReason> for InputStructureValidationFailure {
    fn from(reason: GlobalFlatFoldabilityUnknownReason) -> Self {
        Self::Unknown(reason)
    }
}

impl From<GlobalFlatFoldabilityExecutionError> for InputStructureValidationFailure {
    fn from(error: GlobalFlatFoldabilityExecutionError) -> Self {
        Self::Execution(error)
    }
}

const fn validation_allocation_failure() -> GlobalFlatFoldabilityExecutionError {
    GlobalFlatFoldabilityExecutionError::Internal {
        reason: GlobalFlatFoldabilityInternalError::AllocationFailed,
    }
}

fn try_validation_vec_with_capacity<T>(
    capacity: usize,
) -> Result<Vec<T>, InputStructureValidationFailure> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| validation_allocation_failure())?;
    Ok(values)
}

fn try_validation_hash_set_with_capacity<T: Eq + Hash>(
    capacity: usize,
) -> Result<HashSet<T>, InputStructureValidationFailure> {
    let mut values = HashSet::new();
    values
        .try_reserve(capacity)
        .map_err(|_| validation_allocation_failure())?;
    Ok(values)
}

fn try_validation_hash_map_with_capacity<K: Eq + Hash, V>(
    capacity: usize,
) -> Result<HashMap<K, V>, InputStructureValidationFailure> {
    let mut values = HashMap::new();
    values
        .try_reserve(capacity)
        .map_err(|_| validation_allocation_failure())?;
    Ok(values)
}

fn validate_topology(
    topology: &TopologySnapshot,
) -> Result<Vec<LayerFace>, InputStructureValidationFailure> {
    let mut face_ids = try_validation_hash_set_with_capacity(topology.faces.len())?;
    let mut face_keys = try_validation_hash_set_with_capacity(topology.faces.len())?;
    let mut keys_by_id = try_validation_hash_map_with_capacity(topology.faces.len())?;
    let mut canonical_faces = try_validation_vec_with_capacity(topology.faces.len())?;
    let mut face_records = try_validation_vec_with_capacity(topology.faces.len())?;
    face_records.extend(topology.faces.iter());
    face_records.sort_unstable_by_key(|face| (face.id.canonical_bytes(), face.key));
    for face in face_records {
        if !face_ids.insert(face.id) {
            return Err(
                inconsistent(FlatFoldabilityInputConsistencyIssue::DuplicateFaceId {
                    face: face.id,
                })
                .into(),
            );
        }
        if !face_keys.insert(face.key) {
            return Err(
                inconsistent(FlatFoldabilityInputConsistencyIssue::DuplicateFaceKey {
                    face_key: face.key,
                })
                .into(),
            );
        }
        keys_by_id.insert(face.id, face.key);
        canonical_faces.push(LayerFace {
            face_id: face.id,
            face_key: face.key,
        });
    }
    canonical_faces.sort_unstable_by_key(|face| (face.face_key, face.face_id.canonical_bytes()));

    let mut incidence_edges = try_validation_hash_set_with_capacity(topology.edge_incidence.len())?;
    let mut incidence_hinges =
        try_validation_hash_map_with_capacity(topology.edge_incidence.len())?;
    let mut incidence_records = try_validation_vec_with_capacity(topology.edge_incidence.len())?;
    incidence_records.extend(topology.edge_incidence.iter().copied());
    incidence_records.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    for (edge, incidence) in incidence_records {
        if !incidence_edges.insert(edge) {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::DuplicateIncidenceEdge { edge },
            )
            .into());
        }
        match incidence {
            EdgeIncidence::Boundary { material } => {
                ensure_face_exists(&keys_by_id, edge, material, false)?;
            }
            EdgeIncidence::Hinge {
                left,
                right,
                assignment,
            } => {
                ensure_face_exists(&keys_by_id, edge, left, false)?;
                ensure_face_exists(&keys_by_id, edge, right, false)?;
                if left == right {
                    return Err(
                        inconsistent(FlatFoldabilityInputConsistencyIssue::SelfHinge {
                            edge,
                            face: left,
                        })
                        .into(),
                    );
                }
                incidence_hinges.insert(edge, (left, right, assignment));
            }
            EdgeIncidence::Cut { left, right } => {
                ensure_face_exists(&keys_by_id, edge, left, false)?;
                ensure_face_exists(&keys_by_id, edge, right, false)?;
            }
            EdgeIncidence::AuxiliaryIgnored => {}
        }
    }

    let mut adjacency_edges =
        try_validation_hash_set_with_capacity(topology.hinge_adjacency.len())?;
    let mut hinge_records = try_validation_vec_with_capacity(topology.hinge_adjacency.len())?;
    hinge_records.extend(topology.hinge_adjacency.iter().copied());
    hinge_records.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    for hinge in hinge_records {
        if !adjacency_edges.insert(hinge.edge) {
            return Err(
                inconsistent(FlatFoldabilityInputConsistencyIssue::DuplicateHingeEdge {
                    edge: hinge.edge,
                })
                .into(),
            );
        }
        ensure_face_exists(&keys_by_id, hinge.edge, hinge.first, true)?;
        ensure_face_exists(&keys_by_id, hinge.edge, hinge.second, true)?;
        if hinge.first == hinge.second {
            return Err(
                inconsistent(FlatFoldabilityInputConsistencyIssue::SelfHinge {
                    edge: hinge.edge,
                    face: hinge.first,
                })
                .into(),
            );
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
            )
            .into());
        }
        let Some((left, right, assignment)) = incidence_hinges.get(&hinge.edge).copied() else {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::HingeIncidenceMissing { edge: hinge.edge },
            )
            .into());
        };
        if assignment != hinge.assignment {
            return Err(inconsistent(
                FlatFoldabilityInputConsistencyIssue::HingeAssignmentMismatch { edge: hinge.edge },
            )
            .into());
        }
        let same_faces = (left == hinge.first && right == hinge.second)
            || (left == hinge.second && right == hinge.first);
        if !same_faces {
            return Err(
                inconsistent(FlatFoldabilityInputConsistencyIssue::HingeFacesMismatch {
                    edge: hinge.edge,
                })
                .into(),
            );
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
        )
        .into());
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
) -> Result<LocalReportEvidence, InputStructureValidationFailure> {
    if report.model != LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1 {
        return Err(
            inconsistent(FlatFoldabilityInputConsistencyIssue::LocalReportStatusMismatch).into(),
        );
    }
    if report.max_exact_fold_degree != MAX_EXACT_FOLD_DEGREE {
        return Err(inconsistent(
            FlatFoldabilityInputConsistencyIssue::UnexpectedLocalFoldDegreeLimit {
                expected: MAX_EXACT_FOLD_DEGREE,
                actual: report.max_exact_fold_degree,
            },
        )
        .into());
    }

    let mut vertices = try_validation_hash_set_with_capacity(report.vertices.len())?;
    let mut satisfied = 0_usize;
    let mut violated = 0_usize;
    let mut not_applicable = 0_usize;
    let mut indeterminate = 0_usize;
    let mut violations = try_validation_vec_with_capacity(report.vertices.len())?;
    let mut vertex_records = try_validation_vec_with_capacity(report.vertices.len())?;
    vertex_records.extend(report.vertices.iter());
    vertex_records.sort_unstable_by_key(|vertex| vertex.vertex.canonical_bytes());
    for vertex in vertex_records {
        if !vertices.insert(vertex.vertex) {
            return Err(
                inconsistent(FlatFoldabilityInputConsistencyIssue::DuplicateLocalVertex {
                    vertex: vertex.vertex,
                })
                .into(),
            );
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
            )
            .into());
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
            )
            .into());
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
        return Err(
            inconsistent(FlatFoldabilityInputConsistencyIssue::LocalReportCountsMismatch).into(),
        );
    }

    if report.status == LocalFlatFoldabilityReportStatus::Blocked {
        if report.total_vertices == 0
            && applicable == 0
            && not_applicable == 0
            && violations.is_empty()
        {
            return Ok(LocalReportEvidence::Blocked);
        }
        return Err(
            inconsistent(FlatFoldabilityInputConsistencyIssue::LocalReportStatusMismatch).into(),
        );
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
        return Err(
            inconsistent(FlatFoldabilityInputConsistencyIssue::LocalReportStatusMismatch).into(),
        );
    }
    violations.sort_unstable_by_key(|violation| violation.vertex.canonical_bytes());
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

    fn retained_layer_face(id_suffix: u64, key: u8) -> LayerFace {
        LayerFace {
            face_id: fixed_id(id_suffix),
            face_key: FaceKey([key; 32]),
        }
    }

    fn retained_exact_rational(seed: u8, numerator_len: usize) -> ExactRationalValue {
        ExactRationalValue {
            sign: ExactSign::Positive,
            numerator_magnitude_be: vec![seed; numerator_len],
            denominator_be: vec![1, seed.max(1)],
        }
    }

    fn retained_exact_point(seed: u8) -> ExactPointValue {
        ExactPointValue {
            x: retained_exact_rational(seed, usize::from(seed % 3) + 1),
            y: retained_exact_rational(seed.wrapping_add(1), usize::from(seed % 4) + 1),
        }
    }

    fn retained_exact_transform(seed: u8) -> ExactAffineTransform {
        ExactAffineTransform {
            m00: retained_exact_rational(seed, 1),
            m01: retained_exact_rational(seed.wrapping_add(1), 2),
            m10: retained_exact_rational(seed.wrapping_add(2), 3),
            m11: retained_exact_rational(seed.wrapping_add(3), 4),
            tx: retained_exact_rational(seed.wrapping_add(4), 5),
            ty: retained_exact_rational(seed.wrapping_add(5), 6),
        }
    }

    fn retained_layer_order_snapshot() -> LayerOrderSnapshot {
        let first = retained_layer_face(0xa01, 1);
        let second = retained_layer_face(0xa02, 2);
        let third = retained_layer_face(0xa03, 3);
        LayerOrderSnapshot {
            model_id: LAYER_ORDER_MODEL_ID,
            material_faces: vec![first, second, third],
            global_bottom_to_top: Some(vec![first, second, third]),
            provenance: LayerOrderProvenance {
                source: GlobalFlatFoldabilityProvenance {
                    identity_namespace: Some(fixed_id(0xa10)),
                    source_revision: REVISION,
                    source_fingerprint: Some(FoldModelFingerprintV1([0xa5; 32])),
                    model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
                },
                derivation: LayerOrderDerivation::FacewiseCertificate {
                    reference_face: first,
                    overlap_cell_count: 2,
                    constraint_count: 2,
                },
            },
            reference_face: Some(third),
            folded_faces: vec![
                FoldedFaceSnapshot {
                    face: first,
                    source_to_flat: retained_exact_transform(10),
                    orientation: FoldedFaceOrientation::FrontUp,
                },
                FoldedFaceSnapshot {
                    face: second,
                    source_to_flat: retained_exact_transform(20),
                    orientation: FoldedFaceOrientation::BackUp,
                },
                FoldedFaceSnapshot {
                    face: third,
                    source_to_flat: retained_exact_transform(30),
                    orientation: FoldedFaceOrientation::FrontUp,
                },
            ],
            overlap_cells: vec![
                OverlapCellSnapshot {
                    cell_key: OverlapCellKey([1; 32]),
                    exact_boundary: vec![retained_exact_point(40), retained_exact_point(50)],
                    covering_faces: vec![first, second],
                    bottom_to_top_faces: vec![first.face_id, second.face_id],
                },
                OverlapCellSnapshot {
                    cell_key: OverlapCellKey([2; 32]),
                    exact_boundary: vec![retained_exact_point(60)],
                    covering_faces: vec![third],
                    bottom_to_top_faces: vec![third.face_id],
                },
            ],
            face_pair_orders: vec![
                FacePairOrderSnapshot {
                    lower_face: first,
                    upper_face: second,
                    supporting_cells: vec![OverlapCellKey([1; 32])],
                },
                FacePairOrderSnapshot {
                    lower_face: second,
                    upper_face: third,
                    supporting_cells: vec![OverlapCellKey([2; 32])],
                },
            ],
            proof_summary: Some(FacewiseProofSummary {
                material_faces: 3,
                overlap_face_pairs: 2,
                overlap_cells: 2,
                constraints: 2,
                search_nodes: 3,
                maximum_ply: 2,
                certificate_bytes: 512,
            }),
        }
    }

    #[test]
    fn deep_retained_bytes_include_nested_vector_spare_capacity() {
        let mut snapshot = retained_layer_order_snapshot();
        let before = snapshot
            .checked_deep_retained_bytes_v1()
            .expect("fixture retained bytes");
        let old_covering_capacity = snapshot.overlap_cells[0].covering_faces.capacity();
        let old_exact_capacity = snapshot.overlap_cells[0].exact_boundary[0]
            .x
            .numerator_magnitude_be
            .capacity();
        let old_supporting_capacity = snapshot.face_pair_orders[0].supporting_cells.capacity();

        snapshot.overlap_cells[0].covering_faces.reserve_exact(7);
        snapshot.overlap_cells[0].exact_boundary[0]
            .x
            .numerator_magnitude_be
            .reserve_exact(17);
        snapshot.face_pair_orders[0]
            .supporting_cells
            .reserve_exact(5);

        let expected_growth = (snapshot.overlap_cells[0].covering_faces.capacity()
            - old_covering_capacity)
            * std::mem::size_of::<LayerFace>()
            + (snapshot.overlap_cells[0].exact_boundary[0]
                .x
                .numerator_magnitude_be
                .capacity()
                - old_exact_capacity)
                * std::mem::size_of::<u8>()
            + (snapshot.face_pair_orders[0].supporting_cells.capacity() - old_supporting_capacity)
                * std::mem::size_of::<OverlapCellKey>();
        assert_eq!(
            snapshot.checked_deep_retained_bytes_v1(),
            before.checked_add(expected_growth)
        );
    }

    #[test]
    fn fallible_clone_preserves_large_exact_payload() {
        let mut snapshot = retained_layer_order_snapshot();
        snapshot.folded_faces[0]
            .source_to_flat
            .m00
            .numerator_magnitude_be = vec![0xab; 1024 * 1024];
        let retained = snapshot
            .checked_deep_retained_bytes_v1()
            .expect("large fixture retained bytes");
        let cloned = snapshot
            .try_clone_with_retained_byte_limit_v1(retained)
            .expect("large exact payload clone");
        assert_eq!(cloned, snapshot);
        assert_eq!(cloned.checked_deep_retained_bytes_v1(), Some(retained));
    }

    #[test]
    fn restricted_clone_filters_every_face_owned_collection() {
        let snapshot = retained_layer_order_snapshot();
        let first = snapshot.material_faces[0];
        let second = snapshot.material_faces[1];
        let faces = [first.face_id, second.face_id];
        let retained = snapshot
            .checked_restricted_deep_retained_bytes_v1(&faces)
            .expect("restricted retained bytes");
        let restricted = snapshot
            .try_restrict_to_faces_with_retained_byte_limit_v1(&faces, retained)
            .expect("restricted clone");

        assert_eq!(restricted.material_faces, vec![first, second]);
        assert_eq!(restricted.global_bottom_to_top, Some(vec![first, second]));
        assert_eq!(
            restricted
                .folded_faces
                .iter()
                .map(|folded| folded.face)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_eq!(restricted.overlap_cells.len(), 1);
        assert_eq!(
            restricted.overlap_cells[0].covering_faces,
            vec![first, second]
        );
        assert_eq!(restricted.overlap_cells[0].bottom_to_top_faces, faces);
        assert_eq!(restricted.face_pair_orders.len(), 1);
        assert_eq!(restricted.face_pair_orders[0].lower_face, first);
        assert_eq!(restricted.face_pair_orders[0].upper_face, second);
        assert_eq!(restricted.reference_face, Some(first));
        assert_eq!(restricted.checked_deep_retained_bytes_v1(), Some(retained));
        assert_eq!(
            snapshot.try_restrict_to_faces_with_retained_byte_limit_v1(&faces, retained - 1),
            Err(LayerOrderSnapshotCloneErrorV1::ByteLimitExceeded {
                observed: retained,
                maximum: retained - 1,
            })
        );
    }

    #[test]
    fn restricted_clone_does_not_charge_or_copy_excluded_nested_regions() {
        let mut snapshot = retained_layer_order_snapshot();
        let first = snapshot.material_faces[0];
        let second = snapshot.material_faces[1];
        let faces = [first.face_id, second.face_id];
        let before = snapshot
            .checked_restricted_deep_retained_bytes_v1(&faces)
            .expect("baseline restricted bytes");

        snapshot.overlap_cells[1].exact_boundary[0]
            .x
            .numerator_magnitude_be = vec![0xcd; 256 * 1024];
        snapshot.face_pair_orders[1].supporting_cells = vec![OverlapCellKey([0xee; 32]); 4_096];

        assert_eq!(
            snapshot.checked_restricted_deep_retained_bytes_v1(&faces),
            Some(before),
            "excluded cell geometry and pair support must not enter the projected clone budget"
        );
        let restricted = snapshot
            .try_restrict_to_faces_with_retained_byte_limit_v1(&faces, before)
            .expect("excluded nested regions are not cloned");
        assert_eq!(restricted.overlap_cells.len(), 1);
        assert_eq!(restricted.face_pair_orders.len(), 1);
        assert_eq!(restricted.checked_deep_retained_bytes_v1(), Some(before));
    }

    #[test]
    fn retained_byte_limit_accepts_exact_and_rejects_one_short() {
        let snapshot = retained_layer_order_snapshot();
        let projected = checked_layer_order_snapshot_projected_bytes_v1(&snapshot, None)
            .expect("projected bytes");
        assert_eq!(
            snapshot.checked_deep_retained_bytes_v1(),
            Some(projected),
            "fixture vectors must have no spare capacity"
        );
        assert_eq!(
            snapshot
                .try_clone_with_retained_byte_limit_v1(projected)
                .expect("exact byte limit"),
            snapshot
        );
        assert_eq!(
            snapshot.try_clone_with_retained_byte_limit_v1(projected - 1),
            Err(LayerOrderSnapshotCloneErrorV1::ByteLimitExceeded {
                observed: projected,
                maximum: projected - 1,
            })
        );
    }

    #[test]
    fn retained_byte_helpers_report_overflow_and_allocation_failure() {
        let mut total = usize::MAX;
        assert_eq!(checked_add_vec_allocation_v1::<u8>(&mut total, 1), None);
        let mut total = 0;
        assert_eq!(
            checked_add_vec_allocation_v1::<u64>(&mut total, usize::MAX),
            None
        );
        assert_eq!(
            LayerOrderSnapshotCloneBudgetV1::new(usize::MAX)
                .expect("unbounded test budget")
                .try_vec_with_exact_capacity::<u8>(usize::MAX),
            Err(LayerOrderSnapshotCloneErrorV1::AllocationFailed)
        );
        let snapshot_bytes = std::mem::size_of::<LayerOrderSnapshot>();
        let mut exhausted =
            LayerOrderSnapshotCloneBudgetV1::new(snapshot_bytes).expect("snapshot shell fits");
        assert!(matches!(
            exhausted.try_vec_with_exact_capacity::<u8>(1),
            Err(LayerOrderSnapshotCloneErrorV1::ByteLimitExceeded {
                observed,
                maximum,
            }) if observed > maximum && maximum == snapshot_bytes
        ));
        assert_eq!(
            exhausted.observed, snapshot_bytes,
            "a rejected allocator capacity must not consume later budget"
        );
        for allocation in [
            try_validation_vec_with_capacity::<u8>(usize::MAX).map(|_| ()),
            try_validation_hash_set_with_capacity::<u8>(usize::MAX).map(|_| ()),
            try_validation_hash_map_with_capacity::<u8, u8>(usize::MAX).map(|_| ()),
        ] {
            assert!(matches!(
                allocation,
                Err(InputStructureValidationFailure::Execution(
                    GlobalFlatFoldabilityExecutionError::Internal {
                        reason: GlobalFlatFoldabilityInternalError::AllocationFailed,
                    }
                ))
            ));
        }
        assert!(matches!(
            required_pair_preflight_failure(
                usize::MAX,
                GlobalFlatFoldabilityLimits {
                    max_overlap_face_pairs: usize::MAX,
                    max_certificate_bytes: usize::MAX,
                    ..GlobalFlatFoldabilityLimits::default()
                },
            ),
            Some(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: usize::MAX,
                observed: usize::MAX,
            })
        ));
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

    #[derive(Default)]
    struct PhaseRecorder {
        phases: Vec<GlobalFlatFoldabilityPhase>,
    }

    impl GlobalFlatFoldabilityObserver for PhaseRecorder {
        fn on_progress(&mut self, progress: GlobalFlatFoldabilityProgress) {
            assert!(
                self.phases
                    .last()
                    .is_none_or(|previous| *previous <= progress.phase),
                "constrained progress phases remain monotonic"
            );
            self.phases.push(progress.phase);
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
    fn constrained_public_api_classifies_required_pairs_and_preserves_empty_result() {
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
        let ordinary =
            analyze_global_flat_foldability(input(), GlobalFlatFoldabilityLimits::default())
                .unwrap()
                .layer_order()
                .unwrap()
                .clone();
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[],
            )
            .unwrap(),
            ordinary,
            "empty requirements must use the ordinary single-pass solve without overlay storage"
        );

        let positive = ordinary.face_pair_orders[0].clone();
        let required = RequiredLayerOrderPair {
            lower_face: positive.lower_face,
            upper_face: positive.upper_face,
        };
        let canonical_direction = (
            required.lower_face.face_key,
            required.lower_face.face_id.canonical_bytes(),
        ) < (
            required.upper_face.face_key,
            required.upper_face.face_id.canonical_bytes(),
        );
        let constrained = analyze_global_flat_foldability_with_required_pair_orders(
            input(),
            GlobalFlatFoldabilityLimits::default(),
            &[required],
        )
        .expect("one existing directed relation remains satisfiable");
        assert!(constrained.face_pair_orders.iter().any(|order| {
            order.lower_face == required.lower_face && order.upper_face == required.upper_face
        }));

        let mut reversed_pattern = pattern.clone();
        for edge in &mut reversed_pattern.edges {
            edge.kind = match edge.kind {
                EdgeKind::Mountain => EdgeKind::Valley,
                EdgeKind::Valley => EdgeKind::Mountain,
                other => other,
            };
        }
        let reversed_topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(1),
            source_revision: REVISION,
            paper: &paper,
            pattern: &reversed_pattern,
        })
        .expect("assignment-reversed accordion topology");
        let reversed_local = analyze_local_flat_foldability(&paper, &reversed_pattern);
        let reversed_input = || {
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(1),
                &paper,
                &reversed_pattern,
                &reversed_topology,
                &reversed_local,
            )
        };
        let reversed_ordinary = analyze_global_flat_foldability(
            reversed_input(),
            GlobalFlatFoldabilityLimits::default(),
        )
        .unwrap()
        .layer_order()
        .unwrap()
        .clone();
        let reverse_direction_required = reversed_ordinary
            .face_pair_orders
            .iter()
            .find(|order| {
                let lower_key = (
                    order.lower_face.face_key,
                    order.lower_face.face_id.canonical_bytes(),
                );
                let upper_key = (
                    order.upper_face.face_key,
                    order.upper_face.face_id.canonical_bytes(),
                );
                (lower_key < upper_key) != canonical_direction
            })
            .map(|order| RequiredLayerOrderPair {
                lower_face: order.lower_face,
                upper_face: order.upper_face,
            })
            .expect("reversing all hinge assignments reverses the canonical pair direction");
        let reverse_direction_constrained =
            analyze_global_flat_foldability_with_required_pair_orders(
                reversed_input(),
                GlobalFlatFoldabilityLimits::default(),
                &[reverse_direction_required],
            )
            .expect("the opposite canonical direction is independently satisfiable");
        assert!(
            reverse_direction_constrained
                .face_pair_orders
                .iter()
                .any(|order| {
                    order.lower_face == reverse_direction_required.lower_face
                        && order.upper_face == reverse_direction_required.upper_face
                })
        );

        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[required, required],
            ),
            Err(RequiredLayerOrderError::DuplicatePair {
                lower: required.lower_face.face_id,
                upper: required.upper_face.face_id,
            })
        );
        let mut required_error_progress = PhaseRecorder::default();
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders_and_observer(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[required, required],
                &mut required_error_progress,
            ),
            Err(RequiredLayerOrderError::DuplicatePair { .. })
        ));
        assert_eq!(
            required_error_progress.phases.last(),
            Some(&GlobalFlatFoldabilityPhase::Completed),
            "facewise required-order failures publish a terminal progress update"
        );
        let reverse = RequiredLayerOrderPair {
            lower_face: required.upper_face,
            upper_face: required.lower_face,
        };
        for requirements in [
            vec![required, required, reverse],
            vec![required, reverse, required],
            vec![reverse, required, required],
        ] {
            assert!(matches!(
                analyze_global_flat_foldability_with_required_pair_orders(
                    input(),
                    GlobalFlatFoldabilityLimits::default(),
                    &requirements,
                ),
                Err(RequiredLayerOrderError::ConflictingPair { .. })
            ));
        }

        let mut unknown_key = required;
        unknown_key.lower_face.face_key.0[0] ^= 0xff;
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[unknown_key],
            ),
            Err(RequiredLayerOrderError::UnknownFace {
                face: unknown_key.lower_face.face_id,
            })
        );
        let mut unknown_id = required;
        unknown_id.lower_face.face_id = FaceId::new();
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[unknown_id],
            ),
            Err(RequiredLayerOrderError::UnknownFace {
                face: unknown_id.lower_face.face_id,
            })
        );
        let equal = RequiredLayerOrderPair {
            lower_face: required.lower_face,
            upper_face: required.lower_face,
        };
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[equal],
            ),
            Err(RequiredLayerOrderError::EqualFace {
                face: equal.lower_face.face_id,
            })
        );
        let mut equal_with_tampered_key = equal;
        equal_with_tampered_key.upper_face.face_key.0[0] ^= 0xff;
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[equal_with_tampered_key],
            ),
            Err(RequiredLayerOrderError::UnknownFace {
                face: equal_with_tampered_key.upper_face.face_id,
            }),
            "exact trusted-face lookup takes precedence over same-ID classification"
        );

        let hinge_pairs = topology
            .hinge_adjacency
            .iter()
            .map(|hinge| {
                if hinge.first.canonical_bytes() < hinge.second.canonical_bytes() {
                    (hinge.first, hinge.second)
                } else {
                    (hinge.second, hinge.first)
                }
            })
            .collect::<HashSet<_>>();
        let inferred = ordinary
            .face_pair_orders
            .iter()
            .find(|order| {
                let key = if order.lower_face.face_id.canonical_bytes()
                    < order.upper_face.face_id.canonical_bytes()
                {
                    (order.lower_face.face_id, order.upper_face.face_id)
                } else {
                    (order.upper_face.face_id, order.lower_face.face_id)
                };
                !hinge_pairs.contains(&key)
            })
            .expect("the accordion has one non-hinge inferred pair");
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[RequiredLayerOrderPair {
                    lower_face: inferred.upper_face,
                    upper_face: inferred.lower_face,
                }],
            ),
            Err(RequiredLayerOrderError::Unsatisfied)
        );
    }

    #[test]
    fn constrained_public_api_rejects_fixed_nonoverlap_resource_deadline_and_cancel() {
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
        let ordinary =
            analyze_global_flat_foldability(input(), GlobalFlatFoldabilityLimits::default())
                .unwrap()
                .layer_order()
                .unwrap()
                .clone();
        let order = &ordinary.face_pair_orders[0];
        let required = RequiredLayerOrderPair {
            lower_face: order.lower_face,
            upper_face: order.upper_face,
        };
        assert_eq!(
            required_pair_preflight_failure(
                1,
                GlobalFlatFoldabilityLimits {
                    max_overlap_face_pairs: 1,
                    max_certificate_bytes: std::mem::size_of::<(usize, bool)>(),
                    ..GlobalFlatFoldabilityLimits::default()
                },
            ),
            None,
            "required-pair count and preflight storage admit equality"
        );
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[RequiredLayerOrderPair {
                    lower_face: required.upper_face,
                    upper_face: required.lower_face,
                }],
            ),
            Err(RequiredLayerOrderError::ContradictsTrustedFixedOrder { .. })
        ));

        let required_storage = std::mem::size_of::<(usize, bool)>();
        assert!(matches!(
            required_pair_preflight_failure(
                1,
                GlobalFlatFoldabilityLimits {
                    max_certificate_bytes: required_storage - 1,
                    ..GlobalFlatFoldabilityLimits::default()
                },
            ),
            Some(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
            }) if limit == required_storage - 1 && observed == required_storage
        ));
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits {
                    max_certificate_bytes: required_storage - 1,
                    ..GlobalFlatFoldabilityLimits::default()
                },
                &[required],
            ),
            Err(RequiredLayerOrderError::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            }) if limit == required_storage - 1 && observed == required_storage
        ));
        let equality = analyze_global_flat_foldability_with_required_pair_orders(
            input(),
            GlobalFlatFoldabilityLimits {
                max_overlap_face_pairs: 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
            &[required],
        )
        .expect("one trusted overlap pair is admitted at the exact count limit");
        assert!(equality.face_pair_orders.iter().any(|order| {
            order.lower_face == required.lower_face && order.upper_face == required.upper_face
        }));

        struct ImmediateCheckpoint(GlobalFlatFoldabilityCheckpoint);
        impl GlobalFlatFoldabilityObserver for ImmediateCheckpoint {
            fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
                self.0
            }
        }
        let capped = GlobalFlatFoldabilityLimits {
            max_overlap_face_pairs: 0,
            ..GlobalFlatFoldabilityLimits::default()
        };
        let mut immediate_deadline =
            ImmediateCheckpoint(GlobalFlatFoldabilityCheckpoint::DeadlineReached);
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders_and_observer(
                input(),
                capped,
                &[required],
                &mut immediate_deadline,
            ),
            Err(RequiredLayerOrderError::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::Capturing,
                },
            }),
            "the Capturing checkpoint takes priority over required-count limits"
        );
        let mut immediate_cancel = ImmediateCheckpoint(GlobalFlatFoldabilityCheckpoint::Cancelled);
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders_and_observer(
                input(),
                capped,
                &[required],
                &mut immediate_cancel,
            ),
            Err(RequiredLayerOrderError::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled
            )),
            "Capturing cancellation takes priority over required-count limits"
        );
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits {
                    max_overlap_face_pairs: 0,
                    ..GlobalFlatFoldabilityLimits::default()
                },
                &[required],
            ),
            Err(RequiredLayerOrderError::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::OverlapFacePairs,
                    limit: 0,
                    observed: 1,
                }
            })
        ));
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders(
                input(),
                GlobalFlatFoldabilityLimits {
                    max_exact_operations: 0,
                    ..GlobalFlatFoldabilityLimits::default()
                },
                &[required],
            ),
            Err(RequiredLayerOrderError::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::ExactOperations,
                    ..
                }
            })
        ));
        let mut deadline = DeadlineAtFacewise {
            phase: GlobalFlatFoldabilityPhase::Capturing,
        };
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders_and_observer(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[required],
                &mut deadline,
            ),
            Err(RequiredLayerOrderError::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
            })
        ));
        let mut cancelled = FixedGlobalFlatFoldabilityObserver {
            control: GlobalFlatFoldabilityExecutionControl::Cancelled,
        };
        assert_eq!(
            analyze_global_flat_foldability_with_required_pair_orders_and_observer(
                input(),
                GlobalFlatFoldabilityLimits::default(),
                &[required],
                &mut cancelled,
            ),
            Err(RequiredLayerOrderError::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled
            ))
        );

        let (paper, mut pattern, _) = three_panel_accordion();
        for vertex in &mut pattern.vertices {
            if vertex.position.x.to_bits() == 2.0_f64.to_bits() {
                vertex.position.x = 1.0;
            }
        }
        let topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(1),
            source_revision: REVISION,
            paper: &paper,
            pattern: &pattern,
        })
        .expect("unequal three-panel strip topology");
        let local = analyze_local_flat_foldability(&paper, &pattern);
        let unequal_input = || {
            GlobalFlatFoldabilityInput::current_with_geometry(
                fixed_id::<ProjectId>(1),
                &paper,
                &pattern,
                &topology,
                &local,
            )
        };
        let unequal = analyze_global_flat_foldability(
            unequal_input(),
            GlobalFlatFoldabilityLimits::default(),
        )
        .unwrap()
        .layer_order()
        .unwrap()
        .clone();
        let ordered = unequal
            .face_pair_orders
            .iter()
            .map(|order| {
                if order.lower_face.face_id.canonical_bytes()
                    < order.upper_face.face_id.canonical_bytes()
                {
                    (order.lower_face.face_id, order.upper_face.face_id)
                } else {
                    (order.upper_face.face_id, order.lower_face.face_id)
                }
            })
            .collect::<HashSet<_>>();
        let non_overlap = unequal
            .material_faces
            .iter()
            .enumerate()
            .find_map(|(first, lower)| {
                unequal
                    .material_faces
                    .iter()
                    .skip(first + 1)
                    .find_map(|upper| {
                        let key =
                            if lower.face_id.canonical_bytes() < upper.face_id.canonical_bytes() {
                                (lower.face_id, upper.face_id)
                            } else {
                                (upper.face_id, lower.face_id)
                            };
                        (!ordered.contains(&key)).then_some(RequiredLayerOrderPair {
                            lower_face: *lower,
                            upper_face: *upper,
                        })
                    })
            })
            .expect("unequal strip has one non-overlapping outer-face pair");
        assert!(matches!(
            analyze_global_flat_foldability_with_required_pair_orders(
                unequal_input(),
                GlobalFlatFoldabilityLimits::default(),
                &[non_overlap],
            ),
            Err(RequiredLayerOrderError::NonOverlappingPair { .. })
        ));
    }

    #[test]
    fn constrained_public_api_preserves_search_node_limit_reason() {
        let (paper, mut pattern, _) = three_panel_accordion();
        for edge in &mut pattern.edges {
            if edge.kind == EdgeKind::Valley {
                edge.kind = EdgeKind::Mountain;
            }
        }
        let topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(1),
            source_revision: REVISION,
            paper: &paper,
            pattern: &pattern,
        })
        .expect("same-direction three-panel topology");
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
        let ordinary =
            analyze_global_flat_foldability(input(), GlobalFlatFoldabilityLimits::default())
                .unwrap()
                .layer_order()
                .unwrap()
                .clone();
        assert!(
            ordinary
                .proof_summary
                .expect("facewise proof summary")
                .search_nodes
                > 0,
            "same-direction outer flaps leave one deterministic search choice"
        );
        let hinge = topology
            .hinge_adjacency
            .first()
            .expect("fixture has a trusted hinge");
        let required = ordinary
            .face_pair_orders
            .iter()
            .find(|order| {
                let pair = [order.lower_face.face_id, order.upper_face.face_id];
                pair.contains(&hinge.first) && pair.contains(&hinge.second)
            })
            .map(|order| RequiredLayerOrderPair {
                lower_face: order.lower_face,
                upper_face: order.upper_face,
            })
            .expect("ordinary certificate contains the hinge order");
        let mut search_limit_progress = PhaseRecorder::default();
        let search_limited = analyze_global_flat_foldability_with_required_pair_orders_and_observer(
            input(),
            GlobalFlatFoldabilityLimits {
                max_search_nodes: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
            &[required],
            &mut search_limit_progress,
        );
        assert!(
            matches!(
                search_limited,
                Err(RequiredLayerOrderError::Inconclusive {
                    reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::SearchNodes,
                        limit: 0,
                        observed: 1,
                    },
                })
            ),
            "search-node exhaustion remains precisely classified: {search_limited:?}"
        );
        assert_eq!(
            search_limit_progress.phases.last(),
            Some(&GlobalFlatFoldabilityPhase::Completed),
            "facewise inconclusive results publish a terminal progress update"
        );
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
        let half_edge = |suffix: u64| HalfEdgeRef {
            edge: fixed_id(0x400 + suffix),
            origin: fixed_id(0x500 + suffix * 2),
            destination: fixed_id(0x501 + suffix * 2),
        };
        boundary_work.faces[0].outer.half_edges.push(half_edge(1));
        boundary_work.faces[0].holes.push(BoundaryWalk {
            half_edges: vec![half_edge(2)],
            signed_double_area: -1.0,
        });
        boundary_work.faces[0].seams.push(BoundaryWalk {
            half_edges: vec![half_edge(3)],
            signed_double_area: 0.0,
        });
        let exact_boundary = analyze(
            &boundary_work,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_face_boundary_half_edges: 3,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            exact_boundary.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
                }
            }
        ));
        let over_boundary = analyze(
            &boundary_work,
            &empty_local,
            GlobalFlatFoldabilityLimits {
                max_face_boundary_half_edges: 2,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert!(matches!(
            over_boundary.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::FaceBoundaryHalfEdges,
                    limit: 2,
                    observed: 3,
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
