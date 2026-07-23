//! Fail-closed bridge from authenticated effective-cut kinematics to future
//! static positive-thickness collision analysis.
//!
//! This module binds prerequisites only. It neither reconstructs the opaque
//! kinematics geometry nor claims that any face pair is collision-free.

use crate::cayley::{
    PositiveThicknessPrismPairDispositionV1, SourceFlatPrismFeatureV1,
    diagnose_source_flat_prism_pair_v1,
};
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
pub const EFFECTIVE_CUT_SOURCE_FLAT_PAIR_OBSERVATION_MODEL_ID_V1: &str =
    "effective_cut_source_flat_pair_observation_v1";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveCutSourceFlatPairObservationLimitsV1 {
    pub max_pairs: usize,
    pub max_shared_vertex_work: usize,
}

impl Default for EffectiveCutSourceFlatPairObservationLimitsV1 {
    fn default() -> Self {
        Self {
            max_pairs: 50_000,
            max_shared_vertex_work: 10_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCutSourceFlatPairObservationV1 {
    geometry_fingerprint: [u8; 32],
    fingerprint: [u8; 32],
    pair_count: usize,
    separated: usize,
    touching: usize,
    shared_hinge_allowed: usize,
    shared_vertex_allowed: usize,
    penetrating: usize,
    indeterminate: usize,
    limits: EffectiveCutSourceFlatPairObservationLimitsV1,
}

impl EffectiveCutSourceFlatPairObservationV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EFFECTIVE_CUT_SOURCE_FLAT_PAIR_OBSERVATION_MODEL_ID_V1
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
    pub const fn separated_pairs(&self) -> usize {
        self.separated
    }
    #[must_use]
    pub const fn touching_pairs(&self) -> usize {
        self.touching
    }
    #[must_use]
    pub const fn shared_hinge_allowed_pairs(&self) -> usize {
        self.shared_hinge_allowed
    }
    #[must_use]
    pub const fn shared_vertex_allowed_pairs(&self) -> usize {
        self.shared_vertex_allowed
    }
    #[must_use]
    pub const fn penetrating_pairs(&self) -> usize {
        self.penetrating
    }
    #[must_use]
    pub const fn indeterminate_pairs(&self) -> usize {
        self.indeterminate
    }
    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_pair_classification(&self) -> bool {
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
    pub fn is_for(
        &self,
        geometry: &EffectiveCutCollisionGeometryV1,
        input: EffectiveCutCollisionGeometryInputV1<'_>,
        limits: EffectiveCutSourceFlatPairObservationLimitsV1,
    ) -> bool {
        self.geometry_fingerprint == geometry.fingerprint_v1()
            && self.limits == limits
            && diagnose_effective_cut_source_flat_pairs_v1(geometry, input, limits)
                .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
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
    canonical_vertices: Vec<(ori_domain::VertexId, [f64; 3])>,
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
        let mut canonical_vertices = Vec::new();
        canonical_vertices
            .try_reserve_exact(boundary.len())
            .map_err(|_| EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
        for occurrence in &boundary {
            canonical_vertices.push((occurrence.origin, occurrence.point));
        }
        canonical_vertices.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
        if canonical_vertices
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
        }
        faces.push(EffectiveCutCollisionFaceV1 {
            face: face.id,
            boundary,
            canonical_vertices,
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

pub fn diagnose_effective_cut_source_flat_pairs_v1(
    geometry: &EffectiveCutCollisionGeometryV1,
    input: EffectiveCutCollisionGeometryInputV1<'_>,
    limits: EffectiveCutSourceFlatPairObservationLimitsV1,
) -> Result<EffectiveCutSourceFlatPairObservationV1, EffectiveCutStaticThicknessPrerequisiteErrorV1>
{
    if limits.max_pairs > 50_000 || limits.max_shared_vertex_work > 10_000_000 {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit);
    }
    let pair_count = geometry
        .face_count()
        .checked_mul(geometry.face_count().saturating_sub(1))
        .and_then(|count| count.checked_div(2))
        .filter(|count| *count <= limits.max_pairs && *count <= 50_000)
        .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    let expected_shared_vertex_work = geometry
        .boundary_occurrence_count()
        .checked_mul(geometry.face_count().saturating_sub(1))
        .filter(|work| *work <= limits.max_shared_vertex_work)
        .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
    if !geometry.is_for(input) || pair_count != input.registry.pair_count() {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    let mut counts = [0_usize; 6];
    let mut analyzed = 0_usize;
    let mut hinge_index = 0_usize;
    let mut shared_vertex_work = 0_usize;
    let mut hash = Sha256::new();
    hash.update(EFFECTIVE_CUT_SOURCE_FLAT_PAIR_OBSERVATION_MODEL_ID_V1.as_bytes());
    hash.update(geometry.fingerprint_v1());
    hash.update((limits.max_pairs as u64).to_be_bytes());
    hash.update((limits.max_shared_vertex_work as u64).to_be_bytes());
    for first_index in 0..geometry.data.faces.len() {
        for second_index in first_index + 1..geometry.data.faces.len() {
            analyzed = analyzed
                .checked_add(1)
                .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
            let first = &geometry.data.faces[first_index];
            let second = &geometry.data.faces[second_index];
            let hinge_start = hinge_index;
            while geometry
                .data
                .hinges
                .get(hinge_index)
                .is_some_and(|hinge| hinge.first == first.face && hinge.second == second.face)
            {
                hinge_index += 1;
            }
            let shared_hinges = &geometry.data.hinges[hinge_start..hinge_index];
            shared_vertex_work = shared_vertex_work
                .checked_add(first.canonical_vertices.len())
                .and_then(|work| work.checked_add(second.canonical_vertices.len()))
                .filter(|work| *work <= limits.max_shared_vertex_work)
                .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit)?;
            let mut left = 0_usize;
            let mut right = 0_usize;
            let mut shared = [None; 3];
            let mut shared_count = 0_usize;
            while left < first.canonical_vertices.len() && right < second.canonical_vertices.len() {
                match first.canonical_vertices[left]
                    .0
                    .canonical_bytes()
                    .cmp(&second.canonical_vertices[right].0.canonical_bytes())
                {
                    std::cmp::Ordering::Less => left += 1,
                    std::cmp::Ordering::Greater => right += 1,
                    std::cmp::Ordering::Equal => {
                        if shared_count < shared.len() {
                            shared[shared_count] = Some(first.canonical_vertices[left]);
                        }
                        shared_count += 1;
                        left += 1;
                        right += 1;
                    }
                }
            }
            let feature = if shared_hinges.len() > 1 || shared_count > 2 {
                SourceFlatPrismFeatureV1::Unsupported
            } else if let Some(hinge) = shared_hinges.first() {
                let endpoint_match = shared_count == 2
                    && shared[..2].iter().flatten().all(|(vertex, _)| {
                        *vertex == hinge.start_vertex || *vertex == hinge.end_vertex
                    });
                if endpoint_match {
                    SourceFlatPrismFeatureV1::SingleHinge([hinge.start, hinge.end])
                } else {
                    SourceFlatPrismFeatureV1::Unsupported
                }
            } else if shared_count == 1 {
                SourceFlatPrismFeatureV1::SingleVertex(
                    shared[0]
                        .ok_or(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding)?
                        .1,
                )
            } else if shared_count == 0 {
                SourceFlatPrismFeatureV1::None
            } else {
                SourceFlatPrismFeatureV1::Unsupported
            };
            let first_points = (first.boundary.len() == 3).then(|| {
                [
                    first.boundary[0].point,
                    first.boundary[1].point,
                    first.boundary[2].point,
                ]
            });
            let second_points = (second.boundary.len() == 3).then(|| {
                [
                    second.boundary[0].point,
                    second.boundary[1].point,
                    second.boundary[2].point,
                ]
            });
            let disposition = diagnose_source_flat_prism_pair_v1(
                first_points
                    .as_ref()
                    .map_or(&[], |points| points.as_slice()),
                second_points
                    .as_ref()
                    .map_or(&[], |points| points.as_slice()),
                input.prerequisite.paper_thickness_mm(),
                feature,
            )
            .map_err(|error| match error {
                crate::cayley::SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded => {
                    EffectiveCutStaticThicknessPrerequisiteErrorV1::ResourceLimit
                }
                crate::cayley::SharedHingeSolidDiagnosticErrorV1::InconsistentPose => {
                    EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding
                }
            })?;
            let index = match disposition {
                PositiveThicknessPrismPairDispositionV1::Separated => 0,
                PositiveThicknessPrismPairDispositionV1::Touching => 1,
                PositiveThicknessPrismPairDispositionV1::SharedHingeCorridorAllowed => 2,
                PositiveThicknessPrismPairDispositionV1::SharedVertexCorridorAllowed => 3,
                PositiveThicknessPrismPairDispositionV1::Penetrating => 4,
                PositiveThicknessPrismPairDispositionV1::Indeterminate => 5,
            };
            counts[index] += 1;
            hash.update(first.face.canonical_bytes());
            hash.update(second.face.canonical_bytes());
            hash.update([index as u8]);
        }
    }
    if analyzed != pair_count
        || counts.iter().sum::<usize>() != pair_count
        || hinge_index != geometry.data.hinges.len()
        || shared_vertex_work != expected_shared_vertex_work
    {
        return Err(EffectiveCutStaticThicknessPrerequisiteErrorV1::InvalidBinding);
    }
    for count in [
        pair_count, counts[0], counts[1], counts[2], counts[3], counts[4], counts[5],
    ] {
        hash.update((count as u64).to_be_bytes());
    }
    Ok(EffectiveCutSourceFlatPairObservationV1 {
        geometry_fingerprint: geometry.fingerprint_v1(),
        fingerprint: hash.finalize().into(),
        pair_count,
        separated: counts[0],
        touching: counts[1],
        shared_hinge_allowed: counts[2],
        shared_vertex_allowed: counts[3],
        penetrating: counts[4],
        indeterminate: counts[5],
        limits,
    })
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
