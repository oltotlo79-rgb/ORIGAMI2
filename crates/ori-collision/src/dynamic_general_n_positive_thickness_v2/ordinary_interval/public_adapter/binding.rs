//! Private limit conversion and complete adapter binding.

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn adapter_binding_v2(
    input: &CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
    seal: &WholeParentPositiveThicknessAdapterSealV2,
    actual_block_count: usize,
    shared_pair_registry_bytes: usize,
    aggregate_peak_bytes: usize,
) -> Result<[u8; 32], AdapterErrorV2> {
    let mut hash = Sha256::new();
    hash.update(ADAPTER_MODEL_ID_V2.as_bytes());
    hash.update(seal.aggregate_binding);
    hash.update(input.profile.binding_fingerprint_v2());
    for value in [
        actual_block_count,
        seal.total_face_pairs,
        seal.ordinary_pairs,
        seal.shared_hinge_pairs,
        seal.shared_vertex_pairs,
        shared_pair_registry_bytes,
        aggregate_peak_bytes,
    ] {
        update_usize_adapter_v2(&mut hash, value)?;
    }
    hash_public_limits_v2(&mut hash, input.limits)?;
    Ok(hash.finalize().into())
}

fn hash_public_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
) -> Result<(), AdapterErrorV2> {
    for value in [
        limits.max_blocks,
        limits.max_shared_pair_registry_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ] {
        update_usize_adapter_v2(hash, value)?;
    }
    hash_ordinary_limits_v2(hash, limits.ordinary)?;
    hash_relief_limits_v2(hash, limits.relief)
}

fn hash_ordinary_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2,
) -> Result<(), AdapterErrorV2> {
    for value in [
        limits.max_faces,
        limits.max_hinges,
        limits.max_boundary_vertex_occurrences,
        limits.max_excluded_shared_pairs,
        limits.max_shared_feature_membership_tests,
        limits.max_collision_leaves,
        limits.schedule_limits.max_hinges,
        limits.schedule_limits.max_degree,
        limits.schedule_limits.max_work,
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
    ] {
        update_usize_adapter_v2(hash, value)?;
    }
    hash.update(limits.max_collision_depth.to_le_bytes());
    hash.update(limits.schedule_limits.max_coefficient_bits.to_le_bytes());
    Ok(())
}

fn hash_relief_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationDynamicGeneralNReliefAggregateLimitsV2,
) -> Result<(), AdapterErrorV2> {
    for value in [
        limits.max_hinge_policy_records,
        limits.max_vertex_policy_records,
        limits.max_vertex_incident_face_occurrences,
        limits.max_shared_pairs,
        limits.max_pair_membership_tests,
        limits.max_pair_hinge_tests,
        limits.max_scope_and_policy_validation_work,
        limits.max_convexity_segment_tests,
        limits.max_rest_carrier_vertices,
        limits.max_exact_clip_operations,
        limits.max_sqrt_calls,
        limits.max_sqrt_operations_per_call,
        limits.max_exact_value_bits,
        limits.max_exact_scratch_bytes,
        limits.max_collision_leaves,
        limits.max_shared_pair_node_tests,
        limits.max_axis_projection_work,
        limits.max_carrier_conversion_work,
        limits.max_hash_work,
        limits.max_logical_work,
        limits.max_temporary_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ] {
        update_usize_adapter_v2(hash, value)?;
    }
    hash.update(limits.max_collision_depth.to_le_bytes());
    Ok(())
}

pub(super) fn ordinary_limits_v2(
    limits: CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2,
) -> OrdinaryIntervalLimitsV2 {
    OrdinaryIntervalLimitsV2 {
        max_faces: limits.max_faces,
        max_hinges: limits.max_hinges,
        max_boundary_vertex_occurrences: limits.max_boundary_vertex_occurrences,
        max_excluded_shared_pairs: limits.max_excluded_shared_pairs,
        max_shared_feature_membership_tests: limits.max_shared_feature_membership_tests,
        max_collision_depth: limits.max_collision_depth,
        max_collision_leaves: limits.max_collision_leaves,
        schedule_limits: limits.schedule_limits,
        max_bridge_retained_bytes: limits.max_bridge_retained_bytes,
        max_bridge_revalidation_peak_bytes: limits.max_bridge_revalidation_peak_bytes,
        max_schedule_retained_bytes: limits.max_schedule_retained_bytes,
        max_session_shell_bytes: limits.max_session_shell_bytes,
        max_schedule_evaluation_workspace_bytes: limits.max_schedule_evaluation_workspace_bytes,
        max_bridge_partition_search_work_per_node: limits.max_bridge_partition_search_work_per_node,
        max_interval_transform_work_per_node: limits.max_interval_transform_work_per_node,
        max_interval_registry_validation_work_per_node: limits
            .max_interval_registry_validation_work_per_node,
        max_interval_registry_sort_comparisons_per_node: limits
            .max_interval_registry_sort_comparisons_per_node,
        max_interval_registry_workspace_bytes: limits.max_interval_registry_workspace_bytes,
        max_interval_registry_retained_bytes: limits.max_interval_registry_retained_bytes,
        max_ordinary_pair_node_tests: limits.max_ordinary_pair_node_tests,
        max_logical_work: limits.max_logical_work,
        max_temporary_bytes: limits.max_temporary_bytes,
        max_publication_bytes: limits.max_publication_bytes,
        max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes,
    }
}

pub(super) fn relief_limits_v2(
    limits: CommonArticulationDynamicGeneralNReliefAggregateLimitsV2,
) -> ReliefAggregateLimitsV2 {
    ReliefAggregateLimitsV2 {
        max_hinge_policy_records: limits.max_hinge_policy_records,
        max_vertex_policy_records: limits.max_vertex_policy_records,
        max_vertex_incident_face_occurrences: limits.max_vertex_incident_face_occurrences,
        max_shared_pairs: limits.max_shared_pairs,
        max_pair_membership_tests: limits.max_pair_membership_tests,
        max_pair_hinge_tests: limits.max_pair_hinge_tests,
        max_scope_and_policy_validation_work: limits.max_scope_and_policy_validation_work,
        max_convexity_segment_tests: limits.max_convexity_segment_tests,
        max_rest_carrier_vertices: limits.max_rest_carrier_vertices,
        max_exact_clip_operations: limits.max_exact_clip_operations,
        max_sqrt_calls: limits.max_sqrt_calls,
        max_sqrt_operations_per_call: limits.max_sqrt_operations_per_call,
        max_exact_value_bits: limits.max_exact_value_bits,
        max_exact_scratch_bytes: limits.max_exact_scratch_bytes,
        max_collision_depth: limits.max_collision_depth,
        max_collision_leaves: limits.max_collision_leaves,
        max_shared_pair_node_tests: limits.max_shared_pair_node_tests,
        max_axis_projection_work: limits.max_axis_projection_work,
        max_carrier_conversion_work: limits.max_carrier_conversion_work,
        max_hash_work: limits.max_hash_work,
        max_logical_work: limits.max_logical_work,
        max_temporary_bytes: limits.max_temporary_bytes,
        max_publication_bytes: limits.max_publication_bytes,
        max_aggregate_peak_bytes: limits.max_aggregate_peak_bytes,
    }
}

fn update_usize_adapter_v2(hash: &mut Sha256, value: usize) -> Result<(), AdapterErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| AdapterErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
