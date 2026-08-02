use super::*;

fn policy_schedule_v2() -> CanonicalCycleScheduleV1 {
    ordinary_schedule_v2(
        [3.0, 9.0],
        vec![
            ordinary_entry_v2(b"policy-a", 90.0, vec![0.0, 5.0]),
            ordinary_entry_v2(b"policy-b", 45.0, vec![1.0, -2.0, 0.5]),
        ],
    )
}

#[test]
fn closed_dyadic_boundary_caps_are_exact_at_equality_and_reject_both_adjacent_values() {
    let schedule = policy_schedule_v2();
    let limits = CycleScheduleLimitsV1::default();
    let bound = schedule
        .checked_closed_dyadic_boundary_resource_bound_v2(limits)
        .unwrap();
    let exact_work = bound.logical_work_required_v2();
    let exact_workspace = bound.workspace_peak_bytes_upper_bound_v2();
    let evidence = schedule
        .prove_closed_dyadic_boundary_evidence_v2(limits, exact_work, exact_workspace)
        .unwrap();
    assert_eq!(evidence.logical_work_v2(), exact_work);
    assert_eq!(
        evidence.workspace_peak_bytes_upper_bound_v2(),
        exact_workspace
    );
    assert!(matches!(
        schedule.prove_closed_dyadic_boundary_evidence_v2(limits, exact_work - 1, exact_workspace,),
        Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)
    ));
    assert!(matches!(
        schedule.prove_closed_dyadic_boundary_evidence_v2(limits, exact_work, exact_workspace - 1,),
        Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)
    ));
    assert!(matches!(
        schedule.prove_closed_dyadic_boundary_evidence_v2(limits, exact_work + 1, exact_workspace,),
        Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)
    ));
    assert!(matches!(
        schedule.prove_closed_dyadic_boundary_evidence_v2(limits, exact_work, exact_workspace + 1,),
        Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)
    ));
}

#[test]
fn closed_dyadic_boundary_rejects_zero_and_maximum_resource_caps() {
    let schedule = policy_schedule_v2();
    let limits = CycleScheduleLimitsV1::default();
    let bound = schedule
        .checked_closed_dyadic_boundary_resource_bound_v2(limits)
        .unwrap();
    for (work, workspace) in [
        (0, bound.workspace_peak_bytes_upper_bound_v2()),
        (usize::MAX, bound.workspace_peak_bytes_upper_bound_v2()),
        (bound.logical_work_required_v2(), 0),
        (bound.logical_work_required_v2(), usize::MAX),
    ] {
        assert!(matches!(
            schedule.prove_closed_dyadic_boundary_evidence_v2(limits, work, workspace,),
            Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)
        ));
    }
}

#[test]
fn closed_dyadic_boundary_resource_bound_reports_borrowed_schedule_separately() {
    let schedule = policy_schedule_v2();
    let bound = schedule
        .checked_closed_dyadic_boundary_resource_bound_v2(CycleScheduleLimitsV1::default())
        .unwrap();
    assert_eq!(bound.hinge_count_v2(), 2);
    assert_eq!(
        bound.schedule_deep_retained_bytes_v2(),
        schedule.checked_deep_retained_bytes_v1().unwrap()
    );
    assert!(bound.logical_work_required_v2() > 0);
    assert!(bound.workspace_peak_bytes_upper_bound_v2() > 0);
}

#[test]
fn closed_dyadic_boundary_maps_entry_and_midstream_cooperative_stops() {
    let schedule = policy_schedule_v2();
    let limits = CycleScheduleLimitsV1::default();
    for (stop, expected) in [
        (
            CycleScheduleClosedDyadicBoundaryStopV2::Cancelled,
            CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled,
        ),
        (
            CycleScheduleClosedDyadicBoundaryStopV2::DeadlineExceeded,
            CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            schedule
                .checked_closed_dyadic_boundary_resource_bound_with_checkpoint_v2(limits, || Err(
                    stop
                ),),
            Err(expected)
        );
        let bound = schedule
            .checked_closed_dyadic_boundary_resource_bound_v2(limits)
            .unwrap();
        let mut polls = 0usize;
        let result = schedule.prove_closed_dyadic_boundary_evidence_with_checkpoint_v2(
            limits,
            bound.logical_work_required_v2(),
            bound.workspace_peak_bytes_upper_bound_v2(),
            || {
                polls += 1;
                if polls == 12 { Err(stop) } else { Ok(()) }
            },
        );
        assert!(polls >= 12);
        assert!(matches!(
            (result, expected),
            (
                Err(CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled),
                CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled,
            ) | (
                Err(CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded),
                CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded,
            )
        ));
    }
}

#[test]
fn closed_dyadic_boundary_checked_work_formula_fails_closed_on_overflow() {
    let shape = resources::BoundaryResourceShapeV2 {
        representation: BoundaryRepresentationV2::HalfAngle,
        hinge_count: usize::MAX,
        ordinary_coefficient_count: 0,
        half_angle_power_coefficient_count: usize::MAX,
        retained_scan_visits: usize::MAX,
    };
    assert_eq!(
        resources::checked_logical_work_required_v2(shape, CycleScheduleLimitsV1::default()),
        None
    );
    assert_eq!(
        binding::checked_endpoint_binding_work_v2(BoundaryRepresentationV2::HalfAngle, usize::MAX,),
        None
    );
    assert_eq!(
        resources::checked_boundary_workspace_peak_v2(usize::MAX),
        None
    );
}

#[test]
fn closed_dyadic_boundary_debug_redacts_all_binding_material() {
    let evidence = prove_exact_v2(&policy_schedule_v2(), CycleScheduleLimitsV1::default());
    let debug = format!("{evidence:?}");
    assert!(debug.contains("canonical_boundary_count"));
    assert!(!debug.contains(&fingerprint_hex_v2(evidence.binding_fingerprint_v2())));
    assert!(!debug.contains("lower_boundary_binding"));
    assert!(!debug.contains("upper_boundary_binding"));
}
