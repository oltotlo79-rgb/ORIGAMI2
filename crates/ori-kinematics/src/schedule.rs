use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use ori_domain::{EdgeId, FaceId};
use ori_numeric::{
    DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1, deterministic_atan2_v1,
    deterministic_radians_to_degrees_v1, deterministic_sin_cos_degrees_v1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CanonicalHingeAngles, HingeAngle, KinematicsError, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, OutwardIntervalV1,
};

const MAX_BOUNDED_KAWASAKI_RATIO_DENOMINATOR_V1: u64 = 64;
pub const CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2: &str =
    "canonical_cycle_schedule_deterministic_transcendental_v2";

fn try_schedule_vec_with_capacity_v1<T>(
    capacity: usize,
) -> Result<Vec<T>, CycleSchedulePrepareErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
    Ok(values)
}

fn try_multi_hinge_vec_with_capacity_v1<T>(
    capacity: usize,
) -> Result<Vec<T>, MultiHingePathCandidateErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| MultiHingePathCandidateErrorV1::ResourceLimit)?;
    Ok(values)
}

/// Evaluates `2 * atan2(numerator, denominator)` under the frozen
/// deterministic transcendental model used by canonical half-angle schedules.
///
/// This operation order is shared by schedule evaluation and native endpoint
/// admission. A non-finite input or result is rejected instead of falling
/// back to the host runtime's libm.
#[must_use]
pub fn deterministic_half_angle_ratio_degrees_v1(numerator: f64, denominator: f64) -> Option<f64> {
    let radians = deterministic_atan2_v1(numerator, denominator).ok()?;
    let degrees = deterministic_radians_to_degrees_v1(radians).ok()?;
    let angle = 2.0 * degrees;
    angle.is_finite().then_some(angle)
}

fn deterministic_half_angle_tangent_v1(angle_degrees: f64) -> Option<f64> {
    if !angle_degrees.is_finite() {
        return None;
    }
    match angle_degrees {
        0.0 => return Some(0.0),
        90.0 => return Some(1.0),
        -90.0 => return Some(-1.0),
        _ => {}
    }
    let half_angle_degrees = angle_degrees * 0.5;
    let (sine, cosine) = deterministic_sin_cos_degrees_v1(half_angle_degrees).ok()?;
    if cosine == 0.0 {
        return None;
    }
    let tangent = sine / cosine;
    tangent.is_finite().then_some(tangent)
}

fn coprime_u64_v1(mut left: u64, mut right: u64) -> bool {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left == 1
}

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
        let lower = deterministic_half_angle_tangent_v1(angle_degrees[0])
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = deterministic_half_angle_tangent_v1(angle_degrees[1])
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
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

fn checked_big_int_heap_bytes_upper_bound_v1(value: &BigInt) -> Option<usize> {
    let digit_bits = u64::from(usize::BITS);
    let digits =
        usize::try_from(value.bits().checked_add(digit_bits.checked_sub(1)?)? / digit_bits).ok()?;
    if digits == 0 {
        return Some(0);
    }
    // `num_bigint` can store one digit inline, but a normalized arithmetic
    // result may also retain a two-slot heap buffer for one live digit.
    // Charging twice the live digit count covers that growth slack without
    // relying on its private `Vec` capacity.
    digits
        .checked_mul(2)?
        .checked_mul(std::mem::size_of::<usize>())
}

fn checked_big_rational_heap_bytes_upper_bound_v1(value: &BigRational) -> Option<usize> {
    checked_big_int_heap_bytes_upper_bound_v1(value.numer())?
        .checked_add(checked_big_int_heap_bytes_upper_bound_v1(value.denom())?)
}

fn checked_big_rational_vec_allocation_bytes_v1(
    values: &[BigRational],
    capacity: usize,
) -> Option<usize> {
    let mut total = std::mem::size_of::<BigRational>().checked_mul(capacity)?;
    for value in values {
        total = total.checked_add(checked_big_rational_heap_bytes_upper_bound_v1(value)?)?;
    }
    Some(total)
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
            let mut coefficients = try_schedule_vec_with_capacity_v1(1)?;
            coefficients.push(BigRational::zero());
            return Ok(Self { coefficients });
        }
        let mut coefficients = try_schedule_vec_with_capacity_v1(degree)?;
        for window in self.coefficients.windows(2) {
            coefficients.push((&window[1] - &window[0]) * BigInt::from(degree));
        }
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
        let mut coefficients = try_schedule_vec_with_capacity_v1(self.coefficients.len())?;
        for (left, right) in self.coefficients.iter().zip(&rhs.coefficients) {
            coefficients.push(left - right);
        }
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
        let mut coefficients = try_schedule_vec_with_capacity_v1(self.coefficients.len())?;
        for (left, right) in self.coefficients.iter().zip(&rhs.coefficients) {
            coefficients.push(left + right);
        }
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
        let n = self
            .coefficients
            .len()
            .checked_sub(1)
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let m = rhs
            .coefficients
            .len()
            .checked_sub(1)
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let coefficient_count = n
            .checked_add(m)
            .and_then(|degree| degree.checked_add(1))
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        let mut coefficients = try_schedule_vec_with_capacity_v1(coefficient_count)?;
        for k in 0..coefficient_count {
            let mut value = BigRational::zero();
            for i in k.saturating_sub(m)..=k.min(n) {
                let j = k - i;
                let weight = checked_binomial_v1(n, i)
                    .and_then(|left| {
                        checked_binomial_v1(m, j).and_then(|right| left.checked_mul(right))
                    })
                    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
                let denominator = checked_binomial_v1(n + m, k)
                    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
                value += &self.coefficients[i]
                    * &rhs.coefficients[j]
                    * BigRational::new(BigInt::from(weight), BigInt::from(denominator));
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
        let degree = self
            .coefficients
            .len()
            .checked_sub(1)
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        if target_degree < degree {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let raise = target_degree - degree;
        let raise_terms = raise
            .checked_add(1)
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        let work = self
            .coefficients
            .len()
            .checked_mul(raise_terms)
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        if work > max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let coefficient_count = target_degree
            .checked_add(1)
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        let mut coefficients = try_schedule_vec_with_capacity_v1(coefficient_count)?;
        for i in 0..coefficient_count {
            let mut value = BigRational::zero();
            for j in i.saturating_sub(raise)..=i.min(degree) {
                let weight = checked_binomial_v1(degree, j)
                    .and_then(|left| {
                        checked_binomial_v1(raise, i - j).and_then(|right| left.checked_mul(right))
                    })
                    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
                let denominator = checked_binomial_v1(target_degree, i)
                    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
                value += &self.coefficients[j]
                    * BigRational::new(BigInt::from(weight), BigInt::from(denominator));
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
        let target = self
            .coefficients
            .len()
            .max(rhs.coefficients.len())
            .checked_sub(1)
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
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
    fn checked_nested_retained_bytes_v1(&self) -> Option<usize> {
        checked_big_rational_vec_allocation_bytes_v1(
            &self.coefficients,
            self.coefficients.capacity(),
        )
    }

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
    let mut level = try_schedule_vec_with_capacity_v1(coefficients.len())?;
    level.extend_from_slice(coefficients);
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
    let enclosure = enclosure
        .intersect_bounds(0.0, 180.0)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    if enclosure.work() > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    Ok(enclosure)
}

pub fn evaluate_half_angle_rational_derivative_interval_v1(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    let mut p_coefficients = try_schedule_vec_with_capacity_v1(numerator.coefficients.len())?;
    p_coefficients.extend_from_slice(&numerator.coefficients);
    let mut q_coefficients = try_schedule_vec_with_capacity_v1(denominator.coefficients.len())?;
    q_coefficients.extend_from_slice(&denominator.coefficients);
    let p = ExactBernsteinRangeV1 {
        coefficients: p_coefficients,
    };
    let q = ExactBernsteinRangeV1 {
        coefficients: q_coefficients,
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
            .checked_mul(power_coefficients.len())
            .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let mut power = try_schedule_vec_with_capacity_v1(power_coefficients.len())?;
    for value in power_coefficients {
        if value.denominator == 0
            || value.numerator.unsigned_abs().checked_ilog2().unwrap_or(0) + 1
                > max_coefficient_bits
            || value.denominator.checked_ilog2().unwrap_or(0) + 1 > max_coefficient_bits
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        power.push(BigRational::new(
            BigInt::from(value.numerator),
            BigInt::from(value.denominator),
        ));
    }
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
        || power
            .len()
            .checked_mul(power.len())
            .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    validate_exact_bits(&power, max_coefficient_bits)?;
    let degree = power.len() - 1;
    let coefficient_count = degree
        .checked_add(1)
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    let mut coefficients = try_schedule_vec_with_capacity_v1(coefficient_count)?;
    for i in 0..=degree {
        let mut value = BigRational::zero();
        for (k, coefficient) in power.iter().enumerate().take(i + 1) {
            let numerator =
                checked_binomial_v1(i, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let denominator =
                checked_binomial_v1(degree, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            value +=
                coefficient * BigRational::new(BigInt::from(numerator), BigInt::from(denominator));
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
    if power
        .len()
        .checked_mul(power.len())
        .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let a = &domain[0];
    let width = &domain[1] - a;
    let mut result = try_schedule_vec_with_capacity_v1(power.len())?;
    for _ in 0..power.len() {
        result.push(BigRational::zero());
    }
    for (degree, coefficient) in power.iter().enumerate() {
        for (k, output) in result.iter_mut().enumerate().take(degree + 1) {
            let weight =
                checked_binomial_v1(degree, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let a_exponent = i32::try_from(degree - k)
                .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let width_exponent =
                i32::try_from(k).map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
            *output +=
                coefficient * BigInt::from(weight) * a.pow(a_exponent) * width.pow(width_exponent);
        }
    }
    validate_exact_bits(&result, max_coefficient_bits)?;
    Ok(result)
}

fn checked_binomial_v1(n: usize, k: usize) -> Option<u128> {
    if k > n {
        return None;
    }
    let k = k.min(n - k);
    (0..k).try_fold(1_u128, |value, i| {
        value
            .checked_mul((n - i) as u128)?
            .checked_div((i + 1) as u128)
    })
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
    fn try_clone_with_fallible_outer_allocations_v1(
        &self,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let clone_coefficients =
            |source: &[BigRational]| -> Result<Vec<BigRational>, CycleSchedulePrepareErrorV1> {
                let mut cloned = try_schedule_vec_with_capacity_v1(source.len())?;
                cloned.extend_from_slice(source);
                Ok(cloned)
            };
        let clone_certificate = |source: &PoleFreeBernsteinCertificateV1| -> Result<
            PoleFreeBernsteinCertificateV1,
            CycleSchedulePrepareErrorV1,
        > {
            Ok(PoleFreeBernsteinCertificateV1 {
                degree: source.degree,
                positive: source.positive,
                coefficients: clone_coefficients(&source.coefficients)?,
            })
        };
        Ok(Self {
            edge: self.edge,
            u_domain: self.u_domain.clone(),
            numerator_power_coefficients: clone_coefficients(&self.numerator_power_coefficients)?,
            denominator_power_coefficients: clone_coefficients(
                &self.denominator_power_coefficients,
            )?,
            numerator_certificate: clone_certificate(&self.numerator_certificate)?,
            denominator_certificate: clone_certificate(&self.denominator_certificate)?,
            derivative_bound_degrees_bits: self.derivative_bound_degrees_bits,
        })
    }

    fn checked_nested_retained_bytes_v1(&self) -> Option<usize> {
        let mut total = 0usize;
        for endpoint in &self.u_domain {
            total = total.checked_add(checked_big_rational_heap_bytes_upper_bound_v1(endpoint)?)?;
        }
        for coefficients in [
            &self.numerator_power_coefficients,
            &self.denominator_power_coefficients,
        ] {
            total = total.checked_add(checked_big_rational_vec_allocation_bytes_v1(
                coefficients,
                coefficients.capacity(),
            )?)?;
        }
        total = total
            .checked_add(
                self.numerator_certificate
                    .checked_nested_retained_bytes_v1()?,
            )?
            .checked_add(
                self.denominator_certificate
                    .checked_nested_retained_bytes_v1()?,
            )?;
        Some(total)
    }

    fn power_profile_is_exact_constant_v1(
        numerator_power_coefficients: &[BigRational],
        denominator_power_coefficients: &[BigRational],
    ) -> bool {
        if numerator_power_coefficients.iter().all(Zero::is_zero) {
            return true;
        }
        if denominator_power_coefficients.iter().all(Zero::is_zero) {
            return true;
        }
        if numerator_power_coefficients.len() != denominator_power_coefficients.len() {
            return false;
        }
        let Some(pivot) = denominator_power_coefficients
            .iter()
            .position(|coefficient| !coefficient.is_zero())
        else {
            return false;
        };
        let numerator_pivot = &numerator_power_coefficients[pivot];
        let denominator_pivot = &denominator_power_coefficients[pivot];
        numerator_power_coefficients
            .iter()
            .zip(denominator_power_coefficients)
            .all(|(numerator, denominator)| {
                numerator * denominator_pivot == denominator * numerator_pivot
            })
    }

    fn is_exact_constant_profile_v1(&self) -> bool {
        Self::power_profile_is_exact_constant_v1(
            &self.numerator_power_coefficients,
            &self.denominator_power_coefficients,
        )
    }

    pub fn prepare(
        input: HalfAngleRationalEntryInputV1,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        for coefficient_count in [
            input.numerator_power_coefficients.len(),
            input.denominator_power_coefficients.len(),
        ] {
            if coefficient_count == 0
                || coefficient_count > limits.max_degree.saturating_add(1)
                || coefficient_count
                    .checked_mul(coefficient_count)
                    .is_none_or(|work| work > limits.max_work)
            {
                return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
            }
        }
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
        let mut numerator_power_coefficients =
            try_schedule_vec_with_capacity_v1(input.numerator_power_coefficients.len())?;
        for coefficient in input.numerator_power_coefficients {
            numerator_power_coefficients.push(to_exact(coefficient)?);
        }
        let mut denominator_power_coefficients =
            try_schedule_vec_with_capacity_v1(input.denominator_power_coefficients.len())?;
        for coefficient in input.denominator_power_coefficients {
            denominator_power_coefficients.push(to_exact(coefficient)?);
        }
        while numerator_power_coefficients.len() > 1
            && numerator_power_coefficients
                .last()
                .is_some_and(Zero::is_zero)
        {
            numerator_power_coefficients.pop();
        }
        while denominator_power_coefficients.len() > 1
            && denominator_power_coefficients
                .last()
                .is_some_and(Zero::is_zero)
        {
            denominator_power_coefficients.pop();
        }
        let exact_zero_denominator = denominator_power_coefficients.iter().all(Zero::is_zero);
        if !exact_zero_denominator {
            let domain_midpoint = (&u_domain[0] + &u_domain[1])
                * BigRational::new(BigInt::from(1_u8), BigInt::from(2_u8));
            let denominator_midpoint = evaluate_exact_power_horner(
                &denominator_power_coefficients,
                &domain_midpoint,
                limits.max_coefficient_bits,
                limits.max_work,
            )?;
            if denominator_midpoint.is_zero() {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            if denominator_midpoint.is_negative() {
                for coefficient in numerator_power_coefficients
                    .iter_mut()
                    .chain(&mut denominator_power_coefficients)
                {
                    *coefficient = -coefficient.clone();
                }
            }
        }
        let exact_zero_numerator = numerator_power_coefficients.iter().all(Zero::is_zero);
        let numerator_certificate = if exact_zero_numerator {
            let mut coefficients = try_schedule_vec_with_capacity_v1(1)?;
            coefficients.push(BigRational::zero());
            PoleFreeBernsteinCertificateV1 {
                degree: 0,
                positive: true,
                coefficients,
            }
        } else {
            prepare_exact_signed_bernstein_certificate(
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
            )?
        };
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
        if !denominator_certificate.positive {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        if !exact_zero_numerator && !numerator_certificate.positive {
            return Err(CycleSchedulePrepareErrorV1::AngleRange);
        }
        let has_projective_origin = numerator_certificate
            .coefficients
            .first()
            .zip(denominator_certificate.coefficients.first())
            .is_some_and(|(numerator, denominator)| numerator.is_zero() && denominator.is_zero())
            || numerator_certificate
                .coefficients
                .last()
                .zip(denominator_certificate.coefficients.last())
                .is_some_and(|(numerator, denominator)| {
                    numerator.is_zero() && denominator.is_zero()
                });
        if has_projective_origin {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let exact_constant_profile = Self::power_profile_is_exact_constant_v1(
            &numerator_power_coefficients,
            &denominator_power_coefficients,
        );
        let radians_bound = if exact_constant_profile {
            0.0
        } else {
            let derivative = evaluate_half_angle_rational_derivative_interval_v1(
                &numerator_certificate,
                &denominator_certificate,
                limits.max_coefficient_bits,
                limits.max_work,
            )?;
            derivative.lower().abs().max(derivative.upper().abs())
        };
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
            derivative_bound_degrees_bits: if exact_constant_profile {
                0.0_f64.to_bits()
            } else {
                derivative_bound_degrees.to_bits().saturating_add(1)
            },
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
        deterministic_half_angle_ratio_degrees_v1(numerator, denominator)
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
        if self.numerator_power_coefficients.iter().all(Zero::is_zero) {
            return OutwardIntervalV1::from_rounded(0.0)
                .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput);
        }
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
        if self.numerator_power_coefficients.iter().all(Zero::is_zero) {
            return OutwardIntervalV1::from_rounded(0.0)
                .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput);
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
        let certificate = |value: BigRational| {
            let positive = !value.is_negative();
            let mut coefficients = try_schedule_vec_with_capacity_v1(1)?;
            coefficients.push(value);
            Ok(PoleFreeBernsteinCertificateV1 {
                degree: 0,
                positive,
                coefficients,
            })
        };
        evaluate_half_angle_rational_degrees_interval_v1(
            &certificate(numerator)?,
            &certificate(denominator)?,
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
            max_hinges: 128,
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
pub enum CycleScheduleRestrictionStopV1 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleRestrictionErrorV1 {
    #[error(transparent)]
    Prepare(#[from] CycleSchedulePrepareErrorV1),
    #[error("cycle schedule restriction was cancelled")]
    Cancelled,
    #[error("cycle schedule restriction deadline elapsed")]
    DeadlineExceeded,
}

/// Frozen model identifier for the opaque common-linear-profile proof.
pub const EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1: &str =
    "exact_common_linear_cycle_profile_v1";

const EXACT_COMMON_LINEAR_MIN_EDGES_V1: usize = 2;
const EXACT_COMMON_LINEAR_MAX_EDGES_V1: usize = 3;
const EXACT_COMMON_LINEAR_EDGE_BYTES_V1: usize = 16;
const EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1: usize = 32;
// Logical cross-runtime streaming storage: one reusable eight-word
// state/final-digest slot, one 64-byte block, and one 64-bit length word.
// Input fields are borrowed and finalization reuses the state slot.
const EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1: usize = 104;

/// Explicit resource envelope for proving one exact common linear profile.
///
/// Storage limits count canonical payload bytes rather than target-dependent
/// Rust object layout. Retained storage consists of the canonical edge bytes
/// and three SHA-256 digests. Peak storage additionally includes the fixed
/// streaming SHA-256 scratch described by this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactCommonLinearCycleProfileLimitsV1 {
    pub max_edges: usize,
    pub max_work: usize,
    pub max_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

impl Default for ExactCommonLinearCycleProfileLimitsV1 {
    fn default() -> Self {
        Self {
            max_edges: EXACT_COMMON_LINEAR_MAX_EDGES_V1,
            max_work: 4_096,
            max_retained_bytes: EXACT_COMMON_LINEAR_MAX_EDGES_V1
                * EXACT_COMMON_LINEAR_EDGE_BYTES_V1
                + 3 * EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1,
            max_peak_bytes: EXACT_COMMON_LINEAR_MAX_EDGES_V1 * EXACT_COMMON_LINEAR_EDGE_BYTES_V1
                + 2 * EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1
                + EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ExactCommonLinearCycleProfileErrorV1 {
    #[error("the common linear profile input is malformed")]
    InvalidInput,
    #[error("the schedule does not use the ordinary representation")]
    UnsupportedRepresentation,
    #[error("the selected edges do not exactly cover one canonical schedule")]
    CarrierSetMismatch,
    #[error("the common linear profile exceeds its explicit resource limits")]
    ResourceLimit,
    #[error("the proof was not issued by this exact schedule")]
    IssuerMismatch,
}

/// Opaque evidence that every edge in a complete two- or three-edge schedule
/// carrier carries one bit-identical, non-constant degree-one ordinary profile.
///
/// This type deliberately has no persistence traits and exposes no raw
/// coefficient accessor. It is only a narrow recognition proof; closure,
/// collision clearance, and project mutation remain independently gated.
///
/// ```compile_fail
/// use ori_kinematics::ExactCommonLinearCycleProfileV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ExactCommonLinearCycleProfileV1>();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactCommonLinearCycleProfileV1 {
    canonical_edges: Vec<EdgeId>,
    issuer_schedule_fingerprint_v2: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    issuer_graph_binding_fingerprint_v1: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    proof_fingerprint_v1: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactCommonLinearCompositionBindingV1 {
    pub(crate) schedule_fingerprint_v2: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    pub(crate) graph_binding_fingerprint_v1: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    pub(crate) proof_fingerprint_v1: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    pub(crate) slope_sign: i8,
}

impl ExactCommonLinearCycleProfileV1 {
    #[must_use]
    pub fn edge_ids(&self) -> &[EdgeId] {
        &self.canonical_edges
    }

    /// Recomputes the complete bounded proof against an alleged issuer.
    ///
    /// The stored edge list is already canonical, but the same proof path is
    /// deliberately reused so representation, profile, schedule-fingerprint,
    /// graph-binding, and model-ID checks cannot drift.
    pub fn revalidate_issuer_schedule_v1(
        &self,
        issuer: &CanonicalCycleScheduleV1,
        limits: ExactCommonLinearCycleProfileLimitsV1,
    ) -> Result<(), ExactCommonLinearCycleProfileErrorV1> {
        let mut meter = ExactCommonLinearCycleProfileMeterV1::new(limits);
        let candidate = issuer
            .prove_exact_common_linear_profile_v1_with_meter(&self.canonical_edges, &mut meter)?;
        let comparison_work = exact_common_linear_retained_bytes_v1(self.canonical_edges.len())?
            .checked_add(1)
            .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        meter.charge_work(comparison_work)?;
        if &candidate == self {
            Ok(())
        } else {
            Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch)
        }
    }

    /// Revalidates the proof and returns only the narrow crate-private binding
    /// needed by downstream exact recognizers. Raw profile coefficients remain
    /// private.
    pub(crate) fn revalidate_composition_binding_v1(
        &self,
        issuer: &CanonicalCycleScheduleV1,
        limits: ExactCommonLinearCycleProfileLimitsV1,
    ) -> Result<ExactCommonLinearCompositionBindingV1, ExactCommonLinearCycleProfileErrorV1> {
        self.revalidate_issuer_schedule_v1(issuer, limits)?;
        if self.issuer_schedule_fingerprint_v2 != issuer.schedule_fingerprint_v2
            || self.issuer_graph_binding_fingerprint_v1 != issuer.binding_fingerprint
            || !issuer.half_angle_entries.is_empty()
            || issuer.entries.len() != self.canonical_edges.len()
        {
            return Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch);
        }
        let mut common_profile_bits = None;
        let mut slope_sign = None;
        for (edge, entry) in self.canonical_edges.iter().zip(&issuer.entries) {
            let [constant, linear] = entry.coefficients.as_slice() else {
                return Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch);
            };
            if edge != &entry.edge
                || !entry.initial.is_finite()
                || !constant.is_finite()
                || !linear.is_finite()
                || *linear == 0.0
            {
                return Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch);
            }
            let profile_bits = [
                entry.initial.to_bits(),
                constant.to_bits(),
                linear.to_bits(),
            ];
            if common_profile_bits.is_some_and(|expected| expected != profile_bits) {
                return Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch);
            }
            common_profile_bits = Some(profile_bits);
            let candidate_sign = if linear.is_sign_negative() { -1 } else { 1 };
            if slope_sign.is_some_and(|expected| expected != candidate_sign) {
                return Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch);
            }
            slope_sign = Some(candidate_sign);
        }
        Ok(ExactCommonLinearCompositionBindingV1 {
            schedule_fingerprint_v2: self.issuer_schedule_fingerprint_v2,
            graph_binding_fingerprint_v1: self.issuer_graph_binding_fingerprint_v1,
            proof_fingerprint_v1: self.proof_fingerprint_v1,
            slope_sign: slope_sign.ok_or(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch)?,
        })
    }

    #[must_use]
    pub const fn authorizes_closure(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct ExactCommonLinearCycleProfileMeterV1 {
    limits: ExactCommonLinearCycleProfileLimitsV1,
    work: usize,
    retained_bytes: usize,
    temporary_bytes: usize,
    peak_bytes: usize,
}

impl ExactCommonLinearCycleProfileMeterV1 {
    const fn new(limits: ExactCommonLinearCycleProfileLimitsV1) -> Self {
        Self {
            limits,
            work: 0,
            retained_bytes: 0,
            temporary_bytes: 0,
            peak_bytes: 0,
        }
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ExactCommonLinearCycleProfileErrorV1> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        if self.work > self.limits.max_work {
            return Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit);
        }
        Ok(())
    }

    fn retain(&mut self, amount: usize) -> Result<(), ExactCommonLinearCycleProfileErrorV1> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(amount)
            .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        if self.retained_bytes > self.limits.max_retained_bytes {
            return Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit);
        }
        self.update_peak()
    }

    fn begin_temporary(
        &mut self,
        amount: usize,
    ) -> Result<(), ExactCommonLinearCycleProfileErrorV1> {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_add(amount)
            .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        self.update_peak()
    }

    fn end_temporary(&mut self, amount: usize) {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_sub(amount)
            .expect("internal exact-profile temporary storage must balance");
    }

    fn update_peak(&mut self) -> Result<(), ExactCommonLinearCycleProfileErrorV1> {
        let current = self
            .retained_bytes
            .checked_add(self.temporary_bytes)
            .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        self.peak_bytes = self.peak_bytes.max(current);
        if self.peak_bytes > self.limits.max_peak_bytes {
            return Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit);
        }
        Ok(())
    }
}

fn exact_common_linear_retained_bytes_v1(
    edge_count: usize,
) -> Result<usize, ExactCommonLinearCycleProfileErrorV1> {
    edge_count
        .checked_mul(EXACT_COMMON_LINEAR_EDGE_BYTES_V1)
        .and_then(|bytes| {
            EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1
                .checked_mul(3)
                .and_then(|fingerprints| bytes.checked_add(fingerprints))
        })
        .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
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
            max_hinges: 128,
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
        .try_evaluate_v1(0.0)
        .map_err(multi_hinge_evaluation_error_v1)?;
    let upper = schedule
        .try_evaluate_v1(1.0)
        .map_err(multi_hinge_evaluation_error_v1)?;
    if lower != *initial || upper != *requested {
        return Err(MultiHingePathCandidateErrorV1::InvalidBinding);
    }
    let mut moving_hinges = try_multi_hinge_vec_with_capacity_v1(initial.as_slice().len())?;
    for (initial, requested) in initial.as_slice().iter().zip(requested.as_slice()) {
        if initial.edge() == requested.edge()
            && initial.angle_degrees().to_bits() != requested.angle_degrees().to_bits()
        {
            moving_hinges.push(initial.edge());
        }
    }
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
    if hinges.len() > limits.max_hinges || limits.max_candidates == 0 {
        return Err(MultiHingePathCandidateErrorV1::ResourceLimit);
    }
    let mut geometry_faces = try_multi_hinge_vec_with_capacity_v1(geometry.face_ids().len())?;
    geometry_faces.extend_from_slice(geometry.face_ids());
    geometry_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if geometry_faces != audit.faces()
        || !audit.faces().contains(&fixed_face)
        || hinges.len() != initial.as_slice().len()
        || hinges.len() != requested.as_slice().len()
    {
        return Err(MultiHingePathCandidateErrorV1::InvalidBinding);
    }
    let work = hinges
        .len()
        .checked_mul(2)
        .ok_or(MultiHingePathCandidateErrorV1::ResourceLimit)?;
    if work > limits.max_work {
        return Err(MultiHingePathCandidateErrorV1::ResourceLimit);
    }
    let mut expected = try_multi_hinge_vec_with_capacity_v1(hinges.len())?;
    expected.extend(hinges.iter().map(|hinge| hinge.edge()));
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
    let mut moving_hinges = try_multi_hinge_vec_with_capacity_v1(hinges.len())?;
    let mut entries = try_multi_hinge_vec_with_capacity_v1(hinges.len())?;
    for (start, end) in initial.as_slice().iter().zip(requested.as_slice()) {
        let start_value = start.angle_degrees();
        let end_value = end.angle_degrees();
        if start_value.to_bits() != end_value.to_bits() {
            moving_hinges.push(start.edge());
        }
        let midpoint = start_value + (end_value - start_value) * 0.5;
        let half_delta = (end_value - start_value) * 0.5;
        let mut chebyshev_coefficients = try_multi_hinge_vec_with_capacity_v1(2)?;
        chebyshev_coefficients.push(RationalCoefficientV1 {
            numerator: 0,
            denominator: 1,
        });
        chebyshev_coefficients.push(binary64_to_rational_coefficient_v1(half_delta)?);
        entries.push(CycleScheduleEntryInputV1 {
            edge: start.edge(),
            initial_angle_degrees_bits: midpoint.to_bits(),
            chebyshev_coefficients,
        });
    }
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
    .map_err(multi_hinge_schedule_prepare_error_v1)?;
    for (parameter, expected) in [(0.0, initial), (1.0, requested)] {
        let evaluated = schedule
            .try_evaluate_v1(parameter)
            .map_err(multi_hinge_evaluation_error_v1)?;
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

fn multi_hinge_schedule_prepare_error_v1(
    error: CycleSchedulePrepareErrorV1,
) -> MultiHingePathCandidateErrorV1 {
    if error == CycleSchedulePrepareErrorV1::ResourceLimit {
        MultiHingePathCandidateErrorV1::ResourceLimit
    } else {
        MultiHingePathCandidateErrorV1::CandidateRejected
    }
}

fn multi_hinge_evaluation_error_v1(error: KinematicsError) -> MultiHingePathCandidateErrorV1 {
    if error == KinematicsError::ResourceLimitExceeded {
        MultiHingePathCandidateErrorV1::ResourceLimit
    } else {
        MultiHingePathCandidateErrorV1::CandidateRejected
    }
}

/// Generates an exact rational half-angle mode for the bounded symmetric
/// Kawasaki degree-four class. The historic name is retained for API
/// compatibility; 120/120/60/60 is the ratio 1/2 member of the admitted
/// denominator-at-most-64 family. Geometry closure remains mandatory.
pub fn generate_kawasaki_120_120_60_60_path_candidate_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    limits: CycleScheduleLimitsV1,
) -> Result<GeneratedMultiHingePathCandidateV1, MultiHingePathCandidateErrorV1> {
    generate_kawasaki_path_candidate_at_scale_v1(geometry, audit, fixed_face, 1, limits)
        .map(|(candidate, _)| candidate)
}

/// Generates the same exact Kawasaki mode over a shorter dyadic endpoint.
/// `endpoint_denominator` must be one of 1, 2, 4, 8 or 16. The returned
/// candidate is mathematical closure evidence only; collision certification
/// remains mandatory before a caller exposes an Apply operation.
pub fn generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    endpoint_denominator: u64,
    limits: CycleScheduleLimitsV1,
) -> Result<GeneratedMultiHingePathCandidateV1, MultiHingePathCandidateErrorV1> {
    if !matches!(endpoint_denominator, 1 | 2 | 4 | 8 | 16) {
        return Err(MultiHingePathCandidateErrorV1::CandidateRejected);
    }
    let (generated, scaled_edges) = generate_kawasaki_path_candidate_at_scale_v1(
        geometry,
        audit,
        fixed_face,
        endpoint_denominator,
        limits,
    )?;
    validate_kawasaki_mountain_assignment_v1(geometry, scaled_edges)?;
    Ok(generated)
}

fn generate_kawasaki_path_candidate_at_scale_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    endpoint_denominator: u64,
    limits: CycleScheduleLimitsV1,
) -> Result<(GeneratedMultiHingePathCandidateV1, [EdgeId; 2]), MultiHingePathCandidateErrorV1> {
    if geometry.hinges().len() != 4 || limits.max_hinges < 4 {
        return Err(MultiHingePathCandidateErrorV1::InvalidBinding);
    }
    let source = geometry.hinges();
    let center = [source[0].start(), source[0].end()]
        .into_iter()
        .find(|point| {
            source
                .iter()
                .all(|hinge| hinge.start() == *point || hinge.end() == *point)
        })
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    let mut rays = try_multi_hinge_vec_with_capacity_v1(source.len())?;
    for hinge in source {
        let endpoint = if hinge.start() == center {
            hinge.end()
        } else {
            hinge.start()
        };
        let x = endpoint.x() - center.x();
        // Material geometry embeds the source sheet in the native X/Z
        // plane; Y is the out-of-sheet axis used by folded poses.
        let y = endpoint.z() - center.z();
        let length_squared = x.mul_add(x, y * y);
        if length_squared <= 0.0 {
            return Err(MultiHingePathCandidateErrorV1::CandidateRejected);
        }
        rays.push((hinge.edge(), x, y, length_squared));
    }
    rays.sort_by(|first, second| {
        let first_half = first.2 > 0.0 || (first.2 == 0.0 && first.1 >= 0.0);
        let second_half = second.2 > 0.0 || (second.2 == 0.0 && second.1 >= 0.0);
        first_half.cmp(&second_half).reverse().then_with(|| {
            let cross = first.1 * second.2 - first.2 * second.1;
            if cross > 0.0 {
                std::cmp::Ordering::Less
            } else if cross < 0.0 {
                std::cmp::Ordering::Greater
            } else {
                first.0.canonical_bytes().cmp(&second.0.canonical_bytes())
            }
        })
    });
    let sector_cosines: [f64; 4] = std::array::from_fn(|index| {
        let first = rays[index];
        let second = rays[(index + 1) % 4];
        (first.1 * second.1 + first.2 * second.2) / (first.3.sqrt() * second.3.sqrt())
    });
    let magnitude = sector_cosines[0].abs();
    let ratio = (1_u64..=MAX_BOUNDED_KAWASAKI_RATIO_DENOMINATOR_V1)
        .filter_map(|denominator| {
            let numerator = (magnitude * denominator as f64).round() as i64;
            (numerator > 0
                && numerator < denominator as i64
                && coprime_u64_v1(numerator as u64, denominator))
            .then_some((
                numerator,
                denominator,
                (magnitude - numerator as f64 / denominator as f64).abs(),
            ))
        })
        .min_by(|first, second| {
            first
                .2
                .total_cmp(&second.2)
                .then_with(|| first.1.cmp(&second.1))
        })
        .filter(|(numerator, denominator, error)| {
            *error <= 1.0e-9
                && numerator * 8 >= *denominator as i64
                && numerator * 8 <= *denominator as i64 * 7
        })
        .map(|(numerator, denominator, _)| (numerator, denominator))
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    let expected = [
        -(ratio.0 as f64 / ratio.1 as f64),
        -(ratio.0 as f64 / ratio.1 as f64),
        ratio.0 as f64 / ratio.1 as f64,
        ratio.0 as f64 / ratio.1 as f64,
    ];
    let rotation = (0..4)
        .find(|rotation| {
            (0..4).all(|index| {
                (sector_cosines[(index + rotation) % 4] - expected[index]).abs() <= 1.0e-9
            })
        })
        .ok_or(MultiHingePathCandidateErrorV1::CandidateRejected)?;
    rays.rotate_left(rotation);
    let unit_edges = [rays[0].0, rays[2].0];
    let scaled_edges = [rays[1].0, rays[3].0];
    let mut hinges = try_multi_hinge_vec_with_capacity_v1(rays.len())?;
    hinges.extend(rays.iter().map(|ray| ray.0));
    hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut entries = try_multi_hinge_vec_with_capacity_v1(hinges.len())?;
    for edge in &hinges {
        let mut numerator_power_coefficients = try_multi_hinge_vec_with_capacity_v1(2)?;
        numerator_power_coefficients.push(RationalCoefficientV1 {
            numerator: 0,
            denominator: 1,
        });
        numerator_power_coefficients.push(RationalCoefficientV1 {
            numerator: if unit_edges.contains(edge) {
                1
            } else {
                ratio.0
            },
            denominator: 1,
        });
        let mut denominator_power_coefficients = try_multi_hinge_vec_with_capacity_v1(1)?;
        denominator_power_coefficients.push(RationalCoefficientV1 {
            numerator: if unit_edges.contains(edge) {
                endpoint_denominator as i64
            } else {
                (ratio.1 * endpoint_denominator) as i64
            },
            denominator: 1,
        });
        entries.push(HalfAngleRationalEntryInputV1 {
            edge: *edge,
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
            numerator_power_coefficients,
            denominator_power_coefficients,
        });
    }
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        geometry, audit, fixed_face, entries, limits,
    )
    .map_err(multi_hinge_schedule_prepare_error_v1)?;
    let initial = schedule
        .try_evaluate_v1(0.0)
        .map_err(multi_hinge_evaluation_error_v1)?;
    let requested = schedule
        .try_evaluate_v1(1.0)
        .map_err(multi_hinge_evaluation_error_v1)?;
    admit_canonical_multi_hinge_path_candidate_v1(schedule, &initial, &requested)
        .map(|candidate| (candidate, scaled_edges))
}

/// Automatically admits the bounded degree-four Kawasaki spherical-linkage
/// mode from material geometry and its physical mountain/valley assignment.
/// Primitive rational sector-cosine profiles through denominator 64 are
/// admitted; this includes the exact 1/2, 3/5, 5/13 and 7/25 families. All
/// four hinges move and the unique mountain must belong to the scaled
/// opposite pair required by the closure theorem.
pub fn generate_bounded_degree_four_kawasaki_path_candidate_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    limits: CycleScheduleLimitsV1,
) -> Result<GeneratedMultiHingePathCandidateV1, MultiHingePathCandidateErrorV1> {
    let (generated, scaled_edges) =
        generate_kawasaki_path_candidate_at_scale_v1(geometry, audit, fixed_face, 1, limits)?;
    validate_kawasaki_mountain_assignment_v1(geometry, scaled_edges)?;
    Ok(generated)
}

fn validate_kawasaki_mountain_assignment_v1(
    geometry: &MaterialHingeGraphGeometry,
    scaled_edges: [EdgeId; 2],
) -> Result<(), MultiHingePathCandidateErrorV1> {
    let mut mountains = geometry
        .hinges()
        .iter()
        .filter(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain);
    let Some(mountain) = mountains.next() else {
        return Err(MultiHingePathCandidateErrorV1::CandidateRejected);
    };
    if mountains.next().is_some() || !scaled_edges.contains(&mountain.edge()) {
        return Err(MultiHingePathCandidateErrorV1::CandidateRejected);
    }
    Ok(())
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

impl Entry {
    fn try_clone_v1(&self) -> Result<Self, CycleSchedulePrepareErrorV1> {
        let mut coefficients = try_schedule_vec_with_capacity_v1(self.coefficients.len())?;
        coefficients.extend_from_slice(&self.coefficients);
        Ok(Self {
            edge: self.edge,
            initial: self.initial,
            coefficients,
            derivative_bound: self.derivative_bound,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalCycleScheduleV1 {
    binding_fingerprint: [u8; 32],
    schedule_fingerprint_v2: [u8; 32],
    fixed_face: FaceId,
    domain: [f64; 2],
    entries: Vec<Entry>,
    half_angle_entries: Vec<PreparedHalfAngleRationalEntryV1>,
}

impl CanonicalCycleScheduleV1 {
    /// Returns the schedule shell plus every owned allocation reachable from
    /// it. Actual vector capacities are charged so allocator spare capacity is
    /// not hidden at a retained-proof boundary.
    #[must_use]
    pub fn checked_deep_retained_bytes_v1(&self) -> Option<usize> {
        let mut total = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<Entry>().checked_mul(self.entries.capacity())?)?
            .checked_add(
                std::mem::size_of::<PreparedHalfAngleRationalEntryV1>()
                    .checked_mul(self.half_angle_entries.capacity())?,
            )?;
        for entry in &self.entries {
            total = total.checked_add(
                std::mem::size_of::<f64>().checked_mul(entry.coefficients.capacity())?,
            )?;
        }
        for entry in &self.half_angle_entries {
            total = total.checked_add(entry.checked_nested_retained_bytes_v1()?)?;
        }
        Some(total)
    }

    /// Rebinds the entries belonging to one canonical edge block to that
    /// block's independent geometry instance. No absent or foreign edge may
    /// enter the restricted schedule.
    pub fn restrict_to_edge_block_v1(
        &self,
        source_geometry: &MaterialHingeGraphGeometry,
        source_audit: &MaterialHingeGraphAudit,
        block_geometry: &MaterialHingeGraphGeometry,
        block_audit: &MaterialHingeGraphAudit,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        self.restrict_to_edge_block_with_fixed_face_v1(
            source_geometry,
            source_audit,
            block_geometry,
            block_audit,
            self.fixed_face,
        )
    }

    /// Rebinds an exact entry restriction to a block-local fixed face.
    ///
    /// The domain and admitted entry representations are retained bit-for-bit;
    /// only the geometry/audit carrier and fixed-face portion of the binding
    /// change. This permits different leaves of a block-cut tree to use their
    /// own articulation frame without manufacturing a new path.
    pub fn restrict_to_edge_block_with_fixed_face_v1(
        &self,
        source_geometry: &MaterialHingeGraphGeometry,
        source_audit: &MaterialHingeGraphAudit,
        block_geometry: &MaterialHingeGraphGeometry,
        block_audit: &MaterialHingeGraphAudit,
        block_fixed_face: FaceId,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        match self.restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
            source_geometry,
            source_audit,
            block_geometry,
            block_audit,
            block_fixed_face,
            || Ok(()),
        ) {
            Ok(schedule) => Ok(schedule),
            Err(CycleScheduleRestrictionErrorV1::Prepare(error)) => Err(error),
            Err(
                CycleScheduleRestrictionErrorV1::Cancelled
                | CycleScheduleRestrictionErrorV1::DeadlineExceeded,
            ) => unreachable!("an unbounded schedule restriction cannot stop"),
        }
    }

    /// Rebinds an exact block restriction while cooperatively checkpointing
    /// every bounded carrier and fingerprint-materialization loop.
    pub fn restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
        &self,
        source_geometry: &MaterialHingeGraphGeometry,
        source_audit: &MaterialHingeGraphAudit,
        block_geometry: &MaterialHingeGraphGeometry,
        block_audit: &MaterialHingeGraphAudit,
        block_fixed_face: FaceId,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
    ) -> Result<Self, CycleScheduleRestrictionErrorV1> {
        cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
        let source_binding = binding_fingerprint_with_checkpoint_v1(
            source_geometry,
            source_audit,
            self.fixed_face,
            &mut checkpoint,
        )?;
        if self.binding_fingerprint != source_binding
            || !source_geometry.face_ids().contains(&block_fixed_face)
            || !source_audit.faces().contains(&block_fixed_face)
            || !block_geometry.face_ids().contains(&block_fixed_face)
            || !block_audit.faces().contains(&block_fixed_face)
            || block_geometry.face_ids().is_empty()
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        for face in block_geometry.face_ids() {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            if !source_geometry.face_ids().contains(face) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
            }
        }
        for block_hinge in block_geometry.hinges() {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            let mut found = false;
            for source_hinge in source_geometry.hinges() {
                cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
                if source_hinge == block_hinge {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
            }
        }
        let source_carrier_count = self
            .entries
            .len()
            .checked_add(self.half_angle_entries.len())
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        if block_geometry.hinges().len() > source_carrier_count {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let mut block_edges = std::collections::HashSet::new();
        block_edges
            .try_reserve(block_geometry.hinges().len())
            .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
        for hinge in block_geometry.hinges() {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            if !block_edges.insert(hinge.edge()) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
            }
        }
        if block_edges.is_empty() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let mut entries = try_schedule_vec_with_capacity_v1(block_edges.len())?;
        for entry in &self.entries {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            if block_edges.contains(&entry.edge) {
                entries.push(entry.try_clone_v1()?);
            }
        }
        let mut half_angle_entries = try_schedule_vec_with_capacity_v1(block_edges.len())?;
        for entry in &self.half_angle_entries {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            if block_edges.contains(&entry.edge()) {
                half_angle_entries.push(entry.try_clone_with_fallible_outer_allocations_v1()?);
            }
        }
        if entries.len() + half_angle_entries.len() != block_edges.len() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        for entry in &entries {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            if !block_edges.contains(&entry.edge) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
            }
        }
        for entry in &half_angle_entries {
            cycle_schedule_restriction_checkpoint_v1(&mut checkpoint)?;
            if !block_edges.contains(&entry.edge()) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
            }
        }
        Ok(Self {
            binding_fingerprint: binding_fingerprint_with_checkpoint_v1(
                block_geometry,
                block_audit,
                block_fixed_face,
                &mut checkpoint,
            )?,
            schedule_fingerprint_v2: schedule_fingerprint_v2_with_checkpoint_v1(
                self.domain,
                &entries,
                &half_angle_entries,
                &mut checkpoint,
            )?,
            fixed_face: block_fixed_face,
            domain: self.domain,
            entries,
            half_angle_entries,
        })
    }

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
        let mut expected = try_schedule_vec_with_capacity_v1(geometry.hinges().len())?;
        expected.extend(geometry.hinges().iter().map(|hinge| hinge.edge()));
        expected.sort_unstable_by_key(EdgeId::canonical_bytes);
        if entries
            .iter()
            .map(|entry| entry.edge)
            .ne(expected.iter().copied())
        {
            return Err(CycleSchedulePrepareErrorV1::NonCanonical);
        }
        let width = domain[1] - domain[0];
        let mut prepared = try_schedule_vec_with_capacity_v1(entries.len())?;
        for input in entries {
            if input.chebyshev_coefficients.len() > limits.max_degree.saturating_add(1) {
                return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
            }
            let initial = f64::from_bits(input.initial_angle_degrees_bits);
            if !initial.is_finite() || !(0.0..=180.0).contains(&initial) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            let derivative_is_mathematically_zero = input
                .chebyshev_coefficients
                .iter()
                .skip(1)
                .all(|coefficient| coefficient.numerator == 0);
            let mut coefficients =
                try_schedule_vec_with_capacity_v1(input.chebyshev_coefficients.len())?;
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
            let derivative_bound = if derivative_is_mathematically_zero {
                0.0
            } else {
                let computed = coefficients
                    .iter()
                    .enumerate()
                    .map(|(degree, value)| 2.0 * (degree * degree) as f64 * value.abs() / width)
                    .sum::<f64>();
                // Exact rational coefficients decide whether the polynomial is
                // constant. An underflowed non-constant bound must never become
                // stationary authority; nonzero and non-finite values are kept.
                if computed == 0.0 {
                    f64::INFINITY
                } else {
                    computed
                }
            };
            prepared.push(Entry {
                edge: input.edge,
                initial,
                coefficients,
                derivative_bound,
            });
        }
        let schedule_fingerprint_v2 = schedule_fingerprint_v2(domain, &prepared, &[]);
        Ok(Self {
            binding_fingerprint: binding_fingerprint(geometry, audit, fixed_face),
            schedule_fingerprint_v2,
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
        let mut expected = try_schedule_vec_with_capacity_v1(geometry.hinges().len())?;
        expected.extend(geometry.hinges().iter().map(|hinge| hinge.edge()));
        expected.sort_unstable_by_key(EdgeId::canonical_bytes);
        if entries
            .iter()
            .map(|entry| entry.edge)
            .ne(expected.iter().copied())
        {
            return Err(CycleSchedulePrepareErrorV1::NonCanonical);
        }
        let mut prepared = try_schedule_vec_with_capacity_v1(entries.len())?;
        for entry in entries {
            prepared.push(PreparedHalfAngleRationalEntryV1::prepare(entry, limits)?);
        }
        let domain = [0.0, 1.0];
        let schedule_fingerprint_v2 = schedule_fingerprint_v2(domain, &[], &prepared);
        Ok(Self {
            binding_fingerprint: binding_fingerprint(geometry, audit, fixed_face),
            schedule_fingerprint_v2,
            fixed_face,
            domain,
            entries: Vec::new(),
            half_angle_entries: prepared,
        })
    }

    /// Proves one exact common linear ordinary profile over the complete
    /// schedule carrier set.
    ///
    /// Caller edge order is intentionally not semantic: this method copies
    /// at most three IDs, applies a fixed-comparison canonical sort, and
    /// rejects duplicates. The resulting proof always stores canonical IDs.
    pub fn prove_exact_common_linear_profile_v1(
        &self,
        edges: &[EdgeId],
        limits: ExactCommonLinearCycleProfileLimitsV1,
    ) -> Result<ExactCommonLinearCycleProfileV1, ExactCommonLinearCycleProfileErrorV1> {
        let mut meter = ExactCommonLinearCycleProfileMeterV1::new(limits);
        self.prove_exact_common_linear_profile_v1_with_meter(edges, &mut meter)
    }

    fn prove_exact_common_linear_profile_v1_with_meter(
        &self,
        edges: &[EdgeId],
        meter: &mut ExactCommonLinearCycleProfileMeterV1,
    ) -> Result<ExactCommonLinearCycleProfileV1, ExactCommonLinearCycleProfileErrorV1> {
        let edge_count = edges.len();
        if !(EXACT_COMMON_LINEAR_MIN_EDGES_V1..=EXACT_COMMON_LINEAR_MAX_EDGES_V1)
            .contains(&edge_count)
        {
            return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
        }
        if edge_count > meter.limits.max_edges {
            return Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit);
        }

        let edge_bytes = edge_count
            .checked_mul(EXACT_COMMON_LINEAR_EDGE_BYTES_V1)
            .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        meter.retain(edge_bytes)?;
        meter.charge_work(edge_count)?;
        let mut canonical_edges = Vec::new();
        canonical_edges
            .try_reserve_exact(edge_count)
            .map_err(|_| ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
        canonical_edges.extend_from_slice(edges);
        // A fixed bubble network makes comparison work input-order invariant:
        // one comparison for two edges and three comparisons for three.
        for unsorted in (1..edge_count).rev() {
            for left in 0..unsorted {
                meter.charge_work(1)?;
                if canonical_edges[left].canonical_bytes()
                    > canonical_edges[left + 1].canonical_bytes()
                {
                    canonical_edges.swap(left, left + 1);
                }
            }
        }
        for pair in canonical_edges.windows(2) {
            meter.charge_work(1)?;
            if pair[0] == pair[1] {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            }
        }

        meter.charge_work(1)?;
        if !self.half_angle_entries.is_empty() {
            return Err(ExactCommonLinearCycleProfileErrorV1::UnsupportedRepresentation);
        }
        meter.charge_work(1)?;
        if self.entries.len() != edge_count {
            return Err(ExactCommonLinearCycleProfileErrorV1::CarrierSetMismatch);
        }

        for endpoint in self.domain {
            meter.charge_work(1)?;
            if !exact_common_linear_binary64_is_canonical_v1(endpoint) {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            }
        }
        meter.charge_work(2)?;
        let width = self.domain[1] - self.domain[0];
        if self.domain[0] >= self.domain[1]
            || !exact_common_linear_binary64_is_canonical_v1(width)
            || width <= 0.0
        {
            return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
        }

        let mut common_profile_bits = None;
        for (index, entry) in self.entries.iter().enumerate() {
            // Edge/order, initial, coefficient count, both coefficients,
            // degree/non-constant, excursion, and all three common-profile
            // bit comparisons are charged uniformly so reject position
            // cannot weaken the declared bound.
            meter.charge_work(10)?;
            if entry.edge != canonical_edges[index] {
                return Err(ExactCommonLinearCycleProfileErrorV1::CarrierSetMismatch);
            }
            if !exact_common_linear_binary64_is_canonical_v1(entry.initial)
                || !(0.0..=180.0).contains(&entry.initial)
            {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            }
            let [constant, linear] = entry.coefficients.as_slice() else {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            };
            if !exact_common_linear_binary64_is_canonical_v1(*constant)
                || !exact_common_linear_binary64_is_canonical_v1(*linear)
                || *linear == 0.0
            {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            }
            let excursion = constant.abs() + linear.abs();
            if !excursion.is_finite()
                || entry.initial - excursion < 0.0
                || entry.initial + excursion > 180.0
            {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            }
            let profile_bits = [
                entry.initial.to_bits(),
                constant.to_bits(),
                linear.to_bits(),
            ];
            if common_profile_bits.is_some_and(|expected| expected != profile_bits) {
                return Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput);
            }
            common_profile_bits = Some(profile_bits);
        }
        let common_profile_bits =
            common_profile_bits.ok_or(ExactCommonLinearCycleProfileErrorV1::InvalidInput)?;

        meter.charge_work(exact_common_linear_schedule_fingerprint_work_v1(
            edge_count,
        )?)?;
        meter.begin_temporary(EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1)?;
        let recomputed_schedule_fingerprint =
            schedule_fingerprint_v2(self.domain, &self.entries, &[]);
        meter.end_temporary(EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1);
        meter.retain(EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1)?;
        meter.charge_work(EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1)?;
        if recomputed_schedule_fingerprint != self.schedule_fingerprint_v2 {
            return Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch);
        }

        meter.retain(EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1)?;
        meter.charge_work(EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1)?;
        let issuer_graph_binding_fingerprint_v1 = self.binding_fingerprint;
        let proof_fingerprint_v1 = exact_common_linear_proof_fingerprint_v1(
            &canonical_edges,
            self.domain.map(f64::to_bits),
            common_profile_bits,
            recomputed_schedule_fingerprint,
            issuer_graph_binding_fingerprint_v1,
            meter,
        )?;
        meter.retain(EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1)?;

        Ok(ExactCommonLinearCycleProfileV1 {
            canonical_edges,
            issuer_schedule_fingerprint_v2: recomputed_schedule_fingerprint,
            issuer_graph_binding_fingerprint_v1,
            proof_fingerprint_v1,
        })
    }

    pub fn try_evaluate_v1(&self, parameter: f64) -> Result<CanonicalHingeAngles, KinematicsError> {
        if !self.half_angle_entries.is_empty() {
            let mut angles = Vec::new();
            angles
                .try_reserve_exact(self.half_angle_entries.len())
                .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
            for entry in &self.half_angle_entries {
                let angle = entry
                    .evaluate_degrees(parameter)
                    .ok_or(KinematicsError::UnrepresentableGeometry)?;
                angles.push(HingeAngle::new(entry.edge(), angle)?);
            }
            return CanonicalHingeAngles::new(angles);
        }
        if !parameter.is_finite() || parameter < self.domain[0] || parameter > self.domain[1] {
            return Err(KinematicsError::UnrepresentableGeometry);
        }
        let x =
            (2.0 * parameter - self.domain[0] - self.domain[1]) / (self.domain[1] - self.domain[0]);
        let mut angles = Vec::new();
        angles
            .try_reserve_exact(self.entries.len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        for entry in &self.entries {
            let mut b1 = 0.0;
            let mut b2 = 0.0;
            for coefficient in entry.coefficients.iter().rev() {
                let b0 = 2.0 * x * b1 - b2 + coefficient;
                b2 = b1;
                b1 = b0;
            }
            angles.push(HingeAngle::new(entry.edge, entry.initial + b1 - x * b2)?);
        }
        CanonicalHingeAngles::new(angles)
    }

    pub fn evaluate(&self, parameter: f64) -> Option<CanonicalHingeAngles> {
        self.try_evaluate_v1(parameter).ok()
    }

    pub fn evaluate_angle_box(
        &self,
        max_work: usize,
    ) -> Result<Vec<(EdgeId, OutwardIntervalV1)>, CycleSchedulePrepareErrorV1> {
        if self.half_angle_entries.is_empty() {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let mut angles = try_schedule_vec_with_capacity_v1(self.half_angle_entries.len())?;
        for entry in &self.half_angle_entries {
            angles.push((entry.edge(), entry.angle_enclosure(max_work)?));
        }
        Ok(angles)
    }

    /// Evaluates one exact dyadic leaf. Adjacent leaf indices share the exact
    /// rational endpoint used during affine reparameterization.
    pub fn evaluate_angle_box_dyadic(
        &self,
        depth: u32,
        index: u64,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Vec<(EdgeId, OutwardIntervalV1)>, CycleSchedulePrepareErrorV1> {
        if depth >= 64 {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let leaf_count = 1u64 << depth;
        if index >= leaf_count
            || self.half_angle_entries.len() > limits.max_hinges
            || self.entries.len() > limits.max_hinges
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        if self.half_angle_entries.is_empty() {
            if self.entries.is_empty() {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            let scale = leaf_count as f64;
            let x = OutwardIntervalV1::new(
                -1.0 + 2.0 * index as f64 / scale,
                -1.0 + 2.0 * (index + 1) as f64 / scale,
            )
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
            let mut angles = try_schedule_vec_with_capacity_v1(self.entries.len())?;
            for entry in &self.entries {
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
                if angle.work() > limits.max_work || angle.lower() < 0.0 || angle.upper() > 180.0 {
                    return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
                }
                angles.push((entry.edge, angle));
            }
            return Ok(angles);
        }
        let mut angles = try_schedule_vec_with_capacity_v1(self.half_angle_entries.len())?;
        for entry in &self.half_angle_entries {
            angles.push((
                entry.edge(),
                entry.angle_enclosure_dyadic(
                    depth,
                    index,
                    limits.max_coefficient_bits,
                    limits.max_degree,
                    limits.max_work,
                )?,
            ));
        }
        Ok(angles)
    }

    /// Evaluates the exact rational schedule endpoint without replacing it by
    /// a nearby dyadic leaf.
    pub fn evaluate_endpoint_angle_box(
        &self,
        upper: bool,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Vec<(EdgeId, OutwardIntervalV1)>, CycleSchedulePrepareErrorV1> {
        if self.half_angle_entries.is_empty() || self.half_angle_entries.len() > limits.max_hinges {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let mut angles = try_schedule_vec_with_capacity_v1(self.half_angle_entries.len())?;
        for entry in &self.half_angle_entries {
            angles.push((
                entry.edge(),
                entry.endpoint_angle_enclosure(
                    upper,
                    limits.max_coefficient_bits,
                    limits.max_work,
                )?,
            ));
        }
        Ok(angles)
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

    /// Returns true only when the prepared native representation proves that
    /// the selected hinge profile is exactly constant over the whole domain.
    #[must_use]
    pub fn is_exact_constant_profile_v1(&self, edge: EdgeId) -> bool {
        if !self.half_angle_entries.is_empty() {
            return self
                .half_angle_entries
                .iter()
                .any(|entry| entry.edge() == edge && entry.is_exact_constant_profile_v1());
        }
        self.entries
            .iter()
            .any(|entry| entry.edge == edge && entry.coefficients.iter().all(|value| *value == 0.0))
    }

    /// Returns the hinges carrying one bit-identical non-constant projective
    /// profile when every other hinge is an exact constant profile.
    /// This is intentionally narrower than comparing sampled angles.
    #[must_use]
    pub fn collective_half_angle_profile_edges_v1(&self) -> Option<Vec<EdgeId>> {
        self.collective_profile_edges_v1()
    }

    /// Returns the exact carrier set of one collective profile for either
    /// admitted schedule representation. Constant hinges are excluded.
    #[must_use]
    pub fn collective_profile_edges_v1(&self) -> Option<Vec<EdgeId>> {
        if self.half_angle_entries.is_empty() {
            let mut moving = try_schedule_vec_with_capacity_v1(self.entries.len()).ok()?;
            let mut profile: Option<&Entry> = None;
            for entry in &self.entries {
                let constant = entry.coefficients.iter().all(|value| *value == 0.0);
                if constant {
                    continue;
                }
                if let Some(expected) = profile {
                    if entry.initial.to_bits() != expected.initial.to_bits()
                        || entry.coefficients != expected.coefficients
                    {
                        return None;
                    }
                } else {
                    profile = Some(entry);
                }
                moving.push(entry.edge);
            }
            return (!moving.is_empty()).then_some(moving);
        }
        let mut moving = try_schedule_vec_with_capacity_v1(self.half_angle_entries.len()).ok()?;
        let mut profile: Option<&PreparedHalfAngleRationalEntryV1> = None;
        for entry in &self.half_angle_entries {
            if entry.is_exact_constant_profile_v1() {
                continue;
            }
            if let Some(expected) = profile {
                if entry.u_domain != expected.u_domain
                    || entry.numerator_power_coefficients != expected.numerator_power_coefficients
                    || entry.denominator_power_coefficients
                        != expected.denominator_power_coefficients
                {
                    return None;
                }
            } else {
                profile = Some(entry);
            }
            moving.push(entry.edge());
        }
        (!moving.is_empty()).then_some(moving)
    }

    /// Recognizes the exact rational degree-4 mode used by the physical
    /// 120/120/60/60 Kawasaki vertex: two hinges carry tan(rho/2)=u and the
    /// opposite pair carries tan(rho/2)=u/2 over the canonical unit domain.
    #[must_use]
    pub fn kawasaki_120_120_60_60_half_angle_pairs_v1(&self) -> Option<(Vec<EdgeId>, Vec<EdgeId>)> {
        self.symmetric_kawasaki_half_angle_pairs_v1(1, 2)
    }

    /// Recognizes a bounded exact rational symmetric degree-4 mode. Exactly
    /// two opposite hinges use `tan(rho/2)=u` and the other pair uses
    /// `tan(rho/2)=numerator*u/denominator`. Sign reversal is rejected by the
    /// physical schedule boundary before it can reach this proof.
    #[must_use]
    pub fn symmetric_kawasaki_half_angle_pairs_v1(
        &self,
        numerator: i64,
        denominator: i64,
    ) -> Option<(Vec<EdgeId>, Vec<EdgeId>)> {
        if self.half_angle_entries.len() != 4 {
            return None;
        }
        let rational = |numerator: i64, denominator: i64| {
            BigRational::new(numerator.into(), denominator.into())
        };
        let unit_domain = [rational(0, 1), rational(1, 1)];
        if numerator <= 0 || denominator <= numerator {
            return None;
        }
        let unit_numerator = [rational(0, 1), rational(1, 1)];
        let scaled_numerator = [rational(0, 1), rational(numerator, 1)];
        let mut unit = try_schedule_vec_with_capacity_v1(2).ok()?;
        let mut scaled = try_schedule_vec_with_capacity_v1(2).ok()?;
        for entry in &self.half_angle_entries {
            if entry.u_domain != unit_domain {
                return None;
            }
            if entry.numerator_power_coefficients == unit_numerator
                && entry.denominator_power_coefficients == [rational(1, 1)]
            {
                unit.push(entry.edge);
            } else if entry.numerator_power_coefficients == scaled_numerator
                && entry.denominator_power_coefficients == [rational(denominator, 1)]
            {
                scaled.push(entry.edge);
            } else {
                return None;
            }
        }
        (unit.len() == 2 && scaled.len() == 2).then_some((unit, scaled))
    }

    /// Extracts the only bounded rational symmetric degree-4 profile admitted
    /// by the generic closure theorem. The reduced ratio is intentionally
    /// capped and kept away from zero and one so near-degenerate sectors and
    /// oversized exact coefficients fail closed.
    #[must_use]
    pub fn bounded_symmetric_kawasaki_profile_v1(
        &self,
    ) -> Option<(Vec<EdgeId>, Vec<EdgeId>, i64, u64)> {
        let mut edges = try_schedule_vec_with_capacity_v1(self.half_angle_entries.len()).ok()?;
        edges.extend(self.half_angle_entries.iter().map(|entry| entry.edge));
        self.bounded_symmetric_kawasaki_profile_for_edges_v1(&edges)
    }

    #[must_use]
    pub fn bounded_symmetric_kawasaki_profile_for_edges_v1(
        &self,
        edges: &[EdgeId],
    ) -> Option<(Vec<EdgeId>, Vec<EdgeId>, i64, u64)> {
        const MAX_TERM: i64 = 64;
        if edges.len() != 4 {
            return None;
        }
        if edges
            .iter()
            .enumerate()
            .any(|(index, edge)| edges[index + 1..].contains(edge))
        {
            return None;
        }
        let rational = |numerator: i64, denominator: i64| {
            BigRational::new(numerator.into(), denominator.into())
        };
        let domain = [rational(0, 1), rational(1, 1)];
        let mut effective_slopes = try_schedule_vec_with_capacity_v1(4).ok()?;
        for entry in self
            .half_angle_entries
            .iter()
            .filter(|entry| edges.contains(&entry.edge))
        {
            if entry.u_domain != domain || entry.denominator_power_coefficients.len() != 1 {
                return None;
            }
            let [zero, slope] = entry.numerator_power_coefficients.as_slice() else {
                return None;
            };
            if !zero.is_zero() || slope <= &BigRational::zero() {
                return None;
            }
            if entry.denominator_power_coefficients[0] <= BigRational::zero() {
                return None;
            }
            let candidate = slope / &entry.denominator_power_coefficients[0];
            if candidate <= BigRational::zero() || candidate > BigRational::from_integer(1.into()) {
                return None;
            }
            effective_slopes.push((entry.edge, candidate));
        }
        let unit_index = effective_slopes
            .iter()
            .enumerate()
            .max_by(|(_, (_, left)), (_, (_, right))| left.cmp(right))
            .map(|(index, _)| index)?;
        let (unit_edge, unit_slope) = effective_slopes.remove(unit_index);
        let mut unit = try_schedule_vec_with_capacity_v1(2).ok()?;
        let mut scaled = try_schedule_vec_with_capacity_v1(2).ok()?;
        unit.push(unit_edge);
        let mut ratio = None;
        for (edge, slope) in effective_slopes {
            if slope == unit_slope {
                unit.push(edge);
            } else {
                let candidate = slope / &unit_slope;
                if ratio.as_ref().is_some_and(|current| current != &candidate) {
                    return None;
                }
                ratio = Some(candidate);
                scaled.push(edge);
            }
        }
        unit.sort_unstable_by_key(EdgeId::canonical_bytes);
        scaled.sort_unstable_by_key(EdgeId::canonical_bytes);
        let ratio = ratio?;
        let numerator = ratio.numer().to_i64()?;
        let denominator = ratio.denom().to_i64()?;
        if unit.len() + scaled.len() != 4
            || unit.len() != 2
            || scaled.len() != 2
            || numerator <= 0
            || denominator <= 0
            || numerator > MAX_TERM
            || denominator > MAX_TERM
            || numerator * 8 < denominator
            || numerator * 8 > denominator * 7
        {
            return None;
        }
        Some((unit, scaled, numerator, u64::try_from(denominator).ok()?))
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

    /// Opaque V2 authentication value used to prevent exchanging certificates
    /// between different schedules bound to the same material graph.
    ///
    /// The SHA-256 preimage is domain-separated and structurally framed. It
    /// includes the representation kind, the outer binary64 domain, the entry
    /// count, an entry tag and canonical edge ID per entry, and every
    /// variable-length coefficient count. Exact rationals use a canonical
    /// sign plus length-prefixed numerator and denominator magnitudes. All
    /// integers and binary64 bit patterns are big-endian.
    #[doc(hidden)]
    #[must_use]
    pub const fn certificate_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.schedule_fingerprint_v2
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn graph_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding_fingerprint
    }
}

fn exact_common_linear_binary64_is_canonical_v1(value: f64) -> bool {
    value.is_finite() && value.to_bits() != (-0.0_f64).to_bits()
}

fn exact_common_linear_schedule_fingerprint_work_v1(
    edge_count: usize,
) -> Result<usize, ExactCommonLinearCycleProfileErrorV1> {
    const DOMAIN_SEPARATOR: &[u8] = b"ORIGAMI2_CANONICAL_CYCLE_SCHEDULE_CERTIFICATE_BINDING_V2";
    const LENGTH_BYTES: usize = 8;
    const ORDINARY_ENTRY_BYTES: usize = 1 + 16 + 8 + 8 + 2 * 8;

    [
        DOMAIN_SEPARATOR.len(),
        LENGTH_BYTES,
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.len(),
        LENGTH_BYTES,
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.len(),
        1,
        2 * 8,
        LENGTH_BYTES,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .and_then(|fixed| {
        edge_count
            .checked_mul(ORDINARY_ENTRY_BYTES)
            .and_then(|entries| fixed.checked_add(entries))
    })
    .ok_or(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
}

fn exact_common_linear_hash_frame_v1(
    hash: &mut Sha256,
    value: &[u8],
    meter: &mut ExactCommonLinearCycleProfileMeterV1,
) -> Result<(), ExactCommonLinearCycleProfileErrorV1> {
    let length = u64::try_from(value.len())
        .map_err(|_| ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?;
    meter.charge_work(8)?;
    meter.charge_work(value.len())?;
    hash.update(length.to_be_bytes());
    hash.update(value);
    Ok(())
}

fn exact_common_linear_proof_fingerprint_v1(
    canonical_edges: &[EdgeId],
    domain_bits: [u64; 2],
    profile_bits: [u64; 3],
    schedule_fingerprint_v2: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    graph_binding_fingerprint_v1: [u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1],
    meter: &mut ExactCommonLinearCycleProfileMeterV1,
) -> Result<[u8; EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1], ExactCommonLinearCycleProfileErrorV1> {
    const DOMAIN_SEPARATOR: &[u8] = b"ORIGAMI2_EXACT_COMMON_LINEAR_CYCLE_PROFILE_PROOF_V1";

    meter.begin_temporary(EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1)?;
    let result = (|| {
        let mut hash = Sha256::new();
        exact_common_linear_hash_frame_v1(&mut hash, DOMAIN_SEPARATOR, meter)?;
        exact_common_linear_hash_frame_v1(
            &mut hash,
            EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        exact_common_linear_hash_frame_v1(
            &mut hash,
            CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes(),
            meter,
        )?;
        exact_common_linear_hash_frame_v1(
            &mut hash,
            DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        exact_common_linear_hash_frame_v1(&mut hash, &schedule_fingerprint_v2, meter)?;
        exact_common_linear_hash_frame_v1(&mut hash, &graph_binding_fingerprint_v1, meter)?;
        exact_common_linear_hash_frame_v1(
            &mut hash,
            &u64::try_from(canonical_edges.len())
                .map_err(|_| ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for edge in canonical_edges {
            exact_common_linear_hash_frame_v1(&mut hash, &edge.canonical_bytes(), meter)?;
        }
        for bits in domain_bits {
            exact_common_linear_hash_frame_v1(&mut hash, &bits.to_be_bytes(), meter)?;
        }
        exact_common_linear_hash_frame_v1(&mut hash, &profile_bits[0].to_be_bytes(), meter)?;
        exact_common_linear_hash_frame_v1(
            &mut hash,
            &u64::try_from(profile_bits.len() - 1)
                .map_err(|_| ExactCommonLinearCycleProfileErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for bits in &profile_bits[1..] {
            exact_common_linear_hash_frame_v1(&mut hash, &bits.to_be_bytes(), meter)?;
        }
        Ok(hash.finalize().into())
    })();
    meter.end_temporary(EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1);
    result
}

/// Computes the V2 schedule certificate preimage as:
///
/// - the exact domain-separation bytes;
/// - the canonical-schedule model ID and deterministic-transcendental model
///   ID, in that order, each preceded by its big-endian `u64` byte length;
/// - one representation-kind byte (`0` ordinary, `1` half-angle rational);
/// - two outer-domain binary64 bit patterns;
/// - one big-endian `u64` entry count;
/// - for every entry, one kind byte and the 16 canonical edge-ID bytes;
/// - ordinary initial/coefficients as binary64 bit patterns, preceded by a
///   big-endian `u64` coefficient count;
/// - half-angle `u_domain`, followed by independent big-endian `u64` P and Q
///   counts and their canonically framed exact rationals.
///
/// This grammar deliberately has no V1 compatibility branch: every authority
/// producer and consumer in the process uses the same V2 digest.
fn schedule_fingerprint_v2(
    domain: [f64; 2],
    entries: &[Entry],
    half_angle_entries: &[PreparedHalfAngleRationalEntryV1],
) -> [u8; 32] {
    schedule_fingerprint_v2_with_model_ids(
        domain,
        entries,
        half_angle_entries,
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes(),
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
    )
}

fn schedule_fingerprint_v2_with_checkpoint_v1(
    domain: [f64; 2],
    entries: &[Entry],
    half_angle_entries: &[PreparedHalfAngleRationalEntryV1],
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<[u8; 32], CycleScheduleRestrictionErrorV1> {
    const ORDINARY_KIND_TAG: u8 = 0;
    const HALF_ANGLE_RATIONAL_KIND_TAG: u8 = 1;

    cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
    debug_assert!(entries.is_empty() || half_angle_entries.is_empty());
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_CANONICAL_CYCLE_SCHEDULE_CERTIFICATE_BINDING_V2");
    update_length_prefixed_bytes_v2(&mut hash, CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes());
    update_length_prefixed_bytes_v2(
        &mut hash,
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
    );
    let half_angle = !half_angle_entries.is_empty();
    hash.update([if half_angle {
        HALF_ANGLE_RATIONAL_KIND_TAG
    } else {
        ORDINARY_KIND_TAG
    }]);
    for endpoint in domain {
        cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
        hash.update(endpoint.to_bits().to_be_bytes());
    }
    hash.update(
        u64::try_from(entries.len() + half_angle_entries.len())
            .expect("an in-memory schedule entry count must fit u64")
            .to_be_bytes(),
    );
    for entry in entries {
        cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
        hash.update([ORDINARY_KIND_TAG]);
        hash.update(entry.edge.canonical_bytes());
        hash.update(entry.initial.to_bits().to_be_bytes());
        hash.update(
            u64::try_from(entry.coefficients.len())
                .expect("an in-memory coefficient count must fit u64")
                .to_be_bytes(),
        );
        for coefficient in &entry.coefficients {
            cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
            hash.update(coefficient.to_bits().to_be_bytes());
        }
    }
    for entry in half_angle_entries {
        cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
        hash.update([HALF_ANGLE_RATIONAL_KIND_TAG]);
        hash.update(entry.edge.canonical_bytes());
        for value in &entry.u_domain {
            update_canonical_big_rational_with_checkpoint_v1(&mut hash, value, checkpoint)?;
        }
        hash.update(
            u64::try_from(entry.numerator_power_coefficients.len())
                .expect("an in-memory numerator coefficient count must fit u64")
                .to_be_bytes(),
        );
        for value in &entry.numerator_power_coefficients {
            update_canonical_big_rational_with_checkpoint_v1(&mut hash, value, checkpoint)?;
        }
        hash.update(
            u64::try_from(entry.denominator_power_coefficients.len())
                .expect("an in-memory denominator coefficient count must fit u64")
                .to_be_bytes(),
        );
        for value in &entry.denominator_power_coefficients {
            update_canonical_big_rational_with_checkpoint_v1(&mut hash, value, checkpoint)?;
        }
    }
    cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
    Ok(hash.finalize().into())
}

fn schedule_fingerprint_v2_with_model_ids(
    domain: [f64; 2],
    entries: &[Entry],
    half_angle_entries: &[PreparedHalfAngleRationalEntryV1],
    canonical_schedule_model_id: &[u8],
    deterministic_transcendental_model_id: &[u8],
) -> [u8; 32] {
    const ORDINARY_KIND_TAG: u8 = 0;
    const HALF_ANGLE_RATIONAL_KIND_TAG: u8 = 1;

    debug_assert!(entries.is_empty() || half_angle_entries.is_empty());
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_CANONICAL_CYCLE_SCHEDULE_CERTIFICATE_BINDING_V2");
    update_length_prefixed_bytes_v2(&mut hash, canonical_schedule_model_id);
    update_length_prefixed_bytes_v2(&mut hash, deterministic_transcendental_model_id);
    let half_angle = !half_angle_entries.is_empty();
    hash.update([if half_angle {
        HALF_ANGLE_RATIONAL_KIND_TAG
    } else {
        ORDINARY_KIND_TAG
    }]);
    for endpoint in domain {
        hash.update(endpoint.to_bits().to_be_bytes());
    }
    hash.update(
        u64::try_from(entries.len() + half_angle_entries.len())
            .expect("an in-memory schedule entry count must fit u64")
            .to_be_bytes(),
    );
    for entry in entries {
        hash.update([ORDINARY_KIND_TAG]);
        hash.update(entry.edge.canonical_bytes());
        hash.update(entry.initial.to_bits().to_be_bytes());
        hash.update(
            u64::try_from(entry.coefficients.len())
                .expect("an in-memory coefficient count must fit u64")
                .to_be_bytes(),
        );
        for coefficient in &entry.coefficients {
            hash.update(coefficient.to_bits().to_be_bytes());
        }
    }
    for entry in half_angle_entries {
        hash.update([HALF_ANGLE_RATIONAL_KIND_TAG]);
        hash.update(entry.edge.canonical_bytes());
        for value in &entry.u_domain {
            update_canonical_big_rational_v2(&mut hash, value);
        }
        hash.update(
            u64::try_from(entry.numerator_power_coefficients.len())
                .expect("an in-memory numerator coefficient count must fit u64")
                .to_be_bytes(),
        );
        for value in &entry.numerator_power_coefficients {
            update_canonical_big_rational_v2(&mut hash, value);
        }
        hash.update(
            u64::try_from(entry.denominator_power_coefficients.len())
                .expect("an in-memory denominator coefficient count must fit u64")
                .to_be_bytes(),
        );
        for value in &entry.denominator_power_coefficients {
            update_canonical_big_rational_v2(&mut hash, value);
        }
    }
    hash.finalize().into()
}

fn update_length_prefixed_bytes_v2(hash: &mut Sha256, value: &[u8]) {
    hash.update(
        u64::try_from(value.len())
            .expect("an in-memory model identifier length must fit u64")
            .to_be_bytes(),
    );
    hash.update(value);
}

/// Appends one reduced [`BigRational`] using the cross-runtime V2 framing.
///
/// `BigRational` keeps denominators positive and values reduced. Encoding the
/// numerator sign separately from its unsigned magnitude therefore gives one
/// byte representation per mathematical rational. Zero is encoded with the
/// `NoSign` tag and one `00` numerator-magnitude byte.
fn update_canonical_big_rational_v2(hash: &mut Sha256, value: &BigRational) {
    hash.update([match value.numer().sign() {
        num_bigint::Sign::Minus => 0,
        num_bigint::Sign::NoSign => 1,
        num_bigint::Sign::Plus => 2,
    }]);
    update_canonical_big_int_magnitude_v2(hash, value.numer());
    update_canonical_big_int_magnitude_v2(hash, value.denom());
}

/// Streams the existing canonical big-endian unsigned magnitude without
/// allocating the temporary byte vectors returned by `BigInt::to_bytes_be`.
fn update_canonical_big_int_magnitude_v2(hash: &mut Sha256, value: &BigInt) {
    let encoded_byte_len = value.bits().div_ceil(8).max(1);
    hash.update(encoded_byte_len.to_be_bytes());
    if value.bits() == 0 {
        hash.update([0]);
        return;
    }
    let leading_byte_count = usize::try_from((encoded_byte_len - 1) % 4 + 1)
        .expect("a leading u32 digit contains at most four bytes");
    for (index, digit) in value.iter_u32_digits().rev().enumerate() {
        let bytes = digit.to_be_bytes();
        if index == 0 {
            hash.update(&bytes[4 - leading_byte_count..]);
        } else {
            hash.update(bytes);
        }
    }
}

fn update_canonical_big_rational_with_checkpoint_v1(
    hash: &mut Sha256,
    value: &BigRational,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<(), CycleScheduleRestrictionErrorV1> {
    cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
    update_canonical_big_rational_v2(hash, value);
    Ok(())
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

fn binding_fingerprint_with_checkpoint_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<[u8; 32], CycleScheduleRestrictionErrorV1> {
    let mut hash = Sha256::new();
    hash.update(fixed_face.canonical_bytes());
    for face in audit.faces() {
        cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
        cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    for hinge in geometry.hinges() {
        cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
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
            cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
            hash.update(value.to_bits().to_be_bytes());
        }
    }
    cycle_schedule_restriction_checkpoint_v1(checkpoint)?;
    Ok(hash.finalize().into())
}

fn cycle_schedule_restriction_checkpoint_v1(
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<(), CycleScheduleRestrictionErrorV1> {
    checkpoint().map_err(|stop| match stop {
        CycleScheduleRestrictionStopV1::Cancelled => CycleScheduleRestrictionErrorV1::Cancelled,
        CycleScheduleRestrictionStopV1::DeadlineExceeded => {
            CycleScheduleRestrictionErrorV1::DeadlineExceeded
        }
    })
}

#[cfg(test)]
#[path = "schedule/tests.rs"]
mod tests;
