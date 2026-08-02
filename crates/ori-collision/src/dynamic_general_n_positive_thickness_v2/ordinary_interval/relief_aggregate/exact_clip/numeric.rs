//! Bounded exact scalar operations used by one relief clip at a time.

use num_rational::BigRational;
use num_traits::{FromPrimitive, Zero};

use super::super::*;

pub(super) struct ExactMeterV2<'a> {
    pub(super) input: &'a ReliefAggregateInputV2<'a>,
    pub(super) resources: &'a mut ReliefAggregateResourcesV2,
}

impl ExactMeterV2<'_> {
    pub(super) fn value(&mut self, value: f64) -> Result<BigRational, ReliefAggregateErrorV2> {
        let value = BigRational::from_f64(value).ok_or(ReliefAggregateErrorV2::InvalidInput)?;
        self.charge(&value)?;
        Ok(value)
    }

    pub(super) fn add(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ReliefAggregateErrorV2> {
        let value = left + right;
        self.charge(&value)?;
        Ok(value)
    }

    pub(super) fn sub(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ReliefAggregateErrorV2> {
        let value = left - right;
        self.charge(&value)?;
        Ok(value)
    }

    pub(super) fn mul(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ReliefAggregateErrorV2> {
        let value = left * right;
        self.charge(&value)?;
        Ok(value)
    }

    pub(super) fn div(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ReliefAggregateErrorV2> {
        if right.is_zero() {
            return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
        }
        let value = left / right;
        self.charge(&value)?;
        Ok(value)
    }

    fn charge(&mut self, value: &BigRational) -> Result<(), ReliefAggregateErrorV2> {
        resources::charge_v2(
            &mut self.resources.exact_clip_operations,
            1,
            self.input.limits.max_exact_clip_operations,
        )?;
        let bits = usize::try_from(
            value
                .numer()
                .bits()
                .checked_add(value.denom().bits())
                .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
        )
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
        if bits > self.input.limits.max_exact_value_bits {
            return Err(ReliefAggregateErrorV2::ResourceLimit);
        }
        Ok(())
    }
}

pub(super) fn validate_hinge_policy_v2(
    policy: &HingeReliefPolicyRecordV1,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    let thickness = input.ordinary.paper_thickness_mm;
    if !policy.cutout_width_mm.is_finite()
        || policy.cutout_width_mm <= 0.0
        || !policy.bevel_angle_degrees.is_finite()
        || policy.bevel_angle_degrees <= 0.0
        || policy.bevel_angle_degrees >= 180.0
        || policy.material_thickness_mm.to_bits() != thickness.to_bits()
    {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    let mut meter = ExactMeterV2 { input, resources };
    let width = meter.value(policy.cutout_width_mm)?;
    let bevel = meter.value(policy.bevel_angle_degrees)?;
    let thickness = meter.value(thickness)?;
    let left = meter.mul(&width, &bevel)?;
    let right = meter.mul(&thickness, &BigRational::from_integer(60.into()))?;
    if left < right {
        return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
    }
    Ok(())
}

pub(super) fn validate_vertex_policy_v2(
    policy: &VertexReliefPolicyRecordV1,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    let thickness = input.ordinary.paper_thickness_mm;
    if !policy.cutout_radius_mm.is_finite()
        || policy.cutout_radius_mm <= 0.0
        || policy.material_thickness_mm.to_bits() != thickness.to_bits()
        || policy.incident_faces.len() < 2
    {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    let mut meter = ExactMeterV2 { input, resources };
    let radius = meter.value(policy.cutout_radius_mm)?;
    let thickness = meter.value(thickness)?;
    if radius < thickness {
        return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
    }
    Ok(())
}

pub(super) fn sqrt_lower_v2(
    value: &BigRational,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<BigRational, ReliefAggregateErrorV2> {
    sqrt_bounds_v2(value, input, resources, checkpoint).map(|bounds| bounds.0)
}

pub(super) fn sqrt_upper_v2(
    value: &BigRational,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<BigRational, ReliefAggregateErrorV2> {
    sqrt_bounds_v2(value, input, resources, checkpoint).map(|bounds| bounds.1)
}

fn sqrt_bounds_v2(
    value: &BigRational,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(BigRational, BigRational), ReliefAggregateErrorV2> {
    relief_checkpoint_v2(checkpoint)?;
    resources.sqrt_calls = resources
        .sqrt_calls
        .checked_add(1)
        .filter(|value| *value <= input.limits.max_sqrt_calls)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let limits = ori_numeric::ExpressionLimits {
        max_operations: input.limits.max_sqrt_operations_per_call,
        max_value_bits: input.limits.max_exact_value_bits,
        ..ori_numeric::ExpressionLimits::default()
    };
    let bounds = ori_numeric::rational_sqrt_bounds(value, limits).map_err(|error| match error {
        ori_numeric::ExpressionError::ResourceLimit(_) => ReliefAggregateErrorV2::ResourceLimit,
        _ => ReliefAggregateErrorV2::UnprovenSharedRelief,
    })?;
    relief_checkpoint_v2(checkpoint)?;
    for bound in [&bounds.0, &bounds.1] {
        let bits = bound
            .numer()
            .bits()
            .checked_add(bound.denom().bits())
            .and_then(|bits| usize::try_from(bits).ok())
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
        if bits > input.limits.max_exact_value_bits {
            return Err(ReliefAggregateErrorV2::ResourceLimit);
        }
    }
    Ok(bounds)
}
