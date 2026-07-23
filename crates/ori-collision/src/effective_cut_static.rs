//! Fail-closed bridge from authenticated effective-cut kinematics to future
//! static positive-thickness collision analysis.
//!
//! This module binds prerequisites only. It neither reconstructs the opaque
//! kinematics geometry nor claims that any face pair is collision-free.

use ori_kinematics::{
    EffectiveCutKinematicsDiagnosticV1, EffectiveCutRetainedFacePairRegistryLimitsV1,
    EffectiveCutRetainedFacePairRegistryV1, TreeKinematicsLimits,
};
use ori_topology::{EffectiveCutMaterialSnapshotDiagnosticV1, FaceExtractionInput};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use thiserror::Error;

pub const EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1: &str =
    "effective_cut_static_thickness_prerequisite_v1";
pub const EFFECTIVE_CUT_STATIC_PAIR_REGISTRY_BRIDGE_MODEL_ID_V1: &str =
    "effective_cut_static_pair_registry_bridge_v1";
pub const EFFECTIVE_CUT_COLLISION_GEOMETRY_MODEL_ID_V1: &str =
    "effective_cut_collision_geometry_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EffectiveCutStaticThicknessPrerequisiteErrorV1 {
    #[error("effective-cut kinematics binding is stale, foreign, or unsupported")]
    InvalidBinding,
    #[error("paper thickness must be finite and strictly positive")]
    InvalidThickness,
    #[error("static face-pair work exceeds the configured resource limit")]
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveCutStaticThicknessLimitsV1 {
    pub max_face_pairs: usize,
}

impl Default for EffectiveCutStaticThicknessLimitsV1 {
    fn default() -> Self {
        Self {
            max_face_pairs: 1_000_000,
        }
    }
}

/// Opaque, non-authoritative prerequisite for future static collision work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCutStaticThicknessPrerequisiteDiagnosticV1 {
    kinematics_fingerprint: [u8; 32],
    fingerprint: [u8; 32],
    thickness_bits: u64,
    face_count: usize,
    hinge_count: usize,
    pair_count: usize,
    kinematics_limits: TreeKinematicsLimits,
    limits: EffectiveCutStaticThicknessLimitsV1,
}

impl EffectiveCutStaticThicknessPrerequisiteDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1
    }
    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }
    #[must_use]
    pub const fn face_count(&self) -> usize {
        self.face_count
    }
    #[must_use]
    pub const fn hinge_count(&self) -> usize {
        self.hinge_count
    }
    #[must_use]
    /// Planned unordered-pair work cardinality. This is not a pair-evidence
    /// registry and proves neither pair coverage nor separation.
    pub const fn planned_unordered_face_pair_count(&self) -> usize {
        self.pair_count
    }
    #[must_use]
    pub const fn observes_source_flat_convention_only(&self) -> bool {
        true
    }
    #[must_use]
    pub const fn paper_thickness_mm(&self) -> f64 {
        f64::from_bits(self.thickness_bits)
    }
    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        kinematics: &EffectiveCutKinematicsDiagnosticV1,
        effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
        input: FaceExtractionInput<'_>,
        kinematics_limits: TreeKinematicsLimits,
        limits: EffectiveCutStaticThicknessLimitsV1,
    ) -> bool {
        self.limits == limits
            && self.kinematics_limits == kinematics_limits
            && self.kinematics_fingerprint == kinematics.fingerprint_v1()
            && input.paper.thickness_mm.to_bits() == self.thickness_bits
            && prepare_effective_cut_static_thickness_prerequisite_v1(
                kinematics,
                effective,
                input,
                kinematics_limits,
                limits,
            )
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

/// Opaque binding of a static-thickness prerequisite to the complete retained
/// face-pair registry. It performs no SAT or pair classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCutStaticPairRegistryBridgeV1 {
    prerequisite_fingerprint: [u8; 32],
    registry_fingerprint: [u8; 32],
    fingerprint: [u8; 32],
    pair_count: usize,
    kinematics_limits: TreeKinematicsLimits,
    prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
    registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveCutCollisionGeometryLimitsV1 {
    pub max_faces: usize,
    pub max_boundary_vertices: usize,
    pub max_hinge_memberships: usize,
}

impl Default for EffectiveCutCollisionGeometryLimitsV1 {
    fn default() -> Self {
        Self {
            max_faces: 10_000,
            max_boundary_vertices: 1_000_000,
            max_hinge_memberships: ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP,
        }
    }
}

#[derive(Debug, Clone)]
struct EffectiveCutCollisionGeometryDataV1 {
    faces: Vec<EffectiveCutCollisionFaceV1>,
    hinges: Vec<EffectiveCutCollisionHingeV1>,
}

#[derive(Debug, Clone)]
struct EffectiveCutCollisionFaceV1 {
    face: ori_domain::FaceId,
    boundary: Vec<EffectiveCutCollisionBoundaryOccurrenceV1>,
}

#[derive(Debug, Clone, Copy)]
struct EffectiveCutCollisionBoundaryOccurrenceV1 {
    edge: ori_domain::EdgeId,
    origin: ori_domain::VertexId,
    destination: ori_domain::VertexId,
    point: [f64; 3],
    converted_cut_boundary: bool,
}

#[derive(Debug, Clone, Copy)]
struct EffectiveCutCollisionHingeV1 {
    edge: ori_domain::EdgeId,
    first: ori_domain::FaceId,
    second: ori_domain::FaceId,
    start_vertex: ori_domain::VertexId,
    end_vertex: ori_domain::VertexId,
    start: [f64; 3],
    end: [f64; 3],
    assignment: ori_topology::FoldAssignment,
}

/// Opaque source-flat geometry retained inside the collision crate.
///
/// There is intentionally no boundary, point, hinge, transform, graph, or
/// pose accessor. Only collision-crate internals may later feed these records
/// to an authenticated closed-prism kernel.
#[derive(Clone)]
pub struct EffectiveCutCollisionGeometryV1 {
    data: Arc<EffectiveCutCollisionGeometryDataV1>,
    bridge_fingerprint: [u8; 32],
    fingerprint: [u8; 32],
    boundary_occurrence_count: usize,
    converted_cut_boundary_occurrence_count: usize,
    limits: EffectiveCutCollisionGeometryLimitsV1,
}

impl std::fmt::Debug for EffectiveCutCollisionGeometryV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EffectiveCutCollisionGeometryV1")
            .field("model_id", &self.model_id())
            .field("fingerprint", &self.fingerprint)
            .field("face_count", &self.face_count())
            .field("hinge_membership_count", &self.hinge_membership_count())
            .finish_non_exhaustive()
    }
}

impl EffectiveCutCollisionGeometryV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EFFECTIVE_CUT_COLLISION_GEOMETRY_MODEL_ID_V1
    }
    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.data.faces.len()
    }
    #[must_use]
    pub fn hinge_membership_count(&self) -> usize {
        self.data.hinges.len()
    }
    #[must_use]
    pub const fn boundary_occurrence_count(&self) -> usize {
        self.boundary_occurrence_count
    }
    #[must_use]
    pub const fn converted_cut_boundary_occurrence_count(&self) -> usize {
        self.converted_cut_boundary_occurrence_count
    }
    #[must_use]
    pub const fn observes_source_flat_identity_only(&self) -> bool {
        true
    }
    #[must_use]
    pub const fn authorizes_pair_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_pose_solving(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(&self, input: EffectiveCutCollisionGeometryInputV1<'_>) -> bool {
        self.bridge_fingerprint == input.bridge.fingerprint_v1()
            && self.limits == input.geometry_limits
            && prepare_effective_cut_collision_geometry_v1(input)
                .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EffectiveCutCollisionGeometryInputV1<'a> {
    pub bridge: &'a EffectiveCutStaticPairRegistryBridgeV1,
    pub prerequisite: &'a EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
    pub registry: &'a EffectiveCutRetainedFacePairRegistryV1,
    pub kinematics: &'a EffectiveCutKinematicsDiagnosticV1,
    pub effective: &'a EffectiveCutMaterialSnapshotDiagnosticV1,
    pub source: FaceExtractionInput<'a>,
    pub kinematics_limits: TreeKinematicsLimits,
    pub prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
    pub registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
    pub geometry_limits: EffectiveCutCollisionGeometryLimitsV1,
}

pub fn prepare_effective_cut_collision_geometry_v1(
    input: EffectiveCutCollisionGeometryInputV1<'_>,
) -> Result<EffectiveCutCollisionGeometryV1, EffectiveCutStaticThicknessPrerequisiteErrorV1> {
    if input.geometry_limits.max_faces > 10_000
        || input.geometry_limits.max_boundary_vertices > 1_000_000
        || input.geometry_limits.max_hinge_memberships > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
    {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit);
    }
    let snapshot = input.effective.snapshot();
    if snapshot.faces.len() > input.geometry_limits.max_faces {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit);
    }
    if snapshot
        .faces
        .iter()
        .any(|face| !face.holes.is_empty() || !face.seams.is_empty())
        || snapshot.faces.len() != input.kinematics.face_count()
        || snapshot
            .faces
            .windows(2)
            .any(|pair| pair[0].id.canonical_bytes() >= pair[1].id.canonical_bytes())
    {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let mut boundary_vertex_count = 0_usize;
    for face in &snapshot.faces {
        boundary_vertex_count = boundary_vertex_count
            .checked_add(face.outer.half_edges.len())
            .filter(|count| *count <= input.geometry_limits.max_boundary_vertices)
            .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    }
    if snapshot.hinge_adjacency.len() > input.geometry_limits.max_hinge_memberships {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit);
    }
    if snapshot.hinge_adjacency.len() != input.kinematics.hinge_count()
        || snapshot.hinge_adjacency.len() != input.registry.shared_hinge_membership_count()
    {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    if !input.bridge.is_for(
        input.prerequisite,
        input.registry,
        input.kinematics,
        input.effective,
        input.source,
        input.kinematics_limits,
        input.prerequisite_limits,
        input.registry_limits,
    ) {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let mut converted = HashSet::new();
    converted
        .try_reserve(input.effective.converted_crossing_cut_boundaries().len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    for edge in input.effective.converted_crossing_cut_boundaries() {
        if !converted.insert(*edge) {
            return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
        }
    }
    let mut effective_boundaries = HashSet::new();
    effective_boundaries
        .try_reserve(snapshot.edge_incidence.len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    for (edge, incidence) in &snapshot.edge_incidence {
        if matches!(incidence, ori_topology::EdgeIncidence::Boundary { .. }) {
            effective_boundaries.insert(*edge);
        }
    }
    let mut consumed_converted = HashSet::new();
    consumed_converted
        .try_reserve(converted.len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    let mut source_positions = HashMap::new();
    source_positions
        .try_reserve(input.source.pattern.vertices.len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    for vertex in &input.source.pattern.vertices {
        if source_positions
            .insert(vertex.id, vertex.position)
            .is_some()
            || !vertex.position.x.is_finite()
            || !vertex.position.y.is_finite()
        {
            return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
        }
    }
    let mut source_edges = HashMap::new();
    source_edges
        .try_reserve(input.source.pattern.edges.len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    for edge in &input.source.pattern.edges {
        if source_edges.insert(edge.id, edge).is_some() {
            return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
        }
    }
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(snapshot.faces.len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    for face in &snapshot.faces {
        let mut boundary = Vec::new();
        boundary
            .try_reserve_exact(face.outer.half_edges.len())
            .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
        for half_edge in &face.outer.half_edges {
            let edge = source_edges
                .get(&half_edge.edge)
                .copied()
                .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding)?;
            if !((edge.start == half_edge.origin && edge.end == half_edge.destination)
                || (edge.end == half_edge.origin && edge.start == half_edge.destination))
            {
                return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
            }
            let point = source_positions
                .get(&half_edge.origin)
                .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding)?;
            let converted_cut_boundary = edge.kind == ori_domain::EdgeKind::Cut;
            if converted_cut_boundary != converted.contains(&half_edge.edge)
                || (converted_cut_boundary
                    && (!effective_boundaries.contains(&half_edge.edge)
                        || !consumed_converted.insert(half_edge.edge)))
            {
                return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
            }
            boundary.push(EffectiveCutCollisionBoundaryOccurrenceV1 {
                edge: half_edge.edge,
                origin: half_edge.origin,
                destination: half_edge.destination,
                point: source_flat_point_v1(*point),
                converted_cut_boundary,
            });
        }
        faces.push(EffectiveCutCollisionFaceV1 {
            face: face.id,
            boundary,
        });
    }
    if consumed_converted != converted {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let mut hinges = Vec::new();
    hinges
        .try_reserve_exact(snapshot.hinge_adjacency.len())
        .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    for adjacent in &snapshot.hinge_adjacency {
        let (first, second) =
            if adjacent.first.canonical_bytes() < adjacent.second.canonical_bytes() {
                (adjacent.first, adjacent.second)
            } else {
                (adjacent.second, adjacent.first)
            };
        let edge = source_edges
            .get(&adjacent.edge)
            .copied()
            .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding)?;
        let expected_kind = match adjacent.assignment {
            ori_topology::FoldAssignment::Mountain => ori_domain::EdgeKind::Mountain,
            ori_topology::FoldAssignment::Valley => ori_domain::EdgeKind::Valley,
        };
        if first == second || edge.kind != expected_kind {
            return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
        }
        let (start_vertex, end_vertex) =
            if edge.start.canonical_bytes() < edge.end.canonical_bytes() {
                (edge.start, edge.end)
            } else {
                (edge.end, edge.start)
            };
        let start = source_positions
            .get(&start_vertex)
            .copied()
            .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding)?;
        let end = source_positions
            .get(&end_vertex)
            .copied()
            .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding)?;
        hinges.push(EffectiveCutCollisionHingeV1 {
            edge: adjacent.edge,
            first,
            second,
            start_vertex,
            end_vertex,
            start: source_flat_point_v1(start),
            end: source_flat_point_v1(end),
            assignment: adjacent.assignment,
        });
    }
    hinges.sort_unstable_by_key(|hinge| {
        (
            hinge.first.canonical_bytes(),
            hinge.second.canonical_bytes(),
            hinge.edge.canonical_bytes(),
        )
    });
    if hinges.windows(2).any(|pair| {
        pair[0].edge == pair[1].edge
            || (
                pair[0].first.canonical_bytes(),
                pair[0].second.canonical_bytes(),
                pair[0].edge.canonical_bytes(),
            ) >= (
                pair[1].first.canonical_bytes(),
                pair[1].second.canonical_bytes(),
                pair[1].edge.canonical_bytes(),
            )
    }) {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let data = Arc::new(EffectiveCutCollisionGeometryDataV1 { faces, hinges });
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_COLLISION_GEOMETRY_MODEL_ID_V1.as_bytes());
    hash.update(input.bridge.fingerprint_v1());
    hash.update(input.effective.fingerprint_v1());
    hash.update((data.faces.len() as u64).to_be_bytes());
    hash.update((data.hinges.len() as u64).to_be_bytes());
    hash.update((boundary_vertex_count as u64).to_be_bytes());
    for face in &data.faces {
        hash.update(face.face.canonical_bytes());
        hash.update((face.boundary.len() as u64).to_be_bytes());
        for occurrence in &face.boundary {
            hash.update(occurrence.edge.canonical_bytes());
            hash.update(occurrence.origin.canonical_bytes());
            hash.update(occurrence.destination.canonical_bytes());
            hash.update([u8::from(occurrence.converted_cut_boundary)]);
            for coordinate in occurrence.point {
                hash.update(coordinate.to_bits().to_be_bytes());
            }
        }
    }
    for hinge in &data.hinges {
        hash.update(hinge.first.canonical_bytes());
        hash.update(hinge.second.canonical_bytes());
        hash.update(hinge.edge.canonical_bytes());
        hash.update(hinge.start_vertex.canonical_bytes());
        hash.update(hinge.end_vertex.canonical_bytes());
        for point in [hinge.start, hinge.end] {
            for coordinate in point {
                hash.update(coordinate.to_bits().to_be_bytes());
            }
        }
        hash.update([match hinge.assignment {
            ori_topology::FoldAssignment::Mountain => 0x4d,
            ori_topology::FoldAssignment::Valley => 0x56,
        }]);
    }
    for value in [
        input.geometry_limits.max_faces,
        input.geometry_limits.max_boundary_vertices,
        input.geometry_limits.max_hinge_memberships,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
    Ok(EffectiveCutCollisionGeometryV1 {
        data,
        bridge_fingerprint: input.bridge.fingerprint_v1(),
        fingerprint: hash.finalize().into(),
        boundary_occurrence_count: boundary_vertex_count,
        converted_cut_boundary_occurrence_count: consumed_converted.len(),
        limits: input.geometry_limits,
    })
}

fn source_flat_point_v1(point: ori_domain::Point2) -> [f64; 3] {
    let x = if point.x == 0.0 { 0.0 } else { point.x };
    let z = if point.y == 0.0 { 0.0 } else { -point.y };
    [x, 0.0, z]
}

impl EffectiveCutStaticPairRegistryBridgeV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EFFECTIVE_CUT_STATIC_PAIR_REGISTRY_BRIDGE_MODEL_ID_V1
    }
    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }
    #[must_use]
    pub const fn pair_count(&self) -> usize {
        self.pair_count
    }
    #[must_use]
    pub const fn authorizes_pair_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn is_for(
        &self,
        prerequisite: &EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
        registry: &EffectiveCutRetainedFacePairRegistryV1,
        kinematics: &EffectiveCutKinematicsDiagnosticV1,
        effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
        input: FaceExtractionInput<'_>,
        kinematics_limits: TreeKinematicsLimits,
        prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
        registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
    ) -> bool {
        self.prerequisite_fingerprint == prerequisite.fingerprint_v1()
            && self.registry_fingerprint == registry.fingerprint_v1()
            && self.kinematics_limits == kinematics_limits
            && self.prerequisite_limits == prerequisite_limits
            && self.registry_limits == registry_limits
            && prepare_effective_cut_static_pair_registry_bridge_v1(
                prerequisite,
                registry,
                kinematics,
                effective,
                input,
                kinematics_limits,
                prerequisite_limits,
                registry_limits,
            )
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_effective_cut_static_pair_registry_bridge_v1(
    prerequisite: &EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
    registry: &EffectiveCutRetainedFacePairRegistryV1,
    kinematics: &EffectiveCutKinematicsDiagnosticV1,
    effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
    input: FaceExtractionInput<'_>,
    kinematics_limits: TreeKinematicsLimits,
    prerequisite_limits: EffectiveCutStaticThicknessLimitsV1,
    registry_limits: EffectiveCutRetainedFacePairRegistryLimitsV1,
) -> Result<EffectiveCutStaticPairRegistryBridgeV1, EffectiveCutStaticThicknessPrerequisiteErrorV1>
{
    if prerequisite_limits.max_face_pairs != registry_limits.max_pairs
        || prerequisite.kinematics_fingerprint != kinematics.fingerprint_v1()
        || prerequisite.kinematics_limits != kinematics_limits
        || prerequisite.limits != prerequisite_limits
        || prerequisite.thickness_bits != input.paper.thickness_mm.to_bits()
        || prerequisite.face_count != kinematics.face_count()
        || prerequisite.hinge_count != kinematics.hinge_count()
        || prerequisite.planned_unordered_face_pair_count() != registry.pair_count()
        || prerequisite.fingerprint
            != static_prerequisite_fingerprint_v1(
                kinematics,
                input.paper.thickness_mm,
                prerequisite.planned_unordered_face_pair_count(),
                prerequisite_limits,
            )
        || !registry.is_for(
            kinematics,
            effective,
            input,
            kinematics_limits,
            registry_limits,
        )
    {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let pair_count = registry.pair_count();
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_STATIC_PAIR_REGISTRY_BRIDGE_MODEL_ID_V1.as_bytes());
    hash.update(prerequisite.fingerprint_v1());
    hash.update(registry.fingerprint_v1());
    hash.update(kinematics.fingerprint_v1());
    hash.update(effective.fingerprint_v1());
    hash.update(prerequisite.thickness_bits.to_be_bytes());
    hash.update((prerequisite.face_count as u64).to_be_bytes());
    hash.update((prerequisite.hinge_count as u64).to_be_bytes());
    hash.update((pair_count as u64).to_be_bytes());
    hash.update((registry.shared_hinge_membership_count() as u64).to_be_bytes());
    for value in [
        kinematics_limits.max_source_vertices,
        kinematics_limits.max_source_edges,
        kinematics_limits.max_paper_boundary_vertices,
        kinematics_limits.max_faces,
        kinematics_limits.max_edge_incidences,
        kinematics_limits.max_hinges,
        kinematics_limits.max_face_boundary_vertices,
        kinematics_limits.max_adjacency_entries,
        prerequisite_limits.max_face_pairs,
        registry_limits.max_pairs,
        registry_limits.max_shared_hinge_memberships,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
    Ok(EffectiveCutStaticPairRegistryBridgeV1 {
        prerequisite_fingerprint: prerequisite.fingerprint_v1(),
        registry_fingerprint: registry.fingerprint_v1(),
        fingerprint: hash.finalize().into(),
        pair_count,
        kinematics_limits,
        prerequisite_limits,
        registry_limits,
    })
}

pub fn prepare_effective_cut_static_thickness_prerequisite_v1(
    kinematics: &EffectiveCutKinematicsDiagnosticV1,
    effective: &EffectiveCutMaterialSnapshotDiagnosticV1,
    input: FaceExtractionInput<'_>,
    kinematics_limits: TreeKinematicsLimits,
    limits: EffectiveCutStaticThicknessLimitsV1,
) -> Result<
    EffectiveCutStaticThicknessPrerequisiteDiagnosticV1,
    EffectiveCutStaticThicknessPrerequisiteErrorV1,
> {
    let thickness = input.paper.thickness_mm;
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidThickness);
    }
    if !kinematics.is_for(effective, input, kinematics_limits) {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let pair_count = kinematics
        .face_count()
        .checked_sub(1)
        .and_then(|less| kinematics.face_count().checked_mul(less))
        .and_then(|twice| twice.checked_div(2))
        .filter(|count| *count <= limits.max_face_pairs)
        .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    let fingerprint = static_prerequisite_fingerprint_v1(kinematics, thickness, pair_count, limits);
    Ok(EffectiveCutStaticThicknessPrerequisiteDiagnosticV1 {
        kinematics_fingerprint: kinematics.fingerprint_v1(),
        fingerprint,
        thickness_bits: thickness.to_bits(),
        face_count: kinematics.face_count(),
        hinge_count: kinematics.hinge_count(),
        pair_count,
        kinematics_limits,
        limits,
    })
}

fn static_prerequisite_fingerprint_v1(
    kinematics: &EffectiveCutKinematicsDiagnosticV1,
    thickness: f64,
    pair_count: usize,
    limits: EffectiveCutStaticThicknessLimitsV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_STATIC_THICKNESS_PREREQUISITE_MODEL_ID_V1.as_bytes());
    hash.update(kinematics.fingerprint_v1());
    hash.update(thickness.to_bits().to_be_bytes());
    hash.update((kinematics.face_count() as u64).to_be_bytes());
    hash.update((kinematics.hinge_count() as u64).to_be_bytes());
    hash.update((pair_count as u64).to_be_bytes());
    hash.update((limits.max_face_pairs as u64).to_be_bytes());
    hash.finalize().into()
}
