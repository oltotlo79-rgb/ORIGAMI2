//! Native-only staging for bounded FOLD 1.2 3D frame previews.

use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use ori_domain::ProjectId;
use ori_formats::{Fold3dFramesPreviewV1, FoldImportLimits, read_fold_3d_frames_preview_v1};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{AppState, lock_project};

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const STALE: &str = "the FOLD 3D frame preview is stale";

#[derive(Default)]
pub(super) struct Fold3dFramesImportState(Mutex<Slot>);

#[derive(Default)]
struct Slot {
    pending: Option<Pending>,
    last_cancelled: Option<ProjectId>,
}

struct Pending {
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    preview: Fold3dFramesPreviewV1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Fold3dFramesPickerResponse {
    canceled: bool,
    preview: Option<Fold3dFramesMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fold3dFramesMetadata {
    token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    frame_count: usize,
    frames: Vec<Fold3dFrameMetadata>,
    authorizes_project_import: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fold3dFrameMetadata {
    index: usize,
    parent: Option<usize>,
    inherits: bool,
    vertex_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SelectFold3dFrameRequest {
    token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    frame_index: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SelectFold3dFrameResponse {
    token: ProjectId,
    frame_index: usize,
    vertex_count: usize,
    source_sha256_hex: String,
    render_coordinates_exposed: bool,
    authorizes_project_import: bool,
    authorizes_applied_pose: bool,
    authorizes_instruction_timeline: bool,
}

#[tauri::command]
pub(super) async fn preview_fold_3d_frames(
    app: AppHandle,
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
) -> Result<Fold3dFramesPickerResponse, String> {
    let (instance_id, project_id, revision) = {
        let project = lock_project(&app_state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    };
    {
        let mut slot = lock(&state)?;
        slot.pending = None;
    }
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("FOLD 1.2 3D frames", &["fold"])
        .set_title("FOLD 3D frames / FOLD 3Dフレーム")
        .blocking_pick_file()
    else {
        return Ok(Fold3dFramesPickerResponse {
            canceled: true,
            preview: None,
        });
    };
    let path = file
        .simplified()
        .into_path()
        .map_err(|_| "selected FOLD file is not local")?;
    let preview = tauri::async_runtime::spawn_blocking(move || load(&path))
        .await
        .map_err(|_| "FOLD 3D frame analysis failed".to_owned())??;
    {
        let project = lock_project(&app_state)?;
        if project.instance_id != instance_id
            || project.project_id != project_id
            || project.editor.revision() != revision
        {
            return Err(STALE.to_owned());
        }
    }
    let token = ProjectId::new();
    let metadata = metadata(token, instance_id, project_id, revision, &preview);
    lock(&state)?.pending = Some(Pending {
        token,
        instance_id,
        project_id,
        revision,
        preview,
    });
    Ok(Fold3dFramesPickerResponse {
        canceled: false,
        preview: Some(metadata),
    })
}

#[tauri::command]
pub(super) fn select_fold_3d_frame(
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
    request: SelectFold3dFrameRequest,
) -> Result<SelectFold3dFrameResponse, String> {
    let slot = lock(&state)?;
    let pending = slot.pending.as_ref().ok_or_else(|| STALE.to_owned())?;
    let project = lock_project(&app_state)?;
    if pending.token != request.token
        || pending.instance_id != request.expected_project_instance_id
        || pending.project_id != request.expected_project_id
        || pending.revision != request.expected_revision
        || project.instance_id != pending.instance_id
        || project.project_id != pending.project_id
        || project.editor.revision() != pending.revision
    {
        return Err(STALE.to_owned());
    }
    let selected = pending
        .preview
        .select_frame(request.frame_index)
        .map_err(|_| STALE.to_owned())?;
    Ok(SelectFold3dFrameResponse {
        token: pending.token,
        frame_index: selected.frame_index(),
        vertex_count: selected.vertices().len(),
        source_sha256_hex: selected
            .source_sha256()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        render_coordinates_exposed: false,
        authorizes_project_import: false,
        authorizes_applied_pose: false,
        authorizes_instruction_timeline: false,
    })
}

#[tauri::command]
pub(super) fn cancel_fold_3d_frames(
    state: State<'_, Fold3dFramesImportState>,
    token: ProjectId,
) -> Result<(), String> {
    cancel_pending(&state, token)
}

fn cancel_pending(state: &Fold3dFramesImportState, token: ProjectId) -> Result<(), String> {
    let mut slot = lock(&state)?;
    if slot
        .pending
        .as_ref()
        .is_some_and(|pending| pending.token == token)
    {
        slot.pending = None;
        slot.last_cancelled = Some(token);
        return Ok(());
    }
    if slot.last_cancelled == Some(token) {
        return Ok(());
    }
    Err(STALE.to_owned())
}

fn load(path: &Path) -> Result<Fold3dFramesPreviewV1, String> {
    if fs::metadata(path)
        .map_err(|_| "cannot inspect FOLD file")?
        .len()
        > MAX_BYTES
    {
        return Err("FOLD file is too large".to_owned());
    }
    let bytes = fs::read(path).map_err(|_| "cannot read FOLD file")?;
    read_fold_3d_frames_preview_v1(&bytes, FoldImportLimits::default())
        .map_err(|_| "FOLD 3D frames are invalid".to_owned())
}

fn metadata(
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    preview: &Fold3dFramesPreviewV1,
) -> Fold3dFramesMetadata {
    Fold3dFramesMetadata {
        token,
        project_instance_id: instance_id,
        project_id,
        revision,
        frame_count: preview.frames().len(),
        frames: preview
            .frames()
            .iter()
            .map(|frame| Fold3dFrameMetadata {
                index: frame.index(),
                parent: frame.parent(),
                inherits: frame.inherits(),
                vertex_count: frame.vertex_count(),
            })
            .collect(),
        authorizes_project_import: false,
    }
}

fn lock(state: &Fold3dFramesImportState) -> Result<MutexGuard<'_, Slot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "FOLD 3D frame registry is unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview() -> Fold3dFramesPreviewV1 {
        read_fold_3d_frames_preview_v1(
            br#"{"file_frames":[{"vertices_coords":[[0,0,0],[1,0,0],[0,1,0]]}]}"#,
            FoldImportLimits::default(),
        )
        .unwrap()
    }

    fn pending(token: ProjectId) -> Pending {
        Pending {
            token,
            instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision: 4,
            preview: preview(),
        }
    }

    #[test]
    fn cancel_is_idempotent_only_for_the_exact_native_token() {
        let state = Fold3dFramesImportState::default();
        let token = ProjectId::new();
        lock(&state).unwrap().pending = Some(pending(token));
        cancel_pending(&state, token).unwrap();
        cancel_pending(&state, token).unwrap();
        assert!(cancel_pending(&state, ProjectId::new()).is_err());
    }

    #[test]
    fn reentry_replaces_the_old_token_and_metadata_never_contains_coordinates() {
        let state = Fold3dFramesImportState::default();
        let old = ProjectId::new();
        let replacement = ProjectId::new();
        lock(&state).unwrap().pending = Some(pending(old));
        lock(&state).unwrap().pending = Some(pending(replacement));
        assert!(cancel_pending(&state, old).is_err());
        let slot = lock(&state).unwrap();
        let current = slot.pending.as_ref().unwrap();
        let value = serde_json::to_value(metadata(
            current.token,
            current.instance_id,
            current.project_id,
            current.revision,
            &current.preview,
        ))
        .unwrap();
        assert!(value.get("vertices").is_none());
        assert!(value["frames"][0].get("vertices").is_none());
        assert_eq!(value["authorizesProjectImport"], false);
    }
}
