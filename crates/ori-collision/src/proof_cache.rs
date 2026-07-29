//! Deterministic, fail-closed storage for proven canonical face pairs.
//!
//! A cache hit is never inferred from geometry similarity. Every lookup uses
//! the complete key, and revision migration is available only through the
//! separately bounded invalidation proof in `proof_cache_invalidation.rs`.
//! Cached observations are opaque, non-serializable runtime values and grant
//! no project-mutation authority.

#[cfg(test)]
use std::cell::Cell;
use std::collections::BTreeMap;

#[path = "proof_cache_encoding.rs"]
mod encoding;

#[path = "proof_cache_evidence.rs"]
mod evidence;

#[path = "proof_cache_invalidation.rs"]
mod invalidation;

#[path = "proof_cache_key.rs"]
mod key;

#[path = "proof_cache_runtime.rs"]
mod runtime;

#[path = "proof_cache_work.rs"]
mod work;

#[cfg(test)]
use encoding::checked_collection_storage_bytes_v1;
use encoding::pair_proof_binding_v1;
pub use evidence::{
    CachedPairProofConclusionV1, CachedPairProofResultV1, ProofCacheBatchLookupV1, ProofCacheHitV1,
    ProofCachePublishReportV1,
};
pub(crate) use evidence::{
    ExactFacePoseCacheWitnessV1, ExactFacePoseComponentsV1, FaceDependencyFootprintV1,
    PairProofCacheCandidateV1, PairProofCacheEntryV1, PairProofDependenciesV1,
    ProofMemoDependencyTokenV1,
};
pub(crate) use invalidation::ProofCacheRebindRequestV1;
pub use invalidation::{ProofCacheInvalidationReportV1, ProofCacheRebindContextV1};
pub(crate) use key::AppliedEditImpactSetV1;
pub use key::{ProofCacheCertificateModelV1, ProofCacheKeyInputV1, ProofCacheKeyV1};
pub use runtime::{
    PersistentPairProofCacheRuntimeV1, ProofCacheEditEpochTicketV1,
    ProofCacheEditInvalidationOutcomeV1, ProofCacheProgressV1, ProofCacheRuntimeBindingV1,
    ProofCacheRuntimeCaptureV1, ProofCacheRuntimeErrorV1, ProofCacheRuntimeRollbackSnapshotV1,
};
pub use work::{
    ProofCacheErrorV1, ProofCacheLimitsV1, ProofCacheOperationControlV1,
    ProofCachePairWorkLimitsV1, ProofCachePairWorkV1,
};

#[cfg(test)]
#[path = "proof_cache_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "proof_cache_runtime_tests.rs"]
mod runtime_tests;

pub const MAX_PROOF_CACHE_ENTRIES_V1: usize = 65_536;
pub const MAX_PROOF_CACHE_STORAGE_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_PROOF_CACHE_INVALIDATION_WORK_V1: usize = 2_000_000;
pub const PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1: usize = 25;
pub const PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1: usize = 10;

const CANONICAL_ID_BYTES_V1: usize = 16;
const FINGERPRINT_BYTES_V1: usize = 32;
const U64_BYTES_V1: usize = 8;
const MODEL_TAG_BYTES_V1: usize = 1;

#[cfg(test)]
thread_local! {
    static PANIC_AFTER_PUBLISH_REPLACEMENT_INSERTS_V1: Cell<Option<usize>> =
        const { Cell::new(None) };
}

#[cfg(test)]
fn panic_after_publish_replacement_inserts_for_test_v1(inserted_entries: usize) {
    PANIC_AFTER_PUBLISH_REPLACEMENT_INSERTS_V1.with(|fault| {
        let should_panic = fault.get() == Some(inserted_entries);
        if should_panic {
            fault.set(None);
            panic!("injected proof-cache replacement publication panic");
        }
    });
}

#[cfg(test)]
fn arm_publish_replacement_panic_for_test_v1(inserted_entries: usize) {
    assert!(inserted_entries > 0, "the fault must follow a real insert");
    PANIC_AFTER_PUBLISH_REPLACEMENT_INSERTS_V1.with(|fault| {
        assert_eq!(
            fault.replace(Some(inserted_entries)),
            None,
            "one replacement publication fault may be armed"
        );
    });
}

pub struct PersistentPairProofCacheV1 {
    limits: ProofCacheLimitsV1,
    entries: BTreeMap<ProofCacheKeyV1, PairProofCacheEntryV1>,
    logical_storage_bytes: usize,
}

impl PersistentPairProofCacheV1 {
    pub fn new(limits: ProofCacheLimitsV1) -> Result<Self, ProofCacheErrorV1> {
        Ok(Self {
            limits: limits.validate()?,
            entries: BTreeMap::new(),
            logical_storage_bytes: 0,
        })
    }

    pub(super) fn clone_for_runtime_rollback_v1(&self) -> Self {
        Self {
            limits: self.limits,
            entries: self.entries.clone(),
            logical_storage_bytes: self.logical_storage_bytes,
        }
    }

    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub const fn logical_storage_bytes(&self) -> usize {
        self.logical_storage_bytes
    }

    fn clear_v1(&mut self) {
        self.entries.clear();
        self.logical_storage_bytes = 0;
    }

    pub fn lookup_v1(
        &self,
        key: &ProofCacheKeyV1,
        work_limits: &ProofCachePairWorkLimitsV1,
    ) -> Result<Option<ProofCacheHitV1>, ProofCacheErrorV1> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        let accounted_work =
            ProofCachePairWorkV1::default().checked_merge(&entry.work, work_limits)?;
        Ok(Some(ProofCacheHitV1 {
            key: entry.key.clone(),
            result: entry.result.clone(),
            accounted_work,
        }))
    }

    pub fn lookup_canonical_batch_v1(
        &self,
        keys: &[ProofCacheKeyV1],
        work_limits: &ProofCachePairWorkLimitsV1,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheBatchLookupV1, ProofCacheErrorV1> {
        control.checkpoint()?;
        let cache_operation_work =
            preflight_canonical_operation_v1(keys.len(), self.limits.max_invalidation_work)?;
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(keys.len())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
        for key in keys {
            control.checkpoint()?;
            canonical.push(key.clone());
        }
        canonical.sort_unstable();
        control.checkpoint()?;
        canonical.dedup();
        let mut hits = Vec::new();
        hits.try_reserve_exact(canonical.len())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
        let mut missing_entries = 0usize;
        let mut total = ProofCachePairWorkV1::default();
        for key in canonical {
            control.checkpoint()?;
            if let Some(entry) = self.entries.get(&key) {
                total = total.checked_merge(&entry.work, work_limits)?;
                hits.push(ProofCacheHitV1 {
                    key: entry.key.clone(),
                    result: entry.result.clone(),
                    accounted_work: entry.work.clone(),
                });
            } else {
                missing_entries = missing_entries
                    .checked_add(1)
                    .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            }
        }
        control.checkpoint()?;
        Ok(ProofCacheBatchLookupV1 {
            hits,
            missing_entries,
            total_accounted_work: total,
            cache_operation_work,
            runtime_operation_work: cache_operation_work,
        })
    }

    pub(crate) fn publish_batch_v1(
        &mut self,
        mut candidates: Vec<PairProofCacheCandidateV1>,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCachePublishReportV1, ProofCacheErrorV1> {
        control.checkpoint()?;
        let cache_operation_work =
            preflight_canonical_operation_v1(candidates.len(), self.limits.max_invalidation_work)?;
        for candidate in &candidates {
            control.checkpoint()?;
            candidate.reauthenticate_v1()?;
        }
        candidates.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        control.checkpoint()?;
        if candidates.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(ProofCacheErrorV1::ConflictingEvidence);
        }
        let mut logical_storage_bytes = self.logical_storage_bytes;
        let mut admitted_entries = 0usize;
        let mut already_present_entries = 0usize;
        let mut unproven_due_to_capacity = 0usize;
        let mut planned_entries = Vec::new();
        planned_entries
            .try_reserve_exact(candidates.len())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
        for candidate in candidates {
            control.checkpoint()?;
            if let Some(existing) = self.entries.get(&candidate.key) {
                if !existing.result.same_content(&candidate.result)
                    || existing.work != candidate.work
                    || existing.dependencies != candidate.dependencies
                {
                    return Err(ProofCacheErrorV1::ConflictingEvidence);
                }
                already_present_entries = already_present_entries
                    .checked_add(1)
                    .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
                continue;
            }
            let next_storage = logical_storage_bytes
                .checked_add(candidate.logical_storage_bytes)
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            let next_entry_count = self
                .entries
                .len()
                .checked_add(planned_entries.len())
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            if next_entry_count >= self.limits.max_entries
                || next_storage > self.limits.max_storage_bytes
            {
                unproven_due_to_capacity = unproven_due_to_capacity
                    .checked_add(1)
                    .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
                continue;
            }
            logical_storage_bytes = next_storage;
            admitted_entries = admitted_entries
                .checked_add(1)
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            planned_entries.push(PairProofCacheEntryV1::from(candidate));
        }
        control.checkpoint()?;
        // Build the complete replacement without touching the live map. This
        // keeps the published state unchanged if allocation or an unexpected
        // unwind interrupts construction. The final swap is the sole commit.
        let mut replacement_entries = self.entries.clone();
        #[cfg(test)]
        let mut inserted_entries = 0usize;
        for entry in planned_entries {
            let previous = replacement_entries.insert(entry.key.clone(), entry);
            debug_assert!(
                previous.is_none(),
                "planned proof-cache entries were preflighted as absent"
            );
            #[cfg(test)]
            {
                inserted_entries += 1;
                panic_after_publish_replacement_inserts_for_test_v1(inserted_entries);
            }
        }
        let total_entries = replacement_entries.len();
        self.entries = replacement_entries;
        self.logical_storage_bytes = logical_storage_bytes;
        Ok(ProofCachePublishReportV1 {
            admitted_entries,
            already_present_entries,
            unproven_due_to_capacity,
            total_entries,
            logical_storage_bytes,
            cache_operation_work,
        })
    }
}

pub(crate) fn preflight_canonical_operation_v1(
    item_count: usize,
    work_limit: usize,
) -> Result<usize, ProofCacheErrorV1> {
    if item_count > MAX_PROOF_CACHE_ENTRIES_V1 {
        return Err(ProofCacheErrorV1::ResourceLimitExceeded);
    }
    let sort_levels = if item_count <= 1 {
        0
    } else {
        usize::try_from(usize::BITS - (item_count - 1).leading_zeros())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?
    };
    // One canonical-copy/validation pass, a deterministic sort upper bound,
    // one duplicate scan and one lookup/publication pass.
    let work = sort_levels
        .checked_add(3)
        .and_then(|per_item| item_count.checked_mul(per_item))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    (work <= work_limit)
        .then_some(work)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)
}
