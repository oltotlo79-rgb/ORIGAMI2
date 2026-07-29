use super::*;

/// Request/revision-bound observation of deterministic-binary64 semantic-MUS
/// certification. The containing response supplies the exact project
/// instance, project, and revision binding. This DTO is never mutation
/// authority; replayability only states whether the same frozen model may be
/// re-certified on another covered target.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum GeometricConstraintSemanticMusResult {
    Certified {
        model_id: &'static str,
        transcendental_model_id: &'static str,
        constraint_ids: Vec<ConstraintId>,
        constraint_count: u32,
        direct_oracle_calls: u32,
        deletion_witness_checks: u32,
        deletion_witness_work: u32,
        current_assignment_witness_count: u32,
        axis_exactification_witness_count: u32,
        single_constraint_constructive_witness_count: u32,
        pair_constraint_constructive_witness_count: u32,
        pair_constraint_algebraic_witness_count: u32,
        length_constraint_constructive_witness_count: u32,
        zero_length_closure_constructive_witness_count: u32,
        anchored_mirror_residual_only_witness_count: u32,
        unit_parallel_fixed_angle_residual_only_witness_count: u32,
        unit_terminal_two_hop_parallel_angle_residual_only_witness_count: u32,
        unit_two_hop_parallel_residual_only_witness_count: u32,
        authorizes_project_mutation: bool,
        replayable_across_runtimes: bool,
    },
    Unknown {
        model_id: &'static str,
        transcendental_model_id: &'static str,
        reason: GeometricConstraintSemanticMusUnknownReason,
        direct_core_constraint_ids: Vec<ConstraintId>,
        direct_oracle_calls: u32,
        deletion_witness_checks: u32,
        certified_deletion_witnesses: u32,
        deletion_witness_work: u32,
        max_deletion_witness_checks: u32,
        max_deletion_witness_work: u32,
        authorizes_project_mutation: bool,
        replayable_across_runtimes: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GeometricConstraintSemanticMusUnknownReason {
    DirectOracleIncomplete,
    DeletionWitnessLimitExceeded,
    DeletionWitnessWorkLimitExceeded,
    DeletionWitnessUnavailable,
    Cancelled,
    DeadlineReached,
}

pub(super) fn analyze_semantic_direct_conflict_with<Certify>(
    prepared: &ori_core::GeometricConstraintSetV1<'_>,
    observer: &mut GeometricConstraintAnalysisObserver,
    certify: Certify,
) -> Result<(BoundedDirectMusResult, GeometricConstraintSemanticMusResult), ()>
where
    Certify: FnOnce(
        &ori_core::GeometricConstraintSetV1<'_>,
        &mut GeometricConstraintAnalysisObserver,
    ) -> ori_core::BoundedCurrentRuntimeSemanticMusV1,
{
    map_semantic_direct_conflict_result(prepared, certify(prepared, observer))
}

pub(super) fn map_semantic_direct_conflict_result(
    prepared: &ori_core::GeometricConstraintSetV1<'_>,
    result: ori_core::BoundedCurrentRuntimeSemanticMusV1,
) -> Result<(BoundedDirectMusResult, GeometricConstraintSemanticMusResult), ()> {
    match result {
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Certified(certificate) => {
            let constraint_ids =
                validated_semantic_core_ids(prepared, certificate.constraint_ids(), false)?;
            let constraint_count = checked_semantic_count(
                constraint_ids.len(),
                ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
            )?;
            let direct_oracle_calls = checked_semantic_count(
                certificate.direct_oracle_calls(),
                ori_core::MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
            )?;
            let deletion_witness_checks = checked_semantic_count(
                certificate.deletion_witness_checks(),
                ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
            )?;
            let deletion_witness_work = checked_semantic_count(
                certificate.deletion_witness_work(),
                ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1,
            )?;
            let current_assignment_witness_count = checked_semantic_count(
                certificate.current_assignment_witness_count(),
                constraint_ids.len(),
            )?;
            let axis_exactification_witness_count = checked_semantic_count(
                certificate.axis_exactification_witness_count(),
                constraint_ids.len(),
            )?;
            let single_constraint_constructive_witness_count = checked_semantic_count(
                certificate.single_constraint_constructive_witness_count(),
                constraint_ids.len(),
            )?;
            let pair_constraint_constructive_witness_count = checked_semantic_count(
                certificate.pair_constraint_constructive_witness_count(),
                constraint_ids.len(),
            )?;
            let pair_constraint_algebraic_witness_count = checked_semantic_count(
                certificate.pair_constraint_algebraic_witness_count(),
                constraint_ids.len(),
            )?;
            let length_constraint_constructive_witness_count = checked_semantic_count(
                certificate.length_constraint_constructive_witness_count(),
                constraint_ids.len(),
            )?;
            let zero_length_closure_constructive_witness_count = checked_semantic_count(
                certificate.zero_length_closure_constructive_witness_count(),
                constraint_ids.len(),
            )?;
            let anchored_mirror_residual_only_witness_count = checked_semantic_count(
                certificate.anchored_mirror_residual_only_witness_count(),
                constraint_ids.len(),
            )?;
            let unit_parallel_fixed_angle_residual_only_witness_count = checked_semantic_count(
                certificate.unit_parallel_fixed_angle_residual_only_witness_count(),
                constraint_ids.len(),
            )?;
            let unit_terminal_two_hop_parallel_angle_residual_only_witness_count =
                checked_semantic_count(
                    certificate.unit_terminal_two_hop_parallel_angle_residual_only_witness_count(),
                    constraint_ids.len(),
                )?;
            let unit_two_hop_parallel_residual_only_witness_count = checked_semantic_count(
                certificate.unit_two_hop_parallel_residual_only_witness_count(),
                constraint_ids.len(),
            )?;
            if deletion_witness_checks != constraint_count
                || deletion_witness_work == 0
                || (unit_parallel_fixed_angle_residual_only_witness_count != 0
                    && (unit_parallel_fixed_angle_residual_only_witness_count != 3
                        || constraint_count != 3))
                || (unit_terminal_two_hop_parallel_angle_residual_only_witness_count != 0
                    && (unit_terminal_two_hop_parallel_angle_residual_only_witness_count != 5
                        || constraint_count != 5))
                || current_assignment_witness_count
                    .checked_add(axis_exactification_witness_count)
                    .and_then(|count| {
                        count.checked_add(single_constraint_constructive_witness_count)
                    })
                    .and_then(|count| count.checked_add(pair_constraint_constructive_witness_count))
                    .and_then(|count| count.checked_add(pair_constraint_algebraic_witness_count))
                    .and_then(|count| {
                        count.checked_add(length_constraint_constructive_witness_count)
                    })
                    .and_then(|count| {
                        count.checked_add(zero_length_closure_constructive_witness_count)
                    })
                    .and_then(|count| {
                        count.checked_add(anchored_mirror_residual_only_witness_count)
                    })
                    .and_then(|count| {
                        count.checked_add(unit_parallel_fixed_angle_residual_only_witness_count)
                    })
                    .and_then(|count| {
                        count.checked_add(
                            unit_terminal_two_hop_parallel_angle_residual_only_witness_count,
                        )
                    })
                    .and_then(|count| {
                        count.checked_add(unit_two_hop_parallel_residual_only_witness_count)
                    })
                    != Some(constraint_count)
                || certificate.direct_oracle_calls() == 0
                || prepared.constraints().len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1
                || certificate.model_id()
                    != ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1
                || certificate.transcendental_model_id()
                    != ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
                || certificate.authorizes_project_mutation()
                || certificate.replayable_across_runtimes()
                    != ori_numeric::deterministic_transcendental_model_supported_v1()
            {
                return Err(());
            }
            let bounded_direct_mus = BoundedDirectMusResult::ProvenUnsatisfiable {
                constraint_ids: constraint_ids.clone(),
                oracle_calls: certificate.direct_oracle_calls(),
            };
            Ok((
                bounded_direct_mus,
                GeometricConstraintSemanticMusResult::Certified {
                    model_id: certificate.model_id(),
                    transcendental_model_id: certificate.transcendental_model_id(),
                    constraint_ids,
                    constraint_count,
                    direct_oracle_calls,
                    deletion_witness_checks,
                    deletion_witness_work,
                    current_assignment_witness_count,
                    axis_exactification_witness_count,
                    single_constraint_constructive_witness_count,
                    pair_constraint_constructive_witness_count,
                    pair_constraint_algebraic_witness_count,
                    length_constraint_constructive_witness_count,
                    zero_length_closure_constructive_witness_count,
                    anchored_mirror_residual_only_witness_count,
                    unit_parallel_fixed_angle_residual_only_witness_count,
                    unit_terminal_two_hop_parallel_angle_residual_only_witness_count,
                    unit_two_hop_parallel_residual_only_witness_count,
                    authorizes_project_mutation: certificate.authorizes_project_mutation(),
                    replayable_across_runtimes: certificate.replayable_across_runtimes(),
                },
            ))
        }
        ori_core::BoundedCurrentRuntimeSemanticMusV1::Unknown {
            reason,
            direct_core_constraint_ids,
            direct_oracle_calls,
            deletion_witness_checks,
            certified_deletion_witnesses,
            deletion_witness_work,
        } => {
            let direct_core_constraint_ids =
                validated_semantic_core_ids(prepared, &direct_core_constraint_ids, true)?;
            let direct_oracle_calls_dto = checked_semantic_count(
                direct_oracle_calls,
                ori_core::MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1,
            )?;
            let deletion_witness_checks_dto = checked_semantic_count(
                deletion_witness_checks,
                ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
            )?;
            let certified_deletion_witnesses_dto =
                checked_semantic_count(certified_deletion_witnesses, deletion_witness_checks)?;
            let deletion_witness_work_dto = checked_semantic_count(
                deletion_witness_work,
                ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1,
            )?;
            if deletion_witness_checks > direct_core_constraint_ids.len()
                || (direct_core_constraint_ids.is_empty()
                    && (deletion_witness_checks != 0
                        || certified_deletion_witnesses != 0
                        || deletion_witness_work != 0))
                || (!direct_core_constraint_ids.is_empty() && direct_oracle_calls == 0)
                || (!direct_core_constraint_ids.is_empty()
                    && prepared.constraints().len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1)
                || (direct_core_constraint_ids.is_empty()
                    && !matches!(
                        reason,
                        ori_core::BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete
                            | ori_core::BoundedSemanticMusUnknownReasonV1::Cancelled
                            | ori_core::BoundedSemanticMusUnknownReasonV1::DeadlineReached
                    ))
                || (!direct_core_constraint_ids.is_empty()
                    && matches!(
                        reason,
                        ori_core::BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete
                    ))
                || !semantic_unknown_phase_is_consistent(
                    reason,
                    direct_core_constraint_ids.is_empty(),
                    deletion_witness_checks,
                    certified_deletion_witnesses,
                    deletion_witness_work,
                )
            {
                return Err(());
            }
            let bounded_direct_mus = if direct_core_constraint_ids.is_empty() {
                BoundedDirectMusResult::Unknown {
                    reason: if prepared.constraints().len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1
                    {
                        BoundedDirectMusUnknownReason::ConstraintLimitExceeded
                    } else {
                        match reason {
                            ori_core::BoundedSemanticMusUnknownReasonV1::Cancelled => {
                                BoundedDirectMusUnknownReason::Cancelled
                            }
                            ori_core::BoundedSemanticMusUnknownReasonV1::DeadlineReached => {
                                BoundedDirectMusUnknownReason::DeadlineReached
                            }
                            _ => BoundedDirectMusUnknownReason::OracleIncomplete,
                        }
                    },
                    oracle_calls: direct_oracle_calls,
                    max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
                }
            } else {
                BoundedDirectMusResult::ProvenUnsatisfiable {
                    constraint_ids: direct_core_constraint_ids.clone(),
                    oracle_calls: direct_oracle_calls,
                }
            };
            Ok((
                bounded_direct_mus,
                GeometricConstraintSemanticMusResult::Unknown {
                    model_id:
                        ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1,
                    transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
                    reason: map_semantic_mus_unknown_reason(reason),
                    direct_core_constraint_ids,
                    direct_oracle_calls: direct_oracle_calls_dto,
                    deletion_witness_checks: deletion_witness_checks_dto,
                    certified_deletion_witnesses: certified_deletion_witnesses_dto,
                    deletion_witness_work: deletion_witness_work_dto,
                    max_deletion_witness_checks: u32::try_from(
                        ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
                    )
                    .map_err(|_| ())?,
                    max_deletion_witness_work: u32::try_from(
                        ori_core::MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1,
                    )
                    .map_err(|_| ())?,
                    authorizes_project_mutation: false,
                    replayable_across_runtimes: false,
                },
            ))
        }
    }
}

fn semantic_unknown_phase_is_consistent(
    reason: ori_core::BoundedSemanticMusUnknownReasonV1,
    direct_core_is_empty: bool,
    deletion_witness_checks: usize,
    certified_deletion_witnesses: usize,
    deletion_witness_work: usize,
) -> bool {
    if deletion_witness_checks > 0 && deletion_witness_work == 0 {
        return false;
    }
    match reason {
        ori_core::BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete => direct_core_is_empty,
        ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded => {
            !direct_core_is_empty
                && deletion_witness_checks == 0
                && certified_deletion_witnesses == 0
                && deletion_witness_work == 0
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded => {
            !direct_core_is_empty
                && if deletion_witness_checks == 0 {
                    certified_deletion_witnesses == 0 && deletion_witness_work == 0
                } else {
                    certified_deletion_witnesses < deletion_witness_checks
                }
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable => {
            !direct_core_is_empty
                && deletion_witness_checks >= 1
                && certified_deletion_witnesses < deletion_witness_checks
                && deletion_witness_work > 0
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::Cancelled
        | ori_core::BoundedSemanticMusUnknownReasonV1::DeadlineReached => true,
    }
}

fn validated_semantic_core_ids(
    prepared: &ori_core::GeometricConstraintSetV1<'_>,
    constraint_ids: &[ConstraintId],
    allow_empty: bool,
) -> Result<Vec<ConstraintId>, ()> {
    if (!allow_empty && constraint_ids.is_empty())
        || constraint_ids.len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1
        || constraint_ids
            .windows(2)
            .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
        || constraint_ids
            .iter()
            .any(|id| prepared.constraints().iter().all(|record| record.id != *id))
    {
        return Err(());
    }
    Ok(constraint_ids.to_vec())
}

fn checked_semantic_count(value: usize, maximum: usize) -> Result<u32, ()> {
    if value > maximum {
        return Err(());
    }
    u32::try_from(value).map_err(|_| ())
}

fn map_semantic_mus_unknown_reason(
    reason: ori_core::BoundedSemanticMusUnknownReasonV1,
) -> GeometricConstraintSemanticMusUnknownReason {
    match reason {
        ori_core::BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete => {
            GeometricConstraintSemanticMusUnknownReason::DirectOracleIncomplete
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded => {
            GeometricConstraintSemanticMusUnknownReason::DeletionWitnessLimitExceeded
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded => {
            GeometricConstraintSemanticMusUnknownReason::DeletionWitnessWorkLimitExceeded
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable => {
            GeometricConstraintSemanticMusUnknownReason::DeletionWitnessUnavailable
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::Cancelled => {
            GeometricConstraintSemanticMusUnknownReason::Cancelled
        }
        ori_core::BoundedSemanticMusUnknownReasonV1::DeadlineReached => {
            GeometricConstraintSemanticMusUnknownReason::DeadlineReached
        }
    }
}
