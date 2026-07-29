use ori_domain::{EdgeKind, Point2};

use super::*;

struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 3],
    edges: [EdgeId; 2],
}

impl Fixture {
    fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let edges = std::array::from_fn(|_| EdgeId::new());
        Self {
            pattern: CreasePattern {
                vertices: vec![
                    Vertex {
                        id: vertices[0],
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: vertices[1],
                        position: Point2::new(1.0, 0.0),
                    },
                    Vertex {
                        id: vertices[2],
                        position: Point2::new(2.0, 0.0),
                    },
                ],
                edges: vec![
                    Edge {
                        id: edges[0],
                        start: vertices[0],
                        end: vertices[1],
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: edges[1],
                        start: vertices[1],
                        end: vertices[2],
                        kind: EdgeKind::Auxiliary,
                    },
                ],
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

fn core_records(
    fixture: &Fixture,
    angle_degrees: f64,
    unit_length: f64,
) -> [GeometricConstraintRecordV1; 4] {
    [
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[1],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            angle_degrees,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: unit_length,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: unit_length,
        }),
    ]
}

fn prepare<'a>(
    fixture: &'a Fixture,
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(constraints),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("unit parallel-angle fixture prepares")
}

fn sorted_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

#[test]
fn unit_parallel_and_nonzero_angle_emit_canonical_direct_unsat_evidence() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 90.0, 1.0);
    let expected_ids = sorted_ids(&records);
    let mut expected_edges = fixture.edges;
    expected_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let prepared = prepare(&fixture, records.clone());
    assert_eq!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                    first_edge: expected_edges[0],
                    second_edge: expected_edges[1],
                },
                constraint_ids: expected_ids.clone(),
            }],
        }
    );
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        prepare(&fixture, reversed).preflight(),
        prepared.preflight(),
        "the strict direct evidence must be invariant to document storage order"
    );
    let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
        find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the exact-unit parallel slice must feed the bounded UNSAT oracle");
    };
    assert_eq!(constraint_ids, expected_ids);
    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned();
        assert!(!matches!(
            prepare(&fixture, subset).preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }
}

#[test]
fn unit_parallel_slice_keeps_a_satisfiable_model_and_nonunit_case_unknown() {
    let fixture = Fixture::new();
    let satisfiable = document(core_records(&fixture, 180.0, 1.0));
    assert!(
        crate::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &fixture.pattern,
            &satisfiable,
        )
        .expect("compatible unit-parallel model is valid")
        .is_some(),
        "the compatible counterpart has an exact binary64 SAT witness"
    );
    assert!(!matches!(
        prepare(&fixture, satisfiable.constraints.clone()).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    let nonunit = prepare(&fixture, core_records(&fixture, 90.0, 2.0));
    assert!(matches!(
        nonunit.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn unit_parallel_slice_preserves_resource_cancel_and_deadline_unknown_boundaries() {
    let fixture = Fixture::new();
    let prepared = prepare(&fixture, core_records(&fixture, 90.0, 1.0));
    let limited = GeometricConstraintSetV1 {
        source_pattern: &fixture.pattern,
        constraints: prepared.constraints.clone(),
        max_preflight_checks: 3,
    };
    assert!(matches!(
        limited.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            ..
        }
    ));

    struct Stop(GeometricConstraintPreflightObserverControlV1);
    impl GeometricConstraintPreflightObserverV1 for Stop {
        fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
            self.0
        }
    }
    for control in [
        GeometricConstraintPreflightObserverControlV1::Cancelled,
        GeometricConstraintPreflightObserverControlV1::DeadlineReached,
    ] {
        let expected_reason = match control {
            GeometricConstraintPreflightObserverControlV1::Cancelled => {
                GeometricConstraintUnknownReasonV1::Cancelled
            }
            GeometricConstraintPreflightObserverControlV1::DeadlineReached => {
                GeometricConstraintUnknownReasonV1::DeadlineReached
            }
            GeometricConstraintPreflightObserverControlV1::Continue => unreachable!(),
        };
        assert!(matches!(
            prepared.preflight_with_observer(&mut Stop(control)),
            ConstraintPreflightV1::Unknown { reason, .. } if reason == expected_reason
        ));
    }
}
