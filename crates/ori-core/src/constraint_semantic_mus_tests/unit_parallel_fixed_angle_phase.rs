use super::*;
use crate::{
    ConstraintPreflightV1, DirectConstraintConflictKindV1,
    constraint_exactification::{
        MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1,
        construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1,
    },
    constraints::deterministic_fixed_angle_residual_binary64_v1,
};
use ori_numeric::{deterministic_atan2_v1, deterministic_hypot_v1};

pub(super) fn unit_parallel_fixed_angle_inventory_fixture() -> SemanticFixture {
    let center = VertexId::new();
    let first_outer = VertexId::new();
    let second_outer = VertexId::new();
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: first_outer,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: second_outer,
                    position: Point2::new(1.0, 1.0),
                },
            ],
            edges: vec![
                Edge {
                    id: first_edge,
                    start: center,
                    end: first_outer,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second_edge,
                    start: center,
                    end: second_outer,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        },
        records: vec![
            record(GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            }),
            record(GeometricConstraintKindV1::FixedAngle {
                vertex: center,
                first_edge,
                second_edge,
                angle_degrees: 45.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: first_edge,
                length_mm: 1.0,
            }),
        ],
    }
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
                DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
            ) && candidate.constraint_ids().len() == 3)
    )
}

fn assert_only_unit_parallel_angle_phase(certificate: &CurrentRuntimeSemanticMusV1) {
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
        0
    );
    assert_eq!(
        certificate.unit_parallel_fixed_angle_residual_only_witness_count(),
        3
    );
}

#[test]
fn exact_three_id_core_recertifies_every_deletion_in_the_dedicated_phase() {
    let fixture = unit_parallel_fixed_angle_inventory_fixture();
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    assert!(has_target(&prepared_set.preflight()));

    for removed in &fixture.records {
        assert!(
            construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &fixture.records,
                removed.id,
                &deletion_document(&fixture, removed.id),
            )
            .is_some(),
            "each immediate deletion must be independently recertified",
        );
    }

    let certificate = certificate_for(&fixture);
    assert_eq!(
        certificate.constraint_ids(),
        sorted_ids(fixture.records.iter().cloned())
    );
    assert_eq!(certificate.deletion_witness_checks(), 3);
    assert_only_unit_parallel_angle_phase(&certificate);
}

#[test]
fn witness_bits_pin_unit_diagonal_and_finite_max_over_infinity_boundaries() {
    let diagonal = f64::from_bits(0x3fe6_a09e_667f_3bcd);
    assert_eq!(
        deterministic_hypot_v1(diagonal, diagonal)
            .expect("unit diagonal hypot")
            .to_bits(),
        1.0_f64.to_bits(),
    );
    let diagonal_angle = deterministic_atan2_v1(diagonal, diagonal).expect("unit diagonal atan2");
    assert_eq!(diagonal_angle.to_bits(), 0x3fe9_21fb_5444_2d18);
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(diagonal_angle, 45.0).to_bits(),
        0.0_f64.to_bits(),
    );

    let scale = f64::from_bits(0x5fec_0000_0000_0000);
    let square = scale * scale;
    let diagonal_hypot = deterministic_hypot_v1(scale, scale).expect("finite large diagonal hypot");
    assert_eq!(square.to_bits(), 0x7fe8_8000_0000_0000);
    assert_eq!(diagonal_hypot.to_bits(), 0x5ff3_cc8a_99af_5453);
    assert!((scale * diagonal_hypot).is_infinite());
    assert_eq!((square / (scale * diagonal_hypot)).to_bits(), 0);
    let overflow_angle =
        deterministic_atan2_v1(square, square).expect("finite equal atan2 operands");
    assert_eq!(overflow_angle.to_bits(), 0x3fe9_21fb_5444_2d18);
    assert_eq!(
        deterministic_fixed_angle_residual_binary64_v1(overflow_angle, 45.0).to_bits(),
        0,
    );
}

#[test]
fn record_order_operands_edge_directions_and_pinned_edge_role_are_invariant() {
    for fixed_role in [0, 1] {
        for direction_mask in 0_u8..4 {
            for reverse_operands in [false, true] {
                let mut fixture = unit_parallel_fixed_angle_inventory_fixture();
                let edges = [fixture.pattern.edges[0].id, fixture.pattern.edges[1].id];
                for (index, edge) in fixture.pattern.edges.iter_mut().enumerate() {
                    if direction_mask & (1 << index) != 0 {
                        std::mem::swap(&mut edge.start, &mut edge.end);
                    }
                }
                let (first, second) = if reverse_operands {
                    (edges[1], edges[0])
                } else {
                    (edges[0], edges[1])
                };
                fixture.records[0].constraint = GeometricConstraintKindV1::Parallel {
                    first_edge: first,
                    second_edge: second,
                };
                let GeometricConstraintKindV1::FixedAngle {
                    vertex,
                    first_edge,
                    second_edge,
                    angle_degrees,
                } = &mut fixture.records[1].constraint
                else {
                    unreachable!();
                };
                *first_edge = first;
                *second_edge = second;
                assert_eq!(*angle_degrees, 45.0);
                assert!(
                    fixture.pattern.edges.iter().all(|edge| {
                        edge.start == *vertex || edge.end == *vertex || !edges.contains(&edge.id)
                    }),
                    "both theorem edges must remain incident to the angle center",
                );
                fixture.records[2].constraint = GeometricConstraintKindV1::FixedLength {
                    edge: edges[fixed_role],
                    length_mm: 1.0,
                };
                if direction_mask & 1 != 0 {
                    fixture.records.rotate_left(1);
                }
                if direction_mask & 2 != 0 {
                    fixture.records.reverse();
                }
                assert_only_unit_parallel_angle_phase(&certificate_for(&fixture));
            }
        }
    }
}

#[test]
fn constructor_rejects_nonexact_values_documents_and_nonstar_topology() {
    let fixture = unit_parallel_fixed_angle_inventory_fixture();
    let removed = fixture.records[0].id;
    let deletion = deletion_document(&fixture, removed);

    for angle in [45.0_f64.next_down(), 45.0_f64.next_up(), 90.0] {
        let mut changed = fixture.records.clone();
        let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } =
            &mut changed[1].constraint
        else {
            unreachable!();
        };
        *angle_degrees = angle;
        assert!(
            construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &changed,
                removed,
                &deletion,
            )
            .is_none()
        );
    }
    for length in [1.0_f64.next_down(), 1.0_f64.next_up(), 0.5, 2.0] {
        let mut changed = fixture.records.clone();
        let GeometricConstraintKindV1::FixedLength { length_mm, .. } = &mut changed[2].constraint
        else {
            unreachable!();
        };
        *length_mm = length;
        assert!(
            construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &changed,
                removed,
                &deletion,
            )
            .is_none()
        );
    }

    let mut extra = deletion.clone();
    extra
        .constraints
        .push(record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.pattern.edges[0].id,
        }));
    assert!(
        construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
            &fixture.pattern,
            &fixture.records,
            removed,
            &extra,
        )
        .is_none()
    );

    let mut nonstar = fixture.pattern.clone();
    nonstar.edges[1].end = nonstar.edges[0].end;
    assert!(
        construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
            &nonstar,
            &fixture.records,
            removed,
            &deletion,
        )
        .is_none()
    );
}

#[test]
fn overlay_vertex_ceiling_and_work_formula_fail_closed_one_over() {
    let fixture = unit_parallel_fixed_angle_inventory_fixture();
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let certificate = certificate_for(&fixture);
    let (setup, ..) = crate::constraint_semantic_mus::witness_phase_work_for_test(
        fixture.pattern.vertices.len(),
        fixture.pattern.edges.len(),
        3,
        2,
    )
    .expect("bounded setup work");
    let phase =
        crate::constraint_semantic_mus::unit_parallel_fixed_angle_residual_only_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            2,
        )
        .expect("bounded dedicated phase work");
    assert_eq!(certificate.deletion_witness_work(), setup + 3 * phase);

    let mut exact_limit = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 3,
                max_deletion_witness_work: certificate.deletion_witness_work(),
            },
            &mut exact_limit,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    let mut one_short = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 3,
                max_deletion_witness_work: certificate.deletion_witness_work() - 1,
            },
            &mut one_short,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks: 3,
            certified_deletion_witnesses: 2,
            deletion_witness_work,
            ..
        } if deletion_witness_work == setup + 2 * phase
    ));

    let removed = fixture.records[0].id;
    let deletion = deletion_document(&fixture, removed);
    let mut exact = fixture.pattern.clone();
    while exact.vertices.len() < MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        exact.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(64.0, 64.0),
        });
    }
    assert!(
        construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
            &exact,
            &fixture.records,
            removed,
            &deletion,
        )
        .is_some()
    );
    exact.vertices.push(Vertex {
        id: VertexId::new(),
        position: Point2::new(64.0, 64.0),
    });
    assert!(
        construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
            &exact,
            &fixture.records,
            removed,
            &deletion,
        )
        .is_none()
    );
    assert!(
        crate::constraint_semantic_mus::
            unit_parallel_fixed_angle_residual_only_phase_work_for_test(
                MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1,
                fixture.pattern.edges.len(),
                2,
            )
            .is_some()
    );
    assert!(
        crate::constraint_semantic_mus::
            unit_parallel_fixed_angle_residual_only_phase_work_for_test(
                MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 + 1,
                fixture.pattern.edges.len(),
                2,
            )
            .is_none()
    );
}

#[derive(Default)]
struct RecordingObserver {
    progress: Vec<BoundedSemanticMusProgressV1>,
}

impl BoundedSemanticMusObserverV1 for RecordingObserver {
    fn checkpoint(
        &mut self,
        progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.progress.push(progress);
        BoundedSemanticMusObserverControlV1::Continue
    }
}

#[test]
fn direct_entry_dedicated_phase_and_prepublish_honor_cancel_and_deadline() {
    let fixture = unit_parallel_fixed_angle_inventory_fixture();
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, ..) = crate::constraint_semantic_mus::witness_phase_work_for_test(
        fixture.pattern.vertices.len(),
        fixture.pattern.edges.len(),
        3,
        2,
    )
    .expect("bounded setup work");
    let phase =
        crate::constraint_semantic_mus::unit_parallel_fixed_angle_residual_only_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            2,
        )
        .expect("bounded dedicated phase work");
    let exact = setup + 3 * phase;

    let mut recording = RecordingObserver::default();
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared_set,
            BoundedSemanticMusLimitsV1::default(),
            &mut recording,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));

    let semantic_entry = recording
        .progress
        .iter()
        .position(|progress| {
            progress.deletion_witness_checks == 1
                && progress.certified_deletion_witnesses == 0
                && progress.deletion_witness_work == setup
        })
        .expect("first dedicated deletion entry")
        + 1;
    let phase_boundaries = recording
        .progress
        .iter()
        .enumerate()
        .filter_map(|(index, progress)| {
            (progress.deletion_witness_checks == 1
                && progress.certified_deletion_witnesses == 0
                && progress.deletion_witness_work == setup + phase)
                .then_some(index + 1)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        phase_boundaries.len(),
        2,
        "dedicated phase needs checkpoints before and after construction"
    );
    let prepublish = recording
        .progress
        .iter()
        .position(|progress| {
            progress.deletion_witness_checks == 3
                && progress.certified_deletion_witnesses == 3
                && progress.deletion_witness_work == exact
        })
        .expect("prepublication checkpoint")
        + 1;

    let boundaries = [
        (1, 0, 0, 0, 0),
        (semantic_entry, 3, 1, 0, setup),
        (phase_boundaries[0], 3, 1, 0, setup + phase),
        (phase_boundaries[1], 3, 1, 0, setup + phase),
        (prepublish, 3, 3, 3, exact),
    ];
    for (stop_at, expected_core_len, expected_checks, expected_certified, expected_work) in
        boundaries
    {
        for (control, expected_reason) in [
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
                    reason,
                    direct_core_constraint_ids,
                    deletion_witness_checks,
                    certified_deletion_witnesses,
                    deletion_witness_work,
                    ..
                } if reason == expected_reason
                    && direct_core_constraint_ids.len() == expected_core_len
                    && deletion_witness_checks == expected_checks
                    && certified_deletion_witnesses == expected_certified
                    && deletion_witness_work == expected_work
            ));
        }
    }
}
