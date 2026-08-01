use std::collections::BTreeMap;

use ori_domain::{CreasePattern, Edge, EdgeId, Point2, VertexId};

pub(in crate::constraint_exactification) type CanonicalAssignment =
    BTreeMap<[u8; 16], (VertexId, Point2)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub(super) const fn rank(self) -> u8 {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }

    pub(super) const fn unit(self) -> Point2 {
        match self {
            Self::Horizontal => Point2::new(1.0, 0.0),
            Self::Vertical => Point2::new(0.0, 1.0),
        }
    }
}

pub(super) fn assign_axis_length(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
    length: f64,
    axis: Axis,
) -> Option<()> {
    if !ordinary_positive(length) {
        return None;
    }
    let vector = match axis {
        Axis::Horizontal => Point2::new(length, 0.0),
        Axis::Vertical => Point2::new(0.0, length),
    };
    assign_canonical_edge(pattern, assignment, edge, Point2::new(0.0, 0.0), vector)
}

pub(super) fn assign_two_edge_lengths(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    first: (EdgeId, f64),
    second: (EdgeId, f64),
) -> Option<()> {
    if !ordinary_positive(first.1) || !ordinary_positive(second.1) {
        return None;
    }
    let first_edge = find_edge(pattern, first.0)?;
    let second_edge = find_edge(pattern, second.0)?;
    if same_endpoint_set(first_edge, second_edge) {
        if first.1.to_bits() != second.1.to_bits() {
            return None;
        }
        return assign_canonical_edge(
            pattern,
            assignment,
            first.0,
            Point2::new(0.0, 0.0),
            Point2::new(first.1, 0.0),
        );
    }

    let mut ordered = [first, second];
    ordered.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    let first_vector = Point2::new(ordered[0].1, 0.0);
    let second_vector = Point2::new(0.0, ordered[1].1);
    let second_origin = Point2::new(0.0, ordered[1].1);
    assign_edge_pair_vectors(
        pattern,
        assignment,
        ordered[0].0,
        first_vector,
        ordered[1].0,
        second_vector,
        second_origin,
    )
}

pub(super) fn assign_axis_pair(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    first: (EdgeId, Axis),
    second: (EdgeId, Axis),
) -> Option<()> {
    let first_edge = find_edge(pattern, first.0)?;
    let second_edge = find_edge(pattern, second.0)?;
    if same_endpoint_set(first_edge, second_edge) {
        if first.1 != second.1 {
            return None;
        }
        return assign_canonical_edge(
            pattern,
            assignment,
            first.0,
            Point2::new(0.0, 0.0),
            first.1.unit(),
        );
    }

    let mut ordered = [first, second];
    ordered.sort_unstable_by_key(|(edge, axis)| (edge.canonical_bytes(), axis.rank()));
    let shared = shared_endpoints(
        find_edge(pattern, ordered[0].0)?,
        find_edge(pattern, ordered[1].0)?,
    );
    let first_vector = ordered[0].1.unit();
    let second_vector = if shared.len() == 1 && ordered[0].1 == ordered[1].1 {
        Point2::new(-first_vector.x, -first_vector.y)
    } else {
        ordered[1].1.unit()
    };
    let second_origin = match ordered[1].1 {
        Axis::Horizontal => Point2::new(0.0, 2.0),
        Axis::Vertical => Point2::new(2.0, 0.0),
    };
    assign_edge_pair_vectors(
        pattern,
        assignment,
        ordered[0].0,
        first_vector,
        ordered[1].0,
        second_vector,
        second_origin,
    )
}

pub(super) fn assign_outward_edge(
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

fn assign_edge_pair_vectors(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    first_id: EdgeId,
    first_vector: Point2,
    second_id: EdgeId,
    second_vector: Point2,
    second_origin: Point2,
) -> Option<()> {
    let first = find_edge(pattern, first_id)?;
    let second = find_edge(pattern, second_id)?;
    match shared_endpoints(first, second).as_slice() {
        [] => {
            assign_canonical_edge(
                pattern,
                assignment,
                first_id,
                Point2::new(0.0, 0.0),
                first_vector,
            )?;
            assign_canonical_edge(
                pattern,
                assignment,
                second_id,
                second_origin,
                checked_add(second_origin, second_vector)?,
            )
        }
        [shared] => {
            assign_point(assignment, *shared, Point2::new(0.0, 0.0))?;
            assign_point(assignment, opposite_endpoint(first, *shared)?, first_vector)?;
            assign_point(
                assignment,
                opposite_endpoint(second, *shared)?,
                second_vector,
            )
        }
        _ => None,
    }
}

fn assign_canonical_edge(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
    start: Point2,
    end: Point2,
) -> Option<()> {
    let [first, second] = canonical_endpoints(find_edge(pattern, edge)?);
    assign_point(assignment, first, start)?;
    assign_point(assignment, second, end)
}

fn canonical_endpoints(edge: &Edge) -> [VertexId; 2] {
    let mut endpoints = [edge.start, edge.end];
    endpoints.sort_unstable_by_key(VertexId::canonical_bytes);
    endpoints
}

fn same_endpoint_set(first: &Edge, second: &Edge) -> bool {
    canonical_endpoints(first) == canonical_endpoints(second)
}

fn shared_endpoints(first: &Edge, second: &Edge) -> Vec<VertexId> {
    canonical_endpoints(first)
        .into_iter()
        .filter(|vertex| canonical_endpoints(second).contains(vertex))
        .collect()
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
    if !ordinary_or_zero(point.x) || !ordinary_or_zero(point.y) {
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

pub(in crate::constraint_exactification) fn assignment_points_are_distinct(
    assignment: &CanonicalAssignment,
) -> bool {
    let points = assignment
        .values()
        .map(|(_, point)| *point)
        .collect::<Vec<_>>();
    points
        .iter()
        .enumerate()
        .all(|(index, point)| !points[index + 1..].contains(point))
}

pub(in crate::constraint_exactification) fn apply_translated_assignment(
    candidate: &mut CreasePattern,
    assignment: &CanonicalAssignment,
    offset: Point2,
) -> bool {
    let mut translated = Vec::with_capacity(assignment.len());
    for (vertex, point) in assignment.values() {
        let Some(point) = checked_add(*point, offset) else {
            return false;
        };
        if translated
            .iter()
            .any(|(_, existing_point)| *existing_point == point)
        {
            return false;
        }
        translated.push((*vertex, point));
    }
    for (vertex, point) in translated {
        let Some(target) = candidate
            .vertices
            .iter_mut()
            .find(|candidate| candidate.id == vertex)
        else {
            return false;
        };
        target.position = point;
    }
    true
}

fn checked_add(left: Point2, right: Point2) -> Option<Point2> {
    let result = Point2::new(left.x + right.x, left.y + right.y);
    (ordinary_or_zero(result.x) && ordinary_or_zero(result.y)).then_some(result)
}

pub(super) fn ordinary_positive(value: f64) -> bool {
    value > 0.0 && value.is_normal()
}

pub(in crate::constraint_exactification) fn ordinary_or_zero(value: f64) -> bool {
    value == 0.0 || value.is_normal()
}
