use super::{
    super::{
        block_cut_decomposition::{
            ContractedBlockCutV1, ContractedProfileClassV1, TarjanBiconnectedBlockV1,
        },
        exact_generator_word::CanonicalInfiniteLineV1,
    },
    FiniteHalfTurnBlockChargeV1, FiniteHalfTurnBoundsV1,
    affine::{FiniteHalfTurnGroupV1, enumerate_finite_half_turn_group_v1},
};

const HALF_TURN_BITS_V1: u64 = 180.0_f64.to_bits();

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedFiniteHalfTurnEdgeV1 {
    pub(super) edge_index: usize,
    pub(super) carrier: usize,
}

#[derive(Debug)]
pub(super) struct PreparedFiniteHalfTurnBlockV1 {
    pub(super) block_index: usize,
    pub(super) group: FiniteHalfTurnGroupV1,
    pub(super) labels: Vec<PreparedFiniteHalfTurnEdgeV1>,
}

fn prepare_one_block_v1(
    decomposition: &ContractedBlockCutV1,
    block_index: usize,
    block: &TarjanBiconnectedBlockV1,
    bounds: FiniteHalfTurnBoundsV1,
    classifications: &mut usize,
) -> Option<PreparedFiniteHalfTurnBlockV1> {
    let edge_indices = block.edge_indices();
    if !block.is_cyclic() || edge_indices.len() < 2 || edge_indices.len() < block.vertices().len() {
        return None;
    }
    let active = decomposition.active_edges();
    let mut carriers = Vec::<CanonicalInfiniteLineV1>::new();
    carriers.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        *classifications = (*classifications).checked_add(1)?;
        if edge.profile_class() != ContractedProfileClassV1::ConstantAngle(HALF_TURN_BITS_V1) {
            return None;
        }
        carriers.push(edge.line().clone());
    }
    carriers.sort_unstable();
    carriers.dedup();
    if carriers.is_empty() || carriers.len() > super::MAX_FINITE_HALF_TURN_CARRIERS_PER_BLOCK_V1 {
        return None;
    }

    let remaining_order = super::MAX_FINITE_HALF_TURN_GROUP_ELEMENTS_V1
        .checked_sub(bounds.group_elements)?
        .min(super::MAX_FINITE_HALF_TURN_GROUP_ORDER_PER_BLOCK_V1);
    let remaining_products =
        super::MAX_FINITE_HALF_TURN_GROUP_PRODUCTS_V1.checked_sub(bounds.group_products)?;
    let remaining_exact_bits =
        super::MAX_FINITE_HALF_TURN_EXACT_STORAGE_BITS_V1.checked_sub(bounds.exact_storage_bits)?;
    let remaining_exact_work_bits =
        super::MAX_FINITE_HALF_TURN_EXACT_WORK_BITS_V1.checked_sub(bounds.exact_work_bits)?;
    let group = enumerate_finite_half_turn_group_v1(
        &carriers,
        remaining_order,
        remaining_products,
        remaining_exact_bits,
        remaining_exact_work_bits,
        super::MAX_FINITE_HALF_TURN_COMPONENT_BITS_V1,
    )?;

    let mut labels = Vec::new();
    labels.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        labels.push(PreparedFiniteHalfTurnEdgeV1 {
            edge_index: *edge_index,
            carrier: carriers.binary_search(edge.line()).ok()?,
        });
    }
    Some(PreparedFiniteHalfTurnBlockV1 {
        block_index,
        group,
        labels,
    })
}

pub(super) fn prepare_finite_half_turn_blocks_v1(
    decomposition: &ContractedBlockCutV1,
) -> Option<(
    Vec<PreparedFiniteHalfTurnBlockV1>,
    FiniteHalfTurnBoundsV1,
    usize,
)> {
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(decomposition.blocks().len())
        .ok()?;
    let mut bounds = FiniteHalfTurnBoundsV1::empty();
    let mut classifications = 0usize;
    for (block_index, block) in decomposition.blocks().iter().enumerate() {
        if block.is_bridge() {
            if block.edge_indices().len() != 1 || block.vertices().len() != 2 {
                return None;
            }
            continue;
        }
        let prepared_block = prepare_one_block_v1(
            decomposition,
            block_index,
            block,
            bounds,
            &mut classifications,
        )?;
        bounds.charge_block(FiniteHalfTurnBlockChargeV1 {
            vertex_count: block.vertices().len(),
            edge_count: block.edge_indices().len(),
            carrier_count: prepared_block.group.carrier_count,
            group_order: prepared_block.group.order,
            group_products: prepared_block.group.products,
            exact_storage_bits: prepared_block.group.exact_storage_bits,
            exact_work_bits: prepared_block.group.exact_work_bits,
        })?;
        prepared.push(prepared_block);
    }
    if prepared.is_empty() || !bounds.is_nonempty() || classifications != bounds.key_classifications
    {
        return None;
    }
    Some((prepared, bounds, classifications))
}
