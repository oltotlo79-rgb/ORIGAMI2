use super::*;

pub(super) struct EvaluatedBoundariesV2 {
    pub(super) representation: BoundaryRepresentationV2,
    pub(super) lower_binding: [u8; 32],
    pub(super) upper_binding: [u8; 32],
    pub(super) hinge_count: usize,
}

pub(super) fn evaluate_boundaries_v2(
    schedule: &CanonicalCycleScheduleV1,
    limits: CycleScheduleLimitsV1,
    bound: CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    max_logical_work: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<EvaluatedBoundariesV2, CycleScheduleClosedDyadicBoundaryErrorV2> {
    let (representation, hinge_count) = representation_and_count_v2(schedule)?;
    if hinge_count != bound.hinge_count {
        return Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit);
    }
    let mut meter = resources::BoundaryWorkMeterV2::new(max_logical_work);
    meter.charge_v2(bound.scan_logical_work)?;
    let lower_binding = evaluate_endpoint_v2(
        schedule,
        representation,
        false,
        limits,
        &mut meter,
        checkpoint,
    )?;
    let upper_binding = evaluate_endpoint_v2(
        schedule,
        representation,
        true,
        limits,
        &mut meter,
        checkpoint,
    )?;
    meter.charge_v2(binding::checked_evidence_binding_work_v2())?;
    if meter.charged_v2() != bound.logical_work_required {
        return Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit);
    }
    Ok(EvaluatedBoundariesV2 {
        representation,
        lower_binding,
        upper_binding,
        hinge_count,
    })
}

fn evaluate_endpoint_v2(
    schedule: &CanonicalCycleScheduleV1,
    representation: BoundaryRepresentationV2,
    upper: bool,
    limits: CycleScheduleLimitsV1,
    meter: &mut resources::BoundaryWorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<[u8; 32], CycleScheduleClosedDyadicBoundaryErrorV2> {
    resources::checkpoint_v2(checkpoint)?;
    let hinge_count = match representation {
        BoundaryRepresentationV2::Ordinary => schedule.entries.len(),
        BoundaryRepresentationV2::HalfAngle => schedule.half_angle_entries.len(),
    };
    let mut hasher = binding::EndpointBindingHasherV2::new_v2(
        representation,
        upper,
        schedule.schedule_fingerprint_v2,
        schedule.binding_fingerprint,
        hinge_count,
        limits,
        meter,
    )?;
    let mut previous_edge_bytes = None;
    match representation {
        BoundaryRepresentationV2::Ordinary => {
            let x = if upper { 1.0 } else { -1.0 };
            for entry in &schedule.entries {
                resources::checkpoint_v2(checkpoint)?;
                validate_canonical_edge_v2(&mut previous_edge_bytes, entry.edge)?;
                let angle = evaluate_ordinary_endpoint_angle_v2(entry, x, meter, checkpoint)?;
                hasher.update_ordinary_v2(entry.edge, angle.angle_degrees().to_bits(), meter)?;
            }
        }
        BoundaryRepresentationV2::HalfAngle => {
            for entry in &schedule.half_angle_entries {
                resources::checkpoint_v2(checkpoint)?;
                validate_canonical_edge_v2(&mut previous_edge_bytes, entry.edge())?;
                let angle =
                    evaluate_half_angle_endpoint_box_v2(entry, upper, limits, meter, checkpoint)?;
                hasher.update_half_angle_v2(
                    entry.edge(),
                    angle.lower().to_bits(),
                    angle.upper().to_bits(),
                    meter,
                )?;
            }
        }
    }
    resources::checkpoint_v2(checkpoint)?;
    Ok(hasher.finalize_v2())
}

fn representation_and_count_v2(
    schedule: &CanonicalCycleScheduleV1,
) -> Result<(BoundaryRepresentationV2, usize), CycleScheduleClosedDyadicBoundaryErrorV2> {
    match (
        schedule.entries.is_empty(),
        schedule.half_angle_entries.is_empty(),
    ) {
        (false, true) => Ok((BoundaryRepresentationV2::Ordinary, schedule.entries.len())),
        (true, false) => Ok((
            BoundaryRepresentationV2::HalfAngle,
            schedule.half_angle_entries.len(),
        )),
        _ => Err(CycleSchedulePrepareErrorV1::InvalidInput.into()),
    }
}

pub(super) fn evaluate_ordinary_endpoint_angle_v2(
    entry: &Entry,
    x: f64,
    meter: &mut resources::BoundaryWorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<HingeAngle, CycleScheduleClosedDyadicBoundaryErrorV2> {
    if x != -1.0 && x != 1.0 {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    let mut b1 = 0.0;
    let mut b2 = 0.0;
    for coefficient in entry.coefficients.iter().rev() {
        resources::checkpoint_v2(checkpoint)?;
        meter.charge_v2(1)?;
        let b0 = 2.0 * x * b1 - b2 + coefficient;
        b2 = b1;
        b1 = b0;
    }
    meter.charge_v2(1)?;
    let angle = entry.initial + b1 - x * b2;
    HingeAngle::new(entry.edge, angle).map_err(map_hinge_angle_error_v2)
}

pub(super) fn evaluate_half_angle_endpoint_box_v2(
    entry: &PreparedHalfAngleRationalEntryV1,
    upper: bool,
    limits: CycleScheduleLimitsV1,
    meter: &mut resources::BoundaryWorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<OutwardIntervalV1, CycleScheduleClosedDyadicBoundaryErrorV2> {
    meter.charge_v2(1)?;
    let u = &entry.u_domain[usize::from(upper)];
    let numerator = evaluate_exact_power_horner_with_checkpoint_v2(
        &entry.numerator_power_coefficients,
        u,
        limits,
        meter,
        checkpoint,
    )?;
    let denominator = evaluate_exact_power_horner_with_checkpoint_v2(
        &entry.denominator_power_coefficients,
        u,
        limits,
        meter,
        checkpoint,
    )?;
    if numerator.is_zero() && denominator.is_zero() {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    let certificate = |value: BigRational| -> Result<
        PoleFreeBernsteinCertificateV1,
        CycleSchedulePrepareErrorV1,
    > {
        let positive = !value.is_negative();
        let mut coefficients = try_schedule_vec_with_capacity_v1(1)?;
        coefficients.push(value);
        Ok(PoleFreeBernsteinCertificateV1 {
            degree: 0,
            positive,
            coefficients,
        })
    };
    meter.charge_v2(limits.max_work)?;
    resources::checkpoint_v2(checkpoint)?;
    let enclosure = evaluate_half_angle_rational_degrees_interval_v1(
        &certificate(numerator)?,
        &certificate(denominator)?,
        limits.max_work,
    )?;
    resources::checkpoint_v2(checkpoint)?;
    Ok(enclosure)
}

fn evaluate_exact_power_horner_with_checkpoint_v2(
    coefficients: &[BigRational],
    u: &BigRational,
    limits: CycleScheduleLimitsV1,
    meter: &mut resources::BoundaryWorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
) -> Result<BigRational, CycleScheduleClosedDyadicBoundaryErrorV2> {
    if coefficients.is_empty() || coefficients.len() > limits.max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    let mut value = BigRational::zero();
    for coefficient in coefficients.iter().rev() {
        resources::checkpoint_v2(checkpoint)?;
        meter.charge_v2(1)?;
        value = value * u + coefficient;
    }
    validate_exact_bits(core::slice::from_ref(&value), limits.max_coefficient_bits)?;
    Ok(value)
}

fn validate_canonical_edge_v2(
    previous: &mut Option<[u8; 16]>,
    edge: EdgeId,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    let current = edge.canonical_bytes();
    if previous.is_some_and(|previous| previous >= current) {
        return Err(CycleSchedulePrepareErrorV1::NonCanonical.into());
    }
    *previous = Some(current);
    Ok(())
}

const fn map_hinge_angle_error_v2(
    error: KinematicsError,
) -> CycleScheduleClosedDyadicBoundaryErrorV2 {
    match error {
        KinematicsError::HingeAngleOutOfRange { .. } => {
            CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(
                CycleSchedulePrepareErrorV1::AngleRange,
            )
        }
        _ => CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::InvalidInput,
        ),
    }
}
