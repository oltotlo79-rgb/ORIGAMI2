//! Adaptive dyadic proof over complete relieved prism carriers.

use super::*;

pub(super) fn prove_relief_partition_v2(
    input: &ReliefAggregateInputV2<'_>,
    validated: &mut ValidatedReliefV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<[u8; 32], ReliefAggregateErrorV2> {
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(input.limits.max_collision_leaves)
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
    if pending.capacity() > input.limits.max_collision_leaves {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    pending.push(DyadicLeafV2 { depth: 0, index: 0 });
    let mut live_leaves = 1usize;
    let mut hash = Sha256::new();
    hash.update(b"origami2/dynamic-general-n/shared-relief-partition/v2");
    while let Some(leaf) = pending.pop() {
        relief_checkpoint_v2(checkpoint)?;
        validated.resources.processed_interval_nodes = validated
            .resources
            .processed_interval_nodes
            .checked_add(1)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
        match prove_leaf_v2(input, validated, leaf, checkpoint)? {
            true => {
                validated.resources.accepted_interval_leaves = validated
                    .resources
                    .accepted_interval_leaves
                    .checked_add(1)
                    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                resources::charge_v2(
                    &mut validated.resources.hash_work,
                    2,
                    input.limits.max_hash_work,
                )?;
                hash.update(leaf.depth.to_le_bytes());
                hash.update(leaf.index.to_le_bytes());
            }
            false => {
                if leaf.depth >= input.limits.max_collision_depth
                    || live_leaves >= input.limits.max_collision_leaves
                {
                    return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
                }
                let depth = leaf
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth < 64)
                    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                let left = leaf
                    .index
                    .checked_mul(2)
                    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                let right = left
                    .checked_add(1)
                    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                live_leaves = live_leaves
                    .checked_add(1)
                    .filter(|value| *value <= input.limits.max_collision_leaves)
                    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                pending.push(DyadicLeafV2 {
                    depth,
                    index: right,
                });
                pending.push(DyadicLeafV2 { depth, index: left });
            }
        }
    }
    if validated.resources.accepted_interval_leaves != live_leaves {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    if validated.resources.certified_shared_pair_leaf_count
        != live_leaves
            .checked_mul(validated.resources.shared_pairs)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?
    {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    update_usize_v2(&mut hash, live_leaves).map_err(map_ordinary_error_v2)?;
    update_usize_v2(&mut hash, validated.resources.shared_pair_node_tests)
        .map_err(map_ordinary_error_v2)?;
    resources::charge_v2(
        &mut validated.resources.hash_work,
        2,
        input.limits.max_hash_work,
    )?;
    Ok(hash.finalize().into())
}

fn prove_leaf_v2(
    input: &ReliefAggregateInputV2<'_>,
    validated: &mut ValidatedReliefV2<'_>,
    leaf: DyadicLeafV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<bool, ReliefAggregateErrorV2> {
    let transforms = match validated
        .ordinary
        .interval_transform_session
        .prepare_leaf_with_checkpoint_v2(
            leaf.depth,
            leaf.index,
            input.ordinary.limits.schedule_limits,
            validated.ordinary.schedule_workspace_bound,
            validated.ordinary.schedule_workspace_bound.peak_bytes(),
            input
                .ordinary
                .limits
                .max_bridge_partition_search_work_per_node,
            &validated.ordinary.interval_transform_workspace_bound,
            || {
                checkpoint().map_err(|stop| match stop {
                    OrdinaryIntervalStopV2::Cancelled => {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled
                    }
                    OrdinaryIntervalStopV2::DeadlineExceeded => {
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded
                    }
                })
            },
        ) {
        Ok(value) => value,
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Inconclusive) => {
            return Ok(false);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit) => {
            return Err(ReliefAggregateErrorV2::ResourceLimit);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput) => {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Cancelled) => {
            return Err(ReliefAggregateErrorV2::Cancelled);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::DeadlineExceeded) => {
            return Err(ReliefAggregateErrorV2::DeadlineExceeded);
        }
    };
    let transform_resources = transforms.resources();
    let registry_resources = transform_resources.registry_resources();
    if transform_resources.schedule_workspace_upper_bound_bytes()
        > validated
            .ordinary
            .resources
            .charged_schedule_evaluation_workspace_bytes
        || transform_resources.angle_box_capacity_bytes()
            > validated.ordinary.resources.charged_angle_box_bytes
        || registry_resources.validation_work_upper_bound()
            > input
                .ordinary
                .limits
                .max_interval_registry_validation_work_per_node
        || registry_resources.sort_comparison_upper_bound()
            > input
                .ordinary
                .limits
                .max_interval_registry_sort_comparisons_per_node
        || registry_resources.construction_peak_bytes()
            > validated
                .ordinary
                .resources
                .charged_interval_registry_workspace_bytes
        || registry_resources.retained_registry_bytes()
            > validated
                .ordinary
                .resources
                .charged_interval_registry_retained_bytes
        || transform_resources.leaf_wrapper_overhead_bytes()
            != validated
                .ordinary
                .resources
                .charged_leaf_wrapper_overhead_bytes
        || transform_resources.retained_leaf_bytes()
            > validated.ordinary.resources.charged_leaf_retained_bytes
    {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    for pair in &validated.pairs {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut validated.resources.shared_pair_node_tests,
            1,
            input.limits.max_shared_pair_node_tests,
        )?;
        if !pair_strictly_separated_v2(
            input,
            pair,
            &transforms,
            &mut validated.resources,
            checkpoint,
        )? {
            return Ok(false);
        }
    }
    validated.resources.certified_shared_pair_leaf_count = validated
        .resources
        .certified_shared_pair_leaf_count
        .checked_add(validated.resources.shared_pairs)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    Ok(true)
}

fn pair_strictly_separated_v2(
    input: &ReliefAggregateInputV2<'_>,
    pair: &PreparedSharedPairV2,
    transforms: &CommonArticulationDynamicClosureIntervalTransformLeafV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<bool, ReliefAggregateErrorV2> {
    let geometry = input.ordinary.geometry;
    let face_position = |face: FaceId| {
        geometry
            .face_ids()
            .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
            .map_err(|_| ReliefAggregateErrorV2::InvalidInput)
    };
    let left_transform = transforms
        .transform_for_canonical_face_position_v2(
            geometry,
            face_position(pair.left.face)?,
            pair.left.face,
        )
        .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
    let right_transform = transforms
        .transform_for_canonical_face_position_v2(
            geometry,
            face_position(pair.right.face)?,
            pair.right.face,
        )
        .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
    let half = OutwardIntervalV1::from_rounded(input.ordinary.paper_thickness_mm)
        .and_then(|value| value.div(OutwardIntervalV1::from_rounded(2.0)?))
        .map_err(map_interval_error_v2)?;
    let lower_y =
        OutwardIntervalV1::new(-half.upper(), -half.lower()).map_err(map_interval_error_v2)?;
    let upper_y = half;
    let left_world = world_aabb_v2(
        &pair.left,
        left_transform,
        [lower_y, upper_y],
        input,
        resources,
        checkpoint,
    )?;
    let right_world = world_aabb_v2(
        &pair.right,
        right_transform,
        [lower_y, upper_y],
        input,
        resources,
        checkpoint,
    )?;
    if (0..3).any(|axis| strict_intervals_disjoint_v2(left_world[axis], right_world[axis])) {
        return Ok(true);
    }
    for (owner, local_axis) in [
        (left_transform, pair.left.support_axis),
        (right_transform, pair.right.support_axis),
    ] {
        relief_checkpoint_v2(checkpoint)?;
        let local_axis = [
            OutwardIntervalV1::from_rounded(local_axis[0]).map_err(map_interval_error_v2)?,
            OutwardIntervalV1::from_rounded(local_axis[1]).map_err(map_interval_error_v2)?,
            OutwardIntervalV1::from_rounded(local_axis[2]).map_err(map_interval_error_v2)?,
        ];
        let axis = owner
            .rotation()
            .apply(
                local_axis,
                input.ordinary.limits.max_interval_transform_work_per_node,
            )
            .map_err(map_interval_error_v2)?;
        let left_anchor = transform_anchor_v2(
            left_transform,
            pair.left.anchor,
            input.ordinary.limits.max_interval_transform_work_per_node,
        )?;
        let right_anchor = transform_anchor_v2(
            right_transform,
            pair.right.anchor,
            input.ordinary.limits.max_interval_transform_work_per_node,
        )?;
        let delta = [
            right_anchor[0]
                .sub(left_anchor[0])
                .map_err(map_interval_error_v2)?,
            right_anchor[1]
                .sub(left_anchor[1])
                .map_err(map_interval_error_v2)?,
            right_anchor[2]
                .sub(left_anchor[2])
                .map_err(map_interval_error_v2)?,
        ];
        if delta
            .iter()
            .any(|value| value.work() > input.ordinary.limits.max_interval_transform_work_per_node)
        {
            return Err(ReliefAggregateErrorV2::ResourceLimit);
        }
        let left_support = relative_support_v2(
            &pair.left,
            left_transform,
            axis,
            [lower_y, upper_y],
            None,
            input,
            resources,
            checkpoint,
        )?;
        let right_support = relative_support_v2(
            &pair.right,
            right_transform,
            axis,
            [lower_y, upper_y],
            Some(delta),
            input,
            resources,
            checkpoint,
        )?;
        if strict_intervals_disjoint_v2(left_support, right_support) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn strict_intervals_disjoint_v2(left: [f64; 2], right: [f64; 2]) -> bool {
    left[1] < right[0] || right[1] < left[0]
}

#[cfg(test)]
pub(super) fn strict_intervals_disjoint_for_test_v2(left: [f64; 2], right: [f64; 2]) -> bool {
    strict_intervals_disjoint_v2(left, right)
}

fn world_aabb_v2(
    cell: &PreparedCellV2,
    transform: ori_kinematics::IntervalRigidTransformV1,
    surfaces: [OutwardIntervalV1; 2],
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<[[f64; 2]; 3], ReliefAggregateErrorV2> {
    let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
    for point in &cell.ring {
        for y in surfaces {
            relief_checkpoint_v2(checkpoint)?;
            resources::charge_v2(
                &mut resources.axis_projection_work,
                3,
                input.limits.max_axis_projection_work,
            )?;
            let world = transform
                .apply(
                    [point[0], y, point[1]],
                    input.ordinary.limits.max_interval_transform_work_per_node,
                )
                .map_err(map_interval_error_v2)?;
            for axis in 0..3 {
                bounds[axis][0] = bounds[axis][0].min(world[axis].lower());
                bounds[axis][1] = bounds[axis][1].max(world[axis].upper());
            }
        }
    }
    if bounds.iter().flatten().any(|value| !value.is_finite()) {
        Err(ReliefAggregateErrorV2::UnprovenSharedRelief)
    } else {
        Ok(bounds)
    }
}

fn transform_anchor_v2(
    transform: ori_kinematics::IntervalRigidTransformV1,
    anchor: [f64; 3],
    max_work: usize,
) -> Result<[OutwardIntervalV1; 3], ReliefAggregateErrorV2> {
    let anchor = [
        OutwardIntervalV1::from_rounded(anchor[0]).map_err(map_interval_error_v2)?,
        OutwardIntervalV1::from_rounded(anchor[1]).map_err(map_interval_error_v2)?,
        OutwardIntervalV1::from_rounded(anchor[2]).map_err(map_interval_error_v2)?,
    ];
    transform
        .apply(anchor, max_work)
        .map_err(map_interval_error_v2)
}

#[allow(clippy::too_many_arguments)]
fn relative_support_v2(
    cell: &PreparedCellV2,
    transform: ori_kinematics::IntervalRigidTransformV1,
    axis: [OutwardIntervalV1; 3],
    surfaces: [OutwardIntervalV1; 2],
    translation: Option<[OutwardIntervalV1; 3]>,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<[f64; 2], ReliefAggregateErrorV2> {
    let anchor_x =
        OutwardIntervalV1::from_rounded(cell.anchor[0]).map_err(map_interval_error_v2)?;
    let anchor_z =
        OutwardIntervalV1::from_rounded(cell.anchor[2]).map_err(map_interval_error_v2)?;
    let mut support = [f64::INFINITY, f64::NEG_INFINITY];
    for point in &cell.ring {
        for y in surfaces {
            relief_checkpoint_v2(checkpoint)?;
            resources::charge_v2(
                &mut resources.axis_projection_work,
                18,
                input.limits.max_axis_projection_work,
            )?;
            let local = [
                point[0].sub(anchor_x).map_err(map_interval_error_v2)?,
                y,
                point[1].sub(anchor_z).map_err(map_interval_error_v2)?,
            ];
            if local.iter().any(|value| {
                value.work() > input.ordinary.limits.max_interval_transform_work_per_node
            }) {
                return Err(ReliefAggregateErrorV2::ResourceLimit);
            }
            let mut world = transform
                .rotation()
                .apply(
                    local,
                    input.ordinary.limits.max_interval_transform_work_per_node,
                )
                .map_err(map_interval_error_v2)?;
            if let Some(translation) = translation {
                for index in 0..3 {
                    world[index] = world[index]
                        .add(translation[index])
                        .map_err(map_interval_error_v2)?;
                    if world[index].work()
                        > input.ordinary.limits.max_interval_transform_work_per_node
                    {
                        return Err(ReliefAggregateErrorV2::ResourceLimit);
                    }
                }
            }
            let mut projection = OutwardIntervalV1::new(0.0, 0.0).map_err(map_interval_error_v2)?;
            for index in 0..3 {
                projection = projection
                    .add(
                        axis[index]
                            .mul(world[index])
                            .map_err(map_interval_error_v2)?,
                    )
                    .map_err(map_interval_error_v2)?;
                if projection.work() > input.ordinary.limits.max_interval_transform_work_per_node {
                    return Err(ReliefAggregateErrorV2::ResourceLimit);
                }
            }
            support[0] = support[0].min(projection.lower());
            support[1] = support[1].max(projection.upper());
        }
    }
    if support.iter().any(|value| !value.is_finite()) {
        Err(ReliefAggregateErrorV2::UnprovenSharedRelief)
    } else {
        Ok(support)
    }
}
