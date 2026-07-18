//! Geometry predicates and structural validation for origami crease patterns.

use std::error::Error;
use std::fmt;

use ori_domain::Point2;

mod validation;

pub use validation::{
    BoundaryEdgeRef, CreasePatternValidation, EdgeEndpoint, PaperValidation, PaperValidationIssue,
    ValidationIssue, exact_polygon_orientation, polygon_signed_double_area,
    validate_crease_pattern, validate_crease_pattern_with_checkpoint, validate_paper,
    validate_paper_with_checkpoint,
};

/// The orientation of an ordered point triple or polygon boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Clockwise,
    CounterClockwise,
    Collinear,
}

/// A sign certified by the floating-point orientation filter.
///
/// [`FilteredOrientation::Indeterminate`] means that ordinary `f64`
/// arithmetic cannot certify either sign. It includes exactly collinear
/// inputs as well as non-collinear inputs whose determinant lies inside the
/// rounding-error bound. This filter does not invoke the arbitrary-precision
/// backend automatically; callers may fail closed or retry with
/// [`exact_orientation`]. It is not safe to resolve an indeterminate result
/// with an epsilon or an ID tie-break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilteredOrientation {
    Clockwise,
    CounterClockwise,
    Indeterminate,
}

/// An error that prevents a geometric predicate from producing a valid answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeometryError {
    /// One of the input points contains `NaN` or positive/negative infinity.
    NonFinitePoint {
        argument: &'static str,
        point: Point2,
    },
    /// The result cannot be represented safely in binary64 despite all inputs
    /// being finite. This includes numeric overflow and a strict interior
    /// intersection whose rounded coordinate coincides with an endpoint.
    ArithmeticOverflow,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinitePoint { argument, point } => write!(
                formatter,
                "point {argument} must have finite coordinates, got ({}, {})",
                point.x, point.y
            ),
            Self::ArithmeticOverflow => {
                formatter.write_str("geometry result cannot be represented safely in finite f64")
            }
        }
    }
}

impl Error for GeometryError {}

/// Classifies the orientation of the ordered triplet `(a, b, c)`.
///
/// Non-finite input is rejected rather than being silently treated as collinear.
pub fn orientation(a: Point2, b: Point2, c: Point2) -> Result<Orientation, GeometryError> {
    ensure_finite("a", a)?;
    ensure_finite("b", b)?;
    ensure_finite("c", c)?;

    let ab = subtract(b, a)?;
    let ac = subtract(c, a)?;
    let determinant = cross(ab, ac);
    if !determinant.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }

    Ok(if determinant > 0.0 {
        Orientation::CounterClockwise
    } else if determinant < 0.0 {
        Orientation::Clockwise
    } else {
        Orientation::Collinear
    })
}

/// Classifies three finite binary64 points using an exact determinant sign.
///
/// Unlike [`orientation`] and [`filtered_orientation`], this function expands
/// every coordinate into its exact integer significand and binary exponent,
/// then evaluates the determinant with arbitrary-precision integers. It can
/// therefore distinguish a non-zero orientation even when the determinant is
/// too small or cancellation-prone for `f64`. It is intended as the
/// correctness fallback for topology-changing decisions, not as the first
/// stage of every high-volume query.
/// For finite inputs the exact integer calculation cannot overflow; the error
/// result is reserved for non-finite coordinates.
pub fn exact_orientation(a: Point2, b: Point2, c: Point2) -> Result<Orientation, GeometryError> {
    ensure_finite("a", a)?;
    ensure_finite("b", b)?;
    ensure_finite("c", c)?;
    Ok(validation::exact_triangle_orientation(a, b, c))
}

/// Certifies the orientation sign of the ordered triplet `(a, b, c)` when a
/// fast `f64` determinant is far enough from its rounding-error boundary.
///
/// For unit roundoff `u = 2^-53`, the standard first-stage `orient2d` filter
/// bounds the determinant error by
/// `(3 + 16u) * u * (|det_left| + |det_right|)`. The implementation uses the
/// slightly larger, exactly representable factor `4u`, and only returns a
/// direction when the computed determinant is strictly outside that bound.
/// If the bound would be subnormal, the result is also indeterminate rather
/// than relying on an underflowed error estimate.
///
/// Non-finite coordinates and overflowing intermediate arithmetic are
/// rejected. This first-stage API does not call [`exact_orientation`]
/// automatically; [`FilteredOrientation::Indeterminate`] must be passed to
/// that fallback or rejected, never guessed into a topological sign with an
/// epsilon or an unrelated stable ID.
///
/// Only a returned direction is certified. Because binary64 subtraction and
/// overflow depend on which argument is chosen as the local origin, permuting
/// the same three inputs can change whether this first-stage filter returns a
/// direction, [`FilteredOrientation::Indeterminate`], or an error. Callers
/// must not rely on those three result categories being permutation-invariant.
pub fn filtered_orientation(
    a: Point2,
    b: Point2,
    c: Point2,
) -> Result<FilteredOrientation, GeometryError> {
    ensure_finite("a", a)?;
    ensure_finite("b", b)?;
    ensure_finite("c", c)?;

    // Using c as the local origin is the conventional orient2d formulation:
    // det = (a.x-c.x)(b.y-c.y) - (a.y-c.y)(b.x-c.x).
    let a_minus_c = subtract(a, c)?;
    let b_minus_c = subtract(b, c)?;
    let determinant_left = checked_product(a_minus_c.x, b_minus_c.y)?;
    let determinant_right = checked_product(a_minus_c.y, b_minus_c.x)?;
    let determinant = determinant_left - determinant_right;
    if !determinant.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }

    let determinant_permanent = determinant_left.abs() + determinant_right.abs();
    if !determinant_permanent.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }

    // f64::EPSILON is 2u, hence this is the conservative 4u envelope of the
    // standard (3 + 16u)u first-stage bound. The extra margin also covers the
    // final rounded multiplication used to construct `error_bound`.
    const ORIENTATION_ERROR_FACTOR: f64 = 2.0 * f64::EPSILON;
    let error_bound = ORIENTATION_ERROR_FACTOR * determinant_permanent;
    if !error_bound.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }

    // Relative-error bounds cannot by themselves account for a subnormal
    // multiplication result. Returning Indeterminate here keeps the filter a
    // one-sided certificate even at the bottom of the f64 range.
    if error_bound < f64::MIN_POSITIVE {
        return Ok(FilteredOrientation::Indeterminate);
    }

    Ok(if determinant > error_bound {
        FilteredOrientation::CounterClockwise
    } else if determinant < -error_bound {
        FilteredOrientation::Clockwise
    } else {
        FilteredOrientation::Indeterminate
    })
}

/// The exact topological relation between a point and a closed segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointSegmentRelation {
    Outside,
    Start,
    End,
    StrictInterior,
}

/// The exact topological relation between a point and a closed polygon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointPolygonRelation {
    Outside,
    Boundary,
    Inside,
}

/// Classifies `point` against an ordered polygon using an exact winding test.
///
/// The last vertex is implicitly connected back to the first. Boundary is
/// checked first with exact point/segment predicates, then interior is defined
/// by the non-zero winding rule. Horizontal edges and vertices on the winding
/// ray follow the standard half-open rule, so the returned relation is
/// invariant under cycle rotation and reversal. Empty and degenerate polygons
/// retain only their exact point/segment boundary; all other points are
/// outside. Callers that require a material sheet must separately validate
/// that `polygon` is simple and has a non-zero area.
pub fn point_polygon_relation(
    point: Point2,
    polygon: &[Point2],
) -> Result<PointPolygonRelation, GeometryError> {
    ensure_finite("point", point)?;
    for vertex in polygon {
        ensure_finite("polygon point", *vertex)?;
    }
    if polygon.is_empty() {
        return Ok(PointPolygonRelation::Outside);
    }

    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        if point_segment_relation(point, start, end)? != PointSegmentRelation::Outside {
            return Ok(PointPolygonRelation::Boundary);
        }
    }

    let mut winding = 0_i128;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        if start.y <= point.y {
            if end.y > point.y
                && exact_orientation(start, end, point)? == Orientation::CounterClockwise
            {
                winding += 1;
            }
        } else if end.y <= point.y
            && exact_orientation(start, end, point)? == Orientation::Clockwise
        {
            winding -= 1;
        }
    }

    Ok(if winding == 0 {
        PointPolygonRelation::Outside
    } else {
        PointPolygonRelation::Inside
    })
}

/// Classifies the exact mathematical midpoint of `start` and `end` against a
/// closed polygon.
///
/// The midpoint remains an exact dyadic rational throughout the boundary and
/// winding tests. No `f64` midpoint is constructed, so adjacent binary64
/// endpoints, subnormal coordinates, and opposite extreme coordinates cannot
/// round the query onto an endpoint or overflow while averaging. Callers that
/// require a material sheet must separately validate that `polygon` is simple
/// and has a non-zero area. This classifies one point only; it does not by
/// itself prove that the entire source segment is contained by the polygon.
pub fn segment_midpoint_polygon_relation(
    start: Point2,
    end: Point2,
    polygon: &[Point2],
) -> Result<PointPolygonRelation, GeometryError> {
    ensure_finite("start", start)?;
    ensure_finite("end", end)?;
    for vertex in polygon {
        ensure_finite("polygon point", *vertex)?;
    }
    Ok(validation::exact_segment_midpoint_polygon_relation(
        start, end, polygon,
    ))
}

/// Classifies `point` against the directed segment `start -> end`.
///
/// Endpoint identity is checked before collinearity so callers can preserve
/// the original edge direction while treating only
/// [`PointSegmentRelation::StrictInterior`] as a split location. The
/// collinearity decision uses the exact binary64 determinant sign, so a tiny
/// non-zero offset is never rounded onto the segment. Non-finite inputs are
/// rejected.
pub fn point_segment_relation(
    point: Point2,
    start: Point2,
    end: Point2,
) -> Result<PointSegmentRelation, GeometryError> {
    ensure_finite("point", point)?;
    ensure_finite("start", start)?;
    ensure_finite("end", end)?;
    if point == start {
        return Ok(PointSegmentRelation::Start);
    }
    if point == end {
        return Ok(PointSegmentRelation::End);
    }
    if exact_orientation(start, end, point)? != Orientation::Collinear
        || !point_within_segment_bounds(point, start, end)
    {
        return Ok(PointSegmentRelation::Outside);
    }
    Ok(PointSegmentRelation::StrictInterior)
}

/// The geometric intersection of two closed line segments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentIntersection {
    None,
    Point(Point2),
    /// The segments share a collinear interval of positive length.
    CollinearOverlap,
}

/// Classifies the intersection of closed segments `ab` and `cd`.
///
/// Collinear segments are distinguished precisely by their projected closed
/// intervals: disjoint intervals return [`SegmentIntersection::None`], a
/// single shared endpoint returns [`SegmentIntersection::Point`], and only a
/// positive-length shared interval returns
/// [`SegmentIntersection::CollinearOverlap`].
pub fn segment_intersection(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
) -> Result<SegmentIntersection, GeometryError> {
    ensure_finite("a", a)?;
    ensure_finite("b", b)?;
    ensure_finite("c", c)?;
    ensure_finite("d", d)?;

    let ab_is_point = a == b;
    let cd_is_point = c == d;
    if ab_is_point && cd_is_point {
        return Ok(if a == c {
            SegmentIntersection::Point(a)
        } else {
            SegmentIntersection::None
        });
    }
    if ab_is_point {
        return Ok(if point_on_segment(a, c, d)? {
            SegmentIntersection::Point(a)
        } else {
            SegmentIntersection::None
        });
    }
    if cd_is_point {
        return Ok(if point_on_segment(c, a, b)? {
            SegmentIntersection::Point(c)
        } else {
            SegmentIntersection::None
        });
    }

    // Decide the topology with exact determinant signs. In particular, an
    // ordinary product underflow must never turn two adjacent, non-collinear
    // edges into a collinear overlap.
    let ab_c = exact_orientation(a, b, c)?;
    let ab_d = exact_orientation(a, b, d)?;
    let cd_a = exact_orientation(c, d, a)?;
    let cd_b = exact_orientation(c, d, b)?;

    if [ab_c, ab_d, cd_a, cd_b]
        .into_iter()
        .all(|orientation| orientation == Orientation::Collinear)
    {
        return Ok(classify_collinear_intersection(a, b, c, d));
    }

    // Return shared endpoints verbatim before interpolation. This also covers
    // a T-junction where exactly one endpoint lies in the other segment.
    for (orientation, point, start, end) in [
        (ab_c, c, a, b),
        (ab_d, d, a, b),
        (cd_a, a, c, d),
        (cd_b, b, c, d),
    ] {
        if orientation == Orientation::Collinear && point_within_segment_bounds(point, start, end) {
            return Ok(SegmentIntersection::Point(point));
        }
    }

    if !orientations_are_opposite(ab_c, ab_d) || !orientations_are_opposite(cd_a, cd_b) {
        return Ok(SegmentIntersection::None);
    }

    // Exact predicates above prove a strict interior crossing. Compute its
    // coordinate as an exact determinant ratio and round only the final x/y
    // values; rounded f64 cross products can otherwise produce a completely
    // different parameter even while remaining inside (0, 1).
    Ok(SegmentIntersection::Point(
        validation::exact_proper_segment_intersection_point(a, b, c, d)?,
    ))
}

fn orientations_are_opposite(left: Orientation, right: Orientation) -> bool {
    matches!(
        (left, right),
        (Orientation::Clockwise, Orientation::CounterClockwise)
            | (Orientation::CounterClockwise, Orientation::Clockwise)
    )
}

fn point_within_segment_bounds(point: Point2, start: Point2, end: Point2) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

fn ensure_finite(argument: &'static str, point: Point2) -> Result<(), GeometryError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(GeometryError::NonFinitePoint { argument, point })
    }
}

fn subtract(left: Point2, right: Point2) -> Result<Point2, GeometryError> {
    let difference = Point2::new(left.x - right.x, left.y - right.y);
    if difference.x.is_finite() && difference.y.is_finite() {
        Ok(difference)
    } else {
        Err(GeometryError::ArithmeticOverflow)
    }
}

fn checked_product(left: f64, right: f64) -> Result<f64, GeometryError> {
    let product = left * right;
    if product.is_finite() {
        Ok(product)
    } else {
        Err(GeometryError::ArithmeticOverflow)
    }
}

fn cross(a: Point2, b: Point2) -> f64 {
    // Keep both products rounded in the same way. Using `mul_add` for only one
    // product can make `cross(v, v)` a tiny non-zero value for ordinary finite
    // coordinates, incorrectly classifying an exactly collinear segment.
    a.x * b.y - a.y * b.x
}

fn point_on_segment(point: Point2, start: Point2, end: Point2) -> Result<bool, GeometryError> {
    Ok(point_segment_relation(point, start, end)? != PointSegmentRelation::Outside)
}

fn classify_collinear_intersection(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
) -> SegmentIntersection {
    // `ab` is known to be non-degenerate here. Choosing a coordinate that
    // changes along it avoids a subtraction which could overflow for finite
    // endpoints near opposite extremes of the binary64 range.
    let use_x = a.x != b.x;
    let project = |point: Point2| if use_x { point.x } else { point.y };

    let a_value = project(a);
    let b_value = project(b);
    let c_value = project(c);
    let d_value = project(d);
    let overlap_start = a_value.min(b_value).max(c_value.min(d_value));
    let overlap_end = a_value.max(b_value).min(c_value.max(d_value));

    if overlap_start > overlap_end {
        return SegmentIntersection::None;
    }
    if overlap_start < overlap_end {
        return SegmentIntersection::CollinearOverlap;
    }

    // A zero-width overlap is necessarily one of the original endpoints. By
    // returning that endpoint verbatim, callers do not accumulate interpolation
    // error when deciding whether an intersection is already split.
    for (point, value) in [(a, a_value), (b, b_value), (c, c_value), (d, d_value)] {
        if value == overlap_start {
            return SegmentIntersection::Point(point);
        }
    }

    unreachable!("the overlap boundary must be one of the segment endpoints")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_orientations() {
        assert_eq!(
            orientation(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, 1.0)
            ),
            Ok(Orientation::CounterClockwise)
        );
        assert_eq!(
            orientation(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, -1.0)
            ),
            Ok(Orientation::Clockwise)
        );
        assert_eq!(
            orientation(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 1.0),
                Point2::new(2.0, 2.0)
            ),
            Ok(Orientation::Collinear)
        );
    }

    #[test]
    fn orientation_rejects_nan_and_infinity() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                orientation(
                    Point2::new(0.0, 0.0),
                    Point2::new(invalid, 0.0),
                    Point2::new(1.0, 1.0)
                ),
                Err(GeometryError::NonFinitePoint { argument: "b", .. })
            ));
        }
    }

    #[test]
    fn filtered_orientation_certifies_clear_signs_and_argument_reversal() {
        let a = Point2::new(-3.0, -1.0);
        let b = Point2::new(5.0, 0.0);
        let c = Point2::new(1.0, 4.0);

        assert_eq!(
            filtered_orientation(a, b, c),
            Ok(FilteredOrientation::CounterClockwise)
        );
        assert_eq!(
            filtered_orientation(a, c, b),
            Ok(FilteredOrientation::Clockwise)
        );
        assert_eq!(
            filtered_orientation(b, c, a),
            Ok(FilteredOrientation::CounterClockwise)
        );
    }

    #[test]
    fn filtered_orientation_is_invariant_under_exact_translation_and_half_turn() {
        let points = [
            Point2::new(-8.0, -2.0),
            Point2::new(6.0, 1.0),
            Point2::new(-1.0, 9.0),
        ];
        let expected = filtered_orientation(points[0], points[1], points[2]);
        let translated =
            points.map(|point| Point2::new(point.x + 1_048_576.0, point.y - 2_097_152.0));
        let half_turned = points.map(|point| Point2::new(-point.x, -point.y));

        assert_eq!(expected, Ok(FilteredOrientation::CounterClockwise));
        assert_eq!(
            filtered_orientation(translated[0], translated[1], translated[2]),
            expected
        );
        assert_eq!(
            filtered_orientation(half_turned[0], half_turned[1], half_turned[2]),
            expected
        );
    }

    #[test]
    fn filtered_orientation_handles_huge_adjacent_floats_without_an_epsilon() {
        let huge = 2.0_f64.powi(500);
        let adjacent = f64::from_bits(huge.to_bits() + 1);
        let a = Point2::new(huge, huge);
        let b = Point2::new(adjacent, huge);
        let c = Point2::new(huge, adjacent);

        assert_eq!(
            filtered_orientation(a, b, c),
            Ok(FilteredOrientation::CounterClockwise)
        );
        assert_eq!(
            filtered_orientation(a, c, b),
            Ok(FilteredOrientation::Clockwise)
        );
    }

    #[test]
    fn filtered_orientation_fails_closed_for_collinearity_and_known_cancellation() {
        assert_eq!(
            filtered_orientation(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 1.0),
                Point2::new(2.0, 2.0),
            ),
            Ok(FilteredOrientation::Indeterminate)
        );

        let next = 1.0 + f64::EPSILON;
        let next_again = 1.0 + 2.0 * f64::EPSILON;
        let cancellation_points = [
            Point2::new(0.0, 0.0),
            Point2::new(next, 1.0),
            Point2::new(next_again, next),
        ];
        // In exact arithmetic over these binary64 inputs, the determinant is
        // EPSILON^2 and therefore counter-clockwise. Both rounded products are
        // `next_again`, however, so a plain f64 determinant cancels to zero.
        // The legacy API intentionally retains that historical behavior.
        assert_eq!(
            orientation(
                cancellation_points[0],
                cancellation_points[1],
                cancellation_points[2],
            ),
            Ok(Orientation::Collinear)
        );
        assert_eq!(
            filtered_orientation(
                cancellation_points[0],
                cancellation_points[1],
                cancellation_points[2],
            ),
            Ok(FilteredOrientation::Indeterminate)
        );
        assert_eq!(
            exact_orientation(
                cancellation_points[0],
                cancellation_points[1],
                cancellation_points[2],
            ),
            Ok(Orientation::CounterClockwise)
        );
    }

    #[test]
    fn exact_orientation_resolves_determinants_below_binary64_range() {
        let minimum = f64::from_bits(1);
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(minimum, 0.0);
        let c = Point2::new(0.0, minimum);

        assert_eq!(orientation(a, b, c), Ok(Orientation::Collinear));
        assert_eq!(
            filtered_orientation(a, b, c),
            Ok(FilteredOrientation::Indeterminate)
        );
        assert_eq!(
            exact_orientation(a, b, c),
            Ok(Orientation::CounterClockwise)
        );
        assert_eq!(exact_orientation(a, c, b), Ok(Orientation::Clockwise));
        assert_eq!(
            exact_orientation(c, a, b),
            Ok(Orientation::CounterClockwise)
        );

        let huge_a = Point2::new(-f64::MAX, 0.0);
        let huge_b = Point2::new(f64::MAX, 0.0);
        let huge_c = Point2::new(0.0, f64::MAX);
        assert_eq!(
            orientation(huge_a, huge_b, huge_c),
            Err(GeometryError::ArithmeticOverflow)
        );
        assert_eq!(
            filtered_orientation(huge_a, huge_b, huge_c),
            Err(GeometryError::ArithmeticOverflow)
        );
        assert_eq!(
            exact_orientation(huge_a, huge_b, huge_c),
            Ok(Orientation::CounterClockwise)
        );
        assert_eq!(
            exact_orientation(
                Point2::new(-0.0, 0.0),
                Point2::new(1.0, 1.0),
                Point2::new(2.0, 2.0),
            ),
            Ok(Orientation::Collinear)
        );
        assert_eq!(exact_orientation(a, a, c), Ok(Orientation::Collinear));
    }

    #[test]
    fn exact_orientation_rejects_each_non_finite_argument() {
        for (a, b, c, expected_argument) in [
            (
                Point2::new(f64::NAN, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 1.0),
                "a",
            ),
            (
                Point2::new(0.0, 0.0),
                Point2::new(f64::INFINITY, 0.0),
                Point2::new(1.0, 1.0),
                "b",
            ),
            (
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, f64::NEG_INFINITY),
                "c",
            ),
        ] {
            assert!(matches!(
                exact_orientation(a, b, c),
                Err(GeometryError::NonFinitePoint { argument, .. })
                    if argument == expected_argument
            ));
        }
    }

    #[test]
    fn filtered_orientation_fails_closed_when_the_error_bound_is_subnormal() {
        assert_eq!(
            filtered_orientation(
                Point2::new(0.0, 0.0),
                Point2::new(f64::MIN_POSITIVE, 0.0),
                Point2::new(0.0, 1.0),
            ),
            Ok(FilteredOrientation::Indeterminate)
        );
    }

    #[test]
    fn filtered_orientation_rejects_non_finite_and_overflowing_arithmetic() {
        for (a, b, c, expected_argument) in [
            (
                Point2::new(f64::NAN, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 1.0),
                "a",
            ),
            (
                Point2::new(0.0, 0.0),
                Point2::new(f64::INFINITY, 0.0),
                Point2::new(1.0, 1.0),
                "b",
            ),
            (
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, f64::NEG_INFINITY),
                "c",
            ),
        ] {
            assert!(matches!(
                filtered_orientation(a, b, c),
                Err(GeometryError::NonFinitePoint { argument, .. })
                    if argument == expected_argument
            ));
        }

        assert_eq!(
            filtered_orientation(
                Point2::new(-f64::MAX, 0.0),
                Point2::new(0.0, 1.0),
                Point2::new(f64::MAX, 0.0),
            ),
            Err(GeometryError::ArithmeticOverflow)
        );

        let product_overflow = 2.0_f64.powi(600);
        assert_eq!(
            filtered_orientation(
                Point2::new(product_overflow, 0.0),
                Point2::new(0.0, product_overflow),
                Point2::new(0.0, 0.0),
            ),
            Err(GeometryError::ArithmeticOverflow)
        );
    }

    #[test]
    fn point_segment_relation_distinguishes_directional_endpoints_and_strict_interior() {
        let start = Point2::new(-2.0, 1.0);
        let end = Point2::new(4.0, 1.0);
        assert_eq!(
            point_segment_relation(start, start, end),
            Ok(PointSegmentRelation::Start)
        );
        assert_eq!(
            point_segment_relation(end, start, end),
            Ok(PointSegmentRelation::End)
        );
        assert_eq!(
            point_segment_relation(Point2::new(1.0, 1.0), start, end),
            Ok(PointSegmentRelation::StrictInterior)
        );
        for outside in [
            Point2::new(-3.0, 1.0),
            Point2::new(5.0, 1.0),
            Point2::new(1.0, 1.0 + f64::EPSILON),
        ] {
            assert_eq!(
                point_segment_relation(outside, start, end),
                Ok(PointSegmentRelation::Outside)
            );
        }
        assert_eq!(
            point_segment_relation(end, end, start),
            Ok(PointSegmentRelation::Start)
        );
    }

    #[test]
    fn point_segment_relation_rejects_non_finite_and_handles_extreme_geometry_exactly() {
        assert!(matches!(
            point_segment_relation(
                Point2::new(f64::NAN, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
            ),
            Err(GeometryError::NonFinitePoint {
                argument: "point",
                ..
            })
        ));
        assert_eq!(
            point_segment_relation(
                Point2::new(0.0, 1.0),
                Point2::new(-f64::MAX, 0.0),
                Point2::new(f64::MAX, 0.0),
            ),
            Ok(PointSegmentRelation::Outside)
        );
        assert_eq!(
            point_segment_relation(
                Point2::new(0.0, 0.0),
                Point2::new(-f64::MAX, 0.0),
                Point2::new(f64::MAX, 0.0),
            ),
            Ok(PointSegmentRelation::StrictInterior)
        );
    }

    #[test]
    fn point_polygon_relation_handles_concavity_boundary_and_orientation() {
        let mut polygon = vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 4.0),
        ];
        let cases = [
            (Point2::new(1.0, 1.0), PointPolygonRelation::Inside),
            (Point2::new(3.0, 3.5), PointPolygonRelation::Outside),
            (Point2::new(3.0, 3.0), PointPolygonRelation::Boundary),
            (Point2::new(2.0, 2.0), PointPolygonRelation::Boundary),
            (Point2::new(-1.0, 2.0), PointPolygonRelation::Outside),
        ];

        for _ in 0..polygon.len() {
            for (point, expected) in cases {
                assert_eq!(point_polygon_relation(point, &polygon), Ok(expected));
            }
            polygon.rotate_left(1);
        }
        polygon.reverse();
        for (point, expected) in cases {
            assert_eq!(point_polygon_relation(point, &polygon), Ok(expected));
        }
    }

    #[test]
    fn point_polygon_relation_is_exact_for_tiny_and_extreme_polygons() {
        let side = f64::from_bits(485_u64 << 52);
        let tiny = [
            Point2::new(0.0, 0.0),
            Point2::new(side, 0.0),
            Point2::new(0.0, side),
        ];
        assert_eq!(
            point_polygon_relation(Point2::new(side, side), &tiny),
            Ok(PointPolygonRelation::Outside)
        );
        assert_eq!(
            point_polygon_relation(Point2::new(side, 0.0), &tiny),
            Ok(PointPolygonRelation::Boundary)
        );

        let extreme = [
            Point2::new(-f64::MAX, -f64::MAX),
            Point2::new(f64::MAX, -f64::MAX),
            Point2::new(f64::MAX, f64::MAX),
            Point2::new(-f64::MAX, f64::MAX),
        ];
        assert_eq!(
            point_polygon_relation(Point2::new(0.0, 0.0), &extreme),
            Ok(PointPolygonRelation::Inside)
        );
        assert_eq!(
            point_polygon_relation(Point2::new(f64::MAX, 0.0), &extreme),
            Ok(PointPolygonRelation::Boundary)
        );
    }

    #[test]
    fn point_polygon_relation_rejects_non_finite_inputs() {
        assert!(matches!(
            point_polygon_relation(Point2::new(f64::NAN, 0.0), &[]),
            Err(GeometryError::NonFinitePoint {
                argument: "point",
                ..
            })
        ));
        assert!(matches!(
            point_polygon_relation(Point2::new(0.0, 0.0), &[Point2::new(0.0, f64::INFINITY)]),
            Err(GeometryError::NonFinitePoint {
                argument: "polygon point",
                ..
            })
        ));
    }

    #[test]
    fn exact_segment_midpoint_relation_does_not_round_onto_an_endpoint() {
        let next_after_one = f64::from_bits(1.0_f64.to_bits() + 1);
        let square = [
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
            Point2::new(1.0, 2.0),
        ];
        let start = Point2::new(1.0, 1.0);
        let end = Point2::new(next_after_one, 1.0);

        assert_eq!((start.x + end.x) / 2.0, start.x);
        assert_eq!(
            segment_midpoint_polygon_relation(start, end, &square),
            Ok(PointPolygonRelation::Inside)
        );
        assert_eq!(
            segment_midpoint_polygon_relation(end, start, &square),
            Ok(PointPolygonRelation::Inside)
        );

        let slanted_boundary = [
            Point2::new(1.0, 0.0),
            Point2::new(next_after_one, 2.0),
            Point2::new(2.0, 0.0),
        ];
        let rounded_midpoint =
            Point2::new((slanted_boundary[0].x + slanted_boundary[1].x) / 2.0, 1.0);
        assert_eq!(
            point_polygon_relation(rounded_midpoint, &slanted_boundary),
            Ok(PointPolygonRelation::Outside)
        );
        assert_eq!(
            segment_midpoint_polygon_relation(
                slanted_boundary[0],
                slanted_boundary[1],
                &slanted_boundary
            ),
            Ok(PointPolygonRelation::Boundary)
        );
    }

    #[test]
    fn exact_segment_midpoint_relation_handles_subnormal_and_extreme_averages() {
        let unit = f64::from_bits(1);
        let tiny_square = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0 * unit, 0.0),
            Point2::new(2.0 * unit, 2.0 * unit),
            Point2::new(0.0, 2.0 * unit),
        ];
        let tiny_start = Point2::new(0.0, unit);
        let tiny_end = Point2::new(unit, unit);
        assert_eq!((tiny_start.x + tiny_end.x) / 2.0, 0.0);
        assert_eq!(
            segment_midpoint_polygon_relation(tiny_start, tiny_end, &tiny_square),
            Ok(PointPolygonRelation::Inside)
        );

        let extreme_square = [
            Point2::new(-f64::MAX, -1.0),
            Point2::new(f64::MAX, -1.0),
            Point2::new(f64::MAX, 1.0),
            Point2::new(-f64::MAX, 1.0),
        ];
        let extreme_start = Point2::new(-f64::MAX, 0.0);
        let extreme_end = Point2::new(f64::MAX, 0.0);
        assert!((extreme_start.x + extreme_end.x).is_finite());
        assert_eq!(
            segment_midpoint_polygon_relation(extreme_start, extreme_end, &extreme_square),
            Ok(PointPolygonRelation::Inside)
        );
        let max_edge_start = Point2::new(f64::MAX, -1.0);
        let max_edge_end = Point2::new(f64::MAX, 1.0);
        assert!((max_edge_start.x + max_edge_end.x).is_infinite());
        assert_eq!(
            segment_midpoint_polygon_relation(max_edge_start, max_edge_end, &extreme_square),
            Ok(PointPolygonRelation::Boundary)
        );
    }

    #[test]
    fn exact_segment_midpoint_relation_handles_concavity_boundary_and_reversal() {
        let mut polygon = vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 4.0),
        ];
        let outside_chord = (Point2::new(4.0, 4.0), Point2::new(0.0, 4.0));
        let boundary_crossing = (Point2::new(-1.0, 2.0), Point2::new(1.0, 2.0));

        for _ in 0..polygon.len() {
            assert_eq!(
                segment_midpoint_polygon_relation(outside_chord.0, outside_chord.1, &polygon),
                Ok(PointPolygonRelation::Outside)
            );
            assert_eq!(
                segment_midpoint_polygon_relation(
                    boundary_crossing.0,
                    boundary_crossing.1,
                    &polygon
                ),
                Ok(PointPolygonRelation::Boundary)
            );
            polygon.rotate_left(1);
        }
        polygon.reverse();
        assert_eq!(
            segment_midpoint_polygon_relation(outside_chord.1, outside_chord.0, &polygon),
            Ok(PointPolygonRelation::Outside)
        );
        assert_eq!(
            segment_midpoint_polygon_relation(
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 2.0),
                &polygon
            ),
            Ok(PointPolygonRelation::Inside)
        );
    }

    #[test]
    fn exact_segment_midpoint_relation_rejects_non_finite_inputs() {
        assert!(matches!(
            segment_midpoint_polygon_relation(
                Point2::new(f64::INFINITY, 0.0),
                Point2::new(0.0, 0.0),
                &[]
            ),
            Err(GeometryError::NonFinitePoint {
                argument: "start",
                ..
            })
        ));
        assert!(matches!(
            segment_midpoint_polygon_relation(
                Point2::new(0.0, 0.0),
                Point2::new(f64::NAN, 0.0),
                &[]
            ),
            Err(GeometryError::NonFinitePoint {
                argument: "end",
                ..
            })
        ));
    }

    #[test]
    fn finds_crossing_segments() {
        assert_eq!(
            segment_intersection(
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 2.0),
                Point2::new(0.0, 2.0),
                Point2::new(2.0, 0.0)
            ),
            Ok(SegmentIntersection::Point(Point2::new(1.0, 1.0)))
        );
    }

    #[test]
    fn exact_intersection_ratio_survives_subnormal_determinant_cancellation() {
        let value = |fraction| f64::from_bits((535_u64 << 52) | fraction);
        let a = Point2::new(value(0x0002_a0d4_fd38_4c1a), value(0x0001_8a9c_66cf_7513));
        let b = Point2::new(value(0x0002_a0d4_fd38_4c16), value(0x0001_8a9c_66cf_751d));
        let c = Point2::new(value(0x0002_a0d4_fd38_4c1c), value(0x0001_8a9c_66cf_751c));
        let d = Point2::new(value(0x0002_a0d4_fd38_4c18), value(0x0001_8a9c_66cf_7511));
        let expected = Point2::new(value(0x0002_a0d4_fd38_4c19), value(0x0001_8a9c_66cf_7515));

        assert_crossing_permutations(a, b, c, d, expected);
    }

    #[test]
    fn exact_intersection_ratio_survives_normal_determinant_cancellation() {
        let p300 = |fraction| f64::from_bits((1323_u64 << 52) | fraction);
        let p301 = |fraction| f64::from_bits((1324_u64 << 52) | fraction);
        let p302 = |fraction| f64::from_bits((1325_u64 << 52) | fraction);
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(p301(0x000a_9b3e_d95a_0de2), p301(0x000a_b82e_f54e_e35c));
        let c = Point2::new(p300(0x000a_9b3e_d95a_0ddc), p300(0x000a_b82e_f54e_e35e));
        let d = Point2::new(p302(0x0003_f46f_2303_8a6c), p302(0x0004_0a23_37fb_2a84));
        let expected = Point2::new(p301(0x0006_faad_7ef8_7294), p301(0x0007_13ab_aaf4_004f));

        assert_crossing_permutations(a, b, c, d, expected);
    }

    #[test]
    fn exact_intersection_rounds_ties_to_even_across_binary64_boundaries() {
        let one = 1.0_f64.to_bits();
        let two = 2.0_f64.to_bits();
        let cases = [
            (f64::from_bits(one), f64::from_bits(one + 1), one),
            (f64::from_bits(one + 1), f64::from_bits(one + 2), one + 2),
            (
                f64::from_bits(0x000f_ffff_ffff_ffff),
                f64::from_bits(0x0010_0000_0000_0000),
                0x0010_0000_0000_0000,
            ),
            (f64::from_bits(two - 1), f64::from_bits(two), two),
            (
                f64::from_bits((1_u64 << 63) | 1),
                f64::from_bits(1_u64 << 63),
                1_u64 << 63,
            ),
        ];

        for (x0, x1, expected_x_bits) in cases {
            let intersection = segment_intersection(
                Point2::new(x0, 0.0),
                Point2::new(x1, 2.0),
                Point2::new(x0, 2.0),
                Point2::new(x1, 0.0),
            )
            .expect("finite exact intersection");
            let SegmentIntersection::Point(point) = intersection else {
                panic!("expected point intersection, got {intersection:?}");
            };
            assert_eq!(point.x.to_bits(), expected_x_bits);
            assert_eq!(point.y, 1.0);
        }
    }

    #[test]
    fn exact_intersection_handles_non_dyadic_ratio_and_extreme_coordinates() {
        assert_eq!(
            segment_intersection(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 3.0),
                Point2::new(-1.0, 1.0),
                Point2::new(1.0, 1.0),
            ),
            Ok(SegmentIntersection::Point(Point2::new(1.0 / 3.0, 1.0)))
        );
        assert_eq!(
            segment_intersection(
                Point2::new(-f64::MAX, 0.0),
                Point2::new(f64::MAX, 0.0),
                Point2::new(0.0, -f64::MAX),
                Point2::new(0.0, f64::MAX),
            ),
            Ok(SegmentIntersection::Point(Point2::new(0.0, 0.0)))
        );
    }

    #[test]
    fn exact_interior_intersection_that_rounds_to_an_endpoint_fails_closed() {
        let minimum = f64::from_bits(1);
        assert_eq!(
            segment_intersection(
                Point2::new(0.0, 0.0),
                Point2::new(minimum, minimum),
                Point2::new(0.0, minimum),
                Point2::new(minimum, 0.0),
            ),
            Err(GeometryError::ArithmeticOverflow)
        );
    }

    fn assert_crossing_permutations(a: Point2, b: Point2, c: Point2, d: Point2, expected: Point2) {
        for (first_start, first_end, second_start, second_end) in [
            (a, b, c, d),
            (b, a, c, d),
            (a, b, d, c),
            (b, a, d, c),
            (c, d, a, b),
            (d, c, a, b),
            (c, d, b, a),
            (d, c, b, a),
        ] {
            assert_eq!(
                segment_intersection(first_start, first_end, second_start, second_end),
                Ok(SegmentIntersection::Point(expected))
            );
        }
    }

    #[test]
    fn exact_predicates_keep_tiny_adjacent_segments_non_collinear() {
        let side = f64::from_bits(485_u64 << 52);
        let origin = Point2::new(0.0, 0.0);
        let x = Point2::new(side, 0.0);
        let y = Point2::new(0.0, side);

        assert_eq!(orientation(origin, x, y), Ok(Orientation::Collinear));
        assert_eq!(
            exact_orientation(origin, x, y),
            Ok(Orientation::CounterClockwise)
        );
        assert_eq!(
            segment_intersection(origin, x, x, y),
            Ok(SegmentIntersection::Point(x))
        );
        assert_eq!(
            point_segment_relation(y, origin, x),
            Ok(PointSegmentRelation::Outside)
        );
    }

    #[test]
    fn finds_non_collinear_shared_endpoint_without_interpolation() {
        let shared = Point2::new(2.0, 2.0);
        assert_eq!(
            segment_intersection(Point2::new(0.0, 0.0), shared, shared, Point2::new(3.0, 0.0),),
            Ok(SegmentIntersection::Point(shared))
        );
    }

    #[test]
    fn separates_collinear_intersection_cases() {
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(2.0, 0.0);

        assert_eq!(
            segment_intersection(a, b, Point2::new(3.0, 0.0), Point2::new(4.0, 0.0)),
            Ok(SegmentIntersection::None)
        );
        assert_eq!(
            segment_intersection(a, b, Point2::new(2.0, 0.0), Point2::new(4.0, 0.0)),
            Ok(SegmentIntersection::Point(b))
        );
        assert_eq!(
            segment_intersection(a, b, Point2::new(1.0, 0.0), Point2::new(4.0, 0.0)),
            Ok(SegmentIntersection::CollinearOverlap)
        );
    }

    #[test]
    fn handles_degenerate_segments_as_points() {
        let point = Point2::new(1.0, 0.0);
        assert_eq!(
            segment_intersection(point, point, Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)),
            Ok(SegmentIntersection::Point(point))
        );
        assert_eq!(
            segment_intersection(
                Point2::new(3.0, 0.0),
                Point2::new(3.0, 0.0),
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 0.0)
            ),
            Ok(SegmentIntersection::None)
        );
    }

    #[test]
    fn segment_intersection_rejects_non_finite_points() {
        assert!(matches!(
            segment_intersection(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, f64::NAN),
                Point2::new(1.0, 1.0)
            ),
            Err(GeometryError::NonFinitePoint { argument: "c", .. })
        ));
        assert!(matches!(
            segment_intersection(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, 1.0),
                Point2::new(f64::INFINITY, 1.0)
            ),
            Err(GeometryError::NonFinitePoint { argument: "d", .. })
        ));
    }
}
