//! Complete-revision differential invalidation for the pair proof cache.

use std::collections::BTreeSet;

use ori_domain::ProjectId;

use super::{
    AppliedEditImpactSetV1, ExactFacePoseCacheWitnessV1, FaceDependencyFootprintV1,
    PersistentPairProofCacheV1, ProofCacheErrorV1, ProofCacheOperationControlV1,
    ProofMemoDependencyTokenV1, pair_proof_binding_v1,
};

#[path = "proof_cache_invalidation/retention.rs"]
mod retention;
use retention::entry_is_retainable_v1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheRebindContextV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    geometry_fingerprint: [u8; 32],
    pose_generation: u64,
    paper_thickness_bits: u64,
    issuer_context: [u8; 32],
}

impl ProofCacheRebindContextV1 {
    /// Creates a model-neutral rebind context.
    ///
    /// Zero thickness is valid for the zero-thickness certificate models, and
    /// its IEEE-754 sign bit is retained in the binding. Production model-4
    /// callers use [`ProofCacheRuntimeBindingV1`](super::ProofCacheRuntimeBindingV1),
    /// which separately requires a strictly positive thickness.
    pub fn new(
        project_instance_id: ProjectId,
        project_id: ProjectId,
        revision: u64,
        geometry_fingerprint: [u8; 32],
        pose_generation: u64,
        paper_thickness_mm: f64,
        issuer_context: [u8; 32],
    ) -> Result<Self, ProofCacheErrorV1> {
        if project_instance_id.canonical_bytes() == [0; 16]
            || project_id.canonical_bytes() == [0; 16]
            || geometry_fingerprint == [0; 32]
            || pose_generation == 0
            || !paper_thickness_mm.is_finite()
            || paper_thickness_mm < 0.0
            || issuer_context == [0; 32]
        {
            return Err(ProofCacheErrorV1::InvalidCandidate);
        }
        Ok(Self {
            project_instance_id,
            project_id,
            revision,
            geometry_fingerprint,
            pose_generation,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            issuer_context,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProofCacheRebindRequestV1 {
    context: ProofCacheRebindContextV1,
    impact: AppliedEditImpactSetV1,
    current_footprints: Vec<FaceDependencyFootprintV1>,
    current_exact_poses: Vec<ExactFacePoseCacheWitnessV1>,
    healthy_memo_dependencies: Vec<ProofMemoDependencyTokenV1>,
}

impl ProofCacheRebindRequestV1 {
    pub(crate) fn from_complete_revision_snapshot_v1(
        context: ProofCacheRebindContextV1,
        impact: AppliedEditImpactSetV1,
        current_footprints: Vec<FaceDependencyFootprintV1>,
        current_exact_poses: Vec<ExactFacePoseCacheWitnessV1>,
        healthy_memo_dependencies: Vec<ProofMemoDependencyTokenV1>,
    ) -> Result<Self, ProofCacheErrorV1> {
        if impact.target_revision != context.revision
            || current_footprints.is_empty()
            || current_exact_poses.is_empty()
        {
            return Err(ProofCacheErrorV1::InvalidCandidate);
        }
        Ok(Self {
            context,
            impact,
            current_footprints,
            current_exact_poses,
            healthy_memo_dependencies,
        })
    }

    fn canonicalize_v1(
        &mut self,
        work: &mut usize,
        control: &ProofCacheOperationControlV1<'_>,
        work_limit: usize,
    ) -> Result<(), ProofCacheErrorV1> {
        let footprint_sort_work = sort_work_v1(self.current_footprints.len())?;
        let pose_sort_work = sort_work_v1(self.current_exact_poses.len())?;
        let memo_sort_work = sort_work_v1(self.healthy_memo_dependencies.len())?;
        let sort_work = footprint_sort_work
            .checked_add(pose_sort_work)
            .and_then(|value| value.checked_add(memo_sort_work))
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        charge_many_v1(work, sort_work, work_limit, control)?;
        control.checkpoint()?;
        self.current_footprints
            .sort_unstable_by_key(|item| item.face.canonical_bytes());
        control.checkpoint()?;
        self.current_exact_poses
            .sort_unstable_by_key(|item| item.face.canonical_bytes());
        control.checkpoint()?;
        self.healthy_memo_dependencies.sort_unstable();
        control.checkpoint()?;
        let canonicalization_work = self
            .current_footprints
            .len()
            .checked_add(self.current_exact_poses.len())
            .and_then(|value| value.checked_add(self.healthy_memo_dependencies.len()))
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        charge_many_v1(work, canonicalization_work, work_limit, control)?;
        if self
            .current_footprints
            .windows(2)
            .any(|pair| pair[0].face == pair[1].face)
            || self
                .current_exact_poses
                .windows(2)
                .any(|pair| pair[0].face == pair[1].face)
            || self
                .current_footprints
                .iter()
                .map(|item| item.face)
                .ne(self.current_exact_poses.iter().map(|item| item.face))
        {
            return Err(ProofCacheErrorV1::InvalidCandidate);
        }
        self.healthy_memo_dependencies.dedup();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofCacheInvalidationReportV1 {
    pub examined_entries: usize,
    pub retained_entries: usize,
    pub unproven_entries: usize,
    pub untouched_entries: usize,
    pub invalidation_work: usize,
    pub total_entries: usize,
    pub logical_storage_bytes: usize,
}

struct PlannedEntryRebindV1 {
    old_key: super::ProofCacheKeyV1,
    rebound: Option<(super::ProofCacheKeyV1, [u8; 32])>,
}

impl PersistentPairProofCacheV1 {
    #[cfg(test)]
    pub(crate) fn rebind_after_complete_edit_v1(
        &mut self,
        request: ProofCacheRebindRequestV1,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheInvalidationReportV1, ProofCacheErrorV1> {
        self.rebind_after_complete_edit_with_initial_work_v1(request, 0, control)
    }

    pub(crate) fn rebind_after_complete_edit_with_initial_work_v1(
        &mut self,
        mut request: ProofCacheRebindRequestV1,
        initial_work: usize,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheInvalidationReportV1, ProofCacheErrorV1> {
        control.checkpoint()?;
        let mut work = (initial_work <= self.limits.max_invalidation_work)
            .then_some(initial_work)
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        charge_many_v1(
            &mut work,
            request.impact.preparation_work,
            self.limits.max_invalidation_work,
            &control,
        )?;
        request.canonicalize_v1(&mut work, &control, self.limits.max_invalidation_work)?;

        let mut next_storage_bytes = self.logical_storage_bytes;
        let mut examined_entries = 0usize;
        let mut retained_entries = 0usize;
        let mut unproven_entries = 0usize;
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(self.entries.len())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
        let mut rebound_keys = BTreeSet::new();

        for (old_key, old_entry) in &self.entries {
            control.checkpoint()?;
            charge_v1(&mut work, self.limits.max_invalidation_work, &control)?;
            if old_key.project_instance_id != request.context.project_instance_id
                || old_key.project_id != request.context.project_id
                || old_key.revision != request.impact.source_revision
            {
                continue;
            }
            examined_entries = examined_entries
                .checked_add(1)
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            charge_v1(&mut work, self.limits.max_invalidation_work, &control)?;
            next_storage_bytes = next_storage_bytes
                .checked_sub(old_entry.logical_storage_bytes)
                .ok_or(ProofCacheErrorV1::InvalidCandidate)?;

            if !entry_is_retainable_v1(
                old_entry,
                &request,
                &mut work,
                self.limits.max_invalidation_work,
                &control,
            )? {
                unproven_entries = unproven_entries
                    .checked_add(1)
                    .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
                plans.push(PlannedEntryRebindV1 {
                    old_key: old_key.clone(),
                    rebound: None,
                });
                continue;
            }

            let mut rebound_key = old_entry.key.clone();
            rebound_key.revision = request.context.revision;
            rebound_key.geometry_fingerprint = request.context.geometry_fingerprint;
            rebound_key.pose_generation = request.context.pose_generation;
            charge_many_v1(
                &mut work,
                old_entry.logical_storage_bytes,
                self.limits.max_invalidation_work,
                &control,
            )?;
            let rebound_binding = pair_proof_binding_v1(
                &rebound_key,
                old_entry.result.conclusion,
                &old_entry.work,
                &old_entry.dependencies,
            )?;
            charge_many_v1(
                &mut work,
                ordered_lookup_work_v1(self.entries.len())?,
                self.limits.max_invalidation_work,
                &control,
            )?;
            if self.entries.contains_key(&rebound_key) {
                return Err(ProofCacheErrorV1::ConflictingEvidence);
            }
            charge_many_v1(
                &mut work,
                ordered_lookup_work_v1(rebound_keys.len())?,
                self.limits.max_invalidation_work,
                &control,
            )?;
            if !rebound_keys.insert(rebound_key.clone()) {
                return Err(ProofCacheErrorV1::ConflictingEvidence);
            }
            next_storage_bytes = next_storage_bytes
                .checked_add(old_entry.logical_storage_bytes)
                .filter(|bytes| *bytes <= self.limits.max_storage_bytes)
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            retained_entries = retained_entries
                .checked_add(1)
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
            charge_v1(&mut work, self.limits.max_invalidation_work, &control)?;
            plans.push(PlannedEntryRebindV1 {
                old_key: old_key.clone(),
                rebound: Some((rebound_key, rebound_binding)),
            });
        }
        control.checkpoint()?;
        let untouched_entries = self
            .entries
            .len()
            .checked_sub(examined_entries)
            .ok_or(ProofCacheErrorV1::InvalidCandidate)?;
        // Every fallible operation and cancellation point precedes this
        // commit. Entries are moved rather than deep-cloned, keeping the
        // transaction bounded while preserving all-or-nothing error behavior.
        for plan in plans {
            let Some(mut entry) = self.entries.remove(&plan.old_key) else {
                unreachable!("a validated cache rebind plan references a missing source key");
            };
            if let Some((rebound_key, rebound_binding)) = plan.rebound {
                entry.key = rebound_key;
                entry.result.binding = rebound_binding;
                let replaced = self.entries.insert(entry.key.clone(), entry);
                debug_assert!(replaced.is_none());
            }
        }
        self.logical_storage_bytes = next_storage_bytes;
        Ok(ProofCacheInvalidationReportV1 {
            examined_entries,
            retained_entries,
            unproven_entries,
            untouched_entries,
            invalidation_work: work,
            total_entries: self.entries.len(),
            logical_storage_bytes: self.logical_storage_bytes,
        })
    }
}

fn sort_work_v1(item_count: usize) -> Result<usize, ProofCacheErrorV1> {
    let levels = if item_count <= 1 {
        0
    } else {
        usize::try_from(usize::BITS - (item_count - 1).leading_zeros())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?
    };
    item_count
        .checked_mul(levels)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)
}

fn ordered_lookup_work_v1(item_count: usize) -> Result<usize, ProofCacheErrorV1> {
    if item_count == 0 {
        Ok(1)
    } else {
        usize::try_from(usize::BITS - item_count.leading_zeros())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)
    }
}

fn charge_v1(
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<(), ProofCacheErrorV1> {
    charge_many_v1(work, 1, work_limit, control)
}

fn charge_many_v1(
    work: &mut usize,
    increment: usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<(), ProofCacheErrorV1> {
    control.checkpoint()?;
    *work = work
        .checked_add(increment)
        .filter(|value| *value <= work_limit)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    Ok(())
}
