use std::mem::size_of;

use super::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2;
type StopV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2;
type LimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2;
type CertificateV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
type InputV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteInputV2;
type ReplayInputV2<'a> = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteRevalidationInputV2<'a>;

pub(super) fn issue_v2(
    input: InputV2,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<CertificateV2, ErrorV2> {
    checkpoint_v2(checkpoint)?;
    let resources = resources::checked_resources_v2(
        &input.boundary_configuration_prerequisite,
        &input.pose_angle_identity_evidence,
        input.limits,
    )?;
    validate_proof_join_v2(
        &input.boundary_configuration_prerequisite,
        &input.pose_angle_identity_evidence,
    )?;
    let binding_fingerprint = binding::binding_fingerprint_v2(
        &input.boundary_configuration_prerequisite,
        &input.pose_angle_identity_evidence,
        resources,
        input.limits,
    )?;
    checkpoint_v2(checkpoint)?;
    Ok(CertificateV2 {
        boundary_configuration_prerequisite: input.boundary_configuration_prerequisite,
        pose_angle_identity_evidence: input.pose_angle_identity_evidence,
        resources,
        limits: input.limits,
        binding_fingerprint,
    })
}

pub(super) fn revalidate_v2(
    certificate: &CertificateV2,
    input: ReplayInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    checkpoint_v2(checkpoint)?;
    preflight_live_caps_v2(certificate, input.limits)?;
    if !resources::limits_match_v2(certificate.limits, input.limits) {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    if !certificate
        .boundary_configuration_prerequisite
        .cheap_replay_tuple_matches_internal_v2(&input.boundary_configuration_replay)
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    validate_live_join_v2(certificate, &input)?;
    let resources = resources::checked_resources_v2(
        &certificate.boundary_configuration_prerequisite,
        &certificate.pose_angle_identity_evidence,
        input.limits,
    )?;
    let binding_fingerprint = binding::binding_fingerprint_v2(
        &certificate.boundary_configuration_prerequisite,
        &certificate.pose_angle_identity_evidence,
        resources,
        input.limits,
    )?;
    if certificate.resources != resources || certificate.binding_fingerprint != binding_fingerprint
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }

    let pose_input = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2 {
        geometry: input.boundary_configuration_replay.geometry,
        audit: input.audit,
        schedule: input.boundary_configuration_replay.schedule,
        closed_boundary_evidence: certificate
            .boundary_configuration_prerequisite
            .closed_boundary_evidence_internal_v2(),
        lower_pose: input.lower_pose,
        upper_pose: input.upper_pose,
        schedule_limits: input.boundary_configuration_replay.schedule_limits,
        limits: resources::pose_identity_limits_v2(input.limits),
    };
    certificate
        .boundary_configuration_prerequisite
        .revalidate_with_checkpoint_v2(input.boundary_configuration_replay, || {
            checkpoint().map_err(map_stop_to_boundary_v2)
        })
        .map_err(map_boundary_error_v2)?;
    certificate
        .pose_angle_identity_evidence
        .revalidate_with_checkpoint_v2(pose_input, || checkpoint().map_err(map_stop_to_pose_v2))
        .map_err(map_pose_error_v2)?;
    checkpoint_v2(checkpoint)
}

fn validate_proof_join_v2(
    boundary: &CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    pose_identity: &CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
) -> Result<(), ErrorV2> {
    if !pose_identity.matches_geometry_instance_anchor_v2(boundary.issuer_geometry_instance_v2())
        || boundary.schedule_binding_fingerprint_internal_v2()
            != pose_identity.schedule_binding_fingerprint_v2()
        || boundary.graph_binding_fingerprint_internal_v2()
            != pose_identity.graph_binding_fingerprint_v1()
        || boundary.closed_boundary_binding_fingerprint_internal_v2()
            != pose_identity.closed_boundary_evidence_binding_fingerprint_v2()
        || boundary.hinge_count_v2() != pose_identity.hinge_count_v2()
        || boundary.closed_dyadic_boundary_configuration_count_v2() != 2
        || pose_identity.representation_boundary_pose_angle_identity_count_v2() != 2
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    Ok(())
}

fn validate_live_join_v2(
    certificate: &CertificateV2,
    input: &ReplayInputV2<'_>,
) -> Result<(), ErrorV2> {
    let boundary = &certificate.boundary_configuration_prerequisite;
    let pose_identity = &certificate.pose_angle_identity_evidence;
    let geometry = input.boundary_configuration_replay.geometry;
    let schedule = input.boundary_configuration_replay.schedule;
    let pose_limits = resources::pose_identity_limits_v2(input.limits);
    let lower_bytes = input
        .lower_pose
        .checked_deep_retained_bytes_v1()
        .ok_or(ErrorV2::ResourceLimit)?;
    let pose_bytes = lower_bytes
        .checked_add(
            input
                .upper_pose
                .checked_deep_retained_bytes_v1()
                .ok_or(ErrorV2::ResourceLimit)?,
        )
        .ok_or(ErrorV2::ResourceLimit)?;
    if !pose_identity.matches_geometry_instance_v2(geometry)
        || !pose_identity.matches_pose_instances_v2(input.lower_pose, input.upper_pose)
        || !input.lower_pose.is_for_geometry(geometry)
        || !input.upper_pose.is_for_geometry(geometry)
        || input.lower_pose.fixed_face() != input.upper_pose.fixed_face()
        || input.lower_pose.fixed_face() != pose_identity.fixed_face_v2()
        || input.lower_pose.hinge_angles().as_slice().len() != pose_identity.hinge_count_v2()
        || input.upper_pose.hinge_angles().as_slice().len() != pose_identity.hinge_count_v2()
        || pose_bytes
            > input
                .limits
                .max_representation_boundary_poses_deep_retained_bytes
        || pose_identity.schedule_binding_fingerprint_v2()
            != schedule.certificate_binding_fingerprint_v2()
        || pose_identity.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || pose_identity.closed_boundary_evidence_binding_fingerprint_v2()
            != boundary.closed_boundary_binding_fingerprint_internal_v2()
        || !pose_identity.replay_policy_matches_v2(
            input.boundary_configuration_replay.schedule_limits,
            pose_limits,
        )
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    Ok(())
}

fn preflight_live_caps_v2(certificate: &CertificateV2, limits: LimitsV2) -> Result<(), ErrorV2> {
    if resources::limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || limits.max_blocks < certificate.actual_block_count_v2()
        || limits.max_hinges < certificate.hinge_count_v2()
        || limits.max_schedule_deep_retained_bytes
            < certificate
                .pose_angle_identity_evidence
                .replay_schedule_deep_retained_bytes_cap_v2()
        || limits.max_representation_boundary_poses_deep_retained_bytes
            < certificate
                .pose_angle_identity_evidence
                .replay_representation_boundary_poses_deep_retained_bytes_cap_v2()
        || limits.max_pose_angle_identity_logical_work
            < certificate.pose_angle_identity_evidence.logical_work_v2()
        || limits.max_pose_angle_identity_workspace_bytes
            < certificate
                .pose_angle_identity_evidence
                .workspace_peak_bytes_upper_bound_v2()
        || limits.max_retained_boundary_configuration_prerequisite_bytes
            < size_of::<CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2>()
        || limits.max_publication_bytes < size_of::<CertificateV2>()
        || limits.max_aggregate_peak_bytes < certificate.resources.aggregate_peak_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

fn checkpoint_v2(checkpoint: &mut impl FnMut() -> Result<(), StopV2>) -> Result<(), ErrorV2> {
    checkpoint().map_err(|stop| match stop {
        StopV2::Cancelled => ErrorV2::Cancelled,
        StopV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
    })
}

const fn map_stop_to_boundary_v2(
    stop: StopV2,
) -> CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2{
    match stop {
        StopV2::Cancelled => CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2::Cancelled,
        StopV2::DeadlineExceeded => CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2::DeadlineExceeded,
    }
}

const fn map_stop_to_pose_v2(
    stop: StopV2,
) -> CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2 {
    match stop {
        StopV2::Cancelled => CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::Cancelled,
        StopV2::DeadlineExceeded => {
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::DeadlineExceeded
        }
    }
}

const fn map_boundary_error_v2(
    error: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
) -> ErrorV2 {
    match error {
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::Cancelled => ErrorV2::Cancelled,
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
        other => ErrorV2::BoundaryConfigurationPositiveThickness(other),
    }
}

const fn map_pose_error_v2(
    error: CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
) -> ErrorV2 {
    match error {
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::Cancelled => {
            ErrorV2::Cancelled
        }
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::DeadlineExceeded => {
            ErrorV2::DeadlineExceeded
        }
        other => ErrorV2::PoseAngleIdentity(other),
    }
}
