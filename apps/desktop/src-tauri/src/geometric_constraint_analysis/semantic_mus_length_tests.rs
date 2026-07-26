use super::*;

#[test]
fn bounded_length_witness_is_a_distinct_strict_wire_counter() {
    let vertices = std::array::from_fn::<_, 6, _>(|_| VertexId::new());
    let edges = std::array::from_fn::<_, 3, _>(|_| EdgeId::new());
    let pattern = CreasePattern {
        vertices: vertices
            .into_iter()
            .enumerate()
            .map(|(index, id)| Vertex {
                id,
                position: Point2::new((index * 3) as f64, (index % 2) as f64),
            })
            .collect(),
        edges: edges
            .into_iter()
            .enumerate()
            .map(|(index, id)| Edge {
                id,
                start: vertices[index * 2],
                end: vertices[index * 2 + 1],
                kind: EdgeKind::Auxiliary,
            })
            .collect(),
    };
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edges[1],
            denominator_edge: edges[0],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edges[2],
            denominator_edge: edges[1],
            ratio: 3.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edges[0],
            denominator_edge: edges[2],
            ratio: 0.25,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("bounded length semantic outcome");
    let semantic_mus = outcome
        .semantic_mus
        .expect("length-only core must map to a semantic certificate");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 4,
            deletion_witness_checks: 4,
            current_assignment_witness_count: 0,
            axis_exactification_witness_count: 0,
            single_constraint_constructive_witness_count: 0,
            pair_constraint_constructive_witness_count: 0,
            pair_constraint_algebraic_witness_count: 0,
            length_constraint_constructive_witness_count: 4,
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
            ..
        } if constraint_ids == &expected_ids
    ));
    let encoded = serde_json::to_value(semantic_mus).expect("serialize length witness counter");
    assert_eq!(encoded["length_constraint_constructive_witness_count"], 4,);
}
