use super::*;
use crate::{ConstraintPreflightV1, DirectConstraintConflictKindV1};

#[derive(Clone, Copy)]
pub(super) enum Family {
    EqualDifferentFixed,
    RatioIncompatibleFixed,
    ParallelPerpendicular,
    SameOrientationAngle,
    PerpendicularAngle,
    HorizontalVertical,
    DifferentRatios,
    EqualNonUnitRatio,
    NonReciprocalRatios,
}

fn two_edge_fixture(
    constraints: impl FnOnce(EdgeId, EdgeId, VertexId) -> Vec<GeometricConstraintRecordV1>,
) -> SemanticFixture {
    let center = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first = EdgeId::new();
    let second = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: first_end,
                    position: Point2::new(3.0, 1.0),
                },
                Vertex {
                    id: second_end,
                    position: Point2::new(-1.0, 4.0),
                },
            ],
            edges: vec![
                Edge {
                    id: first,
                    start: center,
                    end: first_end,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second,
                    start: second_end,
                    end: center,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        records: constraints(first, second, center),
    }
}

pub(super) fn promoted_fixtures() -> Vec<(Family, SemanticFixture)> {
    vec![
        (
            Family::EqualDifferentFixed,
            two_edge_fixture(|first, second, _| {
                vec![
                    record(GeometricConstraintKindV1::EqualLength {
                        first_edge: first,
                        second_edge: second,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: first,
                        length_mm: 2.0,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: second,
                        length_mm: 3.0,
                    }),
                ]
            }),
        ),
        (
            Family::RatioIncompatibleFixed,
            two_edge_fixture(|first, second, _| {
                vec![
                    record(GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first,
                        denominator_edge: second,
                        ratio: 2.0,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: first,
                        length_mm: 3.0,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: second,
                        length_mm: 1.0,
                    }),
                ]
            }),
        ),
        (
            Family::ParallelPerpendicular,
            two_edge_fixture(|first, second, _| {
                vec![
                    record(GeometricConstraintKindV1::Parallel {
                        first_edge: first,
                        second_edge: second,
                    }),
                    record(GeometricConstraintKindV1::Horizontal { edge: first }),
                    record(GeometricConstraintKindV1::Vertical { edge: second }),
                ]
            }),
        ),
        (
            Family::SameOrientationAngle,
            two_edge_fixture(|first, second, center| {
                vec![
                    record(GeometricConstraintKindV1::Horizontal { edge: first }),
                    record(GeometricConstraintKindV1::Horizontal { edge: second }),
                    record(GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge: second,
                        second_edge: first,
                        angle_degrees: 90.0,
                    }),
                ]
            }),
        ),
        (
            Family::PerpendicularAngle,
            two_edge_fixture(|first, second, center| {
                vec![
                    record(GeometricConstraintKindV1::Horizontal { edge: first }),
                    record(GeometricConstraintKindV1::Vertical { edge: second }),
                    record(GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge: second,
                        second_edge: first,
                        angle_degrees: 45.0,
                    }),
                ]
            }),
        ),
    ]
}

pub(super) fn pair_work_fixture() -> SemanticFixture {
    promoted_fixtures().remove(0).1
}

pub(super) fn algebraic_pair_fixtures() -> Vec<(Family, SemanticFixture)> {
    vec![
        (
            Family::HorizontalVertical,
            two_edge_fixture(|edge, _, _| {
                vec![
                    record(GeometricConstraintKindV1::Horizontal { edge }),
                    record(GeometricConstraintKindV1::Vertical { edge }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge,
                        length_mm: 2.0,
                    }),
                ]
            }),
        ),
        (
            Family::DifferentRatios,
            two_edge_fixture(|first, second, _| {
                vec![
                    record(GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first,
                        denominator_edge: second,
                        ratio: 2.0,
                    }),
                    record(GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first,
                        denominator_edge: second,
                        ratio: 3.0,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: second,
                        length_mm: 1.0,
                    }),
                ]
            }),
        ),
        (
            Family::EqualNonUnitRatio,
            two_edge_fixture(|first, second, _| {
                vec![
                    record(GeometricConstraintKindV1::EqualLength {
                        first_edge: first,
                        second_edge: second,
                    }),
                    record(GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first,
                        denominator_edge: second,
                        ratio: 2.0,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: first,
                        length_mm: 1.0,
                    }),
                ]
            }),
        ),
        (
            Family::NonReciprocalRatios,
            two_edge_fixture(|first, second, _| {
                vec![
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
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: first,
                        length_mm: 1.0,
                    }),
                ]
            }),
        ),
    ]
}

pub(super) fn algebraic_work_fixture() -> SemanticFixture {
    algebraic_pair_fixtures().remove(0).1
}

fn has_family(preflight: &ConstraintPreflightV1, family: Family) -> bool {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        return false;
    };
    conflicts.iter().any(|conflict| {
        matches!(
            (family, conflict.conflict()),
            (
                Family::EqualDifferentFixed,
                DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths { .. }
            ) | (
                Family::RatioIncompatibleFixed,
                DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths { .. }
            ) | (
                Family::ParallelPerpendicular,
                DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations { .. }
            ) | (
                Family::SameOrientationAngle,
                DirectConstraintConflictKindV1::SameOrientationWithFixedNonParallelAngle { .. }
            ) | (
                Family::PerpendicularAngle,
                DirectConstraintConflictKindV1::PerpendicularOrientationsWithFixedNonRightAngle {
                    ..
                }
            ) | (
                Family::HorizontalVertical,
                DirectConstraintConflictKindV1::HorizontalAndVertical { .. }
            ) | (
                Family::DifferentRatios,
                DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
            ) | (
                Family::EqualNonUnitRatio,
                DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength { .. }
            ) | (
                Family::NonReciprocalRatios,
                DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength { .. }
            )
        )
    })
}

#[test]
fn pair_language_promotes_five_constructive_direct_families() {
    let fixtures = promoted_fixtures();
    assert_eq!(fixtures.len(), 5);
    for (family, fixture) in fixtures {
        let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
        assert!(has_family(&prepared.preflight(), family));
        assert!(matches!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::ProvenUnsatisfiable {
                ref constraint_ids,
                oracle_calls: 7,
            } if constraint_ids.len() == 3
        ));
        let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
        assert_eq!(certificate.constraint_ids().len(), 3);
        assert!(certificate.pair_constraint_constructive_witness_count() >= 2);
        assert_eq!(
            certificate.current_assignment_witness_count()
                + certificate.axis_exactification_witness_count()
                + certificate.single_constraint_constructive_witness_count()
                + certificate.pair_constraint_constructive_witness_count()
                + certificate.pair_constraint_algebraic_witness_count()
                + certificate.length_constraint_constructive_witness_count()
                + certificate.zero_length_closure_constructive_witness_count(),
            3,
        );
        assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 0);
        assert_eq!(
            certificate.length_constraint_constructive_witness_count(),
            0
        );
    }
}

#[test]
fn algebraic_pair_promotes_four_additional_three_record_families() {
    let fixtures = algebraic_pair_fixtures();
    assert_eq!(fixtures.len(), 4);
    for (family, fixture) in fixtures {
        let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
        assert!(has_family(&prepared.preflight(), family));
        let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
        assert_eq!(certificate.constraint_ids().len(), 3);
        assert_eq!(certificate.current_assignment_witness_count(), 0);
        assert_eq!(certificate.axis_exactification_witness_count(), 0);
        assert_eq!(
            certificate.single_constraint_constructive_witness_count(),
            0,
        );
        assert_eq!(certificate.pair_constraint_constructive_witness_count(), 2);
        assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 1);
        assert_eq!(
            certificate.length_constraint_constructive_witness_count(),
            0
        );
    }
}
