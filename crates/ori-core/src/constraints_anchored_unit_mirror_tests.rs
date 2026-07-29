use ori_domain::{EdgeKind, Point2};

use super::*;

struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 4],
    edges: [EdgeId; 3],
}

impl Fixture {
    fn new() -> Self {
        let mut vertices = std::array::from_fn(|_| VertexId::new());
        if vertices[2].canonical_bytes() > vertices[3].canonical_bytes() {
            vertices.swap(2, 3);
        }
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
                        position: Point2::new(0.0, 0.5),
                    },
                    Vertex {
                        id: vertices[3],
                        position: Point2::new(0.0, -0.5),
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
                        start: vertices[0],
                        end: vertices[2],
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: edges[2],
                        start: vertices[2],
                        end: vertices[3],
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

fn core_records(fixture: &Fixture, separation_length: f64) -> [GeometricConstraintRecordV1; 7] {
    [
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[2],
            second_vertex: fixture.vertices[3],
            axis_edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[2],
            length_mm: separation_length,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[1],
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
    .expect("anchored unit-mirror fixture prepares")
}

fn sorted_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn canonical_mirrored_vertices(fixture: &Fixture) -> (VertexId, VertexId) {
    if fixture.vertices[2].canonical_bytes() < fixture.vertices[3].canonical_bytes() {
        (fixture.vertices[2], fixture.vertices[3])
    } else {
        (fixture.vertices[3], fixture.vertices[2])
    }
}

#[test]
fn anchored_unit_mirror_emits_canonical_order_independent_direct_unsat_evidence() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 1.0);
    let expected_ids = sorted_ids(&records);
    let (first_vertex, second_vertex) = canonical_mirrored_vertices(&fixture);
    let expected = ConstraintPreflightV1::DirectConflict {
        conflicts: vec![DirectConstraintConflictV1 {
            conflict:
                DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                    first_vertex,
                    second_vertex,
                    axis_edge: fixture.edges[0],
                    fixed_separation_edge: fixture.edges[2],
                },
            constraint_ids: expected_ids.clone(),
        }],
    };
    let prepared = prepare(&fixture, records.clone());
    assert_eq!(prepared.preflight(), expected);
    let mut reversed = records.to_vec();
    reversed.reverse();
    assert_eq!(
        prepare(&fixture, reversed).preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict:
                    DirectConstraintConflictKindV1::
                        MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                            first_vertex,
                            second_vertex,
                            axis_edge: fixture.edges[0],
                            fixed_separation_edge: fixture.edges[2],
                        },
                constraint_ids: expected_ids.clone(),
            }],
        }
    );
    let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
        find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the anchored exact mirror slice must feed the bounded UNSAT oracle");
    };
    assert_eq!(constraint_ids, expected_ids);
    for removed in records.iter().map(|record| record.id) {
        let subset = records
            .iter()
            .filter(|record| record.id != removed)
            .cloned();
        assert!(
            !matches!(
                prepare(&fixture, subset).preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "removing every listed evidence record must remove the direct contradiction"
        );
    }
}

#[test]
fn anchored_unit_mirror_keeps_a_satisfiable_witness_and_generic_case_unknown() {
    let fixture = Fixture::new();
    let mut satisfiable = core_records(&fixture, 1.0).into_iter().collect::<Vec<_>>();
    satisfiable.remove(1);
    let _ = satisfiable.pop();
    let _ = satisfiable.pop();
    let satisfiable_document = document(satisfiable.clone());
    assert!(
        crate::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &fixture.pattern,
            &satisfiable_document,
        )
        .expect("compatible anchored mirror model is valid")
        .is_some(),
        "the compatible counterpart has an exact binary64 SAT witness"
    );
    assert!(!matches!(
        prepare(&fixture, satisfiable).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    let generic = prepare(&fixture, core_records(&fixture, 2.0));
    assert!(matches!(
        generic.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn anchored_unit_mirror_preserves_resource_cancel_and_deadline_unknown_boundaries() {
    let fixture = Fixture::new();
    let prepared = prepare(&fixture, core_records(&fixture, 1.0));
    let limited = GeometricConstraintSetV1 {
        source_pattern: &fixture.pattern,
        constraints: prepared.constraints.clone(),
        max_preflight_checks: 6,
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
