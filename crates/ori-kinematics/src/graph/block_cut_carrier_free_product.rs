use ori_domain::FaceId;

use super::{
    MaterialHingeGraphAudit,
    block_cut_decomposition::{ContractedBlockCutV1, prepare_contracted_block_cut_v1},
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

mod normal_form;
mod preparation;
mod proof;

use preparation::prepare_carrier_free_product_blocks_v1;
use proof::prove_prepared_carrier_blocks_v1;

const MAX_CARRIER_FREE_PRODUCT_PROFILES_PER_BLOCK_V1: usize = 64;
const MAX_CARRIER_FREE_PRODUCT_NODES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 3;
const MAX_CARRIER_FREE_PRODUCT_DIRECTED_APPENDS_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
const MAX_CARRIER_FREE_PRODUCT_VECTOR_UNITS_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
    * 2
    * MAX_CARRIER_FREE_PRODUCT_PROFILES_PER_BLOCK_V1;
const MAX_CARRIER_FREE_PRODUCT_KEY_CLASSIFICATIONS_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CarrierFreeProductBoundsV1 {
    cyclic_edges: usize,
    cyclic_blocks: usize,
    node_capacity: usize,
    directed_appends: usize,
    vector_storage_limit: usize,
    vector_work: usize,
    key_classifications: usize,
}

impl CarrierFreeProductBoundsV1 {
    const fn empty() -> Self {
        Self {
            cyclic_edges: 0,
            cyclic_blocks: 0,
            node_capacity: 0,
            directed_appends: 0,
            vector_storage_limit: 0,
            vector_work: 0,
            key_classifications: 0,
        }
    }

    fn charge_block(&mut self, edge_count: usize, profile_count: usize) -> Option<()> {
        if edge_count < 2
            || profile_count == 0
            || profile_count > MAX_CARRIER_FREE_PRODUCT_PROFILES_PER_BLOCK_V1
        {
            return None;
        }
        let directed = edge_count.checked_mul(2)?;
        let nodes = directed.checked_add(1)?;
        let vector_units = directed.checked_mul(profile_count)?;
        self.cyclic_edges = self.cyclic_edges.checked_add(edge_count)?;
        self.cyclic_blocks = self.cyclic_blocks.checked_add(1)?;
        self.node_capacity = self.node_capacity.checked_add(nodes)?;
        self.directed_appends = self.directed_appends.checked_add(directed)?;
        self.vector_storage_limit = self.vector_storage_limit.checked_add(vector_units)?;
        self.vector_work = self.vector_work.checked_add(vector_units)?;
        self.key_classifications = self.key_classifications.checked_add(edge_count)?;
        if self.cyclic_edges > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
            || self.cyclic_blocks > self.cyclic_edges
            || self.node_capacity > MAX_CARRIER_FREE_PRODUCT_NODES_V1
            || self.directed_appends > MAX_CARRIER_FREE_PRODUCT_DIRECTED_APPENDS_V1
            || self.vector_storage_limit > MAX_CARRIER_FREE_PRODUCT_VECTOR_UNITS_V1
            || self.vector_work > MAX_CARRIER_FREE_PRODUCT_VECTOR_UNITS_V1
            || self.key_classifications > MAX_CARRIER_FREE_PRODUCT_KEY_CLASSIFICATIONS_V1
        {
            return None;
        }
        Some(())
    }

    fn is_nonempty(self) -> bool {
        let expected_directed = self.cyclic_edges.checked_mul(2);
        let expected_nodes =
            expected_directed.and_then(|directed| directed.checked_add(self.cyclic_blocks));
        let three_edges = self.cyclic_edges.checked_mul(3);
        self.cyclic_edges > 0
            && self.cyclic_blocks > 0
            && self.node_capacity > 0
            && self.directed_appends > 0
            && self.vector_work > 0
            && Some(self.directed_appends) == expected_directed
            && Some(self.node_capacity) == expected_nodes
            && three_edges.is_some_and(|limit| self.node_capacity <= limit)
            && self.vector_storage_limit == self.vector_work
            && self.key_classifications == self.cyclic_edges
    }
}

#[cfg(test)]
fn bounded_carrier_free_product_counts_v1(
    block_shapes: &[(usize, usize)],
) -> Option<CarrierFreeProductBoundsV1> {
    let mut bounds = CarrierFreeProductBoundsV1::empty();
    for &(edge_count, profile_count) in block_shapes {
        bounds.charge_block(edge_count, profile_count)?;
    }
    bounds.is_nonempty().then_some(bounds)
}

fn prove_carrier_free_product_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> bool {
    let Some((prepared, bounds, key_classifications)) =
        prepare_carrier_free_product_blocks_v1(schedule, decomposition)
    else {
        return false;
    };
    key_classifications == bounds.key_classifications
        && prove_prepared_carrier_blocks_v1(decomposition, &prepared, bounds)
}

/// Exact block-local closure in a free product of coaxial rotation factors.
///
/// Exact-zero hinges are contracted first and graph bridges impose no
/// relation. Inside every cyclic block, one exact infinite carrier is one
/// free-product factor. A nonzero integer vector over that block's exact
/// schedule profiles is a factor syllable. Adjacent syllables on the same
/// carrier are added with checked arithmetic and disappear at the zero vector.
/// A consistent reduced-normal-form potential makes every block cycle the
/// identity. Mapping each abelian carrier factor to its coaxial rigid
/// rotations and then assembling the block-cut tree proves native closure.
pub(super) fn block_cut_carrier_free_product_cycle_closure_premises_v1(
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
    prove_carrier_free_product_v1(schedule, &decomposition)
}

#[cfg(test)]
#[path = "block_cut_carrier_free_product_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "block_cut_carrier_free_product_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "block_cut_carrier_free_product_limits_tests.rs"]
mod limits_tests;
