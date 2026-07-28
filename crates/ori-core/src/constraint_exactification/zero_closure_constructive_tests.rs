use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};
use ori_numeric::deterministic_sin_cos_degrees_v1;

use super::zero_closure_constructive::{
    MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1,
    construct_zero_length_closure_residual_exact_assignment_v1,
};

#[derive(Clone)]
struct Fixture {
    pattern: CreasePattern,
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
}

impl Fixture {
    fn new() -> Self {
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
        Self {
            pattern: CreasePattern {
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
            },
            vertices,
            edges,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Provider {
    FixedLength,
    PointOnLine,
    Mirror,
    AngleBisector,
    Parallel,
    FixedAngle,
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

fn provider_terminal(fixture: &Fixture, provider: Provider) -> (EdgeId, GeometricConstraintKindV1) {
    match provider {
        Provider::FixedLength => (
            fixture.edges[0],
            GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            },
        ),
        Provider::PointOnLine => (
            fixture.edges[5],
            GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[5],
            },
        ),
        Provider::Mirror => (
            fixture.edges[0],
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[2],
                second_vertex: fixture.vertices[4],
                axis_edge: fixture.edges[0],
            },
        ),
        Provider::AngleBisector => (
            fixture.edges[0],
            GeometricConstraintKindV1::AngleBisector {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                bisector_edge: fixture.edges[2],
            },
        ),
        Provider::Parallel => (
            fixture.edges[0],
            GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            },
        ),
        Provider::FixedAngle => (
            fixture.edges[0],
            GeometricConstraintKindV1::FixedAngle {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
                angle_degrees: 90.0,
            },
        ),
    }
}

fn core(fixture: &Fixture, provider: Provider, ratio: bool) -> Vec<GeometricConstraintRecordV1> {
    let forced = fixture.edges[4];
    let (target, terminal) = provider_terminal(fixture, provider);
    vec![
        record(GeometricConstraintKindV1::Horizontal { edge: forced }),
        record(GeometricConstraintKindV1::Vertical { edge: forced }),
        record(if ratio {
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
        }),
        record(terminal),
    ]
}

#[test]
fn all_four_deletions_are_recertified_for_fixed_and_five_provider_terminals() {
    assert_eq!(MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1, 8);
    for provider in [
        Provider::FixedLength,
        Provider::PointOnLine,
        Provider::Mirror,
        Provider::AngleBisector,
        Provider::Parallel,
        Provider::FixedAngle,
    ] {
        for ratio in [false, true] {
            let fixture = Fixture::new();
            let records = core(&fixture, provider, ratio);
            for removed in 0..records.len() {
                let deletion = document(
                    records
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != removed)
                        .map(|(_, record)| record.clone()),
                );
                assert!(
                    construct_zero_length_closure_residual_exact_assignment_v1(
                        &fixture.pattern,
                        &deletion,
                    )
                    .is_some(),
                    "provider={provider:?}, ratio={ratio}, removed={removed}",
                );
            }
        }
    }
}

#[test]
fn fixed_angle_cardinal_and_adjacent_degree_bits_share_the_frozen_witness_kernel() {
    let cardinal = 90.0_f64;
    let below = f64::from_bits(cardinal.to_bits() - 1);
    let above = f64::from_bits(cardinal.to_bits() + 1);
    assert_eq!(deterministic_sin_cos_degrees_v1(cardinal), Ok((1.0, 0.0)));
    for angle_degrees in [below, cardinal, above] {
        let fixture = Fixture::new();
        let mut records = core(&fixture, Provider::FixedAngle, false);
        let GeometricConstraintKindV1::FixedAngle {
            angle_degrees: stored_angle,
            ..
        } = &mut records[3].constraint
        else {
            unreachable!("fixed-angle provider must remain the terminal record");
        };
        *stored_angle = angle_degrees;
        for removed in 0..records.len() {
            let deletion = document(
                records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != removed)
                    .map(|(_, record)| record.clone()),
            );
            assert!(
                construct_zero_length_closure_residual_exact_assignment_v1(
                    &fixture.pattern,
                    &deletion,
                )
                .is_some(),
                "angle_bits={:#018x}, removed={removed}",
                angle_degrees.to_bits(),
            );
        }
    }
}

#[test]
fn witnesses_ignore_storage_order_edge_direction_source_coordinates_and_signed_zero() {
    let fixture = Fixture::new();
    let records = core(&fixture, Provider::PointOnLine, true);
    for removed in 0..records.len() {
        let mut pattern = fixture.pattern.clone();
        pattern.vertices.reverse();
        pattern.edges.reverse();
        for (index, edge) in pattern.edges.iter_mut().enumerate() {
            if index % 2 == 0 {
                std::mem::swap(&mut edge.start, &mut edge.end);
            }
        }
        for (index, vertex) in pattern.vertices.iter_mut().enumerate() {
            vertex.position = if index == 0 {
                Point2::new(-0.0, 0.0)
            } else {
                Point2::new(index as f64 * 37.0, index as f64 * -19.0)
            };
        }
        let deletion = document(
            records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != removed)
                .map(|(_, record)| record.clone())
                .rev(),
        );
        let before_pattern = pattern.clone();
        let before_document = deletion.clone();
        assert!(
            construct_zero_length_closure_residual_exact_assignment_v1(&pattern, &deletion)
                .is_some(),
            "removed={removed}",
        );
        assert_eq!(pattern, before_pattern);
        assert_eq!(deletion, before_document);
    }
}

#[test]
fn shared_forced_endpoint_is_supported_without_weakening_recertification() {
    let mut fixture = Fixture::new();
    // PointOnLine's line edge now shares one endpoint with the forced edge.
    fixture.pattern.edges[5].end = fixture.pattern.edges[4].start;
    let records = core(&fixture, Provider::PointOnLine, false);
    for removed in 0..records.len() {
        let deletion = document(
            records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != removed)
                .map(|(_, record)| record.clone()),
        );
        assert!(
            construct_zero_length_closure_residual_exact_assignment_v1(
                &fixture.pattern,
                &deletion,
            )
            .is_some(),
            "removed={removed}",
        );
    }
}

#[test]
fn rejects_noncanonical_direction_alias_nonfinite_and_wrong_cardinality() {
    let fixture = Fixture::new();
    let forced = fixture.edges[4];
    let target = fixture.edges[0];
    let terminal = record(GeometricConstraintKindV1::FixedLength {
        edge: target,
        length_mm: 1.0,
    });
    let invalid_documents = [
        document([
            record(GeometricConstraintKindV1::Horizontal { edge: forced }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: forced,
                denominator_edge: target,
                ratio: 2.0,
            }),
            terminal.clone(),
        ]),
        document([
            record(GeometricConstraintKindV1::Horizontal { edge: forced }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: target,
                denominator_edge: forced,
                ratio: f64::NAN,
            }),
            terminal.clone(),
        ]),
        document([
            record(GeometricConstraintKindV1::Horizontal { edge: forced }),
            record(GeometricConstraintKindV1::Vertical { edge: forced }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: forced,
                second_edge: forced,
            }),
        ]),
    ];
    for invalid in invalid_documents {
        assert!(
            construct_zero_length_closure_residual_exact_assignment_v1(&fixture.pattern, &invalid,)
                .is_none(),
        );
    }
    assert!(
        construct_zero_length_closure_residual_exact_assignment_v1(
            &fixture.pattern,
            &document([terminal]),
        )
        .is_none(),
    );
}

#[test]
fn subnormal_and_extreme_ratio_seeds_are_accepted_only_after_exact_recertification() {
    let fixture = Fixture::new();
    let forced = fixture.edges[4];
    let target = fixture.edges[0];
    let minimum = f64::from_bits(1);
    for (length, ratio) in [(minimum, minimum), (f64::MAX, f64::MAX)] {
        let records = [
            record(GeometricConstraintKindV1::Horizontal { edge: forced }),
            record(GeometricConstraintKindV1::Vertical { edge: forced }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: target,
                denominator_edge: forced,
                ratio,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: target,
                length_mm: length,
            }),
        ];
        for removed in 0..records.len() {
            assert!(
                construct_zero_length_closure_residual_exact_assignment_v1(
                    &fixture.pattern,
                    &document(
                        records
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| *index != removed)
                            .map(|(_, record)| record.clone()),
                    ),
                )
                .is_some(),
                "length={length:?}, ratio={ratio:?}, removed={removed}",
            );
        }
    }

    // These inverse divisions underflow to zero or overflow to infinity. They
    // are candidate-generation failures, never proofs; the function closes to
    // None without weakening the production residual certificate.
    for (length, ratio) in [(minimum, f64::MAX), (f64::MAX, minimum)] {
        let deletion = document([
            record(GeometricConstraintKindV1::Horizontal { edge: forced }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: target,
                denominator_edge: forced,
                ratio,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: target,
                length_mm: length,
            }),
        ]);
        assert!(
            construct_zero_length_closure_residual_exact_assignment_v1(
                &fixture.pattern,
                &deletion,
            )
            .is_none(),
            "length={length:?}, ratio={ratio:?}",
        );
    }
}
