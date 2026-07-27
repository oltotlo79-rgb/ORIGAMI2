use ori_domain::FaceId;

use super::{
    MaterialHingeGraphAudit,
    block_cut_decomposition::{ContractedBlockCutV1, prepare_contracted_block_cut_v1},
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

mod affine;
mod preparation;
mod proof;

use preparation::prepare_finite_half_turn_blocks_v1;
use proof::prove_prepared_finite_half_turn_blocks_v1;

const MAX_FINITE_HALF_TURN_CARRIERS_PER_BLOCK_V1: usize = 256;
const MAX_FINITE_HALF_TURN_GROUP_ORDER_PER_BLOCK_V1: usize = 256;
const MAX_FINITE_HALF_TURN_GROUP_ELEMENTS_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 6;
const MAX_FINITE_HALF_TURN_GROUP_PRODUCTS_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * MAX_FINITE_HALF_TURN_GROUP_ORDER_PER_BLOCK_V1;
const MAX_FINITE_HALF_TURN_EXACT_STORAGE_BITS_V1: usize = 128 * 1024 * 1024;
const MAX_FINITE_HALF_TURN_EXACT_WORK_BITS_V1: usize = 512 * 1024 * 1024;
const MAX_FINITE_HALF_TURN_COMPONENT_BITS_V1: u64 = 4096;
const MAX_FINITE_HALF_TURN_POTENTIAL_STORAGE_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_FINITE_HALF_TURN_DIRECTED_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
const MAX_FINITE_HALF_TURN_KEY_CLASSIFICATIONS_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FiniteHalfTurnBoundsV1 {
    cyclic_edges: usize,
    cyclic_blocks: usize,
    group_elements: usize,
    group_products: usize,
    exact_storage_bits: usize,
    exact_work_bits: usize,
    potential_storage: usize,
    directed_work: usize,
    key_classifications: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FiniteHalfTurnBlockChargeV1 {
    vertex_count: usize,
    edge_count: usize,
    carrier_count: usize,
    group_order: usize,
    group_products: usize,
    exact_storage_bits: usize,
    exact_work_bits: usize,
}

impl FiniteHalfTurnBoundsV1 {
    const fn empty() -> Self {
        Self {
            cyclic_edges: 0,
            cyclic_blocks: 0,
            group_elements: 0,
            group_products: 0,
            exact_storage_bits: 0,
            exact_work_bits: 0,
            potential_storage: 0,
            directed_work: 0,
            key_classifications: 0,
        }
    }

    fn charge_block(&mut self, charge: FiniteHalfTurnBlockChargeV1) -> Option<()> {
        let FiniteHalfTurnBlockChargeV1 {
            vertex_count,
            edge_count,
            carrier_count,
            group_order,
            group_products,
            exact_storage_bits,
            exact_work_bits,
        } = charge;
        if edge_count < 2
            || edge_count < vertex_count
            || !(1..=MAX_FINITE_HALF_TURN_CARRIERS_PER_BLOCK_V1).contains(&carrier_count)
            || carrier_count > edge_count
            || !(2..=MAX_FINITE_HALF_TURN_GROUP_ORDER_PER_BLOCK_V1).contains(&group_order)
            || carrier_count > group_order
            || group_products != group_order.checked_mul(carrier_count)?
            || exact_storage_bits == 0
            || exact_work_bits == 0
        {
            return None;
        }
        let directed = edge_count.checked_mul(2)?;
        self.cyclic_edges = self.cyclic_edges.checked_add(edge_count)?;
        self.cyclic_blocks = self.cyclic_blocks.checked_add(1)?;
        self.group_elements = self.group_elements.checked_add(group_order)?;
        self.group_products = self.group_products.checked_add(group_products)?;
        self.exact_storage_bits = self.exact_storage_bits.checked_add(exact_storage_bits)?;
        self.exact_work_bits = self.exact_work_bits.checked_add(exact_work_bits)?;
        self.potential_storage = self.potential_storage.checked_add(vertex_count)?;
        self.directed_work = self.directed_work.checked_add(directed)?;
        self.key_classifications = self.key_classifications.checked_add(edge_count)?;
        if self.cyclic_edges > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
            || self.cyclic_blocks > self.cyclic_edges
            || self.group_elements > MAX_FINITE_HALF_TURN_GROUP_ELEMENTS_V1
            || self.group_products > MAX_FINITE_HALF_TURN_GROUP_PRODUCTS_V1
            || self.exact_storage_bits > MAX_FINITE_HALF_TURN_EXACT_STORAGE_BITS_V1
            || self.exact_work_bits > MAX_FINITE_HALF_TURN_EXACT_WORK_BITS_V1
            || self.potential_storage > MAX_FINITE_HALF_TURN_POTENTIAL_STORAGE_V1
            || self.directed_work > MAX_FINITE_HALF_TURN_DIRECTED_WORK_V1
            || self.key_classifications > MAX_FINITE_HALF_TURN_KEY_CLASSIFICATIONS_V1
        {
            return None;
        }
        Some(())
    }

    fn is_nonempty(self) -> bool {
        self.cyclic_edges > 0
            && self.cyclic_blocks > 0
            && self.group_elements > 0
            && self.group_products > 0
            && self.exact_storage_bits > 0
            && self.exact_work_bits > 0
            && self.potential_storage > 0
            && self.directed_work == self.cyclic_edges.saturating_mul(2)
            && self.key_classifications == self.cyclic_edges
    }
}

#[cfg(test)]
fn bounded_finite_half_turn_counts_v1(
    shapes: &[(usize, usize, usize, usize, usize, usize, usize)],
) -> Option<FiniteHalfTurnBoundsV1> {
    let mut bounds = FiniteHalfTurnBoundsV1::empty();
    for &(vertices, edges, carriers, order, products, storage_bits, work_bits) in shapes {
        bounds.charge_block(FiniteHalfTurnBlockChargeV1 {
            vertex_count: vertices,
            edge_count: edges,
            carrier_count: carriers,
            group_order: order,
            group_products: products,
            exact_storage_bits: storage_bits,
            exact_work_bits: work_bits,
        })?;
    }
    bounds.is_nonempty().then_some(bounds)
}

fn prove_finite_half_turn_v1(decomposition: &ContractedBlockCutV1) -> bool {
    let Some((prepared, bounds, classifications)) =
        prepare_finite_half_turn_blocks_v1(decomposition)
    else {
        return false;
    };
    classifications == bounds.key_classifications
        && prove_prepared_finite_half_turn_blocks_v1(decomposition, &prepared, bounds)
}

/// Exact block-local closure in finite groups generated by half-turns.
///
/// Every cyclic edge is an exact constant 180-degree rotation. Its canonical
/// Plücker carrier is lifted to an exact rational affine half-turn, and the
/// carrier-generated subgroup is enumerated to exact closure under strict
/// order, product, component-bit, and total-storage bounds. A canonical group
/// potential proves every cycle in each retained finite block. Infinite
/// translation/rotation groups and finite groups above the fixed bound reject;
/// graph bridges remain relation-free after exact-zero contraction.
pub(super) fn block_cut_finite_half_turn_group_cycle_closure_premises_v1(
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
    prove_finite_half_turn_v1(&decomposition)
}

#[cfg(test)]
#[path = "block_cut_finite_half_turn_group_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_finite_half_turn_group_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_finite_half_turn_group_affine_tests.rs"]
mod affine_tests;

#[cfg(test)]
#[path = "block_cut_finite_half_turn_group_limits_tests.rs"]
mod limits_tests;
