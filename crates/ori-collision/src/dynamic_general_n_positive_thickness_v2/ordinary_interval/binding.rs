//! Digest construction for private replay material.

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn audit_binding_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<[u8; 32], OrdinaryIntervalErrorV2> {
    let mut hash = Sha256::new();
    hash.update(b"origami2/dynamic-general-n/ordinary-interval-audit/v2");
    for face in audit.faces() {
        checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    for edge in audit.spanning_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    for edge in audit.closure_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    for value in [
        audit.faces().len(),
        audit.spanning_hinges().len(),
        audit.closure_hinges().len(),
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

pub(super) fn binding_fingerprint_v2(
    input: &OrdinaryIntervalInputV2<'_>,
    validated: &ValidatedInputV2,
    run: &ProofRunV2,
) -> Result<[u8; 32], OrdinaryIntervalErrorV2> {
    let mut hash = Sha256::new();
    hash.update(ORDINARY_INTERVAL_MODEL_ID_V2.as_bytes());
    hash.update(validated.audit_binding);
    hash.update(input.schedule.certificate_binding_fingerprint_v2());
    hash.update(
        validated
            .interval_transform_session
            .bridge_binding_fingerprint_v2(),
    );
    hash.update(validated.excluded_shared_pair_digest);
    hash.update(run.collision_partition_digest);
    hash.update(input.fixed_face.canonical_bytes());
    hash.update(input.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(input.closure_tolerance.to_bits().to_le_bytes());
    for value in [
        run.accepted_leaf_count,
        run.processed_interval_node_count,
        run.certified_ordinary_pair_leaf_count,
        run.root_lower_boundary_accepted_leaf_count,
        run.root_upper_boundary_accepted_leaf_count,
        validated.resources.face_count,
        validated.resources.hinge_count,
        validated.resources.boundary_vertex_occurrences,
        validated.resources.total_face_pairs,
        validated.resources.excluded_shared_pairs,
        validated.resources.ordinary_face_pairs,
        validated.resources.charged_interval_nodes,
        validated.resources.charged_shared_feature_membership_tests,
        validated.resources.charged_ordinary_pair_node_tests,
        validated.resources.charged_axis_tests,
        validated.resources.charged_surface_vertex_visits,
        validated
            .resources
            .charged_interval_registry_validation_work,
        validated
            .resources
            .charged_interval_registry_sort_comparisons,
        validated.resources.charged_bridge_partition_search_work,
        validated.resources.charged_logical_work,
        validated.resources.charged_pending_partition_bytes,
        validated.resources.charged_bridge_retained_bytes,
        validated.resources.charged_bridge_revalidation_peak_bytes,
        validated.resources.charged_schedule_retained_bytes,
        validated.resources.charged_session_shell_bytes,
        validated.resources.charged_session_steady_retained_bytes,
        validated
            .resources
            .charged_bridge_revalidation_phase_peak_bytes,
        validated
            .resources
            .charged_schedule_evaluation_workspace_bytes,
        validated.resources.charged_angle_box_bytes,
        validated
            .resources
            .charged_interval_registry_workspace_bytes,
        validated.resources.charged_interval_registry_retained_bytes,
        validated.resources.charged_leaf_wrapper_overhead_bytes,
        validated.resources.charged_leaf_retained_bytes,
        validated.resources.charged_face_aabb_bytes,
        validated.resources.charged_temporary_bytes,
        validated.resources.charged_publication_bytes,
        validated.resources.charged_aggregate_peak_bytes,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    hash.update(run.maximum_accepted_depth.to_le_bytes());
    hash_limits_v2(&mut hash, input.limits)?;
    Ok(hash.finalize().into())
}

fn hash_limits_v2(
    hash: &mut Sha256,
    limits: OrdinaryIntervalLimitsV2,
) -> Result<(), OrdinaryIntervalErrorV2> {
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
        update_usize_v2(hash, value)?;
    }
    hash.update(limits.max_collision_depth.to_le_bytes());
    hash.update(limits.schedule_limits.max_coefficient_bits.to_le_bytes());
    Ok(())
}
