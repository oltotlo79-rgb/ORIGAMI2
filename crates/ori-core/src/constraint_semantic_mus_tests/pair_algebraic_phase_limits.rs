use super::pair_phase::algebraic_work_fixture;
use super::*;

#[derive(Default)]
struct AlgebraicRecordingObserver {
    progress: Vec<BoundedSemanticMusProgressV1>,
}

impl BoundedSemanticMusObserverV1 for AlgebraicRecordingObserver {
    fn checkpoint(
        &mut self,
        progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.progress.push(progress);
        BoundedSemanticMusObserverControlV1::Continue
    }
}

#[test]
fn algebraic_pair_reserves_full_work_and_has_pre_post_stop_checkpoints() {
    let fixture = algebraic_work_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, current, axis, _singleton, pair, algebraic, _length) =
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            3,
            2,
        )
        .expect("small algebraic pair work accounting");
    let common = current + axis + pair;
    let baseline = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    assert_eq!(
        baseline.deletion_witness_work(),
        setup + 3 * common + algebraic
    );
    assert_eq!(baseline.pair_constraint_constructive_witness_count(), 2);
    assert_eq!(baseline.pair_constraint_algebraic_witness_count(), 1);

    let fixed_id = fixture
        .records
        .iter()
        .find_map(|record| {
            matches!(
                record.constraint,
                GeometricConstraintKindV1::FixedLength { .. }
            )
            .then_some(record.id)
        })
        .expect("one fixed-length provider");
    let algebraic_index = baseline
        .constraint_ids()
        .iter()
        .position(|id| *id == fixed_id)
        .expect("provider belongs to direct core");
    let before_algebraic = setup + (algebraic_index + 1) * common;
    let checks = algebraic_index + 1;

    let mut one_short = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 3,
                max_deletion_witness_work: before_algebraic + algebraic - 1,
            },
            &mut one_short,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks,
            certified_deletion_witnesses,
            deletion_witness_work,
            ..
        } if deletion_witness_checks == checks
            && certified_deletion_witnesses == algebraic_index
            && deletion_witness_work == before_algebraic
    ));

    let boundary_work = before_algebraic + algebraic;
    let mut recording = AlgebraicRecordingObserver::default();
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1::default(),
            &mut recording,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Certified(_)
    ));
    let boundaries = recording
        .progress
        .iter()
        .enumerate()
        .filter_map(|(index, progress)| {
            (progress.deletion_witness_checks == checks
                && progress.certified_deletion_witnesses == algebraic_index
                && progress.deletion_witness_work == boundary_work)
                .then_some(index + 1)
        })
        .collect::<Vec<_>>();
    assert_eq!(boundaries.len(), 2);
    for (stop_at, control, reason) in [
        (
            boundaries[0],
            BoundedSemanticMusObserverControlV1::Cancelled,
            BoundedSemanticMusUnknownReasonV1::Cancelled,
        ),
        (
            boundaries[0],
            BoundedSemanticMusObserverControlV1::DeadlineReached,
            BoundedSemanticMusUnknownReasonV1::DeadlineReached,
        ),
        (
            boundaries[1],
            BoundedSemanticMusObserverControlV1::Cancelled,
            BoundedSemanticMusUnknownReasonV1::Cancelled,
        ),
        (
            boundaries[1],
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
                deletion_witness_checks,
                certified_deletion_witnesses,
                deletion_witness_work,
                ..
            } if actual == reason
                && deletion_witness_checks == checks
                && certified_deletion_witnesses == algebraic_index
                && deletion_witness_work == boundary_work
        ));
    }
}
