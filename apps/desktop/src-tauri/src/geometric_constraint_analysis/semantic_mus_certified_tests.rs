use super::*;
use ori_core::DirectConstraintConflictKindV1;

#[test]
fn certified_outcome_uses_one_semantic_call_for_both_native_dtos() {
    let fixture = semantic_fixture();
    let pattern_before = fixture.pattern.clone();
    let document = document(fixture.records.iter().cloned());
    let document_before = document.clone();
    let expected_ids = canonical_ids(&fixture.records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &fixture.pattern,
        &document,
        &mut continuing_observer(),
    )
    .expect("valid semantic-MUS DTO");

    let (conflicts, bounded_direct_mus) = match outcome.result {
        GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } => (conflicts, bounded_direct_mus),
        other => panic!("expected direct conflict, got {other:?}"),
    };
    assert_eq!(
        bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids: expected_ids.clone(),
            oracle_calls: 7,
        },
    );
    let semantic_mus = outcome
        .semantic_mus
        .expect("a direct outcome always carries the new field");
    assert_eq!(
        semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            model_id: ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1,
            transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            constraint_ids: expected_ids.clone(),
            constraint_count: 3,
            direct_oracle_calls: 7,
            deletion_witness_checks: 3,
            deletion_witness_work: match &semantic_mus {
                GeometricConstraintSemanticMusResult::Certified {
                    deletion_witness_work,
                    ..
                } => *deletion_witness_work,
                _ => unreachable!(),
            },
            current_assignment_witness_count: 1,
            axis_exactification_witness_count: 2,
            single_constraint_constructive_witness_count: 0,
            pair_constraint_constructive_witness_count: 0,
            pair_constraint_algebraic_witness_count: 0,
            length_constraint_constructive_witness_count: 0,
            zero_length_closure_constructive_witness_count: 0,
            anchored_mirror_residual_only_witness_count: 0,
            unit_parallel_fixed_angle_residual_only_witness_count: 0,
            unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 0,
            unit_two_hop_parallel_residual_only_witness_count: 0,
            authorizes_project_mutation: false,
            replayable_across_runtimes:
                ori_numeric::deterministic_transcendental_model_supported_v1(),
        },
    );
    assert!(matches!(
        serde_json::to_value(&semantic_mus).expect("serialize certified semantic MUS"),
        serde_json::Value::Object(ref value)
            if value.get("status") == Some(&serde_json::json!("certified"))
                && value.get("constraint_ids") == Some(&serde_json::json!(expected_ids))
                && value.get("transcendental_model_id")
                    == Some(&serde_json::json!(
                        ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
                    ))
                && value.get("authorizes_project_mutation") == Some(&serde_json::json!(false))
                && value.get("replayable_across_runtimes")
                    == Some(&serde_json::json!(
                        ori_numeric::deterministic_transcendental_model_supported_v1()
                    ))
    ));

    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let invocation_count = Cell::new(0);
    let (derived_direct, derived_semantic) = analyze_semantic_direct_conflict_with(
        &prepared,
        &mut continuing_observer(),
        |prepared, observer| {
            invocation_count.set(invocation_count.get() + 1);
            ori_core::certify_bounded_current_runtime_semantic_mus_with_observer_v1(
                prepared,
                ori_core::BoundedSemanticMusLimitsV1::default(),
                observer,
            )
        },
    )
    .expect("single semantic invocation maps to both DTOs");
    assert_eq!(invocation_count.get(), 1);
    assert_eq!(derived_direct, bounded_direct_mus);
    assert_eq!(derived_semantic, semantic_mus);

    let response = GeometricConstraintPreflightResponse {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 9,
        result: GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        },
        semantic_mus: Some(semantic_mus),
    };
    let encoded =
        serde_json::to_value(response).expect("serialize response with certified semantic field");
    assert_eq!(encoded["semantic_mus"]["status"], "certified");
    assert_eq!(
        encoded["semantic_mus"]["authorizes_project_mutation"],
        false,
    );
    assert_eq!(
        encoded["semantic_mus"]["transcendental_model_id"],
        ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    );
    assert_eq!(
        encoded["semantic_mus"]["replayable_across_runtimes"],
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    );
    assert_eq!(
        encoded["semantic_mus"]["single_constraint_constructive_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["pair_constraint_constructive_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["pair_constraint_algebraic_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["length_constraint_constructive_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["zero_length_closure_constructive_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["anchored_mirror_residual_only_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["unit_parallel_fixed_angle_residual_only_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["unit_terminal_two_hop_parallel_angle_residual_only_witness_count"],
        0,
    );
    assert_eq!(
        encoded["semantic_mus"]["unit_two_hop_parallel_residual_only_witness_count"],
        0,
    );
    assert_eq!(fixture.pattern, pattern_before);
    assert_eq!(document, document_before);
}

#[test]
fn different_fixed_lengths_are_promoted_by_the_constructive_singleton_witness() {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(1.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        }],
    };
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("constructive singleton semantic outcome");
    assert_eq!(
        match outcome.result {
            GeometricConstraintPreflightResult::DirectConflict {
                bounded_direct_mus, ..
            } => bounded_direct_mus,
            other => panic!("expected direct conflict, got {other:?}"),
        },
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids: expected_ids.clone(),
            oracle_calls: 3,
        },
    );
    let semantic_mus = outcome
        .semantic_mus
        .expect("different fixed lengths now have a semantic certificate");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 2,
            direct_oracle_calls: 3,
            deletion_witness_checks: 2,
            current_assignment_witness_count: 1,
            axis_exactification_witness_count: 0,
            single_constraint_constructive_witness_count: 1,
            pair_constraint_constructive_witness_count: 0,
            pair_constraint_algebraic_witness_count: 0,
            length_constraint_constructive_witness_count: 0,
            zero_length_closure_constructive_witness_count: 0,
            anchored_mirror_residual_only_witness_count: 0,
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
    let encoded =
        serde_json::to_value(semantic_mus).expect("serialize constructive singleton witness count");
    assert_eq!(encoded["single_constraint_constructive_witness_count"], 1,);
}

#[test]
fn unreachable_constructive_candidates_keep_a_strict_unknown_phase_dto() {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    let mut pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(3.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        }],
    };
    for point in [
        Point2::new(1.0, 0.0),
        Point2::new(17.0, 32.0),
        Point2::new(-15.0, 8.0),
        Point2::new(1025.0, -512.0),
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
            start: end,
            end: blocker,
            kind: EdgeKind::Auxiliary,
        });
    }
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 2.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("reachable unavailable phase must map");
    assert!(matches!(
        outcome.result,
        GeometricConstraintPreflightResult::DirectConflict {
            bounded_direct_mus: BoundedDirectMusResult::ProvenUnsatisfiable {
                ref constraint_ids,
                oracle_calls: 3,
            },
            ..
        } if constraint_ids == &expected_ids
    ));
    assert!(matches!(
        outcome.semantic_mus,
        Some(GeometricConstraintSemanticMusResult::Unknown {
            model_id:
                ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1,
            transcendental_model_id:
                ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            reason: GeometricConstraintSemanticMusUnknownReason::DeletionWitnessUnavailable,
            direct_core_constraint_ids,
            direct_oracle_calls: 3,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 0,
            deletion_witness_work,
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
            ..
        }) if direct_core_constraint_ids == expected_ids && deletion_witness_work > 0
    ));
}

#[test]
fn non_direct_response_serializes_null_and_oversized_direct_input_stays_unknown() {
    let fixture = semantic_fixture();
    let horizontal_edge = fixture.pattern.edges[1].id;
    let no_direct = document([record(GeometricConstraintKindV1::Horizontal {
        edge: horizontal_edge,
    })]);
    let no_direct_outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &fixture.pattern,
        &no_direct,
        &mut continuing_observer(),
    )
    .expect("non-direct exact outcome");
    assert!(matches!(
        no_direct_outcome.result,
        GeometricConstraintPreflightResult::ProvenSatisfiable { .. }
    ));
    assert_eq!(no_direct_outcome.semantic_mus, None);

    let response = GeometricConstraintPreflightResponse {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 0,
        result: no_direct_outcome.result,
        semantic_mus: None,
    };
    let encoded = serde_json::to_value(response).expect("serialize response with semantic field");
    assert!(encoded.get("semantic_mus").is_some());
    assert_eq!(encoded["semantic_mus"], serde_json::Value::Null);

    let mut oversized = fixture.records.clone();
    let padding = oversized[0].constraint.clone();
    oversized.extend((oversized.len()..17).map(|_| record(padding.clone())));
    let oversized_outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &fixture.pattern,
        &document(oversized),
        &mut continuing_observer(),
    )
    .expect("bounded direct limit is an explicit semantic unknown");
    assert!(matches!(
        oversized_outcome.result,
        GeometricConstraintPreflightResult::DirectConflict {
            bounded_direct_mus: BoundedDirectMusResult::Unknown {
                reason: BoundedDirectMusUnknownReason::ConstraintLimitExceeded,
                oracle_calls: 0,
                max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
            },
            ..
        }
    ));
    assert!(matches!(
        oversized_outcome.semantic_mus,
        Some(GeometricConstraintSemanticMusResult::Unknown {
            model_id:
                ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1,
            transcendental_model_id:
                ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            reason: GeometricConstraintSemanticMusUnknownReason::DirectOracleIncomplete,
            ref direct_core_constraint_ids,
            direct_oracle_calls: 0,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
            ..
        }) if direct_core_constraint_ids.is_empty()
    ));
}

#[test]
fn reverse_binary64_ratio_domain_uses_the_existing_wire_tag_and_semantic_dto() {
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut edge_ids = Vec::new();
    for index in 0..5 {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        vertices.extend([
            Vertex {
                id: start,
                position: Point2::new(index as f64 * 4.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(index as f64 * 4.0 + 1.0, 0.0),
            },
        ]);
        edges.push(Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        });
        edge_ids.push(edge);
    }
    let pattern = CreasePattern { vertices, edges };
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: edge_ids[4],
            length_mm: 7.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[4],
            denominator_edge: edge_ids[0],
            ratio: 11.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[0],
            denominator_edge: edge_ids[1],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[1],
            denominator_edge: edge_ids[2],
            ratio: 3.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[2],
            denominator_edge: edge_ids[3],
            ratio: 5.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[3],
            denominator_edge: edge_ids[0],
            ratio: 0.1,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("reverse-domain analysis must map through the desktop boundary");

    let (conflicts, bounded_direct_mus) = match outcome.result {
        GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } => (conflicts, bounded_direct_mus),
        other => panic!("expected reverse-domain direct conflict, got {other:?}"),
    };
    assert!(conflicts.iter().any(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
                fixed_edge,
                ratio_constraint_count: 5,
            } if *fixed_edge == edge_ids[4]
        ) && conflict.constraint_ids() == expected_ids.as_slice()
    }));
    assert!(matches!(
        &bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == &expected_ids && *oracle_calls > 0
    ));

    let semantic_mus = outcome
        .semantic_mus
        .expect("reverse-domain direct result must carry semantic status");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 6,
            deletion_witness_checks: 6,
            length_constraint_constructive_witness_count: 6,
            authorizes_project_mutation: false,
            ..
        } if constraint_ids == &expected_ids
    ));

    let response = GeometricConstraintPreflightResponse {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 11,
        result: GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        },
        semantic_mus: Some(semantic_mus),
    };
    let encoded =
        serde_json::to_value(response).expect("serialize reverse-domain desktop response");
    assert_eq!(
        encoded["result"]["conflicts"][0]["conflict"]["kind"],
        "inconsistent_length_ratio_graph_with_fixed_length",
    );
    assert_eq!(
        encoded["result"]["conflicts"][0]["conflict"]["ratio_constraint_count"],
        5,
    );
    assert_eq!(encoded["semantic_mus"]["status"], "certified");
    assert_eq!(
        encoded["semantic_mus"]["length_constraint_constructive_witness_count"],
        6,
    );
}

#[test]
fn disjoint_fixed_root_ratio_domains_cross_the_desktop_boundary_as_a_four_id_mus() {
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut edge_ids = Vec::new();
    for index in 0..3 {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        vertices.extend([
            Vertex {
                id: start,
                position: Point2::new(index as f64 * 4.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(index as f64 * 4.0 + 1.0, 0.0),
            },
        ]);
        edges.push(Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        });
        edge_ids.push(edge);
    }
    let pattern = CreasePattern { vertices, edges };
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: edge_ids[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: edge_ids[1],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[2],
            denominator_edge: edge_ids[0],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[2],
            denominator_edge: edge_ids[1],
            ratio: 3.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("cross-root domain analysis must map through the desktop boundary");

    let (conflicts, bounded_direct_mus) = match outcome.result {
        GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } => (conflicts, bounded_direct_mus),
        other => panic!("expected cross-root direct conflict, got {other:?}"),
    };
    let mut expected_fixed_edges = [edge_ids[0], edge_ids[1]];
    expected_fixed_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    assert!(conflicts.iter().any(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::InconsistentLengthRatioGraphBetweenFixedLengths {
                first_fixed_edge,
                second_fixed_edge,
                ratio_constraint_count: 2,
            } if [*first_fixed_edge, *second_fixed_edge] == expected_fixed_edges
        ) && conflict.constraint_ids() == expected_ids.as_slice()
    }));
    assert!(matches!(
        &bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == &expected_ids && *oracle_calls > 0
    ));

    let semantic_mus = outcome
        .semantic_mus
        .expect("cross-root direct result must carry semantic status");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 4,
            deletion_witness_checks: 4,
            length_constraint_constructive_witness_count: 4,
            authorizes_project_mutation: false,
            ..
        } if constraint_ids == &expected_ids
    ));

    let response = GeometricConstraintPreflightResponse {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 12,
        result: GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        },
        semantic_mus: Some(semantic_mus),
    };
    let encoded = serde_json::to_value(response).expect("serialize cross-root desktop response");
    let encoded_conflict = encoded["result"]["conflicts"]
        .as_array()
        .expect("conflicts array")
        .iter()
        .find(|conflict| {
            conflict["conflict"]["kind"] == "inconsistent_length_ratio_graph_between_fixed_lengths"
        })
        .expect("serialized cross-root conflict");
    assert_eq!(encoded_conflict["conflict"]["ratio_constraint_count"], 2,);
    assert_eq!(
        encoded_conflict["conflict"]["first_fixed_edge"],
        serde_json::json!(expected_fixed_edges[0]),
    );
    assert_eq!(
        encoded_conflict["conflict"]["second_fixed_edge"],
        serde_json::json!(expected_fixed_edges[1]),
    );
    assert_eq!(
        encoded_conflict["constraint_ids"].as_array().unwrap().len(),
        4
    );
    assert_eq!(encoded["semantic_mus"]["status"], "certified");
    assert_eq!(
        encoded["semantic_mus"]["length_constraint_constructive_witness_count"],
        4,
    );
}

#[test]
fn unit_terminal_two_hop_parallel_counter_crosses_the_native_dto_exactly_five_times() {
    let center = VertexId::new();
    let endpoints = [VertexId::new(), VertexId::new(), VertexId::new()];
    let edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: center,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: endpoints[0],
                position: Point2::new(3.0, 1.0),
            },
            Vertex {
                id: endpoints[1],
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: endpoints[2],
                position: Point2::new(1.0, 3.0),
            },
        ],
        edges: edges
            .into_iter()
            .zip(endpoints)
            .map(|(id, end)| Edge {
                id,
                start: center,
                end,
                kind: EdgeKind::Auxiliary,
            })
            .collect(),
    };
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal { edge: edges[0] }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: edges[0],
            second_edge: edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: edges[1],
            second_edge: edges[2],
        }),
        record(GeometricConstraintKindV1::Vertical { edge: edges[2] }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: edges[2],
            length_mm: 1.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("unit-terminal two-hop semantic outcome");
    let (conflicts, bounded_direct_mus) = match outcome.result {
        GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } => (conflicts, bounded_direct_mus),
        other => panic!("expected unit-terminal direct conflict, got {other:?}"),
    };
    assert!(conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
                parallel_constraint_count: 2,
                ..
            }
        ) && candidate.constraint_ids() == expected_ids
    }));
    assert!(matches!(
        &bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == &expected_ids && *oracle_calls > 0
    ));

    let semantic_mus = outcome
        .semantic_mus
        .expect("unit-terminal two-hop core must carry semantic status");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 5,
            deletion_witness_checks: 5,
            current_assignment_witness_count: 0,
            axis_exactification_witness_count: 0,
            single_constraint_constructive_witness_count: 0,
            pair_constraint_constructive_witness_count: 0,
            pair_constraint_algebraic_witness_count: 0,
            length_constraint_constructive_witness_count: 0,
            zero_length_closure_constructive_witness_count: 0,
            anchored_mirror_residual_only_witness_count: 0,
            unit_parallel_fixed_angle_residual_only_witness_count: 0,
            unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 0,
            unit_two_hop_parallel_residual_only_witness_count: 5,
            authorizes_project_mutation: false,
            ..
        } if constraint_ids == &expected_ids
    ));

    let response = GeometricConstraintPreflightResponse {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 13,
        result: GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        },
        semantic_mus: Some(semantic_mus),
    };
    let encoded =
        serde_json::to_value(response).expect("serialize unit-terminal two-hop desktop response");
    let semantic = encoded["semantic_mus"]
        .as_object()
        .expect("certified semantic MUS object");
    assert_eq!(semantic.len(), 21);
    assert_eq!(
        semantic["unit_two_hop_parallel_residual_only_witness_count"],
        5,
    );
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
    .map(|key| semantic[key].as_u64().expect("wire counter is u64"))
    .sum::<u64>();
    assert_eq!(method_sum, 5);
}

#[test]
fn unit_terminal_two_hop_parallel_angle_counter_crosses_the_native_dto_exactly_five_times() {
    let center = VertexId::new();
    let endpoints = [VertexId::new(), VertexId::new(), VertexId::new()];
    let edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: center,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: endpoints[0],
                position: Point2::new(3.0, 1.0),
            },
            Vertex {
                id: endpoints[1],
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: endpoints[2],
                position: Point2::new(1.0, 3.0),
            },
        ],
        edges: edges
            .into_iter()
            .zip(endpoints)
            .map(|(id, end)| Edge {
                id,
                start: center,
                end,
                kind: EdgeKind::Auxiliary,
            })
            .collect(),
    };
    let records = vec![
        record(GeometricConstraintKindV1::Parallel {
            first_edge: edges[0],
            second_edge: edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: edges[1],
            second_edge: edges[2],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: center,
            first_edge: edges[0],
            second_edge: edges[2],
            angle_degrees: 90.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: edges[2],
            length_mm: 1.0,
        }),
    ];
    let expected_ids = canonical_ids(&records);
    let outcome = analyze_geometric_constraint_document_outcome_with_observer(
        &pattern,
        &document(records),
        &mut continuing_observer(),
    )
    .expect("unit-terminal two-hop angle semantic outcome");
    let (conflicts, bounded_direct_mus) = match outcome.result {
        GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } => (conflicts, bounded_direct_mus),
        other => panic!("expected unit-terminal angle direct conflict, got {other:?}"),
    };
    assert!(conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
                parallel_constraint_count: 2,
                ..
            }
        ) && candidate.constraint_ids() == expected_ids
    }));
    assert!(matches!(
        &bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == &expected_ids && *oracle_calls > 0
    ));

    let semantic_mus = outcome
        .semantic_mus
        .expect("unit-terminal two-hop angle core must carry semantic status");
    assert!(matches!(
        &semantic_mus,
        GeometricConstraintSemanticMusResult::Certified {
            constraint_ids,
            constraint_count: 5,
            deletion_witness_checks: 5,
            current_assignment_witness_count: 0,
            axis_exactification_witness_count: 0,
            single_constraint_constructive_witness_count: 0,
            pair_constraint_constructive_witness_count: 0,
            pair_constraint_algebraic_witness_count: 0,
            length_constraint_constructive_witness_count: 0,
            zero_length_closure_constructive_witness_count: 0,
            anchored_mirror_residual_only_witness_count: 0,
            unit_parallel_fixed_angle_residual_only_witness_count: 0,
            unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 5,
            unit_two_hop_parallel_residual_only_witness_count: 0,
            authorizes_project_mutation: false,
            ..
        } if constraint_ids == &expected_ids
    ));

    let response = GeometricConstraintPreflightResponse {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 14,
        result: GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        },
        semantic_mus: Some(semantic_mus),
    };
    let encoded = serde_json::to_value(response)
        .expect("serialize unit-terminal two-hop angle desktop response");
    let semantic = encoded["semantic_mus"]
        .as_object()
        .expect("certified semantic MUS object");
    assert_eq!(semantic.len(), 21);
    assert_eq!(
        semantic["unit_terminal_two_hop_parallel_angle_residual_only_witness_count"],
        5,
    );
    assert_eq!(
        semantic["unit_two_hop_parallel_residual_only_witness_count"],
        0,
    );
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
    .map(|key| semantic[key].as_u64().expect("wire counter is u64"))
    .sum::<u64>();
    assert_eq!(method_sum, 5);
}

#[test]
fn unit_parallel_supplementary_fixed_angle_counter_crosses_the_native_dto_exactly_three_times() {
    for angle_degrees in [45.0, 135.0] {
        let center = VertexId::new();
        let endpoints = [VertexId::new(), VertexId::new()];
        let edges = [EdgeId::new(), EdgeId::new()];
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: endpoints[0],
                    position: Point2::new(3.0, 1.0),
                },
                Vertex {
                    id: endpoints[1],
                    position: Point2::new(1.0, 3.0),
                },
            ],
            edges: edges
                .into_iter()
                .zip(endpoints)
                .map(|(id, end)| Edge {
                    id,
                    start: center,
                    end,
                    kind: EdgeKind::Auxiliary,
                })
                .collect(),
        };
        let records = vec![
            record(GeometricConstraintKindV1::Parallel {
                first_edge: edges[0],
                second_edge: edges[1],
            }),
            record(GeometricConstraintKindV1::FixedAngle {
                vertex: center,
                first_edge: edges[0],
                second_edge: edges[1],
                angle_degrees,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: edges[0],
                length_mm: 1.0,
            }),
        ];
        let expected_ids = canonical_ids(&records);
        let outcome = analyze_geometric_constraint_document_outcome_with_observer(
            &pattern,
            &document(records),
            &mut continuing_observer(),
        )
        .expect("unit parallel-fixed-angle semantic outcome");
        let (conflicts, bounded_direct_mus) = match outcome.result {
            GeometricConstraintPreflightResult::DirectConflict {
                conflicts,
                bounded_direct_mus,
            } => (conflicts, bounded_direct_mus),
            other => panic!("expected unit parallel-fixed-angle direct conflict, got {other:?}"),
        };
        assert!(conflicts.iter().any(|candidate| {
            matches!(
                candidate.conflict(),
                DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
            ) && candidate.constraint_ids() == expected_ids
        }));
        assert!(matches!(
            &bounded_direct_mus,
            BoundedDirectMusResult::ProvenUnsatisfiable {
                constraint_ids,
                oracle_calls,
            } if constraint_ids == &expected_ids && *oracle_calls > 0
        ));

        let semantic_mus = outcome
            .semantic_mus
            .expect("unit parallel-fixed-angle core must carry semantic status");
        assert!(matches!(
            &semantic_mus,
            GeometricConstraintSemanticMusResult::Certified {
                constraint_ids,
                constraint_count: 3,
                deletion_witness_checks: 3,
                deletion_witness_work,
                current_assignment_witness_count: 0,
                axis_exactification_witness_count: 0,
                single_constraint_constructive_witness_count: 0,
                pair_constraint_constructive_witness_count: 0,
                pair_constraint_algebraic_witness_count: 0,
                length_constraint_constructive_witness_count: 0,
                zero_length_closure_constructive_witness_count: 0,
                anchored_mirror_residual_only_witness_count: 0,
                unit_parallel_fixed_angle_residual_only_witness_count: 3,
                unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 0,
                unit_two_hop_parallel_residual_only_witness_count: 0,
                authorizes_project_mutation: false,
                ..
            } if constraint_ids == &expected_ids && *deletion_witness_work > 0
        ));

        let response = GeometricConstraintPreflightResponse {
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision: 15,
            result: GeometricConstraintPreflightResult::DirectConflict {
                conflicts,
                bounded_direct_mus,
            },
            semantic_mus: Some(semantic_mus),
        };
        let encoded = serde_json::to_value(response)
            .expect("serialize unit parallel-fixed-angle desktop response");
        let semantic = encoded["semantic_mus"]
            .as_object()
            .expect("certified semantic MUS object");
        assert_eq!(semantic.len(), 21);
        assert_eq!(
            semantic["unit_parallel_fixed_angle_residual_only_witness_count"],
            3,
        );
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
        .map(|key| semantic[key].as_u64().expect("wire counter is u64"))
        .sum::<u64>();
        assert_eq!(method_sum, 3);
    }
}
