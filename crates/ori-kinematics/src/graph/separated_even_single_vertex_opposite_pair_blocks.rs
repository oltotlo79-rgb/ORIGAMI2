use std::collections::HashSet;

use ori_domain::FaceId;

use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphAudit, MaterialHingeGraphGeometry};

use super::{
    CanonicalEdgeBlockLimitsV1, even_single_vertex_opposite_pair_cycle_closure_premises_v1,
};

const SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1: usize = 8;
const SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_MIN_HINGES_V1: usize =
    SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1 * 4;
const SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_MAX_HINGES_V1: usize =
    SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1
        * super::MAX_EVEN_SINGLE_VERTEX_SECTORS_V1;

fn has_bounded_exact_eight_parent_cardinality_v1(
    closure_hinge_count: usize,
    face_count: usize,
    hinge_count: usize,
) -> bool {
    closure_hinge_count == SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1
        && (SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_MIN_HINGES_V1
            ..=SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_MAX_HINGES_V1)
            .contains(&hinge_count)
        && face_count.checked_add(
            SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1.saturating_sub(1),
        ) == Some(hinge_count)
}

/// Exact composition of bounded straight-fold cycles that share only one fixed
/// articulation face. Canonical edge-block decomposition proves complete,
/// duplicate-free coverage of the parent graph. Every restricted block then
/// satisfies the existing full-domain even single-vertex opposite-pair identity,
/// so the independently closing blocks compose around the unchanged fixed face.
/// Parent source/midpoint/target solves revalidate the decomposition binding and
/// orientation branch before an opaque closure certificate is issued. V1 is
/// deliberately exact-eight-only so existing two-through-seven-block closure
/// partitions and every token derived from them remain unchanged.
pub(super) fn separated_even_single_vertex_opposite_pair_blocks_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> Option<usize> {
    if !schedule.matches_binding(geometry, audit, fixed_face)
        || !tolerance.is_finite()
        || tolerance < 0.0
    {
        return None;
    }
    if !has_bounded_exact_eight_parent_cardinality_v1(
        audit.closure_hinges().len(),
        geometry.face_ids().len(),
        geometry.hinges().len(),
    ) {
        return None;
    }
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1,
                ..CanonicalEdgeBlockLimitsV1::default()
            },
        )
        .ok()?;
    let block_count = decomposition.blocks().len();
    if block_count != SEPARATED_EVEN_SINGLE_VERTEX_OPPOSITE_PAIR_BLOCK_COUNT_V1
        || decomposition.articulation_faces() != [fixed_face]
    {
        return None;
    }

    let mut covered_hinges = HashSet::new();
    let mut covered_faces = HashSet::from([fixed_face]);
    for block in decomposition.blocks() {
        if !block.geometry().face_ids().contains(&fixed_face)
            || block
                .geometry()
                .hinges()
                .iter()
                .any(|hinge| !covered_hinges.insert(hinge.edge()))
            || block
                .geometry()
                .face_ids()
                .iter()
                .copied()
                .filter(|face| *face != fixed_face)
                .any(|face| !covered_faces.insert(face))
        {
            return None;
        }
        let block_schedule = schedule
            .restrict_to_edge_block_with_fixed_face_v1(
                geometry,
                audit,
                block.geometry(),
                block.audit(),
                fixed_face,
            )
            .ok()?;
        if !even_single_vertex_opposite_pair_cycle_closure_premises_v1(
            block.geometry(),
            block.audit(),
            fixed_face,
            &block_schedule,
            tolerance,
        ) {
            return None;
        }
    }
    if covered_hinges.len() != geometry.hinges().len()
        || covered_faces.len() != geometry.face_ids().len()
        || ![0.0, 0.5, 1.0].into_iter().all(|u| {
            schedule.evaluate(u).is_some_and(|angles| {
                geometry
                    .solve_closed(audit, fixed_face, &angles, tolerance)
                    .is_ok()
            })
        })
    {
        return None;
    }
    Some(block_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_eight_parent_cardinality_is_bounded_before_decomposition() {
        for (faces, hinges) in [(25, 32), (41, 48), (185, 192)] {
            assert!(has_bounded_exact_eight_parent_cardinality_v1(
                8, faces, hinges
            ));
        }

        for (closure_hinges, faces, hinges) in [
            (7, 25, 32),
            (9, 25, 32),
            (8, 24, 31),
            (8, 186, 193),
            (8, 24, 32),
            (8, 26, 32),
            (8, usize::MAX, 192),
            (8, 185, usize::MAX),
        ] {
            assert!(!has_bounded_exact_eight_parent_cardinality_v1(
                closure_hinges,
                faces,
                hinges,
            ));
        }
    }
}
