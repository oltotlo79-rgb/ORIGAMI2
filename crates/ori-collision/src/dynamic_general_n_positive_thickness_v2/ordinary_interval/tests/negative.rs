use super::super::*;
use super::support::{OrdinaryFixtureV2, bridge_limits_v2, strict_limits_v2};

pub(super) fn assert_overlapping_ordinary_pair_is_not_certified_v2(fixture: &OrdinaryFixtureV2) {
    let thickness = 1.0e6;
    let common_pose = ori_kinematics::prove_common_articulation_pose_authority_v2(
        ori_kinematics::CommonArticulationPoseInputV2 {
            geometry: &fixture.fixture.geometry,
            pose: &fixture.pose,
            decomposition: &fixture.fixture.decomposition,
            paper_thickness_mm: thickness,
            profile: &fixture.fixture.profile,
        },
    )
    .expect("thick diagnostic common pose");
    let bridge = ori_kinematics::prove_common_articulation_dynamic_closure_bridge_v2(
        ori_kinematics::CommonArticulationDynamicClosureBridgeInputV2 {
            geometry: &fixture.fixture.geometry,
            audit: &fixture.fixture.audit,
            pose: &fixture.pose,
            parent_fixed_face: fixture.fixture.parent_fixed_face,
            parent_schedule: &fixture.schedule,
            decomposition: &fixture.fixture.decomposition,
            common_pose: &common_pose,
            paper_thickness_mm: thickness,
            closure_tolerance: fixture.fixture.closure_tolerance,
            profile: &fixture.fixture.profile,
            limits: bridge_limits_v2(fixture.fixture.profile.actual_block_count_v2()),
        },
    )
    .expect("thick diagnostic closure bridge");
    let input = OrdinaryIntervalInputV2 {
        geometry: &fixture.fixture.geometry,
        audit: &fixture.fixture.audit,
        pose: &fixture.pose,
        fixed_face: fixture.fixture.parent_fixed_face,
        schedule: &fixture.schedule,
        decomposition: &fixture.fixture.decomposition,
        common_pose: &common_pose,
        profile: &fixture.fixture.profile,
        dynamic_closure_bridge: &bridge,
        paper_thickness_mm: thickness,
        closure_tolerance: fixture.fixture.closure_tolerance,
        excluded_shared_pairs: &fixture.excluded_shared_pairs,
        limits: strict_limits_v2(fixture),
    };
    assert_eq!(
        prove_ordinary_interval_clearance_v2(input).unwrap_err(),
        OrdinaryIntervalErrorV2::UnprovenOrdinaryClearance
    );
}
