use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CompactCompletionResult {
    Satisfied {
        candidate: Vec<u8>,
        search_nodes: usize,
    },
    Fallback {
        search_nodes: usize,
    },
    SearchNodeLimit {
        observed: usize,
    },
    DeadlineReached {
        search_nodes: usize,
    },
    Cancelled,
    WorkingMemoryLimit,
    InvalidConstraint,
}

enum CompactPropagationResult {
    Stable,
    Conflict,
    DeadlineReached,
    Cancelled,
    WorkingMemoryLimit,
    InvalidConstraint,
}

#[cfg(test)]
enum CompactTransitivityPropagationResult {
    Stable,
    Changed,
    Conflict,
    InvalidConstraint,
}

enum CompactOrderResult {
    Completed,
    BranchConflict,
    Fallback,
    InvalidConstraint,
}

enum CompactFamilyOrderResult {
    Completed,
    FixedCycle,
    InvalidConstraint,
}

enum CompactCandidateCheck {
    Accepts,
    ExplicitConflict(usize),
    CompactConflict,
    InvalidConstraint,
}

pub(super) struct CompactSearchFrame {
    variable: usize,
    next_branch: u8,
    rollback_mark: CompactRollbackMark,
}

#[derive(Clone, Copy)]
struct CompactRollbackMark {
    domains: usize,
    closure: usize,
}

struct CompactOrderScratch {
    indegrees: Vec<usize>,
    ranks: Vec<usize>,
    selected: Vec<u8>,
}

struct CompactExplicitWorklist {
    offsets: Vec<usize>,
    adjacency: Vec<usize>,
    queue: Vec<usize>,
    queued: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(super) struct CompactPairIncidence {
    family: usize,
    first: usize,
    second: usize,
}

#[derive(Clone, Copy)]
pub(super) struct CompactReachabilityWordChange {
    index: usize,
    previous: usize,
}

struct CompactTransitiveClosure {
    family_offsets: Vec<usize>,
    variable_offsets: Vec<usize>,
    incidences: Vec<CompactPairIncidence>,
    reachability: Vec<usize>,
    fixed_queue: Vec<usize>,
    word_trail: Vec<CompactReachabilityWordChange>,
}

impl CompactExplicitWorklist {
    fn try_new<F>(
        variable_count: usize,
        constraints: &ConstraintSet,
        control: &mut F,
    ) -> Result<Self, CompactPropagationResult>
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        if constraints.compact_explicit_len == 0 {
            return Ok(Self {
                offsets: Vec::new(),
                adjacency: Vec::new(),
                queue: Vec::new(),
                queued: Vec::new(),
            });
        }

        let mut pending = 0_usize;
        let mut cursors = Vec::<usize>::new();
        let mut offsets = Vec::<usize>::new();
        let mut adjacency = Vec::<usize>::new();
        let mut queue = Vec::<usize>::new();
        let mut queued = Vec::<u8>::new();
        if cursors.try_reserve_exact(variable_count).is_err()
            || offsets
                .try_reserve_exact(variable_count.saturating_add(1))
                .is_err()
            || adjacency
                .try_reserve_exact(constraints.compact_explicit_incidence_len)
                .is_err()
            || queue
                .try_reserve_exact(constraints.compact_explicit_len)
                .is_err()
            || queued
                .try_reserve_exact(constraints.explicit.len())
                .is_err()
        {
            return Err(CompactPropagationResult::WorkingMemoryLimit);
        }
        for _ in 0..variable_count {
            cursors.push(0);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        for constraint in &constraints.explicit {
            if tuple_constraint_is_tautology(constraint) {
                continue;
            }
            for variable in &constraint.variables {
                let Some(count) = cursors.get_mut(*variable) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                let Some(next) = count.checked_add(1) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                *count = next;
                poll_after_record_batch(control, 0, &mut pending)
                    .map_err(compact_propagation_abort)?;
            }
        }
        offsets.push(0);
        for count in &cursors {
            let Some(next) = offsets
                .last()
                .copied()
                .and_then(|offset| offset.checked_add(*count))
            else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            offsets.push(next);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        if offsets.last().copied() != Some(constraints.compact_explicit_incidence_len) {
            return Err(CompactPropagationResult::InvalidConstraint);
        }
        for _ in 0..constraints.compact_explicit_incidence_len {
            adjacency.push(usize::MAX);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        for (variable, cursor) in cursors.iter_mut().enumerate() {
            *cursor = offsets[variable];
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        for (constraint_index, constraint) in constraints.explicit.iter().enumerate() {
            if tuple_constraint_is_tautology(constraint) {
                continue;
            }
            for variable in &constraint.variables {
                let Some(cursor) = cursors.get_mut(*variable) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                let Some(slot) = adjacency.get_mut(*cursor) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                *slot = constraint_index;
                let Some(next) = cursor.checked_add(1) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                *cursor = next;
                poll_after_record_batch(control, 0, &mut pending)
                    .map_err(compact_propagation_abort)?;
            }
        }
        for _ in 0..constraints.explicit.len() {
            queued.push(0);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        if pending != 0 {
            poll_control(control, ConstraintSolverEvent::PropagationBatch, 0)
                .map_err(compact_propagation_abort)?;
        }
        Ok(Self {
            offsets,
            adjacency,
            queue,
            queued,
        })
    }

    fn enqueue_all<F>(
        &mut self,
        constraints: &ConstraintSet,
        search_nodes: usize,
        control: &mut F,
    ) -> Result<(), CompactPropagationResult>
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        let mut pending = 0_usize;
        for (index, constraint) in constraints.explicit.iter().enumerate().rev() {
            if !tuple_constraint_is_tautology(constraint) {
                self.enqueue_constraint(index)?;
            }
            poll_after_record_batch(control, search_nodes, &mut pending)
                .map_err(compact_propagation_abort)?;
        }
        if pending != 0 {
            poll_control(
                control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            )
            .map_err(compact_propagation_abort)?;
        }
        Ok(())
    }

    fn enqueue_variable<F>(
        &mut self,
        variable: usize,
        search_nodes: usize,
        control: &mut F,
    ) -> Result<(), CompactPropagationResult>
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        if self.offsets.is_empty() {
            return Ok(());
        }
        let Some(range) = self
            .offsets
            .get(variable)
            .copied()
            .zip(self.offsets.get(variable.saturating_add(1)).copied())
            .map(|(start, end)| start..end)
        else {
            return Err(CompactPropagationResult::InvalidConstraint);
        };
        let mut pending = 0_usize;
        for position in range.rev() {
            let Some(constraint) = self.adjacency.get(position).copied() else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            self.enqueue_constraint(constraint)?;
            poll_after_record_batch(control, search_nodes, &mut pending)
                .map_err(compact_propagation_abort)?;
        }
        if pending != 0 {
            poll_control(
                control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            )
            .map_err(compact_propagation_abort)?;
        }
        Ok(())
    }

    fn enqueue_constraint(&mut self, constraint: usize) -> Result<(), CompactPropagationResult> {
        let Some(queued) = self.queued.get_mut(constraint) else {
            return Err(CompactPropagationResult::InvalidConstraint);
        };
        if *queued != 0 {
            return Ok(());
        }
        if self.queue.len() >= self.queue.capacity() {
            return Err(CompactPropagationResult::WorkingMemoryLimit);
        }
        *queued = 1;
        self.queue.push(constraint);
        Ok(())
    }

    fn pop(&mut self) -> Result<Option<usize>, CompactPropagationResult> {
        let Some(constraint) = self.queue.pop() else {
            return Ok(None);
        };
        let Some(queued) = self.queued.get_mut(constraint) else {
            return Err(CompactPropagationResult::InvalidConstraint);
        };
        *queued = 0;
        Ok(Some(constraint))
    }

    fn incidence_count(&self, variable: usize) -> Option<usize> {
        if self.offsets.is_empty() {
            return Some(0);
        }
        self.offsets
            .get(variable)
            .copied()
            .zip(self.offsets.get(variable.checked_add(1)?).copied())
            .and_then(|(start, end)| end.checked_sub(start))
    }
}

impl CompactTransitiveClosure {
    fn try_new<F>(
        variable_count: usize,
        constraints: &ConstraintSet,
        control: &mut F,
    ) -> Result<Self, CompactPropagationResult>
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        let families = constraints.transitivity.families();
        if families.is_empty() {
            return Ok(Self {
                family_offsets: Vec::new(),
                variable_offsets: Vec::new(),
                incidences: Vec::new(),
                reachability: Vec::new(),
                fixed_queue: Vec::new(),
                word_trail: Vec::new(),
            });
        }
        let Some((incidence_count, reachability_words, word_trail_records)) =
            compact_transitive_closure_shape(constraints)
        else {
            return Err(CompactPropagationResult::InvalidConstraint);
        };

        let mut cursors = Vec::<usize>::new();
        let mut family_offsets = Vec::<usize>::new();
        let mut variable_offsets = Vec::<usize>::new();
        let mut incidences = Vec::<CompactPairIncidence>::new();
        let mut reachability = Vec::<usize>::new();
        let mut fixed_queue = Vec::<usize>::new();
        let mut word_trail = Vec::<CompactReachabilityWordChange>::new();
        if cursors.try_reserve_exact(variable_count).is_err()
            || family_offsets
                .try_reserve_exact(families.len().saturating_add(1))
                .is_err()
            || variable_offsets
                .try_reserve_exact(variable_count.saturating_add(1))
                .is_err()
            || incidences.try_reserve_exact(incidence_count).is_err()
            || reachability.try_reserve_exact(reachability_words).is_err()
            || fixed_queue.try_reserve_exact(variable_count).is_err()
            || word_trail.try_reserve_exact(word_trail_records).is_err()
        {
            return Err(CompactPropagationResult::WorkingMemoryLimit);
        }

        let mut pending = 0_usize;
        for _ in 0..variable_count {
            cursors.push(0);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        family_offsets.push(0);
        for family in families {
            let ply = family.covering_faces.len();
            let Some(family_words) = compact_reachability_word_count(ply) else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            let Some(next_offset) = family_offsets
                .last()
                .copied()
                .and_then(|offset| offset.checked_add(family_words))
            else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            family_offsets.push(next_offset);
            for variable in &family.pair_variables {
                let Some(count) = cursors.get_mut(*variable) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                let Some(next) = count.checked_add(1) else {
                    return Err(CompactPropagationResult::InvalidConstraint);
                };
                *count = next;
                poll_after_record_batch(control, 0, &mut pending)
                    .map_err(compact_propagation_abort)?;
            }
        }
        if family_offsets.last().copied() != Some(reachability_words) {
            return Err(CompactPropagationResult::InvalidConstraint);
        }

        variable_offsets.push(0);
        for count in &cursors {
            let Some(next) = variable_offsets
                .last()
                .copied()
                .and_then(|offset| offset.checked_add(*count))
            else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            variable_offsets.push(next);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        if variable_offsets.last().copied() != Some(incidence_count) {
            return Err(CompactPropagationResult::InvalidConstraint);
        }
        for _ in 0..incidence_count {
            incidences.push(CompactPairIncidence {
                family: usize::MAX,
                first: usize::MAX,
                second: usize::MAX,
            });
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        for (variable, cursor) in cursors.iter_mut().enumerate() {
            *cursor = variable_offsets[variable];
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        for (family_index, family) in families.iter().enumerate() {
            let ply = family.covering_faces.len();
            for first in 0..ply {
                for second in first + 1..ply {
                    let Some(variable) = family.pair_variable(first, second) else {
                        return Err(CompactPropagationResult::InvalidConstraint);
                    };
                    let Some(cursor) = cursors.get_mut(variable) else {
                        return Err(CompactPropagationResult::InvalidConstraint);
                    };
                    let Some(slot) = incidences.get_mut(*cursor) else {
                        return Err(CompactPropagationResult::InvalidConstraint);
                    };
                    *slot = CompactPairIncidence {
                        family: family_index,
                        first,
                        second,
                    };
                    let Some(next) = cursor.checked_add(1) else {
                        return Err(CompactPropagationResult::InvalidConstraint);
                    };
                    *cursor = next;
                    poll_after_record_batch(control, 0, &mut pending)
                        .map_err(compact_propagation_abort)?;
                }
            }
        }
        for _ in 0..reachability_words {
            reachability.push(0);
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        if pending != 0 {
            poll_control(control, ConstraintSolverEvent::PropagationBatch, 0)
                .map_err(compact_propagation_abort)?;
        }
        Ok(Self {
            family_offsets,
            variable_offsets,
            incidences,
            reachability,
            fixed_queue,
            word_trail,
        })
    }

    fn enqueue_initial_domains<F>(
        &mut self,
        candidate: &[u8],
        control: &mut F,
    ) -> Result<(), CompactPropagationResult>
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        let mut pending = 0_usize;
        for (variable, domain) in candidate.iter().copied().enumerate().rev() {
            match domain {
                DOMAIN_FALSE | DOMAIN_TRUE => self.enqueue_fixed_variable(variable)?,
                DOMAIN_BOTH => {}
                _ => return Err(CompactPropagationResult::InvalidConstraint),
            }
            poll_after_record_batch(control, 0, &mut pending).map_err(compact_propagation_abort)?;
        }
        if pending != 0 {
            poll_control(control, ConstraintSolverEvent::PropagationBatch, 0)
                .map_err(compact_propagation_abort)?;
        }
        Ok(())
    }

    fn enqueue_fixed_variable(&mut self, variable: usize) -> Result<(), CompactPropagationResult> {
        if self.variable_offsets.is_empty() {
            return Ok(());
        }
        let Some((start, end)) = self.variable_offsets.get(variable).copied().zip(
            self.variable_offsets
                .get(variable.saturating_add(1))
                .copied(),
        ) else {
            return Err(CompactPropagationResult::InvalidConstraint);
        };
        if start == end {
            return Ok(());
        }
        if self.fixed_queue.len() >= self.fixed_queue.capacity() {
            return Err(CompactPropagationResult::WorkingMemoryLimit);
        }
        self.fixed_queue.push(variable);
        Ok(())
    }

    fn word_trail_mark(&self) -> usize {
        self.word_trail.len()
    }

    fn incidence_count(&self, variable: usize) -> Option<usize> {
        if self.variable_offsets.is_empty() {
            return Some(0);
        }
        self.variable_offsets
            .get(variable)
            .copied()
            .zip(self.variable_offsets.get(variable.checked_add(1)?).copied())
            .and_then(|(start, end)| end.checked_sub(start))
    }

    fn commit_baseline(&mut self) {
        self.word_trail.clear();
    }

    fn rollback<F>(
        &mut self,
        mark: usize,
        search_nodes: usize,
        control: &mut F,
    ) -> CompactPropagationResult
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        if mark > self.word_trail.len() {
            return CompactPropagationResult::InvalidConstraint;
        }
        self.fixed_queue.clear();
        let mut pending = 0_usize;
        while self.word_trail.len() > mark {
            let Some(change) = self.word_trail.pop() else {
                return CompactPropagationResult::InvalidConstraint;
            };
            let Some(word) = self.reachability.get_mut(change.index) else {
                return CompactPropagationResult::InvalidConstraint;
            };
            *word = change.previous;
            if let Err(abort) = poll_after_record_batch(control, search_nodes, &mut pending) {
                return compact_propagation_abort(abort);
            }
        }
        if pending != 0
            && let Err(abort) = poll_control(
                control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            )
        {
            return compact_propagation_abort(abort);
        }
        CompactPropagationResult::Stable
    }

    fn propagate<F>(
        &mut self,
        candidate: &mut [u8],
        constraints: &ConstraintSet,
        search_nodes: usize,
        domain_trail: &mut Vec<(usize, u8)>,
        explicit_worklist: &mut CompactExplicitWorklist,
        control: &mut F,
    ) -> CompactPropagationResult
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        let mut pending = 0_usize;
        while let Some(variable) = self.fixed_queue.pop() {
            let Some(domain) = candidate.get(variable).copied() else {
                return CompactPropagationResult::InvalidConstraint;
            };
            if !matches!(domain, DOMAIN_FALSE | DOMAIN_TRUE) {
                return CompactPropagationResult::InvalidConstraint;
            }
            let Some((start, end)) = self.variable_offsets.get(variable).copied().zip(
                self.variable_offsets
                    .get(variable.saturating_add(1))
                    .copied(),
            ) else {
                return CompactPropagationResult::InvalidConstraint;
            };
            for position in start..end {
                let Some(incidence) = self.incidences.get(position).copied() else {
                    return CompactPropagationResult::InvalidConstraint;
                };
                match self.insert_edge(
                    incidence,
                    domain,
                    candidate,
                    constraints,
                    search_nodes,
                    domain_trail,
                    explicit_worklist,
                    control,
                    &mut pending,
                ) {
                    CompactPropagationResult::Stable => {}
                    result => return result,
                }
                if let Err(abort) = poll_after_record_batch(control, search_nodes, &mut pending) {
                    return compact_propagation_abort(abort);
                }
            }
        }
        if pending != 0
            && let Err(abort) = poll_control(
                control,
                ConstraintSolverEvent::PropagationBatch,
                search_nodes,
            )
        {
            return compact_propagation_abort(abort);
        }
        CompactPropagationResult::Stable
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_edge<F>(
        &mut self,
        incidence: CompactPairIncidence,
        domain: u8,
        candidate: &mut [u8],
        constraints: &ConstraintSet,
        search_nodes: usize,
        domain_trail: &mut Vec<(usize, u8)>,
        explicit_worklist: &mut CompactExplicitWorklist,
        control: &mut F,
        pending: &mut usize,
    ) -> CompactPropagationResult
    where
        F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
    {
        let Some(family) = constraints.transitivity.families().get(incidence.family) else {
            return CompactPropagationResult::InvalidConstraint;
        };
        let ply = family.covering_faces.len();
        let word_bits = usize::BITS as usize;
        let Some(words_per_row) = ply
            .checked_add(word_bits.saturating_sub(1))
            .and_then(|value| value.checked_div(word_bits))
        else {
            return CompactPropagationResult::InvalidConstraint;
        };
        let Some((above, below)) = (match domain {
            DOMAIN_FALSE => Some((incidence.first, incidence.second)),
            DOMAIN_TRUE => Some((incidence.second, incidence.first)),
            _ => None,
        }) else {
            return CompactPropagationResult::InvalidConstraint;
        };
        if above >= ply || below >= ply || above == below {
            return CompactPropagationResult::InvalidConstraint;
        }
        let Some(base) = self.family_offsets.get(incidence.family).copied() else {
            return CompactPropagationResult::InvalidConstraint;
        };
        let Some(end) = self
            .family_offsets
            .get(incidence.family.saturating_add(1))
            .copied()
        else {
            return CompactPropagationResult::InvalidConstraint;
        };
        if end.checked_sub(base) != ply.checked_mul(words_per_row) {
            return CompactPropagationResult::InvalidConstraint;
        }
        let Some(reverse_index) = closure_word_index(base, words_per_row, below, above) else {
            return CompactPropagationResult::InvalidConstraint;
        };
        let Some(forward_index) = closure_word_index(base, words_per_row, above, below) else {
            return CompactPropagationResult::InvalidConstraint;
        };
        let reverse_mask = 1_usize << (above % word_bits);
        let forward_mask = 1_usize << (below % word_bits);
        if self
            .reachability
            .get(reverse_index)
            .is_none_or(|word| *word & reverse_mask != 0)
        {
            return CompactPropagationResult::Conflict;
        }
        if self
            .reachability
            .get(forward_index)
            .is_some_and(|word| *word & forward_mask != 0)
        {
            return CompactPropagationResult::Stable;
        }

        let Some(successor_offset) = below
            .checked_mul(words_per_row)
            .and_then(|offset| base.checked_add(offset))
        else {
            return CompactPropagationResult::InvalidConstraint;
        };
        for predecessor in 0..ply {
            let predecessor_reaches_above = if predecessor == above {
                true
            } else {
                let Some(index) = closure_word_index(base, words_per_row, predecessor, above)
                else {
                    return CompactPropagationResult::InvalidConstraint;
                };
                self.reachability
                    .get(index)
                    .is_some_and(|word| *word & reverse_mask != 0)
            };
            if !predecessor_reaches_above {
                if let Err(abort) = poll_after_record_batch(control, search_nodes, pending) {
                    return compact_propagation_abort(abort);
                }
                continue;
            }
            let Some(target_offset) = predecessor
                .checked_mul(words_per_row)
                .and_then(|offset| base.checked_add(offset))
            else {
                return CompactPropagationResult::InvalidConstraint;
            };
            for word_position in 0..words_per_row {
                let Some(mut successors) = self
                    .reachability
                    .get(successor_offset + word_position)
                    .copied()
                else {
                    return CompactPropagationResult::InvalidConstraint;
                };
                if word_position == below / word_bits {
                    successors |= 1_usize << (below % word_bits);
                }
                let target_index = target_offset + word_position;
                let Some(previous) = self.reachability.get(target_index).copied() else {
                    return CompactPropagationResult::InvalidConstraint;
                };
                let delta = successors & !previous;
                if delta != 0 {
                    if self.word_trail.len() >= self.word_trail.capacity() {
                        return CompactPropagationResult::WorkingMemoryLimit;
                    }
                    self.word_trail.push(CompactReachabilityWordChange {
                        index: target_index,
                        previous,
                    });
                    self.reachability[target_index] = previous | successors;

                    let mut remaining = delta;
                    while remaining != 0 {
                        let bit = remaining.trailing_zeros() as usize;
                        remaining &= remaining - 1;
                        let Some(successor) = word_position
                            .checked_mul(word_bits)
                            .and_then(|offset| offset.checked_add(bit))
                        else {
                            return CompactPropagationResult::InvalidConstraint;
                        };
                        if successor >= ply || predecessor == successor {
                            return CompactPropagationResult::Conflict;
                        }
                        let (first, second, expected) = if predecessor < successor {
                            (predecessor, successor, DOMAIN_FALSE)
                        } else {
                            (successor, predecessor, DOMAIN_TRUE)
                        };
                        let Some(variable) = family.pair_variable(first, second) else {
                            return CompactPropagationResult::InvalidConstraint;
                        };
                        let Some(current) = candidate.get(variable).copied() else {
                            return CompactPropagationResult::InvalidConstraint;
                        };
                        match current {
                            DOMAIN_BOTH => {
                                if domain_trail.len() >= candidate.len() {
                                    return CompactPropagationResult::WorkingMemoryLimit;
                                }
                                domain_trail.push((variable, DOMAIN_BOTH));
                                candidate[variable] = expected;
                                if let Err(result) = explicit_worklist.enqueue_variable(
                                    variable,
                                    search_nodes,
                                    control,
                                ) {
                                    return result;
                                }
                                if let Err(result) = self.enqueue_fixed_variable(variable) {
                                    return result;
                                }
                            }
                            fixed if fixed == expected => {}
                            DOMAIN_FALSE | DOMAIN_TRUE => {
                                return CompactPropagationResult::Conflict;
                            }
                            _ => return CompactPropagationResult::InvalidConstraint,
                        }
                        if let Err(abort) = poll_after_record_batch(control, search_nodes, pending)
                        {
                            return compact_propagation_abort(abort);
                        }
                    }
                }
                if let Err(abort) = poll_after_record_batch(control, search_nodes, pending) {
                    return compact_propagation_abort(abort);
                }
            }
        }
        CompactPropagationResult::Stable
    }
}

pub(super) fn try_compact_completion<F>(
    domains: &[u8],
    constraints: &ConstraintSet,
    max_search_nodes: usize,
    control: &mut F,
) -> CompactCompletionResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    if let Err(abort) = poll_control(control, ConstraintSolverEvent::PropagationBatch, 0) {
        return compact_completion_abort(abort, 0);
    }
    let mut candidate = Vec::new();
    if candidate.try_reserve_exact(domains.len()).is_err() {
        return CompactCompletionResult::WorkingMemoryLimit;
    }
    let mut copy_pending = 0_usize;
    for domain in domains {
        candidate.push(*domain);
        if let Err(abort) = poll_after_record_batch(control, 0, &mut copy_pending) {
            return compact_completion_abort(abort, 0);
        }
    }
    if copy_pending != 0
        && let Err(abort) = poll_control(control, ConstraintSolverEvent::PropagationBatch, 0)
    {
        return compact_completion_abort(abort, 0);
    }

    let maximum_ply = constraints.transitivity.maximum_ply();
    let mut order_scratch = CompactOrderScratch {
        indegrees: Vec::new(),
        ranks: Vec::new(),
        selected: Vec::new(),
    };
    let mut trail = Vec::<(usize, u8)>::new();
    let mut stack = Vec::<CompactSearchFrame>::new();
    if order_scratch
        .indegrees
        .try_reserve_exact(maximum_ply)
        .is_err()
        || order_scratch.ranks.try_reserve_exact(maximum_ply).is_err()
        || order_scratch
            .selected
            .try_reserve_exact(maximum_ply)
            .is_err()
        || trail.try_reserve_exact(domains.len()).is_err()
        || stack.try_reserve_exact(domains.len()).is_err()
    {
        return CompactCompletionResult::WorkingMemoryLimit;
    }
    let mut explicit_worklist =
        match CompactExplicitWorklist::try_new(domains.len(), constraints, control) {
            Ok(worklist) => worklist,
            Err(CompactPropagationResult::DeadlineReached) => {
                return CompactCompletionResult::DeadlineReached { search_nodes: 0 };
            }
            Err(CompactPropagationResult::Cancelled) => {
                return CompactCompletionResult::Cancelled;
            }
            Err(CompactPropagationResult::WorkingMemoryLimit) => {
                return CompactCompletionResult::WorkingMemoryLimit;
            }
            Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
                return CompactCompletionResult::Fallback { search_nodes: 0 };
            }
            Err(CompactPropagationResult::InvalidConstraint) => {
                return CompactCompletionResult::InvalidConstraint;
            }
        };
    let mut closure = match CompactTransitiveClosure::try_new(domains.len(), constraints, control) {
        Ok(closure) => closure,
        Err(CompactPropagationResult::DeadlineReached) => {
            return CompactCompletionResult::DeadlineReached { search_nodes: 0 };
        }
        Err(CompactPropagationResult::Cancelled) => {
            return CompactCompletionResult::Cancelled;
        }
        Err(CompactPropagationResult::WorkingMemoryLimit) => {
            return CompactCompletionResult::WorkingMemoryLimit;
        }
        Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
            return CompactCompletionResult::Fallback { search_nodes: 0 };
        }
        Err(CompactPropagationResult::InvalidConstraint) => {
            return CompactCompletionResult::InvalidConstraint;
        }
    };
    match explicit_worklist.enqueue_all(constraints, 0, control) {
        Ok(()) => {}
        Err(CompactPropagationResult::DeadlineReached) => {
            return CompactCompletionResult::DeadlineReached { search_nodes: 0 };
        }
        Err(CompactPropagationResult::Cancelled) => {
            return CompactCompletionResult::Cancelled;
        }
        Err(CompactPropagationResult::WorkingMemoryLimit) => {
            return CompactCompletionResult::WorkingMemoryLimit;
        }
        Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
            return CompactCompletionResult::Fallback { search_nodes: 0 };
        }
        Err(CompactPropagationResult::InvalidConstraint) => {
            return CompactCompletionResult::InvalidConstraint;
        }
    }
    match closure.enqueue_initial_domains(&candidate, control) {
        Ok(()) => {}
        Err(CompactPropagationResult::DeadlineReached) => {
            return CompactCompletionResult::DeadlineReached { search_nodes: 0 };
        }
        Err(CompactPropagationResult::Cancelled) => {
            return CompactCompletionResult::Cancelled;
        }
        Err(CompactPropagationResult::WorkingMemoryLimit) => {
            return CompactCompletionResult::WorkingMemoryLimit;
        }
        Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
            return CompactCompletionResult::Fallback { search_nodes: 0 };
        }
        Err(CompactPropagationResult::InvalidConstraint) => {
            return CompactCompletionResult::InvalidConstraint;
        }
    }

    match propagate_for_compact_completion(
        &mut candidate,
        constraints,
        0,
        &mut trail,
        &mut closure,
        &mut explicit_worklist,
        control,
    ) {
        CompactPropagationResult::Stable => {
            trail.clear();
            closure.commit_baseline();
        }
        CompactPropagationResult::Conflict => {
            return CompactCompletionResult::Fallback { search_nodes: 0 };
        }
        CompactPropagationResult::DeadlineReached => {
            return CompactCompletionResult::DeadlineReached { search_nodes: 0 };
        }
        CompactPropagationResult::Cancelled => return CompactCompletionResult::Cancelled,
        CompactPropagationResult::WorkingMemoryLimit => {
            return CompactCompletionResult::WorkingMemoryLimit;
        }
        CompactPropagationResult::InvalidConstraint => {
            return CompactCompletionResult::InvalidConstraint;
        }
    }

    let mut search_nodes = 0_usize;
    'evaluate_candidate: loop {
        let completion_mark = trail.len();
        let order_result = match complete_compact_candidate_orders(
            &mut candidate,
            constraints,
            &mut order_scratch,
            &mut trail,
            search_nodes,
            control,
        ) {
            Ok(result) => result,
            Err(abort) => return compact_completion_abort(abort, search_nodes),
        };

        let mut rejected_explicit = None;
        match order_result {
            CompactOrderResult::Completed => {
                match compact_candidate_check(
                    &candidate,
                    constraints,
                    &mut order_scratch,
                    search_nodes,
                    control,
                ) {
                    Ok(CompactCandidateCheck::Accepts) => {
                        return CompactCompletionResult::Satisfied {
                            candidate,
                            search_nodes,
                        };
                    }
                    Ok(CompactCandidateCheck::ExplicitConflict(index)) => {
                        rejected_explicit = Some(index);
                    }
                    Ok(CompactCandidateCheck::CompactConflict) => {
                        undo_domains(&mut candidate, &mut trail, completion_mark);
                        return CompactCompletionResult::Fallback { search_nodes };
                    }
                    Ok(CompactCandidateCheck::InvalidConstraint) => {
                        return CompactCompletionResult::InvalidConstraint;
                    }
                    Err(abort) => return compact_completion_abort(abort, search_nodes),
                }
            }
            CompactOrderResult::BranchConflict => {}
            CompactOrderResult::Fallback => {
                undo_domains(&mut candidate, &mut trail, completion_mark);
                return CompactCompletionResult::Fallback { search_nodes };
            }
            CompactOrderResult::InvalidConstraint => {
                return CompactCompletionResult::InvalidConstraint;
            }
        }
        undo_domains(&mut candidate, &mut trail, completion_mark);

        if let Some(index) = rejected_explicit {
            let Some(constraint) = constraints.explicit.get(index) else {
                return CompactCompletionResult::InvalidConstraint;
            };
            let witness_has_unassigned = constraint
                .variables
                .iter()
                .any(|variable| candidate.get(*variable) == Some(&DOMAIN_BOTH));
            let variable = if witness_has_unassigned {
                match select_compact_branch_variable(
                    &candidate,
                    &explicit_worklist,
                    &closure,
                    search_nodes,
                    control,
                ) {
                    Ok(variable) => variable,
                    Err(CompactPropagationResult::DeadlineReached) => {
                        return CompactCompletionResult::DeadlineReached { search_nodes };
                    }
                    Err(CompactPropagationResult::Cancelled) => {
                        return CompactCompletionResult::Cancelled;
                    }
                    Err(CompactPropagationResult::WorkingMemoryLimit) => {
                        return CompactCompletionResult::WorkingMemoryLimit;
                    }
                    Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
                        return CompactCompletionResult::Fallback { search_nodes };
                    }
                    Err(CompactPropagationResult::InvalidConstraint) => {
                        return CompactCompletionResult::InvalidConstraint;
                    }
                }
            } else {
                None
            };
            if let Some(variable) = variable {
                if stack.len() >= candidate.len() {
                    return CompactCompletionResult::InvalidConstraint;
                }
                stack.push(CompactSearchFrame {
                    variable,
                    next_branch: 0,
                    rollback_mark: CompactRollbackMark {
                        domains: trail.len(),
                        closure: closure.word_trail_mark(),
                    },
                });
            }
        }

        loop {
            let Some(frame) = stack.last_mut() else {
                // The compact witness search is an optimization. Let the
                // established solver produce the global unsatisfied result.
                return CompactCompletionResult::Fallback { search_nodes };
            };
            if frame.next_branch >= 2 {
                let rollback_mark = frame.rollback_mark;
                stack.pop();
                match undo_compact_search_domains(
                    &mut candidate,
                    &mut trail,
                    rollback_mark,
                    &mut closure,
                    &mut explicit_worklist,
                    search_nodes,
                    control,
                ) {
                    CompactPropagationResult::Stable => {}
                    CompactPropagationResult::DeadlineReached => {
                        return CompactCompletionResult::DeadlineReached { search_nodes };
                    }
                    CompactPropagationResult::Cancelled => {
                        return CompactCompletionResult::Cancelled;
                    }
                    CompactPropagationResult::WorkingMemoryLimit => {
                        return CompactCompletionResult::WorkingMemoryLimit;
                    }
                    CompactPropagationResult::Conflict => {
                        return CompactCompletionResult::Fallback { search_nodes };
                    }
                    CompactPropagationResult::InvalidConstraint => {
                        return CompactCompletionResult::InvalidConstraint;
                    }
                }
                continue;
            }

            let variable = frame.variable;
            let rollback_mark = frame.rollback_mark;
            let domain = if frame.next_branch == 0 {
                DOMAIN_FALSE
            } else {
                DOMAIN_TRUE
            };
            frame.next_branch += 1;
            match undo_compact_search_domains(
                &mut candidate,
                &mut trail,
                rollback_mark,
                &mut closure,
                &mut explicit_worklist,
                search_nodes,
                control,
            ) {
                CompactPropagationResult::Stable => {}
                CompactPropagationResult::DeadlineReached => {
                    return CompactCompletionResult::DeadlineReached { search_nodes };
                }
                CompactPropagationResult::Cancelled => {
                    return CompactCompletionResult::Cancelled;
                }
                CompactPropagationResult::WorkingMemoryLimit => {
                    return CompactCompletionResult::WorkingMemoryLimit;
                }
                CompactPropagationResult::Conflict => {
                    return CompactCompletionResult::Fallback { search_nodes };
                }
                CompactPropagationResult::InvalidConstraint => {
                    return CompactCompletionResult::InvalidConstraint;
                }
            }
            if candidate.get(variable) != Some(&DOMAIN_BOTH) || trail.len() >= candidate.len() {
                return CompactCompletionResult::InvalidConstraint;
            }

            let observed = search_nodes.saturating_add(1);
            search_nodes = observed;
            match control(ConstraintSolverEvent::SearchNode, observed) {
                ConstraintSolverControl::Continue => {}
                ConstraintSolverControl::DeadlineReached => {
                    return CompactCompletionResult::DeadlineReached { search_nodes };
                }
                ConstraintSolverControl::Cancelled => return CompactCompletionResult::Cancelled,
                ConstraintSolverControl::WorkingMemoryLimit => {
                    return CompactCompletionResult::WorkingMemoryLimit;
                }
            }
            if observed > max_search_nodes {
                return CompactCompletionResult::SearchNodeLimit { observed };
            }

            trail.push((variable, DOMAIN_BOTH));
            candidate[variable] = domain;
            match explicit_worklist.enqueue_variable(variable, search_nodes, control) {
                Ok(()) => {}
                Err(CompactPropagationResult::DeadlineReached) => {
                    return CompactCompletionResult::DeadlineReached { search_nodes };
                }
                Err(CompactPropagationResult::Cancelled) => {
                    return CompactCompletionResult::Cancelled;
                }
                Err(CompactPropagationResult::WorkingMemoryLimit) => {
                    return CompactCompletionResult::WorkingMemoryLimit;
                }
                Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
                    return CompactCompletionResult::Fallback { search_nodes };
                }
                Err(CompactPropagationResult::InvalidConstraint) => {
                    return CompactCompletionResult::InvalidConstraint;
                }
            }
            match closure.enqueue_fixed_variable(variable) {
                Ok(()) => {}
                Err(CompactPropagationResult::DeadlineReached) => {
                    return CompactCompletionResult::DeadlineReached { search_nodes };
                }
                Err(CompactPropagationResult::Cancelled) => {
                    return CompactCompletionResult::Cancelled;
                }
                Err(CompactPropagationResult::WorkingMemoryLimit) => {
                    return CompactCompletionResult::WorkingMemoryLimit;
                }
                Err(CompactPropagationResult::Stable | CompactPropagationResult::Conflict) => {
                    return CompactCompletionResult::Fallback { search_nodes };
                }
                Err(CompactPropagationResult::InvalidConstraint) => {
                    return CompactCompletionResult::InvalidConstraint;
                }
            }
            match propagate_for_compact_completion(
                &mut candidate,
                constraints,
                search_nodes,
                &mut trail,
                &mut closure,
                &mut explicit_worklist,
                control,
            ) {
                CompactPropagationResult::Stable => continue 'evaluate_candidate,
                CompactPropagationResult::Conflict => {}
                CompactPropagationResult::DeadlineReached => {
                    return CompactCompletionResult::DeadlineReached { search_nodes };
                }
                CompactPropagationResult::Cancelled => {
                    return CompactCompletionResult::Cancelled;
                }
                CompactPropagationResult::WorkingMemoryLimit => {
                    return CompactCompletionResult::WorkingMemoryLimit;
                }
                CompactPropagationResult::InvalidConstraint => {
                    return CompactCompletionResult::InvalidConstraint;
                }
            }
        }
    }
}

fn select_compact_branch_variable<F>(
    candidate: &[u8],
    explicit_worklist: &CompactExplicitWorklist,
    closure: &CompactTransitiveClosure,
    search_nodes: usize,
    control: &mut F,
) -> Result<Option<usize>, CompactPropagationResult>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    // Canonical variable indices can vary with identity-derived face ordering.
    // Prefer variables that constrain the most live structure, using the index
    // only as a deterministic final tie-breaker.
    let mut best = None::<(usize, usize, usize)>;
    let mut pending = 0_usize;
    for (variable, domain) in candidate.iter().copied().enumerate() {
        if domain == DOMAIN_BOTH {
            let Some(explicit_incidence) = explicit_worklist.incidence_count(variable) else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            let Some(closure_incidence) = closure.incidence_count(variable) else {
                return Err(CompactPropagationResult::InvalidConstraint);
            };
            let replace = best.is_none_or(|(best_explicit, best_closure, best_variable)| {
                explicit_incidence > best_explicit
                    || (explicit_incidence == best_explicit
                        && (closure_incidence > best_closure
                            || (closure_incidence == best_closure && variable < best_variable)))
            });
            if replace {
                best = Some((explicit_incidence, closure_incidence, variable));
            }
        } else if !matches!(domain, DOMAIN_FALSE | DOMAIN_TRUE) {
            return Err(CompactPropagationResult::InvalidConstraint);
        }
        poll_after_record_batch(control, search_nodes, &mut pending)
            .map_err(compact_propagation_abort)?;
    }
    if pending != 0 {
        poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        )
        .map_err(compact_propagation_abort)?;
    }
    Ok(best.map(|(_, _, variable)| variable))
}

fn compact_reachability_word_count(maximum_ply: usize) -> Option<usize> {
    let word_bits = usize::BITS as usize;
    let words_per_row = maximum_ply
        .checked_add(word_bits.checked_sub(1)?)?
        .checked_div(word_bits)?;
    maximum_ply.checked_mul(words_per_row)
}

pub(super) fn compact_transitive_closure_shape(
    constraints: &ConstraintSet,
) -> Option<(usize, usize, usize)> {
    constraints.transitivity.families().iter().try_fold(
        (0_usize, 0_usize, 0_usize),
        |(incidences, words, trail_records), family| {
            let ply = family.covering_faces.len();
            Some((
                incidences.checked_add(family.pair_variables.len())?,
                words.checked_add(compact_reachability_word_count(ply)?)?,
                trail_records.checked_add(ply.checked_mul(ply)?)?,
            ))
        },
    )
}

fn closure_word_index(
    family_offset: usize,
    words_per_row: usize,
    from: usize,
    to: usize,
) -> Option<usize> {
    from.checked_mul(words_per_row)?
        .checked_add(to.checked_div(usize::BITS as usize)?)?
        .checked_add(family_offset)
}

/// Alternates materialized-table propagation with compact partial-order
/// closure. Every domain can narrow only once, so this checked fixed-point
/// bound is finite without expanding any logical transitivity triple.
fn propagate_for_compact_completion<F>(
    candidate: &mut [u8],
    constraints: &ConstraintSet,
    search_nodes: usize,
    trail: &mut Vec<(usize, u8)>,
    closure: &mut CompactTransitiveClosure,
    explicit_worklist: &mut CompactExplicitWorklist,
    control: &mut F,
) -> CompactPropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let Some(maximum_rounds) = candidate.len().checked_add(1) else {
        return CompactPropagationResult::InvalidConstraint;
    };
    for _ in 0..maximum_rounds {
        match propagate_explicit_worklist_for_compact_completion(
            candidate,
            constraints,
            search_nodes,
            trail,
            explicit_worklist,
            closure,
            control,
        ) {
            CompactPropagationResult::Stable => {}
            result => return result,
        }
        let compact_trail_mark = trail.len();
        match closure.propagate(
            candidate,
            constraints,
            search_nodes,
            trail,
            explicit_worklist,
            control,
        ) {
            CompactPropagationResult::Stable if trail.len() == compact_trail_mark => {
                return CompactPropagationResult::Stable;
            }
            CompactPropagationResult::Stable => {}
            result => return result,
        }
    }
    CompactPropagationResult::InvalidConstraint
}

#[cfg(test)]
fn propagate_compact_transitivity_reference<F>(
    candidate: &mut [u8],
    constraints: &ConstraintSet,
    search_nodes: usize,
    trail: &mut Vec<(usize, u8)>,
    reachability: &mut Vec<usize>,
    control: &mut F,
) -> Result<CompactTransitivityPropagationResult, ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let word_bits = usize::BITS as usize;
    let candidate_len = candidate.len();
    let mut changed = false;
    let mut pending = 0_usize;
    for family in constraints.transitivity.families() {
        let ply = family.covering_faces.len();
        let Some(words_per_row) = ply
            .checked_add(word_bits - 1)
            .and_then(|value| value.checked_div(word_bits))
        else {
            return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
        };
        let Some(word_count) = ply.checked_mul(words_per_row) else {
            return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
        };
        reachability.clear();
        for _ in 0..word_count {
            reachability.push(0);
            poll_after_record_batch(control, search_nodes, &mut pending)?;
        }

        for first in 0..ply {
            for second in first + 1..ply {
                let Some(variable) = family.pair_variable(first, second) else {
                    return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                };
                let (above, below) = match candidate.get(variable).copied() {
                    Some(DOMAIN_FALSE) => (first, second),
                    Some(DOMAIN_TRUE) => (second, first),
                    Some(DOMAIN_BOTH) => {
                        poll_after_record_batch(control, search_nodes, &mut pending)?;
                        continue;
                    }
                    _ => return Ok(CompactTransitivityPropagationResult::InvalidConstraint),
                };
                let Some(index) = above
                    .checked_mul(words_per_row)
                    .and_then(|offset| offset.checked_add(below / word_bits))
                else {
                    return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                };
                let Some(slot) = reachability.get_mut(index) else {
                    return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                };
                *slot |= 1_usize << (below % word_bits);
                poll_after_record_batch(control, search_nodes, &mut pending)?;
            }
        }

        for intermediate in 0..ply {
            let Some(source_offset) = intermediate.checked_mul(words_per_row) else {
                return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
            };
            let intermediate_word = intermediate / word_bits;
            let intermediate_mask = 1_usize << (intermediate % word_bits);
            for first in 0..ply {
                let Some(target_offset) = first.checked_mul(words_per_row) else {
                    return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                };
                if reachability[target_offset + intermediate_word] & intermediate_mask == 0 {
                    poll_after_record_batch(control, search_nodes, &mut pending)?;
                    continue;
                }
                for word in 0..words_per_row {
                    let source = reachability[source_offset + word];
                    reachability[target_offset + word] |= source;
                    poll_after_record_batch(control, search_nodes, &mut pending)?;
                }
            }
        }

        for face in 0..ply {
            let Some(index) = face
                .checked_mul(words_per_row)
                .and_then(|offset| offset.checked_add(face / word_bits))
            else {
                return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
            };
            if reachability[index] & (1_usize << (face % word_bits)) != 0 {
                return Ok(CompactTransitivityPropagationResult::Conflict);
            }
            poll_after_record_batch(control, search_nodes, &mut pending)?;
        }

        for first in 0..ply {
            for second in first + 1..ply {
                let Some(first_offset) = first.checked_mul(words_per_row) else {
                    return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                };
                let Some(second_offset) = second.checked_mul(words_per_row) else {
                    return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                };
                let first_above_second = reachability[first_offset + second / word_bits]
                    & (1_usize << (second % word_bits))
                    != 0;
                let second_above_first = reachability[second_offset + first / word_bits]
                    & (1_usize << (first % word_bits))
                    != 0;
                let expected = match (first_above_second, second_above_first) {
                    (true, false) => Some(DOMAIN_FALSE),
                    (false, true) => Some(DOMAIN_TRUE),
                    (false, false) => None,
                    (true, true) => return Ok(CompactTransitivityPropagationResult::Conflict),
                };
                if let Some(expected) = expected {
                    let Some(variable) = family.pair_variable(first, second) else {
                        return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                    };
                    let Some(domain) = candidate.get_mut(variable) else {
                        return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                    };
                    match *domain {
                        DOMAIN_BOTH => {
                            if trail.len() >= candidate_len {
                                return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                            }
                            trail.push((variable, DOMAIN_BOTH));
                            *domain = expected;
                            changed = true;
                        }
                        fixed if fixed == expected => {}
                        DOMAIN_FALSE | DOMAIN_TRUE => {
                            return Ok(CompactTransitivityPropagationResult::Conflict);
                        }
                        _ => {
                            return Ok(CompactTransitivityPropagationResult::InvalidConstraint);
                        }
                    }
                }
                poll_after_record_batch(control, search_nodes, &mut pending)?;
            }
        }
    }
    if pending != 0 {
        poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        )?;
    }
    Ok(if changed {
        CompactTransitivityPropagationResult::Changed
    } else {
        CompactTransitivityPropagationResult::Stable
    })
}

/// Drains the explicit constraints affected by the domains changed since the
/// previous compact-closure round. Each narrowing re-enqueues every adjacent
/// explicit table, so an empty queue is the explicit fixed point without a
/// global rescan.
fn propagate_explicit_worklist_for_compact_completion<F>(
    candidate: &mut [u8],
    constraints: &ConstraintSet,
    search_nodes: usize,
    trail: &mut Vec<(usize, u8)>,
    worklist: &mut CompactExplicitWorklist,
    closure: &mut CompactTransitiveClosure,
    control: &mut F,
) -> CompactPropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let candidate_len = candidate.len();
    let mut pending = 0_usize;
    loop {
        let constraint_index = match worklist.pop() {
            Ok(Some(index)) => index,
            Ok(None) => break,
            Err(result) => return result,
        };
        let Some(constraint) = constraints.explicit.get(constraint_index) else {
            return CompactPropagationResult::InvalidConstraint;
        };
        if tuple_constraint_is_tautology(constraint) {
            if let Err(abort) = poll_after_record_batch(control, search_nodes, &mut pending) {
                return compact_propagation_abort(abort);
            }
            continue;
        }

        let mut compatible_rows = 0_usize;
        let mut supports = [0_u8; 6];
        for row in constraint.allowed_rows.iter().copied() {
            let mut compatible = true;
            for (position, variable) in constraint.variables.iter().copied().enumerate() {
                let Some(domain) = candidate.get(variable) else {
                    return CompactPropagationResult::InvalidConstraint;
                };
                if *domain & row_domain(row, position) == 0 {
                    compatible = false;
                    break;
                }
            }
            if compatible {
                compatible_rows = compatible_rows.saturating_add(1);
                for (position, support) in supports
                    .iter_mut()
                    .enumerate()
                    .take(constraint.variables.len())
                {
                    *support |= row_domain(row, position);
                }
            }
            if let Err(abort) = poll_after_record_batch(control, search_nodes, &mut pending) {
                return compact_propagation_abort(abort);
            }
        }
        if compatible_rows == 0 {
            return CompactPropagationResult::Conflict;
        }
        if constraint.variables.len() > supports.len() {
            return CompactPropagationResult::InvalidConstraint;
        }
        for (position, variable) in constraint.variables.iter().copied().enumerate() {
            let Some(previous) = candidate.get(variable).copied() else {
                return CompactPropagationResult::InvalidConstraint;
            };
            let next = previous & supports[position];
            if next == 0 {
                return CompactPropagationResult::Conflict;
            }
            if next != previous {
                if previous != DOMAIN_BOTH || !matches!(next, DOMAIN_FALSE | DOMAIN_TRUE) {
                    return CompactPropagationResult::InvalidConstraint;
                }
                if trail.len() >= candidate_len {
                    return CompactPropagationResult::WorkingMemoryLimit;
                }
                trail.push((variable, previous));
                candidate[variable] = next;
                if let Err(result) = worklist.enqueue_variable(variable, search_nodes, control) {
                    return result;
                }
                if let Err(result) = closure.enqueue_fixed_variable(variable) {
                    return result;
                }
            }
            if let Err(abort) = poll_after_record_batch(control, search_nodes, &mut pending) {
                return compact_propagation_abort(abort);
            }
        }
    }
    if pending != 0
        && let Err(abort) = poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        )
    {
        return compact_propagation_abort(abort);
    }
    CompactPropagationResult::Stable
}

fn complete_compact_candidate_orders<F>(
    candidate: &mut [u8],
    constraints: &ConstraintSet,
    scratch: &mut CompactOrderScratch,
    trail: &mut Vec<(usize, u8)>,
    search_nodes: usize,
    control: &mut F,
) -> Result<CompactOrderResult, ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let mut pending = 0_usize;
    let candidate_len = candidate.len();
    // Check every partial family before any arbitrary completion from an
    // overlapping family is introduced. A cycle found here is a sound branch
    // conflict; a cycle found during the writing pass is only a reason to fall
    // back to the general solver.
    for family in constraints.transitivity.families() {
        match rank_compact_family(
            family,
            candidate,
            scratch,
            None,
            search_nodes,
            &mut pending,
            control,
        )? {
            CompactFamilyOrderResult::Completed => {}
            CompactFamilyOrderResult::FixedCycle => {
                return Ok(CompactOrderResult::BranchConflict);
            }
            CompactFamilyOrderResult::InvalidConstraint => {
                return Ok(CompactOrderResult::InvalidConstraint);
            }
        }
    }
    for family in constraints.transitivity.families() {
        match rank_compact_family(
            family,
            candidate,
            scratch,
            Some(trail),
            search_nodes,
            &mut pending,
            control,
        )? {
            CompactFamilyOrderResult::Completed => {}
            CompactFamilyOrderResult::FixedCycle => return Ok(CompactOrderResult::Fallback),
            CompactFamilyOrderResult::InvalidConstraint => {
                return Ok(CompactOrderResult::InvalidConstraint);
            }
        }
    }
    for (variable, domain) in candidate.iter_mut().enumerate() {
        match *domain {
            DOMAIN_BOTH => {
                if trail.len() >= candidate_len {
                    return Ok(CompactOrderResult::InvalidConstraint);
                }
                trail.push((variable, *domain));
                *domain = DOMAIN_FALSE;
            }
            DOMAIN_FALSE | DOMAIN_TRUE => {}
            _ => return Ok(CompactOrderResult::InvalidConstraint),
        }
        poll_after_record_batch(control, search_nodes, &mut pending)?;
    }
    if pending != 0 {
        poll_control(
            control,
            ConstraintSolverEvent::PropagationBatch,
            search_nodes,
        )?;
    }
    Ok(CompactOrderResult::Completed)
}

fn reset_compact_scratch<T, F>(
    values: &mut Vec<T>,
    len: usize,
    value: T,
    search_nodes: usize,
    pending: &mut usize,
    control: &mut F,
) -> Result<(), ConstraintSolverControl>
where
    T: Copy,
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    if len > values.capacity() {
        return Err(ConstraintSolverControl::WorkingMemoryLimit);
    }
    values.clear();
    for _ in 0..len {
        values.push(value);
        poll_after_record_batch(control, search_nodes, pending)?;
    }
    Ok(())
}

fn rank_compact_family<F>(
    family: &TransitivityConstraintFamily,
    candidate: &mut [u8],
    scratch: &mut CompactOrderScratch,
    mut trail: Option<&mut Vec<(usize, u8)>>,
    search_nodes: usize,
    pending: &mut usize,
    control: &mut F,
) -> Result<CompactFamilyOrderResult, ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let ply = family.covering_faces.len();
    reset_compact_scratch(
        &mut scratch.indegrees,
        ply,
        0,
        search_nodes,
        pending,
        control,
    )?;
    reset_compact_scratch(
        &mut scratch.ranks,
        ply,
        usize::MAX,
        search_nodes,
        pending,
        control,
    )?;
    reset_compact_scratch(
        &mut scratch.selected,
        ply,
        0,
        search_nodes,
        pending,
        control,
    )?;

    for first in 0..ply {
        for second in first + 1..ply {
            let Some(variable) = family.pair_variable(first, second) else {
                return Ok(CompactFamilyOrderResult::InvalidConstraint);
            };
            match candidate.get(variable).copied() {
                Some(DOMAIN_FALSE) => {
                    let Some(next) = scratch.indegrees[second].checked_add(1) else {
                        return Ok(CompactFamilyOrderResult::InvalidConstraint);
                    };
                    scratch.indegrees[second] = next;
                }
                Some(DOMAIN_TRUE) => {
                    let Some(next) = scratch.indegrees[first].checked_add(1) else {
                        return Ok(CompactFamilyOrderResult::InvalidConstraint);
                    };
                    scratch.indegrees[first] = next;
                }
                Some(DOMAIN_BOTH) => {}
                _ => return Ok(CompactFamilyOrderResult::InvalidConstraint),
            }
            poll_after_record_batch(control, search_nodes, pending)?;
        }
    }

    for rank in 0..ply {
        let Some(next) =
            (0..ply).find(|index| scratch.selected[*index] == 0 && scratch.indegrees[*index] == 0)
        else {
            return Ok(CompactFamilyOrderResult::FixedCycle);
        };
        scratch.selected[next] = 1;
        scratch.ranks[next] = rank;
        for other in 0..ply {
            if scratch.selected[other] != 0 || other == next {
                continue;
            }
            let (first, second) = if next < other {
                (next, other)
            } else {
                (other, next)
            };
            let Some(variable) = family.pair_variable(first, second) else {
                return Ok(CompactFamilyOrderResult::InvalidConstraint);
            };
            let next_is_above = if next < other {
                candidate[variable] == DOMAIN_FALSE
            } else {
                candidate[variable] == DOMAIN_TRUE
            };
            if next_is_above {
                let Some(next_indegree) = scratch.indegrees[other].checked_sub(1) else {
                    return Ok(CompactFamilyOrderResult::InvalidConstraint);
                };
                scratch.indegrees[other] = next_indegree;
            }
            poll_after_record_batch(control, search_nodes, pending)?;
        }
    }

    if let Some(changes) = &mut trail {
        for first in 0..ply {
            for second in first + 1..ply {
                let Some(variable) = family.pair_variable(first, second) else {
                    return Ok(CompactFamilyOrderResult::InvalidConstraint);
                };
                let expected = if scratch.ranks[second] < scratch.ranks[first] {
                    DOMAIN_TRUE
                } else {
                    DOMAIN_FALSE
                };
                match candidate[variable] {
                    DOMAIN_BOTH => {
                        if changes.len() >= candidate.len() {
                            return Ok(CompactFamilyOrderResult::InvalidConstraint);
                        }
                        changes.push((variable, DOMAIN_BOTH));
                        candidate[variable] = expected;
                    }
                    fixed if fixed == expected => {}
                    DOMAIN_FALSE | DOMAIN_TRUE => {
                        return Ok(CompactFamilyOrderResult::FixedCycle);
                    }
                    _ => return Ok(CompactFamilyOrderResult::InvalidConstraint),
                }
                poll_after_record_batch(control, search_nodes, pending)?;
            }
        }
    }
    Ok(CompactFamilyOrderResult::Completed)
}

fn compact_candidate_check<F>(
    candidate: &[u8],
    constraints: &ConstraintSet,
    scratch: &mut CompactOrderScratch,
    search_nodes: usize,
    control: &mut F,
) -> Result<CompactCandidateCheck, ConstraintSolverControl>
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    let mut pending = 0_usize;
    for (index, constraint) in constraints.explicit.iter().enumerate() {
        if !tuple_constraint_is_tautology(constraint)
            && !domain_constraint_accepts(constraint, candidate)
        {
            return Ok(CompactCandidateCheck::ExplicitConflict(index));
        }
        poll_after_record_batch(control, search_nodes, &mut pending)?;
    }

    for family in constraints.transitivity.families() {
        let ply = family.covering_faces.len();
        reset_compact_scratch(
            &mut scratch.indegrees,
            ply,
            0,
            search_nodes,
            &mut pending,
            control,
        )?;
        reset_compact_scratch(
            &mut scratch.selected,
            ply,
            0,
            search_nodes,
            &mut pending,
            control,
        )?;
        for first in 0..ply {
            for second in first + 1..ply {
                let Some(variable) = family.pair_variable(first, second) else {
                    return Ok(CompactCandidateCheck::InvalidConstraint);
                };
                match candidate.get(variable).copied() {
                    Some(DOMAIN_FALSE) => scratch.indegrees[first] += 1,
                    Some(DOMAIN_TRUE) => scratch.indegrees[second] += 1,
                    _ => return Ok(CompactCandidateCheck::InvalidConstraint),
                }
                poll_after_record_batch(control, search_nodes, &mut pending)?;
            }
        }
        for &count in &scratch.indegrees {
            if count >= ply || scratch.selected[count] != 0 {
                return Ok(CompactCandidateCheck::CompactConflict);
            }
            scratch.selected[count] = 1;
        }
    }
    poll_logical_records(
        constraints.transitivity.len(),
        ConstraintSolverEvent::VerifyingConstraint,
        search_nodes,
        control,
    )?;
    Ok(CompactCandidateCheck::Accepts)
}

fn domain_constraint_accepts(constraint: &TupleConstraint, domains: &[u8]) -> bool {
    let mut row = 0_u8;
    for (position, variable) in constraint.variables.iter().enumerate() {
        match domains.get(*variable).copied() {
            Some(DOMAIN_FALSE) => {}
            Some(DOMAIN_TRUE) => row |= 1 << position,
            _ => return false,
        }
    }
    constraint.allowed_rows.contains(&row)
}

const fn compact_completion_abort(
    abort: ConstraintSolverControl,
    search_nodes: usize,
) -> CompactCompletionResult {
    match abort {
        ConstraintSolverControl::Continue => CompactCompletionResult::InvalidConstraint,
        ConstraintSolverControl::DeadlineReached => {
            CompactCompletionResult::DeadlineReached { search_nodes }
        }
        ConstraintSolverControl::Cancelled => CompactCompletionResult::Cancelled,
        ConstraintSolverControl::WorkingMemoryLimit => CompactCompletionResult::WorkingMemoryLimit,
    }
}

const fn compact_propagation_abort(abort: ConstraintSolverControl) -> CompactPropagationResult {
    match abort {
        ConstraintSolverControl::Continue => CompactPropagationResult::InvalidConstraint,
        ConstraintSolverControl::DeadlineReached => CompactPropagationResult::DeadlineReached,
        ConstraintSolverControl::Cancelled => CompactPropagationResult::Cancelled,
        ConstraintSolverControl::WorkingMemoryLimit => CompactPropagationResult::WorkingMemoryLimit,
    }
}

fn undo_compact_search_domains<F>(
    domains: &mut [u8],
    trail: &mut Vec<(usize, u8)>,
    rollback_mark: CompactRollbackMark,
    closure: &mut CompactTransitiveClosure,
    worklist: &mut CompactExplicitWorklist,
    search_nodes: usize,
    control: &mut F,
) -> CompactPropagationResult
where
    F: FnMut(ConstraintSolverEvent, usize) -> ConstraintSolverControl,
{
    if rollback_mark.domains > trail.len() {
        return CompactPropagationResult::InvalidConstraint;
    }
    match closure.rollback(rollback_mark.closure, search_nodes, control) {
        CompactPropagationResult::Stable => {}
        result => return result,
    }
    while trail.len() > rollback_mark.domains {
        let Some((variable, previous)) = trail.pop() else {
            return CompactPropagationResult::InvalidConstraint;
        };
        let Some(domain) = domains.get_mut(variable) else {
            return CompactPropagationResult::InvalidConstraint;
        };
        *domain = previous;
        if let Err(result) = worklist.enqueue_variable(variable, search_nodes, control) {
            return result;
        }
    }
    CompactPropagationResult::Stable
}

#[cfg(test)]
mod tests;
