use ori_domain::Point2;

use super::ratio_cycle_tests::{
    Fixture, assert_single_target, core_records, document, prepare, record, sorted_ids,
};
use super::{
    BoundedDirectMusV1, ConstraintPreflightV1, GeometricConstraintKindV1,
    GeometricConstraintLimitsV1, GeometricConstraintPreflightObserverControlV1,
    GeometricConstraintPreflightObserverV1, GeometricConstraintUnknownReasonV1,
    find_bounded_direct_mus_v1, prepare_geometric_constraints_v1,
};
use crate::{
    ConstraintSolveErrorV1, ConstraintSolveLimitsV1, solve_geometric_constraints_v1,
    verify_geometric_constraint_solution_v1,
};

#[test]
fn bounded_oracle_handles_four_eight_sixteen_and_seventeen_records() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let mut records = core_records(&fixture, [0, 2, 1], 2, 1.0, [2.0, 3.0, 0.25]);
        let expected_ids = sorted_ids(records.iter().map(|item| item.id));
        records.extend((4..count).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[3],
            })
        }));
        let prepared = prepare(&fixture, records);
        assert_single_target(&prepared.preflight(), &fixture, [0, 2, 1], 2, &expected_ids);
        if count == 17 {
            assert_eq!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::Unknown { oracle_calls: 0 }
            );
        } else {
            assert!(matches!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::ProvenUnsatisfiable {
                    ref constraint_ids,
                    ..
                } if constraint_ids == &expected_ids
            ));
        }
    }
}

struct StopOnComplete {
    calls: usize,
    control: GeometricConstraintPreflightObserverControlV1,
}

impl GeometricConstraintPreflightObserverV1 for StopOnComplete {
    fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
        self.calls += 1;
        if self.calls == 2 {
            self.control
        } else {
            GeometricConstraintPreflightObserverControlV1::Continue
        }
    }
}

#[test]
fn work_cancel_and_deadline_override_a_completed_cycle_join() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, [0, 1, 2], 0, 1.0, [2.0, 3.0, 0.25]);
    let raw = document(records.clone());
    let limited = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &raw,
        GeometricConstraintLimitsV1 {
            max_preflight_checks: records.len() - 1,
            ..GeometricConstraintLimitsV1::default()
        },
    )
    .unwrap();
    assert!(matches!(
        limited.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids == &sorted_ids(records.iter().map(|item| item.id))
    ));

    for (control, reason) in [
        (
            GeometricConstraintPreflightObserverControlV1::Cancelled,
            GeometricConstraintUnknownReasonV1::Cancelled,
        ),
        (
            GeometricConstraintPreflightObserverControlV1::DeadlineReached,
            GeometricConstraintUnknownReasonV1::DeadlineReached,
        ),
    ] {
        let prepared = prepare(&fixture, records.clone());
        let mut observer = StopOnComplete { calls: 0, control };
        assert!(matches!(
            prepared.preflight_with_observer(&mut observer),
            ConstraintPreflightV1::Unknown { reason: actual, .. } if actual == reason
        ));
        assert_eq!(observer.calls, 2);
    }
}

#[test]
fn solver_and_verifier_cannot_bypass_cycle_preflight_with_maximum_tolerance() {
    let fixture = Fixture::new();
    let raw = document(core_records(&fixture, [0, 1, 2], 0, 1.0, [2.0, 3.0, 0.25]));
    assert_eq!(
        solve_geometric_constraints_v1(
            &fixture.pattern,
            &raw,
            fixture.vertices[0],
            Point2::new(0.0, 0.0),
            ConstraintSolveLimitsV1 {
                residual_tolerance: f64::MAX,
                ..ConstraintSolveLimitsV1::default()
            },
        ),
        Err(ConstraintSolveErrorV1::NonConvergent)
    );
    assert_eq!(
        verify_geometric_constraint_solution_v1(&fixture.pattern, &raw, f64::MAX),
        Err(ConstraintSolveErrorV1::NonConvergent)
    );
}
