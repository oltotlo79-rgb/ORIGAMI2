//! Phase 3J join between scheduled-angle positive thickness and pose-angle identity.
//!
//! This promotion binds the Phase 3I proof for both representation-boundary
//! scheduled-angle configurations to the kinematics proof that two retained
//! pose objects carry those boundary angle bits. It does not authenticate the
//! pose transforms, strengthen tolerance-based closure, or make either
//! representation boundary an application, source/target, or direction tag.

use ori_kinematics::{
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2,
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    ClosedMaterialHingeGraphPose, CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2,
    CycleScheduleRepresentationBoundaryPoseAngleIdentityStopV2, MaterialHingeGraphAudit,
};
use thiserror::Error;

use super::{
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2,
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2,
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
};

pub const COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness_prerequisite_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
pub(crate) const COMPOSITION_WORKSPACE_BYTES_V2: usize = 512;

#[path = "closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness/binding.rs"]
mod binding;
#[path = "closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness/resources.rs"]
mod resources;
#[path = "closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness/validation.rs"]
mod validation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2
{
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2
{
    #[error("the Phase 3J pose-angle identity join exceeds its finite resource envelope")]
    ResourceLimit,
    #[error("the retained Phase 3I prerequisite does not replay: {0}")]
    BoundaryConfigurationPositiveThickness(
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
    ),
    #[error("the retained pose-angle identity evidence does not replay: {0}")]
    PoseAngleIdentity(CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2),
    #[error("the Phase 3J pose-angle identity join does not match the live replay")]
    CertificateBindingMismatch,
    #[error("the Phase 3J pose-angle identity join was cancelled")]
    Cancelled,
    #[error("the Phase 3J pose-angle identity join deadline elapsed")]
    DeadlineExceeded,
}

/// Replay-bound outer policy for Phase 3I replay, the two live pose objects, and K replay.
///
/// The seven count/retained/publication/aggregate `max_*` fields are upper caps:
/// issuance may retain genuine slack, but replay must reproduce each cap
/// exactly. K logical work and workspace are exact identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2
{
    pub max_blocks: usize,
    pub max_hinges: usize,
    pub max_schedule_deep_retained_bytes: usize,
    pub max_representation_boundary_poses_deep_retained_bytes: usize,
    pub max_pose_angle_identity_logical_work: usize,
    pub max_pose_angle_identity_workspace_bytes: usize,
    pub max_retained_boundary_configuration_prerequisite_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

/// Consuming proof-to-proof promotion input. Both proofs remain opaque.
pub struct CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteInputV2
{
    pub boundary_configuration_prerequisite:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    pub pose_angle_identity_evidence:
        CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
    pub limits:
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2,
}

/// Complete live replay input. Geometry, schedule and schedule policy are the
/// exact tuple nested in `boundary_configuration_replay`.
pub struct CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteRevalidationInputV2<
    'a,
> {
    pub boundary_configuration_replay:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<'a>,
    pub audit: &'a MaterialHingeGraphAudit,
    pub lower_pose: &'a ClosedMaterialHingeGraphPose,
    pub upper_pose: &'a ClosedMaterialHingeGraphPose,
    pub limits:
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Phase3JResourcesV2 {
    retained_boundary_configuration_prerequisite_bytes: usize,
    retained_pose_angle_identity_evidence_bytes: usize,
    schedule_deep_retained_bytes_cap: usize,
    representation_boundary_poses_deep_retained_bytes_cap: usize,
    pose_angle_identity_logical_work: usize,
    pose_angle_identity_workspace_bytes: usize,
    delegated_boundary_configuration_replay_peak_bytes: usize,
    composition_workspace_bytes: usize,
    publication_bytes: usize,
    aggregate_peak_bytes: usize,
}

/// Opaque proof that Phase 3I covers the same two scheduled-angle
/// representation points whose angle bits are carried by two retained pose
/// objects.
///
/// This does not prove exact closure, transform realization, pose realization,
/// application parameter identity, source/target identity, direction, layer
/// order, continuous motion, collision clearance, layer transport, or any
/// mutation capability. Strict closure requires a separate future proof.
///
/// It deliberately implements neither `Clone`, serde, `Deref`, raw nested
/// evidence access, nor downgrade conversions.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
/// fn require_deref<T: std::ops::Deref>() {}
/// require_deref::<CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
/// fn fabricate() -> CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2 {
///     CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2 {}
/// }
/// ```
pub struct CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2
{
    boundary_configuration_prerequisite:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    pose_angle_identity_evidence:
        CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2,
    resources: Phase3JResourcesV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug
    for CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2")
            .field("model", &self.model_id_v2())
            .field("actual_block_count", &self.actual_block_count_v2())
            .field("hinge_count", &self.hinge_count_v2())
            .field("scheduled_angle_representation_point_count", &2usize)
            .field("publication_bytes", &self.publication_bytes_v2())
            .field("aggregate_peak_bytes", &self.aggregate_peak_bytes_upper_bound_v2())
            .finish_non_exhaustive()
    }
}

impl CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2
{
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.boundary_configuration_prerequisite.actual_block_count_v2()
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.pose_angle_identity_evidence.hinge_count_v2()
    }

    #[must_use]
    pub const fn scheduled_angle_representation_point_count_v2(&self) -> usize { 2 }

    #[must_use]
    pub const fn both_scheduled_angle_representation_points_have_positive_thickness_v2(
        &self,
    ) -> bool {
        self.boundary_configuration_prerequisite
            .both_closed_dyadic_boundary_configurations_have_positive_thickness_v2()
            && self.pose_angle_identity_evidence
                .representation_boundary_pose_angle_identity_count_v2()
                == 2
    }

    #[must_use]
    pub fn matches_pose_instances_v2(
        &self,
        lower: &ClosedMaterialHingeGraphPose,
        upper: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        self.pose_angle_identity_evidence
            .matches_pose_instances_v2(lower, upper)
    }

    #[must_use]
    pub const fn retained_boundary_configuration_prerequisite_bytes_v2(&self) -> usize {
        self.resources.retained_boundary_configuration_prerequisite_bytes
    }

    #[must_use]
    pub const fn publication_bytes_v2(&self) -> usize { self.resources.publication_bytes }

    #[must_use]
    pub const fn aggregate_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.aggregate_peak_bytes
    }

    pub(super) const fn replay_aggregate_peak_cap_internal_v2(&self) -> usize {
        self.limits.max_aggregate_peak_bytes
    }

    pub(super) const fn block_count_cap_internal_v2(&self) -> usize {
        self.limits.max_blocks
    }

    pub(super) const fn hinge_count_cap_internal_v2(&self) -> usize {
        self.limits.max_hinges
    }

    pub(super) const fn pose_pair_deep_retained_bytes_cap_internal_v2(&self) -> usize {
        self.limits
            .max_representation_boundary_poses_deep_retained_bytes
    }

    pub(super) const fn binding_fingerprint_internal_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2>,
    ) -> Result<(), CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2> {
        validation::revalidate_v2(self, input, &mut checkpoint)
    }

    #[must_use] pub const fn authorizes_source_target_identity(&self) -> bool { false }
    #[must_use] pub const fn authorizes_current_requested_identity(&self) -> bool { false }
    #[must_use] pub const fn authorizes_application_parameter_identity(&self) -> bool { false }
    #[must_use] pub const fn authorizes_direction(&self) -> bool { false }
    #[must_use] pub const fn authorizes_layer_order(&self) -> bool { false }
    #[must_use] pub const fn authorizes_exact_closure(&self) -> bool { false }
    #[must_use] pub const fn authorizes_transform_realization(&self) -> bool { false }
    #[must_use] pub const fn authorizes_pose_realization(&self) -> bool { false }
    #[must_use] pub const fn authorizes_continuous_motion(&self) -> bool { false }
    #[must_use] pub const fn authorizes_collision_clearance(&self) -> bool { false }
    #[must_use] pub const fn authorizes_layer_transport(&self) -> bool { false }
    #[must_use] pub const fn authorizes_project_mutation(&self) -> bool { false }
    #[must_use] pub const fn authorizes_apply(&self) -> bool { false }
    #[must_use] pub const fn authorizes_viewer(&self) -> bool { false }
    #[must_use] pub const fn authorizes_export(&self) -> bool { false }
}

pub fn prove_common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness_prerequisite_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteInputV2,
) -> Result<CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2, CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2>{
    prove_common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness_prerequisite_with_checkpoint_v2(input, || Ok(()))
}

pub fn prove_common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness_prerequisite_with_checkpoint_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteInputV2,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2>,
) -> Result<CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2, CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2>{
    validation::issue_v2(input, &mut checkpoint)
}

#[cfg(test)]
#[path = "closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness/tests.rs"]
mod tests;
