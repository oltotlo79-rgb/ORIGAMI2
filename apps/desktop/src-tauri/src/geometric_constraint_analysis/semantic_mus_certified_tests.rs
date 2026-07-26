use super::*;

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
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
        },
    );
    assert!(matches!(
        serde_json::to_value(&semantic_mus).expect("serialize certified semantic MUS"),
        serde_json::Value::Object(ref value)
            if value.get("status") == Some(&serde_json::json!("certified"))
                && value.get("constraint_ids") == Some(&serde_json::json!(expected_ids))
                && value.get("authorizes_project_mutation") == Some(&serde_json::json!(false))
                && value.get("replayable_across_runtimes") == Some(&serde_json::json!(false))
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
    assert_eq!(encoded["semantic_mus"]["replayable_across_runtimes"], false,);
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
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
            ..
        } if constraint_ids == &expected_ids
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
            reason: GeometricConstraintSemanticMusUnknownReason::DirectOracleIncomplete,
            ref direct_core_constraint_ids,
            direct_oracle_calls: 0,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
            ..
        }) if direct_core_constraint_ids.is_empty()
    ));
}
