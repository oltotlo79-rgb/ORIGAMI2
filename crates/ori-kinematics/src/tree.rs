use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, VertexId};
use ori_geometry::{SegmentIntersection, polygon_signed_double_area, segment_intersection};
use ori_topology::{
    CanonicalFaceKeyError, EdgeIncidence, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot,
    canonical_face_key,
};

use crate::{
    KinematicsError,
    transform::{Point3, RigidTransform, canonical_zero, length, scale, subtract},
};

pub const MATERIAL_TREE_KINEMATICS_MODEL_ID: &str = "material_tree_kinematics_mm_v1";
pub const CALLER_EMBEDDING_OBSERVATION_MODEL_ID: &str =
    "caller_embedding_tree_kinematics_observation_v1";
const MAX_SIMPLE_BOUNDARY_EXACT_INTERSECTION_TESTS: usize = 10_000_000;

/// Hard work bounds checked before model allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeKinematicsLimits {
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_paper_boundary_vertices: usize,
    pub max_faces: usize,
    pub max_edge_incidences: usize,
    pub max_hinges: usize,
    pub max_face_boundary_vertices: usize,
    pub max_adjacency_entries: usize,
}

impl Default for TreeKinematicsLimits {
    fn default() -> Self {
        Self {
            max_source_vertices: 100_000,
            max_source_edges: 100_000,
            max_paper_boundary_vertices: 100_000,
            max_faces: 10_001,
            max_edge_incidences: 100_000,
            max_hinges: ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP,
            max_face_boundary_vertices: 1_000_000,
            max_adjacency_entries: ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2,
        }
    }
}

/// One finite source-vertex position in a caller-selected observation frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexPosition3 {
    vertex: VertexId,
    position: Point3,
}

impl VertexPosition3 {
    #[must_use]
    pub const fn new(vertex: VertexId, position: Point3) -> Self {
        Self { vertex, position }
    }

    #[must_use]
    pub const fn vertex(self) -> VertexId {
        self.vertex
    }

    #[must_use]
    pub const fn position(self) -> Point3 {
        self.position
    }
}

/// One finite, normalized fold angle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HingeAngle {
    edge: EdgeId,
    angle_degrees: f64,
}

impl HingeAngle {
    pub fn new(edge: EdgeId, angle_degrees: f64) -> Result<Self, KinematicsError> {
        if !angle_degrees.is_finite() {
            return Err(KinematicsError::NonFiniteHingeAngle { edge });
        }
        if !(0.0..=180.0).contains(&angle_degrees) {
            return Err(KinematicsError::HingeAngleOutOfRange { edge });
        }
        Ok(Self {
            edge,
            angle_degrees: canonical_zero(angle_degrees),
        })
    }

    #[must_use]
    pub const fn edge(self) -> EdgeId {
        self.edge
    }

    #[must_use]
    pub const fn angle_degrees(self) -> f64 {
        self.angle_degrees
    }
}

/// Converts the public unsigned 0..=180 degree boundary into the signed
/// internal rotation convention selected by the live hinge assignment.
/// The edge identity is checked here so stale/foreign angle records cannot be
/// normalized against another hinge.
pub fn assignment_signed_angle_degrees_v1(
    expected_edge: EdgeId,
    assignment: FoldAssignment,
    angle: HingeAngle,
) -> Result<f64, KinematicsError> {
    if angle.edge() != expected_edge {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let magnitude = angle.angle_degrees();
    Ok(match assignment {
        FoldAssignment::Mountain => magnitude,
        FoldAssignment::Valley if magnitude == 0.0 => 0.0,
        FoldAssignment::Valley => -magnitude,
    })
}

/// A complete angle vector in canonical `EdgeId` byte order.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalHingeAngles {
    angles: Vec<HingeAngle>,
}

impl CanonicalHingeAngles {
    pub fn new(angles: Vec<HingeAngle>) -> Result<Self, KinematicsError> {
        for pair in angles.windows(2) {
            match pair[0]
                .edge
                .canonical_bytes()
                .cmp(&pair[1].edge.canonical_bytes())
            {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(KinematicsError::DuplicateHingeAngle { edge: pair[1].edge });
                }
                std::cmp::Ordering::Greater => {
                    return Err(KinematicsError::NonCanonicalHingeAngles {
                        previous_edge: pair[0].edge,
                        edge: pair[1].edge,
                    });
                }
            }
        }
        Ok(Self { angles })
    }

    #[must_use]
    pub fn as_slice(&self) -> &[HingeAngle] {
        &self.angles
    }
}

/// One canonical material hinge. Geometry and fields are read-only.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeHinge {
    edge: EdgeId,
    assignment: FoldAssignment,
    left_face: FaceId,
    right_face: FaceId,
    start: Point3,
    end: Point3,
    axis: Point3,
}

impl TreeHinge {
    #[cfg(test)]
    pub(crate) const fn new_for_test(
        edge: EdgeId,
        assignment: FoldAssignment,
        left_face: FaceId,
        right_face: FaceId,
        start: Point3,
        end: Point3,
        axis: Point3,
    ) -> Self {
        Self {
            edge,
            assignment,
            left_face,
            right_face,
            start,
            end,
            axis,
        }
    }

    #[must_use]
    pub const fn edge(&self) -> EdgeId {
        self.edge
    }

    #[must_use]
    pub const fn assignment(&self) -> FoldAssignment {
        self.assignment
    }

    #[must_use]
    pub const fn left_face(&self) -> FaceId {
        self.left_face
    }

    #[must_use]
    pub const fn right_face(&self) -> FaceId {
        self.right_face
    }

    #[must_use]
    pub const fn start(&self) -> Point3 {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> Point3 {
        self.end
    }

    #[must_use]
    pub const fn axis(&self) -> Point3 {
        self.axis
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Neighbor {
    face: FaceId,
    hinge_index: usize,
    rotation_sign: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedFaceBoundary {
    face: FaceId,
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, Copy)]
struct SimpleBoundaryValidationBudget {
    remaining_exact_intersection_tests: usize,
}

impl SimpleBoundaryValidationBudget {
    const fn production() -> Self {
        Self {
            remaining_exact_intersection_tests: MAX_SIMPLE_BOUNDARY_EXACT_INTERSECTION_TESTS,
        }
    }

    fn charge_exact_intersection_test(&mut self) -> Result<(), KinematicsError> {
        self.remaining_exact_intersection_tests = self
            .remaining_exact_intersection_tests
            .checked_sub(1)
            .ok_or(KinematicsError::ResourceLimitExceeded)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedTree {
    face_ids: Vec<FaceId>,
    face_boundaries: Vec<PreparedFaceBoundary>,
    positions: HashMap<VertexId, Point3>,
    hinges: Vec<TreeHinge>,
    adjacency: HashMap<FaceId, Vec<Neighbor>>,
}

/// Native material-mm kinematics. This is the only model type eligible for a
/// later native applied-pose certificate.
#[derive(Debug, Clone)]
pub struct MaterialTreeKinematicsModel {
    tree: Arc<PreparedTree>,
}

/// Validated material face and hinge geometry for connected graphs, including
/// cycles. This observation-only model does not solve poses.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialHingeGraphGeometry {
    issuer: Arc<()>,
    face_ids: Vec<FaceId>,
    hinges: Vec<TreeHinge>,
    positions: HashMap<VertexId, Point3>,
    face_boundaries: Vec<PreparedFaceBoundary>,
}

impl MaterialHingeGraphGeometry {
    #[cfg(test)]
    pub(crate) fn new_for_test(face_ids: Vec<FaceId>, hinges: Vec<TreeHinge>) -> Self {
        Self {
            issuer: Arc::new(()),
            face_ids,
            hinges,
            positions: HashMap::new(),
            face_boundaries: Vec::new(),
        }
    }
    /// Prepares canonical material-mm hinge axes without imposing the tree
    /// cardinality required by [`MaterialTreeKinematicsModel`].
    pub fn prepare(
        pattern: &CreasePattern,
        paper: &Paper,
        topology: &TopologySnapshot,
        limits: TreeKinematicsLimits,
    ) -> Result<Self, KinematicsError> {
        let positions = pattern
            .vertices
            .iter()
            .filter(|vertex| vertex.position.x.is_finite() && vertex.position.y.is_finite())
            .map(|vertex| {
                Ok(VertexPosition3::new(
                    vertex.id,
                    Point3::new(vertex.position.x, 0.0, -vertex.position.y)?,
                ))
            })
            .collect::<Result<Vec<_>, KinematicsError>>()?;
        let prepared = prepare_material_graph(pattern, paper, topology, &positions, limits)?;
        Ok(Self {
            issuer: Arc::new(()),
            face_ids: prepared.face_ids,
            hinges: prepared.hinges,
            positions: prepared.positions,
            face_boundaries: prepared.face_boundaries,
        })
    }

    #[must_use]
    pub fn face_ids(&self) -> &[FaceId] {
        &self.face_ids
    }

    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
    }

    #[must_use]
    pub fn hinges(&self) -> &[TreeHinge] {
        &self.hinges
    }

    #[must_use]
    pub fn vertex_position(&self, vertex: VertexId) -> Option<Point3> {
        self.positions.get(&vertex).copied()
    }

    #[must_use]
    pub fn face_boundary_vertices(&self, face: FaceId) -> Option<&[VertexId]> {
        self.face_boundaries
            .binary_search_by_key(&face.canonical_bytes(), |item| item.face.canonical_bytes())
            .ok()
            .map(|index| self.face_boundaries[index].vertices.as_slice())
    }
}

/// Caller-embedded kinematics for projection and other observation-only uses.
///
/// Its distinct type prevents a display embedding from being confused with a
/// native material-mm pose.
#[derive(Debug, Clone)]
pub struct ObservationTreeKinematicsModel {
    tree: Arc<PreparedTree>,
}

/// A material-mm pose produced only by [`MaterialTreeKinematicsModel`].
#[derive(Debug, Clone)]
pub struct MaterialTreePose {
    source: Arc<PreparedTree>,
    pose: Arc<TreePoseData>,
    fixed_face: Option<FaceId>,
    angles: Arc<CanonicalHingeAngles>,
}

/// One canonical counter-clockwise outer walk borrowed from an exact native
/// material-model source.
///
/// The private source and registry index make this a provenance-bearing view,
/// rather than a caller-constructible collection of matching identifiers.
/// Clone and copy preserve the same source identity.
#[derive(Debug, Clone, Copy)]
pub struct MaterialFaceBoundary<'a> {
    source: &'a PreparedTree,
    index: usize,
}

/// A material pose proven to have been issued by one exact model instance.
///
/// The fields are private so callers cannot forge this relationship from
/// matching face or edge identifiers. This binds the issuer only; callers
/// that need to reject same-angle ABA must additionally retain and compare the
/// exact pose instance with [`MaterialTreePose::same_instance`].
#[derive(Debug, Clone, Copy)]
pub struct BoundMaterialTreePose<'a> {
    model: &'a MaterialTreeKinematicsModel,
    pose: &'a MaterialTreePose,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialHingePairCanonicalInputV1 {
    pub hinge_index: usize,
    pub face_indexes: [usize; 2],
    pub excluded_face_indexes: Vec<usize>,
    pub edge: EdgeId,
    pub faces: [FaceId; 2],
    pub assignment: FoldAssignment,
    pub angle_degrees: f64,
    pub axis: [Point3; 2],
    pub world_axis: [Point3; 2],
    pub boundaries: [Vec<VertexId>; 2],
    pub boundary_edges: [Vec<EdgeId>; 2],
    pub rest_positions: [Vec<Point3>; 2],
    pub world_transforms: [RigidTransform; 2],
    pub exact_binary64_affine_bits: [[[u64; 4]; 3]; 2],
}

#[derive(Debug)]
pub struct MaterialHingePairProjectionV1<'a> {
    bound: BoundMaterialTreePose<'a>,
    input: MaterialHingePairCanonicalInputV1,
}

impl MaterialHingePairProjectionV1<'_> {
    #[must_use]
    pub fn observe(&self) -> MaterialHingePairCanonicalInputV1 {
        self.input.clone()
    }

    #[must_use]
    pub const fn authorizes_mutation(&self) -> bool {
        false
    }
}

/// An observation-frame pose produced only by
/// [`ObservationTreeKinematicsModel`].
#[derive(Debug, Clone)]
pub struct ObservationTreePose {
    source: Arc<PreparedTree>,
    pose: TreePoseData,
}

#[derive(Debug, Clone, PartialEq)]
struct TreePoseData {
    face_transforms: Vec<(FaceId, RigidTransform)>,
    hinge_parent_transforms: Vec<(EdgeId, RigidTransform)>,
}

macro_rules! model_observers {
    () => {
        #[must_use]
        pub fn face_ids(&self) -> &[FaceId] {
            &self.tree.face_ids
        }

        #[must_use]
        pub fn hinges(&self) -> &[TreeHinge] {
            &self.tree.hinges
        }

        #[must_use]
        pub fn vertex_position(&self, vertex: VertexId) -> Option<Point3> {
            self.tree.positions.get(&vertex).copied()
        }

        #[must_use]
        pub const fn identity_transform(&self) -> RigidTransform {
            RigidTransform::identity()
        }
    };
}

impl MaterialTreeKinematicsModel {
    /// Prepares canonical `(paper_x, 0, -paper_y)` material coordinates in mm.
    pub fn prepare(
        pattern: &CreasePattern,
        paper: &Paper,
        topology: &TopologySnapshot,
        limits: TreeKinematicsLimits,
    ) -> Result<Self, KinematicsError> {
        check_raw_resource_counts(pattern, paper, topology, pattern.vertices.len(), limits)?;
        validate_paper_scalar_fields(paper)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(pattern.vertices.len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        for vertex in &pattern.vertices {
            if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
                // Material topology deliberately ignores unfinished
                // auxiliary-only and isolated draft vertices. Every material
                // participant is rechecked below and cannot be omitted.
                continue;
            }
            positions.push(VertexPosition3::new(
                vertex.id,
                Point3::new(vertex.position.x, 0.0, -vertex.position.y)?,
            ));
        }
        Ok(Self {
            tree: Arc::new(prepare_tree(pattern, paper, topology, &positions, limits)?),
        })
    }

    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        MATERIAL_TREE_KINEMATICS_MODEL_ID
    }

    pub fn solve(
        &self,
        fixed_face: Option<FaceId>,
        angles: &CanonicalHingeAngles,
    ) -> Result<MaterialTreePose, KinematicsError> {
        let pose = solve_tree(&self.tree, fixed_face, angles)?;
        let angles = try_clone_canonical_angles(angles)?;
        Ok(MaterialTreePose {
            source: Arc::clone(&self.tree),
            pose: Arc::new(pose),
            fixed_face,
            angles: Arc::new(angles),
        })
    }

    /// Returns whether this model instance issued `pose`.
    ///
    /// Deeply equal models prepared in separate calls are intentionally
    /// different issuers. Cloning a model or one of its poses preserves the
    /// same private issuer identity.
    #[must_use]
    pub fn owns_pose(&self, pose: &MaterialTreePose) -> bool {
        Arc::ptr_eq(&self.tree, &pose.source)
    }

    /// Binds a pose to this exact issuer, or fails closed.
    pub fn bind_pose<'a>(
        &'a self,
        pose: &'a MaterialTreePose,
    ) -> Result<BoundMaterialTreePose<'a>, KinematicsError> {
        if !self.owns_pose(pose) {
            return Err(KinematicsError::MaterialPoseIssuerMismatch);
        }
        Ok(BoundMaterialTreePose { model: self, pose })
    }

    /// Returns one validated canonical CCW outer walk from this exact model.
    #[must_use]
    pub fn face_boundary(&self, face: FaceId) -> Option<MaterialFaceBoundary<'_>> {
        find_material_face_boundary(&self.tree, face)
    }

    /// Returns whether `boundary` was borrowed from this exact model source.
    ///
    /// Deeply equal models prepared independently are intentionally distinct.
    #[must_use]
    pub fn owns_face_boundary(&self, boundary: MaterialFaceBoundary<'_>) -> bool {
        owns_material_face_boundary(&self.tree, boundary)
    }

    model_observers!();
}

impl ObservationTreeKinematicsModel {
    /// Prepares a caller-selected finite coordinate embedding.
    ///
    /// This path exists to preserve legacy projection arithmetic. Its distinct
    /// model and pose types are observation-only.
    pub fn prepare_with_positions(
        pattern: &CreasePattern,
        paper: &Paper,
        topology: &TopologySnapshot,
        positions: &[VertexPosition3],
        limits: TreeKinematicsLimits,
    ) -> Result<Self, KinematicsError> {
        check_raw_resource_counts(pattern, paper, topology, positions.len(), limits)?;
        validate_paper_scalar_fields(paper)?;
        Ok(Self {
            tree: Arc::new(prepare_tree(pattern, paper, topology, positions, limits)?),
        })
    }

    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CALLER_EMBEDDING_OBSERVATION_MODEL_ID
    }

    pub fn solve(
        &self,
        fixed_face: Option<FaceId>,
        angles: &CanonicalHingeAngles,
    ) -> Result<ObservationTreePose, KinematicsError> {
        solve_tree(&self.tree, fixed_face, angles).map(|pose| ObservationTreePose {
            source: Arc::clone(&self.tree),
            pose,
        })
    }

    /// Returns whether this observation model instance issued `pose`.
    #[must_use]
    pub fn owns_pose(&self, pose: &ObservationTreePose) -> bool {
        Arc::ptr_eq(&self.tree, &pose.source)
    }

    model_observers!();
}

macro_rules! pose_observers {
    () => {
        #[must_use]
        pub fn face_transform(&self, face: FaceId) -> Option<RigidTransform> {
            find_face_transform(&self.pose, face)
        }

        #[must_use]
        pub fn hinge_parent_transform(&self, edge: EdgeId) -> Option<RigidTransform> {
            find_hinge_parent_transform(&self.pose, edge)
        }
    };
}

impl MaterialTreePose {
    pose_observers!();

    /// Returns the fixed material face captured by this pose.
    #[must_use]
    pub const fn fixed_face(&self) -> Option<FaceId> {
        self.fixed_face
    }

    /// Returns the complete canonical hinge-angle vector captured by this
    /// pose.
    #[must_use]
    pub fn hinge_angles(&self) -> &[HingeAngle] {
        self.angles.as_slice()
    }

    /// Returns whether two values are clones of the same issued pose
    /// instance. Solving the same angle vector again deliberately returns a
    /// different instance.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && Arc::ptr_eq(&self.pose, &other.pose)
    }

    /// Returns the canonical material-face registry belonging to this pose's
    /// private source model.
    #[must_use]
    pub fn face_ids(&self) -> &[FaceId] {
        &self.source.face_ids
    }

    /// Returns the material hinges belonging to this pose's private source
    /// model.
    #[must_use]
    pub fn hinges(&self) -> &[TreeHinge] {
        &self.source.hinges
    }

    /// Returns one source material position from this pose's own model.
    #[must_use]
    pub fn vertex_position(&self, vertex: VertexId) -> Option<Point3> {
        self.source.positions.get(&vertex).copied()
    }

    /// Returns one validated canonical CCW outer walk from this pose's private
    /// source model.
    #[must_use]
    pub fn face_boundary(&self, face: FaceId) -> Option<MaterialFaceBoundary<'_>> {
        find_material_face_boundary(&self.source, face)
    }

    /// Returns whether `boundary` belongs to this pose's exact private source.
    #[must_use]
    pub fn owns_face_boundary(&self, boundary: MaterialFaceBoundary<'_>) -> bool {
        owns_material_face_boundary(&self.source, boundary)
    }
}

impl<'a> MaterialFaceBoundary<'a> {
    /// Returns this boundary's material face identifier.
    #[must_use]
    pub fn face(self) -> FaceId {
        self.source.face_boundaries[self.index].face
    }

    /// Returns the canonical CCW vertex cycle. The first vertex is selected
    /// deterministically from the full `(EdgeId, origin, destination)` token.
    #[must_use]
    pub fn vertices(self) -> &'a [VertexId] {
        &self.source.face_boundaries[self.index].vertices
    }

    /// Returns the edge cycle aligned one-to-one with [`Self::vertices`].
    #[must_use]
    pub fn edges(self) -> &'a [EdgeId] {
        &self.source.face_boundaries[self.index].edges
    }
}

impl<'a> BoundMaterialTreePose<'a> {
    /// Returns the exact issuer that was checked by
    /// [`MaterialTreeKinematicsModel::bind_pose`].
    #[must_use]
    pub const fn model(&self) -> &'a MaterialTreeKinematicsModel {
        self.model
    }

    /// Returns the checked material pose.
    #[must_use]
    pub const fn pose(&self) -> &'a MaterialTreePose {
        self.pose
    }

    /// Returns a boundary from the exact model/pose issuer pair checked by
    /// [`MaterialTreeKinematicsModel::bind_pose`].
    #[must_use]
    pub fn face_boundary(&self, face: FaceId) -> Option<MaterialFaceBoundary<'a>> {
        find_material_face_boundary(&self.model.tree, face)
    }
}

pub fn prepare_material_hinge_pair_projection_v1(
    bound: BoundMaterialTreePose<'_>,
    edge: EdgeId,
) -> Result<MaterialHingePairProjectionV1<'_>, KinematicsError> {
    let hinge_index = bound
        .model
        .hinges()
        .iter()
        .position(|hinge| hinge.edge() == edge)
        .ok_or(KinematicsError::UnsupportedTopology)?;
    if bound
        .model
        .hinges()
        .iter()
        .skip(hinge_index + 1)
        .any(|hinge| hinge.edge() == edge)
    {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let hinge = &bound.model.hinges()[hinge_index];
    let faces = [hinge.left_face(), hinge.right_face()];
    let face_indexes = faces.map(|face| {
        bound
            .model
            .face_ids()
            .iter()
            .position(|candidate| *candidate == face)
            .ok_or(KinematicsError::UnsupportedTopology)
    });
    let face_indexes = [face_indexes[0].clone()?, face_indexes[1].clone()?];
    let boundaries = faces.map(|face| {
        bound
            .face_boundary(face)
            .map(|boundary| boundary.vertices().to_vec())
            .ok_or(KinematicsError::UnsupportedTopology)
    });
    let boundaries = [boundaries[0].clone()?, boundaries[1].clone()?];
    let boundary_edges = faces.map(|face| {
        bound
            .face_boundary(face)
            .map(|boundary| boundary.edges().to_vec())
            .ok_or(KinematicsError::UnsupportedTopology)
    });
    let boundary_edges = [boundary_edges[0].clone()?, boundary_edges[1].clone()?];
    let rest_positions = boundaries.clone().map(|vertices| {
        vertices
            .into_iter()
            .map(|vertex| {
                bound
                    .model
                    .vertex_position(vertex)
                    .ok_or(KinematicsError::UnrepresentableGeometry)
            })
            .collect::<Result<Vec<_>, _>>()
    });
    let angle_degrees = bound
        .pose
        .hinge_angles()
        .iter()
        .find(|angle| angle.edge() == edge)
        .ok_or(KinematicsError::UnsupportedTopology)?
        .angle_degrees();
    let parent_transform = bound
        .pose
        .hinge_parent_transform(edge)
        .ok_or(KinematicsError::UnsupportedTopology)?;
    let canonical_world = |point: Point3| {
        let point = parent_transform.apply_point(point)?;
        Point3::new(
            canonical_zero(point.x()),
            canonical_zero(point.y()),
            canonical_zero(point.z()),
        )
    };
    let world_axis = [
        canonical_world(hinge.start())?,
        canonical_world(hinge.end())?,
    ];
    let world_transforms = faces.map(|face| {
        bound
            .pose
            .face_transform(face)
            .ok_or(KinematicsError::UnsupportedTopology)
    });
    let input = MaterialHingePairCanonicalInputV1 {
        hinge_index,
        face_indexes,
        excluded_face_indexes: (0..bound.model.face_ids().len())
            .filter(|index| !face_indexes.contains(index))
            .collect(),
        edge,
        faces,
        assignment: hinge.assignment(),
        angle_degrees,
        axis: [hinge.start(), hinge.end()],
        world_axis,
        boundaries,
        boundary_edges,
        rest_positions: [rest_positions[0].clone()?, rest_positions[1].clone()?],
        world_transforms: [world_transforms[0].clone()?, world_transforms[1].clone()?],
        exact_binary64_affine_bits: [
            affine_bits(world_transforms[0].clone()?),
            affine_bits(world_transforms[1].clone()?),
        ],
    };
    Ok(MaterialHingePairProjectionV1 { bound, input })
}

fn affine_bits(transform: RigidTransform) -> [[u64; 4]; 3] {
    let rotation = transform.rotation_rows();
    let translation = transform.translation();
    [
        [
            rotation[0][0].to_bits(),
            rotation[0][1].to_bits(),
            rotation[0][2].to_bits(),
            translation.x().to_bits(),
        ],
        [
            rotation[1][0].to_bits(),
            rotation[1][1].to_bits(),
            rotation[1][2].to_bits(),
            translation.y().to_bits(),
        ],
        [
            rotation[2][0].to_bits(),
            rotation[2][1].to_bits(),
            rotation[2][2].to_bits(),
            translation.z().to_bits(),
        ],
    ]
}

pub fn revalidate_material_hinge_pair_projection_v1(
    capability: &MaterialHingePairProjectionV1<'_>,
    bound: BoundMaterialTreePose<'_>,
) -> Option<MaterialHingePairCanonicalInputV1> {
    if !std::ptr::eq(capability.bound.model(), bound.model())
        || !std::ptr::eq(capability.bound.pose(), bound.pose())
    {
        return None;
    }
    let rebuilt = prepare_material_hinge_pair_projection_v1(bound, capability.input.edge).ok()?;
    (rebuilt.input == capability.input).then(|| rebuilt.observe())
}

impl ObservationTreePose {
    pose_observers!();
}

impl PartialEq for MaterialTreeKinematicsModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tree, &other.tree)
    }
}

impl PartialEq for ObservationTreeKinematicsModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tree, &other.tree)
    }
}

impl PartialEq for MaterialTreePose {
    fn eq(&self, other: &Self) -> bool {
        self.same_instance(other)
    }
}

impl PartialEq for MaterialFaceBoundary<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.source, other.source) && self.index == other.index
    }
}

impl Eq for MaterialFaceBoundary<'_> {}

impl PartialEq for ObservationTreePose {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && self.pose == other.pose
    }
}

fn prepare_tree(
    pattern: &CreasePattern,
    paper: &Paper,
    topology: &TopologySnapshot,
    supplied_positions: &[VertexPosition3],
    limits: TreeKinematicsLimits,
) -> Result<PreparedTree, KinematicsError> {
    check_raw_resource_counts(pattern, paper, topology, supplied_positions.len(), limits)?;
    validate_paper_scalar_fields(paper)?;
    if topology.faces.is_empty() || paper.boundary_vertices.len() < 3 {
        return Err(KinematicsError::UnsupportedTopology);
    }

    let (mut positions, source_positions) = unique_positions(pattern, supplied_positions)?;
    let mut simple_boundary_budget = SimpleBoundaryValidationBudget::production();
    let edges = unique_edges(
        pattern,
        paper,
        &positions,
        &source_positions,
        &mut simple_boundary_budget,
    )?;
    let incidences = unique_incidences(topology, &edges)?;
    let (face_ids, face_boundaries, face_keys, occurrences) = validate_faces(
        topology,
        &edges,
        &positions,
        &source_positions,
        &mut simple_boundary_budget,
    )?;
    validate_incidences(&edges, &incidences, &face_ids, &occurrences)?;
    validate_adjacency_registry(topology, &incidences, &face_keys)?;
    retain_material_positions(&mut positions, &edges)?;

    if topology.hinge_adjacency.is_empty() {
        if face_ids.len() != 1 {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let mut adjacency = HashMap::new();
        adjacency
            .try_reserve(1)
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        adjacency.insert(face_ids[0], Vec::new());
        return Ok(PreparedTree {
            face_ids,
            face_boundaries,
            positions,
            hinges: Vec::new(),
            adjacency,
        });
    }
    if topology
        .hinge_adjacency
        .len()
        .checked_add(1)
        .filter(|count| *count == face_ids.len())
        .is_none()
    {
        return Err(KinematicsError::UnsupportedTopology);
    }

    let mut hinges = Vec::new();
    hinges
        .try_reserve_exact(topology.hinge_adjacency.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for adjacent in &topology.hinge_adjacency {
        let EdgeIncidence::Hinge {
            left,
            right,
            assignment,
        } = incidences
            .get(&adjacent.edge)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?
        else {
            return Err(KinematicsError::UnsupportedTopology);
        };
        let source = edges
            .get(&adjacent.edge)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let (start_id, end_id) = canonical_endpoints(source.start, source.end);
        let start = positions
            .get(&start_id)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let end = positions
            .get(&end_id)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let delta = subtract(end, start)?;
        let axis = scale(delta, 1.0 / length(delta)?)?;
        hinges.push(TreeHinge {
            edge: adjacent.edge,
            assignment,
            left_face: left,
            right_face: right,
            start,
            end,
            axis,
        });
    }
    hinges.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());

    let mut hinge_indices = HashMap::new();
    hinge_indices
        .try_reserve(hinges.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for (index, hinge) in hinges.iter().enumerate() {
        if hinge_indices.insert(hinge.edge, index).is_some() {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    let mut adjacency = HashMap::new();
    adjacency
        .try_reserve(face_ids.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for face in &face_ids {
        adjacency.insert(*face, Vec::new());
    }
    for hinge in &hinges {
        let hinge_index = *hinge_indices
            .get(&hinge.edge)
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let sign = match hinge.assignment {
            FoldAssignment::Mountain => 1.0,
            FoldAssignment::Valley => -1.0,
        };
        let left_neighbors = adjacency
            .get_mut(&hinge.left_face)
            .ok_or(KinematicsError::UnsupportedTopology)?;
        left_neighbors
            .try_reserve(1)
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        left_neighbors.push(Neighbor {
            face: hinge.right_face,
            hinge_index,
            rotation_sign: sign,
        });
        let right_neighbors = adjacency
            .get_mut(&hinge.right_face)
            .ok_or(KinematicsError::UnsupportedTopology)?;
        right_neighbors
            .try_reserve(1)
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        right_neighbors.push(Neighbor {
            face: hinge.left_face,
            hinge_index,
            rotation_sign: -sign,
        });
    }
    for neighbors in adjacency.values_mut() {
        neighbors
            .sort_unstable_by_key(|neighbor| hinges[neighbor.hinge_index].edge.canonical_bytes());
    }
    if !connected(&adjacency, face_ids[0], face_ids.len())? {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(PreparedTree {
        face_ids,
        face_boundaries,
        positions,
        hinges,
        adjacency,
    })
}

fn try_clone_canonical_angles(
    angles: &CanonicalHingeAngles,
) -> Result<CanonicalHingeAngles, KinematicsError> {
    let mut snapshot = Vec::new();
    snapshot
        .try_reserve_exact(angles.angles.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    snapshot.extend_from_slice(&angles.angles);
    Ok(CanonicalHingeAngles { angles: snapshot })
}

fn validate_paper_scalar_fields(paper: &Paper) -> Result<(), KinematicsError> {
    if paper.thickness_mm.is_finite() && paper.thickness_mm >= 0.0 {
        Ok(())
    } else {
        Err(KinematicsError::UnrepresentableGeometry)
    }
}

fn check_raw_resource_counts(
    pattern: &CreasePattern,
    paper: &Paper,
    topology: &TopologySnapshot,
    supplied_position_count: usize,
    limits: TreeKinematicsLimits,
) -> Result<(), KinematicsError> {
    if pattern.vertices.len() > limits.max_source_vertices
        || supplied_position_count > limits.max_source_vertices
        || pattern.edges.len() > limits.max_source_edges
        || paper.boundary_vertices.len() > limits.max_paper_boundary_vertices
        || topology.faces.len() > limits.max_faces
        || topology.edge_incidence.len() > limits.max_edge_incidences
        || topology.hinge_adjacency.len() > limits.max_hinges
    {
        return Err(KinematicsError::ResourceLimitExceeded);
    }
    checked_double(topology.hinge_adjacency.len(), limits.max_adjacency_entries)?;
    let mut face_boundary_vertices = 0_usize;
    for face in &topology.faces {
        face_boundary_vertices = checked_accumulate(
            face_boundary_vertices,
            face.outer.half_edges.len(),
            limits.max_face_boundary_vertices,
        )?;
    }
    Ok(())
}

fn unique_positions(
    pattern: &CreasePattern,
    supplied: &[VertexPosition3],
) -> Result<PositionMaps, KinematicsError> {
    let mut source_positions = HashMap::new();
    source_positions
        .try_reserve(pattern.vertices.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for vertex in &pattern.vertices {
        if source_positions
            .insert(vertex.id, vertex.position)
            .is_some()
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    let mut positions = HashMap::new();
    positions
        .try_reserve(supplied.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for position in supplied {
        if !source_positions.contains_key(&position.vertex)
            || positions
                .insert(position.vertex, position.position)
                .is_some()
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    Ok((positions, source_positions))
}

fn unique_edges<'a>(
    pattern: &'a CreasePattern,
    paper: &Paper,
    positions: &HashMap<VertexId, Point3>,
    source_positions: &HashMap<VertexId, Point2>,
    simple_boundary_budget: &mut SimpleBoundaryValidationBudget,
) -> Result<HashMap<EdgeId, &'a Edge>, KinematicsError> {
    let mut boundary_vertices = HashSet::new();
    boundary_vertices
        .try_reserve(paper.boundary_vertices.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut paper_boundary_positions = Vec::new();
    paper_boundary_positions
        .try_reserve_exact(paper.boundary_vertices.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for vertex in &paper.boundary_vertices {
        let source_position = source_positions
            .get(vertex)
            .ok_or(KinematicsError::UnsupportedTopology)?;
        if !source_position.x.is_finite()
            || !source_position.y.is_finite()
            || !positions.contains_key(vertex)
        {
            return Err(KinematicsError::UnrepresentableGeometry);
        }
        if !boundary_vertices.insert(*vertex) {
            return Err(KinematicsError::UnsupportedTopology);
        }
        paper_boundary_positions.push(*source_position);
    }
    validate_simple_boundary(
        &paper.boundary_vertices,
        &paper_boundary_positions,
        simple_boundary_budget,
    )?;
    let mut paper_boundary_edges = HashSet::new();
    paper_boundary_edges
        .try_reserve(paper.boundary_vertices.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for index in 0..paper.boundary_vertices.len() {
        let pair = canonical_endpoints(
            paper.boundary_vertices[index],
            paper.boundary_vertices[(index + 1) % paper.boundary_vertices.len()],
        );
        if pair.0 == pair.1 || !paper_boundary_edges.insert(pair) {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }

    let mut edges = HashMap::new();
    edges
        .try_reserve(pattern.edges.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut source_boundary_edges = HashSet::new();
    source_boundary_edges
        .try_reserve(paper.boundary_vertices.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut material_position_owners = HashMap::new();
    material_position_owners
        .try_reserve(positions.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for edge in &pattern.edges {
        if edges.insert(edge.id, edge).is_some() {
            return Err(KinematicsError::UnsupportedTopology);
        }
        if edge.kind == EdgeKind::Auxiliary {
            continue;
        }
        if edge.kind == EdgeKind::Cut {
            return Err(KinematicsError::UnsupportedTopology);
        }
        if edge.start == edge.end {
            return Err(KinematicsError::UnsupportedTopology);
        }
        for vertex in [edge.start, edge.end] {
            let source_position = source_positions
                .get(&vertex)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            if !source_position.x.is_finite()
                || !source_position.y.is_finite()
                || !positions.contains_key(&vertex)
            {
                return Err(KinematicsError::UnrepresentableGeometry);
            }
            match material_position_owners.entry(exact_point_key(*source_position)) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(vertex);
                }
                std::collections::hash_map::Entry::Occupied(entry) if *entry.get() == vertex => {}
                std::collections::hash_map::Entry::Occupied(_) => {
                    return Err(KinematicsError::UnsupportedTopology);
                }
            }
        }
        if edge.kind == EdgeKind::Boundary
            && !source_boundary_edges.insert(canonical_endpoints(edge.start, edge.end))
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    if source_boundary_edges != paper_boundary_edges {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(edges)
}

fn retain_material_positions(
    positions: &mut HashMap<VertexId, Point3>,
    edges: &HashMap<EdgeId, &Edge>,
) -> Result<(), KinematicsError> {
    let mut material_vertices = HashSet::new();
    material_vertices
        .try_reserve(positions.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for edge in edges.values() {
        if edge.kind != EdgeKind::Auxiliary {
            material_vertices.extend([edge.start, edge.end]);
        }
    }
    positions.retain(|vertex, _| material_vertices.contains(vertex));
    Ok(())
}

fn unique_incidences(
    topology: &TopologySnapshot,
    edges: &HashMap<EdgeId, &Edge>,
) -> Result<HashMap<EdgeId, EdgeIncidence>, KinematicsError> {
    if topology.edge_incidence.len() != edges.len() {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let mut incidences = HashMap::new();
    incidences
        .try_reserve(topology.edge_incidence.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for (edge, incidence) in &topology.edge_incidence {
        if !edges.contains_key(edge) || incidences.insert(*edge, *incidence).is_some() {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    if incidences.len() != edges.len() {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(incidences)
}

type EdgeOccurrence = (FaceId, VertexId, VertexId);
type PositionMaps = (HashMap<VertexId, Point3>, HashMap<VertexId, Point2>);
type ValidatedFaces = (
    Vec<FaceId>,
    Vec<PreparedFaceBoundary>,
    HashMap<FaceId, FaceKey>,
    HashMap<EdgeId, Vec<EdgeOccurrence>>,
);

fn validate_faces(
    topology: &TopologySnapshot,
    edges: &HashMap<EdgeId, &Edge>,
    positions: &HashMap<VertexId, Point3>,
    source_positions: &HashMap<VertexId, Point2>,
    simple_boundary_budget: &mut SimpleBoundaryValidationBudget,
) -> Result<ValidatedFaces, KinematicsError> {
    let mut face_ids_set = HashSet::new();
    face_ids_set
        .try_reserve(topology.faces.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut face_keys_set = HashSet::new();
    face_keys_set
        .try_reserve(topology.faces.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut face_keys = HashMap::new();
    face_keys
        .try_reserve(topology.faces.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut occurrences = HashMap::<EdgeId, Vec<EdgeOccurrence>>::new();
    occurrences
        .try_reserve(edges.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut face_boundaries = Vec::new();
    face_boundaries
        .try_reserve_exact(topology.faces.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for face in &topology.faces {
        let walk = &face.outer.half_edges;
        if walk.len() < 3 || !face_ids_set.insert(face.id) || !face_keys_set.insert(face.key) {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let actual_face_key = canonical_face_key(walk).map_err(map_canonical_face_key_error)?;
        if actual_face_key != face.key {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let mut source_boundary = Vec::new();
        source_boundary
            .try_reserve_exact(walk.len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        let mut boundary_vertices = Vec::new();
        boundary_vertices
            .try_reserve_exact(walk.len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        for half_edge in walk {
            source_boundary.push(
                source_positions
                    .get(&half_edge.origin)
                    .copied()
                    .ok_or(KinematicsError::UnsupportedTopology)?,
            );
            boundary_vertices.push(half_edge.origin);
        }
        validate_simple_boundary(&boundary_vertices, &source_boundary, simple_boundary_budget)?;
        let signed_double_area = polygon_signed_double_area(&source_boundary)
            .map_err(|_| KinematicsError::UnrepresentableGeometry)?;
        let area = signed_double_area * 0.5;
        if signed_double_area != face.outer.signed_double_area
            || area != face.area
            || signed_double_area <= 0.0
            || !signed_double_area.is_finite()
            || area <= 0.0
            || !area.is_finite()
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
        face_keys.insert(face.id, face.key);
        let mut boundary_edges = Vec::new();
        boundary_edges
            .try_reserve_exact(walk.len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        for (index, half_edge) in walk.iter().enumerate() {
            let next = &walk[(index + 1) % walk.len()];
            let source = edges
                .get(&half_edge.edge)
                .copied()
                .ok_or(KinematicsError::UnsupportedTopology)?;
            if half_edge.destination != next.origin
                || !positions.contains_key(&half_edge.origin)
                || !positions.contains_key(&half_edge.destination)
                || !same_endpoints(
                    source.start,
                    source.end,
                    half_edge.origin,
                    half_edge.destination,
                )
            {
                return Err(KinematicsError::UnsupportedTopology);
            }
            let edge_occurrences = occurrences.entry(half_edge.edge).or_default();
            edge_occurrences
                .try_reserve(1)
                .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
            edge_occurrences.push((face.id, half_edge.origin, half_edge.destination));
            boundary_edges.push(half_edge.edge);
        }
        canonicalize_face_boundary(&mut boundary_vertices, &mut boundary_edges)?;
        face_boundaries.push(PreparedFaceBoundary {
            face: face.id,
            vertices: boundary_vertices,
            edges: boundary_edges,
        });
    }
    let mut face_ids = Vec::new();
    face_ids
        .try_reserve_exact(face_ids_set.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    face_ids.extend(face_ids_set);
    face_ids.sort_unstable_by_key(FaceId::canonical_bytes);
    face_boundaries.sort_unstable_by_key(|boundary| boundary.face.canonical_bytes());
    if face_ids.len() != face_boundaries.len()
        || !face_ids
            .iter()
            .zip(&face_boundaries)
            .all(|(face, boundary)| *face == boundary.face)
    {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok((face_ids, face_boundaries, face_keys, occurrences))
}

/// Validates the exact binary64 topology of one closed material boundary.
///
/// Concave polygons and consecutive collinear vertices are supported. A
/// collinear middle vertex is valid only when the two adjacent edges meet at
/// that endpoint without backtracking over a positive interval. Every
/// non-adjacent edge pair must be disjoint. Distinct vertex records at the
/// same exact coordinate, including `-0.0` versus `+0.0`, are rejected before
/// an identity-bearing material boundary can be issued.
fn validate_simple_boundary(
    vertices: &[VertexId],
    points: &[Point2],
    budget: &mut SimpleBoundaryValidationBudget,
) -> Result<(), KinematicsError> {
    if vertices.len() < 3 || vertices.len() != points.len() {
        return Err(KinematicsError::UnsupportedTopology);
    }

    let mut unique_vertices = HashSet::new();
    unique_vertices
        .try_reserve(vertices.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut unique_points = HashSet::new();
    unique_points
        .try_reserve(points.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for (vertex, point) in vertices.iter().copied().zip(points.iter().copied()) {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(KinematicsError::UnrepresentableGeometry);
        }
        if !unique_vertices.insert(vertex) || !unique_points.insert(exact_point_key(point)) {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }

    for index in 0..points.len() {
        if points[index] == points[(index + 1) % points.len()] {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }

    // An x-axis sweep is a conservative broad phase only. All accepted or
    // rejected intersection topology comes from ori-geometry's exact
    // arbitrary-precision binary64 segment predicate.
    let mut by_min_x = Vec::new();
    by_min_x
        .try_reserve_exact(points.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    by_min_x.extend(0..points.len());
    by_min_x.sort_unstable_by(|left, right| {
        boundary_edge_min_x(points, *left)
            .total_cmp(&boundary_edge_min_x(points, *right))
            .then_with(|| left.cmp(right))
    });

    for (sweep_index, first_index) in by_min_x.iter().copied().enumerate() {
        let first_start = points[first_index];
        let first_end = points[(first_index + 1) % points.len()];
        let first_max_x = first_start.x.max(first_end.x);
        let first_min_y = first_start.y.min(first_end.y);
        let first_max_y = first_start.y.max(first_end.y);
        for second_index in by_min_x.iter().copied().skip(sweep_index + 1) {
            if boundary_edge_min_x(points, second_index) > first_max_x {
                break;
            }
            let second_start = points[second_index];
            let second_end = points[(second_index + 1) % points.len()];
            let second_min_y = second_start.y.min(second_end.y);
            let second_max_y = second_start.y.max(second_end.y);
            if second_min_y > first_max_y || first_min_y > second_max_y {
                continue;
            }

            budget.charge_exact_intersection_test()?;
            let adjacent = boundary_edges_are_adjacent(first_index, second_index, points.len());
            match segment_intersection(first_start, first_end, second_start, second_end) {
                Ok(SegmentIntersection::None) => {}
                Ok(SegmentIntersection::Point(point))
                    if adjacent
                        && adjacent_shared_endpoint(first_index, second_index, points) == point => {
                }
                Ok(SegmentIntersection::Point(_) | SegmentIntersection::CollinearOverlap) => {
                    return Err(KinematicsError::UnsupportedTopology);
                }
                Err(_) => return Err(KinematicsError::UnrepresentableGeometry),
            }
        }
    }
    Ok(())
}

fn boundary_edge_min_x(points: &[Point2], index: usize) -> f64 {
    points[index].x.min(points[(index + 1) % points.len()].x)
}

fn boundary_edges_are_adjacent(first: usize, second: usize, length: usize) -> bool {
    first.abs_diff(second) == 1 || first.abs_diff(second) == length - 1
}

fn adjacent_shared_endpoint(first: usize, second: usize, points: &[Point2]) -> Point2 {
    if (first + 1) % points.len() == second {
        points[second]
    } else {
        debug_assert_eq!((second + 1) % points.len(), first);
        points[first]
    }
}

fn exact_point_key(point: Point2) -> (u64, u64) {
    (exact_coordinate_key(point.x), exact_coordinate_key(point.y))
}

fn exact_coordinate_key(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

fn map_canonical_face_key_error(error: CanonicalFaceKeyError) -> KinematicsError {
    match error {
        CanonicalFaceKeyError::AllocationFailed
        | CanonicalFaceKeyError::BoundaryLengthUnrepresentable => {
            KinematicsError::ResourceLimitExceeded
        }
    }
}

fn canonicalize_face_boundary(
    vertices: &mut [VertexId],
    edges: &mut [EdgeId],
) -> Result<(), KinematicsError> {
    if vertices.len() < 3 || vertices.len() != edges.len() {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let start = (0..vertices.len())
        .min_by_key(|index| {
            (
                edges[*index].canonical_bytes(),
                vertices[*index].canonical_bytes(),
                vertices[(*index + 1) % vertices.len()].canonical_bytes(),
            )
        })
        .ok_or(KinematicsError::UnsupportedTopology)?;
    vertices.rotate_left(start);
    edges.rotate_left(start);
    Ok(())
}

fn validate_incidences(
    edges: &HashMap<EdgeId, &Edge>,
    incidences: &HashMap<EdgeId, EdgeIncidence>,
    face_ids: &[FaceId],
    occurrences: &HashMap<EdgeId, Vec<EdgeOccurrence>>,
) -> Result<(), KinematicsError> {
    let mut faces = HashSet::new();
    faces
        .try_reserve(face_ids.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    faces.extend(face_ids.iter().copied());
    for (edge_id, source) in edges {
        let incidence = incidences
            .get(edge_id)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let edge_occurrences = occurrences.get(edge_id).map(Vec::as_slice).unwrap_or(&[]);
        match incidence {
            EdgeIncidence::Boundary { material } => {
                if source.kind != EdgeKind::Boundary
                    || !faces.contains(&material)
                    || edge_occurrences.len() != 1
                    || edge_occurrences[0].0 != material
                {
                    return Err(KinematicsError::UnsupportedTopology);
                }
            }
            EdgeIncidence::Hinge {
                left,
                right,
                assignment,
            } => {
                if left == right
                    || !faces.contains(&left)
                    || !faces.contains(&right)
                    || source.kind
                        != match assignment {
                            FoldAssignment::Mountain => EdgeKind::Mountain,
                            FoldAssignment::Valley => EdgeKind::Valley,
                        }
                    || edge_occurrences.len() != 2
                {
                    return Err(KinematicsError::UnsupportedTopology);
                }
                let (start, end) = canonical_endpoints(source.start, source.end);
                let left_valid = edge_occurrences.contains(&(left, start, end));
                let right_valid = edge_occurrences.contains(&(right, end, start));
                if !left_valid || !right_valid {
                    return Err(KinematicsError::UnsupportedTopology);
                }
            }
            EdgeIncidence::Cut { left, right } => {
                if !faces.contains(&left)
                    || !faces.contains(&right)
                    || source.kind != EdgeKind::Cut
                    || edge_occurrences.len() != 2
                {
                    return Err(KinematicsError::UnsupportedTopology);
                }
                let (start, end) = canonical_endpoints(source.start, source.end);
                if !edge_occurrences.contains(&(left, start, end))
                    || !edge_occurrences.contains(&(right, end, start))
                {
                    return Err(KinematicsError::UnsupportedTopology);
                }
            }
            EdgeIncidence::AuxiliaryIgnored => {
                if source.kind != EdgeKind::Auxiliary || !edge_occurrences.is_empty() {
                    return Err(KinematicsError::UnsupportedTopology);
                }
            }
        }
    }
    Ok(())
}

fn validate_adjacency_registry(
    topology: &TopologySnapshot,
    incidences: &HashMap<EdgeId, EdgeIncidence>,
    face_keys: &HashMap<FaceId, FaceKey>,
) -> Result<(), KinematicsError> {
    let hinge_count = incidences
        .values()
        .filter(|incidence| matches!(incidence, EdgeIncidence::Hinge { .. }))
        .count();
    if hinge_count != topology.hinge_adjacency.len() {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let mut expected_hinges = HashSet::new();
    expected_hinges
        .try_reserve(hinge_count)
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for (edge, incidence) in incidences {
        if matches!(incidence, EdgeIncidence::Hinge { .. }) && !expected_hinges.insert(*edge) {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    let mut observed_hinges = HashSet::new();
    observed_hinges
        .try_reserve(topology.hinge_adjacency.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for adjacent in &topology.hinge_adjacency {
        validate_one_adjacency(adjacent, incidences, face_keys)?;
        if !observed_hinges.insert(adjacent.edge) {
            return Err(KinematicsError::UnsupportedTopology);
        }
    }
    if observed_hinges != expected_hinges {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(())
}

fn validate_one_adjacency(
    adjacent: &FaceAdjacency,
    incidences: &HashMap<EdgeId, EdgeIncidence>,
    face_keys: &HashMap<FaceId, FaceKey>,
) -> Result<(), KinematicsError> {
    let first_key = face_keys
        .get(&adjacent.first)
        .ok_or(KinematicsError::UnsupportedTopology)?;
    let second_key = face_keys
        .get(&adjacent.second)
        .ok_or(KinematicsError::UnsupportedTopology)?;
    if adjacent.first == adjacent.second || first_key >= second_key {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let EdgeIncidence::Hinge {
        left,
        right,
        assignment,
    } = incidences
        .get(&adjacent.edge)
        .copied()
        .ok_or(KinematicsError::UnsupportedTopology)?
    else {
        return Err(KinematicsError::UnsupportedTopology);
    };
    if assignment != adjacent.assignment
        || !same_faces(adjacent.first, adjacent.second, left, right)
    {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(())
}

fn find_material_face_boundary(
    source: &PreparedTree,
    face: FaceId,
) -> Option<MaterialFaceBoundary<'_>> {
    let index = source
        .face_boundaries
        .binary_search_by_key(&face.canonical_bytes(), |boundary| {
            boundary.face.canonical_bytes()
        })
        .ok()?;
    Some(MaterialFaceBoundary { source, index })
}

fn owns_material_face_boundary(source: &PreparedTree, boundary: MaterialFaceBoundary<'_>) -> bool {
    std::ptr::eq(source, boundary.source)
        && source
            .face_boundaries
            .get(boundary.index)
            .is_some_and(|candidate| candidate.face == boundary.face())
}

struct PreparedMaterialGraph {
    face_ids: Vec<FaceId>,
    hinges: Vec<TreeHinge>,
    positions: HashMap<VertexId, Point3>,
    face_boundaries: Vec<PreparedFaceBoundary>,
}

fn prepare_material_graph(
    pattern: &CreasePattern,
    paper: &Paper,
    topology: &TopologySnapshot,
    supplied_positions: &[VertexPosition3],
    limits: TreeKinematicsLimits,
) -> Result<PreparedMaterialGraph, KinematicsError> {
    check_raw_resource_counts(pattern, paper, topology, supplied_positions.len(), limits)?;
    validate_paper_scalar_fields(paper)?;
    if topology.faces.is_empty() || paper.boundary_vertices.len() < 3 {
        return Err(KinematicsError::UnsupportedTopology);
    }
    let (mut positions, source_positions) = unique_positions(pattern, supplied_positions)?;
    let mut simple_boundary_budget = SimpleBoundaryValidationBudget::production();
    let edges = unique_edges(
        pattern,
        paper,
        &positions,
        &source_positions,
        &mut simple_boundary_budget,
    )?;
    let incidences = unique_incidences(topology, &edges)?;
    let (face_ids, face_boundaries, face_keys, occurrences) = validate_faces(
        topology,
        &edges,
        &positions,
        &source_positions,
        &mut simple_boundary_budget,
    )?;
    validate_incidences(&edges, &incidences, &face_ids, &occurrences)?;
    validate_adjacency_registry(topology, &incidences, &face_keys)?;
    retain_material_positions(&mut positions, &edges)?;

    let mut hinges = Vec::new();
    hinges
        .try_reserve_exact(topology.hinge_adjacency.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for adjacent in &topology.hinge_adjacency {
        let EdgeIncidence::Hinge {
            left,
            right,
            assignment,
        } = incidences
            .get(&adjacent.edge)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?
        else {
            return Err(KinematicsError::UnsupportedTopology);
        };
        let source = edges
            .get(&adjacent.edge)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let (start_id, end_id) = canonical_endpoints(source.start, source.end);
        let start = positions
            .get(&start_id)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let end = positions
            .get(&end_id)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let delta = subtract(end, start)?;
        let axis = scale(delta, 1.0 / length(delta)?)?;
        hinges.push(TreeHinge {
            edge: adjacent.edge,
            assignment,
            left_face: left,
            right_face: right,
            start,
            end,
            axis,
        });
    }
    hinges.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    if hinges.windows(2).any(|pair| pair[0].edge == pair[1].edge) {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(PreparedMaterialGraph {
        face_ids,
        hinges,
        positions,
        face_boundaries,
    })
}

fn solve_tree(
    model: &PreparedTree,
    fixed_face: Option<FaceId>,
    angles: &CanonicalHingeAngles,
) -> Result<TreePoseData, KinematicsError> {
    validate_complete_angles(model, angles)?;
    if model.hinges.is_empty() {
        if let Some(face) = fixed_face {
            return Err(KinematicsError::UnexpectedFixedFace { face });
        }
        let mut face_transforms = Vec::new();
        face_transforms
            .try_reserve_exact(1)
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        face_transforms.push((model.face_ids[0], RigidTransform::identity()));
        return Ok(TreePoseData {
            face_transforms,
            hinge_parent_transforms: Vec::new(),
        });
    }
    let root = fixed_face.ok_or(KinematicsError::MissingFixedFace)?;
    if model
        .face_ids
        .binary_search_by_key(&root.canonical_bytes(), FaceId::canonical_bytes)
        .is_err()
    {
        return Err(KinematicsError::UnknownFixedFace { face: root });
    }

    let mut transforms = HashMap::new();
    transforms
        .try_reserve(model.face_ids.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    transforms.insert(root, RigidTransform::identity());
    let mut hinge_parents = Vec::new();
    hinge_parents
        .try_reserve_exact(model.hinges.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    hinge_parents.resize(model.hinges.len(), None);
    let mut queue = VecDeque::new();
    queue
        .try_reserve(model.face_ids.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    queue.push_back(root);
    while let Some(parent_face) = queue.pop_front() {
        let parent = transforms
            .get(&parent_face)
            .copied()
            .ok_or(KinematicsError::UnsupportedTopology)?;
        let neighbors = model
            .adjacency
            .get(&parent_face)
            .ok_or(KinematicsError::UnsupportedTopology)?;
        for neighbor in neighbors {
            if transforms.contains_key(&neighbor.face) {
                continue;
            }
            let hinge = model
                .hinges
                .get(neighbor.hinge_index)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            if hinge_parents
                .get_mut(neighbor.hinge_index)
                .ok_or(KinematicsError::UnsupportedTopology)?
                .replace(parent)
                .is_some()
            {
                return Err(KinematicsError::UnsupportedTopology);
            }
            let angle = angle_for(angles, hinge.edge)
                .ok_or(KinematicsError::MissingHingeAngle { edge: hinge.edge })?;
            let local = RigidTransform::around_axis(
                hinge.start,
                hinge.axis,
                angle * neighbor.rotation_sign,
            )?;
            let child = parent.compose(local)?;
            if transforms.insert(neighbor.face, child).is_some() {
                return Err(KinematicsError::UnsupportedTopology);
            }
            queue.push_back(neighbor.face);
        }
    }
    if transforms.len() != model.face_ids.len() {
        return Err(KinematicsError::UnsupportedTopology);
    }

    let mut face_transforms = Vec::new();
    face_transforms
        .try_reserve_exact(model.face_ids.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for face in &model.face_ids {
        face_transforms.push((
            *face,
            transforms
                .get(face)
                .copied()
                .ok_or(KinematicsError::UnsupportedTopology)?,
        ));
    }
    let mut hinge_parent_transforms = Vec::new();
    hinge_parent_transforms
        .try_reserve_exact(model.hinges.len())
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    for (hinge, transform) in model.hinges.iter().zip(hinge_parents) {
        hinge_parent_transforms.push((
            hinge.edge,
            transform.ok_or(KinematicsError::UnsupportedTopology)?,
        ));
    }
    Ok(TreePoseData {
        face_transforms,
        hinge_parent_transforms,
    })
}

fn validate_complete_angles(
    model: &PreparedTree,
    angles: &CanonicalHingeAngles,
) -> Result<(), KinematicsError> {
    if angles.angles.len() < model.hinges.len() {
        let missing = model
            .hinges
            .iter()
            .find(|hinge| angle_for(angles, hinge.edge).is_none())
            .ok_or(KinematicsError::UnsupportedTopology)?;
        return Err(KinematicsError::MissingHingeAngle { edge: missing.edge });
    }
    if angles.angles.len() > model.hinges.len() {
        let extra = angles
            .angles
            .iter()
            .find(|angle| hinge_index(model, angle.edge).is_none())
            .ok_or(KinematicsError::UnsupportedTopology)?;
        return Err(KinematicsError::ExtraHingeAngle { edge: extra.edge });
    }
    for angle in &angles.angles {
        if hinge_index(model, angle.edge).is_none() {
            return Err(KinematicsError::UnknownHingeAngle { edge: angle.edge });
        }
    }
    Ok(())
}

fn angle_for(angles: &CanonicalHingeAngles, edge: EdgeId) -> Option<f64> {
    angles
        .angles
        .binary_search_by_key(&edge.canonical_bytes(), |angle| {
            angle.edge.canonical_bytes()
        })
        .ok()
        .map(|index| angles.angles[index].angle_degrees)
}

fn hinge_index(model: &PreparedTree, edge: EdgeId) -> Option<usize> {
    model
        .hinges
        .binary_search_by_key(&edge.canonical_bytes(), |hinge| {
            hinge.edge.canonical_bytes()
        })
        .ok()
}

fn find_face_transform(pose: &TreePoseData, face: FaceId) -> Option<RigidTransform> {
    pose.face_transforms
        .binary_search_by_key(&face.canonical_bytes(), |entry| entry.0.canonical_bytes())
        .ok()
        .map(|index| pose.face_transforms[index].1)
}

fn find_hinge_parent_transform(pose: &TreePoseData, edge: EdgeId) -> Option<RigidTransform> {
    pose.hinge_parent_transforms
        .binary_search_by_key(&edge.canonical_bytes(), |entry| entry.0.canonical_bytes())
        .ok()
        .map(|index| pose.hinge_parent_transforms[index].1)
}

fn connected(
    adjacency: &HashMap<FaceId, Vec<Neighbor>>,
    root: FaceId,
    expected: usize,
) -> Result<bool, KinematicsError> {
    let mut visited = HashSet::new();
    visited
        .try_reserve(expected)
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    let mut queue = VecDeque::new();
    queue
        .try_reserve(expected)
        .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
    queue.push_back(root);
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
    Ok(visited.len() == expected)
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

fn checked_accumulate(
    current: usize,
    additional: usize,
    maximum: usize,
) -> Result<usize, KinematicsError> {
    current
        .checked_add(additional)
        .filter(|total| *total <= maximum)
        .ok_or(KinematicsError::ResourceLimitExceeded)
}

fn checked_double(count: usize, maximum: usize) -> Result<usize, KinematicsError> {
    count
        .checked_mul(2)
        .filter(|total| *total <= maximum)
        .ok_or(KinematicsError::ResourceLimitExceeded)
}

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, Point2, VertexId};
    use ori_topology::{CanonicalFaceKeyError, FoldAssignment};

    use super::{
        HingeAngle, SimpleBoundaryValidationBudget, assignment_signed_angle_degrees_v1,
        checked_accumulate, checked_double, exact_point_key, map_canonical_face_key_error,
        validate_simple_boundary,
    };
    use crate::KinematicsError;

    fn vertex_ids(count: usize) -> Vec<VertexId> {
        (0..count).map(|_| VertexId::new()).collect()
    }

    fn validate_points(points: &[Point2]) -> Result<(), KinematicsError> {
        validate_simple_boundary(
            &vertex_ids(points.len()),
            points,
            &mut SimpleBoundaryValidationBudget::production(),
        )
    }

    #[test]
    fn assignment_signed_boundary_is_bit_exact_and_live_edge_bound() {
        let edge = EdgeId::new();
        for magnitude in [0.0, 30.0, 180.0] {
            let angle = HingeAngle::new(edge, magnitude).unwrap();
            assert_eq!(
                assignment_signed_angle_degrees_v1(edge, FoldAssignment::Mountain, angle)
                    .unwrap()
                    .to_bits(),
                magnitude.to_bits()
            );
            let expected = if magnitude == 0.0 { 0.0 } else { -magnitude };
            assert_eq!(
                assignment_signed_angle_degrees_v1(edge, FoldAssignment::Valley, angle)
                    .unwrap()
                    .to_bits(),
                expected.to_bits()
            );
        }
        assert_eq!(
            assignment_signed_angle_degrees_v1(
                EdgeId::new(),
                FoldAssignment::Mountain,
                HingeAngle::new(edge, 30.0).unwrap(),
            ),
            Err(KinematicsError::UnsupportedTopology)
        );
    }

    #[test]
    fn resource_arithmetic_cannot_overflow_or_cross_its_limit() {
        assert_eq!(checked_accumulate(7, 5, 12), Ok(12));
        assert_eq!(
            checked_accumulate(7, 6, 12),
            Err(KinematicsError::ResourceLimitExceeded)
        );
        assert_eq!(
            checked_accumulate(usize::MAX, 1, usize::MAX),
            Err(KinematicsError::ResourceLimitExceeded)
        );
        assert_eq!(checked_double(6, 12), Ok(12));
        assert_eq!(
            checked_double(7, 12),
            Err(KinematicsError::ResourceLimitExceeded)
        );
        assert_eq!(
            checked_double(usize::MAX, usize::MAX),
            Err(KinematicsError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn exact_simple_boundary_accepts_concavity_collinear_vertices_and_cycle_symmetry() {
        let points = vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 4.0),
        ];
        for rotation in 0..points.len() {
            let mut rotated = points.clone();
            rotated.rotate_left(rotation);
            assert_eq!(validate_points(&rotated), Ok(()));
            rotated.reverse();
            assert_eq!(validate_points(&rotated), Ok(()));
        }
    }

    #[test]
    fn exact_simple_boundary_rejects_every_non_simple_contact_class() {
        let invalid = [
            // Strict crossing with positive signed area.
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(0.0, 4.0),
                Point2::new(4.0, 4.0),
                Point2::new(0.0, 3.0),
            ],
            // A non-adjacent vertex touches the strict interior of edge 0.
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 4.0),
                Point2::new(2.0, 0.0),
                Point2::new(0.0, 4.0),
            ],
            // Adjacent collinear edges backtrack over positive length.
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(2.0, 0.0),
                Point2::new(4.0, 4.0),
                Point2::new(0.0, 4.0),
            ],
            // A non-adjacent edge overlaps the first edge.
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 4.0),
                Point2::new(1.0, 0.0),
                Point2::new(3.0, 0.0),
                Point2::new(0.0, 4.0),
            ],
        ];
        for points in invalid {
            for rotation in 0..points.len() {
                let mut rotated = points.clone();
                rotated.rotate_left(rotation);
                assert_eq!(
                    validate_points(&rotated),
                    Err(KinematicsError::UnsupportedTopology)
                );
                rotated.reverse();
                assert_eq!(
                    validate_points(&rotated),
                    Err(KinematicsError::UnsupportedTopology)
                );
            }
        }
    }

    #[test]
    fn exact_simple_boundary_rejects_duplicate_identity_and_exact_coordinate() {
        let points = [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(0.0, 4.0),
        ];
        let mut duplicate_identity = vertex_ids(points.len());
        duplicate_identity[2] = duplicate_identity[0];
        assert_eq!(
            validate_simple_boundary(
                &duplicate_identity,
                &points,
                &mut SimpleBoundaryValidationBudget::production(),
            ),
            Err(KinematicsError::UnsupportedTopology)
        );

        let duplicate_coordinate = [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(-0.0, 0.0),
            Point2::new(0.0, 4.0),
        ];
        assert_eq!(
            validate_points(&duplicate_coordinate),
            Err(KinematicsError::UnsupportedTopology)
        );
        assert_eq!(
            exact_point_key(Point2::new(-0.0, 0.0)),
            exact_point_key(Point2::new(0.0, -0.0))
        );
    }

    #[test]
    fn exact_simple_boundary_handles_maximum_and_subnormal_finite_coordinates() {
        let maximum = f64::MAX;
        assert_eq!(
            validate_points(&[
                Point2::new(-maximum, -maximum),
                Point2::new(maximum, -maximum),
                Point2::new(maximum, maximum),
                Point2::new(-maximum, maximum),
            ]),
            Ok(())
        );

        let subnormal = f64::from_bits(1);
        assert_eq!(
            validate_points(&[
                Point2::new(0.0, 0.0),
                Point2::new(subnormal, 0.0),
                Point2::new(subnormal, subnormal),
                Point2::new(0.0, subnormal),
            ]),
            Ok(())
        );
    }

    #[test]
    fn exact_simple_boundary_never_accepts_extreme_scale_crossings() {
        for scale in [f64::from_bits(1), f64::MAX / 4.0] {
            let crossing = [
                Point2::new(0.0, 0.0),
                Point2::new(4.0 * scale, 0.0),
                Point2::new(0.0, 4.0 * scale),
                Point2::new(4.0 * scale, 4.0 * scale),
                Point2::new(0.0, 3.0 * scale),
            ];
            assert!(
                validate_points(&crossing).is_err(),
                "an exact crossing must fail closed at scale {scale}"
            );
        }
    }

    #[test]
    fn exact_simple_boundary_work_budget_is_fail_closed_at_one_past_limit() {
        let points = [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(0.0, 4.0),
        ];
        let vertices = vertex_ids(points.len());
        let mut measured = SimpleBoundaryValidationBudget {
            remaining_exact_intersection_tests: 100,
        };
        validate_simple_boundary(&vertices, &points, &mut measured).expect("measured square");
        let exact = 100 - measured.remaining_exact_intersection_tests;
        assert!(exact > 0);

        assert_eq!(
            validate_simple_boundary(
                &vertices,
                &points,
                &mut SimpleBoundaryValidationBudget {
                    remaining_exact_intersection_tests: exact,
                },
            ),
            Ok(())
        );
        assert_eq!(
            validate_simple_boundary(
                &vertices,
                &points,
                &mut SimpleBoundaryValidationBudget {
                    remaining_exact_intersection_tests: exact - 1,
                },
            ),
            Err(KinematicsError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn canonical_face_key_allocation_failure_remains_a_resource_failure() {
        assert_eq!(
            map_canonical_face_key_error(CanonicalFaceKeyError::AllocationFailed),
            KinematicsError::ResourceLimitExceeded
        );
        assert_eq!(
            map_canonical_face_key_error(CanonicalFaceKeyError::BoundaryLengthUnrepresentable),
            KinematicsError::ResourceLimitExceeded
        );
    }
}
