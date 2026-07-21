use num_bigint::BigInt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DyadicPointV1 {
    pub x_numerator: i128,
    pub y_numerator: i128,
    pub denominator_power: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DyadicSegmentV1 {
    pub start: DyadicPointV1,
    pub end: DyadicPointV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactSegmentRelationV1 {
    Disjoint,
    EndpointTouch,
    ProperCrossing,
    CollinearOverlap,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactDyadicIntersectionLimitsV1 {
    pub max_denominator_power: u32,
    pub max_integer_bits: usize,
}
impl Default for ExactDyadicIntersectionLimitsV1 {
    fn default() -> Self {
        Self {
            max_denominator_power: 4096,
            max_integer_bits: 8192,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ExactDyadicIntersectionErrorV1 {
    #[error("exact dyadic predicate exceeds its configured bound")]
    ResourceLimit,
    #[error("segment is degenerate")]
    Degenerate,
}

pub fn classify_exact_dyadic_segment_intersection_v1(
    first: DyadicSegmentV1,
    second: DyadicSegmentV1,
    limits: ExactDyadicIntersectionLimitsV1,
) -> Result<ExactSegmentRelationV1, ExactDyadicIntersectionErrorV1> {
    let power = [
        first.start.denominator_power,
        first.end.denominator_power,
        second.start.denominator_power,
        second.end.denominator_power,
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    if power > limits.max_denominator_power {
        return Err(ExactDyadicIntersectionErrorV1::ResourceLimit);
    }
    let point = |p: DyadicPointV1| -> Result<(BigInt, BigInt), ExactDyadicIntersectionErrorV1> {
        let shift = (power - p.denominator_power) as usize;
        let x = BigInt::from(p.x_numerator) << shift;
        let y = BigInt::from(p.y_numerator) << shift;
        if x.bits() as usize > limits.max_integer_bits
            || y.bits() as usize > limits.max_integer_bits
        {
            return Err(ExactDyadicIntersectionErrorV1::ResourceLimit);
        }
        Ok((x, y))
    };
    let a = point(first.start)?;
    let b = point(first.end)?;
    let c = point(second.start)?;
    let d = point(second.end)?;
    if a == b || c == d {
        return Err(ExactDyadicIntersectionErrorV1::Degenerate);
    }
    let orient = |p: &(BigInt, BigInt), q: &(BigInt, BigInt), r: &(BigInt, BigInt)| {
        (&q.0 - &p.0) * (&r.1 - &p.1) - (&q.1 - &p.1) * (&r.0 - &p.0)
    };
    let o1 = orient(&a, &b, &c);
    let o2 = orient(&a, &b, &d);
    let o3 = orient(&c, &d, &a);
    let o4 = orient(&c, &d, &b);
    let opposite = |x: &BigInt, y: &BigInt| {
        (x.sign() == num_bigint::Sign::Minus && y.sign() == num_bigint::Sign::Plus)
            || (x.sign() == num_bigint::Sign::Plus && y.sign() == num_bigint::Sign::Minus)
    };
    if opposite(&o1, &o2) && opposite(&o3, &o4) {
        return Ok(ExactSegmentRelationV1::ProperCrossing);
    }
    let between = |p: &(BigInt, BigInt), q: &(BigInt, BigInt), r: &(BigInt, BigInt)| {
        q.0 >= p.0.clone().min(r.0.clone())
            && q.0 <= p.0.clone().max(r.0.clone())
            && q.1 >= p.1.clone().min(r.1.clone())
            && q.1 <= p.1.clone().max(r.1.clone())
    };
    let touches = [
        (&o1, &a, &c, &b),
        (&o2, &a, &d, &b),
        (&o3, &c, &a, &d),
        (&o4, &c, &b, &d),
    ]
    .into_iter()
    .filter(|(o, p, q, r)| o == &&BigInt::from(0) && between(p, q, r))
    .count();
    if o1 == BigInt::from(0)
        && o2 == BigInt::from(0)
        && o3 == BigInt::from(0)
        && o4 == BigInt::from(0)
    {
        return Ok(if touches >= 2 {
            ExactSegmentRelationV1::CollinearOverlap
        } else if touches == 1 {
            ExactSegmentRelationV1::EndpointTouch
        } else {
            ExactSegmentRelationV1::Disjoint
        });
    }
    Ok(if touches > 0 {
        ExactSegmentRelationV1::EndpointTouch
    } else {
        ExactSegmentRelationV1::Disjoint
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: i128, y: i128, power: u32) -> DyadicPointV1 {
        DyadicPointV1 {
            x_numerator: x,
            y_numerator: y,
            denominator_power: power,
        }
    }
    #[test]
    fn near_touch_and_crossing_are_exact() {
        let l = ExactDyadicIntersectionLimitsV1::default();
        let base = DyadicSegmentV1 {
            start: p(0, 0, 0),
            end: p(2, 0, 0),
        };
        assert_eq!(
            classify_exact_dyadic_segment_intersection_v1(
                base,
                DyadicSegmentV1 {
                    start: p(1, 1, 80),
                    end: p(1, 2, 80)
                },
                l
            )
            .unwrap(),
            ExactSegmentRelationV1::Disjoint
        );
        assert_eq!(
            classify_exact_dyadic_segment_intersection_v1(
                base,
                DyadicSegmentV1 {
                    start: p(1, -1, 80),
                    end: p(1, 1, 80)
                },
                l
            )
            .unwrap(),
            ExactSegmentRelationV1::ProperCrossing
        );
        assert_eq!(
            classify_exact_dyadic_segment_intersection_v1(
                base,
                DyadicSegmentV1 {
                    start: p(2, 0, 0),
                    end: p(3, 1, 0)
                },
                l
            )
            .unwrap(),
            ExactSegmentRelationV1::EndpointTouch
        );
        assert!(matches!(
            classify_exact_dyadic_segment_intersection_v1(
                base,
                base,
                ExactDyadicIntersectionLimitsV1 {
                    max_denominator_power: 0,
                    max_integer_bits: 1
                }
            ),
            Err(ExactDyadicIntersectionErrorV1::ResourceLimit)
        ));
    }
}
