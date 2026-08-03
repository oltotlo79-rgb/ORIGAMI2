//! Instance-bound canonical binary64 transform realization for two graph poses.
//!
//! This seam regenerates only the deterministic spanning-tree transforms used
//! by [`MaterialHingeGraphGeometry::solve_closed`]. It does not strengthen the
//! caller-tolerance closure observation retained by either pose.

use std::sync::Arc;

use ori_domain::FaceId;
use thiserror::Error;

use crate::{
    ClosedMaterialHingeGraphPose, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    MaterialHingeGraphInstanceV1,
};

mod binding;
mod resources;
mod validation;

pub const CANONICAL_BINARY64_POSE_PAIR_TRANSFORM_REALIZATION_EVIDENCE_MODEL_ID_V2: &str =
    "canonical_binary64_pose_pair_transform_realization_evidence_v2";

const CANONICAL_BINARY64_REALIZED_POSE_COUNT_V2: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalBinary64PosePairTransformRealizationStopV2 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalBinary64PosePairTransformRealizationErrorV2 {
    #[error("canonical binary64 transform realization exceeds its finite resource envelope")]
    ResourceLimit,
    #[error("the audit does not describe the submitted canonical geometry")]
    AuditMismatch,
    #[error("a submitted pose was not issued for the exact geometry and fixed face")]
    PoseIssuerMismatch,
    #[error("a submitted pose transform is not the canonical binary64 spanning-tree realization")]
    TransformMismatch,
    #[error("canonical binary64 transform realization does not match the live replay")]
    CertificateBindingMismatch,
    #[error("canonical binary64 transform realization was cancelled")]
    Cancelled,
    #[error("canonical binary64 transform realization deadline elapsed")]
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalBinary64PosePairTransformRealizationLimitsV2 {
    /// Upper bound for the live canonical face count.
    pub max_faces: usize,
    /// Upper bound for the live canonical hinge count.
    pub max_hinges: usize,
    /// Upper bound for both externally retained pose objects.
    pub max_pose_pair_deep_retained_bytes: usize,
    /// Exact logical-work policy identity.
    pub max_logical_work: usize,
    /// Upper cap for physically observed temporary vector capacity. Replay
    /// retains this policy value exactly while allowing allocator variation
    /// within the cap.
    pub max_workspace_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalBinary64PosePairTransformRealizationResourceBoundV2 {
    pub(super) face_count: usize,
    pub(super) hinge_count: usize,
    pub(super) spanning_hinge_count: usize,
    pub(super) pose_pair_deep_retained_bytes: usize,
    pub(super) logical_work: usize,
    pub(super) workspace_structural_requirement_bytes: usize,
}

impl CanonicalBinary64PosePairTransformRealizationResourceBoundV2 {
    #[must_use]
    pub const fn face_count_v2(self) -> usize {
        self.face_count
    }

    #[must_use]
    pub const fn hinge_count_v2(self) -> usize {
        self.hinge_count
    }

    #[must_use]
    pub const fn pose_pair_deep_retained_bytes_v2(self) -> usize {
        self.pose_pair_deep_retained_bytes
    }

    #[must_use]
    pub const fn logical_work_required_v2(self) -> usize {
        self.logical_work
    }

    /// Deterministic reservation requirement derived from element sizes and
    /// requested lengths. Allocator-dependent capacity is checked separately
    /// against the caller's workspace cap during issuance and replay.
    #[must_use]
    pub const fn workspace_structural_requirement_bytes_v2(self) -> usize {
        self.workspace_structural_requirement_bytes
    }
}

#[derive(Clone, Copy)]
pub struct CanonicalBinary64PosePairTransformRealizationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub fixed_face: FaceId,
    pub lower_pose: &'a ClosedMaterialHingeGraphPose,
    pub upper_pose: &'a ClosedMaterialHingeGraphPose,
    pub limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CanonicalBinary64PosePairTransformRealizationResourcesV2 {
    pub(super) face_count: usize,
    pub(super) hinge_count: usize,
    pub(super) spanning_hinge_count: usize,
    pub(super) pose_pair_deep_retained_bytes: usize,
    pub(super) logical_work: usize,
    pub(super) workspace_structural_requirement_bytes: usize,
}

/// Opaque evidence for two exact pose instances' implementation-level,
/// canonical binary64 spanning-tree transform realization.
///
/// Pose `Arc` identities are retained outside the deterministic fingerprint.
/// This type is intentionally non-`Clone`, non-serde and non-`Deref`.
///
/// ```compile_fail
/// use ori_kinematics::CanonicalBinary64PosePairTransformRealizationEvidenceV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CanonicalBinary64PosePairTransformRealizationEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalBinary64PosePairTransformRealizationEvidenceV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CanonicalBinary64PosePairTransformRealizationEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalBinary64PosePairTransformRealizationEvidenceV2;
/// fn require_deserialize<T: serde::de::DeserializeOwned>() {}
/// require_deserialize::<CanonicalBinary64PosePairTransformRealizationEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalBinary64PosePairTransformRealizationEvidenceV2;
/// fn require_deref<T: std::ops::Deref>() {}
/// require_deref::<CanonicalBinary64PosePairTransformRealizationEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalBinary64PosePairTransformRealizationEvidenceV2;
/// fn fabricate() -> CanonicalBinary64PosePairTransformRealizationEvidenceV2 {
///     CanonicalBinary64PosePairTransformRealizationEvidenceV2 {}
/// }
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalBinary64PosePairTransformRealizationEvidenceV2;
/// fn expose(value: &CanonicalBinary64PosePairTransformRealizationEvidenceV2) {
///     let _ = value.lower_pose_instance;
/// }
/// ```
pub struct CanonicalBinary64PosePairTransformRealizationEvidenceV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    lower_pose_instance: Arc<()>,
    upper_pose_instance: Arc<()>,
    fixed_face: FaceId,
    audit_binding: [u8; 32],
    resources: CanonicalBinary64PosePairTransformRealizationResourcesV2,
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug for CanonicalBinary64PosePairTransformRealizationEvidenceV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalBinary64PosePairTransformRealizationEvidenceV2")
            .field("model", &self.model_id_v2())
            .field("face_count", &self.resources.face_count)
            .field("hinge_count", &self.resources.hinge_count)
            .field("realized_pose_count", &self.realized_pose_count_v2())
            .field("logical_work", &self.resources.logical_work)
            .field(
                "workspace_structural_requirement_bytes",
                &self.resources.workspace_structural_requirement_bytes,
            )
            .finish_non_exhaustive()
    }
}

impl CanonicalBinary64PosePairTransformRealizationEvidenceV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        CANONICAL_BINARY64_POSE_PAIR_TRANSFORM_REALIZATION_EVIDENCE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn realized_pose_count_v2(&self) -> usize {
        CANONICAL_BINARY64_REALIZED_POSE_COUNT_V2
    }

    #[must_use]
    pub const fn face_count_v2(&self) -> usize {
        self.resources.face_count
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.resources.hinge_count
    }

    #[must_use]
    pub const fn fixed_face_v2(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.resources.logical_work
    }

    /// Deterministic reservation requirement retained in the evidence
    /// binding; this is not a claim about allocator-selected capacity.
    #[must_use]
    pub const fn workspace_structural_requirement_bytes_v2(&self) -> usize {
        self.resources.workspace_structural_requirement_bytes
    }

    /// Retained upper cap enforced on temporary allocation capacity before a
    /// replay can succeed. The deterministic structural requirement can be
    /// smaller because allocator capacity growth is permitted up to this cap.
    #[must_use]
    pub const fn workspace_peak_bytes_upper_bound_v2(&self) -> usize {
        self.limits.max_workspace_bytes
    }

    #[must_use]
    pub const fn pose_pair_deep_retained_bytes_v2(&self) -> usize {
        self.resources.pose_pair_deep_retained_bytes
    }

    #[must_use]
    pub fn matches_geometry_instance_v2(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.issuer_geometry.matches(geometry)
    }

    #[must_use]
    pub fn matches_pose_instances_v2(
        &self,
        lower_pose: &ClosedMaterialHingeGraphPose,
        upper_pose: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        Arc::ptr_eq(&self.lower_pose_instance, &lower_pose.instance_anchor_v2())
            && Arc::ptr_eq(&self.upper_pose_instance, &upper_pose.instance_anchor_v2())
    }

    pub fn revalidate_v2(
        &self,
        input: CanonicalBinary64PosePairTransformRealizationInputV2<'_>,
    ) -> Result<(), CanonicalBinary64PosePairTransformRealizationErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CanonicalBinary64PosePairTransformRealizationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CanonicalBinary64PosePairTransformRealizationStopV2>,
    ) -> Result<(), CanonicalBinary64PosePairTransformRealizationErrorV2> {
        validation::revalidate_v2(self, input, &mut checkpoint)
    }

    /// The one positive fact established by this evidence.
    #[must_use]
    pub const fn proves_both_pose_instances_are_canonical_binary64_transform_realizations_v2(
        &self,
    ) -> bool {
        true
    }

    #[must_use]
    pub const fn authorizes_source_target_identity(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_current_requested_identity(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_application_parameter_identity(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_direction(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_layer_order(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_exact_closure(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_transform_realization(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_pose_realization(&self) -> bool {
        false
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
    #[must_use]
    pub const fn authorizes_export(&self) -> bool {
        false
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[doc(hidden)]
    #[must_use]
    pub fn replay_policy_matches_v2(
        &self,
        limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
    ) -> bool {
        self.limits == limits
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_face_count_cap_v2(&self) -> usize {
        self.limits.max_faces
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_hinge_count_cap_v2(&self) -> usize {
        self.limits.max_hinges
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_pose_pair_deep_retained_bytes_cap_v2(&self) -> usize {
        self.limits.max_pose_pair_deep_retained_bytes
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_logical_work_v2(&self) -> usize {
        self.limits.max_logical_work
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_workspace_bytes_cap_v2(&self) -> usize {
        self.limits.max_workspace_bytes
    }

    #[doc(hidden)]
    /// Peak retained bytes for a successful revalidation phase, excluding
    /// borrowed geometry and audit storage owned by the caller.
    #[must_use]
    pub fn checked_revalidation_peak_bytes_upper_bound_v2(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.limits.max_pose_pair_deep_retained_bytes)?
            .checked_add(
                self.limits
                    .max_workspace_bytes
                    .max(std::mem::size_of::<Self>()),
            )
    }
}

impl MaterialHingeGraphGeometry {
    pub fn checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        lower_pose: &ClosedMaterialHingeGraphPose,
        upper_pose: &ClosedMaterialHingeGraphPose,
    ) -> Result<
        CanonicalBinary64PosePairTransformRealizationResourceBoundV2,
        CanonicalBinary64PosePairTransformRealizationErrorV2,
    > {
        self.checked_canonical_binary64_pose_pair_transform_realization_resource_bound_with_checkpoint_v2(
            audit,
            lower_pose,
            upper_pose,
            || Ok(()),
        )
    }

    pub fn checked_canonical_binary64_pose_pair_transform_realization_resource_bound_with_checkpoint_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        lower_pose: &ClosedMaterialHingeGraphPose,
        upper_pose: &ClosedMaterialHingeGraphPose,
        mut checkpoint: impl FnMut() -> Result<(), CanonicalBinary64PosePairTransformRealizationStopV2>,
    ) -> Result<
        CanonicalBinary64PosePairTransformRealizationResourceBoundV2,
        CanonicalBinary64PosePairTransformRealizationErrorV2,
    > {
        resources::checked_resource_bound_v2(self, audit, lower_pose, upper_pose, &mut checkpoint)
    }
}

pub fn prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(
    input: CanonicalBinary64PosePairTransformRealizationInputV2<'_>,
) -> Result<
    CanonicalBinary64PosePairTransformRealizationEvidenceV2,
    CanonicalBinary64PosePairTransformRealizationErrorV2,
> {
    prove_canonical_binary64_pose_pair_transform_realization_evidence_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

pub fn prove_canonical_binary64_pose_pair_transform_realization_evidence_with_checkpoint_v2(
    input: CanonicalBinary64PosePairTransformRealizationInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CanonicalBinary64PosePairTransformRealizationStopV2>,
) -> Result<
    CanonicalBinary64PosePairTransformRealizationEvidenceV2,
    CanonicalBinary64PosePairTransformRealizationErrorV2,
> {
    validation::issue_v2(input, &mut checkpoint)
}

#[cfg(test)]
#[path = "canonical_binary64_pose_pair_transform_realization_v2/tests.rs"]
mod tests;
