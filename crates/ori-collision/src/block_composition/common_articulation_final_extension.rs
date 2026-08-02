use std::sync::Arc;

use ori_domain::EdgeId;
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV1,
    ClosedMaterialHingeGraphPose, CommonArticulationPoseExtensionLimitsV1, CycleScheduleLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MAX_BLOCKS_V1,
    COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1, CanonicalBlockBindingV1,
    CommonArticulationBlockComposedPathErrorV1,
    CommonArticulationBlockComposedPathExtensionAuthorityV1,
    CommonArticulationBlockComposedPathExtensionErrorV1,
    CommonArticulationBlockComposedPathExtensionRevalidationInputV1,
    CommonArticulationContinuousLayerPathErrorV1, CompleteMultiBlockPositiveLayerAuthorityV2,
    CompleteMultiBlockPositiveLayerErrorV2, CompleteMultiBlockPositiveLayerRevalidationInputV2,
    canonical_decomposition_block_bindings_v1, canonical_target_angles_for_final_path_v1,
};
use crate::{
    CommonArticulationClearanceExtensionLimitsV1,
    CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionV2, CooperativeOperationControlV1,
    CooperativeOperationStopV1,
};

pub const COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MODEL_ID_V2: &str =
    "common_articulation_continuous_layer_path_extension_authority_v2";
pub const COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MIN_BLOCKS_V2: usize =
    COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1;
pub const COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2: usize =
    COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MAX_BLOCKS_V1;

/// Exact live inputs for the separately typed, non-authorizing 11..=32 final
/// layer extension.
///
/// The staged extension, complete positive-layer authority, and whole-parent
/// layer proof are all moved into a successful result. The complete authority
/// must have been issued by the bounded-extension lower scope under the same
/// configured cap as every pose and clearance premise.
pub struct CommonArticulationContinuousLayerPathExtensionInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub staged: CommonArticulationBlockComposedPathExtensionAuthorityV1,
    pub common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub clearance_limits: CommonArticulationClearanceExtensionLimitsV1,
    pub complete: CompleteMultiBlockPositiveLayerAuthorityV2,
    pub block_sources: &'a [&'a LayerOrderSnapshot],
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub target_angles: &'a [(EdgeId, f64)],
    pub source: &'a LayerOrderSnapshot,
    pub whole_parent_layer: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
    pub parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
}

/// Exact live inputs required to revalidate a retained final-layer extension.
#[derive(Clone, Copy)]
pub struct CommonArticulationContinuousLayerPathExtensionRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub clearance_limits: CommonArticulationClearanceExtensionLimitsV1,
    pub block_sources: &'a [&'a LayerOrderSnapshot],
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub target_angles: &'a [(EdgeId, f64)],
    pub source: &'a LayerOrderSnapshot,
    pub parent_graph_admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationContinuousLayerPathExtensionErrorV2 {
    #[error("the final-layer common-articulation extension input is malformed")]
    InvalidInput,
    #[error("the final-layer common-articulation extension exceeded a resource limit")]
    ResourceLimit,
    #[error("the staged extension failed exact revalidation: {0}")]
    Staged(CommonArticulationBlockComposedPathExtensionErrorV1),
    #[error("the bounded complete positive-layer authority failed exact revalidation")]
    CompleteMultiBlockMismatch,
    #[error("the staged, complete, and live decomposition bind different canonical blocks")]
    CanonicalBlockPartitionMismatch,
    #[error("a complete-authority block schedule is not the exact full-path restriction")]
    BlockScheduleRestrictionMismatch,
    #[error("a block layer source is not the exact whole-parent source restriction")]
    BlockSourceRestrictionMismatch,
    #[error("the whole-parent layer transport proof failed exact revalidation")]
    WholeParentLayerMismatch,
    #[error("the exact parent-graph admission failed live revalidation: {0}")]
    ParentGraphAdmission(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2),
    #[error("the retained final-layer extension binding does not match the live inputs")]
    BindingMismatch,
    #[error("the final-layer common-articulation extension operation was cancelled")]
    Cancelled,
    #[error("the final-layer common-articulation extension operation deadline elapsed")]
    DeadlineExceeded,
}

/// Opaque non-authorizing final-layer composition for 11 through one
/// configured cap no greater than 32.
///
/// This is an intentionally separate research boundary. It is neither the
/// legacy final authority nor an input to certified-path, desktop, Apply, or
/// viewer publication routes.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationContinuousLayerPathAuthorityV1,
///     CommonArticulationContinuousLayerPathExtensionAuthorityV2,
/// };
///
/// fn legacy_final(_: CommonArticulationContinuousLayerPathAuthorityV1) {}
/// fn cannot_enter_legacy_final(
///     extension: CommonArticulationContinuousLayerPathExtensionAuthorityV2,
/// ) {
///     legacy_final(extension);
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationContinuousLayerPathAuthorityV1,
///     CommonArticulationContinuousLayerPathExtensionAuthorityV2,
/// };
///
/// struct DesktopApplyViewerRoute {
///     authority: CommonArticulationContinuousLayerPathAuthorityV1,
/// }
/// fn cannot_publish(
///     extension: CommonArticulationContinuousLayerPathExtensionAuthorityV2,
/// ) {
///     let _ = DesktopApplyViewerRoute { authority: extension };
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationContinuousLayerPathExtensionAuthorityV2;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationContinuousLayerPathExtensionAuthorityV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationContinuousLayerPathExtensionAuthorityV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationContinuousLayerPathExtensionAuthorityV2>();
/// ```
pub struct CommonArticulationContinuousLayerPathExtensionAuthorityV2 {
    binding: [u8; 32],
    configured_max_blocks: usize,
    actual_block_count: usize,
    blocks: Vec<CanonicalBlockBindingV1>,
    staged: CommonArticulationBlockComposedPathExtensionAuthorityV1,
    complete: CompleteMultiBlockPositiveLayerAuthorityV2,
    whole_parent_layer: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
    parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
}

impl std::fmt::Debug for CommonArticulationContinuousLayerPathExtensionAuthorityV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommonArticulationContinuousLayerPathExtensionAuthorityV2")
            .field("binding", &self.binding)
            .field("configured_max_blocks", &self.configured_max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .finish_non_exhaustive()
    }
}

impl CommonArticulationContinuousLayerPathExtensionAuthorityV2 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MODEL_ID_V2
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub const fn configured_max_blocks_v2(&self) -> usize {
        self.configured_max_blocks
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub fn block_count_v2(&self) -> usize {
        self.blocks.len()
    }

    #[must_use]
    pub const fn staged_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.staged.binding_fingerprint_v1()
    }

    #[must_use]
    pub const fn complete_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.complete.binding_fingerprint_v2()
    }

    #[must_use]
    pub fn whole_parent_target_order_hash_v2(&self) -> [u8; 32] {
        self.whole_parent_layer.target_order_hash_v1()
    }

    #[must_use]
    pub fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_graph_admission.binding_fingerprint_v2()
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationContinuousLayerPathExtensionRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2> {
        self.revalidate_with_control_v2(input, &CooperativeOperationControlV1::unbounded())
    }

    pub fn revalidate_with_control_v2(
        &self,
        input: CommonArticulationContinuousLayerPathExtensionRevalidationInputV2<'_>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2> {
        self.revalidate_with_checkpoint_v2(input, control, &mut || {
            final_extension_checkpoint_v2(control)
        })
    }

    fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationContinuousLayerPathExtensionRevalidationInputV2<'_>,
        control: &CooperativeOperationControlV1<'_>,
        checkpoint: &mut impl FnMut()
            -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
    ) -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2> {
        let live_parent_graph_admission = input.parent_graph_admission;
        let (binding, configured_max_blocks, actual_block_count, blocks) =
            validate_common_articulation_continuous_layer_path_extension_v2(
                input,
                &self.staged,
                &self.complete,
                &self.whole_parent_layer,
                control,
                checkpoint,
            )?;
        checkpoint()?;
        if configured_max_blocks != self.configured_max_blocks
            || actual_block_count != self.actual_block_count
            || blocks != self.blocks
            || binding != self.binding
            || !self
                .parent_graph_admission
                .same_evidence_v2(live_parent_graph_admission)
        {
            return Err(CommonArticulationContinuousLayerPathExtensionErrorV2::BindingMismatch);
        }
        checkpoint()
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

pub fn issue_common_articulation_continuous_layer_path_extension_authority_v2(
    input: CommonArticulationContinuousLayerPathExtensionInputV2<'_>,
) -> Result<
    CommonArticulationContinuousLayerPathExtensionAuthorityV2,
    CommonArticulationContinuousLayerPathExtensionErrorV2,
> {
    issue_common_articulation_continuous_layer_path_extension_authority_with_control_v2(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_common_articulation_continuous_layer_path_extension_authority_with_control_v2(
    input: CommonArticulationContinuousLayerPathExtensionInputV2<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    CommonArticulationContinuousLayerPathExtensionAuthorityV2,
    CommonArticulationContinuousLayerPathExtensionErrorV2,
> {
    issue_common_articulation_continuous_layer_path_extension_authority_with_checkpoint_v2(
        input,
        control,
        &mut || final_extension_checkpoint_v2(control),
    )
}

fn issue_common_articulation_continuous_layer_path_extension_authority_with_checkpoint_v2(
    input: CommonArticulationContinuousLayerPathExtensionInputV2<'_>,
    control: &CooperativeOperationControlV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
) -> Result<
    CommonArticulationContinuousLayerPathExtensionAuthorityV2,
    CommonArticulationContinuousLayerPathExtensionErrorV2,
> {
    let live_input = CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
        geometry: input.geometry,
        audit: input.audit,
        pose: input.pose,
        decomposition: input.decomposition,
        common_pose_limits: input.common_pose_limits,
        schedule: input.schedule,
        schedule_limits: input.schedule_limits,
        closure: input.closure,
        paper_thickness_mm: input.paper_thickness_mm,
        clearance_limits: input.clearance_limits,
        block_sources: input.block_sources,
        issuer_context: input.issuer_context,
        articulation_layer_fingerprint: input.articulation_layer_fingerprint,
        target_angles: input.target_angles,
        source: input.source,
        parent_graph_admission: input.parent_graph_admission.as_ref(),
    };
    let (binding, configured_max_blocks, actual_block_count, blocks) =
        validate_common_articulation_continuous_layer_path_extension_v2(
            live_input,
            &input.staged,
            &input.complete,
            &input.whole_parent_layer,
            control,
            checkpoint,
        )?;
    checkpoint()?;
    Ok(CommonArticulationContinuousLayerPathExtensionAuthorityV2 {
        binding,
        configured_max_blocks,
        actual_block_count,
        blocks,
        staged: input.staged,
        complete: input.complete,
        whole_parent_layer: input.whole_parent_layer,
        parent_graph_admission: input.parent_graph_admission,
    })
}

fn validate_common_articulation_continuous_layer_path_extension_v2(
    input: CommonArticulationContinuousLayerPathExtensionRevalidationInputV2<'_>,
    staged: &CommonArticulationBlockComposedPathExtensionAuthorityV1,
    complete: &CompleteMultiBlockPositiveLayerAuthorityV2,
    whole_parent_layer: &CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
    control: &CooperativeOperationControlV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
) -> Result<
    ([u8; 32], usize, usize, Vec<CanonicalBlockBindingV1>),
    CommonArticulationContinuousLayerPathExtensionErrorV2,
> {
    checkpoint()?;
    if !input.decomposition.is_for_geometry(input.geometry)
        || !input.pose.is_for_geometry(input.geometry)
        || !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || input.issuer_context == [0; 32]
        || input.articulation_layer_fingerprint == [0; 32]
    {
        return Err(CommonArticulationContinuousLayerPathExtensionErrorV2::InvalidInput);
    }
    let (configured_max_blocks, actual_block_count) = validate_extension_cardinality_v2(
        input.decomposition,
        staged,
        input.common_pose_limits,
        input.clearance_limits,
        complete,
        input.block_sources.len(),
    )?;

    checkpoint()?;
    staged
        .revalidate_with_control_v1(
            CommonArticulationBlockComposedPathExtensionRevalidationInputV1 {
                geometry: input.geometry,
                audit: input.audit,
                pose: input.pose,
                decomposition: input.decomposition,
                common_pose_limits: input.common_pose_limits,
                schedule: input.schedule,
                schedule_limits: input.schedule_limits,
                closure: input.closure,
                paper_thickness_mm: input.paper_thickness_mm,
                clearance_limits: input.clearance_limits,
            },
            control,
        )
        .map_err(map_staged_extension_error_v1)?;

    checkpoint()?;
    complete
        .revalidate_with_checkpoint_v2(
            CompleteMultiBlockPositiveLayerRevalidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                decomposition: input.decomposition,
                configured_max_blocks,
                source: input.source,
                block_sources: input.block_sources,
                paper_thickness_mm: input.paper_thickness_mm,
                issuer_context: input.issuer_context,
                articulation_layer_fingerprint: input.articulation_layer_fingerprint,
                target_angles: input.target_angles,
                whole_parent_schedule: input.schedule,
                whole_parent_closure: input.closure,
                positive_graph_limits: staged.positive_graph_limits_v1(),
                parent_graph_admission: input.parent_graph_admission,
            },
            control,
            &mut || map_final_checkpoint_to_complete_v2(checkpoint()),
        )
        .map_err(map_complete_error_v2)?;

    let parent_graph_admission_binding = input.parent_graph_admission.binding_fingerprint_v2();
    if staged.parent_graph_admission_binding_fingerprint_v2() != parent_graph_admission_binding
        || whole_parent_layer.parent_graph_admission_binding_fingerprint_v2()
            != parent_graph_admission_binding
        || complete.parent_graph_admission_binding_fingerprint_v2()
            != parent_graph_admission_binding
    {
        return Err(CommonArticulationContinuousLayerPathExtensionErrorV2::BindingMismatch);
    }

    checkpoint()?;
    let blocks = canonical_decomposition_block_bindings_v1(input.decomposition, control)
        .map_err(map_canonical_decomposition_error_v1)?;
    if blocks.len() != actual_block_count || blocks != complete.canonical_blocks_v2() {
        return Err(
            CommonArticulationContinuousLayerPathExtensionErrorV2::CanonicalBlockPartitionMismatch,
        );
    }

    checkpoint()?;
    let whole_parent_matches = whole_parent_layer
        .is_for_with_checkpoint_v1(
            input.geometry,
            input.audit,
            input.decomposition,
            configured_max_blocks,
            input.source,
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
            staged.positive_graph_limits_v1(),
            || map_final_extension_checkpoint_to_stop_v1(checkpoint()),
        )
        .map_err(map_stop_to_final_extension_error_v1)?;
    if !whole_parent_matches {
        return Err(
            CommonArticulationContinuousLayerPathExtensionErrorV2::WholeParentLayerMismatch,
        );
    }

    checkpoint()?;
    let canonical_target_angles = {
        let mut legacy_checkpoint = || map_extension_checkpoint_to_legacy_v1(checkpoint());
        canonical_target_angles_for_final_path_v1(input.target_angles, &mut legacy_checkpoint)
            .map_err(map_legacy_final_error_v1)?
    };
    let binding = common_articulation_continuous_layer_path_extension_binding_v2(
        FinalExtensionBindingInputV2 {
            schedule: input.schedule,
            closure: input.closure,
            paper_thickness_mm: input.paper_thickness_mm,
            configured_max_blocks,
            actual_block_count,
            blocks: &blocks,
            staged_binding: staged.binding_fingerprint_v1(),
            complete_binding: complete.binding_fingerprint_v2(),
            whole_parent_layer,
            issuer_context: input.issuer_context,
            articulation_layer_fingerprint: input.articulation_layer_fingerprint,
            canonical_target_angles: &canonical_target_angles,
            parent_graph_admission: input.parent_graph_admission,
        },
        checkpoint,
    )?;
    Ok((binding, configured_max_blocks, actual_block_count, blocks))
}

fn validate_extension_cardinality_v2(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    staged: &CommonArticulationBlockComposedPathExtensionAuthorityV1,
    common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    clearance_limits: CommonArticulationClearanceExtensionLimitsV1,
    complete: &CompleteMultiBlockPositiveLayerAuthorityV2,
    block_source_count: usize,
) -> Result<(usize, usize), CommonArticulationContinuousLayerPathExtensionErrorV2> {
    let configured_max_blocks = common_pose_limits.max_blocks;
    let actual_block_count = decomposition.blocks().len();
    if !(COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MIN_BLOCKS_V2
        ..=COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2)
        .contains(&configured_max_blocks)
        || !(COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MIN_BLOCKS_V2
            ..=COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2)
            .contains(&actual_block_count)
        || actual_block_count > configured_max_blocks
        || clearance_limits.max_blocks != configured_max_blocks
        || staged.configured_max_blocks_v1() != configured_max_blocks
        || staged.actual_block_count_v1() != actual_block_count
        || staged.block_count_v1() != actual_block_count
        || complete.configured_max_blocks_v2() != configured_max_blocks
        || complete.actual_block_count_v2() != actual_block_count
        || complete.block_count_v2() != actual_block_count
        || block_source_count != actual_block_count
    {
        return Err(CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit);
    }
    Ok((configured_max_blocks, actual_block_count))
}

struct FinalExtensionBindingInputV2<'a> {
    schedule: &'a CanonicalCycleScheduleV1,
    closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    configured_max_blocks: usize,
    actual_block_count: usize,
    blocks: &'a [CanonicalBlockBindingV1],
    staged_binding: [u8; 32],
    complete_binding: [u8; 32],
    whole_parent_layer: &'a CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
    canonical_target_angles: &'a [(EdgeId, f64)],
    parent_graph_admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
}

fn common_articulation_continuous_layer_path_extension_binding_v2(
    input: FinalExtensionBindingInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
) -> Result<[u8; 32], CommonArticulationContinuousLayerPathExtensionErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MODEL_ID_V2.as_bytes());
    for value in [
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MIN_BLOCKS_V2,
        input.configured_max_blocks,
        input.actual_block_count,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    hash.update(input.schedule.graph_binding_fingerprint_v1());
    hash.update(input.schedule.certificate_binding_fingerprint_v2());
    hash.update(input.closure.partition_binding_fingerprint_v2());
    hash.update(input.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(input.staged_binding);
    hash.update(input.complete_binding);

    hash.update(input.whole_parent_layer.model_id().as_bytes());
    hash.update(input.whole_parent_layer.binding_fingerprint_v1());
    hash.update(
        input
            .whole_parent_layer
            .paper_thickness_mm_v1()
            .to_bits()
            .to_le_bytes(),
    );
    hash.update(input.whole_parent_layer.target_order_hash_v1());
    hash.update((input.whole_parent_layer.transition_hashes_v1().len() as u64).to_le_bytes());
    for transition_hash in input.whole_parent_layer.transition_hashes_v1() {
        checkpoint()?;
        hash.update(transition_hash);
    }
    hash.update((input.whole_parent_layer.pair_order_count_v1() as u64).to_le_bytes());
    hash.update(input.issuer_context);
    hash.update(input.articulation_layer_fingerprint);
    hash.update(input.parent_graph_admission.model_id_v2().as_bytes());
    hash.update(
        input
            .parent_graph_admission
            .identity_namespace_v2()
            .canonical_bytes(),
    );
    hash.update(
        input
            .parent_graph_admission
            .source_revision_v2()
            .to_le_bytes(),
    );
    hash.update(input.parent_graph_admission.fold_model_fingerprint_v2());
    hash.update(input.parent_graph_admission.semantic_graph_digest_v2());
    hash.update(input.parent_graph_admission.binding_fingerprint_v2());
    let admission_limits = input.parent_graph_admission.limits_v2();
    for value in [
        admission_limits.max_faces,
        admission_limits.max_hinges,
        admission_limits.max_boundary_vertex_occurrences,
        admission_limits.max_vertices,
        admission_limits.max_edges,
        admission_limits.max_vertex_pairs,
        admission_limits.max_vertex_edge_tests,
        admission_limits.max_edge_pair_tests,
        admission_limits.max_face_pair_tests,
        admission_limits.max_point_in_polygon_edge_tests,
        admission_limits.max_exact_operations,
        admission_limits.max_logical_work,
        admission_limits.max_workspace_bytes,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    let admission_resources = input.parent_graph_admission.resources_v2();
    for value in [
        admission_resources.face_count_v2(),
        admission_resources.hinge_count_v2(),
        admission_resources.boundary_vertex_occurrences_v2(),
        admission_resources.vertex_count_v2(),
        admission_resources.edge_count_v2(),
        admission_resources.vertex_pair_tests_v2(),
        admission_resources.vertex_edge_tests_v2(),
        admission_resources.edge_pair_tests_v2(),
        admission_resources.face_pair_tests_v2(),
        admission_resources.point_in_polygon_edge_tests_v2(),
        admission_resources.exact_operations_v2(),
        admission_resources.logical_work_v2(),
        admission_resources.workspace_bytes_upper_bound_v2(),
    ] {
        hash.update((value as u64).to_le_bytes());
    }

    hash.update((input.blocks.len() as u64).to_le_bytes());
    for block in input.blocks {
        checkpoint()?;
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
        hash.update((block.faces.len() as u64).to_le_bytes());
        for face in &block.faces {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
    }
    hash.update((input.canonical_target_angles.len() as u64).to_le_bytes());
    for (edge, angle) in input.canonical_target_angles {
        checkpoint()?;
        hash.update(edge.canonical_bytes());
        hash.update(angle.to_bits().to_le_bytes());
    }
    checkpoint()?;
    Ok(hash.finalize().into())
}

fn map_staged_extension_error_v1(
    error: CommonArticulationBlockComposedPathExtensionErrorV1,
) -> CommonArticulationContinuousLayerPathExtensionErrorV2 {
    match error {
        CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
        error => CommonArticulationContinuousLayerPathExtensionErrorV2::Staged(error),
    }
}

fn map_parent_graph_admission_error_v2(
    error: CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
) -> CommonArticulationContinuousLayerPathExtensionErrorV2 {
    match error {
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit
        }
        error => CommonArticulationContinuousLayerPathExtensionErrorV2::ParentGraphAdmission(error),
    }
}

fn map_complete_error_v2(
    error: CompleteMultiBlockPositiveLayerErrorV2,
) -> CommonArticulationContinuousLayerPathExtensionErrorV2 {
    match error {
        CompleteMultiBlockPositiveLayerErrorV2::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
        CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit
        }
        CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::CanonicalBlockPartitionMismatch
        }
        CompleteMultiBlockPositiveLayerErrorV2::BlockScheduleRestrictionMismatch => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::BlockScheduleRestrictionMismatch
        }
        CompleteMultiBlockPositiveLayerErrorV2::BlockSourceRestrictionMismatch => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::BlockSourceRestrictionMismatch
        }
        CompleteMultiBlockPositiveLayerErrorV2::ParentGraphAdmission(error) => {
            map_parent_graph_admission_error_v2(error)
        }
        _ => CommonArticulationContinuousLayerPathExtensionErrorV2::CompleteMultiBlockMismatch,
    }
}

fn map_final_checkpoint_to_complete_v2(
    result: Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    result.map_err(|error| match error {
        CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded => {
            CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded
        }
        CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit => {
            CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit
        }
        _ => CompleteMultiBlockPositiveLayerErrorV2::Cancelled,
    })
}

fn map_canonical_decomposition_error_v1(
    error: CommonArticulationBlockComposedPathErrorV1,
) -> CommonArticulationContinuousLayerPathExtensionErrorV2 {
    match error {
        CommonArticulationBlockComposedPathErrorV1::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
        CommonArticulationBlockComposedPathErrorV1::ResourceLimit => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit
        }
        _ => CommonArticulationContinuousLayerPathExtensionErrorV2::CanonicalBlockPartitionMismatch,
    }
}

fn map_extension_checkpoint_to_legacy_v1(
    result: Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
) -> Result<(), CommonArticulationContinuousLayerPathErrorV1> {
    result.map_err(|error| match error {
        CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled => {
            CommonArticulationContinuousLayerPathErrorV1::Cancelled
        }
        CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded
        }
        CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit => {
            CommonArticulationContinuousLayerPathErrorV1::ResourceLimit
        }
        _ => CommonArticulationContinuousLayerPathErrorV1::BindingMismatch,
    })
}

fn map_final_extension_checkpoint_to_stop_v1(
    result: Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2>,
) -> Result<(), CooperativeOperationStopV1> {
    result.map_err(|error| match error {
        CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded => {
            CooperativeOperationStopV1::DeadlineExceeded
        }
        _ => CooperativeOperationStopV1::Cancelled,
    })
}

fn map_stop_to_final_extension_error_v1(
    stop: CooperativeOperationStopV1,
) -> CommonArticulationContinuousLayerPathExtensionErrorV2 {
    match stop {
        CooperativeOperationStopV1::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
    }
}

fn map_legacy_final_error_v1(
    error: CommonArticulationContinuousLayerPathErrorV1,
) -> CommonArticulationContinuousLayerPathExtensionErrorV2 {
    match error {
        CommonArticulationContinuousLayerPathErrorV1::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
        CommonArticulationContinuousLayerPathErrorV1::ResourceLimit => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit
        }
        CommonArticulationContinuousLayerPathErrorV1::BlockScheduleRestrictionMismatch => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::BlockScheduleRestrictionMismatch
        }
        CommonArticulationContinuousLayerPathErrorV1::BlockSourceRestrictionMismatch => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::BlockSourceRestrictionMismatch
        }
        CommonArticulationContinuousLayerPathErrorV1::WholeParentLayerMismatch => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::WholeParentLayerMismatch
        }
        _ => CommonArticulationContinuousLayerPathExtensionErrorV2::InvalidInput,
    }
}

fn final_extension_checkpoint_v2(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationContinuousLayerPathExtensionErrorV2> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded
        }
    })
}

#[cfg(test)]
#[path = "common_articulation_final_extension_tests.rs"]
mod tests;
