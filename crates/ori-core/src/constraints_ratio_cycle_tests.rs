use ori_domain::{EdgeKind, Point2};

use super::*;

pub(super) struct Fixture {
    pub(super) pattern: CreasePattern,
    pub(super) vertices: [VertexId; 8],
    pub(super) edges: [EdgeId; 4],
}

impl Fixture {
    pub(super) fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 2.0),
            Point2::new(1.0, 2.0),
            Point2::new(0.0, 4.0),
            Point2::new(1.0, 4.0),
            Point2::new(0.0, 6.0),
            Point2::new(1.0, 6.0),
        ];
        let vertex_records = vertices
            .into_iter()
            .zip(positions)
            .map(|(id, position)| Vertex { id, position })
            .collect();
        let edges = std::array::from_fn(|_| EdgeId::new());
        let edge_records = edges
            .into_iter()
            .zip([(0, 1), (2, 3), (4, 5), (6, 7)])
            .map(|(id, (start, end))| Edge {
                id,
                start: vertices[start],
                end: vertices[end],
                kind: EdgeKind::Auxiliary,
            })
            .collect();
        Self {
            pattern: CreasePattern {
                vertices: vertex_records,
                edges: edge_records,
            },
            vertices,
            edges,
        }
    }
}

pub(super) fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

pub(super) fn document(
    constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: constraints.into_iter().collect(),
    }
}

pub(super) fn prepare<'a>(
    fixture: &'a Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("directed ratio-cycle fixture must prepare")
}

pub(super) fn sorted_ids(ids: impl IntoIterator<Item = ConstraintId>) -> Vec<ConstraintId> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

pub(super) fn core_records(
    fixture: &Fixture,
    cycle: [usize; 3],
    fixed_index: usize,
    fixed_length: f64,
    ratios: [f64; 3],
) -> Vec<GeometricConstraintRecordV1> {
    let edges = cycle.map(|index| fixture.edges[index]);
    vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: edges[fixed_index],
            length_mm: fixed_length,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edges[0],
            denominator_edge: edges[1],
            ratio: ratios[0],
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edges[1],
            denominator_edge: edges[2],
            ratio: ratios[1],
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edges[2],
            denominator_edge: edges[0],
            ratio: ratios[2],
        }),
    ]
}

pub(super) fn target_conflict(
    outcome: &ConstraintPreflightV1,
) -> Option<&DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        return None;
    };
    conflicts.iter().find(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength { .. }
        )
    })
}

fn canonical_cycle(fixture: &Fixture, cycle: [usize; 3]) -> [EdgeId; 3] {
    let start = (0..3)
        .min_by_key(|index| fixture.edges[cycle[*index]].canonical_bytes())
        .unwrap();
    std::array::from_fn(|offset| fixture.edges[cycle[(start + offset) % 3]])
}

pub(super) fn assert_single_target(
    outcome: &ConstraintPreflightV1,
    fixture: &Fixture,
    cycle: [usize; 3],
    fixed_index: usize,
    expected_ids: &[ConstraintId],
) {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        panic!("expected one binary64 cycle-closure proof: {outcome:?}");
    };
    assert_eq!(conflicts.len(), 1);
    let expected_cycle = canonical_cycle(fixture, cycle);
    let fixed_edge = fixture.edges[cycle[fixed_index]];
    assert!(matches!(
        conflicts[0].conflict(),
        DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength {
            first_edge,
            second_edge,
            third_edge,
            fixed_edge: actual_fixed,
        } if [*first_edge, *second_edge, *third_edge] == expected_cycle
            && *actual_fixed == fixed_edge
    ));
    assert_eq!(conflicts[0].constraint_ids(), expected_ids);
}

#[test]
fn every_fixed_vertex_and_both_cycle_orientations_are_canonical_and_irredundant() {
    let fixture = Fixture::new();
    for cycle in [[0, 1, 2], [0, 2, 1]] {
        for fixed_index in 0..3 {
            let records = core_records(&fixture, cycle, fixed_index, 1.0, [2.0, 3.0, 0.25]);
            let expected_ids = sorted_ids(records.iter().map(|item| item.id));
            let expected = prepare(&fixture, records.clone()).preflight();
            assert_single_target(&expected, &fixture, cycle, fixed_index, &expected_ids);

            let mut reordered = records.clone();
            reordered.reverse();
            assert_eq!(prepare(&fixture, reordered).preflight(), expected);
            for removed in 0..records.len() {
                let subset = records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != removed)
                    .map(|(_, item)| item.clone());
                assert!(target_conflict(&prepare(&fixture, subset).preflight()).is_none());
            }
        }
    }
}

#[test]
fn rounded_zero_underflow_and_overflow_follow_only_production_steps() {
    let minimum = f64::from_bits(1);
    let one_up = 1.0_f64.next_up();
    for fixed_index in 0..3 {
        for (fixed, ratios, proven) in
            [(minimum, [one_up; 3], false), (minimum, [minimum; 3], true)]
        {
            let residual =
                length_ratio_cycle_closure_residual_binary64_v1(fixed_index, fixed, ratios);
            assert_eq!(residual != 0.0, proven);
            let fixture = Fixture::new();
            let records = core_records(&fixture, [0, 1, 2], fixed_index, fixed, ratios);
            let ids = sorted_ids(records.iter().map(|item| item.id));
            let outcome = prepare(&fixture, records).preflight();
            if proven {
                assert_single_target(&outcome, &fixture, [0, 1, 2], fixed_index, &ids);
            } else {
                assert!(matches!(
                    outcome,
                    ConstraintPreflightV1::Unknown {
                        reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                        ..
                    }
                ));
            }
        }

        let mut underflow_ratios = [1.0; 3];
        underflow_ratios[(fixed_index + 2) % 3] = 0.5;
        underflow_ratios[fixed_index] = 2.0;
        let underflow =
            length_ratio_cycle_closure_residual_binary64_v1(fixed_index, minimum, underflow_ratios);
        assert_ne!(underflow, 0.0, "an exact-unit product can underflow");

        let mut overflow_ratios = [1.0; 3];
        overflow_ratios[(fixed_index + 2) % 3] = 2.0;
        overflow_ratios[fixed_index] = 0.5;
        let overflow =
            length_ratio_cycle_closure_residual_binary64_v1(fixed_index, f64::MAX, overflow_ratios);
        assert!(!overflow.is_finite());

        for (fixed, ratios) in [(minimum, underflow_ratios), (f64::MAX, overflow_ratios)] {
            let fixture = Fixture::new();
            let records = core_records(&fixture, [0, 1, 2], fixed_index, fixed, ratios);
            let ids = sorted_ids(records.iter().map(|item| item.id));
            assert_single_target(
                &prepare(&fixture, records).preflight(),
                &fixture,
                [0, 1, 2],
                fixed_index,
                &ids,
            );
        }
    }
}

#[test]
fn duplicate_ratio_and_multiple_fixed_groups_choose_one_canonical_witness() {
    let fixture = Fixture::new();
    let [first, second, third, _] = fixture.edges;
    let fixed = [first, second, third].map(|edge| {
        record(GeometricConstraintKindV1::FixedLength {
            edge,
            length_mm: 1.0,
        })
    });
    let directed = [
        (first, second, 2.0),
        (second, third, 3.0),
        (third, first, 0.25),
    ];
    let ratio_groups = directed.map(|(numerator_edge, denominator_edge, ratio)| {
        std::array::from_fn::<_, 2, _>(|_| {
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            })
        })
    });
    let fixed_witness = fixed
        .iter()
        .min_by_key(|item| item.id.canonical_bytes())
        .unwrap();
    let fixed_edge = match fixed_witness.constraint {
        GeometricConstraintKindV1::FixedLength { edge, .. } => edge,
        _ => unreachable!(),
    };
    let expected_ids = sorted_ids(
        [fixed_witness.id]
            .into_iter()
            .chain(ratio_groups.iter().map(|group| {
                group
                    .iter()
                    .map(|item| item.id)
                    .min_by_key(ConstraintId::canonical_bytes)
                    .unwrap()
            })),
    );
    let mut records = fixed
        .into_iter()
        .chain(ratio_groups.into_iter().flatten())
        .collect::<Vec<_>>();
    let expected = prepare(&fixture, records.clone()).preflight();
    let target = target_conflict(&expected).expect("cycle proof must be present");
    assert_eq!(target.constraint_ids(), expected_ids);
    assert!(matches!(
        target.conflict(),
        DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength {
            fixed_edge: actual,
            ..
        } if *actual == fixed_edge
    ));
    records.reverse();
    assert_eq!(prepare(&fixture, records).preflight(), expected);
}

#[test]
fn inconsistent_groups_and_inexact_edge_cycle_never_form_a_target_proof() {
    let fixture = Fixture::new();
    for group in 0..3 {
        let fixed_index = (group + 2) % 3;
        let mut records = core_records(&fixture, [0, 1, 2], fixed_index, 1.0, [2.0, 3.0, 0.25]);
        let numerator_edge = fixture.edges[group];
        let denominator_edge = fixture.edges[(group + 1) % 3];
        records.push(record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio: 7.0,
        }));
        assert!(target_conflict(&prepare(&fixture, records).preflight()).is_none());
    }
    for fixed_index in 0..3 {
        let mut records = core_records(&fixture, [0, 1, 2], fixed_index, 1.0, [2.0, 3.0, 0.25]);
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[fixed_index],
            length_mm: 2.0,
        }));
        assert!(target_conflict(&prepare(&fixture, records).preflight()).is_none());
    }

    let [first, second, third, unrelated] = fixture.edges;
    let inexact = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: first,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: second,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: second,
            denominator_edge: third,
            ratio: 3.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: third,
            denominator_edge: unrelated,
            ratio: 0.25,
        }),
    ];
    assert!(target_conflict(&prepare(&fixture, inexact).preflight()).is_none());
}
