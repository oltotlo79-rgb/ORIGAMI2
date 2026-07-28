//! Native authority for canvas constructions that depend on transcendental
//! arithmetic.
//!
//! The WebView may preview a point, but it only sends source element IDs and
//! the original construction scalars here. This module re-reads the
//! revision-bound geometry, reconstructs the point with the frozen numeric
//! model, verifies whether the native result is an add or a unique split, and
//! commits exactly one existing editor command.

use super::*;
use num_bigint::BigUint;

const CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1: u32 = 1;
const CONSTRUCTED_VERTEX_MODEL_ID_V1: &str = "ori_canvas_constructed_vertex_binary64_native_v1";
const CONSTRUCTED_VERTEX_INVALID_MESSAGE: &str = "constructed_vertex_request_invalid";
const MAX_CONSTRUCTION_COORDINATE_ABS_V1: f64 = 1.0e150;
const MAX_CONSTRUCTION_RADIUS_V1: f64 = 1.0e150;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PlaceConstructedVertexRequestV1 {
    schema_version: u32,
    construction_model_id: String,
    transcendental_model_id: String,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_placement: ExpectedConstructedPlacementV1,
    construction: ConstructedVertexSourceV1,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct MoveConstructedVertexRequestV1 {
    schema_version: u32,
    construction_model_id: String,
    transcendental_model_id: String,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertex_id: VertexId,
    construction: ConstructedVertexSourceV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum ExpectedConstructedPlacementV1 {
    Add,
    SplitEdge { edge_id: EdgeId },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum AngleSideV1 {
    Counterclockwise,
    Clockwise,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum AngleReferenceKindV1 {
    GlobalHorizontal,
    Edge,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum ConstructedVertexSourceV1 {
    Angle {
        anchor_id: VertexId,
        raw_x: f64,
        raw_y: f64,
        angle_degrees: f64,
        angle_side: AngleSideV1,
        reference_kind: AngleReferenceKindV1,
        reference_edge_id: Option<EdgeId>,
    },
    CircleLine {
        center_vertex_id: VertexId,
        radius: f64,
        edge_id: EdgeId,
        root_side: u8,
    },
    CircleCircle {
        first_center_vertex_id: VertexId,
        first_radius: f64,
        second_center_vertex_id: VertexId,
        second_radius: f64,
        intersection_side: u8,
    },
}

#[tauri::command]
pub(super) fn place_constructed_vertex_v1(
    state: State<'_, AppState>,
    request: PlaceConstructedVertexRequestV1,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    place_constructed_vertex_inner_v1(&mut project, request)
}

#[tauri::command]
pub(super) fn move_constructed_vertex_v1(
    state: State<'_, AppState>,
    request: MoveConstructedVertexRequestV1,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    move_constructed_vertex_inner_v1(&mut project, request)
}

fn place_constructed_vertex_inner_v1(
    project: &mut ProjectState,
    request: PlaceConstructedVertexRequestV1,
) -> Result<ProjectSnapshot, String> {
    place_constructed_vertex_inner_with_model_support_v1(
        project,
        request,
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    )
}

fn place_constructed_vertex_inner_with_model_support_v1(
    project: &mut ProjectState,
    request: PlaceConstructedVertexRequestV1,
    deterministic_model_supported: bool,
) -> Result<ProjectSnapshot, String> {
    validate_request_model_with_support_v1(&request, deterministic_model_supported)?;
    let expectation = ProjectExpectation::new(
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    );
    ensure_project_expectation(project, expectation)?;

    let point = reconstruct_point_v1(project.editor.pattern(), &request.construction)?;
    if !finite_bounded_point_v1(point) {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    ensure_point_within_current_paper_v1(project.editor.pattern(), project.editor.paper(), point)?;
    let native_placement = classify_native_placement_v1(project.editor.pattern(), point)?;
    if native_placement != request.expected_placement {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }

    match native_placement {
        ExpectedConstructedPlacementV1::Add => {
            let id = VertexId::new();
            let expression = prepare_native_vertex_expression_v1(id, point)?;
            execute_expected_command(
                project,
                expectation,
                Command::AddVertex {
                    id,
                    position: point,
                },
            )?;
            project.adopt_vertex_coordinate_expression(expression);
            Ok(snapshot(project))
        }
        ExpectedConstructedPlacementV1::SplitEdge { edge_id } => {
            let (edge, start, end) = unique_edge_geometry_v1(project.editor.pattern(), edge_id)?;
            let fraction = strict_segment_fraction_v1(start, end, point)
                .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
            let new_vertex = VertexId::new();
            let new_edge = EdgeId::new();
            // `SplitEdge` and `SplitBoundaryEdge` persist this exact operation
            // order. Prepare the expression before mutation so a successful
            // command has no fallible post-commit bookkeeping.
            let persisted_point = Point2::new(
                stable_convex_combination_v1(start.x, end.x, fraction),
                stable_convex_combination_v1(start.y, end.y, fraction),
            );
            let expression = prepare_native_vertex_expression_v1(new_vertex, persisted_point)?;
            let command = if edge.kind == EdgeKind::Boundary {
                Command::SplitBoundaryEdge {
                    edge: edge_id,
                    new_vertex,
                    new_edge,
                    fraction,
                }
            } else {
                Command::SplitEdge {
                    edge: edge_id,
                    new_vertex,
                    new_edge,
                    fraction,
                }
            };
            execute_expected_command(project, expectation, command)?;
            project.adopt_vertex_coordinate_expression(expression);
            Ok(snapshot(project))
        }
    }
}

fn move_constructed_vertex_inner_v1(
    project: &mut ProjectState,
    request: MoveConstructedVertexRequestV1,
) -> Result<ProjectSnapshot, String> {
    move_constructed_vertex_inner_with_model_support_v1(
        project,
        request,
        ori_numeric::deterministic_transcendental_model_supported_v1(),
    )
}

fn move_constructed_vertex_inner_with_model_support_v1(
    project: &mut ProjectState,
    request: MoveConstructedVertexRequestV1,
    deterministic_model_supported: bool,
) -> Result<ProjectSnapshot, String> {
    validate_models_with_support_v1(
        request.schema_version,
        &request.construction_model_id,
        &request.transcendental_model_id,
        deterministic_model_supported,
    )?;
    let expectation = ProjectExpectation::new(
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    );
    ensure_project_expectation(project, expectation)?;
    let ConstructedVertexSourceV1::Angle { anchor_id, .. } = &request.construction else {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    };
    if *anchor_id != request.vertex_id {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let construction = request.construction;
    let current_position = unique_vertex_position_v1(project.editor.pattern(), request.vertex_id)?;
    let point = reconstruct_point_v1(project.editor.pattern(), &construction)?;
    if !finite_bounded_point_v1(point) || point == current_position {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    ensure_point_within_current_paper_v1(project.editor.pattern(), project.editor.paper(), point)?;
    if project
        .editor
        .pattern()
        .vertices
        .iter()
        .any(|vertex| vertex.id != request.vertex_id && vertex.position == point)
        || point_lies_on_any_current_edge_v1(project.editor.pattern(), point)?
    {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }

    let expression = prepare_native_vertex_expression_v1(request.vertex_id, point)?;
    execute_expected_command(
        project,
        expectation,
        Command::MoveVertex {
            id: request.vertex_id,
            position: point,
        },
    )?;
    project.adopt_vertex_coordinate_expression(expression);
    Ok(snapshot(project))
}

fn validate_request_model_with_support_v1(
    request: &PlaceConstructedVertexRequestV1,
    deterministic_model_supported: bool,
) -> Result<(), String> {
    validate_models_with_support_v1(
        request.schema_version,
        &request.construction_model_id,
        &request.transcendental_model_id,
        deterministic_model_supported,
    )
}

fn validate_models_with_support_v1(
    schema_version: u32,
    construction_model_id: &str,
    transcendental_model_id: &str,
    deterministic_model_supported: bool,
) -> Result<(), String> {
    if !deterministic_model_supported
        || schema_version != CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1
        || construction_model_id != CONSTRUCTED_VERTEX_MODEL_ID_V1
        || transcendental_model_id != ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    Ok(())
}

fn reconstruct_point_v1(
    pattern: &CreasePattern,
    construction: &ConstructedVertexSourceV1,
) -> Result<Point2, String> {
    match *construction {
        ConstructedVertexSourceV1::Angle {
            anchor_id,
            raw_x,
            raw_y,
            angle_degrees,
            angle_side,
            reference_kind,
            reference_edge_id,
        } => reconstruct_angle_point_v1(
            pattern,
            anchor_id,
            Point2::new(raw_x, raw_y),
            angle_degrees,
            angle_side,
            reference_kind,
            reference_edge_id,
        ),
        ConstructedVertexSourceV1::CircleLine {
            center_vertex_id,
            radius,
            edge_id,
            root_side,
        } => {
            reconstruct_circle_line_point_v1(pattern, center_vertex_id, radius, edge_id, root_side)
        }
        ConstructedVertexSourceV1::CircleCircle {
            first_center_vertex_id,
            first_radius,
            second_center_vertex_id,
            second_radius,
            intersection_side,
        } => reconstruct_circle_circle_point_v1(
            pattern,
            first_center_vertex_id,
            first_radius,
            second_center_vertex_id,
            second_radius,
            intersection_side,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_angle_point_v1(
    pattern: &CreasePattern,
    anchor_id: VertexId,
    raw: Point2,
    angle_degrees: f64,
    angle_side: AngleSideV1,
    reference_kind: AngleReferenceKindV1,
    reference_edge_id: Option<EdgeId>,
) -> Result<Point2, String> {
    let anchor = unique_vertex_position_v1(pattern, anchor_id)?;
    if !finite_bounded_point_v1(raw)
        || raw == anchor
        || !angle_degrees.is_finite()
        || !(0.0..=90.0).contains(&angle_degrees)
        || angle_degrees == 0.0
        || (angle_degrees == 90.0 && angle_side != AngleSideV1::Counterclockwise)
    {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }

    let base = match (reference_kind, reference_edge_id) {
        (AngleReferenceKindV1::GlobalHorizontal, None) => Point2::new(1.0, 0.0),
        (AngleReferenceKindV1::Edge, Some(edge_id)) => {
            let (_, first, second) = unique_edge_geometry_v1(pattern, edge_id)?;
            let (start, end) = canonical_points_v1(first, second);
            stable_unit_direction_v1(start, end)?
        }
        _ => return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned()),
    };
    let (sine, cosine) =
        ori_numeric::deterministic_sin_cos_degrees_v1(canonical_zero(angle_degrees))
            .map_err(|_| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
    if sine <= 0.0 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let signed_sine = if angle_side == AngleSideV1::Counterclockwise {
        sine
    } else {
        -sine
    };
    let rotated = Point2::new(
        base.x * cosine - base.y * signed_sine,
        base.x * signed_sine + base.y * cosine,
    );
    let direction = normalize_direction_v1(rotated)?;
    project_onto_anchored_direction_v1(raw, anchor, direction)
}

fn reconstruct_circle_line_point_v1(
    pattern: &CreasePattern,
    center_vertex_id: VertexId,
    radius: f64,
    edge_id: EdgeId,
    root_side: u8,
) -> Result<Point2, String> {
    validate_radius_v1(radius)?;
    if root_side > 1 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let center = unique_vertex_position_v1(pattern, center_vertex_id)?;
    let (_, start, end) = unique_edge_geometry_v1(pattern, edge_id)?;
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let ox = start.x - center.x;
    let oy = start.y - center.y;
    let length_squared = dx * dx + dy * dy;
    let b = 2.0 * (ox * dx + oy * dy);
    let c = ox * ox + oy * oy - radius * radius;
    let discriminant = b * b - 4.0 * length_squared * c;
    if ![dx, dy, ox, oy, length_squared, b, c, discriminant]
        .into_iter()
        .all(f64::is_finite)
        || length_squared <= 0.0
        || discriminant < 0.0
    {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    // `sqrt` is an IEEE-754 correctly rounded basic operation, unlike the
    // transcendental angle kernel. Positive mutations are nevertheless
    // admitted only after the release-target support gate above, and the
    // resulting binary64 coordinate bits are persisted as expressions.
    let root = discriminant.sqrt();
    if !root.is_finite() || (root == 0.0 && root_side != 0) {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let numerator = if root_side == 0 { -b - root } else { -b + root };
    let fraction = numerator / (2.0 * length_squared);
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    finite_point_result_v1(Point2::new(
        start.x + fraction * dx,
        start.y + fraction * dy,
    ))
}

fn reconstruct_circle_circle_point_v1(
    pattern: &CreasePattern,
    first_center_vertex_id: VertexId,
    first_radius: f64,
    second_center_vertex_id: VertexId,
    second_radius: f64,
    intersection_side: u8,
) -> Result<Point2, String> {
    validate_radius_v1(first_radius)?;
    validate_radius_v1(second_radius)?;
    if intersection_side > 1 || first_center_vertex_id == second_center_vertex_id {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let first = unique_vertex_position_v1(pattern, first_center_vertex_id)?;
    let second = unique_vertex_position_v1(pattern, second_center_vertex_id)?;
    let dx = second.x - first.x;
    let dy = second.y - first.y;
    let distance = ori_numeric::deterministic_hypot_v1(dx, dy)
        .map_err(|_| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
    if distance == 0.0
        || distance > first_radius + second_radius
        || distance < (first_radius - second_radius).abs()
    {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let along = (first_radius * first_radius - second_radius * second_radius + distance * distance)
        / (2.0 * distance);
    let height_squared = first_radius * first_radius - along * along;
    if !along.is_finite() || !height_squared.is_finite() || height_squared < 0.0 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    // Keep circle construction under the same target gate and persisted-bit
    // contract as the line case; no WebView-computed root is authoritative.
    let height = height_squared.max(0.0).sqrt();
    if !height.is_finite() || (height == 0.0 && intersection_side != 0) {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let base_x = first.x + along * dx / distance;
    let base_y = first.y + along * dy / distance;
    let perpendicular_x = -dy * height / distance;
    let perpendicular_y = dx * height / distance;
    let sign = if intersection_side == 0 { 1.0 } else { -1.0 };
    finite_point_result_v1(Point2::new(
        base_x + sign * perpendicular_x,
        base_y + sign * perpendicular_y,
    ))
}

fn validate_radius_v1(radius: f64) -> Result<(), String> {
    if !radius.is_finite() || radius <= 0.0 || radius > MAX_CONSTRUCTION_RADIUS_V1 {
        Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())
    } else {
        Ok(())
    }
}

fn classify_native_placement_v1(
    pattern: &CreasePattern,
    point: Point2,
) -> Result<ExpectedConstructedPlacementV1, String> {
    let vertices = validated_vertex_positions_v1(pattern)?;
    if vertices.values().any(|position| *position == point) {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let mut edge_ids = HashSet::with_capacity(pattern.edges.len());
    let mut split: Option<EdgeId> = None;
    for edge in &pattern.edges {
        if !edge_ids.insert(edge.id) || edge.start == edge.end {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        let start = vertices
            .get(&edge.start)
            .copied()
            .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
        let end = vertices
            .get(&edge.end)
            .copied()
            .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
        if start == end {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        if point == start || point == end {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        if strict_segment_fraction_v1(start, end, point).is_none() {
            continue;
        }
        if split.replace(edge.id).is_some() {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
    }
    Ok(
        split.map_or(ExpectedConstructedPlacementV1::Add, |edge_id| {
            ExpectedConstructedPlacementV1::SplitEdge { edge_id }
        }),
    )
}

fn point_lies_on_any_current_edge_v1(
    pattern: &CreasePattern,
    point: Point2,
) -> Result<bool, String> {
    let vertices = validated_vertex_positions_v1(pattern)?;
    let mut edge_ids = HashSet::with_capacity(pattern.edges.len());
    for edge in &pattern.edges {
        if !edge_ids.insert(edge.id) || edge.start == edge.end {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        let start = vertices
            .get(&edge.start)
            .copied()
            .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
        let end = vertices
            .get(&edge.end)
            .copied()
            .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
        if start == end {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        if point == start || point == end || strict_segment_fraction_v1(start, end, point).is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validated_vertex_positions_v1(
    pattern: &CreasePattern,
) -> Result<HashMap<VertexId, Point2>, String> {
    let mut positions = HashMap::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        if !finite_bounded_point_v1(vertex.position)
            || positions
                .insert(
                    vertex.id,
                    Point2::new(
                        canonical_zero(vertex.position.x),
                        canonical_zero(vertex.position.y),
                    ),
                )
                .is_some()
        {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
    }
    Ok(positions)
}

fn ensure_point_within_current_paper_v1(
    pattern: &CreasePattern,
    paper: &Paper,
    point: Point2,
) -> Result<(), String> {
    // Unit fixtures built through cfg(test)-only `ProjectState::new` use
    // `Paper::default()`. Invalid persisted projects remain loadable for
    // repair, so the compatibility bypass itself must not exist in production.
    if paper.boundary_vertices.is_empty() {
        #[cfg(test)]
        {
            return Ok(());
        }
        #[cfg(not(test))]
        {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
    }
    if paper.boundary_vertices.len() < 3 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let positions = validated_vertex_positions_v1(pattern)?;
    let mut boundary_ids = HashSet::with_capacity(paper.boundary_vertices.len());
    let mut polygon = Vec::with_capacity(paper.boundary_vertices.len());
    for id in &paper.boundary_vertices {
        if !boundary_ids.insert(*id) {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        polygon.push(
            positions
                .get(id)
                .copied()
                .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?,
        );
    }
    match segment_midpoint_polygon_relation(point, point, &polygon) {
        Ok(PointPolygonRelation::Inside | PointPolygonRelation::Boundary) => Ok(()),
        _ => Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned()),
    }
}

fn unique_vertex_position_v1(pattern: &CreasePattern, id: VertexId) -> Result<Point2, String> {
    let mut matches = pattern.vertices.iter().filter(|vertex| vertex.id == id);
    let position = matches
        .next()
        .map(|vertex| vertex.position)
        .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
    if matches.next().is_some() || !finite_bounded_point_v1(position) {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    Ok(Point2::new(
        canonical_zero(position.x),
        canonical_zero(position.y),
    ))
}

fn unique_edge_geometry_v1(
    pattern: &CreasePattern,
    id: EdgeId,
) -> Result<(ori_domain::Edge, Point2, Point2), String> {
    let mut matches = pattern.edges.iter().filter(|edge| edge.id == id);
    let edge = matches
        .next()
        .cloned()
        .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
    if matches.next().is_some() || edge.start == edge.end {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let start = unique_vertex_position_v1(pattern, edge.start)?;
    let end = unique_vertex_position_v1(pattern, edge.end)?;
    if start == end {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    Ok((edge, start, end))
}

fn prepare_native_vertex_expression_v1(
    vertex: VertexId,
    point: Point2,
) -> Result<VertexCoordinateExpressions, String> {
    let point = finite_point_result_v1(point)?;
    Ok(VertexCoordinateExpressions::new(
        vertex,
        exact_binary64_coordinate_source_v1(point.x)?,
        exact_binary64_coordinate_source_v1(point.y)?,
        canonical_zero(point.x),
        canonical_zero(point.y),
    ))
}

fn exact_binary64_coordinate_source_v1(value: f64) -> Result<String, String> {
    if !value.is_finite() {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    let value = canonical_zero(value);
    if value == 0.0 {
        return Ok("0".to_owned());
    }

    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let biased_exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mut significand, mut binary_exponent) = if biased_exponent == 0 {
        (fraction, -1074)
    } else {
        ((1_u64 << 52) | fraction, biased_exponent - 1023 - 52)
    };
    if significand == 0 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    while significand & 1 == 0 {
        significand >>= 1;
        binary_exponent += 1;
    }

    let magnitude = if binary_exponent >= 0 {
        (BigUint::from(significand)
            << usize::try_from(binary_exponent)
                .map_err(|_| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?)
        .to_string()
    } else {
        let denominator = BigUint::from(1_u8)
            << usize::try_from(-binary_exponent)
                .map_err(|_| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
        format!("{significand} / {denominator}")
    };
    Ok(if negative {
        format!("-{magnitude}")
    } else {
        magnitude
    })
}

fn project_onto_anchored_direction_v1(
    point: Point2,
    anchor: Point2,
    direction: Point2,
) -> Result<Point2, String> {
    let offset = stable_normalized_difference_v1(anchor, point)?;
    let first_term = offset.x * direction.x;
    let second_term = offset.y * direction.y;
    let factor = first_term + second_term;
    let normalized_x = factor * direction.x;
    let normalized_y = factor * direction.y;
    let projected_x = canonical_zero(anchor.x + normalized_x * offset.scale);
    let projected_y = canonical_zero(anchor.y + normalized_y * offset.scale);
    finite_point_result_v1(Point2::new(projected_x, projected_y))
}

#[derive(Clone, Copy)]
struct NormalizedDifferenceV1 {
    x: f64,
    y: f64,
    scale: f64,
}

fn stable_normalized_difference_v1(
    start: Point2,
    end: Point2,
) -> Result<NormalizedDifferenceV1, String> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let scale = dx.abs().max(dy.abs());
    let x = dx / scale;
    let y = dy / scale;
    if ![dx, dy, scale, x, y].into_iter().all(f64::is_finite) || scale <= 0.0 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    Ok(NormalizedDifferenceV1 { x, y, scale })
}

fn stable_unit_direction_v1(start: Point2, end: Point2) -> Result<Point2, String> {
    normalize_direction_v1(stable_direction_components_v1(start, end)?)
}

fn normalize_direction_v1(direction: Point2) -> Result<Point2, String> {
    let length = ori_numeric::deterministic_hypot_v1(direction.x, direction.y)
        .map_err(|_| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())?;
    if length <= 0.0 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    finite_point_result_v1(Point2::new(direction.x / length, direction.y / length))
}

fn stable_direction_components_v1(start: Point2, end: Point2) -> Result<Point2, String> {
    let mut dx = end.x - start.x;
    let mut dy = end.y - start.y;
    if !dx.is_finite() || !dy.is_finite() {
        let scale = start
            .x
            .abs()
            .max(start.y.abs())
            .max(end.x.abs())
            .max(end.y.abs());
        if !scale.is_finite() || scale <= 0.0 {
            return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
        }
        dx = end.x / scale - start.x / scale;
        dy = end.y / scale - start.y / scale;
    }
    let scale = dx.abs().max(dy.abs());
    if !scale.is_finite() || scale <= 0.0 {
        return Err(CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned());
    }
    finite_point_result_v1(Point2::new(dx / scale, dy / scale))
}

fn strict_segment_fraction_v1(start: Point2, end: Point2, point: Point2) -> Option<f64> {
    let midpoint = Point2::new(
        stable_convex_combination_v1(start.x, end.x, 0.5),
        stable_convex_combination_v1(start.y, end.y, 0.5),
    );
    let direction = stable_direction_components_v1(start, end).ok()?;
    if !point_is_on_direction_line_v1(point, midpoint, direction, false) {
        return None;
    }
    let fraction = if direction.x.abs() >= direction.y.abs() {
        (point.x - start.x) / (end.x - start.x)
    } else {
        (point.y - start.y) / (end.y - start.y)
    };
    (fraction.is_finite() && fraction > 0.0 && fraction < 1.0).then_some(fraction)
}

fn point_is_on_direction_line_v1(
    point: Point2,
    anchor: Point2,
    direction: Point2,
    include_coordinate_rounding: bool,
) -> bool {
    let offset_x = point.x - anchor.x;
    let offset_y = point.y - anchor.y;
    let maximum_offset = offset_x.abs().max(offset_y.abs());
    if !offset_x.is_finite() || !offset_y.is_finite() || !maximum_offset.is_finite() {
        return false;
    }
    if maximum_offset == 0.0 {
        return true;
    }
    let normalized_x = offset_x / maximum_offset;
    let normalized_y = offset_y / maximum_offset;
    let first_term = normalized_x * direction.y;
    let second_term = normalized_y * direction.x;
    let cross = first_term - second_term;
    if ![first_term, second_term, cross]
        .into_iter()
        .all(f64::is_finite)
    {
        return false;
    }
    let mut tolerance = 64.0 * f64::EPSILON * (1.0 + first_term.abs() + second_term.abs());
    if include_coordinate_rounding {
        let coordinate_scale = 1.0_f64
            .max(point.x.abs())
            .max(point.y.abs())
            .max(anchor.x.abs())
            .max(anchor.y.abs());
        tolerance += 16.0 * f64::EPSILON * coordinate_scale / maximum_offset
            * (direction.x.abs() + direction.y.abs());
    }
    cross.abs() <= tolerance
}

fn stable_convex_combination_v1(start: f64, end: f64, fraction: f64) -> f64 {
    if start.is_sign_negative() == end.is_sign_negative() {
        start + (end - start) * fraction
    } else {
        start * (1.0 - fraction) + end * fraction
    }
}

fn canonical_points_v1(first: Point2, second: Point2) -> (Point2, Point2) {
    if first.x < second.x || (first.x == second.x && first.y < second.y) {
        (first, second)
    } else {
        (second, first)
    }
}

fn finite_bounded_point_v1(point: Point2) -> bool {
    point.x.is_finite()
        && point.y.is_finite()
        && point.x.abs() <= MAX_CONSTRUCTION_COORDINATE_ABS_V1
        && point.y.abs() <= MAX_CONSTRUCTION_COORDINATE_ABS_V1
}

fn finite_point_result_v1(point: Point2) -> Result<Point2, String> {
    let canonical = Point2::new(canonical_zero(point.x), canonical_zero(point.y));
    finite_bounded_point_v1(canonical)
        .then_some(canonical)
        .ok_or_else(|| CONSTRUCTED_VERTEX_INVALID_MESSAGE.to_owned())
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{Edge, Vertex};
    use ori_formats::{read_project_archive_ori2, write_project_archive_ori2};

    fn vertex(id: VertexId, x: f64, y: f64) -> Vertex {
        Vertex {
            id,
            position: Point2::new(x, y),
        }
    }

    fn request(
        project: &ProjectState,
        expected_placement: ExpectedConstructedPlacementV1,
        construction: ConstructedVertexSourceV1,
    ) -> PlaceConstructedVertexRequestV1 {
        PlaceConstructedVertexRequestV1 {
            schema_version: CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1,
            construction_model_id: CONSTRUCTED_VERTEX_MODEL_ID_V1.to_owned(),
            transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
                .to_owned(),
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            expected_placement,
            construction,
        }
    }

    fn move_request(
        project: &ProjectState,
        vertex_id: VertexId,
        construction: ConstructedVertexSourceV1,
    ) -> MoveConstructedVertexRequestV1 {
        MoveConstructedVertexRequestV1 {
            schema_version: CONSTRUCTED_VERTEX_SCHEMA_VERSION_V1,
            construction_model_id: CONSTRUCTED_VERTEX_MODEL_ID_V1.to_owned(),
            transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
                .to_owned(),
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            vertex_id,
            construction,
        }
    }

    fn empty_project_with(vertices: Vec<Vertex>) -> ProjectState {
        ProjectState::new(CreasePattern {
            vertices,
            edges: Vec::new(),
        })
    }

    fn assert_expression_matches_unique_vertex(project: &ProjectState, vertex_id: VertexId) {
        let vertex = project
            .editor
            .pattern()
            .vertices
            .iter()
            .filter(|vertex| vertex.id == vertex_id)
            .collect::<Vec<_>>();
        let binding = project
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .filter(|binding| binding.vertex == vertex_id)
            .collect::<Vec<_>>();
        assert_eq!(vertex.len(), 1);
        assert_eq!(binding.len(), 1);
        assert_eq!(
            vertex[0].position.x.to_bits(),
            binding[0].adopted_x_mm.to_bits()
        );
        assert_eq!(
            vertex[0].position.y.to_bits(),
            binding[0].adopted_y_mm.to_bits()
        );
    }

    #[test]
    fn angle_add_uses_native_bits_and_one_undoable_history_entry() {
        let anchor = VertexId::new();
        let mut project = empty_project_with(vec![vertex(anchor, 1.0, 2.0)]);
        let construction = ConstructedVertexSourceV1::Angle {
            anchor_id: anchor,
            raw_x: 4.0,
            raw_y: 5.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let expected =
            reconstruct_point_v1(project.editor.pattern(), &construction).expect("native angle");
        let before_count = project.editor.pattern().vertices.len();
        let add_request = request(&project, ExpectedConstructedPlacementV1::Add, construction);
        let response =
            place_constructed_vertex_inner_with_model_support_v1(&mut project, add_request, true)
                .expect("place angle");
        assert_eq!(response.revision, 1);
        assert_eq!(project.editor.pattern().vertices.len(), before_count + 1);
        let added = project
            .editor
            .pattern()
            .vertices
            .last()
            .expect("added vertex");
        assert_eq!(added.position.x.to_bits(), expected.x.to_bits());
        assert_eq!(added.position.y.to_bits(), expected.y.to_bits());
        assert!(project.editor.can_undo());
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
        assert_expression_matches_unique_vertex(&project, added.id);

        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        execute_undo(&mut project, instance_id, project_id, revision).expect("undo");
        assert_eq!(project.editor.pattern().vertices.len(), before_count);
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        let revision = project.editor.revision();
        execute_redo(&mut project, instance_id, project_id, revision).expect("redo");
        assert_eq!(project.editor.pattern().vertices.len(), before_count + 1);
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
    }

    #[test]
    fn unsupported_numeric_target_rejects_place_and_move_before_any_mutation() {
        let anchor = VertexId::new();
        let mut project = empty_project_with(vec![vertex(anchor, 0.0, 0.0)]);
        let construction = ConstructedVertexSourceV1::Angle {
            anchor_id: anchor,
            raw_x: 3.0,
            raw_y: 4.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let place_request = request(
            &project,
            ExpectedConstructedPlacementV1::Add,
            construction.clone(),
        );
        assert!(
            place_constructed_vertex_inner_with_model_support_v1(
                &mut project,
                place_request,
                false,
            )
            .is_err()
        );
        assert_eq!(project.editor.revision(), 0);
        assert_eq!(project.editor.pattern().vertices.len(), 1);
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        assert!(!project.editor.can_undo());

        let move_request = move_request(&project, anchor, construction);
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(&mut project, move_request, false,)
                .is_err()
        );
        assert_eq!(project.editor.revision(), 0);
        assert_eq!(
            unique_vertex_position_v1(project.editor.pattern(), anchor).expect("anchor"),
            Point2::new(0.0, 0.0),
        );
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        assert!(!project.editor.can_undo());
    }

    #[test]
    fn edge_referenced_angle_survives_authenticated_archive_reopen_bit_exactly() {
        let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let anchor = VertexId::new();
        pattern.vertices.push(vertex(anchor, 5.0, 5.0));
        let reference_edge = pattern
            .edges
            .iter()
            .find_map(|edge| {
                let start = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.start)?
                    .position;
                let end = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.end)?
                    .position;
                (edge.kind == EdgeKind::Boundary && start.y == 0.0 && end.y == 0.0)
                    .then_some(edge.id)
            })
            .expect("horizontal boundary reference");
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let construction = ConstructedVertexSourceV1::Angle {
            anchor_id: anchor,
            raw_x: 8.0,
            raw_y: 8.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::Edge,
            reference_edge_id: Some(reference_edge),
        };
        let expected =
            reconstruct_point_v1(project.editor.pattern(), &construction).expect("edge angle");
        let place_request = request(&project, ExpectedConstructedPlacementV1::Add, construction);
        place_constructed_vertex_inner_with_model_support_v1(&mut project, place_request, true)
            .expect("place edge-referenced angle");
        let added_id = project.numeric_expressions.vertex_coordinates[0].vertex;
        assert_expression_matches_unique_vertex(&project, added_id);
        validate_current_project_numeric_expression_bindings_with_model_support(&project, true)
            .expect("reauthenticate the live constructed coordinate");

        let archive = project
            .project_archive_with_geometry_reference_model_support(true)
            .expect("construct archive");
        validate_loaded_numeric_expression_bindings_with_model_support(&archive.document, true)
            .expect("reauthenticate archived expression bindings");
        let bytes = write_project_archive_ori2(&archive).expect("write authenticated archive");
        let decoded = read_project_archive_ori2(&bytes).expect("read authenticated archive");
        validate_loaded_numeric_expression_archive_with_model_support(&decoded, true)
            .expect("reauthenticate archived constructed coordinate");
        let reopened =
            ProjectState::from_project_archive(decoded, PathBuf::from("constructed-angle.ori2"))
                .expect("reopen constructed archive");
        let reopened_position = unique_vertex_position_v1(reopened.editor.pattern(), added_id)
            .expect("reopened vertex");
        assert_eq!(reopened_position.x.to_bits(), expected.x.to_bits());
        assert_eq!(reopened_position.y.to_bits(), expected.y.to_bits());
        assert_expression_matches_unique_vertex(&reopened, added_id);
        assert!(reopened.editor.can_undo());
    }

    #[test]
    fn angle_cardinal_and_adjacent_bits_use_frozen_kernel() {
        let anchor = VertexId::new();
        let project = empty_project_with(vec![vertex(anchor, 0.0, 0.0)]);
        for angle in [
            f64::from_bits(90.0_f64.to_bits() - 1),
            90.0,
            f64::from_bits(90.0_f64.to_bits() + 1),
        ] {
            let side = AngleSideV1::Counterclockwise;
            let result = reconstruct_angle_point_v1(
                project.editor.pattern(),
                anchor,
                Point2::new(3.0, 4.0),
                angle,
                side,
                AngleReferenceKindV1::GlobalHorizontal,
                None,
            );
            if angle > 90.0 {
                assert!(result.is_err());
            } else {
                let point = result.expect("admitted adjacent angle");
                assert!(point.x.is_finite() && point.y.is_finite());
            }
        }
    }

    #[test]
    fn persisted_native_coordinate_sources_round_trip_every_binary64_class_exactly() {
        for value in [
            0.0,
            -0.0,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::from_bits(7.999_999_999_999_999_f64.to_bits()),
            f64::from_bits(1.0_f64.to_bits() + 1),
            1.0e150,
            -1.0e150,
        ] {
            let source = exact_binary64_coordinate_source_v1(value).expect("finite source");
            let (round_trip, zero) =
                evaluate_finite_millimetre_pair(source.clone(), "0".to_owned())
                    .expect("exact dyadic expression");
            assert_eq!(
                round_trip.to_bits(),
                canonical_zero(value).to_bits(),
                "source={source}"
            );
            assert_eq!(zero.to_bits(), 0.0_f64.to_bits());
        }
    }

    #[test]
    fn paper_containment_has_only_a_cfg_test_empty_paper_compatibility_branch() {
        let id = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![vertex(id, 0.0, 0.0)],
            edges: Vec::new(),
        };
        assert!(
            ensure_point_within_current_paper_v1(
                &pattern,
                &Paper::default(),
                Point2::new(1.0, 1.0),
            )
            .is_ok()
        );

        let mut malformed_non_empty = Paper::default();
        malformed_non_empty.boundary_vertices.push(id);
        assert!(
            ensure_point_within_current_paper_v1(
                &pattern,
                &malformed_non_empty,
                Point2::new(0.0, 0.0),
            )
            .is_err()
        );
    }

    #[test]
    fn valid_paper_admits_inside_and_boundary_but_rejects_outside() {
        let sheet = create_rectangular_sheet(10.0, 10.0, false).expect("sheet");
        let (pattern, paper) = sheet.into_parts();
        assert!(
            ensure_point_within_current_paper_v1(&pattern, &paper, Point2::new(5.0, 5.0),).is_ok()
        );
        assert!(
            ensure_point_within_current_paper_v1(&pattern, &paper, Point2::new(0.0, 5.0),).is_ok()
        );
        assert!(
            ensure_point_within_current_paper_v1(&pattern, &paper, Point2::new(-1.0, 5.0),)
                .is_err()
        );
    }

    #[test]
    fn constructed_add_and_move_reject_points_outside_a_valid_paper() {
        let sheet = create_rectangular_sheet(10.0, 10.0, false).expect("sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let moving = VertexId::new();
        pattern.vertices.push(vertex(moving, 5.0, 5.0));
        let boundary_anchor = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == Point2::new(0.0, 0.0))
            .expect("bottom-left")
            .id;
        let mut project = ProjectState::new_with_paper(pattern, paper);

        let add_source = ConstructedVertexSourceV1::Angle {
            anchor_id: boundary_anchor,
            raw_x: -5.0,
            raw_y: -5.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let add_request = request(&project, ExpectedConstructedPlacementV1::Add, add_source);
        assert!(
            place_constructed_vertex_inner_with_model_support_v1(&mut project, add_request, true,)
                .is_err()
        );

        let move_source = ConstructedVertexSourceV1::Angle {
            anchor_id: moving,
            raw_x: 20.0,
            raw_y: 20.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let move_request = move_request(&project, moving, move_source);
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(&mut project, move_request, true,)
                .is_err()
        );
        assert_eq!(project.editor.revision(), 0);
    }

    #[test]
    fn angle_move_reconstructs_natively_and_is_one_undoable_history_entry() {
        let moving = VertexId::new();
        let mut project = empty_project_with(vec![vertex(moving, 1.0, 2.0)]);
        let construction = ConstructedVertexSourceV1::Angle {
            anchor_id: moving,
            raw_x: 4.0,
            raw_y: 5.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let expected = reconstruct_point_v1(project.editor.pattern(), &construction)
            .expect("native move point");
        let move_request = move_request(&project, moving, construction);
        move_constructed_vertex_inner_with_model_support_v1(&mut project, move_request, true)
            .expect("native move");
        assert_eq!(project.editor.revision(), 1);
        assert_eq!(
            unique_vertex_position_v1(project.editor.pattern(), moving)
                .expect("moved")
                .x
                .to_bits(),
            expected.x.to_bits(),
        );
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
        assert_expression_matches_unique_vertex(&project, moving);

        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        execute_undo(&mut project, instance_id, project_id, revision).expect("undo move");
        assert_eq!(
            unique_vertex_position_v1(project.editor.pattern(), moving).expect("original"),
            Point2::new(1.0, 2.0),
        );
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        let revision = project.editor.revision();
        execute_redo(&mut project, instance_id, project_id, revision).expect("redo move");
        assert_eq!(
            unique_vertex_position_v1(project.editor.pattern(), moving)
                .expect("redone")
                .x
                .to_bits(),
            expected.x.to_bits(),
        );
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
    }

    #[test]
    fn angle_move_rejects_stale_forged_missing_and_non_angle_sources() {
        let moving = VertexId::new();
        let other = VertexId::new();
        let angle = ConstructedVertexSourceV1::Angle {
            anchor_id: moving,
            raw_x: 2.0,
            raw_y: 2.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let mut project =
            empty_project_with(vec![vertex(moving, 0.0, 0.0), vertex(other, 10.0, 0.0)]);

        let mut stale = move_request(&project, moving, angle.clone());
        stale.expected_revision += 1;
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(&mut project, stale, true).is_err()
        );

        let mut forged = move_request(&project, moving, angle.clone());
        forged.transcendental_model_id = "forged".to_owned();
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(&mut project, forged, true)
                .is_err()
        );

        let wrong_source = move_request(&project, other, angle);
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(&mut project, wrong_source, true,)
                .is_err()
        );

        let missing = VertexId::new();
        let missing_source = ConstructedVertexSourceV1::Angle {
            anchor_id: missing,
            raw_x: 1.0,
            raw_y: 1.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let missing_request = move_request(&project, missing, missing_source);
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(
                &mut project,
                missing_request,
                true,
            )
            .is_err()
        );

        let circle = ConstructedVertexSourceV1::CircleCircle {
            first_center_vertex_id: moving,
            first_radius: 1.0,
            second_center_vertex_id: other,
            second_radius: 1.0,
            intersection_side: 0,
        };
        let circle_request = move_request(&project, moving, circle);
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(
                &mut project,
                circle_request,
                true,
            )
            .is_err()
        );

        let no_op_source = ConstructedVertexSourceV1::Angle {
            anchor_id: moving,
            raw_x: 1.0,
            raw_y: 0.0,
            angle_degrees: 90.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let no_op_request = move_request(&project, moving, no_op_source);
        assert!(
            move_constructed_vertex_inner_with_model_support_v1(&mut project, no_op_request, true,)
                .is_err()
        );
        assert_eq!(project.editor.revision(), 0);
    }

    #[test]
    fn circle_line_tangent_has_one_branch_and_two_roots_are_distinct() {
        let center = VertexId::new();
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                vertex(center, 0.0, 0.0),
                vertex(start, -10.0, 5.0),
                vertex(end, 10.0, 5.0),
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        };
        let tangent =
            reconstruct_circle_line_point_v1(&pattern, center, 5.0, edge, 0).expect("tangent");
        assert_eq!(tangent, Point2::new(0.0, 5.0));
        assert!(reconstruct_circle_line_point_v1(&pattern, center, 5.0, edge, 1).is_err());

        let first =
            reconstruct_circle_line_point_v1(&pattern, center, 6.0, edge, 0).expect("first root");
        let second =
            reconstruct_circle_line_point_v1(&pattern, center, 6.0, edge, 1).expect("second root");
        assert_ne!(first.x.to_bits(), second.x.to_bits());
        assert_eq!(first.y.to_bits(), second.y.to_bits());
    }

    #[test]
    fn circle_circle_branches_and_tangent_are_unambiguous() {
        let first_id = VertexId::new();
        let second_id = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![vertex(first_id, 0.0, 0.0), vertex(second_id, 6.0, 0.0)],
            edges: Vec::new(),
        };
        let upper = reconstruct_circle_circle_point_v1(&pattern, first_id, 5.0, second_id, 5.0, 0)
            .expect("upper");
        let lower = reconstruct_circle_circle_point_v1(&pattern, first_id, 5.0, second_id, 5.0, 1)
            .expect("lower");
        assert_eq!(upper.x.to_bits(), lower.x.to_bits());
        assert_eq!(upper.y.to_bits(), (-lower.y).to_bits());

        let tangent_pattern = CreasePattern {
            vertices: vec![vertex(first_id, 0.0, 0.0), vertex(second_id, 10.0, 0.0)],
            edges: Vec::new(),
        };
        assert!(
            reconstruct_circle_circle_point_v1(&tangent_pattern, first_id, 5.0, second_id, 5.0, 0,)
                .is_ok()
        );
        assert!(
            reconstruct_circle_circle_point_v1(&tangent_pattern, first_id, 5.0, second_id, 5.0, 1,)
                .is_err()
        );
    }

    #[test]
    fn stale_forged_degenerate_and_expected_operation_mismatch_fail_closed() {
        let anchor = VertexId::new();
        let construction = ConstructedVertexSourceV1::Angle {
            anchor_id: anchor,
            raw_x: 2.0,
            raw_y: 1.0,
            angle_degrees: 45.0,
            angle_side: AngleSideV1::Counterclockwise,
            reference_kind: AngleReferenceKindV1::GlobalHorizontal,
            reference_edge_id: None,
        };
        let mut project = empty_project_with(vec![vertex(anchor, 0.0, 0.0)]);

        let mut stale = request(
            &project,
            ExpectedConstructedPlacementV1::Add,
            construction.clone(),
        );
        stale.expected_revision += 1;
        assert!(
            place_constructed_vertex_inner_with_model_support_v1(&mut project, stale, true)
                .is_err()
        );

        let mut forged = request(
            &project,
            ExpectedConstructedPlacementV1::Add,
            construction.clone(),
        );
        forged.construction_model_id = "forged".to_owned();
        assert!(
            place_constructed_vertex_inner_with_model_support_v1(&mut project, forged, true)
                .is_err()
        );

        let missing_edge = EdgeId::new();
        let mismatch = request(
            &project,
            ExpectedConstructedPlacementV1::SplitEdge {
                edge_id: missing_edge,
            },
            construction,
        );
        assert!(
            place_constructed_vertex_inner_with_model_support_v1(&mut project, mismatch, true,)
                .is_err()
        );
        assert_eq!(project.editor.revision(), 0);
        assert_eq!(project.editor.pattern().vertices.len(), 1);
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        assert!(!project.editor.can_undo());

        assert!(
            reconstruct_circle_circle_point_v1(
                project.editor.pattern(),
                anchor,
                1.0,
                anchor,
                2.0,
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn native_split_uses_current_edge_and_preserves_single_history_step() {
        let center = VertexId::new();
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![
                vertex(center, 0.0, 0.0),
                vertex(start, -10.0, 5.0),
                vertex(end, 10.0, 5.0),
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        });
        let construction = ConstructedVertexSourceV1::CircleLine {
            center_vertex_id: center,
            radius: 5.0,
            edge_id: edge,
            root_side: 0,
        };
        let before_vertices = project.editor.pattern().vertices.len();
        let before_edges = project.editor.pattern().edges.len();
        let split_request = request(
            &project,
            ExpectedConstructedPlacementV1::SplitEdge { edge_id: edge },
            construction,
        );
        place_constructed_vertex_inner_with_model_support_v1(&mut project, split_request, true)
            .expect("native split");
        assert_eq!(project.editor.revision(), 1);
        assert_eq!(project.editor.pattern().vertices.len(), before_vertices + 1);
        assert_eq!(project.editor.pattern().edges.len(), before_edges + 1);
        assert!(project.editor.can_undo());
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
        let split_vertex = project.numeric_expressions.vertex_coordinates[0].vertex;
        assert_expression_matches_unique_vertex(&project, split_vertex);

        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        execute_undo(&mut project, instance_id, project_id, revision).expect("undo split");
        assert_eq!(project.editor.pattern().vertices.len(), before_vertices);
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        let revision = project.editor.revision();
        execute_redo(&mut project, instance_id, project_id, revision).expect("redo split");
        assert_eq!(project.editor.pattern().vertices.len(), before_vertices + 1);
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
    }

    #[test]
    fn native_boundary_split_uses_sheet_operation_and_preserves_cutting_policy() {
        let sheet = create_rectangular_sheet(10.0, 10.0, false).expect("sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let center = VertexId::new();
        pattern.vertices.push(vertex(center, 5.0, 5.0));
        let target = pattern
            .edges
            .iter()
            .find_map(|edge| {
                let start = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.start)?
                    .position;
                let end = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.end)?
                    .position;
                (edge.kind == EdgeKind::Boundary && start.x == 0.0 && end.x == 0.0)
                    .then_some(edge.id)
            })
            .expect("left boundary");
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let construction = ConstructedVertexSourceV1::CircleLine {
            center_vertex_id: center,
            radius: 5.0,
            edge_id: target,
            root_side: 0,
        };
        let before_vertices = project.editor.pattern().vertices.len();
        let before_boundary = project.editor.paper().boundary_vertices.len();
        let split_request = request(
            &project,
            ExpectedConstructedPlacementV1::SplitEdge { edge_id: target },
            construction,
        );
        place_constructed_vertex_inner_with_model_support_v1(&mut project, split_request, true)
            .expect("native boundary split");
        assert_eq!(project.editor.revision(), 1);
        assert_eq!(project.editor.pattern().vertices.len(), before_vertices + 1);
        assert_eq!(
            project.editor.paper().boundary_vertices.len(),
            before_boundary + 1,
        );
        assert_eq!(project.numeric_expressions.vertex_coordinates.len(), 1);
        let split_vertex = project.numeric_expressions.vertex_coordinates[0].vertex;
        assert_expression_matches_unique_vertex(&project, split_vertex);
        assert!(!project.editor.cutting_allowed());
    }

    #[test]
    fn command_rejection_after_expression_preparation_leaves_no_partial_binding() {
        let sheet = create_rectangular_sheet(10.0, 10.0, false).expect("sheet");
        let (mut pattern, mut paper) = sheet.into_parts();
        let center = VertexId::new();
        pattern.vertices.push(vertex(center, 5.0, 5.0));
        let target = pattern
            .edges
            .iter()
            .find_map(|edge| {
                let start = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.start)?
                    .position;
                let end = pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.end)?
                    .position;
                (edge.kind == EdgeKind::Boundary && start.x == 0.0 && end.x == 0.0)
                    .then_some(edge.id)
            })
            .expect("left boundary");
        paper.length_display_unit = LengthDisplayUnit::PaperEdgeRatio {
            reference_edge: target,
        };
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let before_vertices = project.editor.pattern().vertices.len();
        let before_edges = project.editor.pattern().edges.len();
        let construction = ConstructedVertexSourceV1::CircleLine {
            center_vertex_id: center,
            radius: 5.0,
            edge_id: target,
            root_side: 0,
        };
        let split_request = request(
            &project,
            ExpectedConstructedPlacementV1::SplitEdge { edge_id: target },
            construction,
        );

        assert!(
            place_constructed_vertex_inner_with_model_support_v1(
                &mut project,
                split_request,
                true,
            )
            .is_err()
        );
        assert_eq!(project.editor.revision(), 0);
        assert_eq!(project.editor.pattern().vertices.len(), before_vertices);
        assert_eq!(project.editor.pattern().edges.len(), before_edges);
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
    }
}
