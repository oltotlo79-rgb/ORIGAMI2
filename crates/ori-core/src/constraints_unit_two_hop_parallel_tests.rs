use ori_domain::{EdgeKind, Point2};

use super::*;

pub(super) struct UnitTwoHopFixture {
    pub(super) pattern: CreasePattern,
    pub(super) center: VertexId,
    pub(super) edges: [EdgeId; 3],
}

impl UnitTwoHopFixture {
    pub(super) fn new() -> Self {
        let center = VertexId::new();
        let endpoints = [VertexId::new(), VertexId::new(), VertexId::new()];
        let edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
        Self {
            pattern: CreasePattern {
                vertices: vec![
                    Vertex {
                        id: center,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: endpoints[0],
                        position: Point2::new(3.0, 1.0),
                    },
                    Vertex {
                        id: endpoints[1],
                        position: Point2::new(2.0, 2.0),
                    },
                    Vertex {
                        id: endpoints[2],
                        position: Point2::new(1.0, 3.0),
                    },
                ],
                edges: edges
                    .into_iter()
                    .zip(endpoints)
                    .map(|(id, end)| Edge {
                        id,
                        start: center,
                        end,
                        kind: EdgeKind::Auxiliary,
                    })
                    .collect(),
            },
            center,
            edges,
        }
    }

    pub(super) fn reverse_edge_storage(&mut self) {
        for edge in &mut self.pattern.edges {
            (edge.start, edge.end) = (edge.end, edge.start);
        }
    }
}

pub(super) fn unit_two_hop_records(
    fixture: &UnitTwoHopFixture,
    terminal_length: f64,
) -> Vec<GeometricConstraintRecordV1> {
    vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[2],
            length_mm: terminal_length,
        }),
    ]
}

pub(super) fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

pub(super) fn document(
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
    }
}

fn prepared<'a>(
    fixture: &'a UnitTwoHopFixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("unit two-hop fixture prepares")
}

pub(super) fn sorted_ids(
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> Vec<ConstraintId> {
    let mut ids = records
        .into_iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn target_conflict<'a>(
    preflight: &'a ConstraintPreflightV1,
    fixture: &UnitTwoHopFixture,
) -> Option<&'a DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        return None;
    };
    conflicts.iter().find(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
                horizontal_edge,
                vertical_edge,
                parallel_constraint_count: 2,
            } if *horizontal_edge == fixture.edges[0]
                && *vertical_edge == fixture.edges[2]
        )
    })
}

#[test]
fn exact_unit_two_hop_subset_emits_five_canonical_causes_and_is_direct_minimal() {
    let fixture = UnitTwoHopFixture::new();
    let records = unit_two_hop_records(&fixture, 1.0);
    let expected = sorted_ids(records.iter().cloned());
    let prepared_set = prepared(&fixture, records.iter().cloned());
    let preflight = prepared_set.preflight();
    assert_eq!(
        target_conflict(&preflight, &fixture)
            .expect("the exact theorem must be emitted")
            .constraint_ids(),
        expected,
    );
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared_set),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == expected && oracle_calls <= 31
    ));
    for removed in &records {
        assert!(
            !matches!(
                prepared(
                    &fixture,
                    records
                        .iter()
                        .filter(|record| record.id != removed.id)
                        .cloned(),
                )
                .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "every immediate deletion must remove the direct theorem",
        );
    }
}

#[test]
fn source_operand_and_edge_storage_order_do_not_change_the_canonical_theorem() {
    let fixture = UnitTwoHopFixture::new();
    let records = unit_two_hop_records(&fixture, 1.0);
    let expected = prepared(&fixture, records.iter().cloned()).preflight();

    let mut reversed_fixture = UnitTwoHopFixture {
        pattern: fixture.pattern.clone(),
        center: fixture.center,
        edges: fixture.edges,
    };
    reversed_fixture.pattern.vertices.reverse();
    reversed_fixture.pattern.edges.reverse();
    reversed_fixture.reverse_edge_storage();
    let mut reversed_records = records.clone();
    reversed_records.reverse();
    for record in &mut reversed_records {
        if let GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } = &mut record.constraint
        {
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
    }
    assert_eq!(
        prepared(&reversed_fixture, reversed_records).preflight(),
        expected,
    );
}

#[test]
fn nonunit_one_ulp_missing_and_longer_paths_remain_solver_required() {
    for terminal_length in [1.0_f64.next_down(), 1.0_f64.next_up(), 0.5, 2.0, f64::MAX] {
        let fixture = UnitTwoHopFixture::new();
        let result =
            prepared(&fixture, unit_two_hop_records(&fixture, terminal_length)).preflight();
        assert!(matches!(
            result,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ));
    }

    let fixture = UnitTwoHopFixture::new();
    let mut missing = unit_two_hop_records(&fixture, 1.0);
    missing.pop();
    assert!(matches!(
        prepared(&fixture, missing).preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));

    let extra_vertex = VertexId::new();
    let extra_edge = EdgeId::new();
    let mut long_fixture = UnitTwoHopFixture {
        pattern: fixture.pattern.clone(),
        center: fixture.center,
        edges: fixture.edges,
    };
    long_fixture.pattern.vertices.push(Vertex {
        id: extra_vertex,
        position: Point2::new(-2.0, 4.0),
    });
    long_fixture.pattern.edges.push(Edge {
        id: extra_edge,
        start: fixture.center,
        end: extra_vertex,
        kind: EdgeKind::Auxiliary,
    });
    let long = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[2],
            second_edge: extra_edge,
        }),
        record(GeometricConstraintKindV1::Vertical { edge: extra_edge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: extra_edge,
            length_mm: 1.0,
        }),
    ];
    assert!(matches!(
        prepared(&long_fixture, long).preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn reused_terminal_edge_is_owned_by_the_smaller_same_edge_theorem() {
    let fixture = UnitTwoHopFixture::new();
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
    ];
    let preflight = prepared(&fixture, records).preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        panic!("same-edge horizontal/vertical/unit core must be direct");
    };
    assert!(conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::HorizontalAndVertical { edge }
                if *edge == fixture.edges[0]
        ) && candidate.constraint_ids().len() == 3
    }));
    assert!(!conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent { .. }
        )
    }));
}

#[test]
fn scanner_work_and_storage_exact_limits_are_admitted_and_one_short_fails_closed() {
    let fixture = UnitTwoHopFixture::new();
    let records = unit_two_hop_records(&fixture, 1.0);
    let expected_ids = sorted_ids(records.iter().cloned());

    UNIT_TWO_HOP_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
    UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_LIMIT.with(|limit| limit.set(None));
    assert!(matches!(
        prepared(&fixture, records.iter().cloned()).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    let exact_work = UNIT_TWO_HOP_PARALLEL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
    let exact_storage = UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_OBSERVED.with(std::cell::Cell::get);
    assert!(exact_work > 0);
    assert!(exact_storage > 0);

    UNIT_TWO_HOP_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work)));
    assert!(matches!(
        prepared(&fixture, records.iter().cloned()).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    UNIT_TWO_HOP_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
    assert_eq!(
        prepared(&fixture, records.iter().cloned()).preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            unchecked_constraint_ids: expected_ids.clone(),
        }
    );
    UNIT_TWO_HOP_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));

    UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_LIMIT.with(|limit| limit.set(Some(exact_storage)));
    assert!(matches!(
        prepared(&fixture, records.iter().cloned()).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_LIMIT.with(|limit| limit.set(Some(exact_storage - 1)));
    assert_eq!(
        prepared(&fixture, records).preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::StorageLimitExceeded,
            unchecked_constraint_ids: expected_ids,
        }
    );
    UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_LIMIT.with(|limit| limit.set(None));
}

struct StopObserver {
    calls: usize,
    stop_at: usize,
    control: GeometricConstraintPreflightObserverControlV1,
}

impl GeometricConstraintPreflightObserverV1 for StopObserver {
    fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
        self.calls += 1;
        if self.calls == self.stop_at {
            self.control
        } else {
            GeometricConstraintPreflightObserverControlV1::Continue
        }
    }
}

#[test]
fn cancellation_and_deadline_at_each_reachable_checkpoint_fail_closed() {
    let fixture = UnitTwoHopFixture::new();
    let records = unit_two_hop_records(&fixture, 1.0);
    let prepared = prepared(&fixture, records);
    let mut baseline = StopObserver {
        calls: 0,
        stop_at: usize::MAX,
        control: GeometricConstraintPreflightObserverControlV1::Cancelled,
    };
    assert!(matches!(
        prepared.preflight_with_observer(&mut baseline),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    assert!(baseline.calls >= 3);

    for stop_at in 1..=baseline.calls {
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
            let mut observer = StopObserver {
                calls: 0,
                stop_at,
                control,
            };
            assert!(matches!(
                prepared.preflight_with_observer(&mut observer),
                ConstraintPreflightV1::Unknown {
                    reason: actual,
                    ..
                } if actual == reason
            ));
            assert_eq!(observer.calls, stop_at);
        }
    }
}
