use std::collections::BTreeMap;

use ori_domain::Point2;

use super::bounded_zero_closure::{
    Checkpoint, NoopObserver, Observer, ObserverControl, Phase, UnknownReason,
};
use super::directed_ratio_closure::{self, Limits, Outcome};
use super::general_ratio_graph_tests::{
    Fixture, assert_target, document, prepare, record, remote_two_cycle_records, sorted_ids,
};
use super::*;
use crate::{
    ConstraintSolveErrorV1, ConstraintSolveLimitsV1, solve_geometric_constraints_v1,
    verify_geometric_constraint_solution_v1,
};

#[test]
fn bounded_oracle_handles_four_eight_sixteen_and_seventeen_records() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let mut records = remote_two_cycle_records(&fixture);
        let expected_ids = sorted_ids(records.iter().map(|item| item.id));
        records.extend((4..count).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[11],
            })
        }));
        let prepared = prepare(&fixture, records);
        assert_target(&prepared.preflight(), fixture.edges[0], &expected_ids);
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

type RatioInput = (
    BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>>,
    BTreeMap<CanonicalId, ScalarGroupSummary>,
    BTreeMap<CanonicalId, EdgeId>,
);

fn ratio_input(ratio_count: usize) -> RatioInput {
    assert!(ratio_count >= 3);
    let edges = (0..ratio_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
    let nodes = edges
        .iter()
        .map(EdgeId::canonical_bytes)
        .collect::<Vec<_>>();
    let edge_ids = nodes
        .iter()
        .copied()
        .zip(edges.iter().copied())
        .collect::<BTreeMap<_, _>>();
    let mut ratios = BTreeMap::new();
    for index in 0..ratio_count - 2 {
        ratios.insert(
            (nodes[index + 1], nodes[index]),
            vec![ScalarAssignment {
                id: ConstraintId::new(),
                value: 1.0,
            }],
        );
    }
    ratios.insert(
        (nodes[ratio_count - 1], nodes[ratio_count - 2]),
        vec![ScalarAssignment {
            id: ConstraintId::new(),
            value: 2.0,
        }],
    );
    ratios.insert(
        (nodes[ratio_count - 2], nodes[ratio_count - 1]),
        vec![ScalarAssignment {
            id: ConstraintId::new(),
            value: 0.25,
        }],
    );
    let fixed_lengths = BTreeMap::from([(
        nodes[0],
        ScalarGroupSummary::new(ScalarAssignment {
            id: ConstraintId::new(),
            value: 1.0,
        }),
    )]);
    (ratios, fixed_lengths, edge_ids)
}

fn reverse_domain_input() -> RatioInput {
    let edges = (0..6).map(|_| EdgeId::new()).collect::<Vec<_>>();
    let nodes = edges
        .iter()
        .map(EdgeId::canonical_bytes)
        .collect::<Vec<_>>();
    let edge_ids = nodes
        .iter()
        .copied()
        .zip(edges.iter().copied())
        .collect::<BTreeMap<_, _>>();
    let mut ratios = BTreeMap::new();
    for (numerator, denominator, ratio) in [
        (4, 0, 11.0),
        (0, 1, 2.0),
        (1, 2, 3.0),
        (2, 3, 5.0),
        (3, 0, 0.1),
    ] {
        ratios.insert(
            (nodes[numerator], nodes[denominator]),
            vec![ScalarAssignment {
                id: ConstraintId::new(),
                value: ratio,
            }],
        );
    }
    let fixed_lengths = BTreeMap::from([(
        nodes[4],
        ScalarGroupSummary::new(ScalarAssignment {
            id: ConstraintId::new(),
            value: 7.0,
        }),
    )]);
    (ratios, fixed_lengths, edge_ids)
}

fn cross_root_input(ratio_count: usize) -> RatioInput {
    assert!(ratio_count >= 2);
    let edges = (0..=ratio_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
    let nodes = edges
        .iter()
        .map(EdgeId::canonical_bytes)
        .collect::<Vec<_>>();
    let edge_ids = nodes
        .iter()
        .copied()
        .zip(edges.iter().copied())
        .collect::<BTreeMap<_, _>>();
    let ratios = (0..ratio_count)
        .map(|index| {
            (
                (nodes[index + 1], nodes[index]),
                vec![ScalarAssignment {
                    id: ConstraintId::new(),
                    value: 1.0,
                }],
            )
        })
        .collect::<BTreeMap<_, _>>();
    let fixed_lengths = BTreeMap::from([
        (
            nodes[0],
            ScalarGroupSummary::new(ScalarAssignment {
                id: ConstraintId::new(),
                value: 1.0,
            }),
        ),
        (
            nodes[ratio_count],
            ScalarGroupSummary::new(ScalarAssignment {
                id: ConstraintId::new(),
                value: 2.0,
            }),
        ),
    ]);
    (ratios, fixed_lengths, edge_ids)
}

fn direct_with(
    input: &RatioInput,
    limits: Limits,
    observer: &mut impl Observer,
) -> (Outcome, directed_ratio_closure::Stats) {
    directed_ratio_closure::conflict_with_limits_and_observer(
        &input.0, &input.1, &input.2, limits, observer,
    )
}

#[test]
fn witness_and_exact_work_storage_boundaries_are_fail_closed() {
    let at_cap = ratio_input(255);
    let (at_cap_outcome, _) = direct_with(&at_cap, Limits::default(), &mut NoopObserver);
    assert!(matches!(
        at_cap_outcome,
        Outcome::Proven(ref conflict)
            if conflict.constraint_ids().len() == MAX_DIRECT_CONFLICT_CAUSE_IDS_V1
                && matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::
                        InconsistentLengthRatioGraphWithFixedLength {
                            ratio_constraint_count: 255,
                            ..
                        }
                )
    ));
    let over_cap = ratio_input(256);
    assert!(matches!(
        direct_with(&over_cap, Limits::default(), &mut NoopObserver).0,
        Outcome::NoProof
    ));

    let input = ratio_input(4);
    let (expected, stats) = direct_with(&input, Limits::default(), &mut NoopObserver);
    assert!(matches!(expected, Outcome::Proven(_)));
    let exact = Limits {
        max_work: stats.completed_work,
        max_storage_units: stats.peak_storage_units,
    };
    assert_eq!(direct_with(&input, exact, &mut NoopObserver).0, expected);
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::WorkLimitExceeded,
            ..
        }
    ));
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_storage_units: exact.max_storage_units - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::StorageLimitExceeded,
            ..
        }
    ));
}

#[test]
fn cross_root_witness_and_exact_resource_boundaries_are_fail_closed() {
    let at_cap = cross_root_input(MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2);
    let (at_cap_outcome, _) = direct_with(&at_cap, Limits::default(), &mut NoopObserver);
    assert!(matches!(
        at_cap_outcome,
        Outcome::Proven(ref conflict)
            if conflict.constraint_ids().len() == MAX_DIRECT_CONFLICT_CAUSE_IDS_V1
                && matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::
                        InconsistentLengthRatioGraphBetweenFixedLengths {
                            ratio_constraint_count: 254,
                            ..
                        }
                )
    ));
    let over_cap = cross_root_input(MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 1);
    assert!(matches!(
        direct_with(&over_cap, Limits::default(), &mut NoopObserver).0,
        Outcome::NoProof
    ));

    let input = cross_root_input(2);
    let (expected, stats) = direct_with(&input, Limits::default(), &mut NoopObserver);
    assert!(matches!(
        expected,
        Outcome::Proven(ref conflict)
            if matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    InconsistentLengthRatioGraphBetweenFixedLengths {
                        ratio_constraint_count: 2,
                        ..
                    }
            )
    ));
    let exact = Limits {
        max_work: stats.completed_work,
        max_storage_units: stats.peak_storage_units,
    };
    assert_eq!(direct_with(&input, exact, &mut NoopObserver).0, expected);
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::WorkLimitExceeded,
            ..
        }
    ));
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_storage_units: exact.max_storage_units - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::StorageLimitExceeded,
            ..
        }
    ));

    struct StopAt {
        phase: Phase,
        control: ObserverControl,
    }
    impl Observer for StopAt {
        fn checkpoint(&mut self, checkpoint: Checkpoint) -> ObserverControl {
            if checkpoint.phase == self.phase {
                self.control
            } else {
                ObserverControl::Continue
            }
        }
    }
    for (control, reason) in [
        (ObserverControl::Cancelled, UnknownReason::Cancelled),
        (
            ObserverControl::DeadlineReached,
            UnknownReason::DeadlineReached,
        ),
    ] {
        for phase in [Phase::GraphBuild, Phase::ProofSearch, Phase::Complete] {
            let mut observer = StopAt { phase, control };
            assert!(matches!(
                direct_with(&input, Limits::default(), &mut observer).0,
                Outcome::Unknown { reason: actual, .. } if actual == reason
            ));
        }
    }
}

#[test]
fn reverse_domain_exact_work_storage_and_every_stop_boundary_are_fail_closed() {
    let input = reverse_domain_input();
    let (expected, stats) = direct_with(&input, Limits::default(), &mut NoopObserver);
    assert!(matches!(expected, Outcome::Proven(_)));
    let exact = Limits {
        max_work: stats.completed_work,
        max_storage_units: stats.peak_storage_units,
    };
    assert_eq!(direct_with(&input, exact, &mut NoopObserver).0, expected);
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::WorkLimitExceeded,
            ..
        }
    ));
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_storage_units: exact.max_storage_units - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::StorageLimitExceeded,
            ..
        }
    ));

    struct StopAt {
        phase: Phase,
        minimum_work: u64,
        control: ObserverControl,
    }
    impl Observer for StopAt {
        fn checkpoint(&mut self, checkpoint: Checkpoint) -> ObserverControl {
            if checkpoint.phase == self.phase && checkpoint.completed_work >= self.minimum_work {
                self.control
            } else {
                ObserverControl::Continue
            }
        }
    }

    for (control, reason) in [
        (ObserverControl::Cancelled, UnknownReason::Cancelled),
        (
            ObserverControl::DeadlineReached,
            UnknownReason::DeadlineReached,
        ),
    ] {
        for (phase, minimum_work) in [
            (Phase::GraphBuild, 0),
            (Phase::ProofSearch, 128),
            (Phase::Complete, 0),
        ] {
            let mut observer = StopAt {
                phase,
                minimum_work,
                control,
            };
            assert!(matches!(
                direct_with(&input, Limits::default(), &mut observer).0,
                Outcome::Unknown { reason: actual, .. } if actual == reason
            ));
        }
    }
}

#[test]
fn duplicate_ratio_group_scan_has_exact_work_and_observer_accounting() {
    let mut input = ratio_input(3);
    let key = *input.0.keys().next().unwrap();
    input.0.insert(
        key,
        (0..128)
            .map(|_| ScalarAssignment {
                id: ConstraintId::new(),
                value: 1.0,
            })
            .collect(),
    );
    let (expected, stats) = direct_with(&input, Limits::default(), &mut NoopObserver);
    assert!(matches!(expected, Outcome::Proven(_)));
    let exact = Limits {
        max_work: stats.completed_work,
        max_storage_units: Limits::default().max_storage_units,
    };
    assert_eq!(direct_with(&input, exact, &mut NoopObserver).0, expected);
    assert!(matches!(
        direct_with(
            &input,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &mut NoopObserver,
        )
        .0,
        Outcome::Unknown {
            reason: UnknownReason::WorkLimitExceeded,
            ..
        }
    ));

    struct StopAfterDuplicateScan(usize);
    impl Observer for StopAfterDuplicateScan {
        fn checkpoint(&mut self, checkpoint: Checkpoint) -> ObserverControl {
            if checkpoint.phase == Phase::GraphBuild {
                self.0 += 1;
                if self.0 == 2 {
                    return ObserverControl::Cancelled;
                }
            }
            ObserverControl::Continue
        }
    }
    let outcome = direct_with(&input, Limits::default(), &mut StopAfterDuplicateScan(0)).0;
    assert!(matches!(
        outcome,
        Outcome::Unknown {
            reason: UnknownReason::Cancelled,
            stats: directed_ratio_closure::Stats {
                completed_work: 129,
                ..
            },
        }
    ));
}

struct StopAtProofRoot {
    proof_roots: usize,
    control: ObserverControl,
}

impl Observer for StopAtProofRoot {
    fn checkpoint(&mut self, checkpoint: Checkpoint) -> ObserverControl {
        if checkpoint.phase == Phase::ProofSearch {
            self.proof_roots += 1;
            if self.proof_roots == 2 {
                return self.control;
            }
        }
        ObserverControl::Continue
    }
}

#[test]
fn a_later_root_cancel_or_deadline_overrides_an_already_found_candidate() {
    let mut input = ratio_input(3);
    let second_root = EdgeId::new();
    let second_key = second_root.canonical_bytes();
    input.2.insert(second_key, second_root);
    input.1.insert(
        second_key,
        ScalarGroupSummary::new(ScalarAssignment {
            id: ConstraintId::new(),
            value: 1.0,
        }),
    );
    let cycle_first = *input
        .0
        .keys()
        .find_map(|(numerator, denominator)| input.1.contains_key(denominator).then_some(numerator))
        .unwrap();
    input.0.insert(
        (cycle_first, second_key),
        vec![ScalarAssignment {
            id: ConstraintId::new(),
            value: 1.0,
        }],
    );
    for (control, reason) in [
        (ObserverControl::Cancelled, UnknownReason::Cancelled),
        (
            ObserverControl::DeadlineReached,
            UnknownReason::DeadlineReached,
        ),
    ] {
        let mut observer = StopAtProofRoot {
            proof_roots: 0,
            control,
        };
        assert!(matches!(
            direct_with(&input, Limits::default(), &mut observer).0,
            Outcome::Unknown { reason: actual, .. } if actual == reason
        ));
        assert_eq!(observer.proof_roots, 2);
    }
}

#[test]
fn preflight_maps_internal_work_storage_cancel_and_deadline_to_unknown() {
    let fixture = Fixture::new();
    let records = remote_two_cycle_records(&fixture);
    for (limits, reason) in [
        (
            Limits {
                max_work: 0,
                ..Limits::default()
            },
            GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
        ),
        (
            Limits {
                max_storage_units: 0,
                ..Limits::default()
            },
            GeometricConstraintUnknownReasonV1::StorageLimitExceeded,
        ),
    ] {
        assert_eq!(replace_directed_ratio_test_limits_v1(Some(limits)), None);
        let outcome = prepare(&fixture, records.clone()).preflight();
        assert_eq!(replace_directed_ratio_test_limits_v1(None), Some(limits));
        assert!(matches!(
            outcome,
            ConstraintPreflightV1::Unknown { reason: actual, .. } if actual == reason
        ));
    }

    struct StopOnSecondCall {
        calls: usize,
        control: GeometricConstraintPreflightObserverControlV1,
    }
    impl GeometricConstraintPreflightObserverV1 for StopOnSecondCall {
        fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
            self.calls += 1;
            if self.calls == 2 {
                self.control
            } else {
                GeometricConstraintPreflightObserverControlV1::Continue
            }
        }
    }
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
        let mut observer = StopOnSecondCall { calls: 0, control };
        assert!(matches!(
            prepared.preflight_with_observer(&mut observer),
            ConstraintPreflightV1::Unknown { reason: actual, .. } if actual == reason
        ));
        assert_eq!(observer.calls, 2);
    }
}

#[test]
fn solver_and_verifier_cannot_bypass_general_ratio_preflight() {
    let fixture = Fixture::new();
    let raw = document(remote_two_cycle_records(&fixture));
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
