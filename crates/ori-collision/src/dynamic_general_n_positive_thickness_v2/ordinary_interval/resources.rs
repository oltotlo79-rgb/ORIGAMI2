//! Input validation and checked resource preflight.

use ori_kinematics::{
    CommonArticulationDynamicClosureBridgeErrorV2,
    CommonArticulationDynamicClosureBridgeRevalidationInputV2,
    CommonArticulationDynamicClosureBridgeStopV2, CycleScheduleDyadicEvaluationErrorV2,
    CycleScheduleDyadicEvaluationStopV2, DyadicIntervalClosureStopV1,
    IntervalFaceTransformWorkspaceErrorV2, IntervalFaceTransformWorkspaceLimitsV2,
};

use super::*;

mod preflight;

pub(super) use preflight::preflight_resources_v2;

pub(super) fn validate_input_v2<'a>(
    input: &OrdinaryIntervalInputV2<'a>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<ValidatedInputV2<'a>, OrdinaryIntervalErrorV2> {
    validate_limits_v2(input.limits)?;
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.closure_tolerance.is_finite()
        || input.closure_tolerance < 0.0
    {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    let excluded_pair_count = input.excluded_shared_pairs.len();
    if face_count > input.limits.max_faces
        || hinge_count > input.limits.max_hinges
        || excluded_pair_count > input.limits.max_excluded_shared_pairs
        || input.dynamic_closure_bridge.retained_bytes_upper_bound_v2()
            > input.limits.max_bridge_retained_bytes
        || input
            .dynamic_closure_bridge
            .revalidation_peak_bytes_upper_bound_v2()
            > input.limits.max_bridge_revalidation_peak_bytes
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    if excluded_pair_count > unordered_pair_count_v2(face_count)? {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    validate_face_order_v2(input.geometry.face_ids(), checkpoint)?;
    validate_audit_face_order_v2(input.audit.faces(), input.geometry.face_ids(), checkpoint)?;
    validate_fixed_face_membership_v2(input.geometry.face_ids(), input.fixed_face, checkpoint)?;
    let boundary_vertex_occurrences = validate_planar_boundaries_v2(
        input.geometry,
        input.limits.max_boundary_vertex_occurrences,
        checkpoint,
    )?;
    if boundary_vertex_occurrences
        .checked_mul(boundary_vertex_occurrences)
        .is_none_or(|work| work > input.limits.max_shared_feature_membership_tests)
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    checkpoint_v2(checkpoint)?;
    let session_resource_bound = input
        .dynamic_closure_bridge
        .checked_interval_transform_session_resources_with_checkpoint_v2(
            input.schedule,
            input.limits.max_schedule_retained_bytes,
            || {
                checkpoint().map_err(|stop| match stop {
                    OrdinaryIntervalStopV2::Cancelled => {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled
                    }
                    OrdinaryIntervalStopV2::DeadlineExceeded => {
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded
                    }
                })
            },
        )
        .map_err(map_bridge_error_v2)?;
    let schedule_workspace_bound = input
        .schedule
        .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(
            input.limits.max_collision_depth,
            input.limits.schedule_limits,
            || {
                checkpoint().map_err(|stop| match stop {
                    OrdinaryIntervalStopV2::Cancelled => {
                        CycleScheduleDyadicEvaluationStopV2::Cancelled
                    }
                    OrdinaryIntervalStopV2::DeadlineExceeded => {
                        CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                    }
                })
            },
        )
        .map_err(map_schedule_workspace_error_v2)?;
    let interval_transform_workspace_bound = input
        .geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            input.audit,
            input.fixed_face,
            IntervalFaceTransformWorkspaceLimitsV2 {
                max_work: input.limits.max_interval_transform_work_per_node,
                max_validation_work: input.limits.max_interval_registry_validation_work_per_node,
                max_sort_comparisons: input.limits.max_interval_registry_sort_comparisons_per_node,
                max_workspace_bytes: input.limits.max_interval_registry_workspace_bytes,
                max_retained_bytes: input.limits.max_interval_registry_retained_bytes,
            },
            || {
                checkpoint().map_err(|stop| match stop {
                    OrdinaryIntervalStopV2::Cancelled => DyadicIntervalClosureStopV1::Cancelled,
                    OrdinaryIntervalStopV2::DeadlineExceeded => {
                        DyadicIntervalClosureStopV1::DeadlineExceeded
                    }
                })
            },
        )
        .map_err(map_transform_workspace_error_v2)?;
    let resources = preflight_resources_v2(
        input,
        boundary_vertex_occurrences,
        schedule_workspace_bound,
        interval_transform_workspace_bound.checked_resources(),
        session_resource_bound,
    )?;
    validate_excluded_pair_order_v2(input.excluded_shared_pairs, checkpoint)?;
    let excluded_shared_pair_digest = super::geometry::validate_exact_shared_pair_registry_v2(
        input.geometry,
        input.excluded_shared_pairs,
        resources.charged_shared_feature_membership_tests,
        checkpoint,
    )?;
    let audit_binding = super::binding::audit_binding_v2(input.audit, checkpoint)?;
    let interval_transform_session = input
        .dynamic_closure_bridge
        .prepare_interval_transform_session_with_checkpoint_v2(
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                pose: input.pose,
                parent_fixed_face: input.fixed_face,
                parent_schedule: input.schedule,
                decomposition: input.decomposition,
                common_pose: input.common_pose,
                paper_thickness_mm: input.paper_thickness_mm,
                closure_tolerance: input.closure_tolerance,
                profile: input.profile,
            },
            || {
                checkpoint().map_err(|stop| match stop {
                    OrdinaryIntervalStopV2::Cancelled => {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled
                    }
                    OrdinaryIntervalStopV2::DeadlineExceeded => {
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded
                    }
                })
            },
        )
        .map_err(map_bridge_error_v2)?;
    if interval_transform_session.resources() != session_resource_bound {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    Ok(ValidatedInputV2 {
        audit_binding,
        excluded_shared_pair_digest,
        resources,
        schedule_workspace_bound,
        interval_transform_workspace_bound,
        interval_transform_session,
    })
}

fn validate_limits_v2(limits: OrdinaryIntervalLimitsV2) -> Result<(), OrdinaryIntervalErrorV2> {
    let schedule = limits.schedule_limits;
    if [
        limits.max_faces,
        limits.max_hinges,
        limits.max_boundary_vertex_occurrences,
        limits.max_excluded_shared_pairs,
        limits.max_shared_feature_membership_tests,
        limits.max_collision_leaves,
        limits.max_bridge_retained_bytes,
        limits.max_bridge_revalidation_peak_bytes,
        limits.max_schedule_retained_bytes,
        limits.max_session_shell_bytes,
        limits.max_schedule_evaluation_workspace_bytes,
        limits.max_bridge_partition_search_work_per_node,
        limits.max_interval_transform_work_per_node,
        limits.max_interval_registry_validation_work_per_node,
        limits.max_interval_registry_sort_comparisons_per_node,
        limits.max_interval_registry_workspace_bytes,
        limits.max_interval_registry_retained_bytes,
        limits.max_ordinary_pair_node_tests,
        limits.max_logical_work,
        limits.max_temporary_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
        schedule.max_hinges,
        schedule.max_degree,
        schedule.max_work,
    ]
    .into_iter()
    .any(|value| value == 0 || value == usize::MAX)
        || limits.max_collision_depth == 0
        || limits.max_collision_depth >= 64
        || schedule.max_coefficient_bits == 0
        || schedule.max_coefficient_bits == u32::MAX
        || schedule.max_hinges > limits.max_hinges
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    Ok(())
}

fn validate_face_order_v2(
    faces: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    if faces.is_empty() {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    for pair in faces.windows(2) {
        checkpoint_v2(checkpoint)?;
        if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
            return Err(OrdinaryIntervalErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn validate_audit_face_order_v2(
    audit_faces: &[FaceId],
    geometry_faces: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    if audit_faces.len() != geometry_faces.len() {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    for (audit_face, geometry_face) in audit_faces.iter().zip(geometry_faces) {
        checkpoint_v2(checkpoint)?;
        if audit_face != geometry_face {
            return Err(OrdinaryIntervalErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn validate_fixed_face_membership_v2(
    faces: &[FaceId],
    fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    let target = fixed_face.canonical_bytes();
    let mut lower = 0usize;
    let mut upper = faces.len();
    while lower < upper {
        checkpoint_v2(checkpoint)?;
        let middle = lower + (upper - lower) / 2;
        match faces[middle].canonical_bytes().cmp(&target) {
            std::cmp::Ordering::Less => lower = middle + 1,
            std::cmp::Ordering::Greater => upper = middle,
            std::cmp::Ordering::Equal => return Ok(()),
        }
    }
    Err(OrdinaryIntervalErrorV2::InvalidInput)
}

fn validate_planar_boundaries_v2(
    geometry: &MaterialHingeGraphGeometry,
    maximum_occurrences: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<usize, OrdinaryIntervalErrorV2> {
    let mut occurrences = 0usize;
    for face in geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        let boundary = geometry
            .face_boundary_vertices(*face)
            .filter(|vertices| vertices.len() >= 3)
            .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
        occurrences = occurrences
            .checked_add(boundary.len())
            .filter(|value| *value <= maximum_occurrences)
            .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
        for vertex in boundary {
            checkpoint_v2(checkpoint)?;
            let point = geometry
                .vertex_position(*vertex)
                .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
            if !point.x().is_finite()
                || !point.y().is_finite()
                || !point.z().is_finite()
                || point.y() != 0.0
            {
                return Err(OrdinaryIntervalErrorV2::InvalidInput);
            }
        }
    }
    Ok(occurrences)
}

fn validate_excluded_pair_order_v2(
    pairs: &[OrdinaryIntervalFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    for pair in pairs {
        checkpoint_v2(checkpoint)?;
        if pair.first.canonical_bytes() >= pair.second.canonical_bytes() {
            return Err(OrdinaryIntervalErrorV2::NonCanonicalExcludedSharedPairRegistry);
        }
    }
    for pair in pairs.windows(2) {
        checkpoint_v2(checkpoint)?;
        match compare_pair_v2(&pair[0], &pair[1]) {
            Ordering::Less => {}
            Ordering::Equal => return Err(OrdinaryIntervalErrorV2::DuplicateExcludedSharedPair),
            Ordering::Greater => {
                return Err(OrdinaryIntervalErrorV2::NonCanonicalExcludedSharedPairRegistry);
            }
        }
    }
    Ok(())
}

fn unordered_pair_count_v2(count: usize) -> Result<usize, OrdinaryIntervalErrorV2> {
    count
        .checked_mul(
            count
                .checked_sub(1)
                .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?,
        )
        .map(|value| value / 2)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)
}

fn map_transform_workspace_error_v2(
    error: IntervalFaceTransformWorkspaceErrorV2,
) -> OrdinaryIntervalErrorV2 {
    match error {
        IntervalFaceTransformWorkspaceErrorV2::InvalidInput => {
            OrdinaryIntervalErrorV2::InvalidInput
        }
        IntervalFaceTransformWorkspaceErrorV2::ResourceLimit => {
            OrdinaryIntervalErrorV2::ResourceLimit
        }
        IntervalFaceTransformWorkspaceErrorV2::Unproven => {
            OrdinaryIntervalErrorV2::UnprovenOrdinaryClearance
        }
        IntervalFaceTransformWorkspaceErrorV2::Cancelled => OrdinaryIntervalErrorV2::Cancelled,
        IntervalFaceTransformWorkspaceErrorV2::DeadlineExceeded => {
            OrdinaryIntervalErrorV2::DeadlineExceeded
        }
    }
}

fn map_schedule_workspace_error_v2(
    error: CycleScheduleDyadicEvaluationErrorV2,
) -> OrdinaryIntervalErrorV2 {
    match error {
        CycleScheduleDyadicEvaluationErrorV2::Prepare(_) => OrdinaryIntervalErrorV2::ResourceLimit,
        CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit => {
            OrdinaryIntervalErrorV2::ResourceLimit
        }
        CycleScheduleDyadicEvaluationErrorV2::Cancelled => OrdinaryIntervalErrorV2::Cancelled,
        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded => {
            OrdinaryIntervalErrorV2::DeadlineExceeded
        }
    }
}

fn map_bridge_error_v2(
    error: CommonArticulationDynamicClosureBridgeErrorV2,
) -> OrdinaryIntervalErrorV2 {
    match error {
        CommonArticulationDynamicClosureBridgeErrorV2::ResourceLimit => {
            OrdinaryIntervalErrorV2::ResourceLimit
        }
        CommonArticulationDynamicClosureBridgeErrorV2::Cancelled => {
            OrdinaryIntervalErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded => {
            OrdinaryIntervalErrorV2::DeadlineExceeded
        }
        CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput
        | CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch
        | CommonArticulationDynamicClosureBridgeErrorV2::UnprovenClosure { .. } => {
            OrdinaryIntervalErrorV2::InvalidInput
        }
    }
}
