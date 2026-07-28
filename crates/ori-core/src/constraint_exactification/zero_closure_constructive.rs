use std::collections::BTreeMap;

use ori_domain::{
    CreasePattern, Edge, EdgeId, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2,
    VertexId,
};
use ori_numeric::{
    deterministic_atan2_v1, deterministic_hypot_v1, deterministic_sin_cos_degrees_v1,
};

use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    constraint_solver::{
        Binary64ResidualOnlyConstraintSatisfactionV1,
        certify_binary64_residual_only_constraint_overlay_v1,
    },
    constraints::deterministic_fixed_angle_residual_binary64_v1,
    prepare_geometric_constraints_v1,
};

pub(crate) const MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1: usize = 8;

type CanonicalAssignment = BTreeMap<[u8; 16], (VertexId, Point2)>;

#[derive(Clone, Copy)]
enum RemainingAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy)]
enum Propagation {
    Equal { target: EdgeId },
    Ratio { target: EdgeId, ratio: f64 },
}

impl Propagation {
    const fn target(self) -> EdgeId {
        match self {
            Self::Equal { target } | Self::Ratio { target, .. } => target,
        }
    }

    fn forced_length_seed(self, target_length: f64) -> Option<f64> {
        let value = match self {
            Self::Equal { .. } => target_length,
            Self::Ratio { ratio, .. } => target_length / ratio,
        };
        (value.is_finite() && value > 0.0).then_some(value)
    }
}

#[derive(Clone)]
struct CanonicalDeletionShape {
    forced_edge: EdgeId,
    remaining_axis: Option<RemainingAxis>,
    propagation: Option<Propagation>,
    terminal: Option<GeometricConstraintKindV1>,
}

/// Constructs a fresh bounded residual-only deletion witness for the direct
/// four-record zero-length-closure cores.
///
/// Only the four three-record deletion shapes are admitted: removing the
/// terminal, propagation, horizontal, or vertical record. The propagation
/// direction is exactly the theorem direction (both ways for `EqualLength`,
/// denominator to numerator for `LengthRatio`). Every finite template is only
/// a candidate; the complete original document is always re-evaluated by the
/// unchanged production binary64 residual implementation before this function
/// returns an opaque witness.
pub(crate) fn construct_zero_length_closure_residual_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    if document.constraints.len() != 3 {
        return None;
    }
    let prepared =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
            .ok()?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return None;
    }

    let shape = classify_deletion_shape(document)?;
    let assignments = candidate_assignments(pattern, shape)?;
    if assignments.len() > MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1 {
        return None;
    }
    for assignment in assignments {
        let overlay = complete_overlay(pattern, &assignment);
        if let Some(certificate) =
            certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
                .ok()
                .flatten()
        {
            return Some(certificate);
        }
    }
    None
}

pub(crate) fn zero_length_closure_constructive_candidate_bound_v1(
    document: &GeometricConstraintDocumentV1,
) -> usize {
    let Some(shape) = classify_deletion_shape(document) else {
        return MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1;
    };
    let base = match shape.terminal {
        None => 1,
        Some(GeometricConstraintKindV1::FixedLength { .. }) => 4,
        Some(
            GeometricConstraintKindV1::PointOnLine { .. }
            | GeometricConstraintKindV1::MirrorSymmetry { .. }
            | GeometricConstraintKindV1::AngleBisector { .. }
            | GeometricConstraintKindV1::Parallel { .. }
            | GeometricConstraintKindV1::FixedAngle { .. },
        ) => 4,
        Some(_) => MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1,
    };
    let multiplier = usize::from(shape.remaining_axis.is_some()) + 1;
    base.saturating_mul(multiplier)
        .min(MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1)
}

fn classify_deletion_shape(
    document: &GeometricConstraintDocumentV1,
) -> Option<CanonicalDeletionShape> {
    let mut horizontal = None;
    let mut vertical = None;
    let mut propagation_kind = None;
    let mut terminal = None;
    for record in &document.constraints {
        match record.constraint.clone() {
            GeometricConstraintKindV1::Horizontal { edge } => {
                set_once(&mut horizontal, edge)?;
            }
            GeometricConstraintKindV1::Vertical { edge } => {
                set_once(&mut vertical, edge)?;
            }
            kind @ (GeometricConstraintKindV1::EqualLength { .. }
            | GeometricConstraintKindV1::LengthRatio { .. }) => {
                set_once(&mut propagation_kind, kind)?;
            }
            kind @ (GeometricConstraintKindV1::FixedLength { .. }
            | GeometricConstraintKindV1::PointOnLine { .. }
            | GeometricConstraintKindV1::MirrorSymmetry { .. }
            | GeometricConstraintKindV1::AngleBisector { .. }
            | GeometricConstraintKindV1::Parallel { .. }
            | GeometricConstraintKindV1::FixedAngle { .. }) => {
                set_once(&mut terminal, kind)?;
            }
            GeometricConstraintKindV1::RotationalSymmetry { .. } => return None,
        }
    }

    match (horizontal, vertical, propagation_kind, terminal) {
        (Some(horizontal), Some(vertical), Some(kind), None) if horizontal == vertical => {
            let propagation = propagation_from_forced(horizontal, kind)?;
            Some(CanonicalDeletionShape {
                forced_edge: horizontal,
                remaining_axis: None,
                propagation: Some(propagation),
                terminal: None,
            })
        }
        (Some(horizontal), Some(vertical), None, Some(terminal)) if horizontal == vertical => {
            terminal_is_admitted(&terminal)?;
            Some(CanonicalDeletionShape {
                forced_edge: horizontal,
                remaining_axis: None,
                propagation: None,
                terminal: Some(terminal),
            })
        }
        (Some(forced), None, Some(kind), Some(terminal)) => {
            let propagation = propagation_from_forced(forced, kind)?;
            terminal_provides_edge(&terminal, propagation.target())?;
            Some(CanonicalDeletionShape {
                forced_edge: forced,
                remaining_axis: Some(RemainingAxis::Horizontal),
                propagation: Some(propagation),
                terminal: Some(terminal),
            })
        }
        (None, Some(forced), Some(kind), Some(terminal)) => {
            let propagation = propagation_from_forced(forced, kind)?;
            terminal_provides_edge(&terminal, propagation.target())?;
            Some(CanonicalDeletionShape {
                forced_edge: forced,
                remaining_axis: Some(RemainingAxis::Vertical),
                propagation: Some(propagation),
                terminal: Some(terminal),
            })
        }
        _ => None,
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    if slot.is_some() {
        None
    } else {
        *slot = Some(value);
        Some(())
    }
}

fn propagation_from_forced(forced: EdgeId, kind: GeometricConstraintKindV1) -> Option<Propagation> {
    match kind {
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        } if first_edge == forced && second_edge != forced => Some(Propagation::Equal {
            target: second_edge,
        }),
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        } if second_edge == forced && first_edge != forced => {
            Some(Propagation::Equal { target: first_edge })
        }
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio,
        } if denominator_edge == forced
            && numerator_edge != forced
            && ratio.is_finite()
            && ratio > 0.0 =>
        {
            Some(Propagation::Ratio {
                target: numerator_edge,
                ratio,
            })
        }
        _ => None,
    }
}

fn terminal_is_admitted(terminal: &GeometricConstraintKindV1) -> Option<()> {
    match terminal {
        GeometricConstraintKindV1::FixedLength { length_mm, .. }
            if length_mm.is_finite() && *length_mm > 0.0 =>
        {
            Some(())
        }
        GeometricConstraintKindV1::PointOnLine { .. }
        | GeometricConstraintKindV1::MirrorSymmetry { .. }
        | GeometricConstraintKindV1::AngleBisector { .. }
        | GeometricConstraintKindV1::Parallel { .. } => Some(()),
        GeometricConstraintKindV1::FixedAngle { angle_degrees, .. }
            if fixed_angle_rejects_collapsed_cross(*angle_degrees) =>
        {
            Some(())
        }
        _ => None,
    }
}

fn terminal_provides_edge(terminal: &GeometricConstraintKindV1, target: EdgeId) -> Option<()> {
    terminal_is_admitted(terminal)?;
    let provides = match terminal {
        GeometricConstraintKindV1::FixedLength { edge, .. } => *edge == target,
        GeometricConstraintKindV1::PointOnLine { line_edge, .. } => *line_edge == target,
        GeometricConstraintKindV1::MirrorSymmetry { axis_edge, .. } => *axis_edge == target,
        GeometricConstraintKindV1::AngleBisector {
            first_edge,
            second_edge,
            bisector_edge,
            ..
        } => [*first_edge, *second_edge, *bisector_edge].contains(&target),
        GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::FixedAngle {
            first_edge,
            second_edge,
            ..
        } => *first_edge == target || *second_edge == target,
        _ => false,
    };
    provides.then_some(())
}

fn fixed_angle_rejects_collapsed_cross(angle_degrees: f64) -> bool {
    angle_degrees.is_finite()
        && (0.0..=180.0).contains(&angle_degrees)
        && [
            0.0,
            -0.0,
            1.0,
            -1.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ]
        .into_iter()
        .all(|dot| match deterministic_atan2_v1(0.0, dot) {
            Ok(actual) => {
                let residual =
                    deterministic_fixed_angle_residual_binary64_v1(actual, angle_degrees);
                !residual.is_finite() || residual != 0.0
            }
            Err(_) => true,
        })
}

fn candidate_assignments(
    pattern: &CreasePattern,
    shape: CanonicalDeletionShape,
) -> Option<Vec<CanonicalAssignment>> {
    match (shape.remaining_axis, shape.propagation, shape.terminal) {
        (None, Some(_), None) => {
            let mut assignment = CanonicalAssignment::new();
            for vertex in &pattern.vertices {
                assign_point(&mut assignment, vertex.id, Point2::new(0.0, 0.0))?;
            }
            Some(vec![assignment])
        }
        (None, None, Some(terminal)) => {
            let mut result = Vec::new();
            for mut assignment in terminal_assignments(pattern, terminal)? {
                if collapse_edge(pattern, &mut assignment, shape.forced_edge).is_some() {
                    result.push(assignment);
                }
            }
            (!result.is_empty()).then_some(result)
        }
        (Some(axis), Some(propagation), Some(terminal)) => {
            let mut result = Vec::new();
            for assignment in terminal_assignments(pattern, terminal)? {
                let target_length =
                    assigned_edge_length(pattern, &assignment, propagation.target())?;
                let forced_length = propagation.forced_length_seed(target_length)?;
                for sign in [1.0, -1.0] {
                    let mut candidate = assignment.clone();
                    if assign_axis_edge(
                        pattern,
                        &mut candidate,
                        shape.forced_edge,
                        axis,
                        forced_length,
                        sign,
                    )
                    .is_some()
                    {
                        result.push(candidate);
                    }
                }
            }
            (!result.is_empty()
                && result.len() <= MAX_ZERO_LENGTH_CLOSURE_CONSTRUCTIVE_CANDIDATES_V1)
                .then_some(result)
        }
        _ => None,
    }
}

fn terminal_assignments(
    pattern: &CreasePattern,
    terminal: GeometricConstraintKindV1,
) -> Option<Vec<CanonicalAssignment>> {
    let mut result = Vec::new();
    match terminal {
        GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
            for vector in cardinal_vectors(length_mm) {
                let mut assignment = CanonicalAssignment::new();
                if assign_directed_edge(
                    pattern,
                    &mut assignment,
                    edge,
                    Point2::new(0.0, 0.0),
                    vector,
                )
                .is_some()
                {
                    result.push(assignment);
                }
            }
        }
        GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
            // These are the four distinct directed-line images of the D4
            // orbit; the other four are bit-identical for y == 0.
            for transform in [0, 1, 4, 5] {
                let start = transform_point(Point2::new(-2.0, 0.0), transform);
                let end = transform_point(Point2::new(2.0, 0.0), transform);
                let point = transform_point(Point2::new(0.0, 0.0), transform);
                let mut assignment = CanonicalAssignment::new();
                if assign_edge_points(pattern, &mut assignment, line_edge, start, end)
                    .and_then(|()| assign_point(&mut assignment, vertex, point))
                    .is_some()
                {
                    result.push(assignment);
                }
            }
        }
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        } => {
            // Four non-duplicate directed-axis D4 images cover horizontal and
            // vertical roles in both storage directions.
            for transform in [0, 1, 4, 5] {
                let start = transform_point(Point2::new(-2.0, 0.0), transform);
                let end = transform_point(Point2::new(2.0, 0.0), transform);
                let first = transform_point(Point2::new(0.0, 1.0), transform);
                let second = transform_point(Point2::new(0.0, -1.0), transform);
                let mut assignment = CanonicalAssignment::new();
                if assign_edge_points(pattern, &mut assignment, axis_edge, start, end)
                    .and_then(|()| assign_point(&mut assignment, first_vertex, first))
                    .and_then(|()| assign_point(&mut assignment, second_vertex, second))
                    .is_some()
                {
                    result.push(assignment);
                }
            }
        }
        GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => {
            for vector in [Point2::new(1.0, 0.0), Point2::new(0.0, 1.0)] {
                for second_sign in [1.0, -1.0] {
                    let second = scale(vector, second_sign)?;
                    if let Some(assignment) =
                        parallel_assignment(pattern, first_edge, second_edge, vector, second)
                    {
                        result.push(assignment);
                    }
                }
            }
        }
        GeometricConstraintKindV1::AngleBisector {
            vertex,
            first_edge,
            second_edge,
            bisector_edge,
        } => {
            for transform in [0, 1, 4, 5] {
                let mut assignment = CanonicalAssignment::new();
                let first = transform_point(Point2::new(1.0, 0.0), transform);
                let second = transform_point(Point2::new(0.0, 1.0), transform);
                let bisector = transform_point(Point2::new(1.0, 1.0), transform);
                if assign_outward_edge(pattern, &mut assignment, vertex, first_edge, first)
                    .and_then(|()| {
                        assign_outward_edge(pattern, &mut assignment, vertex, second_edge, second)
                    })
                    .and_then(|()| {
                        assign_outward_edge(
                            pattern,
                            &mut assignment,
                            vertex,
                            bisector_edge,
                            bisector,
                        )
                    })
                    .is_some()
                {
                    result.push(assignment);
                }
            }
        }
        GeometricConstraintKindV1::FixedAngle {
            vertex,
            first_edge,
            second_edge,
            angle_degrees,
        } => {
            let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees).ok()?;
            let base_second = Point2::new(cos, sin);
            if !finite_point(base_second) {
                return None;
            }
            for transform in [0, 1, 4, 5] {
                let mut assignment = CanonicalAssignment::new();
                let first = transform_point(Point2::new(1.0, 0.0), transform);
                let second = transform_point(base_second, transform);
                if assign_outward_edge(pattern, &mut assignment, vertex, first_edge, first)
                    .and_then(|()| {
                        assign_outward_edge(pattern, &mut assignment, vertex, second_edge, second)
                    })
                    .is_some()
                {
                    result.push(assignment);
                }
            }
        }
        _ => return None,
    }
    (!result.is_empty()).then_some(result)
}

fn collapse_edge(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
) -> Option<()> {
    let edge = find_edge(pattern, edge)?;
    let start = assigned_point(assignment, edge.start);
    let end = assigned_point(assignment, edge.end);
    let point = match (start, end) {
        (Some(start), Some(end)) if start == end => start,
        (Some(_), Some(_)) => return None,
        (Some(point), None) | (None, Some(point)) => point,
        (None, None) => Point2::new(0.0, 0.0),
    };
    assign_point(assignment, edge.start, point)?;
    assign_point(assignment, edge.end, point)
}

fn assign_axis_edge(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
    axis: RemainingAxis,
    length: f64,
    sign: f64,
) -> Option<()> {
    if !length.is_finite() || length <= 0.0 {
        return None;
    }
    let vector = match axis {
        RemainingAxis::Horizontal => Point2::new(sign * length, 0.0),
        RemainingAxis::Vertical => Point2::new(0.0, sign * length),
    };
    if !finite_point(vector) {
        return None;
    }
    let edge = find_edge(pattern, edge)?;
    match (
        assigned_point(assignment, edge.start),
        assigned_point(assignment, edge.end),
    ) {
        (None, None) => {
            assign_point(assignment, edge.start, Point2::new(0.0, 0.0))?;
            assign_point(assignment, edge.end, vector)
        }
        (Some(start), None) => {
            let end = add(start, vector)?;
            assign_point(assignment, edge.end, end)
        }
        (None, Some(end)) => {
            let start = subtract(end, vector)?;
            assign_point(assignment, edge.start, start)
        }
        (Some(start), Some(end)) => {
            let actual = subtract(end, start)?;
            (actual == vector).then_some(())
        }
    }
}

fn assigned_edge_length(
    pattern: &CreasePattern,
    assignment: &CanonicalAssignment,
    edge: EdgeId,
) -> Option<f64> {
    let edge = find_edge(pattern, edge)?;
    let start = assigned_point(assignment, edge.start)?;
    let end = assigned_point(assignment, edge.end)?;
    let vector = subtract(end, start)?;
    let length = deterministic_hypot_v1(vector.x, vector.y).ok()?;
    (length > 0.0).then_some(length)
}

fn parallel_assignment(
    pattern: &CreasePattern,
    first_id: EdgeId,
    second_id: EdgeId,
    first_vector: Point2,
    second_vector: Point2,
) -> Option<CanonicalAssignment> {
    let first = find_edge(pattern, first_id)?;
    let second = find_edge(pattern, second_id)?;
    let mut assignment = CanonicalAssignment::new();
    let shared = [first.start, first.end]
        .into_iter()
        .filter(|vertex| [second.start, second.end].contains(vertex))
        .collect::<Vec<_>>();
    match shared.as_slice() {
        [] => {
            assign_directed_edge(
                pattern,
                &mut assignment,
                first_id,
                Point2::new(0.0, 0.0),
                first_vector,
            )?;
            assign_directed_edge(
                pattern,
                &mut assignment,
                second_id,
                Point2::new(0.0, 4.0),
                second_vector,
            )?;
        }
        [shared] => {
            let origin = Point2::new(0.0, 0.0);
            assign_point(&mut assignment, *shared, origin)?;
            assign_other_for_directed_vector(&mut assignment, first, *shared, first_vector)?;
            assign_other_for_directed_vector(&mut assignment, second, *shared, second_vector)?;
        }
        [_, _] => {
            assign_directed_edge(
                pattern,
                &mut assignment,
                first_id,
                Point2::new(0.0, 0.0),
                first_vector,
            )?;
            let actual_second = directed_vector(second, &assignment)?;
            if actual_second != second_vector {
                return None;
            }
        }
        _ => return None,
    }
    Some(assignment)
}

fn assign_other_for_directed_vector(
    assignment: &mut CanonicalAssignment,
    edge: &Edge,
    shared: VertexId,
    vector: Point2,
) -> Option<()> {
    if edge.start == shared {
        assign_point(assignment, edge.end, vector)
    } else if edge.end == shared {
        assign_point(assignment, edge.start, scale(vector, -1.0)?)
    } else {
        None
    }
}

fn directed_vector(edge: &Edge, assignment: &CanonicalAssignment) -> Option<Point2> {
    subtract(
        assigned_point(assignment, edge.end)?,
        assigned_point(assignment, edge.start)?,
    )
}

fn assign_directed_edge(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
    origin: Point2,
    vector: Point2,
) -> Option<()> {
    let edge = find_edge(pattern, edge)?;
    assign_point(assignment, edge.start, origin)?;
    assign_point(assignment, edge.end, add(origin, vector)?)
}

fn assign_edge_points(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
    start: Point2,
    end: Point2,
) -> Option<()> {
    let edge = find_edge(pattern, edge)?;
    assign_point(assignment, edge.start, start)?;
    assign_point(assignment, edge.end, end)
}

fn assign_outward_edge(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    vertex: VertexId,
    edge: EdgeId,
    outward: Point2,
) -> Option<()> {
    let edge = find_edge(pattern, edge)?;
    let opposite = if edge.start == vertex {
        edge.end
    } else if edge.end == vertex {
        edge.start
    } else {
        return None;
    };
    assign_point(assignment, vertex, Point2::new(0.0, 0.0))?;
    assign_point(assignment, opposite, outward)
}

fn assign_point(
    assignment: &mut CanonicalAssignment,
    vertex: VertexId,
    point: Point2,
) -> Option<()> {
    if !finite_point(point) {
        return None;
    }
    match assignment.get(&vertex.canonical_bytes()) {
        Some((existing_vertex, existing_point))
            if *existing_vertex != vertex || *existing_point != point =>
        {
            None
        }
        Some(_) => Some(()),
        None => {
            assignment.insert(vertex.canonical_bytes(), (vertex, point));
            Some(())
        }
    }
}

fn assigned_point(assignment: &CanonicalAssignment, vertex: VertexId) -> Option<Point2> {
    assignment
        .get(&vertex.canonical_bytes())
        .and_then(|(stored, point)| (*stored == vertex).then_some(*point))
}

fn complete_overlay(
    pattern: &CreasePattern,
    assignment: &CanonicalAssignment,
) -> Vec<(VertexId, Point2)> {
    pattern
        .vertices
        .iter()
        .map(|vertex| {
            (
                vertex.id,
                assigned_point(assignment, vertex.id).unwrap_or(vertex.position),
            )
        })
        .collect()
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn cardinal_vectors(length: f64) -> [Point2; 4] {
    [
        Point2::new(length, 0.0),
        Point2::new(-length, 0.0),
        Point2::new(0.0, length),
        Point2::new(0.0, -length),
    ]
}

fn transform_point(point: Point2, transform: usize) -> Point2 {
    match transform {
        0 => Point2::new(point.x, point.y),
        1 => Point2::new(-point.x, point.y),
        2 => Point2::new(point.x, -point.y),
        3 => Point2::new(-point.x, -point.y),
        4 => Point2::new(point.y, point.x),
        5 => Point2::new(-point.y, point.x),
        6 => Point2::new(point.y, -point.x),
        _ => Point2::new(-point.y, -point.x),
    }
}

fn scale(point: Point2, scalar: f64) -> Option<Point2> {
    let result = Point2::new(point.x * scalar, point.y * scalar);
    finite_point(result).then_some(result)
}

fn add(left: Point2, right: Point2) -> Option<Point2> {
    let result = Point2::new(left.x + right.x, left.y + right.y);
    finite_point(result).then_some(result)
}

fn subtract(left: Point2, right: Point2) -> Option<Point2> {
    let result = Point2::new(left.x - right.x, left.y - right.y);
    finite_point(result).then_some(result)
}

fn finite_point(point: Point2) -> bool {
    point.x.is_finite() && point.y.is_finite()
}
