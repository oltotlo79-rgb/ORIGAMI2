//! Cooperative cancellation and absolute-deadline control shared by bounded
//! collision operations.

use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

/// Why a bounded native operation stopped before publishing its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooperativeOperationStopV1 {
    Cancelled,
    DeadlineExceeded,
}

/// Immutable execution control for one native operation.
///
/// The boolean source is monotonic.  A generation source is an ABA guard: a
/// worker may proceed only while it observes the generation captured when it
/// started.  `None` means no absolute deadline, which preserves legacy entry
/// point behaviour.
#[derive(Clone, Copy)]
pub struct CooperativeOperationControlV1<'a> {
    cancellation: Option<&'a AtomicBool>,
    generation_cancellation: Option<(&'a AtomicU64, u64)>,
    absolute_deadline: Option<Instant>,
}

impl<'a> CooperativeOperationControlV1<'a> {
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            cancellation: None,
            generation_cancellation: None,
            absolute_deadline: None,
        }
    }

    #[must_use]
    pub const fn new(cancellation: Option<&'a AtomicBool>, absolute_deadline: Instant) -> Self {
        Self {
            cancellation,
            generation_cancellation: None,
            absolute_deadline: Some(absolute_deadline),
        }
    }

    #[must_use]
    pub const fn new_with_generation(
        cancellation: Option<&'a AtomicBool>,
        generation: &'a AtomicU64,
        expected_generation: u64,
        absolute_deadline: Instant,
    ) -> Self {
        Self {
            cancellation,
            generation_cancellation: Some((generation, expected_generation)),
            absolute_deadline: Some(absolute_deadline),
        }
    }

    /// Preserves the established proof-cache ordering: an observed explicit
    /// cancellation wins over a concurrently elapsed deadline.
    pub fn checkpoint(&self) -> Result<(), CooperativeOperationStopV1> {
        if self
            .cancellation
            .is_some_and(|signal| signal.load(Ordering::Acquire))
            || self
                .generation_cancellation
                .is_some_and(|(generation, expected)| {
                    generation.load(Ordering::Acquire) != expected
                })
        {
            Err(CooperativeOperationStopV1::Cancelled)
        } else if self
            .absolute_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            Err(CooperativeOperationStopV1::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicBool, AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    use super::{CooperativeOperationControlV1, CooperativeOperationStopV1};

    #[test]
    fn cancellation_generation_and_deadline_are_distinguished() {
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1)
            )
            .checkpoint(),
            Err(CooperativeOperationStopV1::Cancelled)
        );

        let generation = AtomicU64::new(7);
        let active = AtomicBool::new(false);
        let control = CooperativeOperationControlV1::new_with_generation(
            Some(&active),
            &generation,
            7,
            Instant::now() + Duration::from_secs(1),
        );
        assert_eq!(control.checkpoint(), Ok(()));
        generation.store(8, Ordering::Release);
        assert_eq!(
            control.checkpoint(),
            Err(CooperativeOperationStopV1::Cancelled)
        );

        let expired = CooperativeOperationControlV1::new(Some(&active), Instant::now());
        assert_eq!(
            expired.checkpoint(),
            Err(CooperativeOperationStopV1::DeadlineExceeded)
        );
    }
}
