//! Geometry predicates and structural validation for origami crease patterns.

use std::error::Error;
use std::fmt;

use ori_domain::Point2;

mod validation;

pub use validation::{
    CreasePatternValidation, EdgeEndpoint, ValidationIssue, validate_crease_pattern,
};

/// The orientation of three ordered points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Clockwise,
    CounterClockwise,
    Collinear,
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

fn cross(a: Point2, b: Point2) -> f64 {
    // Keep both products rounded in the same way. Using `mul_add` for only one
    // product can make `cross(v, v)` a tiny non-zero value for ordinary finite
    // coordinates, incorrectly classifying an exactly collinear segment.
    a.x * b.y - a.y * b.x
}

fn point_on_segment(point: Point2, start: Point2, end: Point2) -> Result<bool, GeometryError> {
    if orientation(start, end, point)? != Orientation::Collinear {
        return Ok(false);
    }

    Ok(point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y))
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
