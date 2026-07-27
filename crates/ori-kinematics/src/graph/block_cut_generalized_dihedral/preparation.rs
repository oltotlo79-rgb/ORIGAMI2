use std::collections::HashSet;

use ori_domain::EdgeId;

use super::{
    super::{
        block_cut_decomposition::{
            ContractedActiveEdgeV1, ContractedBlockCutV1, ContractedProfileClassV1,
            TarjanBiconnectedBlockV1,
        },
        exact_generator_word::{
            CanonicalInfiniteLineV1, ExactGeneratorProfileV1, exact_perpendicular_intersection_v1,
        },
    },
    GeneralizedDihedralBoundsV1,
};
use crate::CanonicalCycleScheduleV1;

const HALF_TURN_BITS_V1: u64 = 180.0_f64.to_bits();

#[derive(Debug, Clone, Copy)]
pub(super) enum PreparedDihedralEdgeKindV1 {
    Primary { profile: usize },
    HalfTurn,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedDihedralEdgeV1 {
    pub(super) edge_index: usize,
    pub(super) kind: PreparedDihedralEdgeKindV1,
}

#[derive(Debug)]
pub(super) struct PreparedDihedralBlockV1 {
    pub(super) block_index: usize,
    pub(super) profile_count: usize,
    pub(super) labels: Vec<PreparedDihedralEdgeV1>,
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
    edge: &ContractedActiveEdgeV1,
    moving: Option<&HashSet<EdgeId>>,
) -> Option<ExactGeneratorProfileV1> {
    match edge.profile_class() {
        ContractedProfileClassV1::Nonconstant => {
            if moving.is_none_or(|moving| !moving.contains(&edge.edge())) {
                return None;
            }
            Some(ExactGeneratorProfileV1::CollectiveNonconstant)
        }
        ContractedProfileClassV1::ConstantAngle(bits) => {
            Some(ExactGeneratorProfileV1::ConstantAngle(bits))
        }
    }
}

fn is_half_turn_v1(edge: &ContractedActiveEdgeV1) -> bool {
    edge.profile_class() == ContractedProfileClassV1::ConstantAngle(HALF_TURN_BITS_V1)
}

fn prepare_one_block_v1(
    active: &[ContractedActiveEdgeV1],
    block_index: usize,
    block: &TarjanBiconnectedBlockV1,
    moving: Option<&HashSet<EdgeId>>,
    classifications: &mut usize,
) -> Option<PreparedDihedralBlockV1> {
    let edge_indices = block.edge_indices();
    if !block.is_cyclic() || edge_indices.len() < 2 || edge_indices.len() < block.vertices().len() {
        return None;
    }

    let mut carriers = Vec::<CanonicalInfiniteLineV1>::new();
    carriers.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        carriers.push(active.get(*edge_index)?.line().clone());
    }
    carriers.sort_unstable();
    carriers.dedup();
    let [first, second] = carriers.as_slice() else {
        return None;
    };
    if !exact_perpendicular_intersection_v1(first, second) {
        return None;
    }

    let mut carrier_is_half_turn = [true; 2];
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        let carrier = carriers.binary_search(edge.line()).ok()?;
        carrier_is_half_turn[carrier] &= is_half_turn_v1(edge);
    }
    // The exact carrier sort is storage invariant. If both factors consist
    // only of half-turns, the higher canonical carrier is deterministically B;
    // treating the lower carrier as a free integer factor is conservative.
    let half_turn_carrier = match carrier_is_half_turn {
        [false, true] => 1,
        [true, false] => 0,
        [true, true] => 1,
        [false, false] => return None,
    };
    let primary_carrier = 1usize.checked_sub(half_turn_carrier)?;

    let mut raw_profiles = Vec::new();
    raw_profiles.try_reserve_exact(edge_indices.len()).ok()?;
    let mut profiles = Vec::new();
    profiles.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        *classifications = (*classifications).checked_add(1)?;
        let carrier = carriers.binary_search(edge.line()).ok()?;
        if carrier == half_turn_carrier {
            if !is_half_turn_v1(edge) {
                return None;
            }
            raw_profiles.push(None);
        } else if carrier == primary_carrier {
            let profile = exact_profile_v1(edge, moving)?;
            profiles.push(profile);
            raw_profiles.push(Some(profile));
        } else {
            return None;
        }
    }
    profiles.sort_unstable();
    profiles.dedup();
    if profiles.is_empty() || profiles.len() > super::MAX_DIHEDRAL_PROFILES_PER_BLOCK_V1 {
        return None;
    }

    let mut labels = Vec::new();
    labels.try_reserve_exact(edge_indices.len()).ok()?;
    for (edge_index, profile) in edge_indices.iter().zip(raw_profiles) {
        labels.push(PreparedDihedralEdgeV1 {
            edge_index: *edge_index,
            kind: match profile {
                Some(profile) => PreparedDihedralEdgeKindV1::Primary {
                    profile: profiles.binary_search(&profile).ok()?,
                },
                None => PreparedDihedralEdgeKindV1::HalfTurn,
            },
        });
    }
    Some(PreparedDihedralBlockV1 {
        block_index,
        profile_count: profiles.len(),
        labels,
    })
}

pub(super) fn prepare_generalized_dihedral_blocks_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> Option<(
    Vec<PreparedDihedralBlockV1>,
    GeneralizedDihedralBoundsV1,
    usize,
)> {
    let moving = collective_partition_v1(schedule, decomposition)?;
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(decomposition.blocks().len())
        .ok()?;
    let mut bounds = GeneralizedDihedralBoundsV1::empty();
    let mut classifications = 0usize;
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
            &mut classifications,
        )?;
        let shape = decomposition.blocks().get(block.block_index)?;
        bounds.charge_block(
            shape.vertices().len(),
            shape.edge_indices().len(),
            block.profile_count,
        )?;
        prepared.push(block);
    }
    if prepared.is_empty() || !bounds.is_nonempty() || classifications != bounds.key_classifications
    {
        return None;
    }
    Some((prepared, bounds, classifications))
}
