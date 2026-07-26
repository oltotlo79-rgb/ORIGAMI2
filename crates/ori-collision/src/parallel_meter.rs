//! Deterministic execution support for independent canonical face-pair work.
//!
//! This module owns no proof semantics and never uses Rayon's global pool.
//! Callers must reserve their complete hard envelope before entering
//! [`execute_canonical_pairs`]. Every indexed slot is written at most once,
//! all workers are joined even when one task fails, and callers merge the
//! completed deltas later in canonical input order.

use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CanonicalPairExecutionError {
    Cancelled,
    ResourceLimitExceeded,
}

#[derive(Debug)]
enum CanonicalPairTaskResult<T> {
    Completed(T),
    Cancelled,
}

pub(super) fn execute_canonical_pairs<T, F>(
    pair_count: usize,
    worker_threads: usize,
    cancellation: Option<&AtomicBool>,
    execute: F,
) -> Result<Vec<T>, CanonicalPairExecutionError>
where
    T: Send,
    F: Fn(usize) -> T + Send + Sync,
{
    if worker_threads == 0 {
        return Err(CanonicalPairExecutionError::ResourceLimitExceeded);
    }

    let mut slots = Vec::new();
    slots
        .try_reserve_exact(pair_count)
        .map_err(|_| CanonicalPairExecutionError::ResourceLimitExceeded)?;
    slots.resize_with(pair_count, || None);
    let mut completed = Vec::new();
    completed
        .try_reserve_exact(pair_count)
        .map_err(|_| CanonicalPairExecutionError::ResourceLimitExceeded)?;

    let run = |index: usize| {
        if cancellation.is_some_and(|signal| signal.load(Ordering::Acquire)) {
            CanonicalPairTaskResult::Cancelled
        } else {
            CanonicalPairTaskResult::Completed(execute(index))
        }
    };

    if worker_threads == 1 {
        for (index, slot) in slots.iter_mut().enumerate() {
            *slot = Some(run(index));
        }
    } else {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(worker_threads)
            .thread_name(|index| format!("ori-collision-pair-{index}"))
            .build()
            .map_err(|_| CanonicalPairExecutionError::ResourceLimitExceeded)?;
        pool.install(|| {
            slots
                .par_iter_mut()
                .enumerate()
                .for_each(|(index, slot)| *slot = Some(run(index)));
        });
    }

    // Cancellation is checked again only after every dedicated-pool worker
    // has joined. A late cancellation can therefore never publish authority
    // assembled from partial or already-completed pair results.
    let cancelled = cancellation.is_some_and(|signal| signal.load(Ordering::Acquire));
    for slot in slots {
        match slot.ok_or(CanonicalPairExecutionError::ResourceLimitExceeded)? {
            CanonicalPairTaskResult::Completed(result) => completed.push(result),
            CanonicalPairTaskResult::Cancelled => {
                return Err(CanonicalPairExecutionError::Cancelled);
            }
        }
    }
    if cancelled {
        return Err(CanonicalPairExecutionError::Cancelled);
    }
    Ok(completed)
}
