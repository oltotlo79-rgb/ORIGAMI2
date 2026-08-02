use num_rational::BigRational;
use ori_kinematics::{ClosedMaterialHingeGraphPose, MaterialHingeGraphGeometry, Point3};
use std::{collections::HashSet, sync::Arc};

mod admitted_graph_proof_v2;
mod parent_graph_admission_v2;

pub use admitted_graph_proof_v2::*;
pub use parent_graph_admission_v2::*;

/// Crate-internal classification of the narrowly scoped bases allowed to
/// discharge a shared material-feature pair. `LegacyFlatPoseV1` freezes only
/// the existing V1 zero-pose compatibility contract and must never establish
/// new V2 authority. The continuous V2 path retains its schedule-bound
/// certificate; the general-N V2 path requires an exact parent admission at
/// the bit-identical stationary zero pose.
#[derive(Clone, Copy)]
pub(in crate::graph_positive_thickness) enum PositiveThicknessSharedContactScopeV2<'a> {
    LegacyFlatPoseV1,
    Continuous(&'a CommonArticulationAdmittedSharedFeatureContactCertificateV2),
    StationaryParent(&'a CommonArticulationPositiveThicknessParentGraphAdmissionV2),
}

impl PositiveThicknessSharedContactScopeV2<'_> {
    fn proves_shared_contact_for_pose_v2(
        self,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        match self {
            Self::LegacyFlatPoseV1 => {
                pose.is_for_geometry(geometry)
                    && pose.hinge_angles().as_slice().len() == geometry.hinges().len()
                    && pose
                        .hinge_angles()
                        .as_slice()
                        .iter()
                        .all(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits())
            }
            Self::Continuous(evidence) => {
                evidence.proves_shared_contact_for_pose_v2(geometry, pose)
            }
            Self::StationaryParent(admission) => {
                admission.matches_geometry_instance_v2(geometry)
                    && pose.is_for_geometry(geometry)
                    && pose.hinge_angles().as_slice().len() == geometry.hinges().len()
                    && pose
                        .hinge_angles()
                        .as_slice()
                        .iter()
                        .all(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits())
            }
        }
    }
}

pub const POSITIVE_THICKNESS_GRAPH_GEOMETRY_PROOF_V1: &str =
    "positive_thickness_graph_geometry_proof_v1";

/// The explicit 11..=32 common-articulation resource extension may inspect a
/// 257-face parent graph.  This is deliberately separate from the legacy
/// 97-face entry point below.
pub const COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1: usize = 257;
pub const COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_UNORDERED_FACE_PAIRS_V1:
    usize = 32_896;
pub const COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_SHARED_FEATURE_PAIRS_V1:
    usize = 32_896;

// This ceiling is checked before any per-boundary geometry work.  Together
// with the face-pair caps above, it bounds the outer scans and the exact prism
// SAT work: a pair can have at most 66_565 candidate axes, each projecting at
// most 1_024 extruded vertices.  The 256-vertex envelope admits the largest
// current rational-cycle fixtures (the 32-bay outer ring has 162 vertices)
// while preserving a fixed, fail-closed resource limit.
const POSITIVE_THICKNESS_GRAPH_MAX_FACE_BOUNDARY_VERTICES_V1: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositiveThicknessGraphLimitsV1 {
    pub max_unordered_face_pairs: usize,
    pub max_shared_feature_pairs: usize,
}

impl Default for PositiveThicknessGraphLimitsV1 {
    fn default() -> Self {
        Self {
            max_unordered_face_pairs: 4_656,
            max_shared_feature_pairs: 4_656,
        }
    }
}

/// Explicit pair budgets for the separately scoped 257-face
/// common-articulation graph proof extension.
///
/// There is intentionally no `Default`: callers must opt into the extension
/// and record its resource envelope.  Each supplied cap may be smaller than
/// the fixed extension maximum, but never larger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
    pub max_unordered_face_pairs: usize,
    pub max_shared_feature_pairs: usize,
}

impl CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
    /// Returns the inclusive fixed V1 envelope for a 257-face parent graph.
    #[must_use]
    pub const fn fixed_v1() -> Self {
        Self {
            max_unordered_face_pairs:
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_UNORDERED_FACE_PAIRS_V1,
            max_shared_feature_pairs:
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_SHARED_FEATURE_PAIRS_V1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositiveThicknessGraphProofErrorV1 {
    InvalidInput,
    ResourceLimit,
    PairEvidenceUnavailable,
}

#[derive(Debug, Clone)]
pub struct NativePositiveThicknessGraphGeometryProofV1 {
    identity: Arc<()>,
    geometry: MaterialHingeGraphGeometry,
    pose: ClosedMaterialHingeGraphPose,
    paper_thickness_bits: u64,
    analyzed_unordered_face_pairs: usize,
}

impl NativePositiveThicknessGraphGeometryProofV1 {
    #[must_use]
    pub fn is_for_geometry(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
        paper_thickness_mm: f64,
    ) -> bool {
        self.geometry.same_instance(geometry)
            && self.pose.same_instance(pose)
            && self.paper_thickness_bits == paper_thickness_mm.to_bits()
    }

    #[must_use]
    pub const fn analyzed_unordered_face_pairs(&self) -> usize {
        self.analyzed_unordered_face_pairs
    }

    #[must_use]
    pub fn paper_thickness_bits(&self) -> u64 {
        self.paper_thickness_bits
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.geometry.face_ids().len()
    }

    #[must_use]
    pub fn same_proof(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

pub fn prove_positive_thickness_graph_geometry_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: PositiveThicknessGraphLimitsV1,
) -> Result<NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphProofErrorV1> {
    prove_positive_thickness_graph_geometry_with_max_faces_v1(
        geometry,
        pose,
        paper_thickness_mm,
        limits,
        97,
    )
}

/// Proves the native positive-thickness graph condition under the explicit
/// 257-face common-articulation extension envelope.
///
/// This does not widen the legacy graph-proof entry point or its default
/// limits.  The returned native proof remains bound to this geometry and pose.
pub(crate) fn prove_common_articulation_positive_thickness_graph_geometry_extension_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
) -> Result<NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphProofErrorV1> {
    validate_common_articulation_positive_thickness_graph_extension_limits_v1(limits)?;
    prove_positive_thickness_graph_geometry_with_max_faces_v1(
        geometry,
        pose,
        paper_thickness_mm,
        PositiveThicknessGraphLimitsV1 {
            max_unordered_face_pairs: limits.max_unordered_face_pairs,
            max_shared_feature_pairs: limits.max_shared_feature_pairs,
        },
        COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
    )
}

fn validate_common_articulation_positive_thickness_graph_extension_limits_v1(
    limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
) -> Result<(), PositiveThicknessGraphProofErrorV1> {
    if limits.max_unordered_face_pairs
        > COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_UNORDERED_FACE_PAIRS_V1
        || limits.max_shared_feature_pairs
            > COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_SHARED_FEATURE_PAIRS_V1
    {
        return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
    }
    Ok(())
}

fn checked_unordered_face_pair_count_v1(
    face_count: usize,
    max_faces: usize,
    max_unordered_face_pairs: usize,
) -> Result<usize, PositiveThicknessGraphProofErrorV1> {
    if !(3..=max_faces).contains(&face_count) {
        return Err(PositiveThicknessGraphProofErrorV1::InvalidInput);
    }
    let pair_count = face_count
        .checked_mul(face_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
    if pair_count > max_unordered_face_pairs {
        return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
    }
    Ok(pair_count)
}

fn check_shared_feature_pair_limit_v1(
    shared_feature_pairs: usize,
    max_shared_feature_pairs: usize,
) -> Result<(), PositiveThicknessGraphProofErrorV1> {
    if shared_feature_pairs > max_shared_feature_pairs {
        return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
    }
    Ok(())
}

fn check_positive_thickness_graph_face_boundary_vertex_count_v1(
    boundary_vertex_count: usize,
) -> Result<(), PositiveThicknessGraphProofErrorV1> {
    if boundary_vertex_count > POSITIVE_THICKNESS_GRAPH_MAX_FACE_BOUNDARY_VERTICES_V1 {
        return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
    }
    Ok(())
}

fn exact_input_rational_v1(value: f64) -> Result<BigRational, PositiveThicknessGraphProofErrorV1> {
    BigRational::from_float(value).ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)
}

fn prove_positive_thickness_graph_geometry_with_max_faces_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: PositiveThicknessGraphLimitsV1,
    max_faces: usize,
) -> Result<NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphProofErrorV1> {
    prove_positive_thickness_graph_geometry_with_max_faces_and_admission_v2(
        geometry,
        pose,
        paper_thickness_mm,
        limits,
        max_faces,
        Some(PositiveThicknessSharedContactScopeV2::LegacyFlatPoseV1),
    )
}

pub(in crate::graph_positive_thickness) fn prove_positive_thickness_graph_geometry_with_max_faces_and_admission_v2(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: PositiveThicknessGraphLimitsV1,
    max_faces: usize,
    admitted_shared_contact: Option<PositiveThicknessSharedContactScopeV2<'_>>,
) -> Result<NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphProofErrorV1> {
    prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpoint_v2(
        geometry,
        pose,
        paper_thickness_mm,
        limits,
        max_faces,
        admitted_shared_contact,
        &mut || Ok(()),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckpointedPositiveThicknessGraphProofErrorV2<S> {
    Geometry(PositiveThicknessGraphProofErrorV1),
    Stopped(S),
}

pub(in crate::graph_positive_thickness) fn prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpointed_v2<
    S,
>(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: PositiveThicknessGraphLimitsV1,
    max_faces: usize,
    admitted_shared_contact: Option<PositiveThicknessSharedContactScopeV2<'_>>,
    checkpoint: &mut impl FnMut() -> Result<(), S>,
) -> Result<
    NativePositiveThicknessGraphGeometryProofV1,
    CheckpointedPositiveThicknessGraphProofErrorV2<S>,
> {
    let mut requested_stop = None;
    let result = {
        let mut proof_checkpoint = || match checkpoint() {
            Ok(()) => Ok(()),
            Err(stop) => {
                requested_stop = Some(stop);
                Err(PositiveThicknessGraphProofErrorV1::InvalidInput)
            }
        };
        prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpoint_v2(
            geometry,
            pose,
            paper_thickness_mm,
            limits,
            max_faces,
            admitted_shared_contact,
            &mut proof_checkpoint,
        )
    };
    match requested_stop {
        Some(stop) => Err(CheckpointedPositiveThicknessGraphProofErrorV2::Stopped(
            stop,
        )),
        None => result.map_err(CheckpointedPositiveThicknessGraphProofErrorV2::Geometry),
    }
}

fn prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpoint_v2(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: PositiveThicknessGraphLimitsV1,
    max_faces: usize,
    admitted_shared_contact: Option<PositiveThicknessSharedContactScopeV2<'_>>,
    checkpoint: &mut impl FnMut() -> Result<(), PositiveThicknessGraphProofErrorV1>,
) -> Result<NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphProofErrorV1> {
    let face_count = geometry.face_ids().len();
    let checked_hinges = pose.closure_certificate().checked_hinges();
    let checked_hinge_set = checked_hinges.iter().copied().collect::<HashSet<_>>();
    if !pose.is_for_geometry(geometry)
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
        || checked_hinges.len() != geometry.hinges().len()
        || checked_hinge_set.len() != geometry.hinges().len()
        || !checked_hinges
            .iter()
            .all(|edge| geometry.hinges().iter().any(|hinge| hinge.edge() == *edge))
        || pose
            .hinge_angles()
            .as_slice()
            .iter()
            .any(|angle| angle.angle_degrees() >= 90.0)
    {
        return Err(PositiveThicknessGraphProofErrorV1::InvalidInput);
    }
    let pair_count = checked_unordered_face_pair_count_v1(
        face_count,
        max_faces,
        limits.max_unordered_face_pairs,
    )?;
    let mut shared_feature_pairs = 0usize;
    for first_index in 0..face_count {
        let first = geometry.face_ids()[first_index];
        for second in &geometry.face_ids()[first_index + 1..] {
            checkpoint()?;
            let first_boundary = geometry
                .face_boundary_vertices(first)
                .filter(|boundary| boundary.len() >= 3)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let second_boundary = geometry
                .face_boundary_vertices(*second)
                .filter(|boundary| boundary.len() >= 3)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            check_positive_thickness_graph_face_boundary_vertex_count_v1(first_boundary.len())?;
            check_positive_thickness_graph_face_boundary_vertex_count_v1(second_boundary.len())?;
            let mut shared = Vec::new();
            shared
                .try_reserve_exact(2)
                .map_err(|_| PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
            for vertex in first_boundary {
                if second_boundary.contains(vertex) {
                    if shared.len() == 2 {
                        return Err(PositiveThicknessGraphProofErrorV1::InvalidInput);
                    }
                    shared.push(*vertex);
                }
            }
            if !shared.is_empty() {
                shared_feature_pairs = shared_feature_pairs
                    .checked_add(1)
                    .ok_or(PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
                check_shared_feature_pair_limit_v1(
                    shared_feature_pairs,
                    limits.max_shared_feature_pairs,
                )?;
                if admitted_shared_contact.is_some_and(|evidence| {
                    evidence.proves_shared_contact_for_pose_v2(geometry, pose)
                }) {
                    // `LegacyFlatPoseV1` preserves only the frozen public V1
                    // zero-pose behavior and is never a basis for new V2
                    // authority. Each V2 scope instead carries its own opaque
                    // exact topology evidence. In every scope this is only an
                    // allowed topology contact, not clearance; continuous
                    // hinge-relief premises remain independently required.
                    continue;
                }
                // With no admitted scope, even an apparent material hinge can
                // originate in a forged public snapshot whose face interiors
                // overlap elsewhere, so the proof remains closed.
                return Err(PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable);
            }
            let bounds = |face, boundary: &[ori_domain::VertexId]| {
                let transform = pose.face_transform(face)?;
                let normal = transform
                    .apply_vector(Point3::new(0.0, 1.0, 0.0).ok()?)
                    .ok()?;
                let mut lower = [f64::INFINITY; 3];
                let mut upper = [f64::NEG_INFINITY; 3];
                for vertex in boundary {
                    let point = transform
                        .apply_point(geometry.vertex_position(*vertex)?)
                        .ok()?;
                    for sign in [-1.0, 1.0] {
                        for (axis, value) in [
                            point.x() + sign * paper_thickness_mm * 0.5 * normal.x(),
                            point.y() + sign * paper_thickness_mm * 0.5 * normal.y(),
                            point.z() + sign * paper_thickness_mm * 0.5 * normal.z(),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            lower[axis] = lower[axis].min(value);
                            upper[axis] = upper[axis].max(value);
                        }
                    }
                }
                Some((lower, upper))
            };
            let (first_lower, first_upper) = bounds(first, first_boundary)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let (second_lower, second_upper) = bounds(*second, second_boundary)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let exact_lower = [
                exact_input_rational_v1(first_lower[0].max(second_lower[0]))?,
                exact_input_rational_v1(first_lower[1].max(second_lower[1]))?,
                exact_input_rational_v1(first_lower[2].max(second_lower[2]))?,
            ];
            let exact_upper = [
                exact_input_rational_v1(first_upper[0].min(second_upper[0]))?,
                exact_input_rational_v1(first_upper[1].min(second_upper[1]))?,
                exact_input_rational_v1(first_upper[2].min(second_upper[2]))?,
            ];
            if (0..3).any(|axis| exact_lower[axis] > exact_upper[axis]) {
                continue;
            }
            if shared.is_empty() {
                let world_polygon = |face, boundary: &[ori_domain::VertexId]| {
                    let transform = pose
                        .face_transform(face)
                        .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                    let mut polygon = Vec::new();
                    polygon
                        .try_reserve_exact(boundary.len())
                        .map_err(|_| PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
                    for vertex in boundary {
                        let point = geometry
                            .vertex_position(*vertex)
                            .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                        polygon.push(
                            transform
                                .apply_point(point)
                                .map_err(|_| PositiveThicknessGraphProofErrorV1::InvalidInput)?,
                        );
                    }
                    Ok::<_, PositiveThicknessGraphProofErrorV1>(polygon)
                };
                let first_polygon = world_polygon(first, first_boundary)?;
                let second_polygon = world_polygon(*second, second_boundary)?;
                let prism_separated = {
                    let prism = |face, polygon: &[Point3]| {
                        let transform = pose
                            .face_transform(face)
                            .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                        let normal =
                            transform
                                .apply_vector(Point3::new(0.0, 1.0, 0.0).map_err(|_| {
                                    PositiveThicknessGraphProofErrorV1::InvalidInput
                                })?)
                                .map_err(|_| PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                        let vertex_capacity = polygon
                            .len()
                            .checked_mul(2)
                            .ok_or(PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
                        let mut vertices = Vec::new();
                        vertices
                            .try_reserve_exact(vertex_capacity)
                            .map_err(|_| PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
                        for point in polygon {
                            for sign in [-1.0, 1.0] {
                                vertices.push((
                                    point.x() + sign * paper_thickness_mm * 0.5 * normal.x(),
                                    point.y() + sign * paper_thickness_mm * 0.5 * normal.y(),
                                    point.z() + sign * paper_thickness_mm * 0.5 * normal.z(),
                                ));
                            }
                        }
                        let mut edges = Vec::new();
                        edges
                            .try_reserve_exact(
                                polygon
                                    .len()
                                    .checked_add(1)
                                    .ok_or(PositiveThicknessGraphProofErrorV1::ResourceLimit)?,
                            )
                            .map_err(|_| PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
                        for index in 0..polygon.len() {
                            let start = polygon[index];
                            let end = polygon[(index + 1) % polygon.len()];
                            edges.push((
                                end.x() - start.x(),
                                end.y() - start.y(),
                                end.z() - start.z(),
                            ));
                        }
                        edges.push((normal.x(), normal.y(), normal.z()));
                        Ok::<_, PositiveThicknessGraphProofErrorV1>((normal, vertices, edges))
                    };
                    let (first_normal, first_vertices, first_edges) = prism(first, &first_polygon)?;
                    let (second_normal, second_vertices, second_edges) =
                        prism(*second, &second_polygon)?;
                    let cross = |a: (f64, f64, f64), b: (f64, f64, f64)| {
                        (
                            a.1 * b.2 - a.2 * b.1,
                            a.2 * b.0 - a.0 * b.2,
                            a.0 * b.1 - a.1 * b.0,
                        )
                    };
                    let first_normal_axis = (first_normal.x(), first_normal.y(), first_normal.z());
                    let second_normal_axis =
                        (second_normal.x(), second_normal.y(), second_normal.z());
                    let separates = |axis: (f64, f64, f64)| {
                        let squared = axis.0 * axis.0 + axis.1 * axis.1 + axis.2 * axis.2;
                        if !squared.is_finite() || squared <= f64::EPSILON {
                            return false;
                        }
                        let interval = |vertices: &[(f64, f64, f64)]| {
                            vertices.iter().try_fold(
                                (None::<BigRational>, None::<BigRational>),
                                |(lower, upper), vertex| {
                                    let value = BigRational::from_float(vertex.0)?
                                        * BigRational::from_float(axis.0)?
                                        + BigRational::from_float(vertex.1)?
                                            * BigRational::from_float(axis.1)?
                                        + BigRational::from_float(vertex.2)?
                                            * BigRational::from_float(axis.2)?;
                                    Some((
                                        Some(lower.map_or_else(
                                            || value.clone(),
                                            |current| current.min(value.clone()),
                                        )),
                                        Some(upper.map_or_else(
                                            || value.clone(),
                                            |current| current.max(value.clone()),
                                        )),
                                    ))
                                },
                            )
                        };
                        let Some((first_min, first_max)) = interval(&first_vertices) else {
                            return false;
                        };
                        let Some((second_min, second_max)) = interval(&second_vertices) else {
                            return false;
                        };
                        first_max < second_min || second_max < first_min
                    };
                    let axes = [first_normal_axis, second_normal_axis]
                        .into_iter()
                        .chain(
                            first_edges
                                .iter()
                                .map(|edge| cross(*edge, first_normal_axis)),
                        )
                        .chain(
                            second_edges
                                .iter()
                                .map(|edge| cross(*edge, second_normal_axis)),
                        )
                        .chain(first_edges.iter().flat_map(|first_edge| {
                            second_edges
                                .iter()
                                .map(move |second_edge| cross(*first_edge, *second_edge))
                        }));
                    let mut separated = false;
                    for axis in axes {
                        checkpoint()?;
                        if separates(axis) {
                            separated = true;
                            break;
                        }
                    }
                    separated
                };
                // An absence of boundary crossings is not separation: one
                // polygon can contain the other, and collinear/touching
                // boundaries also overlap once positive thickness is added.
                // Accept only an explicit strict separating axis.
                if prism_separated {
                    continue;
                }
                return Err(PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable);
            }
        }
    }
    Ok(NativePositiveThicknessGraphGeometryProofV1 {
        identity: Arc::new(()),
        geometry: geometry.clone(),
        pose: pose.clone(),
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        analyzed_unordered_face_pairs: pair_count,
    })
}

#[cfg(test)]
#[allow(clippy::duplicate_mod)]
#[path = "../../../test-support/four_bay_cycle.rs"]
mod four_bay_cycle_test_support;

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};
    use ori_kinematics::{
        CanonicalCycleScheduleV1, CanonicalHingeAngles, CycleScheduleEntryInputV1,
        CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, HingeAngle, MaterialHingeGraphAudit,
        MaterialHingeGraphGeometry, RationalCoefficientV1, TreeKinematicsLimits,
        admit_canonical_multi_hinge_path_candidate_v1,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    #[test]
    fn positive_thickness_graph_face_boundary_vertex_cap_is_inclusive_and_fail_closed_v1() {
        assert_eq!(
            check_positive_thickness_graph_face_boundary_vertex_count_v1(
                POSITIVE_THICKNESS_GRAPH_MAX_FACE_BOUNDARY_VERTICES_V1,
            ),
            Ok(())
        );
        assert_eq!(
            check_positive_thickness_graph_face_boundary_vertex_count_v1(
                POSITIVE_THICKNESS_GRAPH_MAX_FACE_BOUNDARY_VERTICES_V1 + 1,
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn common_articulation_extension_limits_are_fixed_hard_and_fail_closed_v1() {
        let exact = CommonArticulationPositiveThicknessGraphExtensionLimitsV1::fixed_v1();
        assert_eq!(
            exact.max_unordered_face_pairs,
            COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_UNORDERED_FACE_PAIRS_V1
        );
        assert_eq!(
            exact.max_shared_feature_pairs,
            COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_SHARED_FEATURE_PAIRS_V1
        );
        assert_eq!(
            checked_unordered_face_pair_count_v1(
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
                exact.max_unordered_face_pairs,
            ),
            Ok(32_896)
        );
        assert_eq!(
            check_shared_feature_pair_limit_v1(32_896, exact.max_shared_feature_pairs,),
            Ok(())
        );

        assert_eq!(
            checked_unordered_face_pair_count_v1(
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
                exact.max_unordered_face_pairs - 1,
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        );
        assert_eq!(
            check_shared_feature_pair_limit_v1(32_896, exact.max_shared_feature_pairs - 1,),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        );
        assert_eq!(
            checked_unordered_face_pair_count_v1(
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1 + 1,
                COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
                exact.max_unordered_face_pairs,
            ),
            Err(PositiveThicknessGraphProofErrorV1::InvalidInput)
        );
        assert_eq!(
            checked_unordered_face_pair_count_v1(usize::MAX, usize::MAX, usize::MAX),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        );

        assert_eq!(
            validate_common_articulation_positive_thickness_graph_extension_limits_v1(
                CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
                    max_unordered_face_pairs: exact.max_unordered_face_pairs + 1,
                    ..exact
                }
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        );
        assert_eq!(
            validate_common_articulation_positive_thickness_graph_extension_limits_v1(
                CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
                    max_shared_feature_pairs: exact.max_shared_feature_pairs + 1,
                    ..exact
                }
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        );

        // The legacy surface stays at 97 faces and its established default.
        assert_eq!(
            PositiveThicknessGraphLimitsV1::default(),
            PositiveThicknessGraphLimitsV1 {
                max_unordered_face_pairs: 4_656,
                max_shared_feature_pairs: 4_656,
            }
        );
        assert_eq!(
            checked_unordered_face_pair_count_v1(98, 97, usize::MAX),
            Err(PositiveThicknessGraphProofErrorV1::InvalidInput)
        );
    }

    fn theta_shared_hinge_pattern() -> (CreasePattern, Paper) {
        let namespace = ProjectId::new();
        let points = [
            (-3.0, 0.0),
            (-1.0, -2.0),
            (1.0, -2.0),
            (3.0, 0.0),
            (1.0, 2.0),
            (-1.0, 2.0),
            (-1.0, 0.0),
            (1.0, 0.0),
        ];
        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..6]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let mut edges = (0..6)
            .map(|index| Edge {
                id: ori_domain::EdgeId::derive_v5(namespace, &[0x10, index as u8]),
                start: boundary[index],
                end: boundary[(index + 1) % 6],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (index, (start, end)) in [(6, 0), (6, 1), (6, 5), (6, 7), (7, 2), (7, 3), (7, 4)]
            .into_iter()
            .enumerate()
        {
            edges.push(Edge {
                id: ori_domain::EdgeId::derive_v5(namespace, &[0x20, index as u8]),
                start: vertices[start].id,
                end: vertices[end].id,
                kind: if matches!(index, 0 | 3 | 5) {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        )
    }

    fn three_by_three_dense_cycle_pattern() -> (CreasePattern, Paper) {
        let namespace = ProjectId::new();
        let vertices = (0..4)
            .flat_map(|y| {
                (0..4).map(move |x| Vertex {
                    id: VertexId::derive_v5(namespace, &[0x31, y, x]),
                    position: Point2::new(f64::from(x), f64::from(y)),
                })
            })
            .collect::<Vec<_>>();
        let vertex = |x: usize, y: usize| vertices[y * 4 + x].id;
        let mut edges = Vec::new();
        for y in 0..4 {
            for x in 0..3 {
                edges.push(Edge {
                    id: ori_domain::EdgeId::derive_v5(namespace, &[0x32, y as u8, x as u8]),
                    start: vertex(x, y),
                    end: vertex(x + 1, y),
                    kind: if y == 0 || y == 3 {
                        EdgeKind::Boundary
                    } else {
                        EdgeKind::Mountain
                    },
                });
            }
        }
        for x in 0..4 {
            for y in 0..3 {
                edges.push(Edge {
                    id: ori_domain::EdgeId::derive_v5(namespace, &[0x33, x as u8, y as u8]),
                    start: vertex(x, y),
                    end: vertex(x, y + 1),
                    kind: if x == 0 || x == 3 {
                        EdgeKind::Boundary
                    } else {
                        EdgeKind::Valley
                    },
                });
            }
        }
        let boundary_vertices = (0..4)
            .map(|x| vertex(x, 0))
            .chain((1..4).map(|y| vertex(3, y)))
            .chain((0..3).rev().map(|x| vertex(x, 3)))
            .chain((1..3).rev().map(|y| vertex(0, y)))
            .collect();
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices,
                thickness_mm: 0.1,
                ..Paper::default()
            },
        )
    }

    #[test]
    fn dense_rank_four_graph_sampling_does_not_replace_a_continuous_theorem() {
        let (pattern, paper) = three_by_three_dense_cycle_pattern();
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: ProjectId::new(),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("three-by-three material grid");
        assert_eq!(
            (topology.faces.len(), topology.hinge_adjacency.len()),
            (9, 12)
        );
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), 4, "cycle rank exceeds theta");
        let fixed = geometry.face_ids()[0];
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    let moving = (hinge.end().z() - hinge.start().z()).abs() > 0.5;
                    CycleScheduleEntryInputV1 {
                        edge: hinge.edge(),
                        initial_angle_degrees_bits: if moving {
                            15.0_f64.to_bits()
                        } else {
                            0.0_f64.to_bits()
                        },
                        chebyshev_coefficients: if moving {
                            vec![
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 15,
                                    denominator: 1,
                                },
                            ]
                        } else {
                            vec![RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            }]
                        },
                    }
                })
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        for progress in [0.0, 0.25, 0.5, 1.0] {
            let trial = schedule.evaluate(progress).unwrap();
            geometry
                .solve_closed(&audit, fixed, &trial, 1.0e-8)
                .unwrap_or_else(|error| panic!("dense grid closes at {progress}: {error:?}"));
        }
        assert_eq!(schedule.collective_profile_edges_v1().unwrap().len(), 6);
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .expect("stationary dense graph has exact one-leaf closure");
        let angles = schedule.evaluate(0.0).unwrap();
        let diagnosis = crate::diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            paper.thickness_mm,
            1,
        );
        assert!(
            diagnosis.continuous_certificate_model_id().is_none(),
            "static samples and closure alone must not mint positive-thickness continuous authority"
        );

        let pose = geometry.solve_closed(&audit, fixed, &angles, 0.0).unwrap();
        assert!(matches!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                paper.thickness_mm,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: 35,
                    max_shared_feature_pairs: 36
                },
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        ));
        let proof = prove_positive_thickness_graph_geometry_v1(
            &geometry,
            &pose,
            paper.thickness_mm,
            PositiveThicknessGraphLimitsV1 {
                max_unordered_face_pairs: 36,
                max_shared_feature_pairs: 36,
            },
        )
        .unwrap();
        assert_eq!(proof.analyzed_unordered_face_pairs(), 36);
        let extension_proof =
            prove_common_articulation_positive_thickness_graph_geometry_extension_v1(
                &geometry,
                &pose,
                paper.thickness_mm,
                CommonArticulationPositiveThicknessGraphExtensionLimitsV1::fixed_v1(),
            )
            .expect("the separately scoped extension shares the native proof internals");
        assert_eq!(extension_proof.analyzed_unordered_face_pairs(), 36);
        assert!(!proof.is_for_geometry(
            &geometry,
            &pose,
            f64::from_bits(paper.thickness_mm.to_bits() + 1)
        ));

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum TestStopV2 {
            Cancelled,
            DeadlineExceeded,
        }
        for stop in [TestStopV2::Cancelled, TestStopV2::DeadlineExceeded] {
            let mut polls = 0usize;
            let stopped =
                prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpointed_v2(
                    &geometry,
                    &pose,
                    paper.thickness_mm,
                    PositiveThicknessGraphLimitsV1 {
                        max_unordered_face_pairs: 36,
                        max_shared_feature_pairs: 36,
                    },
                    97,
                    Some(PositiveThicknessSharedContactScopeV2::LegacyFlatPoseV1),
                    &mut || {
                        polls += 1;
                        if polls == 4 { Err(stop) } else { Ok(()) }
                    },
                )
                .expect_err("a mid-pair-scan stop suppresses the graph proof");
            assert_eq!(
                stopped,
                CheckpointedPositiveThicknessGraphProofErrorV2::Stopped(stop)
            );
            assert_eq!(polls, 4);
        }
    }

    #[test]
    fn real_theta_shared_hinge_static_proof_checks_every_face_pair_once() {
        let (pattern, mut paper) = theta_shared_hinge_pattern();
        paper.thickness_mm = 0.1;
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: ProjectId::new(),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("two physical vertices sharing one hinge form a theta dual graph");
        assert_eq!(topology.faces.len(), 6);
        assert_eq!(topology.hinge_adjacency.len(), 7);
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), 2);
        let angles = CanonicalHingeAngles::new(
            geometry
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = geometry
            .solve_closed(&audit, geometry.face_ids()[0], &angles, 0.0)
            .unwrap();
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            geometry.face_ids()[0],
            [0.0, 1.0],
            geometry
                .hinges()
                .iter()
                .map(|hinge| CycleScheduleEntryInputV1 {
                    edge: hinge.edge(),
                    initial_angle_degrees_bits: if hinge.assignment()
                        == ori_topology::FoldAssignment::Mountain
                    {
                        15.0_f64.to_bits()
                    } else {
                        0.0_f64.to_bits()
                    },
                    chebyshev_coefficients: if hinge.assignment()
                        == ori_topology::FoldAssignment::Mountain
                    {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 15,
                                denominator: 1,
                            },
                        ]
                    } else {
                        vec![RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                })
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        for progress in [0.25, 0.5, 1.0] {
            let scheduled = schedule.evaluate(progress).unwrap();
            geometry
                .solve_closed(&audit, geometry.face_ids()[0], &scheduled, 1.0e-8)
                .unwrap_or_else(|error| {
                    panic!("theta schedule must close at {progress}: {error:?}")
                });
        }
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                geometry.face_ids()[0],
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .expect("exact theta opposite-pair interval theorem");
        assert_eq!(closure.leaves().len(), 1);
        for one_short in [
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 0,
                max_work: 1,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 0,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(
                    &audit,
                    geometry.face_ids()[0],
                    &schedule,
                    1.0e-8,
                    one_short,
                ),
                Err(ori_kinematics::DyadicIntervalClosureErrorV1::InvalidInput)
            );
        }
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate =
            admit_canonical_multi_hinge_path_candidate_v1(schedule.clone(), &initial, &requested)
                .unwrap();
        for thickness in [0.1, 1.0, 3.0] {
            let continuous = crate::diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry,
                &audit,
                geometry.face_ids()[0],
                &candidate,
                &closure,
                thickness,
                32,
            );
            assert!(
                continuous.continuous_certificate_model_id().is_none(),
                "theta endpoint closure and static proof do not establish swept solid clearance"
            );
            assert!(
                crate::diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry,
                    &audit,
                    geometry.face_ids()[0],
                    &candidate,
                    &closure,
                    thickness,
                    0,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
        let collision_schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            geometry.face_ids()[0],
            [0.0, 1.0],
            geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    let moves = hinge.assignment() == ori_topology::FoldAssignment::Mountain;
                    CycleScheduleEntryInputV1 {
                        edge: hinge.edge(),
                        initial_angle_degrees_bits: if moves {
                            45.0_f64.to_bits()
                        } else {
                            0.0_f64.to_bits()
                        },
                        chebyshev_coefficients: if moves {
                            vec![
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 45,
                                    denominator: 1,
                                },
                            ]
                        } else {
                            vec![RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            }]
                        },
                    }
                })
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let collision_closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                geometry.face_ids()[0],
                &collision_schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        let collision_initial = collision_schedule.evaluate(0.0).unwrap();
        let collision_target = collision_schedule.evaluate(1.0).unwrap();
        let collision_candidate = admit_canonical_multi_hinge_path_candidate_v1(
            collision_schedule,
            &collision_initial,
            &collision_target,
        )
        .unwrap();
        assert!(
            crate::diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry,
                &audit,
                geometry.face_ids()[0],
                &collision_candidate,
                &collision_closure,
                0.1,
                32,
            )
            .continuous_certificate_model_id()
            .is_none(),
            "the thickness singularity at 90 degrees must issue no swept certificate"
        );
        for thickness in [0.1, 1.0, 3.0] {
            let proof = prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                thickness,
                PositiveThicknessGraphLimitsV1::default(),
            )
            .expect("flat real theta positive-thickness proof");
            assert_eq!(proof.face_count(), 6);
            assert_eq!(proof.analyzed_unordered_face_pairs(), 15);
            assert_eq!(proof.paper_thickness_bits(), thickness.to_bits());
            assert!(!proof.is_for_geometry(
                &geometry,
                &pose,
                f64::from_bits(thickness.to_bits() + 1),
            ));
        }
        assert_eq!(pose.closure_certificate().checked_hinges().len(), 7);
        let shared_hinge = geometry
            .hinges()
            .iter()
            .find(|hinge| {
                geometry
                    .hinges()
                    .iter()
                    .filter(|candidate| {
                        candidate.start() == hinge.start() || candidate.end() == hinge.start()
                    })
                    .count()
                    >= 4
                    && geometry
                        .hinges()
                        .iter()
                        .filter(|candidate| {
                            candidate.start() == hinge.end() || candidate.end() == hinge.end()
                        })
                        .count()
                        >= 4
            })
            .expect("unique hinge joining both degree-four physical vertices")
            .edge();
        let damaged_angles = CanonicalHingeAngles::new(
            geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    HingeAngle::new(
                        hinge.edge(),
                        if hinge.edge() == shared_hinge {
                            1.0
                        } else {
                            0.0
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        assert!(
            geometry
                .solve_closed(&audit, geometry.face_ids()[0], &damaged_angles, 0.0)
                .is_err(),
            "damaged shared theta hinge must issue neither closed pose nor thickness proof"
        );
        assert!(matches!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                paper.thickness_mm,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: 14,
                    ..PositiveThicknessGraphLimitsV1::default()
                },
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        ));
    }

    #[test]
    fn two_to_sixteen_cycle_cactus_proof_is_instance_bound_and_resource_bounded() {
        for group_count in [2, 3, 16] {
            let (pattern, paper, hinges) = match group_count {
                2 => super::four_bay_cycle_test_support::two_bay_rational_cycle_pattern(),
                3 => super::four_bay_cycle_test_support::three_bay_rational_cycle_pattern(),
                _ => super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern(),
            };
            let topology = analyze_faces(FaceExtractionInput {
                identity_namespace: ori_domain::ProjectId::new(),
                source_revision: 1,
                paper: &paper,
                pattern: &pattern,
            })
            .snapshot
            .expect("three-cycle cactus topology");
            let geometry = MaterialHingeGraphGeometry::prepare(
                &pattern,
                &paper,
                &topology,
                TreeKinematicsLimits::default(),
            )
            .unwrap();
            let audit =
                MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
                    .unwrap();
            let fixed = geometry.face_ids()[0];
            let mut angles = hinges
                .iter()
                .copied()
                .map(|edge| HingeAngle::new(edge, 0.0).unwrap())
                .collect::<Vec<_>>();
            angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
            let pose = geometry
                .solve_closed(
                    &audit,
                    fixed,
                    &CanonicalHingeAngles::new(angles).unwrap(),
                    1.0e-9,
                )
                .unwrap();
            let proof = prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1::default(),
            )
            .expect("cactus exact-AABB proof");
            assert!(proof.is_for_geometry(&geometry, &pose, 0.1));
            for thickness in [1.0, 3.0] {
                assert!(
                    prove_positive_thickness_graph_geometry_v1(
                        &geometry,
                        &pose,
                        thickness,
                        PositiveThicknessGraphLimitsV1::default(),
                    )
                    .is_ok(),
                    "cactus group {group_count} supports {thickness} mm"
                );
            }
            assert!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    10_000.0,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .is_ok(),
                "an exact-flat extrusion remains partitioned in-plane; an unrelated hinge-length \
                 corridor must not impose a synthetic thickness cutoff"
            );
            let expected_pairs = geometry.face_ids().len() * (geometry.face_ids().len() - 1) / 2;
            assert_eq!(proof.analyzed_unordered_face_pairs(), expected_pairs);
            assert!(matches!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1 {
                        max_unordered_face_pairs: expected_pairs - 1,
                        ..PositiveThicknessGraphLimitsV1::default()
                    },
                ),
                Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
            ));
            let foreign = geometry.clone();
            assert!(proof.is_for_geometry(&foreign, &pose, 0.1));
            assert!(proof.same_proof(&proof.clone()));
            if group_count == 2 {
                let mut aba_angles = hinges
                    .iter()
                    .copied()
                    .map(|edge| HingeAngle::new(edge, 0.0).unwrap())
                    .collect::<Vec<_>>();
                aba_angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
                let aba_pose = geometry
                    .solve_closed(
                        &audit,
                        fixed,
                        &CanonicalHingeAngles::new(aba_angles).unwrap(),
                        1.0e-9,
                    )
                    .unwrap();
                assert!(!proof.is_for_geometry(&geometry, &aba_pose, 0.1));
                let aba_proof = prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &aba_pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .expect("a freshly issued same-geometry pose remains admissible");
                assert!(aba_proof.is_for_geometry(&geometry, &aba_pose, 0.1));
                assert!(!aba_proof.is_for_geometry(&geometry, &pose, 0.1));
                let second_proof = prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .unwrap();
                assert!(!proof.same_proof(&second_proof));
                assert!(matches!(
                    prove_positive_thickness_graph_geometry_v1(
                        &geometry,
                        &pose,
                        0.0,
                        PositiveThicknessGraphLimitsV1::default(),
                    ),
                    Err(PositiveThicknessGraphProofErrorV1::InvalidInput)
                ));
            }
            let separately_prepared = MaterialHingeGraphGeometry::prepare(
                &pattern,
                &paper,
                &topology,
                TreeKinematicsLimits::default(),
            )
            .unwrap();
            assert!(!proof.is_for_geometry(&separately_prepared, &pose, 0.1));
            let separately_prepared_audit =
                MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
                    .unwrap();
            let mut separately_prepared_angles = hinges
                .iter()
                .copied()
                .map(|edge| HingeAngle::new(edge, 0.0).unwrap())
                .collect::<Vec<_>>();
            separately_prepared_angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
            let separately_prepared_pose = separately_prepared
                .solve_closed(
                    &separately_prepared_audit,
                    separately_prepared.face_ids()[0],
                    &CanonicalHingeAngles::new(separately_prepared_angles).unwrap(),
                    1.0e-9,
                )
                .unwrap();
            assert!(matches!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &separately_prepared_pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1::default(),
                ),
                Err(PositiveThicknessGraphProofErrorV1::InvalidInput)
            ));
        }
    }
}
