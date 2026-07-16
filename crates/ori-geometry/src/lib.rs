//! Geometry predicates and structural validation for origami crease patterns.

use std::error::Error;
use std::fmt;

use ori_domain::Point2;

mod validation;

pub use validation::{
    BoundaryEdgeRef, CreasePatternValidation, EdgeEndpoint, PaperValidation, PaperValidationIssue,
    ValidationIssue, polygon_signed_double_area, validate_crease_pattern, validate_paper,
};

/// The orientation of three ordered points.
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
/// rounding-error bound. This crate does not yet provide an adaptive exact
/// backend, so callers must fail closed or hand an indeterminate result to an
/// exact predicate. It is not safe to resolve it with an epsilon or an ID
/// tie-break.
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
    /// Intermediate `f64` arithmetic overflowed despite all inputs being finite.
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
                formatter.write_str("geometry calculation exceeded the finite f64 range")
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
/// rejected. No adaptive exact fallback is implemented yet; therefore
/// [`FilteredOrientation::Indeterminate`] must never be guessed into a
/// topological sign with an epsilon or an unrelated stable ID.
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

/// Classifies `point` against the directed segment `start -> end`.
///
/// Endpoint identity is checked before collinearity so callers can preserve
/// the original edge direction while treating only
/// [`PointSegmentRelation::StrictInterior`] as a split location. Non-finite
/// inputs and overflowing orientation arithmetic are rejected.
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
    if orientation(start, end, point)? != Orientation::Collinear
        || point.x < start.x.min(end.x)
        || point.x > start.x.max(end.x)
        || point.y < start.y.min(end.y)
        || point.y > start.y.max(end.y)
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

    let r = subtract(b, a)?;
    let s = subtract(d, c)?;
    let c_minus_a = subtract(c, a)?;
    let cross_rs = checked_cross(r, s)?;
    let cross_cma_r = checked_cross(c_minus_a, r)?;

    if cross_rs == 0.0 {
        return if cross_cma_r == 0.0 {
            Ok(classify_collinear_intersection(a, b, c, d, r))
        } else {
            Ok(SegmentIntersection::None)
        };
    }

    let t = checked_cross(c_minus_a, s)? / cross_rs;
    let u = cross_cma_r / cross_rs;
    if !t.is_finite() || !u.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }

    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Ok(SegmentIntersection::Point(intersection_point(
            a, b, c, d, t, u,
        )?))
    } else {
        Ok(SegmentIntersection::None)
    }
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

fn checked_cross(a: Point2, b: Point2) -> Result<f64, GeometryError> {
    let value = cross(a, b);
    if value.is_finite() {
        Ok(value)
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
    direction: Point2,
) -> SegmentIntersection {
    let use_x = direction.x.abs() >= direction.y.abs();
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

fn intersection_point(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
    t: f64,
    u: f64,
) -> Result<Point2, GeometryError> {
    // Preserve original endpoints exactly. This matters for topology checks,
    // which identify a correctly split intersection by its shared vertex ID.
    let point = if t == 0.0 {
        a
    } else if t == 1.0 {
        b
    } else if u == 0.0 {
        c
    } else if u == 1.0 {
        d
    } else {
        Point2::new(
            (1.0 - t).mul_add(a.x, t * b.x),
            (1.0 - t).mul_add(a.y, t * b.y),
        )
    };

    if point.x.is_finite() && point.y.is_finite() {
        Ok(point)
    } else {
        Err(GeometryError::ArithmeticOverflow)
    }
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
    fn point_segment_relation_rejects_non_finite_and_overflowing_geometry() {
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
            Err(GeometryError::ArithmeticOverflow)
        );
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
