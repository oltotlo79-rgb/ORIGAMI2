//! Process-local synchronization boundary for the persistent pair cache.
//!
//! The runtime epoch is advanced before an edit can mutate semantic project
//! state.  A cold proof therefore publishes only when the exact capture it
//! started from is still current.  Project and pose locks live above this
//! crate; no method in this module acquires either of them.

use std::sync::{Arc, Mutex, MutexGuard};

use ori_domain::{EdgeId, FaceId, ProjectId, VertexId};
use thiserror::Error;

use super::{
    AppliedEditImpactSetV1, ExactFacePoseCacheWitnessV1, FaceDependencyFootprintV1,
    PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1, PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1,
    PairProofCacheCandidateV1, PersistentPairProofCacheV1, ProofCacheBatchLookupV1,
    ProofCacheCertificateModelV1, ProofCacheErrorV1, ProofCacheLimitsV1,
    ProofCacheOperationControlV1, ProofCachePairWorkLimitsV1, ProofCachePairWorkV1,
    ProofCachePublishReportV1, ProofCacheRebindContextV1, ProofCacheRebindRequestV1,
};

#[path = "proof_cache_runtime/edit_epoch.rs"]
mod edit_epoch;
#[path = "proof_cache_runtime/snapshot_validation.rs"]
mod snapshot_validation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheRuntimeBindingV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    geometry_fingerprint: [u8; 32],
    pose_generation: u64,
    paper_thickness_bits: u64,
}

impl ProofCacheRuntimeBindingV1 {
    pub fn new(
        project_instance_id: ProjectId,
        project_id: ProjectId,
        revision: u64,
        geometry_fingerprint: [u8; 32],
        pose_generation: u64,
        paper_thickness_mm: f64,
    ) -> Result<Self, ProofCacheRuntimeErrorV1> {
        if project_instance_id.canonical_bytes() == [0; 16]
            || project_id.canonical_bytes() == [0; 16]
            || geometry_fingerprint == [0; 32]
            || pose_generation == 0
            || !paper_thickness_mm.is_finite()
            || paper_thickness_mm <= 0.0
        {
            return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
        }
        Ok(Self {
            project_instance_id,
            project_id,
            revision,
            geometry_fingerprint,
            pose_generation,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn pose_generation(&self) -> u64 {
        self.pose_generation
    }

    #[must_use]
    pub const fn paper_thickness_bits(&self) -> u64 {
        self.paper_thickness_bits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheRuntimeCaptureV1 {
    epoch: u64,
    binding: ProofCacheRuntimeBindingV1,
}

impl ProofCacheRuntimeCaptureV1 {
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub const fn paper_thickness_bits(&self) -> u64 {
        self.binding.paper_thickness_bits
    }

    pub(crate) fn key_input_v1(
        &self,
        faces: [FaceId; 2],
        issuer_context: [u8; 32],
    ) -> super::ProofCacheKeyInputV1 {
        super::ProofCacheKeyInputV1 {
            project_instance_id: self.binding.project_instance_id,
            project_id: self.binding.project_id,
            revision: self.binding.revision,
            geometry_fingerprint: self.binding.geometry_fingerprint,
            pose_generation: self.binding.pose_generation,
            paper_thickness_mm: f64::from_bits(self.binding.paper_thickness_bits),
            faces,
            certificate_model: ProofCacheCertificateModelV1::TwoHingePositiveThickness,
            issuer_context,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProofCacheRuntimeErrorV1 {
    #[error("the proof-cache runtime binding is invalid")]
    InvalidBinding,
    #[error("the proof-cache runtime lock is poisoned")]
    LockPoisoned,
    #[error("the proof result is stale for the current runtime epoch")]
    StaleProof,
    #[error("a complete edit impact is still being prepared")]
    InvalidationPending,
    #[error("the proof-cache runtime epoch overflowed")]
    EpochOverflow,
    #[error(transparent)]
    Cache(#[from] ProofCacheErrorV1),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofCacheEditInvalidationOutcomeV1 {
    pub epoch: u64,
    pub differential_retention_possible: bool,
}

pub struct ProofCacheEditEpochTicketV1 {
    epoch: u64,
    inner: Arc<Mutex<ProofCacheRuntimeStateV1>>,
    armed: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProofCacheProgressV1 {
    pub epoch: u64,
    pub proven_pairs: usize,
    pub total_pairs: usize,
    pub cache_hits: usize,
    pub cold_proofs: usize,
    pub persistent_cached_pairs: usize,
    pub capacity_unproven_pairs: usize,
    pub accounted_additive_work: [usize; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
    pub accounted_maximum_work: [usize; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
}

#[derive(Clone)]
pub struct PersistentPairProofCacheRuntimeV1 {
    inner: Arc<Mutex<ProofCacheRuntimeStateV1>>,
}

/// Opaque rollback image for one desktop-owned atomic pose adoption.
///
/// The fields and runtime identity are private, and the value is neither
/// clonable nor serializable. A successful rollback consumes its state.
///
/// The rollback operation belongs to this image, not to an arbitrary runtime:
/// it restores the retained state into its retained origin mutex exactly once.
/// It deliberately bypasses ordinary epoch/binding staleness policy because
/// this is the desktop transaction's already-authenticated recovery image.
pub struct ProofCacheRuntimeRollbackSnapshotV1 {
    inner: Arc<Mutex<ProofCacheRuntimeStateV1>>,
    state: Option<ProofCacheRuntimeStateV1>,
}

struct ProofCacheRuntimeStateV1 {
    epoch: u64,
    binding: Option<ProofCacheRuntimeBindingV1>,
    pending_impact: Option<AppliedEditImpactSetV1>,
    impact_preparation_in_progress: bool,
    cache: PersistentPairProofCacheV1,
    progress: ProofCacheProgressV1,
}

impl Clone for ProofCacheRuntimeStateV1 {
    fn clone(&self) -> Self {
        Self {
            epoch: self.epoch,
            binding: self.binding.clone(),
            pending_impact: self.pending_impact.clone(),
            impact_preparation_in_progress: self.impact_preparation_in_progress,
            cache: self.cache.clone_for_runtime_rollback_v1(),
            progress: self.progress,
        }
    }
}

impl ProofCacheEditEpochTicketV1 {
    fn belongs_to_v1(&self, runtime: &PersistentPairProofCacheRuntimeV1) -> bool {
        Arc::ptr_eq(&self.inner, &runtime.inner)
    }

    fn disarm_v1(&mut self) {
        self.armed = false;
    }
}

impl Drop for ProofCacheEditEpochTicketV1 {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // A completion unwind must not strand the runtime behind a permanent
        // invalidation barrier. Recover poison only by replacing all
        // in-progress cache state with one fail-closed, internally consistent
        // image for this exact epoch.
        let (mut state, recovered_poison) = match self.inner.lock() {
            Ok(state) => (state, false),
            Err(poisoned) => (poisoned.into_inner(), true),
        };
        if state.epoch != self.epoch {
            return;
        }
        state.cache.clear_v1();
        state.pending_impact = None;
        state.impact_preparation_in_progress = false;
        state.progress = ProofCacheProgressV1 {
            epoch: state.epoch,
            ..ProofCacheProgressV1::default()
        };
        if recovered_poison {
            self.inner.clear_poison();
        }
        self.armed = false;
    }
}

impl PersistentPairProofCacheRuntimeV1 {
    pub fn new(limits: ProofCacheLimitsV1) -> Result<Self, ProofCacheRuntimeErrorV1> {
        Ok(Self {
            inner: Arc::new(Mutex::new(ProofCacheRuntimeStateV1 {
                epoch: 1,
                binding: None,
                pending_impact: None,
                impact_preparation_in_progress: false,
                cache: PersistentPairProofCacheV1::new(limits)?,
                progress: ProofCacheProgressV1 {
                    epoch: 1,
                    ..ProofCacheProgressV1::default()
                },
            })),
        })
    }

    pub fn capture_rollback_snapshot_v1(
        &self,
    ) -> Result<ProofCacheRuntimeRollbackSnapshotV1, ProofCacheRuntimeErrorV1> {
        let state = self.lock_v1()?.clone();
        Ok(ProofCacheRuntimeRollbackSnapshotV1 {
            inner: Arc::clone(&self.inner),
            state: Some(state),
        })
    }
}

impl ProofCacheRuntimeRollbackSnapshotV1 {
    /// Restores this exact image into its private originating runtime.
    ///
    /// This is rollback-only and finite: the sole ordinary error is an
    /// already-consumed image. Epoch and binding comparisons intentionally do
    /// not participate, because they are normal-operation staleness checks
    /// and cannot make an exact transaction recovery safer.
    pub fn restore_origin_exact_for_rollback_v1(&mut self) -> Result<(), ProofCacheRuntimeErrorV1> {
        let before = self
            .state
            .take()
            .ok_or(ProofCacheRuntimeErrorV1::InvalidBinding)?;
        // Rollback owns an exact runtime image and is the recovery path for a
        // panic while the normal cache lock was held. Recovering poison here
        // is safe because the captured state replaces the whole protected
        // value before the lock is exposed again.
        let (mut state, recovered_poison) = match self.inner.lock() {
            Ok(state) => (state, false),
            Err(poisoned) => {
                let state = poisoned.into_inner();
                (state, true)
            }
        };
        *state = before;
        if recovered_poison {
            self.inner.clear_poison();
        }
        Ok(())
    }
}

impl PersistentPairProofCacheRuntimeV1 {
    #[cfg(test)]
    pub(crate) fn poison_rollback_lock_for_test_v1(&self) {
        let _state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        panic!("injected proof-cache rollback lock poison");
    }

    /// Captures the exact runtime epoch after the caller has acquired project
    /// then pose authority.  A revision transition with a complete pending
    /// impact is finalized lazily by the first exact endpoint session.
    pub fn capture_v1(
        &self,
        binding: ProofCacheRuntimeBindingV1,
    ) -> Result<ProofCacheRuntimeCaptureV1, ProofCacheRuntimeErrorV1> {
        let mut state = self.lock_v1()?;
        if state.impact_preparation_in_progress {
            return Err(ProofCacheRuntimeErrorV1::InvalidationPending);
        }
        let compatible_pending = state.pending_impact.as_ref().is_some_and(|impact| {
            state.binding.as_ref().is_some_and(|current| {
                current.project_instance_id == binding.project_instance_id
                    && current.project_id == binding.project_id
                    && current.revision == impact.source_revision
                    && binding.revision == impact.target_revision
                    && current.paper_thickness_bits == binding.paper_thickness_bits
            })
        });
        if state.binding.as_ref().is_none() {
            state.binding = Some(binding.clone());
        } else if state.binding.as_ref() != Some(&binding) && !compatible_pending {
            Self::advance_epoch_locked_v1(&mut state)?;
            state.cache.clear_v1();
            state.pending_impact = None;
            state.binding = Some(binding.clone());
            state.progress = ProofCacheProgressV1 {
                epoch: state.epoch,
                ..ProofCacheProgressV1::default()
            };
        }
        Ok(ProofCacheRuntimeCaptureV1 {
            epoch: state.epoch,
            binding,
        })
    }

    pub fn progress_v1(&self) -> Result<ProofCacheProgressV1, ProofCacheRuntimeErrorV1> {
        Ok(self.lock_v1()?.progress)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish_two_hinge_positive_v1(
        &self,
        capture: &ProofCacheRuntimeCaptureV1,
        issuer_context: [u8; 32],
        candidates: Vec<PairProofCacheCandidateV1>,
        proven_pairs: usize,
        total_pairs: usize,
        cache_hits: usize,
        accounted_work: &ProofCachePairWorkV1,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCachePublishReportV1, ProofCacheRuntimeErrorV1> {
        let mut state = self.lock_v1()?;
        Self::validate_capture_locked_v1(&state, capture)?;
        if state.binding.as_ref() != Some(&capture.binding) || state.pending_impact.is_some() {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        let candidate_keys = candidates
            .iter()
            .map(|candidate| candidate.key.clone())
            .collect::<Vec<_>>();
        Self::validate_model4_keys_v1(capture, issuer_context, &candidate_keys)?;
        let cold_proofs = candidates.len();
        Self::validate_publication_progress_v1(cold_proofs, proven_pairs, total_pairs, cache_hits)?;
        let report = state.cache.publish_batch_v1(candidates, control)?;
        let persistent_cached_pairs = cache_hits
            .checked_add(report.admitted_entries)
            .and_then(|value| value.checked_add(report.already_present_entries))
            .ok_or(ProofCacheRuntimeErrorV1::Cache(
                ProofCacheErrorV1::ResourceLimitExceeded,
            ))?;
        state.progress = ProofCacheProgressV1 {
            epoch: state.epoch,
            proven_pairs,
            total_pairs,
            cache_hits,
            cold_proofs,
            persistent_cached_pairs,
            capacity_unproven_pairs: report.unproven_due_to_capacity,
            accounted_additive_work: *accounted_work.additive_counters(),
            accounted_maximum_work: *accounted_work.maximum_counters(),
        };
        Ok(report)
    }

    fn validate_capture_locked_v1(
        state: &ProofCacheRuntimeStateV1,
        capture: &ProofCacheRuntimeCaptureV1,
    ) -> Result<(), ProofCacheRuntimeErrorV1> {
        if state.impact_preparation_in_progress {
            Err(ProofCacheRuntimeErrorV1::InvalidationPending)
        } else if state.epoch != capture.epoch {
            Err(ProofCacheRuntimeErrorV1::StaleProof)
        } else {
            Ok(())
        }
    }

    fn validate_model4_keys_v1(
        capture: &ProofCacheRuntimeCaptureV1,
        issuer_context: [u8; 32],
        keys: &[super::ProofCacheKeyV1],
    ) -> Result<(), ProofCacheRuntimeErrorV1> {
        if issuer_context == [0; 32]
            || keys.iter().any(|key| {
                key.project_instance_id != capture.binding.project_instance_id
                    || key.project_id != capture.binding.project_id
                    || key.revision != capture.binding.revision
                    || key.geometry_fingerprint != capture.binding.geometry_fingerprint
                    || key.pose_generation != capture.binding.pose_generation
                    || key.paper_thickness_bits != capture.binding.paper_thickness_bits
                    || key.certificate_model
                        != ProofCacheCertificateModelV1::TwoHingePositiveThickness
                    || key.issuer_context != issuer_context
            })
        {
            Err(ProofCacheRuntimeErrorV1::InvalidBinding)
        } else {
            Ok(())
        }
    }

    fn validate_publication_progress_v1(
        cold_proofs: usize,
        proven_pairs: usize,
        total_pairs: usize,
        cache_hits: usize,
    ) -> Result<(), ProofCacheRuntimeErrorV1> {
        if proven_pairs > total_pairs
            || cache_hits > proven_pairs
            || cold_proofs > proven_pairs - cache_hits
        {
            Err(ProofCacheRuntimeErrorV1::InvalidBinding)
        } else {
            Ok(())
        }
    }

    fn advance_epoch_locked_v1(
        state: &mut ProofCacheRuntimeStateV1,
    ) -> Result<(), ProofCacheRuntimeErrorV1> {
        state.epoch = state
            .epoch
            .checked_add(1)
            .ok_or(ProofCacheRuntimeErrorV1::EpochOverflow)?;
        Ok(())
    }

    fn lock_v1(
        &self,
    ) -> Result<MutexGuard<'_, ProofCacheRuntimeStateV1>, ProofCacheRuntimeErrorV1> {
        self.inner
            .lock()
            .map_err(|_| ProofCacheRuntimeErrorV1::LockPoisoned)
    }
}
