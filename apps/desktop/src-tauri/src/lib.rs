mod diagnostics;

use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use diagnostics::{
    DiagnosticsState, prepare_diagnostics_share_preview, record_unexpected_diagnostic,
    save_diagnostics_share_preview,
};
use ori_core::{
    BoundaryEdgeRef, Command, EditorState, EditorTopology, IntersectionEdgeTarget,
    JunctionVertexIntent, LocalFlatFoldabilityReport, PaperValidationIssue, TopologyAnalysisInput,
    TopologyIssue, TopologySnapshot, ValidationIssue, analyze_local_flat_foldability,
    create_rectangular_sheet, validate_paper,
};
use ori_domain::{
    CreasePattern, EdgeId, EdgeKind, FaceId, InstructionHingeAngle, InstructionPose,
    InstructionPoseModel, InstructionStep, InstructionStepId, InstructionTimeline,
    MAX_INSTRUCTION_HINGES_PER_STEP, Paper, Point2, ProjectId, RgbaColor, VertexId,
};
use ori_formats::{
    CURRENT_FORMAT_VERSION, Ori2Limits, ProjectDocument, read_project_ori2_with_limits,
    write_project_ori2,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[cfg(target_os = "windows")]
use std::{
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, RawHandle},
    },
    ptr,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_RENAME_INFO, FILE_SHARE_READ,
    FileRenameInfo, SetFileInformationByHandle,
};

#[cfg(target_os = "macos")]
use tauri::menu::{
    AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};

const UNTITLED_PROJECT_NAME: &str = "Untitled";
const DEFAULT_SHEET_SIZE_MM: f64 = 400.0;
const MAX_PROJECT_NAME_CHARS: usize = 120;
const MAX_BENCHMARK_EDGE_COUNT: usize = 100_000;
static NEXT_STAGED_FILE_ID: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "macos")]
const MACOS_QUIT_MENU_ID: &str = "origami2_quit";

struct AppState(Mutex<ProjectState>);

#[derive(Default)]
struct ExitGuard {
    allow_once: AtomicBool,
    dialog_open: AtomicBool,
}

struct ProjectState {
    /// Non-persisted identity for this particular open/new project instance.
    ///
    /// A persisted project ID can legitimately reappear after reopening the
    /// same file. Delayed mutating work must therefore bind to this identity
    /// as well as the document ID and revision.
    instance_id: ProjectId,
    project_id: ProjectId,
    name: String,
    current_path: Option<PathBuf>,
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
        let editor = EditorState::with_paper(pattern, paper);
        let mut project = Self {
            instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            name: UNTITLED_PROJECT_NAME.to_owned(),
            current_path: None,
            editor,
            saved_revision: None,
            saved_document: None,
        };
        // The built-in startup sheet is a clean baseline. In contrast, a
        // user-created project uses `new_unsaved` and remains dirty until its
        // first successful save.
        project.saved_document = Some(project.document());
        project
    }

    fn new_unsaved(name: String, pattern: CreasePattern, paper: Paper) -> Self {
        let editor = EditorState::with_paper(pattern, paper);
        Self {
            instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            name,
            current_path: None,
            editor,
            saved_revision: None,
            saved_document: None,
        }
    }

    fn from_document(document: ProjectDocument, current_path: PathBuf) -> Self {
        let saved_document = document.clone();
        let editor = EditorState::with_document_parts(
            document.crease_pattern,
            document.paper,
            document.instruction_timeline,
        );
        Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: Some(current_path),
            saved_revision: Some(editor.revision()),
            saved_document: Some(saved_document),
            editor,
        }
    }

    fn document(&self) -> ProjectDocument {
        ProjectDocument {
            format_version: CURRENT_FORMAT_VERSION,
            project_id: self.project_id,
            name: self.name.clone(),
            paper: self.editor.paper().clone(),
            crease_pattern: self.editor.pattern().clone(),
            instruction_timeline: self.editor.instruction_timeline().clone(),
        }
    }

    fn is_dirty(&self) -> bool {
        let Some(saved) = &self.saved_document else {
            return true;
        };
        saved.format_version != CURRENT_FORMAT_VERSION
            || saved.project_id != self.project_id
            || saved.name != self.name
            || saved.paper != *self.editor.paper()
            || saved.crease_pattern != *self.editor.pattern()
            || saved.instruction_timeline != *self.editor.instruction_timeline()
    }
}

fn initial_project_state() -> ProjectState {
    let sheet = create_rectangular_sheet(DEFAULT_SHEET_SIZE_MM, DEFAULT_SHEET_SIZE_MM, false)
        .expect("the built-in default sheet dimensions must be valid");
    let (pattern, paper) = sheet.into_parts();
    ProjectState::new_with_paper(pattern, paper)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct PatternResponse {
    requested_edge_count: usize,
    vertex_count: usize,
    edge_count: usize,
    vertices: Vec<BenchmarkVertex>,
    edges: Vec<BenchmarkEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct BenchmarkVertex {
    id: String,
    position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct BenchmarkEdge {
    id: String,
    start: String,
    end: String,
    kind: EdgeKind,
}

#[derive(Debug, Serialize)]
struct ProjectSnapshot {
    project_id: ProjectId,
    name: String,
    current_path: Option<String>,
    revision: u64,
    saved_revision: Option<u64>,
    is_dirty: bool,
    paper: Paper,
    crease_pattern: CreasePattern,
    instruction_timeline: InstructionTimeline,
    fold_model_fingerprint: String,
    can_undo: bool,
    can_redo: bool,
    cutting_allowed: bool,
}

#[derive(Debug, Serialize)]
struct ProjectFileResponse {
    canceled: bool,
    project: ProjectSnapshot,
}

#[derive(Debug)]
struct LoadedProjectFile {
    path: PathBuf,
    document: ProjectDocument,
}

#[derive(Debug, Serialize)]
struct EdgeIntersectionResponse {
    snapshot: ProjectSnapshot,
    vertex_id: VertexId,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum IntersectionClusterRelation {
    Interior,
    Endpoint,
}

const MIN_INTERSECTION_CLUSTER_TARGETS: usize = 3;
const MAX_INTERSECTION_CLUSTER_TARGETS: usize = 64;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntersectionClusterTargetRequest {
    edge_id: EdgeId,
    relation: IntersectionClusterRelation,
}

#[derive(Debug, Serialize)]
struct TJunctionResponse {
    snapshot: ProjectSnapshot,
    vertex_id: VertexId,
}

#[derive(Debug, Serialize)]
struct ValidationSnapshot {
    project_id: ProjectId,
    revision: u64,
    is_valid: bool,
    issues: Vec<ValidationIssueSnapshot>,
    local_flat_foldability: LocalFlatFoldabilityReport,
}

#[derive(Debug, Serialize)]
struct ValidationIssueSnapshot {
    code: &'static str,
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
}

#[derive(Debug, Serialize)]
struct ProjectTopologyResponse {
    project_id: ProjectId,
    revision: u64,
    /// Strict gate for folding consumers. A false response never carries a
    /// snapshot, even if analysis later gains partial diagnostic snapshots.
    simulation_ready: bool,
    snapshot: Option<TopologySnapshot>,
    issues: Vec<TopologyIssue>,
}

struct NewProjectParameters {
    name: String,
    width_mm: f64,
    height_mm: f64,
    thickness_mm: f64,
    cutting_allowed: bool,
    front_color: RgbaColor,
    back_color: RgbaColor,
}

#[tauri::command]
fn generate_benchmark_pattern(edge_count: usize) -> PatternResponse {
    let edge_count = edge_count.min(MAX_BENCHMARK_EDGE_COUNT);
    if edge_count == 0 {
        return PatternResponse {
            requested_edge_count: edge_count,
            vertex_count: 0,
            edge_count: 0,
            vertices: Vec::new(),
            edges: Vec::new(),
        };
    }

    // Keep the payload independent from the open project and its undo history.
    // Stable index-based IDs also make native and browser benchmark fixtures
    // structurally comparable without leaking random domain IDs into metrics.
    let mut side = ((edge_count as f64 / 2.0).sqrt().ceil() as usize).max(2);
    while 2 * side * (side - 1) < edge_count {
        side += 1;
    }

    let vertices = (0..side * side)
        .map(|index| BenchmarkVertex {
            id: benchmark_vertex_id(index),
            position: Point2::new((index % side) as f64, (index / side) as f64),
        })
        .collect::<Vec<_>>();

    let mut edges = Vec::with_capacity(edge_count);
    'grid: for y in 0..side {
        for x in 0..side {
            let index = y * side + x;
            if x + 1 < side {
                edges.push(BenchmarkEdge {
                    id: benchmark_edge_id(edges.len()),
                    start: benchmark_vertex_id(index),
                    end: benchmark_vertex_id(index + 1),
                    kind: if y % 2 == 0 {
                        EdgeKind::Mountain
                    } else {
                        EdgeKind::Valley
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
            if y + 1 < side {
                edges.push(BenchmarkEdge {
                    id: benchmark_edge_id(edges.len()),
                    start: benchmark_vertex_id(index),
                    end: benchmark_vertex_id(index + side),
                    kind: if x % 2 == 0 {
                        EdgeKind::Valley
                    } else {
                        EdgeKind::Mountain
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
        }
    }

    PatternResponse {
        requested_edge_count: edge_count,
        vertex_count: vertices.len(),
        edge_count: edges.len(),
        vertices,
        edges,
    }
}

fn benchmark_vertex_id(index: usize) -> String {
    format!("benchmark-v-{index}")
}

fn benchmark_edge_id(index: usize) -> String {
    format!("benchmark-e-{index}")
}

#[tauri::command]
fn project_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project = lock_project(&state)?;
    Ok(snapshot(&project))
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn new_project(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    name: String,
    width_mm: f64,
    height_mm: f64,
    thickness_mm: f64,
    cutting_allowed: bool,
    front_color: RgbaColor,
    back_color: RgbaColor,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    replace_with_new_project(
        &mut project,
        expected_project_id,
        expected_revision,
        NewProjectParameters {
            name,
            width_mm,
            height_mm,
            thickness_mm,
            cutting_allowed,
            front_color,
            back_color,
        },
    )
}

#[tauri::command]
fn validate_project(state: State<'_, AppState>) -> Result<ValidationSnapshot, String> {
    let project = lock_project(&state)?;
    Ok(validation_snapshot(&project))
}

/// Analyzes immutable topology input away from the project-state lock.
///
/// Unsupported or invalid folding geometry is a successful command response
/// with structured issues. Operational failures and stale results are command
/// errors, so the UI cannot accidentally display topology from another edit.
#[tauri::command]
async fn analyze_project_topology(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectTopologyResponse, String> {
    let input = {
        let project = lock_project(&state)?;
        capture_topology_input(&project, expected_project_id, expected_revision)?
    };
    let (input, topology) = tauri::async_runtime::spawn_blocking(move || {
        let topology = input.analyze();
        (input, topology)
    })
    .await
    .map_err(|error| format!("topology analysis task failed: {error}"))?;

    let project = lock_project(&state)?;
    finish_topology_response(&project, &input, topology)
}

#[tauri::command]
async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
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
        .map_err(|error| format!("the selected location is not a local file: {error}"))?;
    let loaded = load_project_file(path)?;

    let mut project = lock_project(&state)?;
    apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        loaded,
    )
}

#[tauri::command]
async fn save_project(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectFileResponse, String> {
    {
        let mut project = lock_project(&state)?;
        if let Some(path) = project.current_path.clone() {
            return save_project_to_path(&mut project, path);
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

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn add_instruction_step(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    title: String,
    description: String,
    caution: String,
    duration_ms: u32,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<ProjectSnapshot, String> {
    let analyzed = analyze_instruction_pose(
        &state,
        expected_project_id,
        expected_revision,
        fixed_face,
        hinge_angles,
    )
    .await?;
    let mut project = lock_project(&state)?;
    let pose = finish_instruction_pose(&project, expected_project_id, expected_revision, analyzed)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: InstructionStepId::new(),
                title,
                description,
                caution,
                duration_ms,
                pose,
            },
        },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn update_instruction_step_metadata(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
    title: String,
    description: String,
    caution: String,
    duration_ms: u32,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::UpdateInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
        },
    )
}

#[tauri::command]
async fn replace_instruction_step_pose(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<ProjectSnapshot, String> {
    let analyzed = analyze_instruction_pose(
        &state,
        expected_project_id,
        expected_revision,
        fixed_face,
        hinge_angles,
    )
    .await?;
    let mut project = lock_project(&state)?;
    let pose = finish_instruction_pose(&project, expected_project_id, expected_revision, analyzed)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::ReplaceInstructionStepPose { step_id, pose },
    )
}

#[tauri::command]
fn remove_instruction_step(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::RemoveInstructionStep { step_id },
    )
}

#[tauri::command]
fn move_instruction_step(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
    target_index: usize,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::MoveInstructionStep {
            step_id,
            target_index,
        },
    )
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

#[tauri::command]
fn update_paper_properties(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    thickness_mm: f64,
    front_color: RgbaColor,
    back_color: RgbaColor,
    cutting_allowed: bool,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::UpdatePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            cutting_allowed,
        },
    )
}

#[tauri::command]
fn resize_rectangular_paper(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    width_mm: f64,
    height_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::ResizeRectangularPaper {
            width_mm,
            height_mm,
        },
    )
}

#[tauri::command]
fn split_edge(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_edge_split(
        &mut project,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_edge_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_id,
        expected_revision,
        Command::SplitEdge {
            edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction,
        },
    )
}

#[tauri::command]
fn connect_edge_intersection(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let mut project = lock_project(&state)?;
    execute_edge_intersection_connection(
        &mut project,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_edge_intersection_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let vertex_id = VertexId::new();
    let snapshot = execute_command(
        project,
        expected_project_id,
        expected_revision,
        Command::ConnectEdgeIntersection {
            first_edge,
            second_edge,
            new_vertex: vertex_id,
            first_new_edge: EdgeId::new(),
            second_new_edge: EdgeId::new(),
        },
    )?;
    Ok(EdgeIntersectionResponse {
        snapshot,
        vertex_id,
    })
}

#[tauri::command]
fn connect_intersection_cluster(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    validate_intersection_cluster_target_count(targets.len())?;
    let mut project = lock_project(&state)?;
    execute_intersection_cluster_connection(
        &mut project,
        expected_project_id,
        expected_revision,
        targets,
        junction_vertex_id,
    )
}

fn execute_intersection_cluster_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    validate_intersection_cluster_target_count(targets.len())?;
    let (junction, vertex_id) = match junction_vertex_id {
        Some(id) => (JunctionVertexIntent::Reuse { id }, id),
        None => {
            let id = VertexId::new();
            (JunctionVertexIntent::Create { id }, id)
        }
    };
    let targets = targets
        .into_iter()
        .map(|target| IntersectionEdgeTarget {
            edge: target.edge_id,
            new_edge: match target.relation {
                IntersectionClusterRelation::Interior => Some(EdgeId::new()),
                IntersectionClusterRelation::Endpoint => None,
            },
        })
        .collect();
    let snapshot = execute_command(
        project,
        expected_project_id,
        expected_revision,
        Command::ConnectIntersectionCluster { junction, targets },
    )?;
    Ok(EdgeIntersectionResponse {
        snapshot,
        vertex_id,
    })
}

fn validate_intersection_cluster_target_count(count: usize) -> Result<(), String> {
    if count < MIN_INTERSECTION_CLUSTER_TARGETS {
        return Err(format!(
            "an intersection cluster requires at least three target edges, found {count}"
        ));
    }
    if count > MAX_INTERSECTION_CLUSTER_TARGETS {
        return Err(format!(
            "an intersection cluster supports at most {MAX_INTERSECTION_CLUSTER_TARGETS} target edges, found {count}"
        ));
    }
    Ok(())
}

#[tauri::command]
fn connect_t_junction(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let mut project = lock_project(&state)?;
    execute_t_junction_connection(
        &mut project,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_t_junction_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let new_edge = EdgeId::new();
    let snapshot = execute_command(
        project,
        expected_project_id,
        expected_revision,
        Command::ConnectTJunction {
            first_edge,
            second_edge,
            new_edge,
        },
    )?;
    let vertex_id = snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| edge.id == new_edge)
        .map(|edge| edge.start)
        .expect("a successful T-junction command must create its requested edge");
    Ok(TJunctionResponse {
        snapshot,
        vertex_id,
    })
}

#[tauri::command]
fn split_boundary_edge(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_boundary_split(
        &mut project,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_boundary_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_id,
        expected_revision,
        Command::SplitBoundaryEdge {
            edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction,
        },
    )
}

#[tauri::command]
fn remove_boundary_vertex(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertex: VertexId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_id,
        expected_revision,
        Command::RemoveBoundaryVertex { vertex },
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

fn replace_with_new_project(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    parameters: NewProjectParameters,
) -> Result<ProjectSnapshot, String> {
    ensure_project_identity(project, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err(format!(
            "expected revision {expected_revision}, but the current revision is {}",
            project.editor.revision()
        ));
    }

    let replacement = create_new_project_state(parameters)?;
    *project = replacement;
    Ok(snapshot(project))
}

fn create_new_project_state(parameters: NewProjectParameters) -> Result<ProjectState, String> {
    let name = normalize_project_name(&parameters.name)?;
    validate_paper_thickness(parameters.thickness_mm)?;
    let sheet = create_rectangular_sheet(
        parameters.width_mm,
        parameters.height_mm,
        parameters.cutting_allowed,
    )
    .map_err(|error| format!("failed to create the paper sheet: {error}"))?;
    let (pattern, mut paper) = sheet.into_parts();
    paper.thickness_mm = parameters.thickness_mm;
    paper.front.color = parameters.front_color;
    paper.back.color = parameters.back_color;

    if !validate_paper(&paper, &pattern).is_valid() {
        return Err("the generated paper failed final validation".to_owned());
    }

    Ok(ProjectState::new_unsaved(name, pattern, paper))
}

fn normalize_project_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let character_count = trimmed.chars().count();
    if !(1..=MAX_PROJECT_NAME_CHARS).contains(&character_count) {
        return Err(format!(
            "project name must contain between 1 and {MAX_PROJECT_NAME_CHARS} characters after trimming"
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("project name must not contain control characters".to_owned());
    }
    Ok(trimmed.to_owned())
}

fn validate_paper_thickness(thickness_mm: f64) -> Result<(), String> {
    if !thickness_mm.is_finite() {
        return Err("paper thickness must be finite".to_owned());
    }
    if thickness_mm < 0.0 {
        return Err("paper thickness must be zero or greater".to_owned());
    }
    Ok(())
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
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<(), String> {
    if project.instance_id != expected_instance_id {
        return Err("the open project instance changed while the file dialog was open".to_owned());
    }
    ensure_project_identity(project, expected_project_id)?;
    if project.editor.revision() == expected_revision {
        Ok(())
    } else {
        Err("the project changed while the file dialog was open".to_owned())
    }
}

fn capture_topology_input(
    project: &ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<TopologyAnalysisInput, String> {
    ensure_project_identity(project, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err(format!(
            "expected revision {expected_revision}, but the current revision is {}",
            project.editor.revision()
        ));
    }
    Ok(project.editor.topology_analysis_input(project.project_id))
}

struct AnalyzedInstructionPose {
    project_instance_id: ProjectId,
    input: TopologyAnalysisInput,
    topology: EditorTopology,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
}

async fn analyze_instruction_pose(
    state: &AppState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<AnalyzedInstructionPose, String> {
    if hinge_angles.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(format!(
            "an instruction step may contain at most {MAX_INSTRUCTION_HINGES_PER_STEP} hinges"
        ));
    }
    validate_instruction_hinge_angle_values(&hinge_angles)?;
    let (project_instance_id, input) = {
        let project = lock_project(state)?;
        (
            project.instance_id,
            capture_topology_input(&project, expected_project_id, expected_revision)?,
        )
    };
    let (input, topology) = tauri::async_runtime::spawn_blocking(move || {
        let topology = input.analyze();
        (input, topology)
    })
    .await
    .map_err(|error| format!("instruction topology analysis task failed: {error}"))?;

    Ok(AnalyzedInstructionPose {
        project_instance_id,
        input,
        topology,
        fixed_face,
        hinge_angles,
    })
}

fn finish_instruction_pose(
    project: &ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    analyzed: AnalyzedInstructionPose,
) -> Result<InstructionPose, String> {
    ensure_project_identity(project, expected_project_id)?;
    if project.instance_id != analyzed.project_instance_id {
        return Err(
            "the open project instance changed while the instruction pose was being analyzed"
                .to_owned(),
        );
    }
    if project.editor.revision() != expected_revision {
        return Err(format!(
            "expected revision {expected_revision}, but the current revision is {}",
            project.editor.revision()
        ));
    }
    if !analyzed
        .input
        .is_current_for(project.project_id, &project.editor)
    {
        return Err("the project changed while the instruction pose was being analyzed".to_owned());
    }
    if analyzed.topology.revision() != analyzed.input.revision() {
        return Err("instruction topology returned an unexpected revision".to_owned());
    }
    let topology = analyzed
        .topology
        .simulation_snapshot()
        .ok_or_else(|| "the current crease pattern cannot produce a foldable pose".to_owned())?;

    let topology = prepare_instruction_topology(topology)?;
    instruction_pose_from_context(
        &topology,
        project.editor.fold_model_fingerprint_v1(),
        analyzed.fixed_face,
        analyzed.hinge_angles,
    )
}

struct InstructionTopologyContext {
    face_ids: HashSet<FaceId>,
    expected_edges: Vec<EdgeId>,
    planar: bool,
}

fn prepare_instruction_topology(
    topology: &TopologySnapshot,
) -> Result<InstructionTopologyContext, String> {
    if topology.faces.is_empty() {
        return Err("an instruction pose requires at least one material face".to_owned());
    }
    if topology.hinge_adjacency.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(format!(
            "an instruction fold model may contain at most {MAX_INSTRUCTION_HINGES_PER_STEP} hinges"
        ));
    }

    let face_ids = topology
        .faces
        .iter()
        .map(|face| face.id)
        .collect::<HashSet<_>>();
    if face_ids.len() != topology.faces.len() {
        return Err("the fold model contains a duplicate material face".to_owned());
    }

    let planar = topology.hinge_adjacency.is_empty();
    if planar {
        if topology.faces.len() != 1 {
            return Err(
                "a hinge-free instruction pose must contain exactly one material face".to_owned(),
            );
        }
    } else {
        if topology.hinge_adjacency.len() + 1 != topology.faces.len() {
            return Err("instruction poses currently require a tree-shaped fold graph".to_owned());
        }
        let mut adjacency = face_ids
            .iter()
            .copied()
            .map(|face| (face, Vec::new()))
            .collect::<HashMap<_, _>>();
        for hinge in &topology.hinge_adjacency {
            if hinge.first == hinge.second
                || !face_ids.contains(&hinge.first)
                || !face_ids.contains(&hinge.second)
            {
                return Err("the fold model contains an invalid hinge face reference".to_owned());
            }
            adjacency
                .get_mut(&hinge.first)
                .expect("validated first hinge face must exist")
                .push(hinge.second);
            adjacency
                .get_mut(&hinge.second)
                .expect("validated second hinge face must exist")
                .push(hinge.first);
        }

        let mut reached = HashSet::with_capacity(topology.faces.len());
        let mut pending = vec![topology.faces[0].id];
        while let Some(face) = pending.pop() {
            if !reached.insert(face) {
                continue;
            }
            pending.extend(
                adjacency
                    .get(&face)
                    .expect("validated material face must have an adjacency entry")
                    .iter()
                    .copied(),
            );
        }
        if reached != face_ids {
            return Err("instruction poses currently require a connected fold graph".to_owned());
        }
    }

    let mut expected_edges = topology
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    expected_edges.sort_by_key(EdgeId::canonical_bytes);
    if expected_edges.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("the fold model contains a duplicate hinge edge".to_owned());
    }

    Ok(InstructionTopologyContext {
        face_ids,
        expected_edges,
        planar,
    })
}

#[cfg(test)]
fn instruction_pose_from_topology(
    topology: &TopologySnapshot,
    source_model_fingerprint: String,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<InstructionPose, String> {
    let topology = prepare_instruction_topology(topology)?;
    instruction_pose_from_context(
        &topology,
        source_model_fingerprint,
        fixed_face,
        hinge_angles,
    )
}

fn instruction_pose_from_context(
    topology: &InstructionTopologyContext,
    source_model_fingerprint: String,
    fixed_face: Option<FaceId>,
    mut hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<InstructionPose, String> {
    if hinge_angles.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(format!(
            "an instruction step may contain at most {MAX_INSTRUCTION_HINGES_PER_STEP} hinges"
        ));
    }
    validate_instruction_hinge_angle_values(&hinge_angles)?;
    if topology.planar {
        if fixed_face.is_some() {
            return Err("a planar instruction pose must not specify a fixed face".to_owned());
        }
        if !hinge_angles.is_empty() {
            return Err("a planar instruction pose must not contain hinge angles".to_owned());
        }
    } else {
        let fixed_face = fixed_face
            .ok_or_else(|| "a folded instruction pose requires a fixed face".to_owned())?;
        if !topology.face_ids.contains(&fixed_face) {
            return Err("the fixed face does not exist in the current fold model".to_owned());
        }
    }

    hinge_angles.sort_by_key(|hinge| hinge.edge.canonical_bytes());
    if hinge_angles.len() != topology.expected_edges.len()
        || hinge_angles
            .iter()
            .zip(&topology.expected_edges)
            .any(|(angle, expected)| angle.edge != *expected)
    {
        return Err(
            "the instruction pose must contain every current hinge exactly once".to_owned(),
        );
    }
    Ok(InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint,
        fixed_face,
        hinge_angles,
    })
}

fn validate_instruction_hinge_angle_values(
    hinge_angles: &[InstructionHingeAngle],
) -> Result<(), String> {
    if hinge_angles
        .iter()
        .any(|hinge| !hinge.angle_degrees.is_finite())
    {
        return Err("instruction hinge angles must be finite".to_owned());
    }
    if hinge_angles
        .iter()
        .any(|hinge| !(0.0..=180.0).contains(&hinge.angle_degrees))
    {
        return Err("instruction hinge angles must be between 0 and 180 degrees".to_owned());
    }
    Ok(())
}

fn finish_topology_response(
    project: &ProjectState,
    input: &TopologyAnalysisInput,
    topology: ori_core::EditorTopology,
) -> Result<ProjectTopologyResponse, String> {
    if !input.is_current_for(project.project_id, &project.editor) {
        return Err("the project changed while topology was being analyzed".to_owned());
    }
    if topology.revision() != input.revision() {
        return Err("topology analysis returned an unexpected revision".to_owned());
    }

    let simulation_ready = topology.is_simulation_ready();
    let report = topology.into_report();
    if report
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.source_revision != input.revision())
    {
        return Err("topology snapshot returned an unexpected source revision".to_owned());
    }
    Ok(ProjectTopologyResponse {
        project_id: project.project_id,
        revision: input.revision(),
        simulation_ready,
        snapshot: simulation_ready.then_some(report.snapshot).flatten(),
        issues: report.issues,
    })
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
        paper: project.editor.paper().clone(),
        crease_pattern: project.editor.pattern().clone(),
        instruction_timeline: project.editor.instruction_timeline().clone(),
        fold_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
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
    let (
        expected_instance_id,
        expected_project_id,
        expected_revision,
        initial_directory,
        suggested_name,
    ) = {
        let project = lock_project(state)?;
        (
            project.instance_id,
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
    let mut project = lock_project(state)?;
    save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        path,
    )
}

fn save_project_as_selected_path(
    project: &mut ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    selected_path: PathBuf,
) -> Result<ProjectFileResponse, String> {
    ensure_expected_project(
        project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    save_project_to_path(project, ensure_ori2_extension(selected_path))
}

fn save_project_to_path(
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

fn load_project_file(path: PathBuf) -> Result<LoadedProjectFile, String> {
    let document = load_document_from_path(&path)?;
    Ok(LoadedProjectFile { path, document })
}

fn apply_loaded_project_file(
    project: &mut ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    loaded: LoadedProjectFile,
) -> Result<ProjectFileResponse, String> {
    ensure_expected_project(
        project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    *project = ProjectState::from_document(loaded.document, loaded.path);
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

    let document = read_project_ori2_with_limits(&bytes, limits)
        .map_err(|error| format!("failed to validate {}: {error}", path.display()))?;
    validate_document_instruction_poses(&document).map_err(|error| {
        format!(
            "failed to validate folding instructions in {}: {error}",
            path.display()
        )
    })?;
    Ok(document)
}

fn validate_document_instruction_poses(document: &ProjectDocument) -> Result<(), String> {
    if document.instruction_timeline.steps.is_empty() {
        return Ok(());
    }
    let editor = EditorState::with_paper(document.crease_pattern.clone(), document.paper.clone());
    let current_fingerprint = editor.fold_model_fingerprint_v1();
    if !document
        .instruction_timeline
        .steps
        .iter()
        .any(|step| step.pose.source_model_fingerprint == current_fingerprint)
    {
        // Poses authored for an older crease pattern remain intentionally
        // loadable as stale, editable records. Playback keeps them disabled
        // until the user captures a new pose against the current model.
        return Ok(());
    }

    let topology = editor
        .topology_analysis_input(document.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().ok_or_else(|| {
        "a current instruction pose refers to a crease pattern that is not simulation-ready"
            .to_owned()
    })?;
    let topology = prepare_instruction_topology(snapshot)?;
    for (index, step) in document.instruction_timeline.steps.iter().enumerate() {
        if step.pose.source_model_fingerprint != current_fingerprint {
            continue;
        }
        instruction_pose_from_context(
            &topology,
            current_fingerprint.clone(),
            step.pose.fixed_face,
            step.pose.hinge_angles.clone(),
        )
        .map_err(|error| format!("instruction step {} is invalid: {error}", index + 1))?;
    }
    Ok(())
}

fn persist_document(path: &Path, document: &ProjectDocument) -> Result<(), String> {
    if path.file_name().is_none() {
        return Err(format!("{} is not a file path", path.display()));
    }

    validate_document_instruction_poses(document)
        .map_err(|error| format!("failed to validate folding instructions before save: {error}"))?;
    let bytes = write_project_ori2(document)
        .map_err(|error| format!("failed to create .ori2 data: {error}"))?;
    persist_document_atomically(path, document, &bytes)
}

#[cfg(not(target_os = "windows"))]
fn persist_document_atomically(
    path: &Path,
    document: &ProjectDocument,
    bytes: &[u8],
) -> Result<(), String> {
    let mut staged = prepare_staged_file(path, document, bytes)?;
    std::fs::rename(&staged.path, path)
        .map_err(|error| format!("failed to commit {} atomically: {error}", path.display()))?;
    staged.committed = true;
    let parent = containing_directory(path)
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to synchronize the project directory for {}: {error}",
                path.display()
            )
        })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn persist_document_atomically(
    path: &Path,
    document: &ProjectDocument,
    bytes: &[u8],
) -> Result<(), String> {
    let mut staged = prepare_staged_file(path, document, bytes)?;
    rename_windows_staged_file(staged.file(), path)?;
    staged.committed = true;
    Ok(())
}

struct StagedFile {
    file: Option<File>,
    path: PathBuf,
    committed: bool,
}

impl StagedFile {
    fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("a staged file handle remains present until drop")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("a staged file handle remains present until drop")
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        // Windows sharing deliberately denies deletion while this handle is
        // open. Closing first is harmless and makes cleanup consistent on all
        // platforms.
        drop(self.file.take());
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn prepare_staged_file(
    path: &Path,
    document: &ProjectDocument,
    bytes: &[u8],
) -> Result<StagedFile, String> {
    let mut staged = create_staged_file(path)?;
    staged.file_mut().write_all(bytes).map_err(|error| {
        format!(
            "failed to write staged project data for {}: {error}",
            path.display()
        )
    })?;
    staged.file_mut().sync_all().map_err(|error| {
        format!(
            "failed to synchronize staged project data for {}: {error}",
            path.display()
        )
    })?;

    // Re-read the staged file through the same handle before its same-directory
    // rename. Windows additionally denies writer/delete sharing for the life
    // of this handle.
    staged
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|error| {
            format!(
                "failed to rewind staged project data for {}: {error}",
                path.display()
            )
        })?;
    let mut staged_bytes = Vec::with_capacity(bytes.len());
    staged
        .file_mut()
        .read_to_end(&mut staged_bytes)
        .map_err(|error| {
            format!(
                "failed to verify staged project data for {}: {error}",
                path.display()
            )
        })?;
    if staged_bytes != bytes {
        return Err(format!(
            "staged project data for {} changed before commit",
            path.display()
        ));
    }
    verify_generated_ori2(document, &staged_bytes)?;
    Ok(staged)
}

fn create_staged_file(path: &Path) -> Result<StagedFile, String> {
    let parent = containing_directory(path)
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;
    path.file_name()
        .ok_or_else(|| format!("{} is not a file path", path.display()))?;

    for _ in 0..128 {
        let id = NEXT_STAGED_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let mut staged_name = OsString::from(".origami2-");
        staged_name.push(format!("{}-{id}.tmp", std::process::id()));
        let staged_path = parent.join(staged_name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(target_os = "windows")]
        options
            .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
            .share_mode(FILE_SHARE_READ);
        match options.open(&staged_path) {
            Ok(file) => {
                let staged = StagedFile {
                    file: Some(file),
                    path: staged_path,
                    committed: false,
                };
                #[cfg(not(target_os = "windows"))]
                match std::fs::metadata(path) {
                    Ok(metadata) if metadata.is_file() => staged
                        .file()
                        .set_permissions(metadata.permissions())
                        .map_err(|error| {
                            format!(
                                "failed to preserve permissions for {}: {error}",
                                path.display()
                            )
                        })?,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "failed to inspect permissions for {}: {error}",
                            path.display()
                        ));
                    }
                }
                return Ok(staged);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to prepare atomic save for {}: {error}",
                    path.display()
                ));
            }
        }
    }

    Err(format!(
        "failed to prepare atomic save for {}: could not allocate a unique staged file",
        path.display()
    ))
}

fn containing_directory(path: &Path) -> Option<&Path> {
    path.parent().map(|parent| {
        if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        }
    })
}

#[cfg(target_os = "windows")]
fn rename_windows_staged_file(staged_file: &File, destination: &Path) -> Result<(), String> {
    let destination_wide = destination.as_os_str().encode_wide().collect::<Vec<_>>();
    if destination_wide.contains(&0) {
        return Err(format!(
            "failed to commit {} atomically: the path contains a NUL character",
            destination.display()
        ));
    }

    let file_name_bytes = destination_wide
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| {
            format!(
                "failed to commit {} atomically: the path is too long",
                destination.display()
            )
        })?;
    let buffer_size = size_of::<FILE_RENAME_INFO>()
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| {
            format!(
                "failed to commit {} atomically: the rename request is too large",
                destination.display()
            )
        })?;
    let buffer_size_u32 = u32::try_from(buffer_size).map_err(|_| {
        format!(
            "failed to commit {} atomically: the rename request is too large",
            destination.display()
        )
    })?;
    let word_size = size_of::<usize>();
    let word_count = buffer_size
        .checked_add(word_size - 1)
        .map(|length| length / word_size)
        .ok_or_else(|| {
            format!(
                "failed to commit {} atomically: the rename request is too large",
                destination.display()
            )
        })?;
    let mut buffer = vec![0usize; word_count];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();

    // SAFETY: `buffer` is usize-aligned and large enough for the fixed header,
    // destination UTF-16 units, and a trailing NUL. The handle remains owned
    // by `staged_file` throughout the call. FileRenameInfo renames that exact
    // open file, so a pathname swap cannot substitute unverified bytes.
    let renamed = unsafe {
        (*info).Anonymous.ReplaceIfExists = true;
        (*info).RootDirectory = ptr::null_mut();
        (*info).FileNameLength = file_name_bytes;
        let file_name = ptr::addr_of_mut!((*info).FileName).cast::<u16>();
        ptr::copy_nonoverlapping(destination_wide.as_ptr(), file_name, destination_wide.len());
        SetFileInformationByHandle(
            staged_file.as_raw_handle() as RawHandle,
            FileRenameInfo,
            info.cast(),
            buffer_size_u32,
        )
    };
    if renamed == 0 {
        return Err(format!(
            "failed to commit {} atomically: {}",
            destination.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
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
    let paper_validation = validate_paper(project.editor.paper(), project.editor.pattern());
    let local_flat_foldability =
        analyze_local_flat_foldability(project.editor.paper(), project.editor.pattern());
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
        local_flat_foldability,
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
            vertices: unique_vertex_ids(project.editor.paper().boundary_vertices.iter().copied()),
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
        .setup(|app| {
            app.manage(DiagnosticsState::from_app_handle(app.handle()));
            Ok(())
        })
        .manage(AppState(Mutex::new(initial_project_state())))
        .manage(ExitGuard::default())
        .invoke_handler(tauri::generate_handler![
            generate_benchmark_pattern,
            project_snapshot,
            new_project,
            validate_project,
            analyze_project_topology,
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
            add_instruction_step,
            update_instruction_step_metadata,
            replace_instruction_step_pose,
            remove_instruction_step,
            move_instruction_step,
            set_cutting_allowed,
            update_paper_properties,
            resize_rectangular_paper,
            split_edge,
            connect_edge_intersection,
            connect_intersection_cluster,
            connect_t_junction,
            split_boundary_edge,
            remove_boundary_vertex,
            record_unexpected_diagnostic,
            prepare_diagnostics_share_preview,
            save_diagnostics_share_preview
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
    use std::{
        collections::BTreeSet,
        fs,
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use ori_domain::{Edge, Vertex};

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow the Unix epoch")
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-native-file-tests-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated native-file test directory");
            Self { path }
        }

        fn join(&self, name: impl AsRef<Path>) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn new_project_parameters() -> NewProjectParameters {
        NewProjectParameters {
            name: "  Test sheet  ".to_owned(),
            width_mm: 210.0,
            height_mm: 297.0,
            thickness_mm: 0.2,
            cutting_allowed: true,
            front_color: RgbaColor {
                red: 10,
                green: 20,
                blue: 30,
                alpha: 240,
            },
            back_color: RgbaColor {
                red: 220,
                green: 210,
                blue: 200,
                alpha: 230,
            },
        }
    }

    fn cellular_multi_fold_project_state() -> ProjectState {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(8.0, 0.0),
            Point2::new(8.0, 6.0),
            Point2::new(6.0, 6.0),
            Point2::new(2.0, 6.0),
            Point2::new(0.0, 6.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([
            Edge {
                id: EdgeId::new(),
                start: vertices[1].id,
                end: vertices[6].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: vertices[2].id,
                end: vertices[5].id,
                kind: EdgeKind::Valley,
            },
        ]);
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper)
    }

    fn four_ray_square_project_state(
        fold_endpoint_indices: [usize; 4],
        assignments: [EdgeKind; 4],
    ) -> (ProjectState, VertexId) {
        let boundary_positions = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(20.0, 0.0),
            Point2::new(20.0, 10.0),
            Point2::new(20.0, 20.0),
            Point2::new(10.0, 20.0),
            Point2::new(0.0, 20.0),
            Point2::new(0.0, 10.0),
        ];
        let mut vertices = boundary_positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let center = Vertex {
            id: VertexId::new(),
            position: Point2::new(10.0, 10.0),
        };
        let center_id = center.id;
        vertices.push(center);

        let mut edges = (0..boundary_positions.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % boundary_positions.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend(
            fold_endpoint_indices
                .into_iter()
                .zip(assignments)
                .map(|(endpoint, kind)| Edge {
                    id: EdgeId::new(),
                    start: center_id,
                    end: vertices[endpoint].id,
                    kind,
                }),
        );
        let paper = Paper {
            boundary_vertices: vertices[..boundary_positions.len()]
                .iter()
                .map(|vertex| vertex.id)
                .collect(),
            ..Paper::default()
        };
        (
            ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper),
            center_id,
        )
    }

    #[derive(Debug, PartialEq)]
    struct ProjectStateSignature {
        instance_id: ProjectId,
        project_id: ProjectId,
        document: ProjectDocument,
        current_path: Option<PathBuf>,
        saved_revision: Option<u64>,
        saved_document: Option<ProjectDocument>,
        revision: u64,
        can_undo: bool,
        can_redo: bool,
        is_dirty: bool,
    }

    fn project_state_signature(project: &ProjectState) -> ProjectStateSignature {
        ProjectStateSignature {
            instance_id: project.instance_id,
            project_id: project.project_id,
            document: project.document(),
            current_path: project.current_path.clone(),
            saved_revision: project.saved_revision,
            saved_document: project.saved_document.clone(),
            revision: project.editor.revision(),
            can_undo: project.editor.can_undo(),
            can_redo: project.editor.can_redo(),
            is_dirty: project.is_dirty(),
        }
    }

    #[test]
    fn topology_bridge_returns_revision_bound_boundary_snapshot_without_mutation() {
        let project = initial_project_state();
        let before = project_state_signature(&project);
        let input =
            capture_topology_input(&project, project.project_id, 0).expect("capture initial sheet");
        let topology = input.analyze();

        let response =
            finish_topology_response(&project, &input, topology).expect("finish current topology");

        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.revision, 0);
        assert!(response.simulation_ready);
        assert!(response.issues.is_empty());
        let snapshot = response.snapshot.expect("boundary snapshot");
        assert_eq!(snapshot.source_revision, response.revision);
        assert_eq!(snapshot.faces.len(), 1);
        assert!(snapshot.hinge_adjacency.is_empty());
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn topology_bridge_returns_two_faces_and_one_hinge_for_one_fold() {
        let mut project = initial_project_state();
        let fold = EdgeId::new();
        let endpoints = [
            project.editor.paper().boundary_vertices[0],
            project.editor.paper().boundary_vertices[2],
        ];
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddEdge {
                id: fold,
                start: endpoints[0],
                end: endpoints[1],
                kind: EdgeKind::Mountain,
            },
        )
        .expect("add one fold");
        let before = project_state_signature(&project);
        let input = capture_topology_input(&project, project_id, 1).expect("capture fold");

        let response = finish_topology_response(&project, &input, input.analyze())
            .expect("finish fold topology");

        assert!(response.simulation_ready);
        assert!(response.issues.is_empty());
        let snapshot = response.snapshot.expect("fold snapshot");
        assert_eq!(snapshot.source_revision, 1);
        assert_eq!(snapshot.faces.len(), 2);
        assert_eq!(snapshot.hinge_adjacency.len(), 1);
        assert_eq!(snapshot.hinge_adjacency[0].edge, fold);
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn instruction_pose_accepts_planar_and_complete_tree_models() {
        let project = initial_project_state();
        let input = capture_topology_input(&project, project.project_id, 0)
            .expect("capture planar instruction model");
        let topology = input.analyze();
        let planar = instruction_pose_from_topology(
            topology
                .simulation_snapshot()
                .expect("planar topology must be simulation-ready"),
            "0".repeat(64),
            None,
            Vec::new(),
        )
        .expect("accept planar instruction pose");
        assert_eq!(planar.fixed_face, None);
        assert!(planar.hinge_angles.is_empty());

        let mut folded = initial_project_state();
        let fold = EdgeId::new();
        let boundary = folded.editor.paper().boundary_vertices.clone();
        let project_id = folded.project_id;
        execute_command(
            &mut folded,
            project_id,
            0,
            Command::AddEdge {
                id: fold,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Mountain,
            },
        )
        .expect("add one instruction hinge");
        let input = capture_topology_input(&folded, project_id, 1).expect("capture fold model");
        let topology = input.analyze();
        let snapshot = topology
            .simulation_snapshot()
            .expect("one-fold topology must be simulation-ready");
        let fixed_face = snapshot.faces[0].id;
        let pose = instruction_pose_from_topology(
            snapshot,
            folded.editor.fold_model_fingerprint_v1(),
            Some(fixed_face),
            vec![InstructionHingeAngle {
                edge: fold,
                angle_degrees: 37.5,
            }],
        )
        .expect("accept complete one-fold instruction pose");

        assert_eq!(pose.fixed_face, Some(fixed_face));
        assert_eq!(pose.hinge_angles.len(), 1);
        assert_eq!(pose.hinge_angles[0].edge, fold);
        assert_eq!(pose.hinge_angles[0].angle_degrees, 37.5);
        assert_eq!(
            pose.source_model_fingerprint,
            folded.editor.fold_model_fingerprint_v1()
        );
    }

    #[test]
    fn instruction_pose_rejects_wrong_faces_incomplete_hinges_and_bad_angles() {
        let mut project = initial_project_state();
        let fold = EdgeId::new();
        let boundary = project.editor.paper().boundary_vertices.clone();
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddEdge {
                id: fold,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Valley,
            },
        )
        .expect("add one instruction hinge");
        let input = capture_topology_input(&project, project_id, 1).expect("capture fold model");
        let topology = input.analyze();
        let snapshot = topology
            .simulation_snapshot()
            .expect("one-fold topology must be simulation-ready");
        let fingerprint = project.editor.fold_model_fingerprint_v1();

        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint.clone(),
                None,
                vec![InstructionHingeAngle {
                    edge: fold,
                    angle_degrees: 45.0,
                }],
            )
            .expect_err("a folded pose needs a fixed face"),
            "a folded instruction pose requires a fixed face"
        );
        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint.clone(),
                Some(FaceId::new()),
                vec![InstructionHingeAngle {
                    edge: fold,
                    angle_degrees: 45.0,
                }],
            )
            .expect_err("the fixed face must be current"),
            "the fixed face does not exist in the current fold model"
        );
        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint.clone(),
                Some(snapshot.faces[0].id),
                Vec::new(),
            )
            .expect_err("every hinge is required"),
            "the instruction pose must contain every current hinge exactly once"
        );
        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint,
                Some(snapshot.faces[0].id),
                vec![InstructionHingeAngle {
                    edge: fold,
                    angle_degrees: f64::NAN,
                }],
            )
            .expect_err("non-finite angles are rejected"),
            "instruction hinge angles must be finite"
        );
    }

    #[test]
    fn instruction_pose_rejects_fold_graph_cycles() {
        let (project, _) = four_ray_square_project_state(
            [1, 3, 5, 7],
            [
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let input = capture_topology_input(&project, project.project_id, 0)
            .expect("capture cyclic fold model");
        let topology = input.analyze();
        let snapshot = topology
            .simulation_snapshot()
            .expect("the topology layer admits the cyclic model");
        let hinge_angles = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| InstructionHingeAngle {
                edge: hinge.edge,
                angle_degrees: 0.0,
            })
            .collect();

        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                project.editor.fold_model_fingerprint_v1(),
                Some(snapshot.faces[0].id),
                hinge_angles,
            )
            .expect_err("the first instruction player supports trees only"),
            "instruction poses currently require a tree-shaped fold graph"
        );
    }

    #[test]
    fn instruction_step_updates_snapshot_document_dirty_state_and_history() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        let step_id = InstructionStepId::new();
        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::AddInstructionStep {
                step: InstructionStep {
                    id: step_id,
                    title: "折る前".to_owned(),
                    description: "平らな開始姿勢".to_owned(),
                    caution: String::new(),
                    duration_ms: 1_500,
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: fingerprint.clone(),
                        fixed_face: None,
                        hinge_angles: Vec::new(),
                    },
                },
            },
        )
        .expect("add planar instruction step");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert_eq!(response.fold_model_fingerprint, fingerprint);
        assert_eq!(response.instruction_timeline.steps.len(), 1);
        assert_eq!(response.instruction_timeline.steps[0].id, step_id);
        assert_eq!(
            project.document().instruction_timeline,
            response.instruction_timeline
        );

        let bytes = write_project_ori2(&project.document()).expect("persist instruction timeline");
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default())
            .expect("restore instruction timeline");
        assert_eq!(
            restored.instruction_timeline,
            project.document().instruction_timeline
        );

        project.editor.undo(1).expect("undo instruction addition");
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(!project.is_dirty());
        project.editor.redo(2).expect("redo instruction addition");
        assert_eq!(project.editor.instruction_timeline().steps[0].id, step_id);
        assert!(project.is_dirty());
    }

    #[test]
    fn loaded_current_instruction_poses_are_semantically_checked_but_stale_ones_survive() {
        let project = initial_project_state();
        let mut document = project.document();
        let current_fingerprint = project.editor.fold_model_fingerprint_v1();
        let invalid_current_step = InstructionStep {
            id: InstructionStepId::new(),
            title: "不正な現在姿勢".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: 1_000,
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: current_fingerprint,
                fixed_face: Some(FaceId::new()),
                hinge_angles: Vec::new(),
            },
        };
        document
            .instruction_timeline
            .steps
            .push(invalid_current_step.clone());

        assert_eq!(
            validate_document_instruction_poses(&document)
                .expect_err("current malformed pose must fail semantic loading"),
            "instruction step 1 is invalid: a planar instruction pose must not specify a fixed face"
        );

        document.instruction_timeline.steps[0]
            .pose
            .source_model_fingerprint = "f".repeat(64);
        validate_document_instruction_poses(&document)
            .expect("an old-model pose remains loadable as an editable stale step");
    }

    #[test]
    fn delayed_instruction_pose_cannot_land_after_reopening_the_same_document() {
        let project = initial_project_state();
        let project_id = project.project_id;
        let input =
            capture_topology_input(&project, project_id, 0).expect("capture instruction topology");
        let topology = input.analyze();
        let analyzed = AnalyzedInstructionPose {
            project_instance_id: project.instance_id,
            input,
            topology,
            fixed_face: None,
            hinge_angles: Vec::new(),
        };

        let reopened =
            ProjectState::from_document(project.document(), PathBuf::from("same-project.ori2"));
        assert_eq!(reopened.project_id, project_id);
        assert_eq!(reopened.editor.revision(), 0);
        assert_eq!(reopened.editor.pattern(), project.editor.pattern());
        assert_eq!(reopened.editor.paper(), project.editor.paper());
        assert_ne!(reopened.instance_id, project.instance_id);

        assert_eq!(
            finish_instruction_pose(&reopened, project_id, 0, analyzed)
                .expect_err("an old open-instance analysis must not mutate the reopened project"),
            "the open project instance changed while the instruction pose was being analyzed"
        );
    }

    #[test]
    fn semantic_instruction_failure_cannot_overwrite_an_existing_save() {
        let project = initial_project_state();
        let mut document = project.document();
        document.instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: "不正な現在姿勢".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: 1_000,
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
                fixed_face: Some(FaceId::new()),
                hinge_angles: Vec::new(),
            },
        });
        let directory = TestDirectory::new();
        let path = directory.join("existing.ori2");
        let original = b"existing project bytes";
        fs::write(&path, original).expect("create existing target");

        let error = persist_document(&path, &document)
            .expect_err("semantic validation must run before staging a save");

        assert!(error.starts_with("failed to validate folding instructions before save:"));
        assert_eq!(fs::read(&path).expect("read preserved target"), original);
        assert_eq!(
            fs::read_dir(&directory.path)
                .expect("inspect save directory")
                .count(),
            1,
            "semantic rejection must not leave a staged file"
        );
    }

    #[test]
    fn topology_bridge_preserves_three_faces_and_two_hinges_for_multiple_folds() {
        let project = cellular_multi_fold_project_state();
        let before = project_state_signature(&project);
        let input = capture_topology_input(&project, project.project_id, 0)
            .expect("capture cellular fold graph");

        let response = finish_topology_response(&project, &input, input.analyze())
            .expect("finish cellular fold topology");

        assert!(response.simulation_ready);
        assert!(response.issues.is_empty());
        let snapshot = response.snapshot.expect("cellular fold snapshot");
        assert_eq!(snapshot.source_revision, 0);
        assert_eq!(snapshot.faces.len(), 3);
        assert_eq!(snapshot.hinge_adjacency.len(), 2);
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn topology_bridge_preserves_structured_unsupported_diagnostics() {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("cut-enabled sheet");
        let (pattern, paper) = sheet.into_parts();
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let boundary = project.editor.paper().boundary_vertices.clone();
        let cut = EdgeId::new();
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddEdge {
                id: cut,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Cut,
            },
        )
        .expect("add supported editor cut");
        let input =
            capture_topology_input(&project, project_id, 1).expect("capture unsupported graph");

        let response = finish_topology_response(&project, &input, input.analyze())
            .expect("unsupported topology is a diagnostic response");

        assert!(!response.simulation_ready);
        assert!(response.snapshot.is_none());
        assert!(matches!(
            response.issues.as_slice(),
            [TopologyIssue {
                kind: ori_core::TopologyIssueKind::UnsupportedActiveEdge {
                    edge,
                    edge_kind: EdgeKind::Cut,
                },
                ..
            }] if *edge == cut
        ));
    }

    #[test]
    fn topology_bridge_rejects_stale_capture_and_delayed_aba_result() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        assert_eq!(
            capture_topology_input(&project, project_id, 1).expect_err("stale requested revision"),
            "expected revision 1, but the current revision is 0"
        );
        assert!(capture_topology_input(&project, ProjectId::new(), 0).is_err());

        let input = capture_topology_input(&project, project_id, 0).expect("capture old input");
        let topology = input.analyze();
        let replacement =
            create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
        let (pattern, paper) = replacement.into_parts();
        project.editor = EditorState::with_paper(pattern, paper);
        assert_eq!(project.editor.revision(), 0, "ABA revision fixture");

        assert_eq!(
            finish_topology_response(&project, &input, topology)
                .expect_err("same identity/revision with different content is stale"),
            "the project changed while topology was being analyzed"
        );
    }

    fn unsaved_project_with_redo_history(name: &str) -> ProjectState {
        let mut project =
            ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 34.0),
            },
        )
        .expect("add history fixture vertex");
        project.editor.undo(1).expect("create redo history");
        assert!(project.editor.can_redo());
        project
    }

    fn file_document(name: &str, x: f64) -> ProjectDocument {
        ProjectDocument::new(
            name,
            CreasePattern {
                vertices: vec![Vertex {
                    id: VertexId::new(),
                    position: Point2::new(x, 5.0),
                }],
                edges: Vec::new(),
            },
        )
    }

    fn crossing_project() -> (ProjectState, Edge, Edge) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        pattern.vertices.extend([
            Vertex {
                id: ids[0],
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: ids[1],
                position: Point2::new(80.0, 80.0),
            },
            Vertex {
                id: ids[2],
                position: Point2::new(20.0, 80.0),
            },
            Vertex {
                id: ids[3],
                position: Point2::new(80.0, 20.0),
            },
        ]);
        let first = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Mountain,
        };
        let second = Edge {
            id: EdgeId::new(),
            start: ids[2],
            end: ids[3],
            kind: EdgeKind::Valley,
        };
        pattern.edges.extend([first.clone(), second.clone()]);
        (ProjectState::new_with_paper(pattern, paper), first, second)
    }

    fn t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let interior_start = VertexId::new();
        let interior_end = VertexId::new();
        let stem_other = VertexId::new();
        let junction = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: interior_start,
                position: Point2::new(10.0, 40.0),
            },
            Vertex {
                id: interior_end,
                position: Point2::new(90.0, 40.0),
            },
            Vertex {
                id: stem_other,
                position: Point2::new(34.0, 10.0),
            },
            Vertex {
                id: junction,
                position: Point2::new(34.0, 40.0),
            },
        ]);
        let interior = Edge {
            id: EdgeId::new(),
            start: interior_start,
            end: interior_end,
            kind: EdgeKind::Mountain,
        };
        let stem = Edge {
            id: EdgeId::new(),
            start: stem_other,
            end: junction,
            kind: EdgeKind::Valley,
        };
        pattern.edges.extend([interior.clone(), stem.clone()]);
        (
            ProjectState::new_with_paper(pattern, paper),
            interior,
            stem,
            junction,
        )
    }

    fn boundary_t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary = pattern.edges[0].clone();
        let junction = VertexId::new();
        let stem_other = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: junction,
                position: Point2::new(40.0, 0.0),
            },
            Vertex {
                id: stem_other,
                position: Point2::new(40.0, 30.0),
            },
        ]);
        let stem = Edge {
            id: EdgeId::new(),
            start: stem_other,
            end: junction,
            kind: EdgeKind::Mountain,
        };
        pattern.edges.push(stem.clone());
        (
            ProjectState::new_with_paper(pattern, paper),
            boundary,
            stem,
            junction,
        )
    }

    fn append_cluster_test_edge(
        pattern: &mut CreasePattern,
        start_position: Point2,
        end_position: Point2,
        kind: EdgeKind,
    ) -> Edge {
        let start = VertexId::new();
        let end = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: start,
                position: start_position,
            },
            Vertex {
                id: end,
                position: end_position,
            },
        ]);
        let edge = Edge {
            id: EdgeId::new(),
            start,
            end,
            kind,
        };
        pattern.edges.push(edge.clone());
        edge
    }

    fn create_cluster_project(include_omitted_edge: bool) -> (ProjectState, Vec<Edge>) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let mut edges = vec![
            append_cluster_test_edge(
                &mut pattern,
                Point2::new(10.0, 50.0),
                Point2::new(90.0, 50.0),
                EdgeKind::Mountain,
            ),
            append_cluster_test_edge(
                &mut pattern,
                Point2::new(50.0, 10.0),
                Point2::new(50.0, 90.0),
                EdgeKind::Valley,
            ),
            append_cluster_test_edge(
                &mut pattern,
                Point2::new(20.0, 20.0),
                Point2::new(80.0, 80.0),
                EdgeKind::Auxiliary,
            ),
        ];
        if include_omitted_edge {
            edges.push(append_cluster_test_edge(
                &mut pattern,
                Point2::new(20.0, 80.0),
                Point2::new(80.0, 20.0),
                EdgeKind::Mountain,
            ));
        }
        (ProjectState::new_with_paper(pattern, paper), edges)
    }

    fn maximum_cluster_project() -> (ProjectState, Vec<Edge>) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let mut edges = Vec::with_capacity(MAX_INTERSECTION_CLUSTER_TARGETS);
        for index in 0..MAX_INTERSECTION_CLUSTER_TARGETS {
            let offset = index as f64 - 32.0;
            let edge = append_cluster_test_edge(
                &mut pattern,
                Point2::new(10.0, 50.0 - offset),
                Point2::new(90.0, 50.0 + offset),
                match index % 4 {
                    0 => EdgeKind::Mountain,
                    1 => EdgeKind::Valley,
                    2 => EdgeKind::Auxiliary,
                    _ => EdgeKind::Cut,
                },
            );
            edges.push(edge);
        }
        (ProjectState::new_with_paper(pattern, paper), edges)
    }

    fn reuse_cluster_project() -> (ProjectState, [Edge; 3], VertexId) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let horizontal = append_cluster_test_edge(
            &mut pattern,
            Point2::new(10.0, 50.0),
            Point2::new(90.0, 50.0),
            EdgeKind::Mountain,
        );
        let vertical = append_cluster_test_edge(
            &mut pattern,
            Point2::new(50.0, 10.0),
            Point2::new(50.0, 90.0),
            EdgeKind::Valley,
        );
        let junction = VertexId::new();
        let stem_start = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: stem_start,
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: junction,
                position: Point2::new(50.0, 50.0),
            },
        ]);
        let stem = Edge {
            id: EdgeId::new(),
            start: stem_start,
            end: junction,
            kind: EdgeKind::Auxiliary,
        };
        pattern.edges.push(stem.clone());
        (
            ProjectState::new_with_paper(pattern, paper),
            [horizontal, vertical, stem],
            junction,
        )
    }

    #[test]
    fn benchmark_pattern_response_contains_stable_renderable_geometry() {
        let response = generate_benchmark_pattern(4);

        assert_eq!(response.requested_edge_count, 4);
        assert_eq!(response.vertex_count, 4);
        assert_eq!(response.edge_count, 4);
        assert_eq!(
            response.vertices,
            vec![
                BenchmarkVertex {
                    id: "benchmark-v-0".to_owned(),
                    position: Point2::new(0.0, 0.0),
                },
                BenchmarkVertex {
                    id: "benchmark-v-1".to_owned(),
                    position: Point2::new(1.0, 0.0),
                },
                BenchmarkVertex {
                    id: "benchmark-v-2".to_owned(),
                    position: Point2::new(0.0, 1.0),
                },
                BenchmarkVertex {
                    id: "benchmark-v-3".to_owned(),
                    position: Point2::new(1.0, 1.0),
                },
            ]
        );
        assert_eq!(
            response.edges,
            vec![
                BenchmarkEdge {
                    id: "benchmark-e-0".to_owned(),
                    start: "benchmark-v-0".to_owned(),
                    end: "benchmark-v-1".to_owned(),
                    kind: EdgeKind::Mountain,
                },
                BenchmarkEdge {
                    id: "benchmark-e-1".to_owned(),
                    start: "benchmark-v-0".to_owned(),
                    end: "benchmark-v-2".to_owned(),
                    kind: EdgeKind::Valley,
                },
                BenchmarkEdge {
                    id: "benchmark-e-2".to_owned(),
                    start: "benchmark-v-1".to_owned(),
                    end: "benchmark-v-3".to_owned(),
                    kind: EdgeKind::Mountain,
                },
                BenchmarkEdge {
                    id: "benchmark-e-3".to_owned(),
                    start: "benchmark-v-2".to_owned(),
                    end: "benchmark-v-3".to_owned(),
                    kind: EdgeKind::Valley,
                },
            ]
        );
        assert_eq!(generate_benchmark_pattern(4), response);
    }

    #[test]
    fn benchmark_pattern_response_has_all_ten_thousand_edges_and_valid_references() {
        let response = generate_benchmark_pattern(10_000);

        assert_eq!(response.requested_edge_count, 10_000);
        assert_eq!(response.vertex_count, 5_184);
        assert_eq!(response.edge_count, 10_000);
        let vertex_ids = response
            .vertices
            .iter()
            .map(|vertex| vertex.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(response.edges.iter().all(|edge| {
            vertex_ids.contains(edge.start.as_str()) && vertex_ids.contains(edge.end.as_str())
        }));
    }

    #[test]
    fn benchmark_pattern_response_is_empty_for_zero_edges() {
        let response = generate_benchmark_pattern(0);

        assert_eq!(response.requested_edge_count, 0);
        assert_eq!(response.vertex_count, 0);
        assert_eq!(response.edge_count, 0);
        assert!(response.vertices.is_empty());
        assert!(response.edges.is_empty());
    }

    #[test]
    fn project_name_is_trimmed_and_validated_by_unicode_character_count() {
        assert_eq!(normalize_project_name("  Crane  "), Ok("Crane".to_owned()));
        assert_eq!(
            normalize_project_name("\n  Crane  \t"),
            Ok("Crane".to_owned())
        );
        assert!(normalize_project_name("").is_err());
        assert!(normalize_project_name(" \t\n ").is_err());
        assert!(normalize_project_name("Crane\0draft").is_err());

        let maximum = "鶴".repeat(MAX_PROJECT_NAME_CHARS);
        assert_eq!(normalize_project_name(&maximum), Ok(maximum.clone()));
        assert!(normalize_project_name(&format!("{maximum}鶴")).is_err());
    }

    #[test]
    fn paper_thickness_accepts_zero_and_rejects_negative_or_non_finite_values() {
        assert_eq!(validate_paper_thickness(0.0), Ok(()));
        assert_eq!(validate_paper_thickness(-0.0), Ok(()));
        for invalid in [-f64::MIN_POSITIVE, -1.0, f64::NAN, f64::INFINITY] {
            assert!(validate_paper_thickness(invalid).is_err());
        }
    }

    #[test]
    fn new_project_state_has_requested_paper_and_no_saved_baseline() {
        let parameters = new_project_parameters();
        let expected_front = parameters.front_color;
        let expected_back = parameters.back_color;

        let project = create_new_project_state(parameters).expect("valid new project");
        let response = snapshot(&project);

        assert_eq!(project.name, "Test sheet");
        assert!(project.current_path.is_none());
        assert!(project.saved_revision.is_none());
        assert!(project.saved_document.is_none());
        assert_eq!(project.editor.revision(), 0);
        assert!(!project.editor.can_undo());
        assert!(!project.editor.can_redo());
        assert!(project.editor.cutting_allowed());
        assert!(project.is_dirty());
        assert_eq!(project.editor.paper().thickness_mm, 0.2);
        assert_eq!(project.editor.paper().front.color, expected_front);
        assert_eq!(project.editor.paper().back.color, expected_back);
        assert_eq!(project.editor.paper().front.texture_asset, None);
        assert_eq!(project.editor.paper().back.texture_asset, None);
        assert_eq!(
            project.editor.pattern().vertices[2].position,
            Point2::new(210.0, 297.0)
        );
        assert!(validate_paper(project.editor.paper(), project.editor.pattern()).is_valid());

        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.name, "Test sheet");
        assert!(response.current_path.is_none());
        assert_eq!(response.revision, 0);
        assert!(response.saved_revision.is_none());
        assert!(response.is_dirty);
        assert_eq!(&response.paper, project.editor.paper());
        assert!(response.cutting_allowed);
        assert!(!response.can_undo);
        assert!(!response.can_redo);
    }

    #[test]
    fn snapshot_paper_uses_the_current_editor_cutting_setting() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        assert!(!project.editor.paper().cutting_allowed);

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::SetCuttingAllowed { allowed: true },
        )
        .expect("enable cutting");

        assert!(response.cutting_allowed);
        assert!(response.paper.cutting_allowed);
        assert!(project.document().paper.cutting_allowed);
    }

    #[test]
    fn paper_properties_follow_undo_redo_dirty_save_and_validation() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original = project.editor.paper().clone();
        let front_color = RgbaColor::opaque(15, 35, 55);
        let back_color = RgbaColor::opaque(205, 185, 165);

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::UpdatePaperProperties {
                thickness_mm: 0.0,
                front_color,
                back_color,
                cutting_allowed: true,
            },
        )
        .expect("update paper properties");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert_eq!(response.paper.thickness_mm, 0.0);
        assert_eq!(response.paper.front.color, front_color);
        assert_eq!(response.paper.back.color, back_color);
        assert!(response.paper.cutting_allowed);
        assert!(validation_snapshot(&project).is_valid);

        project.editor.undo(1).expect("undo properties");
        assert_eq!(project.editor.paper(), &original);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo properties");
        assert!(project.is_dirty());
        let saved_document = project.document();
        project.saved_revision = Some(project.editor.revision());
        project.saved_document = Some(saved_document.clone());
        assert!(!project.is_dirty());
        assert_eq!(project.document(), saved_document);

        project.editor.undo(3).expect("undo after save");
        assert!(project.is_dirty());
        project.editor.redo(4).expect("redo to saved content");
        assert!(!project.is_dirty());
    }

    #[test]
    fn invalid_paper_property_command_preserves_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let conflict = execute_command(
            &mut project,
            project_id,
            1,
            Command::UpdatePaperProperties {
                thickness_mm: 0.3,
                front_color: RgbaColor::opaque(1, 2, 3),
                back_color: RgbaColor::opaque(4, 5, 6),
                cutting_allowed: true,
            },
        )
        .expect_err("stale property update must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let error = execute_command(
            &mut project,
            project_id,
            0,
            Command::UpdatePaperProperties {
                thickness_mm: f64::NAN,
                front_color: RgbaColor::opaque(1, 2, 3),
                back_color: RgbaColor::opaque(4, 5, 6),
                cutting_allowed: true,
            },
        )
        .expect_err("invalid thickness must fail");

        assert_eq!(error, "paper thickness must be finite");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn rectangular_resize_updates_document_dirty_state_and_undo_redo() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = project
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edges = project.editor.pattern().edges.clone();
        let original_paper = project.editor.paper().clone();

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: 210.0,
                height_mm: 297.0,
            },
        )
        .expect("resize paper");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(response.paper, original_paper);
        assert_eq!(
            response
                .crease_pattern
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            original_vertex_ids
        );
        assert_eq!(response.crease_pattern.edges, original_edges);
        assert!(
            response
                .crease_pattern
                .vertices
                .iter()
                .any(|vertex| vertex.position == Point2::new(210.0, 297.0))
        );
        assert!(validation_snapshot(&project).is_valid);
        let resized_document = project.document();
        assert_ne!(resized_document, original_document);
        assert_eq!(resized_document.paper, original_paper);

        project.editor.undo(1).expect("undo resize");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo resize");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), resized_document);
        assert!(project.is_dirty());
    }

    #[test]
    fn same_size_resize_has_history_without_making_the_document_dirty() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: DEFAULT_SHEET_SIZE_MM,
                height_mm: DEFAULT_SHEET_SIZE_MM,
            },
        )
        .expect("same-size resize");

        assert_eq!(response.revision, 1);
        assert!(response.can_undo);
        assert!(!response.is_dirty);
        assert_eq!(project.document(), original_document);
    }

    #[test]
    fn resize_conflicts_invalid_dimensions_and_overflow_preserve_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let conflict = execute_command(
            &mut project,
            project_id,
            1,
            Command::ResizeRectangularPaper {
                width_mm: 210.0,
                height_mm: 297.0,
            },
        )
        .expect_err("stale resize must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let invalid = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: 0.0,
                height_mm: 297.0,
            },
        )
        .expect_err("zero width must fail");
        assert_eq!(invalid, "paper width must be greater than zero");
        assert_eq!(project_state_signature(&project), before);

        let overflow = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: f64::MAX,
                height_mm: 2.0,
            },
        )
        .expect_err("unrepresentable area must fail");
        assert_eq!(
            overflow,
            "target paper area is too large to represent safely"
        );
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn generated_id_edge_split_updates_snapshot_document_and_history() {
        let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
        let (mut pattern, paper) = sheet.into_parts();
        let crease = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Valley,
        };
        pattern.edges.push(crease.clone());
        let original_vertex_ids = pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
        let original_edge_index = pattern.edges.len() - 1;
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let project_id = project.project_id;
        let original_document = project.document();

        let response = execute_edge_split(&mut project, project_id, 0, crease.id, 0.5)
            .expect("split crease edge");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(response.paper, original_document.paper);
        assert_eq!(response.crease_pattern.vertices.len(), 5);
        let generated_vertices = response
            .crease_pattern
            .vertices
            .iter()
            .filter(|vertex| !original_vertex_ids.contains(&vertex.id))
            .collect::<Vec<_>>();
        assert_eq!(generated_vertices.len(), 1);
        let generated_vertex = generated_vertices[0];
        assert_eq!(generated_vertex.position, Point2::new(50.0, 40.0));
        assert_eq!(response.crease_pattern.edges.len(), 6);
        assert_eq!(
            response.crease_pattern.edges[original_edge_index],
            Edge {
                end: generated_vertex.id,
                ..crease.clone()
            }
        );
        let generated_edge = &response.crease_pattern.edges[original_edge_index + 1];
        assert!(!original_edge_ids.contains(&generated_edge.id));
        assert_eq!(generated_edge.start, generated_vertex.id);
        assert_eq!(generated_edge.end, crease.end);
        assert_eq!(generated_edge.kind, EdgeKind::Valley);
        assert!(validation_snapshot(&project).is_valid);
        let split_document = project.document();
        assert_ne!(split_document, original_document);

        project.editor.undo(1).expect("undo edge split");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo edge split");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), split_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn edge_split_conflicts_invalid_fractions_and_boundary_targets_preserve_project_state() {
        let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary_edge = pattern.edges[0].id;
        let crease = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        };
        pattern.edges.push(crease.clone());
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let conflict = execute_edge_split(&mut project, project_id, 1, crease.id, 0.5)
            .expect_err("stale split must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let invalid = execute_edge_split(&mut project, project_id, 0, crease.id, f64::NAN)
            .expect_err("non-finite split must fail");
        assert_eq!(invalid, "edge split fraction must be finite");
        assert_eq!(project_state_signature(&project), before);

        let boundary = execute_edge_split(&mut project, project_id, 0, boundary_edge, 0.5)
            .expect_err("boundary split must use the sheet command");
        assert!(boundary.contains("must be changed through a sheet-boundary operation"));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn edge_intersection_connection_returns_vertex_and_exact_undoable_snapshot() {
        let (mut project, first, second) = crossing_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = original_document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();

        let response =
            execute_edge_intersection_connection(&mut project, project_id, 0, second.id, first.id)
                .expect("connect crossing edges");

        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        let created_vertex = response
            .snapshot
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == response.vertex_id)
            .expect("explicitly returned generated vertex");
        assert_eq!(created_vertex.position, Point2::new(50.0, 50.0));
        assert!(!original_vertex_ids.contains(&response.vertex_id));
        let generated_edges = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .filter(|edge| !original_edge_ids.contains(&edge.id))
            .collect::<Vec<_>>();
        assert_eq!(generated_edges.len(), 2);
        assert!(
            generated_edges
                .iter()
                .all(|edge| edge.start == response.vertex_id)
        );
        assert_eq!(
            generated_edges
                .iter()
                .map(|edge| edge.kind)
                .collect::<Vec<_>>(),
            vec![EdgeKind::Mountain, EdgeKind::Valley]
        );
        assert_eq!(
            response.snapshot.crease_pattern,
            project.editor.pattern().clone()
        );
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo intersection connection");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo intersection connection");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn edge_intersection_api_rejections_preserve_entire_project_state() {
        let (mut project, first, second) = crossing_project();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let wrong_project = execute_edge_intersection_connection(
            &mut project,
            ProjectId::new(),
            0,
            first.id,
            second.id,
        )
        .expect_err("wrong project must fail");
        assert!(wrong_project.contains("active project changed"));
        assert_eq!(project_state_signature(&project), before);

        let stale =
            execute_edge_intersection_connection(&mut project, project_id, 4, first.id, second.id)
                .expect_err("stale revision must fail");
        assert_eq!(stale, "expected revision 4, but the current revision is 0");
        assert_eq!(project_state_signature(&project), before);

        let same_edge =
            execute_edge_intersection_connection(&mut project, project_id, 0, first.id, first.id)
                .expect_err("same target edge must fail");
        assert_eq!(same_edge, "the two intersection edge IDs must be different");
        assert_eq!(project_state_signature(&project), before);

        let boundary = project.editor.pattern().edges[0].id;
        let boundary_error =
            execute_edge_intersection_connection(&mut project, project_id, 0, boundary, first.id)
                .expect_err("boundary target must fail");
        assert!(boundary_error.contains("must not be a boundary edge"));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn edge_intersection_api_rejects_t_junction_without_mutation() {
        let (project, first, second) = crossing_project();
        let mut document = project.document();
        document
            .crease_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == second.start)
            .expect("second start")
            .position = Point2::new(50.0, 50.0);
        let mut project = ProjectState::new_with_paper(document.crease_pattern, document.paper);
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let error =
            execute_edge_intersection_connection(&mut project, project_id, 0, first.id, second.id)
                .expect_err("T-junction must fail");

        assert_eq!(
            error,
            "the selected edges must intersect strictly inside both edges"
        );
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn intersection_cluster_api_creates_three_way_junction_with_one_step_history() {
        let (mut project, edges) = create_cluster_project(false);
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = original_document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let targets = edges
            .iter()
            .map(|edge| IntersectionClusterTargetRequest {
                edge_id: edge.id,
                relation: IntersectionClusterRelation::Interior,
            })
            .collect();

        let response =
            execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
                .expect("connect a newly created three-edge intersection cluster");

        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(response.snapshot.paper, original_document.paper);
        assert!(!original_vertex_ids.contains(&response.vertex_id));
        assert_eq!(
            response
                .snapshot
                .crease_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == response.vertex_id)
                .expect("created cluster junction")
                .position,
            Point2::new(50.0, 50.0)
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_document.crease_pattern.vertices.len() + 1
        );
        assert_eq!(
            response.snapshot.crease_pattern.edges.len(),
            original_document.crease_pattern.edges.len() + edges.len()
        );
        for edge in &edges {
            let split_original = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| candidate.id == edge.id)
                .expect("split original cluster edge");
            assert_eq!(split_original.start, edge.start);
            assert_eq!(split_original.end, response.vertex_id);
            assert_eq!(split_original.kind, edge.kind);
            let generated = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| {
                    !original_edge_ids.contains(&candidate.id)
                        && candidate.start == response.vertex_id
                        && candidate.end == edge.end
                })
                .expect("generated cluster edge");
            assert_eq!(generated.kind, edge.kind);
        }
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo created intersection cluster");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo created intersection cluster");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn intersection_cluster_api_accepts_64_targets_and_returns_the_created_junction() {
        let (mut project, edges) = maximum_cluster_project();
        assert_eq!(edges.len(), MAX_INTERSECTION_CLUSTER_TARGETS);
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = original_document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let targets = edges
            .iter()
            .map(|edge| IntersectionClusterTargetRequest {
                edge_id: edge.id,
                relation: IntersectionClusterRelation::Interior,
            })
            .collect();

        let response =
            execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
                .expect("the inclusive 64-target API limit must connect");

        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert!(!original_vertex_ids.contains(&response.vertex_id));
        assert_eq!(
            response
                .snapshot
                .crease_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == response.vertex_id),
            Some(&Vertex {
                id: response.vertex_id,
                position: Point2::new(50.0, 50.0),
            })
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_document.crease_pattern.vertices.len() + 1
        );
        assert_eq!(
            response.snapshot.crease_pattern.edges.len(),
            original_document.crease_pattern.edges.len() + MAX_INTERSECTION_CLUSTER_TARGETS
        );
        for source in &edges {
            let split_original = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|edge| edge.id == source.id)
                .expect("each maximum-cluster source edge remains");
            assert_eq!(split_original.start, source.start);
            assert_eq!(split_original.end, response.vertex_id);
            assert_eq!(split_original.kind, source.kind);
            let generated = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|edge| {
                    !edges.iter().any(|source| source.id == edge.id)
                        && edge.start == response.vertex_id
                        && edge.end == source.end
                })
                .expect("each maximum-cluster source gets one generated half");
            assert_eq!(generated.kind, source.kind);
        }
        assert!(validation_snapshot(&project).is_valid);

        let (mut rejected_project, rejected_edges) = maximum_cluster_project();
        let rejected_project_id = rejected_project.project_id;
        let rejected_before = project_state_signature(&rejected_project);
        let error = execute_intersection_cluster_connection(
            &mut rejected_project,
            rejected_project_id,
            0,
            (0..=MAX_INTERSECTION_CLUSTER_TARGETS)
                .map(|index| IntersectionClusterTargetRequest {
                    edge_id: rejected_edges[index % rejected_edges.len()].id,
                    relation: IntersectionClusterRelation::Interior,
                })
                .collect(),
            None,
        )
        .expect_err("65 targets must be rejected at the API boundary");
        assert_eq!(
            error,
            "an intersection cluster supports at most 64 target edges, found 65"
        );
        assert_eq!(project_state_signature(&rejected_project), rejected_before);
    }

    #[test]
    fn intersection_cluster_api_reuses_junction_with_interior_and_endpoint_targets() {
        let (mut project, [horizontal, vertical, stem], junction) = reuse_cluster_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let targets = vec![
            IntersectionClusterTargetRequest {
                edge_id: stem.id,
                relation: IntersectionClusterRelation::Endpoint,
            },
            IntersectionClusterTargetRequest {
                edge_id: vertical.id,
                relation: IntersectionClusterRelation::Interior,
            },
            IntersectionClusterTargetRequest {
                edge_id: horizontal.id,
                relation: IntersectionClusterRelation::Interior,
            },
        ];

        let response = execute_intersection_cluster_connection(
            &mut project,
            project_id,
            0,
            targets,
            Some(junction),
        )
        .expect("connect a mixed interior/endpoint cluster at an existing vertex");

        assert_eq!(response.vertex_id, junction);
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(
            response.snapshot.crease_pattern.vertices,
            original_document.crease_pattern.vertices
        );
        assert_eq!(
            response.snapshot.crease_pattern.edges.len(),
            original_document.crease_pattern.edges.len() + 2
        );
        assert!(
            response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge == &stem)
        );
        for edge in [&horizontal, &vertical] {
            let split_original = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| candidate.id == edge.id)
                .expect("split original cluster edge");
            assert_eq!(split_original.start, edge.start);
            assert_eq!(split_original.end, junction);
            assert_eq!(split_original.kind, edge.kind);
            let generated = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| {
                    !original_edge_ids.contains(&candidate.id)
                        && candidate.start == junction
                        && candidate.end == edge.end
                })
                .expect("generated cluster edge");
            assert_eq!(generated.kind, edge.kind);
        }
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo reused intersection cluster");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo reused intersection cluster");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn intersection_cluster_api_rejections_are_atomic_and_boundary_remains_unsupported() {
        let interior_target = |edge: &Edge| IntersectionClusterTargetRequest {
            edge_id: edge.id,
            relation: IntersectionClusterRelation::Interior,
        };

        let (mut bounded_project, bounded_edges) = create_cluster_project(false);
        let bounded_project_id = bounded_project.project_id;
        let bounded_before = project_state_signature(&bounded_project);
        let too_few_error = execute_intersection_cluster_connection(
            &mut bounded_project,
            bounded_project_id,
            0,
            bounded_edges[..2].iter().map(interior_target).collect(),
            None,
        )
        .expect_err("fewer than three request targets must fail before ID allocation");
        assert_eq!(
            too_few_error,
            "an intersection cluster requires at least three target edges, found 2"
        );
        let too_many_error = execute_intersection_cluster_connection(
            &mut bounded_project,
            bounded_project_id,
            0,
            (0..65)
                .map(|_| interior_target(&bounded_edges[0]))
                .collect(),
            None,
        )
        .expect_err("more than 64 request targets must fail before ID allocation");
        assert_eq!(
            too_many_error,
            "an intersection cluster supports at most 64 target edges, found 65"
        );
        assert_eq!(project_state_signature(&bounded_project), bounded_before);

        let (mut stale_project, stale_edges) = create_cluster_project(false);
        let stale_project_id = stale_project.project_id;
        let stale_before = project_state_signature(&stale_project);
        let stale_error = execute_intersection_cluster_connection(
            &mut stale_project,
            stale_project_id,
            1,
            stale_edges.iter().map(interior_target).collect(),
            None,
        )
        .expect_err("stale cluster command must fail");
        assert_eq!(
            stale_error,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&stale_project), stale_before);

        let (mut incomplete_project, incomplete_edges) = create_cluster_project(true);
        let incomplete_project_id = incomplete_project.project_id;
        let incomplete_before = project_state_signature(&incomplete_project);
        let incomplete_error = execute_intersection_cluster_connection(
            &mut incomplete_project,
            incomplete_project_id,
            0,
            incomplete_edges[..3].iter().map(interior_target).collect(),
            None,
        )
        .expect_err("an omitted intersecting edge must reject the whole cluster");
        assert!(incomplete_error.contains("also passes through the intersection cluster"));
        assert!(incomplete_error.contains(&format!("{:?}", incomplete_edges[3].id)));
        assert_eq!(
            project_state_signature(&incomplete_project),
            incomplete_before
        );

        let (mut boundary_project, boundary_edges) = create_cluster_project(false);
        let boundary_project_id = boundary_project.project_id;
        let boundary_before = project_state_signature(&boundary_project);
        let boundary = boundary_project.editor.pattern().edges[0].clone();
        let boundary_error = execute_intersection_cluster_connection(
            &mut boundary_project,
            boundary_project_id,
            0,
            vec![
                interior_target(&boundary),
                interior_target(&boundary_edges[1]),
                interior_target(&boundary_edges[2]),
            ],
            None,
        )
        .expect_err("boundary clusters remain unsupported in the first core increment");
        assert!(boundary_error.contains("does not yet support boundary edge"));
        assert_eq!(project_state_signature(&boundary_project), boundary_before);
    }

    #[test]
    fn t_junction_connection_returns_reused_vertex_and_undoable_dirty_snapshot() {
        let (mut project, interior, stem, junction) = t_junction_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_count = original_document.crease_pattern.vertices.len();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();

        let response =
            execute_t_junction_connection(&mut project, project_id, 0, stem.id, interior.id)
                .expect("connect T-junction with reverse arguments");

        assert_eq!(response.vertex_id, junction);
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_vertex_count
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices,
            original_document.crease_pattern.vertices
        );
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| edge.id == interior.id)
            .expect("split original edge");
        assert_eq!(split_original.start, interior.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, EdgeKind::Mountain);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| !original_edge_ids.contains(&edge.id))
            .expect("generated T-junction edge");
        assert_eq!(generated.start, junction);
        assert_eq!(generated.end, interior.end);
        assert_eq!(generated.kind, EdgeKind::Mountain);
        assert!(
            response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge == &stem)
        );
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project.editor.undo(1).expect("undo T-junction connection");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo T-junction connection");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn boundary_t_junction_api_splits_sheet_outline_with_reused_vertex_and_exact_history() {
        let (mut project, boundary, stem, junction) = boundary_t_junction_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_count = original_document.crease_pattern.vertices.len();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let original_boundary_vertices = original_document.paper.boundary_vertices.clone();

        let response =
            execute_t_junction_connection(&mut project, project_id, 0, stem.id, boundary.id)
                .expect("connect a crease endpoint to the strict interior of the sheet boundary");

        assert_eq!(response.vertex_id, junction);
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_vertex_count
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices,
            original_document.crease_pattern.vertices
        );
        assert_eq!(
            response.snapshot.paper.boundary_vertices,
            vec![
                original_boundary_vertices[0],
                junction,
                original_boundary_vertices[1],
                original_boundary_vertices[2],
                original_boundary_vertices[3],
            ]
        );

        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| edge.id == boundary.id)
            .expect("original boundary segment");
        assert_eq!(split_original.start, boundary.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, EdgeKind::Boundary);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| !original_edge_ids.contains(&edge.id))
            .expect("generated boundary segment");
        assert_eq!(generated.start, junction);
        assert_eq!(generated.end, boundary.end);
        assert_eq!(generated.kind, EdgeKind::Boundary);
        assert!(
            response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge == &stem)
        );
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo boundary T-junction connection");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo boundary T-junction connection");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn t_junction_api_conflicts_and_wrong_geometry_preserve_project_state() {
        let (mut project, interior, stem, _) = t_junction_project();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let wrong_project =
            execute_t_junction_connection(&mut project, ProjectId::new(), 0, interior.id, stem.id)
                .expect_err("wrong project must fail");
        assert!(wrong_project.contains("active project changed"));
        assert_eq!(project_state_signature(&project), before);

        let stale =
            execute_t_junction_connection(&mut project, project_id, 3, interior.id, stem.id)
                .expect_err("stale revision must fail");
        assert_eq!(stale, "expected revision 3, but the current revision is 0");
        assert_eq!(project_state_signature(&project), before);

        let boundary = project.editor.pattern().edges[0].id;
        let boundary_error =
            execute_t_junction_connection(&mut project, project_id, 0, boundary, interior.id)
                .expect_err("non-intersecting boundary target must fail");
        assert_eq!(
            boundary_error,
            "the selected edges do not form exactly one strict T-junction"
        );
        assert_eq!(project_state_signature(&project), before);

        let (mut crossing, first, second) = crossing_project();
        let crossing_project_id = crossing.project_id;
        let crossing_before = project_state_signature(&crossing);
        let proper_x = execute_t_junction_connection(
            &mut crossing,
            crossing_project_id,
            0,
            first.id,
            second.id,
        )
        .expect_err("proper X must not be accepted as T-junction");
        assert_eq!(
            proper_x,
            "the selected edges do not form exactly one strict T-junction"
        );
        assert_eq!(project_state_signature(&crossing), crossing_before);
    }

    #[test]
    fn generated_id_boundary_split_handles_reverse_closing_edge_and_document_history() {
        let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
        let (mut pattern, paper) = sheet.into_parts();
        let forward_closing_edge = pattern.edges[3].clone();
        pattern.edges[3] = Edge {
            start: forward_closing_edge.end,
            end: forward_closing_edge.start,
            ..forward_closing_edge
        };
        let target_edge = pattern.edges[3].clone();
        let original_vertex_ids = pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let project_id = project.project_id;
        let original_document = project.document();

        let response = execute_boundary_split(&mut project, project_id, 0, target_edge.id, 0.25)
            .expect("split reverse closing edge");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(response.paper.boundary_vertices.len(), 5);
        let new_vertex = response.paper.boundary_vertices[4];
        assert!(!original_vertex_ids.contains(&new_vertex));
        assert_eq!(response.crease_pattern.vertices.len(), 5);
        assert_eq!(
            response.crease_pattern.vertices[4],
            Vertex {
                id: new_vertex,
                position: Point2::new(0.0, 20.0),
            }
        );
        assert_eq!(response.crease_pattern.edges.len(), 5);
        assert_eq!(response.crease_pattern.edges[3].id, target_edge.id);
        assert_eq!(response.crease_pattern.edges[3].start, target_edge.start);
        assert_eq!(response.crease_pattern.edges[3].end, new_vertex);
        let generated_edge = &response.crease_pattern.edges[4];
        assert!(!original_edge_ids.contains(&generated_edge.id));
        assert_eq!(generated_edge.start, new_vertex);
        assert_eq!(generated_edge.end, target_edge.end);
        assert_eq!(generated_edge.kind, EdgeKind::Boundary);
        assert!(validation_snapshot(&project).is_valid);
        let split_document = project.document();
        assert_ne!(split_document, original_document);

        project.editor.undo(1).expect("undo boundary split");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo boundary split");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), split_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn boundary_split_conflict_and_invalid_fraction_preserve_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let edge = project.editor.pattern().edges[0].id;
        let before = project_state_signature(&project);

        let conflict = execute_boundary_split(&mut project, project_id, 1, edge, 0.5)
            .expect_err("stale split must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let invalid = execute_boundary_split(&mut project, project_id, 0, edge, f64::NAN)
            .expect_err("non-finite split must fail");
        assert_eq!(invalid, "boundary split fraction must be finite");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn boundary_vertex_removal_updates_document_dirty_state_and_history() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();
        let target = project.editor.paper().boundary_vertices[1];
        let previous = project.editor.paper().boundary_vertices[0];
        let next = project.editor.paper().boundary_vertices[2];
        let remaining = project.editor.paper().boundary_vertices[3];
        let kept_edge = project.editor.pattern().edges[0].clone();
        let removed_edge = project.editor.pattern().edges[1].clone();

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::RemoveBoundaryVertex { vertex: target },
        )
        .expect("remove boundary vertex");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(
            response.paper.boundary_vertices,
            vec![previous, next, remaining]
        );
        assert!(
            !response
                .crease_pattern
                .vertices
                .iter()
                .any(|vertex| vertex.id == target)
        );
        assert_eq!(response.crease_pattern.edges[0].id, kept_edge.id);
        assert_eq!(response.crease_pattern.edges[0].start, previous);
        assert_eq!(response.crease_pattern.edges[0].end, next);
        assert!(
            !response
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge.id == removed_edge.id)
        );
        assert!(validation_snapshot(&project).is_valid);
        let removed_document = project.document();
        assert_ne!(removed_document, original_document);

        project.editor.undo(1).expect("undo boundary removal");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo boundary removal");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), removed_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn boundary_vertex_removal_conflict_preserves_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let target = project.editor.paper().boundary_vertices[1];
        let before = project_state_signature(&project);

        let error = execute_command(
            &mut project,
            project_id,
            1,
            Command::RemoveBoundaryVertex { vertex: target },
        )
        .expect_err("stale boundary removal must fail");

        assert_eq!(error, "expected revision 1, but the current revision is 0");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn new_project_replaces_only_the_expected_unchanged_project() {
        let mut project = initial_project_state();
        let old_project_id = project.project_id;

        let response =
            replace_with_new_project(&mut project, old_project_id, 0, new_project_parameters())
                .expect("replace current project");

        assert_ne!(response.project_id, old_project_id);
        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.name, "Test sheet");
        assert!(response.current_path.is_none());
        assert_eq!(response.revision, 0);
        assert!(response.saved_revision.is_none());
        assert!(response.is_dirty);
        assert!(!response.can_undo);
        assert!(!response.can_redo);
        assert!(project.saved_document.is_none());
    }

    #[test]
    fn new_project_errors_leave_existing_state_untouched() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        assert!(
            replace_with_new_project(&mut project, ProjectId::new(), 0, new_project_parameters(),)
                .is_err()
        );
        assert_eq!(project_state_signature(&project), before);

        assert!(
            replace_with_new_project(&mut project, project_id, 1, new_project_parameters())
                .is_err()
        );
        assert_eq!(project_state_signature(&project), before);

        let mut invalid_name = new_project_parameters();
        invalid_name.name = " \0 ".to_owned();
        assert!(replace_with_new_project(&mut project, project_id, 0, invalid_name).is_err());
        assert_eq!(project_state_signature(&project), before);

        let mut invalid_dimensions = new_project_parameters();
        invalid_dimensions.width_mm = 0.0;
        assert!(replace_with_new_project(&mut project, project_id, 0, invalid_dimensions).is_err());
        assert_eq!(project_state_signature(&project), before);

        let mut invalid_thickness = new_project_parameters();
        invalid_thickness.thickness_mm = f64::NAN;
        assert!(replace_with_new_project(&mut project, project_id, 0, invalid_thickness).is_err());
        assert_eq!(project_state_signature(&project), before);
    }

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
        assert_eq!(project.editor.paper().boundary_vertices.len(), 4);
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
    fn initial_sheet_reports_boundary_vertices_as_locally_not_applicable() {
        let project = initial_project_state();

        let response = validation_snapshot(&project);
        let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
        let local = &encoded["local_flat_foldability"];

        assert_eq!(local["model"], "interior_single_vertex_zero_thickness_v1");
        assert_eq!(local["status"], "not_applicable");
        assert_eq!(local["total_vertices"], 4);
        assert_eq!(local["applicable_vertices"], 0);
        assert_eq!(local["not_applicable_vertices"], 4);
        for vertex in local["vertices"].as_array().expect("vertex reports") {
            assert_eq!(vertex["verdict"], "not_applicable");
            assert_eq!(vertex["reason"], "paper_boundary");
            assert_eq!(vertex["kawasaki"], "not_applicable");
            assert_eq!(vertex["maekawa"], "not_applicable");
        }
    }

    #[test]
    fn cardinal_mmmv_vertex_reports_both_local_conditions_satisfied() {
        let (project, center) = four_ray_square_project_state(
            [3, 5, 7, 1],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );

        let response = validation_snapshot(&project);
        let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
        let center = serde_json::to_value(center).expect("serialize center vertex ID");
        let local = encoded["local_flat_foldability"]
            .as_object()
            .expect("local report object");
        let center_report = local["vertices"]
            .as_array()
            .expect("vertex reports")
            .iter()
            .find(|report| report["vertex"] == center)
            .expect("center report");

        assert_eq!(local["status"], "necessary_conditions_satisfied");
        assert_eq!(local["applicable_vertices"], 1);
        assert_eq!(local["satisfied_vertices"], 1);
        assert_eq!(center_report["fold_degree"], 4);
        assert_eq!(center_report["mountain_count"], 3);
        assert_eq!(center_report["valley_count"], 1);
        assert_eq!(center_report["verdict"], "satisfied");
        assert_eq!(center_report["reason"], serde_json::Value::Null);
        assert_eq!(center_report["kawasaki"], "satisfied");
        assert_eq!(center_report["maekawa"], "satisfied");
    }

    #[test]
    fn local_report_keeps_kawasaki_and_maekawa_violations_independent() {
        let (kawasaki_project, kawasaki_center) = four_ray_square_project_state(
            [3, 5, 7, 0],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let (maekawa_project, maekawa_center) = four_ray_square_project_state(
            [3, 5, 7, 1],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Valley,
            ],
        );

        let kawasaki = validation_snapshot(&kawasaki_project);
        let kawasaki_json =
            serde_json::to_value(&kawasaki).expect("serialize Kawasaki counterexample");
        let kawasaki_center =
            serde_json::to_value(kawasaki_center).expect("serialize Kawasaki center vertex ID");
        let kawasaki_center_report = kawasaki_json["local_flat_foldability"]["vertices"]
            .as_array()
            .expect("Kawasaki vertex reports")
            .iter()
            .find(|report| report["vertex"] == kawasaki_center)
            .expect("Kawasaki center report");
        assert_eq!(kawasaki_center_report["kawasaki"], "violated");
        assert_eq!(kawasaki_center_report["maekawa"], "satisfied");
        assert_eq!(kawasaki_center_report["verdict"], "violated");

        let maekawa = validation_snapshot(&maekawa_project);
        let maekawa_json =
            serde_json::to_value(&maekawa).expect("serialize Maekawa counterexample");
        let maekawa_center =
            serde_json::to_value(maekawa_center).expect("serialize Maekawa center vertex ID");
        let maekawa_center_report = maekawa_json["local_flat_foldability"]["vertices"]
            .as_array()
            .expect("Maekawa vertex reports")
            .iter()
            .find(|report| report["vertex"] == maekawa_center)
            .expect("Maekawa center report");
        assert_eq!(maekawa_center_report["kawasaki"], "satisfied");
        assert_eq!(maekawa_center_report["maekawa"], "violated");
        assert_eq!(maekawa_center_report["verdict"], "violated");
    }

    #[test]
    fn local_flat_foldability_json_contract_is_exact_and_does_not_change_geometry_validity() {
        let (project, center) = four_ray_square_project_state(
            [3, 5, 7, 1],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Valley,
            ],
        );

        let response = validation_snapshot(&project);
        assert!(response.is_valid);
        assert!(response.issues.is_empty());
        let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
        let center = serde_json::to_value(center).expect("serialize center vertex ID");
        let root_keys = encoded
            .as_object()
            .expect("validation object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let local = encoded["local_flat_foldability"]
            .as_object()
            .expect("local report object");
        let local_keys = local.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let center_report = local["vertices"]
            .as_array()
            .expect("vertex reports")
            .iter()
            .find(|report| report["vertex"] == center)
            .expect("center report")
            .as_object()
            .expect("center report object");
        let center_keys = center_report
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            root_keys,
            [
                "project_id",
                "revision",
                "is_valid",
                "issues",
                "local_flat_foldability"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            local_keys,
            [
                "model",
                "max_exact_fold_degree",
                "status",
                "total_vertices",
                "applicable_vertices",
                "satisfied_vertices",
                "violated_vertices",
                "not_applicable_vertices",
                "indeterminate_vertices",
                "vertices",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            center_keys,
            [
                "vertex",
                "fold_degree",
                "mountain_count",
                "valley_count",
                "verdict",
                "reason",
                "kawasaki",
                "maekawa",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(local["status"], "violated");
        assert_eq!(center_report["kawasaki"], "satisfied");
        assert_eq!(center_report["maekawa"], "violated");
    }

    #[test]
    fn paper_thickness_issues_are_included_without_highlight_targets() {
        let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
        let (pattern, mut paper) = sheet.into_parts();
        paper.thickness_mm = -0.01;
        let project = ProjectState::new_with_paper(pattern.clone(), paper);

        let response = validation_snapshot(&project);

        assert!(!response.is_valid);
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].code, "negative_thickness");
        assert!(response.issues[0].vertices.is_empty());
        assert!(response.issues[0].edges.is_empty());

        let mut zero_paper = project.editor.paper().clone();
        zero_paper.thickness_mm = 0.0;
        let zero_project = ProjectState::new_with_paper(pattern, zero_paper);
        let zero_thickness = validation_snapshot(&zero_project);
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
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        let project = ProjectState::new_with_paper(pattern, paper);

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
    fn native_save_as_writes_a_loadable_file_and_preserves_editor_history() {
        let directory = TestDirectory::new();
        let selected_path = directory.join("折り紙設計.backup");
        let expected_path = directory.join("折り紙設計.ori2");
        let mut project = unsaved_project_with_redo_history("First project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let document = project.document();
        let can_undo = project.editor.can_undo();
        let can_redo = project.editor.can_redo();

        let response = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            selected_path,
        )
        .expect("save project under a selected path");

        assert!(!response.canceled);
        assert_eq!(
            project.current_path.as_deref(),
            Some(expected_path.as_path())
        );
        assert_eq!(project.saved_revision, Some(expected_revision));
        assert_eq!(project.saved_document.as_ref(), Some(&document));
        assert!(!project.is_dirty());
        assert_eq!(project.editor.revision(), expected_revision);
        assert_eq!(project.editor.can_undo(), can_undo);
        assert_eq!(project.editor.can_redo(), can_redo);
        assert_eq!(load_document_from_path(&expected_path).unwrap(), document);
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[test]
    fn native_save_overwrites_atomically_and_keeps_undo_redo_history() {
        let directory = TestDirectory::new();
        let path = directory.join("overwrite.ori2");
        fs::write(&path, b"pre-existing invalid project").expect("write overwrite sentinel");
        let mut project = unsaved_project_with_redo_history("Overwrite project");

        save_project_to_path(&mut project, path.clone()).expect("replace existing file");
        let first_bytes = fs::read(&path).expect("read first native save");
        let first_document = project.document();
        assert_ne!(first_bytes, b"pre-existing invalid project");
        assert_eq!(load_document_from_path(&path).unwrap(), first_document);
        assert!(project.editor.can_redo());

        let revision_before_redo = project.editor.revision();
        project
            .editor
            .redo(revision_before_redo)
            .expect("restore the saved redo command");
        assert!(project.is_dirty());
        let second_document = project.document();
        let revision_before_save = project.editor.revision();
        let can_undo = project.editor.can_undo();
        let can_redo = project.editor.can_redo();

        save_project_to_path(&mut project, path.clone()).expect("overwrite with edited project");
        let second_bytes = fs::read(&path).expect("read overwritten native save");
        assert_ne!(second_bytes, first_bytes);
        assert_eq!(load_document_from_path(&path).unwrap(), second_document);
        assert_eq!(project.editor.revision(), revision_before_save);
        assert_eq!(project.editor.can_undo(), can_undo);
        assert_eq!(project.editor.can_redo(), can_redo);
        assert!(!project.is_dirty());
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_staged_save_denies_concurrent_writers_and_cleans_up() {
        let directory = TestDirectory::new();
        let path = directory.join("writer-sharing.ori2");
        let staged = create_staged_file(&path).expect("create protected staged file");

        let writer_error = OpenOptions::new()
            .write(true)
            .open(&staged.path)
            .expect_err("a concurrent writer must be denied while staging");
        let rename_error = fs::rename(&staged.path, directory.join("swapped-stage"))
            .expect_err("a concurrent rename must be denied while staging");

        assert_eq!(writer_error.raw_os_error(), Some(32));
        assert_eq!(rename_error.raw_os_error(), Some(32));
        drop(staged);
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn native_save_overwrite_preserves_unix_file_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let path = directory.join("mode-preservation.ori2");
        fs::write(&path, b"pre-existing invalid project").expect("write mode fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set fixture mode");
        let mut project = unsaved_project_with_redo_history("Mode preservation");

        save_project_to_path(&mut project, path.clone()).expect("overwrite mode fixture");

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(load_document_from_path(&path).unwrap(), project.document());
    }

    #[test]
    fn native_open_replaces_the_project_only_after_loading_and_validation() {
        let directory = TestDirectory::new();
        let path = directory.join("opened.ori2");
        let mut document = file_document("Opened project", 42.0);
        document.paper.cutting_allowed = true;
        persist_document(&path, &document).expect("write open fixture");

        let mut project = unsaved_project_with_redo_history("Replaced project");
        let expected_instance_id = project.instance_id;
        let replaced_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let loaded = load_project_file(path.clone()).expect("load native project");
        let response = apply_loaded_project_file(
            &mut project,
            expected_instance_id,
            replaced_project_id,
            expected_revision,
            loaded,
        )
        .expect("apply validated native project");

        assert!(!response.canceled);
        assert_ne!(project.project_id, replaced_project_id);
        assert_eq!(project.document(), document);
        assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
        assert_eq!(project.editor.revision(), 0);
        assert!(!project.editor.can_undo());
        assert!(!project.editor.can_redo());
        assert!(!project.is_dirty());
    }

    #[test]
    fn corrupt_native_open_preserves_project_state_and_history() {
        let directory = TestDirectory::new();
        let path = directory.join("corrupt.ori2");
        fs::write(&path, b"not an ORIGAMI2 archive").expect("write corrupt fixture");
        let project = unsaved_project_with_redo_history("Unaffected project");
        let before = project_state_signature(&project);

        let error = load_project_file(path).expect_err("corrupt project must fail validation");

        assert!(error.contains("failed to validate"));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn stale_native_open_is_rejected_without_replacing_newer_history() {
        let directory = TestDirectory::new();
        let path = directory.join("stale-open.ori2");
        persist_document(&path, &file_document("Stale open", 17.0))
            .expect("write stale-open fixture");
        let mut project = unsaved_project_with_redo_history("Active project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let stale_revision = project.editor.revision();
        let loaded = load_project_file(path).expect("prepare native open");
        execute_command(
            &mut project,
            expected_project_id,
            stale_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(8.0, 9.0),
            },
        )
        .expect("edit while the file dialog is open");
        let before_apply = project_state_signature(&project);

        let error = apply_loaded_project_file(
            &mut project,
            expected_instance_id,
            expected_project_id,
            stale_revision,
            loaded,
        )
        .expect_err("stale open must not replace the active project");

        assert_eq!(error, "the project changed while the file dialog was open");
        assert_eq!(project_state_signature(&project), before_apply);
    }

    #[test]
    fn native_file_dialog_results_cannot_land_after_reopening_the_same_document() {
        let directory = TestDirectory::new();
        let current_path = directory.join("same-document.ori2");
        let opened_path = directory.join("other-document.ori2");
        let selected_path = directory.join("must-not-save.ori2");
        let document = file_document("Same document", 21.0);
        persist_document(&current_path, &document).expect("write same-document fixture");
        persist_document(&opened_path, &file_document("Other document", 34.0))
            .expect("write other-document fixture");

        let mut project = ProjectState::from_document(document.clone(), current_path.clone());
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let loaded = load_project_file(opened_path).expect("load delayed open result");

        project = ProjectState::from_document(document, current_path);
        assert_eq!(project.project_id, expected_project_id);
        assert_eq!(project.editor.revision(), expected_revision);
        assert_ne!(project.instance_id, stale_instance_id);
        let before = project_state_signature(&project);

        let open_error = apply_loaded_project_file(
            &mut project,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            loaded,
        )
        .expect_err("a delayed open must not replace a reopened project instance");
        assert_eq!(
            open_error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), before);

        let save_error = save_project_as_selected_path(
            &mut project,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            selected_path.clone(),
        )
        .expect_err("a delayed save must not target a reopened project instance");
        assert_eq!(
            save_error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), before);
        assert!(!selected_path.exists());
    }

    #[test]
    fn native_save_failure_preserves_state_history_and_existing_target() {
        let directory = TestDirectory::new();
        let occupied_path = directory.join("occupied.ori2");
        fs::create_dir(&occupied_path).expect("create an unreplaceable save target");
        let sentinel = occupied_path.join("keep.txt");
        fs::write(&sentinel, b"keep this directory").expect("write save-failure sentinel");
        let mut project = unsaved_project_with_redo_history("Failed save");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let before = project_state_signature(&project);

        let error = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            occupied_path.clone(),
        )
        .expect_err("a directory cannot be replaced by a project file");

        assert!(error.contains("failed to"));
        assert_eq!(project_state_signature(&project), before);
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep this directory");
        assert!(occupied_path.is_dir());
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[test]
    fn stale_native_save_as_is_rejected_before_touching_the_selected_path() {
        let directory = TestDirectory::new();
        let selected_path = directory.join("stale-save");
        let normalized_path = directory.join("stale-save.ori2");
        let mut project = unsaved_project_with_redo_history("Stale save");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let stale_revision = project.editor.revision();
        execute_command(
            &mut project,
            expected_project_id,
            stale_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(99.0, 100.0),
            },
        )
        .expect("edit before stale save-as is applied");
        let before_save = project_state_signature(&project);

        let error = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            stale_revision,
            selected_path,
        )
        .expect_err("stale save-as must fail");

        assert_eq!(error, "the project changed while the file dialog was open");
        assert_eq!(project_state_signature(&project), before_save);
        assert!(!normalized_path.exists());
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
    fn relative_save_path_uses_the_current_directory_for_staging_and_sync() {
        assert_eq!(
            containing_directory(Path::new("bird.ori2")),
            Some(Path::new("."))
        );
        assert_eq!(
            containing_directory(Path::new("projects/bird.ori2")),
            Some(Path::new("projects"))
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
        assert_eq!(response.paper, document.paper);
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
