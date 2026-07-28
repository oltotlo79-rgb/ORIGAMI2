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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofCacheEditEpochTicketV1 {
    epoch: u64,
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

/// Opaque one-shot rollback image for one desktop-owned atomic pose adoption.
///
/// The fields and runtime identity are private, and the value is neither
/// clonable nor serializable. Restoration succeeds only on the originating
/// runtime and before more than the single expected pose-authority epoch
/// transition has occurred.
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

    pub fn restore_rollback_snapshot_v1(
        &self,
        mut snapshot: ProofCacheRuntimeRollbackSnapshotV1,
    ) -> Result<(), ProofCacheRuntimeErrorV1> {
        if !Arc::ptr_eq(&self.inner, &snapshot.inner) {
            return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
        }
        let before = snapshot
            .state
            .take()
            .ok_or(ProofCacheRuntimeErrorV1::InvalidBinding)?;
        let allowed_advanced_epoch = before.epoch.checked_add(1);
        let mut state = self.lock_v1()?;
        if state.epoch != before.epoch && Some(state.epoch) != allowed_advanced_epoch {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        *state = before;
        Ok(())
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
