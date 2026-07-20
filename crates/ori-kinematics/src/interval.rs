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
        assert_eq!(OutwardIntervalV1::new(-0.0, 0.0).unwrap().lower().to_bits(), 0);
        assert!(OutwardIntervalV1::new(f64::MAX, f64::MAX)
            .unwrap()
            .add(OutwardIntervalV1::new(f64::MAX, f64::MAX).unwrap())
            .is_err());
    }
}
