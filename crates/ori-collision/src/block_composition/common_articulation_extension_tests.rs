use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CommonArticulationPoseExtensionAuthorityV1, CommonArticulationPoseExtensionLimitsV1,
    CycleScheduleLimitsV1,
};
use sha2::{Digest, Sha256};

use super::*;
use crate::{
    CommonArticulationClearanceExtensionLimitsV1,
    CommonArticulationClearanceExtensionPrerequisiteV1,
    common_articulation_extension_test_support::{
        ExtensionClearanceFixtureV1, clearance_extension_limits_v1, issue_extension_clearance_v1,
        pose_extension_limits_v1, prepare_extension_clearance_fixture_v1,
    },
};

fn staged_input_v1<'a>(
    fixture: &'a ExtensionClearanceFixtureV1,
    common_pose: CommonArticulationPoseExtensionAuthorityV1,
    clearance: Box<CommonArticulationClearanceExtensionPrerequisiteV1>,
    configured_max_blocks: usize,
    blocks: Vec<Vec<EdgeId>>,
) -> CommonArticulationBlockComposedPathExtensionInputV1<'a> {
    staged_input_with_limits_v1(
        fixture,
        common_pose,
        clearance,
        pose_extension_limits_v1(configured_max_blocks),
        clearance_extension_limits_v1(configured_max_blocks),
        blocks,
    )
}

fn staged_input_with_limits_v1<'a>(
    fixture: &'a ExtensionClearanceFixtureV1,
    common_pose: CommonArticulationPoseExtensionAuthorityV1,
    clearance: Box<CommonArticulationClearanceExtensionPrerequisiteV1>,
    common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    clearance_limits: CommonArticulationClearanceExtensionLimitsV1,
    blocks: Vec<Vec<EdgeId>>,
) -> CommonArticulationBlockComposedPathExtensionInputV1<'a> {
    CommonArticulationBlockComposedPathExtensionInputV1 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        decomposition: &fixture.decomposition,
        common_pose,
        common_pose_limits,
        schedule: &fixture.schedule,
        schedule_limits: CycleScheduleLimitsV1::default(),
        closure: &fixture.closure,
        paper_thickness_mm: fixture.paper_thickness_mm,
        clearance: *clearance,
        clearance_limits,
        blocks,
    }
}

fn revalidation_input_v1(
    fixture: &ExtensionClearanceFixtureV1,
    configured_max_blocks: usize,
) -> CommonArticulationBlockComposedPathExtensionRevalidationInputV1<'_> {
    CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        decomposition: &fixture.decomposition,
        common_pose_limits: pose_extension_limits_v1(configured_max_blocks),
        schedule: &fixture.schedule,
        schedule_limits: CycleScheduleLimitsV1::default(),
        closure: &fixture.closure,
        paper_thickness_mm: fixture.paper_thickness_mm,
        clearance_limits: clearance_extension_limits_v1(configured_max_blocks),
    }
}

fn issue_staged_extension_v1(
    fixture: &ExtensionClearanceFixtureV1,
    configured_max_blocks: usize,
) -> CommonArticulationBlockComposedPathExtensionAuthorityV1 {
    let common_pose = fixture.pose_authority_v1(configured_max_blocks);
    let clearance = issue_extension_clearance_v1(fixture, &common_pose, configured_max_blocks);
    issue_common_articulation_block_composed_path_extension_authority_v1(staged_input_v1(
        fixture,
        common_pose,
        clearance,
        configured_max_blocks,
        fixture.canonical_edge_partition_v1(),
    ))
    .expect("staged common-articulation extension")
}

fn extension_prerequisites_v1(
    fixture: &ExtensionClearanceFixtureV1,
    configured_max_blocks: usize,
) -> (
    CommonArticulationPoseExtensionAuthorityV1,
    Box<CommonArticulationClearanceExtensionPrerequisiteV1>,
) {
    let common_pose = fixture.pose_authority_v1(configured_max_blocks);
    let clearance = issue_extension_clearance_v1(fixture, &common_pose, configured_max_blocks);
    (common_pose, clearance)
}

fn issue_staged_extension_with_blocks_v1(
    fixture: &ExtensionClearanceFixtureV1,
    configured_max_blocks: usize,
    blocks: Vec<Vec<EdgeId>>,
) -> Result<
    CommonArticulationBlockComposedPathExtensionAuthorityV1,
    CommonArticulationBlockComposedPathExtensionErrorV1,
> {
    let common_pose = fixture.pose_authority_v1(configured_max_blocks);
    let clearance = issue_extension_clearance_v1(fixture, &common_pose, configured_max_blocks);
    issue_common_articulation_block_composed_path_extension_authority_v1(staged_input_v1(
        fixture,
        common_pose,
        clearance,
        configured_max_blocks,
        blocks,
    ))
}

fn independent_canonical_blocks_v1(
    fixture: &ExtensionClearanceFixtureV1,
) -> Vec<(Vec<EdgeId>, Vec<FaceId>)> {
    let mut blocks = fixture
        .canonical_edge_partition_v1()
        .into_iter()
        .map(|mut edges| {
            edges.sort_unstable_by_key(EdgeId::canonical_bytes);
            let mut faces = edges
                .iter()
                .flat_map(|edge| {
                    let hinge = fixture
                        .geometry
                        .hinges()
                        .iter()
                        .find(|hinge| hinge.edge() == *edge)
                        .expect("fixture edge belongs to parent geometry");
                    [hinge.left_face(), hinge.right_face()]
                })
                .collect::<Vec<_>>();
            faces.sort_unstable_by_key(FaceId::canonical_bytes);
            faces.dedup();
            (edges, faces)
        })
        .collect::<Vec<_>>();
    blocks.sort_unstable_by_key(|(edges, _)| edges[0].canonical_bytes());
    blocks
}

fn direct_staged_extension_binding_v1(
    fixture: &ExtensionClearanceFixtureV1,
    authority: &CommonArticulationBlockComposedPathExtensionAuthorityV1,
    configured_max_blocks: usize,
) -> [u8; 32] {
    let blocks = independent_canonical_blocks_v1(fixture);
    let mut hash = Sha256::new();
    hash.update(b"common_articulation_block_composed_path_extension_authority_v1");
    for value in [11_u64, configured_max_blocks as u64, blocks.len() as u64] {
        hash.update(value.to_le_bytes());
    }
    hash.update(fixture.schedule.certificate_binding_fingerprint_v2());
    hash.update(fixture.closure.partition_binding_fingerprint_v2());
    hash.update(fixture.paper_thickness_mm.to_bits().to_be_bytes());
    hash.update(authority.common_pose_binding_fingerprint_v1());
    hash.update(authority.clearance_binding_fingerprint_v1());
    hash.update((blocks.len() as u64).to_be_bytes());
    for (edges, faces) in blocks {
        hash.update((edges.len() as u64).to_be_bytes());
        for edge in edges {
            hash.update(edge.canonical_bytes());
        }
        hash.update((faces.len() as u64).to_be_bytes());
        for face in faces {
            hash.update(face.canonical_bytes());
        }
    }
    hash.finalize().into()
}

#[test]
fn staged_extension_domain_binds_configured_cap_and_actual_count_v1() {
    assert_eq!(
        COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1,
        11,
    );
    assert_eq!(
        COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MAX_BLOCKS_V1,
        32,
    );
    for (actual_count, current_cap, replay_cap) in [(11, 11, 12), (12, 12, 13)] {
        let fixture = prepare_extension_clearance_fixture_v1(actual_count);
        let current = issue_staged_extension_v1(&fixture, current_cap);
        let replay = issue_staged_extension_v1(&fixture, replay_cap);

        assert_eq!(
            current.model_id(),
            COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MODEL_ID_V1,
        );
        assert_eq!(current.configured_max_blocks_v1(), current_cap);
        assert_eq!(current.actual_block_count_v1(), actual_count);
        assert_eq!(current.block_count_v1(), actual_count);
        assert_eq!(replay.configured_max_blocks_v1(), replay_cap);
        assert_eq!(
            current.binding_fingerprint_v1(),
            direct_staged_extension_binding_v1(&fixture, &current, current_cap),
        );
        assert_eq!(
            replay.binding_fingerprint_v1(),
            direct_staged_extension_binding_v1(&fixture, &replay, replay_cap),
        );
        assert_ne!(
            current.binding_fingerprint_v1(),
            replay.binding_fingerprint_v1(),
            "configured-cap replay must change the staged extension binding",
        );
        current
            .revalidate_v1(revalidation_input_v1(&fixture, current_cap))
            .expect("current staged extension revalidation");
        replay
            .revalidate_v1(revalidation_input_v1(&fixture, replay_cap))
            .expect("replay staged extension revalidation");
        assert_eq!(
            current
                .revalidate_v1(revalidation_input_v1(&fixture, replay_cap))
                .expect_err("cross-cap staged extension replay"),
            CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit,
        );
        assert!(!current.authorizes_continuous_motion());
        assert!(!current.authorizes_collision_clearance());
        assert!(!current.authorizes_project_mutation());
        assert!(!current.authorizes_apply());
        assert!(!current.authorizes_viewer());
    }
}

#[test]
fn staged_extension_rejects_each_cap_and_actual_source_mismatch_independently_v1() {
    let eleven = prepare_extension_clearance_fixture_v1(11);
    let twelve = prepare_extension_clearance_fixture_v1(12);
    let pose_limits_twelve = pose_extension_limits_v1(12);
    let clearance_limits_twelve = clearance_extension_limits_v1(12);

    let mut cap_drift = Vec::new();
    let pose_eleven = eleven.pose_authority_v1(11);
    let (_, clearance_twelve) = extension_prerequisites_v1(&eleven, 12);
    cap_drift.push((
        "pose authority cap",
        staged_input_with_limits_v1(
            &eleven,
            pose_eleven,
            clearance_twelve,
            pose_limits_twelve,
            clearance_limits_twelve,
            eleven.canonical_edge_partition_v1(),
        ),
    ));
    let (pose_twelve, clearance_twelve) = extension_prerequisites_v1(&eleven, 12);
    cap_drift.push((
        "pose limits cap",
        staged_input_with_limits_v1(
            &eleven,
            pose_twelve,
            clearance_twelve,
            pose_extension_limits_v1(11),
            clearance_limits_twelve,
            eleven.canonical_edge_partition_v1(),
        ),
    ));
    let pose_twelve = eleven.pose_authority_v1(12);
    let (_, clearance_eleven) = extension_prerequisites_v1(&eleven, 11);
    cap_drift.push((
        "clearance authority cap",
        staged_input_with_limits_v1(
            &eleven,
            pose_twelve,
            clearance_eleven,
            pose_limits_twelve,
            clearance_limits_twelve,
            eleven.canonical_edge_partition_v1(),
        ),
    ));
    let (pose_twelve, clearance_twelve) = extension_prerequisites_v1(&eleven, 12);
    cap_drift.push((
        "clearance limits cap",
        staged_input_with_limits_v1(
            &eleven,
            pose_twelve,
            clearance_twelve,
            pose_limits_twelve,
            clearance_extension_limits_v1(11),
            eleven.canonical_edge_partition_v1(),
        ),
    ));
    for (label, input) in cap_drift {
        assert_eq!(
            issue_common_articulation_block_composed_path_extension_authority_v1(input)
                .expect_err("independent staged extension cap drift"),
            CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit,
            "{label} must fail closed",
        );
    }

    let mut actual_drift = Vec::new();
    let pose_eleven = eleven.pose_authority_v1(12);
    let (_, clearance_twelve) = extension_prerequisites_v1(&twelve, 12);
    actual_drift.push((
        "pose authority actual count",
        staged_input_with_limits_v1(
            &twelve,
            pose_eleven,
            clearance_twelve,
            pose_limits_twelve,
            clearance_limits_twelve,
            twelve.canonical_edge_partition_v1(),
        ),
    ));
    let pose_twelve = twelve.pose_authority_v1(12);
    let (_, clearance_eleven) = extension_prerequisites_v1(&eleven, 12);
    actual_drift.push((
        "clearance authority actual count",
        staged_input_with_limits_v1(
            &twelve,
            pose_twelve,
            clearance_eleven,
            pose_limits_twelve,
            clearance_limits_twelve,
            twelve.canonical_edge_partition_v1(),
        ),
    ));
    let (pose_twelve, clearance_twelve) = extension_prerequisites_v1(&twelve, 12);
    actual_drift.push((
        "decomposition actual count",
        staged_input_with_limits_v1(
            &eleven,
            pose_twelve,
            clearance_twelve,
            pose_limits_twelve,
            clearance_limits_twelve,
            eleven.canonical_edge_partition_v1(),
        ),
    ));
    for (label, input) in actual_drift {
        assert_eq!(
            issue_common_articulation_block_composed_path_extension_authority_v1(input)
                .expect_err("independent staged extension actual drift"),
            CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit,
            "{label} must fail closed",
        );
    }
}

#[test]
fn staged_extension_hard_thirty_two_is_inclusive_and_other_arities_fail_v1() {
    let thirty_two = prepare_extension_clearance_fixture_v1(32);
    let authority = issue_staged_extension_v1(&thirty_two, 32);
    assert_eq!(authority.actual_block_count_v1(), 32);
    authority
        .revalidate_v1(revalidation_input_v1(&thirty_two, 32))
        .expect("inclusive thirty-two staged extension revalidation");

    let eleven = prepare_extension_clearance_fixture_v1(11);
    let common_pose = eleven.pose_authority_v1(11);
    let clearance = issue_extension_clearance_v1(&eleven, &common_pose, 11);
    let ten = prepare_extension_clearance_fixture_v1(10);
    assert_eq!(
        issue_common_articulation_block_composed_path_extension_authority_v1(staged_input_v1(
            &ten,
            common_pose,
            clearance,
            11,
            ten.canonical_edge_partition_v1(),
        ))
        .expect_err("ten blocks are below the staged extension minimum"),
        CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit,
    );

    for invalid_cap in [10, 33, usize::MAX] {
        let common_pose = eleven.pose_authority_v1(11);
        let clearance = issue_extension_clearance_v1(&eleven, &common_pose, 11);
        let mut input = staged_input_v1(
            &eleven,
            common_pose,
            clearance,
            11,
            eleven.canonical_edge_partition_v1(),
        );
        input.common_pose_limits = CommonArticulationPoseExtensionLimitsV1 {
            max_blocks: invalid_cap,
            ..input.common_pose_limits
        };
        assert_eq!(
            issue_common_articulation_block_composed_path_extension_authority_v1(input)
                .expect_err("invalid staged extension cap"),
            CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit,
        );
    }
}

#[test]
fn staged_extension_partition_is_order_invariant_and_exact_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let canonical = fixture.canonical_edge_partition_v1();
    let baseline = issue_staged_extension_with_blocks_v1(&fixture, 11, canonical.clone())
        .expect("canonical staged extension partition");
    let mut reversed = canonical.clone();
    reversed.reverse();
    for edges in &mut reversed {
        edges.reverse();
    }
    let reordered = issue_staged_extension_with_blocks_v1(&fixture, 11, reversed)
        .expect("reordered staged extension partition");
    assert_eq!(
        baseline.binding_fingerprint_v1(),
        reordered.binding_fingerprint_v1(),
    );

    assert_eq!(
        issue_staged_extension_with_blocks_v1(
            &fixture,
            11,
            canonical[..canonical.len() - 1].to_vec(),
        )
        .expect_err("missing staged extension block"),
        CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch,
    );
    let mut duplicate = canonical.clone();
    duplicate[1] = duplicate[0].clone();
    assert_eq!(
        issue_staged_extension_with_blocks_v1(&fixture, 11, duplicate)
            .expect_err("duplicate staged extension block"),
        CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch,
    );
    let mut foreign = canonical;
    foreign[0][0] = EdgeId::new();
    assert_eq!(
        issue_staged_extension_with_blocks_v1(&fixture, 11, foreign)
            .expect_err("foreign staged extension edge"),
        CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch,
    );
}

#[test]
fn staged_extension_revalidation_rejects_all_live_binding_drift_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let foreign = prepare_extension_clearance_fixture_v1(11);
    let authority = issue_staged_extension_v1(&fixture, 11);
    let baseline = revalidation_input_v1(&fixture, 11);
    authority
        .revalidate_v1(baseline)
        .expect("baseline staged extension revalidation");

    let drifted = [
        (
            "geometry",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                geometry: &foreign.geometry,
                ..baseline
            },
        ),
        (
            "pose",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                pose: &foreign.pose,
                ..baseline
            },
        ),
        (
            "decomposition",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                decomposition: &foreign.decomposition,
                ..baseline
            },
        ),
        (
            "schedule",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                schedule: &foreign.schedule,
                ..baseline
            },
        ),
        (
            "closure",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                closure: &foreign.closure,
                ..baseline
            },
        ),
        (
            "thickness",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                paper_thickness_mm: f64::from_bits(fixture.paper_thickness_mm.to_bits() + 1),
                ..baseline
            },
        ),
        (
            "cap",
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                common_pose_limits: pose_extension_limits_v1(12),
                clearance_limits: clearance_extension_limits_v1(12),
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
}

#[test]
fn staged_extension_rejects_foreign_and_cross_cap_prerequisites_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let foreign = prepare_extension_clearance_fixture_v1(11);

    let foreign_pose = foreign.pose_authority_v1(11);
    let foreign_clearance = issue_extension_clearance_v1(&foreign, &foreign_pose, 11);
    assert!(matches!(
        issue_common_articulation_block_composed_path_extension_authority_v1(staged_input_v1(
            &fixture,
            foreign_pose,
            foreign_clearance,
            11,
            fixture.canonical_edge_partition_v1(),
        )),
        Err(CommonArticulationBlockComposedPathExtensionErrorV1::CommonPose(_))
    ));

    let pose = fixture.pose_authority_v1(11);
    let foreign_pose = foreign.pose_authority_v1(11);
    let foreign_clearance = issue_extension_clearance_v1(&foreign, &foreign_pose, 11);
    assert!(matches!(
        issue_common_articulation_block_composed_path_extension_authority_v1(staged_input_v1(
            &fixture,
            pose,
            foreign_clearance,
            11,
            fixture.canonical_edge_partition_v1(),
        )),
        Err(CommonArticulationBlockComposedPathExtensionErrorV1::Clearance(_))
    ));

    let pose_eleven = fixture.pose_authority_v1(11);
    let pose_twelve = fixture.pose_authority_v1(12);
    let clearance_twelve = issue_extension_clearance_v1(&fixture, &pose_twelve, 12);
    let mut cross_cap = staged_input_v1(
        &fixture,
        pose_eleven,
        clearance_twelve,
        11,
        fixture.canonical_edge_partition_v1(),
    );
    cross_cap.clearance_limits = clearance_extension_limits_v1(12);
    assert_eq!(
        issue_common_articulation_block_composed_path_extension_authority_v1(cross_cap)
            .expect_err("cross-cap staged prerequisites"),
        CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit,
    );
}

#[test]
fn staged_extension_checkpoint_boundaries_and_public_control_map_stops_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let unbounded = CooperativeOperationControlV1::unbounded();
    let (pose, clearance) = extension_prerequisites_v1(&fixture, 11);
    let mut issuance_checkpoint_count = 0usize;
    issue_common_articulation_block_composed_path_extension_authority_with_checkpoint_v1(
        staged_input_v1(
            &fixture,
            pose,
            clearance,
            11,
            fixture.canonical_edge_partition_v1(),
        ),
        &unbounded,
        &mut || {
            issuance_checkpoint_count += 1;
            Ok(())
        },
    )
    .expect("count staged extension issuance checkpoints");
    assert!(issuance_checkpoint_count >= 4);
    for stop_at in [1, issuance_checkpoint_count / 2, issuance_checkpoint_count] {
        for expected in [
            CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled,
            CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded,
        ] {
            let (pose, clearance) = extension_prerequisites_v1(&fixture, 11);
            let mut observed = 0usize;
            assert_eq!(
                issue_common_articulation_block_composed_path_extension_authority_with_checkpoint_v1(
                    staged_input_v1(
                        &fixture,
                        pose,
                        clearance,
                        11,
                        fixture.canonical_edge_partition_v1(),
                    ),
                    &unbounded,
                    &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("deterministic staged extension issuance stop"),
                expected,
            );
        }
    }

    let authority = issue_staged_extension_v1(&fixture, 11);
    let revalidation_input = revalidation_input_v1(&fixture, 11);
    let mut revalidation_checkpoint_count = 0usize;
    authority
        .revalidate_with_checkpoint_v1(revalidation_input, &unbounded, &mut || {
            revalidation_checkpoint_count += 1;
            Ok(())
        })
        .expect("count staged extension revalidation checkpoints");
    assert!(revalidation_checkpoint_count >= 4);
    for stop_at in [
        1,
        revalidation_checkpoint_count / 2,
        revalidation_checkpoint_count,
    ] {
        for expected in [
            CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled,
            CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                authority
                    .revalidate_with_checkpoint_v1(revalidation_input, &unbounded, &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    },)
                    .expect_err("deterministic staged extension revalidation stop"),
                expected,
            );
        }
    }

    let cancelled = AtomicBool::new(true);
    let active = AtomicBool::new(false);

    let pose = fixture.pose_authority_v1(11);
    let clearance = issue_extension_clearance_v1(&fixture, &pose, 11);
    assert_eq!(
        issue_common_articulation_block_composed_path_extension_authority_with_control_v1(
            staged_input_v1(
                &fixture,
                pose,
                clearance,
                11,
                fixture.canonical_edge_partition_v1(),
            ),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .expect_err("staged extension cancellation"),
        CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled,
    );

    let pose = fixture.pose_authority_v1(11);
    let clearance = issue_extension_clearance_v1(&fixture, &pose, 11);
    assert_eq!(
        issue_common_articulation_block_composed_path_extension_authority_with_control_v1(
            staged_input_v1(
                &fixture,
                pose,
                clearance,
                11,
                fixture.canonical_edge_partition_v1(),
            ),
            &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
        )
        .expect_err("staged extension deadline"),
        CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded,
    );

    assert_eq!(
        authority
            .revalidate_with_control_v1(
                revalidation_input_v1(&fixture, 11),
                &CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(1),
                ),
            )
            .expect_err("staged extension revalidation cancellation"),
        CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled,
    );
    assert_eq!(
        authority
            .revalidate_with_control_v1(
                revalidation_input_v1(&fixture, 11),
                &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
            )
            .expect_err("staged extension revalidation deadline"),
        CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded,
    );
}
