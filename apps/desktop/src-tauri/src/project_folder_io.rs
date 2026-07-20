//! Native filesystem adapter for the expanded project-folder format.
//!
//! The format core owns schema and content admission. This module owns only
//! native directory selection, no-follow filesystem admission, immutable
//! project capture, and create-new directory publication.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use ori_domain::ProjectId;
use ori_formats::{
    MAX_PROJECT_FOLDER_ENTRY_COUNT, MAX_PROJECT_FOLDER_MANIFEST_BYTES,
    MAX_PROJECT_FOLDER_PREVIEW_BYTES, MAX_PROJECT_FOLDER_TOTAL_BYTES, Ori2ProjectArchive,
    PROJECT_FOLDER_EDITOR_HISTORY_PATH, PROJECT_FOLDER_MANIFEST_PATH, PROJECT_FOLDER_PREVIEW_PATH,
    PROJECT_FOLDER_PROJECT_PATH, ProjectFolderArtifactV1, ProjectFolderEntryV1,
    ProjectFolderLimits, read_project_folder_v1_with_limits, write_project_folder_v1,
};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{
    AppState, ProjectSnapshot, ProjectState, commit_project_replacement, ensure_expected_project,
    lock_project, recovery::RecoveryRuntime, snapshot, validate_document_instruction_poses,
    validate_loaded_numeric_expression_bindings,
};

#[cfg(unix)]
#[path = "project_folder_io/unix.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "project_folder_io/windows.rs"]
mod platform;

use platform::{PinnedDirectory, PinnedFile};

const OPEN_TITLE_JA: &str = "展開フォルダー形式のORIGAMI2プロジェクトを開く";
const OPEN_TITLE_EN: &str = "Open expanded ORIGAMI2 project folder";
const SAVE_PARENT_TITLE_JA: &str =
    "展開フォルダー形式のORIGAMI2プロジェクトを保存する親フォルダーを選択";
const SAVE_PARENT_TITLE_EN: &str = "Choose a parent folder for the expanded ORIGAMI2 project";
const TARGET_SUFFIX: &str = ".origami2-folder";
const DEFAULT_TARGET_BASE: &str = "origami2-project";
const MAX_TARGET_BASE_BYTES: usize = 64;
const PROJECT_ID_SUFFIX_BYTES: usize = 8;
const PROJECT_ID_SUFFIX_HEX_CHARS: usize = PROJECT_ID_SUFFIX_BYTES * 2;
const MAX_ENUMERATED_ROOT_ENTRIES: usize = 4;
const MAX_ENUMERATED_PREVIEW_ENTRIES: usize = 1;
const STAGING_ATTEMPTS: usize = 128;
const ERROR_BUSY: &str = "project_folder_busy";
const ERROR_INVALID_LOCALE: &str = "project_folder_invalid_locale";
const ERROR_INVALID_REQUEST: &str = "project_folder_invalid_request";
const ERROR_OPEN_FAILED: &str = "project_folder_open_failed";
const ERROR_INVALID: &str = "project_folder_invalid";
const ERROR_TOO_LARGE: &str = "project_folder_too_large";
const ERROR_LINK_OR_SPECIAL: &str = "project_folder_link_or_special_entry";
const ERROR_RACE: &str = "project_folder_changed_during_read";
const ERROR_SAVE_FAILED: &str = "project_folder_save_failed";
const ERROR_TARGET_EXISTS: &str = "project_folder_target_exists";
const ERROR_STALE: &str = "project_folder_project_changed";

static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectFolderFilesystemError {
    InvalidRequest,
    OpenFailed,
    InvalidTree,
    TooLarge,
    LinkOrSpecialEntry,
    ChangedDuringRead,
    ReadFailed,
    WriteFailed,
    TargetExists,
    StaleProject,
}

impl ProjectFolderFilesystemError {
    const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => ERROR_INVALID_REQUEST,
            Self::OpenFailed => ERROR_OPEN_FAILED,
            Self::InvalidTree | Self::ReadFailed => ERROR_INVALID,
            Self::TooLarge => ERROR_TOO_LARGE,
            Self::LinkOrSpecialEntry => ERROR_LINK_OR_SPECIAL,
            Self::ChangedDuringRead => ERROR_RACE,
            Self::WriteFailed => ERROR_SAVE_FAILED,
            Self::TargetExists => ERROR_TARGET_EXISTS,
            Self::StaleProject => ERROR_STALE,
        }
    }
}

type FsResult<T> = Result<T, ProjectFolderFilesystemError>;

#[derive(Clone, Default)]
pub(super) struct ProjectFolderIoState {
    busy: Arc<AtomicBool>,
}

impl ProjectFolderIoState {
    fn try_acquire(&self) -> Result<ProjectFolderIoPermit, String> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ERROR_BUSY.to_owned())?;
        Ok(ProjectFolderIoPermit {
            busy: Arc::clone(&self.busy),
        })
    }
}

struct ProjectFolderIoPermit {
    busy: Arc<AtomicBool>,
}

impl Drop for ProjectFolderIoPermit {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
enum DialogLocale {
    Ja,
    En,
}

impl DialogLocale {
    fn parse(locale: &str) -> Result<Self, String> {
        match locale {
            "ja" => Ok(Self::Ja),
            "en" => Ok(Self::En),
            _ => Err(ERROR_INVALID_LOCALE.to_owned()),
        }
    }

    const fn open_title(self) -> &'static str {
        match self {
            Self::Ja => OPEN_TITLE_JA,
            Self::En => OPEN_TITLE_EN,
        }
    }

    const fn save_parent_title(self) -> &'static str {
        match self {
            Self::Ja => SAVE_PARENT_TITLE_JA,
            Self::En => SAVE_PARENT_TITLE_EN,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectFolderFileResponse {
    canceled: bool,
    project: ProjectSnapshot,
}

#[derive(Clone)]
struct ProjectBinding {
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    archive: Ori2ProjectArchive,
}

struct SaveCapture {
    binding: ProjectBinding,
    initial_directory: Option<PathBuf>,
    target_name: String,
}

struct OpenCapture {
    binding: ProjectBinding,
    initial_directory: Option<PathBuf>,
}

struct LoadedProjectFolder {
    replacement: ProjectState,
}

/// Opens a directory selected entirely inside the native process.
#[tauri::command]
pub(super) async fn open_project_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    io_state: State<'_, ProjectFolderIoState>,
    recovery: State<'_, RecoveryRuntime>,
    locale: String,
) -> Result<ProjectFolderFileResponse, String> {
    let locale = DialogLocale::parse(&locale)?;
    let permit = io_state.try_acquire()?;
    let capture = {
        let project = lock_project(&state)?;
        capture_open_binding(&project).map_err(error_string)?
    };

    let mut dialog = app.dialog().file().set_title(locale.open_title());
    if let Some(directory) = &capture.initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_pick_folder() else {
        return canceled_response(&state);
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| ERROR_OPEN_FAILED.to_owned())?;
    let (loaded, _permit) =
        tauri::async_runtime::spawn_blocking(move || (load_project_folder(path), permit))
            .await
            .map_err(|_| ERROR_OPEN_FAILED.to_owned())?;
    let loaded = loaded.map_err(error_string)?;

    let mut project = lock_project(&state)?;
    ensure_binding_current(&project, &capture.binding).map_err(error_string)?;
    commit_project_replacement(&mut project, loaded.replacement)
        .map_err(|_| ERROR_INVALID.to_owned())?;
    let response = ProjectFolderFileResponse {
        canceled: false,
        project: redacted_snapshot(&project),
    };
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &response.project);
    Ok(response)
}

/// Saves a captured project into a new child of a native-selected parent.
///
/// Existing target directories are never replaced by this V1 adapter.
#[tauri::command]
pub(super) async fn save_project_folder_as(
    app: AppHandle,
    state: State<'_, AppState>,
    io_state: State<'_, ProjectFolderIoState>,
    recovery: State<'_, RecoveryRuntime>,
    locale: String,
) -> Result<ProjectFolderFileResponse, String> {
    let locale = DialogLocale::parse(&locale)?;
    let permit = io_state.try_acquire()?;
    let capture = {
        let project = lock_project(&state)?;
        capture_save_source(&project).map_err(error_string)?
    };
    let archive_for_writer = capture.binding.archive.clone();
    let (artifact, permit) = tauri::async_runtime::spawn_blocking(move || {
        (
            write_project_folder_v1(&archive_for_writer)
                .map_err(|_| ProjectFolderFilesystemError::InvalidTree),
            permit,
        )
    })
    .await
    .map_err(|_| ERROR_SAVE_FAILED.to_owned())?;
    let artifact = artifact.map_err(error_string)?;

    let mut dialog = app.dialog().file().set_title(locale.save_parent_title());
    if let Some(directory) = &capture.initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_pick_folder() else {
        return canceled_response(&state);
    };
    let parent = selected
        .simplified()
        .into_path()
        .map_err(|_| ERROR_SAVE_FAILED.to_owned())?;
    let target_name = capture.target_name.clone();
    let (prepared, _permit) = tauri::async_runtime::spawn_blocking(move || {
        (
            prepare_new_project_folder(&parent, &target_name, artifact),
            permit,
        )
    })
    .await
    .map_err(|_| ERROR_SAVE_FAILED.to_owned())?;
    let mut prepared = prepared.map_err(error_string)?;

    // Publication and saved-baseline update form one project-state
    // linearization point. All expensive writing and verification happened
    // before this lock was reacquired.
    let mut project = lock_project(&state)?;
    ensure_binding_current(&project, &capture.binding).map_err(error_string)?;
    prepared.publish().map_err(error_string)?;
    drop(prepared);
    project.current_path = None;
    project.saved_revision = Some(capture.binding.revision);
    project.saved_document = Some(capture.binding.archive.document);
    let response = ProjectFolderFileResponse {
        canceled: false,
        project: redacted_snapshot(&project),
    };
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &response.project);
    Ok(response)
}

fn capture_open_binding(project: &ProjectState) -> FsResult<OpenCapture> {
    Ok(OpenCapture {
        binding: capture_binding(project)?,
        initial_directory: project
            .current_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf),
    })
}

fn capture_save_source(project: &ProjectState) -> FsResult<SaveCapture> {
    let binding = capture_binding(project)?;
    Ok(SaveCapture {
        target_name: target_folder_name(
            &binding.archive.document.name,
            binding.archive.document.project_id,
        ),
        initial_directory: project
            .current_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf),
        binding,
    })
}

fn capture_binding(project: &ProjectState) -> FsResult<ProjectBinding> {
    let archive = project
        .project_archive()
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
    validate_document_instruction_poses(&archive.document)
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
    super::restore_archive_editor(&archive)
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
    Ok(ProjectBinding {
        instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        archive,
    })
}

fn ensure_binding_current(project: &ProjectState, binding: &ProjectBinding) -> FsResult<()> {
    ensure_expected_project(
        project,
        binding.instance_id,
        binding.project_id,
        binding.revision,
    )
    .map_err(|_| ProjectFolderFilesystemError::StaleProject)?;
    let current = project
        .project_archive()
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
    if current != binding.archive {
        return Err(ProjectFolderFilesystemError::StaleProject);
    }
    Ok(())
}

fn redacted_snapshot(project: &ProjectState) -> ProjectSnapshot {
    let mut response = snapshot(project);
    response.current_path = None;
    response
}

fn canceled_response(state: &AppState) -> Result<ProjectFolderFileResponse, String> {
    let project = lock_project(state)?;
    Ok(ProjectFolderFileResponse {
        canceled: true,
        project: redacted_snapshot(&project),
    })
}

fn error_string(error: ProjectFolderFilesystemError) -> String {
    error.code().to_owned()
}

fn load_project_folder(path: PathBuf) -> FsResult<LoadedProjectFolder> {
    let artifact = load_project_folder_artifact(&path, ProjectFolderLimits::default())?;
    validate_loaded_numeric_expression_bindings(&artifact.archive().document)
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
    let mut replacement = ProjectState::from_project_archive(artifact.into_archive(), path)
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
    // A directory must never enter the single-file Save path.
    replacement.current_path = None;
    Ok(LoadedProjectFolder { replacement })
}

fn load_project_folder_artifact(
    path: &Path,
    limits: ProjectFolderLimits,
) -> FsResult<ProjectFolderArtifactV1> {
    load_project_folder_artifact_with_hook(path, limits, || {})
}

fn load_project_folder_artifact_with_hook<F>(
    path: &Path,
    limits: ProjectFolderLimits,
    after_handles_open: F,
) -> FsResult<ProjectFolderArtifactV1>
where
    F: FnOnce(),
{
    let root = PinnedDirectory::open_selected(path)?;
    load_project_folder_artifact_from_pinned_with_hook(&root, limits, after_handles_open)
}

fn load_project_folder_artifact_from_pinned(
    root: &PinnedDirectory,
    limits: ProjectFolderLimits,
) -> FsResult<ProjectFolderArtifactV1> {
    load_project_folder_artifact_from_pinned_with_hook(root, limits, || {})
}

fn load_project_folder_artifact_from_pinned_with_hook<F>(
    root: &PinnedDirectory,
    limits: ProjectFolderLimits,
    after_handles_open: F,
) -> FsResult<ProjectFolderArtifactV1>
where
    F: FnOnce(),
{
    let first_root = validate_root_names(root.list_names(MAX_ENUMERATED_ROOT_ENTRIES + 1)?)?;
    let preview = root.open_child_directory("preview")?;
    let first_preview =
        validate_preview_names(preview.list_names(MAX_ENUMERATED_PREVIEW_ENTRIES + 1)?)?;

    let manifest_limit = effective_manifest_limit(limits);
    let project_limit = effective_project_limit(limits);
    let history_limit = effective_history_limit(limits);
    let preview_limit = effective_preview_limit(limits);
    let mut manifest = root.open_child_file("manifest.json", manifest_limit)?;
    let mut project = root.open_child_file("project.json", project_limit)?;
    let mut history = if first_root.has_history {
        Some(root.open_child_file("editor-history.json", history_limit)?)
    } else {
        None
    };
    let mut preview_file = preview.open_child_file("crease-pattern.svg", preview_limit)?;

    let declared_total = manifest
        .declared_size()
        .checked_add(project.declared_size())
        .and_then(|size| size.checked_add(history.as_ref().map_or(0, PinnedFile::declared_size)))
        .and_then(|size| size.checked_add(preview_file.declared_size()))
        .ok_or(ProjectFolderFilesystemError::TooLarge)?;
    if declared_total > effective_total_limit(limits) {
        return Err(ProjectFolderFilesystemError::TooLarge);
    }

    after_handles_open();

    let manifest_bytes =
        manifest.read_bounded_and_revalidate(root, "manifest.json", manifest_limit)?;
    let project_bytes = project.read_bounded_and_revalidate(root, "project.json", project_limit)?;
    let history_bytes = match &mut history {
        Some(history) => {
            Some(history.read_bounded_and_revalidate(root, "editor-history.json", history_limit)?)
        }
        None => None,
    };
    let preview_bytes =
        preview_file.read_bounded_and_revalidate(&preview, "crease-pattern.svg", preview_limit)?;

    let actual_total = (manifest_bytes.len() as u64)
        .checked_add(project_bytes.len() as u64)
        .and_then(|size| {
            size.checked_add(history_bytes.as_ref().map_or(0, |bytes| bytes.len() as u64))
        })
        .and_then(|size| size.checked_add(preview_bytes.len() as u64))
        .ok_or(ProjectFolderFilesystemError::TooLarge)?;
    if actual_total > effective_total_limit(limits) {
        return Err(ProjectFolderFilesystemError::TooLarge);
    }

    let second_root = validate_root_names(root.list_names(MAX_ENUMERATED_ROOT_ENTRIES + 1)?)?;
    let second_preview =
        validate_preview_names(preview.list_names(MAX_ENUMERATED_PREVIEW_ENTRIES + 1)?)?;
    if first_root != second_root || first_preview != second_preview {
        return Err(ProjectFolderFilesystemError::ChangedDuringRead);
    }
    root.revalidate_selected_path()?;
    root.revalidate_child_directory("preview", &preview)?;

    let mut entries = Vec::with_capacity(if first_root.has_history { 4 } else { 3 });
    entries.push(ProjectFolderEntryV1::new(
        PROJECT_FOLDER_MANIFEST_PATH,
        manifest_bytes,
    ));
    entries.push(ProjectFolderEntryV1::new(
        PROJECT_FOLDER_PROJECT_PATH,
        project_bytes,
    ));
    if let Some(history_bytes) = history_bytes {
        entries.push(ProjectFolderEntryV1::new(
            PROJECT_FOLDER_EDITOR_HISTORY_PATH,
            history_bytes,
        ));
    }
    entries.push(ProjectFolderEntryV1::new(
        PROJECT_FOLDER_PREVIEW_PATH,
        preview_bytes,
    ));
    read_project_folder_v1_with_limits(&entries, limits)
        .map_err(|_| ProjectFolderFilesystemError::InvalidTree)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RootLayout {
    names: Vec<String>,
    has_history: bool,
}

fn validate_root_names(names: Vec<OsString>) -> FsResult<RootLayout> {
    let names = validate_names(names, MAX_ENUMERATED_ROOT_ENTRIES)?;
    let required = ["manifest.json", "project.json", "preview"];
    if required
        .iter()
        .any(|required| !names.iter().any(|name| name == required))
    {
        return Err(ProjectFolderFilesystemError::InvalidTree);
    }
    let has_history = names.iter().any(|name| name == "editor-history.json");
    let expected_count = if has_history { 4 } else { 3 };
    if names.len() != expected_count
        || names.iter().any(|name| {
            !matches!(
                name.as_str(),
                "manifest.json" | "project.json" | "editor-history.json" | "preview"
            )
        })
    {
        return Err(ProjectFolderFilesystemError::InvalidTree);
    }
    Ok(RootLayout { names, has_history })
}

fn validate_preview_names(names: Vec<OsString>) -> FsResult<Vec<String>> {
    let names = validate_names(names, MAX_ENUMERATED_PREVIEW_ENTRIES)?;
    if names != ["crease-pattern.svg"] {
        return Err(ProjectFolderFilesystemError::InvalidTree);
    }
    Ok(names)
}

fn validate_names(names: Vec<OsString>, maximum: usize) -> FsResult<Vec<String>> {
    if names.len() > maximum {
        return Err(ProjectFolderFilesystemError::InvalidTree);
    }
    let mut exact = HashSet::with_capacity(names.len());
    let mut folded = HashMap::with_capacity(names.len());
    let mut canonical = Vec::with_capacity(names.len());
    for name in names {
        let name = name
            .into_string()
            .map_err(|_| ProjectFolderFilesystemError::InvalidTree)?;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.bytes().any(|byte| matches!(byte, b'/' | b'\\'))
        {
            return Err(ProjectFolderFilesystemError::InvalidTree);
        }
        if !exact.insert(name.clone()) {
            return Err(ProjectFolderFilesystemError::InvalidTree);
        }
        let folded_name = name.to_ascii_lowercase();
        if folded.insert(folded_name, name.clone()).is_some() {
            return Err(ProjectFolderFilesystemError::InvalidTree);
        }
        canonical.push(name);
    }
    canonical.sort_unstable();
    Ok(canonical)
}

struct PreparedProjectFolder {
    parent: PinnedDirectory,
    staging: PinnedDirectory,
    staging_name: String,
    target_name: String,
    expected_artifact: ProjectFolderArtifactV1,
    committed: bool,
}

impl PreparedProjectFolder {
    fn publish(&mut self) -> FsResult<()> {
        self.parent.revalidate_selected_path()?;
        self.parent
            .revalidate_child_directory(&self.staging_name, &self.staging)?;
        if self.parent.child_exists(&self.target_name)? {
            return Err(ProjectFolderFilesystemError::TargetExists);
        }
        // Preparing can finish well before the project lock is reacquired.
        // Strictly re-open the complete tree here so a late extra entry or
        // altered known payload is never published based on identity alone.
        let verified = load_project_folder_artifact_from_pinned(
            &self.staging,
            ProjectFolderLimits::default(),
        )?;
        if verified != self.expected_artifact {
            return Err(ProjectFolderFilesystemError::ChangedDuringRead);
        }
        self.parent
            .revalidate_child_directory(&self.staging_name, &self.staging)?;
        self.parent.sync_directory()?;
        self.parent.publish_child_directory_no_replace(
            &self.staging_name,
            &self.staging,
            &self.target_name,
        )?;
        self.committed = true;
        // Once published, a durability retry must not be reported as an
        // ordinary failure that could encourage a duplicate target.
        let _ = self.parent.sync_directory();
        Ok(())
    }
}

impl Drop for PreparedProjectFolder {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        cleanup_owned_staging(&self.parent, &self.staging_name, &self.staging);
    }
}

fn prepare_new_project_folder(
    parent_path: &Path,
    target_name: &str,
    artifact: ProjectFolderArtifactV1,
) -> FsResult<PreparedProjectFolder> {
    validate_native_child_name(target_name)?;
    let parent = PinnedDirectory::open_selected(parent_path)?;
    if parent.child_exists(target_name)? {
        return Err(ProjectFolderFilesystemError::TargetExists);
    }
    let (staging_name, staging) = create_staging_directory(&parent)?;
    let result = populate_and_verify_staging(&staging, &artifact);
    if let Err(error) = result {
        cleanup_owned_staging(&parent, &staging_name, &staging);
        return Err(error);
    }
    Ok(PreparedProjectFolder {
        parent,
        staging,
        staging_name,
        target_name: target_name.to_owned(),
        expected_artifact: artifact,
        committed: false,
    })
}

fn create_staging_directory(parent: &PinnedDirectory) -> FsResult<(String, PinnedDirectory)> {
    for _ in 0..STAGING_ATTEMPTS {
        let id = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!(".origami2-folder-stage-{}-{id}", std::process::id());
        match parent.create_child_directory(&name, true) {
            Ok(directory) => return Ok((name, directory)),
            Err(ProjectFolderFilesystemError::TargetExists) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(ProjectFolderFilesystemError::WriteFailed)
}

fn populate_and_verify_staging(
    staging: &PinnedDirectory,
    artifact: &ProjectFolderArtifactV1,
) -> FsResult<()> {
    if artifact.entries().len() > MAX_PROJECT_FOLDER_ENTRY_COUNT {
        return Err(ProjectFolderFilesystemError::InvalidTree);
    }
    let manifest = artifact_entry(artifact, PROJECT_FOLDER_MANIFEST_PATH)?;
    let project = artifact_entry(artifact, PROJECT_FOLDER_PROJECT_PATH)?;
    let history = artifact
        .entries()
        .iter()
        .find(|entry| entry.path == PROJECT_FOLDER_EDITOR_HISTORY_PATH);
    let preview_bytes = artifact_entry(artifact, PROJECT_FOLDER_PREVIEW_PATH)?;
    if artifact.entries().len() != if history.is_some() { 4 } else { 3 } {
        return Err(ProjectFolderFilesystemError::InvalidTree);
    }

    staging.write_child_file("manifest.json", manifest)?;
    staging.write_child_file("project.json", project)?;
    if let Some(history) = history {
        staging.write_child_file("editor-history.json", &history.bytes)?;
    }
    let preview = staging.create_child_directory("preview", false)?;
    preview.write_child_file("crease-pattern.svg", preview_bytes)?;
    preview.sync_directory()?;
    staging.sync_directory()?;

    let reopened =
        load_project_folder_artifact_from_pinned(staging, ProjectFolderLimits::default())?;
    if reopened != *artifact {
        return Err(ProjectFolderFilesystemError::WriteFailed);
    }
    Ok(())
}

fn artifact_entry<'a>(artifact: &'a ProjectFolderArtifactV1, path: &str) -> FsResult<&'a [u8]> {
    artifact
        .entries()
        .iter()
        .find(|entry| entry.path == path)
        .map(|entry| entry.bytes.as_slice())
        .ok_or(ProjectFolderFilesystemError::InvalidTree)
}

fn cleanup_owned_staging(parent: &PinnedDirectory, staging_name: &str, staging: &PinnedDirectory) {
    // Never clean by pathname unless it still resolves to the exact staging
    // directory created by this operation. Cleanup is fixed-entry-only and
    // intentionally leaves any unknown injected content in place.
    if parent
        .revalidate_child_directory(staging_name, staging)
        .is_err()
    {
        return;
    }
    let _ = staging.remove_child_file("manifest.json");
    let _ = staging.remove_child_file("project.json");
    let _ = staging.remove_child_file("editor-history.json");
    if let Ok(preview) = staging.open_child_directory("preview") {
        let _ = preview.remove_child_file("crease-pattern.svg");
    }
    let _ = staging.remove_child_directory("preview");
    if parent
        .revalidate_child_directory(staging_name, staging)
        .is_ok()
    {
        let _ = parent.remove_child_directory_if_same(staging_name, staging);
    }
}

fn target_folder_name(project_name: &str, project_id: ProjectId) -> String {
    let mut base = String::with_capacity(MAX_TARGET_BASE_BYTES);
    let mut previous_separator = false;
    for byte in project_name.trim().bytes() {
        if base.len() == MAX_TARGET_BASE_BYTES {
            break;
        }
        let character = if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            previous_separator = false;
            byte as char
        } else if previous_separator {
            continue;
        } else {
            previous_separator = true;
            '-'
        };
        base.push(character);
    }
    let base = base.trim_matches(['-', '_']);
    let base = if base.is_empty() {
        DEFAULT_TARGET_BASE
    } else {
        base
    };
    format!(
        "{base}-{}{TARGET_SUFFIX}",
        project_id_filename_suffix(project_id)
    )
}

fn project_id_filename_suffix(project_id: ProjectId) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = project_id.canonical_bytes();
    let mut suffix = String::with_capacity(PROJECT_ID_SUFFIX_HEX_CHARS);
    for byte in bytes.iter().take(PROJECT_ID_SUFFIX_BYTES) {
        suffix.push(HEX[(byte >> 4) as usize] as char);
        suffix.push(HEX[(byte & 0x0f) as usize] as char);
    }
    suffix
}

fn validate_native_child_name(name: &str) -> FsResult<()> {
    if name.is_empty()
        || name.len()
            > MAX_TARGET_BASE_BYTES + 1 + PROJECT_ID_SUFFIX_HEX_CHARS + TARGET_SUFFIX.len()
        || !name.ends_with(TARGET_SUFFIX)
        || name.bytes().any(|byte| matches!(byte, b'/' | b'\\' | b':'))
        || name
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')))
    {
        return Err(ProjectFolderFilesystemError::InvalidRequest);
    }
    Ok(())
}

const fn effective_entry_limit(limits: ProjectFolderLimits) -> u64 {
    let hard = ori_formats::MAX_PROJECT_JSON_BYTES as u64;
    if limits.max_entry_bytes < hard {
        limits.max_entry_bytes
    } else {
        hard
    }
}

const fn effective_manifest_limit(limits: ProjectFolderLimits) -> u64 {
    let requested = if limits.max_manifest_bytes < MAX_PROJECT_FOLDER_MANIFEST_BYTES {
        limits.max_manifest_bytes
    } else {
        MAX_PROJECT_FOLDER_MANIFEST_BYTES
    };
    if requested < effective_entry_limit(limits) {
        requested
    } else {
        effective_entry_limit(limits)
    }
}

const fn effective_project_limit(limits: ProjectFolderLimits) -> u64 {
    let hard = ori_formats::MAX_PROJECT_JSON_BYTES as u64;
    let requested = if limits.max_project_bytes < hard {
        limits.max_project_bytes
    } else {
        hard
    };
    if requested < effective_entry_limit(limits) {
        requested
    } else {
        effective_entry_limit(limits)
    }
}

const fn effective_history_limit(limits: ProjectFolderLimits) -> u64 {
    let hard = ori_formats::MAX_EDITOR_HISTORY_JSON_BYTES;
    let requested = if limits.max_editor_history_bytes < hard {
        limits.max_editor_history_bytes
    } else {
        hard
    };
    if requested < effective_entry_limit(limits) {
        requested
    } else {
        effective_entry_limit(limits)
    }
}

const fn effective_preview_limit(limits: ProjectFolderLimits) -> u64 {
    let requested = if limits.max_preview_bytes < MAX_PROJECT_FOLDER_PREVIEW_BYTES {
        limits.max_preview_bytes
    } else {
        MAX_PROJECT_FOLDER_PREVIEW_BYTES
    };
    if requested < effective_entry_limit(limits) {
        requested
    } else {
        effective_entry_limit(limits)
    }
}

const fn effective_total_limit(limits: ProjectFolderLimits) -> u64 {
    if limits.max_total_bytes < MAX_PROJECT_FOLDER_TOTAL_BYTES {
        limits.max_total_bytes
    } else {
        MAX_PROJECT_FOLDER_TOTAL_BYTES
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            for _ in 0..128 {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "origami2-project-folder-{label}-{}-{id}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create test directory: {error}"),
                }
            }
            panic!("allocate test directory");
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_archive(with_history: bool) -> Ori2ProjectArchive {
        let first = VertexId::new();
        let second = VertexId::new();
        let document = ori_formats::ProjectDocument::new(
            "Filesystem safety",
            CreasePattern {
                vertices: vec![
                    Vertex {
                        id: first,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: second,
                        position: Point2::new(100.0, 50.0),
                    },
                ],
                edges: vec![Edge {
                    id: EdgeId::new(),
                    start: first,
                    end: second,
                    kind: EdgeKind::Mountain,
                }],
            },
        );
        let editor_history = with_history.then(|| {
            serde_json::from_value(serde_json::json!({
                "schema_version": ori_core::EDITOR_HISTORY_SCHEMA_VERSION_V1,
                "project_id": document.project_id,
                "history_entry_limit": 7,
                "undo_stack": [],
                "redo_stack": [],
            }))
            .expect("history fixture")
        });
        Ori2ProjectArchive {
            document,
            editor_history,
        }
    }

    fn write_fixture(root: &Path, artifact: &ProjectFolderArtifactV1) {
        fs::create_dir(root).expect("create project root");
        fs::create_dir(root.join("preview")).expect("create preview directory");
        for entry in artifact.entries() {
            fs::write(root.join(&entry.path), &entry.bytes).expect("write fixture entry");
        }
    }

    #[test]
    fn canonical_folder_round_trips_with_and_without_history() {
        for with_history in [false, true] {
            let directory = TestDirectory::new("round-trip");
            let root = directory.0.join("project");
            let archive = sample_archive(with_history);
            let artifact = write_project_folder_v1(&archive).expect("artifact");
            write_fixture(&root, &artifact);

            let reopened =
                load_project_folder_artifact(&root, ProjectFolderLimits::default()).expect("read");
            assert_eq!(reopened, artifact);
            assert_eq!(reopened.archive(), &archive);
        }
    }

    #[test]
    fn directory_enumeration_restarts_from_the_beginning() {
        let directory = TestDirectory::new("repeat-enumeration");
        fs::write(directory.0.join("first"), b"1").expect("first");
        fs::write(directory.0.join("second"), b"2").expect("second");
        let pinned = PinnedDirectory::open_selected(&directory.0).expect("open");
        let mut first = pinned.list_names(2).expect("first enumeration");
        let mut second = pinned.list_names(2).expect("second enumeration");
        first.sort();
        second.sort();
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
    }

    #[test]
    fn owned_io_permit_stays_busy_while_a_worker_owns_it() {
        let state = ProjectFolderIoState::default();
        let permit = state.try_acquire().expect("first permit");
        let worker_state = state.clone();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _permit = permit;
            release_rx.recv().expect("release");
        });

        assert_eq!(
            worker_state.try_acquire().err(),
            Some(ERROR_BUSY.to_owned())
        );
        release_tx.send(()).expect("signal");
        worker.join().expect("worker");
        drop(worker_state.try_acquire().expect("permit after worker"));
    }

    #[test]
    fn project_binding_rejects_instance_project_revision_and_archive_aba() {
        let project = ProjectState::new(CreasePattern::empty());
        let binding = capture_binding(&project).expect("binding");
        ensure_binding_current(&project, &binding).expect("current binding");

        let mut wrong_instance = binding.clone();
        wrong_instance.instance_id = ProjectId::new();
        let mut wrong_project = binding.clone();
        wrong_project.project_id = ProjectId::new();
        let mut wrong_revision = binding.clone();
        wrong_revision.revision = wrong_revision.revision.saturating_add(1);
        let mut history_or_document_aba = binding.clone();
        history_or_document_aba
            .archive
            .document
            .name
            .push_str(" changed");

        for stale in [
            wrong_instance,
            wrong_project,
            wrong_revision,
            history_or_document_aba,
        ] {
            assert_eq!(
                ensure_binding_current(&project, &stale),
                Err(ProjectFolderFilesystemError::StaleProject)
            );
        }
    }

    #[test]
    fn dialog_locale_is_strict_and_titles_are_localized() {
        assert_eq!(
            DialogLocale::parse("ja").expect("ja").open_title(),
            OPEN_TITLE_JA
        );
        assert_eq!(
            DialogLocale::parse("en").expect("en").save_parent_title(),
            SAVE_PARENT_TITLE_EN
        );
        for invalid in ["", "JA", "ja-JP", "en-US", "fr"] {
            assert_eq!(
                DialogLocale::parse(invalid).err(),
                Some(ERROR_INVALID_LOCALE.to_owned())
            );
        }
    }

    #[test]
    fn extra_case_colliding_and_nested_entries_are_rejected() {
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        for (label, relative, directory_entry) in [
            ("extra", "extra.json", false),
            ("case", "Manifest.json", false),
            ("nested", "unexpected", true),
            ("preview-extra", "preview/extra.svg", false),
        ] {
            let directory = TestDirectory::new(label);
            let root = directory.0.join("project");
            write_fixture(&root, &artifact);
            let path = root.join(relative);
            if directory_entry {
                fs::create_dir(path).expect("create extra directory");
            } else {
                fs::write(path, b"extra").expect("write extra entry");
            }
            assert!(matches!(
                load_project_folder_artifact(&root, ProjectFolderLimits::default()),
                Err(ProjectFolderFilesystemError::InvalidTree)
            ));
        }
    }

    #[test]
    fn every_payload_and_total_limit_accept_exact_and_reject_one_short() {
        let directory = TestDirectory::new("limits");
        let root = directory.0.join("project");
        let artifact = write_project_folder_v1(&sample_archive(true)).expect("artifact");
        write_fixture(&root, &artifact);
        let size = |path: &str| {
            artifact
                .entries()
                .iter()
                .find(|entry| entry.path == path)
                .expect("entry")
                .bytes
                .len() as u64
        };
        let total = artifact
            .entries()
            .iter()
            .map(|entry| entry.bytes.len() as u64)
            .sum::<u64>();
        let boundaries = [
            (
                ProjectFolderLimits {
                    max_manifest_bytes: size(PROJECT_FOLDER_MANIFEST_PATH),
                    ..ProjectFolderLimits::default()
                },
                ProjectFolderLimits {
                    max_manifest_bytes: size(PROJECT_FOLDER_MANIFEST_PATH) - 1,
                    ..ProjectFolderLimits::default()
                },
            ),
            (
                ProjectFolderLimits {
                    max_project_bytes: size(PROJECT_FOLDER_PROJECT_PATH),
                    ..ProjectFolderLimits::default()
                },
                ProjectFolderLimits {
                    max_project_bytes: size(PROJECT_FOLDER_PROJECT_PATH) - 1,
                    ..ProjectFolderLimits::default()
                },
            ),
            (
                ProjectFolderLimits {
                    max_editor_history_bytes: size(PROJECT_FOLDER_EDITOR_HISTORY_PATH),
                    ..ProjectFolderLimits::default()
                },
                ProjectFolderLimits {
                    max_editor_history_bytes: size(PROJECT_FOLDER_EDITOR_HISTORY_PATH) - 1,
                    ..ProjectFolderLimits::default()
                },
            ),
            (
                ProjectFolderLimits {
                    max_preview_bytes: size(PROJECT_FOLDER_PREVIEW_PATH),
                    ..ProjectFolderLimits::default()
                },
                ProjectFolderLimits {
                    max_preview_bytes: size(PROJECT_FOLDER_PREVIEW_PATH) - 1,
                    ..ProjectFolderLimits::default()
                },
            ),
            (
                ProjectFolderLimits {
                    max_total_bytes: total,
                    ..ProjectFolderLimits::default()
                },
                ProjectFolderLimits {
                    max_total_bytes: total - 1,
                    ..ProjectFolderLimits::default()
                },
            ),
        ];
        for (exact, one_short) in boundaries {
            load_project_folder_artifact(&root, exact).expect("exact limit");
            assert!(matches!(
                load_project_folder_artifact(&root, one_short),
                Err(ProjectFolderFilesystemError::TooLarge)
            ));
        }
    }

    #[test]
    fn hard_linked_payload_is_rejected() {
        let directory = TestDirectory::new("hard-link");
        let root = directory.0.join("project");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        write_fixture(&root, &artifact);
        let project = root.join("project.json");
        let outside_link = directory.0.join("second-name.json");
        fs::hard_link(&project, outside_link).expect("create hard link");

        assert!(matches!(
            load_project_folder_artifact(&root, ProjectFolderLimits::default()),
            Err(ProjectFolderFilesystemError::LinkOrSpecialEntry)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_and_fifo_payloads_are_rejected_without_following() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt, os::unix::fs::symlink};

        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");

        let link_directory = TestDirectory::new("symlink");
        let link_root = link_directory.0.join("project");
        write_fixture(&link_root, &artifact);
        let real_project = link_directory.0.join("real-project.json");
        fs::rename(link_root.join("project.json"), &real_project).expect("move project");
        symlink(&real_project, link_root.join("project.json")).expect("create symlink");
        assert!(matches!(
            load_project_folder_artifact(&link_root, ProjectFolderLimits::default()),
            Err(ProjectFolderFilesystemError::LinkOrSpecialEntry)
        ));

        let fifo_directory = TestDirectory::new("fifo");
        let fifo_root = fifo_directory.0.join("project");
        write_fixture(&fifo_root, &artifact);
        fs::remove_file(fifo_root.join("project.json")).expect("remove project");
        let fifo =
            CString::new(fifo_root.join("project.json").as_os_str().as_bytes()).expect("FIFO path");
        let created = unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) };
        assert_eq!(created, 0, "create FIFO");
        assert!(matches!(
            load_project_folder_artifact(&fifo_root, ProjectFolderLimits::default()),
            Err(ProjectFolderFilesystemError::LinkOrSpecialEntry)
        ));
    }

    #[test]
    fn opened_payload_replacement_is_blocked_or_detected() {
        let directory = TestDirectory::new("file-race");
        let root = directory.0.join("project");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        write_fixture(&root, &artifact);
        let project = root.join("project.json");
        let retired = root.join("retired.json");
        let mut replacement_succeeded = false;
        let result =
            load_project_folder_artifact_with_hook(&root, ProjectFolderLimits::default(), || {
                if fs::rename(&project, &retired).is_ok() {
                    replacement_succeeded = true;
                    fs::write(&project, b"{}").expect("write replacement");
                }
            });
        if replacement_succeeded {
            assert!(matches!(
                result,
                Err(ProjectFolderFilesystemError::ChangedDuringRead)
                    | Err(ProjectFolderFilesystemError::InvalidTree)
            ));
        } else {
            result.expect("open handle blocked replacement");
        }
    }

    #[test]
    fn post_enumeration_extra_entry_is_blocked_or_detected() {
        let directory = TestDirectory::new("enumeration-race");
        let root = directory.0.join("project");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        write_fixture(&root, &artifact);
        let extra = root.join("late-extra.json");
        let mut insertion_succeeded = false;
        let result =
            load_project_folder_artifact_with_hook(&root, ProjectFolderLimits::default(), || {
                if fs::write(&extra, b"late").is_ok() {
                    insertion_succeeded = true;
                }
            });
        if insertion_succeeded {
            assert!(matches!(
                result,
                Err(ProjectFolderFilesystemError::InvalidTree)
                    | Err(ProjectFolderFilesystemError::ChangedDuringRead)
            ));
        } else {
            result.expect("directory handle blocked insertion");
        }
    }

    #[test]
    fn create_new_publish_round_trips_and_never_replaces_existing_target() {
        let directory = TestDirectory::new("publish");
        let artifact = write_project_folder_v1(&sample_archive(true)).expect("artifact");
        let target_name = "safe-project.origami2-folder";
        let mut prepared = prepare_new_project_folder(&directory.0, target_name, artifact.clone())
            .expect("prepare");
        assert!(!directory.0.join(target_name).exists());
        prepared.publish().expect("publish");
        // Windows publication owns a DELETE-capable source handle until the
        // prepared transaction is dropped.
        drop(prepared);
        let reopened = load_project_folder_artifact(
            &directory.0.join(target_name),
            ProjectFolderLimits::default(),
        )
        .expect("reopen");
        assert_eq!(reopened, artifact);

        assert!(matches!(
            prepare_new_project_folder(&directory.0, target_name, artifact),
            Err(ProjectFolderFilesystemError::TargetExists)
        ));
    }

    #[test]
    fn publish_race_preserves_existing_target_and_drop_cleans_owned_staging() {
        let directory = TestDirectory::new("publish-race");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        let target_name = "raced.origami2-folder";
        let mut prepared =
            prepare_new_project_folder(&directory.0, target_name, artifact).expect("prepare");
        let staging_name = prepared.staging_name.clone();
        let target = directory.0.join(target_name);
        fs::create_dir(&target).expect("create raced target");
        fs::write(target.join("sentinel"), b"keep").expect("write sentinel");

        assert!(matches!(
            prepared.publish(),
            Err(ProjectFolderFilesystemError::TargetExists)
        ));
        drop(prepared);
        assert_eq!(
            fs::read(target.join("sentinel")).expect("sentinel"),
            b"keep"
        );
        assert!(!directory.0.join(staging_name).exists());
    }

    #[test]
    fn late_unknown_staging_entry_is_not_published_or_recursively_deleted() {
        let directory = TestDirectory::new("late-extra-staging");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        let target_name = "late-extra.origami2-folder";
        let mut prepared =
            prepare_new_project_folder(&directory.0, target_name, artifact).expect("prepare");
        let staging = directory.0.join(&prepared.staging_name);
        fs::write(staging.join("unknown-sentinel"), b"keep").expect("inject unknown entry");

        assert!(matches!(
            prepared.publish(),
            Err(ProjectFolderFilesystemError::InvalidTree)
                | Err(ProjectFolderFilesystemError::ChangedDuringRead)
        ));
        drop(prepared);
        assert!(!directory.0.join(target_name).exists());
        assert_eq!(
            fs::read(staging.join("unknown-sentinel")).expect("unknown entry retained"),
            b"keep"
        );
    }

    #[test]
    fn late_known_payload_change_is_not_published_and_owned_stage_is_cleaned() {
        let directory = TestDirectory::new("late-payload-staging");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        let target_name = "late-payload.origami2-folder";
        let mut prepared =
            prepare_new_project_folder(&directory.0, target_name, artifact).expect("prepare");
        let staging = directory.0.join(&prepared.staging_name);
        fs::write(staging.join("project.json"), b"{}").expect("alter known payload");

        assert!(matches!(
            prepared.publish(),
            Err(ProjectFolderFilesystemError::InvalidTree)
                | Err(ProjectFolderFilesystemError::ChangedDuringRead)
        ));
        drop(prepared);
        assert!(!directory.0.join(target_name).exists());
        assert!(!staging.exists());
    }

    #[test]
    fn uncommitted_staging_drop_leaves_no_target_or_recursive_unknown_deletion() {
        let directory = TestDirectory::new("cleanup");
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        let target_name = "never-published.origami2-folder";
        let prepared =
            prepare_new_project_folder(&directory.0, target_name, artifact).expect("prepare");
        let staging = directory.0.join(&prepared.staging_name);
        fs::write(staging.join("unknown-sentinel"), b"keep").expect("inject unknown entry");
        drop(prepared);

        assert!(!directory.0.join(target_name).exists());
        assert!(staging.join("unknown-sentinel").exists());
    }

    #[test]
    fn target_name_is_native_bounded_and_contains_no_path_components() {
        let project_id = ProjectId::new();
        let suffix = project_id_filename_suffix(project_id);
        assert_eq!(
            target_folder_name("  Crane / 鶴 : design  ", project_id),
            format!("Crane-design-{suffix}.origami2-folder")
        );
        assert_eq!(
            target_folder_name("???", project_id),
            format!("origami2-project-{suffix}.origami2-folder")
        );
        let long = target_folder_name(&"a".repeat(200), project_id);
        assert_eq!(
            long.len(),
            MAX_TARGET_BASE_BYTES + 1 + PROJECT_ID_SUFFIX_HEX_CHARS + TARGET_SUFFIX.len()
        );
        assert_eq!(suffix.len(), PROJECT_ID_SUFFIX_HEX_CHARS);
        assert!(suffix.bytes().all(|byte| byte.is_ascii_hexdigit()));
        for name in [
            "../escape.origami2-folder",
            "C:escape.origami2-folder",
            "nested/name.origami2-folder",
            "wrong.txt",
        ] {
            assert_eq!(
                validate_native_child_name(name),
                Err(ProjectFolderFilesystemError::InvalidRequest)
            );
        }
    }

    #[test]
    fn non_ascii_project_names_are_distinguished_by_canonical_project_id() {
        let first_id = ProjectId::new();
        let second_id = ProjectId::new();
        let first = target_folder_name("鶴", first_id);
        let second = target_folder_name("花", second_id);
        assert_ne!(first, second);
        assert_eq!(
            first,
            format!(
                "origami2-project-{}.origami2-folder",
                project_id_filename_suffix(first_id)
            )
        );
        assert_eq!(
            second,
            format!(
                "origami2-project-{}.origami2-folder",
                project_id_filename_suffix(second_id)
            )
        );
    }

    #[test]
    fn public_error_codes_are_fixed_and_contain_no_filesystem_path() {
        for error in [
            ProjectFolderFilesystemError::InvalidRequest,
            ProjectFolderFilesystemError::OpenFailed,
            ProjectFolderFilesystemError::InvalidTree,
            ProjectFolderFilesystemError::TooLarge,
            ProjectFolderFilesystemError::LinkOrSpecialEntry,
            ProjectFolderFilesystemError::ChangedDuringRead,
            ProjectFolderFilesystemError::ReadFailed,
            ProjectFolderFilesystemError::WriteFailed,
            ProjectFolderFilesystemError::TargetExists,
            ProjectFolderFilesystemError::StaleProject,
        ] {
            let code = error.code();
            assert!(code.starts_with("project_folder_"));
            assert!(!code.bytes().any(|byte| matches!(byte, b'/' | b'\\' | b':')));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_directory_reparse_points_are_rejected_when_supported() {
        use std::os::windows::fs::symlink_dir;

        let directory = TestDirectory::new("windows-reparse");
        let real = directory.0.join("real");
        let link = directory.0.join("link");
        fs::create_dir(&real).expect("real directory");
        if symlink_dir(&real, &link).is_err() {
            // Creating symlinks requires Developer Mode or an elevated token
            // on some Windows installations. CI exercises this whenever the
            // host policy permits it.
            return;
        }
        assert!(matches!(
            PinnedDirectory::open_selected(&link),
            Err(ProjectFolderFilesystemError::LinkOrSpecialEntry)
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_root_and_preview_names_are_pinned_during_admission() {
        let artifact = write_project_folder_v1(&sample_archive(false)).expect("artifact");
        for component in ["root", "preview"] {
            let directory = TestDirectory::new("windows-directory-race");
            let root = directory.0.join("project");
            write_fixture(&root, &artifact);
            let selected = if component == "root" {
                root.clone()
            } else {
                root.join("preview")
            };
            let moved = if component == "root" {
                directory.0.join("moved-project")
            } else {
                root.join("moved-preview")
            };
            let mut rename_succeeded = false;
            let result = load_project_folder_artifact_with_hook(
                &root,
                ProjectFolderLimits::default(),
                || {
                    rename_succeeded = std::thread::spawn(move || {
                        if fs::rename(&selected, &moved).is_err() {
                            return false;
                        }
                        fs::rename(&moved, &selected).expect("restore renamed directory");
                        true
                    })
                    .join()
                    .expect("rename contender");
                },
            );

            assert!(
                !rename_succeeded,
                "{component} final component must be pinned against ABA replacement"
            );
            result.expect("blocked rename preserves canonical admission");
        }
    }
}
