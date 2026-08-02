use std::mem::size_of;

use crate::CanonicalCycleScheduleV1;
use crate::graph::{
    DyadicIntervalClosureWorkspaceResourcesV2, WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
};
use crate::schedule::CycleScheduleRestrictionWorkspaceResourcesV2;

use super::*;

fn closure_limits_contain_max_v2(limits: DyadicIntervalClosureWorkspaceLimitsV2) -> bool {
    [
        limits.max_leaves,
        limits.max_work,
        limits.schedule_limits.max_hinges,
        limits.schedule_limits.max_degree,
        limits.schedule_limits.max_work,
        limits.max_theorem_recognizer_work,
        limits.max_theorem_recognizer_workspace_bytes,
        limits.max_carrier_index_workspace_bytes,
        limits.max_schedule_evaluation_workspace_bytes,
        limits.max_big_rational_payload_bytes,
        limits.max_exact_rational_object_bytes,
        limits.max_interval_closure_workspace_bytes,
        limits.max_partition_workspace_bytes,
        limits.max_retained_material_bytes,
        limits.max_publication_workspace_bytes,
        limits.max_peak_workspace_bytes,
    ]
    .contains(&usize::MAX)
}

pub(super) fn invalid_resource_policy_v2(
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> bool {
    [
        limits.max_blocks,
        limits.max_validation_work,
        limits.max_block_record_bytes,
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
        limits.block_restriction_limits.max_work,
        limits
            .block_restriction_limits
            .max_restricted_schedule_retained_bytes,
        limits.block_restriction_limits.max_restriction_peak_bytes,
        limits.parent_schedule_restriction_limits.max_work,
        limits
            .parent_schedule_restriction_limits
            .max_restricted_schedule_retained_bytes,
        limits
            .parent_schedule_restriction_limits
            .max_restriction_peak_bytes,
    ]
    .iter()
    .any(|value| *value == 0 || *value == usize::MAX)
        || closure_limits_contain_max_v2(limits.per_block_closure_limits)
        || closure_limits_contain_max_v2(limits.parent_closure_limits)
        || limits.per_block_closure_limits.max_leaves == 0
        || limits.per_block_closure_limits.max_work == 0
        || limits.per_block_closure_limits.schedule_limits.max_hinges == 0
        || limits.per_block_closure_limits.schedule_limits.max_work == 0
        || limits
            .per_block_closure_limits
            .schedule_limits
            .max_coefficient_bits
            == u32::MAX
        || limits.parent_closure_limits.max_leaves == 0
        || limits.parent_closure_limits.max_work == 0
        || limits.parent_closure_limits.schedule_limits.max_hinges == 0
        || limits.parent_closure_limits.schedule_limits.max_work == 0
        || limits
            .parent_closure_limits
            .schedule_limits
            .max_coefficient_bits
            == u32::MAX
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BundleValidationMeterV2 {
    pub(super) work: usize,
    max_work: usize,
}

impl BundleValidationMeterV2 {
    pub(super) fn new(max_work: usize) -> Self {
        Self { work: 0, max_work }
    }

    pub(super) fn charge(
        &mut self,
        amount: usize,
    ) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
        self.work = self
            .work
            .checked_add(amount)
            .filter(|work| *work <= self.max_work)
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        Ok(())
    }

    pub(super) fn poll(
        &mut self,
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
    ) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
        checkpoint_v2(checkpoint)?;
        self.charge(1)
    }
}

pub(super) fn checked_schedule_nested_upper_bound_v2(
    resources: CycleScheduleRestrictionWorkspaceResourcesV2,
) -> Option<usize> {
    resources
        .charged_restricted_schedule_retained_upper_bound_bytes
        .checked_sub(size_of::<CanonicalCycleScheduleV1>())
}

pub(super) fn checked_closure_nested_upper_bound_v2(
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
) -> Option<usize> {
    resources
        .charged_retained_material_upper_bound_bytes
        .checked_sub(size_of::<
            WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
        >())
}

pub(super) fn resources_fit_limits_v2(
    resources: CommonArticulationDynamicClosureBundleResourcesV2,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> bool {
    resources.charged_block_record_bytes <= limits.max_block_record_bytes
        && resources.charged_validation_work <= limits.max_validation_work
        && resources.charged_total_restriction_work <= limits.max_total_restriction_work
        && resources.charged_total_restricted_schedule_retained_upper_bound_bytes
            <= limits.max_total_restricted_schedule_retained_bytes
        && resources.charged_total_block_closure_retained_upper_bound_bytes
            <= limits.max_total_block_closure_retained_bytes
        && resources.charged_total_block_leaves <= limits.max_total_block_leaves
        && resources.charged_parent_schedule_retained_upper_bound_bytes
            <= limits.max_parent_schedule_retained_bytes
        && resources.charged_parent_closure_retained_upper_bound_bytes
            <= limits.max_parent_closure_retained_bytes
        && resources.charged_parent_leaves <= limits.max_parent_leaves
        && resources.charged_bundle_retained_upper_bound_bytes <= limits.max_bundle_retained_bytes
        && resources.charged_issuance_peak_upper_bound_bytes <= limits.max_issuance_peak_bytes
        && resources.charged_revalidation_peak_upper_bound_bytes
            <= limits.max_revalidation_peak_bytes
}

pub(super) fn checked_container_bytes_v2(records_capacity: usize) -> Option<usize> {
    size_of::<CommonArticulationDynamicClosureBundleV2>()
        .checked_add(size_of::<DynamicBlockClosureRecordV2>().checked_mul(records_capacity)?)
}

pub(super) fn checked_bundle_retained_upper_bound_v2(
    records_capacity: usize,
    accumulated_block_nested: usize,
    parent_schedule_resources: CycleScheduleRestrictionWorkspaceResourcesV2,
    parent_closure_resources: DyadicIntervalClosureWorkspaceResourcesV2,
) -> Option<usize> {
    let mut retained =
        checked_container_bytes_v2(records_capacity)?.checked_add(accumulated_block_nested)?;
    retained = retained
        .checked_add(checked_schedule_nested_upper_bound_v2(
            parent_schedule_resources,
        )?)?
        .checked_add(checked_closure_nested_upper_bound_v2(
            parent_closure_resources,
        )?)?;
    Some(retained)
}

pub(super) fn empty_resources_v2(
    block_record_bytes: usize,
) -> CommonArticulationDynamicClosureBundleResourcesV2 {
    CommonArticulationDynamicClosureBundleResourcesV2 {
        charged_block_record_bytes: block_record_bytes,
        charged_validation_work: 0,
        charged_total_restriction_work: 0,
        charged_total_restricted_schedule_retained_upper_bound_bytes: 0,
        charged_total_block_closure_retained_upper_bound_bytes: 0,
        charged_total_block_leaves: 0,
        charged_parent_schedule_retained_upper_bound_bytes: 0,
        charged_parent_closure_retained_upper_bound_bytes: 0,
        charged_parent_leaves: 0,
        charged_max_block_restriction_peak_upper_bound_bytes: 0,
        charged_max_block_closure_peak_upper_bound_bytes: 0,
        charged_parent_schedule_restriction_peak_upper_bound_bytes: 0,
        charged_parent_closure_peak_upper_bound_bytes: 0,
        charged_bundle_retained_upper_bound_bytes: 0,
        charged_issuance_peak_upper_bound_bytes: 0,
        charged_revalidation_peak_upper_bound_bytes: 0,
    }
}

pub(super) fn add_total(
    value: &mut usize,
    amount: usize,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    *value = value
        .checked_add(amount)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    Ok(())
}

pub(super) fn observe_issuance_peak_v2(
    resources: &mut CommonArticulationDynamicClosureBundleResourcesV2,
    candidate: usize,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
    retained_revalidation_offset: usize,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    resources.charged_issuance_peak_upper_bound_bytes = resources
        .charged_issuance_peak_upper_bound_bytes
        .max(candidate);
    if resources.charged_issuance_peak_upper_bound_bytes > limits.max_issuance_peak_bytes {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    ensure_revalidation_phase_fits_v2(candidate, retained_revalidation_offset, limits)?;
    Ok(())
}

pub(super) fn ensure_revalidation_phase_fits_v2(
    candidate_issuance_phase: usize,
    retained_revalidation_offset: usize,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    if candidate_issuance_phase > limits.max_issuance_peak_bytes
        || (retained_revalidation_offset != 0
            && retained_revalidation_offset
                .checked_add(candidate_issuance_phase)
                .is_none_or(|peak| peak > limits.max_revalidation_peak_bytes))
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    Ok(())
}

pub(super) fn remaining_issuance_phase_bytes_v2(
    outer_live_bytes: usize,
    retained_revalidation_offset: usize,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> Result<usize, CommonArticulationDynamicClosureBundleErrorV2> {
    let mut ceiling = limits.max_issuance_peak_bytes;
    if retained_revalidation_offset != 0 {
        ceiling = ceiling.min(
            limits
                .max_revalidation_peak_bytes
                .checked_sub(retained_revalidation_offset)
                .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
        );
    }
    ceiling
        .checked_sub(outer_live_bytes)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)
}

pub(super) fn schedule_retained_cap_from_bundle_remaining_v2(
    outer_live_bytes: usize,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> Result<usize, CommonArticulationDynamicClosureBundleErrorV2> {
    limits
        .max_bundle_retained_bytes
        .checked_sub(outer_live_bytes)
        .and_then(|nested| nested.checked_add(size_of::<CanonicalCycleScheduleV1>()))
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)
}

pub(super) fn closure_retained_cap_from_bundle_remaining_v2(
    outer_live_bytes: usize,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> Result<usize, CommonArticulationDynamicClosureBundleErrorV2> {
    limits
        .max_bundle_retained_bytes
        .checked_sub(outer_live_bytes)
        .and_then(|nested| {
            nested.checked_add(size_of::<
                WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
            >())
        })
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)
}
