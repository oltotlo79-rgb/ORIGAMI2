//! Checked resource equations for V2 retained layer-source observations.

use super::*;

const TRANSPORT_BASE_WORK_V2: usize = 96;
const TRANSPORT_BASE_RETAINED_BYTES_V2: usize = 1_024;
const TRANSPORT_WORKSPACE_BYTES_V2: usize = 1_024;

/// Exact resource totals retained beside one source snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TransportResourceWorkV2 {
    pub(super) transitions: usize,
    pub(super) layer_records: usize,
    pub(super) boundary_vertices: usize,
    pub(super) boundary_samples: usize,
    pub(super) logical_work: usize,
    pub(super) retained_bytes: usize,
    pub(super) peak_bytes: usize,
}

/// Applies every explicit caller cap and returns the checked V2 work equation.
///
/// `T = parent_closure_leaves + 1`, `L = sum(cell layer records)`, and
/// `B = sum(cell boundary vertices * cell layer records * T)`. All terms are
/// checked so a malformed source cannot wrap a resource budget into success.
pub(super) fn checked_transport_resource_work_v2(
    actual_block_count: usize,
    source: SourceMetricsV2,
    clearance_logical_work: usize,
    clearance_storage_bytes: usize,
    parent_closure_leaves: usize,
    limits: CommonArticulationGeneralCellTransportLimitsV2,
) -> Result<TransportResourceWorkV2, CommonArticulationGeneralCellTransportErrorV2> {
    let resource = transport_resource_totals_v2(
        source,
        clearance_logical_work,
        clearance_storage_bytes,
        parent_closure_leaves,
    )?;
    if actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > limits.max_blocks
        || source.charged_source_bytes > limits.max_source_retained_bytes
        || source.material_faces > limits.max_material_faces
        || source.folded_faces > limits.max_folded_faces
        || source.overlap_cells > limits.max_overlap_cells
        || source.face_pair_orders > limits.max_face_pair_orders
        || source.global_order_faces > limits.max_global_order_faces
        || source.layer_records > limits.max_layer_records
        || source.boundary_vertices > limits.max_boundary_vertices
        || resource.boundary_samples > limits.max_boundary_samples
        || resource.transitions > limits.max_transitions
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit);
    }
    if resource.logical_work > limits.max_logical_work
        || resource.retained_bytes > limits.max_retained_bytes
        || resource.peak_bytes > limits.max_peak_bytes
    {
        return Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit);
    }
    Ok(resource)
}

/// Computes the checked measured totals before caller caps are applied. This
/// is private to the V2 transport boundary and lets the live N=33 test helper
/// derive exact caps without ever admitting an unlimited issue call.
pub(super) fn transport_resource_totals_v2(
    source: SourceMetricsV2,
    clearance_logical_work: usize,
    clearance_storage_bytes: usize,
    parent_closure_leaves: usize,
) -> Result<TransportResourceWorkV2, CommonArticulationGeneralCellTransportErrorV2> {
    let transitions = parent_closure_leaves
        .checked_add(1)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    let boundary_samples = source
        .boundary_layer_products
        .checked_mul(transitions)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    let logical_work = TRANSPORT_BASE_WORK_V2
        .checked_add(clearance_logical_work)
        .and_then(|value| value.checked_add(source.traversal_work))
        .and_then(|value| value.checked_add(transitions))
        .and_then(|value| value.checked_add(source.layer_records))
        .and_then(|value| value.checked_add(source.boundary_vertices))
        .and_then(|value| value.checked_add(boundary_samples))
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    let retained_bytes = TRANSPORT_BASE_RETAINED_BYTES_V2
        .checked_add(source.charged_source_bytes)
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    // The borrowed live source overlaps both independent phases. During the
    // clone phase it overlaps another source-sized allocation; during
    // clearance replay it overlaps the clearance prerequisite's upper-bound
    // storage. The phases are sequential, so charge their maximum rather than
    // adding them. The checkpoint/hash workspace is conservatively retained.
    let concurrent_phase_bytes = source.charged_source_bytes.max(clearance_storage_bytes);
    let peak_bytes = retained_bytes
        .checked_add(concurrent_phase_bytes)
        .and_then(|value| value.checked_add(TRANSPORT_WORKSPACE_BYTES_V2))
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    Ok(TransportResourceWorkV2 {
        transitions,
        layer_records: source.layer_records,
        boundary_vertices: source.boundary_vertices,
        boundary_samples,
        logical_work,
        retained_bytes,
        peak_bytes,
    })
}
