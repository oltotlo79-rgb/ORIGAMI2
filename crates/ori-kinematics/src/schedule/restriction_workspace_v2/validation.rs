use crate::TreeHinge;

use super::*;

pub(super) fn checked_binding_work_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
) -> Option<usize> {
    audit
        .faces()
        .len()
        .checked_add(audit.spanning_hinges().len())?
        .checked_add(audit.closure_hinges().len())?
        .checked_add(geometry.hinges().len().checked_mul(10)?)?
        .checked_add(1)
}

pub(super) fn edge_is_in_block_preflight_v2(
    edge: EdgeId,
    block_geometry: &MaterialHingeGraphGeometry,
    charged_passes: usize,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    for hinge in block_geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        meter.charge_work(charged_passes)?;
        if hinge.edge() == edge {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn edge_is_in_block_poll_only_v2(
    edge: EdgeId,
    block_geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    for hinge in block_geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        if hinge.edge() == edge {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn slice_contains_face_with_checkpoint_v2(
    faces: &[FaceId],
    expected: FaceId,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    for face in faces {
        checkpoint_v2(checkpoint)?;
        meter.charge_work(1)?;
        if *face == expected {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn face_sets_equal_with_checkpoint_v2(
    expected: &[FaceId],
    actual: &[FaceId],
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    // Neither carrier type promises that its face slice uses the other's
    // physical order. Bidirectional membership plus equal cardinality also
    // rejects duplicates without allocating a temporary set.
    for (needles, haystack) in [(expected, actual), (actual, expected)] {
        for expected in needles {
            if !slice_contains_face_with_checkpoint_v2(haystack, *expected, meter, checkpoint)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn edges_contain_with_checkpoint_v2(
    edges: &[EdgeId],
    expected: EdgeId,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    for edge in edges {
        checkpoint_v2(checkpoint)?;
        meter.charge_work(1)?;
        if *edge == expected {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn audit_covers_block_edge_with_checkpoint_v2(
    audit: &MaterialHingeGraphAudit,
    expected: EdgeId,
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    let spanning =
        edges_contain_with_checkpoint_v2(audit.spanning_hinges(), expected, meter, checkpoint)?;
    let closure =
        edges_contain_with_checkpoint_v2(audit.closure_hinges(), expected, meter, checkpoint)?;
    if spanning == closure {
        return Ok(false);
    }
    Ok(true)
}

pub(super) fn block_edges_are_unique_with_checkpoint_v2(
    hinges: &[TreeHinge],
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    for (position, hinge) in hinges.iter().enumerate() {
        for prior in &hinges[..position] {
            checkpoint_v2(checkpoint)?;
            meter.charge_work(1)?;
            if prior.edge() == hinge.edge() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn block_hinges_are_from_source_with_checkpoint_v2(
    block_hinges: &[TreeHinge],
    source_hinges: &[TreeHinge],
    meter: &mut RestrictionMeterV2,
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<bool, CycleScheduleRestrictionWorkspaceErrorV2> {
    for block_hinge in block_hinges {
        checkpoint_v2(checkpoint)?;
        let mut found = false;
        for source_hinge in source_hinges {
            checkpoint_v2(checkpoint)?;
            meter.charge_work(1)?;
            if source_hinge == block_hinge {
                found = true;
                break;
            }
        }
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}
