//! Private parent-partition containment checks.

use super::*;

fn compare_dyadic_left_endpoints_v2(
    left_depth: u32,
    left_index: u64,
    right_depth: u32,
    right_index: u64,
) -> std::cmp::Ordering {
    let common_depth = left_depth.max(right_depth);
    let left = u128::from(left_index) << (common_depth - left_depth);
    let right = u128::from(right_index) << (common_depth - right_depth);
    left.cmp(&right)
}

pub(super) fn parent_partition_covers_leaf_v2(
    bridge: &CommonArticulationDynamicClosureBridgeV2,
    depth: u32,
    index: u64,
    max_comparisons: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBridgeStopV2>,
) -> Result<bool, CommonArticulationDynamicClosureIntervalTransformLeafErrorV2> {
    checkpoint_leaf_v2(checkpoint)?;
    if depth >= 64 || index >= (1_u64 << depth) {
        return Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput);
    }
    let count = bridge.parent_partition_leaf_count_v2();
    let mut lower = 0usize;
    let mut upper = count;
    let mut comparisons = 0usize;
    while lower < upper {
        checkpoint_leaf_v2(checkpoint)?;
        comparisons = comparisons
            .checked_add(1)
            .filter(|value| *value <= max_comparisons)
            .ok_or(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit)?;
        let middle = lower + (upper - lower) / 2;
        let (parent_depth, parent_index) = bridge
            .parent_partition_leaf_coordinates_v2(middle)
            .ok_or(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput)?;
        if compare_dyadic_left_endpoints_v2(parent_depth, parent_index, depth, index)
            != std::cmp::Ordering::Greater
        {
            lower = middle.checked_add(1).ok_or(
                CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit,
            )?;
        } else {
            upper = middle;
        }
    }
    let Some(candidate) = lower.checked_sub(1) else {
        checkpoint_leaf_v2(checkpoint)?;
        return Ok(false);
    };
    let (parent_depth, parent_index) = bridge
        .parent_partition_leaf_coordinates_v2(candidate)
        .ok_or(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput)?;
    let covered = depth >= parent_depth && (index >> (depth - parent_depth)) == parent_index;
    checkpoint_leaf_v2(checkpoint)?;
    Ok(covered)
}
