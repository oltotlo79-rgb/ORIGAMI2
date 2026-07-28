use std::collections::BTreeMap;

use ori_domain::{
    CreasePattern, Edge, EdgeId, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2,
    VertexId,
};
use ori_numeric::{deterministic_degrees_to_radians_v1, deterministic_sin_cos_degrees_v1};

use super::CurrentRuntimeExactConstraintAssignmentV1;
use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1, prepare_geometric_constraints_v1,
};

pub(crate) const MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1: usize = 4;

/// Constructs a bounded candidate assignment for one validated constraint and
/// returns it only after the complete production residual language reissues an
/// exact current-runtime certificate.
///
/// V1 tries four fixed translations of one canonical template. Referenced edge
/// endpoints are assigned by role, with incident-edge templates expressed as
/// outward vectors from the required vertex and two-edge templates handling a
/// shared endpoint explicitly. Trigonometric templates use the same frozen
/// degree conversion, `sin`, and `cos` operations as the proof residuals.
///
/// Invalid documents, documents containing other than one record, non-finite
/// or subnormal trigonometric components, conflicting shared roles, collapsed
/// geometry, and every candidate with any nonzero production residual return
/// `None`. Failure is not evidence of unsatisfiability.
#[must_use]
pub fn construct_single_constraint_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != 1 {
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

    let base_assignment =
        single_constraint_canonical_assignment(pattern, &document.constraints[0].constraint)?;
    let offsets: [Point2; MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1] = [
        Point2::new(0.0, 0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
        Point2::new(1024.0, -512.0),
    ];
    for offset in offsets {
        let mut candidate = pattern.clone();
        if !apply_translated_assignment(&mut candidate, &base_assignment, offset) {
            continue;
        }
        let certificate =
            certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
                .ok()
                .flatten();
        if let Some(certificate) = certificate {
            return Some(CurrentRuntimeExactConstraintAssignmentV1 {
                pattern: candidate,
                certificate,
            });
        }
    }
    None
}

type CanonicalAssignment = BTreeMap<[u8; 16], (VertexId, Point2)>;

fn single_constraint_canonical_assignment(
    pattern: &CreasePattern,
    constraint: &GeometricConstraintKindV1,
) -> Option<CanonicalAssignment> {
    let mut assignment = CanonicalAssignment::new();
    match *constraint {
        GeometricConstraintKindV1::FixedLength { edge, length_mm } => assign_edge_segment(
            pattern,
            &mut assignment,
            edge,
            Point2::new(0.0, 0.0),
            Point2::new(length_mm, 0.0),
        )?,
        GeometricConstraintKindV1::FixedAngle {
            vertex,
            first_edge,
            second_edge,
            angle_degrees,
        } => {
            let angle = checked_deterministic_angle(angle_degrees, angle_degrees != 0.0)?;
            let first_length = if angle_degrees == 0.0 { 2.0 } else { 1.0 };
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                first_edge,
                Point2::new(first_length, 0.0),
            )?;
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                second_edge,
                Point2::new(angle.cos, angle.sin),
            )?;
        }
        GeometricConstraintKindV1::Horizontal { edge } => assign_edge_segment(
            pattern,
            &mut assignment,
            edge,
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
        )?,
        GeometricConstraintKindV1::Vertical { edge } => assign_edge_segment(
            pattern,
            &mut assignment,
            edge,
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 1.0),
        )?,
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        } => assign_edge_pair(
            pattern,
            &mut assignment,
            first_edge,
            second_edge,
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        )?,
        GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => assign_edge_pair(
            pattern,
            &mut assignment,
            first_edge,
            second_edge,
            Point2::new(1.0, 0.0),
            Point2::new(-1.0, 0.0),
        )?,
        GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
            assign_edge_segment(
                pattern,
                &mut assignment,
                line_edge,
                Point2::new(-2.0, 0.0),
                Point2::new(2.0, 0.0),
            )?;
            assign_point(&mut assignment, vertex, Point2::new(0.0, 0.0))?;
        }
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        } => {
            assign_edge_segment(
                pattern,
                &mut assignment,
                axis_edge,
                Point2::new(-2.0, 0.0),
                Point2::new(2.0, 0.0),
            )?;
            assign_point(&mut assignment, first_vertex, Point2::new(0.0, 1.0))?;
            assign_point(&mut assignment, second_vertex, Point2::new(0.0, -1.0))?;
        }
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex,
            source_vertex,
            target_vertex,
            angle_degrees,
        } => {
            let angle = checked_deterministic_angle(angle_degrees, true)?;
            assign_point(&mut assignment, center_vertex, Point2::new(0.0, 0.0))?;
            assign_point(&mut assignment, source_vertex, Point2::new(1.0, 0.0))?;
            assign_point(
                &mut assignment,
                target_vertex,
                Point2::new(angle.cos, angle.sin),
            )?;
        }
        GeometricConstraintKindV1::AngleBisector {
            vertex,
            first_edge,
            second_edge,
            bisector_edge,
        } => {
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                first_edge,
                Point2::new(1.0, 0.0),
            )?;
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                second_edge,
                Point2::new(0.0, 1.0),
            )?;
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                bisector_edge,
                Point2::new(1.0, 1.0),
            )?;
        }
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio,
        } => assign_edge_pair(
            pattern,
            &mut assignment,
            numerator_edge,
            denominator_edge,
            Point2::new(ratio, 0.0),
            Point2::new(0.0, 1.0),
        )?,
    }
    Some(assignment)
}

#[derive(Clone, Copy)]
struct DeterministicAngle {
    sin: f64,
    cos: f64,
}

fn checked_deterministic_angle(
    angle_degrees: f64,
    require_nonzero: bool,
) -> Option<DeterministicAngle> {
    let radians = deterministic_degrees_to_radians_v1(angle_degrees).ok()?;
    let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees).ok()?;
    if !ordinary_or_zero(radians)
        || !ordinary_or_zero(sin)
        || !ordinary_or_zero(cos)
        || (require_nonzero && radians == 0.0)
    {
        return None;
    }
    Some(DeterministicAngle { sin, cos })
}

fn ordinary_or_zero(value: f64) -> bool {
    value == 0.0 || value.is_normal()
}

fn assign_edge_segment(
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
    let opposite = opposite_endpoint(edge, vertex)?;
    assign_point(assignment, vertex, Point2::new(0.0, 0.0))?;
    assign_point(assignment, opposite, outward)
}

fn assign_edge_pair(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    first: EdgeId,
    second: EdgeId,
    first_vector: Point2,
    second_vector: Point2,
) -> Option<()> {
    let first = find_edge(pattern, first)?;
    let second = find_edge(pattern, second)?;
    let first_vertices = [first.start, first.end];
    let second_vertices = [second.start, second.end];
    let shared = first_vertices
        .into_iter()
        .filter(|vertex| second_vertices.contains(vertex))
        .collect::<Vec<_>>();
    match shared.as_slice() {
        [] => {
            assign_point(assignment, first.start, Point2::new(0.0, 0.0))?;
            assign_point(assignment, first.end, first_vector)?;
            let second_origin = Point2::new(0.0, 4.0);
            assign_point(assignment, second.start, second_origin)?;
            assign_point(
                assignment,
                second.end,
                checked_add(second_origin, second_vector)?,
            )
        }
        [shared] => {
            let first_other = opposite_endpoint(first, *shared)?;
            let second_other = opposite_endpoint(second, *shared)?;
            assign_point(assignment, *shared, Point2::new(0.0, 0.0))?;
            assign_point(assignment, first_other, first_vector)?;
            assign_point(assignment, second_other, second_vector)
        }
        _ => None,
    }
}

fn opposite_endpoint(edge: &Edge, vertex: VertexId) -> Option<VertexId> {
    if edge.start == vertex {
        Some(edge.end)
    } else if edge.end == vertex {
        Some(edge.start)
    } else {
        None
    }
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn assign_point(
    assignment: &mut CanonicalAssignment,
    vertex: VertexId,
    point: Point2,
) -> Option<()> {
    if !point.x.is_finite() || !point.y.is_finite() {
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

fn apply_translated_assignment(
    candidate: &mut CreasePattern,
    assignment: &CanonicalAssignment,
    offset: Point2,
) -> bool {
    for (vertex, point) in assignment.values() {
        let Some(translated) = checked_add(*point, offset) else {
            return false;
        };
        let Some(target) = candidate
            .vertices
            .iter_mut()
            .find(|candidate| candidate.id == *vertex)
        else {
            return false;
        };
        target.position = translated;
    }
    true
}

fn checked_add(left: Point2, right: Point2) -> Option<Point2> {
    let result = Point2::new(left.x + right.x, left.y + right.y);
    (result.x.is_finite() && result.y.is_finite()).then_some(result)
}
