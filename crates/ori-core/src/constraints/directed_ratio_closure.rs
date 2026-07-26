use std::collections::BTreeMap;

use super::{
    CanonicalId, ConstraintId, DirectConstraintConflictKindV1, DirectConstraintConflictV1, EdgeId,
    MAX_DIRECT_CONFLICT_CAUSE_IDS_V1, ScalarAssignment, ScalarGroupSummary,
    bounded_zero_closure::{Checkpoint, Observer, ObserverControl, Phase, UnknownReason},
    canonical_id_slice_cmp, canonicalize_constraint_ids, consistent_scalar_assignment,
    length_ratio_residual_binary64_v1, length_ratio_scaled_denominator_binary64_v1,
};

pub(super) const MAX_DIRECTED_RATIO_CLOSURE_WORK_V1: u64 = 2_000_000;
pub(super) const MAX_DIRECTED_RATIO_CLOSURE_STORAGE_UNITS_V1: usize = 1_048_576;

const OBSERVER_WORK_INTERVAL_V1: u64 = 128;
const MAX_FORCED_PATH_RATIO_IDS_V1: usize = MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Limits {
    pub max_work: u64,
    pub max_storage_units: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: MAX_DIRECTED_RATIO_CLOSURE_WORK_V1,
            max_storage_units: MAX_DIRECTED_RATIO_CLOSURE_STORAGE_UNITS_V1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stats {
    pub completed_work: u64,
    pub peak_storage_units: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Proven(DirectConstraintConflictV1),
    NoProof,
    Unknown { reason: UnknownReason, stats: Stats },
}

#[derive(Clone, Copy)]
struct Arc {
    numerator: CanonicalId,
    constraint_id: ConstraintId,
    ratio: f64,
}

#[derive(Clone)]
struct ForcedValue {
    value: f64,
    ratio_ids: Vec<ConstraintId>,
}

struct Candidate {
    constraint_ids: Vec<ConstraintId>,
    fixed_edge: CanonicalId,
    storage_units: usize,
}

struct Budget<'a, O> {
    observer: &'a mut O,
    limits: Limits,
    completed_work: u64,
    next_checkpoint: u64,
    storage_units: usize,
    peak_storage_units: usize,
}

impl<O: Observer> Budget<'_, O> {
    fn stats(&self) -> Stats {
        Stats {
            completed_work: self.completed_work,
            peak_storage_units: self.peak_storage_units,
        }
    }

    fn checkpoint(&mut self, phase: Phase) -> Result<(), UnknownReason> {
        match self.observer.checkpoint(Checkpoint {
            phase,
            completed_work: self.completed_work,
            reserved_storage_units: self.storage_units,
        }) {
            ObserverControl::Continue => Ok(()),
            ObserverControl::Cancelled => Err(UnknownReason::Cancelled),
            ObserverControl::DeadlineReached => Err(UnknownReason::DeadlineReached),
        }
    }

    fn work(&mut self, phase: Phase, amount: u64) -> Result<(), UnknownReason> {
        self.completed_work = self
            .completed_work
            .checked_add(amount)
            .ok_or(UnknownReason::WorkLimitExceeded)?;
        if self.completed_work > self.limits.max_work {
            return Err(UnknownReason::WorkLimitExceeded);
        }
        if self.completed_work >= self.next_checkpoint {
            while self.next_checkpoint <= self.completed_work {
                self.next_checkpoint = self
                    .next_checkpoint
                    .checked_add(OBSERVER_WORK_INTERVAL_V1)
                    .ok_or(UnknownReason::WorkLimitExceeded)?;
            }
            self.checkpoint(phase)?;
        }
        Ok(())
    }

    fn reserve(&mut self, amount: usize) -> Result<(), UnknownReason> {
        self.storage_units = self
            .storage_units
            .checked_add(amount)
            .ok_or(UnknownReason::StorageLimitExceeded)?;
        self.peak_storage_units = self.peak_storage_units.max(self.storage_units);
        (self.storage_units <= self.limits.max_storage_units)
            .then_some(())
            .ok_or(UnknownReason::StorageLimitExceeded)
    }

    fn restore_storage(&mut self, amount: usize) {
        debug_assert!(amount <= self.storage_units);
        self.storage_units = amount;
    }

    fn release(&mut self, amount: usize) {
        debug_assert!(amount <= self.storage_units);
        self.storage_units -= amount;
    }
}

pub(super) fn conflict(
    ratios: &BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
    observer: &mut impl Observer,
) -> Outcome {
    #[cfg(test)]
    let limits = super::directed_ratio_test_limits_v1();
    #[cfg(not(test))]
    let limits = Limits::default();
    conflict_with_limits_and_observer(ratios, fixed_lengths, edge_ids, limits, observer).0
}

/// Finds a contradiction using only directed production binary64 derivations.
///
/// Each consistent positive finite fixed length is considered as an independent
/// root. A `LengthRatio` arc is traversed only from denominator to numerator.
/// New finite values are derived through the shared multiplication helper once;
/// a closing arc is accepted only when the shared production residual is
/// non-zero or non-finite. Different fixed roots are never joined.
///
/// Logical storage accounts for graph nodes/arcs, the retained root list,
/// forced paths, frontier/proposal keys, the best witness, and every temporary
/// path or witness-union buffer.
pub(super) fn conflict_with_limits_and_observer(
    ratios: &BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
    limits: Limits,
    observer: &mut impl Observer,
) -> (Outcome, Stats) {
    let mut budget = Budget {
        observer,
        limits,
        completed_work: 0,
        next_checkpoint: OBSERVER_WORK_INTERVAL_V1,
        storage_units: 0,
        peak_storage_units: 0,
    };
    if let Err(reason) = budget.checkpoint(Phase::GraphBuild) {
        return unknown(reason, &budget);
    }

    let mut graph = BTreeMap::<CanonicalId, Vec<Arc>>::new();
    for ((numerator, denominator), assignments) in ratios {
        // One group unit plus every assignment visited by consistency scanning.
        let Some(group_work) = u64::try_from(assignments.len())
            .ok()
            .and_then(|length| length.checked_add(1))
        else {
            return unknown(UnknownReason::WorkLimitExceeded, &budget);
        };
        if let Err(reason) = budget.work(Phase::GraphBuild, group_work) {
            return unknown(reason, &budget);
        }
        let Some(assignment) = consistent_scalar_assignment(assignments) else {
            continue;
        };
        if !assignment.value.is_finite() || assignment.value <= 0.0 {
            continue;
        }
        if !graph.contains_key(denominator)
            && let Err(reason) = budget.reserve(1)
        {
            return unknown(reason, &budget);
        }
        graph.entry(*denominator).or_default();
        if !graph.contains_key(numerator)
            && let Err(reason) = budget.reserve(1)
        {
            return unknown(reason, &budget);
        }
        graph.entry(*numerator).or_default();
        if let Err(reason) = budget.reserve(1) {
            return unknown(reason, &budget);
        }
        graph
            .get_mut(denominator)
            .expect("inserted denominator")
            .push(Arc {
                numerator: *numerator,
                constraint_id: assignment.id,
                ratio: assignment.value,
            });
    }
    for arcs in graph.values_mut() {
        arcs.sort_unstable_by_key(|arc| (arc.numerator, arc.constraint_id.canonical_bytes()));
    }
    let mut roots = fixed_lengths
        .iter()
        .filter_map(|(edge, summary)| {
            let fixed = summary.consistent_assignment()?;
            (fixed.value.is_finite() && fixed.value > 0.0 && graph.contains_key(edge))
                .then_some((*edge, fixed))
        })
        .collect::<Vec<_>>();
    if let Err(reason) = budget.reserve(roots.len()) {
        return unknown(reason, &budget);
    }
    roots.sort_unstable_by_key(|(edge, fixed)| (fixed.id.canonical_bytes(), *edge));
    let base_storage = budget.storage_units;

    let mut best = None;
    for (fixed_edge, fixed) in roots {
        if let Err(reason) = budget.checkpoint(Phase::ProofSearch) {
            return unknown(reason, &budget);
        }
        // One unit owns the forced-map entry and one owns its frontier key.
        if let Err(reason) = budget.reserve(2) {
            return unknown(reason, &budget);
        }
        let mut forced = BTreeMap::from([(
            fixed_edge,
            ForcedValue {
                value: fixed.value,
                ratio_ids: Vec::new(),
            },
        )]);
        let mut frontier = vec![fixed_edge];

        while !frontier.is_empty() {
            frontier.sort_unstable_by(|left, right| {
                canonical_id_slice_cmp(&forced[left].ratio_ids, &forced[right].ratio_ids)
                    .then_with(|| left.cmp(right))
            });
            let mut proposals = BTreeMap::<CanonicalId, ForcedValue>::new();
            for denominator in &frontier {
                let Some(parent) = forced.get(denominator) else {
                    return unknown(UnknownReason::StorageLimitExceeded, &budget);
                };
                for arc in graph.get(denominator).into_iter().flatten() {
                    if let Err(reason) = budget.work(Phase::ProofSearch, 1) {
                        return unknown(reason, &budget);
                    }
                    if let Some(existing) = forced.get(&arc.numerator) {
                        let residual = length_ratio_residual_binary64_v1(
                            existing.value,
                            arc.ratio,
                            parent.value,
                        );
                        if residual != 0.0
                            && let Err(reason) = consider_candidate(
                                &mut best,
                                fixed_edge,
                                fixed.id,
                                &existing.ratio_ids,
                                &parent.ratio_ids,
                                arc.constraint_id,
                                &mut budget,
                            )
                        {
                            return unknown(reason, &budget);
                        }
                        continue;
                    }

                    if parent.ratio_ids.len() >= MAX_FORCED_PATH_RATIO_IDS_V1 {
                        continue;
                    }
                    if let Some(existing) = proposals.get(&arc.numerator) {
                        let residual = length_ratio_residual_binary64_v1(
                            existing.value,
                            arc.ratio,
                            parent.value,
                        );
                        if residual != 0.0
                            && let Err(reason) = consider_candidate(
                                &mut best,
                                fixed_edge,
                                fixed.id,
                                &existing.ratio_ids,
                                &parent.ratio_ids,
                                arc.constraint_id,
                                &mut budget,
                            )
                        {
                            return unknown(reason, &budget);
                        }
                        let ratio_ids = match extended_path(
                            &parent.ratio_ids,
                            arc.constraint_id,
                            &mut budget,
                        ) {
                            Ok(ratio_ids) => ratio_ids,
                            Err(reason) => return unknown(reason, &budget),
                        };
                        let scratch_units = ratio_ids.len();
                        let replace =
                            canonical_id_slice_cmp(&ratio_ids, &existing.ratio_ids).is_lt();
                        if replace {
                            let derived = if residual == 0.0 {
                                existing.value
                            } else {
                                length_ratio_scaled_denominator_binary64_v1(arc.ratio, parent.value)
                            };
                            if derived.is_finite() {
                                let old_path_units = existing.ratio_ids.len();
                                proposals.insert(
                                    arc.numerator,
                                    ForcedValue {
                                        value: derived,
                                        ratio_ids,
                                    },
                                );
                                budget.release(old_path_units);
                            } else {
                                budget.release(scratch_units);
                            }
                        } else {
                            budget.release(scratch_units);
                        }
                        continue;
                    }

                    let derived =
                        length_ratio_scaled_denominator_binary64_v1(arc.ratio, parent.value);
                    if !derived.is_finite() {
                        continue;
                    }
                    let ratio_ids =
                        match extended_path(&parent.ratio_ids, arc.constraint_id, &mut budget) {
                            Ok(ratio_ids) => ratio_ids,
                            Err(reason) => return unknown(reason, &budget),
                        };
                    // The path is already charged; these own the proposal-map
                    // key and its eventual frontier key.
                    if let Err(reason) = budget.reserve(2) {
                        return unknown(reason, &budget);
                    }
                    proposals.insert(
                        arc.numerator,
                        ForcedValue {
                            value: derived,
                            ratio_ids,
                        },
                    );
                }
            }
            budget.release(frontier.len());
            let next_frontier = proposals.keys().copied().collect::<Vec<_>>();
            forced.extend(proposals);
            frontier = next_frontier;
        }
        let retained_storage = base_storage
            + best
                .as_ref()
                .map_or(0, |candidate: &Candidate| candidate.storage_units);
        budget.restore_storage(retained_storage);
    }

    let stats = budget.stats();
    let outcome = best.map_or(Outcome::NoProof, |candidate: Candidate| {
        let ratio_constraint_count = u16::try_from(candidate.constraint_ids.len() - 1)
            .expect("a bounded witness count fits u16");
        Outcome::Proven(DirectConstraintConflictV1 {
            conflict: DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
                fixed_edge: edge_ids[&candidate.fixed_edge],
                ratio_constraint_count,
            },
            constraint_ids: candidate.constraint_ids,
        })
    });
    (outcome, stats)
}

#[allow(clippy::too_many_arguments)]
fn consider_candidate(
    best: &mut Option<Candidate>,
    fixed_edge: CanonicalId,
    fixed_id: ConstraintId,
    existing_path: &[ConstraintId],
    closing_parent_path: &[ConstraintId],
    closing_id: ConstraintId,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<(), UnknownReason> {
    let storage_before = budget.storage_units;
    let capacity = existing_path
        .len()
        .checked_add(closing_parent_path.len())
        .and_then(|length| length.checked_add(2))
        .ok_or(UnknownReason::StorageLimitExceeded)?;
    budget.reserve(capacity)?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(capacity)
        .map_err(|_| UnknownReason::StorageLimitExceeded)?;
    ids.extend_from_slice(existing_path);
    ids.extend_from_slice(closing_parent_path);
    ids.extend([closing_id, fixed_id]);
    let sort_factor = usize::BITS - ids.len().leading_zeros();
    let work = u64::try_from(ids.len())
        .ok()
        .and_then(|length| length.checked_mul(u64::from(sort_factor) + 1))
        .ok_or(UnknownReason::WorkLimitExceeded)?;
    budget.work(Phase::ProofSearch, work)?;
    canonicalize_constraint_ids(&mut ids);
    let admissible = ids
        .len()
        .checked_sub(1)
        .is_some_and(|ratio_count| (3..MAX_DIRECT_CONFLICT_CAUSE_IDS_V1).contains(&ratio_count));
    let replace = admissible
        && best.as_ref().is_none_or(|current| {
            ids.len() < current.constraint_ids.len()
                || (ids.len() == current.constraint_ids.len()
                    && canonical_id_slice_cmp(&ids, &current.constraint_ids)
                        .then_with(|| fixed_edge.cmp(&current.fixed_edge))
                        .is_lt())
        });
    if replace {
        let old_storage = best.as_ref().map_or(0, |candidate| candidate.storage_units);
        *best = Some(Candidate {
            constraint_ids: ids,
            fixed_edge,
            storage_units: capacity,
        });
        budget.release(old_storage);
    } else {
        budget.restore_storage(storage_before);
    }
    Ok(())
}

fn insert_canonical_id(ids: &mut Vec<ConstraintId>, id: ConstraintId) {
    let key = id.canonical_bytes();
    let index = ids
        .binary_search_by_key(&key, ConstraintId::canonical_bytes)
        .unwrap_or_else(|index| index);
    ids.insert(index, id);
}

fn extended_path(
    parent: &[ConstraintId],
    closing_id: ConstraintId,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<Vec<ConstraintId>, UnknownReason> {
    let length = parent
        .len()
        .checked_add(1)
        .ok_or(UnknownReason::StorageLimitExceeded)?;
    budget.reserve(length)?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(length)
        .map_err(|_| UnknownReason::StorageLimitExceeded)?;
    ids.extend_from_slice(parent);
    insert_canonical_id(&mut ids, closing_id);
    Ok(ids)
}

fn unknown(reason: UnknownReason, budget: &Budget<'_, impl Observer>) -> (Outcome, Stats) {
    let stats = budget.stats();
    (Outcome::Unknown { reason, stats }, stats)
}
