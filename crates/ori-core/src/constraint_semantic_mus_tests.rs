use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use crate::{
    BoundedCurrentRuntimeSemanticMusV1, BoundedDirectMusV1, BoundedSemanticMusLimitsV1,
    BoundedSemanticMusObserverControlV1, BoundedSemanticMusObserverV1,
    BoundedSemanticMusProgressV1, BoundedSemanticMusUnknownReasonV1, ConstraintSolvePreviewV1,
    CurrentRuntimeSemanticMusV1, GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1,
    GeometricConstraintLimitsV1, MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
    MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
    MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1, NoopBoundedSemanticMusObserverV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1,
    certify_bounded_current_runtime_semantic_mus_v1,
    certify_bounded_current_runtime_semantic_mus_with_observer_v1,
    exactify_axis_aligned_constraint_preview_v1, find_bounded_direct_mus_v1,
    prepare_geometric_constraints_v1,
};

struct SemanticFixture {
    pattern: CreasePattern,
    records: Vec<GeometricConstraintRecordV1>,
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

fn empty_preview() -> ConstraintSolvePreviewV1 {
    ConstraintSolvePreviewV1 {
        positions: Vec::new(),
        iterations: 0,
        maximum_residual: 0.0,
        rank: 0,
        degrees_of_freedom: 0,
        equation_count: 0,
        condition_estimate: 1.0,
    }
}

fn semantic_fixture() -> SemanticFixture {
    let mut vertices = [VertexId::new(), VertexId::new(), VertexId::new()];
    vertices.sort_unstable_by_key(VertexId::canonical_bytes);
    let diagonal_endpoint = vertices[0];
    let origin = vertices[1];
    let horizontal_endpoint = vertices[2];
    let horizontal_edge = EdgeId::new();
    let diagonal_edge = EdgeId::new();

    // The diagonal endpoint is the canonical representative of every Y class
    // that contains it. This makes all three immediate-deletion assignments
    // deterministic:
    // - H(horizontal)+45° is already exact;
    // - projecting H(diagonal) moves the origin to Y=1, leaving the other
    //   edge at -45° from the resulting horizontal edge;
    // - projecting both H records makes both edges horizontal.
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: horizontal_endpoint,
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: origin,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: diagonal_endpoint,
                position: Point2::new(1.0, 1.0),
            },
        ],
        edges: vec![
            Edge {
                id: diagonal_edge,
                start: origin,
                end: diagonal_endpoint,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: horizontal_edge,
                start: origin,
                end: horizontal_endpoint,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: horizontal_edge,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: diagonal_edge,
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: origin,
            first_edge: horizontal_edge,
            second_edge: diagonal_edge,
            angle_degrees: 45.0,
        }),
    ];
    SemanticFixture { pattern, records }
}

fn prepared<'a>(
    pattern: &'a CreasePattern,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> crate::GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("semantic-MUS fixture must prepare")
}

fn sorted_ids(records: impl IntoIterator<Item = GeometricConstraintRecordV1>) -> Vec<ConstraintId> {
    let mut ids = records
        .into_iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

fn certified(result: BoundedCurrentRuntimeSemanticMusV1) -> CurrentRuntimeSemanticMusV1 {
    match result {
        BoundedCurrentRuntimeSemanticMusV1::Certified(certificate) => certificate,
        other => panic!("expected a semantic-MUS certificate, got {other:?}"),
    }
}

#[test]
fn direct_core_is_promoted_only_after_every_deletion_has_an_independent_exact_assignment() {
    let fixture = semantic_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let expected_ids = sorted_ids(fixture.records.iter().cloned());
    assert!(
        expected_ids
            .windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    );
    assert_eq!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids: expected_ids.clone(),
            oracle_calls: 7,
        },
        "the complete three-record core needs the sound direct theorem",
    );

    let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    assert_eq!(
        certificate.model_id(),
        GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1,
    );
    assert_eq!(certificate.constraint_ids(), expected_ids);
    assert_eq!(certificate.direct_oracle_calls(), 7);
    assert_eq!(certificate.deletion_witness_checks(), 3);
    assert_eq!(certificate.current_assignment_witness_count(), 1);
    assert_eq!(certificate.axis_exactification_witness_count(), 2);
    assert_eq!(
        certificate.single_constraint_constructive_witness_count(),
        0,
    );
    assert!(certificate.deletion_witness_work() > 0);
    assert!(
        certificate.deletion_witness_work() <= MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1,
    );
    assert!(!certificate.authorizes_project_mutation());
    assert!(!certificate.replayable_across_runtimes());

    let mut current_witnesses = 0;
    let mut axis_witnesses = 0;
    for removed in &fixture.records {
        let deletion = document(
            fixture
                .records
                .iter()
                .filter(|record| record.id != removed.id)
                .cloned(),
        );
        if certify_binary64_exact_geometric_constraint_satisfaction_v1(&fixture.pattern, &deletion)
            .expect("the deletion subset remains structurally valid")
            .is_some()
        {
            current_witnesses += 1;
            continue;
        }
        let exact = exactify_axis_aligned_constraint_preview_v1(
            &fixture.pattern,
            &deletion,
            &empty_preview(),
        )
        .expect("the deletion subset must have a separately projected exact assignment");
        assert!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                exact.pattern(),
                &deletion,
            )
            .expect("the explicit projected assignment remains valid")
            .is_some(),
        );
        axis_witnesses += 1;
    }
    assert_eq!((current_witnesses, axis_witnesses), (1, 2));
}

#[test]
fn certificate_is_invariant_to_pattern_and_document_storage_order() {
    let fixture = semantic_fixture();
    let forward = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let expected = certify_bounded_current_runtime_semantic_mus_v1(&forward);

    let mut reversed_pattern = fixture.pattern.clone();
    reversed_pattern.vertices.reverse();
    reversed_pattern.edges.reverse();
    let reversed = prepared(&reversed_pattern, fixture.records.iter().rev().cloned());
    assert_eq!(
        certify_bounded_current_runtime_semantic_mus_v1(&reversed),
        expected,
    );
}

#[test]
fn witness_count_and_work_limits_admit_exact_bounds_and_fail_closed_one_short() {
    let fixture = semantic_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let baseline = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    let exact_work = baseline.deletion_witness_work();

    let mut exact_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 3,
                max_deletion_witness_work: exact_work,
            },
            &mut exact_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));

    let mut one_short_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 3,
                max_deletion_witness_work: exact_work - 1,
            },
            &mut one_short_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_work,
            ..
        } if deletion_witness_work < exact_work
    ));

    let mut one_short_count_observer = NoopBoundedSemanticMusObserverV1;
    assert_eq!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 2,
                max_deletion_witness_work: exact_work,
            },
            &mut one_short_count_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            direct_core_constraint_ids: baseline.constraint_ids().to_vec(),
            direct_oracle_calls: 7,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        }
    );

    let mut zero_check_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 0,
                max_deletion_witness_work:
                    MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1,
            },
            &mut zero_check_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            direct_core_constraint_ids,
            deletion_witness_checks: 0,
            deletion_witness_work: 0,
            ..
        } if direct_core_constraint_ids == prepared
            .constraints()
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>()
    ));
}

#[test]
fn zero_invalid_limits_and_overflowed_work_math_never_start_a_witness_phase() {
    let fixture = semantic_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let mut observer = NoopBoundedSemanticMusObserverV1;
    assert_eq!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
                max_deletion_witness_work: 0,
            },
            &mut observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            direct_core_constraint_ids: prepared
                .constraints()
                .iter()
                .map(|record| record.id)
                .collect(),
            direct_oracle_calls: 7,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
    );
    assert!(
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        )
        .is_none(),
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
fn cancellation_and_deadline_checkpoints_withhold_every_incomplete_positive_claim() {
    let fixture = semantic_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());

    let mut counter = StopAtCheckpoint {
        calls: 0,
        stop_at: usize::MAX,
        control: BoundedSemanticMusObserverControlV1::Cancelled,
    };
    let complete = certified(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1::default(),
            &mut counter,
        ),
    );
    assert_eq!(counter.calls, 18);

    for (stop_at, control, expected_reason) in [
        (
            1,
            BoundedSemanticMusObserverControlV1::Cancelled,
            BoundedSemanticMusUnknownReasonV1::Cancelled,
        ),
        (
            8,
            BoundedSemanticMusObserverControlV1::DeadlineReached,
            BoundedSemanticMusUnknownReasonV1::DeadlineReached,
        ),
        (
            counter.calls,
            BoundedSemanticMusObserverControlV1::Cancelled,
            BoundedSemanticMusUnknownReasonV1::Cancelled,
        ),
        (
            counter.calls,
            BoundedSemanticMusObserverControlV1::DeadlineReached,
            BoundedSemanticMusUnknownReasonV1::DeadlineReached,
        ),
    ] {
        let mut observer = StopAtCheckpoint {
            calls: 0,
            stop_at,
            control,
        };
        let outcome = certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1::default(),
            &mut observer,
        );
        assert!(matches!(
            outcome,
            BoundedCurrentRuntimeSemanticMusV1::Unknown { reason, .. }
                if reason == expected_reason
        ));
        if stop_at == counter.calls {
            assert!(matches!(
                outcome,
                BoundedCurrentRuntimeSemanticMusV1::Unknown {
                    deletion_witness_checks: 3,
                    certified_deletion_witnesses: 3,
                    deletion_witness_work,
                    ..
                } if deletion_witness_work == complete.deletion_witness_work()
            ));
        }
    }
}

#[test]
fn direct_oracle_hard_bound_remains_separate_from_witness_work() {
    let fixture = semantic_fixture();
    let edge = fixture.pattern.edges[0].id;
    let records = (0..16)
        .map(|_| record(GeometricConstraintKindV1::Horizontal { edge }))
        .collect::<Vec<_>>();
    let prepared_set = prepared(&fixture.pattern, records);
    assert_eq!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared_set),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            direct_core_constraint_ids: Vec::new(),
            direct_oracle_calls: MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
    );

    let mut oversized_records = prepared_set.constraints().to_vec();
    oversized_records.push(record(GeometricConstraintKindV1::Horizontal { edge }));
    let oversized = prepared(&fixture.pattern, oversized_records);
    assert_eq!(
        certify_bounded_current_runtime_semantic_mus_v1(&oversized),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            direct_core_constraint_ids: Vec::new(),
            direct_oracle_calls: 0,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
    );
}

#[path = "constraint_semantic_mus_tests/singleton_phase.rs"]
mod singleton_phase;
