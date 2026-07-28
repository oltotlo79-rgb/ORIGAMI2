use super::atomic_revert::{
    RevertProofLocationV1, RevertProofOutcomeV1, RevertProofReasonV1,
    revert_post_apply_proof_failure_inner_v1,
    revert_post_apply_proof_failure_with_interleave_for_test_v1, revert_unavailable_message_v1,
    validate_revert_request_v1,
};
use super::*;
use crate::global_flat_foldability::GlobalFlatFoldabilityState;
use ori_core::SpeculativeUnprovenFoldHistoryLocationV1;

fn prepare_started_actual_job_v1() -> (
    AppState,
    StackedFoldTransactionState,
    PostApplyProofJobRequestV1,
    usize,
) {
    let (app_state, transaction_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let started = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("start retained proof premise");
    let total_pair_count = started.total_pair_count;
    (
        app_state,
        transaction_state,
        PostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
            job_token: started.job_token,
        },
        total_pair_count,
    )
}

fn execute_memo_edit_v1(app_state: &AppState, memo: &str) -> u64 {
    let mut project = crate::lock_project(app_state).expect("project");
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    crate::execute_command(
        &mut project,
        instance_id,
        project_id,
        revision,
        ori_core::Command::UpdateProjectMemo {
            memo: memo.to_owned(),
        },
    )
    .expect("memo edit");
    project.editor.revision()
}

fn execute_undo_once_v1(app_state: &AppState) -> u64 {
    let mut project = crate::lock_project(app_state).expect("project");
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    crate::execute_undo(&mut project, instance_id, project_id, revision).expect("Undo");
    project.editor.revision()
}

fn cancel_and_poll_v1(
    app_state: &AppState,
    transaction_state: &StackedFoldTransactionState,
    request: &PostApplyProofJobRequestV1,
) -> PostApplyProofProgressV1 {
    cancel_post_apply_proof_job_inner_v1(app_state, transaction_state, request.clone())
        .expect("cancel retained proof premise");
    tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        app_state,
        transaction_state,
        request.clone(),
    ))
    .expect("poll cancelled proof premise")
}

fn cancelled_revert_request_v1(
    request: &PostApplyProofJobRequestV1,
    expected_revision: u64,
    subsequent_edit_count: u64,
) -> RevertPostApplyProofFailureRequestV1 {
    RevertPostApplyProofFailureRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: request.project_instance_id,
        project_id: request.project_id,
        expected_revision,
        job_token: request.job_token,
        expected_location: RevertProofLocationV1::AppliedRetainedUndo,
        expected_outcome: RevertProofOutcomeV1::Unknown,
        expected_reason: Some(RevertProofReasonV1::Cancelled),
        expected_subsequent_edit_count: subsequent_edit_count,
        expected_undo_steps_to_revert: u32::try_from(subsequent_edit_count)
            .ok()
            .and_then(|count| count.checked_add(1)),
        explicit_confirmation: true,
    }
}

fn proof_failure_json_v1(progress: PostApplyProofProgressV1) -> serde_json::Value {
    serde_json::to_value(progress.proof_failure.expect("coarse proof failure"))
        .expect("proof failure JSON")
}

#[test]
fn progressive_schedule_and_resource_bounds_are_fixed() {
    assert_eq!(POST_APPLY_PROOF_SAMPLE_INTERVALS_V1, [16, 32, 64]);
    assert_eq!(
        POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.iter().sum::<usize>(),
        POST_APPLY_PROOF_TOTAL_WORK_V1
    );
    assert!(MAX_POST_APPLY_PROOF_JOBS_V1 > 0);
    assert!(MAX_POST_APPLY_PROOF_JOB_BYTES_V1 <= MAX_POST_APPLY_PROOF_RETAINED_BYTES_V1);
    assert!(POST_APPLY_PROOF_DEADLINE_V1 < POST_APPLY_PROOF_START_RETENTION_V1);
}

#[test]
fn progress_wire_is_coarse_and_contains_no_geometry() {
    let instance = ProjectId::new();
    let project = ProjectId::new();
    let token = ProjectId::new();
    let progress = PostApplyProofProgressV1 {
        version: 1,
        project_instance_id: instance,
        project_id: project,
        revision: 9,
        job_token: token,
        status: "proving",
        proven_pair_count: 0,
        total_pair_count: 3,
        proof_failure: None,
    };
    let value = serde_json::to_value(progress).expect("progress JSON");
    assert_eq!(
        value,
        serde_json::json!({
            "version": 1,
            "projectInstanceId": instance,
            "projectId": project,
            "revision": 9,
            "jobToken": token,
            "status": "proving",
            "provenPairCount": 0,
            "totalPairCount": 3,
            "proofFailure": null
        })
    );
    let keys = value
        .as_object()
        .expect("progress object")
        .keys()
        .collect::<Vec<_>>();
    for forbidden in [
        "geometry",
        "fingerprint",
        "coordinate",
        "vertex",
        "edge",
        "face",
        "angle",
    ] {
        assert!(
            keys.iter().all(|key| !key.contains(forbidden)),
            "leaked {forbidden}"
        );
    }
}

#[test]
fn v1_trusts_only_its_explicit_continuous_certificate_models() {
    for model in [
        STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
        STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
        STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
        STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
        STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
        STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    ] {
        assert!(trusted_continuous_certificate_model_v1(model));
    }
    assert!(
        !trusted_continuous_certificate_model_v1("stacked_fold_future_continuous_certificate_v2"),
        "a future model must not silently gain V1 authority"
    );
    for retired in [
        "stacked_fold_single_hinge_positive_thickness_continuous_certificate_v1",
        "stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1",
    ] {
        assert!(
            !trusted_continuous_certificate_model_v1(retired),
            "a retired positive-thickness V1 model must not retain authority"
        );
    }
}

#[test]
fn deadline_precedes_both_positive_and_negative_proof_results() {
    for completed in [
        PostApplyProofTerminalV1::Certified,
        PostApplyProofTerminalV1::Blocked,
    ] {
        assert_eq!(
            terminal_after_attempt_v1(true, AttemptResultV1::Terminal(completed)),
            Some(PostApplyProofTerminalV1::UnknownDeadlineReached)
        );
    }
}

#[test]
fn an_in_flight_result_is_ignored_after_cancel_terminal_wins() {
    assert!(run_result_is_current_v1(
        PostApplyProofJobStateV1::InFlight {
            run_generation: 7,
            stage: 1,
        },
        7,
        1,
    ));
    assert!(!run_result_is_current_v1(
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::UnknownCancelled),
        7,
        1,
    ));
    assert!(!run_result_is_current_v1(
        PostApplyProofJobStateV1::InFlight {
            run_generation: 8,
            stage: 1,
        },
        7,
        1,
    ));
}

#[test]
fn an_actual_applied_premise_starts_and_cancel_resolves_only_its_mark() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, instance_id, project_id, revision) =
        crate::stacked_fold_read::tests::prepare_applied_speculative_project_with_scheduler_v1();
    let document_before_cancel = crate::lock_project(&app_state)
        .expect("applied project")
        .document();

    let started = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .expect("start retained proof premise");
    assert_eq!(started.status, "proving");
    assert_eq!(started.proven_pair_count, 0);
    assert!(started.total_pair_count > 0);
    let request = PostApplyProofJobRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: instance_id,
        project_id,
        revision,
        job_token: started.job_token,
    };

    cancel_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        PostApplyProofJobRequestV1 {
            version: request.version,
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
            job_token: request.job_token,
        },
    )
    .expect("cancel retained proof premise");
    let terminal = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("poll cancelled proof premise");
    assert_eq!(terminal.status, "unknown_cancelled");
    assert_eq!(terminal.proven_pair_count, 0);
    assert_eq!(terminal.total_pair_count, started.total_pair_count);

    let project = crate::lock_project(&app_state).expect("resolved project");
    assert_eq!(project.editor.revision(), revision);
    assert_eq!(project.document(), document_before_cancel);
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_cancelled, 1);
    assert_eq!(summary.applied.total(), 1);
}

#[test]
fn terminal_failure_report_tracks_later_edits_undo_and_history_trim() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();

    let (app_state, transaction_state, request, total_pair_count) = prepare_started_actual_job_v1();
    execute_memo_edit_v1(&app_state, "after speculative Apply");
    let terminal = cancel_and_poll_v1(&app_state, &transaction_state, &request);
    assert_eq!(terminal.status, "unknown_cancelled");
    assert_eq!(terminal.total_pair_count, total_pair_count);
    assert_eq!(
        proof_failure_json_v1(terminal),
        serde_json::json!({
            "location": "applied_retained_undo",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 1,
            "undoStepsToRevert": 2
        })
    );

    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    execute_undo_once_v1(&app_state);
    let terminal = cancel_and_poll_v1(&app_state, &transaction_state, &request);
    assert_eq!(
        proof_failure_json_v1(terminal),
        serde_json::json!({
            "location": "unapplied_redo",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 0,
            "undoStepsToRevert": null
        })
    );
    let project = crate::lock_project(&app_state).expect("undone project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.total(), 0);
    assert_eq!(summary.unapplied_redo.unknown_cancelled, 1);
    drop(project);

    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    {
        let mut project = crate::lock_project(&app_state).expect("project");
        project
            .editor
            .set_history_entry_limit(1)
            .expect("one-entry history");
        project.trim_numeric_expression_history(1);
    }
    execute_memo_edit_v1(&app_state, "trim speculative Apply into base");
    let terminal = cancel_and_poll_v1(&app_state, &transaction_state, &request);
    assert_eq!(
        proof_failure_json_v1(terminal),
        serde_json::json!({
            "location": "applied_trimmed_base",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 1,
            "undoStepsToRevert": null
        })
    );
    let project = crate::lock_project(&app_state).expect("trimmed project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.unknown_cancelled, 1);
    assert_eq!(summary.unapplied_redo.total(), 0);
}

#[test]
fn atomic_revert_rechecks_dynamic_report_and_undoes_the_exact_suffix() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    execute_memo_edit_v1(&app_state, "first later edit");
    let terminal = cancel_and_poll_v1(&app_state, &transaction_state, &request);
    assert_eq!(
        proof_failure_json_v1(terminal),
        serde_json::json!({
            "location": "applied_retained_undo",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 1,
            "undoStepsToRevert": 2
        })
    );

    let current_revision = execute_memo_edit_v1(&app_state, "second later edit");
    let before_rejected = {
        let project = crate::lock_project(&app_state).expect("project");
        (
            project.document(),
            project.editor.revision(),
            project.numeric_expressions.clone(),
            project
                .applied_pose_authority
                .test_snapshot()
                .expect("pose authority"),
            project.current_layer_evidence.clone(),
        )
    };
    let stale_confirmed = cancelled_revert_request_v1(&request, current_revision, 1);
    let rejected = tauri::async_runtime::block_on(revert_post_apply_proof_failure_inner_v1(
        &app_state,
        &GlobalFlatFoldabilityState::default(),
        &transaction_state,
        stale_confirmed,
    ));
    assert_eq!(rejected.unwrap_err(), revert_unavailable_message_v1());
    let project = crate::lock_project(&app_state).expect("unchanged project");
    assert_eq!(project.document(), before_rejected.0);
    assert_eq!(project.editor.revision(), before_rejected.1);
    assert_eq!(project.numeric_expressions, before_rejected.2);
    assert_eq!(
        project
            .applied_pose_authority
            .test_snapshot()
            .expect("pose authority"),
        before_rejected.3
    );
    assert_eq!(project.current_layer_evidence, before_rejected.4);
    drop(project);

    let foldability_state = GlobalFlatFoldabilityState::default();
    let before_authority = crate::lock_project(&app_state)
        .expect("project")
        .applied_pose_authority
        .test_snapshot()
        .expect("pose authority");
    let reverted_revision =
        tauri::async_runtime::block_on(revert_post_apply_proof_failure_inner_v1(
            &app_state,
            &foldability_state,
            &transaction_state,
            cancelled_revert_request_v1(&request, current_revision, 2),
        ))
        .expect("atomic exact-suffix revert");
    assert_eq!(reverted_revision, current_revision + 3);

    let project = crate::lock_project(&app_state).expect("reverted project");
    assert_eq!(project.editor.revision(), reverted_revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert_eq!(project.numeric_expressions.undo_stack.len(), 0);
    assert_eq!(project.numeric_expressions.vertex_undo_stack.len(), 0);
    assert_eq!(project.numeric_expressions.redo_stack.len(), 3);
    assert_eq!(project.numeric_expressions.vertex_redo_stack.len(), 3);
    assert!(project.current_layer_evidence.is_none());
    let after_authority = project
        .applied_pose_authority
        .test_snapshot()
        .expect("pose authority");
    assert_eq!(after_authority.generation, before_authority.generation + 1);
    assert!(!after_authority.has_current);
    assert!(!after_authority.has_pending);
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.total(), 0);
    assert_eq!(summary.unapplied_redo.unknown_cancelled, 1);
    drop(project);

    let refreshed = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("poll reverted proof premise");
    assert_eq!(
        proof_failure_json_v1(refreshed),
        serde_json::json!({
            "location": "unapplied_redo",
            "outcome": "unknown",
            "reason": "cancelled",
            "subsequentEditCount": 0,
            "undoStepsToRevert": null
        })
    );
}

#[test]
fn atomic_revert_rejects_a_revision_neutral_history_limit_race_without_overwriting_it() {
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };

    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let current_revision = execute_memo_edit_v1(&app_state, "later edit retained by the race");
    let terminal = cancel_and_poll_v1(&app_state, &transaction_state, &request);
    let revert_request = cancelled_revert_request_v1(&request, current_revision, 1);

    let app_state = Arc::new(app_state);
    let transaction_state = Arc::new(transaction_state);
    let worker_app_state = Arc::clone(&app_state);
    let worker_transaction_state = Arc::clone(&transaction_state);
    let (captured_tx, captured_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let worker = std::thread::spawn(move || {
        let foldability_state = GlobalFlatFoldabilityState::default();
        tauri::async_runtime::block_on(revert_post_apply_proof_failure_with_interleave_for_test_v1(
            &worker_app_state,
            &foldability_state,
            &worker_transaction_state,
            revert_request,
            move || {
                captured_tx.send(()).expect("signal captured revert input");
                release_rx.recv().expect("release captured revert input");
            },
        ))
    });

    captured_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("revert reached its unlocked preparation window");
    let expected_after_setting = {
        let mut project = crate::lock_project(&app_state).expect("project during revert race");
        assert_eq!(project.editor.history_entry_limit(), 128);
        let numeric_history_shape = (
            project.numeric_expressions.undo_stack.len(),
            project.numeric_expressions.redo_stack.len(),
            project.numeric_expressions.vertex_undo_stack.len(),
            project.numeric_expressions.vertex_redo_stack.len(),
        );
        assert!(
            [
                numeric_history_shape.0,
                numeric_history_shape.1,
                numeric_history_shape.2,
                numeric_history_shape.3,
            ]
            .into_iter()
            .all(|entries| entries <= 64)
        );
        project
            .editor
            .set_history_entry_limit(64)
            .expect("revision-neutral history limit");
        project.trim_numeric_expression_history(64);
        assert_eq!(project.editor.revision(), current_revision);
        assert_eq!(
            (
                project.numeric_expressions.undo_stack.len(),
                project.numeric_expressions.redo_stack.len(),
                project.numeric_expressions.vertex_undo_stack.len(),
                project.numeric_expressions.vertex_redo_stack.len(),
            ),
            numeric_history_shape,
            "128 -> 64 must exercise the no-trim race"
        );
        (
            format!("{:?}", project.editor),
            project.numeric_expressions.clone(),
            project.document(),
            project.current_layer_evidence.clone(),
            project
                .applied_pose_authority
                .test_snapshot()
                .expect("pose authority after setting"),
        )
    };
    let report_after_setting = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request.clone(),
    ))
    .expect("poll proof report after the no-trim setting change");
    assert_eq!(report_after_setting, terminal);
    release_tx.send(()).expect("release revert preparation");

    assert_eq!(
        worker.join().expect("join racing revert"),
        Err(revert_unavailable_message_v1())
    );
    let project = crate::lock_project(&app_state).expect("project after rejected revert");
    assert_eq!(project.editor.history_entry_limit(), 64);
    assert_eq!(project.editor.revision(), current_revision);
    assert_eq!(format!("{:?}", project.editor), expected_after_setting.0);
    assert_eq!(
        project.numeric_expressions, expected_after_setting.1,
        "numeric history must retain the accepted limit change"
    );
    assert_eq!(project.document(), expected_after_setting.2);
    assert_eq!(project.current_layer_evidence, expected_after_setting.3);
    assert_eq!(
        project
            .applied_pose_authority
            .test_snapshot()
            .expect("pose authority after rejected revert"),
        expected_after_setting.4
    );
    drop(project);

    let refreshed = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("poll proof report after rejected revert");
    assert_eq!(refreshed, terminal);
}

#[test]
fn revert_request_validation_accepts_only_confirmed_applied_failure_suffixes() {
    let request = PostApplyProofJobRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 9,
        job_token: ProjectId::new(),
    };
    let valid = cancelled_revert_request_v1(&request, 12, 2);
    assert_eq!(
        validate_revert_request_v1(&valid).expect("valid report"),
        SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo,
            outcome: SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::Cancelled,
            },
            subsequent_edit_count: 2,
            undo_steps_to_revert: Some(3),
        }
    );

    let mut no_confirmation = cancelled_revert_request_v1(&request, 12, 2);
    no_confirmation.explicit_confirmation = false;
    assert!(validate_revert_request_v1(&no_confirmation).is_err());

    let mut redo = cancelled_revert_request_v1(&request, 12, 2);
    redo.expected_location = RevertProofLocationV1::UnappliedRedo;
    assert!(validate_revert_request_v1(&redo).is_err());

    let mut trimmed = cancelled_revert_request_v1(&request, 12, 2);
    trimmed.expected_location = RevertProofLocationV1::AppliedTrimmedBase;
    assert!(validate_revert_request_v1(&trimmed).is_err());

    let mut reasonless_unknown = cancelled_revert_request_v1(&request, 12, 2);
    reasonless_unknown.expected_reason = None;
    assert!(validate_revert_request_v1(&reasonless_unknown).is_err());

    let mut reasoned_blocked = cancelled_revert_request_v1(&request, 12, 2);
    reasoned_blocked.expected_outcome = RevertProofOutcomeV1::Blocked;
    assert!(validate_revert_request_v1(&reasoned_blocked).is_err());

    let mut wrong_steps = cancelled_revert_request_v1(&request, 12, 2);
    wrong_steps.expected_undo_steps_to_revert = Some(2);
    assert!(validate_revert_request_v1(&wrong_steps).is_err());
}
