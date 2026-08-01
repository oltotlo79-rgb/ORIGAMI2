//! Iterative Tarjan traversal and canonical block materialization for V2.

use std::collections::{HashMap, HashSet};

use ori_domain::FaceId;

use super::{
    CANONICAL_MIURA_FACES_PER_BLOCK_V2, CANONICAL_MIURA_HINGES_PER_BLOCK_V2,
    CanonicalMaterialEdgeBlockV1, CommonArticulationDecompositionErrorV2,
    CommonArticulationDecompositionStopV2, MaterialHingeGraphAudit, RawBlockV2, TarjanFrameV2,
    UNASSIGNED_BLOCK_V2, WorkMeterV2, reserved_options_v2, reserved_zeros_v2,
};
use crate::MaterialHingeGraphGeometry;

pub(super) fn prepare_face_index_v2(
    geometry: &MaterialHingeGraphGeometry,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<HashMap<FaceId, usize>, CommonArticulationDecompositionErrorV2> {
    let mut face_index = HashMap::new();
    face_index
        .try_reserve(geometry.face_ids().len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for (index, face) in geometry.face_ids().iter().copied().enumerate() {
        meter.account(1, checkpoint)?;
        if face_index.insert(face, index).is_some() {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    (face_index.len() == geometry.face_ids().len())
        .then_some(face_index)
        .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)
}

pub(super) fn prepare_adjacency_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    face_index: &HashMap<FaceId, usize>,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<Vec<(usize, usize)>>, CommonArticulationDecompositionErrorV2> {
    let face_count = geometry.face_ids().len();
    let hinge_count = geometry.hinges().len();
    let adjacency_entries = hinge_count
        .checked_mul(2)
        .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    let mut audit_edges = HashSet::new();
    audit_edges
        .try_reserve(hinge_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
        meter.account(1, checkpoint)?;
        if !audit_edges.insert(*edge) {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    if audit_edges.len() != hinge_count {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }

    let mut degrees: Vec<usize> = Vec::new();
    degrees
        .try_reserve_exact(face_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for _ in 0..face_count {
        meter.account(1, checkpoint)?;
        degrees.push(0);
    }
    let mut geometry_edges = HashSet::new();
    geometry_edges
        .try_reserve(hinge_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for hinge in geometry.hinges() {
        meter.account(1, checkpoint)?;
        let left = *face_index
            .get(&hinge.left_face())
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        let right = *face_index
            .get(&hinge.right_face())
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        if left == right
            || !geometry_edges.insert(hinge.edge())
            || !audit_edges.contains(&hinge.edge())
        {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
        degrees[left] = degrees[left]
            .checked_add(1)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        degrees[right] = degrees[right]
            .checked_add(1)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    }
    let mut observed_adjacency_entries = 0usize;
    for degree in &degrees {
        meter.account(1, checkpoint)?;
        observed_adjacency_entries = observed_adjacency_entries
            .checked_add(*degree)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    }
    if observed_adjacency_entries != adjacency_entries {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }

    let mut adjacency = Vec::new();
    adjacency
        .try_reserve(face_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for degree in degrees {
        meter.account(1, checkpoint)?;
        let mut neighbors = Vec::new();
        neighbors
            .try_reserve_exact(degree)
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        adjacency.push(neighbors);
    }
    for (edge_index, hinge) in geometry.hinges().iter().enumerate() {
        meter.account(1, checkpoint)?;
        let left = *face_index
            .get(&hinge.left_face())
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        let right = *face_index
            .get(&hinge.right_face())
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        adjacency[left].push((right, edge_index));
        adjacency[right].push((left, edge_index));
    }
    // Hinges arrive in canonical edge order.  Validating the per-face order
    // avoids an uninterruptible general-purpose sort and makes traversal
    // deterministic without assuming storage order.
    for neighbors in &adjacency {
        for pair in neighbors.windows(2) {
            meter.account(1, checkpoint)?;
            let previous = geometry.hinges()[pair[0].1].edge().canonical_bytes();
            let current = geometry.hinges()[pair[1].1].edge().canonical_bytes();
            if previous >= current {
                return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
            }
        }
    }
    Ok(adjacency)
}

pub(super) fn tarjan_block_assignments_v2(
    adjacency: &[Vec<(usize, usize)>],
    hinge_count: usize,
    expected_block_count: usize,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<usize>, CommonArticulationDecompositionErrorV2> {
    if adjacency.is_empty() {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }
    let expected_adjacency_work = hinge_count
        .checked_mul(2)
        .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    let mut discovery = reserved_zeros_v2(adjacency.len(), meter, checkpoint)?;
    let mut low = reserved_zeros_v2(adjacency.len(), meter, checkpoint)?;
    let mut parent_node = reserved_options_v2(adjacency.len(), meter, checkpoint)?;
    let mut parent_edge = reserved_options_v2(adjacency.len(), meter, checkpoint)?;
    let mut block_by_edge = Vec::new();
    block_by_edge
        .try_reserve_exact(hinge_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for _ in 0..hinge_count {
        meter.account(1, checkpoint)?;
        block_by_edge.push(UNASSIGNED_BLOCK_V2);
    }
    let mut edge_stack = Vec::new();
    edge_stack
        .try_reserve_exact(hinge_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(adjacency.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;

    let mut next_time = 1usize;
    discovery[0] = next_time;
    low[0] = next_time;
    frames.push(TarjanFrameV2 {
        node: 0,
        next_neighbor: 0,
    });
    let mut block_count = 0usize;
    let mut adjacency_work = 0usize;
    while let Some(frame) = frames.last_mut() {
        let node = frame.node;
        let neighbor_index = frame.next_neighbor;
        if neighbor_index < adjacency[node].len() {
            frame.next_neighbor = frame
                .next_neighbor
                .checked_add(1)
                .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
            adjacency_work = adjacency_work
                .checked_add(1)
                .filter(|work| *work <= expected_adjacency_work)
                .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
            meter.account(1, checkpoint)?;
            let (next, edge) = adjacency[node][neighbor_index];
            if parent_edge[node] == Some(edge) {
                continue;
            }
            if discovery[next] == 0 {
                edge_stack.push(edge);
                next_time = next_time
                    .checked_add(1)
                    .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
                discovery[next] = next_time;
                low[next] = next_time;
                parent_node[next] = Some(node);
                parent_edge[next] = Some(edge);
                frames.push(TarjanFrameV2 {
                    node: next,
                    next_neighbor: 0,
                });
            } else if discovery[next] < discovery[node] {
                edge_stack.push(edge);
                low[node] = low[node].min(discovery[next]);
            }
        } else {
            let node = frames
                .pop()
                .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?
                .node;
            meter.account(1, checkpoint)?;
            if let (Some(parent), Some(edge)) = (parent_node[node], parent_edge[node]) {
                if low[node] >= discovery[parent] {
                    if block_count >= expected_block_count {
                        return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
                    }
                    loop {
                        meter.account(1, checkpoint)?;
                        let popped = edge_stack
                            .pop()
                            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
                        if block_by_edge[popped] != UNASSIGNED_BLOCK_V2 {
                            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
                        }
                        block_by_edge[popped] = block_count;
                        if popped == edge {
                            break;
                        }
                    }
                    block_count = block_count
                        .checked_add(1)
                        .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
                }
                low[parent] = low[parent].min(low[node]);
            }
        }
    }
    let mut every_face_discovered = true;
    for discovery_time in &discovery {
        meter.account(1, checkpoint)?;
        every_face_discovered &= *discovery_time != 0;
    }
    let mut every_edge_assigned = true;
    for assignment in &block_by_edge {
        meter.account(1, checkpoint)?;
        every_edge_assigned &= *assignment != UNASSIGNED_BLOCK_V2;
    }
    if adjacency_work != expected_adjacency_work
        || !every_face_discovered
        || !edge_stack.is_empty()
        || block_count != expected_block_count
        || !every_edge_assigned
    {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }
    Ok(block_by_edge)
}

pub(super) fn materialize_raw_blocks_v2(
    geometry: &MaterialHingeGraphGeometry,
    block_by_edge: Vec<usize>,
    expected_block_count: usize,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<RawBlockV2>, CommonArticulationDecompositionErrorV2> {
    let mut sizes = reserved_zeros_v2(expected_block_count, meter, checkpoint)?;
    for block in &block_by_edge {
        meter.account(1, checkpoint)?;
        let size = sizes
            .get_mut(*block)
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        *size = size
            .checked_add(1)
            .filter(|size| *size <= CANONICAL_MIURA_HINGES_PER_BLOCK_V2)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(expected_block_count)
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for size in sizes {
        meter.account(1, checkpoint)?;
        if size != CANONICAL_MIURA_HINGES_PER_BLOCK_V2 {
            return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
        }
        let mut hinge_indices = Vec::new();
        hinge_indices
            .try_reserve_exact(size)
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        raw.push(RawBlockV2 {
            faces: Vec::new(),
            hinge_indices,
        });
    }
    for (edge, block) in block_by_edge.into_iter().enumerate() {
        meter.account(1, checkpoint)?;
        raw.get_mut(block)
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?
            .hinge_indices
            .push(edge);
    }
    for block in &mut raw {
        sort_small_hinge_indices_v2(geometry, &mut block.hinge_indices, meter, checkpoint)?;
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(
                CANONICAL_MIURA_HINGES_PER_BLOCK_V2
                    .checked_mul(2)
                    .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?,
            )
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        for edge in &block.hinge_indices {
            meter.account(1, checkpoint)?;
            let hinge = geometry
                .hinges()
                .get(*edge)
                .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
            faces.push(hinge.left_face());
            faces.push(hinge.right_face());
        }
        sort_small_faces_v2(&mut faces, meter, checkpoint)?;
        faces.dedup();
        if faces.len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2 {
            return Err(CommonArticulationDecompositionErrorV2::ResourceLimit);
        }
        block.faces = faces;
    }
    Ok(raw)
}

pub(super) fn canonical_raw_block_order_v2(
    geometry: &MaterialHingeGraphGeometry,
    raw: &[RawBlockV2],
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<usize>, CommonArticulationDecompositionErrorV2> {
    let mut order = Vec::new();
    order
        .try_reserve_exact(raw.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for index in 0..raw.len() {
        meter.account(1, checkpoint)?;
        order.push(index);
    }
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(raw.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for _ in 0..raw.len() {
        meter.account(1, checkpoint)?;
        scratch.push(0);
    }
    let mut width = 1usize;
    while width < order.len() {
        let step = width
            .checked_mul(2)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        let mut start = 0usize;
        while start < order.len() {
            let middle = start
                .checked_add(width)
                .map(|value| value.min(order.len()))
                .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
            let end = start
                .checked_add(step)
                .map(|value| value.min(order.len()))
                .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
            let mut left = start;
            let mut right = middle;
            let mut write = start;
            while left < middle && right < end {
                meter.account(1, checkpoint)?;
                if raw_block_key_v2(geometry, &raw[order[left]])?
                    <= raw_block_key_v2(geometry, &raw[order[right]])?
                {
                    scratch[write] = order[left];
                    left += 1;
                } else {
                    scratch[write] = order[right];
                    right += 1;
                }
                write += 1;
            }
            while left < middle {
                meter.account(1, checkpoint)?;
                scratch[write] = order[left];
                left += 1;
                write += 1;
            }
            while right < end {
                meter.account(1, checkpoint)?;
                scratch[write] = order[right];
                right += 1;
                write += 1;
            }
            start = start
                .checked_add(step)
                .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        }
        for (target, source) in order.iter_mut().zip(&scratch) {
            meter.account(1, checkpoint)?;
            *target = *source;
        }
        width = step;
    }
    Ok(order)
}

pub(super) fn reorder_raw_blocks_v2(
    raw: Vec<RawBlockV2>,
    order: Vec<usize>,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<RawBlockV2>, CommonArticulationDecompositionErrorV2> {
    if raw.len() != order.len() {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(raw.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for block in raw {
        meter.account(1, checkpoint)?;
        slots.push(Some(block));
    }
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(slots.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for index in order {
        meter.account(1, checkpoint)?;
        ordered.push(
            slots
                .get_mut(index)
                .and_then(Option::take)
                .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?,
        );
    }
    Ok(ordered)
}

pub(super) fn articulation_faces_v2(
    raw: &[RawBlockV2],
    canonical_faces: &[FaceId],
    expected_block_count: usize,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<FaceId>, CommonArticulationDecompositionErrorV2> {
    let mut incidence = HashMap::<FaceId, u8>::new();
    incidence
        .try_reserve(canonical_faces.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for block in raw {
        for face in &block.faces {
            meter.account(1, checkpoint)?;
            let count = incidence.entry(*face).or_default();
            *count = count
                .checked_add(1)
                .filter(|count| *count <= 2)
                .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        }
    }
    if incidence.len() != canonical_faces.len() {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }
    let mut articulation_faces = Vec::new();
    articulation_faces
        .try_reserve_exact(
            expected_block_count
                .checked_sub(1)
                .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?,
        )
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for face in canonical_faces {
        meter.account(1, checkpoint)?;
        if incidence.get(face) == Some(&2) {
            articulation_faces.push(*face);
        }
    }
    if articulation_faces.len()
        != expected_block_count
            .checked_sub(1)
            .ok_or(CommonArticulationDecompositionErrorV2::ResourceLimit)?
    {
        return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
    }
    Ok(articulation_faces)
}

pub(super) fn materialize_blocks_v2(
    geometry: &MaterialHingeGraphGeometry,
    raw: Vec<RawBlockV2>,
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<Vec<CanonicalMaterialEdgeBlockV1>, CommonArticulationDecompositionErrorV2> {
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(raw.len())
        .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
    for raw_block in raw {
        meter.account(1, checkpoint)?;
        let mut hinges = Vec::new();
        hinges
            .try_reserve_exact(raw_block.hinge_indices.len())
            .map_err(|_| CommonArticulationDecompositionErrorV2::ResourceLimit)?;
        for index in raw_block.hinge_indices {
            meter.account(1, checkpoint)?;
            hinges.push(
                geometry
                    .hinges()
                    .get(index)
                    .cloned()
                    .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?,
            );
        }
        let audit = MaterialHingeGraphAudit::from_block(&raw_block.faces, &hinges)
            .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
        let geometry = geometry
            .edge_block_instance_v2(raw_block.faces, hinges)
            .map_err(|_| CommonArticulationDecompositionErrorV2::InvalidInput)?;
        blocks.push(CanonicalMaterialEdgeBlockV1 { geometry, audit });
    }
    Ok(blocks)
}

fn raw_block_key_v2(
    geometry: &MaterialHingeGraphGeometry,
    block: &RawBlockV2,
) -> Result<([u8; 16], [u8; 16]), CommonArticulationDecompositionErrorV2> {
    let first_face = block
        .faces
        .first()
        .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
    let first_hinge = block
        .hinge_indices
        .first()
        .and_then(|index| geometry.hinges().get(*index))
        .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?;
    Ok((
        first_face.canonical_bytes(),
        first_hinge.edge().canonical_bytes(),
    ))
}

fn sort_small_hinge_indices_v2(
    geometry: &MaterialHingeGraphGeometry,
    values: &mut [usize],
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<(), CommonArticulationDecompositionErrorV2> {
    for current in 1..values.len() {
        let mut index = current;
        while index > 0 {
            meter.account(1, checkpoint)?;
            let previous = geometry
                .hinges()
                .get(values[index - 1])
                .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?
                .edge()
                .canonical_bytes();
            let next = geometry
                .hinges()
                .get(values[index])
                .ok_or(CommonArticulationDecompositionErrorV2::InvalidInput)?
                .edge()
                .canonical_bytes();
            if previous <= next {
                break;
            }
            values.swap(index - 1, index);
            index -= 1;
        }
    }
    for pair in values.windows(2) {
        meter.account(1, checkpoint)?;
        if pair[0] == pair[1] {
            return Err(CommonArticulationDecompositionErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn sort_small_faces_v2(
    values: &mut [FaceId],
    meter: &mut WorkMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDecompositionStopV2>,
) -> Result<(), CommonArticulationDecompositionErrorV2> {
    for current in 1..values.len() {
        let mut index = current;
        while index > 0 {
            meter.account(1, checkpoint)?;
            if values[index - 1].canonical_bytes() <= values[index].canonical_bytes() {
                break;
            }
            values.swap(index - 1, index);
            index -= 1;
        }
    }
    Ok(())
}
