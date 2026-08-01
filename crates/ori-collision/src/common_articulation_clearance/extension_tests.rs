use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use super::*;
use crate::common_articulation_extension_test_support::{
    ExtensionClearanceFixtureV1, clearance_extension_limits_v1, issue_extension_clearance_v1,
    pose_extension_limits_v1, prepare_extension_clearance_fixture_v1, raw_pair_candidate_count_v1,
};
use ori_domain::FaceId;
use sha2::{Digest, Sha256};

fn direct_extension_clearance_binding_v1(
    fixture: &ExtensionClearanceFixtureV1,
    authority: &CommonArticulationClearanceExtensionPrerequisiteV1,
    common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    limits: CommonArticulationClearanceExtensionLimitsV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"common_articulation_cross_block_clearance_extension_prerequisite_v1");
    for value in [
        11_u64,
        limits.max_blocks as u64,
        fixture.decomposition.blocks().len() as u64,
    ] {
        hash.update(value.to_le_bytes());
    }
    hash.update(authority.common_pose_binding_fingerprint_v1());
    hash.update(authority.schedule_binding_fingerprint_v1());
    hash.update(authority.closure_binding_fingerprint_v1());
    hash.update(authority.paper_thickness_mm_v1().to_bits().to_be_bytes());
    for value in [
        common_pose_limits.max_blocks,
        common_pose_limits.max_faces,
        common_pose_limits.max_hinges,
        common_pose_limits.max_work,
        common_pose_limits.max_retained_bytes,
        CycleScheduleLimitsV1::default().max_hinges,
        CycleScheduleLimitsV1::default().max_degree,
        CycleScheduleLimitsV1::default().max_work,
        limits.max_blocks,
        limits.max_faces,
        limits.max_cross_block_pairs,
        limits.max_pair_candidates,
        limits.max_work,
        limits.max_storage_bytes,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
    hash.update(
        CycleScheduleLimitsV1::default()
            .max_coefficient_bits
            .to_be_bytes(),
    );
    hash.update((authority.cross_block_pairs_v1().len() as u64).to_be_bytes());
    for pair in authority.cross_block_pairs_v1() {
        hash.update(pair.first().canonical_bytes());
        hash.update(pair.second().canonical_bytes());
    }
    hash.finalize().into()
}

#[test]
fn extension_domain_binds_minimum_configured_cap_and_actual_count_in_order_v1() {
    assert_eq!(COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1, 10);
    assert_eq!(COMMON_ARTICULATION_CLEARANCE_EXTENSION_MIN_BLOCKS_V1, 11);
    assert_eq!(COMMON_ARTICULATION_CLEARANCE_EXTENSION_MAX_BLOCKS_V1, 32);

    for (actual_count, current_cap, replay_cap) in [(11, 11, 12), (12, 12, 13)] {
        let fixture = prepare_extension_clearance_fixture_v1(actual_count);
        let current_pose = fixture.pose_authority_v1(current_cap);
        let replay_pose = fixture.pose_authority_v1(replay_cap);
        let current = issue_extension_clearance_v1(&fixture, &current_pose, current_cap);
        let replay = issue_extension_clearance_v1(&fixture, &replay_pose, replay_cap);

        assert_eq!(
            current.model_id(),
            COMMON_ARTICULATION_CLEARANCE_EXTENSION_PREREQUISITE_MODEL_ID_V1,
        );
        assert_eq!(current.configured_max_blocks_v1(), current_cap);
        assert_eq!(current.actual_block_count_v1(), actual_count);
        assert_eq!(replay.configured_max_blocks_v1(), replay_cap);
        assert_eq!(replay.actual_block_count_v1(), actual_count);
        assert_eq!(
            current.binding_fingerprint_v1(),
            direct_extension_clearance_binding_v1(
                &fixture,
                &current,
                pose_extension_limits_v1(current_cap),
                clearance_extension_limits_v1(current_cap),
            ),
        );
        assert_eq!(
            replay.binding_fingerprint_v1(),
            direct_extension_clearance_binding_v1(
                &fixture,
                &replay,
                pose_extension_limits_v1(replay_cap),
                clearance_extension_limits_v1(replay_cap),
            ),
        );
        assert_ne!(
            current.binding_fingerprint_v1(),
            replay.binding_fingerprint_v1(),
            "configured-cap replay must change the clearance binding",
        );
        current
            .revalidate_v1(fixture.revalidation_input(
                &current_pose,
                pose_extension_limits_v1(current_cap),
                clearance_extension_limits_v1(current_cap),
            ))
            .expect("current extension clearance revalidation");
        replay
            .revalidate_v1(fixture.revalidation_input(
                &replay_pose,
                pose_extension_limits_v1(replay_cap),
                clearance_extension_limits_v1(replay_cap),
            ))
            .expect("replay extension clearance revalidation");
        assert_eq!(
            current
                .revalidate_v1(fixture.revalidation_input(
                    &replay_pose,
                    pose_extension_limits_v1(replay_cap),
                    clearance_extension_limits_v1(replay_cap),
                ))
                .expect_err("foreign cap replay"),
            CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch,
        );
        assert!(!current.authorizes_continuous_motion());
        assert!(!current.authorizes_collision_clearance());
        assert!(!current.authorizes_project_mutation());
        assert!(!current.authorizes_apply());
        assert!(!current.authorizes_viewer());
    }
}

#[test]
fn extension_hard_thirty_two_boundary_is_inclusive_and_other_caps_fail_closed_v1() {
    for invalid in [0, 10, 33, usize::MAX] {
        assert!(
            CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(invalid).is_none()
        );
    }
    assert!(CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(11).is_some());
    assert!(CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(32).is_some());

    let thirty_two = prepare_extension_clearance_fixture_v1(32);
    let pose = thirty_two.pose_authority_v1(32);
    let authority = issue_extension_clearance_v1(&thirty_two, &pose, 32);
    assert_eq!(authority.actual_block_count_v1(), 32);
    assert_eq!(authority.configured_max_blocks_v1(), 32);
    authority
        .revalidate_v1(thirty_two.revalidation_input(
            &pose,
            pose_extension_limits_v1(32),
            clearance_extension_limits_v1(32),
        ))
        .expect("inclusive thirty-two-block clearance revalidation");

    let eleven = prepare_extension_clearance_fixture_v1(11);
    let eleven_pose = eleven.pose_authority_v1(32);
    issue_extension_clearance_v1(&eleven, &eleven_pose, 32);

    // An extension pose authority cannot be minted for ten blocks. Supplying
    // a foreign valid extension authority still reaches the explicit actual
    // cardinality gate before any issuer comparison.
    let ten = prepare_extension_clearance_fixture_v1(10);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(ten.input(
            &eleven_pose,
            pose_extension_limits_v1(32),
            &ten.pairs,
            Some(ten.positive.clone()),
            clearance_extension_limits_v1(32),
        ))
        .expect_err("ten blocks are below the extension minimum"),
        CommonArticulationClearanceErrorV1::ResourceLimit,
    );

    let valid_pose = eleven.pose_authority_v1(11);
    let baseline = clearance_extension_limits_v1(11);
    for invalid_cap in [0, 10, 33, usize::MAX] {
        let invalid = CommonArticulationClearanceExtensionLimitsV1 {
            max_blocks: invalid_cap,
            ..baseline
        };
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(eleven.input(
                &valid_pose,
                pose_extension_limits_v1(11),
                &eleven.pairs,
                Some(eleven.positive.clone()),
                invalid,
            ))
            .expect_err("invalid explicit extension cap"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }
}

#[test]
fn extension_exact_resource_envelope_passes_and_every_one_short_limit_fails_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let baseline = issue_extension_clearance_v1(&fixture, &pose, 11);
    let exact = CommonArticulationClearanceExtensionLimitsV1 {
        max_blocks: 11,
        max_faces: fixture.geometry.face_ids().len(),
        max_cross_block_pairs: fixture.pairs.len(),
        max_pair_candidates: raw_pair_candidate_count_v1(&fixture.decomposition),
        max_work: baseline.logical_work_v1(),
        max_storage_bytes: baseline.storage_bytes_upper_bound_v1(),
    };
    issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
        &pose,
        pose_extension_limits_v1(11),
        &fixture.pairs,
        Some(fixture.positive.clone()),
        exact,
    ))
    .expect("exact extension clearance resource envelope");

    let one_short = [
        CommonArticulationClearanceExtensionLimitsV1 {
            max_blocks: 10,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_cross_block_pairs: exact.max_cross_block_pairs - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_pair_candidates: exact.max_pair_candidates - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_work: exact.max_work - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_storage_bytes: exact.max_storage_bytes - 1,
            ..exact
        },
    ];
    for limits in one_short {
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
                &pose,
                pose_extension_limits_v1(11),
                &fixture.pairs,
                Some(fixture.positive.clone()),
                limits,
            ))
            .expect_err("one-short extension clearance resource"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }

    for overflow in [
        CommonArticulationClearanceExtensionLimitsV1 {
            max_faces: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_cross_block_pairs: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_pair_candidates: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_work: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_storage_bytes: usize::MAX,
            ..exact
        },
    ] {
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
                &pose,
                pose_extension_limits_v1(11),
                &fixture.pairs,
                Some(fixture.positive.clone()),
                overflow,
            ))
            .expect_err("overflowing extension clearance limit"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }
    assert_eq!(
        sort_work_upper_bound_v1(usize::MAX).expect_err("checked sort-work overflow"),
        CommonArticulationClearanceErrorV1::ResourceLimit,
    );
}

#[test]
fn extension_pair_registry_pose_cap_and_whole_parent_provenance_fail_closed_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let pose_limits = pose_extension_limits_v1(11);
    let limits = clearance_extension_limits_v1(11);

    assert!(fixture.pairs.len() > 1);
    assert!(matches!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &fixture.pairs[..fixture.pairs.len() - 1],
            Some(fixture.positive.clone()),
            limits,
        )),
        Err(CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch { .. })
    ));

    let mut duplicate = fixture.pairs.clone();
    duplicate.push(fixture.pairs[0]);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &duplicate,
            Some(fixture.positive.clone()),
            limits,
        ))
        .expect_err("duplicate extension pair"),
        CommonArticulationClearanceErrorV1::DuplicateCrossBlockPair,
    );

    let mut extra = fixture.pairs.clone();
    extra.push(
        CommonArticulationCrossBlockFacePairV1::new(fixture.pairs[0].first(), FaceId::new())
            .expect("extra canonical pair"),
    );
    assert!(matches!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &extra,
            Some(fixture.positive.clone()),
            limits,
        )),
        Err(CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch { .. })
    ));

    let foreign = prepare_extension_clearance_fixture_v1(11);
    let foreign_pose = foreign.pose_authority_v1(11);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &foreign_pose,
            pose_limits,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            limits,
        ))
        .expect_err("foreign extension pose issuer"),
        CommonArticulationClearanceErrorV1::CommonPose(
            CommonArticulationPoseErrorV1::IssuerMismatch
        ),
    );
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &fixture.pairs,
            Some(foreign.positive.clone()),
            limits,
        ))
        .expect_err("foreign whole-parent extension certificate"),
        CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch,
    );

    let cap_twelve = clearance_extension_limits_v1(12);
    for (foreign_pose_limits, foreign_clearance_limits) in [
        (pose_extension_limits_v1(12), cap_twelve),
        (pose_limits, cap_twelve),
        (pose_extension_limits_v1(12), limits),
    ] {
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
                &pose,
                foreign_pose_limits,
                &fixture.pairs,
                Some(fixture.positive.clone()),
                foreign_clearance_limits,
            ))
            .expect_err("inconsistent extension cap"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }
}

#[test]
fn extension_unsupported_gap_is_cap_bound_and_never_authorizes_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose_eleven = fixture.pose_authority_v1(11);
    let pose_twelve = fixture.pose_authority_v1(12);
    let outcome_eleven =
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose_eleven,
            pose_extension_limits_v1(11),
            &fixture.pairs,
            None,
            clearance_extension_limits_v1(11),
        ))
        .expect("cap-eleven unsupported outcome");
    let outcome_twelve =
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose_twelve,
            pose_extension_limits_v1(12),
            &fixture.pairs,
            None,
            clearance_extension_limits_v1(12),
        ))
        .expect("cap-twelve unsupported outcome");
    assert!(!outcome_eleven.is_certified());
    assert!(!outcome_twelve.is_certified());
    let gap_eleven = outcome_eleven.as_gap().expect("cap-eleven gap");
    let gap_twelve = outcome_twelve.as_gap().expect("cap-twelve gap");
    assert_eq!(
        gap_eleven.model_id(),
        COMMON_ARTICULATION_CLEARANCE_EXTENSION_GAP_MODEL_ID_V1,
    );
    assert_eq!(
        gap_eleven.reason(),
        CommonArticulationClearanceUnsupportedReasonV1::WholeParentOpenIntervalProofUnavailable,
    );
    assert_eq!(gap_eleven.configured_max_blocks_v1(), 11);
    assert_eq!(gap_twelve.configured_max_blocks_v1(), 12);
    assert_eq!(gap_eleven.actual_block_count_v1(), 11);
    assert_eq!(gap_twelve.actual_block_count_v1(), 11);
    assert_ne!(
        gap_eleven.common_pose_binding_fingerprint_v1(),
        gap_twelve.common_pose_binding_fingerprint_v1(),
        "the unsupported diagnostic must retain cap-bound pose provenance",
    );
    assert_eq!(gap_eleven.cross_block_pairs_v1(), fixture.pairs);
    assert!(!gap_eleven.endpoint_observations_are_authority_v1());
    assert!(!gap_eleven.sampled_poses_are_authority_v1());
    assert!(!gap_eleven.broad_phase_aabbs_are_authority_v1());
    assert!(!gap_eleven.per_block_certificates_are_cross_block_authority_v1());
    assert!(!gap_eleven.authorizes_continuous_motion());
    assert!(!gap_eleven.authorizes_collision_clearance());
    assert!(!gap_eleven.authorizes_project_mutation());
    assert!(!gap_eleven.authorizes_apply());
    assert!(!gap_eleven.authorizes_viewer());
}

#[test]
fn extension_revalidation_rejects_every_live_binding_and_retained_pair_drift_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let foreign = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let foreign_cap_pose = fixture.pose_authority_v1(12);
    let pose_limits = pose_extension_limits_v1(11);
    let limits = clearance_extension_limits_v1(11);
    let mut authority = issue_extension_clearance_v1(&fixture, &pose, 11);
    let baseline = fixture.revalidation_input(&pose, pose_limits, limits);
    authority
        .revalidate_v1(baseline)
        .expect("baseline extension clearance revalidation");

    let drifted = [
        (
            "geometry",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                geometry: &foreign.geometry,
                ..baseline
            },
        ),
        (
            "pose",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                pose: &foreign.pose,
                ..baseline
            },
        ),
        (
            "decomposition",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                decomposition: &foreign.decomposition,
                ..baseline
            },
        ),
        (
            "schedule",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                schedule: &foreign.schedule,
                ..baseline
            },
        ),
        (
            "closure",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                closure: &foreign.closure,
                ..baseline
            },
        ),
        (
            "thickness",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                paper_thickness_mm: f64::from_bits(fixture.paper_thickness_mm.to_bits() + 1),
                ..baseline
            },
        ),
        (
            "configured cap",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                common_pose: &foreign_cap_pose,
                common_pose_limits: pose_extension_limits_v1(12),
                limits: clearance_extension_limits_v1(12),
                ..baseline
            },
        ),
    ];
    for (label, input) in drifted {
        assert!(
            authority.revalidate_v1(input).is_err(),
            "{label} drift must fail closed",
        );
    }

    authority.cross_block_pairs.pop();
    assert!(
        authority.revalidate_v1(baseline).is_err(),
        "retained cross-block-pair drift must fail closed",
    );
}

#[test]
fn extension_issuance_and_revalidation_stop_at_entry_midpoint_and_final_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let pose_limits = pose_extension_limits_v1(11);
    let limits = clearance_extension_limits_v1(11);

    let mut issuance_checkpoint_count = 0usize;
    issue_common_articulation_clearance_extension_prerequisite_with_checkpoint_v1(
        fixture.input(
            &pose,
            pose_limits,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            limits,
        ),
        &mut || {
            issuance_checkpoint_count += 1;
            Ok(())
        },
    )
    .expect("count extension clearance issuance checkpoints");
    assert!(issuance_checkpoint_count > 4);
    for stop_at in [1, issuance_checkpoint_count / 2, issuance_checkpoint_count] {
        for expected in [
            CommonArticulationClearanceErrorV1::Cancelled,
            CommonArticulationClearanceErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                issue_common_articulation_clearance_extension_prerequisite_with_checkpoint_v1(
                    fixture.input(
                        &pose,
                        pose_limits,
                        &fixture.pairs,
                        Some(fixture.positive.clone()),
                        limits,
                    ),
                    &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("extension clearance issuance stop"),
                expected,
            );
        }
    }

    let authority = issue_extension_clearance_v1(&fixture, &pose, 11);
    let revalidation_input = fixture.revalidation_input(&pose, pose_limits, limits);
    let mut revalidation_checkpoint_count = 0usize;
    authority
        .revalidate_with_checkpoint_v1(revalidation_input, &mut || {
            revalidation_checkpoint_count += 1;
            Ok(())
        })
        .expect("count extension clearance revalidation checkpoints");
    assert!(revalidation_checkpoint_count > 4);
    for stop_at in [
        1,
        revalidation_checkpoint_count / 2,
        revalidation_checkpoint_count,
    ] {
        for expected in [
            CommonArticulationClearanceErrorV1::Cancelled,
            CommonArticulationClearanceErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                authority
                    .revalidate_with_checkpoint_v1(revalidation_input, &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    })
                    .expect_err("extension clearance revalidation stop"),
                expected,
            );
        }
    }

    let cancelled = AtomicBool::new(true);
    let active = AtomicBool::new(false);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_with_control_v1(
            fixture.input(
                &pose,
                pose_limits,
                &fixture.pairs,
                Some(fixture.positive.clone()),
                limits,
            ),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .expect_err("public extension clearance cancellation"),
        CommonArticulationClearanceErrorV1::Cancelled,
    );
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_with_control_v1(
            fixture.input(
                &pose,
                pose_limits,
                &fixture.pairs,
                Some(fixture.positive.clone()),
                limits,
            ),
            &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
        )
        .expect_err("public extension clearance deadline"),
        CommonArticulationClearanceErrorV1::DeadlineExceeded,
    );
}
