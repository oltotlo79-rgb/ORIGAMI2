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
    pub(super) exact_satisfaction: Option<GeometricConstraintSolveExactSatisfaction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometricConstraintSolveVertex {
    vertex_id: VertexId,
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GeometricConstraintSolveExactSatisfaction {
    model_id: &'static str,
    constraint_count: usize,
    equation_count: usize,
    authorizes_project_mutation: bool,
    replayable_across_runtimes: bool,
}

#[derive(Debug)]
struct PreparedGeometricConstraintSolve {
    positions: Vec<(VertexId, Point2)>,
    exact_satisfaction: Option<GeometricConstraintSolveExactSatisfaction>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    exact_satisfaction: Option<GeometricConstraintSolveExactSatisfaction>,
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
    let (response, stage) = finish_geometric_constraint_solve_preview(
        token,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &solved,
        None,
        None,
    );
    let mut slot = state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())?;
    *slot = Some(stage);
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
    let (response, stage) = finish_geometric_constraint_solve_preview(
        token,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &solved,
        None,
        None,
    );
    *state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())? = Some(stage);
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
    let (response, stage) = finish_geometric_constraint_solve_preview(
        token,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &solved,
        Some(&project.numeric_expressions.vertex_coordinates),
        Some(&drivers),
    );
    *state
        .3
        .lock()
        .map_err(|_| "geometric constraint preview state unavailable".to_owned())? = Some(stage);
    Ok(response)
}

pub(super) fn reevaluate_saved_vertex_expressions(
    project: &ProjectState,
) -> Result<Vec<(VertexId, Point2)>, String> {
    reevaluate_saved_vertex_expressions_with_legacy_policy(
        project,
        LegacyEdgeGeometryReferencePolicy::ReevaluateDeterministically,
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    )
}

#[cfg(test)]
pub(super) fn reevaluate_saved_vertex_expressions_with_model_support_for_test(
    project: &ProjectState,
    deterministic_model_supported: bool,
) -> Result<Vec<(VertexId, Point2)>, String> {
    reevaluate_saved_vertex_expressions_with_legacy_policy(
        project,
        LegacyEdgeGeometryReferencePolicy::ReevaluateDeterministically,
        deterministic_model_supported,
    )
}

#[cfg(test)]
pub(super) fn reevaluate_saved_vertex_expressions_for_archive_load(
    project: &ProjectState,
) -> Result<Vec<(VertexId, Point2)>, String> {
    reevaluate_saved_vertex_expressions_for_archive_load_with_model_support(
        project,
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    )
}

pub(super) fn reevaluate_saved_vertex_expressions_for_archive_load_with_model_support(
    project: &ProjectState,
    deterministic_model_supported: bool,
) -> Result<Vec<(VertexId, Point2)>, String> {
    reevaluate_saved_vertex_expressions_with_legacy_policy(
        project,
        LegacyEdgeGeometryReferencePolicy::AdoptPersistedCoordinate,
        deterministic_model_supported,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyEdgeGeometryReferencePolicy {
    ReevaluateDeterministically,
    AdoptPersistedCoordinate,
}

fn reevaluate_saved_vertex_expressions_with_legacy_policy(
    project: &ProjectState,
    legacy_policy: LegacyEdgeGeometryReferencePolicy,
    deterministic_model_supported: bool,
) -> Result<Vec<(VertexId, Point2)>, String> {
    if project.numeric_expressions.vertex_coordinates.is_empty()
        || project.numeric_expressions.vertex_coordinates.len()
            > ConstraintSolveLimitsV1::default().max_vertices
    {
        return Err("saved numeric expression set is empty or too large".to_owned());
    }
    if legacy_policy == LegacyEdgeGeometryReferencePolicy::ReevaluateDeterministically
        && !deterministic_model_supported
        && project
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .any(|binding| binding.x_source.contains("e.") || binding.y_source.contains("e."))
    {
        return Err("deterministic geometry reference model is unsupported".to_owned());
    }
    let mut seen = HashSet::new();
    for binding in &project.numeric_expressions.vertex_coordinates {
        validate_saved_vertex_expression_transcendental_model_with_support(
            binding,
            deterministic_model_supported,
        )?;
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
            legacy_policy,
        )?;
        let y = resolve_saved_coordinate(
            project,
            binding.vertex,
            true,
            &mut memo,
            &mut visiting,
            &mut work,
            0,
            legacy_policy,
        )?;
        drivers.push((binding.vertex, Point2::new(x, y)));
    }
    Ok(drivers)
}

pub(super) fn validate_saved_vertex_expression_transcendental_model_with_support(
    binding: &VertexCoordinateExpressions,
    deterministic_model_supported: bool,
) -> Result<(), String> {
    match (
        binding.schema_version,
        binding.transcendental_model_id.as_deref(),
    ) {
        (ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_LEGACY_V1, None) => Ok(()),
        (
            ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2,
            Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1),
        ) if deterministic_model_supported
            && (binding.uses_edge_geometry_reference() || binding.polar_construction.is_some()) =>
        {
            Ok(())
        }
        _ => Err("saved numeric expression transcendental model is invalid".to_owned()),
    }
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
    legacy_policy: LegacyEdgeGeometryReferencePolicy,
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
        let expanded = expand_saved_vertex_references_with_legacy_policy(
            project,
            source,
            memo,
            visiting,
            work,
            depth,
            legacy_policy,
        )?;
        let pair = if y_axis {
            evaluate_finite_millimetre_pair("0".to_owned(), expanded)
        } else {
            evaluate_finite_millimetre_pair(expanded, "0".to_owned())
        }
        .map_err(|error| error.user_input_message().to_owned())?;
        let evaluated = if y_axis { pair.1 } else { pair.0 };
        if legacy_policy == LegacyEdgeGeometryReferencePolicy::AdoptPersistedCoordinate
            && ((binding.polar_construction.is_some()
                && binding.schema_version
                    == ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_LEGACY_V1
                && binding.transcendental_model_id.is_none())
                || (binding.uses_legacy_edge_geometry_reference_v1()
                    && source_uses_edge_geometry_reference(source)))
        {
            if y_axis {
                binding.adopted_y_mm
            } else {
                binding.adopted_x_mm
            }
        } else {
            evaluated
        }
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

#[cfg(test)]
pub(super) fn expand_saved_vertex_references(
    project: &ProjectState,
    source: &str,
    memo: &mut HashMap<(VertexId, bool), f64>,
    visiting: &mut HashSet<(VertexId, bool)>,
    work: &mut usize,
    depth: usize,
) -> Result<String, String> {
    expand_saved_vertex_references_with_legacy_policy(
        project,
        source,
        memo,
        visiting,
        work,
        depth,
        LegacyEdgeGeometryReferencePolicy::ReevaluateDeterministically,
    )
}

fn expand_saved_vertex_references_with_legacy_policy(
    project: &ProjectState,
    source: &str,
    memo: &mut HashMap<(VertexId, bool), f64>,
    visiting: &mut HashSet<(VertexId, bool)>,
    work: &mut usize,
    depth: usize,
    legacy_policy: LegacyEdgeGeometryReferencePolicy,
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
            if source
                .get(end..)
                .and_then(|tail| tail.chars().next())
                .is_some_and(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '.')
                })
            {
                return Err("invalid edge reference".to_owned());
            }
            let referenced: EdgeId = serde_json::from_str(&format!("\"{uuid}\""))
                .map_err(|_| "invalid edge reference".to_owned())?;
            if referenced.canonical_bytes().iter().all(|byte| *byte == 0) {
                return Err("invalid edge reference".to_owned());
            }
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
                legacy_policy,
            )?;
            let start_y = resolve_saved_coordinate(
                project,
                edge.start,
                true,
                memo,
                visiting,
                work,
                depth + 1,
                legacy_policy,
            )?;
            let end_x = resolve_saved_coordinate(
                project,
                edge.end,
                false,
                memo,
                visiting,
                work,
                depth + 1,
                legacy_policy,
            )?;
            let end_y = resolve_saved_coordinate(
                project,
                edge.end,
                true,
                memo,
                visiting,
                work,
                depth + 1,
                legacy_policy,
            )?;
            let delta_x = end_x - start_x;
            let delta_y = end_y - start_y;
            let (edge_length, edge_angle_degrees) =
                deterministic_saved_edge_reference_geometry(delta_x, delta_y)?;
            let value = if y_axis_angle {
                edge_angle_degrees
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
            legacy_policy,
        )?;
        result.push('(');
        result.push_str(&value.to_string());
        result.push(')');
        cursor = end;
    }
    result.push_str(&source[cursor..]);
    Ok(result)
}

fn source_uses_edge_geometry_reference(source: &str) -> bool {
    source.contains("e.")
}

fn deterministic_saved_edge_reference_geometry(
    delta_x: f64,
    delta_y: f64,
) -> Result<(f64, f64), String> {
    let edge_length = ori_numeric::deterministic_hypot_v1(delta_x, delta_y)
        .map_err(|_| "edge reference result is non-finite".to_owned())?;
    if edge_length <= 0.0 {
        return Err("edge reference geometry is degenerate".to_owned());
    }
    let angle_radians = ori_numeric::deterministic_atan2_v1(delta_y, delta_x)
        .map_err(|_| "edge reference result is non-finite".to_owned())?;
    let angle_degrees = ori_numeric::deterministic_radians_to_degrees_v1(angle_radians)
        .map_err(|_| "edge reference result is non-finite".to_owned())?
        .rem_euclid(360.0);
    let angle_degrees = if angle_degrees == 0.0 {
        0.0
    } else {
        angle_degrees
    };
    if !angle_degrees.is_finite() {
        return Err("edge reference result is non-finite".to_owned());
    }
    Ok((edge_length, angle_degrees))
}

fn finish_geometric_constraint_solve_preview(
    token: ProjectId,
    expectation: ProjectExpectation,
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    solved: &ori_core::ConstraintSolvePreviewV1,
    expression_bindings: Option<&[VertexCoordinateExpressions]>,
    expression_drivers: Option<&[(VertexId, Point2)]>,
) -> (
    GeometricConstraintSolvePreviewResponse,
    GeometricConstraintSolveStage,
) {
    finish_geometric_constraint_solve_preview_with_model_support(
        token,
        expectation,
        pattern,
        document,
        solved,
        expression_bindings,
        expression_drivers,
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    )
}

fn finish_geometric_constraint_solve_preview_with_model_support(
    token: ProjectId,
    expectation: ProjectExpectation,
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    solved: &ori_core::ConstraintSolvePreviewV1,
    expression_bindings: Option<&[VertexCoordinateExpressions]>,
    expression_drivers: Option<&[(VertexId, Point2)]>,
    deterministic_model_supported: bool,
) -> (
    GeometricConstraintSolvePreviewResponse,
    GeometricConstraintSolveStage,
) {
    let mut prepared = prepare_geometric_constraint_solve(pattern, document, solved);
    // Axis-aligned exactification has no driver metadata and may project a
    // fixed expression vertex onto another class member. Preserve the
    // reevaluated expression bits as the authority for expression previews.
    if prepared.exact_satisfaction.is_some()
        && expression_drivers.is_some_and(|drivers| {
            drivers.iter().any(|(vertex, driver)| {
                prepared
                    .positions
                    .iter()
                    .find(|(candidate, _)| candidate == vertex)
                    .map(|(_, point)| *point)
                    .or_else(|| {
                        pattern
                            .vertices
                            .iter()
                            .find(|candidate| candidate.id == *vertex)
                            .map(|candidate| candidate.position)
                    })
                    .is_none_or(|candidate| {
                        candidate.x.to_bits() != driver.x.to_bits()
                            || candidate.y.to_bits() != driver.y.to_bits()
                    })
            })
        })
    {
        let mut positions = solved.positions.clone();
        if let Some(drivers) = expression_drivers {
            for (vertex, driver) in drivers {
                if let Some((_, point)) = positions
                    .iter_mut()
                    .find(|(candidate, _)| candidate == vertex)
                {
                    *point = *driver;
                    continue;
                }
                if pattern
                    .vertices
                    .iter()
                    .find(|candidate| candidate.id == *vertex)
                    .is_some_and(|candidate| {
                        candidate.position.x.to_bits() != driver.x.to_bits()
                            || candidate.position.y.to_bits() != driver.y.to_bits()
                    })
                {
                    positions.push((*vertex, *driver));
                }
            }
            positions.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
        }
        prepared = PreparedGeometricConstraintSolve {
            positions,
            exact_satisfaction: None,
        };
    }
    let response =
        geometric_constraint_solve_response(token, expectation.revision, solved, &prepared);
    let expression_bindings = expression_bindings.map(|bindings| {
        bindings
            .iter()
            .filter_map(|binding| {
                let point = prepared
                    .positions
                    .iter()
                    .find(|(vertex, _)| *vertex == binding.vertex)
                    .map(|(_, point)| *point)
                    .or_else(|| {
                        expression_drivers.and_then(|drivers| {
                            drivers
                                .iter()
                                .find(|(vertex, _)| *vertex == binding.vertex)
                                .map(|(_, point)| *point)
                        })
                    })
                    .or_else(|| {
                        pattern
                            .vertices
                            .iter()
                            .find(|vertex| vertex.id == binding.vertex)
                            .map(|vertex| vertex.position)
                    })?;
                let mut updated = binding.clone();
                updated.adopted_x_mm = point.x;
                updated.adopted_y_mm = point.y;
                upgrade_expression_binding_after_deterministic_reevaluation(
                    &mut updated,
                    deterministic_model_supported,
                );
                let changed = updated.adopted_x_mm.to_bits() != binding.adopted_x_mm.to_bits()
                    || updated.adopted_y_mm.to_bits() != binding.adopted_y_mm.to_bits()
                    || updated.schema_version != binding.schema_version
                    || updated.transcendental_model_id.as_deref()
                        != binding.transcendental_model_id.as_deref();
                changed.then_some(updated)
            })
            .collect()
    });
    let stage = GeometricConstraintSolveStage {
        token,
        project_instance_id: expectation.instance_id,
        project_id: expectation.project_id,
        revision: expectation.revision,
        positions: prepared.positions,
        expression_bindings,
        exact_satisfaction: prepared.exact_satisfaction,
    };
    (response, stage)
}

pub(super) fn upgrade_expression_binding_after_deterministic_reevaluation(
    binding: &mut VertexCoordinateExpressions,
    deterministic_model_supported: bool,
) {
    if binding.uses_edge_geometry_reference() && deterministic_model_supported {
        binding.schema_version =
            ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2;
        binding.transcendental_model_id =
            Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.to_owned());
    }
}

fn geometric_constraint_solve_response(
    token: ProjectId,
    revision: u64,
    solved: &ori_core::ConstraintSolvePreviewV1,
    prepared: &PreparedGeometricConstraintSolve,
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
        changed_vertices: prepared
            .positions
            .iter()
            .map(|(vertex_id, point)| GeometricConstraintSolveVertex {
                vertex_id: *vertex_id,
                x: point.x,
                y: point.y,
            })
            .collect(),
        exact_satisfaction: prepared.exact_satisfaction,
    }
}

fn prepare_geometric_constraint_solve(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    solved: &ori_core::ConstraintSolvePreviewV1,
) -> PreparedGeometricConstraintSolve {
    let Some(exact) =
        ori_core::exactify_axis_aligned_constraint_preview_v1(pattern, document, solved)
    else {
        return PreparedGeometricConstraintSolve {
            positions: solved.positions.clone(),
            exact_satisfaction: None,
        };
    };

    let mut positions = exact
        .pattern()
        .vertices
        .iter()
        .filter_map(|vertex| {
            let original = pattern
                .vertices
                .iter()
                .find(|candidate| candidate.id == vertex.id)?;
            (original.position.x.to_bits() != vertex.position.x.to_bits()
                || original.position.y.to_bits() != vertex.position.y.to_bits())
            .then_some((vertex.id, vertex.position))
        })
        .collect::<Vec<_>>();
    positions.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
    let certificate = exact.certificate();
    PreparedGeometricConstraintSolve {
        positions,
        exact_satisfaction: Some(GeometricConstraintSolveExactSatisfaction {
            model_id: exact.model_id(),
            constraint_count: certificate.constraint_count(),
            equation_count: certificate.equation_count(),
            authorizes_project_mutation: exact.authorizes_project_mutation(),
            replayable_across_runtimes: exact.replayable_across_runtimes(),
        }),
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
    let expression_bindings = staged.expression_bindings.as_deref().unwrap_or_default();
    let mut apply_positions = staged.positions.clone();
    let mut seen_expression_vertices = HashSet::with_capacity(expression_bindings.len());
    for binding in expression_bindings {
        let staged_point = staged
            .positions
            .iter()
            .find(|(vertex, _)| *vertex == binding.vertex)
            .map(|(_, point)| *point);
        let target_exists = project
            .editor
            .pattern()
            .vertices
            .iter()
            .any(|vertex| vertex.id == binding.vertex);
        let point =
            staged_point.unwrap_or_else(|| Point2::new(binding.adopted_x_mm, binding.adopted_y_mm));
        if !seen_expression_vertices.insert(binding.vertex)
            || !target_exists
            || !point.x.is_finite()
            || !point.y.is_finite()
            || staged_point.is_some_and(|staged_point| {
                binding.adopted_x_mm.to_bits() != staged_point.x.to_bits()
                    || binding.adopted_y_mm.to_bits() != staged_point.y.to_bits()
            })
        {
            return Err("geometric constraint preview binding is invalid".to_owned());
        }
        if !apply_positions
            .iter()
            .any(|(vertex, _)| *vertex == binding.vertex)
        {
            apply_positions.push((binding.vertex, point));
        }
    }
    apply_positions.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
    if let Some(exact_satisfaction) = staged.exact_satisfaction {
        recertify_staged_exact_geometric_constraint_solve(
            project.editor.pattern(),
            project.editor.geometric_constraints(),
            &apply_positions,
            exact_satisfaction,
        )?;
    }
    if apply_positions.is_empty() {
        return Ok(snapshot(project));
    }
    let updates = apply_positions
        .iter()
        .map(|(vertex, position)| VertexPositionUpdate {
            vertex: *vertex,
            position: *position,
        })
        .collect();
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::MoveVertices { updates },
    )?;
    for binding in expression_bindings {
        project.adopt_vertex_coordinate_expression(binding.clone());
    }
    project.reconcile_vertex_coordinate_expressions();
    Ok(snapshot(project))
}

fn recertify_staged_exact_geometric_constraint_solve(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    positions: &[(VertexId, Point2)],
    expected: GeometricConstraintSolveExactSatisfaction,
) -> Result<(), String> {
    if expected.model_id
        != ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1
        || expected.authorizes_project_mutation
        || expected.replayable_across_runtimes
        || positions.len() > pattern.vertices.len()
    {
        return Err("exact geometric constraint preview certificate is invalid".to_owned());
    }
    let mut candidate = pattern.clone();
    let mut seen = HashSet::with_capacity(positions.len());
    for (vertex, position) in positions {
        if !position.x.is_finite() || !position.y.is_finite() || !seen.insert(*vertex) {
            return Err("exact geometric constraint preview assignment is invalid".to_owned());
        }
        let target = candidate
            .vertices
            .iter_mut()
            .find(|candidate| candidate.id == *vertex)
            .ok_or_else(|| "exact geometric constraint preview assignment is invalid".to_owned())?;
        target.position = *position;
    }
    let certificate =
        ori_core::certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
            .map_err(|_| "exact geometric constraint preview could not be re-certified".to_owned())?
            .ok_or_else(|| {
                "exact geometric constraint preview could not be re-certified".to_owned()
            })?;
    if certificate.constraint_count() != expected.constraint_count
        || certificate.equation_count() != expected.equation_count
    {
        return Err("exact geometric constraint preview certificate is stale".to_owned());
    }
    Ok(())
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

#[cfg(test)]
#[path = "geometric_constraint_commands_exact_tests.rs"]
mod exact_satisfaction_tests;
