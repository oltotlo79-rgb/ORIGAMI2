use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleEntryInputV1, CycleScheduleLimitsV1, Point3,
};
use serde::de::DeserializeOwned;

use super::{
    Meter, MultiHingeReliefUnionErrorV2, MultiHingeReliefUnionLimitsV2,
    certify_multi_hinge_relief_union_v2, certify_multi_hinge_relief_union_with_cancel_v2,
    compound_corridor::{
        CompoundCorridorBindingV2, CompoundCorridorSegmentInputV2,
        normalize_compound_corridor_pair_v2,
    },
    diagnose_multi_hinge_relief_union_gaps_v2,
    tests::{relief, segmented_crease},
};

fn id<T: DeserializeOwned>(prefix: &str, suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{suffix:012x}\"")).unwrap()
}

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn pair() -> [FaceId; 2] {
    [id("b301", 1), id("b301", 2)]
}

fn binding() -> CompoundCorridorBindingV2 {
    CompoundCorridorBindingV2 {
        radial_depth_bits: 7.0_f64.to_bits(),
        thickness_bits: 0.1_f64.to_bits(),
        bevel_angle_bits: 1.0_f64.to_bits(),
        source_angle_bits: 90.0_f64.to_bits(),
        target_angle_bits: 90.0_f64.to_bits(),
        derivative_bound_bits: 0.0_f64.to_bits(),
        assignment_tag: 0x4d,
    }
}

fn segment(index: u64, lower: f64, upper: f64) -> CompoundCorridorSegmentInputV2 {
    CompoundCorridorSegmentInputV2 {
        pair: pair(),
        hinge: id::<EdgeId>("b302", index),
        start: point(lower, 0.0, 0.0),
        end: point(upper, 0.0, 0.0),
        binding: binding(),
    }
}

fn normalize(
    inputs: &[CompoundCorridorSegmentInputV2],
    limits: MultiHingeReliefUnionLimitsV2,
) -> Result<
    (
        super::compound_corridor::CompoundLogicalCorridorCertificateV2,
        usize,
        usize,
    ),
    MultiHingeReliefUnionErrorV2,
> {
    let mut meter = Meter::new(limits)?;
    let certificate = normalize_compound_corridor_pair_v2(inputs, [0x5a; 32], limits, &mut meter)?;
    Ok((certificate, meter.work, meter.peak))
}

#[test]
fn production_two_and_three_splits_issue_only_an_internal_non_authorizing_corridor() {
    for hinge_count in [2_usize, 3] {
        let (geometry, audit, schedule, fixed) = segmented_crease(hinge_count, 1);
        let limits = MultiHingeReliefUnionLimitsV2::default();
        let gaps = diagnose_multi_hinge_relief_union_gaps_v2(
            &geometry, &audit, fixed, &schedule, 0.1, limits,
        )
        .unwrap();
        let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);
        let certificate = certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        )
        .unwrap();

        assert_eq!(certificate.compound_corridors.len(), 1);
        let compound = &certificate.compound_corridors[0];
        assert_eq!(compound.pair(), gaps.gaps()[0].pair());
        assert_eq!(compound.hinges().len(), hinge_count);
        assert!(!compound.authorizes_continuous_motion());
        assert!(!compound.authorizes_collision_free_classification());
        assert!(!compound.authorizes_project_mutation());
        assert!(!certificate.authorizes_continuous_motion());
        assert!(!certificate.authorizes_collision_free_classification());
        assert!(!certificate.authorizes_project_mutation());
    }
}

#[test]
fn cancellation_after_compound_normalization_never_returns_a_certificate() {
    let (geometry, audit, schedule, fixed) = segmented_crease(3, 1);
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let gaps =
        diagnose_multi_hinge_relief_union_gaps_v2(&geometry, &audit, fixed, &schedule, 0.1, limits)
            .unwrap();
    let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);

    let mut total_checkpoints = 0_usize;
    let certificate = certify_multi_hinge_relief_union_with_cancel_v2(
        &gaps,
        &geometry,
        &audit,
        fixed,
        &schedule,
        0.1,
        &prerequisite,
        &local,
        &policies,
        &schedules,
        policy_limits,
        limits,
        || {
            total_checkpoints += 1;
            false
        },
    )
    .unwrap();
    assert_eq!(certificate.compound_corridors.len(), 1);
    assert!(total_checkpoints >= 2);

    // The last checkpoint is the outer pre-return gate. Therefore the
    // penultimate checkpoint is reached only after every private compound
    // corridor has been normalized and retained.
    let late_checkpoint = total_checkpoints - 1;
    let mut replay_checkpoints = 0_usize;
    let result = certify_multi_hinge_relief_union_with_cancel_v2(
        &gaps,
        &geometry,
        &audit,
        fixed,
        &schedule,
        0.1,
        &prerequisite,
        &local,
        &policies,
        &schedules,
        policy_limits,
        limits,
        || {
            replay_checkpoints += 1;
            replay_checkpoints == late_checkpoint
        },
    );
    assert!(matches!(
        result,
        Err(MultiHingeReliefUnionErrorV2::Cancelled)
    ));
    assert_eq!(replay_checkpoints, late_checkpoint);
}

#[test]
fn canonicalization_is_invariant_to_input_order_and_segment_orientation() {
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let forward = vec![
        segment(30, 0.0, 1.0),
        segment(10, 1.0, 2.0),
        segment(20, 2.0, 3.0),
    ];
    let (expected, expected_work, expected_peak) = normalize(&forward, limits).unwrap();

    let mut reversed = forward
        .iter()
        .rev()
        .map(|segment| CompoundCorridorSegmentInputV2 {
            start: segment.end,
            end: segment.start,
            ..*segment
        })
        .collect::<Vec<_>>();
    reversed.rotate_left(1);
    let (actual, actual_work, actual_peak) = normalize(&reversed, limits).unwrap();

    assert_eq!(actual, expected);
    assert_eq!(actual_work, expected_work);
    assert_eq!(actual_peak, expected_peak);

    let diagonal = vec![
        CompoundCorridorSegmentInputV2 {
            start: point(0.0, 0.0, 0.0),
            end: point(7.0, 11.0, 0.0),
            ..segment(1, 0.0, 1.0)
        },
        CompoundCorridorSegmentInputV2 {
            start: point(7.0, 11.0, 0.0),
            end: point(14.0, 22.0, 0.0),
            ..segment(2, 1.0, 2.0)
        },
    ];
    assert!(
        normalize(&diagonal, limits).is_ok(),
        "an exactly collinear general-direction split must not depend on rounded unit axes"
    );

    assert_eq!(
        actual
            .hinges()
            .iter()
            .map(|edge| edge.canonical_bytes())
            .collect::<Vec<_>>(),
        vec![
            id::<EdgeId>("b302", 30).canonical_bytes(),
            id::<EdgeId>("b302", 10).canonical_bytes(),
            id::<EdgeId>("b302", 20).canonical_bytes(),
        ]
    );
}

#[test]
fn compound_preflight_work_and_storage_limits_are_exact_and_one_short() {
    let generous = MultiHingeReliefUnionLimitsV2::default();
    let inputs = vec![
        segment(1, 0.0, 1.0),
        segment(2, 1.0, 2.0),
        segment(3, 2.0, 3.0),
    ];
    let (_, work, peak) = normalize(&inputs, generous).unwrap();
    let exact = MultiHingeReliefUnionLimitsV2 {
        max_work: work,
        max_storage_bytes: peak,
        ..generous
    };
    assert!(normalize(&inputs, exact).is_ok());
    for one_short in [
        MultiHingeReliefUnionLimitsV2 {
            max_work: work - 1,
            ..exact
        },
        MultiHingeReliefUnionLimitsV2 {
            max_storage_bytes: peak - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            normalize(&inputs, one_short),
            Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
        ));
    }
}

#[test]
fn noncollinear_gap_overlap_duplicate_foreign_pair_and_four_segments_are_rejected() {
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let baseline = vec![segment(1, 0.0, 1.0), segment(2, 1.0, 2.0)];

    let mut bent_line = baseline.clone();
    bent_line[1].end = point(2.0, f64::from_bits(1), 0.0);
    assert!(matches!(
        normalize(&bent_line, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut one_ulp_gap = baseline.clone();
    one_ulp_gap[1].start = point(f64::from_bits(1.0_f64.to_bits() + 1), 0.0, 0.0);
    assert!(matches!(
        normalize(&one_ulp_gap, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut overlap = baseline.clone();
    overlap[0].end = point(1.5, 0.0, 0.0);
    assert!(matches!(
        normalize(&overlap, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut duplicate = baseline.clone();
    duplicate[1].start = duplicate[0].start;
    duplicate[1].end = duplicate[0].end;
    assert!(matches!(
        normalize(&duplicate, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut duplicate_hinge = baseline.clone();
    duplicate_hinge[1].hinge = duplicate_hinge[0].hinge;
    assert!(matches!(
        normalize(&duplicate_hinge, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut foreign_pair = baseline.clone();
    foreign_pair[1].pair = [id("b303", 1), id("b303", 2)];
    assert!(matches!(
        normalize(&foreign_pair, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut noncanonical_pair = baseline.clone();
    let canonical = pair();
    for segment in &mut noncanonical_pair {
        segment.pair = [canonical[1], canonical[0]];
    }
    assert!(matches!(
        normalize(&noncanonical_pair, limits),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let four = vec![
        segment(1, 0.0, 1.0),
        segment(2, 1.0, 2.0),
        segment(3, 2.0, 3.0),
        segment(4, 3.0, 4.0),
    ];
    assert!(matches!(
        normalize(&four, limits),
        Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
    ));
}

#[test]
fn every_radial_depth_thickness_schedule_and_policy_binding_difference_is_rejected() {
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let baseline = vec![segment(1, 0.0, 1.0), segment(2, 1.0, 2.0)];
    let mutations: [fn(&mut CompoundCorridorBindingV2); 7] = [
        |value| value.radial_depth_bits += 1,
        |value| value.thickness_bits = 0.2_f64.to_bits(),
        |value| value.bevel_angle_bits = 2.0_f64.to_bits(),
        |value| value.source_angle_bits = 91.0_f64.to_bits(),
        |value| value.target_angle_bits = 91.0_f64.to_bits(),
        |value| value.derivative_bound_bits = 1.0_f64.to_bits(),
        |value| value.assignment_tag = 0x56,
    ];
    for mutate in mutations {
        let mut changed = baseline.clone();
        mutate(&mut changed[1].binding);
        assert!(matches!(
            normalize(&changed, limits),
            Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
        ));
    }
}

#[test]
fn shared_source_and_target_angles_outside_the_local_relief_range_are_rejected() {
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let baseline = vec![segment(1, 0.0, 1.0), segment(2, 1.0, 2.0)];
    let mutations: [fn(&mut CompoundCorridorBindingV2); 4] = [
        |value| value.source_angle_bits = 0.0_f64.to_bits(),
        |value| value.source_angle_bits = (f64::from_bits(180.0_f64.to_bits() + 1)).to_bits(),
        |value| value.target_angle_bits = 0.0_f64.to_bits(),
        |value| value.target_angle_bits = (f64::from_bits(180.0_f64.to_bits() + 1)).to_bits(),
    ];
    for mutate in mutations {
        let mut changed = baseline.clone();
        for segment in &mut changed {
            mutate(&mut segment.binding);
        }
        assert!(matches!(
            normalize(&changed, limits),
            Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
        ));
    }
}

#[test]
fn production_preflight_rejects_distinct_endpoint_summaries_and_relief_depths() {
    let (geometry, audit, _, fixed) = segmented_crease(3, 1);
    let mut entries = geometry
        .hinges()
        .iter()
        .enumerate()
        .map(|(index, hinge)| CycleScheduleEntryInputV1 {
            edge: hinge.edge(),
            initial_angle_degrees_bits: if index == 0 {
                90.0_f64.to_bits()
            } else {
                91.0_f64.to_bits()
            },
            chebyshev_coefficients: Vec::new(),
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let gaps =
        diagnose_multi_hinge_relief_union_gaps_v2(&geometry, &audit, fixed, &schedule, 0.1, limits)
            .unwrap();
    let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);
    assert!(matches!(
        certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        ),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let (geometry, audit, schedule, fixed) = segmented_crease(3, 2);
    let gaps =
        diagnose_multi_hinge_relief_union_gaps_v2(&geometry, &audit, fixed, &schedule, 0.1, limits)
            .unwrap();
    let (mut policies, schedules, _, _, policy_limits) = relief(&gaps, &geometry);
    policies[1].cutout_width_mm = 8.0;
    let prerequisite =
        crate::prepare_hinge_relief_prerequisite_v1(&geometry, 0.1, &policies, policy_limits)
            .unwrap();
    let local = crate::certify_hinge_relief_local_intervals_v1(
        &prerequisite,
        &geometry,
        0.1,
        &policies,
        &schedules,
        policy_limits,
    )
    .unwrap();
    assert!(matches!(
        certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        ),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));
}
