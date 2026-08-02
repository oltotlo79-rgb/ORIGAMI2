use ori_kinematics::{
    CycleScheduleDyadicEvaluationErrorV2, CycleScheduleDyadicEvaluationStopV2,
    CycleScheduleLimitsV1,
};

use super::super::*;
use super::support::{OrdinaryFixtureV2, schedule_limits_v2};

pub(super) fn assert_phase_accounting_v2(validated: &ValidatedInputV2<'_>) {
    let resources = validated.resources;
    assert_eq!(
        resources.charged_session_steady_retained_bytes,
        resources
            .charged_bridge_retained_bytes
            .checked_add(resources.charged_schedule_retained_bytes)
            .and_then(|value| value.checked_add(resources.charged_session_shell_bytes))
            .unwrap(),
        "the steady ledger includes proof carriers and session shell only"
    );
    let schedule_phase = resources
        .charged_session_steady_retained_bytes
        .checked_add(resources.charged_pending_partition_bytes)
        .and_then(|value| value.checked_add(resources.charged_schedule_evaluation_workspace_bytes))
        .unwrap();
    let builder_phase = resources
        .charged_session_steady_retained_bytes
        .checked_add(resources.charged_pending_partition_bytes)
        .and_then(|value| value.checked_add(resources.charged_angle_box_bytes))
        .and_then(|value| value.checked_add(resources.charged_interval_registry_workspace_bytes))
        .and_then(|value| value.checked_add(resources.charged_leaf_wrapper_overhead_bytes))
        .unwrap();
    let retained_pair_phase = resources
        .charged_session_steady_retained_bytes
        .checked_add(resources.charged_pending_partition_bytes)
        .and_then(|value| value.checked_add(resources.charged_leaf_retained_bytes))
        .and_then(|value| value.checked_add(resources.charged_face_aabb_bytes))
        .unwrap();
    assert_eq!(
        resources.charged_temporary_bytes,
        resources
            .charged_bridge_revalidation_phase_peak_bytes
            .max(schedule_phase)
            .max(builder_phase)
            .max(retained_pair_phase)
    );
    assert!(resources.charged_temporary_bytes >= builder_phase);
    let publication_phase = resources
        .charged_session_steady_retained_bytes
        .checked_add(resources.charged_publication_bytes)
        .unwrap();
    assert_eq!(
        resources.charged_aggregate_peak_bytes,
        resources.charged_temporary_bytes.max(publication_phase)
    );
}

pub(super) fn assert_schedule_checkpoint_contract_v2(fixture: &OrdinaryFixtureV2) {
    let limits = schedule_limits_v2(fixture);
    assert_eq!(
        fixture
            .schedule
            .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(
                64,
                CycleScheduleLimitsV1 {
                    max_hinges: 0,
                    ..limits
                },
                || Err(CycleScheduleDyadicEvaluationStopV2::Cancelled),
            )
            .unwrap_err(),
        CycleScheduleDyadicEvaluationErrorV2::Cancelled
    );
    let mut bound_successful_polls = 0usize;
    let bound = fixture
        .schedule
        .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(6, limits, || {
            bound_successful_polls += 1;
            Ok(())
        })
        .unwrap();
    let mut bound_mid_polls = 0usize;
    assert_eq!(
        fixture
            .schedule
            .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(6, limits, || {
                bound_mid_polls += 1;
                if bound_mid_polls == bound_successful_polls / 2 {
                    Err(CycleScheduleDyadicEvaluationStopV2::Cancelled)
                } else {
                    Ok(())
                }
            })
            .unwrap_err(),
        CycleScheduleDyadicEvaluationErrorV2::Cancelled
    );
    let mut bound_final_polls = 0usize;
    assert_eq!(
        fixture
            .schedule
            .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(6, limits, || {
                bound_final_polls += 1;
                if bound_final_polls == bound_successful_polls {
                    Err(CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded)
                } else {
                    Ok(())
                }
            })
            .unwrap_err(),
        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded
    );
    assert!(bound.peak_bytes() >= bound.angle_box_bytes());
}
