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

fn fixed_fixture_entity_id_v1<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\""))
        .expect("fixed fixture ID")
}
