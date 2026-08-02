use super::*;

#[test]
fn closed_dyadic_boundary_ordinary_uses_normalized_x_endpoints_not_physical_zero_one() {
    let entry = ordinary_entry_v2(b"non-unit-domain", 90.0, vec![0.0, 10.0]);
    let schedule = ordinary_schedule_v2([10.0, 20.0], vec![entry]);
    assert!(schedule.try_evaluate_v1(0.0).is_err());
    assert!(schedule.try_evaluate_v1(1.0).is_err());

    let mut meter = resources::BoundaryWorkMeterV2::new(1_000);
    let mut checkpoint = || Ok(());
    let lower = evaluate::evaluate_ordinary_endpoint_angle_v2(
        &schedule.entries[0],
        -1.0,
        &mut meter,
        &mut checkpoint,
    )
    .unwrap();
    let upper = evaluate::evaluate_ordinary_endpoint_angle_v2(
        &schedule.entries[0],
        1.0,
        &mut meter,
        &mut checkpoint,
    )
    .unwrap();
    assert_eq!(lower.angle_degrees().to_bits(), 80.0_f64.to_bits());
    assert_eq!(upper.angle_degrees().to_bits(), 100.0_f64.to_bits());
    assert_eq!(
        schedule.try_evaluate_v1(10.0).unwrap().as_slice()[0]
            .angle_degrees()
            .to_bits(),
        lower.angle_degrees().to_bits()
    );
    assert_eq!(
        schedule.try_evaluate_v1(20.0).unwrap().as_slice()[0]
            .angle_degrees()
            .to_bits(),
        upper.angle_degrees().to_bits()
    );
}

#[test]
fn closed_dyadic_boundary_ordinary_canonicalizes_signed_zero() {
    let schedule = ordinary_schedule_v2(
        [-7.0, 11.0],
        vec![ordinary_entry_v2(b"signed-zero", -0.0, vec![-0.0])],
    );
    let mut meter = resources::BoundaryWorkMeterV2::new(1_000);
    let angle = evaluate::evaluate_ordinary_endpoint_angle_v2(
        &schedule.entries[0],
        -1.0,
        &mut meter,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(angle.angle_degrees().to_bits(), 0.0_f64.to_bits());
}

#[test]
fn closed_dyadic_boundary_ordinary_rejects_noncanonical_edge_order() {
    let mut entries = vec![
        ordinary_entry_v2(b"edge-a", 90.0, vec![0.0]),
        ordinary_entry_v2(b"edge-b", 90.0, vec![0.0]),
    ];
    entries.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.edge.canonical_bytes()));
    let schedule = ordinary_schedule_in_order_v2([-1.0, 1.0], entries);
    let bound = schedule
        .checked_closed_dyadic_boundary_resource_bound_v2(CycleScheduleLimitsV1::default())
        .unwrap();
    assert!(matches!(
        schedule.prove_closed_dyadic_boundary_evidence_v2(
            CycleScheduleLimitsV1::default(),
            bound.logical_work_required_v2(),
            bound.workspace_peak_bytes_upper_bound_v2(),
        ),
        Err(CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::NonCanonical,
        ))
    ));
}

#[test]
fn closed_dyadic_boundary_ordinary_binding_has_cross_runtime_golden_vector() {
    let schedule = ordinary_schedule_v2(
        [-2.5, 4.0],
        vec![
            ordinary_entry_v2(b"golden-a", 90.0, vec![-0.0, 0.5, -2.25]),
            ordinary_entry_v2(b"golden-b", 45.0, vec![1.0, -0.25, 0.125, -0.0625]),
        ],
    );
    let evidence = prove_exact_v2(&schedule, CycleScheduleLimitsV1::default());
    assert_eq!(
        fingerprint_hex_v2(evidence.binding_fingerprint_v2()),
        "7df0e1e5a49a0363cc290dc0d26bbeaae72956b3190d67088b0c13745dfbe46f"
    );
}
