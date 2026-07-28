use std::collections::VecDeque;

use super::{
    super::block_cut_decomposition::{ContractedBlockCutV1, TarjanBiconnectedBlockV1},
    CardinalRotationBoundsV1,
    normal_form::CardinalRotationV1,
    preparation::PreparedCardinalBlockV1,
};

#[derive(Debug, Clone, Copy)]
struct CardinalRotationExecutionV1 {
    storage: usize,
    work: usize,
    directed_edges: usize,
}

impl CardinalRotationExecutionV1 {
    const fn empty() -> Self {
        Self {
            storage: 0,
            work: 0,
            directed_edges: 0,
        }
    }

    fn charge(&mut self, storage: usize, work: usize, directed_edges: usize) -> Option<()> {
        self.storage = self.storage.checked_add(storage)?;
        self.work = self.work.checked_add(work)?;
        self.directed_edges = self.directed_edges.checked_add(directed_edges)?;
        Some(())
    }
}

fn prove_one_block_v1(
    decomposition: &ContractedBlockCutV1,
    block: &TarjanBiconnectedBlockV1,
    prepared: &PreparedCardinalBlockV1,
) -> Option<(usize, usize, usize)> {
    let edge_indices = block.edge_indices();
    let vertices = block.vertices();
    if !block.is_cyclic()
        || edge_indices.len() < vertices.len()
        || prepared.labels.len() != edge_indices.len()
        || !(1..=super::MAX_CARDINAL_CARRIERS_PER_BLOCK_V1).contains(&prepared.carrier_count)
    {
        return None;
    }
    let mut seen_axes = Vec::new();
    seen_axes.try_reserve_exact(prepared.carrier_count).ok()?;
    seen_axes.resize(prepared.carrier_count, false);
    for label in &prepared.labels {
        *seen_axes.get_mut(label.axis)? = true;
        if !matches!(label.quarter_turns, -1 | 1 | 2)
            || CardinalRotationV1::quarter_turn(label.axis, label.quarter_turns).is_none()
        {
            return None;
        }
    }
    if seen_axes.contains(&false) {
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
        if label.edge_index != *edge_index || label.axis >= prepared.carrier_count {
            return None;
        }
        let edge = decomposition.active_edges().get(*edge_index)?;
        let left = vertices.binary_search(&edge.left()).ok()?;
        let right = vertices.binary_search(&edge.right()).ok()?;
        let forward = CardinalRotationV1::quarter_turn(label.axis, label.quarter_turns)?;
        let reverse_turns = match label.quarter_turns {
            -1 => 1,
            1 => -1,
            2 => 2,
            _ => return None,
        };
        let reverse = CardinalRotationV1::quarter_turn(label.axis, reverse_turns)?;
        if forward.inverse()? != reverse {
            return None;
        }
        adjacency[left].push((right, label.axis, label.quarter_turns, forward, edge.edge()));
        adjacency[right].push((left, label.axis, reverse_turns, reverse, edge.edge()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, axis, turns, _, edge)| {
            (*vertex, *axis, i16::from(*turns), edge.canonical_bytes())
        });
    }

    let storage = vertices.len().checked_mul(super::CARDINAL_STATE_UNITS_V1)?;
    let mut potentials = Vec::new();
    potentials.try_reserve_exact(vertices.len()).ok()?;
    potentials.resize(vertices.len(), None);
    potentials[0] = Some(CardinalRotationV1::identity());
    let mut queue = VecDeque::new();
    queue.try_reserve_exact(vertices.len()).ok()?;
    queue.push_back(0usize);
    let mut work = 0usize;
    let mut directed = 0usize;
    // Every local rigid transform is T_p * M * T_-p for the one exact block
    // center p proved during preparation. Consequently equality of these
    // proper signed-permutation potentials includes translation equality:
    // an identity matrix cycle is the complete affine identity, not merely
    // a rotation-only check.
    while let Some(vertex) = queue.pop_front() {
        let source = potentials.get(vertex)?.as_ref().copied()?;
        for &(next, _, _, rotation, _) in &adjacency[vertex] {
            directed = directed.checked_add(1)?;
            work = work.checked_add(super::CARDINAL_PRODUCT_WORK_V1)?;
            let expected = source.right_product(rotation)?;
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
    let expected_work = directed_limit.checked_mul(super::CARDINAL_PRODUCT_WORK_V1)?;
    (potentials.into_iter().all(|state| state.is_some())
        && directed == directed_limit
        && work == expected_work)
        .then_some((storage, work, directed))
}

pub(super) fn prove_prepared_cardinal_rotation_blocks_v1(
    decomposition: &ContractedBlockCutV1,
    prepared: &[PreparedCardinalBlockV1],
    bounds: CardinalRotationBoundsV1,
) -> bool {
    if prepared.len() != bounds.cyclic_blocks {
        return false;
    }
    let mut execution = CardinalRotationExecutionV1::empty();
    for block in prepared {
        let Some(shape) = decomposition.blocks().get(block.block_index) else {
            return false;
        };
        let Some((storage, work, directed)) = prove_one_block_v1(decomposition, shape, block)
        else {
            return false;
        };
        if execution.charge(storage, work, directed).is_none()
            || execution.storage > bounds.storage
            || execution.work > bounds.work
            || execution.directed_edges > bounds.directed_edges
        {
            return false;
        }
    }
    execution.storage == bounds.storage
        && execution.work == bounds.work
        && execution.directed_edges == bounds.directed_edges
}
