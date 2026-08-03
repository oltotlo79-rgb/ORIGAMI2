use super::*;

#[test]
fn representation_boundary_pose_accepts_upper_cap_slack_but_requires_exact_work_and_policy_identity()
 {
    let fixture = ordinary_fixture_v2();
    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();
    for field in 0..5 {
        let exact = limit_value_v2(fixture.limits, field);
        for invalid in [0, exact - 1, usize::MAX] {
            let mut input = fixture.input_v2();
            input.limits = set_limit_v2(fixture.limits, field, invalid);
            assert_eq!(
                prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(input)
                    .unwrap_err(),
                CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit,
                "field {field}, invalid {invalid}",
            );
        }
    }
    for field in [3, 4] {
        let mut input = fixture.input_v2();
        input.limits = set_limit_v2(
            fixture.limits,
            field,
            limit_value_v2(fixture.limits, field) + 1,
        );
        assert_eq!(
            prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
                input,
            )
            .unwrap_err(),
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit,
        );
    }

    let mut slack_limits = fixture.limits;
    slack_limits.max_hinges += 1;
    slack_limits.max_schedule_deep_retained_bytes += 1;
    slack_limits.max_representation_boundary_poses_deep_retained_bytes += 1;
    let mut slack_issue = fixture.input_v2();
    slack_issue.limits = slack_limits;
    let slack_evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            slack_issue,
        )
        .expect("upper caps may retain genuine slack");
    assert!(slack_evidence.hinge_count_v2() < slack_limits.max_hinges);
    let bound = fixture
        .schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &fixture.geometry,
            &fixture.audit,
            &fixture.lower_pose,
            &fixture.upper_pose,
            fixture.schedule_limits,
        )
        .unwrap();
    assert!(
        bound.schedule_deep_retained_bytes_v2() < slack_limits.max_schedule_deep_retained_bytes
    );
    assert!(
        bound.representation_boundary_poses_deep_retained_bytes_v2()
            < slack_limits.max_representation_boundary_poses_deep_retained_bytes
    );
    let mut slack_replay = fixture.input_v2();
    slack_replay.limits = slack_limits;
    slack_evidence.revalidate_v2(slack_replay).unwrap();

    for field in 0..5 {
        let mut policy_drift = fixture.input_v2();
        policy_drift.limits = set_limit_v2(
            fixture.limits,
            field,
            limit_value_v2(fixture.limits, field) + 1,
        );
        assert_eq!(
            evidence.revalidate_v2(policy_drift),
            Err(
                CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
            ),
            "retained policy field {field}",
        );
    }

    let mut schedule_policy_drift = fixture.input_v2();
    schedule_policy_drift.schedule_limits.max_hinges += 1;
    assert_eq!(
        evidence.revalidate_v2(schedule_policy_drift),
        Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
        )
    );
}

#[test]
fn representation_boundary_pose_accepts_degree_zero_constant_ordinary_and_half_angle_schedules() {
    let ordinary = ordinary_constant_fixture_v2();
    assert_eq!(ordinary.schedule_limits.max_degree, 0);
    assert_eq!(ordinary.schedule_limits.max_coefficient_bits, 0);
    let ordinary_evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            ordinary.input_v2(),
        )
        .expect("ordinary constant schedules do not consume rational coefficient bits");
    ordinary_evidence
        .revalidate_v2(ordinary.input_v2())
        .unwrap();

    let half_angle = half_angle_constant_fixture_v2();
    assert_eq!(half_angle.schedule_limits.max_degree, 0);
    assert_eq!(half_angle.schedule_limits.max_coefficient_bits, 1);
    let half_angle_evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            half_angle.input_v2(),
        )
        .expect("half-angle constant schedules admit the exact one-bit coefficient cap");
    half_angle_evidence
        .revalidate_v2(half_angle.input_v2())
        .unwrap();
}

#[test]
fn representation_boundary_pose_preserves_entry_and_deep_stop_precedence() {
    let fixture = half_angle_fixture_v2();
    for (stop, expected) in [
        (
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::Cancelled,
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::Cancelled,
        ),
        (
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::DeadlineExceeded,
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_with_checkpoint_v2(
                fixture.input_v2(),
                || Err(stop),
            )
            .unwrap_err(),
            expected
        );
    }

    let mut polls = 0usize;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_with_checkpoint_v2(
            fixture.input_v2(),
            || {
                polls += 1;
                if polls == 12 {
                    Err(CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::Cancelled)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::Cancelled
    );
    assert_eq!(polls, 12);

    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();
    let mut replay_polls = 0usize;
    assert_eq!(
        evidence.revalidate_with_checkpoint_v2(fixture.input_v2(), || {
            replay_polls += 1;
            if replay_polls == 12 {
                Err(CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::DeadlineExceeded)
            } else {
                Ok(())
            }
        }),
        Err(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::DeadlineExceeded)
    );
    assert_eq!(replay_polls, 12);
}

#[test]
fn representation_boundary_pose_charges_and_checkpoints_its_dedicated_face_order_scan() {
    let fixture = ordinary_fixture_v2();
    let bound = fixture
        .schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &fixture.geometry,
            &fixture.audit,
            &fixture.lower_pose,
            &fixture.upper_pose,
            fixture.schedule_limits,
        )
        .unwrap();
    let expected_graph_work = fixture.geometry.hinges().len()
        + fixture.audit.faces().len()
        + fixture.audit.spanning_hinges().len()
        + fixture.audit.closure_hinges().len()
        + fixture.audit.faces().len()
        + 2;
    assert_eq!(bound.graph_binding_work, expected_graph_work);
    assert_eq!(
        bound.logical_work_required_v2(),
        fixture.limits.max_logical_work
    );
    let mut one_short = fixture.input_v2();
    one_short.limits.max_logical_work -= 1;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            one_short,
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit
    );

    assert!(fixture.audit.faces().len() > 1);
    for (stop, expected) in [
        (
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::Cancelled,
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::Cancelled,
        ),
        (
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2::DeadlineExceeded,
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::DeadlineExceeded,
        ),
    ] {
        let mut polls = 0usize;
        assert_eq!(
            evaluate_pose::validate_live_join_v2(fixture.input_v2(), &mut || {
                polls += 1;
                if polls == 2 { Err(stop) } else { Ok(()) }
            }),
            Err(expected)
        );
        assert_eq!(polls, 2);
    }
}

#[test]
fn representation_boundary_pose_replay_rejects_cheap_mismatches_at_entry() {
    let fixture = ordinary_fixture_v2();
    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();

    let fresh_lower = fixture.fresh_lower_pose_v2();
    let mut fresh_pose = fixture.input_v2();
    fresh_pose.lower_pose = &fresh_lower;
    assert_entry_rejection_v2(&evidence, fresh_pose);

    let mut policy_drift = fixture.input_v2();
    policy_drift.limits.max_schedule_deep_retained_bytes += 1;
    assert_entry_rejection_v2(&evidence, policy_drift);

    let mut drifted_schedule = fixture.schedule.clone();
    drifted_schedule.schedule_fingerprint_v2[0] ^= 0x40;
    let mut schedule_drift = fixture.input_v2();
    schedule_drift.schedule = &drifted_schedule;
    assert_entry_rejection_v2(&evidence, schedule_drift);

    let foreign = half_angle_fixture_v2();
    let mut evidence_drift = fixture.input_v2();
    evidence_drift.closed_boundary_evidence = &foreign.closed_boundary;
    assert_entry_rejection_v2(&evidence, evidence_drift);
}

#[test]
fn representation_boundary_pose_issue_rejects_foreign_closed_header_before_endpoint_evaluation() {
    let fixture = ordinary_fixture_v2();
    let mut valid_polls = 0usize;
    prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_with_checkpoint_v2(
        fixture.input_v2(),
        || {
            valid_polls += 1;
            Ok(())
        },
    )
    .unwrap();

    let foreign = half_angle_fixture_v2();
    let mut invalid = fixture.input_v2();
    invalid.closed_boundary_evidence = &foreign.closed_boundary;
    let mut invalid_polls = 0usize;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_with_checkpoint_v2(
            invalid,
            || {
                invalid_polls += 1;
                Ok(())
            },
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ClosedBoundaryEvidenceMismatch
    );
    assert!(invalid_polls < valid_polls);
}

#[test]
fn representation_boundary_pose_binding_is_deterministic_and_resource_metrics_are_exact() {
    let fixture = ordinary_fixture_v2();
    let first =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();
    let second =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();
    assert_eq!(
        first.binding_fingerprint_v2(),
        second.binding_fingerprint_v2()
    );
    assert_eq!(
        first.schedule_deep_retained_bytes_upper_bound_v2(),
        fixture.limits.max_schedule_deep_retained_bytes
    );
    assert_eq!(
        first.representation_boundary_poses_deep_retained_bytes_upper_bound_v2(),
        fixture
            .limits
            .max_representation_boundary_poses_deep_retained_bytes
    );
    assert_eq!(first.logical_work_v2(), fixture.limits.max_logical_work);
    assert_eq!(
        first.workspace_peak_bytes_upper_bound_v2(),
        fixture.limits.max_workspace_bytes
    );
    let debug = format!("{first:?}");
    for secret in [
        "issuer_geometry",
        "lower_pose_instance",
        "upper_pose_instance",
        "schedule_binding",
        "graph_binding",
        "closed_boundary",
        "binding_fingerprint",
        "hinge_angles",
        "transforms",
        "closure",
        "tolerance",
    ] {
        assert!(!debug.contains(secret), "Debug leaked {secret}");
    }
}

#[test]
fn half_angle_resource_formula_charges_point_and_exact_box_coefficients_at_both_boundaries() {
    let fixture = half_angle_fixture_v2();
    let bound = fixture
        .schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &fixture.geometry,
            &fixture.audit,
            &fixture.lower_pose,
            &fixture.upper_pose,
            fixture.schedule_limits,
        )
        .unwrap();
    let power_count = fixture.schedule.half_angle_entries[0]
        .numerator_power_coefficients
        .len()
        + fixture.schedule.half_angle_entries[0]
            .denominator_power_coefficients
            .len();
    let expected_pose_evaluation = power_count * 4 + (fixture.schedule_limits.max_work + 5) * 2;
    let actual_pose_evaluation = bound.logical_work_required
        - bound.closed_boundary_bound.logical_work_required_v2()
        - bound.pose_retained_scan_work
        - bound.graph_binding_work
        - pose_resources::POSE_IDENTITY_FIXED_WORK_V2
        - binding::checked_binding_work_v2();
    assert_eq!(actual_pose_evaluation, expected_pose_evaluation);
}

fn assert_entry_rejection_v2(
    evidence: &CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
    input: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'_>,
) {
    let mut polls = 0usize;
    assert_eq!(
        evidence.revalidate_with_checkpoint_v2(input, || {
            polls += 1;
            Ok(())
        }),
        Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
        )
    );
    assert_eq!(polls, 1);
}

const fn limit_value_v2(
    limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    field: usize,
) -> usize {
    match field {
        0 => limits.max_hinges,
        1 => limits.max_schedule_deep_retained_bytes,
        2 => limits.max_representation_boundary_poses_deep_retained_bytes,
        3 => limits.max_logical_work,
        4 => limits.max_workspace_bytes,
        _ => unreachable!(),
    }
}

const fn set_limit_v2(
    mut limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    field: usize,
    value: usize,
) -> CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
    match field {
        0 => limits.max_hinges = value,
        1 => limits.max_schedule_deep_retained_bytes = value,
        2 => limits.max_representation_boundary_poses_deep_retained_bytes = value,
        3 => limits.max_logical_work = value,
        4 => limits.max_workspace_bytes = value,
        _ => unreachable!(),
    }
    limits
}
