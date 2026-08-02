use super::*;

fn clone_big_rational_vec_v2(
    source: &[BigRational],
    base_live_bytes: usize,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<(Vec<BigRational>, usize), CycleScheduleRestrictionWorkspaceErrorV2> {
    let logical_outer = std::mem::size_of::<BigRational>()
        .checked_mul(source.len())
        .and_then(|bytes| bytes.checked_add(base_live_bytes))
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    meter.observe_retained(logical_outer)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(source.len())
        .map_err(|_| CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    let physical_outer = std::mem::size_of::<BigRational>()
        .checked_mul(values.capacity())
        .and_then(|bytes| bytes.checked_add(base_live_bytes))
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    meter.observe_retained(physical_outer)?;
    let mut allocation_bytes = std::mem::size_of::<BigRational>()
        .checked_mul(values.capacity())
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    for value in source {
        checkpoint_v2(checkpoint)?;
        let next_allocation_bytes = allocation_bytes
            .checked_add(
                checked_big_rational_heap_bytes_upper_bound_v1(value)
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
            )
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        // Admit each representation-aware BigInt payload before cloning it.
        // This preserves the single scan while ensuring a one-short retained
        // or peak limit cannot be crossed by a partial clone.
        meter.observe_retained(
            base_live_bytes
                .checked_add(next_allocation_bytes)
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        )?;
        values.push(value.clone());
        allocation_bytes = next_allocation_bytes;
    }
    meter.observe_retained(
        base_live_bytes
            .checked_add(allocation_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
    )?;
    Ok((values, allocation_bytes))
}

pub(super) fn checked_half_angle_nested_retained_with_checkpoint_v2(
    entry: &PreparedHalfAngleRationalEntryV1,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<usize, CycleScheduleRestrictionWorkspaceErrorV2> {
    let mut total = 0usize;
    for value in &entry.u_domain {
        checkpoint_v2(checkpoint)?;
        meter.charge_work(1)?;
        total = total
            .checked_add(
                checked_big_rational_heap_bytes_upper_bound_v1(value)
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
            )
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    }
    for values in [
        &entry.numerator_power_coefficients,
        &entry.denominator_power_coefficients,
        &entry.numerator_certificate.coefficients,
        &entry.denominator_certificate.coefficients,
    ] {
        total = total
            .checked_add(
                std::mem::size_of::<BigRational>()
                    .checked_mul(values.capacity())
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
            )
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        for value in values {
            checkpoint_v2(checkpoint)?;
            meter.charge_work(1)?;
            total = total
                .checked_add(
                    checked_big_rational_heap_bytes_upper_bound_v1(value)
                        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
                )
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        }
    }
    Ok(total)
}

pub(super) fn clone_half_angle_entry_v2(
    source: &PreparedHalfAngleRationalEntryV1,
    base_live_bytes: usize,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<(PreparedHalfAngleRationalEntryV1, usize), CycleScheduleRestrictionWorkspaceErrorV2> {
    checkpoint_v2(checkpoint)?;
    let mut nested_bytes = 0usize;
    for value in &source.u_domain {
        checkpoint_v2(checkpoint)?;
        nested_bytes = nested_bytes
            .checked_add(
                checked_big_rational_heap_bytes_upper_bound_v1(value)
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
            )
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    }
    meter.observe_retained(
        base_live_bytes
            .checked_add(nested_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
    )?;
    let u_domain = source.u_domain.clone();

    let (numerator_power_coefficients, bytes) = clone_big_rational_vec_v2(
        &source.numerator_power_coefficients,
        base_live_bytes
            .checked_add(nested_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        meter,
        checkpoint,
    )?;
    nested_bytes = nested_bytes
        .checked_add(bytes)
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    let (denominator_power_coefficients, bytes) = clone_big_rational_vec_v2(
        &source.denominator_power_coefficients,
        base_live_bytes
            .checked_add(nested_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        meter,
        checkpoint,
    )?;
    nested_bytes = nested_bytes
        .checked_add(bytes)
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    let (numerator_coefficients, bytes) = clone_big_rational_vec_v2(
        &source.numerator_certificate.coefficients,
        base_live_bytes
            .checked_add(nested_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        meter,
        checkpoint,
    )?;
    nested_bytes = nested_bytes
        .checked_add(bytes)
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    let (denominator_coefficients, bytes) = clone_big_rational_vec_v2(
        &source.denominator_certificate.coefficients,
        base_live_bytes
            .checked_add(nested_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        meter,
        checkpoint,
    )?;
    nested_bytes = nested_bytes
        .checked_add(bytes)
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
    Ok((
        PreparedHalfAngleRationalEntryV1 {
            edge: source.edge,
            u_domain,
            numerator_power_coefficients,
            denominator_power_coefficients,
            numerator_certificate: PoleFreeBernsteinCertificateV1 {
                degree: source.numerator_certificate.degree,
                positive: source.numerator_certificate.positive,
                coefficients: numerator_coefficients,
            },
            denominator_certificate: PoleFreeBernsteinCertificateV1 {
                degree: source.denominator_certificate.degree,
                positive: source.denominator_certificate.positive,
                coefficients: denominator_coefficients,
            },
            derivative_bound_degrees_bits: source.derivative_bound_degrees_bits,
        },
        nested_bytes,
    ))
}
