use super::pair_phase::pair_work_fixture;
use super::*;

#[derive(Default)]
struct PairRecordingObserver {
    progress: Vec<BoundedSemanticMusProgressV1>,
}

impl BoundedSemanticMusObserverV1 for PairRecordingObserver {
    fn checkpoint(
        &mut self,
        progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.progress.push(progress);
        BoundedSemanticMusObserverControlV1::Continue
    }
}

#[test]
fn pair_phase_reserves_full_work_and_has_pre_post_stop_checkpoints() {
    let fixture = pair_work_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, current, axis, _singleton, pair) =
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            3,
            2,
        )
        .expect("small pair work accounting");
    let phase = current + axis + pair;
    let baseline = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    assert_eq!(baseline.deletion_witness_work(), setup + 3 * phase);
    assert_eq!(baseline.pair_constraint_constructive_witness_count(), 3);

    let mut one_short_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 3,
                max_deletion_witness_work: setup + phase - 1,
            },
            &mut one_short_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 0,
            deletion_witness_work,
            ..
        } if deletion_witness_work == setup + current + axis
    ));

    let first_boundary_work = setup + phase;
    let mut recording = PairRecordingObserver::default();
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
            (progress.deletion_witness_checks == 1
                && progress.certified_deletion_witnesses == 0
                && progress.deletion_witness_work == first_boundary_work)
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
                deletion_witness_checks: 1,
                certified_deletion_witnesses: 0,
                deletion_witness_work,
                ..
            } if actual == reason && deletion_witness_work == first_boundary_work
        ));
    }
}
