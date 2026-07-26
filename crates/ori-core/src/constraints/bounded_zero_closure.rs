use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    CanonicalId, ConstraintId, DirectConstraintConflictKindV1, DirectConstraintConflictV1, EdgeId,
    EdgePairKey, GeometricConstraintKindV1, GeometricConstraintSetV1,
    MAX_DIRECT_CONFLICT_CAUSE_IDS_V1, ScalarAssignment, ScalarGroupSummary,
    ZeroLengthClosureProviderKindV1, canonical_id_slice_cmp, canonicalize_constraint_ids,
    fixed_angle_rejects_zero_cross_binary64_v1,
};

/// Whole-document ceiling for the exact-zero implication theorem.
///
/// This is deliberately independent from the 16-record exhaustive subset
/// oracle. A 17-through-256 record document can have a sound closure proof
/// while still being too large for subset enumeration.
pub(super) const MAX_BOUNDED_ZERO_CLOSURE_CONSTRAINTS_V1: usize = 256;
pub(super) const MAX_BOUNDED_ZERO_CLOSURE_WORK_V1: u64 = 2_000_000;
pub(super) const MAX_BOUNDED_ZERO_CLOSURE_STORAGE_UNITS_V1: usize = 8_192;

const WORK_PER_PATTERN_EDGE_V1: u64 = 4;
const LINEAR_WORK_PER_CONSTRAINT_V1: u64 = 24;
const QUADRATIC_WORK_PER_CONSTRAINT_PAIR_V1: u64 = 24;
const STORAGE_UNITS_PER_CONSTRAINT_V1: usize = 32;
const OBSERVER_WORK_INTERVAL_V1: u64 = 128;

type Graph = BTreeMap<CanonicalId, Vec<(CanonicalId, ConstraintId)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Limits {
    pub max_constraints: usize,
    pub max_work: u64,
    pub max_storage_units: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_constraints: MAX_BOUNDED_ZERO_CLOSURE_CONSTRAINTS_V1,
            max_work: MAX_BOUNDED_ZERO_CLOSURE_WORK_V1,
            max_storage_units: MAX_BOUNDED_ZERO_CLOSURE_STORAGE_UNITS_V1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Start,
    DirectPreflightScan,
    SourcePatternScan,
    GraphBuild,
    ProofSearch,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Checkpoint {
    pub phase: Phase,
    pub completed_work: u64,
    pub reserved_storage_units: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObserverControl {
    Continue,
    Cancelled,
    DeadlineReached,
}

pub(super) trait Observer {
    fn checkpoint(&mut self, checkpoint: Checkpoint) -> ObserverControl;
}

#[derive(Debug, Default)]
pub(super) struct NoopObserver;

impl Observer for NoopObserver {
    fn checkpoint(&mut self, _checkpoint: Checkpoint) -> ObserverControl {
        ObserverControl::Continue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnknownReason {
    ConstraintLimitExceeded,
    WorkLimitExceeded,
    StorageLimitExceeded,
    Cancelled,
    DeadlineReached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Proven(DirectConstraintConflictV1),
    NoProof,
    Unknown {
        reason: UnknownReason,
        completed_work: u64,
        reserved_storage_units: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TerminalKind {
    FixedLength,
    Provider(ZeroLengthClosureProviderKindV1),
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Terminal {
    edge: CanonicalId,
    constraint_id: ConstraintId,
    kind: TerminalKind,
}

impl Terminal {
    fn sort_key(self) -> ([u8; 16], TerminalKind, CanonicalId) {
        (self.constraint_id.canonical_bytes(), self.kind, self.edge)
    }
}

struct Candidate {
    constraint_ids: Vec<ConstraintId>,
    terminal: Terminal,
    forced_zero_edge: CanonicalId,
    horizontal_constraint_count: u16,
    vertical_constraint_count: u16,
    zero_propagation_constraint_count: u16,
}

struct WorkTracker<'a, O> {
    observer: &'a mut O,
    completed_work: u64,
    next_checkpoint: u64,
    reserved_storage_units: usize,
    admitted_work: u64,
}

impl<O: Observer> WorkTracker<'_, O> {
    fn checkpoint(&mut self, phase: Phase) -> Result<(), UnknownReason> {
        let control = self.observer.checkpoint(Checkpoint {
            phase,
            completed_work: self.completed_work,
            reserved_storage_units: self.reserved_storage_units,
        });
        match control {
            ObserverControl::Continue => Ok(()),
            ObserverControl::Cancelled => Err(UnknownReason::Cancelled),
            ObserverControl::DeadlineReached => Err(UnknownReason::DeadlineReached),
        }
    }

    fn consume(&mut self, phase: Phase, units: u64) -> Result<(), UnknownReason> {
        self.completed_work = self
            .completed_work
            .checked_add(units)
            .ok_or(UnknownReason::WorkLimitExceeded)?;
        if self.completed_work > self.admitted_work {
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
}

/// Proves an exact-zero contradiction over a bounded five-kind implication
/// graph plus binary64-proven non-degeneracy terminal providers.
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
///   smallest subnormal;
/// * `PointOnLine` and `MirrorSymmetry` reject a collapsed line/axis through
///   `unit_vector`; `AngleBisector` rejects collapse in any of its three edge
///   roles; `Parallel` rejects collapse in either normalized edge role;
/// * `FixedAngle` is a terminal only when the exact production operation
///   rejects both collapsed `atan2` outcomes, zero and pi. This includes
///   signed-zero and degree-to-radian underflow behavior.
///
/// No converse ratio implication is used: a small positive ratio can underflow
/// a non-zero subnormal denominator to zero. Parallel constraints are terminals
/// only, never propagation edges: overflow in the normalizing denominator can
/// make a non-parallel finite cross product produce an exact zero residual.
/// Current coordinates, solver tolerance, and numerical-solver failure are
/// never premises. The theorem concerns exact zero of the production binary64
/// residuals.
#[allow(clippy::too_many_arguments)]
pub(super) fn conflict_with_limits_and_observer(
    set: &GeometricConstraintSetV1<'_>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    horizontal: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    vertical: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    equal_lengths: &BTreeMap<EdgePairKey, Vec<ConstraintId>>,
    ratios: &BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
    limits: Limits,
    observer: &mut impl Observer,
) -> Outcome {
    let constraint_count = set.constraints.len();
    if constraint_count > limits.max_constraints {
        return unknown(UnknownReason::ConstraintLimitExceeded, 0, 0);
    }
    if horizontal.is_empty() || vertical.is_empty() {
        return Outcome::NoProof;
    }

    let Some(reserved_storage_units) =
        constraint_count.checked_mul(STORAGE_UNITS_PER_CONSTRAINT_V1)
    else {
        return unknown(UnknownReason::StorageLimitExceeded, 0, 0);
    };
    if reserved_storage_units > limits.max_storage_units {
        return unknown(
            UnknownReason::StorageLimitExceeded,
            0,
            reserved_storage_units,
        );
    }
    let Some(admitted_work) = required_work(set.source_pattern.edges.len(), constraint_count)
    else {
        return unknown(UnknownReason::WorkLimitExceeded, 0, reserved_storage_units);
    };
    if admitted_work > limits.max_work {
        return unknown(UnknownReason::WorkLimitExceeded, 0, reserved_storage_units);
    }

    let mut work = WorkTracker {
        observer,
        completed_work: 0,
        next_checkpoint: OBSERVER_WORK_INTERVAL_V1,
        reserved_storage_units,
        admitted_work,
    };
    if let Err(reason) = work.checkpoint(Phase::Start) {
        return unknown(reason, work.completed_work, reserved_storage_units);
    }

    let mut terminals = BTreeMap::<CanonicalId, Vec<Terminal>>::new();
    for (edge, summary) in fixed_lengths {
        let Some(fixed) = summary.consistent_assignment() else {
            // Inconsistent groups are handled by an earlier direct theorem.
            continue;
        };
        add_terminal(
            &mut terminals,
            Terminal {
                edge: *edge,
                constraint_id: fixed.id,
                kind: TerminalKind::FixedLength,
            },
        );
    }
    for record in &set.constraints {
        if let Err(reason) = work.consume(Phase::GraphBuild, 1) {
            return unknown(reason, work.completed_work, reserved_storage_units);
        }
        match record.constraint {
            GeometricConstraintKindV1::PointOnLine { line_edge, .. } => add_provider_terminal(
                &mut terminals,
                line_edge,
                record.id,
                ZeroLengthClosureProviderKindV1::PointOnLine,
            ),
            GeometricConstraintKindV1::MirrorSymmetry { axis_edge, .. } => add_provider_terminal(
                &mut terminals,
                axis_edge,
                record.id,
                ZeroLengthClosureProviderKindV1::MirrorSymmetryAxis,
            ),
            GeometricConstraintKindV1::AngleBisector {
                first_edge,
                second_edge,
                bisector_edge,
                ..
            } => {
                for edge in [first_edge, second_edge, bisector_edge] {
                    add_provider_terminal(
                        &mut terminals,
                        edge,
                        record.id,
                        ZeroLengthClosureProviderKindV1::AngleBisector,
                    );
                }
            }
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => {
                for edge in [first_edge, second_edge] {
                    add_provider_terminal(
                        &mut terminals,
                        edge,
                        record.id,
                        ZeroLengthClosureProviderKindV1::Parallel,
                    );
                }
            }
            GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                angle_degrees,
                ..
            } if fixed_angle_rejects_zero_cross_binary64_v1(angle_degrees) => {
                for edge in [first_edge, second_edge] {
                    add_provider_terminal(
                        &mut terminals,
                        edge,
                        record.id,
                        ZeroLengthClosureProviderKindV1::FixedAngle,
                    );
                }
            }
            _ => {}
        }
    }
    if terminals.is_empty() {
        return Outcome::NoProof;
    }
    for items in terminals.values_mut() {
        items.sort_unstable_by_key(|terminal| terminal.sort_key());
        items.dedup_by_key(|terminal| terminal.sort_key());
    }

    let mut endpoints = BTreeMap::new();
    for edge in &set.source_pattern.edges {
        if let Err(reason) = work.consume(Phase::SourcePatternScan, 1) {
            return unknown(reason, work.completed_work, reserved_storage_units);
        }
        let key = edge.id.canonical_bytes();
        if edge_ids.contains_key(&key) {
            endpoints.insert(
                key,
                (edge.start.canonical_bytes(), edge.end.canonical_bytes()),
            );
        }
    }
    if endpoints.len() != edge_ids.len() {
        // Preparation validates references. Keep a defensive fail-closed path
        // if that invariant is refactored.
        return unknown(
            UnknownReason::WorkLimitExceeded,
            work.completed_work,
            reserved_storage_units,
        );
    }

    let horizontal_graph = match orientation_equality_graph(horizontal, &endpoints, &mut work) {
        Ok(graph) => graph,
        Err(reason) => return unknown(reason, work.completed_work, reserved_storage_units),
    };
    let vertical_graph = match orientation_equality_graph(vertical, &endpoints, &mut work) {
        Ok(graph) => graph,
        Err(reason) => return unknown(reason, work.completed_work, reserved_storage_units),
    };
    let mut propagation_graph = Graph::new();
    for (pair, ids) in equal_lengths {
        let Some(id) = ids.iter().min_by_key(|id| id.canonical_bytes()).copied() else {
            return unknown(
                UnknownReason::WorkLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        };
        if let Err(reason) = add_arc(
            &mut propagation_graph,
            pair.first,
            pair.second,
            id,
            &mut work,
        )
        .and_then(|()| {
            add_arc(
                &mut propagation_graph,
                pair.second,
                pair.first,
                id,
                &mut work,
            )
        }) {
            return unknown(reason, work.completed_work, reserved_storage_units);
        }
    }
    for ((numerator, denominator), assignments) in ratios {
        let Some(id) = assignments
            .iter()
            .min_by_key(|assignment| assignment.id.canonical_bytes())
            .map(|assignment| assignment.id)
        else {
            return unknown(
                UnknownReason::WorkLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        };
        // Only this implication direction is universally sound in binary64.
        if let Err(reason) = add_arc(
            &mut propagation_graph,
            *denominator,
            *numerator,
            id,
            &mut work,
        ) {
            return unknown(reason, work.completed_work, reserved_storage_units);
        }
    }
    if let Err(reason) = canonicalize_graph(&mut propagation_graph, &mut work) {
        return unknown(reason, work.completed_work, reserved_storage_units);
    }

    let mut best: Option<Candidate> = None;
    for forced_zero_edge in edge_ids.keys().copied() {
        if let Err(reason) = work.checkpoint(Phase::ProofSearch) {
            return unknown(reason, work.completed_work, reserved_storage_units);
        }
        if let Err(reason) = work.consume(Phase::ProofSearch, 1) {
            return unknown(reason, work.completed_work, reserved_storage_units);
        }
        let (start, end) = endpoints[&forced_zero_edge];
        let horizontal_path = match canonical_path(start, end, &horizontal_graph, &mut work) {
            Ok(Some(path)) => path,
            Ok(None) => continue,
            Err(reason) => return unknown(reason, work.completed_work, reserved_storage_units),
        };
        let vertical_path = match canonical_path(start, end, &vertical_graph, &mut work) {
            Ok(Some(path)) => path,
            Ok(None) => continue,
            Err(reason) => return unknown(reason, work.completed_work, reserved_storage_units),
        };
        // A validated edge has distinct endpoint IDs, so both proof paths
        // contain at least one orientation constraint.
        if horizontal_path.is_empty() || vertical_path.is_empty() {
            return unknown(
                UnknownReason::WorkLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        }

        let (propagation_path, terminal) = match canonical_path_to_terminal(
            forced_zero_edge,
            &propagation_graph,
            &terminals,
            &mut work,
        ) {
            Ok(Some(found)) => found,
            Ok(None) => continue,
            Err(reason) => return unknown(reason, work.completed_work, reserved_storage_units),
        };
        let expected_length = horizontal_path
            .len()
            .checked_add(vertical_path.len())
            .and_then(|count| count.checked_add(propagation_path.len()))
            .and_then(|count| count.checked_add(1));
        let Some(expected_length) = expected_length else {
            return unknown(
                UnknownReason::StorageLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        };
        let mut constraint_ids = Vec::new();
        if constraint_ids.try_reserve_exact(expected_length).is_err() {
            return unknown(
                UnknownReason::StorageLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        }
        constraint_ids.extend(horizontal_path.iter().copied());
        constraint_ids.extend(vertical_path.iter().copied());
        constraint_ids.extend(propagation_path.iter().copied());
        constraint_ids.push(terminal.constraint_id);
        canonicalize_constraint_ids(&mut constraint_ids);
        if constraint_ids.len() != expected_length
            || constraint_ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1
        {
            return unknown(
                UnknownReason::StorageLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        }

        let Ok(horizontal_constraint_count) = u16::try_from(horizontal_path.len()) else {
            return unknown(
                UnknownReason::StorageLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        };
        let Ok(vertical_constraint_count) = u16::try_from(vertical_path.len()) else {
            return unknown(
                UnknownReason::StorageLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        };
        let Ok(zero_propagation_constraint_count) = u16::try_from(propagation_path.len()) else {
            return unknown(
                UnknownReason::StorageLimitExceeded,
                work.completed_work,
                reserved_storage_units,
            );
        };
        let candidate = Candidate {
            constraint_ids,
            terminal,
            forced_zero_edge,
            horizontal_constraint_count,
            vertical_constraint_count,
            zero_propagation_constraint_count,
        };
        if best
            .as_ref()
            .is_none_or(|current| candidate_precedes(&candidate, current))
        {
            best = Some(candidate);
        }
    }

    if let Err(reason) = work.checkpoint(Phase::Complete) {
        return unknown(reason, work.completed_work, reserved_storage_units);
    }
    let Some(best) = best else {
        return Outcome::NoProof;
    };
    let conflict = match best.terminal.kind {
        TerminalKind::FixedLength => {
            DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
                fixed_edge: edge_ids[&best.terminal.edge],
                forced_zero_edge: edge_ids[&best.forced_zero_edge],
                horizontal_constraint_count: best.horizontal_constraint_count,
                vertical_constraint_count: best.vertical_constraint_count,
                zero_propagation_constraint_count: best.zero_propagation_constraint_count,
            }
        }
        TerminalKind::Provider(provider_kind) => {
            DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider {
                provider_kind,
                provider_edge: edge_ids[&best.terminal.edge],
                forced_zero_edge: edge_ids[&best.forced_zero_edge],
                horizontal_constraint_count: best.horizontal_constraint_count,
                vertical_constraint_count: best.vertical_constraint_count,
                zero_propagation_constraint_count: best.zero_propagation_constraint_count,
            }
        }
    };
    Outcome::Proven(DirectConstraintConflictV1 {
        conflict,
        constraint_ids: best.constraint_ids,
    })
}

pub(super) fn required_work(pattern_edge_count: usize, constraint_count: usize) -> Option<u64> {
    let edges = u64::try_from(pattern_edge_count).ok()?;
    let constraints = u64::try_from(constraint_count).ok()?;
    edges
        .checked_mul(WORK_PER_PATTERN_EDGE_V1)?
        .checked_add(constraints.checked_mul(LINEAR_WORK_PER_CONSTRAINT_V1)?)?
        .checked_add(
            constraints
                .checked_mul(constraints)?
                .checked_mul(QUADRATIC_WORK_PER_CONSTRAINT_PAIR_V1)?,
        )
}

fn unknown(reason: UnknownReason, completed_work: u64, reserved_storage_units: usize) -> Outcome {
    Outcome::Unknown {
        reason,
        completed_work,
        reserved_storage_units,
    }
}

fn add_terminal(terminals: &mut BTreeMap<CanonicalId, Vec<Terminal>>, terminal: Terminal) {
    terminals.entry(terminal.edge).or_default().push(terminal);
}

fn add_provider_terminal(
    terminals: &mut BTreeMap<CanonicalId, Vec<Terminal>>,
    edge: EdgeId,
    constraint_id: ConstraintId,
    provider_kind: ZeroLengthClosureProviderKindV1,
) {
    add_terminal(
        terminals,
        Terminal {
            edge: edge.canonical_bytes(),
            constraint_id,
            kind: TerminalKind::Provider(provider_kind),
        },
    );
}

fn orientation_equality_graph<O: Observer>(
    constraints: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    endpoints: &BTreeMap<CanonicalId, (CanonicalId, CanonicalId)>,
    work: &mut WorkTracker<'_, O>,
) -> Result<Graph, UnknownReason> {
    let mut graph = Graph::new();
    for (edge, ids) in constraints {
        let (start, end) = endpoints
            .get(edge)
            .copied()
            .ok_or(UnknownReason::WorkLimitExceeded)?;
        let id = ids
            .iter()
            .min_by_key(|id| id.canonical_bytes())
            .copied()
            .ok_or(UnknownReason::WorkLimitExceeded)?;
        add_arc(&mut graph, start, end, id, work)?;
        add_arc(&mut graph, end, start, id, work)?;
    }
    canonicalize_graph(&mut graph, work)?;
    Ok(graph)
}

fn add_arc<O: Observer>(
    graph: &mut Graph,
    start: CanonicalId,
    end: CanonicalId,
    id: ConstraintId,
    work: &mut WorkTracker<'_, O>,
) -> Result<(), UnknownReason> {
    work.consume(Phase::GraphBuild, 1)?;
    graph.entry(start).or_default().push((end, id));
    Ok(())
}

fn canonicalize_graph<O: Observer>(
    graph: &mut Graph,
    work: &mut WorkTracker<'_, O>,
) -> Result<(), UnknownReason> {
    for arcs in graph.values_mut() {
        work.consume(
            Phase::GraphBuild,
            u64::try_from(arcs.len()).map_err(|_| UnknownReason::WorkLimitExceeded)?,
        )?;
        arcs.sort_unstable_by_key(|(neighbor, id)| (id.canonical_bytes(), *neighbor));
        arcs.dedup();
    }
    Ok(())
}

fn canonical_path<O: Observer>(
    start: CanonicalId,
    target: CanonicalId,
    graph: &Graph,
    work: &mut WorkTracker<'_, O>,
) -> Result<Option<Vec<ConstraintId>>, UnknownReason> {
    if start == target {
        return Ok(Some(Vec::new()));
    }
    let mut parents = BTreeMap::new();
    let mut visited = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        work.consume(Phase::ProofSearch, 1)?;
        for (neighbor, id) in graph.get(&node).into_iter().flatten() {
            work.consume(Phase::ProofSearch, 1)?;
            if !visited.insert(*neighbor) {
                continue;
            }
            parents.insert(*neighbor, (node, *id));
            if *neighbor == target {
                return reconstruct_path(start, target, &parents, work).map(Some);
            }
            queue.push_back(*neighbor);
        }
    }
    Ok(None)
}

fn canonical_path_to_terminal<O: Observer>(
    start: CanonicalId,
    graph: &Graph,
    terminals: &BTreeMap<CanonicalId, Vec<Terminal>>,
    work: &mut WorkTracker<'_, O>,
) -> Result<Option<(Vec<ConstraintId>, Terminal)>, UnknownReason> {
    if let Some(terminal) = terminals
        .get(&start)
        .and_then(|items| items.first())
        .copied()
    {
        return Ok(Some((Vec::new(), terminal)));
    }
    let mut parents = BTreeMap::new();
    let mut visited = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        work.consume(Phase::ProofSearch, 1)?;
        for (neighbor, id) in graph.get(&node).into_iter().flatten() {
            work.consume(Phase::ProofSearch, 1)?;
            if !visited.insert(*neighbor) {
                continue;
            }
            parents.insert(*neighbor, (node, *id));
            if let Some(terminal) = terminals
                .get(neighbor)
                .and_then(|items| items.first())
                .copied()
            {
                let path = reconstruct_path(start, *neighbor, &parents, work)?;
                return Ok(Some((path, terminal)));
            }
            queue.push_back(*neighbor);
        }
    }
    Ok(None)
}

fn reconstruct_path<O: Observer>(
    start: CanonicalId,
    target: CanonicalId,
    parents: &BTreeMap<CanonicalId, (CanonicalId, ConstraintId)>,
    work: &mut WorkTracker<'_, O>,
) -> Result<Vec<ConstraintId>, UnknownReason> {
    work.checkpoint(Phase::ProofSearch)?;
    let mut path = Vec::new();
    path.try_reserve_exact(parents.len())
        .map_err(|_| UnknownReason::StorageLimitExceeded)?;
    let mut cursor = target;
    while cursor != start {
        work.consume(Phase::ProofSearch, 1)?;
        let (parent, constraint) = parents
            .get(&cursor)
            .copied()
            .ok_or(UnknownReason::WorkLimitExceeded)?;
        path.push(constraint);
        cursor = parent;
    }
    path.reverse();
    Ok(path)
}

fn candidate_precedes(candidate: &Candidate, current: &Candidate) -> bool {
    candidate.constraint_ids.len() < current.constraint_ids.len()
        || (candidate.constraint_ids.len() == current.constraint_ids.len()
            && (canonical_id_slice_cmp(&candidate.constraint_ids, &current.constraint_ids).is_lt()
                || (candidate.constraint_ids == current.constraint_ids
                    && (
                        candidate.terminal.sort_key(),
                        candidate.forced_zero_edge,
                        candidate.horizontal_constraint_count,
                        candidate.vertical_constraint_count,
                        candidate.zero_propagation_constraint_count,
                    ) < (
                        current.terminal.sort_key(),
                        current.forced_zero_edge,
                        current.horizontal_constraint_count,
                        current.vertical_constraint_count,
                        current.zero_propagation_constraint_count,
                    ))))
}
