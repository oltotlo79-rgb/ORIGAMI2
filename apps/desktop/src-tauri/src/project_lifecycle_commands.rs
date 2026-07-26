//! Private native command boundary for project lifecycle operations.
//!
//! Snapshot, create, validate, open/save, and recent-project commands live here;
//! persistence, recovery, and project-folder mechanics remain in their own owners.

use super::*;

#[tauri::command]
pub(super) fn project_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project = lock_project(&state)?;
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn update_project_memo(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    memo: String,
) -> Result<ProjectSnapshot, String> {
    const MAX_PROJECT_MEMO_CHARS: usize = 16_000;
    if memo.chars().count() > MAX_PROJECT_MEMO_CHARS
        || memo
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err("project memo must contain at most 16000 printable characters".to_owned());
    }
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateProjectMemo { memo },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(super) async fn new_project(
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    name: String,
    width_expression: String,
    height_expression: String,
    thickness_mm: f64,
    cutting_allowed: bool,
    front_color: RgbaColor,
    back_color: RgbaColor,
) -> Result<ProjectSnapshot, String> {
    let (width_mm, height_mm) = evaluate_positive_millimetre_pair_in_worker(
        width_expression.clone(),
        height_expression.clone(),
    )
    .await
    .map_err(|error| error.user_input_message().to_owned())?;
    let mut project = lock_project(&state)?;
    let response = replace_with_new_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        NewProjectParameters {
            name,
            width_expression,
            height_expression,
            width_mm,
            height_mm,
            thickness_mm,
            cutting_allowed,
            front_color,
            back_color,
        },
    )?;
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &response);
    Ok(response)
}

#[tauri::command]
pub(super) async fn validate_project(
    state: State<'_, AppState>,
) -> Result<ValidationSnapshot, String> {
    validate_project_with_worker(&state, |input| Ok(analyze_validation_input(input))).await
}

#[tauri::command]
pub(super) async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
) -> Result<ProjectFileResponse, String> {
    let (expected_instance_id, expected_project_id, expected_revision, initial_directory) = {
        let project = lock_project(&state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            project
                .current_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        )
    };

    let mut dialog = app
        .dialog()
        .file()
        .add_filter("ORIGAMI2 project", &["ori2"])
        .set_title("Open ORIGAMI2 project");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }

    let Some(selected) = dialog.blocking_pick_file() else {
        return canceled_file_response(&state);
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| "選択されたファイルはローカルファイルではありません。".to_owned())?;
    let loaded = tauri::async_runtime::spawn_blocking(move || load_project_file(path))
        .await
        .map_err(|_| PROJECT_OPEN_TASK_FAILED_MESSAGE.to_owned())??;

    let mut project = lock_project(&state)?;
    let response = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        loaded,
    )?;
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &response.project);
    remember_current_project(&app, &state);
    Ok(response)
}

#[tauri::command]
pub(super) async fn save_project(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
) -> Result<ProjectFileResponse, String> {
    let saved_to_current_path = {
        let mut project = lock_project(&state)?;
        if let Some(path) = project.current_path.clone() {
            Some(save_project_to_path(&mut project, path)?)
        } else {
            None
        }
    };
    if let Some(response) = saved_to_current_path {
        let _ = recovery.clear_after_normal_completion(&state, &response.project);
        remember_current_project(&app, &state);
        return Ok(response);
    }
    let response = save_project_with_dialog(&app, &state)?;
    if !response.canceled {
        let _ = recovery.clear_after_normal_completion(&state, &response.project);
        remember_current_project(&app, &state);
    }
    Ok(response)
}

#[tauri::command]
pub(super) async fn save_project_as(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
) -> Result<ProjectFileResponse, String> {
    let response = save_project_with_dialog(&app, &state)?;
    if !response.canceled {
        let _ = recovery.clear_after_normal_completion(&state, &response.project);
        remember_current_project(&app, &state);
    }
    Ok(response)
}

fn recent_storage(app: &AppHandle) -> Result<recent_projects::FileRecentProjectStorage, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "recent_projects_unavailable".to_owned())?;
    Ok(recent_projects::FileRecentProjectStorage::new(
        root.join("recent-projects-v1.json"),
    ))
}

fn remember_current_project(app: &AppHandle, state: &AppState) {
    let Ok(project) = lock_project(state) else {
        return;
    };
    let (Some(path), name) = (project.current_path.clone(), project.name.clone()) else {
        return;
    };
    drop(project);
    // A lease serializes publication, while the storage CAS rejects a registry
    // loaded before another process committed. Reload once so both successful
    // normal saves remain in MRU order instead of silently losing one update.
    for _ in 0..2 {
        let Ok(mut storage) = recent_storage(app) else {
            return;
        };
        let mut registry = recent_projects::RecentProjectRegistry::load(&storage);
        if registry
            .remember(
                path.clone(),
                &name,
                &recent_projects::LocalRecentProjectFilesystem,
                &mut storage,
            )
            .is_ok()
        {
            return;
        }
    }
}

#[tauri::command]
pub(super) fn list_recent_projects(
    app: AppHandle,
) -> Result<Vec<recent_projects::RecentProjectView>, String> {
    let storage = recent_storage(&app)?;
    Ok(recent_projects::RecentProjectRegistry::load(&storage).views())
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum OpenRecentProjectResponse {
    Opened { file: ProjectFileResponse },
    Invalidated,
}

#[tauri::command]
pub(super) async fn open_recent_project(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    opaque_id: String,
) -> Result<OpenRecentProjectResponse, String> {
    let (expected_instance_id, expected_project_id, expected_revision) = {
        let project = lock_project(&state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    };
    let mut storage = recent_storage(&app)?;
    let mut registry = recent_projects::RecentProjectRegistry::load(&storage);
    let Some(path) = registry
        .select(
            &opaque_id,
            &recent_projects::LocalRecentProjectFilesystem,
            &mut storage,
        )
        .map_err(|_| "recent_projects_unavailable".to_owned())?
    else {
        return Ok(OpenRecentProjectResponse::Invalidated);
    };
    let loaded = tauri::async_runtime::spawn_blocking(move || load_project_file(path))
        .await
        .map_err(|_| PROJECT_OPEN_TASK_FAILED_MESSAGE.to_owned())??;
    let mut project = lock_project(&state)?;
    let file = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        loaded,
    )?;
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &file.project);
    remember_current_project(&app, &state);
    Ok(OpenRecentProjectResponse::Opened { file })
}
