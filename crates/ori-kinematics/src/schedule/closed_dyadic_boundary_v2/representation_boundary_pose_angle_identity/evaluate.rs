use super::*;

type ErrorV2 = CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2;
type StopV2 = CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2;
type EvidenceV2 = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;
type InputV2<'a> = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'a>;

pub(super) fn issue_v2(
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<EvidenceV2, ErrorV2> {
    derive_v2(input, checkpoint)
}

pub(super) fn revalidate_v2(
    evidence: &EvidenceV2,
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    pose_resources::checkpoint_v2(checkpoint)?;
    replay_preflight_v2(evidence, input)?;
    let candidate = derive_v2(input, checkpoint)?;
    pose_resources::checkpoint_v2(checkpoint)?;
    if evidence.fixed_face != candidate.fixed_face
        || evidence.schedule_binding_fingerprint != candidate.schedule_binding_fingerprint
        || evidence.graph_binding_fingerprint != candidate.graph_binding_fingerprint
        || evidence.closed_boundary_evidence_binding_fingerprint
            != candidate.closed_boundary_evidence_binding_fingerprint
        || evidence.schedule_limits != candidate.schedule_limits
        || evidence.resources != candidate.resources
        || evidence.limits != candidate.limits
        || evidence.binding_fingerprint != candidate.binding_fingerprint
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    pose_resources::checkpoint_v2(checkpoint)
}

fn derive_v2(
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<EvidenceV2, ErrorV2> {
    pose_resources::checkpoint_v2(checkpoint)?;
    preflight_v2(input)?;
    let bound = pose_resources::checked_resource_bound_for_limits_v2(
        input.schedule,
        input.geometry,
        input.audit,
        input.lower_pose,
        input.upper_pose,
        input.closed_boundary_evidence,
        input.schedule_limits,
        input.limits,
        checkpoint,
    )?;
    pose_resources::poll_pose_retained_v2(input.lower_pose, checkpoint)?;
    pose_resources::poll_pose_retained_v2(input.upper_pose, checkpoint)?;
    let pose_work = bound
        .logical_work_required
        .checked_sub(bound.closed_boundary_bound.logical_work_required_v2())
        .ok_or(ErrorV2::ResourceLimit)?;
    let mut meter = resources::BoundaryWorkMeterV2::new(pose_work);
    charge_v2(&mut meter, bound.pose_retained_scan_work)?;
    charge_v2(&mut meter, bound.graph_binding_work)?;
    validate_live_join_v2(input, checkpoint)?;
    validate_closed_boundary_header_v2(input, bound)?;
    charge_v2(&mut meter, pose_resources::POSE_IDENTITY_FIXED_WORK_V2)?;

    let evaluated = evaluate::evaluate_boundaries_v2(
        input.schedule,
        input.schedule_limits,
        bound.closed_boundary_bound,
        bound.closed_boundary_bound.logical_work_required_v2(),
        &mut || checkpoint().map_err(pose_resources::map_stop_to_closed_v2),
    )
    .map_err(pose_resources::map_closed_error_v2)?;
    validate_closed_boundary_evidence_v2(input, bound, &evaluated)?;
    evaluate_pose_identity_v2(input, evaluated.representation, &mut meter, checkpoint)?;

    let resources = RepresentationBoundaryPoseAngleIdentityResourcesV2 {
        hinge_count: bound.hinge_count_v2(),
        // Retain the replay policy cap, not allocator-dependent capacity.
        // The actual schedule footprint was already validated `<=` this cap.
        schedule_deep_retained_bytes_cap: input.limits.max_schedule_deep_retained_bytes,
        representation_boundary_poses_deep_retained_bytes: bound
            .representation_boundary_poses_deep_retained_bytes_v2(),
        logical_work: bound.logical_work_required_v2(),
        workspace_peak_bytes: bound.workspace_peak_bytes_upper_bound_v2(),
    };
    charge_v2(&mut meter, binding::checked_binding_work_v2())?;
    let binding_fingerprint = binding::binding_fingerprint_v2(
        evaluated.representation,
        input.lower_pose.fixed_face(),
        input.schedule.certificate_binding_fingerprint_v2(),
        input.schedule.graph_binding_fingerprint_v1(),
        input.closed_boundary_evidence.binding_fingerprint_v2(),
        input.schedule_limits,
        resources,
        input.limits,
    )?;
    if meter.charged_v2() != pose_work {
        return Err(ErrorV2::ResourceLimit);
    }
    pose_resources::checkpoint_v2(checkpoint)?;
    Ok(EvidenceV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        lower_pose_instance: input.lower_pose.instance_anchor_v2(),
        upper_pose_instance: input.upper_pose.instance_anchor_v2(),
        fixed_face: input.lower_pose.fixed_face(),
        schedule_binding_fingerprint: input.schedule.certificate_binding_fingerprint_v2(),
        graph_binding_fingerprint: input.schedule.graph_binding_fingerprint_v1(),
        closed_boundary_evidence_binding_fingerprint: input
            .closed_boundary_evidence
            .binding_fingerprint_v2(),
        schedule_limits: input.schedule_limits,
        resources,
        limits: input.limits,
        binding_fingerprint,
    })
}

fn replay_preflight_v2(evidence: &EvidenceV2, input: InputV2<'_>) -> Result<(), ErrorV2> {
    if evidence.limits != input.limits
        || evidence.schedule_limits != input.schedule_limits
        || !evidence.issuer_geometry.matches(input.geometry)
        || !Arc::ptr_eq(
            &evidence.lower_pose_instance,
            &input.lower_pose.instance_anchor_v2(),
        )
        || !Arc::ptr_eq(
            &evidence.upper_pose_instance,
            &input.upper_pose.instance_anchor_v2(),
        )
        || evidence.fixed_face != input.lower_pose.fixed_face()
        || evidence.fixed_face != input.upper_pose.fixed_face()
        || evidence.fixed_face != input.schedule.fixed_face
        || evidence.schedule_binding_fingerprint
            != input.schedule.certificate_binding_fingerprint_v2()
        || evidence.graph_binding_fingerprint != input.schedule.graph_binding_fingerprint_v1()
        || evidence.closed_boundary_evidence_binding_fingerprint
            != input.closed_boundary_evidence.binding_fingerprint_v2()
        || evidence.resources.hinge_count > input.limits.max_hinges
        || evidence.resources.schedule_deep_retained_bytes_cap
            != input.limits.max_schedule_deep_retained_bytes
        || evidence
            .resources
            .representation_boundary_poses_deep_retained_bytes
            > input
                .limits
                .max_representation_boundary_poses_deep_retained_bytes
        || evidence.resources.logical_work != input.limits.max_logical_work
        || evidence.resources.workspace_peak_bytes != input.limits.max_workspace_bytes
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    Ok(())
}

fn preflight_v2(input: InputV2<'_>) -> Result<(), ErrorV2> {
    let schedule_hinge_count = match (
        input.schedule.entries.is_empty(),
        input.schedule.half_angle_entries.is_empty(),
    ) {
        (false, true) => input.schedule.entries.len(),
        (true, false) => input.schedule.half_angle_entries.len(),
        _ => return Err(ErrorV2::ScheduleBindingMismatch),
    };
    if pose_resources::limit_values_v2(input.limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || input.geometry.hinges().is_empty()
        || input.geometry.hinges().len() > input.limits.max_hinges
        || schedule_hinge_count == 0
        || schedule_hinge_count > input.limits.max_hinges
        || input.schedule_limits.max_hinges == 0
        || input.schedule_limits.max_hinges == usize::MAX
        || input.schedule_limits.max_degree == usize::MAX
        || input.schedule_limits.max_coefficient_bits == u32::MAX
        || input.schedule_limits.max_work == 0
        || input.schedule_limits.max_work == usize::MAX
    {
        return Err(ErrorV2::ResourceLimit);
    }
    if input.closed_boundary_evidence.schedule_binding_fingerprint
        != input.schedule.schedule_fingerprint_v2
        || input.closed_boundary_evidence.graph_binding_fingerprint
            != input.schedule.binding_fingerprint
        || input.closed_boundary_evidence.canonical_boundary_count_v2() != 2
    {
        return Err(ErrorV2::ClosedBoundaryEvidenceMismatch);
    }
    if !input.lower_pose.is_for_geometry(input.geometry)
        || !input.upper_pose.is_for_geometry(input.geometry)
        || input.lower_pose.fixed_face() != input.upper_pose.fixed_face()
    {
        return Err(ErrorV2::BoundaryPoseMismatch);
    }
    if input.lower_pose.fixed_face() != input.schedule.fixed_face {
        return Err(ErrorV2::ScheduleBindingMismatch);
    }
    Ok(())
}

pub(super) fn validate_live_join_v2(
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    if !input.lower_pose.is_for_geometry(input.geometry)
        || !input.upper_pose.is_for_geometry(input.geometry)
        || input.lower_pose.fixed_face() != input.upper_pose.fixed_face()
        || input.audit.faces().len() != input.geometry.face_ids().len()
    {
        return Err(ErrorV2::BoundaryPoseMismatch);
    }
    for (audited_face, geometry_face) in input.audit.faces().iter().zip(input.geometry.face_ids()) {
        pose_resources::checkpoint_v2(checkpoint)?;
        if audited_face != geometry_face {
            return Err(ErrorV2::BoundaryPoseMismatch);
        }
    }
    let matches = input
        .schedule
        .matches_binding_with_checkpoint_v2(
            input.geometry,
            input.audit,
            input.lower_pose.fixed_face(),
            checkpoint,
        )
        .map_err(map_stop_v2)?;
    if !matches {
        return Err(ErrorV2::ScheduleBindingMismatch);
    }
    Ok(())
}

fn validate_closed_boundary_evidence_v2(
    input: InputV2<'_>,
    bound: CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
    evaluated: &evaluate::EvaluatedBoundariesV2,
) -> Result<(), ErrorV2> {
    let evidence = input.closed_boundary_evidence;
    let aggregate = super::super::binding::evidence_binding_fingerprint_v2(
        evaluated.representation,
        input.schedule.schedule_fingerprint_v2,
        input.schedule.binding_fingerprint,
        evaluated.lower_binding,
        evaluated.upper_binding,
        evaluated.hinge_count,
        input.schedule_limits,
        bound.closed_boundary_bound.logical_work_required_v2(),
        bound
            .closed_boundary_bound
            .workspace_peak_bytes_upper_bound_v2(),
    )
    .map_err(pose_resources::map_closed_error_v2)?;
    if evidence.schedule_binding_fingerprint != input.schedule.schedule_fingerprint_v2
        || evidence.graph_binding_fingerprint != input.schedule.binding_fingerprint
        || evidence.lower_boundary_binding_fingerprint != evaluated.lower_binding
        || evidence.upper_boundary_binding_fingerprint != evaluated.upper_binding
        || evidence.binding_fingerprint != aggregate
        || evidence.hinge_count != evaluated.hinge_count
        || evidence.logical_work != bound.closed_boundary_bound.logical_work_required_v2()
        || evidence.workspace_peak_bytes
            != bound
                .closed_boundary_bound
                .workspace_peak_bytes_upper_bound_v2()
    {
        return Err(ErrorV2::ClosedBoundaryEvidenceMismatch);
    }
    Ok(())
}

fn validate_closed_boundary_header_v2(
    input: InputV2<'_>,
    bound: CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
) -> Result<(), ErrorV2> {
    let evidence = input.closed_boundary_evidence;
    if evidence.schedule_binding_fingerprint != input.schedule.schedule_fingerprint_v2
        || evidence.graph_binding_fingerprint != input.schedule.binding_fingerprint
        || evidence.canonical_boundary_count_v2() != 2
        || evidence.hinge_count != bound.hinge_count_v2()
        || evidence.logical_work != bound.closed_boundary_bound.logical_work_required_v2()
        || evidence.workspace_peak_bytes
            != bound
                .closed_boundary_bound
                .workspace_peak_bytes_upper_bound_v2()
    {
        return Err(ErrorV2::ClosedBoundaryEvidenceMismatch);
    }
    Ok(())
}

fn evaluate_pose_identity_v2(
    input: InputV2<'_>,
    representation: BoundaryRepresentationV2,
    meter: &mut resources::BoundaryWorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    for (upper, pose) in [(false, input.lower_pose), (true, input.upper_pose)] {
        match representation {
            BoundaryRepresentationV2::Ordinary => {
                let x = if upper { 1.0 } else { -1.0 };
                for (entry, posed) in input
                    .schedule
                    .entries
                    .iter()
                    .zip(pose.hinge_angles().as_slice())
                {
                    pose_resources::checkpoint_v2(checkpoint)?;
                    let expected =
                        evaluate::evaluate_ordinary_endpoint_angle_v2(entry, x, meter, &mut || {
                            checkpoint().map_err(pose_resources::map_stop_to_closed_v2)
                        })
                        .map_err(pose_resources::map_closed_error_v2)?;
                    charge_v2(meter, 1)?;
                    if expected.edge() != posed.edge()
                        || expected.angle_degrees().to_bits() != posed.angle_degrees().to_bits()
                    {
                        return Err(ErrorV2::BoundaryPoseMismatch);
                    }
                }
            }
            BoundaryRepresentationV2::HalfAngle => {
                for (entry, posed) in input
                    .schedule
                    .half_angle_entries
                    .iter()
                    .zip(pose.hinge_angles().as_slice())
                {
                    pose_resources::checkpoint_v2(checkpoint)?;
                    let expected = evaluate_half_angle_point_v2(entry, upper, meter, checkpoint)?;
                    let enclosure = evaluate::evaluate_half_angle_endpoint_box_v2(
                        entry,
                        upper,
                        input.schedule_limits,
                        meter,
                        &mut || checkpoint().map_err(pose_resources::map_stop_to_closed_v2),
                    )
                    .map_err(pose_resources::map_closed_error_v2)?;
                    charge_v2(meter, 1)?;
                    let angle = expected.angle_degrees();
                    if expected.edge() != posed.edge()
                        || angle.to_bits() != posed.angle_degrees().to_bits()
                        || angle < enclosure.lower()
                        || angle > enclosure.upper()
                    {
                        return Err(ErrorV2::BoundaryPoseMismatch);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn evaluate_half_angle_point_v2(
    entry: &PreparedHalfAngleRationalEntryV1,
    upper_endpoint: bool,
    meter: &mut resources::BoundaryWorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<HingeAngle, ErrorV2> {
    charge_v2(meter, 1)?;
    pose_resources::checkpoint_v2(checkpoint)?;
    let lower = entry.u_domain[0]
        .to_f64()
        .ok_or(ErrorV2::BoundaryPoseMismatch)?;
    charge_v2(meter, 1)?;
    pose_resources::checkpoint_v2(checkpoint)?;
    let upper = entry.u_domain[1]
        .to_f64()
        .ok_or(ErrorV2::BoundaryPoseMismatch)?;
    let parameter = f64::from(upper_endpoint);
    let u = lower + (upper - lower) * parameter;
    let mut evaluate = |coefficients: &[BigRational]| -> Result<f64, ErrorV2> {
        let mut value = 0.0_f64;
        for coefficient in coefficients.iter().rev() {
            pose_resources::checkpoint_v2(checkpoint)?;
            charge_v2(meter, 1)?;
            value = value * u + coefficient.to_f64().ok_or(ErrorV2::BoundaryPoseMismatch)?;
        }
        Ok(value)
    };
    let numerator = evaluate(&entry.numerator_power_coefficients)?;
    let denominator = evaluate(&entry.denominator_power_coefficients)?;
    charge_v2(meter, 1)?;
    let angle = deterministic_half_angle_ratio_degrees_v1(numerator, denominator)
        .ok_or(ErrorV2::BoundaryPoseMismatch)?;
    HingeAngle::new(entry.edge(), angle).map_err(|_| ErrorV2::BoundaryPoseMismatch)
}

fn charge_v2(meter: &mut resources::BoundaryWorkMeterV2, amount: usize) -> Result<(), ErrorV2> {
    meter
        .charge_v2(amount)
        .map_err(pose_resources::map_closed_error_v2)
}

const fn map_stop_v2(stop: StopV2) -> ErrorV2 {
    match stop {
        StopV2::Cancelled => ErrorV2::Cancelled,
        StopV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
    }
}
