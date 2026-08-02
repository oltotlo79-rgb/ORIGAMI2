//! Exact XZ predicates and shared-contact classification for parent admission.

use super::*;

fn orientation_v2<F>(
    first: &ExactPointV2,
    second: &ExactPointV2,
    third: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<Ordering, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    meter.step(1, 7, checkpoint)?;
    let determinant = (&second.x - &first.x) * (&third.z - &first.z)
        - (&second.z - &first.z) * (&third.x - &first.x);
    Ok(determinant.cmp(&BigRational::zero()))
}

pub(super) fn point_on_segment_v2<F>(
    point: &ExactPointV2,
    first: &ExactPointV2,
    second: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    if orientation_v2(first, second, point, meter, checkpoint)? != Ordering::Equal {
        return Ok(false);
    }
    meter.step(1, 4, checkpoint)?;
    Ok(between_inclusive_v2(&point.x, &first.x, &second.x)
        && between_inclusive_v2(&point.z, &first.z, &second.z))
}

fn between_inclusive_v2(value: &BigRational, first: &BigRational, second: &BigRational) -> bool {
    if first <= second {
        first <= value && value <= second
    } else {
        second <= value && value <= first
    }
}

pub(super) fn point_strictly_outside_segment_bounds_v2<F>(
    point: &ExactPointV2,
    first: &ExactPointV2,
    second: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    meter.step(1, 4, checkpoint)?;
    Ok(!between_inclusive_v2(&point.x, &first.x, &second.x)
        || !between_inclusive_v2(&point.z, &first.z, &second.z))
}

pub(super) fn segment_bounds_strictly_disjoint_v2<F>(
    a: &ExactPointV2,
    b: &ExactPointV2,
    c: &ExactPointV2,
    d: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    meter.step(1, 8, checkpoint)?;
    let (ab_min_x, ab_max_x) = if a.x <= b.x {
        (&a.x, &b.x)
    } else {
        (&b.x, &a.x)
    };
    let (ab_min_z, ab_max_z) = if a.z <= b.z {
        (&a.z, &b.z)
    } else {
        (&b.z, &a.z)
    };
    let (cd_min_x, cd_max_x) = if c.x <= d.x {
        (&c.x, &d.x)
    } else {
        (&d.x, &c.x)
    };
    let (cd_min_z, cd_max_z) = if c.z <= d.z {
        (&c.z, &d.z)
    } else {
        (&d.z, &c.z)
    };
    Ok(ab_max_x < cd_min_x || cd_max_x < ab_min_x || ab_max_z < cd_min_z || cd_max_z < ab_min_z)
}

pub(super) fn segments_intersect_closed_v2<F>(
    a: &ExactPointV2,
    b: &ExactPointV2,
    c: &ExactPointV2,
    d: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let abc = orientation_v2(a, b, c, meter, checkpoint)?;
    let abd = orientation_v2(a, b, d, meter, checkpoint)?;
    let cda = orientation_v2(c, d, a, meter, checkpoint)?;
    let cdb = orientation_v2(c, d, b, meter, checkpoint)?;
    if abc == Ordering::Equal && point_on_segment_v2(c, a, b, meter, checkpoint)? {
        return Ok(true);
    }
    if abd == Ordering::Equal && point_on_segment_v2(d, a, b, meter, checkpoint)? {
        return Ok(true);
    }
    if cda == Ordering::Equal && point_on_segment_v2(a, c, d, meter, checkpoint)? {
        return Ok(true);
    }
    if cdb == Ordering::Equal && point_on_segment_v2(b, c, d, meter, checkpoint)? {
        return Ok(true);
    }
    Ok(abc != abd && cda != cdb)
}

pub(super) fn edges_share_vertex_v2(first: GraphEdgeV2, second: GraphEdgeV2) -> bool {
    first.first == second.first
        || first.first == second.second
        || first.second == second.first
        || first.second == second.second
}

pub(super) fn validate_exact_face_geometry_v2<F>(
    face: &FaceRecordV2,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut signed_double_area = BigRational::zero();
    for index in 0..face.boundary.len() {
        meter.step(1, 4, checkpoint)?;
        let first = &vertices[face.boundary_indices[index]].point;
        let second =
            &vertices[face.boundary_indices[(index + 1) % face.boundary_indices.len()]].point;
        signed_double_area += &first.x * &second.z - &first.z * &second.x;
    }
    // Source-XY CCW becomes XZ clockwise under (x, 0, -y).
    if signed_double_area >= BigRational::zero() {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
        );
    }
    for first_index in 0..face.boundary.len() {
        for second_index in first_index + 1..face.boundary.len() {
            meter.step(1, 0, checkpoint)?;
            let adjacent = second_index == first_index + 1
                || (first_index == 0 && second_index + 1 == face.boundary.len());
            if adjacent {
                continue;
            }
            let a = &vertices[face.boundary_indices[first_index]].point;
            let b = &vertices
                [face.boundary_indices[(first_index + 1) % face.boundary_indices.len()]]
            .point;
            let c = &vertices[face.boundary_indices[second_index]].point;
            let d = &vertices
                [face.boundary_indices[(second_index + 1) % face.boundary_indices.len()]]
            .point;
            if segments_intersect_closed_v2(a, b, c, d, meter, checkpoint)? {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
        }
    }
    Ok(())
}

pub(super) fn validate_exact_hinge_axis_v2<F>(
    start: &ExactPointV2,
    end: &ExactPointV2,
    axis: Point3,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    meter.step(1, 12, checkpoint)?;
    let axis_x = BigRational::from_float(axis.x())
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
    let axis_z = BigRational::from_float(axis.z())
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
    let delta_x = &end.x - &start.x;
    let delta_z = &end.z - &start.z;
    let cross = &delta_x * &axis_z - &delta_z * &axis_x;
    let dot = delta_x * axis_x + delta_z * axis_z;
    if !cross.is_zero() || dot <= BigRational::zero() {
        return Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput);
    }
    Ok(())
}

pub(super) fn canonical_face_pair_v2(first: FaceId, second: FaceId) -> (FaceId, FaceId) {
    if first.canonical_bytes() < second.canonical_bytes() {
        (first, second)
    } else {
        (second, first)
    }
}

pub(super) fn shared_face_edge_key_v2(edge: SharedFaceEdgeV2) -> ([u8; 16], [u8; 16]) {
    (
        edge.first_face.canonical_bytes(),
        edge.second_face.canonical_bytes(),
    )
}

pub(super) fn find_shared_face_edge_v2<F>(
    first: FaceId,
    second: FaceId,
    shared_edges: &[SharedFaceEdgeV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<Option<GraphEdgeV2>, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let (first, second) = canonical_face_pair_v2(first, second);
    let target = (first.canonical_bytes(), second.canonical_bytes());
    let mut lower = 0usize;
    let mut upper = shared_edges.len();
    while lower < upper {
        meter.step(1, 0, checkpoint)?;
        let middle = lower + (upper - lower) / 2;
        match shared_face_edge_key_v2(shared_edges[middle]).cmp(&target) {
            Ordering::Less => lower = middle + 1,
            Ordering::Greater => upper = middle,
            Ordering::Equal => return Ok(Some(shared_edges[middle].edge)),
        }
    }
    Ok(None)
}

pub(super) fn validate_face_pair_shared_features_v2<F>(
    first: &FaceRecordV2,
    second: &FaceRecordV2,
    shared_edge: Option<GraphEdgeV2>,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut shared_vertex_count = 0usize;
    let mut shared_vertex = None;
    for first_vertex in &first.boundary {
        for second_vertex in &second.boundary {
            meter.step(1, 0, checkpoint)?;
            if first_vertex != second_vertex {
                continue;
            }
            shared_vertex_count = shared_vertex_count.checked_add(1).ok_or(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit,
            )?;
            shared_vertex = Some(*first_vertex);
            match shared_edge {
                Some(edge) if *first_vertex != edge.first && *first_vertex != edge.second => {
                    return Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                    );
                }
                Some(_) if shared_vertex_count > 2 => {
                    return Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                    );
                }
                None if shared_vertex_count > 1 => {
                    return Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                    );
                }
                _ => {}
            }
        }
    }
    if shared_edge.is_some() && shared_vertex_count != 2 {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
        );
    }
    if shared_edge.is_none()
        && let Some(vertex) = shared_vertex
        && shared_vertex_wedges_overlap_v2(first, second, vertex, vertices, meter, checkpoint)?
    {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
        );
    }
    Ok(())
}

pub(super) fn validate_adjacent_face_half_planes_v2<F>(
    first_face: &FaceRecordV2,
    second_face: &FaceRecordV2,
    edge: GraphEdgeV2,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let edge_start = &vertices[edge.first_index].point;
    let edge_end = &vertices[edge.second_index].point;
    for face in [first_face, second_face] {
        let forward = if face.face == edge.first_face {
            edge.first_forward
        } else if edge.second_face == Some(face.face) {
            edge.second_forward
        } else {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        };
        let expected = if forward {
            Ordering::Less
        } else {
            Ordering::Greater
        };
        let mut occurrence = None;
        for index in 0..face.boundary.len() {
            meter.step(1, 0, checkpoint)?;
            let start = face.boundary[index];
            let end = face.boundary[(index + 1) % face.boundary.len()];
            if (start == edge.first && end == edge.second)
                || (start == edge.second && end == edge.first)
            {
                if occurrence.is_some() {
                    return Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
                    );
                }
                occurrence = Some(index);
            }
        }
        let occurrence = occurrence
            .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
        if (face.boundary[occurrence] == edge.first) != forward {
            return Err(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput,
            );
        }
        // Only the local rays own the incident interior; remote concave wrap
        // is handled by complete intersection and containment below.
        for walk_forward in [false, true] {
            let mut found_noncollinear = false;
            for offset in 1..face.boundary.len() - 1 {
                meter.step(1, 0, checkpoint)?;
                let index = if walk_forward {
                    (occurrence + 1 + offset) % face.boundary.len()
                } else {
                    (occurrence + face.boundary.len() - offset) % face.boundary.len()
                };
                let side = orientation_v2(
                    edge_start,
                    edge_end,
                    &vertices[face.boundary_indices[index]].point,
                    meter,
                    checkpoint,
                )?;
                if side == Ordering::Equal {
                    continue;
                }
                if side != expected {
                    return Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                    );
                }
                found_noncollinear = true;
                break;
            }
            if !found_noncollinear {
                return Err(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
                );
            }
        }
    }
    Ok(())
}

pub(super) fn shared_vertex_wedges_overlap_v2<F>(
    first: &FaceRecordV2,
    second: &FaceRecordV2,
    shared: VertexId,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let (first_start, first_end) =
        local_clockwise_interior_arc_v2(first, shared, vertices, meter, checkpoint)?;
    let (second_start, second_end) =
        local_clockwise_interior_arc_v2(second, shared, vertices, meter, checkpoint)?;
    open_ccw_arcs_overlap_v2(
        &first_start,
        &first_end,
        &second_start,
        &second_end,
        meter,
        checkpoint,
    )
}

fn local_clockwise_interior_arc_v2<F>(
    face: &FaceRecordV2,
    shared: VertexId,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<
    (ExactPointV2, ExactPointV2),
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut shared_index = None;
    for (index, vertex) in face.boundary.iter().enumerate() {
        meter.step(1, 0, checkpoint)?;
        if *vertex == shared {
            shared_index = Some(index);
            break;
        }
    }
    let index = shared_index
        .ok_or(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)?;
    let center = &vertices[face.boundary_indices[index]].point;
    let previous = &vertices[face.boundary_indices
        [(index + face.boundary_indices.len() - 1) % face.boundary_indices.len()]]
    .point;
    let next = &vertices[face.boundary_indices[(index + 1) % face.boundary_indices.len()]].point;
    meter.step(1, 4, checkpoint)?;
    let start = ExactPointV2 {
        x: &previous.x - &center.x,
        z: &previous.z - &center.z,
    };
    let end = ExactPointV2 {
        x: &next.x - &center.x,
        z: &next.z - &center.z,
    };
    if (start.x.is_zero() && start.z.is_zero()) || (end.x.is_zero() && end.z.is_zero()) {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
        );
    }
    if polar_direction_cmp_v2(&start, &end, meter, checkpoint)? == Ordering::Equal {
        return Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection,
        );
    }
    // A clockwise XZ face owns the open CCW arc from predecessor to successor,
    // including the >pi arc at a reflex vertex.
    Ok((start, end))
}

fn open_ccw_arcs_overlap_v2<F>(
    first_start: &ExactPointV2,
    first_end: &ExactPointV2,
    second_start: &ExactPointV2,
    second_end: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let same_start =
        polar_direction_cmp_v2(first_start, second_start, meter, checkpoint)? == Ordering::Equal;
    let same_end =
        polar_direction_cmp_v2(first_end, second_end, meter, checkpoint)? == Ordering::Equal;
    if same_start && same_end {
        return Ok(true);
    }
    Ok(direction_strictly_in_open_ccw_arc_v2(
        second_start,
        first_start,
        first_end,
        meter,
        checkpoint,
    )? || direction_strictly_in_open_ccw_arc_v2(
        second_end,
        first_start,
        first_end,
        meter,
        checkpoint,
    )? || direction_strictly_in_open_ccw_arc_v2(
        first_start,
        second_start,
        second_end,
        meter,
        checkpoint,
    )? || direction_strictly_in_open_ccw_arc_v2(
        first_end,
        second_start,
        second_end,
        meter,
        checkpoint,
    )?)
}

fn direction_strictly_in_open_ccw_arc_v2<F>(
    direction: &ExactPointV2,
    start: &ExactPointV2,
    end: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let start_end = polar_direction_cmp_v2(start, end, meter, checkpoint)?;
    let start_direction = polar_direction_cmp_v2(start, direction, meter, checkpoint)?;
    let direction_end = polar_direction_cmp_v2(direction, end, meter, checkpoint)?;
    Ok(match start_end {
        Ordering::Less => start_direction == Ordering::Less && direction_end == Ordering::Less,
        Ordering::Greater => start_direction == Ordering::Less || direction_end == Ordering::Less,
        Ordering::Equal => false,
    })
}

fn polar_direction_cmp_v2<F>(
    first: &ExactPointV2,
    second: &ExactPointV2,
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<Ordering, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    meter.step(1, 5, checkpoint)?;
    let first_upper =
        first.z > BigRational::zero() || (first.z.is_zero() && first.x >= BigRational::zero());
    let second_upper =
        second.z > BigRational::zero() || (second.z.is_zero() && second.x >= BigRational::zero());
    if first_upper != second_upper {
        return Ok(if first_upper {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let cross = &first.x * &second.z - &first.z * &second.x;
    Ok(match cross.cmp(&BigRational::zero()) {
        Ordering::Greater => Ordering::Less,
        Ordering::Less => Ordering::Greater,
        Ordering::Equal => Ordering::Equal,
    })
}

pub(super) fn face_bounds_strictly_disjoint_v2<F>(
    first: &FaceRecordV2,
    second: &FaceRecordV2,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    meter.step(1, 4, checkpoint)?;
    Ok(
        vertices[first.bounds_indices[1]].point.x < vertices[second.bounds_indices[0]].point.x
            || vertices[second.bounds_indices[1]].point.x
                < vertices[first.bounds_indices[0]].point.x
            || vertices[first.bounds_indices[3]].point.z
                < vertices[second.bounds_indices[2]].point.z
            || vertices[second.bounds_indices[3]].point.z
                < vertices[first.bounds_indices[2]].point.z,
    )
}

pub(super) fn face_has_strictly_contained_vertex_v2<F>(
    candidate_vertices: &FaceRecordV2,
    container: &FaceRecordV2,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<bool, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    for index in &candidate_vertices.boundary_indices {
        meter.step(1, 0, checkpoint)?;
        let point = &vertices[*index].point;
        if point_location_in_face_v2(point, container, vertices, meter, checkpoint)?
            == PointLocationV2::Inside
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointLocationV2 {
    Outside,
    Boundary,
    Inside,
}

fn point_location_in_face_v2<F>(
    point: &ExactPointV2,
    face: &FaceRecordV2,
    vertices: &[ExactVertexV2],
    meter: &mut AdmissionMeterV2,
    checkpoint: &mut F,
) -> Result<PointLocationV2, CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2>
where
    F: FnMut() -> Result<(), CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut winding = 0i64;
    for index in 0..face.boundary.len() {
        meter.count_point_in_polygon_edge(checkpoint)?;
        let first = &vertices[face.boundary_indices[index]].point;
        let second =
            &vertices[face.boundary_indices[(index + 1) % face.boundary_indices.len()]].point;
        if point_on_segment_v2(point, first, second, meter, checkpoint)? {
            return Ok(PointLocationV2::Boundary);
        }
        meter.step(1, 2, checkpoint)?;
        if first.z <= point.z {
            if second.z > point.z
                && orientation_v2(first, second, point, meter, checkpoint)? == Ordering::Greater
            {
                winding = winding.checked_add(1).ok_or(
                    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit,
                )?;
            }
        } else if second.z <= point.z
            && orientation_v2(first, second, point, meter, checkpoint)? == Ordering::Less
        {
            winding = winding.checked_sub(1).ok_or(
                CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit,
            )?;
        }
    }
    Ok(if winding == 0 {
        PointLocationV2::Outside
    } else {
        PointLocationV2::Inside
    })
}
