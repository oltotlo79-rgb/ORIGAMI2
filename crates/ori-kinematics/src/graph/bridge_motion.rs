use std::collections::{HashMap, HashSet};

use ori_domain::FaceId;

use super::MaterialHingeGraphAudit;
use crate::{
    CanonicalCycleScheduleV1, MaterialHingeGraphGeometry, TreeHinge,
    transform::{length, scale, subtract},
};

// The recognizer stays inside the native material-graph ceilings and performs
// one canonical iterative Tarjan traversal after linear authentication.
const MAX_BRIDGE_MOTION_FACES_V1: usize = 10_001;
const MAX_BRIDGE_MOTION_HINGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_BRIDGE_MOTION_ADJACENCY_ENTRIES_V1: usize = MAX_BRIDGE_MOTION_HINGES_V1 * 2;

fn bounded_bridge_motion_counts_v1(face_count: usize, hinge_count: usize) -> bool {
    (2..=MAX_BRIDGE_MOTION_FACES_V1).contains(&face_count)
        && (1..=MAX_BRIDGE_MOTION_HINGES_V1).contains(&hinge_count)
        && hinge_count
            .checked_mul(2)
            .is_some_and(|entries| entries <= MAX_BRIDGE_MOTION_ADJACENCY_ENTRIES_V1)
}

fn canonical_finite_bits_v1(value: f64) -> Option<u64> {
    value
        .is_finite()
        .then(|| if value == 0.0 { 0.0 } else { value }.to_bits())
}

fn bridge_has_native_axis_v1(hinge: &TreeHinge) -> bool {
    let Ok(delta) = subtract(hinge.end(), hinge.start()) else {
        return false;
    };
    let Ok(delta_length) = length(delta) else {
        return false;
    };
    let Ok(expected) = scale(delta, 1.0 / delta_length) else {
        return false;
    };
    [expected.x(), expected.y(), expected.z()]
        .into_iter()
        .map(canonical_finite_bits_v1)
        .eq([hinge.axis().x(), hinge.axis().y(), hinge.axis().z()]
            .into_iter()
            .map(canonical_finite_bits_v1))
}

fn recognize_bridge_edges_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
) -> Option<Vec<bool>> {
    if !bounded_bridge_motion_counts_v1(geometry.face_ids().len(), geometry.hinges().len())
        || audit.closure_hinges().is_empty()
    {
        return None;
    }

    let mut faces = geometry.face_ids().to_vec();
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if faces.windows(2).any(|pair| pair[0] == pair[1]) || faces != audit.faces() {
        return None;
    }
    let face_indices = faces
        .iter()
        .copied()
        .enumerate()
        .map(|(index, face)| (face, index))
        .collect::<HashMap<_, _>>();

    let mut audit_edges = HashSet::with_capacity(geometry.hinges().len());
    if audit
        .spanning_hinges()
        .iter()
        .chain(audit.closure_hinges())
        .any(|edge| !audit_edges.insert(*edge))
        || audit_edges.len() != geometry.hinges().len()
    {
        return None;
    }

    let mut geometry_edges = HashSet::with_capacity(geometry.hinges().len());
    let mut face_pairs = HashSet::with_capacity(geometry.hinges().len());
    let mut adjacency = vec![Vec::<(usize, usize)>::new(); faces.len()];
    for (edge_index, hinge) in geometry.hinges().iter().enumerate() {
        let left = *face_indices.get(&hinge.left_face())?;
        let right = *face_indices.get(&hinge.right_face())?;
        if left == right
            || !geometry_edges.insert(hinge.edge())
            || !audit_edges.contains(&hinge.edge())
        {
            return None;
        }
        let pair = if hinge.left_face().canonical_bytes() < hinge.right_face().canonical_bytes() {
            (hinge.left_face(), hinge.right_face())
        } else {
            (hinge.right_face(), hinge.left_face())
        };
        if !face_pairs.insert(pair) {
            return None;
        }
        adjacency[left].push((right, edge_index));
        adjacency[right].push((left, edge_index));
    }
    if geometry_edges != audit_edges {
        return None;
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(face, edge)| {
            (
                faces[*face].canonical_bytes(),
                geometry.hinges()[*edge].edge().canonical_bytes(),
            )
        });
    }

    let mut discovery = vec![0usize; faces.len()];
    let mut low = vec![0usize; faces.len()];
    let mut parent_node = vec![None; faces.len()];
    let mut parent_edge = vec![None; faces.len()];
    let mut bridges = vec![false; geometry.hinges().len()];
    let mut next_time = 1usize;
    discovery[0] = next_time;
    low[0] = next_time;
    let mut stack = Vec::with_capacity(faces.len());
    stack.push((0usize, 0usize));

    // Iterative Tarjan avoids a source-sized call stack. Both face seeds and
    // every adjacency list are canonical, so storage permutation cannot alter
    // discovery/low-link evidence.
    while !stack.is_empty() {
        let frame = stack.len() - 1;
        let node = stack[frame].0;
        let neighbor_index = stack[frame].1;
        if neighbor_index < adjacency[node].len() {
            stack[frame].1 += 1;
            let (next, edge) = adjacency[node][neighbor_index];
            if parent_edge[node] == Some(edge) {
                continue;
            }
            if discovery[next] == 0 {
                next_time = next_time.checked_add(1)?;
                discovery[next] = next_time;
                low[next] = next_time;
                parent_node[next] = Some(node);
                parent_edge[next] = Some(edge);
                stack.push((next, 0));
            } else {
                low[node] = low[node].min(discovery[next]);
            }
        } else {
            stack.pop();
            if let (Some(parent), Some(edge)) = (parent_node[node], parent_edge[node]) {
                if low[node] > discovery[parent] {
                    bridges[edge] = true;
                }
                low[parent] = low[parent].min(low[node]);
            }
        }
    }
    discovery.iter().all(|time| *time != 0).then_some(bridges)
}

/// Exact closure identity for arbitrary cyclic cores connected only by moving
/// graph bridges.
///
/// A bridge belongs to no closed walk. Requiring every non-bridge edge to be
/// exact-zero therefore makes every cycle transform the identity for the
/// complete schedule domain. Contracting the zero components leaves a tree,
/// so each arbitrary bridge transform is composed exactly once and requires
/// neither commutation nor a sampled closure check.
pub(super) fn bridge_only_motion_cycle_closure_premises_v1(
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
    let Some(bridges) = recognize_bridge_edges_v1(geometry, audit) else {
        return false;
    };
    let Some(initial) = schedule.evaluate(0.0) else {
        return false;
    };
    let initial_by_edge = initial
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect::<HashMap<_, _>>();
    if initial_by_edge.len() != geometry.hinges().len() {
        return false;
    }

    let mut active_bridge_count = 0usize;
    for (hinge, is_bridge) in geometry.hinges().iter().zip(bridges) {
        let Some(initial_bits) = initial_by_edge.get(&hinge.edge()).copied() else {
            return false;
        };
        if schedule.derivative_bound(hinge.edge()).is_none() {
            return false;
        }
        let exact_zero = schedule.is_exact_constant_profile_v1(hinge.edge())
            && initial_bits == 0.0_f64.to_bits();
        if !is_bridge && !exact_zero {
            return false;
        }
        if is_bridge && !exact_zero {
            if !bridge_has_native_axis_v1(hinge) {
                return false;
            }
            active_bridge_count = match active_bridge_count.checked_add(1) {
                Some(count) => count,
                None => return false,
            };
        }
    }
    active_bridge_count > 0
}

#[cfg(test)]
#[path = "bridge_motion_tests.rs"]
mod tests;
