use std::collections::BTreeSet;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};
use ori_numeric::{deterministic_atan2_v1, deterministic_hypot_v1};

use crate::constraints::{
    charge_unit_terminal_angle_parallel_work_for_test_v1,
    replace_unit_terminal_angle_parallel_test_limits_v1,
    reserve_unit_terminal_angle_parallel_storage_for_test_v1,
    unit_terminal_angle_parallel_test_observed_v1,
};
use crate::{
    BoundedDirectMusV1, ConstraintPreflightV1, DirectConstraintConflictKindV1,
    DirectConstraintConflictV1, GeometricConstraintLimitsV1, GeometricConstraintSetV1,
    GeometricConstraintUnknownReasonV1, find_bounded_direct_mus_v1,
    prepare_geometric_constraints_v1,
};

const A: usize = 0;
const M: usize = 1;
const B: usize = 2;
const N: usize = 3;
const EXTRA: usize = 4;

struct UnitTerminalAngleParallelTestLimitsGuard {
    previous: (Option<u64>, Option<usize>),
}

impl UnitTerminalAngleParallelTestLimitsGuard {
    fn reset() -> Self {
        Self {
            previous: replace_unit_terminal_angle_parallel_test_limits_v1((None, None)),
        }
    }
}

impl Drop for UnitTerminalAngleParallelTestLimitsGuard {
    fn drop(&mut self) {
        replace_unit_terminal_angle_parallel_test_limits_v1(self.previous);
    }
}

#[derive(Clone)]
struct Fixture {
    pattern: CreasePattern,
    center: VertexId,
    endpoints: [VertexId; 5],
    edges: [EdgeId; 5],
}

impl Fixture {
    fn new() -> Self {
        let center = VertexId::new();
        let endpoints = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let edges = [
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
        ];
        let positions = [(3.0, 1.0), (2.0, 2.0), (1.0, 3.0), (-1.0, 3.0), (-2.0, 2.0)];
        let mut vertices = vec![Vertex {
            id: center,
            position: Point2::new(0.0, 0.0),
        }];
        vertices.extend(
            endpoints
                .into_iter()
                .zip(positions)
                .map(|(id, (x, y))| Vertex {
                    id,
                    position: Point2::new(x, y),
                }),
        );
        Self {
            pattern: CreasePattern {
                vertices,
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
            },
            center,
            endpoints,
            edges,
        }
    }

    fn reverse_storage(&mut self, mask: usize) {
        for (index, edge) in self.pattern.edges.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
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
    .expect("terminal-unit two-hop fixture prepares")
}

fn core_records(fixture: &Fixture) -> Vec<GeometricConstraintRecordV1> {
    vec![
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[M],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[M],
            second_edge: fixture.edges[B],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.center,
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
            angle_degrees: 90.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[A],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[B],
            length_mm: 1.0,
        }),
    ]
}

fn sorted_ids(records: impl IntoIterator<Item = GeometricConstraintRecordV1>) -> Vec<ConstraintId> {
    let mut ids = records
        .into_iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids.dedup();
    ids
}

fn target_conflict<'a>(
    outcome: &'a ConstraintPreflightV1,
    fixture: &Fixture,
) -> Option<&'a DirectConstraintConflictV1> {
    let ConstraintPreflightV1::DirectConflict { conflicts } = outcome else {
        return None;
    };
    let mut expected_edges = [
        fixture.edges[A].canonical_bytes(),
        fixture.edges[B].canonical_bytes(),
    ];
    expected_edges.sort_unstable();
    conflicts.iter().find(|candidate| {
        let DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
            vertex,
            first_edge,
            second_edge,
            parallel_constraint_count,
        } = candidate.conflict()
        else {
            return false;
        };
        *vertex == fixture.center
            && *parallel_constraint_count == 2
            && [first_edge.canonical_bytes(), second_edge.canonical_bytes()] == expected_edges
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

fn index_permutations() -> Vec<[usize; 5]> {
    fn visit(index: usize, values: &mut [usize; 5], output: &mut Vec<[usize; 5]>) {
        if index == values.len() {
            output.push(*values);
            return;
        }
        for swap in index..values.len() {
            values.swap(index, swap);
            visit(index + 1, values, output);
            values.swap(index, swap);
        }
    }

    let mut values = [0, 1, 2, 3, 4];
    let mut output = Vec::new();
    visit(0, &mut values, &mut output);
    output
}

#[test]
fn exact_five_id_core_is_direct_canonical_and_deletion_minimal() {
    let fixture = Fixture::new();
    let records = core_records(&fixture);
    let expected = sorted_ids(records.iter().cloned());
    assert_eq!(
        BTreeSet::from([
            fixture.edges[A].canonical_bytes(),
            fixture.edges[M].canonical_bytes(),
            fixture.edges[B].canonical_bytes(),
        ])
        .len(),
        3,
    );
    let prepared_set = prepared(&fixture, records.iter().cloned());
    let outcome = prepared_set.preflight();
    assert_eq!(
        target_conflict(&outcome, &fixture)
            .expect("exact terminal-unit theorem must be emitted")
            .constraint_ids(),
        expected,
    );
    assert!(matches!(
        find_bounded_direct_mus_v1(&prepared_set),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } if constraint_ids == expected && oracle_calls <= 31
    ));

    for removed in &records {
        let subset = records
            .iter()
            .filter(|record| record.id != removed.id)
            .cloned()
            .collect::<Vec<_>>();
        let outcome = prepared(&fixture, subset).preflight();
        assert!(
            target_conflict(&outcome, &fixture).is_none(),
            "removing any one of the five causes must remove this theorem",
        );
        assert_solver_required(&outcome);
    }
}

#[test]
fn every_input_and_constraint_id_permutation_preserves_the_canonical_conflict() {
    let fixture = Fixture::new();
    let records = core_records(&fixture);
    let baseline = prepared(&fixture, records.iter().cloned()).preflight();
    let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();

    for permutation in index_permutations() {
        let input_permuted = permutation
            .into_iter()
            .map(|index| records[index].clone())
            .collect::<Vec<_>>();
        assert_eq!(prepared(&fixture, input_permuted).preflight(), baseline);

        let mut id_permuted = records.clone();
        for (role, id_source) in permutation.into_iter().enumerate() {
            id_permuted[role].id = ids[id_source];
        }
        assert_eq!(prepared(&fixture, id_permuted).preflight(), baseline);
    }
}

#[test]
fn edge_storage_and_constraint_operand_orientation_are_irrelevant() {
    let fixture = Fixture::new();
    let records = core_records(&fixture);
    let baseline = prepared(&fixture, records.iter().cloned()).preflight();

    for mask in 0..8 {
        let mut oriented_fixture = fixture.clone();
        oriented_fixture.reverse_storage(mask);
        if mask & 1 != 0 {
            oriented_fixture.pattern.vertices.reverse();
            oriented_fixture.pattern.edges.reverse();
        }
        let mut oriented_records = records.clone();
        if mask & 1 != 0 {
            let GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } = &mut oriented_records[0].constraint
            else {
                unreachable!();
            };
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
        if mask & 2 != 0 {
            let GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } = &mut oriented_records[1].constraint
            else {
                unreachable!();
            };
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
        if mask & 4 != 0 {
            let GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                ..
            } = &mut oriented_records[2].constraint
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
fn distinct_middle_common_vertex_and_terminal_roles_are_required() {
    let fixture = Fixture::new();

    let mut repeated_leg = core_records(&fixture);
    repeated_leg[1].constraint = GeometricConstraintKindV1::Parallel {
        first_edge: fixture.edges[M],
        second_edge: fixture.edges[A],
    };
    assert_solver_required(&prepared(&fixture, repeated_leg).preflight());

    let mut different_angle_endpoint = core_records(&fixture);
    different_angle_endpoint[2].constraint = GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.center,
        first_edge: fixture.edges[A],
        second_edge: fixture.edges[N],
        angle_degrees: 90.0,
    };
    assert_solver_required(&prepared(&fixture, different_angle_endpoint).preflight());

    let mut middle_unit_instead_of_terminal = core_records(&fixture);
    middle_unit_instead_of_terminal[4].constraint = GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[M],
        length_mm: 1.0,
    };
    assert_solver_required(&prepared(&fixture, middle_unit_instead_of_terminal).preflight());

    let mut invalid_vertex = core_records(&fixture);
    invalid_vertex[2].constraint = GeometricConstraintKindV1::FixedAngle {
        vertex: fixture.endpoints[A],
        first_edge: fixture.edges[A],
        second_edge: fixture.edges[B],
        angle_degrees: 90.0,
    };
    assert!(
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            &document(invalid_vertex),
            GeometricConstraintLimitsV1::default(),
        )
        .is_err(),
        "the common angle vertex is validated before direct scanning",
    );
}

#[test]
fn unit_and_ninety_degree_requirements_are_bit_exact() {
    let fixture = Fixture::new();
    for role in [3, 4] {
        for value in [1.0_f64.next_down(), 1.0_f64.next_up(), 0.5, 2.0, f64::MAX] {
            let mut records = core_records(&fixture);
            let GeometricConstraintKindV1::FixedLength { length_mm, .. } =
                &mut records[role].constraint
            else {
                unreachable!();
            };
            *length_mm = value;
            assert_solver_required(&prepared(&fixture, records).preflight());
        }
    }

    for angle_degrees in [90.0_f64.next_down(), 90.0_f64.next_up(), 89.0, 91.0] {
        let mut records = core_records(&fixture);
        let GeometricConstraintKindV1::FixedAngle {
            angle_degrees: stored,
            ..
        } = &mut records[2].constraint
        else {
            unreachable!();
        };
        *stored = angle_degrees;
        assert_solver_required(&prepared(&fixture, records).preflight());
    }
}

#[test]
fn one_and_three_hop_cases_retain_their_existing_boundaries() {
    let fixture = Fixture::new();
    let one_hop_legacy = vec![
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.center,
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
            angle_degrees: 90.0,
        }),
    ];
    assert_solver_required(&prepared(&fixture, one_hop_legacy).preflight());

    let one_hop_exact = vec![
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.center,
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
            angle_degrees: 90.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[A],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[B],
            length_mm: 1.0,
        }),
    ];
    let one_hop_expected = sorted_ids(one_hop_exact.iter().cloned());
    let one_hop_outcome = prepared(&fixture, one_hop_exact).preflight();
    let ConstraintPreflightV1::DirectConflict { conflicts } = one_hop_outcome else {
        panic!("existing one-hop unit family must remain direct");
    };
    assert!(conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
        ) && candidate.constraint_ids() == one_hop_expected
    }));
    assert!(!conflicts.iter().any(|candidate| matches!(
        candidate.conflict(),
        DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent { .. }
    )));

    let three_hop = vec![
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[M],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[M],
            second_edge: fixture.edges[N],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[N],
            second_edge: fixture.edges[B],
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.center,
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
            angle_degrees: 90.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[A],
            length_mm: 1.0,
        }),
        record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[B],
            length_mm: 1.0,
        }),
    ];
    assert_solver_required(&prepared(&fixture, three_hop).preflight());
}

#[test]
fn duplicate_records_and_unrelated_extras_cannot_expand_the_five_id_cause() {
    let fixture = Fixture::new();
    let originals = core_records(&fixture);
    let mut duplicate_id_core = originals.clone();
    duplicate_id_core[4].id = duplicate_id_core[3].id;
    assert!(
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            &document(duplicate_id_core),
            GeometricConstraintLimitsV1::default(),
        )
        .is_err(),
        "duplicate cause IDs are rejected before candidate construction",
    );

    let mut records = originals.clone();
    let mut expected = Vec::new();
    for original in &originals {
        let duplicate = record(original.constraint.clone());
        expected.push(
            if original.id.canonical_bytes() < duplicate.id.canonical_bytes() {
                original.id
            } else {
                duplicate.id
            },
        );
        records.push(duplicate);
    }
    let unrelated = record(GeometricConstraintKindV1::FixedLength {
        edge: fixture.edges[EXTRA],
        length_mm: 1.0,
    });
    let unrelated_id = unrelated.id;
    records.push(unrelated);
    expected.sort_unstable_by_key(ConstraintId::canonical_bytes);

    let outcome = prepared(&fixture, records).preflight();
    let conflict = target_conflict(&outcome, &fixture).expect("duplicate groups retain exact core");
    assert_eq!(conflict.constraint_ids(), expected);
    assert_eq!(conflict.constraint_ids().len(), 5);
    assert!(!conflict.constraint_ids().contains(&unrelated_id));
}

#[test]
fn adjacent_angle_assignment_cannot_hide_the_exact_ninety_degree_subset() {
    let fixture = Fixture::new();
    for adjacent_angle in [90.0_f64.next_down(), 90.0_f64.next_up()] {
        let mut records = core_records(&fixture);
        let exact_angle_id = records[2].id;
        let adjacent = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.center,
            first_edge: fixture.edges[A],
            second_edge: fixture.edges[B],
            angle_degrees: adjacent_angle,
        });
        let adjacent_id = adjacent.id;
        records.push(adjacent);

        let mut expected = vec![
            records[0].id,
            records[1].id,
            exact_angle_id,
            records[3].id,
            records[4].id,
        ];
        expected.sort_unstable_by_key(ConstraintId::canonical_bytes);

        let outcome = prepared(&fixture, records).preflight();
        let conflict = target_conflict(&outcome, &fixture)
            .expect("an adjacent stored angle cannot hide the exact 90-degree subset");
        assert_eq!(conflict.constraint_ids(), expected);
        assert!(!conflict.constraint_ids().contains(&adjacent_id));
    }
}

#[test]
fn scanner_work_and_storage_exact_limits_are_admitted_and_one_short_fails_closed() {
    let _limits = UnitTerminalAngleParallelTestLimitsGuard::reset();
    let fixture = Fixture::new();
    let records = core_records(&fixture);
    let expected_ids = sorted_ids(records.iter().cloned());

    let baseline = prepared(&fixture, records.iter().cloned()).preflight();
    assert!(
        target_conflict(&baseline, &fixture).is_some(),
        "the unlimited baseline must reach the exact90 scanner proof",
    );
    let (exact_work, exact_storage) = unit_terminal_angle_parallel_test_observed_v1();
    assert!(exact_work > 0);
    assert!(exact_storage > 0);

    replace_unit_terminal_angle_parallel_test_limits_v1((Some(exact_work), None));
    let exact_work_outcome = prepared(&fixture, records.iter().cloned()).preflight();
    assert!(
        target_conflict(&exact_work_outcome, &fixture).is_some(),
        "equality with the observed work budget must remain admissible",
    );

    replace_unit_terminal_angle_parallel_test_limits_v1((Some(exact_work - 1), None));
    assert_eq!(
        prepared(&fixture, records.iter().cloned()).preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            unchecked_constraint_ids: expected_ids.clone(),
        },
    );

    replace_unit_terminal_angle_parallel_test_limits_v1((None, Some(exact_storage)));
    let exact_storage_outcome = prepared(&fixture, records.iter().cloned()).preflight();
    assert!(
        target_conflict(&exact_storage_outcome, &fixture).is_some(),
        "equality with the observed storage budget must remain admissible",
    );

    replace_unit_terminal_angle_parallel_test_limits_v1((None, Some(exact_storage - 1)));
    assert_eq!(
        prepared(&fixture, records).preflight(),
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::StorageLimitExceeded,
            unchecked_constraint_ids: expected_ids,
        },
    );
}

#[test]
fn scanner_budget_arithmetic_overflow_fails_closed_without_wrapping() {
    let mut work = u64::MAX;
    assert_eq!(
        charge_unit_terminal_angle_parallel_work_for_test_v1(&mut work, u64::MAX, 1),
        Err(GeometricConstraintUnknownReasonV1::WorkLimitExceeded),
    );
    assert_eq!(work, u64::MAX);

    let mut storage = usize::MAX;
    assert_eq!(
        reserve_unit_terminal_angle_parallel_storage_for_test_v1(&mut storage, usize::MAX, 1,),
        Err(GeometricConstraintUnknownReasonV1::StorageLimitExceeded),
    );
    assert_eq!(storage, usize::MAX);
}

#[test]
fn the_legacy_three_id_parallel_angle_candidate_remains_quarantined() {
    let fixture = Fixture::new();
    let records = core_records(&fixture)
        .into_iter()
        .take(3)
        .collect::<Vec<_>>();
    let expected = sorted_ids(records.iter().cloned());
    let outcome = prepared(&fixture, records).preflight();
    assert_eq!(
        outcome,
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: expected,
        }
    );
}

fn pinned_hypot(vector: (f64, f64)) -> f64 {
    deterministic_hypot_v1(vector.0, vector.1).expect("finite pinned hypot")
}

fn parallel_terms(first: (f64, f64), second: (f64, f64)) -> (f64, f64, f64) {
    let numerator = first.0 * second.1 - first.1 * second.0;
    let denominator = pinned_hypot(first) * pinned_hypot(second);
    (numerator, denominator, numerator / denominator)
}

fn angle_terms(first: (f64, f64), second: (f64, f64)) -> (f64, f64, f64) {
    let absolute_cross = (first.0 * second.1 - first.1 * second.0).abs();
    let dot = first.0 * second.0 + first.1 * second.1;
    let actual = deterministic_atan2_v1(absolute_cross, dot).expect("finite pinned atan2");
    (absolute_cross, dot, actual)
}

#[test]
fn normal_middle_zero_tie_and_subnormal_sixty_degree_boundary_bits_are_frozen() {
    let minimum_subnormal = f64::from_bits(1);

    let normal_a = (1.0, 0.0);
    let normal_m = (2.0, minimum_subnormal);
    let normal_b = (1.0, minimum_subnormal);
    assert_eq!(pinned_hypot(normal_a).to_bits(), 1.0_f64.to_bits());
    assert_eq!(pinned_hypot(normal_m).to_bits(), 2.0_f64.to_bits());
    assert_eq!(pinned_hypot(normal_b).to_bits(), 1.0_f64.to_bits());
    let normal_first = parallel_terms(normal_a, normal_m);
    let normal_second = parallel_terms(normal_m, normal_b);
    assert_eq!(normal_first.0.to_bits(), 1);
    assert_eq!(normal_first.1.to_bits(), 2.0_f64.to_bits());
    assert_eq!(normal_first.2.to_bits(), 0);
    assert_eq!(normal_second.0.to_bits(), 1);
    assert_eq!(normal_second.1.to_bits(), 2.0_f64.to_bits());
    assert_eq!(normal_second.2.to_bits(), 0);
    assert_eq!(angle_terms(normal_a, normal_b).2.to_bits(), 1);

    let x = f64::from_bits(0x3feb_b67a_e858_4cab);
    let subnormal_a = (x, 0.5);
    let subnormal_m = (minimum_subnormal, 0.0);
    let subnormal_b = (x, -0.5);
    assert_eq!(pinned_hypot(subnormal_a).to_bits(), 1.0_f64.to_bits());
    assert_eq!(
        pinned_hypot(subnormal_m).to_bits(),
        minimum_subnormal.to_bits()
    );
    assert_eq!(pinned_hypot(subnormal_b).to_bits(), 1.0_f64.to_bits());
    let subnormal_first = parallel_terms(subnormal_a, subnormal_m);
    let subnormal_second = parallel_terms(subnormal_m, subnormal_b);
    assert_eq!(subnormal_first.0.to_bits(), 0);
    assert_eq!(subnormal_first.2.to_bits(), 0);
    assert_eq!(subnormal_second.0.to_bits(), 0x8000_0000_0000_0000);
    assert_eq!(subnormal_second.2.to_bits(), 0x8000_0000_0000_0000);
    let (cross, dot, actual) = angle_terms(subnormal_a, subnormal_b);
    assert_eq!(cross.to_bits(), 0x3feb_b67a_e858_4cab);
    assert_eq!(dot.to_bits(), 0x3fe0_0000_0000_0001);
    assert_eq!(actual.to_bits(), 0x3ff0_c152_382d_7365);
}
