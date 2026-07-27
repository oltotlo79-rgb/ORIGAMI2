use std::collections::{HashMap, HashSet, VecDeque};

use num_rational::BigRational;
use ori_domain::{EdgeId, FaceId};
use ori_topology::FoldAssignment;

use super::MaterialHingeGraphAudit;
use crate::{
    CanonicalCycleScheduleV1, MaterialHingeGraphGeometry, Point3, TreeHinge,
    transform::{length, scale, subtract},
};

// Native material-graph ceilings. Recognition is linear and rejects before
// allocating graph-sized work when either ceiling is exceeded.
const MAX_EXACT_CUT_CARRIER_FACES_V1: usize = 10_001;
const MAX_EXACT_CUT_CARRIER_HINGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_EXACT_CUT_CARRIER_ADJACENCY_ENTRIES_V1: usize = MAX_EXACT_CUT_CARRIER_HINGES_V1 * 2;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalDirectedInfiniteLineV1 {
    direction_bits: [u64; 3],
    exact_moment: [BigRational; 3],
}

fn canonical_finite_bits_v1(value: f64) -> Option<u64> {
    value
        .is_finite()
        .then(|| if value == 0.0 { 0.0 } else { value }.to_bits())
}

fn canonical_finite_vector_bits_v1(value: [f64; 3]) -> Option<[u64; 3]> {
    Some([
        canonical_finite_bits_v1(value[0])?,
        canonical_finite_bits_v1(value[1])?,
        canonical_finite_bits_v1(value[2])?,
    ])
}

fn exact_binary64_cross_v1(first: [f64; 3], second: [f64; 3]) -> Option<[BigRational; 3]> {
    let first = first.map(BigRational::from_float);
    let second = second.map(BigRational::from_float);
    let [Some(ax), Some(ay), Some(az)] = first else {
        return None;
    };
    let [Some(bx), Some(by), Some(bz)] = second else {
        return None;
    };
    Some([
        &ay * &bz - &az * &by,
        &az * &bx - &ax * &bz,
        &ax * &by - &ay * &bx,
    ])
}

fn point_components_v1(point: Point3) -> [f64; 3] {
    [point.x(), point.y(), point.z()]
}

fn exact_directed_line_v1(
    hinge: &TreeHinge,
    left_component: u8,
    right_component: u8,
) -> Option<CanonicalDirectedInfiniteLineV1> {
    if left_component == right_component || left_component > 1 || right_component > 1 {
        return None;
    }
    let axis = point_components_v1(hinge.axis());
    let start = point_components_v1(hinge.start());
    let delta = subtract(hinge.end(), hinge.start()).ok()?;
    let expected_axis = scale(delta, 1.0 / length(delta).ok()?).ok()?;
    if canonical_finite_vector_bits_v1(point_components_v1(expected_axis))?
        != canonical_finite_vector_bits_v1(axis)?
    {
        return None;
    }

    // observe_closed authenticates `right = left * R(axis, assignment*angle)`.
    // Normalize every stored representation to the component-0 -> component-1
    // generator before comparing carriers.
    let assignment_sign = match hinge.assignment() {
        FoldAssignment::Mountain => 1.0,
        FoldAssignment::Valley => -1.0,
    };
    let side_sign = if left_component == 0 { 1.0 } else { -1.0 };
    let sign = assignment_sign * side_sign;
    let direction = [sign * axis[0], sign * axis[1], sign * axis[2]];
    if direction.iter().any(|value| !value.is_finite()) {
        return None;
    }

    // Exact-binary64 Pluecker direction plus moment p x d is independent of
    // which finite point p on the infinite carrier is stored by an individual
    // hinge; BigRational arithmetic prevents subnormal underflow from merging
    // two distinct parallel carriers.
    let exact_moment = exact_binary64_cross_v1(start, direction)?;
    Some(CanonicalDirectedInfiniteLineV1 {
        direction_bits: canonical_finite_vector_bits_v1(direction)?,
        exact_moment,
    })
}

fn bounded_exact_cut_carrier_counts_v1(face_count: usize, hinge_count: usize) -> bool {
    (2..=MAX_EXACT_CUT_CARRIER_FACES_V1).contains(&face_count)
        && (1..=MAX_EXACT_CUT_CARRIER_HINGES_V1).contains(&hinge_count)
        && hinge_count
            .checked_mul(2)
            .is_some_and(|entries| entries <= MAX_EXACT_CUT_CARRIER_ADJACENCY_ENTRIES_V1)
}

fn exact_cut_components_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    moving: &HashSet<EdgeId>,
) -> Option<HashMap<FaceId, u8>> {
    if !bounded_exact_cut_carrier_counts_v1(geometry.face_ids().len(), geometry.hinges().len())
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
    let mut adjacency = vec![Vec::<(usize, EdgeId)>::new(); faces.len()];
    for hinge in geometry.hinges() {
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
        if !moving.contains(&hinge.edge()) {
            adjacency[left].push((right, hinge.edge()));
            adjacency[right].push((left, hinge.edge()));
        }
    }
    if geometry_edges != audit_edges || !moving.is_subset(&geometry_edges) {
        return None;
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(face, edge)| {
            (faces[*face].canonical_bytes(), edge.canonical_bytes())
        });
    }

    // Faces and each neighbor list are canonical, so component labels and BFS
    // visitation are independent of source/storage order.
    let mut component_by_index = vec![u8::MAX; faces.len()];
    let mut component_count = 0u8;
    for seed in 0..faces.len() {
        if component_by_index[seed] != u8::MAX {
            continue;
        }
        if component_count >= 2 {
            return None;
        }
        let mut queue = VecDeque::with_capacity(faces.len());
        component_by_index[seed] = component_count;
        queue.push_back(seed);
        while let Some(face) = queue.pop_front() {
            for &(next, _) in &adjacency[face] {
                if component_by_index[next] == u8::MAX {
                    component_by_index[next] = component_count;
                    queue.push_back(next);
                }
            }
        }
        component_count += 1;
    }
    if component_count != 2
        || component_by_index.iter().any(|component| *component > 1)
        || ![0, 1]
            .into_iter()
            .all(|component| component_by_index.contains(&component))
    {
        return None;
    }
    let components = faces
        .iter()
        .copied()
        .zip(component_by_index)
        .collect::<HashMap<_, _>>();
    Some(components)
}

/// Exact all-parameter closure identity for an arbitrary connected graph cut
/// into two stationary components by one collective directed line generator.
///
/// Every stationary edge is the identity. Assigning identity to component 0
/// and the common rotation T(u) to component 1 therefore satisfies every
/// internal edge and every crossing edge for all schedule parameters. No
/// sampled pose, tolerance-based collinearity, or rotation commutation is used.
pub(super) fn exact_cut_carrier_cycle_closure_premises_v1(
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
    let Some(moving_edges) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    let moving = moving_edges.iter().copied().collect::<HashSet<_>>();
    if moving.is_empty() || moving.len() != moving_edges.len() {
        return false;
    }
    let Some(initial) = schedule.evaluate(0.0) else {
        return false;
    };
    let initial_by_edge = initial
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect::<HashMap<_, _>>();
    if initial_by_edge.len() != geometry.hinges().len()
        || geometry.hinges().iter().any(|hinge| {
            !initial_by_edge.contains_key(&hinge.edge())
                || (!moving.contains(&hinge.edge())
                    && (!schedule.is_exact_constant_profile_v1(hinge.edge())
                        || initial_by_edge.get(&hinge.edge()).copied() != Some(0.0_f64.to_bits())))
        })
    {
        return false;
    }

    let Some(components) = exact_cut_components_v1(geometry, audit, &moving) else {
        return false;
    };
    let mut reference = None;
    let mut crossing_count = 0usize;
    for hinge in geometry.hinges() {
        if !moving.contains(&hinge.edge()) {
            continue;
        }
        let (Some(&left_component), Some(&right_component)) = (
            components.get(&hinge.left_face()),
            components.get(&hinge.right_face()),
        ) else {
            return false;
        };
        if left_component == right_component {
            return false;
        }
        let Some(line) = exact_directed_line_v1(hinge, left_component, right_component) else {
            return false;
        };
        if reference.as_ref().is_some_and(|expected| expected != &line) {
            return false;
        }
        reference = Some(line);
        crossing_count = match crossing_count.checked_add(1) {
            Some(count) => count,
            None => return false,
        };
    }
    reference.is_some() && crossing_count == moving.len()
}

#[cfg(test)]
#[path = "exact_cut_carrier_tests.rs"]
mod tests;
