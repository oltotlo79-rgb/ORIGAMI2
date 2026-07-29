//! Sound cross-block clearance prerequisite for one common articulation path.
//!
//! A set of independently certified blocks does not prove separation between
//! different blocks.  This module therefore promotes only an existing
//! positive-thickness continuous certificate issued for the complete parent
//! geometry.  Endpoint observations, samples, broad-phase AABBs, and
//! per-block certificates are intentionally not accepted as substitutes.

use std::mem::size_of;

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV1,
    ClosedMaterialHingeGraphPose, CommonArticulationPoseAuthorityV1, CommonArticulationPoseErrorV1,
    CommonArticulationPoseInputV1, CommonArticulationPoseLimitsV1, CommonArticulationPoseStopV1,
    CycleScheduleLimitsV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1,
    PositiveThicknessContinuousCertificateV1,
};

pub const COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V1: &str =
    "common_articulation_cross_block_clearance_prerequisite_v1";
pub const COMMON_ARTICULATION_CLEARANCE_GAP_MODEL_ID_V1: &str =
    "common_articulation_cross_block_clearance_gap_v1";
pub const COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1: usize = 8;
pub const COMMON_ARTICULATION_CLEARANCE_MAX_FACES_V1: usize = 256;
pub const COMMON_ARTICULATION_CLEARANCE_MAX_CROSS_BLOCK_PAIRS_V1: usize =
    COMMON_ARTICULATION_CLEARANCE_MAX_FACES_V1 * (COMMON_ARTICULATION_CLEARANCE_MAX_FACES_V1 - 1)
        / 2;
pub const COMMON_ARTICULATION_CLEARANCE_MAX_PAIR_CANDIDATES_V1: usize = 65_536;
pub const COMMON_ARTICULATION_CLEARANCE_MAX_WORK_V1: usize = 1_000_000;
pub const COMMON_ARTICULATION_CLEARANCE_MAX_STORAGE_BYTES_V1: usize = 4 * 1024 * 1024;

const CLEARANCE_BASE_STORAGE_BYTES_V1: usize = 1_024;
const CLEARANCE_FACE_POSE_STORAGE_BYTES_V1: usize = 128;
const CLEARANCE_HINGE_ANGLE_STORAGE_BYTES_V1: usize = 32;
const CLEARANCE_BASE_WORK_V1: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationClearanceLimitsV1 {
    pub max_blocks: usize,
    pub max_faces: usize,
    pub max_cross_block_pairs: usize,
    pub max_pair_candidates: usize,
    pub max_work: usize,
    pub max_storage_bytes: usize,
}

impl Default for CommonArticulationClearanceLimitsV1 {
    fn default() -> Self {
        Self {
            max_blocks: COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1,
            max_faces: COMMON_ARTICULATION_CLEARANCE_MAX_FACES_V1,
            max_cross_block_pairs: COMMON_ARTICULATION_CLEARANCE_MAX_CROSS_BLOCK_PAIRS_V1,
            max_pair_candidates: COMMON_ARTICULATION_CLEARANCE_MAX_PAIR_CANDIDATES_V1,
            max_work: COMMON_ARTICULATION_CLEARANCE_MAX_WORK_V1,
            max_storage_bytes: COMMON_ARTICULATION_CLEARANCE_MAX_STORAGE_BYTES_V1,
        }
    }
}

/// One canonical unordered pair belonging to two distinct canonical blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommonArticulationCrossBlockFacePairV1 {
    first: FaceId,
    second: FaceId,
}

impl CommonArticulationCrossBlockFacePairV1 {
    #[must_use]
    pub fn new(first: FaceId, second: FaceId) -> Option<Self> {
        if first == second {
            return None;
        }
        if first.canonical_bytes() < second.canonical_bytes() {
            Some(Self { first, second })
        } else {
            Some(Self {
                first: second,
                second: first,
            })
        }
    }

    #[must_use]
    pub const fn first(self) -> FaceId {
        self.first
    }

    #[must_use]
    pub const fn second(self) -> FaceId {
        self.second
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationClearanceUnsupportedReasonV1 {
    /// No open-interval positive-thickness proof for the complete parent was
    /// supplied. Per-block proofs cannot fill this gap.
    WholeParentOpenIntervalProofUnavailable,
    /// The schedule cannot be evaluated at the canonical source parameter, so
    /// the common pose cannot soundly be identified with the path source.
    CanonicalSourcePoseUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationClearanceErrorV1 {
    #[error("the common-articulation clearance input is malformed")]
    InvalidInput,
    #[error("the common-articulation clearance proof exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the supplied common-pose authority does not revalidate for the exact live inputs")]
    CommonPose(CommonArticulationPoseErrorV1),
    #[error("the parent schedule or interval closure is not bound to the exact live graph")]
    PathBindingMismatch,
    #[error("the canonical path source differs from the common articulation pose")]
    PathSourcePoseMismatch,
    #[error("the submitted cross-block pair registry contains a duplicate")]
    DuplicateCrossBlockPair,
    #[error(
        "the submitted cross-block pair registry is incomplete or contains an extra pair \
         (expected {expected}, actual {actual})"
    )]
    CrossBlockPairCoverageMismatch { expected: usize, actual: usize },
    #[error("the whole-parent continuous certificate does not match the exact path")]
    WholeParentContinuousProofMismatch,
    #[error("the common-articulation clearance operation was cancelled")]
    Cancelled,
    #[error("the common-articulation clearance operation deadline elapsed")]
    DeadlineExceeded,
}

pub struct CommonArticulationClearanceInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub common_pose: &'a CommonArticulationPoseAuthorityV1,
    pub common_pose_limits: CommonArticulationPoseLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV1],
    /// Only a certificate issued for `geometry` itself is accepted. A caller
    /// cannot submit a list of per-block certificates through this boundary.
    ///
    /// This certificate is moved into a successful prerequisite so it remains
    /// available for exact revalidation. A caller that also needs to retain
    /// the positive certificate must explicitly clone it before issuance.
    pub whole_parent_continuous: Option<PositiveThicknessContinuousCertificateV1>,
    pub limits: CommonArticulationClearanceLimitsV1,
}

/// Live inputs required to revalidate an issued clearance prerequisite.
#[derive(Debug, Clone, Copy)]
pub struct CommonArticulationClearanceRevalidationInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub common_pose: &'a CommonArticulationPoseAuthorityV1,
    pub common_pose_limits: CommonArticulationPoseLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub limits: CommonArticulationClearanceLimitsV1,
}

/// Sealed proof of the cross-block open-interval prerequisite.
///
/// The proof is deliberately neither cloneable nor serializable and grants no
/// Apply, viewer, mutation, or general motion authority.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationClearancePrerequisiteV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationClearancePrerequisiteV1>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationClearancePrerequisiteV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationClearancePrerequisiteV1>();
/// ```
#[derive(Debug)]
pub struct CommonArticulationClearancePrerequisiteV1 {
    issuer_pose: ClosedMaterialHingeGraphPose,
    whole_parent_continuous: PositiveThicknessContinuousCertificateV1,
    common_pose_binding: [u8; 32],
    schedule_binding: [u8; 32],
    closure_binding: [u8; 32],
    paper_thickness_bits: u64,
    common_pose_limits: CommonArticulationPoseLimitsV1,
    schedule_limits: CycleScheduleLimitsV1,
    limits: CommonArticulationClearanceLimitsV1,
    cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV1>,
    logical_work: usize,
    storage_bytes_upper_bound: usize,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationClearancePrerequisiteV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V1
    }

    #[must_use]
    pub fn is_for_pose_v1(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        pose.is_for_geometry(geometry) && self.issuer_pose.same_instance(pose)
    }

    #[must_use]
    pub fn cross_block_pairs_v1(&self) -> &[CommonArticulationCrossBlockFacePairV1] {
        &self.cross_block_pairs
    }

    #[must_use]
    pub const fn paper_thickness_mm_v1(&self) -> f64 {
        f64::from_bits(self.paper_thickness_bits)
    }

    #[must_use]
    pub const fn logical_work_v1(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn storage_bytes_upper_bound_v1(&self) -> usize {
        self.storage_bytes_upper_bound
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn common_pose_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.common_pose_binding
    }

    #[must_use]
    pub const fn schedule_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.schedule_binding
    }

    #[must_use]
    pub const fn closure_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.closure_binding
    }

    /// Revalidates the retained whole-parent proof and every exact live
    /// geometry, pose, decomposition, schedule, closure, thickness, limit, and
    /// canonical cross-block-pair binding.
    pub fn revalidate_v1(
        &self,
        input: CommonArticulationClearanceRevalidationInputV1<'_>,
    ) -> Result<(), CommonArticulationClearanceErrorV1> {
        self.revalidate_with_control_v1(input, &CooperativeOperationControlV1::unbounded())
    }

    pub fn revalidate_with_control_v1(
        &self,
        input: CommonArticulationClearanceRevalidationInputV1<'_>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), CommonArticulationClearanceErrorV1> {
        let mut checkpoint = || clearance_checkpoint_v1(control);
        checkpoint()?;
        let validation_input = CommonArticulationClearanceInputV1 {
            geometry: input.geometry,
            audit: input.audit,
            pose: input.pose,
            decomposition: input.decomposition,
            common_pose: input.common_pose,
            common_pose_limits: input.common_pose_limits,
            schedule: input.schedule,
            schedule_limits: input.schedule_limits,
            closure: input.closure,
            paper_thickness_mm: input.paper_thickness_mm,
            submitted_cross_block_pairs: &self.cross_block_pairs,
            whole_parent_continuous: Some(self.whole_parent_continuous.clone()),
            limits: input.limits,
        };
        let validated = validate_clearance_input_v1(&validation_input, &mut checkpoint)?;
        checkpoint()?;
        let common_pose_binding = input.common_pose.binding_fingerprint_v1();
        let schedule_binding = input.schedule.certificate_binding_fingerprint_v2();
        let closure_binding = input.closure.partition_binding_fingerprint_v2();
        let paper_thickness_bits = input.paper_thickness_mm.to_bits();
        let binding_fingerprint = clearance_binding_fingerprint_v1(&ClearanceBindingMaterialV1 {
            common_pose_binding,
            schedule_binding,
            closure_binding,
            paper_thickness_bits,
            common_pose_limits: input.common_pose_limits,
            schedule_limits: input.schedule_limits,
            limits: input.limits,
            pairs: &validated.cross_block_pairs,
        });
        if validated.unsupported_reason.is_some()
            || !self.issuer_pose.same_instance(input.pose)
            || !input.pose.is_for_geometry(input.geometry)
            || self.common_pose_binding != common_pose_binding
            || self.schedule_binding != schedule_binding
            || self.closure_binding != closure_binding
            || self.paper_thickness_bits != paper_thickness_bits
            || self.common_pose_limits != input.common_pose_limits
            || self.schedule_limits != input.schedule_limits
            || self.limits != input.limits
            || self.cross_block_pairs != validated.cross_block_pairs
            || self.logical_work != validated.logical_work
            || self.storage_bytes_upper_bound != validated.storage_bytes_upper_bound
            || self.binding_fingerprint != binding_fingerprint
            || !self.whole_parent_continuous.is_for(
                input.geometry,
                input.audit,
                input.pose.fixed_face(),
                input.schedule,
                input.closure,
                input.paper_thickness_mm,
            )
        {
            return Err(CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch);
        }
        checkpoint()
    }

    #[must_use]
    pub const fn cross_block_open_interval_clearance_proven_v1(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

/// Read-only explanation of the exact missing theorem boundary.
///
/// This diagnostic is intentionally neither Clone nor Serialize and never
/// carries positive authority.
#[derive(Debug)]
pub struct CommonArticulationClearanceGapDiagnosticV1 {
    reason: CommonArticulationClearanceUnsupportedReasonV1,
    common_pose_binding: [u8; 32],
    schedule_binding: [u8; 32],
    closure_binding: [u8; 32],
    paper_thickness_bits: u64,
    cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV1>,
    logical_work: usize,
    storage_bytes_upper_bound: usize,
}

impl CommonArticulationClearanceGapDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_CLEARANCE_GAP_MODEL_ID_V1
    }

    #[must_use]
    pub const fn reason(&self) -> CommonArticulationClearanceUnsupportedReasonV1 {
        self.reason
    }

    #[must_use]
    pub fn cross_block_pairs_v1(&self) -> &[CommonArticulationCrossBlockFacePairV1] {
        &self.cross_block_pairs
    }

    #[must_use]
    pub const fn logical_work_v1(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn storage_bytes_upper_bound_v1(&self) -> usize {
        self.storage_bytes_upper_bound
    }

    #[must_use]
    pub const fn paper_thickness_mm_v1(&self) -> f64 {
        f64::from_bits(self.paper_thickness_bits)
    }

    #[must_use]
    pub const fn common_pose_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.common_pose_binding
    }

    #[must_use]
    pub const fn schedule_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.schedule_binding
    }

    #[must_use]
    pub const fn closure_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.closure_binding
    }

    #[must_use]
    pub const fn endpoint_observations_are_authority_v1(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn sampled_poses_are_authority_v1(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn broad_phase_aabbs_are_authority_v1(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn per_block_certificates_are_cross_block_authority_v1(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum CommonArticulationClearanceOutcomeV1 {
    Certified(Box<CommonArticulationClearancePrerequisiteV1>),
    Unsupported(CommonArticulationClearanceGapDiagnosticV1),
}

impl CommonArticulationClearanceOutcomeV1 {
    #[must_use]
    pub const fn is_certified(&self) -> bool {
        matches!(self, Self::Certified(_))
    }

    #[must_use]
    pub fn as_certified(&self) -> Option<&CommonArticulationClearancePrerequisiteV1> {
        match self {
            Self::Certified(authority) => Some(authority.as_ref()),
            Self::Unsupported(_) => None,
        }
    }

    #[must_use]
    pub const fn as_gap(&self) -> Option<&CommonArticulationClearanceGapDiagnosticV1> {
        match self {
            Self::Certified(_) => None,
            Self::Unsupported(gap) => Some(gap),
        }
    }
}

struct ValidatedClearanceInputV1 {
    cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV1>,
    logical_work: usize,
    storage_bytes_upper_bound: usize,
    unsupported_reason: Option<CommonArticulationClearanceUnsupportedReasonV1>,
}

#[derive(Debug, Clone, Copy)]
struct ResourceEnvelopeV1 {
    raw_pair_candidates: usize,
    logical_work: usize,
    storage_bytes_upper_bound: usize,
}

pub fn issue_common_articulation_clearance_prerequisite_v1(
    input: CommonArticulationClearanceInputV1<'_>,
) -> Result<CommonArticulationClearanceOutcomeV1, CommonArticulationClearanceErrorV1> {
    issue_common_articulation_clearance_prerequisite_with_control_v1(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_common_articulation_clearance_prerequisite_with_control_v1(
    input: CommonArticulationClearanceInputV1<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<CommonArticulationClearanceOutcomeV1, CommonArticulationClearanceErrorV1> {
    issue_common_articulation_clearance_prerequisite_with_checkpoint_v1(input, &mut || {
        clearance_checkpoint_v1(control)
    })
}

fn issue_common_articulation_clearance_prerequisite_with_checkpoint_v1(
    mut input: CommonArticulationClearanceInputV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceErrorV1>,
) -> Result<CommonArticulationClearanceOutcomeV1, CommonArticulationClearanceErrorV1> {
    checkpoint()?;
    let validated = validate_clearance_input_v1(&input, checkpoint)?;
    checkpoint()?;

    let common_pose_binding = input.common_pose.binding_fingerprint_v1();
    let schedule_binding = input.schedule.certificate_binding_fingerprint_v2();
    let closure_binding = input.closure.partition_binding_fingerprint_v2();
    let paper_thickness_bits = input.paper_thickness_mm.to_bits();
    if let Some(reason) = validated.unsupported_reason {
        checkpoint()?;
        return Ok(CommonArticulationClearanceOutcomeV1::Unsupported(
            CommonArticulationClearanceGapDiagnosticV1 {
                reason,
                common_pose_binding,
                schedule_binding,
                closure_binding,
                paper_thickness_bits,
                cross_block_pairs: validated.cross_block_pairs,
                logical_work: validated.logical_work,
                storage_bytes_upper_bound: validated.storage_bytes_upper_bound,
            },
        ));
    }

    let binding_fingerprint = clearance_binding_fingerprint_v1(&ClearanceBindingMaterialV1 {
        common_pose_binding,
        schedule_binding,
        closure_binding,
        paper_thickness_bits,
        common_pose_limits: input.common_pose_limits,
        schedule_limits: input.schedule_limits,
        limits: input.limits,
        pairs: &validated.cross_block_pairs,
    });
    let whole_parent_continuous = input
        .whole_parent_continuous
        .take()
        .ok_or(CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch)?;
    checkpoint()?;
    Ok(CommonArticulationClearanceOutcomeV1::Certified(Box::new(
        CommonArticulationClearancePrerequisiteV1 {
            issuer_pose: input.pose.clone(),
            whole_parent_continuous,
            common_pose_binding,
            schedule_binding,
            closure_binding,
            paper_thickness_bits,
            common_pose_limits: input.common_pose_limits,
            schedule_limits: input.schedule_limits,
            limits: input.limits,
            cross_block_pairs: validated.cross_block_pairs,
            logical_work: validated.logical_work,
            storage_bytes_upper_bound: validated.storage_bytes_upper_bound,
            binding_fingerprint,
        },
    )))
}

fn validate_clearance_input_v1(
    input: &CommonArticulationClearanceInputV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceErrorV1>,
) -> Result<ValidatedClearanceInputV1, CommonArticulationClearanceErrorV1> {
    validate_limits_v1(input.limits)?;
    if !input.paper_thickness_mm.is_finite() || input.paper_thickness_mm <= 0.0 {
        return Err(CommonArticulationClearanceErrorV1::InvalidInput);
    }
    let block_count = input.decomposition.blocks().len();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    if !(2..=COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1).contains(&block_count)
        || block_count > input.limits.max_blocks
        || face_count == 0
        || face_count > input.limits.max_faces
        || input.submitted_cross_block_pairs.len() > input.limits.max_cross_block_pairs
    {
        return Err(CommonArticulationClearanceErrorV1::ResourceLimit);
    }
    if !input.pose.is_for_geometry(input.geometry)
        || !input.decomposition.is_for_geometry(input.geometry)
    {
        return Err(CommonArticulationClearanceErrorV1::InvalidInput);
    }

    let envelope = resource_envelope_v1(
        input.decomposition,
        face_count,
        hinge_count,
        input.submitted_cross_block_pairs.len(),
        input.common_pose.logical_work_v1(),
    )?;
    if envelope.raw_pair_candidates > input.limits.max_pair_candidates
        || envelope.logical_work > input.limits.max_work
        || envelope.storage_bytes_upper_bound > input.limits.max_storage_bytes
    {
        return Err(CommonArticulationClearanceErrorV1::ResourceLimit);
    }

    checkpoint()?;
    revalidate_common_pose_with_clearance_checkpoint_v1(
        input.common_pose,
        CommonArticulationPoseInputV1 {
            geometry: input.geometry,
            pose: input.pose,
            decomposition: input.decomposition,
            paper_thickness_mm: input.paper_thickness_mm,
            limits: input.common_pose_limits,
        },
        checkpoint,
    )?;
    checkpoint()?;

    if !input
        .schedule
        .matches_binding(input.geometry, input.audit, input.pose.fixed_face())
        || input.closure.fixed_face() != input.pose.fixed_face()
        || input.closure.schedule_binding_fingerprint_v2()
            != input.schedule.certificate_binding_fingerprint_v2()
        || input.closure.graph_binding_fingerprint_v1()
            != input.schedule.graph_binding_fingerprint_v1()
        || !input.closure.every_leaf_covers_graph_v1(input.geometry)
    {
        return Err(CommonArticulationClearanceErrorV1::PathBindingMismatch);
    }
    checkpoint()?;

    // Successful lower-endpoint enclosure is also the representation check
    // that restricts this V1 bridge to the canonical [0, 1] half-angle path.
    // Merely evaluating an arbitrary ordinary schedule at zero could select an
    // interior sample and is therefore insufficient source-pose provenance.
    let canonical_source_endpoint = input
        .schedule
        .evaluate_endpoint_angle_box(false, input.schedule_limits)
        .ok();
    let source_angles = canonical_source_endpoint
        .as_ref()
        .and_then(|_| input.schedule.evaluate(0.0));
    if source_angles.as_ref().is_some_and(|source| {
        !exact_hinge_angle_bits_match_v1(source.as_slice(), input.pose.hinge_angles().as_slice())
    }) {
        return Err(CommonArticulationClearanceErrorV1::PathSourcePoseMismatch);
    }

    let expected = enumerate_cross_block_pairs_v1(
        input.decomposition,
        envelope.raw_pair_candidates,
        checkpoint,
    )?;
    if expected.is_empty() || expected.len() > input.limits.max_cross_block_pairs {
        return Err(CommonArticulationClearanceErrorV1::ResourceLimit);
    }
    validate_submitted_pairs_v1(input.submitted_cross_block_pairs, &expected, checkpoint)?;
    checkpoint()?;

    let unsupported_reason = if source_angles.is_none() {
        Some(CommonArticulationClearanceUnsupportedReasonV1::CanonicalSourcePoseUnavailable)
    } else if let Some(continuous) = input.whole_parent_continuous.as_ref() {
        if !continuous.is_for(
            input.geometry,
            input.audit,
            input.pose.fixed_face(),
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
        ) {
            return Err(CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch);
        }
        None
    } else {
        Some(
            CommonArticulationClearanceUnsupportedReasonV1::WholeParentOpenIntervalProofUnavailable,
        )
    };
    checkpoint()?;

    Ok(ValidatedClearanceInputV1 {
        cross_block_pairs: expected,
        logical_work: envelope.logical_work,
        storage_bytes_upper_bound: envelope.storage_bytes_upper_bound,
        unsupported_reason,
    })
}

fn validate_limits_v1(
    limits: CommonArticulationClearanceLimitsV1,
) -> Result<(), CommonArticulationClearanceErrorV1> {
    if limits.max_blocks > COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1
        || limits.max_faces > COMMON_ARTICULATION_CLEARANCE_MAX_FACES_V1
        || limits.max_cross_block_pairs > COMMON_ARTICULATION_CLEARANCE_MAX_CROSS_BLOCK_PAIRS_V1
        || limits.max_pair_candidates > COMMON_ARTICULATION_CLEARANCE_MAX_PAIR_CANDIDATES_V1
        || limits.max_work > COMMON_ARTICULATION_CLEARANCE_MAX_WORK_V1
        || limits.max_storage_bytes > COMMON_ARTICULATION_CLEARANCE_MAX_STORAGE_BYTES_V1
    {
        return Err(CommonArticulationClearanceErrorV1::ResourceLimit);
    }
    Ok(())
}

fn resource_envelope_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    face_count: usize,
    hinge_count: usize,
    submitted_pair_count: usize,
    common_pose_work: usize,
) -> Result<ResourceEnvelopeV1, CommonArticulationClearanceErrorV1> {
    let blocks = decomposition.blocks();
    let mut raw_pair_candidates = 0usize;
    for first in 0..blocks.len() {
        for second in first + 1..blocks.len() {
            raw_pair_candidates = raw_pair_candidates
                .checked_add(
                    blocks[first]
                        .geometry()
                        .face_ids()
                        .len()
                        .checked_mul(blocks[second].geometry().face_ids().len())
                        .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)?,
                )
                .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)?;
        }
    }
    let sort_work = sort_work_upper_bound_v1(raw_pair_candidates)?
        .checked_add(sort_work_upper_bound_v1(submitted_pair_count)?)
        .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)?;
    let logical_work = CLEARANCE_BASE_WORK_V1
        .checked_add(common_pose_work)
        .and_then(|value| value.checked_add(face_count))
        .and_then(|value| value.checked_add(hinge_count))
        .and_then(|value| value.checked_add(blocks.len()))
        .and_then(|value| value.checked_add(raw_pair_candidates.checked_mul(3)?))
        .and_then(|value| value.checked_add(submitted_pair_count.checked_mul(2)?))
        .and_then(|value| value.checked_add(sort_work))
        .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)?;
    let pair_bytes = raw_pair_candidates
        .checked_add(submitted_pair_count)
        .and_then(|count| count.checked_mul(size_of::<CommonArticulationCrossBlockFacePairV1>()))
        .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)?;
    let storage_bytes_upper_bound = CLEARANCE_BASE_STORAGE_BYTES_V1
        .checked_add(pair_bytes)
        .and_then(|value| {
            value.checked_add(face_count.checked_mul(CLEARANCE_FACE_POSE_STORAGE_BYTES_V1)?)
        })
        .and_then(|value| {
            value.checked_add(hinge_count.checked_mul(CLEARANCE_HINGE_ANGLE_STORAGE_BYTES_V1)?)
        })
        .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)?;
    Ok(ResourceEnvelopeV1 {
        raw_pair_candidates,
        logical_work,
        storage_bytes_upper_bound,
    })
}

fn sort_work_upper_bound_v1(count: usize) -> Result<usize, CommonArticulationClearanceErrorV1> {
    let comparisons_per_item = usize::BITS as usize - count.max(1).leading_zeros() as usize;
    count
        .checked_mul(comparisons_per_item)
        .ok_or(CommonArticulationClearanceErrorV1::ResourceLimit)
}

fn enumerate_cross_block_pairs_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    raw_pair_candidates: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceErrorV1>,
) -> Result<Vec<CommonArticulationCrossBlockFacePairV1>, CommonArticulationClearanceErrorV1> {
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(raw_pair_candidates)
        .map_err(|_| CommonArticulationClearanceErrorV1::ResourceLimit)?;
    let blocks = decomposition.blocks();
    for first in 0..blocks.len() {
        checkpoint()?;
        for second in first + 1..blocks.len() {
            for first_face in blocks[first].geometry().face_ids().iter().copied() {
                for second_face in blocks[second].geometry().face_ids().iter().copied() {
                    checkpoint()?;
                    if let Some(pair) =
                        CommonArticulationCrossBlockFacePairV1::new(first_face, second_face)
                    {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    pairs.sort_unstable_by(compare_pair_v1);
    pairs.dedup();
    Ok(pairs)
}

fn validate_submitted_pairs_v1(
    submitted: &[CommonArticulationCrossBlockFacePairV1],
    expected: &[CommonArticulationCrossBlockFacePairV1],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceErrorV1>,
) -> Result<(), CommonArticulationClearanceErrorV1> {
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(submitted.len())
        .map_err(|_| CommonArticulationClearanceErrorV1::ResourceLimit)?;
    for pair in submitted {
        checkpoint()?;
        canonical.push(*pair);
    }
    canonical.sort_unstable_by(compare_pair_v1);
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CommonArticulationClearanceErrorV1::DuplicateCrossBlockPair);
    }
    if canonical != expected {
        return Err(
            CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch {
                expected: expected.len(),
                actual: canonical.len(),
            },
        );
    }
    Ok(())
}

fn compare_pair_v1(
    left: &CommonArticulationCrossBlockFacePairV1,
    right: &CommonArticulationCrossBlockFacePairV1,
) -> std::cmp::Ordering {
    left.first
        .canonical_bytes()
        .cmp(&right.first.canonical_bytes())
        .then_with(|| {
            left.second
                .canonical_bytes()
                .cmp(&right.second.canonical_bytes())
        })
}

fn exact_hinge_angle_bits_match_v1(
    first: &[ori_kinematics::HingeAngle],
    second: &[ori_kinematics::HingeAngle],
) -> bool {
    first.len() == second.len()
        && first.iter().zip(second).all(|(first, second)| {
            first.edge() == second.edge()
                && first.angle_degrees().to_bits() == second.angle_degrees().to_bits()
        })
}

fn revalidate_common_pose_with_clearance_checkpoint_v1(
    authority: &CommonArticulationPoseAuthorityV1,
    input: CommonArticulationPoseInputV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceErrorV1>,
) -> Result<(), CommonArticulationClearanceErrorV1> {
    let mut unexpected_checkpoint_error = None;
    let result = authority.revalidate_with_checkpoint_v1(input, || match checkpoint() {
        Ok(()) => Ok(()),
        Err(CommonArticulationClearanceErrorV1::Cancelled) => {
            Err(CommonArticulationPoseStopV1::Cancelled)
        }
        Err(CommonArticulationClearanceErrorV1::DeadlineExceeded) => {
            Err(CommonArticulationPoseStopV1::DeadlineExceeded)
        }
        Err(error) => {
            unexpected_checkpoint_error = Some(error);
            Err(CommonArticulationPoseStopV1::Cancelled)
        }
    });
    if let Some(error) = unexpected_checkpoint_error {
        return Err(error);
    }
    result.map_err(|error| match error {
        CommonArticulationPoseErrorV1::Cancelled => CommonArticulationClearanceErrorV1::Cancelled,
        CommonArticulationPoseErrorV1::DeadlineExceeded => {
            CommonArticulationClearanceErrorV1::DeadlineExceeded
        }
        error => CommonArticulationClearanceErrorV1::CommonPose(error),
    })
}

struct ClearanceBindingMaterialV1<'a> {
    common_pose_binding: [u8; 32],
    schedule_binding: [u8; 32],
    closure_binding: [u8; 32],
    paper_thickness_bits: u64,
    common_pose_limits: CommonArticulationPoseLimitsV1,
    schedule_limits: CycleScheduleLimitsV1,
    limits: CommonArticulationClearanceLimitsV1,
    pairs: &'a [CommonArticulationCrossBlockFacePairV1],
}

fn clearance_binding_fingerprint_v1(material: &ClearanceBindingMaterialV1<'_>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V1.as_bytes());
    hash.update(material.common_pose_binding);
    hash.update(material.schedule_binding);
    hash.update(material.closure_binding);
    hash.update(material.paper_thickness_bits.to_be_bytes());
    for value in [
        material.common_pose_limits.max_blocks,
        material.common_pose_limits.max_faces,
        material.common_pose_limits.max_hinges,
        material.common_pose_limits.max_work,
        material.common_pose_limits.max_retained_bytes,
        material.schedule_limits.max_hinges,
        material.schedule_limits.max_degree,
        material.schedule_limits.max_work,
        material.limits.max_blocks,
        material.limits.max_faces,
        material.limits.max_cross_block_pairs,
        material.limits.max_pair_candidates,
        material.limits.max_work,
        material.limits.max_storage_bytes,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
    hash.update(material.schedule_limits.max_coefficient_bits.to_be_bytes());
    hash.update((material.pairs.len() as u64).to_be_bytes());
    for pair in material.pairs {
        hash.update(pair.first.canonical_bytes());
        hash.update(pair.second.canonical_bytes());
    }
    hash.finalize().into()
}

fn clearance_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationClearanceErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => CommonArticulationClearanceErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationClearanceErrorV1::DeadlineExceeded
        }
    })
}

#[cfg(test)]
#[path = "common_articulation_clearance/tests.rs"]
mod tests;
