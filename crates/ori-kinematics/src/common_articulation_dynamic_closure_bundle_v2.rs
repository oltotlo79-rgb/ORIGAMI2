//! Crate-private, workspace-bounded dynamic closure material for general-N
//! common-articulation consumers.
//!
//! This path is independent of the public legacy block/whole-parent closure
//! observations. It retains owned restricted schedules and the Phase 1 sealed
//! closure material, but exposes only read-only dyadic leaf coordinates. It
//! cannot be converted into V1 closure or pose authority and has no persistence
//! or public authorization surface.

use std::{marker::PhantomData, sync::Arc};

use ori_domain::FaceId;

#[cfg(test)]
use crate::graph::DyadicIntervalClosureWorkspaceResourcesV2;
use crate::graph::{
    DyadicIntervalClosureWorkspaceLimitsV2, WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
};
use crate::schedule::{
    CycleScheduleRestrictionWorkspaceLimitsV2, CycleScheduleRestrictionWorkspaceResourcesV2,
};
use crate::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationPoseAuthorityV2,
    CommonArticulationResourceProfileV2, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    MaterialHingeGraphInstanceV1,
};

mod binding;
mod issue;
mod resources;

#[allow(unused_imports)]
pub(crate) use issue::prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2;

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const BUNDLE_DOMAIN_V2: &[u8] = b"ORIGAMI2_COMMON_ARTICULATION_DYNAMIC_CLOSURE_BUNDLE_V2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommonArticulationDynamicClosureBundleStopV2 {
    Cancelled,
    DeadlineExceeded,
}

// Keep both crate-private callback outcomes part of the compiled boundary,
// independently of which transport caller is linked in a given target.
const _: [CommonArticulationDynamicClosureBundleStopV2; 2] = [
    CommonArticulationDynamicClosureBundleStopV2::Cancelled,
    CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommonArticulationDynamicClosureBundleErrorV2 {
    InvalidInput,
    ResourceLimit,
    IssuerMismatch,
    UnprovenClosure { depth: u32, index: u64 },
    Cancelled,
    DeadlineExceeded,
}

/// Complete resource policy for owned block restrictions, all block proofs,
/// the owned parent schedule/proof, publication, and a simultaneous live
/// revalidation candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommonArticulationDynamicClosureBundleLimitsV2 {
    pub(crate) max_blocks: usize,
    pub(crate) max_validation_work: usize,
    pub(crate) max_block_record_bytes: usize,
    pub(crate) max_total_restriction_work: usize,
    pub(crate) max_total_restricted_schedule_retained_bytes: usize,
    pub(crate) max_total_block_closure_retained_bytes: usize,
    pub(crate) max_total_block_leaves: usize,
    pub(crate) max_parent_schedule_retained_bytes: usize,
    pub(crate) max_parent_closure_retained_bytes: usize,
    pub(crate) max_parent_leaves: usize,
    pub(crate) max_bundle_retained_bytes: usize,
    pub(crate) max_issuance_peak_bytes: usize,
    pub(crate) max_revalidation_peak_bytes: usize,
    pub(crate) block_restriction_limits: CycleScheduleRestrictionWorkspaceLimitsV2,
    pub(crate) parent_schedule_restriction_limits: CycleScheduleRestrictionWorkspaceLimitsV2,
    pub(crate) per_block_closure_limits: DyadicIntervalClosureWorkspaceLimitsV2,
    pub(crate) parent_closure_limits: DyadicIntervalClosureWorkspaceLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommonArticulationDynamicClosureBundleResourcesV2 {
    pub(crate) charged_block_record_bytes: usize,
    pub(crate) charged_validation_work: usize,
    pub(crate) charged_total_restriction_work: usize,
    pub(crate) charged_total_restricted_schedule_retained_upper_bound_bytes: usize,
    pub(crate) charged_total_block_closure_retained_upper_bound_bytes: usize,
    pub(crate) charged_total_block_leaves: usize,
    pub(crate) charged_parent_schedule_retained_upper_bound_bytes: usize,
    pub(crate) charged_parent_closure_retained_upper_bound_bytes: usize,
    pub(crate) charged_parent_leaves: usize,
    pub(crate) charged_max_block_restriction_peak_upper_bound_bytes: usize,
    pub(crate) charged_max_block_closure_peak_upper_bound_bytes: usize,
    pub(crate) charged_parent_schedule_restriction_peak_upper_bound_bytes: usize,
    pub(crate) charged_parent_closure_peak_upper_bound_bytes: usize,
    pub(crate) charged_bundle_retained_upper_bound_bytes: usize,
    pub(crate) charged_issuance_peak_upper_bound_bytes: usize,
    pub(crate) charged_revalidation_peak_upper_bound_bytes: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct CommonArticulationDynamicClosureBundleInputV2<'a> {
    pub(crate) geometry: &'a MaterialHingeGraphGeometry,
    pub(crate) audit: &'a MaterialHingeGraphAudit,
    pub(crate) pose: &'a ClosedMaterialHingeGraphPose,
    pub(crate) parent_fixed_face: FaceId,
    pub(crate) parent_schedule: &'a CanonicalCycleScheduleV1,
    pub(crate) decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub(crate) common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub(crate) paper_thickness_mm: f64,
    pub(crate) closure_tolerance: f64,
    pub(crate) profile: &'a CommonArticulationResourceProfileV2,
    pub(crate) limits: CommonArticulationDynamicClosureBundleLimitsV2,
}

/// The only leaf view available outside this module. It carries no checked
/// hinge list, V1 certificate, pose, transform, or authorization predicate.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CommonArticulationDynamicClosureLeafDescriptorV2<'bundle> {
    depth: u32,
    index: u64,
    bundle_lifetime: PhantomData<&'bundle CommonArticulationDynamicClosureBundleV2>,
}

impl CommonArticulationDynamicClosureLeafDescriptorV2<'_> {
    #[must_use]
    pub(crate) const fn depth(&self) -> u32 {
        self.depth
    }

    #[must_use]
    pub(crate) const fn index(&self) -> u64 {
        self.index
    }
}

// Compile-time API assertions keep the sealed descriptor coordinates available
// to crate consumers even in targets that do not yet have a transport caller.
const _: for<'bundle> fn(&CommonArticulationDynamicClosureLeafDescriptorV2<'bundle>) -> u32 =
    |descriptor| descriptor.depth();
const _: for<'bundle> fn(&CommonArticulationDynamicClosureLeafDescriptorV2<'bundle>) -> u64 =
    |descriptor| descriptor.index();

#[derive(Debug)]
struct DynamicBlockClosureRecordV2 {
    block_index: usize,
    issuer_geometry: MaterialHingeGraphInstanceV1,
    fixed_face: FaceId,
    geometry_audit_binding: [u8; 32],
    restricted_schedule: CanonicalCycleScheduleV1,
    restriction_resources: CycleScheduleRestrictionWorkspaceResourcesV2,
    closure: WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
}

/// Debug-only sealed material. Deliberately no `Clone`, serde, conversion,
/// dereference, V1 certificate accessor, or pose-authority constructor exists.
#[derive(Debug)]
pub(crate) struct CommonArticulationDynamicClosureBundleV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    issuer_pose: Arc<()>,
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
    policy: CommonArticulationDynamicClosureBundleLimitsV2,
    blocks: Vec<DynamicBlockClosureRecordV2>,
    parent_schedule: CanonicalCycleScheduleV1,
    parent_schedule_restriction_resources: CycleScheduleRestrictionWorkspaceResourcesV2,
    parent_closure: WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
    resources: CommonArticulationDynamicClosureBundleResourcesV2,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationDynamicClosureBundleV2 {
    #[must_use]
    pub(crate) const fn resources(&self) -> CommonArticulationDynamicClosureBundleResourcesV2 {
        self.resources
    }

    #[must_use]
    pub(crate) const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub(crate) const fn policy_v2(&self) -> CommonArticulationDynamicClosureBundleLimitsV2 {
        self.policy
    }

    #[must_use]
    pub(crate) const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[cfg(test)]
    pub(crate) fn block_restriction_resources_v2(
        &self,
        block_index: usize,
    ) -> Option<CycleScheduleRestrictionWorkspaceResourcesV2> {
        self.blocks
            .get(block_index)
            .map(|record| record.restriction_resources)
    }

    #[cfg(test)]
    pub(crate) fn block_closure_resources_v2(
        &self,
        block_index: usize,
    ) -> Option<DyadicIntervalClosureWorkspaceResourcesV2> {
        self.blocks
            .get(block_index)
            .map(|record| record.closure.resources())
    }

    #[cfg(test)]
    pub(crate) const fn parent_restriction_resources_v2(
        &self,
    ) -> CycleScheduleRestrictionWorkspaceResourcesV2 {
        self.parent_schedule_restriction_resources
    }

    #[cfg(test)]
    pub(crate) const fn parent_closure_resources_v2(
        &self,
    ) -> DyadicIntervalClosureWorkspaceResourcesV2 {
        self.parent_closure.resources()
    }

    #[must_use]
    pub(crate) fn block_leaf_descriptor(
        &self,
        block_index: usize,
        leaf_index: usize,
    ) -> Option<CommonArticulationDynamicClosureLeafDescriptorV2<'_>> {
        self.blocks
            .get(block_index)?
            .closure
            .partition()
            .get(leaf_index)
            .map(
                |&(depth, index)| CommonArticulationDynamicClosureLeafDescriptorV2 {
                    depth,
                    index,
                    bundle_lifetime: PhantomData,
                },
            )
    }

    #[must_use]
    pub(crate) fn parent_leaf_descriptor(
        &self,
        leaf_index: usize,
    ) -> Option<CommonArticulationDynamicClosureLeafDescriptorV2<'_>> {
        self.parent_closure
            .partition()
            .get(leaf_index)
            .map(
                |&(depth, index)| CommonArticulationDynamicClosureLeafDescriptorV2 {
                    depth,
                    index,
                    bundle_lifetime: PhantomData,
                },
            )
    }

    #[allow(dead_code)] // Phase 3 transport consumes this sealed revalidation seam.
    pub(crate) fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicClosureBundleInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
    ) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        // Retain the owned parent material for the sealed transport surface;
        // this revalidation seam intentionally does not expose it.
        let _ = &self.parent_schedule;
        let retained_offset = self.resources.charged_bundle_retained_upper_bound_bytes;
        let sealed_revalidation_peak = retained_offset
            .checked_add(self.resources.charged_issuance_peak_upper_bound_bytes)
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        if sealed_revalidation_peak != self.resources.charged_revalidation_peak_upper_bound_bytes
            || sealed_revalidation_peak > input.limits.max_revalidation_peak_bytes
        {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
        }
        // Candidate issuance repeats every live-input validation under the
        // sealed validation-work ceiling. Its charged work also reserves the
        // final one-pass block-issuer comparison below, so revalidation never
        // resets or silently exceeds that ceiling.
        let candidate = issue::issue_v2(input, &mut checkpoint, retained_offset).map_err(
            |error| match error {
                CommonArticulationDynamicClosureBundleErrorV2::InvalidInput
                | CommonArticulationDynamicClosureBundleErrorV2::IssuerMismatch => {
                    CommonArticulationDynamicClosureBundleErrorV2::IssuerMismatch
                }
                other => other,
            },
        )?;
        if !binding::bundles_match_with_checkpoint_v2(self, &candidate, &mut checkpoint)? {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::IssuerMismatch);
        }
        checkpoint_v2(&mut checkpoint)
    }
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicClosureBundleStopV2::Cancelled => {
            CommonArticulationDynamicClosureBundleErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_maps_each_stop_variant() {
        let cancelled =
            checkpoint_v2(&mut || Err(CommonArticulationDynamicClosureBundleStopV2::Cancelled));
        assert_eq!(
            cancelled,
            Err(CommonArticulationDynamicClosureBundleErrorV2::Cancelled)
        );

        let deadline_exceeded = checkpoint_v2(&mut || {
            Err(CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded)
        });
        assert_eq!(
            deadline_exceeded,
            Err(CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded)
        );
    }

    #[test]
    fn resource_accessor_test_seams_remain_callable() {
        let _ = CommonArticulationDynamicClosureBundleV2::block_restriction_resources_v2;
        let _ = CommonArticulationDynamicClosureBundleV2::block_closure_resources_v2;
        let _ = CommonArticulationDynamicClosureBundleV2::parent_restriction_resources_v2;
        let _ = CommonArticulationDynamicClosureBundleV2::parent_closure_resources_v2;
    }
}
