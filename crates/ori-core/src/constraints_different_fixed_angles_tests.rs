use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::{
    ConstraintPreflightV1, DirectConstraintConflictKindV1, GeometricConstraintLimitsV1,
    GeometricConstraintUnknownReasonV1, deterministic_fixed_angle_residual_binary64_v1,
    fixed_angle_zero_actual_enclosure_v1, prepare_geometric_constraints_v1,
};
use crate::{
    ConstraintSolveErrorV1, ConstraintSolveLimitsV1, solve_geometric_constraints_v1,
    verify_geometric_constraint_solution_v1,
};

struct Fixture {
    pattern: CreasePattern,
    vertex: VertexId,
    first_edge: EdgeId,
    second_edge: EdgeId,
}

fn fixture() -> Fixture {
    let vertex = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    Fixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: first_end,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: second_end,
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![
                Edge {
                    id: first_edge,
                    start: vertex,
                    end: first_end,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second_edge,
                    start: vertex,
                    end: second_end,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        vertex,
        first_edge,
        second_edge,
    }
}

fn fixed_angle(fixture: &Fixture, angle_degrees: f64) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint: GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertex,
            first_edge: fixture.first_edge,
            second_edge: fixture.second_edge,
            angle_degrees,
        },
    }
}

fn document(
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
    }
}

fn preflight(
    fixture: &Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> ConstraintPreflightV1 {
    let document = document(records);
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("fixed-angle fixture must prepare")
    .preflight()
    .clone()
}

#[test]
fn disjoint_zero_residual_enclosures_promote_different_fixed_angles() {
    let fixture = fixture();
    let first = fixed_angle(&fixture, 30.0);
    let second = fixed_angle(&fixture, 120.0);
    let expected_ids = {
        let mut ids = vec![first.id, second.id];
        ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
        ids
    };
    let outcome = preflight(&fixture, [second.clone(), first.clone()]);
    let expected_outcome = outcome.clone();
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        panic!("widely separated fixed angles must have no shared exact-zero residual");
    };
    assert_eq!(conflicts.len(), 1);
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::DifferentFixedAngles {
            vertex,
            first_edge,
            second_edge,
        } if *vertex == fixture.vertex
            && [*first_edge, *second_edge].contains(&fixture.first_edge)
            && [*first_edge, *second_edge].contains(&fixture.second_edge)
    ));
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);

    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.vertices.reverse();
    reversed_pattern.edges.reverse();
    let reversed_fixture = Fixture {
        pattern: reversed_pattern,
        ..fixture
    };
    let mut first_reversed = first;
    let GeometricConstraintKindV1::FixedAngle {
        first_edge,
        second_edge,
        ..
    } = &mut first_reversed.constraint
    else {
        unreachable!()
    };
    std::mem::swap(first_edge, second_edge);
    assert_eq!(
        preflight(&reversed_fixture, [first_reversed, second]),
        expected_outcome,
    );
}

#[test]
fn solver_and_verifier_reject_the_proven_pair_before_tolerance() {
    let fixture = fixture();
    let document = document([fixed_angle(&fixture, 30.0), fixed_angle(&fixture, 120.0)]);
    assert_eq!(
        solve_geometric_constraints_v1(
            &fixture.pattern,
            &document,
            fixture.vertex,
            Point2::new(0.0, 0.0),
            ConstraintSolveLimitsV1 {
                residual_tolerance: 1.0e300,
                ..ConstraintSolveLimitsV1::default()
            },
        ),
        Err(ConstraintSolveErrorV1::NonConvergent),
    );
    assert_eq!(
        verify_geometric_constraint_solution_v1(&fixture.pattern, &document, 1.0e300),
        Err(ConstraintSolveErrorV1::NonConvergent),
    );
}

#[test]
fn rounded_aliases_and_overlapping_enclosures_remain_solver_required() {
    let fixture = fixture();
    for angles in [
        [f64::from_bits(1), f64::from_bits(2)],
        [90.0, 90.0_f64.next_up()],
        [0.0, -0.0],
    ] {
        let records = angles.map(|angle| fixed_angle(&fixture, angle));
        let ids = {
            let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
            ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
            ids
        };
        assert_eq!(
            preflight(&fixture, records),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: ids,
            },
            "an enclosure overlap is intentionally a fail-closed negative",
        );
    }
}

#[test]
fn extrema_select_the_same_two_record_witness_with_duplicates_and_reordering() {
    let fixture = fixture();
    let low = fixed_angle(&fixture, 30.0);
    let low_duplicate = fixed_angle(&fixture, 30.0);
    let middle = fixed_angle(&fixture, 90.0);
    let high = fixed_angle(&fixture, 150.0);
    let expected_low = [low.id, low_duplicate.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("two low assignments");
    let expected = preflight(
        &fixture,
        [
            low.clone(),
            low_duplicate.clone(),
            middle.clone(),
            high.clone(),
        ],
    );
    let ConstraintPreflightV1::DirectConflict { conflicts } = &expected else {
        panic!("the global fixed-angle extrema must be incompatible");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids().len(), 2);
    assert!(conflicts[0].constraint_ids().contains(&expected_low));
    assert!(conflicts[0].constraint_ids().contains(&high.id));
    assert_eq!(
        preflight(&fixture, [high, middle, low_duplicate, low]),
        expected,
    );
}

#[test]
fn enclosure_contains_every_sampled_exact_zero_actual() {
    for angle in [
        0.0,
        f64::from_bits(1),
        1.0e-12,
        30.0,
        90.0,
        120.0,
        180.0_f64.next_down(),
        180.0,
    ] {
        let (lower, upper) = fixed_angle_zero_actual_enclosure_v1(angle)
            .expect("every validated fixed angle must have a conservative enclosure");
        let expected =
            ori_numeric::deterministic_degrees_to_radians_v1(angle).expect("validated test angle");
        let mut samples = vec![0.0, std::f64::consts::PI, expected];
        let mut below = expected;
        let mut above = expected;
        for _ in 0..64 {
            below = below.next_down();
            above = above.next_up();
            samples.extend([below.max(0.0), above.min(std::f64::consts::PI)]);
        }
        for actual in samples {
            if deterministic_fixed_angle_residual_binary64_v1(actual, angle) == 0.0 {
                assert!(
                    (lower..=upper).contains(&actual),
                    "zero residual escaped enclosure: angle={angle:?}, actual={actual:?}, \
                     enclosure=({lower:?}, {upper:?})",
                );
            }
        }
    }
}
