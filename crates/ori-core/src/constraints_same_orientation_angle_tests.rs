use ori_domain::{EdgeKind, Point2};
use ori_numeric::{deterministic_atan2_v1, deterministic_degrees_to_radians_v1};

use super::*;

pub(super) struct Fixture {
    pub(super) pattern: CreasePattern,
    pub(super) vertices: [VertexId; 5],
    pub(super) edges: [EdgeId; 4],
}

impl Fixture {
    pub(super) fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 1.0),
            Point2::new(-1.0, 3.0),
            Point2::new(4.0, -2.0),
            Point2::new(8.0, 8.0),
        ];
        let vertex_records = vertices
            .into_iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = std::array::from_fn(|_| EdgeId::new());
        let edge_records = [
            Edge {
                id: edges[0],
                start: vertices[0],
                end: vertices[1],
                kind: EdgeKind::Auxiliary,
            },
            // Reverse storage direction exercises the outward-vector branch.
            Edge {
                id: edges[1],
                start: vertices[2],
                end: vertices[0],
                kind: EdgeKind::Auxiliary,
            },
            // A distinct ID for the same real segment must never be joined.
            Edge {
                id: edges[2],
                start: vertices[0],
                end: vertices[1],
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: edges[3],
                start: vertices[0],
                end: vertices[3],
                kind: EdgeKind::Auxiliary,
            },
        ];
        Self {
            pattern: CreasePattern {
                vertices: vertex_records,
                edges: edge_records.into(),
            },
            vertices,
            edges,
        }
    }
}

pub(super) fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

pub(super) fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().collect(),
    }
}

pub(super) fn core_records(
    fixture: &Fixture,
    vertical: bool,
    angle_degrees: f64,
) -> Vec<GeometricConstraintRecordV1> {
    let orientation = |edge| {
        if vertical {
            GeometricConstraintKindV1::Vertical { edge }
        } else {
            GeometricConstraintKindV1::Horizontal { edge }
        }
    };
    vec![
        record(orientation(fixture.edges[0])),
        record(orientation(fixture.edges[1])),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[0],
            angle_degrees,
        }),
    ]
}

pub(super) fn prepare<'a>(
    fixture: &'a Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("same-orientation fixture must prepare")
}

pub(super) fn sorted_ids(ids: impl IntoIterator<Item = ConstraintId>) -> Vec<ConstraintId> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

pub(super) fn has_target(outcome: &ConstraintPreflightV1) -> bool {
    matches!(
        outcome,
        ConstraintPreflightV1::DirectConflict { conflicts }
            if conflicts.iter().any(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    SameOrientationWithFixedNonParallelAngle { .. }
            ))
    )
}

pub(super) fn assert_target(
    outcome: &ConstraintPreflightV1,
    fixture: &Fixture,
    expected_ids: &[ConstraintId],
) {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        panic!("expected a same-orientation binary64 conflict: {outcome:?}");
    };
    assert_eq!(conflicts.len(), 1);
    let mut expected_edges = [fixture.edges[0], fixture.edges[1]];
    expected_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::SameOrientationWithFixedNonParallelAngle {
            first_edge,
            second_edge,
        } if [*first_edge, *second_edge] == expected_edges
    ));
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);
}

#[test]
fn zero_cross_classification_uses_the_frozen_binary64_residual() {
    let minimum_degrees = f64::from_bits(1);
    assert_eq!(
        deterministic_degrees_to_radians_v1(minimum_degrees)
            .unwrap()
            .to_bits(),
        0.0_f64.to_bits(),
        "the smallest positive degree underflows to positive radian zero"
    );
    let wrapped_subnormal = f64::from_bits(0x39);
    assert_ne!(
        deterministic_degrees_to_radians_v1(wrapped_subnormal).unwrap(),
        0.0
    );
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(0.0, wrapped_subnormal),
        0.0,
        "a non-zero stored degree can disappear in deterministic proof wrapping"
    );
    assert_eq!(
        deterministic_degrees_to_radians_v1(180.0)
            .unwrap()
            .to_bits(),
        std::f64::consts::PI.to_bits()
    );
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(std::f64::consts::PI, 180.0),
        0.0
    );
    assert_ne!(
        deterministic_degrees_to_radians_v1(180.0_f64.next_down())
            .unwrap()
            .to_bits(),
        std::f64::consts::PI.to_bits(),
        "the immediately lower stored degree remains distinguishable at pi"
    );
    for angle in [-0.0, 0.0, 180.0, f64::from_bits(1), wrapped_subnormal] {
        assert!(!fixed_angle_rejects_zero_cross_binary64_v1(angle));
    }
    for angle in [1.0e-12, 90.0, 180.0_f64.next_down()] {
        assert!(fixed_angle_rejects_zero_cross_binary64_v1(angle));
    }

    let dot_classes = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    let axes = [
        f64::NEG_INFINITY,
        -f64::MAX,
        -1.0,
        -0.0,
        0.0,
        1.0,
        f64::MAX,
        f64::INFINITY,
    ];
    let mut saw_nonfinite_cross = false;
    for first_axis in axes {
        for second_axis in axes {
            for first_zero in [-0.0, 0.0] {
                for second_zero in [-0.0, 0.0] {
                    let horizontal_cross = first_axis * second_zero - first_zero * second_axis;
                    let horizontal_dot = first_axis * second_axis + first_zero * second_zero;
                    let vertical_cross = first_zero * second_axis - first_axis * second_zero;
                    let vertical_dot = first_zero * second_zero + first_axis * second_axis;
                    saw_nonfinite_cross |= horizontal_cross.is_nan() || vertical_cross.is_nan();
                    for (absolute_cross, dot) in [
                        (horizontal_cross.abs(), horizontal_dot),
                        (vertical_cross.abs(), vertical_dot),
                    ] {
                        if let Ok(actual) = deterministic_atan2_v1(absolute_cross, dot) {
                            assert!(dot_classes.iter().any(|dot_class| {
                                deterministic_atan2_v1(0.0, *dot_class)
                                    .is_ok_and(|class| actual.to_bits() == class.to_bits())
                            }));
                            let residual =
                                deterministic_fixed_angle_residual_binary64_v1(actual, 90.0);
                            assert!(!residual.is_finite() || residual != 0.0);
                        }
                    }
                }
            }
        }
    }
    assert!(
        saw_nonfinite_cross,
        "overflowed outward components times zero must exercise a NaN cross"
    );

    for (first, second) in [
        ((0.0, 0.0), (0.0, 0.0)),
        ((0.0, -0.0), (1.0, 0.0)),
        ((-1.0, -0.0), (-0.0, 0.0)),
    ] {
        let cross: f64 = first.0 * second.1 - first.1 * second.0;
        let dot: f64 = first.0 * second.0 + first.1 * second.1;
        let actual = deterministic_atan2_v1(cross.abs(), dot).unwrap();
        let residual = deterministic_fixed_angle_residual_binary64_v1(actual, 90.0);
        assert!(
            !residual.is_finite() || residual != 0.0,
            "both-edge and either one-edge collapse remain rejected"
        );
    }
}

#[test]
fn only_angles_rejecting_every_zero_cross_class_are_promoted() {
    for vertical in [false, true] {
        for angle in [1.0e-12, 90.0, 180.0_f64.next_down()] {
            let fixture = Fixture::new();
            let records = core_records(&fixture, vertical, angle);
            let expected_ids = sorted_ids(records.iter().map(|item| item.id));
            assert_target(
                &prepare(&fixture, records).preflight(),
                &fixture,
                &expected_ids,
            );
        }
        for angle in [-0.0, 0.0, 180.0, f64::from_bits(1), f64::from_bits(0x39)] {
            let fixture = Fixture::new();
            let outcome = prepare(&fixture, core_records(&fixture, vertical, angle)).preflight();
            assert!(!has_target(&outcome), "{angle:?}: {outcome:?}");
            assert!(matches!(
                outcome,
                ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    ..
                }
            ));
        }
    }
}

#[test]
fn witness_is_canonical_storage_invariant_and_irredundant() {
    let fixture = Fixture::new();
    let records = core_records(&fixture, false, 90.0);
    let expected_ids = sorted_ids(records.iter().map(|item| item.id));
    let expected = prepare(&fixture, records.clone()).preflight();
    assert_target(&expected, &fixture, &expected_ids);

    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(prepare(&fixture, reversed).preflight(), expected);
    for removed in 0..records.len() {
        let subset = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed)
            .map(|(_, item)| item.clone());
        assert!(!has_target(&prepare(&fixture, subset).preflight()));
    }

    let mut duplicated = records;
    duplicated.extend(core_records(&fixture, false, 90.0));
    let expected_ids = sorted_ids([
        duplicated[..2]
            .iter()
            .chain(&duplicated[3..5])
            .filter(|item| {
                matches!(
                    item.constraint,
                    GeometricConstraintKindV1::Horizontal { edge }
                        if edge == fixture.edges[0]
                )
            })
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap(),
        duplicated[..2]
            .iter()
            .chain(&duplicated[3..5])
            .filter(|item| {
                matches!(
                    item.constraint,
                    GeometricConstraintKindV1::Horizontal { edge }
                        if edge == fixture.edges[1]
                )
            })
            .map(|item| item.id)
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap(),
        [duplicated[2].id, duplicated[5].id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap(),
    ]);
    let expected = prepare(&fixture, duplicated.clone()).preflight();
    assert_target(&expected, &fixture, &expected_ids);
    duplicated.reverse();
    assert_eq!(prepare(&fixture, duplicated).preflight(), expected);
}

#[test]
fn endpoint_storage_direction_never_changes_the_exact_edge_proof() {
    for reverse_first in [false, true] {
        for reverse_second in [false, true] {
            let mut fixture = Fixture::new();
            if reverse_first {
                let edge = &mut fixture.pattern.edges[0];
                std::mem::swap(&mut edge.start, &mut edge.end);
            }
            if reverse_second {
                let edge = &mut fixture.pattern.edges[1];
                std::mem::swap(&mut edge.start, &mut edge.end);
            }
            let records = core_records(&fixture, false, 90.0);
            let expected_ids = sorted_ids(records.iter().map(|item| item.id));
            assert_target(
                &prepare(&fixture, records).preflight(),
                &fixture,
                &expected_ids,
            );
        }
    }
}

#[test]
fn proof_requires_exact_edge_and_validated_shared_vertex_identity() {
    let fixture = Fixture::new();
    let angle = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 90.0,
    });
    let alias = [
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[1],
        }),
        angle.clone(),
    ];
    assert!(!has_target(&prepare(&fixture, alias).preflight()));

    let mixed = [
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[1],
        }),
        angle,
    ];
    assert!(!has_target(&prepare(&fixture, mixed).preflight()));

    let wrong_pair = [
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[3],
            angle_degrees: 90.0,
        }),
    ];
    assert!(!has_target(&prepare(&fixture, wrong_pair).preflight()));

    let invalid_vertex = document([record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[4],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 90.0,
    })]);
    assert!(matches!(
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            &invalid_vertex,
            GeometricConstraintLimitsV1::default(),
        ),
        Err(GeometricConstraintErrorV1::VertexNotIncidentToEdge { .. })
    ));

    for angle_degrees in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let invalid = document([record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            angle_degrees,
        })]);
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &fixture.pattern,
                &invalid,
                GeometricConstraintLimitsV1::default(),
            ),
            Err(GeometricConstraintErrorV1::NonFiniteValue {
                field: ConstraintScalarFieldV1::AngleDegrees,
                ..
            })
        ));
    }
}
