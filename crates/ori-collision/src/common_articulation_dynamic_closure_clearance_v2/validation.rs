//! Validation, resource preflight, nested bridge replay, and binding.

use std::mem::size_of;

use sha2::{Digest, Sha256};

use super::*;
mod pair_registry;
use pair_registry::{
    canonical_pair_budget_v2, enumerate_and_copy_canonical_pairs_v2, raw_pair_candidate_budget_v2,
    validate_submitted_pair_order_v2,
};

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const CANONICAL_MIURA_FACES_PER_BLOCK_V2: usize = 9;
const CANONICAL_MIURA_HINGES_PER_BLOCK_V2: usize = 12;

pub(super) struct ValidatedInputV2 {
    pub(super) profile_binding: [u8; 32],
    pub(super) decomposition_binding: [u8; 32],
    pub(super) common_pose_binding: [u8; 32],
    pub(super) audit_binding: [u8; 32],
    pub(super) parent_schedule_binding: [u8; 32],
    pub(super) bridge_binding: [u8; 32],
    pub(super) parent_fixed_face: FaceId,
    pub(super) paper_thickness_bits: u64,
    pub(super) closure_tolerance_bits: u64,
    pub(super) actual_block_count: usize,
    pub(super) actual_face_count: usize,
    pub(super) cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV2>,
    pub(super) pair_registry_retained_bytes: usize,
    pub(super) pair_registry_temporary_bytes: usize,
    pub(super) publication_bytes: usize,
    pub(super) aggregate_peak_bytes: usize,
    pub(super) limits: CommonArticulationDynamicClosureClearanceLimitsV2,
}

struct PreflightResourcesV2 {
    actual_block_count: usize,
    actual_face_count: usize,
    raw_pair_candidates: usize,
    canonical_pair_count: usize,
    pair_registry_retained_bytes: usize,
    pair_registry_temporary_bytes: usize,
    publication_bytes: usize,
    aggregate_peak_bytes: usize,
}

pub(super) fn validate_input_v2(
    input: &CommonArticulationDynamicClosureClearanceInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<ValidatedInputV2, CommonArticulationDynamicClosureClearanceErrorV2> {
    validate_limits_v2(input.limits)?;
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.closure_tolerance.is_finite()
        || input.closure_tolerance < 0.0
    {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::InvalidInput);
    }
    let preflight = preflight_resources_v2(input)?;
    validate_submitted_pair_order_v2(input.submitted_cross_block_pairs, checkpoint)?;
    revalidate_dynamic_bridge_v2(input, checkpoint)?;
    let cross_block_pairs = enumerate_and_copy_canonical_pairs_v2(
        input.decomposition,
        preflight.raw_pair_candidates,
        preflight.canonical_pair_count,
        input.submitted_cross_block_pairs,
        checkpoint,
    )?;
    let audit_binding = audit_binding_fingerprint_v2(input.audit, checkpoint)?;
    Ok(ValidatedInputV2 {
        profile_binding: input.profile.binding_fingerprint_v2(),
        decomposition_binding: input.decomposition.binding_fingerprint_v2(),
        common_pose_binding: input.common_pose.binding_fingerprint_v2(),
        audit_binding,
        parent_schedule_binding: input.parent_schedule.certificate_binding_fingerprint_v2(),
        bridge_binding: input.dynamic_closure_bridge.binding_fingerprint_v2(),
        parent_fixed_face: input.parent_fixed_face,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        actual_block_count: preflight.actual_block_count,
        actual_face_count: preflight.actual_face_count,
        cross_block_pairs,
        pair_registry_retained_bytes: preflight.pair_registry_retained_bytes,
        pair_registry_temporary_bytes: preflight.pair_registry_temporary_bytes,
        publication_bytes: preflight.publication_bytes,
        aggregate_peak_bytes: preflight.aggregate_peak_bytes,
        limits: input.limits,
    })
}

fn validate_limits_v2(
    limits: CommonArticulationDynamicClosureClearanceLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    if [
        limits.max_blocks,
        limits.max_faces,
        limits.max_cross_block_pairs,
        limits.max_pair_registry_retained_bytes,
        limits.max_pair_registry_temporary_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
    .iter()
    .any(|value| *value == 0 || *value == usize::MAX)
    {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }
    Ok(())
}

fn preflight_resources_v2(
    input: &CommonArticulationDynamicClosureClearanceInputV2<'_>,
) -> Result<PreflightResourcesV2, CommonArticulationDynamicClosureClearanceErrorV2> {
    let declared_aggregate_peak = input
        .dynamic_closure_bridge
        .revalidation_peak_bytes_upper_bound_v2()
        .checked_add(input.limits.max_pair_registry_retained_bytes)
        .and_then(|value| value.checked_add(input.limits.max_pair_registry_temporary_bytes))
        .and_then(|value| value.checked_add(input.limits.max_publication_bytes))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    if declared_aggregate_peak > input.limits.max_aggregate_peak_bytes {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }
    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let raw_pair_candidates = raw_pair_candidate_budget_v2(actual_block_count)?;
    let canonical_pair_count = canonical_pair_budget_v2(actual_block_count)?;
    // The global parent has one shared articulation face plus eight new
    // faces per local 3x3 block.  Nine is only the local block face count.
    let face_count = actual_block_count
        .checked_mul(CANONICAL_MIURA_FACES_PER_BLOCK_V2 - 1)
        .and_then(|value| value.checked_add(1))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let hinge_count = actual_block_count
        .checked_mul(CANONICAL_MIURA_HINGES_PER_BLOCK_V2)
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || actual_block_count > input.limits.max_blocks
        || face_count > input.limits.max_faces
        || actual_block_count != input.dynamic_closure_bridge.actual_block_count_v2()
        || actual.block_count_v2() != actual_block_count
        || actual.face_count_v2() != face_count
        || actual.hinge_count_v2() != hinge_count
        || actual.raw_cross_block_pair_candidates_v2() != raw_pair_candidates
        || actual.canonical_cross_block_pairs_v2() != canonical_pair_count
        || input.geometry.face_ids().len() != face_count
        || input.geometry.hinges().len() != hinge_count
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.common_pose.actual_block_count_v2() != actual_block_count
        || input.common_pose.configured_max_blocks_v2() != configured_max_blocks
        || actual_block_count > maximum.block_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
    {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }
    if input.submitted_cross_block_pairs.len() != canonical_pair_count {
        return Err(
            CommonArticulationDynamicClosureClearanceErrorV2::CrossBlockPairCoverageMismatch {
                expected: canonical_pair_count,
                actual: input.submitted_cross_block_pairs.len(),
            },
        );
    }
    if canonical_pair_count > input.limits.max_cross_block_pairs {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }

    let pair_bytes = size_of::<CommonArticulationCrossBlockFacePairV2>();
    let raw_bytes = raw_pair_candidates
        .checked_mul(pair_bytes)
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let retained_bytes = canonical_pair_count
        .checked_mul(pair_bytes)
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let local_pair_count = actual_block_count
        .checked_mul(
            CANONICAL_MIURA_FACES_PER_BLOCK_V2
                .checked_mul(CANONICAL_MIURA_FACES_PER_BLOCK_V2 - 1)
                .and_then(|value| value.checked_div(2))
                .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?,
        )
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let local_bytes = local_pair_count
        .checked_mul(pair_bytes)
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let temporary_bytes = raw_bytes
        .checked_add(local_bytes.max(retained_bytes))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let publication_bytes = size_of::<CommonArticulationDynamicClosureClearancePrerequisiteV2>()
        .checked_add(size_of::<CommonArticulationDynamicClosureClearanceOutcomeV2>())
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let aggregate_peak_bytes = input
        .dynamic_closure_bridge
        .revalidation_peak_bytes_upper_bound_v2()
        .checked_add(retained_bytes)
        .and_then(|value| value.checked_add(temporary_bytes))
        .and_then(|value| value.checked_add(publication_bytes))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    if retained_bytes > input.limits.max_pair_registry_retained_bytes
        || temporary_bytes > input.limits.max_pair_registry_temporary_bytes
        || publication_bytes > input.limits.max_publication_bytes
        || aggregate_peak_bytes > input.limits.max_aggregate_peak_bytes
    {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }
    Ok(PreflightResourcesV2 {
        actual_block_count,
        actual_face_count: face_count,
        raw_pair_candidates,
        canonical_pair_count,
        pair_registry_retained_bytes: retained_bytes,
        pair_registry_temporary_bytes: temporary_bytes,
        publication_bytes,
        aggregate_peak_bytes,
    })
}

fn revalidate_dynamic_bridge_v2(
    input: &CommonArticulationDynamicClosureClearanceInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    let mut requested_stop = None;
    let result = input.dynamic_closure_bridge.revalidate_with_checkpoint_v2(
        CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
            geometry: input.geometry,
            audit: input.audit,
            pose: input.pose,
            parent_fixed_face: input.parent_fixed_face,
            parent_schedule: input.parent_schedule,
            decomposition: input.decomposition,
            common_pose: input.common_pose,
            paper_thickness_mm: input.paper_thickness_mm,
            closure_tolerance: input.closure_tolerance,
            profile: input.profile,
        },
        || match checkpoint_v2(checkpoint) {
            Ok(()) => Ok(()),
            Err(error) => {
                requested_stop = Some(error);
                Err(match error {
                    CommonArticulationDynamicClosureClearanceErrorV2::Cancelled => {
                        CommonArticulationDynamicClosureBridgeStopV2::Cancelled
                    }
                    CommonArticulationDynamicClosureClearanceErrorV2::DeadlineExceeded => {
                        CommonArticulationDynamicClosureBridgeStopV2::DeadlineExceeded
                    }
                    _ => unreachable!("outer checkpoint contains only stop outcomes"),
                })
            }
        },
    );
    if let Some(error) = requested_stop {
        return Err(error);
    }
    result.map_err(map_bridge_error_v2)
}

fn map_bridge_error_v2(
    error: CommonArticulationDynamicClosureBridgeErrorV2,
) -> CommonArticulationDynamicClosureClearanceErrorV2 {
    match error {
        CommonArticulationDynamicClosureBridgeErrorV2::Cancelled => {
            CommonArticulationDynamicClosureClearanceErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureBridgeErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureClearanceErrorV2::DeadlineExceeded
        }
        error => CommonArticulationDynamicClosureClearanceErrorV2::DynamicClosureBridge(error),
    }
}

pub(super) fn binding_fingerprint_v2(
    validated: &ValidatedInputV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<[u8; 32], CommonArticulationDynamicClosureClearanceErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_PREREQUISITE_MODEL_ID_V2.as_bytes());
    for binding in [
        validated.profile_binding,
        validated.decomposition_binding,
        validated.common_pose_binding,
        validated.audit_binding,
        validated.parent_schedule_binding,
        validated.bridge_binding,
    ] {
        checkpoint_v2(checkpoint)?;
        hash.update(binding);
    }
    hash.update(validated.parent_fixed_face.canonical_bytes());
    hash.update(validated.paper_thickness_bits.to_le_bytes());
    hash.update(validated.closure_tolerance_bits.to_le_bytes());
    for value in [
        validated.actual_block_count,
        validated.actual_face_count,
        validated.pair_registry_retained_bytes,
        validated.pair_registry_temporary_bytes,
        validated.publication_bytes,
        validated.aggregate_peak_bytes,
        validated.cross_block_pairs.len(),
    ] {
        checkpoint_v2(checkpoint)?;
        update_usize_v2(&mut hash, value)?;
    }
    hash_limits_v2(&mut hash, validated.limits)?;
    for pair in &validated.cross_block_pairs {
        checkpoint_v2(checkpoint)?;
        hash.update(pair.first_v2().canonical_bytes());
        hash.update(pair.second_v2().canonical_bytes());
    }
    Ok(hash.finalize().into())
}

fn audit_binding_fingerprint_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<[u8; 32], CommonArticulationDynamicClosureClearanceErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_DYNAMIC_CLOSURE_CLEARANCE_PREREQUISITE_MODEL_ID_V2.as_bytes());
    for value in [
        audit.faces().len(),
        audit.spanning_hinges().len(),
        audit.closure_hinges().len(),
    ] {
        checkpoint_v2(checkpoint)?;
        update_usize_v2(&mut hash, value)?;
    }
    for face in audit.faces() {
        checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    for edge in audit.spanning_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    for edge in audit.closure_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    Ok(hash.finalize().into())
}

fn hash_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationDynamicClosureClearanceLimitsV2,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    for value in [
        limits.max_blocks,
        limits.max_faces,
        limits.max_cross_block_pairs,
        limits.max_pair_registry_retained_bytes,
        limits.max_pair_registry_temporary_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ] {
        update_usize_v2(hash, value)?;
    }
    Ok(())
}

fn update_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicClosureClearanceStopV2::Cancelled => {
            CommonArticulationDynamicClosureClearanceErrorV2::Cancelled
        }
        CommonArticulationDynamicClosureClearanceStopV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureClearanceErrorV2::DeadlineExceeded
        }
    })
}
