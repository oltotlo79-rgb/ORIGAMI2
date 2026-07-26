use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::{
    construct_pair_constraint_algebraic_exact_assignment_v1,
    construct_pair_constraint_exact_assignment_v1,
};

struct PairFixture {
    pattern: CreasePattern,
    first: EdgeId,
    second: EdgeId,
    point: VertexId,
}

fn fixture(shared: bool) -> PairFixture {
    let vertices = std::array::from_fn::<_, 5, _>(|_| VertexId::new());
    let first = EdgeId::new();
    let second = EdgeId::new();
    PairFixture {
        pattern: CreasePattern {
            vertices: vertices
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(3.0, 1.0),
                    Point2::new(5.0, 2.0),
                    Point2::new(7.0, 6.0),
                    Point2::new(1.0, 4.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: vec![
                Edge {
                    id: first,
                    start: vertices[0],
                    end: vertices[1],
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second,
                    start: vertices[3],
                    end: if shared { vertices[0] } else { vertices[2] },
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        first,
        second,
        point: vertices[4],
    }
}

fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintKindV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints
            .into_iter()
            .map(|constraint| GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            })
            .collect(),
    }
}

fn ratio(
    numerator_edge: EdgeId,
    denominator_edge: EdgeId,
    ratio: f64,
) -> GeometricConstraintKindV1 {
    GeometricConstraintKindV1::LengthRatio {
        numerator_edge,
        denominator_edge,
        ratio,
    }
}

#[test]
fn supported_zero_length_relations_are_recertified_for_shared_and_disjoint_edges() {
    for shared in [false, true] {
        let fixture = fixture(shared);
        let cases = [
            document([
                ratio(fixture.first, fixture.second, 2.0),
                ratio(fixture.first, fixture.second, 3.0),
            ]),
            document([
                ratio(fixture.first, fixture.second, 2.0),
                ratio(fixture.second, fixture.first, 2.0),
            ]),
            document([
                GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.first,
                    second_edge: fixture.second,
                },
                ratio(fixture.second, fixture.first, 2.0),
            ]),
        ];
        for case in cases {
            assert!(
                construct_pair_constraint_exact_assignment_v1(&fixture.pattern, &case).is_none(),
                "the non-degenerate pair language must fail before algebraic fallback",
            );
            assert!(
                construct_pair_constraint_algebraic_exact_assignment_v1(&fixture.pattern, &case)
                    .is_some(),
            );
        }

        let orientations = document([
            GeometricConstraintKindV1::Vertical {
                edge: fixture.first,
            },
            GeometricConstraintKindV1::Horizontal {
                edge: fixture.first,
            },
        ]);
        assert!(
            construct_pair_constraint_algebraic_exact_assignment_v1(
                &fixture.pattern,
                &orientations,
            )
            .is_some(),
        );
    }
}

#[test]
fn algebraic_pair_is_invariant_to_input_storage_and_edge_directions() {
    let fixture = fixture(false);
    let source = document([
        GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.second,
            second_edge: fixture.first,
        },
        ratio(fixture.first, fixture.second, 2.0),
    ]);
    assert!(
        construct_pair_constraint_algebraic_exact_assignment_v1(&fixture.pattern, &source)
            .is_some()
    );

    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.vertices.reverse();
    reversed_pattern.edges.reverse();
    for edge in &mut reversed_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let mut reversed_document = source;
    reversed_document.constraints.reverse();
    assert!(
        construct_pair_constraint_algebraic_exact_assignment_v1(
            &reversed_pattern,
            &reversed_document,
        )
        .is_some(),
    );
}

#[test]
fn unsupported_normalized_unrelated_invalid_and_subnormal_pairs_fail_closed() {
    let fixture = fixture(false);
    let rejected = [
        document([
            ratio(fixture.first, fixture.second, f64::from_bits(1)),
            ratio(fixture.first, fixture.second, 2.0),
        ]),
        document([
            GeometricConstraintKindV1::Horizontal {
                edge: fixture.first,
            },
            GeometricConstraintKindV1::Vertical {
                edge: fixture.second,
            },
        ]),
        document([
            GeometricConstraintKindV1::Parallel {
                first_edge: fixture.first,
                second_edge: fixture.second,
            },
            GeometricConstraintKindV1::Horizontal {
                edge: fixture.first,
            },
        ]),
        document([
            GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.point,
                line_edge: fixture.first,
            },
            GeometricConstraintKindV1::Horizontal {
                edge: fixture.first,
            },
        ]),
        document([
            ratio(fixture.first, EdgeId::new(), 2.0),
            ratio(fixture.first, fixture.second, 3.0),
        ]),
    ];
    for document in rejected {
        assert!(
            construct_pair_constraint_algebraic_exact_assignment_v1(&fixture.pattern, &document,)
                .is_none(),
        );
    }
    assert!(
        construct_pair_constraint_algebraic_exact_assignment_v1(
            &fixture.pattern,
            &document([ratio(fixture.first, fixture.second, 2.0)]),
        )
        .is_none(),
    );
}
