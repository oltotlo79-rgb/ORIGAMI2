use std::collections::BTreeMap;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::construct_pair_constraint_exact_assignment_v1;
use crate::{ConstraintSolveErrorV1, certify_binary64_exact_geometric_constraint_satisfaction_v1};

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintKindV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().map(record).collect(),
    }
}

fn shared_edge_pattern() -> (CreasePattern, EdgeId, EdgeId, VertexId) {
    let center = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first = EdgeId::new();
    let second = EdgeId::new();
    (
        CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(10.0, 10.0),
                },
                Vertex {
                    id: first_end,
                    position: Point2::new(13.0, 11.0),
                },
                Vertex {
                    id: second_end,
                    position: Point2::new(9.0, 14.0),
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
                    start: center,
                    end: second_end,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        first,
        second,
        center,
    )
}

fn assert_pair_is_recertified(pattern: &CreasePattern, document: &GeometricConstraintDocumentV1) {
    let assignment = construct_pair_constraint_exact_assignment_v1(pattern, document)
        .unwrap_or_else(|| panic!("expected pair template for {document:?}"));
    assert_eq!(assignment.certificate().constraint_count(), 2);
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            assignment.pattern(),
            document,
        )
        .expect("constructed pattern must remain structurally valid")
        .is_some(),
    );
}

#[test]
fn supported_pair_templates_are_all_recertified_by_the_complete_production_api() {
    let (pattern, first, second, center) = shared_edge_pattern();
    let cases = [
        document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            },
            GeometricConstraintKindV1::Horizontal { edge: first },
        ]),
        document([
            GeometricConstraintKindV1::Vertical { edge: first },
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 3.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: 3.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: 2.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: 2.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 4.0,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::Horizontal { edge: first },
            GeometricConstraintKindV1::Vertical { edge: second },
        ]),
        document([
            GeometricConstraintKindV1::Horizontal { edge: first },
            GeometricConstraintKindV1::Horizontal { edge: second },
        ]),
        document([
            GeometricConstraintKindV1::Horizontal { edge: first },
            GeometricConstraintKindV1::Horizontal { edge: first },
        ]),
        document([
            GeometricConstraintKindV1::Parallel {
                first_edge: first,
                second_edge: second,
            },
            GeometricConstraintKindV1::Vertical { edge: second },
        ]),
        document([
            GeometricConstraintKindV1::FixedAngle {
                vertex: center,
                first_edge: first,
                second_edge: second,
                angle_degrees: 45.0,
            },
            GeometricConstraintKindV1::Horizontal { edge: second },
        ]),
    ];
    for case in cases {
        assert_pair_is_recertified(&pattern, &case);
    }

    let mut disjoint = pattern.clone();
    let detached = VertexId::new();
    disjoint.vertices.push(Vertex {
        id: detached,
        position: Point2::new(8.0, 9.0),
    });
    disjoint.edges[1].end = detached;
    assert_pair_is_recertified(
        &disjoint,
        &document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: 3.0,
            },
        ]),
    );
}

#[test]
fn assignment_is_invariant_to_document_pattern_and_edge_storage_orders() {
    let (pattern, first, second, _) = shared_edge_pattern();
    let source = document([
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: second,
            ratio: 2.0,
        },
        GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 4.0,
        },
    ]);
    let expected = construct_pair_constraint_exact_assignment_v1(&pattern, &source)
        .expect("forward assignment");

    let mut reversed_pattern = pattern.clone();
    reversed_pattern.vertices.reverse();
    reversed_pattern.edges.reverse();
    for edge in &mut reversed_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let mut reversed_document = source.clone();
    reversed_document.constraints.reverse();
    let reversed =
        construct_pair_constraint_exact_assignment_v1(&reversed_pattern, &reversed_document)
            .expect("reversed assignment");

    let positions = |candidate: &CreasePattern| {
        candidate
            .vertices
            .iter()
            .map(|vertex| (vertex.id.canonical_bytes(), vertex.position))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(positions(expected.pattern()), positions(reversed.pattern()));
}

#[path = "pair_constructive_tests/fail_closed.rs"]
mod fail_closed;
