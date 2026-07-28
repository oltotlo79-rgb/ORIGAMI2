use super::same_orientation_angle_tests::{Fixture, document, prepare, record, sorted_ids};
use super::*;
use ori_numeric::deterministic_atan2_v1;

pub(super) fn core_records(
    fixture: &Fixture,
    reverse_roles: bool,
    angle_degrees: f64,
) -> Vec<GeometricConstraintRecordV1> {
    let (horizontal_edge, vertical_edge) = if reverse_roles {
        (fixture.edges[1], fixture.edges[0])
    } else {
        (fixture.edges[0], fixture.edges[1])
    };
    vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: horizontal_edge,
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: vertical_edge,
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[0],
            angle_degrees,
        }),
    ]
}

pub(super) fn has_target(outcome: &ConstraintPreflightV1) -> bool {
    matches!(
        outcome,
        ConstraintPreflightV1::DirectConflict { conflicts }
            if conflicts.iter().any(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    PerpendicularOrientationsWithFixedNonRightAngle { .. }
            ))
    )
}

pub(super) fn assert_target(
    outcome: &ConstraintPreflightV1,
    fixture: &Fixture,
    reverse_roles: bool,
    expected_ids: &[ConstraintId],
) {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        panic!("expected a perpendicular binary64 conflict: {outcome:?}");
    };
    assert_eq!(conflicts.len(), 1);
    let (horizontal_edge, vertical_edge) = if reverse_roles {
        (fixture.edges[1], fixture.edges[0])
    } else {
        (fixture.edges[0], fixture.edges[1])
    };
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::PerpendicularOrientationsWithFixedNonRightAngle {
            horizontal_edge: actual_horizontal,
            vertical_edge: actual_vertical,
        } if *actual_horizontal == horizontal_edge && *actual_vertical == vertical_edge
    ));
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);
}

fn deterministic_proof_actual(
    horizontal_axis: f64,
    horizontal_zero: f64,
    vertical_zero: f64,
    vertical_axis: f64,
) -> (f64, f64, Option<f64>) {
    let cross = horizontal_axis * vertical_axis - horizontal_zero * vertical_zero;
    let dot = horizontal_axis * vertical_zero + horizontal_zero * vertical_axis;
    (cross, dot, deterministic_atan2_v1(cross.abs(), dot).ok())
}

#[test]
fn perpendicular_actual_classes_use_only_frozen_binary64_operations() {
    let right = deterministic_atan2_v1(1.0, 0.0).unwrap();
    for angle in [
        -0.0,
        0.0,
        90.0,
        180.0,
        f64::from_bits(1),
        f64::from_bits(0x39),
        90.0_f64.next_down(),
        90.0_f64.next_up(),
    ] {
        assert!(
            !fixed_angle_rejects_perpendicular_binary64_v1(angle),
            "{angle:?}"
        );
    }
    for angle in [1.0e-12, 45.0, 180.0_f64.next_down()] {
        assert!(
            fixed_angle_rejects_perpendicular_binary64_v1(angle),
            "{angle:?}"
        );
    }
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(right, 90.0_f64.next_down()),
        0.0,
        "the lower one-ULP degree remains a deterministic proof right angle"
    );
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(right, 90.0_f64.next_up()),
        0.0,
        "the upper one-ULP degree remains a deterministic proof right angle"
    );

    let (underflow_cross, underflow_dot, underflow_actual) =
        deterministic_proof_actual(f64::from_bits(1), 0.0, 0.0, 0.5);
    assert_eq!(underflow_cross, 0.0);
    assert_eq!(underflow_dot, 0.0);
    assert_eq!(
        underflow_actual.unwrap().to_bits(),
        deterministic_atan2_v1(0.0, 0.0).unwrap().to_bits()
    );

    let (overflow_cross, overflow_dot, overflow_actual) =
        deterministic_proof_actual(f64::MAX, 0.0, 0.0, 2.0);
    assert_eq!(overflow_cross, f64::INFINITY);
    assert_eq!(overflow_dot, 0.0);
    assert!(overflow_actual.is_none());

    let (_, nonfinite_dot, nonfinite_actual) =
        deterministic_proof_actual(f64::INFINITY, 0.0, 0.0, 1.0);
    assert!(nonfinite_dot.is_nan());
    assert!(nonfinite_actual.is_none());
    let (nonfinite_cross, _, cross_nan_actual) =
        deterministic_proof_actual(f64::INFINITY, 0.0, 0.0, 0.0);
    assert!(nonfinite_cross.is_nan());
    assert!(cross_nan_actual.is_none());

    let classes = [
        (0.0, 0.0),
        (0.0, -0.0),
        (f64::from_bits(1), 0.0),
        (f64::from_bits(1), -0.0),
        (1.0, 0.0),
        (1.0, -0.0),
        (f64::INFINITY, 0.0),
        (f64::INFINITY, -0.0),
        (f64::NAN, 0.0),
        (1.0, f64::NAN),
    ];
    let mut saw_zero = false;
    let mut saw_right = false;
    let mut saw_pi = false;
    let mut saw_nonfinite_error = false;
    for (horizontal_axis, vertical_axis) in [
        (0.0, 0.0),
        (0.0, 1.0),
        (1.0, 0.0),
        (1.0, 1.0),
        (-1.0, 1.0),
        (f64::from_bits(1), 0.5),
        (f64::MAX, 2.0),
        (f64::INFINITY, 1.0),
        (f64::INFINITY, 0.0),
    ] {
        for horizontal_zero in [-0.0, 0.0] {
            for vertical_zero in [-0.0, 0.0] {
                let (_, _, actual) = deterministic_proof_actual(
                    horizontal_axis,
                    horizontal_zero,
                    vertical_zero,
                    vertical_axis,
                );
                if let Some(actual) = actual {
                    assert!(classes.iter().any(|(absolute_cross, dot)| {
                        deterministic_atan2_v1(*absolute_cross, *dot)
                            .is_ok_and(|class| actual.to_bits() == class.to_bits())
                    }));
                    saw_zero |=
                        actual.to_bits() == deterministic_atan2_v1(0.0, 0.0).unwrap().to_bits();
                    saw_right |= actual.to_bits() == right.to_bits();
                    saw_pi |=
                        actual.to_bits() == deterministic_atan2_v1(0.0, -0.0).unwrap().to_bits();
                    let residual = deterministic_fixed_angle_residual_binary64_v1(actual, 45.0);
                    assert!(!residual.is_finite() || residual != 0.0);
                } else {
                    saw_nonfinite_error = true;
                }
            }
        }
    }
    assert!(saw_zero && saw_right && saw_pi && saw_nonfinite_error);
}

#[test]
fn only_angles_rejecting_zero_right_pi_and_nan_classes_are_promoted() {
    for reverse_roles in [false, true] {
        for angle in [1.0e-12, 45.0, 180.0_f64.next_down()] {
            let fixture = Fixture::new();
            let records = core_records(&fixture, reverse_roles, angle);
            let expected_ids = sorted_ids(records.iter().map(|item| item.id));
            assert_target(
                &prepare(&fixture, records).preflight(),
                &fixture,
                reverse_roles,
                &expected_ids,
            );
        }
        for angle in [
            -0.0,
            0.0,
            90.0,
            180.0,
            f64::from_bits(1),
            f64::from_bits(0x39),
            90.0_f64.next_down(),
            90.0_f64.next_up(),
        ] {
            let fixture = Fixture::new();
            let outcome =
                prepare(&fixture, core_records(&fixture, reverse_roles, angle)).preflight();
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
    let records = core_records(&fixture, false, 45.0);
    let expected_ids = sorted_ids(records.iter().map(|item| item.id));
    let expected = prepare(&fixture, records.clone()).preflight();
    assert_target(&expected, &fixture, false, &expected_ids);
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
    duplicated.extend(core_records(&fixture, false, 45.0));
    let horizontal = duplicated
        .iter()
        .filter_map(|item| match item.constraint {
            GeometricConstraintKindV1::Horizontal { edge } if edge == fixture.edges[0] => {
                Some(item.id)
            }
            _ => None,
        })
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let vertical = duplicated
        .iter()
        .filter_map(|item| match item.constraint {
            GeometricConstraintKindV1::Vertical { edge } if edge == fixture.edges[1] => {
                Some(item.id)
            }
            _ => None,
        })
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let angle = [duplicated[2].id, duplicated[5].id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let expected_ids = sorted_ids([horizontal, vertical, angle]);
    let expected = prepare(&fixture, duplicated.clone()).preflight();
    assert_target(&expected, &fixture, false, &expected_ids);
    duplicated.reverse();
    assert_eq!(prepare(&fixture, duplicated).preflight(), expected);
}

#[test]
fn exact_edge_vertex_roles_and_endpoint_directions_are_required() {
    for reverse_first in [false, true] {
        for reverse_second in [false, true] {
            for reverse_roles in [false, true] {
                let mut fixture = Fixture::new();
                if reverse_first {
                    let edge = &mut fixture.pattern.edges[0];
                    std::mem::swap(&mut edge.start, &mut edge.end);
                }
                if reverse_second {
                    let edge = &mut fixture.pattern.edges[1];
                    std::mem::swap(&mut edge.start, &mut edge.end);
                }
                let records = core_records(&fixture, reverse_roles, 45.0);
                let ids = sorted_ids(records.iter().map(|item| item.id));
                assert_target(
                    &prepare(&fixture, records).preflight(),
                    &fixture,
                    reverse_roles,
                    &ids,
                );
            }
        }
    }

    let fixture = Fixture::new();
    let angle = record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[0],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 45.0,
    });
    let alias = [
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[2],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[1],
        }),
        angle.clone(),
    ];
    assert!(!has_target(&prepare(&fixture, alias).preflight()));
    let wrong_pair = [
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[3],
            angle_degrees: 45.0,
        }),
    ];
    assert!(!has_target(&prepare(&fixture, wrong_pair).preflight()));

    let invalid_vertex = document([record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[4],
        first_edge: fixture.edges[0],
        second_edge: fixture.edges[1],
        angle_degrees: 45.0,
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
