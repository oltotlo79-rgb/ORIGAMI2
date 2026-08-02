//! Checkpoint-pollable canonical cross-block pair construction.

use std::cmp::Ordering;

use ori_kinematics::CanonicalMaterialEdgeBlockDecompositionV2;

use super::super::*;
use super::checkpoint_v2;

const CANONICAL_MIURA_FACES_PER_BLOCK_V2: usize = 9;
const CANONICAL_MIURA_HINGES_PER_BLOCK_V2: usize = 12;
const RAW_PAIR_CANDIDATES_PER_BLOCK_PAIR_V2: usize = 81;
const CANONICAL_PAIRS_PER_ORDERED_BLOCK_PAIR_V2: usize = 32;

pub(super) fn raw_pair_candidate_budget_v2(
    block_count: usize,
) -> Result<usize, CommonArticulationDynamicClosureClearanceErrorV2> {
    unordered_pair_count_v2(block_count)?
        .checked_mul(RAW_PAIR_CANDIDATES_PER_BLOCK_PAIR_V2)
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)
}

pub(super) fn canonical_pair_budget_v2(
    block_count: usize,
) -> Result<usize, CommonArticulationDynamicClosureClearanceErrorV2> {
    block_count
        .checked_mul(
            block_count
                .checked_sub(1)
                .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_mul(CANONICAL_PAIRS_PER_ORDERED_BLOCK_PAIR_V2))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)
}

fn unordered_pair_count_v2(
    count: usize,
) -> Result<usize, CommonArticulationDynamicClosureClearanceErrorV2> {
    count
        .checked_mul(
            count
                .checked_sub(1)
                .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)
}

pub(super) fn validate_submitted_pair_order_v2(
    submitted: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    for index in 1..submitted.len() {
        checkpoint_v2(checkpoint)?;
        match compare_pair_v2(&submitted[index - 1], &submitted[index]) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(CommonArticulationDynamicClosureClearanceErrorV2::DuplicateCrossBlockPair)
            }
            Ordering::Greater => {
                return Err(
                    CommonArticulationDynamicClosureClearanceErrorV2::NonCanonicalCrossBlockPairRegistry,
                )
            }
        }
    }
    Ok(())
}

pub(super) fn enumerate_and_copy_canonical_pairs_v2(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    raw_pair_candidates: usize,
    expected_pair_count: usize,
    submitted: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<
    Vec<CommonArticulationCrossBlockFacePairV2>,
    CommonArticulationDynamicClosureClearanceErrorV2,
> {
    let blocks = decomposition.blocks();
    let mut raw_pairs = bounded_vec_v2(raw_pair_candidates)?;
    for first_index in 0..blocks.len() {
        checkpoint_v2(checkpoint)?;
        let first = &blocks[first_index];
        if first.geometry().face_ids().len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2
            || first.geometry().hinges().len() != CANONICAL_MIURA_HINGES_PER_BLOCK_V2
        {
            return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
        }
        for second in blocks.iter().skip(first_index + 1) {
            checkpoint_v2(checkpoint)?;
            if second.geometry().face_ids().len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2
                || second.geometry().hinges().len() != CANONICAL_MIURA_HINGES_PER_BLOCK_V2
            {
                return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
            }
            for first_face in first.geometry().face_ids() {
                checkpoint_v2(checkpoint)?;
                for second_face in second.geometry().face_ids() {
                    checkpoint_v2(checkpoint)?;
                    if let Some(pair) =
                        CommonArticulationCrossBlockFacePairV2::new(*first_face, *second_face)
                    {
                        raw_pairs.push(pair);
                    }
                }
            }
        }
    }
    let mut local_pairs = canonical_within_block_pairs_v2(blocks, checkpoint)?;
    heap_sort_pairs_v2(&mut local_pairs, checkpoint)?;
    dedup_sorted_pairs_v2(&mut local_pairs, checkpoint)?;
    filter_pairs_not_local_v2(&mut raw_pairs, &local_pairs, checkpoint)?;
    drop(local_pairs);
    heap_sort_pairs_v2(&mut raw_pairs, checkpoint)?;
    dedup_sorted_pairs_v2(&mut raw_pairs, checkpoint)?;
    if raw_pairs.len() != expected_pair_count {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }
    validate_complete_registry_v2(submitted, &raw_pairs, checkpoint)?;
    let mut retained = bounded_vec_v2(expected_pair_count)?;
    for pair in raw_pairs {
        checkpoint_v2(checkpoint)?;
        retained.push(pair);
    }
    Ok(retained)
}

fn canonical_within_block_pairs_v2(
    blocks: &[ori_kinematics::CanonicalMaterialEdgeBlockV1],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<
    Vec<CommonArticulationCrossBlockFacePairV2>,
    CommonArticulationDynamicClosureClearanceErrorV2,
> {
    let pairs_per_block = CANONICAL_MIURA_FACES_PER_BLOCK_V2
        .checked_mul(CANONICAL_MIURA_FACES_PER_BLOCK_V2 - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    let mut pairs = bounded_vec_v2(
        blocks
            .len()
            .checked_mul(pairs_per_block)
            .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?,
    )?;
    for block in blocks {
        checkpoint_v2(checkpoint)?;
        let faces = block.geometry().face_ids();
        for first in 0..faces.len() {
            checkpoint_v2(checkpoint)?;
            for second in (first + 1)..faces.len() {
                checkpoint_v2(checkpoint)?;
                pairs.push(
                    CommonArticulationCrossBlockFacePairV2::new(faces[first], faces[second])
                        .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::InvalidInput)?,
                );
            }
        }
    }
    Ok(pairs)
}

fn bounded_vec_v2(
    capacity: usize,
) -> Result<
    Vec<CommonArticulationCrossBlockFacePairV2>,
    CommonArticulationDynamicClosureClearanceErrorV2,
> {
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(capacity)
        .map_err(|_| CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
    if pairs.capacity() > capacity {
        return Err(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit);
    }
    Ok(pairs)
}

fn validate_complete_registry_v2(
    submitted: &[CommonArticulationCrossBlockFacePairV2],
    expected: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    if submitted.len() != expected.len() {
        return Err(
            CommonArticulationDynamicClosureClearanceErrorV2::CrossBlockPairCoverageMismatch {
                expected: expected.len(),
                actual: submitted.len(),
            },
        );
    }
    for (submitted_pair, expected_pair) in submitted.iter().zip(expected) {
        checkpoint_v2(checkpoint)?;
        if submitted_pair != expected_pair {
            return Err(
                CommonArticulationDynamicClosureClearanceErrorV2::CrossBlockPairCoverageMismatch {
                    expected: expected.len(),
                    actual: submitted.len(),
                },
            );
        }
    }
    Ok(())
}

fn filter_pairs_not_local_v2(
    pairs: &mut Vec<CommonArticulationCrossBlockFacePairV2>,
    local_pairs: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    let mut retained = 0usize;
    for index in 0..pairs.len() {
        checkpoint_v2(checkpoint)?;
        if !contains_sorted_pair_v2(local_pairs, &pairs[index], checkpoint)? {
            if retained != index {
                pairs[retained] = pairs[index];
            }
            retained = retained
                .checked_add(1)
                .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
        }
    }
    pairs.truncate(retained);
    Ok(())
}

fn contains_sorted_pair_v2(
    pairs: &[CommonArticulationCrossBlockFacePairV2],
    target: &CommonArticulationCrossBlockFacePairV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<bool, CommonArticulationDynamicClosureClearanceErrorV2> {
    let mut lower = 0usize;
    let mut upper = pairs.len();
    while lower < upper {
        checkpoint_v2(checkpoint)?;
        let middle = lower
            .checked_add((upper - lower) / 2)
            .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
        match compare_pair_v2(&pairs[middle], target) {
            Ordering::Less => lower = middle + 1,
            Ordering::Greater => upper = middle,
            Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}

fn heap_sort_pairs_v2(
    pairs: &mut [CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    for start in (0..pairs.len() / 2).rev() {
        sift_down_pair_v2(pairs, start, pairs.len(), checkpoint)?;
    }
    for end in (1..pairs.len()).rev() {
        checkpoint_v2(checkpoint)?;
        pairs.swap(0, end);
        sift_down_pair_v2(pairs, 0, end, checkpoint)?;
    }
    Ok(())
}

fn sift_down_pair_v2(
    pairs: &mut [CommonArticulationCrossBlockFacePairV2],
    mut root: usize,
    end: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
    loop {
        checkpoint_v2(checkpoint)?;
        let child = root
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
        if child >= end {
            return Ok(());
        }
        let mut largest = root;
        if compare_pair_v2(&pairs[largest], &pairs[child]) == Ordering::Less {
            largest = child;
        }
        let right = child
            .checked_add(1)
            .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
        if right < end && compare_pair_v2(&pairs[largest], &pairs[right]) == Ordering::Less {
            largest = right;
        }
        if largest == root {
            return Ok(());
        }
        pairs.swap(root, largest);
        root = largest;
    }
}

fn dedup_sorted_pairs_v2(
    pairs: &mut Vec<CommonArticulationCrossBlockFacePairV2>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureClearanceStopV2>,
) -> Result<(), CommonArticulationDynamicClosureClearanceErrorV2> {
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
                .ok_or(CommonArticulationDynamicClosureClearanceErrorV2::ResourceLimit)?;
        }
    }
    pairs.truncate(retained);
    Ok(())
}

fn compare_pair_v2(
    left: &CommonArticulationCrossBlockFacePairV2,
    right: &CommonArticulationCrossBlockFacePairV2,
) -> Ordering {
    left.first_v2()
        .canonical_bytes()
        .cmp(&right.first_v2().canonical_bytes())
        .then_with(|| {
            left.second_v2()
                .canonical_bytes()
                .cmp(&right.second_v2().canonical_bytes())
        })
}
