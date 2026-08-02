//! Public finite resource envelopes for direct relieved clearance.

use ori_kinematics::CycleScheduleLimitsV1;

/// Finite bounds for the ordinary-pair interval proof.
///
/// The shared-pair derivation and the ordinary proof each perform their own
/// bounded membership scan. Both scans use the limits sealed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2 {
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_boundary_vertex_occurrences: usize,
    pub max_excluded_shared_pairs: usize,
    pub max_shared_feature_membership_tests: usize,
    pub max_collision_depth: u32,
    pub max_collision_leaves: usize,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub max_bridge_retained_bytes: usize,
    pub max_bridge_revalidation_peak_bytes: usize,
    pub max_schedule_retained_bytes: usize,
    pub max_session_shell_bytes: usize,
    pub max_schedule_evaluation_workspace_bytes: usize,
    pub max_bridge_partition_search_work_per_node: usize,
    pub max_interval_transform_work_per_node: usize,
    pub max_interval_registry_validation_work_per_node: usize,
    pub max_interval_registry_sort_comparisons_per_node: usize,
    pub max_interval_registry_workspace_bytes: usize,
    pub max_interval_registry_retained_bytes: usize,
    pub max_ordinary_pair_node_tests: usize,
    pub max_logical_work: usize,
    pub max_temporary_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

/// Finite bounds for shared-hinge/shared-vertex relief and aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNReliefAggregateLimitsV2 {
    pub max_hinge_policy_records: usize,
    pub max_vertex_policy_records: usize,
    pub max_vertex_incident_face_occurrences: usize,
    pub max_shared_pairs: usize,
    pub max_pair_membership_tests: usize,
    pub max_pair_hinge_tests: usize,
    pub max_scope_and_policy_validation_work: usize,
    pub max_convexity_segment_tests: usize,
    pub max_rest_carrier_vertices: usize,
    pub max_exact_clip_operations: usize,
    pub max_sqrt_calls: usize,
    pub max_sqrt_operations_per_call: usize,
    pub max_exact_value_bits: usize,
    pub max_exact_scratch_bytes: usize,
    pub max_collision_depth: u32,
    pub max_collision_leaves: usize,
    pub max_shared_pair_node_tests: usize,
    pub max_axis_projection_work: usize,
    pub max_carrier_conversion_work: usize,
    pub max_hash_work: usize,
    pub max_logical_work: usize,
    pub max_temporary_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

/// Complete finite envelope for direct dynamic general-N relieved clearance.
///
/// `max_shared_pair_registry_bytes` charges the internally derived registry,
/// which remains live while the private whole-parent proof runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2 {
    pub max_blocks: usize,
    pub max_shared_pair_registry_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
    pub ordinary: CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2,
    pub relief: CommonArticulationDynamicGeneralNReliefAggregateLimitsV2,
}
