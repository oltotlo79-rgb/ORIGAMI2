use std::mem::size_of;

use crate::RigidTransform;

use super::*;

const FIXED_LOGICAL_WORK_V2: usize = 32;
const SHA256_STREAMING_WORKSPACE_BYTES_V2: usize = 104;

pub(super) fn checked_resource_bound_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
    checkpoint: &mut impl FnMut() -> Result<(), CanonicalBinary64PosePairTransformRealizationStopV2>,
) -> Result<
    CanonicalBinary64PosePairTransformRealizationResourceBoundV2,
    CanonicalBinary64PosePairTransformRealizationErrorV2,
> {
    checkpoint_v2(checkpoint)?;
    let face_count = geometry.face_ids().len();
    let hinge_count = geometry.hinges().len();
    let spanning_hinge_count = audit.spanning_hinges().len();
    if face_count == 0
        || audit.faces().len() != face_count
        || spanning_hinge_count.checked_add(audit.closure_hinges().len()) != Some(hinge_count)
        || spanning_hinge_count != face_count.saturating_sub(1)
        || [lower_pose, upper_pose].into_iter().any(|pose| {
            pose.transforms().len() != face_count
                || pose.hinge_angles().as_slice().len() != hinge_count
        })
    {
        return Err(CanonicalBinary64PosePairTransformRealizationErrorV2::AuditMismatch);
    }
    let pose_pair_deep_retained_bytes = lower_pose
        .checked_deep_retained_bytes_v1()
        .and_then(|bytes| bytes.checked_add(upper_pose.checked_deep_retained_bytes_v1()?))
        .ok_or(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit)?;
    let logical_work = checked_logical_work_v2(face_count, hinge_count)?;
    let workspace_structural_requirement_bytes =
        checked_workspace_structural_requirement_bytes_v2(face_count, hinge_count)?;
    checkpoint_v2(checkpoint)?;
    Ok(
        CanonicalBinary64PosePairTransformRealizationResourceBoundV2 {
            face_count,
            hinge_count,
            spanning_hinge_count,
            pose_pair_deep_retained_bytes,
            logical_work,
            workspace_structural_requirement_bytes,
        },
    )
}

fn checked_logical_work_v2(
    face_count: usize,
    hinge_count: usize,
) -> Result<usize, CanonicalBinary64PosePairTransformRealizationErrorV2> {
    // Shape/audit + marker construction + two all-hinge BFS traversals + two
    // canonical 12-scalar comparisons + complete semantic binding scans. The
    // linear coefficients deliberately charge every record/scalar loop even
    // when a cheap validation branch could finish early.
    let face_searches = hinge_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(face_count.checked_mul(4)?))
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(resource_error)?;
    let edge_searches = hinge_count.checked_mul(2).ok_or_else(resource_error)?;
    let lookup_work = checked_binary_search_comparison_bound_v2(face_count)
        .checked_mul(face_searches)
        .and_then(|value| {
            value.checked_add(
                checked_binary_search_comparison_bound_v2(hinge_count)
                    .checked_mul(edge_searches)?,
            )
        })
        .ok_or_else(resource_error)?;
    FIXED_LOGICAL_WORK_V2
        .checked_add(face_count.checked_mul(64).ok_or(resource_error())?)
        .and_then(|value| value.checked_add(hinge_count.checked_mul(12)?))
        .and_then(|value| value.checked_add(face_count.checked_mul(hinge_count)?.checked_mul(2)?))
        .and_then(|value| value.checked_add(lookup_work))
        .ok_or_else(resource_error)
}

const fn checked_binary_search_comparison_bound_v2(len: usize) -> usize {
    usize::BITS as usize - len.leading_zeros() as usize
}

fn checked_workspace_structural_requirement_bytes_v2(
    face_count: usize,
    hinge_count: usize,
) -> Result<usize, CanonicalBinary64PosePairTransformRealizationErrorV2> {
    let requested = size_of::<u8>()
        .checked_mul(hinge_count)
        .and_then(|value| {
            value.checked_add(size_of::<Option<RigidTransform>>().checked_mul(face_count)?)
        })
        .and_then(|value| value.checked_add(size_of::<usize>().checked_mul(face_count)?))
        .ok_or_else(resource_error)?;
    Ok(requested
        .checked_mul(2)
        .ok_or_else(resource_error)?
        .max(SHA256_STREAMING_WORKSPACE_BYTES_V2))
}

pub(super) fn validate_limits_v2(
    bound: CanonicalBinary64PosePairTransformRealizationResourceBoundV2,
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
) -> Result<(), CanonicalBinary64PosePairTransformRealizationErrorV2> {
    if limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || bound.face_count > limits.max_faces
        || bound.hinge_count > limits.max_hinges
        || bound.pose_pair_deep_retained_bytes > limits.max_pose_pair_deep_retained_bytes
        || bound.logical_work != limits.max_logical_work
        || bound.workspace_structural_requirement_bytes > limits.max_workspace_bytes
    {
        return Err(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit);
    }
    Ok(())
}

pub(super) fn preflight_live_limits_v2(
    geometry: &MaterialHingeGraphGeometry,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
) -> Result<(), CanonicalBinary64PosePairTransformRealizationErrorV2> {
    if limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
    {
        return Err(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit);
    }
    let face_count = geometry.face_ids().len();
    let hinge_count = geometry.hinges().len();
    let pose_pair_deep_retained_bytes = lower_pose
        .checked_deep_retained_bytes_v1()
        .and_then(|bytes| bytes.checked_add(upper_pose.checked_deep_retained_bytes_v1()?))
        .ok_or(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit)?;
    let logical_work = checked_logical_work_v2(face_count, hinge_count)?;
    let workspace_structural_requirement_bytes =
        checked_workspace_structural_requirement_bytes_v2(face_count, hinge_count)?;
    if face_count > limits.max_faces
        || hinge_count > limits.max_hinges
        || pose_pair_deep_retained_bytes > limits.max_pose_pair_deep_retained_bytes
        || logical_work > limits.max_logical_work
        || workspace_structural_requirement_bytes > limits.max_workspace_bytes
    {
        return Err(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit);
    }
    Ok(())
}

pub(super) const fn limit_values_v2(
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
) -> [usize; 5] {
    [
        limits.max_faces,
        limits.max_hinges,
        limits.max_pose_pair_deep_retained_bytes,
        limits.max_logical_work,
        limits.max_workspace_bytes,
    ]
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CanonicalBinary64PosePairTransformRealizationStopV2>,
) -> Result<(), CanonicalBinary64PosePairTransformRealizationErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CanonicalBinary64PosePairTransformRealizationStopV2::Cancelled => {
            CanonicalBinary64PosePairTransformRealizationErrorV2::Cancelled
        }
        CanonicalBinary64PosePairTransformRealizationStopV2::DeadlineExceeded => {
            CanonicalBinary64PosePairTransformRealizationErrorV2::DeadlineExceeded
        }
    })
}

const fn resource_error() -> CanonicalBinary64PosePairTransformRealizationErrorV2 {
    CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit
}
