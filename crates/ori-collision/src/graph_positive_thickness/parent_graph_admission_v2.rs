//! Exact whole-parent planar-graph admission for the general-N path.
//!
//! This is deliberately separate from the V1 static thickness proof.  It
//! authenticates the rest-sheet embedding needed by a later whole-parent
//! positive-thickness theorem, but it is not itself collision-clearance or
//! motion authority.

use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::{FaceId, ProjectId, VertexId};
use ori_kinematics::{MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1, Point3};
use ori_topology::FoldAssignment;
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, mem::size_of};
use thiserror::Error;

mod exact_planarity;

use exact_planarity::*;

/// Stable domain identifier for exact general-N parent-graph admission.
pub const COMMON_ARTICULATION_POSITIVE_THICKNESS_PARENT_GRAPH_ADMISSION_MODEL_ID_V2: &str =
    "common_articulation_positive_thickness_parent_graph_admission_v2";

// An exact binary64 rational has a reduced numerator and denominator of at
// most 1,075 significant bits. Two BigInts plus allocator headers fit well
// below 1 KiB; 2 KiB per retained XZ point leaves more than a 2x margin.
const EXACT_VERTEX_DYNAMIC_BYTES_UPPER_BOUND_V2: usize = 2_048;
// The deepest predicate here holds fewer than 32 BigInts. Coordinate
// differences/products, a shoelace accumulator, and axis products remain
// below 4,400 + usize::BITS bits each for binary64 inputs. 64 KiB therefore
// bounds all simultaneously live exact-predicate payloads with wide margin.
const EXACT_PREDICATE_SCRATCH_BYTES_UPPER_BOUND_V2: usize = 65_536;

/// Finite resource envelope for one exact parent-graph admission run.
///
/// `usize::MAX` is rejected for every field: callers must state an actual
/// finite policy rather than smuggling an unbounded sentinel across this
/// proof boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_boundary_vertex_occurrences: usize,
    pub max_vertices: usize,
    pub max_edges: usize,
    pub max_vertex_pairs: usize,
    pub max_vertex_edge_tests: usize,
    pub max_edge_pair_tests: usize,
    pub max_face_pair_tests: usize,
    pub max_point_in_polygon_edge_tests: usize,
    pub max_exact_operations: usize,
    pub max_logical_work: usize,
    pub max_workspace_bytes: usize,
}

impl Default for CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
    fn default() -> Self {
        Self {
            max_faces: 2_049,
            max_hinges: 3_072,
            max_boundary_vertex_occurrences: 16_384,
            max_vertices: 8_192,
            max_edges: 16_384,
            max_vertex_pairs: 8_000_000,
            max_vertex_edge_tests: 16_000_000,
            max_edge_pair_tests: 16_000_000,
            max_face_pair_tests: 4_000_000,
            max_point_in_polygon_edge_tests: 64_000_000,
            max_exact_operations: 1_000_000_000,
            max_logical_work: 128_000_000,
            max_workspace_bytes: 256 * 1_024 * 1_024,
        }
    }
}

impl CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
    fn is_explicitly_finite_v2(self) -> bool {
        [
            self.max_faces,
            self.max_hinges,
            self.max_boundary_vertex_occurrences,
            self.max_vertices,
            self.max_edges,
            self.max_vertex_pairs,
            self.max_vertex_edge_tests,
            self.max_edge_pair_tests,
            self.max_face_pair_tests,
            self.max_point_in_polygon_edge_tests,
            self.max_exact_operations,
            self.max_logical_work,
            self.max_workspace_bytes,
        ]
        .into_iter()
        .all(|limit| limit != usize::MAX)
    }
}

/// Cooperative stop requested by a parent-graph admission operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationPositiveThicknessParentGraphAdmissionStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Failure while issuing or revalidating exact parent-graph admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2 {
    #[error("the parent material graph input or provenance is malformed")]
    InvalidInput,
    #[error("the parent material graph exceeds the submitted finite resource envelope")]
    ResourceLimit,
    #[error("the exact XZ parent embedding is not a planar material graph")]
    NonPlanarProjection,
    #[error("the retained parent-graph admission does not match the exact live geometry")]
    AdmissionBindingMismatch,
    #[error("the parent-graph admission operation was cancelled")]
    Cancelled,
    #[error("the parent-graph admission operation deadline elapsed")]
    DeadlineExceeded,
}

/// Exact observed resource counts for one successful admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2 {
    face_count: usize,
    hinge_count: usize,
    boundary_vertex_occurrences: usize,
    vertex_count: usize,
    edge_count: usize,
    vertex_pair_tests: usize,
    vertex_edge_tests: usize,
    edge_pair_tests: usize,
    face_pair_tests: usize,
    point_in_polygon_edge_tests: usize,
    exact_operations: usize,
    logical_work: usize,
    workspace_bytes_upper_bound: usize,
}

impl CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2 {
    #[must_use]
    pub const fn face_count_v2(self) -> usize {
        self.face_count
    }

    #[must_use]
    pub const fn hinge_count_v2(self) -> usize {
        self.hinge_count
    }

    #[must_use]
    pub const fn boundary_vertex_occurrences_v2(self) -> usize {
        self.boundary_vertex_occurrences
    }

    #[must_use]
    pub const fn vertex_count_v2(self) -> usize {
        self.vertex_count
    }

    #[must_use]
    pub const fn edge_count_v2(self) -> usize {
        self.edge_count
    }

    #[must_use]
    pub const fn vertex_pair_tests_v2(self) -> usize {
        self.vertex_pair_tests
    }

    #[must_use]
    pub const fn vertex_edge_tests_v2(self) -> usize {
        self.vertex_edge_tests
    }

    #[must_use]
    pub const fn edge_pair_tests_v2(self) -> usize {
        self.edge_pair_tests
    }

    #[must_use]
    pub const fn face_pair_tests_v2(self) -> usize {
        self.face_pair_tests
    }

    #[must_use]
    pub const fn point_in_polygon_edge_tests_v2(self) -> usize {
        self.point_in_polygon_edge_tests
    }

    #[must_use]
    pub const fn exact_operations_v2(self) -> usize {
        self.exact_operations
    }

    #[must_use]
    pub const fn logical_work_v2(self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn workspace_bytes_upper_bound_v2(self) -> usize {
        self.workspace_bytes_upper_bound
    }

    /// Returns the inclusive envelope matching this successful observation.
    #[must_use]
    pub const fn exact_limits_v2(
        self,
    ) -> CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_faces: self.face_count,
            max_hinges: self.hinge_count,
            max_boundary_vertex_occurrences: self.boundary_vertex_occurrences,
            max_vertices: self.vertex_count,
            max_edges: self.edge_count,
            max_vertex_pairs: self.vertex_pair_tests,
            max_vertex_edge_tests: self.vertex_edge_tests,
            max_edge_pair_tests: self.edge_pair_tests,
            max_face_pair_tests: self.face_pair_tests,
            max_point_in_polygon_edge_tests: self.point_in_polygon_edge_tests,
            max_exact_operations: self.exact_operations,
            max_logical_work: self.logical_work,
            max_workspace_bytes: self.workspace_bytes_upper_bound,
        }
    }
}

/// Sealed, process-local admission of one exact rest-sheet parent graph.
///
/// This type has no V1 conversion, persistence traits, clearance methods, or
/// project-mutation authority. Revalidation repeats the complete exact scan.
#[derive(Debug)]
pub struct CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
    geometry_instance: MaterialHingeGraphInstanceV1,
    identity_namespace: ProjectId,
    source_revision: u64,
    fold_model_fingerprint: [u8; 32],
    semantic_graph_digest: [u8; 32],
    binding_fingerprint: [u8; 32],
    limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    resources: CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2,
}

impl CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_POSITIVE_THICKNESS_PARENT_GRAPH_ADMISSION_MODEL_ID_V2
    }

    #[must_use]
    pub const fn identity_namespace_v2(&self) -> ProjectId {
        self.identity_namespace
    }

    #[must_use]
    pub const fn source_revision_v2(&self) -> u64 {
        self.source_revision
    }

    #[must_use]
    pub const fn fold_model_fingerprint_v2(&self) -> [u8; 32] {
        self.fold_model_fingerprint
    }

    #[must_use]
    pub const fn semantic_graph_digest_v2(&self) -> [u8; 32] {
        self.semantic_graph_digest
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn limits_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
        self.limits
    }

    #[must_use]
    pub const fn resources_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2 {
        self.resources
    }

    /// Returns whether this sealed admission was issued for this exact,
    /// process-local geometry instance.  The geometry is immutable after
    /// preparation, so descendants may use this cheap check after an outer
    /// boundary has completed the full exact revalidation pass.
    pub(crate) fn matches_geometry_instance_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
    ) -> bool {
        self.geometry_instance.matches(geometry)
    }

    /// Compares every retained semantic and resource field without exposing
    /// the private process-local geometry anchor.
    pub(crate) fn same_evidence_v2(&self, other: &Self) -> bool {
        self.identity_namespace == other.identity_namespace
            && self.source_revision == other.source_revision
            && self.fold_model_fingerprint == other.fold_model_fingerprint
            && self.semantic_graph_digest == other.semantic_graph_digest
            && self.binding_fingerprint == other.binding_fingerprint
            && self.limits == other.limits
            && self.resources == other.resources
    }
}

#[derive(Debug, Clone)]
struct ExactVertexV2 {
    id: VertexId,
    point: ExactPointV2,
    x_bits: u64,
    z_bits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactPointV2 {
    x: BigRational,
    z: BigRational,
}

#[derive(Debug)]
struct FaceRecordV2 {
    face: FaceId,
    boundary: Vec<VertexId>,
    boundary_indices: Vec<usize>,
    bounds_indices: [usize; 4],
    digest_boundary: Vec<VertexId>,
}

#[derive(Debug, Clone, Copy)]
struct EdgeOccurrenceV2 {
    first: VertexId,
    second: VertexId,
    face: FaceId,
    forward: bool,
}

#[derive(Debug, Clone, Copy)]
struct GraphEdgeV2 {
    first: VertexId,
    second: VertexId,
    first_index: usize,
    second_index: usize,
    first_face: FaceId,
    second_face: Option<FaceId>,
    first_forward: bool,
    second_forward: bool,
    has_canonical_hinge: bool,
}

#[derive(Debug, Clone, Copy)]
struct SharedFaceEdgeV2 {
    first_face: FaceId,
    second_face: FaceId,
    edge: GraphEdgeV2,
}

#[derive(Debug, Clone, Copy)]
struct CanonicalHingeV2 {
    edge_bytes: [u8; 16],
    first_vertex: VertexId,
    second_vertex: VertexId,
    left_face: FaceId,
    right_face: FaceId,
    assignment: u8,
}

#[derive(Debug)]
struct AdmissionMeterV2 {
    limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    logical_work: usize,
    exact_operations: usize,
    vertex_pair_tests: usize,
    vertex_edge_tests: usize,
    edge_pair_tests: usize,
    face_pair_tests: usize,
    point_in_polygon_edge_tests: usize,
}

impl AdmissionMeterV2 {
    fn new(limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2) -> Self {
        Self {
            limits,
            logical_work: 0,
            exact_operations: 0,
            vertex_pair_tests: 0,
            vertex_edge_tests: 0,
            edge_pair_tests: 0,
            face_pair_tests: 0,
            point_in_polygon_edge_tests: 0,
        }
    }

    fn step<F>(
        &mut self,
        logical_work: usize,
        exact_operations: usize,
        checkpoint: &mut F,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
    where
        F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
    {
        checkpoint().map_err(map_stop_v2)?;
        self.logical_work = self
            .logical_work
            .checked_add(logical_work)
            .filter(|value| *value <= self.limits.max_logical_work)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
        self.exact_operations = self
            .exact_operations
            .checked_add(exact_operations)
            .filter(|value| *value <= self.limits.max_exact_operations)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
        Ok(())
    }

    fn count_vertex_pair<F>(
        &mut self,
        checkpoint: &mut F,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
    where
        F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
    {
        self.vertex_pair_tests =
            checked_increment_with_cap_v2(self.vertex_pair_tests, self.limits.max_vertex_pairs)?;
        self.step(1, 2, checkpoint)
    }

    fn count_vertex_edge<F>(
        &mut self,
        checkpoint: &mut F,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
    where
        F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
    {
        self.vertex_edge_tests = checked_increment_with_cap_v2(
            self.vertex_edge_tests,
            self.limits.max_vertex_edge_tests,
        )?;
        self.step(1, 0, checkpoint)
    }

    fn count_edge_pair<F>(
        &mut self,
        checkpoint: &mut F,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
    where
        F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
    {
        self.edge_pair_tests =
            checked_increment_with_cap_v2(self.edge_pair_tests, self.limits.max_edge_pair_tests)?;
        self.step(1, 0, checkpoint)
    }

    fn count_face_pair<F>(
        &mut self,
        checkpoint: &mut F,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
    where
        F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
    {
        self.face_pair_tests =
            checked_increment_with_cap_v2(self.face_pair_tests, self.limits.max_face_pair_tests)?;
        self.step(1, 0, checkpoint)
    }

    fn count_point_in_polygon_edge<F>(
        &mut self,
        checkpoint: &mut F,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
    where
        F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
    {
        self.point_in_polygon_edge_tests = checked_increment_with_cap_v2(
            self.point_in_polygon_edge_tests,
            self.limits.max_point_in_polygon_edge_tests,
        )?;
        self.step(1, 0, checkpoint)
    }
}

fn map_stop_v2(
    stop: CommonArticulationPositiveThicknessParentGraphAdmissionStopV2,
) -> CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2 {
    match stop {
        CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::Cancelled => {
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled
        }
        CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::DeadlineExceeded => {
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded
        }
    }
}

fn checked_increment_with_cap_v2(
    current: usize,
    cap: usize,
) -> Result<usize, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    current
        .checked_add(1)
        .filter(|value| *value <= cap)
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)
}

/// Admits a live parent material graph using the exact XZ embedding.
pub fn admit_common_articulation_positive_thickness_parent_graph_v2(
    geometry: &MaterialHingeGraphGeometry,
    limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
) -> Result<
    CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
> {
    admit_common_articulation_positive_thickness_parent_graph_with_checkpoint_v2(
        geometry,
        limits,
        || Ok(()),
    )
}

/// Checkpointed form of exact parent-graph admission.
pub fn admit_common_articulation_positive_thickness_parent_graph_with_checkpoint_v2<F>(
    geometry: &MaterialHingeGraphGeometry,
    limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    mut checkpoint: F,
) -> Result<
    CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    build_parent_graph_admission_v2(geometry, limits, &mut checkpoint)
}

/// Revalidates the complete exact embedding against the retained envelope.
pub fn revalidate_common_articulation_positive_thickness_parent_graph_admission_v2(
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    geometry: &MaterialHingeGraphGeometry,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    revalidate_common_articulation_positive_thickness_parent_graph_admission_with_checkpoint_v2(
        admission,
        geometry,
        || Ok(()),
    )
}

/// Checkpointed form of complete exact live revalidation.
pub fn revalidate_common_articulation_positive_thickness_parent_graph_admission_with_checkpoint_v2<
    F,
>(
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    geometry: &MaterialHingeGraphGeometry,
    mut checkpoint: F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    if !admission.geometry_instance.matches(geometry) {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::AdmissionBindingMismatch,
        );
    }
    let candidate = build_parent_graph_admission_v2(geometry, admission.limits, &mut checkpoint)?;
    if candidate.identity_namespace != admission.identity_namespace
        || candidate.source_revision != admission.source_revision
        || candidate.fold_model_fingerprint != admission.fold_model_fingerprint
        || candidate.semantic_graph_digest != admission.semantic_graph_digest
        || candidate.binding_fingerprint != admission.binding_fingerprint
        || candidate.resources != admission.resources
    {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::AdmissionBindingMismatch,
        );
    }
    Ok(())
}

fn build_parent_graph_admission_v2<F>(
    geometry: &MaterialHingeGraphGeometry,
    limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    checkpoint: &mut F,
) -> Result<
    CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    if !limits.is_explicitly_finite_v2() {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput);
    }
    let mut meter = AdmissionMeterV2::new(limits);
    meter.step(1, 0, checkpoint)?;

    let identity_namespace = geometry
        .source_identity_namespace_v1()
        .filter(|namespace| namespace.canonical_bytes() != [0; 16])
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
    let source_revision = geometry
        .source_revision_v1()
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
    let fold_model_fingerprint = geometry
        .fold_model_fingerprint_v1()
        .filter(|fingerprint| *fingerprint != [0; 32])
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;

    let face_count = geometry.face_ids().len();
    let hinge_count = geometry.hinges().len();
    check_cardinality_v2(face_count, limits.max_faces, true)?;
    check_cardinality_v2(hinge_count, limits.max_hinges, false)?;

    let mut boundary_vertex_occurrences = 0usize;
    for face in geometry.face_ids() {
        meter.step(1, 0, checkpoint)?;
        let boundary = geometry
            .face_boundary_vertices(*face)
            .filter(|boundary| boundary.len() >= 3)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
        boundary_vertex_occurrences = boundary_vertex_occurrences
            .checked_add(boundary.len())
            .filter(|count| *count <= limits.max_boundary_vertex_occurrences)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    }
    let charged_workspace_bytes_upper_bound = checked_workspace_bytes_upper_bound_v2(
        face_count,
        hinge_count,
        boundary_vertex_occurrences,
    )?;
    if charged_workspace_bytes_upper_bound > limits.max_workspace_bytes {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit);
    }

    let mut face_ids = Vec::new();
    try_reserve_exact_v2(&mut face_ids, face_count)?;
    for face in geometry.face_ids() {
        meter.step(1, 0, checkpoint)?;
        face_ids.push(*face);
    }
    heap_sort_by_v2(
        &mut face_ids,
        |first, second| first.canonical_bytes().cmp(&second.canonical_bytes()),
        &mut meter,
        checkpoint,
    )?;
    for index in 1..face_ids.len() {
        meter.step(1, 0, checkpoint)?;
        if face_ids[index - 1].canonical_bytes() == face_ids[index].canonical_bytes() {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
    }

    let mut vertex_ids = Vec::new();
    try_reserve_exact_v2(&mut vertex_ids, boundary_vertex_occurrences)?;
    let mut faces = Vec::new();
    try_reserve_exact_v2(&mut faces, face_count)?;
    for face in &face_ids {
        meter.step(1, 0, checkpoint)?;
        let boundary = geometry
            .face_boundary_vertices(*face)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
        let mut local = Vec::new();
        try_reserve_exact_v2(&mut local, boundary.len())?;
        for vertex in boundary {
            meter.step(1, 0, checkpoint)?;
            for existing in &local {
                meter.step(1, 0, checkpoint)?;
                if existing == vertex {
                    return Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
                    );
                }
            }
            local.push(*vertex);
            vertex_ids.push(*vertex);
        }
        let digest_boundary = canonical_unoriented_cycle_v2(&local, &mut meter, checkpoint)?;
        faces.push(FaceRecordV2 {
            face: *face,
            boundary: local,
            boundary_indices: Vec::new(),
            bounds_indices: [0; 4],
            digest_boundary,
        });
    }

    heap_sort_by_v2(
        &mut vertex_ids,
        |first, second| first.canonical_bytes().cmp(&second.canonical_bytes()),
        &mut meter,
        checkpoint,
    )?;
    let mut write = 0usize;
    for read in 0..vertex_ids.len() {
        meter.step(1, 0, checkpoint)?;
        if write == 0 || vertex_ids[read] != vertex_ids[write - 1] {
            vertex_ids[write] = vertex_ids[read];
            write += 1;
        }
    }
    vertex_ids.truncate(write);
    check_cardinality_v2(vertex_ids.len(), limits.max_vertices, true)?;

    let mut vertices = Vec::new();
    try_reserve_exact_v2(&mut vertices, vertex_ids.len())?;
    for id in &vertex_ids {
        meter.step(1, 3, checkpoint)?;
        let position = geometry
            .vertex_position(*id)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
        validate_sheet_point_v2(position)?;
        let point = ExactPointV2 {
            x: BigRational::from_float(position.x()).ok_or(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            )?,
            z: BigRational::from_float(position.z()).ok_or(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            )?,
        };
        vertices.push(ExactVertexV2 {
            id: *id,
            point,
            x_bits: normalized_zero_bits_v2(position.x()),
            z_bits: normalized_zero_bits_v2(position.z()),
        });
    }

    for face in &mut faces {
        try_reserve_exact_v2(&mut face.boundary_indices, face.boundary.len())?;
        for vertex in &face.boundary {
            meter.step(1, 0, checkpoint)?;
            face.boundary_indices.push(find_vertex_index_v2(
                &vertices, *vertex, &mut meter, checkpoint,
            )?);
        }
        let first = *face
            .boundary_indices
            .first()
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
        face.bounds_indices = [first; 4];
        for index in &face.boundary_indices[1..] {
            meter.step(1, 4, checkpoint)?;
            if vertices[*index].point.x < vertices[face.bounds_indices[0]].point.x {
                face.bounds_indices[0] = *index;
            }
            if vertices[*index].point.x > vertices[face.bounds_indices[1]].point.x {
                face.bounds_indices[1] = *index;
            }
            if vertices[*index].point.z < vertices[face.bounds_indices[2]].point.z {
                face.bounds_indices[2] = *index;
            }
            if vertices[*index].point.z > vertices[face.bounds_indices[3]].point.z {
                face.bounds_indices[3] = *index;
            }
        }
    }

    let mut edge_occurrences = Vec::new();
    try_reserve_exact_v2(&mut edge_occurrences, boundary_vertex_occurrences)?;
    for face in &faces {
        for index in 0..face.boundary.len() {
            meter.step(1, 0, checkpoint)?;
            let start = face.boundary[index];
            let end = face.boundary[(index + 1) % face.boundary.len()];
            let (first, second, forward) = canonical_edge_endpoints_v2(start, end);
            if first == second {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
            let first_vertex = find_vertex_v2(&vertices, first, &mut meter, checkpoint)?;
            let second_vertex = find_vertex_v2(&vertices, second, &mut meter, checkpoint)?;
            if first_vertex.point == second_vertex.point {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
            edge_occurrences.push(EdgeOccurrenceV2 {
                first,
                second,
                face: face.face,
                forward,
            });
        }
    }
    heap_sort_by_v2(
        &mut edge_occurrences,
        |first, second| edge_occurrence_key_v2(*first).cmp(&edge_occurrence_key_v2(*second)),
        &mut meter,
        checkpoint,
    )?;

    let mut edges = Vec::new();
    try_reserve_exact_v2(&mut edges, edge_occurrences.len())?;
    let mut index = 0usize;
    while index < edge_occurrences.len() {
        meter.step(1, 0, checkpoint)?;
        let occurrence = edge_occurrences[index];
        let mut end = index + 1;
        while end < edge_occurrences.len()
            && edge_occurrences[end].first == occurrence.first
            && edge_occurrences[end].second == occurrence.second
        {
            meter.step(1, 0, checkpoint)?;
            end += 1;
        }
        let count = end - index;
        if count > 2
            || (count == 2
                && (edge_occurrences[index].face == edge_occurrences[index + 1].face
                    || edge_occurrences[index].forward == edge_occurrences[index + 1].forward))
        {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
        let second_face = if count == 2 {
            Some(edge_occurrences[index + 1].face)
        } else {
            None
        };
        edges.push(GraphEdgeV2 {
            first: occurrence.first,
            second: occurrence.second,
            first_index: find_vertex_index_v2(&vertices, occurrence.first, &mut meter, checkpoint)?,
            second_index: find_vertex_index_v2(
                &vertices,
                occurrence.second,
                &mut meter,
                checkpoint,
            )?,
            first_face: occurrence.face,
            second_face,
            first_forward: occurrence.forward,
            second_forward: count == 2 && edge_occurrences[index + 1].forward,
            has_canonical_hinge: false,
        });
        index = end;
    }
    check_cardinality_v2(edges.len(), limits.max_edges, true)?;

    let mut hinges = Vec::new();
    try_reserve_exact_v2(&mut hinges, hinge_count)?;
    for hinge in geometry.hinges() {
        meter.step(1, 0, checkpoint)?;
        let start = hinge.start();
        let end = hinge.end();
        let axis = hinge.axis();
        validate_sheet_point_v2(start)?;
        validate_sheet_point_v2(end)?;
        if !axis.x().is_finite()
            || !axis.y().is_finite()
            || !axis.z().is_finite()
            || axis.y() != 0.0
            || (axis.x() == 0.0 && axis.z() == 0.0)
        {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
        let start_id = find_vertex_by_position_v2(&vertices, start, &mut meter, checkpoint)?;
        let end_id = find_vertex_by_position_v2(&vertices, end, &mut meter, checkpoint)?;
        if start_id.canonical_bytes() >= end_id.canonical_bytes() {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
        let start_vertex = find_vertex_v2(&vertices, start_id, &mut meter, checkpoint)?;
        let end_vertex = find_vertex_v2(&vertices, end_id, &mut meter, checkpoint)?;
        validate_exact_hinge_axis_v2(
            &start_vertex.point,
            &end_vertex.point,
            axis,
            &mut meter,
            checkpoint,
        )?;
        let edge_index = find_edge_index_v2(&edges, start_id, end_id, &mut meter, checkpoint)?;
        let edge = &mut edges[edge_index];
        let Some(second_face) = edge.second_face else {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        };
        let (forward_face, reverse_face) = if edge.first_forward {
            (edge.first_face, second_face)
        } else if edge.second_forward {
            (second_face, edge.first_face)
        } else {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        };
        if edge.has_canonical_hinge
            || hinge.left_face() != forward_face
            || hinge.right_face() != reverse_face
        {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
        edge.has_canonical_hinge = true;
        hinges.push(CanonicalHingeV2 {
            edge_bytes: hinge.edge().canonical_bytes(),
            first_vertex: start_id,
            second_vertex: end_id,
            left_face: hinge.left_face(),
            right_face: hinge.right_face(),
            assignment: match hinge.assignment() {
                FoldAssignment::Mountain => 0,
                FoldAssignment::Valley => 1,
            },
        });
    }
    for edge in &edges {
        meter.step(1, 0, checkpoint)?;
        if edge.second_face.is_some() != edge.has_canonical_hinge {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
    }
    heap_sort_by_v2(
        &mut hinges,
        |first, second| first.edge_bytes.cmp(&second.edge_bytes),
        &mut meter,
        checkpoint,
    )?;
    for index in 1..hinges.len() {
        meter.step(1, 0, checkpoint)?;
        if hinges[index - 1].edge_bytes == hinges[index].edge_bytes {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
    }

    let mut shared_face_edges = Vec::new();
    try_reserve_exact_v2(&mut shared_face_edges, hinge_count)?;
    for edge in &edges {
        meter.step(1, 0, checkpoint)?;
        if let Some(second_face) = edge.second_face {
            let (first_face, second_face) = canonical_face_pair_v2(edge.first_face, second_face);
            shared_face_edges.push(SharedFaceEdgeV2 {
                first_face,
                second_face,
                edge: *edge,
            });
        }
    }
    heap_sort_by_v2(
        &mut shared_face_edges,
        |first, second| shared_face_edge_key_v2(*first).cmp(&shared_face_edge_key_v2(*second)),
        &mut meter,
        checkpoint,
    )?;
    for index in 1..shared_face_edges.len() {
        meter.step(1, 0, checkpoint)?;
        if shared_face_edge_key_v2(shared_face_edges[index - 1])
            == shared_face_edge_key_v2(shared_face_edges[index])
        {
            // Multiple shared material edges are multiple contact features;
            // this admission accepts exactly one canonical hinge per pair.
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
            );
        }
    }

    // `try_reserve_exact` may legally receive more capacity from an
    // allocator than requested. Recompute from every simultaneously live
    // physical Vec capacity (including nested face cycles), then retain the
    // larger of that observation and the allocator-independent charged bound.
    let physical_workspace_bytes_upper_bound = checked_physical_workspace_bytes_v2(
        &face_ids,
        &vertex_ids,
        &faces,
        &vertices,
        &edge_occurrences,
        &edges,
        &hinges,
        &shared_face_edges,
    )?;
    let workspace_bytes_upper_bound =
        charged_workspace_bytes_upper_bound.max(physical_workspace_bytes_upper_bound);
    if workspace_bytes_upper_bound > limits.max_workspace_bytes {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit);
    }

    preflight_high_order_minimum_work_v2(
        &faces,
        vertices.len(),
        edges.len(),
        meter.logical_work,
        limits.max_logical_work,
    )?;

    checked_unordered_pair_count_against_limit_v2(vertices.len(), limits.max_vertex_pairs)?;
    for first in 0..vertices.len() {
        for second in first + 1..vertices.len() {
            meter.count_vertex_pair(checkpoint)?;
            if vertices[first].point == vertices[second].point {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
        }
    }

    for face in &faces {
        validate_exact_face_geometry_v2(face, &vertices, &mut meter, checkpoint)?;
    }

    let vertex_edge_product = vertices
        .len()
        .checked_mul(edges.len())
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    if vertex_edge_product > limits.max_vertex_edge_tests {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit);
    }
    for vertex in &vertices {
        for edge in &edges {
            meter.count_vertex_edge(checkpoint)?;
            if vertex.id == edge.first || vertex.id == edge.second {
                continue;
            }
            let first = &vertices[edge.first_index].point;
            let second = &vertices[edge.second_index].point;
            if point_strictly_outside_segment_bounds_v2(
                &vertex.point,
                first,
                second,
                &mut meter,
                checkpoint,
            )? {
                continue;
            }
            if point_on_segment_v2(&vertex.point, first, second, &mut meter, checkpoint)? {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
        }
    }

    checked_unordered_pair_count_against_limit_v2(edges.len(), limits.max_edge_pair_tests)?;
    for first_index in 0..edges.len() {
        for second_index in first_index + 1..edges.len() {
            meter.count_edge_pair(checkpoint)?;
            let first = edges[first_index];
            let second = edges[second_index];
            if edges_share_vertex_v2(first, second) {
                continue;
            }
            let a = &vertices[first.first_index].point;
            let b = &vertices[first.second_index].point;
            let c = &vertices[second.first_index].point;
            let d = &vertices[second.second_index].point;
            if segment_bounds_strictly_disjoint_v2(a, b, c, d, &mut meter, checkpoint)? {
                continue;
            }
            if segments_intersect_closed_v2(a, b, c, d, &mut meter, checkpoint)? {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
        }
    }

    checked_unordered_pair_count_against_limit_v2(faces.len(), limits.max_face_pair_tests)?;
    for first_index in 0..faces.len() {
        for second_index in first_index + 1..faces.len() {
            meter.count_face_pair(checkpoint)?;
            let first = &faces[first_index];
            let second = &faces[second_index];
            let shared_edge = find_shared_face_edge_v2(
                first.face,
                second.face,
                &shared_face_edges,
                &mut meter,
                checkpoint,
            )?;
            validate_face_pair_shared_features_v2(
                first,
                second,
                shared_edge,
                &vertices,
                &mut meter,
                checkpoint,
            )?;
            if let Some(shared_edge) = shared_edge {
                validate_adjacent_face_half_planes_v2(
                    first,
                    second,
                    shared_edge,
                    &vertices,
                    &mut meter,
                    checkpoint,
                )?;
            }
            if face_bounds_strictly_disjoint_v2(first, second, &vertices, &mut meter, checkpoint)? {
                continue;
            }
            if face_has_strictly_contained_vertex_v2(
                first, second, &vertices, &mut meter, checkpoint,
            )? || face_has_strictly_contained_vertex_v2(
                second, first, &vertices, &mut meter, checkpoint,
            )? {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
        }
    }

    let semantic_graph_digest = semantic_graph_digest_v2(
        identity_namespace,
        source_revision,
        fold_model_fingerprint,
        &vertices,
        &faces,
        &edges,
        &hinges,
        &mut meter,
        checkpoint,
    )?;
    let resources = CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2 {
        face_count,
        hinge_count,
        boundary_vertex_occurrences,
        vertex_count: vertices.len(),
        edge_count: edges.len(),
        vertex_pair_tests: meter.vertex_pair_tests,
        vertex_edge_tests: meter.vertex_edge_tests,
        edge_pair_tests: meter.edge_pair_tests,
        face_pair_tests: meter.face_pair_tests,
        point_in_polygon_edge_tests: meter.point_in_polygon_edge_tests,
        exact_operations: meter.exact_operations,
        logical_work: meter.logical_work,
        workspace_bytes_upper_bound,
    };
    let binding_fingerprint = admission_binding_fingerprint_v2(
        identity_namespace,
        source_revision,
        fold_model_fingerprint,
        semantic_graph_digest,
        limits,
        resources,
    )?;
    Ok(CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
        geometry_instance: geometry.instance_anchor_v1(),
        identity_namespace,
        source_revision,
        fold_model_fingerprint,
        semantic_graph_digest,
        binding_fingerprint,
        limits,
        resources,
    })
}

fn validate_sheet_point_v2(
    point: Point3,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    if !point.x().is_finite()
        || !point.y().is_finite()
        || !point.z().is_finite()
        || point.y() != 0.0
    {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput);
    }
    Ok(())
}

fn normalized_zero_bits_v2(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

fn check_cardinality_v2(
    count: usize,
    limit: usize,
    require_nonzero: bool,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    if require_nonzero && count == 0 {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput);
    }
    if count > limit {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit);
    }
    Ok(())
}

fn checked_unordered_pair_count_against_limit_v2(
    count: usize,
    limit: usize,
) -> Result<usize, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    let pairs = count
        .checked_mul(count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    if pairs > limit {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit);
    }
    Ok(pairs)
}

fn preflight_high_order_minimum_work_v2(
    faces: &[FaceRecordV2],
    vertex_count: usize,
    edge_count: usize,
    work_already_charged: usize,
    max_logical_work: usize,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    let vertex_pairs = vertex_count
        .checked_mul(vertex_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    let vertex_edge_tests = vertex_count
        .checked_mul(edge_count)
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    let edge_pairs = edge_count
        .checked_mul(edge_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    let face_pairs = faces
        .len()
        .checked_mul(faces.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    let mut boundary_sum = 0usize;
    let mut boundary_square_sum = 0usize;
    for face in faces {
        boundary_sum = boundary_sum
            .checked_add(face.boundary.len())
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
        boundary_square_sum = boundary_square_sum
            .checked_add(face.boundary.len().checked_mul(face.boundary.len()).ok_or(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit,
            )?)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    }
    let cross_face_boundary_comparisons = boundary_sum
        .checked_mul(boundary_sum)
        .and_then(|value| value.checked_sub(boundary_square_sum))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    let minimum = [
        vertex_pairs,
        vertex_edge_tests,
        edge_pairs,
        face_pairs,
        cross_face_boundary_comparisons,
    ]
    .into_iter()
    .try_fold(work_already_charged, usize::checked_add)
    .filter(|value| *value <= max_logical_work)
    .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    let _ = minimum;
    Ok(())
}

fn checked_workspace_bytes_upper_bound_v2(
    face_count: usize,
    hinge_count: usize,
    occurrences: usize,
) -> Result<usize, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    let checked_product = |count: usize, bytes: usize| {
        count
            .checked_mul(bytes)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)
    };
    let mut total = EXACT_PREDICATE_SCRATCH_BYTES_UPPER_BOUND_V2;
    for bytes in [
        checked_product(face_count, size_of::<FaceRecordV2>())?,
        checked_product(face_count, size_of::<FaceId>())?,
        checked_product(occurrences, size_of::<VertexId>())?,
        checked_product(occurrences, size_of::<VertexId>())?,
        checked_product(occurrences, size_of::<VertexId>())?,
        checked_product(occurrences, size_of::<usize>())?,
        checked_product(occurrences, size_of::<EdgeOccurrenceV2>())?,
        checked_product(occurrences, size_of::<GraphEdgeV2>())?,
        checked_product(occurrences, size_of::<ExactVertexV2>())?,
        checked_product(occurrences, EXACT_VERTEX_DYNAMIC_BYTES_UPPER_BOUND_V2)?,
        checked_product(hinge_count, size_of::<CanonicalHingeV2>())?,
        checked_product(hinge_count, size_of::<SharedFaceEdgeV2>())?,
    ] {
        total = total
            .checked_add(bytes)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    }
    Ok(total)
}

#[allow(clippy::too_many_arguments)]
fn checked_physical_workspace_bytes_v2(
    face_ids: &Vec<FaceId>,
    vertex_ids: &Vec<VertexId>,
    faces: &Vec<FaceRecordV2>,
    vertices: &Vec<ExactVertexV2>,
    edge_occurrences: &Vec<EdgeOccurrenceV2>,
    edges: &Vec<GraphEdgeV2>,
    hinges: &Vec<CanonicalHingeV2>,
    shared_face_edges: &Vec<SharedFaceEdgeV2>,
) -> Result<usize, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    fn add_capacity_v2<T>(
        total: &mut usize,
        values: &Vec<T>,
    ) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
        *total = total
            .checked_add(bytes)
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
        Ok(())
    }

    let mut total = EXACT_PREDICATE_SCRATCH_BYTES_UPPER_BOUND_V2;
    add_capacity_v2(&mut total, face_ids)?;
    add_capacity_v2(&mut total, vertex_ids)?;
    add_capacity_v2(&mut total, faces)?;
    for face in faces {
        add_capacity_v2(&mut total, &face.boundary)?;
        add_capacity_v2(&mut total, &face.boundary_indices)?;
        add_capacity_v2(&mut total, &face.digest_boundary)?;
    }
    add_capacity_v2(&mut total, vertices)?;
    total = total
        .checked_add(
            vertices
                .len()
                .checked_mul(EXACT_VERTEX_DYNAMIC_BYTES_UPPER_BOUND_V2)
                .ok_or(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit,
                )?,
        )
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)?;
    add_capacity_v2(&mut total, edge_occurrences)?;
    add_capacity_v2(&mut total, edges)?;
    add_capacity_v2(&mut total, hinges)?;
    add_capacity_v2(&mut total, shared_face_edges)?;
    Ok(total)
}

fn try_reserve_exact_v2<T>(
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit)
}

fn canonical_edge_endpoints_v2(start: VertexId, end: VertexId) -> (VertexId, VertexId, bool) {
    if start.canonical_bytes() < end.canonical_bytes() {
        (start, end, true)
    } else {
        (end, start, false)
    }
}

fn edge_occurrence_key_v2(occurrence: EdgeOccurrenceV2) -> ([u8; 16], [u8; 16], [u8; 16]) {
    (
        occurrence.first.canonical_bytes(),
        occurrence.second.canonical_bytes(),
        occurrence.face.canonical_bytes(),
    )
}

fn canonical_unoriented_cycle_v2<F>(
    boundary: &[VertexId],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<Vec<VertexId>, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut best_start = 0usize;
    let mut best_reversed = false;
    for reversed in [false, true] {
        for start in 0..boundary.len() {
            meter.step(1, 0, checkpoint)?;
            if cycle_rotation_less_v2(
                boundary,
                start,
                reversed,
                best_start,
                best_reversed,
                meter,
                checkpoint,
            )? {
                best_start = start;
                best_reversed = reversed;
            }
        }
    }
    let mut canonical = Vec::new();
    try_reserve_exact_v2(&mut canonical, boundary.len())?;
    for offset in 0..boundary.len() {
        meter.step(1, 0, checkpoint)?;
        canonical.push(cycle_vertex_v2(boundary, best_start, best_reversed, offset));
    }
    Ok(canonical)
}

fn cycle_rotation_less_v2<F>(
    boundary: &[VertexId],
    candidate_start: usize,
    candidate_reversed: bool,
    best_start: usize,
    best_reversed: bool,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    for offset in 0..boundary.len() {
        meter.step(1, 0, checkpoint)?;
        let candidate = cycle_vertex_v2(boundary, candidate_start, candidate_reversed, offset)
            .canonical_bytes();
        let best = cycle_vertex_v2(boundary, best_start, best_reversed, offset).canonical_bytes();
        match candidate.cmp(&best) {
            Ordering::Less => return Ok(true),
            Ordering::Greater => return Ok(false),
            Ordering::Equal => {}
        }
    }
    Ok(false)
}

fn cycle_vertex_v2(boundary: &[VertexId], start: usize, reversed: bool, offset: usize) -> VertexId {
    let index = if reversed {
        (start + boundary.len() - (offset % boundary.len())) % boundary.len()
    } else {
        (start + offset) % boundary.len()
    };
    boundary[index]
}

fn heap_sort_by_v2<T, C, F>(
    values: &mut [T],
    mut compare: C,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    C: FnMut(&T, &T) -> Ordering,
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    if values.len() < 2 {
        return Ok(());
    }
    for root in (0..values.len() / 2).rev() {
        sift_down_v2(values, root, values.len(), &mut compare, meter, checkpoint)?;
    }
    for end in (1..values.len()).rev() {
        meter.step(1, 0, checkpoint)?;
        values.swap(0, end);
        sift_down_v2(values, 0, end, &mut compare, meter, checkpoint)?;
    }
    Ok(())
}

fn sift_down_v2<T, C, F>(
    values: &mut [T],
    mut root: usize,
    end: usize,
    compare: &mut C,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    C: FnMut(&T, &T) -> Ordering,
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    loop {
        let Some(left) = root.checked_mul(2).and_then(|value| value.checked_add(1)) else {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit,
            );
        };
        if left >= end {
            return Ok(());
        }
        let right = left + 1;
        let mut child = left;
        if right < end {
            meter.step(1, 0, checkpoint)?;
            if compare(&values[left], &values[right]) == Ordering::Less {
                child = right;
            }
        }
        meter.step(1, 0, checkpoint)?;
        if compare(&values[root], &values[child]) != Ordering::Less {
            return Ok(());
        }
        values.swap(root, child);
        root = child;
    }
}

fn find_vertex_v2<'a, F>(
    vertices: &'a [ExactVertexV2],
    id: VertexId,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<&'a ExactVertexV2, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let index = find_vertex_index_v2(vertices, id, meter, checkpoint)?;
    Ok(&vertices[index])
}

fn find_vertex_index_v2<F>(
    vertices: &[ExactVertexV2],
    id: VertexId,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<usize, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let target = id.canonical_bytes();
    let mut lower = 0usize;
    let mut upper = vertices.len();
    while lower < upper {
        meter.step(1, 0, checkpoint)?;
        let middle = lower + (upper - lower) / 2;
        match vertices[middle].id.canonical_bytes().cmp(&target) {
            Ordering::Less => lower = middle + 1,
            Ordering::Greater => upper = middle,
            Ordering::Equal => return Ok(middle),
        }
    }
    Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)
}

fn find_vertex_by_position_v2<F>(
    vertices: &[ExactVertexV2],
    point: Point3,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<VertexId, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let x_bits = normalized_zero_bits_v2(point.x());
    let z_bits = normalized_zero_bits_v2(point.z());
    for vertex in vertices {
        meter.step(1, 2, checkpoint)?;
        if vertex.x_bits == x_bits && vertex.z_bits == z_bits {
            return Ok(vertex.id);
        }
    }
    Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)
}

fn find_edge_index_v2<F>(
    edges: &[GraphEdgeV2],
    first: VertexId,
    second: VertexId,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<usize, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let target = (first.canonical_bytes(), second.canonical_bytes());
    let mut lower = 0usize;
    let mut upper = edges.len();
    while lower < upper {
        meter.step(1, 0, checkpoint)?;
        let middle = lower + (upper - lower) / 2;
        let key = (
            edges[middle].first.canonical_bytes(),
            edges[middle].second.canonical_bytes(),
        );
        match key.cmp(&target) {
            Ordering::Less => lower = middle + 1,
            Ordering::Greater => upper = middle,
            Ordering::Equal => return Ok(middle),
        }
    }
    Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)
}

#[allow(clippy::too_many_arguments)]
fn semantic_graph_digest_v2<F>(
    identity_namespace: ProjectId,
    source_revision: u64,
    fold_model_fingerprint: [u8; 32],
    vertices: &[ExactVertexV2],
    faces: &[FaceRecordV2],
    edges: &[GraphEdgeV2],
    hinges: &[CanonicalHingeV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<[u8; 32], CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_POSITIVE_THICKNESS_PARENT_GRAPH_ADMISSION_MODEL_ID_V2);
    hash.update(identity_namespace.canonical_bytes());
    hash.update(source_revision.to_be_bytes());
    hash.update(fold_model_fingerprint);
    update_len_v2(&mut hash, vertices.len())?;
    for vertex in vertices {
        meter.step(1, 0, checkpoint)?;
        hash.update(vertex.id.canonical_bytes());
        hash.update(vertex.x_bits.to_be_bytes());
        hash.update(vertex.z_bits.to_be_bytes());
    }
    update_len_v2(&mut hash, faces.len())?;
    for face in faces {
        meter.step(1, 0, checkpoint)?;
        hash.update(face.face.canonical_bytes());
        update_len_v2(&mut hash, face.digest_boundary.len())?;
        for vertex in &face.digest_boundary {
            meter.step(1, 0, checkpoint)?;
            hash.update(vertex.canonical_bytes());
        }
    }
    update_len_v2(&mut hash, edges.len())?;
    for edge in edges {
        meter.step(1, 0, checkpoint)?;
        hash.update(edge.first.canonical_bytes());
        hash.update(edge.second.canonical_bytes());
        let mut incident = [edge.first_face.canonical_bytes(), [0; 16]];
        let count = if let Some(second) = edge.second_face {
            incident[1] = second.canonical_bytes();
            incident.sort_unstable();
            2
        } else {
            1
        };
        hash.update([count]);
        for face in &incident[..count as usize] {
            hash.update(face);
        }
    }
    update_len_v2(&mut hash, hinges.len())?;
    for hinge in hinges {
        meter.step(1, 0, checkpoint)?;
        hash.update(hinge.edge_bytes);
        hash.update(hinge.first_vertex.canonical_bytes());
        hash.update(hinge.second_vertex.canonical_bytes());
        hash.update(hinge.left_face.canonical_bytes());
        hash.update(hinge.right_face.canonical_bytes());
        hash.update([hinge.assignment]);
    }
    Ok(hash.finalize().into())
}

fn admission_binding_fingerprint_v2(
    identity_namespace: ProjectId,
    source_revision: u64,
    fold_model_fingerprint: [u8; 32],
    semantic_graph_digest: [u8; 32],
    limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    resources: CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2,
) -> Result<[u8; 32], CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_PARENT_GRAPH_ADMISSION_BINDING_V2");
    hash.update(identity_namespace.canonical_bytes());
    hash.update(source_revision.to_be_bytes());
    hash.update(fold_model_fingerprint);
    hash.update(semantic_graph_digest);
    for value in [
        limits.max_faces,
        limits.max_hinges,
        limits.max_boundary_vertex_occurrences,
        limits.max_vertices,
        limits.max_edges,
        limits.max_vertex_pairs,
        limits.max_vertex_edge_tests,
        limits.max_edge_pair_tests,
        limits.max_face_pair_tests,
        limits.max_point_in_polygon_edge_tests,
        limits.max_exact_operations,
        limits.max_logical_work,
        limits.max_workspace_bytes,
        resources.face_count,
        resources.hinge_count,
        resources.boundary_vertex_occurrences,
        resources.vertex_count,
        resources.edge_count,
        resources.vertex_pair_tests,
        resources.vertex_edge_tests,
        resources.edge_pair_tests,
        resources.face_pair_tests,
        resources.point_in_polygon_edge_tests,
        resources.exact_operations,
        resources.logical_work,
        resources.workspace_bytes_upper_bound,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

fn update_len_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    update_usize_v2(hash, value)
}

fn update_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2> {
    let value = u64::try_from(value).map_err(|_| {
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit
    })?;
    hash.update(value.to_be_bytes());
    Ok(())
}

#[cfg(test)]
mod tests;
