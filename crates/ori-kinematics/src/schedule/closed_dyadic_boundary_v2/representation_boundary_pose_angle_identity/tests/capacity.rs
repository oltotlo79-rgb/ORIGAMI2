use super::*;

#[test]
fn representation_boundary_pose_schedule_byte_cap_allows_semantic_capacity_drift() {
    let fixture = ordinary_fixture_v2();
    let original_bound = fixture
        .schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &fixture.geometry,
            &fixture.audit,
            &fixture.lower_pose,
            &fixture.upper_pose,
            fixture.schedule_limits,
        )
        .unwrap();
    let mut drifted_schedule = fixture.schedule.clone();
    drifted_schedule.entries.reserve_exact(4);
    drifted_schedule.entries[0].coefficients.reserve_exact(8);
    assert_eq!(
        drifted_schedule.certificate_binding_fingerprint_v2(),
        fixture.schedule.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        drifted_schedule.graph_binding_fingerprint_v1(),
        fixture.schedule.graph_binding_fingerprint_v1()
    );
    let drifted_bound = drifted_schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &fixture.geometry,
            &fixture.audit,
            &fixture.lower_pose,
            &fixture.upper_pose,
            fixture.schedule_limits,
        )
        .unwrap();
    assert!(
        original_bound.schedule_deep_retained_bytes_v2()
            < drifted_bound.schedule_deep_retained_bytes_v2()
    );

    let mut shared_limits = fixture.limits;
    shared_limits.max_schedule_deep_retained_bytes =
        drifted_bound.schedule_deep_retained_bytes_v2();
    let mut issue = fixture.input_v2();
    issue.limits = shared_limits;
    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            issue,
        )
        .expect("the retained schedule byte metric is the shared policy cap");
    assert_eq!(
        evidence.schedule_deep_retained_bytes_upper_bound_v2(),
        shared_limits.max_schedule_deep_retained_bytes
    );

    let mut capacity_drift_replay = fixture.input_v2();
    capacity_drift_replay.schedule = &drifted_schedule;
    capacity_drift_replay.limits = shared_limits;
    evidence
        .revalidate_v2(capacity_drift_replay)
        .expect("semantic replay admits allocator-capacity drift within the retained cap");

    let mut one_short = fixture.input_v2();
    one_short.schedule = &drifted_schedule;
    one_short.limits = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
        max_schedule_deep_retained_bytes: shared_limits.max_schedule_deep_retained_bytes - 1,
        ..shared_limits
    };
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            one_short,
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit
    );

    let mut policy_drift = fixture.input_v2();
    policy_drift.limits = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
        max_schedule_deep_retained_bytes: shared_limits.max_schedule_deep_retained_bytes + 1,
        ..shared_limits
    };
    assert_eq!(
        evidence.revalidate_v2(policy_drift),
        Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
        )
    );
}
