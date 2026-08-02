//! Checkpointed exact-rational interval evaluation for half-angle schedules.

use super::*;

#[derive(Debug, Default)]
pub(super) struct CycleScheduleExactVectorMeterV2 {
    pub(super) peak_bytes: usize,
    max_bytes: usize,
}

impl CycleScheduleExactVectorMeterV2 {
    pub(super) fn new(max_bytes: usize) -> Self {
        Self {
            peak_bytes: 0,
            max_bytes,
        }
    }

    fn observe(&mut self, live_bytes: usize) -> Result<(), CycleScheduleDyadicEvaluationErrorV2> {
        if live_bytes > self.max_bytes {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        self.peak_bytes = self.peak_bytes.max(live_bytes);
        Ok(())
    }
}

fn try_exact_workspace_vec_v2(
    capacity: usize,
    base_live_bytes: usize,
    meter: &mut CycleScheduleExactVectorMeterV2,
) -> Result<(Vec<BigRational>, usize), CycleScheduleDyadicEvaluationErrorV2> {
    let logical_bytes = std::mem::size_of::<BigRational>()
        .checked_mul(capacity)
        .and_then(|bytes| bytes.checked_add(base_live_bytes))
        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    meter.observe(logical_bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    let allocation_bytes = std::mem::size_of::<BigRational>()
        .checked_mul(values.capacity())
        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    meter.observe(
        base_live_bytes
            .checked_add(allocation_bytes)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
    )?;
    Ok((values, allocation_bytes))
}

fn validate_exact_bits_with_checkpoint_v2(
    coefficients: &[BigRational],
    maximum: u32,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
) -> Result<(), CycleScheduleDyadicEvaluationErrorV2> {
    for value in coefficients {
        checkpoint()?;
        if value.numer().bits() > u64::from(maximum) || value.denom().bits() > u64::from(maximum) {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
        }
    }
    Ok(())
}

fn certificate_range_interval_with_checkpoint_v2(
    certificate: &PoleFreeBernsteinCertificateV1,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
) -> Result<OutwardIntervalV1, CycleScheduleDyadicEvaluationErrorV2> {
    let mut minimum: Option<&BigRational> = None;
    let mut maximum: Option<&BigRational> = None;
    for value in &certificate.coefficients {
        checkpoint()?;
        if minimum.is_none_or(|current| value < current) {
            minimum = Some(value);
        }
        if maximum.is_none_or(|current| value > current) {
            maximum = Some(value);
        }
    }
    let lower = minimum
        .and_then(ToPrimitive::to_f64)
        .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
    let upper = maximum
        .and_then(ToPrimitive::to_f64)
        .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
    let lower = OutwardIntervalV1::from_rounded(lower)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    let upper = OutwardIntervalV1::from_rounded(upper)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    OutwardIntervalV1::new(lower.lower(), upper.upper())
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput.into())
}

fn evaluate_pole_free_rational_interval_with_checkpoint_v2(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    max_work: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
) -> Result<OutwardIntervalV1, CycleScheduleDyadicEvaluationErrorV2> {
    let work = numerator
        .coefficients
        .len()
        .checked_add(denominator.coefficients.len())
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    if work > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    let numerator = certificate_range_interval_with_checkpoint_v2(numerator, checkpoint)?;
    let denominator = certificate_range_interval_with_checkpoint_v2(denominator, checkpoint)?;
    if denominator.lower() <= 0.0 && denominator.upper() >= 0.0 {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    numerator
        .div(denominator)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput.into())
}

fn evaluate_half_angle_rational_degrees_interval_with_checkpoint_v2(
    y: &PoleFreeBernsteinCertificateV1,
    x: &PoleFreeBernsteinCertificateV1,
    max_work: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
) -> Result<OutwardIntervalV1, CycleScheduleDyadicEvaluationErrorV2> {
    let x_has_endpoint_zero = x.coefficients.first().is_some_and(Zero::is_zero)
        || x.coefficients.last().is_some_and(Zero::is_zero);
    let mut y_strictly_positive = true;
    for value in &y.coefficients {
        checkpoint()?;
        y_strictly_positive &= value.is_positive();
    }
    let radians = if x_has_endpoint_zero && x.positive && y.positive && y_strictly_positive {
        let ratio =
            evaluate_pole_free_rational_interval_with_checkpoint_v2(x, y, max_work, checkpoint)?;
        let atan = crate::atan_interval_v1(ratio, max_work)
            .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
        let half_pi = OutwardIntervalV1::from_rounded(core::f64::consts::FRAC_PI_2)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        half_pi
            .sub(atan)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?
    } else {
        let ratio =
            evaluate_pole_free_rational_interval_with_checkpoint_v2(y, x, max_work, checkpoint)?;
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
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
        }
        angle
    };
    checkpoint()?;
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
        return Err(CycleSchedulePrepareErrorV1::AngleRange.into());
    }
    let enclosure = enclosure
        .intersect_bounds(0.0, 180.0)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
    if enclosure.work() > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    checkpoint()?;
    Ok(enclosure)
}

#[allow(clippy::too_many_arguments)]
fn affine_reparameterize_power_with_workspace_v2(
    power: &[BigRational],
    domain: &[BigRational; 2],
    max_coefficient_bits: u32,
    max_work: usize,
    base_live_bytes: usize,
    meter: &mut CycleScheduleExactVectorMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
) -> Result<(Vec<BigRational>, usize), CycleScheduleDyadicEvaluationErrorV2> {
    checkpoint()?;
    if power
        .len()
        .checked_mul(power.len())
        .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    let a = &domain[0];
    let width = &domain[1] - a;
    let (mut result, allocation_bytes) =
        try_exact_workspace_vec_v2(power.len(), base_live_bytes, meter)?;
    for _ in 0..power.len() {
        checkpoint()?;
        result.push(BigRational::zero());
    }
    for (degree, coefficient) in power.iter().enumerate() {
        checkpoint()?;
        for (k, output) in result.iter_mut().enumerate().take(degree + 1) {
            checkpoint()?;
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
    validate_exact_bits_with_checkpoint_v2(&result, max_coefficient_bits, checkpoint)?;
    Ok((result, allocation_bytes))
}

#[allow(clippy::too_many_arguments)]
fn prepare_exact_signed_bernstein_certificate_with_workspace_v2(
    power: Vec<BigRational>,
    power_allocation_bytes: usize,
    max_degree: usize,
    max_coefficient_bits: u32,
    max_work: usize,
    allow_endpoint_zero: bool,
    external_live_bytes: usize,
    meter: &mut CycleScheduleExactVectorMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
) -> Result<(PoleFreeBernsteinCertificateV1, usize), CycleScheduleDyadicEvaluationErrorV2> {
    checkpoint()?;
    if power.is_empty()
        || power.len() > max_degree.saturating_add(1)
        || power
            .len()
            .checked_mul(power.len())
            .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    meter.observe(
        external_live_bytes
            .checked_add(power_allocation_bytes)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
    )?;
    validate_exact_bits_with_checkpoint_v2(&power, max_coefficient_bits, checkpoint)?;
    let degree = power.len() - 1;
    let coefficient_count = degree
        .checked_add(1)
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    let power_live_bytes = external_live_bytes
        .checked_add(power_allocation_bytes)
        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    let (mut coefficients, coefficient_allocation_bytes) =
        try_exact_workspace_vec_v2(coefficient_count, power_live_bytes, meter)?;
    for i in 0..=degree {
        checkpoint()?;
        let mut value = BigRational::zero();
        for (k, coefficient) in power.iter().enumerate().take(i + 1) {
            checkpoint()?;
            let numerator =
                checked_binomial_v1(i, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let denominator =
                checked_binomial_v1(degree, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            value +=
                coefficient * BigRational::new(BigInt::from(numerator), BigInt::from(denominator));
        }
        coefficients.push(value);
    }
    validate_exact_bits_with_checkpoint_v2(&coefficients, max_coefficient_bits, checkpoint)?;
    let mut strictly_positive = true;
    let mut strictly_negative = true;
    let mut endpoint_zero = allow_endpoint_zero;
    for (index, value) in coefficients.iter().enumerate() {
        checkpoint()?;
        strictly_positive &= value.is_positive();
        strictly_negative &= value.is_negative();
        endpoint_zero &= value.is_positive()
            || (value.is_zero() && (index == 0 || index + 1 == coefficients.len()));
    }
    if !strictly_positive && !strictly_negative && !endpoint_zero {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    Ok((
        PoleFreeBernsteinCertificateV1 {
            degree,
            positive: strictly_positive || endpoint_zero,
            coefficients,
        },
        coefficient_allocation_bytes,
    ))
}

impl PreparedHalfAngleRationalEntryV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the private exact evaluator keeps each independent finite policy and meter explicit"
    )]
    pub(super) fn angle_enclosure_dyadic_with_workspace_v2(
        &self,
        depth: u32,
        index: u64,
        max_coefficient_bits: u32,
        max_degree: usize,
        max_work: usize,
        meter: &mut CycleScheduleExactVectorMeterV2,
        checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
    ) -> Result<OutwardIntervalV1, CycleScheduleDyadicEvaluationErrorV2> {
        checkpoint()?;
        if depth >= 64 || index >= (1u64 << depth) {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let mut numerator_is_zero = true;
        for coefficient in &self.numerator_power_coefficients {
            checkpoint()?;
            numerator_is_zero &= coefficient.is_zero();
        }
        if numerator_is_zero {
            return OutwardIntervalV1::from_rounded(0.0)
                .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let denominator = BigInt::from(1u64 << depth);
        let width = &self.u_domain[1] - &self.u_domain[0];
        let lower =
            &self.u_domain[0] + &width * BigRational::new(BigInt::from(index), denominator.clone());
        let upper =
            &self.u_domain[0] + width * BigRational::new(BigInt::from(index + 1), denominator);
        let domain = [lower, upper];
        let (numerator_power, numerator_power_bytes) =
            affine_reparameterize_power_with_workspace_v2(
                &self.numerator_power_coefficients,
                &domain,
                max_coefficient_bits,
                max_work,
                0,
                meter,
                checkpoint,
            )?;
        let (numerator, numerator_bytes) =
            prepare_exact_signed_bernstein_certificate_with_workspace_v2(
                numerator_power,
                numerator_power_bytes,
                max_degree,
                max_coefficient_bits,
                max_work,
                true,
                0,
                meter,
                checkpoint,
            )?;
        let (denominator_power, denominator_power_bytes) =
            affine_reparameterize_power_with_workspace_v2(
                &self.denominator_power_coefficients,
                &domain,
                max_coefficient_bits,
                max_work,
                numerator_bytes,
                meter,
                checkpoint,
            )?;
        let (denominator, denominator_bytes) =
            prepare_exact_signed_bernstein_certificate_with_workspace_v2(
                denominator_power,
                denominator_power_bytes,
                max_degree,
                max_coefficient_bits,
                max_work,
                true,
                numerator_bytes,
                meter,
                checkpoint,
            )?;
        meter.observe(
            numerator_bytes
                .checked_add(denominator_bytes)
                .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
        )?;
        for (numerator, denominator) in numerator.coefficients.iter().zip(&denominator.coefficients)
        {
            checkpoint()?;
            if numerator.is_zero() && denominator.is_zero() {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
            }
        }
        let result = evaluate_half_angle_rational_degrees_interval_with_checkpoint_v2(
            &numerator,
            &denominator,
            max_work,
            checkpoint,
        )?;
        checkpoint()?;
        Ok(result)
    }
}
