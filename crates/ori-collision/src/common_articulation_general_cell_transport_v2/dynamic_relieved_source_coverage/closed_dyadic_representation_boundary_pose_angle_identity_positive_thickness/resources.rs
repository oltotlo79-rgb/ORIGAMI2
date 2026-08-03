use std::mem::size_of;

use super::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2;
type LimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2;
type CertificateV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
type BoundaryV2 = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
type PoseIdentityV2 = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;

pub(super) fn checked_resources_v2(
    boundary: &BoundaryV2,
    pose_identity: &PoseIdentityV2,
    limits: LimitsV2,
) -> Result<Phase3JResourcesV2, ErrorV2> {
    if limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || boundary.actual_block_count_v2() < GENERAL_N_MIN_BLOCKS_V2
        || boundary.actual_block_count_v2() > limits.max_blocks
        || boundary.block_count_cap_internal_v2() != limits.max_blocks
        || boundary.hinge_count_v2() > limits.max_hinges
        || pose_identity.hinge_count_v2() > limits.max_hinges
        || boundary.hinge_count_cap_internal_v2() != limits.max_hinges
        || pose_identity.replay_hinge_count_cap_v2() != limits.max_hinges
        || boundary.closed_dyadic_boundary_configuration_count_v2() != 2
        || pose_identity.representation_boundary_pose_angle_identity_count_v2() != 2
        || !boundary.both_closed_dyadic_boundary_configurations_have_positive_thickness_v2()
        || boundary.schedule_deep_retained_bytes_cap_internal_v2()
            != limits.max_schedule_deep_retained_bytes
        || pose_identity.replay_schedule_deep_retained_bytes_cap_v2()
            != limits.max_schedule_deep_retained_bytes
        || pose_identity.replay_representation_boundary_poses_deep_retained_bytes_cap_v2()
            != limits.max_representation_boundary_poses_deep_retained_bytes
        || pose_identity.logical_work_v2() != limits.max_pose_angle_identity_logical_work
        || pose_identity.workspace_peak_bytes_upper_bound_v2()
            != limits.max_pose_angle_identity_workspace_bytes
        || !pose_identity.replay_policy_matches_v2(
            boundary.schedule_limits_internal_v2(),
            pose_identity_limits_v2(limits),
        )
        || size_of::<BoundaryV2>() > limits.max_retained_boundary_configuration_prerequisite_bytes
        || size_of::<CertificateV2>() > limits.max_publication_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }

    let publication_bytes = size_of::<CertificateV2>();
    let retained_boundary = size_of::<BoundaryV2>();
    let outer_shell_delta = publication_bytes
        .checked_sub(retained_boundary)
        .ok_or(ErrorV2::ResourceLimit)?;
    let boundary_replay_phase = boundary
        .replay_aggregate_peak_cap_internal_v2()
        .checked_add(outer_shell_delta)
        .and_then(|bytes| {
            bytes.checked_add(limits.max_representation_boundary_poses_deep_retained_bytes)
        })
        .ok_or(ErrorV2::ResourceLimit)?;
    let pose_join_phase = publication_bytes
        .checked_add(limits.max_schedule_deep_retained_bytes)
        .and_then(|bytes| {
            bytes.checked_add(limits.max_representation_boundary_poses_deep_retained_bytes)
        })
        .and_then(|bytes| bytes.checked_add(limits.max_pose_angle_identity_workspace_bytes))
        .and_then(|bytes| bytes.checked_add(size_of::<PoseIdentityV2>()))
        .ok_or(ErrorV2::ResourceLimit)?;
    let composition_phase = publication_bytes
        .checked_add(COMPOSITION_WORKSPACE_BYTES_V2)
        .ok_or(ErrorV2::ResourceLimit)?;
    let aggregate_peak_bytes = boundary_replay_phase
        .max(pose_join_phase)
        .max(composition_phase);
    if aggregate_peak_bytes > limits.max_aggregate_peak_bytes {
        return Err(ErrorV2::ResourceLimit);
    }

    Ok(Phase3JResourcesV2 {
        retained_boundary_configuration_prerequisite_bytes: retained_boundary,
        retained_pose_angle_identity_evidence_bytes: size_of::<PoseIdentityV2>(),
        schedule_deep_retained_bytes_cap: limits.max_schedule_deep_retained_bytes,
        representation_boundary_poses_deep_retained_bytes_cap: limits
            .max_representation_boundary_poses_deep_retained_bytes,
        pose_angle_identity_logical_work: limits.max_pose_angle_identity_logical_work,
        pose_angle_identity_workspace_bytes: limits.max_pose_angle_identity_workspace_bytes,
        delegated_boundary_configuration_replay_peak_bytes: boundary
            .replay_aggregate_peak_cap_internal_v2(),
        composition_workspace_bytes: COMPOSITION_WORKSPACE_BYTES_V2,
        publication_bytes,
        aggregate_peak_bytes,
    })
}

pub(super) const fn pose_identity_limits_v2(
    limits: LimitsV2,
) -> CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
        max_hinges: limits.max_hinges,
        max_schedule_deep_retained_bytes: limits.max_schedule_deep_retained_bytes,
        max_representation_boundary_poses_deep_retained_bytes: limits
            .max_representation_boundary_poses_deep_retained_bytes,
        max_logical_work: limits.max_pose_angle_identity_logical_work,
        max_workspace_bytes: limits.max_pose_angle_identity_workspace_bytes,
    }
}

pub(super) const fn limits_match_v2(retained: LimitsV2, live: LimitsV2) -> bool {
    retained.max_blocks == live.max_blocks
        && retained.max_hinges == live.max_hinges
        && retained.max_schedule_deep_retained_bytes == live.max_schedule_deep_retained_bytes
        && retained.max_representation_boundary_poses_deep_retained_bytes
            == live.max_representation_boundary_poses_deep_retained_bytes
        && retained.max_pose_angle_identity_logical_work
            == live.max_pose_angle_identity_logical_work
        && retained.max_pose_angle_identity_workspace_bytes
            == live.max_pose_angle_identity_workspace_bytes
        && retained.max_retained_boundary_configuration_prerequisite_bytes
            == live.max_retained_boundary_configuration_prerequisite_bytes
        && retained.max_publication_bytes == live.max_publication_bytes
        && retained.max_aggregate_peak_bytes == live.max_aggregate_peak_bytes
}

pub(super) const fn limit_values_v2(limits: LimitsV2) -> [usize; 9] {
    [
        limits.max_blocks,
        limits.max_hinges,
        limits.max_schedule_deep_retained_bytes,
        limits.max_representation_boundary_poses_deep_retained_bytes,
        limits.max_pose_angle_identity_logical_work,
        limits.max_pose_angle_identity_workspace_bytes,
        limits.max_retained_boundary_configuration_prerequisite_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
}
