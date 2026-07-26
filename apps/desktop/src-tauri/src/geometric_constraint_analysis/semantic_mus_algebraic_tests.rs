use super::*;

#[test]
fn algebraic_pair_witness_is_a_distinct_strict_wire_counter() {
    let center = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    let pattern = CreasePattern {
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
                id: first_edge,
                start: center,
                end: first_end,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: second_edge,
                start: second_end,
                end: center,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal { edge: first_edge }),
        record(GeometricConstraintKindV1::Vertical { edge: first_edge }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_edge,
            length_mm: 2.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("algebraic pair semantic outcome");
    let semantic_mus = outcome
        .semantic_mus
        .expect("algebraic pair must map to a semantic certificate");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 3,
            deletion_witness_checks: 3,
            current_assignment_witness_count: 0,
            axis_exactification_witness_count: 0,
            single_constraint_constructive_witness_count: 0,
            pair_constraint_constructive_witness_count: 2,
            pair_constraint_algebraic_witness_count: 1,
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
            ..
        } if constraint_ids == &expected_ids
    ));

    let encoded = serde_json::to_value(semantic_mus).expect("serialize algebraic pair counters");
    let object = encoded
        .as_object()
        .expect("semantic certificate must be an object");
    assert_eq!(object.len(), 14);
    assert_eq!(object["pair_constraint_constructive_witness_count"], 2);
    assert_eq!(object["pair_constraint_algebraic_witness_count"], 1);
    assert_eq!(
        [
            "current_assignment_witness_count",
            "axis_exactification_witness_count",
            "single_constraint_constructive_witness_count",
            "pair_constraint_constructive_witness_count",
            "pair_constraint_algebraic_witness_count",
        ]
        .into_iter()
        .map(|key| object[key].as_u64().expect("strict witness counter"))
        .sum::<u64>(),
        object["constraint_count"]
            .as_u64()
            .expect("strict constraint count"),
    );
}
