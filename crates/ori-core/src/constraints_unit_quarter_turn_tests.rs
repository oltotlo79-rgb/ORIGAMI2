use ori_domain::{EdgeKind, Point2};

use super::*;

struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 3],
    radius_edge: EdgeId,
}

impl Fixture {
    fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let radius_edge = EdgeId::new();
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
                        position: Point2::new(0.0, 1.0),
                    },
                ],
                edges: vec![Edge {
                    id: radius_edge,
                    start: vertices[0],
                    end: vertices[1],
                    kind: EdgeKind::Auxiliary,
                }],
            },
            vertices,
            radius_edge,
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

fn core_records(fixture: &Fixture, angle_degrees: f64) -> [GeometricConstraintRecordV1; 4] {
    [
        record(GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: fixture.vertices[0],
            source_vertex: fixture.vertices[1],
            target_vertex: fixture.vertices[2],
            angle_degrees,
        }),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.radius_edge,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.radius_edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.radius_edge,
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
    .expect("unit quarter-turn fixture prepares")
}

fn sorted_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

#[test]
fn unit_horizontal_radius_quarter_turn_emits_canonical_direct_unsat_evidence() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, 90.0);
    let expected_ids = sorted_ids(&records);
    let prepared = prepare(&fixture, records.clone());
    let expected = ConstraintPreflightV1::DirectConflict {
        conflicts: vec![DirectConstraintConflictV1 {
            conflict: DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius {
                center_vertex: fixture.vertices[0],
                source_vertex: fixture.vertices[1],
                target_vertex: fixture.vertices[2],
                line_edge: fixture.radius_edge,
            },
            constraint_ids: expected_ids.clone(),
        }],
    };
    assert_eq!(prepared.preflight(), expected);
    let mut reversed = records.to_vec();
    reversed.reverse();
    assert_eq!(
        prepare(&fixture, reversed).preflight(),
        ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius {
                    center_vertex: fixture.vertices[0],
                    source_vertex: fixture.vertices[1],
                    target_vertex: fixture.vertices[2],
                    line_edge: fixture.radius_edge,
                },
                constraint_ids: expected_ids.clone(),
            }],
        }
    );
    let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
        find_bounded_direct_mus_v1(&prepared)
    else {
        panic!("the exact quarter-turn slice must feed the bounded UNSAT oracle");
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
fn unit_quarter_turn_slice_keeps_a_satisfiable_witness_and_generic_case_unknown() {
    let fixture = Fixture::new();
    let mut satisfiable = core_records(&fixture, 90.0).into_iter().collect::<Vec<_>>();
    satisfiable.remove(1);
    let satisfiable_document = document(satisfiable.clone());
    assert!(
        crate::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &fixture.pattern,
            &satisfiable_document,
        )
        .expect("compatible unit quarter-turn model is valid")
        .is_some(),
        "the compatible counterpart has an exact binary64 SAT witness"
    );
    assert!(!matches!(
        prepare(&fixture, satisfiable).preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));

    let generic = prepare(&fixture, core_records(&fixture, 45.0));
    assert!(matches!(
        generic.preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn unit_quarter_turn_slice_preserves_resource_cancel_and_deadline_unknown_boundaries() {
    let fixture = Fixture::new();
    let prepared = prepare(&fixture, core_records(&fixture, 90.0));
    let limited = GeometricConstraintSetV1 {
        source_pattern: &fixture.pattern,
        constraints: prepared.constraints.clone(),
        raw_mirror_roles: prepared.raw_mirror_roles.clone(),
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
