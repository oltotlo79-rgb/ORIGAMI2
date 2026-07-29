use serde_json::{Value, json};

use super::tests::{
    Fixture, deterministic_shuffle, document, prepare, record, reverse_unordered_operands,
    sorted_ids, uuid_string,
};
use super::*;

#[test]
fn no_direct_conflict_and_unknown_are_distinct_canonical_native_outputs() {
    let fixture = Fixture::new();
    let checked = prepare(
        &fixture,
        &document([
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
        ]),
    )
    .expect("valid checked constraints");
    assert_eq!(checked.preflight(), ConstraintPreflightV1::NoDirectConflict);

    let solver_required = record(GeometricConstraintKindV1::PointOnLine {
        vertex: fixture.vertices[2],
        line_edge: fixture.edges[5],
    });
    let unchecked = prepare(&fixture, &document([solver_required.clone()]))
        .expect("valid solver-required constraint");
    let outcome = unchecked.preflight();
    assert_eq!(
        outcome,
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: vec![solver_required.id],
        }
    );
    let wire = serde_json::to_string(&outcome).expect("serialize preflight result");
    let expected_wire = format!(
        r#"{{"status":"unknown","reason":"solver_required_constraint_kinds","unchecked_constraint_ids":["{}"]}}"#,
        uuid_string(solver_required.id)
    );
    assert_eq!(wire, expected_wire);
    assert_eq!(
        serde_json::from_str::<Value>(&wire).expect("native output is valid JSON"),
        json!({
            "status": "unknown",
            "reason": "solver_required_constraint_kinds",
            "unchecked_constraint_ids": [uuid_string(solver_required.id)],
        })
    );
}

#[test]
fn storage_order_geometry_order_and_unordered_operand_property_are_invariant() {
    let fixture = Fixture::new();
    let mut records = fixture
        .all_kinds()
        .into_iter()
        .map(record)
        .collect::<Vec<_>>();
    records.push(record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 21.0,
    }));

    let baseline = prepare(&fixture, &document(records.clone())).expect("baseline");
    let baseline_outcome = baseline.preflight();

    let mut reordered_pattern = fixture.pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let reordered_fixture = Fixture {
        pattern: reordered_pattern,
        vertices: fixture.vertices,
        edges: fixture.edges,
    };

    let mut seed = 0x9e37_79b9_u64;
    for _ in 0..128 {
        deterministic_shuffle(&mut records, &mut seed);
        for record in &mut records {
            reverse_unordered_operands(&mut record.constraint);
        }
        let candidate =
            prepare(&reordered_fixture, &document(records.clone())).expect("permutation");
        assert_eq!(candidate.constraints(), baseline.constraints());
        assert_eq!(candidate.preflight(), baseline_outcome);
    }
}

#[test]
fn validation_error_selection_is_invariant_to_storage_permutations() {
    let fixture = Fixture::new();
    let missing_a = EdgeId::new();
    let missing_b = EdgeId::new();
    let first = record(GeometricConstraintKindV1::Horizontal { edge: missing_a });
    let second = record(GeometricConstraintKindV1::Vertical { edge: missing_b });
    let expected_id = if first.id.canonical_bytes() < second.id.canonical_bytes() {
        first.id
    } else {
        second.id
    };
    let forward = prepare(&fixture, &document([first.clone(), second.clone()]))
        .expect_err("both documents contain missing references");
    let reverse = prepare(&fixture, &document([second, first]))
        .expect_err("both documents contain missing references");
    assert_eq!(forward, reverse);
    assert!(matches!(
        forward,
        GeometricConstraintErrorV1::MissingEdge { constraint, .. }
            if constraint == expected_id
    ));
}

#[test]
fn validation_normalizes_unordered_operands_before_selecting_an_error() {
    let fixture = Fixture::new();
    let first_missing = EdgeId::new();
    let second_missing = EdgeId::new();
    let constraint_id = ConstraintId::new();
    let forward = GeometricConstraintRecordV1 {
        id: constraint_id,
        constraint: GeometricConstraintKindV1::EqualLength {
            first_edge: first_missing,
            second_edge: second_missing,
        },
    };
    let reverse = GeometricConstraintRecordV1 {
        id: constraint_id,
        constraint: GeometricConstraintKindV1::EqualLength {
            first_edge: second_missing,
            second_edge: first_missing,
        },
    };
    let forward_error =
        prepare(&fixture, &document([forward])).expect_err("both references are missing");
    let reverse_error =
        prepare(&fixture, &document([reverse])).expect_err("both references are missing");
    assert_eq!(forward_error, reverse_error);

    let canonical_first = if first_missing.canonical_bytes() < second_missing.canonical_bytes() {
        first_missing
    } else {
        second_missing
    };
    assert_eq!(
        forward_error,
        GeometricConstraintErrorV1::MissingEdge {
            constraint: constraint_id,
            role: ConstraintEdgeRoleV1::First,
            edge: canonical_first,
        }
    );
}

#[test]
fn prepared_set_borrows_and_identifies_its_exact_source_pattern() {
    let fixture = Fixture::new();
    let prepared = prepare(&fixture, &document([])).expect("empty constraints are valid");
    assert!(std::ptr::eq(prepared.source_pattern(), &fixture.pattern));
    assert!(prepared.is_for_pattern(&fixture.pattern));

    let equal_but_distinct_pattern = fixture.pattern.clone();
    assert_eq!(equal_but_distinct_pattern, fixture.pattern);
    assert!(!prepared.is_for_pattern(&equal_but_distinct_pattern));
}

#[test]
fn bounded_direct_oracle_limit_is_the_complete_nonempty_subset_count() {
    assert_eq!(MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1, 16);
    assert_eq!(MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1, 65_535);
    assert_eq!(
        MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
        (1_usize << MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1) - 1
    );
}

#[test]
fn bounded_direct_oracle_returns_cardinality_smallest_proof_core_at_four_eight_sixteen() {
    for count in [4, 8, 16] {
        let fixture = Fixture::new();
        let mut records = vec![
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
        ];
        records.extend((3..count).map(|index| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[index % 6],
            })
        }));
        let prepared = prepare(&fixture, &document(records)).unwrap();
        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("the exact direct theorem must return a bounded oracle proof core")
        };
        assert_eq!(constraint_ids.len(), 3);
        assert!(oracle_calls <= MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1);
        for removed in &constraint_ids {
            let subset = prepared
                .constraints
                .iter()
                .filter(|record| constraint_ids.contains(&record.id) && record.id != *removed)
                .cloned()
                .collect();
            let candidate = GeometricConstraintSetV1 {
                source_pattern: &fixture.pattern,
                constraints: subset,
                raw_mirror_roles: prepared.raw_mirror_roles.clone(),
                max_preflight_checks: prepared.max_preflight_checks,
            };
            assert!(!matches!(
                preflight_direct_conflicts_v1(&candidate),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }
    }
    let fixture = Fixture::new();
    let records = (0..17).map(|index| {
        record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[index % 6],
        })
    });
    let prepared = prepare(&fixture, &document(records)).unwrap();
    assert_eq!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::Unknown { oracle_calls: 0 }
    );
}

#[test]
fn rounded_length_ratio_cause_is_bounded_at_four_eight_sixteen_and_preserved_at_seventeen() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let numerator = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 0.3,
        });
        let denominator = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: 0.1,
        });
        let ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 3.0,
        });
        let expected_ids = sorted_ids(&[numerator.id, denominator.id, ratio.id]);
        let mut records = vec![numerator, denominator, ratio];
        records.extend((3..count).map(|index| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[index % fixture.edges.len()],
            })
        }));
        let prepared =
            prepare(&fixture, &document(records)).expect("bounded rounded-residual cause");
        assert!(
            matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict {
                    ref conflicts
                } if conflicts.len() == 1
                    && matches!(
                        conflicts[0].conflict(),
                        DirectConstraintConflictKindV1::
                            LengthRatioWithIncompatibleFixedLengths { .. }
                    )
                    && conflicts[0].constraint_ids() == expected_ids
            ),
            "{count}: the direct proof itself must survive every document size"
        );

        if count == 17 {
            assert_eq!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::Unknown { oracle_calls: 0 },
                "seventeen records keep the direct proof but skip bounded minimization"
            );
            continue;
        }

        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("{count}: the rounded residual theorem must feed the bounded oracle")
        };
        assert_eq!(constraint_ids, expected_ids, "{count}");
        assert!(
            oracle_calls <= MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
            "{count}"
        );
    }
}

#[test]
fn different_ratio_product_cause_is_bounded_at_four_eight_sixteen_and_preserved_at_seventeen() {
    for count in [4, 8, 16, 17] {
        let fixture = Fixture::new();
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: 1.0,
        });
        let first = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let second = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 3.0,
        });
        let expected_ids = sorted_ids(&[fixed.id, first.id, second.id]);
        let mut records = vec![fixed, first, second];
        records.extend((3..count).map(|index| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[index % fixture.edges.len()],
            })
        }));
        let prepared = prepare(&fixture, &document(records)).expect("bounded ratio-product cause");
        assert!(
            matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict {
                    ref conflicts
                } if conflicts.len() == 1
                    && matches!(
                        conflicts[0].conflict(),
                        DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
                    )
                    && conflicts[0].constraint_ids() == expected_ids
            ),
            "{count}: the direct proof itself must survive every document size"
        );

        if count == 17 {
            assert_eq!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::Unknown { oracle_calls: 0 },
                "seventeen records keep the proof but skip bounded minimization"
            );
            continue;
        }

        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("{count}: the ratio-product theorem must feed the bounded oracle")
        };
        assert_eq!(constraint_ids, expected_ids, "{count}");
        assert!(
            oracle_calls <= MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
            "{count}"
        );
    }
}
