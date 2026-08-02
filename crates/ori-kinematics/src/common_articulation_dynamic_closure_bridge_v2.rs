//! Opaque cross-crate transport for the private dynamic common-articulation
//! closure bundle.
//!
//! The bridge owns the bundle but intentionally exposes only its binding and
//! aggregate resource charges.  In particular, it is not a closure
//! certificate, pose authority, or motion authorization.
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureBridgeV2;
//!
//! let _ = CommonArticulationDynamicClosureBridgeV2 {};
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureBridgeV2;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<CommonArticulationDynamicClosureBridgeV2>();
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureBridgeV2;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<CommonArticulationDynamicClosureBridgeV2>();
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::CommonArticulationDynamicClosureBridgeV2;
//!
//! fn require_deref<T: std::ops::Deref>() {}
//! require_deref::<CommonArticulationDynamicClosureBridgeV2>();
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::{
//!     CommonArticulationDynamicClosureBridgeV2, CommonArticulationPoseAuthorityV1,
//! };
//!
//! fn requires_v1_authority(_: CommonArticulationPoseAuthorityV1) {}
//! fn rejects_bridge(value: CommonArticulationDynamicClosureBridgeV2) {
//!     requires_v1_authority(value);
//! }
//! ```

use ori_domain::FaceId;
use thiserror::Error;

use crate::common_articulation_dynamic_closure_bundle_v2::{
    CommonArticulationDynamicClosureBundleErrorV2, CommonArticulationDynamicClosureBundleInputV2,
    CommonArticulationDynamicClosureBundleLimitsV2, CommonArticulationDynamicClosureBundleStopV2,
    CommonArticulationDynamicClosureBundleV2,
    prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2,
};
use crate::graph::DyadicIntervalClosureWorkspaceLimitsV2;
use crate::schedule::CycleScheduleRestrictionWorkspaceLimitsV2;
use crate::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationPoseAuthorityV2,
    CommonArticulationResourceProfileV2, CycleScheduleLimitsV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry,
};

/// Cooperative stop requested while issuing or replaying an opaque bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicClosureBridgeStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Fail-closed error from opaque dynamic-closure bridge issuance or replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicClosureBridgeErrorV2 {
    #[error("the dynamic closure bridge input is malformed")]
    InvalidInput,
    #[error("the dynamic closure bridge exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the retained dynamic closure bridge does not match its live issuer")]
    IssuerMismatch,
    #[error("the dynamic closure bundle has no proof for dyadic leaf ({depth}, {index})")]
    UnprovenClosure { depth: u32, index: u64 },
    #[error("the dynamic closure bridge operation was cancelled")]
    Cancelled,
    #[error("the dynamic closure bridge operation deadline elapsed")]
    DeadlineExceeded,
}

/// Public, finite aggregate limits for the private dynamic closure bundle.
///
/// The implementation derives the private restriction and dyadic workspace
/// ceilings from these values.  This keeps cross-crate callers from naming
/// the retained schedules, closures, or their internal workspace types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicClosureBridgeLimitsV2 {
    pub max_blocks: usize,
    pub max_validation_work: usize,
    pub max_total_restriction_work: usize,
    pub max_total_restricted_schedule_retained_bytes: usize,
    pub max_total_block_closure_retained_bytes: usize,
    pub max_total_block_leaves: usize,
    pub max_parent_schedule_retained_bytes: usize,
    pub max_parent_closure_retained_bytes: usize,
    pub max_parent_leaves: usize,
    pub max_bundle_retained_bytes: usize,
    pub max_issuance_peak_bytes: usize,
    pub max_revalidation_peak_bytes: usize,
    pub max_schedule_degree: usize,
    pub max_schedule_coefficient_bits: u32,
    pub max_dyadic_depth: u32,
    pub max_dyadic_leaves_per_closure: usize,
    pub max_dyadic_work_per_closure: usize,
}

/// Exact live inputs for one opaque dynamic-closure bridge issuance.
#[derive(Clone, Copy)]
pub struct CommonArticulationDynamicClosureBridgeInputV2<'a> {
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
    pub limits: CommonArticulationDynamicClosureBridgeLimitsV2,
}

/// Exact live inputs required to replay an opaque dynamic-closure bridge.
///
/// The resource policy remains sealed in the bridge so replay cannot replace
/// it with a more permissive workspace envelope.
#[derive(Clone, Copy)]
pub struct CommonArticulationDynamicClosureBridgeRevalidationInputV2<'a> {
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
}

/// Opaque, non-authorizing owner of a workspace-bounded dynamic closure
/// bundle.  It deliberately implements only `Debug`.
#[derive(Debug)]
#[repr(transparent)]
pub struct CommonArticulationDynamicClosureBridgeV2 {
    bundle: CommonArticulationDynamicClosureBundleV2,
}

// The public transport must add no uncharged retained state around the bundle
// shell that its resource ledger already includes.
const _: [(); std::mem::size_of::<CommonArticulationDynamicClosureBundleV2>()] =
    [(); std::mem::size_of::<CommonArticulationDynamicClosureBridgeV2>()];
const _: [(); std::mem::align_of::<CommonArticulationDynamicClosureBundleV2>()] =
    [(); std::mem::align_of::<CommonArticulationDynamicClosureBridgeV2>()];

impl CommonArticulationDynamicClosureBridgeV2 {
    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.bundle.binding_fingerprint_v2()
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.bundle.actual_block_count_v2()
    }

    #[must_use]
    pub const fn retained_bytes_upper_bound_v2(&self) -> usize {
        self.bundle
            .resources()
            .charged_bundle_retained_upper_bound_bytes
    }

    #[must_use]
    pub const fn issuance_peak_bytes_upper_bound_v2(&self) -> usize {
        self.bundle
            .resources()
            .charged_issuance_peak_upper_bound_bytes
    }

    #[must_use]
    pub const fn revalidation_peak_bytes_upper_bound_v2(&self) -> usize {
        self.bundle
            .resources()
            .charged_revalidation_peak_upper_bound_bytes
    }

    /// Replays the complete private bundle against the exact live tuple.
    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationDynamicClosureBridgeErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    /// As [`Self::revalidate_v2`], with cooperative cancellation and deadline
    /// checkpoints.
    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
    ) -> Result<(), CommonArticulationDynamicClosureBridgeErrorV2> {
        checkpoint_bridge_v2(&mut checkpoint)?;
        let limits = self.bundle.policy_v2();
        self.bundle
            .revalidate_with_checkpoint_v2(private_revalidation_input_v2(input, limits), || {
                checkpoint().map_err(map_bridge_stop_to_bundle_stop_v2)
            })
            .map_err(map_bundle_error_v2)
    }
}

/// Issues an opaque dynamic-closure bridge for the exact live input.
pub fn prove_common_articulation_dynamic_closure_bridge_v2(
    input: CommonArticulationDynamicClosureBridgeInputV2<'_>,
) -> Result<CommonArticulationDynamicClosureBridgeV2, CommonArticulationDynamicClosureBridgeErrorV2>
{
    prove_common_articulation_dynamic_closure_bridge_with_checkpoint_v2(input, || Ok(()))
}

/// As [`prove_common_articulation_dynamic_closure_bridge_v2`], with
/// cooperative cancellation and deadline checkpoints.
pub fn prove_common_articulation_dynamic_closure_bridge_with_checkpoint_v2(
    input: CommonArticulationDynamicClosureBridgeInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
) -> Result<CommonArticulationDynamicClosureBridgeV2, CommonArticulationDynamicClosureBridgeErrorV2>
{
    checkpoint_bridge_v2(&mut checkpoint)?;
    let limits = private_limits_v2(input.geometry, input.limits)?;
    let bundle = prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
        CommonArticulationDynamicClosureBundleInputV2 {
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
            limits,
        },
        || checkpoint().map_err(map_bridge_stop_to_bundle_stop_v2),
    )
    .map_err(map_bundle_error_v2)?;
    Ok(CommonArticulationDynamicClosureBridgeV2 { bundle })
}

fn private_revalidation_input_v2(
    input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'_>,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> CommonArticulationDynamicClosureBundleInputV2<'_> {
    CommonArticulationDynamicClosureBundleInputV2 {
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
        limits,
    }
}

fn private_limits_v2(
    geometry: &MaterialHingeGraphGeometry,
    limits: CommonArticulationDynamicClosureBridgeLimitsV2,
) -> Result<
    CommonArticulationDynamicClosureBundleLimitsV2,
    CommonArticulationDynamicClosureBridgeErrorV2,
> {
    validate_public_limits_v2(limits)?;
    let hinge_count = geometry.hinges().len();
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: hinge_count,
        max_degree: limits.max_schedule_degree,
        max_coefficient_bits: limits.max_schedule_coefficient_bits,
        max_work: limits.max_dyadic_work_per_closure,
    };
    let block_restriction_limits = CycleScheduleRestrictionWorkspaceLimitsV2 {
        max_work: limits.max_total_restriction_work,
        max_restricted_schedule_retained_bytes: limits.max_total_restricted_schedule_retained_bytes,
        max_restriction_peak_bytes: limits.max_issuance_peak_bytes,
    };
    let parent_schedule_restriction_limits = CycleScheduleRestrictionWorkspaceLimitsV2 {
        max_work: limits.max_total_restriction_work,
        max_restricted_schedule_retained_bytes: limits.max_parent_schedule_retained_bytes,
        max_restriction_peak_bytes: limits.max_issuance_peak_bytes,
    };
    let per_block_closure_limits = closure_limits_v2(
        schedule_limits,
        limits.max_dyadic_depth,
        limits
            .max_dyadic_leaves_per_closure
            .min(limits.max_total_block_leaves),
        limits.max_dyadic_work_per_closure,
        limits
            .max_total_block_closure_retained_bytes
            .min(limits.max_bundle_retained_bytes),
        limits.max_issuance_peak_bytes,
    );
    let parent_closure_limits = closure_limits_v2(
        schedule_limits,
        limits.max_dyadic_depth,
        limits
            .max_dyadic_leaves_per_closure
            .min(limits.max_parent_leaves),
        limits.max_dyadic_work_per_closure,
        limits
            .max_parent_closure_retained_bytes
            .min(limits.max_bundle_retained_bytes),
        limits.max_issuance_peak_bytes,
    );
    if per_block_closure_limits.max_leaves == 0 || parent_closure_limits.max_leaves == 0 {
        return Err(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit);
    }
    Ok(CommonArticulationDynamicClosureBundleLimitsV2 {
        max_blocks: limits.max_blocks,
        max_validation_work: limits.max_validation_work,
        max_block_record_bytes: limits.max_bundle_retained_bytes,
        max_total_restriction_work: limits.max_total_restriction_work,
        max_total_restricted_schedule_retained_bytes: limits
            .max_total_restricted_schedule_retained_bytes,
        max_total_block_closure_retained_bytes: limits.max_total_block_closure_retained_bytes,
        max_total_block_leaves: limits.max_total_block_leaves,
        max_parent_schedule_retained_bytes: limits.max_parent_schedule_retained_bytes,
        max_parent_closure_retained_bytes: limits.max_parent_closure_retained_bytes,
        max_parent_leaves: limits.max_parent_leaves,
        max_bundle_retained_bytes: limits.max_bundle_retained_bytes,
        max_issuance_peak_bytes: limits.max_issuance_peak_bytes,
        max_revalidation_peak_bytes: limits.max_revalidation_peak_bytes,
        block_restriction_limits,
        parent_schedule_restriction_limits,
        per_block_closure_limits,
        parent_closure_limits,
    })
}

fn closure_limits_v2(
    schedule_limits: CycleScheduleLimitsV1,
    max_depth: u32,
    max_leaves: usize,
    max_work: usize,
    max_retained_material_bytes: usize,
    max_peak_workspace_bytes: usize,
) -> DyadicIntervalClosureWorkspaceLimitsV2 {
    DyadicIntervalClosureWorkspaceLimitsV2 {
        max_depth,
        max_leaves,
        max_work,
        schedule_limits,
        max_theorem_recognizer_work: max_work,
        max_theorem_recognizer_workspace_bytes: max_peak_workspace_bytes,
        max_carrier_index_workspace_bytes: max_peak_workspace_bytes,
        max_schedule_evaluation_workspace_bytes: max_peak_workspace_bytes,
        max_big_rational_payload_bytes: max_peak_workspace_bytes,
        max_exact_rational_object_bytes: max_peak_workspace_bytes,
        max_interval_closure_workspace_bytes: max_peak_workspace_bytes,
        max_partition_workspace_bytes: max_peak_workspace_bytes,
        max_retained_material_bytes,
        max_publication_workspace_bytes: max_peak_workspace_bytes,
        max_peak_workspace_bytes,
    }
}

fn validate_public_limits_v2(
    limits: CommonArticulationDynamicClosureBridgeLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureBridgeErrorV2> {
    let finite_nonzero = [
        limits.max_blocks,
        limits.max_validation_work,
        limits.max_total_restriction_work,
        limits.max_total_restricted_schedule_retained_bytes,
        limits.max_total_block_closure_retained_bytes,
        limits.max_total_block_leaves,
        limits.max_parent_schedule_retained_bytes,
        limits.max_parent_closure_retained_bytes,
        limits.max_parent_leaves,
        limits.max_bundle_retained_bytes,
        limits.max_issuance_peak_bytes,
        limits.max_revalidation_peak_bytes,
        limits.max_dyadic_leaves_per_closure,
        limits.max_dyadic_work_per_closure,
    ];
    if finite_nonzero
        .iter()
        .any(|value| *value == 0 || *value == usize::MAX)
        || limits.max_schedule_degree == 0
        || limits.max_schedule_degree == usize::MAX
        || limits.max_schedule_coefficient_bits == 0
        || limits.max_schedule_coefficient_bits == u32::MAX
        || limits.max_dyadic_depth >= 64
    {
        return Err(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit);
    }
    let required_revalidation_peak = limits
        .max_bundle_retained_bytes
        .checked_add(limits.max_issuance_peak_bytes)
        .ok_or(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit)?;
    if required_revalidation_peak > limits.max_revalidation_peak_bytes {
        return Err(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit);
    }
    Ok(())
}

fn checkpoint_bridge_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
) -> Result<(), CommonArticulationDynamicClosureBridgeErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
            CommonArticulationDynamicClosureBridgeErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded
        }
    })
}

fn map_bridge_stop_to_bundle_stop_v2(
    stop: CommonArticulationDynamicClosureBridgeStopV2,
) -> CommonArticulationDynamicClosureBundleStopV2 {
    match stop {
        CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
            CommonArticulationDynamicClosureBundleStopV2::Cancelled
        }
        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded
        }
    }
}

fn map_bundle_error_v2(
    error: CommonArticulationDynamicClosureBundleErrorV2,
) -> CommonArticulationDynamicClosureBridgeErrorV2 {
    match error {
        CommonArticulationDynamicClosureBundleErrorV2::InvalidInput => {
            CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput
        }
        CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit => {
            CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit
        }
        CommonArticulationDynamicClosureBundleErrorV2::IssuerMismatch => {
            CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch
        }
        CommonArticulationDynamicClosureBundleErrorV2::UnprovenClosure { depth, index } => {
            CommonArticulationDynamicClosureBridgeErrorV2::UnprovenClosure { depth, index }
        }
        CommonArticulationDynamicClosureBundleErrorV2::Cancelled => {
            CommonArticulationDynamicClosureBridgeErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded
        }
    }
}
