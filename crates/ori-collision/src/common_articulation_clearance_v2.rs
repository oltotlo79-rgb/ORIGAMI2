//! General-N common-articulation clearance prerequisites.
//!
//! This V2 boundary deliberately does **not** manufacture a clearance
//! certificate from endpoint observations, per-block proofs, or a canonical
//! pair registry.  A whole-parent positive-thickness continuous certificate
//! for V2 does not exist yet.  The value issued here is therefore explicitly
//! unpromoted: it records and revalidates every input a later positive proof
//! must bind, while granting no collision-clearance authority.
//!
//! The complete public surface is intentionally nameable from outside this
//! crate even though no value can be forged without the live V2 evidence.
//!
//! ```
//! use ori_collision::{
//!     COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V2,
//!     COMMON_ARTICULATION_CLEARANCE_UNPROMOTED_MODEL_ID_V2,
//!     CommonArticulationClearanceErrorV2, CommonArticulationClearanceInputV2,
//!     CommonArticulationClearanceOutcomeV2, CommonArticulationClearancePrerequisiteV2,
//!     CommonArticulationClearanceRevalidationInputV2, CommonArticulationClearanceStopV2,
//!     CommonArticulationClearanceUnpromotedReasonV2, CommonArticulationCrossBlockFacePairV2,
//!     issue_common_articulation_clearance_prerequisite_v2,
//!     issue_common_articulation_clearance_prerequisite_with_checkpoint_v2,
//! };
//!
//! fn public_types_are_nameable<'a>(
//!     _: Option<&CommonArticulationClearanceErrorV2>,
//!     _: Option<&CommonArticulationClearanceInputV2<'a>>,
//!     _: Option<&CommonArticulationClearanceOutcomeV2>,
//!     _: Option<&CommonArticulationClearancePrerequisiteV2>,
//!     _: Option<&CommonArticulationClearanceRevalidationInputV2<'a>>,
//!     _: Option<&CommonArticulationClearanceStopV2>,
//!     _: Option<&CommonArticulationClearanceUnpromotedReasonV2>,
//!     _: Option<&CommonArticulationCrossBlockFacePairV2>,
//! ) {}
//! let _ = COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V2;
//! let _ = COMMON_ARTICULATION_CLEARANCE_UNPROMOTED_MODEL_ID_V2;
//! ```

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationBlockClosureSetV2,
    CommonArticulationPoseAuthorityV2, CommonArticulationPoseErrorV2,
    CommonArticulationPoseInputV2, CommonArticulationPoseStopV2,
    CommonArticulationResourceProfileV2, CommonArticulationWholeParentClosureErrorV2,
    CommonArticulationWholeParentClosureInputV2, CommonArticulationWholeParentClosureLimitsV2,
    CommonArticulationWholeParentClosureStopV2, CommonArticulationWholeParentClosureV2,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use thiserror::Error;

/// Stable domain identifier for the unpromoted V2 prerequisite.
pub const COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V2: &str =
    "common_articulation_cross_block_clearance_prerequisite_v2";
/// Stable domain identifier for the corresponding outcome.
pub const COMMON_ARTICULATION_CLEARANCE_UNPROMOTED_MODEL_ID_V2: &str =
    "common_articulation_cross_block_clearance_unpromoted_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const CANONICAL_MIURA_FACES_PER_BLOCK_V2: usize = 9;
const CANONICAL_MIURA_HINGES_PER_BLOCK_V2: usize = 12;
const CANONICAL_MIURA_RAW_PAIR_CANDIDATES_PER_BLOCK_PAIR_V2: usize = 81;
const CANONICAL_MIURA_CANONICAL_PAIRS_PER_ORDERED_BLOCK_PAIR_V2: usize = 32;
const CLEARANCE_BASE_WORK_V2: usize = 32;
const CLEARANCE_PAIR_BYTES_V2: usize = 32;
const CLEARANCE_BASE_BYTES_V2: usize = 1_024;
const CLEARANCE_FACE_BYTES_V2: usize = 128;
const CLEARANCE_HINGE_BYTES_V2: usize = 32;
// `CommonArticulationBlockClosureSetV2` retains one private record per block.
// The record owns two already-charged V1 observations plus a fixed binding
// header. Keep a deliberately roomy public-boundary charge for that header;
// the dynamic schedule/closure payload is charged separately below.
const CLEARANCE_REVALIDATION_BLOCK_RECORD_BYTES_UPPER_BOUND_V2: usize = 512;
const CLEARANCE_REVALIDATION_BASE_BYTES_V2: usize = 1_024;
// Covers checkpoint-pollable raw/local heap sorts, local dedup, explicit
// write-index compaction, and sorted local-pair membership.  The explicit
// factor keeps V2 resource accounting ahead of the implementation without an
// opaque library sort.
const CLEARANCE_HEAPSORT_COMPARISON_FACTOR_V2: usize = 8;

/// Cooperative stop requested by a V2 clearance prerequisite operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationClearanceStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// V2 prerequisite issuance or revalidation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationClearanceErrorV2 {
    #[error("the general-N common-articulation clearance input is malformed")]
    InvalidInput,
    #[error("the general-N common-articulation clearance input exceeds its resource profile")]
    ResourceLimit,
    #[error("the live graph audit does not match the parent geometry")]
    AuditBindingMismatch,
    #[error("the retained V2 pose authority does not match the exact live input: {0}")]
    CommonPose(CommonArticulationPoseErrorV2),
    #[error("the retained V2 whole-parent closure does not match the exact live input: {0}")]
    WholeParentClosure(CommonArticulationWholeParentClosureErrorV2),
    #[error("the submitted cross-block pair registry is not in canonical order")]
    NonCanonicalCrossBlockPairRegistry,
    #[error("the submitted cross-block pair registry contains a duplicate")]
    DuplicateCrossBlockPair,
    #[error(
        "the submitted cross-block pair registry is incomplete or contains an extra pair \\
         (expected {expected}, actual {actual})"
    )]
    CrossBlockPairCoverageMismatch { expected: usize, actual: usize },
    #[error("the retained V2 clearance prerequisite does not match the exact live input")]
    PrerequisiteBindingMismatch,
    #[error("the V2 clearance prerequisite operation was cancelled")]
    Cancelled,
    #[error("the V2 clearance prerequisite operation deadline elapsed")]
    DeadlineExceeded,
}

/// One canonical unordered face pair spanning two distinct V2 edge blocks.
///
/// The pair itself carries no positive collision fact.  It merely fixes the
/// complete registry that a future whole-parent proof must cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommonArticulationCrossBlockFacePairV2 {
    first: FaceId,
    second: FaceId,
}

impl CommonArticulationCrossBlockFacePairV2 {
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
    pub const fn first_v2(self) -> FaceId {
        self.first
    }

    #[must_use]
    pub const fn second_v2(self) -> FaceId {
        self.second
    }
}

/// Why a V2 clearance prerequisite deliberately remains unpromoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationClearanceUnpromotedReasonV2 {
    /// No V2 whole-parent positive-thickness continuous certificate type is
    /// available.  Independent block facts cannot fill this theorem gap.
    WholeParentPositiveThicknessEvidenceUnavailable,
}

/// Live inputs for a general-N clearance prerequisite.
///
/// The submitted pair registry must already be canonical, exactly complete,
/// and tied to the same V2 profile as both the decomposition and pose token.
/// Parent schedule/fixed-face/tolerance inputs and both closure observations
/// are likewise one live tuple: the whole-parent revalidation below reissues
/// its internal all-block observation before this boundary retains anything.
#[derive(Clone, Copy)]
pub struct CommonArticulationClearanceInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    /// Retained independently, and live-revalidated through the enclosing
    /// whole-parent closure before this prerequisite is issued.
    pub block_closure_set: &'a CommonArticulationBlockClosureSetV2,
    pub whole_parent_closure: &'a CommonArticulationWholeParentClosureV2,
    pub whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    pub submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV2],
}

/// Live inputs required to revalidate an unpromoted V2 prerequisite.
///
/// The canonical registry is retained by the prerequisite itself; callers
/// cannot replace it during revalidation.  All parent-proof inputs remain
/// explicit so replay cannot silently substitute a stale closure observation.
#[derive(Debug, Clone, Copy)]
pub struct CommonArticulationClearanceRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub block_closure_set: &'a CommonArticulationBlockClosureSetV2,
    pub whole_parent_closure: &'a CommonArticulationWholeParentClosureV2,
    pub whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
}

/// Sealed but explicitly unpromoted V2 clearance prerequisite.
///
/// It has no V1 conversion, no `Deref`, no persistence traits, and no
/// authorization.  Until a positive whole-parent V2 certificate is added,
/// this is the strongest sound result available at this boundary.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationClearancePrerequisiteV1,
///     CommonArticulationClearancePrerequisiteV2,
/// };
///
/// fn accepts_v1(_: CommonArticulationClearancePrerequisiteV1) {}
/// fn rejects_v2(value: CommonArticulationClearancePrerequisiteV2) {
///     accepts_v1(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationClearancePrerequisiteV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationClearancePrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationClearancePrerequisiteV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationClearancePrerequisiteV2>();
/// ```
#[derive(Debug)]
pub struct CommonArticulationClearancePrerequisiteV2 {
    profile_binding: [u8; 32],
    decomposition_binding: [u8; 32],
    common_pose_binding: [u8; 32],
    block_closure_set_binding: [u8; 32],
    whole_parent_closure_binding: [u8; 32],
    audit_binding: [u8; 32],
    parent_schedule_binding: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    closure_tolerance_bits: u64,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV2>,
    logical_work: usize,
    storage_bytes_upper_bound: usize,
    whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationClearancePrerequisiteV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn unpromoted_reason_v2(&self) -> CommonArticulationClearanceUnpromotedReasonV2 {
        CommonArticulationClearanceUnpromotedReasonV2::WholeParentPositiveThicknessEvidenceUnavailable
    }

    #[must_use]
    pub fn cross_block_pairs_v2(&self) -> &[CommonArticulationCrossBlockFacePairV2] {
        &self.cross_block_pairs
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn face_count_v2(&self) -> usize {
        self.face_count
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.hinge_count
    }

    #[must_use]
    pub const fn paper_thickness_mm_v2(&self) -> f64 {
        f64::from_bits(self.paper_thickness_bits)
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn storage_bytes_upper_bound_v2(&self) -> usize {
        self.storage_bytes_upper_bound
    }

    #[must_use]
    pub const fn profile_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.profile_binding
    }

    #[must_use]
    pub const fn decomposition_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.decomposition_binding
    }

    #[must_use]
    pub const fn common_pose_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.common_pose_binding
    }

    #[must_use]
    pub const fn block_closure_set_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.block_closure_set_binding
    }

    #[must_use]
    pub const fn whole_parent_closure_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.whole_parent_closure_binding
    }

    #[must_use]
    pub const fn audit_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.audit_binding
    }

    #[must_use]
    pub const fn parent_schedule_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_schedule_binding
    }

    #[must_use]
    pub const fn parent_fixed_face_v2(&self) -> FaceId {
        self.parent_fixed_face
    }

    #[must_use]
    pub const fn closure_tolerance_v2(&self) -> f64 {
        f64::from_bits(self.closure_tolerance_bits)
    }

    #[must_use]
    pub const fn whole_parent_closure_limits_v2(
        &self,
    ) -> CommonArticulationWholeParentClosureLimitsV2 {
        self.whole_parent_closure_limits
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    /// Revalidates every exact V2 source binding and the retained canonical
    /// pair registry.  Success still does not promote the prerequisite to a
    /// positive clearance certificate.
    pub fn revalidate_v2(
        &self,
        input: CommonArticulationClearanceRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationClearanceErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationClearanceRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
    ) -> Result<(), CommonArticulationClearanceErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        let validation_input = CommonArticulationClearanceInputV2 {
            geometry: input.geometry,
            audit: input.audit,
            pose: input.pose,
            decomposition: input.decomposition,
            common_pose: input.common_pose,
            parent_fixed_face: input.parent_fixed_face,
            parent_schedule: input.parent_schedule,
            profile: input.profile,
            paper_thickness_mm: input.paper_thickness_mm,
            closure_tolerance: input.closure_tolerance,
            block_closure_set: input.block_closure_set,
            whole_parent_closure: input.whole_parent_closure,
            whole_parent_closure_limits: input.whole_parent_closure_limits,
            submitted_cross_block_pairs: &self.cross_block_pairs,
        };
        let validated = validate_input_v2(&validation_input, &mut checkpoint).map_err(|error| {
            // The registry is sealed inside this prerequisite, so a registry
            // mismatch during revalidation can only arise from a foreign live
            // source rather than fresh caller-supplied pair data.
            match error {
                CommonArticulationClearanceErrorV2::CrossBlockPairCoverageMismatch { .. }
                | CommonArticulationClearanceErrorV2::DuplicateCrossBlockPair
                | CommonArticulationClearanceErrorV2::NonCanonicalCrossBlockPairRegistry => {
                    CommonArticulationClearanceErrorV2::PrerequisiteBindingMismatch
                }
                error => error,
            }
        })?;
        let binding_fingerprint = clearance_binding_fingerprint_v2(
            &validated,
            self.cross_block_pairs.as_slice(),
            &mut checkpoint,
        )?;
        let pairs_match = cross_block_pairs_equal_with_checkpoint_v2(
            &self.cross_block_pairs,
            &validated.cross_block_pairs,
            &mut checkpoint,
        )?;
        checkpoint_v2(&mut checkpoint)?;
        let bindings_match = self.profile_binding == validated.profile_binding
            && self.decomposition_binding == validated.decomposition_binding
            && self.common_pose_binding == validated.common_pose_binding
            && self.block_closure_set_binding == validated.block_closure_set_binding
            && self.whole_parent_closure_binding == validated.whole_parent_closure_binding
            && self.audit_binding == validated.audit_binding
            && self.parent_schedule_binding == validated.parent_schedule_binding
            && self.parent_fixed_face == validated.parent_fixed_face
            && self.paper_thickness_bits == validated.paper_thickness_bits
            && self.closure_tolerance_bits == validated.closure_tolerance_bits
            && self.actual_block_count == validated.actual_block_count
            && self.face_count == validated.face_count
            && self.hinge_count == validated.hinge_count
            && self.logical_work == validated.logical_work
            && self.storage_bytes_upper_bound == validated.storage_bytes_upper_bound
            && self.whole_parent_closure_limits == validated.whole_parent_closure_limits
            && self.binding_fingerprint == binding_fingerprint;
        if !bindings_match || !pairs_match {
            checkpoint_v2(&mut checkpoint)?;
            return Err(CommonArticulationClearanceErrorV2::PrerequisiteBindingMismatch);
        }
        checkpoint_v2(&mut checkpoint)
    }

    /// There is no V2 whole-parent positive continuous evidence yet.
    #[must_use]
    pub const fn cross_block_open_interval_clearance_proven_v2(&self) -> bool {
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

    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
}

/// The only currently sound V2 clearance result.
///
/// There is intentionally no `Certified` variant.  Adding one requires an
/// independently issued whole-parent positive-thickness V2 certificate.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationClearanceOutcomeV1, CommonArticulationClearanceOutcomeV2,
/// };
///
/// fn accepts_v1(_: CommonArticulationClearanceOutcomeV1) {}
/// fn rejects_v2(value: CommonArticulationClearanceOutcomeV2) {
///     accepts_v1(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationClearanceOutcomeV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationClearanceOutcomeV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationClearanceOutcomeV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationClearanceOutcomeV2>();
/// ```
#[derive(Debug)]
pub enum CommonArticulationClearanceOutcomeV2 {
    Unpromoted(Box<CommonArticulationClearancePrerequisiteV2>),
}

impl CommonArticulationClearanceOutcomeV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_CLEARANCE_UNPROMOTED_MODEL_ID_V2
    }

    #[must_use]
    pub const fn is_certified_v2(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn unpromoted_reason_v2(&self) -> CommonArticulationClearanceUnpromotedReasonV2 {
        CommonArticulationClearanceUnpromotedReasonV2::WholeParentPositiveThicknessEvidenceUnavailable
    }

    #[must_use]
    pub fn as_unpromoted_v2(&self) -> &CommonArticulationClearancePrerequisiteV2 {
        match self {
            Self::Unpromoted(prerequisite) => prerequisite.as_ref(),
        }
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

    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
}

/// Issues a closed, unpromoted prerequisite for the exact V2 live inputs.
pub fn issue_common_articulation_clearance_prerequisite_v2(
    input: CommonArticulationClearanceInputV2<'_>,
) -> Result<CommonArticulationClearanceOutcomeV2, CommonArticulationClearanceErrorV2> {
    issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(input, || Ok(()))
}

/// As [`issue_common_articulation_clearance_prerequisite_v2`], with
/// cooperative cancellation and deadline checkpoints.
pub fn issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(
    input: CommonArticulationClearanceInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<CommonArticulationClearanceOutcomeV2, CommonArticulationClearanceErrorV2> {
    checkpoint_v2(&mut checkpoint)?;
    let validated = validate_input_v2(&input, &mut checkpoint)?;
    let binding_fingerprint = clearance_binding_fingerprint_v2(
        &validated,
        validated.cross_block_pairs.as_slice(),
        &mut checkpoint,
    )?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(CommonArticulationClearanceOutcomeV2::Unpromoted(Box::new(
        CommonArticulationClearancePrerequisiteV2 {
            profile_binding: validated.profile_binding,
            decomposition_binding: validated.decomposition_binding,
            common_pose_binding: validated.common_pose_binding,
            block_closure_set_binding: validated.block_closure_set_binding,
            whole_parent_closure_binding: validated.whole_parent_closure_binding,
            audit_binding: validated.audit_binding,
            parent_schedule_binding: validated.parent_schedule_binding,
            parent_fixed_face: validated.parent_fixed_face,
            paper_thickness_bits: validated.paper_thickness_bits,
            closure_tolerance_bits: validated.closure_tolerance_bits,
            actual_block_count: validated.actual_block_count,
            face_count: validated.face_count,
            hinge_count: validated.hinge_count,
            cross_block_pairs: validated.cross_block_pairs,
            logical_work: validated.logical_work,
            storage_bytes_upper_bound: validated.storage_bytes_upper_bound,
            whole_parent_closure_limits: validated.whole_parent_closure_limits,
            binding_fingerprint,
        },
    )))
}

mod validation;

use validation::{
    checkpoint_v2, clearance_binding_fingerprint_v2, cross_block_pairs_equal_with_checkpoint_v2,
    validate_input_v2,
};

#[cfg(test)]
#[path = "common_articulation_clearance_v2/test_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "common_articulation_clearance_v2/tests.rs"]
mod tests;
