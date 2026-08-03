use super::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteErrorV2;
type StopV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteStopV2;
type CertificateV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteV2;
type InputV2<'a> = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteInputV2<'a>;
type ReplayInputV2<'a> = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteRevalidationInputV2<'a>;

pub(super) fn issue_v2(
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<CertificateV2, ErrorV2> {
    checkpoint_v2(checkpoint)?;
    let resources = resources::checked_resources_v2(
        &input.phase3j,
        &input.transform_realization,
        input.limits,
    )?;
    validate_join_v2(
        &input.phase3j,
        &input.transform_realization,
        input.geometry,
        input.lower_pose,
        input.upper_pose,
    )?;
    let binding_fingerprint = binding::binding_fingerprint_v2(
        &input.phase3j,
        &input.transform_realization,
        resources,
        input.limits,
    )?;
    checkpoint_v2(checkpoint)?;
    Ok(CertificateV2 {
        phase3j: input.phase3j,
        transform_realization: input.transform_realization,
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
    resources::preflight_live_caps_v2(certificate, &input)?;
    if !resources::limits_match_v2(certificate.limits, input.limits)
        || !certificate
            .transform_realization
            .replay_policy_matches_v2(input.transform_realization_limits)
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }

    let transform_input = transform_replay_input_v2(&input);
    validate_join_v2(
        &certificate.phase3j,
        &certificate.transform_realization,
        transform_input.geometry,
        transform_input.lower_pose,
        transform_input.upper_pose,
    )?;
    if certificate.transform_realization.fixed_face_v2() != transform_input.fixed_face {
        return Err(ErrorV2::CertificateBindingMismatch);
    }

    let resources = resources::checked_resources_v2(
        &certificate.phase3j,
        &certificate.transform_realization,
        input.limits,
    )?;
    let binding_fingerprint = binding::binding_fingerprint_v2(
        &certificate.phase3j,
        &certificate.transform_realization,
        resources,
        input.limits,
    )?;
    if certificate.resources != resources || certificate.binding_fingerprint != binding_fingerprint
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }

    certificate
        .phase3j
        .revalidate_with_checkpoint_v2(input.phase3j_replay, || {
            checkpoint().map_err(map_stop_to_phase3j_v2)
        })
        .map_err(map_phase3j_error_v2)?;
    certificate
        .transform_realization
        .revalidate_with_checkpoint_v2(transform_input, || {
            checkpoint().map_err(map_stop_to_transform_v2)
        })
        .map_err(map_transform_error_v2)?;
    checkpoint_v2(checkpoint)
}

fn validate_join_v2(
    phase3j: &CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2,
    transform: &CanonicalBinary64PosePairTransformRealizationEvidenceV2,
    geometry: &MaterialHingeGraphGeometry,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
) -> Result<(), ErrorV2> {
    if !phase3j.matches_pose_instances_v2(lower_pose, upper_pose)
        || !transform.matches_geometry_instance_v2(geometry)
        || !transform.matches_pose_instances_v2(lower_pose, upper_pose)
        || !lower_pose.is_for_geometry(geometry)
        || !upper_pose.is_for_geometry(geometry)
        || lower_pose.fixed_face() != upper_pose.fixed_face()
        || transform.fixed_face_v2() != lower_pose.fixed_face()
        || !phase3j.both_scheduled_angle_representation_points_have_positive_thickness_v2()
        || !transform.proves_both_pose_instances_are_canonical_binary64_transform_realizations_v2()
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    Ok(())
}

fn checkpoint_v2(checkpoint: &mut impl FnMut() -> Result<(), StopV2>) -> Result<(), ErrorV2> {
    checkpoint().map_err(|stop| match stop {
        StopV2::Cancelled => ErrorV2::Cancelled,
        StopV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
    })
}

const fn map_stop_to_phase3j_v2(
    stop: StopV2,
) -> CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2{
    match stop {
        StopV2::Cancelled => CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2::Cancelled,
        StopV2::DeadlineExceeded => CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2::DeadlineExceeded,
    }
}

const fn map_stop_to_transform_v2(
    stop: StopV2,
) -> CanonicalBinary64PosePairTransformRealizationStopV2 {
    match stop {
        StopV2::Cancelled => CanonicalBinary64PosePairTransformRealizationStopV2::Cancelled,
        StopV2::DeadlineExceeded => {
            CanonicalBinary64PosePairTransformRealizationStopV2::DeadlineExceeded
        }
    }
}

const fn map_phase3j_error_v2(
    error: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2,
) -> ErrorV2 {
    match error {
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::Cancelled => ErrorV2::Cancelled,
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
        other => ErrorV2::PoseAngleIdentityPositiveThickness(other),
    }
}

const fn map_transform_error_v2(
    error: CanonicalBinary64PosePairTransformRealizationErrorV2,
) -> ErrorV2 {
    match error {
        CanonicalBinary64PosePairTransformRealizationErrorV2::Cancelled => ErrorV2::Cancelled,
        CanonicalBinary64PosePairTransformRealizationErrorV2::DeadlineExceeded => {
            ErrorV2::DeadlineExceeded
        }
        other => ErrorV2::CanonicalBinary64Transform(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegated_and_final_stop_mappings_remain_fail_closed_v2() {
        for (stop, expected) in [
            (StopV2::Cancelled, ErrorV2::Cancelled),
            (StopV2::DeadlineExceeded, ErrorV2::DeadlineExceeded),
        ] {
            let phase3j = match stop {
                StopV2::Cancelled => CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::Cancelled,
                StopV2::DeadlineExceeded => CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded,
            };
            let transform = match stop {
                StopV2::Cancelled => {
                    CanonicalBinary64PosePairTransformRealizationErrorV2::Cancelled
                }
                StopV2::DeadlineExceeded => {
                    CanonicalBinary64PosePairTransformRealizationErrorV2::DeadlineExceeded
                }
            };
            assert_eq!(map_phase3j_error_v2(phase3j), expected);
            assert_eq!(map_transform_error_v2(transform), expected);
            assert_eq!(checkpoint_v2(&mut || Err(stop)), Err(expected));
        }
    }
}
