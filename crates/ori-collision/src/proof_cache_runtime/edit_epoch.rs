//! Epoch transitions and complete editor-impact aggregation.

use super::*;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static PANIC_NEXT_COMPLETE_EDIT_WHILE_LOCKED_V1: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn inject_complete_edit_panic_while_locked_for_test_v1() {
    PANIC_NEXT_COMPLETE_EDIT_WHILE_LOCKED_V1.with(|fault| {
        if fault.replace(false) {
            panic!("injected complete-edit panic while the runtime lock is held");
        }
    });
}

impl PersistentPairProofCacheRuntimeV1 {
    /// Advances the epoch before semantic mutation, then prepares the trusted
    /// complete impact outside the cache lock. Repeated edits aggregate from
    /// the cache's original revision.
    pub fn begin_complete_edit_v1(
        &self,
        source_revision: u64,
        target_revision: u64,
        vertices: Vec<VertexId>,
        edges: Vec<EdgeId>,
        faces: Vec<FaceId>,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheEditInvalidationOutcomeV1, ProofCacheRuntimeErrorV1> {
        let ticket = self.begin_edit_epoch_v1()?;
        self.complete_edit_epoch_v1(
            ticket,
            source_revision,
            target_revision,
            vertices,
            edges,
            faces,
            control,
        )
    }

    /// Advances the stale-publication epoch while the caller still owns pose
    /// authority. Complete impact derivation may then run without the mutex.
    pub fn begin_edit_epoch_v1(
        &self,
    ) -> Result<ProofCacheEditEpochTicketV1, ProofCacheRuntimeErrorV1> {
        let mut state = self.lock_v1()?;
        Self::advance_epoch_locked_v1(&mut state)?;
        state.impact_preparation_in_progress = true;
        state.progress = ProofCacheProgressV1 {
            epoch: state.epoch,
            ..ProofCacheProgressV1::default()
        };
        Ok(ProofCacheEditEpochTicketV1 {
            epoch: state.epoch,
            inner: Arc::clone(&self.inner),
            armed: true,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_edit_epoch_v1(
        &self,
        ticket: ProofCacheEditEpochTicketV1,
        source_revision: u64,
        target_revision: u64,
        vertices: Vec<VertexId>,
        edges: Vec<EdgeId>,
        faces: Vec<FaceId>,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheEditInvalidationOutcomeV1, ProofCacheRuntimeErrorV1> {
        self.complete_edit_epoch_with_upstream_work_v1(
            ticket,
            source_revision,
            target_revision,
            vertices,
            edges,
            faces,
            0,
            control,
        )
    }

    /// Carries bounded upstream impact-derivation work into the eventual
    /// differential-invalidation report.
    #[allow(clippy::too_many_arguments)]
    pub fn complete_edit_epoch_with_upstream_work_v1(
        &self,
        mut ticket: ProofCacheEditEpochTicketV1,
        source_revision: u64,
        target_revision: u64,
        mut vertices: Vec<VertexId>,
        mut edges: Vec<EdgeId>,
        mut faces: Vec<FaceId>,
        upstream_preparation_work: usize,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheEditInvalidationOutcomeV1, ProofCacheRuntimeErrorV1> {
        if !ticket.belongs_to_v1(self) {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        let prior_impact = {
            let state = self.lock_v1()?;
            if state.epoch != ticket.epoch || !state.impact_preparation_in_progress {
                return Err(ProofCacheRuntimeErrorV1::StaleProof);
            }
            #[cfg(test)]
            inject_complete_edit_panic_while_locked_for_test_v1();
            state.pending_impact.clone()
        };
        let aggregate_source = prior_impact
            .as_ref()
            .filter(|impact| impact.target_revision == source_revision)
            .map_or(source_revision, |impact| impact.source_revision);
        let mut aggregate_upstream_preparation_work = upstream_preparation_work;
        if let Some(prior) = &prior_impact
            && prior.target_revision == source_revision
        {
            let Some(combined_work) =
                aggregate_upstream_preparation_work.checked_add(prior.upstream_preparation_work)
            else {
                return Err(ProofCacheRuntimeErrorV1::Cache(
                    ProofCacheErrorV1::ResourceLimitExceeded,
                ));
            };
            aggregate_upstream_preparation_work = combined_work;
            if vertices.try_reserve(prior.vertices.len()).is_err()
                || edges.try_reserve(prior.edges.len()).is_err()
                || faces.try_reserve(prior.faces.len()).is_err()
            {
                return Err(ProofCacheRuntimeErrorV1::Cache(
                    ProofCacheErrorV1::ResourceLimitExceeded,
                ));
            }
            vertices.extend_from_slice(&prior.vertices);
            edges.extend_from_slice(&prior.edges);
            faces.extend_from_slice(&prior.faces);
        }
        let prepared = AppliedEditImpactSetV1::from_complete_aggregate_with_upstream_work_v1(
            aggregate_source,
            target_revision,
            vertices,
            edges,
            faces,
            aggregate_upstream_preparation_work,
            &control,
        )
        .map_err(ProofCacheRuntimeErrorV1::Cache)?;
        let mut state = self.lock_v1()?;
        if state.epoch != ticket.epoch || !state.impact_preparation_in_progress {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        state.impact_preparation_in_progress = false;
        let differential_retention_possible = if state
            .binding
            .as_ref()
            .is_none_or(|binding| binding.revision == aggregate_source)
        {
            state.pending_impact = Some(prepared);
            true
        } else {
            state.cache.clear_v1();
            state.pending_impact = None;
            false
        };
        let epoch = state.epoch;
        ticket.disarm_v1();
        Ok(ProofCacheEditInvalidationOutcomeV1 {
            epoch,
            differential_retention_possible,
        })
    }

    /// Completes a failed/incomplete edit by explicitly emptying the cache.
    pub fn abandon_edit_epoch_v1(
        &self,
        mut ticket: ProofCacheEditEpochTicketV1,
    ) -> Result<ProofCacheEditInvalidationOutcomeV1, ProofCacheRuntimeErrorV1> {
        if !ticket.belongs_to_v1(self) {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        let mut state = self.lock_v1()?;
        if state.epoch != ticket.epoch {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        state.cache.clear_v1();
        state.pending_impact = None;
        state.impact_preparation_in_progress = false;
        state.progress = ProofCacheProgressV1 {
            epoch: state.epoch,
            ..ProofCacheProgressV1::default()
        };
        let epoch = state.epoch;
        ticket.disarm_v1();
        Ok(ProofCacheEditInvalidationOutcomeV1 {
            epoch,
            differential_retention_possible: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn panic_next_complete_edit_while_locked_for_test_v1() {
        PANIC_NEXT_COMPLETE_EDIT_WHILE_LOCKED_V1.with(|fault| {
            assert!(
                !fault.replace(true),
                "one complete-edit panic fault may be armed"
            );
        });
    }

    /// Fail-closed invalidation for pose/project replacement or incomplete
    /// impact derivation.
    pub fn invalidate_all_v1(
        &self,
    ) -> Result<ProofCacheEditInvalidationOutcomeV1, ProofCacheRuntimeErrorV1> {
        let mut state = self.lock_v1()?;
        Self::advance_epoch_locked_v1(&mut state)?;
        state.cache.clear_v1();
        state.binding = None;
        state.pending_impact = None;
        state.impact_preparation_in_progress = false;
        state.progress = ProofCacheProgressV1 {
            epoch: state.epoch,
            ..ProofCacheProgressV1::default()
        };
        Ok(ProofCacheEditInvalidationOutcomeV1 {
            epoch: state.epoch,
            differential_retention_possible: false,
        })
    }

    /// Advances the epoch for a newly adopted pose. Pending complete impact
    /// survives only for the edit's target revision.
    pub fn advance_pose_authority_v1(
        &self,
        revision: u64,
    ) -> Result<ProofCacheEditInvalidationOutcomeV1, ProofCacheRuntimeErrorV1> {
        let mut state = self.lock_v1()?;
        Self::advance_epoch_locked_v1(&mut state)?;
        let retain_pending = !state.impact_preparation_in_progress
            && state
                .pending_impact
                .as_ref()
                .is_some_and(|impact| impact.target_revision == revision);
        if !retain_pending {
            state.cache.clear_v1();
            state.binding = None;
            state.pending_impact = None;
            state.impact_preparation_in_progress = false;
        }
        state.progress = ProofCacheProgressV1 {
            epoch: state.epoch,
            ..ProofCacheProgressV1::default()
        };
        Ok(ProofCacheEditInvalidationOutcomeV1 {
            epoch: state.epoch,
            differential_retention_possible: retain_pending,
        })
    }
}
