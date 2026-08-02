//! Focused public-boundary tests for dynamic clearance.

use std::collections::HashSet;

use crate::common_articulation_clearance_v2::test_support::{
    MiuraFixtureV2, miura_fixture_v2, miura_fixture_v2_with_profile,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CommonArticulationDynamicClosureBridgeInputV2,
    CommonArticulationDynamicClosureBridgeLimitsV2, CommonArticulationDynamicClosureBridgeV2,
    CommonArticulationPoseAuthorityV2, CommonArticulationPoseInputV2, CycleScheduleEntryInputV1,
    CycleScheduleLimitsV1, RationalCoefficientV1,
    prove_common_articulation_dynamic_closure_bridge_v2,
    prove_common_articulation_pose_authority_v2,
};

use super::*;

const N33: usize = 33;
const LIMIT: usize = 1 << 30;

struct DynamicFixtureV2 {
    fixture: MiuraFixtureV2,
    schedule: CanonicalCycleScheduleV1,
    pose: ori_kinematics::ClosedMaterialHingeGraphPose,
    common_pose: CommonArticulationPoseAuthorityV2,
    bridge: CommonArticulationDynamicClosureBridgeV2,
}

fn bridge_limits_v2(max_blocks: usize) -> CommonArticulationDynamicClosureBridgeLimitsV2 {
    CommonArticulationDynamicClosureBridgeLimitsV2 {
        max_blocks,
        max_validation_work: LIMIT,
        max_total_restriction_work: LIMIT,
        max_total_restricted_schedule_retained_bytes: LIMIT,
        max_total_block_closure_retained_bytes: LIMIT,
        max_total_block_leaves: LIMIT,
        max_parent_schedule_retained_bytes: LIMIT,
        max_parent_closure_retained_bytes: LIMIT,
        max_parent_leaves: LIMIT,
        max_bundle_retained_bytes: LIMIT,
        max_issuance_peak_bytes: LIMIT,
        max_revalidation_peak_bytes: LIMIT * 3,
        max_schedule_degree: 1,
        max_schedule_coefficient_bits: 53,
        max_dyadic_depth: 2,
        max_dyadic_leaves_per_closure: 4,
        max_dyadic_work_per_closure: LIMIT,
    }
}

fn clearance_limits_v2() -> CommonArticulationDynamicClosureClearanceLimitsV2 {
    CommonArticulationDynamicClosureClearanceLimitsV2 {
        max_blocks: N33,
        max_faces: LIMIT,
        max_cross_block_pairs: LIMIT,
        max_pair_registry_retained_bytes: LIMIT,
        max_pair_registry_temporary_bytes: LIMIT,
        max_publication_bytes: LIMIT,
        max_aggregate_peak_bytes: LIMIT * 4,
    }
}

fn dynamic_fixture_v2() -> DynamicFixtureV2 {
    dynamic_fixture_with_blocks_v2(N33)
}

fn dynamic_fixture_with_blocks_v2(actual_blocks: usize) -> DynamicFixtureV2 {
    let fixture = if actual_blocks == N33 {
        miura_fixture_v2()
    } else {
        miura_fixture_v2_with_profile(actual_blocks, actual_blocks)
    };
    let schedule = nonstationary_schedule_v2(&fixture);
    let pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.geometry.face_ids()[0],
            &schedule.evaluate(0.0).expect("midpoint schedule"),
            1.0e-8,
        )
        .expect("N33 midpoint pose");
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &pose,
        decomposition: &fixture.decomposition,
        paper_thickness_mm: 0.1,
        profile: &fixture.profile,
    })
    .expect("N33 common pose");
    let bridge = prove_common_articulation_dynamic_closure_bridge_v2(
        CommonArticulationDynamicClosureBridgeInputV2 {
            geometry: &fixture.geometry,
            audit: &fixture.audit,
            pose: &pose,
            parent_fixed_face: fixture.geometry.face_ids()[0],
            parent_schedule: &schedule,
            decomposition: &fixture.decomposition,
            common_pose: &common_pose,
            paper_thickness_mm: 0.1,
            closure_tolerance: 0.0,
            profile: &fixture.profile,
            limits: bridge_limits_v2(actual_blocks),
        },
    )
    .expect("N33 dynamic bridge");
    DynamicFixtureV2 {
        fixture,
        schedule,
        pose,
        common_pose,
        bridge,
    }
}

fn nonstationary_schedule_v2(fixture: &MiuraFixtureV2) -> CanonicalCycleScheduleV1 {
    let first_block = &fixture.decomposition.blocks()[0];
    let moving = (0..3)
        .flat_map(|axis_index| {
            first_block
                .geometry()
                .hinges()
                .iter()
                .filter(move |hinge| {
                    [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()][axis_index].abs() == 1.0
                        && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                })
                .map(move |reference| {
                    let reference_start = [
                        reference.start().x(),
                        reference.start().y(),
                        reference.start().z(),
                    ];
                    first_block
                        .geometry()
                        .hinges()
                        .iter()
                        .filter(|hinge| {
                            let axis = [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()];
                            let start = [hinge.start().x(), hinge.start().y(), hinge.start().z()];
                            axis[axis_index].abs() == 1.0
                                && axis
                                    .iter()
                                    .enumerate()
                                    .all(|(i, value)| i == axis_index || *value == 0.0)
                                && start.iter().enumerate().all(|(i, value)| {
                                    i == axis_index
                                        || value.to_bits() == reference_start[i].to_bits()
                                })
                                && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                        })
                        .map(|hinge| hinge.edge())
                        .collect::<HashSet<_>>()
                })
        })
        .find(|family| family.len() == 3)
        .expect("all-mountain parallel carrier family");
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let slope = RationalCoefficientV1 {
        numerator: 1,
        denominator: 2,
    };
    let mut entries = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let moves = moving.contains(&hinge.edge());
            CycleScheduleEntryInputV1 {
                edge: hinge.edge(),
                initial_angle_degrees_bits: if moves {
                    1.0_f64.to_bits()
                } else {
                    0.0_f64.to_bits()
                },
                chebyshev_coefficients: if moves { vec![zero, slope] } else { vec![zero] },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.geometry.face_ids()[0],
        [-1.0, 1.0],
        entries,
        CycleScheduleLimitsV1 {
            max_hinges: fixture.geometry.hinges().len(),
            max_degree: 1,
            max_coefficient_bits: 53,
            max_work: LIMIT,
        },
    )
    .expect("N33 nonstationary schedule");
    let moving_edge = *moving.iter().next().expect("moving edge");
    let observed = |parameter| {
        schedule
            .evaluate(parameter)
            .expect("schedule evaluation")
            .as_slice()
            .iter()
            .find(|value| value.edge() == moving_edge)
            .expect("moving angle")
            .angle_degrees()
    };
    assert_eq!(observed(-1.0).to_bits(), 0.5_f64.to_bits());
    assert_eq!(observed(1.0).to_bits(), 1.5_f64.to_bits());
    schedule
}

fn input_v2<'a>(
    dynamic: &'a DynamicFixtureV2,
    pairs: &'a [CommonArticulationCrossBlockFacePairV2],
    limits: CommonArticulationDynamicClosureClearanceLimitsV2,
) -> CommonArticulationDynamicClosureClearanceInputV2<'a> {
    CommonArticulationDynamicClosureClearanceInputV2 {
        geometry: &dynamic.fixture.geometry,
        audit: &dynamic.fixture.audit,
        pose: &dynamic.pose,
        decomposition: &dynamic.fixture.decomposition,
        common_pose: &dynamic.common_pose,
        parent_fixed_face: dynamic.fixture.geometry.face_ids()[0],
        parent_schedule: &dynamic.schedule,
        profile: &dynamic.fixture.profile,
        paper_thickness_mm: 0.1,
        closure_tolerance: 0.0,
        dynamic_closure_bridge: &dynamic.bridge,
        submitted_cross_block_pairs: pairs,
        limits,
    }
}

fn replay_input_v2<'a>(
    dynamic: &'a DynamicFixtureV2,
) -> CommonArticulationDynamicClosureClearanceRevalidationInputV2<'a> {
    CommonArticulationDynamicClosureClearanceRevalidationInputV2 {
        geometry: &dynamic.fixture.geometry,
        audit: &dynamic.fixture.audit,
        pose: &dynamic.pose,
        decomposition: &dynamic.fixture.decomposition,
        common_pose: &dynamic.common_pose,
        parent_fixed_face: dynamic.fixture.geometry.face_ids()[0],
        parent_schedule: &dynamic.schedule,
        profile: &dynamic.fixture.profile,
        paper_thickness_mm: 0.1,
        closure_tolerance: 0.0,
        dynamic_closure_bridge: &dynamic.bridge,
    }
}

#[test]
fn n33_dynamic_clearance_replays_and_rejects_foreign_pairs_resources_and_stops() {
    let dynamic = dynamic_fixture_v2();
    let outcome = issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
        &dynamic,
        &dynamic.fixture.pairs,
        clearance_limits_v2(),
    ))
    .expect("N33 dynamic clearance prerequisite");
    assert_eq!(
        outcome.model_id_v2(),
        COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_UNPROMOTED_MODEL_ID_V2
    );
    assert!(!outcome.is_certified_v2());
    let prerequisite = outcome.as_unpromoted_v2();
    assert_eq!(prerequisite.actual_block_count_v2(), N33);
    assert_eq!(prerequisite.actual_face_count_v2(), 8 * N33 + 1);
    assert_ne!(prerequisite.binding_fingerprint_v2(), [0; 32]);
    prerequisite
        .revalidate_v2(replay_input_v2(&dynamic))
        .expect("same tuple replay");

    let exact = CommonArticulationDynamicClosureClearanceLimitsV2 {
        max_blocks: N33,
        max_faces: dynamic.fixture.geometry.face_ids().len(),
        max_cross_block_pairs: dynamic.fixture.pairs.len(),
        max_pair_registry_retained_bytes: prerequisite
            .pair_registry_retained_bytes_upper_bound_v2(),
        max_pair_registry_temporary_bytes: prerequisite
            .pair_registry_temporary_bytes_upper_bound_v2(),
        max_publication_bytes: prerequisite.publication_bytes_upper_bound_v2(),
        max_aggregate_peak_bytes: prerequisite.aggregate_peak_bytes_upper_bound_v2(),
    };
    issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
        &dynamic,
        &dynamic.fixture.pairs,
        exact,
    ))
    .expect("exact resource limits");
    let one_short_limits = [
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_blocks: N33 - 1,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_faces: dynamic.fixture.geometry.face_ids().len() - 1,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_cross_block_pairs: dynamic.fixture.pairs.len() - 1,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_pair_registry_retained_bytes: exact.max_pair_registry_retained_bytes - 1,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_pair_registry_temporary_bytes: exact.max_pair_registry_temporary_bytes - 1,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_publication_bytes: exact.max_publication_bytes - 1,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_aggregate_peak_bytes: exact.max_aggregate_peak_bytes - 1,
            ..exact
        },
    ];
    for limits in one_short_limits {
        assert!(matches!(
            issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
                &dynamic,
                &dynamic.fixture.pairs,
                limits,
            )),
            Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)
        ));
    }
    for limits in [
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_blocks: usize::MAX,
            ..exact
        },
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_pair_registry_retained_bytes: usize::MAX - 1,
            ..clearance_limits_v2()
        },
    ] {
        assert!(matches!(
            issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
                &dynamic,
                &dynamic.fixture.pairs,
                limits,
            )),
            Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)
        ));
    }

    let mut unordered = dynamic.fixture.pairs.clone();
    unordered.swap(0, 1);
    let mut duplicate = dynamic.fixture.pairs.clone();
    duplicate[1] = duplicate[0];
    let mut incomplete = dynamic.fixture.pairs.clone();
    incomplete.pop();
    for (pairs, expected) in [
        (
            unordered.as_slice(),
            CommonArticulationDynamicClosureClearanceErrorV2::NonCanonicalCrossBlockPairRegistry,
        ),
        (
            duplicate.as_slice(),
            CommonArticulationDynamicClosureClearanceErrorV2::DuplicateCrossBlockPair,
        ),
        (
            incomplete.as_slice(),
            CommonArticulationDynamicClosureClearanceErrorV2::CrossBlockPairCoverageMismatch {
                expected: dynamic.fixture.pairs.len(),
                actual: dynamic.fixture.pairs.len() - 1,
            },
        ),
    ] {
        assert!(matches!(
            issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
                &dynamic,
                pairs,
                clearance_limits_v2(),
            )),
            Err(actual) if actual == expected
        ));
    }

    let first_block_faces = dynamic.fixture.decomposition.blocks()[0]
        .geometry()
        .face_ids();
    let local_pair =
        CommonArticulationCrossBlockFacePairV2::new(first_block_faces[0], first_block_faces[1])
            .expect("distinct within-block faces");
    assert!(!dynamic.fixture.pairs.contains(&local_pair));
    let mut substituted = dynamic.fixture.pairs.clone();
    let _removed_pair = substituted.pop().expect("non-empty cross-block registry");
    substituted.push(local_pair);
    substituted.sort_unstable_by(|left, right| {
        left.first_v2()
            .canonical_bytes()
            .cmp(&right.first_v2().canonical_bytes())
            .then_with(|| {
                left.second_v2()
                    .canonical_bytes()
                    .cmp(&right.second_v2().canonical_bytes())
            })
    });
    assert!(substituted.windows(2).all(|window| window[0] != window[1]));
    assert!(matches!(
        issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
            &dynamic,
            &substituted,
            clearance_limits_v2(),
        )),
        Err(
            CommonArticulationDynamicClosureClearanceErrorV2::CrossBlockPairCoverageMismatch {
                expected,
                actual,
            }
        ) if expected == dynamic.fixture.pairs.len() && actual == expected
    ));

    assert!(matches!(
        issue_common_articulation_dynamic_closure_clearance_prerequisite_with_checkpoint_v2(
            input_v2(
                &dynamic,
                &dynamic.fixture.pairs,
                CommonArticulationDynamicClosureClearanceLimitsV2 {
                    max_blocks: 0,
                    ..clearance_limits_v2()
                },
            ),
            || Err(CommonArticulationDynamicClosureClearanceStopV2::DeadlineExceeded),
        ),
        Err(CommonArticulationDynamicClosureClearanceErrorV2::DeadlineExceeded)
    ));
    assert_eq!(
        prerequisite.revalidate_with_checkpoint_v2(replay_input_v2(&dynamic), || {
            Err(CommonArticulationDynamicClosureClearanceStopV2::Cancelled)
        }),
        Err(CommonArticulationDynamicClosureClearanceErrorV2::Cancelled)
    );

    let foreign = dynamic_fixture_v2();
    assert!(matches!(
        issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(
            CommonArticulationDynamicClosureClearanceInputV2 {
                dynamic_closure_bridge: &foreign.bridge,
                ..input_v2(&dynamic, &dynamic.fixture.pairs, clearance_limits_v2())
            }
        ),
        Err(
            CommonArticulationDynamicClosureClearanceErrorV2::DynamicClosureBridge(
                ori_kinematics::CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch
            )
        )
    ));
    assert!(matches!(
        issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(
            CommonArticulationDynamicClosureClearanceInputV2 {
                parent_fixed_face: dynamic.fixture.geometry.face_ids()[1],
                ..input_v2(&dynamic, &dynamic.fixture.pairs, clearance_limits_v2())
            }
        ),
        Err(CommonArticulationDynamicClosureClearanceErrorV2::DynamicClosureBridge(_))
    ));
}

#[test]
fn n34_dynamic_clearance_uses_global_parent_face_count() {
    let dynamic = dynamic_fixture_with_blocks_v2(34);
    let exact_faces = dynamic.fixture.geometry.face_ids().len();
    let outcome = issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
        &dynamic,
        &dynamic.fixture.pairs,
        CommonArticulationDynamicClosureClearanceLimitsV2 {
            max_blocks: 34,
            max_faces: exact_faces,
            ..clearance_limits_v2()
        },
    ))
    .expect("N34 dynamic clearance prerequisite");
    assert_eq!(outcome.as_unpromoted_v2().actual_block_count_v2(), 34);
    assert_eq!(
        outcome.as_unpromoted_v2().actual_face_count_v2(),
        8 * 34 + 1
    );
    assert!(matches!(
        issue_common_articulation_dynamic_closure_clearance_prerequisite_v2(input_v2(
            &dynamic,
            &dynamic.fixture.pairs,
            CommonArticulationDynamicClosureClearanceLimitsV2 {
                max_blocks: 34,
                max_faces: exact_faces - 1,
                ..clearance_limits_v2()
            },
        )),
        Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)
    ));
}
