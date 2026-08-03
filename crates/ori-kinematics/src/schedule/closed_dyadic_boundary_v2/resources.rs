use super::*;

const SHA256_STREAMING_WORKSPACE_BYTES_V2: usize = 104;
const RETAINED_ENDPOINT_DIGEST_BYTES_V2: usize = 2 * 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BoundaryResourceShapeV2 {
    pub(super) representation: BoundaryRepresentationV2,
    pub(super) hinge_count: usize,
    pub(super) ordinary_coefficient_count: usize,
    pub(super) half_angle_power_coefficient_count: usize,
    pub(super) retained_scan_visits: usize,
    pub(super) ordinary_max_coefficient_count: usize,
    pub(super) half_angle_max_coefficient_count: usize,
    pub(super) has_empty_coefficient_vector: bool,
}

pub(super) struct BoundaryWorkMeterV2 {
    charged: usize,
    maximum: usize,
}

impl BoundaryWorkMeterV2 {
    pub(super) const fn new(maximum: usize) -> Self {
        Self {
            charged: 0,
            maximum,
        }
    }

    pub(super) fn charge_v2(
        &mut self,
        amount: usize,
    ) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
        self.charged = self
            .charged
            .checked_add(amount)
            .filter(|charged| *charged <= self.maximum)
            .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
        Ok(())
    }

    pub(super) const fn charged_v2(&self) -> usize {
        self.charged
    }
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CycleScheduleClosedDyadicBoundaryStopV2::Cancelled => {
            CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled
        }
        CycleScheduleClosedDyadicBoundaryStopV2::DeadlineExceeded => {
            CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded
        }
    })
}

pub(super) fn checked_resource_bound_v2(
    schedule: &CanonicalCycleScheduleV1,
    limits: CycleScheduleLimitsV1,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<
    CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    CycleScheduleClosedDyadicBoundaryErrorV2,
> {
    checkpoint_v2(checkpoint)?;
    let dyadic = schedule
        .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(0, limits, || {
            checkpoint().map_err(map_stop_to_dyadic_v2)
        })
        .map_err(map_dyadic_error_v2)?;
    let schedule_deep_retained_bytes = schedule
        .checked_deep_retained_bytes_with_checkpoint_v2(usize::MAX, || {
            checkpoint().map_err(map_stop_to_dyadic_v2)
        })
        .map_err(map_dyadic_error_v2)?;
    let shape = checked_resource_shape_v2(schedule, checkpoint)?;
    finish_resource_bound_v2(
        shape,
        limits,
        schedule_deep_retained_bytes,
        dyadic.peak_bytes(),
        checkpoint,
    )
}

pub(super) fn checked_resource_bound_from_shape_v2(
    schedule: &CanonicalCycleScheduleV1,
    limits: CycleScheduleLimitsV1,
    maximum_schedule_deep_retained_bytes: usize,
    shape: BoundaryResourceShapeV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<
    CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    CycleScheduleClosedDyadicBoundaryErrorV2,
> {
    if shape.hinge_count == 0 || shape.hinge_count > limits.max_hinges {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    let schedule_deep_retained_bytes = schedule
        .checked_deep_retained_bytes_with_checkpoint_v2(
            maximum_schedule_deep_retained_bytes,
            || checkpoint().map_err(map_stop_to_dyadic_v2),
        )
        .map_err(map_dyadic_error_v2)?;
    let dyadic = schedule
        .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(0, limits, || {
            checkpoint().map_err(map_stop_to_dyadic_v2)
        })
        .map_err(map_dyadic_error_v2)?;
    finish_resource_bound_v2(
        shape,
        limits,
        schedule_deep_retained_bytes,
        dyadic.peak_bytes(),
        checkpoint,
    )
}

fn finish_resource_bound_v2(
    shape: BoundaryResourceShapeV2,
    limits: CycleScheduleLimitsV1,
    schedule_deep_retained_bytes: usize,
    dyadic_peak_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<
    CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    CycleScheduleClosedDyadicBoundaryErrorV2,
> {
    validate_shape_against_limits_v2(shape, limits)?;
    if shape.hinge_count == 0 || shape.hinge_count > limits.max_hinges {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    let workspace_peak_bytes = checked_boundary_workspace_peak_v2(dyadic_peak_bytes)
        .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    let scan_logical_work = checked_scan_logical_work_v2(shape)
        .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    let logical_work_required = checked_logical_work_required_v2(shape, limits)
        .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    checkpoint_v2(checkpoint)?;
    Ok(CycleScheduleClosedDyadicBoundaryResourceBoundV2 {
        hinge_count: shape.hinge_count,
        schedule_deep_retained_bytes,
        scan_logical_work,
        logical_work_required,
        workspace_peak_bytes,
    })
}

pub(super) fn checked_resource_shape_v2(
    schedule: &CanonicalCycleScheduleV1,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<BoundaryResourceShapeV2, CycleScheduleClosedDyadicBoundaryErrorV2> {
    checked_resource_shape_impl_v2(schedule, true, checkpoint)
}

pub(super) fn checked_resource_projection_shape_v2(
    schedule: &CanonicalCycleScheduleV1,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<BoundaryResourceShapeV2, CycleScheduleClosedDyadicBoundaryErrorV2> {
    checked_resource_shape_impl_v2(schedule, false, checkpoint)
}

fn checked_resource_shape_impl_v2(
    schedule: &CanonicalCycleScheduleV1,
    poll_coefficients: bool,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<BoundaryResourceShapeV2, CycleScheduleClosedDyadicBoundaryErrorV2> {
    checkpoint_v2(checkpoint)?;
    let (representation, hinge_count) = match (
        schedule.entries.is_empty(),
        schedule.half_angle_entries.is_empty(),
    ) {
        (false, true) => (BoundaryRepresentationV2::Ordinary, schedule.entries.len()),
        (true, false) => (
            BoundaryRepresentationV2::HalfAngle,
            schedule.half_angle_entries.len(),
        ),
        _ => return Err(CycleSchedulePrepareErrorV1::InvalidInput.into()),
    };
    let mut ordinary_coefficient_count = 0usize;
    let mut half_angle_power_coefficient_count = 0usize;
    let mut retained_scan_visits = 2usize;
    let mut ordinary_max_coefficient_count = 0usize;
    let mut half_angle_max_coefficient_count = 0usize;
    let mut has_empty_coefficient_vector = false;

    for entry in &schedule.entries {
        checkpoint_v2(checkpoint)?;
        retained_scan_visits = retained_scan_visits
            .checked_add(1)
            .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
        if poll_coefficients {
            for _ in &entry.coefficients {
                checkpoint_v2(checkpoint)?;
                ordinary_coefficient_count = ordinary_coefficient_count
                    .checked_add(1)
                    .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
                retained_scan_visits = retained_scan_visits
                    .checked_add(1)
                    .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
            }
        } else {
            ordinary_coefficient_count = ordinary_coefficient_count
                .checked_add(entry.coefficients.len())
                .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
            retained_scan_visits = retained_scan_visits
                .checked_add(entry.coefficients.len())
                .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
        }
        ordinary_max_coefficient_count =
            ordinary_max_coefficient_count.max(entry.coefficients.len());
        has_empty_coefficient_vector |= entry.coefficients.is_empty();
    }
    for entry in &schedule.half_angle_entries {
        checkpoint_v2(checkpoint)?;
        retained_scan_visits = retained_scan_visits
            .checked_add(3)
            .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
        for coefficients in [
            &entry.numerator_power_coefficients,
            &entry.denominator_power_coefficients,
            &entry.numerator_certificate.coefficients,
            &entry.denominator_certificate.coefficients,
        ] {
            half_angle_max_coefficient_count =
                half_angle_max_coefficient_count.max(coefficients.len());
            has_empty_coefficient_vector |= coefficients.is_empty();
            if poll_coefficients {
                for _ in coefficients {
                    checkpoint_v2(checkpoint)?;
                    retained_scan_visits = retained_scan_visits
                        .checked_add(1)
                        .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
                }
            } else {
                retained_scan_visits = retained_scan_visits
                    .checked_add(coefficients.len())
                    .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
            }
        }
        half_angle_power_coefficient_count = half_angle_power_coefficient_count
            .checked_add(entry.numerator_power_coefficients.len())
            .and_then(|count| count.checked_add(entry.denominator_power_coefficients.len()))
            .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    }
    checkpoint_v2(checkpoint)?;
    Ok(BoundaryResourceShapeV2 {
        representation,
        hinge_count,
        ordinary_coefficient_count,
        half_angle_power_coefficient_count,
        retained_scan_visits,
        ordinary_max_coefficient_count,
        half_angle_max_coefficient_count,
        has_empty_coefficient_vector,
    })
}

fn validate_shape_against_limits_v2(
    shape: BoundaryResourceShapeV2,
    limits: CycleScheduleLimitsV1,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    let degree_slots = limits
        .max_degree
        .checked_add(1)
        .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    let invalid_policy = limits.max_hinges == 0
        || limits.max_hinges == usize::MAX
        || limits.max_degree == usize::MAX
        || limits.max_work == 0
        || limits.max_work == usize::MAX
        || limits.max_coefficient_bits == u32::MAX;
    let invalid_shape = match shape.representation {
        BoundaryRepresentationV2::Ordinary => {
            shape.ordinary_max_coefficient_count > degree_slots
                || shape.ordinary_max_coefficient_count > limits.max_work
        }
        BoundaryRepresentationV2::HalfAngle => {
            shape.half_angle_max_coefficient_count > degree_slots
                || shape
                    .half_angle_max_coefficient_count
                    .checked_mul(shape.half_angle_max_coefficient_count)
                    .is_none_or(|work| work > limits.max_work)
        }
    };
    if invalid_policy || shape.has_empty_coefficient_vector || invalid_shape {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    Ok(())
}

pub(super) fn checked_logical_work_required_v2(
    shape: BoundaryResourceShapeV2,
    limits: CycleScheduleLimitsV1,
) -> Option<usize> {
    let scan_work = checked_scan_logical_work_v2(shape)?;
    let evaluation_work = match shape.representation {
        BoundaryRepresentationV2::Ordinary => shape
            .ordinary_coefficient_count
            .checked_add(shape.hinge_count)?
            .checked_mul(2)?,
        BoundaryRepresentationV2::HalfAngle => shape
            .half_angle_power_coefficient_count
            .checked_add(
                shape
                    .hinge_count
                    .checked_mul(limits.max_work.checked_add(1)?)?,
            )?
            .checked_mul(2)?,
    };
    scan_work
        .checked_add(evaluation_work)?
        .checked_add(
            binding::checked_endpoint_binding_work_v2(shape.representation, shape.hinge_count)?
                .checked_mul(2)?,
        )?
        .checked_add(binding::checked_evidence_binding_work_v2())
}

const fn checked_scan_logical_work_v2(shape: BoundaryResourceShapeV2) -> Option<usize> {
    shape.retained_scan_visits.checked_mul(3)
}

pub(super) fn checked_boundary_workspace_peak_v2(dyadic_peak_bytes: usize) -> Option<usize> {
    dyadic_peak_bytes
        .checked_add(SHA256_STREAMING_WORKSPACE_BYTES_V2)
        .and_then(|bytes| bytes.checked_add(RETAINED_ENDPOINT_DIGEST_BYTES_V2))
}

pub(super) fn checked_projected_boundary_workspace_peak_v2(
    shape: BoundaryResourceShapeV2,
    limits: CycleScheduleLimitsV1,
) -> Option<usize> {
    // This allocation-free projection mirrors
    // `dyadic_workspace_v2::workspace_bound`; the ordinary/half-angle parity
    // regression in `policy_tests` must be updated with any formula change.
    validate_shape_against_limits_v2(shape, limits).ok()?;
    let angle_box_bytes =
        std::mem::size_of::<(EdgeId, OutwardIntervalV1)>().checked_mul(shape.hinge_count)?;
    let dyadic_peak_bytes = match shape.representation {
        BoundaryRepresentationV2::Ordinary => angle_box_bytes,
        BoundaryRepresentationV2::HalfAngle => {
            let degree_slots = limits.max_degree.checked_add(1)?;
            let source_bits = usize::try_from(limits.max_coefficient_bits)
                .ok()?
                .checked_add(64)?
                .checked_add(128)?;
            let affine_factors = degree_slots.checked_mul(2)?.checked_add(3)?;
            let one_term_bits = source_bits.checked_mul(affine_factors)?.checked_add(256)?;
            let transient_bits = one_term_bits.checked_mul(limits.max_work.checked_add(1)?)?;
            let digit_bits = usize::try_from(usize::BITS).ok()?;
            let digits = transient_bits
                .checked_add(digit_bits.checked_sub(1)?)?
                .checked_div(digit_bits)?
                .max(1);
            let one_big_int_payload = digits
                .checked_mul(2)?
                .checked_mul(std::mem::size_of::<usize>())?;
            let live_rationals = degree_slots.checked_mul(4)?.checked_add(32)?;
            let live_big_ints = live_rationals.checked_mul(2)?.checked_add(32)?;
            let big_rational_payload_bytes = one_big_int_payload.checked_mul(live_big_ints)?;
            let exact_nonvector_object_bytes = std::mem::size_of::<num_rational::BigRational>()
                .checked_mul(32)?
                .checked_add(std::mem::size_of::<num_bigint::BigInt>().checked_mul(32)?)?;
            let exact_object_bytes = std::mem::size_of::<num_rational::BigRational>()
                .checked_mul(degree_slots.checked_mul(4)?)?
                .checked_add(exact_nonvector_object_bytes)?;
            angle_box_bytes
                .checked_add(big_rational_payload_bytes)?
                .checked_add(exact_object_bytes)?
        }
    };
    checked_boundary_workspace_peak_v2(dyadic_peak_bytes)
}

const fn map_stop_to_dyadic_v2(
    stop: CycleScheduleClosedDyadicBoundaryStopV2,
) -> CycleScheduleDyadicEvaluationStopV2 {
    match stop {
        CycleScheduleClosedDyadicBoundaryStopV2::Cancelled => {
            CycleScheduleDyadicEvaluationStopV2::Cancelled
        }
        CycleScheduleClosedDyadicBoundaryStopV2::DeadlineExceeded => {
            CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
        }
    }
}

const fn map_dyadic_error_v2(
    error: CycleScheduleDyadicEvaluationErrorV2,
) -> CycleScheduleClosedDyadicBoundaryErrorV2 {
    match error {
        CycleScheduleDyadicEvaluationErrorV2::Prepare(error) => {
            CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(error)
        }
        CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit => {
            CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit
        }
        CycleScheduleDyadicEvaluationErrorV2::Cancelled => {
            CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled
        }
        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded => {
            CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded
        }
    }
}
