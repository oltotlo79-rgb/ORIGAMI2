use ori_domain::FaceId;

use super::{
    MaterialHingeGraphAudit,
    block_cut_decomposition::{ContractedBlockCutV1, prepare_contracted_block_cut_v1},
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

mod normal_form;
mod preparation;
mod proof;

use preparation::prepare_orthogonal_half_turn_blocks_v1;
use proof::prove_prepared_orthogonal_half_turn_blocks_v1;

const MAX_ORTHOGONAL_FREE_PROFILES_PER_BLOCK_V1: usize = 64;
const MAX_ORTHOGONAL_STATE_WIDTH_V1: usize = MAX_ORTHOGONAL_FREE_PROFILES_PER_BLOCK_V1 + 2;
const MAX_ORTHOGONAL_STORAGE_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * MAX_ORTHOGONAL_STATE_WIDTH_V1;
const MAX_ORTHOGONAL_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2 * MAX_ORTHOGONAL_STATE_WIDTH_V1;
const MAX_ORTHOGONAL_DIRECTED_EDGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
const MAX_ORTHOGONAL_KEY_CLASSIFICATIONS_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_ORTHOGONAL_EXACT_RELATIONS_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrthogonalHalfTurnBoundsV1 {
    cyclic_edges: usize,
    cyclic_blocks: usize,
    storage: usize,
    work: usize,
    directed_edges: usize,
    key_classifications: usize,
    exact_relations: usize,
}

impl OrthogonalHalfTurnBoundsV1 {
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
        profile_count: usize,
        has_half_turn_coordinate: bool,
        secondary_count: usize,
        exact_relations: usize,
    ) -> Option<()> {
        if edge_count < 2
            || edge_count < vertex_count
            || profile_count > MAX_ORTHOGONAL_FREE_PROFILES_PER_BLOCK_V1
            || secondary_count > 2
        {
            return None;
        }
        let expected_relations = match secondary_count {
            0 => 0,
            1 => 1,
            2 if has_half_turn_coordinate => 3,
            _ => return None,
        };
        if exact_relations != expected_relations {
            return None;
        }
        let state_width = profile_count
            .checked_add(usize::from(has_half_turn_coordinate))?
            .checked_add(usize::from(secondary_count > 0))?;
        if state_width == 0 || state_width > MAX_ORTHOGONAL_STATE_WIDTH_V1 {
            return None;
        }
        let directed = edge_count.checked_mul(2)?;
        let storage = vertex_count.checked_mul(state_width)?;
        let work = directed.checked_mul(state_width)?;
        self.cyclic_edges = self.cyclic_edges.checked_add(edge_count)?;
        self.cyclic_blocks = self.cyclic_blocks.checked_add(1)?;
        self.storage = self.storage.checked_add(storage)?;
        self.work = self.work.checked_add(work)?;
        self.directed_edges = self.directed_edges.checked_add(directed)?;
        self.key_classifications = self.key_classifications.checked_add(edge_count)?;
        self.exact_relations = self.exact_relations.checked_add(exact_relations)?;
        if self.cyclic_edges > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
            || self.cyclic_blocks > self.cyclic_edges
            || self.storage > MAX_ORTHOGONAL_STORAGE_V1
            || self.work > MAX_ORTHOGONAL_WORK_V1
            || self.directed_edges > MAX_ORTHOGONAL_DIRECTED_EDGES_V1
            || self.key_classifications > MAX_ORTHOGONAL_KEY_CLASSIFICATIONS_V1
            || self.exact_relations > MAX_ORTHOGONAL_EXACT_RELATIONS_V1
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
fn bounded_orthogonal_half_turn_counts_v1(
    shapes: &[(usize, usize, usize, bool, usize, usize)],
) -> Option<OrthogonalHalfTurnBoundsV1> {
    let mut bounds = OrthogonalHalfTurnBoundsV1::empty();
    for &(vertices, edges, profiles, has_half_turn, secondaries, relations) in shapes {
        bounds.charge_block(
            vertices,
            edges,
            profiles,
            has_half_turn,
            secondaries,
            relations,
        )?;
    }
    bounds.is_nonempty().then_some(bounds)
}

fn prove_orthogonal_half_turn_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> bool {
    let Some((prepared, bounds, classifications, exact_relations)) =
        prepare_orthogonal_half_turn_blocks_v1(schedule, decomposition)
    else {
        return false;
    };
    classifications == bounds.key_classifications
        && exact_relations == bounds.exact_relations
        && prove_prepared_orthogonal_half_turn_blocks_v1(decomposition, &prepared, bounds)
}

/// Exact block-local closure for one primary rotation carrier extended by up
/// to two concurrent orthogonal half-turn carriers.
///
/// A block uses `(Z^P x C2(H)) semidirect C2(R)`: `R` inverts the free
/// primary coordinates and fixes the primary half-turn `H`. The second
/// secondary half-turn is exactly `HR`. Primary constant 180-degree edges
/// inhabit only `H`; a nonconstant profile that merely attains 180 degrees at
/// some parameter remains an independent conservative free coordinate.
/// Exact-zero contraction and the block-cut tree assemble the independently
/// proved cyclic blocks while bridges impose no relation.
pub(super) fn block_cut_orthogonal_half_turn_cycle_closure_premises_v1(
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
    prove_orthogonal_half_turn_v1(schedule, &decomposition)
}

#[cfg(test)]
#[path = "block_cut_orthogonal_half_turn_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_orthogonal_half_turn_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_orthogonal_half_turn_normal_form_tests.rs"]
mod normal_form_tests;

#[cfg(test)]
#[path = "block_cut_orthogonal_half_turn_limits_tests.rs"]
mod limits_tests;
