//! Resource limits, cancellation and pair-local logical work accounting.

use std::{
    sync::atomic::{AtomicBool, AtomicU64},
    time::Instant,
};

use thiserror::Error;

use super::{
    MAX_PROOF_CACHE_ENTRIES_V1, MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
    MAX_PROOF_CACHE_STORAGE_BYTES_V1, PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1,
    PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1,
};
use crate::{CooperativeOperationControlV1, CooperativeOperationStopV1};

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProofCacheErrorV1 {
    #[error("the proof-cache key is invalid")]
    InvalidKey,
    #[error("the proof-cache candidate is internally inconsistent")]
    InvalidCandidate,
    #[error("the proof-cache operation exceeded a hard resource limit")]
    ResourceLimitExceeded,
    #[error("the proof-cache operation was cancelled")]
    Cancelled,
    #[error("the proof-cache absolute deadline elapsed")]
    DeadlineExceeded,
    #[error("conflicting proof evidence exists for one complete key")]
    ConflictingEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofCacheLimitsV1 {
    pub max_entries: usize,
    pub max_storage_bytes: usize,
    pub max_invalidation_work: usize,
}

impl Default for ProofCacheLimitsV1 {
    fn default() -> Self {
        Self {
            max_entries: MAX_PROOF_CACHE_ENTRIES_V1,
            max_storage_bytes: MAX_PROOF_CACHE_STORAGE_BYTES_V1,
            max_invalidation_work: MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
        }
    }
}

impl ProofCacheLimitsV1 {
    pub(super) fn validate(self) -> Result<Self, ProofCacheErrorV1> {
        (self.max_entries <= MAX_PROOF_CACHE_ENTRIES_V1
            && self.max_storage_bytes <= MAX_PROOF_CACHE_STORAGE_BYTES_V1
            && self.max_invalidation_work <= MAX_PROOF_CACHE_INVALIDATION_WORK_V1)
            .then_some(self)
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)
    }
}

#[derive(Clone, Copy)]
pub struct ProofCacheOperationControlV1<'a> {
    control: CooperativeOperationControlV1<'a>,
}

impl<'a> ProofCacheOperationControlV1<'a> {
    #[must_use]
    pub const fn new(cancellation: Option<&'a AtomicBool>, absolute_deadline: Instant) -> Self {
        Self {
            control: CooperativeOperationControlV1::new(cancellation, absolute_deadline),
        }
    }

    /// Creates a control that is cancelled when `generation` no longer
    /// equals `expected_generation`.
    ///
    /// This complements the monotonic [`AtomicBool`] source accepted by
    /// [`Self::new`]. It is intended for process-wide request generations:
    /// replacing or explicitly cancelling the request advances the counter,
    /// and every outstanding checkpoint then fails closed.
    #[must_use]
    pub const fn new_with_generation(
        cancellation: Option<&'a AtomicBool>,
        generation: &'a AtomicU64,
        expected_generation: u64,
        absolute_deadline: Instant,
    ) -> Self {
        Self {
            control: CooperativeOperationControlV1::new_with_generation(
                cancellation,
                generation,
                expected_generation,
                absolute_deadline,
            ),
        }
    }

    pub(super) fn checkpoint(&self) -> Result<(), ProofCacheErrorV1> {
        self.control.checkpoint().map_err(|stop| match stop {
            CooperativeOperationStopV1::Cancelled => ProofCacheErrorV1::Cancelled,
            CooperativeOperationStopV1::DeadlineExceeded => ProofCacheErrorV1::DeadlineExceeded,
        })
    }

    /// Cooperative checkpoint for trusted production-side preparation that
    /// feeds the same bounded cache transaction.
    pub fn check_v1(&self) -> Result<(), ProofCacheErrorV1> {
        self.checkpoint()
    }
}

#[cfg(test)]
mod operation_control_tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    use super::{ProofCacheErrorV1, ProofCacheOperationControlV1};

    #[test]
    fn exact_generation_remains_live_and_mismatch_cancels() {
        let generation = AtomicU64::new(7);
        let control = ProofCacheOperationControlV1::new_with_generation(
            None,
            &generation,
            7,
            Instant::now() + Duration::from_secs(30),
        );
        assert_eq!(control.check_v1(), Ok(()));

        generation.store(8, Ordering::Release);
        assert_eq!(control.check_v1(), Err(ProofCacheErrorV1::Cancelled));
    }

    #[test]
    fn already_replaced_generation_is_cancelled_at_first_checkpoint() {
        let generation = AtomicU64::new(12);
        let control = ProofCacheOperationControlV1::new_with_generation(
            None,
            &generation,
            11,
            Instant::now() + Duration::from_secs(30),
        );

        assert_eq!(control.check_v1(), Err(ProofCacheErrorV1::Cancelled));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCachePairWorkV1 {
    pub(super) additive: [usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
    pub(super) maximum: [usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
}

impl ProofCachePairWorkV1 {
    pub(crate) const fn from_exact_pair_counters_v1(
        additive: [usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
        maximum: [usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
    ) -> Self {
        Self { additive, maximum }
    }

    #[must_use]
    pub const fn additive_counters(&self) -> &[usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1] {
        &self.additive
    }

    #[must_use]
    pub const fn maximum_counters(&self) -> &[usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1] {
        &self.maximum
    }

    pub(crate) fn checked_merge(
        &self,
        additional: &Self,
        limits: &ProofCachePairWorkLimitsV1,
    ) -> Result<Self, ProofCacheErrorV1> {
        let mut additive = [0; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1];
        for (index, output) in additive.iter_mut().enumerate() {
            *output = self.additive[index]
                .checked_add(additional.additive[index])
                .filter(|value| *value <= limits.additive[index])
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        }
        let mut maximum = [0; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1];
        for (index, output) in maximum.iter_mut().enumerate() {
            *output = self.maximum[index].max(additional.maximum[index]);
            if *output > limits.maximum[index] {
                return Err(ProofCacheErrorV1::ResourceLimitExceeded);
            }
        }
        Ok(Self { additive, maximum })
    }
}

impl Default for ProofCachePairWorkV1 {
    fn default() -> Self {
        Self {
            additive: [0; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
            maximum: [0; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCachePairWorkLimitsV1 {
    pub(super) additive: [usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
    pub(super) maximum: [usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
}

impl ProofCachePairWorkLimitsV1 {
    #[must_use]
    pub const fn new(
        additive: [usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
        maximum: [usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
    ) -> Self {
        Self { additive, maximum }
    }

    #[must_use]
    pub const fn additive_counters(&self) -> &[usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1] {
        &self.additive
    }

    #[must_use]
    pub const fn maximum_counters(&self) -> &[usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1] {
        &self.maximum
    }
}
