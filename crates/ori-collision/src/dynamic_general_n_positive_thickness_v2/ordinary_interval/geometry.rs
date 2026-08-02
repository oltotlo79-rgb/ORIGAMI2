//! Exact shared-feature classification and outward swept-AABB geometry.

use ori_kinematics::{
    CommonArticulationDynamicClosureBridgeStopV2,
    CommonArticulationDynamicClosureIntervalTransformLeafErrorV2, OutwardIntervalErrorV1,
    OutwardIntervalV1,
};
use sha2::{Digest, Sha256};

use super::*;

mod shared_pair_registry;

pub(super) fn derive_exact_shared_pair_registry_v2(
    geometry: &MaterialHingeGraphGeometry,
    pair_cap: usize,
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<Vec<OrdinaryIntervalFacePairV2>, OrdinaryIntervalErrorV2> {
    shared_pair_registry::derive_exact_shared_pair_registry_v2(
        geometry,
        pair_cap,
        membership_test_cap,
        checkpoint,
    )
}

pub(super) fn validate_exact_shared_pair_registry_v2(
    geometry: &MaterialHingeGraphGeometry,
    submitted: &[OrdinaryIntervalFacePairV2],
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<[u8; 32], OrdinaryIntervalErrorV2> {
    let mut cursor = 0usize;
    let mut membership_tests = 0usize;
    let mut hash = Sha256::new();
    hash.update(b"origami2/dynamic-general-n/shared-feature-registry/v2");
    for first in 0..geometry.face_ids().len() {
        checkpoint_v2(checkpoint)?;
        for second in first + 1..geometry.face_ids().len() {
            checkpoint_v2(checkpoint)?;
            let pair = OrdinaryIntervalFacePairV2 {
                first: geometry.face_ids()[first],
                second: geometry.face_ids()[second],
            };
            let shares_feature = faces_share_vertex_v2(
                geometry,
                pair,
                &mut membership_tests,
                membership_test_cap,
                checkpoint,
            )?;
            let submitted_matches = submitted.get(cursor).is_some_and(|value| *value == pair);
            if shares_feature != submitted_matches {
                return Err(OrdinaryIntervalErrorV2::ExcludedSharedPairCoverageMismatch);
            }
            if shares_feature {
                hash.update(pair.first.canonical_bytes());
                hash.update(pair.second.canonical_bytes());
                cursor = cursor
                    .checked_add(1)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
            }
        }
    }
    if cursor != submitted.len() {
        return Err(OrdinaryIntervalErrorV2::ExcludedSharedPairCoverageMismatch);
    }
    update_usize_v2(&mut hash, cursor)?;
    Ok(hash.finalize().into())
}

fn faces_share_vertex_v2(
    geometry: &MaterialHingeGraphGeometry,
    pair: OrdinaryIntervalFacePairV2,
    membership_tests: &mut usize,
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<bool, OrdinaryIntervalErrorV2> {
    let first = geometry
        .face_boundary_vertices(pair.first)
        .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
    let second = geometry
        .face_boundary_vertices(pair.second)
        .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
    for left in first {
        checkpoint_v2(checkpoint)?;
        for right in second {
            checkpoint_v2(checkpoint)?;
            *membership_tests = membership_tests
                .checked_add(1)
                .filter(|value| *value <= membership_test_cap)
                .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
            if left == right {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn prove_leaf_v2(
    input: &OrdinaryIntervalInputV2<'_>,
    leaf: DyadicLeafV2,
    validated: &ValidatedInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<bool, OrdinaryIntervalErrorV2> {
    checkpoint_v2(checkpoint)?;
    let transforms = match validated
        .interval_transform_session
        .prepare_leaf_with_checkpoint_v2(
            leaf.depth,
            leaf.index,
            input.limits.schedule_limits,
            validated.schedule_workspace_bound,
            validated.schedule_workspace_bound.peak_bytes(),
            input.limits.max_bridge_partition_search_work_per_node,
            &validated.interval_transform_workspace_bound,
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
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit) => {
            return Err(OrdinaryIntervalErrorV2::ResourceLimit);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Inconclusive) => {
            return Ok(false);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput) => {
            return Err(OrdinaryIntervalErrorV2::InvalidInput);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Cancelled) => {
            return Err(OrdinaryIntervalErrorV2::Cancelled);
        }
        Err(CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::DeadlineExceeded) => {
            return Err(OrdinaryIntervalErrorV2::DeadlineExceeded);
        }
    };
    checkpoint_v2(checkpoint)?;
    let transform_resources = transforms.resources();
    let registry_resources = transform_resources.registry_resources();
    if transform_resources.schedule_workspace_upper_bound_bytes()
        > validated
            .resources
            .charged_schedule_evaluation_workspace_bytes
        || transform_resources.angle_box_capacity_bytes()
            > validated.resources.charged_angle_box_bytes
        || registry_resources.validation_work_upper_bound()
            > input.limits.max_interval_registry_validation_work_per_node
        || registry_resources.sort_comparison_upper_bound()
            > input.limits.max_interval_registry_sort_comparisons_per_node
        || registry_resources.construction_peak_bytes()
            > validated
                .resources
                .charged_interval_registry_workspace_bytes
        || registry_resources.retained_registry_bytes()
            > validated.resources.charged_interval_registry_retained_bytes
        || transform_resources.leaf_wrapper_overhead_bytes()
            != validated.resources.charged_leaf_wrapper_overhead_bytes
        || transform_resources.retained_leaf_bytes()
            > validated.resources.charged_leaf_retained_bytes
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    let Some(bounds) = thick_face_aabbs_v2(input, &transforms, checkpoint)? else {
        return Ok(false);
    };
    all_ordinary_pairs_strictly_separated_v2(
        input.geometry.face_ids(),
        &bounds,
        input.excluded_shared_pairs,
        validated.resources.ordinary_face_pairs,
        checkpoint,
    )
}

fn thick_face_aabbs_v2(
    input: &OrdinaryIntervalInputV2<'_>,
    transforms: &CommonArticulationDynamicClosureIntervalTransformLeafV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<Option<Vec<ThickFaceAabbV2>>, OrdinaryIntervalErrorV2> {
    let two = OutwardIntervalV1::from_rounded(2.0).map_err(map_interval_error_v2)?;
    let thickness =
        OutwardIntervalV1::from_rounded(input.paper_thickness_mm).map_err(map_interval_error_v2)?;
    let half = thickness.div(two).map_err(map_interval_error_v2)?;
    let upper_y = half;
    let lower_y =
        OutwardIntervalV1::new(-half.upper(), -half.lower()).map_err(map_interval_error_v2)?;
    let mut bounds = Vec::new();
    bounds
        .try_reserve_exact(input.geometry.face_ids().len())
        .map_err(|_| OrdinaryIntervalErrorV2::ResourceLimit)?;
    if bounds.capacity() > input.geometry.face_ids().len() {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    for (face_position, face) in input.geometry.face_ids().iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        let transform = transforms
            .transform_for_canonical_face_position_v2(input.geometry, face_position, *face)
            .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
        let boundary = input
            .geometry
            .face_boundary_vertices(*face)
            .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
        let mut lower = [f64::INFINITY; AXIS_COUNT_V2];
        let mut upper = [f64::NEG_INFINITY; AXIS_COUNT_V2];
        for vertex in boundary {
            checkpoint_v2(checkpoint)?;
            let point = input
                .geometry
                .vertex_position(*vertex)
                .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
            let x = OutwardIntervalV1::from_rounded(point.x()).map_err(map_interval_error_v2)?;
            let z = OutwardIntervalV1::from_rounded(point.z()).map_err(map_interval_error_v2)?;
            for y in [lower_y, upper_y] {
                checkpoint_v2(checkpoint)?;
                let world = match transform
                    .apply([x, y, z], input.limits.max_interval_transform_work_per_node)
                {
                    Ok(world) => world,
                    Err(OutwardIntervalErrorV1::ResourceLimit) => {
                        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
                    }
                    Err(
                        OutwardIntervalErrorV1::InvalidEndpoint
                        | OutwardIntervalErrorV1::DivisionByZeroInterval,
                    ) => return Ok(None),
                };
                for axis in 0..AXIS_COUNT_V2 {
                    lower[axis] = lower[axis].min(world[axis].lower());
                    upper[axis] = upper[axis].max(world[axis].upper());
                }
            }
        }
        if lower.iter().chain(&upper).any(|value| !value.is_finite()) {
            return Err(OrdinaryIntervalErrorV2::InvalidInput);
        }
        bounds.push(ThickFaceAabbV2 {
            face: *face,
            lower,
            upper,
        });
    }
    Ok(Some(bounds))
}

fn all_ordinary_pairs_strictly_separated_v2(
    faces: &[FaceId],
    bounds: &[ThickFaceAabbV2],
    excluded_shared_pairs: &[OrdinaryIntervalFacePairV2],
    expected_ordinary_pairs: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<bool, OrdinaryIntervalErrorV2> {
    if faces.len() != bounds.len() {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    for (face, bound) in faces.iter().zip(bounds) {
        checkpoint_v2(checkpoint)?;
        if *face != bound.face {
            return Err(OrdinaryIntervalErrorV2::InvalidInput);
        }
    }
    let mut excluded_cursor = 0usize;
    let mut ordinary_pairs = 0usize;
    for first in 0..faces.len() {
        checkpoint_v2(checkpoint)?;
        for second in first + 1..faces.len() {
            checkpoint_v2(checkpoint)?;
            let pair = OrdinaryIntervalFacePairV2 {
                first: faces[first],
                second: faces[second],
            };
            if excluded_shared_pairs
                .get(excluded_cursor)
                .is_some_and(|excluded| *excluded == pair)
            {
                excluded_cursor = excluded_cursor
                    .checked_add(1)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                continue;
            }
            ordinary_pairs = ordinary_pairs
                .checked_add(1)
                .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
            let mut separated = false;
            for axis in 0..AXIS_COUNT_V2 {
                checkpoint_v2(checkpoint)?;
                if bounds[first].upper[axis] < bounds[second].lower[axis]
                    || bounds[second].upper[axis] < bounds[first].lower[axis]
                {
                    separated = true;
                    break;
                }
            }
            if !separated {
                return Ok(false);
            }
        }
    }
    if excluded_cursor != excluded_shared_pairs.len() || ordinary_pairs != expected_ordinary_pairs {
        return Err(OrdinaryIntervalErrorV2::ExcludedSharedPairCoverageMismatch);
    }
    Ok(true)
}

fn map_interval_error_v2(error: OutwardIntervalErrorV1) -> OrdinaryIntervalErrorV2 {
    match error {
        OutwardIntervalErrorV1::ResourceLimit => OrdinaryIntervalErrorV2::ResourceLimit,
        OutwardIntervalErrorV1::InvalidEndpoint
        | OutwardIntervalErrorV1::DivisionByZeroInterval => OrdinaryIntervalErrorV2::InvalidInput,
    }
}
