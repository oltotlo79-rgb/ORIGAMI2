use super::zero_closure_phase::{Provider, zero_closure_fixture};
use super::*;

#[derive(Default)]
struct ZeroClosureRecordingObserver {
    progress: Vec<BoundedSemanticMusProgressV1>,
}

impl BoundedSemanticMusObserverV1 for ZeroClosureRecordingObserver {
    fn checkpoint(
        &mut self,
        progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        self.progress.push(progress);
        BoundedSemanticMusObserverControlV1::Continue
    }
}

#[test]
fn zero_closure_phase_reserves_exact_work_and_rejects_one_short() {
    let fixture = zero_closure_fixture(Provider::FixedLength, true);
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, current, axis, _singleton, _pair, _algebraic, length) =
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            4,
            3,
        )
        .expect("small zero-closure common phase work");
    let deletions = canonical_deletions(&fixture.records);
    let zero_work = deletions
        .iter()
        .map(|deletion| {
            crate::constraint_semantic_mus::zero_length_closure_phase_work_for_test(
                fixture.pattern.vertices.len(),
                fixture.pattern.edges.len(),
                deletion,
            )
            .expect("small zero-closure phase work")
        })
        .collect::<Vec<_>>();
    let common = current + axis + length;
    let exact = setup + 4 * common + zero_work.iter().sum::<usize>();
    assert!(exact <= MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1);

    let baseline = certified(certify_bounded_current_runtime_semantic_mus_v1(&prepared));
    assert_eq!(baseline.deletion_witness_work(), exact);
    assert_eq!(baseline.zero_length_closure_constructive_witness_count(), 4,);

    let mut exact_observer = NoopBoundedSemanticMusObserverV1;
    assert!(matches!(
        certify_bounded_current_runtime_semantic_mus_with_observer_v1(
            &prepared,
            BoundedSemanticMusLimitsV1 {
                max_deletion_witness_checks: 4,
                max_deletion_witness_work: exact,
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
                max_deletion_witness_checks: 4,
                max_deletion_witness_work: setup + common + zero_work[0] - 1,
            },
            &mut one_short_observer,
        ),
        BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 0,
            deletion_witness_work,
            ..
        } if deletion_witness_work == setup + current + axis + length
    ));
}

#[test]
fn zero_closure_phase_has_cancel_and_deadline_checkpoints_before_and_after() {
    let fixture = zero_closure_fixture(Provider::FixedLength, false);
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let (setup, current, axis, _singleton, _pair, _algebraic, length) =
        crate::constraint_semantic_mus::witness_phase_work_for_test(
            fixture.pattern.vertices.len(),
            fixture.pattern.edges.len(),
            4,
            3,
        )
        .expect("small zero-closure common phase work");
    let deletions = canonical_deletions(&fixture.records);
    let first_zero = crate::constraint_semantic_mus::zero_length_closure_phase_work_for_test(
        fixture.pattern.vertices.len(),
        fixture.pattern.edges.len(),
        &deletions[0],
    )
    .expect("small zero-closure phase work");
    let first_boundary_work = setup + current + axis + length + first_zero;

    let mut recording = ZeroClosureRecordingObserver::default();
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

fn canonical_deletions(
    records: &[GeometricConstraintRecordV1],
) -> Vec<GeometricConstraintDocumentV1> {
    let mut records = records.to_vec();
    records.sort_unstable_by_key(|record| record.id.canonical_bytes());
    (0..records.len())
        .map(|removed| {
            document(
                records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != removed)
                    .map(|(_, record)| record.clone()),
            )
        })
        .collect()
}
