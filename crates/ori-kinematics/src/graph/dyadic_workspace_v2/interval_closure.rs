use super::*;

pub(super) fn map_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<(), IntervalAttemptErrorV2> {
    checkpoint().map_err(|stop| match stop {
        DyadicIntervalClosureStopV1::Cancelled => IntervalAttemptErrorV2::Cancelled,
        DyadicIntervalClosureStopV1::DeadlineExceeded => IntervalAttemptErrorV2::DeadlineExceeded,
    })
}

fn face_index_v2(audit: &MaterialHingeGraphAudit, face: FaceId) -> Option<usize> {
    audit
        .faces()
        .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
        .ok()
}

pub(super) fn is_spanning_v2(audit: &MaterialHingeGraphAudit, edge: EdgeId) -> bool {
    audit
        .spanning_hinges()
        .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
        .is_ok()
}

fn checked_interval_physical_capacity_bytes_v2(
    adjacency: &[Vec<(usize, usize, bool)>],
    adjacency_outer_capacity: usize,
    degree_capacity: usize,
    poses_capacity: usize,
    queue_capacity: usize,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<usize, IntervalAttemptErrorV2> {
    let mut total = checked_vec_bytes_v2::<Vec<(usize, usize, bool)>>(adjacency_outer_capacity)
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    for neighbors in adjacency {
        map_checkpoint_v2(checkpoint)?;
        total = total
            .checked_add(
                checked_vec_bytes_v2::<(usize, usize, bool)>(neighbors.capacity())
                    .ok_or(IntervalAttemptErrorV2::ResourceLimit)?,
            )
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    }
    total = total
        .checked_add(
            checked_vec_bytes_v2::<usize>(degree_capacity)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?,
        )
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?
        .checked_add(
            checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(poses_capacity)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?,
        )
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?
        .checked_add(
            checked_vec_bytes_v2::<usize>(queue_capacity)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?,
        )
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    Ok(total)
}

pub(in crate::graph) struct IntervalClosureRequestV2<'a> {
    pub(in crate::graph) geometry: &'a MaterialHingeGraphGeometry,
    pub(in crate::graph) audit: &'a MaterialHingeGraphAudit,
    pub(in crate::graph) fixed_face: FaceId,
    pub(in crate::graph) canonical_hinge_indices: &'a [usize],
    pub(in crate::graph) angle_boxes: &'a [(EdgeId, OutwardIntervalV1)],
    pub(in crate::graph) tolerance: f64,
    pub(in crate::graph) max_work: usize,
    pub(in crate::graph) max_workspace_bytes: usize,
    pub(in crate::graph) max_pose_capacity_bytes: usize,
    pub(in crate::graph) verification_mode: IntervalClosureVerificationModeV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph) enum IntervalClosureVerificationModeV2 {
    FullClosure,
    SpanningObservation,
}

pub(in crate::graph) fn prove_interval_closure_with_workspace_v2(
    request: IntervalClosureRequestV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<IntervalAttemptSuccessV2, IntervalAttemptErrorV2> {
    let IntervalClosureRequestV2 {
        geometry,
        audit,
        fixed_face,
        canonical_hinge_indices,
        angle_boxes,
        tolerance,
        max_work,
        max_workspace_bytes,
        max_pose_capacity_bytes,
        verification_mode,
    } = request;
    map_checkpoint_v2(checkpoint)?;
    if !tolerance.is_finite()
        || tolerance < 0.0
        || max_work == 0
        || geometry.face_ids().len() != audit.faces().len()
        || geometry.hinges().len() != angle_boxes.len()
        || geometry.hinges().len() != canonical_hinge_indices.len()
        || geometry.hinges().len()
            != audit
                .spanning_hinges()
                .len()
                .checked_add(audit.closure_hinges().len())
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?
    {
        return Err(IntervalAttemptErrorV2::InvalidInput);
    }
    for (geometry_face, audit_face) in geometry.face_ids().iter().zip(audit.faces()) {
        map_checkpoint_v2(checkpoint)?;
        if geometry_face != audit_face {
            return Err(IntervalAttemptErrorV2::InvalidInput);
        }
    }
    let mut fixed_face_present = false;
    for face in audit.faces() {
        map_checkpoint_v2(checkpoint)?;
        if *face == fixed_face {
            fixed_face_present = true;
            break;
        }
    }
    if !fixed_face_present {
        return Err(IntervalAttemptErrorV2::InvalidInput);
    }
    for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
        map_checkpoint_v2(checkpoint)?;
        let hinge = geometry
            .hinges()
            .get(geometry_index)
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        if angle_boxes.get(position).map(|(edge, _)| *edge) != Some(hinge.edge()) {
            return Err(IntervalAttemptErrorV2::InvalidInput);
        }
    }

    let faces = audit.faces().len();
    let mut adjacency = Vec::<Vec<(usize, usize, bool)>>::new();
    adjacency
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    for _ in 0..faces {
        map_checkpoint_v2(checkpoint)?;
        adjacency.push(Vec::new());
    }
    let mut degrees = Vec::<usize>::new();
    degrees
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    for _ in 0..faces {
        map_checkpoint_v2(checkpoint)?;
        degrees.push(0);
    }
    for geometry_index in canonical_hinge_indices.iter().copied() {
        map_checkpoint_v2(checkpoint)?;
        let hinge = &geometry.hinges()[geometry_index];
        if !is_spanning_v2(audit, hinge.edge()) {
            continue;
        }
        let left =
            face_index_v2(audit, hinge.left_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        let right =
            face_index_v2(audit, hinge.right_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        degrees[left] = degrees[left]
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
        degrees[right] = degrees[right]
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    }
    for (neighbors, degree) in adjacency.iter_mut().zip(&degrees) {
        map_checkpoint_v2(checkpoint)?;
        neighbors
            .try_reserve_exact(*degree)
            .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    }
    for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
        map_checkpoint_v2(checkpoint)?;
        let hinge = &geometry.hinges()[geometry_index];
        if !is_spanning_v2(audit, hinge.edge()) {
            continue;
        }
        let left =
            face_index_v2(audit, hinge.left_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        let right =
            face_index_v2(audit, hinge.right_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        adjacency[left].push((right, position, false));
        adjacency[right].push((left, position, true));
    }

    let mut poses = Vec::<Option<IntervalRigidTransformV1>>::new();
    poses
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    for _ in 0..faces {
        map_checkpoint_v2(checkpoint)?;
        poses.push(None);
    }
    let mut queue = Vec::<usize>::new();
    queue
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    let pose_capacity_bytes =
        checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(poses.capacity())
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    let physical_capacity_bytes = checked_interval_physical_capacity_bytes_v2(
        &adjacency,
        adjacency.capacity(),
        degrees.capacity(),
        poses.capacity(),
        queue.capacity(),
        checkpoint,
    )?;
    if physical_capacity_bytes > max_workspace_bytes
        || pose_capacity_bytes > max_pose_capacity_bytes
    {
        return Err(IntervalAttemptErrorV2::ResourceLimit);
    }

    let interval_error = |error| match error {
        crate::OutwardIntervalErrorV1::ResourceLimit => IntervalAttemptErrorV2::ResourceLimit,
        crate::OutwardIntervalErrorV1::InvalidEndpoint
        | crate::OutwardIntervalErrorV1::DivisionByZeroInterval => IntervalAttemptErrorV2::Unproven,
    };
    let fixed_index =
        face_index_v2(audit, fixed_face).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
    poses[fixed_index] = Some(IntervalRigidTransformV1::identity().map_err(interval_error)?);
    queue.push(fixed_index);
    let mut queue_cursor = 0usize;
    let mut visited_faces = 1usize;
    let mut charged = 0usize;
    while let Some(&parent_face) = queue.get(queue_cursor) {
        queue_cursor = queue_cursor
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
        map_checkpoint_v2(checkpoint)?;
        let parent = poses[parent_face].ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        for &(child_face, hinge_position, reverse) in &adjacency[parent_face] {
            map_checkpoint_v2(checkpoint)?;
            if poses[child_face].is_some() {
                continue;
            }
            charged = charged
                .checked_add(1)
                .filter(|value| *value <= max_work)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
            let geometry_index = canonical_hinge_indices[hinge_position];
            let hinge = &geometry.hinges()[geometry_index];
            let degrees = angle_boxes[hinge_position].1;
            let mountain = hinge.assignment() == FoldAssignment::Mountain;
            let sign = if reverse ^ !mountain { -1.0 } else { 1.0 };
            let local = IntervalRigidTransformV1::about_axis_reusing_exact_zero_v2(
                [
                    sign * hinge.axis().x(),
                    sign * hinge.axis().y(),
                    sign * hinge.axis().z(),
                ],
                [hinge.start().x(), hinge.start().y(), hinge.start().z()],
                degrees,
                max_work,
            )
            .map_err(interval_error)?;
            poses[child_face] = Some(
                parent
                    .compose_reusing_exact_identity_v2(local, max_work)
                    .map_err(interval_error)?,
            );
            visited_faces = visited_faces
                .checked_add(1)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
            queue.push(child_face);
        }
    }
    if visited_faces != faces {
        return Err(IntervalAttemptErrorV2::InvalidInput);
    }

    if verification_mode == IntervalClosureVerificationModeV2::FullClosure {
        for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
            map_checkpoint_v2(checkpoint)?;
            charged = charged
                .checked_add(1)
                .filter(|value| *value <= max_work)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
            let hinge = &geometry.hinges()[geometry_index];
            if is_spanning_v2(audit, hinge.edge()) {
                continue;
            }
            let left = poses[face_index_v2(audit, hinge.left_face())
                .ok_or(IntervalAttemptErrorV2::InvalidInput)?]
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
            let right = poses[face_index_v2(audit, hinge.right_face())
                .ok_or(IntervalAttemptErrorV2::InvalidInput)?]
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
            let degrees = angle_boxes[position].1;
            let sign = if hinge.assignment() == FoldAssignment::Mountain {
                1.0
            } else {
                -1.0
            };
            let local = IntervalRigidTransformV1::about_axis_reusing_exact_zero_v2(
                [
                    sign * hinge.axis().x(),
                    sign * hinge.axis().y(),
                    sign * hinge.axis().z(),
                ],
                [hinge.start().x(), hinge.start().y(), hinge.start().z()],
                degrees,
                max_work,
            )
            .map_err(interval_error)?;
            let expected = left
                .compose_reusing_exact_identity_v2(local, max_work)
                .map_err(interval_error)?;
            if !expected.universally_matches_within_reusing_pristine_equality_v2(right, tolerance) {
                return Err(IntervalAttemptErrorV2::Unproven);
            }
        }
    }
    Ok(IntervalAttemptSuccessV2 {
        physical_capacity_bytes,
        poses,
    })
}
