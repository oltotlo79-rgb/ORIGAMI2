//! Private native command boundary for 2D crease-pattern editing.
//!
//! It owns benchmark, vertex/edge transforms, arrays, cutting, and topology edits;
//! only `pub(super)` handler entries and test regression hooks leave this module.

use super::*;

const MAX_BENCHMARK_EDGE_COUNT: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct PatternResponse {
    pub(super) requested_edge_count: usize,
    pub(super) vertex_count: usize,
    pub(super) edge_count: usize,
    pub(super) vertices: Vec<BenchmarkVertex>,
    pub(super) edges: Vec<BenchmarkEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct BenchmarkVertex {
    pub(super) id: String,
    pub(super) position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct BenchmarkEdge {
    pub(super) id: String,
    pub(super) start: String,
    pub(super) end: String,
    pub(super) kind: EdgeKind,
}

#[tauri::command]
pub(super) fn generate_benchmark_pattern(edge_count: usize) -> PatternResponse {
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
pub(super) fn add_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    x: f64,
    y: f64,
    x_expression: String,
    y_expression: String,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(&x_expression, &y_expression, x, y)?;
    let id = VertexId::new();
    execute_expected_command(
        &mut project,
        expectation,
        Command::AddVertex {
            id,
            position: Point2::new(x, y),
        },
    )?;
    project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
        id,
        x_expression,
        y_expression,
        x,
        y,
    ));
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn move_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: VertexId,
    x: f64,
    y: f64,
    x_expression: String,
    y_expression: String,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(&x_expression, &y_expression, x, y)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::MoveVertex {
            id,
            position: Point2::new(x, y),
        },
    )?;
    project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
        id,
        x_expression,
        y_expression,
        x,
        y,
    ));
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn move_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    delta_x_expression: String,
    delta_y_expression: String,
    delta_x_mm: f64,
    delta_y_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(
        &delta_x_expression,
        &delta_y_expression,
        delta_x_mm,
        delta_y_mm,
    )?;
    let edge = project
        .editor
        .pattern()
        .edges
        .iter()
        .find(|edge| edge.id == id)
        .cloned()
        .ok_or_else(|| "edge not found".to_owned())?;
    let position = |vertex_id| {
        project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == vertex_id)
            .map(|vertex| vertex.position)
            .ok_or_else(|| "vertex not found".to_owned())
    };
    let start = position(edge.start)?;
    let end = position(edge.end)?;
    let start_position = Point2::new(start.x + delta_x_mm, start.y + delta_y_mm);
    let end_position = Point2::new(end.x + delta_x_mm, end.y + delta_y_mm);
    if !start_position.x.is_finite()
        || !start_position.y.is_finite()
        || !end_position.x.is_finite()
        || !end_position.y.is_finite()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::MoveEdge {
            id,
            start_position,
            end_position,
        },
    )?;
    for (vertex, previous, adopted) in [
        (edge.start, start, start_position),
        (edge.end, end, end_position),
    ] {
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex,
            format!("({})+({delta_x_expression})", previous.x),
            format!("({})+({delta_y_expression})", previous.y),
            adopted.x,
            adopted.y,
        ));
    }
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn mirror_edge_left_right(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    axis_x_expression: String,
    axis_x_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let (evaluated, _) = evaluate_finite_millimetre_pair(axis_x_expression.clone(), "0".to_owned())
        .map_err(map_loaded_numeric_expression_error)?;
    if evaluated.to_bits() != axis_x_mm.to_bits() {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    transform_edge_points(
        state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        id,
        |point| mirror_point_left_right(point, axis_x_mm),
        |point| {
            (
                format!("2*({axis_x_expression})-({})", point.x),
                point.y.to_string(),
            )
        },
    )
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MirrorSelectionRequestV1 {
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
    axis: MirrorAxisV1,
    mode: MirrorSelectionModeV1,
    new_vertices: Vec<VertexId>,
    new_edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct MirrorSelectionPreflightV1 {
    allowed: bool,
    mode: MirrorSelectionModeV1,
    vertex_count: usize,
    edge_count: usize,
    issue: Option<&'static str>,
}

fn validate_mirror_selection_request_v1(
    request: &MirrorSelectionRequestV1,
) -> MirrorSelectionPreflightV1 {
    let finite_axis = request.axis.start.x.is_finite()
        && request.axis.start.y.is_finite()
        && request.axis.end.x.is_finite()
        && request.axis.end.y.is_finite();
    let nondegenerate_axis = finite_axis
        && (request.axis.start.x != request.axis.end.x
            || request.axis.start.y != request.axis.end.y);
    let canonical_vertices = request
        .vertices
        .windows(2)
        .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes());
    let canonical_edges = request
        .edges
        .windows(2)
        .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes());
    let new_vertex_count_ok = match request.mode {
        MirrorSelectionModeV1::Move => request.new_vertices.is_empty(),
        MirrorSelectionModeV1::Duplicate => {
            request.new_vertices.len() == request.vertices.len()
                && request
                    .new_vertices
                    .windows(2)
                    .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
        }
    };
    let new_edge_count_ok = match request.mode {
        MirrorSelectionModeV1::Move => request.new_edges.is_empty(),
        MirrorSelectionModeV1::Duplicate => {
            request.new_edges.len() == request.edges.len()
                && request
                    .new_edges
                    .windows(2)
                    .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
        }
    };
    let issue = if !nondegenerate_axis {
        Some("invalid_axis")
    } else if request.vertices.is_empty() && request.edges.is_empty() {
        Some("empty_selection")
    } else if !canonical_vertices || !canonical_edges {
        Some("noncanonical_selection")
    } else if !new_vertex_count_ok || !new_edge_count_ok {
        Some("invalid_new_ids")
    } else {
        None
    };
    MirrorSelectionPreflightV1 {
        allowed: issue.is_none(),
        mode: request.mode,
        vertex_count: request.vertices.len(),
        edge_count: request.edges.len(),
        issue,
    }
}

#[tauri::command]
pub(super) fn preflight_mirror_selection(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: MirrorSelectionRequestV1,
) -> Result<MirrorSelectionPreflightV1, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let mut result = validate_mirror_selection_request_v1(&request);
    if result.allowed {
        let mut probe = project.editor.clone();
        if probe
            .execute(
                expected_revision,
                Command::MirrorSelection {
                    vertices: request.vertices.clone(),
                    edges: request.edges.clone(),
                    axis: request.axis,
                    mode: request.mode,
                    new_vertices: request.new_vertices.clone(),
                    new_edges: request.new_edges.clone(),
                },
            )
            .is_err()
        {
            result.allowed = false;
            result.issue = Some("core_rejected");
        }
    }
    Ok(result)
}

#[tauri::command]
pub(super) fn apply_mirror_selection(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: MirrorSelectionRequestV1,
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
        Command::MirrorSelection {
            vertices: request.vertices,
            edges: request.edges,
            axis: request.axis,
            mode: request.mode,
            new_vertices: request.new_vertices,
            new_edges: request.new_edges,
        },
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LinearArrayRequestV1 {
    pub(super) vertices: Vec<VertexId>,
    pub(super) edges: Vec<EdgeId>,
    pub(super) additional_copies: u8,
    pub(super) delta: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct LinearArrayPreviewV1 {
    pub(super) version: u8,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) request_sha256: String,
    pub(super) source_vertex_count: usize,
    pub(super) source_edge_count: usize,
    pub(super) additional_copies: u8,
    pub(super) generated_vertex_count: usize,
    pub(super) generated_edge_seed_count: usize,
    pub(super) authorizes_project_mutation: bool,
}

pub(super) fn linear_array_request_sha256(
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    request: &LinearArrayRequestV1,
) -> Result<String, String> {
    let payload = serde_json::to_vec(&(project_instance_id, project_id, revision, request))
        .map_err(|_| "linear_array_request_invalid".to_owned())?;
    Ok(lowercase_hex(&sha2::Sha256::digest(payload)))
}

#[tauri::command]
pub(super) fn preview_linear_array(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: LinearArrayRequestV1,
) -> Result<LinearArrayPreviewV1, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    preview_linear_array_verified(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request,
    )
}

#[cfg(test)]
pub(super) fn preview_linear_array_inner(
    project: &ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: LinearArrayRequestV1,
) -> Result<LinearArrayPreviewV1, String> {
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    preview_linear_array_verified(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request,
    )
}

fn preview_linear_array_verified(
    project: &ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: LinearArrayRequestV1,
) -> Result<LinearArrayPreviewV1, String> {
    project
        .editor
        .plan_linear_array(
            expected_revision,
            request.vertices.clone(),
            request.edges.clone(),
            request.additional_copies,
            request.delta,
        )
        .map_err(|error| error.to_string())?;
    Ok(LinearArrayPreviewV1 {
        version: 1,
        project_instance_id: expected_project_instance_id,
        project_id: expected_project_id,
        revision: expected_revision,
        request_sha256: linear_array_request_sha256(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            &request,
        )?,
        source_vertex_count: request.vertices.len(),
        source_edge_count: request.edges.len(),
        additional_copies: request.additional_copies,
        generated_vertex_count: request
            .vertices
            .len()
            .saturating_mul(usize::from(request.additional_copies)),
        generated_edge_seed_count: request
            .edges
            .len()
            .saturating_mul(usize::from(request.additional_copies)),
        authorizes_project_mutation: false,
    })
}

#[tauri::command]
pub(super) fn confirm_linear_array(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: LinearArrayRequestV1,
    expected_request_sha256: String,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    confirm_linear_array_inner(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request,
        expected_request_sha256,
    )
}

pub(super) fn confirm_linear_array_inner(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: LinearArrayRequestV1,
    expected_request_sha256: String,
) -> Result<ProjectSnapshot, String> {
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let live_digest = linear_array_request_sha256(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        &request,
    )?;
    if expected_request_sha256 != live_digest {
        return Err("linear_array_preview_stale".to_owned());
    }
    let command = project
        .editor
        .plan_linear_array(
            expected_revision,
            request.vertices,
            request.edges,
            request.additional_copies,
            request.delta,
        )
        .map_err(|error| error.to_string())?;
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        command,
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RadialArrayRequestV1 {
    pub(super) center: VertexId,
    pub(super) vertices: Vec<VertexId>,
    pub(super) edges: Vec<EdgeId>,
    pub(super) additional_copies: u8,
    pub(super) angle_microdegrees: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct RadialArrayPreviewV1 {
    pub(super) version: u8,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) request_sha256: String,
    pub(super) source_vertex_count: usize,
    pub(super) source_edge_count: usize,
    pub(super) additional_copies: u8,
    pub(super) angle_microdegrees: u32,
    pub(super) authorizes_project_mutation: bool,
}
fn radial_array_request_sha256(
    instance: ProjectId,
    id: ProjectId,
    revision: u64,
    request: &RadialArrayRequestV1,
) -> Result<String, String> {
    let mut digest = sha2::Sha256::new();
    digest.update(b"ORIGAMI2\0radial-array-v1\0");
    digest.update(
        serde_json::to_vec(&(instance, id, revision, request))
            .map_err(|_| "radial_array_request_invalid".to_owned())?,
    );
    Ok(lowercase_hex(&digest.finalize()))
}
#[cfg(test)]
pub(super) fn preview_radial_array_inner(
    project: &ProjectState,
    instance: ProjectId,
    id: ProjectId,
    revision: u64,
    request: RadialArrayRequestV1,
) -> Result<RadialArrayPreviewV1, String> {
    ensure_project_expectation(project, ProjectExpectation::new(instance, id, revision))?;
    preview_radial_array_verified(project, instance, id, revision, request)
}
fn preview_radial_array_verified(
    project: &ProjectState,
    instance: ProjectId,
    id: ProjectId,
    revision: u64,
    request: RadialArrayRequestV1,
) -> Result<RadialArrayPreviewV1, String> {
    project
        .editor
        .plan_radial_array(
            revision,
            request.center,
            request.vertices.clone(),
            request.edges.clone(),
            request.additional_copies,
            request.angle_microdegrees,
        )
        .map_err(|e| e.to_string())?;
    Ok(RadialArrayPreviewV1 {
        version: 1,
        project_instance_id: instance,
        project_id: id,
        revision,
        request_sha256: radial_array_request_sha256(instance, id, revision, &request)?,
        source_vertex_count: request.vertices.len(),
        source_edge_count: request.edges.len(),
        additional_copies: request.additional_copies,
        angle_microdegrees: request.angle_microdegrees,
        authorizes_project_mutation: false,
    })
}
#[tauri::command]
pub(super) fn preview_radial_array(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: RadialArrayRequestV1,
) -> Result<RadialArrayPreviewV1, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    preview_radial_array_verified(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request,
    )
}
pub(super) fn confirm_radial_array_inner(
    project: &mut ProjectState,
    instance: ProjectId,
    id: ProjectId,
    revision: u64,
    request: RadialArrayRequestV1,
    expected_request_sha256: String,
) -> Result<ProjectSnapshot, String> {
    ensure_project_expectation(project, ProjectExpectation::new(instance, id, revision))?;
    if radial_array_request_sha256(instance, id, revision, &request)? != expected_request_sha256 {
        return Err("radial_array_preview_stale".to_owned());
    }
    let command = project
        .editor
        .plan_radial_array(
            revision,
            request.center,
            request.vertices,
            request.edges,
            request.additional_copies,
            request.angle_microdegrees,
        )
        .map_err(|e| e.to_string())?;
    execute_expected_command(
        project,
        ProjectExpectation::new(instance, id, revision),
        command,
    )
}
#[tauri::command]
pub(super) fn confirm_radial_array(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request: RadialArrayRequestV1,
    expected_request_sha256: String,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    confirm_radial_array_inner(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request,
        expected_request_sha256,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(super) fn rotate_edge_about_point(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    center_x_expression: String,
    center_y_expression: String,
    angle_degrees_expression: String,
    center_x_mm: f64,
    center_y_mm: f64,
    angle_degrees: f64,
) -> Result<ProjectSnapshot, String> {
    let (evaluated_x, evaluated_y) =
        evaluate_finite_millimetre_pair(center_x_expression.clone(), center_y_expression.clone())
            .map_err(map_loaded_numeric_expression_error)?;
    let (evaluated_angle, _) =
        evaluate_finite_millimetre_pair(angle_degrees_expression.clone(), "0".to_owned())
            .map_err(map_loaded_numeric_expression_error)?;
    if evaluated_x.to_bits() != center_x_mm.to_bits()
        || evaluated_y.to_bits() != center_y_mm.to_bits()
        || evaluated_angle.to_bits() != angle_degrees.to_bits()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let (sin, cos) = symmetry_sin_cos(angle_degrees);
    if !sin.is_finite() || !cos.is_finite() {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    transform_edge_points(
        state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        id,
        |point| rotate_point_about(point, Point2::new(center_x_mm, center_y_mm), sin, cos),
        |point| {
            (
                format!(
                    "({center_x_expression})+(({})-({center_x_expression}))*cos(({angle_degrees_expression})*pi/180)-(({})-({center_y_expression}))*sin(({angle_degrees_expression})*pi/180)",
                    point.x, point.y
                ),
                format!(
                    "({center_y_expression})+(({})-({center_x_expression}))*sin(({angle_degrees_expression})*pi/180)+(({})-({center_y_expression}))*cos(({angle_degrees_expression})*pi/180)",
                    point.x, point.y
                ),
            )
        },
    )
}

pub(super) fn mirror_point_left_right(point: Point2, axis_x: f64) -> Point2 {
    Point2::new(axis_x.mul_add(2.0, -point.x), point.y)
}

pub(super) fn rotate_point_about(point: Point2, center: Point2, sin: f64, cos: f64) -> Point2 {
    let x = point.x - center.x;
    let y = point.y - center.y;
    Point2::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos)
}

pub(super) fn symmetry_sin_cos(angle_degrees: f64) -> (f64, f64) {
    match angle_degrees.rem_euclid(360.0) {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        angle => angle.to_radians().sin_cos(),
    }
}

fn transform_edge_points(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    transform: impl Fn(Point2) -> Point2,
    expression: impl Fn(Point2) -> (String, String),
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let edge = project
        .editor
        .pattern()
        .edges
        .iter()
        .find(|edge| edge.id == id)
        .cloned()
        .ok_or_else(|| "edge not found".to_owned())?;
    let position = |vertex_id| {
        project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == vertex_id)
            .map(|vertex| vertex.position)
            .ok_or_else(|| "vertex not found".to_owned())
    };
    let start = position(edge.start)?;
    let end = position(edge.end)?;
    let start_position = transform(start);
    let end_position = transform(end);
    if !start_position.x.is_finite()
        || !start_position.y.is_finite()
        || !end_position.x.is_finite()
        || !end_position.y.is_finite()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::MoveEdge {
            id,
            start_position,
            end_position,
        },
    )?;
    for (vertex, previous, adopted) in [
        (edge.start, start, start_position),
        (edge.end, end, end_position),
    ] {
        let (x_source, y_source) = expression(previous);
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex, x_source, y_source, adopted.x, adopted.y,
        ));
    }
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn move_vertices(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertices: Vec<VertexId>,
    delta_x_expression: String,
    delta_y_expression: String,
    delta_x_mm: f64,
    delta_y_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(
        &delta_x_expression,
        &delta_y_expression,
        delta_x_mm,
        delta_y_mm,
    )?;
    if vertices.is_empty() || vertices.len() > ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let mut unique = HashSet::with_capacity(vertices.len());
    let mut planned = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        if !unique.insert(vertex) {
            return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
        }
        let previous = project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|candidate| candidate.id == vertex)
            .map(|candidate| candidate.position)
            .ok_or_else(|| "vertex not found".to_owned())?;
        let position = Point2::new(previous.x + delta_x_mm, previous.y + delta_y_mm);
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
        }
        planned.push((vertex, previous, position));
    }
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::MoveVertices {
            updates: planned
                .iter()
                .map(|(vertex, _, position)| VertexPositionUpdate {
                    vertex: *vertex,
                    position: *position,
                })
                .collect(),
        },
    )?;
    for (vertex, previous, adopted) in planned {
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex,
            format!("({})+({delta_x_expression})", previous.x),
            format!("({})+({delta_y_expression})", previous.y),
            adopted.x,
            adopted.y,
        ));
    }
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn remove_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: VertexId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(&mut project, expectation, Command::RemoveVertex { id })?;
    project.remove_vertex_coordinate_expression(id);
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn add_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    start: VertexId,
    end: VertexId,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    let command = project
        .editor
        .plan_add_edge_with_intersections(expected_revision, EdgeId::new(), start, end, kind)
        .map_err(|error| error.to_string())?;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        command,
    )
}

#[tauri::command]
pub(super) fn add_ray_to_first_target(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    start: VertexId,
    angle_microdegrees: u32,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    let command = project
        .editor
        .plan_add_ray_to_first_target(expected_revision, start, angle_microdegrees, kind)
        .map_err(|error| error.to_string())?;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        command,
    )
}

#[tauri::command]
pub(super) fn add_connected_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    start: VertexId,
    length_expression: String,
    angle_degrees_expression: String,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let (length_mm, angle_degrees) = evaluate_finite_millimetre_pair(
        length_expression.clone(),
        angle_degrees_expression.clone(),
    )
    .map_err(map_loaded_numeric_expression_error)?;
    if length_mm <= 0.0 || angle_degrees.abs() > 360_000.0 {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let start_position = project
        .editor
        .pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == start)
        .map(|vertex| vertex.position)
        .ok_or_else(|| PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())?;
    let endpoint = deterministic_polar_endpoint_with_model_support(
        start_position,
        length_mm,
        angle_degrees,
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    )?;
    let x = endpoint.x;
    let y = endpoint.y;
    let vertex_id = VertexId::new();
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::AddConnectedVertex {
            vertex_id,
            position: Point2::new(x, y),
            edge_id: EdgeId::new(),
            start,
            kind,
        },
    )?;
    let mut binding =
        VertexCoordinateExpressions::new(vertex_id, x.to_string(), y.to_string(), x, y);
    binding.schema_version =
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2;
    binding.transcendental_model_id =
        Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.to_owned());
    binding.polar_construction = Some(PolarVertexConstructionExpressions {
        schema_version: ori_formats::PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION,
        start_vertex: start,
        adopted_start_x_mm: start_position.x,
        adopted_start_y_mm: start_position.y,
        length_source: length_expression,
        angle_degrees_source: angle_degrees_expression,
        adopted_length_mm: length_mm,
        adopted_angle_degrees: angle_degrees,
    });
    project.adopt_vertex_coordinate_expression(binding);
    Ok(snapshot(&project))
}

pub(super) fn deterministic_polar_endpoint_with_model_support(
    start: Point2,
    length_mm: f64,
    angle_degrees: f64,
    deterministic_model_supported: bool,
) -> Result<Point2, String> {
    if !deterministic_model_supported {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let (x, y) =
        ori_numeric::deterministic_polar_endpoint_v2(start.x, start.y, length_mm, angle_degrees)
            .map_err(|_| PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())?;
    Ok(Point2::new(x, y))
}

#[tauri::command]
pub(super) fn remove_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(&mut project, expectation, Command::RemoveEdge { id })
}

#[tauri::command]
pub(super) fn set_cutting_allowed(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    allowed: bool,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::SetCuttingAllowed { allowed },
    )
}

#[tauri::command]
pub(super) fn resize_rectangular_paper(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    width_expression: String,
    height_expression: String,
    width_mm: f64,
    height_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let (evaluated_width_mm, evaluated_height_mm) =
        evaluate_positive_millimetre_pair(width_expression.clone(), height_expression.clone())
            .map_err(map_loaded_numeric_expression_error)?;
    if evaluated_width_mm.to_bits() != width_mm.to_bits()
        || evaluated_height_mm.to_bits() != height_mm.to_bits()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::ResizeRectangularPaper {
            width_mm,
            height_mm,
        },
    )?;
    project.numeric_expressions.rectangular_paper_creation =
        Some(RectangularPaperCreationExpressions::new(
            width_expression,
            height_expression,
            width_mm,
            height_mm,
        ));
    Ok(snapshot(&project))
}

#[tauri::command]
pub(super) fn split_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_edge_split(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

pub(super) fn execute_edge_split(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::SplitEdge {
            edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction,
        },
    )
}

#[tauri::command]
pub(super) fn connect_edge_intersection(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let mut project = lock_project(&state)?;
    execute_edge_intersection_connection(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

pub(super) fn execute_edge_intersection_connection(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let vertex_id = VertexId::new();
    let snapshot = execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
pub(super) fn connect_intersection_cluster(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    validate_intersection_cluster_target_count(targets.len())?;
    let mut project = lock_project(&state)?;
    execute_intersection_cluster_connection(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        targets,
        junction_vertex_id,
    )
}

#[tauri::command]
pub(super) fn repair_all_unsplit_intersections(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_project_instance_identity(&project, expected_project_instance_id, expected_project_id)?;
    let before_pattern = project.editor.pattern().clone();
    let before_paper = project.editor.paper().clone();
    let authority = project.applied_pose_authority.clone();
    let invalidation = authority
        .begin_invalidation()
        .map_err(|error| error.to_string())?;
    project
        .editor
        .repair_all_unsplit_intersections(expected_revision)
        .map_err(|error| error.to_string())?;
    project.record_numeric_expression_edit();
    project.reconcile_vertex_coordinate_expressions();
    project.current_layer_evidence = None;
    commit_editor_pose_and_proof_invalidation_v1(
        invalidation,
        expected_revision,
        &before_pattern,
        &before_paper,
        &project,
    );
    Ok(snapshot(&project))
}

pub(super) fn execute_intersection_cluster_connection(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
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
    let snapshot = execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
pub(super) fn connect_t_junction(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let mut project = lock_project(&state)?;
    execute_t_junction_connection(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

pub(super) fn execute_t_junction_connection(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let new_edge = EdgeId::new();
    let snapshot = execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
        .ok_or_else(|| "プロジェクト更新結果が不整合です。".to_owned())?;
    Ok(TJunctionResponse {
        snapshot,
        vertex_id,
    })
}

#[tauri::command]
pub(super) fn split_boundary_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_boundary_split(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

pub(super) fn execute_boundary_split(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::SplitBoundaryEdge {
            edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction,
        },
    )
}

#[tauri::command]
pub(super) fn remove_boundary_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertex: VertexId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::RemoveBoundaryVertex { vertex },
    )?;
    project.remove_vertex_coordinate_expression(vertex);
    Ok(snapshot(&project))
}
