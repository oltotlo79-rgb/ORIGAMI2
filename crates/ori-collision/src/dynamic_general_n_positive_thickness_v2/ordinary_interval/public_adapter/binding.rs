//! Private limit conversion and complete adapter binding.

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn adapter_binding_v2(
    input: &CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
    seal: &WholeParentPositiveThicknessAdapterSealV2,
    actual_block_count: usize,
    shared_pair_registry_bytes: usize,
    aggregate_peak_bytes: usize,
) -> Result<([u8; 32], [u8; 32]), AdapterErrorV2> {
    let limits_binding = public_limits_binding_v2(input.limits)?;
    let mut hash = Sha256::new();
    hash.update(ADAPTER_MODEL_ID_V2.as_bytes());
    hash.update(seal.aggregate_binding);
    hash.update(limits_binding);
    hash.update(input.profile.binding_fingerprint_v2());
    for value in [
        actual_block_count,
        seal.total_face_pairs,
        seal.ordinary_pairs,
        seal.shared_hinge_pairs,
        seal.shared_vertex_pairs,
        seal.closed_domain_boundary_coverage
            .ordinary_lower_accepted_leaves,
        seal.closed_domain_boundary_coverage
            .ordinary_upper_accepted_leaves,
        seal.closed_domain_boundary_coverage
            .shared_relief_lower_accepted_leaves,
        seal.closed_domain_boundary_coverage
            .shared_relief_upper_accepted_leaves,
        shared_pair_registry_bytes,
        aggregate_peak_bytes,
    ] {
        update_usize_adapter_v2(&mut hash, value)?;
    }
    Ok((hash.finalize().into(), limits_binding))
}

pub(super) fn public_limits_binding_v2(
    limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
) -> Result<[u8; 32], AdapterErrorV2> {
    let mut hash = Sha256::new();
    hash.update(b"common-articulation-dynamic-general-n-relieved-clearance-limits/v2");
    hash_public_limits_v2(&mut hash, limits)?;
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

#[cfg(test)]
mod tests {
    use ori_kinematics::CycleScheduleLimitsV1;

    use super::*;

    #[test]
    fn public_limits_digest_binds_all_fifty_five_fields() {
        let limits = dummy_limits_v2();
        let retained = public_limits_binding_v2(limits).expect("finite retained limits");
        let mut checked = 0usize;
        macro_rules! assert_drift {
            ($($field:ident).+) => {{
                let mut candidate = limits;
                candidate$(.$field)+ += 1;
                assert_ne!(
                    retained,
                    public_limits_binding_v2(candidate).expect("finite upward drift"),
                    "{}",
                    stringify!($($field).+)
                );
                checked += 1;
            }};
        }

        assert_drift!(max_blocks);
        assert_drift!(max_shared_pair_registry_bytes);
        assert_drift!(max_publication_bytes);
        assert_drift!(max_aggregate_peak_bytes);
        assert_drift!(ordinary.max_faces);
        assert_drift!(ordinary.max_hinges);
        assert_drift!(ordinary.max_boundary_vertex_occurrences);
        assert_drift!(ordinary.max_excluded_shared_pairs);
        assert_drift!(ordinary.max_shared_feature_membership_tests);
        assert_drift!(ordinary.max_collision_depth);
        assert_drift!(ordinary.max_collision_leaves);
        assert_drift!(ordinary.schedule_limits.max_hinges);
        assert_drift!(ordinary.schedule_limits.max_degree);
        assert_drift!(ordinary.schedule_limits.max_coefficient_bits);
        assert_drift!(ordinary.schedule_limits.max_work);
        assert_drift!(ordinary.max_bridge_retained_bytes);
        assert_drift!(ordinary.max_bridge_revalidation_peak_bytes);
        assert_drift!(ordinary.max_schedule_retained_bytes);
        assert_drift!(ordinary.max_session_shell_bytes);
        assert_drift!(ordinary.max_schedule_evaluation_workspace_bytes);
        assert_drift!(ordinary.max_bridge_partition_search_work_per_node);
        assert_drift!(ordinary.max_interval_transform_work_per_node);
        assert_drift!(ordinary.max_interval_registry_validation_work_per_node);
        assert_drift!(ordinary.max_interval_registry_sort_comparisons_per_node);
        assert_drift!(ordinary.max_interval_registry_workspace_bytes);
        assert_drift!(ordinary.max_interval_registry_retained_bytes);
        assert_drift!(ordinary.max_ordinary_pair_node_tests);
        assert_drift!(ordinary.max_logical_work);
        assert_drift!(ordinary.max_temporary_bytes);
        assert_drift!(ordinary.max_publication_bytes);
        assert_drift!(ordinary.max_aggregate_peak_bytes);
        assert_drift!(relief.max_hinge_policy_records);
        assert_drift!(relief.max_vertex_policy_records);
        assert_drift!(relief.max_vertex_incident_face_occurrences);
        assert_drift!(relief.max_shared_pairs);
        assert_drift!(relief.max_pair_membership_tests);
        assert_drift!(relief.max_pair_hinge_tests);
        assert_drift!(relief.max_scope_and_policy_validation_work);
        assert_drift!(relief.max_convexity_segment_tests);
        assert_drift!(relief.max_rest_carrier_vertices);
        assert_drift!(relief.max_exact_clip_operations);
        assert_drift!(relief.max_sqrt_calls);
        assert_drift!(relief.max_sqrt_operations_per_call);
        assert_drift!(relief.max_exact_value_bits);
        assert_drift!(relief.max_exact_scratch_bytes);
        assert_drift!(relief.max_collision_depth);
        assert_drift!(relief.max_collision_leaves);
        assert_drift!(relief.max_shared_pair_node_tests);
        assert_drift!(relief.max_axis_projection_work);
        assert_drift!(relief.max_carrier_conversion_work);
        assert_drift!(relief.max_hash_work);
        assert_drift!(relief.max_logical_work);
        assert_drift!(relief.max_temporary_bytes);
        assert_drift!(relief.max_publication_bytes);
        assert_drift!(relief.max_aggregate_peak_bytes);
        assert_eq!(checked, 55);
    }

    fn dummy_limits_v2() -> CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
        CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
            max_blocks: 64,
            max_shared_pair_registry_bytes: 64,
            max_publication_bytes: 64,
            max_aggregate_peak_bytes: 64,
            ordinary: CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2 {
                max_faces: 64,
                max_hinges: 64,
                max_boundary_vertex_occurrences: 64,
                max_excluded_shared_pairs: 64,
                max_shared_feature_membership_tests: 64,
                max_collision_depth: 8,
                max_collision_leaves: 64,
                schedule_limits: CycleScheduleLimitsV1 {
                    max_hinges: 64,
                    max_degree: 64,
                    max_coefficient_bits: 32,
                    max_work: 64,
                },
                max_bridge_retained_bytes: 64,
                max_bridge_revalidation_peak_bytes: 64,
                max_schedule_retained_bytes: 64,
                max_session_shell_bytes: 64,
                max_schedule_evaluation_workspace_bytes: 64,
                max_bridge_partition_search_work_per_node: 64,
                max_interval_transform_work_per_node: 64,
                max_interval_registry_validation_work_per_node: 64,
                max_interval_registry_sort_comparisons_per_node: 64,
                max_interval_registry_workspace_bytes: 64,
                max_interval_registry_retained_bytes: 64,
                max_ordinary_pair_node_tests: 64,
                max_logical_work: 64,
                max_temporary_bytes: 64,
                max_publication_bytes: 64,
                max_aggregate_peak_bytes: 64,
            },
            relief: CommonArticulationDynamicGeneralNReliefAggregateLimitsV2 {
                max_hinge_policy_records: 64,
                max_vertex_policy_records: 64,
                max_vertex_incident_face_occurrences: 64,
                max_shared_pairs: 64,
                max_pair_membership_tests: 64,
                max_pair_hinge_tests: 64,
                max_scope_and_policy_validation_work: 64,
                max_convexity_segment_tests: 64,
                max_rest_carrier_vertices: 64,
                max_exact_clip_operations: 64,
                max_sqrt_calls: 64,
                max_sqrt_operations_per_call: 64,
                max_exact_value_bits: 64,
                max_exact_scratch_bytes: 64,
                max_collision_depth: 8,
                max_collision_leaves: 64,
                max_shared_pair_node_tests: 64,
                max_axis_projection_work: 64,
                max_carrier_conversion_work: 64,
                max_hash_work: 64,
                max_logical_work: 64,
                max_temporary_bytes: 64,
                max_publication_bytes: 64,
                max_aggregate_peak_bytes: 64,
            },
        }
    }
}
