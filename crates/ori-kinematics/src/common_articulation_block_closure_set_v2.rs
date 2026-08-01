//! Non-authorizing, live-revalidatable all-block closure evidence for the
//! general-N common-articulation path.
//!
//! V1 schedules and interval closures are retained solely as observations.
//! This boundary creates neither a V2-to-V1 authority conversion nor a
//! continuous-motion, positive-thickness, or mutation authorization.

use std::fmt;

use ori_domain::FaceId;
use thiserror::Error;

use crate::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationPoseAuthorityV2,
    CommonArticulationResourceProfileV2, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
};

mod validation;

/// Stable model identifier for general-N block-closure-set provenance.
pub const COMMON_ARTICULATION_BLOCK_CLOSURE_SET_MODEL_ID_V2: &str =
    "common_articulation_block_closure_set_v2";

/// Cooperative stop requested by block-closure-set issuance or revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationBlockClosureSetStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Failure while creating or revalidating all-block V2 closure evidence.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationBlockClosureSetErrorV2 {
    #[error("the general-N block-closure-set input is malformed or foreign")]
    InvalidInput,
    #[error("the general-N block-closure-set exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the retained block-closure-set does not match the live input")]
    IssuerMismatch,
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("the operation deadline elapsed")]
    DeadlineExceeded,
}

/// Explicit bounds for retained per-block schedule and closure observations.
///
/// `max_blocks` must equal the profile's *configured* maximum.  Actual N is
/// retained separately, so configured N=40 / actual N=34 remains valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationBlockClosureSetLimitsV2 {
    pub max_blocks: usize,
    pub max_parent_schedule_bytes: usize,
    pub max_block_schedule_bytes: usize,
    pub max_total_block_schedule_bytes: usize,
    pub max_block_closure_bytes: usize,
    pub max_total_block_closure_bytes: usize,
    pub max_total_closure_leaves: usize,
    pub per_block_closure_limits: DyadicIntervalClosureLimitsV1,
}

/// Inputs for one sealed, non-authorizing all-block closure-set observation.
#[derive(Clone, Copy)]
pub struct CommonArticulationBlockClosureSetInputV2<'a> {
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
    pub limits: CommonArticulationBlockClosureSetLimitsV2,
}

/// Sealed all-block V2 closure evidence.
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationBlockClosureSetV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationBlockClosureSetV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationBlockClosureSetV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationBlockClosureSetV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::{
///     CommonArticulationBlockClosureSetV2, DyadicMaterialHingeIntervalClosureCertificateV1,
/// };
///
/// fn accepts_v1(_: DyadicMaterialHingeIntervalClosureCertificateV1) {}
/// fn reject_v2(value: CommonArticulationBlockClosureSetV2) {
///     accepts_v1(value);
/// }
/// ```
pub struct CommonArticulationBlockClosureSetV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    profile_binding: [u8; 32],
    decomposition_binding: [u8; 32],
    common_pose_binding: [u8; 32],
    audit_binding: [u8; 32],
    parent_schedule_binding: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    closure_tolerance_bits: u64,
    configured_max_blocks: usize,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    limits: CommonArticulationBlockClosureSetLimitsV2,
    total_block_schedule_bytes: usize,
    total_block_closure_bytes: usize,
    total_closure_leaves: usize,
    blocks: Vec<BlockClosureRecordV2>,
    binding_fingerprint: [u8; 32],
}

impl fmt::Debug for CommonArticulationBlockClosureSetV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommonArticulationBlockClosureSetV2")
            .field(
                "model_id",
                &COMMON_ARTICULATION_BLOCK_CLOSURE_SET_MODEL_ID_V2,
            )
            .field("configured_max_blocks", &self.configured_max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .field("profile_binding", &self.profile_binding)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct BlockClosureRecordV2 {
    block_index: usize,
    fixed_face: FaceId,
    geometry_audit_binding: [u8; 32],
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    schedule_bytes: usize,
    closure_bytes: usize,
    closure_leaves: usize,
}

impl CommonArticulationBlockClosureSetV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_BLOCK_CLOSURE_SET_MODEL_ID_V2
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
    pub const fn total_block_schedule_bytes_v2(&self) -> usize {
        self.total_block_schedule_bytes
    }

    #[must_use]
    pub const fn total_block_closure_bytes_v2(&self) -> usize {
        self.total_block_closure_bytes
    }

    #[must_use]
    pub const fn total_closure_leaves_v2(&self) -> usize {
        self.total_closure_leaves
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    /// Live-reissues every restriction and closure before comparing retained
    /// observations. A candidate is never published on stop or mismatch.
    pub fn revalidate_v2(
        &self,
        input: CommonArticulationBlockClosureSetInputV2<'_>,
    ) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationBlockClosureSetInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
    ) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
        let candidate = validation::issue_v2(input, &mut checkpoint)?;
        checkpoint_v2(&mut checkpoint)?;
        if self.issuer_geometry != candidate.issuer_geometry
            || self.profile_binding != candidate.profile_binding
            || self.decomposition_binding != candidate.decomposition_binding
            || self.common_pose_binding != candidate.common_pose_binding
            || self.audit_binding != candidate.audit_binding
            || self.parent_schedule_binding != candidate.parent_schedule_binding
            || self.parent_fixed_face != candidate.parent_fixed_face
            || self.paper_thickness_bits != candidate.paper_thickness_bits
            || self.closure_tolerance_bits != candidate.closure_tolerance_bits
            || self.configured_max_blocks != candidate.configured_max_blocks
            || self.actual_block_count != candidate.actual_block_count
            || self.face_count != candidate.face_count
            || self.hinge_count != candidate.hinge_count
            || self.limits != candidate.limits
            || self.total_block_schedule_bytes != candidate.total_block_schedule_bytes
            || self.total_block_closure_bytes != candidate.total_block_closure_bytes
            || self.total_closure_leaves != candidate.total_closure_leaves
            || self.binding_fingerprint != candidate.binding_fingerprint
            || !validation::records_equal_v2(&self.blocks, &candidate.blocks, &mut checkpoint)?
        {
            return Err(CommonArticulationBlockClosureSetErrorV2::IssuerMismatch);
        }
        checkpoint_v2(&mut checkpoint)
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

/// Issues retained, non-authorizing V2 all-block closure observations.
pub fn prove_common_articulation_block_closure_set_v2(
    input: CommonArticulationBlockClosureSetInputV2<'_>,
) -> Result<CommonArticulationBlockClosureSetV2, CommonArticulationBlockClosureSetErrorV2> {
    validation::issue_v2(input, || Ok(()))
}

/// Controlled issuance with checkpoints at every bounded phase and before
/// publication. Stops never expose a partial set.
pub fn prove_common_articulation_block_closure_set_with_checkpoint_v2(
    input: CommonArticulationBlockClosureSetInputV2<'_>,
    checkpoint: impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<CommonArticulationBlockClosureSetV2, CommonArticulationBlockClosureSetErrorV2> {
    validation::issue_v2(input, checkpoint)
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationBlockClosureSetStopV2::Cancelled => {
            CommonArticulationBlockClosureSetErrorV2::Cancelled
        }
        CommonArticulationBlockClosureSetStopV2::DeadlineExceeded => {
            CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded
        }
    })
}
