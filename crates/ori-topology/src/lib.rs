//! Deterministic planar topology derived from an ORIGAMI2 crease pattern.
//!
//! Boundary-only sheets and one-fold legacy snapshots share the same stable
//! identity contract with admitted cellular multi-fold graphs. Cut topology,
//! holes, seams, and non-simple material faces remain explicit diagnostics.

use std::collections::{HashMap, HashSet};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, VertexId,
};
use ori_geometry::{
    polygon_signed_double_area, validate_crease_pattern_with_checkpoint,
    validate_paper_with_checkpoint,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const FACE_KEY_DOMAIN: &[u8] = b"ORIGAMI2_FACE_KEY_V1";
const COOPERATIVE_CHECK_INTERVAL: usize = 64;

// The embedding internals remain crate-private; only validated snapshots and
// stable diagnostics cross the public analysis boundary.
#[allow(dead_code)]
mod admission;
mod closed_cut;
mod closed_cut_topology;
mod cut_material_selection;
#[allow(dead_code)]
mod dcel;
mod fold_graph;
mod local_flat_foldability;
mod single_fold;

use admission::PaperGraphAdmissionError;
use fold_graph::{FoldGraphError, extract_fold_graph_snapshot_with_checkpoint};
use single_fold::{SingleFoldError, extract_single_fold_faces};

pub use closed_cut::{
    CLOSED_CUT_LOOP_DIAGNOSTIC_MODEL_ID_V1, ClosedCutLoopDiagnosticErrorV1,
    ClosedCutLoopDiagnosticLimitsV1, ClosedCutLoopDiagnosticV1,
    DEFAULT_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1, MAX_CLOSED_CUT_DIAGNOSTIC_EDGES_V1,
    MAX_CLOSED_CUT_DIAGNOSTIC_INTERSECTION_TESTS_V1, MAX_CLOSED_CUT_DIAGNOSTIC_VERTICES_V1,
    diagnose_closed_cut_loops_v1,
};
pub use closed_cut_topology::{
    CLOSED_CUT_TOPOLOGY_SNAPSHOT_MODEL_ID_V1, ClosedCutTopologySnapshotDiagnosticV1,
    ClosedCutTopologySnapshotErrorV1, diagnose_closed_cut_topology_snapshot_v1,
};
pub use cut_material_selection::{
    CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1,
    CUT_MATERIAL_REMOVAL_PLAN_DIAGNOSTIC_MODEL_ID_V1, CutMaterialComponentSelectionDiagnosticV1,
    CutMaterialComponentSelectionErrorV1, CutMaterialComponentSelectionV1,
    CutMaterialRemovalPlanDiagnosticV1, CutMaterialRemovalPlanErrorV1,
    EFFECTIVE_CUT_MATERIAL_SNAPSHOT_DIAGNOSTIC_MODEL_ID_V1,
    EffectiveCutMaterialSnapshotDiagnosticV1, EffectiveCutMaterialSnapshotErrorV1,
    diagnose_cut_material_component_selection_v1, diagnose_cut_material_removal_plan_v1,
    diagnose_effective_cut_material_snapshot_v1,
};

pub use local_flat_foldability::{
    ASSIGNED_LOCAL_SUFFICIENCY_MODEL_ID_V1, AssignedCrimpReductionV1,
    AssignedLocalSufficiencyBatchV1, AssignedLocalSufficiencyLimitsV1,
    AssignedLocalSufficiencyReasonV1, AssignedLocalSufficiencyV1, LocalFlatFoldabilityModel,
    LocalFlatFoldabilityReport, LocalFlatFoldabilityReportStatus, LocalFoldabilityConditionStatus,
    LocalFoldabilityReason, LocalVertexFoldability, LocalVertexFoldabilityVerdict,
    MAX_EXACT_FOLD_DEGREE, analyze_local_flat_foldability,
    analyze_local_flat_foldability_with_checkpoint, prove_all_assigned_local_sufficiency_v1,
    prove_assigned_local_sufficiency_v1,
};

/// Result requested by a cooperative preprocessing checkpoint.
///
/// Cancellation and deadline expiry remain distinct so a native caller can
/// preserve the difference between an explicit user action and a bounded
/// analysis that returned `unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooperativeAnalysisCheckpoint {
    Continue,
    Cancelled,
    DeadlineReached,
}

/// Stable reason why cooperative topology preprocessing stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CooperativeAnalysisAbort {
    #[error("topology preprocessing was cancelled")]
    Cancelled,
    #[error("topology preprocessing reached its deadline")]
    DeadlineReached,
}

pub(crate) fn run_cooperative_checkpoint<F>(
    checkpoint: &mut F,
) -> Result<(), CooperativeAnalysisAbort>
where
    F: FnMut() -> CooperativeAnalysisCheckpoint + ?Sized,
{
    match checkpoint() {
        CooperativeAnalysisCheckpoint::Continue => Ok(()),
        CooperativeAnalysisCheckpoint::Cancelled => Err(CooperativeAnalysisAbort::Cancelled),
        CooperativeAnalysisCheckpoint::DeadlineReached => {
            Err(CooperativeAnalysisAbort::DeadlineReached)
        }
    }
}

pub(crate) fn poll_cooperative_checkpoint<F>(
    checkpoint: &mut F,
    iteration: usize,
) -> Result<(), CooperativeAnalysisAbort>
where
    F: FnMut() -> CooperativeAnalysisCheckpoint + ?Sized,
{
    if iteration.is_multiple_of(COOPERATIVE_CHECK_INTERVAL) {
        run_cooperative_checkpoint(checkpoint)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CooperativeOperationError<E> {
    Aborted(CooperativeAnalysisAbort),
    Operation(E),
}

impl<E> From<CooperativeAnalysisAbort> for CooperativeOperationError<E> {
    fn from(abort: CooperativeAnalysisAbort) -> Self {
        Self::Aborted(abort)
    }
}

impl<E> CooperativeOperationError<E> {
    pub(crate) fn map_operation<T>(self, map: impl FnOnce(E) -> T) -> CooperativeOperationError<T> {
        match self {
            Self::Aborted(abort) => CooperativeOperationError::Aborted(abort),
            Self::Operation(error) => CooperativeOperationError::Operation(map(error)),
        }
    }
}

/// Immutable input used to derive a topology snapshot.
#[derive(Debug, Clone, Copy)]
pub struct FaceExtractionInput<'a> {
    pub identity_namespace: ProjectId,
    pub source_revision: u64,
    pub paper: &'a Paper,
    pub pattern: &'a CreasePattern,
}

/// Canonical SHA-256 digest of one material face boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FaceKey(pub [u8; 32]);

/// Fail-closed reason why a canonical material-face key could not be derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CanonicalFaceKeyError {
    #[error("the face boundary length cannot be represented canonically")]
    BoundaryLengthUnrepresentable,
    #[error("memory for canonical face-key derivation could not be reserved")]
    AllocationFailed,
}

/// Derives the canonical face key for a cyclic half-edge boundary.
///
/// The cycle start is canonicalized before hashing, so equivalent rotations
/// produce one key. Direction is intentionally preserved because it carries
/// the material-side orientation.
pub fn canonical_face_key(half_edges: &[HalfEdgeRef]) -> Result<FaceKey, CanonicalFaceKeyError> {
    let length = canonical_face_key_length(half_edges.len())?;
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(half_edges.len())
        .map_err(|_| CanonicalFaceKeyError::AllocationFailed)?;
    canonical.extend_from_slice(half_edges);
    canonicalize_cycle(&mut canonical);
    let mut hasher = Sha256::new();
    hasher.update(FACE_KEY_DOMAIN);
    hasher.update(length.to_be_bytes());
    for half_edge in &canonical {
        hasher.update(half_edge_token(half_edge));
    }
    Ok(FaceKey(hasher.finalize().into()))
}

/// One directed occurrence of a source edge in a face boundary walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HalfEdgeRef {
    pub edge: EdgeId,
    pub origin: VertexId,
    pub destination: VertexId,
}

/// A counter-clockwise walk whose material lies on its left side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryWalk {
    pub half_edges: Vec<HalfEdgeRef>,
    pub signed_double_area: f64,
}

/// One connected two-dimensional material region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub id: FaceId,
    pub key: FaceKey,
    pub outer: BoundaryWalk,
    /// Clockwise material exclusions owned by this face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<BoundaryWalk>,
    /// Zero-area open or branched cut walks embedded in this face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seams: Vec<BoundaryWalk>,
    /// Independently rounded binary64 measurement. Topology never relies on
    /// this value, and sums of several face measurements may differ from the
    /// independently rounded paper area by a final-rounding unit.
    pub area: f64,
}

/// Mountain/valley meaning carried by a hinge without affecting face identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldAssignment {
    Mountain,
    Valley,
}

/// The relation of a source edge to the extracted material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EdgeIncidence {
    Boundary {
        material: FaceId,
    },
    Hinge {
        /// Material on the left of the edge's canonical VertexId direction.
        left: FaceId,
        /// Material on the right of the edge's canonical VertexId direction.
        right: FaceId,
        assignment: FoldAssignment,
    },
    Cut {
        left: FaceId,
        right: FaceId,
    },
    AuxiliaryIgnored,
}

/// One fold hinge connecting two distinct material faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceAdjacency {
    pub edge: EdgeId,
    /// The incident face with the smaller canonical [`FaceKey`].
    pub first: FaceId,
    /// The incident face with the larger canonical [`FaceKey`].
    pub second: FaceId,
    pub assignment: FoldAssignment,
}

/// Canonical SHA-256 digest of one connected material component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MaterialComponentKey(pub [u8; 32]);

/// Faces still belonging to one connected piece of the original sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub key: MaterialComponentKey,
    pub sheet_origin: ProjectId,
    pub faces: Vec<FaceId>,
}

/// Canonical, revision-labelled output of face extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopologySnapshot {
    pub source_revision: u64,
    pub faces: Vec<Face>,
    pub edge_incidence: Vec<(EdgeId, EdgeIncidence)>,
    #[serde(default)]
    pub hinge_adjacency: Vec<FaceAdjacency>,
    #[serde(default)]
    pub material_components: Vec<MaterialComponent>,
}

pub(crate) fn connected_sheet_component(
    sheet_origin: ProjectId,
    faces: &[Face],
) -> MaterialComponent {
    let mut keyed_faces = faces
        .iter()
        .map(|face| (face.key, face.id))
        .collect::<Vec<_>>();
    keyed_faces.sort_by_key(|(key, _)| *key);
    let mut hasher = Sha256::new();
    hasher.update(b"ORIGAMI2_MATERIAL_COMPONENT_KEY_V1");
    hasher.update(sheet_origin.canonical_bytes());
    hasher.update((keyed_faces.len() as u64).to_be_bytes());
    for (key, _) in &keyed_faces {
        hasher.update(key.0);
    }
    MaterialComponent {
        key: MaterialComponentKey(hasher.finalize().into()),
        sheet_origin,
        faces: keyed_faces.into_iter().map(|(_, id)| id).collect(),
    }
}

/// Whether a topology issue permits downstream simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyIssueSeverity {
    Warning,
    BlocksSimulation,
    Fatal,
}

/// Machine-readable reason that a topology snapshot was not produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TopologyIssueKind {
    DuplicateVertexId {
        vertex: VertexId,
    },
    DuplicateEdgeId {
        edge: EdgeId,
    },
    InvalidPaper {
        issue_count: usize,
    },
    InvalidCreasePattern {
        issue_count: usize,
    },
    UnsupportedActiveEdge {
        edge: EdgeId,
        edge_kind: EdgeKind,
    },
    /// Retained for compatibility with reports produced before general fold
    /// graphs were admitted by the public analysis route.
    TooManyActiveFoldEdges {
        edges: Vec<EdgeId>,
    },
    ActiveEdgeOutsidePaper {
        edge: EdgeId,
    },
    DisconnectedFoldGraph {
        edge: EdgeId,
    },
    NonSeparatingFold {
        edge: EdgeId,
    },
    UnsupportedFoldGraph {
        edge: EdgeId,
    },
    InvalidEdgeIncidence {
        edge: EdgeId,
    },
    FoldEndpointNotOnBoundary {
        edge: EdgeId,
        vertex: VertexId,
    },
    UnsupportedAdjacentBoundaryFold {
        edge: EdgeId,
    },
    UnsupportedNonConvexFoldSheet {
        edge: EdgeId,
        vertex: VertexId,
    },
    DegenerateFoldFace {
        edge: EdgeId,
    },
    UnrepresentableFaceArea,
    InternalBoundaryResolution,
}

/// One stable diagnostic returned by face analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyIssue {
    pub severity: TopologyIssueSeverity,
    pub kind: TopologyIssueKind,
}

/// Diagnostic result used by the editor while the document may be invalid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceExtractionReport {
    pub snapshot: Option<TopologySnapshot>,
    pub issues: Vec<TopologyIssue>,
}

/// Strict extraction rejected an input that is not safe for simulation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("face extraction was rejected by {issue_count} topology issue(s)")]
pub struct FaceExtractionRejected {
    issue_count: usize,
    pub issues: Vec<TopologyIssue>,
}

impl FaceExtractionRejected {
    #[must_use]
    pub fn issue_count(&self) -> usize {
        self.issue_count
    }
}

/// Analyzes the document without mutating it.
///
/// Boundary-only sheets and the established single-chord subset retain their
/// legacy extraction path. Connected, cut-free planar graphs with two or more
/// mountain/valley edges use the deterministic half-edge extractor. Graphs
/// that cannot be represented as simple material faces remain explicitly
/// blocked; silently treating them as auxiliary would create unsafe 3D input.
#[must_use]
pub fn analyze_faces(input: FaceExtractionInput<'_>) -> FaceExtractionReport {
    let mut checkpoint = || CooperativeAnalysisCheckpoint::Continue;
    match analyze_faces_with_checkpoint(input, &mut checkpoint) {
        Ok(report) => report,
        Err(_) => unreachable!("the no-op topology checkpoint cannot abort"),
    }
}

/// Analyzes the document with bounded-frequency cooperative interruption.
///
/// The callback is polled at phase boundaries and at least once every 64
/// records in the vertex/edge preprocessing loops. Existing callers that do
/// not need interruption can continue to use [`analyze_faces`].
pub fn analyze_faces_with_checkpoint<F>(
    input: FaceExtractionInput<'_>,
    checkpoint: &mut F,
) -> Result<FaceExtractionReport, CooperativeAnalysisAbort>
where
    F: FnMut() -> CooperativeAnalysisCheckpoint + ?Sized,
{
    run_cooperative_checkpoint(checkpoint)?;

    let mut vertex_ids = HashSet::with_capacity(input.pattern.vertices.len());
    let mut duplicate_vertex = None;
    for (index, vertex) in input.pattern.vertices.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        if !vertex_ids.insert(vertex.id)
            && duplicate_vertex.is_none_or(|current: VertexId| {
                vertex.id.canonical_bytes() < current.canonical_bytes()
            })
        {
            duplicate_vertex = Some(vertex.id);
        }
    }
    if let Some(vertex) = duplicate_vertex {
        return Ok(rejected(
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::DuplicateVertexId { vertex },
        ));
    }

    let mut edge_ids = HashSet::with_capacity(input.pattern.edges.len());
    let mut duplicate_edge = None;
    for (index, edge) in input.pattern.edges.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        if !edge_ids.insert(edge.id)
            && duplicate_edge
                .is_none_or(|current: EdgeId| edge.id.canonical_bytes() < current.canonical_bytes())
        {
            duplicate_edge = Some(edge.id);
        }
    }
    if let Some(edge) = duplicate_edge {
        return Ok(rejected(
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::DuplicateEdgeId { edge },
        ));
    }

    let paper_validation = {
        let mut geometry_checkpoint = || run_cooperative_checkpoint(checkpoint);
        validate_paper_with_checkpoint(input.paper, input.pattern, &mut geometry_checkpoint)?
    };
    run_cooperative_checkpoint(checkpoint)?;
    if !paper_validation.is_valid() {
        return Ok(rejected(
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InvalidPaper {
                issue_count: paper_validation.issues.len(),
            },
        ));
    }

    let mut first_cut = None;
    for (index, edge) in input.pattern.edges.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        if edge.kind == EdgeKind::Cut
            && first_cut.is_none_or(|current: &Edge| {
                edge.id.canonical_bytes() < current.id.canonical_bytes()
            })
        {
            first_cut = Some(edge);
        }
    }
    if let Some(edge) = first_cut {
        return Ok(rejected(
            TopologyIssueSeverity::BlocksSimulation,
            TopologyIssueKind::UnsupportedActiveEdge {
                edge: edge.id,
                edge_kind: edge.kind,
            },
        ));
    }

    let mut fold_edges = Vec::new();
    for (index, edge) in input.pattern.edges.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        if matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley) {
            fold_edges.push(edge);
        }
    }
    fold_edges.sort_by_key(|edge| edge.id.canonical_bytes());
    if !fold_edges.is_empty() {
        let issue_count = validate_fold_participants(input.pattern, checkpoint)?;
        if issue_count != 0 {
            return Ok(rejected(
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::InvalidCreasePattern { issue_count },
            ));
        }
    }
    run_cooperative_checkpoint(checkpoint)?;
    let extracted = match fold_edges.len() {
        0 => extract_boundary_face(input).map_err(|kind| (TopologyIssueSeverity::Fatal, kind)),
        1 => {
            let fold = fold_edges[0];
            let mut endpoints = [fold.start, fold.end];
            endpoints.sort_by_key(VertexId::canonical_bytes);
            for vertex in endpoints {
                if !input.paper.boundary_vertices.contains(&vertex) {
                    return Ok(rejected(
                        TopologyIssueSeverity::BlocksSimulation,
                        TopologyIssueKind::FoldEndpointNotOnBoundary {
                            edge: fold.id,
                            vertex,
                        },
                    ));
                }
            }
            extract_single_fold_snapshot(input, fold)
        }
        _ => match extract_fold_graph_snapshot_with_checkpoint(input, checkpoint) {
            Ok(snapshot) => Ok(snapshot),
            Err(CooperativeOperationError::Aborted(abort)) => return Err(abort),
            Err(CooperativeOperationError::Operation(error)) => Err(map_fold_graph_error(error)),
        },
    };
    run_cooperative_checkpoint(checkpoint)?;

    Ok(match extracted {
        Ok(snapshot) => FaceExtractionReport {
            snapshot: Some(snapshot),
            issues: Vec::new(),
        },
        Err((severity, kind)) => rejected(severity, kind),
    })
}

fn map_fold_graph_error(error: FoldGraphError) -> (TopologyIssueSeverity, TopologyIssueKind) {
    match error {
        FoldGraphError::Admission(admission) => match admission {
            PaperGraphAdmissionError::DuplicateVertexId { vertex } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::DuplicateVertexId { vertex },
            ),
            PaperGraphAdmissionError::DuplicateEdgeId { edge } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::DuplicateEdgeId { edge },
            ),
            PaperGraphAdmissionError::InvalidPaper { issue_count } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::InvalidPaper { issue_count },
            ),
            PaperGraphAdmissionError::CutNotAllowed { edge } => (
                TopologyIssueSeverity::BlocksSimulation,
                TopologyIssueKind::UnsupportedActiveEdge {
                    edge,
                    edge_kind: EdgeKind::Cut,
                },
            ),
            PaperGraphAdmissionError::InvalidParticipantPattern { issue_count } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::InvalidCreasePattern { issue_count },
            ),
            PaperGraphAdmissionError::ActiveEdgeOutsidePaper { edge } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::ActiveEdgeOutsidePaper { edge },
            ),
            PaperGraphAdmissionError::ContainmentPredicateFailure { .. }
            | PaperGraphAdmissionError::ContainmentInvariantViolation { .. }
            | PaperGraphAdmissionError::InternalBoundaryResolution
            | PaperGraphAdmissionError::Dcel(_) => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::InternalBoundaryResolution,
            ),
        },
        FoldGraphError::DisconnectedParticipantGraph { edge } => (
            TopologyIssueSeverity::BlocksSimulation,
            TopologyIssueKind::DisconnectedFoldGraph { edge },
        ),
        FoldGraphError::NonSeparatingFold { edge } => (
            TopologyIssueSeverity::BlocksSimulation,
            TopologyIssueKind::NonSeparatingFold { edge },
        ),
        FoldGraphError::ExteriorFoldIncidence { edge } => (
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InvalidEdgeIncidence { edge },
        ),
        FoldGraphError::UnexpectedWalkOrientation { edge }
        | FoldGraphError::UnsupportedNonSimpleFace { edge } => (
            TopologyIssueSeverity::BlocksSimulation,
            TopologyIssueKind::UnsupportedFoldGraph { edge },
        ),
        FoldGraphError::FaceBuild(kind) => (TopologyIssueSeverity::Fatal, kind),
        FoldGraphError::InternalInvariant => (
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InternalBoundaryResolution,
        ),
    }
}

/// Returns a topology snapshot only when no simulation-blocking issue exists.
pub fn extract_faces_strict(
    input: FaceExtractionInput<'_>,
) -> Result<TopologySnapshot, FaceExtractionRejected> {
    let report = analyze_faces(input);
    if report.issues.iter().any(|issue| {
        matches!(
            issue.severity,
            TopologyIssueSeverity::BlocksSimulation | TopologyIssueSeverity::Fatal
        )
    }) {
        return Err(FaceExtractionRejected {
            issue_count: report.issues.len(),
            issues: report.issues,
        });
    }
    report.snapshot.ok_or_else(|| FaceExtractionRejected {
        issue_count: 1,
        issues: vec![TopologyIssue {
            severity: TopologyIssueSeverity::Fatal,
            kind: TopologyIssueKind::InternalBoundaryResolution,
        }],
    })
}

fn rejected(severity: TopologyIssueSeverity, kind: TopologyIssueKind) -> FaceExtractionReport {
    FaceExtractionReport {
        snapshot: None,
        issues: vec![TopologyIssue { severity, kind }],
    }
}

/// Validates only records that can affect material topology.
///
/// Auxiliary construction geometry is deliberately excluded: it may cross,
/// overlap, or reference incomplete draft vertices without changing a face.
/// Global record-ID uniqueness is checked before this function, so excluding
/// auxiliary records cannot hide an identity collision.
fn validate_fold_participants<F>(
    pattern: &CreasePattern,
    checkpoint: &mut F,
) -> Result<usize, CooperativeAnalysisAbort>
where
    F: FnMut() -> CooperativeAnalysisCheckpoint + ?Sized,
{
    let mut participant_edges = Vec::new();
    for (index, edge) in pattern.edges.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        if matches!(
            edge.kind,
            EdgeKind::Boundary | EdgeKind::Mountain | EdgeKind::Valley
        ) {
            participant_edges.push(edge.clone());
        }
    }
    let mut participant_vertices = HashSet::new();
    for (index, edge) in participant_edges.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        participant_vertices.extend([edge.start, edge.end]);
    }
    let mut vertices = Vec::new();
    for (index, vertex) in pattern.vertices.iter().enumerate() {
        poll_cooperative_checkpoint(checkpoint, index)?;
        if participant_vertices.contains(&vertex.id) {
            vertices.push(vertex.clone());
        }
    }

    let issue_count = {
        let mut geometry_checkpoint = || run_cooperative_checkpoint(checkpoint);
        validate_crease_pattern_with_checkpoint(
            &CreasePattern {
                vertices,
                edges: participant_edges,
            },
            &mut geometry_checkpoint,
        )?
        .issues
        .len()
    };
    run_cooperative_checkpoint(checkpoint)?;
    Ok(issue_count)
}

fn extract_single_fold_snapshot(
    input: FaceExtractionInput<'_>,
    fold: &Edge,
) -> Result<TopologySnapshot, (TopologyIssueSeverity, TopologyIssueKind)> {
    let extracted = extract_single_fold_faces(input.paper, input.pattern, fold).map_err(
        |error| match error {
            SingleFoldError::NonConvex { vertex } => (
                TopologyIssueSeverity::BlocksSimulation,
                TopologyIssueKind::UnsupportedNonConvexFoldSheet {
                    edge: fold.id,
                    vertex,
                },
            ),
            SingleFoldError::AdjacentEndpoints { .. } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::UnsupportedAdjacentBoundaryFold { edge: fold.id },
            ),
            SingleFoldError::DegenerateFace { .. } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::DegenerateFoldFace { edge: fold.id },
            ),
            SingleFoldError::UnresolvedEdge { .. } => (
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::InternalBoundaryResolution,
            ),
        },
    )?;
    if extracted.fold != fold.id
        || extracted.canonical_start.canonical_bytes() >= extracted.canonical_end.canonical_bytes()
    {
        return Err((
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InternalBoundaryResolution,
        ));
    }

    let left_face = face_from_walk(input.identity_namespace, extracted.left)
        .map_err(|kind| (TopologyIssueSeverity::Fatal, kind))?;
    let right_face = face_from_walk(input.identity_namespace, extracted.right)
        .map_err(|kind| (TopologyIssueSeverity::Fatal, kind))?;
    if left_face.key == right_face.key || left_face.id == right_face.id {
        return Err((
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InternalBoundaryResolution,
        ));
    }

    let left = left_face.id;
    let right = right_face.id;
    if !walk_has_directed_edge(
        &left_face.outer,
        fold.id,
        extracted.canonical_start,
        extracted.canonical_end,
    ) || !walk_has_directed_edge(
        &right_face.outer,
        fold.id,
        extracted.canonical_end,
        extracted.canonical_start,
    ) {
        return Err((
            TopologyIssueSeverity::Fatal,
            TopologyIssueKind::InternalBoundaryResolution,
        ));
    }

    let assignment = match fold.kind {
        EdgeKind::Mountain => FoldAssignment::Mountain,
        EdgeKind::Valley => FoldAssignment::Valley,
        _ => {
            return Err((
                TopologyIssueSeverity::Fatal,
                TopologyIssueKind::InternalBoundaryResolution,
            ));
        }
    };

    let mut faces = vec![left_face, right_face];
    faces.sort_by_key(|face| face.key);
    let mut edge_incidence = Vec::with_capacity(input.pattern.edges.len());
    for edge in &input.pattern.edges {
        let incidence = match edge.kind {
            EdgeKind::Boundary => {
                let mut materials = faces.iter().filter_map(|face| {
                    face.outer
                        .half_edges
                        .iter()
                        .any(|half_edge| half_edge.edge == edge.id)
                        .then_some(face.id)
                });
                let Some(material) = materials.next() else {
                    return Err((
                        TopologyIssueSeverity::Fatal,
                        TopologyIssueKind::InternalBoundaryResolution,
                    ));
                };
                if materials.next().is_some() {
                    return Err((
                        TopologyIssueSeverity::Fatal,
                        TopologyIssueKind::InternalBoundaryResolution,
                    ));
                }
                EdgeIncidence::Boundary { material }
            }
            EdgeKind::Mountain | EdgeKind::Valley if edge.id == fold.id => EdgeIncidence::Hinge {
                left,
                right,
                assignment,
            },
            EdgeKind::Auxiliary => EdgeIncidence::AuxiliaryIgnored,
            EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut => {
                return Err((
                    TopologyIssueSeverity::Fatal,
                    TopologyIssueKind::InternalBoundaryResolution,
                ));
            }
        };
        edge_incidence.push((edge.id, incidence));
    }
    edge_incidence.sort_by_key(|(edge, _)| edge.canonical_bytes());

    let hinge_adjacency = vec![FaceAdjacency {
        edge: fold.id,
        first: faces[0].id,
        second: faces[1].id,
        assignment,
    }];
    Ok(TopologySnapshot {
        source_revision: input.source_revision,
        material_components: vec![connected_sheet_component(input.identity_namespace, &faces)],
        faces,
        edge_incidence,
        hinge_adjacency,
    })
}

fn face_from_walk(
    identity_namespace: ProjectId,
    outer: BoundaryWalk,
) -> Result<Face, TopologyIssueKind> {
    let area = outer.signed_double_area * 0.5;
    if area <= 0.0 || !area.is_finite() {
        return Err(TopologyIssueKind::UnrepresentableFaceArea);
    }
    let key = canonical_face_key(&outer.half_edges)
        .map_err(|_| TopologyIssueKind::InternalBoundaryResolution)?;
    Ok(Face {
        id: FaceId::derive_v5(identity_namespace, &key.0),
        key,
        outer,
        holes: Vec::new(),
        seams: Vec::new(),
        area,
    })
}

fn walk_has_directed_edge(
    walk: &BoundaryWalk,
    edge: EdgeId,
    origin: VertexId,
    destination: VertexId,
) -> bool {
    walk.half_edges.iter().any(|half_edge| {
        half_edge.edge == edge && half_edge.origin == origin && half_edge.destination == destination
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UndirectedEndpoints {
    first: VertexId,
    second: VertexId,
}

impl UndirectedEndpoints {
    fn new(first: VertexId, second: VertexId) -> Self {
        if first.canonical_bytes() <= second.canonical_bytes() {
            Self { first, second }
        } else {
            Self {
                first: second,
                second: first,
            }
        }
    }
}

/// Read-only resolution indexes for the boundary-only extraction stage.
///
/// Construction happens only after the public validation stages. The
/// collision checks remain fail-closed so a future caller cannot accidentally
/// make face identity depend on whichever duplicate record was inserted first.
#[derive(Debug)]
struct BoundaryIndex {
    positions: HashMap<VertexId, Point2>,
    boundary_edges: HashMap<UndirectedEndpoints, EdgeId>,
}

impl BoundaryIndex {
    fn build(pattern: &CreasePattern) -> Result<Self, TopologyIssueKind> {
        let mut positions = HashMap::with_capacity(pattern.vertices.len());
        for vertex in &pattern.vertices {
            if positions.insert(vertex.id, vertex.position).is_some() {
                return Err(TopologyIssueKind::InternalBoundaryResolution);
            }
        }

        let boundary_count = pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count();
        let mut boundary_edges = HashMap::with_capacity(boundary_count);
        for edge in pattern
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
        {
            let endpoints = UndirectedEndpoints::new(edge.start, edge.end);
            if boundary_edges.insert(endpoints, edge.id).is_some() {
                return Err(TopologyIssueKind::InternalBoundaryResolution);
            }
        }

        Ok(Self {
            positions,
            boundary_edges,
        })
    }

    fn position(&self, vertex: VertexId) -> Option<Point2> {
        self.positions.get(&vertex).copied()
    }

    fn boundary_edge(&self, first: VertexId, second: VertexId) -> Option<EdgeId> {
        self.boundary_edges
            .get(&UndirectedEndpoints::new(first, second))
            .copied()
    }
}

fn extract_boundary_face(
    input: FaceExtractionInput<'_>,
) -> Result<TopologySnapshot, TopologyIssueKind> {
    let boundary_index = BoundaryIndex::build(input.pattern)?;
    let mut boundary_vertices = input.paper.boundary_vertices.clone();
    let boundary_positions = boundary_vertices
        .iter()
        .map(|vertex| boundary_index.position(*vertex))
        .collect::<Option<Vec<_>>>()
        .ok_or(TopologyIssueKind::InternalBoundaryResolution)?;
    let signed_double_area = polygon_signed_double_area(&boundary_positions)
        .map_err(|_| TopologyIssueKind::InternalBoundaryResolution)?;
    if signed_double_area == 0.0 {
        return Err(TopologyIssueKind::InternalBoundaryResolution);
    }
    if signed_double_area < 0.0 {
        boundary_vertices.reverse();
    }

    let mut half_edges = Vec::with_capacity(boundary_vertices.len());
    for vertex_index in 0..boundary_vertices.len() {
        let origin = boundary_vertices[vertex_index];
        let destination = boundary_vertices[(vertex_index + 1) % boundary_vertices.len()];
        let edge = boundary_index
            .boundary_edge(origin, destination)
            .ok_or(TopologyIssueKind::InternalBoundaryResolution)?;
        half_edges.push(HalfEdgeRef {
            edge,
            origin,
            destination,
        });
    }
    canonicalize_cycle(&mut half_edges);

    let key = canonical_face_key(&half_edges)
        .map_err(|_| TopologyIssueKind::InternalBoundaryResolution)?;
    let face_id = FaceId::derive_v5(input.identity_namespace, &key.0);
    let area = signed_double_area.abs() * 0.5;
    if area == 0.0 || !area.is_finite() {
        return Err(TopologyIssueKind::UnrepresentableFaceArea);
    }
    let face = Face {
        id: face_id,
        key,
        outer: BoundaryWalk {
            half_edges,
            signed_double_area: signed_double_area.abs(),
        },
        holes: Vec::new(),
        seams: Vec::new(),
        area,
    };

    let mut edge_incidence = input
        .pattern
        .edges
        .iter()
        .map(|edge| {
            let incidence = match edge.kind {
                EdgeKind::Boundary => EdgeIncidence::Boundary { material: face_id },
                EdgeKind::Auxiliary => EdgeIncidence::AuxiliaryIgnored,
                EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut => return None,
            };
            Some((edge.id, incidence))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(TopologyIssueKind::InternalBoundaryResolution)?;
    edge_incidence.sort_by_key(|(edge, _)| edge.canonical_bytes());

    Ok(TopologySnapshot {
        source_revision: input.source_revision,
        material_components: vec![connected_sheet_component(
            input.identity_namespace,
            std::slice::from_ref(&face),
        )],
        faces: vec![face],
        edge_incidence,
        hinge_adjacency: Vec::new(),
    })
}

fn canonicalize_cycle(half_edges: &mut [HalfEdgeRef]) {
    // Each directed source edge occurs at most once in a valid DCEL walk, so
    // every token is unique. The rotation beginning with the minimum token is
    // therefore the lexicographically minimum cyclic rotation.
    if let Some((best, _)) = half_edges
        .iter()
        .enumerate()
        .min_by_key(|(_, half_edge)| half_edge_token(half_edge))
    {
        half_edges.rotate_left(best);
    }
}

fn half_edge_token(half_edge: &HalfEdgeRef) -> [u8; 48] {
    let mut token = [0_u8; 48];
    token[..16].copy_from_slice(&half_edge.edge.canonical_bytes());
    token[16..32].copy_from_slice(&half_edge.origin.canonical_bytes());
    token[32..].copy_from_slice(&half_edge.destination.canonical_bytes());
    token
}

fn canonical_face_key_length(length: usize) -> Result<u64, CanonicalFaceKeyError> {
    u64::try_from(length).map_err(|_| CanonicalFaceKeyError::BoundaryLengthUnrepresentable)
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, Paper, Point2, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn polygon_fixture(points: &[Point2]) -> (ProjectId, Paper, CreasePattern) {
        let namespace = ProjectId::new();
        let vertices = points
            .iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position: *position,
            })
            .collect::<Vec<_>>();
        let boundary_vertices = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect();
        let paper = Paper {
            boundary_vertices,
            ..Paper::default()
        };
        (namespace, paper, CreasePattern { vertices, edges })
    }

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID fixture")
    }

    fn subdivided_rectangle_fixture(edge_count: usize) -> (ProjectId, Paper, CreasePattern) {
        assert!(edge_count >= 4 && edge_count.is_multiple_of(2));
        let width = (edge_count - 2) / 2;
        let mut points = Vec::with_capacity(edge_count);
        points.extend((0..width).map(|x| Point2::new(x as f64, 0.0)));
        points.push(Point2::new(width as f64, 0.0));
        points.push(Point2::new(width as f64, 1.0));
        points.extend((1..width).rev().map(|x| Point2::new(x as f64, 1.0)));
        points.push(Point2::new(0.0, 1.0));
        assert_eq!(points.len(), edge_count);

        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, position)| Vertex {
                id: fixed_id(0x10_0000 + index as u64),
                position,
            })
            .collect::<Vec<_>>();
        let boundary_vertices = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..edge_count)
            .map(|index| Edge {
                id: fixed_id(0x20_0000 + index as u64),
                start: vertices[index].id,
                end: vertices[(index + 1) % edge_count].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for edge in edges.iter_mut().step_by(2) {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        edges.reverse();

        (
            fixed_id(0x30_0000),
            Paper {
                boundary_vertices,
                ..Paper::default()
            },
            CreasePattern { vertices, edges },
        )
    }

    #[test]
    fn public_canonical_face_key_matches_snapshot_and_ignores_cycle_start() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 3.0),
            Point2::new(0.0, 3.0),
        ]);
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 9,
            paper: &paper,
            pattern: &pattern,
        });
        let face = &report.snapshot.expect("boundary topology").faces[0];
        assert_eq!(canonical_face_key(&face.outer.half_edges), Ok(face.key));
        let mut rotated = face.outer.half_edges.clone();
        rotated.rotate_left(2);
        assert_eq!(canonical_face_key(&rotated), Ok(face.key));
    }

    #[test]
    fn canonical_face_key_length_conversion_is_checked() {
        if usize::BITS > u64::BITS {
            assert_eq!(
                canonical_face_key_length(usize::MAX),
                Err(CanonicalFaceKeyError::BoundaryLengthUnrepresentable)
            );
        } else {
            assert_eq!(canonical_face_key_length(usize::MAX), Ok(usize::MAX as u64));
        }
    }

    fn strict(namespace: ProjectId, paper: &Paper, pattern: &CreasePattern) -> TopologySnapshot {
        extract_faces_strict(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 7,
            paper,
            pattern,
        })
        .expect("extract boundary-only topology")
    }

    #[test]
    fn square_extracts_one_face_and_sorted_boundary_incidence() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ]);

        let snapshot = strict(namespace, &paper, &pattern);

        assert_eq!(snapshot.source_revision, 7);
        assert_eq!(snapshot.faces.len(), 1);
        assert_eq!(snapshot.faces[0].area, 100.0);
        assert_eq!(snapshot.faces[0].outer.signed_double_area, 200.0);
        assert_eq!(snapshot.faces[0].outer.half_edges.len(), 4);
        assert!(snapshot.edge_incidence.iter().all(|(_, incidence)| {
            matches!(incidence, EdgeIncidence::Boundary { material } if *material == snapshot.faces[0].id)
        }));
        assert!(
            snapshot
                .edge_incidence
                .windows(2)
                .all(|pair| pair[0].0.canonical_bytes() < pair[1].0.canonical_bytes())
        );
    }

    #[test]
    fn stable_face_identity_matches_the_v1_golden_vector() {
        let namespace = fixed_id(1);
        let vertex_ids = [
            fixed_id(0x101),
            fixed_id(0x102),
            fixed_id(0x103),
            fixed_id(0x104),
        ];
        let edge_ids = [
            fixed_id(0x201),
            fixed_id(0x202),
            fixed_id(0x203),
            fixed_id(0x204),
        ];
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 2.0),
        ];
        let vertices = vertex_ids
            .iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id: *id, position })
            .collect::<Vec<_>>();
        let edges = (0..4)
            .map(|index| Edge {
                id: edge_ids[index],
                start: vertex_ids[index],
                end: vertex_ids[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect();
        let paper = Paper {
            boundary_vertices: vertex_ids.to_vec(),
            ..Paper::default()
        };
        let pattern = CreasePattern { vertices, edges };

        let face = &strict(namespace, &paper, &pattern).faces[0];

        assert_eq!(
            (face.key.0, face.id.canonical_bytes()),
            (
                [
                    34, 113, 28, 115, 75, 184, 146, 47, 25, 14, 93, 61, 99, 234, 88, 143, 156, 135,
                    97, 14, 14, 143, 237, 92, 65, 97, 20, 62, 98, 234, 24, 11,
                ],
                [
                    167, 199, 122, 53, 123, 194, 89, 121, 169, 212, 34, 191, 48, 106, 196, 29,
                ],
            )
        );
    }

    #[test]
    fn undirected_boundary_keys_ignore_endpoint_direction() {
        let first = fixed_id(0x401);
        let second = fixed_id(0x402);

        assert_eq!(
            UndirectedEndpoints::new(first, second),
            UndirectedEndpoints::new(second, first)
        );
    }

    #[test]
    fn boundary_index_rejects_pair_collisions_in_every_record_order() {
        let vertices = vec![
            Vertex {
                id: fixed_id(0x501),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: fixed_id(0x502),
                position: Point2::new(1.0, 0.0),
            },
        ];
        let first = Edge {
            id: fixed_id(0x601),
            start: vertices[0].id,
            end: vertices[1].id,
            kind: EdgeKind::Boundary,
        };
        let second = Edge {
            id: fixed_id(0x602),
            start: vertices[1].id,
            end: vertices[0].id,
            kind: EdgeKind::Boundary,
        };

        for edges in [vec![first.clone(), second.clone()], vec![second, first]] {
            let result = BoundaryIndex::build(&CreasePattern {
                vertices: vertices.clone(),
                edges,
            });
            assert!(matches!(
                result,
                Err(TopologyIssueKind::InternalBoundaryResolution)
            ));
        }
    }

    #[test]
    fn cycle_normalization_begins_with_the_unique_minimum_token() {
        let vertices = [fixed_id(0x701), fixed_id(0x702), fixed_id(0x703)];
        let mut half_edges = vec![
            HalfEdgeRef {
                edge: fixed_id(0x803),
                origin: vertices[0],
                destination: vertices[1],
            },
            HalfEdgeRef {
                edge: fixed_id(0x801),
                origin: vertices[1],
                destination: vertices[2],
            },
            HalfEdgeRef {
                edge: fixed_id(0x802),
                origin: vertices[2],
                destination: vertices[0],
            },
        ];
        let expected = half_edges
            .iter()
            .map(half_edge_token)
            .min()
            .expect("non-empty cycle");

        canonicalize_cycle(&mut half_edges);

        assert_eq!(half_edge_token(&half_edges[0]), expected);
    }

    #[test]
    fn concave_boundary_extracts_its_exact_area() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 4.0),
        ]);

        let snapshot = strict(namespace, &paper, &pattern);

        assert_eq!(snapshot.faces[0].area, 12.0);
    }

    #[test]
    fn canonical_snapshot_ignores_each_equivalent_storage_transform() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(2.5, -0.2),
            Point2::new(0.0, 3.8),
            Point2::new(-2.5, 0.3),
            Point2::new(0.3, -3.2),
        ]);
        let expected = strict(namespace, &paper, &pattern);

        let mut cyclic_paper = paper.clone();
        cyclic_paper.boundary_vertices.rotate_left(1);
        assert_eq!(strict(namespace, &cyclic_paper, &pattern), expected);

        let mut clockwise_paper = paper.clone();
        clockwise_paper.boundary_vertices.reverse();
        assert_eq!(strict(namespace, &clockwise_paper, &pattern), expected);

        let mut reversed_edges = pattern.clone();
        for edge in &mut reversed_edges.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        assert_eq!(strict(namespace, &paper, &reversed_edges), expected);

        let mut reordered_records = pattern.clone();
        reordered_records.vertices.reverse();
        reordered_records.edges.reverse();
        assert_eq!(strict(namespace, &paper, &reordered_records), expected);
    }

    #[test]
    fn face_identity_and_area_are_invariant_under_large_exact_translation() {
        let (namespace, paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(0.0, 1.0),
        ]);
        let expected = strict(namespace, &paper, &pattern);
        for vertex in &mut pattern.vertices {
            vertex.position.x += 1_000_000_000_000.0;
            vertex.position.y -= 1_000_000_000_000.0;
        }

        let translated = strict(namespace, &paper, &pattern);

        assert_eq!(translated, expected);
        assert_eq!(translated.faces[0].area, 1.0);
    }

    #[test]
    fn face_extraction_rejects_area_that_underflows_when_halved() {
        let side = f64::from_bits(486_u64 << 52);
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(side, 0.0),
            Point2::new(0.0, side),
        ]);

        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &pattern,
        });

        assert!(report.snapshot.is_none());
        assert_eq!(
            report.issues,
            vec![TopologyIssue {
                severity: TopologyIssueSeverity::Fatal,
                kind: TopologyIssueKind::UnrepresentableFaceArea,
            }]
        );
    }

    #[test]
    fn auxiliary_edges_do_not_change_face_identity() {
        let (namespace, paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);
        let baseline = strict(namespace, &paper, &pattern);
        let auxiliary_id = EdgeId::new();
        pattern.edges.insert(
            0,
            Edge {
                id: auxiliary_id,
                start: pattern.vertices[1].id,
                end: pattern.vertices[0].id,
                kind: EdgeKind::Auxiliary,
            },
        );

        let with_auxiliary = strict(namespace, &paper, &pattern);

        assert_eq!(with_auxiliary.faces, baseline.faces);
        assert_eq!(
            with_auxiliary
                .edge_incidence
                .iter()
                .find(|(edge, _)| *edge == auxiliary_id)
                .map(|(_, value)| value),
            Some(&EdgeIncidence::AuxiliaryIgnored)
        );
    }

    #[test]
    fn ten_thousand_edge_boundary_uses_indexed_resolution() {
        const EDGE_COUNT: usize = 10_000;
        let (namespace, paper, pattern) = subdivided_rectangle_fixture(EDGE_COUNT);

        let snapshot = strict(namespace, &paper, &pattern);

        assert_eq!(snapshot.faces[0].outer.half_edges.len(), EDGE_COUNT);
        assert_eq!(snapshot.edge_incidence.len(), EDGE_COUNT);
        assert_eq!(snapshot.faces[0].area, 4_999.0);
    }

    #[test]
    fn cut_edges_remain_explicitly_blocked() {
        let (namespace, mut paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);
        paper.cutting_allowed = true;
        let edge_id = EdgeId::new();
        pattern.edges.push(Edge {
            id: edge_id,
            start: pattern.vertices[0].id,
            end: pattern.vertices[2].id,
            kind: EdgeKind::Cut,
        });

        let error = extract_faces_strict(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &pattern,
        })
        .expect_err("cut topology is not implemented in this slice");

        assert_eq!(error.issue_count(), 1);
        assert_eq!(
            error.issues[0],
            TopologyIssue {
                severity: TopologyIssueSeverity::BlocksSimulation,
                kind: TopologyIssueKind::UnsupportedActiveEdge {
                    edge: edge_id,
                    edge_kind: EdgeKind::Cut,
                },
            }
        );
    }

    #[test]
    fn invalid_paper_and_duplicate_ids_fail_closed() {
        let (namespace, mut paper, mut pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);
        paper.boundary_vertices.pop();
        let invalid = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(invalid.snapshot.is_none());
        assert!(matches!(
            invalid.issues[0].kind,
            TopologyIssueKind::InvalidPaper { .. }
        ));

        let duplicate = pattern.vertices[0].clone();
        pattern.vertices.push(duplicate.clone());
        let duplicate_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &Paper {
                boundary_vertices: pattern.vertices[..4]
                    .iter()
                    .map(|vertex| vertex.id)
                    .collect(),
                ..Paper::default()
            },
            pattern: &pattern,
        });
        assert_eq!(
            duplicate_report.issues[0].kind,
            TopologyIssueKind::DuplicateVertexId {
                vertex: duplicate.id
            }
        );
    }

    #[test]
    fn duplicate_edges_missing_endpoints_and_non_finite_vertices_fail_closed() {
        let (namespace, paper, pattern) = polygon_fixture(&[
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(5.0, 5.0),
            Point2::new(0.0, 5.0),
        ]);

        let mut duplicate_edges = pattern.clone();
        duplicate_edges.edges.push(pattern.edges[0].clone());
        let duplicate_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &duplicate_edges,
        });
        assert_eq!(
            duplicate_report.issues[0].kind,
            TopologyIssueKind::DuplicateEdgeId {
                edge: pattern.edges[0].id
            }
        );

        let mut missing_endpoint = pattern.clone();
        missing_endpoint.edges[0].end = VertexId::new();
        let missing_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &missing_endpoint,
        });
        assert!(matches!(
            missing_report.issues[0].kind,
            TopologyIssueKind::InvalidPaper { .. }
        ));

        let mut non_finite = pattern.clone();
        non_finite.vertices[0].position.x = f64::NAN;
        let non_finite_report = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: 0,
            paper: &paper,
            pattern: &non_finite,
        });
        assert!(matches!(
            non_finite_report.issues[0].kind,
            TopologyIssueKind::InvalidPaper { .. }
        ));
    }
}
