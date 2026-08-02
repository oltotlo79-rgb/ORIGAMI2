use std::ops::Range;

use crate::{FacewiseConstraintKind, OverlapCellKey};

mod compact_completion;
mod transitivity;

use compact_completion::{
    CompactCompletionResult, CompactPairIncidence, CompactReachabilityWordChange,
    CompactSearchFrame, compact_transitive_closure_shape, try_compact_completion,
};

pub(crate) use transitivity::{
    TRANSITIVITY_ALLOWED_ROWS, TransitivityConstraintFamily, TransitivityConstraints, choose_three,
    choose_two,
};
use transitivity::{TransitivityConstraint, TransitivityConstraintIter};

const DOMAIN_FALSE: u8 = 0b01;
const DOMAIN_TRUE: u8 = 0b10;
const DOMAIN_BOTH: u8 = DOMAIN_FALSE | DOMAIN_TRUE;
const CONTROL_BATCH_RECORDS: usize = 1_024;
// Below this boundary the established explicit DFS remains cheap enough and
// retains its historical small-fixture search-node trace. Above it, repeatedly
// scanning every logical triple for each branch is no longer a boundedly useful
// implementation of the same total-order problem.
const COMPACT_COMPLETION_LOGICAL_THRESHOLD: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TupleConstraint {
    pub kind: FacewiseConstraintKind,
    pub variables: Vec<usize>,
    pub allowed_rows: Vec<u8>,
    pub faces: Vec<usize>,
    pub supporting_cell: Option<OverlapCellKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintSet {
    explicit: Vec<TupleConstraint>,
    transitivity: TransitivityConstraints,
    transitivity_insertion: usize,
    logical_len: usize,
    compact_explicit_len: usize,
    compact_explicit_incidence_len: usize,
}

impl ConstraintSet {
    pub(crate) fn new(
        explicit: Vec<TupleConstraint>,
        transitivity: TransitivityConstraints,
        transitivity_insertion: usize,
    ) -> Option<Self> {
        if transitivity_insertion > explicit.len() {
            return None;
        }
        let logical_len = explicit.len().checked_add(transitivity.len())?;
        let (compact_explicit_len, compact_explicit_incidence_len) =
            compact_explicit_shape(&explicit)?;
        Some(Self {
            explicit,
            transitivity,
            transitivity_insertion,
            logical_len,
            compact_explicit_len,
            compact_explicit_incidence_len,
        })
    }

    #[cfg(test)]
    fn from_explicit(explicit: Vec<TupleConstraint>) -> Self {
        let logical_len = explicit.len();
        let (compact_explicit_len, compact_explicit_incidence_len) =
            compact_explicit_shape(&explicit).expect("the test constraint shape fits usize");
        Self {
            explicit,
            transitivity: TransitivityConstraints::try_new(Vec::new(), 0)
                .expect("the empty compact constraint set is valid"),
            transitivity_insertion: logical_len,
            logical_len,
            compact_explicit_len,
            compact_explicit_incidence_len,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.logical_len
    }

    pub(crate) fn iterator_working_memory_upper_bound(&self) -> Option<usize> {
        self.transitivity.iterator_working_memory_upper_bound()
    }

    pub(crate) fn iterator_initialization_records(&self) -> usize {
        self.transitivity.family_count()
    }

    fn compact_completion_working_memory_upper_bound(
        &self,
        variable_count: usize,
    ) -> Option<usize> {
        let maximum_ply = self.transitivity.maximum_ply();
        let order_scratch = maximum_ply
            .checked_mul(std::mem::size_of::<usize>())?
            .checked_add(maximum_ply.checked_mul(std::mem::size_of::<usize>())?)?
            .checked_add(maximum_ply.checked_mul(std::mem::size_of::<u8>())?)?;
        let (closure_incidences, closure_words, closure_trail_records) =
            compact_transitive_closure_shape(self)?;
        let closure_scratch = if self.transitivity.family_count() == 0 {
            0
        } else {
            variable_count
                .checked_mul(std::mem::size_of::<usize>())?
                .checked_add(
                    self.transitivity
                        .family_count()
                        .checked_add(1)?
                        .checked_mul(std::mem::size_of::<usize>())?,
                )?
                .checked_add(
                    variable_count
                        .checked_add(1)?
                        .checked_mul(std::mem::size_of::<usize>())?,
                )?
                .checked_add(
                    closure_incidences.checked_mul(std::mem::size_of::<CompactPairIncidence>())?,
                )?
                .checked_add(closure_words.checked_mul(std::mem::size_of::<usize>())?)?
                .checked_add(variable_count.checked_mul(std::mem::size_of::<usize>())?)?
                .checked_add(
                    closure_trail_records
                        .checked_mul(std::mem::size_of::<CompactReachabilityWordChange>())?,
                )?
        };
        let explicit_scratch = if self.compact_explicit_len == 0 {
            0
        } else {
            variable_count
                .checked_mul(std::mem::size_of::<usize>())?
                .checked_add(
                    variable_count
                        .checked_add(1)?
                        .checked_mul(std::mem::size_of::<usize>())?,
                )?
                .checked_add(
                    self.compact_explicit_incidence_len
                        .checked_mul(std::mem::size_of::<usize>())?,
                )?
                .checked_add(
                    self.compact_explicit_len
                        .checked_mul(std::mem::size_of::<usize>())?,
                )?
                .checked_add(self.explicit.len().checked_mul(std::mem::size_of::<u8>())?)?
        };
        order_scratch
            .checked_add(closure_scratch)?
            .checked_add(explicit_scratch)
    }

    fn uses_compact_completion(&self) -> bool {
        self.transitivity.len() >= COMPACT_COMPLETION_LOGICAL_THRESHOLD
    }

    pub(crate) fn try_iter(&self) -> Result<ConstraintSetIter<'_>, ()> {
        Ok(ConstraintSetIter {
            explicit: &self.explicit,
            explicit_position: 0,
            transitivity_insertion: self.transitivity_insertion,
            transitivity: self.transitivity.try_iter()?,
            phase: ConstraintSetIterPhase::ExplicitPrefix,
        })
    }
}

fn compact_explicit_shape(explicit: &[TupleConstraint]) -> Option<(usize, usize)> {
    explicit.iter().try_fold(
        (0_usize, 0_usize),
        |(constraint_count, incidence_count), constraint| {
            if tuple_constraint_is_tautology(constraint) {
                Some((constraint_count, incidence_count))
            } else {
                Some((
                    constraint_count.checked_add(1)?,
                    incidence_count.checked_add(constraint.variables.len())?,
                ))
            }
        },
    )
}

fn tuple_constraint_is_tautology(constraint: &TupleConstraint) -> bool {
    let Ok(arity) = u32::try_from(constraint.variables.len()) else {
        return false;
    };
    let Some(row_count) = 1_u64.checked_shl(arity) else {
        return false;
    };
    let expected = if row_count == 64 {
        u64::MAX
    } else {
        (1_u64 << row_count) - 1
    };
    constraint
        .allowed_rows
        .iter()
        .filter(|row| u64::from(**row) < row_count)
        .fold(0_u64, |seen, row| seen | (1_u64 << *row))
        == expected
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConstraintView<'a> {
    Explicit(&'a TupleConstraint),
    Transitivity(TransitivityConstraint),
}

impl ConstraintView<'_> {
    pub(crate) fn kind(self) -> FacewiseConstraintKind {
        match self {
            Self::Explicit(constraint) => constraint.kind,
            Self::Transitivity(_) => FacewiseConstraintKind::Transitivity,
        }
    }

    pub(crate) fn variables(&self) -> &[usize] {
        match self {
            Self::Explicit(constraint) => &constraint.variables,
            Self::Transitivity(constraint) => &constraint.variables,
        }
    }

    pub(crate) fn allowed_rows(&self) -> &[u8] {
        match self {
            Self::Explicit(constraint) => &constraint.allowed_rows,
            Self::Transitivity(_) => &TRANSITIVITY_ALLOWED_ROWS,
        }
    }

    pub(crate) fn faces(&self) -> &[usize] {
        match self {
            Self::Explicit(constraint) => &constraint.faces,
            Self::Transitivity(constraint) => &constraint.faces,
        }
    }

    pub(crate) fn supporting_cell(self) -> Option<OverlapCellKey> {
        match self {
            Self::Explicit(constraint) => constraint.supporting_cell,
            Self::Transitivity(constraint) => Some(constraint.supporting_cell),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConstraintConflict {
    pub(crate) logical_index: usize,
    pub(crate) kind: FacewiseConstraintKind,
    faces: [usize; 6],
    face_count: u8,
    pub(crate) supporting_cell: Option<OverlapCellKey>,
}

impl ConstraintConflict {
    fn from_view(logical_index: usize, constraint: ConstraintView<'_>) -> Option<Self> {
        let face_count = u8::try_from(constraint.faces().len()).ok()?;
        if usize::from(face_count) > 6 {
            return None;
        }
        let mut faces = [0_usize; 6];
        faces[..usize::from(face_count)].copy_from_slice(constraint.faces());
        Some(Self {
            logical_index,
            kind: constraint.kind(),
            faces,
            face_count,
            supporting_cell: constraint.supporting_cell(),
        })
    }

    pub(crate) fn faces(&self) -> &[usize] {
        &self.faces[..usize::from(self.face_count)]
    }
}

enum ConstraintSetIterPhase {
    ExplicitPrefix,
    Transitivity,
    ExplicitSuffix,
    Complete,
}

pub(crate) struct ConstraintSetIter<'a> {
    explicit: &'a [TupleConstraint],
    explicit_position: usize,
    transitivity_insertion: usize,
    transitivity: TransitivityConstraintIter<'a>,
    phase: ConstraintSetIterPhase,
}

impl<'a> Iterator for ConstraintSetIter<'a> {
    type Item = ConstraintView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.phase {
                ConstraintSetIterPhase::ExplicitPrefix => {
                    if self.explicit_position < self.transitivity_insertion {
                        let constraint = &self.explicit[self.explicit_position];
                        self.explicit_position += 1;
                        return Some(ConstraintView::Explicit(constraint));
                    }
                    self.phase = ConstraintSetIterPhase::Transitivity;
                }
                ConstraintSetIterPhase::Transitivity => {
                    if let Some(constraint) = self.transitivity.next() {
                        return Some(ConstraintView::Transitivity(constraint));
                    }
                    self.phase = ConstraintSetIterPhase::ExplicitSuffix;
                }
                ConstraintSetIterPhase::ExplicitSuffix => {
                    if self.explicit_position < self.explicit.len() {
                        let constraint = &self.explicit[self.explicit_position];
                        self.explicit_position += 1;
                        return Some(ConstraintView::Explicit(constraint));
                    }
                    self.phase = ConstraintSetIterPhase::Complete;
                }
                ConstraintSetIterPhase::Complete => return None,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstraintSolverEvent {
    PropagationBatch,
    SearchNode,
    VerifyingConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstraintSolverControl {
    Continue,
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConstraintSolverResult {
    Satisfied {
        assignment: Vec<bool>,
        search_nodes: usize,
    },
    Unsatisfied {
        conflict_constraint: Option<ConstraintConflict>,
        search_nodes: usize,
    },
    SearchNodeLimit {
        observed: usize,
    },
    DeadlineReached {
        search_nodes: usize,
    },
    Cancelled,
    WorkingMemoryLimit {
        observed: usize,
    },
    InvalidConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompleteAssignmentVerificationResult {
    Accepts,
    Rejects,
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit { observed: usize },
    InvalidConstraint,
}

/// Returns the conservative logical storage charge for requested solver
/// buffers. Allocator metadata, padding, and implementation-specific excess
/// capacity are deliberately outside this supported 64-bit-target contract.
pub(crate) fn solver_working_memory_upper_bound(variable_count: usize) -> Option<usize> {
    let allocations = [
        // Domains and the final assignment coexist while the assignment is
        // materialized. Vec<bool> is bit-packed today, but accounting one byte
        // per value remains a safe implementation-independent upper bound.
        (variable_count, std::mem::size_of::<u8>()),
        (variable_count, std::mem::size_of::<u8>()),
        // Disjoint-set storage used to derive independent components.
        (variable_count, std::mem::size_of::<usize>()),
        (variable_count, std::mem::size_of::<u8>()),
        (variable_count, std::mem::size_of::<(usize, usize)>()),
        // Component ranges and their single contiguous variable payload.
        (variable_count, std::mem::size_of::<Range<usize>>()),
        (variable_count, std::mem::size_of::<usize>()),
        // The generic and compact witness searches use only one stack at a
        // time; either stack and its rollback trail contain at most one live
        // record per variable on the active path.
        (
            variable_count,
            std::mem::size_of::<SearchFrame>().max(std::mem::size_of::<CompactSearchFrame>()),
        ),
        (variable_count, std::mem::size_of::<(usize, u8)>()),
    ];
    allocations
        .into_iter()
        .try_fold(0_usize, |total, (count, element_size)| {
            total.checked_add(count.checked_mul(element_size)?)
        })
}

pub(crate) fn complete_assignment_verification_working_memory_upper_bound(
    constraints: &ConstraintSet,
) -> Option<usize> {
    let maximum_ply = constraints.transitivity.maximum_ply();
    maximum_ply
        .checked_mul(std::mem::size_of::<usize>())?
        .checked_add(maximum_ply.checked_mul(std::mem::size_of::<u8>())?)
}

pub(crate) fn verify_complete_assignment_with_memory<F>(
    assignment: &[bool],
    constraints: &ConstraintSet,
    max_working_memory_bytes: usize,
    mut control: F,
) -> CompleteAssignmentVerificationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let required = complete_assignment_verification_working_memory_upper_bound(constraints)
        .unwrap_or(usize::MAX);
    match control(ConstraintSolverEvent::VerifyingConstraint, 0) {
        ConstraintSolverControl::Continue => {}
        ConstraintSolverControl::DeadlineReached => {
            return CompleteAssignmentVerificationResult::DeadlineReached;
        }
        ConstraintSolverControl::Cancelled => {
            return CompleteAssignmentVerificationResult::Cancelled;
        }
        ConstraintSolverControl::WorkingMemoryLimit => {
            return CompleteAssignmentVerificationResult::WorkingMemoryLimit { observed: required };
        }
    }
    if required == usize::MAX || required > max_working_memory_bytes {
        return CompleteAssignmentVerificationResult::WorkingMemoryLimit { observed: required };
    }
    let mut above_counts = Vec::<usize>::new();
    let mut seen_counts = Vec::<u8>::new();
    let maximum_ply = constraints.transitivity.maximum_ply();
    if above_counts.try_reserve_exact(maximum_ply).is_err()
        || seen_counts.try_reserve_exact(maximum_ply).is_err()
    {
        return CompleteAssignmentVerificationResult::WorkingMemoryLimit { observed: required };
    }
    let mut pending = 0_usize;
    for constraint in &constraints.explicit {
        let view = ConstraintView::Explicit(constraint);
        if !valid_constraint(view, assignment.len()) {
            return CompleteAssignmentVerificationResult::InvalidConstraint;
        }
        if !constraint_accepts(view, assignment) {
            return CompleteAssignmentVerificationResult::Rejects;
        }
        if let Err(abort) = poll_after_record_batch(&mut control, 0, &mut pending) {
            return complete_assignment_verification_abort(abort, required);
        }
    }
    for family in constraints.transitivity.families() {
        let ply = family.covering_faces.len();
        above_counts.clear();
        above_counts.resize(ply, 0);
        seen_counts.clear();
        seen_counts.resize(ply, 0);
        for first in 0..ply {
            for second in first + 1..ply {
                let Some(variable) = family.pair_variable(first, second) else {
                    return CompleteAssignmentVerificationResult::InvalidConstraint;
                };
                let Some(second_above_first) = assignment.get(variable).copied() else {
                    return CompleteAssignmentVerificationResult::InvalidConstraint;
                };
                let winner = if second_above_first { second } else { first };
                let Some(next) = above_counts[winner].checked_add(1) else {
                    return CompleteAssignmentVerificationResult::InvalidConstraint;
                };
                above_counts[winner] = next;
                if let Err(abort) = poll_after_record_batch(&mut control, 0, &mut pending) {
                    return complete_assignment_verification_abort(abort, required);
                }
            }
        }
        for &count in &above_counts {
            if count >= ply || seen_counts[count] != 0 {
                return CompleteAssignmentVerificationResult::Rejects;
            }
            seen_counts[count] = 1;
        }
    }
    if pending != 0 {
        match control(ConstraintSolverEvent::VerifyingConstraint, 0) {
            ConstraintSolverControl::Continue => {}
            abort => return complete_assignment_verification_abort(abort, required),
        }
    }
    CompleteAssignmentVerificationResult::Accepts
}

const fn complete_assignment_verification_abort(
    abort: ConstraintSolverControl,
    required: usize,
) -> CompleteAssignmentVerificationResult {
    match abort {
        ConstraintSolverControl::Continue => {
            CompleteAssignmentVerificationResult::InvalidConstraint
        }
        ConstraintSolverControl::DeadlineReached => {
            CompleteAssignmentVerificationResult::DeadlineReached
        }
        ConstraintSolverControl::Cancelled => CompleteAssignmentVerificationResult::Cancelled,
        ConstraintSolverControl::WorkingMemoryLimit => {
            CompleteAssignmentVerificationResult::WorkingMemoryLimit { observed: required }
        }
    }
}

#[cfg(test)]
pub(crate) fn solve_constraints<F>(
    variable_count: usize,
    constraints: &[TupleConstraint],
    fixed_assignments: &[Option<bool>],
    max_search_nodes: usize,
    control: F,
) -> ConstraintSolverResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let constraints = ConstraintSet::from_explicit(constraints.to_vec());
    solve_constraints_with_memory(
        variable_count,
        &constraints,
        fixed_assignments,
        max_search_nodes,
        usize::MAX,
        control,
    )
}

pub(crate) fn solve_constraints_with_memory<F>(
    variable_count: usize,
    constraints: &ConstraintSet,
    fixed_assignments: &[Option<bool>],
    max_search_nodes: usize,
    max_working_memory_bytes: usize,
    mut control: F,
) -> ConstraintSolverResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    if fixed_assignments.len() != variable_count {
        return ConstraintSolverResult::InvalidConstraint;
    }
    let required_working_memory = solver_working_memory_upper_bound(variable_count)
        .and_then(|base| base.checked_add(constraints.iterator_working_memory_upper_bound()?))
        .and_then(|base| {
            if constraints.uses_compact_completion() {
                base.checked_add(
                    constraints.compact_completion_working_memory_upper_bound(variable_count)?,
                )
            } else {
                Some(base)
            }
        })
        .unwrap_or(usize::MAX);
    if required_working_memory == usize::MAX || required_working_memory > max_working_memory_bytes {
        if let Some(abort) =
            control_abort_result(&mut control, ConstraintSolverEvent::PropagationBatch, 0)
        {
            return abort;
        }
        return ConstraintSolverResult::WorkingMemoryLimit {
            observed: required_working_memory,
        };
    }
    match validate_constraint_set(constraints, variable_count, &mut control) {
        Ok(true) => {}
        Ok(false) => return ConstraintSolverResult::InvalidConstraint,
        Err(abort) => return solver_abort_result(abort, 0, required_working_memory),
    }
    let mut domains = Vec::new();
    if domains.try_reserve_exact(variable_count).is_err() {
        return ConstraintSolverResult::WorkingMemoryLimit {
            observed: required_working_memory,
        };
    }
    domains.extend(fixed_assignments.iter().map(|assignment| match assignment {
        Some(false) => DOMAIN_FALSE,
        Some(true) => DOMAIN_TRUE,
        None => DOMAIN_BOTH,
    }));
    let mut search_nodes = 0_usize;
    if constraints.uses_compact_completion() {
        match try_compact_completion(&domains, constraints, max_search_nodes, &mut control) {
            CompactCompletionResult::Satisfied {
                candidate,
                search_nodes: compact_search_nodes,
            } => {
                drop(domains);
                let mut assignment = Vec::new();
                if assignment.try_reserve_exact(variable_count).is_err() {
                    return ConstraintSolverResult::WorkingMemoryLimit {
                        observed: required_working_memory,
                    };
                }
                for domain in candidate {
                    assignment.push(match domain {
                        DOMAIN_FALSE => false,
                        DOMAIN_TRUE => true,
                        _ => return ConstraintSolverResult::InvalidConstraint,
                    });
                }
                return ConstraintSolverResult::Satisfied {
                    assignment,
                    search_nodes: compact_search_nodes,
                };
            }
            CompactCompletionResult::Fallback {
                search_nodes: compact_search_nodes,
            } => search_nodes = compact_search_nodes,
            CompactCompletionResult::SearchNodeLimit { observed } => {
                return ConstraintSolverResult::SearchNodeLimit { observed };
            }
            CompactCompletionResult::DeadlineReached {
                search_nodes: compact_search_nodes,
            } => {
                return ConstraintSolverResult::DeadlineReached {
                    search_nodes: compact_search_nodes,
                };
            }
            CompactCompletionResult::Cancelled => return ConstraintSolverResult::Cancelled,
            CompactCompletionResult::WorkingMemoryLimit => {
                return ConstraintSolverResult::WorkingMemoryLimit {
                    observed: required_working_memory,
                };
            }
            CompactCompletionResult::InvalidConstraint => {
                return ConstraintSolverResult::InvalidConstraint;
            }
        }
    }
    match propagate(&mut domains, constraints, &mut control, search_nodes) {
        PropagationResult::Stable => {}
        PropagationResult::Conflict(conflict) => {
            if let Some(abort) = control_abort_result(
                &mut control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            ) {
                return abort;
            }
            return ConstraintSolverResult::Unsatisfied {
                conflict_constraint: Some(conflict),
                search_nodes,
            };
        }
        PropagationResult::DeadlineReached => {
            return ConstraintSolverResult::DeadlineReached { search_nodes };
        }
        PropagationResult::Cancelled => return ConstraintSolverResult::Cancelled,
        PropagationResult::WorkingMemoryLimit => {
            return ConstraintSolverResult::WorkingMemoryLimit {
                observed: required_working_memory,
            };
        }
        PropagationResult::InvalidConstraint => return ConstraintSolverResult::InvalidConstraint,
    }

    let components =
        match variable_components(variable_count, constraints, &mut control, search_nodes) {
            Ok(components) => components,
            Err(ConstraintSolverControl::DeadlineReached) => {
                return ConstraintSolverResult::DeadlineReached { search_nodes };
            }
            Err(ConstraintSolverControl::Cancelled) => {
                return ConstraintSolverResult::Cancelled;
            }
            Err(ConstraintSolverControl::Continue) => {
                return ConstraintSolverResult::InvalidConstraint;
            }
            Err(ConstraintSolverControl::WorkingMemoryLimit) => {
                return ConstraintSolverResult::WorkingMemoryLimit {
                    observed: required_working_memory,
                };
            }
        };
    let VariableComponents {
        variables: component_variables,
        ranges: component_ranges,
    } = components;
    let mut components_since_poll = 0_usize;
    for component_range in component_ranges {
        components_since_poll += 1;
        if components_since_poll == CONTROL_BATCH_RECORDS {
            if let Some(abort) = control_abort_result(
                &mut control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            ) {
                return abort;
            }
            components_since_poll = 0;
        }
        match search_component(
            domains,
            &component_variables[component_range],
            constraints,
            max_search_nodes,
            &mut search_nodes,
            &mut control,
        ) {
            SearchResult::Satisfied(next) => domains = next,
            SearchResult::Unsatisfied(conflict_constraint) => {
                if let Some(abort) = control_abort_result(
                    &mut control,
                    ConstraintSolverEvent::SearchNode,
                    search_nodes,
                ) {
                    return abort;
                }
                return ConstraintSolverResult::Unsatisfied {
                    conflict_constraint,
                    search_nodes,
                };
            }
            SearchResult::Limit(observed) => {
                return ConstraintSolverResult::SearchNodeLimit { observed };
            }
            SearchResult::DeadlineReached => {
                return ConstraintSolverResult::DeadlineReached { search_nodes };
            }
            SearchResult::Cancelled => return ConstraintSolverResult::Cancelled,
            SearchResult::WorkingMemoryLimit => {
                return ConstraintSolverResult::WorkingMemoryLimit {
                    observed: required_working_memory,
                };
            }
            SearchResult::InvalidConstraint => return ConstraintSolverResult::InvalidConstraint,
        }
    }

    let mut assignment = Vec::new();
    if assignment.try_reserve_exact(variable_count).is_err() {
        return ConstraintSolverResult::WorkingMemoryLimit {
            observed: required_working_memory,
        };
    }
    for domain in domains {
        assignment.push(match domain {
            DOMAIN_FALSE => false,
            DOMAIN_TRUE => true,
            _ => return ConstraintSolverResult::InvalidConstraint,
        });
    }
    if let Err(abort) = poll_iterator_initialization(constraints, &mut control, search_nodes) {
        return solver_abort_result(abort, search_nodes, required_working_memory);
    }
    let Ok(constraint_iter) = constraints.try_iter() else {
        return ConstraintSolverResult::WorkingMemoryLimit {
            observed: required_working_memory,
        };
    };
    for (index, constraint) in constraint_iter.enumerate() {
        match control(ConstraintSolverEvent::VerifyingConstraint, search_nodes) {
            ConstraintSolverControl::Continue => {}
            ConstraintSolverControl::DeadlineReached => {
                return ConstraintSolverResult::DeadlineReached { search_nodes };
            }
            ConstraintSolverControl::Cancelled => return ConstraintSolverResult::Cancelled,
            ConstraintSolverControl::WorkingMemoryLimit => {
                return ConstraintSolverResult::WorkingMemoryLimit {
                    observed: required_working_memory,
                };
            }
        }
        if !constraint_accepts(constraint, &assignment) {
            let Some(conflict) = ConstraintConflict::from_view(index, constraint) else {
                return ConstraintSolverResult::InvalidConstraint;
            };
            return ConstraintSolverResult::Unsatisfied {
                conflict_constraint: Some(conflict),
                search_nodes,
            };
        }
    }
    if let Some(abort) = control_abort_result(
        &mut control,
        ConstraintSolverEvent::VerifyingConstraint,
        search_nodes,
    ) {
        return abort;
    }
    ConstraintSolverResult::Satisfied {
        assignment,
        search_nodes,
    }
}

fn valid_constraint(constraint: ConstraintView<'_>, variable_count: usize) -> bool {
    let variables = constraint.variables();
    let allowed_rows = constraint.allowed_rows();
    let arity = variables.len();
    if arity > 6
        || constraint.faces().len() > 6
        || allowed_rows.is_empty()
        || variables.iter().any(|variable| *variable >= variable_count)
    {
        return false;
    }
    if variables
        .iter()
        .enumerate()
        .any(|(index, variable)| variables[..index].contains(variable))
    {
        return false;
    }
    let row_limit = 1_u8.checked_shl(arity as u32).unwrap_or(0);
    allowed_rows.iter().all(|row| *row < row_limit)
}

fn validate_constraint_set<F>(
    constraints: &ConstraintSet,
    variable_count: usize,
    control: &mut F,
) -> Result<bool, ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    poll_iterator_initialization(constraints, control, 0)?;
    let mut explicit_since_poll = 0_usize;
    for constraint in &constraints.explicit[..constraints.transitivity_insertion] {
        if !valid_constraint(ConstraintView::Explicit(constraint), variable_count) {
            return Ok(false);
        }
        poll_after_record_batch(control, 0, &mut explicit_since_poll)?;
    }

    let mut recomputed_logical_len = 0_usize;
    let mut compact_since_poll = 0_usize;
    for family in constraints.transitivity.families() {
        let Some(pair_count) = choose_two(family.covering_faces.len()) else {
            return Ok(false);
        };
        if family.covering_faces.len() < 3
            || family.pair_variables.len() != pair_count
            || !family
                .covering_faces
                .windows(2)
                .all(|faces| faces[0] < faces[1])
            || !family
                .pair_variables
                .windows(2)
                .all(|variables| variables[0] < variables[1])
        {
            return Ok(false);
        }
        for variable in &family.pair_variables {
            if *variable >= variable_count {
                return Ok(false);
            }
            poll_after_record_batch(control, 0, &mut compact_since_poll)?;
        }
        let Some(family_len) = family.logical_len() else {
            return Ok(false);
        };
        let Some(next_len) = recomputed_logical_len.checked_add(family_len) else {
            return Ok(false);
        };
        recomputed_logical_len = next_len;
    }
    if recomputed_logical_len != constraints.transitivity.len() {
        return Ok(false);
    }
    poll_logical_records(
        constraints.transitivity.len(),
        ConstraintSolverEvent::PropagationBatch,
        0,
        control,
    )?;

    for constraint in &constraints.explicit[constraints.transitivity_insertion..] {
        if !valid_constraint(ConstraintView::Explicit(constraint), variable_count) {
            return Ok(false);
        }
        poll_after_record_batch(control, 0, &mut explicit_since_poll)?;
    }
    Ok(true)
}

fn poll_logical_records<F>(
    logical_records: usize,
    event: ConstraintSolverEvent,
    search_nodes: usize,
    control: &mut F,
) -> Result<(), ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let batches = logical_records / CONTROL_BATCH_RECORDS;
    for _ in 0..batches {
        poll_control(control, event, search_nodes)?;
    }
    Ok(())
}

enum PropagationResult {
    Stable,
    Conflict(ConstraintConflict),
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit,
    InvalidConstraint,
}

fn propagate<F>(
    domains: &mut [u8],
    constraints: &ConstraintSet,
    control: &mut F,
    search_nodes: usize,
) -> PropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    propagate_internal(domains, constraints, control, search_nodes, None)
}

fn propagate_with_trail<F>(
    domains: &mut [u8],
    constraints: &ConstraintSet,
    control: &mut F,
    search_nodes: usize,
    trail: &mut Vec<(usize, u8)>,
) -> PropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    propagate_internal(domains, constraints, control, search_nodes, Some(trail))
}

fn propagate_internal<F>(
    domains: &mut [u8],
    constraints: &ConstraintSet,
    control: &mut F,
    search_nodes: usize,
    mut trail: Option<&mut Vec<(usize, u8)>>,
) -> PropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    loop {
        if let Err(abort) = poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        ) {
            return propagation_abort(abort);
        }
        let mut changed = false;
        let mut processed_since_poll = 0_usize;
        if let Err(abort) = poll_iterator_initialization(constraints, control, search_nodes) {
            return propagation_abort(abort);
        }
        let Ok(constraint_iter) = constraints.try_iter() else {
            return PropagationResult::WorkingMemoryLimit;
        };
        for (constraint_index, constraint) in constraint_iter.enumerate() {
            let variables = constraint.variables();
            let allowed_rows = constraint.allowed_rows();
            let mut compatible_rows = 0_usize;
            let mut supports = [0_u8; 6];
            for row in allowed_rows.iter().copied() {
                if variables
                    .iter()
                    .enumerate()
                    .all(|(position, variable)| domains[*variable] & row_domain(row, position) != 0)
                {
                    compatible_rows += 1;
                    for (position, support) in supports.iter_mut().enumerate().take(variables.len())
                    {
                        *support |= row_domain(row, position);
                    }
                }
            }
            if compatible_rows == 0 {
                let Some(conflict) = ConstraintConflict::from_view(constraint_index, constraint)
                else {
                    return finish_propagation(
                        control,
                        search_nodes,
                        PropagationResult::InvalidConstraint,
                    );
                };
                return finish_propagation(
                    control,
                    search_nodes,
                    PropagationResult::Conflict(conflict),
                );
            }
            for (position, variable) in variables.iter().enumerate() {
                let next = domains[*variable] & supports[position];
                if next == 0 {
                    let Some(conflict) =
                        ConstraintConflict::from_view(constraint_index, constraint)
                    else {
                        return finish_propagation(
                            control,
                            search_nodes,
                            PropagationResult::InvalidConstraint,
                        );
                    };
                    return finish_propagation(
                        control,
                        search_nodes,
                        PropagationResult::Conflict(conflict),
                    );
                }
                if next != domains[*variable] {
                    if let Some(changes) = &mut trail {
                        (**changes).push((*variable, domains[*variable]));
                    }
                    domains[*variable] = next;
                    changed = true;
                }
            }
            processed_since_poll += 1;
            if processed_since_poll == CONTROL_BATCH_RECORDS {
                if let Err(abort) = poll_control(
                    control,
                    ConstraintSolverEvent::PropagationBatch,
                    search_nodes,
                ) {
                    return propagation_abort(abort);
                }
                processed_since_poll = 0;
            }
        }
        if !changed {
            return finish_propagation(control, search_nodes, PropagationResult::Stable);
        }
    }
}

fn finish_propagation<F>(
    control: &mut F,
    search_nodes: usize,
    result: PropagationResult,
) -> PropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    match poll_control(
        control,
        ConstraintSolverEvent::PropagationBatch,
        search_nodes,
    ) {
        Ok(()) => result,
        Err(abort) => propagation_abort(abort),
    }
}

const fn propagation_abort(control: ConstraintSolverControl) -> PropagationResult {
    match control {
        ConstraintSolverControl::Continue => PropagationResult::Stable,
        ConstraintSolverControl::DeadlineReached => PropagationResult::DeadlineReached,
        ConstraintSolverControl::Cancelled => PropagationResult::Cancelled,
        ConstraintSolverControl::WorkingMemoryLimit => PropagationResult::WorkingMemoryLimit,
    }
}

fn row_domain(row: u8, position: usize) -> u8 {
    if row & (1 << position) == 0 {
        DOMAIN_FALSE
    } else {
        DOMAIN_TRUE
    }
}

struct VariableComponents {
    variables: Vec<usize>,
    ranges: Vec<Range<usize>>,
}

fn variable_components<F>(
    variable_count: usize,
    constraints: &ConstraintSet,
    control: &mut F,
    search_nodes: usize,
) -> Result<VariableComponents, ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    poll_control(
        control,
        ConstraintSolverEvent::PropagationBatch,
        search_nodes,
    )?;
    let mut parents = Vec::new();
    parents
        .try_reserve_exact(variable_count)
        .map_err(|_| ConstraintSolverControl::WorkingMemoryLimit)?;
    parents.extend(0..variable_count);
    let mut ranks = Vec::new();
    ranks
        .try_reserve_exact(variable_count)
        .map_err(|_| ConstraintSolverControl::WorkingMemoryLimit)?;
    ranks.resize(variable_count, 0_u8);
    let mut processed_since_poll = 0_usize;
    poll_iterator_initialization(constraints, control, search_nodes)?;
    let constraint_iter = constraints
        .try_iter()
        .map_err(|_| ConstraintSolverControl::WorkingMemoryLimit)?;
    for constraint in constraint_iter {
        if let Some((&first, rest)) = constraint.variables().split_first() {
            for &second in rest {
                union_components(&mut parents, &mut ranks, first, second);
            }
        }
        poll_after_record_batch(control, search_nodes, &mut processed_since_poll)?;
    }
    let mut grouped_variables = Vec::new();
    grouped_variables
        .try_reserve_exact(variable_count)
        .map_err(|_| ConstraintSolverControl::WorkingMemoryLimit)?;
    for variable in 0..variable_count {
        poll_after_record_batch(control, search_nodes, &mut processed_since_poll)?;
        let root = find_component_root(&mut parents, variable);
        grouped_variables.push((root, variable));
    }
    grouped_variables.sort_unstable();
    let mut component_variables = Vec::new();
    component_variables
        .try_reserve_exact(variable_count)
        .map_err(|_| ConstraintSolverControl::WorkingMemoryLimit)?;
    let mut component_ranges = Vec::new();
    component_ranges
        .try_reserve_exact(variable_count)
        .map_err(|_| ConstraintSolverControl::WorkingMemoryLimit)?;
    let mut cursor = 0_usize;
    while cursor < grouped_variables.len() {
        poll_after_record_batch(control, search_nodes, &mut processed_since_poll)?;
        let end = grouped_variables[cursor..]
            .iter()
            .position(|(root, _)| *root != grouped_variables[cursor].0)
            .map_or(grouped_variables.len(), |offset| cursor + offset);
        let component_start = component_variables.len();
        for &(_, variable) in &grouped_variables[cursor..end] {
            poll_after_record_batch(control, search_nodes, &mut processed_since_poll)?;
            component_variables.push(variable);
        }
        component_ranges.push(component_start..component_variables.len());
        cursor = end;
    }
    component_ranges.sort_unstable_by_key(|component| {
        (
            component.len(),
            component_variables
                .get(component.start)
                .copied()
                .unwrap_or(usize::MAX),
        )
    });
    if processed_since_poll != 0 {
        poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        )?;
    }
    Ok(VariableComponents {
        variables: component_variables,
        ranges: component_ranges,
    })
}

fn find_component_root(parents: &mut [usize], variable: usize) -> usize {
    let mut root = variable;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = variable;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_components(parents: &mut [usize], ranks: &mut [u8], first: usize, second: usize) {
    let first_root = find_component_root(parents, first);
    let second_root = find_component_root(parents, second);
    if first_root == second_root {
        return;
    }
    match ranks[first_root].cmp(&ranks[second_root]) {
        std::cmp::Ordering::Less => parents[first_root] = second_root,
        std::cmp::Ordering::Greater => parents[second_root] = first_root,
        std::cmp::Ordering::Equal => {
            parents[second_root] = first_root;
            ranks[first_root] = ranks[first_root].saturating_add(1);
        }
    }
}

fn poll_after_record_batch<F>(
    control: &mut F,
    search_nodes: usize,
    processed_since_poll: &mut usize,
) -> Result<(), ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    *processed_since_poll += 1;
    if *processed_since_poll < CONTROL_BATCH_RECORDS {
        return Ok(());
    }
    *processed_since_poll = 0;
    poll_control(
        control,
        ConstraintSolverEvent::PropagationBatch,
        search_nodes,
    )
}

fn poll_iterator_initialization<F>(
    constraints: &ConstraintSet,
    control: &mut F,
    search_nodes: usize,
) -> Result<(), ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let mut pending = 0_usize;
    for _ in 0..constraints.iterator_initialization_records() {
        poll_after_record_batch(control, search_nodes, &mut pending)?;
    }
    if pending != 0 {
        poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        )?;
    }
    Ok(())
}

fn poll_control<F>(
    control: &mut F,
    event: ConstraintSolverEvent,
    search_nodes: usize,
) -> Result<(), ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    match control(event, search_nodes) {
        ConstraintSolverControl::Continue => Ok(()),
        abort => Err(abort),
    }
}

fn control_abort_result<F>(
    control: &mut F,
    event: ConstraintSolverEvent,
    search_nodes: usize,
) -> Option<ConstraintSolverResult>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    match control(event, search_nodes) {
        ConstraintSolverControl::Continue => None,
        ConstraintSolverControl::DeadlineReached => {
            Some(ConstraintSolverResult::DeadlineReached { search_nodes })
        }
        ConstraintSolverControl::Cancelled => Some(ConstraintSolverResult::Cancelled),
        ConstraintSolverControl::WorkingMemoryLimit => {
            Some(ConstraintSolverResult::WorkingMemoryLimit {
                observed: usize::MAX,
            })
        }
    }
}

fn solver_abort_result(
    abort: ConstraintSolverControl,
    search_nodes: usize,
    required_working_memory: usize,
) -> ConstraintSolverResult {
    match abort {
        ConstraintSolverControl::Continue => ConstraintSolverResult::InvalidConstraint,
        ConstraintSolverControl::DeadlineReached => {
            ConstraintSolverResult::DeadlineReached { search_nodes }
        }
        ConstraintSolverControl::Cancelled => ConstraintSolverResult::Cancelled,
        ConstraintSolverControl::WorkingMemoryLimit => ConstraintSolverResult::WorkingMemoryLimit {
            observed: required_working_memory,
        },
    }
}

enum SearchResult {
    Satisfied(Vec<u8>),
    Unsatisfied(Option<ConstraintConflict>),
    Limit(usize),
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit,
    InvalidConstraint,
}

struct SearchFrame {
    component_position: usize,
    variable: usize,
    next_branch: u8,
    trail_mark: usize,
}

fn search_component<F>(
    mut domains: Vec<u8>,
    component: &[usize],
    constraints: &ConstraintSet,
    max_search_nodes: usize,
    search_nodes: &mut usize,
    control: &mut F,
) -> SearchResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let Some((first_position, first_variable)) = first_unassigned_variable(component, &domains, 0)
    else {
        return SearchResult::Satisfied(domains);
    };

    let mut trail = Vec::new();
    if trail.try_reserve_exact(component.len()).is_err() {
        return SearchResult::WorkingMemoryLimit;
    }
    let mut stack = Vec::new();
    if stack.try_reserve_exact(component.len()).is_err() {
        return SearchResult::WorkingMemoryLimit;
    }
    stack.push(SearchFrame {
        component_position: first_position,
        variable: first_variable,
        next_branch: 0,
        trail_mark: 0,
    });

    loop {
        let Some(frame) = stack.last_mut() else {
            // Branch-local conflicts do not identify a single globally
            // contradictory constraint. Exhausting every explicit frame is
            // reported distinctly so callers can produce a search-exhausted
            // proof.
            return SearchResult::Unsatisfied(None);
        };
        if frame.next_branch >= 2 {
            let trail_mark = frame.trail_mark;
            stack.pop();
            undo_domains(&mut domains, &mut trail, trail_mark);
            continue;
        }

        let component_position = frame.component_position;
        let variable = frame.variable;
        let trail_mark = frame.trail_mark;
        let domain = if frame.next_branch == 0 {
            DOMAIN_FALSE
        } else {
            DOMAIN_TRUE
        };
        frame.next_branch += 1;
        undo_domains(&mut domains, &mut trail, trail_mark);
        debug_assert_eq!(domains[variable], DOMAIN_BOTH);

        let observed = search_nodes.checked_add(1).unwrap_or(usize::MAX);
        let prior_search_nodes = *search_nodes;
        *search_nodes = observed;
        match control(ConstraintSolverEvent::SearchNode, observed) {
            ConstraintSolverControl::Continue => {}
            ConstraintSolverControl::DeadlineReached => {
                return SearchResult::DeadlineReached;
            }
            ConstraintSolverControl::Cancelled => return SearchResult::Cancelled,
            ConstraintSolverControl::WorkingMemoryLimit => {
                return SearchResult::WorkingMemoryLimit;
            }
        }
        if observed > max_search_nodes {
            *search_nodes = prior_search_nodes;
            return SearchResult::Limit(observed);
        }

        trail.push((variable, domains[variable]));
        domains[variable] = domain;
        match propagate_with_trail(
            &mut domains,
            constraints,
            control,
            *search_nodes,
            &mut trail,
        ) {
            PropagationResult::Stable => {
                let Some((next_position, next_variable)) =
                    first_unassigned_variable(component, &domains, component_position + 1)
                else {
                    return SearchResult::Satisfied(domains);
                };
                stack.push(SearchFrame {
                    component_position: next_position,
                    variable: next_variable,
                    next_branch: 0,
                    trail_mark: trail.len(),
                });
            }
            PropagationResult::Conflict(_) => {}
            PropagationResult::DeadlineReached => return SearchResult::DeadlineReached,
            PropagationResult::Cancelled => return SearchResult::Cancelled,
            PropagationResult::WorkingMemoryLimit => return SearchResult::WorkingMemoryLimit,
            PropagationResult::InvalidConstraint => return SearchResult::InvalidConstraint,
        }
    }
}

fn first_unassigned_variable(
    component: &[usize],
    domains: &[u8],
    start: usize,
) -> Option<(usize, usize)> {
    component
        .iter()
        .copied()
        .enumerate()
        .skip(start)
        .find(|(_, variable)| domains[*variable] == DOMAIN_BOTH)
}

fn undo_domains(domains: &mut [u8], trail: &mut Vec<(usize, u8)>, trail_mark: usize) {
    while trail.len() > trail_mark {
        let (variable, previous) = trail
            .pop()
            .expect("the trail length was checked before popping");
        domains[variable] = previous;
    }
}

fn constraint_accepts(constraint: ConstraintView<'_>, assignment: &[bool]) -> bool {
    let row = constraint
        .variables()
        .iter()
        .enumerate()
        .fold(0_u8, |row, (position, variable)| {
            row | (u8::from(assignment[*variable]) << position)
        });
    constraint.allowed_rows().contains(&row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_constraints() -> ConstraintSet {
        ConstraintSet::from_explicit(Vec::new())
    }

    fn constraint(variables: &[usize], allowed_rows: &[u8]) -> TupleConstraint {
        TupleConstraint {
            kind: FacewiseConstraintKind::Transitivity,
            variables: variables.to_vec(),
            allowed_rows: allowed_rows.to_vec(),
            faces: Vec::new(),
            supporting_cell: None,
        }
    }

    #[test]
    fn propagation_and_canonical_false_first_search_are_deterministic() {
        let constraints = vec![constraint(&[0, 1], &[0b00, 0b11])];
        let result = solve_constraints(2, &constraints, &[None, None], 10, |_, _| {
            ConstraintSolverControl::Continue
        });
        assert_eq!(
            result,
            ConstraintSolverResult::Satisfied {
                assignment: vec![false, false],
                search_nodes: 1,
            }
        );
    }

    #[test]
    fn exhaustive_conflict_is_distinct_from_limit_timeout_and_cancel() {
        let impossible = vec![
            constraint(&[0], &[0]),
            TupleConstraint {
                allowed_rows: vec![1],
                ..constraint(&[0], &[0])
            },
        ];
        assert!(matches!(
            solve_constraints(1, &impossible, &[None], 10, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::Unsatisfied { .. }
        ));
        assert_eq!(
            solve_constraints(1, &[], &[None], 0, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::SearchNodeLimit { observed: 1 }
        );
        assert!(matches!(
            solve_constraints(1, &[], &[None], 10, |_, _| {
                ConstraintSolverControl::DeadlineReached
            }),
            ConstraintSolverResult::DeadlineReached { .. }
        ));
        assert_eq!(
            solve_constraints(1, &[], &[None], 10, |_, _| {
                ConstraintSolverControl::Cancelled
            }),
            ConstraintSolverResult::Cancelled
        );
    }

    #[test]
    fn fresh_evaluator_rejects_invalid_or_duplicate_variable_tuples() {
        let duplicate = constraint(&[0, 0], &[0]);
        assert_eq!(
            solve_constraints(1, &[duplicate], &[None], 10, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::InvalidConstraint
        );
        let out_of_range_row = constraint(&[0], &[2]);
        assert_eq!(
            solve_constraints(1, &[out_of_range_row], &[None], 10, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::InvalidConstraint
        );
    }

    #[test]
    fn six_variable_taco_tuple_is_supported() {
        let constraint = TupleConstraint {
            kind: FacewiseConstraintKind::TacoTaco,
            variables: vec![0, 1, 2, 3, 4, 5],
            allowed_rows: vec![0b11_1111],
            faces: vec![0, 1, 2, 3],
            supporting_cell: None,
        };
        assert_eq!(
            solve_constraints(6, &[constraint], &[Some(true); 6], 0, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::Satisfied {
                assignment: vec![true; 6],
                search_nodes: 0,
            }
        );
    }

    #[test]
    fn propagation_polls_deadline_within_a_bounded_constraint_batch() {
        let constraints = (0..2_048)
            .map(|_| constraint(&[0], &[0, 1]))
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        let result = solve_constraints(1, &constraints, &[Some(false)], 10, |event, _| {
            events.push(event);
            if events.len() == 4 {
                ConstraintSolverControl::DeadlineReached
            } else {
                ConstraintSolverControl::Continue
            }
        });

        assert_eq!(
            result,
            ConstraintSolverResult::DeadlineReached { search_nodes: 0 }
        );
        assert_eq!(
            events,
            vec![
                ConstraintSolverEvent::PropagationBatch,
                ConstraintSolverEvent::PropagationBatch,
                ConstraintSolverEvent::PropagationBatch,
                ConstraintSolverEvent::PropagationBatch,
            ],
            "validation uses the first two polls, propagation starts at the third, and the fourth must interrupt its first bounded batch"
        );
    }

    #[test]
    fn conflict_rechecks_deadline_before_becoming_an_impossible_proof() {
        let constraints = vec![constraint(&[0], &[1])];
        let mut calls = 0_usize;
        let result = solve_constraints(1, &constraints, &[Some(false)], 10, |_, _| {
            calls += 1;
            if calls == 2 {
                ConstraintSolverControl::DeadlineReached
            } else {
                ConstraintSolverControl::Continue
            }
        });

        assert_eq!(
            result,
            ConstraintSolverResult::DeadlineReached { search_nodes: 0 },
            "a conflict discovered after the deadline must remain Unknown, not become an Impossible verdict"
        );
    }

    #[test]
    fn conflict_rechecks_cancellation_before_becoming_an_impossible_proof() {
        let constraints = vec![constraint(&[0], &[1])];
        let mut calls = 0_usize;
        let result = solve_constraints(1, &constraints, &[Some(false)], 10, |_, _| {
            calls += 1;
            if calls == 2 {
                ConstraintSolverControl::Cancelled
            } else {
                ConstraintSolverControl::Continue
            }
        });

        assert_eq!(
            result,
            ConstraintSolverResult::Cancelled,
            "a conflict discovered after cancellation must remain Unknown, not become an Impossible verdict"
        );
    }

    #[test]
    fn component_construction_observes_cancellation_before_search() {
        let constraints = vec![constraint(&[0, 1], &[0, 1, 2, 3])];
        let mut events = Vec::new();
        let result = solve_constraints(2, &constraints, &[None, None], 10, |event, _| {
            events.push(event);
            if events.len() == 3 {
                ConstraintSolverControl::Cancelled
            } else {
                ConstraintSolverControl::Continue
            }
        });

        assert_eq!(result, ConstraintSolverResult::Cancelled);
        assert_eq!(
            events,
            vec![
                ConstraintSolverEvent::PropagationBatch,
                ConstraintSolverEvent::PropagationBatch,
                ConstraintSolverEvent::PropagationBatch,
            ],
            "component construction needs its own cooperative checkpoint before DFS begins"
        );
    }

    #[test]
    fn component_search_preserves_recursive_false_first_fixture() {
        let constraints = vec![
            constraint(&[0, 1], &[0b00, 0b11]),
            constraint(&[1, 2], &[0b01, 0b10]),
        ];

        assert_eq!(
            solve_constraints(3, &constraints, &[None; 3], 10, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::Satisfied {
                assignment: vec![false, false, true],
                search_nodes: 1,
            },
            "the explicit stack must preserve variable order, false-before-true order, and node accounting"
        );
    }

    #[test]
    fn component_search_rolls_back_propagated_domains_before_the_true_branch() {
        let constraints = vec![
            // With variable 0=false this requires variables 1 and 2 to
            // agree. With variable 0=true every suffix is allowed.
            constraint(&[0, 1, 2], &[0b000, 0b001, 0b011, 0b101, 0b110, 0b111]),
            // With variable 0=false this instead requires variables 1 and 2
            // to disagree, making the false root branch exhaustively
            // contradictory only after variable 1 is chosen.
            constraint(&[0, 1, 2], &[0b001, 0b010, 0b011, 0b100, 0b101, 0b111]),
        ];

        assert_eq!(
            solve_constraints(3, &constraints, &[None; 3], 10, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::Satisfied {
                assignment: vec![true, false, false],
                search_nodes: 6,
            },
            "propagation changes from the exhausted false branch must be undone before the true branch"
        );
    }

    #[test]
    fn component_search_closes_fifty_thousand_variables_at_the_node_limit() {
        const VARIABLE_COUNT: usize = 50_000;
        let component = (0..VARIABLE_COUNT).collect::<Vec<_>>();
        let mut search_nodes = 0_usize;

        let result = search_component(
            vec![DOMAIN_BOTH; VARIABLE_COUNT],
            &component,
            &empty_constraints(),
            VARIABLE_COUNT - 1,
            &mut search_nodes,
            &mut |_, _| ConstraintSolverControl::Continue,
        );

        assert!(matches!(result, SearchResult::Limit(VARIABLE_COUNT)));
        assert_eq!(search_nodes, VARIABLE_COUNT - 1);
    }

    #[test]
    fn component_search_closes_large_depth_on_deadline_and_cancellation() {
        const VARIABLE_COUNT: usize = 50_000;
        const STOP_AT: usize = 20_000;
        let component = (0..VARIABLE_COUNT).collect::<Vec<_>>();

        for expected_control in [
            ConstraintSolverControl::DeadlineReached,
            ConstraintSolverControl::Cancelled,
        ] {
            let mut search_nodes = 0_usize;
            let result = search_component(
                vec![DOMAIN_BOTH; VARIABLE_COUNT],
                &component,
                &empty_constraints(),
                VARIABLE_COUNT,
                &mut search_nodes,
                &mut |_, observed| {
                    if observed == STOP_AT {
                        expected_control
                    } else {
                        ConstraintSolverControl::Continue
                    }
                },
            );

            assert_eq!(search_nodes, STOP_AT);
            assert!(matches!(
                (expected_control, result),
                (
                    ConstraintSolverControl::DeadlineReached,
                    SearchResult::DeadlineReached
                ) | (ConstraintSolverControl::Cancelled, SearchResult::Cancelled)
            ));
        }
    }

    #[test]
    fn solver_working_memory_budget_accepts_exact_limit_and_rejects_one_byte_less() {
        let fixed = [Some(false), Some(true), Some(false), Some(true)];
        let required =
            solver_working_memory_upper_bound(fixed.len()).expect("small fixture fits usize");
        assert!(matches!(
            solve_constraints_with_memory(4, &empty_constraints(), &fixed, 0, required, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::Satisfied { .. }
        ));
        assert_eq!(
            solve_constraints_with_memory(
                4,
                &empty_constraints(),
                &fixed,
                0,
                required - 1,
                |_, _| { ConstraintSolverControl::Continue }
            ),
            ConstraintSolverResult::WorkingMemoryLimit { observed: required }
        );
    }

    #[test]
    fn solver_working_memory_preflight_rejects_large_variable_count_before_workspace_allocation() {
        const VARIABLE_COUNT: usize = 100_000;
        let fixed = vec![Some(false); VARIABLE_COUNT];
        let required =
            solver_working_memory_upper_bound(VARIABLE_COUNT).expect("fixture fits usize");
        assert_eq!(
            solve_constraints_with_memory(
                VARIABLE_COUNT,
                &empty_constraints(),
                &fixed,
                0,
                1,
                |_, _| { ConstraintSolverControl::Continue }
            ),
            ConstraintSolverResult::WorkingMemoryLimit { observed: required }
        );
    }

    #[test]
    fn solver_working_memory_size_overflow_is_fail_closed() {
        assert_eq!(solver_working_memory_upper_bound(usize::MAX), None);
    }

    #[test]
    fn deadline_and_cancellation_override_a_pending_memory_limit() {
        let fixed = [None; 4];
        assert!(matches!(
            solve_constraints_with_memory(4, &empty_constraints(), &fixed, 10, 0, |_, _| {
                ConstraintSolverControl::DeadlineReached
            }),
            ConstraintSolverResult::DeadlineReached { .. }
        ));
        assert_eq!(
            solve_constraints_with_memory(4, &empty_constraints(), &fixed, 10, 0, |_, _| {
                ConstraintSolverControl::Cancelled
            }),
            ConstraintSolverResult::Cancelled
        );
    }

    #[test]
    fn deadline_and_cancellation_override_a_pending_search_node_limit() {
        let fixed = [None];
        assert!(matches!(
            solve_constraints_with_memory(
                1,
                &empty_constraints(),
                &fixed,
                0,
                usize::MAX,
                |event, _| {
                    if event == ConstraintSolverEvent::SearchNode {
                        ConstraintSolverControl::DeadlineReached
                    } else {
                        ConstraintSolverControl::Continue
                    }
                }
            ),
            ConstraintSolverResult::DeadlineReached { .. }
        ));
        assert_eq!(
            solve_constraints_with_memory(
                1,
                &empty_constraints(),
                &fixed,
                0,
                usize::MAX,
                |event, _| {
                    if event == ConstraintSolverEvent::SearchNode {
                        ConstraintSolverControl::Cancelled
                    } else {
                        ConstraintSolverControl::Continue
                    }
                }
            ),
            ConstraintSolverResult::Cancelled
        );
    }
}
