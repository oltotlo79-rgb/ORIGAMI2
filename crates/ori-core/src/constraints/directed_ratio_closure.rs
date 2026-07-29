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
const MAX_NONNEGATIVE_FINITE_BITS_V1: u64 = f64::MAX.to_bits();

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

#[derive(Clone, Copy)]
struct ReverseArc {
    denominator: CanonicalId,
    constraint_id: ConstraintId,
    ratio: f64,
}

#[derive(Clone)]
struct ForcedValue {
    value: f64,
    ratio_ids: Vec<ConstraintId>,
}

/// Conservative non-negative finite binary64 values reachable from one fixed
/// root through one canonical ratio path.
///
/// The bit interval is ordered numerically because every admitted value has a
/// clear sign bit. It may contain values that are not produced by the path;
/// that deliberate over-approximation makes an empty intersection a sound
/// contradiction while an overlap remains inconclusive.
#[derive(Clone)]
struct ForcedDomain {
    lower_bits: u64,
    upper_bits: u64,
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

/// Finds a contradiction using only production binary64 derivations.
///
/// Each consistent positive finite fixed length is considered as an independent
/// root. The exact-value phase traverses `LengthRatio` arcs from denominator to
/// numerator through the shared multiplication helper. The conservative-domain
/// phase may also traverse an arc backwards, but never divides: it searches the
/// ordered non-negative finite binary64 bit domain for the complete interval
/// whose production multiplication can land in the current numerator domain.
/// Underflow plateaus, overflow, and rounding aliases therefore remain admitted.
/// A cycle is contradictory only when two conservative domains are disjoint.
/// Different fixed roots are never joined.
///
/// Logical storage accounts for graph nodes and both arc orientations, the
/// retained root list, exact values or conservative domains, frontier/proposal
/// keys, the best witness, and every temporary path or witness-union buffer.
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
    let mut reverse_graph = BTreeMap::<CanonicalId, Vec<ReverseArc>>::new();
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
        if !reverse_graph.contains_key(numerator)
            && let Err(reason) = budget.reserve(1)
        {
            return unknown(reason, &budget);
        }
        reverse_graph.entry(*numerator).or_default();
        if let Err(reason) = budget.reserve(1) {
            return unknown(reason, &budget);
        }
        reverse_graph
            .get_mut(numerator)
            .expect("inserted numerator")
            .push(ReverseArc {
                denominator: *denominator,
                constraint_id: assignment.id,
                ratio: assignment.value,
            });
    }
    for arcs in graph.values_mut() {
        arcs.sort_unstable_by_key(|arc| (arc.numerator, arc.constraint_id.canonical_bytes()));
    }
    for arcs in reverse_graph.values_mut() {
        arcs.sort_unstable_by_key(|arc| (arc.denominator, arc.constraint_id.canonical_bytes()));
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
    for &(fixed_edge, fixed) in &roots {
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
            let mut next_frontier = Vec::new();
            if next_frontier.try_reserve_exact(proposals.len()).is_err() {
                return unknown(UnknownReason::StorageLimitExceeded, &budget);
            }
            next_frontier.extend(proposals.keys().copied());
            forced.extend(proposals);
            frontier = next_frontier;
        }
        let retained_storage = base_storage
            + best
                .as_ref()
                .map_or(0, |candidate: &Candidate| candidate.storage_units);
        budget.restore_storage(retained_storage);
    }

    for &(fixed_edge, fixed) in &roots {
        if let Err(reason) = budget.checkpoint(Phase::ProofSearch) {
            return unknown(reason, &budget);
        }
        if let Err(reason) = search_bidirectional_domain_cycles(
            fixed_edge,
            fixed,
            &graph,
            &reverse_graph,
            &mut best,
            &mut budget,
        ) {
            return unknown(reason, &budget);
        }
        let retained_storage = base_storage
            + best
                .as_ref()
                .map_or(0, |candidate: &Candidate| candidate.storage_units);
        budget.restore_storage(retained_storage);
    }

    if let Err(reason) = budget.checkpoint(Phase::Complete) {
        return unknown(reason, &budget);
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

fn search_bidirectional_domain_cycles(
    fixed_edge: CanonicalId,
    fixed: ScalarAssignment,
    graph: &BTreeMap<CanonicalId, Vec<Arc>>,
    reverse_graph: &BTreeMap<CanonicalId, Vec<ReverseArc>>,
    best: &mut Option<Candidate>,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<(), UnknownReason> {
    // One unit owns the domain-map entry and one owns its frontier key.
    budget.reserve(2)?;
    let fixed_bits = fixed.value.to_bits();
    let mut domains = BTreeMap::from([(
        fixed_edge,
        ForcedDomain {
            lower_bits: fixed_bits,
            upper_bits: fixed_bits,
            ratio_ids: Vec::new(),
        },
    )]);
    let mut frontier = vec![fixed_edge];

    while !frontier.is_empty() {
        frontier.sort_unstable_by(|left, right| {
            canonical_id_slice_cmp(&domains[left].ratio_ids, &domains[right].ratio_ids)
                .then_with(|| left.cmp(right))
        });
        let mut proposals = BTreeMap::<CanonicalId, ForcedDomain>::new();
        for source in &frontier {
            let Some(parent) = domains.get(source) else {
                return Err(UnknownReason::StorageLimitExceeded);
            };
            for arc in graph.get(source).into_iter().flatten() {
                budget.work(Phase::ProofSearch, 1)?;
                let Some((lower_bits, upper_bits)) = forward_domain(parent, arc.ratio, budget)?
                else {
                    // An empty image would itself be a sound contradiction, but
                    // the stable wire family requires a cycle of at least three
                    // ratio records. Leave that smaller, currently
                    // unrepresentable theorem fail-closed.
                    continue;
                };
                visit_domain_step(
                    fixed_edge,
                    fixed.id,
                    arc.numerator,
                    arc.constraint_id,
                    parent,
                    lower_bits,
                    upper_bits,
                    &domains,
                    &mut proposals,
                    best,
                    budget,
                )?;
            }
            for arc in reverse_graph.get(source).into_iter().flatten() {
                budget.work(Phase::ProofSearch, 1)?;
                let Some((lower_bits, upper_bits)) = backward_domain(parent, arc.ratio, budget)?
                else {
                    // See the forward empty-image note above. Most
                    // importantly, this path never substitutes `n / r`, so an
                    // underflow plateau mapping positive denominators to zero
                    // remains represented whenever it exists.
                    continue;
                };
                visit_domain_step(
                    fixed_edge,
                    fixed.id,
                    arc.denominator,
                    arc.constraint_id,
                    parent,
                    lower_bits,
                    upper_bits,
                    &domains,
                    &mut proposals,
                    best,
                    budget,
                )?;
            }
        }
        budget.release(frontier.len());
        let mut next_frontier = Vec::new();
        next_frontier
            .try_reserve_exact(proposals.len())
            .map_err(|_| UnknownReason::StorageLimitExceeded)?;
        next_frontier.extend(proposals.keys().copied());
        domains.extend(proposals);
        frontier = next_frontier;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn visit_domain_step(
    fixed_edge: CanonicalId,
    fixed_id: ConstraintId,
    target: CanonicalId,
    closing_id: ConstraintId,
    parent: &ForcedDomain,
    lower_bits: u64,
    upper_bits: u64,
    domains: &BTreeMap<CanonicalId, ForcedDomain>,
    proposals: &mut BTreeMap<CanonicalId, ForcedDomain>,
    best: &mut Option<Candidate>,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<(), UnknownReason> {
    debug_assert!(lower_bits <= upper_bits);
    if let Some(existing) = domains.get(&target) {
        if domains_are_disjoint(existing, lower_bits, upper_bits) {
            consider_candidate(
                best,
                fixed_edge,
                fixed_id,
                &existing.ratio_ids,
                &parent.ratio_ids,
                closing_id,
                budget,
            )?;
        }
        return Ok(());
    }
    if parent.ratio_ids.len() >= MAX_FORCED_PATH_RATIO_IDS_V1 {
        return Ok(());
    }

    if let Some(existing) = proposals.get(&target) {
        if domains_are_disjoint(existing, lower_bits, upper_bits) {
            consider_candidate(
                best,
                fixed_edge,
                fixed_id,
                &existing.ratio_ids,
                &parent.ratio_ids,
                closing_id,
                budget,
            )?;
        }
        let ratio_ids = extended_path(&parent.ratio_ids, closing_id, budget)?;
        let scratch_units = ratio_ids.len();
        let replace = domain_path_precedes(&ratio_ids, lower_bits, upper_bits, existing);
        if replace {
            let old_path_units = existing.ratio_ids.len();
            proposals.insert(
                target,
                ForcedDomain {
                    lower_bits,
                    upper_bits,
                    ratio_ids,
                },
            );
            budget.release(old_path_units);
        } else {
            budget.release(scratch_units);
        }
        return Ok(());
    }

    let ratio_ids = extended_path(&parent.ratio_ids, closing_id, budget)?;
    // The path is already charged; these own the proposal-map key and its
    // eventual frontier key.
    budget.reserve(2)?;
    proposals.insert(
        target,
        ForcedDomain {
            lower_bits,
            upper_bits,
            ratio_ids,
        },
    );
    Ok(())
}

fn domains_are_disjoint(existing: &ForcedDomain, lower_bits: u64, upper_bits: u64) -> bool {
    existing.upper_bits < lower_bits || upper_bits < existing.lower_bits
}

fn domain_path_precedes(
    ratio_ids: &[ConstraintId],
    lower_bits: u64,
    upper_bits: u64,
    existing: &ForcedDomain,
) -> bool {
    ratio_ids.len() < existing.ratio_ids.len()
        || (ratio_ids.len() == existing.ratio_ids.len()
            && canonical_id_slice_cmp(ratio_ids, &existing.ratio_ids)
                .then_with(|| lower_bits.cmp(&existing.lower_bits))
                .then_with(|| upper_bits.cmp(&existing.upper_bits))
                .is_lt())
}

fn forward_domain(
    parent: &ForcedDomain,
    ratio: f64,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<Option<(u64, u64)>, UnknownReason> {
    let lower = scaled_denominator_at_bits(ratio, parent.lower_bits, budget)?;
    let upper = scaled_denominator_at_bits(ratio, parent.upper_bits, budget)?;
    if lower.is_nan() || upper.is_nan() || !lower.is_finite() {
        return Ok(None);
    }
    let upper_bits = if upper.is_finite() {
        upper.to_bits()
    } else {
        // The exact image ends in +infinity. Keeping every finite value in the
        // hull is conservative; the backward search will remove values whose
        // products cannot reach a finite numerator interval.
        MAX_NONNEGATIVE_FINITE_BITS_V1
    };
    let lower_bits = lower.to_bits();
    Ok((lower_bits <= upper_bits).then_some((lower_bits, upper_bits)))
}

fn backward_domain(
    parent: &ForcedDomain,
    ratio: f64,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<Option<(u64, u64)>, UnknownReason> {
    let Some(lower_bits) =
        first_denominator_with_product_at_least(ratio, parent.lower_bits, budget)?
    else {
        return Ok(None);
    };
    let Some(upper_bits) = last_denominator_with_product_at_most(ratio, parent.upper_bits, budget)?
    else {
        return Ok(None);
    };
    Ok((lower_bits <= upper_bits).then_some((lower_bits, upper_bits)))
}

fn first_denominator_with_product_at_least(
    ratio: f64,
    target_bits: u64,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<Option<u64>, UnknownReason> {
    let target = f64::from_bits(target_bits);
    let maximum = scaled_denominator_at_bits(ratio, MAX_NONNEGATIVE_FINITE_BITS_V1, budget)?;
    if maximum.is_nan() || maximum < target {
        return Ok(None);
    }
    let mut lower = 0_u64;
    let mut upper = MAX_NONNEGATIVE_FINITE_BITS_V1;
    while lower < upper {
        let middle = lower + ((upper - lower) >> 1);
        let product = scaled_denominator_at_bits(ratio, middle, budget)?;
        if product.is_nan() {
            return Ok(None);
        }
        if product >= target {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    Ok(Some(lower))
}

fn last_denominator_with_product_at_most(
    ratio: f64,
    target_bits: u64,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<Option<u64>, UnknownReason> {
    let target = f64::from_bits(target_bits);
    let minimum = scaled_denominator_at_bits(ratio, 0, budget)?;
    if minimum.is_nan() || minimum > target {
        return Ok(None);
    }
    let mut lower = 0_u64;
    let mut upper = MAX_NONNEGATIVE_FINITE_BITS_V1;
    while lower < upper {
        let middle = lower + ((upper - lower) >> 1) + 1;
        let product = scaled_denominator_at_bits(ratio, middle, budget)?;
        if product.is_nan() {
            return Ok(None);
        }
        if product <= target {
            lower = middle;
        } else {
            upper = middle - 1;
        }
    }
    Ok(Some(lower))
}

fn scaled_denominator_at_bits(
    ratio: f64,
    denominator_bits: u64,
    budget: &mut Budget<'_, impl Observer>,
) -> Result<f64, UnknownReason> {
    budget.work(Phase::ProofSearch, 1)?;
    Ok(length_ratio_scaled_denominator_binary64_v1(
        ratio,
        f64::from_bits(denominator_bits),
    ))
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

#[cfg(test)]
mod inverse_domain_tests {
    use super::super::bounded_zero_closure::NoopObserver;
    use super::*;

    fn inverse(ratio: f64, lower: f64, upper: f64) -> Option<(u64, u64)> {
        let mut observer = NoopObserver;
        let mut budget = Budget {
            observer: &mut observer,
            limits: Limits::default(),
            completed_work: 0,
            next_checkpoint: OBSERVER_WORK_INTERVAL_V1,
            storage_units: 0,
            peak_storage_units: 0,
        };
        backward_domain(
            &ForcedDomain {
                lower_bits: lower.to_bits(),
                upper_bits: upper.to_bits(),
                ratio_ids: Vec::new(),
            },
            ratio,
            &mut budget,
        )
        .expect("default inverse-domain work must fit")
    }

    #[test]
    fn inverse_search_preserves_underflow_zero_plateau_and_positive_zero() {
        let (lower, upper) = inverse(0.5, 0.0, 0.0).expect("zero has a multiplication preimage");
        assert_eq!(lower, 0.0_f64.to_bits());
        assert!(!f64::from_bits(lower).is_sign_negative());
        assert!(
            (-0.0_f64).to_bits() > MAX_NONNEGATIVE_FINITE_BITS_V1,
            "the ordered length domain must exclude negative zero"
        );
        assert!(
            upper >= f64::from_bits(1).to_bits(),
            "the minimum positive denominator must survive 0.5 * min -> +0"
        );
        assert_eq!(0.5 * f64::from_bits(1), 0.0);
    }

    #[test]
    fn inverse_search_contains_every_adjacent_rounding_alias() {
        let first = 1.0000000000000002e300_f64;
        let second = first.next_up();
        let target = 1.5 * first;
        assert_eq!(target, 1.5 * second, "fixture must exercise a real alias");
        let (lower, upper) =
            inverse(1.5, target, target).expect("the rounded target has a preimage");
        assert!(lower <= first.to_bits() && first.to_bits() <= upper);
        assert!(lower <= second.to_bits() && second.to_bits() <= upper);
    }

    #[test]
    fn inverse_search_handles_maximum_finite_and_overflow_without_division() {
        let (lower, upper) =
            inverse(f64::MAX, f64::MAX, f64::MAX).expect("one maps exactly to MAX");
        assert!(lower <= 1.0_f64.to_bits() && 1.0_f64.to_bits() <= upper);
        assert_eq!(f64::MAX * 1.0, f64::MAX);
        assert!((f64::MAX * 1.0_f64.next_up()).is_infinite());

        assert!(
            inverse(f64::from_bits(1), f64::MAX, f64::MAX).is_none(),
            "an unreachable finite target must have an empty inverse domain"
        );
    }

    #[test]
    fn inverse_interval_contains_every_representative_brute_force_preimage() {
        let minimum_normal_bits = f64::MIN_POSITIVE.to_bits();
        let denominator_bits = [
            0,
            1,
            2,
            3,
            minimum_normal_bits - 1,
            minimum_normal_bits,
            1.0_f64.next_down().to_bits(),
            1.0_f64.to_bits(),
            1.0_f64.next_up().to_bits(),
            (f64::MAX / 2.0).to_bits(),
            f64::MAX.next_down().to_bits(),
            f64::MAX.to_bits(),
        ];
        let targets = [
            0.0,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            1.0,
            1.5,
            f64::MAX / 2.0,
            f64::MAX,
        ];
        for ratio in [f64::from_bits(1), 0.5, 1.0, 1.5, f64::MAX] {
            for &lower in &targets {
                for &upper in targets.iter().filter(|upper| **upper >= lower) {
                    let inverse = inverse(ratio, lower, upper);
                    for bits in denominator_bits {
                        let product = ratio * f64::from_bits(bits);
                        if product.is_finite() && lower <= product && product <= upper {
                            let (inverse_lower, inverse_upper) = inverse.unwrap_or_else(|| {
                                panic!(
                                    "actual preimage missing: ratio={ratio:?}, \
                                     target=[{lower:?}, {upper:?}], denominator={:?}",
                                    f64::from_bits(bits)
                                )
                            });
                            assert!(
                                inverse_lower <= bits && bits <= inverse_upper,
                                "actual preimage escaped interval: ratio={ratio:?}, \
                                 target=[{lower:?}, {upper:?}], denominator={:?}, \
                                 inverse=[{:?}, {:?}]",
                                f64::from_bits(bits),
                                f64::from_bits(inverse_lower),
                                f64::from_bits(inverse_upper),
                            );
                        }
                    }
                }
            }
        }
    }
}
