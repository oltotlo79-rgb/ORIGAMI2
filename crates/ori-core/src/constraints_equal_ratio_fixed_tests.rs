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
    .expect("equal/ratio/fixed fixture must prepare")
}

fn sorted_ids(ids: impl IntoIterator<Item = ConstraintId>) -> Vec<ConstraintId> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn minimum_id(records: &[GeometricConstraintRecordV1]) -> ConstraintId {
    records
        .iter()
        .map(|item| item.id)
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap()
}

fn core_records(
    fixture: &Fixture,
    fixed_edge: usize,
    reverse_equal: bool,
    reverse_ratio: bool,
    fixed_length: f64,
    ratio: f64,
) -> Vec<GeometricConstraintRecordV1> {
    let [first, second, _] = fixture.edges;
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[fixed_edge],
        length_mm: fixed_length,
    });
    let (equal_first, equal_second) = if reverse_equal {
        (second, first)
    } else {
        (first, second)
    };
    let equal = record(GeometricConstraintKindV1::EqualLength {
        first_edge: equal_first,
        second_edge: equal_second,
    });
    let (numerator_edge, denominator_edge) = if reverse_ratio {
        (second, first)
    } else {
        (first, second)
    };
    let ratio = record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge,
        denominator_edge,
        ratio,
    });
    vec![fixed, equal, ratio]
}

fn assert_single_target(
    outcome: &ConstraintPreflightV1,
    fixture: &Fixture,
    expected_ids: &[ConstraintId],
) {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        panic!("expected a direct rounded-residual conflict: {outcome:?}");
    };
    assert_eq!(conflicts.len(), 1);
    let mut expected_edges = [fixture.edges[0], fixture.edges[1]];
    expected_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength {
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
                    EqualLengthWithNonUnitRatioAndFixedLength { .. }
            ))
    )
}

#[test]
fn exact_pair_join_is_orientation_independent_and_source_order_canonical() {
    let fixture = Fixture::new();
    for fixed_edge in 0..2 {
        for reverse_equal in [false, true] {
            for reverse_ratio in [false, true] {
                let records =
                    core_records(&fixture, fixed_edge, reverse_equal, reverse_ratio, 1.0, 2.0);
                let expected_ids = sorted_ids(records.iter().map(|item| item.id));
                let expected = prepare(&fixture, records.clone()).preflight();
                assert_single_target(&expected, &fixture, &expected_ids);

                let mut reversed = records;
                reversed.reverse();
                assert_eq!(
                    prepare(&fixture, reversed).preflight(),
                    expected,
                    "prepared IDs, not input or operand orientation, select the proof"
                );
            }
        }
    }
}

#[test]
fn proof_requires_each_exact_cause_and_a_consistent_fixed_group() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 0, false, false, 1.0, 2.0);
    for removed in 0..records.len() {
        let subset = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed)
            .map(|(_, item)| item.clone());
        assert!(
            !matches!(
                prepare(&fixture, subset).preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "removing cause {removed} must withdraw the proof"
        );
    }

    let [first, second, third] = fixture.edges;
    let wrong_ratio_pair = prepare(
        &fixture,
        [
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: third,
                ratio: 2.0,
            }),
        ],
    )
    .preflight();
    assert!(!contains_target(&wrong_ratio_pair));

    let wrong_fixed_edge = prepare(
        &fixture,
        [
            record(GeometricConstraintKindV1::FixedLength {
                edge: third,
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            }),
        ],
    )
    .preflight();
    assert!(!contains_target(&wrong_fixed_edge));

    let inconsistent_fixed = prepare(
        &fixture,
        [
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 3.0,
            }),
        ],
    )
    .preflight();
    assert!(!contains_target(&inconsistent_fixed));
}

#[test]
fn duplicate_equal_fixed_and_ratio_records_choose_one_canonical_witness() {
    let fixture = Fixture::new();
    let [first, second, _] = fixture.edges;
    let equals = [
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: first,
            second_edge: second,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: second,
            second_edge: first,
        }),
    ];
    let fixed = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 1.0,
        }),
    ];
    let ratios = [
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: second,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: second,
            denominator_edge: first,
            ratio: 2.0,
        }),
    ];
    let fixed_id = minimum_id(&fixed);
    let expected_ids = sorted_ids([minimum_id(&equals), fixed_id, minimum_id(&ratios)]);
    let reciprocal_ids = sorted_ids([fixed_id, ratios[0].id, ratios[1].id]);
    let mut records = equals
        .into_iter()
        .chain(fixed)
        .chain(ratios)
        .collect::<Vec<_>>();
    let expected = prepare(&fixture, records.clone()).preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = &expected else {
        panic!("both rounded-residual families must prove");
    };
    assert_eq!(conflicts.len(), 2);
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength { .. }
    ) && conflict.constraint_ids() == expected_ids));
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength { .. }
    ) && conflict.constraint_ids() == reciprocal_ids));
    records.reverse();
    assert_eq!(prepare(&fixture, records).preflight(), expected);
}

#[test]
fn only_the_production_binary64_residual_decides_promotion() {
    let minimum = f64::from_bits(1);
    let one_up = 1.0_f64.next_up();
    let one_down = 1.0_f64.next_down();
    let cases = [
        (1.0, 1.0, false),
        (minimum, one_up, false),
        (minimum, one_down, false),
        (1.0, one_up, true),
        (minimum, 0.5, true),
        (f64::MAX, 2.0, true),
    ];
    for (fixed, ratio, proven) in cases {
        let residual = length_ratio_residual_binary64_v1(fixed, ratio, fixed);
        assert_eq!(residual != 0.0, proven, "{fixed:?}, {ratio:?}");
        let fixture = Fixture::new();
        let records = core_records(&fixture, 0, false, false, fixed, ratio);
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
fn bounded_oracle_boundaries_preserve_the_direct_three_record_core() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let mut records = core_records(&fixture, 1, true, true, 1.0, 2.0);
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

#[derive(Clone, Copy)]
struct StopOnCheckpoint {
    calls: usize,
    stop_on: usize,
    control: GeometricConstraintPreflightObserverControlV1,
}

impl GeometricConstraintPreflightObserverV1 for StopOnCheckpoint {
    fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
        self.calls += 1;
        if self.calls == self.stop_on {
            self.control
        } else {
            GeometricConstraintPreflightObserverControlV1::Continue
        }
    }
}

#[test]
fn work_cancellation_and_deadline_fail_closed_after_joining() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 0, false, false, 1.0, 2.0);
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
        let mut observer = StopOnCheckpoint {
            calls: 0,
            stop_on: 2,
            control,
        };
        assert!(matches!(
            prepared.preflight_with_observer(&mut observer),
            ConstraintPreflightV1::Unknown {
                reason: actual,
                ..
            } if actual == reason
        ));
        assert_eq!(
            observer.calls, 2,
            "the complete checkpoint must override output"
        );
    }
}

#[test]
fn solver_and_verifier_cannot_bypass_the_preflight_with_maximum_tolerance() {
    let fixture = Fixture::new();
    let raw = document(core_records(&fixture, 0, false, false, 1.0, 2.0));
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
