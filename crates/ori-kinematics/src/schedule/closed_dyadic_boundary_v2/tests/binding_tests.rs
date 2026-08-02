use super::*;

fn constant_schedule_v2() -> CanonicalCycleScheduleV1 {
    ordinary_schedule_v2(
        [-1.0, 1.0],
        vec![ordinary_entry_v2(b"constant", 90.0, vec![0.0])],
    )
}

#[test]
fn closed_dyadic_boundary_tags_separate_equal_lower_and_upper_values() {
    let evidence = prove_exact_v2(&constant_schedule_v2(), CycleScheduleLimitsV1::default());
    assert_ne!(
        evidence.lower_boundary_binding_fingerprint, evidence.upper_boundary_binding_fingerprint,
        "endpoint tags must distinguish equal numeric configurations"
    );
}

#[test]
fn closed_dyadic_boundary_aggregate_binding_rejects_endpoint_swap() {
    let evidence = prove_exact_v2(&constant_schedule_v2(), CycleScheduleLimitsV1::default());
    let limits = CycleScheduleLimitsV1::default();
    let retained = binding::evidence_binding_fingerprint_v2(
        BoundaryRepresentationV2::Ordinary,
        evidence.schedule_binding_fingerprint_v2(),
        evidence.graph_binding_fingerprint,
        evidence.lower_boundary_binding_fingerprint,
        evidence.upper_boundary_binding_fingerprint,
        evidence.hinge_count_v2(),
        limits,
        evidence.logical_work_v2(),
        evidence.workspace_peak_bytes_upper_bound_v2(),
    )
    .unwrap();
    let swapped = binding::evidence_binding_fingerprint_v2(
        BoundaryRepresentationV2::Ordinary,
        evidence.schedule_binding_fingerprint_v2(),
        evidence.graph_binding_fingerprint,
        evidence.upper_boundary_binding_fingerprint,
        evidence.lower_boundary_binding_fingerprint,
        evidence.hinge_count_v2(),
        limits,
        evidence.logical_work_v2(),
        evidence.workspace_peak_bytes_upper_bound_v2(),
    )
    .unwrap();
    assert_eq!(retained, evidence.binding_fingerprint_v2());
    assert_ne!(retained, swapped);
}

#[test]
fn closed_dyadic_boundary_binding_changes_for_one_ulp_coefficient_and_domain_drift() {
    let baseline = ordinary_schedule_v2(
        [-3.0, 7.0],
        vec![ordinary_entry_v2(b"drift", 90.0, vec![0.0, 2.0])],
    );
    let coefficient_drift = ordinary_schedule_v2(
        [-3.0, 7.0],
        vec![ordinary_entry_v2(
            b"drift",
            90.0,
            vec![0.0, f64::from_bits(2.0_f64.to_bits() + 1)],
        )],
    );
    let domain_drift = ordinary_schedule_v2(
        [-3.0, f64::from_bits(7.0_f64.to_bits() + 1)],
        vec![ordinary_entry_v2(b"drift", 90.0, vec![0.0, 2.0])],
    );
    let limits = CycleScheduleLimitsV1::default();
    let baseline = prove_exact_v2(&baseline, limits);
    assert_ne!(
        baseline.binding_fingerprint_v2(),
        prove_exact_v2(&coefficient_drift, limits).binding_fingerprint_v2()
    );
    assert_ne!(
        baseline.binding_fingerprint_v2(),
        prove_exact_v2(&domain_drift, limits).binding_fingerprint_v2()
    );
}

#[test]
fn closed_dyadic_boundary_binding_separates_representation_kind() {
    let ordinary = constant_schedule_v2();
    let half_angle = half_angle_schedule_v2(vec![HalfAngleRationalEntryInputV1 {
        edge: test_edge_v2(b"constant"),
        u_domain: [rational_v2(0, 1), rational_v2(1, 1)],
        numerator_power_coefficients: vec![rational_v2(1, 1)],
        denominator_power_coefficients: vec![rational_v2(1, 1)],
    }]);
    assert_ne!(
        prove_exact_v2(&ordinary, CycleScheduleLimitsV1::default()).binding_fingerprint_v2(),
        prove_exact_v2(&half_angle, CycleScheduleLimitsV1::default()).binding_fingerprint_v2()
    );
}

#[test]
fn closed_dyadic_boundary_aggregate_binding_binds_every_schedule_limit_field() {
    let evidence = prove_exact_v2(&constant_schedule_v2(), CycleScheduleLimitsV1::default());
    let baseline_limits = CycleScheduleLimitsV1::default();
    let baseline = evidence.binding_fingerprint_v2();
    let drifts = [
        CycleScheduleLimitsV1 {
            max_hinges: baseline_limits.max_hinges + 1,
            ..baseline_limits
        },
        CycleScheduleLimitsV1 {
            max_degree: baseline_limits.max_degree + 1,
            ..baseline_limits
        },
        CycleScheduleLimitsV1 {
            max_coefficient_bits: baseline_limits.max_coefficient_bits + 1,
            ..baseline_limits
        },
        CycleScheduleLimitsV1 {
            max_work: baseline_limits.max_work + 1,
            ..baseline_limits
        },
    ];
    for drift in drifts {
        let rebound = binding::evidence_binding_fingerprint_v2(
            BoundaryRepresentationV2::Ordinary,
            evidence.schedule_binding_fingerprint_v2(),
            evidence.graph_binding_fingerprint,
            evidence.lower_boundary_binding_fingerprint,
            evidence.upper_boundary_binding_fingerprint,
            evidence.hinge_count_v2(),
            drift,
            evidence.logical_work_v2(),
            evidence.workspace_peak_bytes_upper_bound_v2(),
        )
        .unwrap();
        assert_ne!(baseline, rebound);
    }
}
