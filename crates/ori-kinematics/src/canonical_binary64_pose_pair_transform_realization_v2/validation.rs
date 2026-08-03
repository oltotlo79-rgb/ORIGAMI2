use std::{mem::size_of, sync::Arc};

use ori_domain::{EdgeId, FaceId};
use ori_topology::FoldAssignment;

use crate::{RigidTransform, TreeHinge};

use super::*;

type ErrorV2 = CanonicalBinary64PosePairTransformRealizationErrorV2;
type StopV2 = CanonicalBinary64PosePairTransformRealizationStopV2;
type InputV2<'a> = CanonicalBinary64PosePairTransformRealizationInputV2<'a>;
type EvidenceV2 = CanonicalBinary64PosePairTransformRealizationEvidenceV2;

pub(super) fn issue_v2(
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<EvidenceV2, ErrorV2> {
    resources::checkpoint_v2(checkpoint)?;
    preflight_limit_values_v2(input.limits)?;
    validate_pose_issuers_v2(input)?;
    let bound = resources::checked_resource_bound_v2(
        input.geometry,
        input.audit,
        input.lower_pose,
        input.upper_pose,
        checkpoint,
    )?;
    resources::validate_limits_v2(bound, input.limits)?;
    let spanning_marker = validate_shape_and_build_spanning_marker_v2(input, checkpoint)?;
    validate_pose_shape_v2(
        input.geometry,
        input.fixed_face,
        input.lower_pose,
        checkpoint,
    )?;
    validate_pose_shape_v2(
        input.geometry,
        input.fixed_face,
        input.upper_pose,
        checkpoint,
    )?;
    let spanning_marker_bytes = checked_vec_bytes_v2(&spanning_marker)?;
    verify_pose_pair_v2(input, &spanning_marker, spanning_marker_bytes, checkpoint)?;
    // Publication and fingerprinting are a later workspace phase. Make the
    // marker allocation's lifetime explicit so replay peak accounting may
    // conservatively take `max(vector workspace, candidate evidence shell)`.
    drop(spanning_marker);

    let resources = CanonicalBinary64PosePairTransformRealizationResourcesV2 {
        face_count: bound.face_count,
        hinge_count: bound.hinge_count,
        spanning_hinge_count: bound.spanning_hinge_count,
        pose_pair_deep_retained_bytes: bound.pose_pair_deep_retained_bytes,
        logical_work: bound.logical_work,
        workspace_structural_requirement_bytes: bound.workspace_structural_requirement_bytes,
    };
    let audit_binding = binding::audit_binding_with_checkpoint_v2(input.audit, checkpoint)?;
    let binding_fingerprint =
        binding::evidence_binding_with_checkpoint_v2(input, audit_binding, resources, checkpoint)?;
    resources::checkpoint_v2(checkpoint)?;
    Ok(EvidenceV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        lower_pose_instance: input.lower_pose.instance_anchor_v2(),
        upper_pose_instance: input.upper_pose.instance_anchor_v2(),
        fixed_face: input.fixed_face,
        audit_binding,
        resources,
        limits: input.limits,
        binding_fingerprint,
    })
}

pub(super) fn revalidate_v2(
    evidence: &EvidenceV2,
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    resources::checkpoint_v2(checkpoint)?;
    resources::preflight_live_limits_v2(
        input.geometry,
        input.lower_pose,
        input.upper_pose,
        input.limits,
    )?;
    if evidence.limits != input.limits
        || evidence.fixed_face != input.fixed_face
        || !evidence.issuer_geometry.matches(input.geometry)
        || !Arc::ptr_eq(
            &evidence.lower_pose_instance,
            &input.lower_pose.instance_anchor_v2(),
        )
        || !Arc::ptr_eq(
            &evidence.upper_pose_instance,
            &input.upper_pose.instance_anchor_v2(),
        )
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    let candidate = issue_v2(input, checkpoint)?;
    if evidence.issuer_geometry != candidate.issuer_geometry
        || !Arc::ptr_eq(
            &evidence.lower_pose_instance,
            &candidate.lower_pose_instance,
        )
        || !Arc::ptr_eq(
            &evidence.upper_pose_instance,
            &candidate.upper_pose_instance,
        )
        || evidence.fixed_face != candidate.fixed_face
        || evidence.audit_binding != candidate.audit_binding
        || evidence.resources != candidate.resources
        || evidence.limits != candidate.limits
        || evidence.binding_fingerprint != candidate.binding_fingerprint
    {
        return Err(ErrorV2::CertificateBindingMismatch);
    }
    resources::checkpoint_v2(checkpoint)
}

fn preflight_limit_values_v2(
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
) -> Result<(), ErrorV2> {
    if resources::limit_values_v2(limits)
        .into_iter()
        .any(|value| value == 0 || value == usize::MAX)
    {
        return Err(ErrorV2::ResourceLimit);
    }
    Ok(())
}

fn validate_pose_issuers_v2(input: InputV2<'_>) -> Result<(), ErrorV2> {
    if !input.lower_pose.is_for_geometry(input.geometry)
        || !input.upper_pose.is_for_geometry(input.geometry)
        || input.lower_pose.fixed_face() != input.fixed_face
        || input.upper_pose.fixed_face() != input.fixed_face
    {
        return Err(ErrorV2::PoseIssuerMismatch);
    }
    Ok(())
}

fn validate_shape_and_build_spanning_marker_v2(
    input: InputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<Vec<u8>, ErrorV2> {
    let geometry = input.geometry;
    let audit = input.audit;
    for (position, (geometry_face, audit_face)) in
        geometry.face_ids().iter().zip(audit.faces()).enumerate()
    {
        resources::checkpoint_v2(checkpoint)?;
        if geometry_face != audit_face
            || position > 0
                && geometry.face_ids()[position - 1].canonical_bytes()
                    >= geometry_face.canonical_bytes()
        {
            return Err(ErrorV2::AuditMismatch);
        }
    }
    if face_index_with_checkpoint_v2(geometry, input.fixed_face, checkpoint)?.is_none()
        || !strictly_ordered_edges_v2(audit.spanning_hinges(), checkpoint)?
        || !strictly_ordered_edges_v2(audit.closure_hinges(), checkpoint)?
    {
        return Err(ErrorV2::AuditMismatch);
    }

    let mut marker = Vec::<u8>::new();
    marker
        .try_reserve_exact(geometry.hinges().len())
        .map_err(|_| ErrorV2::ResourceLimit)?;
    if checked_vec_bytes_v2(&marker)? > input.limits.max_workspace_bytes {
        return Err(ErrorV2::ResourceLimit);
    }
    for (position, hinge) in geometry.hinges().iter().enumerate() {
        resources::checkpoint_v2(checkpoint)?;
        if position > 0
            && geometry.hinges()[position - 1].edge().canonical_bytes()
                >= hinge.edge().canonical_bytes()
            || hinge.left_face() == hinge.right_face()
            || face_index_with_checkpoint_v2(geometry, hinge.left_face(), checkpoint)?.is_none()
            || face_index_with_checkpoint_v2(geometry, hinge.right_face(), checkpoint)?.is_none()
        {
            return Err(ErrorV2::AuditMismatch);
        }
        let spanning =
            contains_edge_with_checkpoint_v2(audit.spanning_hinges(), hinge.edge(), checkpoint)?;
        let closure =
            contains_edge_with_checkpoint_v2(audit.closure_hinges(), hinge.edge(), checkpoint)?;
        if spanning == closure {
            return Err(ErrorV2::AuditMismatch);
        }
        marker.push(u8::from(spanning));
    }
    Ok(marker)
}

fn strictly_ordered_edges_v2(
    edges: &[EdgeId],
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<bool, ErrorV2> {
    for (position, edge) in edges.iter().enumerate() {
        resources::checkpoint_v2(checkpoint)?;
        if position > 0 && edges[position - 1].canonical_bytes() >= edge.canonical_bytes() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_pose_shape_v2(
    geometry: &MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    pose: &ClosedMaterialHingeGraphPose,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    if !pose.is_for_geometry(geometry)
        || pose.fixed_face() != fixed_face
        || pose.hinge_angles().as_slice().len() != geometry.hinges().len()
        || pose.transforms().len() != geometry.face_ids().len()
    {
        return Err(ErrorV2::PoseIssuerMismatch);
    }
    for (hinge, angle) in geometry.hinges().iter().zip(pose.hinge_angles().as_slice()) {
        resources::checkpoint_v2(checkpoint)?;
        if hinge.edge() != angle.edge() || !angle.angle_degrees().is_finite() {
            return Err(ErrorV2::TransformMismatch);
        }
    }
    for (face, transform) in geometry.face_ids().iter().zip(pose.transforms()) {
        resources::checkpoint_v2(checkpoint)?;
        if transform.face() != *face
            || binding::transform_bits_v2(transform.transform())
                .into_iter()
                .any(|bits| !f64::from_bits(bits).is_finite())
        {
            return Err(ErrorV2::TransformMismatch);
        }
    }
    Ok(())
}

fn verify_pose_pair_v2(
    input: InputV2<'_>,
    spanning_marker: &[u8],
    spanning_marker_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    let face_count = input.geometry.face_ids().len();
    let mut poses = Vec::<Option<RigidTransform>>::new();
    poses
        .try_reserve_exact(face_count)
        .map_err(|_| ErrorV2::ResourceLimit)?;
    let marker_and_poses_bytes = spanning_marker_bytes
        .checked_add(checked_vec_bytes_v2(&poses)?)
        .ok_or(ErrorV2::ResourceLimit)?;
    if marker_and_poses_bytes > input.limits.max_workspace_bytes {
        return Err(ErrorV2::ResourceLimit);
    }
    for _ in 0..face_count {
        resources::checkpoint_v2(checkpoint)?;
        poses.push(None);
    }
    let mut queue = Vec::<usize>::new();
    queue
        .try_reserve_exact(face_count)
        .map_err(|_| ErrorV2::ResourceLimit)?;
    let physical_bytes = marker_and_poses_bytes
        .checked_add(checked_vec_bytes_v2(&queue)?)
        .ok_or(ErrorV2::ResourceLimit)?;
    if physical_bytes > input.limits.max_workspace_bytes {
        return Err(ErrorV2::ResourceLimit);
    }

    verify_one_pose_v2(
        input.geometry,
        input.fixed_face,
        input.lower_pose,
        spanning_marker,
        &mut poses,
        &mut queue,
        checkpoint,
    )?;
    for pose in &mut poses {
        resources::checkpoint_v2(checkpoint)?;
        *pose = None;
    }
    queue.clear();
    verify_one_pose_v2(
        input.geometry,
        input.fixed_face,
        input.upper_pose,
        spanning_marker,
        &mut poses,
        &mut queue,
        checkpoint,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_one_pose_v2(
    geometry: &MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    live_pose: &ClosedMaterialHingeGraphPose,
    spanning_marker: &[u8],
    poses: &mut [Option<RigidTransform>],
    queue: &mut Vec<usize>,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    let fixed_index = face_index_with_checkpoint_v2(geometry, fixed_face, checkpoint)?
        .ok_or(ErrorV2::AuditMismatch)?;
    let identity = RigidTransform::identity();
    if !transform_bits_equal_with_checkpoint_v2(
        identity,
        live_pose.transforms()[fixed_index].transform(),
        checkpoint,
    )? {
        return Err(ErrorV2::TransformMismatch);
    }
    poses[fixed_index] = Some(identity);
    queue.push(fixed_index);
    let mut cursor = 0usize;
    while let Some(&parent_index) = queue.get(cursor) {
        cursor = cursor.checked_add(1).ok_or(ErrorV2::ResourceLimit)?;
        resources::checkpoint_v2(checkpoint)?;
        let parent_face = geometry.face_ids()[parent_index];
        let parent = poses[parent_index].ok_or(ErrorV2::TransformMismatch)?;
        for (hinge_index, hinge) in geometry.hinges().iter().enumerate() {
            resources::checkpoint_v2(checkpoint)?;
            if spanning_marker.get(hinge_index).copied() != Some(1) {
                continue;
            }
            let Some((child_face, rotation_sign)) = traversal_v2(parent_face, hinge) else {
                continue;
            };
            let child_index = face_index_with_checkpoint_v2(geometry, child_face, checkpoint)?
                .ok_or(ErrorV2::AuditMismatch)?;
            if poses[child_index].is_some() {
                continue;
            }
            let angle = live_pose.hinge_angles().as_slice()[hinge_index].angle_degrees();
            let local =
                RigidTransform::around_axis(hinge.start(), hinge.axis(), angle * rotation_sign)
                    .map_err(|_| ErrorV2::TransformMismatch)?;
            let expected = parent
                .compose(local)
                .map_err(|_| ErrorV2::TransformMismatch)?;
            if !transform_bits_equal_with_checkpoint_v2(
                expected,
                live_pose.transforms()[child_index].transform(),
                checkpoint,
            )? {
                return Err(ErrorV2::TransformMismatch);
            }
            poses[child_index] = Some(expected);
            queue.push(child_index);
        }
    }
    for pose in poses {
        resources::checkpoint_v2(checkpoint)?;
        if pose.is_none() {
            return Err(ErrorV2::TransformMismatch);
        }
    }
    Ok(())
}

fn traversal_v2(parent: FaceId, hinge: &TreeHinge) -> Option<(FaceId, f64)> {
    let assignment_sign = match hinge.assignment() {
        FoldAssignment::Mountain => 1.0,
        FoldAssignment::Valley => -1.0,
    };
    if parent == hinge.left_face() {
        Some((hinge.right_face(), assignment_sign))
    } else if parent == hinge.right_face() {
        Some((hinge.left_face(), -assignment_sign))
    } else {
        None
    }
}

fn transform_bits_equal_with_checkpoint_v2(
    expected: RigidTransform,
    actual: RigidTransform,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<bool, ErrorV2> {
    for (expected, actual) in binding::transform_bits_v2(expected)
        .into_iter()
        .zip(binding::transform_bits_v2(actual))
    {
        resources::checkpoint_v2(checkpoint)?;
        if expected != actual {
            return Ok(false);
        }
    }
    Ok(true)
}

fn face_index_with_checkpoint_v2(
    geometry: &MaterialHingeGraphGeometry,
    face: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<Option<usize>, ErrorV2> {
    let needle = face.canonical_bytes();
    let mut lower = 0usize;
    let mut upper = geometry.face_ids().len();
    while lower < upper {
        resources::checkpoint_v2(checkpoint)?;
        let middle = lower + (upper - lower) / 2;
        match geometry.face_ids()[middle].canonical_bytes().cmp(&needle) {
            std::cmp::Ordering::Less => lower = middle + 1,
            std::cmp::Ordering::Greater => upper = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}

fn contains_edge_with_checkpoint_v2(
    edges: &[EdgeId],
    edge: EdgeId,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<bool, ErrorV2> {
    let needle = edge.canonical_bytes();
    let mut lower = 0usize;
    let mut upper = edges.len();
    while lower < upper {
        resources::checkpoint_v2(checkpoint)?;
        let middle = lower + (upper - lower) / 2;
        match edges[middle].canonical_bytes().cmp(&needle) {
            std::cmp::Ordering::Less => lower = middle + 1,
            std::cmp::Ordering::Greater => upper = middle,
            std::cmp::Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}

fn checked_vec_bytes_v2<T>(values: &Vec<T>) -> Result<usize, ErrorV2> {
    checked_vec_bytes_from_capacity_v2::<T>(values.capacity())
}

fn checked_vec_bytes_from_capacity_v2<T>(capacity: usize) -> Result<usize, ErrorV2> {
    size_of::<T>()
        .checked_mul(capacity)
        .ok_or(ErrorV2::ResourceLimit)
}
