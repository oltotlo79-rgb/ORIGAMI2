//! Private native boundary for geometric-constraint solve and mutation commands.
//!
//! This module owns solve previews, saved-expression driver authority, staged
//! atomic apply, and constraint add/remove commands; shared project mechanics
//! remain in the crate root.

use super::*;

#[derive(Clone)]
pub(super) struct GeometricConstraintSolveStage {
    pub(super) token: ProjectId,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) positions: Vec<(VertexId, Point2)>,
    pub(super) expression_bindings: Option<Vec<VertexCoordinateExpressions>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometricConstraintSolveVertex {
    vertex_id: VertexId,
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GeometricConstraintSolvePreviewResponse {
    token: ProjectId,
    revision: u64,
    iterations: usize,
    maximum_residual: f64,
    rank: usize,
    degrees_of_freedom: usize,
    equation_count: usize,
    condition_estimate: f64,
    system_classification: &'static str,
    changed_vertices: Vec<GeometricConstraintSolveVertex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EdgeOrientationConstraint {
    Horizontal,
    Vertical,
}

#[tauri::command]
pub(super) fn preview_geometric_constraint_solve(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    driving_vertex: VertexId,
    x_mm: f64,
    y_mm: f64,
) -> Result<GeometricConstraintSolvePreviewResponse, String> {
    let project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err("project revision is stale".to_owned());
    }
    let solved = solve_geometric_constraints_v1(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        driving_vertex,
        Point2::new(x_mm, y_mm),
        ConstraintSolveLimitsV1::default(),
    )
    .map_err(|error| format!("geometric constraint solve failed: {error}"))?;
    let token = ProjectId::new();
    let response = GeometricConstraintSolvePreviewResponse {
        token,
        revision: expected_revision,
        iterations: solved.iterations,
        maximum_residual: solved.maximum_residual,
        rank: solved.rank,
        degrees_of_freedom: solved.degrees_of_freedom,
        equation_count: solved.equation_count,
        condition_estimate: solved.condition_estimate,
        system_classification: solve_system_classification(&solved),
        changed_vertices: solved
            .positions
            .iter()
            .map(|(vertex_id, point)| GeometricConstraintSolveVertex {
                vertex_id: *vertex_id,
                x: point.x,
                y: point.y,
            })
            .collect(),
    };
    let mut slot = state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())?;
    *slot = Some(GeometricConstraintSolveStage {
        token,
        project_instance_id: expected_project_instance_id,
        project_id: expected_project_id,
        revision: expected_revision,
        positions: solved.positions,
        expression_bindings: None,
    });
    Ok(response)
}

#[tauri::command]
pub(super) fn preview_geometric_constraint_edge_solve(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    driving_edge: EdgeId,
    start_x_mm: f64,
    start_y_mm: f64,
    end_x_mm: f64,
    end_y_mm: f64,
) -> Result<GeometricConstraintSolvePreviewResponse, String> {
    let project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err("project revision is stale".to_owned());
    }
    let edge = project
        .editor
        .pattern()
        .edges
        .iter()
        .find(|edge| edge.id == driving_edge)
        .ok_or_else(|| "driving edge is missing".to_owned())?;
    let solved = solve_geometric_constraints_with_drivers_v1(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &[
            (edge.start, Point2::new(start_x_mm, start_y_mm)),
            (edge.end, Point2::new(end_x_mm, end_y_mm)),
        ],
        ConstraintSolveLimitsV1::default(),
    )
    .map_err(|error| format!("geometric constraint solve failed: {error}"))?;
    let token = ProjectId::new();
    let response = GeometricConstraintSolvePreviewResponse {
        token,
        revision: expected_revision,
        iterations: solved.iterations,
        maximum_residual: solved.maximum_residual,
        rank: solved.rank,
        degrees_of_freedom: solved.degrees_of_freedom,
        equation_count: solved.equation_count,
        condition_estimate: solved.condition_estimate,
        system_classification: solve_system_classification(&solved),
        changed_vertices: solved
            .positions
            .iter()
            .map(|(vertex_id, point)| GeometricConstraintSolveVertex {
                vertex_id: *vertex_id,
                x: point.x,
                y: point.y,
            })
            .collect(),
    };
    *state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())? =
        Some(GeometricConstraintSolveStage {
            token,
            project_instance_id: expected_project_instance_id,
            project_id: expected_project_id,
            revision: expected_revision,
            positions: solved.positions,
            expression_bindings: None,
        });
    Ok(response)
}

#[tauri::command]
pub(super) fn preview_geometric_constraint_expression_solve(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<GeometricConstraintSolvePreviewResponse, String> {
    let project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err("project revision is stale".to_owned());
    }
    let drivers = reevaluate_saved_vertex_expressions(&project)?;
    let solved = solve_geometric_constraints_with_drivers_v1(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &drivers,
        ConstraintSolveLimitsV1::default(),
    )
    .map_err(|error| format!("geometric constraint solve failed: {error}"))?;
    let token = ProjectId::new();
    let response = geometric_constraint_solve_response(token, expected_revision, &solved);
    let expression_bindings = project
        .numeric_expressions
        .vertex_coordinates
        .iter()
        .filter_map(|binding| {
            solved
                .positions
                .iter()
                .find(|(vertex, _)| *vertex == binding.vertex)
                .map(|(_, point)| {
                    let mut binding = binding.clone();
                    binding.adopted_x_mm = point.x;
                    binding.adopted_y_mm = point.y;
                    binding
                })
        })
        .collect();
    *state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())? =
        Some(GeometricConstraintSolveStage {
            token,
            project_instance_id: expected_project_instance_id,
            project_id: expected_project_id,
            revision: expected_revision,
            positions: solved.positions,
            expression_bindings: Some(expression_bindings),
        });
    Ok(response)
}

pub(super) fn reevaluate_saved_vertex_expressions(
    project: &ProjectState,
) -> Result<Vec<(VertexId, Point2)>, String> {
    if project.numeric_expressions.vertex_coordinates.is_empty()
        || project.numeric_expressions.vertex_coordinates.len()
            > ConstraintSolveLimitsV1::default().max_vertices
    {
        return Err("saved numeric expression set is empty or too large".to_owned());
    }
    let mut seen = HashSet::new();
    for binding in &project.numeric_expressions.vertex_coordinates {
        if !seen.insert(binding.vertex) {
            return Err("saved numeric expressions contain a cycle or duplicate".to_owned());
        }
    }
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let mut work = 0usize;
    let mut drivers = Vec::with_capacity(project.numeric_expressions.vertex_coordinates.len());
    for binding in &project.numeric_expressions.vertex_coordinates {
        let x = resolve_saved_coordinate(
            project,
            binding.vertex,
            false,
            &mut memo,
            &mut visiting,
            &mut work,
            0,
        )?;
        let y = resolve_saved_coordinate(
            project,
            binding.vertex,
            true,
            &mut memo,
            &mut visiting,
            &mut work,
            0,
        )?;
        drivers.push((binding.vertex, Point2::new(x, y)));
    }
    Ok(drivers)
}

const MAX_SAVED_EXPRESSION_DEPENDENCY_DEPTH: usize = 64;
const MAX_SAVED_EXPRESSION_REFERENCES: usize = 4_096;

fn resolve_saved_coordinate(
    project: &ProjectState,
    vertex: VertexId,
    y_axis: bool,
    memo: &mut HashMap<(VertexId, bool), f64>,
    visiting: &mut HashSet<(VertexId, bool)>,
    work: &mut usize,
    depth: usize,
) -> Result<f64, String> {
    let key = (vertex, y_axis);
    if let Some(value) = memo.get(&key) {
        return Ok(*value);
    }
    if depth > MAX_SAVED_EXPRESSION_DEPENDENCY_DEPTH || !visiting.insert(key) {
        return Err("saved numeric expressions contain a dependency cycle".to_owned());
    }
    let binding = project
        .numeric_expressions
        .vertex_coordinates
        .iter()
        .find(|binding| binding.vertex == vertex);
    let value = if let Some(binding) = binding {
        let source = if y_axis {
            &binding.y_source
        } else {
            &binding.x_source
        };
        let expanded =
            expand_saved_vertex_references(project, source, memo, visiting, work, depth)?;
        let pair = if y_axis {
            evaluate_finite_millimetre_pair("0".to_owned(), expanded)
        } else {
            evaluate_finite_millimetre_pair(expanded, "0".to_owned())
        }
        .map_err(|error| error.user_input_message().to_owned())?;
        if y_axis { pair.1 } else { pair.0 }
    } else {
        let point = project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|candidate| candidate.id == vertex)
            .map(|candidate| candidate.position)
            .ok_or_else(|| "saved numeric expression has a dangling vertex reference".to_owned())?;
        if y_axis { point.y } else { point.x }
    };
    visiting.remove(&key);
    memo.insert(key, value);
    Ok(value)
}

pub(super) fn expand_saved_vertex_references(
    project: &ProjectState,
    source: &str,
    memo: &mut HashMap<(VertexId, bool), f64>,
    visiting: &mut HashSet<(VertexId, bool)>,
    work: &mut usize,
    depth: usize,
) -> Result<String, String> {
    let mut result = String::with_capacity(source.len());
    let mut cursor = 0;
    loop {
        let remaining = &source[cursor..];
        let vertex_start = remaining.find("v.");
        let edge_start = remaining.find("e.");
        let Some(relative) = (match (vertex_start, edge_start) {
            (Some(vertex), Some(edge)) => Some(vertex.min(edge)),
            (Some(vertex), None) => Some(vertex),
            (None, Some(edge)) => Some(edge),
            (None, None) => None,
        }) else {
            break;
        };
        let start = cursor + relative;
        result.push_str(&source[cursor..start]);
        if source[start..].starts_with("e.") {
            let id_end = start
                .checked_add(38)
                .ok_or_else(|| "invalid edge reference".to_owned())?;
            let uuid = source
                .get(start + 2..id_end)
                .ok_or_else(|| "invalid edge reference".to_owned())?;
            let suffix = source
                .get(id_end..)
                .ok_or_else(|| "invalid edge reference".to_owned())?;
            let (y_axis_angle, end) = if suffix.starts_with(".length") {
                (false, id_end + 7)
            } else if suffix.starts_with(".angle") {
                (true, id_end + 6)
            } else {
                return Err("invalid edge reference".to_owned());
            };
            let referenced: EdgeId = serde_json::from_str(&format!("\"{uuid}\""))
                .map_err(|_| "invalid edge reference".to_owned())?;
            let canonical = serde_json::to_value(referenced)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| "invalid edge reference".to_owned())?;
            if canonical != uuid {
                return Err("invalid edge reference".to_owned());
            }
            let edge = project
                .editor
                .pattern()
                .edges
                .iter()
                .find(|edge| edge.id == referenced)
                .ok_or_else(|| {
                    "saved numeric expression has a dangling edge reference".to_owned()
                })?;
            *work = work
                .checked_add(1)
                .ok_or_else(|| "expression reference limit".to_owned())?;
            if *work > MAX_SAVED_EXPRESSION_REFERENCES {
                return Err("expression reference limit".to_owned());
            }
            let start_x = resolve_saved_coordinate(
                project,
                edge.start,
                false,
                memo,
                visiting,
                work,
                depth + 1,
            )?;
            let start_y = resolve_saved_coordinate(
                project,
                edge.start,
                true,
                memo,
                visiting,
                work,
                depth + 1,
            )?;
            let end_x = resolve_saved_coordinate(
                project,
                edge.end,
                false,
                memo,
                visiting,
                work,
                depth + 1,
            )?;
            let end_y =
                resolve_saved_coordinate(project, edge.end, true, memo, visiting, work, depth + 1)?;
            let delta_x = end_x - start_x;
            let delta_y = end_y - start_y;
            let edge_length = delta_x.hypot(delta_y);
            if !edge_length.is_finite() || edge_length <= 0.0 {
                return Err("edge reference geometry is degenerate".to_owned());
            }
            let value = if y_axis_angle {
                delta_y.atan2(delta_x).to_degrees().rem_euclid(360.0)
            } else {
                edge_length
            };
            if !value.is_finite() {
                return Err("edge reference result is non-finite".to_owned());
            }
            result.push('(');
            result.push_str(&value.to_string());
            result.push(')');
            cursor = end;
            continue;
        }
        let end = start
            .checked_add(40)
            .ok_or_else(|| "invalid vertex reference".to_owned())?;
        let token = source
            .get(start..end)
            .ok_or_else(|| "invalid vertex reference".to_owned())?;
        let uuid = token
            .get(2..38)
            .ok_or_else(|| "invalid vertex reference".to_owned())?;
        let axis = token
            .get(38..40)
            .ok_or_else(|| "invalid vertex reference".to_owned())?;
        if !matches!(axis, ".x" | ".y") {
            return Err("invalid vertex reference".to_owned());
        }
        let referenced: VertexId = serde_json::from_str(&format!("\"{uuid}\""))
            .map_err(|_| "invalid vertex reference".to_owned())?;
        let canonical = serde_json::to_value(referenced)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| "invalid vertex reference".to_owned())?;
        if canonical != uuid {
            return Err("invalid vertex reference".to_owned());
        }
        *work = work
            .checked_add(1)
            .ok_or_else(|| "expression reference limit".to_owned())?;
        if *work > MAX_SAVED_EXPRESSION_REFERENCES {
            return Err("expression reference limit".to_owned());
        }
        let value = resolve_saved_coordinate(
            project,
            referenced,
            axis == ".y",
            memo,
            visiting,
            work,
            depth + 1,
        )?;
        result.push('(');
        result.push_str(&value.to_string());
        result.push(')');
        cursor = end;
    }
    result.push_str(&source[cursor..]);
    Ok(result)
}

fn geometric_constraint_solve_response(
    token: ProjectId,
    revision: u64,
    solved: &ori_core::ConstraintSolvePreviewV1,
) -> GeometricConstraintSolvePreviewResponse {
    GeometricConstraintSolvePreviewResponse {
        token,
        revision,
        iterations: solved.iterations,
        maximum_residual: solved.maximum_residual,
        rank: solved.rank,
        degrees_of_freedom: solved.degrees_of_freedom,
        equation_count: solved.equation_count,
        condition_estimate: solved.condition_estimate,
        system_classification: solve_system_classification(solved),
        changed_vertices: solved
            .positions
            .iter()
            .map(|(vertex_id, point)| GeometricConstraintSolveVertex {
                vertex_id: *vertex_id,
                x: point.x,
                y: point.y,
            })
            .collect(),
    }
}

fn solve_system_classification(solved: &ori_core::ConstraintSolvePreviewV1) -> &'static str {
    if solved.degrees_of_freedom > 0 {
        "under_constrained"
    } else if solved.equation_count > solved.rank {
        "over_constrained"
    } else {
        "well_constrained"
    }
}

#[tauri::command]
pub(super) fn apply_geometric_constraint_solve(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    token: ProjectId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    let staged = state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())?
        .clone()
        .ok_or_else(|| "geometric constraint preview is missing".to_owned())?;
    let result = apply_geometric_constraint_solve_stage(
        &mut project,
        &staged,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        token,
    )?;
    let mut slot = state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())?;
    if slot.as_ref().is_some_and(|current| current.token == token) {
        *slot = None;
    }
    Ok(result)
}

pub(super) fn apply_geometric_constraint_solve_stage(
    project: &mut ProjectState,
    staged: &GeometricConstraintSolveStage,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    token: ProjectId,
) -> Result<ProjectSnapshot, String> {
    if staged.token != token
        || staged.project_instance_id != expected_project_instance_id
        || staged.project_id != expected_project_id
        || staged.revision != expected_revision
        || project.editor.revision() != expected_revision
    {
        return Err("geometric constraint preview is stale".to_owned());
    }
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::MoveVertices {
            updates: staged
                .positions
                .iter()
                .map(|(vertex, position)| VertexPositionUpdate {
                    vertex: *vertex,
                    position: *position,
                })
                .collect(),
        },
    )?;
    if let Some(bindings) = &staged.expression_bindings {
        for binding in bindings {
            project.adopt_vertex_coordinate_expression(binding.clone());
        }
    }
    Ok(snapshot(project))
}

#[tauri::command]
pub(super) fn add_edge_orientation_constraint(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    orientation: EdgeOrientationConstraint,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let constraint = match orientation {
        EdgeOrientationConstraint::Horizontal => GeometricConstraintKindV1::Horizontal { edge },
        EdgeOrientationConstraint::Vertical => GeometricConstraintKindV1::Vertical { edge },
    };
    execute_expected_command(
        &mut project,
        expectation,
        Command::AddGeometricConstraint {
            record: GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            },
        },
    )
}

#[tauri::command]
pub(super) fn add_geometric_constraint(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    constraint: GeometricConstraintKindV1,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::AddGeometricConstraint {
            record: GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            },
        },
    )
}

#[tauri::command]
pub(super) fn remove_geometric_constraint(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    constraint: ConstraintId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::RemoveGeometricConstraint { id: constraint },
    )
}
