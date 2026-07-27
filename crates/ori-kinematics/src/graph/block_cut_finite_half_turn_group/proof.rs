use std::collections::VecDeque;

use super::{
    super::block_cut_decomposition::{ContractedBlockCutV1, TarjanBiconnectedBlockV1},
    FiniteHalfTurnBoundsV1,
    preparation::PreparedFiniteHalfTurnBlockV1,
};

#[derive(Debug, Clone, Copy)]
struct FiniteHalfTurnExecutionV1 {
    potential_storage: usize,
    directed_work: usize,
}

impl FiniteHalfTurnExecutionV1 {
    const fn empty() -> Self {
        Self {
            potential_storage: 0,
            directed_work: 0,
        }
    }

    fn charge(&mut self, potential_storage: usize, directed_work: usize) -> Option<()> {
        self.potential_storage = self.potential_storage.checked_add(potential_storage)?;
        self.directed_work = self.directed_work.checked_add(directed_work)?;
        Some(())
    }
}

fn prove_one_block_v1(
    decomposition: &ContractedBlockCutV1,
    block: &TarjanBiconnectedBlockV1,
    prepared: &PreparedFiniteHalfTurnBlockV1,
) -> Option<(usize, usize)> {
    let edge_indices = block.edge_indices();
    let vertices = block.vertices();
    if !block.is_cyclic()
        || edge_indices.len() < vertices.len()
        || prepared.labels.len() != edge_indices.len()
        || prepared.group.order < 2
        || prepared.group.carrier_count == 0
        || prepared.group.transitions.len()
            != prepared
                .group
                .order
                .checked_mul(prepared.group.carrier_count)?
    {
        return None;
    }
    let mut seen_carriers = Vec::new();
    seen_carriers
        .try_reserve_exact(prepared.group.carrier_count)
        .ok()?;
    seen_carriers.resize(prepared.group.carrier_count, false);
    for label in &prepared.labels {
        *seen_carriers.get_mut(label.carrier)? = true;
    }
    if seen_carriers.contains(&false)
        || prepared
            .group
            .transitions
            .iter()
            .any(|state| *state >= prepared.group.order)
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
        if label.edge_index != *edge_index || label.carrier >= prepared.group.carrier_count {
            return None;
        }
        let edge = decomposition.active_edges().get(*edge_index)?;
        let left = vertices.binary_search(&edge.left()).ok()?;
        let right = vertices.binary_search(&edge.right()).ok()?;
        // A half-turn is its own inverse. Stored axis direction, fold
        // assignment, and graph traversal direction therefore share one
        // exact carrier transition.
        adjacency[left].push((right, label.carrier, edge.edge()));
        adjacency[right].push((left, label.carrier, edge.edge()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, carrier, edge)| {
            (*vertex, *carrier, edge.canonical_bytes())
        });
    }

    let mut potentials = Vec::new();
    potentials.try_reserve_exact(vertices.len()).ok()?;
    potentials.resize(vertices.len(), None);
    potentials[0] = Some(0usize);
    let mut queue = VecDeque::new();
    queue.try_reserve_exact(vertices.len()).ok()?;
    queue.push_back(0usize);
    let mut work = 0usize;
    while let Some(vertex) = queue.pop_front() {
        let state = potentials.get(vertex)?.as_ref().copied()?;
        for &(next, carrier, _) in &adjacency[vertex] {
            work = work.checked_add(1)?;
            let expected = prepared.group.transition(state, carrier)?;
            if let Some(existing) = potentials[next] {
                if existing != expected {
                    return None;
                }
            } else {
                potentials[next] = Some(expected);
                queue.push_back(next);
            }
        }
    }
    (work == directed_limit && potentials.into_iter().all(|state| state.is_some()))
        .then_some((vertices.len(), work))
}

pub(super) fn prove_prepared_finite_half_turn_blocks_v1(
    decomposition: &ContractedBlockCutV1,
    prepared: &[PreparedFiniteHalfTurnBlockV1],
    bounds: FiniteHalfTurnBoundsV1,
) -> bool {
    if prepared.len() != bounds.cyclic_blocks {
        return false;
    }
    let mut execution = FiniteHalfTurnExecutionV1::empty();
    for block in prepared {
        let Some(shape) = decomposition.blocks().get(block.block_index) else {
            return false;
        };
        let Some((storage, work)) = prove_one_block_v1(decomposition, shape, block) else {
            return false;
        };
        if execution.charge(storage, work).is_none()
            || execution.potential_storage > bounds.potential_storage
            || execution.directed_work > bounds.directed_work
        {
            return false;
        }
    }
    execution.potential_storage == bounds.potential_storage
        && execution.directed_work == bounds.directed_work
}
