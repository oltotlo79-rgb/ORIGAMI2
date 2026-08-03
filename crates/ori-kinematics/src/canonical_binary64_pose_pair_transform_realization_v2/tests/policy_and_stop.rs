use super::*;

#[test]
fn every_limit_rejects_zero_one_short_and_max_v2() {
    let fixture = fixture_v2();
    let bound = fixture
        .geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &fixture.audit,
            &fixture.lower_pose,
            &fixture.upper_pose,
        )
        .unwrap();
    let one_short = [
        bound.face_count_v2() - 1,
        bound.hinge_count_v2() - 1,
        bound.pose_pair_deep_retained_bytes_v2() - 1,
        bound.logical_work_required_v2() - 1,
        bound.workspace_structural_requirement_bytes_v2() - 1,
    ];
    for (field, one_short) in one_short.into_iter().enumerate() {
        for invalid in [0, one_short, usize::MAX] {
            let mut limits = fixture.limits;
            set_limit_v2(&mut limits, field, invalid);
            let input = CanonicalBinary64PosePairTransformRealizationInputV2 {
                limits,
                ..fixture.input_v2()
            };
            assert!(
                matches!(
                    prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(input),
                    Err(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit)
                ),
                "field {field}, invalid {invalid}"
            );
        }
    }
    let mut non_exact_work = fixture.limits;
    non_exact_work.max_logical_work += 1;
    assert!(matches!(
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(
            CanonicalBinary64PosePairTransformRealizationInputV2 {
                limits: non_exact_work,
                ..fixture.input_v2()
            }
        ),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit)
    ));
}

#[test]
fn valid_slack_is_retained_but_replay_policy_drift_fails_cheaply_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    assert!(evidence.face_count_v2() < evidence.replay_face_count_cap_v2());
    assert!(evidence.hinge_count_v2() < evidence.replay_hinge_count_cap_v2());
    assert!(
        evidence.pose_pair_deep_retained_bytes_v2()
            < evidence.replay_pose_pair_deep_retained_bytes_cap_v2()
    );
    assert_eq!(
        evidence.logical_work_v2(),
        evidence.replay_logical_work_v2()
    );
    assert!(
        evidence.workspace_structural_requirement_bytes_v2()
            < evidence.workspace_peak_bytes_upper_bound_v2()
    );
    assert_eq!(
        evidence.workspace_peak_bytes_upper_bound_v2(),
        evidence.replay_workspace_bytes_cap_v2()
    );
    for field in 0..5 {
        let mut drifted = fixture.limits;
        let value = limit_value_v2(drifted, field) + 1;
        set_limit_v2(&mut drifted, field, value);
        let mut polls = 0usize;
        let result = evidence.revalidate_with_checkpoint_v2(
            CanonicalBinary64PosePairTransformRealizationInputV2 {
                limits: drifted,
                ..fixture.input_v2()
            },
            || {
                polls += 1;
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(CanonicalBinary64PosePairTransformRealizationErrorV2::CertificateBindingMismatch)
        );
        assert_eq!(polls, 1);
    }
}

#[test]
fn replay_resource_failures_precede_policy_binding_mismatch_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    let one_short = [
        evidence.face_count_v2() - 1,
        evidence.hinge_count_v2() - 1,
        evidence.pose_pair_deep_retained_bytes_v2() - 1,
        evidence.logical_work_v2() - 1,
        evidence.workspace_structural_requirement_bytes_v2() - 1,
    ];
    for (field, one_short) in one_short.into_iter().enumerate() {
        for invalid in [0, one_short, usize::MAX] {
            let mut limits = fixture.limits;
            set_limit_v2(&mut limits, field, invalid);
            let mut polls = 0usize;
            let result = evidence.revalidate_with_checkpoint_v2(
                CanonicalBinary64PosePairTransformRealizationInputV2 {
                    limits,
                    ..fixture.input_v2()
                },
                || {
                    polls += 1;
                    Ok(())
                },
            );
            assert_eq!(
                result,
                Err(CanonicalBinary64PosePairTransformRealizationErrorV2::ResourceLimit),
                "field {field}, invalid {invalid}"
            );
            assert_eq!(polls, 1, "field {field}, invalid {invalid}");
        }
    }
}

#[test]
fn branching_large_face_issue_poll_count_is_within_logical_work_v2() {
    let fixture = branching_fixture_v2(65);
    let mut polls = 0usize;
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_with_checkpoint_v2(
            fixture.input_v2(),
            || {
                polls += 1;
                Ok(())
            },
        )
        .unwrap();
    assert!(polls <= evidence.logical_work_v2());
}

#[test]
fn issue_honors_every_cancel_and_deadline_checkpoint_v2() {
    let fixture = fixture_v2();
    let mut successful_polls = 0usize;
    prove_canonical_binary64_pose_pair_transform_realization_evidence_with_checkpoint_v2(
        fixture.input_v2(),
        || {
            successful_polls += 1;
            Ok(())
        },
    )
    .unwrap();
    assert!(successful_polls > 50);
    for stop in [
        CanonicalBinary64PosePairTransformRealizationStopV2::Cancelled,
        CanonicalBinary64PosePairTransformRealizationStopV2::DeadlineExceeded,
    ] {
        let expected = match stop {
            CanonicalBinary64PosePairTransformRealizationStopV2::Cancelled => {
                CanonicalBinary64PosePairTransformRealizationErrorV2::Cancelled
            }
            CanonicalBinary64PosePairTransformRealizationStopV2::DeadlineExceeded => {
                CanonicalBinary64PosePairTransformRealizationErrorV2::DeadlineExceeded
            }
        };
        for stop_at in 0..successful_polls {
            let mut polls = 0usize;
            let result =
                prove_canonical_binary64_pose_pair_transform_realization_evidence_with_checkpoint_v2(
                    fixture.input_v2(),
                    || {
                        if polls == stop_at {
                            return Err(stop);
                        }
                        polls += 1;
                        Ok(())
                    },
                );
            assert!(
                result.is_err_and(|error| error == expected),
                "stop {stop:?} at {stop_at}"
            );
        }
    }
}

#[test]
fn replay_honors_every_cancel_and_deadline_checkpoint_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    let mut successful_polls = 0usize;
    evidence
        .revalidate_with_checkpoint_v2(fixture.input_v2(), || {
            successful_polls += 1;
            Ok(())
        })
        .unwrap();
    for stop in [
        CanonicalBinary64PosePairTransformRealizationStopV2::Cancelled,
        CanonicalBinary64PosePairTransformRealizationStopV2::DeadlineExceeded,
    ] {
        let expected = match stop {
            CanonicalBinary64PosePairTransformRealizationStopV2::Cancelled => {
                CanonicalBinary64PosePairTransformRealizationErrorV2::Cancelled
            }
            CanonicalBinary64PosePairTransformRealizationStopV2::DeadlineExceeded => {
                CanonicalBinary64PosePairTransformRealizationErrorV2::DeadlineExceeded
            }
        };
        for stop_at in 0..successful_polls {
            let mut polls = 0usize;
            let result = evidence.revalidate_with_checkpoint_v2(fixture.input_v2(), || {
                if polls == stop_at {
                    return Err(stop);
                }
                polls += 1;
                Ok(())
            });
            assert_eq!(result, Err(expected), "stop {stop:?} at {stop_at}");
        }
    }
}

fn set_limit_v2(
    limits: &mut CanonicalBinary64PosePairTransformRealizationLimitsV2,
    field: usize,
    value: usize,
) {
    match field {
        0 => limits.max_faces = value,
        1 => limits.max_hinges = value,
        2 => limits.max_pose_pair_deep_retained_bytes = value,
        3 => limits.max_logical_work = value,
        4 => limits.max_workspace_bytes = value,
        _ => unreachable!(),
    }
}

const fn limit_value_v2(
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
    field: usize,
) -> usize {
    match field {
        0 => limits.max_faces,
        1 => limits.max_hinges,
        2 => limits.max_pose_pair_deep_retained_bytes,
        3 => limits.max_logical_work,
        4 => limits.max_workspace_bytes,
        _ => 0,
    }
}
