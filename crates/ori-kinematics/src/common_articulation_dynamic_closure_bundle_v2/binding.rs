use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::CanonicalCycleScheduleV1;
use crate::graph::{
    DyadicIntervalClosureWorkspaceResourcesV2, WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
};
use crate::schedule::CycleScheduleRestrictionWorkspaceResourcesV2;

use super::{resources::BundleValidationMeterV2, *};

pub(super) fn geometry_audit_binding_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
    meter: &mut BundleValidationMeterV2,
) -> Result<[u8; 32], CommonArticulationDynamicClosureBundleErrorV2> {
    if geometry.face_ids().len() != audit.faces().len() {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }
    for (geometry_face, audit_face) in geometry.face_ids().iter().zip(audit.faces()) {
        meter.poll(checkpoint)?;
        if geometry_face != audit_face {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
        }
    }
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_DYNAMIC_CLOSURE_GEOMETRY_AUDIT_BINDING_V2");
    hash_count_v2(&mut hash, geometry.face_ids().len())?;
    for face in geometry.face_ids() {
        meter.poll(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    hash_count_v2(&mut hash, geometry.hinges().len())?;
    for hinge in geometry.hinges() {
        meter.poll(checkpoint)?;
        hash.update(hinge.edge().canonical_bytes());
        hash.update(hinge.left_face().canonical_bytes());
        hash.update(hinge.right_face().canonical_bytes());
        hash.update([match hinge.assignment() {
            ori_topology::FoldAssignment::Mountain => 0,
            ori_topology::FoldAssignment::Valley => 1,
        }]);
        for value in [
            hinge.start().x(),
            hinge.start().y(),
            hinge.start().z(),
            hinge.end().x(),
            hinge.end().y(),
            hinge.end().z(),
            hinge.axis().x(),
            hinge.axis().y(),
            hinge.axis().z(),
        ] {
            meter.poll(checkpoint)?;
            hash.update(value.to_bits().to_be_bytes());
        }
    }
    for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
        meter.poll(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

fn hash_count_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?
            .to_be_bytes(),
    );
    Ok(())
}

fn hash_restriction_limits_v2(
    hash: &mut Sha256,
    limits: CycleScheduleRestrictionWorkspaceLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    for value in [
        limits.max_work,
        limits.max_restricted_schedule_retained_bytes,
        limits.max_restriction_peak_bytes,
    ] {
        hash_count_v2(hash, value)?;
    }
    Ok(())
}

fn hash_closure_limits_v2(
    hash: &mut Sha256,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    hash.update(limits.max_depth.to_be_bytes());
    for value in [
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
    ] {
        hash_count_v2(hash, value)?;
    }
    hash.update(limits.schedule_limits.max_coefficient_bits.to_be_bytes());
    Ok(())
}

fn hash_bundle_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    for value in [
        limits.max_blocks,
        limits.max_validation_work,
        limits.max_block_record_bytes,
        limits.max_total_restriction_work,
        limits.max_total_restricted_schedule_retained_bytes,
        limits.max_total_block_closure_retained_bytes,
        limits.max_total_block_leaves,
        limits.max_parent_schedule_retained_bytes,
        limits.max_parent_closure_retained_bytes,
        limits.max_parent_leaves,
        limits.max_bundle_retained_bytes,
        limits.max_issuance_peak_bytes,
        limits.max_revalidation_peak_bytes,
    ] {
        hash_count_v2(hash, value)?;
    }
    hash_restriction_limits_v2(hash, limits.block_restriction_limits)?;
    hash_restriction_limits_v2(hash, limits.parent_schedule_restriction_limits)?;
    hash_closure_limits_v2(hash, limits.per_block_closure_limits)?;
    hash_closure_limits_v2(hash, limits.parent_closure_limits)
}

fn hash_restriction_resources_v2(
    hash: &mut Sha256,
    resources: CycleScheduleRestrictionWorkspaceResourcesV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    for value in [
        resources.charged_work,
        resources.charged_restricted_schedule_retained_upper_bound_bytes,
        resources.charged_restriction_peak_upper_bound_bytes,
    ] {
        hash_count_v2(hash, value)?;
    }
    Ok(())
}

fn hash_closure_resources_v2(
    hash: &mut Sha256,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    for value in [
        resources.charged_binding_validation_upper_bound_bytes,
        resources.charged_theorem_recognizer_work,
        resources.charged_theorem_recognizer_upper_bound_bytes,
        resources.charged_carrier_index_workspace_upper_bound_bytes,
        resources.charged_schedule_evaluation_workspace_upper_bound_bytes,
        resources.charged_big_rational_payload_upper_bound_bytes,
        resources.charged_exact_rational_object_upper_bound_bytes,
        resources.charged_interval_closure_workspace_upper_bound_bytes,
        resources.charged_partition_workspace_upper_bound_bytes,
        resources.charged_retained_material_upper_bound_bytes,
        resources.charged_publication_workspace_upper_bound_bytes,
        resources.charged_peak_workspace_upper_bound_bytes,
        resources.visited_partition_nodes,
        resources.issued_leaves,
    ] {
        hash_count_v2(hash, value)?;
    }
    Ok(())
}

fn hash_bundle_resources_v2(
    hash: &mut Sha256,
    resources: CommonArticulationDynamicClosureBundleResourcesV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    for value in [
        resources.charged_block_record_bytes,
        resources.charged_validation_work,
        resources.charged_total_restriction_work,
        resources.charged_total_restricted_schedule_retained_upper_bound_bytes,
        resources.charged_total_block_closure_retained_upper_bound_bytes,
        resources.charged_total_block_leaves,
        resources.charged_parent_schedule_retained_upper_bound_bytes,
        resources.charged_parent_closure_retained_upper_bound_bytes,
        resources.charged_parent_leaves,
        resources.charged_max_block_restriction_peak_upper_bound_bytes,
        resources.charged_max_block_closure_peak_upper_bound_bytes,
        resources.charged_parent_schedule_restriction_peak_upper_bound_bytes,
        resources.charged_parent_closure_peak_upper_bound_bytes,
        resources.charged_bundle_retained_upper_bound_bytes,
        resources.charged_issuance_peak_upper_bound_bytes,
        resources.charged_revalidation_peak_upper_bound_bytes,
    ] {
        hash_count_v2(hash, value)?;
    }
    Ok(())
}

fn hash_schedule_material_v2(
    hash: &mut Sha256,
    schedule: &CanonicalCycleScheduleV1,
    resources: CycleScheduleRestrictionWorkspaceResourcesV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    hash.update(schedule.certificate_binding_fingerprint_v2());
    hash.update(schedule.graph_binding_fingerprint_v1());
    hash_restriction_resources_v2(hash, resources)
}

fn hash_closure_material_v2(
    hash: &mut Sha256,
    closure: &WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
) -> Result<(), CommonArticulationDynamicClosureBundleErrorV2> {
    hash.update(closure.partition_binding_fingerprint_v2());
    hash_closure_resources_v2(hash, closure.resources())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn binding_fingerprint_v2(
    input: CommonArticulationDynamicClosureBundleInputV2<'_>,
    audit_binding: [u8; 32],
    blocks: &[DynamicBlockClosureRecordV2],
    parent_schedule: &CanonicalCycleScheduleV1,
    parent_schedule_resources: CycleScheduleRestrictionWorkspaceResourcesV2,
    parent_closure: &WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
    resources: CommonArticulationDynamicClosureBundleResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
) -> Result<[u8; 32], CommonArticulationDynamicClosureBundleErrorV2> {
    checkpoint_v2(checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(BUNDLE_DOMAIN_V2);
    hash.update(input.profile.binding_fingerprint_v2());
    hash.update(input.decomposition.binding_fingerprint_v2());
    hash.update(input.common_pose.binding_fingerprint_v2());
    hash.update(audit_binding);
    hash.update(input.parent_schedule.certificate_binding_fingerprint_v2());
    hash.update(input.parent_fixed_face.canonical_bytes());
    hash.update(input.paper_thickness_mm.to_bits().to_be_bytes());
    hash.update(input.closure_tolerance.to_bits().to_be_bytes());
    for value in [
        input.profile.configured_max_blocks_v2(),
        input.profile.actual_block_count_v2(),
        input.geometry.face_ids().len(),
        input.geometry.hinges().len(),
    ] {
        hash_count_v2(&mut hash, value)?;
    }
    hash_bundle_limits_v2(&mut hash, input.limits)?;
    hash_count_v2(&mut hash, blocks.len())?;
    for record in blocks {
        checkpoint_v2(checkpoint)?;
        hash_count_v2(&mut hash, record.block_index)?;
        hash.update(record.fixed_face.canonical_bytes());
        hash.update(record.geometry_audit_binding);
        hash_schedule_material_v2(
            &mut hash,
            &record.restricted_schedule,
            record.restriction_resources,
        )?;
        hash_closure_material_v2(&mut hash, &record.closure)?;
    }
    checkpoint_v2(checkpoint)?;
    hash_schedule_material_v2(&mut hash, parent_schedule, parent_schedule_resources)?;
    hash_closure_material_v2(&mut hash, parent_closure)?;
    hash_bundle_resources_v2(&mut hash, resources)?;
    checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

pub(super) fn bundles_match_with_checkpoint_v2(
    retained: &CommonArticulationDynamicClosureBundleV2,
    candidate: &CommonArticulationDynamicClosureBundleV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
) -> Result<bool, CommonArticulationDynamicClosureBundleErrorV2> {
    checkpoint_v2(checkpoint)?;
    if retained.issuer_geometry != candidate.issuer_geometry
        || !Arc::ptr_eq(&retained.issuer_pose, &candidate.issuer_pose)
        || retained.profile_binding != candidate.profile_binding
        || retained.decomposition_binding != candidate.decomposition_binding
        || retained.common_pose_binding != candidate.common_pose_binding
        || retained.audit_binding != candidate.audit_binding
        || retained.parent_schedule_binding != candidate.parent_schedule_binding
        || retained.parent_fixed_face != candidate.parent_fixed_face
        || retained.paper_thickness_bits != candidate.paper_thickness_bits
        || retained.closure_tolerance_bits != candidate.closure_tolerance_bits
        || retained.configured_max_blocks != candidate.configured_max_blocks
        || retained.actual_block_count != candidate.actual_block_count
        || retained.face_count != candidate.face_count
        || retained.hinge_count != candidate.hinge_count
        || retained.policy != candidate.policy
        || retained.resources != candidate.resources
        || retained.binding_fingerprint != candidate.binding_fingerprint
        || retained.blocks.len() != candidate.blocks.len()
        || retained.parent_schedule_restriction_resources
            != candidate.parent_schedule_restriction_resources
    {
        return Ok(false);
    }
    for (retained_record, candidate_record) in retained.blocks.iter().zip(&candidate.blocks) {
        checkpoint_v2(checkpoint)?;
        // The bundle fingerprint commits the record index, fixed face, audit,
        // schedule/closure fingerprints, resources, and parent material. The
        // per-block instance identity is intentionally checked separately: a
        // freshly issued but value-equal decomposition is not interchangeable.
        if retained_record.issuer_geometry != candidate_record.issuer_geometry {
            return Ok(false);
        }
    }
    Ok(true)
}
