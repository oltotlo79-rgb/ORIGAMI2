use std::collections::VecDeque;

use super::{
    super::block_cut_decomposition::{ContractedBlockCutV1, TarjanBiconnectedBlockV1},
    GeneralizedDihedralBoundsV1,
    preparation::{PreparedDihedralBlockV1, PreparedDihedralEdgeKindV1},
};

#[derive(Debug, Clone, Copy)]
enum DirectedDihedralLabelV1 {
    Primary { profile: usize, sign: i8 },
    HalfTurn,
}

#[derive(Debug, Clone, Copy)]
struct DihedralExecutionV1 {
    storage: usize,
    work: usize,
    directed_edges: usize,
}

impl DihedralExecutionV1 {
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
    prepared: &PreparedDihedralBlockV1,
) -> Option<(usize, usize, usize)> {
    let edge_indices = block.edge_indices();
    let vertices = block.vertices();
    if !block.is_cyclic()
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
        if label.edge_index != *edge_index {
            return None;
        }
        let edge = decomposition.active_edges().get(*edge_index)?;
        let left = vertices.binary_search(&edge.left()).ok()?;
        let right = vertices.binary_search(&edge.right()).ok()?;
        let (forward, reverse) = match label.kind {
            PreparedDihedralEdgeKindV1::Primary { profile } => {
                if profile >= prepared.profile_count {
                    return None;
                }
                (
                    DirectedDihedralLabelV1::Primary {
                        profile,
                        sign: edge.sign(),
                    },
                    DirectedDihedralLabelV1::Primary {
                        profile,
                        sign: edge.sign().checked_neg()?,
                    },
                )
            }
            // A half-turn is its own inverse, so reversing storage or graph
            // traversal does not alter this abstract edge label.
            PreparedDihedralEdgeKindV1::HalfTurn => (
                DirectedDihedralLabelV1::HalfTurn,
                DirectedDihedralLabelV1::HalfTurn,
            ),
        };
        adjacency[left].push((right, forward, edge.edge()));
        adjacency[right].push((left, reverse, edge.edge()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, label, edge)| {
            let key = match label {
                DirectedDihedralLabelV1::Primary { profile, sign } => {
                    (0usize, *profile, i16::from(*sign))
                }
                DirectedDihedralLabelV1::HalfTurn => (1usize, 0usize, 0i16),
            };
            (*vertex, key, edge.canonical_bytes())
        });
    }

    let storage = vertices.len().checked_mul(prepared.profile_count)?;
    let mut potentials = Vec::new();
    potentials.try_reserve_exact(storage).ok()?;
    potentials.resize(storage, 0_i32);
    let mut parity = Vec::new();
    parity.try_reserve_exact(vertices.len()).ok()?;
    parity.resize(vertices.len(), false);
    let mut assigned = Vec::new();
    assigned.try_reserve_exact(vertices.len()).ok()?;
    assigned.resize(vertices.len(), false);
    assigned[0] = true;
    let mut queue = VecDeque::new();
    queue.try_reserve_exact(vertices.len()).ok()?;
    queue.push_back(0usize);
    let mut work = 0usize;
    let mut directed = 0usize;
    while let Some(vertex) = queue.pop_front() {
        let source_start = vertex.checked_mul(prepared.profile_count)?;
        for &(next, label, _) in &adjacency[vertex] {
            directed = directed.checked_add(1)?;
            let target_start = next.checked_mul(prepared.profile_count)?;
            let expected_parity = match label {
                DirectedDihedralLabelV1::Primary { .. } => parity[vertex],
                DirectedDihedralLabelV1::HalfTurn => !parity[vertex],
            };
            for coordinate in 0..prepared.profile_count {
                work = work.checked_add(1)?;
                let delta = match label {
                    DirectedDihedralLabelV1::Primary { profile, sign } if profile == coordinate => {
                        if parity[vertex] {
                            i32::from(sign).checked_neg()?
                        } else {
                            i32::from(sign)
                        }
                    }
                    _ => 0,
                };
                let expected = potentials[source_start + coordinate].checked_add(delta)?;
                if assigned[next] {
                    if potentials[target_start + coordinate] != expected {
                        return None;
                    }
                } else {
                    potentials[target_start + coordinate] = expected;
                }
            }
            if assigned[next] {
                if parity[next] != expected_parity {
                    return None;
                }
            } else {
                parity[next] = expected_parity;
                assigned[next] = true;
                queue.push_back(next);
            }
        }
    }
    let expected_work = directed_limit.checked_mul(prepared.profile_count)?;
    (assigned.into_iter().all(|value| value) && directed == directed_limit && work == expected_work)
        .then_some((storage, work, directed))
}

pub(super) fn prove_prepared_generalized_dihedral_blocks_v1(
    decomposition: &ContractedBlockCutV1,
    prepared: &[PreparedDihedralBlockV1],
    bounds: GeneralizedDihedralBoundsV1,
) -> bool {
    if prepared.len() != bounds.cyclic_blocks {
        return false;
    }
    let mut execution = DihedralExecutionV1::empty();
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
