use super::*;

fn non_unit_half_angle_schedule_v2() -> CanonicalCycleScheduleV1 {
    half_angle_schedule_v2(vec![HalfAngleRationalEntryInputV1 {
        edge: test_edge_v2(b"non-unit-half-angle"),
        u_domain: [rational_v2(-1, 1), rational_v2(2, 1)],
        numerator_power_coefficients: vec![rational_v2(1, 1), rational_v2(1, 1)],
        denominator_power_coefficients: vec![rational_v2(64, 1)],
    }])
}

#[test]
fn closed_dyadic_boundary_half_angle_uses_exact_non_unit_u_domain_endpoints() {
    let schedule = non_unit_half_angle_schedule_v2();
    let limits = CycleScheduleLimitsV1::default();
    let mut meter = resources::BoundaryWorkMeterV2::new(10_000);
    for upper in [false, true] {
        let actual = evaluate::evaluate_half_angle_endpoint_box_v2(
            &schedule.half_angle_entries[0],
            upper,
            limits,
            &mut meter,
            &mut || Ok(()),
        )
        .unwrap();
        let existing = schedule.evaluate_endpoint_angle_box(upper, limits).unwrap();
        assert_eq!(existing.len(), 1);
        assert_eq!(actual, existing[0].1);
        let point = schedule.try_evaluate_v1(f64::from(upper)).unwrap();
        let angle = point.as_slice()[0].angle_degrees();
        assert!(actual.lower() <= angle && angle <= actual.upper());
    }
}

#[test]
fn closed_dyadic_boundary_half_angle_preserves_closed_projective_pole_endpoint() {
    let schedule = half_angle_schedule_v2(vec![HalfAngleRationalEntryInputV1 {
        edge: test_edge_v2(b"projective-pole"),
        u_domain: [rational_v2(0, 1), rational_v2(1, 1)],
        numerator_power_coefficients: vec![rational_v2(1, 1)],
        denominator_power_coefficients: vec![rational_v2(1, 1), rational_v2(-1, 1)],
    }]);
    let evidence = prove_exact_v2(&schedule, CycleScheduleLimitsV1::default());
    assert_eq!(evidence.canonical_boundary_count_v2(), 2);
    assert_ne!(
        evidence.lower_boundary_binding_fingerprint,
        evidence.upper_boundary_binding_fingerprint
    );
}

#[test]
fn closed_dyadic_boundary_half_angle_binding_has_cross_runtime_golden_vector() {
    let schedule = half_angle_schedule_v2(vec![
        HalfAngleRationalEntryInputV1 {
            edge: test_edge_v2(b"golden-half-a"),
            u_domain: [rational_v2(-2, 3), rational_v2(5, 7)],
            numerator_power_coefficients: vec![rational_v2(1, 2), rational_v2(-1, 7)],
            denominator_power_coefficients: vec![rational_v2(2, 3), rational_v2(1, 5)],
        },
        HalfAngleRationalEntryInputV1 {
            edge: test_edge_v2(b"golden-half-b"),
            u_domain: [rational_v2(0, 1), rational_v2(2, 5)],
            numerator_power_coefficients: vec![rational_v2(0, 1), rational_v2(1, 8)],
            denominator_power_coefficients: vec![rational_v2(1, 1)],
        },
    ]);
    let evidence = prove_exact_v2(&schedule, CycleScheduleLimitsV1::default());
    assert_eq!(
        fingerprint_hex_v2(evidence.binding_fingerprint_v2()),
        "cee894cd5f123b520003f8b94d2e60757c70f2845eda421350d40de697d8969c"
    );
}
