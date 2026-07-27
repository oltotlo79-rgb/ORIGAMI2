use ori_domain::FaceId;

use super::{
    MaterialHingeGraphAudit,
    block_cut_decomposition::{ContractedBlockCutV1, prepare_contracted_block_cut_v1},
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

mod preparation;
mod proof;

use preparation::prepare_generalized_dihedral_blocks_v1;
use proof::prove_prepared_generalized_dihedral_blocks_v1;

const MAX_DIHEDRAL_PROFILES_PER_BLOCK_V1: usize = 64;
const MAX_DIHEDRAL_STORAGE_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * MAX_DIHEDRAL_PROFILES_PER_BLOCK_V1;
const MAX_DIHEDRAL_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2 * MAX_DIHEDRAL_PROFILES_PER_BLOCK_V1;
const MAX_DIHEDRAL_DIRECTED_EDGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
const MAX_DIHEDRAL_KEY_CLASSIFICATIONS_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GeneralizedDihedralBoundsV1 {
    cyclic_edges: usize,
    cyclic_blocks: usize,
    storage: usize,
    work: usize,
    directed_edges: usize,
    key_classifications: usize,
}

impl GeneralizedDihedralBoundsV1 {
    const fn empty() -> Self {
        Self {
            cyclic_edges: 0,
            cyclic_blocks: 0,
            storage: 0,
            work: 0,
            directed_edges: 0,
            key_classifications: 0,
        }
    }

    fn charge_block(
        &mut self,
        vertex_count: usize,
        edge_count: usize,
        profile_count: usize,
    ) -> Option<()> {
        if edge_count < 2
            || edge_count < vertex_count
            || profile_count == 0
            || profile_count > MAX_DIHEDRAL_PROFILES_PER_BLOCK_V1
        {
            return None;
        }
        let directed = edge_count.checked_mul(2)?;
        let storage = vertex_count.checked_mul(profile_count)?;
        let work = directed.checked_mul(profile_count)?;
        self.cyclic_edges = self.cyclic_edges.checked_add(edge_count)?;
        self.cyclic_blocks = self.cyclic_blocks.checked_add(1)?;
        self.storage = self.storage.checked_add(storage)?;
        self.work = self.work.checked_add(work)?;
        self.directed_edges = self.directed_edges.checked_add(directed)?;
        self.key_classifications = self.key_classifications.checked_add(edge_count)?;
        if self.cyclic_edges > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
            || self.cyclic_blocks > self.cyclic_edges
            || self.storage > MAX_DIHEDRAL_STORAGE_V1
            || self.work > MAX_DIHEDRAL_WORK_V1
            || self.directed_edges > MAX_DIHEDRAL_DIRECTED_EDGES_V1
            || self.key_classifications > MAX_DIHEDRAL_KEY_CLASSIFICATIONS_V1
        {
            return None;
        }
        Some(())
    }

    fn is_nonempty(self) -> bool {
        self.cyclic_edges > 0
            && self.cyclic_blocks > 0
            && self.storage > 0
            && self.work > 0
            && self.directed_edges == self.cyclic_edges.saturating_mul(2)
            && self.key_classifications == self.cyclic_edges
    }
}

#[cfg(test)]
fn bounded_generalized_dihedral_counts_v1(
    shapes: &[(usize, usize, usize)],
) -> Option<GeneralizedDihedralBoundsV1> {
    let mut bounds = GeneralizedDihedralBoundsV1::empty();
    for &(vertices, edges, profiles) in shapes {
        bounds.charge_block(vertices, edges, profiles)?;
    }
    bounds.is_nonempty().then_some(bounds)
}

fn prove_generalized_dihedral_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> bool {
    let Some((prepared, bounds, classifications)) =
        prepare_generalized_dihedral_blocks_v1(schedule, decomposition)
    else {
        return false;
    };
    classifications == bounds.key_classifications
        && prove_prepared_generalized_dihedral_blocks_v1(decomposition, &prepared, bounds)
}

/// Exact closure in block-local generalized dihedral groups.
///
/// A cyclic block has one primary carrier with exact profile lattice `Z^P`
/// and one exactly intersecting perpendicular carrier whose edges are exact
/// half-turns. In normal form `A(v)B^p`, right multiplication by `A(w)` adds
/// `(-1)^p w`, while right multiplication by `B` toggles parity. A stored
/// half-turn sign is irrelevant because rotations by `+180°` and `-180°`
/// about the same line are the same rigid transform. Exact group potentials
/// prove every block cycle, and the shared block-cut decomposition assembles
/// those identities while leaving bridges relation-free.
pub(super) fn block_cut_generalized_dihedral_cycle_closure_premises_v1(
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
    prove_generalized_dihedral_v1(schedule, &decomposition)
}

#[cfg(test)]
#[path = "block_cut_generalized_dihedral_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_generalized_dihedral_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_generalized_dihedral_limits_tests.rs"]
mod limits_tests;
