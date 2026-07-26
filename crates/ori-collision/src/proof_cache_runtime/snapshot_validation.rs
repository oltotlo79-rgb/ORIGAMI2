//! Bounded current-snapshot preparation and normal-hit revalidation.

use super::*;

impl PersistentPairProofCacheRuntimeV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lookup_two_hinge_positive_v1(
        &self,
        capture: &ProofCacheRuntimeCaptureV1,
        issuer_context: [u8; 32],
        current_footprints: Vec<FaceDependencyFootprintV1>,
        current_exact_poses: Vec<ExactFacePoseCacheWitnessV1>,
        keys: &[super::super::ProofCacheKeyV1],
        work_limits: &ProofCachePairWorkLimitsV1,
        control: ProofCacheOperationControlV1<'_>,
    ) -> Result<ProofCacheBatchLookupV1, ProofCacheRuntimeErrorV1> {
        self.lookup_two_hinge_positive_inner_v1(
            capture,
            issuer_context,
            current_footprints,
            current_exact_poses,
            keys,
            work_limits,
            control,
            || {},
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lookup_two_hinge_positive_after_cache_hook_v1(
        &self,
        capture: &ProofCacheRuntimeCaptureV1,
        issuer_context: [u8; 32],
        current_footprints: Vec<FaceDependencyFootprintV1>,
        current_exact_poses: Vec<ExactFacePoseCacheWitnessV1>,
        keys: &[super::super::ProofCacheKeyV1],
        work_limits: &ProofCachePairWorkLimitsV1,
        control: ProofCacheOperationControlV1<'_>,
        after_cache_operation: impl FnOnce(),
    ) -> Result<ProofCacheBatchLookupV1, ProofCacheRuntimeErrorV1> {
        self.lookup_two_hinge_positive_inner_v1(
            capture,
            issuer_context,
            current_footprints,
            current_exact_poses,
            keys,
            work_limits,
            control,
            after_cache_operation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn lookup_two_hinge_positive_inner_v1(
        &self,
        capture: &ProofCacheRuntimeCaptureV1,
        issuer_context: [u8; 32],
        mut current_footprints: Vec<FaceDependencyFootprintV1>,
        mut current_exact_poses: Vec<ExactFacePoseCacheWitnessV1>,
        keys: &[super::super::ProofCacheKeyV1],
        work_limits: &ProofCachePairWorkLimitsV1,
        control: ProofCacheOperationControlV1<'_>,
        after_cache_operation: impl FnOnce(),
    ) -> Result<ProofCacheBatchLookupV1, ProofCacheRuntimeErrorV1> {
        Self::validate_model4_keys_v1(capture, issuer_context, keys)?;
        let (work_limit, pending_impact) = {
            let state = self.lock_v1()?;
            Self::validate_capture_locked_v1(&state, capture)?;
            if state.pending_impact.is_none() && state.binding.as_ref() != Some(&capture.binding) {
                return Err(ProofCacheRuntimeErrorV1::StaleProof);
            }
            (
                state.cache.limits.max_invalidation_work,
                state.pending_impact.clone(),
            )
        };
        // Complete sorting/preparation stays outside the cache mutex. Epoch
        // and pending-impact identity are rechecked before lookup or mutation.
        let mut runtime_work = canonicalize_current_snapshots_v1(
            &mut current_footprints,
            &mut current_exact_poses,
            if pending_impact.is_some() {
                0
            } else {
                keys.len()
            },
            work_limit,
            &control,
        )?;
        let cache_lookup_work =
            super::super::preflight_canonical_operation_v1(keys.len(), work_limit)?;
        runtime_charge_work_v1(&mut runtime_work, cache_lookup_work, work_limit, &control)?;
        let rebind_request = if let Some(impact) = pending_impact.clone() {
            let context = ProofCacheRebindContextV1::new(
                capture.binding.project_instance_id,
                capture.binding.project_id,
                capture.binding.revision,
                capture.binding.geometry_fingerprint,
                capture.binding.pose_generation,
                f64::from_bits(capture.binding.paper_thickness_bits),
                issuer_context,
            )?;
            Some(
                ProofCacheRebindRequestV1::from_complete_revision_snapshot_v1(
                    context,
                    impact,
                    current_footprints.clone(),
                    current_exact_poses.clone(),
                    Vec::new(),
                )?,
            )
        } else {
            None
        };

        let mut state = self.lock_v1()?;
        Self::validate_capture_locked_v1(&state, capture)?;
        if state.pending_impact != pending_impact {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        }
        let rebound = if let Some(request) = rebind_request {
            let report = state
                .cache
                .rebind_after_complete_edit_with_initial_work_v1(request, runtime_work, control)?;
            runtime_work = report.invalidation_work;
            state.pending_impact = None;
            state.binding = Some(capture.binding.clone());
            true
        } else if state.binding.as_ref() != Some(&capture.binding) {
            return Err(ProofCacheRuntimeErrorV1::StaleProof);
        } else {
            false
        };
        let mut lookup = state
            .cache
            .lookup_canonical_batch_v1(keys, work_limits, control)?;
        if lookup.cache_operation_work() != cache_lookup_work {
            return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
        }
        after_cache_operation();
        control.checkpoint()?;
        if !rebound {
            validate_hits_against_current_snapshots_v1(
                &state,
                &lookup,
                &current_footprints,
                &current_exact_poses,
                &mut runtime_work,
                work_limit,
                &control,
            )?;
        }
        lookup.set_runtime_operation_work_v1(runtime_work);
        Ok(lookup)
    }
}

fn canonicalize_current_snapshots_v1(
    footprints: &mut [FaceDependencyFootprintV1],
    exact_poses: &mut [ExactFacePoseCacheWitnessV1],
    key_count: usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<usize, ProofCacheRuntimeErrorV1> {
    if footprints.is_empty() || exact_poses.is_empty() {
        return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
    }
    let lookup_work = runtime_lookup_snapshot_work_v1(footprints.len(), key_count)?;
    let mut work = runtime_sort_work_v1(footprints.len())?
        .checked_add(runtime_sort_work_v1(exact_poses.len())?)
        .and_then(|value| value.checked_add(lookup_work))
        .filter(|value| *value <= work_limit)
        .ok_or(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded,
        ))?;
    control.checkpoint()?;
    footprints.sort_unstable_by_key(|item| item.face.canonical_bytes());
    control.checkpoint()?;
    exact_poses.sort_unstable_by_key(|item| item.face.canonical_bytes());
    control.checkpoint()?;
    if footprints.len() != exact_poses.len()
        || footprints
            .windows(2)
            .any(|pair| pair[0].face == pair[1].face)
        || exact_poses
            .windows(2)
            .any(|pair| pair[0].face == pair[1].face)
        || footprints
            .iter()
            .map(|item| item.face)
            .ne(exact_poses.iter().map(|item| item.face))
    {
        return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
    }
    runtime_charge_work_v1(
        &mut work,
        footprints
            .len()
            .checked_add(exact_poses.len())
            .ok_or(ProofCacheRuntimeErrorV1::Cache(
                ProofCacheErrorV1::ResourceLimitExceeded,
            ))?,
        work_limit,
        control,
    )?;
    Ok(work)
}

fn validate_hits_against_current_snapshots_v1(
    state: &ProofCacheRuntimeStateV1,
    lookup: &ProofCacheBatchLookupV1,
    footprints: &[FaceDependencyFootprintV1],
    exact_poses: &[ExactFacePoseCacheWitnessV1],
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<(), ProofCacheRuntimeErrorV1> {
    for hit in lookup.hits() {
        control.checkpoint()?;
        let entry = state
            .cache
            .entries
            .get(hit.key())
            .ok_or(ProofCacheRuntimeErrorV1::InvalidBinding)?;
        for expected in &entry.dependencies.footprints {
            let index = footprints
                .binary_search_by_key(&expected.face.canonical_bytes(), |item| {
                    item.face.canonical_bytes()
                })
                .map_err(|_| ProofCacheRuntimeErrorV1::InvalidBinding)?;
            if !runtime_footprint_equal_v1(expected, &footprints[index], work, work_limit, control)?
            {
                return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
            }
        }
        for expected in &entry.dependencies.exact_poses {
            let index = exact_poses
                .binary_search_by_key(&expected.face.canonical_bytes(), |item| {
                    item.face.canonical_bytes()
                })
                .map_err(|_| ProofCacheRuntimeErrorV1::InvalidBinding)?;
            if expected.face != exact_poses[index].face
                || !runtime_exact_bytes_equal_v1(
                    &expected.canonical_exact_bytes,
                    &exact_poses[index].canonical_exact_bytes,
                    work,
                    work_limit,
                    control,
                )?
            {
                return Err(ProofCacheRuntimeErrorV1::InvalidBinding);
            }
        }
    }
    control.checkpoint()?;
    Ok(())
}

fn runtime_sort_work_v1(item_count: usize) -> Result<usize, ProofCacheRuntimeErrorV1> {
    let levels = if item_count <= 1 {
        0
    } else {
        usize::try_from(usize::BITS - (item_count - 1).leading_zeros()).map_err(|_| {
            ProofCacheRuntimeErrorV1::Cache(ProofCacheErrorV1::ResourceLimitExceeded)
        })?
    };
    item_count
        .checked_mul(levels.saturating_add(2))
        .ok_or(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded,
        ))
}

fn runtime_lookup_snapshot_work_v1(
    face_count: usize,
    key_count: usize,
) -> Result<usize, ProofCacheRuntimeErrorV1> {
    let levels = if face_count <= 1 {
        1
    } else {
        usize::try_from(usize::BITS - (face_count - 1).leading_zeros()).map_err(|_| {
            ProofCacheRuntimeErrorV1::Cache(ProofCacheErrorV1::ResourceLimitExceeded)
        })?
    };
    key_count
        .checked_mul(4)
        .and_then(|lookups| lookups.checked_mul(levels.saturating_add(1)))
        .ok_or(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded,
        ))
}

fn runtime_footprint_equal_v1(
    expected: &FaceDependencyFootprintV1,
    current: &FaceDependencyFootprintV1,
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<bool, ProofCacheRuntimeErrorV1> {
    let id_work = expected
        .vertices
        .len()
        .checked_add(current.vertices.len())
        .and_then(|value| value.checked_add(expected.edges.len()))
        .and_then(|value| value.checked_add(current.edges.len()))
        .and_then(|value| value.checked_add(2))
        .ok_or(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded,
        ))?;
    runtime_charge_work_v1(work, id_work, work_limit, control)?;
    if expected.face != current.face
        || expected.vertices.len() != current.vertices.len()
        || expected.edges.len() != current.edges.len()
    {
        return Ok(false);
    }
    for (expected_chunk, current_chunk) in expected
        .vertices
        .chunks(1_024)
        .zip(current.vertices.chunks(1_024))
    {
        control.checkpoint()?;
        if expected_chunk != current_chunk {
            return Ok(false);
        }
    }
    for (expected_chunk, current_chunk) in expected
        .edges
        .chunks(1_024)
        .zip(current.edges.chunks(1_024))
    {
        control.checkpoint()?;
        if expected_chunk != current_chunk {
            return Ok(false);
        }
    }
    Ok(true)
}

fn runtime_exact_bytes_equal_v1(
    expected: &[u8],
    current: &[u8],
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<bool, ProofCacheRuntimeErrorV1> {
    let byte_work =
        expected
            .len()
            .checked_add(current.len())
            .ok_or(ProofCacheRuntimeErrorV1::Cache(
                ProofCacheErrorV1::ResourceLimitExceeded,
            ))?;
    runtime_charge_work_v1(work, byte_work, work_limit, control)?;
    if expected.len() != current.len() {
        return Ok(false);
    }
    for (expected_chunk, current_chunk) in expected.chunks(4_096).zip(current.chunks(4_096)) {
        control.checkpoint()?;
        if expected_chunk != current_chunk {
            return Ok(false);
        }
    }
    Ok(true)
}

fn runtime_charge_work_v1(
    work: &mut usize,
    increment: usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<(), ProofCacheRuntimeErrorV1> {
    control.checkpoint()?;
    *work = work
        .checked_add(increment)
        .filter(|value| *value <= work_limit)
        .ok_or(ProofCacheRuntimeErrorV1::Cache(
            ProofCacheErrorV1::ResourceLimitExceeded,
        ))?;
    Ok(())
}
