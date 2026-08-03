use super::*;

#[test]
fn phase3j_every_outer_policy_field_is_replay_identity() {
    let retained = limits_v2();
    assert!(resources::limits_match_v2(retained, retained));
    for field in 0..9 {
        let mut drifted = retained;
        match field {
            0 => drifted.max_blocks += 1,
            1 => drifted.max_hinges += 1,
            2 => drifted.max_schedule_deep_retained_bytes += 1,
            3 => drifted.max_representation_boundary_poses_deep_retained_bytes += 1,
            4 => drifted.max_pose_angle_identity_logical_work += 1,
            5 => drifted.max_pose_angle_identity_workspace_bytes += 1,
            6 => drifted.max_retained_boundary_configuration_prerequisite_bytes += 1,
            7 => drifted.max_publication_bytes += 1,
            8 => drifted.max_aggregate_peak_bytes += 1,
            _ => unreachable!(),
        }
        assert!(!resources::limits_match_v2(retained, drifted));
    }
}

fn limits_v2() -> CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2{
    CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2 {
        max_blocks: 33,
        max_hinges: 32,
        max_schedule_deep_retained_bytes: 4_096,
        max_representation_boundary_poses_deep_retained_bytes: 8_192,
        max_pose_angle_identity_logical_work: 16_384,
        max_pose_angle_identity_workspace_bytes: 32_768,
        max_retained_boundary_configuration_prerequisite_bytes: 1_024,
        max_publication_bytes: 2_048,
        max_aggregate_peak_bytes: 65_536,
    }
}
