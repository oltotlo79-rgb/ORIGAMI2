use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum OutwardIntervalErrorV1 {
    #[error("interval endpoint is not a supported finite normal value")]
    InvalidEndpoint,
    #[error("interval operation crosses a forbidden zero denominator")]
    DivisionByZeroInterval,
    #[error("interval work accounting overflowed")]
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutwardIntervalV1 {
    lower: f64,
    upper: f64,
    work: usize,
}

impl OutwardIntervalV1 {
    pub fn new(lower: f64, upper: f64) -> Result<Self, OutwardIntervalErrorV1> {
        let lower = canonical_finite(lower)?;
        let upper = canonical_finite(upper)?;
        if lower > upper {
            return Err(OutwardIntervalErrorV1::InvalidEndpoint);
        }
        Ok(Self {
            lower,
            upper,
            work: 0,
        })
    }

    pub fn from_rounded(value: f64) -> Result<Self, OutwardIntervalErrorV1> {
        if value == 0.0 {
            return Self::new(0.0, 0.0);
        }
        Self::new(next_down(value), next_up(value))
    }

    #[must_use]
    pub const fn lower(&self) -> f64 {
        self.lower
    }

    #[must_use]
    pub const fn upper(&self) -> f64 {
        self.upper
    }

    #[must_use]
    pub const fn work(&self) -> usize {
        self.work
    }

    #[must_use]
    pub fn gamma_bound(&self) -> f64 {
        let n = self.work as f64;
        n * f64::EPSILON / (1.0 - n * f64::EPSILON)
    }

    pub fn add(self, rhs: Self) -> Result<Self, OutwardIntervalErrorV1> {
        binary(self, rhs, self.lower + rhs.lower, self.upper + rhs.upper)
    }

    pub fn sub(self, rhs: Self) -> Result<Self, OutwardIntervalErrorV1> {
        binary(self, rhs, self.lower - rhs.upper, self.upper - rhs.lower)
    }

    pub fn mul(self, rhs: Self) -> Result<Self, OutwardIntervalErrorV1> {
        let values = [
            self.lower * rhs.lower,
            self.lower * rhs.upper,
            self.upper * rhs.lower,
            self.upper * rhs.upper,
        ];
        binary(
            self,
            rhs,
            values.into_iter().fold(f64::INFINITY, f64::min),
            values.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    pub fn div(self, rhs: Self) -> Result<Self, OutwardIntervalErrorV1> {
        if rhs.lower <= 0.0 && rhs.upper >= 0.0 {
            return Err(OutwardIntervalErrorV1::DivisionByZeroInterval);
        }
        self.mul(Self::new(1.0 / rhs.upper, 1.0 / rhs.lower)?)
    }
}

pub fn atan_interval_v1(
    input: OutwardIntervalV1,
    max_work: usize,
) -> Result<OutwardIntervalV1, OutwardIntervalErrorV1> {
    let threshold = libm::sqrt(2.0) - 1.0;
    let (reduced, offset, negate) = if input.lower >= 0.0 && input.upper > threshold {
        let one = OutwardIntervalV1::from_rounded(1.0)?;
        (
            input.sub(one)?.div(input.add(one)?)?,
            Some(OutwardIntervalV1::from_rounded(
                core::f64::consts::FRAC_PI_4,
            )?),
            false,
        )
    } else if input.upper <= 0.0 && input.lower < -threshold {
        let positive = OutwardIntervalV1::new(-input.upper, -input.lower)?;
        let one = OutwardIntervalV1::from_rounded(1.0)?;
        (
            positive.sub(one)?.div(positive.add(one)?)?,
            Some(OutwardIntervalV1::from_rounded(
                core::f64::consts::FRAC_PI_4,
            )?),
            true,
        )
    } else if input.lower >= -threshold && input.upper <= threshold {
        (input, None, false)
    } else {
        return Err(OutwardIntervalErrorV1::InvalidEndpoint);
    };
    let square = reduced.mul(reduced)?;
    let mut polynomial = OutwardIntervalV1::from_rounded(1.0 / 65.0)?;
    for degree in (0..=31).rev() {
        let denominator = (2 * degree + 1) as f64;
        let coefficient = if degree % 2 == 0 {
            1.0 / denominator
        } else {
            -1.0 / denominator
        };
        polynomial = polynomial
            .mul(square)?
            .add(OutwardIntervalV1::from_rounded(coefficient)?)?;
    }
    let mut result = reduced.mul(polynomial)?;
    let max_abs = reduced.lower.abs().max(reduced.upper.abs());
    let remainder = OutwardIntervalV1::from_rounded(libm::pow(max_abs, 67.0) / 67.0)?;
    result = result.add(OutwardIntervalV1::new(-remainder.upper, remainder.upper)?)?;
    if let Some(offset) = offset {
        result = offset.add(result)?;
    }
    if negate {
        result = OutwardIntervalV1::new(-result.upper, -result.lower)?;
    }
    if result.work > max_work {
        return Err(OutwardIntervalErrorV1::ResourceLimit);
    }
    Ok(result)
}

pub fn sin_cos_degrees_interval_v1(
    degrees: OutwardIntervalV1,
    max_work: usize,
) -> Result<(OutwardIntervalV1, OutwardIntervalV1), OutwardIntervalErrorV1> {
    if degrees.lower < 0.0 || degrees.upper > 180.0 || degrees.lower > degrees.upper {
        return Err(OutwardIntervalErrorV1::InvalidEndpoint);
    }
    let work = 96usize
        .checked_add(degrees.work)
        .ok_or(OutwardIntervalErrorV1::ResourceLimit)?;
    if work > max_work {
        return Err(OutwardIntervalErrorV1::ResourceLimit);
    }
    let radians = |value: f64| value * core::f64::consts::PI / 180.0;
    let sin_endpoint = |value: f64| taylor_sin(radians(value));
    let cos_endpoint = |value: f64| taylor_cos(radians(value));
    let (sin_lower, sin_upper) = if degrees.lower == degrees.upper
        && (degrees.lower == 0.0 || degrees.lower == 90.0 || degrees.lower == 180.0)
    {
        let exact = if degrees.lower == 90.0 { 1.0 } else { 0.0 };
        (exact, exact)
    } else {
        let left = sin_endpoint(degrees.lower);
        let right = sin_endpoint(degrees.upper);
        let upper = if degrees.lower <= 90.0 && degrees.upper >= 90.0 {
            1.0
        } else {
            left.1.max(right.1)
        };
        (left.0.min(right.0).max(0.0), upper.min(1.0))
    };
    let exact_cos = |value: f64| match value {
        0.0 => (1.0, 1.0),
        90.0 => (0.0, 0.0),
        180.0 => (-1.0, -1.0),
        _ => cos_endpoint(value),
    };
    let left = exact_cos(degrees.lower);
    let right = exact_cos(degrees.upper);
    Ok((
        OutwardIntervalV1 {
            lower: sin_lower,
            upper: sin_upper,
            work,
        },
        OutwardIntervalV1 {
            lower: right.0.max(-1.0),
            upper: left.1.min(1.0),
            work,
        },
    ))
}

fn taylor_sin(mut x: f64) -> (f64, f64) {
    if x > core::f64::consts::FRAC_PI_2 {
        x = core::f64::consts::PI - x;
    }
    let square = x * x;
    let mut term = x;
    let mut sum = x;
    for k in 1..=12 {
        term *= -square / ((2 * k) * (2 * k + 1)) as f64;
        sum += term;
    }
    let remainder = term.abs() * square / (26.0 * 27.0);
    let error = remainder + taylor_roundoff_bound();
    (next_down(sum - error), next_up(sum + error))
}

fn taylor_cos(x: f64) -> (f64, f64) {
    if x == core::f64::consts::FRAC_PI_2 {
        return (0.0, 0.0);
    }
    let negate = x > core::f64::consts::FRAC_PI_2;
    let x = if negate { core::f64::consts::PI - x } else { x };
    let square = x * x;
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..=12 {
        term *= -square / ((2 * k - 1) * (2 * k)) as f64;
        sum += term;
    }
    let remainder = term.abs() * square / (25.0 * 26.0);
    let error = remainder + taylor_roundoff_bound();
    let (lower, upper) = (next_down(sum - error), next_up(sum + error));
    if negate {
        (-upper, -lower)
    } else {
        (lower, upper)
    }
}

fn taylor_roundoff_bound() -> f64 {
    // Each kernel performs fewer than 64 rounded binary64 operations.  The
    // absolute sum of all terms is below exp(pi/2) < 5.
    let gamma = 64.0 * f64::EPSILON / (1.0 - 64.0 * f64::EPSILON);
    next_up(5.0 * gamma)
}

fn binary(
    lhs: OutwardIntervalV1,
    rhs: OutwardIntervalV1,
    lower: f64,
    upper: f64,
) -> Result<OutwardIntervalV1, OutwardIntervalErrorV1> {
    let work = lhs
        .work
        .checked_add(rhs.work)
        .and_then(|value| value.checked_add(1))
        .ok_or(OutwardIntervalErrorV1::ResourceLimit)?;
    let lower = canonical_finite(next_down(lower))?;
    let upper = canonical_finite(next_up(upper))?;
    Ok(OutwardIntervalV1 { lower, upper, work })
}

fn canonical_finite(value: f64) -> Result<f64, OutwardIntervalErrorV1> {
    if !value.is_finite() || (value != 0.0 && !value.is_normal()) {
        return Err(OutwardIntervalErrorV1::InvalidEndpoint);
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn next_up(value: f64) -> f64 {
    if value == 0.0 {
        f64::from_bits(1)
    } else if value > 0.0 {
        f64::from_bits(value.to_bits() + 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}

fn next_down(value: f64) -> f64 {
    -next_up(-value)
}

#[cfg(test)]
mod tests {
    use num_rational::BigRational;

    use super::*;

    fn exact(value: f64) -> BigRational {
        BigRational::from_float(value).unwrap()
    }

    #[test]
    fn outward_arithmetic_contains_exact_endpoint_operations() {
        let a = OutwardIntervalV1::new(0.1, 0.2).unwrap();
        let b = OutwardIntervalV1::new(0.3, 0.4).unwrap();
        for (actual, exact_values) in [
            (
                a.add(b).unwrap(),
                vec![exact(0.1) + exact(0.3), exact(0.2) + exact(0.4)],
            ),
            (
                a.sub(b).unwrap(),
                vec![exact(0.1) - exact(0.4), exact(0.2) - exact(0.3)],
            ),
            (
                a.mul(b).unwrap(),
                vec![exact(0.1) * exact(0.3), exact(0.2) * exact(0.4)],
            ),
            (
                a.div(b).unwrap(),
                vec![exact(0.1) / exact(0.4), exact(0.2) / exact(0.3)],
            ),
        ] {
            let lower = exact(actual.lower());
            let upper = exact(actual.upper());
            assert!(
                exact_values
                    .iter()
                    .all(|value| value >= &lower && value <= &upper)
            );
            assert!(actual.gamma_bound() > 0.0);
        }
    }

    #[test]
    fn exceptional_binary64_inputs_fail_closed() {
        for value in [f64::NAN, f64::INFINITY, f64::MIN_POSITIVE / 2.0] {
            assert_eq!(
                OutwardIntervalV1::new(value, value),
                Err(OutwardIntervalErrorV1::InvalidEndpoint)
            );
        }
        assert_eq!(
            OutwardIntervalV1::new(1.0, 1.0)
                .unwrap()
                .div(OutwardIntervalV1::new(-1.0, 1.0).unwrap()),
            Err(OutwardIntervalErrorV1::DivisionByZeroInterval)
        );
        assert_eq!(
            OutwardIntervalV1::new(-0.0, 0.0).unwrap().lower().to_bits(),
            0
        );
        assert!(
            OutwardIntervalV1::new(f64::MAX, f64::MAX)
                .unwrap()
                .add(OutwardIntervalV1::new(f64::MAX, f64::MAX).unwrap())
                .is_err()
        );
    }

    #[test]
    fn fixed_atan_kernel_contains_small_and_reduced_reference_values() {
        for value in [-1.0, -0.25, 0.0, 0.25, 1.0] {
            let interval =
                atan_interval_v1(OutwardIntervalV1::from_rounded(value).unwrap(), 512).unwrap();
            let reference = value.atan();
            assert!(interval.lower() <= reference && reference <= interval.upper());
        }
        assert_eq!(
            atan_interval_v1(OutwardIntervalV1::new(-1.0, 1.0).unwrap(), 512),
            Err(OutwardIntervalErrorV1::InvalidEndpoint)
        );
        assert_eq!(
            atan_interval_v1(OutwardIntervalV1::from_rounded(0.25).unwrap(), 1),
            Err(OutwardIntervalErrorV1::ResourceLimit)
        );
        let rational_quarter = BigRational::new(1.into(), 4.into());
        assert_eq!(rational_quarter, exact(0.25));
    }

    #[test]
    fn sin_cos_kernel_is_exact_at_cardinals_and_encloses_nearby_values() {
        for (degrees, expected_sin, expected_cos) in
            [(0.0, 0.0, 1.0), (90.0, 1.0, 0.0), (180.0, 0.0, -1.0)]
        {
            let point = OutwardIntervalV1::new(degrees, degrees).unwrap();
            let (sin, cos) = sin_cos_degrees_interval_v1(point, 96).unwrap();
            assert_eq!((sin.lower(), sin.upper()), (expected_sin, expected_sin));
            assert_eq!((cos.lower(), cos.upper()), (expected_cos, expected_cos));
        }
        let near = OutwardIntervalV1::new(89.999, 90.001).unwrap();
        let (sin, cos) = sin_cos_degrees_interval_v1(near, 96).unwrap();
        assert!(sin.lower() <= (89.999_f64.to_radians()).sin());
        assert_eq!(sin.upper(), 1.0);
        assert!(cos.lower() < 0.0 && cos.upper() > 0.0);
        assert_eq!(
            sin_cos_degrees_interval_v1(near, 95),
            Err(OutwardIntervalErrorV1::ResourceLimit)
        );
    }
}
