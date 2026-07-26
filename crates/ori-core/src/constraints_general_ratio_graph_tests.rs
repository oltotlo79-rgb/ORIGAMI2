use ori_domain::{EdgeKind, Point2};

use super::*;

pub(super) struct Fixture {
    pub(super) pattern: CreasePattern,
    pub(super) vertices: [VertexId; 24],
    pub(super) edges: [EdgeId; 12],
}

impl Fixture {
    pub(super) fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let vertex_records = vertices
            .into_iter()
            .enumerate()
            .map(|(index, id)| Vertex {
                id,
                position: Point2::new((index % 2) as f64, (index / 2) as f64),
            })
            .collect();
        let edges = std::array::from_fn(|_| EdgeId::new());
        let edge_records = edges
            .into_iter()
            .enumerate()
            .map(|(index, id)| Edge {
                id,
                start: vertices[index * 2],
                end: vertices[index * 2 + 1],
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
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
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
    .expect("directed general-ratio fixture must prepare")
}

pub(super) fn sorted_ids(ids: impl IntoIterator<Item = ConstraintId>) -> Vec<ConstraintId> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

pub(super) fn target(outcome: &ConstraintPreflightV1) -> Option<&DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        return None;
    };
    conflicts.iter().find(|conflict| {
        matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength { .. }
        )
    })
}

pub(super) fn assert_target(
    outcome: &ConstraintPreflightV1,
    fixed_edge: EdgeId,
    expected_ids: &[ConstraintId],
) {
    let conflict =
        target(outcome).unwrap_or_else(|| panic!("expected directed closure: {outcome:?}"));
    assert!(matches!(
        conflict.conflict(),
        DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
            fixed_edge: actual,
            ratio_constraint_count,
        } if *actual == fixed_edge
            && usize::from(*ratio_constraint_count) + 1 == expected_ids.len()
    ));
    assert_eq!(conflict.constraint_ids(), expected_ids);
}

pub(super) fn directed_cycle_records(
    fixture: &Fixture,
    order: [usize; 4],
    fixed_position: usize,
    fixed_length: f64,
    ratios: [f64; 4],
) -> Vec<GeometricConstraintRecordV1> {
    let mut records = vec![record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[order[fixed_position]],
        length_mm: fixed_length,
    })];
    records.extend((0..4).map(|index| {
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[order[(index + 1) % 4]],
            denominator_edge: fixture.edges[order[index]],
            ratio: ratios[index],
        })
    }));
    records
}

pub(super) fn remote_two_cycle_records(fixture: &Fixture) -> Vec<GeometricConstraintRecordV1> {
    let [root, first, second, ..] = fixture.edges;
    vec![
        record(GeometricConstraintKindV1::FixedLength {
            edge: root,
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: root,
            ratio: 1.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: second,
            denominator_edge: first,
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: first,
            denominator_edge: second,
            ratio: 0.25,
        }),
    ]
}

#[test]
fn every_four_cycle_root_and_both_directions_are_canonical_and_irredundant() {
    for order in [[0, 1, 2, 3], [0, 3, 2, 1]] {
        for fixed_position in 0..4 {
            let fixture = Fixture::new();
            let records =
                directed_cycle_records(&fixture, order, fixed_position, 1.0, [2.0, 3.0, 5.0, 0.1]);
            let expected_ids = sorted_ids(records.iter().map(|item| item.id));
            let expected = prepare(&fixture, records.clone()).preflight();
            assert_target(
                &expected,
                fixture.edges[order[fixed_position]],
                &expected_ids,
            );

            let mut reversed = records.clone();
            reversed.reverse();
            assert_eq!(prepare(&fixture, reversed).preflight(), expected);
            for removed in 0..records.len() {
                let subset = records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != removed)
                    .map(|(_, item)| item.clone());
                assert!(target(&prepare(&fixture, subset).preflight()).is_none());
            }
        }
    }

    let fixture = Fixture::new();
    let mut records = directed_cycle_records(&fixture, [0, 1, 2, 3], 0, 1.0, [2.0, 3.0, 5.0, 0.1]);
    let second_fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[2],
        length_mm: 1.0,
    });
    let first_fixed = records[0].clone();
    records.push(second_fixed.clone());
    let canonical_fixed = [first_fixed, second_fixed]
        .into_iter()
        .min_by_key(|item| item.id.canonical_bytes())
        .unwrap();
    let fixed_edge = match canonical_fixed.constraint {
        GeometricConstraintKindV1::FixedLength { edge, .. } => edge,
        _ => unreachable!(),
    };
    let expected_ids = sorted_ids(
        [canonical_fixed.id]
            .into_iter()
            .chain(records[1..5].iter().map(|item| item.id)),
    );
    assert_target(
        &prepare(&fixture, records).preflight(),
        fixed_edge,
        &expected_ids,
    );
}

fn diamond(
    fixture: &Fixture,
    root: usize,
    branches: [usize; 2],
    merge: usize,
    ratios: [f64; 4],
) -> [GeometricConstraintRecordV1; 4] {
    [
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[branches[0]],
            denominator_edge: fixture.edges[root],
            ratio: ratios[0],
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[branches[1]],
            denominator_edge: fixture.edges[root],
            ratio: ratios[1],
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[merge],
            denominator_edge: fixture.edges[branches[0]],
            ratio: ratios[2],
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[merge],
            denominator_edge: fixture.edges[branches[1]],
            ratio: ratios[3],
        }),
    ]
}

#[test]
fn diamonds_choose_cardinality_then_canonical_ids_with_duplicates_and_reordering() {
    let fixture = Fixture::new();
    let fixed = [
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        }),
    ];
    let diamonds = [
        diamond(&fixture, 0, [1, 2], 3, [2.0, 3.0, 5.0, 7.0]),
        diamond(&fixture, 0, [4, 5], 6, [11.0, 13.0, 17.0, 19.0]),
    ];
    let duplicates = diamonds.clone().map(|group| {
        group.map(|item| {
            let GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            } = item.constraint
            else {
                unreachable!()
            };
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            })
        })
    });
    let fixed_id = fixed
        .iter()
        .map(|item| item.id)
        .min_by_key(ConstraintId::canonical_bytes)
        .unwrap();
    let candidates = std::array::from_fn::<_, 2, _>(|group| {
        sorted_ids([fixed_id].into_iter().chain((0..4).map(|index| {
            [diamonds[group][index].id, duplicates[group][index].id]
                .into_iter()
                .min_by_key(ConstraintId::canonical_bytes)
                .unwrap()
        })))
    });
    let expected_ids = candidates
        .into_iter()
        .min_by(|left, right| canonical_id_slice_cmp(left, right))
        .unwrap();
    let mut records = fixed
        .into_iter()
        .chain(diamonds.into_iter().flatten())
        .chain(duplicates.into_iter().flatten())
        .collect::<Vec<_>>();
    let expected = prepare(&fixture, records.clone()).preflight();
    assert_target(&expected, fixture.edges[0], &expected_ids);
    records.reverse();
    assert_eq!(prepare(&fixture, records).preflight(), expected);
}

#[test]
fn consistent_same_depth_diamond_selects_the_canonical_path_before_a_shorter_closure() {
    let fixture = Fixture::new();
    let fixed = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[0],
        length_mm: 1.0,
    });
    let path_a = [
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[0],
            ratio: 2.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[3],
            denominator_edge: fixture.edges[1],
            ratio: 3.0,
        }),
    ];
    let path_b = [
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[2],
            denominator_edge: fixture.edges[0],
            ratio: 3.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[3],
            denominator_edge: fixture.edges[2],
            ratio: 2.0,
        }),
    ];
    let tail = [
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[4],
            denominator_edge: fixture.edges[3],
            ratio: 5.0,
        }),
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[4],
            ratio: 0.1,
        }),
    ];
    let longer = [
        (7, 0, 1.0),
        (8, 7, 2.0),
        (9, 8, 3.0),
        (10, 9, 5.0),
        (7, 10, 0.1),
    ]
    .map(|(numerator, denominator, ratio)| {
        record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[numerator],
            denominator_edge: fixture.edges[denominator],
            ratio,
        })
    });
    let expected_path = [path_a.clone(), path_b.clone()]
        .map(|path| sorted_ids(path.map(|item| item.id)))
        .into_iter()
        .min_by(|left, right| canonical_id_slice_cmp(left, right))
        .unwrap();
    let expected_ids = sorted_ids(
        [fixed.id]
            .into_iter()
            .chain(expected_path)
            .chain(tail.iter().map(|item| item.id)),
    );
    let mut records = [fixed]
        .into_iter()
        .chain(path_a)
        .chain(path_b)
        .chain(tail)
        .chain(longer)
        .collect::<Vec<_>>();
    let expected = prepare(&fixture, records.clone()).preflight();
    assert_target(&expected, fixture.edges[0], &expected_ids);
    records.reverse();
    assert_eq!(prepare(&fixture, records).preflight(), expected);
}

#[test]
fn rounded_zero_subnormal_underflow_overflow_and_nonfinite_propagation_are_exact() {
    let minimum = f64::from_bits(1);
    let cases = [
        (minimum, [1.0_f64.next_up(); 4], false),
        (minimum, [1.0, 1.0, 1.0, 2.0], true),
        (minimum, [0.5, 2.0, 1.0, 1.0], true),
        (f64::MAX, [1.0, 1.0, 1.0, 2.0], true),
        (f64::MAX, [2.0, 0.5, 1.0, 1.0], false),
    ];
    for (fixed, ratios, proven) in cases {
        let fixture = Fixture::new();
        let records = directed_cycle_records(&fixture, [0, 1, 2, 3], 0, fixed, ratios);
        let expected_ids = sorted_ids(records.iter().map(|item| item.id));
        let outcome = prepare(&fixture, records).preflight();
        if proven {
            assert_target(&outcome, fixture.edges[0], &expected_ids);
        } else {
            assert!(target(&outcome).is_none(), "{fixed:?}, {ratios:?}");
        }
    }
}
