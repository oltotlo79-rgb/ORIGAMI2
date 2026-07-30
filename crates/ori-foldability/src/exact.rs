use std::cmp::Ordering;

use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use serde::Serialize;

use crate::GlobalFlatFoldabilityPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactSign {
    Negative,
    Zero,
    Positive,
}

/// Canonical numerator magnitude and positive denominator, both big-endian.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ExactRationalValue {
    pub sign: ExactSign,
    pub numerator_magnitude_be: Vec<u8>,
    pub denominator_be: Vec<u8>,
}

impl ExactRationalValue {
    #[must_use]
    pub fn to_f64(&self) -> Option<f64> {
        let magnitude = BigInt::from_bytes_be(Sign::Plus, &self.numerator_magnitude_be);
        let numerator = match self.sign {
            ExactSign::Negative => -magnitude,
            ExactSign::Zero => BigInt::from(0),
            ExactSign::Positive => magnitude,
        };
        let denominator = BigInt::from_bytes_be(Sign::Plus, &self.denominator_be);
        if denominator.is_zero() {
            return None;
        }
        BigRational::new(numerator, denominator)
            .to_f64()
            .filter(|value| value.is_finite())
    }

    /// Deterministic finite projection without constructing heap-backed
    /// `BigInt`/`BigRational` temporaries.
    #[must_use]
    pub fn to_f64_without_heap_v1(&self) -> Option<f64> {
        let (denominator, denominator_exponent) =
            leading_binary64_from_big_endian_bytes_v1(&self.denominator_be)?;
        if denominator == 0.0 {
            return None;
        }
        let (numerator, numerator_exponent) =
            leading_binary64_from_big_endian_bytes_v1(&self.numerator_magnitude_be)?;
        if numerator == 0.0 || self.sign == ExactSign::Zero {
            return Some(0.0);
        }
        let exponent = numerator_exponent.checked_sub(denominator_exponent)?;
        let magnitude = scale_binary64_by_power_of_two_v1(numerator / denominator, exponent)?;
        Some(match self.sign {
            ExactSign::Negative => -magnitude,
            ExactSign::Zero => 0.0,
            ExactSign::Positive => magnitude,
        })
    }
}

fn leading_binary64_from_big_endian_bytes_v1(bytes: &[u8]) -> Option<(f64, i32)> {
    let bytes = bytes
        .iter()
        .position(|byte| *byte != 0)
        .map_or(&[][..], |first| &bytes[first..]);
    if bytes.is_empty() {
        return Some((0.0, 0));
    }
    let retained = bytes.len().min(8);
    let mut prefix = 0.0_f64;
    for byte in &bytes[..retained] {
        prefix = prefix * 256.0 + f64::from(*byte);
    }
    let trailing_bytes = bytes.len().checked_sub(retained)?;
    let exponent = i32::try_from(trailing_bytes.checked_mul(8)?).ok()?;
    Some((prefix, exponent))
}

fn scale_binary64_by_power_of_two_v1(mut value: f64, mut exponent: i32) -> Option<f64> {
    const UP_512: f64 = f64::from_bits((1023_u64 + 512) << 52);
    const DOWN_512: f64 = f64::from_bits((1023_u64 - 512) << 52);
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    if exponent > 4096 {
        return None;
    }
    if exponent < -4096 {
        return Some(0.0);
    }
    while exponent > 512 {
        value *= UP_512;
        if !value.is_finite() {
            return None;
        }
        exponent -= 512;
    }
    while exponent < -512 {
        value *= DOWN_512;
        if value == 0.0 {
            return Some(0.0);
        }
        exponent += 512;
    }
    let factor_exponent = u64::try_from(1023_i32.checked_add(exponent)?).ok()?;
    value *= f64::from_bits(factor_exponent << 52);
    value.is_finite().then_some(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ExactPointValue {
    pub x: ExactRationalValue,
    pub y: ExactRationalValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ExactAffineTransform {
    pub m00: ExactRationalValue,
    pub m01: ExactRationalValue,
    pub m10: ExactRationalValue,
    pub m11: ExactRationalValue,
    pub tx: ExactRationalValue,
    pub ty: ExactRationalValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactError {
    NonFiniteBinary64,
    NegativeZero,
    DegenerateDivision,
    IntegerBitLimitReached {
        limit_bits: usize,
        observed_bits: usize,
    },
    WorkLimitReached {
        limit: usize,
        observed: usize,
    },
    DeadlineReached {
        phase: GlobalFlatFoldabilityPhase,
    },
    Cancelled,
    InternalFailure,
}

pub(crate) trait ExactBudget {
    fn record_exact_operation(&mut self) -> Result<(), ExactError>;
    fn record_exact_value(&mut self, value: &BigRational) -> Result<(), ExactError>;
}

pub(crate) type Rational = BigRational;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Point {
    pub x: Rational,
    pub y: Rational,
}

impl Point {
    pub(crate) fn to_value(&self) -> ExactPointValue {
        ExactPointValue {
            x: rational_value(&self.x),
            y: rational_value(&self.y),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Transform {
    pub m00: Rational,
    pub m01: Rational,
    pub m10: Rational,
    pub m11: Rational,
    pub tx: Rational,
    pub ty: Rational,
}

impl Transform {
    pub(crate) fn identity() -> Self {
        Self {
            m00: Rational::one(),
            m01: Rational::zero(),
            m10: Rational::zero(),
            m11: Rational::one(),
            tx: Rational::zero(),
            ty: Rational::zero(),
        }
    }

    pub(crate) fn to_value(&self) -> ExactAffineTransform {
        ExactAffineTransform {
            m00: rational_value(&self.m00),
            m01: rational_value(&self.m01),
            m10: rational_value(&self.m10),
            m11: rational_value(&self.m11),
            tx: rational_value(&self.tx),
            ty: rational_value(&self.ty),
        }
    }
}

pub(crate) fn rational_value(value: &Rational) -> ExactRationalValue {
    let (numerator_sign, numerator) = value.numer().to_bytes_be();
    let (_, denominator) = value.denom().to_bytes_be();
    let sign = if value.is_zero() {
        ExactSign::Zero
    } else if numerator_sign == Sign::Minus {
        ExactSign::Negative
    } else {
        ExactSign::Positive
    };
    ExactRationalValue {
        sign,
        numerator_magnitude_be: numerator,
        denominator_be: denominator,
    }
}

pub(crate) fn rational_bytes(value: &Rational) -> Result<Vec<u8>, ExactError> {
    let capacity = rational_storage_bytes(value)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| ExactError::InternalFailure)?;
    bytes.push(match value.numer().sign() {
        Sign::Minus => 0,
        Sign::NoSign => 1,
        Sign::Plus => 2,
    });
    append_big_int_magnitude_v1(&mut bytes, value.numer())?;
    append_big_int_magnitude_v1(&mut bytes, value.denom())?;
    Ok(bytes)
}

/// Bytes required by the canonical binary exact-value payload.
///
/// This mirrors `rational_bytes` without allocating the numerator and
/// denominator byte vectors, so callers can reserve aggregate certificate
/// storage before materializing a snapshot value.
pub(crate) fn rational_storage_bytes(value: &Rational) -> Result<usize, ExactError> {
    let numerator_bytes = magnitude_byte_len_v1(value.numer().bits())?;
    let denominator_bytes = magnitude_byte_len_v1(value.denom().bits())?;
    1_usize
        .checked_add(std::mem::size_of::<u64>())
        .and_then(|total| total.checked_add(numerator_bytes))
        .and_then(|total| total.checked_add(std::mem::size_of::<u64>()))
        .and_then(|total| total.checked_add(denominator_bytes))
        .ok_or(ExactError::InternalFailure)
}

fn magnitude_byte_len_v1(bits: u64) -> Result<usize, ExactError> {
    let rounded = bits.checked_add(7).ok_or(ExactError::InternalFailure)? / 8;
    usize::try_from(rounded.max(1)).map_err(|_| ExactError::InternalFailure)
}

fn append_big_int_magnitude_v1(target: &mut Vec<u8>, value: &BigInt) -> Result<(), ExactError> {
    let byte_len = magnitude_byte_len_v1(value.bits())?;
    let encoded_len = u64::try_from(byte_len).map_err(|_| ExactError::InternalFailure)?;
    target.extend_from_slice(&encoded_len.to_be_bytes());
    if value.is_zero() {
        target.push(0);
        return Ok(());
    }
    let leading_byte_count = (byte_len - 1) % 4 + 1;
    for (index, digit) in value.iter_u32_digits().rev().enumerate() {
        let bytes = digit.to_be_bytes();
        if index == 0 {
            target.extend_from_slice(&bytes[4 - leading_byte_count..]);
        } else {
            target.extend_from_slice(&bytes);
        }
    }
    Ok(())
}

pub(crate) fn from_binary64<B: ExactBudget>(
    value: f64,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    budget.record_exact_operation()?;
    let bits = value.to_bits();
    if bits == (-0.0_f64).to_bits() {
        return Err(ExactError::NegativeZero);
    }
    if !value.is_finite() {
        return Err(ExactError::NonFiniteBinary64);
    }
    if value == 0.0 {
        let result = Rational::zero();
        budget.record_exact_value(&result)?;
        return Ok(result);
    }

    let negative = bits >> 63 != 0;
    let encoded_exponent = ((bits >> 52) & 0x7ff) as i32;
    let encoded_mantissa = bits & ((1_u64 << 52) - 1);
    let (mantissa, exponent) = if encoded_exponent == 0 {
        (encoded_mantissa, 1 - 1023 - 52)
    } else {
        (
            encoded_mantissa | (1_u64 << 52),
            encoded_exponent - 1023 - 52,
        )
    };
    let mut numerator = BigInt::from(mantissa);
    if negative {
        numerator = -numerator;
    }
    let result = if exponent >= 0 {
        let shift = usize::try_from(exponent).map_err(|_| ExactError::InternalFailure)?;
        Rational::from_integer(numerator << shift)
    } else {
        let shift = usize::try_from(-exponent).map_err(|_| ExactError::InternalFailure)?;
        Rational::new(numerator, BigInt::one() << shift)
    };
    budget.record_exact_value(&result)?;
    Ok(result)
}

pub(crate) fn point_from_binary64<B: ExactBudget>(
    x: f64,
    y: f64,
    budget: &mut B,
) -> Result<Point, ExactError> {
    Ok(Point {
        x: from_binary64(x, budget)?,
        y: from_binary64(y, budget)?,
    })
}

pub(crate) fn add<B: ExactBudget>(
    left: &Rational,
    right: &Rational,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    budget.record_exact_operation()?;
    let result = left + right;
    budget.record_exact_value(&result)?;
    Ok(result)
}

pub(crate) fn sub<B: ExactBudget>(
    left: &Rational,
    right: &Rational,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    budget.record_exact_operation()?;
    let result = left - right;
    budget.record_exact_value(&result)?;
    Ok(result)
}

pub(crate) fn mul<B: ExactBudget>(
    left: &Rational,
    right: &Rational,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    budget.record_exact_operation()?;
    let result = left * right;
    budget.record_exact_value(&result)?;
    Ok(result)
}

pub(crate) fn div<B: ExactBudget>(
    numerator: &Rational,
    denominator: &Rational,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    if denominator.is_zero() {
        return Err(ExactError::DegenerateDivision);
    }
    budget.record_exact_operation()?;
    let result = numerator / denominator;
    budget.record_exact_value(&result)?;
    Ok(result)
}

pub(crate) fn neg<B: ExactBudget>(
    value: &Rational,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    budget.record_exact_operation()?;
    let result = -value;
    budget.record_exact_value(&result)?;
    Ok(result)
}

pub(crate) fn cmp<B: ExactBudget>(
    left: &Rational,
    right: &Rational,
    budget: &mut B,
) -> Result<Ordering, ExactError> {
    budget.record_exact_operation()?;
    Ok(left.cmp(right))
}

pub(crate) fn cross<B: ExactBudget>(
    origin: &Point,
    first: &Point,
    second: &Point,
    budget: &mut B,
) -> Result<Rational, ExactError> {
    let ax = sub(&first.x, &origin.x, budget)?;
    let ay = sub(&first.y, &origin.y, budget)?;
    let bx = sub(&second.x, &origin.x, budget)?;
    let by = sub(&second.y, &origin.y, budget)?;
    let left = mul(&ax, &by, budget)?;
    let right = mul(&ay, &bx, budget)?;
    sub(&left, &right, budget)
}

pub(crate) fn signed_double_area<B: ExactBudget>(
    polygon: &[Point],
    budget: &mut B,
) -> Result<Rational, ExactError> {
    let mut area = Rational::zero();
    for index in 0..polygon.len() {
        let current = &polygon[index];
        let next = &polygon[(index + 1) % polygon.len()];
        let first = mul(&current.x, &next.y, budget)?;
        let second = mul(&current.y, &next.x, budget)?;
        let term = sub(&first, &second, budget)?;
        area = add(&area, &term, budget)?;
    }
    Ok(area)
}

pub(crate) fn apply<B: ExactBudget>(
    transform: &Transform,
    point: &Point,
    budget: &mut B,
) -> Result<Point, ExactError> {
    let x0 = mul(&transform.m00, &point.x, budget)?;
    let x1 = mul(&transform.m01, &point.y, budget)?;
    let x = add(&add(&x0, &x1, budget)?, &transform.tx, budget)?;
    let y0 = mul(&transform.m10, &point.x, budget)?;
    let y1 = mul(&transform.m11, &point.y, budget)?;
    let y = add(&add(&y0, &y1, budget)?, &transform.ty, budget)?;
    Ok(Point { x, y })
}

pub(crate) fn reflection_across<B: ExactBudget>(
    first: &Point,
    second: &Point,
    budget: &mut B,
) -> Result<Transform, ExactError> {
    let dx = sub(&second.x, &first.x, budget)?;
    let dy = sub(&second.y, &first.y, budget)?;
    let dx2 = mul(&dx, &dx, budget)?;
    let dy2 = mul(&dy, &dy, budget)?;
    let denominator = add(&dx2, &dy2, budget)?;
    if denominator.is_zero() {
        return Err(ExactError::DegenerateDivision);
    }
    let m00 = div(&sub(&dx2, &dy2, budget)?, &denominator, budget)?;
    let two = Rational::from_integer(BigInt::from(2_u8));
    budget.record_exact_value(&two)?;
    let m01 = div(
        &mul(&two, &mul(&dx, &dy, budget)?, budget)?,
        &denominator,
        budget,
    )?;
    let m10 = m01.clone();
    let m11 = neg(&m00, budget)?;
    let projected_x = add(
        &mul(&m00, &first.x, budget)?,
        &mul(&m01, &first.y, budget)?,
        budget,
    )?;
    let projected_y = add(
        &mul(&m10, &first.x, budget)?,
        &mul(&m11, &first.y, budget)?,
        budget,
    )?;
    let tx = sub(&first.x, &projected_x, budget)?;
    let ty = sub(&first.y, &projected_y, budget)?;
    Ok(Transform {
        m00,
        m01,
        m10,
        m11,
        tx,
        ty,
    })
}

pub(crate) fn compose<B: ExactBudget>(
    outer: &Transform,
    inner: &Transform,
    budget: &mut B,
) -> Result<Transform, ExactError> {
    let m00 = add(
        &mul(&outer.m00, &inner.m00, budget)?,
        &mul(&outer.m01, &inner.m10, budget)?,
        budget,
    )?;
    let m01 = add(
        &mul(&outer.m00, &inner.m01, budget)?,
        &mul(&outer.m01, &inner.m11, budget)?,
        budget,
    )?;
    let m10 = add(
        &mul(&outer.m10, &inner.m00, budget)?,
        &mul(&outer.m11, &inner.m10, budget)?,
        budget,
    )?;
    let m11 = add(
        &mul(&outer.m10, &inner.m01, budget)?,
        &mul(&outer.m11, &inner.m11, budget)?,
        budget,
    )?;
    let tx = add(
        &add(
            &mul(&outer.m00, &inner.tx, budget)?,
            &mul(&outer.m01, &inner.ty, budget)?,
            budget,
        )?,
        &outer.tx,
        budget,
    )?;
    let ty = add(
        &add(
            &mul(&outer.m10, &inner.tx, budget)?,
            &mul(&outer.m11, &inner.ty, budget)?,
            budget,
        )?,
        &outer.ty,
        budget,
    )?;
    Ok(Transform {
        m00,
        m01,
        m10,
        m11,
        tx,
        ty,
    })
}

pub(crate) fn midpoint<B: ExactBudget>(
    first: &Point,
    second: &Point,
    budget: &mut B,
) -> Result<Point, ExactError> {
    let two = Rational::from_integer(BigInt::from(2_u8));
    budget.record_exact_value(&two)?;
    Ok(Point {
        x: div(&add(&first.x, &second.x, budget)?, &two, budget)?,
        y: div(&add(&first.y, &second.y, budget)?, &two, budget)?,
    })
}

pub(crate) fn average3<B: ExactBudget>(
    first: &Point,
    second: &Point,
    third: &Point,
    budget: &mut B,
) -> Result<Point, ExactError> {
    let three = Rational::from_integer(BigInt::from(3_u8));
    budget.record_exact_value(&three)?;
    Ok(Point {
        x: div(
            &add(&add(&first.x, &second.x, budget)?, &third.x, budget)?,
            &three,
            budget,
        )?,
        y: div(
            &add(&add(&first.y, &second.y, budget)?, &third.y, budget)?,
            &three,
            budget,
        )?,
    })
}

pub(crate) fn bit_len(value: &Rational) -> Result<usize, ExactError> {
    let numerator =
        usize::try_from(value.numer().bits()).map_err(|_| ExactError::InternalFailure)?;
    let denominator =
        usize::try_from(value.denom().bits()).map_err(|_| ExactError::InternalFailure)?;
    Ok(numerator.max(denominator))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Budget;

    impl ExactBudget for Budget {
        fn record_exact_operation(&mut self) -> Result<(), ExactError> {
            Ok(())
        }

        fn record_exact_value(&mut self, _value: &BigRational) -> Result<(), ExactError> {
            Ok(())
        }
    }

    #[test]
    fn binary64_conversion_preserves_the_exact_stored_value() {
        let value = from_binary64(0.1, &mut Budget).expect("finite binary64");
        assert_eq!(
            value,
            BigRational::new(
                BigInt::from(3_602_879_701_896_397_u64),
                BigInt::from(36_028_797_018_963_968_u64)
            )
        );
        assert_eq!(
            from_binary64(-0.0, &mut Budget),
            Err(ExactError::NegativeZero)
        );
        assert_eq!(
            from_binary64(f64::INFINITY, &mut Budget),
            Err(ExactError::NonFiniteBinary64)
        );
    }

    #[test]
    fn exact_reflection_keeps_the_axis_and_flips_the_other_side() {
        let mut budget = Budget;
        let first = point_from_binary64(0.0, 0.0, &mut budget).expect("point");
        let second = point_from_binary64(1.0, 0.0, &mut budget).expect("point");
        let point = point_from_binary64(2.0, 3.0, &mut budget).expect("point");
        let reflection = reflection_across(&first, &second, &mut budget).expect("axis");
        let reflected = apply(&reflection, &point, &mut budget).expect("reflection");
        assert_eq!(
            reflected,
            point_from_binary64(2.0, -3.0, &mut budget).expect("point")
        );
    }

    #[test]
    fn exact_storage_size_matches_the_canonical_binary_payload() {
        let large_numerator: BigInt =
            (BigInt::from(1_u8) << 96_usize) + BigInt::from(0x0102_0304_u32);
        let large_denominator: BigInt = (BigInt::from(1_u8) << 65_usize) + BigInt::from(3_u8);
        for value in [
            BigRational::from_integer(BigInt::from(0)),
            BigRational::from_integer(BigInt::from(-255)),
            BigRational::new(BigInt::from(65_537), BigInt::from(257)),
            BigRational::new(-large_numerator, large_denominator),
        ] {
            let bytes = rational_bytes(&value).expect("canonical payload");
            assert_eq!(
                rational_storage_bytes(&value).expect("storage size"),
                bytes.len()
            );
            let (sign, numerator) = value.numer().to_bytes_be();
            let (_, denominator) = value.denom().to_bytes_be();
            let mut reference = vec![match sign {
                Sign::Minus => 0,
                Sign::NoSign => 1,
                Sign::Plus => 2,
            }];
            reference.extend_from_slice(&(numerator.len() as u64).to_be_bytes());
            reference.extend_from_slice(&numerator);
            reference.extend_from_slice(&(denominator.len() as u64).to_be_bytes());
            reference.extend_from_slice(&denominator);
            assert_eq!(bytes, reference);
        }
    }

    #[test]
    fn canonical_exact_rational_has_a_finite_viewer_projection() {
        let value = ExactRationalValue {
            sign: ExactSign::Negative,
            numerator_magnitude_be: vec![3],
            denominator_be: vec![2],
        };
        assert_eq!(value.to_f64(), Some(-1.5));
        assert_eq!(value.to_f64_without_heap_v1(), Some(-1.5));
        let invalid = ExactRationalValue {
            denominator_be: vec![0],
            ..value
        };
        assert_eq!(invalid.to_f64(), None);
        assert_eq!(invalid.to_f64_without_heap_v1(), None);
        for sign in [ExactSign::Negative, ExactSign::Zero, ExactSign::Positive] {
            let zero_over_zero = ExactRationalValue {
                sign,
                numerator_magnitude_be: vec![0],
                denominator_be: vec![0],
            };
            assert_eq!(zero_over_zero.to_f64(), None);
            assert_eq!(zero_over_zero.to_f64_without_heap_v1(), None);
        }
    }

    #[test]
    fn heapless_exact_projection_matches_representative_big_rationals() {
        for value in [
            ExactRationalValue {
                sign: ExactSign::Positive,
                numerator_magnitude_be: vec![1],
                denominator_be: vec![3],
            },
            ExactRationalValue {
                sign: ExactSign::Negative,
                numerator_magnitude_be: vec![0xff; 24],
                denominator_be: vec![0x80; 24],
            },
            ExactRationalValue {
                sign: ExactSign::Positive,
                numerator_magnitude_be: vec![1; 32],
                denominator_be: vec![0xff; 16],
            },
        ] {
            let expected = value.to_f64().expect("finite BigRational projection");
            let actual = value
                .to_f64_without_heap_v1()
                .expect("finite heapless projection");
            assert!(
                (actual - expected).abs() <= expected.abs().max(1.0) * f64::EPSILON * 2.0,
                "heapless projection drifted: expected={expected:?} actual={actual:?}"
            );
        }
    }
}
