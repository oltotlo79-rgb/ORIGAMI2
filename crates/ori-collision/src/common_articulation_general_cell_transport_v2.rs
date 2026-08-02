//! General-N, unpromoted layer-source transport observations.
//!
//! This boundary deliberately records a `LayerOrderSnapshot` only after the
//! complete V2 common-articulation prerequisite tuple has been replayed.  It
//! does not establish a positive-thickness layer-transport theorem: the V2
//! clearance prerequisite itself is intentionally unpromoted.  Consequently
//! this module is useful for deterministic replay and resource accounting,
//! but is never an apply, viewer, collision, motion, or layer authority.
//! The sealed source binds the prepared geometry's fold-model fingerprint,
//! source revision, and material-sheet identity namespace. It is not an
//! editor-current/project-mutation proof; that stronger application boundary
//! remains intentionally outside this module.

use ori_domain::FaceId;
use ori_foldability::{GlobalFlatLayerOrderSourceAuthorityV2, LayerOrderSnapshot};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationBlockClosureSetV2,
    CommonArticulationPoseAuthorityV2, CommonArticulationResourceProfileV2,
    CommonArticulationWholeParentClosureLimitsV2, CommonArticulationWholeParentClosureV2,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use thiserror::Error;

use crate::{
    CommonArticulationClearanceErrorV2, CommonArticulationClearancePrerequisiteV2,
    CommonArticulationClearanceRevalidationInputV2, CommonArticulationClearanceStopV2,
};

/// Stable domain identifier for the retained general-N observation.
pub const COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_MODEL_ID_V2: &str =
    "common_articulation_general_cell_transport_v2";
/// Stable domain identifier for the explicit non-promotion outcome.
pub const COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_UNPROMOTED_MODEL_ID_V2: &str =
    "common_articulation_general_cell_transport_unpromoted_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;

/// Cooperative stop requested while issuing or replaying a V2 observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationGeneralCellTransportStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Fail-closed V2 transport-observation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationGeneralCellTransportErrorV2 {
    #[error("the general-N layer transport input is malformed")]
    InvalidInput,
    #[error("the general-N layer transport input exceeds an explicit resource limit")]
    ResourceLimit,
    #[error("the retained V2 clearance prerequisite does not replay: {0}")]
    Clearance(CommonArticulationClearanceErrorV2),
    #[error("the supplied layer-order source is malformed or foreign")]
    SourceBindingMismatch,
    #[error("the retained general-N layer transport observation does not match the live input")]
    PrerequisiteBindingMismatch,
    #[error("the general-N layer transport operation was cancelled")]
    Cancelled,
    #[error("the general-N layer transport operation deadline elapsed")]
    DeadlineExceeded,
}

/// Caller-owned upper bounds for one retained general-N layer source.
///
/// `max_blocks` is deliberately exact: it must equal the profile's configured
/// cap, rather than merely allowing the observed block count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationGeneralCellTransportLimitsV2 {
    pub max_blocks: usize,
    pub max_source_retained_bytes: usize,
    pub max_material_faces: usize,
    pub max_folded_faces: usize,
    pub max_overlap_cells: usize,
    pub max_face_pair_orders: usize,
    pub max_global_order_faces: usize,
    pub max_layer_records: usize,
    pub max_boundary_vertices: usize,
    pub max_boundary_samples: usize,
    pub max_transitions: usize,
    pub max_logical_work: usize,
    pub max_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

/// Exact live inputs for issuing an unpromoted general-N observation.
pub struct CommonArticulationGeneralCellTransportInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub block_closure_set: &'a CommonArticulationBlockClosureSetV2,
    pub whole_parent_closure: &'a CommonArticulationWholeParentClosureV2,
    pub whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    pub clearance: &'a CommonArticulationClearancePrerequisiteV2,
    /// Opaque source authority emitted by a completed global-foldability run.
    pub source_authority: GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    pub limits: CommonArticulationGeneralCellTransportLimitsV2,
}

/// Exact live inputs required to replay an unpromoted V2 observation.
pub struct CommonArticulationGeneralCellTransportRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub block_closure_set: &'a CommonArticulationBlockClosureSetV2,
    pub whole_parent_closure: &'a CommonArticulationWholeParentClosureV2,
    pub whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    pub clearance: &'a CommonArticulationClearancePrerequisiteV2,
    pub source_authority: GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    pub limits: CommonArticulationGeneralCellTransportLimitsV2,
}

/// Why the current V2 result is deliberately not a layer-transport proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationGeneralCellTransportUnpromotedReasonV2 {
    /// The V2 prerequisite validates bindings but has no whole-parent
    /// positive-thickness continuous layer theorem to promote.
    WholeParentPositiveThicknessEvidenceUnavailable,
}

/// Sealed, retained observation of a canonical layer-order source.
///
/// No V1 conversion, `Deref`, `Clone`, or persistence trait is supplied.
/// The retained source remains private so callers must replay their live
/// source rather than treating this observation as a reusable layer proof.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
///     CommonArticulationGeneralCellTransportPrerequisiteV2,
/// };
///
/// fn accepts_v1(_: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1) {}
/// fn rejects_v2(value: CommonArticulationGeneralCellTransportPrerequisiteV2) {
///     accepts_v1(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationGeneralCellTransportPrerequisiteV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationGeneralCellTransportPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationGeneralCellTransportPrerequisiteV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationGeneralCellTransportPrerequisiteV2>();
/// ```
#[derive(Debug)]
pub struct CommonArticulationGeneralCellTransportPrerequisiteV2 {
    profile_binding: [u8; 32],
    decomposition_binding: [u8; 32],
    common_pose_binding: [u8; 32],
    block_closure_set_binding: [u8; 32],
    whole_parent_closure_binding: [u8; 32],
    clearance_binding: [u8; 32],
    audit_binding: [u8; 32],
    parent_schedule_binding: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    closure_tolerance_bits: u64,
    actual_block_count: usize,
    source_digest: [u8; 32],
    source_provenance: ori_foldability::GlobalFlatFoldabilityProvenance,
    source_metrics: SourceMetricsV2,
    resource: TransportResourceWorkV2,
    limits: CommonArticulationGeneralCellTransportLimitsV2,
    source: LayerOrderSnapshot,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationGeneralCellTransportPrerequisiteV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_MODEL_ID_V2
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn source_digest_v2(&self) -> [u8; 32] {
        self.source_digest
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.resource.logical_work
    }

    #[must_use]
    pub const fn retained_bytes_v2(&self) -> usize {
        self.resource.retained_bytes
    }

    #[must_use]
    pub const fn peak_bytes_v2(&self) -> usize {
        self.resource.peak_bytes
    }

    #[must_use]
    pub const fn unpromoted_reason_v2(
        &self,
    ) -> CommonArticulationGeneralCellTransportUnpromotedReasonV2 {
        CommonArticulationGeneralCellTransportUnpromotedReasonV2::WholeParentPositiveThicknessEvidenceUnavailable
    }

    /// Replays every live V2 prerequisite and the complete retained source.
    pub fn revalidate_v2(
        &self,
        input: CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
    ) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
        self.revalidate_borrowed_with_checkpoint_v2(&input, &mut checkpoint)
    }

    pub(crate) fn revalidate_borrowed_with_checkpoint_v2(
        &self,
        input: &CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
        checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
    ) -> Result<(), CommonArticulationGeneralCellTransportErrorV2> {
        checkpoint_v2(checkpoint)?;
        let validated = validate_input_v2(input, checkpoint)?;
        let binding = transport_binding_fingerprint_v2(&validated, checkpoint)?;
        let sources_match = source_equal_with_checkpoint_v2(
            &self.source,
            input.source_authority.layer_order_snapshot_v2(),
            checkpoint,
        )?;
        checkpoint_v2(checkpoint)?;
        let matches = self.profile_binding == validated.profile_binding
            && self.decomposition_binding == validated.decomposition_binding
            && self.common_pose_binding == validated.common_pose_binding
            && self.block_closure_set_binding == validated.block_closure_set_binding
            && self.whole_parent_closure_binding == validated.whole_parent_closure_binding
            && self.clearance_binding == validated.clearance_binding
            && self.audit_binding == validated.audit_binding
            && self.parent_schedule_binding == validated.parent_schedule_binding
            && self.parent_fixed_face == validated.parent_fixed_face
            && self.paper_thickness_bits == validated.paper_thickness_bits
            && self.closure_tolerance_bits == validated.closure_tolerance_bits
            && self.actual_block_count == validated.actual_block_count
            && self.source_digest == validated.source_digest
            && self.source_provenance == validated.source_provenance
            && self.source_metrics == validated.source_metrics
            && self.resource == validated.resource
            && self.limits == input.limits
            && self.binding_fingerprint == binding;
        if !matches || !sources_match {
            checkpoint_v2(checkpoint)?;
            return Err(CommonArticulationGeneralCellTransportErrorV2::PrerequisiteBindingMismatch);
        }
        checkpoint_v2(checkpoint)
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

/// Current V2 transport result.  It has no certified variant.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
///     CommonArticulationGeneralCellTransportOutcomeV2,
/// };
/// fn accepts_v1(_: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1) {}
/// fn rejects_v2(value: CommonArticulationGeneralCellTransportOutcomeV2) { accepts_v1(value); }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationGeneralCellTransportOutcomeV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationGeneralCellTransportOutcomeV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationGeneralCellTransportOutcomeV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationGeneralCellTransportOutcomeV2>();
/// ```
#[derive(Debug)]
pub enum CommonArticulationGeneralCellTransportOutcomeV2 {
    Unpromoted(Box<CommonArticulationGeneralCellTransportPrerequisiteV2>),
}

impl CommonArticulationGeneralCellTransportOutcomeV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_GENERAL_CELL_TRANSPORT_UNPROMOTED_MODEL_ID_V2
    }
    #[must_use]
    pub const fn is_certified_v2(&self) -> bool {
        false
    }
    #[must_use]
    pub fn as_unpromoted_v2(&self) -> &CommonArticulationGeneralCellTransportPrerequisiteV2 {
        match self {
            Self::Unpromoted(value) => value,
        }
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

/// Issues an unpromoted observation after a complete, checkpointed V2 replay.
pub fn issue_common_articulation_general_cell_transport_prerequisite_v2(
    input: CommonArticulationGeneralCellTransportInputV2<'_>,
) -> Result<
    CommonArticulationGeneralCellTransportOutcomeV2,
    CommonArticulationGeneralCellTransportErrorV2,
> {
    issue_common_articulation_general_cell_transport_prerequisite_with_checkpoint_v2(input, || {
        Ok(())
    })
}

/// As [`issue_common_articulation_general_cell_transport_prerequisite_v2`],
/// with cooperative cancellation and deadline checkpoints.
pub fn issue_common_articulation_general_cell_transport_prerequisite_with_checkpoint_v2(
    input: CommonArticulationGeneralCellTransportInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<
    CommonArticulationGeneralCellTransportOutcomeV2,
    CommonArticulationGeneralCellTransportErrorV2,
> {
    checkpoint_v2(&mut checkpoint)?;
    let revalidation = CommonArticulationGeneralCellTransportRevalidationInputV2 {
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
        clearance: input.clearance,
        source_authority: input.source_authority,
        limits: input.limits,
    };
    let validated = validate_input_v2(&revalidation, &mut checkpoint)?;
    let source = {
        let mut clone_checkpoint = || checkpoint_v2(&mut checkpoint);
        revalidation
            .source_authority
            .layer_order_snapshot_v2()
            .try_clone_with_retained_byte_limit_with_checkpoint_v2(
                validated.limits.max_source_retained_bytes,
                &mut clone_checkpoint,
            )?
            .map_err(|_| CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?
    };
    let clone_within_limit = {
        let mut size_checkpoint = || checkpoint_v2(&mut checkpoint);
        source.checked_deep_retained_bytes_with_limit_and_checkpoint_v2(
            validated.limits.max_source_retained_bytes,
            &mut size_checkpoint,
        )?
    };
    if matches!(
        clone_within_limit,
        ori_foldability::LayerOrderSnapshotRetainedByteLimitV2::Exceeded { .. }
    ) {
        return Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit);
    }
    let binding_fingerprint = transport_binding_fingerprint_v2(&validated, &mut checkpoint)?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(CommonArticulationGeneralCellTransportOutcomeV2::Unpromoted(
        Box::new(CommonArticulationGeneralCellTransportPrerequisiteV2 {
            profile_binding: validated.profile_binding,
            decomposition_binding: validated.decomposition_binding,
            common_pose_binding: validated.common_pose_binding,
            block_closure_set_binding: validated.block_closure_set_binding,
            whole_parent_closure_binding: validated.whole_parent_closure_binding,
            clearance_binding: validated.clearance_binding,
            audit_binding: validated.audit_binding,
            parent_schedule_binding: validated.parent_schedule_binding,
            parent_fixed_face: validated.parent_fixed_face,
            paper_thickness_bits: validated.paper_thickness_bits,
            closure_tolerance_bits: validated.closure_tolerance_bits,
            actual_block_count: validated.actual_block_count,
            source_digest: validated.source_digest,
            source_provenance: validated.source_provenance,
            source_metrics: validated.source_metrics,
            resource: validated.resource,
            limits: input.limits,
            source,
            binding_fingerprint,
        }),
    ))
}

mod compact_pair_source;
mod resource;
mod source_binding;
mod validation;
mod whole_parent_positive_thickness;

pub use compact_pair_source::*;
pub use whole_parent_positive_thickness::*;

use resource::{TransportResourceWorkV2, checked_transport_resource_work_v2};
use source_binding::{SourceMetricsV2, source_equal_with_checkpoint_v2};
use validation::{checkpoint_v2, transport_binding_fingerprint_v2, validate_input_v2};

#[cfg(test)]
#[path = "../../../test-support/n33_compact_pair_assignment_v2.rs"]
mod n33_compact_pair_assignment_fixture_v2;
#[cfg(test)]
#[path = "common_articulation_general_cell_transport_v2/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "common_articulation_general_cell_transport_v2/tests.rs"]
mod tests;
