use std::sync::Mutex;

use ori_core::{Command, EditorState};
use ori_domain::{CreasePattern, Point2, VertexId};
use serde::Serialize;
use tauri::State;

struct AppState(Mutex<EditorState>);

#[derive(Serialize)]
struct PatternResponse {
    vertex_count: usize,
    edge_count: usize,
}

#[derive(Serialize)]
struct ProjectSnapshot {
    revision: u64,
    crease_pattern: CreasePattern,
    can_undo: bool,
    can_redo: bool,
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
    let editor = state.0.lock().map_err(|error| error.to_string())?;
    Ok(snapshot(&editor))
}

#[tauri::command]
fn add_vertex(
    state: State<'_, AppState>,
    expected_revision: u64,
    x: f64,
    y: f64,
) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    editor
        .execute(
            expected_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(x, y),
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&editor))
}

#[tauri::command]
fn undo(state: State<'_, AppState>, expected_revision: u64) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    editor
        .undo(expected_revision)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&editor))
}

#[tauri::command]
fn redo(state: State<'_, AppState>, expected_revision: u64) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    editor
        .redo(expected_revision)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&editor))
}

fn snapshot(editor: &EditorState) -> ProjectSnapshot {
    ProjectSnapshot {
        revision: editor.revision(),
        crease_pattern: editor.pattern().clone(),
        can_undo: editor.can_undo(),
        can_redo: editor.can_redo(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState(Mutex::new(EditorState::new(
            CreasePattern::empty(),
        ))))
        .invoke_handler(tauri::generate_handler![
            generate_benchmark_pattern,
            project_snapshot,
            add_vertex,
            undo,
            redo
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ORIGAMI2 desktop application");
}
