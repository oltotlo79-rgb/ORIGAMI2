use super::*;

#[cfg(test)]
pub(super) fn has_nonempty_canonical_complete_partition_v2(partition: &[(u32, u64)]) -> bool {
    if partition.is_empty() {
        return false;
    }
    let mut cursor = 0_u128;
    for (depth, index) in partition {
        if *depth >= 64 || *index >= (1_u64 << depth) {
            return false;
        }
        let width = 1_u128 << (64 - depth);
        let start = u128::from(*index) * width;
        if start != cursor {
            return false;
        }
        cursor = match cursor.checked_add(width) {
            Some(value) => value,
            None => return false,
        };
    }
    cursor == (1_u128 << 64)
}

pub(super) fn validate_partition_with_checkpoint_v2(
    partition: &[(u32, u64)],
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<bool, DyadicIntervalClosureControlErrorV1> {
    if partition.is_empty() {
        return Ok(false);
    }
    let mut cursor = 0_u128;
    for (depth, index) in partition {
        closure_checkpoint_v1(checkpoint)?;
        if *depth >= 64 || *index >= (1_u64 << depth) {
            return Ok(false);
        }
        let width = 1_u128 << (64 - depth);
        let start = u128::from(*index) * width;
        if start != cursor {
            return Ok(false);
        }
        cursor = match cursor.checked_add(width) {
            Some(value) => value,
            None => return Ok(false),
        };
    }
    Ok(cursor == (1_u128 << 64))
}

pub(super) fn validate_audit_order_with_checkpoint_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<bool, DyadicIntervalClosureControlErrorV1> {
    for pair in audit.faces().windows(2) {
        closure_checkpoint_v1(checkpoint)?;
        if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
            return Ok(false);
        }
    }
    for edges in [audit.spanning_hinges(), audit.closure_hinges()] {
        for pair in edges.windows(2) {
            closure_checkpoint_v1(checkpoint)?;
            if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn validate_carrier_with_checkpoint_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    canonical_hinge_indices: &[usize],
    canonical_checked_hinges: &[EdgeId],
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<bool, DyadicIntervalClosureControlErrorV1> {
    if canonical_hinge_indices.len() != geometry.hinges().len()
        || canonical_checked_hinges.len() != geometry.hinges().len()
    {
        return Ok(false);
    }
    for position in 0..canonical_checked_hinges.len() {
        closure_checkpoint_v1(checkpoint)?;
        let edge = canonical_checked_hinges[position];
        if geometry.hinges()[canonical_hinge_indices[position]].edge() != edge
            || (position > 0
                && canonical_checked_hinges[position - 1].canonical_bytes()
                    >= edge.canonical_bytes())
        {
            return Ok(false);
        }
        let spanning = is_spanning_v2(audit, edge);
        let closure = audit
            .closure_hinges()
            .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
            .is_ok();
        if spanning == closure {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_partition_binding_with_checkpoint_v2(
    fixed_face: FaceId,
    schedule_binding_fingerprint_v2: [u8; 32],
    graph_binding_fingerprint_v1: [u8; 32],
    tolerance_bits: u64,
    policy: DyadicIntervalClosureWorkspaceLimitsV2,
    partition: &[(u32, u64)],
    canonical_checked_hinges: &[EdgeId],
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    exact_parallel_cut: bool,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<[u8; 32], DyadicIntervalClosureControlErrorV1> {
    closure_checkpoint_v1(checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_WORKSPACE_BOUNDED_DYADIC_CLOSURE_V2");
    if exact_parallel_cut {
        hash.update(b"EXACT_PARALLEL_CUT_AFFINE_V2");
    }
    hash.update(fixed_face.canonical_bytes());
    hash.update(schedule_binding_fingerprint_v2);
    hash.update(graph_binding_fingerprint_v1);
    hash.update(tolerance_bits.to_be_bytes());
    hash.update(policy.max_depth.to_be_bytes());
    for value in [
        policy.max_leaves,
        policy.max_work,
        policy.schedule_limits.max_hinges,
        policy.schedule_limits.max_degree,
        policy.schedule_limits.max_work,
        policy.max_theorem_recognizer_work,
        policy.max_theorem_recognizer_workspace_bytes,
        policy.max_carrier_index_workspace_bytes,
        policy.max_schedule_evaluation_workspace_bytes,
        policy.max_big_rational_payload_bytes,
        policy.max_exact_rational_object_bytes,
        policy.max_interval_closure_workspace_bytes,
        policy.max_partition_workspace_bytes,
        policy.max_retained_material_bytes,
        policy.max_publication_workspace_bytes,
        policy.max_peak_workspace_bytes,
    ] {
        closure_checkpoint_v1(checkpoint)?;
        let framed =
            u64::try_from(value).map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        hash.update(framed.to_be_bytes());
    }
    hash.update(policy.schedule_limits.max_coefficient_bits.to_be_bytes());
    for value in [
        resources.charged_binding_validation_upper_bound_bytes,
        resources.charged_theorem_recognizer_work,
        resources.charged_theorem_recognizer_upper_bound_bytes,
        resources.charged_carrier_index_workspace_upper_bound_bytes,
        resources.charged_schedule_evaluation_workspace_upper_bound_bytes,
        resources.charged_big_rational_payload_upper_bound_bytes,
        resources.charged_exact_rational_object_upper_bound_bytes,
        resources.charged_interval_closure_workspace_upper_bound_bytes,
        resources.charged_partition_workspace_upper_bound_bytes,
        resources.charged_retained_material_upper_bound_bytes,
        resources.charged_publication_workspace_upper_bound_bytes,
        resources.charged_peak_workspace_upper_bound_bytes,
        resources.visited_partition_nodes,
        resources.issued_leaves,
    ] {
        closure_checkpoint_v1(checkpoint)?;
        let framed =
            u64::try_from(value).map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        hash.update(framed.to_be_bytes());
    }
    hash.update(
        u64::try_from(partition.len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?
            .to_be_bytes(),
    );
    for (depth, index) in partition {
        closure_checkpoint_v1(checkpoint)?;
        hash.update(depth.to_be_bytes());
        hash.update(index.to_be_bytes());
    }
    hash.update(
        u64::try_from(canonical_checked_hinges.len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?
            .to_be_bytes(),
    );
    for edge in canonical_checked_hinges {
        closure_checkpoint_v1(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    closure_checkpoint_v1(checkpoint)?;
    Ok(hash.finalize().into())
}

pub(super) fn map_interval_control_error_v2(
    error: IntervalAttemptErrorV2,
) -> DyadicIntervalClosureControlErrorV1 {
    match error {
        IntervalAttemptErrorV2::InvalidInput => DyadicIntervalClosureErrorV1::InvalidInput.into(),
        IntervalAttemptErrorV2::ResourceLimit => DyadicIntervalClosureErrorV1::ResourceLimit.into(),
        IntervalAttemptErrorV2::Unproven => unreachable!("unproven is handled by subdivision"),
        IntervalAttemptErrorV2::Cancelled => DyadicIntervalClosureControlErrorV1::Cancelled,
        IntervalAttemptErrorV2::DeadlineExceeded => {
            DyadicIntervalClosureControlErrorV1::DeadlineExceeded
        }
    }
}

pub(super) fn map_heap_sort_error_v2(
    error: CheckpointHeapSortErrorV1<DyadicIntervalClosureStopV1>,
) -> DyadicIntervalClosureControlErrorV1 {
    match error {
        CheckpointHeapSortErrorV1::ResourceLimit => {
            DyadicIntervalClosureErrorV1::ResourceLimit.into()
        }
        CheckpointHeapSortErrorV1::Stop(DyadicIntervalClosureStopV1::Cancelled) => {
            DyadicIntervalClosureControlErrorV1::Cancelled
        }
        CheckpointHeapSortErrorV1::Stop(DyadicIntervalClosureStopV1::DeadlineExceeded) => {
            DyadicIntervalClosureControlErrorV1::DeadlineExceeded
        }
    }
}

pub(super) fn split_partition_leaf_v2(
    depth: u32,
    index: u64,
    pending: &mut Vec<(u32, u64)>,
    completed_len: usize,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Result<(), DyadicIntervalClosureControlErrorV1> {
    let child_depth = depth
        .checked_add(1)
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    let left = index
        .checked_mul(2)
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    let right = left
        .checked_add(1)
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    let future_leaves = pending
        .len()
        .checked_add(2)
        .and_then(|count| count.checked_add(completed_len))
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    if future_leaves > limits.max_leaves {
        return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
    }
    // Stack order makes traversal and retained publication left-first.
    pending.push((child_depth, right));
    pending.push((child_depth, left));
    Ok(())
}
