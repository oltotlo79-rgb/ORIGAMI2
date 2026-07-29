use super::*;

#[test]
fn anchored_mirror_residual_only_counter_maps_and_serializes_exactly_once() {
    let axis_start = VertexId::new();
    let axis_end = VertexId::new();
    let mut symmetry_vertices = [VertexId::new(), VertexId::new()];
    symmetry_vertices.sort_unstable_by_key(VertexId::canonical_bytes);
    let raw_source = symmetry_vertices[1];
    let raw_target = symmetry_vertices[0];
    let axis_edge = EdgeId::new();
    let connector_edge = EdgeId::new();
    let separation_edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: axis_start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: axis_end,
                position: Point2::new(2.0, 1.0),
            },
            Vertex {
                id: raw_source,
                position: Point2::new(3.0, 5.0),
            },
            Vertex {
                id: raw_target,
                position: Point2::new(7.0, 11.0),
            },
        ],
        edges: vec![
            Edge {
                id: axis_edge,
                start: axis_start,
                end: axis_end,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: connector_edge,
                start: axis_start,
                end: raw_source,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: separation_edge,
                start: raw_source,
                end: raw_target,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let records = vec![
        record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: raw_source,
            second_vertex: raw_target,
            axis_edge,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: separation_edge,
            length_mm: 2.0,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: connector_edge,
        }),
        record(GeometricConstraintKindV1::Vertical {
            edge: connector_edge,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("anchored mirror semantic outcome");
    let semantic_mus = outcome
        .semantic_mus
        .expect("anchored mirror core must map to a semantic certificate");
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
            zero_length_closure_constructive_witness_count: 0,
            anchored_mirror_residual_only_witness_count: 4,
            unit_parallel_fixed_angle_residual_only_witness_count: 0,
            unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 0,
            unit_two_hop_parallel_residual_only_witness_count: 0,
            authorizes_project_mutation: false,
            replayable_across_runtimes,
            ..
        } if constraint_ids == &expected_ids
            && *replayable_across_runtimes
                == ori_numeric::deterministic_transcendental_model_supported_v1()
    ));

    let encoded = serde_json::to_value(semantic_mus).expect("serialize anchored mirror counter");
    assert_eq!(encoded["anchored_mirror_residual_only_witness_count"], 4,);
    let method_sum = [
        "current_assignment_witness_count",
        "axis_exactification_witness_count",
        "single_constraint_constructive_witness_count",
        "pair_constraint_constructive_witness_count",
        "pair_constraint_algebraic_witness_count",
        "length_constraint_constructive_witness_count",
        "zero_length_closure_constructive_witness_count",
        "anchored_mirror_residual_only_witness_count",
        "unit_parallel_fixed_angle_residual_only_witness_count",
        "unit_terminal_two_hop_parallel_angle_residual_only_witness_count",
        "unit_two_hop_parallel_residual_only_witness_count",
    ]
    .into_iter()
    .map(|key| encoded[key].as_u64().expect("wire counter is u64"))
    .sum::<u64>();
    assert_eq!(method_sum, 4);
}
