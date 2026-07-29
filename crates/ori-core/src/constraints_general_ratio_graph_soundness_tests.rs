use super::general_ratio_graph_tests::{
    Fixture, assert_target, directed_cycle_records, document, prepare, record,
    remote_two_cycle_records, sorted_ids, target,
};
use super::*;
use crate::certify_binary64_exact_geometric_constraint_satisfaction_v1;

fn cross_root_target(outcome: &ConstraintPreflightV1) -> Option<&DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        return None;
    };
    conflicts.iter().find(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::InconsistentLengthRatioGraphBetweenFixedLengths { .. }
        )
    })
}

fn assert_cross_root_target(
    outcome: &ConstraintPreflightV1,
    fixed_edges: [EdgeId; 2],
    expected_ids: &[ConstraintId],
) {
    let conflict = cross_root_target(outcome)
        .unwrap_or_else(|| panic!("expected cross-root directed closure: {outcome:?}"));
    let mut expected_edges = fixed_edges;
    expected_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    assert!(matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::InconsistentLengthRatioGraphBetweenFixedLengths {
            first_fixed_edge,
            second_fixed_edge,
            ratio_constraint_count: 2,
        } if [*first_fixed_edge, *second_fixed_edge] == expected_edges
    ));
    assert_eq!(conflict.constraint_ids(), expected_ids);
}

fn cross_root_records(
    fixture: &Fixture,
    first_fixed: f64,
    second_fixed: f64,
    first_ratio: (usize, usize, f64),
    second_ratio: (usize, usize, f64),
) -> Vec<GeometricConstraintRecordV1> {
    vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: first_fixed,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: second_fixed,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[first_ratio.0],
            denominator_edge: fixture.edges[first_ratio.1],
            ratio: first_ratio.2,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[second_ratio.0],
            denominator_edge: fixture.edges[second_ratio.1],
            ratio: second_ratio.2,
        }),
    ]
}

#[test]
fn two_fixed_roots_with_disjoint_domains_form_a_canonical_four_id_mus() {
    let fixture = Fixture::new();
    let records = cross_root_records(&fixture, 1.0, 1.0, (2, 0, 2.0), (2, 1, 3.0));
    let expected_ids = sorted_ids(records.iter().map(|record| record.id));
    let prepared = prepare(&fixture, records.clone());
    assert_cross_root_target(
        &prepared.preflight(),
        [fixture.edges[0], fixture.edges[1]],
        &expected_ids,
    );
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            ref constraint_ids,
            ..
        } if constraint_ids == &expected_ids
    ));

    for removed in 0..records.len() {
        let subset = records
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed)
            .map(|(_, record)| record.clone());
        assert!(
            cross_root_target(&prepare(&fixture, subset).preflight()).is_none(),
            "every one-record deletion must remove the cross-root proof"
        );
    }

    let mut reversed = records;
    reversed.reverse();
    assert_cross_root_target(
        &prepare(&fixture, reversed).preflight(),
        [fixture.edges[0], fixture.edges[1]],
        &expected_ids,
    );
}

#[test]
fn cross_root_domains_cover_forward_reverse_direction_combinations() {
    for (first_ratio, second_ratio) in [
        ((2, 0, 2.0), (2, 1, 3.0)),
        ((2, 0, 2.0), (1, 2, 3.0)),
        ((0, 2, 2.0), (1, 2, 3.0)),
    ] {
        let fixture = Fixture::new();
        let records = cross_root_records(&fixture, 1.0, 1.0, first_ratio, second_ratio);
        let expected_ids = sorted_ids(records.iter().map(|record| record.id));
        assert_cross_root_target(
            &prepare(&fixture, records).preflight(),
            [fixture.edges[0], fixture.edges[1]],
            &expected_ids,
        );
    }
}

#[test]
fn cross_root_domains_distinguish_one_ulp_maximum_and_subnormal_values() {
    for (first_fixed, second_fixed) in [
        (1.0, 1.0_f64.next_up()),
        (f64::MAX, f64::from_bits(f64::MAX.to_bits() - 1)),
        (f64::from_bits(1), f64::from_bits(2)),
    ] {
        let fixture = Fixture::new();
        let records = cross_root_records(
            &fixture,
            first_fixed,
            second_fixed,
            (2, 0, 1.0),
            (2, 1, 1.0),
        );
        let expected_ids = sorted_ids(records.iter().map(|record| record.id));
        assert_cross_root_target(
            &prepare(&fixture, records).preflight(),
            [fixture.edges[0], fixture.edges[1]],
            &expected_ids,
        );
    }
}

#[test]
fn cross_root_rounding_aliases_and_overflow_remain_fail_closed() {
    let minimum = f64::from_bits(1);
    let mut alias_fixture = Fixture::new();
    for edge_index in 0..3 {
        let end = alias_fixture.pattern.edges[edge_index].end;
        alias_fixture
            .pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == end)
            .unwrap()
            .position
            .x = minimum;
    }
    let aliases = cross_root_records(
        &alias_fixture,
        minimum,
        minimum,
        (2, 0, 1.0_f64.next_up()),
        (2, 1, f64::from_bits(1.0_f64.to_bits() - 1)),
    );
    assert!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &alias_fixture.pattern,
            &document(aliases.clone()),
        )
        .expect("alias fixture must be valid")
        .is_some(),
        "both distinct ratio products round to the same minimum subnormal"
    );
    assert!(
        cross_root_target(&prepare(&alias_fixture, aliases).preflight()).is_none(),
        "overlapping conservative domains must never be promoted"
    );

    let overflow_fixture = Fixture::new();
    let overflow = cross_root_records(
        &overflow_fixture,
        f64::MAX,
        f64::MAX,
        (2, 0, 2.0),
        (2, 1, 1.0),
    );
    assert!(
        cross_root_target(&prepare(&overflow_fixture, overflow).preflight()).is_none(),
        "a non-finite forward image is deliberately left unknown"
    );

    let multistep_fixture = Fixture::new();
    let multistep_overflow = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: multistep_fixture.edges[0],
            length_mm: f64::MAX / 2.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: multistep_fixture.edges[1],
            length_mm: f64::MAX,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: multistep_fixture.edges[2],
            denominator_edge: multistep_fixture.edges[0],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: multistep_fixture.edges[3],
            denominator_edge: multistep_fixture.edges[2],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: multistep_fixture.edges[3],
            denominator_edge: multistep_fixture.edges[1],
            ratio: 1.0,
        }),
    ];
    assert!(
        cross_root_target(&prepare(&multistep_fixture, multistep_overflow).preflight()).is_none(),
        "proof-local replay must also reject an overflow reached after a finite step"
    );
}

#[test]
fn one_ratio_between_fixed_roots_remains_owned_by_the_existing_pair_family() {
    let fixture = Fixture::new();
    let records = vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: 3.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[0],
            ratio: 2.0,
        }),
    ];
    let outcome = prepare(&fixture, records).preflight();
    assert!(cross_root_target(&outcome).is_none());
    assert!(matches!(
        outcome,
        ConstraintPreflightV1::DirectConflict { ref conflicts }
            if conflicts.iter().any(|conflict| matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths {
                    numerator_edge,
                    denominator_edge,
                } if *numerator_edge == fixture.edges[1]
                    && *denominator_edge == fixture.edges[0]
            ))
    ));
}

#[test]
fn roots_are_combined_only_when_their_binary64_domains_are_disjoint() {
    let fixture = Fixture::new();
    let [
        first_root,
        second_root,
        merge,
        cycle_a,
        cycle_b,
        unrelated,
        ..,
    ] = fixture.edges;
    let disjoint_roots = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: first_root,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: second_root,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: merge,
            denominator_edge: first_root,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: merge,
            denominator_edge: second_root,
            ratio: 3.0,
        }),
    ];
    assert!(
        cross_root_target(&prepare(&fixture, disjoint_roots).preflight()).is_some(),
        "the two roots force the merge edge to exactly disjoint values"
    );

    let mut inconsistent =
        directed_cycle_records(&fixture, [0, 1, 2, 3], 0, 1.0, [2.0, 3.0, 5.0, 0.1]);
    inconsistent.push(record(GeometricConstraintKindV1::LengthRatio {
        numerator_edge: fixture.edges[3],
        denominator_edge: fixture.edges[2],
        ratio: 7.0,
    }));
    assert!(target(&prepare(&fixture, inconsistent).preflight()).is_none());

    let mut inconsistent_fixed =
        directed_cycle_records(&fixture, [0, 1, 2, 3], 0, 1.0, [2.0, 3.0, 5.0, 0.1]);
    inconsistent_fixed.push(record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 2.0,
    }));
    assert!(target(&prepare(&fixture, inconsistent_fixed).preflight()).is_none());

    let disconnected = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: unrelated,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: cycle_b,
            denominator_edge: cycle_a,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: cycle_a,
            denominator_edge: cycle_b,
            ratio: 0.25,
        }),
    ];
    assert!(target(&prepare(&fixture, disconnected).preflight()).is_none());

    let reverse_only = remote_two_cycle_records(&fixture)
        .into_iter()
        .map(|mut item| {
            if let GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            } = item.constraint
            {
                item.constraint = GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: denominator_edge,
                    denominator_edge: numerator_edge,
                    ratio: ratio.recip(),
                };
            }
            item
        })
        .collect::<Vec<_>>();
    let expected_ids = sorted_ids(reverse_only.iter().map(|record| record.id));
    assert_target(
        &prepare(&fixture, reverse_only).preflight(),
        fixture.edges[0],
        &expected_ids,
    );
}

#[test]
fn traversing_the_same_binary64_ratio_backward_then_forward_never_conflicts() {
    let fixture = Fixture::new();
    let root = fixture.edges[0];
    let denominator = fixture.edges[1];
    let round_trip = fixture.edges[2];
    let records = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: root,
            length_mm: 7.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: root,
            denominator_edge: denominator,
            ratio: 11.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: round_trip,
            denominator_edge: denominator,
            ratio: 11.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: root,
            denominator_edge: round_trip,
            ratio: 1.0,
        }),
    ];
    assert!(
        target(&prepare(&fixture, records).preflight()).is_none(),
        "the conservative inverse followed by the identical production \
         multiplication must retain its source value"
    );
}
