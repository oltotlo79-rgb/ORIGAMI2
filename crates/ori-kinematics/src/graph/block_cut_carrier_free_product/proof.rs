use std::collections::VecDeque;

use super::{
    super::block_cut_decomposition::{ContractedBlockCutV1, TarjanBiconnectedBlockV1},
    CarrierFreeProductBoundsV1,
    normal_form::{CarrierFreeProductInternerV1, NormalFormUsageV1},
    preparation::PreparedCarrierBlockV1,
};

#[derive(Debug, Clone, Copy)]
struct CarrierFreeProductExecutionV1 {
    directed_appends: usize,
    vector_work: usize,
    retained_vector_storage: usize,
    node_storage: usize,
}

impl CarrierFreeProductExecutionV1 {
    const fn empty() -> Self {
        Self {
            directed_appends: 0,
            vector_work: 0,
            retained_vector_storage: 0,
            node_storage: 0,
        }
    }

    fn charge(&mut self, usage: NormalFormUsageV1) -> Option<()> {
        self.directed_appends = self.directed_appends.checked_add(usage.directed_appends)?;
        self.vector_work = self.vector_work.checked_add(usage.vector_work)?;
        self.retained_vector_storage = self
            .retained_vector_storage
            .checked_add(usage.retained_vector_storage)?;
        self.node_storage = self.node_storage.checked_add(usage.node_storage)?;
        Some(())
    }
}

fn prove_one_block_v1(
    decomposition: &ContractedBlockCutV1,
    block: &TarjanBiconnectedBlockV1,
    prepared: &PreparedCarrierBlockV1,
) -> Option<NormalFormUsageV1> {
    let edge_indices = block.edge_indices();
    let vertices = block.vertices();
    if !block.is_cyclic()
        || edge_indices.len() < 2
        || edge_indices.len() < vertices.len()
        || prepared.labels.len() != edge_indices.len()
        || prepared.profile_count == 0
    {
        return None;
    }

    let mut degrees = Vec::new();
    degrees.try_reserve_exact(vertices.len()).ok()?;
    degrees.resize(vertices.len(), 0usize);
    for edge_index in edge_indices {
        let edge = decomposition.active_edges().get(*edge_index)?;
        let left = vertices.binary_search(&edge.left()).ok()?;
        let right = vertices.binary_search(&edge.right()).ok()?;
        degrees[left] = degrees[left].checked_add(1)?;
        degrees[right] = degrees[right].checked_add(1)?;
    }
    let directed_limit = edge_indices.len().checked_mul(2)?;
    if degrees
        .iter()
        .try_fold(0usize, |sum, degree| sum.checked_add(*degree))?
        != directed_limit
    {
        return None;
    }

    let mut adjacency = Vec::new();
    adjacency.try_reserve_exact(vertices.len()).ok()?;
    for degree in degrees {
        let mut neighbors = Vec::new();
        neighbors.try_reserve_exact(degree).ok()?;
        adjacency.push(neighbors);
    }
    for (edge_index, label) in edge_indices.iter().zip(&prepared.labels) {
        if label.edge_index != *edge_index || label.profile >= prepared.profile_count {
            return None;
        }
        let edge = decomposition.active_edges().get(*edge_index)?;
        let left = vertices.binary_search(&edge.left()).ok()?;
        let right = vertices.binary_search(&edge.right()).ok()?;
        adjacency[left].push((
            right,
            label.carrier,
            label.profile,
            edge.sign(),
            edge.edge(),
        ));
        adjacency[right].push((
            left,
            label.carrier,
            label.profile,
            edge.sign().checked_neg()?,
            edge.edge(),
        ));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, carrier, profile, sign, edge)| {
            (*vertex, *carrier, *profile, *sign, edge.canonical_bytes())
        });
    }

    let node_limit = directed_limit.checked_add(1)?;
    let mut interner = CarrierFreeProductInternerV1::prepare(prepared.profile_count, node_limit)?;
    let mut words = Vec::new();
    words.try_reserve_exact(vertices.len()).ok()?;
    words.resize(vertices.len(), None);
    words[0] = Some(0usize);
    let mut queue = VecDeque::new();
    queue.try_reserve_exact(vertices.len()).ok()?;
    queue.push_back(0usize);
    while let Some(vertex) = queue.pop_front() {
        let word = words[vertex]?;
        for &(next, carrier, profile, sign, _) in &adjacency[vertex] {
            let expected = interner.append(word, carrier, profile, sign)?;
            if let Some(existing) = words[next] {
                if existing != expected {
                    return None;
                }
            } else {
                words[next] = Some(expected);
                queue.push_back(next);
            }
        }
    }
    let usage = interner.usage();
    let expected_vector_work = directed_limit.checked_mul(prepared.profile_count)?;
    (words.into_iter().all(|word| word.is_some())
        && interner.invariant_holds()
        && usage.directed_appends == directed_limit
        && usage.vector_work == expected_vector_work
        && usage.retained_vector_storage <= expected_vector_work
        && usage.node_storage <= node_limit)
        .then_some(usage)
}

pub(super) fn prove_prepared_carrier_blocks_v1(
    decomposition: &ContractedBlockCutV1,
    prepared: &[PreparedCarrierBlockV1],
    bounds: CarrierFreeProductBoundsV1,
) -> bool {
    if prepared.len() != bounds.cyclic_blocks {
        return false;
    }
    let mut execution = CarrierFreeProductExecutionV1::empty();
    for block in prepared {
        let Some(block_shape) = decomposition.blocks().get(block.block_index) else {
            return false;
        };
        let Some(usage) = prove_one_block_v1(decomposition, block_shape, block) else {
            return false;
        };
        if execution.charge(usage).is_none()
            || execution.directed_appends > bounds.directed_appends
            || execution.vector_work > bounds.vector_work
            || execution.retained_vector_storage > bounds.vector_storage_limit
            || execution.node_storage > bounds.node_capacity
        {
            return false;
        }
    }
    execution.directed_appends == bounds.directed_appends
        && execution.vector_work == bounds.vector_work
        && execution.retained_vector_storage <= bounds.vector_storage_limit
        && execution.node_storage <= bounds.node_capacity
}
