use super::*;

#[test]
fn zero_length_closure_witness_is_a_distinct_strict_wire_counter() {
    let vertices = std::array::from_fn::<_, 4, _>(|_| VertexId::new());
    let target = EdgeId::new();
    let forced = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: vertices[0],
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: vertices[1],
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: vertices[2],
                position: Point2::new(3.0, 0.0),
            },
            Vertex {
                id: vertices[3],
                position: Point2::new(3.0, 1.0),
            },
        ],
        edges: vec![
            Edge {
                id: target,
                start: vertices[0],
                end: vertices[1],
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: forced,
                start: vertices[2],
                end: vertices[3],
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal { edge: forced }),
        record(GeometricConstraintKindV1::Vertical { edge: forced }),
        record(GeometricConstraintKindV1::EqualLength {
            first_edge: forced,
            second_edge: target,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: target,
            length_mm: 2.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("zero-length-closure semantic outcome");
    let semantic_mus = outcome
        .semantic_mus
        .expect("zero-length-closure core must map to a semantic certificate");
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
            length_constraint_constructive_witness_count: 0,
            zero_length_closure_constructive_witness_count: 4,
            authorizes_project_mutation: false,
            replayable_across_runtimes,
            ..
        } if constraint_ids == &expected_ids
            && *replayable_across_runtimes
                == ori_numeric::deterministic_transcendental_model_supported_v1()
    ));
    let encoded =
        serde_json::to_value(semantic_mus).expect("serialize zero-closure witness counter");
    assert_eq!(
        encoded["transcendental_model_id"],
        ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    );
    assert_eq!(encoded["zero_length_closure_constructive_witness_count"], 4,);
    let method_sum = [
        "current_assignment_witness_count",
        "axis_exactification_witness_count",
        "single_constraint_constructive_witness_count",
        "pair_constraint_constructive_witness_count",
        "pair_constraint_algebraic_witness_count",
        "length_constraint_constructive_witness_count",
        "zero_length_closure_constructive_witness_count",
    ]
    .into_iter()
    .map(|key| encoded[key].as_u64().expect("wire counter is u64"))
    .sum::<u64>();
    assert_eq!(method_sum, 4);
}
