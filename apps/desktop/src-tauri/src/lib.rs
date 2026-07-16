use std::sync::Mutex;

use ori_core::{Command, EditorState};
use ori_domain::{CreasePattern, EdgeId, EdgeKind, Point2, VertexId};
use serde::Serialize;
use tauri::State;

struct AppState(Mutex<EditorState>);

#[derive(Serialize)]
struct PatternResponse {
    vertex_count: usize,
    edge_count: usize,
}

#[derive(Debug, Serialize)]
struct ProjectSnapshot {
    revision: u64,
    crease_pattern: CreasePattern,
    can_undo: bool,
    can_redo: bool,
    cutting_allowed: bool,
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
    execute_command(
        &mut editor,
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
    expected_revision: u64,
    id: VertexId,
    x: f64,
    y: f64,
) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    execute_command(
        &mut editor,
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
    expected_revision: u64,
    id: VertexId,
) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    execute_command(&mut editor, expected_revision, Command::RemoveVertex { id })
}

#[tauri::command]
fn add_edge(
    state: State<'_, AppState>,
    expected_revision: u64,
    start: VertexId,
    end: VertexId,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    execute_command(
        &mut editor,
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
    expected_revision: u64,
    id: EdgeId,
) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    execute_command(&mut editor, expected_revision, Command::RemoveEdge { id })
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

#[tauri::command]
fn set_cutting_allowed(
    state: State<'_, AppState>,
    expected_revision: u64,
    allowed: bool,
) -> Result<ProjectSnapshot, String> {
    let mut editor = state.0.lock().map_err(|error| error.to_string())?;
    execute_command(
        &mut editor,
        expected_revision,
        Command::SetCuttingAllowed { allowed },
    )
}

fn execute_command(
    editor: &mut EditorState,
    expected_revision: u64,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    editor
        .execute(expected_revision, command)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(editor))
}

fn snapshot(editor: &EditorState) -> ProjectSnapshot {
    ProjectSnapshot {
        revision: editor.revision(),
        crease_pattern: editor.pattern().clone(),
        can_undo: editor.can_undo(),
        can_redo: editor.can_redo(),
        cutting_allowed: editor.cutting_allowed(),
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
            move_vertex,
            remove_vertex,
            add_edge,
            remove_edge,
            undo,
            redo,
            set_cutting_allowed
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ORIGAMI2 desktop application");
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, Vertex};

    use super::*;

    #[test]
    fn move_vertex_returns_the_updated_revision_and_snapshot() {
        let id = VertexId::new();
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![Vertex {
                id,
                position: Point2::new(1.0, 2.0),
            }],
            edges: Vec::new(),
        });

        let response = execute_command(
            &mut editor,
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
    }

    #[test]
    fn remove_edge_then_vertex_returns_each_current_snapshot() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut editor = EditorState::new(CreasePattern {
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

        let response =
            execute_command(&mut editor, 0, Command::RemoveEdge { id: edge }).expect("remove edge");
        assert_eq!(response.revision, 1);
        assert!(response.crease_pattern.edges.is_empty());

        let response = execute_command(&mut editor, 1, Command::RemoveVertex { id: start })
            .expect("remove vertex");
        assert_eq!(response.revision, 2);
        assert_eq!(response.crease_pattern.vertices.len(), 1);
        assert_eq!(response.crease_pattern.vertices[0].id, end);
    }

    #[test]
    fn edit_commands_preserve_revision_conflict_errors() {
        let id = VertexId::new();
        let mut editor = EditorState::new(CreasePattern {
            vertices: vec![Vertex {
                id,
                position: Point2::new(0.0, 0.0),
            }],
            edges: Vec::new(),
        });

        let error = execute_command(&mut editor, 4, Command::RemoveVertex { id })
            .expect_err("stale command must fail");

        assert_eq!(error, "expected revision 4, but the current revision is 0");
        assert_eq!(editor.pattern().vertices.len(), 1);
    }
}
