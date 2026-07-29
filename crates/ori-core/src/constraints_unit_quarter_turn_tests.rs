use ori_domain::{EdgeKind, Point2};

use super::*;

struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 4],
    radius_edge: EdgeId,
    unrelated_edge: EdgeId,
}

impl Fixture {
    fn new(reverse_radius: bool) -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let radius_edge = EdgeId::new();
        let unrelated_edge = EdgeId::new();
        let (radius_start, radius_end) = if reverse_radius {
            (vertices[1], vertices[0])
        } else {
            (vertices[0], vertices[1])
        };
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
                    Vertex {
                        id: vertices[3],
                        position: Point2::new(2.0, 2.0),
                    },
                ],
                edges: vec![
                    Edge {
                        id: radius_edge,
                        start: radius_start,
                        end: radius_end,
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: unrelated_edge,
                        start: vertices[0],
                        end: vertices[3],
                        kind: EdgeKind::Auxiliary,
                    },
                ],
            },
            vertices,
            radius_edge,
            unrelated_edge,
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

fn core_records(fixture: &Fixture, angle_degrees: f64) -> [GeometricConstraintRecordV1; 2] {
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
    .expect("quarter-turn collinear-radius fixture prepares")
}

fn sorted_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn assert_solver_required(preflight: ConstraintPreflightV1) {
    assert!(matches!(
        preflight,
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn exact_quarter_turns_emit_canonical_two_id_direct_unsat_evidence() {
    for angle_degrees in [90.0, 270.0] {
        let fixture = Fixture::new(false);
        let records = core_records(&fixture, angle_degrees);
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
        assert_eq!(prepare(&fixture, reversed).preflight(), expected);
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
            assert_solver_required(prepare(&fixture, subset).preflight());
        }
    }
}

#[test]
fn duplicate_records_choose_one_canonical_pair_independent_of_order() {
    let fixture = Fixture::new(false);
    let first = core_records(&fixture, 90.0);
    let second = core_records(&fixture, 90.0);
    let records = vec![
        first[0].clone(),
        second[0].clone(),
        first[1].clone(),
        second[1].clone(),
    ];
    let expected = sorted_ids(&[
        [first[0].clone(), second[0].clone()]
            .into_iter()
            .min_by_key(|record| record.id.canonical_bytes())
            .expect("rotation minimum"),
        [first[1].clone(), second[1].clone()]
            .into_iter()
            .min_by_key(|record| record.id.canonical_bytes())
            .expect("point minimum"),
    ]);
    for ordered in [records.clone(), records.into_iter().rev().collect()] {
        let ConstraintPreflightV1::DirectConflict { conflicts } =
            prepare(&fixture, ordered).preflight()
        else {
            panic!("duplicate exact witnesses still have one direct conflict");
        };
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].constraint_ids(), expected);
    }
}

#[test]
fn nonquarter_angles_reversed_or_unrelated_edges_remain_unknown() {
    let fixture = Fixture::new(false);
    for angle in [
        180.0,
        45.0,
        90.0_f64.next_down(),
        90.0_f64.next_up(),
        270.0_f64.next_down(),
        270.0_f64.next_up(),
        f64::from_bits(1),
    ] {
        assert_solver_required(prepare(&fixture, core_records(&fixture, angle)).preflight());
    }
    let overflow = document(core_records(&fixture, f64::MAX));
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            &overflow,
            GeometricConstraintLimitsV1::default(),
        ),
        Err(GeometricConstraintErrorV1::RotationAngleOutOfRange { .. })
    ));

    let reversed = Fixture::new(true);
    assert_solver_required(prepare(&reversed, core_records(&reversed, 90.0)).preflight());

    let unrelated = [
        core_records(&fixture, 90.0)[0].clone(),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.unrelated_edge,
        }),
    ];
    assert_solver_required(prepare(&fixture, unrelated).preflight());

    let wrong_point = [
        core_records(&fixture, 90.0)[0].clone(),
        record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[3],
            line_edge: fixture.radius_edge,
        }),
    ];
    assert_solver_required(prepare(&fixture, wrong_point).preflight());
}

#[test]
fn exact_quarter_turn_preserves_resource_cancel_and_deadline_unknown_boundaries() {
    let fixture = Fixture::new(false);
    let prepared = prepare(&fixture, core_records(&fixture, 90.0));
    let limited = GeometricConstraintSetV1 {
        source_pattern: &fixture.pattern,
        constraints: prepared.constraints.clone(),
        raw_mirror_roles: prepared.raw_mirror_roles.clone(),
        max_preflight_checks: 1,
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
    for (control, expected_reason) in [
        (
            GeometricConstraintPreflightObserverControlV1::Cancelled,
            GeometricConstraintUnknownReasonV1::Cancelled,
        ),
        (
            GeometricConstraintPreflightObserverControlV1::DeadlineReached,
            GeometricConstraintUnknownReasonV1::DeadlineReached,
        ),
    ] {
        assert!(matches!(
            prepared.preflight_with_observer(&mut Stop(control)),
            ConstraintPreflightV1::Unknown { reason, .. } if reason == expected_reason
        ));
    }
}
