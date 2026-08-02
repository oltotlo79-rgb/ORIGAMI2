//! Exact retained and per-leaf resource ledgers for sealed sessions.

use std::mem::size_of;

use super::*;

/// Exact proof-carrier and session-publication inventory for one sealed
/// interval-transform session.
///
/// The bridge and parent schedule are the retained proof carriers replayed by
/// this session. Caller-owned immutable issuer models (geometry, audit, pose,
/// decomposition, common-pose authority and resource profile) are explicitly
/// outside this ledger and remain governed by their own issuer/input caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2 {
    pub(super) bridge_retained_bytes: usize,
    pub(super) bridge_revalidation_peak_bytes: usize,
    pub(super) schedule_retained_bytes: usize,
    pub(super) session_shell_bytes: usize,
    pub(super) steady_retained_bytes: usize,
    pub(super) revalidation_phase_peak_bytes: usize,
    pub(super) coverage_search_comparison_upper_bound: usize,
    pub(super) interval_registry_shell_bytes: usize,
    pub(super) leaf_wrapper_overhead_bytes: usize,
}

impl CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2 {
    #[must_use]
    pub const fn bridge_retained_bytes(self) -> usize {
        self.bridge_retained_bytes
    }

    #[must_use]
    pub const fn bridge_revalidation_peak_bytes(self) -> usize {
        self.bridge_revalidation_peak_bytes
    }

    #[must_use]
    pub const fn schedule_retained_bytes(self) -> usize {
        self.schedule_retained_bytes
    }

    #[must_use]
    pub const fn session_shell_bytes(self) -> usize {
        self.session_shell_bytes
    }

    /// Bridge and schedule proof-carrier backing plus the borrowed session
    /// shell while the session is live. Immutable issuer base models are not
    /// included.
    #[must_use]
    pub const fn steady_retained_bytes(self) -> usize {
        self.steady_retained_bytes
    }

    /// Revalidation peak with borrowed schedule backing and the returned
    /// session shell charged once. The bridge's own retained backing is
    /// already included in its revalidation peak.
    #[must_use]
    pub const fn revalidation_phase_peak_bytes(self) -> usize {
        self.revalidation_phase_peak_bytes
    }

    #[must_use]
    pub const fn coverage_search_comparison_upper_bound(self) -> usize {
        self.coverage_search_comparison_upper_bound
    }

    #[must_use]
    pub const fn interval_registry_shell_bytes(self) -> usize {
        self.interval_registry_shell_bytes
    }

    #[must_use]
    pub const fn leaf_wrapper_overhead_bytes(self) -> usize {
        self.leaf_wrapper_overhead_bytes
    }
}

/// Exact per-leaf allocation inventory after an internal schedule evaluation
/// and spanning-tree interval traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicClosureIntervalTransformLeafResourcesV2 {
    pub(super) schedule_workspace_upper_bound_bytes: usize,
    pub(super) angle_box_capacity_bytes: usize,
    pub(super) registry_resources: IntervalFaceTransformWorkspaceResourcesV2,
    pub(super) leaf_wrapper_overhead_bytes: usize,
    pub(super) retained_leaf_bytes: usize,
    pub(super) leaf_phase_peak_bytes: usize,
}

impl CommonArticulationDynamicClosureIntervalTransformLeafResourcesV2 {
    #[must_use]
    pub const fn schedule_workspace_upper_bound_bytes(self) -> usize {
        self.schedule_workspace_upper_bound_bytes
    }

    #[must_use]
    pub const fn angle_box_capacity_bytes(self) -> usize {
        self.angle_box_capacity_bytes
    }

    #[must_use]
    pub const fn registry_resources(self) -> IntervalFaceTransformWorkspaceResourcesV2 {
        self.registry_resources
    }

    #[must_use]
    pub const fn leaf_wrapper_overhead_bytes(self) -> usize {
        self.leaf_wrapper_overhead_bytes
    }

    #[must_use]
    pub const fn retained_leaf_bytes(self) -> usize {
        self.retained_leaf_bytes
    }

    #[must_use]
    pub const fn leaf_phase_peak_bytes(self) -> usize {
        self.leaf_phase_peak_bytes
    }
}

pub(super) fn checked_session_resources_with_checkpoint_v2(
    bridge: &CommonArticulationDynamicClosureBridgeV2,
    schedule: &CanonicalCycleScheduleV1,
    max_schedule_retained_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
) -> Result<
    CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
    CommonArticulationDynamicClosureBridgeErrorV2,
> {
    let schedule_retained_bytes = schedule
        .checked_deep_retained_bytes_with_checkpoint_v2(max_schedule_retained_bytes, || {
            checkpoint().map_err(|stop| match stop {
                CommonArticulationDynamicClosureBridgeStopV2::Cancelled => {
                    CycleScheduleDyadicEvaluationStopV2::Cancelled
                }
                CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded => {
                    CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                }
            })
        })
        .map_err(map_schedule_resource_error_v2)?;
    let session_shell_bytes =
        size_of::<CommonArticulationDynamicClosureIntervalTransformSessionV2<'static>>();
    let bridge_retained_bytes = bridge.retained_bytes_upper_bound_v2();
    let bridge_revalidation_peak_bytes = bridge.revalidation_peak_bytes_upper_bound_v2();
    let steady_retained_bytes = bridge_retained_bytes
        .checked_add(schedule_retained_bytes)
        .and_then(|value| value.checked_add(session_shell_bytes))
        .ok_or(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit)?;
    let revalidation_phase_peak_bytes = bridge_revalidation_peak_bytes
        .checked_add(schedule_retained_bytes)
        .and_then(|value| value.checked_add(session_shell_bytes))
        .ok_or(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit)?;
    let parent_leaf_count = bridge.parent_partition_leaf_count_v2();
    if parent_leaf_count == 0 {
        return Err(CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput);
    }
    let coverage_search_comparison_upper_bound =
        binary_search_comparisons_upper_bound_v2(parent_leaf_count)
            .ok_or(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit)?;
    let interval_registry_shell_bytes =
        size_of::<WorkspaceBoundedMaterialFaceTransformRegistryV2>();
    let leaf_wrapper_overhead_bytes =
        size_of::<CommonArticulationDynamicClosureIntervalTransformLeafV2<'static>>()
            .checked_sub(interval_registry_shell_bytes)
            .ok_or(CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit)?;
    Ok(
        CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2 {
            bridge_retained_bytes,
            bridge_revalidation_peak_bytes,
            schedule_retained_bytes,
            session_shell_bytes,
            steady_retained_bytes,
            revalidation_phase_peak_bytes,
            coverage_search_comparison_upper_bound,
            interval_registry_shell_bytes,
            leaf_wrapper_overhead_bytes,
        },
    )
}

fn binary_search_comparisons_upper_bound_v2(count: usize) -> Option<usize> {
    if count == 0 {
        return Some(0);
    }
    let bit_length = usize::BITS as usize - count.leading_zeros() as usize;
    bit_length.checked_add(1)
}
