use super::*;
use crate::{
    ConstraintPreflightV1, DirectConstraintConflictKindV1,
    constraint_exactification::construct_unit_two_hop_parallel_residual_exact_deletion_assignment_v1,
};

pub(super) fn unit_two_hop_parallel_inventory_fixture() -> SemanticFixture {
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
        record(GeometricConstraintKindV1::Horizontal { edge: edges[0] }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: edges[0],
            second_edge: edges[1],
        }),
        record(GeometricConstraintKindV1::Parallel {
            first_edge: edges[1],
            second_edge: edges[2],
        }),
        record(GeometricConstraintKindV1::Vertical { edge: edges[2] }),
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

fn assert_only_unit_two_hop_phase(certificate: &CurrentRuntimeSemanticMusV1) {
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
        5
    );
}

#[test]
fn all_five_deletions_have_independent_production_exact_residual_witnesses() {
    let fixture = unit_two_hop_parallel_inventory_fixture();
    let expected = sorted_ids(fixture.records.iter().cloned());
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let ConstraintPreflightV1::DirectConflict { conflicts } = prepared_set.preflight() else {
        panic!("the exact two-hop unit-terminal theorem must be direct");
    };
    assert!(conflicts.iter().any(|candidate| {
        matches!(
            candidate.conflict(),
            DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
                parallel_constraint_count: 2,
                ..
            }
        ) && candidate.constraint_ids() == expected
    }));

    for removed in &fixture.records {
        let deletion = document(
            fixture
                .records
                .iter()
                .filter(|record| record.id != removed.id)
                .cloned(),
        );
        assert!(
            construct_unit_two_hop_parallel_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &fixture.records,
                removed.id,
                &deletion,
            )
            .is_some(),
            "each immediate deletion must be independently re-evaluated as exact SAT",
        );
    }

    let certificate = certificate_for(&fixture);
    assert_eq!(certificate.constraint_ids(), expected);
    assert_eq!(certificate.deletion_witness_checks(), 5);
    assert_only_unit_two_hop_phase(&certificate);
}

#[test]
fn fixed_length_deletion_template_uses_exact_underflow_and_overflow_zeros() {
    let minimum = f64::from_bits(1);
    let half_maximum = f64::from_bits(0x7fdf_ffff_ffff_ffff);
    let middle_length = ori_numeric::deterministic_hypot_v1(2.0, 0.5).expect("finite pinned hypot");
    assert_eq!((minimum * 0.5).to_bits(), 0.0_f64.to_bits());
    assert_eq!((minimum * middle_length).to_bits(), 2);
    assert_eq!((2.0 * half_maximum).to_bits(), f64::MAX.to_bits());
    assert_eq!(middle_length.to_bits(), 0x4000_7e0f_66af_ed07);
    assert!((middle_length * half_maximum).is_infinite());
    assert_eq!(
        ((minimum * 0.5) / (minimum * middle_length)).to_bits(),
        0.0_f64.to_bits(),
    );
    assert_eq!(
        ((2.0 * half_maximum) / (middle_length * half_maximum)).to_bits(),
        0.0_f64.to_bits(),
    );
}

#[test]
fn pattern_document_operand_and_edge_storage_order_are_invariant() {
    let fixture = unit_two_hop_parallel_inventory_fixture();
    let expected = certificate_for(&fixture);
    let mut reversed = SemanticFixture {
        pattern: fixture.pattern.clone(),
        records: fixture.records.clone(),
    };
    reversed.pattern.vertices.reverse();
    reversed.pattern.edges.reverse();
    for edge in &mut reversed.pattern.edges {
        (edge.start, edge.end) = (edge.end, edge.start);
    }
    reversed.records.reverse();
    for record in &mut reversed.records {
        if let GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } = &mut record.constraint
        {
            (*first_edge, *second_edge) = (*second_edge, *first_edge);
        }
    }
    assert_eq!(certificate_for(&reversed), expected);
}

#[test]
fn nonstar_topology_is_direct_but_the_dedicated_semantic_constructor_fails_closed() {
    let mut fixture = unit_two_hop_parallel_inventory_fixture();
    let vertices = fixture
        .pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    fixture.pattern.edges[0].start = vertices[0];
    fixture.pattern.edges[0].end = vertices[1];
    fixture.pattern.edges[1].start = vertices[1];
    fixture.pattern.edges[1].end = vertices[2];
    fixture.pattern.edges[2].start = vertices[2];
    fixture.pattern.edges[2].end = vertices[3];
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    assert!(matches!(
        prepared_set.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ));
    for removed in &fixture.records {
        let deletion = document(
            fixture
                .records
                .iter()
                .filter(|record| record.id != removed.id)
                .cloned(),
        );
        assert!(
            construct_unit_two_hop_parallel_residual_exact_deletion_assignment_v1(
                &fixture.pattern,
                &fixture.records,
                removed.id,
                &deletion,
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
    .expect("bounded two-hop setup work");
    let phase =
        crate::constraint_semantic_mus::unit_two_hop_parallel_residual_only_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            4,
        )
        .expect("bounded two-hop deletion work");
    (setup, phase)
}

#[test]
fn witness_count_and_work_limits_admit_exact_bounds_and_fail_closed_one_short() {
    let fixture = unit_two_hop_parallel_inventory_fixture();
    let prepared_set = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let complete = certificate_for(&fixture);
    let (setup, phase) = phase_work(&fixture);
    assert_eq!(
        complete.deletion_witness_work(),
        setup + fixture.records.len() * phase,
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
fn cancellation_and_deadline_at_entry_midpoint_and_prepublication_fail_closed() {
    let fixture = unit_two_hop_parallel_inventory_fixture();
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
