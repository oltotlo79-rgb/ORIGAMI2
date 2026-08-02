//! Dynamic general-N cross-block clearance prerequisites.
//!
//! This is a sibling of the stationary V2 clearance route.  It accepts only
//! the opaque dynamic-closure bridge from `ori-kinematics`, fully replays that
//! bridge for each issue/revalidation operation, and records a complete
//! cross-block face-pair registry.  It deliberately grants no clearance,
//! motion, transport, Apply, or viewer authority.
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicClosureClearancePrerequisiteV2;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<CommonArticulationDynamicClosureClearancePrerequisiteV2>();
//! ```
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicClosureClearancePrerequisiteV2;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<CommonArticulationDynamicClosureClearancePrerequisiteV2>();
//! ```
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicClosureClearancePrerequisiteV2;
//!
//! fn require_deref<T: std::ops::Deref>() {}
//! require_deref::<CommonArticulationDynamicClosureClearancePrerequisiteV2>();
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationClearancePrerequisiteV1,
//!     CommonArticulationDynamicClosureClearancePrerequisiteV2,
//! };
//!
//! fn accepts_v1(_: CommonArticulationClearancePrerequisiteV1) {}
//! fn rejects_dynamic(value: CommonArticulationDynamicClosureClearancePrerequisiteV2) {
//!     accepts_v1(value);
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationDynamicClosureClearancePrerequisiteV2,
//!     CommonArticulationGeneralCellTransportPrerequisiteV2,
//! };
//!
//! fn accepts_general_cell(_: CommonArticulationGeneralCellTransportPrerequisiteV2) {}
//! fn rejects_dynamic(value: CommonArticulationDynamicClosureClearancePrerequisiteV2) {
//!     accepts_general_cell(value);
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2,
//!     CommonArticulationDynamicClosureClearancePrerequisiteV2,
//! };
//!
//! fn accepts_compact(_: CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2) {}
//! fn rejects_dynamic(value: CommonArticulationDynamicClosureClearancePrerequisiteV2) {
//!     accepts_compact(value);
//! }
//! ```

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationDynamicClosureBridgeErrorV2,
    CommonArticulationDynamicClosureBridgeRevalidationInputV2,
    CommonArticulationDynamicClosureBridgeStopV2, CommonArticulationDynamicClosureBridgeV2,
    CommonArticulationPoseAuthorityV2, CommonArticulationResourceProfileV2,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use thiserror::Error;

use crate::CommonArticulationCrossBlockFacePairV2;

/// Stable domain identifier for the dynamic, unpromoted prerequisite.
pub const COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_PREREQUISITE_MODEL_ID_V2: &str =
    "common_articulation_dynamic_closure_clearance_prerequisite_v2";
/// Stable domain identifier for the matching outcome.
pub const COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_UNPROMOTED_MODEL_ID_V2: &str =
    "common_articulation_dynamic_closure_clearance_unpromoted_v2";

/// Cooperative stop requested while issuing or replaying dynamic clearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicClosureClearanceStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Fail-closed dynamic clearance issue or replay error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicClosureClearanceErrorV2 {
    #[error("the dynamic clearance input is malformed")]
    InvalidInput,
    #[error("the dynamic clearance input exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the supplied dynamic closure bridge does not replay: {0}")]
    DynamicClosureBridge(CommonArticulationDynamicClosureBridgeErrorV2),
    #[error("the submitted cross-block pair registry is not in canonical order")]
    NonCanonicalCrossBlockPairRegistry,
    #[error("the submitted cross-block pair registry contains a duplicate")]
    DuplicateCrossBlockPair,
    #[error(
        "the submitted cross-block pair registry is incomplete or contains an extra pair \
         (expected {expected}, actual {actual})"
    )]
    CrossBlockPairCoverageMismatch { expected: usize, actual: usize },
    #[error("the retained dynamic clearance prerequisite does not match the live input")]
    PrerequisiteBindingMismatch,
    #[error("the dynamic clearance operation was cancelled")]
    Cancelled,
    #[error("the dynamic clearance operation deadline elapsed")]
    DeadlineExceeded,
}

/// Finite outer bounds for one dynamic clearance prerequisite.
///
/// The opaque bridge has its own sealed inner policy.  These limits charge
/// the bridge's complete revalidation peak plus this route's independent pair
/// registry and publication allocations before replay begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicClosureClearanceLimitsV2 {
    pub max_blocks: usize,
    pub max_faces: usize,
    pub max_cross_block_pairs: usize,
    pub max_pair_registry_retained_bytes: usize,
    pub max_pair_registry_temporary_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

/// Exact live inputs for a dynamic clearance prerequisite issue.
#[derive(Clone, Copy)]
pub struct CommonArticulationDynamicClosureClearanceInputV2<'a> {
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
    pub dynamic_closure_bridge: &'a CommonArticulationDynamicClosureBridgeV2,
    pub submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV2],
    pub limits: CommonArticulationDynamicClosureClearanceLimitsV2,
}

/// Exact live inputs required to replay a dynamic clearance prerequisite.
///
/// The retained pair registry and outer resource policy are sealed in the
/// prerequisite, while the bridge remains caller-owned and must replay the
/// complete live source tuple again.
#[derive(Clone, Copy)]
pub struct CommonArticulationDynamicClosureClearanceRevalidationInputV2<'a> {
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
    pub dynamic_closure_bridge: &'a CommonArticulationDynamicClosureBridgeV2,
}

/// Why dynamic clearance deliberately remains unpromoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicClosureClearanceUnpromotedReasonV2 {
    /// A dynamic closure proof and complete pair registry are not a positive
    /// whole-parent clearance theorem.
    WholeParentPositiveThicknessEvidenceUnavailable,
}

/// Debug-only, sealed dynamic clearance prerequisite.
///
/// It owns neither the bridge nor any legacy block/whole-parent closure.  It
/// has no V1 conversion, serde, `Deref`, raw bridge accessor, or authorization
/// predicate.
#[derive(Debug)]
pub struct CommonArticulationDynamicClosureClearancePrerequisiteV2 {
    profile_binding: [u8; 32],
    decomposition_binding: [u8; 32],
    common_pose_binding: [u8; 32],
    audit_binding: [u8; 32],
    parent_schedule_binding: [u8; 32],
    bridge_binding: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    closure_tolerance_bits: u64,
    actual_block_count: usize,
    actual_face_count: usize,
    cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV2>,
    pair_registry_retained_bytes: usize,
    pair_registry_temporary_bytes: usize,
    publication_bytes: usize,
    aggregate_peak_bytes: usize,
    limits: CommonArticulationDynamicClosureClearanceLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationDynamicClosureClearancePrerequisiteV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_PREREQUISITE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn unpromoted_reason_v2(
        &self,
    ) -> CommonArticulationDynamicClosureClearanceUnpromotedReasonV2 {
        CommonArticulationDynamicClosureClearanceUnpromotedReasonV2::WholeParentPositiveThicknessEvidenceUnavailable
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn actual_face_count_v2(&self) -> usize {
        self.actual_face_count
    }

    #[must_use]
    pub const fn pair_registry_retained_bytes_upper_bound_v2(&self) -> usize {
        self.pair_registry_retained_bytes
    }

    #[must_use]
    pub const fn pair_registry_temporary_bytes_upper_bound_v2(&self) -> usize {
        self.pair_registry_temporary_bytes
    }

    #[must_use]
    pub const fn publication_bytes_upper_bound_v2(&self) -> usize {
        self.publication_bytes
    }

    #[must_use]
    pub const fn aggregate_peak_bytes_upper_bound_v2(&self) -> usize {
        self.aggregate_peak_bytes
    }

    /// Fully replays the caller-owned bridge and all retained bindings.
    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicClosureClearanceRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    /// As [`Self::revalidate_v2`], with cooperative checkpoints.
    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicClosureClearanceRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
    ) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
        validation::checkpoint_v2(&mut checkpoint)?;
        let candidate = CommonArticulationDynamicClosureClearanceInputV2 {
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
            dynamic_closure_bridge: input.dynamic_closure_bridge,
            submitted_cross_block_pairs: self.cross_block_pairs.as_slice(),
            limits: self.limits,
        };
        let validated = validation::validate_input_v2(&candidate, &mut checkpoint)?;
        let binding = validation::binding_fingerprint_v2(&validated, &mut checkpoint)?;
        validation::checkpoint_v2(&mut checkpoint)?;
        let bindings_match = self.profile_binding == validated.profile_binding
            && self.decomposition_binding == validated.decomposition_binding
            && self.common_pose_binding == validated.common_pose_binding
            && self.audit_binding == validated.audit_binding
            && self.parent_schedule_binding == validated.parent_schedule_binding
            && self.bridge_binding == validated.bridge_binding
            && self.parent_fixed_face == validated.parent_fixed_face
            && self.paper_thickness_bits == validated.paper_thickness_bits
            && self.closure_tolerance_bits == validated.closure_tolerance_bits
            && self.actual_block_count == validated.actual_block_count
            && self.actual_face_count == validated.actual_face_count
            && self.cross_block_pairs == validated.cross_block_pairs
            && self.pair_registry_retained_bytes == validated.pair_registry_retained_bytes
            && self.pair_registry_temporary_bytes == validated.pair_registry_temporary_bytes
            && self.publication_bytes == validated.publication_bytes
            && self.aggregate_peak_bytes == validated.aggregate_peak_bytes
            && self.limits == validated.limits
            && self.binding_fingerprint == binding;
        if !bindings_match {
            return Err(
                CommonArticulationDynamicClosureClearanceErrorV2::PrerequisiteBindingMismatch,
            );
        }
        validation::checkpoint_v2(&mut checkpoint)
    }
}

/// The only sound dynamic clearance result currently available.
#[derive(Debug)]
pub enum CommonArticulationDynamicClosureClearanceOutcomeV2 {
    Unpromoted(Box<CommonArticulationDynamicClosureClearancePrerequisiteV2>),
}

impl CommonArticulationDynamicClosureClearanceOutcomeV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_UNPROMOTED_MODEL_ID_V2
    }

    #[must_use]
    pub const fn is_certified_v2(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn unpromoted_reason_v2(
        &self,
    ) -> CommonArticulationDynamicClosureClearanceUnpromotedReasonV2 {
        CommonArticulationDynamicClosureClearanceUnpromotedReasonV2::WholeParentPositiveThicknessEvidenceUnavailable
    }

    #[must_use]
    pub fn as_unpromoted_v2(&self) -> &CommonArticulationDynamicClosureClearancePrerequisiteV2 {
        match self {
            Self::Unpromoted(value) => value,
        }
    }
}

/// Issues an unpromoted dynamic clearance prerequisite.
pub fn issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(
    input: CommonArticulationDynamicClosureClearanceInputV2<'_>,
) -> Result<
    CommonArticulationDynamicClosureClearanceOutcomeV2,
    CommonArticulationDynamicClosureClearanceErrorV2,
> {
    issue_common_articulation_dynamic_closure_clearance_prerequisite_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

/// As [`issue_common_articulation_dynamic_closure_clearance_prerequisite_v2`],
/// with cooperative cancellation and deadline checkpoints.
pub fn issue_common_articulation_dynamic_closure_clearance_prerequisite_with_checkpoint_v2(
    input: CommonArticulationDynamicClosureClearanceInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<
    CommonArticulationDynamicClosureClearanceOutcomeV2,
    CommonArticulationDynamicClosureClearanceErrorV2,
> {
    validation::checkpoint_v2(&mut checkpoint)?;
    let validated = validation::validate_input_v2(&input, &mut checkpoint)?;
    let binding_fingerprint = validation::binding_fingerprint_v2(&validated, &mut checkpoint)?;
    validation::checkpoint_v2(&mut checkpoint)?;
    Ok(
        CommonArticulationDynamicClosureClearanceOutcomeV2::Unpromoted(Box::new(
            CommonArticulationDynamicClosureClearancePrerequisiteV2 {
                profile_binding: validated.profile_binding,
                decomposition_binding: validated.decomposition_binding,
                common_pose_binding: validated.common_pose_binding,
                audit_binding: validated.audit_binding,
                parent_schedule_binding: validated.parent_schedule_binding,
                bridge_binding: validated.bridge_binding,
                parent_fixed_face: validated.parent_fixed_face,
                paper_thickness_bits: validated.paper_thickness_bits,
                closure_tolerance_bits: validated.closure_tolerance_bits,
                actual_block_count: validated.actual_block_count,
                actual_face_count: validated.actual_face_count,
                cross_block_pairs: validated.cross_block_pairs,
                pair_registry_retained_bytes: validated.pair_registry_retained_bytes,
                pair_registry_temporary_bytes: validated.pair_registry_temporary_bytes,
                publication_bytes: validated.publication_bytes,
                aggregate_peak_bytes: validated.aggregate_peak_bytes,
                limits: validated.limits,
                binding_fingerprint,
            },
        )),
    )
}

mod validation;

#[cfg(test)]
#[path = "common_articulation_dynamic_closure_clearance_v2/tests.rs"]
mod tests;
