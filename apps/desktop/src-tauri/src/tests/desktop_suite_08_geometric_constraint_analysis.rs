use crate::geometric_constraint_analysis::GeometricConstraintSatisfactionEvidenceKind;

#[test]
fn geometric_constraint_preflight_exposes_exact_positive_and_fail_closed_states() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let first_edge = pattern.edges[0].id;
    let second_edge = pattern.edges[1].id;
    let horizontal = GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint: GeometricConstraintKindV1::Horizontal { edge: first_edge },
    };

    let exact_positive = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![horizontal.clone()],
    };
    assert_eq!(
        analyze_geometric_constraint_document(pattern, &exact_positive),
        GeometricConstraintPreflightResult::ProvenSatisfiable {
            model_id: ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
            transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            evidence_kind: GeometricConstraintSatisfactionEvidenceKind::CurrentAssignment,
            constraint_count: 1,
            equation_count: 1,
            authorizes_project_mutation: false,
            replayable_across_runtimes:
                ori_numeric::deterministic_transcendental_model_supported_v1(),
        }
    );
    assert_eq!(
        serde_json::to_value(analyze_geometric_constraint_document(
            pattern,
            &exact_positive,
        ))
        .expect("serialize exact positive constraint result"),
        serde_json::json!({
            "status": "proven_satisfiable",
            "model_id": "geometric_constraint_deterministic_binary64_exact_satisfaction_v2",
            "transcendental_model_id":
                ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            "evidence_kind": "current_assignment",
            "constraint_count": 1,
            "equation_count": 1,
            "authorizes_project_mutation": false,
            "replayable_across_runtimes":
                ori_numeric::deterministic_transcendental_model_supported_v1(),
        })
    );

    let constructed_positive = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge: second_edge },
        }],
    };
    assert_eq!(
        analyze_geometric_constraint_document(pattern, &constructed_positive),
        GeometricConstraintPreflightResult::ProvenSatisfiable {
            model_id: ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
            transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            evidence_kind:
                GeometricConstraintSatisfactionEvidenceKind::DetachedConstructedAssignment,
            constraint_count: 1,
            equation_count: 1,
            authorizes_project_mutation: false,
            replayable_across_runtimes:
                ori_numeric::deterministic_transcendental_model_supported_v1(),
        }
    );
    assert_eq!(
        serde_json::to_value(analyze_geometric_constraint_document(
            pattern,
            &constructed_positive,
        ))
        .expect("serialize constructed positive constraint result"),
        serde_json::json!({
            "status": "proven_satisfiable",
            "model_id": "geometric_constraint_deterministic_binary64_exact_satisfaction_v2",
            "transcendental_model_id":
                ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            "evidence_kind": "detached_constructed_assignment",
            "constraint_count": 1,
            "equation_count": 1,
            "authorizes_project_mutation": false,
            "replayable_across_runtimes":
                ori_numeric::deterministic_transcendental_model_supported_v1(),
        })
    );

    let no_direct = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: (0..=ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1)
            .map(|_| GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge: second_edge },
            })
            .collect(),
    };
    assert_eq!(
        analyze_geometric_constraint_document(pattern, &no_direct),
        GeometricConstraintPreflightResult::NoDirectConflict,
    );

    let zero_length_escape = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            horizontal.clone(),
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
            },
        ],
    };
    assert!(matches!(
        analyze_geometric_constraint_document(pattern, &zero_length_escape),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::SolverRequiredConstraintKinds,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids.len() == 2
    ));

    let direct = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            horizontal,
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: first_edge,
                    length_mm: 1.0,
                },
            },
        ],
    };
    let mut expected_mus_ids = direct
        .constraints
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    expected_mus_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    let GeometricConstraintPreflightResult::DirectConflict {
        conflicts,
        bounded_direct_mus,
    } = analyze_geometric_constraint_document(pattern, &direct)
    else {
        panic!("horizontal plus vertical must be a direct conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids().len(), 3);
    assert_eq!(
        bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids: expected_mus_ids.clone(),
            oracle_calls: 7,
        }
    );
    assert_eq!(
        serde_json::to_value(&bounded_direct_mus)
            .expect("serialize the bounded direct-conflict result"),
        serde_json::json!({
            "status": "proven_unsatisfiable",
            "constraint_ids": expected_mus_ids,
            "oracle_calls": 7,
        })
    );

    let edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    let mut solver_required_records = Vec::new();
    'edge_pairs: for &numerator_edge in &edge_ids {
        for &denominator_edge in &edge_ids {
            if numerator_edge == denominator_edge {
                continue;
            }
            solver_required_records.push(GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ratio: 2.0,
                },
            });
            if solver_required_records.len()
                > ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
            {
                break 'edge_pairs;
            }
        }
    }
    assert_eq!(
        solver_required_records.len(),
        ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 + 1,
        "the startup pattern must provide nine distinct valid ordered edge roles",
    );
    let solver_required = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: solver_required_records,
    };
    assert!(matches!(
        analyze_geometric_constraint_document(pattern, &solver_required),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::SolverRequiredConstraintKinds,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids.len()
            == ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 + 1
    ));
}

#[test]
fn exact_positive_publication_rechecks_late_cancel_and_deadline() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let constraint_id = ConstraintId::new();
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[0].id,
            },
        }],
    };
    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(pattern, &document)
            .expect("valid exact fixture")
            .expect("initial horizontal edge is exact");

    for (runtime, expected_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future test deadline"),
            },
            GeometricConstraintUnknownReason::Cancelled,
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            GeometricConstraintUnknownReason::DeadlineReached,
        ),
    ] {
        assert_eq!(
            crate::geometric_constraint_analysis::finish_exact_geometric_constraint_satisfaction(
                &document,
                &mut GeometricConstraintAnalysisObserver::new(runtime),
                certificate,
            ),
            GeometricConstraintPreflightResult::Unknown {
                reason: expected_reason,
                unchecked_constraint_ids: vec![constraint_id],
            }
        );
    }
}

#[test]
fn geometric_constraint_analysis_observer_reports_cancel_and_deadline_without_mutation() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let constraint_id = ConstraintId::new();
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[0].id,
            },
        }],
    };
    let pattern_before = pattern.clone();
    let document_before = document.clone();

    for (runtime, expected_reason, expected_wire_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future test deadline"),
            },
            GeometricConstraintUnknownReason::Cancelled,
            "cancelled",
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            GeometricConstraintUnknownReason::DeadlineReached,
            "deadline_reached",
        ),
    ] {
        let result = analyze_geometric_constraint_document_with_observer(
            pattern,
            &document,
            &mut GeometricConstraintAnalysisObserver::new(runtime),
        );
        assert_eq!(
            result,
            GeometricConstraintPreflightResult::Unknown {
                reason: expected_reason,
                unchecked_constraint_ids: vec![constraint_id],
            }
        );
        assert_eq!(
            serde_json::to_value(&result)
                .expect("serialize stopped geometric-constraint preflight")["reason"],
            expected_wire_reason
        );
    }
    assert_eq!(pattern, &pattern_before);
    assert_eq!(document, document_before);
}

#[test]
fn bounded_direct_mus_reports_cancel_and_deadline_as_distinct_unknown_reasons() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let edge = pattern.edges[0].id;
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge,
                    length_mm: 1.0,
                },
            },
        ],
    };
    let prepared = prepare_geometric_constraints_v1(
        pattern,
        &document,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("prepare direct-conflict MUS fixture");

    for (runtime, expected_reason, expected_wire_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future test deadline"),
            },
            BoundedDirectMusUnknownReason::Cancelled,
            "cancelled",
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            BoundedDirectMusUnknownReason::DeadlineReached,
            "deadline_reached",
        ),
    ] {
        let result = analyze_bounded_direct_mus_with_observer(
            &prepared,
            &mut GeometricConstraintAnalysisObserver::new(runtime),
        );
        assert_eq!(
            result,
            BoundedDirectMusResult::Unknown {
                reason: expected_reason,
                oracle_calls: 0,
                max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
            }
        );
        assert_eq!(
            serde_json::to_value(&result).expect("serialize stopped bounded direct MUS")["reason"],
            expected_wire_reason
        );
    }
}

#[test]
fn geometric_constraint_direct_mus_honors_the_sixteen_constraint_boundary() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let first_edge = pattern.edges[0].id;

    for count in [16_usize, 17] {
        let mut constraints = vec![
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: first_edge,
                    length_mm: 1.0,
                },
            },
        ];
        constraints.extend((3..count).map(|_| GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::FixedLength {
                edge: first_edge,
                length_mm: 1.0,
            },
        }));
        let document = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints,
        };

        let GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } = analyze_geometric_constraint_document(pattern, &document)
        else {
            panic!("the direct conflict must remain visible at the MUS size boundary");
        };

        assert!(!conflicts.is_empty());
        if count == MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
            let BoundedDirectMusResult::ProvenUnsatisfiable {
                constraint_ids,
                oracle_calls,
            } = bounded_direct_mus
            else {
                panic!("sixteen constraints must still run the bounded direct oracle");
            };
            assert_eq!(constraint_ids.len(), 3);
            assert!((1..=ori_core::MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1).contains(&oracle_calls));
        } else {
            assert_eq!(
                bounded_direct_mus,
                BoundedDirectMusResult::Unknown {
                    reason: BoundedDirectMusUnknownReason::ConstraintLimitExceeded,
                    oracle_calls: 0,
                    max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
                }
            );
            assert_eq!(
                serde_json::to_value(&bounded_direct_mus)
                    .expect("serialize the skipped bounded direct-conflict result"),
                serde_json::json!({
                    "status": "unknown",
                    "reason": "constraint_limit_exceeded",
                    "oracle_calls": 0,
                    "max_constraints": MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
                })
            );
        }
    }
}

fn oversized_geometric_constraint_vertex_pattern() -> CreasePattern {
    let vertices = (0..=ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES)
        .map(|index| Vertex {
            id: VertexId::new(),
            position: Point2::new(index as f64, (index % 2) as f64),
        })
        .collect::<Vec<_>>();
    let edges = vec![Edge {
        id: EdgeId::new(),
        start: vertices[0].id,
        end: vertices[1].id,
        kind: EdgeKind::Mountain,
    }];
    CreasePattern { vertices, edges }
}

#[test]
fn geometric_constraint_empty_v1_preflight_skips_oversized_and_repair_geometry() {
    let empty = GeometricConstraintDocumentV1::default();
    let empty_before = empty.clone();
    let oversized = oversized_geometric_constraint_vertex_pattern();
    let oversized_before = oversized.clone();

    assert_eq!(oversized.vertices.len(), 100_001);
    assert_eq!(
        analyze_geometric_constraint_document(&oversized, &empty),
        GeometricConstraintPreflightResult::NoDirectConflict
    );
    assert_eq!(oversized, oversized_before);
    assert_eq!(empty, empty_before);

    let duplicate_vertex = VertexId::new();
    let repair_geometry = CreasePattern {
        vertices: vec![
            Vertex {
                id: duplicate_vertex,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: duplicate_vertex,
                position: Point2::new(1.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: EdgeId::new(),
            start: duplicate_vertex,
            end: VertexId::new(),
            kind: EdgeKind::Valley,
        }],
    };
    let repair_geometry_before = repair_geometry.clone();

    assert_eq!(
        analyze_geometric_constraint_document(&repair_geometry, &empty),
        GeometricConstraintPreflightResult::NoDirectConflict
    );
    assert_eq!(repair_geometry, repair_geometry_before);
    assert_eq!(empty, empty_before);
}

#[test]
fn geometric_constraint_empty_invalid_schema_remains_unknown() {
    let pattern = CreasePattern::empty();
    let invalid = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1 + 1,
        constraints: Vec::new(),
    };
    let pattern_before = pattern.clone();
    let invalid_before = invalid.clone();

    assert_eq!(
        analyze_geometric_constraint_document(&pattern, &invalid),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
            unchecked_constraint_ids: Vec::new(),
        }
    );
    assert_eq!(pattern, pattern_before);
    assert_eq!(invalid, invalid_before);
}

#[test]
fn geometric_constraint_non_empty_oversized_geometry_remains_unknown() {
    let pattern = oversized_geometric_constraint_vertex_pattern();
    let constraint_id = ConstraintId::new();
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[0].id,
            },
        }],
    };
    let pattern_before = pattern.clone();
    let document_before = document.clone();

    assert_eq!(
        analyze_geometric_constraint_document(&pattern, &document),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
            unchecked_constraint_ids: vec![constraint_id],
        }
    );
    assert_eq!(pattern, pattern_before);
    assert_eq!(document, document_before);
}

#[test]
fn geometric_constraint_preflight_fails_closed_for_invalid_references() {
    let project = initial_project_state();
    let first = ConstraintId::new();
    let second = ConstraintId::new();
    let invalid = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            GeometricConstraintRecordV1 {
                id: first,
                constraint: GeometricConstraintKindV1::Horizontal {
                    edge: EdgeId::new(),
                },
            },
            GeometricConstraintRecordV1 {
                id: second,
                constraint: GeometricConstraintKindV1::Vertical {
                    edge: EdgeId::new(),
                },
            },
        ],
    };

    let GeometricConstraintPreflightResult::Unknown {
        reason,
        unchecked_constraint_ids,
    } = analyze_geometric_constraint_document(project.editor.pattern(), &invalid)
    else {
        panic!("invalid references must not be reported as safe");
    };
    assert_eq!(
        reason,
        GeometricConstraintUnknownReason::InvalidDocumentOrGeometry
    );
    let mut expected = vec![first, second];
    expected.sort_unstable_by_key(ConstraintId::canonical_bytes);
    assert_eq!(unchecked_constraint_ids, expected);
}
