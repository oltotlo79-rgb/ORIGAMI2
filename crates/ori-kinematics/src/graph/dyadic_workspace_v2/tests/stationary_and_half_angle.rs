use super::*;
use crate::CycleScheduleDyadicEvaluationStopV2;

#[test]
fn high_degree_half_angle_exact_scans_honor_middle_and_final_stops() {
    let fixture = nonstationary_exact_tree_fixture();
    let edge = fixture.geometry.hinges()[0].edge();
    let rational = |numerator, denominator| RationalCoefficientV1 {
        numerator,
        denominator,
    };
    let degree = 12usize;
    let limits = CycleScheduleLimitsV1 {
        max_hinges: 1,
        max_degree: degree,
        max_coefficient_bits: 4_096,
        max_work: 1 << 20,
    };
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        vec![HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [rational(0, 1), rational(1, 1)],
            numerator_power_coefficients: (0..=degree).map(|_| rational(1, 1)).collect(),
            denominator_power_coefficients: (0..=degree)
                .map(|index| rational(if index == 0 { 2 } else { 1 }, 1))
                .collect(),
        }],
        limits,
    )
    .unwrap();
    let bound = schedule
        .checked_dyadic_workspace_upper_bound_v2(2, limits)
        .unwrap();
    let mut bound_scan_polls = 0usize;
    assert_eq!(
        schedule
            .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(2, limits, || {
                bound_scan_polls += 1;
                Ok(())
            })
            .unwrap(),
        bound
    );
    assert!(bound_scan_polls > degree * 4);
    for (schedule_stop_at, stop) in [
        (2, DyadicIntervalClosureStopV1::Cancelled),
        (
            bound_scan_polls / 2,
            DyadicIntervalClosureStopV1::DeadlineExceeded,
        ),
        (bound_scan_polls, DyadicIntervalClosureStopV1::Cancelled),
    ] {
        let graph_stop_at = schedule_stop_at + 1;
        let mut graph_polls = 0usize;
        let result = fixture
            .geometry
            .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                &schedule,
                1.0e-8,
                generous_limits(limits),
                || {
                    graph_polls += 1;
                    if graph_polls == graph_stop_at {
                        Err(stop)
                    } else {
                        Ok(())
                    }
                },
            );
        assert!(matches!(
            (stop, result),
            (
                DyadicIntervalClosureStopV1::Cancelled,
                Err(DyadicIntervalClosureControlErrorV1::Cancelled)
            ) | (
                DyadicIntervalClosureStopV1::DeadlineExceeded,
                Err(DyadicIntervalClosureControlErrorV1::DeadlineExceeded)
            )
        ));
    }
    let mut successful_polls = 0usize;
    schedule
        .evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
            2,
            1,
            limits,
            bound,
            bound.peak_bytes(),
            || {
                successful_polls += 1;
                Ok(())
            },
        )
        .unwrap();
    assert!(successful_polls > degree * degree);

    for (stop_at, expected) in [
        (
            successful_polls / 2,
            CycleScheduleDyadicEvaluationErrorV2::Cancelled,
        ),
        (
            successful_polls,
            CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded,
        ),
    ] {
        let mut polls = 0usize;
        assert_eq!(
            schedule
                .evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
                    2,
                    1,
                    limits,
                    bound,
                    bound.peak_bytes(),
                    || {
                        polls += 1;
                        if polls == stop_at {
                            Err(
                                if expected == CycleScheduleDyadicEvaluationErrorV2::Cancelled {
                                    CycleScheduleDyadicEvaluationStopV2::Cancelled
                                } else {
                                    CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                                },
                            )
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn private_metered_schedule_carrier_polls_nested_work_and_prepublication() {
    let fixture = nonstationary_exact_tree_fixture();
    let bound = fixture
        .exact
        .checked_dyadic_workspace_upper_bound_v2(2, fixture.schedule_limits)
        .unwrap();
    let mut successful_polls = 0usize;
    let evaluation = fixture
        .exact
        .evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
            2,
            0,
            fixture.schedule_limits,
            bound,
            bound.peak_bytes(),
            || {
                successful_polls += 1;
                Ok(())
            },
        )
        .unwrap();
    assert!(!evaluation.angle_boxes().is_empty());
    assert!(successful_polls > fixture.geometry.hinges().len());

    let mut middle_polls = 0usize;
    assert_eq!(
        fixture
            .exact
            .evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
                2,
                0,
                fixture.schedule_limits,
                bound,
                bound.peak_bytes(),
                || {
                    middle_polls += 1;
                    if middle_polls == successful_polls / 2 {
                        Err(CycleScheduleDyadicEvaluationStopV2::Cancelled)
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err(),
        CycleScheduleDyadicEvaluationErrorV2::Cancelled
    );
    let mut final_polls = 0usize;
    assert_eq!(
        fixture
            .exact
            .evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
                2,
                0,
                fixture.schedule_limits,
                bound,
                bound.peak_bytes(),
                || {
                    final_polls += 1;
                    if final_polls == successful_polls {
                        Err(CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded)
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err(),
        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded
    );

    let mut retained_polls = 0usize;
    let retained = fixture
        .exact
        .checked_deep_retained_bytes_with_checkpoint_v2(usize::MAX, || {
            retained_polls += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        Some(retained),
        fixture.exact.checked_deep_retained_bytes_v1()
    );
    assert!(retained_polls > fixture.geometry.hinges().len());
}

#[test]
fn exact_half_angle_workspace_is_tight_and_every_byte_one_short_fails() {
    let fixture = nonstationary_exact_tree_fixture();
    let generous = generous_limits(fixture.schedule_limits);
    assert!(fixture.geometry.hinges().iter().all(|hinge| {
        fixture
            .exact
            .derivative_bound(hinge.edge())
            .is_some_and(|bound| bound > 0.0)
    }));
    let bound = fixture
        .exact
        .checked_dyadic_workspace_upper_bound_v2(0, fixture.schedule_limits)
        .unwrap();
    let legacy_boxes = fixture
        .exact
        .evaluate_angle_box_dyadic(0, 0, fixture.schedule_limits)
        .unwrap();
    let metered_evaluation = fixture
        .exact
        .evaluate_angle_box_dyadic_with_workspace_v2(
            0,
            0,
            fixture.schedule_limits,
            bound,
            bound.peak_bytes(),
        )
        .unwrap();
    assert_eq!(metered_evaluation.angle_boxes(), legacy_boxes);
    assert!(metered_evaluation.exact_vector_capacity_peak_bytes() > 0);
    let first = issue(&fixture, &fixture.exact, generous).unwrap();
    let resources = first.resources();
    assert!(resources.charged_big_rational_payload_upper_bound_bytes > 0);
    assert!(resources.charged_theorem_recognizer_work > 0);
    assert_eq!(resources.charged_theorem_recognizer_upper_bound_bytes, 0);
    assert_eq!(first.partition(), &[(0, 0)]);
    assert_eq!(
        first.canonical_checked_hinges().len(),
        fixture.geometry.hinges().len()
    );
    assert!(first.has_nonempty_canonical_complete_partition_v2());
    assert!(first.issuer_geometry.matches(&fixture.geometry));
    assert_eq!(first.fixed_face, fixture.fixed_face);
    assert_eq!(first.tolerance_bits, 1.0e-8_f64.to_bits());
    assert_eq!(first.policy, generous);
    assert_eq!(
        first.schedule_binding_fingerprint_v2,
        fixture.exact.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        first.graph_binding_fingerprint_v1,
        fixture.exact.graph_binding_fingerprint_v1()
    );
    let exact = exact_limits(generous, resources);
    let second = issue(&fixture, &fixture.exact, exact).unwrap();
    assert_eq!(second.resources(), resources);
    assert_ne!(
        second.partition_binding_fingerprint_v2(),
        first.partition_binding_fingerprint_v2()
    );
    let mut exact_object_policy_mutation = generous;
    exact_object_policy_mutation.max_exact_rational_object_bytes -= 1;
    let policy_mutated = issue(&fixture, &fixture.exact, exact_object_policy_mutation).unwrap();
    assert_ne!(
        policy_mutated.partition_binding_fingerprint_v2(),
        first.partition_binding_fingerprint_v2()
    );

    let mut cases = Vec::new();
    macro_rules! one_short {
        ($field:ident, $resource:ident) => {{
            assert!(resources.$resource > 0);
            let mut candidate = exact;
            candidate.$field = resources.$resource - 1;
            cases.push(candidate);
        }};
    }
    one_short!(max_theorem_recognizer_work, charged_theorem_recognizer_work);
    one_short!(
        max_carrier_index_workspace_bytes,
        charged_carrier_index_workspace_upper_bound_bytes
    );
    one_short!(
        max_schedule_evaluation_workspace_bytes,
        charged_schedule_evaluation_workspace_upper_bound_bytes
    );
    one_short!(
        max_big_rational_payload_bytes,
        charged_big_rational_payload_upper_bound_bytes
    );
    one_short!(
        max_exact_rational_object_bytes,
        charged_exact_rational_object_upper_bound_bytes
    );
    one_short!(
        max_interval_closure_workspace_bytes,
        charged_interval_closure_workspace_upper_bound_bytes
    );
    one_short!(
        max_partition_workspace_bytes,
        charged_partition_workspace_upper_bound_bytes
    );
    one_short!(
        max_retained_material_bytes,
        charged_retained_material_upper_bound_bytes
    );
    one_short!(
        max_publication_workspace_bytes,
        charged_publication_workspace_upper_bound_bytes
    );
    one_short!(
        max_peak_workspace_bytes,
        charged_peak_workspace_upper_bound_bytes
    );
    for candidate in cases {
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }

    let mut hinge_short = generous;
    hinge_short.schedule_limits.max_hinges = 0;
    let mut degree_short = generous;
    degree_short.schedule_limits.max_degree = 0;
    let mut bits_exact = generous;
    bits_exact.schedule_limits.max_coefficient_bits = 2;
    assert!(issue(&fixture, &fixture.exact, bits_exact).is_ok());
    let mut bits_short = generous;
    bits_short.schedule_limits.max_coefficient_bits = 1;
    let mut work_exact = generous;
    work_exact.schedule_limits.max_work = 297;
    assert!(issue(&fixture, &fixture.exact, work_exact).is_ok());
    let mut work_short = generous;
    work_short.schedule_limits.max_work = 296;
    for candidate in [hinge_short, degree_short, bits_short, work_short] {
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }

    let legacy = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.exact,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1_000_000,
                schedule_limits: fixture.schedule_limits,
            },
        )
        .unwrap();
    assert_eq!(legacy.leaves().len(), first.partition().len());
}

#[test]
fn legacy_v1_stationary_partition_and_binding_remain_unchanged() {
    let fixture = fixture();
    let legacy = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1_000_000,
                schedule_limits: fixture.schedule_limits,
            },
        )
        .unwrap();
    let binding_before = legacy.partition_binding_fingerprint_v2();
    assert_eq!(legacy.leaves().len(), 1);
    assert!(legacy.has_canonical_complete_partition_v1());
    assert!(legacy.every_leaf_covers_graph_v1(&fixture.geometry));

    let v2 = issue(
        &fixture,
        &fixture.ordinary,
        generous_limits(fixture.schedule_limits),
    )
    .unwrap();
    assert_eq!(v2.partition(), &[(0, 0)]);
    assert_eq!(
        legacy.leaves()[0].2.checked_hinges(),
        v2.canonical_checked_hinges()
    );
    assert_eq!(binding_before, legacy.partition_binding_fingerprint_v2());
}
