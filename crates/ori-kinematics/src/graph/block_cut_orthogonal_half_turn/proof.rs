use std::collections::VecDeque;

use super::{
    super::block_cut_decomposition::{ContractedBlockCutV1, TarjanBiconnectedBlockV1},
    OrthogonalHalfTurnBoundsV1,
    normal_form::{DirectedOrthogonalLabelV1, OrthogonalNormalFormV1},
    preparation::{PreparedOrthogonalBlockV1, PreparedOrthogonalEdgeKindV1},
};

#[derive(Debug, Clone, Copy)]
struct OrthogonalHalfTurnExecutionV1 {
    storage: usize,
    work: usize,
    directed_edges: usize,
}

impl OrthogonalHalfTurnExecutionV1 {
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
    prepared: &PreparedOrthogonalBlockV1,
) -> Option<(usize, usize, usize)> {
    let edge_indices = block.edge_indices();
    let vertices = block.vertices();
    if !block.is_cyclic()
        || edge_indices.len() < vertices.len()
        || prepared.labels.len() != edge_indices.len()
        || prepared.schema.state_width()? == 0
    {
        return None;
    }

    let mut seen_profiles = Vec::new();
    seen_profiles
        .try_reserve_exact(prepared.schema.profile_count)
        .ok()?;
    seen_profiles.resize(prepared.schema.profile_count, false);
    let mut primary_half_turn_labels = 0usize;
    let mut reflection_labels = 0usize;
    let mut twisted_labels = 0usize;
    for label in &prepared.labels {
        match label.kind {
            PreparedOrthogonalEdgeKindV1::Primary { profile } => {
                *seen_profiles.get_mut(profile)? = true;
            }
            PreparedOrthogonalEdgeKindV1::PrimaryHalfTurn => {
                primary_half_turn_labels = primary_half_turn_labels.checked_add(1)?;
            }
            PreparedOrthogonalEdgeKindV1::Reflection => {
                reflection_labels = reflection_labels.checked_add(1)?;
            }
            PreparedOrthogonalEdgeKindV1::TwistedReflection => {
                twisted_labels = twisted_labels.checked_add(1)?;
            }
        }
    }
    if seen_profiles.contains(&false)
        || (primary_half_turn_labels > 0) != prepared.has_primary_half_turn_label
        || (reflection_labels > 0) != (prepared.secondary_count >= 1)
        || (twisted_labels > 0) != (prepared.secondary_count == 2)
        || prepared.schema.has_twisted_reflection != (prepared.secondary_count == 2)
        || prepared.schema.has_reflection != (prepared.secondary_count >= 1)
        || prepared.schema.has_half_turn
            != (prepared.has_primary_half_turn_label || prepared.secondary_count == 2)
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
        if label.edge_index != *edge_index {
            return None;
        }
        let edge = decomposition.active_edges().get(*edge_index)?;
        let left = vertices.binary_search(&edge.left()).ok()?;
        let right = vertices.binary_search(&edge.right()).ok()?;
        let forward = match label.kind {
            PreparedOrthogonalEdgeKindV1::Primary { profile } => {
                DirectedOrthogonalLabelV1::Primary {
                    profile,
                    sign: edge.sign(),
                }
            }
            PreparedOrthogonalEdgeKindV1::PrimaryHalfTurn => {
                DirectedOrthogonalLabelV1::PrimaryHalfTurn
            }
            PreparedOrthogonalEdgeKindV1::Reflection => DirectedOrthogonalLabelV1::Reflection,
            PreparedOrthogonalEdgeKindV1::TwistedReflection => {
                DirectedOrthogonalLabelV1::TwistedReflection
            }
        };
        let reverse = forward.inverse()?;
        adjacency[left].push((right, forward, edge.edge()));
        adjacency[right].push((left, reverse, edge.edge()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, label, edge)| {
            let key = match label {
                DirectedOrthogonalLabelV1::Primary { profile, sign } => {
                    (0usize, *profile, i16::from(*sign))
                }
                DirectedOrthogonalLabelV1::PrimaryHalfTurn => (1usize, 0usize, 0i16),
                DirectedOrthogonalLabelV1::Reflection => (2usize, 0usize, 0i16),
                DirectedOrthogonalLabelV1::TwistedReflection => (3usize, 0usize, 0i16),
            };
            (*vertex, key, edge.canonical_bytes())
        });
    }

    let state_width = prepared.schema.state_width()?;
    let storage = vertices.len().checked_mul(state_width)?;
    let mut potentials = Vec::new();
    potentials.try_reserve_exact(vertices.len()).ok()?;
    potentials.resize_with(vertices.len(), || None);
    potentials[0] = Some(OrthogonalNormalFormV1::identity(prepared.schema)?);
    let mut queue = VecDeque::new();
    queue.try_reserve_exact(vertices.len()).ok()?;
    queue.push_back(0usize);
    let mut work = 0usize;
    let mut directed = 0usize;
    while let Some(vertex) = queue.pop_front() {
        // Temporarily taking the source avoids an unaccounted state-vector
        // clone. Self-loops were rejected by the shared contracted graph.
        let source = potentials.get_mut(vertex)?.take()?;
        for &(next, label, _) in &adjacency[vertex] {
            directed = directed.checked_add(1)?;
            work = work.checked_add(state_width)?;
            let expected = source.right_product(prepared.schema, label)?;
            if let Some(existing) = &potentials[next] {
                if existing != &expected {
                    return None;
                }
            } else {
                potentials[next] = Some(expected);
                queue.push_back(next);
            }
        }
        potentials[vertex] = Some(source);
    }
    let expected_work = directed_limit.checked_mul(state_width)?;
    (potentials.into_iter().all(|state| state.is_some())
        && directed == directed_limit
        && work == expected_work)
        .then_some((storage, work, directed))
}

pub(super) fn prove_prepared_orthogonal_half_turn_blocks_v1(
    decomposition: &ContractedBlockCutV1,
    prepared: &[PreparedOrthogonalBlockV1],
    bounds: OrthogonalHalfTurnBoundsV1,
) -> bool {
    if prepared.len() != bounds.cyclic_blocks {
        return false;
    }
    let mut execution = OrthogonalHalfTurnExecutionV1::empty();
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
