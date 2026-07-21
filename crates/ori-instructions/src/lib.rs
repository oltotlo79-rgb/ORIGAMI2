//! Deterministic drawing plans derived from authored folding instructions.
//!
//! This crate owns the CPU-side pose and fixed-camera projection used by
//! portable instruction exports. It deliberately does not inspect the
//! interactive Three.js scene, the current viewport, or GPU pixels.

mod fold_technique_file;
mod technique_motion;

pub use fold_technique_file::{
    FOLD_TECHNIQUE_FILE_SCHEMA_V1, FOLD_TECHNIQUE_FILE_VERSION_V1, FoldTechniqueActionV1,
    FoldTechniqueCapabilityV1, FoldTechniqueChoiceOptionV1, FoldTechniqueComparisonV1,
    FoldTechniqueExecutionSupportV1, FoldTechniqueFileDocumentV1, FoldTechniqueFileError,
    FoldTechniqueFileV1, FoldTechniqueLocalizedTextV1, FoldTechniqueMetadataV1,
    FoldTechniqueOperationV1, FoldTechniqueParameterBindingV1, FoldTechniqueParameterDefinitionV1,
    FoldTechniqueParameterLiteralV1, FoldTechniqueParameterTypeV1,
    FoldTechniquePreconditionDefinitionV1, FoldTechniquePreconditionV1, FoldTechniqueSinkKindV1,
    FoldTechniqueSourceV1, FoldTechniqueTemplateV1, FoldTechniqueUnsupportedPhysicalOperationV1,
    MAX_FOLD_TECHNIQUE_FILE_BYTES, read_fold_technique_file_v1, validate_fold_technique_file_v1,
    write_fold_technique_file_v1,
};
pub use technique_motion::{
    AccordionFoldMotionError, AccordionFoldMotionRequestV1, BookFoldMotionError,
    BookFoldMotionRequestV1, LayerSelectiveMotionRequestV1, PhysicalTechniqueCompilerV1,
    ReverseFoldKindV1, ReverseFoldMotionError, ReverseFoldMotionRequestV1, SinkFoldMotionRequestV1,
    append_certified_dyadic_path_timeline_v1, compile_certified_accordion_fold_timeline_v1,
    compile_certified_book_fold_timeline_v1, compile_certified_layer_selective_timeline_v1,
    compile_certified_reverse_fold_timeline_v1, compile_certified_sink_fold_timeline_v1,
    instruction_pose_fingerprint_v1, path_certificate_reference_from_native_v1,
    physical_technique_compiler_v1,
};

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use ori_domain::{
    CreasePattern, EdgeId, FaceId, InstructionPoseModel, InstructionTimeline, Paper, RgbaColor,
    VertexId, validate_instruction_timeline,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, KinematicsError, ObservationTreeKinematicsModel, Point3,
    TreeKinematicsLimits, VertexPosition3,
};
use ori_topology::{FoldAssignment, TopologySnapshot};
use thiserror::Error;

const PREVIEW_WORLD_SIZE: f64 = 4.4;
const PROJECTION_QUANTIZATION: f64 = 1_000_000_000.0;

// Fixed orthographic camera looking from the same general quadrant as the
// interactive preview's default camera. Precomputed unit vectors avoid
// platform-dependent normalization in the serialized drawing boundary.
const CAMERA_TO_VIEWER: [f64; 3] = [
    0.577_350_269_189_625_8,
    0.577_350_269_189_625_8,
    0.577_350_269_189_625_8,
];
const CAMERA_RIGHT: [f64; 3] = [
    std::f64::consts::FRAC_1_SQRT_2,
    0.0,
    -std::f64::consts::FRAC_1_SQRT_2,
];
const CAMERA_UP: [f64; 3] = [
    -0.408_248_290_463_863_1,
    0.816_496_580_927_726_1,
    -0.408_248_290_463_863_1,
];

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
    /// Whether this page is an intentionally non-executable explanation.
    pub declarative_only: bool,
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

struct PreparedModel {
    faces: Vec<PreparedFace>,
    kinematics: ObservationTreeKinematicsModel,
}

struct PreparedFace {
    id: FaceId,
    points: Vec<Point3>,
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
        if step.pose.model == InstructionPoseModel::AbsoluteHingeAnglesV1
            && step.pose.source_model_fingerprint != current_fold_model_fingerprint
        {
            return Err(InstructionDiagramError::StaleStep { step_index });
        }
    }

    let model = prepare_model(pattern, paper, topology, limits)?;
    let face_visits_per_step = model
        .faces
        .iter()
        .try_fold(0_usize, |total, face| total.checked_add(face.points.len()))
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    let physical_step_count = timeline
        .steps
        .iter()
        .filter(|step| step.pose.model == InstructionPoseModel::AbsoluteHingeAnglesV1)
        .count();
    let total_visits = checked_projected_vertex_visits(
        face_visits_per_step,
        model.kinematics.hinges().len(),
        physical_step_count,
        limits.max_projected_vertex_visits,
    )?;

    let mut bounds: Option<DiagramBounds> = None;
    let mut previous_angles = HashMap::<EdgeId, f64>::new();
    let mut steps = Vec::with_capacity(timeline.steps.len());
    for step in &timeline.steps {
        if step.pose.model == InstructionPoseModel::DeclarativeOnlyV1 {
            steps.push(InstructionDiagramStep {
                faces: Vec::new(),
                hinges: Vec::new(),
                changed_hinge_count: 0,
                declarative_only: true,
            });
            continue;
        }
        let angles = CanonicalHingeAngles::new(
            step.pose
                .hinge_angles
                .iter()
                .map(|angle| HingeAngle::new(angle.edge, angle.angle_degrees))
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_kinematics_error)?,
        )
        .map_err(map_kinematics_error)?;
        let current_angles = angles
            .as_slice()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<HashMap<_, _>>();
        let pose = model
            .kinematics
            .solve(step.pose.fixed_face, &angles)
            .map_err(map_kinematics_error)?;
        let mut rendered_faces = Vec::with_capacity(model.faces.len());
        for face in &model.faces {
            let transform = pose
                .face_transform(face.id)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            let mut depth_total = 0.0;
            let mut points = Vec::with_capacity(face.points.len());
            for point in &face.points {
                let transformed = transform
                    .apply_point(*point)
                    .map_err(map_kinematics_error)?;
                let (projected, depth) = project(transformed)?;
                bounds = Some(expand_bounds(bounds, projected));
                depth_total += depth;
                points.push(projected);
            }
            let normal = transform
                .apply_vector(Point3::new(0.0, 1.0, 0.0).map_err(map_kinematics_error)?)
                .map_err(map_kinematics_error)?;
            let front_visible = dot_point(normal, CAMERA_TO_VIEWER) >= 0.0;
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

        let mut rendered_hinges = Vec::with_capacity(model.kinematics.hinges().len());
        let mut changed_hinge_count = 0;
        for hinge in model.kinematics.hinges() {
            let parent_transform = pose
                .hinge_parent_transform(hinge.edge())
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            let (start, _) = project(
                parent_transform
                    .apply_point(hinge.start())
                    .map_err(map_kinematics_error)?,
            )?;
            let (end, _) = project(
                parent_transform
                    .apply_point(hinge.end())
                    .map_err(map_kinematics_error)?,
            )?;
            bounds = Some(expand_bounds(bounds, start));
            bounds = Some(expand_bounds(bounds, end));
            let current = current_angles
                .get(&hinge.edge())
                .copied()
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            let previous = previous_angles.get(&hinge.edge()).copied().unwrap_or(0.0);
            let changed = canonical_zero(current) != canonical_zero(previous);
            changed_hinge_count += usize::from(changed);
            rendered_hinges.push(InstructionDiagramHinge {
                start,
                end,
                kind: match hinge.assignment() {
                    FoldAssignment::Mountain => InstructionDiagramFoldKind::Mountain,
                    FoldAssignment::Valley => InstructionDiagramFoldKind::Valley,
                },
                changed,
            });
        }
        rendered_hinges.sort_by(compare_hinges);
        previous_angles = current_angles;
        steps.push(InstructionDiagramStep {
            faces: rendered_faces,
            hinges: rendered_hinges,
            changed_hinge_count,
            declarative_only: false,
        });
    }

    if bounds.is_none() {
        for face in &model.faces {
            for point in &face.points {
                let (projected, _) = project(*point)?;
                bounds = Some(expand_bounds(bounds, projected));
            }
        }
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
    physical_step_count: usize,
    maximum: usize,
) -> Result<usize, InstructionDiagramError> {
    if physical_step_count == 0 {
        return if face_vertices_per_step > maximum {
            Err(InstructionDiagramError::ResourceLimitExceeded)
        } else {
            Ok(face_vertices_per_step)
        };
    }
    let hinge_endpoints_per_step = hinge_count
        .checked_mul(2)
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    let visits_per_step = face_vertices_per_step
        .checked_add(hinge_endpoints_per_step)
        .ok_or(InstructionDiagramError::ResourceLimitExceeded)?;
    let total = visits_per_step
        .checked_mul(physical_step_count)
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
    let frame = projection_frame(paper, &positions)?;
    let mut embedded_positions = Vec::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        let position = positions
            .get(&vertex.id)
            .copied()
            .ok_or(InstructionDiagramError::UnsupportedTopology)?;
        embedded_positions.push(VertexPosition3::new(vertex.id, frame.to_world(position)?));
    }
    let kinematics = ObservationTreeKinematicsModel::prepare_with_positions(
        pattern,
        paper,
        topology,
        &embedded_positions,
        TreeKinematicsLimits {
            max_source_vertices: limits.max_source_vertices,
            max_source_edges: limits.max_source_edges,
            max_paper_boundary_vertices: limits.max_source_vertices,
            max_faces: limits.max_faces_per_step,
            max_edge_incidences: limits.max_source_edges,
            max_hinges: limits.max_hinges_per_step,
            max_face_boundary_vertices: limits.max_projected_vertex_visits,
            max_adjacency_entries: limits.max_hinges_per_step.saturating_mul(2),
        },
    )
    .map_err(map_kinematics_error)?;
    let mut faces = Vec::with_capacity(topology.faces.len());
    for face in &topology.faces {
        let mut points = Vec::with_capacity(face.outer.half_edges.len());
        for half_edge in &face.outer.half_edges {
            let position = kinematics
                .vertex_position(half_edge.origin)
                .ok_or(InstructionDiagramError::UnsupportedTopology)?;
            points.push(position);
        }
        faces.push(PreparedFace {
            id: face.id,
            points,
        });
    }
    Ok(PreparedModel { faces, kinematics })
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

struct ProjectionFrame {
    min_x: f64,
    min_y: f64,
    largest: f64,
    normalized_width: f64,
    normalized_height: f64,
}

impl ProjectionFrame {
    fn to_world(&self, point: ori_domain::Point2) -> Result<Point3, InstructionDiagramError> {
        let x = ((point.x - self.min_x) / self.largest - self.normalized_width / 2.0)
            * PREVIEW_WORLD_SIZE;
        let z = -((point.y - self.min_y) / self.largest - self.normalized_height / 2.0)
            * PREVIEW_WORLD_SIZE;
        Point3::new(x, 0.0, z).map_err(map_kinematics_error)
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

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn project(point: Point3) -> Result<(DiagramPoint, f64), InstructionDiagramError> {
    Ok((
        DiagramPoint {
            x: quantize(dot_point(point, CAMERA_RIGHT))?,
            y: quantize(dot_point(point, CAMERA_UP))?,
        },
        quantize(dot_point(point, CAMERA_TO_VIEWER))?,
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

fn dot_point(point: Point3, vector: [f64; 3]) -> f64 {
    point.x() * vector[0] + point.y() * vector[1] + point.z() * vector[2]
}

#[cfg(test)]
fn dot_components(first: [f64; 3], second: [f64; 3]) -> f64 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

const fn map_kinematics_error(error: KinematicsError) -> InstructionDiagramError {
    match error {
        KinematicsError::ResourceLimitExceeded => InstructionDiagramError::ResourceLimitExceeded,
        KinematicsError::UnrepresentableGeometry
        | KinematicsError::NonFiniteHingeAngle { .. }
        | KinematicsError::HingeAngleOutOfRange { .. } => {
            InstructionDiagramError::UnrepresentableGeometry
        }
        KinematicsError::UnsupportedTopology
        | KinematicsError::DuplicateHingeAngle { .. }
        | KinematicsError::NonCanonicalHingeAngles { .. }
        | KinematicsError::MissingHingeAngle { .. }
        | KinematicsError::ExtraHingeAngle { .. }
        | KinematicsError::UnknownHingeAngle { .. }
        | KinematicsError::MissingFixedFace
        | KinematicsError::UnknownFixedFace { .. }
        | KinematicsError::UnexpectedFixedFace { .. }
        | KinematicsError::MaterialPoseIssuerMismatch => {
            InstructionDiagramError::UnsupportedTopology
        }
    }
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
    use ori_kinematics::deterministic_sin_cos_degrees;
    use ori_topology::{EdgeIncidence, FaceExtractionInput, analyze_faces};
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
                    visual: Default::default(),
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

    fn declarative_step(title: &str) -> InstructionStep {
        InstructionStep {
            id: InstructionStepId::new(),
            title: title.to_owned(),
            description: "説明テンプレート".to_owned(),
            caution: "物理操作なし".to_owned(),
            duration_ms: 1_000,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: "f".repeat(64),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
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
    fn declarative_step_exports_without_pose_solving_or_stale_rejection() {
        let fixture = single_fold_fixture();
        let timeline = InstructionTimeline {
            steps: vec![declarative_step("中割り折り（説明）")],
        };

        let plan = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("declarative diagram");
        assert_eq!(plan.steps.len(), 1);
        assert!(plan.steps[0].declarative_only);
        assert!(plan.steps[0].faces.is_empty());
        assert!(plan.steps[0].hinges.is_empty());
        assert_eq!(plan.steps[0].changed_hinge_count, 0);
        assert_eq!(
            plan.projected_vertex_visits,
            fixture
                .topology
                .faces
                .iter()
                .map(|face| face.outer.half_edges.len())
                .sum::<usize>()
        );
        assert!(plan.bounds.max_x > plan.bounds.min_x);
        assert!(plan.bounds.max_y > plan.bounds.min_y);
    }

    #[test]
    fn declarative_steps_do_not_consume_physical_projection_work_budget() {
        let fixture = single_fold_fixture();
        let physical = timeline(&fixture.topology, fixture.fold, &[0.0, 90.0]);
        let physical_plan = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &physical,
            &fixture.topology,
        )
        .expect("physical baseline");
        assert_eq!(physical_plan.projected_vertex_visits, 20);

        let mut mixed = physical.clone();
        mixed.steps.insert(0, declarative_step("導入説明"));
        mixed.steps.insert(2, declarative_step("途中説明"));
        mixed.steps.push(declarative_step("補足説明"));

        let exact = InstructionDiagramLimits {
            max_projected_vertex_visits: 20,
            ..InstructionDiagramLimits::default()
        };
        let mixed_plan = build_instruction_diagram_plan_with_limits(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &mixed,
            &fixture.topology,
            exact,
        )
        .expect("declarative steps add no physical projection work");
        assert_eq!(mixed_plan.projected_vertex_visits, 20);
        assert_eq!(
            mixed_plan
                .steps
                .iter()
                .filter(|step| !step.declarative_only)
                .collect::<Vec<_>>(),
            physical_plan.steps.iter().collect::<Vec<_>>()
        );

        assert_eq!(
            build_instruction_diagram_plan_with_limits(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &mixed,
                &fixture.topology,
                InstructionDiagramLimits {
                    max_projected_vertex_visits: 19,
                    ..exact
                },
            ),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );
        assert_eq!(
            build_instruction_diagram_plan_with_limits(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &mixed,
                &fixture.topology,
                InstructionDiagramLimits {
                    max_projected_vertex_visits: 21,
                    ..exact
                },
            )
            .expect("one spare projected visit")
            .projected_vertex_visits,
            20
        );
    }

    #[test]
    fn all_declarative_timeline_only_counts_one_rest_bounds_projection_at_step_limit() {
        let fixture = single_fold_fixture();
        let timeline = InstructionTimeline {
            steps: (0..ori_domain::MAX_INSTRUCTION_STEPS)
                .map(|index| declarative_step(&format!("説明 {}", index + 1)))
                .collect(),
        };
        let face_boundary_vertices = fixture
            .topology
            .faces
            .iter()
            .map(|face| face.outer.half_edges.len())
            .sum();
        let exact = InstructionDiagramLimits {
            max_projected_vertex_visits: face_boundary_vertices,
            ..InstructionDiagramLimits::default()
        };
        let plan = build_instruction_diagram_plan_with_limits(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            exact,
        )
        .expect("description-only timeline does not spend physical-pose budget");
        assert_eq!(plan.steps.len(), ori_domain::MAX_INSTRUCTION_STEPS);
        assert!(plan.steps.iter().all(|step| step.declarative_only));
        assert_eq!(plan.projected_vertex_visits, face_boundary_vertices);
        assert_eq!(
            build_instruction_diagram_plan_with_limits(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
                InstructionDiagramLimits {
                    max_projected_vertex_visits: face_boundary_vertices - 1,
                    ..exact
                },
            ),
            Err(InstructionDiagramError::ResourceLimitExceeded)
        );
        assert_eq!(
            build_instruction_diagram_plan_with_limits(
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
                InstructionDiagramLimits {
                    max_projected_vertex_visits: face_boundary_vertices + 1,
                    ..exact
                },
            )
            .expect("one spare rest-bounds projection")
            .projected_vertex_visits,
            face_boundary_vertices
        );
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
    fn isolated_and_auxiliary_draft_geometry_do_not_change_the_plan() {
        let fixture = single_fold_fixture();
        let timeline = timeline(&fixture.topology, fixture.fold, &[0.0, 90.0, 180.0]);
        let expected = build_instruction_diagram_plan(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("baseline");

        let auxiliary = edge(
            99,
            fixture_vertex_id(998),
            fixture_vertex_id(999),
            EdgeKind::Auxiliary,
        );
        let mut pattern = fixture.pattern.clone();
        pattern.vertices.push(vertex(99, 30.0, 30.0));
        pattern.edges.push(auxiliary.clone());
        let mut topology = fixture.topology.clone();
        topology
            .edge_incidence
            .push((auxiliary.id, EdgeIncidence::AuxiliaryIgnored));
        topology
            .edge_incidence
            .sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());

        let actual = build_instruction_diagram_plan(
            FINGERPRINT,
            &pattern,
            &fixture.paper,
            &timeline,
            &topology,
        )
        .expect("draft-only geometry ignored");
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
        let norm = |value| dot_components(value, value);
        assert!((norm(CAMERA_TO_VIEWER) - 1.0).abs() < 1.0e-12);
        assert!((norm(CAMERA_RIGHT) - 1.0).abs() < 1.0e-12);
        assert!((norm(CAMERA_UP) - 1.0).abs() < 1.0e-12);
        assert!(dot_components(CAMERA_TO_VIEWER, CAMERA_RIGHT).abs() < 1.0e-12);
        assert!(dot_components(CAMERA_TO_VIEWER, CAMERA_UP).abs() < 1.0e-12);
        assert!(dot_components(CAMERA_RIGHT, CAMERA_UP).abs() < 1.0e-12);
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
