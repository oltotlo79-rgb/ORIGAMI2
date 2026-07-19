use ori_domain::ProjectId;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{AppState, ProjectState, ensure_expected_project, lock_project};

const HISTORY_SETTINGS_SCHEMA_VERSION: u32 = 1;
const MIN_HISTORY_ENTRY_LIMIT: usize = 1;
const MAX_HISTORY_ENTRY_LIMIT: usize = 128;
const HISTORY_SETTINGS_INVALID_REQUEST: &str = "history settings request is invalid";
const HISTORY_SETTINGS_STALE_REQUEST: &str = "history settings request is stale";
const HISTORY_SETTINGS_UNAVAILABLE: &str = "history settings unavailable";

/// The complete project binding is echoed so the WebView can reject a delayed
/// response after an open/new/recovery replacement or a concurrent edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryEntryLimitResponse {
    schema_version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    history_entry_limit: usize,
}

/// A mutating history-preference request is bound to one exact open-project
/// generation. Unknown fields are rejected so a misspelled binding cannot be
/// silently ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SetHistoryEntryLimitRequest {
    schema_version: u32,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    history_entry_limit: usize,
}

#[tauri::command]
pub(super) fn get_history_entry_limit(
    state: State<'_, AppState>,
) -> Result<HistoryEntryLimitResponse, &'static str> {
    get_history_entry_limit_from_state(&state)
}

#[tauri::command]
pub(super) fn set_history_entry_limit(
    state: State<'_, AppState>,
    request: SetHistoryEntryLimitRequest,
) -> Result<HistoryEntryLimitResponse, &'static str> {
    set_history_entry_limit_in_state(&state, request)
}

fn get_history_entry_limit_from_state(
    state: &AppState,
) -> Result<HistoryEntryLimitResponse, &'static str> {
    let project = lock_project(state).map_err(|_| HISTORY_SETTINGS_UNAVAILABLE)?;
    Ok(response(&project))
}

fn set_history_entry_limit_in_state(
    state: &AppState,
    request: SetHistoryEntryLimitRequest,
) -> Result<HistoryEntryLimitResponse, &'static str> {
    validate_request(request)?;

    let mut project = lock_project(state).map_err(|_| HISTORY_SETTINGS_UNAVAILABLE)?;
    ensure_expected_project(
        &project,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    )
    .map_err(|_| HISTORY_SETTINGS_STALE_REQUEST)?;

    project
        .editor
        .set_history_entry_limit(request.history_entry_limit)
        .map_err(|_| HISTORY_SETTINGS_UNAVAILABLE)?;
    Ok(response(&project))
}

fn validate_request(request: SetHistoryEntryLimitRequest) -> Result<(), &'static str> {
    if request.schema_version != HISTORY_SETTINGS_SCHEMA_VERSION
        || !(MIN_HISTORY_ENTRY_LIMIT..=MAX_HISTORY_ENTRY_LIMIT)
            .contains(&request.history_entry_limit)
    {
        return Err(HISTORY_SETTINGS_INVALID_REQUEST);
    }
    Ok(())
}

fn response(project: &ProjectState) -> HistoryEntryLimitResponse {
    HistoryEntryLimitResponse {
        schema_version: HISTORY_SETTINGS_SCHEMA_VERSION,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        history_entry_limit: project.editor.history_entry_limit(),
    }
}

#[cfg(test)]
mod tests {
    use ori_core::{AppliedPoseLimitsV1, Command, EditorState, prepare_applied_pose_v1};
    use ori_domain::{EdgeId, FaceId, Point2, VertexId};
    use serde_json::{Value, json};

    use super::*;
    use crate::initial_project_state;

    fn binding(state: &AppState) -> (ProjectId, ProjectId, u64) {
        let project = lock_project(state).expect("lock history-settings fixture");
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    }

    fn request(
        binding: (ProjectId, ProjectId, u64),
        history_entry_limit: usize,
    ) -> SetHistoryEntryLimitRequest {
        SetHistoryEntryLimitRequest {
            schema_version: HISTORY_SETTINGS_SCHEMA_VERSION,
            expected_project_instance_id: binding.0,
            expected_project_id: binding.1,
            expected_revision: binding.2,
            history_entry_limit,
        }
    }

    fn runtime_pose(angle_degrees: f64) -> ori_core::AppliedPoseV1 {
        let mut faces = [FaceId::new(), FaceId::new()];
        faces.sort_by_key(FaceId::canonical_bytes);
        let hinge = EdgeId::new();
        prepare_applied_pose_v1(
            &faces,
            &[hinge],
            Some(faces[0]),
            &[(hinge, angle_degrees)],
            AppliedPoseLimitsV1::default(),
        )
        .expect("prepare history-settings pose fixture")
    }

    fn add_history_vertices(project: &mut ProjectState, count: usize) -> Vec<VertexId> {
        (0..count)
            .map(|index| {
                let id = VertexId::new();
                project
                    .editor
                    .execute(
                        project.editor.revision(),
                        Command::AddVertex {
                            id,
                            position: Point2::new(index as f64 + 1_000.0, 1_000.0),
                        },
                    )
                    .expect("add history-settings fixture vertex");
                id
            })
            .collect()
    }

    #[test]
    fn response_uses_the_exact_camel_case_v1_wire_shape() {
        let state = AppState::new(initial_project_state());
        let response = get_history_entry_limit_from_state(&state).expect("get history entry limit");
        let encoded = serde_json::to_value(response).expect("serialize history response");
        let (instance_id, project_id, revision) = binding(&state);

        assert_eq!(
            encoded,
            json!({
                "schemaVersion": 1,
                "projectInstanceId": instance_id,
                "projectId": project_id,
                "revision": revision,
                "historyEntryLimit": 128,
            })
        );
        assert_eq!(
            encoded.as_object().map(serde_json::Map::len),
            Some(5),
            "the response must not grow an accidental wire field"
        );
    }

    #[test]
    fn request_deserialization_is_exact_and_rejects_unknown_or_misspelled_fields() {
        let state = AppState::new(initial_project_state());
        let (instance_id, project_id, revision) = binding(&state);
        let exact = json!({
            "schemaVersion": 1,
            "expectedProjectInstanceId": instance_id,
            "expectedProjectId": project_id,
            "expectedRevision": revision,
            "historyEntryLimit": 32,
        });

        let decoded: SetHistoryEntryLimitRequest =
            serde_json::from_value(exact.clone()).expect("decode exact request");
        assert_eq!(decoded, request((instance_id, project_id, revision), 32));

        let mut unknown = exact.clone();
        unknown
            .as_object_mut()
            .expect("request object")
            .insert("unexpected".to_owned(), Value::Bool(true));
        assert!(serde_json::from_value::<SetHistoryEntryLimitRequest>(unknown).is_err());

        let mut misspelled = exact;
        let object = misspelled.as_object_mut().expect("request object");
        let value = object
            .remove("historyEntryLimit")
            .expect("history limit field");
        object.insert("history_entry_limit".to_owned(), value);
        assert!(serde_json::from_value::<SetHistoryEntryLimitRequest>(misspelled).is_err());
    }

    #[test]
    fn inclusive_limits_are_accepted_and_out_of_range_requests_are_atomic() {
        let state = AppState::new(initial_project_state());

        for limit in [MIN_HISTORY_ENTRY_LIMIT, MAX_HISTORY_ENTRY_LIMIT] {
            let current = binding(&state);
            let result = set_history_entry_limit_in_state(&state, request(current, limit))
                .expect("set inclusive history limit");
            assert_eq!(result.history_entry_limit, limit);
            assert_eq!(result.revision, current.2);
        }

        for (schema_version, limit) in [
            (HISTORY_SETTINGS_SCHEMA_VERSION, 0),
            (HISTORY_SETTINGS_SCHEMA_VERSION, MAX_HISTORY_ENTRY_LIMIT + 1),
            (HISTORY_SETTINGS_SCHEMA_VERSION + 1, 32),
        ] {
            let current = binding(&state);
            let before = {
                let project = lock_project(&state).expect("lock before invalid request");
                format!("{:?}", project.editor)
            };
            let mut invalid = request(current, limit);
            invalid.schema_version = schema_version;
            assert_eq!(
                set_history_entry_limit_in_state(&state, invalid),
                Err(HISTORY_SETTINGS_INVALID_REQUEST)
            );
            let project = lock_project(&state).expect("lock after invalid request");
            assert_eq!(format!("{:?}", project.editor), before);
        }
    }

    #[test]
    fn every_stale_binding_dimension_is_rejected_atomically_with_a_fixed_error() {
        let state = AppState::new(initial_project_state());
        let current = binding(&state);
        let stale_requests = [
            SetHistoryEntryLimitRequest {
                expected_project_instance_id: ProjectId::new(),
                ..request(current, 7)
            },
            SetHistoryEntryLimitRequest {
                expected_project_id: ProjectId::new(),
                ..request(current, 7)
            },
            SetHistoryEntryLimitRequest {
                expected_revision: current.2 + 1,
                ..request(current, 7)
            },
        ];

        for stale in stale_requests {
            let before = {
                let project = lock_project(&state).expect("lock before stale request");
                format!("{:?}", project.editor)
            };
            assert_eq!(
                set_history_entry_limit_in_state(&state, stale),
                Err(HISTORY_SETTINGS_STALE_REQUEST)
            );
            let project = lock_project(&state).expect("lock after stale request");
            assert_eq!(format!("{:?}", project.editor), before);
        }
    }

    #[test]
    fn shrinking_preserves_document_revision_dirty_and_pose_while_trimming_both_stacks() {
        let mut project = initial_project_state();
        let vertices = add_history_vertices(&mut project, 4);
        project
            .editor
            .undo(project.editor.revision())
            .expect("undo fourth fixture vertex");
        project
            .editor
            .undo(project.editor.revision())
            .expect("undo third fixture vertex");
        let pose = runtime_pose(27.0);
        project.editor.adopt_current_applied_pose(pose.clone());

        let document_before = project.document();
        let revision_before = project.editor.revision();
        let dirty_before = project.is_dirty();
        let authority_before = project
            .applied_pose_authority
            .test_snapshot()
            .expect("snapshot pose authority");
        let state = AppState::new(project);
        let current = binding(&state);

        let response = set_history_entry_limit_in_state(&state, request(current, 1))
            .expect("shrink history limit");
        assert_eq!(response.history_entry_limit, 1);
        assert_eq!(response.revision, revision_before);

        let retained_editor = {
            let project = lock_project(&state).expect("lock shrunken project");
            assert_eq!(project.document(), document_before);
            assert_eq!(project.editor.revision(), revision_before);
            assert_eq!(project.is_dirty(), dirty_before);
            assert_eq!(project.editor.current_applied_pose(), Some(&pose));
            assert_eq!(
                project
                    .applied_pose_authority
                    .test_snapshot()
                    .expect("snapshot pose authority after shrink"),
                authority_before
            );
            project.editor.clone()
        };

        let mut undo_editor = retained_editor.clone();
        undo_editor
            .undo(undo_editor.revision())
            .expect("undo retained newest entry");
        assert!(!undo_editor.can_undo());
        assert!(
            undo_editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[0]),
            "the discarded oldest undo entry must not be recoverable"
        );
        assert!(
            !undo_editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[1]),
            "the newest retained undo entry must still apply"
        );

        let mut redo_editor = retained_editor;
        redo_editor
            .redo(redo_editor.revision())
            .expect("redo retained newest entry");
        assert!(!redo_editor.can_redo());
        assert!(
            redo_editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[2]),
            "the newest retained redo entry must still apply"
        );
        assert!(
            !redo_editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[3]),
            "the discarded oldest redo entry must not be recoverable"
        );
    }

    #[test]
    fn increasing_the_limit_does_not_restore_trimmed_history() {
        let mut project = initial_project_state();
        let vertices = add_history_vertices(&mut project, 3);
        let state = AppState::new(project);

        let current = binding(&state);
        set_history_entry_limit_in_state(&state, request(current, 1))
            .expect("shrink history limit");
        let current = binding(&state);
        set_history_entry_limit_in_state(&state, request(current, 8))
            .expect("increase history limit");

        let mut editor = lock_project(&state)
            .expect("lock expanded project")
            .editor
            .clone();
        editor
            .undo(editor.revision())
            .expect("undo sole retained entry");
        assert!(!editor.can_undo());
        assert!(
            editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[0])
        );
        assert!(
            editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[1])
        );
        assert!(
            !editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == vertices[2])
        );
    }

    #[test]
    fn response_builder_reports_the_active_editor_limit_without_mutating_it() {
        let mut project = initial_project_state();
        project
            .editor
            .set_history_entry_limit(19)
            .expect("configure fixture limit");
        let before: EditorState = project.editor.clone();
        let response = response(&project);

        assert_eq!(response.history_entry_limit, 19);
        assert_eq!(response.revision, before.revision());
        assert_eq!(format!("{:?}", project.editor), format!("{before:?}"));
    }
}
