//! Complete private binding for relief and whole-parent aggregation.

use super::*;

pub(super) fn relief_binding_v2(
    input: &ReliefAggregateInputV2<'_>,
    validated: &ValidatedReliefV2<'_>,
    partition_digest: [u8; 32],
) -> Result<[u8; 32], ReliefAggregateErrorV2> {
    let mut hash = Sha256::new();
    hash.update(RELIEF_MODEL_ID_V2.as_bytes());
    hash.update(input.ordinary.schedule.certificate_binding_fingerprint_v2());
    hash.update(
        validated
            .ordinary
            .interval_transform_session
            .bridge_binding_fingerprint_v2(),
    );
    hash.update(validated.ordinary.audit_binding);
    hash.update(validated.shared_pair_digest);
    hash.update(validated.policy_digest);
    hash.update(partition_digest);
    hash.update(input.ordinary.fixed_face.canonical_bytes());
    hash.update(input.ordinary.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(input.ordinary.closure_tolerance.to_bits().to_le_bytes());
    hash_limits_v2(&mut hash, input.limits)?;
    hash_resources_v2(&mut hash, validated.resources)?;
    Ok(hash.finalize().into())
}

pub(super) fn aggregate_binding_v2(
    input: &ReliefAggregateInputV2<'_>,
    ordinary: &OrdinaryIntervalEvidenceV2,
    relief: &SharedReliefEvidenceV2,
) -> Result<[u8; 32], ReliefAggregateErrorV2> {
    let mut hash = Sha256::new();
    hash.update(AGGREGATE_MODEL_ID_V2.as_bytes());
    hash.update(ordinary.binding_fingerprint);
    hash.update(relief.binding);
    hash.update(relief.shared_pair_digest);
    hash.update(relief.policy_digest);
    hash.update(relief.partition_digest);
    hash_limits_v2(&mut hash, input.limits)?;
    hash_resources_v2(&mut hash, relief.resources)?;
    Ok(hash.finalize().into())
}

fn hash_limits_v2(
    hash: &mut Sha256,
    limits: ReliefAggregateLimitsV2,
) -> Result<(), ReliefAggregateErrorV2> {
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
        update_usize_v2(hash, value).map_err(map_ordinary_error_v2)?;
    }
    hash.update(limits.max_collision_depth.to_le_bytes());
    Ok(())
}

fn hash_resources_v2(
    hash: &mut Sha256,
    resources: ReliefAggregateResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    for value in [
        resources.hinge_policy_records,
        resources.vertex_policy_records,
        resources.vertex_incident_face_occurrences,
        resources.shared_pairs,
        resources.shared_hinge_pairs,
        resources.shared_vertex_pairs,
        resources.pair_membership_tests,
        resources.pair_hinge_tests,
        resources.scope_and_policy_validation_work,
        resources.convexity_segment_tests,
        resources.rest_carrier_vertices,
        resources.exact_clip_operations,
        resources.sqrt_calls,
        resources.processed_interval_nodes,
        resources.accepted_interval_leaves,
        resources.certified_shared_pair_leaf_count,
        resources.shared_pair_node_tests,
        resources.axis_projection_work,
        resources.carrier_conversion_work,
        resources.hash_work,
        resources.logical_work,
        resources.retained_carrier_bytes,
        resources.exact_scratch_bytes,
        resources.temporary_bytes,
        resources.publication_bytes,
        resources.aggregate_peak_bytes,
    ] {
        update_usize_v2(hash, value).map_err(map_ordinary_error_v2)?;
    }
    Ok(())
}
