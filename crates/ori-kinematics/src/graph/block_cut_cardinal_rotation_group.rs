use ori_domain::FaceId;

use super::{
    MaterialHingeGraphAudit,
    block_cut_decomposition::{ContractedBlockCutV1, prepare_contracted_block_cut_v1},
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

mod normal_form;
mod preparation;
mod proof;

use preparation::prepare_cardinal_rotation_blocks_v1;
use proof::prove_prepared_cardinal_rotation_blocks_v1;

const MAX_CARDINAL_CARRIERS_PER_BLOCK_V1: usize = 3;
const CARDINAL_STATE_UNITS_V1: usize = 9;
const CARDINAL_PRODUCT_WORK_V1: usize = 27;
const MAX_CARDINAL_STORAGE_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * CARDINAL_STATE_UNITS_V1;
const MAX_CARDINAL_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2 * CARDINAL_PRODUCT_WORK_V1;
const MAX_CARDINAL_DIRECTED_EDGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
const MAX_CARDINAL_KEY_CLASSIFICATIONS_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_CARDINAL_EXACT_RELATIONS_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CardinalRotationBoundsV1 {
    cyclic_edges: usize,
    cyclic_blocks: usize,
    storage: usize,
    work: usize,
    directed_edges: usize,
    key_classifications: usize,
    exact_relations: usize,
}

impl CardinalRotationBoundsV1 {
    const fn empty() -> Self {
        Self {
            cyclic_edges: 0,
            cyclic_blocks: 0,
            storage: 0,
            work: 0,
            directed_edges: 0,
            key_classifications: 0,
            exact_relations: 0,
        }
    }

    fn charge_block(
        &mut self,
        vertex_count: usize,
        edge_count: usize,
        carrier_count: usize,
        exact_relations: usize,
    ) -> Option<()> {
        if vertex_count == 0
            || edge_count < 2
            || edge_count < vertex_count
            || !(1..=MAX_CARDINAL_CARRIERS_PER_BLOCK_V1).contains(&carrier_count)
            || carrier_count > edge_count
            || exact_relations != carrier_count.checked_mul(carrier_count.checked_sub(1)?)? / 2
            || exact_relations > edge_count
        {
            return None;
        }
        let directed = edge_count.checked_mul(2)?;
        let storage = vertex_count.checked_mul(CARDINAL_STATE_UNITS_V1)?;
        let work = directed.checked_mul(CARDINAL_PRODUCT_WORK_V1)?;
        self.cyclic_edges = self.cyclic_edges.checked_add(edge_count)?;
        self.cyclic_blocks = self.cyclic_blocks.checked_add(1)?;
        self.storage = self.storage.checked_add(storage)?;
        self.work = self.work.checked_add(work)?;
        self.directed_edges = self.directed_edges.checked_add(directed)?;
        self.key_classifications = self.key_classifications.checked_add(edge_count)?;
        self.exact_relations = self.exact_relations.checked_add(exact_relations)?;
        if self.cyclic_edges > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
            || self.cyclic_blocks > self.cyclic_edges
            || self.storage > MAX_CARDINAL_STORAGE_V1
            || self.work > MAX_CARDINAL_WORK_V1
            || self.directed_edges > MAX_CARDINAL_DIRECTED_EDGES_V1
            || self.key_classifications > MAX_CARDINAL_KEY_CLASSIFICATIONS_V1
            || self.exact_relations > MAX_CARDINAL_EXACT_RELATIONS_V1
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
fn bounded_cardinal_rotation_counts_v1(
    shapes: &[(usize, usize, usize, usize)],
) -> Option<CardinalRotationBoundsV1> {
    let mut bounds = CardinalRotationBoundsV1::empty();
    for &(vertices, edges, carriers, relations) in shapes {
        bounds.charge_block(vertices, edges, carriers, relations)?;
    }
    bounds.is_nonempty().then_some(bounds)
}

fn prove_cardinal_rotation_v1(decomposition: &ContractedBlockCutV1) -> bool {
    let Some((prepared, bounds, classifications, exact_relations)) =
        prepare_cardinal_rotation_blocks_v1(decomposition)
    else {
        return false;
    };
    classifications == bounds.key_classifications
        && exact_relations == bounds.exact_relations
        && prove_prepared_cardinal_rotation_blocks_v1(decomposition, &prepared, bounds)
}

/// Exact block-local closure in the orientation-preserving cardinal group.
///
/// Each cyclic block uses one to three exact carrier lines through one common
/// center. Distinct carriers are pairwise perpendicular, and every cyclic
/// edge is an exact constant quarter-turn or half-turn. The first two
/// canonical carriers define an oriented orthogonal frame; a possible third
/// carrier is aligned exactly against their cross-product. Signed-permutation
/// matrices then represent the corresponding rigid rotations without
/// transcendental or tolerance-based arithmetic. A canonical group potential
/// proves every block cycle, while exact-zero contraction and the block-cut
/// tree leave bridges relation-free.
pub(super) fn block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
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
    prove_cardinal_rotation_v1(&decomposition)
}

#[cfg(test)]
#[path = "block_cut_cardinal_rotation_group_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_cardinal_rotation_group_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_cardinal_rotation_group_normal_form_tests.rs"]
mod normal_form_tests;

#[cfg(test)]
#[path = "block_cut_cardinal_rotation_group_limits_tests.rs"]
mod limits_tests;
