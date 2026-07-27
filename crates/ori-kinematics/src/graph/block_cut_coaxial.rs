use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};

use super::{
    MaterialHingeGraphAudit,
    exact_generator_word::{
        AuthenticatedGraphV1, CanonicalInfiniteLineV1, authenticate_graph_v1,
        exact_generator_line_v1,
    },
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

mod decomposition;

use decomposition::{
    block_vertices_v1, decompose_active_edge_blocks_v1, prepare_active_quotient_v1,
};

const MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1: usize = 64;
const MAX_BLOCK_CUT_COAXIAL_STORAGE_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1;
const MAX_BLOCK_CUT_COAXIAL_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2 * MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1;
const MAX_BLOCK_CUT_COAXIAL_CLASSIFICATION_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveScheduleClassV1 {
    CollectiveNonconstant,
    ConstantAngle(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum BlockProfileKeyV1 {
    CollectiveNonconstant,
    ConstantAngle(u64),
}

#[derive(Debug, Clone)]
struct ActiveQuotientEdgeV1 {
    geometry_index: usize,
    edge: EdgeId,
    left: usize,
    right: usize,
    schedule_class: ActiveScheduleClassV1,
    line: CanonicalInfiniteLineV1,
    sign: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockCutResourceTotalsV1 {
    storage: usize,
    work: usize,
    classification_work: usize,
}

impl BlockCutResourceTotalsV1 {
    const fn empty() -> Self {
        Self {
            storage: 0,
            work: 0,
            classification_work: 0,
        }
    }

    fn charge_block(
        &mut self,
        vertex_count: usize,
        edge_count: usize,
        profile_count: usize,
    ) -> Option<()> {
        if profile_count == 0
            || profile_count > MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1
            || edge_count < vertex_count
        {
            return None;
        }
        let storage = vertex_count.checked_mul(profile_count)?;
        let work = edge_count.checked_mul(2)?.checked_mul(profile_count)?;
        self.storage = self.storage.checked_add(storage)?;
        self.work = self.work.checked_add(work)?;
        if self.storage > MAX_BLOCK_CUT_COAXIAL_STORAGE_V1
            || self.work > MAX_BLOCK_CUT_COAXIAL_WORK_V1
        {
            return None;
        }
        Some(())
    }

    fn charge_profile_comparison(&mut self) -> Option<()> {
        self.classification_work = self.classification_work.checked_add(1)?;
        (self.classification_work <= MAX_BLOCK_CUT_COAXIAL_CLASSIFICATION_WORK_V1).then_some(())
    }
}

fn insert_profile_v1(
    profiles: &mut Vec<BlockProfileKeyV1>,
    profile: BlockProfileKeyV1,
    totals: &mut BlockCutResourceTotalsV1,
) -> Option<()> {
    for existing in profiles.iter() {
        totals.charge_profile_comparison()?;
        if *existing == profile {
            return Some(());
        }
    }
    if profiles.len() >= MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1 {
        return None;
    }
    profiles.push(profile);
    Some(())
}

fn exact_block_lattice_potential_v1(
    active: &[ActiveQuotientEdgeV1],
    block: &[usize],
    vertices: &[usize],
    profiles: &[BlockProfileKeyV1],
    profile_by_edge: &HashMap<EdgeId, BlockProfileKeyV1>,
    totals: &mut BlockCutResourceTotalsV1,
) -> bool {
    if block.len() < vertices.len()
        || block.len() < 2
        || profiles.is_empty()
        || profiles.len() > MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1
        || profiles.windows(2).any(|pair| pair[0] >= pair[1])
        || totals
            .charge_block(vertices.len(), block.len(), profiles.len())
            .is_none()
    {
        return false;
    }

    let mut degrees = Vec::new();
    if degrees.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    degrees.resize(vertices.len(), 0usize);
    for edge in block {
        let edge = &active[*edge];
        let (Ok(left), Ok(right)) = (
            vertices.binary_search(&edge.left),
            vertices.binary_search(&edge.right),
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
    let expected_entries = match block.len().checked_mul(2) {
        Some(value) => value,
        None => return false,
    };
    if degrees
        .iter()
        .try_fold(0usize, |sum, degree| sum.checked_add(*degree))
        != Some(expected_entries)
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
    for edge_index in block {
        let edge = &active[*edge_index];
        let (Ok(left), Ok(right), Some(profile)) = (
            vertices.binary_search(&edge.left),
            vertices.binary_search(&edge.right),
            profile_by_edge.get(&edge.edge).copied(),
        ) else {
            return false;
        };
        let Ok(profile) = profiles.binary_search(&profile) else {
            return false;
        };
        adjacency[left].push((right, profile, edge.sign, edge.edge));
        let Some(reverse_sign) = edge.sign.checked_neg() else {
            return false;
        };
        adjacency[right].push((left, profile, reverse_sign, edge.edge));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(vertex, profile, sign, edge)| {
            (*vertex, *profile, *sign, edge.canonical_bytes())
        });
    }

    let Some(storage) = vertices.len().checked_mul(profiles.len()) else {
        return false;
    };
    let mut potentials = Vec::new();
    if potentials.try_reserve_exact(storage).is_err() {
        return false;
    }
    potentials.resize(storage, 0_i32);
    let mut assigned = Vec::new();
    if assigned.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    assigned.resize(vertices.len(), false);
    assigned[0] = true;
    let mut queue = VecDeque::new();
    if queue.try_reserve_exact(vertices.len()).is_err() {
        return false;
    }
    queue.push_back(0usize);
    let width = profiles.len();
    let mut work = 0usize;
    while let Some(vertex) = queue.pop_front() {
        let Some(source_start) = vertex.checked_mul(width) else {
            return false;
        };
        for &(next, profile, sign, _) in &adjacency[vertex] {
            work = match work.checked_add(width) {
                Some(value) => value,
                None => return false,
            };
            let Some(target_start) = next.checked_mul(width) else {
                return false;
            };
            if !assigned[next] {
                for coordinate in 0..width {
                    potentials[target_start + coordinate] = potentials[source_start + coordinate];
                }
                potentials[target_start + profile] =
                    match potentials[target_start + profile].checked_add(i32::from(sign)) {
                        Some(value) => value,
                        None => return false,
                    };
                assigned[next] = true;
                queue.push_back(next);
            } else {
                for coordinate in 0..width {
                    let delta = if coordinate == profile {
                        i32::from(sign)
                    } else {
                        0
                    };
                    let expected = match potentials[source_start + coordinate].checked_add(delta) {
                        Some(value) => value,
                        None => return false,
                    };
                    if potentials[target_start + coordinate] != expected {
                        return false;
                    }
                }
            }
        }
    }
    expected_entries
        .checked_mul(width)
        .is_some_and(|expected_work| work == expected_work)
        && assigned.into_iter().all(|value| value)
}

fn prove_cyclic_blocks_v1(
    geometry: &MaterialHingeGraphGeometry,
    schedule: &CanonicalCycleScheduleV1,
    active: &[ActiveQuotientEdgeV1],
    blocks: &[Vec<usize>],
) -> bool {
    let mut cyclic_edges = Vec::new();
    if cyclic_edges.try_reserve_exact(active.len()).is_err() {
        return false;
    }
    cyclic_edges.resize(active.len(), false);
    let mut cyclic_block_count = 0usize;
    for block in blocks {
        let Some(vertices) = block_vertices_v1(active, block) else {
            return false;
        };
        if block.len() == 1 {
            if vertices.len() != 2 {
                return false;
            }
            continue;
        }
        // In an undirected edge-biconnected block, E >= V is the explicit
        // indication that the block carries cycle constraints. E < V would
        // signal a malformed decomposition rather than a bridge.
        if block.len() < vertices.len() {
            return false;
        }
        cyclic_block_count = match cyclic_block_count.checked_add(1) {
            Some(value) => value,
            None => return false,
        };
        for edge in block {
            if cyclic_edges[*edge] {
                return false;
            }
            cyclic_edges[*edge] = true;
        }
    }
    if cyclic_block_count == 0 {
        return false;
    }

    let cyclic_nonconstant = active.iter().enumerate().any(|(edge, value)| {
        cyclic_edges[edge] && value.schedule_class == ActiveScheduleClassV1::CollectiveNonconstant
    });
    let moving = if cyclic_nonconstant {
        let Some(edges) = schedule.collective_profile_edges_v1() else {
            return false;
        };
        let mut moving = HashSet::new();
        if moving.try_reserve(edges.len()).is_err()
            || edges.is_empty()
            || edges.iter().any(|edge| !moving.insert(*edge))
        {
            return false;
        }
        if active.iter().any(|edge| {
            moving.contains(&edge.edge)
                != (edge.schedule_class == ActiveScheduleClassV1::CollectiveNonconstant)
        }) {
            return false;
        }
        Some(moving)
    } else {
        None
    };

    let mut totals = BlockCutResourceTotalsV1::empty();
    let mut profile_by_edge = HashMap::new();
    if profile_by_edge.try_reserve(active.len()).is_err() {
        return false;
    }
    for (edge_index, edge) in active.iter().enumerate() {
        if !cyclic_edges[edge_index] {
            // The line was validated while constructing the quotient, but no
            // carrier or profile equality is read for a bridge block.
            continue;
        }
        let profile = match edge.schedule_class {
            ActiveScheduleClassV1::CollectiveNonconstant => {
                if moving
                    .as_ref()
                    .is_none_or(|moving| !moving.contains(&edge.edge))
                {
                    return false;
                }
                BlockProfileKeyV1::CollectiveNonconstant
            }
            ActiveScheduleClassV1::ConstantAngle(bits) => BlockProfileKeyV1::ConstantAngle(bits),
        };
        if profile_by_edge.insert(edge.edge, profile).is_some() {
            return false;
        }
    }

    for block in blocks {
        if block.len() == 1 {
            continue;
        }
        let Some(vertices) = block_vertices_v1(active, block) else {
            return false;
        };
        if block.len() < vertices.len() {
            return false;
        }
        let mut profiles = Vec::new();
        if profiles
            .try_reserve_exact(MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1)
            .is_err()
        {
            return false;
        }
        let mut reference_line: Option<&CanonicalInfiniteLineV1> = None;
        for edge in block {
            let edge = &active[*edge];
            if reference_line.is_some_and(|reference| reference != &edge.line) {
                return false;
            }
            reference_line = Some(&edge.line);
            let Some(profile) = profile_by_edge.get(&edge.edge).copied() else {
                return false;
            };
            if insert_profile_v1(&mut profiles, profile, &mut totals).is_none() {
                return false;
            }
            if geometry.hinges().get(edge.geometry_index).is_none() {
                return false;
            }
        }
        profiles.sort_unstable();
        if reference_line.is_none()
            || !exact_block_lattice_potential_v1(
                active,
                block,
                &vertices,
                &profiles,
                &profile_by_edge,
                &mut totals,
            )
        {
            return false;
        }
    }
    totals.storage > 0 && totals.work > 0
}

/// Exact block-cut closure after contracting native exact-zero hinges.
///
/// Every active single-edge block is a graph bridge and carries no closure
/// relation. Every cyclic edge-biconnected block independently proves an
/// integer profile coboundary over one exact infinite carrier. Closed walks
/// decompose into cycles inside individual blocks, and the block-cut incidence
/// graph is a tree, so the resulting block-local group identities assemble
/// without commuting, comparing, or reordering transforms from different
/// blocks. Exact-zero contraction, not sampling or a tolerance, identifies
/// articulation face classes.
pub(super) fn block_cut_coaxial_cycle_closure_premises_v1(
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
    let Some(graph) = authenticate_graph_v1(geometry, audit) else {
        return false;
    };
    let Some((component_count, active)) = prepare_active_quotient_v1(geometry, &graph, schedule)
    else {
        return false;
    };
    let Some(blocks) = decompose_active_edge_blocks_v1(component_count, &active) else {
        return false;
    };
    prove_cyclic_blocks_v1(geometry, schedule, &active, &blocks)
}

#[cfg(test)]
#[path = "block_cut_coaxial_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_coaxial_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_coaxial_limits_tests.rs"]
mod limits_tests;
