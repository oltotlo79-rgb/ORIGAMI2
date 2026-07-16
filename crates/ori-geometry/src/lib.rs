use ori_domain::Point2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Clockwise,
    CounterClockwise,
    Collinear,
}

#[must_use]
pub fn orientation(a: Point2, b: Point2, c: Point2) -> Orientation {
    let determinant = (b.x - a.x).mul_add(c.y - a.y, -(b.y - a.y) * (c.x - a.x));
    if determinant > 0.0 {
        Orientation::CounterClockwise
    } else if determinant < 0.0 {
        Orientation::Clockwise
    } else {
        Orientation::Collinear
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentIntersection {
    None,
    Point(Point2),
    CollinearOverlap,
}

#[must_use]
pub fn segment_intersection(a: Point2, b: Point2, c: Point2, d: Point2) -> SegmentIntersection {
    let r = Point2::new(b.x - a.x, b.y - a.y);
    let s = Point2::new(d.x - c.x, d.y - c.y);
    let cross_rs = cross(r, s);
    let cma = Point2::new(c.x - a.x, c.y - a.y);
    if cross_rs == 0.0 {
        return if cross(cma, r) == 0.0 {
            SegmentIntersection::CollinearOverlap
        } else {
            SegmentIntersection::None
        };
    }
    let t = cross(cma, s) / cross_rs;
    let u = cross(cma, r) / cross_rs;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        SegmentIntersection::Point(Point2::new(a.x + t * r.x, a.y + t * r.y))
    } else {
        SegmentIntersection::None
    }
}

const fn cross(a: Point2, b: Point2) -> f64 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_orientation() {
        assert_eq!(
            orientation(
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(1.0, 1.0)
            ),
            Orientation::CounterClockwise
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
            SegmentIntersection::Point(Point2::new(1.0, 1.0))
        );
    }
}
