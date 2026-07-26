use ori_domain::{EdgeKind, Point2};

use super::*;
use crate::{
    ConstraintSolveErrorV1, ConstraintSolveLimitsV1, solve_geometric_constraints_v1,
    verify_geometric_constraint_solution_v1,
};

struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 6],
    edges: [EdgeId; 3],
}

impl Fixture {
    fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 2.0),
            Point2::new(1.0, 2.0),
            Point2::new(0.0, 4.0),
            Point2::new(1.0, 4.0),
        ];
        let vertex_records = vertices
            .into_iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = std::array::from_fn(|_| EdgeId::new());
        let edge_records = edges
            .into_iter()
            .zip([(0, 1), (2, 3), (4, 5)])
            .map(|(id, (start, end))| Edge {
                id,
                start: vertices[start],
                end: vertices[end],
                kind: EdgeKind::Auxiliary,
            })
            .collect();
        Self {
            pattern: CreasePattern {
                vertices: vertex_records,
                edges: edge_records,
            },
            vertices,
            edges,
        }
    }
}

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().collect(),
    }
}

fn prepare<'a>(
    fixture: &'a Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("opposing-ratio fixture must prepare")
}

fn sorted_ids(ids: impl IntoIterator<Item = ConstraintId>) -> Vec<ConstraintId> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn core_records(
    fixture: &Fixture,
    fixed_on_first: bool,
    fixed_length: f64,
    forward_ratio: f64,
    reverse_ratio: f64,
) -> Vec<GeometricConstraintRecordV1> {
    let [first, second, _] = fixture.edges;
    vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: if fixed_on_first { first } else { second },
            length_mm: fixed_length,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: second,
            ratio: forward_ratio,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: second,
            denominator_edge: first,
            ratio: reverse_ratio,
        }),
    ]
}

fn closure_residual(fixed_on_first: bool, fixed: f64, forward: f64, reverse: f64) -> f64 {
    if fixed_on_first {
        let second = length_ratio_scaled_denominator_binary64_v1(reverse, fixed);
        length_ratio_residual_binary64_v1(fixed, forward, second)
    } else {
        let first = length_ratio_scaled_denominator_binary64_v1(forward, fixed);
        length_ratio_residual_binary64_v1(fixed, reverse, first)
    }
}

fn assert_single_target(
    outcome: &ConstraintPreflightV1,
    fixture: &Fixture,
    expected_ids: &[ConstraintId],
) {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        panic!("expected one opposing-ratio closure conflict: {outcome:?}");
    };
    assert_eq!(conflicts.len(), 1);
    let mut expected_edges = [fixture.edges[0], fixture.edges[1]];
    expected_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength {
            first_edge,
            second_edge,
        } if [*first_edge, *second_edge] == expected_edges
    ));
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);
}

fn contains_target(outcome: &ConstraintPreflightV1) -> bool {
    matches!(
        outcome,
        ConstraintPreflightV1::DirectConflict { conflicts }
            if conflicts.iter().any(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    NonReciprocalLengthRatiosWithFixedLength { .. }
            ))
    )
}

#[test]
fn both_fixed_sides_and_ratio_orientations_are_canonical_and_irredundant() {
    let fixture = Fixture::new();
    for fixed_on_first in [false, true] {
        for (forward, reverse) in [(2.0, 0.25), (0.25, 2.0)] {
            let records = core_records(&fixture, fixed_on_first, 10.0, forward, reverse);
            let expected_ids = sorted_ids(records.iter().map(|item| item.id));
            let expected = prepare(&fixture, records.clone()).preflight();
            assert_single_target(&expected, &fixture, &expected_ids);

            let mut reordered = records.clone();
            reordered.reverse();
            assert_eq!(prepare(&fixture, reordered).preflight(), expected);
            for removed in 0..records.len() {
                let subset = records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != removed)
                    .map(|(_, item)| item.clone());
                assert!(!matches!(
                    prepare(&fixture, subset).preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ));
            }
        }
    }
}

#[test]
fn only_the_two_production_residual_steps_decide_closure() {
    let minimum = f64::from_bits(1);
    let one_up = 1.0_f64.next_up();
    let one_down = 1.0_f64.next_down();
    let cases = [
        (true, 10.0, 2.0, 0.5, false),
        (false, 10.0, 2.0, 0.5, false),
        (true, minimum, one_up, one_down, false),
        (false, minimum, one_up, one_down, false),
        (true, minimum, 2.0, 0.5, true),
        (false, minimum, 0.5, 2.0, true),
        (true, minimum, minimum, minimum, true),
        (false, minimum, minimum, minimum, true),
        (true, f64::MAX, 0.5, 2.0, true),
        (false, f64::MAX, 2.0, 0.5, true),
    ];
    for (fixed_on_first, fixed, forward, reverse, proven) in cases {
        let residual = closure_residual(fixed_on_first, fixed, forward, reverse);
        assert_eq!(
            residual != 0.0,
            proven,
            "{fixed_on_first}, {fixed:?}, {forward:?}, {reverse:?}"
        );
        let fixture = Fixture::new();
        let records = core_records(&fixture, fixed_on_first, fixed, forward, reverse);
        let expected_ids = sorted_ids(records.iter().map(|item| item.id));
        let outcome = prepare(&fixture, records).preflight();
        if proven {
            assert_single_target(&outcome, &fixture, &expected_ids);
        } else {
            assert!(matches!(
                outcome,
                ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    ..
                }
            ));
            assert!(!contains_target(&outcome));
        }
    }
}

#[test]
fn duplicate_groups_choose_the_canonical_three_ids() {
    let fixture = Fixture::new();
    let [first, second, _] = fixture.edges;
    let fixed = std::array::from_fn::<_, 2, _>(|_| {
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 10.0,
        })
    });
    let forward = std::array::from_fn::<_, 2, _>(|_| {
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: second,
            ratio: 2.0,
        })
    });
    let reverse = std::array::from_fn::<_, 2, _>(|_| {
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: second,
            denominator_edge: first,
            ratio: 0.25,
        })
    });
    let expected_ids = sorted_ids([
        fixed
            .iter()
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap(),
        forward
            .iter()
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap(),
        reverse
            .iter()
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap(),
    ]);
    let mut records = fixed
        .into_iter()
        .chain(forward)
        .chain(reverse)
        .collect::<Vec<_>>();
    let expected = prepare(&fixture, records.clone()).preflight();
    assert_single_target(&expected, &fixture, &expected_ids);
    records.reverse();
    assert_eq!(prepare(&fixture, records).preflight(), expected);

    let mut both_fixed = core_records(&fixture, true, 10.0, 2.0, 0.25);
    let second_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: second,
        length_mm: 10.0,
    });
    let canonical_fixed = [both_fixed[0].id, second_fixed.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let expected_ids = sorted_ids([canonical_fixed, both_fixed[1].id, both_fixed[2].id]);
    both_fixed.push(second_fixed);
    assert!(matches!(
        prepare(&fixture, both_fixed).preflight(),
        ConstraintPreflightV1::DirectConflict { conflicts }
            if conflicts.iter().any(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    NonReciprocalLengthRatiosWithFixedLength { .. }
            ) && conflict.constraint_ids() == expected_ids)
    ));
}

#[test]
fn exact_pair_and_consistent_groups_are_required() {
    let fixture = Fixture::new();
    let [first, second, third] = fixture.edges;
    let mismatched_pair = prepare(
        &fixture,
        [
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 10.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: third,
                denominator_edge: first,
                ratio: 0.25,
            }),
        ],
    )
    .preflight();
    assert!(!contains_target(&mismatched_pair));

    let unrelated_fixed = prepare(
        &fixture,
        [
            record(GeometricConstraintKindV1::FixedLength {
                edge: third,
                length_mm: 10.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: second,
                denominator_edge: first,
                ratio: 0.25,
            }),
        ],
    )
    .preflight();
    assert!(!contains_target(&unrelated_fixed));

    let inconsistent_forward = prepare(
        &fixture,
        core_records(&fixture, true, 10.0, 2.0, 0.25)
            .into_iter()
            .chain([record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 3.0,
            })]),
    )
    .preflight();
    assert!(!contains_target(&inconsistent_forward));

    let inconsistent_fixed = prepare(
        &fixture,
        core_records(&fixture, true, 10.0, 2.0, 0.25)
            .into_iter()
            .chain([record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 11.0,
            })]),
    )
    .preflight();
    assert!(!contains_target(&inconsistent_fixed));

    for extra in [
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge: second,
            denominator_edge: first,
            ratio: 0.2,
        },
        GeometricConstraintKindV1::FixedLength {
            edge: second,
            length_mm: 11.0,
        },
    ] {
        let outcome = prepare(
            &fixture,
            core_records(&fixture, false, 10.0, 2.0, 0.25)
                .into_iter()
                .chain([record(extra)]),
        )
        .preflight();
        assert!(!contains_target(&outcome));
    }
}

#[test]
fn bounded_oracle_handles_four_eight_sixteen_and_seventeen() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let mut records = core_records(&fixture, false, 10.0, 2.0, 0.25);
        let expected_ids = sorted_ids(records.iter().map(|item| item.id));
        records.extend((3..count).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[2],
            })
        }));
        let prepared = prepare(&fixture, records);
        assert_single_target(&prepared.preflight(), &fixture, &expected_ids);
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

struct StopOnCheckpoint {
    calls: usize,
    control: GeometricConstraintPreflightObserverControlV1,
}

impl GeometricConstraintPreflightObserverV1 for StopOnCheckpoint {
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
fn work_cancel_deadline_and_numerical_tolerance_fail_closed() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, true, 10.0, 2.0, 0.25);
    let raw = document(records.clone());
    let limited = prepare_geometric_constraints_v1(
        &fixture.pattern,
        &raw,
        GeometricConstraintLimitsV1 {
            max_preflight_checks: 2,
            ..GeometricConstraintLimitsV1::default()
        },
    )
    .unwrap();
    assert!(matches!(
        limited.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ..
        }
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
        let mut observer = StopOnCheckpoint { calls: 0, control };
        assert!(matches!(
            prepared.preflight_with_observer(&mut observer),
            ConstraintPreflightV1::Unknown { reason: actual, .. } if actual == reason
        ));
        assert_eq!(observer.calls, 2);
    }

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
