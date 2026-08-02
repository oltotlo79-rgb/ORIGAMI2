use super::*;
use crate::CycleSchedulePrepareErrorV1;

#[test]
fn root_angle_range_overapproximation_is_split_instead_of_rejected() {
    let mut fixture = nonstationary_exact_tree_fixture();
    let edge = fixture.geometry.hinges()[0].edge();
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: 1,
        max_degree: 2,
        max_coefficient_bits: 53,
        max_work: 4_096,
    };
    fixture.ordinary = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        [-1.0, 1.0],
        vec![CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: 1.1_f64.to_bits(),
            // 0.5*T0 + 0.5*T2 == x^2. The function is in range, while
            // dependency-losing root Clenshaw intervals cross below zero.
            chebyshev_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 2,
                },
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 2,
                },
            ],
        }],
        schedule_limits,
    )
    .unwrap();
    fixture.schedule_limits = schedule_limits;
    let bound = fixture
        .ordinary
        .checked_dyadic_workspace_upper_bound_v2(8, schedule_limits)
        .unwrap();
    assert_eq!(
        fixture
            .ordinary
            .evaluate_angle_box_dyadic_with_workspace_v2(
                0,
                0,
                schedule_limits,
                bound,
                bound.peak_bytes(),
            )
            .unwrap_err(),
        CycleScheduleDyadicEvaluationErrorV2::Prepare(CycleSchedulePrepareErrorV1::AngleRange)
    );
    let mut limits = generous_limits(schedule_limits);
    limits.max_depth = 8;
    limits.max_leaves = 256;
    limits.max_schedule_evaluation_workspace_bytes = bound.peak_bytes() + 1;
    let material = issue(&fixture, &fixture.ordinary, limits)
        .expect("overprovisioned schedule cap remains an upper-bound policy");
    assert!(material.partition().len() > 1);
    assert!(material.has_nonempty_canonical_complete_partition_v2());
}

#[test]
fn every_usize_max_limit_is_rejected_as_resource_limit() {
    let fixture = fixture();
    let base = generous_limits(fixture.schedule_limits);
    let mut cases = Vec::new();
    macro_rules! maximum {
        ($field:ident) => {{
            let mut candidate = base;
            candidate.$field = usize::MAX;
            cases.push(candidate);
        }};
    }
    maximum!(max_leaves);
    maximum!(max_work);
    maximum!(max_theorem_recognizer_work);
    maximum!(max_theorem_recognizer_workspace_bytes);
    maximum!(max_carrier_index_workspace_bytes);
    maximum!(max_schedule_evaluation_workspace_bytes);
    maximum!(max_big_rational_payload_bytes);
    maximum!(max_exact_rational_object_bytes);
    maximum!(max_interval_closure_workspace_bytes);
    maximum!(max_partition_workspace_bytes);
    maximum!(max_retained_material_bytes);
    maximum!(max_publication_workspace_bytes);
    maximum!(max_peak_workspace_bytes);
    for field in 0..3 {
        let mut candidate = base;
        match field {
            0 => candidate.schedule_limits.max_hinges = usize::MAX,
            1 => candidate.schedule_limits.max_degree = usize::MAX,
            _ => candidate.schedule_limits.max_work = usize::MAX,
        }
        cases.push(candidate);
    }
    let mut max_coefficient_bits = base;
    max_coefficient_bits.schedule_limits.max_coefficient_bits = u32::MAX;
    cases.push(max_coefficient_bits);
    for candidate in cases {
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }
    for depth in [64, u32::MAX] {
        let mut candidate = base;
        candidate.max_depth = depth;
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::InvalidInput
            ))
        ));
    }
}

#[test]
fn finite_preflight_rejects_one_short_and_invalid_schedule_limits_before_binding_scans() {
    let fixture = fixture();
    let base = generous_limits(fixture.schedule_limits);
    let schedule_bound = fixture
        .exact
        .checked_dyadic_workspace_upper_bound_v2(base.max_depth, fixture.schedule_limits)
        .unwrap();

    let mut cases = Vec::new();
    let mut one_short_workspace = base;
    one_short_workspace.max_schedule_evaluation_workspace_bytes = schedule_bound.peak_bytes() - 1;
    cases.push(one_short_workspace);
    let mut invalid_schedule_limit = base;
    invalid_schedule_limit.schedule_limits.max_hinges = fixture.schedule_limits.max_hinges - 1;
    cases.push(invalid_schedule_limit);

    for limits in cases {
        let mut schedule_polls = 0usize;
        let _ = fixture
            .exact
            .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(
                limits.max_depth,
                limits.schedule_limits,
                || {
                    schedule_polls += 1;
                    Ok(())
                },
            );
        let mut polls = 0usize;
        let result = fixture
            .geometry
            .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                &fixture.exact,
                1.0e-8,
                limits,
                || {
                    polls += 1;
                    Ok(())
                },
            );
        assert!(matches!(
            result,
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
        assert_eq!(
            polls,
            schedule_polls + 1,
            "resource preflight must precede binding scans"
        );
    }
}

#[test]
fn foreign_audit_carrier_and_checked_arithmetic_overflow_fail_closed() {
    let fixture = fixture();
    let limits = generous_limits(fixture.schedule_limits);
    let mut foreign_audit = fixture.audit.clone();
    foreign_audit.closure_hinges[0] = EdgeId::derive_v5(
        ProjectId::schema_namespace([
            0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e,
            0x7f, 0x80,
        ]),
        b"foreign-audit-edge",
    );
    let mut canonical_indices = (0..fixture.geometry.hinges().len()).collect::<Vec<_>>();
    canonical_indices
        .sort_unstable_by_key(|index| fixture.geometry.hinges()[*index].edge().canonical_bytes());
    let canonical_edges = canonical_indices
        .iter()
        .map(|index| fixture.geometry.hinges()[*index].edge())
        .collect::<Vec<_>>();
    let mut checkpoint = || -> Result<(), DyadicIntervalClosureStopV1> { Ok(()) };
    assert!(
        !validate_carrier_with_checkpoint_v2(
            &fixture.geometry,
            &foreign_audit,
            &canonical_indices,
            &canonical_edges,
            &mut checkpoint,
        )
        .unwrap()
    );
    let foreign = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &foreign_audit,
            fixture.fixed_face,
            &fixture.exact,
            1.0e-8,
            limits,
            || Ok(()),
        );
    assert!(matches!(
        foreign,
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::InvalidInput
        ))
    ));

    let mut partition_overflow = limits;
    partition_overflow.max_leaves = usize::MAX - 1;
    assert!(matches!(
        issue(&fixture, &fixture.exact, partition_overflow),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
    let mut exact_overflow = limits;
    exact_overflow.schedule_limits.max_work = usize::MAX - 1;
    assert!(matches!(
        issue(&fixture, &fixture.exact, exact_overflow),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
}

#[test]
fn stop_class_is_exact_at_entry_and_publication() {
    let fixture = fixture();
    let limits = generous_limits(fixture.schedule_limits);
    let mut polls = 0usize;
    fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.exact,
            1.0e-8,
            limits,
            || {
                polls += 1;
                Ok(())
            },
        )
        .unwrap();
    let successful_poll_count = polls;
    assert!(successful_poll_count > 1);
    for stop in [
        DyadicIntervalClosureStopV1::Cancelled,
        DyadicIntervalClosureStopV1::DeadlineExceeded,
    ] {
        for stop_at in 1..=successful_poll_count {
            let mut polls = 0usize;
            let result = fixture
                .geometry
                .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
                    &fixture.audit,
                    fixture.fixed_face,
                    &fixture.exact,
                    1.0e-8,
                    limits,
                    || {
                        polls += 1;
                        if polls == stop_at { Err(stop) } else { Ok(()) }
                    },
                );
            match stop {
                DyadicIntervalClosureStopV1::Cancelled => assert!(matches!(
                    result,
                    Err(DyadicIntervalClosureControlErrorV1::Cancelled)
                )),
                DyadicIntervalClosureStopV1::DeadlineExceeded => assert!(matches!(
                    result,
                    Err(DyadicIntervalClosureControlErrorV1::DeadlineExceeded)
                )),
            }
        }
    }
}

#[test]
fn adaptive_split_has_tight_depth_leaf_work_and_split_stop_boundaries() {
    let fixture = adaptive_correlated_cycle_fixture();
    let tolerance = 0.1;
    let mut limits = generous_limits(fixture.schedule_limits);
    limits.max_depth = 2;
    limits.max_leaves = 4;
    limits.max_work = 7_202;
    let material = issue_at_tolerance(&fixture, &fixture.ordinary, limits, tolerance)
        .expect("the fixed correlated schedule closes on four depth-two leaves");
    assert!(fixture.geometry.hinges().iter().all(|hinge| {
        fixture
            .ordinary
            .derivative_bound(hinge.edge())
            .is_some_and(|bound| bound > 0.0)
    }));
    assert!(material.has_nonempty_canonical_complete_partition_v2());
    assert_eq!(material.partition(), &[(2, 0), (2, 1), (2, 2), (2, 3)]);
    let resources = material.resources();
    assert_eq!(resources.charged_theorem_recognizer_upper_bound_bytes, 0);
    assert_eq!(resources.issued_leaves, 4);
    assert_eq!(resources.visited_partition_nodes, 7);
    let legacy = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            tolerance,
            DyadicIntervalClosureLimitsV1 {
                max_depth: limits.max_depth,
                max_leaves: limits.max_leaves,
                max_work: limits.max_work,
                schedule_limits: limits.schedule_limits,
            },
        )
        .unwrap();
    assert_eq!(
        legacy
            .leaves()
            .iter()
            .map(|(depth, index, _)| (*depth, *index))
            .collect::<Vec<_>>(),
        material.partition()
    );

    let mut depth_short = limits;
    depth_short.max_depth = 1;
    assert!(matches!(
        issue_at_tolerance(&fixture, &fixture.ordinary, depth_short, tolerance),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::UnprovenClosure { .. }
        ))
    ));
    let mut leaves_short = limits;
    leaves_short.max_leaves = 3;
    assert!(matches!(
        issue_at_tolerance(&fixture, &fixture.ordinary, leaves_short, tolerance),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
    let mut work_exact = limits;
    work_exact.max_work = 3_995;
    assert!(issue_at_tolerance(&fixture, &fixture.ordinary, work_exact, tolerance).is_ok());
    let mut work_short = limits;
    work_short.max_work = 3_994;
    assert!(matches!(
        issue_at_tolerance(&fixture, &fixture.ordinary, work_short, tolerance),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));

    let mut depth_zero = limits;
    depth_zero.max_depth = 0;
    depth_zero.max_leaves = 1;
    let mut root_polls = 0usize;
    let root = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            tolerance,
            depth_zero,
            || {
                root_polls += 1;
                Ok(())
            },
        );
    assert!(matches!(
        root,
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::UnprovenClosure { .. }
        ))
    ));
    let split_poll = root_polls + 1;
    let mut polls = 0usize;
    let stopped = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            tolerance,
            limits,
            || {
                polls += 1;
                if polls == split_poll {
                    Err(DyadicIntervalClosureStopV1::Cancelled)
                } else {
                    Ok(())
                }
            },
        );
    assert!(matches!(
        stopped,
        Err(DyadicIntervalClosureControlErrorV1::Cancelled)
    ));
}
