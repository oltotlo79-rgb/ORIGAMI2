//! Validation, registry construction, and binding internals for V2.

use std::{cmp::Ordering, mem::size_of};

use sha2::{Digest, Sha256};

use super::*;
mod nested_proof;
mod pair_compare;
use nested_proof::{
    audit_binding_fingerprint_v2, hash_whole_parent_closure_limits_v2,
    heap_sort_comparisons_per_item_v2, revalidate_common_pose_v2,
    revalidate_whole_parent_closure_v2,
};
pub(crate) use pair_compare::cross_block_pairs_equal_with_checkpoint_v2;

pub(super) struct ValidatedInputV2 {
    pub(super) profile_binding: [u8; 32],
    pub(super) decomposition_binding: [u8; 32],
    pub(super) common_pose_binding: [u8; 32],
    pub(super) block_closure_set_binding: [u8; 32],
    pub(super) whole_parent_closure_binding: [u8; 32],
    pub(super) audit_binding: [u8; 32],
    pub(super) parent_schedule_binding: [u8; 32],
    pub(super) parent_fixed_face: FaceId,
    pub(super) paper_thickness_bits: u64,
    pub(super) closure_tolerance_bits: u64,
    pub(super) actual_block_count: usize,
    pub(super) face_count: usize,
    pub(super) hinge_count: usize,
    pub(super) cross_block_pairs: Vec<CommonArticulationCrossBlockFacePairV2>,
    pub(super) logical_work: usize,
    pub(super) storage_bytes_upper_bound: usize,
    pub(super) whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
}

pub(super) fn validate_input_v2(
    input: &CommonArticulationClearanceInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<ValidatedInputV2, CommonArticulationClearanceErrorV2> {
    if !input.paper_thickness_mm.is_finite() || input.paper_thickness_mm <= 0.0 {
        return Err(CommonArticulationClearanceErrorV2::InvalidInput);
    }

    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || actual.block_count_v2() != actual_block_count
        || actual_block_count > maximum.block_count_v2()
        || face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.common_pose.actual_block_count_v2() != actual_block_count
        || input.common_pose.configured_max_blocks_v2() != configured_max_blocks
        || input.common_pose.logical_work_v2() != actual.pose_logical_work_v2()
        || input.common_pose.retained_bytes_upper_bound_v2() != actual.pose_retained_bytes_v2()
        || input.decomposition.logical_work_v2() != actual.decomposition_logical_work_v2()
        || input.decomposition.storage_bytes_upper_bound_v2()
            != actual.decomposition_storage_bytes_v2()
        || !input.decomposition.is_for_geometry(input.geometry)
        || !input.decomposition.is_for_profile_v2(input.profile)
    {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }

    // Reject a foreign schedule/fixed-face tuple before the stationary-only
    // workspace gate.  Otherwise a missing live derivative is misclassified
    // as a resource-envelope failure even though the retained nested proof
    // rejects the same tuple as malformed input.  This binding comparison is
    // allocation-free and therefore preserves the pre-allocation boundary.
    if !input
        .parent_schedule
        .matches_binding(input.geometry, input.audit, input.parent_fixed_face)
    {
        return Err(CommonArticulationClearanceErrorV2::WholeParentClosure(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                ori_kinematics::CommonArticulationBlockClosureSetErrorV2::InvalidInput,
            ),
        ));
    }

    // The current general-N clearance boundary has a complete physical
    // workspace proof only for the stationary, single-leaf V1 route.  A
    // non-stationary schedule can enter V1's adaptive dyadic prover, whose
    // rational-evaluation and worklist workspace is intentionally not yet a
    // V2 byte-bounded public contract.  Reject that route before pair or
    // nested-proof materialization rather than treating retained certificate
    // bytes as a substitute for its peak workspace.
    if !stationary_single_leaf_closure_contract_v2(input) {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }

    // This is allocation-free and must precede the stationary solve peak
    // calculation below: `checked_solve_closed_peak_bytes_v1` is meaningful
    // only for a geometry/audit pair with the declared carrier shape.
    audit_matches_geometry_v2(input.geometry, input.audit, checkpoint)?;

    let raw_pair_candidates = actual.raw_cross_block_pair_candidates_v2();
    let canonical_pair_count = actual.canonical_cross_block_pairs_v2();
    let unordered_face_pair_count = unordered_pair_count_v2(face_count)?;
    let observed_raw_pair_budget = raw_pair_candidate_budget_v2(actual_block_count)?;
    let observed_canonical_pair_budget = canonical_pair_budget_v2(actual_block_count)?;
    if actual.unordered_face_pair_count_v2() != unordered_face_pair_count
        || raw_pair_candidates != observed_raw_pair_budget
        || canonical_pair_count != observed_canonical_pair_budget
        || actual.raw_sort_comparisons_per_item_v2()
            != heap_sort_comparisons_per_item_v2(raw_pair_candidates)?
        || actual.canonical_sort_comparisons_per_item_v2()
            != heap_sort_comparisons_per_item_v2(canonical_pair_count)?
        || actual.unordered_face_pair_count_v2() > maximum.unordered_face_pair_count_v2()
        || raw_pair_candidates > maximum.raw_cross_block_pair_candidates_v2()
        || canonical_pair_count > maximum.canonical_cross_block_pairs_v2()
        || actual.raw_sort_comparisons_per_item_v2() > maximum.raw_sort_comparisons_per_item_v2()
        || actual.canonical_sort_comparisons_per_item_v2()
            > maximum.canonical_sort_comparisons_per_item_v2()
        || actual.pose_logical_work_v2() > maximum.pose_logical_work_v2()
        || actual.pose_retained_bytes_v2() > maximum.pose_retained_bytes_v2()
        || actual.decomposition_logical_work_v2() > maximum.decomposition_logical_work_v2()
        || actual.decomposition_storage_bytes_v2() > maximum.decomposition_storage_bytes_v2()
    {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }

    let pairs = enumerate_canonical_cross_block_pairs_v2(
        input.decomposition,
        raw_pair_candidates,
        canonical_pair_count,
        checkpoint,
    )?;
    validate_submitted_pairs_v2(input.submitted_cross_block_pairs, &pairs, checkpoint)?;

    let logical_work = clearance_logical_work_v2(
        actual.pose_logical_work_v2(),
        face_count,
        hinge_count,
        actual_block_count,
        raw_pair_candidates,
        canonical_pair_count,
        actual.raw_sort_comparisons_per_item_v2(),
        actual.canonical_sort_comparisons_per_item_v2(),
    )?;
    let storage_bytes_upper_bound = clearance_storage_bytes_v2(
        raw_pair_candidates,
        canonical_pair_count,
        face_count,
        hinge_count,
    )?;
    if logical_work != actual.clearance_logical_work_v2()
        || storage_bytes_upper_bound != actual.clearance_storage_bytes_v2()
        || logical_work > maximum.clearance_logical_work_v2()
        || storage_bytes_upper_bound > maximum.clearance_storage_bytes_v2()
    {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }

    // The retained profile field is the complete V2 clearance allocation
    // envelope, not merely the pair-registry allocation. Revalidation first
    // materializes fresh pose and closure candidates, so reject caller-supplied
    // closure ceilings that could exceed that envelope before any nested
    // candidate begins allocating. The phases are sequential; only their
    // maximum is live at once.
    let revalidation_workspace_bytes = clearance_revalidation_workspace_upper_bound_v2(
        actual.pose_retained_bytes_v2(),
        actual_block_count,
        face_count,
        hinge_count,
        input,
    )?;
    if revalidation_workspace_bytes > storage_bytes_upper_bound {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }

    revalidate_common_pose_v2(
        input.common_pose,
        CommonArticulationPoseInputV2 {
            geometry: input.geometry,
            pose: input.pose,
            decomposition: input.decomposition,
            paper_thickness_mm: input.paper_thickness_mm,
            profile: input.profile,
        },
        checkpoint,
    )?;
    revalidate_whole_parent_closure_v2(
        input.whole_parent_closure,
        CommonArticulationWholeParentClosureInputV2 {
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
            block_closure_set: input.block_closure_set,
            limits: input.whole_parent_closure_limits,
        },
        checkpoint,
    )?;

    let audit_binding = audit_binding_fingerprint_v2(input.audit, checkpoint)?;
    Ok(ValidatedInputV2 {
        profile_binding: input.profile.binding_fingerprint_v2(),
        decomposition_binding: input.decomposition.binding_fingerprint_v2(),
        common_pose_binding: input.common_pose.binding_fingerprint_v2(),
        block_closure_set_binding: input.block_closure_set.binding_fingerprint_v2(),
        whole_parent_closure_binding: input.whole_parent_closure.binding_fingerprint_v2(),
        audit_binding,
        parent_schedule_binding: input.parent_schedule.certificate_binding_fingerprint_v2(),
        parent_fixed_face: input.parent_fixed_face,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        actual_block_count,
        face_count,
        hinge_count,
        cross_block_pairs: pairs,
        logical_work,
        storage_bytes_upper_bound,
        whole_parent_closure_limits: input.whole_parent_closure_limits,
    })
}

fn audit_matches_geometry_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    if geometry.face_ids().len() != audit.faces().len()
        || geometry.hinges().len()
            != audit
                .spanning_hinges()
                .len()
                .checked_add(audit.closure_hinges().len())
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?
    {
        return Err(CommonArticulationClearanceErrorV2::AuditBindingMismatch);
    }
    for (geometry_face, audit_face) in geometry.face_ids().iter().zip(audit.faces()) {
        checkpoint_v2(checkpoint)?;
        if geometry_face != audit_face {
            return Err(CommonArticulationClearanceErrorV2::AuditBindingMismatch);
        }
    }

    let spanning = audit.spanning_hinges();
    let closure = audit.closure_hinges();
    let mut spanning_index = 0usize;
    let mut closure_index = 0usize;
    for hinge in geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        let next = match (spanning.get(spanning_index), closure.get(closure_index)) {
            (Some(first), Some(second)) => {
                if first.canonical_bytes() < second.canonical_bytes() {
                    spanning_index = spanning_index
                        .checked_add(1)
                        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
                    *first
                } else {
                    closure_index = closure_index
                        .checked_add(1)
                        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
                    *second
                }
            }
            (Some(first), None) => {
                spanning_index = spanning_index
                    .checked_add(1)
                    .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
                *first
            }
            (None, Some(second)) => {
                closure_index = closure_index
                    .checked_add(1)
                    .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
                *second
            }
            (None, None) => return Err(CommonArticulationClearanceErrorV2::AuditBindingMismatch),
        };
        if next != hinge.edge() {
            return Err(CommonArticulationClearanceErrorV2::AuditBindingMismatch);
        }
    }
    if spanning_index != spanning.len() || closure_index != closure.len() {
        return Err(CommonArticulationClearanceErrorV2::AuditBindingMismatch);
    }
    Ok(())
}

pub(crate) fn raw_pair_candidate_budget_v2(
    block_count: usize,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    unordered_pair_count_v2(block_count)?
        .checked_mul(CANONICAL_MIURA_RAW_PAIR_CANDIDATES_PER_BLOCK_PAIR_V2)
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

pub(crate) fn canonical_pair_budget_v2(
    block_count: usize,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    block_count
        .checked_mul(
            block_count
                .checked_sub(1)
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?,
        )
        .and_then(|value| {
            value.checked_mul(CANONICAL_MIURA_CANONICAL_PAIRS_PER_ORDERED_BLOCK_PAIR_V2)
        })
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

fn unordered_pair_count_v2(count: usize) -> Result<usize, CommonArticulationClearanceErrorV2> {
    count
        .checked_mul(
            count
                .checked_sub(1)
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

fn canonical_within_block_pairs_v2(
    blocks: &[ori_kinematics::CanonicalMaterialEdgeBlockV1],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<Vec<CommonArticulationCrossBlockFacePairV2>, CommonArticulationClearanceErrorV2> {
    let pairs_per_block = CANONICAL_MIURA_FACES_PER_BLOCK_V2
        .checked_mul(CANONICAL_MIURA_FACES_PER_BLOCK_V2 - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let capacity = blocks
        .len()
        .checked_mul(pairs_per_block)
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let mut local_pairs = Vec::new();
    local_pairs
        .try_reserve_exact(capacity)
        .map_err(|_| CommonArticulationClearanceErrorV2::ResourceLimit)?;
    if local_pairs.capacity() > capacity {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }
    for block in blocks {
        checkpoint_v2(checkpoint)?;
        let faces = block.geometry().face_ids();
        if faces.len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2 {
            return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
        }
        for first in 0..faces.len() {
            checkpoint_v2(checkpoint)?;
            let second_start = first
                .checked_add(1)
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
            for second in second_start..faces.len() {
                checkpoint_v2(checkpoint)?;
                let pair = CommonArticulationCrossBlockFacePairV2::new(faces[first], faces[second])
                    .ok_or(CommonArticulationClearanceErrorV2::InvalidInput)?;
                // A pair co-resident in any block is not a cross-block pair.
                // Overlap is compacted after the checkpoint-pollable sort.
                local_pairs.push(pair);
            }
        }
    }
    Ok(local_pairs)
}

pub(crate) fn filter_pairs_not_local_v2(
    pairs: &mut Vec<CommonArticulationCrossBlockFacePairV2>,
    sorted_local_pairs: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    let mut retained = 0usize;
    for index in 0..pairs.len() {
        checkpoint_v2(checkpoint)?;
        if !contains_sorted_pair_v2(sorted_local_pairs, &pairs[index], checkpoint)? {
            if retained != index {
                pairs[retained] = pairs[index];
            }
            retained = retained
                .checked_add(1)
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        }
    }
    pairs.truncate(retained);
    Ok(())
}

fn contains_sorted_pair_v2(
    sorted_pairs: &[CommonArticulationCrossBlockFacePairV2],
    target: &CommonArticulationCrossBlockFacePairV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<bool, CommonArticulationClearanceErrorV2> {
    let mut lower = 0usize;
    let mut upper = sorted_pairs.len();
    while lower < upper {
        checkpoint_v2(checkpoint)?;
        let middle = lower
            .checked_add((upper - lower) / 2)
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        match compare_pair_v2(&sorted_pairs[middle], target) {
            Ordering::Less => {
                lower = middle
                    .checked_add(1)
                    .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
            }
            Ordering::Greater => upper = middle,
            Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}

fn heap_sort_pairs_v2(
    pairs: &mut [CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    heap_sort_pairs_with_comparison_hook_v2(pairs, checkpoint, &mut || {})
}

#[cfg(test)]
pub(crate) fn heap_sort_pairs_and_count_comparisons_v2(
    pairs: &mut [CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    let mut comparisons = 0usize;
    heap_sort_pairs_with_comparison_hook_v2(pairs, checkpoint, &mut || {
        comparisons = comparisons
            .checked_add(1)
            .expect("test comparison count fits usize");
    })?;
    Ok(comparisons)
}

fn heap_sort_pairs_with_comparison_hook_v2(
    pairs: &mut [CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
    comparison_hook: &mut impl FnMut(),
) -> Result<(), CommonArticulationClearanceErrorV2> {
    let mut root = pairs.len() / 2;
    while root > 0 {
        checkpoint_v2(checkpoint)?;
        root = root
            .checked_sub(1)
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        sift_down_pair_v2(pairs, root, pairs.len(), checkpoint, comparison_hook)?;
    }
    let mut end = pairs.len();
    while end > 1 {
        end = end
            .checked_sub(1)
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        checkpoint_v2(checkpoint)?;
        pairs.swap(0, end);
        sift_down_pair_v2(pairs, 0, end, checkpoint, comparison_hook)?;
    }
    Ok(())
}

fn sift_down_pair_v2(
    pairs: &mut [CommonArticulationCrossBlockFacePairV2],
    mut root: usize,
    end: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
    comparison_hook: &mut impl FnMut(),
) -> Result<(), CommonArticulationClearanceErrorV2> {
    loop {
        checkpoint_v2(checkpoint)?;
        let child = root
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        if child >= end {
            return Ok(());
        }
        let mut largest = root;
        comparison_hook();
        if compare_pair_v2(&pairs[largest], &pairs[child]) == Ordering::Less {
            largest = child;
        }
        let right = child
            .checked_add(1)
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        if right < end {
            comparison_hook();
            if compare_pair_v2(&pairs[largest], &pairs[right]) == Ordering::Less {
                largest = right;
            }
        }
        if largest == root {
            return Ok(());
        }
        pairs.swap(root, largest);
        root = largest;
    }
}

pub(crate) fn dedup_sorted_pairs_v2(
    pairs: &mut Vec<CommonArticulationCrossBlockFacePairV2>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    if pairs.is_empty() {
        return Ok(());
    }
    let mut retained = 1usize;
    for index in 1..pairs.len() {
        checkpoint_v2(checkpoint)?;
        if pairs[index] != pairs[retained - 1] {
            if retained != index {
                pairs[retained] = pairs[index];
            }
            retained = retained
                .checked_add(1)
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        }
    }
    pairs.truncate(retained);
    Ok(())
}

pub(crate) fn enumerate_canonical_cross_block_pairs_v2(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    raw_pair_candidates: usize,
    expected_canonical_pair_count: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<Vec<CommonArticulationCrossBlockFacePairV2>, CommonArticulationClearanceErrorV2> {
    let blocks = decomposition.blocks();
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(raw_pair_candidates)
        .map_err(|_| CommonArticulationClearanceErrorV2::ResourceLimit)?;
    if pairs.capacity() > raw_pair_candidates {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }
    for first in 0..blocks.len() {
        checkpoint_v2(checkpoint)?;
        let first_faces = blocks[first].geometry().face_ids();
        if first_faces.len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2
            || blocks[first].geometry().hinges().len() != CANONICAL_MIURA_HINGES_PER_BLOCK_V2
        {
            return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
        }
        let second_start = first
            .checked_add(1)
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
        for second_block in blocks.iter().skip(second_start) {
            checkpoint_v2(checkpoint)?;
            let second_faces = second_block.geometry().face_ids();
            if second_faces.len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2
                || second_block.geometry().hinges().len() != CANONICAL_MIURA_HINGES_PER_BLOCK_V2
            {
                return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
            }
            for first_face in first_faces.iter().copied() {
                checkpoint_v2(checkpoint)?;
                for second_face in second_faces.iter().copied() {
                    checkpoint_v2(checkpoint)?;
                    if let Some(pair) =
                        CommonArticulationCrossBlockFacePairV2::new(first_face, second_face)
                    {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    let mut local_pairs = canonical_within_block_pairs_v2(blocks, checkpoint)?;
    heap_sort_pairs_v2(&mut local_pairs, checkpoint)?;
    dedup_sorted_pairs_v2(&mut local_pairs, checkpoint)?;
    filter_pairs_not_local_v2(&mut pairs, &local_pairs, checkpoint)?;
    heap_sort_pairs_v2(&mut pairs, checkpoint)?;
    dedup_sorted_pairs_v2(&mut pairs, checkpoint)?;
    if pairs.len() != expected_canonical_pair_count {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }
    Ok(pairs)
}

pub(crate) fn validate_submitted_pairs_v2(
    submitted: &[CommonArticulationCrossBlockFacePairV2],
    expected: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    if submitted.len() != expected.len() {
        return Err(
            CommonArticulationClearanceErrorV2::CrossBlockPairCoverageMismatch {
                expected: expected.len(),
                actual: submitted.len(),
            },
        );
    }
    for (index, pair) in submitted.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        if index > 0 {
            match compare_pair_v2(&submitted[index - 1], pair) {
                Ordering::Equal => {
                    return Err(CommonArticulationClearanceErrorV2::DuplicateCrossBlockPair);
                }
                Ordering::Greater => {
                    return Err(
                        CommonArticulationClearanceErrorV2::NonCanonicalCrossBlockPairRegistry,
                    );
                }
                Ordering::Less => {}
            }
        }
    }
    for (index, pair) in submitted.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        if pair != &expected[index] {
            return Err(
                CommonArticulationClearanceErrorV2::CrossBlockPairCoverageMismatch {
                    expected: expected.len(),
                    actual: submitted.len(),
                },
            );
        }
    }
    Ok(())
}

fn compare_pair_v2(
    left: &CommonArticulationCrossBlockFacePairV2,
    right: &CommonArticulationCrossBlockFacePairV2,
) -> Ordering {
    left.first
        .canonical_bytes()
        .cmp(&right.first.canonical_bytes())
        .then_with(|| {
            left.second
                .canonical_bytes()
                .cmp(&right.second.canonical_bytes())
        })
}

#[allow(clippy::too_many_arguments)]
fn clearance_logical_work_v2(
    pose_work: usize,
    face_count: usize,
    hinge_count: usize,
    block_count: usize,
    raw_pair_candidates: usize,
    canonical_pair_count: usize,
    raw_sort_comparisons_per_item: usize,
    canonical_sort_comparisons_per_item: usize,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    CLEARANCE_BASE_WORK_V2
        .checked_add(pose_work)
        .and_then(|value| value.checked_add(face_count))
        .and_then(|value| value.checked_add(hinge_count))
        .and_then(|value| value.checked_add(block_count))
        .and_then(|value| value.checked_add(raw_pair_candidates.checked_mul(3)?))
        .and_then(|value| value.checked_add(canonical_pair_count.checked_mul(2)?))
        .and_then(|value| {
            value.checked_add(raw_pair_candidates.checked_mul(raw_sort_comparisons_per_item)?)
        })
        .and_then(|value| {
            value
                .checked_add(canonical_pair_count.checked_mul(canonical_sort_comparisons_per_item)?)
        })
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

fn clearance_storage_bytes_v2(
    raw_pair_candidates: usize,
    canonical_pair_count: usize,
    face_count: usize,
    hinge_count: usize,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    if size_of::<CommonArticulationCrossBlockFacePairV2>() != CLEARANCE_PAIR_BYTES_V2 {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }
    // The raw vector and <= 36N local-pair vector coexist while filtering.
    // For N >= 33, the canonical allocation has 32N(N - 1) >= 1_024N slots,
    // so its credited storage safely covers that local vector while the raw
    // allocation remains live, and then covers retained registry capacity.
    raw_pair_candidates
        .checked_add(canonical_pair_count)
        .and_then(|value| value.checked_mul(CLEARANCE_PAIR_BYTES_V2))
        .and_then(|value| value.checked_add(CLEARANCE_BASE_BYTES_V2))
        .and_then(|value| value.checked_add(face_count.checked_mul(CLEARANCE_FACE_BYTES_V2)?))
        .and_then(|value| value.checked_add(hinge_count.checked_mul(CLEARANCE_HINGE_BYTES_V2)?))
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

/// Bounds the dynamic allocations retained by one fresh V2 revalidation
/// candidate. This deliberately uses the submitted closure ceilings rather
/// than the already-retained observations: revalidation must fail before a
/// permissive caller limit can make an oversized candidate live.
///
/// The V2 candidate phases are sequential: common-pose replay, all-block
/// replay, and parent-closure replay. The all-block candidate is the only
/// phase that retains both its block records and their schedule/closure
/// payloads. The two edge vectors coexist while complete parent coverage is
/// checked. Per-block payload limits are charged once more because the
/// current item has been materialized before a total-limit rejection can run.
fn clearance_revalidation_workspace_upper_bound_v2(
    pose_retained_bytes: usize,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    input: &CommonArticulationClearanceInputV2<'_>,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    let limits = input.whole_parent_closure_limits;
    let block = limits.block_closure_set_limits;
    let block_closure_bytes = dyadic_closure_allocation_upper_bound_v2(
        block.per_block_closure_limits.max_leaves,
        CANONICAL_MIURA_HINGES_PER_BLOCK_V2,
    )?;
    let parent_closure_bytes = dyadic_closure_allocation_upper_bound_v2(
        limits.parent_closure_limits.max_leaves,
        hinge_count,
    )?;
    if block_closure_bytes > block.max_block_closure_bytes
        || parent_closure_bytes > limits.max_parent_closure_bytes
    {
        return Err(CommonArticulationClearanceErrorV2::ResourceLimit);
    }
    let block_dyadic_peak =
        input
            .decomposition
            .blocks()
            .iter()
            .try_fold(0usize, |maximum, block| {
                stationary_single_leaf_dyadic_peak_bytes_v2(block.geometry(), block.audit())
                    .map(|peak| maximum.max(peak))
            })?;
    let parent_dyadic_peak =
        stationary_single_leaf_dyadic_peak_bytes_v2(input.geometry, input.audit)?;
    let block_record_headers = actual_block_count
        .checked_mul(CLEARANCE_REVALIDATION_BLOCK_RECORD_BYTES_UPPER_BOUND_V2)
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let edge_coverage_vectors = hinge_count
        .checked_mul(size_of::<ori_domain::EdgeId>())
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let articulation_faces = actual_block_count
        .checked_mul(size_of::<FaceId>())
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let block_candidate = CLEARANCE_REVALIDATION_BASE_BYTES_V2
        // Restriction can materialize the current block schedule before its
        // own per-block retained-byte comparison; the source schedule is the
        // finite outer ceiling for that transient clone.
        .checked_add(block.max_parent_schedule_bytes)
        .and_then(|bytes| bytes.checked_add(block.max_total_block_schedule_bytes))
        .and_then(|bytes| bytes.checked_add(block.max_block_schedule_bytes))
        .and_then(|bytes| bytes.checked_add(block.max_total_block_closure_bytes))
        .and_then(|bytes| bytes.checked_add(block.max_block_closure_bytes))
        // Retained closure ceilings above cover published proof payloads. This
        // independent term covers the stationary issuer's live
        // `evaluate(0) + solve_closed` workspace before its single leaf can
        // become part of a record.
        .and_then(|bytes| bytes.checked_add(block_dyadic_peak))
        .and_then(|bytes| bytes.checked_add(block_record_headers))
        .and_then(|bytes| bytes.checked_add(edge_coverage_vectors))
        .and_then(|bytes| bytes.checked_add(articulation_faces))
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    // The parent schedule is borrowed, but its submitted ceiling is charged
    // alongside the fresh parent certificate so the envelope remains valid
    // if a later V2 parent reissuer materializes that schedule.
    let parent_candidate = CLEARANCE_REVALIDATION_BASE_BYTES_V2
        .checked_add(limits.max_parent_schedule_bytes)
        .and_then(|bytes| bytes.checked_add(limits.max_parent_closure_bytes))
        .and_then(|bytes| bytes.checked_add(parent_dyadic_peak))
        .and_then(|bytes| bytes.checked_add(face_count.checked_mul(size_of::<FaceId>())?))
        .and_then(|bytes| {
            bytes.checked_add(hinge_count.checked_mul(size_of::<ori_domain::EdgeId>())?)
        })
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    Ok(pose_retained_bytes
        .max(block_candidate)
        .max(parent_candidate))
}

/// Returns whether the submitted V2 closure configuration is in the only V1
/// dyadic route whose complete workspace is presently included in the
/// clearance byte envelope.
///
/// A stationary parent schedule restricts to stationary block schedules
/// because restriction copies the exact edge profiles rather than evaluating
/// or synthesizing new motion.  The canonical N>=33 block shapes also fail
/// every multi-leaf special premise by their cardinality before allocation,
/// so V1 deterministically takes its stationary single-leaf path.
fn stationary_single_leaf_closure_contract_v2(
    input: &CommonArticulationClearanceInputV2<'_>,
) -> bool {
    let limits = input.whole_parent_closure_limits;
    let block_limits = limits.block_closure_set_limits.per_block_closure_limits;
    let parent_limits = limits.parent_closure_limits;
    block_limits.max_depth == 0
        && block_limits.max_leaves == 1
        && parent_limits.max_depth == 0
        && parent_limits.max_leaves == 1
        && input.geometry.hinges().iter().all(|hinge| {
            input
                .parent_schedule
                .derivative_bound(hinge.edge())
                .is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())
        })
}

/// Physical peak for V1's stationary dyadic path under the preceding
/// single-leaf contract.  `solve_closed` already reports its internal maps,
/// vectors, allocator slack, and returned pose.  The schedule evaluation that
/// supplies that solve is live at the same time, while the final one-leaf
/// dyadic certificate is produced only after the pose is discarded.
pub(super) fn stationary_single_leaf_dyadic_peak_bytes_v2(
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    audit: &ori_kinematics::MaterialHingeGraphAudit,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    let hinge_count = geometry.hinges().len();
    let evaluated_angles_bytes = hinge_count
        .checked_mul(size_of::<ori_kinematics::HingeAngle>())
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let solve_peak = geometry
        .checked_solve_closed_peak_bytes_v1(audit)
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let stationary_solve_peak = evaluated_angles_bytes
        .checked_add(solve_peak)
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    let single_leaf_retained = dyadic_closure_allocation_upper_bound_v2(1, hinge_count)?;
    Ok(stationary_solve_peak.max(single_leaf_retained))
}

/// Maximum dynamically retained payload of one V1 dyadic candidate under its
/// public leaf and hinge limits. The V1 issuer checks its retained-byte limit
/// after construction, so this is also checked before invoking it from V2.
fn dyadic_closure_allocation_upper_bound_v2(
    max_leaves: usize,
    hinge_count: usize,
) -> Result<usize, CommonArticulationClearanceErrorV2> {
    let leaf = size_of::<(
        u32,
        u64,
        ori_kinematics::MaterialHingeIntervalClosureCertificateV1,
    )>()
    .checked_add(
        hinge_count
            .checked_mul(size_of::<ori_domain::EdgeId>())
            .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?,
    )
    .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?;
    size_of::<ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1>()
        .checked_add(
            max_leaves
                .checked_mul(leaf)
                .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)?,
        )
        .ok_or(CommonArticulationClearanceErrorV2::ResourceLimit)
}

pub(super) fn clearance_binding_fingerprint_v2(
    validated: &ValidatedInputV2,
    pairs: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<[u8; 32], CommonArticulationClearanceErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V2.as_bytes());
    hash.update(validated.profile_binding);
    hash.update(validated.decomposition_binding);
    hash.update(validated.common_pose_binding);
    hash.update(validated.block_closure_set_binding);
    hash.update(validated.whole_parent_closure_binding);
    hash.update(validated.audit_binding);
    hash.update(validated.parent_schedule_binding);
    hash.update(validated.parent_fixed_face.canonical_bytes());
    hash.update(validated.paper_thickness_bits.to_le_bytes());
    hash.update(validated.closure_tolerance_bits.to_le_bytes());
    for value in [
        validated.actual_block_count,
        validated.face_count,
        validated.hinge_count,
        validated.logical_work,
        validated.storage_bytes_upper_bound,
        pairs.len(),
    ] {
        checkpoint_v2(checkpoint)?;
        update_usize_v2(&mut hash, value)?;
    }
    hash_whole_parent_closure_limits_v2(&mut hash, validated.whole_parent_closure_limits)?;
    for pair in pairs {
        checkpoint_v2(checkpoint)?;
        hash.update(pair.first.canonical_bytes());
        hash.update(pair.second.canonical_bytes());
    }
    Ok(hash.finalize().into())
}

fn update_usize_v2(
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

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<(), CommonArticulationClearanceErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationClearanceStopV2::Cancelled => {
            CommonArticulationClearanceErrorV2::Cancelled
        }
        CommonArticulationClearanceStopV2::DeadlineExceeded => {
            CommonArticulationClearanceErrorV2::DeadlineExceeded
        }
    })
}
