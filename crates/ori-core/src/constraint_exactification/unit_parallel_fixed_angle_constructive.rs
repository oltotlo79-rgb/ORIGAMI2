use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, GeometricConstraintDocumentV1,
    GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2, VertexId,
};

use crate::constraint_solver::{
    Binary64ResidualOnlyConstraintSatisfactionV1,
    certify_binary64_residual_only_constraint_overlay_v1,
};

pub(crate) const MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1: usize = 256;

#[derive(Clone, Copy)]
struct CoreShape {
    parallel_id: ConstraintId,
    angle_id: ConstraintId,
    fixed_length_id: ConstraintId,
    center: VertexId,
    first_edge: EdgeId,
    second_edge: EdgeId,
}

impl CoreShape {
    fn contains_id(self, id: ConstraintId) -> bool {
        [self.parallel_id, self.angle_id, self.fixed_length_id].contains(&id)
    }
}

/// Constructs the three deletion witnesses for the exact unit/parallel/45°
/// theorem.
///
/// The semantic theorem is deliberately narrower than the direct theorem: the
/// two distinct edges must form a three-vertex star at the declared angle
/// center. That topology permits their outward vectors to be assigned
/// independently. Every candidate is accepted only after the complete frozen
/// production residual evaluator recertifies the exact deletion document.
pub(crate) fn construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
    pattern: &CreasePattern,
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
    document: &GeometricConstraintDocumentV1,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    if pattern.vertices.len() > MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        return None;
    }
    let shape = classify_core(core)?;
    if !shape.contains_id(removed) || !is_exact_deletion_document(core, removed, document) {
        return None;
    }

    let first = find_edge(pattern, shape.first_edge)?;
    let second = find_edge(pattern, shape.second_edge)?;
    let first_outer = outer_vertex(first, shape.center)?;
    let second_outer = outer_vertex(second, shape.center)?;
    if shape.center == first_outer || shape.center == second_outer || first_outer == second_outer {
        return None;
    }

    let diagonal_unit = f64::from_bits(0x3fe6_a09e_667f_3bcd);
    let overflow_scale = f64::from_bits(0x5fec_0000_0000_0000);
    let (first_vector, second_vector) = if removed == shape.parallel_id {
        (
            Point2::new(1.0, 0.0),
            Point2::new(diagonal_unit, diagonal_unit),
        )
    } else if removed == shape.angle_id {
        (Point2::new(1.0, 0.0), Point2::new(-1.0, 0.0))
    } else if removed == shape.fixed_length_id {
        (
            Point2::new(overflow_scale, 0.0),
            Point2::new(overflow_scale, overflow_scale),
        )
    } else {
        return None;
    };

    let assignments = [
        (shape.center, Point2::new(0.0, 0.0)),
        (first_outer, first_vector),
        (second_outer, second_vector),
    ];
    if assignments
        .iter()
        .any(|(_, point)| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }

    let mut overlay = Vec::new();
    overlay.try_reserve_exact(pattern.vertices.len()).ok()?;
    for vertex in &pattern.vertices {
        let position = assignments
            .iter()
            .find_map(|(id, position)| (*id == vertex.id).then_some(*position))
            .unwrap_or(Point2::new(0.0, 0.0));
        overlay.push((vertex.id, position));
    }
    certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
        .ok()
        .flatten()
}

fn classify_core(core: &[GeometricConstraintRecordV1]) -> Option<CoreShape> {
    if core.len() != 3 || !all_distinct_constraint_ids(core) {
        return None;
    }
    let mut parallel = None;
    let mut angle = None;
    let mut fixed_length = None;
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } if parallel.is_none() && first_edge != second_edge => {
                parallel = Some((record.id, *first_edge, *second_edge));
            }
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            } if angle.is_none()
                && first_edge != second_edge
                && angle_degrees.to_bits() == 45.0_f64.to_bits() =>
            {
                angle = Some((record.id, *vertex, *first_edge, *second_edge));
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if fixed_length.is_none() && length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed_length = Some((record.id, *edge));
            }
            _ => return None,
        }
    }
    let (parallel_id, first_edge, second_edge) = parallel?;
    let (angle_id, center, angle_first, angle_second) = angle?;
    let (fixed_length_id, fixed_edge) = fixed_length?;
    if !same_unordered_pair(first_edge, second_edge, angle_first, angle_second)
        || (fixed_edge != first_edge && fixed_edge != second_edge)
    {
        return None;
    }
    Some(CoreShape {
        parallel_id,
        angle_id,
        fixed_length_id,
        center,
        first_edge,
        second_edge,
    })
}

fn all_distinct_constraint_ids(core: &[GeometricConstraintRecordV1]) -> bool {
    core.iter()
        .enumerate()
        .all(|(index, record)| core[index + 1..].iter().all(|other| record.id != other.id))
}

fn is_exact_deletion_document(
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
    document: &GeometricConstraintDocumentV1,
) -> bool {
    document.constraints.len() == 2
        && core
            .iter()
            .filter(|record| record.id != removed)
            .all(|record| {
                document
                    .constraints
                    .iter()
                    .any(|candidate| candidate == record)
            })
        && document
            .constraints
            .iter()
            .all(|record| record.id != removed)
}

fn same_unordered_pair(
    first: EdgeId,
    second: EdgeId,
    other_first: EdgeId,
    other_second: EdgeId,
) -> bool {
    (first == other_first && second == other_second)
        || (first == other_second && second == other_first)
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn outer_vertex(edge: &Edge, center: VertexId) -> Option<VertexId> {
    if edge.start == center && edge.end != center {
        Some(edge.end)
    } else if edge.end == center && edge.start != center {
        Some(edge.start)
    } else {
        None
    }
}
