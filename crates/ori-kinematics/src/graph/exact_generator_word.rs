use std::collections::{HashMap, HashSet, VecDeque};

use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::{EdgeId, FaceId};
use ori_topology::FoldAssignment;

use super::MaterialHingeGraphAudit;
use crate::{
    CanonicalCycleScheduleV1, MaterialHingeGraphGeometry, Point3, TreeHinge,
    transform::{length, scale, subtract},
};

const MAX_EXACT_GENERATOR_WORD_FACES_V1: usize = 10_001;
const MAX_EXACT_GENERATOR_WORD_HINGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_EXACT_GENERATOR_WORD_ADJACENCY_ENTRIES_V1: usize = MAX_EXACT_GENERATOR_WORD_HINGES_V1 * 2;
const MAX_EXACT_GENERATOR_WORD_NODES_V1: usize = MAX_EXACT_GENERATOR_WORD_ADJACENCY_ENTRIES_V1 + 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct CanonicalInfiniteLineV1 {
    direction_bits: [u64; 3],
    exact_moment: [BigRational; 3],
}

pub(super) fn exact_plucker_components_v1(
    line: &CanonicalInfiniteLineV1,
) -> Option<([BigRational; 3], [BigRational; 3])> {
    let [Some(x), Some(y), Some(z)] = line
        .direction_bits
        .map(|value| BigRational::from_float(f64::from_bits(value)))
    else {
        return None;
    };
    Some(([x, y, z], line.exact_moment.clone()))
}

/// Proves that two exact Plücker lines meet at one point and have
/// perpendicular directions.
///
/// With `m = p × d`, the reciprocal product
/// `d₁·m₂ + d₂·m₁` vanishes exactly iff two nonparallel lines are coplanar.
/// Nonparallel coplanar lines have one intersection point. All operands are
/// exact rationals reconstructed from native binary64 geometry.
pub(super) fn exact_perpendicular_intersection_v1(
    first: &CanonicalInfiniteLineV1,
    second: &CanonicalInfiniteLineV1,
) -> bool {
    let Some((first_direction, first_moment)) = exact_plucker_components_v1(first) else {
        return false;
    };
    let Some((second_direction, second_moment)) = exact_plucker_components_v1(second) else {
        return false;
    };
    let dot = |left: &[BigRational; 3], right: &[BigRational; 3]| {
        &left[0] * &right[0] + &left[1] * &right[1] + &left[2] * &right[2]
    };
    let cross = [
        &first_direction[1] * &second_direction[2] - &first_direction[2] * &second_direction[1],
        &first_direction[2] * &second_direction[0] - &first_direction[0] * &second_direction[2],
        &first_direction[0] * &second_direction[1] - &first_direction[1] * &second_direction[0],
    ];
    if !dot(&first_direction, &second_direction).is_zero()
        || cross.iter().all(|value| value.is_zero())
    {
        return false;
    }
    let reciprocal = dot(&first_direction, &second_moment) + dot(&second_direction, &first_moment);
    reciprocal.is_zero()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum ExactGeneratorProfileV1 {
    CollectiveNonconstant,
    ConstantAngle(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct ExactGeneratorKeyV1 {
    line: CanonicalInfiniteLineV1,
    profile: ExactGeneratorProfileV1,
}

#[derive(Debug, Clone)]
pub(super) struct AuthenticatedHingeV1 {
    geometry_index: usize,
    left: usize,
    right: usize,
}

#[derive(Debug)]
pub(super) struct AuthenticatedGraphV1 {
    faces: Vec<FaceId>,
    hinges: Vec<AuthenticatedHingeV1>,
    adjacency_entry_limit: usize,
    word_node_limit: usize,
}

#[derive(Debug, Clone)]
struct PendingLabelV1 {
    left: usize,
    right: usize,
    edge: EdgeId,
    generator: Option<ExactGeneratorKeyV1>,
    generator_sign: i8,
}

impl AuthenticatedHingeV1 {
    pub(super) const fn geometry_index(&self) -> usize {
        self.geometry_index
    }

    pub(super) const fn left(&self) -> usize {
        self.left
    }

    pub(super) const fn right(&self) -> usize {
        self.right
    }
}

impl AuthenticatedGraphV1 {
    pub(super) fn faces(&self) -> &[FaceId] {
        &self.faces
    }

    pub(super) fn hinges(&self) -> &[AuthenticatedHingeV1] {
        &self.hinges
    }

    pub(super) const fn adjacency_entry_limit(&self) -> usize {
        self.adjacency_entry_limit
    }
}

#[derive(Debug, Clone, Copy)]
struct ReducedWordNodeV1 {
    prefix: usize,
    last: i32,
}

pub(super) struct ReducedWordInternerV1 {
    nodes: Vec<ReducedWordNodeV1>,
    // Hashing only locates candidates. Rust's HashMap confirms complete
    // `(prefix, signed generator)` key equality before a word ID is reused, so
    // a hash collision cannot become closure authority.
    by_extension: HashMap<(usize, i32), usize>,
    limit: usize,
}

impl ExactGeneratorKeyV1 {
    pub(super) fn new(line: CanonicalInfiniteLineV1, profile: ExactGeneratorProfileV1) -> Self {
        Self { line, profile }
    }
}

impl ReducedWordInternerV1 {
    pub(super) fn prepare(limit: usize) -> Option<Self> {
        if limit == 0 || limit > MAX_EXACT_GENERATOR_WORD_NODES_V1 {
            return None;
        }
        let extension_limit = limit.checked_sub(1)?;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(limit).ok()?;
        nodes.push(ReducedWordNodeV1 { prefix: 0, last: 0 });
        let mut by_extension = HashMap::new();
        by_extension.try_reserve(extension_limit).ok()?;
        Some(Self {
            nodes,
            by_extension,
            limit,
        })
    }

    pub(super) fn append(&mut self, word: usize, signed_generator: i32) -> Option<usize> {
        if signed_generator == 0 {
            return Some(word);
        }
        let current = *self.nodes.get(word)?;
        if word != 0 && current.last.checked_neg()? == signed_generator {
            return Some(current.prefix);
        }
        let key = (word, signed_generator);
        if let Some(existing) = self.by_extension.get(&key) {
            return Some(*existing);
        }
        if self.nodes.len() >= self.limit {
            return None;
        }
        let index = self.nodes.len();
        self.nodes.push(ReducedWordNodeV1 {
            prefix: word,
            last: signed_generator,
        });
        if self.by_extension.insert(key, index).is_some() {
            return None;
        }
        Some(index)
    }
}

fn bounded_exact_generator_word_counts_v1(
    face_count: usize,
    hinge_count: usize,
) -> Option<(usize, usize)> {
    if !(2..=MAX_EXACT_GENERATOR_WORD_FACES_V1).contains(&face_count)
        || !(1..=MAX_EXACT_GENERATOR_WORD_HINGES_V1).contains(&hinge_count)
    {
        return None;
    }
    let adjacency_entries = hinge_count.checked_mul(2)?;
    let word_nodes = adjacency_entries.checked_add(1)?;
    (adjacency_entries <= MAX_EXACT_GENERATOR_WORD_ADJACENCY_ENTRIES_V1
        && word_nodes <= MAX_EXACT_GENERATOR_WORD_NODES_V1)
        .then_some((adjacency_entries, word_nodes))
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

fn point_components_v1(point: Point3) -> [f64; 3] {
    [point.x(), point.y(), point.z()]
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

/// Returns one storage-invariant infinite carrier and the sign of the actual
/// left-to-right hinge generator relative to that carrier.
///
/// Native graph solving requires
/// `right = left * R(axis, assignment_sign * angle)`. Thus
/// `d = assignment_sign * axis` is the actual directed generator. Reversing
/// stored faces and segment direction negates both `d` and the graph edge
/// direction, leaving the physical constraint unchanged. We orient the
/// infinite line by the first nonzero component of `d`; a negative orientation
/// becomes the inverse free-group label. Consequently label negation maps to
/// the rigid-transform group inverse for every schedule parameter, not merely
/// at selected angles.
pub(super) fn exact_generator_line_v1(hinge: &TreeHinge) -> Option<(CanonicalInfiniteLineV1, i8)> {
    let axis = point_components_v1(hinge.axis());
    let delta = subtract(hinge.end(), hinge.start()).ok()?;
    let expected_axis = scale(delta, 1.0 / length(delta).ok()?).ok()?;
    if canonical_finite_vector_bits_v1(point_components_v1(expected_axis))?
        != canonical_finite_vector_bits_v1(axis)?
    {
        return None;
    }

    let assignment_sign = match hinge.assignment() {
        FoldAssignment::Mountain => 1.0,
        FoldAssignment::Valley => -1.0,
    };
    let directed = axis.map(|value| assignment_sign * value);
    let first_nonzero = directed.iter().copied().find(|value| *value != 0.0)?;
    let (canonical_direction, generator_sign) = if first_nonzero.is_sign_negative() {
        (directed.map(|value| -value), -1)
    } else {
        (directed, 1)
    };
    let direction_bits = canonical_finite_vector_bits_v1(canonical_direction)?;

    // For a fixed exact direction d, (p + t*d) x d = p x d. BigRational
    // arithmetic over the stored binary64 values therefore accepts different
    // finite start points on exactly the same infinite carrier without
    // allowing a subnormal/one-ULP parallel offset to collapse to that line.
    let exact_moment =
        exact_binary64_cross_v1(point_components_v1(hinge.start()), canonical_direction)?;
    Some((
        CanonicalInfiniteLineV1 {
            direction_bits,
            exact_moment,
        },
        generator_sign,
    ))
}

/// Classifies an already-proved exact constant public fold angle.
///
/// Public schedules admit only 0..=180 degrees. Both signed zeros are the
/// identity; every other finite bit pattern, including cardinal values and
/// values adjacent to 180 degrees, remains a distinct conservative generator.
/// In particular, the free reducer never exploits the special self-inverse
/// geometry of a 180-degree rotation.
fn exact_constant_profile_v1(angle_bits: u64) -> Option<Option<ExactGeneratorProfileV1>> {
    let angle = f64::from_bits(angle_bits);
    if !angle.is_finite() || !(0.0..=180.0).contains(&angle) {
        return None;
    }
    if angle == 0.0 {
        Some(None)
    } else {
        Some(Some(ExactGeneratorProfileV1::ConstantAngle(
            angle.to_bits(),
        )))
    }
}

pub(super) fn authenticate_graph_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
) -> Option<AuthenticatedGraphV1> {
    let (adjacency_entry_limit, word_node_limit) =
        bounded_exact_generator_word_counts_v1(geometry.face_ids().len(), geometry.hinges().len())?;
    if audit.closure_hinges().is_empty() {
        return None;
    }

    let mut faces = Vec::new();
    faces.try_reserve_exact(geometry.face_ids().len()).ok()?;
    faces.extend_from_slice(geometry.face_ids());
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if faces.windows(2).any(|pair| pair[0] == pair[1]) || faces != audit.faces() {
        return None;
    }
    let mut face_indices = HashMap::new();
    face_indices.try_reserve(faces.len()).ok()?;
    for (index, face) in faces.iter().copied().enumerate() {
        if face_indices.insert(face, index).is_some() {
            return None;
        }
    }

    let mut audit_edges = HashSet::new();
    audit_edges.try_reserve(geometry.hinges().len()).ok()?;
    if audit
        .spanning_hinges()
        .iter()
        .chain(audit.closure_hinges())
        .any(|edge| !audit_edges.insert(*edge))
        || audit_edges.len() != geometry.hinges().len()
    {
        return None;
    }

    let mut edge_order = Vec::new();
    edge_order.try_reserve_exact(geometry.hinges().len()).ok()?;
    edge_order.extend(0..geometry.hinges().len());
    edge_order.sort_unstable_by_key(|index| geometry.hinges()[*index].edge().canonical_bytes());
    let mut geometry_edges = HashSet::new();
    geometry_edges.try_reserve(geometry.hinges().len()).ok()?;
    let mut face_pairs = HashSet::new();
    face_pairs.try_reserve(geometry.hinges().len()).ok()?;
    let mut hinges = Vec::new();
    hinges.try_reserve_exact(geometry.hinges().len()).ok()?;
    for geometry_index in edge_order {
        let hinge = &geometry.hinges()[geometry_index];
        let left = *face_indices.get(&hinge.left_face())?;
        let right = *face_indices.get(&hinge.right_face())?;
        if left == right
            || !geometry_edges.insert(hinge.edge())
            || !audit_edges.contains(&hinge.edge())
            || !face_pairs.insert((left.min(right), left.max(right)))
        {
            return None;
        }
        hinges.push(AuthenticatedHingeV1 {
            geometry_index,
            left,
            right,
        });
    }
    if geometry_edges != audit_edges {
        return None;
    }
    Some(AuthenticatedGraphV1 {
        faces,
        hinges,
        adjacency_entry_limit,
        word_node_limit,
    })
}

fn classify_labels_v1(
    geometry: &MaterialHingeGraphGeometry,
    graph: &AuthenticatedGraphV1,
    schedule: &CanonicalCycleScheduleV1,
) -> Option<Vec<PendingLabelV1>> {
    // This observer proves exact profile identity inside one prepared
    // representation. We never equate polynomial and half-angle schedules by
    // sampled values, nor infer equality for any profile it does not return.
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
    labels.try_reserve_exact(graph.hinges.len()).ok()?;
    for record in &graph.hinges {
        let hinge = &geometry.hinges()[record.geometry_index];
        let edge = hinge.edge();
        let exact_constant = schedule.is_exact_constant_profile_v1(edge);
        let profile = if moving.contains(&edge) {
            if exact_constant {
                return None;
            }
            Some(ExactGeneratorProfileV1::CollectiveNonconstant)
        } else {
            if !exact_constant {
                return None;
            }
            exact_constant_profile_v1(*initial_by_edge.get(&edge)?)?
        };
        schedule.derivative_bound(edge)?;
        let (generator, generator_sign) = if let Some(profile) = profile {
            let (line, sign) = exact_generator_line_v1(hinge)?;
            (Some(ExactGeneratorKeyV1 { line, profile }), sign)
        } else {
            (None, 0)
        };
        labels.push(PendingLabelV1 {
            left: record.left,
            right: record.right,
            edge,
            generator,
            generator_sign,
        });
    }
    (labels
        .iter()
        .filter(|label| {
            label.generator.as_ref().is_some_and(|generator| {
                generator.profile == ExactGeneratorProfileV1::CollectiveNonconstant
            })
        })
        .count()
        == moving.len())
    .then_some(labels)
}

fn exact_free_group_potential_v1(
    graph: &AuthenticatedGraphV1,
    fixed_face: FaceId,
    labels: &[PendingLabelV1],
) -> bool {
    if labels.len() != graph.hinges.len() {
        return false;
    }
    let Some(root) = graph
        .faces
        .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
        .ok()
    else {
        return false;
    };

    let mut generators = Vec::new();
    if generators.try_reserve_exact(labels.len()).is_err() {
        return false;
    }
    generators.extend(
        labels
            .iter()
            .filter_map(|label| label.generator.as_ref().cloned()),
    );
    generators.sort_unstable();
    generators.dedup();
    if generators.len() > i32::MAX as usize {
        return false;
    }

    let mut degrees = Vec::new();
    if degrees.try_reserve_exact(graph.faces.len()).is_err() {
        return false;
    }
    degrees.resize(graph.faces.len(), 0usize);
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
    if degree_sum != graph.adjacency_entry_limit {
        return false;
    }
    let mut adjacency = Vec::new();
    if adjacency.try_reserve_exact(graph.faces.len()).is_err() {
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
        let signed_generator = if let Some(generator) = &label.generator {
            let Ok(index) = generators.binary_search(generator) else {
                return false;
            };
            let Some(one_based) = index.checked_add(1) else {
                return false;
            };
            let Ok(identifier) = i32::try_from(one_based) else {
                return false;
            };
            match identifier.checked_mul(i32::from(label.generator_sign)) {
                Some(value) if value != 0 => value,
                _ => return false,
            }
        } else {
            0
        };
        adjacency[label.left].push((label.right, signed_generator, label.edge));
        adjacency[label.right].push((
            label.left,
            match signed_generator.checked_neg() {
                Some(value) => value,
                None => return false,
            },
            label.edge,
        ));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(face, generator, edge)| {
            (
                graph.faces[*face].canonical_bytes(),
                *generator,
                edge.canonical_bytes(),
            )
        });
    }

    let mut interner = match ReducedWordInternerV1::prepare(graph.word_node_limit) {
        Some(interner) => interner,
        None => return false,
    };
    let mut words = Vec::new();
    if words.try_reserve_exact(graph.faces.len()).is_err() {
        return false;
    }
    words.resize(graph.faces.len(), None);
    words[root] = Some(0usize);
    let mut queue = VecDeque::new();
    if queue.try_reserve_exact(graph.faces.len()).is_err() {
        return false;
    }
    queue.push_back(root);
    let mut work = 0usize;
    while let Some(face) = queue.pop_front() {
        let Some(word) = words[face] else {
            return false;
        };
        for &(next, signed_generator, _) in &adjacency[face] {
            work = match work.checked_add(1) {
                Some(value) if value <= graph.adjacency_entry_limit => value,
                _ => return false,
            };
            let Some(expected) = interner.append(word, signed_generator) else {
                return false;
            };
            if let Some(existing) = words[next] {
                if existing != expected {
                    return false;
                }
            } else {
                words[next] = Some(expected);
                queue.push_back(next);
            }
        }
    }
    work == graph.adjacency_entry_limit && words.into_iter().all(|word| word.is_some())
}

/// Exact all-parameter closure for a graph whose hinge labels are a
/// free-group coboundary.
///
/// Each exact carrier/profile pair is an abstract generator and exact-zero
/// hinges are the identity. A canonical reduced word is assigned to every
/// face, and every material hinge must append exactly its signed generator.
/// For any schedule parameter, mapping abstract generators to their actual
/// rigid rotations is a group homomorphism; mapped face words therefore
/// satisfy every hinge simultaneously. Only adjacent inverse cancellation is
/// admitted. No rotation commutativity, 180-degree special relation, sampled
/// solve, or tolerance-based geometric comparison enters the proof.
pub(super) fn exact_generator_word_cycle_closure_premises_v1(
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
    let Some(labels) = classify_labels_v1(geometry, &graph, schedule) else {
        return false;
    };
    exact_free_group_potential_v1(&graph, fixed_face, &labels)
}

#[cfg(test)]
#[path = "exact_generator_word_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "exact_generator_word_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "exact_generator_word_limits_tests.rs"]
mod limits_tests;
