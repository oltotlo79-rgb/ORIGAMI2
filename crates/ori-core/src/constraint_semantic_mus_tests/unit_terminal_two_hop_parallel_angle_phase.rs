use super::*;
use crate::{
    ConstraintPreflightV1, DirectConstraintConflictKindV1,
    constraint_exactification::{
        MAX_UNIT_TERMINAL_TWO_HOP_PARALLEL_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1,
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1,
    },
    constraints::{
        deterministic_fixed_angle_residual_binary64_v1, fixed_angle_zero_actual_enclosure_v1,
    },
};
use ori_numeric::{deterministic_atan2_v1, deterministic_hypot_v1};

pub(super) fn unit_terminal_two_hop_parallel_angle_inventory_fixture() -> SemanticFixture {
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
    SemanticFixture { pattern, records }
}

fn certificate_for(fixture: &SemanticFixture) -> CurrentRuntimeSemanticMusV1 {
    certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared(
        &fixture.pattern,
        fixture.records.iter().cloned(),
    )))
}

fn has_target(preflight: &ConstraintPreflightV1) -> bool {
    matches!(
        preflight,
        ConstraintPreflightV1::DirectConflict { conflicts }
            if conflicts.iter().any(|candidate| matches!(
                candidate.conflict(),
                DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
                    parallel_constraint_count: 2,
                    ..
                }
            ))
    )
}

fn deletion_document(
    fixture: &SemanticFixture,
    removed: ConstraintId,
) -> GeometricConstraintDocumentV1 {
    document(
        fixture
            .records
            .iter()
            .filter(|record| record.id != removed)
            .cloned(),
    )
}

fn assert_only_unit_terminal_angle_phase(certificate: &CurrentRuntimeSemanticMusV1) {
    assert_eq!(certificate.current_assignment_witness_count(), 0);
    assert_eq!(certificate.axis_exactification_witness_count(), 0);
    assert_eq!(
        certificate.single_constraint_constructive_witness_count(),
        0
    );
    assert_eq!(certificate.pair_constraint_constructive_witness_count(), 0);
    assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 0);
    assert_eq!(
        certificate.length_constraint_constructive_witness_count(),
        0
    );
    assert_eq!(
        certificate.zero_length_closure_constructive_witness_count(),
        0
    );
    assert_eq!(certificate.anchored_mirror_residual_only_witness_count(), 0);
    assert_eq!(
        certificate.unit_two_hop_parallel_residual_only_witness_count(),
        0
    );
    assert_eq!(
        certificate.unit_terminal_two_hop_parallel_angle_residual_only_witness_count(),
        5
    );
}

#[test]
fn exact_five_id_core_recertifies_all_deletions_and_publishes_only_the_new_counter() {
    let fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    let expected = sorted_ids(fixture.records.iter().cloned());
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared_set.preflight() else {
        panic!("the exact terminal-unit angle theorem must be direct");
    };
    assert!(conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
                parallel_constraint_count: 2,
                ..
            }
        ) && candidate.constraint_ids() == expected
    }));

    for removed in &fixture.records {
        assert!(
            construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &fixture.records,
                removed.id,
                &deletion_document(&fixture, removed.id),
            )
            .is_some(),
            "every one-record deletion must receive a fresh complete residual certificate",
        );
    }

    let certificate = certificate_for(&fixture);
    assert_eq!(certificate.constraint_ids(), expected);
    assert_eq!(certificate.deletion_witness_checks(), 5);
    assert_eq!(
        certificate.model_id(),
        "geometric_constraint_deterministic_binary64_semantic_mus_v4"
    );
    assert_only_unit_terminal_angle_phase(&certificate);
}

#[test]
fn private_constructor_rejects_every_nonexact_deletion_document_shape() {
    let fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    let removed = fixture.records[0].id;
    let exact = deletion_document(&fixture, removed);

    let mut wrong_schema = exact.clone();
    wrong_schema.schema_version = GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1 + 1;
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &fixture.pattern,
            &fixture.records,
            removed,
            &wrong_schema,
        )
        .is_none(),
        "production recertification must reject an unsupported document schema",
    );

    let mut removed_still_present = exact.clone();
    removed_still_present.constraints[0] = fixture.records[0].clone();
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &fixture.pattern,
            &fixture.records,
            removed,
            &removed_still_present,
        )
        .is_none(),
        "a same-length document that restores the removed record is not a deletion",
    );

    let mut missing_record = exact.clone();
    missing_record.constraints.pop();
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &fixture.pattern,
            &fixture.records,
            removed,
            &missing_record,
        )
        .is_none(),
        "a strict subset of the requested deletion cannot be substituted",
    );

    let mut duplicate_replacement = exact;
    duplicate_replacement.constraints[3] = duplicate_replacement.constraints[0].clone();
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &fixture.pattern,
            &fixture.records,
            removed,
            &duplicate_replacement,
        )
        .is_none(),
        "duplicate retained records cannot replace a missing deletion member",
    );
}

fn next_permutation(values: &mut [usize]) -> bool {
    let Some(pivot) = (0..values.len().saturating_sub(1))
        .rev()
        .find(|index| values[*index] < values[*index + 1])
    else {
        return false;
    };
    let successor = (pivot + 1..values.len())
        .rev()
        .find(|index| values[*index] > values[pivot])
        .expect("a lexicographic successor exists");
    values.swap(pivot, successor);
    values[pivot + 1..].reverse();
    true
}

#[test]
fn all_record_permutations_operand_orders_and_edge_orientations_are_canonical() {
    let fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    let expected = certificate_for(&fixture);

    let mut order = [0, 1, 2, 3, 4];
    let mut permutation_count = 0;
    loop {
        let records = order
            .into_iter()
            .map(|index| fixture.records[index].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            certificate_for(&SemanticFixture {
                pattern: fixture.pattern.clone(),
                records,
            }),
            expected,
        );
        permutation_count += 1;
        if !next_permutation(&mut order) {
            break;
        }
    }
    assert_eq!(permutation_count, 120);

    for orientation_mask in 0_u8..8 {
        let mut variant = SemanticFixture {
            pattern: fixture.pattern.clone(),
            records: fixture.records.clone(),
        };
        for (index, edge) in variant.pattern.edges.iter_mut().enumerate() {
            if orientation_mask & (1 << index) != 0 {
                (edge.start, edge.end) = (edge.end, edge.start);
            }
        }
        variant.pattern.vertices.reverse();
        variant.pattern.edges.reverse();
        for record in &mut variant.records {
            match &mut record.constraint {
                GeometricConstraintKindV1::Parallel {
                    first_edge,
                    second_edge,
                }
                | GeometricConstraintKindV1::FixedAngle {
                    first_edge,
                    second_edge,
                    ..
                } => {
                    (*first_edge, *second_edge) = (*second_edge, *first_edge);
                }
                _ => {}
            }
        }
        assert_eq!(certificate_for(&variant), expected);
    }
}

fn cross(first: Point2, second: Point2) -> f64 {
    let positive = first.x * second.y;
    let negative = first.y * second.x;
    positive - negative
}

fn dot(first: Point2, second: Point2) -> f64 {
    let first_product = first.x * second.x;
    let second_product = first.y * second.y;
    first_product + second_product
}

fn pinned_length(vector: Point2) -> f64 {
    deterministic_hypot_v1(vector.x, vector.y).expect("finite pinned hypot")
}

fn parallel_residual(first: Point2, second: Point2) -> f64 {
    cross(first, second) / (pinned_length(first) * pinned_length(second))
}

fn angle_actual(first: Point2, second: Point2) -> f64 {
    deterministic_atan2_v1(cross(first, second).abs(), dot(first, second))
        .expect("finite pinned angle")
}

#[test]
fn normal_and_subnormal_middle_boundaries_stay_far_from_the_right_angle_zero_enclosure() {
    let minimum = f64::from_bits(1);
    let normal_first = Point2::new(1.0, 0.0);
    let normal_middle = Point2::new(f64::MAX, minimum);
    let normal_second = Point2::new(1.0, 0.0);
    assert_eq!(pinned_length(normal_middle).to_bits(), f64::MAX.to_bits());
    assert_eq!(
        parallel_residual(normal_first, normal_middle).to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        parallel_residual(normal_middle, normal_second).to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_ne!(
        deterministic_fixed_angle_residual_binary64_v1(
            angle_actual(normal_first, normal_second),
            90.0,
        ),
        0.0
    );

    let boundary_x = f64::from_bits(0x3feb_b67a_e858_4cab);
    let subnormal_first = Point2::new(boundary_x, 0.5);
    let subnormal_middle = Point2::new(minimum, 0.0);
    let subnormal_second = Point2::new(boundary_x, -0.5);
    assert_eq!(pinned_length(subnormal_first).to_bits(), 1.0_f64.to_bits());
    assert_eq!(pinned_length(subnormal_second).to_bits(), 1.0_f64.to_bits());
    assert_eq!(
        parallel_residual(subnormal_first, subnormal_middle).to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        parallel_residual(subnormal_middle, subnormal_second).to_bits(),
        (-0.0_f64).to_bits()
    );
    let actual = angle_actual(subnormal_first, subnormal_second);
    assert_eq!(actual.to_bits(), 0x3ff0_c152_382d_7365);
    assert_ne!(
        deterministic_fixed_angle_residual_binary64_v1(actual, 90.0),
        0.0
    );
    let enclosure =
        fixed_angle_zero_actual_enclosure_v1(90.0).expect("exact right-angle enclosure");
    assert_eq!(
        (enclosure.0.to_bits(), enclosure.1.to_bits()),
        (0x3ff9_21fb_5444_2d15, 0x3ff9_21fb_5444_2d1b)
    );
    assert!(actual < enclosure.0);
}

#[test]
fn fixed_length_deletions_use_the_exact_maximum_over_infinity_signed_zero_witnesses() {
    let huge = Point2::new(
        f64::from_bits(0x5fed_c4c2_f9c3_bdb0),
        f64::from_bits(0xdfd7_7af1_2e6b_a7b3),
    );
    let middle = Point2::new(
        f64::from_bits(0x5fd7_7af1_2e6b_a7b3),
        f64::from_bits(0x5fed_c4c2_f9c3_bdb0),
    );
    let unit = Point2::new(
        f64::from_bits(0x3fd7_7af1_2e6b_a7b4),
        f64::from_bits(0x3fed_c4c2_f9c3_bdb1),
    );
    assert_eq!(pinned_length(huge).to_bits(), 0x5ff0_0000_0000_0000);
    assert_eq!(pinned_length(middle).to_bits(), 0x5ff0_0000_0000_0000);
    assert_eq!(pinned_length(unit).to_bits(), 1.0_f64.to_bits());

    assert_eq!(cross(huge, middle).to_bits(), f64::MAX.to_bits());
    assert!((pinned_length(huge) * pinned_length(middle)).is_infinite());
    assert_eq!(parallel_residual(huge, middle).to_bits(), 0.0_f64.to_bits());
    assert_eq!(parallel_residual(middle, unit).to_bits(), 0.0_f64.to_bits());
    assert_eq!(cross(huge, unit).abs().to_bits(), 0x5ff0_0000_0000_0000);
    assert_eq!(dot(huge, unit).to_bits(), 0.0_f64.to_bits());
    let actual = angle_actual(huge, unit);
    assert_eq!(actual.to_bits(), 0x3ff9_21fb_5444_2d18);
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(actual, 90.0).to_bits(),
        0.0_f64.to_bits()
    );

    assert_eq!(parallel_residual(unit, middle).to_bits(), 0.0_f64.to_bits());
    assert_eq!(cross(middle, huge).to_bits(), (-f64::MAX).to_bits());
    assert_eq!(
        parallel_residual(middle, huge).to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(dot(unit, huge).to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(angle_actual(unit, huge), 90.0,).to_bits(),
        0.0_f64.to_bits()
    );
}

fn assert_public_target_absent(fixture: &SemanticFixture) {
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    assert!(!has_target(&prepared_set.preflight()));
    assert!(!matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared_set),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
}

#[test]
fn nonunit_and_nonright_scalar_neighbors_never_enter_the_exact_family() {
    for length in [1.0_f64.next_down(), 1.0_f64.next_up(), 0.5, 2.0, f64::MAX] {
        for record_index in [3, 4] {
            let mut fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
            let GeometricConstraintKindV1::FixedLength { length_mm, .. } =
                &mut fixture.records[record_index].constraint
            else {
                unreachable!()
            };
            *length_mm = length;
            assert_public_target_absent(&fixture);
        }
    }
    for angle in [90.0_f64.next_down(), 90.0_f64.next_up()] {
        let mut fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
        let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } =
            &mut fixture.records[2].constraint
        else {
            unreachable!()
        };
        *angle_degrees = angle;
        assert_public_target_absent(&fixture);
    }
}

#[test]
fn missing_extra_duplicate_one_hop_and_three_hop_shapes_are_fail_closed_or_canonically_reduced() {
    let fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    for removed in &fixture.records {
        let missing = SemanticFixture {
            pattern: fixture.pattern.clone(),
            records: fixture
                .records
                .iter()
                .filter(|record| record.id != removed.id)
                .cloned()
                .collect(),
        };
        assert_public_target_absent(&missing);
    }

    let mut extra = SemanticFixture {
        pattern: fixture.pattern.clone(),
        records: fixture.records.clone(),
    };
    extra
        .records
        .push(record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.pattern.edges[1].id,
        }));
    let extra_certificate = certificate_for(&extra);
    assert_eq!(
        extra_certificate.constraint_ids(),
        sorted_ids(fixture.records.iter().cloned())
    );
    assert_only_unit_terminal_angle_phase(&extra_certificate);
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &extra.pattern,
            &extra.records,
            fixture.records[0].id,
            &deletion_document(&fixture, fixture.records[0].id),
        )
        .is_none(),
        "the private constructor accepts an exact five-record core, not an oversized document",
    );

    let mut duplicate_id_core = fixture.records.clone();
    duplicate_id_core[1].id = duplicate_id_core[0].id;
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &fixture.pattern,
            &duplicate_id_core,
            duplicate_id_core[0].id,
            &deletion_document(&fixture, fixture.records[0].id),
        )
        .is_none(),
    );

    let first_edge = fixture.pattern.edges[0].id;
    let middle_edge = fixture.pattern.edges[1].id;
    let second_edge = fixture.pattern.edges[2].id;
    let mut one_hop = SemanticFixture {
        pattern: fixture.pattern.clone(),
        records: fixture.records.clone(),
    };
    one_hop.records[1].constraint = GeometricConstraintKindV1::Parallel {
        first_edge,
        second_edge,
    };
    assert!(!has_target(
        &prepared(&one_hop.pattern, one_hop.records.iter().cloned()).preflight()
    ));
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &one_hop.pattern,
            &one_hop.records,
            one_hop.records[0].id,
            &deletion_document(&one_hop, one_hop.records[0].id),
        )
        .is_none(),
    );

    let mut three_hop = SemanticFixture {
        pattern: fixture.pattern.clone(),
        records: fixture.records.clone(),
    };
    let fourth_vertex = VertexId::new();
    let fourth_edge = EdgeId::new();
    let center = fixture.pattern.edges[0].start;
    three_hop.pattern.vertices.push(Vertex {
        id: fourth_vertex,
        position: Point2::new(4.0, 4.0),
    });
    three_hop.pattern.edges.push(Edge {
        id: fourth_edge,
        start: center,
        end: fourth_vertex,
        kind: EdgeKind::Auxiliary,
    });
    three_hop.records[1].constraint = GeometricConstraintKindV1::Parallel {
        first_edge: middle_edge,
        second_edge: fourth_edge,
    };
    three_hop
        .records
        .push(record(GeometricConstraintKindV1::Parallel {
            first_edge: fourth_edge,
            second_edge,
        }));
    assert_public_target_absent(&three_hop);
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &three_hop.pattern,
            &three_hop.records,
            three_hop.records[0].id,
            &deletion_document(&three_hop, three_hop.records[0].id),
        )
        .is_none(),
    );
}

#[test]
fn nonstar_topology_keeps_the_direct_theorem_but_never_promotes_semantically() {
    let mut fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    let first_outer = fixture.pattern.edges[0].end;
    let middle_outer = fixture.pattern.edges[1].end;
    fixture.pattern.edges[1].start = first_outer;
    fixture.pattern.edges[1].end = middle_outer;
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    assert!(has_target(&prepared_set.preflight()));
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared_set),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            ..
        }
    ));
    for removed in &fixture.records {
        assert!(
            construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &fixture.records,
                removed.id,
                &deletion_document(&fixture, removed.id),
            )
            .is_none(),
        );
    }
}

fn phase_work(fixture: &SemanticFixture) -> (usize, usize) {
    let (setup, ..) = crate::constraint_semantic_mus::witness_phase_work_for_test(
        fixture.pattern.vertices.len(),
        fixture.pattern.edges.len(),
        5,
        4,
    )
    .expect("bounded setup work");
    let phase = crate::constraint_semantic_mus::
        unit_terminal_two_hop_parallel_angle_residual_only_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            4,
        )
        .expect("bounded terminal-angle deletion work");
    (setup, phase)
}

#[test]
fn witness_work_and_overlay_storage_boundaries_are_exact_and_one_short_fails_closed() {
    let fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let complete = certificate_for(&fixture);
    let (setup, phase) = phase_work(&fixture);
    assert_eq!(
        complete.deletion_witness_work(),
        setup + fixture.records.len() * phase
    );

    let mut exact = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 5,
                max_deletion_witness_work: complete.deletion_witness_work(),
            },
            &mut exact,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    let mut one_short_count = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 4,
                max_deletion_witness_work: complete.deletion_witness_work(),
            },
            &mut one_short_count,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            ..
        }
    ));
    let mut one_short_work = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 5,
                max_deletion_witness_work: complete.deletion_witness_work() - 1,
            },
            &mut one_short_work,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks: 5,
            certified_deletion_witnesses: 4,
            ..
        }
    ));

    let maximum_vertices =
        MAX_UNIT_TERMINAL_TWO_HOP_PARALLEL_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1;
    assert!(
        crate::constraint_semantic_mus::
            unit_terminal_two_hop_parallel_angle_residual_only_phase_work_for_test(
                maximum_vertices,
                fixture.pattern.edges.len(),
                4,
            )
            .is_some()
    );
    assert!(
        crate::constraint_semantic_mus::
            unit_terminal_two_hop_parallel_angle_residual_only_phase_work_for_test(
                maximum_vertices + 1,
                fixture.pattern.edges.len(),
                4,
            )
            .is_none()
    );

    let mut exact_storage = SemanticFixture {
        pattern: fixture.pattern.clone(),
        records: fixture.records.clone(),
    };
    while exact_storage.pattern.vertices.len() < maximum_vertices {
        let ordinal = exact_storage.pattern.vertices.len() as f64;
        exact_storage.pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(ordinal + 10.0, ordinal + 20.0),
        });
    }
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &exact_storage.pattern,
            &exact_storage.records,
            exact_storage.records[0].id,
            &deletion_document(&exact_storage, exact_storage.records[0].id),
        )
        .is_some()
    );
    exact_storage.pattern.vertices.push(Vertex {
        id: VertexId::new(),
        position: Point2::new(1_000.0, 2_000.0),
    });
    assert!(
        construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
            &exact_storage.pattern,
            &exact_storage.records,
            exact_storage.records[0].id,
            &deletion_document(&exact_storage, exact_storage.records[0].id),
        )
        .is_none()
    );
}

struct StopAtCheckpoint {
    calls: usize,
    stop_at: usize,
    control: BoundedSemanticMusObserverControlV1,
}

impl BoundedSemanticMusObserverV1 for StopAtCheckpoint {
    fn checkpoint(
        &mut self,
        _progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.calls += 1;
        if self.calls == self.stop_at {
            self.control
        } else {
            BoundedSemanticMusObserverControlV1::Continue
        }
    }
}

#[test]
fn cancellation_and_deadline_at_entry_midpoint_and_prepublication_withhold_the_certificate() {
    let fixture = unit_terminal_two_hop_parallel_angle_inventory_fixture();
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let mut baseline = StopAtCheckpoint {
        calls: 0,
        stop_at: usize::MAX,
        control: BoundedSemanticMusObserverControlV1::Cancelled,
    };
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1::default(),
            &mut baseline,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    assert!(baseline.calls > 5);

    for stop_at in [1, baseline.calls / 2, baseline.calls] {
        for (control, reason) in [
            (
                BoundedSemanticMusObserverControlV1::Cancelled,
                BoundedSemanticMusUnknownReasonV1::Cancelled,
            ),
            (
                BoundedSemanticMusObserverControlV1::DeadlineReached,
                BoundedSemanticMusUnknownReasonV1::DeadlineReached,
            ),
        ] {
            let mut observer = StopAtCheckpoint {
                calls: 0,
                stop_at,
                control,
            };
            assert!(matches!(
                certify_bounded_current_runtime_semantic_mus_with_observer_v1(
                    &prepared_set,
                    BoundedSemanticMusLimitsV1::default(),
                    &mut observer,
                ),
                BoundedCurrentRuntimeSemanticMusV1::Unknown {
                    reason: actual,
                    ..
                } if actual == reason
            ));
            assert_eq!(observer.calls, stop_at);
        }
    }
}
