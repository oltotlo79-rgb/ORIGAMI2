use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    CanonicalId, ConstraintId, DirectConstraintConflictKindV1, DirectConstraintConflictV1, EdgeId,
    EdgePairKey, GeometricConstraintSetV1, MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1, ScalarAssignment,
    ScalarGroupSummary, canonical_id_slice_cmp, canonicalize_constraint_ids,
};

type Graph = BTreeMap<CanonicalId, Vec<(CanonicalId, ConstraintId)>>;

struct Candidate {
    constraint_ids: Vec<ConstraintId>,
    fixed_edge: CanonicalId,
    forced_zero_edge: CanonicalId,
    horizontal_constraint_count: u16,
    vertical_constraint_count: u16,
    zero_propagation_constraint_count: u16,
}

/// Proves an exact-zero contradiction over a bounded five-kind subset.
///
/// The proof mirrors the operation order in `constraint_solver::residuals`:
///
/// * a zero `Horizontal` residual makes the two endpoint y coordinates
///   numerically equal, while a zero `Vertical` residual does the same for x;
///   equality paths in both graphs therefore make the implemented `hypot`
///   length of `forced_zero_edge` exactly zero;
/// * from a zero length, a zero `EqualLength` residual propagates zero in both
///   directions;
/// * from a zero denominator length, the implemented `LengthRatio` order is
///   `ratio * 0.0` followed by `numerator - 0.0`, so every admitted positive
///   finite ratio propagates zero only from denominator to numerator;
/// * the implemented `FixedLength` residual at zero is `0.0 - length_mm`,
///   which is non-zero for every admitted positive finite value, including the
///   smallest subnormal.
///
/// No converse ratio implication is used: a small positive ratio can underflow
/// a non-zero subnormal denominator to zero. Likewise, current coordinates,
/// solver tolerance, and numerical-solver failure are never premises. The
/// theorem concerns exact zero of the production binary64 residuals.
pub(super) fn conflict(
    set: &GeometricConstraintSetV1<'_>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    horizontal: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    vertical: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    equal_lengths: &BTreeMap<EdgePairKey, Vec<ConstraintId>>,
    ratios: &BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
) -> Result<Option<DirectConstraintConflictV1>, ()> {
    // This first production slice is intentionally bounded by the same limit
    // as subset minimization. Larger documents remain solver-required instead
    // of turning a skipped graph proof into a false negative or an unbounded
    // preflight cost.
    if set.constraints.len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
        return Ok(None);
    }
    if fixed_lengths.is_empty() || horizontal.is_empty() || vertical.is_empty() {
        return Ok(None);
    }

    let relevant_edges = edge_ids.keys().copied().collect::<BTreeSet<_>>();
    let mut endpoints = BTreeMap::new();
    for edge in &set.source_pattern.edges {
        let key = edge.id.canonical_bytes();
        if relevant_edges.contains(&key) {
            endpoints.insert(
                key,
                (edge.start.canonical_bytes(), edge.end.canonical_bytes()),
            );
        }
    }
    if endpoints.len() != relevant_edges.len() {
        // Preparation has already validated every reference. Keep this
        // defensive branch fail-closed if that invariant is ever refactored.
        return Err(());
    }

    let horizontal_graph = orientation_equality_graph(horizontal, &endpoints)?;
    let vertical_graph = orientation_equality_graph(vertical, &endpoints)?;
    let mut propagation_graph = Graph::new();
    for (pair, ids) in equal_lengths {
        let id = ids
            .iter()
            .min_by_key(|id| id.canonical_bytes())
            .copied()
            .ok_or(())?;
        add_arc(&mut propagation_graph, pair.first, pair.second, id);
        add_arc(&mut propagation_graph, pair.second, pair.first, id);
    }
    for ((numerator, denominator), assignments) in ratios {
        let id = assignments
            .iter()
            .min_by_key(|assignment| assignment.id.canonical_bytes())
            .map(|assignment| assignment.id)
            .ok_or(())?;
        // Only this implication direction is universally sound in binary64.
        add_arc(&mut propagation_graph, *denominator, *numerator, id);
    }
    canonicalize_graph(&mut propagation_graph);

    let mut best: Option<Candidate> = None;
    for forced_zero_edge in relevant_edges.iter().copied() {
        let (start, end) = endpoints[&forced_zero_edge];
        let Some(horizontal_path) = canonical_path(start, end, &horizontal_graph) else {
            continue;
        };
        let Some(vertical_path) = canonical_path(start, end, &vertical_graph) else {
            continue;
        };
        // A real edge has distinct endpoint IDs, so both proof paths contain at
        // least one orientation constraint.
        if horizontal_path.is_empty() || vertical_path.is_empty() {
            return Err(());
        }

        for (fixed_edge, summary) in fixed_lengths {
            let Some(fixed) = summary.consistent_assignment() else {
                // An inconsistent fixed group is handled by the earlier direct
                // theorem, and must never be simplified to one assignment here.
                continue;
            };
            let Some(propagation_path) =
                canonical_path(forced_zero_edge, *fixed_edge, &propagation_graph)
            else {
                continue;
            };
            let mut constraint_ids = Vec::new();
            constraint_ids.extend(horizontal_path.iter().copied());
            constraint_ids.extend(vertical_path.iter().copied());
            constraint_ids.extend(propagation_path.iter().copied());
            constraint_ids.push(fixed.id);
            canonicalize_constraint_ids(&mut constraint_ids);

            let horizontal_constraint_count =
                u16::try_from(horizontal_path.len()).map_err(|_| ())?;
            let vertical_constraint_count = u16::try_from(vertical_path.len()).map_err(|_| ())?;
            let zero_propagation_constraint_count =
                u16::try_from(propagation_path.len()).map_err(|_| ())?;
            let expected_length = horizontal_path
                .len()
                .checked_add(vertical_path.len())
                .and_then(|count| count.checked_add(propagation_path.len()))
                .and_then(|count| count.checked_add(1))
                .ok_or(())?;
            if constraint_ids.len() != expected_length
                || constraint_ids.len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1
            {
                return Err(());
            }

            let candidate = Candidate {
                constraint_ids,
                fixed_edge: *fixed_edge,
                forced_zero_edge,
                horizontal_constraint_count,
                vertical_constraint_count,
                zero_propagation_constraint_count,
            };
            if best.as_ref().is_none_or(|current| {
                candidate.constraint_ids.len() < current.constraint_ids.len()
                    || (candidate.constraint_ids.len() == current.constraint_ids.len()
                        && (canonical_id_slice_cmp(
                            &candidate.constraint_ids,
                            &current.constraint_ids,
                        )
                        .is_lt()
                            || (candidate.constraint_ids == current.constraint_ids
                                && (
                                    candidate.fixed_edge,
                                    candidate.forced_zero_edge,
                                    candidate.horizontal_constraint_count,
                                    candidate.vertical_constraint_count,
                                    candidate.zero_propagation_constraint_count,
                                ) < (
                                    current.fixed_edge,
                                    current.forced_zero_edge,
                                    current.horizontal_constraint_count,
                                    current.vertical_constraint_count,
                                    current.zero_propagation_constraint_count,
                                ))))
            }) {
                best = Some(candidate);
            }
        }
    }

    let Some(best) = best else {
        return Ok(None);
    };
    Ok(Some(DirectConstraintConflictV1 {
        conflict: DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
            fixed_edge: edge_ids[&best.fixed_edge],
            forced_zero_edge: edge_ids[&best.forced_zero_edge],
            horizontal_constraint_count: best.horizontal_constraint_count,
            vertical_constraint_count: best.vertical_constraint_count,
            zero_propagation_constraint_count: best.zero_propagation_constraint_count,
        },
        constraint_ids: best.constraint_ids,
    }))
}

fn orientation_equality_graph(
    constraints: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    endpoints: &BTreeMap<CanonicalId, (CanonicalId, CanonicalId)>,
) -> Result<Graph, ()> {
    let mut graph = Graph::new();
    for (edge, ids) in constraints {
        let (start, end) = endpoints.get(edge).copied().ok_or(())?;
        let id = ids
            .iter()
            .min_by_key(|id| id.canonical_bytes())
            .copied()
            .ok_or(())?;
        add_arc(&mut graph, start, end, id);
        add_arc(&mut graph, end, start, id);
    }
    canonicalize_graph(&mut graph);
    Ok(graph)
}

fn add_arc(graph: &mut Graph, start: CanonicalId, end: CanonicalId, id: ConstraintId) {
    graph.entry(start).or_default().push((end, id));
}

fn canonicalize_graph(graph: &mut Graph) {
    for arcs in graph.values_mut() {
        arcs.sort_unstable_by_key(|(neighbor, id)| (id.canonical_bytes(), *neighbor));
        arcs.dedup();
    }
}

fn canonical_path(
    start: CanonicalId,
    target: CanonicalId,
    graph: &Graph,
) -> Option<Vec<ConstraintId>> {
    if start == target {
        return Some(Vec::new());
    }
    let mut parents = BTreeMap::new();
    let mut visited = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        for (neighbor, id) in graph.get(&node).into_iter().flatten() {
            if !visited.insert(*neighbor) {
                continue;
            }
            parents.insert(*neighbor, (node, *id));
            if *neighbor == target {
                let mut path = Vec::new();
                let mut cursor = target;
                while cursor != start {
                    let (parent, constraint) = parents.get(&cursor).copied()?;
                    path.push(constraint);
                    cursor = parent;
                }
                path.reverse();
                return Some(path);
            }
            queue.push_back(*neighbor);
        }
    }
    None
}
