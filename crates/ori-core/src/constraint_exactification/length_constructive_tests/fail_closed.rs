use super::*;
use crate::constraint_exactification::length_constructive::construct_length_constraint_residual_exact_assignment_with_pass_limit_for_test;

#[test]
fn unsupported_shared_invalid_and_over_storage_documents_fail_closed() {
    let fixture = MatchingFixture::new(2);
    let unsupported = document([record(GeometricConstraintKindV1::Horizontal {
        edge: fixture.edges[0],
    })]);
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(&fixture.pattern, &unsupported)
            .is_none(),
    );

    let shared_start = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first = EdgeId::new();
    let second = EdgeId::new();
    let shared_pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: shared_start,
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
                id: first,
                start: shared_start,
                end: first_end,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: second,
                start: shared_start,
                end: second_end,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let shared = document([record(GeometricConstraintKindV1::EqualLength {
        first_edge: first,
        second_edge: second,
    })]);
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(&shared_pattern, &shared)
            .is_none(),
    );

    let oversized = MatchingFixture::new(MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1 + 1);
    let records = oversized.edges.iter().map(|edge| {
        record(GeometricConstraintKindV1::FixedLength {
            edge: *edge,
            length_mm: 1.0,
        })
    });
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(
            &oversized.pattern,
            &document(records),
        )
        .is_none(),
    );
}

#[test]
fn nonfinite_propagation_bit_mismatch_and_iteration_exhaustion_fail_closed() {
    let fixture = MatchingFixture::new(2);
    let [first, second] = fixture.edges[..] else {
        unreachable!()
    };
    for document in [
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: second,
                length_mm: f64::MAX,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            }),
        ]),
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: f64::MAX,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: f64::from_bits(1),
            }),
        ]),
        document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 1.0_f64.next_down(),
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 3.0,
            }),
        ]),
    ] {
        assert!(
            construct_length_constraint_residual_exact_assignment_v1(&fixture.pattern, &document,)
                .is_none(),
        );
    }

    let chain = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: first,
            second_edge: second,
        }),
    ]);
    assert!(
        construct_length_constraint_residual_exact_assignment_with_pass_limit_for_test(
            &fixture.pattern,
            &chain,
            0,
        )
        .is_none(),
    );
}

#[test]
fn multiple_incompatible_roots_and_invalid_source_are_never_witnesses() {
    let fixture = MatchingFixture::new(2);
    let incompatible = document([
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        }),
    ]);
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(&fixture.pattern, &incompatible,)
            .is_none(),
    );

    let mut invalid_pattern = fixture.pattern.clone();
    invalid_pattern.vertices[0].position.x = f64::NAN;
    let valid = document([record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    })]);
    assert!(
        construct_length_constraint_residual_exact_assignment_v1(&invalid_pattern, &valid)
            .is_none(),
    );
}
