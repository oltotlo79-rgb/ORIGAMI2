use std::{collections::HashSet, sync::Arc};

use ori_domain::FaceId;
use ori_kinematics::{
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel, MaterialTreePose, Point3,
};
use thiserror::Error;

use crate::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
    NativeStaticCollisionGeometryProof, TOPOLOGY_CONTACT_POLICY_V2,
};

/// First current-pose cell-order model.
///
/// Version 1 deliberately certifies only the no-hinge, one-material-face
/// class. It is an internal bootstrap for the general transport proof and
/// must never be interpreted as support for a multi-face current pose.
pub const CURRENT_POSE_CELL_ORDER_MODEL_ID_V1: &str = "current_pose_single_face_cell_order_v1";

/// First opaque native cell-order transport proof format.
pub const NATIVE_CELL_ORDER_TRANSPORT_PROOF_V1: &str =
    "native_single_face_cell_order_transport_proof_v1";

/// Deterministic count limits charged before publishing a current-pose cell
/// order.
///
/// These are logical work and retained-record ceilings, not a claim about a
/// process-wide heap limit. Equality is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellOrderTransportLimitsV1 {
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_cells: usize,
    pub max_boundary_vertices_per_cell: usize,
    pub max_total_boundary_vertices: usize,
    pub max_total_layer_records: usize,
}

impl Default for CellOrderTransportLimitsV1 {
    fn default() -> Self {
        Self {
            max_faces: 10_001,
            max_hinges: 10_000,
            max_cells: 100_000,
            max_boundary_vertices_per_cell: 4_096,
            max_total_boundary_vertices: 50_000,
            max_total_layer_records: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellOrderTransportResourceV1 {
    Faces,
    Hinges,
    Cells,
    BoundaryVerticesPerCell,
    TotalBoundaryVertices,
    LayerRecords,
    WorldBoundaryStorage,
    LayerRecordStorage,
}

/// Every failure is blocking. No error permits a caller to infer a current
/// layer order.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CellOrderTransportErrorV1 {
    #[error("the material pose was issued by a different kinematics model instance")]
    PoseIssuerMismatch,
    #[error("the static-collision proof is not bound to the exact model and pose")]
    CollisionProofMismatch,
    #[error("the cell-order proof is not bound to the exact supplied authority objects")]
    ProofBindingMismatch,
    #[error(
        "the current cell-order proof does not support this pose class ({faces} faces, {hinges} hinges)"
    )]
    UnsupportedPoseClass { faces: usize, hinges: usize },
    #[error("the material pose registry is internally inconsistent")]
    InconsistentMaterialPose,
    #[error("the current world-space cell boundary could not be represented")]
    WorldGeometryUnavailable,
    #[error("the current world-space cell boundary is degenerate")]
    WorldBoundaryDegenerate,
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimitExceeded {
        resource: CellOrderTransportResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("cell-order resource counting overflowed")]
    ResourceCountOverflow,
    #[error("cell-order storage allocation failed for {resource:?}")]
    AllocationFailed {
        resource: CellOrderTransportResourceV1,
    },
    #[error("immutable current-pose cell-order revalidation failed")]
    CertificateReverificationFailed,
}

/// Stable key for the sole cell admitted by the V1 proof class.
///
/// A single-face cell is exactly that material face, so its canonical face ID
/// is sufficient inside this versioned model. General multi-face transport
/// will require a distinct arrangement-derived key and a new model ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrentPoseCellKeyV1([u8; 16]);

impl CurrentPoseCellKeyV1 {
    #[must_use]
    pub const fn canonical_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// One completely enumerated current world-space cell and its local order.
///
/// Fields are private so partial boundaries and partial layer lists cannot be
/// constructed as certified records.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentPoseLayerCellV1 {
    cell_key: CurrentPoseCellKeyV1,
    cell_face: FaceId,
    world_boundary: Vec<Point3>,
    bottom_to_top_faces: Vec<FaceId>,
}

impl CurrentPoseLayerCellV1 {
    #[must_use]
    pub const fn cell_key(&self) -> CurrentPoseCellKeyV1 {
        self.cell_key
    }

    #[must_use]
    pub const fn cell_face(&self) -> FaceId {
        self.cell_face
    }

    #[must_use]
    pub fn world_boundary(&self) -> &[Point3] {
        &self.world_boundary
    }

    #[must_use]
    pub fn bottom_to_top_faces(&self) -> &[FaceId] {
        &self.bottom_to_top_faces
    }
}

#[derive(Debug)]
struct CellOrderTransportProofV1 {
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
    collision: NativeStaticCollisionGeometryProof,
    paper_thickness_bits: u64,
    material_faces: Vec<FaceId>,
    cells: Vec<CurrentPoseLayerCellV1>,
}

/// Opaque geometry proof for one complete current-pose cell order.
///
/// Clones preserve proof identity. Re-solving the same hinge angles or
/// re-running static collision produces different native objects and is
/// rejected by [`Self::is_for_geometry_and_collision`].
///
/// This proof carries no project, revision, current-pose generation, or
/// current layer-order capability. It therefore cannot authorize a project
/// mutation by itself.
///
/// ```compile_fail
/// use ori_collision::NativeCellOrderTransportProofV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<NativeCellOrderTransportProofV1>();
/// ```
#[derive(Debug, Clone)]
pub struct NativeCellOrderTransportProofV1 {
    proof: Arc<CellOrderTransportProofV1>,
}

impl PartialEq for NativeCellOrderTransportProofV1 {
    fn eq(&self, other: &Self) -> bool {
        self.same_proof(other)
    }
}

impl Eq for NativeCellOrderTransportProofV1 {}

impl NativeCellOrderTransportProofV1 {
    #[must_use]
    pub const fn proof_id(&self) -> &'static str {
        NATIVE_CELL_ORDER_TRANSPORT_PROOF_V1
    }

    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CURRENT_POSE_CELL_ORDER_MODEL_ID_V1
    }

    #[must_use]
    pub const fn kinematics_model_id(&self) -> &'static str {
        MATERIAL_TREE_KINEMATICS_MODEL_ID
    }

    #[must_use]
    pub const fn collision_proof_id(&self) -> &'static str {
        NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1
    }

    #[must_use]
    pub const fn thickness_model_id(&self) -> &'static str {
        CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
    }

    #[must_use]
    pub fn paper_thickness_bits(&self) -> u64 {
        self.proof.paper_thickness_bits
    }

    #[must_use]
    pub fn paper_thickness_mm(&self) -> f64 {
        f64::from_bits(self.proof.paper_thickness_bits)
    }

    #[must_use]
    pub fn material_faces(&self) -> &[FaceId] {
        &self.proof.material_faces
    }

    #[must_use]
    pub fn cells(&self) -> &[CurrentPoseLayerCellV1] {
        &self.proof.cells
    }

    #[must_use]
    pub fn same_proof(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.proof, &other.proof)
    }

    /// Checks exact issuer, pose-instance, collision-proof identity, and
    /// bit-exact thickness binding without recomputing geometry.
    #[must_use]
    pub fn is_for_geometry_and_collision(
        &self,
        model: &MaterialTreeKinematicsModel,
        pose: &MaterialTreePose,
        collision: &NativeStaticCollisionGeometryProof,
    ) -> bool {
        self.proof.model == *model
            && self.proof.pose.same_instance(pose)
            && self.proof.collision.same_proof(collision)
            && collision.is_for_geometry(model, pose, self.paper_thickness_mm())
            && collision.proof_id() == NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1
            && collision.policy_id() == TOPOLOGY_CONTACT_POLICY_V2
            && collision.kinematics_model_id() == MATERIAL_TREE_KINEMATICS_MODEL_ID
            && collision.thickness_model_id() == CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
    }
}

#[derive(Debug)]
struct SingleFaceCellOrderAnalysis {
    paper_thickness_bits: u64,
    material_faces: Vec<FaceId>,
    cells: Vec<CurrentPoseLayerCellV1>,
}

/// Proves the complete current world-space cell order for the no-hinge,
/// single-material-face bootstrap class.
///
/// All multi-face, shared-feature, and unresolved collision cases fail closed:
/// the required static proof is currently unissuable for them, and this
/// function independently rejects every non-single-face pose class.
pub fn prove_single_face_cell_order_transport_v1(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    collision: &NativeStaticCollisionGeometryProof,
    limits: CellOrderTransportLimitsV1,
) -> Result<NativeCellOrderTransportProofV1, CellOrderTransportErrorV1> {
    let analysis = analyze_single_face_cell_order(model, pose, collision, limits)?;
    Ok(NativeCellOrderTransportProofV1 {
        proof: Arc::new(CellOrderTransportProofV1 {
            model: model.clone(),
            pose: pose.clone(),
            collision: collision.clone(),
            paper_thickness_bits: analysis.paper_thickness_bits,
            material_faces: analysis.material_faces,
            cells: analysis.cells,
        }),
    })
}

/// Reconstructs the complete admitted cell from immutable native geometry and
/// compares every face, world coordinate bit pattern, and local layer record
/// with an existing opaque proof.
pub fn revalidate_single_face_cell_order_transport_v1(
    proof: &NativeCellOrderTransportProofV1,
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    collision: &NativeStaticCollisionGeometryProof,
    limits: CellOrderTransportLimitsV1,
) -> Result<(), CellOrderTransportErrorV1> {
    if !proof.is_for_geometry_and_collision(model, pose, collision) {
        return Err(CellOrderTransportErrorV1::ProofBindingMismatch);
    }
    let analysis = analyze_single_face_cell_order(model, pose, collision, limits)?;
    if proof.paper_thickness_bits() != analysis.paper_thickness_bits
        || proof.material_faces() != analysis.material_faces
        || !cells_bit_exact_equal(proof.cells(), &analysis.cells)
    {
        return Err(CellOrderTransportErrorV1::CertificateReverificationFailed);
    }
    Ok(())
}

fn analyze_single_face_cell_order(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    collision: &NativeStaticCollisionGeometryProof,
    limits: CellOrderTransportLimitsV1,
) -> Result<SingleFaceCellOrderAnalysis, CellOrderTransportErrorV1> {
    let bound = model
        .bind_pose(pose)
        .map_err(|_| CellOrderTransportErrorV1::PoseIssuerMismatch)?;
    let face_count = pose.face_ids().len();
    let hinge_count = pose.hinges().len();
    check_limit(
        CellOrderTransportResourceV1::Faces,
        face_count,
        limits.max_faces,
    )?;
    check_limit(
        CellOrderTransportResourceV1::Hinges,
        hinge_count,
        limits.max_hinges,
    )?;
    if face_count != 1 || hinge_count != 0 {
        return Err(CellOrderTransportErrorV1::UnsupportedPoseClass {
            faces: face_count,
            hinges: hinge_count,
        });
    }
    if pose.face_ids() != model.face_ids()
        || !pose.hinge_angles().is_empty()
        || pose.fixed_face().is_some()
    {
        return Err(CellOrderTransportErrorV1::InconsistentMaterialPose);
    }

    let thickness = collision.paper_thickness_mm();
    if !thickness.is_finite()
        || thickness < 0.0
        || !collision.is_for_geometry(model, pose, thickness)
        || collision.proof_id() != NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1
        || collision.policy_id() != TOPOLOGY_CONTACT_POLICY_V2
        || collision.kinematics_model_id() != MATERIAL_TREE_KINEMATICS_MODEL_ID
        || collision.thickness_model_id() != CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
        || collision.face_count() != 1
        || collision.expected_unordered_face_pairs() != 0
        || collision.analyzed_unordered_face_pairs() != 0
        || collision.expected_triangle_pairs() != 0
        || collision.analyzed_triangle_pairs() != 0
    {
        return Err(CellOrderTransportErrorV1::CollisionProofMismatch);
    }

    check_limit(CellOrderTransportResourceV1::Cells, 1, limits.max_cells)?;
    let face = pose.face_ids()[0];
    let boundary = bound
        .face_boundary(face)
        .ok_or(CellOrderTransportErrorV1::InconsistentMaterialPose)?;
    if boundary.face() != face
        || boundary.vertices().len() != boundary.edges().len()
        || boundary.vertices().len() < 3
    {
        return Err(CellOrderTransportErrorV1::InconsistentMaterialPose);
    }
    let boundary_count = boundary.vertices().len();
    check_limit(
        CellOrderTransportResourceV1::BoundaryVerticesPerCell,
        boundary_count,
        limits.max_boundary_vertices_per_cell,
    )?;
    check_limit(
        CellOrderTransportResourceV1::TotalBoundaryVertices,
        boundary_count,
        limits.max_total_boundary_vertices,
    )?;
    check_limit(
        CellOrderTransportResourceV1::LayerRecords,
        1,
        limits.max_total_layer_records,
    )?;

    let transform = pose
        .face_transform(face)
        .ok_or(CellOrderTransportErrorV1::InconsistentMaterialPose)?;
    if transform != model.identity_transform() {
        return Err(CellOrderTransportErrorV1::InconsistentMaterialPose);
    }
    let mut world_boundary = Vec::new();
    world_boundary
        .try_reserve_exact(boundary_count)
        .map_err(|_| CellOrderTransportErrorV1::AllocationFailed {
            resource: CellOrderTransportResourceV1::WorldBoundaryStorage,
        })?;
    let mut unique_world_points = HashSet::new();
    unique_world_points
        .try_reserve(boundary_count)
        .map_err(|_| CellOrderTransportErrorV1::AllocationFailed {
            resource: CellOrderTransportResourceV1::WorldBoundaryStorage,
        })?;
    for vertex in boundary.vertices() {
        let source = model
            .vertex_position(*vertex)
            .ok_or(CellOrderTransportErrorV1::InconsistentMaterialPose)?;
        if pose.vertex_position(*vertex) != Some(source) {
            return Err(CellOrderTransportErrorV1::InconsistentMaterialPose);
        }
        let world = transform
            .apply_point(source)
            .map_err(|_| CellOrderTransportErrorV1::WorldGeometryUnavailable)?;
        if !unique_world_points.insert(point_bits(world)) {
            return Err(CellOrderTransportErrorV1::WorldBoundaryDegenerate);
        }
        world_boundary.push(world);
    }
    if unique_world_points.len() != boundary_count {
        return Err(CellOrderTransportErrorV1::WorldBoundaryDegenerate);
    }

    let mut bottom_to_top_faces = Vec::new();
    bottom_to_top_faces.try_reserve_exact(1).map_err(|_| {
        CellOrderTransportErrorV1::AllocationFailed {
            resource: CellOrderTransportResourceV1::LayerRecordStorage,
        }
    })?;
    bottom_to_top_faces.push(face);
    let mut material_faces = Vec::new();
    material_faces.try_reserve_exact(1).map_err(|_| {
        CellOrderTransportErrorV1::AllocationFailed {
            resource: CellOrderTransportResourceV1::LayerRecordStorage,
        }
    })?;
    material_faces.push(face);
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(1)
        .map_err(|_| CellOrderTransportErrorV1::AllocationFailed {
            resource: CellOrderTransportResourceV1::LayerRecordStorage,
        })?;
    cells.push(CurrentPoseLayerCellV1 {
        cell_key: CurrentPoseCellKeyV1(face.canonical_bytes()),
        cell_face: face,
        world_boundary,
        bottom_to_top_faces,
    });

    Ok(SingleFaceCellOrderAnalysis {
        paper_thickness_bits: thickness.to_bits(),
        material_faces,
        cells,
    })
}

fn check_limit(
    resource: CellOrderTransportResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), CellOrderTransportErrorV1> {
    if actual > maximum {
        Err(CellOrderTransportErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn cells_bit_exact_equal(
    first: &[CurrentPoseLayerCellV1],
    second: &[CurrentPoseLayerCellV1],
) -> bool {
    first.len() == second.len()
        && first.iter().zip(second).all(|(first, second)| {
            first.cell_key == second.cell_key
                && first.cell_face == second.cell_face
                && first.bottom_to_top_faces == second.bottom_to_top_faces
                && first.world_boundary.len() == second.world_boundary.len()
                && first
                    .world_boundary
                    .iter()
                    .zip(&second.world_boundary)
                    .all(|(first, second)| point_bits(*first) == point_bits(*second))
        })
}

fn point_bits(point: Point3) -> [u64; 3] {
    [
        point.x().to_bits(),
        point.y().to_bits(),
        point.z().to_bits(),
    ]
}
