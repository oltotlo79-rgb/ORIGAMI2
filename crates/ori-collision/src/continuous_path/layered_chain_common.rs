//! Bounded, proof-neutral building blocks shared by the narrow layered-chain
//! continuous certificates.
//!
//! This module does not issue a certificate.  It only validates finite chain
//! structure, one-moving-hinge schedules, caller limits, and strict outward
//! interval gaps while preserving cooperative stop causes.

use std::collections::{HashMap, HashSet};

use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::{
    MaterialTreeDyadicFaceIntervalRegistryV1, MaterialTreeDyadicIntervalLimitsV1,
    MaterialTreeKinematicsModel, OutwardIntervalV1,
};

use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1,
    NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1, StaticCollisionLimits,
};

pub(super) const MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1: usize = 64;
pub(super) const MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1: usize = 100_000;
pub(super) const MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1: usize = 10_000;
pub(super) const LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1: StaticCollisionLimits =
    StaticCollisionLimits {
        max_faces: 10_001,
        max_unordered_face_pairs: NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1,
        max_boundary_vertices_per_face: 4_096,
        max_total_boundary_vertices: 50_000,
        max_triangles_per_face: 4_094,
        max_total_triangles: 50_000,
        max_triangulation_work_per_face: 100_000_000,
        max_total_triangulation_work: 500_000_000,
        max_registry_authentication_work: 10_000_000,
        max_triangle_pairs_per_face_pair: 250_000,
        max_total_triangle_pairs: 1_000_000,
        max_boundary_relation_work_per_face_pair: 10_000_000,
        max_total_boundary_relation_work: 40_000_000,
        max_rational_input_bits: 4_096,
        max_total_rational_input_storage_bits: 536_870_912,
        max_total_rational_retained_clone_bits: 4_294_967_296,
        max_rational_operations: 1_000_000_000,
        max_rational_intermediate_bits: 32_768,
        max_rational_gcd_fallback_calls: 1_000_000,
        max_rational_gcd_fallback_input_bits: 8_589_934_592,
        max_rational_allocations: 1_000_000,
        max_rational_allocation_bits: 65_536,
        max_total_rational_allocation_bits: 1_073_741_824,
        max_rational_output_bits: 32_768,
        max_total_rational_output_bits: 1_073_741_824,
        max_shared_hinge_boundary_diagnostics: 10_000,
        max_shared_hinge_solid_diagnostics: 120,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayeredChainResourceErrorV1 {
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayeredChainIntervalErrorV1 {
    ResourceLimit,
    Cancelled,
    DeadlineExceeded,
    IntervalUnavailable,
    IntervalOverlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayeredChainIntervalCheckpointPhaseV1 {
    Pair,
    Axis,
    Vertex,
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LayeredChainPairPartitionV1 {
    pub(super) direct_pairs: Vec<[FaceId; 2]>,
    pub(super) nonadjacent_pairs: Vec<[FaceId; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LayeredChainDirectHingeV1 {
    pub(super) edge: EdgeId,
    pub(super) pair: [FaceId; 2],
    pub(super) moving: bool,
}

pub(super) type LayeredChainFaceOutwardIntervalsV1 = [(VertexId, [OutwardIntervalV1; 3])];
pub(super) type LayeredChainNonadjacentIntervalPairV1<'a> = (
    &'a LayeredChainFaceOutwardIntervalsV1,
    &'a LayeredChainFaceOutwardIntervalsV1,
);

pub(super) fn canonical_pair_v1(first: FaceId, second: FaceId) -> [FaceId; 2] {
    if first.canonical_bytes() < second.canonical_bytes() {
        [first, second]
    } else {
        [second, first]
    }
}

pub(super) fn pair_key_v1(pair: &[FaceId; 2]) -> ([u8; 16], [u8; 16]) {
    (pair[0].canonical_bytes(), pair[1].canonical_bytes())
}

pub(super) fn bounded_face_boundaries_v1(
    model: &MaterialTreeKinematicsModel,
    maximum: usize,
) -> bool {
    model.face_ids().iter().all(|face| {
        model
            .face_boundary(*face)
            .is_some_and(|boundary| boundary.vertices().len() <= maximum)
    })
}

pub(super) fn layered_leaf_count_v1(depth: u8, maximum: usize) -> Option<usize> {
    let count = 1_usize.checked_shl(u32::from(depth))?;
    (depth <= 52 && count <= maximum && maximum <= super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1)
        .then_some(count)
}

pub(super) fn layered_continuous_resource_limits_within_hard_caps_v1(
    interval: MaterialTreeDyadicIntervalLimitsV1,
    required_faces: usize,
    required_hinges: usize,
    static_collision: StaticCollisionLimits,
) -> bool {
    if interval.max_faces != required_faces
        || interval.max_hinges != required_hinges
        || interval.max_vertices > MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1
        || interval.max_interval_work > MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1
        || interval.max_total_interval_work > MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1
    {
        return false;
    }
    let hard = LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1;
    [
        (static_collision.max_faces, hard.max_faces),
        (
            static_collision.max_unordered_face_pairs,
            hard.max_unordered_face_pairs,
        ),
        (
            static_collision.max_boundary_vertices_per_face,
            hard.max_boundary_vertices_per_face,
        ),
        (
            static_collision.max_total_boundary_vertices,
            hard.max_total_boundary_vertices,
        ),
        (
            static_collision.max_triangles_per_face,
            hard.max_triangles_per_face,
        ),
        (
            static_collision.max_total_triangles,
            hard.max_total_triangles,
        ),
        (
            static_collision.max_triangulation_work_per_face,
            hard.max_triangulation_work_per_face,
        ),
        (
            static_collision.max_total_triangulation_work,
            hard.max_total_triangulation_work,
        ),
        (
            static_collision.max_registry_authentication_work,
            hard.max_registry_authentication_work,
        ),
        (
            static_collision.max_triangle_pairs_per_face_pair,
            hard.max_triangle_pairs_per_face_pair,
        ),
        (
            static_collision.max_total_triangle_pairs,
            hard.max_total_triangle_pairs,
        ),
        (
            static_collision.max_boundary_relation_work_per_face_pair,
            hard.max_boundary_relation_work_per_face_pair,
        ),
        (
            static_collision.max_total_boundary_relation_work,
            hard.max_total_boundary_relation_work,
        ),
        (
            static_collision.max_rational_input_bits,
            hard.max_rational_input_bits,
        ),
        (
            static_collision.max_total_rational_input_storage_bits,
            hard.max_total_rational_input_storage_bits,
        ),
        (
            static_collision.max_total_rational_retained_clone_bits,
            hard.max_total_rational_retained_clone_bits,
        ),
        (
            static_collision.max_rational_operations,
            hard.max_rational_operations,
        ),
        (
            static_collision.max_rational_intermediate_bits,
            hard.max_rational_intermediate_bits,
        ),
        (
            static_collision.max_rational_gcd_fallback_calls,
            hard.max_rational_gcd_fallback_calls,
        ),
        (
            static_collision.max_rational_gcd_fallback_input_bits,
            hard.max_rational_gcd_fallback_input_bits,
        ),
        (
            static_collision.max_rational_allocations,
            hard.max_rational_allocations,
        ),
        (
            static_collision.max_rational_allocation_bits,
            hard.max_rational_allocation_bits,
        ),
        (
            static_collision.max_total_rational_allocation_bits,
            hard.max_total_rational_allocation_bits,
        ),
        (
            static_collision.max_rational_output_bits,
            hard.max_rational_output_bits,
        ),
        (
            static_collision.max_total_rational_output_bits,
            hard.max_total_rational_output_bits,
        ),
        (
            static_collision.max_shared_hinge_boundary_diagnostics,
            hard.max_shared_hinge_boundary_diagnostics,
        ),
        (
            static_collision.max_shared_hinge_solid_diagnostics,
            hard.max_shared_hinge_solid_diagnostics,
        ),
    ]
    .into_iter()
    .all(|(configured, maximum)| configured <= maximum)
}

pub(super) fn validate_linear_chain_hinges_v1(
    faces: &[FaceId],
    hinges: &[(EdgeId, [FaceId; 2])],
    expected_faces: usize,
    maximum_pairs: usize,
) -> Result<Option<LayeredChainPairPartitionV1>, LayeredChainResourceErrorV1> {
    let expected_hinges = expected_faces
        .checked_sub(1)
        .ok_or(LayeredChainResourceErrorV1::ResourceLimit)?;
    let all_pair_count = expected_faces
        .checked_mul(expected_hinges)
        .and_then(|value| value.checked_div(2))
        .ok_or(LayeredChainResourceErrorV1::ResourceLimit)?;
    if expected_faces < 2
        || all_pair_count > maximum_pairs
        || maximum_pairs > NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1
    {
        return Err(LayeredChainResourceErrorV1::ResourceLimit);
    }
    if faces.len() != expected_faces || hinges.len() != expected_hinges {
        return Ok(None);
    }

    let mut face_set = HashSet::new();
    face_set
        .try_reserve(expected_faces)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    if faces.iter().copied().any(|face| !face_set.insert(face)) {
        return Ok(None);
    }

    let mut edges = HashSet::new();
    edges
        .try_reserve(expected_hinges)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    let mut direct = Vec::new();
    direct
        .try_reserve_exact(expected_hinges)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    let mut degrees = Vec::new();
    degrees
        .try_reserve_exact(expected_faces)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    degrees.resize(expected_faces, 0_usize);

    for &(edge, pair) in hinges {
        let Some(first_index) = faces.iter().position(|face| *face == pair[0]) else {
            return Ok(None);
        };
        let Some(second_index) = faces.iter().position(|face| *face == pair[1]) else {
            return Ok(None);
        };
        if !edges.insert(edge)
            || first_index == second_index
            || canonical_pair_v1(pair[0], pair[1]) != pair
            || direct.contains(&pair)
        {
            return Ok(None);
        }
        direct.push(pair);
        degrees[first_index] = degrees[first_index].saturating_add(1);
        degrees[second_index] = degrees[second_index].saturating_add(1);
    }

    degrees.sort_unstable();
    if degrees.first() != Some(&1)
        || degrees.get(1) != Some(&1)
        || degrees[2..].iter().any(|degree| *degree != 2)
    {
        return Ok(None);
    }

    let mut reached = Vec::new();
    reached
        .try_reserve_exact(expected_faces)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    reached.resize(expected_faces, false);
    reached[0] = true;
    loop {
        let mut changed = false;
        for pair in &direct {
            let first_index = faces
                .iter()
                .position(|face| *face == pair[0])
                .expect("validated direct face");
            let second_index = faces
                .iter()
                .position(|face| *face == pair[1])
                .expect("validated direct face");
            if reached[first_index] != reached[second_index] {
                reached[first_index] = true;
                reached[second_index] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if reached.iter().any(|reached| !reached) {
        return Ok(None);
    }

    direct.sort_unstable_by_key(pair_key_v1);
    let mut nonadjacent = Vec::new();
    nonadjacent
        .try_reserve_exact(all_pair_count.saturating_sub(expected_hinges))
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    for (index, first) in faces.iter().enumerate() {
        for second in &faces[index + 1..] {
            let pair = canonical_pair_v1(*first, *second);
            if !direct.contains(&pair) {
                nonadjacent.push(pair);
            }
        }
    }
    nonadjacent.sort_unstable_by_key(pair_key_v1);
    Ok(Some(LayeredChainPairPartitionV1 {
        direct_pairs: direct,
        nonadjacent_pairs: nonadjacent,
    }))
}

pub(super) fn validate_single_moving_flat_chain_schedule_v1(
    direct_hinges: &[(EdgeId, [FaceId; 2])],
    source: &[(EdgeId, f64)],
    target: &[(EdgeId, f64)],
    expected_hinges: usize,
    maximum_hinges: usize,
) -> Result<Option<Vec<LayeredChainDirectHingeV1>>, LayeredChainResourceErrorV1> {
    if expected_hinges == 0
        || expected_hinges > maximum_hinges
        || maximum_hinges > NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1
    {
        return Err(LayeredChainResourceErrorV1::ResourceLimit);
    }
    if direct_hinges.len() != expected_hinges
        || source.len() != expected_hinges
        || target.len() != expected_hinges
    {
        return Ok(None);
    }

    let mut source_by_edge = HashMap::new();
    source_by_edge
        .try_reserve(expected_hinges)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    let mut target_by_edge = HashMap::new();
    target_by_edge
        .try_reserve(expected_hinges)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    for &(edge, angle) in source {
        if !angle.is_finite() || source_by_edge.insert(edge, angle).is_some() {
            return Ok(None);
        }
    }
    for &(edge, angle) in target {
        if !angle.is_finite() || target_by_edge.insert(edge, angle).is_some() {
            return Ok(None);
        }
    }

    let mut output = Vec::new();
    output
        .try_reserve_exact(expected_hinges)
        .map_err(|_| LayeredChainResourceErrorV1::ResourceLimit)?;
    let mut moving = 0_usize;
    let mut stationary = 0_usize;
    for &(edge, pair) in direct_hinges {
        let Some(source) = source_by_edge.get(&edge).copied() else {
            return Ok(None);
        };
        let Some(target) = target_by_edge.get(&edge).copied() else {
            return Ok(None);
        };
        let is_moving = source.to_bits() == 0.0_f64.to_bits() && target > 0.0 && target < 180.0;
        let is_stationary =
            source.to_bits() == 180.0_f64.to_bits() && target.to_bits() == 180.0_f64.to_bits();
        if !is_moving && !is_stationary {
            return Ok(None);
        }
        moving += usize::from(is_moving);
        stationary += usize::from(is_stationary);
        output.push(LayeredChainDirectHingeV1 {
            edge,
            pair,
            moving: is_moving,
        });
    }
    if moving != 1
        || stationary != expected_hinges.saturating_sub(1)
        || source_by_edge.len() != expected_hinges
        || target_by_edge.len() != expected_hinges
    {
        return Ok(None);
    }
    output.sort_unstable_by_key(|hinge| pair_key_v1(&hinge.pair));
    Ok(Some(output))
}

pub(super) fn verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
    registry: &MaterialTreeDyadicFaceIntervalRegistryV1,
    pairs: &[[FaceId; 2]],
    expected_pairs: usize,
    maximum_pairs: usize,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), LayeredChainIntervalErrorV1> {
    verify_layered_chain_nonadjacent_registry_gaps_with_checkpoint_v1(
        registry,
        pairs,
        expected_pairs,
        maximum_pairs,
        |_| layered_chain_interval_checkpoint_v1(control),
    )
}

pub(super) fn verify_layered_chain_nonadjacent_registry_gaps_with_checkpoint_v1<F>(
    registry: &MaterialTreeDyadicFaceIntervalRegistryV1,
    pairs: &[[FaceId; 2]],
    expected_pairs: usize,
    maximum_pairs: usize,
    mut checkpoint: F,
) -> Result<(), LayeredChainIntervalErrorV1>
where
    F: FnMut(LayeredChainIntervalCheckpointPhaseV1) -> Result<(), LayeredChainIntervalErrorV1>,
{
    validate_gap_pair_count_v1(pairs.len(), expected_pairs, maximum_pairs)?;
    for pair in pairs {
        checkpoint(LayeredChainIntervalCheckpointPhaseV1::Pair)?;
        let intervals = registry
            .face_vertices(pair[0])
            .zip(registry.face_vertices(pair[1]))
            .ok_or(LayeredChainIntervalErrorV1::IntervalUnavailable)?;
        if !strictly_separated_interval_pair_with_checkpoint_v1(intervals, &mut checkpoint)? {
            return Err(LayeredChainIntervalErrorV1::IntervalOverlap);
        }
        checkpoint(LayeredChainIntervalCheckpointPhaseV1::Final)?;
    }
    checkpoint(LayeredChainIntervalCheckpointPhaseV1::Final)
}

#[cfg(test)]
pub(super) fn verify_layered_chain_nonadjacent_gaps_with_control_v1(
    pairs: &[LayeredChainNonadjacentIntervalPairV1<'_>],
    expected_pairs: usize,
    maximum_pairs: usize,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), LayeredChainIntervalErrorV1> {
    validate_gap_pair_count_v1(pairs.len(), expected_pairs, maximum_pairs)?;
    for intervals in pairs {
        layered_chain_interval_checkpoint_v1(control)?;
        if !strictly_separated_interval_pair_with_checkpoint_v1(*intervals, &mut |_| {
            layered_chain_interval_checkpoint_v1(control)
        })? {
            return Err(LayeredChainIntervalErrorV1::IntervalOverlap);
        }
        layered_chain_interval_checkpoint_v1(control)?;
    }
    layered_chain_interval_checkpoint_v1(control)
}

fn validate_gap_pair_count_v1(
    actual_pairs: usize,
    expected_pairs: usize,
    maximum_pairs: usize,
) -> Result<(), LayeredChainIntervalErrorV1> {
    if actual_pairs != expected_pairs
        || actual_pairs > maximum_pairs
        || maximum_pairs > NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1
    {
        return Err(LayeredChainIntervalErrorV1::ResourceLimit);
    }
    Ok(())
}

fn strictly_separated_interval_pair_with_checkpoint_v1<F>(
    (first, second): LayeredChainNonadjacentIntervalPairV1<'_>,
    checkpoint: &mut F,
) -> Result<bool, LayeredChainIntervalErrorV1>
where
    F: FnMut(LayeredChainIntervalCheckpointPhaseV1) -> Result<(), LayeredChainIntervalErrorV1>,
{
    for axis in 0..3 {
        checkpoint(LayeredChainIntervalCheckpointPhaseV1::Axis)?;
        if strict_axis_gap_with_checkpoint_v1(first, second, axis, checkpoint)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn strict_axis_gap_with_checkpoint_v1<F>(
    first: &LayeredChainFaceOutwardIntervalsV1,
    second: &LayeredChainFaceOutwardIntervalsV1,
    axis: usize,
    checkpoint: &mut F,
) -> Result<bool, LayeredChainIntervalErrorV1>
where
    F: FnMut(LayeredChainIntervalCheckpointPhaseV1) -> Result<(), LayeredChainIntervalErrorV1>,
{
    checkpoint(LayeredChainIntervalCheckpointPhaseV1::Vertex)?;
    let Some((_, first_point)) = first.first() else {
        return Ok(false);
    };
    let mut first_lower = first_point[axis].lower();
    let mut first_upper = first_point[axis].upper();
    for (_, point) in &first[1..] {
        checkpoint(LayeredChainIntervalCheckpointPhaseV1::Vertex)?;
        first_lower = first_lower.min(point[axis].lower());
        first_upper = first_upper.max(point[axis].upper());
    }

    checkpoint(LayeredChainIntervalCheckpointPhaseV1::Vertex)?;
    let Some((_, second_point)) = second.first() else {
        return Ok(false);
    };
    let mut second_lower = second_point[axis].lower();
    let mut second_upper = second_point[axis].upper();
    for (_, point) in &second[1..] {
        checkpoint(LayeredChainIntervalCheckpointPhaseV1::Vertex)?;
        second_lower = second_lower.min(point[axis].lower());
        second_upper = second_upper.max(point[axis].upper());
    }
    checkpoint(LayeredChainIntervalCheckpointPhaseV1::Vertex)?;
    Ok(first_upper < second_lower || second_upper < first_lower)
}

fn layered_chain_interval_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), LayeredChainIntervalErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => LayeredChainIntervalErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            LayeredChainIntervalErrorV1::DeadlineExceeded
        }
    })
}
