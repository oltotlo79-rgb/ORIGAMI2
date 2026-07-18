//! Exact zero-thickness evidence boundary for one authenticated triangle pair.
//!
//! This module is deliberately private. `TopologyRelation` and
//! `IntersectionEvidenceV2` are public policy labels, not certificates, so a
//! caller-provided triangle or relation must never enter the runtime
//! dispatcher through this boundary.
//!
//! `ori-kinematics` exposes a read-only canonical face-boundary registry from
//! the exact private source retained by `MaterialTreePose`. Production
//! construction nevertheless remains blocked until collision owns a
//! deterministic authenticated triangulation and proves complete
//! triangle-by-triangle coverage for each whole face pair. Treating an
//! arbitrary polygon as one triangle would create a false-safe proof.

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use ori_kinematics::{MaterialTreePose, Point3};

use crate::{
    IntersectionEvidenceV2, TopologyContactDecision, TopologyRelation,
    classify_runtime_topology_contact_v2,
};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "production construction is deliberately blocked until authenticated triangulation and complete triangle-pair coverage exist"
    )
)]
enum AuthenticatedTopology {
    NoSharedFeature,
    SharedVertex(Point3),
    SharedHingeEdge { start: Point3, end: Point3 },
    SameFace,
}

impl AuthenticatedTopology {
    const fn relation(&self) -> TopologyRelation {
        match self {
            Self::NoSharedFeature => TopologyRelation::NoSharedFeature,
            Self::SharedVertex(_) => TopologyRelation::SharedVertex,
            Self::SharedHingeEdge { .. } => TopologyRelation::SharedHingeEdge,
            Self::SameFace => TopologyRelation::SameFace,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AuthenticatedTrianglePair {
    first: [Point3; 3],
    second: [Point3; 3],
    topology: AuthenticatedTopology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PairDispatch {
    evidence: IntersectionEvidenceV2,
    decision: TopologyContactDecision,
}

impl PairDispatch {
    pub(super) const fn evidence(&self) -> IntersectionEvidenceV2 {
        self.evidence
    }

    pub(super) const fn decision(&self) -> TopologyContactDecision {
        self.decision
    }
}

/// Attempts to derive and dispatch one pair from the pose's private source.
///
/// `None` is a blocking absence of evidence, never separation. Although the
/// pose now exposes provenance-bearing canonical face boundaries, there is
/// intentionally no production constructor for `AuthenticatedTrianglePair`
/// until deterministic triangulation and complete face-pair aggregation are
/// implemented. A future constructor must authenticate shared `VertexId` and
/// `EdgeId` values against the hinge registry; caller-provided topology labels
/// remain ineligible.
pub(super) fn dispatch_material_pose_pair(
    pose: &MaterialTreePose,
    first_face_index: usize,
    second_face_index: usize,
) -> Option<PairDispatch> {
    let pair = authenticated_triangle_pair_from_pose(pose, first_face_index, second_face_index)?;
    Some(dispatch_authenticated_zero_thickness_pair(&pair))
}

fn authenticated_triangle_pair_from_pose(
    pose: &MaterialTreePose,
    first_face_index: usize,
    second_face_index: usize,
) -> Option<AuthenticatedTrianglePair> {
    let _ = (pose, first_face_index, second_face_index);
    None
}

fn dispatch_authenticated_zero_thickness_pair(pair: &AuthenticatedTrianglePair) -> PairDispatch {
    let topology = pair.topology.relation();
    if matches!(topology, TopologyRelation::SameFace) {
        return PairDispatch {
            evidence: IntersectionEvidenceV2::Indeterminate,
            decision: TopologyContactDecision::Indeterminate,
        };
    }

    let first = ExactTriangle::from_points(pair.first);
    let second = ExactTriangle::from_points(pair.second);
    let intersection = classify_triangle_intersection(&first, &second);
    let evidence = evidence_for_authenticated_topology(intersection, &pair.topology);
    PairDispatch {
        evidence,
        decision: classify_runtime_topology_contact_v2(topology, evidence),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactPoint3 {
    coordinates: [BigRational; 3],
}

impl ExactPoint3 {
    fn from_point(point: Point3) -> Self {
        Self {
            coordinates: [
                exact_binary64(point.x()),
                exact_binary64(point.y()),
                exact_binary64(point.z()),
            ],
        }
    }

    fn coordinate(&self, index: usize) -> &BigRational {
        &self.coordinates[index]
    }

    fn interpolate(&self, other: &Self, parameter: &BigRational) -> Self {
        Self {
            coordinates: std::array::from_fn(|index| {
                self.coordinates[index].clone()
                    + parameter
                        * (other.coordinates[index].clone() - self.coordinates[index].clone())
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactVector3 {
    coordinates: [BigRational; 3],
}

impl ExactVector3 {
    fn between(start: &ExactPoint3, end: &ExactPoint3) -> Self {
        Self {
            coordinates: std::array::from_fn(|index| {
                end.coordinates[index].clone() - start.coordinates[index].clone()
            }),
        }
    }

    fn cross(&self, other: &Self) -> Self {
        let [ax, ay, az] = &self.coordinates;
        let [bx, by, bz] = &other.coordinates;
        Self {
            coordinates: [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx],
        }
    }

    fn dot(&self, other: &Self) -> BigRational {
        self.coordinates
            .iter()
            .zip(&other.coordinates)
            .map(|(left, right)| left * right)
            .sum()
    }

    fn is_zero(&self) -> bool {
        self.coordinates.iter().all(Zero::is_zero)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactTriangle {
    points: [ExactPoint3; 3],
    normal: ExactVector3,
}

impl ExactTriangle {
    fn from_points(points: [Point3; 3]) -> Self {
        let points = points.map(ExactPoint3::from_point);
        let first_edge = ExactVector3::between(&points[0], &points[1]);
        let second_edge = ExactVector3::between(&points[0], &points[2]);
        let normal = first_edge.cross(&second_edge);
        Self { points, normal }
    }

    fn signed_plane_distance(&self, point: &ExactPoint3) -> BigRational {
        self.normal
            .dot(&ExactVector3::between(&self.points[0], point))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExactIntersection {
    Separated,
    Point(ExactPoint3),
    BoundaryLine {
        start: ExactPoint3,
        end: ExactPoint3,
    },
    CoplanarArea,
    Transversal,
    Indeterminate,
}

fn classify_triangle_intersection(
    first: &ExactTriangle,
    second: &ExactTriangle,
) -> ExactIntersection {
    if first.normal.is_zero() || second.normal.is_zero() {
        return ExactIntersection::Indeterminate;
    }

    let second_distances = second
        .points
        .each_ref()
        .map(|point| first.signed_plane_distance(point));
    if strictly_same_side(&second_distances) {
        return ExactIntersection::Separated;
    }
    let first_distances = first
        .points
        .each_ref()
        .map(|point| second.signed_plane_distance(point));
    if strictly_same_side(&first_distances) {
        return ExactIntersection::Separated;
    }

    if second_distances.iter().all(Zero::is_zero) {
        if !first_distances.iter().all(Zero::is_zero) {
            return ExactIntersection::Indeterminate;
        }
        return classify_coplanar_intersection(first, second);
    }

    classify_non_coplanar_intersection(first, second, &first_distances, &second_distances)
}

fn strictly_same_side(distances: &[BigRational; 3]) -> bool {
    distances.iter().all(|value| value.is_positive())
        || distances.iter().all(|value| value.is_negative())
}

#[derive(Debug)]
struct PlaneCut {
    points: Vec<ExactPoint3>,
    is_boundary_edge: bool,
}

fn triangle_plane_cut(triangle: &ExactTriangle, distances: &[BigRational; 3]) -> Option<PlaneCut> {
    let zero_count = distances.iter().filter(|value| value.is_zero()).count();
    let mut points = Vec::new();
    points.try_reserve_exact(2).ok()?;

    for (point, distance) in triangle.points.iter().zip(distances) {
        if distance.is_zero() && !push_unique_bounded(&mut points, point.clone(), 2) {
            return None;
        }
    }
    for index in 0..3 {
        let next = (index + 1) % 3;
        if distances[index].signum() == distances[next].signum()
            || distances[index].is_zero()
            || distances[next].is_zero()
        {
            continue;
        }
        let denominator = distances[index].clone() - distances[next].clone();
        if denominator.is_zero() {
            return None;
        }
        let parameter = distances[index].clone() / denominator;
        if !push_unique_bounded(
            &mut points,
            triangle.points[index].interpolate(&triangle.points[next], &parameter),
            2,
        ) {
            return None;
        }
    }

    if points.len() > 2 {
        return None;
    }
    Some(PlaneCut {
        points,
        is_boundary_edge: zero_count == 2,
    })
}

fn classify_non_coplanar_intersection(
    first: &ExactTriangle,
    second: &ExactTriangle,
    first_distances: &[BigRational; 3],
    second_distances: &[BigRational; 3],
) -> ExactIntersection {
    let Some(first_cut) = triangle_plane_cut(first, first_distances) else {
        return ExactIntersection::Indeterminate;
    };
    let Some(second_cut) = triangle_plane_cut(second, second_distances) else {
        return ExactIntersection::Indeterminate;
    };
    if first_cut.points.is_empty() || second_cut.points.is_empty() {
        return ExactIntersection::Separated;
    }

    let Some(axis) = varying_axis(&first_cut.points).or_else(|| varying_axis(&second_cut.points))
    else {
        return if first_cut.points[0] == second_cut.points[0] {
            ExactIntersection::Point(first_cut.points[0].clone())
        } else {
            ExactIntersection::Separated
        };
    };
    let first_interval = cut_interval(&first_cut, axis);
    let second_interval = cut_interval(&second_cut, axis);
    let overlap_start = first_interval.0.max(second_interval.0);
    let overlap_end = first_interval.1.min(second_interval.1);
    match overlap_start.cmp(&overlap_end) {
        Ordering::Greater => ExactIntersection::Separated,
        Ordering::Equal => point_at_coordinate(&first_cut, &second_cut, axis, &overlap_start)
            .map_or(ExactIntersection::Indeterminate, ExactIntersection::Point),
        Ordering::Less => {
            if !first_cut.is_boundary_edge && !second_cut.is_boundary_edge {
                return ExactIntersection::Transversal;
            }
            let Some(start) = point_at_coordinate(&first_cut, &second_cut, axis, &overlap_start)
            else {
                return ExactIntersection::Indeterminate;
            };
            let Some(end) = point_at_coordinate(&first_cut, &second_cut, axis, &overlap_end) else {
                return ExactIntersection::Indeterminate;
            };
            ExactIntersection::BoundaryLine { start, end }
        }
    }
}

fn varying_axis(points: &[ExactPoint3]) -> Option<usize> {
    let first = points.first()?;
    let second = points.get(1)?;
    (0..3).find(|index| first.coordinate(*index) != second.coordinate(*index))
}

fn cut_interval(cut: &PlaneCut, axis: usize) -> (BigRational, BigRational) {
    let first = cut.points[0].coordinate(axis).clone();
    let second = cut
        .points
        .get(1)
        .map_or_else(|| first.clone(), |point| point.coordinate(axis).clone());
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn point_at_coordinate(
    first: &PlaneCut,
    second: &PlaneCut,
    axis: usize,
    coordinate: &BigRational,
) -> Option<ExactPoint3> {
    point_on_cut_at_coordinate(first, axis, coordinate)
        .or_else(|| point_on_cut_at_coordinate(second, axis, coordinate))
}

fn point_on_cut_at_coordinate(
    cut: &PlaneCut,
    axis: usize,
    coordinate: &BigRational,
) -> Option<ExactPoint3> {
    let start = &cut.points[0];
    let Some(end) = cut.points.get(1) else {
        return (start.coordinate(axis) == coordinate).then(|| start.clone());
    };
    let denominator = end.coordinate(axis) - start.coordinate(axis);
    if denominator.is_zero() {
        return None;
    }
    let parameter = (coordinate - start.coordinate(axis)) / denominator;
    Some(start.interpolate(end, &parameter))
}

fn classify_coplanar_intersection(
    first: &ExactTriangle,
    second: &ExactTriangle,
) -> ExactIntersection {
    let Some(drop_axis) = first
        .normal
        .coordinates
        .iter()
        .position(|component| !component.is_zero())
    else {
        return ExactIntersection::Indeterminate;
    };
    let [first_axis, second_axis] = projected_axes(drop_axis);
    let clip_orientation = projected_line_value(
        &second.points[0],
        &second.points[1],
        &second.points[2],
        first_axis,
        second_axis,
    );
    if clip_orientation.is_zero() {
        return ExactIntersection::Indeterminate;
    }

    let mut polygon = Vec::new();
    if polygon.try_reserve_exact(3).is_err() {
        return ExactIntersection::Indeterminate;
    }
    polygon.extend(first.points.iter().cloned());
    for edge_index in 0..3 {
        let edge_start = &second.points[edge_index];
        let edge_end = &second.points[(edge_index + 1) % 3];
        let Some(clipped) = clip_polygon_against_line(
            &polygon,
            edge_start,
            edge_end,
            clip_orientation.is_positive(),
            first_axis,
            second_axis,
        ) else {
            return ExactIntersection::Indeterminate;
        };
        polygon = clipped;
        if polygon.is_empty() {
            return ExactIntersection::Separated;
        }
    }
    if !deduplicate_polygon(&mut polygon) {
        return ExactIntersection::Indeterminate;
    }

    match polygon.as_slice() {
        [] => ExactIntersection::Separated,
        [point] => ExactIntersection::Point(point.clone()),
        [start, end] => line_or_point(start, end),
        _ => {
            let area = projected_polygon_double_area(&polygon, first_axis, second_axis);
            if area.is_zero() {
                collapsed_polygon_intersection(&polygon, first_axis, second_axis)
                    .unwrap_or(ExactIntersection::Indeterminate)
            } else {
                ExactIntersection::CoplanarArea
            }
        }
    }
}

const fn projected_axes(drop_axis: usize) -> [usize; 2] {
    match drop_axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

fn projected_line_value(
    start: &ExactPoint3,
    end: &ExactPoint3,
    point: &ExactPoint3,
    first_axis: usize,
    second_axis: usize,
) -> BigRational {
    (end.coordinate(first_axis) - start.coordinate(first_axis))
        * (point.coordinate(second_axis) - start.coordinate(second_axis))
        - (end.coordinate(second_axis) - start.coordinate(second_axis))
            * (point.coordinate(first_axis) - start.coordinate(first_axis))
}

fn clip_polygon_against_line(
    polygon: &[ExactPoint3],
    line_start: &ExactPoint3,
    line_end: &ExactPoint3,
    positive_inside: bool,
    first_axis: usize,
    second_axis: usize,
) -> Option<Vec<ExactPoint3>> {
    if polygon.is_empty() {
        return Some(Vec::new());
    }
    let mut result = Vec::new();
    let result_bound = polygon.len().checked_add(1)?;
    result.try_reserve_exact(result_bound).ok()?;

    for index in 0..polygon.len() {
        let current = &polygon[index];
        let next = &polygon[(index + 1) % polygon.len()];
        let current_value =
            projected_line_value(line_start, line_end, current, first_axis, second_axis);
        let next_value = projected_line_value(line_start, line_end, next, first_axis, second_axis);
        let current_inside = is_inside(&current_value, positive_inside);
        let next_inside = is_inside(&next_value, positive_inside);

        if current_inside != next_inside {
            let denominator = current_value.clone() - next_value.clone();
            if denominator.is_zero() {
                return None;
            }
            let parameter = current_value / denominator;
            if !push_unique_bounded(
                &mut result,
                current.interpolate(next, &parameter),
                result_bound,
            ) {
                return None;
            }
        }
        if next_inside && !push_unique_bounded(&mut result, next.clone(), result_bound) {
            return None;
        }
    }
    if result.len() > 1 && result.first() == result.last() {
        result.pop();
    }
    Some(result)
}

fn is_inside(value: &BigRational, positive_inside: bool) -> bool {
    value.is_zero() || value.is_positive() == positive_inside
}

fn deduplicate_polygon(polygon: &mut Vec<ExactPoint3>) -> bool {
    let mut unique = Vec::new();
    if unique.try_reserve_exact(polygon.len()).is_err() {
        return false;
    }
    let bound = polygon.len();
    for point in polygon.drain(..) {
        if !push_unique_bounded(&mut unique, point, bound) {
            return false;
        }
    }
    *polygon = unique;
    true
}

fn projected_polygon_double_area(
    polygon: &[ExactPoint3],
    first_axis: usize,
    second_axis: usize,
) -> BigRational {
    (0..polygon.len())
        .map(|index| {
            let current = &polygon[index];
            let next = &polygon[(index + 1) % polygon.len()];
            current.coordinate(first_axis) * next.coordinate(second_axis)
                - current.coordinate(second_axis) * next.coordinate(first_axis)
        })
        .sum()
}

fn collapsed_polygon_intersection(
    polygon: &[ExactPoint3],
    first_axis: usize,
    second_axis: usize,
) -> Option<ExactIntersection> {
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(polygon.len()).ok()?;
    ordered.extend(polygon);
    ordered.sort_by(|left, right| {
        left.coordinate(first_axis)
            .cmp(right.coordinate(first_axis))
            .then_with(|| {
                left.coordinate(second_axis)
                    .cmp(right.coordinate(second_axis))
            })
    });
    Some(match (ordered.first(), ordered.last()) {
        (Some(start), Some(end)) => line_or_point(start, end),
        _ => ExactIntersection::Separated,
    })
}

fn line_or_point(start: &ExactPoint3, end: &ExactPoint3) -> ExactIntersection {
    if start == end {
        ExactIntersection::Point(start.clone())
    } else {
        ExactIntersection::BoundaryLine {
            start: start.clone(),
            end: end.clone(),
        }
    }
}

fn push_unique_bounded(points: &mut Vec<ExactPoint3>, point: ExactPoint3, bound: usize) -> bool {
    if points.contains(&point) {
        true
    } else if points.len() < bound {
        points.push(point);
        true
    } else {
        false
    }
}

fn evidence_for_authenticated_topology(
    intersection: ExactIntersection,
    topology: &AuthenticatedTopology,
) -> IntersectionEvidenceV2 {
    match intersection {
        ExactIntersection::Separated => IntersectionEvidenceV2::Separated,
        ExactIntersection::Point(point)
            if matches!(
                topology,
                AuthenticatedTopology::SharedVertex(shared)
                    if point == ExactPoint3::from_point(*shared)
            ) =>
        {
            IntersectionEvidenceV2::SharedFeatureContact
        }
        ExactIntersection::Point(_) => IntersectionEvidenceV2::PointContact,
        ExactIntersection::BoundaryLine { start, end }
            if matches!(
                topology,
                AuthenticatedTopology::SharedHingeEdge {
                    start: shared_start,
                    end: shared_end
                } if unordered_segment_eq(
                    &start,
                    &end,
                    &ExactPoint3::from_point(*shared_start),
                    &ExactPoint3::from_point(*shared_end),
                )
            ) =>
        {
            IntersectionEvidenceV2::SharedFeatureContact
        }
        ExactIntersection::BoundaryLine { .. } => IntersectionEvidenceV2::BoundaryLineContact,
        ExactIntersection::CoplanarArea => IntersectionEvidenceV2::CoplanarAreaOverlap,
        ExactIntersection::Transversal => IntersectionEvidenceV2::TransversalCrossing,
        ExactIntersection::Indeterminate => IntersectionEvidenceV2::Indeterminate,
    }
}

fn unordered_segment_eq(
    first_start: &ExactPoint3,
    first_end: &ExactPoint3,
    second_start: &ExactPoint3,
    second_end: &ExactPoint3,
) -> bool {
    (first_start == second_start && first_end == second_end)
        || (first_start == second_end && first_end == second_start)
}

/// Converts one finite binary64 coordinate into exact integer units of
/// `2^-1074`. `Point3` has already rejected non-finite values.
fn exact_binary64(value: f64) -> BigRational {
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, shift) = if exponent == 0 {
        (fraction, 0)
    } else {
        (fraction | (1_u64 << 52), exponent - 1)
    };
    let mut integer = BigInt::from(significand) << shift;
    if negative {
        integer = -integer;
    }
    BigRational::from_integer(integer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE_PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).expect("finite test point")
    }

    fn triangle(points: [[f64; 3]; 3]) -> [Point3; 3] {
        points.map(|[x, y, z]| point(x, y, z))
    }

    fn no_shared(first: [[f64; 3]; 3], second: [[f64; 3]; 3]) -> PairDispatch {
        dispatch_authenticated_zero_thickness_pair(&AuthenticatedTrianglePair {
            first: triangle(first),
            second: triangle(second),
            topology: AuthenticatedTopology::NoSharedFeature,
        })
    }

    fn permute(points: [[f64; 3]; 3], permutation: [usize; 3]) -> [[f64; 3]; 3] {
        permutation.map(|index| points[index])
    }

    #[test]
    fn clear_zero_thickness_intersection_dimensions_reach_the_v2_runtime_dispatcher() {
        let first = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        let cases = [
            (
                [[3.0, 3.0, 0.0], [4.0, 3.0, 0.0], [3.0, 4.0, 0.0]],
                IntersectionEvidenceV2::Separated,
                TopologyContactDecision::Separated,
            ),
            (
                [[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]],
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[1.0, 0.0, 0.0], [1.0, -1.0, 0.0], [2.0, -1.0, 0.0]],
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[0.5, 0.5, 0.0], [1.5, 0.25, 0.0], [0.25, 1.5, 0.0]],
                IntersectionEvidenceV2::CoplanarAreaOverlap,
                TopologyContactDecision::Penetrating,
            ),
            (
                [[0.5, 0.25, -1.0], [0.5, 0.25, 1.0], [0.5, 1.5, 0.0]],
                IntersectionEvidenceV2::TransversalCrossing,
                TopologyContactDecision::Penetrating,
            ),
        ];

        for (second, evidence, decision) in cases {
            for first_permutation in TRIANGLE_PERMUTATIONS {
                for second_permutation in TRIANGLE_PERMUTATIONS {
                    assert_eq!(
                        no_shared(
                            permute(first, first_permutation),
                            permute(second, second_permutation)
                        ),
                        PairDispatch { evidence, decision },
                        "{evidence:?}:{first_permutation:?}:{second_permutation:?}"
                    );
                    assert_eq!(
                        no_shared(
                            permute(second, second_permutation),
                            permute(first, first_permutation)
                        ),
                        PairDispatch { evidence, decision },
                        "swapped:{evidence:?}:{first_permutation:?}:{second_permutation:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn intersecting_support_planes_with_disjoint_cut_intervals_are_separated() {
        let horizontal = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        let vertical = [[3.0, 0.0, -1.0], [3.0, 0.0, 1.0], [3.0, 1.0, 0.0]];
        let expected = PairDispatch {
            evidence: IntersectionEvidenceV2::Separated,
            decision: TopologyContactDecision::Separated,
        };
        assert_eq!(no_shared(horizontal, vertical), expected);
        assert_eq!(no_shared(vertical, horizontal), expected);
    }

    #[test]
    fn exact_shared_feature_is_the_only_route_to_a_topology_allowance() {
        let first = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
        let point_second = triangle([[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]]);
        let point_pair = AuthenticatedTrianglePair {
            first,
            second: point_second,
            topology: AuthenticatedTopology::SharedVertex(point(2.0, 0.0, 0.0)),
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&point_pair),
            PairDispatch {
                evidence: IntersectionEvidenceV2::SharedFeatureContact,
                decision: TopologyContactDecision::AllowedSharedVertexContact,
            }
        );

        let line_second = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]]);
        let hinge_pair = AuthenticatedTrianglePair {
            first,
            second: line_second,
            topology: AuthenticatedTopology::SharedHingeEdge {
                start: point(0.0, 0.0, 0.0),
                end: point(2.0, 0.0, 0.0),
            },
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&hinge_pair),
            PairDispatch {
                evidence: IntersectionEvidenceV2::SharedFeatureContact,
                decision: TopologyContactDecision::RequiresHingeModel,
            }
        );
    }

    #[test]
    fn mismatched_or_partial_shared_geometry_never_enters_a_feature_allowance() {
        let first = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
        let point_second = triangle([[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]]);
        let wrong_vertex = AuthenticatedTrianglePair {
            first,
            second: point_second,
            topology: AuthenticatedTopology::SharedVertex(point(0.0, 2.0, 0.0)),
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&wrong_vertex),
            PairDispatch {
                evidence: IntersectionEvidenceV2::PointContact,
                decision: TopologyContactDecision::Touching,
            }
        );

        let line_second = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]]);
        let partial_hinge = AuthenticatedTrianglePair {
            first,
            second: line_second,
            topology: AuthenticatedTopology::SharedHingeEdge {
                start: point(0.0, 0.0, 0.0),
                end: point(3.0, 0.0, 0.0),
            },
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&partial_hinge),
            PairDispatch {
                evidence: IntersectionEvidenceV2::BoundaryLineContact,
                decision: TopologyContactDecision::Indeterminate,
            }
        );

        let area_overlap = AuthenticatedTrianglePair {
            first,
            second: triangle([[0.5, 0.5, 0.0], [1.5, 0.25, 0.0], [0.25, 1.5, 0.0]]),
            topology: AuthenticatedTopology::SharedVertex(point(0.0, 0.0, 0.0)),
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&area_overlap),
            PairDispatch {
                evidence: IntersectionEvidenceV2::CoplanarAreaOverlap,
                decision: TopologyContactDecision::Penetrating,
            }
        );
    }

    #[test]
    fn same_face_arrival_and_unrepresentable_triangle_fail_closed() {
        let triangle = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
        let same_face = AuthenticatedTrianglePair {
            first: triangle,
            second: triangle,
            topology: AuthenticatedTopology::SameFace,
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&same_face),
            PairDispatch {
                evidence: IntersectionEvidenceV2::Indeterminate,
                decision: TopologyContactDecision::Indeterminate,
            }
        );

        let degenerate = no_shared(
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        );
        assert_eq!(
            degenerate,
            PairDispatch {
                evidence: IntersectionEvidenceV2::Indeterminate,
                decision: TopologyContactDecision::Indeterminate,
            }
        );
    }

    #[test]
    fn exact_binary64_conversion_has_no_arithmetic_overflow_fallback() {
        assert_eq!(exact_binary64(-0.0), BigRational::zero());
        assert_eq!(
            exact_binary64(f64::from_bits(1)),
            BigRational::from_integer(BigInt::from(1_u8))
        );
        assert_eq!(
            exact_binary64(1.0),
            BigRational::from_integer(BigInt::from(1_u8) << 1074_usize)
        );
        assert_eq!(
            exact_binary64(f64::MAX),
            BigRational::from_integer(BigInt::from((1_u64 << 53) - 1) << 2045_usize)
        );
    }

    #[test]
    fn subnormal_and_near_maximum_coordinates_keep_exact_classification() {
        let subnormal = f64::from_bits(1);
        let twice_subnormal = f64::from_bits(2);
        assert_eq!(
            no_shared(
                [
                    [0.0, 0.0, 0.0],
                    [twice_subnormal, 0.0, 0.0],
                    [0.0, twice_subnormal, 0.0],
                ],
                [
                    [twice_subnormal, 0.0, 0.0],
                    [twice_subnormal + subnormal, 0.0, 0.0],
                    [twice_subnormal, -subnormal, 0.0],
                ],
            ),
            PairDispatch {
                evidence: IntersectionEvidenceV2::PointContact,
                decision: TopologyContactDecision::Touching,
            }
        );

        let maximum = f64::MAX;
        let previous = f64::from_bits(maximum.to_bits() - 1);
        let before_previous = f64::from_bits(maximum.to_bits() - 2);
        assert_eq!(
            no_shared(
                [
                    [maximum, maximum, 0.0],
                    [previous, maximum, 0.0],
                    [maximum, previous, 0.0],
                ],
                [
                    [-maximum, -maximum, 0.0],
                    [-previous, -maximum, 0.0],
                    [-maximum, -before_previous, 0.0],
                ],
            ),
            PairDispatch {
                evidence: IntersectionEvidenceV2::Separated,
                decision: TopologyContactDecision::Separated,
            }
        );
    }
}
