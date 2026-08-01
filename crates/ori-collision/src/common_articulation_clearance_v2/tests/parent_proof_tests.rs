//! Parent-proof binding and cooperative-stop tests for clearance V2.

use super::super::test_support::{golden_n33_miura_fixture_v2, miura_fixture_v2};
use super::super::*;

#[test]
fn n33_fixed_fixture_binds_parent_proofs_and_limits() {
    let fixture = golden_n33_miura_fixture_v2();
    let first = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("fixed N33 parent-bound prerequisite");
    let second = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("fixed N33 replay");
    let prerequisite = first.as_unpromoted_v2();

    assert_eq!(
        prerequisite.block_closure_set_binding_fingerprint_v2(),
        fixture.block_closure_set.binding_fingerprint_v2(),
    );
    assert_eq!(
        prerequisite.whole_parent_closure_binding_fingerprint_v2(),
        fixture.whole_parent_closure.binding_fingerprint_v2(),
    );
    assert_eq!(
        prerequisite.parent_schedule_binding_fingerprint_v2(),
        fixture.parent_schedule.certificate_binding_fingerprint_v2(),
    );
    assert_eq!(
        prerequisite.parent_fixed_face_v2(),
        fixture.parent_fixed_face
    );
    assert_eq!(
        prerequisite.closure_tolerance_v2().to_bits(),
        fixture.closure_tolerance.to_bits(),
    );
    assert_eq!(
        prerequisite.whole_parent_closure_limits_v2(),
        fixture.whole_parent_closure_limits,
    );
    assert_eq!(
        prerequisite.binding_fingerprint_v2(),
        second.as_unpromoted_v2().binding_fingerprint_v2(),
    );
    assert_eq!(
        prerequisite.binding_fingerprint_v2(),
        [
            31, 213, 86, 253, 40, 25, 30, 34, 189, 133, 91, 245, 247, 144, 107, 83, 83, 53, 0, 148,
            17, 238, 201, 106, 205, 246, 141, 184, 182, 98, 177, 22,
        ],
    );
}

#[test]
fn parent_proof_sources_tolerance_and_limits_reject_single_substitutions() {
    let fixture = miura_fixture_v2();
    let foreign = miura_fixture_v2();
    let prerequisite = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("base parent-bound prerequisite");
    let prerequisite = prerequisite.as_unpromoted_v2();

    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                geometry: &foreign.geometry,
                ..fixture.revalidation_input()
            })
            .expect_err("foreign geometry"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );
    assert!(matches!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                common_pose: &foreign.common_pose,
                ..fixture.revalidation_input()
            })
            .expect_err("foreign common pose"),
        CommonArticulationClearanceErrorV2::CommonPose(_),
    ));
    let same_actual_different_configured =
        CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
            .expect("same actual N, configured cap drift");
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                profile: &same_actual_different_configured,
                ..fixture.revalidation_input()
            })
            .expect_err("same actual N with different configured cap"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );

    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                parent_fixed_face: foreign.parent_fixed_face,
                ..fixture.revalidation_input()
            })
            .expect_err("foreign parent fixed face"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                ori_kinematics::CommonArticulationBlockClosureSetErrorV2::InvalidInput,
            ),
        ),
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                parent_schedule: &foreign.parent_schedule,
                ..fixture.revalidation_input()
            })
            .expect_err("foreign parent schedule"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                ori_kinematics::CommonArticulationBlockClosureSetErrorV2::InvalidInput,
            ),
        ),
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                block_closure_set: &foreign.block_closure_set,
                ..fixture.revalidation_input()
            })
            .expect_err("foreign all-block closure set"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                ori_kinematics::CommonArticulationBlockClosureSetErrorV2::IssuerMismatch,
            ),
        ),
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                whole_parent_closure: &foreign.whole_parent_closure,
                ..fixture.revalidation_input()
            })
            .expect_err("foreign whole-parent closure"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::IssuerMismatch,
        ),
    );

    let one_ulp_tolerance = f64::from_bits(fixture.closure_tolerance.to_bits() + 1);
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                closure_tolerance: one_ulp_tolerance,
                ..fixture.revalidation_input()
            })
            .expect_err("one ULP tolerance drift"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                ori_kinematics::CommonArticulationBlockClosureSetErrorV2::IssuerMismatch,
            ),
        ),
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                closure_tolerance: -0.0,
                ..fixture.revalidation_input()
            })
            .expect_err("negative-zero tolerance"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                ori_kinematics::CommonArticulationBlockClosureSetErrorV2::InvalidInput,
            ),
        ),
    );

    let drifted_limits = ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
        max_parent_closure_bytes: fixture.whole_parent_closure_limits.max_parent_closure_bytes + 1,
        ..fixture.whole_parent_closure_limits
    };
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                whole_parent_closure_limits: drifted_limits,
                ..fixture.revalidation_input()
            })
            .expect_err("whole-parent closure limits drift"),
        CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::IssuerMismatch,
        ),
    );
}

#[test]
fn each_positive_whole_parent_limit_rejects_its_one_short_replay() {
    let fixture = miura_fixture_v2();
    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("base prerequisite");
    let prerequisite = outcome.as_unpromoted_v2();
    let limits = fixture.whole_parent_closure_limits;
    let block = limits.block_closure_set_limits;
    let per_block = block.per_block_closure_limits;
    let parent = limits.parent_closure_limits;
    let one_short = [
        (
            "block max blocks",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_blocks: block.max_blocks - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "block parent schedule bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_parent_schedule_bytes: block.max_parent_schedule_bytes - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "block schedule bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_block_schedule_bytes: block.max_block_schedule_bytes - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "total block schedule bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_total_block_schedule_bytes: block.max_total_block_schedule_bytes - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "block closure bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_block_closure_bytes: block.max_block_closure_bytes - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "total block closure bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_total_block_closure_bytes: block.max_total_block_closure_bytes - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "total closure leaves",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        max_total_closure_leaves: block.max_total_closure_leaves - 1,
                        ..block
                    },
                ..limits
            },
        ),
        (
            "per block leaves",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        per_block_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                            max_leaves: per_block.max_leaves - 1,
                            ..per_block
                        },
                        ..block
                    },
                ..limits
            },
        ),
        (
            "per block work",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        per_block_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                            max_work: per_block.max_work - 1,
                            ..per_block
                        },
                        ..block
                    },
                ..limits
            },
        ),
        (
            "per block schedule hinges",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        per_block_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                            schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                                max_hinges: per_block.schedule_limits.max_hinges - 1,
                                ..per_block.schedule_limits
                            },
                            ..per_block
                        },
                        ..block
                    },
                ..limits
            },
        ),
        (
            "per block coefficient bits",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        per_block_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                            schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                                max_coefficient_bits: per_block
                                    .schedule_limits
                                    .max_coefficient_bits
                                    - 1,
                                ..per_block.schedule_limits
                            },
                            ..per_block
                        },
                        ..block
                    },
                ..limits
            },
        ),
        (
            "per block schedule work",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                block_closure_set_limits:
                    ori_kinematics::CommonArticulationBlockClosureSetLimitsV2 {
                        per_block_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                            schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                                max_work: per_block.schedule_limits.max_work - 1,
                                ..per_block.schedule_limits
                            },
                            ..per_block
                        },
                        ..block
                    },
                ..limits
            },
        ),
        (
            "parent schedule bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                max_parent_schedule_bytes: limits.max_parent_schedule_bytes - 1,
                ..limits
            },
        ),
        (
            "parent closure bytes",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                max_parent_closure_bytes: limits.max_parent_closure_bytes - 1,
                ..limits
            },
        ),
        (
            "parent closure leaves",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                max_parent_closure_leaves: limits.max_parent_closure_leaves - 1,
                ..limits
            },
        ),
        (
            "parent dyadic leaves",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                parent_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_leaves: parent.max_leaves - 1,
                    ..parent
                },
                ..limits
            },
        ),
        (
            "parent dyadic work",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                parent_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_work: parent.max_work - 1,
                    ..parent
                },
                ..limits
            },
        ),
        (
            "parent schedule hinges",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                parent_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                        max_hinges: parent.schedule_limits.max_hinges - 1,
                        ..parent.schedule_limits
                    },
                    ..parent
                },
                ..limits
            },
        ),
        (
            "parent coefficient bits",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                parent_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                        max_coefficient_bits: parent.schedule_limits.max_coefficient_bits - 1,
                        ..parent.schedule_limits
                    },
                    ..parent
                },
                ..limits
            },
        ),
        (
            "parent schedule work",
            ori_kinematics::CommonArticulationWholeParentClosureLimitsV2 {
                parent_closure_limits: ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1 {
                        max_work: parent.schedule_limits.max_work - 1,
                        ..parent.schedule_limits
                    },
                    ..parent
                },
                ..limits
            },
        ),
    ];

    for (label, one_short) in one_short {
        assert!(
            prerequisite
                .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                    whole_parent_closure_limits: one_short,
                    ..fixture.revalidation_input()
                })
                .is_err(),
            "{label} one-short limit must fail closed",
        );
    }
}

#[test]
fn issue_and_revalidation_normalize_entry_mid_and_final_stops() {
    let fixture = miura_fixture_v2();
    let mut issue_polls = 0usize;
    let issued = issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(
        fixture.input(),
        || {
            issue_polls += 1;
            Ok(())
        },
    )
    .expect("count issue checkpoints");
    assert!(issue_polls >= 3, "entry, nested, and publication polls");
    for (at, stop, expected) in [
        (
            1,
            CommonArticulationClearanceStopV2::Cancelled,
            CommonArticulationClearanceErrorV2::Cancelled,
        ),
        (
            (issue_polls / 2).max(2),
            CommonArticulationClearanceStopV2::DeadlineExceeded,
            CommonArticulationClearanceErrorV2::DeadlineExceeded,
        ),
        (
            issue_polls,
            CommonArticulationClearanceStopV2::Cancelled,
            CommonArticulationClearanceErrorV2::Cancelled,
        ),
    ] {
        let mut polls = 0usize;
        assert_eq!(
            issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(
                fixture.input(),
                || {
                    polls += 1;
                    if polls == at { Err(stop) } else { Ok(()) }
                },
            )
            .expect_err("entry/mid/final issue stop must hide the candidate"),
            expected,
            "issue stop at checkpoint {at}",
        );
    }

    let prerequisite = issued.as_unpromoted_v2();
    let mut revalidation_polls = 0usize;
    prerequisite
        .revalidate_with_checkpoint_v2(fixture.revalidation_input(), || {
            revalidation_polls += 1;
            Ok(())
        })
        .expect("count revalidation checkpoints");
    assert!(
        revalidation_polls >= 3,
        "entry, nested, and final revalidation polls"
    );
    for (at, stop, expected) in [
        (
            1,
            CommonArticulationClearanceStopV2::DeadlineExceeded,
            CommonArticulationClearanceErrorV2::DeadlineExceeded,
        ),
        (
            (revalidation_polls / 2).max(2),
            CommonArticulationClearanceStopV2::Cancelled,
            CommonArticulationClearanceErrorV2::Cancelled,
        ),
        (
            revalidation_polls,
            CommonArticulationClearanceStopV2::DeadlineExceeded,
            CommonArticulationClearanceErrorV2::DeadlineExceeded,
        ),
    ] {
        let mut polls = 0usize;
        assert_eq!(
            prerequisite
                .revalidate_with_checkpoint_v2(fixture.revalidation_input(), || {
                    polls += 1;
                    if polls == at { Err(stop) } else { Ok(()) }
                })
                .expect_err("entry/mid/final revalidation stop must not authenticate"),
            expected,
            "revalidation stop at checkpoint {at}",
        );
    }
}

#[test]
fn revalidation_honors_cancel_and_deadline_inside_large_pair_comparison() {
    let fixture = miura_fixture_v2();
    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("base prerequisite");
    let prerequisite = outcome.as_unpromoted_v2();
    let pair_count = prerequisite.cross_block_pairs_v2().len();
    assert!(
        pair_count >= 2,
        "general-N fixture has a large pair registry"
    );

    let mut successful_polls = 0usize;
    prerequisite
        .revalidate_with_checkpoint_v2(fixture.revalidation_input(), || {
            successful_polls += 1;
            Ok(())
        })
        .expect("count pair-comparison revalidation checkpoints");
    let deep_pair_poll = successful_polls
        .checked_sub(pair_count / 2)
        .expect("the successful revalidation reaches the pair comparison");
    assert!(deep_pair_poll > 1, "mid-pair checkpoint is not entry");

    for (stop, expected) in [
        (
            CommonArticulationClearanceStopV2::Cancelled,
            CommonArticulationClearanceErrorV2::Cancelled,
        ),
        (
            CommonArticulationClearanceStopV2::DeadlineExceeded,
            CommonArticulationClearanceErrorV2::DeadlineExceeded,
        ),
    ] {
        let mut polls = 0usize;
        assert_eq!(
            prerequisite
                .revalidate_with_checkpoint_v2(fixture.revalidation_input(), || {
                    polls += 1;
                    if polls == deep_pair_poll {
                        Err(stop)
                    } else {
                        Ok(())
                    }
                })
                .expect_err("deep pair comparison stop must hide revalidation success"),
            expected,
        );
    }
}
