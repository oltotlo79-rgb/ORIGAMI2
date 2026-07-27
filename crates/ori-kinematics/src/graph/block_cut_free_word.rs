use std::collections::{HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};

use super::{
    MaterialHingeGraphAudit,
    block_cut_decomposition::{
        ContractedActiveEdgeV1, ContractedBlockCutV1, ContractedProfileClassV1,
        TarjanBiconnectedBlockV1, prepare_contracted_block_cut_v1,
    },
    exact_generator_word::{ExactGeneratorKeyV1, ExactGeneratorProfileV1, ReducedWordInternerV1},
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

const MAX_BLOCK_CUT_FREE_WORD_NODES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 3;
const MAX_BLOCK_CUT_FREE_WORD_DIRECTED_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
const MAX_BLOCK_CUT_FREE_WORD_KEY_CLASSIFICATIONS_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockCutFreeWordBoundsV1 {
    node_capacity: usize,
    directed_work: usize,
    key_classifications: usize,
}

fn bounded_block_cut_free_word_counts_v1(
    cyclic_edge_count: usize,
    cyclic_block_count: usize,
) -> Option<BlockCutFreeWordBoundsV1> {
    if cyclic_edge_count == 0
        || cyclic_edge_count > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
        || cyclic_block_count == 0
        || cyclic_block_count > cyclic_edge_count
    {
        return None;
    }
    let node_capacity = cyclic_edge_count
        .checked_mul(2)?
        .checked_add(cyclic_block_count)?;
    let directed_work = cyclic_edge_count.checked_mul(2)?;
    let key_classifications = cyclic_edge_count;
    if node_capacity > MAX_BLOCK_CUT_FREE_WORD_NODES_V1
        || directed_work > MAX_BLOCK_CUT_FREE_WORD_DIRECTED_WORK_V1
        || key_classifications > MAX_BLOCK_CUT_FREE_WORD_KEY_CLASSIFICATIONS_V1
    {
        return None;
    }
    Some(BlockCutFreeWordBoundsV1 {
        node_capacity,
        directed_work,
        key_classifications,
    })
}

fn bounded_block_cut_free_word_resources_v1(
    blocks: &[TarjanBiconnectedBlockV1],
) -> Option<BlockCutFreeWordBoundsV1> {
    let mut cyclic_edge_count = 0usize;
    let mut cyclic_block_count = 0usize;
    for block in blocks {
        if block.is_bridge() {
            if block.edge_indices().len() != 1 || block.vertices().len() != 2 {
                return None;
            }
            continue;
        }
        let edge_count = block.edge_indices().len();
        if !block.is_cyclic() || edge_count < 2 || edge_count < block.vertices().len() {
            return None;
        }
        cyclic_edge_count = cyclic_edge_count.checked_add(edge_count)?;
        cyclic_block_count = cyclic_block_count.checked_add(1)?;
    }
    bounded_block_cut_free_word_counts_v1(cyclic_edge_count, cyclic_block_count)
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

fn exact_generator_key_v1(
    edge: &ContractedActiveEdgeV1,
    moving: Option<&HashSet<EdgeId>>,
) -> Option<ExactGeneratorKeyV1> {
    let profile = match edge.profile_class() {
        ContractedProfileClassV1::Nonconstant => {
            if moving.is_none_or(|moving| !moving.contains(&edge.edge())) {
                return None;
            }
            ExactGeneratorProfileV1::CollectiveNonconstant
        }
        ContractedProfileClassV1::ConstantAngle(bits) => {
            ExactGeneratorProfileV1::ConstantAngle(bits)
        }
    };
    Some(ExactGeneratorKeyV1::new(edge.line().clone(), profile))
}

fn exact_block_free_word_potential_v1(
    active: &[ContractedActiveEdgeV1],
    block: &TarjanBiconnectedBlockV1,
    moving: Option<&HashSet<EdgeId>>,
    bounds: BlockCutFreeWordBoundsV1,
    directed_work: &mut usize,
    key_classifications: &mut usize,
) -> bool {
    if block.is_bridge()
        || !block.is_cyclic()
        || block.edge_indices().len() < block.vertices().len()
    {
        return false;
    }
    let edge_indices = block.edge_indices();
    let vertices = block.vertices();
    let mut edge_keys = Vec::new();
    if edge_keys.try_reserve_exact(edge_indices.len()).is_err() {
        return false;
    }
    for edge in edge_indices {
        *key_classifications = match (*key_classifications).checked_add(1) {
            Some(value) if value <= bounds.key_classifications => value,
            _ => return false,
        };
        let Some(key) = exact_generator_key_v1(&active[*edge], moving) else {
            return false;
        };
        edge_keys.push(key);
    }
    let mut generators = Vec::new();
    if generators.try_reserve_exact(edge_keys.len()).is_err() {
        return false;
    }
    generators.extend(edge_keys.iter().cloned());
    generators.sort_unstable();
    generators.dedup();
    if generators.is_empty() || generators.len() > i32::MAX as usize {
        return false;
    }

    let mut degrees = Vec::new();
    if degrees.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    degrees.resize(vertices.len(), 0usize);
    for edge in edge_indices {
        let edge = &active[*edge];
        let (Ok(left), Ok(right)) = (
            vertices.binary_search(&edge.left()),
            vertices.binary_search(&edge.right()),
        ) else {
            return false;
        };
        degrees[left] = match degrees[left].checked_add(1) {
            Some(value) => value,
            None => return false,
        };
        degrees[right] = match degrees[right].checked_add(1) {
            Some(value) => value,
            None => return false,
        };
    }
    let expected_work = match edge_indices.len().checked_mul(2) {
        Some(value) => value,
        None => return false,
    };
    if degrees
        .iter()
        .try_fold(0usize, |sum, degree| sum.checked_add(*degree))
        != Some(expected_work)
    {
        return false;
    }

    let mut adjacency = Vec::new();
    if adjacency.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    for degree in degrees {
        let mut neighbors = Vec::new();
        if neighbors.try_reserve_exact(degree).is_err() {
            return false;
        }
        adjacency.push(neighbors);
    }
    for (edge_index, key) in edge_indices.iter().zip(&edge_keys) {
        let edge = &active[*edge_index];
        let (Ok(left), Ok(right), Ok(generator)) = (
            vertices.binary_search(&edge.left()),
            vertices.binary_search(&edge.right()),
            generators.binary_search(key),
        ) else {
            return false;
        };
        let Some(one_based) = generator.checked_add(1) else {
            return false;
        };
        let Ok(identifier) = i32::try_from(one_based) else {
            return false;
        };
        let signed_generator = match identifier.checked_mul(i32::from(edge.sign())) {
            Some(value) if value != 0 => value,
            _ => return false,
        };
        adjacency[left].push((right, signed_generator, edge.edge()));
        let Some(reverse) = signed_generator.checked_neg() else {
            return false;
        };
        adjacency[right].push((left, reverse, edge.edge()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, generator, edge)| {
            (*vertex, *generator, edge.canonical_bytes())
        });
    }

    let Some(node_limit) = expected_work.checked_add(1) else {
        return false;
    };
    let Some(mut interner) = ReducedWordInternerV1::prepare(node_limit) else {
        return false;
    };
    let mut words = Vec::new();
    if words.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    words.resize(vertices.len(), None);
    words[0] = Some(0usize);
    let mut queue = VecDeque::new();
    if queue.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    queue.push_back(0usize);
    let work_before = *directed_work;
    while let Some(vertex) = queue.pop_front() {
        let Some(word) = words[vertex] else {
            return false;
        };
        for &(next, generator, _) in &adjacency[vertex] {
            *directed_work = match (*directed_work).checked_add(1) {
                Some(value) if value <= bounds.directed_work => value,
                _ => return false,
            };
            let Some(expected) = interner.append(word, generator) else {
                return false;
            };
            if let Some(existing) = words[next] {
                if existing != expected {
                    return false;
                }
            } else {
                words[next] = Some(expected);
                queue.push_back(next);
            }
        }
    }
    (*directed_work).checked_sub(work_before) == Some(expected_work)
        && words.into_iter().all(|word| word.is_some())
}

fn prove_block_cut_free_words_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> bool {
    let Some(bounds) = bounded_block_cut_free_word_resources_v1(decomposition.blocks()) else {
        return false;
    };
    let Some(moving) = collective_partition_v1(schedule, decomposition) else {
        return false;
    };
    let mut directed_work = 0usize;
    let mut key_classifications = 0usize;
    for block in decomposition.blocks() {
        if block.is_bridge() {
            // No profile, carrier equality, or word relation is read here.
            continue;
        }
        if !exact_block_free_word_potential_v1(
            decomposition.active_edges(),
            block,
            moving.as_ref(),
            bounds,
            &mut directed_work,
            &mut key_classifications,
        ) {
            return false;
        }
    }
    directed_work == bounds.directed_work
        && key_classifications == bounds.key_classifications
        && bounds.node_capacity <= MAX_BLOCK_CUT_FREE_WORD_NODES_V1
}

/// Exact closure for a block-cut graph whose cyclic blocks are independently
/// free-group coboundaries.
///
/// Exact-zero hinges are contracted first. A single-edge Tarjan biconnected
/// block is a bridge and contributes no closure relation. In every cyclic
/// block, each exact carrier/profile pair is one abstract free generator and
/// canonical face words must differ by precisely the signed edge generator.
/// Every closed walk decomposes into block-local cycles, so mapping those
/// words to actual rigid transforms proves closure without comparing,
/// commuting, or reordering transforms from different blocks or bridges.
pub(super) fn block_cut_free_word_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    if !tolerance.is_finite()
        || tolerance < 0.0
        || !schedule.matches_binding(geometry, audit, fixed_face)
    {
        return false;
    }
    let Some(decomposition) = prepare_contracted_block_cut_v1(geometry, audit, schedule) else {
        return false;
    };
    prove_block_cut_free_words_v1(schedule, &decomposition)
}

#[cfg(test)]
#[path = "block_cut_free_word_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_free_word_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_free_word_limits_tests.rs"]
mod limits_tests;
