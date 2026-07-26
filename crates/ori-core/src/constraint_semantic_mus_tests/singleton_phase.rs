use super::*;

fn different_fixed_lengths_fixture(current_length: f64) -> SemanticFixture {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    SemanticFixture {
        pattern: CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(current_length, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Auxiliary,
            }],
        },
        records: vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 2.0,
            }),
        ],
    }
}

fn blocked_different_fixed_lengths_fixture() -> SemanticFixture {
    let mut fixture = different_fixed_lengths_fixture(3.0);
    let target_end = fixture.pattern.edges[0].end;
    for point in [
        Point2::new(1.0, 0.0),
        Point2::new(17.0, 32.0),
        Point2::new(-15.0, 8.0),
        Point2::new(1025.0, -512.0),
        Point2::new(2.0, 0.0),
        Point2::new(18.0, 32.0),
        Point2::new(-14.0, 8.0),
        Point2::new(1026.0, -512.0),
    ] {
        let blocker = VertexId::new();
        fixture.pattern.vertices.push(Vertex {
            id: blocker,
            position: point,
        });
        fixture.pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: target_end,
            end: blocker,
            kind: EdgeKind::Auxiliary,
        });
    }
    fixture
}

#[test]
fn different_fixed_lengths_are_promoted_by_the_constructive_singleton_witness() {
    let fixture = different_fixed_lengths_fixture(1.0);
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let expected_ids = sorted_ids(fixture.records.iter().cloned());
    assert_eq!(
        find_bounded_direct_mus_v1(&prepared),
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids: expected_ids.clone(),
            oracle_calls: 3,
        },
    );

    let certificate = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    assert_eq!(certificate.constraint_ids(), expected_ids);
    assert_eq!(certificate.direct_oracle_calls(), 3);
    assert_eq!(certificate.deletion_witness_checks(), 2);
    assert_eq!(certificate.current_assignment_witness_count(), 1);
    assert_eq!(certificate.axis_exactification_witness_count(), 0);
    assert_eq!(
        certificate.single_constraint_constructive_witness_count(),
        1,
    );
    assert_eq!(certificate.pair_constraint_constructive_witness_count(), 0);
    assert_eq!(certificate.pair_constraint_algebraic_witness_count(), 0);
}

#[test]
fn unavailable_constructive_singletons_remain_unknown_with_phase_evidence() {
    let fixture = blocked_different_fixed_lengths_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let expected_ids = sorted_ids(fixture.records.iter().cloned());
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_v1(&prepared),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            direct_core_constraint_ids,
            direct_oracle_calls: 3,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 0,
            deletion_witness_work,
        } if direct_core_constraint_ids == expected_ids && deletion_witness_work > 0
    ));
}

#[test]
fn constructive_singleton_phase_reserves_its_complete_work_before_starting() {
    let fixture = different_fixed_lengths_fixture(3.0);
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let baseline = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    let (setup_work, current_work, axis_work, singleton_work, _pair_work, _algebraic_work) =
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            2,
            1,
        )
        .expect("small singleton work accounting");
    let phase_work = current_work + axis_work + singleton_work;
    assert_eq!(
        baseline.deletion_witness_work(),
        setup_work + 2 * phase_work,
    );
    assert_eq!(baseline.single_constraint_constructive_witness_count(), 2,);

    let one_short = setup_work + current_work + axis_work + singleton_work - 1;
    let mut observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 2,
                max_deletion_witness_work: one_short,
            },
            &mut observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 0,
            deletion_witness_work,
            ..
        } if deletion_witness_work == setup_work + current_work + axis_work
    ));
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
fn constructive_singleton_has_distinct_cancel_and_deadline_checkpoints_before_and_after() {
    let fixture = different_fixed_lengths_fixture(3.0);
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup_work, current_work, axis_work, singleton_work, _pair_work, _algebraic_work) =
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            2,
            1,
        )
        .expect("small singleton work accounting");
    let first_singleton_work = setup_work + current_work + axis_work + singleton_work;
    let mut recording = RecordingObserver::default();
    let complete = certify_bounded_current_runtime_semantic_mus_with_observer_v1(
        &prepared,
        BoundedSemanticMusLimitsV1::default(),
        &mut recording,
    );
    assert!(matches!(
        complete,
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    let singleton_boundaries = recording
        .progress
        .iter()
        .enumerate()
        .filter_map(|(index, progress)| {
            (progress.deletion_witness_checks == 1
                && progress.certified_deletion_witnesses == 0
                && progress.deletion_witness_work == first_singleton_work)
                .then_some(index + 1)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        singleton_boundaries.len(),
        2,
        "the constructive call must have one checkpoint on each side",
    );

    for (stop_at, control, reason) in [
        (
            singleton_boundaries[0],
            BoundedSemanticMusObserverControlV1::Cancelled,
            BoundedSemanticMusUnknownReasonV1::Cancelled,
        ),
        (
            singleton_boundaries[0],
            BoundedSemanticMusObserverControlV1::DeadlineReached,
            BoundedSemanticMusUnknownReasonV1::DeadlineReached,
        ),
        (
            singleton_boundaries[1],
            BoundedSemanticMusObserverControlV1::Cancelled,
            BoundedSemanticMusUnknownReasonV1::Cancelled,
        ),
        (
            singleton_boundaries[1],
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
                &prepared,
                BoundedSemanticMusLimitsV1::default(),
                &mut observer,
            ),
            BoundedCurrentRuntimeSemanticMusV1::Unknown {
                reason: actual,
                deletion_witness_checks: 1,
                certified_deletion_witnesses: 0,
                deletion_witness_work,
                ..
            } if actual == reason && deletion_witness_work == first_singleton_work
        ));
    }
}
