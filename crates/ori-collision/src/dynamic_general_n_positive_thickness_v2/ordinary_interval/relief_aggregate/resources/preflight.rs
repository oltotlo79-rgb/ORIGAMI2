//! Allocation-free scalar, scope, and structural resource preflight.

use super::*;

pub(in crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::relief_aggregate) fn preflight_limits_v2(
    input: &ReliefAggregateInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), ReliefAggregateErrorV2> {
    let limits = input.limits;
    let scalar_limits = [
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
    ];
    let profile_actual = input.ordinary.profile.actual_v2();
    let profile_maximum = input.ordinary.profile.maximum_v2();
    let maximum_vertex_occurrences = profile_maximum
        .face_count_v2()
        .checked_mul(4)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    if scalar_limits.contains(&usize::MAX)
        || scalar_limits.contains(&0)
        || limits.max_hinge_policy_records > HARD_MAX_RELIEF_RECORDS_V2
        || limits.max_hinge_policy_records > profile_maximum.hinge_count_v2()
        || limits.max_vertex_policy_records > HARD_MAX_RELIEF_RECORDS_V2
        || limits.max_vertex_policy_records > maximum_vertex_occurrences
        || limits.max_vertex_incident_face_occurrences > maximum_vertex_occurrences
        || limits.max_shared_pairs > HARD_MAX_SHARED_PAIRS_V2
        || limits.max_collision_leaves == 0
        || limits.max_collision_leaves > HARD_MAX_RELIEF_LEAVES_V2
        || limits.max_collision_depth >= 64
        || limits.max_collision_depth == 0
        || limits.max_sqrt_operations_per_call == 0
        || limits.max_sqrt_operations_per_call > HARD_MAX_SQRT_OPERATIONS_V2
        || limits.max_exact_value_bits == 0
        || limits.max_exact_value_bits > HARD_MAX_EXACT_VALUE_BITS_V2
        || input.hinge_policies.len() > limits.max_hinge_policy_records
        || input.vertex_policies.len() > limits.max_vertex_policy_records
        || input.ordinary.excluded_shared_pairs.len() > limits.max_shared_pairs
        || input.ordinary.geometry.face_ids().len() != profile_actual.face_count_v2()
        || input.ordinary.geometry.hinges().len() != profile_actual.hinge_count_v2()
    {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    let face_count = input.ordinary.geometry.face_ids().len();
    // Phase 3E intentionally proves the canonical quadrilateral material
    // model. Every structural formula below (membership, clipping, carriers,
    // and projection work) depends on four boundary vertices, so reject any
    // broader polygonal input before evaluating those formulas or allocating
    // proof carriers.
    for face in input.ordinary.geometry.face_ids() {
        relief_checkpoint_v2(checkpoint)?;
        if input
            .ordinary
            .geometry
            .face_boundary_vertices(*face)
            .is_none_or(|boundary| boundary.len() != 4)
        {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
    }
    let total_pairs = face_count
        .checked_mul(face_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    if total_pairs > HARD_MAX_SHARED_PAIRS_V2
        || input.ordinary.geometry.hinges().len() > limits.max_hinge_policy_records
    {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    let shared_count = input.ordinary.excluded_shared_pairs.len();
    let hinge_count = input.ordinary.geometry.hinges().len();
    let required_membership = total_pairs
        .checked_mul(16)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let required_hinge_tests = shared_count
        .checked_mul(hinge_count)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let required_convexity = shared_count
        .checked_mul(4)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let required_carrier_vertices = charged_rest_carrier_vertices_v2(input)?;
    let required_clip_operations = charged_exact_clip_operations_v2(input)?;
    let required_sqrt_calls = shared_count
        .checked_mul(4)
        .and_then(|value| {
            hinge_count
                .checked_mul(3)
                .and_then(|hinges| value.checked_sub(hinges))
        })
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let required_pair_nodes = charged_pair_node_tests_v2(input)?;
    let exact_scratch = exact_scratch_upper_bound_v2(input.limits.max_exact_value_bits)?;
    let carrier_upper = carrier_upper_bound_v2(input)?;
    let publication = publication_bytes_v2()?;
    if limits.max_pair_membership_tests < required_membership
        || limits.max_pair_hinge_tests < required_hinge_tests
        || limits.max_convexity_segment_tests < required_convexity
        || limits.max_rest_carrier_vertices < required_carrier_vertices
        || limits.max_exact_clip_operations < required_clip_operations
        || limits.max_sqrt_calls < required_sqrt_calls
        || limits.max_shared_pair_node_tests < required_pair_nodes
        || limits.max_exact_scratch_bytes < exact_scratch
        || limits.max_publication_bytes < publication
    {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    let mut occurrences = 0usize;
    for record in input.vertex_policies {
        relief_checkpoint_v2(checkpoint)?;
        if record.incident_faces.len() > limits.max_vertex_incident_face_occurrences {
            return Err(ReliefAggregateErrorV2::ResourceLimit);
        }
        occurrences = occurrences
            .checked_add(record.incident_faces.len())
            .filter(|value| *value <= limits.max_vertex_incident_face_occurrences)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    }
    let required_policy_work = hinge_count
        .saturating_sub(1)
        .checked_add(input.vertex_policies.len().saturating_sub(1))
        .and_then(|value| value.checked_add(hinge_count))
        .and_then(|value| value.checked_add(input.vertex_policies.len()))
        .and_then(|value| {
            input
                .vertex_policies
                .len()
                .checked_mul(face_count.checked_mul(5)?)
                .and_then(|scan| value.checked_add(scan))
        })
        .and_then(|value| value.checked_add(occurrences))
        // The outer whole-parent preflight and the classification preflight
        // each validate every quadrilateral and inspect every vertex-policy
        // incident-list length. Classification additionally counts incident
        // occurrences, initializes its use ledger, and checks it at the end.
        .and_then(|value| {
            face_count
                .checked_mul(2)
                .and_then(|work| value.checked_add(work))
        })
        .and_then(|value| {
            input
                .vertex_policies
                .len()
                .checked_mul(5)
                .and_then(|work| value.checked_add(work))
        })
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let required_hash_work = charged_hash_work_v2(input, occurrences)?;
    let required_conversion = charged_carrier_conversion_work_v2(input)?;
    let required_projection = charged_axis_projection_work_v2(input)?;
    let required_logical = [
        required_membership,
        required_hinge_tests,
        required_policy_work,
        required_convexity,
        required_clip_operations,
        required_pair_nodes,
        required_projection,
        required_conversion,
        required_hash_work,
        required_sqrt_calls
            .checked_mul(limits.max_sqrt_operations_per_call)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    if limits.max_scope_and_policy_validation_work < required_policy_work
        || limits.max_hash_work < required_hash_work
        || limits.max_carrier_conversion_work < required_conversion
        || limits.max_axis_projection_work < required_projection
        || limits.max_logical_work < required_logical
        || carrier_upper == 0
    {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    Ok(())
}
