use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CanonicalHingeAngles, HingeAngle, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    OutwardIntervalV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalCoefficientV1 {
    pub numerator: i64,
    pub denominator: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HalfAngleDomainV1 {
    angle_degrees: [f64; 2],
    half_angle_tangent: OutwardIntervalV1,
}

impl HalfAngleDomainV1 {
    pub fn prepare(angle_degrees: [f64; 2]) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if !angle_degrees[0].is_finite()
            || !angle_degrees[1].is_finite()
            || angle_degrees[0] >= angle_degrees[1]
            || angle_degrees[0] <= -180.0
            || angle_degrees[1] >= 180.0
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let lower = libm::tan(angle_degrees[0] * core::f64::consts::PI / 360.0);
        let upper = libm::tan(angle_degrees[1] * core::f64::consts::PI / 360.0);
        let lower = OutwardIntervalV1::from_rounded(lower)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = OutwardIntervalV1::from_rounded(upper)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let half_angle_tangent = OutwardIntervalV1::new(lower.lower(), upper.upper())
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        Ok(Self {
            angle_degrees,
            half_angle_tangent,
        })
    }

    #[must_use]
    pub const fn angle_degrees(&self) -> [f64; 2] {
        self.angle_degrees
    }

    #[must_use]
    pub const fn half_angle_tangent(&self) -> OutwardIntervalV1 {
        self.half_angle_tangent
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoleFreeBernsteinCertificateV1 {
    degree: usize,
    positive: bool,
    coefficients: Vec<BigRational>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactBernsteinRangeV1 {
    coefficients: Vec<BigRational>,
}

impl ExactBernsteinRangeV1 {
    fn range_interval(&self) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
        let lower = self
            .coefficients
            .iter()
            .min()
            .and_then(|value| value.to_f64())
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = self
            .coefficients
            .iter()
            .max()
            .and_then(|value| value.to_f64())
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let lower = OutwardIntervalV1::from_rounded(lower)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = OutwardIntervalV1::from_rounded(upper)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        OutwardIntervalV1::new(lower.lower(), upper.upper())
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
    }

    fn derivative(
        &self,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let degree = self.coefficients.len().saturating_sub(1);
        if degree > max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        if degree == 0 {
            return Ok(Self {
                coefficients: vec![BigRational::zero()],
            });
        }
        let coefficients = self
            .coefficients
            .windows(2)
            .map(|window| (&window[1] - &window[0]) * BigInt::from(degree))
            .collect::<Vec<_>>();
        validate_exact_bits(&coefficients, max_coefficient_bits)?;
        Ok(Self { coefficients })
    }

    fn sub_same_degree(
        &self,
        rhs: &Self,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if self.coefficients.len() != rhs.coefficients.len() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        if self.coefficients.len() > max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let coefficients = self
            .coefficients
            .iter()
            .zip(&rhs.coefficients)
            .map(|(left, right)| left - right)
            .collect::<Vec<_>>();
        validate_exact_bits(&coefficients, max_coefficient_bits)?;
        Ok(Self { coefficients })
    }

    fn add_same_degree(
        &self,
        rhs: &Self,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if self.coefficients.len() != rhs.coefficients.len() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        if self.coefficients.len() > max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let coefficients = self
            .coefficients
            .iter()
            .zip(&rhs.coefficients)
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        validate_exact_bits(&coefficients, max_coefficient_bits)?;
        Ok(Self { coefficients })
    }

    fn product(
        &self,
        rhs: &Self,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let work = self
            .coefficients
            .len()
            .checked_mul(rhs.coefficients.len())
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        if work > max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let n = self.coefficients.len() - 1;
        let m = rhs.coefficients.len() - 1;
        let mut coefficients = Vec::with_capacity(n + m + 1);
        for k in 0..=n + m {
            let mut value = BigRational::zero();
            for i in k.saturating_sub(m)..=k.min(n) {
                let j = k - i;
                let weight = binomial(n, i)
                    .checked_mul(binomial(m, j))
                    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
                value += &self.coefficients[i]
                    * &rhs.coefficients[j]
                    * BigRational::new(BigInt::from(weight), BigInt::from(binomial(n + m, k)));
            }
            coefficients.push(value);
        }
        validate_exact_bits(&coefficients, max_coefficient_bits)?;
        Ok(Self { coefficients })
    }

    fn elevate(
        &self,
        target_degree: usize,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let degree = self.coefficients.len() - 1;
        if target_degree < degree {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let raise = target_degree - degree;
        let work = self
            .coefficients
            .len()
            .checked_mul(raise + 1)
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        if work > max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let mut coefficients = Vec::with_capacity(target_degree + 1);
        for i in 0..=target_degree {
            let mut value = BigRational::zero();
            for j in i.saturating_sub(raise)..=i.min(degree) {
                let weight = binomial(degree, j)
                    .checked_mul(binomial(raise, i - j))
                    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
                value += &self.coefficients[j]
                    * BigRational::new(
                        BigInt::from(weight),
                        BigInt::from(binomial(target_degree, i)),
                    );
            }
            coefficients.push(value);
        }
        validate_exact_bits(&coefficients, max_coefficient_bits)?;
        Ok(Self { coefficients })
    }

    fn sub(
        &self,
        rhs: &Self,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let target = self.coefficients.len().max(rhs.coefficients.len()) - 1;
        self.elevate(target, max_coefficient_bits, max_work)?
            .sub_same_degree(
                &rhs.elevate(target, max_coefficient_bits, max_work)?,
                max_coefficient_bits,
                max_work,
            )
    }
}

fn validate_exact_bits(
    coefficients: &[BigRational],
    maximum: u32,
) -> Result<(), CycleSchedulePrepareErrorV1> {
    if coefficients.iter().any(|value| {
        value.numer().bits() > u64::from(maximum) || value.denom().bits() > u64::from(maximum)
    }) {
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    } else {
        Ok(())
    }
}

impl PoleFreeBernsteinCertificateV1 {
    fn range_interval(&self) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
        let lower = self
            .coefficients
            .iter()
            .min()
            .and_then(|value| value.to_f64())
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = self
            .coefficients
            .iter()
            .max()
            .and_then(|value| value.to_f64())
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let lower = OutwardIntervalV1::from_rounded(lower)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = OutwardIntervalV1::from_rounded(upper)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        OutwardIntervalV1::new(lower.lower(), upper.upper())
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
    }
}

pub fn evaluate_pole_free_rational_interval_v1(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    max_work: usize,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    let work = numerator
        .coefficients
        .len()
        .checked_add(denominator.coefficients.len())
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    if work > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let numerator = numerator.range_interval()?;
    let denominator = denominator.range_interval()?;
    if denominator.lower() <= 0.0 && denominator.upper() >= 0.0 {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    numerator
        .div(denominator)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
}

pub fn evaluate_pole_free_rational_dyadic_v1(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    normalized_u: f64,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<BigRational, CycleSchedulePrepareErrorV1> {
    if !normalized_u.is_finite()
        || (normalized_u != 0.0 && !normalized_u.is_normal())
        || !(0.0..=1.0).contains(&normalized_u)
    {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    let parameter = BigRational::from_float(if normalized_u == 0.0 {
        0.0
    } else {
        normalized_u
    })
    .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
    let numerator = evaluate_exact_bernstein_point(
        &numerator.coefficients,
        &parameter,
        max_coefficient_bits,
        max_work,
    )?;
    let denominator = evaluate_exact_bernstein_point(
        &denominator.coefficients,
        &parameter,
        max_coefficient_bits,
        max_work,
    )?;
    if denominator.is_zero() {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    let value = numerator / denominator;
    validate_exact_bits(core::slice::from_ref(&value), max_coefficient_bits)?;
    Ok(value)
}

fn evaluate_exact_bernstein_point(
    coefficients: &[BigRational],
    parameter: &BigRational,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<BigRational, CycleSchedulePrepareErrorV1> {
    let work = coefficients
        .len()
        .checked_mul(coefficients.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    if work > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let one_minus = BigRational::from_integer(1.into()) - parameter;
    let mut level = coefficients.to_vec();
    for remaining in (1..level.len()).rev() {
        for index in 0..remaining {
            level[index] = &level[index] * &one_minus + &level[index + 1] * parameter;
        }
    }
    let value = level
        .into_iter()
        .next()
        .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
    validate_exact_bits(core::slice::from_ref(&value), max_coefficient_bits)?;
    Ok(value)
}

pub fn evaluate_pole_free_atan2_interval_v1(
    y: &PoleFreeBernsteinCertificateV1,
    x: &PoleFreeBernsteinCertificateV1,
    max_work: usize,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    let x_has_endpoint_zero = x.coefficients.first().is_some_and(Zero::is_zero)
        || x.coefficients.last().is_some_and(Zero::is_zero);
    if x_has_endpoint_zero
        && x.positive
        && y.positive
        && y.coefficients.iter().all(|value| value.is_positive())
    {
        let ratio = evaluate_pole_free_rational_interval_v1(x, y, max_work)?;
        let atan = crate::atan_interval_v1(ratio, max_work)
            .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
        let half_pi = OutwardIntervalV1::from_rounded(core::f64::consts::FRAC_PI_2)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        return half_pi
            .sub(atan)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput);
    }
    let ratio = evaluate_pole_free_rational_interval_v1(y, x, max_work)?;
    let mut angle = crate::atan_interval_v1(ratio, max_work)
        .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
    if !x.positive {
        let pi = OutwardIntervalV1::from_rounded(core::f64::consts::PI)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        angle = if y.positive {
            angle.add(pi)
        } else {
            angle.sub(pi)
        }
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    }
    if angle.work() > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    Ok(angle)
}

pub fn evaluate_half_angle_rational_degrees_interval_v1(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    max_work: usize,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    let radians = evaluate_pole_free_atan2_interval_v1(numerator, denominator, max_work)?;
    let two = OutwardIntervalV1::from_rounded(2.0)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    let degrees = OutwardIntervalV1::from_rounded(180.0)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    let pi = OutwardIntervalV1::from_rounded(core::f64::consts::PI)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    let enclosure = radians
        .mul(two)
        .and_then(|value| value.mul(degrees))
        .and_then(|value| value.div(pi))
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    const ENDPOINT_ROUNDING_GUARD_DEGREES: f64 = 1.0e-9;
    if enclosure.lower() < -ENDPOINT_ROUNDING_GUARD_DEGREES
        || enclosure.upper() > 180.0 + ENDPOINT_ROUNDING_GUARD_DEGREES
    {
        return Err(CycleSchedulePrepareErrorV1::AngleRange);
    }
    enclosure
        .intersect_bounds(0.0, 180.0)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
}

pub fn evaluate_half_angle_rational_derivative_interval_v1(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    let p = ExactBernsteinRangeV1 {
        coefficients: numerator.coefficients.clone(),
    };
    let q = ExactBernsteinRangeV1 {
        coefficients: denominator.coefficients.clone(),
    };
    let p_derivative = p.derivative(max_coefficient_bits, max_work)?;
    let q_derivative = q.derivative(max_coefficient_bits, max_work)?;
    let left = p_derivative.product(&q, max_coefficient_bits, max_work)?;
    let right = p.product(&q_derivative, max_coefficient_bits, max_work)?;
    let derivative_numerator = left.sub(&right, max_coefficient_bits, max_work)?;
    let p_squared = p.product(&p, max_coefficient_bits, max_work)?;
    let q_squared = q.product(&q, max_coefficient_bits, max_work)?;
    let denominator_degree = p_squared
        .coefficients
        .len()
        .max(q_squared.coefficients.len())
        - 1;
    let derivative_denominator = p_squared
        .elevate(denominator_degree, max_coefficient_bits, max_work)?
        .add_same_degree(
            &q_squared.elevate(denominator_degree, max_coefficient_bits, max_work)?,
            max_coefficient_bits,
            max_work,
        )?;
    if !derivative_denominator
        .coefficients
        .iter()
        .all(|value| value.is_positive())
    {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    let numerator_interval = derivative_numerator.range_interval()?;
    let denominator_interval = derivative_denominator.range_interval()?;
    numerator_interval
        .div(denominator_interval)
        .and_then(|value| value.mul(OutwardIntervalV1::from_rounded(2.0)?))
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
}

pub fn prepare_pole_free_bernstein_certificate_v1(
    power_coefficients: &[RationalCoefficientV1],
    max_degree: usize,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<PoleFreeBernsteinCertificateV1, CycleSchedulePrepareErrorV1> {
    if power_coefficients.is_empty()
        || power_coefficients.len() > max_degree.saturating_add(1)
        || power_coefficients
            .len()
            .saturating_mul(power_coefficients.len())
            > max_work
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let power = power_coefficients
        .iter()
        .map(|value| {
            if value.denominator == 0
                || value.numerator.unsigned_abs().checked_ilog2().unwrap_or(0) + 1
                    > max_coefficient_bits
                || value.denominator.checked_ilog2().unwrap_or(0) + 1 > max_coefficient_bits
            {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            Ok(BigRational::new(
                BigInt::from(value.numerator),
                BigInt::from(value.denominator),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    prepare_exact_pole_free_bernstein_certificate(power, max_degree, max_coefficient_bits, max_work)
}

fn prepare_exact_pole_free_bernstein_certificate(
    power: Vec<BigRational>,
    max_degree: usize,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<PoleFreeBernsteinCertificateV1, CycleSchedulePrepareErrorV1> {
    prepare_exact_signed_bernstein_certificate(
        power,
        max_degree,
        max_coefficient_bits,
        max_work,
        false,
    )
}

fn prepare_exact_signed_bernstein_certificate(
    power: Vec<BigRational>,
    max_degree: usize,
    max_coefficient_bits: u32,
    max_work: usize,
    allow_endpoint_zero: bool,
) -> Result<PoleFreeBernsteinCertificateV1, CycleSchedulePrepareErrorV1> {
    if power.is_empty()
        || power.len() > max_degree.saturating_add(1)
        || power.len().saturating_mul(power.len()) > max_work
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    validate_exact_bits(&power, max_coefficient_bits)?;
    let degree = power.len() - 1;
    let mut coefficients = Vec::with_capacity(degree + 1);
    for i in 0..=degree {
        let mut value = BigRational::zero();
        for (k, coefficient) in power.iter().enumerate().take(i + 1) {
            value += coefficient
                * BigRational::new(
                    BigInt::from(binomial(i, k)),
                    BigInt::from(binomial(degree, k)),
                );
        }
        coefficients.push(value);
    }
    validate_exact_bits(&coefficients, max_coefficient_bits)?;
    let exact_range = ExactBernsteinRangeV1 { coefficients };
    let strictly_positive = exact_range
        .coefficients
        .iter()
        .all(|value| value.is_positive());
    let strictly_negative = exact_range
        .coefficients
        .iter()
        .all(|value| value.is_negative());
    let endpoint_zero = allow_endpoint_zero
        && exact_range
            .coefficients
            .iter()
            .enumerate()
            .all(|(index, value)| {
                value.is_positive()
                    || (value.is_zero()
                        && (index == 0 || index + 1 == exact_range.coefficients.len()))
            });
    if !strictly_positive && !strictly_negative && !endpoint_zero {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    Ok(PoleFreeBernsteinCertificateV1 {
        degree,
        positive: strictly_positive || endpoint_zero,
        coefficients: exact_range.coefficients,
    })
}

fn affine_reparameterize_power(
    power: &[BigRational],
    domain: &[BigRational; 2],
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<Vec<BigRational>, CycleSchedulePrepareErrorV1> {
    if power.len().saturating_mul(power.len()) > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let a = &domain[0];
    let width = &domain[1] - a;
    let mut result = vec![BigRational::zero(); power.len()];
    for (degree, coefficient) in power.iter().enumerate() {
        for (k, output) in result.iter_mut().enumerate().take(degree + 1) {
            *output += coefficient
                * BigInt::from(binomial(degree, k))
                * a.pow((degree - k) as i32)
                * width.pow(k as i32);
        }
    }
    validate_exact_bits(&result, max_coefficient_bits)?;
    Ok(result)
}

fn binomial(n: usize, k: usize) -> u128 {
    let k = k.min(n - k);
    (0..k).fold(1_u128, |value, i| value * (n - i) as u128 / (i + 1) as u128)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleScheduleEntryInputV1 {
    pub edge: EdgeId,
    pub initial_angle_degrees_bits: u64,
    pub chebyshev_coefficients: Vec<RationalCoefficientV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalfAngleRationalEntryInputV1 {
    pub edge: EdgeId,
    pub u_domain: [RationalCoefficientV1; 2],
    pub numerator_power_coefficients: Vec<RationalCoefficientV1>,
    pub denominator_power_coefficients: Vec<RationalCoefficientV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedHalfAngleRationalEntryV1 {
    edge: EdgeId,
    u_domain: [BigRational; 2],
    numerator_power_coefficients: Vec<BigRational>,
    denominator_power_coefficients: Vec<BigRational>,
    numerator_certificate: PoleFreeBernsteinCertificateV1,
    denominator_certificate: PoleFreeBernsteinCertificateV1,
    derivative_bound_degrees_bits: u64,
}

impl PreparedHalfAngleRationalEntryV1 {
    pub fn prepare(
        input: HalfAngleRationalEntryInputV1,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let to_exact = |value: RationalCoefficientV1| {
            if value.denominator == 0 {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            Ok(BigRational::new(
                BigInt::from(value.numerator),
                BigInt::from(value.denominator),
            ))
        };
        let u_domain = [to_exact(input.u_domain[0])?, to_exact(input.u_domain[1])?];
        if u_domain[0] >= u_domain[1] {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let numerator_power_coefficients = input
            .numerator_power_coefficients
            .into_iter()
            .map(to_exact)
            .collect::<Result<Vec<_>, _>>()?;
        let denominator_power_coefficients = input
            .denominator_power_coefficients
            .into_iter()
            .map(to_exact)
            .collect::<Result<Vec<_>, _>>()?;
        let numerator_certificate = prepare_exact_signed_bernstein_certificate(
            affine_reparameterize_power(
                &numerator_power_coefficients,
                &u_domain,
                limits.max_coefficient_bits,
                limits.max_work,
            )?,
            limits.max_degree,
            limits.max_coefficient_bits,
            limits.max_work,
            true,
        )?;
        let denominator_certificate = prepare_exact_signed_bernstein_certificate(
            affine_reparameterize_power(
                &denominator_power_coefficients,
                &u_domain,
                limits.max_coefficient_bits,
                limits.max_work,
            )?,
            limits.max_degree,
            limits.max_coefficient_bits,
            limits.max_work,
            true,
        )?;
        if numerator_certificate
            .coefficients
            .iter()
            .zip(&denominator_certificate.coefficients)
            .any(|(numerator, denominator)| numerator.is_zero() && denominator.is_zero())
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let derivative = evaluate_half_angle_rational_derivative_interval_v1(
            &numerator_certificate,
            &denominator_certificate,
            limits.max_coefficient_bits,
            limits.max_work,
        )?;
        let radians_bound = derivative.lower().abs().max(derivative.upper().abs());
        let derivative_bound_degrees = radians_bound * 180.0 / core::f64::consts::PI;
        if !derivative_bound_degrees.is_finite() {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        Ok(Self {
            edge: input.edge,
            u_domain,
            numerator_power_coefficients,
            denominator_power_coefficients,
            numerator_certificate,
            denominator_certificate,
            derivative_bound_degrees_bits: derivative_bound_degrees.to_bits().saturating_add(1),
        })
    }

    #[must_use]
    pub const fn edge(&self) -> EdgeId {
        self.edge
    }

    fn evaluate_degrees(&self, parameter: f64) -> Option<f64> {
        if !(0.0..=1.0).contains(&parameter) {
            return None;
        }
        let lower = self.u_domain[0].to_f64()?;
        let upper = self.u_domain[1].to_f64()?;
        let u = lower + (upper - lower) * parameter;
        let evaluate = |coefficients: &[BigRational]| {
            coefficients
                .iter()
                .rev()
                .try_fold(0.0_f64, |value, coefficient| {
                    Some(value * u + coefficient.to_f64()?)
                })
        };
        let numerator = evaluate(&self.numerator_power_coefficients)?;
        let denominator = evaluate(&self.denominator_power_coefficients)?;
        let angle = 2.0 * numerator.atan2(denominator).to_degrees();
        angle.is_finite().then_some(angle)
    }

    pub fn evaluate_exact(
        &self,
        u: RationalCoefficientV1,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<BigRational, CycleSchedulePrepareErrorV1> {
        if u.denominator == 0 {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let u = BigRational::new(BigInt::from(u.numerator), BigInt::from(u.denominator));
        if u < self.u_domain[0] || u > self.u_domain[1] {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let numerator = evaluate_exact_power_horner(
            &self.numerator_power_coefficients,
            &u,
            max_coefficient_bits,
            max_work,
        )?;
        let denominator = evaluate_exact_power_horner(
            &self.denominator_power_coefficients,
            &u,
            max_coefficient_bits,
            max_work,
        )?;
        if denominator.is_zero() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let value = numerator / denominator;
        validate_exact_bits(core::slice::from_ref(&value), max_coefficient_bits)?;
        Ok(value)
    }

    pub fn angle_enclosure(
        &self,
        max_work: usize,
    ) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
        evaluate_half_angle_rational_degrees_interval_v1(
            &self.numerator_certificate,
            &self.denominator_certificate,
            max_work,
        )
    }

    fn angle_enclosure_dyadic(
        &self,
        depth: u32,
        index: u64,
        max_coefficient_bits: u32,
        max_degree: usize,
        max_work: usize,
    ) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
        if depth >= 64 || index >= (1u64 << depth) {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let denominator = BigInt::from(1u64 << depth);
        let width = &self.u_domain[1] - &self.u_domain[0];
        let lower =
            &self.u_domain[0] + &width * BigRational::new(BigInt::from(index), denominator.clone());
        let upper =
            &self.u_domain[0] + width * BigRational::new(BigInt::from(index + 1), denominator);
        let domain = [lower, upper];
        let numerator = prepare_exact_signed_bernstein_certificate(
            affine_reparameterize_power(
                &self.numerator_power_coefficients,
                &domain,
                max_coefficient_bits,
                max_work,
            )?,
            max_degree,
            max_coefficient_bits,
            max_work,
            true,
        )?;
        let denominator = prepare_exact_signed_bernstein_certificate(
            affine_reparameterize_power(
                &self.denominator_power_coefficients,
                &domain,
                max_coefficient_bits,
                max_work,
            )?,
            max_degree,
            max_coefficient_bits,
            max_work,
            true,
        )?;
        if numerator
            .coefficients
            .iter()
            .zip(&denominator.coefficients)
            .any(|(numerator, denominator)| numerator.is_zero() && denominator.is_zero())
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        evaluate_half_angle_rational_degrees_interval_v1(&numerator, &denominator, max_work)
    }

    fn endpoint_angle_enclosure(
        &self,
        upper: bool,
        max_coefficient_bits: u32,
        max_work: usize,
    ) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
        let u = &self.u_domain[usize::from(upper)];
        let numerator = evaluate_exact_power_horner(
            &self.numerator_power_coefficients,
            u,
            max_coefficient_bits,
            max_work,
        )?;
        let denominator = evaluate_exact_power_horner(
            &self.denominator_power_coefficients,
            u,
            max_coefficient_bits,
            max_work,
        )?;
        if numerator.is_zero() && denominator.is_zero() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let certificate = |value: BigRational| PoleFreeBernsteinCertificateV1 {
            degree: 0,
            positive: !value.is_negative(),
            coefficients: vec![value],
        };
        evaluate_half_angle_rational_degrees_interval_v1(
            &certificate(numerator),
            &certificate(denominator),
            max_work,
        )
    }
}

fn evaluate_exact_power_horner(
    coefficients: &[BigRational],
    u: &BigRational,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<BigRational, CycleSchedulePrepareErrorV1> {
    if coefficients.is_empty() || coefficients.len() > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let value = coefficients
        .iter()
        .rev()
        .fold(BigRational::zero(), |value, coefficient| {
            value * u + coefficient
        });
    validate_exact_bits(core::slice::from_ref(&value), max_coefficient_bits)?;
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleScheduleLimitsV1 {
    pub max_hinges: usize,
    pub max_degree: usize,
    pub max_coefficient_bits: u32,
    pub max_work: usize,
}

impl Default for CycleScheduleLimitsV1 {
    fn default() -> Self {
        Self {
            max_hinges: 64,
            max_degree: 8,
            max_coefficient_bits: 53,
            max_work: 576,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CycleSchedulePrepareErrorV1 {
    #[error("cycle schedule input is malformed")]
    InvalidInput,
    #[error("cycle schedule order or carrier set is not canonical")]
    NonCanonical,
    #[error("cycle schedule exceeds its work limits")]
    ResourceLimit,
    #[error("cycle schedule leaves the physical hinge-angle range")]
    AngleRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiHingePathCandidateLimitsV1 {
    pub max_hinges: usize,
    pub max_candidates: usize,
    pub max_work: usize,
}

impl Default for MultiHingePathCandidateLimitsV1 {
    fn default() -> Self {
        Self {
            max_hinges: 64,
            max_candidates: 1,
            max_work: 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MultiHingePathCandidateErrorV1 {
    #[error("the graph, fixed face, or endpoint registry is inconsistent")]
    InvalidBinding,
    #[error("the endpoint angle vector contains no motion")]
    NoMotion,
    #[error("candidate generation exceeded its explicit resource limits")]
    ResourceLimit,
    #[error("the generated candidate could not satisfy schedule admission")]
    CandidateRejected,
}

/// Read-only candidate transport. It is neither closure nor collision
/// authority and cannot authorize project mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedMultiHingePathCandidateV1 {
    schedule: CanonicalCycleScheduleV1,
    moving_hinges: Vec<EdgeId>,
}

impl GeneratedMultiHingePathCandidateV1 {
    #[must_use]
    pub const fn schedule(&self) -> &CanonicalCycleScheduleV1 {
        &self.schedule
    }

    #[must_use]
    pub fn moving_hinges(&self) -> &[EdgeId] {
        &self.moving_hinges
    }

    #[must_use]
    pub const fn authorizes_closure(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }
}

/// Admits a caller-supplied canonical schedule as a detached path candidate.
/// Both endpoints must match bit-for-bit and at least one hinge must move.
pub fn admit_canonical_multi_hinge_path_candidate_v1(
    schedule: CanonicalCycleScheduleV1,
    initial: &CanonicalHingeAngles,
    requested: &CanonicalHingeAngles,
) -> Result<GeneratedMultiHingePathCandidateV1, MultiHingePathCandidateErrorV1> {
    let lower = schedule
        .evaluate(0.0)
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    let upper = schedule
        .evaluate(1.0)
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    if lower != *initial || upper != *requested {
        return Err(MultiHingePathCandidateErrorV1::InvalidBinding);
    }
    let moving_hinges = initial
        .as_slice()
        .iter()
        .zip(requested.as_slice())
        .filter_map(|(initial, requested)| {
            (initial.edge() == requested.edge()
                && initial.angle_degrees().to_bits() != requested.angle_degrees().to_bits())
            .then_some(initial.edge())
        })
        .collect::<Vec<_>>();
    if moving_hinges.is_empty() {
        return Err(MultiHingePathCandidateErrorV1::NoMotion);
    }
    Ok(GeneratedMultiHingePathCandidateV1 {
        schedule,
        moving_hinges,
    })
}

/// Generates the deterministic straight segment in complete hinge-angle
/// space. This is only a candidate; cyclic closure and collision clearance
/// must be proved independently over its full domain.
pub fn generate_linear_multi_hinge_path_candidate_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    initial: &CanonicalHingeAngles,
    requested: &CanonicalHingeAngles,
    limits: MultiHingePathCandidateLimitsV1,
) -> Result<GeneratedMultiHingePathCandidateV1, MultiHingePathCandidateErrorV1> {
    let hinges = geometry.hinges();
    let mut geometry_faces = geometry.face_ids().to_vec();
    geometry_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if geometry_faces != audit.faces()
        || !audit.faces().contains(&fixed_face)
        || hinges.len() != initial.as_slice().len()
        || hinges.len() != requested.as_slice().len()
    {
        return Err(MultiHingePathCandidateErrorV1::InvalidBinding);
    }
    if hinges.len() > limits.max_hinges || limits.max_candidates == 0 {
        return Err(MultiHingePathCandidateErrorV1::ResourceLimit);
    }
    let work = hinges
        .len()
        .checked_mul(2)
        .ok_or(MultiHingePathCandidateErrorV1::ResourceLimit)?;
    if work > limits.max_work {
        return Err(MultiHingePathCandidateErrorV1::ResourceLimit);
    }
    let mut expected = hinges.iter().map(|hinge| hinge.edge()).collect::<Vec<_>>();
    expected.sort_unstable_by_key(EdgeId::canonical_bytes);
    if initial
        .as_slice()
        .iter()
        .map(|angle| angle.edge())
        .ne(expected.iter().copied())
        || requested
            .as_slice()
            .iter()
            .map(|angle| angle.edge())
            .ne(expected.iter().copied())
    {
        return Err(MultiHingePathCandidateErrorV1::InvalidBinding);
    }
    let mut moving_hinges = Vec::new();
    let entries = initial
        .as_slice()
        .iter()
        .zip(requested.as_slice())
        .map(|(start, end)| {
            let start_value = start.angle_degrees();
            let end_value = end.angle_degrees();
            if start_value.to_bits() != end_value.to_bits() {
                moving_hinges.push(start.edge());
            }
            let midpoint = start_value + (end_value - start_value) * 0.5;
            let half_delta = (end_value - start_value) * 0.5;
            Ok(CycleScheduleEntryInputV1 {
                edge: start.edge(),
                initial_angle_degrees_bits: midpoint.to_bits(),
                chebyshev_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    binary64_to_rational_coefficient_v1(half_delta)?,
                ],
            })
        })
        .collect::<Result<Vec<_>, MultiHingePathCandidateErrorV1>>()?;
    if moving_hinges.is_empty() {
        return Err(MultiHingePathCandidateErrorV1::NoMotion);
    }
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: limits.max_hinges,
        max_degree: 1,
        max_coefficient_bits: 63,
        max_work: limits.max_work,
    };
    let schedule = CanonicalCycleScheduleV1::prepare(
        geometry,
        audit,
        fixed_face,
        [0.0, 1.0],
        entries,
        schedule_limits,
    )
    .map_err(|_| MultiHingePathCandidateErrorV1::CandidateRejected)?;
    for (parameter, expected) in [(0.0, initial), (1.0, requested)] {
        let evaluated = schedule
            .evaluate(parameter)
            .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
        if evaluated
            .as_slice()
            .iter()
            .zip(expected.as_slice())
            .any(|(actual, expected)| {
                actual.edge() != expected.edge()
                    || actual.angle_degrees().to_bits() != expected.angle_degrees().to_bits()
            })
        {
            return Err(MultiHingePathCandidateErrorV1::CandidateRejected);
        }
    }
    Ok(GeneratedMultiHingePathCandidateV1 {
        schedule,
        moving_hinges,
    })
}

fn binary64_to_rational_coefficient_v1(
    value: f64,
) -> Result<RationalCoefficientV1, MultiHingePathCandidateErrorV1> {
    if !value.is_finite() {
        return Err(MultiHingePathCandidateErrorV1::CandidateRejected);
    }
    let rational =
        BigRational::from_float(value).ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    let numerator = rational
        .numer()
        .to_i64()
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    let denominator = rational
        .denom()
        .to_u64()
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    Ok(RationalCoefficientV1 {
        numerator,
        denominator,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    edge: EdgeId,
    initial: f64,
    coefficients: Vec<f64>,
    derivative_bound: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalCycleScheduleV1 {
    binding_fingerprint: [u8; 32],
    schedule_fingerprint: [u8; 32],
    fixed_face: FaceId,
    domain: [f64; 2],
    entries: Vec<Entry>,
    half_angle_entries: Vec<PreparedHalfAngleRationalEntryV1>,
}

impl CanonicalCycleScheduleV1 {
    pub fn prepare(
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        domain: [f64; 2],
        entries: Vec<CycleScheduleEntryInputV1>,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if !domain[0].is_finite()
            || !domain[1].is_finite()
            || domain[0] >= domain[1]
            || entries.is_empty()
            || entries.len() > limits.max_hinges
            || entries.len() != geometry.hinges().len()
            || !audit.faces().contains(&fixed_face)
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let work = entries
            .iter()
            .try_fold(0usize, |sum, entry| {
                sum.checked_add(entry.chebyshev_coefficients.len())
            })
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        if work > limits.max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let mut expected = geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(EdgeId::canonical_bytes);
        if entries.iter().map(|entry| entry.edge).collect::<Vec<_>>() != expected {
            return Err(CycleSchedulePrepareErrorV1::NonCanonical);
        }
        let width = domain[1] - domain[0];
        let mut prepared = Vec::with_capacity(entries.len());
        for input in entries {
            if input.chebyshev_coefficients.len() > limits.max_degree + 1 {
                return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
            }
            let initial = f64::from_bits(input.initial_angle_degrees_bits);
            if !initial.is_finite() || !(0.0..=180.0).contains(&initial) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            let mut coefficients = Vec::with_capacity(input.chebyshev_coefficients.len());
            for coefficient in input.chebyshev_coefficients {
                if coefficient.denominator == 0
                    || coefficient
                        .numerator
                        .unsigned_abs()
                        .checked_ilog2()
                        .unwrap_or(0)
                        .saturating_add(1)
                        > limits.max_coefficient_bits
                    || coefficient
                        .denominator
                        .checked_ilog2()
                        .unwrap_or(0)
                        .saturating_add(1)
                        > limits.max_coefficient_bits
                {
                    return Err(CycleSchedulePrepareErrorV1::InvalidInput);
                }
                coefficients.push(coefficient.numerator as f64 / coefficient.denominator as f64);
            }
            let excursion = coefficients.iter().map(|value| value.abs()).sum::<f64>();
            if initial - excursion < 0.0 || initial + excursion > 180.0 {
                return Err(CycleSchedulePrepareErrorV1::AngleRange);
            }
            let derivative_bound = coefficients
                .iter()
                .enumerate()
                .map(|(degree, value)| 2.0 * (degree * degree) as f64 * value.abs() / width)
                .sum();
            prepared.push(Entry {
                edge: input.edge,
                initial,
                coefficients,
                derivative_bound,
            });
        }
        let schedule_fingerprint = schedule_fingerprint_v1(&prepared, &[]);
        Ok(Self {
            binding_fingerprint: binding_fingerprint(geometry, audit, fixed_face),
            schedule_fingerprint,
            fixed_face,
            domain,
            entries: prepared,
            half_angle_entries: Vec::new(),
        })
    }

    pub fn prepare_half_angle_rational(
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        entries: Vec<HalfAngleRationalEntryInputV1>,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if entries.is_empty()
            || entries.len() > limits.max_hinges
            || entries.len() != geometry.hinges().len()
            || !audit.faces().contains(&fixed_face)
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let mut expected = geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(EdgeId::canonical_bytes);
        if entries.iter().map(|entry| entry.edge).collect::<Vec<_>>() != expected {
            return Err(CycleSchedulePrepareErrorV1::NonCanonical);
        }
        let prepared = entries
            .into_iter()
            .map(|entry| PreparedHalfAngleRationalEntryV1::prepare(entry, limits))
            .collect::<Result<Vec<_>, _>>()?;
        let schedule_fingerprint = schedule_fingerprint_v1(&[], &prepared);
        Ok(Self {
            binding_fingerprint: binding_fingerprint(geometry, audit, fixed_face),
            schedule_fingerprint,
            fixed_face,
            domain: [0.0, 1.0],
            entries: Vec::new(),
            half_angle_entries: prepared,
        })
    }

    pub fn evaluate(&self, parameter: f64) -> Option<CanonicalHingeAngles> {
        if !self.half_angle_entries.is_empty() {
            return self
                .half_angle_entries
                .iter()
                .map(|entry| HingeAngle::new(entry.edge(), entry.evaluate_degrees(parameter)?).ok())
                .collect::<Option<Vec<_>>>()
                .and_then(|angles| CanonicalHingeAngles::new(angles).ok());
        }
        if !parameter.is_finite() || parameter < self.domain[0] || parameter > self.domain[1] {
            return None;
        }
        let x =
            (2.0 * parameter - self.domain[0] - self.domain[1]) / (self.domain[1] - self.domain[0]);
        self.entries
            .iter()
            .map(|entry| {
                let mut b1 = 0.0;
                let mut b2 = 0.0;
                for coefficient in entry.coefficients.iter().rev() {
                    let b0 = 2.0 * x * b1 - b2 + coefficient;
                    b2 = b1;
                    b1 = b0;
                }
                HingeAngle::new(entry.edge, entry.initial + b1 - x * b2).ok()
            })
            .collect::<Option<Vec<_>>>()
            .and_then(|angles| CanonicalHingeAngles::new(angles).ok())
    }

    pub fn evaluate_angle_box(
        &self,
        max_work: usize,
    ) -> Result<Vec<(EdgeId, OutwardIntervalV1)>, CycleSchedulePrepareErrorV1> {
        if self.half_angle_entries.is_empty() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        self.half_angle_entries
            .iter()
            .map(|entry| Ok((entry.edge(), entry.angle_enclosure(max_work)?)))
            .collect()
    }

    /// Evaluates one exact dyadic leaf. Adjacent leaf indices share the exact
    /// rational endpoint used during affine reparameterization.
    pub fn evaluate_angle_box_dyadic(
        &self,
        depth: u32,
        index: u64,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Vec<(EdgeId, OutwardIntervalV1)>, CycleSchedulePrepareErrorV1> {
        if depth >= 64 || self.half_angle_entries.len() > limits.max_hinges {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        if self.half_angle_entries.is_empty() {
            if self.entries.is_empty() || index >= (1u64 << depth) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            let scale = (1u64 << depth) as f64;
            let x = OutwardIntervalV1::new(
                -1.0 + 2.0 * index as f64 / scale,
                -1.0 + 2.0 * (index + 1) as f64 / scale,
            )
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
            return self
                .entries
                .iter()
                .map(|entry| {
                    let zero = OutwardIntervalV1::new(0.0, 0.0)
                        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                    let two = OutwardIntervalV1::from_rounded(2.0)
                        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                    let mut b1 = zero;
                    let mut b2 = zero;
                    for coefficient in entry.coefficients.iter().rev() {
                        let coefficient = OutwardIntervalV1::from_rounded(*coefficient)
                            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                        let b0 = two
                            .mul(x)
                            .and_then(|value| value.mul(b1))
                            .and_then(|value| value.sub(b2))
                            .and_then(|value| value.add(coefficient))
                            .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
                        b2 = b1;
                        b1 = b0;
                    }
                    let initial = OutwardIntervalV1::from_rounded(entry.initial)
                        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                    let angle = initial
                        .add(b1)
                        .and_then(|value| value.sub(x.mul(b2)?))
                        .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
                    if angle.work() > limits.max_work
                        || angle.lower() < 0.0
                        || angle.upper() > 180.0
                    {
                        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
                    }
                    Ok((entry.edge, angle))
                })
                .collect();
        }
        self.half_angle_entries
            .iter()
            .map(|entry| {
                Ok((
                    entry.edge(),
                    entry.angle_enclosure_dyadic(
                        depth,
                        index,
                        limits.max_coefficient_bits,
                        limits.max_degree,
                        limits.max_work,
                    )?,
                ))
            })
            .collect()
    }

    /// Evaluates the exact rational schedule endpoint without replacing it by
    /// a nearby dyadic leaf.
    pub fn evaluate_endpoint_angle_box(
        &self,
        upper: bool,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Vec<(EdgeId, OutwardIntervalV1)>, CycleSchedulePrepareErrorV1> {
        if self.half_angle_entries.is_empty() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        self.half_angle_entries
            .iter()
            .map(|entry| {
                Ok((
                    entry.edge(),
                    entry.endpoint_angle_enclosure(
                        upper,
                        limits.max_coefficient_bits,
                        limits.max_work,
                    )?,
                ))
            })
            .collect()
    }

    #[must_use]
    pub fn derivative_bound(&self, edge: EdgeId) -> Option<f64> {
        if !self.half_angle_entries.is_empty() {
            return self
                .half_angle_entries
                .iter()
                .find(|entry| entry.edge() == edge)
                .map(|entry| f64::from_bits(entry.derivative_bound_degrees_bits));
        }
        self.entries
            .iter()
            .find(|entry| entry.edge == edge)
            .map(|entry| entry.derivative_bound)
    }

    #[must_use]
    pub fn matches_binding(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
    ) -> bool {
        self.fixed_face == fixed_face
            && self.binding_fingerprint == binding_fingerprint(geometry, audit, fixed_face)
    }

    /// Opaque authentication value used to prevent exchanging certificates
    /// between different schedules bound to the same material graph.
    #[doc(hidden)]
    #[must_use]
    pub const fn certificate_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.schedule_fingerprint
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn graph_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding_fingerprint
    }
}

fn schedule_fingerprint_v1(
    entries: &[Entry],
    half_angle_entries: &[PreparedHalfAngleRationalEntryV1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"canonical_cycle_schedule_v1");
    for entry in entries {
        hash.update(entry.edge.canonical_bytes());
        hash.update(entry.initial.to_bits().to_be_bytes());
        for coefficient in &entry.coefficients {
            hash.update(coefficient.to_bits().to_be_bytes());
        }
    }
    for entry in half_angle_entries {
        hash.update(entry.edge.canonical_bytes());
        for value in entry
            .u_domain
            .iter()
            .chain(&entry.numerator_power_coefficients)
            .chain(&entry.denominator_power_coefficients)
        {
            let (numerator_sign, numerator) = value.numer().to_bytes_be();
            let denominator = value.denom().to_bytes_be().1;
            hash.update([match numerator_sign {
                num_bigint::Sign::Minus => 0,
                num_bigint::Sign::NoSign => 1,
                num_bigint::Sign::Plus => 2,
            }]);
            hash.update((numerator.len() as u64).to_be_bytes());
            hash.update(numerator);
            hash.update((denominator.len() as u64).to_be_bytes());
            hash.update(denominator);
        }
    }
    hash.finalize().into()
}

fn binding_fingerprint(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(fixed_face.canonical_bytes());
    for face in audit.faces() {
        hash.update(face.canonical_bytes());
    }
    for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
        hash.update(edge.canonical_bytes());
    }
    for hinge in geometry.hinges() {
        hash.update(hinge.edge().canonical_bytes());
        hash.update(hinge.left_face().canonical_bytes());
        hash.update(hinge.right_face().canonical_bytes());
        hash.update([match hinge.assignment() {
            ori_topology::FoldAssignment::Mountain => 0,
            ori_topology::FoldAssignment::Valley => 1,
        }]);
        for value in [
            hinge.start().x(),
            hinge.start().y(),
            hinge.start().z(),
            hinge.end().x(),
            hinge.end().y(),
            hinge.end().z(),
            hinge.axis().x(),
            hinge.axis().y(),
            hinge.axis().z(),
        ] {
            hash.update(value.to_bits().to_be_bytes());
        }
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, FaceId, ProjectId};
    use ori_topology::{
        BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot,
    };

    use super::*;
    use crate::{Point3, TreeHinge, TreeKinematicsLimits};

    fn fixture() -> (
        MaterialHingeGraphGeometry,
        MaterialHingeGraphAudit,
        FaceId,
        Vec<EdgeId>,
    ) {
        let ns = ProjectId::new();
        let faces = [b"a", b"b", b"c"].map(|name| FaceId::derive_v5(ns, name));
        let edges = [b"ab", b"bc", b"ca"].map(|name| EdgeId::derive_v5(ns, name));
        let topology = TopologySnapshot {
            source_revision: 1,
            faces: faces
                .iter()
                .map(|id| Face {
                    id: *id,
                    key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
                    outer: BoundaryWalk {
                        half_edges: Vec::new(),
                        signed_double_area: 1.0,
                    },
                    holes: Vec::new(),
                    seams: Vec::new(),
                    area: 0.5,
                })
                .collect(),
            edge_incidence: Vec::new(),
            hinge_adjacency: (0..3)
                .map(|i| FaceAdjacency {
                    edge: edges[i],
                    first: faces[i],
                    second: faces[(i + 1) % 3],
                    assignment: FoldAssignment::Mountain,
                })
                .collect(),
            material_components: Vec::new(),
        };
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let end = Point3::new(1.0, 0.0, 0.0).unwrap();
        let hinges = (0..3)
            .map(|i| {
                TreeHinge::new_for_test(
                    edges[i],
                    FoldAssignment::Mountain,
                    faces[i],
                    faces[(i + 1) % 3],
                    start,
                    end,
                    end,
                )
            })
            .collect();
        (
            MaterialHingeGraphGeometry::new_for_test(faces.to_vec(), hinges),
            audit,
            faces[0],
            edges.to_vec(),
        )
    }

    fn entries(edges: &[EdgeId]) -> Vec<CycleScheduleEntryInputV1> {
        let mut entries = edges
            .iter()
            .map(|edge| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 90.0_f64.to_bits(),
                chebyshev_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 10,
                        denominator: 1,
                    },
                ],
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        entries
    }

    #[test]
    fn canonical_schedule_evaluates_and_bounds_derivative() {
        let (geometry, audit, fixed, edges) = fixture();
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            entries(&edges),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        assert!(schedule.matches_binding(&geometry, &audit, fixed));
        let forged_fixed = audit
            .faces()
            .iter()
            .copied()
            .find(|face| *face != fixed)
            .unwrap();
        assert!(!schedule.matches_binding(&geometry, &audit, forged_fixed));
        assert_eq!(
            schedule.evaluate(0.5).unwrap().as_slice()[0].angle_degrees(),
            90.0
        );
        assert_eq!(schedule.derivative_bound(edges[0]), Some(20.0));
        assert!(schedule.evaluate(-0.1).is_none());
    }

    #[test]
    fn linear_multi_hinge_candidate_is_bounded_deterministic_and_not_authority() {
        let (geometry, audit, fixed, mut edges) = fixture();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |value| {
            CanonicalHingeAngles::new(
                edges
                    .iter()
                    .map(|edge| HingeAngle::new(*edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let initial = angles(20.0);
        let requested = angles(40.0);
        let candidate = generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed,
            &initial,
            &requested,
            MultiHingePathCandidateLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(candidate.moving_hinges(), edges);
        assert!(!candidate.authorizes_closure());
        assert!(!candidate.authorizes_collision_clearance());
        for (parameter, expected) in [(0.0, 20.0), (1.0, 40.0)] {
            assert!(
                candidate
                    .schedule()
                    .evaluate(parameter)
                    .unwrap()
                    .as_slice()
                    .iter()
                    .all(|angle| angle.angle_degrees() == expected)
            );
        }
        assert_eq!(
            generate_linear_multi_hinge_path_candidate_v1(
                &geometry,
                &audit,
                fixed,
                &initial,
                &initial,
                MultiHingePathCandidateLimitsV1::default(),
            ),
            Err(MultiHingePathCandidateErrorV1::NoMotion)
        );
        assert_eq!(
            generate_linear_multi_hinge_path_candidate_v1(
                &geometry,
                &audit,
                fixed,
                &initial,
                &requested,
                MultiHingePathCandidateLimitsV1 {
                    max_work: edges.len() * 2 - 1,
                    ..MultiHingePathCandidateLimitsV1::default()
                },
            ),
            Err(MultiHingePathCandidateErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn nonclosing_linear_candidate_never_produces_closure_authority() {
        let (geometry, audit, fixed, mut edges) = fixture();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |value| {
            CanonicalHingeAngles::new(
                edges
                    .iter()
                    .map(|edge| HingeAngle::new(*edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let candidate = generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed,
            &angles(20.0),
            &angles(40.0),
            MultiHingePathCandidateLimitsV1::default(),
        )
        .unwrap();
        let result = geometry.prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            candidate.schedule(),
            1.0e-9,
            crate::DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1_000_000,
                schedule_limits: CycleScheduleLimitsV1 {
                    max_degree: 1,
                    max_work: 100_000,
                    ..CycleScheduleLimitsV1::default()
                },
            },
        );
        assert!(
            matches!(
                result,
                Err(crate::DyadicIntervalClosureErrorV1::ResourceLimit)
            ),
            "{result:?}"
        );
    }

    #[test]
    fn schedule_binding_rejects_assignment_and_axis_aba() {
        let (geometry, audit, fixed, edges) = fixture();
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            entries(&edges),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let rebuild = |change_assignment: bool, change_axis: bool| {
            let hinges = geometry
                .hinges()
                .iter()
                .enumerate()
                .map(|(index, hinge)| {
                    TreeHinge::new_for_test(
                        hinge.edge(),
                        if change_assignment && index == 0 {
                            match hinge.assignment() {
                                FoldAssignment::Mountain => FoldAssignment::Valley,
                                FoldAssignment::Valley => FoldAssignment::Mountain,
                            }
                        } else {
                            hinge.assignment()
                        },
                        hinge.left_face(),
                        hinge.right_face(),
                        hinge.start(),
                        hinge.end(),
                        if change_axis && index == 0 {
                            Point3::new(0.0, 1.0, 0.0).unwrap()
                        } else {
                            hinge.axis()
                        },
                    )
                })
                .collect();
            MaterialHingeGraphGeometry::new_for_test(geometry.face_ids().to_vec(), hinges)
        };
        assert!(!schedule.matches_binding(&rebuild(true, false), &audit, fixed));
        assert!(!schedule.matches_binding(&rebuild(false, true), &audit, fixed));
    }

    #[test]
    fn malformed_order_coefficients_and_limits_fail_closed() {
        let (geometry, audit, fixed, edges) = fixture();
        let limits = CycleScheduleLimitsV1::default();
        let mut reversed = entries(&edges);
        reversed.reverse();
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                reversed,
                limits
            ),
            Err(CycleSchedulePrepareErrorV1::NonCanonical)
        );
        let mut bad = entries(&edges);
        bad[0].chebyshev_coefficients[0].denominator = 0;
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], bad, limits),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
        let mut excessive = entries(&edges);
        excessive[0].chebyshev_coefficients.resize(
            limits.max_degree + 2,
            RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            },
        );
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                excessive,
                limits
            ),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
        let mut wide = entries(&edges);
        wide[0].chebyshev_coefficients[0].numerator = 1_i64 << 54;
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], wide, limits),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
        let mut out_of_range = entries(&edges);
        out_of_range[0].chebyshev_coefficients[1].numerator = 91;
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                out_of_range,
                limits,
            ),
            Err(CycleSchedulePrepareErrorV1::AngleRange)
        );
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                entries(&edges),
                CycleScheduleLimitsV1 {
                    max_work: 1,
                    ..limits
                },
            ),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn exact_bernstein_certificate_proves_only_strict_single_sign_denominators() {
        let positive = prepare_pole_free_bernstein_certificate_v1(
            &[
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            4,
            8,
            16,
        )
        .unwrap();
        assert!(positive.positive);
        assert_eq!(positive.degree, 1);
        assert!(
            positive
                .coefficients
                .iter()
                .all(|value| value.is_positive())
        );
        let denominator = prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 2,
                denominator: 1,
            }],
            4,
            8,
            16,
        )
        .unwrap();
        let quotient =
            evaluate_pole_free_rational_interval_v1(&positive, &denominator, 16).unwrap();
        assert!(quotient.lower() <= 0.5);
        assert!(quotient.upper() >= 1.0);
        assert_eq!(
            evaluate_pole_free_rational_interval_v1(&positive, &denominator, 1),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
        let near_zero = prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 1,
                denominator: 1_u64 << 50,
            }],
            4,
            53,
            16,
        )
        .unwrap();
        let large = evaluate_pole_free_rational_interval_v1(&positive, &near_zero, 16).unwrap();
        assert!(large.upper().is_finite());
        assert!(large.lower() > 0.0);
        for invalid in [
            vec![
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: -2,
                    denominator: 1,
                },
            ],
            vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 0,
            }],
        ] {
            assert!(prepare_pole_free_bernstein_certificate_v1(&invalid, 4, 8, 16).is_err());
        }
        assert!(
            prepare_pole_free_bernstein_certificate_v1(
                &[RationalCoefficientV1 {
                    numerator: 256,
                    denominator: 1,
                }],
                4,
                8,
                16,
            )
            .is_err()
        );
        assert!(
            prepare_pole_free_bernstein_certificate_v1(
                &[RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }; 5],
                3,
                8,
                16,
            )
            .is_err()
        );
    }

    #[test]
    fn half_angle_domain_separates_tangent_poles_and_encloses_known_angles() {
        let domain = HalfAngleDomainV1::prepare([-90.0, 90.0]).unwrap();
        assert_eq!(domain.angle_degrees(), [-90.0, 90.0]);
        let tangent = domain.half_angle_tangent();
        assert!(tangent.lower() <= -1.0);
        assert!(tangent.upper() >= 1.0);
        for invalid in [[-180.0, 0.0], [0.0, 180.0], [1.0, 1.0], [f64::NAN, 1.0]] {
            assert_eq!(
                HalfAngleDomainV1::prepare(invalid),
                Err(CycleSchedulePrepareErrorV1::InvalidInput)
            );
        }
        let near_poles = HalfAngleDomainV1::prepare([-179.0, 179.0]).unwrap();
        assert!(near_poles.half_angle_tangent().lower() < -100.0);
        assert!(near_poles.half_angle_tangent().upper() > 100.0);
    }

    #[test]
    fn pole_free_atan2_encloses_all_strict_quadrants_and_half_angles() {
        let certificate = |numerator| {
            prepare_pole_free_bernstein_certificate_v1(
                &[RationalCoefficientV1 {
                    numerator,
                    denominator: 1,
                }],
                1,
                8,
                4,
            )
            .unwrap()
        };
        let positive = certificate(1);
        let negative = certificate(-1);
        for (y, x, expected) in [
            (&positive, &positive, core::f64::consts::FRAC_PI_4),
            (&positive, &negative, 3.0 * core::f64::consts::FRAC_PI_4),
            (&negative, &negative, -3.0 * core::f64::consts::FRAC_PI_4),
            (&negative, &positive, -core::f64::consts::FRAC_PI_4),
        ] {
            let angle = evaluate_pole_free_atan2_interval_v1(y, x, 262_144).unwrap();
            assert!(angle.lower() <= expected && expected <= angle.upper());
        }
        let half = evaluate_half_angle_rational_degrees_interval_v1(&positive, &positive, 262_144)
            .unwrap();
        assert!(half.lower() <= 90.0 && half.upper() >= 90.0);
        assert_eq!(
            evaluate_pole_free_atan2_interval_v1(&positive, &positive, 1),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn exact_bernstein_derivative_and_same_degree_sub_are_bounded() {
        let range = ExactBernsteinRangeV1 {
            coefficients: [1_i64, 3, 6]
                .map(|value| BigRational::from_integer(value.into()))
                .to_vec(),
        };
        let derivative = range.derivative(16, 8).unwrap();
        assert_eq!(
            derivative.coefficients,
            [4_i64, 6]
                .map(|value| BigRational::from_integer(value.into()))
                .to_vec()
        );
        let difference = range
            .sub_same_degree(
                &ExactBernsteinRangeV1 {
                    coefficients: [1_i64, 1, 1]
                        .map(|value| BigRational::from_integer(value.into()))
                        .to_vec(),
                },
                16,
                8,
            )
            .unwrap();
        assert_eq!(
            difference.coefficients,
            [0_i64, 2, 5]
                .map(|value| BigRational::from_integer(value.into()))
                .to_vec()
        );
        assert_eq!(
            range.derivative(2, 8),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
        assert_eq!(
            range.sub_same_degree(&derivative, 16, 8),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
        let linear = ExactBernsteinRangeV1 {
            coefficients: [1_i64, 2]
                .map(|value| BigRational::from_integer(value.into()))
                .to_vec(),
        };
        let square = linear.product(&linear, 16, 8).unwrap();
        assert_eq!(
            square.coefficients,
            [1_i64, 2, 4]
                .map(|value| BigRational::from_integer(value.into()))
                .to_vec()
        );
        let elevated = linear.elevate(2, 16, 8).unwrap();
        assert_eq!(
            elevated.coefficients,
            [
                BigRational::from_integer(1.into()),
                BigRational::new(3.into(), 2.into()),
                BigRational::from_integer(2.into()),
            ]
        );
        assert_eq!(
            elevated.sub(&linear, 16, 16).unwrap().coefficients,
            vec![BigRational::zero(); 3]
        );
        assert_eq!(
            linear.product(&linear, 16, 1),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
        let p = prepare_pole_free_bernstein_certificate_v1(
            &[
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            2,
            32,
            16,
        )
        .unwrap();
        let q = prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
            2,
            32,
            16,
        )
        .unwrap();
        let derivative =
            evaluate_half_angle_rational_derivative_interval_v1(&p, &q, 64, 64).unwrap();
        assert!(derivative.lower() <= 0.4);
        assert!(derivative.upper() >= 1.0);
        assert_eq!(
            evaluate_pole_free_rational_dyadic_v1(&p, &q, 0.5, 64, 16).unwrap(),
            BigRational::new(3.into(), 2.into())
        );
        for invalid in [f64::NAN, -0.1, 1.1, f64::MIN_POSITIVE / 2.0] {
            assert!(evaluate_pole_free_rational_dyadic_v1(&p, &q, invalid, 64, 16).is_err());
        }
        assert_eq!(
            evaluate_pole_free_rational_dyadic_v1(&p, &q, 0.5, 64, 0),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn half_angle_entry_uses_exact_u_domain_and_horner_evaluation() {
        let entry = PreparedHalfAngleRationalEntryV1::prepare(
            HalfAngleRationalEntryInputV1 {
                edge: EdgeId::derive_v5(ProjectId::new(), b"half-angle-entry"),
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: -1,
                        denominator: 4,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 2,
                    },
                ],
                numerator_power_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 2,
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
            },
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            entry
                .evaluate_exact(
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 4,
                    },
                    128,
                    16,
                )
                .unwrap(),
            BigRational::new(3.into(), 2.into())
        );
        assert!(
            entry
                .evaluate_exact(
                    RationalCoefficientV1 {
                        numerator: 3,
                        denominator: 4,
                    },
                    128,
                    16,
                )
                .is_err()
        );
    }

    #[test]
    fn projective_half_angle_allows_closed_q_zero_endpoint_but_not_crossing_or_origin() {
        let edge = EdgeId::derive_v5(ProjectId::new(), b"projective-endpoint");
        let input = |numerator, denominator| HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: numerator,
            denominator_power_coefficients: denominator,
        };
        let entry = PreparedHalfAngleRationalEntryV1::prepare(
            input(
                vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
                vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
            ),
            CycleScheduleLimitsV1::default(),
        )
        .expect("q=u is projectively regular on the closed domain");
        let endpoint = entry
            .endpoint_angle_enclosure(false, 128, CycleScheduleLimitsV1::default().max_work)
            .unwrap();
        assert!(endpoint.lower() <= 180.0 && endpoint.upper() >= 180.0);
        assert!(
            PreparedHalfAngleRationalEntryV1::prepare(
                input(
                    vec![RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1
                    }],
                    vec![RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1
                    }],
                ),
                CycleScheduleLimitsV1::default(),
            )
            .is_ok(),
            "constant 180-degree projective entry is regular"
        );

        for invalid in [
            input(
                vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
                vec![
                    RationalCoefficientV1 {
                        numerator: -1,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 2,
                        denominator: 1,
                    },
                ],
            ),
            input(
                vec![RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                }],
                vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
            ),
        ] {
            assert!(
                PreparedHalfAngleRationalEntryV1::prepare(
                    invalid,
                    CycleScheduleLimitsV1::default(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn dyadic_angle_boxes_cover_in_canonical_shared_endpoint_order() {
        let (geometry, audit, fixed, edges) = fixture();
        let mut inputs = edges
            .into_iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let evaluated = schedule.evaluate(0.5).expect("certified point evaluation");
        assert!(evaluated.as_slice().iter().all(|angle| {
            (angle.angle_degrees() - 90.0).abs() <= 1.0e-12
                && schedule
                    .derivative_bound(angle.edge())
                    .is_some_and(|bound| bound >= 0.0 && bound <= 1.0e-12)
        }));
        let left = schedule
            .evaluate_angle_box_dyadic(1, 0, CycleScheduleLimitsV1::default())
            .unwrap();
        let right = schedule
            .evaluate_angle_box_dyadic(1, 1, CycleScheduleLimitsV1::default())
            .unwrap();
        assert_eq!(left, right);
        for upper in [false, true] {
            let endpoint = schedule
                .evaluate_endpoint_angle_box(upper, CycleScheduleLimitsV1::default())
                .unwrap();
            assert!(
                endpoint
                    .iter()
                    .all(|(_, angle)| { angle.lower() <= 90.0 && angle.upper() >= 90.0 })
            );
        }
        assert_eq!(
            schedule.evaluate_angle_box_dyadic(1, 2, CycleScheduleLimitsV1::default()),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
    }

    #[test]
    fn dyadic_angle_boxes_admit_a_certified_flat_endpoint() {
        let (geometry, audit, fixed, edges) = fixture();
        let mut inputs = edges
            .into_iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
                denominator_power_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 5,
                        denominator: 1,
                    },
                ],
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();

        let root = schedule
            .evaluate_angle_box_dyadic(
                8,
                0,
                CycleScheduleLimitsV1 {
                    max_work: 1_048_576,
                    ..CycleScheduleLimitsV1::default()
                },
            )
            .unwrap();
        assert!(
            root.iter()
                .all(|(_, angle)| angle.lower() <= 180.0 && angle.upper() >= 180.0)
        );
    }

    #[test]
    fn half_angle_schedule_admission_binds_both_endpoints_bit_exactly() {
        let (geometry, audit, fixed, edges) = fixture();
        let mut inputs = edges
            .into_iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }],
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let admitted =
            admit_canonical_multi_hinge_path_candidate_v1(schedule.clone(), &initial, &requested)
                .unwrap();
        assert_eq!(admitted.moving_hinges().len(), geometry.hinges().len());

        let mut forged = requested.as_slice().to_vec();
        forged[0] = HingeAngle::new(
            forged[0].edge(),
            f64::from_bits(forged[0].angle_degrees().to_bits() - 1),
        )
        .unwrap();
        let forged = CanonicalHingeAngles::new(forged).unwrap();
        assert_eq!(
            admit_canonical_multi_hinge_path_candidate_v1(schedule, &initial, &forged),
            Err(MultiHingePathCandidateErrorV1::InvalidBinding)
        );
    }
}
