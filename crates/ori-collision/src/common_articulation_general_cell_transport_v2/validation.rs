//! Live prerequisite replay and domain-separated V2 transport binding.

use sha2::{Digest, Sha256};

use super::*;
use crate::CommonArticulationClearanceErrorV2;

pub(super) struct ValidatedTransportInputV2 {
    pub(super) profile_binding: [u8; 32],
    pub(super) decomposition_binding: [u8; 32],
    pub(super) common_pose_binding: [u8; 32],
    pub(super) block_closure_set_binding: [u8; 32],
    pub(super) whole_parent_closure_binding: [u8; 32],
    pub(super) clearance_binding: [u8; 32],
    pub(super) audit_binding: [u8; 32],
    pub(super) parent_schedule_binding: [u8; 32],
    pub(super) parent_fixed_face: FaceId,
    pub(super) paper_thickness_bits: u64,
    pub(super) closure_tolerance_bits: u64,
    pub(super) actual_block_count: usize,
    pub(super) source_digest: [u8; 32],
    pub(super) source_provenance: ori_foldability::GlobalFlatFoldabilityProvenance,
    pub(super) source_metrics: SourceMetricsV2,
    pub(super) resource: TransportResourceWorkV2,
    pub(super) limits: CommonArticulationGeneralCellTransportLimitsV2,
    pub(super) whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
}

pub(super) fn validate_input_v2(
    input: &CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<ValidatedTransportInputV2, CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint_v2(checkpoint)?;
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.closure_tolerance.is_finite()
        || input.closure_tolerance < 0.0
        || input.closure_tolerance.to_bits() == (-0.0f64).to_bits()
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::InvalidInput);
    }
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || input.limits.max_blocks != configured_max_blocks
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.common_pose.actual_block_count_v2() != actual_block_count
        || input.common_pose.configured_max_blocks_v2() != configured_max_blocks
        || input.whole_parent_closure.actual_block_count_v2() != actual_block_count
        || input.whole_parent_closure.configured_max_blocks_v2() != configured_max_blocks
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit);
    }

    // Reject an unauthenticated, foreign, or over-budget layer source before
    // replaying the independent clearance prerequisite. The source pass only
    // reads immutable transport inputs and is not contingent on clearance
    // success, so this ordering prevents a cheap source failure from buying a
    // full N-block clearance traversal.
    if !input.source_authority.is_current_v2() {
        return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
    }
    let source_provenance = input.source_authority.provenance_v2();
    if !geometry_matches_source_provenance_v2(input.geometry, source_provenance) {
        return Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch);
    }
    let (source_digest, source_metrics) = source_binding::source_digest_and_metrics_v2(
        input.source_authority.layer_order_snapshot_v2(),
        input.geometry,
        input.decomposition,
        input.profile,
        source_provenance,
        input.limits,
        checkpoint,
    )?;

    // The sealed prerequisite retains its complete clearance replay envelope.
    // Admit that envelope against transport caps before asking it to regenerate
    // any N-block candidate, so a too-small transport request fails without
    // paying the independently expensive clearance replay.
    let resource = checked_transport_resource_work_v2(
        actual_block_count,
        source_metrics,
        input.clearance.logical_work_v2(),
        input.clearance.storage_bytes_upper_bound_v2(),
        input.whole_parent_closure.parent_closure_leaves_v2(),
        input.limits,
    )?;
    revalidate_clearance_v2(input, checkpoint)?;
    let profile_binding = input.profile.binding_fingerprint_v2();
    let decomposition_binding = input.decomposition.binding_fingerprint_v2();
    let common_pose_binding = input.common_pose.binding_fingerprint_v2();
    let block_closure_set_binding = input.block_closure_set.binding_fingerprint_v2();
    let whole_parent_closure_binding = input.whole_parent_closure.binding_fingerprint_v2();
    let clearance_binding = input.clearance.binding_fingerprint_v2();
    let audit_binding = input.clearance.audit_binding_fingerprint_v2();
    let parent_schedule_binding = input.parent_schedule.certificate_binding_fingerprint_v2();
    if input.clearance.profile_binding_fingerprint_v2() != profile_binding
        || input.clearance.decomposition_binding_fingerprint_v2() != decomposition_binding
        || input.clearance.common_pose_binding_fingerprint_v2() != common_pose_binding
        || input.clearance.block_closure_set_binding_fingerprint_v2() != block_closure_set_binding
        || input
            .clearance
            .whole_parent_closure_binding_fingerprint_v2()
            != whole_parent_closure_binding
        || input.clearance.parent_schedule_binding_fingerprint_v2() != parent_schedule_binding
        || input.clearance.parent_fixed_face_v2() != input.parent_fixed_face
        || input.clearance.paper_thickness_mm_v2().to_bits() != input.paper_thickness_mm.to_bits()
        || input.clearance.closure_tolerance_v2().to_bits() != input.closure_tolerance.to_bits()
        || input.clearance.actual_block_count_v2() != actual_block_count
        || input.clearance.whole_parent_closure_limits_v2() != input.whole_parent_closure_limits
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::PrerequisiteBindingMismatch);
    }

    checkpoint_v2(checkpoint)?;
    Ok(ValidatedTransportInputV2 {
        profile_binding,
        decomposition_binding,
        common_pose_binding,
        block_closure_set_binding,
        whole_parent_closure_binding,
        clearance_binding,
        audit_binding,
        parent_schedule_binding,
        parent_fixed_face: input.parent_fixed_face,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        actual_block_count,
        source_digest,
        source_provenance,
        source_metrics,
        resource,
        limits: input.limits,
        whole_parent_closure_limits: input.whole_parent_closure_limits,
    })
}

/// Confines the three-part source-origin comparison to the transport
/// authentication boundary.  A matching fold model and revision alone are
/// insufficient: the complete face registry must also be derived under the
/// same material-sheet namespace.
pub(super) fn geometry_matches_source_provenance_v2(
    geometry: &MaterialHingeGraphGeometry,
    source_provenance: ori_foldability::GlobalFlatFoldabilityProvenance,
) -> bool {
    let (Some(source_namespace), Some(source_fingerprint)) = (
        source_provenance.identity_namespace,
        source_provenance.source_fingerprint,
    ) else {
        return false;
    };
    geometry.fold_model_fingerprint_v1() == Some(source_fingerprint.0)
        && geometry.source_revision_v1() == Some(source_provenance.source_revision)
        && geometry.source_identity_namespace_v1() == Some(source_namespace)
}

fn revalidate_clearance_v2(
    input: &CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    let mut clearance_checkpoint = || {
        checkpoint().map_err(|stop| match stop {
            CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                CommonArticulationClearanceStopV2::Cancelled
            }
            CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                CommonArticulationClearanceStopV2::DeadlineExceeded
            }
        })
    };
    input
        .clearance
        .revalidate_with_checkpoint_v2(
            CommonArticulationClearanceRevalidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                pose: input.pose,
                decomposition: input.decomposition,
                common_pose: input.common_pose,
                parent_fixed_face: input.parent_fixed_face,
                parent_schedule: input.parent_schedule,
                profile: input.profile,
                paper_thickness_mm: input.paper_thickness_mm,
                closure_tolerance: input.closure_tolerance,
                block_closure_set: input.block_closure_set,
                whole_parent_closure: input.whole_parent_closure,
                whole_parent_closure_limits: input.whole_parent_closure_limits,
            },
            &mut clearance_checkpoint,
        )
        .map_err(map_clearance_error_v2)
}

fn map_clearance_error_v2(
    error: CommonArticulationClearanceErrorV2,
) -> CommonArticulationGeneralCellTransportErrorV2 {
    match error {
        CommonArticulationClearanceErrorV2::Cancelled => {
            CommonArticulationGeneralCellTransportErrorV2::Cancelled
        }
        CommonArticulationClearanceErrorV2::DeadlineExceeded => {
            CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded
        }
        error => CommonArticulationGeneralCellTransportErrorV2::Clearance(error),
    }
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationGeneralCellTransportStopV2::Cancelled => {
            CommonArticulationGeneralCellTransportErrorV2::Cancelled
        }
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
            CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded
        }
    })
}

pub(super) fn transport_binding_fingerprint_v2(
    value: &ValidatedTransportInputV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<[u8; 32], CommonArticulationGeneralCellTransportErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_MODEL_ID_V2.as_bytes());
    for binding in [
        value.profile_binding,
        value.decomposition_binding,
        value.common_pose_binding,
        value.block_closure_set_binding,
        value.whole_parent_closure_binding,
        value.clearance_binding,
        value.audit_binding,
        value.parent_schedule_binding,
        value.source_digest,
    ] {
        checkpoint_v2(checkpoint)?;
        hash.update(binding);
    }
    hash.update(value.parent_fixed_face.canonical_bytes());
    hash.update(value.paper_thickness_bits.to_le_bytes());
    hash.update(value.closure_tolerance_bits.to_le_bytes());
    hash_source_provenance_v2(&mut hash, value.source_provenance)?;
    for number in [
        value.actual_block_count,
        value.source_metrics.material_faces,
        value.source_metrics.folded_faces,
        value.source_metrics.overlap_cells,
        value.source_metrics.face_pair_orders,
        value.source_metrics.global_order_faces,
        value.source_metrics.layer_records,
        value.source_metrics.boundary_vertices,
        value.source_metrics.boundary_layer_products,
        value.source_metrics.projected_source_bytes,
        value.source_metrics.charged_source_bytes,
        value.source_metrics.traversal_work,
        value.resource.transitions,
        value.resource.layer_records,
        value.resource.boundary_vertices,
        value.resource.boundary_samples,
        value.resource.logical_work,
        value.resource.retained_bytes,
        value.resource.peak_bytes,
    ] {
        checkpoint_v2(checkpoint)?;
        hash_usize_v2(&mut hash, number)?;
    }
    hash_transport_limits_v2(&mut hash, value.limits)?;
    hash_whole_parent_limits_v2(&mut hash, value.whole_parent_closure_limits)?;
    checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

fn hash_transport_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationGeneralCellTransportLimitsV2,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    for value in [
        limits.max_blocks,
        limits.max_source_retained_bytes,
        limits.max_material_faces,
        limits.max_folded_faces,
        limits.max_overlap_cells,
        limits.max_face_pair_orders,
        limits.max_global_order_faces,
        limits.max_layer_records,
        limits.max_boundary_vertices,
        limits.max_boundary_samples,
        limits.max_transitions,
        limits.max_logical_work,
        limits.max_retained_bytes,
        limits.max_peak_bytes,
    ] {
        hash_usize_v2(hash, value)?;
    }
    Ok(())
}

fn hash_source_provenance_v2(
    hash: &mut Sha256,
    provenance: ori_foldability::GlobalFlatFoldabilityProvenance,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash.update([match provenance.model_id {
        ori_foldability::GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1 => 1,
    }]);
    let namespace = provenance
        .identity_namespace
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)?;
    let fingerprint = provenance
        .source_fingerprint
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)?;
    hash.update(namespace.canonical_bytes());
    hash.update(provenance.source_revision.to_le_bytes());
    hash.update(fingerprint.0);
    Ok(())
}

fn hash_whole_parent_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationWholeParentClosureLimitsV2,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    let block = limits.block_closure_set_limits;
    let block_dyadic = block.per_block_closure_limits;
    let block_schedule = block_dyadic.schedule_limits;
    let parent_dyadic = limits.parent_closure_limits;
    let parent_schedule = parent_dyadic.schedule_limits;
    for value in [
        block.max_blocks,
        block.max_parent_schedule_bytes,
        block.max_block_schedule_bytes,
        block.max_total_block_schedule_bytes,
        block.max_block_closure_bytes,
        block.max_total_block_closure_bytes,
        block.max_total_closure_leaves,
        block_dyadic.max_depth as usize,
        block_dyadic.max_leaves,
        block_dyadic.max_work,
        block_schedule.max_hinges,
        block_schedule.max_degree,
        block_schedule.max_coefficient_bits as usize,
        block_schedule.max_work,
        limits.max_parent_schedule_bytes,
        limits.max_parent_closure_bytes,
        limits.max_parent_closure_leaves,
        parent_dyadic.max_depth as usize,
        parent_dyadic.max_leaves,
        parent_dyadic.max_work,
        parent_schedule.max_hinges,
        parent_schedule.max_degree,
        parent_schedule.max_coefficient_bits as usize,
        parent_schedule.max_work,
    ] {
        hash_usize_v2(hash, value)?;
    }
    Ok(())
}

fn hash_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
