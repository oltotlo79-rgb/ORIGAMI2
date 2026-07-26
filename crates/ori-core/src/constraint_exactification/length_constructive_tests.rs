use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::length_constructive::{
    MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1,
    construct_length_constraint_residual_exact_assignment_v1,
};

#[derive(Clone)]
struct MatchingFixture {
    pattern: CreasePattern,
    edges: Vec<EdgeId>,
}

impl MatchingFixture {
    fn new(edge_count: usize) -> Self {
        let mut vertices = Vec::with_capacity(edge_count * 2);
        let mut edges = Vec::with_capacity(edge_count);
        let mut edge_records = Vec::with_capacity(edge_count);
        for index in 0..edge_count {
            let start = VertexId::new();
            let end = VertexId::new();
            let edge = EdgeId::new();
            vertices.push(Vertex {
                id: start,
                position: Point2::new(index as f64 * 3.0, 1.0),
            });
            vertices.push(Vertex {
                id: end,
                position: Point2::new(index as f64 * 3.0 + 1.0, 2.0),
            });
            edges.push(edge);
            edge_records.push(Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Auxiliary,
            });
        }
        Self {
            pattern: CreasePattern {
                vertices,
                edges: edge_records,
            },
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

#[test]
fn rooted_rootless_reverse_and_underflow_candidates_are_fully_recertified() {
    let fixture = MatchingFixture::new(4);
    let [first, second, third, fourth] = fixture.edges[..] else {
        unreachable!()
    };
    let minimum = f64::from_bits(1);
    let documents = [
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: third,
                denominator_edge: second,
                ratio: 3.0,
            }),
        ]),
        document([
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: second,
                denominator_edge: third,
                ratio: 3.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: third,
                denominator_edge: first,
                ratio: 0.25,
            }),
        ]),
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: third,
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: third,
                denominator_edge: fourth,
                ratio: 2.0,
            }),
        ]),
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: minimum,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: minimum,
            }),
        ]),
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            }),
        ]),
    ];
    for document in documents {
        assert!(
            construct_length_constraint_residual_exact_assignment_v1(&fixture.pattern, &document,)
                .is_some(),
            "{document:?}",
        );
    }
}

#[test]
fn result_is_invariant_to_pattern_document_and_edge_storage_direction() {
    let fixture = MatchingFixture::new(3);
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[2],
            denominator_edge: fixture.edges[1],
            ratio: 3.0,
        }),
    ];
    let original_pattern = fixture.pattern.clone();
    let original_document = document(records.clone());
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(
            &fixture.pattern,
            &original_document,
        )
        .is_some(),
    );
    assert_eq!(fixture.pattern, original_pattern);

    let mut reordered_pattern = fixture.pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    for edge in &mut reordered_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    for (index, vertex) in reordered_pattern.vertices.iter_mut().enumerate() {
        vertex.position = Point2::new(index as f64 * 101.0, index as f64 * -37.0 - 5.0);
    }
    let reordered_document = document(records.into_iter().rev());
    let document_before = reordered_document.clone();
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(
            &reordered_pattern,
            &reordered_document,
        )
        .is_some(),
    );
    assert_eq!(reordered_document, document_before);
}

#[test]
fn rootless_zero_is_invariant_to_signed_zero_source_and_constraint_order() {
    let mut fixture = MatchingFixture::new(2);
    fixture.pattern.vertices[0].position = Point2::new(-0.0, -0.0);
    let records = vec![
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
    ];
    for records in [records.clone(), records.into_iter().rev().collect()] {
        assert!(
            construct_length_constraint_residual_exact_assignment_v1(
                &fixture.pattern,
                &document(records),
            )
            .is_some(),
        );
    }
}

#[test]
fn exact_storage_boundary_is_admitted() {
    let constraint_count = MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1;
    let fixture = MatchingFixture::new(constraint_count * 2);
    let records = (0..constraint_count).map(|index| {
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[index * 2],
            second_edge: fixture.edges[index * 2 + 1],
        })
    });
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(
            &fixture.pattern,
            &document(records),
        )
        .is_some(),
    );
}

#[path = "length_constructive_tests/fail_closed.rs"]
mod fail_closed;
