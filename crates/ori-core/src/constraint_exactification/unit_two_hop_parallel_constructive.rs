use std::collections::{BTreeMap, BTreeSet};

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, GeometricConstraintDocumentV1,
    GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2, VertexId,
};

use crate::constraint_solver::{
    Binary64ResidualOnlyConstraintSatisfactionV1,
    certify_binary64_residual_only_constraint_overlay_v1,
};

pub(crate) const MAX_UNIT_TWO_HOP_PARALLEL_RESIDUAL_ONLY_OVERLAY_VERTICES_V1: usize = 256;

#[derive(Clone, Copy)]
struct CoreShape {
    horizontal_id: ConstraintId,
    first_parallel_id: ConstraintId,
    second_parallel_id: ConstraintId,
    vertical_id: ConstraintId,
    fixed_length_id: ConstraintId,
    horizontal_edge: EdgeId,
    middle_edge: EdgeId,
    vertical_edge: EdgeId,
}

/// Constructs one of the five independent exact deletion witnesses for the
/// bit-exact unit-terminal, two-hop perpendicular-parallel theorem.
///
/// The constructor deliberately accepts only a three-edge star in the source
/// topology. That bounded shape is sufficient for the public semantic
/// inventory and makes every directed vector assignment independent. Other
/// topologies fail closed even though the direct unsatisfiability theorem
/// remains valid. Every template is re-evaluated through the unchanged
/// deterministic production residual implementation before a witness escapes.
pub(crate) fn construct_unit_two_hop_parallel_residual_exact_deletion_assignment_v1(
    pattern: &CreasePattern,
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
    document: &GeometricConstraintDocumentV1,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    if pattern.vertices.len() > MAX_UNIT_TWO_HOP_PARALLEL_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        return None;
    }
    let shape = classify_core(core)?;
    let horizontal = find_edge(pattern, shape.horizontal_edge)?;
    let middle = find_edge(pattern, shape.middle_edge)?;
    let vertical = find_edge(pattern, shape.vertical_edge)?;
    let center = common_vertex(horizontal, middle, vertical)?;

    let minimum = f64::from_bits(1);
    let half_maximum = f64::from_bits(0x7fdf_ffff_ffff_ffff);
    let (horizontal_vector, middle_vector, vertical_vector) = if removed == shape.horizontal_id {
        (
            Point2::new(0.0, 3.0),
            Point2::new(0.0, 2.0),
            Point2::new(0.0, 1.0),
        )
    } else if removed == shape.first_parallel_id {
        (
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 2.0),
            Point2::new(0.0, 1.0),
        )
    } else if removed == shape.second_parallel_id {
        (
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 1.0),
        )
    } else if removed == shape.vertical_id {
        (
            Point2::new(3.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(1.0, 0.0),
        )
    } else if removed == shape.fixed_length_id {
        (
            Point2::new(minimum, 0.0),
            Point2::new(2.0, 0.5),
            Point2::new(0.0, half_maximum),
        )
    } else {
        return None;
    };

    let mut assigned = BTreeMap::new();
    assign_point(&mut assigned, center, Point2::new(0.0, 0.0))?;
    assign_directed_vector_from_center(&mut assigned, horizontal, center, horizontal_vector)?;
    assign_directed_vector_from_center(&mut assigned, middle, center, middle_vector)?;
    assign_directed_vector_from_center(&mut assigned, vertical, center, vertical_vector)?;

    let overlay = pattern
        .vertices
        .iter()
        .map(|vertex| {
            (
                vertex.id,
                assigned
                    .get(&vertex.id.canonical_bytes())
                    .map(|(_, point)| *point)
                    .unwrap_or(Point2::new(0.0, 0.0)),
            )
        })
        .collect::<Vec<_>>();
    certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
        .ok()
        .flatten()
}

fn classify_core(core: &[GeometricConstraintRecordV1]) -> Option<CoreShape> {
    if core.len() != 5
        || core
            .iter()
            .map(|record| record.id.canonical_bytes())
            .collect::<BTreeSet<_>>()
            .len()
            != 5
    {
        return None;
    }
    let mut horizontal = None;
    let mut vertical = None;
    let mut fixed = None;
    let mut parallels = Vec::new();
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::Horizontal { edge } if horizontal.is_none() => {
                horizontal = Some((record.id, *edge));
            }
            GeometricConstraintKindV1::Vertical { edge } if vertical.is_none() => {
                vertical = Some((record.id, *edge));
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if fixed.is_none() && length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed = Some((record.id, *edge));
            }
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } if parallels.len() < 2 => {
                parallels.push((record.id, *first_edge, *second_edge));
            }
            _ => return None,
        }
    }
    let (horizontal_id, horizontal_edge) = horizontal?;
    let (vertical_id, vertical_edge) = vertical?;
    let (fixed_length_id, fixed_edge) = fixed?;
    if fixed_edge != vertical_edge || horizontal_edge == vertical_edge || parallels.len() != 2 {
        return None;
    }

    let mut edges = BTreeMap::new();
    for (_, first, second) in &parallels {
        edges.insert(first.canonical_bytes(), *first);
        edges.insert(second.canonical_bytes(), *second);
    }
    if edges.len() != 3
        || !edges.contains_key(&horizontal_edge.canonical_bytes())
        || !edges.contains_key(&vertical_edge.canonical_bytes())
    {
        return None;
    }
    let middle_edge = edges
        .into_iter()
        .find_map(|(_, edge)| (edge != horizontal_edge && edge != vertical_edge).then_some(edge))?;
    let first_parallel_id = parallels.iter().find_map(|(id, first, second)| {
        pair_matches(*first, *second, horizontal_edge, middle_edge).then_some(*id)
    })?;
    let second_parallel_id = parallels.iter().find_map(|(id, first, second)| {
        pair_matches(*first, *second, middle_edge, vertical_edge).then_some(*id)
    })?;
    (first_parallel_id != second_parallel_id).then_some(CoreShape {
        horizontal_id,
        first_parallel_id,
        second_parallel_id,
        vertical_id,
        fixed_length_id,
        horizontal_edge,
        middle_edge,
        vertical_edge,
    })
}

fn pair_matches(first: EdgeId, second: EdgeId, left: EdgeId, right: EdgeId) -> bool {
    (first == left && second == right) || (first == right && second == left)
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn common_vertex(first: &Edge, second: &Edge, third: &Edge) -> Option<VertexId> {
    [first.start, first.end]
        .into_iter()
        .find(|vertex| edge_contains(second, *vertex) && edge_contains(third, *vertex))
}

fn edge_contains(edge: &Edge, vertex: VertexId) -> bool {
    edge.start == vertex || edge.end == vertex
}

fn assign_directed_vector_from_center(
    assigned: &mut BTreeMap<[u8; 16], (VertexId, Point2)>,
    edge: &Edge,
    center: VertexId,
    vector: Point2,
) -> Option<()> {
    let (outer, point) = if edge.start == center {
        (edge.end, vector)
    } else if edge.end == center {
        (edge.start, Point2::new(-vector.x, -vector.y))
    } else {
        return None;
    };
    assign_point(assigned, outer, point)
}

fn assign_point(
    assigned: &mut BTreeMap<[u8; 16], (VertexId, Point2)>,
    vertex: VertexId,
    point: Point2,
) -> Option<()> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    match assigned.get(&vertex.canonical_bytes()) {
        Some((existing_vertex, existing))
            if *existing_vertex == vertex
                && existing.x.to_bits() == point.x.to_bits()
                && existing.y.to_bits() == point.y.to_bits() =>
        {
            Some(())
        }
        Some(_) => None,
        None => {
            assigned.insert(vertex.canonical_bytes(), (vertex, point));
            Some(())
        }
    }
}
