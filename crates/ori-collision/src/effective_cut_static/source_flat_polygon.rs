//! Exact source-flat polygon classification.
//!
//! This module is deliberately limited to polygons in the authenticated
//! source-flat plane. Every polygon is extruded through the same strictly
//! positive thickness interval along `+Y`. Consequently, two such closed
//! prisms have positive-volume intersection exactly when their exact 2D
//! polygon interiors have positive-area intersection. This equivalence does
//! not apply to a folded pose or to continuous collision detection.

use num_rational::BigRational;
use num_traits::Zero;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceFlatPolygonError {
    InvalidGeometry,
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceFlatPolygonIntersection {
    Separated,
    Touching,
    PositiveArea,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SourceFlatPolygonLimits {
    pub max_vertices_per_polygon: usize,
    pub max_storage_items: usize,
    pub max_predicate_work: usize,
    pub max_triangle_pairs: usize,
}

#[derive(Debug, Default)]
pub(super) struct SourceFlatPolygonMeter {
    storage_items: usize,
    predicate_work: usize,
    triangle_pairs: usize,
}

impl SourceFlatPolygonMeter {
    fn add_storage(
        &mut self,
        amount: usize,
        limits: SourceFlatPolygonLimits,
    ) -> Result<(), SourceFlatPolygonError> {
        self.storage_items = self
            .storage_items
            .checked_add(amount)
            .filter(|value| *value <= limits.max_storage_items)
            .ok_or(SourceFlatPolygonError::ResourceLimit)?;
        Ok(())
    }

    fn work(
        &mut self,
        amount: usize,
        limits: SourceFlatPolygonLimits,
    ) -> Result<(), SourceFlatPolygonError> {
        self.predicate_work = self
            .predicate_work
            .checked_add(amount)
            .filter(|value| *value <= limits.max_predicate_work)
            .ok_or(SourceFlatPolygonError::ResourceLimit)?;
        Ok(())
    }

    fn triangle_pair(
        &mut self,
        limits: SourceFlatPolygonLimits,
    ) -> Result<(), SourceFlatPolygonError> {
        self.triangle_pairs = self
            .triangle_pairs
            .checked_add(1)
            .filter(|value| *value <= limits.max_triangle_pairs)
            .ok_or(SourceFlatPolygonError::ResourceLimit)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ExactPoint2 {
    x: BigRational,
    z: BigRational,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PreparedSourceFlatPolygon {
    points: Vec<ExactPoint2>,
    triangles: Vec<[usize; 3]>,
}

pub(super) fn prepare_source_flat_polygon(
    input_len: usize,
    input: impl IntoIterator<Item = [f64; 3]>,
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<PreparedSourceFlatPolygon, SourceFlatPolygonError> {
    if input_len < 3 || input_len > limits.max_vertices_per_polygon {
        return Err(if input_len > limits.max_vertices_per_polygon {
            SourceFlatPolygonError::ResourceLimit
        } else {
            SourceFlatPolygonError::InvalidGeometry
        });
    }
    meter.add_storage(
        input_len
            // Conservative scalar-item peak: exact points and retained
            // copies (2n), active indices plus triangle indices (4n-6), and
            // the edge-multiplicity verification vectors (8n-12). Charging
            // all phases cumulatively also bounds simultaneously retained
            // prepared polygons without relying on allocator reuse.
            .checked_mul(14)
            .and_then(|value| value.checked_sub(18))
            .ok_or(SourceFlatPolygonError::ResourceLimit)?,
        limits,
    )?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(input_len)
        .map_err(|_| SourceFlatPolygonError::ResourceLimit)?;
    for point in input {
        meter.work(1, limits)?;
        if point.iter().any(|coordinate| !coordinate.is_finite()) || point[1] != 0.0 {
            return Err(SourceFlatPolygonError::InvalidGeometry);
        }
        points.push(ExactPoint2 {
            x: BigRational::from_float(point[0]).ok_or(SourceFlatPolygonError::InvalidGeometry)?,
            z: BigRational::from_float(point[2]).ok_or(SourceFlatPolygonError::InvalidGeometry)?,
        });
    }
    if points.len() != input_len {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    validate_simple_polygon(&points, limits, meter)?;
    remove_collinear_vertices(&mut points, limits, meter)?;
    if points.len() < 3 {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    canonicalize_counterclockwise(&mut points, limits, meter)?;
    let triangles = triangulate_canonical_polygon(&points, limits, meter)?;
    verify_triangulation_union(&points, &triangles, limits, meter)?;
    Ok(PreparedSourceFlatPolygon { points, triangles })
}

pub(super) fn classify_source_flat_polygon_pair(
    first: &PreparedSourceFlatPolygon,
    second: &PreparedSourceFlatPolygon,
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<SourceFlatPolygonIntersection, SourceFlatPolygonError> {
    let mut touching = false;
    for first_triangle in &first.triangles {
        for second_triangle in &second.triangles {
            meter.triangle_pair(limits)?;
            let disposition = classify_triangle_pair(
                triangle_points(&first.points, *first_triangle),
                triangle_points(&second.points, *second_triangle),
                limits,
                meter,
            )?;
            match disposition {
                SourceFlatPolygonIntersection::PositiveArea => {
                    return Ok(SourceFlatPolygonIntersection::PositiveArea);
                }
                SourceFlatPolygonIntersection::Touching => touching = true,
                SourceFlatPolygonIntersection::Separated => {}
            }
        }
    }
    Ok(if touching {
        SourceFlatPolygonIntersection::Touching
    } else {
        SourceFlatPolygonIntersection::Separated
    })
}

fn validate_simple_polygon(
    points: &[ExactPoint2],
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<(), SourceFlatPolygonError> {
    for first in 0..points.len() {
        for second in first + 1..points.len() {
            meter.work(1, limits)?;
            if points[first] == points[second] {
                return Err(SourceFlatPolygonError::InvalidGeometry);
            }
        }
    }
    let mut twice_area = BigRational::zero();
    for index in 0..points.len() {
        meter.work(1, limits)?;
        let next = (index + 1) % points.len();
        twice_area += &points[index].x * &points[next].z - &points[index].z * &points[next].x;
    }
    if twice_area.is_zero() {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    for first in 0..points.len() {
        let first_next = (first + 1) % points.len();
        for second in first + 1..points.len() {
            let second_next = (second + 1) % points.len();
            meter.work(1, limits)?;
            let adjacent =
                first == second_next || second == first_next || (first == 0 && second_next == 0);
            if adjacent {
                let shared = if first_next == second {
                    &points[first_next]
                } else {
                    &points[first]
                };
                let first_other = if &points[first] == shared {
                    &points[first_next]
                } else {
                    &points[first]
                };
                let second_other = if &points[second] == shared {
                    &points[second_next]
                } else {
                    &points[second]
                };
                if orientation(first_other, shared, second_other).is_zero()
                    && dot_from(shared, first_other, second_other) >= BigRational::zero()
                {
                    return Err(SourceFlatPolygonError::InvalidGeometry);
                }
            } else if segments_intersect(
                &points[first],
                &points[first_next],
                &points[second],
                &points[second_next],
            ) {
                return Err(SourceFlatPolygonError::InvalidGeometry);
            }
        }
    }
    Ok(())
}

fn remove_collinear_vertices(
    points: &mut Vec<ExactPoint2>,
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<(), SourceFlatPolygonError> {
    loop {
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(points.len())
            .map_err(|_| SourceFlatPolygonError::ResourceLimit)?;
        let mut removed = false;
        for index in 0..points.len() {
            meter.work(1, limits)?;
            let previous = &points[(index + points.len() - 1) % points.len()];
            let current = &points[index];
            let next = &points[(index + 1) % points.len()];
            if orientation(previous, current, next).is_zero()
                && point_on_segment(current, previous, next)
            {
                removed = true;
            } else {
                retained.push(current.clone());
            }
        }
        if !removed {
            return Ok(());
        }
        if retained.len() < 3 {
            return Err(SourceFlatPolygonError::InvalidGeometry);
        }
        *points = retained;
    }
}

fn canonicalize_counterclockwise(
    points: &mut [ExactPoint2],
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<(), SourceFlatPolygonError> {
    let mut twice_area = BigRational::zero();
    for index in 0..points.len() {
        meter.work(1, limits)?;
        let next = (index + 1) % points.len();
        twice_area += &points[index].x * &points[next].z - &points[index].z * &points[next].x;
    }
    if twice_area < BigRational::zero() {
        points.reverse();
    } else if twice_area.is_zero() {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    let start = points
        .iter()
        .enumerate()
        .min_by(|(_, first), (_, second)| first.cmp(second))
        .map(|(index, _)| index)
        .ok_or(SourceFlatPolygonError::InvalidGeometry)?;
    points.rotate_left(start);
    Ok(())
}

fn triangulate_canonical_polygon(
    points: &[ExactPoint2],
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<Vec<[usize; 3]>, SourceFlatPolygonError> {
    let mut active = (0..points.len()).collect::<Vec<_>>();
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(points.len() - 2)
        .map_err(|_| SourceFlatPolygonError::ResourceLimit)?;
    while active.len() > 3 {
        let mut ear = None;
        for position in 0..active.len() {
            meter.work(1, limits)?;
            let previous = active[(position + active.len() - 1) % active.len()];
            let current = active[position];
            let next = active[(position + 1) % active.len()];
            if orientation(&points[previous], &points[current], &points[next])
                <= BigRational::zero()
            {
                continue;
            }
            let mut contains_vertex = false;
            for candidate in &active {
                if *candidate == previous || *candidate == current || *candidate == next {
                    continue;
                }
                meter.work(1, limits)?;
                if point_in_ccw_triangle_inclusive(
                    &points[*candidate],
                    &points[previous],
                    &points[current],
                    &points[next],
                ) {
                    contains_vertex = true;
                    break;
                }
            }
            if !contains_vertex {
                ear = Some((position, [previous, current, next]));
                break;
            }
        }
        let (position, triangle) = ear.ok_or(SourceFlatPolygonError::InvalidGeometry)?;
        triangles.push(triangle);
        active.remove(position);
    }
    if active.len() != 3
        || orientation(&points[active[0]], &points[active[1]], &points[active[2]])
            <= BigRational::zero()
    {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    triangles.push([active[0], active[1], active[2]]);
    Ok(triangles)
}

fn verify_triangulation_union(
    points: &[ExactPoint2],
    triangles: &[[usize; 3]],
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<(), SourceFlatPolygonError> {
    if triangles.len() != points.len() - 2 {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    let mut polygon_twice_area = BigRational::zero();
    for index in 0..points.len() {
        meter.work(1, limits)?;
        let next = (index + 1) % points.len();
        polygon_twice_area +=
            &points[index].x * &points[next].z - &points[index].z * &points[next].x;
    }
    let mut triangle_twice_area = BigRational::zero();
    let mut triangle_edges = Vec::new();
    triangle_edges
        .try_reserve_exact(
            triangles
                .len()
                .checked_mul(3)
                .ok_or(SourceFlatPolygonError::ResourceLimit)?,
        )
        .map_err(|_| SourceFlatPolygonError::ResourceLimit)?;
    for triangle in triangles {
        meter.work(1, limits)?;
        if triangle.iter().any(|index| *index >= points.len()) {
            return Err(SourceFlatPolygonError::InvalidGeometry);
        }
        let area = orientation(
            &points[triangle[0]],
            &points[triangle[1]],
            &points[triangle[2]],
        );
        if area <= BigRational::zero() {
            return Err(SourceFlatPolygonError::InvalidGeometry);
        }
        triangle_twice_area += area;
        for edge in 0..3 {
            triangle_edges.push((triangle[edge], triangle[(edge + 1) % 3]));
        }
        let centroid = ExactPoint2 {
            x: (&points[triangle[0]].x + &points[triangle[1]].x + &points[triangle[2]].x)
                / BigRational::from_integer(3.into()),
            z: (&points[triangle[0]].z + &points[triangle[1]].z + &points[triangle[2]].z)
                / BigRational::from_integer(3.into()),
        };
        if !point_in_polygon_inclusive(&centroid, points, limits, meter)? {
            return Err(SourceFlatPolygonError::InvalidGeometry);
        }
    }
    if triangle_twice_area != polygon_twice_area {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    triangle_edges.sort_unstable_by_key(|(start, end)| ordered_index_edge(*start, *end));
    let boundary_edges = (0..points.len())
        .map(|index| (index, (index + 1) % points.len()))
        .collect::<Vec<_>>();
    let mut boundary_seen = Vec::new();
    let mut start = 0;
    while start < triangle_edges.len() {
        let mut end = start + 1;
        while end < triangle_edges.len()
            && ordered_index_edge(triangle_edges[end].0, triangle_edges[end].1)
                == ordered_index_edge(triangle_edges[start].0, triangle_edges[start].1)
        {
            end += 1;
        }
        meter.work(1, limits)?;
        match end - start {
            1 => boundary_seen.push(triangle_edges[start]),
            2 if triangle_edges[start]
                == (triangle_edges[start + 1].1, triangle_edges[start + 1].0) => {}
            _ => return Err(SourceFlatPolygonError::InvalidGeometry),
        }
        start = end;
    }
    boundary_seen.sort_unstable_by_key(|(start, end)| ordered_index_edge(*start, *end));
    let mut expected_boundary = boundary_edges;
    expected_boundary.sort_unstable_by_key(|(start, end)| ordered_index_edge(*start, *end));
    if boundary_seen != expected_boundary {
        return Err(SourceFlatPolygonError::InvalidGeometry);
    }
    for first in 0..triangles.len() {
        for second in first + 1..triangles.len() {
            meter.work(1, limits)?;
            if classify_triangle_pair(
                triangle_points(points, triangles[first]),
                triangle_points(points, triangles[second]),
                limits,
                meter,
            )? == SourceFlatPolygonIntersection::PositiveArea
            {
                return Err(SourceFlatPolygonError::InvalidGeometry);
            }
        }
    }
    Ok(())
}

fn classify_triangle_pair(
    first: [&ExactPoint2; 3],
    second: [&ExactPoint2; 3],
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<SourceFlatPolygonIntersection, SourceFlatPolygonError> {
    let mut touching_axis = false;
    for triangle in [first, second] {
        for edge in 0..3 {
            meter.work(1, limits)?;
            let start = triangle[edge];
            let end = triangle[(edge + 1) % 3];
            let axis_x = &end.z - &start.z;
            let axis_z = &start.x - &end.x;
            let (first_min, first_max) = projection_interval(first, &axis_x, &axis_z);
            let (second_min, second_max) = projection_interval(second, &axis_x, &axis_z);
            if first_max < second_min || second_max < first_min {
                return Ok(SourceFlatPolygonIntersection::Separated);
            }
            if first_max == second_min || second_max == first_min {
                touching_axis = true;
            }
        }
    }
    Ok(if touching_axis {
        SourceFlatPolygonIntersection::Touching
    } else {
        SourceFlatPolygonIntersection::PositiveArea
    })
}

fn projection_interval(
    triangle: [&ExactPoint2; 3],
    axis_x: &BigRational,
    axis_z: &BigRational,
) -> (BigRational, BigRational) {
    let mut minimum = axis_x * &triangle[0].x + axis_z * &triangle[0].z;
    let mut maximum = minimum.clone();
    for point in &triangle[1..] {
        let projection = axis_x * &point.x + axis_z * &point.z;
        if projection < minimum {
            minimum = projection.clone();
        }
        if projection > maximum {
            maximum = projection;
        }
    }
    (minimum, maximum)
}

fn triangle_points(points: &[ExactPoint2], triangle: [usize; 3]) -> [&ExactPoint2; 3] {
    [
        &points[triangle[0]],
        &points[triangle[1]],
        &points[triangle[2]],
    ]
}

fn ordered_index_edge(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn orientation(first: &ExactPoint2, second: &ExactPoint2, third: &ExactPoint2) -> BigRational {
    (&second.x - &first.x) * (&third.z - &first.z) - (&second.z - &first.z) * (&third.x - &first.x)
}

fn dot_from(origin: &ExactPoint2, first: &ExactPoint2, second: &ExactPoint2) -> BigRational {
    (&first.x - &origin.x) * (&second.x - &origin.x)
        + (&first.z - &origin.z) * (&second.z - &origin.z)
}

fn point_on_segment(point: &ExactPoint2, start: &ExactPoint2, end: &ExactPoint2) -> bool {
    orientation(start, end, point).is_zero()
        && ((start.x <= point.x && point.x <= end.x) || (end.x <= point.x && point.x <= start.x))
        && ((start.z <= point.z && point.z <= end.z) || (end.z <= point.z && point.z <= start.z))
}

fn segments_intersect(
    first_start: &ExactPoint2,
    first_end: &ExactPoint2,
    second_start: &ExactPoint2,
    second_end: &ExactPoint2,
) -> bool {
    let first_side_start = orientation(first_start, first_end, second_start);
    let first_side_end = orientation(first_start, first_end, second_end);
    let second_side_start = orientation(second_start, second_end, first_start);
    let second_side_end = orientation(second_start, second_end, first_end);
    (first_side_start.is_zero() && point_on_segment(second_start, first_start, first_end))
        || (first_side_end.is_zero() && point_on_segment(second_end, first_start, first_end))
        || (second_side_start.is_zero() && point_on_segment(first_start, second_start, second_end))
        || (second_side_end.is_zero() && point_on_segment(first_end, second_start, second_end))
        || ((first_side_start < BigRational::zero()) != (first_side_end < BigRational::zero())
            && (second_side_start < BigRational::zero()) != (second_side_end < BigRational::zero()))
}

fn point_in_ccw_triangle_inclusive(
    point: &ExactPoint2,
    first: &ExactPoint2,
    second: &ExactPoint2,
    third: &ExactPoint2,
) -> bool {
    orientation(first, second, point) >= BigRational::zero()
        && orientation(second, third, point) >= BigRational::zero()
        && orientation(third, first, point) >= BigRational::zero()
}

fn point_in_polygon_inclusive(
    point: &ExactPoint2,
    polygon: &[ExactPoint2],
    limits: SourceFlatPolygonLimits,
    meter: &mut SourceFlatPolygonMeter,
) -> Result<bool, SourceFlatPolygonError> {
    let mut inside = false;
    for index in 0..polygon.len() {
        meter.work(1, limits)?;
        let start = &polygon[index];
        let end = &polygon[(index + 1) % polygon.len()];
        if point_on_segment(point, start, end) {
            return Ok(true);
        }
        if (start.z > point.z) != (end.z > point.z) {
            let side = orientation(start, end, point);
            if (end.z > start.z && side > BigRational::zero())
                || (end.z < start.z && side < BigRational::zero())
            {
                inside = !inside;
            }
        }
    }
    Ok(inside)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> SourceFlatPolygonLimits {
        SourceFlatPolygonLimits {
            max_vertices_per_polygon: 64,
            max_storage_items: 1_024,
            max_predicate_work: 100_000,
            max_triangle_pairs: 1_024,
        }
    }

    fn prepare(points: &[[f64; 2]]) -> PreparedSourceFlatPolygon {
        let input = points
            .iter()
            .map(|point| [point[0], 0.0, point[1]])
            .collect::<Vec<_>>();
        prepare_source_flat_polygon(
            input.len(),
            input,
            limits(),
            &mut SourceFlatPolygonMeter::default(),
        )
        .unwrap()
    }

    fn classify(
        first: &PreparedSourceFlatPolygon,
        second: &PreparedSourceFlatPolygon,
    ) -> SourceFlatPolygonIntersection {
        classify_source_flat_polygon_pair(
            first,
            second,
            limits(),
            &mut SourceFlatPolygonMeter::default(),
        )
        .unwrap()
    }

    #[test]
    fn exact_polygon_pair_distinguishes_empty_touching_and_positive_area() {
        let square = prepare(&[[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]);
        let separated = prepare(&[[3.0, 0.0], [5.0, 0.0], [5.0, 2.0], [3.0, 2.0]]);
        let touching = prepare(&[[2.0, 0.0], [4.0, 0.0], [4.0, 2.0], [2.0, 2.0]]);
        let point_touching = prepare(&[[2.0, 2.0], [3.0, 2.0], [3.0, 3.0], [2.0, 3.0]]);
        let overlapping = prepare(&[[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]]);
        assert_eq!(
            classify(&square, &separated),
            SourceFlatPolygonIntersection::Separated
        );
        assert_eq!(
            classify(&square, &touching),
            SourceFlatPolygonIntersection::Touching
        );
        assert_eq!(
            classify(&square, &point_touching),
            SourceFlatPolygonIntersection::Touching
        );
        assert_eq!(
            classify(&square, &overlapping),
            SourceFlatPolygonIntersection::PositiveArea
        );
        let horizontal = prepare(&[[-2.0, -0.5], [2.0, -0.5], [2.0, 0.5], [-2.0, 0.5]]);
        let vertical = prepare(&[[-0.5, -2.0], [0.5, -2.0], [0.5, 2.0], [-0.5, 2.0]]);
        assert_eq!(
            classify(&horizontal, &vertical),
            SourceFlatPolygonIntersection::PositiveArea
        );
        let binary64_sum = 0.1_f64 + 0.2_f64;
        let exact_binary64_first = prepare(&[
            [0.0, 0.0],
            [binary64_sum, 0.0],
            [binary64_sum, 1.0],
            [0.0, 1.0],
        ]);
        let exact_binary64_second = prepare(&[[0.3, 0.0], [0.6, 0.0], [0.6, 1.0], [0.3, 1.0]]);
        assert_eq!(
            classify(&exact_binary64_first, &exact_binary64_second),
            SourceFlatPolygonIntersection::PositiveArea
        );
    }

    #[test]
    fn concave_collinear_polygon_is_rotation_and_winding_invariant() {
        let first = [
            [0.0, 0.0],
            [1.0, 0.0],
            [3.0, 0.0],
            [3.0, 3.0],
            [2.0, 3.0],
            [2.0, 1.0],
            [0.0, 1.0],
        ];
        let rotated = [
            [3.0, 3.0],
            [3.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [2.0, 1.0],
            [2.0, 3.0],
        ];
        let probe = prepare(&[[1.5, 0.5], [2.5, 0.5], [2.5, 1.5], [1.5, 1.5]]);
        let cavity = prepare(&[[0.5, 1.5], [1.5, 1.5], [1.5, 2.5], [0.5, 2.5]]);
        assert_eq!(
            classify(&prepare(&first), &probe),
            SourceFlatPolygonIntersection::PositiveArea
        );
        assert_eq!(
            classify(&prepare(&first), &cavity),
            SourceFlatPolygonIntersection::Separated
        );
        assert_eq!(
            classify(&prepare(&first), &probe),
            classify(&prepare(&rotated), &probe)
        );
        let diagonal_vertex =
            prepare(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 2.0], [0.0, 4.0]]);
        assert_eq!(
            classify(&diagonal_vertex, &diagonal_vertex),
            SourceFlatPolygonIntersection::PositiveArea
        );
    }

    #[test]
    fn malformed_and_resource_exhausted_inputs_fail_closed() {
        let bow_tie = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 2.0],
            [0.0, 0.0, 2.0],
            [2.0, 0.0, 0.0],
        ];
        assert_eq!(
            prepare_source_flat_polygon(
                bow_tie.len(),
                bow_tie,
                limits(),
                &mut SourceFlatPolygonMeter::default()
            ),
            Err(SourceFlatPolygonError::InvalidGeometry)
        );
        let square = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 2.0],
            [0.0, 0.0, 2.0],
        ];
        assert_eq!(
            prepare_source_flat_polygon(
                square.len(),
                square,
                SourceFlatPolygonLimits {
                    max_vertices_per_polygon: 3,
                    ..limits()
                },
                &mut SourceFlatPolygonMeter::default(),
            ),
            Err(SourceFlatPolygonError::ResourceLimit)
        );
        for invalid in [
            vec![[0.0, 0.0, 0.0], [2.0, 0.25, 0.0], [0.0, 0.0, 2.0]],
            vec![[0.0, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0], [0.0, 0.0, 2.0]],
        ] {
            assert_eq!(
                prepare_source_flat_polygon(
                    invalid.len(),
                    invalid,
                    limits(),
                    &mut SourceFlatPolygonMeter::default(),
                ),
                Err(SourceFlatPolygonError::InvalidGeometry)
            );
        }
    }

    #[test]
    fn every_resource_meter_fails_at_exactly_one_short() {
        let first_input = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 2.0],
            [0.0, 0.0, 2.0],
        ];
        let second_input = [
            [3.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [5.0, 0.0, 2.0],
            [3.0, 0.0, 2.0],
        ];
        let mut exact_meter = SourceFlatPolygonMeter::default();
        let exact_first =
            prepare_source_flat_polygon(first_input.len(), first_input, limits(), &mut exact_meter)
                .unwrap();
        let exact_second = prepare_source_flat_polygon(
            second_input.len(),
            second_input,
            limits(),
            &mut exact_meter,
        )
        .unwrap();
        assert_eq!(
            classify_source_flat_polygon_pair(
                &exact_first,
                &exact_second,
                limits(),
                &mut exact_meter,
            )
            .unwrap(),
            SourceFlatPolygonIntersection::Separated
        );
        assert!(exact_meter.storage_items > 0);
        assert!(exact_meter.predicate_work > 0);
        assert_eq!(exact_meter.triangle_pairs, 4);

        let run = |constrained: SourceFlatPolygonLimits| {
            let mut meter = SourceFlatPolygonMeter::default();
            let first = prepare_source_flat_polygon(
                first_input.len(),
                first_input,
                constrained,
                &mut meter,
            )?;
            let second = prepare_source_flat_polygon(
                second_input.len(),
                second_input,
                constrained,
                &mut meter,
            )?;
            classify_source_flat_polygon_pair(&first, &second, constrained, &mut meter)
        };
        for constrained in [
            SourceFlatPolygonLimits {
                max_vertices_per_polygon: first_input.len() - 1,
                ..limits()
            },
            SourceFlatPolygonLimits {
                max_storage_items: exact_meter.storage_items - 1,
                ..limits()
            },
            SourceFlatPolygonLimits {
                max_predicate_work: exact_meter.predicate_work - 1,
                ..limits()
            },
            SourceFlatPolygonLimits {
                max_triangle_pairs: exact_meter.triangle_pairs - 1,
                ..limits()
            },
        ] {
            assert_eq!(run(constrained), Err(SourceFlatPolygonError::ResourceLimit));
        }
    }
}
