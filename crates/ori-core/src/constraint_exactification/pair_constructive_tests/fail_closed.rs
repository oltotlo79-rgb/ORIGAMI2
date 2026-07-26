use super::*;

#[test]
fn invalid_subnormal_conflicting_and_unsupported_pairs_fail_closed() {
    let (pattern, first, second, center) = shared_edge_pattern();
    for invalid_length in [f64::from_bits(1), f64::NAN, f64::INFINITY] {
        let invalid = document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: invalid_length,
            },
            GeometricConstraintKindV1::Horizontal { edge: first },
        ]);
        assert!(construct_pair_constraint_exact_assignment_v1(&pattern, &invalid).is_none());
    }
    for invalid in [
        document([
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: f64::from_bits(1),
            },
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 1.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::FixedAngle {
                vertex: center,
                first_edge: first,
                second_edge: second,
                angle_degrees: f64::from_bits(1),
            },
            GeometricConstraintKindV1::Horizontal { edge: first },
        ]),
        document([
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 1.0,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: first,
                length_mm: 2.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::Horizontal { edge: first },
            GeometricConstraintKindV1::Vertical { edge: first },
        ]),
        document([
            GeometricConstraintKindV1::FixedLength {
                edge: EdgeId::new(),
                length_mm: 1.0,
            },
            GeometricConstraintKindV1::Horizontal { edge: first },
        ]),
    ] {
        assert!(
            construct_pair_constraint_exact_assignment_v1(&pattern, &invalid).is_none(),
            "{invalid:?}",
        );
    }
    assert!(
        construct_pair_constraint_exact_assignment_v1(
            &pattern,
            &document([GeometricConstraintKindV1::Horizontal { edge: first }]),
        )
        .is_none(),
    );
}

#[test]
fn algebraic_zero_length_escapes_are_rejected_by_production_geometry_preflight() {
    let (pattern, first, second, _) = shared_edge_pattern();
    let escape_documents = [
        document([
            GeometricConstraintKindV1::Horizontal { edge: first },
            GeometricConstraintKindV1::Vertical { edge: first },
        ]),
        document([
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 3.0,
            },
        ]),
        document([
            GeometricConstraintKindV1::EqualLength {
                first_edge: first,
                second_edge: second,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first,
                denominator_edge: second,
                ratio: 2.0,
            },
        ]),
    ];
    let mut collapsed = pattern.clone();
    let shared = collapsed.edges[0].start;
    let collapsed_position = collapsed
        .vertices
        .iter()
        .find(|vertex| vertex.id == shared)
        .expect("shared vertex")
        .position;
    for vertex in &mut collapsed.vertices {
        vertex.position = collapsed_position;
    }
    for escape in escape_documents {
        assert_eq!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(&collapsed, &escape),
            Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry),
        );
        assert!(construct_pair_constraint_exact_assignment_v1(&pattern, &escape).is_none());
    }
}

#[test]
fn all_four_fixed_translations_fail_closed_when_incident_edges_would_collapse() {
    let (mut pattern, first, _, _) = shared_edge_pattern();
    let [_, moved] = {
        let edge = pattern.edges.iter().find(|edge| edge.id == first).unwrap();
        let mut endpoints = [edge.start, edge.end];
        endpoints.sort_unstable_by_key(VertexId::canonical_bytes);
        endpoints
    };
    for point in [
        Point2::new(2.0, 0.0),
        Point2::new(18.0, 32.0),
        Point2::new(-14.0, 8.0),
        Point2::new(1026.0, -512.0),
    ] {
        let blocker = VertexId::new();
        pattern.vertices.push(Vertex {
            id: blocker,
            position: point,
        });
        pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: moved,
            end: blocker,
            kind: EdgeKind::Auxiliary,
        });
    }
    let target = document([
        GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 2.0,
        },
        GeometricConstraintKindV1::Horizontal { edge: first },
    ]);
    assert!(construct_pair_constraint_exact_assignment_v1(&pattern, &target).is_none());
}
