use sha2::{Digest, Sha256};

use super::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2;
type LimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2;
type BoundaryV2 = CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
type PoseIdentityV2 = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityEvidenceV2;

pub(super) fn binding_fingerprint_v2(
    boundary: &BoundaryV2,
    pose_identity: &PoseIdentityV2,
    resources: Phase3JResourcesV2,
    limits: LimitsV2,
) -> Result<[u8; 32], ErrorV2> {
    let mut hash = Sha256::new();
    hash.update(
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
            .as_bytes(),
    );
    hash.update(b"retained-phase3i-and-k-evidence-instance-authority-outside-fingerprint:scheduled-angle-representation-points-only:no-closure-transform-pose-realization");
    hash.update(boundary.binding_fingerprint_internal_v2());
    hash.update(pose_identity.binding_fingerprint_v2());
    hash.update(boundary.schedule_binding_fingerprint_internal_v2());
    hash.update(boundary.graph_binding_fingerprint_internal_v2());
    hash.update(boundary.closed_boundary_binding_fingerprint_internal_v2());
    hash.update(pose_identity.fixed_face_v2().canonical_bytes());
    let schedule_limits = boundary.schedule_limits_internal_v2();
    for value in [
        boundary.actual_block_count_v2(),
        boundary.hinge_count_v2(),
        boundary.closed_dyadic_boundary_configuration_count_v2(),
        pose_identity.representation_boundary_pose_angle_identity_count_v2(),
        schedule_limits.max_hinges,
        schedule_limits.max_degree,
        schedule_limits.max_work,
        resources.retained_boundary_configuration_prerequisite_bytes,
        resources.retained_pose_angle_identity_evidence_bytes,
        resources.schedule_deep_retained_bytes_cap,
        resources.representation_boundary_poses_deep_retained_bytes_cap,
        resources.pose_angle_identity_logical_work,
        resources.pose_angle_identity_workspace_bytes,
        resources.delegated_boundary_configuration_replay_peak_bytes,
        resources.composition_workspace_bytes,
        resources.publication_bytes,
        resources.aggregate_peak_bytes,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    hash.update(schedule_limits.max_coefficient_bits.to_le_bytes());
    for value in resources::limit_values_v2(limits) {
        update_usize_v2(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

fn update_usize_v2(hash: &mut Sha256, value: usize) -> Result<(), ErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| ErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
