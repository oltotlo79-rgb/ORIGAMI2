use super::*;

const DOMAIN_SEPARATOR_V2: &[u8] =
    b"ORIGAMI2_CANONICAL_CYCLE_SCHEDULE_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_EVIDENCE_V2";
const REPRESENTATION_MAPPING_V2: &[u8] =
    b"semantic-angle-bits-binding-instance-arc-retained-outside-fingerprint-no-closure-transform-realization:ordinary-normalized-x-minus-plus-one-or-half-angle-public-f64-affine-operation-order-point-bits-inside-exact-u-domain-endpoint-outward-box";

#[allow(clippy::too_many_arguments)]
pub(super) fn binding_fingerprint_v2(
    representation: BoundaryRepresentationV2,
    fixed_face: FaceId,
    schedule_binding: [u8; 32],
    graph_binding: [u8; 32],
    closed_boundary_binding: [u8; 32],
    schedule_limits: CycleScheduleLimitsV1,
    resources: RepresentationBoundaryPoseAngleIdentityResourcesV2,
    limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
) -> Result<[u8; 32], CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    let mut hash = Sha256::new();
    for value in [
        DOMAIN_SEPARATOR_V2,
        CANONICAL_CYCLE_SCHEDULE_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_EVIDENCE_MODEL_ID_V2
            .as_bytes(),
        CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2.as_bytes(),
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes(),
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
        REPRESENTATION_MAPPING_V2,
        &[representation.tag_v2()],
        &fixed_face.canonical_bytes(),
        &schedule_binding,
        &graph_binding,
        &closed_boundary_binding,
    ] {
        update_frame_v2(&mut hash, value)?;
    }
    for value in [
        REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_COUNT_V2,
        resources.hinge_count,
        schedule_limits.max_hinges,
        schedule_limits.max_degree,
        schedule_limits.max_work,
        resources.schedule_deep_retained_bytes_cap,
        resources.representation_boundary_poses_deep_retained_bytes,
        resources.logical_work,
        resources.workspace_peak_bytes,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    update_frame_v2(
        &mut hash,
        &schedule_limits.max_coefficient_bits.to_be_bytes(),
    )?;
    for value in pose_resources::limit_values_v2(limits) {
        update_usize_v2(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

pub(super) fn checked_binding_work_v2() -> usize {
    let fixed_lengths = [
        DOMAIN_SEPARATOR_V2.len(),
        CANONICAL_CYCLE_SCHEDULE_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_EVIDENCE_MODEL_ID_V2
            .len(),
        CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2.len(),
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.len(),
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.len(),
        REPRESENTATION_MAPPING_V2.len(),
        1,
        16,
        32,
        32,
        32,
        4,
    ];
    fixed_lengths
        .into_iter()
        .chain(std::iter::repeat_n(8, 14))
        .map(|length| 8 + length)
        .sum()
}

fn update_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    update_frame_v2(
        hash,
        &u64::try_from(value)
            .map_err(|_| {
                CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit
            })?
            .to_be_bytes(),
    )
}

fn update_frame_v2(
    hash: &mut Sha256,
    value: &[u8],
) -> Result<(), CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2> {
    let length = u64::try_from(value.len())
        .map_err(|_| CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ResourceLimit)?;
    hash.update(length.to_be_bytes());
    hash.update(value);
    Ok(())
}
