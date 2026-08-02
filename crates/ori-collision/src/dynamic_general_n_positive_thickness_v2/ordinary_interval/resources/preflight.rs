use std::mem::size_of;

use ori_domain::EdgeId;
use ori_kinematics::{
    CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
    CycleScheduleDyadicWorkspaceBoundV2, IntervalFaceTransformWorkspaceResourcesV2,
    OutwardIntervalV1,
};

use super::super::*;
use super::unordered_pair_count_v2;

pub(in crate::dynamic_general_n_positive_thickness_v2::ordinary_interval) fn preflight_resources_v2(
    input: &OrdinaryIntervalInputV2<'_>,
    boundary_vertex_occurrences: usize,
    schedule_workspace_bound: CycleScheduleDyadicWorkspaceBoundV2,
    interval_transform_resources: IntervalFaceTransformWorkspaceResourcesV2,
    session_resources: CommonArticulationDynamicClosureIntervalTransformSessionResourcesV2,
) -> Result<OrdinaryIntervalResourcesV2, OrdinaryIntervalErrorV2> {
    let limits = input.limits;
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    let excluded_shared_pairs = input.excluded_shared_pairs.len();
    let total_face_pairs = unordered_pair_count_v2(face_count)?;
    let ordinary_face_pairs = total_face_pairs
        .checked_sub(excluded_shared_pairs)
        .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
    if face_count > limits.max_faces
        || hinge_count > limits.max_hinges
        || boundary_vertex_occurrences > limits.max_boundary_vertex_occurrences
        || excluded_shared_pairs > limits.max_excluded_shared_pairs
        || excluded_shared_pairs > total_face_pairs
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }

    let charged_interval_nodes = limits
        .max_collision_leaves
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_shared_feature_membership_tests = boundary_vertex_occurrences
        .checked_mul(boundary_vertex_occurrences)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_ordinary_pair_node_tests = charged_interval_nodes
        .checked_mul(ordinary_face_pairs)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_axis_tests = charged_ordinary_pair_node_tests
        .checked_mul(AXIS_COUNT_V2)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_surface_vertex_visits = charged_interval_nodes
        .checked_mul(boundary_vertex_occurrences)
        .and_then(|value| value.checked_mul(THICK_SURFACE_COUNT_V2))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_pair_classification_visits = charged_interval_nodes
        .checked_mul(total_face_pairs)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let schedule_work_per_node = hinge_count
        .checked_mul(
            limits
                .schedule_limits
                .max_degree
                .checked_add(1)
                .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_mul(limits.schedule_limits.max_work))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_schedule_work = charged_interval_nodes
        .checked_mul(schedule_work_per_node)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    // This is the registry's shared interval-arithmetic complexity ceiling per
    // leaf. Thick-surface `apply` calls use that provenance cap but do not each
    // consume it; their physical call count is charged separately by
    // `charged_surface_vertex_visits`.
    let charged_transform_work = charged_interval_nodes
        .checked_mul(limits.max_interval_transform_work_per_node)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_interval_registry_validation_work = charged_interval_nodes
        .checked_add(1)
        .and_then(|nodes| {
            nodes.checked_mul(interval_transform_resources.validation_work_upper_bound())
        })
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    // Each V already encloses one complete leaf workflow (precoverage bound
    // validation, registry recheck/input binding and final self-match). N+1
    // is therefore a conservative envelope for issuance plus N workflows,
    // not an exact count of validation calls.
    let charged_interval_registry_sort_comparisons = charged_interval_nodes
        .checked_mul(interval_transform_resources.sort_comparison_upper_bound())
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let coverage_work_per_node = session_resources.coverage_search_comparison_upper_bound();
    if coverage_work_per_node > limits.max_bridge_partition_search_work_per_node {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    let charged_bridge_partition_search_work = charged_interval_nodes
        .checked_mul(coverage_work_per_node)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_logical_work = charged_shared_feature_membership_tests
        .checked_add(charged_schedule_work)
        .and_then(|value| value.checked_add(charged_transform_work))
        .and_then(|value| value.checked_add(charged_bridge_partition_search_work))
        .and_then(|value| value.checked_add(charged_interval_registry_validation_work))
        .and_then(|value| value.checked_add(charged_interval_registry_sort_comparisons))
        .and_then(|value| value.checked_add(charged_surface_vertex_visits))
        .and_then(|value| value.checked_add(charged_pair_classification_visits))
        .and_then(|value| value.checked_add(charged_axis_tests))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;

    let charged_pending_partition_bytes = limits
        .max_collision_leaves
        .checked_mul(size_of::<DyadicLeafV2>())
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_bridge_retained_bytes = session_resources.bridge_retained_bytes();
    let charged_bridge_revalidation_peak_bytes = session_resources.bridge_revalidation_peak_bytes();
    let charged_schedule_retained_bytes = session_resources.schedule_retained_bytes();
    let charged_session_shell_bytes = session_resources.session_shell_bytes();
    let charged_session_steady_retained_bytes = session_resources.steady_retained_bytes();
    let charged_bridge_revalidation_phase_peak_bytes =
        session_resources.revalidation_phase_peak_bytes();
    if charged_bridge_retained_bytes > limits.max_bridge_retained_bytes
        || charged_bridge_revalidation_peak_bytes > limits.max_bridge_revalidation_peak_bytes
        || charged_schedule_retained_bytes > limits.max_schedule_retained_bytes
        || charged_session_shell_bytes > limits.max_session_shell_bytes
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    let charged_schedule_evaluation_workspace_bytes = schedule_workspace_bound.peak_bytes();
    let charged_angle_box_bytes = schedule_workspace_bound.angle_box_bytes();
    let expected_angle_box_bytes = hinge_count
        .checked_mul(size_of::<(EdgeId, OutwardIntervalV1)>())
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    if charged_angle_box_bytes != expected_angle_box_bytes
        || charged_schedule_evaluation_workspace_bytes
            > limits.max_schedule_evaluation_workspace_bytes
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    let charged_face_aabb_bytes = face_count
        .checked_mul(size_of::<ThickFaceAabbV2>())
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_interval_registry_workspace_bytes =
        interval_transform_resources.construction_peak_bytes();
    let charged_interval_registry_retained_bytes =
        interval_transform_resources.retained_registry_bytes();
    let charged_leaf_wrapper_overhead_bytes = session_resources.leaf_wrapper_overhead_bytes();
    let charged_leaf_retained_bytes = charged_interval_registry_retained_bytes
        .checked_add(charged_leaf_wrapper_overhead_bytes)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    // This uses the same proof-carrier allocation ledger as the session
    // resources, not process RSS. Caller-owned immutable geometry, audit,
    // pose, decomposition, common-pose and profile backing remains excluded
    // here and governed by its input/issuer caps.
    // The bridge revalidation peak already includes retained bridge backing.
    // Steady leaf phases add bridge, schedule and session backing once. Exact
    // schedule temporaries drop before registry construction, and angle boxes
    // drop before AABB pair testing.
    let schedule_phase = charged_session_steady_retained_bytes
        .checked_add(charged_pending_partition_bytes)
        .and_then(|value| value.checked_add(charged_schedule_evaluation_workspace_bytes))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let registry_phase = charged_session_steady_retained_bytes
        .checked_add(charged_pending_partition_bytes)
        .and_then(|value| value.checked_add(charged_angle_box_bytes))
        .and_then(|value| value.checked_add(charged_interval_registry_workspace_bytes))
        .and_then(|value| value.checked_add(charged_leaf_wrapper_overhead_bytes))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let pair_phase = charged_session_steady_retained_bytes
        .checked_add(charged_pending_partition_bytes)
        .and_then(|value| value.checked_add(charged_leaf_retained_bytes))
        .and_then(|value| value.checked_add(charged_face_aabb_bytes))
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_temporary_bytes = charged_bridge_revalidation_phase_peak_bytes
        .max(schedule_phase)
        .max(registry_phase)
        .max(pair_phase);
    let charged_publication_bytes = size_of::<OrdinaryIntervalEvidenceV2>();
    let publication_phase = charged_session_steady_retained_bytes
        .checked_add(charged_publication_bytes)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    let charged_aggregate_peak_bytes = charged_temporary_bytes.max(publication_phase);

    if charged_shared_feature_membership_tests > limits.max_shared_feature_membership_tests
        || charged_ordinary_pair_node_tests > limits.max_ordinary_pair_node_tests
        || charged_logical_work > limits.max_logical_work
        || charged_temporary_bytes > limits.max_temporary_bytes
        || charged_publication_bytes > limits.max_publication_bytes
        || charged_aggregate_peak_bytes > limits.max_aggregate_peak_bytes
    {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    Ok(OrdinaryIntervalResourcesV2 {
        face_count,
        hinge_count,
        boundary_vertex_occurrences,
        total_face_pairs,
        excluded_shared_pairs,
        ordinary_face_pairs,
        charged_interval_nodes,
        charged_shared_feature_membership_tests,
        charged_ordinary_pair_node_tests,
        charged_axis_tests,
        charged_surface_vertex_visits,
        charged_interval_registry_validation_work,
        charged_interval_registry_sort_comparisons,
        charged_bridge_partition_search_work,
        charged_logical_work,
        charged_pending_partition_bytes,
        charged_bridge_retained_bytes,
        charged_bridge_revalidation_peak_bytes,
        charged_schedule_retained_bytes,
        charged_session_shell_bytes,
        charged_session_steady_retained_bytes,
        charged_bridge_revalidation_phase_peak_bytes,
        charged_schedule_evaluation_workspace_bytes,
        charged_angle_box_bytes,
        charged_interval_registry_workspace_bytes,
        charged_interval_registry_retained_bytes,
        charged_leaf_wrapper_overhead_bytes,
        charged_leaf_retained_bytes,
        charged_face_aabb_bytes,
        charged_temporary_bytes,
        charged_publication_bytes,
        charged_aggregate_peak_bytes,
    })
}
