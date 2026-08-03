use std::mem::size_of;

use super::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteErrorV2;
type LimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteLimitsV2;
type Phase3JV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
type TransformEvidenceV2 = CanonicalBinary64PosePairTransformRealizationEvidenceV2;
type CertificateV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteV2;

pub(super) fn checked_resources_v2(
    phase3j: &Phase3JV2,
    transform: &TransformEvidenceV2,
    limits: LimitsV2,
) -> Result<Phase3KResourcesV2, ErrorV2> {
    validate_limit_shape_v2(limits)?;
    if phase3j.actual_block_count_v2() > limits.max_blocks
        || phase3j.block_count_cap_internal_v2() != limits.max_blocks
        || phase3j.hinge_count_v2() != transform.hinge_count_v2()
        || phase3j.hinge_count_cap_internal_v2() != limits.max_hinges
        || phase3j.pose_pair_deep_retained_bytes_cap_internal_v2()
            != limits.max_pose_pair_deep_retained_bytes
        || phase3j.scheduled_angle_representation_point_count_v2()
            != transform.realized_pose_count_v2()
        || transform.face_count_v2() > limits.max_faces
        || transform.hinge_count_v2() > limits.max_hinges
        || transform.replay_face_count_cap_v2() != limits.max_faces
        || transform.replay_hinge_count_cap_v2() != limits.max_hinges
        || transform.replay_pose_pair_deep_retained_bytes_cap_v2()
            != limits.max_pose_pair_deep_retained_bytes
        || transform.replay_logical_work_v2() != limits.max_canonical_transform_logical_work
        || transform.logical_work_v2() > limits.max_canonical_transform_logical_work
        || transform.replay_workspace_bytes_cap_v2()
            != limits.max_canonical_transform_workspace_bytes
        || transform.workspace_structural_requirement_bytes_v2()
            > limits.max_canonical_transform_workspace_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }

    let retained_phase3j_prerequisite_bytes = size_of::<Phase3JV2>();
    let retained_transform_realization_evidence_bytes = size_of::<TransformEvidenceV2>();
    let publication_bytes = size_of::<CertificateV2>();
    if retained_phase3j_prerequisite_bytes > limits.max_retained_phase3j_prerequisite_bytes
        || retained_transform_realization_evidence_bytes
            > limits.max_retained_transform_realization_evidence_bytes
        || publication_bytes > limits.max_publication_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }

    let outer_shell_over_phase3j = publication_bytes
        .checked_sub(retained_phase3j_prerequisite_bytes)
        .ok_or(ErrorV2::ResourceLimit)?;
    let delegated_phase3j_replay_peak_bytes = phase3j
        .replay_aggregate_peak_cap_internal_v2()
        .checked_add(outer_shell_over_phase3j)
        .ok_or(ErrorV2::ResourceLimit)?;
    let transform_replay_phase = publication_bytes
        .checked_add(limits.max_pose_pair_deep_retained_bytes)
        .and_then(|value| value.checked_add(limits.max_canonical_transform_workspace_bytes))
        .and_then(|value| value.checked_add(retained_transform_realization_evidence_bytes))
        .ok_or(ErrorV2::ResourceLimit)?;
    let composition_phase = publication_bytes
        .checked_add(COMPOSITION_WORKSPACE_BYTES_V2)
        .ok_or(ErrorV2::ResourceLimit)?;
    let aggregate_peak_bytes = delegated_phase3j_replay_peak_bytes
        .max(transform_replay_phase)
        .max(composition_phase);
    if aggregate_peak_bytes > limits.max_aggregate_peak_bytes {
        return Err(ErrorV2::ResourceLimit);
    }

    Ok(Phase3KResourcesV2 {
        retained_phase3j_prerequisite_bytes,
        retained_transform_realization_evidence_bytes,
        pose_pair_deep_retained_bytes_cap: limits.max_pose_pair_deep_retained_bytes,
        canonical_transform_logical_work: limits.max_canonical_transform_logical_work,
        canonical_transform_workspace_bytes: limits.max_canonical_transform_workspace_bytes,
        delegated_phase3j_replay_peak_bytes,
        composition_workspace_bytes: COMPOSITION_WORKSPACE_BYTES_V2,
        publication_bytes,
        aggregate_peak_bytes,
    })
}

pub(super) fn preflight_live_caps_v2(
    certificate: &CertificateV2,
    input: &CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
) -> Result<(), ErrorV2> {
    validate_limit_shape_v2(input.limits)?;
    let geometry = input.phase3j_replay.boundary_configuration_replay.geometry;
    let transform_limits = input.transform_realization_limits;
    if [
        transform_limits.max_faces,
        transform_limits.max_hinges,
        transform_limits.max_pose_pair_deep_retained_bytes,
        transform_limits.max_logical_work,
        transform_limits.max_workspace_bytes,
    ]
    .into_iter()
    .any(|value| value == 0 || value == usize::MAX)
    {
        return Err(ErrorV2::ResourceLimit);
    }
    let pose_pair_deep_retained_bytes = input
        .phase3j_replay
        .lower_pose
        .checked_deep_retained_bytes_v1()
        .and_then(|value| {
            value.checked_add(
                input
                    .phase3j_replay
                    .upper_pose
                    .checked_deep_retained_bytes_v1()?,
            )
        })
        .ok_or(ErrorV2::ResourceLimit)?;
    if certificate.actual_block_count_v2() > input.limits.max_blocks
        || geometry.face_ids().len() > input.limits.max_faces
        || geometry.hinges().len() > input.limits.max_hinges
        || geometry.face_ids().len() > transform_limits.max_faces
        || geometry.hinges().len() > transform_limits.max_hinges
        || pose_pair_deep_retained_bytes > input.limits.max_pose_pair_deep_retained_bytes
        || pose_pair_deep_retained_bytes > transform_limits.max_pose_pair_deep_retained_bytes
        || certificate.transform_realization.logical_work_v2()
            > input.limits.max_canonical_transform_logical_work
        || certificate.transform_realization.logical_work_v2() > transform_limits.max_logical_work
        || certificate
            .transform_realization
            .workspace_structural_requirement_bytes_v2()
            > input.limits.max_canonical_transform_workspace_bytes
        || certificate
            .transform_realization
            .workspace_structural_requirement_bytes_v2()
            > transform_limits.max_workspace_bytes
        || size_of::<Phase3JV2>() > input.limits.max_retained_phase3j_prerequisite_bytes
        || size_of::<TransformEvidenceV2>()
            > input
                .limits
                .max_retained_transform_realization_evidence_bytes
        || size_of::<CertificateV2>() > input.limits.max_publication_bytes
        || certificate.resources.aggregate_peak_bytes > input.limits.max_aggregate_peak_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

pub(super) const fn limits_match_v2(first: LimitsV2, second: LimitsV2) -> bool {
    first.max_blocks == second.max_blocks
        && first.max_faces == second.max_faces
        && first.max_hinges == second.max_hinges
        && first.max_pose_pair_deep_retained_bytes == second.max_pose_pair_deep_retained_bytes
        && first.max_canonical_transform_logical_work == second.max_canonical_transform_logical_work
        && first.max_canonical_transform_workspace_bytes
            == second.max_canonical_transform_workspace_bytes
        && first.max_retained_phase3j_prerequisite_bytes
            == second.max_retained_phase3j_prerequisite_bytes
        && first.max_retained_transform_realization_evidence_bytes
            == second.max_retained_transform_realization_evidence_bytes
        && first.max_publication_bytes == second.max_publication_bytes
        && first.max_aggregate_peak_bytes == second.max_aggregate_peak_bytes
}

fn validate_limit_shape_v2(limits: LimitsV2) -> Result<(), ErrorV2> {
    if limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

pub(super) const fn limit_values_v2(limits: LimitsV2) -> [usize; 10] {
    [
        limits.max_blocks,
        limits.max_faces,
        limits.max_hinges,
        limits.max_pose_pair_deep_retained_bytes,
        limits.max_canonical_transform_logical_work,
        limits.max_canonical_transform_workspace_bytes,
        limits.max_retained_phase3j_prerequisite_bytes,
        limits.max_retained_transform_realization_evidence_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
}
