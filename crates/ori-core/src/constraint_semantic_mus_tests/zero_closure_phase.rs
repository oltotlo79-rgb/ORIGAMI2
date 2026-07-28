use super::*;
use crate::{ConstraintPreflightV1, DirectConstraintConflictKindV1};

#[derive(Clone, Copy, Debug)]
pub(super) enum Provider {
    FixedLength,
    PointOnLine,
    Mirror,
    AngleBisector,
    Parallel,
    FixedAngle,
}

pub(super) fn zero_closure_fixture(provider: Provider, ratio: bool) -> SemanticFixture {
    let vertices = (0..7).map(|_| VertexId::new()).collect::<Vec<_>>();
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 0.0),
        Point2::new(0.0, 1.0),
        Point2::new(-1.0, 0.0),
        Point2::new(0.0, -1.0),
        Point2::new(2.0, 0.0),
        Point2::new(2.0, 1.0),
    ];
    let endpoints = [(0, 1), (0, 2), (0, 3), (0, 4), (5, 6), (1, 5)];
    let edges = endpoints.iter().map(|_| EdgeId::new()).collect::<Vec<_>>();
    let pattern = CreasePattern {
        vertices: vertices
            .iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id: *id, position })
            .collect(),
        edges: endpoints
            .iter()
            .zip(&edges)
            .map(|((start, end), id)| Edge {
                id: *id,
                start: vertices[*start],
                end: vertices[*end],
                kind: EdgeKind::Auxiliary,
            })
            .collect(),
    };
    let (target, terminal) = match provider {
        Provider::FixedLength => (
            edges[0],
            GeometricConstraintKindV1::FixedLength {
                edge: edges[0],
                length_mm: 2.0,
            },
        ),
        Provider::PointOnLine => (
            edges[5],
            GeometricConstraintKindV1::PointOnLine {
                vertex: vertices[2],
                line_edge: edges[5],
            },
        ),
        Provider::Mirror => (
            edges[0],
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: vertices[2],
                second_vertex: vertices[3],
                axis_edge: edges[0],
            },
        ),
        Provider::AngleBisector => (
            edges[0],
            GeometricConstraintKindV1::AngleBisector {
                vertex: vertices[0],
                first_edge: edges[0],
                second_edge: edges[1],
                bisector_edge: edges[2],
            },
        ),
        Provider::Parallel => (
            edges[0],
            GeometricConstraintKindV1::Parallel {
                first_edge: edges[0],
                second_edge: edges[1],
            },
        ),
        Provider::FixedAngle => (
            edges[0],
            GeometricConstraintKindV1::FixedAngle {
                vertex: vertices[0],
                first_edge: edges[0],
                second_edge: edges[1],
                angle_degrees: 45.0,
            },
        ),
    };
    let forced = edges[4];
    let propagation = if ratio {
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge: target,
            denominator_edge: forced,
            ratio: 2.0,
        }
    } else {
        GeometricConstraintKindV1::EqualLength {
            first_edge: forced,
            second_edge: target,
        }
    };
    SemanticFixture {
        pattern,
        records: vec![
            record(GeometricConstraintKindV1::Horizontal { edge: forced }),
            record(GeometricConstraintKindV1::Vertical { edge: forced }),
            record(propagation),
            record(terminal),
        ],
    }
}

fn has_zero_closure_family(preflight: &ConstraintPreflightV1, provider: Provider) -> bool {
    let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
        return false;
    };
    conflicts.iter().any(|conflict| {
        matches!(
            (provider, conflict.conflict()),
            (
                Provider::FixedLength,
                DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
                    horizontal_constraint_count: 1,
                    vertical_constraint_count: 1,
                    zero_propagation_constraint_count: 1,
                    ..
                },
            ) | (
                Provider::PointOnLine
                    | Provider::Mirror
                    | Provider::AngleBisector
                    | Provider::Parallel
                    | Provider::FixedAngle,
                DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider {
                    horizontal_constraint_count: 1,
                    vertical_constraint_count: 1,
                    zero_propagation_constraint_count: 1,
                    ..
                },
            )
        )
    })
}

#[test]
fn fixed_and_five_provider_direct_families_are_promoted_for_equal_and_sound_ratio() {
    for provider in [
        Provider::FixedLength,
        Provider::PointOnLine,
        Provider::Mirror,
        Provider::AngleBisector,
        Provider::Parallel,
        Provider::FixedAngle,
    ] {
        for ratio in [false, true] {
            let fixture = zero_closure_fixture(provider, ratio);
            let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
            assert!(
                has_zero_closure_family(&prepared.preflight(), provider),
                "provider={provider:?}, ratio={ratio}",
            );
            let expected = sorted_ids(fixture.records.iter().cloned());
            let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
            assert_eq!(certificate.constraint_ids(), expected);
            assert_eq!(certificate.deletion_witness_checks(), 4);
            assert_eq!(certificate.current_assignment_witness_count(), 0);
            assert_eq!(certificate.axis_exactification_witness_count(), 0);
            assert_eq!(
                certificate.single_constraint_constructive_witness_count(),
                0,
            );
            assert_eq!(certificate.pair_constraint_constructive_witness_count(), 0);
            assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 0);
            assert_eq!(
                certificate.length_constraint_constructive_witness_count(),
                0,
            );
            assert_eq!(
                certificate.zero_length_closure_constructive_witness_count(),
                4,
            );
        }
    }
}

#[test]
fn order_direction_and_shared_endpoint_variants_remain_semantic_mus() {
    let mut fixture = zero_closure_fixture(Provider::PointOnLine, true);
    fixture.pattern.edges[5].end = fixture.pattern.edges[4].start;
    fixture.pattern.vertices.reverse();
    fixture.pattern.edges.reverse();
    for (index, edge) in fixture.pattern.edges.iter_mut().enumerate() {
        if index % 2 == 0 {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
    }
    fixture.records.reverse();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    assert_eq!(
        certificate.zero_length_closure_constructive_witness_count(),
        4,
    );
}
