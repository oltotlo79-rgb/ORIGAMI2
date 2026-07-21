use std::collections::{HashMap, HashSet, hash_map::Entry};

use crate::{
    DEFAULT_MAX_CONSTRAINT_EDGES, DEFAULT_MAX_CONSTRAINT_VERTICES, GeometricConstraintErrorV1,
    GeometricConstraintResourceV1, applied_pose::AppliedPoseV1,
    validate_geometric_constraint_record_against_pattern_v1,
};
use ori_domain::{
    AnnotationDocumentV1, AnnotationId, AnnotationRecordV1, BeginnerDesignProfileV1, ConstraintId,
    CreasePattern, DEFAULT_PROJECT_LAYER_ID, Edge, EdgeId, EdgeKind, EdgeLayerAssignmentV1,
    ElementMetadataDocumentV1, ElementMetadataV1, FaceId, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintDocumentValidationErrorV1,
    GeometricConstraintKindV1, GeometricConstraintRecordV1, InstructionPose, InstructionStep,
    InstructionStepId, InstructionTimeline, InstructionTimelineValidationError, InstructionVisual,
    LayerId, LayerRecordV1, LengthDisplayUnit, MAX_LAYER_EDGE_ASSIGNMENTS,
    MAX_PROJECT_LAYER_INDEX_EDGES, Paper, Point2, ProjectLayerDocumentV1,
    ProjectLayerDocumentValidationErrorV1, RgbaColor, UnderlayDocumentV1, UnderlayId,
    UnderlayRecordV1, Vertex, VertexId, validate_beginner_design_profile_v1,
    validate_element_metadata_v1, validate_geometric_constraint_document_v1,
    validate_instruction_timeline, validate_project_layer_document_against_pattern_v1,
    validate_underlay_document_v1,
};
use ori_geometry::{
    GeometryError, Orientation, PointSegmentRelation, SegmentIntersection, exact_orientation,
    point_segment_relation, segment_intersection, validate_crease_pattern, validate_paper,
};
use thiserror::Error;

mod history_persistence;

pub use history_persistence::{
    EDITOR_HISTORY_SCHEMA_VERSION_V1, EditorHistoryErrorV1, EditorHistoryV1,
};

pub type Revision = u64;

/// Largest revision that can round-trip exactly through a JavaScript number.
pub const MAX_REVISION: Revision = (1_u64 << 53) - 1;

/// Chooses whether a cluster creates its common vertex or reuses one already
/// stored in the crease pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunctionVertexIntent {
    Create { id: VertexId },
    Reuse { id: VertexId },
}

impl JunctionVertexIntent {
    const fn id(self) -> VertexId {
        match self {
            Self::Create { id } | Self::Reuse { id } => id,
        }
    }
}

/// Associates one source edge with the ID for its optional second half.
///
/// `new_edge` is required for a strict-interior split and must be absent when
/// the source edge already has the cluster junction as an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntersectionEdgeTarget {
    pub edge: EdgeId,
    pub new_edge: Option<EdgeId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexPositionUpdate {
    pub vertex: VertexId,
    pub position: Point2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorSelectionModeV1 {
    Move,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MirrorAxisV1 {
    pub start: Point2,
    pub end: Point2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ElementMetadataTargetV1 {
    Vertex(VertexId),
    Edge(EdgeId),
    Face(FaceId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    UpdateProjectMemo {
        memo: String,
    },
    UpdateBeginnerDesignProfile {
        profile: Box<BeginnerDesignProfileV1>,
    },
    SetElementMetadata {
        target: ElementMetadataTargetV1,
        metadata: Option<ElementMetadataV1>,
    },
    AddVertex {
        id: VertexId,
        position: Point2,
    },
    MoveVertex {
        id: VertexId,
        position: Point2,
    },
    MoveEdge {
        id: EdgeId,
        start_position: Point2,
        end_position: Point2,
    },
    MoveVertices {
        updates: Vec<VertexPositionUpdate>,
    },
    RemoveVertex {
        id: VertexId,
    },
    AddEdge {
        id: EdgeId,
        start: VertexId,
        end: VertexId,
        kind: EdgeKind,
    },
    AddConnectedVertex {
        vertex_id: VertexId,
        position: Point2,
        edge_id: EdgeId,
        start: VertexId,
        kind: EdgeKind,
    },
    RemoveConnectedVertex {
        vertex_id: VertexId,
        edge_id: EdgeId,
    },
    RemoveEdge {
        id: EdgeId,
    },
    SetCuttingAllowed {
        allowed: bool,
    },
    UpdatePaperProperties {
        thickness_mm: f64,
        front_color: RgbaColor,
        back_color: RgbaColor,
        front_texture_asset: Option<ori_domain::AssetId>,
        back_texture_asset: Option<ori_domain::AssetId>,
        cutting_allowed: bool,
    },
    SetLengthDisplayUnit {
        unit: LengthDisplayUnit,
    },
    ResizeRectangularPaper {
        width_mm: f64,
        height_mm: f64,
    },
    SplitEdge {
        edge: EdgeId,
        new_vertex: VertexId,
        new_edge: EdgeId,
        fraction: f64,
    },
    ConnectEdgeIntersection {
        first_edge: EdgeId,
        second_edge: EdgeId,
        new_vertex: VertexId,
        first_new_edge: EdgeId,
        second_new_edge: EdgeId,
    },
    ConnectTJunction {
        first_edge: EdgeId,
        second_edge: EdgeId,
        new_edge: EdgeId,
    },
    ConnectIntersectionCluster {
        junction: JunctionVertexIntent,
        targets: Vec<IntersectionEdgeTarget>,
    },
    SplitBoundaryEdge {
        edge: EdgeId,
        new_vertex: VertexId,
        new_edge: EdgeId,
        fraction: f64,
    },
    RemoveBoundaryVertex {
        vertex: VertexId,
    },
    /// Adds one strictly validated record. Persisted-domain validation runs
    /// before reference and local-geometry validation.
    AddGeometricConstraint {
        record: GeometricConstraintRecordV1,
    },
    /// Removes the first matching raw record without validating unrelated
    /// records, allowing an unchecked loaded document to be repaired one
    /// record at a time. Undo restores the exact record and vector index.
    RemoveGeometricConstraint {
        id: ConstraintId,
    },
    AddAnnotation {
        record: AnnotationRecordV1,
    },
    UpdateAnnotation {
        record: AnnotationRecordV1,
    },
    RemoveAnnotation {
        id: AnnotationId,
    },
    AddUnderlay {
        record: UnderlayRecordV1,
    },
    UpdateUnderlay {
        record: UnderlayRecordV1,
    },
    RemoveUnderlay {
        id: UnderlayId,
    },
    AddInstructionStep {
        step: InstructionStep,
    },
    /// Appends a bounded group of instruction steps as one revision and one
    /// Undo/Redo history entry. Validation is performed against the complete
    /// candidate timeline before any step becomes visible.
    AppendInstructionSteps {
        steps: Vec<InstructionStep>,
    },
    UpdateInstructionStepMetadata {
        step_id: InstructionStepId,
        title: String,
        description: String,
        caution: String,
        duration_ms: u32,
        visual: InstructionVisual,
    },
    ReplaceInstructionStepPose {
        step_id: InstructionStepId,
        pose: InstructionPose,
    },
    RemoveInstructionStep {
        step_id: InstructionStepId,
    },
    MoveInstructionStep {
        step_id: InstructionStepId,
        target_index: usize,
    },
    /// Atomically replaces a timeline only when the change is exactly one
    /// adjacent split or merge. The inverse uses the same symmetric guard.
    RewriteInstructionTimelineSplitMerge {
        timeline: InstructionTimeline,
    },
    CreateLayer {
        layer: LayerRecordV1,
        target_index: usize,
    },
    RenameLayer {
        layer: LayerId,
        name: String,
    },
    UpdateLayerPresentation {
        layer: LayerId,
        visible: bool,
        locked: bool,
        opacity: f64,
    },
    MoveLayer {
        layer: LayerId,
        target_index: usize,
    },
    DeleteLayer {
        layer: LayerId,
    },
    AssignEdgeToLayer {
        edge: EdgeId,
        layer: LayerId,
    },
    MirrorSelection {
        vertices: Vec<VertexId>,
        edges: Vec<EdgeId>,
        axis: MirrorAxisV1,
        mode: MirrorSelectionModeV1,
        new_vertices: Vec<VertexId>,
        new_edges: Vec<EdgeId>,
    },
    ApplyNormalizedEdgeDocument {
        pattern: CreasePattern,
        project_layers: ProjectLayerDocumentV1,
    },
    ApplyStackedFoldDocument {
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        beginner_design_profile: Box<BeginnerDesignProfileV1>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    pub revision: Revision,
    pub changed_vertices: Vec<VertexId>,
    pub changed_edges: Vec<EdgeId>,
    pub settings_changed: bool,
    pub instructions_changed: bool,
    /// Whether the authored geometric-constraint document changed.
    pub constraints_changed: bool,
}

#[derive(Debug, Error, PartialEq)]
pub enum CommandError {
    #[error("annotation is invalid or unavailable")]
    InvalidAnnotation,
    #[error("annotation {0:?} was not found")]
    AnnotationNotFound(AnnotationId),
    #[error("annotation {0:?} already exists")]
    AnnotationAlreadyExists(AnnotationId),
    #[error("underlay is invalid or unavailable")]
    InvalidUnderlay,
    #[error("underlay {0:?} was not found")]
    UnderlayNotFound(UnderlayId),
    #[error("underlay {0:?} already exists")]
    UnderlayAlreadyExists(UnderlayId),
    #[error("the stacked-fold target document is invalid")]
    InvalidStackedFoldDocument,
    #[error("the mirror-selection request or resulting geometry is invalid")]
    InvalidMirrorSelection,
    #[error("the beginner-design evaluation profile is invalid")]
    InvalidBeginnerDesignProfile,
    #[error("expected revision {expected}, but the current revision is {actual}")]
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    #[error("revision {revision} cannot advance beyond the maximum supported revision")]
    RevisionExhausted { revision: Revision },
    #[error("vertex {0:?} already exists")]
    VertexAlreadyExists(VertexId),
    #[error("vertex {0:?} was not found")]
    VertexNotFound(VertexId),
    #[error("edge {0:?} already exists")]
    EdgeAlreadyExists(EdgeId),
    #[error("edge {0:?} was not found")]
    EdgeNotFound(EdgeId),
    #[error("element metadata is invalid")]
    InvalidElementMetadata,
    #[error("the vertex move batch must contain a bounded set of unique vertices")]
    InvalidVertexMoveBatch,
    #[error("the requested position for vertex {vertex:?} must be finite")]
    VertexMovePositionNotFinite { vertex: VertexId },
    #[error("an edge cannot connect vertex {0:?} to itself")]
    DegenerateEdge(VertexId),
    #[error("vertex {vertex:?} is still used by edge {edge:?}")]
    VertexHasConnectedEdge { vertex: VertexId, edge: EdgeId },
    #[error("cut edges are disabled for this project")]
    CuttingDisabled,
    #[error("paper thickness must be finite")]
    PaperThicknessNotFinite,
    #[error("paper thickness must be zero or greater")]
    PaperThicknessNegative,
    #[error("cutting cannot be disabled while cut edge {edge:?} exists")]
    CutEdgesPreventDisabling { edge: EdgeId },
    #[error("edge {edge:?} is not a valid paper-length display reference")]
    LengthDisplayReferenceEdgeInvalid { edge: EdgeId },
    #[error("boundary edge {edge:?} is the active paper-length display reference")]
    LengthDisplayReferenceEdgeMutationBlocked { edge: EdgeId },
    #[error("moving this vertex would invalidate paper-length reference edge {edge:?}")]
    LengthDisplayReferenceEdgeWouldBecomeInvalid { edge: EdgeId },
    #[error("paper width must be finite")]
    PaperWidthNotFinite,
    #[error("paper width must be greater than zero")]
    PaperWidthNotPositive,
    #[error("paper height must be finite")]
    PaperHeightNotFinite,
    #[error("paper height must be greater than zero")]
    PaperHeightNotPositive,
    #[error("target paper area is too large to represent safely")]
    PaperResizeAreaNotRepresentable,
    #[error("paper boundary must contain exactly four vertices, found {actual}")]
    RectangularPaperBoundaryVertexCount { actual: usize },
    #[error("paper boundary references vertex {vertex:?} more than once")]
    RectangularPaperBoundaryDuplicateVertex { vertex: VertexId },
    #[error("paper boundary vertex {0:?} was not found")]
    RectangularPaperBoundaryVertexNotFound(VertexId),
    #[error("paper boundary vertex {vertex:?} has a non-finite position")]
    RectangularPaperBoundaryPositionNotFinite { vertex: VertexId },
    #[error("paper boundary does not have a finite positive rectangular area")]
    RectangularPaperBoundaryAreaNotRepresentable,
    #[error("paper boundary is not a rectangle")]
    PaperBoundaryNotRectangle,
    #[error("paper boundary is a rectangle but is not axis-aligned")]
    PaperBoundaryNotAxisAligned,
    #[error("paper boundary vertices must list adjacent corners in boundary order")]
    PaperBoundaryVerticesNotAdjacent,
    #[error("paper resize scale cannot be represented as a positive finite number")]
    PaperResizeScaleNotRepresentable,
    #[error("resizing the paper would produce a non-finite position for vertex {vertex:?}")]
    PaperResizeVertexPositionNotFinite { vertex: VertexId },
    #[error("requested paper dimensions cannot be represented at the current boundary origin")]
    PaperResizeBoundaryNotRepresentable,
    #[error("edge {0:?} is not a boundary edge")]
    EdgeIsNotBoundary(EdgeId),
    #[error("target boundary edge ID {edge:?} occurs more than once")]
    BoundarySplitTargetEdgeIdAmbiguous { edge: EdgeId },
    #[error("boundary edge {0:?} does not match a consecutive paper-boundary pair")]
    BoundaryEdgeNotInPaperBoundary(EdgeId),
    #[error("boundary edge {edge:?} matches more than one paper-boundary pair")]
    BoundaryEdgeMatchesMultiplePaperSegments { edge: EdgeId },
    #[error("boundary split fraction must be finite")]
    BoundarySplitFractionNotFinite,
    #[error("boundary split fraction must be strictly between zero and one")]
    BoundarySplitFractionOutOfRange,
    #[error("boundary edge {edge:?} endpoint {vertex:?} has a non-finite position")]
    BoundarySplitEndpointPositionNotFinite { edge: EdgeId, vertex: VertexId },
    #[error("boundary split position is not finite")]
    BoundarySplitPositionNotFinite,
    #[error("boundary split position must be distinct from both edge endpoints")]
    BoundarySplitPositionNotDistinct,
    #[error("boundary split position is already occupied by vertex {vertex:?}")]
    BoundarySplitPositionOccupied { vertex: VertexId },
    #[error("removing a boundary vertex requires at least four boundary entries, found {actual}")]
    BoundaryVertexRemovalNeedsFourVertices { actual: usize },
    #[error("vertex {0:?} is not in the paper boundary")]
    VertexNotInPaperBoundary(VertexId),
    #[error("vertex {vertex:?} occurs more than once in the paper boundary")]
    BoundaryVertexOccursMultipleTimes { vertex: VertexId },
    #[error("vertex ID {vertex:?} has more than one pattern vertex record")]
    BoundaryVertexRecordAmbiguous { vertex: VertexId },
    #[error("boundary vertex {vertex:?} has the same previous and next vertex {neighbor:?}")]
    BoundaryVertexNeighborsNotDistinct {
        vertex: VertexId,
        neighbor: VertexId,
    },
    #[error("vertex {vertex:?} has no unique preceding boundary edge")]
    BoundaryVertexPrecedingEdgeMissing { vertex: VertexId },
    #[error("vertex {vertex:?} has multiple preceding boundary edges")]
    BoundaryVertexPrecedingEdgeAmbiguous { vertex: VertexId },
    #[error("vertex {vertex:?} has no unique following boundary edge")]
    BoundaryVertexFollowingEdgeMissing { vertex: VertexId },
    #[error("vertex {vertex:?} has multiple following boundary edges")]
    BoundaryVertexFollowingEdgeAmbiguous { vertex: VertexId },
    #[error("vertex {vertex:?} must have two distinct adjacent boundary edge records")]
    BoundaryVertexAdjacentEdgesNotDistinct { vertex: VertexId },
    #[error("adjacent boundary edge ID {edge:?} for vertex {vertex:?} occurs more than once")]
    BoundaryVertexAdjacentEdgeIdAmbiguous { vertex: VertexId, edge: EdgeId },
    #[error("vertex {vertex:?} is connected to additional edge {edge:?}")]
    BoundaryVertexHasAdditionalEdge { vertex: VertexId, edge: EdgeId },
    #[error("edge {edge:?} already connects the neighbors of boundary vertex {vertex:?}")]
    BoundaryVertexNeighborEdgeAlreadyExists { vertex: VertexId, edge: EdgeId },
    #[error("removing the boundary vertex would invalidate a currently valid paper")]
    BoundaryVertexRemovalWouldInvalidatePaper,
    #[error("boundary edge {0:?} must be changed through a sheet-boundary operation")]
    BoundaryEdgeRequiresSheetOperation(EdgeId),
    #[error("target edge ID {edge:?} occurs more than once")]
    EdgeSplitTargetEdgeIdAmbiguous { edge: EdgeId },
    #[error("edge split fraction must be finite")]
    EdgeSplitFractionNotFinite,
    #[error("edge split fraction must be strictly between zero and one")]
    EdgeSplitFractionOutOfRange,
    #[error("edge {edge:?} endpoint {vertex:?} has more than one vertex record")]
    EdgeSplitEndpointVertexRecordAmbiguous { edge: EdgeId, vertex: VertexId },
    #[error("edge {edge:?} endpoint {vertex:?} has a non-finite position")]
    EdgeSplitEndpointPositionNotFinite { edge: EdgeId, vertex: VertexId },
    #[error("edge split position is not finite")]
    EdgeSplitPositionNotFinite,
    #[error("edge split position must be distinct from both edge endpoints")]
    EdgeSplitPositionNotDistinct,
    #[error("edge split position is already occupied by vertex {vertex:?}")]
    EdgeSplitPositionOccupied { vertex: VertexId },
    #[error("the two intersection edge IDs must be different")]
    EdgeIntersectionTargetsNotDistinct,
    #[error("intersection target edge ID {edge:?} occurs more than once")]
    EdgeIntersectionTargetEdgeIdAmbiguous { edge: EdgeId },
    #[error("intersection edge {0:?} must not be a boundary edge")]
    EdgeIntersectionBoundaryEdge(EdgeId),
    #[error("the two generated intersection edge IDs must be different")]
    EdgeIntersectionNewEdgeIdsNotDistinct,
    #[error("intersection edge {edge:?} endpoint {vertex:?} has more than one vertex record")]
    EdgeIntersectionEndpointVertexRecordAmbiguous { edge: EdgeId, vertex: VertexId },
    #[error("intersection edge {edge:?} endpoint {vertex:?} has a non-finite position")]
    EdgeIntersectionEndpointPositionNotFinite { edge: EdgeId, vertex: VertexId },
    #[error("edge intersection geometry cannot be represented with finite coordinates")]
    EdgeIntersectionGeometryNotRepresentable,
    #[error("the selected edges do not have a single point intersection")]
    EdgeIntersectionNotSinglePoint,
    #[error("the selected edges must intersect strictly inside both edges")]
    EdgeIntersectionNotProper,
    #[error("edge intersection position is already occupied by vertex {vertex:?}")]
    EdgeIntersectionPositionOccupied { vertex: VertexId },
    #[error("the two T-junction edge IDs must be different")]
    TJunctionTargetsNotDistinct,
    #[error("T-junction target edge ID {edge:?} occurs more than once")]
    TJunctionTargetEdgeIdAmbiguous { edge: EdgeId },
    #[error("a T-junction cannot connect two boundary edges")]
    TJunctionBothEdgesBoundary,
    #[error("boundary T-junction edge {edge:?} must contain the junction strictly inside it")]
    TJunctionBoundaryEdgeMustBeInterior { edge: EdgeId },
    #[error("T-junction vertex {vertex:?} is already present in the paper boundary")]
    TJunctionBoundaryVertexAlreadyPresent { vertex: VertexId },
    #[error("T-junction vertex {vertex:?} is already connected to other boundary edge {edge:?}")]
    TJunctionVertexHasOtherBoundaryEdge { vertex: VertexId, edge: EdgeId },
    #[error("T-junction edge {edge:?} endpoint {vertex:?} has more than one vertex record")]
    TJunctionEndpointVertexRecordAmbiguous { edge: EdgeId, vertex: VertexId },
    #[error("T-junction edge {edge:?} endpoint {vertex:?} has a non-finite position")]
    TJunctionEndpointPositionNotFinite { edge: EdgeId, vertex: VertexId },
    #[error("T-junction geometry cannot be represented with finite coordinates")]
    TJunctionGeometryNotRepresentable,
    #[error("the selected edges do not form exactly one strict T-junction")]
    NotTJunction,
    #[error("T-junction position is also occupied by distinct vertex {vertex:?}")]
    TJunctionPositionOccupied { vertex: VertexId },
    #[error("an intersection cluster requires at least three target edges, found {actual}")]
    IntersectionClusterNeedsThreeTargets { actual: usize },
    #[error("an intersection cluster supports at most {maximum} target edges, found {actual}")]
    IntersectionClusterTooManyTargets { actual: usize, maximum: usize },
    #[error("intersection cluster target edge {edge:?} was supplied more than once")]
    IntersectionClusterTargetDuplicate { edge: EdgeId },
    #[error("intersection cluster target edge ID {edge:?} occurs more than once")]
    IntersectionClusterTargetEdgeIdAmbiguous { edge: EdgeId },
    #[error("intersection cluster does not yet support boundary edge {0:?}")]
    IntersectionClusterBoundaryEdge(EdgeId),
    #[error(
        "intersection cluster edge {edge:?} endpoint {vertex:?} has more than one vertex record"
    )]
    IntersectionClusterEndpointVertexRecordAmbiguous { edge: EdgeId, vertex: VertexId },
    #[error("intersection cluster edge {edge:?} endpoint {vertex:?} has a non-finite position")]
    IntersectionClusterEndpointPositionNotFinite { edge: EdgeId, vertex: VertexId },
    #[error("intersection cluster edge {edge:?} has zero geometric length")]
    IntersectionClusterZeroLengthEdge { edge: EdgeId },
    #[error("intersection cluster generated edge ID {new_edge:?} was supplied more than once")]
    IntersectionClusterGeneratedEdgeIdDuplicate { new_edge: EdgeId },
    #[error("intersection cluster edge {edge:?} requires a generated edge ID")]
    IntersectionClusterNewEdgeRequired { edge: EdgeId },
    #[error("intersection cluster edge {edge:?} already has the junction as an endpoint")]
    IntersectionClusterNewEdgeUnexpected { edge: EdgeId },
    #[error("intersection cluster geometry cannot be represented with finite coordinates")]
    IntersectionClusterGeometryNotRepresentable,
    #[error("the first two intersection cluster edges do not meet at one point")]
    IntersectionClusterNoSingleIntersection,
    #[error("intersection cluster edges {first_edge:?} and {second_edge:?} overlap collinearly")]
    IntersectionClusterCollinearOverlap {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    #[error("intersection cluster edge {edge:?} does not contain the common junction")]
    IntersectionClusterDifferentIntersection { edge: EdgeId },
    #[error("intersection cluster junction vertex ID {vertex:?} has more than one record")]
    IntersectionClusterJunctionVertexRecordAmbiguous { vertex: VertexId },
    #[error("intersection cluster junction vertex {vertex:?} has a non-finite position")]
    IntersectionClusterJunctionPositionNotFinite { vertex: VertexId },
    #[error("intersection cluster junction position is occupied by vertex {vertex:?}")]
    IntersectionClusterJunctionPositionOccupied { vertex: VertexId },
    #[error("intersection cluster junction position has more than one vertex record")]
    IntersectionClusterJunctionPositionAmbiguous,
    #[error(
        "intersection cluster endpoint on edge {edge:?} is vertex {actual:?}, not {expected:?}"
    )]
    IntersectionClusterEndpointVertexMismatch {
        edge: EdgeId,
        expected: VertexId,
        actual: VertexId,
    },
    #[error("an intersection cluster must split at least one edge")]
    IntersectionClusterNeedsSplit,
    #[error("edge {edge:?} also passes through the intersection cluster but was omitted")]
    IncompleteIntersectionCluster { edge: EdgeId },
    #[error("instruction step {0:?} already exists")]
    InstructionStepAlreadyExists(InstructionStepId),
    #[error("instruction step {0:?} was not found")]
    InstructionStepNotFound(InstructionStepId),
    #[error("a declarative-only instruction step cannot be converted into an executable pose")]
    DeclarativeInstructionPoseImmutable,
    #[error("an instruction-step append batch must not be empty")]
    InstructionStepAppendBatchEmpty,
    #[error("the appended instruction steps are no longer the exact timeline suffix")]
    InstructionStepAppendHistoryMismatch,
    #[error("instruction step target index {target_index} is out of bounds for {step_count} steps")]
    InstructionStepTargetIndexOutOfBounds {
        target_index: usize,
        step_count: usize,
    },
    #[error("invalid instruction timeline: {0}")]
    InstructionTimelineInvalid(#[from] InstructionTimelineValidationError),
    #[error("invalid geometric-constraint document: {0}")]
    GeometricConstraintDocumentInvalid(#[from] GeometricConstraintDocumentValidationErrorV1),
    #[error("geometric constraint is invalid for the current geometry: {0}")]
    GeometricConstraintGeometryInvalid(GeometricConstraintErrorV1),
    #[error(
        "geometric-constraint {resource:?} count overflowed while projecting the command result"
    )]
    GeometricConstraintGeometryCountOverflow {
        resource: GeometricConstraintResourceV1,
    },
    #[error(
        "geometric-constraint {resource:?} index would contain {actual} records; the hard maximum is {maximum}"
    )]
    GeometricConstraintGeometryLimitExceeded {
        resource: GeometricConstraintResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("geometric constraint {0:?} already exists")]
    GeometricConstraintAlreadyExists(ConstraintId),
    #[error("geometric constraint {0:?} was not found")]
    GeometricConstraintNotFound(ConstraintId),
    #[error("invalid project-layer document: {0}")]
    ProjectLayerDocumentInvalid(#[from] ProjectLayerDocumentValidationErrorV1),
    #[error("project layer {0:?} already exists")]
    LayerAlreadyExists(LayerId),
    #[error("project layer {0:?} was not found")]
    LayerNotFound(LayerId),
    #[error("project layer {0:?} is locked")]
    LayerLocked(LayerId),
    #[error("the reserved default project layer cannot be deleted")]
    DefaultLayerDeletionForbidden,
    #[error("layer target index {target_index} is out of bounds for {layer_count} layers")]
    LayerTargetIndexOutOfBounds {
        target_index: usize,
        layer_count: usize,
    },
    #[error("edge {edge:?} has more than one record and cannot be assigned unambiguously")]
    LayerAssignmentEdgeIdAmbiguous { edge: EdgeId },
    #[error(
        "edge-layer assignment count would exceed the hard maximum {maximum}; current {current}, added {added}"
    )]
    TooManyLayerEdgeAssignments {
        current: usize,
        added: usize,
        maximum: usize,
    },
    #[error("edge {edge:?} does not have the exact layer assignment required by history")]
    LayerHistoryAssignmentMismatch { edge: EdgeId },
    #[error(
        "geometric constraint {constraint:?} must be removed before changing referenced geometry"
    )]
    GeometricConstraintBlocksGeometryMutation { constraint: ConstraintId },
}

fn source_edges_preserved_by_exact_subdivision(
    source: &CreasePattern,
    target: &CreasePattern,
) -> bool {
    let source_positions = source
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let target_positions = target
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    source.edges.iter().all(|source_edge| {
        if target
            .edges
            .iter()
            .any(|target_edge| target_edge == source_edge)
        {
            return true;
        }
        let (Some(start), Some(end)) = (
            source_positions.get(&source_edge.start).copied(),
            source_positions.get(&source_edge.end).copied(),
        ) else {
            return false;
        };
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let parameter = |point: Point2| {
            if dx.abs() >= dy.abs() {
                (point.x - start.x) / dx
            } else {
                (point.y - start.y) / dy
            }
        };
        let mut intervals = target
            .edges
            .iter()
            .filter(|edge| edge.kind == source_edge.kind)
            .filter_map(|edge| {
                let (first, second) = (
                    target_positions.get(&edge.start).copied()?,
                    target_positions.get(&edge.end).copied()?,
                );
                if point_segment_relation(first, start, end).ok()? == PointSegmentRelation::Outside
                    || point_segment_relation(second, start, end).ok()?
                        == PointSegmentRelation::Outside
                {
                    return None;
                }
                let mut interval = [parameter(first), parameter(second)];
                interval.sort_by(f64::total_cmp);
                Some(interval)
            })
            .collect::<Vec<_>>();
        intervals.sort_by(|left, right| {
            left[0]
                .total_cmp(&right[0])
                .then_with(|| left[1].total_cmp(&right[1]))
        });
        intervals.first().is_some_and(|interval| interval[0] == 0.0)
            && intervals.last().is_some_and(|interval| interval[1] == 1.0)
            && intervals.windows(2).all(|pair| pair[0][1] == pair[1][0])
    })
}

/// Reports an unsupported per-editor undo/redo history entry limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HistoryEntryLimitError {
    #[error(
        "editor history entry limit must be between {minimum} and {maximum} inclusive; got {requested}"
    )]
    OutOfRange {
        requested: usize,
        minimum: usize,
        maximum: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct HistoryEntry {
    forward: Command,
    inverse: Inverse,
    applied_pose: AppliedPoseHistoryTransition,
}

/// Runtime-pose behavior attached to one document-history edge.
///
/// `Restore` already carries both sides so a future atomic stacked-fold
/// command can store a non-planar `after` pose. Ordinary geometry commands
/// currently start with `after: None`.
#[derive(Debug, Clone, PartialEq)]
enum AppliedPoseHistoryTransition {
    PreserveCurrent,
    Restore {
        before: Option<AppliedPoseV1>,
        after: Option<AppliedPoseV1>,
    },
}

impl AppliedPoseHistoryTransition {
    fn capture_after(&mut self, current: &Option<AppliedPoseV1>) {
        if let Self::Restore { after, .. } = self {
            after.clone_from(current);
        }
    }

    fn capture_before(&mut self, current: &Option<AppliedPoseV1>) {
        if let Self::Restore { before, .. } = self {
            before.clone_from(current);
        }
    }

    fn restore_before(&self, current: &mut Option<AppliedPoseV1>) {
        if let Self::Restore { before, .. } = self {
            current.clone_from(before);
        }
    }

    fn restore_after(&self, current: &mut Option<AppliedPoseV1>) {
        if let Self::Restore { after, .. } = self {
            current.clone_from(after);
        }
    }
}

pub const MAX_EDITOR_HISTORY_ENTRIES: usize = 128;

fn trim_history_to_limit(stack: &mut Vec<HistoryEntry>, limit: usize) {
    let discard_count = stack.len().saturating_sub(limit);
    if discard_count > 0 {
        stack.drain(..discard_count);
    }
}

fn push_bounded_history(stack: &mut Vec<HistoryEntry>, entry: HistoryEntry, limit: usize) {
    debug_assert!((1..=MAX_EDITOR_HISTORY_ENTRIES).contains(&limit));
    let discard_count = stack.len().saturating_add(1).saturating_sub(limit);
    if discard_count > 0 {
        stack.drain(..discard_count);
    }
    stack.push(entry);
}

#[allow(clippy::too_many_arguments)]
fn mirror_selection_target(
    pattern: &CreasePattern,
    paper: &Paper,
    layers: &ProjectLayerDocumentV1,
    vertices: &[VertexId],
    edges: &[EdgeId],
    axis: MirrorAxisV1,
    mode: MirrorSelectionModeV1,
    new_vertices: &[VertexId],
    new_edges: &[EdgeId],
) -> Result<(CreasePattern, ProjectLayerDocumentV1), CommandError> {
    let canonical = |ids: &[VertexId]| {
        ids.windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    };
    let canonical_edges = edges
        .windows(2)
        .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes());
    let canonical_new_edges = new_edges
        .windows(2)
        .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes());
    let dx = axis.end.x - axis.start.x;
    let dy = axis.end.y - axis.start.y;
    let length_squared = dx.mul_add(dx, dy * dy);
    if vertices.is_empty() && edges.is_empty()
        || !canonical(vertices)
        || !canonical_edges
        || !axis.start.x.is_finite()
        || !axis.start.y.is_finite()
        || !axis.end.x.is_finite()
        || !axis.end.y.is_finite()
        || !length_squared.is_finite()
        || length_squared <= 0.0
        || match mode {
            MirrorSelectionModeV1::Move => !new_vertices.is_empty() || !new_edges.is_empty(),
            MirrorSelectionModeV1::Duplicate => {
                new_vertices.len() != vertices.len()
                    || new_edges.len() != edges.len()
                    || !canonical(new_vertices)
                    || !canonical_new_edges
            }
        }
    {
        return Err(CommandError::InvalidMirrorSelection);
    }
    let selected_vertices = vertices.iter().copied().collect::<HashSet<_>>();
    let selected_edges = edges.iter().copied().collect::<HashSet<_>>();
    for id in vertices {
        if paper.boundary_vertices.contains(id) {
            return Err(CommandError::InvalidMirrorSelection);
        }
        if !pattern.vertices.iter().any(|vertex| vertex.id == *id) {
            return Err(CommandError::VertexNotFound(*id));
        }
    }
    for id in edges {
        if !pattern.edges.iter().any(|edge| edge.id == *id) {
            return Err(CommandError::EdgeNotFound(*id));
        }
    }
    for edge in pattern
        .edges
        .iter()
        .filter(|edge| selected_edges.contains(&edge.id))
    {
        if edge.kind == EdgeKind::Boundary
            || !selected_vertices.contains(&edge.start)
            || !selected_vertices.contains(&edge.end)
        {
            return Err(CommandError::InvalidMirrorSelection);
        }
        let layer = layers.layer_for_edge(edge.id);
        if layers
            .layers
            .iter()
            .any(|item| item.id == layer && item.locked)
        {
            return Err(CommandError::LayerLocked(layer));
        }
    }
    if mode == MirrorSelectionModeV1::Move {
        for edge in pattern.edges.iter().filter(|edge| {
            selected_vertices.contains(&edge.start) || selected_vertices.contains(&edge.end)
        }) {
            let layer = layers.layer_for_edge(edge.id);
            if layers
                .layers
                .iter()
                .any(|item| item.id == layer && item.locked)
            {
                return Err(CommandError::LayerLocked(layer));
            }
        }
    }
    let reflected = vertices
        .iter()
        .map(|id| {
            let source = pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == *id)
                .ok_or(CommandError::VertexNotFound(*id))?;
            let rx = source.position.x - axis.start.x;
            let ry = source.position.y - axis.start.y;
            let projection = rx.mul_add(dx, ry * dy) / length_squared;
            let px = axis.start.x + projection * dx;
            let py = axis.start.y + projection * dy;
            let result = Point2::new(
                px.mul_add(2.0, -source.position.x),
                py.mul_add(2.0, -source.position.y),
            );
            (result.x.is_finite() && result.y.is_finite())
                .then_some(result)
                .ok_or(CommandError::InvalidMirrorSelection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut target = pattern.clone();
    let mut target_layers = layers.clone();
    for (source, position) in vertices.iter().zip(&reflected) {
        if target.vertices.iter().any(|vertex| {
            (mode == MirrorSelectionModeV1::Duplicate || vertex.id != *source)
                && vertex.position == *position
        }) {
            return Err(CommandError::InvalidMirrorSelection);
        }
    }
    match mode {
        MirrorSelectionModeV1::Move => {
            for (id, position) in vertices.iter().zip(reflected) {
                target
                    .vertices
                    .iter_mut()
                    .find(|vertex| vertex.id == *id)
                    .ok_or(CommandError::VertexNotFound(*id))?
                    .position = position;
            }
        }
        MirrorSelectionModeV1::Duplicate => {
            let map = vertices
                .iter()
                .copied()
                .zip(new_vertices.iter().copied())
                .collect::<HashMap<_, _>>();
            for (id, position) in new_vertices.iter().zip(reflected) {
                if target.vertices.iter().any(|vertex| vertex.id == *id) {
                    return Err(CommandError::VertexAlreadyExists(*id));
                }
                target.vertices.push(Vertex { id: *id, position });
            }
            for (source_id, new_id) in edges.iter().zip(new_edges) {
                if target.edges.iter().any(|edge| edge.id == *new_id) {
                    return Err(CommandError::EdgeAlreadyExists(*new_id));
                }
                let source = pattern
                    .edges
                    .iter()
                    .find(|edge| edge.id == *source_id)
                    .ok_or(CommandError::EdgeNotFound(*source_id))?;
                target.edges.push(Edge {
                    id: *new_id,
                    start: *map
                        .get(&source.start)
                        .ok_or(CommandError::InvalidMirrorSelection)?,
                    end: *map
                        .get(&source.end)
                        .ok_or(CommandError::InvalidMirrorSelection)?,
                    kind: source.kind,
                });
                let layer = layers.layer_for_edge(source.id);
                if layer != DEFAULT_PROJECT_LAYER_ID {
                    target_layers.edge_assignments.push(EdgeLayerAssignmentV1 {
                        edge: *new_id,
                        layer,
                    });
                }
            }
        }
    }
    if !validate_crease_pattern(&target).is_valid()
        || !validate_paper(paper, &target).is_valid()
        || validate_project_layer_document_against_pattern_v1(&target_layers, &target).is_err()
        || target == *pattern
    {
        return Err(CommandError::InvalidMirrorSelection);
    }
    Ok((target, target_layers))
}

#[derive(Debug, Clone, PartialEq)]
enum Inverse {
    RestoreMirrorSelection {
        pattern: CreasePattern,
        project_layers: ProjectLayerDocumentV1,
    },
    RestoreStackedFoldDocument {
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        beginner_design_profile: Box<BeginnerDesignProfileV1>,
    },
    RestoreProjectMemo {
        memo: String,
    },
    RestoreBeginnerDesignProfile {
        profile: Box<BeginnerDesignProfileV1>,
    },
    RestoreElementMetadata {
        target: ElementMetadataTargetV1,
        metadata: Option<ElementMetadataV1>,
    },
    Command(Command),
    RestoreVertex {
        index: usize,
        vertex: Vertex,
    },
    RestoreEdge {
        index: usize,
        edge: Edge,
        layer_assignment: Option<(usize, EdgeLayerAssignmentV1)>,
    },
    RestorePaperProperties {
        thickness_mm: f64,
        front_color: RgbaColor,
        back_color: RgbaColor,
        front_texture_asset: Option<ori_domain::AssetId>,
        back_texture_asset: Option<ori_domain::AssetId>,
        cutting_allowed: bool,
    },
    RestoreLengthDisplayUnit {
        unit: LengthDisplayUnit,
    },
    RestoreVertexPositions {
        vertices: Vec<(VertexId, Point2)>,
    },
    RestoreBoundarySplit {
        boundary_vertices: Vec<VertexId>,
        original_edge_index: usize,
        original_edge: Edge,
        new_vertex_index: usize,
        new_vertex: Vertex,
        new_edge_index: usize,
        new_edge: Edge,
        new_edge_assignment: Option<EdgeLayerAssignmentV1>,
    },
    RestoreEdgeSplit {
        original_edge_index: usize,
        original_edge: Edge,
        new_vertex_index: usize,
        new_vertex: Vertex,
        new_edge_index: usize,
        new_edge: Edge,
        new_edge_assignment: Option<EdgeLayerAssignmentV1>,
    },
    RestoreEdgeIntersection {
        original_edges: [(usize, Edge); 2],
        new_edges: [(usize, Edge); 2],
        new_vertex_index: usize,
        new_vertex: Vertex,
        new_edge_assignments: Vec<EdgeLayerAssignmentV1>,
    },
    RestoreTJunction {
        original_edge_index: usize,
        original_edge: Edge,
        new_edge_index: usize,
        new_edge: Edge,
        boundary_vertices: Option<Vec<VertexId>>,
        changed_vertices: [VertexId; 4],
        changed_edges: [EdgeId; 3],
        new_edge_assignment: Option<EdgeLayerAssignmentV1>,
    },
    RestoreIntersectionCluster {
        original_boundary_vertices: Option<Vec<VertexId>>,
        original_edges: Vec<(usize, Edge)>,
        inserted_edges: Vec<(usize, Edge)>,
        created_vertex: Option<(usize, Vertex)>,
        junction_vertex: VertexId,
        changed_vertices: Vec<VertexId>,
        changed_edges: Vec<EdgeId>,
        new_edge_assignments: Vec<EdgeLayerAssignmentV1>,
    },
    RestoreBoundaryVertexRemoval {
        boundary_index: usize,
        vertex_index: usize,
        vertex: Vertex,
        kept_edge_index: usize,
        kept_edge: Edge,
        removed_edge_index: usize,
        removed_edge: Edge,
        previous_vertex: VertexId,
        next_vertex: VertexId,
        removed_edge_assignment: Option<(usize, EdgeLayerAssignmentV1)>,
    },
    RemoveAddedGeometricConstraint {
        id: ConstraintId,
    },
    RestoreRemovedGeometricConstraint {
        index: usize,
        record: GeometricConstraintRecordV1,
    },
    RemoveAddedInstructionStep {
        step_id: InstructionStepId,
    },
    RemoveAppendedInstructionSteps {
        step_ids: Vec<InstructionStepId>,
    },
    RestoreInstructionStepMetadata {
        step_id: InstructionStepId,
        title: String,
        description: String,
        caution: String,
        duration_ms: u32,
        visual: InstructionVisual,
    },
    RestoreInstructionStepPose {
        step_id: InstructionStepId,
        pose: InstructionPose,
    },
    RestoreRemovedInstructionStep {
        index: usize,
        step: InstructionStep,
    },
    RestoreInstructionStepOrder {
        step_id: InstructionStepId,
        previous_index: usize,
    },
    RestoreDeletedLayer {
        index: usize,
        layer: LayerRecordV1,
        assignments: Vec<(usize, EdgeLayerAssignmentV1)>,
    },
}

const MAX_INTERSECTION_CLUSTER_TARGETS: usize = 64;

fn is_one_instruction_split_or_merge(
    source: &InstructionTimeline,
    target: &InstructionTimeline,
) -> bool {
    fn split(source: &[InstructionStep], target: &[InstructionStep]) -> bool {
        if target.len() != source.len().saturating_add(1) {
            return false;
        }
        let Some(index) = source.iter().zip(target).position(|(a, b)| a != b) else {
            return false;
        };
        if source[..index] != target[..index] || source[index + 1..] != target[index + 2..] {
            return false;
        }
        let original = &source[index];
        let first = &target[index];
        let second = &target[index + 1];
        if first.id != original.id
            || second.id == original.id
            || target
                .iter()
                .enumerate()
                .any(|(position, step)| position != index + 1 && step.id == second.id)
        {
            return false;
        }
        let mut expected_first = original.clone();
        let mut expected_second = original.clone();
        expected_first.duration_ms = first.duration_ms;
        expected_second.id = second.id;
        expected_second.duration_ms = second.duration_ms;
        first == &expected_first
            && second == &expected_second
            && first.duration_ms.checked_add(second.duration_ms) == Some(original.duration_ms)
    }
    split(&source.steps, &target.steps) || split(&target.steps, &source.steps)
}
const INTERSECTION_CLUSTER_ROUNDOFF_FACTOR: f64 = 16.0;

#[derive(Debug, Clone, Copy)]
enum VertexRecordState {
    Unique(Point2),
    Ambiguous,
}

#[derive(Debug, Clone)]
enum EdgeRecordState {
    Unique { index: usize, edge: Edge },
    Ambiguous,
}

#[derive(Debug, Clone)]
struct PlannedIntersectionTarget {
    original_index: usize,
    original_edge: Edge,
    start_position: Point2,
    end_position: Point2,
    new_edge_id: Option<EdgeId>,
}

#[derive(Debug, Clone)]
struct IntersectionClusterPlan {
    junction_vertex: Vertex,
    create_vertex: bool,
    targets: Vec<PlannedIntersectionTarget>,
    changed_vertices: Vec<VertexId>,
    changed_edges: Vec<EdgeId>,
}

fn intersection_cluster_vertex_records(
    pattern: &CreasePattern,
) -> HashMap<VertexId, VertexRecordState> {
    let mut records = HashMap::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        match records.entry(vertex.id) {
            Entry::Vacant(entry) => {
                entry.insert(VertexRecordState::Unique(vertex.position));
            }
            Entry::Occupied(mut entry) => {
                entry.insert(VertexRecordState::Ambiguous);
            }
        }
    }
    records
}

fn intersection_cluster_edge_records(pattern: &CreasePattern) -> HashMap<EdgeId, EdgeRecordState> {
    let mut records = HashMap::with_capacity(pattern.edges.len());
    for (index, edge) in pattern.edges.iter().enumerate() {
        match records.entry(edge.id) {
            Entry::Vacant(entry) => {
                entry.insert(EdgeRecordState::Unique {
                    index,
                    edge: edge.clone(),
                });
            }
            Entry::Occupied(mut entry) => {
                entry.insert(EdgeRecordState::Ambiguous);
            }
        }
    }
    records
}

fn intersection_cluster_endpoint_position(
    records: &HashMap<VertexId, VertexRecordState>,
    edge: &Edge,
    vertex: VertexId,
) -> Result<Point2, CommandError> {
    let position = match records.get(&vertex) {
        None => return Err(CommandError::VertexNotFound(vertex)),
        Some(VertexRecordState::Ambiguous) => {
            return Err(
                CommandError::IntersectionClusterEndpointVertexRecordAmbiguous {
                    edge: edge.id,
                    vertex,
                },
            );
        }
        Some(VertexRecordState::Unique(position)) => *position,
    };
    if !position.x.is_finite() || !position.y.is_finite() {
        return Err(CommandError::IntersectionClusterEndpointPositionNotFinite {
            edge: edge.id,
            vertex,
        });
    }
    Ok(position)
}

fn intersection_cluster_point_segment_relation(
    point: Point2,
    start: Point2,
    end: Point2,
) -> Result<PointSegmentRelation, GeometryError> {
    let exact = point_segment_relation(point, start, end)?;
    if exact != PointSegmentRelation::Outside {
        return Ok(exact);
    }

    // A correctly rounded rational intersection is usually not an exactly
    // representable point on either source line. Bound only that final
    // coordinate-rounding residue. The tolerance scales with the two local
    // determinant products, never with an arbitrary world-coordinate epsilon,
    // so a translated one-ULP near miss remains visible.
    let direction_x = end.x - start.x;
    let direction_y = end.y - start.y;
    let offset_x = point.x - start.x;
    let offset_y = point.y - start.y;
    if !direction_x.is_finite()
        || !direction_y.is_finite()
        || !offset_x.is_finite()
        || !offset_y.is_finite()
    {
        return Err(GeometryError::ArithmeticOverflow);
    }
    let first_product = direction_x * offset_y;
    let second_product = direction_y * offset_x;
    let determinant = first_product - second_product;
    if !first_product.is_finite() || !second_product.is_finite() || !determinant.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }
    let product_magnitude = first_product.abs() + second_product.abs();
    if !product_magnitude.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }
    let error_bound = INTERSECTION_CLUSTER_ROUNDOFF_FACTOR * f64::EPSILON * product_magnitude;
    if determinant.abs() > error_bound
        || point.x < start.x.min(end.x)
        || point.x > start.x.max(end.x)
        || point.y < start.y.min(end.y)
        || point.y > start.y.max(end.y)
    {
        return Ok(PointSegmentRelation::Outside);
    }
    Ok(PointSegmentRelation::StrictInterior)
}

fn first_strict_intersection_cluster_candidate(
    targets: &[PlannedIntersectionTarget],
) -> Option<Point2> {
    for first_index in 0..targets.len() {
        let first = &targets[first_index];
        for second in &targets[first_index + 1..] {
            // The exact determinant-ratio implementation is invariant under
            // segment reversal and exchange, so each unordered pair supplies
            // one canonical candidate.
            let Some(position) = intersection_cluster_pair_candidate(
                first.start_position,
                first.end_position,
                second.start_position,
                second.end_position,
            ) else {
                continue;
            };
            if targets.iter().all(|target| {
                intersection_cluster_point_segment_relation(
                    position,
                    target.start_position,
                    target.end_position,
                ) == Ok(PointSegmentRelation::StrictInterior)
            }) {
                return Some(position);
            }
        }
    }
    None
}

fn intersection_cluster_pair_candidate(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> Option<Point2> {
    match segment_intersection(first_start, first_end, second_start, second_end) {
        Ok(SegmentIntersection::Point(position)) => Some(position),
        Ok(SegmentIntersection::None | SegmentIntersection::CollinearOverlap) | Err(_) => None,
    }
}

fn plan_intersection_cluster(
    pattern: &CreasePattern,
    paper: &Paper,
    junction: JunctionVertexIntent,
    targets: &[IntersectionEdgeTarget],
) -> Result<IntersectionClusterPlan, CommandError> {
    if targets.len() < 3 {
        return Err(CommandError::IntersectionClusterNeedsThreeTargets {
            actual: targets.len(),
        });
    }
    if targets.len() > MAX_INTERSECTION_CLUSTER_TARGETS {
        return Err(CommandError::IntersectionClusterTooManyTargets {
            actual: targets.len(),
            maximum: MAX_INTERSECTION_CLUSTER_TARGETS,
        });
    }

    let vertex_records = intersection_cluster_vertex_records(pattern);
    let edge_records = intersection_cluster_edge_records(pattern);
    let mut target_ids = HashSet::with_capacity(targets.len());
    let mut generated_ids = HashSet::with_capacity(targets.len());
    let mut planned_targets = Vec::with_capacity(targets.len());

    for target in targets {
        if !target_ids.insert(target.edge) {
            return Err(CommandError::IntersectionClusterTargetDuplicate { edge: target.edge });
        }
        let (original_index, original_edge) = match edge_records.get(&target.edge) {
            None => return Err(CommandError::EdgeNotFound(target.edge)),
            Some(EdgeRecordState::Ambiguous) => {
                return Err(CommandError::IntersectionClusterTargetEdgeIdAmbiguous {
                    edge: target.edge,
                });
            }
            Some(EdgeRecordState::Unique { index, edge }) => (*index, edge.clone()),
        };
        if original_edge.kind == EdgeKind::Boundary {
            return Err(CommandError::IntersectionClusterBoundaryEdge(
                original_edge.id,
            ));
        }
        if let Some(new_edge) = target.new_edge {
            if !generated_ids.insert(new_edge) {
                return Err(CommandError::IntersectionClusterGeneratedEdgeIdDuplicate { new_edge });
            }
            if edge_records.contains_key(&new_edge) {
                return Err(CommandError::EdgeAlreadyExists(new_edge));
            }
        }
        let start_position = intersection_cluster_endpoint_position(
            &vertex_records,
            &original_edge,
            original_edge.start,
        )?;
        let end_position = intersection_cluster_endpoint_position(
            &vertex_records,
            &original_edge,
            original_edge.end,
        )?;
        if start_position == end_position {
            return Err(CommandError::IntersectionClusterZeroLengthEdge {
                edge: original_edge.id,
            });
        }
        planned_targets.push(PlannedIntersectionTarget {
            original_index,
            original_edge,
            start_position,
            end_position,
            new_edge_id: target.new_edge,
        });
    }
    planned_targets.sort_by_key(|target| target.original_index);

    let junction_id = junction.id();
    let junction_position = match junction {
        JunctionVertexIntent::Create { id } => {
            if vertex_records.contains_key(&id)
                || paper.boundary_vertices.contains(&id)
                || pattern
                    .edges
                    .iter()
                    .any(|edge| edge.start == id || edge.end == id)
            {
                return Err(CommandError::VertexAlreadyExists(id));
            }
            let first = &planned_targets[0];
            let second = &planned_targets[1];
            // The original first-pair result remains the fallback and error
            // authority. A later pair can replace only its rounded point when
            // that point is strictly contained by every target.
            let fallback_position = match segment_intersection(
                first.start_position,
                first.end_position,
                second.start_position,
                second.end_position,
            ) {
                Ok(SegmentIntersection::Point(position)) => position,
                Ok(SegmentIntersection::CollinearOverlap) => {
                    return Err(CommandError::IntersectionClusterCollinearOverlap {
                        first_edge: first.original_edge.id,
                        second_edge: second.original_edge.id,
                    });
                }
                Ok(SegmentIntersection::None) => {
                    return Err(CommandError::IntersectionClusterNoSingleIntersection);
                }
                Err(GeometryError::NonFinitePoint { .. } | GeometryError::ArithmeticOverflow) => {
                    return Err(CommandError::IntersectionClusterGeometryNotRepresentable);
                }
            };
            first_strict_intersection_cluster_candidate(&planned_targets)
                .unwrap_or(fallback_position)
        }
        JunctionVertexIntent::Reuse { id } => match vertex_records.get(&id) {
            None => return Err(CommandError::VertexNotFound(id)),
            Some(VertexRecordState::Ambiguous) => {
                return Err(
                    CommandError::IntersectionClusterJunctionVertexRecordAmbiguous { vertex: id },
                );
            }
            Some(VertexRecordState::Unique(position)) => {
                if !position.x.is_finite() || !position.y.is_finite() {
                    return Err(CommandError::IntersectionClusterJunctionPositionNotFinite {
                        vertex: id,
                    });
                }
                *position
            }
        },
    };

    let occupants = pattern
        .vertices
        .iter()
        .filter(|vertex| vertex.position == junction_position)
        .collect::<Vec<_>>();
    match junction {
        JunctionVertexIntent::Create { .. } => match occupants.as_slice() {
            [] => {}
            [vertex] => {
                return Err(CommandError::IntersectionClusterJunctionPositionOccupied {
                    vertex: vertex.id,
                });
            }
            _ => return Err(CommandError::IntersectionClusterJunctionPositionAmbiguous),
        },
        JunctionVertexIntent::Reuse { id } => match occupants.as_slice() {
            [vertex] if vertex.id == id => {}
            [_] => unreachable!("the unique reused vertex must occupy its own position"),
            _ => return Err(CommandError::IntersectionClusterJunctionPositionAmbiguous),
        },
    }

    let mut split_count = 0;
    for target in &planned_targets {
        let relation = intersection_cluster_point_segment_relation(
            junction_position,
            target.start_position,
            target.end_position,
        )
        .map_err(|_| CommandError::IntersectionClusterGeometryNotRepresentable)?;
        match relation {
            PointSegmentRelation::Outside => {
                return Err(CommandError::IntersectionClusterDifferentIntersection {
                    edge: target.original_edge.id,
                });
            }
            PointSegmentRelation::StrictInterior => {
                if target.new_edge_id.is_none() {
                    return Err(CommandError::IntersectionClusterNewEdgeRequired {
                        edge: target.original_edge.id,
                    });
                }
                split_count += 1;
            }
            PointSegmentRelation::Start | PointSegmentRelation::End => {
                let endpoint = if relation == PointSegmentRelation::Start {
                    target.original_edge.start
                } else {
                    target.original_edge.end
                };
                if endpoint != junction_id {
                    return Err(CommandError::IntersectionClusterEndpointVertexMismatch {
                        edge: target.original_edge.id,
                        expected: junction_id,
                        actual: endpoint,
                    });
                }
                if target.new_edge_id.is_some() {
                    return Err(CommandError::IntersectionClusterNewEdgeUnexpected {
                        edge: target.original_edge.id,
                    });
                }
            }
        }
    }
    if split_count == 0 {
        return Err(CommandError::IntersectionClusterNeedsSplit);
    }

    for first_index in 0..planned_targets.len() {
        let first = &planned_targets[first_index];
        for second in &planned_targets[first_index + 1..] {
            match segment_intersection(
                first.start_position,
                first.end_position,
                second.start_position,
                second.end_position,
            ) {
                Ok(SegmentIntersection::Point(_)) => {}
                Ok(SegmentIntersection::None) => {
                    return Err(CommandError::IntersectionClusterDifferentIntersection {
                        edge: second.original_edge.id,
                    });
                }
                Ok(SegmentIntersection::CollinearOverlap) => {
                    return Err(CommandError::IntersectionClusterCollinearOverlap {
                        first_edge: first.original_edge.id,
                        second_edge: second.original_edge.id,
                    });
                }
                Err(GeometryError::NonFinitePoint { .. } | GeometryError::ArithmeticOverflow) => {
                    return Err(CommandError::IntersectionClusterGeometryNotRepresentable);
                }
            }
        }
    }

    for edge in &pattern.edges {
        if target_ids.contains(&edge.id) {
            continue;
        }
        let Some(VertexRecordState::Unique(start_position)) = vertex_records.get(&edge.start)
        else {
            continue;
        };
        let Some(VertexRecordState::Unique(end_position)) = vertex_records.get(&edge.end) else {
            continue;
        };
        if !start_position.x.is_finite()
            || !start_position.y.is_finite()
            || !end_position.x.is_finite()
            || !end_position.y.is_finite()
            || start_position == end_position
        {
            continue;
        }
        let relation = intersection_cluster_point_segment_relation(
            junction_position,
            *start_position,
            *end_position,
        )
        .map_err(|_| CommandError::IntersectionClusterGeometryNotRepresentable)?;
        if relation == PointSegmentRelation::Outside {
            continue;
        }
        if edge.kind == EdgeKind::Boundary {
            return Err(CommandError::IntersectionClusterBoundaryEdge(edge.id));
        }
        return Err(CommandError::IncompleteIntersectionCluster { edge: edge.id });
    }

    let mut affected_vertices = HashSet::with_capacity(planned_targets.len() * 2 + 1);
    affected_vertices.insert(junction_id);
    for target in &planned_targets {
        affected_vertices.insert(target.original_edge.start);
        affected_vertices.insert(target.original_edge.end);
    }
    let mut changed_vertices = Vec::with_capacity(affected_vertices.len());
    for vertex in &pattern.vertices {
        if affected_vertices.remove(&vertex.id) {
            changed_vertices.push(vertex.id);
        }
    }
    if matches!(junction, JunctionVertexIntent::Create { .. }) {
        changed_vertices.push(junction_id);
    }

    let mut changed_edges = Vec::with_capacity(planned_targets.len() + split_count);
    for target in &planned_targets {
        changed_edges.push(target.original_edge.id);
        if let Some(new_edge) = target.new_edge_id {
            changed_edges.push(new_edge);
        }
    }

    Ok(IntersectionClusterPlan {
        junction_vertex: Vertex {
            id: junction_id,
            position: junction_position,
        },
        create_vertex: matches!(junction, JunctionVertexIntent::Create { .. }),
        targets: planned_targets,
        changed_vertices,
        changed_edges,
    })
}

fn intersection_cluster_changes(
    pattern: &CreasePattern,
    junction: JunctionVertexIntent,
    targets: &[IntersectionEdgeTarget],
) -> Changes {
    let mut canonical_targets = targets
        .iter()
        .filter_map(|target| {
            pattern
                .edges
                .iter()
                .enumerate()
                .find(|(_, edge)| edge.id == target.edge)
                .map(|(index, edge)| (index, edge, target.new_edge))
        })
        .collect::<Vec<_>>();
    canonical_targets.sort_by_key(|(index, _, _)| *index);

    let mut affected_vertices = HashSet::with_capacity(canonical_targets.len() * 2 + 1);
    affected_vertices.insert(junction.id());
    for (_, edge, _) in &canonical_targets {
        affected_vertices.insert(edge.start);
        affected_vertices.insert(edge.end);
    }
    let mut vertices = Vec::with_capacity(affected_vertices.len());
    for vertex in &pattern.vertices {
        if affected_vertices.remove(&vertex.id) {
            vertices.push(vertex.id);
        }
    }
    if matches!(junction, JunctionVertexIntent::Create { .. }) && !vertices.contains(&junction.id())
    {
        vertices.push(junction.id());
    }

    let mut edges = Vec::with_capacity(canonical_targets.len() * 2);
    let mut seen_edges = HashSet::with_capacity(canonical_targets.len() * 2);
    for (_, edge, new_edge) in canonical_targets {
        if seen_edges.insert(edge.id) {
            edges.push(edge.id);
        }
        if let Some(new_edge) = new_edge
            && seen_edges.insert(new_edge)
        {
            edges.push(new_edge);
        }
    }
    Changes {
        vertices,
        edges,
        settings: false,
        instructions: false,
        constraints: false,
    }
}

#[derive(Debug, Clone, Copy)]
struct RectangularBoundary {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn undirected_endpoints_match(
    first_start: VertexId,
    first_end: VertexId,
    second_start: VertexId,
    second_end: VertexId,
) -> bool {
    (first_start == second_start && first_end == second_end)
        || (first_start == second_end && first_end == second_start)
}

fn stable_convex_combination(start: f64, end: f64, fraction: f64) -> f64 {
    if start.is_sign_negative() == end.is_sign_negative() {
        start + (end - start) * fraction
    } else {
        start * (1.0 - fraction) + end * fraction
    }
}

fn segment_fraction(start: Point2, end: Point2, point: Point2) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if dx.abs() >= dy.abs() {
        (point.x - start.x) / dx
    } else {
        (point.y - start.y) / dy
    }
}

fn point_lies_on_closed_segment(start: Point2, end: Point2, point: Point2) -> bool {
    exact_orientation(start, end, point).is_ok_and(|side| side == Orientation::Collinear)
        && point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

const fn orientations_are_opposite(first: Orientation, second: Orientation) -> bool {
    matches!(
        (first, second),
        (Orientation::Clockwise, Orientation::CounterClockwise)
            | (Orientation::CounterClockwise, Orientation::Clockwise)
    )
}

fn point_is_strictly_inside_segment(
    point: Point2,
    start: Point2,
    end: Point2,
) -> Result<bool, GeometryError> {
    Ok(point_segment_relation(point, start, end)? == PointSegmentRelation::StrictInterior)
}

fn apply_boundary_vertex_removal(
    pattern: &mut CreasePattern,
    paper: &mut Paper,
    boundary_index: usize,
    vertex_index: usize,
    kept_edge_index: usize,
    removed_edge_index: usize,
    merged_edge: &Edge,
) {
    paper.boundary_vertices.remove(boundary_index);
    pattern.vertices.remove(vertex_index);
    pattern.edges[kept_edge_index] = merged_edge.clone();
    pattern.edges.remove(removed_edge_index);
}

#[derive(Default)]
struct ConstraintMutationTargets {
    all_geometry: bool,
    vertices: HashSet<VertexId>,
    edges: HashSet<EdgeId>,
}

impl ConstraintMutationTargets {
    fn is_empty(&self) -> bool {
        !self.all_geometry && self.vertices.is_empty() && self.edges.is_empty()
    }

    fn is_referenced_by(&self, constraint: &GeometricConstraintKindV1) -> bool {
        if self.all_geometry {
            return true;
        }
        match *constraint {
            GeometricConstraintKindV1::FixedLength { edge, .. }
            | GeometricConstraintKindV1::Horizontal { edge }
            | GeometricConstraintKindV1::Vertical { edge } => self.edges.contains(&edge),
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                ..
            } => {
                self.vertices.contains(&vertex)
                    || self.edges.contains(&first_edge)
                    || self.edges.contains(&second_edge)
            }
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            }
            | GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => self.edges.contains(&first_edge) || self.edges.contains(&second_edge),
            GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                self.vertices.contains(&vertex) || self.edges.contains(&line_edge)
            }
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                axis_edge,
            } => {
                self.vertices.contains(&first_vertex)
                    || self.vertices.contains(&second_vertex)
                    || self.edges.contains(&axis_edge)
            }
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex,
                source_vertex,
                target_vertex,
                ..
            } => {
                self.vertices.contains(&center_vertex)
                    || self.vertices.contains(&source_vertex)
                    || self.vertices.contains(&target_vertex)
            }
            GeometricConstraintKindV1::AngleBisector {
                vertex,
                first_edge,
                second_edge,
                bisector_edge,
            } => {
                self.vertices.contains(&vertex)
                    || self.edges.contains(&first_edge)
                    || self.edges.contains(&second_edge)
                    || self.edges.contains(&bisector_edge)
            }
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ..
            } => self.edges.contains(&numerator_edge) || self.edges.contains(&denominator_edge),
        }
    }
}

fn point_bits_equal(first: Point2, second: Point2) -> bool {
    first.x.to_bits() == second.x.to_bits() && first.y.to_bits() == second.y.to_bits()
}

#[cfg(test)]
thread_local! {
    static CONSTRAINT_LOCK_VISITS: std::cell::Cell<Option<(usize, usize)>> =
        const { std::cell::Cell::new(None) };
}

fn record_constraint_lock_edge_visit() {
    #[cfg(test)]
    CONSTRAINT_LOCK_VISITS.with(|counter| {
        if let Some((edges, constraints)) = counter.get() {
            counter.set(Some((edges + 1, constraints)));
        }
    });
}

fn record_constraint_lock_record_visit() {
    #[cfg(test)]
    CONSTRAINT_LOCK_VISITS.with(|counter| {
        if let Some((edges, constraints)) = counter.get() {
            counter.set(Some((edges, constraints + 1)));
        }
    });
}

#[cfg(test)]
fn begin_constraint_lock_visit_count() {
    CONSTRAINT_LOCK_VISITS.with(|counter| counter.set(Some((0, 0))));
}

#[cfg(test)]
fn finish_constraint_lock_visit_count() -> (usize, usize) {
    CONSTRAINT_LOCK_VISITS.with(|counter| {
        let result = counter
            .get()
            .expect("constraint-lock visit counting must have been started");
        counter.set(None);
        result
    })
}

fn collect_incident_constraint_edges(
    pattern: &CreasePattern,
    vertex: VertexId,
    target_edges: &mut HashSet<EdgeId>,
) {
    for edge in &pattern.edges {
        record_constraint_lock_edge_visit();
        if edge.start == vertex || edge.end == vertex {
            target_edges.insert(edge.id);
        }
    }
}

fn ensure_geometric_constraint_result_count(
    resource: GeometricConstraintResourceV1,
    current: usize,
    added: usize,
    maximum: usize,
) -> Result<(), CommandError> {
    let actual = current
        .checked_add(added)
        .ok_or(CommandError::GeometricConstraintGeometryCountOverflow { resource })?;
    if actual > maximum {
        Err(CommandError::GeometricConstraintGeometryLimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pattern: CreasePattern,
    paper: Paper,
    geometric_constraints: GeometricConstraintDocumentV1,
    instruction_timeline: InstructionTimeline,
    project_layers: ProjectLayerDocumentV1,
    element_metadata: ElementMetadataDocumentV1,
    annotations: AnnotationDocumentV1,
    underlays: UnderlayDocumentV1,
    project_memo: String,
    beginner_design_profile: BeginnerDesignProfileV1,
    /// Non-persisted runtime meaning only; this is not project authority.
    current_applied_pose: Option<AppliedPoseV1>,
    revision: Revision,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    history_entry_limit: usize,
}

impl EditorState {
    #[must_use]
    pub fn new(pattern: CreasePattern) -> Self {
        Self::with_paper(pattern, Paper::default())
    }

    /// Restores an editor from a pattern and its persisted paper definition.
    ///
    /// The restored state starts at revision zero with empty undo and redo
    /// histories. Loading persisted paper data therefore cannot be undone as an
    /// editing operation.
    #[must_use]
    pub fn with_paper(pattern: CreasePattern, paper: Paper) -> Self {
        Self::with_document_parts(pattern, paper, InstructionTimeline { steps: Vec::new() })
    }

    /// Restores all persisted, user-editable document parts.
    ///
    /// The restored state starts at revision zero with empty undo and redo
    /// histories. Validation remains an explicit admission step so callers can
    /// inspect or repair documents created by an older version.
    #[must_use]
    pub fn with_document_parts(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
    ) -> Self {
        Self::with_document_parts_and_constraints(
            pattern,
            paper,
            instruction_timeline,
            GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: Vec::new(),
            },
        )
    }

    /// Restores all persisted, user-editable document parts including authored
    /// geometric constraints.
    ///
    /// Loading is intentionally not an edit and therefore starts with revision
    /// zero and empty history. Admission remains explicit at the persistence
    /// boundary so repairable legacy geometry is not silently rewritten here.
    #[must_use]
    pub fn with_document_parts_and_constraints(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
    ) -> Self {
        Self::with_document_parts_constraints_and_layers(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            ProjectLayerDocumentV1::default(),
        )
    }

    /// Restores every persisted document part managed by the editor.
    ///
    /// As with the compatibility constructors, loading starts at revision zero
    /// with empty history and does not silently rewrite unchecked data.
    #[must_use]
    pub fn with_document_parts_constraints_and_layers(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
    ) -> Self {
        Self::with_all_document_parts(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            ElementMetadataDocumentV1 {
                vertices: Vec::new(),
                edges: Vec::new(),
                faces: Vec::new(),
            },
        )
    }

    #[must_use]
    pub fn with_all_document_parts(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
    ) -> Self {
        Self {
            pattern,
            paper,
            geometric_constraints,
            instruction_timeline,
            project_layers,
            element_metadata,
            annotations: AnnotationDocumentV1 {
                schema_version: ori_domain::ANNOTATION_SCHEMA_VERSION_V1,
                annotations: Vec::new(),
            },
            underlays: UnderlayDocumentV1 {
                schema_version: ori_domain::UNDERLAY_SCHEMA_VERSION_V1,
                underlays: Vec::new(),
            },
            project_memo: String::new(),
            beginner_design_profile: BeginnerDesignProfileV1 {
                schema_version: ori_domain::BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1,
                preset: ori_domain::BeginnerDesignPresetV1::Balanced,
                shape_fidelity_weight: 35,
                foldability_weight: 35,
                step_count_weight: 15,
                paper_efficiency_weight: 15,
                generation_constraints: ori_domain::BeginnerGenerationConstraintsV1::default(),
                generation_provenance: None,
                reference_surface_landmarks_tenths_mm: None,
                outline_edit_authority: None,
                archived_reference_model_asset_ids: Vec::new(),
            },
            current_applied_pose: None,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            history_entry_limit: MAX_EDITOR_HISTORY_ENTRIES,
        }
    }

    #[must_use]
    pub fn with_all_document_parts_and_memo(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        project_memo: String,
    ) -> Self {
        let mut editor = Self::with_all_document_parts(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
        );
        editor.project_memo = project_memo;
        editor
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_all_document_parts_annotations_and_memo(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        annotations: AnnotationDocumentV1,
        project_memo: String,
    ) -> Self {
        let mut editor = Self::with_all_document_parts_and_memo(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
            project_memo,
        );
        editor.annotations = annotations;
        editor
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_all_document_parts_annotations_underlays_and_memo(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        annotations: AnnotationDocumentV1,
        underlays: UnderlayDocumentV1,
        project_memo: String,
    ) -> Self {
        let mut editor = Self::with_all_document_parts_annotations_and_memo(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
            annotations,
            project_memo,
        );
        editor.underlays = underlays;
        editor
    }

    #[must_use]
    pub const fn pattern(&self) -> &CreasePattern {
        &self.pattern
    }

    #[must_use]
    pub const fn annotations(&self) -> &AnnotationDocumentV1 {
        &self.annotations
    }

    #[must_use]
    pub const fn underlays(&self) -> &UnderlayDocumentV1 {
        &self.underlays
    }

    /// Restores the separately validated current underlay document.
    ///
    /// This is load-time state admission, not an editing operation.
    pub fn restore_underlays(&mut self, underlays: UnderlayDocumentV1) {
        self.underlays = underlays;
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    #[must_use]
    pub const fn paper(&self) -> &Paper {
        &self.paper
    }

    #[must_use]
    pub const fn element_metadata(&self) -> &ElementMetadataDocumentV1 {
        &self.element_metadata
    }

    #[must_use]
    pub fn project_memo(&self) -> &str {
        &self.project_memo
    }

    #[must_use]
    pub const fn beginner_design_profile(&self) -> &BeginnerDesignProfileV1 {
        &self.beginner_design_profile
    }

    pub fn restore_beginner_design_profile(
        &mut self,
        profile: BeginnerDesignProfileV1,
    ) -> Result<(), CommandError> {
        if !validate_beginner_design_profile_v1(&profile) {
            return Err(CommandError::InvalidBeginnerDesignProfile);
        }
        self.beginner_design_profile = profile;
        Ok(())
    }

    #[must_use]
    pub const fn instruction_timeline(&self) -> &InstructionTimeline {
        &self.instruction_timeline
    }

    #[must_use]
    pub const fn geometric_constraints(&self) -> &GeometricConstraintDocumentV1 {
        &self.geometric_constraints
    }

    #[must_use]
    pub const fn project_layers(&self) -> &ProjectLayerDocumentV1 {
        &self.project_layers
    }

    /// Returns the non-persisted semantic pose currently shown as applied.
    ///
    /// This value is not a certificate and cannot authorize project mutation.
    #[must_use]
    pub const fn current_applied_pose(&self) -> Option<&AppliedPoseV1> {
        self.current_applied_pose.as_ref()
    }

    /// Replaces the runtime semantic pose without editing the document.
    ///
    /// Revision, undo/redo history, and persisted dirty state are unchanged.
    /// The caller remains responsible for checking a native project-bound
    /// certificate before adopting a prepared semantic value.
    pub fn adopt_current_applied_pose(&mut self, pose: AppliedPoseV1) -> Option<AppliedPoseV1> {
        self.current_applied_pose.replace(pose)
    }

    /// Clears the runtime semantic pose without editing the document.
    ///
    /// Revision, undo/redo history, and persisted dirty state are unchanged.
    pub fn clear_current_applied_pose(&mut self) -> Option<AppliedPoseV1> {
        self.current_applied_pose.take()
    }

    /// Returns the identity of the current fold geometry, independent from
    /// project metadata, history, and the instruction timeline itself.
    #[must_use]
    pub fn fold_model_fingerprint_v1(&self) -> String {
        crate::fold_model_fingerprint::fold_model_fingerprint_v1(&self.pattern, &self.paper)
    }

    #[must_use]
    pub const fn cutting_allowed(&self) -> bool {
        self.paper.cutting_allowed
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns the maximum number of entries retained in each history stack.
    #[must_use]
    pub const fn history_entry_limit(&self) -> usize {
        self.history_entry_limit
    }

    /// Changes the maximum number of entries retained in each history stack.
    ///
    /// Both stacks are trimmed immediately from their oldest side when the
    /// limit shrinks. Increasing the limit cannot recover entries that were
    /// already discarded. This runtime preference does not edit the document,
    /// advance its revision, or change the currently applied pose.
    pub fn set_history_entry_limit(&mut self, limit: usize) -> Result<(), HistoryEntryLimitError> {
        if !(1..=MAX_EDITOR_HISTORY_ENTRIES).contains(&limit) {
            return Err(HistoryEntryLimitError::OutOfRange {
                requested: limit,
                minimum: 1,
                maximum: MAX_EDITOR_HISTORY_ENTRIES,
            });
        }

        trim_history_to_limit(&mut self.undo_stack, limit);
        trim_history_to_limit(&mut self.redo_stack, limit);
        self.history_entry_limit = limit;
        Ok(())
    }

    pub fn execute(
        &mut self,
        expected_revision: Revision,
        command: Command,
    ) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        self.ensure_geometric_constraint_resource_admission(&command)?;
        let result = command.changes(&self.pattern, &self.paper);
        let geometry_before = command
            .may_change_kinematic_geometry()
            .then(|| self.fold_model_fingerprint_v1());
        let inverse = self.apply(&command)?;
        let applied_pose =
            if geometry_before.is_some_and(|before| before != self.fold_model_fingerprint_v1()) {
                AppliedPoseHistoryTransition::Restore {
                    before: self.current_applied_pose.take(),
                    after: None,
                }
            } else {
                AppliedPoseHistoryTransition::PreserveCurrent
            };
        push_bounded_history(
            &mut self.undo_stack,
            HistoryEntry {
                forward: command,
                inverse,
                applied_pose,
            },
            self.history_entry_limit,
        );
        self.redo_stack.clear();
        self.revision = next_revision;
        Ok(self.result(result))
    }

    pub fn execute_stacked_fold_document(
        &mut self,
        expected_revision: Revision,
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        applied_pose: AppliedPoseV1,
    ) -> Result<CommandResult, CommandError> {
        let pose_before = self.current_applied_pose.clone();
        let result = self.execute(
            expected_revision,
            Command::ApplyStackedFoldDocument {
                pattern,
                paper,
                instruction_timeline,
                project_layers,
                beginner_design_profile: Box::new(self.beginner_design_profile.clone()),
            },
        )?;
        self.current_applied_pose = Some(applied_pose.clone());
        let entry = self
            .undo_stack
            .last_mut()
            .ok_or(CommandError::InvalidStackedFoldDocument)?;
        match &mut entry.applied_pose {
            AppliedPoseHistoryTransition::Restore { after, .. } => *after = Some(applied_pose),
            transition @ AppliedPoseHistoryTransition::PreserveCurrent => {
                *transition = AppliedPoseHistoryTransition::Restore {
                    before: pose_before,
                    after: Some(applied_pose),
                };
            }
        }
        Ok(result)
    }

    /// Adds one authored edge and normalizes every non-boundary intersection
    /// it creates as one authenticated history operation.
    pub fn plan_add_edge_with_intersections(
        &self,
        expected_revision: Revision,
        id: EdgeId,
        start: VertexId,
        end: VertexId,
        kind: EdgeKind,
    ) -> Result<Command, CommandError> {
        self.ensure_revision(expected_revision)?;
        let start_position = self
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == start)
            .ok_or(CommandError::VertexNotFound(start))?
            .position;
        let end_position = self
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == end)
            .ok_or(CommandError::VertexNotFound(end))?
            .position;
        let mut points = Vec::<(f64, Point2)>::new();
        for edge in self
            .pattern
            .edges
            .iter()
            .filter(|edge| edge.kind != EdgeKind::Boundary)
        {
            let Some(a) = self
                .pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == edge.start)
                .map(|v| v.position)
            else {
                continue;
            };
            let Some(b) = self
                .pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == edge.end)
                .map(|v| v.position)
            else {
                continue;
            };
            if let Ok(SegmentIntersection::Point(point)) =
                segment_intersection(start_position, end_position, a, b)
            {
                let fraction = segment_fraction(start_position, end_position, point);
                if fraction.is_finite()
                    && (0.0..=1.0).contains(&fraction)
                    && !points.iter().any(|(_, candidate)| *candidate == point)
                {
                    points.push((fraction, point));
                }
            }
        }
        points.sort_by(|left, right| right.0.total_cmp(&left.0));

        let mut staged = self.clone();
        let mut revision = staged.revision();
        staged.execute(
            revision,
            Command::AddEdge {
                id,
                start,
                end,
                kind,
            },
        )?;
        revision += 1;
        for (_, point) in points {
            let mut targets = Vec::new();
            let mut endpoint_ids = Vec::new();
            for edge in staged
                .pattern
                .edges
                .iter()
                .filter(|edge| edge.kind != EdgeKind::Boundary)
            {
                let Some(a) = staged
                    .pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.start)
                    .map(|v| v.position)
                else {
                    continue;
                };
                let Some(b) = staged
                    .pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.end)
                    .map(|v| v.position)
                else {
                    continue;
                };
                if !point_lies_on_closed_segment(a, b, point) {
                    continue;
                }
                let endpoint = if point == a {
                    Some(edge.start)
                } else if point == b {
                    Some(edge.end)
                } else {
                    None
                };
                if let Some(vertex) = endpoint {
                    endpoint_ids.push(vertex);
                }
                targets.push(IntersectionEdgeTarget {
                    edge: edge.id,
                    new_edge: endpoint.is_none().then(EdgeId::new),
                });
            }
            targets.sort_unstable_by_key(|target| target.edge.canonical_bytes());
            targets.dedup_by_key(|target| target.edge);
            endpoint_ids.sort_unstable_by_key(|vertex| vertex.canonical_bytes());
            endpoint_ids.dedup();
            if targets.len() < 2 {
                continue;
            }
            let command = if targets.len() == 2 {
                match (targets[0].new_edge, targets[1].new_edge) {
                    (Some(first_new_edge), Some(second_new_edge)) => {
                        Command::ConnectEdgeIntersection {
                            first_edge: targets[0].edge,
                            second_edge: targets[1].edge,
                            new_vertex: VertexId::new(),
                            first_new_edge,
                            second_new_edge,
                        }
                    }
                    (Some(new_edge), None) | (None, Some(new_edge)) => Command::ConnectTJunction {
                        first_edge: targets[0].edge,
                        second_edge: targets[1].edge,
                        new_edge,
                    },
                    (None, None) => continue,
                }
            } else {
                let junction = match endpoint_ids.as_slice() {
                    [vertex] => JunctionVertexIntent::Reuse { id: *vertex },
                    [] => JunctionVertexIntent::Create {
                        id: VertexId::new(),
                    },
                    _ => return Err(CommandError::IntersectionClusterJunctionPositionAmbiguous),
                };
                Command::ConnectIntersectionCluster { junction, targets }
            };
            staged.execute(revision, command)?;
            revision += 1;
        }

        Ok(Command::ApplyNormalizedEdgeDocument {
            pattern: staged.pattern.clone(),
            project_layers: staged.project_layers.clone(),
        })
    }

    pub fn execute_add_edge_with_intersections(
        &mut self,
        expected_revision: Revision,
        id: EdgeId,
        start: VertexId,
        end: VertexId,
        kind: EdgeKind,
    ) -> Result<CommandResult, CommandError> {
        let command =
            self.plan_add_edge_with_intersections(expected_revision, id, start, end, kind)?;
        self.execute(expected_revision, command)
    }

    pub fn undo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        let Some(mut entry) = self.undo_stack.last().cloned() else {
            return Ok(self.result(Changes::default()));
        };
        entry.applied_pose.capture_after(&self.current_applied_pose);
        let result = entry.inverse.changes(&self.pattern, &self.paper);
        self.apply_inverse(&entry.inverse)?;
        entry
            .applied_pose
            .restore_before(&mut self.current_applied_pose);
        self.undo_stack
            .pop()
            .expect("the successfully applied undo entry must still be present");
        push_bounded_history(&mut self.redo_stack, entry, self.history_entry_limit);
        self.revision = next_revision;
        Ok(self.result(result))
    }

    pub fn redo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        let Some(mut entry) = self.redo_stack.last().cloned() else {
            return Ok(self.result(Changes::default()));
        };
        entry
            .applied_pose
            .capture_before(&self.current_applied_pose);
        let result = entry.forward.changes(&self.pattern, &self.paper);
        self.apply(&entry.forward)?;
        entry
            .applied_pose
            .restore_after(&mut self.current_applied_pose);
        self.redo_stack
            .pop()
            .expect("the successfully applied redo entry must still be present");
        push_bounded_history(&mut self.undo_stack, entry, self.history_entry_limit);
        self.revision = next_revision;
        Ok(self.result(result))
    }

    fn apply(&mut self, command: &Command) -> Result<Inverse, CommandError> {
        self.ensure_geometric_constraint_resource_admission(command)?;
        self.ensure_project_layer_resource_admission(command)?;
        self.ensure_project_layers_allow(command)?;
        self.ensure_geometric_constraints_allow(command)?;
        match *command {
            Command::MirrorSelection {
                ref vertices,
                ref edges,
                axis,
                mode,
                ref new_vertices,
                ref new_edges,
            } => {
                let (pattern, project_layers) = mirror_selection_target(
                    &self.pattern,
                    &self.paper,
                    &self.project_layers,
                    vertices,
                    edges,
                    axis,
                    mode,
                    new_vertices,
                    new_edges,
                )?;
                for record in &self.geometric_constraints.constraints {
                    validate_geometric_constraint_record_against_pattern_v1(&pattern, record)
                        .map_err(CommandError::GeometricConstraintGeometryInvalid)?;
                }
                Ok(Inverse::RestoreMirrorSelection {
                    pattern: std::mem::replace(&mut self.pattern, pattern),
                    project_layers: std::mem::replace(&mut self.project_layers, project_layers),
                })
            }
            Command::ApplyNormalizedEdgeDocument {
                ref pattern,
                ref project_layers,
            } => {
                if (validate_crease_pattern(&self.pattern).is_valid()
                    && validate_paper(&self.paper, &self.pattern).is_valid())
                    && (!validate_crease_pattern(pattern).is_valid()
                        || !validate_paper(&self.paper, pattern).is_valid())
                {
                    return Err(CommandError::InvalidStackedFoldDocument);
                }
                validate_project_layer_document_against_pattern_v1(project_layers, pattern)?;
                for record in &self.geometric_constraints.constraints {
                    validate_geometric_constraint_record_against_pattern_v1(pattern, record)
                        .map_err(CommandError::GeometricConstraintGeometryInvalid)?;
                }
                Ok(Inverse::RestoreMirrorSelection {
                    pattern: std::mem::replace(&mut self.pattern, pattern.clone()),
                    project_layers: std::mem::replace(
                        &mut self.project_layers,
                        project_layers.clone(),
                    ),
                })
            }
            Command::ApplyStackedFoldDocument {
                ref pattern,
                ref paper,
                ref instruction_timeline,
                ref project_layers,
                ref beginner_design_profile,
            } => {
                if paper.thickness_mm.to_bits() != self.paper.thickness_mm.to_bits()
                    || instruction_timeline.steps.len() <= self.instruction_timeline.steps.len()
                    || instruction_timeline.steps.len()
                        > self.instruction_timeline.steps.len().saturating_add(31)
                    || self
                        .pattern
                        .vertices
                        .iter()
                        .any(|source| !pattern.vertices.iter().any(|target| target == source))
                    || !source_edges_preserved_by_exact_subdivision(&self.pattern, pattern)
                    || !validate_crease_pattern(pattern).is_valid()
                    || !validate_paper(paper, pattern).is_valid()
                    || !validate_beginner_design_profile_v1(beginner_design_profile)
                {
                    return Err(CommandError::InvalidStackedFoldDocument);
                }
                validate_instruction_timeline(instruction_timeline)?;
                validate_project_layer_document_against_pattern_v1(project_layers, pattern)?;
                for record in &self.geometric_constraints.constraints {
                    validate_geometric_constraint_record_against_pattern_v1(pattern, record)
                        .map_err(CommandError::GeometricConstraintGeometryInvalid)?;
                }
                Ok(Inverse::RestoreStackedFoldDocument {
                    pattern: std::mem::replace(&mut self.pattern, pattern.clone()),
                    paper: std::mem::replace(&mut self.paper, paper.clone()),
                    instruction_timeline: std::mem::replace(
                        &mut self.instruction_timeline,
                        instruction_timeline.clone(),
                    ),
                    project_layers: std::mem::replace(
                        &mut self.project_layers,
                        project_layers.clone(),
                    ),
                    beginner_design_profile: std::mem::replace(
                        &mut self.beginner_design_profile,
                        beginner_design_profile.as_ref().clone(),
                    )
                    .into(),
                })
            }
            Command::UpdateProjectMemo { ref memo } => {
                const MAX_PROJECT_MEMO_CHARS: usize = 16_000;
                if memo.chars().count() > MAX_PROJECT_MEMO_CHARS
                    || memo.chars().any(|character| {
                        character.is_control() && !matches!(character, '\n' | '\r' | '\t')
                    })
                {
                    return Err(CommandError::InvalidElementMetadata);
                }
                Ok(Inverse::RestoreProjectMemo {
                    memo: std::mem::replace(&mut self.project_memo, memo.clone()),
                })
            }
            Command::UpdateBeginnerDesignProfile { ref profile } => {
                if !validate_beginner_design_profile_v1(profile) {
                    return Err(CommandError::InvalidBeginnerDesignProfile);
                }
                Ok(Inverse::RestoreBeginnerDesignProfile {
                    profile: Box::new(std::mem::replace(
                        &mut self.beginner_design_profile,
                        profile.as_ref().clone(),
                    )),
                })
            }
            Command::SetElementMetadata {
                target,
                ref metadata,
            } => {
                if let Some(metadata) = metadata {
                    validate_element_metadata_v1(metadata)
                        .map_err(|_| CommandError::InvalidElementMetadata)?;
                }
                match target {
                    ElementMetadataTargetV1::Vertex(id) => {
                        self.vertex_index(id)
                            .ok_or(CommandError::VertexNotFound(id))?;
                    }
                    ElementMetadataTargetV1::Edge(id) => {
                        self.edge_index(id).ok_or(CommandError::EdgeNotFound(id))?;
                    }
                    ElementMetadataTargetV1::Face(_) => {}
                }
                let previous = self.replace_element_metadata(target, metadata.clone());
                Ok(Inverse::RestoreElementMetadata {
                    target,
                    metadata: previous,
                })
            }
            Command::AddVertex { id, position } => {
                if self.vertex_index(id).is_some() {
                    return Err(CommandError::VertexAlreadyExists(id));
                }
                self.pattern.vertices.push(Vertex { id, position });
                Ok(Inverse::Command(Command::RemoveVertex { id }))
            }
            Command::MoveVertex { id, position } => {
                let index = self
                    .vertex_index(id)
                    .ok_or(CommandError::VertexNotFound(id))?;
                self.ensure_length_display_reference_survives_vertex_move(id, position)?;
                let previous = self.pattern.vertices[index].position;
                self.pattern.vertices[index].position = position;
                Ok(Inverse::Command(Command::MoveVertex {
                    id,
                    position: previous,
                }))
            }
            Command::MoveEdge {
                id,
                start_position,
                end_position,
            } => {
                let edge = self
                    .pattern
                    .edges
                    .iter()
                    .find(|edge| edge.id == id)
                    .cloned()
                    .ok_or(CommandError::EdgeNotFound(id))?;
                let start_index = self
                    .vertex_index(edge.start)
                    .ok_or(CommandError::VertexNotFound(edge.start))?;
                let end_index = self
                    .vertex_index(edge.end)
                    .ok_or(CommandError::VertexNotFound(edge.end))?;
                if !start_position.x.is_finite() || !start_position.y.is_finite() {
                    return Err(CommandError::VertexMovePositionNotFinite { vertex: edge.start });
                }
                if !end_position.x.is_finite() || !end_position.y.is_finite() {
                    return Err(CommandError::VertexMovePositionNotFinite { vertex: edge.end });
                }
                self.ensure_length_display_reference_survives_vertex_move(
                    edge.start,
                    start_position,
                )?;
                self.ensure_length_display_reference_survives_vertex_move(edge.end, end_position)?;
                let previous_start = self.pattern.vertices[start_index].position;
                let previous_end = self.pattern.vertices[end_index].position;
                self.pattern.vertices[start_index].position = start_position;
                self.pattern.vertices[end_index].position = end_position;
                Ok(Inverse::Command(Command::MoveEdge {
                    id,
                    start_position: previous_start,
                    end_position: previous_end,
                }))
            }
            Command::MoveVertices { ref updates } => {
                if updates.is_empty() || updates.len() > DEFAULT_MAX_CONSTRAINT_VERTICES {
                    return Err(CommandError::InvalidVertexMoveBatch);
                }
                let mut seen = std::collections::HashSet::with_capacity(updates.len());
                let mut planned = Vec::with_capacity(updates.len());
                for update in updates {
                    if !seen.insert(update.vertex) {
                        return Err(CommandError::InvalidVertexMoveBatch);
                    }
                    if !update.position.x.is_finite() || !update.position.y.is_finite() {
                        return Err(CommandError::VertexMovePositionNotFinite {
                            vertex: update.vertex,
                        });
                    }
                    let index = self
                        .vertex_index(update.vertex)
                        .ok_or(CommandError::VertexNotFound(update.vertex))?;
                    self.ensure_length_display_reference_survives_vertex_move(
                        update.vertex,
                        update.position,
                    )?;
                    planned.push((index, self.pattern.vertices[index].position, *update));
                }
                for (index, _, update) in &planned {
                    self.pattern.vertices[*index].position = update.position;
                }
                Ok(Inverse::Command(Command::MoveVertices {
                    updates: planned
                        .into_iter()
                        .map(|(_, previous, update)| VertexPositionUpdate {
                            vertex: update.vertex,
                            position: previous,
                        })
                        .collect(),
                }))
            }
            Command::RemoveVertex { id } => {
                if let Some(edge) = self
                    .pattern
                    .edges
                    .iter()
                    .find(|edge| edge.start == id || edge.end == id)
                {
                    return Err(CommandError::VertexHasConnectedEdge {
                        vertex: id,
                        edge: edge.id,
                    });
                }
                let index = self
                    .vertex_index(id)
                    .ok_or(CommandError::VertexNotFound(id))?;
                let vertex = self.pattern.vertices.remove(index);
                Ok(Inverse::RestoreVertex { index, vertex })
            }
            Command::AddEdge {
                id,
                start,
                end,
                kind,
            } => {
                if kind == EdgeKind::Boundary {
                    return Err(CommandError::BoundaryEdgeRequiresSheetOperation(id));
                }
                if kind == EdgeKind::Cut && !self.paper.cutting_allowed {
                    return Err(CommandError::CuttingDisabled);
                }
                if self.edge_index(id).is_some() {
                    return Err(CommandError::EdgeAlreadyExists(id));
                }
                if start == end {
                    return Err(CommandError::DegenerateEdge(start));
                }
                if self.vertex_index(start).is_none() {
                    return Err(CommandError::VertexNotFound(start));
                }
                if self.vertex_index(end).is_none() {
                    return Err(CommandError::VertexNotFound(end));
                }
                self.pattern.edges.push(Edge {
                    id,
                    start,
                    end,
                    kind,
                });
                Ok(Inverse::Command(Command::RemoveEdge { id }))
            }
            Command::AddConnectedVertex {
                vertex_id,
                position,
                edge_id,
                start,
                kind,
            } => {
                if kind == EdgeKind::Boundary {
                    return Err(CommandError::BoundaryEdgeRequiresSheetOperation(edge_id));
                }
                if kind == EdgeKind::Cut && !self.paper.cutting_allowed {
                    return Err(CommandError::CuttingDisabled);
                }
                if self.vertex_index(vertex_id).is_some() {
                    return Err(CommandError::VertexAlreadyExists(vertex_id));
                }
                if self.edge_index(edge_id).is_some() {
                    return Err(CommandError::EdgeAlreadyExists(edge_id));
                }
                if self.vertex_index(start).is_none() {
                    return Err(CommandError::VertexNotFound(start));
                }
                self.pattern.vertices.push(Vertex {
                    id: vertex_id,
                    position,
                });
                self.pattern.edges.push(Edge {
                    id: edge_id,
                    start,
                    end: vertex_id,
                    kind,
                });
                Ok(Inverse::Command(Command::RemoveConnectedVertex {
                    vertex_id,
                    edge_id,
                }))
            }
            Command::RemoveConnectedVertex { vertex_id, edge_id } => {
                let edge_index = self
                    .edge_index(edge_id)
                    .ok_or(CommandError::EdgeNotFound(edge_id))?;
                let edge = self.pattern.edges[edge_index].clone();
                if edge.end != vertex_id {
                    return Err(CommandError::VertexHasConnectedEdge {
                        vertex: vertex_id,
                        edge: edge_id,
                    });
                }
                if let Some(other) = self.pattern.edges.iter().find(|candidate| {
                    candidate.id != edge_id
                        && (candidate.start == vertex_id || candidate.end == vertex_id)
                }) {
                    return Err(CommandError::VertexHasConnectedEdge {
                        vertex: vertex_id,
                        edge: other.id,
                    });
                }
                let vertex_index = self
                    .vertex_index(vertex_id)
                    .ok_or(CommandError::VertexNotFound(vertex_id))?;
                let vertex = self.pattern.vertices[vertex_index].clone();
                self.pattern.edges.remove(edge_index);
                self.pattern.vertices.remove(vertex_index);
                Ok(Inverse::Command(Command::AddConnectedVertex {
                    vertex_id,
                    position: vertex.position,
                    edge_id,
                    start: edge.start,
                    kind: edge.kind,
                }))
            }
            Command::RemoveEdge { id } => {
                let index = self.edge_index(id).ok_or(CommandError::EdgeNotFound(id))?;
                if self.pattern.edges[index].kind == EdgeKind::Boundary {
                    return Err(CommandError::BoundaryEdgeRequiresSheetOperation(id));
                }
                let edge = self.pattern.edges.remove(index);
                let layer_assignment = self.remove_explicit_layer_assignment(id);
                Ok(Inverse::RestoreEdge {
                    index,
                    edge,
                    layer_assignment,
                })
            }
            Command::SetCuttingAllowed { allowed } => {
                self.ensure_cutting_can_be_set(allowed)?;
                let previous = self.paper.cutting_allowed;
                self.paper.cutting_allowed = allowed;
                Ok(Inverse::RestorePaperProperties {
                    thickness_mm: self.paper.thickness_mm,
                    front_color: self.paper.front.color,
                    back_color: self.paper.back.color,
                    front_texture_asset: self.paper.front.texture_asset,
                    back_texture_asset: self.paper.back.texture_asset,
                    cutting_allowed: previous,
                })
            }
            Command::UpdatePaperProperties {
                thickness_mm,
                front_color,
                back_color,
                front_texture_asset,
                back_texture_asset,
                cutting_allowed,
            } => {
                Self::validate_paper_thickness(thickness_mm)?;
                self.ensure_cutting_can_be_set(cutting_allowed)?;
                let inverse = Inverse::RestorePaperProperties {
                    thickness_mm: self.paper.thickness_mm,
                    front_color: self.paper.front.color,
                    back_color: self.paper.back.color,
                    front_texture_asset: self.paper.front.texture_asset,
                    back_texture_asset: self.paper.back.texture_asset,
                    cutting_allowed: self.paper.cutting_allowed,
                };
                self.paper.thickness_mm = thickness_mm;
                self.paper.front.color = front_color;
                self.paper.back.color = back_color;
                self.paper.front.texture_asset = front_texture_asset;
                self.paper.back.texture_asset = back_texture_asset;
                self.paper.cutting_allowed = cutting_allowed;
                Ok(inverse)
            }
            Command::SetLengthDisplayUnit { unit } => {
                if let LengthDisplayUnit::PaperEdgeRatio { reference_edge } = unit {
                    self.validated_length_display_reference_edge_length(reference_edge)?;
                }
                let previous = self.paper.length_display_unit;
                self.paper.length_display_unit = unit;
                Ok(Inverse::RestoreLengthDisplayUnit { unit: previous })
            }
            Command::ResizeRectangularPaper {
                width_mm,
                height_mm,
            } => self.resize_rectangular_paper(width_mm, height_mm),
            Command::SplitEdge {
                edge,
                new_vertex,
                new_edge,
                fraction,
            } => self.split_edge(edge, new_vertex, new_edge, fraction),
            Command::ConnectEdgeIntersection {
                first_edge,
                second_edge,
                new_vertex,
                first_new_edge,
                second_new_edge,
            } => self.connect_edge_intersection(
                first_edge,
                second_edge,
                new_vertex,
                first_new_edge,
                second_new_edge,
            ),
            Command::ConnectTJunction {
                first_edge,
                second_edge,
                new_edge,
            } => self.connect_t_junction(first_edge, second_edge, new_edge),
            Command::ConnectIntersectionCluster {
                junction,
                ref targets,
            } => self.connect_intersection_cluster(junction, targets),
            Command::SplitBoundaryEdge {
                edge,
                new_vertex,
                new_edge,
                fraction,
            } => self.split_boundary_edge(edge, new_vertex, new_edge, fraction),
            Command::RemoveBoundaryVertex { vertex } => self.remove_boundary_vertex(vertex),
            Command::AddGeometricConstraint { ref record } => {
                if self
                    .geometric_constraints
                    .constraints
                    .iter()
                    .any(|candidate| candidate.id == record.id)
                {
                    return Err(CommandError::GeometricConstraintAlreadyExists(record.id));
                }
                let mut candidate = self.geometric_constraints.clone();
                candidate.constraints.push(record.clone());
                // Persisted-domain validation intentionally precedes all
                // reference and geometry checks, establishing one stable
                // public error order for every constraint kind.
                validate_geometric_constraint_document_v1(&candidate)?;
                self.validate_new_constraint_geometry(record)?;
                self.geometric_constraints = candidate;
                Ok(Inverse::RemoveAddedGeometricConstraint { id: record.id })
            }
            Command::RemoveGeometricConstraint { id } => {
                let mut candidate = self.geometric_constraints.clone();
                let index = candidate
                    .constraints
                    .iter()
                    .position(|record| record.id == id)
                    .ok_or(CommandError::GeometricConstraintNotFound(id))?;
                let record = candidate.constraints.remove(index);
                // Removal is a monotonic repair operation. A loaded document
                // is allowed to be temporarily invalid, so unrelated
                // remaining records are deliberately not revalidated here.
                self.geometric_constraints = candidate;
                Ok(Inverse::RestoreRemovedGeometricConstraint { index, record })
            }
            Command::AddAnnotation { ref record } => {
                if self
                    .annotations
                    .annotations
                    .iter()
                    .any(|item| item.id == record.id)
                {
                    return Err(CommandError::AnnotationAlreadyExists(record.id));
                }
                self.validate_annotation_record(record)?;
                self.annotations.annotations.push(record.clone());
                self.annotations
                    .annotations
                    .sort_by_key(|item| item.id.canonical_bytes());
                Ok(Inverse::Command(Command::RemoveAnnotation {
                    id: record.id,
                }))
            }
            Command::UpdateAnnotation { ref record } => {
                self.validate_annotation_record(record)?;
                let current = self
                    .annotations
                    .annotations
                    .iter_mut()
                    .find(|item| item.id == record.id)
                    .ok_or(CommandError::AnnotationNotFound(record.id))?;
                let previous = std::mem::replace(current, record.clone());
                Ok(Inverse::Command(Command::UpdateAnnotation {
                    record: previous,
                }))
            }
            Command::RemoveAnnotation { id } => {
                let index = self
                    .annotations
                    .annotations
                    .iter()
                    .position(|item| item.id == id)
                    .ok_or(CommandError::AnnotationNotFound(id))?;
                let record = self.annotations.annotations.remove(index);
                Ok(Inverse::Command(Command::AddAnnotation { record }))
            }
            Command::AddUnderlay { ref record } => {
                if self
                    .underlays
                    .underlays
                    .iter()
                    .any(|item| item.id == record.id)
                {
                    return Err(CommandError::UnderlayAlreadyExists(record.id));
                }
                self.validate_underlay_record(record)?;
                self.underlays.underlays.push(record.clone());
                self.underlays
                    .underlays
                    .sort_by_key(|item| item.id.canonical_bytes());
                Ok(Inverse::Command(Command::RemoveUnderlay { id: record.id }))
            }
            Command::UpdateUnderlay { ref record } => {
                self.validate_underlay_record(record)?;
                let current = self
                    .underlays
                    .underlays
                    .iter_mut()
                    .find(|item| item.id == record.id)
                    .ok_or(CommandError::UnderlayNotFound(record.id))?;
                let previous = std::mem::replace(current, record.clone());
                Ok(Inverse::Command(Command::UpdateUnderlay {
                    record: previous,
                }))
            }
            Command::RemoveUnderlay { id } => {
                let index = self
                    .underlays
                    .underlays
                    .iter()
                    .position(|item| item.id == id)
                    .ok_or(CommandError::UnderlayNotFound(id))?;
                let record = self.underlays.underlays.remove(index);
                Ok(Inverse::Command(Command::AddUnderlay { record }))
            }
            Command::AddInstructionStep { ref step } => {
                if self
                    .instruction_timeline
                    .steps
                    .iter()
                    .any(|candidate| candidate.id == step.id)
                {
                    return Err(CommandError::InstructionStepAlreadyExists(step.id));
                }
                let mut candidate = self.instruction_timeline.clone();
                candidate.steps.push(step.clone());
                self.commit_instruction_timeline(candidate)?;
                Ok(Inverse::RemoveAddedInstructionStep { step_id: step.id })
            }
            Command::AppendInstructionSteps { ref steps } => {
                if steps.is_empty() {
                    return Err(CommandError::InstructionStepAppendBatchEmpty);
                }
                for step in steps {
                    if self
                        .instruction_timeline
                        .steps
                        .iter()
                        .any(|candidate| candidate.id == step.id)
                    {
                        return Err(CommandError::InstructionStepAlreadyExists(step.id));
                    }
                }
                let mut candidate = self.instruction_timeline.clone();
                candidate.steps.extend(steps.iter().cloned());
                self.commit_instruction_timeline(candidate)?;
                Ok(Inverse::RemoveAppendedInstructionSteps {
                    step_ids: steps.iter().map(|step| step.id).collect(),
                })
            }
            Command::UpdateInstructionStepMetadata {
                step_id,
                ref title,
                ref description,
                ref caution,
                duration_ms,
                ref visual,
            } => {
                let mut candidate = self.instruction_timeline.clone();
                let step = candidate
                    .steps
                    .iter_mut()
                    .find(|step| step.id == step_id)
                    .ok_or(CommandError::InstructionStepNotFound(step_id))?;
                let inverse = Inverse::RestoreInstructionStepMetadata {
                    step_id,
                    title: step.title.clone(),
                    description: step.description.clone(),
                    caution: step.caution.clone(),
                    duration_ms: step.duration_ms,
                    visual: step.visual.clone(),
                };
                step.title.clone_from(title);
                step.description.clone_from(description);
                step.caution.clone_from(caution);
                step.duration_ms = duration_ms;
                step.visual.clone_from(visual);
                self.commit_instruction_timeline(candidate)?;
                Ok(inverse)
            }
            Command::ReplaceInstructionStepPose { step_id, ref pose } => {
                let mut candidate = self.instruction_timeline.clone();
                let step = candidate
                    .steps
                    .iter_mut()
                    .find(|step| step.id == step_id)
                    .ok_or(CommandError::InstructionStepNotFound(step_id))?;
                if step.pose.model == ori_domain::InstructionPoseModel::DeclarativeOnlyV1 {
                    return Err(CommandError::DeclarativeInstructionPoseImmutable);
                }
                let inverse = Inverse::RestoreInstructionStepPose {
                    step_id,
                    pose: step.pose.clone(),
                };
                step.pose.clone_from(pose);
                self.commit_instruction_timeline(candidate)?;
                Ok(inverse)
            }
            Command::RemoveInstructionStep { step_id } => {
                let mut candidate = self.instruction_timeline.clone();
                let index = candidate
                    .steps
                    .iter()
                    .position(|step| step.id == step_id)
                    .ok_or(CommandError::InstructionStepNotFound(step_id))?;
                let step = candidate.steps.remove(index);
                self.commit_instruction_timeline(candidate)?;
                Ok(Inverse::RestoreRemovedInstructionStep { index, step })
            }
            Command::MoveInstructionStep {
                step_id,
                target_index,
            } => {
                let mut candidate = self.instruction_timeline.clone();
                let index = candidate
                    .steps
                    .iter()
                    .position(|step| step.id == step_id)
                    .ok_or(CommandError::InstructionStepNotFound(step_id))?;
                if target_index >= candidate.steps.len() {
                    return Err(CommandError::InstructionStepTargetIndexOutOfBounds {
                        target_index,
                        step_count: candidate.steps.len(),
                    });
                }
                let step = candidate.steps.remove(index);
                candidate.steps.insert(target_index, step);
                self.commit_instruction_timeline(candidate)?;
                Ok(Inverse::RestoreInstructionStepOrder {
                    step_id,
                    previous_index: index,
                })
            }
            Command::RewriteInstructionTimelineSplitMerge { ref timeline } => {
                if !is_one_instruction_split_or_merge(&self.instruction_timeline, timeline) {
                    return Err(CommandError::InstructionStepAppendHistoryMismatch);
                }
                let previous = self.instruction_timeline.clone();
                self.commit_instruction_timeline(timeline.clone())?;
                Ok(Inverse::Command(
                    Command::RewriteInstructionTimelineSplitMerge { timeline: previous },
                ))
            }
            Command::CreateLayer {
                ref layer,
                target_index,
            } => {
                if self
                    .project_layers
                    .layers
                    .iter()
                    .any(|candidate| candidate.id == layer.id)
                {
                    return Err(CommandError::LayerAlreadyExists(layer.id));
                }
                if target_index > self.project_layers.layers.len() {
                    return Err(CommandError::LayerTargetIndexOutOfBounds {
                        target_index,
                        layer_count: self.project_layers.layers.len(),
                    });
                }
                let mut candidate = self.project_layers.clone();
                candidate.layers.insert(target_index, layer.clone());
                self.commit_project_layers(candidate)?;
                Ok(Inverse::Command(Command::DeleteLayer { layer: layer.id }))
            }
            Command::RenameLayer { layer, ref name } => {
                let mut candidate = self.project_layers.clone();
                let record = candidate
                    .layers
                    .iter_mut()
                    .find(|record| record.id == layer)
                    .ok_or(CommandError::LayerNotFound(layer))?;
                let previous = std::mem::replace(&mut record.name, name.clone());
                self.commit_project_layers(candidate)?;
                Ok(Inverse::Command(Command::RenameLayer {
                    layer,
                    name: previous,
                }))
            }
            Command::UpdateLayerPresentation {
                layer,
                visible,
                locked,
                opacity,
            } => {
                let mut candidate = self.project_layers.clone();
                let record = candidate
                    .layers
                    .iter_mut()
                    .find(|record| record.id == layer)
                    .ok_or(CommandError::LayerNotFound(layer))?;
                let inverse = Inverse::Command(Command::UpdateLayerPresentation {
                    layer,
                    visible: record.visible,
                    locked: record.locked,
                    opacity: record.opacity,
                });
                record.visible = visible;
                record.locked = locked;
                record.opacity = opacity;
                self.commit_project_layers(candidate)?;
                Ok(inverse)
            }
            Command::MoveLayer {
                layer,
                target_index,
            } => {
                let mut candidate = self.project_layers.clone();
                let previous_index = candidate
                    .layers
                    .iter()
                    .position(|record| record.id == layer)
                    .ok_or(CommandError::LayerNotFound(layer))?;
                if target_index >= candidate.layers.len() {
                    return Err(CommandError::LayerTargetIndexOutOfBounds {
                        target_index,
                        layer_count: candidate.layers.len(),
                    });
                }
                let record = candidate.layers.remove(previous_index);
                candidate.layers.insert(target_index, record);
                self.commit_project_layers(candidate)?;
                Ok(Inverse::Command(Command::MoveLayer {
                    layer,
                    target_index: previous_index,
                }))
            }
            Command::DeleteLayer { layer } => {
                if layer == DEFAULT_PROJECT_LAYER_ID {
                    return Err(CommandError::DefaultLayerDeletionForbidden);
                }
                let mut candidate = self.project_layers.clone();
                let index = candidate
                    .layers
                    .iter()
                    .position(|record| record.id == layer)
                    .ok_or(CommandError::LayerNotFound(layer))?;
                let record = candidate.layers.remove(index);
                let assignments = candidate
                    .edge_assignments
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(_, assignment)| assignment.layer == layer)
                    .collect::<Vec<_>>();
                candidate
                    .edge_assignments
                    .retain(|assignment| assignment.layer != layer);
                self.commit_project_layers(candidate)?;
                Ok(Inverse::RestoreDeletedLayer {
                    index,
                    layer: record,
                    assignments,
                })
            }
            Command::AssignEdgeToLayer { edge, layer } => {
                let mut edge_records = self.pattern.edges.iter().filter(|record| record.id == edge);
                if edge_records.next().is_none() {
                    return Err(CommandError::EdgeNotFound(edge));
                }
                if edge_records.next().is_some() {
                    return Err(CommandError::LayerAssignmentEdgeIdAmbiguous { edge });
                }
                if !self
                    .project_layers
                    .layers
                    .iter()
                    .any(|record| record.id == layer)
                {
                    return Err(CommandError::LayerNotFound(layer));
                }

                let mut candidate = self.project_layers.clone();
                let key = edge.canonical_bytes();
                let assignment_index = candidate
                    .edge_assignments
                    .binary_search_by_key(&key, |assignment| assignment.edge.canonical_bytes());
                let previous_layer = match assignment_index {
                    Ok(index) => candidate.edge_assignments[index].layer,
                    Err(_) => DEFAULT_PROJECT_LAYER_ID,
                };
                match (assignment_index, layer == DEFAULT_PROJECT_LAYER_ID) {
                    (Ok(index), true) => {
                        candidate.edge_assignments.remove(index);
                    }
                    (Ok(index), false) => {
                        candidate.edge_assignments[index].layer = layer;
                    }
                    (Err(_), true) => {}
                    (Err(index), false) => {
                        self.ensure_layer_assignment_capacity(1)?;
                        candidate
                            .edge_assignments
                            .insert(index, EdgeLayerAssignmentV1 { edge, layer });
                    }
                }
                self.commit_project_layers(candidate)?;
                Ok(Inverse::Command(Command::AssignEdgeToLayer {
                    edge,
                    layer: previous_layer,
                }))
            }
        }
    }

    fn commit_instruction_timeline(
        &mut self,
        candidate: InstructionTimeline,
    ) -> Result<(), CommandError> {
        validate_instruction_timeline(&candidate)?;
        self.instruction_timeline = candidate;
        Ok(())
    }

    fn commit_project_layers(
        &mut self,
        candidate: ProjectLayerDocumentV1,
    ) -> Result<(), CommandError> {
        validate_project_layer_document_against_pattern_v1(&candidate, &self.pattern)?;
        self.project_layers = candidate;
        Ok(())
    }

    fn ensure_layer_assignment_capacity(&self, added: usize) -> Result<(), CommandError> {
        let current = self.project_layers.edge_assignments.len();
        if current
            .checked_add(added)
            .is_none_or(|total| total > MAX_LAYER_EDGE_ASSIGNMENTS)
        {
            Err(CommandError::TooManyLayerEdgeAssignments {
                current,
                added,
                maximum: MAX_LAYER_EDGE_ASSIGNMENTS,
            })
        } else {
            Ok(())
        }
    }

    fn ensure_project_layer_resource_admission(
        &self,
        command: &Command,
    ) -> Result<(), CommandError> {
        if self.project_layers.edge_assignments.is_empty() {
            return Ok(());
        }
        let added_edges = command
            .geometric_resource_growth()?
            .map_or(0, |(_, added_edges)| added_edges);
        if added_edges == 0 {
            return Ok(());
        }
        let actual = self.pattern.edges.len().saturating_add(added_edges);
        if actual > MAX_PROJECT_LAYER_INDEX_EDGES {
            Err(CommandError::ProjectLayerDocumentInvalid(
                ProjectLayerDocumentValidationErrorV1::TooManyPatternEdges {
                    actual,
                    maximum: MAX_PROJECT_LAYER_INDEX_EDGES,
                },
            ))
        } else {
            Ok(())
        }
    }

    /// Rejects every geometry or assignment edit that can affect a locked
    /// project layer. Vertices are implicit members of the default layer and
    /// also affect every incident edge layer when moved or removed.
    fn ensure_project_layers_allow(&self, command: &Command) -> Result<(), CommandError> {
        match command {
            Command::AddAnnotation { record } | Command::UpdateAnnotation { record } => {
                self.ensure_layer_unlocked(record.layer)?;
            }
            Command::AddUnderlay { record } | Command::UpdateUnderlay { record } => {
                self.ensure_layer_unlocked(record.layer)?;
            }
            Command::RemoveUnderlay { id } => {
                let record = self.underlays.underlays.iter().find(|item| item.id == *id)
                    .ok_or(CommandError::UnderlayNotFound(*id))?;
                self.ensure_layer_unlocked(record.layer)?;
            }
            Command::RemoveAnnotation { id } => {
                let record = self.annotations.annotations.iter().find(|item| item.id == *id)
                    .ok_or(CommandError::AnnotationNotFound(*id))?;
                self.ensure_layer_unlocked(record.layer)?;
            }
            Command::DeleteLayer { layer } if self.annotations.annotations.iter()
                .any(|annotation| annotation.layer == *layer) => {
                return Err(CommandError::InvalidAnnotation);
            }
            Command::DeleteLayer { layer } if self.underlays.underlays.iter()
                .any(|underlay| underlay.layer == *layer) => {
                return Err(CommandError::InvalidUnderlay);
            }
            Command::RemoveVertex { id } if self.annotations.annotations.iter().any(|annotation| {
                matches!(annotation.anchor, ori_domain::AnnotationAnchorV1::Vertex { vertex, .. } if vertex == *id)
            }) => return Err(CommandError::InvalidAnnotation),
            _ => {}
        }
        match command {
            Command::AddVertex { .. }
            | Command::AddEdge { .. }
            | Command::AddConnectedVertex { .. } => {
                self.ensure_layer_unlocked(DEFAULT_PROJECT_LAYER_ID)
            }
            Command::MoveVertex { id, .. } | Command::RemoveVertex { id } => {
                if self.vertex_index(*id).is_none() {
                    return Ok(());
                }
                let mut has_incident_edge = false;
                for edge in self
                    .pattern
                    .edges
                    .iter()
                    .filter(|edge| edge.start == *id || edge.end == *id)
                {
                    has_incident_edge = true;
                    self.ensure_edge_layer_unlocked(edge.id)?;
                }
                if has_incident_edge {
                    Ok(())
                } else {
                    self.ensure_layer_unlocked(DEFAULT_PROJECT_LAYER_ID)
                }
            }
            Command::MoveEdge { id, .. } => {
                let Some(edge) = self.pattern.edges.iter().find(|edge| edge.id == *id) else {
                    return Ok(());
                };
                self.ensure_edge_layer_unlocked(*id)?;
                for incident in self.pattern.edges.iter().filter(|candidate| {
                    candidate.start == edge.start
                        || candidate.end == edge.start
                        || candidate.start == edge.end
                        || candidate.end == edge.end
                }) {
                    self.ensure_edge_layer_unlocked(incident.id)?;
                }
                Ok(())
            }
            Command::MoveVertices { updates } => {
                for update in updates {
                    if self.vertex_index(update.vertex).is_none() {
                        continue;
                    }
                    let mut has_incident_edge = false;
                    for edge in self
                        .pattern
                        .edges
                        .iter()
                        .filter(|edge| edge.start == update.vertex || edge.end == update.vertex)
                    {
                        has_incident_edge = true;
                        self.ensure_edge_layer_unlocked(edge.id)?;
                    }
                    if !has_incident_edge {
                        self.ensure_layer_unlocked(DEFAULT_PROJECT_LAYER_ID)?;
                    }
                }
                Ok(())
            }
            Command::RemoveEdge { id }
            | Command::RemoveConnectedVertex { edge_id: id, .. }
            | Command::SplitEdge { edge: id, .. }
            | Command::SplitBoundaryEdge { edge: id, .. } => self.ensure_edge_layer_unlocked(*id),
            Command::ConnectEdgeIntersection {
                first_edge,
                second_edge,
                ..
            }
            | Command::ConnectTJunction {
                first_edge,
                second_edge,
                ..
            } => {
                self.ensure_edge_layer_unlocked(*first_edge)?;
                self.ensure_edge_layer_unlocked(*second_edge)
            }
            Command::ConnectIntersectionCluster { targets, .. } => {
                for target in targets {
                    self.ensure_edge_layer_unlocked(target.edge)?;
                }
                Ok(())
            }
            Command::RemoveBoundaryVertex { vertex } => {
                if self.vertex_index(*vertex).is_none() {
                    return Ok(());
                }
                let mut has_incident_edge = false;
                for edge in self
                    .pattern
                    .edges
                    .iter()
                    .filter(|edge| edge.start == *vertex || edge.end == *vertex)
                {
                    has_incident_edge = true;
                    self.ensure_edge_layer_unlocked(edge.id)?;
                }
                if has_incident_edge {
                    Ok(())
                } else {
                    self.ensure_layer_unlocked(DEFAULT_PROJECT_LAYER_ID)
                }
            }
            Command::ResizeRectangularPaper { .. } => {
                if self.pattern.edges.is_empty() {
                    return self.ensure_layer_unlocked(DEFAULT_PROJECT_LAYER_ID);
                }
                for edge in &self.pattern.edges {
                    self.ensure_edge_layer_unlocked(edge.id)?;
                }
                Ok(())
            }
            Command::DeleteLayer { layer } => self.ensure_layer_unlocked(*layer),
            Command::AssignEdgeToLayer { edge, layer } => {
                self.ensure_edge_layer_unlocked(*edge)?;
                self.ensure_layer_unlocked(*layer)
            }
            Command::UpdateProjectMemo { .. }
            | Command::UpdateBeginnerDesignProfile { .. }
            | Command::SetElementMetadata { .. }
            | Command::SetCuttingAllowed { .. }
            | Command::UpdatePaperProperties { .. }
            | Command::SetLengthDisplayUnit { .. }
            | Command::AddGeometricConstraint { .. }
            | Command::RemoveGeometricConstraint { .. }
            | Command::AddAnnotation { .. }
            | Command::UpdateAnnotation { .. }
            | Command::RemoveAnnotation { .. }
            | Command::AddUnderlay { .. }
            | Command::UpdateUnderlay { .. }
            | Command::RemoveUnderlay { .. }
            | Command::AddInstructionStep { .. }
            | Command::AppendInstructionSteps { .. }
            | Command::UpdateInstructionStepMetadata { .. }
            | Command::ReplaceInstructionStepPose { .. }
            | Command::RemoveInstructionStep { .. }
            | Command::MoveInstructionStep { .. }
            | Command::RewriteInstructionTimelineSplitMerge { .. }
            | Command::CreateLayer { .. }
            | Command::RenameLayer { .. }
            | Command::UpdateLayerPresentation { .. }
            | Command::MoveLayer { .. }
            | Command::MirrorSelection { .. }
            | Command::ApplyNormalizedEdgeDocument { .. }
            | Command::ApplyStackedFoldDocument { .. } => Ok(()),
        }
    }

    fn ensure_edge_layer_unlocked(&self, edge: EdgeId) -> Result<(), CommandError> {
        if self.edge_index(edge).is_none() {
            return Ok(());
        }
        self.ensure_layer_unlocked(self.project_layers.layer_for_edge(edge))
    }

    fn ensure_layer_unlocked(&self, layer: LayerId) -> Result<(), CommandError> {
        if self
            .project_layers
            .layers
            .iter()
            .find(|record| record.id == layer)
            .is_some_and(|record| record.locked)
        {
            Err(CommandError::LayerLocked(layer))
        } else {
            Ok(())
        }
    }

    fn explicit_layer_assignment(&self, edge: EdgeId) -> Option<(usize, EdgeLayerAssignmentV1)> {
        let key = edge.canonical_bytes();
        self.project_layers
            .edge_assignments
            .binary_search_by_key(&key, |assignment| assignment.edge.canonical_bytes())
            .ok()
            .map(|index| (index, self.project_layers.edge_assignments[index]))
    }

    fn inherited_layer_assignment(
        &self,
        source_edge: EdgeId,
        new_edge: EdgeId,
    ) -> Option<EdgeLayerAssignmentV1> {
        self.explicit_layer_assignment(source_edge)
            .map(|(_, assignment)| EdgeLayerAssignmentV1 {
                edge: new_edge,
                layer: assignment.layer,
            })
    }

    fn insert_explicit_layer_assignment(&mut self, assignment: EdgeLayerAssignmentV1) {
        let key = assignment.edge.canonical_bytes();
        let index = self
            .project_layers
            .edge_assignments
            .binary_search_by_key(&key, |candidate| candidate.edge.canonical_bytes())
            .expect_err("a generated edge cannot already have a layer assignment");
        self.project_layers
            .edge_assignments
            .insert(index, assignment);
    }

    fn remove_explicit_layer_assignment(
        &mut self,
        edge: EdgeId,
    ) -> Option<(usize, EdgeLayerAssignmentV1)> {
        let (index, assignment) = self.explicit_layer_assignment(edge)?;
        self.project_layers.edge_assignments.remove(index);
        Some((index, assignment))
    }

    fn remove_expected_layer_assignment(
        &mut self,
        expected: Option<EdgeLayerAssignmentV1>,
    ) -> Result<(), CommandError> {
        let Some(expected) = expected else {
            return Ok(());
        };
        let Some((index, actual)) = self.explicit_layer_assignment(expected.edge) else {
            return Err(CommandError::LayerHistoryAssignmentMismatch {
                edge: expected.edge,
            });
        };
        if actual != expected {
            return Err(CommandError::LayerHistoryAssignmentMismatch {
                edge: expected.edge,
            });
        }
        self.project_layers.edge_assignments.remove(index);
        Ok(())
    }

    fn validate_new_constraint_geometry(
        &self,
        record: &GeometricConstraintRecordV1,
    ) -> Result<(), CommandError> {
        match validate_geometric_constraint_record_against_pattern_v1(&self.pattern, record) {
            Ok(()) => Ok(()),
            Err(GeometricConstraintErrorV1::MissingVertex { vertex, .. }) => {
                Err(CommandError::VertexNotFound(vertex))
            }
            Err(GeometricConstraintErrorV1::MissingEdge { edge, .. }) => {
                Err(CommandError::EdgeNotFound(edge))
            }
            Err(error) => Err(CommandError::GeometricConstraintGeometryInvalid(error)),
        }
    }

    fn ensure_geometric_constraint_resource_admission(
        &self,
        command: &Command,
    ) -> Result<(), CommandError> {
        let adds_constraint = matches!(command, Command::AddGeometricConstraint { .. });
        if self.geometric_constraints.is_empty() && !adds_constraint {
            return Ok(());
        }

        let Some((added_vertices, added_edges)) = command.geometric_resource_growth()? else {
            if !adds_constraint {
                return Ok(());
            }
            return self.ensure_geometric_constraint_result_counts(0, 0);
        };
        self.ensure_geometric_constraint_result_counts(added_vertices, added_edges)
    }

    fn ensure_geometric_constraint_result_counts(
        &self,
        added_vertices: usize,
        added_edges: usize,
    ) -> Result<(), CommandError> {
        ensure_geometric_constraint_result_count(
            GeometricConstraintResourceV1::Vertices,
            self.pattern.vertices.len(),
            added_vertices,
            DEFAULT_MAX_CONSTRAINT_VERTICES,
        )?;
        ensure_geometric_constraint_result_count(
            GeometricConstraintResourceV1::Edges,
            self.pattern.edges.len(),
            added_edges,
            DEFAULT_MAX_CONSTRAINT_EDGES,
        )
    }

    fn ensure_geometric_constraints_allow(&self, command: &Command) -> Result<(), CommandError> {
        if self.geometric_constraints.constraints.is_empty() {
            return Ok(());
        }
        if let Command::MoveVertices { updates } = command {
            let mut candidate = self.pattern.clone();
            let mut complete = true;
            for update in updates {
                if let Some(vertex) = candidate
                    .vertices
                    .iter_mut()
                    .find(|vertex| vertex.id == update.vertex)
                {
                    vertex.position = update.position;
                } else {
                    complete = false;
                    break;
                }
            }
            if complete
                && crate::verify_geometric_constraint_solution_v1(
                    &candidate,
                    &self.geometric_constraints,
                    crate::ConstraintSolveLimitsV1::default().residual_tolerance,
                )
                .is_ok()
            {
                return Ok(());
            }
        }
        let targets = self.constraint_mutation_targets(command)?;
        if targets.is_empty() {
            return Ok(());
        }

        let mut blocker = None;
        for record in &self.geometric_constraints.constraints {
            record_constraint_lock_record_visit();
            if targets.is_referenced_by(&record.constraint)
                && blocker.is_none_or(|current: &GeometricConstraintRecordV1| {
                    record.id.canonical_bytes() < current.id.canonical_bytes()
                })
            {
                blocker = Some(record);
            }
        }
        if let Some(record) = blocker {
            Err(CommandError::GeometricConstraintBlocksGeometryMutation {
                constraint: record.id,
            })
        } else {
            Ok(())
        }
    }

    fn validate_annotation_record(&self, record: &AnnotationRecordV1) -> Result<(), CommandError> {
        let layer = self
            .project_layers
            .layers
            .iter()
            .find(|layer| layer.id == record.layer)
            .filter(|layer| {
                layer.content_kind == ori_domain::LayerContentKindV1::Annotation && !layer.locked
            })
            .ok_or(CommandError::InvalidAnnotation)?;
        let _ = layer;
        if let ori_domain::AnnotationAnchorV1::Vertex { vertex, .. } = record.anchor
            && !self.pattern.vertices.iter().any(|item| item.id == vertex)
        {
            return Err(CommandError::InvalidAnnotation);
        }
        let mut document = AnnotationDocumentV1::default();
        document.annotations.push(record.clone());
        ori_domain::validate_annotation_document_v1(&document)
            .map_err(|_| CommandError::InvalidAnnotation)
    }

    fn validate_underlay_record(&self, record: &UnderlayRecordV1) -> Result<(), CommandError> {
        self.project_layers
            .layers
            .iter()
            .find(|layer| layer.id == record.layer)
            .filter(|layer| {
                layer.content_kind == ori_domain::LayerContentKindV1::Underlay && !layer.locked
            })
            .ok_or(CommandError::InvalidUnderlay)?;
        let mut document = UnderlayDocumentV1::default();
        document.underlays.push(record.clone());
        validate_underlay_document_v1(&document).map_err(|_| CommandError::InvalidUnderlay)
    }

    fn constraint_mutation_targets(
        &self,
        command: &Command,
    ) -> Result<ConstraintMutationTargets, CommandError> {
        let mut targets = ConstraintMutationTargets::default();
        match command {
            Command::MoveVertex { id, position } => {
                if self.vertex_index(*id).is_some_and(|index| {
                    point_bits_equal(self.pattern.vertices[index].position, *position)
                }) {
                    return Ok(targets);
                }
                targets.vertices.insert(*id);
                collect_incident_constraint_edges(&self.pattern, *id, &mut targets.edges);
            }
            Command::MoveEdge {
                id,
                start_position,
                end_position,
            } => {
                if let Some(edge) = self.pattern.edges.iter().find(|edge| edge.id == *id) {
                    if let Some(index) = self.vertex_index(edge.start)
                        && !point_bits_equal(self.pattern.vertices[index].position, *start_position)
                    {
                        targets.vertices.insert(edge.start);
                        collect_incident_constraint_edges(
                            &self.pattern,
                            edge.start,
                            &mut targets.edges,
                        );
                    }
                    if let Some(index) = self.vertex_index(edge.end)
                        && !point_bits_equal(self.pattern.vertices[index].position, *end_position)
                    {
                        targets.vertices.insert(edge.end);
                        collect_incident_constraint_edges(
                            &self.pattern,
                            edge.end,
                            &mut targets.edges,
                        );
                    }
                }
            }
            Command::MoveVertices { updates } => {
                for update in updates {
                    if self.vertex_index(update.vertex).is_some_and(|index| {
                        !point_bits_equal(self.pattern.vertices[index].position, update.position)
                    }) {
                        targets.vertices.insert(update.vertex);
                        collect_incident_constraint_edges(
                            &self.pattern,
                            update.vertex,
                            &mut targets.edges,
                        );
                    }
                }
            }
            Command::RemoveVertex { id } => {
                targets.vertices.insert(*id);
                collect_incident_constraint_edges(&self.pattern, *id, &mut targets.edges);
            }
            Command::RemoveEdge { id }
            | Command::RemoveConnectedVertex {
                vertex_id: _,
                edge_id: id,
            }
            | Command::SplitEdge { edge: id, .. }
            | Command::SplitBoundaryEdge { edge: id, .. } => {
                targets.edges.insert(*id);
            }
            Command::ResizeRectangularPaper {
                width_mm,
                height_mm,
            } => {
                let resized = self.planned_rectangular_positions(*width_mm, *height_mm)?;
                targets.all_geometry = self
                    .pattern
                    .vertices
                    .iter()
                    .zip(resized)
                    .any(|(vertex, position)| !point_bits_equal(vertex.position, position));
            }
            Command::ConnectEdgeIntersection {
                first_edge,
                second_edge,
                ..
            }
            | Command::ConnectTJunction {
                first_edge,
                second_edge,
                ..
            } => {
                targets.edges.insert(*first_edge);
                targets.edges.insert(*second_edge);
            }
            Command::ConnectIntersectionCluster {
                targets: cluster_targets,
                ..
            } => {
                targets
                    .edges
                    .extend(cluster_targets.iter().map(|target| target.edge));
            }
            Command::RemoveBoundaryVertex { vertex } => {
                targets.vertices.insert(*vertex);
                collect_incident_constraint_edges(&self.pattern, *vertex, &mut targets.edges);
            }
            Command::AddVertex { .. }
            | Command::AddEdge { .. }
            | Command::AddConnectedVertex { .. }
            | Command::UpdateProjectMemo { .. }
            | Command::UpdateBeginnerDesignProfile { .. }
            | Command::SetElementMetadata { .. }
            | Command::SetCuttingAllowed { .. }
            | Command::UpdatePaperProperties { .. }
            | Command::SetLengthDisplayUnit { .. }
            | Command::AddGeometricConstraint { .. }
            | Command::RemoveGeometricConstraint { .. }
            | Command::AddAnnotation { .. }
            | Command::UpdateAnnotation { .. }
            | Command::RemoveAnnotation { .. }
            | Command::AddUnderlay { .. }
            | Command::UpdateUnderlay { .. }
            | Command::RemoveUnderlay { .. }
            | Command::AddInstructionStep { .. }
            | Command::AppendInstructionSteps { .. }
            | Command::UpdateInstructionStepMetadata { .. }
            | Command::ReplaceInstructionStepPose { .. }
            | Command::RemoveInstructionStep { .. }
            | Command::MoveInstructionStep { .. }
            | Command::RewriteInstructionTimelineSplitMerge { .. }
            | Command::CreateLayer { .. }
            | Command::RenameLayer { .. }
            | Command::UpdateLayerPresentation { .. }
            | Command::MoveLayer { .. }
            | Command::DeleteLayer { .. }
            | Command::AssignEdgeToLayer { .. }
            | Command::MirrorSelection { .. }
            | Command::ApplyNormalizedEdgeDocument { .. }
            | Command::ApplyStackedFoldDocument { .. } => {}
        }
        Ok(targets)
    }

    fn remove_boundary_vertex(&mut self, vertex_id: VertexId) -> Result<Inverse, CommandError> {
        let boundary_len = self.paper.boundary_vertices.len();
        if boundary_len < 4 {
            return Err(CommandError::BoundaryVertexRemovalNeedsFourVertices {
                actual: boundary_len,
            });
        }

        let mut boundary_occurrences = self
            .paper
            .boundary_vertices
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == vertex_id);
        let Some((boundary_index, _)) = boundary_occurrences.next() else {
            return Err(CommandError::VertexNotInPaperBoundary(vertex_id));
        };
        if boundary_occurrences.next().is_some() {
            return Err(CommandError::BoundaryVertexOccursMultipleTimes { vertex: vertex_id });
        }

        let mut vertex_records = self
            .pattern
            .vertices
            .iter()
            .enumerate()
            .filter(|(_, vertex)| vertex.id == vertex_id);
        let Some((vertex_index, vertex)) = vertex_records
            .next()
            .map(|(index, vertex)| (index, vertex.clone()))
        else {
            return Err(CommandError::VertexNotFound(vertex_id));
        };
        if vertex_records.next().is_some() {
            return Err(CommandError::BoundaryVertexRecordAmbiguous { vertex: vertex_id });
        }

        let previous_vertex =
            self.paper.boundary_vertices[(boundary_index + boundary_len - 1) % boundary_len];
        let next_vertex = self.paper.boundary_vertices[(boundary_index + 1) % boundary_len];
        if previous_vertex == next_vertex {
            return Err(CommandError::BoundaryVertexNeighborsNotDistinct {
                vertex: vertex_id,
                neighbor: previous_vertex,
            });
        }
        let preceding_edges = self.matching_boundary_edge_indices(previous_vertex, vertex_id);
        let kept_edge_index = match preceding_edges.as_slice() {
            [] => {
                return Err(CommandError::BoundaryVertexPrecedingEdgeMissing { vertex: vertex_id });
            }
            [index] => *index,
            _ => {
                return Err(CommandError::BoundaryVertexPrecedingEdgeAmbiguous {
                    vertex: vertex_id,
                });
            }
        };
        let following_edges = self.matching_boundary_edge_indices(vertex_id, next_vertex);
        let removed_edge_index = match following_edges.as_slice() {
            [] => {
                return Err(CommandError::BoundaryVertexFollowingEdgeMissing { vertex: vertex_id });
            }
            [index] => *index,
            _ => {
                return Err(CommandError::BoundaryVertexFollowingEdgeAmbiguous {
                    vertex: vertex_id,
                });
            }
        };
        let kept_edge = self.pattern.edges[kept_edge_index].clone();
        let removed_edge = self.pattern.edges[removed_edge_index].clone();
        if kept_edge_index == removed_edge_index || kept_edge.id == removed_edge.id {
            return Err(CommandError::BoundaryVertexAdjacentEdgesNotDistinct { vertex: vertex_id });
        }
        for edge in [&kept_edge, &removed_edge] {
            if self
                .pattern
                .edges
                .iter()
                .filter(|candidate| candidate.id == edge.id)
                .count()
                != 1
            {
                return Err(CommandError::BoundaryVertexAdjacentEdgeIdAmbiguous {
                    vertex: vertex_id,
                    edge: edge.id,
                });
            }
        }
        self.ensure_length_display_reference_edge_not_mutated(kept_edge.id)?;
        self.ensure_length_display_reference_edge_not_mutated(removed_edge.id)?;

        if let Some(edge) = self
            .pattern
            .edges
            .iter()
            .enumerate()
            .find(|(index, edge)| {
                *index != kept_edge_index
                    && *index != removed_edge_index
                    && (edge.start == vertex_id || edge.end == vertex_id)
            })
            .map(|(_, edge)| edge)
        {
            return Err(CommandError::BoundaryVertexHasAdditionalEdge {
                vertex: vertex_id,
                edge: edge.id,
            });
        }
        if let Some(edge) = self.pattern.edges.iter().find(|edge| {
            undirected_endpoints_match(edge.start, edge.end, previous_vertex, next_vertex)
        }) {
            return Err(CommandError::BoundaryVertexNeighborEdgeAlreadyExists {
                vertex: vertex_id,
                edge: edge.id,
            });
        }

        let mut merged_edge = kept_edge.clone();
        if merged_edge.start == vertex_id {
            merged_edge.start = next_vertex;
        } else {
            debug_assert_eq!(merged_edge.end, vertex_id);
            merged_edge.end = next_vertex;
        }

        let current_crease_is_valid = validate_crease_pattern(&self.pattern).is_valid();
        let current_paper_is_valid = validate_paper(&self.paper, &self.pattern).is_valid();
        if current_crease_is_valid && current_paper_is_valid {
            let mut candidate_pattern = self.pattern.clone();
            let mut candidate_paper = self.paper.clone();
            apply_boundary_vertex_removal(
                &mut candidate_pattern,
                &mut candidate_paper,
                boundary_index,
                vertex_index,
                kept_edge_index,
                removed_edge_index,
                &merged_edge,
            );
            if !validate_crease_pattern(&candidate_pattern).is_valid()
                || !validate_paper(&candidate_paper, &candidate_pattern).is_valid()
            {
                return Err(CommandError::BoundaryVertexRemovalWouldInvalidatePaper);
            }
        }

        let removed_edge_assignment = self.explicit_layer_assignment(removed_edge.id);
        apply_boundary_vertex_removal(
            &mut self.pattern,
            &mut self.paper,
            boundary_index,
            vertex_index,
            kept_edge_index,
            removed_edge_index,
            &merged_edge,
        );
        if let Some((assignment_index, _)) = removed_edge_assignment {
            self.project_layers
                .edge_assignments
                .remove(assignment_index);
        }

        Ok(Inverse::RestoreBoundaryVertexRemoval {
            boundary_index,
            vertex_index,
            vertex,
            kept_edge_index,
            kept_edge,
            removed_edge_index,
            removed_edge,
            previous_vertex,
            next_vertex,
            removed_edge_assignment,
        })
    }

    fn matching_boundary_edge_indices(&self, start: VertexId, end: VertexId) -> Vec<usize> {
        self.pattern
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.kind == EdgeKind::Boundary
                    && undirected_endpoints_match(edge.start, edge.end, start, end)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn split_edge(
        &mut self,
        edge_id: EdgeId,
        new_vertex_id: VertexId,
        new_edge_id: EdgeId,
        fraction: f64,
    ) -> Result<Inverse, CommandError> {
        if !fraction.is_finite() {
            return Err(CommandError::EdgeSplitFractionNotFinite);
        }
        if fraction <= 0.0 || fraction >= 1.0 {
            return Err(CommandError::EdgeSplitFractionOutOfRange);
        }
        if self.vertex_index(new_vertex_id).is_some()
            || self.paper.boundary_vertices.contains(&new_vertex_id)
            || self
                .pattern
                .edges
                .iter()
                .any(|edge| edge.start == new_vertex_id || edge.end == new_vertex_id)
        {
            return Err(CommandError::VertexAlreadyExists(new_vertex_id));
        }
        if self.edge_index(new_edge_id).is_some() {
            return Err(CommandError::EdgeAlreadyExists(new_edge_id));
        }

        let mut target_edges = self
            .pattern
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.id == edge_id);
        let Some((original_edge_index, original_edge)) = target_edges
            .next()
            .map(|(index, edge)| (index, edge.clone()))
        else {
            return Err(CommandError::EdgeNotFound(edge_id));
        };
        if target_edges.next().is_some() {
            return Err(CommandError::EdgeSplitTargetEdgeIdAmbiguous { edge: edge_id });
        }
        if original_edge.kind == EdgeKind::Boundary {
            return Err(CommandError::BoundaryEdgeRequiresSheetOperation(edge_id));
        }

        let unique_endpoint_index = |vertex_id| {
            let mut matches = self
                .pattern
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, vertex)| vertex.id == vertex_id);
            let Some((index, _)) = matches.next() else {
                return Err(CommandError::VertexNotFound(vertex_id));
            };
            if matches.next().is_some() {
                return Err(CommandError::EdgeSplitEndpointVertexRecordAmbiguous {
                    edge: edge_id,
                    vertex: vertex_id,
                });
            }
            Ok(index)
        };
        let start_index = unique_endpoint_index(original_edge.start)?;
        let end_index = unique_endpoint_index(original_edge.end)?;
        let start_position = self.pattern.vertices[start_index].position;
        let end_position = self.pattern.vertices[end_index].position;
        if !start_position.x.is_finite() || !start_position.y.is_finite() {
            return Err(CommandError::EdgeSplitEndpointPositionNotFinite {
                edge: edge_id,
                vertex: original_edge.start,
            });
        }
        if !end_position.x.is_finite() || !end_position.y.is_finite() {
            return Err(CommandError::EdgeSplitEndpointPositionNotFinite {
                edge: edge_id,
                vertex: original_edge.end,
            });
        }
        let position = Point2::new(
            stable_convex_combination(start_position.x, end_position.x, fraction),
            stable_convex_combination(start_position.y, end_position.y, fraction),
        );
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(CommandError::EdgeSplitPositionNotFinite);
        }
        if position == start_position || position == end_position {
            return Err(CommandError::EdgeSplitPositionNotDistinct);
        }
        if let Some(vertex) = self
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
        {
            return Err(CommandError::EdgeSplitPositionOccupied { vertex: vertex.id });
        }

        let new_vertex_index = self.pattern.vertices.len();
        let new_edge_index = original_edge_index + 1;
        let new_vertex = Vertex {
            id: new_vertex_id,
            position,
        };
        let new_edge = Edge {
            id: new_edge_id,
            start: new_vertex_id,
            end: original_edge.end,
            kind: original_edge.kind,
        };
        let new_edge_assignment = self.inherited_layer_assignment(original_edge.id, new_edge_id);
        self.ensure_layer_assignment_capacity(usize::from(new_edge_assignment.is_some()))?;

        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[original_edge_index].end = new_vertex_id;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());
        if let Some(assignment) = new_edge_assignment {
            self.insert_explicit_layer_assignment(assignment);
        }

        Ok(Inverse::RestoreEdgeSplit {
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
            new_edge_assignment,
        })
    }

    fn connect_edge_intersection(
        &mut self,
        first_edge_id: EdgeId,
        second_edge_id: EdgeId,
        new_vertex_id: VertexId,
        first_new_edge_id: EdgeId,
        second_new_edge_id: EdgeId,
    ) -> Result<Inverse, CommandError> {
        if first_edge_id == second_edge_id {
            return Err(CommandError::EdgeIntersectionTargetsNotDistinct);
        }
        if first_new_edge_id == second_new_edge_id {
            return Err(CommandError::EdgeIntersectionNewEdgeIdsNotDistinct);
        }
        if self.vertex_index(new_vertex_id).is_some()
            || self.paper.boundary_vertices.contains(&new_vertex_id)
            || self
                .pattern
                .edges
                .iter()
                .any(|edge| edge.start == new_vertex_id || edge.end == new_vertex_id)
        {
            return Err(CommandError::VertexAlreadyExists(new_vertex_id));
        }
        for new_edge_id in [first_new_edge_id, second_new_edge_id] {
            if self.edge_index(new_edge_id).is_some() {
                return Err(CommandError::EdgeAlreadyExists(new_edge_id));
            }
        }

        let unique_target = |edge_id| {
            let mut matches = self
                .pattern
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| edge.id == edge_id);
            let Some((index, edge)) = matches.next() else {
                return Err(CommandError::EdgeNotFound(edge_id));
            };
            if matches.next().is_some() {
                return Err(CommandError::EdgeIntersectionTargetEdgeIdAmbiguous { edge: edge_id });
            }
            Ok((index, edge.clone()))
        };
        let (first_edge_index, first_edge) = unique_target(first_edge_id)?;
        let (second_edge_index, second_edge) = unique_target(second_edge_id)?;
        for edge in [&first_edge, &second_edge] {
            if edge.kind == EdgeKind::Boundary {
                return Err(CommandError::EdgeIntersectionBoundaryEdge(edge.id));
            }
        }

        let unique_endpoint_position = |edge: &Edge, vertex_id| {
            let mut matches = self
                .pattern
                .vertices
                .iter()
                .filter(|vertex| vertex.id == vertex_id);
            let Some(vertex) = matches.next() else {
                return Err(CommandError::VertexNotFound(vertex_id));
            };
            if matches.next().is_some() {
                return Err(
                    CommandError::EdgeIntersectionEndpointVertexRecordAmbiguous {
                        edge: edge.id,
                        vertex: vertex_id,
                    },
                );
            }
            if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
                return Err(CommandError::EdgeIntersectionEndpointPositionNotFinite {
                    edge: edge.id,
                    vertex: vertex_id,
                });
            }
            Ok(vertex.position)
        };
        let first_start = unique_endpoint_position(&first_edge, first_edge.start)?;
        let first_end = unique_endpoint_position(&first_edge, first_edge.end)?;
        let second_start = unique_endpoint_position(&second_edge, second_edge.start)?;
        let second_end = unique_endpoint_position(&second_edge, second_edge.end)?;

        let position = match segment_intersection(first_start, first_end, second_start, second_end)
        {
            Ok(SegmentIntersection::Point(position)) => position,
            Ok(SegmentIntersection::None | SegmentIntersection::CollinearOverlap) => {
                return Err(CommandError::EdgeIntersectionNotSinglePoint);
            }
            Err(GeometryError::NonFinitePoint { .. } | GeometryError::ArithmeticOverflow) => {
                return Err(CommandError::EdgeIntersectionGeometryNotRepresentable);
            }
        };
        let first_side_start = exact_orientation(first_start, first_end, second_start)
            .map_err(|_| CommandError::EdgeIntersectionGeometryNotRepresentable)?;
        let first_side_end = exact_orientation(first_start, first_end, second_end)
            .map_err(|_| CommandError::EdgeIntersectionGeometryNotRepresentable)?;
        let second_side_start = exact_orientation(second_start, second_end, first_start)
            .map_err(|_| CommandError::EdgeIntersectionGeometryNotRepresentable)?;
        let second_side_end = exact_orientation(second_start, second_end, first_end)
            .map_err(|_| CommandError::EdgeIntersectionGeometryNotRepresentable)?;
        if !orientations_are_opposite(first_side_start, first_side_end)
            || !orientations_are_opposite(second_side_start, second_side_end)
            || [first_start, first_end, second_start, second_end].contains(&position)
        {
            return Err(CommandError::EdgeIntersectionNotProper);
        }
        if let Some(vertex) = self
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
        {
            return Err(CommandError::EdgeIntersectionPositionOccupied { vertex: vertex.id });
        }

        let new_vertex_index = self.pattern.vertices.len();
        let new_vertex = Vertex {
            id: new_vertex_id,
            position,
        };
        let mut splits = [
            (first_edge_index, first_edge, first_new_edge_id),
            (second_edge_index, second_edge, second_new_edge_id),
        ];
        splits.sort_by_key(|(index, _, _)| *index);
        let original_edges = [
            (splits[0].0, splits[0].1.clone()),
            (splits[1].0, splits[1].1.clone()),
        ];
        let created_edges = [
            Edge {
                id: splits[0].2,
                start: new_vertex_id,
                end: splits[0].1.end,
                kind: splits[0].1.kind,
            },
            Edge {
                id: splits[1].2,
                start: new_vertex_id,
                end: splits[1].1.end,
                kind: splits[1].1.kind,
            },
        ];
        let mut new_edge_assignments = [
            self.inherited_layer_assignment(first_edge_id, first_new_edge_id),
            self.inherited_layer_assignment(second_edge_id, second_new_edge_id),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        new_edge_assignments.sort_unstable_by_key(|assignment| assignment.edge.canonical_bytes());
        self.ensure_layer_assignment_capacity(new_edge_assignments.len())?;

        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[splits[0].0].end = new_vertex_id;
        self.pattern.edges[splits[1].0].end = new_vertex_id;
        self.pattern
            .edges
            .insert(splits[1].0 + 1, created_edges[1].clone());
        self.pattern
            .edges
            .insert(splits[0].0 + 1, created_edges[0].clone());
        for assignment in &new_edge_assignments {
            self.insert_explicit_layer_assignment(*assignment);
        }

        Ok(Inverse::RestoreEdgeIntersection {
            original_edges,
            new_edges: [
                (splits[0].0 + 1, created_edges[0].clone()),
                (splits[1].0 + 2, created_edges[1].clone()),
            ],
            new_vertex_index,
            new_vertex,
            new_edge_assignments,
        })
    }

    fn connect_t_junction(
        &mut self,
        first_edge_id: EdgeId,
        second_edge_id: EdgeId,
        new_edge_id: EdgeId,
    ) -> Result<Inverse, CommandError> {
        if first_edge_id == second_edge_id {
            return Err(CommandError::TJunctionTargetsNotDistinct);
        }
        if self.edge_index(new_edge_id).is_some() {
            return Err(CommandError::EdgeAlreadyExists(new_edge_id));
        }

        let unique_target = |edge_id| {
            let mut matches = self
                .pattern
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| edge.id == edge_id);
            let Some((index, edge)) = matches.next() else {
                return Err(CommandError::EdgeNotFound(edge_id));
            };
            if matches.next().is_some() {
                return Err(CommandError::TJunctionTargetEdgeIdAmbiguous { edge: edge_id });
            }
            Ok((index, edge.clone()))
        };
        let (first_edge_index, first_edge) = unique_target(first_edge_id)?;
        let (second_edge_index, second_edge) = unique_target(second_edge_id)?;
        if first_edge.kind == EdgeKind::Boundary && second_edge.kind == EdgeKind::Boundary {
            return Err(CommandError::TJunctionBothEdgesBoundary);
        }

        let unique_endpoint_position = |edge: &Edge, vertex_id| {
            let mut matches = self
                .pattern
                .vertices
                .iter()
                .filter(|vertex| vertex.id == vertex_id);
            let Some(vertex) = matches.next() else {
                return Err(CommandError::VertexNotFound(vertex_id));
            };
            if matches.next().is_some() {
                return Err(CommandError::TJunctionEndpointVertexRecordAmbiguous {
                    edge: edge.id,
                    vertex: vertex_id,
                });
            }
            if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
                return Err(CommandError::TJunctionEndpointPositionNotFinite {
                    edge: edge.id,
                    vertex: vertex_id,
                });
            }
            Ok(vertex.position)
        };
        let first_start = unique_endpoint_position(&first_edge, first_edge.start)?;
        let first_end = unique_endpoint_position(&first_edge, first_edge.end)?;
        let second_start = unique_endpoint_position(&second_edge, second_edge.start)?;
        let second_end = unique_endpoint_position(&second_edge, second_edge.end)?;

        match segment_intersection(first_start, first_end, second_start, second_end) {
            Ok(SegmentIntersection::Point(_)) => {}
            Ok(SegmentIntersection::None | SegmentIntersection::CollinearOverlap) => {
                return Err(CommandError::NotTJunction);
            }
            Err(GeometryError::NonFinitePoint { .. } | GeometryError::ArithmeticOverflow) => {
                return Err(CommandError::TJunctionGeometryNotRepresentable);
            }
        }

        let mut candidates = Vec::with_capacity(2);
        for (junction_vertex, junction_position) in
            [(first_edge.start, first_start), (first_edge.end, first_end)]
        {
            if point_is_strictly_inside_segment(junction_position, second_start, second_end)
                .map_err(|_| CommandError::TJunctionGeometryNotRepresentable)?
            {
                candidates.push((
                    junction_vertex,
                    junction_position,
                    second_edge_index,
                    second_edge.clone(),
                ));
            }
        }
        for (junction_vertex, junction_position) in [
            (second_edge.start, second_start),
            (second_edge.end, second_end),
        ] {
            if point_is_strictly_inside_segment(junction_position, first_start, first_end)
                .map_err(|_| CommandError::TJunctionGeometryNotRepresentable)?
            {
                candidates.push((
                    junction_vertex,
                    junction_position,
                    first_edge_index,
                    first_edge.clone(),
                ));
            }
        }
        let [candidate] = candidates.as_slice() else {
            return Err(CommandError::NotTJunction);
        };
        let (junction_vertex, junction_position, interior_edge_index, interior_edge) = candidate;
        if let Some(boundary_edge) = [&first_edge, &second_edge]
            .into_iter()
            .find(|edge| edge.kind == EdgeKind::Boundary)
            && interior_edge.kind != EdgeKind::Boundary
        {
            return Err(CommandError::TJunctionBoundaryEdgeMustBeInterior {
                edge: boundary_edge.id,
            });
        }
        if let Some(vertex) =
            self.pattern.vertices.iter().find(|vertex| {
                vertex.id != *junction_vertex && vertex.position == *junction_position
            })
        {
            return Err(CommandError::TJunctionPositionOccupied { vertex: vertex.id });
        }

        // A boundary edge may be the segment whose strict interior is being
        // connected. In that case the existing junction vertex becomes part
        // of the sheet outline, so its exact insertion point in the persisted
        // cyclic boundary must be known before either document is mutated.
        // Boundary edges that merely carry the endpoint are rejected above;
        // phase one only supports the outline-splitting direction.
        let boundary_change = if interior_edge.kind == EdgeKind::Boundary {
            if let Some(edge) = self.pattern.edges.iter().find(|edge| {
                edge.kind == EdgeKind::Boundary
                    && edge.id != first_edge_id
                    && edge.id != second_edge_id
                    && (edge.start == *junction_vertex || edge.end == *junction_vertex)
            }) {
                return Err(CommandError::TJunctionVertexHasOtherBoundaryEdge {
                    vertex: *junction_vertex,
                    edge: edge.id,
                });
            }
            let mut matching_boundary_indices = Vec::new();
            if !self.paper.boundary_vertices.is_empty() {
                for (index, start) in self.paper.boundary_vertices.iter().copied().enumerate() {
                    let end = self.paper.boundary_vertices
                        [(index + 1) % self.paper.boundary_vertices.len()];
                    if undirected_endpoints_match(
                        interior_edge.start,
                        interior_edge.end,
                        start,
                        end,
                    ) {
                        matching_boundary_indices.push(index);
                    }
                }
            }
            let boundary_index = match matching_boundary_indices.as_slice() {
                [] => {
                    return Err(CommandError::BoundaryEdgeNotInPaperBoundary(
                        interior_edge.id,
                    ));
                }
                [index] => *index,
                _ => {
                    return Err(CommandError::BoundaryEdgeMatchesMultiplePaperSegments {
                        edge: interior_edge.id,
                    });
                }
            };
            if self
                .paper
                .boundary_vertices
                .iter()
                .any(|vertex| vertex == junction_vertex)
            {
                return Err(CommandError::TJunctionBoundaryVertexAlreadyPresent {
                    vertex: *junction_vertex,
                });
            }
            self.ensure_length_display_reference_edge_not_mutated(interior_edge.id)?;
            Some((boundary_index, self.paper.boundary_vertices.clone()))
        } else {
            None
        };

        let mut ordered_targets = [
            (first_edge_index, &first_edge),
            (second_edge_index, &second_edge),
        ];
        ordered_targets.sort_by_key(|(index, _)| *index);
        let changed_vertices = [
            ordered_targets[0].1.start,
            ordered_targets[0].1.end,
            ordered_targets[1].1.start,
            ordered_targets[1].1.end,
        ];
        let changed_edges = [
            ordered_targets[0].1.id,
            ordered_targets[1].1.id,
            new_edge_id,
        ];
        let original_edge_index = *interior_edge_index;
        let original_edge = interior_edge.clone();
        let new_edge_index = original_edge_index + 1;
        let new_edge = Edge {
            id: new_edge_id,
            start: *junction_vertex,
            end: original_edge.end,
            kind: original_edge.kind,
        };
        let new_edge_assignment = self.inherited_layer_assignment(original_edge.id, new_edge_id);
        self.ensure_layer_assignment_capacity(usize::from(new_edge_assignment.is_some()))?;

        if let Some((boundary_index, _)) = &boundary_change {
            self.paper
                .boundary_vertices
                .insert(*boundary_index + 1, *junction_vertex);
        }
        self.pattern.edges[original_edge_index].end = *junction_vertex;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());
        if let Some(assignment) = new_edge_assignment {
            self.insert_explicit_layer_assignment(assignment);
        }

        Ok(Inverse::RestoreTJunction {
            original_edge_index,
            original_edge,
            new_edge_index,
            new_edge,
            boundary_vertices: boundary_change.map(|(_, boundary_vertices)| boundary_vertices),
            changed_vertices,
            changed_edges,
            new_edge_assignment,
        })
    }

    fn connect_intersection_cluster(
        &mut self,
        junction: JunctionVertexIntent,
        targets: &[IntersectionEdgeTarget],
    ) -> Result<Inverse, CommandError> {
        let plan = plan_intersection_cluster(&self.pattern, &self.paper, junction, targets)?;
        let junction_id = plan.junction_vertex.id;
        let mut split_edges = Vec::new();
        for target in &plan.targets {
            if let Some(new_edge_id) = target.new_edge_id {
                split_edges.push((
                    target.original_index,
                    target.original_edge.clone(),
                    Edge {
                        id: new_edge_id,
                        start: junction_id,
                        end: target.original_edge.end,
                        kind: target.original_edge.kind,
                    },
                ));
            }
        }
        let original_edges = split_edges
            .iter()
            .map(|(index, edge, _)| (*index, edge.clone()))
            .collect::<Vec<_>>();
        let inserted_edges = split_edges
            .iter()
            .enumerate()
            .map(|(lower_split_count, (original_index, _, edge))| {
                (*original_index + lower_split_count + 1, edge.clone())
            })
            .collect::<Vec<_>>();
        let mut new_edge_assignments = split_edges
            .iter()
            .filter_map(|(_, source, new_edge)| {
                self.inherited_layer_assignment(source.id, new_edge.id)
            })
            .collect::<Vec<_>>();
        new_edge_assignments.sort_unstable_by_key(|assignment| assignment.edge.canonical_bytes());
        self.ensure_layer_assignment_capacity(new_edge_assignments.len())?;
        let created_vertex = if plan.create_vertex {
            Some((self.pattern.vertices.len(), plan.junction_vertex.clone()))
        } else {
            None
        };

        if let Some((_, vertex)) = &created_vertex {
            self.pattern.vertices.push(vertex.clone());
        }
        for (original_index, _, new_edge) in split_edges.iter().rev() {
            self.pattern.edges[*original_index].end = junction_id;
            self.pattern
                .edges
                .insert(*original_index + 1, new_edge.clone());
        }
        for assignment in &new_edge_assignments {
            self.insert_explicit_layer_assignment(*assignment);
        }

        Ok(Inverse::RestoreIntersectionCluster {
            original_boundary_vertices: None,
            original_edges,
            inserted_edges,
            created_vertex,
            junction_vertex: junction_id,
            changed_vertices: plan.changed_vertices,
            changed_edges: plan.changed_edges,
            new_edge_assignments,
        })
    }

    fn split_boundary_edge(
        &mut self,
        edge_id: EdgeId,
        new_vertex_id: VertexId,
        new_edge_id: EdgeId,
        fraction: f64,
    ) -> Result<Inverse, CommandError> {
        if !fraction.is_finite() {
            return Err(CommandError::BoundarySplitFractionNotFinite);
        }
        if fraction <= 0.0 || fraction >= 1.0 {
            return Err(CommandError::BoundarySplitFractionOutOfRange);
        }
        if self.vertex_index(new_vertex_id).is_some()
            || self.paper.boundary_vertices.contains(&new_vertex_id)
            || self
                .pattern
                .edges
                .iter()
                .any(|edge| edge.start == new_vertex_id || edge.end == new_vertex_id)
        {
            return Err(CommandError::VertexAlreadyExists(new_vertex_id));
        }
        if self.edge_index(new_edge_id).is_some() {
            return Err(CommandError::EdgeAlreadyExists(new_edge_id));
        }

        let mut target_edges = self
            .pattern
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.id == edge_id);
        let Some((original_edge_index, original_edge)) = target_edges
            .next()
            .map(|(index, edge)| (index, edge.clone()))
        else {
            return Err(CommandError::EdgeNotFound(edge_id));
        };
        if target_edges.next().is_some() {
            return Err(CommandError::BoundarySplitTargetEdgeIdAmbiguous { edge: edge_id });
        }
        if original_edge.kind != EdgeKind::Boundary {
            return Err(CommandError::EdgeIsNotBoundary(edge_id));
        }

        let mut matching_boundary_indices = Vec::new();
        if !self.paper.boundary_vertices.is_empty() {
            for (index, start) in self.paper.boundary_vertices.iter().copied().enumerate() {
                let end =
                    self.paper.boundary_vertices[(index + 1) % self.paper.boundary_vertices.len()];
                if undirected_endpoints_match(original_edge.start, original_edge.end, start, end) {
                    matching_boundary_indices.push(index);
                }
            }
        }
        let boundary_index = match matching_boundary_indices.as_slice() {
            [] => return Err(CommandError::BoundaryEdgeNotInPaperBoundary(edge_id)),
            [index] => *index,
            _ => {
                return Err(CommandError::BoundaryEdgeMatchesMultiplePaperSegments {
                    edge: edge_id,
                });
            }
        };
        self.ensure_length_display_reference_edge_not_mutated(edge_id)?;

        let start_index = self
            .vertex_index(original_edge.start)
            .ok_or(CommandError::VertexNotFound(original_edge.start))?;
        let end_index = self
            .vertex_index(original_edge.end)
            .ok_or(CommandError::VertexNotFound(original_edge.end))?;
        let start_position = self.pattern.vertices[start_index].position;
        let end_position = self.pattern.vertices[end_index].position;
        if !start_position.x.is_finite() || !start_position.y.is_finite() {
            return Err(CommandError::BoundarySplitEndpointPositionNotFinite {
                edge: edge_id,
                vertex: original_edge.start,
            });
        }
        if !end_position.x.is_finite() || !end_position.y.is_finite() {
            return Err(CommandError::BoundarySplitEndpointPositionNotFinite {
                edge: edge_id,
                vertex: original_edge.end,
            });
        }
        let position = Point2::new(
            stable_convex_combination(start_position.x, end_position.x, fraction),
            stable_convex_combination(start_position.y, end_position.y, fraction),
        );
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(CommandError::BoundarySplitPositionNotFinite);
        }
        if position == start_position || position == end_position {
            return Err(CommandError::BoundarySplitPositionNotDistinct);
        }
        if let Some(vertex) = self
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
        {
            return Err(CommandError::BoundarySplitPositionOccupied { vertex: vertex.id });
        }

        let boundary_vertices = self.paper.boundary_vertices.clone();
        let new_vertex_index = self.pattern.vertices.len();
        let new_edge_index = original_edge_index + 1;
        let new_vertex = Vertex {
            id: new_vertex_id,
            position,
        };
        let new_edge = Edge {
            id: new_edge_id,
            start: new_vertex_id,
            end: original_edge.end,
            kind: EdgeKind::Boundary,
        };
        let new_edge_assignment = self.inherited_layer_assignment(original_edge.id, new_edge_id);
        self.ensure_layer_assignment_capacity(usize::from(new_edge_assignment.is_some()))?;

        self.paper
            .boundary_vertices
            .insert(boundary_index + 1, new_vertex_id);
        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[original_edge_index].end = new_vertex_id;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());
        if let Some(assignment) = new_edge_assignment {
            self.insert_explicit_layer_assignment(assignment);
        }

        Ok(Inverse::RestoreBoundarySplit {
            boundary_vertices,
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
            new_edge_assignment,
        })
    }

    fn resize_rectangular_paper(
        &mut self,
        width_mm: f64,
        height_mm: f64,
    ) -> Result<Inverse, CommandError> {
        let resized_positions = self.planned_rectangular_positions(width_mm, height_mm)?;
        let previous_positions = self
            .pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<Vec<_>>();
        for (vertex, position) in self.pattern.vertices.iter_mut().zip(resized_positions) {
            vertex.position = position;
        }
        Ok(Inverse::RestoreVertexPositions {
            vertices: previous_positions,
        })
    }

    /// Computes the exact coordinates the resize command would commit.
    ///
    /// The constraint mutation guard and the mutating implementation share
    /// this planner so a resize bypasses a lock only when every resulting
    /// coordinate is bit-for-bit unchanged (including signed zero).
    fn planned_rectangular_positions(
        &self,
        width_mm: f64,
        height_mm: f64,
    ) -> Result<Vec<Point2>, CommandError> {
        Self::validate_resize_dimensions(width_mm, height_mm)?;
        let boundary = self.rectangular_boundary()?;
        let same_width = width_mm == boundary.max_x - boundary.min_x;
        let same_height = height_mm == boundary.max_y - boundary.min_y;
        let target_max_x = if same_width {
            boundary.max_x
        } else {
            boundary.min_x + width_mm
        };
        let target_max_y = if same_height {
            boundary.max_y
        } else {
            boundary.min_y + height_mm
        };
        if !target_max_x.is_finite()
            || !target_max_y.is_finite()
            || target_max_x <= boundary.min_x
            || target_max_y <= boundary.min_y
        {
            return Err(CommandError::PaperResizeBoundaryNotRepresentable);
        }

        let current_width = boundary.max_x - boundary.min_x;
        let current_height = boundary.max_y - boundary.min_y;
        let scale_x = width_mm / current_width;
        let scale_y = height_mm / current_height;
        if !scale_x.is_finite() || scale_x <= 0.0 || !scale_y.is_finite() || scale_y <= 0.0 {
            return Err(CommandError::PaperResizeScaleNotRepresentable);
        }

        let mut resized_positions = Vec::with_capacity(self.pattern.vertices.len());
        for vertex in &self.pattern.vertices {
            if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
                return Err(CommandError::PaperResizeVertexPositionNotFinite { vertex: vertex.id });
            }
            let x = if same_width {
                vertex.position.x
            } else {
                boundary.min_x + (vertex.position.x - boundary.min_x) * scale_x
            };
            let y = if same_height {
                vertex.position.y
            } else {
                boundary.min_y + (vertex.position.y - boundary.min_y) * scale_y
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(CommandError::PaperResizeVertexPositionNotFinite { vertex: vertex.id });
            }
            resized_positions.push(Point2::new(x, y));
        }

        // Set the four corners explicitly so floating-point multiplication can
        // never leave a boundary corner just short of its requested target.
        for boundary_id in &self.paper.boundary_vertices {
            let index = self.vertex_index(*boundary_id).ok_or(
                CommandError::RectangularPaperBoundaryVertexNotFound(*boundary_id),
            )?;
            let original = self.pattern.vertices[index].position;
            resized_positions[index] = Point2::new(
                if original.x == boundary.min_x {
                    boundary.min_x
                } else {
                    target_max_x
                },
                if original.y == boundary.min_y {
                    boundary.min_y
                } else {
                    target_max_y
                },
            );
        }

        Ok(resized_positions)
    }

    fn validate_resize_dimensions(width_mm: f64, height_mm: f64) -> Result<(), CommandError> {
        if !width_mm.is_finite() {
            return Err(CommandError::PaperWidthNotFinite);
        }
        if width_mm <= 0.0 {
            return Err(CommandError::PaperWidthNotPositive);
        }
        if !height_mm.is_finite() {
            return Err(CommandError::PaperHeightNotFinite);
        }
        if height_mm <= 0.0 {
            return Err(CommandError::PaperHeightNotPositive);
        }
        let doubled_area = width_mm * height_mm * 2.0;
        if !doubled_area.is_finite() || doubled_area <= 0.0 {
            return Err(CommandError::PaperResizeAreaNotRepresentable);
        }
        Ok(())
    }

    fn rectangular_boundary(&self) -> Result<RectangularBoundary, CommandError> {
        if self.paper.boundary_vertices.len() != 4 {
            return Err(CommandError::RectangularPaperBoundaryVertexCount {
                actual: self.paper.boundary_vertices.len(),
            });
        }

        for (index, vertex) in self.paper.boundary_vertices.iter().enumerate() {
            if self.paper.boundary_vertices[..index].contains(vertex) {
                return Err(CommandError::RectangularPaperBoundaryDuplicateVertex {
                    vertex: *vertex,
                });
            }
        }

        let mut points = [Point2::new(0.0, 0.0); 4];
        for (index, vertex_id) in self.paper.boundary_vertices.iter().enumerate() {
            let vertex_index = self.vertex_index(*vertex_id).ok_or(
                CommandError::RectangularPaperBoundaryVertexNotFound(*vertex_id),
            )?;
            let position = self.pattern.vertices[vertex_index].position;
            if !position.x.is_finite() || !position.y.is_finite() {
                return Err(CommandError::RectangularPaperBoundaryPositionNotFinite {
                    vertex: *vertex_id,
                });
            }
            points[index] = position;
        }

        let min_x = points
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min);
        let min_y = points
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = points
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let width = max_x - min_x;
        let height = max_y - min_y;
        let doubled_area = width * height * 2.0;
        if !width.is_finite()
            || width <= 0.0
            || !height.is_finite()
            || height <= 0.0
            || !doubled_area.is_finite()
            || doubled_area <= 0.0
        {
            return Err(CommandError::RectangularPaperBoundaryAreaNotRepresentable);
        }

        let mut seen_corners = [false; 4];
        let mut corner_indices = [0usize; 4];
        let mut corners_match = true;
        for (index, point) in points.iter().enumerate() {
            let corner = match (
                point.x == min_x,
                point.x == max_x,
                point.y == min_y,
                point.y == max_y,
            ) {
                (true, false, true, false) => Some(0),
                (false, true, true, false) => Some(1),
                (false, true, false, true) => Some(2),
                (true, false, false, true) => Some(3),
                _ => None,
            };
            let Some(corner) = corner else {
                corners_match = false;
                break;
            };
            if seen_corners[corner] {
                corners_match = false;
                break;
            }
            seen_corners[corner] = true;
            corner_indices[index] = corner;
        }

        if !corners_match {
            return if Self::ordered_points_form_rectangle(points) {
                Err(CommandError::PaperBoundaryNotAxisAligned)
            } else {
                Err(CommandError::PaperBoundaryNotRectangle)
            };
        }

        let adjacent_pairs = [
            (corner_indices[0], corner_indices[1]),
            (corner_indices[1], corner_indices[2]),
            (corner_indices[2], corner_indices[3]),
            (corner_indices[3], corner_indices[0]),
        ];
        if adjacent_pairs
            .into_iter()
            .any(|(current, next)| current.abs_diff(next) == 2)
        {
            return Err(CommandError::PaperBoundaryVerticesNotAdjacent);
        }

        Ok(RectangularBoundary {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    fn ordered_points_form_rectangle(points: [Point2; 4]) -> bool {
        let edges = [
            (points[1].x - points[0].x, points[1].y - points[0].y),
            (points[2].x - points[1].x, points[2].y - points[1].y),
            (points[3].x - points[2].x, points[3].y - points[2].y),
            (points[0].x - points[3].x, points[0].y - points[3].y),
        ];
        if edges
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite() || (*x == 0.0 && *y == 0.0))
        {
            return false;
        }
        let dot = edges[0].0 * edges[1].0 + edges[0].1 * edges[1].1;
        dot.is_finite()
            && dot == 0.0
            && edges[0].0 == -edges[2].0
            && edges[0].1 == -edges[2].1
            && edges[1].0 == -edges[3].0
            && edges[1].1 == -edges[3].1
    }

    fn validate_paper_thickness(thickness_mm: f64) -> Result<(), CommandError> {
        if !thickness_mm.is_finite() {
            return Err(CommandError::PaperThicknessNotFinite);
        }
        if thickness_mm < 0.0 {
            return Err(CommandError::PaperThicknessNegative);
        }
        Ok(())
    }

    fn validated_length_display_reference_edge_length(
        &self,
        reference_edge: EdgeId,
    ) -> Result<f64, CommandError> {
        let invalid = || CommandError::LengthDisplayReferenceEdgeInvalid {
            edge: reference_edge,
        };
        let mut matching_edges = self
            .pattern
            .edges
            .iter()
            .filter(|edge| edge.id == reference_edge);
        let edge = matching_edges.next().ok_or_else(invalid)?;
        if matching_edges.next().is_some() || edge.kind != EdgeKind::Boundary {
            return Err(invalid());
        }
        if self
            .pattern
            .edges
            .iter()
            .filter(|candidate| {
                candidate.kind == EdgeKind::Boundary
                    && undirected_endpoints_match(
                        candidate.start,
                        candidate.end,
                        edge.start,
                        edge.end,
                    )
            })
            .count()
            != 1
        {
            return Err(invalid());
        }

        if self.paper.boundary_vertices.len() < 3
            || self
                .paper
                .boundary_vertices
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
                != self.paper.boundary_vertices.len()
        {
            return Err(invalid());
        }
        let mut matching_boundary_segments = 0_usize;
        for (index, start) in self.paper.boundary_vertices.iter().copied().enumerate() {
            let end =
                self.paper.boundary_vertices[(index + 1) % self.paper.boundary_vertices.len()];
            if undirected_endpoints_match(edge.start, edge.end, start, end) {
                matching_boundary_segments += 1;
            }
        }
        if matching_boundary_segments != 1 {
            return Err(invalid());
        }

        let unique_position = |vertex_id: VertexId| {
            let mut matches = self
                .pattern
                .vertices
                .iter()
                .filter(|vertex| vertex.id == vertex_id);
            let vertex = matches.next().ok_or_else(invalid)?;
            if matches.next().is_some()
                || !vertex.position.x.is_finite()
                || !vertex.position.y.is_finite()
            {
                return Err(invalid());
            }
            Ok(vertex.position)
        };
        let start = unique_position(edge.start)?;
        let end = unique_position(edge.end)?;
        let length = (end.x - start.x).hypot(end.y - start.y);
        if !length.is_finite() || length <= 0.0 {
            return Err(invalid());
        }
        Ok(length)
    }

    fn ensure_length_display_reference_edge_not_mutated(
        &self,
        edge: EdgeId,
    ) -> Result<(), CommandError> {
        if matches!(
            self.paper.length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio { reference_edge }
                if reference_edge == edge
        ) {
            Err(CommandError::LengthDisplayReferenceEdgeMutationBlocked { edge })
        } else {
            Ok(())
        }
    }

    fn ensure_length_display_reference_survives_vertex_move(
        &self,
        vertex: VertexId,
        candidate: Point2,
    ) -> Result<(), CommandError> {
        let LengthDisplayUnit::PaperEdgeRatio { reference_edge } = self.paper.length_display_unit
        else {
            return Ok(());
        };
        // An already malformed persisted reference remains editable so the
        // user can repair it or switch back to an absolute unit. Once a valid
        // reference is active, however, an ordinary vertex move must not make
        // the display scale undefined.
        if self
            .validated_length_display_reference_edge_length(reference_edge)
            .is_err()
        {
            return Ok(());
        }
        let edge = self
            .pattern
            .edges
            .iter()
            .find(|edge| edge.id == reference_edge)
            .expect("a validated display reference has one edge record");
        let other = if edge.start == vertex {
            edge.end
        } else if edge.end == vertex {
            edge.start
        } else {
            return Ok(());
        };
        let other_position = self
            .pattern
            .vertices
            .iter()
            .find(|candidate| candidate.id == other)
            .expect("a validated display reference has both endpoint records")
            .position;
        let length = (candidate.x - other_position.x).hypot(candidate.y - other_position.y);
        if !candidate.x.is_finite()
            || !candidate.y.is_finite()
            || !length.is_finite()
            || length <= 0.0
        {
            Err(CommandError::LengthDisplayReferenceEdgeWouldBecomeInvalid {
                edge: reference_edge,
            })
        } else {
            Ok(())
        }
    }

    fn ensure_cutting_can_be_set(&self, allowed: bool) -> Result<(), CommandError> {
        if self.paper.cutting_allowed
            && !allowed
            && let Some(edge) = self
                .pattern
                .edges
                .iter()
                .find(|edge| edge.kind == EdgeKind::Cut)
        {
            return Err(CommandError::CutEdgesPreventDisabling { edge: edge.id });
        }
        Ok(())
    }

    fn apply_inverse(&mut self, inverse: &Inverse) -> Result<(), CommandError> {
        match inverse {
            Inverse::RestoreMirrorSelection {
                pattern,
                project_layers,
            } => {
                self.pattern.clone_from(pattern);
                self.project_layers.clone_from(project_layers);
            }
            Inverse::RestoreStackedFoldDocument {
                pattern,
                paper,
                instruction_timeline,
                project_layers,
                beginner_design_profile,
            } => {
                self.pattern.clone_from(pattern);
                self.paper.clone_from(paper);
                self.instruction_timeline.clone_from(instruction_timeline);
                self.project_layers.clone_from(project_layers);
                self.beginner_design_profile
                    .clone_from(beginner_design_profile.as_ref());
            }
            Inverse::RestoreProjectMemo { memo } => {
                self.project_memo.clone_from(memo);
            }
            Inverse::RestoreBeginnerDesignProfile { profile } => {
                self.beginner_design_profile.clone_from(profile.as_ref());
            }
            Inverse::RestoreElementMetadata { target, metadata } => {
                self.replace_element_metadata(*target, metadata.clone());
            }
            Inverse::Command(command) => {
                self.apply(command)?;
            }
            Inverse::RestoreVertex { index, vertex } => {
                debug_assert!(self.vertex_index(vertex.id).is_none());
                debug_assert!(*index <= self.pattern.vertices.len());
                self.pattern.vertices.insert(*index, vertex.clone());
            }
            Inverse::RestoreEdge {
                index,
                edge,
                layer_assignment,
            } => {
                debug_assert!(self.edge_index(edge.id).is_none());
                debug_assert!(*index <= self.pattern.edges.len());
                self.pattern.edges.insert(*index, edge.clone());
                if let Some((assignment_index, assignment)) = layer_assignment {
                    debug_assert!(*assignment_index <= self.project_layers.edge_assignments.len());
                    self.project_layers
                        .edge_assignments
                        .insert(*assignment_index, *assignment);
                }
            }
            Inverse::RestorePaperProperties {
                thickness_mm,
                front_color,
                back_color,
                front_texture_asset,
                back_texture_asset,
                cutting_allowed,
            } => {
                self.paper.thickness_mm = *thickness_mm;
                self.paper.front.color = *front_color;
                self.paper.back.color = *back_color;
                self.paper.front.texture_asset = *front_texture_asset;
                self.paper.back.texture_asset = *back_texture_asset;
                self.paper.cutting_allowed = *cutting_allowed;
            }
            Inverse::RestoreLengthDisplayUnit { unit } => {
                self.paper.length_display_unit = *unit;
            }
            Inverse::RestoreVertexPositions { vertices } => {
                debug_assert_eq!(vertices.len(), self.pattern.vertices.len());
                for (vertex, (expected_id, position)) in
                    self.pattern.vertices.iter_mut().zip(vertices)
                {
                    debug_assert_eq!(vertex.id, *expected_id);
                    vertex.position = *position;
                }
            }
            Inverse::RestoreBoundarySplit {
                boundary_vertices,
                original_edge_index,
                original_edge,
                new_vertex_index,
                new_vertex,
                new_edge_index,
                new_edge,
                new_edge_assignment,
            } => {
                self.remove_expected_layer_assignment(*new_edge_assignment)?;
                debug_assert_eq!(
                    self.pattern.edges.get(*new_edge_index).map(|edge| edge.id),
                    Some(new_edge.id)
                );
                self.pattern.edges.remove(*new_edge_index);
                debug_assert_eq!(
                    self.pattern
                        .edges
                        .get(*original_edge_index)
                        .map(|edge| edge.id),
                    Some(original_edge.id)
                );
                self.pattern.edges[*original_edge_index] = original_edge.clone();
                debug_assert_eq!(
                    self.pattern
                        .vertices
                        .get(*new_vertex_index)
                        .map(|vertex| vertex.id),
                    Some(new_vertex.id)
                );
                self.pattern.vertices.remove(*new_vertex_index);
                self.paper.boundary_vertices = boundary_vertices.clone();
            }
            Inverse::RestoreEdgeSplit {
                original_edge_index,
                original_edge,
                new_vertex_index,
                new_vertex,
                new_edge_index,
                new_edge,
                new_edge_assignment,
            } => {
                self.remove_expected_layer_assignment(*new_edge_assignment)?;
                debug_assert_eq!(
                    self.pattern.edges.get(*new_edge_index).map(|edge| edge.id),
                    Some(new_edge.id)
                );
                self.pattern.edges.remove(*new_edge_index);
                debug_assert_eq!(
                    self.pattern
                        .edges
                        .get(*original_edge_index)
                        .map(|edge| edge.id),
                    Some(original_edge.id)
                );
                self.pattern.edges[*original_edge_index] = original_edge.clone();
                debug_assert_eq!(
                    self.pattern
                        .vertices
                        .get(*new_vertex_index)
                        .map(|vertex| vertex.id),
                    Some(new_vertex.id)
                );
                self.pattern.vertices.remove(*new_vertex_index);
            }
            Inverse::RestoreEdgeIntersection {
                original_edges,
                new_edges,
                new_vertex_index,
                new_vertex,
                new_edge_assignments,
            } => {
                for assignment in new_edge_assignments {
                    self.remove_expected_layer_assignment(Some(*assignment))?;
                }
                for (new_edge_index, new_edge) in new_edges.iter().rev() {
                    debug_assert_eq!(
                        self.pattern.edges.get(*new_edge_index).map(|edge| edge.id),
                        Some(new_edge.id)
                    );
                    self.pattern.edges.remove(*new_edge_index);
                }
                for (original_edge_index, original_edge) in original_edges {
                    debug_assert_eq!(
                        self.pattern
                            .edges
                            .get(*original_edge_index)
                            .map(|edge| edge.id),
                        Some(original_edge.id)
                    );
                    self.pattern.edges[*original_edge_index] = original_edge.clone();
                }
                debug_assert_eq!(
                    self.pattern
                        .vertices
                        .get(*new_vertex_index)
                        .map(|vertex| vertex.id),
                    Some(new_vertex.id)
                );
                self.pattern.vertices.remove(*new_vertex_index);
            }
            Inverse::RestoreTJunction {
                original_edge_index,
                original_edge,
                new_edge_index,
                new_edge,
                boundary_vertices,
                new_edge_assignment,
                ..
            } => {
                self.remove_expected_layer_assignment(*new_edge_assignment)?;
                debug_assert_eq!(
                    self.pattern.edges.get(*new_edge_index).map(|edge| edge.id),
                    Some(new_edge.id)
                );
                self.pattern.edges.remove(*new_edge_index);
                debug_assert_eq!(
                    self.pattern
                        .edges
                        .get(*original_edge_index)
                        .map(|edge| edge.id),
                    Some(original_edge.id)
                );
                self.pattern.edges[*original_edge_index] = original_edge.clone();
                if let Some(boundary_vertices) = boundary_vertices {
                    self.paper.boundary_vertices = boundary_vertices.clone();
                }
            }
            Inverse::RestoreIntersectionCluster {
                original_boundary_vertices,
                original_edges,
                inserted_edges,
                created_vertex,
                junction_vertex,
                new_edge_assignments,
                ..
            } => {
                for assignment in new_edge_assignments {
                    self.remove_expected_layer_assignment(Some(*assignment))?;
                }
                debug_assert!(
                    self.pattern
                        .vertices
                        .iter()
                        .any(|vertex| vertex.id == *junction_vertex)
                );
                for (inserted_index, inserted_edge) in inserted_edges.iter().rev() {
                    debug_assert_eq!(
                        self.pattern.edges.get(*inserted_index).map(|edge| edge.id),
                        Some(inserted_edge.id)
                    );
                    self.pattern.edges.remove(*inserted_index);
                }
                for (original_index, original_edge) in original_edges {
                    debug_assert_eq!(
                        self.pattern.edges.get(*original_index).map(|edge| edge.id),
                        Some(original_edge.id)
                    );
                    self.pattern.edges[*original_index] = original_edge.clone();
                }
                if let Some((vertex_index, vertex)) = created_vertex {
                    debug_assert_eq!(
                        self.pattern
                            .vertices
                            .get(*vertex_index)
                            .map(|candidate| candidate.id),
                        Some(vertex.id)
                    );
                    self.pattern.vertices.remove(*vertex_index);
                }
                if let Some(boundary_vertices) = original_boundary_vertices {
                    self.paper.boundary_vertices = boundary_vertices.clone();
                }
            }
            Inverse::RestoreBoundaryVertexRemoval {
                boundary_index,
                vertex_index,
                vertex,
                kept_edge_index,
                kept_edge,
                removed_edge_index,
                removed_edge,
                removed_edge_assignment,
                ..
            } => {
                let current_kept_index = if removed_edge_index < kept_edge_index {
                    *kept_edge_index - 1
                } else {
                    *kept_edge_index
                };
                debug_assert_eq!(
                    self.pattern
                        .edges
                        .get(current_kept_index)
                        .map(|edge| edge.id),
                    Some(kept_edge.id)
                );
                self.pattern
                    .edges
                    .insert(*removed_edge_index, removed_edge.clone());
                self.pattern.edges[*kept_edge_index] = kept_edge.clone();
                self.pattern.vertices.insert(*vertex_index, vertex.clone());
                self.paper
                    .boundary_vertices
                    .insert(*boundary_index, vertex.id);
                if let Some((assignment_index, assignment)) = removed_edge_assignment {
                    debug_assert!(*assignment_index <= self.project_layers.edge_assignments.len());
                    self.project_layers
                        .edge_assignments
                        .insert(*assignment_index, *assignment);
                }
            }
            Inverse::RemoveAddedGeometricConstraint { id } => {
                let index = self
                    .geometric_constraints
                    .constraints
                    .iter()
                    .position(|record| record.id == *id)
                    .ok_or(CommandError::GeometricConstraintNotFound(*id))?;
                // This inverse is a trusted delta created only after strict
                // Add admission. Removing that exact record must also work if
                // the surrounding loaded document is repairably invalid.
                self.geometric_constraints.constraints.remove(index);
            }
            Inverse::RestoreRemovedGeometricConstraint { index, record } => {
                if *index > self.geometric_constraints.constraints.len() {
                    return Err(CommandError::GeometricConstraintNotFound(record.id));
                }
                // Undo restores the exact raw record and index captured by
                // Remove. Revalidating here would make an invalid-but-loaded
                // document impossible to repair and impossible to restore
                // byte-for-byte through trusted history.
                self.geometric_constraints
                    .constraints
                    .insert(*index, record.clone());
            }
            Inverse::RemoveAddedInstructionStep { step_id } => {
                let mut candidate = self.instruction_timeline.clone();
                let index = candidate
                    .steps
                    .iter()
                    .position(|step| step.id == *step_id)
                    .ok_or(CommandError::InstructionStepNotFound(*step_id))?;
                candidate.steps.remove(index);
                self.commit_instruction_timeline(candidate)?;
            }
            Inverse::RemoveAppendedInstructionSteps { step_ids } => {
                if step_ids.is_empty() || step_ids.len() > self.instruction_timeline.steps.len() {
                    return Err(CommandError::InstructionStepAppendHistoryMismatch);
                }
                let suffix_start = self.instruction_timeline.steps.len() - step_ids.len();
                if self.instruction_timeline.steps[suffix_start..]
                    .iter()
                    .map(|step| step.id)
                    .ne(step_ids.iter().copied())
                {
                    return Err(CommandError::InstructionStepAppendHistoryMismatch);
                }
                let mut candidate = self.instruction_timeline.clone();
                candidate.steps.truncate(suffix_start);
                self.commit_instruction_timeline(candidate)?;
            }
            Inverse::RestoreInstructionStepMetadata {
                step_id,
                title,
                description,
                caution,
                duration_ms,
                visual,
            } => {
                let mut candidate = self.instruction_timeline.clone();
                let step = candidate
                    .steps
                    .iter_mut()
                    .find(|step| step.id == *step_id)
                    .ok_or(CommandError::InstructionStepNotFound(*step_id))?;
                step.title.clone_from(title);
                step.description.clone_from(description);
                step.caution.clone_from(caution);
                step.duration_ms = *duration_ms;
                step.visual.clone_from(visual);
                self.commit_instruction_timeline(candidate)?;
            }
            Inverse::RestoreInstructionStepPose { step_id, pose } => {
                let mut candidate = self.instruction_timeline.clone();
                let step = candidate
                    .steps
                    .iter_mut()
                    .find(|step| step.id == *step_id)
                    .ok_or(CommandError::InstructionStepNotFound(*step_id))?;
                step.pose.clone_from(pose);
                self.commit_instruction_timeline(candidate)?;
            }
            Inverse::RestoreRemovedInstructionStep { index, step } => {
                let mut candidate = self.instruction_timeline.clone();
                if candidate
                    .steps
                    .iter()
                    .any(|candidate| candidate.id == step.id)
                {
                    return Err(CommandError::InstructionStepAlreadyExists(step.id));
                }
                if *index > candidate.steps.len() {
                    return Err(CommandError::InstructionStepTargetIndexOutOfBounds {
                        target_index: *index,
                        step_count: candidate.steps.len().saturating_add(1),
                    });
                }
                candidate.steps.insert(*index, step.clone());
                self.commit_instruction_timeline(candidate)?;
            }
            Inverse::RestoreInstructionStepOrder {
                step_id,
                previous_index,
            } => {
                let mut candidate = self.instruction_timeline.clone();
                let current_index = candidate
                    .steps
                    .iter()
                    .position(|step| step.id == *step_id)
                    .ok_or(CommandError::InstructionStepNotFound(*step_id))?;
                if *previous_index >= candidate.steps.len() {
                    return Err(CommandError::InstructionStepTargetIndexOutOfBounds {
                        target_index: *previous_index,
                        step_count: candidate.steps.len(),
                    });
                }
                let step = candidate.steps.remove(current_index);
                candidate.steps.insert(*previous_index, step);
                self.commit_instruction_timeline(candidate)?;
            }
            Inverse::RestoreDeletedLayer {
                index,
                layer,
                assignments,
            } => {
                let mut candidate = self.project_layers.clone();
                if *index > candidate.layers.len() {
                    return Err(CommandError::LayerTargetIndexOutOfBounds {
                        target_index: *index,
                        layer_count: candidate.layers.len().saturating_add(1),
                    });
                }
                if candidate.layers.iter().any(|record| record.id == layer.id) {
                    return Err(CommandError::LayerAlreadyExists(layer.id));
                }
                candidate.layers.insert(*index, layer.clone());
                for (assignment_index, assignment) in assignments {
                    if *assignment_index > candidate.edge_assignments.len() {
                        return Err(CommandError::LayerHistoryAssignmentMismatch {
                            edge: assignment.edge,
                        });
                    }
                    candidate
                        .edge_assignments
                        .insert(*assignment_index, *assignment);
                }
                self.commit_project_layers(candidate)?;
            }
        }
        Ok(())
    }

    fn vertex_index(&self, id: VertexId) -> Option<usize> {
        self.pattern
            .vertices
            .iter()
            .position(|vertex| vertex.id == id)
    }

    fn edge_index(&self, id: EdgeId) -> Option<usize> {
        self.pattern.edges.iter().position(|edge| edge.id == id)
    }

    fn replace_element_metadata(
        &mut self,
        target: ElementMetadataTargetV1,
        metadata: Option<ElementMetadataV1>,
    ) -> Option<ElementMetadataV1> {
        match target {
            ElementMetadataTargetV1::Vertex(vertex) => replace_metadata_record(
                &mut self.element_metadata.vertices,
                |record| record.vertex == vertex,
                |record| &record.metadata,
                metadata,
                |metadata| ori_domain::VertexMetadataRecordV1 { vertex, metadata },
            ),
            ElementMetadataTargetV1::Edge(edge) => replace_metadata_record(
                &mut self.element_metadata.edges,
                |record| record.edge == edge,
                |record| &record.metadata,
                metadata,
                |metadata| ori_domain::EdgeMetadataRecordV1 { edge, metadata },
            ),
            ElementMetadataTargetV1::Face(face) => replace_metadata_record(
                &mut self.element_metadata.faces,
                |record| record.face == face,
                |record| &record.metadata,
                metadata,
                |metadata| ori_domain::FaceMetadataRecordV1 { face, metadata },
            ),
        }
    }

    const fn ensure_revision(&self, expected: Revision) -> Result<(), CommandError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(CommandError::RevisionConflict {
                expected,
                actual: self.revision,
            })
        }
    }

    const fn next_revision(&self) -> Result<Revision, CommandError> {
        if self.revision >= MAX_REVISION {
            Err(CommandError::RevisionExhausted {
                revision: self.revision,
            })
        } else {
            Ok(self.revision + 1)
        }
    }

    fn result(&self, changes: Changes) -> CommandResult {
        CommandResult {
            revision: self.revision,
            changed_vertices: changes.vertices,
            changed_edges: changes.edges,
            settings_changed: changes.settings,
            instructions_changed: changes.instructions,
            constraints_changed: changes.constraints,
        }
    }
}

fn replace_metadata_record<R>(
    records: &mut Vec<R>,
    matches: impl Fn(&R) -> bool,
    read: impl Fn(&R) -> &ElementMetadataV1,
    metadata: Option<ElementMetadataV1>,
    create: impl FnOnce(ElementMetadataV1) -> R,
) -> Option<ElementMetadataV1> {
    let index = records.iter().position(matches);
    let previous = index.map(|index| read(&records[index]).clone());
    match (index, metadata) {
        (Some(index), Some(metadata)) => records[index] = create(metadata),
        (Some(index), None) => {
            records.remove(index);
        }
        (None, Some(metadata)) => records.push(create(metadata)),
        (None, None) => {}
    }
    previous
}

#[derive(Default)]
struct Changes {
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
    settings: bool,
    instructions: bool,
    constraints: bool,
}

impl Command {
    fn geometric_resource_growth(&self) -> Result<Option<(usize, usize)>, CommandError> {
        let growth = match self {
            Self::AddVertex { .. } => (1, 0),
            Self::AddConnectedVertex { .. } => (1, 1),
            Self::AddEdge { .. } | Self::ConnectTJunction { .. } => (0, 1),
            Self::SplitEdge { .. } | Self::SplitBoundaryEdge { .. } => (1, 1),
            Self::ConnectEdgeIntersection { .. } => (1, 2),
            Self::ConnectIntersectionCluster { junction, targets } => {
                let added_edges = targets.iter().try_fold(0usize, |count, target| {
                    if target.new_edge.is_some() {
                        count.checked_add(1).ok_or(
                            CommandError::GeometricConstraintGeometryCountOverflow {
                                resource: GeometricConstraintResourceV1::Edges,
                            },
                        )
                    } else {
                        Ok(count)
                    }
                })?;
                (
                    usize::from(matches!(junction, JunctionVertexIntent::Create { .. })),
                    added_edges,
                )
            }
            Self::UpdateProjectMemo { .. }
            | Self::UpdateBeginnerDesignProfile { .. }
            | Self::SetElementMetadata { .. }
            | Self::MoveVertex { .. }
            | Self::MoveEdge { .. }
            | Self::MoveVertices { .. }
            | Self::RemoveVertex { .. }
            | Self::RemoveConnectedVertex { .. }
            | Self::RemoveEdge { .. }
            | Self::SetCuttingAllowed { .. }
            | Self::UpdatePaperProperties { .. }
            | Self::SetLengthDisplayUnit { .. }
            | Self::ResizeRectangularPaper { .. }
            | Self::RemoveBoundaryVertex { .. }
            | Self::AddGeometricConstraint { .. }
            | Self::RemoveGeometricConstraint { .. }
            | Self::AddAnnotation { .. }
            | Self::UpdateAnnotation { .. }
            | Self::RemoveAnnotation { .. }
            | Self::AddUnderlay { .. }
            | Self::UpdateUnderlay { .. }
            | Self::RemoveUnderlay { .. }
            | Self::AddInstructionStep { .. }
            | Self::AppendInstructionSteps { .. }
            | Self::UpdateInstructionStepMetadata { .. }
            | Self::ReplaceInstructionStepPose { .. }
            | Self::RemoveInstructionStep { .. }
            | Self::MoveInstructionStep { .. }
            | Self::RewriteInstructionTimelineSplitMerge { .. }
            | Self::CreateLayer { .. }
            | Self::RenameLayer { .. }
            | Self::UpdateLayerPresentation { .. }
            | Self::MoveLayer { .. }
            | Self::DeleteLayer { .. }
            | Self::AssignEdgeToLayer { .. } => return Ok(None),
            Self::MirrorSelection {
                new_vertices,
                new_edges,
                ..
            } => (new_vertices.len(), new_edges.len()),
            Self::ApplyNormalizedEdgeDocument { pattern, .. }
            | Self::ApplyStackedFoldDocument { pattern, .. } => {
                let added_vertices = pattern.vertices.len().saturating_sub(0);
                let added_edges = pattern.edges.len().saturating_sub(0);
                (added_vertices, added_edges)
            }
        };
        Ok(Some(growth))
    }

    /// Returns whether this command can change the canonical material
    /// kinematics geometry.
    ///
    /// Paper thickness, appearance, and cutting permission are deliberately
    /// excluded: they invalidate stronger native certificates through their
    /// revision/binding, but do not change the central-surface semantic pose.
    const fn may_change_kinematic_geometry(&self) -> bool {
        match self {
            Self::AddVertex { .. }
            | Self::MoveVertex { .. }
            | Self::MoveEdge { .. }
            | Self::MoveVertices { .. }
            | Self::RemoveVertex { .. }
            | Self::AddEdge { .. }
            | Self::AddConnectedVertex { .. }
            | Self::RemoveConnectedVertex { .. }
            | Self::RemoveEdge { .. }
            | Self::ResizeRectangularPaper { .. }
            | Self::SplitEdge { .. }
            | Self::ConnectEdgeIntersection { .. }
            | Self::ConnectTJunction { .. }
            | Self::ConnectIntersectionCluster { .. }
            | Self::SplitBoundaryEdge { .. }
            | Self::RemoveBoundaryVertex { .. }
            | Self::MirrorSelection { .. }
            | Self::ApplyNormalizedEdgeDocument { .. }
            | Self::ApplyStackedFoldDocument { .. } => true,
            Self::UpdateProjectMemo { .. }
            | Self::UpdateBeginnerDesignProfile { .. }
            | Self::SetElementMetadata { .. }
            | Self::SetCuttingAllowed { .. }
            | Self::UpdatePaperProperties { .. }
            | Self::SetLengthDisplayUnit { .. }
            | Self::AddGeometricConstraint { .. }
            | Self::RemoveGeometricConstraint { .. }
            | Self::AddAnnotation { .. }
            | Self::UpdateAnnotation { .. }
            | Self::RemoveAnnotation { .. }
            | Self::AddUnderlay { .. }
            | Self::UpdateUnderlay { .. }
            | Self::RemoveUnderlay { .. }
            | Self::AddInstructionStep { .. }
            | Self::AppendInstructionSteps { .. }
            | Self::UpdateInstructionStepMetadata { .. }
            | Self::ReplaceInstructionStepPose { .. }
            | Self::RemoveInstructionStep { .. }
            | Self::MoveInstructionStep { .. }
            | Self::RewriteInstructionTimelineSplitMerge { .. }
            | Self::CreateLayer { .. }
            | Self::RenameLayer { .. }
            | Self::UpdateLayerPresentation { .. }
            | Self::MoveLayer { .. }
            | Self::DeleteLayer { .. }
            | Self::AssignEdgeToLayer { .. } => false,
        }
    }

    fn changes(&self, pattern: &CreasePattern, paper: &Paper) -> Changes {
        match *self {
            Self::UpdateProjectMemo { .. } | Self::UpdateBeginnerDesignProfile { .. } => Changes {
                settings: true,
                ..Changes::default()
            },
            Self::SetElementMetadata { target, .. } => Changes {
                vertices: match target {
                    ElementMetadataTargetV1::Vertex(id) => vec![id],
                    _ => Vec::new(),
                },
                edges: match target {
                    ElementMetadataTargetV1::Edge(id) => vec![id],
                    _ => Vec::new(),
                },
                settings: true,
                ..Changes::default()
            },
            Self::AddVertex { id, .. }
            | Self::MoveVertex { id, .. }
            | Self::RemoveVertex { id } => Changes {
                vertices: vec![id],
                edges: Vec::new(),
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::MoveEdge { id, .. } => {
                let vertices = pattern
                    .edges
                    .iter()
                    .find(|edge| edge.id == id)
                    .map(|edge| vec![edge.start, edge.end])
                    .unwrap_or_default();
                Changes {
                    vertices,
                    edges: vec![id],
                    settings: false,
                    instructions: false,
                    constraints: false,
                }
            }
            Self::MoveVertices { ref updates } => Changes {
                vertices: updates.iter().map(|update| update.vertex).collect(),
                edges: pattern
                    .edges
                    .iter()
                    .filter(|edge| {
                        updates
                            .iter()
                            .any(|update| edge.start == update.vertex || edge.end == update.vertex)
                    })
                    .map(|edge| edge.id)
                    .collect(),
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::AddEdge { id, start, end, .. } => Changes {
                vertices: vec![start, end],
                edges: vec![id],
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::AddConnectedVertex {
                vertex_id,
                edge_id,
                start,
                ..
            } => Changes {
                vertices: vec![start, vertex_id],
                edges: vec![edge_id],
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::RemoveConnectedVertex { vertex_id, edge_id } => Changes {
                vertices: vec![vertex_id],
                edges: vec![edge_id],
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::RemoveEdge { id } => Changes {
                vertices: Vec::new(),
                edges: vec![id],
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::SetCuttingAllowed { .. }
            | Self::UpdatePaperProperties { .. }
            | Self::SetLengthDisplayUnit { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::ResizeRectangularPaper { .. } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: Vec::new(),
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::SplitEdge {
                edge,
                new_vertex,
                new_edge,
                ..
            } => {
                let mut vertices = vec![new_vertex];
                if let Some(original_edge) =
                    pattern.edges.iter().find(|candidate| candidate.id == edge)
                {
                    vertices.push(original_edge.start);
                    vertices.push(original_edge.end);
                }
                Changes {
                    vertices,
                    edges: vec![edge, new_edge],
                    settings: false,
                    instructions: false,
                    constraints: false,
                }
            }
            Self::ConnectEdgeIntersection {
                first_edge,
                second_edge,
                new_vertex,
                first_new_edge,
                second_new_edge,
            } => {
                let mut changed = [
                    pattern
                        .edges
                        .iter()
                        .enumerate()
                        .find(|(_, edge)| edge.id == first_edge)
                        .map(|(index, edge)| (index, edge, first_new_edge)),
                    pattern
                        .edges
                        .iter()
                        .enumerate()
                        .find(|(_, edge)| edge.id == second_edge)
                        .map(|(index, edge)| (index, edge, second_new_edge)),
                ];
                changed.sort_by_key(|entry| entry.map_or(usize::MAX, |(index, _, _)| index));
                let mut vertices = vec![new_vertex];
                let mut edges = Vec::with_capacity(4);
                for entry in changed.into_iter().flatten() {
                    let (_, edge, generated_edge) = entry;
                    vertices.push(edge.start);
                    vertices.push(edge.end);
                    edges.push(edge.id);
                    edges.push(generated_edge);
                }
                Changes {
                    vertices,
                    edges,
                    settings: false,
                    instructions: false,
                    constraints: false,
                }
            }
            Self::ConnectTJunction {
                first_edge,
                second_edge,
                new_edge,
            } => {
                let mut targets = [
                    pattern
                        .edges
                        .iter()
                        .enumerate()
                        .find(|(_, edge)| edge.id == first_edge),
                    pattern
                        .edges
                        .iter()
                        .enumerate()
                        .find(|(_, edge)| edge.id == second_edge),
                ];
                targets.sort_by_key(|entry| entry.map_or(usize::MAX, |(index, _)| index));
                let mut vertices = Vec::with_capacity(4);
                let mut edges = Vec::with_capacity(3);
                for (_, edge) in targets.into_iter().flatten() {
                    vertices.push(edge.start);
                    vertices.push(edge.end);
                    edges.push(edge.id);
                }
                edges.push(new_edge);
                Changes {
                    vertices,
                    edges,
                    settings: targets
                        .into_iter()
                        .flatten()
                        .any(|(_, edge)| edge.kind == EdgeKind::Boundary),
                    instructions: false,
                    constraints: false,
                }
            }
            Self::ConnectIntersectionCluster {
                junction,
                ref targets,
            } => intersection_cluster_changes(pattern, junction, targets),
            Self::SplitBoundaryEdge {
                edge,
                new_vertex,
                new_edge,
                ..
            } => {
                let mut vertices = vec![new_vertex];
                if let Some(original_edge) =
                    pattern.edges.iter().find(|candidate| candidate.id == edge)
                {
                    vertices.push(original_edge.start);
                    vertices.push(original_edge.end);
                }
                Changes {
                    vertices,
                    edges: vec![edge, new_edge],
                    settings: true,
                    instructions: false,
                    constraints: false,
                }
            }
            Self::RemoveBoundaryVertex { vertex } => {
                let mut vertices = vec![vertex];
                let mut edges = Vec::new();
                if let Some(boundary_index) = paper
                    .boundary_vertices
                    .iter()
                    .position(|candidate| *candidate == vertex)
                {
                    let boundary_len = paper.boundary_vertices.len();
                    let previous =
                        paper.boundary_vertices[(boundary_index + boundary_len - 1) % boundary_len];
                    let next = paper.boundary_vertices[(boundary_index + 1) % boundary_len];
                    vertices.push(previous);
                    vertices.push(next);
                    if let Some(edge) = pattern.edges.iter().find(|edge| {
                        edge.kind == EdgeKind::Boundary
                            && undirected_endpoints_match(edge.start, edge.end, previous, vertex)
                    }) {
                        edges.push(edge.id);
                    }
                    if let Some(edge) = pattern.edges.iter().find(|edge| {
                        edge.kind == EdgeKind::Boundary
                            && undirected_endpoints_match(edge.start, edge.end, vertex, next)
                    }) {
                        edges.push(edge.id);
                    }
                }
                Changes {
                    vertices,
                    edges,
                    settings: true,
                    instructions: false,
                    constraints: false,
                }
            }
            Self::AddGeometricConstraint { .. } | Self::RemoveGeometricConstraint { .. } => {
                Changes {
                    constraints: true,
                    ..Changes::default()
                }
            }
            Self::AddAnnotation { .. }
            | Self::UpdateAnnotation { .. }
            | Self::RemoveAnnotation { .. } => Changes {
                settings: true,
                ..Changes::default()
            },
            Self::AddUnderlay { .. }
            | Self::UpdateUnderlay { .. }
            | Self::RemoveUnderlay { .. } => Changes {
                settings: true,
                ..Changes::default()
            },
            Self::AddInstructionStep { .. }
            | Self::AppendInstructionSteps { .. }
            | Self::UpdateInstructionStepMetadata { .. }
            | Self::ReplaceInstructionStepPose { .. }
            | Self::RemoveInstructionStep { .. }
            | Self::MoveInstructionStep { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: false,
                instructions: true,
                constraints: false,
            },
            Self::RewriteInstructionTimelineSplitMerge { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: false,
                instructions: true,
                constraints: false,
            },
            Self::CreateLayer { .. }
            | Self::RenameLayer { .. }
            | Self::UpdateLayerPresentation { .. }
            | Self::MoveLayer { .. }
            | Self::DeleteLayer { .. } => Changes {
                settings: true,
                ..Changes::default()
            },
            Self::AssignEdgeToLayer { edge, .. } => Changes {
                edges: vec![edge],
                settings: true,
                ..Changes::default()
            },
            Self::MirrorSelection {
                ref vertices,
                ref edges,
                ref new_vertices,
                ref new_edges,
                ..
            } => Changes {
                vertices: vertices.iter().chain(new_vertices).copied().collect(),
                edges: edges.iter().chain(new_edges).copied().collect(),
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::ApplyNormalizedEdgeDocument { ref pattern, .. } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: pattern.edges.iter().map(|edge| edge.id).collect(),
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::ApplyStackedFoldDocument { ref pattern, .. } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: pattern.edges.iter().map(|edge| edge.id).collect(),
                settings: true,
                instructions: true,
                constraints: false,
            },
        }
    }
}

impl Inverse {
    fn changes(&self, pattern: &CreasePattern, paper: &Paper) -> Changes {
        match self {
            Self::RestoreMirrorSelection { pattern, .. } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: pattern.edges.iter().map(|edge| edge.id).collect(),
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::RestoreStackedFoldDocument {
                pattern,
                paper: _,
                instruction_timeline: _,
                project_layers: _,
                beginner_design_profile: _,
            } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: pattern.edges.iter().map(|edge| edge.id).collect(),
                settings: true,
                instructions: true,
                constraints: false,
            },
            Self::RestoreProjectMemo { .. } | Self::RestoreBeginnerDesignProfile { .. } => {
                Changes {
                    settings: true,
                    ..Changes::default()
                }
            }
            Self::RestoreElementMetadata { target, .. } => Changes {
                vertices: match target {
                    ElementMetadataTargetV1::Vertex(id) => vec![*id],
                    _ => Vec::new(),
                },
                edges: match target {
                    ElementMetadataTargetV1::Edge(id) => vec![*id],
                    _ => Vec::new(),
                },
                settings: true,
                ..Changes::default()
            },
            Self::Command(command) => command.changes(pattern, paper),
            Self::RestoreVertex { vertex, .. } => Changes {
                vertices: vec![vertex.id],
                edges: Vec::new(),
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::RestoreEdge { edge, .. } => Changes {
                vertices: vec![edge.start, edge.end],
                edges: vec![edge.id],
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::RestorePaperProperties { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::RestoreLengthDisplayUnit { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::RestoreVertexPositions { vertices } => Changes {
                vertices: vertices.iter().map(|(id, _)| *id).collect(),
                edges: Vec::new(),
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::RestoreBoundarySplit {
                original_edge,
                new_vertex,
                new_edge,
                ..
            } => Changes {
                vertices: vec![new_vertex.id, original_edge.start, original_edge.end],
                edges: vec![original_edge.id, new_edge.id],
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::RestoreEdgeSplit {
                original_edge,
                new_vertex,
                new_edge,
                ..
            } => Changes {
                vertices: vec![new_vertex.id, original_edge.start, original_edge.end],
                edges: vec![original_edge.id, new_edge.id],
                settings: false,
                instructions: false,
                constraints: false,
            },
            Self::RestoreEdgeIntersection {
                original_edges,
                new_edges,
                new_vertex,
                ..
            } => {
                let mut vertices = vec![new_vertex.id];
                let mut edges = Vec::with_capacity(4);
                for ((_, original_edge), (_, new_edge)) in
                    original_edges.iter().zip(new_edges.iter())
                {
                    vertices.push(original_edge.start);
                    vertices.push(original_edge.end);
                    edges.push(original_edge.id);
                    edges.push(new_edge.id);
                }
                Changes {
                    vertices,
                    edges,
                    settings: false,
                    instructions: false,
                    constraints: false,
                }
            }
            Self::RestoreTJunction {
                boundary_vertices,
                changed_vertices,
                changed_edges,
                ..
            } => Changes {
                vertices: changed_vertices.to_vec(),
                edges: changed_edges.to_vec(),
                settings: boundary_vertices.is_some(),
                instructions: false,
                constraints: false,
            },
            Self::RestoreIntersectionCluster {
                original_boundary_vertices,
                changed_vertices,
                changed_edges,
                ..
            } => Changes {
                vertices: changed_vertices.clone(),
                edges: changed_edges.clone(),
                settings: original_boundary_vertices.is_some(),
                instructions: false,
                constraints: false,
            },
            Self::RestoreBoundaryVertexRemoval {
                vertex,
                kept_edge,
                removed_edge,
                previous_vertex,
                next_vertex,
                ..
            } => Changes {
                vertices: vec![vertex.id, *previous_vertex, *next_vertex],
                edges: vec![kept_edge.id, removed_edge.id],
                settings: true,
                instructions: false,
                constraints: false,
            },
            Self::RemoveAddedGeometricConstraint { .. }
            | Self::RestoreRemovedGeometricConstraint { .. } => Changes {
                constraints: true,
                ..Changes::default()
            },
            Self::RemoveAddedInstructionStep { .. }
            | Self::RemoveAppendedInstructionSteps { .. }
            | Self::RestoreInstructionStepMetadata { .. }
            | Self::RestoreInstructionStepPose { .. }
            | Self::RestoreRemovedInstructionStep { .. }
            | Self::RestoreInstructionStepOrder { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: false,
                instructions: true,
                constraints: false,
            },
            Self::RestoreDeletedLayer { assignments, .. } => Changes {
                edges: assignments
                    .iter()
                    .map(|(_, assignment)| assignment.edge)
                    .collect(),
                settings: true,
                ..Changes::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::AssetId;

    use super::*;

    fn vertex_at(x: f64, y: f64) -> Vertex {
        Vertex {
            id: VertexId::new(),
            position: Point2::new(x, y),
        }
    }

    #[test]
    fn add_edge_normalizes_one_proper_intersection_as_one_history_entry() {
        let left = vertex_at(-1.0, 0.0);
        let right = vertex_at(1.0, 0.0);
        let bottom = vertex_at(0.0, -1.0);
        let top = vertex_at(0.0, 1.0);
        let source = Edge {
            id: EdgeId::new(),
            start: left.id,
            end: right.id,
            kind: EdgeKind::Mountain,
        };
        let original = CreasePattern {
            vertices: vec![left.clone(), right.clone(), bottom.clone(), top.clone()],
            edges: vec![source],
        };
        let mut editor = EditorState::new(original.clone());
        editor
            .execute_add_edge_with_intersections(
                0,
                EdgeId::new(),
                bottom.id,
                top.id,
                EdgeKind::Valley,
            )
            .expect("add and normalize crossing");
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.pattern().vertices.len(), 5);
        assert_eq!(editor.pattern().edges.len(), 4);
        let normalized = editor.pattern().clone();
        editor.undo(1).expect("undo atomic add");
        assert_eq!(editor.pattern(), &original);
        editor.redo(2).expect("redo atomic add");
        assert_eq!(editor.pattern(), &normalized);
    }

    #[test]
    fn add_third_balloon_base_line_normalizes_shared_three_edge_cluster() {
        let a = vertex_at(-1.0, 0.0);
        let b = vertex_at(1.0, 0.0);
        let c = vertex_at(0.0, -1.0);
        let d = vertex_at(0.0, 1.0);
        let e = vertex_at(-1.0, -1.0);
        let f = vertex_at(1.0, 1.0);
        let first = Edge {
            id: EdgeId::new(),
            start: a.id,
            end: b.id,
            kind: EdgeKind::Mountain,
        };
        let second = Edge {
            id: EdgeId::new(),
            start: c.id,
            end: d.id,
            kind: EdgeKind::Valley,
        };
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![a, b, c, d, e.clone(), f.clone()],
            edges: vec![first, second],
        });
        editor
            .execute_add_edge_with_intersections(0, EdgeId::new(), e.id, f.id, EdgeKind::Auxiliary)
            .expect("normalize balloon base cluster");
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.pattern().vertices.len(), 7);
        assert_eq!(editor.pattern().edges.len(), 6);
        let center = editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.position == Point2::new(0.0, 0.0))
            .expect("shared center");
        assert_eq!(
            editor
                .pattern()
                .edges
                .iter()
                .filter(|edge| edge.start == center.id || edge.end == center.id)
                .count(),
            6
        );
    }

    #[test]
    fn add_edge_normalizes_endpoint_contact_as_t_junction() {
        let left = vertex_at(-1.0, 0.0);
        let right = vertex_at(1.0, 0.0);
        let junction = vertex_at(0.0, 0.0);
        let top = vertex_at(0.0, 1.0);
        let carrier = Edge {
            id: EdgeId::new(),
            start: left.id,
            end: right.id,
            kind: EdgeKind::Mountain,
        };
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![left, right, junction.clone(), top.clone()],
            edges: vec![carrier],
        });
        editor
            .execute_add_edge_with_intersections(
                0,
                EdgeId::new(),
                junction.id,
                top.id,
                EdgeKind::Valley,
            )
            .expect("normalize endpoint contact");
        assert_eq!(editor.pattern().vertices.len(), 4);
        assert_eq!(editor.pattern().edges.len(), 3);
        assert_eq!(
            editor
                .pattern()
                .edges
                .iter()
                .filter(|edge| edge.start == junction.id || edge.end == junction.id)
                .count(),
            3
        );
    }

    #[test]
    fn add_edge_normalizes_multiple_distinct_intersections_in_descending_order() {
        let left_low = vertex_at(-1.0, -0.5);
        let right_low = vertex_at(1.0, -0.5);
        let left_high = vertex_at(-1.0, 0.5);
        let right_high = vertex_at(1.0, 0.5);
        let bottom = vertex_at(0.0, -1.0);
        let top = vertex_at(0.0, 1.0);
        let low = Edge {
            id: EdgeId::new(),
            start: left_low.id,
            end: right_low.id,
            kind: EdgeKind::Mountain,
        };
        let high = Edge {
            id: EdgeId::new(),
            start: left_high.id,
            end: right_high.id,
            kind: EdgeKind::Valley,
        };
        let original = CreasePattern {
            vertices: vec![
                left_low,
                right_low,
                left_high,
                right_high,
                bottom.clone(),
                top.clone(),
            ],
            edges: vec![low, high],
        };
        let mut editor = EditorState::new(original.clone());
        editor
            .execute_add_edge_with_intersections(
                0,
                EdgeId::new(),
                bottom.id,
                top.id,
                EdgeKind::Auxiliary,
            )
            .expect("normalize two crossings");
        assert_eq!(editor.pattern().vertices.len(), 8);
        assert_eq!(editor.pattern().edges.len(), 7);
        editor
            .undo(1)
            .expect("undo all crossings and authored edge");
        assert_eq!(editor.pattern(), &original);
    }

    #[test]
    fn underlay_crud_undo_redo_and_layer_guards_are_atomic() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let layer = LayerRecordV1 {
            id: LayerId::new(),
            name: "Reference".to_owned(),
            content_kind: ori_domain::LayerContentKindV1::Underlay,
            visible: true,
            locked: false,
            opacity: 1.0,
        };
        editor
            .execute(
                0,
                Command::CreateLayer {
                    layer: layer.clone(),
                    target_index: 1,
                },
            )
            .expect("create layer");
        let mut record = UnderlayRecordV1 {
            id: UnderlayId::new(),
            asset: AssetId::new(),
            transform: ori_domain::UnderlayTransformV1 {
                position: Point2::new(0.0, 0.0),
                scale_x: 1.0,
                scale_y: 1.0,
                rotation_degrees: 0.0,
            },
            opacity: 0.5,
            layer: layer.id,
        };
        editor
            .execute(
                1,
                Command::AddUnderlay {
                    record: record.clone(),
                },
            )
            .unwrap();
        record.opacity = 0.75;
        editor
            .execute(
                2,
                Command::UpdateUnderlay {
                    record: record.clone(),
                },
            )
            .unwrap();
        assert_eq!(
            editor.execute(3, Command::DeleteLayer { layer: layer.id }),
            Err(CommandError::InvalidUnderlay)
        );
        editor
            .execute(3, Command::RemoveUnderlay { id: record.id })
            .unwrap();
        editor.undo(4).unwrap();
        editor.undo(5).unwrap();
        assert_eq!(editor.underlays().underlays[0].opacity, 0.5);
        editor.redo(6).unwrap();
        assert_eq!(editor.underlays().underlays[0].opacity, 0.75);
    }

    #[derive(Debug, PartialEq)]
    struct EditorStateSnapshot {
        pattern: CreasePattern,
        paper: Paper,
        geometric_constraints: GeometricConstraintDocumentV1,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        current_applied_pose: Option<crate::AppliedPoseV1>,
        revision: Revision,
        history_entry_limit: usize,
        undo_stack: String,
        redo_stack: String,
    }

    fn editor_state_snapshot(editor: &EditorState) -> EditorStateSnapshot {
        EditorStateSnapshot {
            pattern: editor.pattern.clone(),
            paper: editor.paper.clone(),
            geometric_constraints: editor.geometric_constraints.clone(),
            instruction_timeline: editor.instruction_timeline.clone(),
            project_layers: editor.project_layers.clone(),
            current_applied_pose: editor.current_applied_pose.clone(),
            revision: editor.revision,
            history_entry_limit: editor.history_entry_limit(),
            undo_stack: format!("{:?}", editor.undo_stack),
            redo_stack: format!("{:?}", editor.redo_stack),
        }
    }

    struct ConstraintResourceFixture {
        editor: EditorState,
        vertices: [VertexId; 4],
        edges: [EdgeId; 3],
        repair_vertex: VertexId,
        filler_vertex: Vertex,
        filler_edge: Edge,
    }

    fn constraint_resource_fixture(
        vertex_count: usize,
        edge_count: usize,
    ) -> ConstraintResourceFixture {
        assert!(vertex_count >= 5);
        assert!(edge_count >= 3);

        let vertices = std::array::from_fn::<_, 4, _>(|_| VertexId::new());
        let edges = std::array::from_fn::<_, 3, _>(|_| EdgeId::new());
        let mut pattern_vertices = vec![
            Vertex {
                id: vertices[0],
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: vertices[1],
                position: Point2::new(10.0, 10.0),
            },
            Vertex {
                id: vertices[2],
                position: Point2::new(0.0, 10.0),
            },
            Vertex {
                id: vertices[3],
                position: Point2::new(10.0, 0.0),
            },
        ];
        let repair_vertex = VertexId::new();
        pattern_vertices.push(Vertex {
            id: repair_vertex,
            position: Point2::new(20.0, 20.0),
        });
        let filler_vertex = Vertex {
            id: VertexId::new(),
            position: Point2::new(30.0, 30.0),
        };
        pattern_vertices.resize(vertex_count, filler_vertex.clone());

        let mut pattern_edges = vec![
            Edge {
                id: edges[0],
                start: vertices[0],
                end: vertices[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: edges[1],
                start: vertices[2],
                end: vertices[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: edges[2],
                start: vertices[0],
                end: vertices[2],
                kind: EdgeKind::Auxiliary,
            },
        ];
        let filler_edge = Edge {
            id: EdgeId::new(),
            start: vertices[1],
            end: vertices[2],
            kind: EdgeKind::Mountain,
        };
        pattern_edges.resize(edge_count, filler_edge.clone());

        let constraint = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge: edges[0] },
        };
        let editor = EditorState::with_document_parts_and_constraints(
            CreasePattern {
                vertices: pattern_vertices,
                edges: pattern_edges,
            },
            Paper::default(),
            InstructionTimeline::default(),
            GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: vec![constraint],
            },
        );
        ConstraintResourceFixture {
            editor,
            vertices,
            edges,
            repair_vertex,
            filler_vertex,
            filler_edge,
        }
    }

    fn rectangular_editor() -> (EditorState, CreasePattern, Paper) {
        let bottom_left = VertexId::new();
        let bottom_right = VertexId::new();
        let top_right = VertexId::new();
        let top_left = VertexId::new();
        let internal = VertexId::new();
        let outside = VertexId::new();
        let vertices = vec![
            Vertex {
                id: internal,
                position: Point2::new(60.0, 45.0),
            },
            Vertex {
                id: bottom_left,
                position: Point2::new(10.0, 20.0),
            },
            Vertex {
                id: outside,
                position: Point2::new(-40.0, 95.0),
            },
            Vertex {
                id: top_right,
                position: Point2::new(110.0, 70.0),
            },
            Vertex {
                id: top_left,
                position: Point2::new(10.0, 70.0),
            },
            Vertex {
                id: bottom_right,
                position: Point2::new(110.0, 20.0),
            },
        ];
        // Counter-clockwise and clockwise boundary orders are both valid. Use
        // the clockwise order here to exercise the less common orientation.
        let boundary_vertices = vec![bottom_left, top_left, top_right, bottom_right];
        let edges = vec![
            Edge {
                id: EdgeId::new(),
                start: bottom_left,
                end: top_left,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: top_left,
                end: top_right,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: top_right,
                end: bottom_right,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: bottom_right,
                end: bottom_left,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: internal,
                end: top_right,
                kind: EdgeKind::Mountain,
            },
        ];
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices,
            thickness_mm: 0.25,
            cutting_allowed: true,
            ..Paper::default()
        };
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
        )
    }

    fn simple_rectangular_editor() -> (EditorState, CreasePattern, Paper) {
        let sheet =
            crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid simple rectangle");
        let (pattern, paper) = sheet.into_parts();
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
        )
    }

    fn crossing_edges_editor() -> (EditorState, CreasePattern, Paper, Edge, Edge) {
        let sheet =
            crate::create_rectangular_sheet(100.0, 100.0, true).expect("valid crossing test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        pattern.vertices.extend([
            Vertex {
                id: ids[0],
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: ids[1],
                position: Point2::new(80.0, 80.0),
            },
            Vertex {
                id: ids[2],
                position: Point2::new(20.0, 80.0),
            },
            Vertex {
                id: ids[3],
                position: Point2::new(80.0, 20.0),
            },
        ]);
        let first = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Mountain,
        };
        let second = Edge {
            id: EdgeId::new(),
            start: ids[2],
            end: ids[3],
            kind: EdgeKind::Valley,
        };
        pattern.edges.extend([first.clone(), second.clone()]);
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
            first,
            second,
        )
    }

    fn two_edge_editor(points: [Point2; 4]) -> (EditorState, Edge, Edge) {
        let ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let vertices = ids
            .into_iter()
            .zip(points)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let first = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Mountain,
        };
        let second = Edge {
            id: EdgeId::new(),
            start: ids[2],
            end: ids[3],
            kind: EdgeKind::Valley,
        };
        let editor = EditorState::new(CreasePattern {
            vertices,
            edges: vec![first.clone(), second.clone()],
        });
        (editor, first, second)
    }

    fn t_junction_editor() -> (EditorState, CreasePattern, Paper, Edge, Edge, Edge) {
        let sheet = crate::create_rectangular_sheet(100.0, 100.0, true)
            .expect("valid T-junction test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let interior_start = VertexId::new();
        let interior_end = VertexId::new();
        let stem_other = VertexId::new();
        let junction = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: interior_start,
                position: Point2::new(20.0, 50.0),
            },
            Vertex {
                id: interior_end,
                position: Point2::new(80.0, 50.0),
            },
            Vertex {
                id: stem_other,
                position: Point2::new(32.0, 20.0),
            },
            Vertex {
                id: junction,
                position: Point2::new(32.0, 50.0),
            },
        ]);
        let interior = Edge {
            id: EdgeId::new(),
            start: interior_start,
            end: interior_end,
            kind: EdgeKind::Cut,
        };
        let unrelated = Edge {
            id: EdgeId::new(),
            start: interior_start,
            end: stem_other,
            kind: EdgeKind::Mountain,
        };
        let stem = Edge {
            id: EdgeId::new(),
            start: junction,
            end: stem_other,
            kind: EdgeKind::Auxiliary,
        };
        pattern
            .edges
            .extend([interior.clone(), unrelated.clone(), stem.clone()]);
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
            interior,
            stem,
            unrelated,
        )
    }

    fn boundary_t_junction_editor(
        boundary_edge_index: usize,
    ) -> (EditorState, CreasePattern, Paper, Edge, Edge, VertexId) {
        let sheet = crate::create_rectangular_sheet(100.0, 100.0, true)
            .expect("valid boundary T-junction test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let junction = VertexId::new();
        let stem_other = VertexId::new();
        let (junction_position, stem_other_position) = match boundary_edge_index {
            0 => (Point2::new(40.0, 0.0), Point2::new(40.0, 30.0)),
            3 => (Point2::new(0.0, 40.0), Point2::new(30.0, 40.0)),
            _ => panic!("test helper only needs the top and closing boundary edges"),
        };
        pattern.vertices.extend([
            Vertex {
                id: junction,
                position: junction_position,
            },
            Vertex {
                id: stem_other,
                position: stem_other_position,
            },
        ]);
        let boundary_edge = pattern.edges[boundary_edge_index].clone();
        let stem = Edge {
            id: EdgeId::new(),
            start: junction,
            end: stem_other,
            kind: EdgeKind::Mountain,
        };
        pattern.edges.push(stem.clone());
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
            boundary_edge,
            stem,
            junction,
        )
    }

    fn three_way_create_cluster() -> (CreasePattern, Paper, [Edge; 3], Edge) {
        let horizontal_start = VertexId::new();
        let horizontal_end = VertexId::new();
        let vertical_start = VertexId::new();
        let vertical_end = VertexId::new();
        let diagonal_start = VertexId::new();
        let diagonal_end = VertexId::new();
        let unrelated_start = VertexId::new();
        let unrelated_end = VertexId::new();
        let vertices = vec![
            Vertex {
                id: horizontal_start,
                position: Point2::new(-10.0, 0.0),
            },
            Vertex {
                id: unrelated_start,
                position: Point2::new(30.0, 40.0),
            },
            Vertex {
                id: vertical_end,
                position: Point2::new(0.0, 10.0),
            },
            Vertex {
                id: diagonal_start,
                position: Point2::new(-10.0, -10.0),
            },
            Vertex {
                id: horizontal_end,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: vertical_start,
                position: Point2::new(0.0, -10.0),
            },
            Vertex {
                id: unrelated_end,
                position: Point2::new(40.0, 40.0),
            },
            Vertex {
                id: diagonal_end,
                position: Point2::new(10.0, 10.0),
            },
        ];
        let horizontal = Edge {
            id: EdgeId::new(),
            start: horizontal_start,
            end: horizontal_end,
            kind: EdgeKind::Mountain,
        };
        let vertical = Edge {
            id: EdgeId::new(),
            start: vertical_start,
            end: vertical_end,
            kind: EdgeKind::Valley,
        };
        let diagonal = Edge {
            id: EdgeId::new(),
            start: diagonal_start,
            end: diagonal_end,
            kind: EdgeKind::Cut,
        };
        let unrelated = Edge {
            id: EdgeId::new(),
            start: unrelated_start,
            end: unrelated_end,
            kind: EdgeKind::Auxiliary,
        };
        (
            CreasePattern {
                vertices,
                edges: vec![
                    vertical.clone(),
                    unrelated.clone(),
                    horizontal.clone(),
                    diagonal.clone(),
                ],
            },
            Paper::default(),
            [horizontal, vertical, diagonal],
            unrelated,
        )
    }

    fn maximum_size_create_cluster() -> (CreasePattern, Vec<Edge>) {
        let mut pattern = CreasePattern::empty();
        let mut edges = Vec::with_capacity(MAX_INTERSECTION_CLUSTER_TARGETS);
        for index in 0..MAX_INTERSECTION_CLUSTER_TARGETS {
            let offset = index as f64 - 32.0;
            let start = VertexId::new();
            let end = VertexId::new();
            pattern.vertices.extend([
                Vertex {
                    id: start,
                    position: Point2::new(-100.0, -offset),
                },
                Vertex {
                    id: end,
                    position: Point2::new(100.0, offset),
                },
            ]);
            let edge = Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: match index % 4 {
                    0 => EdgeKind::Mountain,
                    1 => EdgeKind::Valley,
                    2 => EdgeKind::Auxiliary,
                    _ => EdgeKind::Cut,
                },
            };
            pattern.edges.push(edge.clone());
            edges.push(edge);
        }
        (pattern, edges)
    }

    fn mixed_reuse_cluster() -> (CreasePattern, Paper, VertexId, [Edge; 4], Edge) {
        let junction = VertexId::new();
        let diagonal_start = VertexId::new();
        let diagonal_end = VertexId::new();
        let vertical_end = VertexId::new();
        let horizontal_start = VertexId::new();
        let horizontal_end = VertexId::new();
        let auxiliary_start = VertexId::new();
        let unrelated_start = VertexId::new();
        let unrelated_end = VertexId::new();
        let vertices = vec![
            Vertex {
                id: diagonal_start,
                position: Point2::new(-10.0, -10.0),
            },
            Vertex {
                id: vertical_end,
                position: Point2::new(0.0, 10.0),
            },
            Vertex {
                id: junction,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: horizontal_end,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: unrelated_start,
                position: Point2::new(30.0, 30.0),
            },
            Vertex {
                id: auxiliary_start,
                position: Point2::new(10.0, 5.0),
            },
            Vertex {
                id: diagonal_end,
                position: Point2::new(10.0, 10.0),
            },
            Vertex {
                id: horizontal_start,
                position: Point2::new(-10.0, 0.0),
            },
            Vertex {
                id: unrelated_end,
                position: Point2::new(40.0, 30.0),
            },
        ];
        let diagonal = Edge {
            id: EdgeId::new(),
            start: diagonal_start,
            end: diagonal_end,
            kind: EdgeKind::Cut,
        };
        let endpoint_start = Edge {
            id: EdgeId::new(),
            start: junction,
            end: vertical_end,
            kind: EdgeKind::Valley,
        };
        let horizontal = Edge {
            id: EdgeId::new(),
            start: horizontal_start,
            end: horizontal_end,
            kind: EdgeKind::Mountain,
        };
        let endpoint_end = Edge {
            id: EdgeId::new(),
            start: auxiliary_start,
            end: junction,
            kind: EdgeKind::Auxiliary,
        };
        let unrelated = Edge {
            id: EdgeId::new(),
            start: unrelated_start,
            end: unrelated_end,
            kind: EdgeKind::Mountain,
        };
        (
            CreasePattern {
                vertices,
                edges: vec![
                    diagonal.clone(),
                    endpoint_start.clone(),
                    unrelated.clone(),
                    horizontal.clone(),
                    endpoint_end.clone(),
                ],
            },
            Paper::default(),
            junction,
            [diagonal, endpoint_start, horizontal, endpoint_end],
            unrelated,
        )
    }

    fn decimal_roundoff_cluster() -> (CreasePattern, [Edge; 3]) {
        let points = [
            Point2::new(-0.6813186813186812, -8.192513368983956),
            Point2::new(2.1758241758241756, 13.171122994652405),
            Point2::new(-0.967032967032967, -2.648331550802139),
            Point2::new(2.6043956043956045, 4.854850267379678),
            Point2::new(-1.2527472527472527, -15.831422459893046),
            Point2::new(3.032967032967033, 24.62948663101604),
        ];
        let ids = points.map(|_| VertexId::new());
        let vertices = ids
            .into_iter()
            .zip(points)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[2],
                end: ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[4],
                end: ids[5],
                kind: EdgeKind::Auxiliary,
            },
        ];
        (
            CreasePattern {
                vertices,
                edges: edges.to_vec(),
            },
            edges,
        )
    }

    fn intersection_candidate_authority_fixture() -> (CreasePattern, [Edge; 3]) {
        let points = [
            Point2::new(173.07621414592302, 91.66043241538091),
            Point2::new(569.9014920216193, 386.3713463650028),
            Point2::new(663.6958618662285, 139.55388677767561),
            Point2::new(174.18088943020507, 622.95489315637),
            Point2::new(506.91442317040105, 315.5937659528375),
            Point2::new(288.44553146386033, 353.9680334718498),
        ];
        let ids = points.map(|_| VertexId::new());
        let vertices = ids
            .into_iter()
            .zip(points)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[2],
                end: ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[4],
                end: ids[5],
                kind: EdgeKind::Auxiliary,
            },
        ];
        (
            CreasePattern {
                vertices,
                edges: edges.to_vec(),
            },
            edges,
        )
    }

    fn fma_intersection_candidate_fixture() -> (CreasePattern, [Edge; 3]) {
        let points = [
            Point2::new(-761.6238217708569, 358.3305812537483),
            Point2::new(-67.42483397026956, 649.4029881962348),
            Point2::new(-482.92748512729344, 475.6081546439962),
            Point2::new(-134.2414665987278, 607.9754595950404),
            Point2::new(-520.4852242823567, 85.03606537939055),
            Point2::new(-425.8068100384997, 860.1397882516304),
        ];
        let ids = points.map(|_| VertexId::new());
        let vertices = ids
            .into_iter()
            .zip(points)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[2],
                end: ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[4],
                end: ids[5],
                kind: EdgeKind::Cut,
            },
        ];
        (
            CreasePattern {
                vertices,
                edges: edges.to_vec(),
            },
            edges,
        )
    }

    fn reverse_only_intersection_candidate_fixture() -> (CreasePattern, [Edge; 3]) {
        let points = [
            Point2::new(1244.6740584037207, 192.04027142069572),
            Point2::new(-35.10840974525854, 937.7761247925646),
            Point2::new(1186.7878486119257, 183.0718879233308),
            Point2::new(300.887521406075, 756.4467701217807),
            Point2::new(528.9518373283253, 586.240607350301),
            Point2::new(388.36943464413514, 1474.8760422947796),
        ];
        let ids = points.map(|_| VertexId::new());
        let vertices = ids
            .into_iter()
            .zip(points)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[2],
                end: ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[4],
                end: ids[5],
                kind: EdgeKind::Auxiliary,
            },
        ];
        (
            CreasePattern {
                vertices,
                edges: edges.to_vec(),
            },
            edges,
        )
    }

    fn collinear_after_removal_editor() -> (EditorState, CreasePattern, Paper, VertexId) {
        let previous = VertexId::new();
        let target = VertexId::new();
        let next = VertexId::new();
        let middle = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: previous,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(1.0, 1.0),
                },
                Vertex {
                    id: next,
                    position: Point2::new(2.0, 0.0),
                },
                Vertex {
                    id: middle,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![
                Edge {
                    id: EdgeId::new(),
                    start: previous,
                    end: target,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: EdgeId::new(),
                    start: target,
                    end: next,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: EdgeId::new(),
                    start: next,
                    end: middle,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: EdgeId::new(),
                    start: middle,
                    end: previous,
                    kind: EdgeKind::Boundary,
                },
            ],
        };
        let paper = Paper {
            boundary_vertices: vec![previous, target, next, middle],
            ..Paper::default()
        };
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
            target,
        )
    }

    fn assert_split_rejected(editor: &mut EditorState, command: Command, expected: CommandError) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let error = editor
            .execute(editor.revision(), command)
            .expect_err("edge split must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    fn assert_intersection_rejected(
        editor: &mut EditorState,
        command: Command,
        expected: CommandError,
    ) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let revision = editor.revision();
        let error = editor
            .execute(revision, command)
            .expect_err("edge intersection connection must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), revision);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    fn assert_cluster_rejected(editor: &mut EditorState, command: Command, expected: CommandError) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let revision = editor.revision();
        let error = editor
            .execute(revision, command)
            .expect_err("intersection cluster must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), revision);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    fn assert_t_junction_rejected(
        editor: &mut EditorState,
        command: Command,
        expected: CommandError,
    ) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let revision = editor.revision();
        let error = editor
            .execute(revision, command)
            .expect_err("T-junction connection must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), revision);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    fn assert_boundary_removal_rejected(
        editor: &mut EditorState,
        vertex: VertexId,
        expected: CommandError,
    ) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let error = editor
            .execute(editor.revision(), Command::RemoveBoundaryVertex { vertex })
            .expect_err("boundary vertex removal must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn pattern_only_constructor_uses_default_paper_without_history() {
        let editor = EditorState::new(CreasePattern::empty());

        assert_eq!(editor.paper(), &Paper::default());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn add_move_undo_redo_preserves_vertex_id() {
        let id = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("add vertex");
        editor
            .execute(
                1,
                Command::MoveVertex {
                    id,
                    position: Point2::new(5.0, 8.0),
                },
            )
            .expect("move vertex");
        editor.undo(2).expect("undo move");
        assert_eq!(editor.pattern().vertices[0].position, Point2::new(1.0, 2.0));
        editor.redo(3).expect("redo move");
        assert_eq!(editor.pattern().vertices[0].id, id);
        assert_eq!(editor.pattern().vertices[0].position, Point2::new(5.0, 8.0));
    }

    #[test]
    fn rejects_stale_revision_without_mutation() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let error = editor
            .execute(
                9,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect_err("stale command must fail");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 9,
                actual: 0
            }
        );
        assert!(editor.pattern().vertices.is_empty());
    }

    #[test]
    fn revision_exhaustion_rejects_execute_without_mutating_document_or_history() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor.revision = MAX_REVISION;
        let before = editor_state_snapshot(&editor);

        let result = editor.execute(
            MAX_REVISION,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 2.0),
            },
        );

        assert_eq!(
            result,
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn revision_exhaustion_rejects_undo_without_mutating_document_or_history() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("prepare undo history");
        editor.revision = MAX_REVISION;
        let before = editor_state_snapshot(&editor);

        let result = editor.undo(MAX_REVISION);

        assert_eq!(
            result,
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
        assert_eq!(editor.pattern.vertices[0].id, vertex);
    }

    #[test]
    fn revision_exhaustion_rejects_redo_without_mutating_document_or_history() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("prepare undo history");
        editor.undo(1).expect("prepare redo history");
        editor.revision = MAX_REVISION;
        let before = editor_state_snapshot(&editor);

        let result = editor.redo(MAX_REVISION);

        assert_eq!(
            result,
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
        assert!(editor.pattern.vertices.is_empty());
    }

    #[test]
    fn revision_exhaustion_rejects_empty_undo_and_redo() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor.revision = MAX_REVISION;
        let before = editor_state_snapshot(&editor);

        assert_eq!(
            editor.undo(MAX_REVISION),
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
        assert_eq!(
            editor.redo(MAX_REVISION),
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn revision_can_advance_to_the_last_json_safe_integer_exactly_once() {
        assert_eq!(MAX_REVISION, 9_007_199_254_740_991);
        let first_vertex = VertexId::new();
        let second_vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor.revision = MAX_REVISION - 1;

        let result = editor
            .execute(
                MAX_REVISION - 1,
                Command::AddVertex {
                    id: first_vertex,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("the final safe revision must remain usable");

        assert_eq!(result.revision, MAX_REVISION);
        let before_rejection = editor_state_snapshot(&editor);
        assert_eq!(
            editor.execute(
                MAX_REVISION,
                Command::AddVertex {
                    id: second_vertex,
                    position: Point2::new(3.0, 4.0),
                },
            ),
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before_rejection);
        assert_eq!(editor.pattern.vertices[0].id, first_vertex);
    }

    #[test]
    fn revision_advances_monotonically_across_execute_undo_and_redo() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());

        let add = editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("add vertex");
        let move_vertex = editor
            .execute(
                add.revision,
                Command::MoveVertex {
                    id: vertex,
                    position: Point2::new(3.0, 4.0),
                },
            )
            .expect("move vertex");
        let undo = editor.undo(move_vertex.revision).expect("undo move");
        let redo = editor.redo(undo.revision).expect("redo move");

        assert_eq!(
            [
                add.revision,
                move_vertex.revision,
                undo.revision,
                redo.revision
            ],
            [1, 2, 3, 4]
        );
        assert_eq!(editor.revision(), 4);
    }

    #[test]
    fn move_edge_translates_both_endpoints_as_one_undoable_edit() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(1.0, 2.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(4.0, 6.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        });

        let moved = editor
            .execute(
                0,
                Command::MoveEdge {
                    id: edge,
                    start_position: Point2::new(11.0, -3.0),
                    end_position: Point2::new(14.0, 1.0),
                },
            )
            .expect("move the whole edge");
        assert_eq!(moved.revision, 1);
        assert_eq!(moved.changed_vertices, vec![start, end]);
        assert_eq!(moved.changed_edges, vec![edge]);
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(11.0, -3.0));
        assert_eq!(editor.pattern.vertices[1].position, Point2::new(14.0, 1.0));

        editor.undo(1).expect("undo the whole edge move");
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(1.0, 2.0));
        assert_eq!(editor.pattern.vertices[1].position, Point2::new(4.0, 6.0));
        editor.redo(2).expect("redo the whole edge move");
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(11.0, -3.0));
        assert_eq!(editor.pattern.vertices[1].position, Point2::new(14.0, 1.0));
    }

    #[test]
    fn move_vertices_is_atomic_bounded_unique_and_undoable() {
        let first = VertexId::new();
        let second = VertexId::new();
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(1.0, 2.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(3.0, 4.0),
                },
            ],
            edges: Vec::new(),
        });
        let duplicate = Command::MoveVertices {
            updates: vec![
                VertexPositionUpdate {
                    vertex: first,
                    position: Point2::new(5.0, 6.0),
                },
                VertexPositionUpdate {
                    vertex: first,
                    position: Point2::new(7.0, 8.0),
                },
            ],
        };
        assert_eq!(
            editor.execute(0, duplicate),
            Err(CommandError::InvalidVertexMoveBatch)
        );
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(1.0, 2.0));
        assert_eq!(
            editor.execute(
                0,
                Command::MoveVertices {
                    updates: vec![VertexPositionUpdate {
                        vertex: first,
                        position: Point2::new(f64::NAN, 2.0),
                    }],
                },
            ),
            Err(CommandError::VertexMovePositionNotFinite { vertex: first })
        );
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(1.0, 2.0));

        editor
            .execute(
                0,
                Command::MoveVertices {
                    updates: vec![
                        VertexPositionUpdate {
                            vertex: first,
                            position: Point2::new(11.0, 12.0),
                        },
                        VertexPositionUpdate {
                            vertex: second,
                            position: Point2::new(13.0, 14.0),
                        },
                    ],
                },
            )
            .expect("move a face vertex batch");
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(11.0, 12.0));
        assert_eq!(editor.pattern.vertices[1].position, Point2::new(13.0, 14.0));
        editor.undo(1).expect("undo the entire vertex batch");
        assert_eq!(editor.pattern.vertices[0].position, Point2::new(1.0, 2.0));
        assert_eq!(editor.pattern.vertices[1].position, Point2::new(3.0, 4.0));
    }

    #[test]
    fn stale_revision_takes_precedence_over_revision_exhaustion() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor.revision = MAX_REVISION;
        let before = editor_state_snapshot(&editor);

        assert_eq!(
            editor.execute(
                MAX_REVISION - 1,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(1.0, 2.0),
                },
            ),
            Err(CommandError::RevisionConflict {
                expected: MAX_REVISION - 1,
                actual: MAX_REVISION
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn edge_is_undoable_and_keeps_its_id() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("add start");
        editor
            .execute(
                1,
                Command::AddVertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            )
            .expect("add end");
        editor
            .execute(
                2,
                Command::AddEdge {
                    id: edge,
                    start,
                    end,
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add edge");
        editor.undo(3).expect("undo edge");
        assert!(editor.pattern().edges.is_empty());
        editor.redo(4).expect("redo edge");
        assert_eq!(editor.pattern().edges[0].id, edge);
    }

    #[test]
    fn connected_vertex_and_edge_are_one_atomic_undoable_command() {
        let start = VertexId::new();
        let endpoint = VertexId::new();
        let edge = EdgeId::new();
        let position = Point2::new(3.0, 4.0);
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            }],
            edges: Vec::new(),
        });

        editor
            .execute(
                0,
                Command::AddConnectedVertex {
                    vertex_id: endpoint,
                    position,
                    edge_id: edge,
                    start,
                    kind: EdgeKind::Valley,
                },
            )
            .expect("add endpoint and edge atomically");
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.pattern().vertices.len(), 2);
        assert_eq!(editor.pattern().vertices[1].id, endpoint);
        assert_eq!(editor.pattern().vertices[1].position, position);
        assert_eq!(editor.pattern().edges.len(), 1);
        assert_eq!(
            editor.pattern().edges[0],
            Edge {
                id: edge,
                start,
                end: endpoint,
                kind: EdgeKind::Valley,
            }
        );

        editor.undo(1).expect("one undo removes both records");
        assert_eq!(editor.pattern().vertices.len(), 1);
        assert!(editor.pattern().edges.is_empty());
        editor.redo(2).expect("one redo restores both records");
        assert_eq!(editor.pattern().vertices[1].id, endpoint);
        assert_eq!(editor.pattern().edges[0].id, edge);
    }

    #[test]
    fn connected_vertex_failure_preserves_the_entire_editor() {
        let start = VertexId::new();
        let existing_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            }],
            edges: vec![Edge {
                id: existing_edge,
                start,
                end: VertexId::new(),
                kind: EdgeKind::Mountain,
            }],
        };
        let mut editor = EditorState::new(pattern);
        let before = editor_state_snapshot(&editor);
        assert!(matches!(
            editor.execute(
                0,
                Command::AddConnectedVertex {
                    vertex_id: VertexId::new(),
                    position: Point2::new(5.0, 0.0),
                    edge_id: existing_edge,
                    start,
                    kind: EdgeKind::Mountain,
                },
            ),
            Err(CommandError::EdgeAlreadyExists(id)) if id == existing_edge
        ));
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn connected_vertex_cannot_be_removed() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Valley,
            }],
        };
        let mut editor = EditorState::new(pattern);
        let error = editor
            .execute(0, Command::RemoveVertex { id: start })
            .expect_err("connected vertex removal must fail");
        assert_eq!(
            error,
            CommandError::VertexHasConnectedEdge {
                vertex: start,
                edge
            }
        );
    }

    #[test]
    fn boundary_edge_cannot_be_added_by_a_generic_command() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(pattern);

        let error = editor
            .execute(
                0,
                Command::AddEdge {
                    id: edge,
                    start,
                    end,
                    kind: EdgeKind::Boundary,
                },
            )
            .expect_err("generic boundary creation must fail");

        assert_eq!(
            error,
            CommandError::BoundaryEdgeRequiresSheetOperation(edge)
        );
        assert!(editor.pattern().edges.is_empty());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn boundary_edge_cannot_be_removed_by_a_generic_command() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let boundary = Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Boundary,
        };
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![boundary.clone()],
        };
        let mut editor = EditorState::new(pattern);

        let error = editor
            .execute(0, Command::RemoveEdge { id: edge })
            .expect_err("generic boundary removal must fail");

        assert_eq!(
            error,
            CommandError::BoundaryEdgeRequiresSheetOperation(edge)
        );
        assert_eq!(editor.pattern().edges, vec![boundary]);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn cut_edges_require_an_undoable_project_setting() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(pattern);
        let cut = Command::AddEdge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Cut,
        };
        assert_eq!(
            editor
                .execute(0, cut.clone())
                .expect_err("cut must be disabled"),
            CommandError::CuttingDisabled
        );
        editor
            .execute(0, Command::SetCuttingAllowed { allowed: true })
            .expect("enable cutting");
        editor.execute(1, cut).expect("add cut");
        assert_eq!(editor.pattern().edges.len(), 1);
        editor.undo(2).expect("undo cut");
        editor.undo(3).expect("undo setting");
        assert!(!editor.cutting_allowed());
    }

    #[test]
    fn paper_properties_are_one_undoable_command_including_textures() {
        let front_texture = ori_domain::AssetId::new();
        let back_texture = ori_domain::AssetId::new();
        let mut paper = Paper::default();
        paper.front.texture_asset = Some(front_texture);
        paper.back.texture_asset = Some(back_texture);
        let original = paper.clone();
        let mut editor = EditorState::with_paper(CreasePattern::empty(), paper);
        let front_color = RgbaColor::opaque(12, 34, 56);
        let back_color = RgbaColor::opaque(210, 190, 170);

        let result = editor
            .execute(
                0,
                Command::UpdatePaperProperties {
                    thickness_mm: 0.0,
                    front_color,
                    back_color,
                    front_texture_asset: None,
                    back_texture_asset: None,
                    cutting_allowed: true,
                },
            )
            .expect("update paper properties");

        assert_eq!(result.revision, 1);
        assert!(result.settings_changed);
        assert!(result.changed_vertices.is_empty());
        assert!(result.changed_edges.is_empty());
        assert_eq!(editor.paper().thickness_mm, 0.0);
        assert_eq!(editor.paper().front.color, front_color);
        assert_eq!(editor.paper().back.color, back_color);
        assert_eq!(editor.paper().front.texture_asset, None);
        assert_eq!(editor.paper().back.texture_asset, None);
        assert!(editor.paper().cutting_allowed);

        editor.undo(1).expect("undo paper properties");
        assert_eq!(editor.paper(), &original);
        editor.redo(2).expect("redo paper properties");
        assert_eq!(editor.paper().thickness_mm, 0.0);
        assert_eq!(editor.paper().front.color, front_color);
        assert_eq!(editor.paper().back.color, back_color);
        assert_eq!(editor.paper().front.texture_asset, None);
        assert_eq!(editor.paper().back.texture_asset, None);
        assert!(editor.paper().cutting_allowed);
    }

    #[test]
    fn vertex_edge_and_face_metadata_are_undoable_and_persistable() {
        let (mut editor, pattern, _) = simple_rectangular_editor();
        let metadata = ElementMetadataV1 {
            name: "基準".to_owned(),
            color: Some(RgbaColor::opaque(12, 34, 56)),
            memo: "選択要素".to_owned(),
        };
        let targets = [
            ElementMetadataTargetV1::Vertex(pattern.vertices[0].id),
            ElementMetadataTargetV1::Edge(pattern.edges[0].id),
            ElementMetadataTargetV1::Face(FaceId::new()),
        ];
        for (revision, target) in targets.into_iter().enumerate() {
            editor
                .execute(
                    revision as u64,
                    Command::SetElementMetadata {
                        target,
                        metadata: Some(metadata.clone()),
                    },
                )
                .expect("set metadata");
        }
        assert_eq!(editor.element_metadata().vertices.len(), 1);
        assert_eq!(editor.element_metadata().edges.len(), 1);
        assert_eq!(editor.element_metadata().faces.len(), 1);
        editor.undo(3).expect("undo face metadata");
        assert!(editor.element_metadata().faces.is_empty());
        editor.redo(4).expect("redo face metadata");
        assert_eq!(editor.element_metadata().faces[0].metadata, metadata);
    }

    #[test]
    fn length_display_unit_is_undoable_and_ratio_uses_a_live_boundary_edge() {
        let (mut editor, original_pattern, original_paper) = simple_rectangular_editor();
        let reference_edge = original_pattern.edges[0].id;
        let fingerprint = editor.fold_model_fingerprint_v1();

        let result = editor
            .execute(
                0,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::PaperEdgeRatio { reference_edge },
                },
            )
            .expect("select valid boundary reference");

        assert_eq!(result.revision, 1);
        assert!(result.settings_changed);
        assert!(result.changed_vertices.is_empty());
        assert!(result.changed_edges.is_empty());
        assert_eq!(
            editor.paper().length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio { reference_edge }
        );
        assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);

        let endpoint = original_pattern.edges[0].end;
        editor
            .execute(
                1,
                Command::MoveVertex {
                    id: endpoint,
                    position: Point2::new(125.0, 0.0),
                },
            )
            .expect("reference endpoint can move");
        editor
            .execute(
                2,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::PaperEdgeRatio { reference_edge },
                },
            )
            .expect("moved reference is revalidated at its live length");

        editor.undo(3).expect("undo repeated ratio setting");
        editor.undo(4).expect("undo reference endpoint move");
        editor.undo(5).expect("undo ratio setting");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(6).expect("redo ratio setting");
        assert_eq!(
            editor.paper().length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio { reference_edge }
        );
    }

    #[test]
    fn absolute_length_display_units_do_not_require_geometry() {
        for unit in [
            LengthDisplayUnit::Millimeter,
            LengthDisplayUnit::Centimeter,
            LengthDisplayUnit::Inch,
        ] {
            let mut editor = EditorState::new(CreasePattern::empty());
            let result = editor
                .execute(0, Command::SetLengthDisplayUnit { unit })
                .expect("absolute display unit");
            assert!(result.settings_changed);
            assert_eq!(editor.paper().length_display_unit, unit);
            editor.undo(1).expect("undo absolute display unit");
            assert_eq!(
                editor.paper().length_display_unit,
                LengthDisplayUnit::Millimeter
            );
        }
    }

    #[test]
    fn invalid_ratio_references_are_rejected_atomically() {
        fn assert_invalid(mut editor: EditorState, reference_edge: EdgeId) {
            let before = editor_state_snapshot(&editor);
            let error = editor
                .execute(
                    0,
                    Command::SetLengthDisplayUnit {
                        unit: LengthDisplayUnit::PaperEdgeRatio { reference_edge },
                    },
                )
                .expect_err("invalid display reference must fail");
            assert_eq!(
                error,
                CommandError::LengthDisplayReferenceEdgeInvalid {
                    edge: reference_edge
                }
            );
            assert_eq!(editor_state_snapshot(&editor), before);
        }

        let (_, pattern, paper) = simple_rectangular_editor();
        assert_invalid(
            EditorState::with_paper(pattern.clone(), paper.clone()),
            EdgeId::new(),
        );

        let mut non_boundary = pattern.clone();
        let auxiliary = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Auxiliary,
        };
        non_boundary.edges.push(auxiliary.clone());
        assert_invalid(
            EditorState::with_paper(non_boundary, paper.clone()),
            auxiliary.id,
        );

        let mut duplicate = pattern.clone();
        duplicate.edges.push(pattern.edges[0].clone());
        assert_invalid(
            EditorState::with_paper(duplicate, paper.clone()),
            pattern.edges[0].id,
        );

        let mut duplicate_carrier = pattern.clone();
        duplicate_carrier.edges.push(Edge {
            id: EdgeId::new(),
            ..pattern.edges[0].clone()
        });
        assert_invalid(
            EditorState::with_paper(duplicate_carrier, paper.clone()),
            pattern.edges[0].id,
        );

        let mut detached = pattern.clone();
        let detached_edge = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Boundary,
        };
        detached.edges.push(detached_edge.clone());
        assert_invalid(
            EditorState::with_paper(detached, paper.clone()),
            detached_edge.id,
        );

        let mut collapsed = pattern.clone();
        let reference_edge = collapsed.edges[0].clone();
        let start_position = collapsed
            .vertices
            .iter()
            .find(|vertex| vertex.id == reference_edge.start)
            .expect("reference start")
            .position;
        collapsed
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == reference_edge.end)
            .expect("reference end")
            .position = start_position;
        assert_invalid(
            EditorState::with_paper(collapsed, paper.clone()),
            reference_edge.id,
        );

        let mut missing_endpoint = pattern.clone();
        missing_endpoint
            .vertices
            .retain(|vertex| vertex.id != reference_edge.end);
        assert_invalid(
            EditorState::with_paper(missing_endpoint, paper.clone()),
            reference_edge.id,
        );

        let mut duplicate_endpoint = pattern.clone();
        duplicate_endpoint.vertices.push(
            duplicate_endpoint
                .vertices
                .iter()
                .find(|vertex| vertex.id == reference_edge.end)
                .expect("reference endpoint")
                .clone(),
        );
        assert_invalid(
            EditorState::with_paper(duplicate_endpoint, paper.clone()),
            reference_edge.id,
        );

        let mut non_finite_endpoint = pattern.clone();
        non_finite_endpoint
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == reference_edge.end)
            .expect("reference endpoint")
            .position
            .x = f64::INFINITY;
        assert_invalid(
            EditorState::with_paper(non_finite_endpoint, paper.clone()),
            reference_edge.id,
        );

        let mut overflowing_length = pattern.clone();
        overflowing_length
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == reference_edge.start)
            .expect("reference start")
            .position
            .x = -f64::MAX;
        overflowing_length
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == reference_edge.end)
            .expect("reference end")
            .position
            .x = f64::MAX;
        assert_invalid(
            EditorState::with_paper(overflowing_length, paper.clone()),
            reference_edge.id,
        );

        let mut multiply_matched_paper = paper;
        multiply_matched_paper.boundary_vertices = vec![
            reference_edge.start,
            reference_edge.end,
            reference_edge.start,
            pattern.edges[1].end,
        ];
        assert_invalid(
            EditorState::with_paper(pattern, multiply_matched_paper),
            reference_edge.id,
        );
    }

    #[test]
    fn a_valid_ratio_reference_cannot_be_collapsed_or_made_non_finite() {
        for invalid_position in [
            Point2::new(0.0, 0.0),
            Point2::new(f64::NAN, 0.0),
            Point2::new(f64::MAX, f64::MAX),
        ] {
            let (mut editor, pattern, _) = simple_rectangular_editor();
            let edge = pattern.edges[0].clone();
            editor
                .execute(
                    0,
                    Command::SetLengthDisplayUnit {
                        unit: LengthDisplayUnit::PaperEdgeRatio {
                            reference_edge: edge.id,
                        },
                    },
                )
                .expect("set ratio reference");
            let before = editor_state_snapshot(&editor);

            let error = editor
                .execute(
                    1,
                    Command::MoveVertex {
                        id: edge.end,
                        position: invalid_position,
                    },
                )
                .expect_err("invalid reference move must fail");

            assert_eq!(
                error,
                CommandError::LengthDisplayReferenceEdgeWouldBecomeInvalid { edge: edge.id }
            );
            assert_eq!(editor_state_snapshot(&editor), before);
        }
    }

    #[test]
    fn reference_edge_split_and_removal_are_blocked_without_rebasing() {
        let (mut split_editor, split_pattern, _) = simple_rectangular_editor();
        let split_reference = split_pattern.edges[0].id;
        split_editor
            .execute(
                0,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::PaperEdgeRatio {
                        reference_edge: split_reference,
                    },
                },
            )
            .expect("set split reference");
        let before_split = editor_state_snapshot(&split_editor);
        let split_error = split_editor
            .execute(
                1,
                Command::SplitBoundaryEdge {
                    edge: split_reference,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect_err("reference split must fail");
        assert_eq!(
            split_error,
            CommandError::LengthDisplayReferenceEdgeMutationBlocked {
                edge: split_reference
            }
        );
        assert_eq!(editor_state_snapshot(&split_editor), before_split);

        for reference_index in [0, 1] {
            let (mut remove_editor, remove_pattern, remove_paper) = simple_rectangular_editor();
            let reference = remove_pattern.edges[reference_index].id;
            remove_editor
                .execute(
                    0,
                    Command::SetLengthDisplayUnit {
                        unit: LengthDisplayUnit::PaperEdgeRatio {
                            reference_edge: reference,
                        },
                    },
                )
                .expect("set removal reference");
            let before_remove = editor_state_snapshot(&remove_editor);
            let error = remove_editor
                .execute(
                    1,
                    Command::RemoveBoundaryVertex {
                        vertex: remove_paper.boundary_vertices[1],
                    },
                )
                .expect_err("adjacent reference removal must fail");
            assert_eq!(
                error,
                CommandError::LengthDisplayReferenceEdgeMutationBlocked { edge: reference }
            );
            assert_eq!(editor_state_snapshot(&remove_editor), before_remove);
        }
    }

    #[test]
    fn ratio_reference_is_direction_independent_and_non_reference_split_still_works() {
        let (_, mut pattern, paper) = simple_rectangular_editor();
        let original_reference = pattern.edges[0].clone();
        pattern.edges[0] = Edge {
            start: original_reference.end,
            end: original_reference.start,
            ..original_reference
        };
        let reference = pattern.edges[0].id;
        let split_target = pattern.edges[1].id;
        let new_vertex = VertexId::new();
        let new_edge = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::PaperEdgeRatio {
                        reference_edge: reference,
                    },
                },
            )
            .expect("reversed boundary reference");
        editor
            .execute(
                1,
                Command::SplitBoundaryEdge {
                    edge: split_target,
                    new_vertex,
                    new_edge,
                    fraction: 0.5,
                },
            )
            .expect("non-reference boundary split");

        assert_eq!(
            editor.paper().length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio {
                reference_edge: reference
            }
        );
        assert!(
            editor
                .pattern()
                .edges
                .iter()
                .any(|edge| edge.id == new_edge)
        );
        assert!(editor.paper().boundary_vertices.contains(&new_vertex));
    }

    #[test]
    fn boundary_t_junction_cannot_split_the_ratio_reference() {
        let (mut editor, _, _, boundary, stem, _) = boundary_t_junction_editor(0);
        editor
            .execute(
                0,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::PaperEdgeRatio {
                        reference_edge: boundary.id,
                    },
                },
            )
            .expect("set T-junction reference");
        let before = editor_state_snapshot(&editor);

        let error = editor
            .execute(
                1,
                Command::ConnectTJunction {
                    first_edge: boundary.id,
                    second_edge: stem.id,
                    new_edge: EdgeId::new(),
                },
            )
            .expect_err("T-junction must not split reference");

        assert_eq!(
            error,
            CommandError::LengthDisplayReferenceEdgeMutationBlocked { edge: boundary.id }
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn malformed_loaded_ratio_can_switch_to_millimetres_and_undo_exactly() {
        let missing_reference = EdgeId::new();
        let paper = Paper {
            length_display_unit: LengthDisplayUnit::PaperEdgeRatio {
                reference_edge: missing_reference,
            },
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(CreasePattern::empty(), paper);

        editor
            .execute(
                0,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::Millimeter,
                },
            )
            .expect("repair malformed display unit");
        assert_eq!(
            editor.paper().length_display_unit,
            LengthDisplayUnit::Millimeter
        );

        editor.undo(1).expect("undo repair exactly");
        assert_eq!(
            editor.paper().length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio {
                reference_edge: missing_reference
            }
        );
        editor.redo(2).expect("redo repair");
        assert_eq!(
            editor.paper().length_display_unit,
            LengthDisplayUnit::Millimeter
        );
    }

    #[test]
    fn rectangular_paper_resize_scales_every_vertex_and_restores_exactly() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_vertex_ids = original_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edges = original_pattern.edges.clone();
        assert!(crate::validate_paper(&original_paper, &original_pattern).is_valid());

        let result = editor
            .execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 200.0,
                    height_mm: 25.0,
                },
            )
            .expect("resize rectangular paper");

        assert_eq!(result.revision, 1);
        assert_eq!(result.changed_vertices, original_vertex_ids);
        assert!(result.changed_edges.is_empty());
        assert!(!result.settings_changed);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.pattern().edges, original_edges);
        assert_eq!(
            editor.pattern().vertices[0].position,
            Point2::new(110.0, 32.5)
        );
        assert_eq!(
            editor.pattern().vertices[1].position,
            Point2::new(10.0, 20.0)
        );
        assert_eq!(
            editor.pattern().vertices[2].position,
            Point2::new(-90.0, 57.5)
        );
        assert_eq!(
            editor.pattern().vertices[3].position,
            Point2::new(210.0, 45.0)
        );
        assert_eq!(
            editor.pattern().vertices[4].position,
            Point2::new(10.0, 45.0)
        );
        assert_eq!(
            editor.pattern().vertices[5].position,
            Point2::new(210.0, 20.0)
        );
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let resized_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo resize");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, original_vertex_ids);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo resize");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, original_vertex_ids);
        assert_eq!(editor.pattern(), &resized_pattern);
        assert_eq!(editor.paper(), &original_paper);

        editor.undo(3).expect("undo resize again");
        assert_eq!(editor.pattern(), &original_pattern);
        editor.redo(4).expect("redo resize again");
        assert_eq!(editor.pattern(), &resized_pattern);
    }

    #[test]
    fn resizing_to_the_same_dimensions_is_an_exact_undoable_command() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let changed_vertices = original_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();

        let result = editor
            .execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 100.0,
                    height_mm: 50.0,
                },
            )
            .expect("same-size resize");

        assert_eq!(result.revision, 1);
        assert_eq!(result.changed_vertices, changed_vertices);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert!(editor.can_undo());
        editor.undo(1).expect("undo same-size resize");
        assert_eq!(editor.pattern(), &original_pattern);
        editor.redo(2).expect("redo same-size resize");
        assert_eq!(editor.pattern(), &original_pattern);
    }

    #[test]
    fn stale_rectangular_resize_preserves_state_and_history() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();

        let error = editor
            .execute(
                7,
                Command::ResizeRectangularPaper {
                    width_mm: 200.0,
                    height_mm: 100.0,
                },
            )
            .expect_err("stale resize must fail");

        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 7,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn invalid_resize_dimensions_do_not_change_state_or_history() {
        let cases = [
            (f64::NAN, 50.0, CommandError::PaperWidthNotFinite),
            (f64::INFINITY, 50.0, CommandError::PaperWidthNotFinite),
            (0.0, 50.0, CommandError::PaperWidthNotPositive),
            (-1.0, 50.0, CommandError::PaperWidthNotPositive),
            (100.0, f64::NAN, CommandError::PaperHeightNotFinite),
            (100.0, 0.0, CommandError::PaperHeightNotPositive),
            (f64::MAX, 2.0, CommandError::PaperResizeAreaNotRepresentable),
            (
                f64::MIN_POSITIVE,
                f64::MIN_POSITIVE,
                CommandError::PaperResizeAreaNotRepresentable,
            ),
        ];

        for (width_mm, height_mm, expected) in cases {
            let (mut editor, original_pattern, original_paper) = rectangular_editor();
            let error = editor
                .execute(
                    0,
                    Command::ResizeRectangularPaper {
                        width_mm,
                        height_mm,
                    },
                )
                .expect_err("invalid resize must fail");

            assert_eq!(error, expected);
            assert_eq!(editor.pattern(), &original_pattern);
            assert_eq!(editor.paper(), &original_paper);
            assert_eq!(editor.revision(), 0);
            assert!(!editor.can_undo());
            assert!(!editor.can_redo());
        }
    }

    #[test]
    fn invalid_rectangular_boundaries_have_specific_errors_without_mutation() {
        let (_, original_pattern, original_paper) = rectangular_editor();
        let resize = Command::ResizeRectangularPaper {
            width_mm: 200.0,
            height_mm: 100.0,
        };

        let mut count_paper = original_paper.clone();
        count_paper.boundary_vertices.pop();
        let mut editor = EditorState::with_paper(original_pattern.clone(), count_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryVertexCount { actual: 3 })
        );

        let mut duplicate_paper = original_paper.clone();
        let duplicate = duplicate_paper.boundary_vertices[0];
        duplicate_paper.boundary_vertices[3] = duplicate;
        let mut editor = EditorState::with_paper(original_pattern.clone(), duplicate_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryDuplicateVertex { vertex: duplicate })
        );

        let mut missing_paper = original_paper.clone();
        let missing = VertexId::new();
        missing_paper.boundary_vertices[2] = missing;
        let mut editor = EditorState::with_paper(original_pattern.clone(), missing_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryVertexNotFound(
                missing
            ))
        );

        let mut non_finite_pattern = original_pattern.clone();
        let non_finite = original_paper.boundary_vertices[1];
        non_finite_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == non_finite)
            .expect("boundary vertex")
            .position
            .x = f64::NAN;
        let mut editor = EditorState::with_paper(non_finite_pattern, original_paper.clone());
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryPositionNotFinite { vertex: non_finite })
        );

        let mut non_rectangle_pattern = original_pattern.clone();
        let top_right = original_paper.boundary_vertices[2];
        non_rectangle_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == top_right)
            .expect("top right")
            .position
            .x = 100.0;
        let mut editor = EditorState::with_paper(non_rectangle_pattern, original_paper.clone());
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::PaperBoundaryNotRectangle)
        );

        let mut non_adjacent_paper = original_paper.clone();
        non_adjacent_paper.boundary_vertices.swap(1, 2);
        let mut editor = EditorState::with_paper(original_pattern.clone(), non_adjacent_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::PaperBoundaryVerticesNotAdjacent)
        );

        let diamond_ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let diamond_pattern = CreasePattern {
            vertices: diamond_ids
                .into_iter()
                .zip([
                    Point2::new(0.0, 1.0),
                    Point2::new(1.0, 2.0),
                    Point2::new(2.0, 1.0),
                    Point2::new(1.0, 0.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let diamond_paper = Paper {
            boundary_vertices: diamond_ids.to_vec(),
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(diamond_pattern, diamond_paper);
        assert_eq!(
            editor.execute(0, resize),
            Err(CommandError::PaperBoundaryNotAxisAligned)
        );
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn resize_rejects_unrepresentable_bounds_and_transformed_vertices_atomically() {
        let boundary_ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let anchored_pattern = CreasePattern {
            vertices: boundary_ids
                .into_iter()
                .zip([
                    Point2::new(1.0e308, 0.0),
                    Point2::new(1.1e308, 0.0),
                    Point2::new(1.1e308, 1.0),
                    Point2::new(1.0e308, 1.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let anchored_paper = Paper {
            boundary_vertices: boundary_ids.to_vec(),
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(anchored_pattern.clone(), anchored_paper.clone());
        assert_eq!(
            editor.execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 1.0,
                    height_mm: 1.0,
                }
            ),
            Err(CommandError::PaperResizeBoundaryNotRepresentable)
        );
        assert_eq!(editor.pattern(), &anchored_pattern);
        assert_eq!(editor.paper(), &anchored_paper);
        assert_eq!(editor.revision(), 0);

        let (_, mut pattern, paper) = rectangular_editor();
        pattern.vertices[2].position.x = f64::MAX;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let overflowing_vertex = pattern.vertices[2].id;
        assert_eq!(
            editor.execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 200.0,
                    height_mm: 50.0,
                }
            ),
            Err(CommandError::PaperResizeVertexPositionNotFinite {
                vertex: overflowing_vertex
            })
        );
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn edge_split_preserves_ids_kind_order_and_history_exactly() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_edge = original_pattern.edges[4].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();

        let result = editor
            .execute(
                0,
                Command::SplitEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.25,
                },
            )
            .expect("split crease edge");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(result.changed_edges, vec![original_edge.id, new_edge_id]);
        assert!(!result.settings_changed);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex_id,
                position: Point2::new(72.5, 51.25),
            })
        );
        assert_eq!(editor.pattern().edges[4].id, original_edge.id);
        assert_eq!(editor.pattern().edges[4].start, original_edge.start);
        assert_eq!(editor.pattern().edges[4].end, new_vertex_id);
        assert_eq!(editor.pattern().edges[4].kind, EdgeKind::Mountain);
        assert_eq!(
            editor.pattern().edges[5],
            Edge {
                id: new_edge_id,
                start: new_vertex_id,
                end: original_edge.end,
                kind: EdgeKind::Mountain,
            }
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let split_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo edge split");
        assert_eq!(undo.revision, 2);
        assert_eq!(
            undo.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(undo.changed_edges, vec![original_edge.id, new_edge_id]);
        assert!(!undo.settings_changed);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo edge split");
        assert_eq!(redo.revision, 3);
        assert_eq!(
            redo.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(redo.changed_edges, vec![original_edge.id, new_edge_id]);
        assert!(!redo.settings_changed);
        assert_eq!(editor.pattern(), &split_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn edge_split_preserves_reverse_orientation_and_each_non_boundary_kind() {
        for kind in [
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Auxiliary,
            EdgeKind::Cut,
        ] {
            let (_, mut pattern, paper) = rectangular_editor();
            let forward_edge = pattern.edges[4].clone();
            pattern.edges[4] = Edge {
                start: forward_edge.end,
                end: forward_edge.start,
                kind,
                ..forward_edge
            };
            let original_edge = pattern.edges[4].clone();
            let new_vertex = VertexId::new();
            let new_edge = EdgeId::new();
            let mut editor = EditorState::with_paper(pattern, paper.clone());

            editor
                .execute(
                    0,
                    Command::SplitEdge {
                        edge: original_edge.id,
                        new_vertex,
                        new_edge,
                        fraction: 0.25,
                    },
                )
                .expect("split reversed non-boundary edge");

            assert_eq!(
                editor
                    .pattern()
                    .vertices
                    .last()
                    .map(|vertex| vertex.position),
                Some(Point2::new(97.5, 63.75))
            );
            assert_eq!(
                editor.pattern().edges[4],
                Edge {
                    end: new_vertex,
                    ..original_edge.clone()
                }
            );
            assert_eq!(
                editor.pattern().edges[5],
                Edge {
                    id: new_edge,
                    start: new_vertex,
                    end: original_edge.end,
                    kind,
                }
            );
            assert_eq!(editor.paper(), &paper);
        }
    }

    #[test]
    fn edge_split_rejects_boundary_missing_and_ambiguous_targets_atomically() {
        let (_, pattern, paper) = rectangular_editor();
        let boundary_edge = pattern.edges[0].clone();
        let crease_edge = pattern.edges[4].clone();

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundaryEdgeRequiresSheetOperation(boundary_edge.id),
        );

        let missing_edge = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: missing_edge,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeNotFound(missing_edge),
        );

        let mut duplicate_pattern = pattern.clone();
        duplicate_pattern.edges.push(crease_edge.clone());
        let mut editor = EditorState::with_paper(duplicate_pattern, paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeSplitTargetEdgeIdAmbiguous {
                edge: crease_edge.id,
            },
        );
    }

    #[test]
    fn edge_split_rejects_non_unique_generated_ids_globally() {
        let (_, pattern, paper) = rectangular_editor();
        let crease_edge = pattern.edges[4].clone();
        let existing_vertex = pattern.vertices[0].id;
        let existing_edge = pattern.edges[1].id;

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: existing_vertex,
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::VertexAlreadyExists(existing_vertex),
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: existing_edge,
                fraction: 0.5,
            },
            CommandError::EdgeAlreadyExists(existing_edge),
        );

        let boundary_only_id = VertexId::new();
        let mut malformed_paper = paper.clone();
        malformed_paper.boundary_vertices.push(boundary_only_id);
        let mut editor = EditorState::with_paper(pattern.clone(), malformed_paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: boundary_only_id,
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::VertexAlreadyExists(boundary_only_id),
        );

        let endpoint_only_id = VertexId::new();
        let mut malformed_pattern = pattern;
        malformed_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: endpoint_only_id,
            end: crease_edge.start,
            kind: EdgeKind::Auxiliary,
        });
        let mut editor = EditorState::with_paper(malformed_pattern, paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: endpoint_only_id,
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::VertexAlreadyExists(endpoint_only_id),
        );
    }

    #[test]
    fn edge_split_rejects_invalid_fractions_positions_and_revisions_atomically() {
        let (_, pattern, paper) = rectangular_editor();
        let crease_edge = pattern.edges[4].clone();
        for (fraction, expected) in [
            (f64::NAN, CommandError::EdgeSplitFractionNotFinite),
            (f64::INFINITY, CommandError::EdgeSplitFractionNotFinite),
            (0.0, CommandError::EdgeSplitFractionOutOfRange),
            (-0.5, CommandError::EdgeSplitFractionOutOfRange),
            (1.0, CommandError::EdgeSplitFractionOutOfRange),
        ] {
            let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
            assert_split_rejected(
                &mut editor,
                Command::SplitEdge {
                    edge: crease_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction,
                },
                expected,
            );
        }

        let occupied_by = VertexId::new();
        let start = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == crease_edge.start)
            .expect("crease start")
            .position;
        let end = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == crease_edge.end)
            .expect("crease end")
            .position;
        let mut occupied_pattern = pattern.clone();
        occupied_pattern.vertices.push(Vertex {
            id: occupied_by,
            position: Point2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0),
        });
        let mut editor = EditorState::with_paper(occupied_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeSplitPositionOccupied {
                vertex: occupied_by,
            },
        );

        let mut non_finite_pattern = pattern.clone();
        non_finite_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == crease_edge.end)
            .expect("crease end")
            .position
            .y = f64::NEG_INFINITY;
        let mut editor = EditorState::with_paper(non_finite_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeSplitEndpointPositionNotFinite {
                edge: crease_edge.id,
                vertex: crease_edge.end,
            },
        );

        for endpoint in [crease_edge.start, crease_edge.end] {
            let mut ambiguous_pattern = pattern.clone();
            ambiguous_pattern.vertices.push(Vertex {
                id: endpoint,
                position: Point2::new(-123.0, 456.0),
            });
            let mut editor = EditorState::with_paper(ambiguous_pattern, paper.clone());
            assert_split_rejected(
                &mut editor,
                Command::SplitEdge {
                    edge: crease_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
                CommandError::EdgeSplitEndpointVertexRecordAmbiguous {
                    edge: crease_edge.id,
                    vertex: endpoint,
                },
            );
        }

        let mut missing_endpoint_pattern = pattern.clone();
        missing_endpoint_pattern
            .vertices
            .retain(|vertex| vertex.id != crease_edge.start);
        let mut editor = EditorState::with_paper(missing_endpoint_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: crease_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::VertexNotFound(crease_edge.start),
        );

        let close_ids = [VertexId::new(), VertexId::new()];
        let close_edge = Edge {
            id: EdgeId::new(),
            start: close_ids[0],
            end: close_ids[1],
            kind: EdgeKind::Valley,
        };
        let close_pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: close_ids[0],
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: close_ids[1],
                    position: Point2::new(2.0, 0.0),
                },
            ],
            edges: vec![close_edge.clone()],
        };
        let mut editor = EditorState::new(close_pattern);
        assert_split_rejected(
            &mut editor,
            Command::SplitEdge {
                edge: close_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: f64::MIN_POSITIVE,
            },
            CommandError::EdgeSplitPositionNotDistinct,
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let original_pattern = editor.pattern().clone();
        let original_paper = editor.paper().clone();
        let error = editor
            .execute(
                7,
                Command::SplitEdge {
                    edge: crease_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect_err("stale split must fail");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 7,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn edge_split_uses_stable_interpolation_for_extreme_finite_endpoints() {
        let ids = [VertexId::new(), VertexId::new()];
        let edge = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Mountain,
        };
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: ids[0],
                    position: Point2::new(-f64::MAX, 0.0),
                },
                Vertex {
                    id: ids[1],
                    position: Point2::new(f64::MAX, 2.0),
                },
            ],
            edges: vec![edge.clone()],
        };
        let new_vertex = VertexId::new();
        let mut editor = EditorState::new(pattern);

        editor
            .execute(
                0,
                Command::SplitEdge {
                    edge: edge.id,
                    new_vertex,
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect("split extreme finite edge");

        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex,
                position: Point2::new(0.0, 1.0),
            })
        );
    }

    #[test]
    fn edge_intersection_connection_is_atomic_ordered_and_exact_through_history() {
        let (_, mut original_pattern, original_paper, first, second) = crossing_edges_editor();
        let original_second_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == second.id)
            .expect("second crossing edge");
        let unrelated_edge = Edge {
            id: EdgeId::new(),
            start: first.start,
            end: second.start,
            kind: EdgeKind::Auxiliary,
        };
        original_pattern
            .edges
            .insert(original_second_index, unrelated_edge.clone());
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let first_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == first.id)
            .expect("first crossing edge");
        let second_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == second.id)
            .expect("second crossing edge");
        assert_eq!(second_index, first_index + 2);
        assert_eq!(original_pattern.edges[first_index + 1], unrelated_edge);
        assert!(!validate_crease_pattern(&original_pattern).is_valid());
        let new_vertex = VertexId::new();
        let first_new_edge = EdgeId::new();
        let second_new_edge = EdgeId::new();

        // Pass the targets in reverse vector order to verify that output order
        // remains tied to the document, while each generated ID remains tied
        // to its requested target edge.
        let result = editor
            .execute(
                0,
                Command::ConnectEdgeIntersection {
                    first_edge: second.id,
                    second_edge: first.id,
                    new_vertex,
                    first_new_edge,
                    second_new_edge,
                },
            )
            .expect("connect proper edge intersection");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![new_vertex, first.start, first.end, second.start, second.end]
        );
        assert_eq!(
            result.changed_edges,
            vec![first.id, second_new_edge, second.id, first_new_edge]
        );
        assert!(!result.settings_changed);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex,
                position: Point2::new(50.0, 50.0),
            })
        );
        assert_eq!(
            editor.pattern().edges[first_index],
            Edge {
                end: new_vertex,
                ..first.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[first_index + 1],
            Edge {
                id: second_new_edge,
                start: new_vertex,
                end: first.end,
                kind: first.kind,
            }
        );
        assert_eq!(
            editor.pattern().edges[second_index + 1],
            Edge {
                end: new_vertex,
                ..second.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[second_index + 2],
            Edge {
                id: first_new_edge,
                start: new_vertex,
                end: second.end,
                kind: second.kind,
            }
        );
        assert_eq!(editor.pattern().edges[first_index + 2], unrelated_edge);
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo intersection connection");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert!(!undo.settings_changed);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo intersection connection");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert!(!redo.settings_changed);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn edge_intersection_connection_handles_asymmetric_proper_fractions() {
        let (mut editor, horizontal, vertical) = two_edge_editor([
            Point2::new(0.0, 3.0),
            Point2::new(10.0, 3.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 10.0),
        ]);
        let new_vertex = VertexId::new();
        let horizontal_new = EdgeId::new();
        let vertical_new = EdgeId::new();

        editor
            .execute(
                0,
                Command::ConnectEdgeIntersection {
                    first_edge: horizontal.id,
                    second_edge: vertical.id,
                    new_vertex,
                    first_new_edge: horizontal_new,
                    second_new_edge: vertical_new,
                },
            )
            .expect("connect asymmetric proper intersection");

        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex,
                position: Point2::new(2.0, 3.0),
            })
        );
        assert_eq!(
            editor.pattern().edges,
            vec![
                Edge {
                    end: new_vertex,
                    ..horizontal.clone()
                },
                Edge {
                    id: horizontal_new,
                    start: new_vertex,
                    end: horizontal.end,
                    kind: EdgeKind::Mountain,
                },
                Edge {
                    end: new_vertex,
                    ..vertical.clone()
                },
                Edge {
                    id: vertical_new,
                    start: new_vertex,
                    end: vertical.end,
                    kind: EdgeKind::Valley,
                },
            ]
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
    }

    #[test]
    fn edge_intersection_connection_preserves_reverse_cut_and_auxiliary_edges() {
        let (_, mut pattern, mut paper, first, second) = crossing_edges_editor();
        paper.cutting_allowed = true;
        let first_index = pattern
            .edges
            .iter()
            .position(|edge| edge.id == first.id)
            .expect("first edge");
        let second_index = pattern
            .edges
            .iter()
            .position(|edge| edge.id == second.id)
            .expect("second edge");
        pattern.edges[first_index] = Edge {
            start: first.end,
            end: first.start,
            kind: EdgeKind::Cut,
            ..first
        };
        pattern.edges[second_index] = Edge {
            start: second.end,
            end: second.start,
            kind: EdgeKind::Auxiliary,
            ..second
        };
        let original_first = pattern.edges[first_index].clone();
        let original_second = pattern.edges[second_index].clone();
        let new_vertex = VertexId::new();
        let first_new = EdgeId::new();
        let second_new = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::ConnectEdgeIntersection {
                    first_edge: original_first.id,
                    second_edge: original_second.id,
                    new_vertex,
                    first_new_edge: first_new,
                    second_new_edge: second_new,
                },
            )
            .expect("connect reversed cut and auxiliary intersection");

        assert_eq!(
            editor.pattern().edges[first_index],
            Edge {
                end: new_vertex,
                ..original_first.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[first_index + 1],
            Edge {
                id: first_new,
                start: new_vertex,
                end: original_first.end,
                kind: EdgeKind::Cut,
            }
        );
        assert_eq!(
            editor.pattern().edges[second_index + 1],
            Edge {
                end: new_vertex,
                ..original_second.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[second_index + 2],
            Edge {
                id: second_new,
                start: new_vertex,
                end: original_second.end,
                kind: EdgeKind::Auxiliary,
            }
        );
    }

    #[test]
    fn edge_intersection_connection_rejects_target_and_generated_id_ambiguity_atomically() {
        let (_, pattern, paper, first, second) = crossing_edges_editor();
        let command = |first_edge, second_edge, new_vertex, first_new_edge, second_new_edge| {
            Command::ConnectEdgeIntersection {
                first_edge,
                second_edge,
                new_vertex,
                first_new_edge,
                second_new_edge,
            }
        };

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                first.id,
                VertexId::new(),
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::EdgeIntersectionTargetsNotDistinct,
        );

        let missing = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                missing,
                VertexId::new(),
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::EdgeNotFound(missing),
        );

        let mut ambiguous_pattern = pattern.clone();
        ambiguous_pattern.edges.push(second.clone());
        let mut editor = EditorState::with_paper(ambiguous_pattern, paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                VertexId::new(),
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::EdgeIntersectionTargetEdgeIdAmbiguous { edge: second.id },
        );

        let boundary = pattern.edges[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                boundary,
                first.id,
                VertexId::new(),
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::EdgeIntersectionBoundaryEdge(boundary),
        );

        let duplicate_new_edge = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                VertexId::new(),
                duplicate_new_edge,
                duplicate_new_edge,
            ),
            CommandError::EdgeIntersectionNewEdgeIdsNotDistinct,
        );

        let existing_vertex = pattern.vertices[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                existing_vertex,
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::VertexAlreadyExists(existing_vertex),
        );

        let boundary_reference_only = VertexId::new();
        let mut malformed_paper = paper.clone();
        malformed_paper
            .boundary_vertices
            .push(boundary_reference_only);
        let mut editor = EditorState::with_paper(pattern.clone(), malformed_paper);
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                boundary_reference_only,
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::VertexAlreadyExists(boundary_reference_only),
        );

        let endpoint_reference_only = VertexId::new();
        let mut malformed_pattern = pattern.clone();
        malformed_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: endpoint_reference_only,
            end: first.start,
            kind: EdgeKind::Auxiliary,
        });
        let mut editor = EditorState::with_paper(malformed_pattern, paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                endpoint_reference_only,
                EdgeId::new(),
                EdgeId::new(),
            ),
            CommandError::VertexAlreadyExists(endpoint_reference_only),
        );

        let existing_edge = pattern.edges[1].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                VertexId::new(),
                existing_edge,
                EdgeId::new(),
            ),
            CommandError::EdgeAlreadyExists(existing_edge),
        );

        let mut editor = EditorState::with_paper(pattern, paper);
        assert_intersection_rejected(
            &mut editor,
            command(
                first.id,
                second.id,
                VertexId::new(),
                EdgeId::new(),
                existing_edge,
            ),
            CommandError::EdgeAlreadyExists(existing_edge),
        );
    }

    #[test]
    fn edge_intersection_connection_rejects_non_proper_geometry_atomically() {
        let cases = [
            (
                [
                    Point2::new(0.0, 0.0),
                    Point2::new(2.0, 0.0),
                    Point2::new(0.0, 2.0),
                    Point2::new(2.0, 2.0),
                ],
                CommandError::EdgeIntersectionNotSinglePoint,
            ),
            (
                [
                    Point2::new(0.0, 0.0),
                    Point2::new(3.0, 0.0),
                    Point2::new(1.0, 0.0),
                    Point2::new(4.0, 0.0),
                ],
                CommandError::EdgeIntersectionNotSinglePoint,
            ),
            (
                [
                    Point2::new(0.0, 0.0),
                    Point2::new(4.0, 0.0),
                    Point2::new(2.0, 0.0),
                    Point2::new(2.0, 2.0),
                ],
                CommandError::EdgeIntersectionNotProper,
            ),
            (
                [
                    Point2::new(0.0, 0.0),
                    Point2::new(2.0, 2.0),
                    Point2::new(2.0, 2.0),
                    Point2::new(4.0, 0.0),
                ],
                CommandError::EdgeIntersectionNotProper,
            ),
        ];
        for (points, expected) in cases {
            let (mut editor, first, second) = two_edge_editor(points);
            assert_intersection_rejected(
                &mut editor,
                Command::ConnectEdgeIntersection {
                    first_edge: first.id,
                    second_edge: second.id,
                    new_vertex: VertexId::new(),
                    first_new_edge: EdgeId::new(),
                    second_new_edge: EdgeId::new(),
                },
                expected,
            );
        }
    }

    #[test]
    fn edge_intersection_connection_rejects_bad_data_occupied_points_and_unrepresentable_rounding()
    {
        let (_, pattern, paper, first, second) = crossing_edges_editor();
        let mut duplicate_endpoint = pattern.clone();
        duplicate_endpoint.vertices.push(
            duplicate_endpoint
                .vertices
                .iter()
                .find(|vertex| vertex.id == first.start)
                .expect("first start")
                .clone(),
        );
        let mut editor = EditorState::with_paper(duplicate_endpoint, paper.clone());
        assert_intersection_rejected(
            &mut editor,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            CommandError::EdgeIntersectionEndpointVertexRecordAmbiguous {
                edge: first.id,
                vertex: first.start,
            },
        );

        let mut missing_endpoint = pattern.clone();
        missing_endpoint
            .vertices
            .retain(|vertex| vertex.id != first.start);
        let mut editor = EditorState::with_paper(missing_endpoint, paper.clone());
        assert_intersection_rejected(
            &mut editor,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            CommandError::VertexNotFound(first.start),
        );

        let mut non_finite = pattern.clone();
        non_finite
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == second.end)
            .expect("second end")
            .position
            .x = f64::INFINITY;
        let mut editor = EditorState::with_paper(non_finite, paper.clone());
        assert_intersection_rejected(
            &mut editor,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            CommandError::EdgeIntersectionEndpointPositionNotFinite {
                edge: second.id,
                vertex: second.end,
            },
        );

        let occupied_by = VertexId::new();
        let mut occupied = pattern;
        occupied.vertices.push(Vertex {
            id: occupied_by,
            position: Point2::new(50.0, 50.0),
        });
        let mut editor = EditorState::with_paper(occupied, paper);
        assert_intersection_rejected(
            &mut editor,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            CommandError::EdgeIntersectionPositionOccupied {
                vertex: occupied_by,
            },
        );

        let minimum = f64::from_bits(1);
        let (mut editor, first, second) = two_edge_editor([
            Point2::new(0.0, 0.0),
            Point2::new(minimum, minimum),
            Point2::new(0.0, minimum),
            Point2::new(minimum, 0.0),
        ]);
        assert_intersection_rejected(
            &mut editor,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            CommandError::EdgeIntersectionGeometryNotRepresentable,
        );
    }

    #[test]
    fn edge_intersection_connection_handles_extreme_finite_coordinates_exactly() {
        let (mut editor, first, second) = two_edge_editor([
            Point2::new(-f64::MAX, -1.0),
            Point2::new(f64::MAX, 1.0),
            Point2::new(-f64::MAX, 1.0),
            Point2::new(f64::MAX, -1.0),
        ]);
        let original = editor.pattern().clone();
        let junction = VertexId::new();

        editor
            .execute(
                0,
                Command::ConnectEdgeIntersection {
                    first_edge: first.id,
                    second_edge: second.id,
                    new_vertex: junction,
                    first_new_edge: EdgeId::new(),
                    second_new_edge: EdgeId::new(),
                },
            )
            .expect("exact predicates keep the finite crossing representable");

        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == junction)
                .expect("created extreme-coordinate junction")
                .position,
            Point2::new(0.0, 0.0)
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        editor.undo(1).expect("undo extreme-coordinate crossing");
        assert_eq!(editor.pattern(), &original);
    }

    #[test]
    fn stale_edge_intersection_connection_preserves_state_and_history() {
        let (mut editor, pattern, paper, first, second) = crossing_edges_editor();
        let error = editor
            .execute(
                9,
                Command::ConnectEdgeIntersection {
                    first_edge: first.id,
                    second_edge: second.id,
                    new_vertex: VertexId::new(),
                    first_new_edge: EdgeId::new(),
                    second_new_edge: EdgeId::new(),
                },
            )
            .expect_err("stale intersection connection must fail");

        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 9,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn create_intersection_cluster_is_canonical_atomic_and_exact_through_history() {
        let (original_pattern, original_paper, [horizontal, vertical, diagonal], unrelated) =
            three_way_create_cluster();
        assert!(!validate_crease_pattern(&original_pattern).is_valid());
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let junction = VertexId::new();
        let horizontal_new = EdgeId::new();
        let vertical_new = EdgeId::new();
        let diagonal_new = EdgeId::new();

        let result = editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets: vec![
                        IntersectionEdgeTarget {
                            edge: horizontal.id,
                            new_edge: Some(horizontal_new),
                        },
                        IntersectionEdgeTarget {
                            edge: diagonal.id,
                            new_edge: Some(diagonal_new),
                        },
                        IntersectionEdgeTarget {
                            edge: vertical.id,
                            new_edge: Some(vertical_new),
                        },
                    ],
                },
            )
            .expect("connect three strict-interior edges");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![
                horizontal.start,
                vertical.end,
                diagonal.start,
                horizontal.end,
                vertical.start,
                diagonal.end,
                junction,
            ]
        );
        assert_eq!(
            result.changed_edges,
            vec![
                vertical.id,
                vertical_new,
                horizontal.id,
                horizontal_new,
                diagonal.id,
                diagonal_new,
            ]
        );
        assert!(!result.settings_changed);
        assert_eq!(
            editor.pattern().vertices.len(),
            original_pattern.vertices.len() + 1
        );
        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: junction,
                position: Point2::new(0.0, 0.0),
            })
        );
        assert_eq!(
            editor.pattern().edges,
            vec![
                Edge {
                    end: junction,
                    ..vertical.clone()
                },
                Edge {
                    id: vertical_new,
                    start: junction,
                    end: vertical.end,
                    kind: vertical.kind,
                },
                unrelated,
                Edge {
                    end: junction,
                    ..horizontal.clone()
                },
                Edge {
                    id: horizontal_new,
                    start: junction,
                    end: horizontal.end,
                    kind: horizontal.kind,
                },
                Edge {
                    end: junction,
                    ..diagonal.clone()
                },
                Edge {
                    id: diagonal_new,
                    start: junction,
                    end: diagonal.end,
                    kind: diagonal.kind,
                },
            ]
        );
        assert_eq!(editor.paper(), &original_paper);
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo intersection cluster");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert!(!undo.settings_changed);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo intersection cluster");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert!(!redo.settings_changed);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn maximum_size_create_intersection_cluster_is_exact_through_history() {
        let (original_pattern, source_edges) = maximum_size_create_cluster();
        assert_eq!(source_edges.len(), MAX_INTERSECTION_CLUSTER_TARGETS);
        let original_paper = Paper::default();
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let junction = VertexId::new();
        let targets = source_edges
            .iter()
            .map(|edge| IntersectionEdgeTarget {
                edge: edge.id,
                new_edge: Some(EdgeId::new()),
            })
            .collect();

        let result = editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets,
                },
            )
            .expect("the inclusive 64-edge cluster limit must connect");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices.len(),
            MAX_INTERSECTION_CLUSTER_TARGETS * 2 + 1
        );
        assert_eq!(
            result.changed_edges.len(),
            MAX_INTERSECTION_CLUSTER_TARGETS * 2
        );
        assert!(!result.settings_changed);
        assert_eq!(
            editor.pattern().vertices.len(),
            original_pattern.vertices.len() + 1
        );
        assert_eq!(
            editor.pattern().edges.len(),
            original_pattern.edges.len() + MAX_INTERSECTION_CLUSTER_TARGETS
        );
        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: junction,
                position: Point2::new(0.0, 0.0),
            })
        );
        for source in &source_edges {
            let split_original = editor
                .pattern()
                .edges
                .iter()
                .find(|edge| edge.id == source.id)
                .expect("each original edge remains at the maximum cluster");
            assert_eq!(split_original.start, source.start);
            assert_eq!(split_original.end, junction);
            assert_eq!(split_original.kind, source.kind);
            let generated = editor
                .pattern()
                .edges
                .iter()
                .find(|edge| {
                    !source_edges.iter().any(|source| source.id == edge.id)
                        && edge.start == junction
                        && edge.end == source.end
                })
                .expect("each maximum-cluster source gets one generated half");
            assert_eq!(generated.kind, source.kind);
        }
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        assert_eq!(editor.paper(), &original_paper);
        let connected_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo maximum intersection cluster");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo maximum intersection cluster");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn create_intersection_cluster_is_identical_for_all_target_permutations() {
        let (pattern, paper, [horizontal, vertical, diagonal], _) = three_way_create_cluster();
        let junction = VertexId::new();
        let targets = [
            IntersectionEdgeTarget {
                edge: horizontal.id,
                new_edge: Some(EdgeId::new()),
            },
            IntersectionEdgeTarget {
                edge: vertical.id,
                new_edge: Some(EdgeId::new()),
            },
            IntersectionEdgeTarget {
                edge: diagonal.id,
                new_edge: Some(EdgeId::new()),
            },
        ];
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut expected = None;

        for permutation in permutations {
            let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
            let result = editor
                .execute(
                    0,
                    Command::ConnectIntersectionCluster {
                        junction: JunctionVertexIntent::Create { id: junction },
                        targets: permutation
                            .into_iter()
                            .map(|index| targets[index])
                            .collect(),
                    },
                )
                .expect("each target permutation must connect");
            if let Some((expected_pattern, expected_result)) = &expected {
                assert_eq!(editor.pattern(), expected_pattern);
                assert_eq!(&result, expected_result);
            } else {
                expected = Some((editor.pattern().clone(), result));
            }
        }
    }

    #[test]
    fn reuse_intersection_cluster_mixes_endpoint_and_interior_targets_exactly() {
        let (
            original_pattern,
            original_paper,
            junction,
            [diagonal, endpoint_start, horizontal, endpoint_end],
            unrelated,
        ) = mixed_reuse_cluster();
        assert!(!validate_crease_pattern(&original_pattern).is_valid());
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let diagonal_new = EdgeId::new();
        let horizontal_new = EdgeId::new();

        let result = editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Reuse { id: junction },
                    targets: vec![
                        IntersectionEdgeTarget {
                            edge: endpoint_end.id,
                            new_edge: None,
                        },
                        IntersectionEdgeTarget {
                            edge: horizontal.id,
                            new_edge: Some(horizontal_new),
                        },
                        IntersectionEdgeTarget {
                            edge: diagonal.id,
                            new_edge: Some(diagonal_new),
                        },
                        IntersectionEdgeTarget {
                            edge: endpoint_start.id,
                            new_edge: None,
                        },
                    ],
                },
            )
            .expect("reuse the unique existing junction");

        assert_eq!(editor.pattern().vertices, original_pattern.vertices);
        assert_eq!(
            result.changed_vertices,
            vec![
                diagonal.start,
                endpoint_start.end,
                junction,
                horizontal.end,
                endpoint_end.start,
                diagonal.end,
                horizontal.start,
            ]
        );
        assert_eq!(
            result.changed_edges,
            vec![
                diagonal.id,
                diagonal_new,
                endpoint_start.id,
                horizontal.id,
                horizontal_new,
                endpoint_end.id,
            ]
        );
        assert!(!result.settings_changed);
        assert_eq!(
            editor.pattern().edges,
            vec![
                Edge {
                    end: junction,
                    ..diagonal.clone()
                },
                Edge {
                    id: diagonal_new,
                    start: junction,
                    end: diagonal.end,
                    kind: diagonal.kind,
                },
                endpoint_start,
                unrelated,
                Edge {
                    end: junction,
                    ..horizontal.clone()
                },
                Edge {
                    id: horizontal_new,
                    start: junction,
                    end: horizontal.end,
                    kind: horizontal.kind,
                },
                endpoint_end,
            ]
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo reused intersection cluster");
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        let redo = editor.redo(2).expect("redo reused intersection cluster");
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn intersection_cluster_rejects_target_and_generated_id_errors_atomically() {
        let (pattern, paper, [horizontal, vertical, diagonal], unrelated) =
            three_way_create_cluster();
        let junction = VertexId::new();
        let command = |targets| Command::ConnectIntersectionCluster {
            junction: JunctionVertexIntent::Create { id: junction },
            targets,
        };
        let target = |edge| IntersectionEdgeTarget {
            edge,
            new_edge: Some(EdgeId::new()),
        };

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![target(horizontal.id), target(vertical.id)]),
            CommandError::IntersectionClusterNeedsThreeTargets { actual: 2 },
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                target(horizontal.id);
                MAX_INTERSECTION_CLUSTER_TARGETS + 1
            ]),
            CommandError::IntersectionClusterTooManyTargets {
                actual: MAX_INTERSECTION_CLUSTER_TARGETS + 1,
                maximum: MAX_INTERSECTION_CLUSTER_TARGETS,
            },
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                target(horizontal.id),
                target(vertical.id),
                target(horizontal.id),
            ]),
            CommandError::IntersectionClusterTargetDuplicate {
                edge: horizontal.id,
            },
        );

        let missing = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                target(horizontal.id),
                target(vertical.id),
                target(missing),
            ]),
            CommandError::EdgeNotFound(missing),
        );

        let mut ambiguous_pattern = pattern.clone();
        ambiguous_pattern.edges.push(horizontal.clone());
        let mut editor = EditorState::with_paper(ambiguous_pattern, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                target(horizontal.id),
                target(vertical.id),
                target(diagonal.id),
            ]),
            CommandError::IntersectionClusterTargetEdgeIdAmbiguous {
                edge: horizontal.id,
            },
        );

        let mut boundary_pattern = pattern.clone();
        boundary_pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == horizontal.id)
            .expect("horizontal edge")
            .kind = EdgeKind::Boundary;
        let mut editor = EditorState::with_paper(boundary_pattern, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                target(horizontal.id),
                target(vertical.id),
                target(diagonal.id),
            ]),
            CommandError::IntersectionClusterBoundaryEdge(horizontal.id),
        );

        let duplicate_new = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                IntersectionEdgeTarget {
                    edge: horizontal.id,
                    new_edge: Some(duplicate_new),
                },
                IntersectionEdgeTarget {
                    edge: vertical.id,
                    new_edge: Some(duplicate_new),
                },
                target(diagonal.id),
            ]),
            CommandError::IntersectionClusterGeneratedEdgeIdDuplicate {
                new_edge: duplicate_new,
            },
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(vec![
                IntersectionEdgeTarget {
                    edge: horizontal.id,
                    new_edge: Some(unrelated.id),
                },
                target(vertical.id),
                target(diagonal.id),
            ]),
            CommandError::EdgeAlreadyExists(unrelated.id),
        );

        let existing_vertex = pattern.vertices[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper);
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create {
                    id: existing_vertex,
                },
                targets: vec![
                    target(horizontal.id),
                    target(vertical.id),
                    target(diagonal.id),
                ],
            },
            CommandError::VertexAlreadyExists(existing_vertex),
        );
    }

    #[test]
    fn intersection_cluster_rejects_invalid_endpoint_and_junction_records_atomically() {
        let (pattern, paper, [horizontal, vertical, diagonal], _) = three_way_create_cluster();
        let junction = VertexId::new();
        let targets = || {
            vec![
                IntersectionEdgeTarget {
                    edge: horizontal.id,
                    new_edge: Some(EdgeId::new()),
                },
                IntersectionEdgeTarget {
                    edge: vertical.id,
                    new_edge: Some(EdgeId::new()),
                },
                IntersectionEdgeTarget {
                    edge: diagonal.id,
                    new_edge: Some(EdgeId::new()),
                },
            ]
        };
        let command = |junction, targets| Command::ConnectIntersectionCluster { junction, targets };

        let mut missing_endpoint = pattern.clone();
        missing_endpoint
            .vertices
            .retain(|vertex| vertex.id != horizontal.start);
        let mut editor = EditorState::with_paper(missing_endpoint, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(JunctionVertexIntent::Create { id: junction }, targets()),
            CommandError::VertexNotFound(horizontal.start),
        );

        let mut duplicate_endpoint = pattern.clone();
        duplicate_endpoint.vertices.push(
            duplicate_endpoint
                .vertices
                .iter()
                .find(|vertex| vertex.id == horizontal.start)
                .expect("horizontal start")
                .clone(),
        );
        let mut editor = EditorState::with_paper(duplicate_endpoint, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(JunctionVertexIntent::Create { id: junction }, targets()),
            CommandError::IntersectionClusterEndpointVertexRecordAmbiguous {
                edge: horizontal.id,
                vertex: horizontal.start,
            },
        );

        let mut non_finite_endpoint = pattern.clone();
        non_finite_endpoint
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == horizontal.start)
            .expect("horizontal start")
            .position
            .x = f64::INFINITY;
        let mut editor = EditorState::with_paper(non_finite_endpoint, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(JunctionVertexIntent::Create { id: junction }, targets()),
            CommandError::IntersectionClusterEndpointPositionNotFinite {
                edge: horizontal.id,
                vertex: horizontal.start,
            },
        );

        let horizontal_start_position = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == horizontal.start)
            .expect("horizontal start")
            .position;
        let mut zero_length = pattern.clone();
        zero_length
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == horizontal.end)
            .expect("horizontal end")
            .position = horizontal_start_position;
        let mut editor = EditorState::with_paper(zero_length, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(JunctionVertexIntent::Create { id: junction }, targets()),
            CommandError::IntersectionClusterZeroLengthEdge {
                edge: horizontal.id,
            },
        );

        let occupied_by = VertexId::new();
        let mut occupied = pattern.clone();
        occupied.vertices.push(Vertex {
            id: occupied_by,
            position: Point2::new(0.0, 0.0),
        });
        let mut editor = EditorState::with_paper(occupied, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(JunctionVertexIntent::Create { id: junction }, targets()),
            CommandError::IntersectionClusterJunctionPositionOccupied {
                vertex: occupied_by,
            },
        );

        let mut ambiguous_position = pattern.clone();
        ambiguous_position.vertices.extend([
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
        ]);
        let mut editor = EditorState::with_paper(ambiguous_position, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(JunctionVertexIntent::Create { id: junction }, targets()),
            CommandError::IntersectionClusterJunctionPositionAmbiguous,
        );

        let missing_junction = VertexId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(
                JunctionVertexIntent::Reuse {
                    id: missing_junction,
                },
                targets(),
            ),
            CommandError::VertexNotFound(missing_junction),
        );

        let ambiguous_junction = VertexId::new();
        let mut duplicate_junction = pattern.clone();
        duplicate_junction.vertices.extend([
            Vertex {
                id: ambiguous_junction,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: ambiguous_junction,
                position: Point2::new(0.0, 0.0),
            },
        ]);
        let mut editor = EditorState::with_paper(duplicate_junction, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            command(
                JunctionVertexIntent::Reuse {
                    id: ambiguous_junction,
                },
                targets(),
            ),
            CommandError::IntersectionClusterJunctionVertexRecordAmbiguous {
                vertex: ambiguous_junction,
            },
        );

        let non_finite_junction = VertexId::new();
        let mut non_finite = pattern;
        non_finite.vertices.push(Vertex {
            id: non_finite_junction,
            position: Point2::new(f64::INFINITY, 0.0),
        });
        let mut editor = EditorState::with_paper(non_finite, paper);
        assert_cluster_rejected(
            &mut editor,
            command(
                JunctionVertexIntent::Reuse {
                    id: non_finite_junction,
                },
                targets(),
            ),
            CommandError::IntersectionClusterJunctionPositionNotFinite {
                vertex: non_finite_junction,
            },
        );
    }

    #[test]
    fn intersection_cluster_rejects_misclassification_and_incomplete_sets_atomically() {
        let (pattern, paper, [horizontal, vertical, diagonal], _) = three_way_create_cluster();
        let junction = VertexId::new();
        let create_target = |edge| IntersectionEdgeTarget {
            edge,
            new_edge: Some(EdgeId::new()),
        };

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: junction },
                targets: vec![
                    IntersectionEdgeTarget {
                        edge: horizontal.id,
                        new_edge: None,
                    },
                    create_target(vertical.id),
                    create_target(diagonal.id),
                ],
            },
            CommandError::IntersectionClusterNewEdgeRequired {
                edge: horizontal.id,
            },
        );

        let (
            reuse_pattern,
            reuse_paper,
            reused,
            [
                reuse_diagonal,
                endpoint_start,
                reuse_horizontal,
                endpoint_end,
            ],
            _,
        ) = mixed_reuse_cluster();
        let valid_reuse_targets = || {
            vec![
                IntersectionEdgeTarget {
                    edge: reuse_diagonal.id,
                    new_edge: Some(EdgeId::new()),
                },
                IntersectionEdgeTarget {
                    edge: endpoint_start.id,
                    new_edge: None,
                },
                IntersectionEdgeTarget {
                    edge: reuse_horizontal.id,
                    new_edge: Some(EdgeId::new()),
                },
                IntersectionEdgeTarget {
                    edge: endpoint_end.id,
                    new_edge: None,
                },
            ]
        };
        let mut unexpected_new_targets = valid_reuse_targets();
        unexpected_new_targets[1].new_edge = Some(EdgeId::new());
        let mut editor = EditorState::with_paper(reuse_pattern.clone(), reuse_paper.clone());
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Reuse { id: reused },
                targets: unexpected_new_targets,
            },
            CommandError::IntersectionClusterNewEdgeUnexpected {
                edge: endpoint_start.id,
            },
        );

        let mut missing_new_targets = valid_reuse_targets();
        missing_new_targets[0].new_edge = None;
        let mut editor = EditorState::with_paper(reuse_pattern.clone(), reuse_paper.clone());
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Reuse { id: reused },
                targets: missing_new_targets,
            },
            CommandError::IntersectionClusterNewEdgeRequired {
                edge: reuse_diagonal.id,
            },
        );

        let spoke_points = [
            Point2::new(10.0, 0.0),
            Point2::new(0.0, 10.0),
            Point2::new(-10.0, -10.0),
        ];
        let mut spoke_pattern = CreasePattern {
            vertices: vec![Vertex {
                id: reused,
                position: Point2::new(0.0, 0.0),
            }],
            edges: Vec::new(),
        };
        for position in spoke_points {
            let endpoint = VertexId::new();
            spoke_pattern.vertices.push(Vertex {
                id: endpoint,
                position,
            });
            spoke_pattern.edges.push(Edge {
                id: EdgeId::new(),
                start: reused,
                end: endpoint,
                kind: EdgeKind::Mountain,
            });
        }
        let spoke_targets = spoke_pattern
            .edges
            .iter()
            .map(|edge| IntersectionEdgeTarget {
                edge: edge.id,
                new_edge: None,
            })
            .collect();
        let mut editor = EditorState::new(spoke_pattern);
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Reuse { id: reused },
                targets: spoke_targets,
            },
            CommandError::IntersectionClusterNeedsSplit,
        );

        let extra_start = VertexId::new();
        let extra_end = VertexId::new();
        let extra_edge = Edge {
            id: EdgeId::new(),
            start: extra_start,
            end: extra_end,
            kind: EdgeKind::Auxiliary,
        };
        let mut incomplete = pattern.clone();
        incomplete.vertices.extend([
            Vertex {
                id: extra_start,
                position: Point2::new(-10.0, 10.0),
            },
            Vertex {
                id: extra_end,
                position: Point2::new(10.0, -10.0),
            },
        ]);
        incomplete.edges.push(extra_edge.clone());
        let valid_create_targets = || {
            vec![
                create_target(horizontal.id),
                create_target(vertical.id),
                create_target(diagonal.id),
            ]
        };
        let mut editor = EditorState::with_paper(incomplete.clone(), paper.clone());
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: junction },
                targets: valid_create_targets(),
            },
            CommandError::IncompleteIntersectionCluster {
                edge: extra_edge.id,
            },
        );

        incomplete.edges.last_mut().expect("extra edge").kind = EdgeKind::Boundary;
        let mut editor = EditorState::with_paper(incomplete, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: junction },
                targets: valid_create_targets(),
            },
            CommandError::IntersectionClusterBoundaryEdge(extra_edge.id),
        );

        let mut different_intersection = pattern.clone();
        different_intersection
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == diagonal.start)
            .expect("diagonal start")
            .position = Point2::new(-10.0, -9.0);
        different_intersection
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == diagonal.end)
            .expect("diagonal end")
            .position = Point2::new(10.0, 11.0);
        let mut editor = EditorState::with_paper(different_intersection, paper.clone());
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: junction },
                targets: valid_create_targets(),
            },
            CommandError::IntersectionClusterDifferentIntersection { edge: diagonal.id },
        );

        let mut overlap = reuse_pattern;
        overlap
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == endpoint_end.start)
            .expect("auxiliary start")
            .position = Point2::new(0.0, 5.0);
        let mut editor = EditorState::with_paper(overlap, reuse_paper);
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Reuse { id: reused },
                targets: valid_reuse_targets(),
            },
            CommandError::IntersectionClusterCollinearOverlap {
                first_edge: endpoint_start.id,
                second_edge: endpoint_end.id,
            },
        );
    }

    #[test]
    fn isolated_vertex_can_be_reused_for_an_all_interior_cluster() {
        let (mut pattern, paper, [horizontal, vertical, diagonal], _) = three_way_create_cluster();
        let junction = VertexId::new();
        pattern.vertices.push(Vertex {
            id: junction,
            position: Point2::new(0.0, 0.0),
        });
        let original_pattern = pattern.clone();
        let mut editor = EditorState::with_paper(pattern, paper.clone());

        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Reuse { id: junction },
                    targets: [horizontal, vertical, diagonal]
                        .into_iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("reuse isolated vertex");

        assert_eq!(editor.pattern().vertices, original_pattern.vertices);
        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .filter(|vertex| vertex.id == junction)
                .count(),
            1
        );
        editor.undo(1).expect("undo isolated reuse");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &paper);
    }

    #[test]
    fn intersection_cluster_handles_extreme_geometry_and_rejects_unrepresentable_rounding() {
        let center = 1.0e150;
        let radius = 1.0e140;
        let points = [
            Point2::new(center - radius, center),
            Point2::new(center + radius, center),
            Point2::new(center, center - radius),
            Point2::new(center, center + radius),
            Point2::new(center - radius, center - radius),
            Point2::new(center + radius, center + radius),
        ];
        let ids = points.map(|_| VertexId::new());
        let vertices = ids
            .into_iter()
            .zip(points)
            .map(|(id, position)| Vertex { id, position })
            .collect::<Vec<_>>();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[2],
                end: ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: ids[4],
                end: ids[5],
                kind: EdgeKind::Auxiliary,
            },
        ];
        let mut editor = EditorState::new(CreasePattern {
            vertices,
            edges: edges.to_vec(),
        });
        let junction = VertexId::new();
        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets: edges
                        .iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("large finite cluster must remain representable");
        let created = editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == junction)
            .expect("created large junction");
        assert_eq!(created.position, Point2::new(center, center));

        let overflow_points = [
            Point2::new(-f64::MAX, 0.0),
            Point2::new(f64::MAX, 0.0),
            Point2::new(0.0, -1.0),
            Point2::new(0.0, 1.0),
            Point2::new(-1.0, -1.0),
            Point2::new(1.0, 1.0),
        ];
        let overflow_ids = overflow_points.map(|_| VertexId::new());
        let overflow_vertices = overflow_ids
            .into_iter()
            .zip(overflow_points)
            .map(|(id, position)| Vertex { id, position })
            .collect::<Vec<_>>();
        let overflow_edges = [
            Edge {
                id: EdgeId::new(),
                start: overflow_ids[0],
                end: overflow_ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: overflow_ids[2],
                end: overflow_ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: overflow_ids[4],
                end: overflow_ids[5],
                kind: EdgeKind::Auxiliary,
            },
        ];
        let mut editor = EditorState::new(CreasePattern {
            vertices: overflow_vertices,
            edges: overflow_edges.to_vec(),
        });
        let extreme_original = editor.pattern().clone();
        let extreme_junction = VertexId::new();
        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create {
                        id: extreme_junction,
                    },
                    targets: overflow_edges
                        .iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("extreme finite cluster has an exactly representable junction");
        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == extreme_junction)
                .expect("created extreme cluster junction")
                .position,
            Point2::new(0.0, 0.0)
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        editor.undo(1).expect("undo extreme cluster");
        assert_eq!(editor.pattern(), &extreme_original);

        let minimum = f64::from_bits(1);
        let unrepresentable_points = [
            Point2::new(0.0, 0.0),
            Point2::new(minimum, minimum),
            Point2::new(0.0, minimum),
            Point2::new(minimum, 0.0),
            Point2::new(-minimum, 0.0),
            Point2::new(2.0 * minimum, minimum),
        ];
        let unrepresentable_ids = unrepresentable_points.map(|_| VertexId::new());
        let unrepresentable_vertices = unrepresentable_ids
            .into_iter()
            .zip(unrepresentable_points)
            .map(|(id, position)| Vertex { id, position })
            .collect::<Vec<_>>();
        let unrepresentable_edges = [
            Edge {
                id: EdgeId::new(),
                start: unrepresentable_ids[0],
                end: unrepresentable_ids[1],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: unrepresentable_ids[2],
                end: unrepresentable_ids[3],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: unrepresentable_ids[4],
                end: unrepresentable_ids[5],
                kind: EdgeKind::Auxiliary,
            },
        ];
        let mut editor = EditorState::new(CreasePattern {
            vertices: unrepresentable_vertices,
            edges: unrepresentable_edges.to_vec(),
        });
        assert_cluster_rejected(
            &mut editor,
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create {
                    id: VertexId::new(),
                },
                targets: unrepresentable_edges
                    .iter()
                    .map(|edge| IntersectionEdgeTarget {
                        edge: edge.id,
                        new_edge: Some(EdgeId::new()),
                    })
                    .collect(),
            },
            CommandError::IntersectionClusterGeometryNotRepresentable,
        );
    }

    #[test]
    fn create_cluster_chooses_the_first_exact_pair_candidate_and_restores_exactly() {
        let (original_pattern, edges) = intersection_candidate_authority_fixture();
        let position = |vertex_id| {
            original_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == vertex_id)
                .expect("authority fixture endpoint")
                .position
        };
        let pair_intersection = |first: &Edge, second: &Edge| match segment_intersection(
            position(first.start),
            position(first.end),
            position(second.start),
            position(second.end),
        )
        .expect("finite authority fixture")
        {
            SegmentIntersection::Point(point) => point,
            intersection => panic!("expected point intersection, got {intersection:?}"),
        };
        let pair_candidate = |first: &Edge, second: &Edge| {
            intersection_cluster_pair_candidate(
                position(first.start),
                position(first.end),
                position(second.start),
                position(second.end),
            )
            .expect("finite planner pair candidate")
        };
        let first_pair_position = pair_intersection(&edges[0], &edges[1]);
        assert_eq!(pair_candidate(&edges[0], &edges[1]), first_pair_position);
        assert_eq!(pair_candidate(&edges[1], &edges[0]), first_pair_position);
        for edge in &edges {
            assert_eq!(
                intersection_cluster_point_segment_relation(
                    first_pair_position,
                    position(edge.start),
                    position(edge.end),
                ),
                Ok(PointSegmentRelation::StrictInterior)
            );
        }

        let original_paper = Paper::default();
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let junction = VertexId::new();
        let result = editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets: [edges[2].id, edges[1].id, edges[0].id]
                        .into_iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("the first document-order pair supplies the canonical junction");

        assert_eq!(result.revision, 1);
        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == junction)
                .expect("created authority fixture junction")
                .position,
            first_pair_position
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo authority fixture cluster");
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo authority fixture cluster");
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn create_cluster_uses_exact_pair_candidate_and_restores_exactly() {
        let (original_pattern, edges) = fma_intersection_candidate_fixture();
        let position = |vertex_id| {
            original_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == vertex_id)
                .expect("FMA fixture endpoint")
                .position
        };
        let candidate_is_strict = |candidate| {
            edges.iter().all(|edge| {
                intersection_cluster_point_segment_relation(
                    candidate,
                    position(edge.start),
                    position(edge.end),
                ) == Ok(PointSegmentRelation::StrictInterior)
            })
        };

        let mut expected_position = None;
        for first_index in 0..edges.len() {
            for second in &edges[first_index + 1..] {
                let public_candidate = match segment_intersection(
                    position(edges[first_index].start),
                    position(edges[first_index].end),
                    position(second.start),
                    position(second.end),
                )
                .expect("finite FMA fixture")
                {
                    SegmentIntersection::Point(point) => point,
                    intersection => {
                        panic!("expected public point intersection, got {intersection:?}")
                    }
                };
                if expected_position.is_none() && candidate_is_strict(public_candidate) {
                    expected_position = Some(public_candidate);
                }
                assert_eq!(
                    intersection_cluster_pair_candidate(
                        position(edges[first_index].start),
                        position(edges[first_index].end),
                        position(second.start),
                        position(second.end),
                    ),
                    Some(public_candidate)
                );
                assert_eq!(
                    intersection_cluster_pair_candidate(
                        position(second.end),
                        position(second.start),
                        position(edges[first_index].end),
                        position(edges[first_index].start),
                    ),
                    Some(public_candidate)
                );
            }
        }
        let expected_position =
            expected_position.expect("one canonical exact pair candidate contains every target");

        let original_paper = Paper::default();
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let junction = VertexId::new();
        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets: edges
                        .iter()
                        .rev()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("the exact pair candidate must connect the cancellation fixture");

        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == junction)
                .expect("created FMA fixture junction")
                .position,
            expected_position
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();

        editor.undo(1).expect("undo FMA fixture cluster");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(2).expect("redo FMA fixture cluster");
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn create_cluster_candidate_is_direction_invariant_and_deterministic() {
        let (original_pattern, edges) = reverse_only_intersection_candidate_fixture();
        let position = |vertex_id| {
            original_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == vertex_id)
                .expect("reverse-only fixture endpoint")
                .position
        };
        let candidate = |first: &Edge, second: &Edge| {
            intersection_cluster_pair_candidate(
                position(first.start),
                position(first.end),
                position(second.start),
                position(second.end),
            )
            .expect("finite reverse-only planner candidate")
        };
        let is_strict = |point| {
            edges.iter().all(|edge| {
                intersection_cluster_point_segment_relation(
                    point,
                    position(edge.start),
                    position(edge.end),
                ) == Ok(PointSegmentRelation::StrictInterior)
            })
        };

        let mut expected_position = None;
        for first_index in 0..edges.len() {
            for second in &edges[first_index + 1..] {
                let forward = candidate(&edges[first_index], second);
                assert_eq!(forward, candidate(second, &edges[first_index]));
                if expected_position.is_none() && is_strict(forward) {
                    expected_position = Some(forward);
                }
            }
        }
        let expected_position =
            expected_position.expect("one canonical exact pair candidate contains every target");

        let original_paper = Paper::default();
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let junction = VertexId::new();
        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets: [edges[1].id, edges[2].id, edges[0].id]
                        .into_iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("the first all-target exact candidate must connect the cluster");

        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == junction)
                .expect("created reverse-only junction")
                .position,
            expected_position
        );
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();
        editor.undo(1).expect("undo reverse-only cluster");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(2).expect("redo reverse-only cluster");
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn decimal_roundoff_cluster_supports_create_reuse_and_exact_history() {
        let (original_pattern, edges) = decimal_roundoff_cluster();
        let position = |vertex_id| {
            original_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == vertex_id)
                .expect("decimal endpoint")
                .position
        };
        let junction_position = match segment_intersection(
            position(edges[0].start),
            position(edges[0].end),
            position(edges[1].start),
            position(edges[1].end),
        )
        .expect("representable decimal intersection")
        {
            SegmentIntersection::Point(point) => point,
            intersection => panic!("expected point intersection, got {intersection:?}"),
        };
        assert_eq!(
            point_segment_relation(
                junction_position,
                position(edges[0].start),
                position(edges[0].end),
            ),
            Ok(PointSegmentRelation::Outside),
            "the public exact predicate intentionally exposes the original roundoff"
        );
        for edge in &edges {
            assert_eq!(
                intersection_cluster_point_segment_relation(
                    junction_position,
                    position(edge.start),
                    position(edge.end),
                ),
                Ok(PointSegmentRelation::StrictInterior)
            );
        }
        let create_junction_position = intersection_cluster_pair_candidate(
            position(edges[0].start),
            position(edges[0].end),
            position(edges[1].start),
            position(edges[1].end),
        )
        .expect("representable decimal planner candidate");
        for edge in &edges {
            assert_eq!(
                intersection_cluster_point_segment_relation(
                    create_junction_position,
                    position(edge.start),
                    position(edge.end),
                ),
                Ok(PointSegmentRelation::StrictInterior)
            );
        }

        let paper = Paper::default();
        let junction = VertexId::new();
        let targets = edges
            .iter()
            .map(|edge| IntersectionEdgeTarget {
                edge: edge.id,
                new_edge: Some(EdgeId::new()),
            })
            .collect::<Vec<_>>();
        let mut editor = EditorState::with_paper(original_pattern.clone(), paper.clone());
        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create { id: junction },
                    targets,
                },
            )
            .expect("create decimal roundoff cluster");
        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == junction)
                .expect("created decimal junction")
                .position,
            create_junction_position
        );
        let connected_pattern = editor.pattern().clone();
        editor.undo(1).expect("undo decimal cluster creation");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &paper);
        editor.redo(2).expect("redo decimal cluster creation");
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &paper);

        let reused = VertexId::new();
        let mut reuse_pattern = original_pattern.clone();
        reuse_pattern.vertices.push(Vertex {
            id: reused,
            position: junction_position,
        });
        let original_reuse_pattern = reuse_pattern.clone();
        let mut reuse_editor = EditorState::with_paper(reuse_pattern, paper.clone());
        reuse_editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Reuse { id: reused },
                    targets: edges
                        .iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("reuse decimal roundoff junction");
        assert_eq!(
            reuse_editor.pattern().vertices,
            original_reuse_pattern.vertices
        );
        reuse_editor.undo(1).expect("undo decimal cluster reuse");
        assert_eq!(reuse_editor.pattern(), &original_reuse_pattern);
        assert_eq!(reuse_editor.paper(), &paper);
    }

    #[test]
    fn cluster_roundoff_bound_separates_adjacent_floats_and_preserves_endpoints() {
        let (pattern, edges) = decimal_roundoff_cluster();
        let position = |vertex_id| {
            pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == vertex_id)
                .expect("decimal endpoint")
                .position
        };
        let start = position(edges[0].start);
        let end = position(edges[0].end);
        let mut last_inside = match segment_intersection(
            start,
            end,
            position(edges[1].start),
            position(edges[1].end),
        )
        .expect("decimal seed")
        {
            SegmentIntersection::Point(point) => point,
            intersection => panic!("expected point intersection, got {intersection:?}"),
        };
        assert_eq!(
            intersection_cluster_point_segment_relation(last_inside, start, end),
            Ok(PointSegmentRelation::StrictInterior)
        );

        let first_outside = (0..100_000)
            .find_map(|_| {
                let candidate =
                    Point2::new(last_inside.x, f64::from_bits(last_inside.y.to_bits() + 1));
                match intersection_cluster_point_segment_relation(candidate, start, end) {
                    Ok(PointSegmentRelation::StrictInterior) => {
                        last_inside = candidate;
                        None
                    }
                    Ok(PointSegmentRelation::Outside) => Some(candidate),
                    relation => panic!("unexpected perturbed relation: {relation:?}"),
                }
            })
            .expect("finite roundoff boundary");
        assert_eq!(first_outside.y.to_bits(), last_inside.y.to_bits() + 1);
        assert_eq!(
            intersection_cluster_point_segment_relation(
                Point2::new(last_inside.x, last_inside.y + 1.0e-9),
                start,
                end,
            ),
            Ok(PointSegmentRelation::Outside)
        );

        let unit_start = Point2::new(0.0, 0.0);
        let unit_end = Point2::new(1.0, 0.0);
        assert_eq!(
            intersection_cluster_point_segment_relation(unit_end, unit_start, unit_end),
            Ok(PointSegmentRelation::End)
        );
        assert_eq!(
            intersection_cluster_point_segment_relation(
                Point2::new(f64::from_bits(1.0f64.to_bits() + 1), 0.0),
                unit_start,
                unit_end,
            ),
            Ok(PointSegmentRelation::Outside),
            "one representable step beyond an endpoint must not become an interior split"
        );
    }

    #[test]
    fn completeness_scan_rejects_unrepresentable_unlisted_edges_and_stale_commands() {
        let (mut pattern, paper, [horizontal, vertical, diagonal], _) = three_way_create_cluster();
        let extreme_start = VertexId::new();
        let extreme_end = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: extreme_start,
                position: Point2::new(-f64::MAX, f64::MAX),
            },
            Vertex {
                id: extreme_end,
                position: Point2::new(f64::MAX, f64::MAX),
            },
        ]);
        pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: extreme_start,
            end: extreme_end,
            kind: EdgeKind::Auxiliary,
        });
        let command = Command::ConnectIntersectionCluster {
            junction: JunctionVertexIntent::Create {
                id: VertexId::new(),
            },
            targets: [horizontal, vertical, diagonal]
                .into_iter()
                .map(|edge| IntersectionEdgeTarget {
                    edge: edge.id,
                    new_edge: Some(EdgeId::new()),
                })
                .collect(),
        };
        let mut editor = EditorState::with_paper(pattern, paper);
        assert_cluster_rejected(
            &mut editor,
            command,
            CommandError::IntersectionClusterGeometryNotRepresentable,
        );

        let (pattern, paper, edges, _) = three_way_create_cluster();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let error = editor
            .execute(
                12,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create {
                        id: VertexId::new(),
                    },
                    targets: edges
                        .into_iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect_err("stale cluster must fail before planning");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 12,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn intersection_cluster_completeness_scan_handles_ten_thousand_sparse_edges() {
        let (mut pattern, paper, target_edges, _) = three_way_create_cluster();
        for index in 0..10_000 {
            let start = VertexId::new();
            let end = VertexId::new();
            let x = 100.0 + index as f64 * 3.0;
            pattern.vertices.extend([
                Vertex {
                    id: start,
                    position: Point2::new(x, 1_000.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(x + 1.0, 1_000.0),
                },
            ]);
            pattern.edges.push(Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Auxiliary,
            });
        }
        let original_edge_count = pattern.edges.len();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create {
                        id: VertexId::new(),
                    },
                    targets: target_edges
                        .into_iter()
                        .map(|edge| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            )
            .expect("sparse completeness scan must remain practical");

        assert_eq!(editor.pattern().edges.len(), original_edge_count + 3);
    }

    #[test]
    fn t_junction_connection_reuses_endpoint_and_restores_nonadjacent_edges_exactly() {
        let (mut editor, original_pattern, original_paper, interior, stem, unrelated) =
            t_junction_editor();
        let interior_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == interior.id)
            .expect("interior edge");
        let unrelated_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == unrelated.id)
            .expect("unrelated edge");
        let stem_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == stem.id)
            .expect("stem edge");
        assert_eq!(unrelated_index, interior_index + 1);
        assert_eq!(stem_index, interior_index + 2);
        assert!(!validate_crease_pattern(&original_pattern).is_valid());
        let original_vertex_count = original_pattern.vertices.len();
        let new_edge = EdgeId::new();

        // Reverse argument order: classification and document ordering must
        // not depend on which target the caller names first.
        let result = editor
            .execute(
                0,
                Command::ConnectTJunction {
                    first_edge: stem.id,
                    second_edge: interior.id,
                    new_edge,
                },
            )
            .expect("connect strict asymmetric T-junction");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![interior.start, interior.end, stem.start, stem.end]
        );
        assert_eq!(result.changed_edges, vec![interior.id, stem.id, new_edge]);
        assert!(!result.settings_changed);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.pattern().vertices.len(), original_vertex_count);
        assert_eq!(editor.pattern().vertices, original_pattern.vertices);
        assert_eq!(
            editor.pattern().edges[interior_index],
            Edge {
                end: stem.start,
                ..interior.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[interior_index + 1],
            Edge {
                id: new_edge,
                start: stem.start,
                end: interior.end,
                kind: EdgeKind::Cut,
            }
        );
        assert_eq!(editor.pattern().edges[unrelated_index + 1], unrelated);
        assert_eq!(editor.pattern().edges[stem_index + 1], stem);
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo T-junction connection");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert!(!undo.settings_changed);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo T-junction connection");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert!(!redo.settings_changed);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &original_paper);
    }

    #[test]
    fn t_junction_classifies_second_edge_endpoints_with_both_stem_orientations() {
        for (junction_is_stem_start, reverse_interior, interior_kind, stem_kind) in [
            (true, false, EdgeKind::Mountain, EdgeKind::Valley),
            (false, true, EdgeKind::Cut, EdgeKind::Auxiliary),
        ] {
            let left = VertexId::new();
            let right = VertexId::new();
            let junction = VertexId::new();
            let stem_other = VertexId::new();
            let vertices = vec![
                Vertex {
                    id: left,
                    position: Point2::new(0.0, 3.0),
                },
                Vertex {
                    id: right,
                    position: Point2::new(10.0, 3.0),
                },
                Vertex {
                    id: junction,
                    position: Point2::new(3.0, 3.0),
                },
                Vertex {
                    id: stem_other,
                    position: Point2::new(3.0, 8.0),
                },
            ];
            let interior = Edge {
                id: EdgeId::new(),
                start: if reverse_interior { right } else { left },
                end: if reverse_interior { left } else { right },
                kind: interior_kind,
            };
            let spacer = Edge {
                id: EdgeId::new(),
                start: left,
                end: stem_other,
                kind: EdgeKind::Mountain,
            };
            let stem = Edge {
                id: EdgeId::new(),
                start: if junction_is_stem_start {
                    junction
                } else {
                    stem_other
                },
                end: if junction_is_stem_start {
                    stem_other
                } else {
                    junction
                },
                kind: stem_kind,
            };
            let original_pattern = CreasePattern {
                vertices,
                edges: vec![interior.clone(), spacer.clone(), stem.clone()],
            };
            let mut editor = EditorState::new(original_pattern.clone());
            let new_edge = EdgeId::new();

            // The stem is deliberately the second command target, so this
            // exercises the second-edge endpoint-on-first-edge classifier.
            let result = editor
                .execute(
                    0,
                    Command::ConnectTJunction {
                        first_edge: interior.id,
                        second_edge: stem.id,
                        new_edge,
                    },
                )
                .expect("connect second-edge endpoint T-junction");

            assert_eq!(result.changed_edges, vec![interior.id, stem.id, new_edge]);
            assert_eq!(
                editor.pattern().edges,
                vec![
                    Edge {
                        end: junction,
                        ..interior.clone()
                    },
                    Edge {
                        id: new_edge,
                        start: junction,
                        end: interior.end,
                        kind: interior.kind,
                    },
                    spacer,
                    stem,
                ]
            );
            assert!(validate_crease_pattern(editor.pattern()).is_valid());
            let connected_pattern = editor.pattern().clone();

            let undo = editor.undo(1).expect("undo classified T-junction");
            assert_eq!(undo.changed_edges, result.changed_edges);
            assert_eq!(editor.pattern(), &original_pattern);

            let redo = editor.redo(2).expect("redo classified T-junction");
            assert_eq!(redo.changed_edges, result.changed_edges);
            assert_eq!(editor.pattern(), &connected_pattern);
        }
    }

    #[test]
    fn t_junction_connection_rejects_targets_ids_and_bad_endpoint_records_atomically() {
        let (_, pattern, paper, interior, stem, _) = t_junction_editor();
        let command = |first_edge, second_edge, new_edge| Command::ConnectTJunction {
            first_edge,
            second_edge,
            new_edge,
        };

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, interior.id, EdgeId::new()),
            CommandError::TJunctionTargetsNotDistinct,
        );

        let missing_edge = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, missing_edge, EdgeId::new()),
            CommandError::EdgeNotFound(missing_edge),
        );

        let mut ambiguous = pattern.clone();
        ambiguous.edges.push(stem.clone());
        let mut editor = EditorState::with_paper(ambiguous, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, stem.id, EdgeId::new()),
            CommandError::TJunctionTargetEdgeIdAmbiguous { edge: stem.id },
        );

        let first_boundary = pattern.edges[0].id;
        let second_boundary = pattern.edges[1].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(first_boundary, second_boundary, EdgeId::new()),
            CommandError::TJunctionBothEdgesBoundary,
        );

        let existing_edge = pattern.edges[1].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, stem.id, existing_edge),
            CommandError::EdgeAlreadyExists(existing_edge),
        );

        let mut duplicate_endpoint = pattern.clone();
        duplicate_endpoint.vertices.push(
            duplicate_endpoint
                .vertices
                .iter()
                .find(|vertex| vertex.id == stem.end)
                .expect("junction endpoint")
                .clone(),
        );
        let mut editor = EditorState::with_paper(duplicate_endpoint, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, stem.id, EdgeId::new()),
            CommandError::TJunctionEndpointVertexRecordAmbiguous {
                edge: stem.id,
                vertex: stem.end,
            },
        );

        let mut missing_endpoint = pattern.clone();
        missing_endpoint
            .vertices
            .retain(|vertex| vertex.id != interior.start);
        let mut editor = EditorState::with_paper(missing_endpoint, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, stem.id, EdgeId::new()),
            CommandError::VertexNotFound(interior.start),
        );

        let mut non_finite = pattern;
        non_finite
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == stem.start)
            .expect("stem start")
            .position
            .y = f64::NEG_INFINITY;
        let mut editor = EditorState::with_paper(non_finite, paper);
        assert_t_junction_rejected(
            &mut editor,
            command(interior.id, stem.id, EdgeId::new()),
            CommandError::TJunctionEndpointPositionNotFinite {
                edge: stem.id,
                vertex: stem.start,
            },
        );
    }

    #[test]
    fn t_junction_connection_rejects_every_other_intersection_class_atomically() {
        let cases = [
            [
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(0.0, 2.0),
                Point2::new(4.0, 2.0),
            ],
            [
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 4.0),
                Point2::new(0.0, 4.0),
                Point2::new(4.0, 0.0),
            ],
            [
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 2.0),
                Point2::new(2.0, 2.0),
                Point2::new(4.0, 0.0),
            ],
            [
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(2.0, 0.0),
                Point2::new(6.0, 0.0),
            ],
        ];
        for points in cases {
            let (mut editor, first, second) = two_edge_editor(points);
            assert_t_junction_rejected(
                &mut editor,
                Command::ConnectTJunction {
                    first_edge: first.id,
                    second_edge: second.id,
                    new_edge: EdgeId::new(),
                },
                CommandError::NotTJunction,
            );
        }

        let (_, mut occupied_pattern, paper, interior, stem, _) = t_junction_editor();
        let occupied_by = VertexId::new();
        occupied_pattern.vertices.push(Vertex {
            id: occupied_by,
            position: Point2::new(32.0, 50.0),
        });
        let mut editor = EditorState::with_paper(occupied_pattern, paper);
        assert_t_junction_rejected(
            &mut editor,
            Command::ConnectTJunction {
                first_edge: interior.id,
                second_edge: stem.id,
                new_edge: EdgeId::new(),
            },
            CommandError::TJunctionPositionOccupied {
                vertex: occupied_by,
            },
        );
    }

    #[test]
    fn t_junction_connection_handles_extreme_finite_segment_exactly() {
        let (mut editor, first, second) = two_edge_editor([
            Point2::new(-f64::MAX, 0.0),
            Point2::new(f64::MAX, 0.0),
            Point2::new(0.0, -1.0),
            Point2::new(0.0, 0.0),
        ]);
        let original = editor.pattern().clone();
        let original_vertex_count = original.vertices.len();
        editor
            .execute(
                0,
                Command::ConnectTJunction {
                    first_edge: first.id,
                    second_edge: second.id,
                    new_edge: EdgeId::new(),
                },
            )
            .expect("exact point-segment relation handles the extreme finite carrier");
        assert_eq!(editor.pattern().vertices.len(), original_vertex_count);
        assert_eq!(editor.pattern().edges.len(), original.edges.len() + 1);
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        editor.undo(1).expect("undo extreme T-junction");
        assert_eq!(editor.pattern(), &original);
    }

    #[test]
    fn stale_t_junction_connection_preserves_state_and_history() {
        let (mut editor, pattern, paper, interior, stem, _) = t_junction_editor();
        let error = editor
            .execute(
                5,
                Command::ConnectTJunction {
                    first_edge: interior.id,
                    second_edge: stem.id,
                    new_edge: EdgeId::new(),
                },
            )
            .expect_err("stale T-junction command must fail");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 5,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn boundary_t_junction_reuses_the_stem_endpoint_and_restores_both_documents_exactly() {
        let (mut editor, original_pattern, original_paper, boundary, stem, junction) =
            boundary_t_junction_editor(0);
        let boundary_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == boundary.id)
            .expect("boundary edge");
        let original_vertex_count = original_pattern.vertices.len();
        let new_edge_id = EdgeId::new();

        // Reverse command order to prove classification is geometric rather
        // than dependent on which selected edge the caller names first.
        let result = editor
            .execute(
                0,
                Command::ConnectTJunction {
                    first_edge: stem.id,
                    second_edge: boundary.id,
                    new_edge: new_edge_id,
                },
            )
            .expect("connect a stem endpoint to a boundary interior");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![boundary.start, boundary.end, stem.start, stem.end]
        );
        assert_eq!(
            result.changed_edges,
            vec![boundary.id, stem.id, new_edge_id]
        );
        assert!(result.settings_changed);
        assert_eq!(editor.pattern().vertices.len(), original_vertex_count);
        assert_eq!(editor.pattern().vertices, original_pattern.vertices);
        assert_eq!(
            editor.pattern().edges[boundary_index],
            Edge {
                end: junction,
                ..boundary.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[boundary_index + 1],
            Edge {
                id: new_edge_id,
                start: junction,
                end: boundary.end,
                kind: EdgeKind::Boundary,
            }
        );
        let mut expected_boundary = original_paper.boundary_vertices.clone();
        expected_boundary.insert(1, junction);
        assert_eq!(editor.paper().boundary_vertices, expected_boundary);
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();
        let connected_paper = editor.paper().clone();

        let undo = editor.undo(1).expect("undo boundary T-junction");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, result.changed_vertices);
        assert_eq!(undo.changed_edges, result.changed_edges);
        assert!(undo.settings_changed);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo boundary T-junction");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, result.changed_vertices);
        assert_eq!(redo.changed_edges, result.changed_edges);
        assert!(redo.settings_changed);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &connected_paper);
    }

    #[test]
    fn boundary_t_junction_preserves_reversed_closing_edges_and_counter_clockwise_order() {
        let (_, mut original_pattern, mut original_paper, boundary, _, junction) =
            boundary_t_junction_editor(3);
        original_paper.boundary_vertices.reverse();
        let stem_index = original_pattern.edges.len() - 1;
        let stem = &mut original_pattern.edges[stem_index];
        std::mem::swap(&mut stem.start, &mut stem.end);
        let stem = stem.clone();
        let mut editor = EditorState::with_paper(original_pattern.clone(), original_paper.clone());
        let boundary_index = original_pattern
            .edges
            .iter()
            .position(|edge| edge.id == boundary.id)
            .expect("closing boundary edge");
        let new_edge_id = EdgeId::new();

        let result = editor
            .execute(
                0,
                Command::ConnectTJunction {
                    first_edge: boundary.id,
                    second_edge: stem.id,
                    new_edge: new_edge_id,
                },
            )
            .expect("connect to reversed closing boundary edge");

        assert!(result.settings_changed);
        assert_eq!(
            editor.pattern().edges[boundary_index],
            Edge {
                end: junction,
                ..boundary.clone()
            }
        );
        assert_eq!(
            editor.pattern().edges[boundary_index + 1],
            Edge {
                id: new_edge_id,
                start: junction,
                end: boundary.end,
                kind: EdgeKind::Boundary,
            }
        );
        let mut expected_boundary = original_paper.boundary_vertices.clone();
        expected_boundary.push(junction);
        assert_eq!(editor.paper().boundary_vertices, expected_boundary);
        assert!(validate_crease_pattern(editor.pattern()).is_valid());
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let connected_pattern = editor.pattern().clone();
        let connected_paper = editor.paper().clone();

        let undo = editor.undo(1).expect("undo closing-boundary T-junction");
        assert!(undo.settings_changed);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        let redo = editor.redo(2).expect("redo closing-boundary T-junction");
        assert!(redo.settings_changed);
        assert_eq!(editor.pattern(), &connected_pattern);
        assert_eq!(editor.paper(), &connected_paper);
    }

    #[test]
    fn boundary_endpoint_carrier_is_rejected_without_changing_the_paper() {
        let sheet = crate::create_rectangular_sheet(100.0, 100.0, true)
            .expect("valid corner T-junction test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary = pattern.edges[0].clone();
        let normal_start = VertexId::new();
        let normal_end = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: normal_start,
                position: Point2::new(-20.0, -20.0),
            },
            Vertex {
                id: normal_end,
                position: Point2::new(20.0, 20.0),
            },
        ]);
        let normal = Edge {
            id: EdgeId::new(),
            start: normal_start,
            end: normal_end,
            kind: EdgeKind::Valley,
        };
        pattern.edges.push(normal.clone());
        let mut editor = EditorState::with_paper(pattern, paper);
        assert_t_junction_rejected(
            &mut editor,
            Command::ConnectTJunction {
                first_edge: normal.id,
                second_edge: boundary.id,
                new_edge: EdgeId::new(),
            },
            CommandError::TJunctionBoundaryEdgeMustBeInterior { edge: boundary.id },
        );
    }

    #[test]
    fn boundary_t_junction_rejects_ambiguous_or_invalid_sheet_links_atomically() {
        let (_, pattern, paper, boundary, stem, junction) = boundary_t_junction_editor(0);
        let command = |new_edge| Command::ConnectTJunction {
            first_edge: boundary.id,
            second_edge: stem.id,
            new_edge,
        };

        let mut missing_segment_paper = paper.clone();
        missing_segment_paper.boundary_vertices.swap(1, 2);
        let mut editor = EditorState::with_paper(pattern.clone(), missing_segment_paper);
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::BoundaryEdgeNotInPaperBoundary(boundary.id),
        );

        let mut multiple_segment_paper = paper.clone();
        multiple_segment_paper.boundary_vertices =
            vec![boundary.start, boundary.end, boundary.start, boundary.end];
        let mut editor = EditorState::with_paper(pattern.clone(), multiple_segment_paper);
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::BoundaryEdgeMatchesMultiplePaperSegments { edge: boundary.id },
        );

        let mut already_present_paper = paper.clone();
        already_present_paper.boundary_vertices.push(junction);
        let mut editor = EditorState::with_paper(pattern.clone(), already_present_paper);
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::TJunctionBoundaryVertexAlreadyPresent { vertex: junction },
        );

        let other_boundary = Edge {
            id: EdgeId::new(),
            start: junction,
            end: stem.end,
            kind: EdgeKind::Boundary,
        };
        let mut already_boundary_connected = pattern.clone();
        already_boundary_connected
            .edges
            .push(other_boundary.clone());
        let mut editor = EditorState::with_paper(already_boundary_connected, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::TJunctionVertexHasOtherBoundaryEdge {
                vertex: junction,
                edge: other_boundary.id,
            },
        );

        let mut ambiguous_edge = pattern.clone();
        ambiguous_edge.edges.push(boundary.clone());
        let mut editor = EditorState::with_paper(ambiguous_edge, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::TJunctionTargetEdgeIdAmbiguous { edge: boundary.id },
        );

        let mut duplicate_endpoint = pattern.clone();
        duplicate_endpoint.vertices.push(
            duplicate_endpoint
                .vertices
                .iter()
                .find(|vertex| vertex.id == boundary.start)
                .expect("boundary start")
                .clone(),
        );
        let mut editor = EditorState::with_paper(duplicate_endpoint, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::TJunctionEndpointVertexRecordAmbiguous {
                edge: boundary.id,
                vertex: boundary.start,
            },
        );

        let mut missing_endpoint = pattern.clone();
        missing_endpoint
            .vertices
            .retain(|vertex| vertex.id != boundary.start);
        let mut editor = EditorState::with_paper(missing_endpoint, paper.clone());
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::VertexNotFound(boundary.start),
        );

        let occupied_by = VertexId::new();
        let junction_position = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == junction)
            .expect("junction vertex")
            .position;
        let mut occupied = pattern.clone();
        occupied.vertices.push(Vertex {
            id: occupied_by,
            position: junction_position,
        });
        let mut editor = EditorState::with_paper(occupied, paper);
        assert_t_junction_rejected(
            &mut editor,
            command(EdgeId::new()),
            CommandError::TJunctionPositionOccupied {
                vertex: occupied_by,
            },
        );
    }

    #[test]
    fn stale_boundary_t_junction_preserves_both_documents_and_history() {
        let (mut editor, pattern, paper, boundary, stem, _) = boundary_t_junction_editor(0);
        let error = editor
            .execute(
                7,
                Command::ConnectTJunction {
                    first_edge: boundary.id,
                    second_edge: stem.id,
                    new_edge: EdgeId::new(),
                },
            )
            .expect_err("stale boundary T-junction must fail");

        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 7,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn boundary_split_preserves_ids_order_and_validation_through_undo_redo() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_edge = original_pattern.edges[0].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();
        assert!(crate::validate_paper(&original_paper, &original_pattern).is_valid());

        let result = editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.25,
                },
            )
            .expect("split boundary edge");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(result.changed_edges, vec![original_edge.id, new_edge_id]);
        assert!(result.settings_changed);
        assert_eq!(
            editor.paper().boundary_vertices,
            vec![
                original_paper.boundary_vertices[0],
                new_vertex_id,
                original_paper.boundary_vertices[1],
                original_paper.boundary_vertices[2],
                original_paper.boundary_vertices[3],
            ]
        );
        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex_id,
                position: Point2::new(10.0, 32.5),
            })
        );
        assert_eq!(editor.pattern().edges[0].id, original_edge.id);
        assert_eq!(editor.pattern().edges[0].start, original_edge.start);
        assert_eq!(editor.pattern().edges[0].end, new_vertex_id);
        assert_eq!(
            editor.pattern().edges[1],
            Edge {
                id: new_edge_id,
                start: new_vertex_id,
                end: original_edge.end,
                kind: EdgeKind::Boundary,
            }
        );
        assert_eq!(editor.pattern().edges[2..], original_pattern.edges[1..]);
        assert_eq!(editor.paper().thickness_mm, original_paper.thickness_mm);
        assert_eq!(
            editor.paper().cutting_allowed,
            original_paper.cutting_allowed
        );
        assert_eq!(editor.paper().front, original_paper.front);
        assert_eq!(editor.paper().back, original_paper.back);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let split_pattern = editor.pattern().clone();
        let split_paper = editor.paper().clone();

        let undo = editor.undo(1).expect("undo boundary split");
        assert_eq!(undo.revision, 2);
        assert_eq!(
            undo.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(undo.changed_edges, vec![original_edge.id, new_edge_id]);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo boundary split");
        assert_eq!(redo.revision, 3);
        assert_eq!(
            redo.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(editor.pattern(), &split_pattern);
        assert_eq!(editor.paper(), &split_paper);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_split_handles_an_edge_opposite_to_the_paper_order() {
        let (_, mut pattern, paper) = rectangular_editor();
        let forward_edge = pattern.edges[0].clone();
        pattern.edges[0] = Edge {
            start: forward_edge.end,
            end: forward_edge.start,
            ..forward_edge
        };
        let original_edge = pattern.edges[0].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.25,
                },
            )
            .expect("split reverse boundary edge");

        assert_eq!(
            editor
                .pattern()
                .vertices
                .last()
                .map(|vertex| vertex.position),
            Some(Point2::new(10.0, 57.5))
        );
        assert_eq!(editor.pattern().edges[0].start, original_edge.start);
        assert_eq!(editor.pattern().edges[0].end, new_vertex_id);
        assert_eq!(editor.pattern().edges[1].start, new_vertex_id);
        assert_eq!(editor.pattern().edges[1].end, original_edge.end);
        assert_eq!(editor.paper().boundary_vertices[1], new_vertex_id);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_split_handles_the_closing_paper_edge() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_edge = original_pattern.edges[3].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();

        editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.5,
                },
            )
            .expect("split closing edge");

        assert_eq!(editor.paper().boundary_vertices.len(), 5);
        assert_eq!(editor.paper().boundary_vertices[4], new_vertex_id);
        assert_eq!(
            editor
                .pattern()
                .vertices
                .last()
                .map(|vertex| vertex.position),
            Some(Point2::new(60.0, 20.0))
        );
        assert_eq!(editor.pattern().edges[3].id, original_edge.id);
        assert_eq!(editor.pattern().edges[3].start, original_edge.start);
        assert_eq!(editor.pattern().edges[3].end, new_vertex_id);
        assert_eq!(editor.pattern().edges[4].id, new_edge_id);
        assert_eq!(editor.pattern().edges[5], original_pattern.edges[4]);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());

        editor.undo(1).expect("undo closing split");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(2).expect("redo closing split");
        assert_eq!(editor.paper().boundary_vertices[4], new_vertex_id);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_split_uses_a_stable_convex_combination_for_extreme_endpoints() {
        let ids = [VertexId::new(), VertexId::new(), VertexId::new()];
        let edge = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Boundary,
        };
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: ids[0],
                    position: Point2::new(-f64::MAX, 0.0),
                },
                Vertex {
                    id: ids[1],
                    position: Point2::new(f64::MAX, 0.0),
                },
                Vertex {
                    id: ids[2],
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![edge.clone()],
        };
        let paper = Paper {
            boundary_vertices: ids.to_vec(),
            ..Paper::default()
        };
        let new_vertex = VertexId::new();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: edge.id,
                    new_vertex,
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect("extreme finite endpoints must interpolate safely");

        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex,
                position: Point2::new(0.0, 0.0),
            })
        );
    }

    #[test]
    fn boundary_split_rejects_an_existing_third_vertex_at_the_new_position() {
        let (_, mut pattern, paper) = rectangular_editor();
        let edge = pattern.edges[0].clone();
        let occupied_by = VertexId::new();
        pattern.vertices.push(Vertex {
            id: occupied_by,
            position: Point2::new(10.0, 45.0),
        });
        let mut editor = EditorState::with_paper(pattern, paper);

        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitPositionOccupied {
                vertex: occupied_by,
            },
        );
    }

    #[test]
    fn boundary_split_checks_duplicate_id_vertex_records_for_occupied_positions() {
        let (_, mut pattern, paper) = rectangular_editor();
        let edge = pattern.edges[0].clone();
        pattern.vertices.push(Vertex {
            id: edge.start,
            position: Point2::new(10.0, 45.0),
        });
        let mut editor = EditorState::with_paper(pattern, paper);

        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitPositionOccupied { vertex: edge.start },
        );
    }

    #[test]
    fn invalid_boundary_split_targets_and_ids_are_atomic() {
        let (_, pattern, paper) = rectangular_editor();
        let boundary_edge = pattern.edges[0].clone();
        let mountain_edge = pattern.edges[4].clone();

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: mountain_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeIsNotBoundary(mountain_edge.id),
        );

        let diagonal = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Boundary,
        };
        let mut diagonal_pattern = pattern.clone();
        diagonal_pattern.edges.push(diagonal.clone());
        let mut editor = EditorState::with_paper(diagonal_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: diagonal.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundaryEdgeNotInPaperBoundary(diagonal.id),
        );

        let mut duplicate_edge_pattern = pattern.clone();
        duplicate_edge_pattern.edges.push(boundary_edge.clone());
        let mut editor = EditorState::with_paper(duplicate_edge_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitTargetEdgeIdAmbiguous {
                edge: boundary_edge.id,
            },
        );

        let first = VertexId::new();
        let second = VertexId::new();
        let ambiguous_edge = Edge {
            id: EdgeId::new(),
            start: first,
            end: second,
            kind: EdgeKind::Boundary,
        };
        let ambiguous_pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![ambiguous_edge.clone()],
        };
        let ambiguous_paper = Paper {
            boundary_vertices: vec![first, second, first],
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(ambiguous_pattern, ambiguous_paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: ambiguous_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundaryEdgeMatchesMultiplePaperSegments {
                edge: ambiguous_edge.id,
            },
        );

        let existing_vertex = pattern.vertices[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: existing_vertex,
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::VertexAlreadyExists(existing_vertex),
        );

        let existing_edge = pattern.edges[1].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: existing_edge,
                fraction: 0.5,
            },
            CommandError::EdgeAlreadyExists(existing_edge),
        );
    }

    #[test]
    fn invalid_boundary_split_fractions_positions_and_conflicts_are_atomic() {
        let (_, pattern, paper) = rectangular_editor();
        let boundary_edge = pattern.edges[0].clone();
        for (fraction, expected) in [
            (f64::NAN, CommandError::BoundarySplitFractionNotFinite),
            (f64::INFINITY, CommandError::BoundarySplitFractionNotFinite),
            (0.0, CommandError::BoundarySplitFractionOutOfRange),
            (
                -f64::MIN_POSITIVE,
                CommandError::BoundarySplitFractionOutOfRange,
            ),
            (1.0, CommandError::BoundarySplitFractionOutOfRange),
        ] {
            let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
            assert_split_rejected(
                &mut editor,
                Command::SplitBoundaryEdge {
                    edge: boundary_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction,
                },
                expected,
            );
        }

        let mut non_finite_pattern = pattern.clone();
        non_finite_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == boundary_edge.start)
            .expect("boundary start")
            .position
            .x = f64::INFINITY;
        let mut editor = EditorState::with_paper(non_finite_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitEndpointPositionNotFinite {
                edge: boundary_edge.id,
                vertex: boundary_edge.start,
            },
        );

        let close_ids = [VertexId::new(), VertexId::new(), VertexId::new()];
        let close_edge = Edge {
            id: EdgeId::new(),
            start: close_ids[0],
            end: close_ids[1],
            kind: EdgeKind::Boundary,
        };
        let close_pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: close_ids[0],
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: close_ids[1],
                    position: Point2::new(2.0, 0.0),
                },
                Vertex {
                    id: close_ids[2],
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![close_edge.clone()],
        };
        let close_paper = Paper {
            boundary_vertices: close_ids.to_vec(),
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(close_pattern, close_paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: close_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: f64::MIN_POSITIVE,
            },
            CommandError::BoundarySplitPositionNotDistinct,
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let original_pattern = editor.pattern().clone();
        let original_paper = editor.paper().clone();
        let error = editor
            .execute(
                9,
                Command::SplitBoundaryEdge {
                    edge: boundary_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect_err("stale split must fail");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 9,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn boundary_vertex_removal_merges_edges_and_restores_exactly() {
        let (mut editor, original_pattern, original_paper) = simple_rectangular_editor();
        let target = original_paper.boundary_vertices[1];
        let previous = original_paper.boundary_vertices[0];
        let next = original_paper.boundary_vertices[2];
        let kept_edge = original_pattern.edges[0].clone();
        let removed_edge = original_pattern.edges[1].clone();

        let result = editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("remove boundary vertex");

        assert_eq!(result.revision, 1);
        assert_eq!(result.changed_vertices, vec![target, previous, next]);
        assert_eq!(result.changed_edges, vec![kept_edge.id, removed_edge.id]);
        assert!(result.settings_changed);
        let mut expected_pattern = original_pattern.clone();
        expected_pattern
            .vertices
            .retain(|vertex| vertex.id != target);
        expected_pattern.edges[0].end = next;
        expected_pattern.edges.remove(1);
        let mut expected_paper = original_paper.clone();
        expected_paper.boundary_vertices.remove(1);
        assert_eq!(editor.pattern(), &expected_pattern);
        assert_eq!(editor.paper(), &expected_paper);
        assert_eq!(editor.pattern().edges[0].id, kept_edge.id);
        assert_eq!(editor.pattern().edges[0].start, kept_edge.start);
        assert_eq!(editor.pattern().edges[0].end, next);
        assert_eq!(editor.pattern().edges[1..], original_pattern.edges[2..]);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());

        let undo = editor.undo(1).expect("undo boundary vertex removal");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, vec![target, previous, next]);
        assert_eq!(undo.changed_edges, vec![kept_edge.id, removed_edge.id]);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo boundary vertex removal");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, vec![target, previous, next]);
        assert_eq!(editor.pattern(), &expected_pattern);
        assert_eq!(editor.paper(), &expected_paper);
    }

    #[test]
    fn boundary_vertex_removal_preserves_reversed_kept_edge_orientation() {
        let (_, mut pattern, paper) = simple_rectangular_editor();
        let target = paper.boundary_vertices[1];
        let previous = paper.boundary_vertices[0];
        let next = paper.boundary_vertices[2];
        let preceding = pattern.edges[0].clone();
        pattern.edges[0] = Edge {
            start: preceding.end,
            end: preceding.start,
            ..preceding
        };
        let following = pattern.edges[1].clone();
        pattern.edges[1] = Edge {
            start: following.end,
            end: following.start,
            ..following
        };
        let kept_id = pattern.edges[0].id;
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("remove vertex with reverse edges");

        assert_eq!(editor.pattern().edges[0].id, kept_id);
        assert_eq!(editor.pattern().edges[0].start, next);
        assert_eq!(editor.pattern().edges[0].end, previous);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_vertex_removal_handles_closing_predecessor_before_exact_undo() {
        let (mut editor, original_pattern, original_paper) = simple_rectangular_editor();
        let target = original_paper.boundary_vertices[0];
        let previous = original_paper.boundary_vertices[3];
        let next = original_paper.boundary_vertices[1];
        let kept_edge = original_pattern.edges[3].clone();
        let removed_edge = original_pattern.edges[0].clone();

        editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("remove vertex at closing boundary junction");

        assert_eq!(
            editor.paper().boundary_vertices,
            original_paper.boundary_vertices[1..]
        );
        assert_eq!(editor.pattern().edges[0], original_pattern.edges[1]);
        assert_eq!(editor.pattern().edges[1], original_pattern.edges[2]);
        assert_eq!(editor.pattern().edges[2].id, kept_edge.id);
        assert_eq!(editor.pattern().edges[2].start, previous);
        assert_eq!(editor.pattern().edges[2].end, next);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());

        editor.undo(1).expect("undo closing vertex removal");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(2).expect("redo closing vertex removal");
        assert_eq!(editor.pattern().edges[2].id, kept_edge.id);
        assert!(
            !editor
                .pattern()
                .edges
                .iter()
                .any(|edge| edge.id == removed_edge.id)
        );
    }

    #[test]
    fn boundary_vertex_removal_rejects_a_collinear_candidate_from_a_valid_state() {
        let (mut editor, original_pattern, original_paper, target) =
            collinear_after_removal_editor();
        assert!(validate_crease_pattern(&original_pattern).is_valid());
        assert!(validate_paper(&original_paper, &original_pattern).is_valid());

        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRemovalWouldInvalidatePaper,
        );
    }

    #[test]
    fn boundary_vertex_removal_rejects_a_new_edge_through_existing_geometry() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let target = original_paper.boundary_vertices[1];
        assert!(validate_crease_pattern(&original_pattern).is_valid());
        assert!(validate_paper(&original_paper, &original_pattern).is_valid());

        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRemovalWouldInvalidatePaper,
        );
    }

    #[test]
    fn boundary_vertex_removal_can_edit_an_already_invalid_state() {
        let (_, pattern, mut paper, target) = collinear_after_removal_editor();
        paper.thickness_mm = -0.1;
        assert!(validate_crease_pattern(&pattern).is_valid());
        assert!(!validate_paper(&paper, &pattern).is_valid());
        let mut editor = EditorState::with_paper(pattern, paper);

        let result = editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("an already invalid project remains editable");

        assert_eq!(result.revision, 1);
        assert!(
            !editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == target)
        );
        assert_eq!(editor.paper().boundary_vertices.len(), 3);
    }

    #[test]
    fn boundary_vertex_removal_rejects_invalid_boundary_and_vertex_identity() {
        let (_, pattern, paper) = rectangular_editor();
        let target = paper.boundary_vertices[1];

        let mut triangle_paper = paper.clone();
        triangle_paper.boundary_vertices.pop();
        let mut editor = EditorState::with_paper(pattern.clone(), triangle_paper);
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRemovalNeedsFourVertices { actual: 3 },
        );

        let not_boundary = pattern.vertices[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            not_boundary,
            CommandError::VertexNotInPaperBoundary(not_boundary),
        );

        let mut duplicate_boundary_paper = paper.clone();
        duplicate_boundary_paper.boundary_vertices[3] = target;
        let mut editor = EditorState::with_paper(pattern.clone(), duplicate_boundary_paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexOccursMultipleTimes { vertex: target },
        );

        let mut missing_pattern = pattern.clone();
        missing_pattern
            .vertices
            .retain(|vertex| vertex.id != target);
        let mut editor = EditorState::with_paper(missing_pattern, paper.clone());
        assert_boundary_removal_rejected(&mut editor, target, CommandError::VertexNotFound(target));

        let mut duplicate_pattern = pattern.clone();
        let duplicate_record = duplicate_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == target)
            .expect("target vertex")
            .clone();
        duplicate_pattern.vertices.push(duplicate_record);
        let mut editor = EditorState::with_paper(duplicate_pattern, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRecordAmbiguous { vertex: target },
        );

        let previous = paper.boundary_vertices[0];
        let other = paper.boundary_vertices[2];
        let malformed_paper = Paper {
            boundary_vertices: vec![previous, target, previous, other],
            ..paper
        };
        let mut editor = EditorState::with_paper(pattern, malformed_paper);
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexNeighborsNotDistinct {
                vertex: target,
                neighbor: previous,
            },
        );
    }

    #[test]
    fn boundary_vertex_removal_rejects_invalid_adjacent_edge_topology() {
        let (_, pattern, paper) = rectangular_editor();
        let target = paper.boundary_vertices[1];
        let previous = paper.boundary_vertices[0];
        let next = paper.boundary_vertices[2];

        let mut missing_preceding = pattern.clone();
        missing_preceding.edges[0].kind = EdgeKind::Auxiliary;
        let mut editor = EditorState::with_paper(missing_preceding, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexPrecedingEdgeMissing { vertex: target },
        );

        let mut ambiguous_preceding = pattern.clone();
        let mut duplicate_preceding = ambiguous_preceding.edges[0].clone();
        duplicate_preceding.id = EdgeId::new();
        ambiguous_preceding.edges.push(duplicate_preceding);
        let mut editor = EditorState::with_paper(ambiguous_preceding, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexPrecedingEdgeAmbiguous { vertex: target },
        );

        let mut missing_following = pattern.clone();
        missing_following.edges[1].kind = EdgeKind::Valley;
        let mut editor = EditorState::with_paper(missing_following, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexFollowingEdgeMissing { vertex: target },
        );

        let mut duplicate_edge_ids = pattern.clone();
        duplicate_edge_ids.edges[1].id = duplicate_edge_ids.edges[0].id;
        let mut editor = EditorState::with_paper(duplicate_edge_ids, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexAdjacentEdgesNotDistinct { vertex: target },
        );

        for adjacent_edge_id in [pattern.edges[0].id, pattern.edges[1].id] {
            let mut ambiguous_edge_id_pattern = pattern.clone();
            let mut unrelated_record = ambiguous_edge_id_pattern.edges[4].clone();
            unrelated_record.id = adjacent_edge_id;
            ambiguous_edge_id_pattern.edges.push(unrelated_record);
            let mut editor = EditorState::with_paper(ambiguous_edge_id_pattern, paper.clone());
            assert_boundary_removal_rejected(
                &mut editor,
                target,
                CommandError::BoundaryVertexAdjacentEdgeIdAmbiguous {
                    vertex: target,
                    edge: adjacent_edge_id,
                },
            );
        }

        let additional_edge = Edge {
            id: EdgeId::new(),
            start: target,
            end: pattern.vertices[0].id,
            kind: EdgeKind::Mountain,
        };
        let mut additionally_connected = pattern.clone();
        additionally_connected.edges.push(additional_edge.clone());
        let mut editor = EditorState::with_paper(additionally_connected, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexHasAdditionalEdge {
                vertex: target,
                edge: additional_edge.id,
            },
        );

        let neighbor_edge = Edge {
            id: EdgeId::new(),
            start: previous,
            end: next,
            kind: EdgeKind::Auxiliary,
        };
        let mut already_connected_neighbors = pattern.clone();
        already_connected_neighbors
            .edges
            .push(neighbor_edge.clone());
        let mut editor = EditorState::with_paper(already_connected_neighbors, paper);
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexNeighborEdgeAlreadyExists {
                vertex: target,
                edge: neighbor_edge.id,
            },
        );
    }

    #[test]
    fn stale_boundary_vertex_removal_preserves_state_and_history() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let target = original_paper.boundary_vertices[1];

        let error = editor
            .execute(8, Command::RemoveBoundaryVertex { vertex: target })
            .expect_err("stale boundary removal must fail");

        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 8,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn invalid_paper_thickness_does_not_change_state_or_history() {
        for (invalid, expected) in [
            (f64::NAN, CommandError::PaperThicknessNotFinite),
            (f64::INFINITY, CommandError::PaperThicknessNotFinite),
            (f64::NEG_INFINITY, CommandError::PaperThicknessNotFinite),
            (-f64::MIN_POSITIVE, CommandError::PaperThicknessNegative),
        ] {
            let mut editor = EditorState::new(CreasePattern::empty());
            let original = editor.paper().clone();
            let error = editor
                .execute(
                    0,
                    Command::UpdatePaperProperties {
                        thickness_mm: invalid,
                        front_color: RgbaColor::opaque(1, 2, 3),
                        back_color: RgbaColor::opaque(4, 5, 6),
                        front_texture_asset: None,
                        back_texture_asset: None,
                        cutting_allowed: true,
                    },
                )
                .expect_err("invalid thickness must fail");

            assert_eq!(error, expected);
            assert_eq!(editor.paper(), &original);
            assert_eq!(editor.revision(), 0);
            assert!(!editor.can_undo());
            assert!(!editor.can_redo());
        }
    }

    #[test]
    fn existing_cut_edge_prevents_disabling_cutting_without_partial_changes() {
        let start = VertexId::new();
        let end = VertexId::new();
        let cut_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: cut_edge,
                start,
                end,
                kind: EdgeKind::Cut,
            }],
        };
        let paper = Paper {
            cutting_allowed: true,
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(pattern.clone(), paper);
        let original = editor.paper().clone();

        let error = editor
            .execute(0, Command::SetCuttingAllowed { allowed: false })
            .expect_err("cut edge must prevent disabling");
        assert_eq!(
            error,
            CommandError::CutEdgesPreventDisabling { edge: cut_edge }
        );
        assert_eq!(editor.paper(), &original);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());

        let error = editor
            .execute(
                0,
                Command::UpdatePaperProperties {
                    thickness_mm: 0.5,
                    front_color: RgbaColor::opaque(1, 2, 3),
                    back_color: RgbaColor::opaque(4, 5, 6),
                    front_texture_asset: None,
                    back_texture_asset: None,
                    cutting_allowed: false,
                },
            )
            .expect_err("combined update must also reject disabling");
        assert_eq!(
            error,
            CommandError::CutEdgesPreventDisabling { edge: cut_edge }
        );
        assert_eq!(editor.paper(), &original);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn undo_restores_a_loaded_cutting_policy_without_revalidating_history() {
        let start = VertexId::new();
        let end = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Cut,
            }],
        };
        let mut editor = EditorState::with_paper(pattern, Paper::default());

        editor
            .execute(0, Command::SetCuttingAllowed { allowed: true })
            .expect("repair loaded cutting policy");
        editor.undo(1).expect("restore exact loaded state");

        assert!(!editor.paper().cutting_allowed);
        assert_eq!(editor.revision(), 2);
        assert!(editor.can_redo());
    }

    #[test]
    fn persisted_paper_restores_without_creating_history() {
        let paper = Paper {
            cutting_allowed: true,
            thickness_mm: 0.25,
            ..Paper::default()
        };
        let editor = EditorState::with_paper(CreasePattern::empty(), paper.clone());

        assert_eq!(editor.paper(), &paper);
        assert!(editor.cutting_allowed());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn restored_cutting_setting_allows_cut_edges_at_revision_zero() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: Vec::new(),
        };
        let paper = Paper {
            cutting_allowed: true,
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::AddEdge {
                    id: edge,
                    start,
                    end,
                    kind: EdgeKind::Cut,
                },
            )
            .expect("add cut using restored setting");

        assert_eq!(editor.pattern().edges[0].id, edge);
        assert_eq!(editor.revision(), 1);
        assert!(editor.can_undo());
    }

    #[test]
    fn undo_remove_vertex_restores_the_original_vector_order() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let original = CreasePattern {
            vertices: vertices.to_vec(),
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(original.clone());

        editor
            .execute(0, Command::RemoveVertex { id: vertices[1].id })
            .expect("remove middle vertex");
        editor.undo(1).expect("restore middle vertex");

        assert_eq!(editor.pattern(), &original);
    }

    #[test]
    fn undo_remove_edge_restores_the_original_vector_order() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Auxiliary,
            },
        ];
        let original = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: edges.to_vec(),
        };
        let mut editor = EditorState::new(original.clone());

        editor
            .execute(0, Command::RemoveEdge { id: edges[1].id })
            .expect("remove middle edge");
        editor.undo(1).expect("restore middle edge");

        assert_eq!(editor.pattern(), &original);
    }

    #[test]
    fn failed_undo_preserves_state_and_history_for_retry() {
        let first = VertexId::new();
        let second = VertexId::new();
        let blocking_edge = EdgeId::new();
        let mut editor = EditorState::new(CreasePattern::empty());

        editor
            .execute(
                0,
                Command::AddVertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("add first vertex");
        editor
            .execute(
                1,
                Command::AddVertex {
                    id: second,
                    position: Point2::new(1.0, 0.0),
                },
            )
            .expect("add second vertex");
        editor
            .undo(2)
            .expect("place second command in redo history");

        editor.pattern.edges.push(Edge {
            id: blocking_edge,
            start: first,
            end: first,
            kind: EdgeKind::Auxiliary,
        });
        let pattern_before = editor.pattern.clone();
        let paper_before = editor.paper.clone();
        let revision_before = editor.revision;
        let undo_before = format!("{:?}", editor.undo_stack);
        let redo_before = format!("{:?}", editor.redo_stack);

        let error = editor
            .undo(revision_before)
            .expect_err("connected edge must prevent removing the vertex");

        assert_eq!(
            error,
            CommandError::VertexHasConnectedEdge {
                vertex: first,
                edge: blocking_edge,
            }
        );
        assert_eq!(editor.pattern, pattern_before);
        assert_eq!(editor.paper, paper_before);
        assert_eq!(editor.revision, revision_before);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);
        assert!(editor.can_undo());
        assert!(editor.can_redo());

        editor.pattern.edges.clear();
        editor
            .undo(revision_before)
            .expect("preserved undo entry must remain retryable");

        assert!(editor.pattern.vertices.is_empty());
        assert_eq!(editor.revision, revision_before + 1);
        assert!(!editor.can_undo());
        assert_eq!(editor.redo_stack.len(), 2);
    }

    #[test]
    fn failed_redo_preserves_state_and_history_for_retry() {
        let first = VertexId::new();
        let second = VertexId::new();
        let second_vertex = Vertex {
            id: second,
            position: Point2::new(1.0, 0.0),
        };
        let mut editor = EditorState::new(CreasePattern::empty());

        editor
            .execute(
                0,
                Command::AddVertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("add first vertex");
        editor
            .execute(
                1,
                Command::AddVertex {
                    id: second,
                    position: second_vertex.position,
                },
            )
            .expect("add second vertex");
        editor
            .undo(2)
            .expect("place second command in redo history");

        editor.pattern.vertices.push(second_vertex.clone());
        let pattern_before = editor.pattern.clone();
        let paper_before = editor.paper.clone();
        let revision_before = editor.revision;
        let undo_before = format!("{:?}", editor.undo_stack);
        let redo_before = format!("{:?}", editor.redo_stack);

        let error = editor
            .redo(revision_before)
            .expect_err("duplicate vertex must prevent replaying the command");

        assert_eq!(error, CommandError::VertexAlreadyExists(second));
        assert_eq!(editor.pattern, pattern_before);
        assert_eq!(editor.paper, paper_before);
        assert_eq!(editor.revision, revision_before);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);
        assert!(editor.can_undo());
        assert!(editor.can_redo());

        editor.pattern.vertices.pop();
        editor
            .redo(revision_before)
            .expect("preserved redo entry must remain retryable");

        assert_eq!(
            editor.pattern.vertices,
            vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                second_vertex,
            ]
        );
        assert_eq!(editor.revision, revision_before + 1);
        assert_eq!(editor.undo_stack.len(), 2);
        assert!(!editor.can_redo());
    }

    fn instruction_step(
        id: InstructionStepId,
        title: &str,
        source_model_fingerprint: String,
    ) -> InstructionStep {
        use ori_domain::InstructionPoseModel;

        InstructionStep {
            id,
            title: title.to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: 1_500,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint,
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        }
    }

    fn declarative_instruction_step(
        id: InstructionStepId,
        title: &str,
        source_model_fingerprint: String,
    ) -> InstructionStep {
        InstructionStep {
            id,
            title: title.to_owned(),
            description: "説明テンプレート".to_owned(),
            caution: "物理操作は自動実行しません。".to_owned(),
            duration_ms: 1_500,
            visual: Default::default(),
            pose: InstructionPose {
                model: ori_domain::InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint,
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        }
    }

    #[test]
    fn persisted_instruction_timeline_restores_without_creating_history() {
        let pattern = CreasePattern::empty();
        let paper = Paper::default();
        let fingerprint =
            crate::fold_model_fingerprint::fold_model_fingerprint_v1(&pattern, &paper);
        let timeline = InstructionTimeline {
            steps: vec![instruction_step(
                InstructionStepId::new(),
                "最初の手順",
                fingerprint,
            )],
        };

        let editor =
            EditorState::with_document_parts(pattern.clone(), paper.clone(), timeline.clone());

        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.instruction_timeline(), &timeline);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
        assert!(
            EditorState::with_paper(pattern, paper)
                .instruction_timeline()
                .steps
                .is_empty()
        );
    }

    #[test]
    fn instruction_commands_share_revision_undo_redo_and_change_reporting() {
        use ori_domain::{InstructionHingeAngle, InstructionPoseModel};

        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let first_id = InstructionStepId::new();
        let second_id = InstructionStepId::new();
        let first = instruction_step(first_id, "手順 1", fingerprint.clone());
        let second = instruction_step(second_id, "手順 2", fingerprint.clone());

        let add = editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: first.clone(),
                },
            )
            .expect("add first instruction");
        assert_eq!(add.revision, 1);
        assert!(add.instructions_changed);
        assert!(add.changed_vertices.is_empty());
        assert!(add.changed_edges.is_empty());
        assert!(!add.settings_changed);

        editor
            .execute(
                1,
                Command::AddInstructionStep {
                    step: second.clone(),
                },
            )
            .expect("add second instruction");
        editor
            .execute(
                2,
                Command::UpdateInstructionStepMetadata {
                    step_id: first_id,
                    title: "谷折りする".to_owned(),
                    description: "中央まで折る".to_owned(),
                    caution: "強く押さえない".to_owned(),
                    duration_ms: 2_000,
                    visual: Default::default(),
                },
            )
            .expect("update metadata");
        assert_eq!(editor.instruction_timeline().steps[0].title, "谷折りする");

        let replacement_pose = InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: "1".repeat(64),
            fixed_face: None,
            hinge_angles: vec![InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: 45.0,
            }],
        };
        editor
            .execute(
                3,
                Command::ReplaceInstructionStepPose {
                    step_id: first_id,
                    pose: replacement_pose.clone(),
                },
            )
            .expect("replace pose");
        assert_eq!(
            editor.instruction_timeline().steps[0].pose,
            replacement_pose
        );

        editor
            .execute(
                4,
                Command::MoveInstructionStep {
                    step_id: first_id,
                    target_index: 1,
                },
            )
            .expect("move first instruction");
        assert_eq!(editor.instruction_timeline().steps[1].id, first_id);

        editor
            .execute(5, Command::RemoveInstructionStep { step_id: second_id })
            .expect("remove second instruction");
        assert_eq!(
            editor
                .instruction_timeline()
                .steps
                .iter()
                .map(|step| step.id)
                .collect::<Vec<_>>(),
            vec![first_id]
        );

        let undo = editor.undo(6).expect("undo removal");
        assert!(undo.instructions_changed);
        assert_eq!(
            editor
                .instruction_timeline()
                .steps
                .iter()
                .map(|step| step.id)
                .collect::<Vec<_>>(),
            vec![second_id, first_id]
        );
        let redo = editor.redo(7).expect("redo removal");
        assert!(redo.instructions_changed);
        assert_eq!(
            editor
                .instruction_timeline()
                .steps
                .iter()
                .map(|step| step.id)
                .collect::<Vec<_>>(),
            vec![first_id]
        );
    }

    #[test]
    fn instruction_batch_append_is_one_atomic_revision_and_one_undo_redo_entry() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let existing = instruction_step(InstructionStepId::new(), "既存", fingerprint.clone());
        editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: existing.clone(),
                },
            )
            .expect("seed timeline");
        let batch = vec![
            declarative_instruction_step(InstructionStepId::new(), "技法", fingerprint.clone()),
            declarative_instruction_step(InstructionStepId::new(), "操作 1", fingerprint),
        ];

        let applied = editor
            .execute(
                1,
                Command::AppendInstructionSteps {
                    steps: batch.clone(),
                },
            )
            .expect("append batch");
        assert_eq!(applied.revision, 2);
        assert!(applied.instructions_changed);
        assert_eq!(
            editor.instruction_timeline().steps,
            vec![existing.clone(), batch[0].clone(), batch[1].clone()]
        );
        assert!(matches!(
            editor.undo_stack.last().map(|entry| &entry.inverse),
            Some(Inverse::RemoveAppendedInstructionSteps { step_ids })
                if step_ids == &batch.iter().map(|step| step.id).collect::<Vec<_>>()
        ));

        editor.undo(2).expect("undo entire append");
        assert_eq!(editor.instruction_timeline().steps, vec![existing.clone()]);
        editor.redo(3).expect("redo entire append");
        assert_eq!(
            editor.instruction_timeline().steps,
            vec![existing, batch[0].clone(), batch[1].clone()]
        );
    }

    #[test]
    fn invalid_instruction_batch_append_is_atomic_and_preserves_redo() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let seed = instruction_step(InstructionStepId::new(), "既存", fingerprint.clone());
        editor
            .execute(0, Command::AddInstructionStep { step: seed })
            .expect("seed timeline");
        editor.undo(1).expect("prepare redo");

        let before = editor.instruction_timeline.clone();
        let revision = editor.revision();
        let undo_before = format!("{:?}", editor.undo_stack);
        let redo_before = format!("{:?}", editor.redo_stack);
        let mut invalid =
            declarative_instruction_step(InstructionStepId::new(), "不正", fingerprint.clone());
        invalid.pose.fixed_face = Some(ori_domain::FaceId::new());

        assert_eq!(
            editor.execute(
                revision,
                Command::AppendInstructionSteps { steps: Vec::new() },
            ),
            Err(CommandError::InstructionStepAppendBatchEmpty)
        );
        assert!(matches!(
            editor.execute(
                revision,
                Command::AppendInstructionSteps {
                    steps: vec![
                        declarative_instruction_step(
                            InstructionStepId::new(),
                            "有効だが追加されない",
                            fingerprint,
                        ),
                        invalid,
                    ],
                },
            ),
            Err(CommandError::InstructionTimelineInvalid(
                ori_domain::InstructionTimelineValidationError::DeclarativePoseHasFixedFace {
                    step_index: 1
                }
            ))
        ));
        assert_eq!(editor.instruction_timeline, before);
        assert_eq!(editor.revision(), revision);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);
    }

    #[test]
    fn declarative_instruction_pose_cannot_be_replaced_by_an_executable_pose() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let step =
            declarative_instruction_step(InstructionStepId::new(), "説明専用", fingerprint.clone());
        let step_id = step.id;
        editor
            .execute(0, Command::AddInstructionStep { step })
            .expect("add declarative instruction");
        let before = editor.instruction_timeline.clone();

        assert_eq!(
            editor.execute(
                1,
                Command::ReplaceInstructionStepPose {
                    step_id,
                    pose: InstructionPose {
                        model: ori_domain::InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: fingerprint,
                        fixed_face: None,
                        hinge_angles: Vec::new(),
                    },
                },
            ),
            Err(CommandError::DeclarativeInstructionPoseImmutable)
        );
        assert_eq!(editor.instruction_timeline, before);
        assert_eq!(editor.revision(), 1);
    }

    #[test]
    fn each_instruction_command_has_an_exact_inverse() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let first_id = InstructionStepId::new();
        let second_id = InstructionStepId::new();
        let initial = InstructionTimeline {
            steps: vec![
                instruction_step(first_id, "手順 1", fingerprint.clone()),
                instruction_step(second_id, "手順 2", fingerprint),
            ],
        };
        editor.instruction_timeline = initial.clone();
        let commands = [
            Command::UpdateInstructionStepMetadata {
                step_id: first_id,
                title: "変更".to_owned(),
                description: "説明".to_owned(),
                caution: "注意".to_owned(),
                duration_ms: 3_000,
                visual: Default::default(),
            },
            Command::ReplaceInstructionStepPose {
                step_id: first_id,
                pose: editor.instruction_timeline.steps[1].pose.clone(),
            },
            Command::MoveInstructionStep {
                step_id: first_id,
                target_index: 1,
            },
            Command::RemoveInstructionStep { step_id: first_id },
            Command::AddInstructionStep {
                step: instruction_step(
                    InstructionStepId::new(),
                    "追加",
                    editor.fold_model_fingerprint_v1(),
                ),
            },
        ];

        for command in commands {
            let before = editor.instruction_timeline.clone();
            let revision = editor.revision();
            editor
                .execute(revision, command)
                .expect("execute instruction command");
            editor.undo(revision + 1).expect("undo instruction command");
            assert_eq!(editor.instruction_timeline, before);
        }
    }

    #[test]
    fn failed_instruction_validation_is_atomic_and_preserves_redo() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let valid = instruction_step(
            InstructionStepId::new(),
            "有効",
            editor.fold_model_fingerprint_v1(),
        );
        editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: valid.clone(),
                },
            )
            .expect("add valid instruction");
        editor.undo(1).expect("prepare redo history");

        let timeline_before = editor.instruction_timeline.clone();
        let pattern_before = editor.pattern.clone();
        let paper_before = editor.paper.clone();
        let revision_before = editor.revision;
        let undo_before = format!("{:?}", editor.undo_stack);
        let redo_before = format!("{:?}", editor.redo_stack);
        let invalid = instruction_step(
            InstructionStepId::new(),
            "",
            editor.fold_model_fingerprint_v1(),
        );

        assert!(matches!(
            editor.execute(2, Command::AddInstructionStep { step: invalid }),
            Err(CommandError::InstructionTimelineInvalid(_))
        ));
        assert_eq!(editor.instruction_timeline, timeline_before);
        assert_eq!(editor.pattern, pattern_before);
        assert_eq!(editor.paper, paper_before);
        assert_eq!(editor.revision, revision_before);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);

        editor.redo(2).expect("preserved redo remains usable");
        assert_eq!(editor.instruction_timeline.steps, vec![valid]);
    }

    #[test]
    fn instruction_reference_errors_and_move_bounds_are_atomic() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let step_id = InstructionStepId::new();
        let missing_id = InstructionStepId::new();
        let step = instruction_step(step_id, "手順", editor.fold_model_fingerprint_v1());
        editor
            .execute(0, Command::AddInstructionStep { step: step.clone() })
            .expect("add instruction");
        let before = editor.instruction_timeline.clone();

        let errors = [
            editor.execute(1, Command::AddInstructionStep { step }),
            editor.execute(
                1,
                Command::UpdateInstructionStepMetadata {
                    step_id: missing_id,
                    title: "なし".to_owned(),
                    description: String::new(),
                    caution: String::new(),
                    duration_ms: 1_500,
                    visual: Default::default(),
                },
            ),
            editor.execute(
                1,
                Command::ReplaceInstructionStepPose {
                    step_id: missing_id,
                    pose: before.steps[0].pose.clone(),
                },
            ),
            editor.execute(
                1,
                Command::RemoveInstructionStep {
                    step_id: missing_id,
                },
            ),
            editor.execute(
                1,
                Command::MoveInstructionStep {
                    step_id,
                    target_index: 1,
                },
            ),
        ];

        assert_eq!(
            errors[0],
            Err(CommandError::InstructionStepAlreadyExists(step_id))
        );
        for error in &errors[1..4] {
            assert_eq!(
                *error,
                Err(CommandError::InstructionStepNotFound(missing_id))
            );
        }
        assert_eq!(
            errors[4],
            Err(CommandError::InstructionStepTargetIndexOutOfBounds {
                target_index: 1,
                step_count: 1,
            })
        );
        assert_eq!(editor.instruction_timeline, before);
        assert_eq!(editor.revision(), 1);
        assert!(editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn metadata_and_pose_replacement_validate_the_whole_candidate_timeline() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let step_id = InstructionStepId::new();
        let step = instruction_step(step_id, "手順", editor.fold_model_fingerprint_v1());
        editor
            .execute(0, Command::AddInstructionStep { step })
            .expect("add instruction");
        let before = editor.instruction_timeline.clone();
        let undo_before = format!("{:?}", editor.undo_stack);

        assert!(matches!(
            editor.execute(
                1,
                Command::UpdateInstructionStepMetadata {
                    step_id,
                    title: String::new(),
                    description: String::new(),
                    caution: String::new(),
                    duration_ms: 1_500,
                    visual: Default::default(),
                },
            ),
            Err(CommandError::InstructionTimelineInvalid(_))
        ));
        let mut invalid_pose = before.steps[0].pose.clone();
        invalid_pose.source_model_fingerprint = "A".repeat(64);
        assert!(matches!(
            editor.execute(
                1,
                Command::ReplaceInstructionStepPose {
                    step_id,
                    pose: invalid_pose,
                },
            ),
            Err(CommandError::InstructionTimelineInvalid(_))
        ));

        assert_eq!(editor.instruction_timeline, before);
        assert_eq!(editor.revision(), 1);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert!(!editor.can_redo());
    }

    #[test]
    fn a_new_instruction_edit_clears_redo_history() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let first = instruction_step(InstructionStepId::new(), "手順 1", fingerprint.clone());
        let second = instruction_step(InstructionStepId::new(), "手順 2", fingerprint);

        editor
            .execute(0, Command::AddInstructionStep { step: first })
            .expect("add first instruction");
        editor.undo(1).expect("prepare redo");
        assert!(editor.can_redo());
        editor
            .execute(
                2,
                Command::AddInstructionStep {
                    step: second.clone(),
                },
            )
            .expect("branch with second instruction");

        assert!(!editor.can_redo());
        assert_eq!(editor.instruction_timeline.steps, vec![second]);
    }

    #[test]
    fn editing_instructions_does_not_change_the_fold_model_fingerprint() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let step_id = InstructionStepId::new();
        let step = instruction_step(step_id, "手順", fingerprint.clone());

        editor
            .execute(0, Command::AddInstructionStep { step })
            .expect("add instruction");
        assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);
        editor
            .execute(
                1,
                Command::UpdateInstructionStepMetadata {
                    step_id,
                    title: "更新".to_owned(),
                    description: "説明".to_owned(),
                    caution: String::new(),
                    duration_ms: 2_000,
                    visual: Default::default(),
                },
            )
            .expect("update instruction");
        assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);
    }

    #[test]
    fn geometry_undo_restores_the_previous_fold_model_fingerprint() {
        let vertex_id = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![Vertex {
                id: vertex_id,
                position: Point2::new(1.0, 2.0),
            }],
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(pattern);
        let original = editor.fold_model_fingerprint_v1();

        editor
            .execute(
                0,
                Command::MoveVertex {
                    id: vertex_id,
                    position: Point2::new(3.0, 4.0),
                },
            )
            .expect("move vertex");
        let moved = editor.fold_model_fingerprint_v1();
        assert_ne!(moved, original);

        editor.undo(1).expect("undo vertex move");
        assert_eq!(editor.fold_model_fingerprint_v1(), original);
        editor.redo(2).expect("redo vertex move");
        assert_eq!(editor.fold_model_fingerprint_v1(), moved);
    }

    #[test]
    fn failed_instruction_redo_preserves_state_and_history_for_retry() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let step = instruction_step(
            InstructionStepId::new(),
            "手順",
            editor.fold_model_fingerprint_v1(),
        );
        editor
            .execute(0, Command::AddInstructionStep { step: step.clone() })
            .expect("add instruction");
        editor.undo(1).expect("place instruction in redo history");

        editor.instruction_timeline.steps.push(step.clone());
        let timeline_before = editor.instruction_timeline.clone();
        let revision_before = editor.revision;
        let undo_before = format!("{:?}", editor.undo_stack);
        let redo_before = format!("{:?}", editor.redo_stack);

        assert_eq!(
            editor.redo(revision_before),
            Err(CommandError::InstructionStepAlreadyExists(step.id))
        );
        assert_eq!(editor.instruction_timeline, timeline_before);
        assert_eq!(editor.revision, revision_before);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);

        editor.instruction_timeline.steps.clear();
        editor
            .redo(revision_before)
            .expect("preserved instruction redo remains retryable");
        assert_eq!(editor.instruction_timeline.steps, vec![step]);
    }

    #[test]
    fn instruction_history_inverses_retain_only_the_changed_delta() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let first_id = InstructionStepId::new();
        let second_id = InstructionStepId::new();
        editor.instruction_timeline = InstructionTimeline {
            steps: vec![
                instruction_step(first_id, "手順 1", fingerprint.clone()),
                instruction_step(second_id, "手順 2", fingerprint.clone()),
            ],
        };

        let added = instruction_step(InstructionStepId::new(), "手順 3", fingerprint);
        editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: added.clone(),
                },
            )
            .expect("add instruction");
        assert!(matches!(
            editor.undo_stack.last().map(|entry| &entry.inverse),
            Some(Inverse::RemoveAddedInstructionStep { step_id }) if *step_id == added.id
        ));

        editor
            .execute(
                1,
                Command::UpdateInstructionStepMetadata {
                    step_id: first_id,
                    title: "更新".to_owned(),
                    description: "説明".to_owned(),
                    caution: "注意".to_owned(),
                    duration_ms: 2_000,
                    visual: Default::default(),
                },
            )
            .expect("update instruction metadata");
        assert!(matches!(
            editor.undo_stack.last().map(|entry| &entry.inverse),
            Some(Inverse::RestoreInstructionStepMetadata {
                step_id,
                title,
                description,
                caution,
                duration_ms,
                ..
            }) if *step_id == first_id
                && title == "手順 1"
                && description.is_empty()
                && caution.is_empty()
                && *duration_ms == 1_500
        ));

        let replacement_pose = editor.instruction_timeline.steps[1].pose.clone();
        editor
            .execute(
                2,
                Command::ReplaceInstructionStepPose {
                    step_id: first_id,
                    pose: replacement_pose,
                },
            )
            .expect("replace instruction pose");
        assert!(matches!(
            editor.undo_stack.last().map(|entry| &entry.inverse),
            Some(Inverse::RestoreInstructionStepPose { step_id, .. })
                if *step_id == first_id
        ));

        editor
            .execute(3, Command::RemoveInstructionStep { step_id: second_id })
            .expect("remove instruction");
        assert!(matches!(
            editor.undo_stack.last().map(|entry| &entry.inverse),
            Some(Inverse::RestoreRemovedInstructionStep { index: 1, step })
                if step.id == second_id
        ));

        editor
            .execute(
                4,
                Command::MoveInstructionStep {
                    step_id: added.id,
                    target_index: 0,
                },
            )
            .expect("move instruction");
        assert!(matches!(
            editor.undo_stack.last().map(|entry| &entry.inverse),
            Some(Inverse::RestoreInstructionStepOrder {
                step_id,
                previous_index: 1,
            }) if *step_id == added.id
        ));
    }

    #[test]
    fn maximum_timeline_metadata_history_retains_no_hinge_vectors() {
        use ori_domain::{
            FaceId, InstructionHingeAngle, InstructionPoseModel, MAX_INSTRUCTION_HINGE_RECORDS,
            MAX_INSTRUCTION_HINGES_PER_STEP,
        };

        let mut hinge_angles = (0..MAX_INSTRUCTION_HINGES_PER_STEP)
            .map(|index| InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: (index % 181) as f64,
            })
            .collect::<Vec<_>>();
        hinge_angles.sort_by_key(|hinge| hinge.edge.canonical_bytes());
        let mut steps = Vec::new();
        let mut remaining = MAX_INSTRUCTION_HINGE_RECORDS;
        while remaining > 0 {
            let count = remaining.min(MAX_INSTRUCTION_HINGES_PER_STEP);
            steps.push(InstructionStep {
                id: InstructionStepId::new(),
                title: format!("手順 {}", steps.len() + 1),
                description: String::new(),
                caution: String::new(),
                duration_ms: 1_500,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: "0".repeat(64),
                    fixed_face: Some(FaceId::new()),
                    hinge_angles: hinge_angles[..count].to_vec(),
                },
            });
            remaining -= count;
        }
        let timeline = InstructionTimeline { steps };
        validate_instruction_timeline(&timeline).expect("maximum timeline must be valid");
        assert_eq!(
            timeline
                .steps
                .iter()
                .map(|step| step.pose.hinge_angles.len())
                .sum::<usize>(),
            MAX_INSTRUCTION_HINGE_RECORDS
        );
        let step_id = timeline.steps[0].id;
        let mut editor =
            EditorState::with_document_parts(CreasePattern::empty(), Paper::default(), timeline);

        for update in 0..MAX_EDITOR_HISTORY_ENTRIES + 16 {
            editor
                .execute(
                    editor.revision(),
                    Command::UpdateInstructionStepMetadata {
                        step_id,
                        title: format!("更新 {update}"),
                        description: String::new(),
                        caution: String::new(),
                        duration_ms: 1_500,
                        visual: Default::default(),
                    },
                )
                .expect("update metadata on maximum timeline");
        }

        assert_eq!(editor.undo_stack.len(), MAX_EDITOR_HISTORY_ENTRIES);
        assert!(editor.undo_stack.iter().all(|entry| {
            matches!(
                (&entry.forward, &entry.inverse),
                (
                    Command::UpdateInstructionStepMetadata { .. },
                    Inverse::RestoreInstructionStepMetadata { .. },
                )
            )
        }));
        assert!(
            !format!("{:?}", editor.undo_stack).contains("hinge_angles"),
            "metadata history must not retain any complete step pose or timeline"
        );
    }

    #[test]
    fn editor_history_discards_the_oldest_entry_after_the_fixed_limit() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let command_count = MAX_EDITOR_HISTORY_ENTRIES + 17;
        let mut vertex_ids = Vec::with_capacity(command_count);
        for index in 0..command_count {
            let id = VertexId::new();
            vertex_ids.push(id);
            editor
                .execute(
                    editor.revision(),
                    Command::AddVertex {
                        id,
                        position: Point2::new(index as f64, 0.0),
                    },
                )
                .expect("add history fixture vertex");
        }

        assert_eq!(editor.undo_stack.len(), MAX_EDITOR_HISTORY_ENTRIES);
        assert!(matches!(
            &editor.undo_stack[0].forward,
            Command::AddVertex { id, .. } if *id == vertex_ids[17]
        ));

        for _ in 0..MAX_EDITOR_HISTORY_ENTRIES {
            editor
                .undo(editor.revision())
                .expect("undo retained history entry");
        }
        assert!(!editor.can_undo());
        assert_eq!(editor.redo_stack.len(), MAX_EDITOR_HISTORY_ENTRIES);
        assert_eq!(
            editor
                .pattern
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            vertex_ids[..17]
        );

        for _ in 0..MAX_EDITOR_HISTORY_ENTRIES {
            editor
                .redo(editor.revision())
                .expect("redo retained history entry");
        }
        assert_eq!(editor.undo_stack.len(), MAX_EDITOR_HISTORY_ENTRIES);
        assert!(!editor.can_redo());
        assert_eq!(editor.pattern.vertices.len(), command_count);
    }

    #[test]
    fn every_editor_constructor_uses_the_default_history_limit_and_clone_preserves_it() {
        let pattern = CreasePattern::empty();
        let paper = Paper::default();
        let timeline = InstructionTimeline::default();
        let constraints = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: Vec::new(),
        };

        assert_eq!(
            EditorState::new(pattern.clone()).history_entry_limit(),
            MAX_EDITOR_HISTORY_ENTRIES
        );
        assert_eq!(
            EditorState::with_paper(pattern.clone(), paper.clone()).history_entry_limit(),
            MAX_EDITOR_HISTORY_ENTRIES
        );
        assert_eq!(
            EditorState::with_document_parts(pattern.clone(), paper.clone(), timeline.clone())
                .history_entry_limit(),
            MAX_EDITOR_HISTORY_ENTRIES
        );
        assert_eq!(
            EditorState::with_document_parts_and_constraints(
                pattern,
                paper,
                timeline,
                constraints,
            )
            .history_entry_limit(),
            MAX_EDITOR_HISTORY_ENTRIES
        );

        let mut configured = EditorState::new(CreasePattern::empty());
        configured
            .set_history_entry_limit(7)
            .expect("valid history limit");
        assert_eq!(configured.clone().history_entry_limit(), 7);
    }

    #[test]
    fn setting_history_limit_trims_both_stacks_from_the_oldest_side_without_touching_state() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let vertex_ids = (0..6)
            .map(|index| {
                let id = VertexId::new();
                editor
                    .execute(
                        editor.revision(),
                        Command::AddVertex {
                            id,
                            position: Point2::new(f64::from(index), 0.0),
                        },
                    )
                    .expect("add history fixture vertex");
                id
            })
            .collect::<Vec<_>>();
        editor
            .undo(editor.revision())
            .expect("create first redo entry");
        editor
            .undo(editor.revision())
            .expect("create second redo entry");
        let pose = runtime_pose(15.0);
        editor.adopt_current_applied_pose(pose.clone());
        let document_before = (
            editor.pattern.clone(),
            editor.paper.clone(),
            editor.geometric_constraints.clone(),
            editor.instruction_timeline.clone(),
        );
        let revision_before = editor.revision();

        editor
            .set_history_entry_limit(1)
            .expect("minimum history limit is valid");

        assert_eq!(editor.history_entry_limit(), 1);
        assert_eq!(editor.undo_stack.len(), 1);
        assert_eq!(editor.redo_stack.len(), 1);
        assert!(matches!(
            &editor.undo_stack[0].forward,
            Command::AddVertex { id, .. } if *id == vertex_ids[3]
        ));
        assert!(matches!(
            &editor.redo_stack[0].forward,
            Command::AddVertex { id, .. } if *id == vertex_ids[4]
        ));
        assert_eq!(
            (
                editor.pattern.clone(),
                editor.paper.clone(),
                editor.geometric_constraints.clone(),
                editor.instruction_timeline.clone(),
            ),
            document_before
        );
        assert_eq!(editor.revision(), revision_before);
        assert_eq!(editor.current_applied_pose(), Some(&pose));
    }

    #[test]
    fn increasing_history_limit_does_not_restore_trimmed_entries() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .set_history_entry_limit(2)
            .expect("small history limit");
        let mut vertex_ids = Vec::new();
        for index in 0..5 {
            let id = VertexId::new();
            vertex_ids.push(id);
            editor
                .execute(
                    editor.revision(),
                    Command::AddVertex {
                        id,
                        position: Point2::new(f64::from(index), 0.0),
                    },
                )
                .expect("add history fixture vertex");
        }
        assert_eq!(editor.undo_stack.len(), 2);
        assert!(matches!(
            &editor.undo_stack[0].forward,
            Command::AddVertex { id, .. } if *id == vertex_ids[3]
        ));

        editor
            .set_history_entry_limit(4)
            .expect("increased history limit");
        assert_eq!(editor.undo_stack.len(), 2);
        for index in 5..8 {
            let id = VertexId::new();
            vertex_ids.push(id);
            editor
                .execute(
                    editor.revision(),
                    Command::AddVertex {
                        id,
                        position: Point2::new(f64::from(index), 0.0),
                    },
                )
                .expect("add post-increase history fixture vertex");
        }

        assert_eq!(editor.undo_stack.len(), 4);
        assert!(matches!(
            &editor.undo_stack[0].forward,
            Command::AddVertex { id, .. } if *id == vertex_ids[4]
        ));
        for _ in 0..4 {
            editor
                .undo(editor.revision())
                .expect("undo retained history");
        }
        assert!(!editor.can_undo());
        assert_eq!(editor.redo_stack.len(), 4);
        assert_eq!(
            editor
                .pattern
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            vertex_ids[..4]
        );
    }

    #[test]
    fn execute_undo_and_redo_pushes_all_use_the_instance_history_limit() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .set_history_entry_limit(2)
            .expect("small history limit");
        for index in 0..4 {
            editor
                .execute(
                    editor.revision(),
                    Command::AddVertex {
                        id: VertexId::new(),
                        position: Point2::new(f64::from(index), 0.0),
                    },
                )
                .expect("add history fixture vertex");
        }
        assert_eq!(editor.undo_stack.len(), 2);

        editor.undo(editor.revision()).expect("first undo");
        editor.undo(editor.revision()).expect("second undo");
        assert_eq!(editor.redo_stack.len(), 2);

        editor.redo(editor.revision()).expect("first redo");
        editor.redo(editor.revision()).expect("second redo");
        assert_eq!(editor.undo_stack.len(), 2);
        assert!(!editor.can_redo());
    }

    #[test]
    fn invalid_history_limits_are_atomic_at_both_boundaries() {
        let mut editor = EditorState::new(CreasePattern::empty());
        for index in 0..3 {
            editor
                .execute(
                    editor.revision(),
                    Command::AddVertex {
                        id: VertexId::new(),
                        position: Point2::new(f64::from(index), 0.0),
                    },
                )
                .expect("add history fixture vertex");
        }
        editor.undo(editor.revision()).expect("create redo history");
        editor.adopt_current_applied_pose(runtime_pose(25.0));
        let before = editor_state_snapshot(&editor);

        for requested in [0, MAX_EDITOR_HISTORY_ENTRIES + 1] {
            assert_eq!(
                editor.set_history_entry_limit(requested),
                Err(HistoryEntryLimitError::OutOfRange {
                    requested,
                    minimum: 1,
                    maximum: MAX_EDITOR_HISTORY_ENTRIES,
                })
            );
            assert_eq!(editor_state_snapshot(&editor), before);
        }
    }

    #[test]
    fn history_limit_accepts_both_inclusive_boundaries() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .set_history_entry_limit(1)
            .expect("minimum history limit");
        assert_eq!(editor.history_entry_limit(), 1);

        editor
            .set_history_entry_limit(MAX_EDITOR_HISTORY_ENTRIES)
            .expect("maximum history limit");
        assert_eq!(editor.history_entry_limit(), MAX_EDITOR_HISTORY_ENTRIES);
    }

    fn runtime_pose(angle_degrees: f64) -> crate::AppliedPoseV1 {
        use ori_domain::FaceId;

        let mut faces = [FaceId::new(), FaceId::new()];
        faces.sort_by_key(FaceId::canonical_bytes);
        let hinge = EdgeId::new();
        crate::prepare_applied_pose_v1(
            &faces,
            &[hinge],
            Some(faces[0]),
            &[(hinge, angle_degrees)],
            crate::AppliedPoseLimitsV1::default(),
        )
        .expect("runtime pose fixture")
    }

    #[test]
    fn stacked_fold_document_is_one_atomic_history_entry() {
        let sheet = crate::create_rectangular_sheet(80.0, 60.0, false).unwrap();
        let (source_pattern, mut source_paper) = sheet.into_parts();
        source_paper.thickness_mm = 0.1;
        let mut editor = EditorState::with_paper(source_pattern.clone(), source_paper.clone());
        let mut target_pattern = source_pattern.clone();
        let hinge = EdgeId::new();
        target_pattern.edges.push(Edge {
            id: hinge,
            start: source_paper.boundary_vertices[0],
            end: source_paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let face = FaceId::new();
        let timeline = InstructionTimeline {
            steps: vec![InstructionStep {
                id: InstructionStepId::new(),
                title: "Stacked fold".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: ori_domain::MIN_INSTRUCTION_DURATION_MS,
                visual: InstructionVisual::default(),
                pose: InstructionPose {
                    model: ori_domain::InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint:
                        crate::fold_model_fingerprint::fold_model_fingerprint_v1(
                            &target_pattern,
                            &source_paper,
                        ),
                    fixed_face: Some(face),
                    hinge_angles: vec![ori_domain::InstructionHingeAngle {
                        edge: hinge,
                        angle_degrees: 90.0,
                    }],
                },
            }],
        };
        let after_pose = runtime_pose(90.0);
        let before_failure = editor_state_snapshot(&editor);
        let mut one_ulp_thickness = source_paper.clone();
        one_ulp_thickness.thickness_mm = f64::from_bits(source_paper.thickness_mm.to_bits() + 1);
        assert_eq!(
            editor.execute_stacked_fold_document(
                0,
                target_pattern.clone(),
                one_ulp_thickness,
                timeline.clone(),
                ProjectLayerDocumentV1::default(),
                after_pose.clone(),
            ),
            Err(CommandError::InvalidStackedFoldDocument)
        );
        assert_eq!(editor_state_snapshot(&editor), before_failure);
        let mut oversized_timeline = InstructionTimeline::default();
        for index in 0..32 {
            let mut step = timeline.steps[0].clone();
            step.id = InstructionStepId::new();
            step.title = format!("Stacked fold {index}");
            oversized_timeline.steps.push(step);
        }
        assert_eq!(
            editor.execute_stacked_fold_document(
                0,
                target_pattern.clone(),
                source_paper.clone(),
                oversized_timeline,
                ProjectLayerDocumentV1::default(),
                after_pose.clone(),
            ),
            Err(CommandError::InvalidStackedFoldDocument)
        );
        assert_eq!(editor_state_snapshot(&editor), before_failure);
        editor
            .execute_stacked_fold_document(
                0,
                target_pattern.clone(),
                source_paper.clone(),
                timeline.clone(),
                ProjectLayerDocumentV1::default(),
                after_pose.clone(),
            )
            .expect("atomic stacked fold");
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.pattern(), &target_pattern);
        assert_eq!(editor.instruction_timeline(), &timeline);
        assert_eq!(editor.current_applied_pose(), Some(&after_pose));
        assert!(editor.can_undo());

        editor.undo(1).expect("undo whole stacked fold");
        assert_eq!(editor.pattern(), &source_pattern);
        assert!(editor.instruction_timeline().steps.is_empty());
        assert!(editor.current_applied_pose().is_none());

        editor.redo(2).expect("redo whole stacked fold");
        assert_eq!(editor.pattern(), &target_pattern);
        assert_eq!(editor.instruction_timeline(), &timeline);
        assert_eq!(editor.current_applied_pose(), Some(&after_pose));

        let mut pose_editor = EditorState::with_paper(target_pattern.clone(), source_paper.clone());
        let before_pose = runtime_pose(0.0);
        pose_editor.adopt_current_applied_pose(before_pose.clone());
        pose_editor
            .execute_stacked_fold_document(
                0,
                target_pattern.clone(),
                source_paper,
                timeline.clone(),
                ProjectLayerDocumentV1::default(),
                after_pose.clone(),
            )
            .expect("atomic already-cyclic pose timeline");
        assert_eq!(pose_editor.current_applied_pose(), Some(&after_pose));
        pose_editor.undo(1).unwrap();
        assert_eq!(pose_editor.current_applied_pose(), Some(&before_pose));
        assert!(pose_editor.instruction_timeline().steps.is_empty());
        pose_editor.redo(2).unwrap();
        assert_eq!(pose_editor.current_applied_pose(), Some(&after_pose));
        assert_eq!(pose_editor.instruction_timeline(), &timeline);
    }

    #[test]
    fn stacked_fold_source_edge_requires_exact_contiguous_subdivision() {
        let start = VertexId::new();
        let middle = VertexId::new();
        let end = VertexId::new();
        let source_edge = EdgeId::new();
        let source = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(10.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: source_edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        };
        let target = CreasePattern {
            vertices: vec![
                source.vertices[0].clone(),
                source.vertices[1].clone(),
                Vertex {
                    id: middle,
                    position: Point2::new(5.0, 0.0),
                },
            ],
            edges: vec![
                Edge {
                    id: EdgeId::new(),
                    start,
                    end: middle,
                    kind: EdgeKind::Mountain,
                },
                Edge {
                    id: EdgeId::new(),
                    start: middle,
                    end,
                    kind: EdgeKind::Mountain,
                },
            ],
        };
        assert!(source_edges_preserved_by_exact_subdivision(
            &source, &target
        ));

        let mut off_line = target;
        off_line
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == middle)
            .unwrap()
            .position = Point2::new(5.0, f64::EPSILON);
        assert!(!source_edges_preserved_by_exact_subdivision(
            &source, &off_line
        ));
    }

    #[test]
    fn new_and_loaded_editors_start_without_runtime_pose() {
        let pattern = CreasePattern::empty();
        let paper = Paper::default();
        let timeline = InstructionTimeline::default();

        assert!(
            EditorState::new(pattern.clone())
                .current_applied_pose()
                .is_none()
        );
        assert!(
            EditorState::with_paper(pattern.clone(), paper.clone())
                .current_applied_pose()
                .is_none()
        );
        assert!(
            EditorState::with_document_parts(pattern, paper, timeline)
                .current_applied_pose()
                .is_none()
        );
    }

    #[test]
    fn pose_only_adopt_and_clear_do_not_change_document_revision_or_history() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let document_before = (
            editor.pattern.clone(),
            editor.paper.clone(),
            editor.instruction_timeline.clone(),
        );
        let revision_before = editor.revision();
        let undo_before = format!("{:?}", editor.undo_stack);
        let redo_before = format!("{:?}", editor.redo_stack);
        let pose = runtime_pose(15.0);

        assert!(editor.adopt_current_applied_pose(pose.clone()).is_none());
        assert_eq!(editor.current_applied_pose(), Some(&pose));
        assert_eq!(
            (
                editor.pattern.clone(),
                editor.paper.clone(),
                editor.instruction_timeline.clone(),
            ),
            document_before
        );
        assert_eq!(editor.revision(), revision_before);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);

        assert_eq!(editor.clear_current_applied_pose(), Some(pose));
        assert!(editor.current_applied_pose().is_none());
        assert_eq!(editor.revision(), revision_before);
        assert_eq!(format!("{:?}", editor.undo_stack), undo_before);
        assert_eq!(format!("{:?}", editor.redo_stack), redo_before);
    }

    #[test]
    fn geometry_history_dynamically_captures_both_pose_sides() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        let original_pose = runtime_pose(10.0);
        let after_pose = runtime_pose(20.0);
        let replacement_before_pose = runtime_pose(30.0);
        editor.adopt_current_applied_pose(original_pose.clone());

        editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("geometry edit");
        assert!(editor.current_applied_pose().is_none());

        editor.adopt_current_applied_pose(after_pose.clone());
        editor.undo(1).expect("undo geometry");
        assert_eq!(editor.current_applied_pose(), Some(&original_pose));

        editor.adopt_current_applied_pose(replacement_before_pose.clone());
        editor.redo(2).expect("redo geometry");
        assert_eq!(editor.current_applied_pose(), Some(&after_pose));

        editor.undo(3).expect("undo geometry again");
        assert_eq!(
            editor.current_applied_pose(),
            Some(&replacement_before_pose)
        );
    }

    #[test]
    fn paper_properties_always_preserve_semantic_pose() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let first_pose = runtime_pose(40.0);
        let second_pose = runtime_pose(50.0);
        editor.adopt_current_applied_pose(first_pose.clone());

        editor
            .execute(
                0,
                Command::UpdatePaperProperties {
                    thickness_mm: 3.0,
                    front_color: RgbaColor::opaque(1, 2, 3),
                    back_color: RgbaColor::opaque(4, 5, 6),
                    front_texture_asset: None,
                    back_texture_asset: None,
                    cutting_allowed: true,
                },
            )
            .expect("paper settings");
        assert_eq!(editor.current_applied_pose(), Some(&first_pose));

        editor.adopt_current_applied_pose(second_pose.clone());
        editor.undo(1).expect("undo paper settings");
        assert_eq!(editor.current_applied_pose(), Some(&second_pose));
        editor.redo(2).expect("redo paper settings");
        assert_eq!(editor.current_applied_pose(), Some(&second_pose));

        editor
            .execute(3, Command::SetCuttingAllowed { allowed: false })
            .expect("cutting setting");
        assert_eq!(editor.current_applied_pose(), Some(&second_pose));

        editor
            .execute(
                4,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::Inch,
                },
            )
            .expect("length display setting");
        assert_eq!(editor.current_applied_pose(), Some(&second_pose));
        editor.undo(5).expect("undo length display setting");
        assert_eq!(editor.current_applied_pose(), Some(&second_pose));
    }

    #[test]
    fn instruction_history_preserves_the_latest_adopted_pose() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let first_pose = runtime_pose(60.0);
        let second_pose = runtime_pose(70.0);
        let third_pose = runtime_pose(80.0);
        editor.adopt_current_applied_pose(first_pose.clone());
        let step = instruction_step(
            InstructionStepId::new(),
            "手順",
            editor.fold_model_fingerprint_v1(),
        );

        editor
            .execute(0, Command::AddInstructionStep { step })
            .expect("instruction edit");
        assert_eq!(editor.current_applied_pose(), Some(&first_pose));

        editor.adopt_current_applied_pose(second_pose.clone());
        editor.undo(1).expect("undo instruction");
        assert_eq!(editor.current_applied_pose(), Some(&second_pose));
        editor.adopt_current_applied_pose(third_pose.clone());
        editor.redo(2).expect("redo instruction");
        assert_eq!(editor.current_applied_pose(), Some(&third_pose));
    }

    #[test]
    fn true_geometry_noop_preserves_the_latest_pose_across_history() {
        let vertex = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![Vertex {
                id: vertex,
                position: Point2::new(-0.0, 2.0),
            }],
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(pattern);
        let first_pose = runtime_pose(90.0);
        let latest_pose = runtime_pose(100.0);
        editor.adopt_current_applied_pose(first_pose.clone());

        editor
            .execute(
                0,
                Command::MoveVertex {
                    id: vertex,
                    position: Point2::new(-0.0, 2.0),
                },
            )
            .expect("bit-exact no-op");
        assert_eq!(editor.current_applied_pose(), Some(&first_pose));

        editor.adopt_current_applied_pose(latest_pose.clone());
        editor.undo(1).expect("undo no-op");
        assert_eq!(editor.current_applied_pose(), Some(&latest_pose));
        editor.redo(2).expect("redo no-op");
        assert_eq!(editor.current_applied_pose(), Some(&latest_pose));
    }

    #[test]
    fn failed_and_revision_exhausted_operations_preserve_pose_and_history() {
        let pose = runtime_pose(110.0);
        let mut stale_editor = EditorState::new(CreasePattern::empty());
        stale_editor.adopt_current_applied_pose(pose.clone());
        let stale_before = editor_state_snapshot(&stale_editor);
        assert!(matches!(
            stale_editor.execute(
                99,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(0.0, 0.0),
                }
            ),
            Err(CommandError::RevisionConflict { .. })
        ));
        assert_eq!(editor_state_snapshot(&stale_editor), stale_before);

        let mut invalid_editor = EditorState::new(CreasePattern::empty());
        invalid_editor.adopt_current_applied_pose(pose.clone());
        let invalid_before = editor_state_snapshot(&invalid_editor);
        let missing_vertex = VertexId::new();
        assert_eq!(
            invalid_editor.execute(
                0,
                Command::MoveVertex {
                    id: missing_vertex,
                    position: Point2::new(0.0, 0.0),
                }
            ),
            Err(CommandError::VertexNotFound(missing_vertex))
        );
        assert_eq!(editor_state_snapshot(&invalid_editor), invalid_before);

        let mut exhausted_editor = EditorState::new(CreasePattern::empty());
        exhausted_editor.adopt_current_applied_pose(pose);
        exhausted_editor.revision = MAX_REVISION;
        let exhausted_before = editor_state_snapshot(&exhausted_editor);
        assert!(matches!(
            exhausted_editor.execute(
                MAX_REVISION,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(0.0, 0.0),
                }
            ),
            Err(CommandError::RevisionExhausted { .. })
        ));
        assert_eq!(editor_state_snapshot(&exhausted_editor), exhausted_before);
    }

    #[test]
    fn every_instruction_variant_preserves_the_latest_pose_through_history() {
        let mut base = EditorState::new(CreasePattern::empty());
        let fingerprint = base.fold_model_fingerprint_v1();
        let first_id = InstructionStepId::new();
        let second_id = InstructionStepId::new();
        let first = instruction_step(first_id, "手順 1", fingerprint.clone());
        let second = instruction_step(second_id, "手順 2", fingerprint.clone());
        base.instruction_timeline = InstructionTimeline {
            steps: vec![first.clone(), second.clone()],
        };
        let commands = [
            Command::AddInstructionStep {
                step: instruction_step(InstructionStepId::new(), "追加", fingerprint),
            },
            Command::UpdateInstructionStepMetadata {
                step_id: first_id,
                title: "更新".to_owned(),
                description: "説明".to_owned(),
                caution: "注意".to_owned(),
                duration_ms: 2_000,
                visual: Default::default(),
            },
            Command::ReplaceInstructionStepPose {
                step_id: first_id,
                pose: second.pose,
            },
            Command::RemoveInstructionStep { step_id: first_id },
            Command::MoveInstructionStep {
                step_id: first_id,
                target_index: 1,
            },
        ];

        for (index, command) in commands.into_iter().enumerate() {
            let mut editor = base.clone();
            let initial = runtime_pose(10.0 + index as f64);
            let after_execute = runtime_pose(30.0 + index as f64);
            let after_undo = runtime_pose(50.0 + index as f64);
            editor.adopt_current_applied_pose(initial.clone());

            editor
                .execute(0, command)
                .expect("execute instruction variant");
            assert_eq!(editor.current_applied_pose(), Some(&initial));

            editor.adopt_current_applied_pose(after_execute.clone());
            editor.undo(1).expect("undo instruction variant");
            assert_eq!(editor.current_applied_pose(), Some(&after_execute));

            editor.adopt_current_applied_pose(after_undo.clone());
            editor.redo(2).expect("redo instruction variant");
            assert_eq!(editor.current_applied_pose(), Some(&after_undo));
        }
    }

    #[test]
    fn failed_undo_and_redo_leave_runtime_pose_and_history_exactly_unchanged() {
        let vertex = VertexId::new();
        let blocker = VertexId::new();
        let blocking_edge = EdgeId::new();
        let mut undo_editor = EditorState::new(CreasePattern::empty());
        undo_editor.adopt_current_applied_pose(runtime_pose(10.0));
        undo_editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("prepare undo");
        undo_editor.pattern.vertices.push(Vertex {
            id: blocker,
            position: Point2::new(1.0, 0.0),
        });
        undo_editor.pattern.edges.push(Edge {
            id: blocking_edge,
            start: vertex,
            end: blocker,
            kind: EdgeKind::Auxiliary,
        });
        undo_editor.adopt_current_applied_pose(runtime_pose(20.0));
        let undo_before = editor_state_snapshot(&undo_editor);

        assert_eq!(
            undo_editor.undo(1),
            Err(CommandError::VertexHasConnectedEdge {
                vertex,
                edge: blocking_edge,
            })
        );
        assert_eq!(editor_state_snapshot(&undo_editor), undo_before);

        let mut redo_editor = EditorState::new(CreasePattern::empty());
        redo_editor.adopt_current_applied_pose(runtime_pose(30.0));
        redo_editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("prepare redo");
        redo_editor.undo(1).expect("place entry on redo stack");
        redo_editor.pattern.vertices.push(Vertex {
            id: vertex,
            position: Point2::new(9.0, 9.0),
        });
        redo_editor.adopt_current_applied_pose(runtime_pose(40.0));
        let redo_before = editor_state_snapshot(&redo_editor);

        assert_eq!(
            redo_editor.redo(2),
            Err(CommandError::VertexAlreadyExists(vertex))
        );
        assert_eq!(editor_state_snapshot(&redo_editor), redo_before);
    }

    #[test]
    fn revision_exhausted_undo_and_redo_preserve_some_pose_and_history() {
        let vertex = VertexId::new();
        let mut undo_editor = EditorState::new(CreasePattern::empty());
        undo_editor.adopt_current_applied_pose(runtime_pose(10.0));
        undo_editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("prepare undo");
        undo_editor.adopt_current_applied_pose(runtime_pose(20.0));
        undo_editor.revision = MAX_REVISION;
        let undo_before = editor_state_snapshot(&undo_editor);
        assert_eq!(
            undo_editor.undo(MAX_REVISION),
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION,
            })
        );
        assert_eq!(editor_state_snapshot(&undo_editor), undo_before);

        let mut redo_editor = EditorState::new(CreasePattern::empty());
        redo_editor.adopt_current_applied_pose(runtime_pose(30.0));
        redo_editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("prepare redo");
        redo_editor.undo(1).expect("place entry on redo stack");
        redo_editor.adopt_current_applied_pose(runtime_pose(40.0));
        redo_editor.revision = MAX_REVISION;
        let redo_before = editor_state_snapshot(&redo_editor);
        assert_eq!(
            redo_editor.redo(MAX_REVISION),
            Err(CommandError::RevisionExhausted {
                revision: MAX_REVISION,
            })
        );
        assert_eq!(editor_state_snapshot(&redo_editor), redo_before);
    }

    #[test]
    fn empty_history_undo_and_redo_preserve_some_pose_and_revision() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor.adopt_current_applied_pose(runtime_pose(70.0));
        let before = editor_state_snapshot(&editor);

        let undo = editor.undo(0).expect("empty undo is a no-op");
        assert_eq!(undo.revision, 0);
        assert!(!undo.constraints_changed);
        assert_eq!(editor_state_snapshot(&editor), before);
        let redo = editor.redo(0).expect("empty redo is a no-op");
        assert_eq!(redo.revision, 0);
        assert!(!redo.constraints_changed);
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn geometric_constraint_add_remove_and_history_preserve_id_and_order() {
        let (mut editor, pattern, _) = rectangular_editor();
        let first = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[4].id,
            },
        };
        let second = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Vertical {
                edge: pattern.edges[0].id,
            },
        };

        editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: first.clone(),
                },
            )
            .expect("add first constraint");
        editor
            .execute(
                1,
                Command::AddGeometricConstraint {
                    record: second.clone(),
                },
            )
            .expect("add second constraint");
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![first.clone(), second.clone()]
        );

        editor
            .execute(2, Command::RemoveGeometricConstraint { id: first.id })
            .expect("remove first constraint");
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![second.clone()]
        );
        editor.undo(3).expect("restore first at its original index");
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![first.clone(), second.clone()]
        );
        editor.undo(4).expect("undo second add");
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![first.clone()]
        );
        editor.redo(5).expect("redo second add");
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![first, second]
        );
        assert_eq!(editor.revision(), 6);
    }

    #[test]
    fn persisted_geometric_constraints_restore_without_history() {
        let (_, pattern, paper) = rectangular_editor();
        let constraints = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal {
                    edge: pattern.edges[4].id,
                },
            }],
        };
        let editor = EditorState::with_document_parts_and_constraints(
            pattern,
            paper,
            InstructionTimeline::default(),
            constraints.clone(),
        );

        assert_eq!(editor.geometric_constraints(), &constraints);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn constraint_commands_reject_duplicate_and_missing_references_atomically() {
        let (mut editor, pattern, _) = rectangular_editor();
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[4].id,
            },
        };
        editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            )
            .expect("add fixture constraint");

        let before_duplicate = editor_state_snapshot(&editor);
        assert_eq!(
            editor.execute(
                1,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            ),
            Err(CommandError::GeometricConstraintAlreadyExists(record.id))
        );
        assert_eq!(editor_state_snapshot(&editor), before_duplicate);

        let missing_edge = EdgeId::new();
        let missing = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Vertical { edge: missing_edge },
        };
        let before_missing = editor_state_snapshot(&editor);
        assert_eq!(
            editor.execute(1, Command::AddGeometricConstraint { record: missing },),
            Err(CommandError::EdgeNotFound(missing_edge))
        );
        assert_eq!(editor_state_snapshot(&editor), before_missing);
    }

    #[test]
    fn referenced_geometry_is_locked_until_constraint_is_explicitly_removed() {
        let (mut editor, pattern, _) = rectangular_editor();
        let constrained_edge = pattern.edges[4].clone();
        let unrelated_vertex = pattern.vertices[2].id;
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: constrained_edge.id,
            },
        };
        editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            )
            .expect("add horizontal constraint");

        for command in [
            Command::MoveVertex {
                id: constrained_edge.start,
                position: Point2::new(61.0, 45.0),
            },
            Command::RemoveEdge {
                id: constrained_edge.id,
            },
            Command::ResizeRectangularPaper {
                width_mm: 120.0,
                height_mm: 60.0,
            },
        ] {
            let before = editor_state_snapshot(&editor);
            assert_eq!(
                editor.execute(editor.revision(), command),
                Err(CommandError::GeometricConstraintBlocksGeometryMutation {
                    constraint: record.id,
                })
            );
            assert_eq!(editor_state_snapshot(&editor), before);
        }

        editor
            .execute(
                1,
                Command::MoveVertex {
                    id: unrelated_vertex,
                    position: Point2::new(-30.0, 90.0),
                },
            )
            .expect("unrelated vertex remains editable");
        editor
            .execute(2, Command::RemoveGeometricConstraint { id: record.id })
            .expect("remove the explicit lock");
        editor
            .execute(
                3,
                Command::MoveVertex {
                    id: constrained_edge.start,
                    position: Point2::new(61.0, 45.0),
                },
            )
            .expect("referenced geometry becomes editable after constraint removal");
    }

    #[test]
    fn constraint_only_edits_preserve_the_current_runtime_pose() {
        let (mut editor, pattern, _) = rectangular_editor();
        let pose = runtime_pose(25.0);
        editor.adopt_current_applied_pose(pose.clone());
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[4].id,
            },
        };

        editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            )
            .expect("constraint metadata does not replace pose");
        assert_eq!(editor.current_applied_pose(), Some(&pose));
        editor.undo(1).expect("undo constraint metadata");
        assert_eq!(editor.current_applied_pose(), Some(&pose));
        editor.redo(2).expect("redo constraint metadata");
        assert_eq!(editor.current_applied_pose(), Some(&pose));
    }

    #[test]
    fn every_constraint_kind_rejects_a_missing_reference_without_any_state_change() {
        #[derive(Clone, Copy)]
        enum ExpectedMissing {
            Vertex(VertexId),
            Edge(EdgeId),
        }

        let (base, pattern, _) = rectangular_editor();
        let missing_vertex = VertexId::new();
        let missing_edge = EdgeId::new();
        let vertex_a = pattern.vertices[0].id;
        let vertex_b = pattern.vertices[1].id;
        let edge_a = pattern.edges[0].id;
        let edge_b = pattern.edges[1].id;
        let edge_c = pattern.edges[2].id;
        let cases = vec![
            (
                "fixed_length",
                GeometricConstraintKindV1::FixedLength {
                    edge: missing_edge,
                    length_mm: 1.0,
                },
                ExpectedMissing::Edge(missing_edge),
            ),
            (
                "fixed_angle",
                GeometricConstraintKindV1::FixedAngle {
                    vertex: missing_vertex,
                    first_edge: edge_a,
                    second_edge: edge_b,
                    angle_degrees: 90.0,
                },
                ExpectedMissing::Vertex(missing_vertex),
            ),
            (
                "horizontal",
                GeometricConstraintKindV1::Horizontal { edge: missing_edge },
                ExpectedMissing::Edge(missing_edge),
            ),
            (
                "vertical",
                GeometricConstraintKindV1::Vertical { edge: missing_edge },
                ExpectedMissing::Edge(missing_edge),
            ),
            (
                "equal_length",
                GeometricConstraintKindV1::EqualLength {
                    first_edge: missing_edge,
                    second_edge: edge_a,
                },
                ExpectedMissing::Edge(missing_edge),
            ),
            (
                "parallel",
                GeometricConstraintKindV1::Parallel {
                    first_edge: edge_a,
                    second_edge: missing_edge,
                },
                ExpectedMissing::Edge(missing_edge),
            ),
            (
                "point_on_line",
                GeometricConstraintKindV1::PointOnLine {
                    vertex: missing_vertex,
                    line_edge: edge_a,
                },
                ExpectedMissing::Vertex(missing_vertex),
            ),
            (
                "mirror_symmetry",
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: missing_vertex,
                    second_vertex: vertex_a,
                    axis_edge: edge_a,
                },
                ExpectedMissing::Vertex(missing_vertex),
            ),
            (
                "rotational_symmetry",
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: missing_vertex,
                    source_vertex: vertex_a,
                    target_vertex: vertex_b,
                    angle_degrees: 120.0,
                },
                ExpectedMissing::Vertex(missing_vertex),
            ),
            (
                "angle_bisector",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: missing_vertex,
                    first_edge: edge_a,
                    second_edge: edge_b,
                    bisector_edge: edge_c,
                },
                ExpectedMissing::Vertex(missing_vertex),
            ),
            (
                "length_ratio",
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: edge_a,
                    denominator_edge: missing_edge,
                    ratio: 2.0,
                },
                ExpectedMissing::Edge(missing_edge),
            ),
        ];

        assert_eq!(cases.len(), 11);
        for (name, constraint, expected) in cases {
            let mut editor = base.clone();
            let before = editor_state_snapshot(&editor);
            let error = editor
                .execute(
                    0,
                    Command::AddGeometricConstraint {
                        record: GeometricConstraintRecordV1 {
                            id: ConstraintId::new(),
                            constraint,
                        },
                    },
                )
                .expect_err(name);
            match expected {
                ExpectedMissing::Vertex(vertex) => {
                    assert_eq!(error, CommandError::VertexNotFound(vertex), "{name}");
                }
                ExpectedMissing::Edge(edge) => {
                    assert_eq!(error, CommandError::EdgeNotFound(edge), "{name}");
                }
            }
            assert_eq!(editor_state_snapshot(&editor), before, "{name}");
        }
    }

    #[test]
    fn adding_a_constraint_checks_domain_before_references_and_normalizes_operand_errors() {
        let (base, _, _) = rectangular_editor();
        let missing_edge = EdgeId::new();
        let invalid_id = ConstraintId::new();
        let mut domain_editor = base.clone();
        let before = editor_state_snapshot(&domain_editor);
        assert_eq!(
            domain_editor.execute(
                0,
                Command::AddGeometricConstraint {
                    record: GeometricConstraintRecordV1 {
                        id: invalid_id,
                        constraint: GeometricConstraintKindV1::FixedLength {
                            edge: missing_edge,
                            length_mm: 0.0,
                        },
                    },
                },
            ),
            Err(CommandError::GeometricConstraintDocumentInvalid(
                GeometricConstraintDocumentValidationErrorV1::NonPositiveFixedLength {
                    constraint: invalid_id,
                },
            ))
        );
        assert_eq!(editor_state_snapshot(&domain_editor), before);

        let first_missing = EdgeId::new();
        let second_missing = EdgeId::new();
        let expected_missing = if first_missing.canonical_bytes() < second_missing.canonical_bytes()
        {
            first_missing
        } else {
            second_missing
        };
        let constraint_id = ConstraintId::new();
        let mut errors = Vec::new();
        for (first_edge, second_edge) in [
            (first_missing, second_missing),
            (second_missing, first_missing),
        ] {
            let mut editor = base.clone();
            let before = editor_state_snapshot(&editor);
            errors.push(
                editor
                    .execute(
                        0,
                        Command::AddGeometricConstraint {
                            record: GeometricConstraintRecordV1 {
                                id: constraint_id,
                                constraint: GeometricConstraintKindV1::EqualLength {
                                    first_edge,
                                    second_edge,
                                },
                            },
                        },
                    )
                    .expect_err("both edge references are missing"),
            );
            assert_eq!(editor_state_snapshot(&editor), before);
        }
        assert_eq!(errors[0], errors[1]);
        assert_eq!(errors[0], CommandError::EdgeNotFound(expected_missing));
    }

    #[test]
    fn new_constraint_geometry_validation_ignores_unrelated_damage_but_rejects_relevant_damage() {
        let (_, mut pattern, paper) = rectangular_editor();
        let target_edge = pattern.edges[4].id;
        pattern.vertices[2].position.x = f64::NAN;
        let mut editor = EditorState::with_paper(pattern, paper);
        let accepted = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge: target_edge },
        };
        editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: accepted.clone(),
                },
            )
            .expect("unrelated malformed vertex must not block admission");
        assert_eq!(editor.geometric_constraints().constraints, vec![accepted]);

        let (_, mut pattern, paper) = rectangular_editor();
        let target = pattern.edges[4].clone();
        let start = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == target.start)
            .expect("target start")
            .position;
        pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == target.end)
            .expect("target end")
            .position = start;
        let mut editor = EditorState::with_paper(pattern, paper);
        let before = editor_state_snapshot(&editor);
        let id = ConstraintId::new();
        assert_eq!(
            editor.execute(
                0,
                Command::AddGeometricConstraint {
                    record: GeometricConstraintRecordV1 {
                        id,
                        constraint: GeometricConstraintKindV1::FixedLength {
                            edge: target.id,
                            length_mm: 10.0,
                        },
                    },
                },
            ),
            Err(CommandError::GeometricConstraintGeometryInvalid(
                GeometricConstraintErrorV1::DegenerateGeometryEdge { edge: target.id },
            ))
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn loaded_invalid_constraints_can_be_repaired_and_undo_restores_exact_raw_records() {
        let (_, pattern, paper) = rectangular_editor();
        let first = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::FixedLength {
                edge: pattern.edges[0].id,
                length_mm: 0.0,
            },
        };
        let second = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::LengthRatio {
                numerator_edge: pattern.edges[1].id,
                denominator_edge: pattern.edges[2].id,
                ratio: -1.0,
            },
        };
        let raw = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![first.clone(), second.clone()],
        };
        let mut editor = EditorState::with_document_parts_and_constraints(
            pattern,
            paper,
            InstructionTimeline::default(),
            raw.clone(),
        );

        let remove_first = editor
            .execute(0, Command::RemoveGeometricConstraint { id: first.id })
            .expect("remove one invalid raw record");
        assert!(remove_first.constraints_changed);
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![second.clone()]
        );
        let remove_second = editor
            .execute(1, Command::RemoveGeometricConstraint { id: second.id })
            .expect("progressively remove the remaining invalid record");
        assert!(remove_second.constraints_changed);
        assert!(editor.geometric_constraints().constraints.is_empty());

        let restore_second = editor.undo(2).expect("restore exact second raw record");
        assert!(restore_second.constraints_changed);
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![second.clone()]
        );
        let restore_first = editor.undo(3).expect("restore exact original document");
        assert!(restore_first.constraints_changed);
        assert_eq!(editor.geometric_constraints(), &raw);

        assert!(
            editor
                .redo(4)
                .expect("redo first removal")
                .constraints_changed
        );
        assert_eq!(
            editor.geometric_constraints().constraints,
            vec![second.clone()]
        );
        assert!(
            editor
                .redo(5)
                .expect("redo second removal")
                .constraints_changed
        );
        assert!(editor.geometric_constraints().constraints.is_empty());

        let duplicate_id = ConstraintId::new();
        let duplicate_raw = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![
                GeometricConstraintRecordV1 {
                    id: duplicate_id,
                    constraint: GeometricConstraintKindV1::Horizontal {
                        edge: editor.pattern.edges[0].id,
                    },
                },
                GeometricConstraintRecordV1 {
                    id: duplicate_id,
                    constraint: GeometricConstraintKindV1::Vertical {
                        edge: editor.pattern.edges[1].id,
                    },
                },
            ],
        };
        let mut duplicate_editor = EditorState::with_document_parts_and_constraints(
            editor.pattern.clone(),
            editor.paper.clone(),
            InstructionTimeline::default(),
            duplicate_raw.clone(),
        );
        duplicate_editor
            .execute(0, Command::RemoveGeometricConstraint { id: duplicate_id })
            .expect("remove one duplicate raw record");
        assert_eq!(
            duplicate_editor.geometric_constraints().constraints,
            vec![duplicate_raw.constraints[1].clone()]
        );
        duplicate_editor
            .undo(1)
            .expect("trusted undo restores a duplicate raw ID exactly");
        assert_eq!(duplicate_editor.geometric_constraints(), &duplicate_raw);
    }

    #[test]
    fn add_is_strict_even_when_loaded_state_was_not_validated() {
        let (_, pattern, paper) = rectangular_editor();
        let invalid_id = ConstraintId::new();
        let invalid_document = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: invalid_id,
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: pattern.edges[0].id,
                    length_mm: 0.0,
                },
            }],
        };
        let mut editor = EditorState::with_document_parts_and_constraints(
            pattern.clone(),
            paper,
            InstructionTimeline::default(),
            invalid_document,
        );
        let before = editor_state_snapshot(&editor);
        assert_eq!(
            editor.execute(
                0,
                Command::AddGeometricConstraint {
                    record: GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::Horizontal {
                            edge: pattern.edges[1].id,
                        },
                    },
                },
            ),
            Err(CommandError::GeometricConstraintDocumentInvalid(
                GeometricConstraintDocumentValidationErrorV1::NonPositiveFixedLength {
                    constraint: invalid_id,
                },
            ))
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }

    #[test]
    fn constraint_change_reporting_is_true_only_for_constraint_history_transitions() {
        let (mut editor, pattern, _) = rectangular_editor();
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[4].id,
            },
        };
        let add = editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            )
            .expect("add");
        assert!(add.constraints_changed);
        assert!(!add.settings_changed);
        assert!(!add.instructions_changed);
        assert!(editor.undo(1).expect("undo add").constraints_changed);
        assert!(editor.redo(2).expect("redo add").constraints_changed);
        assert!(
            editor
                .execute(3, Command::RemoveGeometricConstraint { id: record.id })
                .expect("remove")
                .constraints_changed
        );
        assert!(editor.undo(4).expect("undo remove").constraints_changed);
        assert!(editor.redo(5).expect("redo remove").constraints_changed);
    }

    #[test]
    fn bit_exact_geometry_noops_bypass_constraint_locks_but_bit_changes_do_not() {
        let (mut editor, pattern, _) = rectangular_editor();
        let constrained_edge = pattern.edges[4].clone();
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: constrained_edge.id,
            },
        };
        editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            )
            .expect("constraint");
        let position = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == constrained_edge.start)
            .expect("start")
            .position;
        let exact_move = editor
            .execute(
                1,
                Command::MoveVertex {
                    id: constrained_edge.start,
                    position,
                },
            )
            .expect("bit-exact move is history-bearing but geometry-preserving");
        assert!(!exact_move.constraints_changed);
        assert_eq!(editor.pattern(), &pattern);
        editor.undo(2).expect("undo exact move");
        editor.redo(3).expect("redo exact move");

        let same_size = editor
            .execute(
                4,
                Command::ResizeRectangularPaper {
                    width_mm: 100.0,
                    height_mm: 50.0,
                },
            )
            .expect("bit-exact resize bypasses constraint lock");
        assert!(!same_size.constraints_changed);
        assert_eq!(editor.pattern(), &pattern);

        let sheet =
            crate::create_rectangular_sheet(100.0, 50.0, false).expect("signed-zero fixture");
        let (mut signed_pattern, paper) = sheet.into_parts();
        let left_vertices = signed_pattern
            .vertices
            .iter()
            .enumerate()
            .filter_map(|(index, vertex)| (vertex.position.x == 0.0).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(left_vertices.len(), 2);
        signed_pattern.vertices[left_vertices[0]].position.x = -0.0;
        signed_pattern.vertices[left_vertices[1]].position.x = 0.0;
        let signed_edge = signed_pattern.edges[0].id;
        let signed_constraint = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge: signed_edge },
        };
        let mut signed_editor = EditorState::with_paper(signed_pattern.clone(), paper);
        signed_editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: signed_constraint.clone(),
                },
            )
            .expect("signed-zero fixture constraint");

        let signed_before = editor_state_snapshot(&signed_editor);
        assert_eq!(
            signed_editor.execute(
                1,
                Command::ResizeRectangularPaper {
                    width_mm: 100.0,
                    height_mm: 50.0,
                },
            ),
            Err(CommandError::GeometricConstraintBlocksGeometryMutation {
                constraint: signed_constraint.id,
            })
        );
        assert_eq!(editor_state_snapshot(&signed_editor), signed_before);

        let moved_vertex = signed_pattern.edges[0].start;
        let original = signed_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == moved_vertex)
            .expect("signed edge start")
            .position;
        let changed_zero = Point2::new(
            if original.x.to_bits() == 0.0_f64.to_bits() {
                -0.0
            } else {
                0.0
            },
            original.y,
        );
        let move_before = editor_state_snapshot(&signed_editor);
        assert_eq!(
            signed_editor.execute(
                1,
                Command::MoveVertex {
                    id: moved_vertex,
                    position: changed_zero,
                },
            ),
            Err(CommandError::GeometricConstraintBlocksGeometryMutation {
                constraint: signed_constraint.id,
            })
        );
        assert_eq!(editor_state_snapshot(&signed_editor), move_before);
    }

    #[test]
    fn geometric_constraint_growth_table_accepts_exact_limits_and_rejects_one_over_atomically() {
        type CommandFactory = fn(&ConstraintResourceFixture) -> Command;
        let cases: [(
            &str,
            usize,
            usize,
            GeometricConstraintResourceV1,
            CommandFactory,
        ); 8] = [
            (
                "add_vertex",
                1,
                0,
                GeometricConstraintResourceV1::Vertices,
                |_| Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(40.0, 40.0),
                },
            ),
            (
                "add_edge",
                0,
                1,
                GeometricConstraintResourceV1::Edges,
                |fixture| Command::AddEdge {
                    id: EdgeId::new(),
                    start: fixture.vertices[0],
                    end: fixture.vertices[3],
                    kind: EdgeKind::Mountain,
                },
            ),
            (
                "split_edge",
                1,
                1,
                GeometricConstraintResourceV1::Vertices,
                |fixture| Command::SplitEdge {
                    edge: fixture.edges[0],
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            ),
            (
                "connect_edge_intersection",
                1,
                2,
                GeometricConstraintResourceV1::Vertices,
                |fixture| Command::ConnectEdgeIntersection {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    new_vertex: VertexId::new(),
                    first_new_edge: EdgeId::new(),
                    second_new_edge: EdgeId::new(),
                },
            ),
            (
                "connect_t_junction",
                0,
                1,
                GeometricConstraintResourceV1::Edges,
                |fixture| Command::ConnectTJunction {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    new_edge: EdgeId::new(),
                },
            ),
            (
                "connect_intersection_cluster_create",
                1,
                3,
                GeometricConstraintResourceV1::Vertices,
                |fixture| Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create {
                        id: VertexId::new(),
                    },
                    targets: fixture
                        .edges
                        .iter()
                        .copied()
                        .map(|edge| IntersectionEdgeTarget {
                            edge,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            ),
            (
                "connect_intersection_cluster_reuse",
                0,
                3,
                GeometricConstraintResourceV1::Edges,
                |fixture| Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Reuse {
                        id: fixture.vertices[0],
                    },
                    targets: fixture
                        .edges
                        .iter()
                        .copied()
                        .map(|edge| IntersectionEdgeTarget {
                            edge,
                            new_edge: Some(EdgeId::new()),
                        })
                        .collect(),
                },
            ),
            (
                "split_boundary_edge",
                1,
                1,
                GeometricConstraintResourceV1::Vertices,
                |fixture| Command::SplitBoundaryEdge {
                    edge: fixture.edges[0],
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            ),
        ];

        for (name, added_vertices, added_edges, failing_resource, command_factory) in cases {
            let mut fixture = constraint_resource_fixture(
                DEFAULT_MAX_CONSTRAINT_VERTICES - added_vertices,
                DEFAULT_MAX_CONSTRAINT_EDGES - added_edges,
            );
            let command = command_factory(&fixture);
            assert_eq!(
                command.geometric_resource_growth(),
                Ok(Some((added_vertices, added_edges))),
                "{name} growth mapping"
            );
            assert_eq!(
                fixture
                    .editor
                    .ensure_geometric_constraint_resource_admission(&command),
                Ok(()),
                "{name} must admit exact equality"
            );

            match failing_resource {
                GeometricConstraintResourceV1::Vertices => fixture
                    .editor
                    .pattern
                    .vertices
                    .push(fixture.filler_vertex.clone()),
                GeometricConstraintResourceV1::Edges => fixture
                    .editor
                    .pattern
                    .edges
                    .push(fixture.filler_edge.clone()),
                GeometricConstraintResourceV1::Constraints
                | GeometricConstraintResourceV1::References => {
                    unreachable!("geometry growth only uses vertex and edge resources")
                }
            }
            fixture
                .editor
                .adopt_current_applied_pose(runtime_pose(73.0));
            let before = editor_state_snapshot(&fixture.editor);
            let maximum = match failing_resource {
                GeometricConstraintResourceV1::Vertices => DEFAULT_MAX_CONSTRAINT_VERTICES,
                GeometricConstraintResourceV1::Edges => DEFAULT_MAX_CONSTRAINT_EDGES,
                GeometricConstraintResourceV1::Constraints
                | GeometricConstraintResourceV1::References => unreachable!(),
            };
            assert_eq!(
                fixture.editor.execute(0, command),
                Err(CommandError::GeometricConstraintGeometryLimitExceeded {
                    resource: failing_resource,
                    actual: maximum + 1,
                    maximum,
                }),
                "{name} must reject the first result beyond the hard ceiling"
            );
            assert_eq!(
                editor_state_snapshot(&fixture.editor),
                before,
                "{name} rejection must preserve every editor authority"
            );
        }
    }

    #[test]
    fn simple_constraint_growth_commands_can_reach_the_exact_hard_ceiling() {
        let mut vertex_fixture =
            constraint_resource_fixture(DEFAULT_MAX_CONSTRAINT_VERTICES - 1, 3);
        vertex_fixture
            .editor
            .execute(
                0,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(40.0, 40.0),
                },
            )
            .expect("the exact vertex ceiling is inclusive");
        assert_eq!(
            vertex_fixture.editor.pattern.vertices.len(),
            DEFAULT_MAX_CONSTRAINT_VERTICES
        );

        let mut edge_fixture = constraint_resource_fixture(5, DEFAULT_MAX_CONSTRAINT_EDGES - 1);
        edge_fixture
            .editor
            .execute(
                0,
                Command::AddEdge {
                    id: EdgeId::new(),
                    start: edge_fixture.vertices[0],
                    end: edge_fixture.vertices[3],
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("the exact edge ceiling is inclusive");
        assert_eq!(
            edge_fixture.editor.pattern.edges.len(),
            DEFAULT_MAX_CONSTRAINT_EDGES
        );
    }

    #[test]
    fn first_constraint_checks_shared_pattern_limits_before_mutation() {
        let mut exact = constraint_resource_fixture(
            DEFAULT_MAX_CONSTRAINT_VERTICES,
            DEFAULT_MAX_CONSTRAINT_EDGES,
        );
        exact.editor.geometric_constraints = GeometricConstraintDocumentV1::default();
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: exact.edges[0],
            },
        };
        exact
            .editor
            .execute(
                0,
                Command::AddGeometricConstraint {
                    record: record.clone(),
                },
            )
            .expect("the first constraint accepts exact vertex and edge ceilings");
        assert_eq!(exact.editor.geometric_constraints.constraints, vec![record]);

        for (vertex_count, edge_count, resource, maximum) in [
            (
                DEFAULT_MAX_CONSTRAINT_VERTICES + 1,
                3,
                GeometricConstraintResourceV1::Vertices,
                DEFAULT_MAX_CONSTRAINT_VERTICES,
            ),
            (
                5,
                DEFAULT_MAX_CONSTRAINT_EDGES + 1,
                GeometricConstraintResourceV1::Edges,
                DEFAULT_MAX_CONSTRAINT_EDGES,
            ),
        ] {
            let mut oversized = constraint_resource_fixture(vertex_count, edge_count);
            oversized.editor.geometric_constraints = GeometricConstraintDocumentV1::default();
            oversized
                .editor
                .adopt_current_applied_pose(runtime_pose(74.0));
            let record = GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal {
                    edge: oversized.edges[0],
                },
            };
            let before = editor_state_snapshot(&oversized.editor);
            assert_eq!(
                oversized
                    .editor
                    .execute(0, Command::AddGeometricConstraint { record }),
                Err(CommandError::GeometricConstraintGeometryLimitExceeded {
                    resource,
                    actual: maximum + 1,
                    maximum,
                })
            );
            assert_eq!(editor_state_snapshot(&oversized.editor), before);
        }
    }

    #[test]
    fn empty_constraint_documents_keep_oversized_geometry_repairable_and_editable() {
        let mut vertex_fixture =
            constraint_resource_fixture(DEFAULT_MAX_CONSTRAINT_VERTICES + 1, 3);
        vertex_fixture.editor.geometric_constraints = GeometricConstraintDocumentV1::default();
        vertex_fixture
            .editor
            .execute(
                0,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(50.0, 50.0),
                },
            )
            .expect("constraints-empty oversized vertices remain editable");
        assert_eq!(
            vertex_fixture.editor.pattern.vertices.len(),
            DEFAULT_MAX_CONSTRAINT_VERTICES + 2
        );

        let mut edge_fixture = constraint_resource_fixture(5, DEFAULT_MAX_CONSTRAINT_EDGES + 1);
        edge_fixture.editor.geometric_constraints = GeometricConstraintDocumentV1::default();
        edge_fixture
            .editor
            .execute(
                0,
                Command::AddEdge {
                    id: EdgeId::new(),
                    start: edge_fixture.vertices[0],
                    end: edge_fixture.vertices[3],
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("constraints-empty oversized edges remain editable");
        assert_eq!(
            edge_fixture.editor.pattern.edges.len(),
            DEFAULT_MAX_CONSTRAINT_EDGES + 2
        );
    }

    #[test]
    fn oversized_constraint_repairs_and_trusted_undo_restore_exact_raw_state() {
        let mut fixture = constraint_resource_fixture(DEFAULT_MAX_CONSTRAINT_VERTICES + 1, 3);
        let original_pattern = fixture.editor.pattern.clone();
        let original_constraints = fixture.editor.geometric_constraints.clone();

        fixture
            .editor
            .execute(
                0,
                Command::RemoveVertex {
                    id: fixture.repair_vertex,
                },
            )
            .expect("a decreasing repair remains available above the ceiling");
        assert_eq!(
            fixture.editor.pattern.vertices.len(),
            DEFAULT_MAX_CONSTRAINT_VERTICES
        );
        fixture
            .editor
            .undo(1)
            .expect("trusted undo restores the exact oversized loaded geometry");
        assert_eq!(fixture.editor.pattern, original_pattern);
        assert_eq!(fixture.editor.geometric_constraints, original_constraints);

        let constraint_id = fixture.editor.geometric_constraints.constraints[0].id;
        fixture
            .editor
            .execute(2, Command::RemoveGeometricConstraint { id: constraint_id })
            .expect("constraint deletion remains a repair operation");
        assert!(fixture.editor.geometric_constraints.is_empty());
        fixture
            .editor
            .undo(3)
            .expect("trusted undo restores the exact raw constraint record");
        assert_eq!(fixture.editor.pattern, original_pattern);
        assert_eq!(fixture.editor.geometric_constraints, original_constraints);
    }

    #[test]
    fn execute_limit_failure_preserves_populated_undo_redo_and_pose_authority() {
        let mut fixture = constraint_resource_fixture(DEFAULT_MAX_CONSTRAINT_VERTICES - 2, 3);
        for revision in 0..2 {
            fixture
                .editor
                .execute(
                    revision,
                    Command::AddVertex {
                        id: VertexId::new(),
                        position: Point2::new(60.0 + revision as f64, 60.0),
                    },
                )
                .expect("prepare two undo entries");
        }
        fixture
            .editor
            .undo(2)
            .expect("prepare simultaneous nonempty undo and redo stacks");
        assert!(fixture.editor.can_undo());
        assert!(fixture.editor.can_redo());
        fixture
            .editor
            .pattern
            .vertices
            .push(fixture.filler_vertex.clone());
        fixture
            .editor
            .adopt_current_applied_pose(runtime_pose(74.5));
        let before = editor_state_snapshot(&fixture.editor);

        assert_eq!(
            fixture.editor.execute(
                3,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(63.0, 60.0),
                },
            ),
            Err(CommandError::GeometricConstraintGeometryLimitExceeded {
                resource: GeometricConstraintResourceV1::Vertices,
                actual: DEFAULT_MAX_CONSTRAINT_VERTICES + 1,
                maximum: DEFAULT_MAX_CONSTRAINT_VERTICES,
            })
        );
        assert_eq!(editor_state_snapshot(&fixture.editor), before);
    }

    #[test]
    fn redo_limit_failure_preserves_pattern_paper_constraints_history_revision_and_pose() {
        let mut fixture = constraint_resource_fixture(DEFAULT_MAX_CONSTRAINT_VERTICES - 1, 3);
        let added_vertex = VertexId::new();
        fixture
            .editor
            .execute(
                0,
                Command::AddVertex {
                    id: added_vertex,
                    position: Point2::new(60.0, 60.0),
                },
            )
            .expect("prepare one redo entry at the exact ceiling");
        fixture
            .editor
            .undo(1)
            .expect("return to the pre-add state while retaining redo");
        fixture
            .editor
            .pattern
            .vertices
            .push(fixture.filler_vertex.clone());
        fixture
            .editor
            .adopt_current_applied_pose(runtime_pose(75.0));
        let before = editor_state_snapshot(&fixture.editor);

        assert_eq!(
            fixture.editor.redo(2),
            Err(CommandError::GeometricConstraintGeometryLimitExceeded {
                resource: GeometricConstraintResourceV1::Vertices,
                actual: DEFAULT_MAX_CONSTRAINT_VERTICES + 1,
                maximum: DEFAULT_MAX_CONSTRAINT_VERTICES,
            })
        );
        assert_eq!(editor_state_snapshot(&fixture.editor), before);
        assert!(
            fixture
                .editor
                .pattern
                .vertices
                .iter()
                .all(|vertex| vertex.id != added_vertex)
        );
    }

    #[test]
    fn projected_constraint_resource_counts_use_checked_typed_errors() {
        assert_eq!(
            ensure_geometric_constraint_result_count(
                GeometricConstraintResourceV1::Vertices,
                DEFAULT_MAX_CONSTRAINT_VERTICES,
                0,
                DEFAULT_MAX_CONSTRAINT_VERTICES,
            ),
            Ok(())
        );
        assert_eq!(
            ensure_geometric_constraint_result_count(
                GeometricConstraintResourceV1::Edges,
                DEFAULT_MAX_CONSTRAINT_EDGES,
                1,
                DEFAULT_MAX_CONSTRAINT_EDGES,
            ),
            Err(CommandError::GeometricConstraintGeometryLimitExceeded {
                resource: GeometricConstraintResourceV1::Edges,
                actual: DEFAULT_MAX_CONSTRAINT_EDGES + 1,
                maximum: DEFAULT_MAX_CONSTRAINT_EDGES,
            })
        );
        assert_eq!(
            ensure_geometric_constraint_result_count(
                GeometricConstraintResourceV1::Vertices,
                usize::MAX,
                1,
                usize::MAX,
            ),
            Err(CommandError::GeometricConstraintGeometryCountOverflow {
                resource: GeometricConstraintResourceV1::Vertices,
            })
        );
    }

    #[test]
    fn constraint_lock_visit_count_is_additive_for_maximum_v1_sizes() {
        let pattern = crate::benchmark_pattern(DEFAULT_MAX_CONSTRAINT_EDGES);
        let moved_vertex = pattern.edges[0].start;
        let target_edge = pattern.edges[0].id;
        let constraints = (0..ori_domain::DEFAULT_MAX_CONSTRAINT_RECORDS)
            .map(|_| GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge: target_edge },
            })
            .collect::<Vec<_>>();
        let expected_blocker = constraints
            .iter()
            .min_by_key(|record| record.id.canonical_bytes())
            .expect("nonempty fixture")
            .id;
        let editor = EditorState::with_document_parts_and_constraints(
            pattern,
            Paper::default(),
            InstructionTimeline::default(),
            GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints,
            },
        );

        begin_constraint_lock_visit_count();
        let result = editor.ensure_geometric_constraints_allow(&Command::MoveVertex {
            id: moved_vertex,
            position: Point2::new(0.25, 0.25),
        });
        let (edge_visits, constraint_visits) = finish_constraint_lock_visit_count();

        assert_eq!(
            result,
            Err(CommandError::GeometricConstraintBlocksGeometryMutation {
                constraint: expected_blocker,
            })
        );
        assert_eq!(edge_visits, DEFAULT_MAX_CONSTRAINT_EDGES);
        assert_eq!(
            constraint_visits,
            ori_domain::DEFAULT_MAX_CONSTRAINT_RECORDS
        );
        assert_eq!(
            edge_visits + constraint_visits,
            DEFAULT_MAX_CONSTRAINT_EDGES + ori_domain::DEFAULT_MAX_CONSTRAINT_RECORDS
        );
    }

    fn test_layer(name: &str) -> LayerRecordV1 {
        LayerRecordV1 {
            id: LayerId::new(),
            name: name.to_owned(),
            content_kind: ori_domain::LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        }
    }

    fn editor_with_test_layers(
        pattern: CreasePattern,
        paper: Paper,
        layers: Vec<LayerRecordV1>,
        assignments: Vec<(EdgeId, LayerId)>,
    ) -> EditorState {
        let mut project_layers = ProjectLayerDocumentV1::default();
        project_layers.layers.extend(layers);
        project_layers.edge_assignments = assignments
            .into_iter()
            .map(|(edge, layer)| EdgeLayerAssignmentV1 { edge, layer })
            .collect();
        project_layers
            .edge_assignments
            .sort_unstable_by_key(|assignment| assignment.edge.canonical_bytes());
        validate_project_layer_document_against_pattern_v1(&project_layers, &pattern)
            .expect("valid test layer document");
        EditorState::with_document_parts_constraints_and_layers(
            pattern,
            paper,
            InstructionTimeline::default(),
            GeometricConstraintDocumentV1::default(),
            project_layers,
        )
    }

    #[test]
    fn layer_presentation_is_atomic_undoable_and_can_unlock_itself() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let original = editor.project_layers().clone();

        let result = editor
            .execute(
                0,
                Command::UpdateLayerPresentation {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                    visible: false,
                    locked: true,
                    opacity: 0.25,
                },
            )
            .expect("lock and hide default layer");
        assert!(result.settings_changed);
        assert_eq!(
            editor.project_layers().layers[0],
            LayerRecordV1 {
                visible: false,
                locked: true,
                opacity: 0.25,
                ..original.layers[0].clone()
            }
        );

        editor.undo(1).expect("undo presentation");
        assert_eq!(editor.project_layers(), &original);
        editor.redo(2).expect("redo presentation");
        assert!(editor.project_layers().layers[0].locked);

        editor
            .execute(
                3,
                Command::UpdateLayerPresentation {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                    visible: true,
                    locked: false,
                    opacity: 1.0,
                },
            )
            .expect("a locked layer must be able to unlock itself");
        assert_eq!(editor.project_layers(), &original);
    }

    #[test]
    fn invalid_layer_opacity_is_rejected_without_partial_state_or_history() {
        for opacity in [f64::NAN, f64::INFINITY, -0.0, -0.1, 1.1] {
            let mut editor = EditorState::new(CreasePattern::empty());
            let before = editor_state_snapshot(&editor);
            assert!(matches!(
                editor.execute(
                    0,
                    Command::UpdateLayerPresentation {
                        layer: DEFAULT_PROJECT_LAYER_ID,
                        visible: false,
                        locked: true,
                        opacity,
                    },
                ),
                Err(CommandError::ProjectLayerDocumentInvalid(_))
            ));
            assert_eq!(editor_state_snapshot(&editor), before);
        }
    }

    #[test]
    fn locked_layer_routes_every_edge_and_shared_vertex_mutation_to_one_guard() {
        let shared = VertexId::new();
        let locked_end = VertexId::new();
        let unlocked_end = VertexId::new();
        let locked_edge = EdgeId::new();
        let unlocked_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: shared,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: locked_end,
                    position: Point2::new(10.0, 0.0),
                },
                Vertex {
                    id: unlocked_end,
                    position: Point2::new(0.0, 10.0),
                },
            ],
            edges: vec![
                Edge {
                    id: locked_edge,
                    start: shared,
                    end: locked_end,
                    kind: EdgeKind::Mountain,
                },
                Edge {
                    id: unlocked_edge,
                    start: shared,
                    end: unlocked_end,
                    kind: EdgeKind::Valley,
                },
            ],
        };
        let mut locked_layer = test_layer("Locked fold");
        locked_layer.locked = true;
        let locked_layer_id = locked_layer.id;
        let mut editor = editor_with_test_layers(
            pattern,
            Paper::default(),
            vec![locked_layer],
            vec![(locked_edge, locked_layer_id)],
        );
        let before = editor_state_snapshot(&editor);

        let commands = vec![
            Command::MoveVertex {
                id: shared,
                position: Point2::new(1.0, 1.0),
            },
            Command::RemoveVertex { id: shared },
            Command::RemoveEdge { id: locked_edge },
            Command::SplitEdge {
                edge: locked_edge,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            Command::SplitBoundaryEdge {
                edge: locked_edge,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            Command::ConnectEdgeIntersection {
                first_edge: unlocked_edge,
                second_edge: locked_edge,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            Command::ConnectTJunction {
                first_edge: unlocked_edge,
                second_edge: locked_edge,
                new_edge: EdgeId::new(),
            },
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create {
                    id: VertexId::new(),
                },
                targets: vec![
                    IntersectionEdgeTarget {
                        edge: unlocked_edge,
                        new_edge: Some(EdgeId::new()),
                    },
                    IntersectionEdgeTarget {
                        edge: locked_edge,
                        new_edge: Some(EdgeId::new()),
                    },
                ],
            },
            Command::RemoveBoundaryVertex { vertex: shared },
            Command::ResizeRectangularPaper {
                width_mm: 200.0,
                height_mm: 200.0,
            },
            Command::AssignEdgeToLayer {
                edge: locked_edge,
                layer: DEFAULT_PROJECT_LAYER_ID,
            },
            Command::AssignEdgeToLayer {
                edge: unlocked_edge,
                layer: locked_layer_id,
            },
            Command::DeleteLayer {
                layer: locked_layer_id,
            },
        ];

        for command in commands {
            assert_eq!(
                editor.execute(0, command),
                Err(CommandError::LayerLocked(locked_layer_id))
            );
            assert_eq!(
                editor_state_snapshot(&editor),
                before,
                "a rejected locked-layer edit must be fully atomic",
            );
        }
    }

    #[test]
    fn locked_default_layer_blocks_new_geometry_but_not_unlocking() {
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::UpdateLayerPresentation {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                    visible: true,
                    locked: true,
                    opacity: 1.0,
                },
            )
            .expect("lock default layer");
        let before = editor_state_snapshot(&editor);

        for command in [
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 1.0),
            },
            Command::AddEdge {
                id: EdgeId::new(),
                start: VertexId::new(),
                end: VertexId::new(),
                kind: EdgeKind::Mountain,
            },
        ] {
            assert_eq!(
                editor.execute(1, command),
                Err(CommandError::LayerLocked(DEFAULT_PROJECT_LAYER_ID))
            );
            assert_eq!(editor_state_snapshot(&editor), before);
        }

        editor
            .execute(
                1,
                Command::UpdateLayerPresentation {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                    visible: true,
                    locked: false,
                    opacity: 1.0,
                },
            )
            .expect("unlock remains available");
    }

    #[test]
    fn layer_crud_assignment_and_complete_history_are_atomic_and_fingerprint_neutral() {
        let first = VertexId::new();
        let second = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(10.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start: first,
                end: second,
                kind: EdgeKind::Mountain,
            }],
        };
        let mut editor = EditorState::new(pattern);
        let initial_layers = editor.project_layers().clone();
        let fingerprint = editor.fold_model_fingerprint_v1();
        let crease = test_layer("Details");
        let annotation = LayerRecordV1 {
            id: LayerId::new(),
            name: "Notes".to_owned(),
            content_kind: ori_domain::LayerContentKindV1::Annotation,
            visible: true,
            locked: false,
            opacity: 1.0,
        };

        editor
            .execute(
                0,
                Command::CreateLayer {
                    layer: crease.clone(),
                    target_index: 1,
                },
            )
            .expect("create crease layer");
        editor
            .execute(
                1,
                Command::RenameLayer {
                    layer: crease.id,
                    name: "Fine details".to_owned(),
                },
            )
            .expect("rename layer");
        editor
            .execute(
                2,
                Command::CreateLayer {
                    layer: annotation.clone(),
                    target_index: 1,
                },
            )
            .expect("create annotation layer");
        editor
            .execute(
                3,
                Command::MoveLayer {
                    layer: crease.id,
                    target_index: 0,
                },
            )
            .expect("reorder layer");
        editor
            .execute(
                4,
                Command::AssignEdgeToLayer {
                    edge,
                    layer: crease.id,
                },
            )
            .expect("assign edge");
        assert_eq!(editor.project_layers().layer_for_edge(edge), crease.id);
        assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);

        let before_failure = editor_state_snapshot(&editor);
        assert!(matches!(
            editor.execute(
                5,
                Command::AssignEdgeToLayer {
                    edge,
                    layer: annotation.id,
                },
            ),
            Err(CommandError::ProjectLayerDocumentInvalid(
                ProjectLayerDocumentValidationErrorV1::AssignmentLayerWrongContentKind { .. }
            ))
        ));
        assert_eq!(editor_state_snapshot(&editor), before_failure);
        assert_eq!(
            editor.execute(
                5,
                Command::DeleteLayer {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                },
            ),
            Err(CommandError::DefaultLayerDeletionForbidden)
        );
        assert_eq!(editor_state_snapshot(&editor), before_failure);

        editor
            .execute(5, Command::DeleteLayer { layer: crease.id })
            .expect("delete assigned layer");
        assert_eq!(
            editor.project_layers().layer_for_edge(edge),
            DEFAULT_PROJECT_LAYER_ID
        );
        let final_layers = editor.project_layers().clone();

        for revision in 6..12 {
            editor.undo(revision).expect("undo complete layer history");
        }
        assert_eq!(editor.project_layers(), &initial_layers);
        assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);

        for revision in 12..18 {
            editor.redo(revision).expect("redo complete layer history");
        }
        assert_eq!(editor.project_layers(), &final_layers);
        assert_eq!(editor.fold_model_fingerprint_v1(), fingerprint);
    }

    #[test]
    fn remove_and_split_edge_preserve_explicit_layer_assignments_exactly() {
        let first = VertexId::new();
        let second = VertexId::new();
        let source = Edge {
            id: EdgeId::new(),
            start: first,
            end: second,
            kind: EdgeKind::Valley,
        };
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(10.0, 0.0),
                },
            ],
            edges: vec![source.clone()],
        };
        let layer = test_layer("Fold");

        let mut remove_editor = editor_with_test_layers(
            pattern.clone(),
            Paper::default(),
            vec![layer.clone()],
            vec![(source.id, layer.id)],
        );
        let original_layers = remove_editor.project_layers().clone();
        remove_editor
            .execute(0, Command::RemoveEdge { id: source.id })
            .expect("remove assigned edge");
        assert!(remove_editor.project_layers().edge_assignments.is_empty());
        remove_editor.undo(1).expect("undo assigned removal");
        assert_eq!(remove_editor.project_layers(), &original_layers);
        remove_editor.redo(2).expect("redo assigned removal");
        assert!(remove_editor.project_layers().edge_assignments.is_empty());

        let mut split_editor = editor_with_test_layers(
            pattern,
            Paper::default(),
            vec![layer.clone()],
            vec![(source.id, layer.id)],
        );
        let new_vertex = VertexId::new();
        let new_edge = EdgeId::new();
        let original_layers = split_editor.project_layers().clone();
        split_editor
            .execute(
                0,
                Command::SplitEdge {
                    edge: source.id,
                    new_vertex,
                    new_edge,
                    fraction: 0.5,
                },
            )
            .expect("split assigned edge");
        assert_eq!(
            split_editor.project_layers().layer_for_edge(source.id),
            layer.id
        );
        assert_eq!(
            split_editor.project_layers().layer_for_edge(new_edge),
            layer.id
        );
        split_editor.undo(1).expect("undo assigned split");
        assert_eq!(split_editor.project_layers(), &original_layers);
        split_editor.redo(2).expect("redo assigned split");
        assert_eq!(
            split_editor.project_layers().layer_for_edge(new_edge),
            layer.id
        );

        let third = VertexId::new();
        let fourth = VertexId::new();
        let added_edge = EdgeId::new();
        let add_pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(10.0, 0.0),
                },
                Vertex {
                    id: third,
                    position: Point2::new(0.0, 10.0),
                },
                Vertex {
                    id: fourth,
                    position: Point2::new(10.0, 10.0),
                },
            ],
            edges: vec![source.clone()],
        };
        let mut add_editor = editor_with_test_layers(
            add_pattern,
            Paper::default(),
            vec![layer.clone()],
            vec![(source.id, layer.id)],
        );
        let original_layers = add_editor.project_layers().clone();
        add_editor
            .execute(
                0,
                Command::AddEdge {
                    id: added_edge,
                    start: third,
                    end: fourth,
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add an independently authored edge");
        assert_eq!(
            add_editor.project_layers().layer_for_edge(added_edge),
            DEFAULT_PROJECT_LAYER_ID
        );
        assert_eq!(add_editor.project_layers(), &original_layers);
        add_editor.undo(1).expect("undo default-layer edge");
        assert_eq!(add_editor.project_layers(), &original_layers);
        add_editor.redo(2).expect("redo default-layer edge");
        assert_eq!(
            add_editor.project_layers().layer_for_edge(added_edge),
            DEFAULT_PROJECT_LAYER_ID
        );
    }

    #[test]
    fn boundary_split_and_vertex_removal_preserve_source_layer_lineage() {
        let (_, pattern, paper) = simple_rectangular_editor();
        let boundary = pattern.edges[0].clone();
        let layer = test_layer("Boundary details");
        let mut split_editor = editor_with_test_layers(
            pattern.clone(),
            paper.clone(),
            vec![layer.clone()],
            vec![(boundary.id, layer.id)],
        );
        let original_layers = split_editor.project_layers().clone();
        let new_vertex = VertexId::new();
        let new_edge = EdgeId::new();
        split_editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: boundary.id,
                    new_vertex,
                    new_edge,
                    fraction: 0.5,
                },
            )
            .expect("split assigned boundary");
        assert_eq!(
            split_editor.project_layers().layer_for_edge(new_edge),
            layer.id
        );
        split_editor.undo(1).expect("undo assigned boundary split");
        assert_eq!(split_editor.project_layers(), &original_layers);
        split_editor.redo(2).expect("redo assigned boundary split");
        assert_eq!(
            split_editor.project_layers().layer_for_edge(new_edge),
            layer.id
        );

        let removed_vertex = paper.boundary_vertices[1];
        let previous = paper.boundary_vertices[0];
        let next = paper.boundary_vertices[2];
        let preceding = pattern
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Boundary
                    && undirected_endpoints_match(edge.start, edge.end, previous, removed_vertex)
            })
            .expect("preceding boundary")
            .clone();
        let following = pattern
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Boundary
                    && undirected_endpoints_match(edge.start, edge.end, removed_vertex, next)
            })
            .expect("following boundary")
            .clone();
        let other_layer = test_layer("Other boundary");
        let mut removal_editor = editor_with_test_layers(
            pattern,
            paper,
            vec![layer.clone(), other_layer.clone()],
            vec![(preceding.id, layer.id), (following.id, other_layer.id)],
        );
        let original_layers = removal_editor.project_layers().clone();
        removal_editor
            .execute(
                0,
                Command::RemoveBoundaryVertex {
                    vertex: removed_vertex,
                },
            )
            .expect("remove layered boundary vertex");
        assert_eq!(
            removal_editor.project_layers().layer_for_edge(preceding.id),
            layer.id
        );
        assert!(
            removal_editor
                .project_layers()
                .edge_assignments
                .iter()
                .all(|assignment| assignment.edge != following.id)
        );
        removal_editor
            .undo(1)
            .expect("undo layered boundary vertex removal");
        assert_eq!(removal_editor.project_layers(), &original_layers);
        removal_editor
            .redo(2)
            .expect("redo layered boundary vertex removal");
        assert_eq!(
            removal_editor.project_layers().layer_for_edge(preceding.id),
            layer.id
        );
        assert!(
            removal_editor
                .project_layers()
                .edge_assignments
                .iter()
                .all(|assignment| assignment.edge != following.id)
        );
    }

    #[test]
    fn intersection_t_junction_and_cluster_inherit_each_actual_source_layer() {
        let (_, pattern, paper, first, second) = crossing_edges_editor();
        let first_layer = test_layer("First");
        let second_layer = test_layer("Second");
        let mut crossing = editor_with_test_layers(
            pattern,
            paper,
            vec![first_layer.clone(), second_layer.clone()],
            vec![(first.id, first_layer.id), (second.id, second_layer.id)],
        );
        let crossing_layers = crossing.project_layers().clone();
        let first_new = EdgeId::new();
        let second_new = EdgeId::new();
        crossing
            .execute(
                0,
                Command::ConnectEdgeIntersection {
                    first_edge: first.id,
                    second_edge: second.id,
                    new_vertex: VertexId::new(),
                    first_new_edge: first_new,
                    second_new_edge: second_new,
                },
            )
            .expect("connect layered crossing");
        assert_eq!(
            crossing.project_layers().layer_for_edge(first_new),
            first_layer.id
        );
        assert_eq!(
            crossing.project_layers().layer_for_edge(second_new),
            second_layer.id
        );
        crossing.undo(1).expect("undo layered crossing");
        assert_eq!(crossing.project_layers(), &crossing_layers);
        crossing.redo(2).expect("redo layered crossing");
        assert_eq!(
            crossing.project_layers().layer_for_edge(first_new),
            first_layer.id
        );
        assert_eq!(
            crossing.project_layers().layer_for_edge(second_new),
            second_layer.id
        );

        let (_, pattern, paper, interior, stem, _) = t_junction_editor();
        let mut junction = editor_with_test_layers(
            pattern,
            paper,
            vec![first_layer.clone()],
            vec![(interior.id, first_layer.id)],
        );
        let junction_layers = junction.project_layers().clone();
        let junction_new = EdgeId::new();
        junction
            .execute(
                0,
                Command::ConnectTJunction {
                    first_edge: stem.id,
                    second_edge: interior.id,
                    new_edge: junction_new,
                },
            )
            .expect("connect reversed layered T junction");
        assert_eq!(
            junction.project_layers().layer_for_edge(junction_new),
            first_layer.id
        );
        junction.undo(1).expect("undo layered T junction");
        assert_eq!(junction.project_layers(), &junction_layers);
        junction.redo(2).expect("redo layered T junction");
        assert_eq!(
            junction.project_layers().layer_for_edge(junction_new),
            first_layer.id
        );

        let center_edges = [
            (
                Point2::new(-10.0, 0.0),
                Point2::new(10.0, 0.0),
                EdgeKind::Mountain,
            ),
            (
                Point2::new(0.0, -10.0),
                Point2::new(0.0, 10.0),
                EdgeKind::Valley,
            ),
            (
                Point2::new(-10.0, -10.0),
                Point2::new(10.0, 10.0),
                EdgeKind::Auxiliary,
            ),
        ];
        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        for (start, end, kind) in center_edges {
            let start_id = VertexId::new();
            let end_id = VertexId::new();
            vertices.push(Vertex {
                id: start_id,
                position: start,
            });
            vertices.push(Vertex {
                id: end_id,
                position: end,
            });
            edges.push(Edge {
                id: EdgeId::new(),
                start: start_id,
                end: end_id,
                kind,
            });
        }
        let generated = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
        let mut cluster = editor_with_test_layers(
            CreasePattern {
                vertices,
                edges: edges.clone(),
            },
            Paper::default(),
            vec![first_layer.clone(), second_layer.clone()],
            vec![
                (edges[0].id, first_layer.id),
                (edges[2].id, second_layer.id),
            ],
        );
        let cluster_layers = cluster.project_layers().clone();
        cluster
            .execute(
                0,
                Command::ConnectIntersectionCluster {
                    junction: JunctionVertexIntent::Create {
                        id: VertexId::new(),
                    },
                    targets: edges
                        .iter()
                        .zip(generated)
                        .map(|(edge, new_edge)| IntersectionEdgeTarget {
                            edge: edge.id,
                            new_edge: Some(new_edge),
                        })
                        .collect(),
                },
            )
            .expect("connect layered intersection cluster");
        assert_eq!(
            cluster.project_layers().layer_for_edge(generated[0]),
            first_layer.id
        );
        assert_eq!(
            cluster.project_layers().layer_for_edge(generated[1]),
            DEFAULT_PROJECT_LAYER_ID
        );
        assert_eq!(
            cluster.project_layers().layer_for_edge(generated[2]),
            second_layer.id
        );
        cluster.undo(1).expect("undo layered cluster");
        assert_eq!(cluster.project_layers(), &cluster_layers);
        cluster.redo(2).expect("redo layered cluster");
        assert_eq!(
            cluster.project_layers().layer_for_edge(generated[0]),
            first_layer.id
        );
        assert_eq!(
            cluster.project_layers().layer_for_edge(generated[1]),
            DEFAULT_PROJECT_LAYER_ID
        );
        assert_eq!(
            cluster.project_layers().layer_for_edge(generated[2]),
            second_layer.id
        );
    }

    #[test]
    fn layer_index_edge_limit_rejects_growth_atomically_when_assignments_are_active() {
        let pattern = crate::benchmark_pattern(MAX_PROJECT_LAYER_INDEX_EDGES);
        let source = pattern.edges[0].clone();
        let layer = test_layer("Indexed");
        let mut editor = editor_with_test_layers(
            pattern,
            Paper::default(),
            vec![layer.clone()],
            vec![(source.id, layer.id)],
        );
        let before = editor_state_snapshot(&editor);

        assert_eq!(
            editor.execute(
                0,
                Command::AddEdge {
                    id: EdgeId::new(),
                    start: source.start,
                    end: source.end,
                    kind: EdgeKind::Mountain,
                },
            ),
            Err(CommandError::ProjectLayerDocumentInvalid(
                ProjectLayerDocumentValidationErrorV1::TooManyPatternEdges {
                    actual: MAX_PROJECT_LAYER_INDEX_EDGES + 1,
                    maximum: MAX_PROJECT_LAYER_INDEX_EDGES,
                },
            ))
        );
        assert_eq!(editor_state_snapshot(&editor), before);

        // Unchecked constructors intentionally admit repairable legacy data.
        // Once such a document is already oversized, non-growing layer
        // commands must remain available so the explicit index can be removed.
        editor.pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: source.start,
            end: source.end,
            kind: EdgeKind::Auxiliary,
        });
        editor
            .execute(0, Command::DeleteLayer { layer: layer.id })
            .expect("a non-growing command can repair an oversized loaded layer index");
        assert!(editor.project_layers().edge_assignments.is_empty());
    }

    #[test]
    fn deterministic_layer_command_sequences_round_trip_as_a_property() {
        let pattern = crate::benchmark_pattern(16);
        let initial_fingerprint = EditorState::new(pattern.clone()).fold_model_fingerprint_v1();

        for seed in 1_u64..=32 {
            let mut editor = EditorState::new(pattern.clone());
            let initial_layers = editor.project_layers().clone();
            let layers = (0..4)
                .map(|index| test_layer(&format!("Layer {index}")))
                .collect::<Vec<_>>();
            let mut revision = 0;
            for (index, layer) in layers.iter().enumerate() {
                editor
                    .execute(
                        revision,
                        Command::CreateLayer {
                            layer: layer.clone(),
                            target_index: index + 1,
                        },
                    )
                    .expect("create property-test layer");
                revision += 1;
            }

            let mut state = seed;
            for step in 0..64 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let layer_index = (state as usize) % layers.len();
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let command = match state % 3 {
                    0 => {
                        let edge = pattern.edges[(state as usize) % pattern.edges.len()].id;
                        let assigned_layer = if (state >> 8).is_multiple_of(5) {
                            DEFAULT_PROJECT_LAYER_ID
                        } else {
                            layers[layer_index].id
                        };
                        Command::AssignEdgeToLayer {
                            edge,
                            layer: assigned_layer,
                        }
                    }
                    1 => Command::RenameLayer {
                        layer: layers[layer_index].id,
                        name: format!("Layer {layer_index} seed {seed} step {step}"),
                    },
                    _ => Command::MoveLayer {
                        layer: layers[layer_index].id,
                        target_index: ((state >> 16) as usize)
                            % editor.project_layers().layers.len(),
                    },
                };
                editor
                    .execute(revision, command)
                    .expect("execute deterministic property command");
                validate_project_layer_document_against_pattern_v1(
                    editor.project_layers(),
                    editor.pattern(),
                )
                .expect("every generated state must remain valid");
                assert_eq!(editor.fold_model_fingerprint_v1(), initial_fingerprint);
                revision += 1;
            }

            let final_layers = editor.project_layers().clone();
            let operation_count = revision;
            for _ in 0..operation_count {
                editor
                    .undo(revision)
                    .expect("undo deterministic property sequence");
                revision += 1;
            }
            assert_eq!(editor.project_layers(), &initial_layers);
            assert_eq!(editor.fold_model_fingerprint_v1(), initial_fingerprint);

            for _ in 0..operation_count {
                editor
                    .redo(revision)
                    .expect("redo deterministic property sequence");
                revision += 1;
            }
            assert_eq!(editor.project_layers(), &final_layers);
            assert_eq!(editor.fold_model_fingerprint_v1(), initial_fingerprint);
        }
    }

    #[test]
    fn mirror_selection_duplicate_is_atomic_and_undo_redo_exact() {
        let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
        let (mut pattern, paper) = sheet.into_parts();
        let mut source_vertices = [VertexId::new(), VertexId::new()];
        source_vertices.sort_by_key(|id| id.canonical_bytes());
        let source_edge = EdgeId::new();
        pattern.vertices.extend([
            Vertex {
                id: source_vertices[0],
                position: Point2::new(20.0, 30.0),
            },
            Vertex {
                id: source_vertices[1],
                position: Point2::new(30.0, 40.0),
            },
        ]);
        pattern.edges.push(Edge {
            id: source_edge,
            start: source_vertices[0],
            end: source_vertices[1],
            kind: EdgeKind::Mountain,
        });
        let mut editor = EditorState::with_paper(pattern.clone(), paper);
        let mut new_vertices = [VertexId::new(), VertexId::new()];
        new_vertices.sort_by_key(|id| id.canonical_bytes());
        let new_edge = EdgeId::new();
        let command = Command::MirrorSelection {
            vertices: source_vertices.to_vec(),
            edges: vec![source_edge],
            axis: MirrorAxisV1 {
                start: Point2::new(50.0, 0.0),
                end: Point2::new(50.0, 100.0),
            },
            mode: MirrorSelectionModeV1::Duplicate,
            new_vertices: new_vertices.to_vec(),
            new_edges: vec![new_edge],
        };

        let result = editor.execute(0, command.clone()).unwrap();
        assert_eq!(result.revision, 1);
        assert_eq!(editor.pattern().vertices.len(), pattern.vertices.len() + 2);
        assert_eq!(editor.pattern().edges.len(), pattern.edges.len() + 1);
        assert_eq!(
            editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == new_vertices[0])
                .unwrap()
                .position,
            Point2::new(80.0, 30.0)
        );
        editor.undo(1).unwrap();
        assert_eq!(editor.pattern(), &pattern);
        editor.redo(2).unwrap();
        assert_eq!(editor.pattern().vertices.len(), pattern.vertices.len() + 2);
        assert_eq!(
            editor.execute(3, command),
            Err(CommandError::InvalidMirrorSelection)
        );
    }

    #[test]
    fn mirror_selection_rejects_missing_edges_and_locked_incident_layers() {
        let sheet = crate::create_rectangular_sheet(100.0, 100.0, false).unwrap();
        let (mut pattern, paper) = sheet.into_parts();
        let mut vertices = [VertexId::new(), VertexId::new(), VertexId::new()];
        vertices.sort_by_key(|id| id.canonical_bytes());
        let selected_edge = EdgeId::new();
        let incident_edge = EdgeId::new();
        pattern.vertices.extend([
            Vertex {
                id: vertices[0],
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: vertices[1],
                position: Point2::new(30.0, 30.0),
            },
            Vertex {
                id: vertices[2],
                position: Point2::new(40.0, 20.0),
            },
        ]);
        pattern.edges.extend([
            Edge {
                id: selected_edge,
                start: vertices[0],
                end: vertices[1],
                kind: EdgeKind::Valley,
            },
            Edge {
                id: incident_edge,
                start: vertices[1],
                end: vertices[2],
                kind: EdgeKind::Mountain,
            },
        ]);
        let mut editor = EditorState::with_paper(pattern, paper);
        let mut locked_layer = test_layer("Locked");
        locked_layer.locked = true;
        editor.project_layers.layers.push(locked_layer.clone());
        editor
            .project_layers
            .edge_assignments
            .push(EdgeLayerAssignmentV1 {
                edge: incident_edge,
                layer: locked_layer.id,
            });
        let axis = MirrorAxisV1 {
            start: Point2::new(50.0, 0.0),
            end: Point2::new(50.0, 100.0),
        };
        let missing_edge = EdgeId::new();

        assert_eq!(
            editor.execute(
                0,
                Command::MirrorSelection {
                    vertices: vertices[..2].to_vec(),
                    edges: vec![missing_edge],
                    axis,
                    mode: MirrorSelectionModeV1::Move,
                    new_vertices: vec![],
                    new_edges: vec![],
                },
            ),
            Err(CommandError::EdgeNotFound(missing_edge))
        );
        assert_eq!(
            editor.execute(
                0,
                Command::MirrorSelection {
                    vertices: vertices[..2].to_vec(),
                    edges: vec![selected_edge],
                    axis,
                    mode: MirrorSelectionModeV1::Move,
                    new_vertices: vec![],
                    new_edges: vec![],
                },
            ),
            Err(CommandError::LayerLocked(locked_layer.id))
        );
    }

    #[test]
    fn instruction_split_merge_is_atomic_undoable_and_symmetric() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let fingerprint = editor.fold_model_fingerprint_v1();
        let original = instruction_step(InstructionStepId::new(), "長い手順", fingerprint);
        editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: original.clone(),
                },
            )
            .unwrap();
        let mut first = original.clone();
        first.duration_ms = 700;
        let mut second = original.clone();
        second.id = InstructionStepId::new();
        second.duration_ms = 800;
        let split = InstructionTimeline {
            steps: vec![first, second],
        };
        editor
            .execute(
                1,
                Command::RewriteInstructionTimelineSplitMerge {
                    timeline: split.clone(),
                },
            )
            .unwrap();
        assert_eq!(editor.instruction_timeline(), &split);
        editor.undo(2).unwrap();
        assert_eq!(editor.instruction_timeline().steps, vec![original.clone()]);
        editor.redo(3).unwrap();
        editor
            .execute(
                4,
                Command::RewriteInstructionTimelineSplitMerge {
                    timeline: InstructionTimeline {
                        steps: vec![original.clone()],
                    },
                },
            )
            .unwrap();
        assert_eq!(editor.instruction_timeline().steps, vec![original]);
        let mut forged = editor.instruction_timeline().clone();
        forged.steps[0].title = "改ざん".to_owned();
        assert_eq!(
            editor.execute(
                5,
                Command::RewriteInstructionTimelineSplitMerge { timeline: forged }
            ),
            Err(CommandError::InstructionStepAppendHistoryMismatch),
        );
    }

    #[test]
    fn beginner_design_profile_is_validated_and_undoable() {
        let (mut editor, _, _) = simple_rectangular_editor();
        let initial = editor.beginner_design_profile().clone();
        let profile = BeginnerDesignProfileV1 {
            schema_version: ori_domain::BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1,
            preset: ori_domain::BeginnerDesignPresetV1::ShapePriority,
            shape_fidelity_weight: 60,
            foldability_weight: 20,
            step_count_weight: 10,
            paper_efficiency_weight: 10,
            generation_constraints: ori_domain::BeginnerGenerationConstraintsV1::default(),
            generation_provenance: None,
            reference_surface_landmarks_tenths_mm: Some(vec![[10, 20, 30], [40, 50, 60]]),
            outline_edit_authority: None,
            archived_reference_model_asset_ids: Vec::new(),
        };
        editor
            .execute(
                0,
                Command::UpdateBeginnerDesignProfile {
                    profile: Box::new(profile.clone()),
                },
            )
            .unwrap();
        assert_eq!(editor.beginner_design_profile(), &profile);
        editor.undo(1).unwrap();
        assert_eq!(editor.beginner_design_profile(), &initial);
        editor.redo(2).unwrap();
        assert_eq!(editor.beginner_design_profile(), &profile);

        let mut invalid = profile.clone();
        invalid.paper_efficiency_weight = 11;
        assert_eq!(
            editor.execute(
                3,
                Command::UpdateBeginnerDesignProfile {
                    profile: Box::new(invalid),
                },
            ),
            Err(CommandError::InvalidBeginnerDesignProfile)
        );
        assert_eq!(editor.beginner_design_profile(), &profile);
    }
}
