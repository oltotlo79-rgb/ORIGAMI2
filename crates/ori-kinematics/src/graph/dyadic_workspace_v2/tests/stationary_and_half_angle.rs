use super::*;

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
            usize::MAX - 1,
        )
        .unwrap();
    assert_eq!(metered_evaluation.angle_boxes, legacy_boxes);
    assert!(metered_evaluation.exact_vector_capacity_peak_bytes > 0);
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
