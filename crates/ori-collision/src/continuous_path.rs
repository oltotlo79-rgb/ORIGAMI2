//! Bounded observation of a collective-hinge path.
//!
//! Sampling is deliberately not presented as CCD proof.  The result can find
//! a blocking sampled pose and can recommend the authenticated initial pose as
//! a fail-closed hold, but it never certifies the open intervals between
//! samples or authorizes mutation.

use std::collections::{HashMap, HashSet, VecDeque};

use num_rational::BigRational;
use num_traits::{FromPrimitive, ToPrimitive};
use ori_domain::{EdgeId, FaceId};
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalHingeAngles, DyadicMaterialHingeIntervalClosureCertificateV1,
    GeneratedMultiHingePathCandidateV1, HingeAngle, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

use crate::cayley::{
    prepare_positive_thickness_exact_endpoint_session_v2,
    prepare_swept_tree_hinge_thickness_boundaries_v1,
};
use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1, HingeReliefLinearAngleScheduleV1,
    HingeReliefPolicyLimitsV1, HingeReliefPolicyRecordV1,
    NativeHingeReliefLocalIntervalCertificateV1, NativeHingeReliefPrerequisiteV1,
    PositiveThicknessGraphLimitsV1, StaticCollisionDiagnosticSnapshot, StaticCollisionLimits,
    diagnose_static_collision_geometry_with_control_v1,
    prepare_positive_thickness_pair_separation_v1, prepare_single_hinge_thickness_boundary_v1,
    prove_positive_thickness_graph_geometry_v1, revalidate_hinge_relief_local_intervals_v1,
    revalidate_positive_thickness_pair_separation_v1,
    revalidate_single_hinge_thickness_boundary_v1, revalidate_tree_hinge_thickness_boundaries_v1,
    static_collision::prepare_positive_thickness_tree_endpoint_topology_memo_v1,
};

mod initial_sample_layer_admission;
mod layered_chain_common;
mod layered_four_face_chain;
mod layered_three_face;
mod multi_hinge_union;
mod pair_proof_cache;
pub use initial_sample_layer_admission::{
    NativeStackedFoldInitialSampleLayerAdmissionV1, StackedFoldInitialLayerOrderSourceV1,
    prepare_stacked_fold_initial_sample_layer_admission_v1,
    prepare_stacked_fold_initial_sample_layer_admission_with_control_v1,
};
use initial_sample_layer_admission::{
    SampledLayerAdmissionSnapshotV1, initial_sample_layer_admission_has_issuer_v1,
    initial_sample_layer_admission_matches_snapshot_v1,
    retain_initial_sample_layer_admission_issuer_v1, sampled_layer_admission_matches_snapshot_v1,
};
pub use layered_four_face_chain::{
    LAYERED_FOUR_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    LayeredFourFaceChainContinuousCertificateV1, LayeredFourFaceChainContinuousErrorV1,
    LayeredFourFaceChainContinuousLimitsV1, certify_layered_four_face_chain_continuous_path_v1,
    certify_layered_four_face_chain_continuous_path_with_control_v1,
};
pub use layered_three_face::{
    LAYERED_THREE_FACE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1, LayeredThreeFaceContinuousCertificateV1,
    LayeredThreeFaceContinuousErrorV1, LayeredThreeFaceContinuousLimitsV1,
    certify_layered_three_face_continuous_path_v1,
    certify_layered_three_face_continuous_path_with_control_v1,
};
pub use multi_hinge_union::{
    MAX_MULTI_HINGE_UNION_GEOMETRY_HINGES_V2, MAX_MULTI_HINGE_UNION_HINGES_V2,
    MAX_MULTI_HINGE_UNION_PAIRS_V2, MAX_MULTI_HINGE_UNION_STORAGE_BYTES_V2,
    MAX_MULTI_HINGE_UNION_WORK_V2, MAX_MULTI_HINGES_PER_FACE_PAIR_V2,
    MULTI_HINGE_RELIEF_UNION_CERTIFICATE_MODEL_ID_V2, MULTI_HINGE_RELIEF_UNION_GAP_MODEL_ID_V2,
    MultiHingeReliefUnionCertificateV2, MultiHingeReliefUnionCoveredPairV2,
    MultiHingeReliefUnionErrorV2, MultiHingeReliefUnionGapReportV2, MultiHingeReliefUnionGapV2,
    MultiHingeReliefUnionHingeGapV2, MultiHingeReliefUnionLimitsV2,
    SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_MODEL_ID_V1,
    SplitHingeUnionExteriorReliefAssumptionErrorV1,
    SplitHingeUnionExteriorReliefAssumptionLimitsV1, SplitHingeUnionExteriorReliefAssumptionV1,
    certify_multi_hinge_relief_union_v2, certify_multi_hinge_relief_union_with_cancel_v2,
    diagnose_multi_hinge_relief_union_gaps_v2,
    diagnose_multi_hinge_relief_union_gaps_with_cancel_v2,
    prove_split_hinge_union_exterior_relief_assumption_v1,
    revalidate_multi_hinge_relief_union_certificate_v2,
    revalidate_split_hinge_union_exterior_relief_assumption_v1,
};
pub use pair_proof_cache::diagnose_collective_hinge_path_with_pair_cache_v1;
use pair_proof_cache::{
    PositiveEndpointPairCacheUseV1, prove_positive_endpoint_pairs_with_cache_v1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExactDyadicPathIntersectionErrorV1 {
    #[error("exact path intersection work exceeds its bound")]
    ResourceLimit,
    #[error("exact path intersection was cancelled")]
    Cancelled,
    #[error("exact path segment is invalid")]
    InvalidSegment,
}

pub fn classify_exact_dyadic_path_self_intersection_v1(
    segments: &[crate::DyadicSegmentV1],
    limits: crate::ExactDyadicIntersectionLimitsV1,
    max_pair_tests: usize,
) -> Result<Option<(usize, usize, crate::ExactSegmentRelationV1)>, ExactDyadicPathIntersectionErrorV1>
{
    classify_exact_dyadic_path_self_intersection_with_cancel_v1(
        segments,
        limits,
        max_pair_tests,
        || false,
    )
}

pub fn classify_exact_dyadic_path_self_intersection_with_cancel_v1(
    segments: &[crate::DyadicSegmentV1],
    limits: crate::ExactDyadicIntersectionLimitsV1,
    max_pair_tests: usize,
    cancelled: impl Fn() -> bool,
) -> Result<Option<(usize, usize, crate::ExactSegmentRelationV1)>, ExactDyadicPathIntersectionErrorV1>
{
    let required = segments
        .len()
        .checked_mul(segments.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(ExactDyadicPathIntersectionErrorV1::ResourceLimit)?;
    if required > max_pair_tests {
        return Err(ExactDyadicPathIntersectionErrorV1::ResourceLimit);
    }
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            if cancelled() {
                return Err(ExactDyadicPathIntersectionErrorV1::Cancelled);
            }
            let relation = crate::classify_exact_dyadic_segment_intersection_v1(
                segments[first],
                segments[second],
                limits,
            )
            .map_err(|error| match error {
                crate::ExactDyadicIntersectionErrorV1::ResourceLimit => {
                    ExactDyadicPathIntersectionErrorV1::ResourceLimit
                }
                crate::ExactDyadicIntersectionErrorV1::Degenerate => {
                    ExactDyadicPathIntersectionErrorV1::InvalidSegment
                }
            })?;
            if matches!(
                relation,
                crate::ExactSegmentRelationV1::ProperCrossing
                    | crate::ExactSegmentRelationV1::CollinearOverlap
            ) {
                return Ok(Some((first, second, relation)));
            }
        }
    }
    Ok(None)
}

pub const STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1: &str =
    "stacked_fold_bounded_path_diagnostic_v1";
pub const STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_single_hinge_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2: &str =
    "stacked_fold_single_hinge_positive_thickness_continuous_certificate_v2";
pub const STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_collinear_tree_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2: &str =
    "stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v2";
pub const STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_two_hinge_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_tree_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_cycle_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_cactus_positive_thickness_continuous_certificate_v1";
pub const MAX_STACKED_FOLD_PATH_SAMPLES_V1: usize = 64;
const MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1: usize = 120;
const MAX_POSITIVE_ENDPOINT_TREE_FACES_V1: usize = 64;
pub const MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1: usize = 64;
const MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1: usize = 2_048;
const MAX_STACKED_FOLD_INTERVAL_LEAVES_V1: usize = 128;
const MAX_STACKED_FOLD_INTERVAL_DEPTH_V1: usize = 7;
const MAX_STACKED_FOLD_INTERVAL_WORK_V1: usize =
    MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 * MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1;
pub const MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1: usize = 2_080;
pub const MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1: usize = 128;

#[derive(Debug, Clone)]
pub struct DyadicFaceTransformIntervalLeafV1 {
    depth: u32,
    index: u64,
    transforms: ori_kinematics::MaterialFaceTransformIntervalRegistryV1,
}
impl DyadicFaceTransformIntervalLeafV1 {
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }
    #[must_use]
    pub fn transforms(&self) -> &ori_kinematics::MaterialFaceTransformIntervalRegistryV1 {
        &self.transforms
    }
}

#[derive(Debug, Clone)]
pub struct DyadicFaceTransformIntervalRegistryV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    tolerance_bits: u64,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    max_work_per_leaf: usize,
    leaves: Vec<DyadicFaceTransformIntervalLeafV1>,
}
#[derive(Clone, Copy)]
pub struct DyadicFaceTransformBindingInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub fixed_face: FaceId,
    pub schedule: &'a ori_kinematics::CanonicalCycleScheduleV1,
    pub closure: &'a ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    pub thickness_mm: f64,
    pub tolerance: f64,
    pub schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    pub max_work_per_leaf: usize,
}
impl DyadicFaceTransformIntervalRegistryV1 {
    #[must_use]
    pub fn leaves(&self) -> &[DyadicFaceTransformIntervalLeafV1] {
        &self.leaves
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(&self, input: DyadicFaceTransformBindingInputV1<'_>) -> bool {
        let DyadicFaceTransformBindingInputV1 {
            geometry,
            audit,
            fixed_face,
            schedule,
            closure,
            thickness_mm,
            tolerance,
            schedule_limits,
            max_work_per_leaf,
        } = input;
        self.issuer.same_instance(geometry)
            && self.fixed_face == fixed_face
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == thickness_mm.to_bits()
            && self.tolerance_bits == tolerance.to_bits()
            && self.schedule_limits == schedule_limits
            && self.max_work_per_leaf == max_work_per_leaf
            && schedule.matches_binding(geometry, audit, fixed_face)
            && closure.every_leaf_covers_graph_v1(geometry)
            && self.leaves.iter().all(|leaf| {
                schedule
                    .evaluate_angle_box_dyadic(leaf.depth, leaf.index, schedule_limits)
                    .is_ok_and(|boxes| {
                        leaf.transforms.is_for(
                            geometry,
                            audit,
                            fixed_face,
                            &boxes,
                            tolerance,
                            max_work_per_leaf,
                        )
                    })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DyadicFaceTransformIntervalErrorV1 {
    #[error("dyadic face transform binding is invalid")]
    InvalidBinding,
    #[error("dyadic face transform work exceeds its hard limit")]
    ResourceLimit,
    #[error("dyadic face transform interval could not be proven")]
    Unproven,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharedVertexIntervalPositionV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
    positions: [[ori_kinematics::OutwardIntervalV1; 3]; 2],
}
impl SharedVertexIntervalPositionV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn vertex(&self) -> ori_domain::VertexId {
        self.vertex
    }
    #[must_use]
    pub const fn positions(&self) -> [[ori_kinematics::OutwardIntervalV1; 3]; 2] {
        self.positions
    }
}

#[derive(Debug, Clone)]
pub struct DyadicSharedVertexIntervalDiagnosticLeafV1 {
    depth: u32,
    index: u64,
    positions: Vec<SharedVertexIntervalPositionV1>,
}
impl DyadicSharedVertexIntervalDiagnosticLeafV1 {
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }
    #[must_use]
    pub fn positions(&self) -> &[SharedVertexIntervalPositionV1] {
        &self.positions
    }
}

#[derive(Debug, Clone)]
pub struct DyadicSharedVertexIntervalDiagnosticV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    tolerance_bits: u64,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    transform_max_work_per_leaf: usize,
    max_work_per_position: usize,
    leaves: Vec<DyadicSharedVertexIntervalDiagnosticLeafV1>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharedVertexSectorBoundaryV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
    face: FaceId,
    /// `[predecessor, successor]`, each with `[-thickness/2,+thickness/2]`.
    boundary: [[[ori_kinematics::OutwardIntervalV1; 3]; 2]; 2],
}
type LocalSectorBoundaryV1 = (
    [FaceId; 2],
    ori_domain::VertexId,
    FaceId,
    [[[ori_kinematics::OutwardIntervalV1; 3]; 2]; 2],
);
impl SharedVertexSectorBoundaryV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn vertex(&self) -> ori_domain::VertexId {
        self.vertex
    }
    #[must_use]
    pub const fn face(&self) -> FaceId {
        self.face
    }
    #[must_use]
    pub const fn boundary(&self) -> [[[ori_kinematics::OutwardIntervalV1; 3]; 2]; 2] {
        self.boundary
    }
}

#[derive(Debug, Clone)]
pub struct DyadicSharedVertexSectorBoundaryDiagnosticV1 {
    issuer: MaterialHingeGraphGeometry,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    max_work_per_point: usize,
    radius_binding: Vec<(ori_domain::VertexId, u64)>,
    leaves: Vec<(u32, u64, Vec<SharedVertexSectorBoundaryV1>)>,
}

/// One exact convex remainder of a face after clipping away the circular
/// vertex-relief neighbourhood by a conservative supporting half-plane.
///
/// `vertices` contains every polygon corner on both material surfaces after
/// the dyadic face transform.  It is diagnostic evidence only: the supporting
/// half-plane contains the whole limited convex wedge, but does not by itself
/// prove separation from another moving face.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedVertexWedgeCellV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
    face: FaceId,
    top_ring: Vec<[ori_kinematics::OutwardIntervalV1; 3]>,
    bottom_ring: Vec<[ori_kinematics::OutwardIntervalV1; 3]>,
}
impl SharedVertexWedgeCellV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn vertex(&self) -> ori_domain::VertexId {
        self.vertex
    }
    #[must_use]
    pub const fn face(&self) -> FaceId {
        self.face
    }
    #[must_use]
    pub fn top_ring(&self) -> &[[ori_kinematics::OutwardIntervalV1; 3]] {
        &self.top_ring
    }
    #[must_use]
    pub fn bottom_ring(&self) -> &[[ori_kinematics::OutwardIntervalV1; 3]] {
        &self.bottom_ring
    }
}

#[derive(Debug, Clone)]
pub struct DyadicSharedVertexWedgeDiagnosticV1 {
    issuer: MaterialHingeGraphGeometry,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    max_work_per_cell: usize,
    radius_binding: Vec<(ori_domain::VertexId, u64)>,
    sector_content_hash: [u8; 32],
    leaves: Vec<(u32, u64, Vec<SharedVertexWedgeCellV1>)>,
    content_hash: [u8; 32],
}
impl DyadicSharedVertexWedgeDiagnosticV1 {
    #[must_use]
    pub fn leaves(&self) -> &[(u32, u64, Vec<SharedVertexWedgeCellV1>)] {
        &self.leaves
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn is_for(
        &self,
        sectors: &DyadicSharedVertexSectorBoundaryDiagnosticV1,
        transforms: &DyadicFaceTransformIntervalRegistryV1,
        gaps: &SharedVertexContinuousCorridorGapReportV1,
        prerequisite: &crate::NativeVertexReliefPrerequisiteV1,
        records: &[crate::VertexReliefPolicyRecordV1],
        input: DyadicFaceTransformBindingInputV1<'_>,
        max_work_per_cell: usize,
    ) -> bool {
        self.issuer.same_instance(input.geometry)
            && self.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == input.thickness_mm.to_bits()
            && self.max_work_per_cell == max_work_per_cell
            && self.radius_binding
                == records
                    .iter()
                    .map(|r| (r.vertex, r.cutout_radius_mm.to_bits()))
                    .collect::<Vec<_>>()
            && sector_boundary_content_hash_v1(sectors).is_ok_and(|h| h == self.sector_content_hash)
            && sectors.is_for(
                transforms,
                gaps,
                prerequisite,
                records,
                input,
                sectors.max_work_per_point,
            )
            && transforms.is_for(input)
            && wedge_content_hash_v1(
                &self.leaves,
                self.max_work_per_cell,
                &self.radius_binding,
                self.sector_content_hash,
            )
            .is_ok_and(|h| h == self.content_hash)
            && self.leaves.len() == transforms.leaves.len()
            && self.leaves.len() == sectors.leaves.len()
            && self
                .leaves
                .iter()
                .zip(&transforms.leaves)
                .zip(&sectors.leaves)
                .all(
                    |(((d, i, cells), leaf), (sector_d, sector_i, sector_entries))| {
                        *d == leaf.depth
                            && *i == leaf.index
                            && *d == *sector_d
                            && *i == *sector_i
                            && cells.len() == sector_entries.len()
                            && cells.iter().zip(sector_entries).all(|(cell, sector)| {
                                (cell.pair, cell.vertex, cell.face)
                                    == (sector.pair, sector.vertex, sector.face)
                                    && cell.top_ring.len() >= 3
                                    && cell.top_ring.len() == cell.bottom_ring.len()
                            })
                    },
                )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharedVertexWedgeSeparationLowerV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
    lower_mm: f64,
}
impl SharedVertexWedgeSeparationLowerV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn vertex(&self) -> ori_domain::VertexId {
        self.vertex
    }
    #[must_use]
    pub const fn lower_mm(&self) -> f64 {
        self.lower_mm
    }
}

/// Sound common-axis separation bounds for the complete convex prisms stored
/// by [`DyadicSharedVertexWedgeDiagnosticV1`]. This remains diagnostic:
/// common-axis separation is sufficient, but not complete, for disjointness.
#[derive(Debug, Clone)]
pub struct DyadicSharedVertexWedgeSeparationDiagnosticV1 {
    issuer: MaterialHingeGraphGeometry,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    max_work_per_pair: usize,
    wedge_content_hash: [u8; 32],
    leaves: Vec<(u32, u64, Vec<SharedVertexWedgeSeparationLowerV1>)>,
    content_hash: [u8; 32],
}
impl DyadicSharedVertexWedgeSeparationDiagnosticV1 {
    #[must_use]
    pub fn leaves(&self) -> &[(u32, u64, Vec<SharedVertexWedgeSeparationLowerV1>)] {
        &self.leaves
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        wedges: &DyadicSharedVertexWedgeDiagnosticV1,
        input: DyadicFaceTransformBindingInputV1<'_>,
        max_work_per_pair: usize,
    ) -> bool {
        self.issuer.same_instance(input.geometry)
            && self.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == input.thickness_mm.to_bits()
            && wedges.issuer.same_instance(input.geometry)
            && wedges.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && wedges.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && wedges.thickness_bits == input.thickness_mm.to_bits()
            && self.max_work_per_pair == max_work_per_pair
            && self.wedge_content_hash == wedges.content_hash
            && wedge_separation_content_hash_v1(
                &self.leaves,
                self.max_work_per_pair,
                self.wedge_content_hash,
            )
            .is_ok_and(|hash| hash == self.content_hash)
            && self.leaves.len() == wedges.leaves.len()
            && self.leaves.iter().zip(&wedges.leaves).all(
                |((depth, index, bounds), (wedge_depth, wedge_index, cells))| {
                    let Ok(expected) = wedge_pair_keys_v1(cells) else {
                        return false;
                    };
                    depth == wedge_depth
                        && index == wedge_index
                        && !bounds.is_empty()
                        && bounds.len() == expected.len()
                        && bounds.iter().zip(expected).all(|(bound, expected)| {
                            (bound.pair, bound.vertex) == expected
                                && bound.lower_mm.is_finite()
                                && bound.lower_mm > 0.0
                                && cells.iter().any(|cell| {
                                    cell.pair == bound.pair
                                        && cell.vertex == bound.vertex
                                        && cell.face == bound.pair[0]
                                })
                                && cells.iter().any(|cell| {
                                    cell.pair == bound.pair
                                        && cell.vertex == bound.vertex
                                        && cell.face == bound.pair[1]
                                })
                                && cells
                                    .iter()
                                    .filter(|cell| {
                                        cell.pair == bound.pair
                                            && cell.vertex == bound.vertex
                                            && (cell.face == bound.pair[0]
                                                || cell.face == bound.pair[1])
                                    })
                                    .count()
                                    == 2
                        })
                },
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharedVertexBoundaryPointDistanceLowerV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
    lower_mm: f64,
}
impl SharedVertexBoundaryPointDistanceLowerV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn vertex(&self) -> ori_domain::VertexId {
        self.vertex
    }
    #[must_use]
    pub const fn lower_mm(&self) -> f64 {
        self.lower_mm
    }
}

/// Lower bounds only for the finite predecessor/successor by two-side point
/// boxes. This does not enclose a cutout arc or offset surface.
#[derive(Debug, Clone)]
pub struct DyadicSharedVertexBoundaryPointDistanceDiagnosticV1 {
    issuer: MaterialHingeGraphGeometry,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    max_work: usize,
    sector_content_hash: [u8; 32],
    leaves: Vec<(u32, u64, Vec<SharedVertexBoundaryPointDistanceLowerV1>)>,
}
impl DyadicSharedVertexBoundaryPointDistanceDiagnosticV1 {
    #[must_use]
    pub fn leaves(&self) -> &[(u32, u64, Vec<SharedVertexBoundaryPointDistanceLowerV1>)] {
        &self.leaves
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        sectors: &DyadicSharedVertexSectorBoundaryDiagnosticV1,
        gaps: &SharedVertexContinuousCorridorGapReportV1,
        input: DyadicFaceTransformBindingInputV1<'_>,
        max_work: usize,
    ) -> bool {
        self.issuer.same_instance(input.geometry)
            && self.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == input.thickness_mm.to_bits()
            && self.max_work == max_work
            && sector_boundary_content_hash_v1(sectors)
                .is_ok_and(|hash| self.sector_content_hash == hash)
            && sectors.issuer.same_instance(input.geometry)
            && sectors.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && sectors.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && sectors.thickness_bits == input.thickness_mm.to_bits()
            && gaps.is_for(
                input.geometry,
                input.audit,
                input.fixed_face,
                input.schedule,
                input.thickness_mm,
            )
            && self.leaves.len() == sectors.leaves.len()
            && self.leaves.iter().zip(&sectors.leaves).all(
                |((depth, index, bounds), (sector_depth, sector_index, _))| {
                    depth == sector_depth
                        && index == sector_index
                        && bounds.len() == gaps.gaps.len()
                        && bounds.iter().zip(&gaps.gaps).all(|(bound, gap)| {
                            bound.pair == gap.pair
                                && bound.vertex == gap.vertex
                                && bound.lower_mm.is_finite()
                                && bound.lower_mm >= 0.0
                        })
                },
            )
    }
}
impl DyadicSharedVertexSectorBoundaryDiagnosticV1 {
    #[must_use]
    pub fn leaves(&self) -> &[(u32, u64, Vec<SharedVertexSectorBoundaryV1>)] {
        &self.leaves
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        transforms: &DyadicFaceTransformIntervalRegistryV1,
        gaps: &SharedVertexContinuousCorridorGapReportV1,
        prerequisite: &crate::NativeVertexReliefPrerequisiteV1,
        records: &[crate::VertexReliefPolicyRecordV1],
        input: DyadicFaceTransformBindingInputV1<'_>,
        max_work_per_point: usize,
    ) -> bool {
        self.issuer.same_instance(input.geometry)
            && self.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == input.thickness_mm.to_bits()
            && self.max_work_per_point == max_work_per_point
            && self.radius_binding
                == records
                    .iter()
                    .map(|record| (record.vertex, record.cutout_radius_mm.to_bits()))
                    .collect::<Vec<_>>()
            && crate::revalidate_vertex_relief_prerequisite_v1(
                prerequisite,
                input.geometry,
                input.thickness_mm,
                records,
            )
            .is_ok()
            && gaps.is_for(
                input.geometry,
                input.audit,
                input.fixed_face,
                input.schedule,
                input.thickness_mm,
            )
            && transforms.is_for(input)
            && self.leaves.len() == transforms.leaves.len()
            && self.leaves.iter().zip(&transforms.leaves).all(
                |((depth, index, entries), transform)| {
                    *depth == transform.depth
                        && *index == transform.index
                        && entries.len()
                            == gaps
                                .gaps
                                .iter()
                                .filter_map(|gap| {
                                    records
                                        .binary_search_by_key(
                                            &gap.vertex.canonical_bytes(),
                                            |record| record.vertex.canonical_bytes(),
                                        )
                                        .ok()
                                        .map(|record| records[record].incident_faces.len())
                                })
                                .sum::<usize>()
                        && entries
                            .iter()
                            .zip(gaps.gaps.iter().flat_map(|gap| {
                                records
                                    .binary_search_by_key(&gap.vertex.canonical_bytes(), |record| {
                                        record.vertex.canonical_bytes()
                                    })
                                    .ok()
                                    .into_iter()
                                    .flat_map(move |record| {
                                        records[record]
                                            .incident_faces
                                            .iter()
                                            .map(move |face| (gap.pair, gap.vertex, *face))
                                    })
                            }))
                            .all(|(entry, expected)| {
                                (entry.pair, entry.vertex, entry.face) == expected
                            })
                },
            )
    }
}
impl DyadicSharedVertexIntervalDiagnosticV1 {
    #[must_use]
    pub fn leaves(&self) -> &[DyadicSharedVertexIntervalDiagnosticLeafV1] {
        &self.leaves
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        transforms: &DyadicFaceTransformIntervalRegistryV1,
        gaps: &SharedVertexContinuousCorridorGapReportV1,
        input: DyadicFaceTransformBindingInputV1<'_>,
        max_work_per_position: usize,
    ) -> bool {
        self.issuer.same_instance(input.geometry)
            && self.fixed_face == input.fixed_face
            && self.schedule_hash == input.schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == input.closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == input.thickness_mm.to_bits()
            && self.tolerance_bits == input.tolerance.to_bits()
            && self.schedule_limits == input.schedule_limits
            && self.transform_max_work_per_leaf == input.max_work_per_leaf
            && self.max_work_per_position == max_work_per_position
            && gaps.is_for(
                input.geometry,
                input.audit,
                input.fixed_face,
                input.schedule,
                input.thickness_mm,
            )
            && transforms.is_for(input)
            && self.leaves.len() == transforms.leaves.len()
            && self
                .leaves
                .iter()
                .zip(&transforms.leaves)
                .all(|(diagnostic, transform)| {
                    diagnostic.depth == transform.depth
                        && diagnostic.index == transform.index
                        && diagnostic.positions.len() == gaps.gaps.len()
                        && diagnostic
                            .positions
                            .iter()
                            .zip(&gaps.gaps)
                            .all(|(position, gap)| {
                                position.pair == gap.pair && position.vertex == gap.vertex
                            })
                })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuousPairCoverageKindV1 {
    ExistingNonhingeIntervalCandidate,
    SharedHingeNeedsCorridor,
    SharedVertexNeedsCorridor,
    SameGroupSkipped,
    MetadataMissing,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuousPairCoverageEntryV1 {
    pair: [FaceId; 2],
    kind: ContinuousPairCoverageKindV1,
}

impl ContinuousPairCoverageEntryV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }

    #[must_use]
    pub const fn kind(&self) -> ContinuousPairCoverageKindV1 {
        self.kind
    }
}

/// Read-only exact registry of the pair classes encountered by the existing
/// continuous-path implementation. It deliberately grants no authority: the
/// `NeedsCorridor` and `Skipped` entries make current proof gaps explicit.
#[derive(Debug, Clone)]
pub struct ContinuousPairCoverageRegistryV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    entries: Vec<ContinuousPairCoverageEntryV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedHingeContinuousCorridorGapV1 {
    pair: [FaceId; 2],
    hinge: EdgeId,
    source_angle_bits: u64,
    target_angle_bits: u64,
    derivative_bound_bits: u64,
    triangular_prerequisite: bool,
}

impl SharedHingeContinuousCorridorGapV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn hinge(&self) -> EdgeId {
        self.hinge
    }
    #[must_use]
    pub const fn source_angle_bits(&self) -> u64 {
        self.source_angle_bits
    }
    #[must_use]
    pub const fn target_angle_bits(&self) -> u64 {
        self.target_angle_bits
    }
    #[must_use]
    pub const fn derivative_bound_bits(&self) -> u64 {
        self.derivative_bound_bits
    }
    #[must_use]
    pub const fn triangular_prerequisite(&self) -> bool {
        self.triangular_prerequisite
    }
}

/// Exact inputs still lacking an open-interval Cayley corridor theorem.
/// Endpoint static capabilities are intentionally not accepted as a substitute.
#[derive(Debug, Clone)]
pub struct SharedHingeContinuousCorridorGapReportV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    thickness_bits: u64,
    gaps: Vec<SharedHingeContinuousCorridorGapV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedVertexContinuousCorridorGapV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
}
impl SharedVertexContinuousCorridorGapV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn vertex(&self) -> ori_domain::VertexId {
        self.vertex
    }
}

/// Pure geometry gap classification. This is not layer-order evidence and
/// must never be promoted to motion or mutation authority.
#[derive(Debug, Clone)]
pub struct SharedVertexContinuousCorridorGapReportV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    thickness_bits: u64,
    gaps: Vec<SharedVertexContinuousCorridorGapV1>,
}
impl SharedVertexContinuousCorridorGapReportV1 {
    #[must_use]
    pub fn gaps(&self) -> &[SharedVertexContinuousCorridorGapV1] {
        &self.gaps
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &ori_kinematics::CanonicalCycleScheduleV1,
        thickness_mm: f64,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.fixed_face == fixed_face
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v2()
            && self.thickness_bits == thickness_mm.to_bits()
            && schedule.matches_binding(geometry, audit, fixed_face)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReliefCoveredSharedHingePairV1 {
    pair: [FaceId; 2],
    hinge: EdgeId,
}

impl ReliefCoveredSharedHingePairV1 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub const fn hinge(&self) -> EdgeId {
        self.hinge
    }
}

#[derive(Debug, Clone)]
pub struct SharedHingeReliefCoverageReportV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    thickness_bits: u64,
    covered: Vec<ReliefCoveredSharedHingePairV1>,
    remaining: Vec<ContinuousPairCoverageEntryV1>,
}

impl SharedHingeReliefCoverageReportV1 {
    #[must_use]
    pub fn is_for_geometry(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.issuer.same_instance(geometry)
    }
    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &ori_kinematics::CanonicalCycleScheduleV1,
        thickness_mm: f64,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.fixed_face == fixed_face
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v2()
            && self.thickness_bits == thickness_mm.to_bits()
            && schedule.matches_binding(geometry, audit, fixed_face)
    }
    #[must_use]
    pub fn covered(&self) -> &[ReliefCoveredSharedHingePairV1] {
        &self.covered
    }
    #[must_use]
    pub fn remaining(&self) -> &[ContinuousPairCoverageEntryV1] {
        &self.remaining
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SharedHingeReliefCoverageErrorV1 {
    #[error("continuous pair registry or gap report binding mismatch")]
    ForeignCoverage,
    #[error("local hinge relief certificate binding mismatch")]
    ForeignRelief,
    #[error("shared hinge relief pair coverage is incomplete or duplicated")]
    IncompleteCoverage,
    #[error("shared hinge relief coverage exceeds its hard bound")]
    ResourceLimit,
}

impl SharedHingeContinuousCorridorGapReportV1 {
    #[must_use]
    pub fn gaps(&self) -> &[SharedHingeContinuousCorridorGapV1] {
        &self.gaps
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &ori_kinematics::CanonicalCycleScheduleV1,
        paper_thickness_mm: f64,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.fixed_face == fixed_face
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v2()
            && self.thickness_bits == paper_thickness_mm.to_bits()
            && schedule.matches_binding(geometry, audit, fixed_face)
            && diagnose_continuous_pair_coverage_v1(geometry, audit, fixed_face, schedule)
                .and_then(|registry| {
                    diagnose_shared_hinge_continuous_corridor_gaps_v1(
                        &registry,
                        geometry,
                        audit,
                        fixed_face,
                        schedule,
                        paper_thickness_mm,
                    )
                })
                .is_some_and(|fresh| fresh.gaps == self.gaps)
    }
}

impl ContinuousPairCoverageRegistryV1 {
    #[must_use]
    pub fn entries(&self) -> &[ContinuousPairCoverageEntryV1] {
        &self.entries
    }

    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.fixed_face == fixed_face
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v2()
            && schedule.matches_binding(geometry, audit, fixed_face)
            && checked_unordered_pair_count_v1(geometry.face_ids().len())
                == Some(self.entries.len())
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub fn gap_count(&self) -> usize {
        self.entries.len()
    }
}

fn checked_unordered_pair_count_v1(face_count: usize) -> Option<usize> {
    face_count
        .checked_mul(face_count.checked_sub(1)?)
        .map(|n| n / 2)
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_dyadic_face_transform_interval_registry_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    tolerance: f64,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    max_work_per_leaf: usize,
) -> Result<DyadicFaceTransformIntervalRegistryV1, DyadicFaceTransformIntervalErrorV1> {
    if !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
        || !tolerance.is_finite()
        || tolerance < 0.0
        || max_work_per_leaf == 0
        || !schedule.matches_binding(geometry, audit, fixed_face)
        || closure.fixed_face() != fixed_face
        || closure.schedule_binding_fingerprint_v2()
            != schedule.certificate_binding_fingerprint_v2()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || !closure.every_leaf_covers_graph_v1(geometry)
        || closure.leaves().len() > MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1
    {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(closure.leaves().len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for (depth, index, leaf_closure) in closure.leaves() {
        let boxes = schedule
            .evaluate_angle_box_dyadic(*depth, *index, schedule_limits)
            .map_err(|error| match error {
                ori_kinematics::CycleSchedulePrepareErrorV1::ResourceLimit => {
                    DyadicFaceTransformIntervalErrorV1::ResourceLimit
                }
                _ => DyadicFaceTransformIntervalErrorV1::Unproven,
            })?;
        let transforms = geometry
            .prepare_interval_face_transform_registry_v1(
                audit,
                fixed_face,
                &boxes,
                Some(leaf_closure),
                tolerance,
                max_work_per_leaf,
            )
            .map_err(|error| match error {
                ori_kinematics::KinematicsError::ResourceLimitExceeded => {
                    DyadicFaceTransformIntervalErrorV1::ResourceLimit
                }
                _ => DyadicFaceTransformIntervalErrorV1::Unproven,
            })?;
        if transforms.transforms().len() != geometry.face_ids().len() {
            return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
        }
        leaves.push(DyadicFaceTransformIntervalLeafV1 {
            depth: *depth,
            index: *index,
            transforms,
        });
    }
    Ok(DyadicFaceTransformIntervalRegistryV1 {
        issuer: geometry.clone(),
        fixed_face,
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        closure_hash: closure.partition_binding_fingerprint_v2(),
        thickness_bits: paper_thickness_mm.to_bits(),
        tolerance_bits: tolerance.to_bits(),
        schedule_limits,
        max_work_per_leaf,
        leaves,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn diagnose_dyadic_shared_vertex_interval_positions_v1(
    transforms: &DyadicFaceTransformIntervalRegistryV1,
    gaps: &SharedVertexContinuousCorridorGapReportV1,
    input: DyadicFaceTransformBindingInputV1<'_>,
    max_work_per_position: usize,
) -> Result<DyadicSharedVertexIntervalDiagnosticV1, DyadicFaceTransformIntervalErrorV1> {
    if max_work_per_position == 0
        || !transforms.is_for(DyadicFaceTransformBindingInputV1 {
            geometry: input.geometry,
            audit: input.audit,
            fixed_face: input.fixed_face,
            schedule: input.schedule,
            closure: input.closure,
            thickness_mm: input.thickness_mm,
            tolerance: input.tolerance,
            schedule_limits: input.schedule_limits,
            max_work_per_leaf: input.max_work_per_leaf,
        })
        || !gaps.is_for(
            input.geometry,
            input.audit,
            input.fixed_face,
            input.schedule,
            input.thickness_mm,
        )
        || transforms
            .leaves
            .len()
            .checked_mul(gaps.gaps.len())
            .is_none_or(|count| count > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1)
    {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(transforms.leaves.len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for leaf in &transforms.leaves {
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(gaps.gaps.len())
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        for gap in &gaps.gaps {
            let source = input
                .geometry
                .vertex_position(gap.vertex)
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            let point = [source.x(), source.y(), source.z()]
                .map(ori_kinematics::OutwardIntervalV1::from_rounded)
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?
                .try_into()
                .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
            let mut transformed = Vec::new();
            transformed
                .try_reserve_exact(2)
                .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
            for face in gap.pair {
                let transform = leaf
                    .transforms
                    .transform_for(face)
                    .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
                transformed.push(transform.apply(point, max_work_per_position).map_err(
                    |error| match error {
                        ori_kinematics::OutwardIntervalErrorV1::ResourceLimit => {
                            DyadicFaceTransformIntervalErrorV1::ResourceLimit
                        }
                        _ => DyadicFaceTransformIntervalErrorV1::Unproven,
                    },
                )?);
            }
            positions.push(SharedVertexIntervalPositionV1 {
                pair: gap.pair,
                vertex: gap.vertex,
                positions: transformed
                    .try_into()
                    .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?,
            });
        }
        leaves.push(DyadicSharedVertexIntervalDiagnosticLeafV1 {
            depth: leaf.depth,
            index: leaf.index,
            positions,
        });
    }
    Ok(DyadicSharedVertexIntervalDiagnosticV1 {
        issuer: input.geometry.clone(),
        fixed_face: input.fixed_face,
        schedule_hash: input.schedule.certificate_binding_fingerprint_v2(),
        closure_hash: input.closure.partition_binding_fingerprint_v2(),
        thickness_bits: input.thickness_mm.to_bits(),
        tolerance_bits: input.tolerance.to_bits(),
        schedule_limits: input.schedule_limits,
        transform_max_work_per_leaf: input.max_work_per_leaf,
        max_work_per_position,
        leaves,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn diagnose_dyadic_shared_vertex_sector_boundaries_v1(
    transforms: &DyadicFaceTransformIntervalRegistryV1,
    gaps: &SharedVertexContinuousCorridorGapReportV1,
    prerequisite: &crate::NativeVertexReliefPrerequisiteV1,
    records: &[crate::VertexReliefPolicyRecordV1],
    input: DyadicFaceTransformBindingInputV1<'_>,
    max_work_per_point: usize,
) -> Result<DyadicSharedVertexSectorBoundaryDiagnosticV1, DyadicFaceTransformIntervalErrorV1> {
    if max_work_per_point == 0
        || crate::revalidate_vertex_relief_prerequisite_v1(
            prerequisite,
            input.geometry,
            input.thickness_mm,
            records,
        )
        .is_err()
        || !transforms.is_for(DyadicFaceTransformBindingInputV1 {
            geometry: input.geometry,
            audit: input.audit,
            fixed_face: input.fixed_face,
            schedule: input.schedule,
            closure: input.closure,
            thickness_mm: input.thickness_mm,
            tolerance: input.tolerance,
            schedule_limits: input.schedule_limits,
            max_work_per_leaf: input.max_work_per_leaf,
        })
        || !gaps.is_for(
            input.geometry,
            input.audit,
            input.fixed_face,
            input.schedule,
            input.thickness_mm,
        )
    {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let local_count = gaps
        .gaps
        .iter()
        .try_fold(0_usize, |count, gap| {
            records
                .binary_search_by_key(&gap.vertex.canonical_bytes(), |record| {
                    record.vertex.canonical_bytes()
                })
                .ok()
                .and_then(|index| count.checked_add(records[index].incident_faces.len()))
        })
        .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
    let item_count = local_count
        .checked_mul(transforms.leaves.len())
        .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    if item_count > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1 {
        return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
    }
    let mut local: Vec<LocalSectorBoundaryV1> = Vec::new();
    local
        .try_reserve_exact(local_count)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for gap in &gaps.gaps {
        let record = records
            .binary_search_by_key(&gap.vertex.canonical_bytes(), |record| {
                record.vertex.canonical_bytes()
            })
            .ok()
            .map(|index| &records[index])
            .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
        for &face in &record.incident_faces {
            let boundary = input
                .geometry
                .face_boundary_vertices(face)
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            let matches = boundary
                .iter()
                .enumerate()
                .filter(|(_, vertex)| **vertex == gap.vertex)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if boundary.len() < 3 || matches.len() != 1 {
                return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
            }
            let index = matches[0];
            let adjacent = [
                boundary[(index + boundary.len() - 1) % boundary.len()],
                boundary[(index + 1) % boundary.len()],
            ];
            if adjacent[0] == adjacent[1] {
                return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
            }
            let points = adjacent
                .map(|other| {
                    sector_boundary_local_point(
                        input.geometry,
                        gap.vertex,
                        other,
                        record.cutout_radius_mm,
                        input.thickness_mm,
                    )
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            local.push((
                gap.pair,
                gap.vertex,
                face,
                points
                    .try_into()
                    .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?,
            ));
        }
    }
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(transforms.leaves.len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for leaf in &transforms.leaves {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(local.len())
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        for &(pair, vertex, face, local_boundary) in &local {
            let transform = leaf
                .transforms
                .transform_for(face)
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            let mut world = local_boundary;
            for ray in &mut world {
                for side in ray {
                    *side = transform.apply(*side, max_work_per_point).map_err(
                        |error| match error {
                            ori_kinematics::OutwardIntervalErrorV1::ResourceLimit => {
                                DyadicFaceTransformIntervalErrorV1::ResourceLimit
                            }
                            _ => DyadicFaceTransformIntervalErrorV1::Unproven,
                        },
                    )?;
                }
            }
            entries.push(SharedVertexSectorBoundaryV1 {
                pair,
                vertex,
                face,
                boundary: world,
            });
        }
        leaves.push((leaf.depth, leaf.index, entries));
    }
    Ok(DyadicSharedVertexSectorBoundaryDiagnosticV1 {
        issuer: input.geometry.clone(),
        schedule_hash: input.schedule.certificate_binding_fingerprint_v2(),
        closure_hash: input.closure.partition_binding_fingerprint_v2(),
        thickness_bits: input.thickness_mm.to_bits(),
        max_work_per_point,
        radius_binding: records
            .iter()
            .map(|record| (record.vertex, record.cutout_radius_mm.to_bits()))
            .collect(),
        leaves,
    })
}

fn sector_boundary_local_point(
    geometry: &MaterialHingeGraphGeometry,
    vertex: ori_domain::VertexId,
    other: ori_domain::VertexId,
    radius_mm: f64,
    thickness_mm: f64,
) -> Result<[[ori_kinematics::OutwardIntervalV1; 3]; 2], DyadicFaceTransformIntervalErrorV1> {
    let origin = geometry
        .vertex_position(vertex)
        .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
    let endpoint = geometry
        .vertex_position(other)
        .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
    let o = [origin.x(), origin.y(), origin.z()];
    let e = [endpoint.x(), endpoint.y(), endpoint.z()];
    let exact = |value| {
        BigRational::from_f64(value).ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)
    };
    let delta = [
        exact(e[0])? - exact(o[0])?,
        exact(e[1])? - exact(o[1])?,
        exact(e[2])? - exact(o[2])?,
    ];
    let length_squared = delta.iter().map(|value| value * value).sum::<BigRational>();
    let (length_lower, length_upper) = ori_numeric::rational_sqrt_bounds(
        &length_squared,
        ori_numeric::ExpressionLimits::default(),
    )
    .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let radius = exact(radius_mm)?;
    if length_lower <= radius || length_lower <= BigRational::from_integer(0.into()) {
        return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
    }
    let outward = ori_numeric::rational_interval_to_f64_outward(&length_lower, &length_upper)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let length = ori_kinematics::OutwardIntervalV1::new(outward.lower(), outward.upper())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let radius = ori_kinematics::OutwardIntervalV1::from_rounded(radius_mm)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let mut boundary = [ori_kinematics::OutwardIntervalV1::new(0.0, 0.0)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?; 3];
    for index in 0..3 {
        boundary[index] = ori_kinematics::OutwardIntervalV1::from_rounded(o[index])
            .and_then(|value| {
                value.add(
                    ori_kinematics::OutwardIntervalV1::from_rounded(
                        delta[index]
                            .to_f64()
                            .ok_or(ori_kinematics::OutwardIntervalErrorV1::InvalidEndpoint)?,
                    )?
                    .div(length)?
                    .mul(radius)?,
                )
            })
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    }
    let half = ori_kinematics::OutwardIntervalV1::from_rounded(thickness_mm / 2.0)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let mut lower = boundary;
    let mut upper = boundary;
    // Native material coordinates use the X-Z paper plane and Y normal.
    lower[1] = lower[1]
        .sub(half)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    upper[1] = upper[1]
        .add(half)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    Ok([lower, upper])
}

pub fn diagnose_dyadic_shared_vertex_boundary_point_distances_v1(
    sectors: &DyadicSharedVertexSectorBoundaryDiagnosticV1,
    gaps: &SharedVertexContinuousCorridorGapReportV1,
    input: DyadicFaceTransformBindingInputV1<'_>,
    max_work: usize,
) -> Result<DyadicSharedVertexBoundaryPointDistanceDiagnosticV1, DyadicFaceTransformIntervalErrorV1>
{
    let work = sectors
        .leaves
        .len()
        .checked_mul(gaps.gaps.len())
        .and_then(|value| value.checked_mul(16))
        .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    if max_work == 0 || work > max_work || work > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1 {
        return Err(if max_work == 0 {
            DyadicFaceTransformIntervalErrorV1::InvalidBinding
        } else {
            DyadicFaceTransformIntervalErrorV1::ResourceLimit
        });
    }
    if !sectors.issuer.same_instance(input.geometry)
        || sectors.schedule_hash != input.schedule.certificate_binding_fingerprint_v2()
        || sectors.closure_hash != input.closure.partition_binding_fingerprint_v2()
        || sectors.thickness_bits != input.thickness_mm.to_bits()
        || !gaps.is_for(
            input.geometry,
            input.audit,
            input.fixed_face,
            input.schedule,
            input.thickness_mm,
        )
    {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(sectors.leaves.len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for (depth, index, entries) in &sectors.leaves {
        let mut bounds = Vec::new();
        bounds
            .try_reserve_exact(gaps.gaps.len())
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        for gap in &gaps.gaps {
            let first = entries
                .iter()
                .find(|entry| {
                    entry.pair == gap.pair
                        && entry.vertex == gap.vertex
                        && entry.face == gap.pair[0]
                })
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            let second = entries
                .iter()
                .find(|entry| {
                    entry.pair == gap.pair
                        && entry.vertex == gap.vertex
                        && entry.face == gap.pair[1]
                })
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            let mut minimum = f64::INFINITY;
            for left in first.boundary.iter().flatten() {
                for right in second.boundary.iter().flatten() {
                    minimum = minimum.min(interval_point_distance_lower_v1(*left, *right)?);
                }
            }
            if !minimum.is_finite() {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            bounds.push(SharedVertexBoundaryPointDistanceLowerV1 {
                pair: gap.pair,
                vertex: gap.vertex,
                lower_mm: minimum,
            });
        }
        leaves.push((*depth, *index, bounds));
    }
    Ok(DyadicSharedVertexBoundaryPointDistanceDiagnosticV1 {
        issuer: input.geometry.clone(),
        schedule_hash: input.schedule.certificate_binding_fingerprint_v2(),
        closure_hash: input.closure.partition_binding_fingerprint_v2(),
        thickness_bits: input.thickness_mm.to_bits(),
        max_work,
        sector_content_hash: sector_boundary_content_hash_v1(sectors)?,
        leaves,
    })
}

fn sector_boundary_content_hash_v1(
    sectors: &DyadicSharedVertexSectorBoundaryDiagnosticV1,
) -> Result<[u8; 32], DyadicFaceTransformIntervalErrorV1> {
    use sha2::Digest as _;
    let mut hash = sha2::Sha256::new();
    let canonical_usize = |value: usize| {
        u64::try_from(value)
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)
            .map(u64::to_le_bytes)
    };
    hash.update(b"origami2:shared-vertex-sector-boundary-content:v1");
    hash.update(canonical_usize(sectors.max_work_per_point)?);
    hash.update(canonical_usize(sectors.radius_binding.len())?);
    for (vertex, radius) in &sectors.radius_binding {
        hash.update(vertex.canonical_bytes());
        hash.update(radius.to_le_bytes());
    }
    hash.update(canonical_usize(sectors.leaves.len())?);
    for (depth, index, entries) in &sectors.leaves {
        hash.update(depth.to_le_bytes());
        hash.update(index.to_le_bytes());
        hash.update(canonical_usize(entries.len())?);
        for entry in entries {
            hash.update(entry.pair[0].canonical_bytes());
            hash.update(entry.pair[1].canonical_bytes());
            hash.update(entry.vertex.canonical_bytes());
            hash.update(entry.face.canonical_bytes());
            for value in entry.boundary.iter().flatten().flatten() {
                hash.update(value.lower().to_bits().to_le_bytes());
                hash.update(value.upper().to_bits().to_le_bytes());
            }
        }
    }
    Ok(hash.finalize().into())
}

const MAX_SHARED_VERTEX_WEDGE_VERTICES_V1: usize = 256;
const MAX_SHARED_VERTEX_WEDGE_INTERSECTIONS_V1: usize = 512;
const MAX_SHARED_VERTEX_WEDGE_BITS_V1: usize = 8192;
const MAX_SHARED_VERTEX_WEDGE_WORK_V1: usize = 4_000_000;

#[derive(Clone)]
struct ExactWedgeCellV1 {
    pair: [FaceId; 2],
    vertex: ori_domain::VertexId,
    face: FaceId,
    polygon: Vec<[BigRational; 2]>,
}

/// Constructs a bounded, exact convex half-plane clipping diagnostic for every
/// limited shared-vertex wedge. No sampled point participates in the result.
#[allow(clippy::too_many_arguments)]
pub fn diagnose_dyadic_shared_vertex_wedges_v1(
    sectors: &DyadicSharedVertexSectorBoundaryDiagnosticV1,
    transforms: &DyadicFaceTransformIntervalRegistryV1,
    gaps: &SharedVertexContinuousCorridorGapReportV1,
    prerequisite: &crate::NativeVertexReliefPrerequisiteV1,
    records: &[crate::VertexReliefPolicyRecordV1],
    input: DyadicFaceTransformBindingInputV1<'_>,
    max_work_per_cell: usize,
) -> Result<DyadicSharedVertexWedgeDiagnosticV1, DyadicFaceTransformIntervalErrorV1> {
    if max_work_per_cell == 0
        || max_work_per_cell > MAX_SHARED_VERTEX_WEDGE_WORK_V1
        || !sectors.is_for(
            transforms,
            gaps,
            prerequisite,
            records,
            input,
            sectors.max_work_per_point,
        )
        || !transforms.is_for(input)
    {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let exact =
        |v: f64| BigRational::from_f64(v).ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    let mut local = Vec::new();
    let count = records
        .iter()
        .try_fold(0usize, |n, r| n.checked_add(r.incident_faces.len()))
        .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    local
        .try_reserve_exact(count)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for gap in &gaps.gaps {
        let record = records
            .binary_search_by_key(&gap.vertex.canonical_bytes(), |r| {
                r.vertex.canonical_bytes()
            })
            .ok()
            .map(|i| &records[i])
            .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
        let origin = input
            .geometry
            .vertex_position(gap.vertex)
            .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
        let o = [exact(origin.x())?, exact(origin.z())?];
        let radius = exact(record.cutout_radius_mm)?;
        if radius <= BigRational::from_integer(0.into()) {
            return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
        }
        for &face in &record.incident_faces {
            let boundary = input
                .geometry
                .face_boundary_vertices(face)
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            if boundary.len() < 3 || boundary.len() > MAX_SHARED_VERTEX_WEDGE_VERTICES_V1 {
                return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
            }
            let positions = boundary
                .iter()
                .map(|id| {
                    let p = input
                        .geometry
                        .vertex_position(*id)
                        .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
                    Ok([exact(p.x())?, exact(p.z())?])
                })
                .collect::<Result<Vec<_>, DyadicFaceTransformIntervalErrorV1>>()?;
            let indices = boundary
                .iter()
                .enumerate()
                .filter(|(_, id)| **id == gap.vertex)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if indices.len() != 1 {
                return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
            }
            let mut meter = WedgeExactMeterV1::new(max_work_per_cell)?;
            for p in &positions {
                meter.charge(WedgeExactMeterV1::bits(&p[0]))?;
                meter.charge(WedgeExactMeterV1::bits(&p[1]))?;
            }
            let pivot = indices[0];
            let prev = &positions[(pivot + positions.len() - 1) % positions.len()];
            let next = &positions[(pivot + 1) % positions.len()];
            let dprev = [meter.sub(&prev[0], &o[0])?, meter.sub(&prev[1], &o[1])?];
            let dnext = [meter.sub(&next[0], &o[0])?, meter.sub(&next[1], &o[1])?];
            let p0 = meter.mul(&dprev[0], &dprev[0])?;
            let p1 = meter.mul(&dprev[1], &dprev[1])?;
            let q0 = meter.mul(&dnext[0], &dnext[0])?;
            let q1 = meter.mul(&dnext[1], &dnext[1])?;
            let lp = meter.add(&p0, &p1)?;
            let ln = meter.add(&q0, &q1)?;
            if lp <= BigRational::from_integer(0.into())
                || ln <= BigRational::from_integer(0.into())
            {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            let wedge_sqrt_limits = ori_numeric::ExpressionLimits {
                max_operations: max_work_per_cell.min(ori_numeric::HARD_MAX_OPERATIONS),
                max_value_bits: MAX_SHARED_VERTEX_WEDGE_BITS_V1,
                ..ori_numeric::ExpressionLimits::default()
            };
            let (lp_lower, lp_upper) = ori_numeric::rational_sqrt_bounds(&lp, wedge_sqrt_limits)
                .map_err(|error| match error {
                    ori_numeric::ExpressionError::ResourceLimit(_) => {
                        DyadicFaceTransformIntervalErrorV1::ResourceLimit
                    }
                    _ => DyadicFaceTransformIntervalErrorV1::Unproven,
                })?;
            let (ln_lower, ln_upper) = ori_numeric::rational_sqrt_bounds(&ln, wedge_sqrt_limits)
                .map_err(|error| match error {
                    ori_numeric::ExpressionError::ResourceLimit(_) => {
                        DyadicFaceTransformIntervalErrorV1::ResourceLimit
                    }
                    _ => DyadicFaceTransformIntervalErrorV1::Unproven,
                })?;
            for bound in [&lp_lower, &lp_upper, &ln_lower, &ln_upper] {
                meter.charge(WedgeExactMeterV1::bits(bound))?;
            }
            if radius >= lp_lower || radius >= ln_lower {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            let a0 = meter.div(&dprev[0], &lp)?;
            let b0 = meter.div(&dnext[0], &ln)?;
            let a1 = meter.div(&dprev[1], &lp)?;
            let b1 = meter.div(&dnext[1], &ln)?;
            let n = [meter.add(&a0, &b0)?, meter.add(&a1, &b1)?];
            let a0 = meter.mul(&n[0], &dprev[0])?;
            let a1 = meter.mul(&n[1], &dprev[1])?;
            let b0 = meter.mul(&n[0], &dnext[0])?;
            let b1 = meter.mul(&n[1], &dnext[1])?;
            let dotp = meter.add(&a0, &a1)?;
            let dotn = meter.add(&b0, &b1)?;
            if dotp <= BigRational::from_integer(0.into())
                || dotn <= BigRational::from_integer(0.into())
            {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            // Dividing by an upper sqrt bound is directed downward.
            let rp = meter.mul(&radius, &dotp)?;
            let rn = meter.mul(&radius, &dotn)?;
            let mp = meter.div(&rp, &lp_upper)?;
            let mn = meter.div(&rn, &ln_upper)?;
            let m = std::cmp::min(mp, mn);
            let mut orientation = None;
            for i in 0..positions.len() {
                let a = &positions[i];
                let b = &positions[(i + 1) % positions.len()];
                let c = &positions[(i + 2) % positions.len()];
                let x0 = meter.sub(&b[0], &a[0])?;
                let y1 = meter.sub(&c[1], &b[1])?;
                let y0 = meter.sub(&b[1], &a[1])?;
                let x1 = meter.sub(&c[0], &b[0])?;
                let left = meter.mul(&x0, &y1)?;
                let right = meter.mul(&y0, &x1)?;
                let cross = meter.sub(&left, &right)?;
                if cross == BigRational::from_integer(0.into()) {
                    continue;
                }
                if orientation.is_some_and(|positive| {
                    positive != (cross > BigRational::from_integer(0.into()))
                }) {
                    return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
                }
                orientation = Some(cross > BigRational::from_integer(0.into()));
            }
            if orientation.is_none() {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            let mut work = 0usize;
            let polygon = exact_clip_wedge_v1(
                &positions,
                &o,
                &n,
                &m,
                &mut work,
                max_work_per_cell,
                &mut meter,
            )?;
            if polygon.len() < 3 || polygon.len() > MAX_SHARED_VERTEX_WEDGE_VERTICES_V1 {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            for p in &polygon {
                wedge_check_point_bits_v1(p)?;
            }
            local.push(ExactWedgeCellV1 {
                pair: gap.pair,
                vertex: gap.vertex,
                face,
                polygon,
            });
        }
    }
    let total = local
        .len()
        .checked_mul(transforms.leaves.len())
        .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    let total_vertices = local
        .iter()
        .try_fold(0usize, |n, cell| {
            cell.polygon
                .len()
                .checked_mul(2)
                .and_then(|count| n.checked_add(count))
        })
        .and_then(|count| count.checked_mul(transforms.leaves.len()))
        .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    if total > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1
        || total_vertices > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1
    {
        return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
    }
    let half = ori_kinematics::OutwardIntervalV1::from_rounded(input.thickness_mm / 2.0)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let zero = ori_kinematics::OutwardIntervalV1::new(0.0, 0.0)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(transforms.leaves.len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for leaf in &transforms.leaves {
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(local.len())
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        for cell in &local {
            let transform = leaf
                .transforms
                .transform_for(cell.face)
                .ok_or(DyadicFaceTransformIntervalErrorV1::InvalidBinding)?;
            let mut top_ring = Vec::new();
            let mut bottom_ring = Vec::new();
            top_ring
                .try_reserve_exact(cell.polygon.len())
                .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
            bottom_ring
                .try_reserve_exact(cell.polygon.len())
                .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
            for point in &cell.polygon {
                let x = exact_rational_interval_v1(&point[0])?;
                let z = exact_rational_interval_v1(&point[1])?;
                for (ring, y) in [
                    (&mut bottom_ring, zero.sub(half)),
                    (&mut top_ring, zero.add(half)),
                ] {
                    let local_point = [
                        x,
                        y.map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?,
                        z,
                    ];
                    ring.push(transform.apply(local_point, max_work_per_cell).map_err(
                        |e| match e {
                            ori_kinematics::OutwardIntervalErrorV1::ResourceLimit => {
                                DyadicFaceTransformIntervalErrorV1::ResourceLimit
                            }
                            _ => DyadicFaceTransformIntervalErrorV1::Unproven,
                        },
                    )?);
                }
            }
            // Opposite winding makes the two rings a closed oriented prism;
            // equal indices before reversal define every complete side quad.
            bottom_ring.reverse();
            cells.push(SharedVertexWedgeCellV1 {
                pair: cell.pair,
                vertex: cell.vertex,
                face: cell.face,
                top_ring,
                bottom_ring,
            });
        }
        leaves.push((leaf.depth, leaf.index, cells));
    }
    let radius_binding = records
        .iter()
        .map(|r| (r.vertex, r.cutout_radius_mm.to_bits()))
        .collect::<Vec<_>>();
    let sector_content_hash = sector_boundary_content_hash_v1(sectors)?;
    let content_hash = wedge_content_hash_v1(
        &leaves,
        max_work_per_cell,
        &radius_binding,
        sector_content_hash,
    )?;
    Ok(DyadicSharedVertexWedgeDiagnosticV1 {
        issuer: input.geometry.clone(),
        schedule_hash: input.schedule.certificate_binding_fingerprint_v2(),
        closure_hash: input.closure.partition_binding_fingerprint_v2(),
        thickness_bits: input.thickness_mm.to_bits(),
        max_work_per_cell,
        radius_binding,
        sector_content_hash,
        leaves,
        content_hash,
    })
}

fn exact_clip_wedge_v1(
    polygon: &[[BigRational; 2]],
    origin: &[BigRational; 2],
    normal: &[BigRational; 2],
    m: &BigRational,
    work: &mut usize,
    limit: usize,
    meter: &mut WedgeExactMeterV1,
) -> Result<Vec<[BigRational; 2]>, DyadicFaceTransformIntervalErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(
            polygon
                .len()
                .checked_add(2)
                .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?,
        )
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    let mut intersections = 0usize;
    for i in 0..polygon.len() {
        *work = work
            .checked_add(1)
            .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        if *work > limit {
            return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
        }
        let a = &polygon[i];
        let b = &polygon[(i + 1) % polygon.len()];
        let sa = meter.side(a, origin, normal, m)?;
        let sb = meter.side(b, origin, normal, m)?;
        let ina = sa >= BigRational::from_integer(0.into());
        let inb = sb >= BigRational::from_integer(0.into());
        if ina {
            output.push(a.clone());
        }
        if ina != inb {
            intersections = intersections
                .checked_add(1)
                .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
            if intersections > MAX_SHARED_VERTEX_WEDGE_INTERSECTIONS_V1 {
                return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
            }
            let denominator = meter.sub(&sa, &sb)?;
            if denominator == BigRational::from_integer(0.into()) {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            let t = meter.div(&sa, &denominator)?;
            let dx = meter.sub(&b[0], &a[0])?;
            let dy = meter.sub(&b[1], &a[1])?;
            let mx = meter.mul(&dx, &t)?;
            let my = meter.mul(&dy, &t)?;
            output.push([meter.add(&a[0], &mx)?, meter.add(&a[1], &my)?]);
        }
        if output.len() > MAX_SHARED_VERTEX_WEDGE_VERTICES_V1 {
            return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
        }
    }
    Ok(output)
}

struct WedgeExactMeterV1 {
    bit_work: usize,
    max_bit_work: usize,
}
impl WedgeExactMeterV1 {
    fn new(limit: usize) -> Result<Self, DyadicFaceTransformIntervalErrorV1> {
        Ok(Self {
            bit_work: 0,
            max_bit_work: limit
                .checked_mul(MAX_SHARED_VERTEX_WEDGE_BITS_V1)
                .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?,
        })
    }
    #[cfg(test)]
    fn with_bit_limit(max_bit_work: usize) -> Self {
        Self {
            bit_work: 0,
            max_bit_work,
        }
    }
    #[cfg(test)]
    const fn bit_work(&self) -> usize {
        self.bit_work
    }
    fn bits(v: &BigRational) -> usize {
        v.numer()
            .to_signed_bytes_le()
            .len()
            .max(v.denom().to_signed_bytes_le().len())
            .saturating_mul(8)
    }
    fn charge(&mut self, bits: usize) -> Result<(), DyadicFaceTransformIntervalErrorV1> {
        if bits > MAX_SHARED_VERTEX_WEDGE_BITS_V1 {
            return Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit);
        }
        self.bit_work = self
            .bit_work
            .checked_add(bits)
            .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        if self.bit_work > self.max_bit_work {
            Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
        } else {
            Ok(())
        }
    }
    fn binary<F>(
        &mut self,
        a: &BigRational,
        b: &BigRational,
        op: F,
    ) -> Result<BigRational, DyadicFaceTransformIntervalErrorV1>
    where
        F: FnOnce(&BigRational, &BigRational) -> BigRational,
    {
        self.charge(
            Self::bits(a)
                .checked_add(Self::bits(b))
                .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?,
        )?;
        let result = op(a, b);
        self.charge(Self::bits(&result))?;
        Ok(result)
    }
    fn add(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, DyadicFaceTransformIntervalErrorV1> {
        self.binary(a, b, |x, y| x + y)
    }
    fn sub(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, DyadicFaceTransformIntervalErrorV1> {
        self.binary(a, b, |x, y| x - y)
    }
    fn mul(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, DyadicFaceTransformIntervalErrorV1> {
        self.binary(a, b, |x, y| x * y)
    }
    fn div(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, DyadicFaceTransformIntervalErrorV1> {
        if b == &BigRational::from_integer(0.into()) {
            return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
        }
        self.binary(a, b, |x, y| x / y)
    }
    fn side(
        &mut self,
        p: &[BigRational; 2],
        o: &[BigRational; 2],
        n: &[BigRational; 2],
        m: &BigRational,
    ) -> Result<BigRational, DyadicFaceTransformIntervalErrorV1> {
        let x = self.sub(&p[0], &o[0])?;
        let y = self.sub(&p[1], &o[1])?;
        let nx = self.mul(&n[0], &x)?;
        let ny = self.mul(&n[1], &y)?;
        let sum = self.add(&nx, &ny)?;
        self.sub(&sum, m)
    }
}

fn wedge_check_point_bits_v1(
    point: &[BigRational; 2],
) -> Result<(), DyadicFaceTransformIntervalErrorV1> {
    if point.iter().any(|v| {
        v.numer().to_signed_bytes_le().len().saturating_mul(8) > MAX_SHARED_VERTEX_WEDGE_BITS_V1
            || v.denom().to_signed_bytes_le().len().saturating_mul(8)
                > MAX_SHARED_VERTEX_WEDGE_BITS_V1
    }) {
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    } else {
        Ok(())
    }
}

fn exact_rational_interval_v1(
    value: &BigRational,
) -> Result<ori_kinematics::OutwardIntervalV1, DyadicFaceTransformIntervalErrorV1> {
    let out = ori_numeric::rational_interval_to_f64_outward(value, value)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    ori_kinematics::OutwardIntervalV1::new(out.lower(), out.upper())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)
}

fn wedge_content_hash_v1(
    leaves: &[(u32, u64, Vec<SharedVertexWedgeCellV1>)],
    max_work: usize,
    radii: &[(ori_domain::VertexId, u64)],
    sector_hash: [u8; 32],
) -> Result<[u8; 32], DyadicFaceTransformIntervalErrorV1> {
    use sha2::Digest as _;
    let usize64 = |v| {
        u64::try_from(v)
            .map(u64::to_le_bytes)
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    };
    let mut h = sha2::Sha256::new();
    h.update(b"origami2:shared-vertex-limited-convex-wedge:v1");
    h.update(usize64(max_work)?);
    h.update(sector_hash);
    h.update(usize64(radii.len())?);
    for (v, r) in radii {
        h.update(v.canonical_bytes());
        h.update(r.to_le_bytes());
    }
    h.update(usize64(leaves.len())?);
    for (d, i, cells) in leaves {
        h.update(d.to_le_bytes());
        h.update(i.to_le_bytes());
        h.update(usize64(cells.len())?);
        for c in cells {
            h.update(c.pair[0].canonical_bytes());
            h.update(c.pair[1].canonical_bytes());
            h.update(c.vertex.canonical_bytes());
            h.update(c.face.canonical_bytes());
            for ring in [&c.top_ring, &c.bottom_ring] {
                h.update(usize64(ring.len())?);
                for p in ring {
                    for v in p {
                        h.update(v.lower().to_bits().to_le_bytes());
                        h.update(v.upper().to_bits().to_le_bytes());
                    }
                }
            }
        }
    }
    Ok(h.finalize().into())
}

const MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1: usize = 4_000_000;

/// Proves a positive lower bound between every cross-face pair of complete
/// convex wedge prisms using their world-coordinate AABBs. Because a convex
/// prism is the convex hull of its two rings, coordinate extrema of all ring
/// vertices enclose every point of the prism, rather than only samples.
pub fn diagnose_dyadic_shared_vertex_wedge_separation_v1(
    wedges: &DyadicSharedVertexWedgeDiagnosticV1,
    input: DyadicFaceTransformBindingInputV1<'_>,
    max_work_per_pair: usize,
) -> Result<DyadicSharedVertexWedgeSeparationDiagnosticV1, DyadicFaceTransformIntervalErrorV1> {
    if max_work_per_pair == 0
        || max_work_per_pair > MAX_SHARED_VERTEX_WEDGE_SEPARATION_WORK_V1
        || !wedges.issuer.same_instance(input.geometry)
        || wedges.schedule_hash != input.schedule.certificate_binding_fingerprint_v2()
        || wedges.closure_hash != input.closure.partition_binding_fingerprint_v2()
        || wedges.thickness_bits != input.thickness_mm.to_bits()
        || !wedge_content_hash_v1(
            &wedges.leaves,
            wedges.max_work_per_cell,
            &wedges.radius_binding,
            wedges.sector_content_hash,
        )
        .is_ok_and(|hash| hash == wedges.content_hash)
        || wedges.leaves.is_empty()
    {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(wedges.leaves.len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for (depth, index, cells) in &wedges.leaves {
        let keys = wedge_pair_keys_v1(cells)?;
        let mut bounds = Vec::new();
        bounds
            .try_reserve_exact(keys.len())
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
        for (pair, vertex) in keys {
            let mut work = 0usize;
            let mut left = None;
            let mut right = None;
            for cell in cells
                .iter()
                .filter(|cell| cell.pair == pair && cell.vertex == vertex)
            {
                let slot = if cell.face == pair[0] {
                    &mut left
                } else if cell.face == pair[1] {
                    &mut right
                } else {
                    continue;
                };
                if slot.replace(cell).is_some() {
                    return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
                }
            }
            let (Some(a), Some(b)) = (left, right) else {
                return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
            };
            let aa = wedge_cell_aabb_v1(a, &mut work, max_work_per_pair)?;
            let bb = wedge_cell_aabb_v1(b, &mut work, max_work_per_pair)?;
            charge_wedge_separation_work_v1(&mut work, 3, max_work_per_pair)?;
            let lower = exact_common_axis_gap_lower_v1(&aa, &bb)?;
            if !lower.is_finite() || lower <= 0.0 {
                return Err(DyadicFaceTransformIntervalErrorV1::Unproven);
            }
            bounds.push(SharedVertexWedgeSeparationLowerV1 {
                pair,
                vertex,
                lower_mm: lower,
            });
        }
        leaves.push((*depth, *index, bounds));
    }
    let wedge_content_hash = wedges.content_hash;
    let content_hash =
        wedge_separation_content_hash_v1(&leaves, max_work_per_pair, wedge_content_hash)?;
    Ok(DyadicSharedVertexWedgeSeparationDiagnosticV1 {
        issuer: input.geometry.clone(),
        schedule_hash: input.schedule.certificate_binding_fingerprint_v2(),
        closure_hash: input.closure.partition_binding_fingerprint_v2(),
        thickness_bits: input.thickness_mm.to_bits(),
        max_work_per_pair,
        wedge_content_hash,
        leaves,
        content_hash,
    })
}

fn exact_common_axis_gap_lower_v1(
    a: &[[f64; 2]; 3],
    b: &[[f64; 2]; 3],
) -> Result<f64, DyadicFaceTransformIntervalErrorV1> {
    let exact =
        |value| BigRational::from_f64(value).ok_or(DyadicFaceTransformIntervalErrorV1::Unproven);
    let mut best: Option<BigRational> = None;
    for axis in 0..3 {
        let forward = exact(b[axis][0])? - exact(a[axis][1])?;
        let reverse = exact(a[axis][0])? - exact(b[axis][1])?;
        let gap = forward.max(reverse);
        best = Some(best.map_or(gap.clone(), |current| current.max(gap)));
    }
    let best = best.ok_or(DyadicFaceTransformIntervalErrorV1::Unproven)?;
    let interval = ori_numeric::rational_interval_to_f64_outward(&best, &best)
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    Ok(interval.lower())
}

fn wedge_pair_keys_v1(
    cells: &[SharedVertexWedgeCellV1],
) -> Result<Vec<([FaceId; 2], ori_domain::VertexId)>, DyadicFaceTransformIntervalErrorV1> {
    if cells.is_empty() {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let mut keys = Vec::new();
    keys.try_reserve_exact(cells.len())
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    for cell in cells {
        if cell.pair[0] == cell.pair[1]
            || cell.pair[0].canonical_bytes() >= cell.pair[1].canonical_bytes()
        {
            return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
        }
        keys.push((cell.pair, cell.vertex));
    }
    keys.sort_by(|a, b| {
        a.0[0]
            .canonical_bytes()
            .cmp(&b.0[0].canonical_bytes())
            .then_with(|| a.0[1].canonical_bytes().cmp(&b.0[1].canonical_bytes()))
            .then_with(|| a.1.canonical_bytes().cmp(&b.1.canonical_bytes()))
    });
    keys.dedup();
    Ok(keys)
}

fn wedge_cell_aabb_v1(
    cell: &SharedVertexWedgeCellV1,
    work: &mut usize,
    limit: usize,
) -> Result<[[f64; 2]; 3], DyadicFaceTransformIntervalErrorV1> {
    if cell.top_ring.len() < 3 || cell.top_ring.len() != cell.bottom_ring.len() {
        return Err(DyadicFaceTransformIntervalErrorV1::InvalidBinding);
    }
    let mut out = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
    for point in cell.top_ring.iter().chain(&cell.bottom_ring) {
        charge_wedge_separation_work_v1(work, 3, limit)?;
        for axis in 0..3 {
            out[axis][0] = out[axis][0].min(point[axis].lower());
            out[axis][1] = out[axis][1].max(point[axis].upper());
        }
    }
    if out.iter().flatten().any(|value| !value.is_finite()) {
        Err(DyadicFaceTransformIntervalErrorV1::Unproven)
    } else {
        Ok(out)
    }
}

fn charge_wedge_separation_work_v1(
    work: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), DyadicFaceTransformIntervalErrorV1> {
    *work = work
        .checked_add(amount)
        .ok_or(DyadicFaceTransformIntervalErrorV1::ResourceLimit)?;
    if *work > limit {
        Err(DyadicFaceTransformIntervalErrorV1::ResourceLimit)
    } else {
        Ok(())
    }
}

fn wedge_separation_content_hash_v1(
    leaves: &[(u32, u64, Vec<SharedVertexWedgeSeparationLowerV1>)],
    max_work: usize,
    wedge_hash: [u8; 32],
) -> Result<[u8; 32], DyadicFaceTransformIntervalErrorV1> {
    use sha2::Digest as _;
    let mut hash = sha2::Sha256::new();
    hash.update(b"origami2:shared-vertex-wedge-separation:v1");
    hash.update(
        u64::try_from(max_work)
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?
            .to_le_bytes(),
    );
    hash.update(wedge_hash);
    for (depth, index, bounds) in leaves {
        hash.update(depth.to_le_bytes());
        hash.update(index.to_le_bytes());
        hash.update(
            u64::try_from(bounds.len())
                .map_err(|_| DyadicFaceTransformIntervalErrorV1::ResourceLimit)?
                .to_le_bytes(),
        );
        for bound in bounds {
            hash.update(bound.pair[0].canonical_bytes());
            hash.update(bound.pair[1].canonical_bytes());
            hash.update(bound.vertex.canonical_bytes());
            hash.update(bound.lower_mm.to_bits().to_le_bytes());
        }
    }
    Ok(hash.finalize().into())
}

fn interval_point_distance_lower_v1(
    left: [ori_kinematics::OutwardIntervalV1; 3],
    right: [ori_kinematics::OutwardIntervalV1; 3],
) -> Result<f64, DyadicFaceTransformIntervalErrorV1> {
    let exact =
        |value| BigRational::from_f64(value).ok_or(DyadicFaceTransformIntervalErrorV1::Unproven);
    let zero = BigRational::from_integer(0.into());
    let mut squared = zero.clone();
    for axis in 0..3 {
        let first = exact(left[axis].lower())? - exact(right[axis].upper())?;
        let second = exact(right[axis].lower())? - exact(left[axis].upper())?;
        let separation = std::cmp::max(zero.clone(), std::cmp::max(first, second));
        squared += &separation * &separation;
    }
    let (lower, _) =
        ori_numeric::rational_sqrt_bounds(&squared, ori_numeric::ExpressionLimits::default())
            .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)?;
    ori_numeric::rational_interval_to_f64_outward(&lower, &lower)
        .map(|interval| interval.lower().max(0.0))
        .map_err(|_| DyadicFaceTransformIntervalErrorV1::Unproven)
}

fn classify_continuous_pair_v1(
    shared_hinges: usize,
    shared_vertex: Option<bool>,
    group_membership: Option<(Option<usize>, Option<usize>)>,
) -> ContinuousPairCoverageKindV1 {
    if shared_hinges == 1 {
        ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
    } else if shared_hinges > 1 || shared_vertex.is_none() {
        ContinuousPairCoverageKindV1::Unsupported
    } else if shared_vertex == Some(true) {
        ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor
    } else if group_membership.is_none()
        || group_membership.is_some_and(|(first, second)| first.is_none() || second.is_none())
    {
        ContinuousPairCoverageKindV1::MetadataMissing
    } else if group_membership.is_some_and(|(first, second)| first == second) {
        ContinuousPairCoverageKindV1::SameGroupSkipped
    } else {
        ContinuousPairCoverageKindV1::ExistingNonhingeIntervalCandidate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackedFoldPathDiagnosticLimitsV1 {
    /// Number of equal angle intervals. Both endpoints are observed.
    pub sample_intervals: usize,
    pub static_collision: StaticCollisionLimits,
}

impl Default for StackedFoldPathDiagnosticLimitsV1 {
    fn default() -> Self {
        Self {
            sample_intervals: 8,
            static_collision: StaticCollisionLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedFoldBoundedPathDiagnosticV1 {
    sampled_pose_count: usize,
    sampled_nonblocking_pose_count: usize,
    first_sampled_blocking_angle_degrees: Option<f64>,
    requested_angle_degrees: f64,
    analytic_single_hinge_clearance: bool,
    analytic_collinear_tree_clearance: bool,
    analytic_positive_two_hinge_clearance: bool,
    interval_two_hinge_chain_clearance: bool,
    interval_tree_hinge_count: usize,
    interval_leaf_count: usize,
    interval_pair_work: usize,
    positive_endpoint_memo_pair_entries: usize,
    positive_endpoint_exact_pair_calls: usize,
    positive_thickness_outer_shell: bool,
}

impl StackedFoldBoundedPathDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1
    }

    #[must_use]
    pub const fn sampled_pose_count(&self) -> usize {
        self.sampled_pose_count
    }

    #[must_use]
    pub const fn sampled_nonblocking_pose_count(&self) -> usize {
        self.sampled_nonblocking_pose_count
    }

    #[must_use]
    pub const fn interval_leaf_count(&self) -> usize {
        self.interval_leaf_count
    }

    #[must_use]
    pub const fn interval_pair_work(&self) -> usize {
        self.interval_pair_work
    }

    #[must_use]
    pub const fn positive_endpoint_memo_pair_entries(&self) -> usize {
        self.positive_endpoint_memo_pair_entries
    }

    #[must_use]
    pub const fn positive_endpoint_exact_pair_calls(&self) -> usize {
        self.positive_endpoint_exact_pair_calls
    }

    #[must_use]
    pub const fn positive_endpoint_candidate_limit(&self) -> usize {
        MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
    }

    #[must_use]
    pub const fn interval_candidate_limit(&self) -> usize {
        MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1
    }

    #[must_use]
    pub const fn first_sampled_blocking_angle_degrees(&self) -> Option<f64> {
        self.first_sampled_blocking_angle_degrees
    }

    #[must_use]
    pub const fn requested_angle_degrees(&self) -> f64 {
        self.requested_angle_degrees
    }

    /// Sampling cannot prove an open continuous interval.
    #[must_use]
    pub const fn continuous_clearance_certified(&self) -> bool {
        self.analytic_single_hinge_clearance
            || self.analytic_collinear_tree_clearance
            || self.analytic_positive_two_hinge_clearance
            || self.interval_two_hinge_chain_clearance
    }

    /// The only fail-closed recommendation supplied by this diagnostic is to
    /// retain the already authenticated initial pose.
    #[must_use]
    pub const fn safe_stop_angle_degrees(&self) -> f64 {
        if self.continuous_clearance_certified() {
            self.requested_angle_degrees
        } else {
            0.0
        }
    }

    #[must_use]
    pub const fn continuous_certificate_model_id(&self) -> Option<&'static str> {
        if self.interval_two_hinge_chain_clearance {
            Some(
                if self.sampled_pose_count > 0 && self.interval_tree_hinge_count() > 2 {
                    STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
                } else {
                    STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
                },
            )
        } else if self.analytic_positive_two_hinge_clearance {
            Some(STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2)
        } else if self.analytic_collinear_tree_clearance {
            Some(STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        } else if self.analytic_single_hinge_clearance {
            Some(if self.positive_thickness_outer_shell {
                STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2
            } else {
                STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            })
        } else {
            None
        }
    }

    const fn interval_tree_hinge_count(&self) -> usize {
        // A certified tree has one more face than hinges. The diagnostic does
        // not otherwise expose topology, so this value is stored explicitly
        // below in the next field.
        self.interval_tree_hinge_count
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StackedFoldPathDiagnosticErrorV1 {
    #[error("the path diagnostic limits are invalid")]
    InvalidLimits,
    #[error("the requested angle or moving-hinge set is invalid")]
    InvalidPath,
    #[error("the initial pose is not owned by the supplied model")]
    PoseIssuerMismatch,
    #[error("one sampled pose could not be solved")]
    PoseUnavailable,
    #[error("one sampled static collision diagnosis failed")]
    StaticDiagnosisUnavailable,
    #[error("the path diagnostic was cancelled")]
    Cancelled,
    #[error("the path diagnostic absolute deadline elapsed")]
    DeadlineExceeded,
    #[error("the pair-proof cache operation failed closed")]
    ProofCacheUnavailable,
    #[error("the pair-proof result became stale before publication")]
    StaleProofCacheResult,
    #[error("the initial layer-order admission failed closed")]
    InitialLayerOrderUnavailable,
    #[error("the initial layer-order admission exceeds its resource bound")]
    InitialLayerOrderResourceLimit,
}

fn path_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
        }
    })
}

/// Opaque native evidence for one exact continuously clear Tree path.
///
/// Issuance and revalidation use only the general, admission-free path
/// diagnostic. In particular, an independently admitted initial layer order
/// cannot be converted into this certificate. The type deliberately
/// implements neither `Clone` nor persistence traits.
#[derive(Debug)]
pub struct StackedFoldTreeContinuousCertificateV1 {
    source_absolute: Vec<HingeAngle>,
    target_absolute: Vec<HingeAngle>,
    paper_thickness_bits: u64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    diagnostic: StackedFoldBoundedPathDiagnosticV1,
}

impl StackedFoldTreeContinuousCertificateV1 {
    /// Repeats the admission-free native proof for the exact saved path.
    ///
    /// Source and target angles, including signed zero, and paper thickness
    /// must match bit-for-bit before the diagnostic is rerun.
    #[must_use]
    pub fn is_for(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_absolute: &CanonicalHingeAngles,
        paper_thickness_mm: f64,
    ) -> bool {
        self.is_for_with_control_v1(
            model,
            source_pose,
            target_absolute,
            paper_thickness_mm,
            &CooperativeOperationControlV1::unbounded(),
        )
        .is_ok_and(|matches| matches)
    }

    /// Controlled revalidation. A cancelled revalidation never authenticates
    /// this certificate for a caller.
    pub fn is_for_with_control_v1(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_absolute: &CanonicalHingeAngles,
        paper_thickness_mm: f64,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<bool, StackedFoldPathDiagnosticErrorV1> {
        path_checkpoint_v1(control)?;
        if paper_thickness_mm.to_bits() != self.paper_thickness_bits
            || !exact_hinge_angles_match_v1(source_pose.hinge_angles(), &self.source_absolute)
            || !exact_hinge_angles_match_v1(target_absolute.as_slice(), &self.target_absolute)
        {
            return Ok(false);
        }
        let actual = diagnose_collective_hinge_path_from_pose_with_control_v1(
            model,
            source_pose,
            &self.source_absolute,
            &self.target_absolute,
            paper_thickness_mm,
            self.limits,
            control,
        )?;
        Ok(actual == self.diagnostic
            && actual.continuous_clearance_certified()
            && actual.continuous_certificate_model_id().is_some())
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

/// Mints opaque Tree-path evidence only after the admission-free native
/// analytic or interval classifier certifies the complete exact path.
pub fn certify_tree_continuous_path_from_pose_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_absolute: &CanonicalHingeAngles,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
) -> Result<Option<StackedFoldTreeContinuousCertificateV1>, StackedFoldPathDiagnosticErrorV1> {
    certify_tree_continuous_path_from_pose_with_control_v1(
        model,
        source_pose,
        target_absolute,
        paper_thickness_mm,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn certify_tree_continuous_path_from_pose_with_control_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_absolute: &CanonicalHingeAngles,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<Option<StackedFoldTreeContinuousCertificateV1>, StackedFoldPathDiagnosticErrorV1> {
    path_checkpoint_v1(control)?;
    let source_absolute = source_pose.hinge_angles().to_vec();
    let target_absolute = target_absolute.as_slice().to_vec();
    let diagnostic = diagnose_collective_hinge_path_from_pose_with_control_v1(
        model,
        source_pose,
        &source_absolute,
        &target_absolute,
        paper_thickness_mm,
        limits,
        control,
    )?;
    path_checkpoint_v1(control)?;
    Ok((diagnostic.continuous_clearance_certified()
        && diagnostic.continuous_certificate_model_id().is_some())
    .then_some(StackedFoldTreeContinuousCertificateV1 {
        source_absolute,
        target_absolute,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        limits,
        diagnostic,
    }))
}

fn exact_hinge_angles_match_v1(left: &[HingeAngle], right: &[HingeAngle]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.edge() == right.edge()
                && left.angle_degrees().to_bits() == right.angle_degrees().to_bits()
        })
}

/// Opaque positive-thickness Tree-path evidence.  It retains the complete
/// canonical endpoints and is useful only through [`Self::is_for`], which
/// rebinds the source pose to the supplied model and repeats the native proof.
#[derive(Debug, Clone, PartialEq)]
pub struct PositiveThicknessTreeContinuousCertificateV1 {
    source_absolute: CanonicalHingeAngles,
    target_absolute: CanonicalHingeAngles,
    paper_thickness_bits: u64,
    diagnostic: StackedFoldBoundedPathDiagnosticV1,
}

impl PositiveThicknessTreeContinuousCertificateV1 {
    #[must_use]
    pub fn binding_fingerprint_v1(&self) -> [u8; 32] {
        let mut hash = sha2::Sha256::new();
        use sha2::Digest as _;
        hash.update(b"positive_thickness_tree_continuous_certificate_binding_v2");
        let model_id = self
            .diagnostic
            .continuous_certificate_model_id()
            .expect("a native positive-thickness certificate always has a model ID");
        hash.update((model_id.len() as u64).to_be_bytes());
        hash.update(model_id.as_bytes());
        hash.update(
            (ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.len() as u64).to_be_bytes(),
        );
        hash.update(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes());
        for angles in [&self.source_absolute, &self.target_absolute] {
            hash.update((angles.as_slice().len() as u64).to_be_bytes());
            for angle in angles.as_slice() {
                hash.update(angle.edge().canonical_bytes());
                hash.update(angle.angle_degrees().to_bits().to_be_bytes());
            }
        }
        hash.update(self.paper_thickness_bits.to_be_bytes());
        hash.update(
            self.diagnostic
                .requested_angle_degrees()
                .to_bits()
                .to_be_bytes(),
        );
        hash.finalize().into()
    }

    #[must_use]
    pub fn is_for(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_absolute: &CanonicalHingeAngles,
        paper_thickness_mm: f64,
    ) -> bool {
        paper_thickness_mm.to_bits() == self.paper_thickness_bits
            && target_absolute == &self.target_absolute
            && source_pose.hinge_angles() == self.source_absolute.as_slice()
            && diagnose_collective_hinge_path_from_pose_v1(
                model,
                source_pose,
                self.source_absolute.as_slice(),
                self.target_absolute.as_slice(),
                paper_thickness_mm,
                StackedFoldPathDiagnosticLimitsV1::default(),
            )
            .is_ok_and(|actual| {
                actual == self.diagnostic && actual.continuous_clearance_certified()
            })
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

pub fn certify_positive_thickness_tree_continuous_path_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_absolute: &CanonicalHingeAngles,
    paper_thickness_mm: f64,
) -> Option<PositiveThicknessTreeContinuousCertificateV1> {
    if !paper_thickness_mm.is_finite() || paper_thickness_mm <= 0.0 {
        return None;
    }
    let source_absolute = CanonicalHingeAngles::new(source_pose.hinge_angles().to_vec()).ok()?;
    let diagnostic = diagnose_collective_hinge_path_from_pose_v1(
        model,
        source_pose,
        source_absolute.as_slice(),
        target_absolute.as_slice(),
        paper_thickness_mm,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .ok()?;
    diagnostic.continuous_clearance_certified().then_some(
        PositiveThicknessTreeContinuousCertificateV1 {
            source_absolute,
            target_absolute: target_absolute.clone(),
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            diagnostic,
        },
    )
}

/// Read-only proof that a source ply order can be transported to a Tree
/// endpoint: every broad-phase candidate is authenticated by the endpoint
/// topology memo as shared-vertex-only contact.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedVertexTreeLayerTransportProofV1 {
    source: LayerOrderSnapshot,
    target_absolute: CanonicalHingeAngles,
    paper_thickness_bits: u64,
    enumerated_pairs: usize,
}

impl SharedVertexTreeLayerTransportProofV1 {
    #[must_use]
    pub fn is_for(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        source: &LayerOrderSnapshot,
        target_absolute: &CanonicalHingeAngles,
        paper_thickness_mm: f64,
        positive: &PositiveThicknessTreeContinuousCertificateV1,
    ) -> bool {
        self.source == *source
            && self.target_absolute == *target_absolute
            && self.paper_thickness_bits == paper_thickness_mm.to_bits()
            && positive.is_for(model, source_pose, target_absolute, paper_thickness_mm)
            && prepare_shared_vertex_tree_layer_transport_v1(
                model,
                source_pose,
                source,
                target_absolute,
                paper_thickness_mm,
                positive,
            )
            .is_some_and(|actual| actual == *self)
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

pub fn prepare_shared_vertex_tree_layer_transport_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    source: &LayerOrderSnapshot,
    target_absolute: &CanonicalHingeAngles,
    paper_thickness_mm: f64,
    positive: &PositiveThicknessTreeContinuousCertificateV1,
) -> Option<SharedVertexTreeLayerTransportProofV1> {
    if !positive.is_for(model, source_pose, target_absolute, paper_thickness_mm) {
        return None;
    }
    let target_pose = model
        .solve(source_pose.fixed_face(), target_absolute)
        .ok()?;
    let candidates = positive_endpoint_candidates_v1(model, &target_pose, paper_thickness_mm)?;
    let memo = prepare_positive_thickness_tree_endpoint_topology_memo_v1(
        model,
        &target_pose,
        paper_thickness_mm,
        StaticCollisionLimits::default(),
    )
    .ok()?;
    let expected_pairs = model
        .face_ids()
        .len()
        .checked_mul(model.face_ids().len().saturating_sub(1))?
        / 2;
    if memo.enumerated_pairs() != expected_pairs
        || candidates.iter().any(|(first, second)| {
            !faces_share_material_vertex_v1(model, *first, *second)
                && !memo.proves_shared_vertex_pair(*first, *second)
        })
    {
        return None;
    }
    Some(SharedVertexTreeLayerTransportProofV1 {
        source: source.clone(),
        target_absolute: target_absolute.clone(),
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        enumerated_pairs: memo.enumerated_pairs(),
    })
}

fn positive_tree_max_angle_degrees_v1(hinge_count: usize) -> Option<f64> {
    Some(match hinge_count {
        15 => 1.5,
        14 => 2.0,
        13 => 3.0,
        12 => 4.0,
        11 => 5.0,
        10 => 6.0,
        9 => 8.0,
        8 => 10.0,
        7 => 15.0,
        6 => 20.0,
        5 => 30.0,
        4 => 45.0,
        3 => 60.0,
        2 => 90.0,
        16..=63 => 0.1 / hinge_count as f64,
        _ => return None,
    })
}

fn positive_tree_resource_premises_v1(
    face_count: usize,
    hinge_count: usize,
    moving_count: usize,
) -> bool {
    let Some(_pair_count) = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|product| product / 2)
    else {
        return false;
    };
    (3..=MAX_POSITIVE_ENDPOINT_TREE_FACES_V1).contains(&face_count)
        && hinge_count >= 2
        && hinge_count.checked_add(1) == Some(face_count)
        && moving_count == hinge_count
        && positive_tree_max_angle_degrees_v1(hinge_count).is_some()
}

fn positive_endpoint_candidates_v1(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    paper_thickness_mm: f64,
) -> Option<Vec<(FaceId, FaceId)>> {
    let bound = model.bind_pose(pose).ok()?;
    prepare_positive_thickness_exact_endpoint_session_v2(bound, paper_thickness_mm)
        .ok()?
        .exact_endpoint_candidates_v2(MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1)
        .ok()
}

fn faces_share_material_vertex_v1(
    model: &MaterialTreeKinematicsModel,
    first: FaceId,
    second: FaceId,
) -> bool {
    model.face_boundary(first).is_some_and(|first_boundary| {
        model.face_boundary(second).is_some_and(|second_boundary| {
            first_boundary
                .vertices()
                .iter()
                .any(|vertex| second_boundary.vertices().contains(vertex))
        })
    })
}

pub fn diagnose_collective_hinge_path_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    let (source_absolute, target_absolute) =
        collective_path_absolute_angles_v1(initial_pose, moving_hinges, requested_angle_degrees)?;
    diagnose_collective_hinge_path_from_pose_with_optional_cache_v1(
        model,
        initial_pose,
        source_absolute,
        target_absolute.as_slice(),
        paper_thickness_mm,
        limits,
        None,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Diagnoses a collective path while admitting one independently ordered,
/// exact flat-stack initial sample and only its stationary direct-hinge pairs
/// at later sampled poses.
///
/// The admission is exact-instance-bound to `model` and `initial_pose`.
/// A later sampled hold is admitted only when the complete static scan reports
/// the same authenticated canonical pair as `SharedFeatureFlatStack`, its
/// direct hinge is outside `moving_hinges`, and that hinge remains bit-exact
/// 180 degrees in both the initial and sampled poses. Every other blocking
/// observation remains blocking in this admission-scoped diagnostic; ordinary
/// analytic topology bypasses cannot substitute for a rejected callback.
/// The resulting diagnostic can never report a continuous certificate.
pub fn diagnose_collective_hinge_path_with_initial_sample_layer_admission_v1<T>(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    diagnose_collective_hinge_path_with_initial_sample_layer_admission_with_control_v1(
        model,
        initial_pose,
        moving_hinges,
        requested_angle_degrees,
        paper_thickness_mm,
        limits,
        admission,
        &CooperativeOperationControlV1::unbounded(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn diagnose_collective_hinge_path_with_initial_sample_layer_admission_with_control_v1<T>(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    path_checkpoint_v1(control)?;
    if paper_thickness_mm.to_bits() != 0.0_f64.to_bits() {
        return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable);
    }
    let (source_absolute, target_absolute) =
        collective_path_absolute_angles_v1(initial_pose, moving_hinges, requested_angle_degrees)?;
    let matches_sample_snapshot =
        |sample_index: usize,
         sample_pose: &MaterialTreePose,
         snapshot: &StaticCollisionDiagnosticSnapshot| {
            sampled_layer_admission_matches_snapshot_v1(
                admission,
                SampledLayerAdmissionSnapshotV1 {
                    model,
                    initial_pose,
                    moving_hinges,
                    sample_index,
                    sample_pose,
                    paper_thickness_mm,
                    snapshot,
                },
            )
        };
    diagnose_collective_hinge_path_from_pose_with_optional_authorities_v1(
        model,
        initial_pose,
        source_absolute,
        target_absolute.as_slice(),
        paper_thickness_mm,
        limits,
        None,
        Some(&matches_sample_snapshot),
        control,
    )
}

fn collective_path_absolute_angles_v1<'a>(
    initial_pose: &'a MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
) -> Result<(&'a [HingeAngle], CanonicalHingeAngles), StackedFoldPathDiagnosticErrorV1> {
    let source_absolute = initial_pose.hinge_angles();
    if source_absolute.iter().any(|hinge| {
        moving_hinges.contains(&hinge.edge())
            && hinge.angle_degrees().to_bits() != 0.0_f64.to_bits()
    }) {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    let target_absolute = CanonicalHingeAngles::new(
        source_absolute
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if moving_hinges.contains(&hinge.edge()) {
                        requested_angle_degrees
                    } else {
                        hinge.angle_degrees()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::InvalidPath)?,
    )
    .map_err(|_| StackedFoldPathDiagnosticErrorV1::InvalidPath)?;
    Ok((source_absolute, target_absolute))
}

/// Diagnoses a collective path whose endpoints are explicit absolute hinge
/// angles.  The source is bound bit-for-bit to `initial_pose`; consequently a
/// caller cannot reuse a diagnosis after replacing or partially changing the
/// authenticated source pose.
pub fn diagnose_collective_hinge_path_from_pose_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    source_absolute: &[HingeAngle],
    target_absolute: &[HingeAngle],
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    diagnose_collective_hinge_path_from_pose_with_control_v1(
        model,
        initial_pose,
        source_absolute,
        target_absolute,
        paper_thickness_mm,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn diagnose_collective_hinge_path_from_pose_with_control_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    source_absolute: &[HingeAngle],
    target_absolute: &[HingeAngle],
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    path_checkpoint_v1(control)?;
    diagnose_collective_hinge_path_from_pose_with_optional_cache_v1(
        model,
        initial_pose,
        source_absolute,
        target_absolute,
        paper_thickness_mm,
        limits,
        None,
        control,
    )
}

#[allow(clippy::too_many_arguments)]
fn diagnose_collective_hinge_path_from_pose_with_optional_cache_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    source_absolute: &[HingeAngle],
    target_absolute: &[HingeAngle],
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    pair_cache: Option<&PositiveEndpointPairCacheUseV1<'_>>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    diagnose_collective_hinge_path_from_pose_with_optional_authorities_v1(
        model,
        initial_pose,
        source_absolute,
        target_absolute,
        paper_thickness_mm,
        limits,
        pair_cache,
        None,
        control,
    )
}

type SampledLayerSnapshotAdmissionMatcherV1<'a> =
    dyn Fn(usize, &MaterialTreePose, &StaticCollisionDiagnosticSnapshot) -> bool + 'a;

#[allow(clippy::too_many_arguments)]
fn diagnose_collective_hinge_path_from_pose_with_optional_authorities_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    source_absolute: &[HingeAngle],
    target_absolute: &[HingeAngle],
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    pair_cache: Option<&PositiveEndpointPairCacheUseV1<'_>>,
    sampled_layer_snapshot_matches: Option<&SampledLayerSnapshotAdmissionMatcherV1<'_>>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    if initial_pose.hinge_angles() != source_absolute
        || source_absolute.len() != target_absolute.len()
        || source_absolute
            .iter()
            .zip(target_absolute.iter())
            .any(|(source, target)| source.edge() != target.edge())
    {
        return Err(StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch);
    }
    let changed = source_absolute
        .iter()
        .zip(target_absolute.iter())
        .filter(|(source, target)| {
            source.angle_degrees().to_bits() != target.angle_degrees().to_bits()
        })
        .collect::<Vec<_>>();
    let Some((_, first_target)) = changed.first().copied() else {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    };
    if changed.iter().any(|(_, target)| {
        target.angle_degrees().to_bits() != first_target.angle_degrees().to_bits()
    }) {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    let path_excursion_degrees = changed
        .iter()
        .map(|(source, target)| (target.angle_degrees() - source.angle_degrees()).abs())
        .fold(0.0_f64, f64::max);
    diagnose_collective_hinge_path_absolute_inner_v1(
        model,
        initial_pose,
        &changed
            .iter()
            .map(|(source, _)| source.edge())
            .collect::<Vec<_>>(),
        first_target.angle_degrees(),
        path_excursion_degrees,
        paper_thickness_mm,
        limits,
        pair_cache,
        sampled_layer_snapshot_matches,
        control,
    )
}

#[allow(clippy::too_many_arguments)]
fn diagnose_collective_hinge_path_absolute_inner_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
    path_excursion_degrees: f64,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    pair_cache: Option<&PositiveEndpointPairCacheUseV1<'_>>,
    sampled_layer_snapshot_matches: Option<&SampledLayerSnapshotAdmissionMatcherV1<'_>>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    path_checkpoint_v1(control)?;
    if limits.sample_intervals == 0 || limits.sample_intervals > MAX_STACKED_FOLD_PATH_SAMPLES_V1 {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidLimits);
    }
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || requested_angle_degrees > 180.0
        || !path_excursion_degrees.is_finite()
        || path_excursion_degrees <= 0.0
        || moving_hinges.is_empty()
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    model
        .bind_pose(initial_pose)
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch)?;
    let moving = moving_hinges.iter().copied().collect::<HashSet<_>>();
    if moving.len() != moving_hinges.len()
        || !moving
            .iter()
            .all(|edge| model.hinges().iter().any(|hinge| hinge.edge() == *edge))
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    // Native narrow theorem: a simulation-ready material model containing
    // exactly two faces joined by its only hinge has exactly one unordered
    // face pair. Starting that hinge at bit-exact zero and rotating it
    // monotonically through [0, 180] cannot create a transversal intersection:
    // the two rigid material planes meet only on the shared axis until the
    // terminal flat-stack contact. Positive thickness and every larger graph
    // remain outside this theorem.
    let analytic_single_hinge_topology = model.face_ids().len() == 2
        && model.hinges().len() == 1
        && moving.len() == 1
        && initial_pose
            .hinge_angles()
            .iter()
            .find(|angle| moving.contains(&angle.edge()))
            .is_some_and(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits());
    let zero_thickness = paper_thickness_mm.to_bits() == 0.0_f64.to_bits();
    let analytic_collinear_tree_topology = zero_thickness
        && collinear_collective_tree_premises(
            model,
            initial_pose,
            &moving,
            requested_angle_degrees,
        );
    let positive_thickness = paper_thickness_mm.is_finite() && paper_thickness_mm > 0.0;
    let mut interval_metrics = (0_usize, 0_usize);
    let interval_two_hinge_chain_topology = zero_thickness
        && two_hinge_interval_clearance_premises(
            model,
            initial_pose,
            &moving,
            requested_angle_degrees,
            limits.sample_intervals,
            &mut interval_metrics,
            control,
        );
    if interval_metrics.0 == usize::MAX {
        path_checkpoint_v1(control)?;
        return Err(StackedFoldPathDiagnosticErrorV1::Cancelled);
    }
    let positive_two_hinge_topology = positive_thickness
        && positive_tree_resource_premises_v1(
            model.face_ids().len(),
            model.hinges().len(),
            moving.len(),
        )
        && positive_tree_max_angle_degrees_v1(model.hinges().len())
            .is_some_and(|maximum| path_excursion_degrees <= maximum);
    let mut all_positive_thickness_outer_shells = positive_thickness;

    let has_sampled_layer_admission = sampled_layer_snapshot_matches.is_some();
    let mut sampled_nonblocking_pose_count = 0;
    let mut first_sampled_blocking_angle_degrees = None;
    let mut positive_endpoint_memo_pair_entries = 0;
    let mut positive_endpoint_exact_pair_calls = 0;
    for index in 0..=limits.sample_intervals {
        path_checkpoint_v1(control)?;
        let progress = index as f64 / limits.sample_intervals as f64;
        let angle = requested_angle_degrees * progress;
        let angles = initial_pose
            .hinge_angles()
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if moving.contains(&hinge.edge()) {
                        hinge.angle_degrees()
                            + (requested_angle_degrees - hinge.angle_degrees()) * progress
                    } else {
                        hinge.angle_degrees()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseUnavailable)?;
        let angles = CanonicalHingeAngles::new(angles)
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseUnavailable)?;
        let pose = model
            .solve(initial_pose.fixed_face(), &angles)
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseUnavailable)?;
        if positive_thickness && index > 0 && index < limits.sample_intervals {
            // For the strict two-triangle/one-hinge class up to a right angle,
            // radial separation changes monotonically. The requested endpoint
            // is therefore the worst finite-corridor case; intermediate
            // static recomputation would only duplicate that bounded proof.
            sampled_nonblocking_pose_count += 1;
            continue;
        }
        if positive_thickness && index == limits.sample_intervals {
            let bound = model
                .bind_pose(&pose)
                .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch)?;
            let endpoint_candidates = positive_two_hinge_topology
                .then(|| positive_endpoint_candidates_v1(model, &pose, paper_thickness_mm))
                .flatten();
            let endpoint_topology = if endpoint_candidates.as_ref().is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|(first, second)| !faces_share_material_vertex_v1(model, *first, *second))
            }) {
                Some(
                    prepare_positive_thickness_tree_endpoint_topology_memo_v1(
                        model,
                        &pose,
                        paper_thickness_mm,
                        limits.static_collision,
                    )
                    .map_err(|_| StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)?,
                )
            } else {
                None
            };
            all_positive_thickness_outer_shells &= if positive_two_hinge_topology {
                let boundary_proven =
                    prepare_swept_tree_hinge_thickness_boundaries_v1(bound, paper_thickness_mm)
                        .ok()
                        .flatten()
                        .is_some_and(|boundary| {
                            revalidate_tree_hinge_thickness_boundaries_v1(
                                &boundary,
                                bound,
                                paper_thickness_mm,
                            )
                            .is_some_and(|observations| observations.len() == model.hinges().len())
                        });
                if let (Some(endpoint_candidates), true) =
                    (endpoint_candidates.as_ref(), boundary_proven)
                {
                    let expected_pairs =
                        model.face_ids().len() * model.face_ids().len().saturating_sub(1) / 2;
                    let mut exact_pairs = Vec::new();
                    for (index, first) in model.face_ids().iter().enumerate() {
                        for second in model.face_ids().iter().skip(index + 1) {
                            path_checkpoint_v1(control)?;
                            let adjacent = model.hinges().iter().any(|hinge| {
                                (hinge.left_face() == *first && hinge.right_face() == *second)
                                    || (hinge.left_face() == *second
                                        && hinge.right_face() == *first)
                            });
                            if adjacent || !endpoint_candidates.contains(&(*first, *second)) {
                                continue;
                            }
                            positive_endpoint_memo_pair_entries += 1;
                            if faces_share_material_vertex_v1(model, *first, *second)
                                || endpoint_topology.as_ref().is_some_and(|memo| {
                                    memo.enumerated_pairs() == expected_pairs
                                        && memo.proves_shared_vertex_pair(*first, *second)
                                })
                            {
                                continue;
                            }
                            exact_pairs.push((*first, *second));
                        }
                    }
                    positive_endpoint_exact_pair_calls = exact_pairs.len();
                    if let Some(cache) = pair_cache {
                        prove_positive_endpoint_pairs_with_cache_v1(
                            bound,
                            paper_thickness_mm,
                            &exact_pairs,
                            expected_pairs,
                            cache,
                        )?
                    } else if exact_pairs.is_empty() {
                        true
                    } else if limits.static_collision != StaticCollisionLimits::default() {
                        // A caller-supplied static limit is an active proof
                        // boundary, not a performance hint. Preserve the
                        // established full-snapshot preflight until every
                        // field has an exact-session equivalent; otherwise a
                        // tight limit could be silently widened here. The
                        // legacy capability is only an additional preflight:
                        // the V2 exact prism kernel remains authoritative so
                        // its penetrating result can never be weakened by an
                        // older broadphase exclusion.
                        exact_pairs.iter().all(|(first, second)| {
                            prepare_positive_thickness_pair_separation_v1(
                                bound,
                                paper_thickness_mm,
                                *first,
                                *second,
                                limits.static_collision,
                            )
                            .is_ok_and(|capability| {
                                capability.is_some_and(|capability| {
                                    revalidate_positive_thickness_pair_separation_v1(
                                        &capability,
                                        bound,
                                        paper_thickness_mm,
                                    )
                                })
                            })
                        }) && prepare_positive_thickness_exact_endpoint_session_v2(
                            bound,
                            paper_thickness_mm,
                        )
                        .is_ok_and(|session| {
                            exact_pairs.iter().all(|(first, second)| {
                                session
                                    .exact_pair_strictly_separated_v2(*first, *second)
                                    .is_ok_and(|separated| separated)
                            })
                        })
                    } else {
                        prepare_positive_thickness_exact_endpoint_session_v2(
                            bound,
                            paper_thickness_mm,
                        )
                        .is_ok_and(|session| {
                            exact_pairs.iter().all(|(first, second)| {
                                session
                                    .exact_pair_strictly_separated_v2(*first, *second)
                                    .is_ok_and(|separated| separated)
                            })
                        })
                    }
                } else {
                    false
                }
            } else {
                prepare_single_hinge_thickness_boundary_v1(bound, paper_thickness_mm)
                    .ok()
                    .flatten()
                    .is_some_and(|boundary| {
                        revalidate_single_hinge_thickness_boundary_v1(
                            &boundary,
                            bound,
                            paper_thickness_mm,
                        )
                        .is_some()
                    })
            };
            if all_positive_thickness_outer_shells {
                // The opaque boundary capability is issued only after the
                // complete shared-hinge solid classifier returns Allowed.
                // Re-running the general static entrypoint would duplicate
                // that exact work and can exhaust its independent meter.
                sampled_nonblocking_pose_count += 1;
                continue;
            }
            first_sampled_blocking_angle_degrees.get_or_insert(angle);
            continue;
        }
        if positive_thickness && index == 0 {
            sampled_nonblocking_pose_count += 1;
            continue;
        }
        let snapshot = diagnose_static_collision_geometry_with_control_v1(
            model,
            &pose,
            paper_thickness_mm,
            limits.static_collision,
            control,
        )
        .map_err(|error| match error {
            crate::StaticCollisionError::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
            crate::StaticCollisionError::DeadlineExceeded => {
                StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
            }
            _ => StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable,
        })?;
        let narrow_shared_hinge_classified = analytic_single_hinge_topology
            && snapshot.expected_unordered_face_pairs() == 1
            && snapshot.pairs().len() == 1
            && snapshot.penetrating_pairs() == 0
            && snapshot.pairs().iter().all(|pair| {
                if positive_thickness {
                    pair.shared_hinge_solid_classified()
                } else {
                    pair.shared_hinge_boundary_contact_proven()
                }
            });
        let sampled_layer_order_admitted = match sampled_layer_snapshot_matches {
            Some(matches) if matches(index, &pose, &snapshot) => true,
            Some(_) if index == 0 => {
                return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable);
            }
            Some(_) | None => false,
        };
        let ordinary_analytic_blocking_bypass = !has_sampled_layer_admission
            && ((zero_thickness && analytic_single_hinge_topology)
                || (zero_thickness && analytic_collinear_tree_topology)
                || (zero_thickness && interval_two_hinge_chain_topology)
                || narrow_shared_hinge_classified);
        if snapshot.has_prominent_blocking_hold()
            && !sampled_layer_order_admitted
            && !ordinary_analytic_blocking_bypass
        {
            first_sampled_blocking_angle_degrees.get_or_insert(angle);
        } else {
            sampled_nonblocking_pose_count += 1;
        }
    }
    Ok(StackedFoldBoundedPathDiagnosticV1 {
        sampled_pose_count: limits.sample_intervals + 1,
        sampled_nonblocking_pose_count,
        first_sampled_blocking_angle_degrees,
        requested_angle_degrees,
        analytic_single_hinge_clearance: analytic_single_hinge_topology
            && !has_sampled_layer_admission
            && (!positive_thickness || requested_angle_degrees <= 90.0)
            && (zero_thickness || all_positive_thickness_outer_shells)
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        analytic_collinear_tree_clearance: analytic_collinear_tree_topology
            && !has_sampled_layer_admission
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        analytic_positive_two_hinge_clearance: positive_two_hinge_topology
            && !has_sampled_layer_admission
            && positive_tree_max_angle_degrees_v1(model.hinges().len())
                .is_some_and(|maximum| path_excursion_degrees <= maximum)
            && all_positive_thickness_outer_shells
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        interval_two_hinge_chain_clearance: interval_two_hinge_chain_topology
            && !has_sampled_layer_admission
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        interval_tree_hinge_count: if interval_two_hinge_chain_topology {
            moving.len()
        } else {
            0
        },
        interval_leaf_count: interval_metrics.0,
        interval_pair_work: interval_metrics.1,
        positive_endpoint_memo_pair_entries,
        positive_endpoint_exact_pair_calls,
        positive_thickness_outer_shell: positive_thickness && all_positive_thickness_outer_shells,
    })
}

fn two_hinge_interval_clearance_premises(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    requested_angle_degrees: f64,
    interval_count: usize,
    metrics: &mut (usize, usize),
    control: &CooperativeOperationControlV1<'_>,
) -> bool {
    let hinge_count = model.hinges().len();
    let face_count = model.face_ids().len();
    let Some(_pair_count) = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|n| n / 2)
    else {
        return false;
    };
    if !(2..=MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1).contains(&hinge_count)
        || face_count != hinge_count + 1
        || moving.len() != hinge_count
        || interval_count == 0
        || interval_count > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || initial_pose.fixed_face().is_none()
        || !initial_pose.hinge_angles().iter().all(|angle| {
            moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 0.0_f64.to_bits()
        })
    {
        return false;
    }
    let Some(first_line) = world_hinge_line(initial_pose, &model.hinges()[0]) else {
        return false;
    };
    if model.hinges()[1..].iter().all(|hinge| {
        world_hinge_line(initial_pose, hinge).is_some_and(|line| {
            exact_collinear_line(first_line.0, first_line.2, line.0, line.2)
                && exact_collinear_line(first_line.0, first_line.2, line.1, line.2)
        })
    }) {
        return false;
    }

    let Some(root) = initial_pose.fixed_face() else {
        return false;
    };
    let mut depth = HashMap::<FaceId, usize>::new();
    depth.insert(root, 0);
    let mut queue = VecDeque::from([root]);
    while let Some(face) = queue.pop_front() {
        let parent_depth = depth[&face];
        for hinge in model.hinges() {
            let next = if hinge.left_face() == face {
                Some(hinge.right_face())
            } else if hinge.right_face() == face {
                Some(hinge.left_face())
            } else {
                None
            };
            if let Some(next) = next
                && let std::collections::hash_map::Entry::Vacant(entry) = depth.entry(next)
            {
                let Some(next_depth) = parent_depth.checked_add(1) else {
                    return false;
                };
                entry.insert(next_depth);
                queue.push_back(next);
            }
        }
    }
    if depth.len() != face_count {
        return false;
    }

    let mut material_points = Vec::new();
    for face in model.face_ids() {
        let Some(boundary) = model.face_boundary(*face) else {
            return false;
        };
        for vertex in boundary.vertices() {
            let Some(point) = initial_pose.vertex_position(*vertex) else {
                return false;
            };
            material_points.push(point);
        }
    }
    let hinge_points = model
        .hinges()
        .iter()
        .flat_map(|hinge| [hinge.start(), hinge.end()])
        .collect::<Vec<_>>();
    let mut maximum_radius = 0.0_f64;
    for point in &material_points {
        for origin in &hinge_points {
            let distance = ((point.x() - origin.x()).powi(2)
                + (point.y() - origin.y()).powi(2)
                + (point.z() - origin.z()).powi(2))
            .sqrt();
            if !distance.is_finite() {
                return false;
            }
            maximum_radius = maximum_radius.max(distance);
        }
    }
    if maximum_radius == 0.0 {
        return false;
    }

    let adjacent = |first: ori_domain::FaceId, second: ori_domain::FaceId| {
        model.hinges().iter().any(|hinge| {
            (hinge.left_face() == first && hinge.right_face() == second)
                || (hinge.left_face() == second && hinge.right_face() == first)
        })
    };
    // Build one path-wide conservative candidate set. A face at ancestry
    // depth d moves by at most d*r*theta, so pairs omitted by this rest-order
    // sweep remain strictly x-separated throughout every adaptive leaf.
    let full_width_radians = requested_angle_degrees * std::f64::consts::PI / 180.0;
    let mut path_bounds = Vec::with_capacity(face_count);
    for face in model.face_ids() {
        let expansion =
            *depth.get(face).unwrap_or(&usize::MAX) as f64 * maximum_radius * full_width_radians;
        if !expansion.is_finite() {
            return false;
        }
        let Some(transform) = initial_pose.face_transform(*face) else {
            return false;
        };
        let Some(boundary) = model.face_boundary(*face) else {
            return false;
        };
        let mut minimum_x = f64::INFINITY;
        let mut maximum_x = f64::NEG_INFINITY;
        for vertex in boundary.vertices() {
            let Some(point) = initial_pose.vertex_position(*vertex) else {
                return false;
            };
            let Ok(world) = transform.apply_point(point) else {
                return false;
            };
            minimum_x = minimum_x.min(world.x() - expansion);
            maximum_x = maximum_x.max(world.x() + expansion);
        }
        path_bounds.push((*face, minimum_x, maximum_x));
    }
    path_bounds.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.canonical_bytes().cmp(&right.0.canonical_bytes()))
    });
    let mut canonical_candidates = Vec::new();
    for first in 0..path_bounds.len() {
        if path_checkpoint_v1(control).is_err() {
            *metrics = (usize::MAX, 0);
            return false;
        }
        for second in first + 1..path_bounds.len() {
            if path_bounds[second].1 > path_bounds[first].2 {
                break;
            }
            let pair = (path_bounds[first].0, path_bounds[second].0);
            if !adjacent(pair.0, pair.1) {
                if canonical_candidates.len() >= MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1 {
                    return false;
                }
                canonical_candidates.push(pair);
            }
        }
    }
    canonical_candidates
        .sort_by_key(|(first, second)| (first.canonical_bytes(), second.canonical_bytes()));
    let mut pair_work = 0_usize;
    let mut interrupted = false;
    let mut evaluate = |lower: f64, upper: f64| -> Option<(bool, f64)> {
        if path_checkpoint_v1(control).is_err() {
            interrupted = true;
            return None;
        }
        let midpoint = (lower + upper) / 2.0;
        let half_width_radians = (upper - lower) * std::f64::consts::PI / 360.0;
        let pose = solve_collective_pose(model, initial_pose, moving, midpoint)?;
        let mut bounds = Vec::new();
        for face in model.face_ids() {
            let expansion = *depth.get(face)? as f64 * maximum_radius * half_width_radians;
            if !expansion.is_finite() {
                return None;
            }
            let transform = pose.face_transform(*face)?;
            let boundary = model.face_boundary(*face)?;
            let mut minimum = [f64::INFINITY; 3];
            let mut maximum = [f64::NEG_INFINITY; 3];
            for vertex in boundary.vertices() {
                let world = transform
                    .apply_point(initial_pose.vertex_position(*vertex)?)
                    .ok()?;
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    minimum[axis] = minimum[axis].min(value - expansion);
                    maximum[axis] = maximum[axis].max(value + expansion);
                }
            }
            bounds.push((*face, minimum, maximum));
        }
        let bounds = bounds
            .into_iter()
            .map(|(face, minimum, maximum)| (face, (minimum, maximum)))
            .collect::<HashMap<_, _>>();
        let mut strict_margin = f64::INFINITY;
        for (first, second) in &canonical_candidates {
            if path_checkpoint_v1(control).is_err() {
                interrupted = true;
                return None;
            }
            let first = bounds.get(first)?;
            let second = bounds.get(second)?;
            pair_work = pair_work.checked_add(1)?;
            if pair_work > MAX_STACKED_FOLD_INTERVAL_WORK_V1 {
                return None;
            }
            let pair_margin = (0..3)
                .map(|axis| (second.0[axis] - first.1[axis]).max(first.0[axis] - second.1[axis]))
                .max_by(f64::total_cmp)?;
            strict_margin = strict_margin.min(pair_margin);
        }
        Some((strict_margin > 0.0, strict_margin))
    };
    let mut pending = Vec::with_capacity(interval_count);
    for interval in 0..interval_count {
        if path_checkpoint_v1(control).is_err() {
            *metrics = (usize::MAX, 0);
            return false;
        }
        let lower = requested_angle_degrees * interval as f64 / interval_count as f64;
        let upper = requested_angle_degrees * (interval + 1) as f64 / interval_count as f64;
        let (certified, margin) = match evaluate(lower, upper) {
            Some(value) => value,
            None => {
                if interrupted {
                    *metrics = (usize::MAX, 0);
                }
                return false;
            }
        };
        pending.push((lower, upper, 0_usize, certified, margin));
    }
    let mut leaf_count = interval_count;
    while !pending.is_empty() {
        if path_checkpoint_v1(control).is_err() {
            *metrics = (usize::MAX, 0);
            return false;
        }
        // The least separated leaf is refined first. Lower endpoint and depth
        // are stable tie-breakers, independent of model storage order.
        pending.sort_by(|left, right| {
            left.4
                .total_cmp(&right.4)
                .then_with(|| left.0.total_cmp(&right.0))
                .then_with(|| left.2.cmp(&right.2))
        });
        let (lower, upper, subdivision_depth, certified, _) = pending.remove(0);
        if certified {
            continue;
        }
        let midpoint = (lower + upper) / 2.0;
        if subdivision_depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1
            || leaf_count >= MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
            || !midpoint.is_finite()
            || midpoint <= lower
            || midpoint >= upper
        {
            return false;
        }
        leaf_count += 1;
        for (child_lower, child_upper) in [(lower, midpoint), (midpoint, upper)] {
            if path_checkpoint_v1(control).is_err() {
                *metrics = (usize::MAX, 0);
                return false;
            }
            let (child_certified, child_margin) = match evaluate(child_lower, child_upper) {
                Some(value) => value,
                None => {
                    if interrupted {
                        *metrics = (usize::MAX, 0);
                    }
                    return false;
                }
            };
            pending.push((
                child_lower,
                child_upper,
                subdivision_depth + 1,
                child_certified,
                child_margin,
            ));
        }
    }
    *metrics = (leaf_count, pair_work);
    true
}

fn solve_collective_pose(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    angle: f64,
) -> Option<MaterialTreePose> {
    let angles = initial_pose
        .hinge_angles()
        .iter()
        .map(|hinge| {
            HingeAngle::new(
                hinge.edge(),
                if moving.contains(&hinge.edge()) {
                    angle
                } else {
                    hinge.angle_degrees()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .and_then(|angles| CanonicalHingeAngles::new(angles).ok())?;
    model.solve(initial_pose.fixed_face(), &angles).ok()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedFoldCyclePathDiagnosticV1 {
    certified: bool,
    first_closure_failure_angle_degrees: Option<f64>,
    leaf_count: usize,
    pair_work: usize,
    positive_thickness_bits: Option<u64>,
}

/// Opaque authority for one exact positive-thickness continuous schedule.
#[derive(Debug, Clone)]
pub struct PositiveThicknessContinuousCertificateV1 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    proof_leaf_count: usize,
    pair_work: usize,
}

impl PositiveThicknessContinuousCertificateV1 {
    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &ori_kinematics::CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        thickness: f64,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.fixed_face == fixed_face
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v2()
            && self.closure_hash == closure.partition_binding_fingerprint_v2()
            && self.thickness_bits == thickness.to_bits()
            && (1..=MAX_STACKED_FOLD_INTERVAL_LEAVES_V1).contains(&self.proof_leaf_count)
            && self.pair_work <= MAX_STACKED_FOLD_INTERVAL_WORK_V1
            && schedule.matches_binding(geometry, audit, fixed_face)
            && closure.fixed_face() == fixed_face
            && closure.schedule_binding_fingerprint_v2()
                == schedule.certificate_binding_fingerprint_v2()
            && closure.graph_binding_fingerprint_v1() == schedule.graph_binding_fingerprint_v1()
            && closure.every_leaf_covers_graph_v1(geometry)
    }

    #[must_use]
    pub const fn thickness_bits(&self) -> u64 {
        self.thickness_bits
    }
}

/// A cooperative stop observed while issuing a canonical multi-cycle
/// positive-thickness authority.  This is deliberately distinct from a
/// bounded-proof resource result: callers may retry a cancelled generation
/// without treating it as evidence that the path exceeded its proof budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CanonicalPositiveThicknessCyclePathControlErrorV1 {
    #[error("multi-cycle authority issuance was cancelled")]
    Cancelled,
    #[error("multi-cycle authority issuance reached its absolute deadline")]
    DeadlineExceeded,
}

fn canonical_positive_cycle_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CanonicalPositiveThicknessCyclePathControlErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded
        }
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum UniformCycleClosureRootsV1 {
    Roots(Vec<f64>),
    ProvenInfeasible { examined_leaves: usize },
    Indeterminate { examined_leaves: usize },
}

pub fn enumerate_uniform_cycle_closure_roots_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    initial_angles: &CanonicalHingeAngles,
    moving_edges: &[EdgeId],
    requested_angle_degrees: f64,
    max_leaves: usize,
) -> UniformCycleClosureRootsV1 {
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || max_leaves == 0
        || max_leaves > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || audit.closure_hinges().is_empty()
        || moving_edges.is_empty()
    {
        return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
    }
    let moving = moving_edges.iter().copied().collect::<HashSet<_>>();
    let initial_by_edge = initial_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<HashMap<_, _>>();
    if moving.len() != moving_edges.len()
        || initial_angles.as_slice().len() != geometry.hinges().len()
        || geometry.hinges().iter().any(|hinge| {
            !initial_by_edge.contains_key(&hinge.edge())
                || (moving.contains(&hinge.edge())
                    && initial_by_edge
                        .get(&hinge.edge())
                        .is_some_and(|angle| angle.to_bits() != 0.0_f64.to_bits()))
        })
    {
        return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
    }
    let residual = |angle: f64| -> Option<f64> {
        let values = initial_angles
            .as_slice()
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if moving.contains(&hinge.edge()) {
                        angle
                    } else {
                        hinge.angle_degrees()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let angles = CanonicalHingeAngles::new(values).ok()?;
        geometry
            .measure_spanning_closure(audit, fixed_face, &angles)
            .ok()
            .map(|value| value.maximum_error())
    };
    let Some(requested_residual) = residual(requested_angle_degrees) else {
        return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
    };
    let mut scale = 1.0_f64;
    for face in geometry.face_ids() {
        let Some(boundary) = geometry.face_boundary_vertices(*face) else {
            return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
        };
        for vertex in boundary {
            let Some(point) = geometry.vertex_position(*vertex) else {
                return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
            };
            scale = scale
                .max(point.x().abs())
                .max(point.y().abs())
                .max(point.z().abs());
        }
    }
    // Each spanning composition performs a bounded number of binary64
    // additions and multiplications per hinge. Gamma(n) with 64 operations
    // per hinge bounds their accumulated forward error at material scale.
    let operation_count = geometry.hinges().len().saturating_mul(64) as f64;
    let roundoff_bound =
        operation_count * f64::EPSILON / (1.0 - operation_count * f64::EPSILON) * scale.max(1.0);
    if requested_residual <= roundoff_bound {
        return UniformCycleClosureRootsV1::Roots(vec![requested_angle_degrees]);
    }
    let lipschitz = (geometry.hinges().len() as f64 * 2.0 + 1.0) * scale.max(1.0);
    let mut pending = vec![(0.0, requested_angle_degrees, 0_usize)];
    let mut roots = Vec::new();
    let mut leaves = 1_usize;
    let mut unresolved = false;
    while let Some((lower, upper, depth)) = pending.pop() {
        let midpoint = (lower + upper) / 2.0;
        let Some(value) = residual(midpoint) else {
            return UniformCycleClosureRootsV1::Indeterminate {
                examined_leaves: leaves,
            };
        };
        if midpoint > 0.0 && value <= roundoff_bound {
            roots.push(midpoint);
            continue;
        }
        let enclosure = lipschitz * (upper - lower) * std::f64::consts::PI / 360.0;
        if value > enclosure {
            continue;
        }
        if leaves >= max_leaves || depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1 {
            unresolved = true;
            continue;
        }
        leaves += 1;
        pending.push((midpoint, upper, depth + 1));
        pending.push((lower, midpoint, depth + 1));
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| a.to_bits() == b.to_bits());
    if !roots.is_empty() {
        UniformCycleClosureRootsV1::Roots(roots)
    } else if unresolved {
        UniformCycleClosureRootsV1::Indeterminate {
            examined_leaves: leaves,
        }
    } else {
        UniformCycleClosureRootsV1::ProvenInfeasible {
            examined_leaves: leaves,
        }
    }
}

impl StackedFoldCyclePathDiagnosticV1 {
    #[must_use]
    pub const fn continuous_certificate_model_id(&self) -> Option<&'static str> {
        if self.certified {
            Some(if self.positive_thickness_bits.is_some() {
                STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            } else {
                STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            })
        } else {
            None
        }
    }
    #[must_use]
    pub const fn first_closure_failure_angle_degrees(&self) -> Option<f64> {
        self.first_closure_failure_angle_degrees
    }
    #[must_use]
    pub const fn leaf_count(&self) -> usize {
        self.leaf_count
    }
    #[must_use]
    pub const fn pair_work(&self) -> usize {
        self.pair_work
    }
    #[must_use]
    pub const fn positive_thickness_bits(&self) -> Option<u64> {
        self.positive_thickness_bits
    }
}

/// Narrow cycle theorem for a collective, common-axis zero-thickness motion.
/// Closure at zero and one nonzero canonical spanning solution proves the
/// signed common-axis cycle identity; every adaptive midpoint/endpoint is
/// nevertheless revalidated before its swept boxes are admitted.
pub fn diagnose_collective_cycle_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    initial_angles: &CanonicalHingeAngles,
    moving_edges: &[EdgeId],
    requested_angle_degrees: f64,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    let failed = |angle| StackedFoldCyclePathDiagnosticV1 {
        certified: false,
        first_closure_failure_angle_degrees: angle,
        leaf_count: 0,
        pair_work: 0,
        positive_thickness_bits: None,
    };
    if audit.closure_hinges().is_empty()
        || geometry.hinges().len() > MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1
        || interval_count == 0
        || interval_count > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || requested_angle_degrees > 180.0
        || moving_edges.is_empty()
    {
        return failed(None);
    }
    let moving = moving_edges.iter().copied().collect::<HashSet<_>>();
    let initial_by_edge = initial_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<HashMap<_, _>>();
    if moving.len() != moving_edges.len()
        || initial_angles.as_slice().len() != geometry.hinges().len()
        || geometry.hinges().iter().any(|hinge| {
            !initial_by_edge.contains_key(&hinge.edge())
                || (moving.contains(&hinge.edge())
                    && initial_by_edge
                        .get(&hinge.edge())
                        .is_some_and(|angle| angle.to_bits() != 0.0_f64.to_bits()))
        })
    {
        return failed(None);
    }
    let angles_at = |angle: f64| {
        CanonicalHingeAngles::new(
            initial_angles
                .as_slice()
                .iter()
                .map(|hinge| {
                    HingeAngle::new(
                        hinge.edge(),
                        if moving.contains(&hinge.edge()) {
                            angle
                        } else {
                            hinge.angle_degrees()
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .ok()?,
        )
        .ok()
    };
    let solve = |angle: f64| {
        geometry
            .solve_closed(audit, fixed_face, &angles_at(angle)?, 1.0e-9)
            .ok()
    };
    if solve(0.0).is_none() {
        return failed(Some(0.0));
    }
    let Some(reference) = geometry.hinges().first() else {
        return failed(None);
    };
    let direction = reference.axis();
    if geometry.hinges().iter().skip(1).any(|hinge| {
        !exact_collinear_line(reference.start(), direction, hinge.start(), hinge.axis())
            || !exact_collinear_line(reference.start(), direction, hinge.end(), hinge.axis())
    }) {
        return failed(None);
    }
    let mut maximum_radius = 0.0_f64;
    for face in geometry.face_ids() {
        let Some(boundary) = geometry.face_boundary_vertices(*face) else {
            return failed(None);
        };
        for vertex in boundary {
            let Some(point) = geometry.vertex_position(*vertex) else {
                return failed(None);
            };
            for hinge in geometry.hinges() {
                for origin in [hinge.start(), hinge.end()] {
                    maximum_radius = maximum_radius.max(
                        ((point.x() - origin.x()).powi(2)
                            + (point.y() - origin.y()).powi(2)
                            + (point.z() - origin.z()).powi(2))
                        .sqrt(),
                    );
                }
            }
        }
    }
    if !maximum_radius.is_finite() || maximum_radius == 0.0 {
        return failed(None);
    }
    let adjacent = |a: FaceId, b: FaceId| {
        geometry.hinges().iter().any(|hinge| {
            (hinge.left_face() == a && hinge.right_face() == b)
                || (hinge.left_face() == b && hinge.right_face() == a)
        })
    };
    let mut pending = (0..interval_count)
        .map(|index| {
            (
                requested_angle_degrees * index as f64 / interval_count as f64,
                requested_angle_degrees * (index + 1) as f64 / interval_count as f64,
                0_usize,
            )
        })
        .collect::<Vec<_>>();
    let mut leaves = interval_count;
    let mut work = 0_usize;
    while let Some((lower, upper, depth)) = pending.pop() {
        let midpoint = (lower + upper) / 2.0;
        for angle in [lower, midpoint, upper] {
            if solve(angle).is_none() {
                return failed(Some(angle));
            }
        }
        let Some(pose) = solve(midpoint) else {
            return failed(Some(midpoint));
        };
        let expansion = geometry.hinges().len() as f64
            * maximum_radius
            * (upper - lower)
            * std::f64::consts::PI
            / 360.0;
        let mut bounds = Vec::new();
        for face in geometry.face_ids() {
            let Some(transform) = pose.face_transform(*face) else {
                return failed(Some(midpoint));
            };
            let Some(boundary) = geometry.face_boundary_vertices(*face) else {
                return failed(None);
            };
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for vertex in boundary {
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return failed(None);
                };
                let Ok(world) = transform.apply_point(point) else {
                    return failed(None);
                };
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    min[axis] = min[axis].min(value - expansion);
                    max[axis] = max[axis].max(value + expansion);
                }
            }
            bounds.push((*face, min, max));
        }
        let mut clear = true;
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if adjacent(bounds[first].0, bounds[second].0) {
                    continue;
                }
                work = match work.checked_add(1) {
                    Some(v) if v <= MAX_STACKED_FOLD_INTERVAL_WORK_V1 => v,
                    _ => return failed(None),
                };
                if !(0..3).any(|axis| {
                    bounds[first].2[axis] < bounds[second].1[axis]
                        || bounds[second].2[axis] < bounds[first].1[axis]
                }) {
                    clear = false;
                    break;
                }
            }
            if !clear {
                break;
            }
        }
        if !clear {
            if depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1
                || leaves >= MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
            {
                return failed(None);
            }
            leaves += 1;
            pending.push((lower, midpoint, depth + 1));
            pending.push((midpoint, upper, depth + 1));
        }
    }
    StackedFoldCyclePathDiagnosticV1 {
        certified: true,
        first_closure_failure_angle_degrees: None,
        leaf_count: leaves,
        pair_work: work,
        positive_thickness_bits: None,
    }
}

/// Conservatively certifies zero-thickness clearance for the exact same
/// per-hinge schedule already carrying a full-domain closure certificate.
/// This remains observation-only and never authorizes mutation.
pub fn diagnose_scheduled_cycle_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_v1(
        geometry,
        audit,
        fixed_face,
        candidate.schedule(),
        closure,
        interval_count,
    )
}

/// Certifies a cactus schedule using thickness-expanded swept bounds and exact
/// positive-thickness endpoint/midpoint revalidation on every adaptive leaf.
pub fn diagnose_scheduled_positive_thickness_cycle_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_internal_v1(
        geometry,
        audit,
        fixed_face,
        candidate.schedule(),
        closure,
        interval_count,
        Some(paper_thickness_mm),
        None,
        None,
    )
}

/// Certifies an issuer-bound canonical positive-thickness schedule directly.
/// This entry point includes stationary schedules, which intentionally have no
/// generated motion candidate but still need reusable closure/clearance proof.
pub fn diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_internal_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        closure,
        interval_count,
        Some(paper_thickness_mm),
        None,
        None,
    )
}

/// Mints an issuer-bound authority only after the full positive-thickness
/// continuous classifier succeeds for the exact schedule and closure.
pub fn certify_canonical_positive_thickness_cycle_schedule_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    interval_count: usize,
) -> Option<PositiveThicknessContinuousCertificateV1> {
    certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        closure,
        paper_thickness_mm,
        interval_count,
        &CooperativeOperationControlV1::unbounded(),
    )
    .ok()
    .flatten()
}

/// Controlled variant of [`certify_canonical_positive_thickness_cycle_schedule_path_v1`].
///
/// It performs cooperative checks while consuming each closure leaf and each
/// recognised cactus/symmetric cycle group, and once more immediately before
/// authority minting.  A stop never exposes a partial certificate.
#[allow(
    clippy::too_many_arguments,
    reason = "the controlled compatibility API adds only the operation control to the established issuer contract"
)]
pub fn certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    interval_count: usize,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    Option<PositiveThicknessContinuousCertificateV1>,
    CanonicalPositiveThicknessCyclePathControlErrorV1,
> {
    canonical_positive_cycle_checkpoint_v1(control)?;
    if !closure_every_leaf_covers_graph_with_control_v1(closure, geometry, Some(control))? {
        return Ok(None);
    }
    let groups = match composed_symmetric_rational_local_groups_with_control_v1(
        geometry, audit, fixed_face, schedule, control,
    )? {
        Some(groups) => Some(groups),
        None => rational_cactus_star_local_groups_with_control_v1(
            geometry, audit, fixed_face, schedule, control,
        )?,
    };
    let diagnostic = diagnose_canonical_cycle_schedule_path_internal_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        closure,
        interval_count,
        Some(paper_thickness_mm),
        Some(control),
        groups.as_ref(),
    );
    canonical_positive_cycle_checkpoint_v1(control)?;
    Ok((diagnostic.continuous_certificate_model_id()
        == Some(STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1))
    .then(|| PositiveThicknessContinuousCertificateV1 {
        issuer: geometry.clone(),
        fixed_face,
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        closure_hash: closure.partition_binding_fingerprint_v2(),
        thickness_bits: paper_thickness_mm.to_bits(),
        proof_leaf_count: diagnostic.leaf_count(),
        pair_work: diagnostic.pair_work(),
    }))
}

/// Runs the bounded cycle CCD oracle against a canonical schedule directly.
/// Schedule families without point evaluation or a finite derivative bound
/// remain explicitly uncertified; closure evidence alone is never clearance.
pub fn diagnose_canonical_cycle_schedule_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_internal_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        closure,
        interval_count,
        None,
        None,
        None,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the private shared diagnostic binds graph, audit, schedule, closure, thickness, control, and prevalidated family evidence at one authority boundary"
)]
fn diagnose_canonical_cycle_schedule_path_internal_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
    paper_thickness_mm: Option<f64>,
    operation_control: Option<&CooperativeOperationControlV1<'_>>,
    _precomputed_local_symmetric_groups: Option<&HashMap<FaceId, usize>>,
) -> StackedFoldCyclePathDiagnosticV1 {
    let failed = || StackedFoldCyclePathDiagnosticV1 {
        certified: false,
        first_closure_failure_angle_degrees: None,
        leaf_count: 0,
        pair_work: 0,
        positive_thickness_bits: None,
    };
    let operation_is_current =
        || operation_control.is_none_or(|control| control.checkpoint().is_ok());
    let closure_covers_graph =
        match closure_every_leaf_covers_graph_with_control_v1(closure, geometry, operation_control)
        {
            Ok(covers) => covers,
            Err(_) => return failed(),
        };
    if !operation_is_current()
        || interval_count == 0
        || interval_count > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || !closure_covers_graph
        || closure.fixed_face() != fixed_face
        || closure.schedule_binding_fingerprint_v2()
            != schedule.certificate_binding_fingerprint_v2()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || !schedule.matches_binding(geometry, audit, fixed_face)
        || paper_thickness_mm.is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return failed();
    }
    let derivative_sum = geometry
        .hinges()
        .iter()
        .try_fold(0.0, |sum, hinge| {
            if !operation_is_current() {
                return None;
            }
            schedule
                .derivative_bound(hinge.edge())
                .map(|bound| sum + bound)
        })
        .filter(|value| value.is_finite());
    let Some(derivative_sum) = derivative_sum else {
        return failed();
    };
    if let Some(thickness) = paper_thickness_mm {
        if !operation_is_current() {
            return failed();
        }
        let Some(initial_angles) = schedule.evaluate(0.0) else {
            return failed();
        };
        let Ok(initial_pose) = geometry.solve_closed(audit, fixed_face, &initial_angles, 1.0e-9)
        else {
            return failed();
        };
        if prove_positive_thickness_graph_geometry_v1(
            geometry,
            &initial_pose,
            thickness,
            PositiveThicknessGraphLimitsV1::default(),
        )
        .is_err()
        {
            return failed();
        }
        if !operation_is_current() {
            return failed();
        }
    }
    if paper_thickness_mm.is_none()
        && scheduled_collinear_flat_stack_premises_v1(geometry, audit, fixed_face, schedule)
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: 0,
            positive_thickness_bits: None,
        };
    }
    if paper_thickness_mm.is_none()
        && scheduled_kawasaki_120_120_60_60_premises_v1(geometry, audit, fixed_face, schedule)
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: 0,
            positive_thickness_bits: None,
        };
    }
    if scheduled_opposite_radial_bifold_premises_v1(geometry, audit, fixed_face, schedule, closure)
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: 0,
            positive_thickness_bits: paper_thickness_mm.map(f64::to_bits),
        };
    }
    let mut maximum_radius = 0.0_f64;
    for face in geometry.face_ids() {
        if !operation_is_current() {
            return failed();
        }
        let Some(boundary) = geometry.face_boundary_vertices(*face) else {
            return failed();
        };
        for vertex in boundary {
            if !operation_is_current() {
                return failed();
            }
            let Some(point) = geometry.vertex_position(*vertex) else {
                return failed();
            };
            for hinge in geometry.hinges() {
                if !operation_is_current() {
                    return failed();
                }
                let origin = hinge.start();
                maximum_radius = maximum_radius.max(
                    ((point.x() - origin.x()).powi(2)
                        + (point.y() - origin.y()).powi(2)
                        + (point.z() - origin.z()).powi(2))
                    .sqrt(),
                );
            }
        }
    }
    if !maximum_radius.is_finite() {
        return failed();
    }
    // A constant schedule is a useful, non-vacuous issuer path for arbitrary
    // closed material-hinge graphs (including graphs whose cycle rank exceeds
    // the specialised cactus/theta families).  Bind it to the exact schedule
    // and closure certificates, then run the same all-pair solid proof once.
    // This also avoids exhausting subdivision on coincident swept AABBs: with
    // a zero derivative bound the swept volume is exactly the current pose.
    if let Some(thickness) = paper_thickness_mm
        && derivative_sum.to_bits() == 0.0_f64.to_bits()
    {
        if !operation_is_current() {
            return failed();
        }
        let Some(angles) = schedule.evaluate(0.0) else {
            return failed();
        };
        let Ok(pose) = geometry.solve_closed(audit, fixed_face, &angles, 1.0e-9) else {
            return failed();
        };
        if prove_positive_thickness_graph_geometry_v1(
            geometry,
            &pose,
            thickness,
            PositiveThicknessGraphLimitsV1::default(),
        )
        .is_ok()
        {
            if !operation_is_current() {
                return failed();
            }
            let face_count = geometry.face_ids().len();
            return StackedFoldCyclePathDiagnosticV1 {
                certified: true,
                first_closure_failure_angle_degrees: None,
                leaf_count: closure.leaves().len(),
                pair_work: face_count * (face_count - 1) / 2,
                positive_thickness_bits: Some(thickness.to_bits()),
            };
        }
        return failed();
    }
    let adjacent = |a: FaceId, b: FaceId| {
        geometry.hinges().iter().any(|hinge| {
            (hinge.left_face() == a && hinge.right_face() == b)
                || (hinge.left_face() == b && hinge.right_face() == a)
        })
    };
    if !operation_is_current() {
        return failed();
    }
    if paper_thickness_mm.is_none()
        && theta_collective_axis_continuous_premises_v1(
            geometry, audit, fixed_face, schedule, closure,
        )
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: geometry.face_ids().len() * (geometry.face_ids().len() - 1) / 2,
            positive_thickness_bits: paper_thickness_mm.map(f64::to_bits),
        };
    }
    let mut pending = (0..interval_count)
        .map(|index| {
            (
                index as f64 / interval_count as f64,
                (index + 1) as f64 / interval_count as f64,
                0usize,
            )
        })
        .collect::<Vec<_>>();
    let mut leaves = interval_count;
    let mut work = 0usize;
    while let Some((lower, upper, depth)) = pending.pop() {
        if !operation_is_current() {
            return failed();
        }
        let midpoint = (lower + upper) * 0.5;
        let Some(angles) = schedule.evaluate(midpoint) else {
            return failed();
        };
        let Ok(pose) = geometry.solve_closed(audit, fixed_face, &angles, 1.0e-9) else {
            return failed();
        };
        if let Some(thickness) = paper_thickness_mm {
            for progress in [lower, midpoint, upper] {
                if !operation_is_current() {
                    return failed();
                }
                let Some(exact_angles) = schedule.evaluate(progress) else {
                    return failed();
                };
                let Ok(exact_pose) =
                    geometry.solve_closed(audit, fixed_face, &exact_angles, 1.0e-9)
                else {
                    return failed();
                };
                if prove_positive_thickness_graph_geometry_v1(
                    geometry,
                    &exact_pose,
                    thickness,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .is_err()
                {
                    return failed();
                }
                if !operation_is_current() {
                    return failed();
                }
            }
        }
        let expansion = maximum_radius * derivative_sum * (upper - lower) * std::f64::consts::PI
            / 180.0
            + paper_thickness_mm.unwrap_or(0.0) * 0.5;
        let mut bounds = Vec::new();
        for face in geometry.face_ids() {
            if !operation_is_current() {
                return failed();
            }
            let (Some(transform), Some(boundary)) = (
                pose.face_transform(*face),
                geometry.face_boundary_vertices(*face),
            ) else {
                return failed();
            };
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for vertex in boundary {
                if !operation_is_current() {
                    return failed();
                }
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return failed();
                };
                let Ok(world) = transform.apply_point(point) else {
                    return failed();
                };
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    if !operation_is_current() {
                        return failed();
                    }
                    min[axis] = min[axis].min(value - expansion);
                    max[axis] = max[axis].max(value + expansion);
                }
            }
            bounds.push((*face, min, max));
        }
        let mut clear = true;
        for first in 0..bounds.len() {
            if !operation_is_current() {
                return failed();
            }
            for second in first + 1..bounds.len() {
                if !operation_is_current() {
                    return failed();
                }
                if adjacent(bounds[first].0, bounds[second].0) {
                    continue;
                }
                work = match work.checked_add(1) {
                    Some(value) if value <= MAX_STACKED_FOLD_INTERVAL_WORK_V1 => value,
                    _ => return failed(),
                };
                if !(0..3).any(|axis| {
                    bounds[first].2[axis] < bounds[second].1[axis]
                        || bounds[second].2[axis] < bounds[first].1[axis]
                }) {
                    clear = false;
                    break;
                }
            }
        }
        if !clear {
            if depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1
                || leaves >= MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
            {
                return failed();
            }
            leaves += 1;
            pending.push((midpoint, upper, depth + 1));
            pending.push((lower, midpoint, depth + 1));
        }
    }
    if !operation_is_current() {
        return failed();
    }
    StackedFoldCyclePathDiagnosticV1 {
        certified: true,
        first_closure_failure_angle_degrees: None,
        leaf_count: leaves,
        pair_work: work,
        positive_thickness_bits: paper_thickness_mm.map(f64::to_bits),
    }
}

fn closure_every_leaf_covers_graph_with_control_v1(
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    geometry: &MaterialHingeGraphGeometry,
    operation_control: Option<&CooperativeOperationControlV1<'_>>,
) -> Result<bool, CanonicalPositiveThicknessCyclePathControlErrorV1> {
    for _ in closure.leaves() {
        if let Some(control) = operation_control {
            controlled_closure_leaf_checkpoint_v1(control)?;
        }
    }
    Ok(closure.every_leaf_covers_graph_v1(geometry))
}

fn equal_endpoint_moving_edges_v1(
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> Option<Vec<EdgeId>> {
    let initial = schedule.evaluate(0.0)?;
    let target = schedule.evaluate(1.0)?;
    let initial_by_edge = initial
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect::<HashMap<_, _>>();
    let moving = target
        .as_slice()
        .iter()
        .filter(|angle| {
            initial_by_edge.get(&angle.edge()).copied() != Some(angle.angle_degrees().to_bits())
        })
        .map(|angle| angle.edge())
        .collect::<Vec<_>>();
    let common = target
        .as_slice()
        .iter()
        .find(|angle| moving.contains(&angle.edge()))?
        .angle_degrees()
        .to_bits();
    (!moving.is_empty()
        && target.as_slice().iter().all(|angle| {
            !moving.contains(&angle.edge()) || angle.angle_degrees().to_bits() == common
        }))
    .then_some(moving)
}

fn theta_collective_axis_continuous_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
) -> bool {
    if !closure.every_leaf_covers_graph_v1(geometry) || closure.fixed_face() != fixed_face {
        return false;
    }
    theta_collective_axis_schedule_premises_v1(geometry, audit, fixed_face, schedule)
}

fn theta_collective_axis_schedule_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> bool {
    if geometry.face_ids().len() < 3 || audit.closure_hinges().is_empty() {
        return false;
    }
    let Some(moving) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    if moving.is_empty() {
        return false;
    }
    let moving_hinges = geometry
        .hinges()
        .iter()
        .filter(|hinge| moving.contains(&hinge.edge()))
        .collect::<Vec<_>>();
    let Some(reference) = moving_hinges.first() else {
        return false;
    };
    moving_hinges.iter().skip(1).all(|hinge| {
        exact_collinear_line(
            reference.start(),
            reference.axis(),
            hinge.start(),
            hinge.axis(),
        ) && exact_collinear_line(
            reference.start(),
            reference.axis(),
            hinge.end(),
            hinge.axis(),
        )
    }) && [0.0, 1.0].into_iter().all(|progress| {
        schedule.evaluate(progress).is_some_and(|angles| {
            angles
                .as_slice()
                .iter()
                .all(|angle| angle.angle_degrees() >= 0.0 && angle.angle_degrees() < 90.0)
                && geometry
                    .solve_closed(audit, fixed_face, &angles, 1.0e-9)
                    .is_ok()
        })
    })
}

/// Reports whether positive-thickness scheduled CCD has an exact specialised
/// theorem for this bound graph and schedule. This is a structural admission
/// check only; the positive-thickness proof must still succeed for the actual
/// thickness before a continuous certificate can be issued.
#[must_use]
pub fn supports_scheduled_positive_thickness_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> bool {
    if !schedule.matches_binding(geometry, audit, fixed_face) {
        return false;
    }
    if theta_collective_axis_schedule_premises_v1(geometry, audit, fixed_face, schedule) {
        return true;
    }
    if composed_symmetric_rational_local_groups_v1(geometry, audit, fixed_face, schedule)
        .or_else(|| rational_cactus_star_local_groups_v1(geometry, audit, fixed_face, schedule))
        .is_some()
    {
        return true;
    }
    let face_count = geometry.face_ids().len();
    (3usize..=9).any(|columns| {
        (3usize..=9).any(|rows| {
            columns * rows == face_count
                && geometry.hinges().len() == 2 * columns * rows - columns - rows
                && audit.closure_hinges().len() == (columns - 1) * (rows - 1)
                && schedule
                    .collective_profile_edges_v1()
                    .or_else(|| schedule.collective_half_angle_profile_edges_v1())
                    .is_some_and(|moving| {
                        moving.len() == rows * (columns - 1) || moving.len() == columns * (rows - 1)
                    })
        })
    })
}

fn composed_symmetric_rational_local_groups_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> Option<HashMap<FaceId, usize>> {
    composed_symmetric_rational_local_groups_with_control_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        &CooperativeOperationControlV1::unbounded(),
    )
    .ok()
    .flatten()
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlledCycleTestCheckpointV1 {
    ClosureLeaf,
    CompletedGroup,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct ControlledCycleTestStopV1 {
    checkpoint: ControlledCycleTestCheckpointV1,
    remaining: usize,
    error: CanonicalPositiveThicknessCyclePathControlErrorV1,
}

#[cfg(test)]
std::thread_local! {
    static CONTROLLED_CYCLE_TEST_STOP_V1:
        std::cell::RefCell<Option<ControlledCycleTestStopV1>> =
            const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) struct ControlledCycleTestStopGuardV1;

#[cfg(test)]
impl Drop for ControlledCycleTestStopGuardV1 {
    fn drop(&mut self) {
        CONTROLLED_CYCLE_TEST_STOP_V1.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

#[cfg(test)]
fn configure_controlled_cycle_test_stop_v1(
    checkpoint: ControlledCycleTestCheckpointV1,
    after: usize,
    error: CanonicalPositiveThicknessCyclePathControlErrorV1,
) -> ControlledCycleTestStopGuardV1 {
    assert!(after > 0, "a test stop must follow at least one checkpoint");
    CONTROLLED_CYCLE_TEST_STOP_V1.with(|slot| {
        let previous = slot.borrow_mut().replace(ControlledCycleTestStopV1 {
            checkpoint,
            remaining: after,
            error,
        });
        assert!(previous.is_none(), "nested controlled-cycle test stop");
    });
    ControlledCycleTestStopGuardV1
}

#[cfg(test)]
pub(super) fn configure_controlled_group_test_stop_v1(
    after_completed_groups: usize,
    error: CanonicalPositiveThicknessCyclePathControlErrorV1,
) -> ControlledCycleTestStopGuardV1 {
    configure_controlled_cycle_test_stop_v1(
        ControlledCycleTestCheckpointV1::CompletedGroup,
        after_completed_groups,
        error,
    )
}

#[cfg(test)]
pub(super) fn configure_controlled_closure_leaf_test_stop_v1(
    after_checked_leaves: usize,
    error: CanonicalPositiveThicknessCyclePathControlErrorV1,
) -> ControlledCycleTestStopGuardV1 {
    configure_controlled_cycle_test_stop_v1(
        ControlledCycleTestCheckpointV1::ClosureLeaf,
        after_checked_leaves,
        error,
    )
}

#[cfg(test)]
fn controlled_cycle_test_injected_stop_v1(
    checkpoint: ControlledCycleTestCheckpointV1,
) -> Option<CanonicalPositiveThicknessCyclePathControlErrorV1> {
    CONTROLLED_CYCLE_TEST_STOP_V1.with(|slot| {
        let mut slot = slot.borrow_mut();
        let stop = slot.as_mut()?;
        if stop.checkpoint != checkpoint || stop.remaining == 0 {
            return None;
        }
        stop.remaining -= 1;
        (stop.remaining == 0).then_some(stop.error)
    })
}

fn controlled_closure_leaf_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CanonicalPositiveThicknessCyclePathControlErrorV1> {
    canonical_positive_cycle_checkpoint_v1(control)?;
    #[cfg(test)]
    if let Some(error) =
        controlled_cycle_test_injected_stop_v1(ControlledCycleTestCheckpointV1::ClosureLeaf)
    {
        return Err(error);
    }
    Ok(())
}

fn controlled_group_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CanonicalPositiveThicknessCyclePathControlErrorV1> {
    canonical_positive_cycle_checkpoint_v1(control)
}

fn controlled_group_completed_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CanonicalPositiveThicknessCyclePathControlErrorV1> {
    canonical_positive_cycle_checkpoint_v1(control)?;
    #[cfg(test)]
    if let Some(error) =
        controlled_cycle_test_injected_stop_v1(ControlledCycleTestCheckpointV1::CompletedGroup)
    {
        return Err(error);
    }
    Ok(())
}

fn composed_symmetric_rational_local_groups_with_control_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<Option<HashMap<FaceId, usize>>, CanonicalPositiveThicknessCyclePathControlErrorV1> {
    canonical_positive_cycle_checkpoint_v1(control)?;
    let count = audit.closure_hinges().len();
    if !(2..=32).contains(&count)
        || geometry.hinges().len() != count * 4
        || geometry.face_ids().len() != 1 + count * 3
    {
        return Ok(None);
    }
    let mut remaining = HashSet::new();
    for face in geometry.face_ids() {
        controlled_group_checkpoint_v1(control)?;
        if *face != fixed_face {
            remaining.insert(*face);
        }
    }
    let mut result = HashMap::new();
    for group_index in 0..count {
        controlled_group_checkpoint_v1(control)?;
        let Some(seed) = remaining.iter().next().copied() else {
            return Ok(None);
        };
        let mut stack = vec![seed];
        let mut faces = HashSet::new();
        while let Some(face) = stack.pop() {
            controlled_group_checkpoint_v1(control)?;
            if !remaining.remove(&face) {
                continue;
            }
            faces.insert(face);
            for hinge in geometry.hinges() {
                controlled_group_checkpoint_v1(control)?;
                if hinge.left_face() == face && hinge.right_face() != fixed_face {
                    stack.push(hinge.right_face());
                } else if hinge.right_face() == face && hinge.left_face() != fixed_face {
                    stack.push(hinge.left_face());
                }
            }
        }
        if faces.len() != 3 {
            return Ok(None);
        }
        let mut edges = Vec::new();
        for hinge in geometry.hinges() {
            controlled_group_checkpoint_v1(control)?;
            if faces.contains(&hinge.left_face()) || faces.contains(&hinge.right_face()) {
                edges.push(hinge.edge());
            }
        }
        if schedule
            .bounded_symmetric_kawasaki_profile_for_edges_v1(&edges)
            .is_none()
        {
            return Ok(None);
        }
        for face in faces {
            controlled_group_checkpoint_v1(control)?;
            result.insert(face, group_index);
        }
        controlled_group_completed_checkpoint_v1(control)?;
    }
    Ok((!result.is_empty() && remaining.is_empty()).then_some(result))
}

fn rational_cactus_star_local_groups_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> Option<HashMap<FaceId, usize>> {
    rational_cactus_star_local_groups_with_control_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        &CooperativeOperationControlV1::unbounded(),
    )
    .ok()
    .flatten()
}

fn rational_cactus_star_local_groups_with_control_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<Option<HashMap<FaceId, usize>>, CanonicalPositiveThicknessCyclePathControlErrorV1> {
    controlled_group_checkpoint_v1(control)?;
    let count = audit.closure_hinges().len();
    if !(2..=32).contains(&count)
        || geometry.hinges().len() != count * 4
        || geometry.face_ids().len() != 1 + count * 3
    {
        return Ok(None);
    }
    for shared in geometry
        .face_ids()
        .iter()
        .copied()
        .filter(|face| *face != fixed_face)
    {
        controlled_group_checkpoint_v1(control)?;
        let mut remaining = HashSet::new();
        for face in geometry.face_ids() {
            controlled_group_checkpoint_v1(control)?;
            if *face != shared {
                remaining.insert(*face);
            }
        }
        let mut result = HashMap::new();
        let mut valid = true;
        for group_index in 0..count {
            controlled_group_checkpoint_v1(control)?;
            let Some(seed) = remaining.iter().next().copied() else {
                valid = false;
                break;
            };
            let mut stack = vec![seed];
            let mut faces = HashSet::new();
            while let Some(face) = stack.pop() {
                controlled_group_checkpoint_v1(control)?;
                if !remaining.remove(&face) {
                    continue;
                }
                faces.insert(face);
                for hinge in geometry.hinges() {
                    controlled_group_checkpoint_v1(control)?;
                    if hinge.left_face() == face && hinge.right_face() != shared {
                        stack.push(hinge.right_face());
                    } else if hinge.right_face() == face && hinge.left_face() != shared {
                        stack.push(hinge.left_face());
                    }
                }
            }
            let mut edges = Vec::new();
            for hinge in geometry.hinges() {
                controlled_group_checkpoint_v1(control)?;
                if faces.contains(&hinge.left_face()) || faces.contains(&hinge.right_face()) {
                    edges.push(hinge.edge());
                }
            }
            if faces.len() != 3
                || schedule
                    .bounded_symmetric_kawasaki_profile_for_edges_v1(&edges)
                    .is_none()
            {
                valid = false;
                break;
            }
            for face in faces {
                canonical_positive_cycle_checkpoint_v1(control)?;
                result.insert(face, group_index);
            }
            controlled_group_completed_checkpoint_v1(control)?;
        }
        if valid && remaining.is_empty() && result.len() == count * 3 {
            return Ok(Some(result));
        }
    }
    Ok(None)
}

/// Enumerates every canonical unordered face pair and reports how the current
/// continuous classifier treats it. This is diagnostic evidence only.
#[must_use]
pub fn diagnose_continuous_pair_coverage_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> Option<ContinuousPairCoverageRegistryV1> {
    if !schedule.matches_binding(geometry, audit, fixed_face) {
        return None;
    }
    let pair_count = checked_unordered_pair_count_v1(geometry.face_ids().len())?;
    if pair_count > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1 {
        return None;
    }
    let mut faces = geometry.face_ids().to_vec();
    faces.sort_by_key(FaceId::canonical_bytes);
    if faces.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    let groups = composed_symmetric_rational_local_groups_v1(geometry, audit, fixed_face, schedule)
        .or_else(|| rational_cactus_star_local_groups_v1(geometry, audit, fixed_face, schedule));
    let mut entries = Vec::with_capacity(pair_count);
    for first in 0..faces.len() {
        for second in first + 1..faces.len() {
            let pair = [faces[first], faces[second]];
            let shared_hinges = geometry
                .hinges()
                .iter()
                .filter(|hinge| {
                    (hinge.left_face() == pair[0] && hinge.right_face() == pair[1])
                        || (hinge.left_face() == pair[1] && hinge.right_face() == pair[0])
                })
                .count();
            let first_boundary = geometry.face_boundary_vertices(pair[0]);
            let second_boundary = geometry.face_boundary_vertices(pair[1]);
            let shared_vertex = first_boundary
                .zip(second_boundary)
                .map(|(first, second)| first.iter().any(|vertex| second.contains(vertex)));
            let membership = groups
                .as_ref()
                .map(|groups| (groups.get(&pair[0]).copied(), groups.get(&pair[1]).copied()));
            let kind = classify_continuous_pair_v1(shared_hinges, shared_vertex, membership);
            entries.push(ContinuousPairCoverageEntryV1 { pair, kind });
        }
    }
    (entries.len() == pair_count).then(|| ContinuousPairCoverageRegistryV1 {
        issuer: geometry.clone(),
        fixed_face,
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        entries,
    })
}

/// Consumes the exact gap registry and seals the bounded inputs required by a
/// future shared-hinge open-interval theorem. This remains a gap report.
#[must_use]
pub fn diagnose_shared_hinge_continuous_corridor_gaps_v1(
    registry: &ContinuousPairCoverageRegistryV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    paper_thickness_mm: f64,
) -> Option<SharedHingeContinuousCorridorGapReportV1> {
    if !registry.is_for(geometry, audit, fixed_face, schedule)
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
    {
        return None;
    }
    let source = schedule.evaluate(0.0)?;
    let target = schedule.evaluate(1.0)?;
    let source = source
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect::<HashMap<_, _>>();
    let target = target
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect::<HashMap<_, _>>();
    let expected = registry
        .entries
        .iter()
        .filter(|entry| entry.kind == ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor)
        .count();
    let mut gaps = Vec::with_capacity(expected);
    for entry in registry
        .entries
        .iter()
        .filter(|entry| entry.kind == ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor)
    {
        let hinges = geometry
            .hinges()
            .iter()
            .filter(|hinge| {
                (hinge.left_face() == entry.pair[0] && hinge.right_face() == entry.pair[1])
                    || (hinge.left_face() == entry.pair[1] && hinge.right_face() == entry.pair[0])
            })
            .collect::<Vec<_>>();
        let [hinge] = hinges.as_slice() else {
            return None;
        };
        let triangular_prerequisite = geometry
            .face_boundary_vertices(entry.pair[0])
            .is_some_and(|v| v.len() == 3)
            && geometry
                .face_boundary_vertices(entry.pair[1])
                .is_some_and(|v| v.len() == 3);
        let derivative = schedule.derivative_bound(hinge.edge())?;
        if !derivative.is_finite() || derivative < 0.0 {
            return None;
        }
        gaps.push(SharedHingeContinuousCorridorGapV1 {
            pair: entry.pair,
            hinge: hinge.edge(),
            source_angle_bits: *source.get(&hinge.edge())?,
            target_angle_bits: *target.get(&hinge.edge())?,
            derivative_bound_bits: derivative.to_bits(),
            triangular_prerequisite,
        });
    }
    (gaps.len() == expected).then(|| SharedHingeContinuousCorridorGapReportV1 {
        issuer: geometry.clone(),
        fixed_face,
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        thickness_bits: paper_thickness_mm.to_bits(),
        gaps,
    })
}

#[must_use]
pub fn diagnose_shared_vertex_continuous_corridor_gaps_v1(
    registry: &ContinuousPairCoverageRegistryV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    paper_thickness_mm: f64,
) -> Option<SharedVertexContinuousCorridorGapReportV1> {
    if !registry.is_for(geometry, audit, fixed_face, schedule)
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
    {
        return None;
    }
    let expected = registry
        .entries
        .iter()
        .filter(|entry| entry.kind == ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor)
        .count();
    if expected > MAX_CONTINUOUS_PAIR_COVERAGE_PAIRS_V1 {
        return None;
    }
    let mut gaps = Vec::new();
    gaps.try_reserve_exact(expected).ok()?;
    for entry in registry
        .entries
        .iter()
        .filter(|entry| entry.kind == ContinuousPairCoverageKindV1::SharedVertexNeedsCorridor)
    {
        if geometry.hinges().iter().any(|hinge| {
            [hinge.left_face(), hinge.right_face()] == entry.pair
                || [hinge.right_face(), hinge.left_face()] == entry.pair
        }) {
            return None;
        }
        let first = geometry.face_boundary_vertices(entry.pair[0])?;
        let second = geometry.face_boundary_vertices(entry.pair[1])?;
        let shared = first
            .iter()
            .copied()
            .filter(|vertex| second.contains(vertex))
            .collect::<Vec<_>>();
        let [vertex] = shared.as_slice() else {
            return None;
        };
        gaps.push(SharedVertexContinuousCorridorGapV1 {
            pair: entry.pair,
            vertex: *vertex,
        });
    }
    (gaps.len() == expected).then(|| SharedVertexContinuousCorridorGapReportV1 {
        issuer: geometry.clone(),
        fixed_face,
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        thickness_bits: paper_thickness_mm.to_bits(),
        gaps,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn compose_shared_hinge_relief_coverage_v1(
    registry: &ContinuousPairCoverageRegistryV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    paper_thickness_mm: f64,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    local: &NativeHingeReliefLocalIntervalCertificateV1,
    policies: &[HingeReliefPolicyRecordV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<SharedHingeReliefCoverageReportV1, SharedHingeReliefCoverageErrorV1> {
    if !registry.is_for(geometry, audit, fixed_face, schedule) {
        return Err(SharedHingeReliefCoverageErrorV1::ForeignCoverage);
    }
    let gaps = diagnose_shared_hinge_continuous_corridor_gaps_v1(
        registry,
        geometry,
        audit,
        fixed_face,
        schedule,
        paper_thickness_mm,
    )
    .ok_or(SharedHingeReliefCoverageErrorV1::ForeignCoverage)?;
    revalidate_hinge_relief_local_intervals_v1(
        local,
        prerequisite,
        geometry,
        paper_thickness_mm,
        policies,
        local_schedules,
        limits,
    )
    .map_err(|_| SharedHingeReliefCoverageErrorV1::ForeignRelief)?;
    if gaps.gaps.len() > crate::MAX_HINGE_RELIEF_RECORDS_V1 {
        return Err(SharedHingeReliefCoverageErrorV1::ResourceLimit);
    }
    let policy_edges = policies
        .iter()
        .map(|record| record.edge)
        .collect::<HashSet<_>>();
    if policy_edges.len() != policies.len() || policy_edges.len() != gaps.gaps.len() {
        return Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage);
    }
    let mut covered = match_relief_gap_schedules(&gaps.gaps, local_schedules, |edge| {
        schedule.is_exact_constant_profile_v1(edge)
    })?;
    if covered
        .iter()
        .any(|item| !policy_edges.contains(&item.hinge))
    {
        return Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage);
    }
    covered.sort_unstable_by_key(|item| {
        (
            item.pair[0].canonical_bytes(),
            item.pair[1].canonical_bytes(),
        )
    });
    if covered.windows(2).any(|pair| pair[0].pair == pair[1].pair) {
        return Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage);
    }
    let remaining = registry
        .entries
        .iter()
        .filter(|entry| entry.kind != ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor)
        .copied()
        .collect();
    Ok(SharedHingeReliefCoverageReportV1 {
        issuer: geometry.clone(),
        fixed_face,
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        thickness_bits: paper_thickness_mm.to_bits(),
        covered,
        remaining,
    })
}

fn match_relief_gap_schedules(
    gaps: &[SharedHingeContinuousCorridorGapV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    is_exact_constant: impl Fn(EdgeId) -> bool,
) -> Result<Vec<ReliefCoveredSharedHingePairV1>, SharedHingeReliefCoverageErrorV1> {
    if gaps.len() != local_schedules.len() || gaps.len() > crate::MAX_HINGE_RELIEF_RECORDS_V1 {
        return Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage);
    }
    let mut covered = Vec::new();
    covered
        .try_reserve_exact(gaps.len())
        .map_err(|_| SharedHingeReliefCoverageErrorV1::ResourceLimit)?;
    for gap in gaps {
        let matching = local_schedules
            .iter()
            .filter(|item| item.edge == gap.hinge)
            .collect::<Vec<_>>();
        let [local_schedule] = matching.as_slice() else {
            return Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage);
        };
        let derivative_bound =
            (local_schedule.target_angle_degrees - local_schedule.source_angle_degrees).abs();
        let exact_constant = derivative_bound == 0.0 && is_exact_constant(gap.hinge);
        if local_schedule.source_angle_degrees.to_bits() != gap.source_angle_bits
            || local_schedule.target_angle_degrees.to_bits() != gap.target_angle_bits
            || (!exact_constant && derivative_bound.to_bits() != gap.derivative_bound_bits)
        {
            return Err(SharedHingeReliefCoverageErrorV1::IncompleteCoverage);
        }
        covered.push(ReliefCoveredSharedHingePairV1 {
            pair: gap.pair,
            hinge: gap.hinge,
        });
    }
    Ok(covered)
}

#[cfg(test)]
fn symmetric_groups_have_disjoint_swept_balls_v1(
    geometry: &MaterialHingeGraphGeometry,
    groups: &HashMap<FaceId, usize>,
) -> bool {
    symmetric_groups_have_disjoint_swept_balls_with_control_v1(geometry, groups, None)
}

#[cfg(test)]
fn symmetric_groups_have_disjoint_swept_balls_with_control_v1(
    geometry: &MaterialHingeGraphGeometry,
    groups: &HashMap<FaceId, usize>,
    operation_control: Option<&CooperativeOperationControlV1<'_>>,
) -> bool {
    let operation_is_current =
        || operation_control.is_none_or(|control| control.checkpoint().is_ok());
    let group_count = groups.values().copied().max().map_or(0, |value| value + 1);
    let mut balls = Vec::with_capacity(group_count);
    for group in 0..group_count {
        if !operation_is_current() {
            return false;
        }
        let mut hinges = Vec::new();
        for hinge in geometry.hinges() {
            if !operation_is_current() {
                return false;
            }
            if groups.get(&hinge.left_face()) == Some(&group)
                || groups.get(&hinge.right_face()) == Some(&group)
            {
                hinges.push(hinge);
            }
        }
        if hinges.len() != 4 {
            return false;
        }
        let mut pivot = None;
        for candidate in [hinges[0].start(), hinges[0].end()] {
            if !operation_is_current() {
                return false;
            }
            if hinges.iter().all(|hinge| {
                operation_is_current() && (hinge.start() == candidate || hinge.end() == candidate)
            }) {
                pivot = Some(candidate);
                break;
            }
        }
        let Some(pivot) = pivot else {
            return false;
        };
        let mut radius = 0.0_f64;
        for face in geometry
            .face_ids()
            .iter()
            .filter(|face| groups.get(face) == Some(&group))
        {
            if !operation_is_current() {
                return false;
            }
            let Some(boundary) = geometry.face_boundary_vertices(*face) else {
                return false;
            };
            for vertex in boundary {
                if !operation_is_current() {
                    return false;
                }
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return false;
                };
                radius = radius.max(
                    ((point.x() - pivot.x()).powi(2)
                        + (point.y() - pivot.y()).powi(2)
                        + (point.z() - pivot.z()).powi(2))
                    .sqrt(),
                );
            }
        }
        balls.push((pivot, radius));
    }
    (0..balls.len()).all(|first| {
        (first + 1..balls.len()).all(|second| {
            if !operation_is_current() {
                return false;
            }
            let distance = ((balls[first].0.x() - balls[second].0.x()).powi(2)
                + (balls[first].0.y() - balls[second].0.y()).powi(2)
                + (balls[first].0.z() - balls[second].0.z()).powi(2))
            .sqrt();
            distance.is_finite()
                && balls[first].1.is_finite()
                && balls[second].1.is_finite()
                && distance > balls[first].1 + balls[second].1
        })
    })
}

// Collision-free branch of the convex 120/120/60/60 bird-foot vertex for
// 0 <= tan(rho_BC/2) <= 1.  The exact rational schedule fixes the one-DOF
// mode, the single M crease selects its non-self-intersecting branch, and all
// material faces are triangles meeting at the one physical vertex.
fn scheduled_kawasaki_120_120_60_60_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> bool {
    if geometry.hinges().len() != 4
        || geometry.face_ids().len() != 4
        || audit.closure_hinges().len() != 1
    {
        return false;
    }
    let Some((unit, half)) = schedule.kawasaki_120_120_60_60_half_angle_pairs_v1() else {
        return false;
    };
    let half = half.into_iter().collect::<HashSet<_>>();
    if unit.len() != 2
        || geometry
            .hinges()
            .iter()
            .filter(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .count()
            != 1
        || geometry
            .hinges()
            .iter()
            .find(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .is_none_or(|hinge| !half.contains(&hinge.edge()))
    {
        return false;
    }
    let first = &geometry.hinges()[0];
    let pivot = [first.start(), first.end()].into_iter().find(|candidate| {
        geometry
            .hinges()
            .iter()
            .all(|hinge| hinge.start() == *candidate || hinge.end() == *candidate)
    });
    let Some(pivot) = pivot else { return false };
    let same = |a: ori_kinematics::Point3, b: ori_kinematics::Point3| {
        a.x().to_bits() == b.x().to_bits()
            && a.y().to_bits() == b.y().to_bits()
            && a.z().to_bits() == b.z().to_bits()
    };
    if geometry.hinges().iter().any(|hinge| {
        !same(hinge.start(), pivot) && !same(hinge.end(), pivot) || same(hinge.start(), hinge.end())
    }) || geometry.face_ids().iter().any(|face| {
        geometry
            .face_boundary_vertices(*face)
            .is_none_or(|boundary| boundary.len() != 3)
    }) {
        return false;
    }
    [0.0, 0.5, 1.0].into_iter().all(|u| {
        schedule.evaluate(u).is_some_and(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, 1.0e-9)
                .is_ok()
        })
    })
}

// A convex radial fan folded on two opposite rays is a bifold about one
// infinite line. Exact profile equality keeps each half of the sheet rigid;
// the two halves can meet only on that fold line. This theorem is deliberately
// narrower than the generic swept-AABB classifier, whose boxes cannot separate
// non-adjacent triangles that share the fan pivot.
pub(crate) fn scheduled_opposite_radial_bifold_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
) -> bool {
    let hinge_count = geometry.hinges().len();
    if hinge_count < 6
        || !hinge_count.is_multiple_of(2)
        || geometry.face_ids().len() != hinge_count
        || audit.closure_hinges().len() != 1
        || closure.fixed_face() != fixed_face
        || !closure.every_leaf_covers_graph_v1(geometry)
    {
        return false;
    }
    let Some(moving) = equal_endpoint_moving_edges_v1(schedule) else {
        return false;
    };
    if moving.len() != 2 {
        return false;
    }
    let first = &geometry.hinges()[0];
    let pivot = [first.start(), first.end()].into_iter().find(|candidate| {
        geometry
            .hinges()
            .iter()
            .all(|hinge| hinge.start() == *candidate || hinge.end() == *candidate)
    });
    let Some(pivot) = pivot else { return false };
    if geometry.face_ids().iter().any(|face| {
        geometry
            .face_boundary_vertices(*face)
            .is_none_or(|boundary| {
                boundary.len() != 3
                    || !boundary
                        .iter()
                        .any(|vertex| geometry.vertex_position(*vertex) == Some(pivot))
            })
    }) {
        return false;
    }
    let outer = |edge: EdgeId| {
        let hinge = geometry
            .hinges()
            .iter()
            .find(|hinge| hinge.edge() == edge)?;
        Some(if hinge.start() == pivot {
            hinge.end()
        } else {
            hinge.start()
        })
    };
    let (Some(a), Some(b)) = (outer(moving[0]), outer(moving[1])) else {
        return false;
    };
    let av = [a.x() - pivot.x(), a.y() - pivot.y(), a.z() - pivot.z()];
    let bv = [b.x() - pivot.x(), b.y() - pivot.y(), b.z() - pivot.z()];
    let cross = [
        av[1] * bv[2] - av[2] * bv[1],
        av[2] * bv[0] - av[0] * bv[2],
        av[0] * bv[1] - av[1] * bv[0],
    ];
    if cross.iter().any(|value| *value != 0.0)
        || av.into_iter().zip(bv).map(|(a, b)| a * b).sum::<f64>() >= 0.0
    {
        return false;
    }
    let (Some(initial), Some(target)) = (schedule.evaluate(0.0), schedule.evaluate(1.0)) else {
        return false;
    };
    initial
        .as_slice()
        .iter()
        .all(|angle| angle.angle_degrees() == 0.0)
        && target
            .as_slice()
            .iter()
            .all(|angle| moving.contains(&angle.edge()) || angle.angle_degrees() == 0.0)
}

// Narrow zero-thickness theorem for folding an already flat stack. Exact
// projective-profile equality prevents sampled schedules from impersonating a
// collective motion. Constant hinges must remain bit-exact 180 degrees, all
// moving hinges start at bit-exact zero, and their initial world axes must be
// one exact infinite line. The tree composition can therefore only rotate
// each flat layer about that same line; distinct layer planes meet on the fold
// line, while equal-angle layers preserve their pre-existing flat ordering.
fn scheduled_collinear_flat_stack_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> bool {
    let Some(moving_edges) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    if moving_edges.len() < 2 {
        return false;
    }
    let moving = moving_edges.into_iter().collect::<HashSet<_>>();
    let (Some(initial_angles), Some(midpoint_angles), Some(requested_angles)) = (
        schedule.evaluate(0.0),
        schedule.evaluate(0.5),
        schedule.evaluate(1.0),
    ) else {
        return false;
    };
    let requested_moving = requested_angles
        .as_slice()
        .iter()
        .filter(|angle| moving.contains(&angle.edge()))
        .map(|angle| angle.angle_degrees().to_bits())
        .collect::<HashSet<_>>();
    let initial_moving = initial_angles
        .as_slice()
        .iter()
        .filter(|angle| moving.contains(&angle.edge()))
        .map(|angle| angle.angle_degrees().to_bits())
        .collect::<HashSet<_>>();
    if requested_moving.len() != 1
        || initial_moving.len() != 1
        || requested_moving.iter().next().is_none_or(|bits| {
            let angle = f64::from_bits(*bits);
            !angle.is_finite() || angle <= 0.0 || angle >= 180.0
        })
        || initial_angles.as_slice().iter().any(|angle| {
            !moving.contains(&angle.edge())
                && angle.angle_degrees().to_bits() != 180.0_f64.to_bits()
        })
        || requested_angles.as_slice().iter().any(|angle| {
            !moving.contains(&angle.edge())
                && angle.angle_degrees().to_bits() != 180.0_f64.to_bits()
        })
    {
        return false;
    }
    let Ok(initial_pose) = geometry.solve_closed(audit, fixed_face, &initial_angles, 1.0e-9) else {
        return false;
    };
    let mut moving_hinges = geometry
        .hinges()
        .iter()
        .filter(|hinge| moving.contains(&hinge.edge()));
    let Some(reference) = moving_hinges.next() else {
        return false;
    };
    let Some(reference_transform) = initial_pose.face_transform(reference.left_face()) else {
        return false;
    };
    let (Ok(reference_start), Ok(reference_end), Ok(reference_axis)) = (
        reference_transform.apply_point(reference.start()),
        reference_transform.apply_point(reference.end()),
        reference_transform.apply_vector(reference.axis()),
    ) else {
        return false;
    };
    if !moving_hinges.all(|hinge| {
        let Some(transform) = initial_pose.face_transform(hinge.left_face()) else {
            return false;
        };
        let (Ok(start), Ok(end), Ok(axis)) = (
            transform.apply_point(hinge.start()),
            transform.apply_point(hinge.end()),
            transform.apply_vector(hinge.axis()),
        ) else {
            return false;
        };
        bounded_collinear_line(reference_start, reference_axis, start, axis, 1.0e-9)
            && bounded_collinear_line(reference_start, reference_axis, end, axis, 1.0e-9)
            && bounded_collinear_line(reference_start, reference_axis, reference_end, axis, 1.0e-9)
    }) {
        return false;
    }
    [midpoint_angles, requested_angles]
        .into_iter()
        .all(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, 1.0e-9)
                .ok()
                .is_some_and(|pose| {
                    graph_pose_preserves_common_axis_layers(
                        geometry,
                        &initial_pose,
                        &pose,
                        reference_start,
                        reference_end,
                    )
                })
        })
}

fn bounded_collinear_line(
    origin: ori_kinematics::Point3,
    axis: ori_kinematics::Point3,
    point: ori_kinematics::Point3,
    candidate_axis: ori_kinematics::Point3,
    tolerance: f64,
) -> bool {
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let reference = [axis.x(), axis.y(), axis.z()];
    let candidate = [candidate_axis.x(), candidate_axis.y(), candidate_axis.z()];
    let offset = [
        point.x() - origin.x(),
        point.y() - origin.y(),
        point.z() - origin.z(),
    ];
    let axis_error = cross(reference, candidate);
    let offset_error = cross(offset, reference);
    let offset_scale = offset.into_iter().map(f64::abs).fold(1.0_f64, f64::max);
    axis_error.into_iter().all(|value| value.abs() <= tolerance)
        && offset_error
            .into_iter()
            .all(|value| value.abs() <= tolerance * offset_scale)
}

fn graph_pose_preserves_common_axis_layers(
    geometry: &MaterialHingeGraphGeometry,
    initial_pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
    pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
    axis_start: ori_kinematics::Point3,
    axis_end: ori_kinematics::Point3,
) -> bool {
    let tolerance = 1.0e-9
        * [
            axis_start.x().abs(),
            axis_start.y().abs(),
            axis_start.z().abs(),
            axis_end.x().abs(),
            axis_end.y().abs(),
            axis_end.z().abs(),
            1.0,
        ]
        .into_iter()
        .fold(1.0_f64, f64::max);
    let fixes = |actual: ori_kinematics::Point3, expected: ori_kinematics::Point3| {
        (actual.x() - expected.x()).abs() <= tolerance
            && (actual.y() - expected.y()).abs() <= tolerance
            && (actual.z() - expected.z()).abs() <= tolerance
    };
    let mut moved = false;
    for face in geometry.face_ids() {
        let (Some(initial_transform), Some(transform)) = (
            initial_pose.face_transform(*face),
            pose.face_transform(*face),
        ) else {
            return false;
        };
        let Ok(transform) = transform.relative_to(initial_transform) else {
            return false;
        };
        let (Ok(start), Ok(end)) = (
            transform.apply_point(axis_start),
            transform.apply_point(axis_end),
        ) else {
            return false;
        };
        if !fixes(start, axis_start) || !fixes(end, axis_end) {
            return false;
        }
        moved |= transform != ori_kinematics::RigidTransform::identity();
    }
    moved
}

fn collinear_collective_tree_premises(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    requested_angle_degrees: f64,
) -> bool {
    if model.face_ids().len() < 3
        || model.hinges().len() < 2
        || moving.len() != model.hinges().len()
        || initial_pose.fixed_face().is_none()
        || !initial_pose.hinge_angles().iter().all(|angle| {
            moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 0.0_f64.to_bits()
        })
    {
        return false;
    }
    let Some(reference) = model.hinges().first() else {
        return false;
    };
    let Some(reference_line) = world_hinge_line(initial_pose, reference) else {
        return false;
    };
    if !model.hinges().iter().all(|hinge| {
        let Some((start, end, axis)) = world_hinge_line(initial_pose, hinge) else {
            return false;
        };
        exact_collinear_line(reference_line.0, reference_line.2, start, axis)
            && exact_collinear_line(reference_line.0, reference_line.2, end, axis)
    }) {
        return false;
    }
    [requested_angle_degrees / 2.0, requested_angle_degrees]
        .into_iter()
        .all(|angle| collective_pose_is_one_moving_body(model, initial_pose, moving, angle))
}

fn world_hinge_line(
    pose: &MaterialTreePose,
    hinge: &ori_kinematics::TreeHinge,
) -> Option<(
    ori_kinematics::Point3,
    ori_kinematics::Point3,
    ori_kinematics::Point3,
)> {
    let transform = pose.hinge_parent_transform(hinge.edge())?;
    Some((
        transform.apply_point(hinge.start()).ok()?,
        transform.apply_point(hinge.end()).ok()?,
        transform.apply_vector(hinge.axis()).ok()?,
    ))
}

fn exact_collinear_line(
    origin: ori_kinematics::Point3,
    axis: ori_kinematics::Point3,
    point: ori_kinematics::Point3,
    candidate_axis: ori_kinematics::Point3,
) -> bool {
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let reference = [axis.x(), axis.y(), axis.z()];
    let candidate = [candidate_axis.x(), candidate_axis.y(), candidate_axis.z()];
    let offset = [
        point.x() - origin.x(),
        point.y() - origin.y(),
        point.z() - origin.z(),
    ];
    cross(reference, candidate)
        .into_iter()
        .chain(cross(offset, reference))
        .all(|value| value == 0.0)
}

fn collective_pose_is_one_moving_body(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    angle: f64,
) -> bool {
    let Ok(angles) = initial_pose
        .hinge_angles()
        .iter()
        .map(|hinge| {
            HingeAngle::new(
                hinge.edge(),
                if moving.contains(&hinge.edge()) {
                    angle
                } else {
                    hinge.angle_degrees()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(CanonicalHingeAngles::new)
    else {
        return false;
    };
    let Ok(pose) = model.solve(initial_pose.fixed_face(), &angles) else {
        return false;
    };
    let Some(fixed_face) = initial_pose.fixed_face() else {
        return false;
    };
    let Some(fixed_transform) = pose.face_transform(fixed_face) else {
        return false;
    };
    let mut moving_transform = None;
    for face in model
        .face_ids()
        .iter()
        .copied()
        .filter(|face| *face != fixed_face)
    {
        let Some(transform) = pose.face_transform(face) else {
            return false;
        };
        if transform == fixed_transform {
            return false;
        }
        if let Some(expected) = moving_transform {
            if transform != expected {
                return false;
            }
        } else {
            moving_transform = Some(transform);
        }
    }
    moving_transform.is_some()
}

#[cfg(test)]
#[path = "../../../test-support/dense_grid_cycle.rs"]
mod dense_grid_cycle_test_support;
#[cfg(test)]
#[path = "../../../test-support/four_bay_cycle.rs"]
mod four_bay_cycle_test_support;
#[cfg(test)]
#[path = "../../../test-support/miura_cactus.rs"]
mod miura_cactus_test_support;

#[cfg(test)]
#[rustfmt::skip]
#[path = "continuous_path/tests.rs"]
mod tests;
