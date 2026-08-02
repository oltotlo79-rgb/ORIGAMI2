//! Canonical adaptive collision partition and coverage accounting.

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn prove_partition_v2(
    input: &OrdinaryIntervalInputV2<'_>,
    validated: &ValidatedInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<ProofRunV2, OrdinaryIntervalErrorV2> {
    let resources = validated.resources;
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(input.limits.max_collision_leaves)
        .map_err(|_| OrdinaryIntervalErrorV2::ResourceLimit)?;
    if pending.capacity() > input.limits.max_collision_leaves {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }
    pending.push(DyadicLeafV2 { depth: 0, index: 0 });
    let mut live_leaf_count = 1usize;
    let mut accepted_leaf_count = 0usize;
    let mut processed_interval_node_count = 0usize;
    let mut maximum_accepted_depth = 0u32;
    let mut certified_ordinary_pair_leaf_count = 0usize;
    let mut partition_hash = Sha256::new();
    partition_hash.update(b"origami2/dynamic-general-n/ordinary-collision-partition/v2");
    while let Some(leaf) = pending.pop() {
        checkpoint_v2(checkpoint)?;
        processed_interval_node_count = processed_interval_node_count
            .checked_add(1)
            .filter(|value| *value <= resources.charged_interval_nodes)
            .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
        match super::geometry::prove_leaf_v2(input, leaf, validated, checkpoint)? {
            true => {
                accepted_leaf_count = accepted_leaf_count
                    .checked_add(1)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                maximum_accepted_depth = maximum_accepted_depth.max(leaf.depth);
                certified_ordinary_pair_leaf_count = certified_ordinary_pair_leaf_count
                    .checked_add(resources.ordinary_face_pairs)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                partition_hash.update(leaf.depth.to_le_bytes());
                partition_hash.update(leaf.index.to_le_bytes());
            }
            false => {
                if leaf.depth >= input.limits.max_collision_depth
                    || live_leaf_count >= input.limits.max_collision_leaves
                {
                    return Err(OrdinaryIntervalErrorV2::UnprovenOrdinaryClearance);
                }
                let child_depth = leaf
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth < 64)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                let left_index = leaf
                    .index
                    .checked_mul(2)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                let right_index = left_index
                    .checked_add(1)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                live_leaf_count = live_leaf_count
                    .checked_add(1)
                    .filter(|value| *value <= input.limits.max_collision_leaves)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
                // LIFO: right first, so accepted descriptors are hashed in
                // canonical left-to-right domain order.
                pending.push(DyadicLeafV2 {
                    depth: child_depth,
                    index: right_index,
                });
                pending.push(DyadicLeafV2 {
                    depth: child_depth,
                    index: left_index,
                });
            }
        }
    }
    if accepted_leaf_count != live_leaf_count
        || certified_ordinary_pair_leaf_count
            != accepted_leaf_count
                .checked_mul(resources.ordinary_face_pairs)
                .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?
    {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    update_usize_v2(&mut partition_hash, accepted_leaf_count)?;
    update_usize_v2(&mut partition_hash, certified_ordinary_pair_leaf_count)?;
    Ok(ProofRunV2 {
        collision_partition_digest: partition_hash.finalize().into(),
        accepted_leaf_count,
        processed_interval_node_count,
        maximum_accepted_depth,
        certified_ordinary_pair_leaf_count,
    })
}
