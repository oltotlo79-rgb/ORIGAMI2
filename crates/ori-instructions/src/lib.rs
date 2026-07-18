//! Deterministic drawing plans derived from authored folding instructions.
//!
//! This crate owns the CPU-side pose and fixed-camera projection used by
//! portable instruction exports. It deliberately does not inspect the
//! interactive Three.js scene, the current viewport, or GPU pixels.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, InstructionTimeline, Paper, RgbaColor, VertexId,
    validate_instruction_timeline,
};
use ori_topology::{EdgeIncidence, FoldAssignment, TopologySnapshot};
use thiserror::Error;

const PREVIEW_WORLD_SIZE: f64 = 4.4;
const PROJECTION_QUANTIZATION: f64 = 1_000_000_000.0;
const DEGREES_TO_RADIANS: f64 = 0.017_453_292_519_943_295;

// Fixed orthographic camera looking from the same general quadrant as the
// interactive preview's default camera. Precomputed unit vectors avoid
// platform-dependent normalization in the serialized drawing boundary.
const CAMERA_TO_VIEWER: Vec3 = Vec3 {
    x: 0.577_350_269_189_625_8,
    y: 0.577_350_269_189_625_8,
    z: 0.577_350_269_189_625_8,
};
const CAMERA_RIGHT: Vec3 = Vec3 {
    x: std::f64::consts::FRAC_1_SQRT_2,
    y: 0.0,
    z: -std::f64::consts::FRAC_1_SQRT_2,
};
const CAMERA_UP: Vec3 = Vec3 {
    x: -0.408_248_290_463_863_1,
    y: 0.816_496_580_927_726_1,
    z: -0.408_248_290_463_863_1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionDiagramLimits {
    pub max_steps: usize,
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_faces_per_step: usize,
    pub max_hinges_per_step: usize,
    pub max_projected_vertex_visits: usize,
}

impl Default for InstructionDiagramLimits {
    fn default() -> Self {
        Self {
            max_steps: ori_domain::MAX_INSTRUCTION_STEPS,
            max_source_vertices: 100_000,
            max_source_edges: 100_000,
            max_faces_per_step: 10_001,
            max_hinges_per_step: ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP,
            max_projected_vertex_visits: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagramPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagramBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagramColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstructionDiagramFace {
    pub points: Vec<DiagramPoint>,
    pub fill: DiagramColor,
    pub depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionDiagramFoldKind {
    Mountain,
    Valley,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstructionDiagramHinge {
    pub start: DiagramPoint,
    pub end: DiagramPoint,
    pub kind: InstructionDiagramFoldKind,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstructionDiagramStep {
    pub faces: Vec<InstructionDiagramFace>,
    pub hinges: Vec<InstructionDiagramHinge>,
    pub changed_hinge_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstructionDiagramPlan {
    pub bounds: DiagramBounds,
    pub steps: Vec<InstructionDiagramStep>,
    /// Exact number of face-boundary and hinge-endpoint projections represented
    /// by this plan. This is part of the canonical export resource accounting.
    pub projected_vertex_visits: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstructionDiagramError {
    #[error("the instruction timeline is invalid")]
    InvalidTimeline,
    #[error("the instruction timeline is empty")]
    EmptyTimeline,
    #[error("instruction step {step_index} belongs to a different fold model")]
    StaleStep { step_index: usize },
    #[error("the instruction topology is invalid or unsupported")]
    UnsupportedTopology,
    #[error("the instruction geometry cannot be represented")]
    UnrepresentableGeometry,
    #[error("instruction drawing work exceeds the configured limit")]
    ResourceLimitExceeded,
}

#[derive(Clone, Copy)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Clone, Copy)]
struct Transform {
    rotation: [[f64; 3]; 3],
    translation: Vec3,
}

#[derive(Clone)]
struct HingeModel {
    edge: EdgeId,
    assignment: FoldAssignment,
    start: Vec3,
    end: Vec3,
    axis: Vec3,
}

#[derive(Clone, Copy)]
struct Neighbor {
    face: FaceId,
    hinge_index: usize,
    rotation_sign: f64,
}

struct PreparedModel {
    faces: Vec<PreparedFace>,
    face_indices: HashMap<FaceId, usize>,
    hinges: Vec<HingeModel>,
    adjacency: HashMap<FaceId, Vec<Neighbor>>,
}

struct PreparedFace {
    id: FaceId,
    points: Vec<Vec3>,
}

struct StepTransforms {
    faces: HashMap<FaceId, Transform>,
    hinge_parents: Vec<Transform>,
}

/// Builds a fixed-camera vector plan for every current instruction step.
///
/// All steps must describe the supplied fold model. A stale or unsupported
/// step rejects the complete export rather than producing a partial guide.
pub fn build_instruction_diagram_plan(
    current_fold_model_fingerprint: &str,
    pattern: &CreasePattern,
    paper: &Paper,
    timeline: &InstructionTimeline,
    topology: &TopologySnapshot,
) -> Result<InstructionDiagramPlan, InstructionDiagramError> {
    build_instruction_diagram_plan_with_limits(
        current_fold_model_fingerprint,
        pattern,
        paper,
        timeline,
        topology,
        InstructionDiagramLimits::default(),
    )
}

pub fn build_instruction_diagram_plan_with_limits(
    current_fold_model_fingerprint: &str,
    pattern: &CreasePattern,
    paper: &Paper,
    timeline: &InstructionTimeline,
    topology: &TopologySnapshot,
    limits: InstructionDiagramLimits,
) -> Result<InstructionDiagramPlan, InstructionDiagramError> {
    validate_instruction_timeline(timeline)
        .map_err(|_| InstructionDiagramError::InvalidTimeline)?;
    if timeline.steps.is_empty() {
        return Err(InstructionDiagramError::EmptyTimeline);
    }
    if timeline.steps.len() > limits.max_steps {
        return Err(InstructionDiagramError::ResourceLimitExceeded);
    }
    for (step_index, step) in timeline.steps.iter().enumerate() {
        if step.pose.source_model_fingerprint != current_fold_model_fingerprint {
            return Err(InstructionDiagramError::StaleStep { step_index });
        }
    }

    let model = prepare_model(pattern, paper, topology, limits)?;
    let face_visits_per_step = model
        .faces
        .iter()
        .try_fold(0_usize, |total, face| total.checked_add(face.points.len()))
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    let total_visits = checked_projected_vertex_visits(
        face_visits_per_step,
        model.hinges.len(),
        timeline.steps.len(),
        limits.max_projected_vertex_visits,
    )?;

    let mut bounds: Option<DiagramBounds> = None;
    let mut previous_angles = HashMap::<EdgeId, f64>::new();
    let mut steps = Vec::with_capacity(timeline.steps.len());
    for (step_index, step) in timeline.steps.iter().enumerate() {
        let angles = step
            .pose
            .hinge_angles
            .iter()
            .map(|angle| (angle.edge, angle.angle_degrees))
            .collect::<HashMap<_, _>>();
        if angles.len() != step.pose.hinge_angles.len()
            || angles.len() != model.hinges.len()
            || model
                .hinges
                .iter()
                .any(|hinge| !angles.contains_key(&hinge.edge))
        {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        let transforms = face_transforms(&model, step.pose.fixed_face, &angles, step_index)?;
        let mut rendered_faces = Vec::with_capacity(model.faces.len());
        for face in &model.faces {
            let transform = transforms
                .faces
                .get(&face.id)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            let mut depth_total = 0.0;
            let mut points = Vec::with_capacity(face.points.len());
            for point in &face.points {
                let transformed = transform.apply(*point)?;
                let (projected, depth) = project(transformed)?;
                bounds = Some(expand_bounds(bounds, projected));
                depth_total += depth;
                points.push(projected);
            }
            let normal = transform.rotate(Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            })?;
            let front_visible = dot(normal, CAMERA_TO_VIEWER) >= 0.0;
            let source_color = if front_visible {
                paper.front.color
            } else {
                paper.back.color
            };
            rendered_faces.push(InstructionDiagramFace {
                points,
                fill: composite_over_white(source_color),
                depth: quantize(depth_total / face.points.len() as f64)?,
            });
        }
        rendered_faces.sort_by(compare_faces);

        let mut rendered_hinges = Vec::with_capacity(model.hinges.len());
        let mut changed_hinge_count = 0;
        for (hinge_index, hinge) in model.hinges.iter().enumerate() {
            let parent_transform = transforms
                .hinge_parents
                .get(hinge_index)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            let (start, _) = project(parent_transform.apply(hinge.start)?)?;
            let (end, _) = project(parent_transform.apply(hinge.end)?)?;
            bounds = Some(expand_bounds(bounds, start));
            bounds = Some(expand_bounds(bounds, end));
            let current = angles
                .get(&hinge.edge)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            let previous = previous_angles.get(&hinge.edge).copied().unwrap_or(0.0);
            let changed = canonical_zero(current) != canonical_zero(previous);
            changed_hinge_count += usize::from(changed);
            rendered_hinges.push(InstructionDiagramHinge {
                start,
                end,
                kind: match hinge.assignment {
                    FoldAssignment::Mountain => InstructionDiagramFoldKind::Mountain,
                    FoldAssignment::Valley => InstructionDiagramFoldKind::Valley,
                },
                changed,
            });
        }
        rendered_hinges.sort_by(compare_hinges);
        previous_angles = angles;
        steps.push(InstructionDiagramStep {
            faces: rendered_faces,
            hinges: rendered_hinges,
            changed_hinge_count,
        });
    }

    let bounds = bounds.ok_or(InstructionDiagramError::UnrepresentableGeometry)?;
    if !(bounds.max_x > bounds.min_x && bounds.max_y > bounds.min_y) {
        return Err(InstructionDiagramError::UnrepresentableGeometry);
    }
    Ok(InstructionDiagramPlan {
        bounds,
        steps,
        projected_vertex_visits: total_visits,
    })
}

fn checked_projected_vertex_visits(
    face_vertices_per_step: usize,
    hinge_count: usize,
    step_count: usize,
    maximum: usize,
) -> Result<usize, InstructionDiagramError> {
    let hinge_endpoints_per_step = hinge_count
        .checked_mul(2)
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    let visits_per_step = face_vertices_per_step
        .checked_add(hinge_endpoints_per_step)
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    let total = visits_per_step
        .checked_mul(step_count)
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    if total > maximum {
        Err(InstructionDiagramError::ResourceLimitExceeded)
    } else {
        Ok(total)
    }
}

fn prepare_model(
    pattern: &CreasePattern,
    paper: &Paper,
    topology: &TopologySnapshot,
    limits: InstructionDiagramLimits,
) -> Result<PreparedModel, InstructionDiagramError> {
    checked_model_resource_counts(
        pattern.vertices.len(),
        pattern.edges.len(),
        topology.faces.len(),
        topology.hinge_adjacency.len(),
        limits,
    )?;
    if topology.faces.is_empty() {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }
    let positions = unique_positions(pattern)?;
    let edges = unique_edges(pattern)?;
    let frame = projection_frame(paper, &positions)?;

    let mut face_indices = HashMap::with_capacity(topology.faces.len());
    let mut faces = Vec::with_capacity(topology.faces.len());
    let mut face_keys = HashSet::with_capacity(topology.faces.len());
    let mut prepared_point_count = 0_usize;
    for (face_index, face) in topology.faces.iter().enumerate() {
        if face.outer.half_edges.len() < 3
            || face_indices.insert(face.id, face_index).is_some()
            || !face_keys.insert(face.key)
        {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        prepared_point_count = prepared_point_count
            .checked_add(face.outer.half_edges.len())
            .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
        if prepared_point_count > limits.max_projected_vertex_visits {
            return Err(InstructionDiagramError::ResourceLimitExceeded);
        }
        let mut points = Vec::with_capacity(face.outer.half_edges.len());
        for (index, half_edge) in face.outer.half_edges.iter().enumerate() {
            let next = &face.outer.half_edges[(index + 1) % face.outer.half_edges.len()];
            if half_edge.destination != next.origin {
                return Err(InstructionDiagramError::UnsupportedTopology);
            }
            let source = edges
                .get(&half_edge.edge)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            if !same_endpoints(
                source.start,
                source.end,
                half_edge.origin,
                half_edge.destination,
            ) {
                return Err(InstructionDiagramError::UnsupportedTopology);
            }
            let position = positions
                .get(&half_edge.origin)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            points.push(frame.to_world(position)?);
        }
        faces.push(PreparedFace {
            id: face.id,
            points,
        });
    }

    let incidences = topology
        .edge_incidence
        .iter()
        .map(|(edge, incidence)| (*edge, *incidence))
        .collect::<HashMap<_, _>>();
    if incidences.len() != topology.edge_incidence.len() {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }
    if topology.hinge_adjacency.is_empty() {
        if topology.faces.len() != 1 {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        return Ok(PreparedModel {
            faces,
            face_indices,
            hinges: Vec::new(),
            adjacency: HashMap::from([(topology.faces[0].id, Vec::new())]),
        });
    }
    if topology.hinge_adjacency.len() + 1 != topology.faces.len() {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }

    let mut hinges = Vec::with_capacity(topology.hinge_adjacency.len());
    let mut adjacency = topology
        .faces
        .iter()
        .map(|face| (face.id, Vec::new()))
        .collect::<HashMap<_, _>>();
    let mut hinge_edges = HashSet::with_capacity(topology.hinge_adjacency.len());
    for adjacent in &topology.hinge_adjacency {
        if adjacent.first == adjacent.second
            || !face_indices.contains_key(&adjacent.first)
            || !face_indices.contains_key(&adjacent.second)
            || !hinge_edges.insert(adjacent.edge)
        {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        let EdgeIncidence::Hinge {
            left,
            right,
            assignment,
        } = incidences
            .get(&adjacent.edge)
            .copied()
            .ok_or(InstructionDiagramError::UnsupportedTopology)?
        else {
            return Err(InstructionDiagramError::UnsupportedTopology);
        };
        if assignment != adjacent.assignment
            || !same_faces(adjacent.first, adjacent.second, left, right)
        {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        let edge = edges
            .get(&adjacent.edge)
            .ok_or(InstructionDiagramError::UnsupportedTopology)?;
        if !matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
            || edge.kind
                != match assignment {
                    FoldAssignment::Mountain => EdgeKind::Mountain,
                    FoldAssignment::Valley => EdgeKind::Valley,
                }
        {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        let (start_id, end_id) = canonical_endpoints(edge.start, edge.end);
        let start = frame.to_world(
            positions
                .get(&start_id)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?,
        )?;
        let end = frame.to_world(
            positions
                .get(&end_id)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?,
        )?;
        let delta = subtract(end, start)?;
        let length = length(delta)?;
        let axis = scale(delta, 1.0 / length)?;
        let hinge_index = hinges.len();
        hinges.push(HingeModel {
            edge: adjacent.edge,
            assignment,
            start,
            end,
            axis,
        });
        let sign = match assignment {
            FoldAssignment::Mountain => 1.0,
            FoldAssignment::Valley => -1.0,
        };
        adjacency
            .get_mut(&left)
            .ok_or(InstructionDiagramError::UnsupportedTopology)?
            .push(Neighbor {
                face: right,
                hinge_index,
                rotation_sign: sign,
            });
        adjacency
            .get_mut(&right)
            .ok_or(InstructionDiagramError::UnsupportedTopology)?
            .push(Neighbor {
                face: left,
                hinge_index,
                rotation_sign: -sign,
            });
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(|left, right| {
            hinges[left.hinge_index]
                .edge
                .canonical_bytes()
                .cmp(&hinges[right.hinge_index].edge.canonical_bytes())
        });
    }
    if !connected(&adjacency, topology.faces[0].id, topology.faces.len()) {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }
    hinges.sort_by_key(|hinge| hinge.edge.canonical_bytes());
    // Rebuild indices after canonical hinge ordering.
    let hinge_index_by_edge = hinges
        .iter()
        .enumerate()
        .map(|(index, hinge)| (hinge.edge, index))
        .collect::<HashMap<_, _>>();
    for neighbors in adjacency.values_mut() {
        for neighbor in neighbors.iter_mut() {
            let edge = topology
                .hinge_adjacency
                .get(neighbor.hinge_index)
                .map(|hinge| hinge.edge)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            neighbor.hinge_index = *hinge_index_by_edge
                .get(&edge)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
        }
        neighbors.sort_by_key(|neighbor| hinges[neighbor.hinge_index].edge.canonical_bytes());
    }
    Ok(PreparedModel {
        faces,
        face_indices,
        hinges,
        adjacency,
    })
}

fn checked_model_resource_counts(
    source_vertex_count: usize,
    source_edge_count: usize,
    face_count: usize,
    hinge_count: usize,
    limits: InstructionDiagramLimits,
) -> Result<(), InstructionDiagramError> {
    if source_vertex_count > limits.max_source_vertices
        || source_edge_count > limits.max_source_edges
        || face_count > limits.max_faces_per_step
        || hinge_count > limits.max_hinges_per_step
    {
        Err(InstructionDiagramError::ResourceLimitExceeded)
    } else {
        Ok(())
    }
}

fn face_transforms(
    model: &PreparedModel,
    fixed_face: Option<FaceId>,
    angles: &HashMap<EdgeId, f64>,
    _step_index: usize,
) -> Result<StepTransforms, InstructionDiagramError> {
    if model.hinges.is_empty() {
        if fixed_face.is_some() || !angles.is_empty() || model.faces.len() != 1 {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        return Ok(StepTransforms {
            faces: HashMap::from([(model.faces[0].id, Transform::IDENTITY)]),
            hinge_parents: Vec::new(),
        });
    }
    let root = fixed_face.ok_or(InstructionDiagramError::UnsupportedTopology)?;
    if !model.face_indices.contains_key(&root) {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }
    let mut transforms = HashMap::with_capacity(model.faces.len());
    let mut hinge_parents = vec![None; model.hinges.len()];
    transforms.insert(root, Transform::IDENTITY);
    let mut queue = VecDeque::from([root]);
    while let Some(parent_face) = queue.pop_front() {
        let parent = transforms
            .get(&parent_face)
            .copied()
            .ok_or(InstructionDiagramError::UnsupportedTopology)?;
        let neighbors = model
            .adjacency
            .get(&parent_face)
            .ok_or(InstructionDiagramError::UnsupportedTopology)?;
        for neighbor in neighbors {
            if transforms.contains_key(&neighbor.face) {
                continue;
            }
            let hinge = model
                .hinges
                .get(neighbor.hinge_index)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            if hinge_parents
                .get_mut(neighbor.hinge_index)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?
                .replace(parent)
                .is_some()
            {
                return Err(InstructionDiagramError::UnsupportedTopology);
            }
            let angle = angles
                .get(&hinge.edge)
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
                return Err(InstructionDiagramError::UnsupportedTopology);
            }
            let local =
                Transform::around_axis(hinge.start, hinge.axis, angle * neighbor.rotation_sign)?;
            let child = parent.compose(local)?;
            if transforms.insert(neighbor.face, child).is_some() {
                return Err(InstructionDiagramError::UnsupportedTopology);
            }
            queue.push_back(neighbor.face);
        }
    }
    if transforms.len() != model.faces.len() {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }
    let hinge_parents = hinge_parents
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(InstructionDiagramError::UnsupportedTopology)?;
    Ok(StepTransforms {
        faces: transforms,
        hinge_parents,
    })
}

struct ProjectionFrame {
    min_x: f64,
    min_y: f64,
    largest: f64,
    normalized_width: f64,
    normalized_height: f64,
}

impl ProjectionFrame {
    fn to_world(&self, point: ori_domain::Point2) -> Result<Vec3, InstructionDiagramError> {
        let x = ((point.x - self.min_x) / self.largest - self.normalized_width / 2.0)
            * PREVIEW_WORLD_SIZE;
        let z = -((point.y - self.min_y) / self.largest - self.normalized_height / 2.0)
            * PREVIEW_WORLD_SIZE;
        finite_vec(Vec3 { x, y: 0.0, z })
    }
}

fn projection_frame(
    paper: &Paper,
    positions: &HashMap<VertexId, ori_domain::Point2>,
) -> Result<ProjectionFrame, InstructionDiagramError> {
    if paper.boundary_vertices.len() < 3 {
        return Err(InstructionDiagramError::UnsupportedTopology);
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut boundary_ids = HashSet::with_capacity(paper.boundary_vertices.len());
    for id in &paper.boundary_vertices {
        if !boundary_ids.insert(*id) {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
        let point = positions
            .get(id)
            .copied()
            .ok_or(InstructionDiagramError::UnsupportedTopology)?;
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    let largest = width.max(height);
    if ![min_x, min_y, max_x, max_y, width, height, largest]
        .into_iter()
        .all(f64::is_finite)
        || width <= 0.0
        || height <= 0.0
        || largest <= 0.0
    {
        return Err(InstructionDiagramError::UnrepresentableGeometry);
    }
    Ok(ProjectionFrame {
        min_x,
        min_y,
        largest,
        normalized_width: width / largest,
        normalized_height: height / largest,
    })
}

fn unique_positions(
    pattern: &CreasePattern,
) -> Result<HashMap<VertexId, ori_domain::Point2>, InstructionDiagramError> {
    let mut positions = HashMap::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        if !vertex.position.x.is_finite()
            || !vertex.position.y.is_finite()
            || positions.insert(vertex.id, vertex.position).is_some()
        {
            return Err(InstructionDiagramError::UnrepresentableGeometry);
        }
    }
    Ok(positions)
}

fn unique_edges(
    pattern: &CreasePattern,
) -> Result<HashMap<EdgeId, &Edge>, InstructionDiagramError> {
    let mut edges = HashMap::with_capacity(pattern.edges.len());
    for edge in &pattern.edges {
        if edges.insert(edge.id, edge).is_some() {
            return Err(InstructionDiagramError::UnsupportedTopology);
        }
    }
    Ok(edges)
}

fn connected(adjacency: &HashMap<FaceId, Vec<Neighbor>>, root: FaceId, expected: usize) -> bool {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([root]);
    while let Some(face) = queue.pop_front() {
        if !visited.insert(face) {
            continue;
        }
        queue.extend(
            adjacency
                .get(&face)
                .into_iter()
                .flatten()
                .map(|neighbor| neighbor.face),
        );
    }
    visited.len() == expected
}

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn deterministic_sin_cos_degrees(
    angle_degrees: f64,
) -> Result<(f64, f64), InstructionDiagramError> {
    if !angle_degrees.is_finite() || !(-180.0..=180.0).contains(&angle_degrees) {
        return Err(InstructionDiagramError::UnrepresentableGeometry);
    }
    let (sine, cosine) = match canonical_zero(angle_degrees) {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        -90.0 => (-1.0, 0.0),
        180.0 | -180.0 => (0.0, -1.0),
        angle => libm::sincos(angle * DEGREES_TO_RADIANS),
    };
    if sine.is_finite() && cosine.is_finite() {
        Ok((canonical_zero(sine), canonical_zero(cosine)))
    } else {
        Err(InstructionDiagramError::UnrepresentableGeometry)
    }
}

impl Transform {
    const IDENTITY: Self = Self {
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    };

    fn around_axis(
        point: Vec3,
        axis: Vec3,
        angle_degrees: f64,
    ) -> Result<Self, InstructionDiagramError> {
        let (sine, cosine) = deterministic_sin_cos_degrees(angle_degrees)?;
        let one_minus = 1.0 - cosine;
        let (x, y, z) = (axis.x, axis.y, axis.z);
        let rotation = [
            [
                cosine + x * x * one_minus,
                x * y * one_minus - z * sine,
                x * z * one_minus + y * sine,
            ],
            [
                y * x * one_minus + z * sine,
                cosine + y * y * one_minus,
                y * z * one_minus - x * sine,
            ],
            [
                z * x * one_minus - y * sine,
                z * y * one_minus + x * sine,
                cosine + z * z * one_minus,
            ],
        ];
        let rotated_point = rotate_matrix(rotation, point)?;
        let translation = subtract(point, rotated_point)?;
        finite_transform(Self {
            rotation,
            translation,
        })
    }

    fn compose(self, local: Self) -> Result<Self, InstructionDiagramError> {
        let mut rotation = [[0.0; 3]; 3];
        for (row, target_row) in rotation.iter_mut().enumerate() {
            for (column, target) in target_row.iter_mut().enumerate() {
                *target = (0..3)
                    .map(|index| self.rotation[row][index] * local.rotation[index][column])
                    .sum();
            }
        }
        let translation = add(self.rotate(local.translation)?, self.translation)?;
        finite_transform(Self {
            rotation,
            translation,
        })
    }

    fn rotate(self, point: Vec3) -> Result<Vec3, InstructionDiagramError> {
        rotate_matrix(self.rotation, point)
    }

    fn apply(self, point: Vec3) -> Result<Vec3, InstructionDiagramError> {
        add(self.rotate(point)?, self.translation)
    }
}

fn rotate_matrix(matrix: [[f64; 3]; 3], point: Vec3) -> Result<Vec3, InstructionDiagramError> {
    finite_vec(Vec3 {
        x: matrix[0][0] * point.x + matrix[0][1] * point.y + matrix[0][2] * point.z,
        y: matrix[1][0] * point.x + matrix[1][1] * point.y + matrix[1][2] * point.z,
        z: matrix[2][0] * point.x + matrix[2][1] * point.y + matrix[2][2] * point.z,
    })
}

fn project(point: Vec3) -> Result<(DiagramPoint, f64), InstructionDiagramError> {
    Ok((
        DiagramPoint {
            x: quantize(dot(point, CAMERA_RIGHT))?,
            y: quantize(dot(point, CAMERA_UP))?,
        },
        quantize(dot(point, CAMERA_TO_VIEWER))?,
    ))
}

fn compare_faces(left: &InstructionDiagramFace, right: &InstructionDiagramFace) -> Ordering {
    left.depth
        .total_cmp(&right.depth)
        .then_with(|| compare_point_slices(&left.points, &right.points))
        .then_with(|| {
            (left.fill.red, left.fill.green, left.fill.blue).cmp(&(
                right.fill.red,
                right.fill.green,
                right.fill.blue,
            ))
        })
}

fn compare_hinges(left: &InstructionDiagramHinge, right: &InstructionDiagramHinge) -> Ordering {
    fold_kind_index(left.kind)
        .cmp(&fold_kind_index(right.kind))
        .then_with(|| left.start.x.total_cmp(&right.start.x))
        .then_with(|| left.start.y.total_cmp(&right.start.y))
        .then_with(|| left.end.x.total_cmp(&right.end.x))
        .then_with(|| left.end.y.total_cmp(&right.end.y))
        .then_with(|| left.changed.cmp(&right.changed))
}

fn compare_point_slices(left: &[DiagramPoint], right: &[DiagramPoint]) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| {
        left.iter()
            .zip(right)
            .map(|(left, right)| {
                left.x
                    .total_cmp(&right.x)
                    .then_with(|| left.y.total_cmp(&right.y))
            })
            .find(|ordering| *ordering != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    })
}

const fn fold_kind_index(kind: InstructionDiagramFoldKind) -> u8 {
    match kind {
        InstructionDiagramFoldKind::Mountain => 0,
        InstructionDiagramFoldKind::Valley => 1,
    }
}

fn expand_bounds(current: Option<DiagramBounds>, point: DiagramPoint) -> DiagramBounds {
    match current {
        None => DiagramBounds {
            min_x: point.x,
            min_y: point.y,
            max_x: point.x,
            max_y: point.y,
        },
        Some(bounds) => DiagramBounds {
            min_x: bounds.min_x.min(point.x),
            min_y: bounds.min_y.min(point.y),
            max_x: bounds.max_x.max(point.x),
            max_y: bounds.max_y.max(point.y),
        },
    }
}

fn composite_over_white(color: RgbaColor) -> DiagramColor {
    let alpha = u32::from(color.alpha);
    let blend =
        |channel: u8| ((u32::from(channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
    DiagramColor {
        red: blend(color.red),
        green: blend(color.green),
        blue: blend(color.blue),
    }
}

fn canonical_endpoints(first: VertexId, second: VertexId) -> (VertexId, VertexId) {
    if first.canonical_bytes() <= second.canonical_bytes() {
        (first, second)
    } else {
        (second, first)
    }
}

fn same_endpoints(
    first_start: VertexId,
    first_end: VertexId,
    second_start: VertexId,
    second_end: VertexId,
) -> bool {
    (first_start == second_start && first_end == second_end)
        || (first_start == second_end && first_end == second_start)
}

fn same_faces(first: FaceId, second: FaceId, left: FaceId, right: FaceId) -> bool {
    (first == left && second == right) || (first == right && second == left)
}

fn dot(first: Vec3, second: Vec3) -> f64 {
    first.x * second.x + first.y * second.y + first.z * second.z
}

fn add(first: Vec3, second: Vec3) -> Result<Vec3, InstructionDiagramError> {
    finite_vec(Vec3 {
        x: first.x + second.x,
        y: first.y + second.y,
        z: first.z + second.z,
    })
}

fn subtract(first: Vec3, second: Vec3) -> Result<Vec3, InstructionDiagramError> {
    finite_vec(Vec3 {
        x: first.x - second.x,
        y: first.y - second.y,
        z: first.z - second.z,
    })
}

fn scale(value: Vec3, scalar: f64) -> Result<Vec3, InstructionDiagramError> {
    finite_vec(Vec3 {
        x: value.x * scalar,
        y: value.y * scalar,
        z: value.z * scalar,
    })
}

fn length(value: Vec3) -> Result<f64, InstructionDiagramError> {
    let length = dot(value, value).sqrt();
    if length.is_finite() && length > 0.0 {
        Ok(length)
    } else {
        Err(InstructionDiagramError::UnrepresentableGeometry)
    }
}

fn finite_vec(value: Vec3) -> Result<Vec3, InstructionDiagramError> {
    [value.x, value.y, value.z]
        .into_iter()
        .all(f64::is_finite)
        .then_some(value)
        .ok_or(InstructionDiagramError::UnrepresentableGeometry)
}

fn finite_transform(value: Transform) -> Result<Transform, InstructionDiagramError> {
    value
        .rotation
        .into_iter()
        .flatten()
        .chain([
            value.translation.x,
            value.translation.y,
            value.translation.z,
        ])
        .all(f64::is_finite)
        .then_some(value)
        .ok_or(InstructionDiagramError::UnrepresentableGeometry)
}

fn quantize(value: f64) -> Result<f64, InstructionDiagramError> {
    if !value.is_finite() {
        return Err(InstructionDiagramError::UnrepresentableGeometry);
    }
    let quantized = (value * PROJECTION_QUANTIZATION).round() / PROJECTION_QUANTIZATION;
    if quantized.is_finite() {
        Ok(if quantized == 0.0 { 0.0 } else { quantized })
    } else {
        Err(InstructionDiagramError::UnrepresentableGeometry)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use ori_domain::{
        CreasePattern, Edge, EdgeId, EdgeKind, InstructionHingeAngle, InstructionPose,
        InstructionPoseModel, InstructionStep, InstructionStepId, InstructionTimeline, Paper,
        Point2, ProjectId, Vertex, VertexId,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};
    use sha2::{Digest, Sha256};

    use super::*;

    const FINGERPRINT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct FoldFixture {
        pattern: CreasePattern,
        paper: Paper,
        topology: TopologySnapshot,
        fold: EdgeId,
    }

    fn fixture_vertex_id(index: u64) -> VertexId {
        serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
            .expect("fixed vertex id")
    }

    fn fixture_edge_id(index: u64) -> EdgeId {
        serde_json::from_str(&format!("\"00000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed edge id")
    }

    fn fixture_project_id() -> ProjectId {
        serde_json::from_str("\"00000000-0000-4000-a000-000000000001\"").expect("fixed project id")
    }

    fn vertex(index: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: fixture_vertex_id(index),
            position: Point2::new(x, y),
        }
    }

    fn edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
        Edge {
            id: fixture_edge_id(index),
            start,
            end,
            kind,
        }
    }

    fn single_fold_fixture() -> FoldFixture {
        let vertices = vec![
            vertex(1, 0.0, 0.0),
            vertex(2, 5.0, 0.0),
            vertex(3, 10.0, 0.0),
            vertex(4, 10.0, 10.0),
            vertex(5, 5.0, 10.0),
            vertex(6, 0.0, 10.0),
        ];
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| {
                edge(
                    index as u64 + 1,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        let fold = edge(7, boundary[1], boundary[4], EdgeKind::Mountain);
        let fold_id = fold.id;
        edges.push(fold);
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixture_project_id(),
            source_revision: 7,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        FoldFixture {
            pattern,
            paper,
            topology: report.snapshot.expect("single-fold topology"),
            fold: fold_id,
        }
    }

    fn timeline(topology: &TopologySnapshot, fold: EdgeId, angles: &[f64]) -> InstructionTimeline {
        InstructionTimeline {
            steps: angles
                .iter()
                .enumerate()
                .map(|(index, angle)| InstructionStep {
                    id: InstructionStepId::new(),
                    title: format!("手順 {}", index + 1),
                    description: String::new(),
                    caution: String::new(),
                    duration_ms: 1_000,
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: FINGERPRINT.to_owned(),
                        fixed_face: Some(topology.faces[0].id),
                        hinge_angles: vec![InstructionHingeAngle {
                            edge: fold,
                            angle_degrees: *angle,
                        }],
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn projects_every_pose_with_one_global_deterministic_camera() {
        let fixture = single_fold_fixture();
        let timeline = timeline(&fixture.topology, fixture.fold, &[0.0, 90.0, 180.0]);
        let first = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("diagram");
        let second = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("diagram");
        assert_eq!(first, second);
        assert_eq!(first.steps.len(), 3);
        assert_eq!(first.steps[0].faces.len(), 2);
        assert_eq!(first.steps[0].hinges.len(), 1);
        assert_eq!(first.steps[0].changed_hinge_count, 0);
        assert_eq!(first.steps[1].changed_hinge_count, 1);
        assert_eq!(first.steps[2].changed_hinge_count, 1);
        assert_eq!(first.projected_vertex_visits, 30);
        assert!(first.bounds.max_x > first.bounds.min_x);
        assert!(first.bounds.max_y > first.bounds.min_y);
        assert!(first.steps.iter().flat_map(|step| &step.faces).all(|face| {
            face.points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
                && face.depth.is_finite()
        }));
        assert_ne!(first.steps[0].faces, first.steps[1].faces);
    }

    #[test]
    fn fixed_face_changes_are_supported_without_mutating_the_topology() {
        let fixture = single_fold_fixture();
        let mut timeline = timeline(&fixture.topology, fixture.fold, &[75.0, 75.0]);
        timeline.steps[1].pose.fixed_face = Some(fixture.topology.faces[1].id);
        let before = fixture.topology.clone();
        let plan = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("both anchors");
        assert_eq!(fixture.topology, before);
        assert_ne!(plan.steps[0].faces, plan.steps[1].faces);
        assert_eq!(plan.steps[1].changed_hinge_count, 0);
    }

    #[test]
    fn storage_order_and_source_edge_direction_do_not_change_the_plan() {
        let fixture = single_fold_fixture();
        let timeline = timeline(&fixture.topology, fixture.fold, &[42.5]);
        let expected = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("baseline");

        let mut reordered = fixture.pattern.clone();
        reordered.vertices.reverse();
        reordered.edges.reverse();
        for source in &mut reordered.edges {
            std::mem::swap(&mut source.start, &mut source.end);
        }
        let actual = build_instruction_diagram_plan(
            FINGERPRINT,
            &reordered,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("reordered");
        assert_eq!(actual, expected);
    }

    #[test]
    fn rejects_empty_stale_incomplete_and_over_limit_work() {
        let fixture = single_fold_fixture();
        let empty = InstructionTimeline::default();
        assert_eq!(
            build_instruction_diagram_plan(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &empty,
                &fixture.topology,
            ),
            Err(InstructionDiagramError::EmptyTimeline)
        );

        let stale = timeline(&fixture.topology, fixture.fold, &[10.0]);
        assert_eq!(
            build_instruction_diagram_plan(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                &fixture.pattern,
                &fixture.paper,
                &stale,
                &fixture.topology,
            ),
            Err(InstructionDiagramError::StaleStep { step_index: 0 })
        );

        let mut incomplete = stale.clone();
        incomplete.steps[0].pose.hinge_angles.clear();
        assert_eq!(
            build_instruction_diagram_plan(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &incomplete,
                &fixture.topology,
            ),
            Err(InstructionDiagramError::UnsupportedTopology)
        );

        let limits = InstructionDiagramLimits {
            max_projected_vertex_visits: 3,
            ..InstructionDiagramLimits::default()
        };
        assert_eq!(
            build_instruction_diagram_plan_with_limits(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &stale,
                &fixture.topology,
                limits,
            ),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );

        for limits in [
            InstructionDiagramLimits {
                max_source_vertices: fixture.pattern.vertices.len() - 1,
                ..InstructionDiagramLimits::default()
            },
            InstructionDiagramLimits {
                max_source_edges: fixture.pattern.edges.len() - 1,
                ..InstructionDiagramLimits::default()
            },
        ] {
            assert_eq!(
                build_instruction_diagram_plan_with_limits(
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &stale,
                    &fixture.topology,
                    limits,
                ),
                Err(InstructionDiagramError::ResourceLimitExceeded)
            );
        }
    }

    #[test]
    fn every_diagram_resource_limit_accepts_the_boundary_and_rejects_one_more() {
        let fixture = single_fold_fixture();
        let timeline = timeline(&fixture.topology, fixture.fold, &[37.25]);
        let exact = InstructionDiagramLimits {
            max_steps: 1,
            max_source_vertices: fixture.pattern.vertices.len(),
            max_source_edges: fixture.pattern.edges.len(),
            max_faces_per_step: fixture.topology.faces.len(),
            max_hinges_per_step: fixture.topology.hinge_adjacency.len(),
            max_projected_vertex_visits: 10,
        };
        let plan = build_instruction_diagram_plan_with_limits(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            exact,
        )
        .expect("all counts equal to their limits");
        assert_eq!(plan.projected_vertex_visits, 10);

        for one_too_many in [
            InstructionDiagramLimits {
                max_steps: exact.max_steps - 1,
                ..exact
            },
            InstructionDiagramLimits {
                max_source_vertices: exact.max_source_vertices - 1,
                ..exact
            },
            InstructionDiagramLimits {
                max_source_edges: exact.max_source_edges - 1,
                ..exact
            },
            InstructionDiagramLimits {
                max_faces_per_step: exact.max_faces_per_step - 1,
                ..exact
            },
            InstructionDiagramLimits {
                max_hinges_per_step: exact.max_hinges_per_step - 1,
                ..exact
            },
            InstructionDiagramLimits {
                max_projected_vertex_visits: exact.max_projected_vertex_visits - 1,
                ..exact
            },
        ] {
            assert_eq!(
                build_instruction_diagram_plan_with_limits(
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &timeline,
                    &fixture.topology,
                    one_too_many,
                ),
                Err(InstructionDiagramError::ResourceLimitExceeded)
            );
        }
    }

    #[test]
    fn default_projection_visit_limit_is_exact_and_checked_arithmetic_cannot_wrap() {
        let maximum = InstructionDiagramLimits::default().max_projected_vertex_visits;
        assert_eq!(maximum, 1_000_000);
        assert_eq!(
            checked_projected_vertex_visits(1_998, 1, 500, maximum),
            Ok(maximum)
        );
        assert_eq!(
            checked_projected_vertex_visits(1_998, 1, 501, maximum),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );
        assert_eq!(
            checked_projected_vertex_visits(usize::MAX, 1, 1, usize::MAX),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );
        assert_eq!(
            checked_projected_vertex_visits(0, usize::MAX, 1, usize::MAX),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );
        assert_eq!(
            checked_projected_vertex_visits(usize::MAX / 2 + 1, 0, 2, usize::MAX),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn rejects_the_first_middle_or_last_stale_step_without_partial_output() {
        let fixture = single_fold_fixture();
        let baseline = timeline(&fixture.topology, fixture.fold, &[10.0, 20.0, 30.0]);
        for stale_index in [0, 1, 2] {
            let mut stale = baseline.clone();
            stale.steps[stale_index].pose.source_model_fingerprint =
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
            assert_eq!(
                build_instruction_diagram_plan(
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &stale,
                    &fixture.topology,
                ),
                Err(InstructionDiagramError::StaleStep {
                    step_index: stale_index,
                })
            );
        }
    }

    #[test]
    fn default_model_count_limits_accept_the_boundary_and_reject_one_more() {
        let limits = InstructionDiagramLimits::default();
        assert_eq!(limits.max_source_vertices, 100_000);
        assert_eq!(limits.max_source_edges, 100_000);
        assert_eq!(limits.max_faces_per_step, 10_001);
        assert_eq!(
            limits.max_hinges_per_step,
            ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
        );
        checked_model_resource_counts(
            limits.max_source_vertices,
            limits.max_source_edges,
            limits.max_faces_per_step,
            limits.max_hinges_per_step,
            limits,
        )
        .expect("all model counts equal to their default limits");

        for counts in [
            (
                limits.max_source_vertices + 1,
                limits.max_source_edges,
                limits.max_faces_per_step,
                limits.max_hinges_per_step,
            ),
            (
                limits.max_source_vertices,
                limits.max_source_edges + 1,
                limits.max_faces_per_step,
                limits.max_hinges_per_step,
            ),
            (
                limits.max_source_vertices,
                limits.max_source_edges,
                limits.max_faces_per_step + 1,
                limits.max_hinges_per_step,
            ),
            (
                limits.max_source_vertices,
                limits.max_source_edges,
                limits.max_faces_per_step,
                limits.max_hinges_per_step + 1,
            ),
        ] {
            assert_eq!(
                checked_model_resource_counts(counts.0, counts.1, counts.2, counts.3, limits),
                Err(InstructionDiagramError::ResourceLimitExceeded)
            );
        }
    }

    #[test]
    fn camera_basis_is_orthonormal_to_serialization_precision() {
        let norm = |value: Vec3| dot(value, value);
        assert!((norm(CAMERA_TO_VIEWER) - 1.0).abs() < 1.0e-12);
        assert!((norm(CAMERA_RIGHT) - 1.0).abs() < 1.0e-12);
        assert!((norm(CAMERA_UP) - 1.0).abs() < 1.0e-12);
        assert!(dot(CAMERA_TO_VIEWER, CAMERA_RIGHT).abs() < 1.0e-12);
        assert!(dot(CAMERA_TO_VIEWER, CAMERA_UP).abs() < 1.0e-12);
        assert!(dot(CAMERA_RIGHT, CAMERA_UP).abs() < 1.0e-12);
    }

    #[test]
    fn deterministic_trigonometry_is_accurate_and_tiny_angle_changes_remain_visible() {
        for angle in [
            -180.0,
            -135.25,
            -90.0,
            -37.125,
            0.0,
            0.000_000_000_5,
            45.0,
            89.75,
            180.0,
        ] {
            let (actual_sine, actual_cosine) =
                deterministic_sin_cos_degrees(angle).expect("bounded angle");
            let (expected_sine, expected_cosine) = angle.to_radians().sin_cos();
            assert!((actual_sine - expected_sine).abs() < 5.0e-9);
            assert!((actual_cosine - expected_cosine).abs() < 5.0e-9);
        }
        assert!(deterministic_sin_cos_degrees(f64::NAN).is_err());
        assert!(deterministic_sin_cos_degrees(180.000_000_001).is_err());

        let fixture = single_fold_fixture();
        let timeline = timeline(&fixture.topology, fixture.fold, &[0.0, 0.000_000_000_5]);
        let plan = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("tiny angle change");
        assert_eq!(plan.steps[0].changed_hinge_count, 0);
        assert_eq!(plan.steps[1].changed_hinge_count, 1);
        assert!(plan.steps[1].hinges[0].changed);
    }

    #[test]
    fn projected_plan_has_a_cross_platform_golden_digest() {
        let fixture = single_fold_fixture();
        let timeline = timeline(&fixture.topology, fixture.fold, &[0.0, 37.25, 90.0, 180.0]);
        let plan = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("golden diagram");
        let mut canonical = String::new();
        write!(
            canonical,
            "orthographic_isometric_v1|visits:{}|",
            plan.projected_vertex_visits
        )
        .expect("write profile and visit count");
        for value in [
            plan.bounds.min_x,
            plan.bounds.min_y,
            plan.bounds.max_x,
            plan.bounds.max_y,
        ] {
            write!(canonical, "{:016x}", value.to_bits()).expect("write bounds");
        }
        for step in &plan.steps {
            write!(
                canonical,
                "|{}:{}:{}",
                step.faces.len(),
                step.hinges.len(),
                step.changed_hinge_count
            )
            .expect("write step");
            for face in &step.faces {
                write!(
                    canonical,
                    ";f{:02x}{:02x}{:02x}{:016x}",
                    face.fill.red,
                    face.fill.green,
                    face.fill.blue,
                    face.depth.to_bits()
                )
                .expect("write face");
                for point in &face.points {
                    write!(
                        canonical,
                        "{:016x}{:016x}",
                        point.x.to_bits(),
                        point.y.to_bits()
                    )
                    .expect("write face point");
                }
            }
            for hinge in &step.hinges {
                let kind = match hinge.kind {
                    InstructionDiagramFoldKind::Mountain => 'm',
                    InstructionDiagramFoldKind::Valley => 'v',
                };
                write!(
                    canonical,
                    ";h{kind}{}{:016x}{:016x}{:016x}{:016x}",
                    u8::from(hinge.changed),
                    hinge.start.x.to_bits(),
                    hinge.start.y.to_bits(),
                    hinge.end.x.to_bits(),
                    hinge.end.y.to_bits(),
                )
                .expect("write hinge");
            }
        }
        let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        assert_eq!(
            digest,
            "e8c428a07effe90280e78bc1c1920643197bcfb855913b07a682ce24e18f8a76"
        );
    }
}
