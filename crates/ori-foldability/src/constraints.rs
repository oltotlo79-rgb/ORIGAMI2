use std::ops::Range;

use crate::{FacewiseConstraintKind, OverlapCellKey};

const DOMAIN_FALSE: u8 = 0b01;
const DOMAIN_TRUE: u8 = 0b10;
const DOMAIN_BOTH: u8 = DOMAIN_FALSE | DOMAIN_TRUE;
const CONTROL_BATCH_RECORDS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TupleConstraint {
    pub kind: FacewiseConstraintKind,
    pub variables: Vec<usize>,
    pub allowed_rows: Vec<u8>,
    pub faces: Vec<usize>,
    pub supporting_cell: Option<OverlapCellKey>,
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
        conflict_constraint: Option<usize>,
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
        // The explicit search stack and rollback trail can each contain at
        // most one live record per variable in the active component.
        (variable_count, std::mem::size_of::<SearchFrame>()),
        (variable_count, std::mem::size_of::<(usize, u8)>()),
    ];
    allocations
        .into_iter()
        .try_fold(0_usize, |total, (count, element_size)| {
            total.checked_add(count.checked_mul(element_size)?)
        })
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
    solve_constraints_with_memory(
        variable_count,
        constraints,
        fixed_assignments,
        max_search_nodes,
        usize::MAX,
        control,
    )
}

pub(crate) fn solve_constraints_with_memory<F>(
    variable_count: usize,
    constraints: &[TupleConstraint],
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
    let mut validated_since_poll = 0_usize;
    for constraint in constraints {
        if !valid_constraint(constraint, variable_count) {
            if let Some(abort) =
                control_abort_result(&mut control, ConstraintSolverEvent::PropagationBatch, 0)
            {
                return abort;
            }
            return ConstraintSolverResult::InvalidConstraint;
        }
        validated_since_poll += 1;
        if validated_since_poll == CONTROL_BATCH_RECORDS {
            if let Some(abort) =
                control_abort_result(&mut control, ConstraintSolverEvent::PropagationBatch, 0)
            {
                return abort;
            }
            validated_since_poll = 0;
        }
    }
    let required_working_memory =
        solver_working_memory_upper_bound(variable_count).unwrap_or(usize::MAX);
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
    match propagate(&mut domains, constraints, &mut control, search_nodes) {
        PropagationResult::Stable => {}
        PropagationResult::Conflict(index) => {
            if let Some(abort) = control_abort_result(
                &mut control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            ) {
                return abort;
            }
            return ConstraintSolverResult::Unsatisfied {
                conflict_constraint: Some(index),
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
    for (index, constraint) in constraints.iter().enumerate() {
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
            return ConstraintSolverResult::Unsatisfied {
                conflict_constraint: Some(index),
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

fn valid_constraint(constraint: &TupleConstraint, variable_count: usize) -> bool {
    let arity = constraint.variables.len();
    if arity > 6
        || constraint.allowed_rows.is_empty()
        || constraint
            .variables
            .iter()
            .any(|variable| *variable >= variable_count)
    {
        return false;
    }
    if constraint
        .variables
        .iter()
        .enumerate()
        .any(|(index, variable)| constraint.variables[..index].contains(variable))
    {
        return false;
    }
    let row_limit = 1_u8.checked_shl(arity as u32).unwrap_or(0);
    constraint.allowed_rows.iter().all(|row| *row < row_limit)
}

enum PropagationResult {
    Stable,
    Conflict(usize),
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit,
}

fn propagate<F>(
    domains: &mut [u8],
    constraints: &[TupleConstraint],
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
    constraints: &[TupleConstraint],
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
    constraints: &[TupleConstraint],
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
        for (constraint_index, constraint) in constraints.iter().enumerate() {
            let mut compatible_rows = 0_usize;
            let mut supports = [0_u8; 6];
            for row in constraint.allowed_rows.iter().copied() {
                if constraint
                    .variables
                    .iter()
                    .enumerate()
                    .all(|(position, variable)| domains[*variable] & row_domain(row, position) != 0)
                {
                    compatible_rows += 1;
                    for (position, support) in supports
                        .iter_mut()
                        .enumerate()
                        .take(constraint.variables.len())
                    {
                        *support |= row_domain(row, position);
                    }
                }
            }
            if compatible_rows == 0 {
                return finish_propagation(
                    control,
                    search_nodes,
                    PropagationResult::Conflict(constraint_index),
                );
            }
            for (position, variable) in constraint.variables.iter().enumerate() {
                let next = domains[*variable] & supports[position];
                if next == 0 {
                    return finish_propagation(
                        control,
                        search_nodes,
                        PropagationResult::Conflict(constraint_index),
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
    constraints: &[TupleConstraint],
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
    for constraint in constraints {
        if let Some((&first, rest)) = constraint.variables.split_first() {
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

enum SearchResult {
    Satisfied(Vec<u8>),
    Unsatisfied(Option<usize>),
    Limit(usize),
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit,
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
    constraints: &[TupleConstraint],
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

fn constraint_accepts(constraint: &TupleConstraint, assignment: &[bool]) -> bool {
    let row = constraint
        .variables
        .iter()
        .enumerate()
        .fold(0_u8, |row, (position, variable)| {
            row | (u8::from(assignment[*variable]) << position)
        });
    constraint.allowed_rows.contains(&row)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            &[],
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
                &[],
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
            solve_constraints_with_memory(4, &[], &fixed, 0, required, |_, _| {
                ConstraintSolverControl::Continue
            }),
            ConstraintSolverResult::Satisfied { .. }
        ));
        assert_eq!(
            solve_constraints_with_memory(4, &[], &fixed, 0, required - 1, |_, _| {
                ConstraintSolverControl::Continue
            }),
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
            solve_constraints_with_memory(VARIABLE_COUNT, &[], &fixed, 0, 1, |_, _| {
                ConstraintSolverControl::Continue
            }),
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
            solve_constraints_with_memory(4, &[], &fixed, 10, 0, |_, _| {
                ConstraintSolverControl::DeadlineReached
            }),
            ConstraintSolverResult::DeadlineReached { .. }
        ));
        assert_eq!(
            solve_constraints_with_memory(4, &[], &fixed, 10, 0, |_, _| {
                ConstraintSolverControl::Cancelled
            }),
            ConstraintSolverResult::Cancelled
        );
    }

    #[test]
    fn deadline_and_cancellation_override_a_pending_search_node_limit() {
        let fixed = [None];
        assert!(matches!(
            solve_constraints_with_memory(1, &[], &fixed, 0, usize::MAX, |event, _| {
                if event == ConstraintSolverEvent::SearchNode {
                    ConstraintSolverControl::DeadlineReached
                } else {
                    ConstraintSolverControl::Continue
                }
            }),
            ConstraintSolverResult::DeadlineReached { .. }
        ));
        assert_eq!(
            solve_constraints_with_memory(1, &[], &fixed, 0, usize::MAX, |event, _| {
                if event == ConstraintSolverEvent::SearchNode {
                    ConstraintSolverControl::Cancelled
                } else {
                    ConstraintSolverControl::Continue
                }
            }),
            ConstraintSolverResult::Cancelled
        );
    }
}
