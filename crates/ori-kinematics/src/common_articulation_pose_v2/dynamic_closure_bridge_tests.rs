//! Public-boundary contract tests for the opaque dynamic closure bridge.

use std::collections::HashSet;

use super::*;
use crate::{
    CommonArticulationDynamicClosureBridgeErrorV2, CommonArticulationDynamicClosureBridgeInputV2,
    CommonArticulationDynamicClosureBridgeLimitsV2,
    CommonArticulationDynamicClosureBridgeRevalidationInputV2,
    CommonArticulationDynamicClosureBridgeStopV2, CommonArticulationDynamicClosureBridgeV2,
    prove_common_articulation_dynamic_closure_bridge_v2,
    prove_common_articulation_dynamic_closure_bridge_with_checkpoint_v2,
};

const N33_BLOCKS: usize = 33;
const LIMIT: usize = 1 << 30;

#[path = "dynamic_closure_bridge_tests/foreign_and_policy.rs"]
mod foreign_and_policy;

fn bridge_limits_v2() -> CommonArticulationDynamicClosureBridgeLimitsV2 {
    CommonArticulationDynamicClosureBridgeLimitsV2 {
        max_blocks: N33_BLOCKS,
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

fn nonstationary_parent_schedule_v2(
    fixture: &MiuraFixtureV2,
) -> (CanonicalCycleScheduleV1, HashSet<EdgeId>) {
    let first_block = &fixture.decomposition.blocks()[0];
    let moving = (0..3)
        .flat_map(|dominant_axis| {
            first_block
                .geometry()
                .hinges()
                .iter()
                .filter(move |hinge| {
                    [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()][dominant_axis].abs()
                        == 1.0
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
                            axis[dominant_axis].abs() == 1.0
                                && axis.iter().enumerate().all(|(dimension, value)| {
                                    dimension == dominant_axis || *value == 0.0
                                })
                                && start.iter().enumerate().all(|(dimension, value)| {
                                    dimension == dominant_axis
                                        || value.to_bits() == reference_start[dimension].to_bits()
                                })
                                && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                        })
                        .map(|hinge| hinge.edge())
                        .collect::<HashSet<_>>()
                })
        })
        .find(|family| family.len() == 3)
        .expect("one all-mountain parallel carrier family");
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
    .expect("N33 ordinary nonstationary parent schedule");
    let moving_edge = *moving.iter().next().expect("moving edge");
    let angle = |parameter| {
        schedule
            .evaluate(parameter)
            .expect("in-domain evaluation")
            .as_slice()
            .iter()
            .find(|value| value.edge() == moving_edge)
            .expect("moving angle")
            .angle_degrees()
    };
    assert_eq!(angle(-1.0).to_bits(), 0.5_f64.to_bits());
    assert_eq!(angle(1.0).to_bits(), 1.5_f64.to_bits());
    (schedule, moving)
}

fn bridge_input_v2<'a>(
    fixture: &'a MiuraFixtureV2,
    profile: &'a CommonArticulationResourceProfileV2,
    pose: &'a ClosedMaterialHingeGraphPose,
    common_pose: &'a CommonArticulationPoseAuthorityV2,
    parent_schedule: &'a CanonicalCycleScheduleV1,
    limits: CommonArticulationDynamicClosureBridgeLimitsV2,
) -> CommonArticulationDynamicClosureBridgeInputV2<'a> {
    CommonArticulationDynamicClosureBridgeInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule,
        decomposition: &fixture.decomposition,
        common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 0.0,
        profile,
        limits,
    }
}

fn bridge_revalidation_input_v2<'a>(
    fixture: &'a MiuraFixtureV2,
    profile: &'a CommonArticulationResourceProfileV2,
    pose: &'a ClosedMaterialHingeGraphPose,
    common_pose: &'a CommonArticulationPoseAuthorityV2,
    parent_schedule: &'a CanonicalCycleScheduleV1,
) -> CommonArticulationDynamicClosureBridgeRevalidationInputV2<'a> {
    CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule,
        decomposition: &fixture.decomposition,
        common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 0.0,
        profile,
    }
}

#[test]
fn n33_nonstationary_bridge_is_opaque_replayable_and_resource_exact() {
    let fixture = miura_fixture_v2(N33_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let (parent_schedule, moving) = nonstationary_parent_schedule_v2(&fixture);
    let pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.geometry.face_ids()[0],
            &parent_schedule.evaluate(0.0).unwrap(),
            1.0e-8,
        )
        .unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &pose,
        decomposition: &fixture.decomposition,
        paper_thickness_mm: 0.1,
        profile: &profile,
    })
    .unwrap();
    assert!(moving.iter().all(|edge| {
        parent_schedule
            .derivative_bound(*edge)
            .is_some_and(|bound| bound > 0.0)
    }));

    let bridge = prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
        &fixture,
        &profile,
        &pose,
        &common_pose,
        &parent_schedule,
        bridge_limits_v2(),
    ))
    .expect("N33 ordinary nonstationary bridge");
    assert_eq!(bridge.actual_block_count_v2(), N33_BLOCKS);
    let bridge_debug = format!("{bridge:?}");
    assert!(!bridge_debug.contains("parent_closure"));
    assert!(!bridge_debug.contains("parent_schedule"));
    assert!(!bridge_debug.contains("partition"));
    assert!(!bridge_debug.contains("binding_fingerprint"));
    assert_ne!(bridge.binding_fingerprint_v2(), [0; 32]);
    assert!(bridge.retained_bytes_upper_bound_v2() > 0);
    assert!(bridge.issuance_peak_bytes_upper_bound_v2() > 0);
    assert_eq!(
        bridge.revalidation_peak_bytes_upper_bound_v2(),
        bridge
            .retained_bytes_upper_bound_v2()
            .checked_add(bridge.issuance_peak_bytes_upper_bound_v2())
            .unwrap()
    );
    bridge
        .revalidate_v2(bridge_revalidation_input_v2(
            &fixture,
            &profile,
            &pose,
            &common_pose,
            &parent_schedule,
        ))
        .expect("same dynamic bridge replay");

    let mut exact = bridge_limits_v2();
    exact.max_bundle_retained_bytes = bridge.retained_bytes_upper_bound_v2();
    exact.max_issuance_peak_bytes = bridge.issuance_peak_bytes_upper_bound_v2();
    exact.max_revalidation_peak_bytes = bridge.revalidation_peak_bytes_upper_bound_v2();
    let exact_bridge = prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
        &fixture,
        &profile,
        &pose,
        &common_pose,
        &parent_schedule,
        exact,
    ))
    .expect("exact observed public resource limits admit bridge");
    assert_eq!(
        exact_bridge.retained_bytes_upper_bound_v2(),
        bridge.retained_bytes_upper_bound_v2()
    );
    assert_eq!(
        exact_bridge.issuance_peak_bytes_upper_bound_v2(),
        bridge.issuance_peak_bytes_upper_bound_v2()
    );

    let mut one_short = exact;
    one_short.max_bundle_retained_bytes -= 1;
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &fixture,
            &profile,
            &pose,
            &common_pose,
            &parent_schedule,
            one_short,
        ))
        .expect_err("one-short retained cap"),
        CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit,
    );
    let mut one_short = exact;
    one_short.max_issuance_peak_bytes -= 1;
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &fixture,
            &profile,
            &pose,
            &common_pose,
            &parent_schedule,
            one_short,
        ))
        .expect_err("one-short issuance peak cap"),
        CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit,
    );
    let mut one_short = exact;
    one_short.max_revalidation_peak_bytes -= 1;
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &fixture,
            &profile,
            &pose,
            &common_pose,
            &parent_schedule,
            one_short,
        ))
        .expect_err("one-short revalidation peak cap"),
        CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit,
    );
}

#[test]
fn bridge_rejects_foreign_inputs_invalid_limits_and_stops() {
    let fixture = miura_fixture_v2(N33_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let (parent_schedule, _) = nonstationary_parent_schedule_v2(&fixture);
    let pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.geometry.face_ids()[0],
            &parent_schedule.evaluate(0.0).unwrap(),
            1.0e-8,
        )
        .unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &pose,
        decomposition: &fixture.decomposition,
        paper_thickness_mm: 0.1,
        profile: &profile,
    })
    .unwrap();
    let input = bridge_input_v2(
        &fixture,
        &profile,
        &pose,
        &common_pose,
        &parent_schedule,
        bridge_limits_v2(),
    );
    let bridge = prove_common_articulation_dynamic_closure_bridge_v2(input).unwrap();
    let foreign_pose = fixture.new_pose_instance();
    assert_eq!(
        bridge
            .revalidate_v2(bridge_revalidation_input_v2(
                &fixture,
                &profile,
                &foreign_pose,
                &common_pose,
                &parent_schedule,
            ))
            .expect_err("value-equal foreign pose is not the issuer pose"),
        CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch,
    );

    let mut unbounded = bridge_limits_v2();
    unbounded.max_dyadic_work_per_closure = usize::MAX;
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &fixture,
            &profile,
            &pose,
            &common_pose,
            &parent_schedule,
            unbounded,
        ))
        .expect_err("unbounded public workspace cap"),
        CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit,
    );
    for (label, limits) in [
        (
            "zero schedule degree",
            CommonArticulationDynamicClosureBridgeLimitsV2 {
                max_schedule_degree: 0,
                ..bridge_limits_v2()
            },
        ),
        (
            "unbounded schedule degree",
            CommonArticulationDynamicClosureBridgeLimitsV2 {
                max_schedule_degree: usize::MAX,
                ..bridge_limits_v2()
            },
        ),
        (
            "unsupported dyadic depth",
            CommonArticulationDynamicClosureBridgeLimitsV2 {
                max_dyadic_depth: 64,
                ..bridge_limits_v2()
            },
        ),
    ] {
        assert_eq!(
            prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
                &fixture,
                &profile,
                &pose,
                &common_pose,
                &parent_schedule,
                limits,
            ))
            .expect_err(label),
            CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit,
        );
    }
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_with_checkpoint_v2(input, || {
            Err(CommonArticulationDynamicClosureBridgeStopV2::Cancelled)
        })
        .expect_err("bridge issuance cancellation"),
        CommonArticulationDynamicClosureBridgeErrorV2::Cancelled,
    );
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_with_checkpoint_v2(input, || {
            Err(CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded)
        })
        .expect_err("bridge issuance deadline"),
        CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded,
    );
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_with_checkpoint_v2(
            bridge_input_v2(
                &fixture,
                &profile,
                &pose,
                &common_pose,
                &parent_schedule,
                CommonArticulationDynamicClosureBridgeLimitsV2 {
                    max_schedule_degree: 0,
                    ..bridge_limits_v2()
                },
            ),
            || Err(CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded),
        )
        .expect_err("checkpoint precedes invalid policy preflight"),
        CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded,
    );
    assert_eq!(
        bridge
            .revalidate_with_checkpoint_v2(
                bridge_revalidation_input_v2(
                    &fixture,
                    &profile,
                    &pose,
                    &common_pose,
                    &parent_schedule,
                ),
                || Err(CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded),
            )
            .expect_err("bridge revalidation deadline"),
        CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded,
    );
    assert_eq!(
        bridge
            .revalidate_with_checkpoint_v2(
                bridge_revalidation_input_v2(
                    &fixture,
                    &profile,
                    &foreign_pose,
                    &common_pose,
                    &parent_schedule,
                ),
                || Err(CommonArticulationDynamicClosureBridgeStopV2::Cancelled),
            )
            .expect_err("checkpoint precedes foreign revalidation input"),
        CommonArticulationDynamicClosureBridgeErrorV2::Cancelled,
    );
}

#[test]
fn bridge_rejects_a_one_short_general_n_cap_before_bundle_work() {
    let fixture = miura_fixture_v2(N33_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(fixture.input(&profile)).unwrap();
    let parent_schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.geometry.face_ids()[0],
        [0.0, 1.0],
        zero_cycle_schedule_entries_v2(&fixture.geometry),
        CycleScheduleLimitsV1 {
            max_hinges: fixture.geometry.hinges().len(),
            max_degree: 0,
            max_coefficient_bits: 1,
            max_work: LIMIT,
        },
    )
    .unwrap();
    let mut one_short = bridge_limits_v2();
    one_short.max_blocks = N33_BLOCKS - 1;
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &fixture,
            &profile,
            &fixture.pose,
            &common_pose,
            &parent_schedule,
            one_short,
        ))
        .expect_err("N32 resource cap cannot admit N33"),
        CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput,
    );
    let mut one_over = bridge_limits_v2();
    one_over.max_blocks = N33_BLOCKS + 1;
    assert_eq!(
        prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &fixture,
            &profile,
            &fixture.pose,
            &common_pose,
            &parent_schedule,
            one_over,
        ))
        .expect_err("N34 policy cannot impersonate an exact-N33 profile"),
        CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput,
    );
}
