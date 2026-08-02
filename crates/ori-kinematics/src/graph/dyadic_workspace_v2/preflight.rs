use super::*;

pub(super) fn checked_vec_bytes_v2<T>(count: usize) -> Option<usize> {
    size_of::<T>().checked_mul(count)
}

pub(super) fn limits_contain_usize_max_v2(limits: DyadicIntervalClosureWorkspaceLimitsV2) -> bool {
    [
        limits.max_leaves,
        limits.max_work,
        limits.schedule_limits.max_hinges,
        limits.schedule_limits.max_degree,
        limits.schedule_limits.max_work,
        limits.max_theorem_recognizer_work,
        limits.max_theorem_recognizer_workspace_bytes,
        limits.max_carrier_index_workspace_bytes,
        limits.max_schedule_evaluation_workspace_bytes,
        limits.max_big_rational_payload_bytes,
        limits.max_exact_rational_object_bytes,
        limits.max_interval_closure_workspace_bytes,
        limits.max_partition_workspace_bytes,
        limits.max_retained_material_bytes,
        limits.max_publication_workspace_bytes,
        limits.max_peak_workspace_bytes,
    ]
    .contains(&usize::MAX)
}

pub(super) fn checked_interval_workspace_upper_bound_v2(
    _geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
) -> Option<usize> {
    let faces = audit.faces().len();
    let spanning = audit.spanning_hinges().len();
    let mut total = checked_vec_bytes_v2::<Vec<(usize, usize, bool)>>(faces)?;
    total = total
        .checked_add(checked_vec_bytes_v2::<(usize, usize, bool)>(
            spanning.checked_mul(2)?,
        )?)?
        .checked_add(checked_vec_bytes_v2::<usize>(faces)?)?
        .checked_add(checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(
            faces,
        )?)?
        .checked_add(checked_vec_bytes_v2::<usize>(faces)?)?;
    Some(total)
}

pub(super) fn checked_preflight_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    schedule: CycleScheduleDyadicWorkspaceBoundV2,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Option<WorkspacePreflightV2> {
    let hinges = geometry.hinges().len();
    let carrier_index = checked_vec_bytes_v2::<usize>(hinges)?;
    let interval = checked_interval_workspace_upper_bound_v2(geometry, audit)?;
    let partition_stack = checked_vec_bytes_v2::<(u32, u64)>(limits.max_leaves)?;
    let retained = size_of::<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2>()
        .checked_add(checked_vec_bytes_v2::<(u32, u64)>(limits.max_leaves)?)?
        .checked_add(checked_vec_bytes_v2::<EdgeId>(hinges)?)?;
    // SHA-256 and the result shell are stack-resident, but charging them here
    // makes the publication phase explicit and keeps the peak conservative.
    let publication = size_of::<Sha256>().checked_add(size_of::<
        WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
    >())?;
    let proof_phase = schedule.peak_bytes().checked_add(interval)?;
    let peak = carrier_index
        .checked_add(partition_stack)?
        .checked_add(retained)?
        .checked_add(proof_phase.max(publication))?;
    Some(WorkspacePreflightV2 {
        schedule,
        resources: DyadicIntervalClosureWorkspaceResourcesV2 {
            charged_binding_validation_upper_bound_bytes: 0,
            charged_theorem_recognizer_work: 0,
            charged_theorem_recognizer_upper_bound_bytes: 0,
            charged_carrier_index_workspace_upper_bound_bytes: carrier_index,
            charged_schedule_evaluation_workspace_upper_bound_bytes: schedule.peak_bytes(),
            charged_big_rational_payload_upper_bound_bytes: schedule.big_rational_payload_bytes(),
            charged_exact_rational_object_upper_bound_bytes: schedule.exact_object_bytes(),
            charged_interval_closure_workspace_upper_bound_bytes: interval,
            charged_partition_workspace_upper_bound_bytes: partition_stack,
            charged_retained_material_upper_bound_bytes: retained,
            charged_publication_workspace_upper_bound_bytes: publication,
            charged_peak_workspace_upper_bound_bytes: peak,
            visited_partition_nodes: 0,
            issued_leaves: 0,
        },
    })
}

pub(super) fn resources_fit_limits_v2(
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> bool {
    resources.charged_carrier_index_workspace_upper_bound_bytes
        <= limits.max_carrier_index_workspace_bytes
        && resources.charged_theorem_recognizer_work <= limits.max_theorem_recognizer_work
        && resources.charged_theorem_recognizer_upper_bound_bytes
            <= limits.max_theorem_recognizer_workspace_bytes
        && resources.charged_schedule_evaluation_workspace_upper_bound_bytes
            <= limits.max_schedule_evaluation_workspace_bytes
        && resources.charged_big_rational_payload_upper_bound_bytes
            <= limits.max_big_rational_payload_bytes
        && resources.charged_exact_rational_object_upper_bound_bytes
            <= limits.max_exact_rational_object_bytes
        && resources.charged_interval_closure_workspace_upper_bound_bytes
            <= limits.max_interval_closure_workspace_bytes
        && resources.charged_partition_workspace_upper_bound_bytes
            <= limits.max_partition_workspace_bytes
        && resources.charged_retained_material_upper_bound_bytes
            <= limits.max_retained_material_bytes
        && resources.charged_publication_workspace_upper_bound_bytes
            <= limits.max_publication_workspace_bytes
        && resources.charged_peak_workspace_upper_bound_bytes <= limits.max_peak_workspace_bytes
}

pub(super) fn refresh_peak_v2(
    resources: &mut DyadicIntervalClosureWorkspaceResourcesV2,
) -> Option<()> {
    let proof_phase = resources
        .charged_schedule_evaluation_workspace_upper_bound_bytes
        .checked_add(resources.charged_interval_closure_workspace_upper_bound_bytes)?;
    resources.charged_peak_workspace_upper_bound_bytes = resources
        .charged_carrier_index_workspace_upper_bound_bytes
        .checked_add(resources.charged_partition_workspace_upper_bound_bytes)?
        .checked_add(resources.charged_retained_material_upper_bound_bytes)?
        .checked_add(
            proof_phase
                .max(resources.charged_theorem_recognizer_upper_bound_bytes)
                .max(resources.charged_publication_workspace_upper_bound_bytes),
        )?;
    Some(())
}
