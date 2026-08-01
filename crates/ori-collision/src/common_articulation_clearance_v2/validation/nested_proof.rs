//! Revalidation and hash framing for nested, non-authorizing V2 proofs.
//!
//! The outer clearance boundary owns the stop vocabulary.  Nested proof
//! failures retain their precise error type, except a caller-requested stop,
//! which is normalized before it can cross this boundary.

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn revalidate_common_pose_v2(
    authority: &CommonArticulationPoseAuthorityV2,
    input: CommonArticulationPoseInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    let mut requested_stop = None;
    let result =
        authority.revalidate_with_checkpoint_v2(input, || match checkpoint_v2(checkpoint) {
            Ok(()) => Ok(()),
            Err(error) => {
                requested_stop = Some(error);
                Err(match error {
                    CommonArticulationClearanceErrorV2::Cancelled => {
                        CommonArticulationPoseStopV2::Cancelled
                    }
                    CommonArticulationClearanceErrorV2::DeadlineExceeded => {
                        CommonArticulationPoseStopV2::DeadlineExceeded
                    }
                    _ => unreachable!("outer checkpoint only has stop errors"),
                })
            }
        });
    if let Some(error) = requested_stop {
        return Err(error);
    }
    result.map_err(map_common_pose_error_v2)
}

pub(super) fn revalidate_whole_parent_closure_v2(
    evidence: &CommonArticulationWholeParentClosureV2,
    input: CommonArticulationWholeParentClosureInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    let mut requested_stop = None;
    let result =
        evidence.revalidate_with_checkpoint_v2(input, || match checkpoint_v2(checkpoint) {
            Ok(()) => Ok(()),
            Err(error) => {
                requested_stop = Some(error);
                Err(match error {
                    CommonArticulationClearanceErrorV2::Cancelled => {
                        CommonArticulationWholeParentClosureStopV2::Cancelled
                    }
                    CommonArticulationClearanceErrorV2::DeadlineExceeded => {
                        CommonArticulationWholeParentClosureStopV2::DeadlineExceeded
                    }
                    _ => unreachable!("outer checkpoint only has stop errors"),
                })
            }
        });
    if let Some(error) = requested_stop {
        return Err(error);
    }
    result.map_err(map_whole_parent_closure_error_v2)
}

pub(super) fn hash_whole_parent_closure_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationWholeParentClosureLimitsV2,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    let block = limits.block_closure_set_limits;
    for value in [
        block.max_blocks,
        block.max_parent_schedule_bytes,
        block.max_block_schedule_bytes,
        block.max_total_block_schedule_bytes,
        block.max_block_closure_bytes,
        block.max_total_block_closure_bytes,
        block.max_total_closure_leaves,
        limits.max_parent_schedule_bytes,
        limits.max_parent_closure_bytes,
        limits.max_parent_closure_leaves,
    ] {
        hash_count_v2(hash, value)?;
    }
    hash_dyadic_limits_v2(hash, block.per_block_closure_limits)?;
    hash_dyadic_limits_v2(hash, limits.parent_closure_limits)
}

pub(super) fn audit_binding_fingerprint_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<[u8; 32], CommonArticulationClearanceErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V2.as_bytes());
    for value in [
        audit.faces().len(),
        audit.spanning_hinges().len(),
        audit.closure_hinges().len(),
    ] {
        checkpoint_v2(checkpoint)?;
        hash_count_v2(&mut hash, value)?;
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

fn map_common_pose_error_v2(
    error: CommonArticulationPoseErrorV2,
) -> CommonArticulationClearanceErrorV2 {
    match error {
        CommonArticulationPoseErrorV2::Cancelled => CommonArticulationClearanceErrorV2::Cancelled,
        CommonArticulationPoseErrorV2::DeadlineExceeded => {
            CommonArticulationClearanceErrorV2::DeadlineExceeded
        }
        error => CommonArticulationClearanceErrorV2::CommonPose(error),
    }
}

fn map_whole_parent_closure_error_v2(
    error: CommonArticulationWholeParentClosureErrorV2,
) -> CommonArticulationClearanceErrorV2 {
    match error {
        CommonArticulationWholeParentClosureErrorV2::Cancelled => {
            CommonArticulationClearanceErrorV2::Cancelled
        }
        CommonArticulationWholeParentClosureErrorV2::DeadlineExceeded => {
            CommonArticulationClearanceErrorV2::DeadlineExceeded
        }
        error => CommonArticulationClearanceErrorV2::WholeParentClosure(error),
    }
}

pub(super) fn heap_sort_comparisons_per_item_v2(
    value: usize,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    let bit_length = usize::BITS as usize - value.max(1).leading_zeros() as usize;
    bit_length
        .checked_mul(CLEARANCE_HEAPSORT_COMPARISON_FACTOR_V2)
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

fn hash_dyadic_limits_v2(
    hash: &mut Sha256,
    limits: ori_kinematics::DyadicIntervalClosureLimitsV1,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    hash.update(limits.max_depth.to_le_bytes());
    hash_count_v2(hash, limits.max_leaves)?;
    hash_count_v2(hash, limits.max_work)?;
    hash_count_v2(hash, limits.schedule_limits.max_hinges)?;
    hash_count_v2(hash, limits.schedule_limits.max_degree)?;
    hash.update(limits.schedule_limits.max_coefficient_bits.to_le_bytes());
    hash_count_v2(hash, limits.schedule_limits.max_work)
}

fn hash_count_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationClearanceErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
