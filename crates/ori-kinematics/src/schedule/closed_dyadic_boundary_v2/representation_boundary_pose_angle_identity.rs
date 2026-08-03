//! Representation-boundary pose-object identity and angle-bit membership.
//!
//! This seam does not strengthen the pose object's existing tolerance-based
//! closure observation and does not authenticate its transforms as an exact
//! realization of the scheduled angles.

use std::sync::Arc;

use crate::{
    ClosedMaterialHingeGraphPose, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    MaterialHingeGraphInstanceV1,
};

use super::*;

#[path = "representation_boundary_pose_angle_identity/authority.rs"]
mod authority;
#[path = "representation_boundary_pose_angle_identity/binding.rs"]
mod binding;
#[path = "representation_boundary_pose_angle_identity/evaluate.rs"]
mod evaluate_pose;
#[path = "representation_boundary_pose_angle_identity/resources.rs"]
mod pose_resources;

pub const CANONICAL_CYCLE_SCHEDULE_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_EVIDENCE_MODEL_ID_V2: &str =
    "canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2";

const REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_COUNT_V2: usize = 2;

/// Cooperative stop requested while joining boundary angle bits to pose objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Failure while joining lower/upper boundary angle bits to pose objects.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2 {
    #[error("representation-boundary pose-angle identity evidence exceeds its resource limits")]
    ResourceLimit,
    #[error("the canonical schedule does not match the live graph and fixed face")]
    ScheduleBindingMismatch,
    #[error("the sealed closed-boundary evidence does not match the live schedule")]
    ClosedBoundaryEvidenceMismatch,
    #[error("a pose object's angle bits do not match its canonical representation boundary")]
    BoundaryPoseMismatch,
    #[error("representation-boundary pose-angle identity evidence does not match the live replay")]
    CertificateBindingMismatch,
    #[error("representation-boundary pose-angle identity evaluation was cancelled")]
    Cancelled,
    #[error("representation-boundary pose-angle identity evaluation deadline elapsed")]
    DeadlineExceeded,
}

/// Exact policy retained by representation-boundary pose-angle identity evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
    /// Upper bound for the current representation's hinge count.
    pub max_hinges: usize,
    /// Upper bound for the current schedule's deep retained bytes.
    pub max_schedule_deep_retained_bytes: usize,
    /// Upper bound for both current pose objects' deep retained bytes.
    pub max_representation_boundary_poses_deep_retained_bytes: usize,
    /// Exact logical-work policy identity, not a freely widenable ceiling.
    pub max_logical_work: usize,
    /// Exact workspace policy identity, not a freely widenable ceiling.
    pub max_workspace_bytes: usize,
}

/// Advisory resource inventory for one current schedule and two current poses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2 {
    pub(super) closed_boundary_bound: CycleScheduleClosedDyadicBoundaryResourceBoundV2,
    pub(super) representation_boundary_poses_deep_retained_bytes: usize,
    pub(super) pose_retained_scan_work: usize,
    pub(super) graph_binding_work: usize,
    pub(super) logical_work_required: usize,
    pub(super) workspace_peak_bytes: usize,
}

impl CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2 {
    #[must_use]
    pub const fn hinge_count_v2(self) -> usize {
        self.closed_boundary_bound.hinge_count_v2()
    }

    #[must_use]
    pub const fn schedule_deep_retained_bytes_v2(self) -> usize {
        self.closed_boundary_bound.schedule_deep_retained_bytes_v2()
    }

    #[must_use]
    pub const fn representation_boundary_poses_deep_retained_bytes_v2(self) -> usize {
        self.representation_boundary_poses_deep_retained_bytes
    }

    #[must_use]
    pub const fn logical_work_required_v2(self) -> usize {
        self.logical_work_required
    }

    #[must_use]
    pub const fn workspace_peak_bytes_upper_bound_v2(self) -> usize {
        self.workspace_peak_bytes
    }
}

/// Complete live input for issuing or replaying pose-object and angle-bit identity.
#[derive(Clone, Copy)]
pub struct CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closed_boundary_evidence: &'a CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
    pub lower_pose: &'a ClosedMaterialHingeGraphPose,
    pub upper_pose: &'a ClosedMaterialHingeGraphPose,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentationBoundaryPoseAngleIdentityResourcesV2 {
    pub(super) hinge_count: usize,
    pub(super) schedule_deep_retained_bytes_cap: usize,
    pub(super) representation_boundary_poses_deep_retained_bytes: usize,
    pub(super) logical_work: usize,
    pub(super) workspace_peak_bytes: usize,
}

/// Opaque evidence that two specifically retained pose-object instances carry the angle bits
/// selected at one canonical schedule's lower and upper representation
/// boundaries.
///
/// Ordinary schedules select literal normalized `x = -1/+1`. Half-angle
/// schedules preserve the existing public point-evaluation operation order and
/// additionally require each point to lie in the exact-rational outward box
/// already sealed by closed-boundary evidence.
///
/// The evidence neither strengthens the pose type's caller-selected,
/// tolerance-based closure observation nor authenticates its transforms or an
/// exact pose realization. Strict closure requires a separate future proof.
/// Its deterministic fingerprint also omits pose-object `Arc` identity;
/// instance authority exists only in the retained evidence and its pointer-
/// identity/revalidation methods.
/// This is also not source/target, current/requested, direction, layer order,
/// continuous motion, collision clearance, or mutation authority. It
/// deliberately implements neither `Clone`, serde, `Deref`, nor raw pose or
/// closed-boundary evidence access.
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;
/// fn require_deserialize<T: serde::de::DeserializeOwned>() {}
/// require_deserialize::<CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;
/// fn require_deref<T: std::ops::Deref>() {}
/// require_deref::<CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;
/// fn fabricate() -> CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2 {
///     CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2 {}
/// }
/// ```
pub struct CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2 {
    pub(super) issuer_geometry: MaterialHingeGraphInstanceV1,
    pub(super) lower_pose_instance: Arc<()>,
    pub(super) upper_pose_instance: Arc<()>,
    pub(super) fixed_face: FaceId,
    pub(super) schedule_binding_fingerprint: [u8; 32],
    pub(super) graph_binding_fingerprint: [u8; 32],
    pub(super) closed_boundary_evidence_binding_fingerprint: [u8; 32],
    pub(super) schedule_limits: CycleScheduleLimitsV1,
    pub(super) resources: RepresentationBoundaryPoseAngleIdentityResourcesV2,
    pub(super) limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    pub(super) binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug for CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2")
            .field("model", &self.model_id_v2())
            .field("hinge_count", &self.hinge_count_v2())
            .field("fixed_face", &self.fixed_face)
            .field(
                "representation_boundary_pose_angle_identity_count",
                &self.representation_boundary_pose_angle_identity_count_v2(),
            )
            .field("logical_work", &self.resources.logical_work)
            .field("workspace_peak_bytes", &self.resources.workspace_peak_bytes)
            .finish_non_exhaustive()
    }
}

impl CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        CANONICAL_CYCLE_SCHEDULE_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_EVIDENCE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn representation_boundary_pose_angle_identity_count_v2(&self) -> usize {
        REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_COUNT_V2
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.resources.hinge_count
    }

    #[must_use]
    pub const fn fixed_face_v2(&self) -> FaceId {
        self.fixed_face
    }

    /// Returns the retained schedule-byte policy cap. A semantic-equal replay
    /// schedule may use a different allocator capacity at or below this cap.
    #[must_use]
    pub const fn schedule_deep_retained_bytes_upper_bound_v2(&self) -> usize {
        self.resources.schedule_deep_retained_bytes_cap
    }

    #[must_use]
    pub const fn representation_boundary_poses_deep_retained_bytes_upper_bound_v2(&self) -> usize {
        self.resources
            .representation_boundary_poses_deep_retained_bytes
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.resources.logical_work
    }

    #[must_use]
    pub const fn workspace_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.workspace_peak_bytes
    }

    pub fn revalidate_v2(
        &self,
        input: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'_>,
    ) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
        >,
    ) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
        evaluate_pose::revalidate_v2(self, input, &mut checkpoint)
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn schedule_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.schedule_binding_fingerprint
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn graph_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.graph_binding_fingerprint
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn closed_boundary_evidence_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.closed_boundary_evidence_binding_fingerprint
    }

    /// Checks the complete retained replay policy. This is intended for
    /// proof-to-proof composition; it issues no authority by itself.
    #[doc(hidden)]
    #[must_use]
    pub fn replay_policy_matches_v2(
        &self,
        schedule_limits: CycleScheduleLimitsV1,
        limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    ) -> bool {
        self.schedule_limits == schedule_limits && self.limits == limits
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_schedule_deep_retained_bytes_cap_v2(&self) -> usize {
        self.limits.max_schedule_deep_retained_bytes
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_hinge_count_cap_v2(&self) -> usize {
        self.limits.max_hinges
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn replay_representation_boundary_poses_deep_retained_bytes_cap_v2(&self) -> usize {
        self.limits
            .max_representation_boundary_poses_deep_retained_bytes
    }

    #[doc(hidden)]
    #[must_use]
    pub fn matches_geometry_instance_v2(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.issuer_geometry.matches(geometry)
    }

    /// Compares opaque geometry issuer identity for proof-to-proof promotion.
    #[doc(hidden)]
    #[must_use]
    pub fn matches_geometry_instance_anchor_v2(
        &self,
        issuer: &MaterialHingeGraphInstanceV1,
    ) -> bool {
        &self.issuer_geometry == issuer
    }

    #[doc(hidden)]
    #[must_use]
    pub fn matches_pose_instances_v2(
        &self,
        lower: &ClosedMaterialHingeGraphPose,
        upper: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        Arc::ptr_eq(&self.lower_pose_instance, &lower.instance_anchor_v2())
            && Arc::ptr_eq(&self.upper_pose_instance, &upper.instance_anchor_v2())
    }

    /// Resolves only a lower/upper representation-boundary tag to the exact
    /// pose instance retained by this evidence. No parameter or direction
    /// semantics are implied.
    #[must_use]
    pub fn matches_representation_boundary_pose_angle_identity_instance_v2(
        &self,
        upper: bool,
        pose: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        if upper {
            Arc::ptr_eq(&self.upper_pose_instance, &pose.instance_anchor_v2())
        } else {
            Arc::ptr_eq(&self.lower_pose_instance, &pose.instance_anchor_v2())
        }
    }

    /// Deterministic semantic binding for the schedule and policies. It does
    /// not directly hash pose angle bits or `Arc` object identity, and equal-
    /// valued fresh pose objects can therefore produce the same fingerprint.
    /// Never use this fingerprint alone as instance authority; retain this
    /// evidence and call `matches_pose_instances_v2` or `revalidate_v2`.
    #[doc(hidden)]
    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }
}

impl CanonicalCycleScheduleV1 {
    /// Inventories one current schedule and two pose objects. The
    /// non-checkpointed convenience form may perform work up to the submitted
    /// schedule policy; cooperative callers should prefer the checkpointed
    /// form below.
    pub fn checked_representation_boundary_pose_angle_identity_resource_bound_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        lower_pose: &ClosedMaterialHingeGraphPose,
        upper_pose: &ClosedMaterialHingeGraphPose,
        schedule_limits: CycleScheduleLimitsV1,
    ) -> Result<
        CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
    > {
        self.checked_representation_boundary_pose_angle_identity_resource_bound_with_checkpoint_v2(
            geometry,
            audit,
            lower_pose,
            upper_pose,
            schedule_limits,
            || Ok(()),
        )
    }

    pub fn checked_representation_boundary_pose_angle_identity_resource_bound_with_checkpoint_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        lower_pose: &ClosedMaterialHingeGraphPose,
        upper_pose: &ClosedMaterialHingeGraphPose,
        schedule_limits: CycleScheduleLimitsV1,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2,
        >,
    ) -> Result<
        CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
    > {
        pose_resources::checked_resource_bound_v2(
            self,
            geometry,
            audit,
            lower_pose,
            upper_pose,
            schedule_limits,
            &mut checkpoint,
        )
    }
}

pub fn prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
    input: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'_>,
) -> Result<
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
    CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
> {
    prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

pub fn prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_with_checkpoint_v2(
    input: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'_>,
    mut checkpoint: impl FnMut()
        -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2>,
) -> Result<
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
    CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
> {
    evaluate_pose::issue_v2(input, &mut checkpoint)
}

#[cfg(test)]
#[path = "representation_boundary_pose_angle_identity/tests.rs"]
mod tests;
