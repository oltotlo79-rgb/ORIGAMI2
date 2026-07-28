use ori_domain::Point2;

use super::inverse_cardinal_rotation_tests::{
    Fixture, assert_target, core_records, document, record, sorted_ids,
};
use super::*;
use crate::{
    ConstraintSolveErrorV1, ConstraintSolveLimitsV1, solve_geometric_constraints_v1,
    verify_geometric_constraint_solution_v1,
};

#[test]
fn direct_proof_and_bounded_oracle_cover_four_eight_sixteen_and_seventeen_records() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let mut records = core_records(&fixture, 90.0, 180.0, 0, f64::from_bits(1)).to_vec();
        let expected_ids = sorted_ids(records.iter().map(|item| item.id));
        records.extend((3..count).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[2],
            })
        }));
        let prepared = prepare_geometric_constraints_v1(
            &fixture.pattern,
            &document(records),
            GeometricConstraintLimitsV1::default(),
        )
        .expect("oracle-boundary fixture must prepare");
        assert_target(&prepared.preflight(), &fixture, &expected_ids, 0);
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
                    oracle_calls,
                } if constraint_ids == &expected_ids
                    && oracle_calls <= MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1
            ));
        }
    }
}

#[test]
fn exact_resource_and_work_limits_are_admitted_and_one_short_is_fail_closed() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 90.0, 180.0, 0, 1.0);
    let raw = document(records.clone());
    let ids = sorted_ids(records.iter().map(|item| item.id));
    let exact = GeometricConstraintLimitsV1 {
        max_vertices: fixture.pattern.vertices.len(),
        max_edges: fixture.pattern.edges.len(),
        max_constraints: records.len(),
        max_references: 7,
        max_preflight_checks: records.len(),
    };
    let prepared = prepare_geometric_constraints_v1(&fixture.pattern, &raw, exact)
        .expect("exact storage and work limits must prepare");
    assert_target(&prepared.preflight(), &fixture, &ids, 0);

    let one_short_work = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &raw,
        GeometricConstraintLimitsV1 {
            max_preflight_checks: records.len() - 1,
            ..exact
        },
    )
    .expect("preflight work limits do not invalidate persistence");
    assert_eq!(
        one_short_work.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            unchecked_constraint_ids: ids,
        }
    );
    for limits in [
        GeometricConstraintLimitsV1 {
            max_vertices: fixture.pattern.vertices.len() - 1,
            ..exact
        },
        GeometricConstraintLimitsV1 {
            max_edges: fixture.pattern.edges.len() - 1,
            ..exact
        },
        GeometricConstraintLimitsV1 {
            max_constraints: records.len() - 1,
            ..exact
        },
        GeometricConstraintLimitsV1 {
            max_references: 6,
            ..exact
        },
    ] {
        assert!(matches!(
            prepare_geometric_constraints_v1(&fixture.pattern, &raw, limits),
            Err(GeometricConstraintErrorV1::ResourceLimitExceeded { .. })
        ));
    }
}

struct StopOnSecondCheckpoint {
    calls: usize,
    control: GeometricConstraintPreflightObserverControlV1,
}

impl GeometricConstraintPreflightObserverV1 for StopOnSecondCheckpoint {
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
fn cancellation_or_deadline_after_finding_the_candidate_returns_unknown() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 90.0, 180.0, 0, 1.0);
    let expected_ids = sorted_ids(records.iter().map(|item| item.id));
    let prepared = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("observer fixture must prepare");
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
        let mut observer = StopOnSecondCheckpoint { calls: 0, control };
        assert_eq!(
            prepared.preflight_with_observer(&mut observer),
            ConstraintPreflightV1::Unknown {
                reason,
                unchecked_constraint_ids: expected_ids.clone(),
            }
        );
        assert_eq!(observer.calls, 2);
    }
}

#[test]
fn solver_and_verifier_cannot_bypass_the_promoted_preflight() {
    let fixture = Fixture::new();
    let raw = document(core_records(&fixture, 90.0, 180.0, 0, 1.0));
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
