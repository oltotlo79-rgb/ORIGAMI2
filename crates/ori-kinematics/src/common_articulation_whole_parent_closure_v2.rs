//! Whole-parent interval-closure evidence for the general-N articulation path.
//!
//! This module turns the V1 dyadic closure algorithm into a *private
//! observation* owned by a separately typed V2 boundary.  It proves that the
//! full parent schedule closes over a complete dyadic partition, but it does
//! not prove positive-thickness collision clearance, layer transport, motion,
//! project mutation, Apply, or viewer publication.

use std::fmt;

use ori_domain::FaceId;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationBlockClosureSetErrorV2,
    CommonArticulationBlockClosureSetInputV2, CommonArticulationBlockClosureSetLimitsV2,
    CommonArticulationBlockClosureSetStopV2, CommonArticulationBlockClosureSetV2,
    CommonArticulationPoseAuthorityV2, CommonArticulationResourceProfileV2,
    DyadicIntervalClosureControlErrorV1, DyadicIntervalClosureErrorV1,
    DyadicIntervalClosureLimitsV1, DyadicIntervalClosureStopV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, KinematicsError, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
};

/// Stable model identifier for general-N whole-parent closure evidence.
pub const COMMON_ARTICULATION_WHOLE_PARENT_CLOSURE_MODEL_ID_V2: &str =
    "common_articulation_whole_parent_closure_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;

/// Cooperative stop requested by whole-parent closure issuance or revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationWholeParentClosureStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Failure while creating or revalidating whole-parent closure evidence.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationWholeParentClosureErrorV2 {
    #[error("the general-N whole-parent closure input is malformed or foreign")]
    InvalidInput,
    #[error("the general-N whole-parent closure exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the all-block closure evidence failed live revalidation: {0}")]
    BlockClosureSet(CommonArticulationBlockClosureSetErrorV2),
    #[error("the parent dyadic closure could not be proven: {0:?}")]
    ParentClosure(DyadicIntervalClosureErrorV1),
    #[error("the retained whole-parent closure does not match the live input")]
    IssuerMismatch,
    #[error("the whole-parent closure operation was cancelled")]
    Cancelled,
    #[error("the whole-parent closure operation deadline elapsed")]
    DeadlineExceeded,
}

/// Explicit parent-level bounds, separate from the all-block observation
/// bounds.  The nested block-closure limits are retained because the existing
/// block evidence is reissued before a parent observation is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationWholeParentClosureLimitsV2 {
    pub block_closure_set_limits: CommonArticulationBlockClosureSetLimitsV2,
    pub max_parent_schedule_bytes: usize,
    pub max_parent_closure_bytes: usize,
    pub max_parent_closure_leaves: usize,
    pub parent_closure_limits: DyadicIntervalClosureLimitsV1,
}

/// Exact live inputs for one full-parent, non-authorizing closure observation.
#[derive(Clone, Copy)]
pub struct CommonArticulationWholeParentClosureInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub block_closure_set: &'a CommonArticulationBlockClosureSetV2,
    pub limits: CommonArticulationWholeParentClosureLimitsV2,
}

/// Sealed general-N parent closure evidence.
///
/// The V1 certificate is deliberately private proof material.  This type has
/// no V1 conversion, `Deref`, `Clone`, or persistence trait, and its success
/// never means collision clearance or an authorization to move the project.
///
/// ```compile_fail
/// use ori_kinematics::{
///     CommonArticulationWholeParentClosureV2,
///     DyadicMaterialHingeIntervalClosureCertificateV1,
/// };
///
/// fn accepts_v1(_: DyadicMaterialHingeIntervalClosureCertificateV1) {}
/// fn rejects_v2(value: CommonArticulationWholeParentClosureV2) {
///     accepts_v1(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationWholeParentClosureV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationWholeParentClosureV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationWholeParentClosureV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationWholeParentClosureV2>();
/// ```
pub struct CommonArticulationWholeParentClosureV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    profile_binding: [u8; 32],
    decomposition_binding: [u8; 32],
    common_pose_binding: [u8; 32],
    block_closure_set_binding: [u8; 32],
    audit_binding: [u8; 32],
    parent_schedule_binding: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    closure_tolerance_bits: u64,
    configured_max_blocks: usize,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    parent_schedule_bytes: usize,
    parent_closure_bytes: usize,
    parent_closure_leaves: usize,
    limits: CommonArticulationWholeParentClosureLimitsV2,
    parent_closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    parent_closure_binding: [u8; 32],
    binding_fingerprint: [u8; 32],
}

impl fmt::Debug for CommonArticulationWholeParentClosureV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommonArticulationWholeParentClosureV2")
            .field("model_id", &self.model_id_v2())
            .field("configured_max_blocks", &self.configured_max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .field("parent_closure_leaves", &self.parent_closure_leaves)
            .field("binding_fingerprint", &self.binding_fingerprint)
            .finish_non_exhaustive()
    }
}

impl CommonArticulationWholeParentClosureV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_WHOLE_PARENT_CLOSURE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn configured_max_blocks_v2(&self) -> usize {
        self.configured_max_blocks
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn parent_closure_leaves_v2(&self) -> usize {
        self.parent_closure_leaves
    }

    #[must_use]
    pub const fn parent_schedule_bytes_v2(&self) -> usize {
        self.parent_schedule_bytes
    }

    #[must_use]
    pub const fn parent_closure_bytes_v2(&self) -> usize {
        self.parent_closure_bytes
    }

    #[must_use]
    pub const fn parent_closure_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_closure_binding
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    /// Live-reissues the all-block observation and complete parent partition
    /// before comparing every retained binding.  A stop or mismatch never
    /// authenticates the retained observation.
    pub fn revalidate_v2(
        &self,
        input: CommonArticulationWholeParentClosureInputV2<'_>,
    ) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationWholeParentClosureInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
    ) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
        let candidate = issue_v2(input, &mut checkpoint)?;
        let parent_closure_matches = parent_closures_match_with_checkpoint_v2(
            &self.parent_closure,
            &candidate.parent_closure,
            &mut checkpoint,
        )?;
        let bindings_match = self.issuer_geometry == candidate.issuer_geometry
            && self.profile_binding == candidate.profile_binding
            && self.decomposition_binding == candidate.decomposition_binding
            && self.common_pose_binding == candidate.common_pose_binding
            && self.block_closure_set_binding == candidate.block_closure_set_binding
            && self.audit_binding == candidate.audit_binding
            && self.parent_schedule_binding == candidate.parent_schedule_binding
            && self.parent_fixed_face == candidate.parent_fixed_face
            && self.paper_thickness_bits == candidate.paper_thickness_bits
            && self.closure_tolerance_bits == candidate.closure_tolerance_bits
            && self.configured_max_blocks == candidate.configured_max_blocks
            && self.actual_block_count == candidate.actual_block_count
            && self.face_count == candidate.face_count
            && self.hinge_count == candidate.hinge_count
            && self.parent_schedule_bytes == candidate.parent_schedule_bytes
            && self.parent_closure_bytes == candidate.parent_closure_bytes
            && self.parent_closure_leaves == candidate.parent_closure_leaves
            && self.limits == candidate.limits
            && self.parent_closure_binding == candidate.parent_closure_binding
            && self.binding_fingerprint == candidate.binding_fingerprint;
        checkpoint_v2(&mut checkpoint)?;
        if !bindings_match || !parent_closure_matches {
            return Err(CommonArticulationWholeParentClosureErrorV2::IssuerMismatch);
        }
        Ok(())
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
    pub const fn authorizes_layer_transport(&self) -> bool {
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

/// Issues non-authorizing parent closure evidence for an N>=33 path.
pub fn prove_common_articulation_whole_parent_closure_v2(
    input: CommonArticulationWholeParentClosureInputV2<'_>,
) -> Result<CommonArticulationWholeParentClosureV2, CommonArticulationWholeParentClosureErrorV2> {
    issue_v2(input, || Ok(()))
}

/// Controlled issuer.  Checks occur before the all-block reissue, while the
/// full-parent closure consumes its leaves, and immediately before publication.
pub fn prove_common_articulation_whole_parent_closure_with_checkpoint_v2(
    input: CommonArticulationWholeParentClosureInputV2<'_>,
    checkpoint: impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<CommonArticulationWholeParentClosureV2, CommonArticulationWholeParentClosureErrorV2> {
    issue_v2(input, checkpoint)
}

struct PreflightV2 {
    configured_max_blocks: usize,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    audit_binding: [u8; 32],
    parent_schedule_bytes: usize,
}

fn issue_v2(
    input: CommonArticulationWholeParentClosureInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<CommonArticulationWholeParentClosureV2, CommonArticulationWholeParentClosureErrorV2> {
    checkpoint_v2(&mut checkpoint)?;

    // The independently retained block observations are reissued first.  They
    // never substitute for this parent proof, but a stale lower observation
    // must not be hidden by a fresh parent closure.
    input
        .block_closure_set
        .revalidate_with_checkpoint_v2(
            CommonArticulationBlockClosureSetInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                pose: input.pose,
                parent_fixed_face: input.parent_fixed_face,
                parent_schedule: input.parent_schedule,
                decomposition: input.decomposition,
                common_pose: input.common_pose,
                paper_thickness_mm: input.paper_thickness_mm,
                closure_tolerance: input.closure_tolerance,
                profile: input.profile,
                limits: input.limits.block_closure_set_limits,
            },
            || block_closure_set_checkpoint_v2(&mut checkpoint),
        )
        .map_err(block_closure_set_error_v2)?;
    checkpoint_v2(&mut checkpoint)?;

    let preflight = preflight_v2(input, &mut checkpoint)?;
    schedule_matches_pose_at_zero_v2(
        input.parent_schedule,
        input.pose,
        input.parent_fixed_face,
        &mut checkpoint,
    )?;
    checkpoint_v2(&mut checkpoint)?;

    let parent_closure = input
        .geometry
        .prove_dyadic_schedule_closure_with_checkpoint_v1(
            input.audit,
            input.parent_fixed_face,
            input.parent_schedule,
            input.closure_tolerance,
            input.limits.parent_closure_limits,
            || parent_closure_checkpoint_v2(&mut checkpoint),
        )
        .map_err(parent_closure_error_v2)?;
    checkpoint_v2(&mut checkpoint)?;
    if parent_closure.fixed_face() != input.parent_fixed_face
        || !parent_closure.has_canonical_complete_partition_v1()
        || !parent_closure.every_leaf_covers_graph_v1(input.geometry)
    {
        return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
    }
    let parent_closure_bytes = parent_closure
        .checked_deep_retained_bytes_v1()
        .ok_or(CommonArticulationWholeParentClosureErrorV2::ResourceLimit)?;
    let parent_closure_leaves = parent_closure.leaves().len();
    if parent_closure_bytes > input.limits.max_parent_closure_bytes
        || parent_closure_leaves > input.limits.max_parent_closure_leaves
    {
        return Err(CommonArticulationWholeParentClosureErrorV2::ResourceLimit);
    }
    let parent_closure_binding = parent_closure.partition_binding_fingerprint_v2();
    let binding_fingerprint = binding_fingerprint_v2(
        input,
        &preflight,
        parent_closure_bytes,
        parent_closure_leaves,
        parent_closure_binding,
        &mut checkpoint,
    )?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(CommonArticulationWholeParentClosureV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        profile_binding: input.profile.binding_fingerprint_v2(),
        decomposition_binding: input.decomposition.binding_fingerprint_v2(),
        common_pose_binding: input.common_pose.binding_fingerprint_v2(),
        block_closure_set_binding: input.block_closure_set.binding_fingerprint_v2(),
        audit_binding: preflight.audit_binding,
        parent_schedule_binding: input.parent_schedule.certificate_binding_fingerprint_v2(),
        parent_fixed_face: input.parent_fixed_face,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        configured_max_blocks: preflight.configured_max_blocks,
        actual_block_count: preflight.actual_block_count,
        face_count: preflight.face_count,
        hinge_count: preflight.hinge_count,
        parent_schedule_bytes: preflight.parent_schedule_bytes,
        parent_closure_bytes,
        parent_closure_leaves,
        limits: input.limits,
        parent_closure,
        parent_closure_binding,
        binding_fingerprint,
    })
}

fn preflight_v2(
    input: CommonArticulationWholeParentClosureInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<PreflightV2, CommonArticulationWholeParentClosureErrorV2> {
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.closure_tolerance.is_finite()
        || input.closure_tolerance < 0.0
        || input.closure_tolerance.to_bits() == (-0.0_f64).to_bits()
        || input.limits.parent_closure_limits.max_leaves == 0
        || input.limits.parent_closure_limits.max_work == 0
        || input.limits.parent_closure_limits.max_depth >= 64
    {
        return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
    }
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || input.limits.block_closure_set_limits.max_blocks != configured_max_blocks
        || face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.decomposition.face_count_v2() != face_count
        || input.decomposition.hinge_count_v2() != hinge_count
        || input.decomposition.blocks().len() != actual_block_count
        || !input.decomposition.is_for_geometry(input.geometry)
        || !input.decomposition.is_for_profile_v2(input.profile)
        || input.geometry.face_ids() != input.audit.faces()
        || !input.parent_schedule.matches_binding(
            input.geometry,
            input.audit,
            input.parent_fixed_face,
        )
    {
        return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
    }
    let parent_schedule_bytes = input
        .parent_schedule
        .checked_deep_retained_bytes_v1()
        .ok_or(CommonArticulationWholeParentClosureErrorV2::ResourceLimit)?;
    if parent_schedule_bytes > input.limits.max_parent_schedule_bytes {
        return Err(CommonArticulationWholeParentClosureErrorV2::ResourceLimit);
    }
    let audit_binding = geometry_audit_binding_v2(input.geometry, input.audit, checkpoint)?;
    Ok(PreflightV2 {
        configured_max_blocks,
        actual_block_count,
        face_count,
        hinge_count,
        audit_binding,
        parent_schedule_bytes,
    })
}

fn schedule_matches_pose_at_zero_v2(
    schedule: &CanonicalCycleScheduleV1,
    pose: &ClosedMaterialHingeGraphPose,
    parent_fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
    if pose.fixed_face() != parent_fixed_face {
        return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
    }
    let scheduled = schedule.try_evaluate_v1(0.0).map_err(|error| match error {
        KinematicsError::ResourceLimitExceeded => {
            CommonArticulationWholeParentClosureErrorV2::ResourceLimit
        }
        _ => CommonArticulationWholeParentClosureErrorV2::InvalidInput,
    })?;
    let posed = pose.hinge_angles();
    if scheduled.as_slice().len() != posed.as_slice().len() {
        return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
    }
    for (scheduled, posed) in scheduled.as_slice().iter().zip(posed.as_slice()) {
        checkpoint_v2(checkpoint)?;
        if scheduled.edge() != posed.edge()
            || scheduled.angle_degrees().to_bits() != posed.angle_degrees().to_bits()
        {
            return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn geometry_audit_binding_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<[u8; 32], CommonArticulationWholeParentClosureErrorV2> {
    if geometry.face_ids() != audit.faces() {
        return Err(CommonArticulationWholeParentClosureErrorV2::InvalidInput);
    }
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_WHOLE_PARENT_CLOSURE_GEOMETRY_AUDIT_BINDING_V2");
    hash_count_v2(&mut hash, geometry.face_ids().len())?;
    for face in geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    hash_count_v2(&mut hash, geometry.hinges().len())?;
    for hinge in geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(hinge.edge().canonical_bytes());
        hash.update(hinge.left_face().canonical_bytes());
        hash.update(hinge.right_face().canonical_bytes());
    }
    hash_count_v2(&mut hash, audit.spanning_hinges().len())?;
    for edge in audit.spanning_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    hash_count_v2(&mut hash, audit.closure_hinges().len())?;
    for edge in audit.closure_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    Ok(hash.finalize().into())
}

fn binding_fingerprint_v2(
    input: CommonArticulationWholeParentClosureInputV2<'_>,
    preflight: &PreflightV2,
    parent_closure_bytes: usize,
    parent_closure_leaves: usize,
    parent_closure_binding: [u8; 32],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<[u8; 32], CommonArticulationWholeParentClosureErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_WHOLE_PARENT_CLOSURE_MODEL_ID_V2.as_bytes());
    hash.update(input.profile.binding_fingerprint_v2());
    hash.update(input.decomposition.binding_fingerprint_v2());
    hash.update(input.common_pose.binding_fingerprint_v2());
    hash.update(input.block_closure_set.binding_fingerprint_v2());
    hash.update(preflight.audit_binding);
    hash.update(input.parent_schedule.certificate_binding_fingerprint_v2());
    hash.update(input.parent_fixed_face.canonical_bytes());
    hash.update(input.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(input.closure_tolerance.to_bits().to_le_bytes());
    hash_count_v2(&mut hash, preflight.configured_max_blocks)?;
    hash_count_v2(&mut hash, preflight.actual_block_count)?;
    hash_count_v2(&mut hash, preflight.face_count)?;
    hash_count_v2(&mut hash, preflight.hinge_count)?;
    hash_limits_v2(&mut hash, input.limits)?;
    hash_count_v2(&mut hash, preflight.parent_schedule_bytes)?;
    hash_count_v2(&mut hash, parent_closure_bytes)?;
    hash_count_v2(&mut hash, parent_closure_leaves)?;
    hash.update(parent_closure_binding);
    checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

/// Compares the private V1 closure payload without an unchecked bulk `Eq`
/// operation.  Each retained leaf and hinge is observed through the caller's
/// checkpoint so revalidation remains interruptible after reissuance.
fn parent_closures_match_with_checkpoint_v2(
    retained: &DyadicMaterialHingeIntervalClosureCertificateV1,
    candidate: &DyadicMaterialHingeIntervalClosureCertificateV1,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<bool, CommonArticulationWholeParentClosureErrorV2> {
    checkpoint_v2(checkpoint)?;
    if retained.fixed_face() != candidate.fixed_face()
        || retained.schedule_binding_fingerprint_v2() != candidate.schedule_binding_fingerprint_v2()
        || retained.graph_binding_fingerprint_v1() != candidate.graph_binding_fingerprint_v1()
    {
        return Ok(false);
    }
    let retained_leaves = retained.leaves();
    let candidate_leaves = candidate.leaves();
    if retained_leaves.len() != candidate_leaves.len() {
        return Ok(false);
    }
    for (retained_leaf, candidate_leaf) in retained_leaves.iter().zip(candidate_leaves) {
        checkpoint_v2(checkpoint)?;
        let (retained_depth, retained_index, retained_certificate) = retained_leaf;
        let (candidate_depth, candidate_index, candidate_certificate) = candidate_leaf;
        if retained_depth != candidate_depth
            || retained_index != candidate_index
            || retained_certificate.version() != candidate_certificate.version()
            || retained_certificate.fixed_face() != candidate_certificate.fixed_face()
        {
            return Ok(false);
        }
        let retained_hinges = retained_certificate.checked_hinges();
        let candidate_hinges = candidate_certificate.checked_hinges();
        if retained_hinges.len() != candidate_hinges.len() {
            return Ok(false);
        }
        for (retained_hinge, candidate_hinge) in retained_hinges.iter().zip(candidate_hinges) {
            checkpoint_v2(checkpoint)?;
            if retained_hinge != candidate_hinge {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn hash_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationWholeParentClosureLimitsV2,
) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
    let block = limits.block_closure_set_limits;
    for value in [
        block.max_blocks,
        block.max_parent_schedule_bytes,
        block.max_block_schedule_bytes,
        block.max_total_block_schedule_bytes,
        block.max_block_closure_bytes,
        block.max_total_block_closure_bytes,
        block.max_total_closure_leaves,
        limits.max_parent_schedule_bytes,
        limits.max_parent_closure_bytes,
        limits.max_parent_closure_leaves,
    ] {
        hash_count_v2(hash, value)?;
    }
    hash_dyadic_limits_v2(hash, block.per_block_closure_limits)?;
    hash_dyadic_limits_v2(hash, limits.parent_closure_limits)
}

fn hash_dyadic_limits_v2(
    hash: &mut Sha256,
    limits: DyadicIntervalClosureLimitsV1,
) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
    hash.update(limits.max_depth.to_le_bytes());
    hash_count_v2(hash, limits.max_leaves)?;
    hash_count_v2(hash, limits.max_work)?;
    hash_count_v2(hash, limits.schedule_limits.max_hinges)?;
    hash_count_v2(hash, limits.schedule_limits.max_degree)?;
    hash.update(limits.schedule_limits.max_coefficient_bits.to_le_bytes());
    hash_count_v2(hash, limits.schedule_limits.max_work)
}

fn hash_count_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationWholeParentClosureErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}

fn block_closure_set_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<(), CommonArticulationBlockClosureSetStopV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationWholeParentClosureStopV2::Cancelled => {
            CommonArticulationBlockClosureSetStopV2::Cancelled
        }
        CommonArticulationWholeParentClosureStopV2::DeadlineExceeded => {
            CommonArticulationBlockClosureSetStopV2::DeadlineExceeded
        }
    })
}

fn block_closure_set_error_v2(
    error: CommonArticulationBlockClosureSetErrorV2,
) -> CommonArticulationWholeParentClosureErrorV2 {
    match error {
        CommonArticulationBlockClosureSetErrorV2::Cancelled => {
            CommonArticulationWholeParentClosureErrorV2::Cancelled
        }
        CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded => {
            CommonArticulationWholeParentClosureErrorV2::DeadlineExceeded
        }
        error => CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(error),
    }
}

fn parent_closure_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<(), DyadicIntervalClosureStopV1> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationWholeParentClosureStopV2::Cancelled => {
            DyadicIntervalClosureStopV1::Cancelled
        }
        CommonArticulationWholeParentClosureStopV2::DeadlineExceeded => {
            DyadicIntervalClosureStopV1::DeadlineExceeded
        }
    })
}

fn parent_closure_error_v2(
    error: DyadicIntervalClosureControlErrorV1,
) -> CommonArticulationWholeParentClosureErrorV2 {
    match error {
        DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit,
        ) => CommonArticulationWholeParentClosureErrorV2::ResourceLimit,
        DyadicIntervalClosureControlErrorV1::Closure(error) => {
            CommonArticulationWholeParentClosureErrorV2::ParentClosure(error)
        }
        DyadicIntervalClosureControlErrorV1::Cancelled => {
            CommonArticulationWholeParentClosureErrorV2::Cancelled
        }
        DyadicIntervalClosureControlErrorV1::DeadlineExceeded => {
            CommonArticulationWholeParentClosureErrorV2::DeadlineExceeded
        }
    }
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationWholeParentClosureStopV2>,
) -> Result<(), CommonArticulationWholeParentClosureErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationWholeParentClosureStopV2::Cancelled => {
            CommonArticulationWholeParentClosureErrorV2::Cancelled
        }
        CommonArticulationWholeParentClosureStopV2::DeadlineExceeded => {
            CommonArticulationWholeParentClosureErrorV2::DeadlineExceeded
        }
    })
}
