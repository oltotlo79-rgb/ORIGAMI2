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
    if shape.hinge_count == 0 || shape.hinge_count > limits.max_hinges {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    let workspace_peak_bytes = checked_boundary_workspace_peak_v2(dyadic.peak_bytes())
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

fn checked_resource_shape_v2(
    schedule: &CanonicalCycleScheduleV1,
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

    for entry in &schedule.entries {
        checkpoint_v2(checkpoint)?;
        retained_scan_visits = retained_scan_visits
            .checked_add(1)
            .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
        for _ in &entry.coefficients {
            checkpoint_v2(checkpoint)?;
            ordinary_coefficient_count = ordinary_coefficient_count
                .checked_add(1)
                .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
            retained_scan_visits = retained_scan_visits
                .checked_add(1)
                .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
        }
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
            for _ in coefficients {
                checkpoint_v2(checkpoint)?;
                retained_scan_visits = retained_scan_visits
                    .checked_add(1)
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
    })
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
