use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};

use super::{
    MaterialHingeGraphAudit,
    exact_generator_word::{
        AuthenticatedGraphV1, CanonicalInfiniteLineV1, authenticate_graph_v1,
        exact_generator_line_v1,
    },
};
use crate::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry};

const MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1: usize = 64;
const MAX_COAXIAL_PROFILE_LATTICE_STORAGE_V1: usize =
    10_001 * MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1;
const MAX_COAXIAL_PROFILE_LATTICE_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2 * MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1;
const MAX_COAXIAL_PROFILE_CLASSIFICATION_WORK_V1: usize =
    ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CoaxialProfileKeyV1 {
    CollectiveNonconstant,
    ConstantAngle(u64),
}

#[derive(Debug, Clone)]
struct PendingLatticeEdgeV1 {
    left: usize,
    right: usize,
    edge: EdgeId,
    profile: Option<CoaxialProfileKeyV1>,
    sign: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoaxialLatticeBoundsV1 {
    storage: usize,
    work: usize,
}

fn bounded_coaxial_lattice_work_v1(
    face_count: usize,
    adjacency_entry_count: usize,
    profile_count: usize,
) -> Option<CoaxialLatticeBoundsV1> {
    if !(2..=MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1).contains(&profile_count) {
        return None;
    }
    let storage = face_count.checked_mul(profile_count)?;
    let work = adjacency_entry_count.checked_mul(profile_count)?;
    (storage <= MAX_COAXIAL_PROFILE_LATTICE_STORAGE_V1
        && work <= MAX_COAXIAL_PROFILE_LATTICE_WORK_V1)
        .then_some(CoaxialLatticeBoundsV1 { storage, work })
}

/// Returns the exact semantic key of one already-proved constant public angle.
///
/// Both signed zeros are the identity. Every other finite 0..=180 bit pattern
/// remains distinct, including cardinal values, the minimum positive
/// subnormal, 180 degrees, and both neighbors of 180. No periodic or
/// self-inverse relation is admitted by the integer lattice.
fn exact_constant_profile_v1(angle_bits: u64) -> Option<Option<CoaxialProfileKeyV1>> {
    let angle = f64::from_bits(angle_bits);
    if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
        return None;
    }
    if angle == 0.0 {
        Some(None)
    } else {
        Some(Some(CoaxialProfileKeyV1::ConstantAngle(angle.to_bits())))
    }
}

fn insert_distinct_profile_v1(
    profiles: &mut Vec<CoaxialProfileKeyV1>,
    candidate: CoaxialProfileKeyV1,
    work: &mut usize,
) -> Option<()> {
    for existing in profiles.iter() {
        *work = work.checked_add(1)?;
        if *work > MAX_COAXIAL_PROFILE_CLASSIFICATION_WORK_V1 {
            return None;
        }
        if *existing == candidate {
            return Some(());
        }
    }
    if profiles.len() >= MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1
        || profiles.len() == profiles.capacity()
    {
        return None;
    }
    profiles.push(candidate);
    Some(())
}

fn classify_coaxial_edges_v1(
    geometry: &MaterialHingeGraphGeometry,
    graph: &AuthenticatedGraphV1,
    schedule: &CanonicalCycleScheduleV1,
) -> Option<(Vec<PendingLatticeEdgeV1>, Vec<CoaxialProfileKeyV1>)> {
    // This is the only general exact nonconstant-profile observer currently
    // exposed by CanonicalCycleScheduleV1. It proves one bit-identical profile
    // inside the prepared polynomial or half-angle representation. We never
    // infer a second profile from samples, derivatives, or cross-representation
    // values.
    let moving_edges = schedule.collective_profile_edges_v1()?;
    let mut moving = HashSet::new();
    moving.try_reserve(moving_edges.len()).ok()?;
    if moving_edges.is_empty() || moving_edges.iter().any(|edge| !moving.insert(*edge)) {
        return None;
    }

    let initial = schedule.evaluate(0.0)?;
    let mut initial_by_edge = HashMap::new();
    initial_by_edge.try_reserve(geometry.hinges().len()).ok()?;
    for angle in initial.as_slice() {
        if initial_by_edge
            .insert(angle.edge(), angle.angle_degrees().to_bits())
            .is_some()
        {
            return None;
        }
    }
    if initial_by_edge.len() != geometry.hinges().len() {
        return None;
    }

    let mut labels = Vec::new();
    labels.try_reserve_exact(graph.hinges().len()).ok()?;
    let mut profiles = Vec::new();
    profiles
        .try_reserve_exact(MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1)
        .ok()?;
    let mut profile_classification_work = 0usize;
    let mut reference_line: Option<CanonicalInfiniteLineV1> = None;
    let mut classified_moving = 0usize;
    for record in graph.hinges() {
        let hinge = geometry.hinges().get(record.geometry_index())?;
        let edge = hinge.edge();
        schedule.derivative_bound(edge)?;
        let exact_constant = schedule.is_exact_constant_profile_v1(edge);
        let profile = if moving.contains(&edge) {
            if exact_constant {
                return None;
            }
            classified_moving = classified_moving.checked_add(1)?;
            Some(CoaxialProfileKeyV1::CollectiveNonconstant)
        } else {
            if !exact_constant {
                return None;
            }
            exact_constant_profile_v1(*initial_by_edge.get(&edge)?)?
        };
        let sign = if let Some(profile) = profile {
            let (line, sign) = exact_generator_line_v1(hinge)?;
            if reference_line
                .as_ref()
                .is_some_and(|reference| reference != &line)
            {
                return None;
            }
            if reference_line.is_none() {
                reference_line = Some(line.clone());
            }
            insert_distinct_profile_v1(&mut profiles, profile, &mut profile_classification_work)?;
            sign
        } else {
            0
        };
        labels.push(PendingLatticeEdgeV1 {
            left: record.left(),
            right: record.right(),
            edge,
            profile,
            sign,
        });
    }
    if classified_moving != moving.len() || reference_line.is_none() {
        return None;
    }
    profiles.sort_unstable();
    bounded_coaxial_lattice_work_v1(
        graph.faces().len(),
        graph.adjacency_entry_limit(),
        profiles.len(),
    )?;
    Some((labels, profiles))
}

fn exact_integer_lattice_potential_v1(
    graph: &AuthenticatedGraphV1,
    fixed_face: FaceId,
    labels: &[PendingLatticeEdgeV1],
    profiles: &[CoaxialProfileKeyV1],
) -> bool {
    let Some(bounds) = bounded_coaxial_lattice_work_v1(
        graph.faces().len(),
        graph.adjacency_entry_limit(),
        profiles.len(),
    ) else {
        return false;
    };
    if labels.len() != graph.hinges().len()
        || profiles.windows(2).any(|pair| pair[0] >= pair[1])
        || profiles.first() != Some(&CoaxialProfileKeyV1::CollectiveNonconstant)
    {
        return false;
    }
    let Some(root) = graph
        .faces()
        .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
        .ok()
    else {
        return false;
    };

    let mut degrees = Vec::new();
    if degrees.try_reserve_exact(graph.faces().len()).is_err() {
        return false;
    }
    degrees.resize(graph.faces().len(), 0usize);
    for label in labels {
        degrees[label.left] = match degrees[label.left].checked_add(1) {
            Some(degree) => degree,
            None => return false,
        };
        degrees[label.right] = match degrees[label.right].checked_add(1) {
            Some(degree) => degree,
            None => return false,
        };
    }
    let degree_sum = match degrees
        .iter()
        .try_fold(0usize, |sum, degree| sum.checked_add(*degree))
    {
        Some(sum) => sum,
        None => return false,
    };
    if degree_sum != graph.adjacency_entry_limit() {
        return false;
    }

    let mut adjacency = Vec::new();
    if adjacency.try_reserve_exact(graph.faces().len()).is_err() {
        return false;
    }
    for degree in degrees {
        let mut neighbors = Vec::new();
        if neighbors.try_reserve_exact(degree).is_err() {
            return false;
        }
        adjacency.push(neighbors);
    }
    for label in labels {
        let profile = if let Some(profile) = label.profile {
            match profiles.binary_search(&profile) {
                Ok(index) => Some(index),
                Err(_) => return false,
            }
        } else {
            None
        };
        adjacency[label.left].push((label.right, profile, label.sign, label.edge));
        adjacency[label.right].push((
            label.left,
            profile,
            match label.sign.checked_neg() {
                Some(sign) => sign,
                None => return false,
            },
            label.edge,
        ));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(face, profile, sign, edge)| {
            (
                graph.faces()[*face].canonical_bytes(),
                *profile,
                *sign,
                edge.canonical_bytes(),
            )
        });
    }

    let mut potentials = Vec::new();
    if potentials.try_reserve_exact(bounds.storage).is_err() {
        return false;
    }
    potentials.resize(bounds.storage, 0_i32);
    let mut assigned = Vec::new();
    if assigned.try_reserve_exact(graph.faces().len()).is_err() {
        return false;
    }
    assigned.resize(graph.faces().len(), false);
    assigned[root] = true;
    let mut queue = VecDeque::new();
    if queue.try_reserve_exact(graph.faces().len()).is_err() {
        return false;
    }
    queue.push_back(root);
    let width = profiles.len();
    let mut work = 0usize;
    while let Some(face) = queue.pop_front() {
        let Some(source_start) = face.checked_mul(width) else {
            return false;
        };
        for &(next, profile, sign, _) in &adjacency[face] {
            work = match work.checked_add(width) {
                Some(value) if value <= bounds.work => value,
                _ => return false,
            };
            let Some(target_start) = next.checked_mul(width) else {
                return false;
            };
            if !assigned[next] {
                for coordinate in 0..width {
                    potentials[target_start + coordinate] = potentials[source_start + coordinate];
                }
                if let Some(profile) = profile {
                    potentials[target_start + profile] =
                        match potentials[target_start + profile].checked_add(i32::from(sign)) {
                            Some(value) => value,
                            None => return false,
                        };
                } else if sign != 0 {
                    return false;
                }
                assigned[next] = true;
                queue.push_back(next);
            } else {
                for coordinate in 0..width {
                    let delta = if Some(coordinate) == profile {
                        i32::from(sign)
                    } else {
                        0
                    };
                    let expected = match potentials[source_start + coordinate].checked_add(delta) {
                        Some(value) => value,
                        None => return false,
                    };
                    if potentials[target_start + coordinate] != expected {
                        return false;
                    }
                }
            }
        }
    }
    work == bounds.work && assigned.into_iter().all(|value| value)
}

/// Exact all-parameter closure for a coaxial profile-lattice coboundary.
///
/// Every nonzero hinge rotates about one exact infinite carrier. The one
/// authenticated collective nonconstant profile and every distinct exact
/// nonzero constant angle are independent integer basis generators. Canonical
/// face potentials must differ by exactly the signed basis vector carried by
/// each hinge. For every parameter, rotations about one fixed carrier form an
/// abelian subgroup, so mapping the integer lattice to the actual rotations is
/// a homomorphism and every cycle closes. Different carriers, a second
/// unobserved nonconstant profile, sampled equality, tolerance, commutativity
/// across axes, and 180-degree periodic relations are all rejected.
pub(super) fn coaxial_profile_lattice_cycle_closure_premises_v1(
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
    let Some(graph) = authenticate_graph_v1(geometry, audit) else {
        return false;
    };
    let Some((labels, profiles)) = classify_coaxial_edges_v1(geometry, &graph, schedule) else {
        return false;
    };
    exact_integer_lattice_potential_v1(&graph, fixed_face, &labels, &profiles)
}

#[cfg(test)]
#[path = "coaxial_profile_lattice_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "coaxial_profile_lattice_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "coaxial_profile_lattice_limits_tests.rs"]
mod limits_tests;
