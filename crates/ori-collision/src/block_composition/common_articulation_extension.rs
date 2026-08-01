use ori_domain::EdgeId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV1,
    ClosedMaterialHingeGraphPose, CommonArticulationPoseErrorV1,
    CommonArticulationPoseExtensionAuthorityV1, CommonArticulationPoseExtensionInputV1,
    CommonArticulationPoseExtensionLimitsV1, CommonArticulationPoseStopV1, CycleScheduleLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    BLOCK_COMPOSITION_LIMIT_V1, CanonicalBlockBindingV1,
    CommonArticulationBlockComposedPathErrorV1, canonical_block_partition_for_staged_v1,
    canonical_decomposition_block_bindings_v1,
};
use crate::{
    CommonArticulationClearanceErrorV1, CommonArticulationClearanceExtensionLimitsV1,
    CommonArticulationClearanceExtensionPrerequisiteV1,
    CommonArticulationClearanceExtensionRevalidationInputV1, CooperativeOperationControlV1,
    CooperativeOperationStopV1,
};

pub const COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MODEL_ID_V1: &str =
    "common_articulation_block_composed_path_extension_authority_v1";
pub const COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1: usize = 11;
pub const COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MAX_BLOCKS_V1: usize =
    BLOCK_COMPOSITION_LIMIT_V1;

/// Exact live inputs for the separately typed 11..=32 staged extension.
///
/// Both opaque prerequisites are moved into a successful authority. No
/// snapshot or caller-provided fingerprint can replace either prerequisite.
pub struct CommonArticulationBlockComposedPathExtensionInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub common_pose: CommonArticulationPoseExtensionAuthorityV1,
    pub common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub clearance: CommonArticulationClearanceExtensionPrerequisiteV1,
    pub clearance_limits: CommonArticulationClearanceExtensionLimitsV1,
    /// Caller-facing edge partition. It must equal the canonical extension
    /// decomposition exactly after canonical ordering.
    pub blocks: Vec<Vec<EdgeId>>,
}

/// Exact live inputs required to revalidate one retained staged extension.
#[derive(Clone, Copy)]
pub struct CommonArticulationBlockComposedPathExtensionRevalidationInputV1<'a> {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationBlockComposedPathExtensionErrorV1 {
    #[error("the staged common-articulation extension input is malformed")]
    InvalidInput,
    #[error("the staged common-articulation extension exceeded a resource limit")]
    ResourceLimit,
    #[error("the submitted edge partition differs from the canonical extension decomposition")]
    CanonicalBlockPartitionMismatch,
    #[error("the common-articulation pose extension failed exact revalidation: {0}")]
    CommonPose(CommonArticulationPoseErrorV1),
    #[error("the common-articulation clearance extension failed exact revalidation: {0}")]
    Clearance(CommonArticulationClearanceErrorV1),
    #[error("the staged common-articulation extension operation was cancelled")]
    Cancelled,
    #[error("the staged common-articulation extension operation deadline elapsed")]
    DeadlineExceeded,
}

/// Opaque non-authorizing staged composition for 11 through one configured
/// cap no greater than 32.
///
/// The authority is deliberately neither cloneable nor serializable. Its
/// distinct type cannot enter the legacy final, desktop, Apply, or viewer
/// paths.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationBlockComposedPathAuthorityV1,
///     CommonArticulationBlockComposedPathExtensionAuthorityV1,
/// };
///
/// fn legacy_stage(_: CommonArticulationBlockComposedPathAuthorityV1) {}
/// fn cannot_route(extension: CommonArticulationBlockComposedPathExtensionAuthorityV1) {
///     legacy_stage(extension);
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationBlockComposedPathExtensionAuthorityV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationBlockComposedPathExtensionAuthorityV1>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationBlockComposedPathExtensionAuthorityV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationBlockComposedPathExtensionAuthorityV1>();
/// ```
#[derive(Debug)]
pub struct CommonArticulationBlockComposedPathExtensionAuthorityV1 {
    binding: [u8; 32],
    configured_max_blocks: usize,
    actual_block_count: usize,
    blocks: Vec<CanonicalBlockBindingV1>,
    common_pose: CommonArticulationPoseExtensionAuthorityV1,
    clearance: CommonArticulationClearanceExtensionPrerequisiteV1,
}

impl CommonArticulationBlockComposedPathExtensionAuthorityV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MODEL_ID_V1
    }

    #[must_use]
    pub const fn binding_fingerprint_v1(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub const fn common_pose_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.common_pose.binding_fingerprint_v1()
    }

    #[must_use]
    pub const fn clearance_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.clearance.binding_fingerprint_v1()
    }

    #[must_use]
    pub const fn configured_max_blocks_v1(&self) -> usize {
        self.configured_max_blocks
    }

    #[must_use]
    pub const fn actual_block_count_v1(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub fn block_count_v1(&self) -> usize {
        self.blocks.len()
    }

    pub fn revalidate_v1(
        &self,
        input: CommonArticulationBlockComposedPathExtensionRevalidationInputV1<'_>,
    ) -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1> {
        self.revalidate_with_control_v1(input, &CooperativeOperationControlV1::unbounded())
    }

    pub fn revalidate_with_control_v1(
        &self,
        input: CommonArticulationBlockComposedPathExtensionRevalidationInputV1<'_>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1> {
        self.revalidate_with_checkpoint_v1(input, control, &mut || {
            staged_extension_checkpoint_v1(control)
        })
    }

    fn revalidate_with_checkpoint_v1(
        &self,
        input: CommonArticulationBlockComposedPathExtensionRevalidationInputV1<'_>,
        control: &CooperativeOperationControlV1<'_>,
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1>,
    ) -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1> {
        checkpoint()?;
        validate_basic_live_input_v1(
            input.geometry,
            input.pose,
            input.decomposition,
            input.paper_thickness_mm,
        )?;
        let (configured_max_blocks, actual_block_count) = validate_extension_cardinality_v1(
            input.decomposition,
            &self.common_pose,
            input.common_pose_limits,
            &self.clearance,
            input.clearance_limits,
        )?;
        self.common_pose
            .revalidate_with_checkpoint_v1(
                CommonArticulationPoseExtensionInputV1 {
                    geometry: input.geometry,
                    pose: input.pose,
                    decomposition: input.decomposition,
                    paper_thickness_mm: input.paper_thickness_mm,
                    limits: input.common_pose_limits,
                },
                || staged_extension_common_pose_checkpoint_v1(control),
            )
            .map_err(map_extension_common_pose_error_v1)?;
        checkpoint()?;
        self.clearance
            .revalidate_with_control_v1(
                CommonArticulationClearanceExtensionRevalidationInputV1 {
                    geometry: input.geometry,
                    audit: input.audit,
                    pose: input.pose,
                    decomposition: input.decomposition,
                    common_pose: &self.common_pose,
                    common_pose_limits: input.common_pose_limits,
                    schedule: input.schedule,
                    schedule_limits: input.schedule_limits,
                    closure: input.closure,
                    paper_thickness_mm: input.paper_thickness_mm,
                    limits: input.clearance_limits,
                },
                control,
            )
            .map_err(map_extension_clearance_error_v1)?;
        checkpoint()?;
        let blocks = canonical_decomposition_block_bindings_v1(input.decomposition, control)
            .map_err(map_legacy_partition_error_v1)?;
        let binding = common_articulation_block_composed_path_extension_binding_v1(
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
            configured_max_blocks,
            actual_block_count,
            &blocks,
            self.common_pose.binding_fingerprint_v1(),
            self.clearance.binding_fingerprint_v1(),
        );
        if self.configured_max_blocks != configured_max_blocks
            || self.actual_block_count != actual_block_count
            || self.blocks != blocks
            || self.binding != binding
        {
            return Err(
                CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch,
            );
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

pub fn issue_common_articulation_block_composed_path_extension_authority_v1(
    input: CommonArticulationBlockComposedPathExtensionInputV1<'_>,
) -> Result<
    CommonArticulationBlockComposedPathExtensionAuthorityV1,
    CommonArticulationBlockComposedPathExtensionErrorV1,
> {
    issue_common_articulation_block_composed_path_extension_authority_with_control_v1(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_common_articulation_block_composed_path_extension_authority_with_control_v1(
    input: CommonArticulationBlockComposedPathExtensionInputV1<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    CommonArticulationBlockComposedPathExtensionAuthorityV1,
    CommonArticulationBlockComposedPathExtensionErrorV1,
> {
    issue_common_articulation_block_composed_path_extension_authority_with_checkpoint_v1(
        input,
        control,
        &mut || staged_extension_checkpoint_v1(control),
    )
}

fn issue_common_articulation_block_composed_path_extension_authority_with_checkpoint_v1(
    input: CommonArticulationBlockComposedPathExtensionInputV1<'_>,
    control: &CooperativeOperationControlV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1>,
) -> Result<
    CommonArticulationBlockComposedPathExtensionAuthorityV1,
    CommonArticulationBlockComposedPathExtensionErrorV1,
> {
    checkpoint()?;
    validate_basic_live_input_v1(
        input.geometry,
        input.pose,
        input.decomposition,
        input.paper_thickness_mm,
    )?;
    let (configured_max_blocks, actual_block_count) = validate_extension_cardinality_v1(
        input.decomposition,
        &input.common_pose,
        input.common_pose_limits,
        &input.clearance,
        input.clearance_limits,
    )?;
    if input.blocks.len() != actual_block_count {
        return Err(
            CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch,
        );
    }

    checkpoint()?;
    input
        .common_pose
        .revalidate_with_checkpoint_v1(
            CommonArticulationPoseExtensionInputV1 {
                geometry: input.geometry,
                pose: input.pose,
                decomposition: input.decomposition,
                paper_thickness_mm: input.paper_thickness_mm,
                limits: input.common_pose_limits,
            },
            || staged_extension_common_pose_checkpoint_v1(control),
        )
        .map_err(map_extension_common_pose_error_v1)?;
    checkpoint()?;
    input
        .clearance
        .revalidate_with_control_v1(
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                geometry: input.geometry,
                audit: input.audit,
                pose: input.pose,
                decomposition: input.decomposition,
                common_pose: &input.common_pose,
                common_pose_limits: input.common_pose_limits,
                schedule: input.schedule,
                schedule_limits: input.schedule_limits,
                closure: input.closure,
                paper_thickness_mm: input.paper_thickness_mm,
                limits: input.clearance_limits,
            },
            control,
        )
        .map_err(map_extension_clearance_error_v1)?;

    checkpoint()?;
    let canonical = canonical_block_partition_for_staged_v1(input.geometry, input.blocks, control)
        .map_err(map_legacy_partition_error_v1)?;
    let decomposition = canonical_decomposition_block_bindings_v1(input.decomposition, control)
        .map_err(map_legacy_partition_error_v1)?;
    if canonical != decomposition || canonical.len() != actual_block_count {
        return Err(
            CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch,
        );
    }

    checkpoint()?;
    let binding = common_articulation_block_composed_path_extension_binding_v1(
        input.schedule,
        input.closure,
        input.paper_thickness_mm,
        configured_max_blocks,
        actual_block_count,
        &canonical,
        input.common_pose.binding_fingerprint_v1(),
        input.clearance.binding_fingerprint_v1(),
    );
    checkpoint()?;
    Ok(CommonArticulationBlockComposedPathExtensionAuthorityV1 {
        binding,
        configured_max_blocks,
        actual_block_count,
        blocks: canonical,
        common_pose: input.common_pose,
        clearance: input.clearance,
    })
}

fn validate_basic_live_input_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    paper_thickness_mm: f64,
) -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1> {
    if !decomposition.is_for_geometry(geometry)
        || !pose.is_for_geometry(geometry)
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
    {
        return Err(CommonArticulationBlockComposedPathExtensionErrorV1::InvalidInput);
    }
    Ok(())
}

fn validate_extension_cardinality_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
    common_pose: &CommonArticulationPoseExtensionAuthorityV1,
    common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    clearance: &CommonArticulationClearanceExtensionPrerequisiteV1,
    clearance_limits: CommonArticulationClearanceExtensionLimitsV1,
) -> Result<(usize, usize), CommonArticulationBlockComposedPathExtensionErrorV1> {
    let actual_block_count = decomposition.blocks().len();
    let configured_max_blocks = common_pose_limits.max_blocks;
    if !(COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1
        ..=COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MAX_BLOCKS_V1)
        .contains(&actual_block_count)
        || !(COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1
            ..=COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MAX_BLOCKS_V1)
            .contains(&configured_max_blocks)
        || actual_block_count > configured_max_blocks
        || common_pose.configured_max_blocks_v1() != configured_max_blocks
        || common_pose.block_count_v1() != actual_block_count
        || clearance_limits.max_blocks != configured_max_blocks
        || clearance.configured_max_blocks_v1() != configured_max_blocks
        || clearance.actual_block_count_v1() != actual_block_count
    {
        return Err(CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit);
    }
    Ok((configured_max_blocks, actual_block_count))
}

#[allow(clippy::too_many_arguments)]
fn common_articulation_block_composed_path_extension_binding_v1(
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    configured_max_blocks: usize,
    actual_block_count: usize,
    blocks: &[CanonicalBlockBindingV1],
    common_pose_binding: [u8; 32],
    clearance_binding: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MODEL_ID_V1.as_bytes());
    for value in [
        COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_EXTENSION_MIN_BLOCKS_V1,
        configured_max_blocks,
        actual_block_count,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    hash.update(schedule.certificate_binding_fingerprint_v2());
    hash.update(closure.partition_binding_fingerprint_v2());
    hash.update(paper_thickness_mm.to_bits().to_be_bytes());
    hash.update(common_pose_binding);
    hash.update(clearance_binding);
    hash.update((blocks.len() as u64).to_be_bytes());
    for block in blocks {
        hash.update((block.edges.len() as u64).to_be_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
        hash.update((block.faces.len() as u64).to_be_bytes());
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
    }
    hash.finalize().into()
}

fn map_legacy_partition_error_v1(
    error: CommonArticulationBlockComposedPathErrorV1,
) -> CommonArticulationBlockComposedPathExtensionErrorV1 {
    match error {
        CommonArticulationBlockComposedPathErrorV1::InvalidInput => {
            CommonArticulationBlockComposedPathExtensionErrorV1::InvalidInput
        }
        CommonArticulationBlockComposedPathErrorV1::ResourceLimit => {
            CommonArticulationBlockComposedPathExtensionErrorV1::ResourceLimit
        }
        CommonArticulationBlockComposedPathErrorV1::Cancelled => {
            CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled
        }
        CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded
        }
        CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch
        | CommonArticulationBlockComposedPathErrorV1::CommonPose(_)
        | CommonArticulationBlockComposedPathErrorV1::Clearance(_) => {
            CommonArticulationBlockComposedPathExtensionErrorV1::CanonicalBlockPartitionMismatch
        }
    }
}

fn map_extension_clearance_error_v1(
    error: CommonArticulationClearanceErrorV1,
) -> CommonArticulationBlockComposedPathExtensionErrorV1 {
    match error {
        CommonArticulationClearanceErrorV1::Cancelled => {
            CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled
        }
        CommonArticulationClearanceErrorV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded
        }
        error => CommonArticulationBlockComposedPathExtensionErrorV1::Clearance(error),
    }
}

fn map_extension_common_pose_error_v1(
    error: CommonArticulationPoseErrorV1,
) -> CommonArticulationBlockComposedPathExtensionErrorV1 {
    match error {
        CommonArticulationPoseErrorV1::Cancelled => {
            CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled
        }
        CommonArticulationPoseErrorV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded
        }
        error => CommonArticulationBlockComposedPathExtensionErrorV1::CommonPose(error),
    }
}

fn staged_extension_common_pose_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationPoseStopV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => CommonArticulationPoseStopV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationPoseStopV1::DeadlineExceeded
        }
    })
}

fn staged_extension_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CommonArticulationBlockComposedPathExtensionErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            CommonArticulationBlockComposedPathExtensionErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            CommonArticulationBlockComposedPathExtensionErrorV1::DeadlineExceeded
        }
    })
}

#[cfg(test)]
#[path = "common_articulation_extension_tests.rs"]
mod tests;
