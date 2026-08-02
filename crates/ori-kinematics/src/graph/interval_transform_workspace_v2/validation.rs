//! Checkpointed binding validation and structural work arithmetic.

use super::*;

pub(super) fn checked_vec_bytes_v2<T>(count: usize) -> Option<usize> {
    size_of::<T>().checked_mul(count)
}

fn binary_search_comparisons_upper_bound_v2(count: usize) -> Option<usize> {
    if count == 0 {
        return Some(0);
    }
    (usize::BITS as usize - count.leading_zeros() as usize).checked_add(1)
}

pub(super) fn checked_validation_work_upper_bound_v2(
    face_count: usize,
    hinge_count: usize,
    spanning_count: usize,
    closure_count: usize,
) -> Option<usize> {
    let face_search = binary_search_comparisons_upper_bound_v2(face_count)?;
    let spanning_search = binary_search_comparisons_upper_bound_v2(spanning_count)?;
    let closure_search = binary_search_comparisons_upper_bound_v2(closure_count)?;
    let mut total = 32usize;
    total = total
        .checked_add(face_count.saturating_sub(1))?
        .checked_add(spanning_count.saturating_sub(1))?
        .checked_add(closure_count.saturating_sub(1))?
        .checked_add(face_count)?;
    // Three geometry/audit equality passes and five complete audit digests:
    // bound issuance, session precoverage, prepare recheck, input binding and
    // the final self-match.
    total = total
        .checked_add(face_count.checked_mul(3)?)?
        .checked_add(face_count.checked_add(hinge_count)?.checked_mul(5)?)?;
    total = total
        .checked_add(hinge_count.checked_mul(2)?)?
        .checked_add(hinge_count.checked_mul(spanning_search.checked_add(closure_search)?)?)?
        .checked_add(hinge_count.saturating_sub(1))?;
    total = total
        .checked_add(face_count.checked_mul(2)?)?
        .checked_add(hinge_count)?;
    total = total
        .checked_add(hinge_count.checked_mul(spanning_search)?.checked_mul(2)?)?
        .checked_add(spanning_count.checked_mul(face_search)?.checked_mul(4)?)?;
    total = total
        .checked_add(face_count.checked_mul(5)?)?
        .checked_add(spanning_count.checked_mul(4)?)?;
    total = total
        .checked_add(hinge_count.checked_mul(spanning_search)?)?
        .checked_add(closure_count.checked_mul(face_search)?.checked_mul(2)?)?
        .checked_add(face_search)?;
    total.checked_add(hinge_count.checked_mul(2)?)
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<(), IntervalFaceTransformWorkspaceErrorV2> {
    checkpoint().map_err(|stop| match stop {
        DyadicIntervalClosureStopV1::Cancelled => IntervalFaceTransformWorkspaceErrorV2::Cancelled,
        DyadicIntervalClosureStopV1::DeadlineExceeded => {
            IntervalFaceTransformWorkspaceErrorV2::DeadlineExceeded
        }
    })
}

fn update_usize_v2(hash: &mut Sha256, value: usize) -> Option<()> {
    hash.update(u64::try_from(value).ok()?.to_le_bytes());
    Some(())
}

pub(super) fn audit_binding_with_checkpoint_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<[u8; 32], IntervalFaceTransformWorkspaceErrorV2> {
    checkpoint_v2(checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_INTERVAL_FACE_TRANSFORM_AUDIT_V2");
    for face in audit.faces() {
        checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    for edge in audit.spanning_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    for edge in audit.closure_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    for count in [
        audit.faces().len(),
        audit.spanning_hinges().len(),
        audit.closure_hinges().len(),
    ] {
        update_usize_v2(&mut hash, count)
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
    }
    Ok(hash.finalize().into())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn interval_face_transform_input_binding_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    angle_boxes: &[(EdgeId, OutwardIntervalV1)],
    tolerance: f64,
    max_work: usize,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<[u8; 32], IntervalFaceTransformWorkspaceErrorV2> {
    checkpoint_v2(checkpoint)?;
    validate_geometry_audit_faces_v2(geometry, audit, fixed_face, checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_INTERVAL_FACE_TRANSFORM_INPUT_V2");
    hash.update(fixed_face.canonical_bytes());
    hash.update(tolerance.to_bits().to_le_bytes());
    update_usize_v2(&mut hash, max_work)
        .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
    hash.update(audit_binding_with_checkpoint_v2(audit, checkpoint)?);
    for (edge, interval) in angle_boxes {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
        hash.update(interval.lower().to_bits().to_le_bytes());
        hash.update(interval.upper().to_bits().to_le_bytes());
        update_usize_v2(&mut hash, interval.work())
            .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
    }
    update_usize_v2(&mut hash, angle_boxes.len())
        .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
    Ok(hash.finalize().into())
}

fn validate_geometry_audit_faces_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<(), IntervalFaceTransformWorkspaceErrorV2> {
    if audit.faces().is_empty() || geometry.face_ids().len() != audit.faces().len() {
        return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
    }
    let mut fixed_face_present = false;
    let mut previous: Option<FaceId> = None;
    for (geometry_face, audit_face) in geometry.face_ids().iter().zip(audit.faces()) {
        checkpoint_v2(checkpoint)?;
        if geometry_face != audit_face
            || previous.is_some_and(|value| value.canonical_bytes() >= audit_face.canonical_bytes())
        {
            return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
        }
        fixed_face_present |= *audit_face == fixed_face;
        previous = Some(*audit_face);
    }
    if !fixed_face_present {
        return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
    }
    Ok(())
}

pub(super) fn map_interval_attempt_error_v2(
    error: IntervalAttemptErrorV2,
) -> IntervalFaceTransformWorkspaceErrorV2 {
    match error {
        IntervalAttemptErrorV2::InvalidInput => IntervalFaceTransformWorkspaceErrorV2::InvalidInput,
        IntervalAttemptErrorV2::ResourceLimit => {
            IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
        }
        IntervalAttemptErrorV2::Unproven => IntervalFaceTransformWorkspaceErrorV2::Unproven,
        IntervalAttemptErrorV2::Cancelled => IntervalFaceTransformWorkspaceErrorV2::Cancelled,
        IntervalAttemptErrorV2::DeadlineExceeded => {
            IntervalFaceTransformWorkspaceErrorV2::DeadlineExceeded
        }
    }
}

pub(super) fn validate_canonical_audit_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<(usize, usize), IntervalFaceTransformWorkspaceErrorV2> {
    checkpoint_v2(checkpoint)?;
    validate_geometry_audit_faces_v2(geometry, audit, fixed_face, checkpoint)?;
    for edges in [audit.spanning_hinges(), audit.closure_hinges()] {
        for pair in edges.windows(2) {
            checkpoint_v2(checkpoint)?;
            if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
                return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
            }
        }
    }
    let hinge_count = audit
        .spanning_hinges()
        .len()
        .checked_add(audit.closure_hinges().len())
        .ok_or(IntervalFaceTransformWorkspaceErrorV2::ResourceLimit)?;
    if hinge_count == 0
        || audit.spanning_hinges().len() != audit.faces().len().saturating_sub(1)
        || geometry.hinges().len() != hinge_count
    {
        return Err(IntervalFaceTransformWorkspaceErrorV2::InvalidInput);
    }
    Ok((audit.faces().len(), hinge_count))
}
