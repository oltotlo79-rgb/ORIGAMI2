//! Exact schedule/instance joins, delegated replay, resources, and binding.

use std::mem::size_of;

use ori_kinematics::CycleScheduleClosedDyadicBoundaryResourceBoundV2;
use sha2::{Digest, Sha256};

use super::*;

#[path = "validation_error_mapping.rs"]
mod error_mapping;

use error_mapping::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2;
type StopV2 = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2;
type LimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2;
type CertificateV2 = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;

pub(super) fn issue_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<CertificateV2, ErrorV2> {
    checkpoint_v2(checkpoint)?;
    preflight_static_v2(&input.endpoint_prerequisite, input.limits)?;
    validate_issue_join_v2(&input.endpoint_prerequisite, input.geometry, input.schedule)?;
    let bound =
        checked_boundary_resource_bound_v2(input.schedule, input.schedule_limits, checkpoint)?;
    validate_bound_v2(bound, input.limits)?;
    let evidence = prove_boundary_evidence_v2(
        input.schedule,
        input.schedule_limits,
        input.limits,
        checkpoint,
    )?;
    validate_evidence_v2(
        &input.endpoint_prerequisite,
        input.schedule,
        bound,
        &evidence,
    )?;
    let resources = checked_resources_v2(&input.endpoint_prerequisite, bound, input.limits)?;
    let binding_fingerprint = binding_fingerprint_v2(
        &input.endpoint_prerequisite,
        &evidence,
        input.schedule_limits,
        resources,
        input.limits,
    )?;
    checkpoint_v2(checkpoint)?;
    Ok(CertificateV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        endpoint_prerequisite: input.endpoint_prerequisite,
        boundary_evidence: evidence,
        schedule_limits: input.schedule_limits,
        resources,
        limits: input.limits,
        binding_fingerprint,
    })
}

pub(super) fn revalidate_v2(
    certificate: &CertificateV2,
    input: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    checkpoint_v2(checkpoint)?;
    preflight_static_v2(&certificate.endpoint_prerequisite, input.limits)?;
    preflight_retained_resources_v2(certificate, input.limits)?;
    if !limits_match_v2(certificate.limits, input.limits)
        || certificate.schedule_limits != input.schedule_limits
        || !certificate
            .endpoint_prerequisite
            .replay_limits_match_v2(input.endpoint_replay.limits)
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    validate_replay_join_v2(certificate, &input)?;
    let bound =
        checked_boundary_resource_bound_v2(input.schedule, input.schedule_limits, checkpoint)?;
    validate_bound_v2(bound, input.limits)?;
    let resources = checked_resources_v2(&certificate.endpoint_prerequisite, bound, input.limits)?;
    if certificate.resources != resources {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    revalidate_endpoint_v2(
        &certificate.endpoint_prerequisite,
        input.endpoint_replay,
        checkpoint,
    )?;
    let candidate = prove_boundary_evidence_v2(
        input.schedule,
        input.schedule_limits,
        input.limits,
        checkpoint,
    )?;
    validate_evidence_v2(
        &certificate.endpoint_prerequisite,
        input.schedule,
        bound,
        &candidate,
    )?;
    let binding_fingerprint = binding_fingerprint_v2(
        &certificate.endpoint_prerequisite,
        &candidate,
        input.schedule_limits,
        resources,
        input.limits,
    )?;
    checkpoint_v2(checkpoint)?;
    if !evidence_matches_v2(&certificate.boundary_evidence, &candidate)
        || certificate.binding_fingerprint != binding_fingerprint
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    checkpoint_v2(checkpoint)
}

fn preflight_retained_resources_v2(
    certificate: &CertificateV2,
    limits: LimitsV2,
) -> Result<(), ErrorV2> {
    if certificate.boundary_evidence.hinge_count_v2() > limits.max_hinges
        || certificate
            .resources
            .schedule_deep_retained_bytes_upper_bound
            > limits.max_schedule_deep_retained_bytes
        || certificate.resources.boundary_evidence_logical_work
            > limits.max_boundary_evidence_logical_work
        || certificate.resources.boundary_evidence_workspace_bytes
            > limits.max_boundary_evidence_workspace_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

fn preflight_static_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    limits: LimitsV2,
) -> Result<(), ErrorV2> {
    if limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
        || endpoint.actual_block_count_v2() < GENERAL_N_MIN_BLOCKS_V2
        || endpoint.actual_block_count_v2() > limits.max_blocks
        || size_of::<
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
        >() > limits.max_retained_endpoint_prerequisite_bytes
        || size_of::<CertificateV2>() > limits.max_publication_bytes
        || checked_declared_aggregate_peak_v2(endpoint, limits)? > limits.max_aggregate_peak_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

fn validate_issue_join_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    geometry: &MaterialHingeGraphGeometry,
    schedule: &CanonicalCycleScheduleV1,
) -> Result<(), ErrorV2> {
    if !endpoint.matches_geometry_instance_v2(geometry)
        || !schedule_graph_binding_pair_matches_v2(
            endpoint.schedule_binding_fingerprint_v2(),
            endpoint.graph_binding_fingerprint_v1(),
            schedule.certificate_binding_fingerprint_v2(),
            schedule.graph_binding_fingerprint_v1(),
        )
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    Ok(())
}

fn validate_replay_join_v2(
    certificate: &CertificateV2,
    input: &CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
) -> Result<(), ErrorV2> {
    let replay_geometry = input.endpoint_replay.coverage_replay.live.geometry;
    let replay_schedule = input.endpoint_replay.coverage_replay.live.parent_schedule;
    let retained_schedule = certificate
        .boundary_evidence
        .schedule_binding_fingerprint_v2();
    let retained_graph = certificate
        .endpoint_prerequisite
        .graph_binding_fingerprint_v1();
    if !certificate.issuer_geometry.matches(input.geometry)
        || !certificate.issuer_geometry.matches(replay_geometry)
        || !certificate
            .endpoint_prerequisite
            .matches_geometry_instance_v2(input.geometry)
        || !certificate
            .endpoint_prerequisite
            .matches_geometry_instance_v2(replay_geometry)
        || certificate
            .endpoint_prerequisite
            .schedule_binding_fingerprint_v2()
            != retained_schedule
        || !schedule_graph_binding_pair_matches_v2(
            retained_schedule,
            retained_graph,
            input.schedule.certificate_binding_fingerprint_v2(),
            input.schedule.graph_binding_fingerprint_v1(),
        )
        || !schedule_graph_binding_pair_matches_v2(
            retained_schedule,
            retained_graph,
            replay_schedule.certificate_binding_fingerprint_v2(),
            replay_schedule.graph_binding_fingerprint_v1(),
        )
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    Ok(())
}

fn checked_boundary_resource_bound_v2(
    schedule: &CanonicalCycleScheduleV1,
    schedule_limits: CycleScheduleLimitsV1,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<CycleScheduleClosedDyadicBoundaryResourceBoundV2, ErrorV2> {
    schedule
        .checked_closed_dyadic_boundary_resource_bound_with_checkpoint_v2(schedule_limits, || {
            checkpoint().map_err(map_stop_to_boundary_v2)
        })
        .map_err(map_boundary_error_v2)
}

fn prove_boundary_evidence_v2(
    schedule: &CanonicalCycleScheduleV1,
    schedule_limits: CycleScheduleLimitsV1,
    limits: LimitsV2,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2, ErrorV2> {
    schedule
        .prove_closed_dyadic_boundary_evidence_with_checkpoint_v2(
            schedule_limits,
            limits.max_boundary_evidence_logical_work,
            limits.max_boundary_evidence_workspace_bytes,
            || checkpoint().map_err(map_stop_to_boundary_v2),
        )
        .map_err(map_boundary_error_v2)
}

fn validate_bound_v2(
    bound: CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    limits: LimitsV2,
) -> Result<(), ErrorV2> {
    if bound.hinge_count_v2() == 0
        || bound.hinge_count_v2() > limits.max_hinges
        || bound.schedule_deep_retained_bytes_v2() > limits.max_schedule_deep_retained_bytes
        || bound.logical_work_required_v2() != limits.max_boundary_evidence_logical_work
        || bound.workspace_peak_bytes_upper_bound_v2()
            != limits.max_boundary_evidence_workspace_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

fn validate_evidence_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    schedule: &CanonicalCycleScheduleV1,
    bound: CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    evidence: &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
) -> Result<(), ErrorV2> {
    let schedule_binding = schedule.certificate_binding_fingerprint_v2();
    if endpoint.schedule_binding_fingerprint_v2() != schedule_binding
        || evidence.schedule_binding_fingerprint_v2() != schedule_binding
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    if evidence.canonical_boundary_count_v2() != 2
        || evidence.hinge_count_v2() != bound.hinge_count_v2()
        || evidence.logical_work_v2() != bound.logical_work_required_v2()
        || evidence.workspace_peak_bytes_upper_bound_v2()
            != bound.workspace_peak_bytes_upper_bound_v2()
    {
        return Err(ErrorV2::BoundaryConfigurationUnavailable);
    }
    Ok(())
}

fn checked_resources_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    bound: CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    limits: LimitsV2,
) -> Result<BoundaryConfigurationResourcesV2, ErrorV2> {
    let resources = BoundaryConfigurationResourcesV2 {
        retained_endpoint_prerequisite_bytes: size_of::<
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
        >(),
        // Charge every replay candidate up to the exact retained outer cap;
        // semantically equal schedules may have different vector capacities.
        schedule_deep_retained_bytes_upper_bound: limits.max_schedule_deep_retained_bytes,
        boundary_evidence_bytes: size_of::<CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2>(),
        boundary_evidence_logical_work: bound.logical_work_required_v2(),
        boundary_evidence_workspace_bytes: bound.workspace_peak_bytes_upper_bound_v2(),
        delegated_endpoint_replay_peak_bytes: endpoint.replay_aggregate_peak_cap_v2(),
        composition_workspace_bytes: COMPOSITION_WORKSPACE_BYTES_V2,
        publication_bytes: size_of::<CertificateV2>(),
        aggregate_peak_bytes: checked_declared_aggregate_peak_v2(endpoint, limits)?,
    };
    if resources.retained_endpoint_prerequisite_bytes
        > limits.max_retained_endpoint_prerequisite_bytes
        || resources.boundary_evidence_logical_work > limits.max_boundary_evidence_logical_work
        || resources.boundary_evidence_workspace_bytes
            > limits.max_boundary_evidence_workspace_bytes
        || resources.publication_bytes > limits.max_publication_bytes
        || resources.aggregate_peak_bytes > limits.max_aggregate_peak_bytes
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(resources)
}

fn checked_declared_aggregate_peak_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    limits: LimitsV2,
) -> Result<usize, ErrorV2> {
    let retained_endpoint = size_of::<
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    >();
    let publication = size_of::<CertificateV2>();
    let outer_shell_delta = publication
        .checked_sub(retained_endpoint)
        .ok_or(ErrorV2::ResourceLimit)?;
    // Phase 3H's retained replay cap already includes its own certificate and
    // live schedule. Only the Phase 3I shell delta is simultaneously live.
    let endpoint_replay_phase = endpoint
        .replay_aggregate_peak_cap_v2()
        .checked_add(outer_shell_delta)
        .ok_or(ErrorV2::ResourceLimit)?;
    // Boundary replay keeps the retained Phase 3I publication while holding
    // a foreign candidate schedule, its transient workspace, and candidate
    // evidence. These phases are sequential with Phase 3H, so take `max`.
    let boundary_evidence_phase = publication
        .checked_add(limits.max_schedule_deep_retained_bytes)
        .and_then(|value| value.checked_add(limits.max_boundary_evidence_workspace_bytes))
        .and_then(|value| {
            value.checked_add(size_of::<
                CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
            >())
        })
        .ok_or(ErrorV2::ResourceLimit)?;
    let composition_phase = publication
        .checked_add(COMPOSITION_WORKSPACE_BYTES_V2)
        .ok_or(ErrorV2::ResourceLimit)?;
    Ok(endpoint_replay_phase
        .max(boundary_evidence_phase)
        .max(composition_phase))
}

fn binding_fingerprint_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    evidence: &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
    schedule_limits: CycleScheduleLimitsV1,
    resources: BoundaryConfigurationResourcesV2,
    limits: LimitsV2,
) -> Result<[u8; 32], ErrorV2> {
    let mut hash = Sha256::new();
    hash.update(
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_BOUNDARY_CONFIGURATION_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
            .as_bytes(),
    );
    hash.update(b"same-canonical-schedule-and-material-geometry-instance");
    hash.update(endpoint.binding_fingerprint_v2());
    hash.update(evidence.schedule_binding_fingerprint_v2());
    hash.update(endpoint.graph_binding_fingerprint_v1());
    hash.update(evidence.binding_fingerprint_v2());
    for value in [
        endpoint.actual_block_count_v2(),
        evidence.hinge_count_v2(),
        evidence.canonical_boundary_count_v2(),
        schedule_limits.max_hinges,
        schedule_limits.max_degree,
        schedule_limits.max_work,
        resources.retained_endpoint_prerequisite_bytes,
        resources.schedule_deep_retained_bytes_upper_bound,
        resources.boundary_evidence_bytes,
        resources.boundary_evidence_logical_work,
        resources.boundary_evidence_workspace_bytes,
        resources.delegated_endpoint_replay_peak_bytes,
        resources.composition_workspace_bytes,
        resources.publication_bytes,
        resources.aggregate_peak_bytes,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    hash.update(schedule_limits.max_coefficient_bits.to_le_bytes());
    for value in limit_values_v2(limits) {
        update_usize_v2(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

fn evidence_matches_v2(
    retained: &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
    candidate: &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
) -> bool {
    retained.model_id_v2() == candidate.model_id_v2()
        && retained.schedule_binding_fingerprint_v2() == candidate.schedule_binding_fingerprint_v2()
        && retained.binding_fingerprint_v2() == candidate.binding_fingerprint_v2()
        && retained.canonical_boundary_count_v2() == candidate.canonical_boundary_count_v2()
        && retained.hinge_count_v2() == candidate.hinge_count_v2()
        && retained.logical_work_v2() == candidate.logical_work_v2()
        && retained.workspace_peak_bytes_upper_bound_v2()
            == candidate.workspace_peak_bytes_upper_bound_v2()
}

fn revalidate_endpoint_v2(
    endpoint: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    input: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    endpoint
        .revalidate_with_checkpoint_v2(input, || checkpoint().map_err(map_stop_to_endpoint_v2))
        .map_err(map_endpoint_error_v2)
}

pub(super) const fn limits_match_v2(retained: LimitsV2, live: LimitsV2) -> bool {
    retained.max_blocks == live.max_blocks
        && retained.max_hinges == live.max_hinges
        && retained.max_schedule_deep_retained_bytes == live.max_schedule_deep_retained_bytes
        && retained.max_boundary_evidence_logical_work == live.max_boundary_evidence_logical_work
        && retained.max_boundary_evidence_workspace_bytes
            == live.max_boundary_evidence_workspace_bytes
        && retained.max_retained_endpoint_prerequisite_bytes
            == live.max_retained_endpoint_prerequisite_bytes
        && retained.max_publication_bytes == live.max_publication_bytes
        && retained.max_aggregate_peak_bytes == live.max_aggregate_peak_bytes
}

pub(super) const fn limit_values_v2(limits: LimitsV2) -> [usize; 8] {
    [
        limits.max_blocks,
        limits.max_hinges,
        limits.max_schedule_deep_retained_bytes,
        limits.max_boundary_evidence_logical_work,
        limits.max_boundary_evidence_workspace_bytes,
        limits.max_retained_endpoint_prerequisite_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
}

pub(super) fn schedule_graph_binding_pair_matches_v2(
    retained_schedule: [u8; 32],
    retained_graph: [u8; 32],
    candidate_schedule: [u8; 32],
    candidate_graph: [u8; 32],
) -> bool {
    retained_schedule == candidate_schedule && retained_graph == candidate_graph
}

fn update_usize_v2(hash: &mut Sha256, value: usize) -> Result<(), ErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| ErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
