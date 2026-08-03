use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn phase3k_replay_input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    fresh_authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    boundary_limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
    phase3j_limits: Phase3JLimitsV2,
    lower_pose: &'a ClosedMaterialHingeGraphPose,
    upper_pose: &'a ClosedMaterialHingeGraphPose,
    transform_limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
    phase3k_limits: Phase3KLimitsV2,
) -> CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteRevalidationInputV2<'a>{
    CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteRevalidationInputV2 {
        phase3j_replay: phase3j_replay_input_v2(
            boundary_configuration_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                endpoint_limits,
                schedule_limits,
                boundary_limits,
            ),
            &fixture.fixture.audit,
            lower_pose,
            upper_pose,
            phase3j_limits,
        ),
        transform_realization_limits: transform_limits,
        limits: phase3k_limits,
    }
}

pub(super) const fn phase3k_limit_values_v2(limits: Phase3KLimitsV2) -> [usize; 10] {
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

pub(super) fn set_phase3k_limit_v2(
    mut limits: Phase3KLimitsV2,
    field: usize,
    value: usize,
) -> Phase3KLimitsV2 {
    match field {
        0 => limits.max_blocks = value,
        1 => limits.max_faces = value,
        2 => limits.max_hinges = value,
        3 => limits.max_pose_pair_deep_retained_bytes = value,
        4 => limits.max_canonical_transform_logical_work = value,
        5 => limits.max_canonical_transform_workspace_bytes = value,
        6 => limits.max_retained_phase3j_prerequisite_bytes = value,
        7 => limits.max_retained_transform_realization_evidence_bytes = value,
        8 => limits.max_publication_bytes = value,
        9 => limits.max_aggregate_peak_bytes = value,
        _ => unreachable!(),
    }
    limits
}
