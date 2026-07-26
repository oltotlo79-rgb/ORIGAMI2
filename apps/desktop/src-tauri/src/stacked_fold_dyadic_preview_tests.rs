use super::*;
use crate::{AppState, initial_project_state, lock_project};

fn project_binding(state: &AppState) -> (ProjectId, ProjectId, u64) {
    let project = lock_project(state).unwrap();
    (
        project.instance_id,
        project.project_id,
        project.editor.revision(),
    )
}

fn install_test_record(
    state: &DyadicPathPreviewState,
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
) {
    state.install_record_for_test(
        token,
        instance_id,
        project_id,
        revision,
        [0x11; 32],
        "22".repeat(32),
        "33".repeat(32),
        "44".repeat(32),
        None,
    );
}

fn apply_request(
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
) -> ApplyDyadicPathPreviewRequestV1 {
    ApplyDyadicPathPreviewRequestV1 {
        preview_token: token,
        expected_project_instance_id: instance_id,
        expected_project_id: project_id,
        expected_revision: revision,
        expected_target_binding_sha256: "11".repeat(32),
        expected_path_binding_sha256: "22".repeat(32),
        expected_positive_thickness_binding_sha256: "33".repeat(32),
        expected_layer_transport_binding_sha256: "44".repeat(32),
    }
}

#[test]
fn dyadic_preview_request_schemas_reject_unknown_fields() {
    let project_id = ProjectId::new();
    let mint = serde_json::json!({
        "expectedProjectInstanceId": project_id,
        "expectedProjectId": project_id,
        "expectedRevision": 0,
        "targetAngles": [],
        "maxStates": 1,
        "maxTransitions": 1,
        "levelCount": 3,
        "expectedPathBindingSha256": "00".repeat(32),
        "expectedPositiveThicknessBindingSha256": "11".repeat(32),
        "expectedLayerTransportBindingSha256": "22".repeat(32),
        "unexpected": true,
    });
    assert!(serde_json::from_value::<DyadicPathPreviewRequestV1>(mint).is_err());

    let apply = serde_json::json!({
        "previewToken": project_id,
        "expectedProjectInstanceId": project_id,
        "expectedProjectId": project_id,
        "expectedRevision": 0,
        "expectedTargetBindingSha256": "00".repeat(32),
        "expectedPathBindingSha256": "11".repeat(32),
        "expectedPositiveThicknessBindingSha256": "22".repeat(32),
        "expectedLayerTransportBindingSha256": "33".repeat(32),
        "unexpected": true,
    });
    assert!(serde_json::from_value::<ApplyDyadicPathPreviewRequestV1>(apply).is_err());

    let cancel = serde_json::json!({
        "previewToken": project_id,
        "unexpected": true,
    });
    assert!(serde_json::from_value::<CancelDyadicPathPreviewRequestV1>(cancel).is_err());
}

#[test]
fn foreign_preview_token_is_an_atomic_no_op_and_preserves_the_live_record() {
    let state = AppState::new(initial_project_state());
    let (instance_id, project_id, revision) = project_binding(&state);
    let preview_state = DyadicPathPreviewState::default();
    let token = ProjectId::new();
    install_test_record(&preview_state, token, instance_id, project_id, revision);

    let error = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &preview_state,
        apply_request(ProjectId::new(), instance_id, project_id, revision),
    )
    .unwrap_err();

    assert_eq!(error, CYCLE_PATH_UNCERTIFIED_MESSAGE);
    assert!(!preview_state.is_empty_for_test());
    let project = lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn stale_preview_occ_is_rejected_before_record_consumption_and_is_an_atomic_no_op() {
    let state = AppState::new(initial_project_state());
    let (instance_id, project_id, revision) = project_binding(&state);
    let stale_revision = revision + 1;
    let preview_state = DyadicPathPreviewState::default();
    let token = ProjectId::new();
    install_test_record(
        &preview_state,
        token,
        instance_id,
        project_id,
        stale_revision,
    );

    let error = apply_dyadic_pose_path_preview_inner_v1(
        &state,
        &GlobalFlatFoldabilityState::default(),
        &preview_state,
        apply_request(token, instance_id, project_id, stale_revision),
    )
    .unwrap_err();

    assert_eq!(error, STALE_MESSAGE);
    assert!(!preview_state.is_empty_for_test());
    let project = lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn preview_cancellation_is_one_shot_and_never_mutates_the_project() {
    let state = AppState::new(initial_project_state());
    let (instance_id, project_id, revision) = project_binding(&state);
    let preview_state = DyadicPathPreviewState::default();
    let token = ProjectId::new();
    install_test_record(&preview_state, token, instance_id, project_id, revision);

    cancel_dyadic_pose_path_preview_inner_v1(&preview_state, token).unwrap();
    assert!(preview_state.is_empty_for_test());
    assert_eq!(
        cancel_dyadic_pose_path_preview_inner_v1(&preview_state, token).unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE
    );

    let project = lock_project(&state).unwrap();
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}
