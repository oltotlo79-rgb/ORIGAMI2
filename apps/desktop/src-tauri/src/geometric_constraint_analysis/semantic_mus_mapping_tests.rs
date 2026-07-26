use super::*;

#[test]
fn semantic_observer_and_mapping_preserve_cancel_and_deadline_distinctions() {
    let progress = ori_core::BoundedSemanticMusProgressV1 {
        direct_oracle_calls: 0,
        deletion_witness_checks: 0,
        certified_deletion_witnesses: 0,
        deletion_witness_work: 0,
    };
    let mut cancelled = observer(
        true,
        std::time::Instant::now()
            .checked_add(Duration::from_secs(60))
            .expect("future cancellation deadline"),
    );
    assert_eq!(
        ori_core::BoundedSemanticMusObserverV1::checkpoint(&mut cancelled, progress),
        ori_core::BoundedSemanticMusObserverControlV1::Cancelled,
    );
    let mut deadline = observer(false, std::time::Instant::now());
    assert_eq!(
        ori_core::BoundedSemanticMusObserverV1::checkpoint(&mut deadline, progress),
        ori_core::BoundedSemanticMusObserverControlV1::DeadlineReached,
    );

    let fixture = semantic_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let direct_ids = prepared
        .constraints()
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    for (core_reason, dto_reason) in [
        (
            ori_core::BoundedSemanticMusUnknownReasonV1::Cancelled,
            GeometricConstraintSemanticMusUnknownReason::Cancelled,
        ),
        (
            ori_core::BoundedSemanticMusUnknownReasonV1::DeadlineReached,
            GeometricConstraintSemanticMusUnknownReason::DeadlineReached,
        ),
    ] {
        let (bounded, semantic) = map_semantic_direct_conflict_result(
            &prepared,
            ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
                reason: core_reason,
                direct_core_constraint_ids: direct_ids.clone(),
                direct_oracle_calls: 7,
                deletion_witness_checks: 0,
                certified_deletion_witnesses: 0,
                deletion_witness_work: 0,
            },
        )
        .expect("a stop after direct proof keeps the direct core");
        assert_eq!(
            bounded,
            BoundedDirectMusResult::ProvenUnsatisfiable {
                constraint_ids: direct_ids.clone(),
                oracle_calls: 7,
            },
        );
        assert!(matches!(
            semantic,
            GeometricConstraintSemanticMusResult::Unknown {
                reason,
                ref direct_core_constraint_ids,
                ..
            } if reason == dto_reason && direct_core_constraint_ids == &direct_ids
        ));
    }

    for (
        core_reason,
        dto_reason,
        deletion_witness_checks,
        certified_deletion_witnesses,
        deletion_witness_work,
    ) in [
        (
            ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            GeometricConstraintSemanticMusUnknownReason::DeletionWitnessLimitExceeded,
            0,
            0,
            0,
        ),
        (
            ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            GeometricConstraintSemanticMusUnknownReason::DeletionWitnessWorkLimitExceeded,
            1,
            0,
            1,
        ),
        (
            ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            GeometricConstraintSemanticMusUnknownReason::DeletionWitnessUnavailable,
            1,
            0,
            1,
        ),
    ] {
        let (bounded, semantic) = map_semantic_direct_conflict_result(
            &prepared,
            ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
                reason: core_reason,
                direct_core_constraint_ids: direct_ids.clone(),
                direct_oracle_calls: 7,
                deletion_witness_checks,
                certified_deletion_witnesses,
                deletion_witness_work,
            },
        )
        .expect("a structurally valid post-direct unknown reason maps exactly");
        assert_eq!(
            bounded,
            BoundedDirectMusResult::ProvenUnsatisfiable {
                constraint_ids: direct_ids.clone(),
                oracle_calls: 7,
            },
        );
        assert!(matches!(
            semantic,
            GeometricConstraintSemanticMusResult::Unknown {
                reason,
                ref direct_core_constraint_ids,
                ..
            } if reason == dto_reason && direct_core_constraint_ids == &direct_ids
        ));
    }
}

#[test]
fn mapper_rejects_noncanonical_ids_inconsistent_phases_and_unchecked_integers() {
    let fixture = semantic_fixture();
    let prepared = prepared(&fixture.pattern, fixture.records.iter().cloned());
    let canonical_direct_ids = prepared
        .constraints()
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    let mut reversed_direct_ids = canonical_direct_ids.clone();
    reversed_direct_ids.reverse();
    let invalid_results = [
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            direct_core_constraint_ids: reversed_direct_ids,
            direct_oracle_calls: 7,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            direct_core_constraint_ids: Vec::new(),
            direct_oracle_calls: ori_core::MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1 + 1,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete,
            direct_core_constraint_ids: Vec::new(),
            direct_oracle_calls: 0,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1 + 1,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            direct_core_constraint_ids: Vec::new(),
            direct_oracle_calls: 0,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            direct_core_constraint_ids: canonical_direct_ids.clone(),
            direct_oracle_calls: 7,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            direct_core_constraint_ids: canonical_direct_ids.clone(),
            direct_oracle_calls: 7,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 1,
            deletion_witness_work: 1,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            direct_core_constraint_ids: canonical_direct_ids.clone(),
            direct_oracle_calls: 7,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 1,
        },
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason: ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
            direct_core_constraint_ids: canonical_direct_ids,
            direct_oracle_calls: 7,
            deletion_witness_checks: 1,
            certified_deletion_witnesses: 1,
            deletion_witness_work: 1,
        },
    ];
    for invalid in invalid_results {
        assert_eq!(
            map_semantic_direct_conflict_result(&prepared, invalid),
            Err(()),
        );
    }
}
