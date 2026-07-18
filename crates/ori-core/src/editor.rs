use std::collections::{HashMap, HashSet, hash_map::Entry};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, InstructionPose, InstructionStep, InstructionStepId,
    InstructionTimeline, InstructionTimelineValidationError, Paper, Point2, RgbaColor, Vertex,
    VertexId, validate_instruction_timeline,
};
use ori_geometry::{
    GeometryError, Orientation, PointSegmentRelation, SegmentIntersection, exact_orientation,
    point_segment_relation, segment_intersection, validate_crease_pattern, validate_paper,
};
use thiserror::Error;

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

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddVertex {
        id: VertexId,
        position: Point2,
    },
    MoveVertex {
        id: VertexId,
        position: Point2,
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
        cutting_allowed: bool,
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
    AddInstructionStep {
        step: InstructionStep,
    },
    UpdateInstructionStepMetadata {
        step_id: InstructionStepId,
        title: String,
        description: String,
        caution: String,
        duration_ms: u32,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    pub revision: Revision,
    pub changed_vertices: Vec<VertexId>,
    pub changed_edges: Vec<EdgeId>,
    pub settings_changed: bool,
    pub instructions_changed: bool,
}

#[derive(Debug, Error, PartialEq)]
pub enum CommandError {
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
    #[error("instruction step target index {target_index} is out of bounds for {step_count} steps")]
    InstructionStepTargetIndexOutOfBounds {
        target_index: usize,
        step_count: usize,
    },
    #[error("invalid instruction timeline: {0}")]
    InstructionTimelineInvalid(#[from] InstructionTimelineValidationError),
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    forward: Command,
    inverse: Inverse,
}

const MAX_EDITOR_HISTORY_ENTRIES: usize = 128;

fn push_bounded_history(stack: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    let discard_count = stack
        .len()
        .saturating_add(1)
        .saturating_sub(MAX_EDITOR_HISTORY_ENTRIES);
    if discard_count > 0 {
        stack.drain(..discard_count);
    }
    stack.push(entry);
}

#[derive(Debug, Clone)]
enum Inverse {
    Command(Command),
    RestoreVertex {
        index: usize,
        vertex: Vertex,
    },
    RestoreEdge {
        index: usize,
        edge: Edge,
    },
    RestorePaperProperties {
        thickness_mm: f64,
        front_color: RgbaColor,
        back_color: RgbaColor,
        cutting_allowed: bool,
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
    },
    RestoreEdgeSplit {
        original_edge_index: usize,
        original_edge: Edge,
        new_vertex_index: usize,
        new_vertex: Vertex,
        new_edge_index: usize,
        new_edge: Edge,
    },
    RestoreEdgeIntersection {
        original_edges: [(usize, Edge); 2],
        new_edges: [(usize, Edge); 2],
        new_vertex_index: usize,
        new_vertex: Vertex,
    },
    RestoreTJunction {
        original_edge_index: usize,
        original_edge: Edge,
        new_edge_index: usize,
        new_edge: Edge,
        boundary_vertices: Option<Vec<VertexId>>,
        changed_vertices: [VertexId; 4],
        changed_edges: [EdgeId; 3],
    },
    RestoreIntersectionCluster {
        original_boundary_vertices: Option<Vec<VertexId>>,
        original_edges: Vec<(usize, Edge)>,
        inserted_edges: Vec<(usize, Edge)>,
        created_vertex: Option<(usize, Vertex)>,
        junction_vertex: VertexId,
        changed_vertices: Vec<VertexId>,
        changed_edges: Vec<EdgeId>,
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
    },
    RemoveAddedInstructionStep {
        step_id: InstructionStepId,
    },
    RestoreInstructionStepMetadata {
        step_id: InstructionStepId,
        title: String,
        description: String,
        caution: String,
        duration_ms: u32,
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
}

const MAX_INTERSECTION_CLUSTER_TARGETS: usize = 64;
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

#[derive(Debug, Clone)]
pub struct EditorState {
    pattern: CreasePattern,
    paper: Paper,
    instruction_timeline: InstructionTimeline,
    revision: Revision,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
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
    pub const fn with_paper(pattern: CreasePattern, paper: Paper) -> Self {
        Self::with_document_parts(pattern, paper, InstructionTimeline { steps: Vec::new() })
    }

    /// Restores all persisted, user-editable document parts.
    ///
    /// The restored state starts at revision zero with empty undo and redo
    /// histories. Validation remains an explicit admission step so callers can
    /// inspect or repair documents created by an older version.
    #[must_use]
    pub const fn with_document_parts(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
    ) -> Self {
        Self {
            pattern,
            paper,
            instruction_timeline,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    #[must_use]
    pub const fn pattern(&self) -> &CreasePattern {
        &self.pattern
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
    pub const fn instruction_timeline(&self) -> &InstructionTimeline {
        &self.instruction_timeline
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

    pub fn execute(
        &mut self,
        expected_revision: Revision,
        command: Command,
    ) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        let result = command.changes(&self.pattern, &self.paper);
        let inverse = self.apply(&command)?;
        push_bounded_history(
            &mut self.undo_stack,
            HistoryEntry {
                forward: command,
                inverse,
            },
        );
        self.redo_stack.clear();
        self.revision = next_revision;
        Ok(self.result(result))
    }

    pub fn undo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        let Some(entry) = self.undo_stack.last().cloned() else {
            return Ok(self.result(Changes::default()));
        };
        let result = entry.inverse.changes(&self.pattern, &self.paper);
        self.apply_inverse(&entry.inverse)?;
        let entry = self
            .undo_stack
            .pop()
            .expect("the successfully applied undo entry must still be present");
        push_bounded_history(&mut self.redo_stack, entry);
        self.revision = next_revision;
        Ok(self.result(result))
    }

    pub fn redo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let next_revision = self.next_revision()?;
        let Some(entry) = self.redo_stack.last().cloned() else {
            return Ok(self.result(Changes::default()));
        };
        let result = entry.forward.changes(&self.pattern, &self.paper);
        self.apply(&entry.forward)?;
        let entry = self
            .redo_stack
            .pop()
            .expect("the successfully applied redo entry must still be present");
        push_bounded_history(&mut self.undo_stack, entry);
        self.revision = next_revision;
        Ok(self.result(result))
    }

    fn apply(&mut self, command: &Command) -> Result<Inverse, CommandError> {
        match *command {
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
                let previous = self.pattern.vertices[index].position;
                self.pattern.vertices[index].position = position;
                Ok(Inverse::Command(Command::MoveVertex {
                    id,
                    position: previous,
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
            Command::RemoveEdge { id } => {
                let index = self.edge_index(id).ok_or(CommandError::EdgeNotFound(id))?;
                if self.pattern.edges[index].kind == EdgeKind::Boundary {
                    return Err(CommandError::BoundaryEdgeRequiresSheetOperation(id));
                }
                let edge = self.pattern.edges.remove(index);
                Ok(Inverse::RestoreEdge { index, edge })
            }
            Command::SetCuttingAllowed { allowed } => {
                self.ensure_cutting_can_be_set(allowed)?;
                let previous = self.paper.cutting_allowed;
                self.paper.cutting_allowed = allowed;
                Ok(Inverse::RestorePaperProperties {
                    thickness_mm: self.paper.thickness_mm,
                    front_color: self.paper.front.color,
                    back_color: self.paper.back.color,
                    cutting_allowed: previous,
                })
            }
            Command::UpdatePaperProperties {
                thickness_mm,
                front_color,
                back_color,
                cutting_allowed,
            } => {
                Self::validate_paper_thickness(thickness_mm)?;
                self.ensure_cutting_can_be_set(cutting_allowed)?;
                let inverse = Inverse::RestorePaperProperties {
                    thickness_mm: self.paper.thickness_mm,
                    front_color: self.paper.front.color,
                    back_color: self.paper.back.color,
                    cutting_allowed: self.paper.cutting_allowed,
                };
                self.paper.thickness_mm = thickness_mm;
                self.paper.front.color = front_color;
                self.paper.back.color = back_color;
                self.paper.cutting_allowed = cutting_allowed;
                Ok(inverse)
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
            Command::UpdateInstructionStepMetadata {
                step_id,
                ref title,
                ref description,
                ref caution,
                duration_ms,
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
                };
                step.title.clone_from(title);
                step.description.clone_from(description);
                step.caution.clone_from(caution);
                step.duration_ms = duration_ms;
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

        apply_boundary_vertex_removal(
            &mut self.pattern,
            &mut self.paper,
            boundary_index,
            vertex_index,
            kept_edge_index,
            removed_edge_index,
            &merged_edge,
        );

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

        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[original_edge_index].end = new_vertex_id;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());

        Ok(Inverse::RestoreEdgeSplit {
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
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

        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[splits[0].0].end = new_vertex_id;
        self.pattern.edges[splits[1].0].end = new_vertex_id;
        self.pattern
            .edges
            .insert(splits[1].0 + 1, created_edges[1].clone());
        self.pattern
            .edges
            .insert(splits[0].0 + 1, created_edges[0].clone());

        Ok(Inverse::RestoreEdgeIntersection {
            original_edges,
            new_edges: [
                (splits[0].0 + 1, created_edges[0].clone()),
                (splits[1].0 + 2, created_edges[1].clone()),
            ],
            new_vertex_index,
            new_vertex,
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

        if let Some((boundary_index, _)) = &boundary_change {
            self.paper
                .boundary_vertices
                .insert(*boundary_index + 1, *junction_vertex);
        }
        self.pattern.edges[original_edge_index].end = *junction_vertex;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());

        Ok(Inverse::RestoreTJunction {
            original_edge_index,
            original_edge,
            new_edge_index,
            new_edge,
            boundary_vertices: boundary_change.map(|(_, boundary_vertices)| boundary_vertices),
            changed_vertices,
            changed_edges,
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

        Ok(Inverse::RestoreIntersectionCluster {
            original_boundary_vertices: None,
            original_edges,
            inserted_edges,
            created_vertex,
            junction_vertex: junction_id,
            changed_vertices: plan.changed_vertices,
            changed_edges: plan.changed_edges,
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

        self.paper
            .boundary_vertices
            .insert(boundary_index + 1, new_vertex_id);
        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[original_edge_index].end = new_vertex_id;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());

        Ok(Inverse::RestoreBoundarySplit {
            boundary_vertices,
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
        })
    }

    fn resize_rectangular_paper(
        &mut self,
        width_mm: f64,
        height_mm: f64,
    ) -> Result<Inverse, CommandError> {
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

        let previous_positions = self
            .pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<Vec<_>>();
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

        for (vertex, position) in self.pattern.vertices.iter_mut().zip(resized_positions) {
            vertex.position = position;
        }
        Ok(Inverse::RestoreVertexPositions {
            vertices: previous_positions,
        })
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
            Inverse::Command(command) => {
                self.apply(command)?;
            }
            Inverse::RestoreVertex { index, vertex } => {
                debug_assert!(self.vertex_index(vertex.id).is_none());
                debug_assert!(*index <= self.pattern.vertices.len());
                self.pattern.vertices.insert(*index, vertex.clone());
            }
            Inverse::RestoreEdge { index, edge } => {
                debug_assert!(self.edge_index(edge.id).is_none());
                debug_assert!(*index <= self.pattern.edges.len());
                self.pattern.edges.insert(*index, edge.clone());
            }
            Inverse::RestorePaperProperties {
                thickness_mm,
                front_color,
                back_color,
                cutting_allowed,
            } => {
                self.paper.thickness_mm = *thickness_mm;
                self.paper.front.color = *front_color;
                self.paper.back.color = *back_color;
                self.paper.cutting_allowed = *cutting_allowed;
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
            } => {
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
            } => {
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
            } => {
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
                ..
            } => {
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
                ..
            } => {
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
            Inverse::RestoreInstructionStepMetadata {
                step_id,
                title,
                description,
                caution,
                duration_ms,
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
        }
    }
}

#[derive(Default)]
struct Changes {
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
    settings: bool,
    instructions: bool,
}

impl Command {
    fn changes(&self, pattern: &CreasePattern, paper: &Paper) -> Changes {
        match *self {
            Self::AddVertex { id, .. }
            | Self::MoveVertex { id, .. }
            | Self::RemoveVertex { id } => Changes {
                vertices: vec![id],
                edges: Vec::new(),
                settings: false,
                instructions: false,
            },
            Self::AddEdge { id, start, end, .. } => Changes {
                vertices: vec![start, end],
                edges: vec![id],
                settings: false,
                instructions: false,
            },
            Self::RemoveEdge { id } => Changes {
                vertices: Vec::new(),
                edges: vec![id],
                settings: false,
                instructions: false,
            },
            Self::SetCuttingAllowed { .. } | Self::UpdatePaperProperties { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
                instructions: false,
            },
            Self::ResizeRectangularPaper { .. } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: Vec::new(),
                settings: false,
                instructions: false,
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
                }
            }
            Self::AddInstructionStep { .. }
            | Self::UpdateInstructionStepMetadata { .. }
            | Self::ReplaceInstructionStepPose { .. }
            | Self::RemoveInstructionStep { .. }
            | Self::MoveInstructionStep { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: false,
                instructions: true,
            },
        }
    }
}

impl Inverse {
    fn changes(&self, pattern: &CreasePattern, paper: &Paper) -> Changes {
        match self {
            Self::Command(command) => command.changes(pattern, paper),
            Self::RestoreVertex { vertex, .. } => Changes {
                vertices: vec![vertex.id],
                edges: Vec::new(),
                settings: false,
                instructions: false,
            },
            Self::RestoreEdge { edge, .. } => Changes {
                vertices: vec![edge.start, edge.end],
                edges: vec![edge.id],
                settings: false,
                instructions: false,
            },
            Self::RestorePaperProperties { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
                instructions: false,
            },
            Self::RestoreVertexPositions { vertices } => Changes {
                vertices: vertices.iter().map(|(id, _)| *id).collect(),
                edges: Vec::new(),
                settings: false,
                instructions: false,
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
            },
            Self::RemoveAddedInstructionStep { .. }
            | Self::RestoreInstructionStepMetadata { .. }
            | Self::RestoreInstructionStepPose { .. }
            | Self::RestoreRemovedInstructionStep { .. }
            | Self::RestoreInstructionStepOrder { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: false,
                instructions: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct EditorStateSnapshot {
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        revision: Revision,
        undo_stack: String,
        redo_stack: String,
    }

    fn editor_state_snapshot(editor: &EditorState) -> EditorStateSnapshot {
        EditorStateSnapshot {
            pattern: editor.pattern.clone(),
            paper: editor.paper.clone(),
            instruction_timeline: editor.instruction_timeline.clone(),
            revision: editor.revision,
            undo_stack: format!("{:?}", editor.undo_stack),
            redo_stack: format!("{:?}", editor.redo_stack),
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
    fn paper_properties_are_one_undoable_command_and_preserve_textures() {
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
        assert_eq!(editor.paper().front.texture_asset, Some(front_texture));
        assert_eq!(editor.paper().back.texture_asset, Some(back_texture));
        assert!(editor.paper().cutting_allowed);

        editor.undo(1).expect("undo paper properties");
        assert_eq!(editor.paper(), &original);
        editor.redo(2).expect("redo paper properties");
        assert_eq!(editor.paper().thickness_mm, 0.0);
        assert_eq!(editor.paper().front.color, front_color);
        assert_eq!(editor.paper().back.color, back_color);
        assert_eq!(editor.paper().front.texture_asset, Some(front_texture));
        assert_eq!(editor.paper().back.texture_asset, Some(back_texture));
        assert!(editor.paper().cutting_allowed);
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
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
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
}
