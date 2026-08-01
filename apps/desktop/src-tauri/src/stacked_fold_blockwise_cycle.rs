//! Fail-closed blockwise current-cycle fallback certification.
//!
//! Command orchestration remains in the parent module. This module owns the
//! deterministic two-block decomposition, continuous-layer transport, and
//! private transaction-premise installation path.

use ori_collision::{
    BlockUnionCompletenessInputV1, CommonArticulationBlockComposedPathInputV1,
    CommonArticulationClearanceInputV1, CommonArticulationClearanceLimitsV1,
    CommonArticulationClearanceOutcomeV1, CommonArticulationContinuousLayerPathInputV1,
    CommonArticulationCrossBlockFacePairV1, CooperativeOperationControlV1,
    CooperativeOperationStopV1, GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
    MultiBlockClosureInputV1, MultiBlockPositiveLayerInputV1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
    certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1,
    certify_general_multi_face_cell_transport_v1, checked_general_cell_transport_memory_work_v1,
    diagnose_block_union_completeness_v1,
    issue_common_articulation_block_composed_path_authority_with_control_v1,
    issue_common_articulation_clearance_prerequisite_with_control_v1,
    issue_common_articulation_continuous_layer_path_authority_with_control_v1,
    issue_common_articulation_pose_authority_with_control_v1,
    issue_complete_multi_block_positive_layer_authority_v1, issue_multi_block_closure_authority_v1,
    issue_multi_block_positive_layer_authority_v1, preflight_general_cell_transport_work_v1,
};
use ori_domain::{FaceId, ProjectId};
use ori_kinematics::{
    CanonicalEdgeBlockLimitsV1, CanonicalMaterialEdgeBlockDecompositionV1, CycleBasisLimitsV1,
    DyadicIntervalClosureLimitsV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    GeneratedMultiHingePathCandidateV1,
};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use super::{
    CANCELLED_MESSAGE, CYCLE_NONCLOSING_MESSAGE, CYCLE_PATH_DEADLINE_MESSAGE,
    CYCLE_PATH_RESOURCE_MESSAGE, CYCLE_PATH_UNCERTIFIED_MESSAGE, CYCLE_PATH_UNSUPPORTED_MESSAGE,
    CurrentAppliedPoseCapability, CurrentCyclePosePreviewResponseV1, CurrentLayerOrderCapability,
    LayerOrderPairDtoV1, STACKED_FOLD_READ_GENERATION, emit_current_cycle_progress_v1,
    production_cycle_schedule_limits_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BlockwiseCycleControlGateErrorV1 {
    Cancelled,
    DeadlineExceeded,
}

pub(super) fn blockwise_cycle_control_gate_v1(
    generation: u64,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), BlockwiseCycleControlGateErrorV1> {
    if STACKED_FOLD_READ_GENERATION.load(std::sync::atomic::Ordering::Acquire) != generation {
        return Err(BlockwiseCycleControlGateErrorV1::Cancelled);
    }
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => BlockwiseCycleControlGateErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            BlockwiseCycleControlGateErrorV1::DeadlineExceeded
        }
    })
}

const fn blockwise_cycle_control_message_v1(
    error: BlockwiseCycleControlGateErrorV1,
) -> &'static str {
    match error {
        BlockwiseCycleControlGateErrorV1::Cancelled => CANCELLED_MESSAGE,
        BlockwiseCycleControlGateErrorV1::DeadlineExceeded => CYCLE_PATH_DEADLINE_MESSAGE,
    }
}

pub(super) fn certified_blockwise_layer_pairs_v1(
    sources: &[Box<ori_foldability::LayerOrderSnapshot>],
    certified_pair_count: usize,
) -> Result<Vec<(FaceId, FaceId)>, String> {
    let pair_capacity = sources.iter().try_fold(0usize, |count, source| {
        count.checked_add(source.face_pair_orders.len())
    });
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(pair_capacity.ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for source in sources {
        for pair in &source.face_pair_orders {
            pairs.push((pair.lower_face.face_id, pair.upper_face.face_id));
        }
    }
    pairs.sort_unstable_by_key(|(lower, upper)| (lower.canonical_bytes(), upper.canonical_bytes()));
    pairs.dedup();
    if pairs.len() != certified_pair_count {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    }
    Ok(pairs)
}

const THREE_BLOCK_CURRENT_CYCLE_ARITY_V1: usize = 3;
const FOUR_BLOCK_CURRENT_CYCLE_ARITY_V1: usize = 4;
const FIVE_BLOCK_CURRENT_CYCLE_ARITY_V1: usize = 5;
const SIX_BLOCK_CURRENT_CYCLE_ARITY_V1: usize = 6;
const SEVEN_BLOCK_CURRENT_CYCLE_ARITY_V1: usize = 7;
const BOUNDED_MULTI_BLOCK_CURRENT_CYCLE_MAX_ARITY_V1: usize = SEVEN_BLOCK_CURRENT_CYCLE_ARITY_V1;
const BOUNDED_MULTI_BLOCK_WHOLE_SOURCE_PEAK_MULTIPLICITY_V1: usize = 3;
const BOUNDED_MULTI_BLOCK_RESTRICTED_SOURCE_PEAK_MULTIPLICITY_V1: usize = 2;

pub(super) const fn bounded_multi_block_current_cycle_arity_supported_v1(
    block_count: usize,
) -> bool {
    block_count >= THREE_BLOCK_CURRENT_CYCLE_ARITY_V1
        && block_count <= BOUNDED_MULTI_BLOCK_CURRENT_CYCLE_MAX_ARITY_V1
}

#[cfg(test)]
static BOUNDED_MULTI_BLOCK_LAYER_PEAK_LIMIT_OVERRIDE_V1: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(usize::MAX);
#[cfg(test)]
static BOUNDED_MULTI_BLOCK_LAYER_SOURCE_CLONE_ATTEMPTS_V1: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub(super) struct BoundedMultiBlockLayerPeakLimitOverrideGuardV1 {
    previous: usize,
}

#[cfg(test)]
impl Drop for BoundedMultiBlockLayerPeakLimitOverrideGuardV1 {
    fn drop(&mut self) {
        BOUNDED_MULTI_BLOCK_LAYER_PEAK_LIMIT_OVERRIDE_V1
            .store(self.previous, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
pub(super) fn override_bounded_multi_block_layer_peak_limit_for_test_v1(
    limit: usize,
) -> BoundedMultiBlockLayerPeakLimitOverrideGuardV1 {
    BoundedMultiBlockLayerPeakLimitOverrideGuardV1 {
        previous: BOUNDED_MULTI_BLOCK_LAYER_PEAK_LIMIT_OVERRIDE_V1
            .swap(limit, std::sync::atomic::Ordering::AcqRel),
    }
}

#[cfg(test)]
pub(super) fn reset_bounded_multi_block_layer_source_clone_attempts_for_test_v1() {
    BOUNDED_MULTI_BLOCK_LAYER_SOURCE_CLONE_ATTEMPTS_V1
        .store(0, std::sync::atomic::Ordering::Release);
}

#[cfg(test)]
pub(super) fn bounded_multi_block_layer_source_clone_attempts_for_test_v1() -> usize {
    BOUNDED_MULTI_BLOCK_LAYER_SOURCE_CLONE_ATTEMPTS_V1.load(std::sync::atomic::Ordering::Acquire)
}

fn production_bounded_multi_block_layer_peak_limit_v1() -> usize {
    #[cfg(test)]
    {
        let overridden = BOUNDED_MULTI_BLOCK_LAYER_PEAK_LIMIT_OVERRIDE_V1
            .load(std::sync::atomic::Ordering::Acquire);
        if overridden != usize::MAX {
            return overridden;
        }
    }
    ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES
}

/// Computes the source-retention peak shared by the exact 3..=7-block paths.
///
/// The multiplicities are independent of block count: the live whole source is
/// retained by the capability, materialized input, and completed whole-parent
/// proof (three copies). Every restricted source is retained by its materialized
/// input and its completed per-block proof (two copies). The sum of restricted
/// source bytes therefore scales with block count, while the multiplicities do
/// not.
pub(super) fn checked_bounded_multi_block_layer_peak_retained_bytes_v1(
    whole_source_retained_bytes: usize,
    restricted_sources_retained_bytes: usize,
) -> Option<usize> {
    whole_source_retained_bytes
        .checked_mul(BOUNDED_MULTI_BLOCK_WHOLE_SOURCE_PEAK_MULTIPLICITY_V1)?
        .checked_add(
            restricted_sources_retained_bytes
                .checked_mul(BOUNDED_MULTI_BLOCK_RESTRICTED_SOURCE_PEAK_MULTIPLICITY_V1)?,
        )
}

pub(super) fn checked_bounded_multi_block_operation_peak_retained_bytes_v1(
    layer_source_peak_bytes: usize,
    proof_retained_bytes: usize,
    peak_temporary_bytes: usize,
) -> Option<usize> {
    layer_source_peak_bytes
        .checked_add(proof_retained_bytes)?
        .checked_add(peak_temporary_bytes)
}

pub(super) fn preflight_bounded_multi_block_layer_peak_retained_bytes_v1(
    peak_retained_bytes: usize,
    maximum_peak_retained_bytes: usize,
) -> Result<(), String> {
    if peak_retained_bytes > maximum_peak_retained_bytes {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BoundedMultiBlockLayerRetainedBytesV1 {
    pub(super) whole_source: usize,
    pub(super) block_sources: Vec<usize>,
    pub(super) proof_retained: usize,
    pub(super) peak_temporary: usize,
    pub(super) peak: usize,
}

impl BoundedMultiBlockLayerRetainedBytesV1 {
    pub(super) fn for_source_v1(
        source: &ori_foldability::LayerOrderSnapshot,
        block_face_sets: &[&[FaceId]],
        proof_retained_bytes: usize,
        peak_temporary_bytes: usize,
    ) -> Result<Self, String> {
        if !bounded_multi_block_current_cycle_arity_supported_v1(block_face_sets.len()) {
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
        }
        let whole_source = source
            .checked_deep_retained_bytes_v1()
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let mut block_sources = Vec::new();
        block_sources
            .try_reserve_exact(block_face_sets.len())
            .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let mut restricted_sources = 0usize;
        for faces in block_face_sets {
            let retained = source
                .checked_restricted_deep_retained_bytes_v1(faces)
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            restricted_sources = restricted_sources
                .checked_add(retained)
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            block_sources.push(retained);
        }
        let layer_source_peak = checked_bounded_multi_block_layer_peak_retained_bytes_v1(
            whole_source,
            restricted_sources,
        )
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let peak = checked_bounded_multi_block_operation_peak_retained_bytes_v1(
            layer_source_peak,
            proof_retained_bytes,
            peak_temporary_bytes,
        )
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        Ok(Self {
            whole_source,
            block_sources,
            proof_retained: proof_retained_bytes,
            peak_temporary: peak_temporary_bytes,
            peak,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct BoundedMultiBlockCellTransportWorkV1 {
    pub(super) transitions: usize,
    pub(super) cells: usize,
    pub(super) layer_records: usize,
    pub(super) boundary_samples: usize,
    pub(super) folded_faces: usize,
    pub(super) maximum_boundary_points: usize,
}

impl BoundedMultiBlockCellTransportWorkV1 {
    fn for_source(
        source: &ori_foldability::LayerOrderSnapshot,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    ) -> Result<Self, String> {
        Self::for_source_with_face_filter_v1(source, closure, None)
    }

    fn for_restricted_source_v1(
        source: &ori_foldability::LayerOrderSnapshot,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        faces: &[FaceId],
    ) -> Result<Self, String> {
        Self::for_source_with_face_filter_v1(source, closure, Some(faces))
    }

    fn for_source_with_face_filter_v1(
        source: &ori_foldability::LayerOrderSnapshot,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        faces: Option<&[FaceId]>,
    ) -> Result<Self, String> {
        let transitions = closure
            .leaves()
            .len()
            .checked_add(1)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let mut cells = 0usize;
        let mut layer_records = 0usize;
        let mut boundary_samples_per_transition = 0usize;
        let mut maximum_boundary_points = 0usize;
        for cell in &source.overlap_cells {
            let retained_layer_records = faces.map_or(cell.bottom_to_top_faces.len(), |faces| {
                cell.bottom_to_top_faces
                    .iter()
                    .filter(|face| faces.contains(face))
                    .count()
            });
            if retained_layer_records == 0 && faces.is_some() {
                continue;
            }
            cells = cells
                .checked_add(1)
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            maximum_boundary_points = maximum_boundary_points.max(cell.exact_boundary.len());
            layer_records = layer_records
                .checked_add(retained_layer_records)
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            boundary_samples_per_transition = cell
                .exact_boundary
                .len()
                .checked_mul(retained_layer_records)
                .and_then(|work| boundary_samples_per_transition.checked_add(work))
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        }
        let boundary_samples = boundary_samples_per_transition
            .checked_mul(transitions)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        let folded_faces = source
            .folded_faces
            .iter()
            .filter(|folded| faces.is_none_or(|faces| faces.contains(&folded.face.face_id)))
            .count();
        Ok(Self {
            transitions,
            cells,
            layer_records,
            boundary_samples,
            folded_faces,
            maximum_boundary_points,
        })
    }

    pub(super) fn checked_add_v1(self, other: Self) -> Option<Self> {
        Some(Self {
            transitions: self.transitions.checked_add(other.transitions)?,
            cells: self.cells.checked_add(other.cells)?,
            layer_records: self.layer_records.checked_add(other.layer_records)?,
            boundary_samples: self.boundary_samples.checked_add(other.boundary_samples)?,
            folded_faces: self.folded_faces.checked_add(other.folded_faces)?,
            maximum_boundary_points: self
                .maximum_boundary_points
                .max(other.maximum_boundary_points),
        })
    }

    fn memory_work_v1(self) -> Result<ori_collision::GeneralCellTransportMemoryWorkV1, String> {
        checked_general_cell_transport_memory_work_v1(
            self.transitions,
            self.folded_faces,
            self.cells,
            self.maximum_boundary_points,
        )
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    }

    fn exact_limits(self) -> GeneralCellTransportLimitsV1 {
        GeneralCellTransportLimitsV1 {
            max_transitions: self.transitions,
            max_cells: self.cells,
            max_layer_records: self.layer_records,
            max_boundary_samples: self.boundary_samples,
        }
    }
}

fn production_bounded_multi_block_transport_limits_v1(
    block_count: usize,
) -> Result<GeneralCellTransportLimitsV1, String> {
    if !bounded_multi_block_current_cycle_arity_supported_v1(block_count) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let proof_count = block_count
        .checked_add(1)
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    Ok(GeneralCellTransportLimitsV1 {
        max_transitions: proof_count
            .checked_mul(65_537)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?,
        max_cells: proof_count
            .checked_mul(ori_foldability::DEFAULT_MAX_OVERLAP_CELLS)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?,
        max_layer_records: proof_count
            .checked_mul(ori_foldability::DEFAULT_MAX_TOTAL_RECORDS)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?,
        max_boundary_samples: ori_foldability::DEFAULT_MAX_EXACT_OPERATIONS,
    })
}

pub(super) fn preflight_bounded_multi_block_transport_aggregate_v1(
    work: BoundedMultiBlockCellTransportWorkV1,
    limits: GeneralCellTransportLimitsV1,
) -> Result<(), String> {
    preflight_general_cell_transport_work_v1(
        work.transitions,
        work.cells,
        work.layer_records,
        work.boundary_samples,
        limits,
    )
    .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
}

fn restrict_bounded_multi_block_layer_source_v1(
    source: &ori_foldability::LayerOrderSnapshot,
    faces: &[FaceId],
    retained_bytes: usize,
) -> Result<ori_foldability::LayerOrderSnapshot, String> {
    #[cfg(test)]
    BOUNDED_MULTI_BLOCK_LAYER_SOURCE_CLONE_ATTEMPTS_V1
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    source
        .try_restrict_to_faces_with_retained_byte_limit_v1(faces, retained_bytes)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
}

pub(super) fn materialize_bounded_multi_block_layer_sources_v1(
    source: &ori_foldability::LayerOrderSnapshot,
    block_face_sets: &[&[FaceId]],
    proof_retained_bytes: usize,
    peak_temporary_bytes: usize,
    maximum_peak_retained_bytes: usize,
) -> Result<
    (
        Box<ori_foldability::LayerOrderSnapshot>,
        Vec<ori_foldability::LayerOrderSnapshot>,
        BoundedMultiBlockLayerRetainedBytesV1,
    ),
    String,
> {
    let retained = BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(
        source,
        block_face_sets,
        proof_retained_bytes,
        peak_temporary_bytes,
    )?;
    preflight_bounded_multi_block_layer_peak_retained_bytes_v1(
        retained.peak,
        maximum_peak_retained_bytes,
    )?;
    let mut block_sources = Vec::new();
    block_sources
        .try_reserve_exact(block_face_sets.len())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    #[cfg(test)]
    BOUNDED_MULTI_BLOCK_LAYER_SOURCE_CLONE_ATTEMPTS_V1
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    let whole_source = Box::new(
        source
            .try_clone_with_retained_byte_limit_v1(retained.whole_source)
            .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?,
    );
    for (faces, retained_bytes) in block_face_sets.iter().copied().zip(&retained.block_sources) {
        block_sources.push(restrict_bounded_multi_block_layer_source_v1(
            source,
            faces,
            *retained_bytes,
        )?);
    }
    Ok((whole_source, block_sources, retained))
}

fn exact_bounded_multi_block_cross_pairs_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
) -> Result<Vec<CommonArticulationCrossBlockFacePairV1>, String> {
    let blocks = decomposition.blocks();
    if !bounded_multi_block_current_cycle_arity_supported_v1(blocks.len()) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let mut raw_pair_count = 0usize;
    for left in 0..blocks.len() {
        for right in left + 1..blocks.len() {
            raw_pair_count = blocks[left]
                .geometry()
                .face_ids()
                .len()
                .checked_mul(blocks[right].geometry().face_ids().len())
                .and_then(|count| raw_pair_count.checked_add(count))
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        }
    }
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(raw_pair_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for left in 0..blocks.len() {
        for right in left + 1..blocks.len() {
            for first_face in blocks[left].geometry().face_ids().iter().copied() {
                for second_face in blocks[right].geometry().face_ids().iter().copied() {
                    if let Some(pair) =
                        CommonArticulationCrossBlockFacePairV1::new(first_face, second_face)
                    {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    pairs.sort_unstable_by_key(|pair| {
        (
            pair.first().canonical_bytes(),
            pair.second().canonical_bytes(),
        )
    });
    pairs.dedup();
    Ok(pairs)
}

fn certified_whole_parent_layer_pairs_v1(
    source: &ori_foldability::LayerOrderSnapshot,
    certified_pair_count: usize,
) -> Result<Vec<(FaceId, FaceId)>, String> {
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(source.face_pair_orders.len())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    pairs.extend(
        source
            .face_pair_orders
            .iter()
            .map(|pair| (pair.lower_face.face_id, pair.upper_face.face_id)),
    );
    pairs.sort_unstable_by_key(|(lower, upper)| (lower.canonical_bytes(), upper.canonical_bytes()));
    pairs.dedup();
    if pairs.len() != certified_pair_count {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    Ok(pairs)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_blockwise_current_cycle_fallback_v1(
    app: Option<&AppHandle>,
    transaction_state: &super::stacked_fold_transaction::StackedFoldTransactionState,
    project: &super::ProjectState,
    foldability_state: Option<&super::GlobalFlatFoldabilityState>,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: Option<CurrentLayerOrderCapability>,
    generated: &GeneratedMultiHingePathCandidateV1,
    requested: &ori_kinematics::CanonicalHingeAngles,
    thickness: f64,
    generation: u64,
    progress_request_id: Option<&str>,
    source_revision: u64,
    target_revision: u64,
) -> Result<CurrentCyclePosePreviewResponseV1, String> {
    let control = CooperativeOperationControlV1::unbounded();
    let checkpoint = || {
        blockwise_cycle_control_gate_v1(generation, &control)
            .map_err(|error| blockwise_cycle_control_message_v1(error).to_owned())
    };
    checkpoint()?;
    if super::stacked_fold_transaction::next_current_cycle_target_revision_v1(source_revision)
        != Some(target_revision)
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let layer_capability = layer_capability.ok_or_else(|| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    }
    let (geometry, audit, pose) = pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(audit, CanonicalEdgeBlockLimitsV1::default())
        .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
    checkpoint()?;
    let block_count = decomposition.blocks().len();
    if bounded_multi_block_current_cycle_arity_supported_v1(block_count) {
        drop(decomposition);
        return prepare_bounded_multi_block_current_cycle_fallback_v1(
            app,
            transaction_state,
            project,
            foldability_state,
            pose_capability,
            layer_capability,
            generated,
            requested,
            thickness,
            generation,
            progress_request_id,
            source_revision,
            target_revision,
            block_count,
        );
    }
    let [first, second] = decomposition.blocks() else {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    };
    let [articulation] = decomposition.articulation_faces() else {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    };
    if *articulation != pose.fixed_face() {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    }
    let prepare = |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1| {
        checkpoint()?;
        let schedule = generated
            .schedule()
            .restrict_to_edge_block_v1(geometry, audit, block.geometry(), block.audit())
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
        let closure = block
            .geometry()
            .prove_simultaneous_cycle_basis_schedule_closure_v1(
                block.audit(),
                *articulation,
                &schedule,
                ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                CycleBasisLimitsV1::default(),
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 16,
                    max_leaves: 65_536,
                    max_work: 1_048_576,
                    schedule_limits: production_cycle_schedule_limits_v1(),
                },
            )
            .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?
            .closure()
            .clone();
        let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            block.geometry(),
            block.audit(),
            *articulation,
            &schedule,
            &closure,
            thickness,
            32,
        )
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        checkpoint()?;
        Ok::<_, String>((schedule, closure, positive))
    };
    let (first_schedule, first_closure, first_positive) = prepare(first)?;
    let (second_schedule, second_closure, second_positive) = prepare(second)?;
    let block_faces = [first, second].map(|block| {
        block
            .geometry()
            .face_ids()
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
    });
    let source_snapshot = layer_capability.snapshot();
    let mut cell_owners = std::collections::HashMap::new();
    for cell in &source_snapshot.overlap_cells {
        checkpoint()?;
        if cell.covering_faces.is_empty() || cell.bottom_to_top_faces.is_empty() {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        }
        let owners = block_faces
            .iter()
            .enumerate()
            .filter_map(|(index, faces)| {
                (cell
                    .covering_faces
                    .iter()
                    .all(|face| faces.contains(&face.face_id))
                    && cell
                        .bottom_to_top_faces
                        .iter()
                        .all(|face| faces.contains(face)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        };
        if cell_owners.insert(cell.cell_key, *owner).is_some() {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        }
    }
    for pair in &source_snapshot.face_pair_orders {
        checkpoint()?;
        let owners = block_faces
            .iter()
            .enumerate()
            .filter_map(|(index, faces)| {
                (faces.contains(&pair.lower_face.face_id)
                    && faces.contains(&pair.upper_face.face_id)
                    && !pair.supporting_cells.is_empty()
                    && pair
                        .supporting_cells
                        .iter()
                        .all(|cell| cell_owners.get(cell) == Some(&index)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches!(owners.as_slice(), [_]) {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        }
    }
    let restrict_snapshot = |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1| {
        let faces = block.geometry().face_ids();
        let mut snapshot = layer_capability.snapshot().clone();
        snapshot
            .material_faces
            .retain(|face| faces.contains(&face.face_id));
        snapshot
            .folded_faces
            .retain(|face| faces.contains(&face.face.face_id));
        snapshot
            .global_bottom_to_top
            .as_mut()
            .iter_mut()
            .for_each(|order| {
                order.retain(|face| faces.contains(&face.face_id));
            });
        snapshot.reference_face = snapshot
            .reference_face
            .filter(|face| faces.contains(&face.face_id));
        snapshot.overlap_cells.retain(|cell| {
            !cell.covering_faces.is_empty()
                && cell
                    .covering_faces
                    .iter()
                    .all(|face| faces.contains(&face.face_id))
                && cell
                    .bottom_to_top_faces
                    .iter()
                    .all(|face| faces.contains(face))
        });
        let cells = snapshot
            .overlap_cells
            .iter()
            .map(|cell| cell.cell_key)
            .collect::<std::collections::HashSet<_>>();
        snapshot.face_pair_orders.retain_mut(|pair| {
            pair.supporting_cells.retain(|cell| cells.contains(cell));
            faces.contains(&pair.lower_face.face_id)
                && faces.contains(&pair.upper_face.face_id)
                && !pair.supporting_cells.is_empty()
        });
        snapshot.proof_summary = None;
        Box::new(snapshot)
    };
    let sources = [restrict_snapshot(first), restrict_snapshot(second)];
    let transport =
        |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1,
         source: &ori_foldability::LayerOrderSnapshot,
         schedule: &ori_kinematics::CanonicalCycleScheduleV1,
         closure: &ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
         positive: &ori_collision::PositiveThicknessContinuousCertificateV1| {
            let transitions = closure
                .leaves()
                .len()
                .checked_add(1)
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            let layer_records = source
                .overlap_cells
                .iter()
                .try_fold(0usize, |sum, cell| {
                    sum.checked_add(cell.bottom_to_top_faces.len())
                })
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            let boundary_samples = source
                .overlap_cells
                .iter()
                .try_fold(0usize, |sum, cell| {
                    cell.exact_boundary
                        .len()
                        .checked_mul(cell.bottom_to_top_faces.len())
                        .and_then(|work| sum.checked_add(work))
                })
                .and_then(|work| work.checked_mul(transitions))
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                geometry: block.geometry(),
                audit: block.audit(),
                source,
                schedule,
                closure,
                positive_continuous: positive,
                paper_thickness_mm: thickness,
                tolerance: ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                limits: GeneralCellTransportLimitsV1 {
                    max_transitions: transitions,
                    max_cells: source.overlap_cells.len(),
                    max_layer_records: layer_records,
                    max_boundary_samples: boundary_samples,
                },
            })
            .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
        };
    let first_layer = transport(
        first,
        &sources[0],
        &first_schedule,
        &first_closure,
        &first_positive,
    )?;
    checkpoint()?;
    let second_layer = transport(
        second,
        &sources[1],
        &second_schedule,
        &second_closure,
        &second_positive,
    )?;
    checkpoint()?;
    let issuer_context = ori_foldability::fold_model_fingerprint_v1(
        project.editor.pattern(),
        project.editor.paper(),
    )
    .0;
    let parent = ori_collision::issue_blockwise_closure_authority_v1(
        [
            ori_collision::BlockwiseClosureInputV1 {
                geometry: first.geometry(),
                audit: first.audit(),
                schedule: &first_schedule,
                closure: &first_closure,
            },
            ori_collision::BlockwiseClosureInputV1 {
                geometry: second.geometry(),
                audit: second.audit(),
                schedule: &second_schedule,
                closure: &second_closure,
            },
        ],
        *articulation,
        thickness,
        issuer_context,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"blockwise-current-cycle-articulation-layer-v1");
    fingerprint.update(articulation.canonical_bytes());
    fingerprint.update(issuer_context);
    let articulation_layer_fingerprint: [u8; 32] = fingerprint.finalize().into();
    let authority = ori_collision::issue_blockwise_positive_layer_authority_v1(
        parent,
        [
            ori_collision::BlockwisePositiveLayerInputV1 {
                source: &sources[0],
                positive: first_positive,
                layer: first_layer,
            },
            ori_collision::BlockwisePositiveLayerInputV1 {
                source: &sources[1],
                positive: second_positive,
                layer: second_layer,
            },
        ],
        *articulation,
        thickness,
        issuer_context,
        articulation_layer_fingerprint,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    checkpoint()?;
    let transition_count = authority.transition_count_v1();
    let pair_count = authority.pair_order_count_v1();
    let target_hash = authority
        .target_order_hash_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let layer_pairs = certified_blockwise_layer_pairs_v1(&sources, pair_count)?;
    let response_pairs = layer_pairs
        .iter()
        .map(|(lower_face, upper_face)| LayerOrderPairDtoV1 {
            lower_face: *lower_face,
            upper_face: *upper_face,
        })
        .collect::<Vec<_>>();
    checkpoint()?;
    emit_current_cycle_progress_v1(app, progress_request_id, 1, 1);
    let closure_leaf_count = first_closure.leaves().len() + second_closure.leaves().len();
    let closure_max_depth = first_closure
        .leaves()
        .iter()
        .chain(second_closure.leaves())
        .map(|(depth, _, _)| *depth)
        .max()
        .unwrap_or(0);
    let checked_hinge_count = first.geometry().hinges().len() + second.geometry().hinges().len();
    let total_hinge_count = geometry.hinges().len();
    let pending = super::stacked_fold_transaction::PendingBlockwiseCurrentCyclePremisesV1 {
        expected_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        expected_source_fingerprint: issuer_context,
        expected_pose_generation: pose_capability.generation(),
        expected_layer_generation: layer_capability.generation(),
        geometry: geometry.clone(),
        fixed_face: *articulation,
        authority,
        sources,
        articulation: *articulation,
        thickness,
        issuer_context,
        articulation_layer_fingerprint,
        layer_order_pairs: layer_pairs,
        target_angles: requested
            .as_slice()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect(),
    };
    let foldability_state = foldability_state.ok_or_else(|| super::STALE_MESSAGE.to_owned())?;
    let pose_is_current = project
        .applied_pose_authority
        .revalidate_capability(project, &pose_capability)
        .map_err(|_| super::STALE_MESSAGE.to_owned())?
        .is_some();
    let layer_is_current = super::revalidate_current_layer_order_capability(
        foldability_state,
        project,
        &layer_capability,
    )
    .map_err(|_| super::STALE_MESSAGE.to_owned())?
    .is_some();
    if !pose_is_current || !layer_is_current {
        return Err(super::STALE_MESSAGE.to_owned());
    }
    let token = ProjectId::new();
    let response = CurrentCyclePosePreviewResponseV1 {
        version: 1,
        transaction_token: token,
        source_revision,
        target_revision,
        closure_leaf_count,
        closure_max_depth,
        checked_hinge_count,
        total_hinge_count,
        continuous_path_certified: true,
        continuous_layer_transport_model_id: Some(
            ori_collision::BLOCKWISE_POSITIVE_LAYER_MODEL_ID_V1,
        ),
        continuous_layer_transition_count: transition_count,
        continuous_layer_pair_order_count: pair_count,
        continuous_layer_target_order_sha256: Some(target_hash),
        // The certified blockwise transport preserves the published ordering.
        // Keep independent DTO vectors because the wire response owns both
        // source and target fields.
        target_layer_order: response_pairs.clone(),
        source_layer_order: response_pairs,
        authorizes_project_mutation: false,
    };
    super::with_current_cycle_publication_v1(generation, || {
        super::stacked_fold_transaction::install_pending_blockwise_current_cycle_pose_with_token_v1(
            transaction_state,
            token,
            pending,
            pose_capability,
            layer_capability,
        )
    })?;
    Ok(response)
}

#[allow(clippy::too_many_arguments)]
fn prepare_bounded_multi_block_current_cycle_fallback_v1(
    app: Option<&AppHandle>,
    transaction_state: &super::stacked_fold_transaction::StackedFoldTransactionState,
    project: &super::ProjectState,
    foldability_state: Option<&super::GlobalFlatFoldabilityState>,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: CurrentLayerOrderCapability,
    generated: &GeneratedMultiHingePathCandidateV1,
    requested: &ori_kinematics::CanonicalHingeAngles,
    thickness: f64,
    generation: u64,
    progress_request_id: Option<&str>,
    source_revision: u64,
    target_revision: u64,
    expected_block_count: usize,
) -> Result<CurrentCyclePosePreviewResponseV1, String> {
    let control = CooperativeOperationControlV1::unbounded();
    let checkpoint = || {
        blockwise_cycle_control_gate_v1(generation, &control)
            .map_err(|error| blockwise_cycle_control_message_v1(error).to_owned())
    };
    checkpoint()?;
    if super::stacked_fold_transaction::next_current_cycle_target_revision_v1(source_revision)
        != Some(target_revision)
        || !thickness.is_finite()
        || thickness <= 0.0
        || !bounded_multi_block_current_cycle_arity_supported_v1(expected_block_count)
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let (geometry, audit, pose) = pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: expected_block_count,
                ..CanonicalEdgeBlockLimitsV1::default()
            },
        )
        .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    if decomposition.blocks().len() != expected_block_count {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let schedule_limits = production_cycle_schedule_limits_v1();
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            audit,
            pose.fixed_face(),
            generated.schedule(),
            ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 16,
                max_leaves: 65_536,
                max_work: 1_048_576,
                schedule_limits,
            },
        )
        .map_err(|error| match error {
            ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit => {
                CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
            }
            _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
        })?;
    checkpoint()?;
    let whole_parent_positive =
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            geometry,
            audit,
            pose.fixed_face(),
            generated.schedule(),
            &closure,
            thickness,
            32,
            &control,
        )
        .map_err(|error| match error {
            ori_collision::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled => {
                CANCELLED_MESSAGE.to_owned()
            }
            ori_collision::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded => {
                CYCLE_PATH_DEADLINE_MESSAGE.to_owned()
            }
        })?
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    checkpoint()?;
    let common_pose_limits = ori_kinematics::CommonArticulationPoseLimitsV1 {
        max_blocks: expected_block_count,
        ..ori_kinematics::CommonArticulationPoseLimitsV1::default()
    };
    let common_pose = issue_common_articulation_pose_authority_with_control_v1(
        ori_kinematics::CommonArticulationPoseInputV1 {
            geometry,
            pose,
            decomposition: &decomposition,
            paper_thickness_mm: thickness,
            limits: common_pose_limits,
        },
        &control,
    )
    .map_err(|error| match error {
        ori_kinematics::CommonArticulationPoseErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        ori_kinematics::CommonArticulationPoseErrorV1::Cancelled => CANCELLED_MESSAGE.to_owned(),
        ori_kinematics::CommonArticulationPoseErrorV1::DeadlineExceeded => {
            CYCLE_PATH_DEADLINE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
    })?;
    let cross_block_pairs = exact_bounded_multi_block_cross_pairs_v1(&decomposition)?;
    let clearance_limits = CommonArticulationClearanceLimitsV1 {
        max_blocks: expected_block_count,
        ..CommonArticulationClearanceLimitsV1::default()
    };
    let clearance = issue_common_articulation_clearance_prerequisite_with_control_v1(
        CommonArticulationClearanceInputV1 {
            geometry,
            audit,
            pose,
            decomposition: &decomposition,
            common_pose: &common_pose,
            common_pose_limits,
            schedule: generated.schedule(),
            schedule_limits,
            closure: &closure,
            paper_thickness_mm: thickness,
            submitted_cross_block_pairs: &cross_block_pairs,
            whole_parent_continuous: Some(whole_parent_positive.clone()),
            limits: clearance_limits,
        },
        &control,
    )
    .map_err(|error| match error {
        ori_collision::CommonArticulationClearanceErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        ori_collision::CommonArticulationClearanceErrorV1::Cancelled => {
            CANCELLED_MESSAGE.to_owned()
        }
        ori_collision::CommonArticulationClearanceErrorV1::DeadlineExceeded => {
            CYCLE_PATH_DEADLINE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
    })?;
    let clearance = match clearance {
        CommonArticulationClearanceOutcomeV1::Certified(authority) => *authority,
        CommonArticulationClearanceOutcomeV1::Unsupported(_) => {
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
        }
    };
    let mut canonical_block_edges = Vec::new();
    canonical_block_edges
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for block in decomposition.blocks() {
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(block.geometry().hinges().len())
            .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        edges.extend(block.geometry().hinges().iter().map(|hinge| hinge.edge()));
        canonical_block_edges.push(edges);
    }
    let staged = issue_common_articulation_block_composed_path_authority_with_control_v1(
        CommonArticulationBlockComposedPathInputV1 {
            geometry,
            audit,
            pose,
            decomposition: &decomposition,
            common_pose,
            common_pose_limits,
            schedule: generated.schedule(),
            schedule_limits,
            closure: &closure,
            paper_thickness_mm: thickness,
            clearance,
            clearance_limits,
            blocks: canonical_block_edges,
        },
        &control,
    )
    .map_err(|error| match error {
        ori_collision::CommonArticulationBlockComposedPathErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        ori_collision::CommonArticulationBlockComposedPathErrorV1::Cancelled => {
            CANCELLED_MESSAGE.to_owned()
        }
        ori_collision::CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded => {
            CYCLE_PATH_DEADLINE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
    })?;
    checkpoint()?;
    let layer_source = layer_capability.snapshot();
    let whole_parent_work =
        BoundedMultiBlockCellTransportWorkV1::for_source(layer_source, &closure)?;
    let prepare_block = |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1| {
        checkpoint()?;
        let block_fixed_face = block
            .geometry()
            .face_ids()
            .iter()
            .copied()
            .find(|face| decomposition.articulation_faces().contains(face))
            .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        let block_schedule = generated
            .schedule()
            .restrict_to_edge_block_with_fixed_face_v1(
                geometry,
                audit,
                block.geometry(),
                block.audit(),
                block_fixed_face,
            )
            .map_err(|error| match error {
                ori_kinematics::CycleSchedulePrepareErrorV1::ResourceLimit => {
                    CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
                }
                _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
            })?;
        let block_closure = block
            .geometry()
            .prove_dyadic_schedule_closure_v1(
                block.audit(),
                block_fixed_face,
                &block_schedule,
                ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 16,
                    max_leaves: 65_536,
                    max_work: 1_048_576,
                    schedule_limits,
                },
            )
            .map_err(|error| match error {
                ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit => {
                    CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
                }
                _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
            })?;
        Ok::<_, String>((block_schedule, block_closure))
    };
    let mut block_schedules = Vec::new();
    block_schedules
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for block in decomposition.blocks() {
        block_schedules.push(prepare_block(block)?);
    }
    let mut aggregate_work = whole_parent_work;
    let mut block_transport_work = Vec::new();
    block_transport_work
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for (block, (_, block_closure)) in decomposition.blocks().iter().zip(&block_schedules) {
        let work = BoundedMultiBlockCellTransportWorkV1::for_restricted_source_v1(
            layer_source,
            block_closure,
            block.geometry().face_ids(),
        )?;
        aggregate_work = aggregate_work
            .checked_add_v1(work)
            .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        block_transport_work.push(work);
    }
    preflight_bounded_multi_block_transport_aggregate_v1(
        aggregate_work,
        production_bounded_multi_block_transport_limits_v1(expected_block_count)?,
    )?;
    let whole_memory = whole_parent_work.memory_work_v1()?;
    let (proof_retained_bytes, peak_temporary_bytes) = block_transport_work.iter().try_fold(
        (
            whole_memory.proof_retained_bytes,
            whole_memory.peak_temporary_bytes,
        ),
        |(proof_retained, peak_temporary), work| {
            let memory = work.memory_work_v1()?;
            Ok::<_, String>((
                proof_retained
                    .checked_add(memory.proof_retained_bytes)
                    .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?,
                peak_temporary.max(memory.peak_temporary_bytes),
            ))
        },
    )?;
    checkpoint()?;
    let canonical_source_key = |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1| {
        block
            .geometry()
            .hinges()
            .iter()
            .map(|hinge| hinge.edge().canonical_bytes())
            .min()
            .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
    };
    let mut block_source_specs = Vec::new();
    block_source_specs
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for block in decomposition.blocks() {
        block_source_specs.push((canonical_source_key(block)?, block.geometry().face_ids()));
    }
    block_source_specs.sort_unstable_by_key(|(key, _)| *key);
    let mut canonical_block_keys = Vec::new();
    canonical_block_keys
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    let mut block_face_sets = Vec::new();
    block_face_sets
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for (key, faces) in &block_source_specs {
        canonical_block_keys.push(*key);
        block_face_sets.push(*faces);
    }
    let mut closure_inputs = Vec::new();
    closure_inputs
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    let mut positive_layer_inputs = Vec::new();
    positive_layer_inputs
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    let mut block_hinges = Vec::new();
    block_hinges
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for block in decomposition.blocks() {
        let mut hinges = Vec::new();
        hinges
            .try_reserve_exact(block.geometry().hinges().len())
            .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
        hinges.extend(block.geometry().hinges().iter().map(|hinge| hinge.edge()));
        block_hinges.push(hinges);
    }
    let mut target_angles = Vec::new();
    target_angles
        .try_reserve_exact(requested.as_slice().len())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    target_angles.extend(
        requested
            .as_slice()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees())),
    );
    let (source, block_sources, _retained_bytes) =
        materialize_bounded_multi_block_layer_sources_v1(
            layer_source,
            &block_face_sets,
            proof_retained_bytes,
            peak_temporary_bytes,
            production_bounded_multi_block_layer_peak_limit_v1(),
        )?;
    checkpoint()?;
    let whole_parent_layer =
        certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
            geometry,
            audit,
            source: &source,
            schedule: generated.schedule(),
            closure: &closure,
            positive_continuous: &whole_parent_positive,
            paper_thickness_mm: thickness,
            tolerance: ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            limits: whole_parent_work.exact_limits(),
        })
        .map_err(|error| match error {
            ori_collision::GeneralCellTransportErrorV1::ResourceLimit => {
                CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
            }
            _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
        })?;
    checkpoint()?;
    for ((block, (block_schedule, block_closure)), work) in decomposition
        .blocks()
        .iter()
        .zip(&block_schedules)
        .zip(&block_transport_work)
    {
        checkpoint()?;
        let key = block
            .geometry()
            .hinges()
            .iter()
            .map(|hinge| hinge.edge().canonical_bytes())
            .min()
            .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        let source_index = canonical_block_keys
            .binary_search(&key)
            .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            block.geometry(),
            block.audit(),
            block_closure.fixed_face(),
            block_schedule,
            block_closure,
            thickness,
            32,
        )
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        let layer = certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
            geometry: block.geometry(),
            audit: block.audit(),
            source: &block_sources[source_index],
            schedule: block_schedule,
            closure: block_closure,
            positive_continuous: &positive,
            paper_thickness_mm: thickness,
            tolerance: ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            limits: work.exact_limits(),
        })
        .map_err(|error| match error {
            ori_collision::GeneralCellTransportErrorV1::ResourceLimit => {
                CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
            }
            _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
        })?;
        positive_layer_inputs.push(MultiBlockPositiveLayerInputV1 {
            geometry: block.geometry(),
            source: &block_sources[source_index],
            positive,
            layer,
        });
    }
    let issuer_context = ori_foldability::fold_model_fingerprint_v1(
        project.editor.pattern(),
        project.editor.paper(),
    )
    .0;
    let mut articulation_fingerprint = Sha256::new();
    articulation_fingerprint.update(match expected_block_count {
        THREE_BLOCK_CURRENT_CYCLE_ARITY_V1 => {
            b"three-block-current-cycle-articulation-layer-v1".as_slice()
        }
        FOUR_BLOCK_CURRENT_CYCLE_ARITY_V1 => {
            b"four-block-current-cycle-articulation-layer-v1".as_slice()
        }
        FIVE_BLOCK_CURRENT_CYCLE_ARITY_V1 => {
            b"five-block-current-cycle-articulation-layer-v1".as_slice()
        }
        SIX_BLOCK_CURRENT_CYCLE_ARITY_V1 => {
            b"six-block-current-cycle-articulation-layer-v1".as_slice()
        }
        SEVEN_BLOCK_CURRENT_CYCLE_ARITY_V1 => {
            b"seven-block-current-cycle-articulation-layer-v1".as_slice()
        }
        _ => return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned()),
    });
    articulation_fingerprint.update(issuer_context);
    for face in decomposition.articulation_faces() {
        articulation_fingerprint.update(face.canonical_bytes());
    }
    let articulation_layer_fingerprint: [u8; 32] = articulation_fingerprint.finalize().into();
    for (block, (block_schedule, block_closure)) in
        decomposition.blocks().iter().zip(&block_schedules)
    {
        closure_inputs.push(MultiBlockClosureInputV1 {
            geometry: block.geometry(),
            audit: block.audit(),
            schedule: block_schedule,
            closure: block_closure,
        });
    }
    let closure_parent =
        issue_multi_block_closure_authority_v1(closure_inputs, thickness, issuer_context)
            .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let positive_layer_parent = issue_multi_block_positive_layer_authority_v1(
        closure_parent,
        positive_layer_inputs,
        articulation_layer_fingerprint,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    checkpoint()?;
    let mut completeness_inputs = Vec::new();
    completeness_inputs
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for (block, hinges) in decomposition.blocks().iter().zip(&block_hinges) {
        completeness_inputs.push(BlockUnionCompletenessInputV1 {
            faces: block.geometry().face_ids(),
            hinges,
        });
    }
    let completeness_report = diagnose_block_union_completeness_v1(geometry, &completeness_inputs)
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    if !completeness_report.exact_live_union_observed() {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    if block_sources.len() != expected_block_count {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let mut block_source_refs = Vec::new();
    block_source_refs
        .try_reserve_exact(expected_block_count)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    block_source_refs.extend(block_sources.iter());
    let complete = issue_complete_multi_block_positive_layer_authority_v1(
        geometry,
        completeness_report,
        positive_layer_parent,
        &block_source_refs,
        thickness,
        issuer_context,
        articulation_layer_fingerprint,
        &target_angles,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let transition_count = whole_parent_layer.transition_hashes().len();
    let pair_count = whole_parent_layer.pair_order_count();
    let whole_parent_target_hash = whole_parent_layer.target_order_hash();
    let layer_pairs = certified_whole_parent_layer_pairs_v1(&source, pair_count)?;
    let mut response_pairs = Vec::new();
    response_pairs
        .try_reserve_exact(layer_pairs.len())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    response_pairs.extend(
        layer_pairs
            .iter()
            .map(|(lower_face, upper_face)| LayerOrderPairDtoV1 {
                lower_face: *lower_face,
                upper_face: *upper_face,
            }),
    );
    let authority = issue_common_articulation_continuous_layer_path_authority_with_control_v1(
        CommonArticulationContinuousLayerPathInputV1 {
            geometry,
            audit,
            pose,
            decomposition: &decomposition,
            staged,
            common_pose_limits,
            schedule: generated.schedule(),
            schedule_limits,
            closure: &closure,
            paper_thickness_mm: thickness,
            clearance_limits,
            complete,
            block_sources: &block_source_refs,
            issuer_context,
            articulation_layer_fingerprint,
            target_angles: &target_angles,
            source: &source,
            whole_parent_layer,
        },
        &control,
    )
    .map_err(|error| match error {
        ori_collision::CommonArticulationContinuousLayerPathErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        ori_collision::CommonArticulationContinuousLayerPathErrorV1::Cancelled => {
            CANCELLED_MESSAGE.to_owned()
        }
        ori_collision::CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded => {
            CYCLE_PATH_DEADLINE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
    })?;
    if authority.block_count_v1() != expected_block_count || transition_count == 0 {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    checkpoint()?;
    let foldability_state = foldability_state.ok_or_else(|| super::STALE_MESSAGE.to_owned())?;
    let pose_is_current = project
        .applied_pose_authority
        .revalidate_capability(project, &pose_capability)
        .map_err(|_| super::STALE_MESSAGE.to_owned())?
        .is_some();
    let layer_is_current = super::revalidate_current_layer_order_capability(
        foldability_state,
        project,
        &layer_capability,
    )
    .map_err(|_| super::STALE_MESSAGE.to_owned())?
    .is_some();
    if !pose_is_current || !layer_is_current || source.as_ref() != layer_capability.snapshot() {
        return Err(super::STALE_MESSAGE.to_owned());
    }
    checkpoint()?;
    emit_current_cycle_progress_v1(app, progress_request_id, 1, 1);
    let closure_leaf_count = closure.leaves().len();
    let closure_max_depth = closure
        .leaves()
        .iter()
        .map(|(depth, _, _)| *depth)
        .max()
        .unwrap_or(0);
    let checked_hinge_count = decomposition
        .blocks()
        .iter()
        .map(|block| block.geometry().hinges().len())
        .sum();
    let total_hinge_count = geometry.hinges().len();
    let target_hash_capacity = whole_parent_target_hash
        .len()
        .checked_mul(2)
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    let mut target_hash = String::new();
    target_hash
        .try_reserve_exact(target_hash_capacity)
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for byte in whole_parent_target_hash {
        std::fmt::Write::write_fmt(&mut target_hash, format_args!("{byte:02x}"))
            .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    }
    let pending = super::stacked_fold_transaction::PendingBoundedMultiBlockCurrentCyclePremisesV1 {
        expected_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        expected_source_fingerprint: issuer_context,
        expected_pose_generation: pose_capability.generation(),
        expected_layer_generation: layer_capability.generation(),
        geometry: geometry.clone(),
        audit: audit.clone(),
        pose: pose.clone(),
        decomposition,
        authority,
        common_pose_limits,
        schedule: generated.schedule().clone(),
        schedule_limits,
        closure,
        clearance_limits,
        block_sources,
        source,
        thickness,
        issuer_context,
        articulation_layer_fingerprint,
        layer_order_pairs: layer_pairs,
        transition_count,
        target_angles,
    };
    let token = ProjectId::new();
    let response = CurrentCyclePosePreviewResponseV1 {
        version: 1,
        transaction_token: token,
        source_revision,
        target_revision,
        closure_leaf_count,
        closure_max_depth,
        checked_hinge_count,
        total_hinge_count,
        continuous_path_certified: true,
        continuous_layer_transport_model_id: Some(
            ori_collision::COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1,
        ),
        continuous_layer_transition_count: transition_count,
        continuous_layer_pair_order_count: pair_count,
        continuous_layer_target_order_sha256: Some(target_hash),
        target_layer_order: response_pairs.clone(),
        source_layer_order: response_pairs,
        authorizes_project_mutation: false,
    };
    super::with_current_cycle_publication_v1(generation, || {
        super::stacked_fold_transaction::install_pending_bounded_multi_block_current_cycle_pose_with_token_v1(
            transaction_state,
            token,
            pending,
            pose_capability,
            layer_capability,
        )
    })?;
    Ok(response)
}
