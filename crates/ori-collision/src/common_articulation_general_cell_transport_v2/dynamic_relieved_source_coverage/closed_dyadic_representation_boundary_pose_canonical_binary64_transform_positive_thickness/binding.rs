use sha2::{Digest, Sha256};

use super::*;

type ErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteErrorV2;
type LimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteLimitsV2;

pub(super) fn binding_fingerprint_v2(
    phase3j: &CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2,
    transform: &CanonicalBinary64PosePairTransformRealizationEvidenceV2,
    resources: Phase3KResourcesV2,
    limits: LimitsV2,
) -> Result<[u8; 32], ErrorV2> {
    let mut hash = Sha256::new();
    hash.update(
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_REPRESENTATION_BOUNDARY_POSE_CANONICAL_BINARY64_TRANSFORM_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
            .as_bytes(),
    );
    hash.update(phase3j.binding_fingerprint_internal_v2());
    hash.update(transform.binding_fingerprint_v2());
    for value in [
        phase3j.actual_block_count_v2(),
        transform.face_count_v2(),
        transform.hinge_count_v2(),
        resources.retained_phase3j_prerequisite_bytes,
        resources.retained_transform_realization_evidence_bytes,
        resources.pose_pair_deep_retained_bytes_cap,
        resources.canonical_transform_logical_work,
        resources.canonical_transform_workspace_bytes,
        resources.delegated_phase3j_replay_peak_bytes,
        resources.composition_workspace_bytes,
        resources.publication_bytes,
        resources.aggregate_peak_bytes,
    ]
    .into_iter()
    .chain(resources::limit_values_v2(limits))
    {
        let value = u64::try_from(value).map_err(|_| ErrorV2::ResourceLimit)?;
        hash.update(value.to_be_bytes());
    }
    Ok(hash.finalize().into())
}
