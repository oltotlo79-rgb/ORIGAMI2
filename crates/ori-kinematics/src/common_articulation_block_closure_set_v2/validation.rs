//! Issuance-time validation and bounded materialization for V2 closure sets.

use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};

use crate::graph::{CheckpointHeapSortErrorV1, checkpoint_heap_sort_by_key_v1};
use crate::{
    CommonArticulationPoseErrorV2, CommonArticulationPoseInputV2, CommonArticulationPoseStopV2,
    CycleSchedulePrepareErrorV1, CycleScheduleRestrictionErrorV1, CycleScheduleRestrictionStopV1,
    DyadicIntervalClosureControlErrorV1, DyadicIntervalClosureStopV1, KinematicsError,
};

use super::*;

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const CANONICAL_MIURA_FACES_PER_BLOCK_V2: usize = 9;
const CANONICAL_MIURA_HINGES_PER_BLOCK_V2: usize = 12;

pub(super) fn issue_v2(
    input: CommonArticulationBlockClosureSetInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<CommonArticulationBlockClosureSetV2, CommonArticulationBlockClosureSetErrorV2> {
    checkpoint_v2(&mut checkpoint)?;
    let preflight = preflight_v2(input, &mut checkpoint)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(preflight.actual_block_count)
        .map_err(|_| CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
    let mut covered_edges = Vec::new();
    covered_edges
        .try_reserve_exact(preflight.hinge_count)
        .map_err(|_| CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
    let mut total_schedule_bytes = 0usize;
    let mut total_closure_bytes = 0usize;
    let mut total_closure_leaves = 0usize;

    for (block_index, block) in input.decomposition.blocks().iter().enumerate() {
        checkpoint_v2(&mut checkpoint)?;
        let block_geometry = block.geometry();
        let block_audit = block.audit();
        if block_geometry.face_ids().len() != CANONICAL_MIURA_FACES_PER_BLOCK_V2
            || block_geometry.hinges().len() != CANONICAL_MIURA_HINGES_PER_BLOCK_V2
            || block_geometry.face_ids() != block_audit.faces()
        {
            return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
        }
        let fixed_face = canonical_block_articulation_face_v2(
            block_geometry,
            &preflight.articulation_faces,
            &mut checkpoint,
        )?;
        let geometry_audit_binding =
            geometry_audit_binding_v2(block_geometry, block_audit, &mut checkpoint)?;
        let restricted = input
            .parent_schedule
            .restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
                input.geometry,
                input.audit,
                block_geometry,
                block_audit,
                fixed_face,
                || restriction_checkpoint_v2(&mut checkpoint),
            )
            .map_err(restriction_error_v2)?;
        if !restricted.matches_binding(block_geometry, block_audit, fixed_face) {
            return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
        }
        let schedule_bytes = restricted
            .checked_deep_retained_bytes_v1()
            .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
        if schedule_bytes > input.limits.max_block_schedule_bytes {
            return Err(CommonArticulationBlockClosureSetErrorV2::ResourceLimit);
        }
        total_schedule_bytes = total_schedule_bytes
            .checked_add(schedule_bytes)
            .filter(|total| *total <= input.limits.max_total_block_schedule_bytes)
            .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;

        let closure = block_geometry
            .prove_dyadic_schedule_closure_with_checkpoint_v1(
                block_audit,
                fixed_face,
                &restricted,
                input.closure_tolerance,
                input.limits.per_block_closure_limits,
                || closure_checkpoint_v2(&mut checkpoint),
            )
            .map_err(closure_error_v2)?;
        checkpoint_v2(&mut checkpoint)?;
        if closure.fixed_face() != fixed_face
            || !closure.has_canonical_complete_partition_v1()
            || !closure.every_leaf_covers_graph_v1(block_geometry)
        {
            return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
        }
        let closure_bytes = closure
            .checked_deep_retained_bytes_v1()
            .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
        let closure_leaves = closure.leaves().len();
        if closure_bytes > input.limits.max_block_closure_bytes {
            return Err(CommonArticulationBlockClosureSetErrorV2::ResourceLimit);
        }
        total_closure_bytes = total_closure_bytes
            .checked_add(closure_bytes)
            .filter(|total| *total <= input.limits.max_total_block_closure_bytes)
            .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
        total_closure_leaves = total_closure_leaves
            .checked_add(closure_leaves)
            .filter(|total| *total <= input.limits.max_total_closure_leaves)
            .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
        for hinge in block_geometry.hinges() {
            checkpoint_v2(&mut checkpoint)?;
            covered_edges.push(hinge.edge());
        }
        records.push(BlockClosureRecordV2 {
            block_index,
            fixed_face,
            geometry_audit_binding,
            schedule: restricted,
            closure,
            schedule_bytes,
            closure_bytes,
            closure_leaves,
        });
    }
    verify_complete_parent_edge_coverage_v2(
        input.geometry,
        &mut covered_edges,
        preflight.hinge_count,
        &mut checkpoint,
    )?;
    if records.len() != preflight.actual_block_count {
        return Err(CommonArticulationBlockClosureSetErrorV2::ResourceLimit);
    }
    let binding_fingerprint = binding_fingerprint_v2(
        input,
        &preflight,
        total_schedule_bytes,
        total_closure_bytes,
        total_closure_leaves,
        &records,
        &mut checkpoint,
    )?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(CommonArticulationBlockClosureSetV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        profile_binding: input.profile.binding_fingerprint_v2(),
        decomposition_binding: input.decomposition.binding_fingerprint_v2(),
        common_pose_binding: input.common_pose.binding_fingerprint_v2(),
        audit_binding: preflight.audit_binding,
        parent_schedule_binding: input.parent_schedule.certificate_binding_fingerprint_v2(),
        parent_fixed_face: input.parent_fixed_face,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        configured_max_blocks: preflight.configured_max_blocks,
        actual_block_count: preflight.actual_block_count,
        face_count: preflight.face_count,
        hinge_count: preflight.hinge_count,
        limits: input.limits,
        total_block_schedule_bytes: total_schedule_bytes,
        total_block_closure_bytes: total_closure_bytes,
        total_closure_leaves,
        blocks: records,
        binding_fingerprint,
    })
}

struct PreflightV2 {
    configured_max_blocks: usize,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    articulation_faces: Vec<FaceId>,
    audit_binding: [u8; 32],
}

fn preflight_v2(
    input: CommonArticulationBlockClosureSetInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<PreflightV2, CommonArticulationBlockClosureSetErrorV2> {
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.closure_tolerance.is_finite()
        || input.closure_tolerance < 0.0
        || input.closure_tolerance.to_bits() == (-0.0_f64).to_bits()
    {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || input.limits.max_blocks != configured_max_blocks
        || face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.decomposition.face_count_v2() != face_count
        || input.decomposition.hinge_count_v2() != hinge_count
        || input.decomposition.blocks().len() != actual_block_count
        || !input.decomposition.is_for_geometry(input.geometry)
        || !input.decomposition.is_for_profile_v2(input.profile)
        || input.geometry.face_ids() != input.audit.faces()
        || !input.parent_schedule.matches_binding(
            input.geometry,
            input.audit,
            input.parent_fixed_face,
        )
    {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    let parent_schedule_bytes = input
        .parent_schedule
        .checked_deep_retained_bytes_v1()
        .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
    if parent_schedule_bytes > input.limits.max_parent_schedule_bytes {
        return Err(CommonArticulationBlockClosureSetErrorV2::ResourceLimit);
    }
    schedule_matches_pose_at_zero_v2(
        input.parent_schedule,
        input.pose,
        input.parent_fixed_face,
        checkpoint,
    )?;
    input
        .common_pose
        .revalidate_with_checkpoint_v2(
            CommonArticulationPoseInputV2 {
                geometry: input.geometry,
                pose: input.pose,
                decomposition: input.decomposition,
                paper_thickness_mm: input.paper_thickness_mm,
                profile: input.profile,
            },
            || pose_checkpoint_v2(checkpoint),
        )
        .map_err(pose_error_v2)?;
    if input.common_pose.configured_max_blocks_v2() != configured_max_blocks
        || input.common_pose.actual_block_count_v2() != actual_block_count
        || input.common_pose.profile_binding_fingerprint_v2()
            != input.profile.binding_fingerprint_v2()
    {
        return Err(CommonArticulationBlockClosureSetErrorV2::IssuerMismatch);
    }
    let articulation_faces = canonical_articulation_faces_v2(
        input.decomposition.articulation_faces(),
        actual_block_count,
        input.geometry,
        checkpoint,
    )?;
    let audit_binding = geometry_audit_binding_v2(input.geometry, input.audit, checkpoint)?;
    Ok(PreflightV2 {
        configured_max_blocks,
        actual_block_count,
        face_count,
        hinge_count,
        articulation_faces,
        audit_binding,
    })
}

fn schedule_matches_pose_at_zero_v2(
    schedule: &CanonicalCycleScheduleV1,
    pose: &ClosedMaterialHingeGraphPose,
    parent_fixed_face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
    if pose.fixed_face() != parent_fixed_face {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    let scheduled = schedule.try_evaluate_v1(0.0).map_err(|error| match error {
        KinematicsError::ResourceLimitExceeded => {
            CommonArticulationBlockClosureSetErrorV2::ResourceLimit
        }
        _ => CommonArticulationBlockClosureSetErrorV2::InvalidInput,
    })?;
    let posed = pose.hinge_angles();
    if scheduled.as_slice().len() != posed.as_slice().len() {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    for (scheduled, posed) in scheduled.as_slice().iter().zip(posed.as_slice()) {
        checkpoint_v2(checkpoint)?;
        if scheduled.edge() != posed.edge()
            || scheduled.angle_degrees().to_bits() != posed.angle_degrees().to_bits()
        {
            return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn canonical_articulation_faces_v2(
    source: &[FaceId],
    actual_block_count: usize,
    geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<Vec<FaceId>, CommonArticulationBlockClosureSetErrorV2> {
    if source.len()
        != actual_block_count
            .checked_sub(1)
            .ok_or(CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?
    {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(source.len())
        .map_err(|_| CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
    for face in source {
        checkpoint_v2(checkpoint)?;
        if geometry
            .face_ids()
            .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
            .is_err()
        {
            return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
        }
        faces.push(*face);
    }
    checkpoint_heap_sort_by_key_v1(&mut faces, FaceId::canonical_bytes, checkpoint)
        .map_err(heap_sort_error_v2)?;
    if faces.windows(2).any(|window| window[0] == window[1]) {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    Ok(faces)
}

fn canonical_block_articulation_face_v2(
    geometry: &MaterialHingeGraphGeometry,
    articulation_faces: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<FaceId, CommonArticulationBlockClosureSetErrorV2> {
    let mut selected = None;
    for face in geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        if articulation_faces
            .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
            .is_ok()
            && selected
                .is_none_or(|current: FaceId| face.canonical_bytes() < current.canonical_bytes())
        {
            selected = Some(*face);
        }
    }
    selected.ok_or(CommonArticulationBlockClosureSetErrorV2::InvalidInput)
}

fn verify_complete_parent_edge_coverage_v2(
    geometry: &MaterialHingeGraphGeometry,
    covered_edges: &mut [EdgeId],
    expected_hinge_count: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
    if covered_edges.len() != expected_hinge_count {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(geometry.hinges().len())
        .map_err(|_| CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?;
    for hinge in geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        expected.push(hinge.edge());
    }
    checkpoint_v2(checkpoint)?;
    checkpoint_heap_sort_by_key_v1(covered_edges, EdgeId::canonical_bytes, checkpoint)
        .map_err(heap_sort_error_v2)?;
    checkpoint_heap_sort_by_key_v1(&mut expected, EdgeId::canonical_bytes, checkpoint)
        .map_err(heap_sort_error_v2)?;
    checkpoint_v2(checkpoint)?;
    (covered_edges == expected.as_slice())
        .then_some(())
        .ok_or(CommonArticulationBlockClosureSetErrorV2::InvalidInput)
}

/// Deterministic in-place ascending heap sort with cooperative polling at
/// every heap construction, extraction, and sift comparison.  It retains no
/// auxiliary buffer, so the caller's already-admitted vector remains its
/// complete allocation footprint.
fn heap_sort_error_v2(
    error: CheckpointHeapSortErrorV1<CommonArticulationBlockClosureSetStopV2>,
) -> CommonArticulationBlockClosureSetErrorV2 {
    match error {
        CheckpointHeapSortErrorV1::Stop(stop) => match stop {
            CommonArticulationBlockClosureSetStopV2::Cancelled => {
                CommonArticulationBlockClosureSetErrorV2::Cancelled
            }
            CommonArticulationBlockClosureSetStopV2::DeadlineExceeded => {
                CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded
            }
        },
        CheckpointHeapSortErrorV1::ResourceLimit => {
            CommonArticulationBlockClosureSetErrorV2::ResourceLimit
        }
    }
}

fn geometry_audit_binding_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<[u8; 32], CommonArticulationBlockClosureSetErrorV2> {
    if geometry.face_ids() != audit.faces() {
        return Err(CommonArticulationBlockClosureSetErrorV2::InvalidInput);
    }
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_BLOCK_CLOSURE_GEOMETRY_AUDIT_BINDING_V2");
    hash_count_v2(&mut hash, geometry.face_ids().len())?;
    for face in geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    hash_count_v2(&mut hash, geometry.hinges().len())?;
    for hinge in geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(hinge.edge().canonical_bytes());
        hash.update(hinge.left_face().canonical_bytes());
        hash.update(hinge.right_face().canonical_bytes());
    }
    hash_count_v2(&mut hash, audit.spanning_hinges().len())?;
    for edge in audit.spanning_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    hash_count_v2(&mut hash, audit.closure_hinges().len())?;
    for edge in audit.closure_hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    Ok(hash.finalize().into())
}

fn binding_fingerprint_v2(
    input: CommonArticulationBlockClosureSetInputV2<'_>,
    preflight: &PreflightV2,
    total_schedule_bytes: usize,
    total_closure_bytes: usize,
    total_closure_leaves: usize,
    records: &[BlockClosureRecordV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<[u8; 32], CommonArticulationBlockClosureSetErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_BLOCK_CLOSURE_SET_MODEL_ID_V2.as_bytes());
    hash.update(input.profile.binding_fingerprint_v2());
    hash.update(input.decomposition.binding_fingerprint_v2());
    hash.update(input.common_pose.binding_fingerprint_v2());
    hash.update(preflight.audit_binding);
    hash.update(input.parent_schedule.certificate_binding_fingerprint_v2());
    hash.update(input.parent_fixed_face.canonical_bytes());
    hash.update(input.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(input.closure_tolerance.to_bits().to_le_bytes());
    hash_count_v2(&mut hash, preflight.configured_max_blocks)?;
    hash_count_v2(&mut hash, preflight.actual_block_count)?;
    hash_count_v2(&mut hash, preflight.face_count)?;
    hash_count_v2(&mut hash, preflight.hinge_count)?;
    hash_limits_v2(&mut hash, input.limits)?;
    hash_count_v2(&mut hash, total_schedule_bytes)?;
    hash_count_v2(&mut hash, total_closure_bytes)?;
    hash_count_v2(&mut hash, total_closure_leaves)?;
    hash_count_v2(&mut hash, records.len())?;
    for record in records {
        checkpoint_v2(checkpoint)?;
        hash_count_v2(&mut hash, record.block_index)?;
        hash.update(record.fixed_face.canonical_bytes());
        hash.update(record.geometry_audit_binding);
        hash.update(record.schedule.certificate_binding_fingerprint_v2());
        hash.update(record.closure.partition_binding_fingerprint_v2());
        hash_count_v2(&mut hash, record.schedule_bytes)?;
        hash_count_v2(&mut hash, record.closure_bytes)?;
        hash_count_v2(&mut hash, record.closure_leaves)?;
    }
    Ok(hash.finalize().into())
}

fn hash_limits_v2(
    hash: &mut Sha256,
    limits: CommonArticulationBlockClosureSetLimitsV2,
) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
    hash_count_v2(hash, limits.max_blocks)?;
    hash_count_v2(hash, limits.max_parent_schedule_bytes)?;
    hash_count_v2(hash, limits.max_block_schedule_bytes)?;
    hash_count_v2(hash, limits.max_total_block_schedule_bytes)?;
    hash_count_v2(hash, limits.max_block_closure_bytes)?;
    hash_count_v2(hash, limits.max_total_block_closure_bytes)?;
    hash_count_v2(hash, limits.max_total_closure_leaves)?;
    hash.update(limits.per_block_closure_limits.max_depth.to_le_bytes());
    hash_count_v2(hash, limits.per_block_closure_limits.max_leaves)?;
    hash_count_v2(hash, limits.per_block_closure_limits.max_work)?;
    hash_count_v2(
        hash,
        limits.per_block_closure_limits.schedule_limits.max_hinges,
    )?;
    hash_count_v2(
        hash,
        limits.per_block_closure_limits.schedule_limits.max_degree,
    )?;
    hash.update(
        limits
            .per_block_closure_limits
            .schedule_limits
            .max_coefficient_bits
            .to_le_bytes(),
    );
    hash_count_v2(
        hash,
        limits.per_block_closure_limits.schedule_limits.max_work,
    )
}

fn hash_count_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| CommonArticulationBlockClosureSetErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}

pub(super) fn records_equal_v2(
    left: &[BlockClosureRecordV2],
    right: &[BlockClosureRecordV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<bool, CommonArticulationBlockClosureSetErrorV2> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        checkpoint_v2(checkpoint)?;
        if left.block_index != right.block_index
            || left.fixed_face != right.fixed_face
            || left.geometry_audit_binding != right.geometry_audit_binding
            || left.schedule != right.schedule
            || left.closure != right.closure
            || left.schedule_bytes != right.schedule_bytes
            || left.closure_bytes != right.closure_bytes
            || left.closure_leaves != right.closure_leaves
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), CommonArticulationBlockClosureSetErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationBlockClosureSetStopV2::Cancelled => {
            CommonArticulationBlockClosureSetErrorV2::Cancelled
        }
        CommonArticulationBlockClosureSetStopV2::DeadlineExceeded => {
            CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded
        }
    })
}

fn restriction_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), CycleScheduleRestrictionStopV1> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationBlockClosureSetStopV2::Cancelled => {
            CycleScheduleRestrictionStopV1::Cancelled
        }
        CommonArticulationBlockClosureSetStopV2::DeadlineExceeded => {
            CycleScheduleRestrictionStopV1::DeadlineExceeded
        }
    })
}

fn closure_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), DyadicIntervalClosureStopV1> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationBlockClosureSetStopV2::Cancelled => {
            DyadicIntervalClosureStopV1::Cancelled
        }
        CommonArticulationBlockClosureSetStopV2::DeadlineExceeded => {
            DyadicIntervalClosureStopV1::DeadlineExceeded
        }
    })
}

fn pose_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationBlockClosureSetStopV2>,
) -> Result<(), CommonArticulationPoseStopV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationBlockClosureSetStopV2::Cancelled => {
            CommonArticulationPoseStopV2::Cancelled
        }
        CommonArticulationBlockClosureSetStopV2::DeadlineExceeded => {
            CommonArticulationPoseStopV2::DeadlineExceeded
        }
    })
}

fn restriction_error_v2(
    error: CycleScheduleRestrictionErrorV1,
) -> CommonArticulationBlockClosureSetErrorV2 {
    match error {
        CycleScheduleRestrictionErrorV1::Prepare(CycleSchedulePrepareErrorV1::ResourceLimit) => {
            CommonArticulationBlockClosureSetErrorV2::ResourceLimit
        }
        CycleScheduleRestrictionErrorV1::Prepare(_) => {
            CommonArticulationBlockClosureSetErrorV2::InvalidInput
        }
        CycleScheduleRestrictionErrorV1::Cancelled => {
            CommonArticulationBlockClosureSetErrorV2::Cancelled
        }
        CycleScheduleRestrictionErrorV1::DeadlineExceeded => {
            CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded
        }
    }
}

fn closure_error_v2(
    error: DyadicIntervalClosureControlErrorV1,
) -> CommonArticulationBlockClosureSetErrorV2 {
    match error {
        DyadicIntervalClosureControlErrorV1::Closure(
            crate::DyadicIntervalClosureErrorV1::ResourceLimit,
        ) => CommonArticulationBlockClosureSetErrorV2::ResourceLimit,
        DyadicIntervalClosureControlErrorV1::Closure(_) => {
            CommonArticulationBlockClosureSetErrorV2::InvalidInput
        }
        DyadicIntervalClosureControlErrorV1::Cancelled => {
            CommonArticulationBlockClosureSetErrorV2::Cancelled
        }
        DyadicIntervalClosureControlErrorV1::DeadlineExceeded => {
            CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded
        }
    }
}

fn pose_error_v2(error: CommonArticulationPoseErrorV2) -> CommonArticulationBlockClosureSetErrorV2 {
    match error {
        CommonArticulationPoseErrorV2::ResourceLimit => {
            CommonArticulationBlockClosureSetErrorV2::ResourceLimit
        }
        CommonArticulationPoseErrorV2::Cancelled => {
            CommonArticulationBlockClosureSetErrorV2::Cancelled
        }
        CommonArticulationPoseErrorV2::DeadlineExceeded => {
            CommonArticulationBlockClosureSetErrorV2::DeadlineExceeded
        }
        CommonArticulationPoseErrorV2::IssuerMismatch => {
            CommonArticulationBlockClosureSetErrorV2::IssuerMismatch
        }
        CommonArticulationPoseErrorV2::InvalidInput
        | CommonArticulationPoseErrorV2::PoseIssuerMismatch
        | CommonArticulationPoseErrorV2::DecompositionIssuerMismatch => {
            CommonArticulationBlockClosureSetErrorV2::InvalidInput
        }
    }
}

#[cfg(test)]
mod heap_sort_tests {
    use ori_domain::{EdgeId, ProjectId};

    use super::*;

    const LARGE_SORT_LEN: usize = 1_025;
    const MID_SORT_POLL: usize = 1_025;

    #[test]
    fn large_heap_sort_is_canonical_and_observes_mid_sort_cancel_and_deadline() {
        let mut successful = descending_edges_v2();
        checkpoint_heap_sort_by_key_v1(&mut successful, EdgeId::canonical_bytes, &mut || {
            Ok::<(), CommonArticulationBlockClosureSetStopV2>(())
        })
        .expect("large canonical edge sort");
        assert!(
            successful
                .windows(2)
                .all(|window| window[0].canonical_bytes() < window[1].canonical_bytes())
        );

        for expected_stop in [
            CommonArticulationBlockClosureSetStopV2::Cancelled,
            CommonArticulationBlockClosureSetStopV2::DeadlineExceeded,
        ] {
            let mut edges = descending_edges_v2();
            let mut polls = 0usize;
            assert!(matches!(
                checkpoint_heap_sort_by_key_v1(
                    &mut edges,
                    EdgeId::canonical_bytes,
                    &mut || {
                        polls += 1;
                        (polls != MID_SORT_POLL).then_some(()).ok_or(expected_stop)
                    },
                ),
                Err(CheckpointHeapSortErrorV1::Stop(stop)) if stop == expected_stop,
            ));
            assert_eq!(polls, MID_SORT_POLL);
        }
    }

    fn descending_edges_v2() -> Vec<EdgeId> {
        let namespace = ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x53, 0x4f, 0x52, 0x54, 0, 0, 1,
        ]);
        (0..LARGE_SORT_LEN)
            .rev()
            .map(|index| EdgeId::derive_v5(namespace, &index.to_le_bytes()))
            .collect()
    }
}
