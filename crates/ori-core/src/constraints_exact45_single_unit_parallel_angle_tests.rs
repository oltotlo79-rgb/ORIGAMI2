use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};
use ori_numeric::{
    deterministic_atan2_v1, deterministic_degrees_to_radians_v1, deterministic_hypot_v1,
};

use crate::{
    BoundedDirectMusV1, ConstraintPreflightV1, DirectConstraintConflictKindV1,
    DirectConstraintConflictV1, GeometricConstraintLimitsV1, GeometricConstraintSetV1,
    GeometricConstraintUnknownReasonV1, find_bounded_direct_mus_v1,
    prepare_geometric_constraints_v1,
};

const CENTER: usize = 0;
const A_ENDPOINT: usize = 1;
const B_ENDPOINT: usize = 2;
const EXTRA_ENDPOINT: usize = 3;
const A: usize = 0;
const B: usize = 1;
const EXTRA: usize = 2;

#[derive(Clone)]
struct Fixture {
    pattern: CreasePattern,
    vertices: [VertexId; 4],
    edges: [EdgeId; 3],
}

impl Fixture {
    fn new() -> Self {
        let vertices = std::array::from_fn(|_| VertexId::new());
        let edges = std::array::from_fn(|_| EdgeId::new());
        Self {
            pattern: CreasePattern {
                vertices: vec![
                    Vertex {
                        id: vertices[CENTER],
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: vertices[A_ENDPOINT],
                        position: Point2::new(3.0, 1.0),
                    },
                    Vertex {
                        id: vertices[B_ENDPOINT],
                        position: Point2::new(1.0, 3.0),
                    },
                    Vertex {
                        id: vertices[EXTRA_ENDPOINT],
                        position: Point2::new(-2.0, 2.0),
                    },
                ],
                edges: vec![
                    Edge {
                        id: edges[A],
                        start: vertices[CENTER],
                        end: vertices[A_ENDPOINT],
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: edges[B],
                        start: vertices[CENTER],
                        end: vertices[B_ENDPOINT],
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: edges[EXTRA],
                        start: vertices[CENTER],
                        end: vertices[EXTRA_ENDPOINT],
                        kind: EdgeKind::Auxiliary,
                    },
                ],
            },
            vertices,
            edges,
        }
    }

    fn reverse_storage(&mut self, mask: usize) {
        for index in [A, B] {
            if mask & (1 << index) != 0 {
                let edge = &mut self.pattern.edges[index];
                (edge.start, edge.end) = (edge.end, edge.start);
            }
        }
    }
}

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
    }
}

fn prepared<'a>(
    fixture: &'a Fixture,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        &fixture.pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("exact-45 single-unit fixture prepares")
}

fn core_records(fixture: &Fixture) -> Vec<GeometricConstraintRecordV1> {
    core_records_with_angle(fixture, 45.0)
}

fn core_records_with_angle(
    fixture: &Fixture,
    angle_degrees: f64,
) -> Vec<GeometricConstraintRecordV1> {
    vec![
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[CENTER],
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
            angle_degrees,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[A],
            length_mm: 1.0,
        }),
    ]
}

fn sorted_ids<'a>(
    records: impl IntoIterator<Item = &'a GeometricConstraintRecordV1>,
) -> Vec<ConstraintId> {
    let mut ids = records
        .into_iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids.dedup();
    ids
}

fn sorted_pair(fixture: &Fixture) -> [EdgeId; 2] {
    let mut edges = [fixture.edges[A], fixture.edges[B]];
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    edges
}

fn target_conflict<'a>(
    outcome: &'a ConstraintPreflightV1,
    fixture: &Fixture,
) -> Option<&'a DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        return None;
    };
    let expected = sorted_pair(fixture);
    conflicts.iter().find(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                first_edge,
                second_edge,
            } if [*first_edge, *second_edge] == expected
        )
    })
}

fn assert_solver_required(outcome: &ConstraintPreflightV1) {
    assert!(
        matches!(
            outcome,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ),
        "expected solver-required quarantine, got {outcome:?}",
    );
}

fn complete_overlay(fixture: &Fixture, a: Point2, b: Point2) -> Vec<(VertexId, Point2)> {
    vec![
        (fixture.vertices[CENTER], Point2::new(0.0, 0.0)),
        (fixture.vertices[A_ENDPOINT], a),
        (fixture.vertices[B_ENDPOINT], b),
        (fixture.vertices[EXTRA_ENDPOINT], Point2::new(-2.0, 2.0)),
    ]
}

#[test]
fn exact_three_id_core_is_canonical_and_direct_mus() {
    let fixture = Fixture::new();
    let records = core_records(&fixture);
    let expected = sorted_ids(&records);
    let prepared_set = prepared(&fixture, records.iter().cloned());
    let baseline = prepared_set.preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = &baseline else {
        panic!("exact 45-degree single-unit core must be direct");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        target_conflict(&baseline, &fixture)
            .expect("target conflict")
            .constraint_ids(),
        expected,
    );
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared_set),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == expected && oracle_calls <= 7
    ));

    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        assert_eq!(
            prepared(
                &fixture,
                order.into_iter().map(|index| records[index].clone()),
            )
            .preflight(),
            baseline,
        );
    }
}

#[test]
fn exact_one_hundred_thirty_five_three_id_core_is_canonical_and_fail_closed() {
    let fixture = Fixture::new();
    let records = core_records_with_angle(&fixture, 135.0);
    let expected = sorted_ids(&records);
    let prepared_set = prepared(&fixture, records.iter().cloned());
    let baseline = prepared_set.preflight();
    let conflict = target_conflict(&baseline, &fixture)
        .expect("exact 135-degree single-unit core must be direct");
    assert_eq!(conflict.constraint_ids(), expected);
    assert_eq!(conflict.constraint_ids().len(), 3);
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared_set),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == expected && oracle_calls <= 7
    ));

    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        assert_eq!(
            prepared(
                &fixture,
                order.into_iter().map(|index| records[index].clone()),
            )
            .preflight(),
            baseline,
        );
    }

    let mut unit_on_second_edge = records.clone();
    unit_on_second_edge[2].constraint = GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[B],
        length_mm: 1.0,
    };
    assert_eq!(
        prepared(&fixture, unit_on_second_edge).preflight(),
        baseline,
        "either member of the supplementary pair may carry the sole unit",
    );

    for mask in 0..32 {
        let mut oriented_fixture = fixture.clone();
        oriented_fixture.reverse_storage(mask);
        if mask & 16 != 0 {
            oriented_fixture.pattern.vertices.reverse();
            oriented_fixture.pattern.edges.reverse();
        }
        let mut oriented_records = records.clone();
        if mask & 4 != 0 {
            let GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } = &mut oriented_records[0].constraint
            else {
                unreachable!();
            };
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
        if mask & 8 != 0 {
            let GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                ..
            } = &mut oriented_records[1].constraint
            else {
                unreachable!();
            };
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
        assert_eq!(
            prepared(&oriented_fixture, oriented_records).preflight(),
            baseline,
        );
    }

    let second_unit = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[B],
        length_mm: 1.0,
    });
    let selected_unit = [records[2].id, second_unit.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("two unit IDs");
    let mut both_units = records.clone();
    both_units.push(second_unit);
    let mut expected_with_two_units = vec![records[0].id, records[1].id, selected_unit];
    expected_with_two_units.sort_unstable_by_key(ConstraintId::canonical_bytes);
    let both_units_outcome = prepared(&fixture, both_units.iter().cloned()).preflight();
    let both_units_conflict =
        target_conflict(&both_units_outcome, &fixture).expect("two-unit supplementary conflict");
    assert_eq!(
        both_units_conflict.constraint_ids(),
        expected_with_two_units,
    );
    assert_eq!(both_units_conflict.constraint_ids().len(), 3);
    both_units.reverse();
    assert_eq!(
        prepared(&fixture, both_units).preflight(),
        both_units_outcome,
    );

    for angle_degrees in [135.0_f64.next_down(), 135.0_f64.next_up()] {
        let mut changed = records.clone();
        let GeometricConstraintKindV1::FixedAngle {
            angle_degrees: stored,
            ..
        } = &mut changed[1].constraint
        else {
            unreachable!();
        };
        *stored = angle_degrees;
        assert_solver_required(&prepared(&fixture, changed).preflight());
    }
    for length_mm in [1.0_f64.next_down(), 1.0_f64.next_up(), 0.5, 2.0] {
        let mut changed = records.clone();
        let GeometricConstraintKindV1::FixedLength {
            length_mm: stored, ..
        } = &mut changed[2].constraint
        else {
            unreachable!();
        };
        *stored = length_mm;
        assert_solver_required(&prepared(&fixture, changed).preflight());
    }

    let mut nonstar = fixture.clone();
    nonstar.pattern.edges[B].start = nonstar.vertices[A_ENDPOINT];
    nonstar.pattern.edges[B].end = nonstar.vertices[B_ENDPOINT];
    assert!(
        prepare_geometric_constraints_v1(
            &nonstar.pattern,
            &document(core_records_with_angle(&nonstar, 135.0)),
            GeometricConstraintLimitsV1::default(),
        )
        .is_err(),
        "the supplementary fixed-angle vertex must be common to both real edges",
    );

    let reported = sorted_pair(&fixture);
    assert!(
        crate::constraints::is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
            reported[0],
            reported[1],
            expected,
            &records,
        ),
        "the independent verifier accepts the exact supplementary grammar",
    );
}

#[test]
fn both_units_choose_one_canonical_minimum_and_never_emit_the_legacy_four_id_cause() {
    let fixture = Fixture::new();
    let mut records = core_records(&fixture);
    let second_unit = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[B],
        length_mm: 1.0,
    });
    let selected_unit = [records[2].id, second_unit.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("two unit IDs");
    records.push(second_unit);
    let mut expected = vec![records[0].id, records[1].id, selected_unit];
    expected.sort_unstable_by_key(ConstraintId::canonical_bytes);

    let baseline = prepared(&fixture, records.iter().cloned()).preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = &baseline else {
        panic!("the exact-45 pair with two units remains direct");
    };
    assert_eq!(conflicts.len(), 1);
    let conflict = target_conflict(&baseline, &fixture).expect("target conflict");
    assert_eq!(conflict.constraint_ids(), expected);
    assert_eq!(conflict.constraint_ids().len(), 3);

    let mut reversed = records;
    reversed.reverse();
    assert_eq!(prepared(&fixture, reversed).preflight(), baseline);

    let mut mixed_angles = core_records(&fixture);
    mixed_angles.push(record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[B],
        length_mm: 1.0,
    }));
    mixed_angles.push(record(GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.vertices[CENTER],
        first_edge: fixture.edges[A],
        second_edge: fixture.edges[B],
        angle_degrees: 90.0,
    }));
    let mixed_outcome = prepared(&fixture, mixed_angles).preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = mixed_outcome else {
        panic!("mixed exact angles have direct evidence");
    };
    let target_causes = conflicts
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.conflict(),
                DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
            )
        })
        .map(|candidate| candidate.constraint_ids().len())
        .collect::<Vec<_>>();
    assert_eq!(
        target_causes,
        vec![3],
        "the presence of exact 45 degrees suppresses only this tag's legacy four-ID form",
    );
}

#[test]
fn edge_storage_and_constraint_operand_orientation_do_not_change_the_cause() {
    let fixture = Fixture::new();
    let records = core_records(&fixture);
    let baseline = prepared(&fixture, records.iter().cloned()).preflight();
    let mut unit_on_second_edge = records.clone();
    unit_on_second_edge[2].constraint = GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[B],
        length_mm: 1.0,
    };
    assert_eq!(
        prepared(&fixture, unit_on_second_edge).preflight(),
        baseline,
        "either member of the reported pair may carry the sole unit",
    );

    for mask in 0..32 {
        let mut oriented_fixture = fixture.clone();
        oriented_fixture.reverse_storage(mask);
        if mask & 16 != 0 {
            oriented_fixture.pattern.vertices.reverse();
            oriented_fixture.pattern.edges.reverse();
        }
        let mut oriented_records = records.clone();
        if mask & 4 != 0 {
            let GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } = &mut oriented_records[0].constraint
            else {
                unreachable!();
            };
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
        if mask & 8 != 0 {
            let GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                ..
            } = &mut oriented_records[1].constraint
            else {
                unreachable!();
            };
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
        assert_eq!(
            prepared(&oriented_fixture, oriented_records).preflight(),
            baseline,
        );
    }
}

#[test]
fn every_single_deletion_has_the_frozen_exact_residual_witness() {
    let minimum = f64::from_bits(1);
    let diagonal = f64::from_bits(0x3fe6_a09e_667f_3bcd);
    let expected_angle = deterministic_degrees_to_radians_v1(45.0).expect("frozen 45 radians");
    assert_eq!(expected_angle.to_bits(), 0x3fe9_21fb_5444_2d18);
    assert_eq!(
        deterministic_hypot_v1(diagonal, diagonal)
            .expect("finite diagonal hypot")
            .to_bits(),
        1.0_f64.to_bits(),
    );
    assert_eq!(
        deterministic_atan2_v1(diagonal, diagonal)
            .expect("finite diagonal atan2")
            .to_bits(),
        expected_angle.to_bits(),
    );
    assert_eq!(
        crate::constraints::deterministic_fixed_angle_residual_binary64_v1(expected_angle, 45.0,)
            .to_bits(),
        0,
    );

    let scale = f64::from_bits(0x5fec_0000_0000_0000);
    let square = scale * scale;
    assert_eq!(square.to_bits(), 0x7fe8_8000_0000_0000);
    let large_hypot = deterministic_hypot_v1(scale, scale).expect("finite large hypot");
    assert_eq!(large_hypot.to_bits(), 0x5ff3_cc8a_99af_5453);
    let overflow_denominator =
        deterministic_hypot_v1(scale, 0.0).expect("finite axis hypot") * large_hypot;
    assert_eq!(overflow_denominator.to_bits(), f64::INFINITY.to_bits());
    assert_eq!((square / overflow_denominator).to_bits(), 0);
    assert_eq!(
        ((-square) / overflow_denominator).to_bits(),
        (-0.0_f64).to_bits(),
    );
    let overflow_angle = deterministic_atan2_v1(square, square).expect("finite equal-term atan2");
    assert_eq!(overflow_angle.to_bits(), expected_angle.to_bits());
    assert_eq!(
        crate::constraints::deterministic_fixed_angle_residual_binary64_v1(overflow_angle, 45.0,)
            .to_bits(),
        0,
    );
    assert_eq!((minimum / 2.0).to_bits(), 0);

    for storage_mask in 0..4 {
        let mut fixture = Fixture::new();
        fixture.reverse_storage(storage_mask);
        let records = core_records(&fixture);
        for removed in 0..records.len() {
            let subset = records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != removed)
                .map(|(_, record)| record.clone())
                .collect::<Vec<_>>();
            let overlay = match removed {
                0 => complete_overlay(
                    &fixture,
                    Point2::new(1.0, 0.0),
                    Point2::new(diagonal, diagonal),
                ),
                1 => complete_overlay(&fixture, Point2::new(1.0, 0.0), Point2::new(1.0, 0.0)),
                2 => complete_overlay(&fixture, Point2::new(scale, 0.0), Point2::new(scale, scale)),
                _ => unreachable!(),
            };
            assert!(
                crate::constraint_solver::certify_binary64_residual_only_constraint_overlay_v1(
                    &fixture.pattern,
                    &document(subset),
                    &overlay,
                )
                .unwrap_or_else(|error| {
                    panic!("storage mask {storage_mask}, deletion {removed} failed: {error:?}",)
                })
                .is_some(),
                "storage mask {storage_mask}, deletion {removed} must have an exact witness",
            );
        }
    }
}

#[test]
fn supplementary_135_every_single_deletion_has_a_frozen_exact_residual_witness() {
    let diagonal = f64::from_bits(0x3fe6_a09e_667f_3bcd);
    let expected_angle = deterministic_degrees_to_radians_v1(135.0).expect("frozen 135 radians");
    assert_eq!(expected_angle.to_bits(), 0x4002_d97c_7f33_21d2);
    assert_eq!(
        deterministic_hypot_v1(-diagonal, diagonal)
            .expect("finite supplementary diagonal hypot")
            .to_bits(),
        1.0_f64.to_bits(),
    );
    assert_eq!(
        deterministic_atan2_v1(diagonal, -diagonal)
            .expect("finite supplementary diagonal atan2")
            .to_bits(),
        expected_angle.to_bits(),
    );
    assert_eq!(
        crate::constraints::deterministic_fixed_angle_residual_binary64_v1(expected_angle, 135.0,)
            .to_bits(),
        0,
    );

    let scale = f64::from_bits(0x5fec_0000_0000_0000);
    let square = scale * scale;
    let large_hypot =
        deterministic_hypot_v1(-scale, scale).expect("finite supplementary large hypot");
    let overflow_denominator =
        deterministic_hypot_v1(scale, 0.0).expect("finite axis hypot") * large_hypot;
    assert_eq!(overflow_denominator.to_bits(), f64::INFINITY.to_bits());
    assert_eq!((square / overflow_denominator).to_bits(), 0);
    let overflow_angle =
        deterministic_atan2_v1(square, -square).expect("finite supplementary equal-term atan2");
    assert_eq!(overflow_angle.to_bits(), expected_angle.to_bits());
    assert_eq!(
        crate::constraints::deterministic_fixed_angle_residual_binary64_v1(overflow_angle, 135.0,)
            .to_bits(),
        0,
    );

    for storage_mask in 0..4 {
        let mut fixture = Fixture::new();
        fixture.reverse_storage(storage_mask);
        let records = core_records_with_angle(&fixture, 135.0);
        for removed in 0..records.len() {
            let subset = records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != removed)
                .map(|(_, record)| record.clone())
                .collect::<Vec<_>>();
            let overlay = match removed {
                0 => complete_overlay(
                    &fixture,
                    Point2::new(1.0, 0.0),
                    Point2::new(-diagonal, -diagonal),
                ),
                1 => complete_overlay(&fixture, Point2::new(1.0, 0.0), Point2::new(1.0, 0.0)),
                2 => complete_overlay(
                    &fixture,
                    Point2::new(scale, 0.0),
                    Point2::new(-scale, -scale),
                ),
                _ => unreachable!(),
            };
            assert!(
                crate::constraint_solver::certify_binary64_residual_only_constraint_overlay_v1(
                    &fixture.pattern,
                    &document(subset),
                    &overlay,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "supplementary storage mask {storage_mask}, deletion {removed} failed: {error:?}",
                    )
                })
                .is_some(),
                "supplementary storage mask {storage_mask}, deletion {removed} needs an exact witness",
            );
        }
    }
}

#[test]
fn exact_angle_and_unit_bits_are_required_by_the_three_id_scanner() {
    let fixture = Fixture::new();
    for angle_degrees in [45.0_f64.next_down(), 45.0_f64.next_up()] {
        let mut records = core_records(&fixture);
        let GeometricConstraintKindV1::FixedAngle {
            angle_degrees: stored,
            ..
        } = &mut records[1].constraint
        else {
            unreachable!();
        };
        *stored = angle_degrees;
        assert_solver_required(&prepared(&fixture, records).preflight());
    }
    for length_mm in [1.0_f64.next_down(), 1.0_f64.next_up()] {
        let mut records = core_records(&fixture);
        let GeometricConstraintKindV1::FixedLength {
            length_mm: stored, ..
        } = &mut records[2].constraint
        else {
            unreachable!();
        };
        *stored = length_mm;
        assert_solver_required(&prepared(&fixture, records).preflight());
    }

    let mut wrong_edge = core_records(&fixture);
    wrong_edge[2].constraint = GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[EXTRA],
        length_mm: 1.0,
    };
    assert_solver_required(&prepared(&fixture, wrong_edge).preflight());
}

#[test]
fn duplicates_and_unrelated_extras_preserve_only_the_canonical_three_ids() {
    let fixture = Fixture::new();
    let originals = core_records(&fixture);
    let parallel_duplicate = record(originals[0].constraint.clone());
    let angle_duplicate = record(originals[1].constraint.clone());
    let first_unit_duplicate = record(originals[2].constraint.clone());
    let second_unit = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[B],
        length_mm: 1.0,
    });
    let unrelated = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[EXTRA],
        length_mm: 1.0,
    });
    let unrelated_id = unrelated.id;

    let parallel_id = [originals[0].id, parallel_duplicate.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("parallel IDs");
    let angle_id = [originals[1].id, angle_duplicate.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("angle IDs");
    let unit_id = [originals[2].id, first_unit_duplicate.id, second_unit.id]
        .into_iter()
        .min_by_key(ConstraintId::canonical_bytes)
        .expect("unit IDs");
    let mut expected = vec![parallel_id, angle_id, unit_id];
    expected.sort_unstable_by_key(ConstraintId::canonical_bytes);

    let mut records = originals.clone();
    records.extend([
        parallel_duplicate,
        angle_duplicate,
        first_unit_duplicate,
        second_unit,
        unrelated,
    ]);
    let baseline = prepared(&fixture, records.iter().cloned()).preflight();
    let conflict = target_conflict(&baseline, &fixture).expect("canonical target conflict");
    assert_eq!(conflict.constraint_ids(), expected);
    assert_eq!(conflict.constraint_ids().len(), 3);
    assert!(!conflict.constraint_ids().contains(&unrelated_id));

    records.reverse();
    assert_eq!(prepared(&fixture, records).preflight(), baseline);
}

#[test]
fn malformed_topology_references_ids_and_cause_shapes_fail_closed() {
    let fixture = Fixture::new();
    let records = core_records(&fixture);

    let mut duplicate_ids = records.clone();
    duplicate_ids[1].id = duplicate_ids[0].id;
    assert!(
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            &document(duplicate_ids),
            GeometricConstraintLimitsV1::default(),
        )
        .is_err(),
        "duplicate constraint IDs fail before direct scanning",
    );

    let mut nonstar = fixture.clone();
    nonstar.pattern.edges[B].start = nonstar.vertices[A_ENDPOINT];
    nonstar.pattern.edges[B].end = nonstar.vertices[B_ENDPOINT];
    assert!(
        prepare_geometric_constraints_v1(
            &nonstar.pattern,
            &document(core_records(&nonstar)),
            GeometricConstraintLimitsV1::default(),
        )
        .is_err(),
        "the fixed-angle vertex must be common to both real edges",
    );

    let mut nonexistent = records.clone();
    let ghost = EdgeId::new();
    let GeometricConstraintKindV1::Parallel { second_edge, .. } = &mut nonexistent[0].constraint
    else {
        unreachable!();
    };
    *second_edge = ghost;
    assert!(
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            &document(nonexistent),
            GeometricConstraintLimitsV1::default(),
        )
        .is_err(),
        "nonexistent edge references fail before direct scanning",
    );

    let reported = sorted_pair(&fixture);
    let valid_ids = sorted_ids(&records);
    assert!(
        crate::constraints::is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
            reported[0],
            reported[1],
            valid_ids.clone(),
            &records,
        ),
        "the independent verifier accepts the exact grammar",
    );
    let extra = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[EXTRA],
        length_mm: 1.0,
    });
    let mut records_with_extra = records.clone();
    records_with_extra.push(extra);
    let extra_ids = sorted_ids(&records_with_extra);
    assert!(
        !crate::constraints::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
                reported[0],
                reported[1],
                extra_ids,
                &records_with_extra,
            ),
        "an unrelated fourth cause cannot enter the exact grammar",
    );
    let mut wrong_angle = records.clone();
    let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } =
        &mut wrong_angle[1].constraint
    else {
        unreachable!();
    };
    *angle_degrees = 45.0_f64.next_up();
    assert!(
        !crate::constraints::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
                reported[0],
                reported[1],
                valid_ids.clone(),
                &wrong_angle,
            ),
        "the verifier independently rechecks the exact angle bits",
    );
    let mut reversed_ids = valid_ids.clone();
    reversed_ids.reverse();
    assert!(
        !crate::constraints::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
                reported[0],
                reported[1],
                reversed_ids,
                &records,
            ),
        "noncanonical cause ordering is rejected",
    );
    assert!(
        !crate::constraints::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
                reported[1],
                reported[0],
                valid_ids.clone(),
                &records,
            ),
        "noncanonical reported edge ordering is rejected",
    );
    let mut duplicate_cause = valid_ids.clone();
    duplicate_cause[2] = duplicate_cause[1];
    duplicate_cause.sort_unstable_by_key(ConstraintId::canonical_bytes);
    assert!(
        !crate::constraints::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
                reported[0],
                reported[1],
                duplicate_cause,
                &records,
            ),
        "duplicate cause IDs are rejected independently",
    );
    let mut nonexistent_cause = valid_ids;
    nonexistent_cause[0] = ConstraintId::new();
    nonexistent_cause.sort_unstable_by_key(ConstraintId::canonical_bytes);
    assert!(
        !crate::constraints::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
                reported[0],
                reported[1],
                nonexistent_cause,
                &records,
            ),
        "cause IDs absent from the prepared record set are rejected",
    );
}

#[test]
fn non_exact_single_unit_angles_retain_the_legacy_two_unit_four_id_boundary() {
    let fixture = Fixture::new();
    for angle_degrees in [
        90.0,
        45.0_f64.next_down(),
        45.0_f64.next_up(),
        135.0_f64.next_down(),
        135.0_f64.next_up(),
    ] {
        let mut records = core_records(&fixture);
        let GeometricConstraintKindV1::FixedAngle {
            angle_degrees: stored,
            ..
        } = &mut records[1].constraint
        else {
            unreachable!();
        };
        *stored = angle_degrees;

        assert_solver_required(&prepared(&fixture, records.iter().cloned()).preflight());

        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[B],
            length_mm: 1.0,
        }));
        let expected = sorted_ids(&records);
        let outcome = prepared(&fixture, records).preflight();
        let conflict = target_conflict(&outcome, &fixture)
            .expect("the generic two-unit branch remains direct");
        assert_eq!(conflict.constraint_ids(), expected);
        assert_eq!(conflict.constraint_ids().len(), 4);
    }
}
