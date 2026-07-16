use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use atomic_write_file::AtomicWriteFile;
use ori_core::{
    BoundaryEdgeRef, Command, EditorSettings, EditorState, PaperValidationIssue, ValidationIssue,
    create_rectangular_sheet, validate_paper,
};
use ori_domain::{CreasePattern, EdgeId, EdgeKind, Paper, Point2, ProjectId, VertexId};
use ori_formats::{
    CURRENT_FORMAT_VERSION, Ori2Limits, ProjectDocument, read_project_ori2_with_limits,
    write_project_ori2,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[cfg(target_os = "macos")]
use tauri::menu::{
    AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};

const UNTITLED_PROJECT_NAME: &str = "Untitled";
const DEFAULT_SHEET_SIZE_MM: f64 = 400.0;
#[cfg(target_os = "macos")]
const MACOS_QUIT_MENU_ID: &str = "origami2_quit";

struct AppState(Mutex<ProjectState>);

#[derive(Default)]
struct ExitGuard {
    allow_once: AtomicBool,
    dialog_open: AtomicBool,
}

struct ProjectState {
    project_id: ProjectId,
    name: String,
    current_path: Option<PathBuf>,
    paper: Paper,
    editor: EditorState,
    saved_revision: Option<u64>,
    saved_document: Option<ProjectDocument>,
}

impl ProjectState {
    #[cfg(test)]
    fn new(pattern: CreasePattern) -> Self {
        Self::new_with_paper(pattern, Paper::default())
    }

    fn new_with_paper(pattern: CreasePattern, paper: Paper) -> Self {
        let editor = EditorState::with_settings(
            pattern,
            EditorSettings::new().with_cutting_allowed(paper.cutting_allowed),
        );
        let mut project = Self {
            project_id: ProjectId::new(),
            name: UNTITLED_PROJECT_NAME.to_owned(),
            current_path: None,
            paper,
            editor,
            saved_revision: None,
            saved_document: None,
        };
        // A newly-created sheet is the clean baseline. It becomes dirty only
        // after the user makes an edit, even though it has no file path yet.
        project.saved_document = Some(project.document());
        project
    }

    fn from_document(document: ProjectDocument, current_path: PathBuf) -> Self {
        let saved_document = document.clone();
        let editor = EditorState::with_settings(
            document.crease_pattern,
            EditorSettings::new().with_cutting_allowed(document.paper.cutting_allowed),
        );
        Self {
            project_id: document.project_id,
            name: document.name,
            current_path: Some(current_path),
            paper: document.paper,
            saved_revision: Some(editor.revision()),
            saved_document: Some(saved_document),
            editor,
        }
    }

    fn document(&self) -> ProjectDocument {
        let mut paper = self.paper.clone();
        paper.cutting_allowed = self.editor.settings().cutting_allowed();
        ProjectDocument {
            format_version: CURRENT_FORMAT_VERSION,
            project_id: self.project_id,
            name: self.name.clone(),
            paper,
            crease_pattern: self.editor.pattern().clone(),
        }
    }

    fn is_dirty(&self) -> bool {
        let Some(saved) = &self.saved_document else {
            return true;
        };
        saved.format_version != CURRENT_FORMAT_VERSION
            || saved.project_id != self.project_id
            || saved.name != self.name
            || saved.paper.boundary_vertices != self.paper.boundary_vertices
            || saved.paper.thickness_mm != self.paper.thickness_mm
            || saved.paper.cutting_allowed != self.editor.cutting_allowed()
            || saved.paper.front != self.paper.front
            || saved.paper.back != self.paper.back
            || saved.crease_pattern != *self.editor.pattern()
    }
}

fn initial_project_state() -> ProjectState {
    let sheet = create_rectangular_sheet(DEFAULT_SHEET_SIZE_MM, DEFAULT_SHEET_SIZE_MM, false)
        .expect("the built-in default sheet dimensions must be valid");
    let (pattern, paper) = sheet.into_parts();
    ProjectState::new_with_paper(pattern, paper)
}

#[derive(Serialize)]
struct PatternResponse {
    vertex_count: usize,
    edge_count: usize,
}

#[derive(Debug, Serialize)]
struct ProjectSnapshot {
    project_id: ProjectId,
    name: String,
    current_path: Option<String>,
    revision: u64,
    saved_revision: Option<u64>,
    is_dirty: bool,
    crease_pattern: CreasePattern,
    can_undo: bool,
    can_redo: bool,
    cutting_allowed: bool,
}

#[derive(Debug, Serialize)]
struct ProjectFileResponse {
    canceled: bool,
    project: ProjectSnapshot,
}

#[derive(Debug, Serialize)]
struct ValidationSnapshot {
    project_id: ProjectId,
    revision: u64,
    is_valid: bool,
    issues: Vec<ValidationIssueSnapshot>,
}

#[derive(Debug, Serialize)]
struct ValidationIssueSnapshot {
    code: &'static str,
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
}

#[tauri::command]
fn generate_benchmark_pattern(edge_count: usize) -> PatternResponse {
    let pattern = ori_core::benchmark_pattern(edge_count.min(100_000));
    PatternResponse {
        vertex_count: pattern.vertices.len(),
        edge_count: pattern.edges.len(),
    }
}

#[tauri::command]
fn project_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project = lock_project(&state)?;
    Ok(snapshot(&project))
}

#[tauri::command]
fn validate_project(state: State<'_, AppState>) -> Result<ValidationSnapshot, String> {
    let project = lock_project(&state)?;
    Ok(validation_snapshot(&project))
}

#[tauri::command]
async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectFileResponse, String> {
    let (expected_project_id, expected_revision, initial_directory) = {
        let project = lock_project(&state)?;
        (
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
        .map_err(|error| format!("the selected location is not a local file: {error}"))?;
    let document = load_document_from_path(&path)?;

    let mut project = lock_project(&state)?;
    ensure_expected_project(&project, expected_project_id, expected_revision)?;
    *project = ProjectState::from_document(document, path);
    Ok(ProjectFileResponse {
        canceled: false,
        project: snapshot(&project),
    })
}

#[tauri::command]
async fn save_project(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectFileResponse, String> {
    {
        let mut project = lock_project(&state)?;
        if let Some(path) = project.current_path.clone() {
            return save_locked_project(&mut project, path);
        }
    }
    save_project_with_dialog(&app, &state)
}

#[tauri::command]
async fn save_project_as(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectFileResponse, String> {
    save_project_with_dialog(&app, &state)
}

#[tauri::command]
fn add_vertex(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    x: f64,
    y: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(x, y),
        },
    )
}

#[tauri::command]
fn move_vertex(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: VertexId,
    x: f64,
    y: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::MoveVertex {
            id,
            position: Point2::new(x, y),
        },
    )
}

#[tauri::command]
fn remove_vertex(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: VertexId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::RemoveVertex { id },
    )
}

#[tauri::command]
fn add_edge(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    start: VertexId,
    end: VertexId,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::AddEdge {
            id: EdgeId::new(),
            start,
            end,
            kind,
        },
    )
}

#[tauri::command]
fn remove_edge(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::RemoveEdge { id },
    )
}

#[tauri::command]
fn undo(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_project_identity(&project, expected_project_id)?;
    project
        .editor
        .undo(expected_revision)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&project))
}

#[tauri::command]
fn redo(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_project_identity(&project, expected_project_id)?;
    project
        .editor
        .redo(expected_revision)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&project))
}

#[tauri::command]
fn set_cutting_allowed(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    allowed: bool,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::SetCuttingAllowed { allowed },
    )
}

fn lock_project(state: &AppState) -> Result<MutexGuard<'_, ProjectState>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the project state lock is poisoned".to_owned())
}

fn execute_command(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    ensure_project_identity(project, expected_project_id)?;
    project
        .editor
        .execute(expected_revision, command)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(project))
}

fn ensure_project_identity(
    project: &ProjectState,
    expected_project_id: ProjectId,
) -> Result<(), String> {
    if project.project_id == expected_project_id {
        Ok(())
    } else {
        Err("the active project changed before the command was applied".to_owned())
    }
}

fn ensure_expected_project(
    project: &ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<(), String> {
    ensure_project_identity(project, expected_project_id)?;
    if project.editor.revision() == expected_revision {
        Ok(())
    } else {
        Err("the project changed while the file dialog was open".to_owned())
    }
}

fn snapshot(project: &ProjectState) -> ProjectSnapshot {
    ProjectSnapshot {
        project_id: project.project_id,
        name: project.name.clone(),
        current_path: project
            .current_path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        revision: project.editor.revision(),
        saved_revision: project.saved_revision,
        is_dirty: project.is_dirty(),
        crease_pattern: project.editor.pattern().clone(),
        can_undo: project.editor.can_undo(),
        can_redo: project.editor.can_redo(),
        cutting_allowed: project.editor.cutting_allowed(),
    }
}

fn canceled_file_response(state: &AppState) -> Result<ProjectFileResponse, String> {
    let project = lock_project(state)?;
    Ok(ProjectFileResponse {
        canceled: true,
        project: snapshot(&project),
    })
}

fn save_project_with_dialog(
    app: &AppHandle,
    state: &AppState,
) -> Result<ProjectFileResponse, String> {
    let (expected_project_id, expected_revision, initial_directory, suggested_name) = {
        let project = lock_project(state)?;
        (
            project.project_id,
            project.editor.revision(),
            project
                .current_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
            suggested_file_name(&project.name),
        )
    };

    let mut dialog = app
        .dialog()
        .file()
        .add_filter("ORIGAMI2 project", &["ori2"])
        .set_file_name(suggested_name)
        .set_title("Save ORIGAMI2 project");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }

    let Some(selected) = dialog.blocking_save_file() else {
        return canceled_file_response(state);
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|error| format!("the selected location is not a local file: {error}"))?;
    let path = ensure_ori2_extension(path);

    let mut project = lock_project(state)?;
    ensure_expected_project(&project, expected_project_id, expected_revision)?;
    save_locked_project(&mut project, path)
}

fn save_locked_project(
    project: &mut ProjectState,
    path: PathBuf,
) -> Result<ProjectFileResponse, String> {
    let document = project.document();
    persist_document(&path, &document)?;
    project.current_path = Some(path);
    project.saved_revision = Some(project.editor.revision());
    project.saved_document = Some(document);
    Ok(ProjectFileResponse {
        canceled: false,
        project: snapshot(project),
    })
}

fn load_document_from_path(path: &Path) -> Result<ProjectDocument, String> {
    let limits = Ori2Limits::default();
    let file =
        File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let declared_size = file
        .metadata()
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
        .len();
    if declared_size > limits.max_archive_size {
        return Err(format!(
            "{} is {declared_size} bytes; the .ori2 limit is {} bytes",
            path.display(),
            limits.max_archive_size
        ));
    }

    let capacity = usize::try_from(declared_size)
        .unwrap_or(0)
        .min(usize::try_from(limits.max_archive_size).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(capacity);
    let mut bounded_reader = file.take(limits.max_archive_size.saturating_add(1));
    bounded_reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if bytes.len() as u64 > limits.max_archive_size {
        return Err(format!(
            "{} grew beyond the .ori2 limit of {} bytes while it was read",
            path.display(),
            limits.max_archive_size
        ));
    }

    read_project_ori2_with_limits(&bytes, limits)
        .map_err(|error| format!("failed to validate {}: {error}", path.display()))
}

fn persist_document(path: &Path, document: &ProjectDocument) -> Result<(), String> {
    if path.file_name().is_none() {
        return Err(format!("{} is not a file path", path.display()));
    }

    let bytes = write_project_ori2(document)
        .map_err(|error| format!("failed to create .ori2 data: {error}"))?;
    let mut atomic_file = AtomicWriteFile::open(path).map_err(|error| {
        format!(
            "failed to prepare atomic save for {}: {error}",
            path.display()
        )
    })?;
    atomic_file.write_all(&bytes).map_err(|error| {
        format!(
            "failed to write staged project data for {}: {error}",
            path.display()
        )
    })?;
    atomic_file.sync_all().map_err(|error| {
        format!(
            "failed to synchronize staged project data for {}: {error}",
            path.display()
        )
    })?;

    verify_generated_ori2(document, &bytes)?;
    atomic_file
        .commit()
        .map_err(|error| format!("failed to commit {} atomically: {error}", path.display()))
}

fn verify_generated_ori2(document: &ProjectDocument, bytes: &[u8]) -> Result<(), String> {
    let verified = read_project_ori2_with_limits(bytes, Ori2Limits::default())
        .map_err(|error| format!("generated .ori2 data did not pass validation: {error}"))?;
    if verified != *document {
        return Err("generated .ori2 data did not round-trip exactly".to_owned());
    }
    Ok(())
}

fn ensure_ori2_extension(mut path: PathBuf) -> PathBuf {
    let already_ori2 = path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ori2"));
    if !already_ori2 {
        path.set_extension("ori2");
    }
    path
}

fn suggested_file_name(project_name: &str) -> String {
    let mut sanitized = String::new();
    for character in project_name.trim().chars().take(80) {
        if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }
    let sanitized = sanitized.trim_matches([' ', '.']);
    let base = if sanitized.is_empty() {
        UNTITLED_PROJECT_NAME
    } else {
        sanitized
    };
    format!("{base}.ori2")
}

fn validation_snapshot(project: &ProjectState) -> ValidationSnapshot {
    let crease_validation = project.editor.validation();
    let paper_validation = validate_paper(&project.paper, project.editor.pattern());
    let mut issues =
        Vec::with_capacity(crease_validation.issues().len() + paper_validation.issues.len());
    issues.extend(
        crease_validation
            .issues()
            .iter()
            .map(validation_issue_snapshot),
    );
    issues.extend(
        paper_validation
            .issues
            .iter()
            .map(|issue| paper_validation_issue_snapshot(issue, project)),
    );
    ValidationSnapshot {
        project_id: project.project_id,
        revision: crease_validation.revision(),
        is_valid: issues.is_empty(),
        issues,
    }
}

fn validation_issue_snapshot(issue: &ValidationIssue) -> ValidationIssueSnapshot {
    match issue {
        ValidationIssue::NonFiniteVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "non_finite_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        ValidationIssue::DuplicateVertex {
            first, duplicate, ..
        } => ValidationIssueSnapshot {
            code: "duplicate_vertex",
            vertices: vec![*first, *duplicate],
            edges: Vec::new(),
        },
        ValidationIssue::MissingEndpoint { edge, vertex, .. } => ValidationIssueSnapshot {
            code: "missing_endpoint",
            vertices: vec![*vertex],
            edges: vec![*edge],
        },
        ValidationIssue::ZeroLengthEdge { edge } => ValidationIssueSnapshot {
            code: "zero_length_edge",
            vertices: Vec::new(),
            edges: vec![*edge],
        },
        ValidationIssue::UnsplitIntersection {
            first_edge,
            second_edge,
            ..
        } => ValidationIssueSnapshot {
            code: "unsplit_intersection",
            vertices: Vec::new(),
            edges: vec![*first_edge, *second_edge],
        },
        ValidationIssue::IntersectionCalculationFailed {
            first_edge,
            second_edge,
            ..
        } => ValidationIssueSnapshot {
            code: "intersection_calculation_failed",
            vertices: Vec::new(),
            edges: vec![*first_edge, *second_edge],
        },
    }
}

fn paper_validation_issue_snapshot(
    issue: &PaperValidationIssue,
    project: &ProjectState,
) -> ValidationIssueSnapshot {
    let pattern = project.editor.pattern();
    match issue {
        PaperValidationIssue::NonFiniteThickness { .. } => ValidationIssueSnapshot {
            code: "non_finite_thickness",
            vertices: Vec::new(),
            edges: Vec::new(),
        },
        PaperValidationIssue::NegativeThickness { .. } => ValidationIssueSnapshot {
            code: "negative_thickness",
            vertices: Vec::new(),
            edges: Vec::new(),
        },
        PaperValidationIssue::TooFewBoundaryVertices { .. } => ValidationIssueSnapshot {
            code: "too_few_boundary_vertices",
            vertices: unique_vertex_ids(project.paper.boundary_vertices.iter().copied()),
            edges: Vec::new(),
        },
        PaperValidationIssue::DuplicateBoundaryVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "duplicate_boundary_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        PaperValidationIssue::MissingBoundaryVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "missing_boundary_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        PaperValidationIssue::NonFiniteBoundaryVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "non_finite_boundary_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        PaperValidationIssue::MissingBoundaryEdge { boundary_edge } => ValidationIssueSnapshot {
            code: "missing_boundary_edge",
            vertices: boundary_vertices(&[*boundary_edge]),
            edges: Vec::new(),
        },
        PaperValidationIssue::DuplicateBoundaryEdge {
            boundary_edge,
            first_edge,
            duplicate_edge,
        } => ValidationIssueSnapshot {
            code: "duplicate_boundary_edge",
            vertices: boundary_vertices(&[*boundary_edge]),
            edges: unique_edge_ids([*first_edge, *duplicate_edge]),
        },
        PaperValidationIssue::UnexpectedBoundaryEdge { edge, start, end } => {
            ValidationIssueSnapshot {
                code: "unexpected_boundary_edge",
                vertices: unique_vertex_ids([*start, *end]),
                edges: vec![*edge],
            }
        }
        PaperValidationIssue::ZeroLengthBoundaryEdge { edge } => ValidationIssueSnapshot {
            code: "zero_length_boundary_edge",
            vertices: boundary_vertices(&[*edge]),
            edges: boundary_edge_ids(pattern, &[*edge]),
        },
        PaperValidationIssue::SelfIntersection {
            first_edge,
            second_edge,
            ..
        } => {
            let boundary_edges = [*first_edge, *second_edge];
            ValidationIssueSnapshot {
                code: "boundary_self_intersection",
                vertices: boundary_vertices(&boundary_edges),
                edges: boundary_edge_ids(pattern, &boundary_edges),
            }
        }
        PaperValidationIssue::IntersectionCalculationFailed {
            first_edge,
            second_edge,
            ..
        } => {
            let boundary_edges = [*first_edge, *second_edge];
            ValidationIssueSnapshot {
                code: "boundary_intersection_calculation_failed",
                vertices: boundary_vertices(&boundary_edges),
                edges: boundary_edge_ids(pattern, &boundary_edges),
            }
        }
        PaperValidationIssue::ZeroArea { boundary_vertices } => ValidationIssueSnapshot {
            code: "zero_area_boundary",
            vertices: unique_vertex_ids(boundary_vertices.iter().copied()),
            edges: Vec::new(),
        },
        PaperValidationIssue::AreaCalculationFailed {
            boundary_vertices, ..
        } => ValidationIssueSnapshot {
            code: "boundary_area_calculation_failed",
            vertices: unique_vertex_ids(boundary_vertices.iter().copied()),
            edges: Vec::new(),
        },
    }
}

fn boundary_vertices(boundary_edges: &[BoundaryEdgeRef]) -> Vec<VertexId> {
    unique_vertex_ids(
        boundary_edges
            .iter()
            .flat_map(|edge| [edge.start, edge.end]),
    )
}

fn unique_vertex_ids(vertices: impl IntoIterator<Item = VertexId>) -> Vec<VertexId> {
    let mut unique = Vec::new();
    for vertex in vertices {
        if !unique.contains(&vertex) {
            unique.push(vertex);
        }
    }
    unique
}

fn unique_edge_ids(edges: impl IntoIterator<Item = EdgeId>) -> Vec<EdgeId> {
    let mut unique = Vec::new();
    for edge in edges {
        if !unique.contains(&edge) {
            unique.push(edge);
        }
    }
    unique
}

fn boundary_edge_ids(pattern: &CreasePattern, boundary_edges: &[BoundaryEdgeRef]) -> Vec<EdgeId> {
    let mut matching = Vec::new();
    for boundary_edge in boundary_edges {
        for edge in &pattern.edges {
            let endpoints_match = (edge.start == boundary_edge.start
                && edge.end == boundary_edge.end)
                || (edge.start == boundary_edge.end && edge.end == boundary_edge.start);
            if edge.kind == EdgeKind::Boundary && endpoints_match && !matching.contains(&edge.id) {
                matching.push(edge.id);
            }
        }
    }
    matching
}

#[cfg(target_os = "macos")]
fn macos_menu(app_handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let package = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = AboutMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };
    let quit = MenuItem::with_id(
        app_handle,
        MACOS_QUIT_MENU_ID,
        format!("Quit {}", package.name),
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    let app_menu = Submenu::with_items(
        app_handle,
        package.name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::services(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::hide(app_handle, None)?,
            &PredefinedMenuItem::hide_others(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &quit,
        ],
    )?;
    let file_menu = Submenu::with_items(
        app_handle,
        "File",
        true,
        &[&PredefinedMenuItem::close_window(app_handle, None)?],
    )?;
    let edit_menu = Submenu::with_items(
        app_handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app_handle, None)?,
            &PredefinedMenuItem::redo(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::cut(app_handle, None)?,
            &PredefinedMenuItem::copy(app_handle, None)?,
            &PredefinedMenuItem::paste(app_handle, None)?,
            &PredefinedMenuItem::select_all(app_handle, None)?,
        ],
    )?;
    let view_menu = Submenu::with_items(
        app_handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
    )?;
    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;
    let help_menu = Submenu::with_id_and_items(app_handle, HELP_SUBMENU_ID, "Help", true, &[])?;

    Menu::with_items(
        app_handle,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder
        .enable_macos_default_menu(false)
        .menu(macos_menu)
        .on_menu_event(|app_handle, event| {
            if event.id() == MACOS_QUIT_MENU_ID {
                app_handle.exit(0);
            }
        });

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState(Mutex::new(initial_project_state())))
        .manage(ExitGuard::default())
        .invoke_handler(tauri::generate_handler![
            generate_benchmark_pattern,
            project_snapshot,
            validate_project,
            open_project,
            save_project,
            save_project_as,
            add_vertex,
            move_vertex,
            remove_vertex,
            add_edge,
            remove_edge,
            undo,
            redo,
            set_cutting_allowed
        ])
        .build(tauri::generate_context!())
        .expect("failed to build ORIGAMI2 desktop application");

    app.run(|app_handle, event| {
        let tauri::RunEvent::ExitRequested { api, .. } = event else {
            return;
        };

        // Closing the last window is already confirmed by the WebView's
        // close-requested handler. App-level quit paths (notably Cmd+Q on
        // macOS) arrive while the main window still exists and need their own
        // native confirmation.
        if app_handle.webview_windows().is_empty() {
            return;
        }

        let exit_guard = app_handle.state::<ExitGuard>();
        if exit_guard.allow_once.swap(false, Ordering::SeqCst) {
            return;
        }

        let project_state = app_handle.state::<AppState>();
        let project_is_dirty = lock_project(&project_state)
            .map(|project| project.is_dirty())
            .unwrap_or(true);
        if !project_is_dirty {
            return;
        }

        api.prevent_exit();
        if exit_guard.dialog_open.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut dialog = app_handle
            .dialog()
            .message("未保存の変更があります。変更を破棄して終了しますか？")
            .title("ORIGAMI2")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "変更を破棄して終了".to_owned(),
                "キャンセル".to_owned(),
            ));
        if let Some(window) = app_handle.get_webview_window("main") {
            dialog = dialog.parent(&window);
        }

        let exit_handle = app_handle.clone();
        dialog.show(move |discard_changes| {
            let exit_guard = exit_handle.state::<ExitGuard>();
            exit_guard.dialog_open.store(false, Ordering::SeqCst);
            if discard_changes {
                exit_guard.allow_once.store(true, Ordering::SeqCst);
                exit_handle.exit(0);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, Vertex};

    use super::*;

    #[test]
    fn move_vertex_returns_the_updated_revision_and_snapshot() {
        let id = VertexId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![Vertex {
                id,
                position: Point2::new(1.0, 2.0),
            }],
            edges: Vec::new(),
        });
        let project_id = project.project_id;
        assert!(!project.is_dirty());

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::MoveVertex {
                id,
                position: Point2::new(3.0, 5.0),
            },
        )
        .expect("move vertex");

        assert_eq!(response.revision, 1);
        assert_eq!(
            response.crease_pattern.vertices[0].position,
            Point2::new(3.0, 5.0)
        );
        assert!(response.can_undo);
        assert!(response.is_dirty);
    }

    #[test]
    fn initial_project_is_a_clean_square_sheet() {
        let project = initial_project_state();
        let snapshot = snapshot(&project);

        assert!(!snapshot.is_dirty);
        assert_eq!(snapshot.revision, 0);
        assert_eq!(project.paper.boundary_vertices.len(), 4);
        assert_eq!(snapshot.crease_pattern.vertices.len(), 4);
        assert_eq!(snapshot.crease_pattern.edges.len(), 4);
        assert!(
            snapshot
                .crease_pattern
                .edges
                .iter()
                .all(|edge| edge.kind == EdgeKind::Boundary)
        );
    }

    #[test]
    fn remove_edge_then_vertex_returns_each_current_snapshot() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        });
        let project_id = project.project_id;

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::RemoveEdge { id: edge },
        )
        .expect("remove edge");
        assert_eq!(response.revision, 1);
        assert!(response.crease_pattern.edges.is_empty());

        let response = execute_command(
            &mut project,
            project_id,
            1,
            Command::RemoveVertex { id: start },
        )
        .expect("remove vertex");
        assert_eq!(response.revision, 2);
        assert_eq!(response.crease_pattern.vertices.len(), 1);
        assert_eq!(response.crease_pattern.vertices[0].id, end);
    }

    #[test]
    fn edit_commands_preserve_revision_conflict_errors() {
        let id = VertexId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![Vertex {
                id,
                position: Point2::new(0.0, 0.0),
            }],
            edges: Vec::new(),
        });
        let project_id = project.project_id;

        let error = execute_command(&mut project, project_id, 4, Command::RemoveVertex { id })
            .expect_err("stale command must fail");

        assert_eq!(error, "expected revision 4, but the current revision is 0");
        assert_eq!(project.editor.pattern().vertices.len(), 1);
    }

    #[test]
    fn validation_snapshot_identifies_both_crossing_edges() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let project = ProjectState::new(CreasePattern {
            vertices: vertices.to_vec(),
            edges: vec![
                Edge {
                    id: first_edge,
                    start: vertices[0].id,
                    end: vertices[1].id,
                    kind: EdgeKind::Mountain,
                },
                Edge {
                    id: second_edge,
                    start: vertices[2].id,
                    end: vertices[3].id,
                    kind: EdgeKind::Valley,
                },
            ],
        });

        let response = validation_snapshot(&project);

        assert!(!response.is_valid);
        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.revision, 0);
        assert_eq!(response.issues.len(), 2);
        let crossing = response
            .issues
            .iter()
            .find(|issue| issue.code == "unsplit_intersection")
            .expect("crease-pattern issue");
        assert_eq!(crossing.edges, vec![first_edge, second_edge]);
        assert!(
            response
                .issues
                .iter()
                .any(|issue| issue.code == "too_few_boundary_vertices")
        );
    }

    #[test]
    fn valid_initial_sheet_has_no_combined_validation_issues() {
        let project = initial_project_state();

        let response = validation_snapshot(&project);

        assert!(response.is_valid);
        assert!(response.issues.is_empty());
    }

    #[test]
    fn paper_thickness_issues_are_included_without_highlight_targets() {
        let mut project = initial_project_state();
        project.paper.thickness_mm = -0.01;

        let response = validation_snapshot(&project);

        assert!(!response.is_valid);
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].code, "negative_thickness");
        assert!(response.issues[0].vertices.is_empty());
        assert!(response.issues[0].edges.is_empty());

        project.paper.thickness_mm = 0.0;
        let zero_thickness = validation_snapshot(&project);
        assert!(zero_thickness.is_valid);
        assert!(zero_thickness.issues.is_empty());
    }

    #[test]
    fn paper_intersection_maps_boundary_references_to_domain_edges() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let boundary_edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
        let pattern = CreasePattern {
            vertices: vertices.to_vec(),
            edges: vec![
                Edge {
                    id: boundary_edges[0],
                    start: vertices[0].id,
                    end: vertices[1].id,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: boundary_edges[1],
                    start: vertices[1].id,
                    end: vertices[2].id,
                    kind: EdgeKind::Boundary,
                },
                // Domain edges are undirected for boundary highlighting, so
                // mapping also accepts the reverse of the paper's order.
                Edge {
                    id: boundary_edges[2],
                    start: vertices[3].id,
                    end: vertices[2].id,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: boundary_edges[3],
                    start: vertices[3].id,
                    end: vertices[0].id,
                    kind: EdgeKind::Boundary,
                },
            ],
        };
        let mut project = ProjectState::new(pattern);
        project.paper.boundary_vertices = vertices.iter().map(|vertex| vertex.id).collect();

        let response = validation_snapshot(&project);
        let intersection = response
            .issues
            .iter()
            .find(|issue| issue.code == "boundary_self_intersection")
            .expect("paper self-intersection issue");

        assert_eq!(
            intersection.vertices,
            vec![
                vertices[0].id,
                vertices[1].id,
                vertices[2].id,
                vertices[3].id
            ]
        );
        assert_eq!(
            intersection.edges,
            vec![boundary_edges[0], boundary_edges[2]]
        );
    }

    #[test]
    fn paper_boundary_topology_issues_include_actionable_targets() {
        let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary = paper.boundary_vertices.clone();

        pattern.edges[0].kind = EdgeKind::Mountain;
        let first_duplicate = pattern.edges[1].id;
        let duplicate_edge = Edge {
            id: EdgeId::new(),
            start: pattern.edges[1].end,
            end: pattern.edges[1].start,
            kind: EdgeKind::Boundary,
        };
        let duplicate = duplicate_edge.id;
        pattern.edges.push(duplicate_edge);
        let unexpected_edge = Edge {
            id: EdgeId::new(),
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Boundary,
        };
        let unexpected = unexpected_edge.id;
        pattern.edges.push(unexpected_edge);
        let project = ProjectState::new_with_paper(pattern, paper);

        let response = validation_snapshot(&project);
        let missing = response
            .issues
            .iter()
            .find(|issue| issue.code == "missing_boundary_edge")
            .expect("wrong-kind edge is missing from the Boundary set");
        assert_eq!(missing.vertices, vec![boundary[0], boundary[1]]);
        assert!(missing.edges.is_empty());

        let duplicate_issue = response
            .issues
            .iter()
            .find(|issue| issue.code == "duplicate_boundary_edge")
            .expect("duplicate Boundary record");
        assert_eq!(duplicate_issue.vertices, vec![boundary[1], boundary[2]]);
        assert_eq!(duplicate_issue.edges, vec![first_duplicate, duplicate]);

        let unexpected_issue = response
            .issues
            .iter()
            .find(|issue| issue.code == "unexpected_boundary_edge")
            .expect("unexpected Boundary chord");
        assert_eq!(unexpected_issue.vertices, vec![boundary[0], boundary[2]]);
        assert_eq!(unexpected_issue.edges, vec![unexpected]);
    }

    #[test]
    fn save_as_extension_is_normalized_without_changing_valid_case() {
        assert_eq!(
            ensure_ori2_extension(PathBuf::from("crane")),
            PathBuf::from("crane.ori2")
        );
        assert_eq!(
            ensure_ori2_extension(PathBuf::from("crane.json")),
            PathBuf::from("crane.ori2")
        );
        assert_eq!(
            ensure_ori2_extension(PathBuf::from("crane.ORI2")),
            PathBuf::from("crane.ORI2")
        );
    }

    #[test]
    fn suggested_name_removes_platform_forbidden_characters() {
        assert_eq!(
            suggested_file_name("  Bird: prototype?  "),
            "Bird_ prototype_.ori2"
        );
        assert_eq!(suggested_file_name("..."), "Untitled.ori2");
    }

    #[test]
    fn generated_container_verification_is_pure_and_checks_identity() {
        let document = ProjectDocument::new("Bird", CreasePattern::empty());
        let bytes = write_project_ori2(&document).expect("generate .ori2");
        verify_generated_ori2(&document, &bytes).expect("verify generated .ori2");

        let different_document = ProjectDocument::new("Different", CreasePattern::empty());
        let error = verify_generated_ori2(&different_document, &bytes)
            .expect_err("a different project must not verify");
        assert_eq!(error, "generated .ori2 data did not round-trip exactly");
    }

    #[test]
    fn document_snapshot_keeps_identity_name_and_dirty_state() {
        let mut document = ProjectDocument::new("Loaded bird", CreasePattern::empty());
        document.paper.cutting_allowed = true;
        let project = ProjectState::from_document(document.clone(), PathBuf::from("bird.ori2"));
        let response = snapshot(&project);

        assert_eq!(response.project_id, document.project_id);
        assert_eq!(response.name, "Loaded bird");
        assert_eq!(response.current_path.as_deref(), Some("bird.ori2"));
        assert!(!response.is_dirty);
        assert!(response.cutting_allowed);
        assert!(!response.can_undo);
        assert_eq!(project.document(), document);
    }

    #[test]
    fn stale_project_identity_cannot_mutate_a_replacement_project() {
        let mut project = ProjectState::new(CreasePattern::empty());
        let stale_project_id = ProjectId::new();

        let error = execute_command(
            &mut project,
            stale_project_id,
            0,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 1.0),
            },
        )
        .expect_err("a command for another project must fail");

        assert_eq!(
            error,
            "the active project changed before the command was applied"
        );
        assert!(project.editor.pattern().vertices.is_empty());
    }

    #[test]
    fn undoing_to_saved_content_clears_dirty_state() {
        let vertex_id = VertexId::new();
        let document = ProjectDocument::new(
            "Saved bird",
            CreasePattern {
                vertices: vec![Vertex {
                    id: vertex_id,
                    position: Point2::new(1.0, 2.0),
                }],
                edges: Vec::new(),
            },
        );
        let mut project = ProjectState::from_document(document, PathBuf::from("bird.ori2"));
        let project_id = project.project_id;

        execute_command(
            &mut project,
            project_id,
            0,
            Command::MoveVertex {
                id: vertex_id,
                position: Point2::new(3.0, 4.0),
            },
        )
        .expect("move vertex");
        assert!(project.is_dirty());

        project.editor.undo(1).expect("undo to save point");
        assert!(!project.is_dirty());
    }

    #[test]
    fn undoing_a_removal_to_saved_order_clears_dirty_state() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let document = ProjectDocument::new(
            "Saved bird",
            CreasePattern {
                vertices: vertices.to_vec(),
                edges: Vec::new(),
            },
        );
        let mut project = ProjectState::from_document(document, PathBuf::from("bird.ori2"));
        let project_id = project.project_id;

        execute_command(
            &mut project,
            project_id,
            0,
            Command::RemoveVertex { id: vertices[1].id },
        )
        .expect("remove middle vertex");
        assert!(project.is_dirty());

        project.editor.undo(1).expect("undo to saved ordering");
        assert!(!project.is_dirty());
    }
}
