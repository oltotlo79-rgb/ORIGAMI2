use std::collections::HashSet;

use ori_domain::EdgeId;

use super::{
    super::{
        block_cut_decomposition::{
            ContractedActiveEdgeV1, ContractedBlockCutV1, ContractedProfileClassV1,
            TarjanBiconnectedBlockV1,
        },
        exact_generator_word::{
            CanonicalInfiniteLineV1, ExactGeneratorProfileV1, exact_perpendicular_intersection_v1,
        },
    },
    OrthogonalHalfTurnBoundsV1,
    normal_form::OrthogonalNormalFormSchemaV1,
};
use crate::CanonicalCycleScheduleV1;

const HALF_TURN_BITS_V1: u64 = 180.0_f64.to_bits();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparedOrthogonalEdgeKindV1 {
    Primary { profile: usize },
    PrimaryHalfTurn,
    Reflection,
    TwistedReflection,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedOrthogonalEdgeV1 {
    pub(super) edge_index: usize,
    pub(super) kind: PreparedOrthogonalEdgeKindV1,
}

#[derive(Debug)]
pub(super) struct PreparedOrthogonalBlockV1 {
    pub(super) block_index: usize,
    pub(super) schema: OrthogonalNormalFormSchemaV1,
    pub(super) has_primary_half_turn_label: bool,
    pub(super) secondary_count: usize,
    pub(super) labels: Vec<PreparedOrthogonalEdgeV1>,
}

fn collective_partition_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> Option<Option<HashSet<EdgeId>>> {
    let active = decomposition.active_edges();
    let cyclic_nonconstant = decomposition.blocks().iter().any(|block| {
        block.is_cyclic()
            && block
                .edge_indices()
                .iter()
                .any(|edge| active[*edge].profile_class() == ContractedProfileClassV1::Nonconstant)
    });
    if !cyclic_nonconstant {
        return Some(None);
    }
    let edges = schedule.collective_profile_edges_v1()?;
    let mut moving = HashSet::new();
    moving.try_reserve(edges.len()).ok()?;
    if edges.is_empty() || edges.iter().any(|edge| !moving.insert(*edge)) {
        return None;
    }
    if active.iter().any(|edge| {
        moving.contains(&edge.edge())
            != (edge.profile_class() == ContractedProfileClassV1::Nonconstant)
    }) {
        return None;
    }
    Some(Some(moving))
}

fn is_half_turn_v1(edge: &ContractedActiveEdgeV1) -> bool {
    edge.profile_class() == ContractedProfileClassV1::ConstantAngle(HALF_TURN_BITS_V1)
}

fn non_half_profile_v1(
    edge: &ContractedActiveEdgeV1,
    moving: Option<&HashSet<EdgeId>>,
) -> Option<ExactGeneratorProfileV1> {
    match edge.profile_class() {
        ContractedProfileClassV1::Nonconstant => {
            if moving.is_none_or(|moving| !moving.contains(&edge.edge())) {
                return None;
            }
            Some(ExactGeneratorProfileV1::CollectiveNonconstant)
        }
        ContractedProfileClassV1::ConstantAngle(bits) if bits != HALF_TURN_BITS_V1 => {
            Some(ExactGeneratorProfileV1::ConstantAngle(bits))
        }
        ContractedProfileClassV1::ConstantAngle(_) => None,
    }
}

fn select_primary_v1(carrier_is_half_turn: &[bool]) -> Option<usize> {
    match carrier_is_half_turn {
        [_] => Some(0),
        [first, second] => match (*first, *second) {
            (false, true) => Some(0),
            (true, false) => Some(1),
            (true, true) => Some(0),
            (false, false) => None,
        },
        [first, second, third] => {
            let flags = [*first, *second, *third];
            let mut primary = None;
            for (index, half_turn) in flags.into_iter().enumerate() {
                if !half_turn && primary.replace(index).is_some() {
                    return None;
                }
            }
            Some(primary.unwrap_or(0))
        }
        _ => None,
    }
}

fn prepare_one_block_v1(
    active: &[ContractedActiveEdgeV1],
    block_index: usize,
    block: &TarjanBiconnectedBlockV1,
    moving: Option<&HashSet<EdgeId>>,
    classifications: &mut usize,
    exact_relations: &mut usize,
) -> Option<PreparedOrthogonalBlockV1> {
    let edge_indices = block.edge_indices();
    if !block.is_cyclic() || edge_indices.len() < 2 || edge_indices.len() < block.vertices().len() {
        return None;
    }

    let mut carriers = Vec::<CanonicalInfiniteLineV1>::new();
    carriers.try_reserve_exact(edge_indices.len()).ok()?;
    for edge_index in edge_indices {
        carriers.push(active.get(*edge_index)?.line().clone());
    }
    carriers.sort_unstable();
    carriers.dedup();
    if carriers.is_empty() || carriers.len() > 3 {
        return None;
    }

    let mut block_relations = 0usize;
    for first in 0..carriers.len() {
        for second in first + 1..carriers.len() {
            block_relations = block_relations.checked_add(1)?;
            if !exact_perpendicular_intersection_v1(&carriers[first], &carriers[second]) {
                return None;
            }
        }
    }
    // For three carriers the pairwise Pluecker predicates also prove one
    // common center, rather than three unrelated pair intersections. Let the
    // first two lines meet at p. If the third meets them at p+a*d0 and
    // p+b*d1, their difference is parallel to d2. Exact pairwise
    // perpendicularity makes its dot products with d0 and d1 equal to
    // a*|d0|^2 and -b*|d1|^2, hence a=b=0. All three intersections are p.
    *exact_relations = (*exact_relations).checked_add(block_relations)?;

    let mut carrier_is_half_turn = Vec::new();
    carrier_is_half_turn
        .try_reserve_exact(carriers.len())
        .ok()?;
    carrier_is_half_turn.resize(carriers.len(), true);
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        let carrier = carriers.binary_search(edge.line()).ok()?;
        carrier_is_half_turn[carrier] &= is_half_turn_v1(edge);
    }
    let primary = select_primary_v1(&carrier_is_half_turn)?;
    let mut secondaries = Vec::new();
    secondaries
        .try_reserve_exact(carriers.len().saturating_sub(1))
        .ok()?;
    for carrier in 0..carriers.len() {
        if carrier != primary {
            secondaries.push(carrier);
        }
    }
    if secondaries
        .iter()
        .any(|carrier| !carrier_is_half_turn[*carrier])
    {
        return None;
    }
    // Canonical order only names R and HR. A 180-degree rotation is
    // independent of line orientation and rotation sign. At the exact common
    // center, matrices in the three orthogonal carrier directions have two
    // diagonal -1 entries, so the primary half-turn times R is precisely the
    // remaining secondary half-turn, regardless of stored axis directions.

    let mut raw_profiles = Vec::new();
    raw_profiles.try_reserve_exact(edge_indices.len()).ok()?;
    let mut profiles = Vec::new();
    profiles.try_reserve_exact(edge_indices.len()).ok()?;
    let mut has_primary_half_turn_label = false;
    for edge_index in edge_indices {
        let edge = active.get(*edge_index)?;
        *classifications = (*classifications).checked_add(1)?;
        let carrier = carriers.binary_search(edge.line()).ok()?;
        if carrier == primary {
            if is_half_turn_v1(edge) {
                has_primary_half_turn_label = true;
                raw_profiles.push(None);
            } else {
                let profile = non_half_profile_v1(edge, moving)?;
                profiles.push(profile);
                raw_profiles.push(Some(profile));
            }
        } else {
            raw_profiles.push(None);
        }
    }
    profiles.sort_unstable();
    profiles.dedup();
    if profiles.len() > super::MAX_ORTHOGONAL_FREE_PROFILES_PER_BLOCK_V1 {
        return None;
    }

    let has_half_turn_coordinate = has_primary_half_turn_label || secondaries.len() == 2;
    let schema = OrthogonalNormalFormSchemaV1 {
        profile_count: profiles.len(),
        has_half_turn: has_half_turn_coordinate,
        has_reflection: !secondaries.is_empty(),
        has_twisted_reflection: secondaries.len() == 2,
    };
    if schema.state_width()? == 0 {
        return None;
    }

    let mut labels = Vec::new();
    labels.try_reserve_exact(edge_indices.len()).ok()?;
    for (edge_index, raw_profile) in edge_indices.iter().zip(raw_profiles) {
        let edge = active.get(*edge_index)?;
        let carrier = carriers.binary_search(edge.line()).ok()?;
        let kind = if carrier == primary {
            match raw_profile {
                Some(profile) => PreparedOrthogonalEdgeKindV1::Primary {
                    profile: profiles.binary_search(&profile).ok()?,
                },
                None if is_half_turn_v1(edge) => PreparedOrthogonalEdgeKindV1::PrimaryHalfTurn,
                None => return None,
            }
        } else if secondaries.first() == Some(&carrier) {
            if !is_half_turn_v1(edge) {
                return None;
            }
            PreparedOrthogonalEdgeKindV1::Reflection
        } else if secondaries.get(1) == Some(&carrier) {
            if !is_half_turn_v1(edge) {
                return None;
            }
            PreparedOrthogonalEdgeKindV1::TwistedReflection
        } else {
            return None;
        };
        labels.push(PreparedOrthogonalEdgeV1 {
            edge_index: *edge_index,
            kind,
        });
    }
    Some(PreparedOrthogonalBlockV1 {
        block_index,
        schema,
        has_primary_half_turn_label,
        secondary_count: secondaries.len(),
        labels,
    })
}

pub(super) fn prepare_orthogonal_half_turn_blocks_v1(
    schedule: &CanonicalCycleScheduleV1,
    decomposition: &ContractedBlockCutV1,
) -> Option<(
    Vec<PreparedOrthogonalBlockV1>,
    OrthogonalHalfTurnBoundsV1,
    usize,
    usize,
)> {
    let moving = collective_partition_v1(schedule, decomposition)?;
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(decomposition.blocks().len())
        .ok()?;
    let mut bounds = OrthogonalHalfTurnBoundsV1::empty();
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
            decomposition.active_edges(),
            block_index,
            block,
            moving.as_ref(),
            &mut classifications,
            &mut exact_relations,
        )?;
        let block_relations = exact_relations.checked_sub(before_relations)?;
        bounds.charge_block(
            block.vertices().len(),
            block.edge_indices().len(),
            prepared_block.schema.profile_count,
            prepared_block.schema.has_half_turn,
            prepared_block.secondary_count,
            block_relations,
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
