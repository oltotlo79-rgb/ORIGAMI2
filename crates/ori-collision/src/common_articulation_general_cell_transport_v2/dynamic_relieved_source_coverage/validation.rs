//! Private replay, coverage, resource, and binding validation.

use std::mem::size_of;

use ori_foldability::{
    GlobalFlatFoldabilityModelId, GlobalFlatFoldabilityProvenance,
    GlobalFlatLayerOrderSourceAuthorityV2,
};
use sha2::{Digest, Sha256};

use super::*;
use crate::common_articulation_general_cell_transport_v2::validation::{
    ValidatedAuthenticatedLayerSourceV2, validate_authenticated_layer_source_v2,
};
use crate::{
    CommonArticulationGeneralCellTransportErrorV2, CommonArticulationGeneralCellTransportStopV2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ValidatedCoverageV2 {
    pub(super) source: ValidatedAuthenticatedLayerSourceV2,
    pub(super) resources: CoverageResourcesV2,
    pub(super) binding_fingerprint: [u8; 32],
}

pub(super) fn validate_coverage_v2(
    clearance: &CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    live: CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'_>,
    source_authority: &GlobalFlatLayerOrderSourceAuthorityV2<'_>,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
    >,
) -> Result<ValidatedCoverageV2, CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2>
{
    preflight_limits_v2(&live, limits)?;
    let mut source_checkpoint = || checkpoint().map_err(map_coverage_stop_to_transport_v2);
    let source = validate_authenticated_layer_source_v2(
        live.geometry,
        live.decomposition,
        live.profile,
        source_authority,
        limits.source_limits_v2(),
        &mut source_checkpoint,
    )
    .map_err(map_source_error_v2)?;
    checkpoint_v2(checkpoint)?;

    // `validate_authenticated_layer_source_v2` has just traversed every
    // directed pair, requiring distinct endpoints, canonical material-registry
    // membership, canonical issuer order, no reversed unordered duplicate, and
    // nonempty canonical supporting-cell membership. Thus this cardinality
    // check is the final subset-to-complete-domain join, not the pair proof by
    // itself.
    let total_face_pairs = checked_unordered_pairs_v2(source.metrics.material_faces)?;
    if clearance.actual_block_count_v2() != live.profile.actual_block_count_v2()
        || clearance.total_face_pairs_v2() != total_face_pairs
        || source.metrics.face_pair_orders > total_face_pairs
    {
        return Err(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::CertificateBindingMismatch,
        );
    }
    let resources = coverage_resources_v2(clearance, source.metrics, limits)?;
    revalidate_clearance_v2(clearance, live, checkpoint)?;
    checkpoint_v2(checkpoint)?;
    let binding_fingerprint =
        coverage_binding_v2(clearance, source, resources, limits, checkpoint)?;
    Ok(ValidatedCoverageV2 {
        source,
        resources,
        binding_fingerprint,
    })
}

fn preflight_limits_v2(
    live: &CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'_>,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    if coverage_limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
    {
        return Err(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit,
        );
    }
    let configured = live.profile.configured_max_blocks_v2();
    let actual = live.profile.actual_block_count_v2();
    if configured < GENERAL_N_MIN_BLOCKS_V2
        || actual < GENERAL_N_MIN_BLOCKS_V2
        || actual > configured
        || limits.max_blocks != configured
        || live.decomposition.actual_block_count_v2() != actual
        || live.geometry.face_ids().len() != live.profile.actual_v2().face_count_v2()
    {
        return Err(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit,
        );
    }
    Ok(())
}

fn coverage_resources_v2(
    clearance: &CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    source: SourceMetricsV2,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) -> Result<CoverageResourcesV2, CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2>
{
    checked_coverage_resources_v2(
        clearance.actual_block_count_v2(),
        clearance.aggregate_peak_bytes_upper_bound_v2(),
        source,
        limits,
    )
}

fn checked_coverage_resources_v2(
    actual_block_count: usize,
    clearance_peak_bytes: usize,
    source: SourceMetricsV2,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) -> Result<CoverageResourcesV2, CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2>
{
    let publication_bytes =
        size_of::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
    // `publication_bytes` contains the resident original Phase 3F certificate
    // and the complete outer seal. `clearance_peak_bytes` is Phase 3F's proof
    // peak, including its newly generated candidate evidence. Adding the two
    // therefore charges both simultaneously during replay. The borrowed source
    // remains caller-resident throughout that replay, so its complete charged
    // bytes are additive rather than one arm of the phase maximum. The 1 KiB
    // shell/workspace charge is also additive: it conservatively keeps the
    // authority/input shells, SHA-256 state, validated metrics, and checkpoint
    // stack resident instead of relying on a phase-local stack definition.
    let aggregate_peak_bytes = publication_bytes
        .checked_add(source.charged_source_bytes)
        .and_then(|value| value.checked_add(COVERAGE_WORKSPACE_BYTES_V2))
        .and_then(|value| value.checked_add(clearance_peak_bytes))
        .ok_or(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit,
        )?;
    if actual_block_count > limits.max_blocks
        || source.charged_source_bytes > limits.max_source_retained_bytes
        || source.material_faces > limits.max_material_faces
        || source.folded_faces > limits.max_folded_faces
        || source.overlap_cells > limits.max_overlap_cells
        || source.face_pair_orders > limits.max_face_pair_orders
        || source.global_order_faces > limits.max_global_order_faces
        || source.layer_records > limits.max_layer_records
        || source.boundary_vertices > limits.max_boundary_vertices
        || source.traversal_work > limits.max_source_logical_work
        || publication_bytes > limits.max_publication_bytes
        || aggregate_peak_bytes > limits.max_aggregate_peak_bytes
    {
        return Err(
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit,
        );
    }
    Ok(CoverageResourcesV2 {
        source_logical_work: source.traversal_work,
        source_retained_bytes: source.charged_source_bytes,
        clearance_peak_bytes,
        publication_bytes,
        aggregate_peak_bytes,
    })
}

fn revalidate_clearance_v2(
    clearance: &CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    live: CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
    >,
) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    clearance
        .revalidate_with_checkpoint_v2(live, || {
            checkpoint().map_err(|stop| match stop {
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled => {
                    CommonArticulationDynamicGeneralNRelievedClearanceStopV2::Cancelled
                }
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded => {
                    CommonArticulationDynamicGeneralNRelievedClearanceStopV2::DeadlineExceeded
                }
            })
        })
        .map_err(map_clearance_error_v2)
}

fn coverage_binding_v2(
    clearance: &CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    source: ValidatedAuthenticatedLayerSourceV2,
    resources: CoverageResourcesV2,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
    >,
) -> Result<[u8; 32], CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    let mut hash = Sha256::new();
    hash.update(
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_SOURCE_ORDER_COVERAGE_MODEL_ID_V2.as_bytes(),
    );
    hash.update(source.digest);
    hash_provenance_v2(&mut hash, source.provenance)?;
    for value in [
        clearance.actual_block_count_v2(),
        clearance.total_face_pairs_v2(),
        clearance.ordinary_face_pairs_v2(),
        clearance.shared_hinge_pairs_v2(),
        clearance.shared_vertex_pairs_v2(),
        source.metrics.material_faces,
        source.metrics.folded_faces,
        source.metrics.overlap_cells,
        source.metrics.face_pair_orders,
        source.metrics.global_order_faces,
        source.metrics.layer_records,
        source.metrics.boundary_vertices,
        source.metrics.boundary_layer_products,
        source.metrics.projected_source_bytes,
        source.metrics.charged_source_bytes,
        source.metrics.traversal_work,
        resources.source_logical_work,
        resources.source_retained_bytes,
        resources.clearance_peak_bytes,
        resources.publication_bytes,
        resources.aggregate_peak_bytes,
    ] {
        checkpoint_v2(checkpoint)?;
        hash_usize_v2(&mut hash, value)?;
    }
    for value in coverage_limit_values_v2(limits) {
        checkpoint_v2(checkpoint)?;
        hash_usize_v2(&mut hash, value)?;
    }
    checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

fn coverage_limit_values_v2(
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) -> [usize; 12] {
    [
        limits.max_blocks,
        limits.max_source_retained_bytes,
        limits.max_material_faces,
        limits.max_folded_faces,
        limits.max_overlap_cells,
        limits.max_face_pair_orders,
        limits.max_global_order_faces,
        limits.max_layer_records,
        limits.max_boundary_vertices,
        limits.max_source_logical_work,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
}

fn hash_provenance_v2(
    hash: &mut Sha256,
    provenance: GlobalFlatFoldabilityProvenance,
) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    hash.update([match provenance.model_id {
        GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1 => 1,
    }]);
    let namespace = provenance.identity_namespace.ok_or(
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::SourceBindingMismatch,
    )?;
    let fingerprint = provenance.source_fingerprint.ok_or(
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::SourceBindingMismatch,
    )?;
    hash.update(namespace.canonical_bytes());
    hash.update(provenance.source_revision.to_le_bytes());
    hash.update(fingerprint.0);
    Ok(())
}

fn checked_unordered_pairs_v2(
    face_count: usize,
) -> Result<usize, CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    face_count
        .checked_mul(face_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit)
}

fn hash_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    let value = u64::try_from(value).map_err(|_| {
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit
    })?;
    hash.update(value.to_le_bytes());
    Ok(())
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
    >,
) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
    checkpoint().map_err(map_stop_v2)
}

const fn map_stop_v2(
    stop: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2 {
    match stop {
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled
        }
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded
        }
    }
}

const fn map_coverage_stop_to_transport_v2(
    stop: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
) -> CommonArticulationGeneralCellTransportStopV2 {
    match stop {
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled => {
            CommonArticulationGeneralCellTransportStopV2::Cancelled
        }
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded => {
            CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded
        }
    }
}

const fn map_source_error_v2(
    error: CommonArticulationGeneralCellTransportErrorV2,
) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2 {
    match error {
        CommonArticulationGeneralCellTransportErrorV2::InvalidInput => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::InvalidInput
        }
        CommonArticulationGeneralCellTransportErrorV2::ResourceLimit => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit
        }
        CommonArticulationGeneralCellTransportErrorV2::Cancelled => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled
        }
        CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded
        }
        CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch
        | CommonArticulationGeneralCellTransportErrorV2::Clearance(_)
        | CommonArticulationGeneralCellTransportErrorV2::PrerequisiteBindingMismatch => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::SourceBindingMismatch
        }
    }
}

const fn map_clearance_error_v2(
    error: CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2 {
    match error {
        CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::Cancelled => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled
        }
        CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded
        }
        error => {
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Clearance(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_equation_accepts_exact_and_rejects_every_one_short_cap() {
        let source = SourceMetricsV2 {
            material_faces: 11,
            folded_faces: 12,
            overlap_cells: 13,
            face_pair_orders: 14,
            global_order_faces: 15,
            layer_records: 16,
            boundary_vertices: 17,
            boundary_layer_products: 18,
            projected_source_bytes: 96,
            charged_source_bytes: 128,
            traversal_work: 19,
        };
        let publication =
            size_of::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
        let clearance_peak = 2_048;
        let aggregate = publication + 128 + COVERAGE_WORKSPACE_BYTES_V2 + clearance_peak;
        let exact = CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
            max_blocks: 33,
            max_source_retained_bytes: 128,
            max_material_faces: 11,
            max_folded_faces: 12,
            max_overlap_cells: 13,
            max_face_pair_orders: 14,
            max_global_order_faces: 15,
            max_layer_records: 16,
            max_boundary_vertices: 17,
            max_source_logical_work: 19,
            max_publication_bytes: publication,
            max_aggregate_peak_bytes: aggregate,
        };
        let resources = checked_coverage_resources_v2(33, clearance_peak, source, exact)
            .expect("exact resource envelope");
        assert_eq!(resources.aggregate_peak_bytes, aggregate);

        for field in 0..12 {
            let mut one_short = exact;
            let cap = match field {
                0 => &mut one_short.max_blocks,
                1 => &mut one_short.max_source_retained_bytes,
                2 => &mut one_short.max_material_faces,
                3 => &mut one_short.max_folded_faces,
                4 => &mut one_short.max_overlap_cells,
                5 => &mut one_short.max_face_pair_orders,
                6 => &mut one_short.max_global_order_faces,
                7 => &mut one_short.max_layer_records,
                8 => &mut one_short.max_boundary_vertices,
                9 => &mut one_short.max_source_logical_work,
                10 => &mut one_short.max_publication_bytes,
                11 => &mut one_short.max_aggregate_peak_bytes,
                _ => unreachable!(),
            };
            *cap -= 1;
            assert_eq!(
                checked_coverage_resources_v2(33, clearance_peak, source, one_short),
                Err(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit),
                "cap {field} one-short"
            );
        }
    }

    #[test]
    fn coverage_stop_mapping_is_exact() {
        for (stop, expected) in [
            (
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled,
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled,
            ),
            (
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded,
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded,
            ),
        ] {
            assert_eq!(checkpoint_v2(&mut || Err(stop)), Err(expected));
        }
    }

    #[test]
    fn phase3g_preserves_phase3f_certificate_binding_mismatch() {
        let mismatch =
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::CertificateBindingMismatch;
        assert_eq!(
            map_clearance_error_v2(mismatch),
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Clearance(
                mismatch
            )
        );
    }
}
