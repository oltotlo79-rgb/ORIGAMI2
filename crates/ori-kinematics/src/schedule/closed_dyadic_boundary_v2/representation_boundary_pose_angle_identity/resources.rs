//! Pose `Arc` identity control-block allocator metadata follows the existing
//! logical-retained accounting convention: it is not charged and cannot
//! amplify retained payload bytes.

use super::*;

pub(super) const POSE_IDENTITY_FIXED_WORK_V2: usize = 12;
const SHA256_STREAMING_WORKSPACE_BYTES_V2: usize = 104;

#[derive(Debug, Clone, Copy)]
struct PoseResourceProjectionV2 {
    representation_boundary_poses_deep_retained_bytes: usize,
    pose_retained_scan_work: usize,
    graph_binding_work: usize,
    logical_work_required: usize,
    workspace_peak_bytes: usize,
}

pub(super) fn checked_resource_bound_v2(
    schedule: &CanonicalCycleScheduleV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
    schedule_limits: CycleScheduleLimitsV1,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
    >,
) -> Result<
    CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
    CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
> {
    checkpoint_v2(checkpoint)?;
    let shape = resources::checked_resource_projection_shape_v2(schedule, &mut || {
        checkpoint().map_err(map_stop_to_closed_v2)
    })
    .map_err(map_closed_error_v2)?;
    let closed_boundary_bound = resources::checked_resource_bound_from_shape_v2(
        schedule,
        schedule_limits,
        usize::MAX,
        shape,
        &mut || checkpoint().map_err(map_stop_to_closed_v2),
    )
    .map_err(map_closed_error_v2)?;
    let projection = checked_projection_v2(
        shape,
        geometry,
        audit,
        lower_pose,
        upper_pose,
        schedule_limits,
        closed_boundary_bound.logical_work_required_v2(),
        closed_boundary_bound.workspace_peak_bytes_upper_bound_v2(),
        checkpoint,
    )?;
    checkpoint_v2(checkpoint)?;
    Ok(bound_from_projection_v2(closed_boundary_bound, projection))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn checked_resource_bound_for_limits_v2(
    schedule: &CanonicalCycleScheduleV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
    closed_boundary_evidence: &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
    schedule_limits: CycleScheduleLimitsV1,
    limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
    >,
) -> Result<
    CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
    CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
> {
    let hinge_count =
        match (
            schedule.entries.is_empty(),
            schedule.half_angle_entries.is_empty(),
        ) {
            (false, true) => schedule.entries.len(),
            (true, false) => schedule.half_angle_entries.len(),
            _ => return Err(
                CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::BoundaryPoseMismatch,
            ),
        };
    if hinge_count == 0 || hinge_count > limits.max_hinges {
        return Err(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit);
    }
    let shape = resources::checked_resource_projection_shape_v2(schedule, &mut || {
        checkpoint().map_err(map_stop_to_closed_v2)
    })
    .map_err(map_closed_error_v2)?;
    let projected_closed_logical_work =
        resources::checked_logical_work_required_v2(shape, schedule_limits)
            .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    let projected_closed_workspace_peak_bytes =
        resources::checked_projected_boundary_workspace_peak_v2(shape, schedule_limits)
            .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    if closed_boundary_evidence.hinge_count_v2() != shape.hinge_count
        || closed_boundary_evidence.logical_work_v2() != projected_closed_logical_work
        || closed_boundary_evidence.workspace_peak_bytes_upper_bound_v2()
            != projected_closed_workspace_peak_bytes
    {
        return Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ClosedBoundaryEvidenceMismatch,
        );
    }
    let projection = checked_projection_v2(
        shape,
        geometry,
        audit,
        lower_pose,
        upper_pose,
        schedule_limits,
        projected_closed_logical_work,
        projected_closed_workspace_peak_bytes,
        checkpoint,
    )?;
    if projection.representation_boundary_poses_deep_retained_bytes
        > limits.max_representation_boundary_poses_deep_retained_bytes
        || projection.logical_work_required != limits.max_logical_work
        || projection.workspace_peak_bytes != limits.max_workspace_bytes
    {
        return Err(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit);
    }
    let closed_boundary_bound = resources::checked_resource_bound_from_shape_v2(
        schedule,
        schedule_limits,
        limits.max_schedule_deep_retained_bytes,
        shape,
        &mut || checkpoint().map_err(map_stop_to_closed_v2),
    )
    .map_err(map_closed_error_v2)?;
    let bound = bound_from_projection_v2(closed_boundary_bound, projection);
    validate_limits_v2(bound, limits)?;
    Ok(bound)
}

fn bound_from_projection_v2(
    closed_boundary_bound: CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    projection: PoseResourceProjectionV2,
) -> CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2 {
    CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2 {
        closed_boundary_bound,
        representation_boundary_poses_deep_retained_bytes: projection
            .representation_boundary_poses_deep_retained_bytes,
        pose_retained_scan_work: projection.pose_retained_scan_work,
        graph_binding_work: projection.graph_binding_work,
        logical_work_required: projection.logical_work_required,
        workspace_peak_bytes: projection.workspace_peak_bytes,
    }
}

#[allow(clippy::too_many_arguments)]
fn checked_projection_v2(
    shape: resources::BoundaryResourceShapeV2,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
    schedule_limits: CycleScheduleLimitsV1,
    closed_logical_work: usize,
    closed_workspace_peak_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
    >,
) -> Result<PoseResourceProjectionV2, CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    if lower_pose.hinge_angles().as_slice().len() != shape.hinge_count
        || upper_pose.hinge_angles().as_slice().len() != shape.hinge_count
    {
        return Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::BoundaryPoseMismatch,
        );
    }
    checkpoint_v2(checkpoint)?;
    let lower_bytes = lower_pose
        .checked_deep_retained_bytes_v1()
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    checkpoint_v2(checkpoint)?;
    let upper_bytes = upper_pose
        .checked_deep_retained_bytes_v1()
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    let representation_boundary_poses_deep_retained_bytes = lower_bytes
        .checked_add(upper_bytes)
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    let pose_retained_scan_work = checked_pose_scan_work_v2(lower_pose)?
        .checked_add(checked_pose_scan_work_v2(upper_pose)?)
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    // `matches_binding_with_checkpoint_v2` scans one audit face sequence; the
    // representation-boundary live join independently compares that sequence
    // to the geometry face order, so charge both scans.
    let graph_binding_work = geometry
        .hinges()
        .len()
        .checked_add(audit.faces().len())
        .and_then(|work| work.checked_add(audit.spanning_hinges().len()))
        .and_then(|work| work.checked_add(audit.closure_hinges().len()))
        .and_then(|work| work.checked_add(audit.faces().len()))
        .and_then(|work| work.checked_add(2))
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    let pose_evaluation_work = checked_pose_evaluation_work_v2(shape, schedule_limits)
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    let logical_work_required = closed_logical_work
        .checked_add(pose_retained_scan_work)
        .and_then(|work| work.checked_add(graph_binding_work))
        .and_then(|work| work.checked_add(POSE_IDENTITY_FIXED_WORK_V2))
        .and_then(|work| work.checked_add(pose_evaluation_work))
        .and_then(|work| work.checked_add(binding::checked_binding_work_v2()))
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    Ok(PoseResourceProjectionV2 {
        representation_boundary_poses_deep_retained_bytes,
        pose_retained_scan_work,
        graph_binding_work,
        logical_work_required,
        workspace_peak_bytes: closed_workspace_peak_bytes.max(SHA256_STREAMING_WORKSPACE_BYTES_V2),
    })
}

fn checked_pose_scan_work_v2(
    pose: &ClosedMaterialHingeGraphPose,
) -> Result<usize, CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    3usize
        .checked_add(pose.hinge_angles().as_slice().len())
        .and_then(|visits| visits.checked_add(pose.transforms().len()))
        .and_then(|visits| visits.checked_add(pose.closure_certificate().checked_hinges().len()))
        .ok_or(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)
}

pub(super) fn poll_pose_retained_v2(
    pose: &ClosedMaterialHingeGraphPose,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
    >,
) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    checkpoint_v2(checkpoint)?;
    for _ in pose.hinge_angles().as_slice() {
        checkpoint_v2(checkpoint)?;
    }
    for _ in pose.transforms() {
        checkpoint_v2(checkpoint)?;
    }
    for _ in pose.closure_certificate().checked_hinges() {
        checkpoint_v2(checkpoint)?;
    }
    checkpoint_v2(checkpoint)
}

fn checked_pose_evaluation_work_v2(
    shape: resources::BoundaryResourceShapeV2,
    schedule_limits: CycleScheduleLimitsV1,
) -> Option<usize> {
    match shape.representation {
        BoundaryRepresentationV2::Ordinary => shape
            .ordinary_coefficient_count
            .checked_add(shape.hinge_count.checked_mul(2)?)?
            .checked_mul(2),
        BoundaryRepresentationV2::HalfAngle => shape
            .half_angle_power_coefficient_count
            // Each of the two representation boundaries evaluates every power
            // coefficient twice: once for the existing public point model and
            // once for the exact-rational endpoint box.
            .checked_mul(4)?
            .checked_add(
                shape
                    .hinge_count
                    .checked_mul(schedule_limits.max_work.checked_add(5)?)?
                    .checked_mul(2)?,
            ),
    }
}

pub(super) fn validate_limits_v2(
    bound: CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
    limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    if limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || bound.hinge_count_v2() > limits.max_hinges
        || bound.schedule_deep_retained_bytes_v2() > limits.max_schedule_deep_retained_bytes
        || bound.representation_boundary_poses_deep_retained_bytes_v2()
            > limits.max_representation_boundary_poses_deep_retained_bytes
        || bound.logical_work_required_v2() != limits.max_logical_work
        || bound.workspace_peak_bytes_upper_bound_v2() != limits.max_workspace_bytes
    {
        return Err(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit);
    }
    Ok(())
}

pub(super) const fn limit_values_v2(
    limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
) -> [usize; 5] {
    [
        limits.max_hinges,
        limits.max_schedule_deep_retained_bytes,
        limits.max_representation_boundary_poses_deep_retained_bytes,
        limits.max_logical_work,
        limits.max_workspace_bytes,
    ]
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
    >,
) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::Cancelled => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::Cancelled
        }
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::DeadlineExceeded => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::DeadlineExceeded
        }
    })
}

pub(super) const fn map_stop_to_closed_v2(
    stop: CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
) -> CycleScheduleClosedDyadicBoundaryStopV2 {
    match stop {
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::Cancelled => {
            CycleScheduleClosedDyadicBoundaryStopV2::Cancelled
        }
        CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::DeadlineExceeded => {
            CycleScheduleClosedDyadicBoundaryStopV2::DeadlineExceeded
        }
    }
}

pub(super) const fn map_closed_error_v2(
    error: CycleScheduleClosedDyadicBoundaryErrorV2,
) -> CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2 {
    match error {
        CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::ResourceLimit,
        )
        | CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit
        }
        CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::Cancelled
        }
        CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::DeadlineExceeded
        }
        CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(_) => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ClosedBoundaryEvidenceMismatch
        }
    }
}
