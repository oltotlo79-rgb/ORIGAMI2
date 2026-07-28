use num_rational::BigRational;
use num_traits::{Signed, Zero};

use super::{
    super::{
        block_cut_decomposition::{
            ContractedBlockCutV1, ContractedProfileClassV1, TarjanBiconnectedBlockV1,
        },
        exact_generator_word::{
            CanonicalInfiniteLineV1, exact_perpendicular_intersection_v1,
            exact_plucker_components_v1,
        },
    },
    CardinalRotationBoundsV1,
};

const QUARTER_TURN_BITS_V1: u64 = 90.0_f64.to_bits();
const HALF_TURN_BITS_V1: u64 = 180.0_f64.to_bits();

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedCardinalEdgeV1 {
    pub(super) edge_index: usize,
    pub(super) axis: usize,
    pub(super) quarter_turns: i8,
}

#[derive(Debug)]
pub(super) struct PreparedCardinalBlockV1 {
    pub(super) block_index: usize,
    pub(super) carrier_count: usize,
    pub(super) labels: Vec<PreparedCardinalEdgeV1>,
}

fn oriented_frame_v1(
    carriers: &[CanonicalInfiniteLineV1],
    exact_relations: &mut usize,
) -> Option<Vec<i8>> {
    if !(1..=super::MAX_CARDINAL_CARRIERS_PER_BLOCK_V1).contains(&carriers.len()) {
        return None;
    }
    for first in 0..carriers.len() {
        for second in first + 1..carriers.len() {
            *exact_relations = (*exact_relations).checked_add(1)?;
            if !exact_perpendicular_intersection_v1(&carriers[first], &carriers[second]) {
                return None;
            }
        }
    }
    // With three nonzero pairwise-orthogonal directions, the three exact
    // pair intersections cannot differ. If the first two meet at p, the
    // third meets them at p+a*d0 and p+b*d1. Their difference is parallel to
    // d2, while exact dot products with d0 and d1 are respectively
    // a*|d0|^2 and -b*|d1|^2, so a=b=0. Thus every admitted local rotation
    // has the same fixed center.

    let mut orientation = Vec::new();
    orientation.try_reserve_exact(carriers.len()).ok()?;
    orientation.resize(carriers.len(), 1i8);
    if let [first, second, third] = carriers {
        let (first, _) = exact_plucker_components_v1(first)?;
        let (second, _) = exact_plucker_components_v1(second)?;
        let (third, _) = exact_plucker_components_v1(third)?;
        let cross = [
            &first[1] * &second[2] - &first[2] * &second[1],
            &first[2] * &second[0] - &first[0] * &second[2],
            &first[0] * &second[1] - &first[1] * &second[0],
        ];
        let alignment = cross
            .iter()
            .zip(&third)
            .map(|(cross, third)| cross * third)
            .sum::<BigRational>();
        if alignment.is_zero() {
            return None;
        }
        // e0=d0/|d0|, e1=d1/|d1|, e2=e0×e1. The third canonical
        // carrier may point along either e2 or -e2; only its signed
        // quarter-turn label changes. A half-turn remains sign-independent.
        orientation[2] = if alignment.is_positive() { 1 } else { -1 };
    }
    Some(orientation)
}

fn prepare_one_block_v1(
    decomposition: &ContractedBlockCutV1,
    block_index: usize,
    block: &TarjanBiconnectedBlockV1,
    classifications: &mut usize,
    exact_relations: &mut usize,
) -> Option<PreparedCardinalBlockV1> {
    let edge_indices = block.edge_indices();
    if !block.is_cyclic() || edge_indices.len() < 2 || edge_indices.len() < block.vertices().len() {
        return None;
    }
    let active = decomposition.active_edges();
    let mut carriers = Vec::<CanonicalInfiniteLineV1>::new();
    carriers.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        carriers.push(active.get(*edge_index)?.line().clone());
    }
    carriers.sort_unstable();
    carriers.dedup();
    let orientation = oriented_frame_v1(&carriers, exact_relations)?;

    let mut labels = Vec::new();
    labels.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        *classifications = (*classifications).checked_add(1)?;
        let axis = carriers.binary_search(edge.line()).ok()?;
        let quarter_turns = match edge.profile_class() {
            ContractedProfileClassV1::ConstantAngle(QUARTER_TURN_BITS_V1) => {
                edge.sign().checked_mul(*orientation.get(axis)?)?
            }
            ContractedProfileClassV1::ConstantAngle(HALF_TURN_BITS_V1) => 2,
            ContractedProfileClassV1::Nonconstant | ContractedProfileClassV1::ConstantAngle(_) => {
                return None;
            }
        };
        if !matches!(quarter_turns, -1 | 1 | 2) {
            return None;
        }
        labels.push(PreparedCardinalEdgeV1 {
            edge_index: *edge_index,
            axis,
            quarter_turns,
        });
    }
    Some(PreparedCardinalBlockV1 {
        block_index,
        carrier_count: carriers.len(),
        labels,
    })
}

pub(super) fn prepare_cardinal_rotation_blocks_v1(
    decomposition: &ContractedBlockCutV1,
) -> Option<(
    Vec<PreparedCardinalBlockV1>,
    CardinalRotationBoundsV1,
    usize,
    usize,
)> {
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(decomposition.blocks().len())
        .ok()?;
    let mut bounds = CardinalRotationBoundsV1::empty();
    let mut classifications = 0usize;
    let mut exact_relations = 0usize;
    for (block_index, block) in decomposition.blocks().iter().enumerate() {
        if block.is_bridge() {
            if block.edge_indices().len() != 1 || block.vertices().len() != 2 {
                return None;
            }
            continue;
        }
        let before_relations = exact_relations;
        let prepared_block = prepare_one_block_v1(
            decomposition,
            block_index,
            block,
            &mut classifications,
            &mut exact_relations,
        )?;
        bounds.charge_block(
            block.vertices().len(),
            block.edge_indices().len(),
            prepared_block.carrier_count,
            exact_relations.checked_sub(before_relations)?,
        )?;
        prepared.push(prepared_block);
    }
    if prepared.is_empty()
        || !bounds.is_nonempty()
        || classifications != bounds.key_classifications
        || exact_relations != bounds.exact_relations
    {
        return None;
    }
    Some((prepared, bounds, classifications, exact_relations))
}
