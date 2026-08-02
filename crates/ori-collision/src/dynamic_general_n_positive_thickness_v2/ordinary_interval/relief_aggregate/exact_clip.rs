//! Exact source-plane clipping into finite-strip and shared-vertex remainders.

use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use ori_kinematics::{MaterialHingeGraphGeometry, TreeHinge};

use super::*;

mod numeric;
mod polygon;
use numeric::{ExactMeterV2, sqrt_lower_v2, sqrt_upper_v2};
use polygon::{
    clip_half_plane_v2, dot_v2, exact_face_polygon_v2, shifted_dot_v2, squared_length_v2,
};

pub(super) fn validate_hinge_policy_v2(
    policy: &HingeReliefPolicyRecordV1,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    numeric::validate_hinge_policy_v2(policy, input, resources)
}

pub(super) fn validate_vertex_policy_v2(
    policy: &VertexReliefPolicyRecordV1,
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    numeric::validate_vertex_policy_v2(policy, input, resources)
}

pub(super) type ExactPointV2 = [BigRational; 2];

pub(super) fn verify_hinge_endpoints_v2(
    geometry: &MaterialHingeGraphGeometry,
    hinge: &TreeHinge,
    vertices: [VertexId; 2],
) -> Result<(), ReliefAggregateErrorV2> {
    let positions = [
        geometry
            .vertex_position(vertices[0])
            .ok_or(ReliefAggregateErrorV2::InvalidInput)?,
        geometry
            .vertex_position(vertices[1])
            .ok_or(ReliefAggregateErrorV2::InvalidInput)?,
    ];
    let same = |left: ori_kinematics::Point3, right: ori_kinematics::Point3| {
        [left.x(), left.y(), left.z()]
            .into_iter()
            .zip([right.x(), right.y(), right.z()])
            .all(|(left, right)| left.to_bits() == right.to_bits())
    };
    if !((same(positions[0], hinge.start()) && same(positions[1], hinge.end()))
        || (same(positions[1], hinge.start()) && same(positions[0], hinge.end())))
    {
        return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
    }
    for face in [hinge.left_face(), hinge.right_face()] {
        let boundary = geometry
            .face_boundary_vertices(face)
            .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
        let mut locations = [None, None];
        for (position, candidate) in boundary.iter().enumerate() {
            for (slot, expected) in vertices.iter().enumerate() {
                if candidate == expected && locations[slot].replace(position).is_some() {
                    return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
                }
            }
        }
        let [Some(first), Some(second)] = locations else {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        };
        if (first + 1) % boundary.len() != second && (second + 1) % boundary.len() != first {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        }
    }
    Ok(())
}

pub(super) fn prepare_hinge_cells_v2(
    input: &ReliefAggregateInputV2<'_>,
    pair: OrdinaryIntervalFacePairV2,
    hinge: &TreeHinge,
    policy: &HingeReliefPolicyRecordV1,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(PreparedCellV2, PreparedCellV2), ReliefAggregateErrorV2> {
    let start = hinge.start();
    let end = hinge.end();
    let anchor = [start.x(), start.y(), start.z()];
    let mut meter = ExactMeterV2 { input, resources };
    let a = [meter.value(start.x())?, meter.value(start.z())?];
    let end_x = meter.value(end.x())?;
    let end_z = meter.value(end.z())?;
    let d = [meter.sub(&end_x, &a[0])?, meter.sub(&end_z, &a[1])?];
    let length_squared = squared_length_v2(&d, &mut meter)?;
    let length_lower = sqrt_lower_v2(&length_squared, input, meter.resources, checkpoint)?;
    let width = meter.value(policy.cutout_width_mm)?;
    let threshold = meter.mul(&width, &length_lower)?;
    let base_normal = [-d[1].clone(), d[0].clone()];
    let left = prepare_hinge_cell_v2(
        pair.first,
        anchor,
        &a,
        &d,
        &length_squared,
        &base_normal,
        &threshold,
        input,
        &mut meter,
        checkpoint,
    )?;
    let right = prepare_hinge_cell_v2(
        pair.second,
        anchor,
        &a,
        &d,
        &length_squared,
        &base_normal,
        &threshold,
        input,
        &mut meter,
        checkpoint,
    )?;
    Ok((left, right))
}

#[allow(clippy::too_many_arguments)]
fn prepare_hinge_cell_v2(
    face: FaceId,
    anchor: [f64; 3],
    line_origin: &ExactPointV2,
    hinge_axis: &ExactPointV2,
    hinge_axis_squared: &BigRational,
    base_normal: &ExactPointV2,
    threshold: &BigRational,
    input: &ReliefAggregateInputV2<'_>,
    meter: &mut ExactMeterV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<PreparedCellV2, ReliefAggregateErrorV2> {
    let polygon = exact_face_polygon_v2(face, input, meter, checkpoint)?;
    let mut side = None;
    for point in &polygon {
        relief_checkpoint_v2(checkpoint)?;
        let axial = shifted_dot_v2(point, line_origin, hinge_axis, meter)?;
        if !finite_hinge_axial_position_v2(&axial, hinge_axis_squared) {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        }
        let signed = shifted_dot_v2(point, line_origin, base_normal, meter)?;
        if signed.is_zero() {
            continue;
        }
        let positive = signed > BigRational::zero();
        if side.is_some_and(|known| known != positive) {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        }
        side = Some(positive);
    }
    let side = side.ok_or(ReliefAggregateErrorV2::UnsupportedSharedTopology)?;
    let normal = if side {
        base_normal.clone()
    } else {
        [-base_normal[0].clone(), -base_normal[1].clone()]
    };
    let clipped = clip_half_plane_v2(&polygon, line_origin, &normal, threshold, meter, checkpoint)?;
    cell_from_exact_v2(face, anchor, &normal, clipped, input, meter, checkpoint)
}

fn finite_hinge_axial_position_v2(axial: &BigRational, hinge_axis_squared: &BigRational) -> bool {
    axial >= &BigRational::zero() && axial <= hinge_axis_squared
}

#[cfg(test)]
pub(super) fn finite_hinge_axial_position_for_test_v2(axial: i64, hinge_axis_squared: i64) -> bool {
    finite_hinge_axial_position_v2(
        &BigRational::from_integer(axial.into()),
        &BigRational::from_integer(hinge_axis_squared.into()),
    )
}

pub(super) fn prepare_vertex_cells_v2(
    input: &ReliefAggregateInputV2<'_>,
    pair: OrdinaryIntervalFacePairV2,
    vertex: VertexId,
    policy: &VertexReliefPolicyRecordV1,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(PreparedCellV2, PreparedCellV2), ReliefAggregateErrorV2> {
    let origin = input
        .ordinary
        .geometry
        .vertex_position(vertex)
        .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
    let anchor = [origin.x(), origin.y(), origin.z()];
    let mut meter = ExactMeterV2 { input, resources };
    let exact_origin = [meter.value(origin.x())?, meter.value(origin.z())?];
    let left = prepare_vertex_cell_v2(
        pair.first,
        vertex,
        anchor,
        &exact_origin,
        policy.cutout_radius_mm,
        input,
        &mut meter,
        checkpoint,
    )?;
    let right = prepare_vertex_cell_v2(
        pair.second,
        vertex,
        anchor,
        &exact_origin,
        policy.cutout_radius_mm,
        input,
        &mut meter,
        checkpoint,
    )?;
    Ok((left, right))
}

#[allow(clippy::too_many_arguments)]
fn prepare_vertex_cell_v2(
    face: FaceId,
    vertex: VertexId,
    anchor: [f64; 3],
    origin: &ExactPointV2,
    radius: f64,
    input: &ReliefAggregateInputV2<'_>,
    meter: &mut ExactMeterV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<PreparedCellV2, ReliefAggregateErrorV2> {
    let boundary = input
        .ordinary
        .geometry
        .face_boundary_vertices(face)
        .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
    let mut occurrences = boundary.iter().enumerate().filter(|(_, id)| **id == vertex);
    let pivot = occurrences
        .next()
        .map(|(index, _)| index)
        .ok_or(ReliefAggregateErrorV2::UnsupportedSharedTopology)?;
    if occurrences.next().is_some() {
        return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
    }
    let polygon = exact_face_polygon_v2(face, input, meter, checkpoint)?;
    let previous = &polygon[(pivot + polygon.len() - 1) % polygon.len()];
    let next = &polygon[(pivot + 1) % polygon.len()];
    let d_previous = [
        meter.sub(&previous[0], &origin[0])?,
        meter.sub(&previous[1], &origin[1])?,
    ];
    let d_next = [
        meter.sub(&next[0], &origin[0])?,
        meter.sub(&next[1], &origin[1])?,
    ];
    let previous_squared = squared_length_v2(&d_previous, meter)?;
    let next_squared = squared_length_v2(&d_next, meter)?;
    let previous_upper = sqrt_upper_v2(&previous_squared, input, meter.resources, checkpoint)?;
    let next_upper = sqrt_upper_v2(&next_squared, input, meter.resources, checkpoint)?;
    let previous_x = meter.div(&d_previous[0], &previous_squared)?;
    let next_x = meter.div(&d_next[0], &next_squared)?;
    let previous_z = meter.div(&d_previous[1], &previous_squared)?;
    let next_z = meter.div(&d_next[1], &next_squared)?;
    let normal = [
        meter.add(&previous_x, &next_x)?,
        meter.add(&previous_z, &next_z)?,
    ];
    let previous_dot = dot_v2(&normal, &d_previous, meter)?;
    let next_dot = dot_v2(&normal, &d_next, meter)?;
    if previous_dot <= BigRational::zero() || next_dot <= BigRational::zero() {
        return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
    }
    let radius = meter.value(radius)?;
    let previous_scaled = meter.mul(&radius, &previous_dot)?;
    let next_scaled = meter.mul(&radius, &next_dot)?;
    let previous_threshold = meter.div(&previous_scaled, &previous_upper)?;
    let next_threshold = meter.div(&next_scaled, &next_upper)?;
    let threshold = std::cmp::min(previous_threshold, next_threshold);
    if threshold <= BigRational::zero() {
        return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
    }
    let clipped = clip_half_plane_v2(&polygon, origin, &normal, &threshold, meter, checkpoint)?;
    cell_from_exact_v2(face, anchor, &normal, clipped, input, meter, checkpoint)
}

fn cell_from_exact_v2(
    face: FaceId,
    anchor: [f64; 3],
    normal: &ExactPointV2,
    polygon: Vec<ExactPointV2>,
    input: &ReliefAggregateInputV2<'_>,
    meter: &mut ExactMeterV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<PreparedCellV2, ReliefAggregateErrorV2> {
    resources::charge_v2(
        &mut meter.resources.carrier_conversion_work,
        2,
        input.limits.max_carrier_conversion_work,
    )?;
    let support_axis = [
        normal[0]
            .to_f64()
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
        0.0,
        normal[1]
            .to_f64()
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    ];
    if support_axis.iter().any(|value| !value.is_finite())
        || support_axis[0] == 0.0 && support_axis[2] == 0.0
    {
        return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
    }
    let mut ring = Vec::new();
    ring.try_reserve_exact(polygon.len())
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
    if ring.capacity() > polygon.len() {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    for point in polygon {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut meter.resources.carrier_conversion_work,
            2,
            input.limits.max_carrier_conversion_work,
        )?;
        let x = ori_numeric::rational_interval_to_f64_outward(&point[0], &point[0])
            .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
        let z = ori_numeric::rational_interval_to_f64_outward(&point[1], &point[1])
            .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
        ring.push([
            OutwardIntervalV1::new(x.lower(), x.upper()).map_err(map_interval_error_v2)?,
            OutwardIntervalV1::new(z.lower(), z.upper()).map_err(map_interval_error_v2)?,
        ]);
    }
    Ok(PreparedCellV2 {
        face,
        anchor,
        support_axis,
        ring,
    })
}
