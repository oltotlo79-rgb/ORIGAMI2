use std::collections::HashSet;

use ori_domain::EdgeId;

use super::{
    super::{
        block_cut_decomposition::{
            ContractedActiveEdgeV1, ContractedBlockCutV1, ContractedProfileClassV1,
            TarjanBiconnectedBlockV1,
        },
        exact_generator_word::{CanonicalInfiniteLineV1, ExactGeneratorProfileV1},
    },
    CarrierFreeProductBoundsV1,
};
use crate::CanonicalCycleScheduleV1;

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedCarrierEdgeV1 {
    pub(super) edge_index: usize,
    pub(super) carrier: usize,
    pub(super) profile: usize,
}

#[derive(Debug)]
pub(super) struct PreparedCarrierBlockV1 {
    pub(super) block_index: usize,
    pub(super) profile_count: usize,
    pub(super) labels: Vec<PreparedCarrierEdgeV1>,
}

fn collective_partition_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> Option<Option<HashSet<EdgeId>>> {
    let active = decomposition.active_edges();
    let cyclic_nonconstant = decomposition.blocks().iter().any(|block| {
        block.is_cyclic()
            && block
                .edge_indices()
                .iter()
                .any(|edge| active[*edge].profile_class() == ContractedProfileClassV1::Nonconstant)
    });
    if !cyclic_nonconstant {
        return Some(None);
    }

    let edges = schedule.collective_profile_edges_v1()?;
    let mut moving = HashSet::new();
    moving.try_reserve(edges.len()).ok()?;
    if edges.is_empty() || edges.iter().any(|edge| !moving.insert(*edge)) {
        return None;
    }
    if active.iter().any(|edge| {
        moving.contains(&edge.edge())
            != (edge.profile_class() == ContractedProfileClassV1::Nonconstant)
    }) {
        return None;
    }
    Some(Some(moving))
}

fn exact_profile_v1(
    class: ContractedProfileClassV1,
    edge: EdgeId,
    moving: Option<&HashSet<EdgeId>>,
) -> Option<ExactGeneratorProfileV1> {
    match class {
        ContractedProfileClassV1::Nonconstant => {
            if moving.is_none_or(|moving| !moving.contains(&edge)) {
                return None;
            }
            Some(ExactGeneratorProfileV1::CollectiveNonconstant)
        }
        ContractedProfileClassV1::ConstantAngle(bits) => {
            Some(ExactGeneratorProfileV1::ConstantAngle(bits))
        }
    }
}

fn prepare_one_block_v1(
    active: &[ContractedActiveEdgeV1],
    block_index: usize,
    block: &TarjanBiconnectedBlockV1,
    moving: Option<&HashSet<EdgeId>>,
    key_classifications: &mut usize,
) -> Option<PreparedCarrierBlockV1> {
    if !block.is_cyclic()
        || block.edge_indices().len() < 2
        || block.edge_indices().len() < block.vertices().len()
    {
        return None;
    }
    let edge_indices = block.edge_indices();
    let mut raw_profiles = Vec::new();
    raw_profiles.try_reserve_exact(edge_indices.len()).ok()?;
    let mut profiles = Vec::new();
    profiles.try_reserve_exact(edge_indices.len()).ok()?;
    let mut carriers = Vec::<CanonicalInfiniteLineV1>::new();
    carriers.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        *key_classifications = (*key_classifications).checked_add(1)?;
        let profile = exact_profile_v1(edge.profile_class(), edge.edge(), moving)?;
        raw_profiles.push(profile);
        profiles.push(profile);
        carriers.push(edge.line().clone());
    }
    profiles.sort_unstable();
    profiles.dedup();
    carriers.sort_unstable();
    carriers.dedup();
    if profiles.is_empty()
        || profiles.len() > super::MAX_CARRIER_FREE_PRODUCT_PROFILES_PER_BLOCK_V1
        || carriers.is_empty()
    {
        return None;
    }

    let mut labels = Vec::new();
    labels.try_reserve_exact(edge_indices.len()).ok()?;
    for (edge_index, profile) in edge_indices.iter().zip(raw_profiles) {
        let edge = active.get(*edge_index)?;
        labels.push(PreparedCarrierEdgeV1 {
            edge_index: *edge_index,
            carrier: carriers.binary_search(edge.line()).ok()?,
            profile: profiles.binary_search(&profile).ok()?,
        });
    }
    Some(PreparedCarrierBlockV1 {
        block_index,
        profile_count: profiles.len(),
        labels,
    })
}

pub(super) fn prepare_carrier_free_product_blocks_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> Option<(
    Vec<PreparedCarrierBlockV1>,
    CarrierFreeProductBoundsV1,
    usize,
)> {
    let moving = collective_partition_v1(schedule, decomposition)?;
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(decomposition.blocks().len())
        .ok()?;
    let mut bounds = CarrierFreeProductBoundsV1::empty();
    let mut key_classifications = 0usize;
    for (block_index, block) in decomposition.blocks().iter().enumerate() {
        if block.is_bridge() {
            if block.edge_indices().len() != 1 || block.vertices().len() != 2 {
                return None;
            }
            continue;
        }
        let block = prepare_one_block_v1(
            decomposition.active_edges(),
            block_index,
            block,
            moving.as_ref(),
            &mut key_classifications,
        )?;
        bounds.charge_block(block.labels.len(), block.profile_count)?;
        prepared.push(block);
    }
    if prepared.is_empty()
        || !bounds.is_nonempty()
        || key_classifications != bounds.key_classifications
    {
        return None;
    }
    Some((prepared, bounds, key_classifications))
}
