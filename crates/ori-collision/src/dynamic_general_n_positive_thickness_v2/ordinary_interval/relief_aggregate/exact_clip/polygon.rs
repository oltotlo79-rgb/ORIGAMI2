//! Strict-simple-convex validation and exact half-plane clipping.

use num_rational::BigRational;
use num_traits::Zero;

use super::*;

pub(super) fn exact_face_polygon_v2(
    face: FaceId,
    input: &ReliefAggregateInputV2<'_>,
    meter: &mut ExactMeterV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<Vec<ExactPointV2>, ReliefAggregateErrorV2> {
    let boundary = input
        .ordinary
        .geometry
        .face_boundary_vertices(face)
        .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
    if boundary.len() < 3 || boundary.len() > input.limits.max_rest_carrier_vertices {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    let mut polygon = Vec::new();
    polygon
        .try_reserve_exact(boundary.len())
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
    if polygon.capacity() > boundary.len() {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    for (index, vertex) in boundary.iter().enumerate() {
        relief_checkpoint_v2(checkpoint)?;
        if boundary[..index].contains(vertex) {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        }
        let point = input
            .ordinary
            .geometry
            .vertex_position(*vertex)
            .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
        polygon.push([meter.value(point.x())?, meter.value(point.z())?]);
    }
    validate_strict_simple_convex_v2(&polygon, input, meter, checkpoint)?;
    Ok(polygon)
}

fn validate_strict_simple_convex_v2(
    polygon: &[ExactPointV2],
    input: &ReliefAggregateInputV2<'_>,
    meter: &mut ExactMeterV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), ReliefAggregateErrorV2> {
    let mut orientation = None;
    for index in 0..polygon.len() {
        relief_checkpoint_v2(checkpoint)?;
        let cross = orient_v2(
            &polygon[index],
            &polygon[(index + 1) % polygon.len()],
            &polygon[(index + 2) % polygon.len()],
            meter,
        )?;
        if cross.is_zero() {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        }
        let positive = cross > BigRational::zero();
        if orientation.is_some_and(|known| known != positive) {
            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
        }
        orientation = Some(positive);
    }
    for first in 0..polygon.len() {
        for second in first + 1..polygon.len() {
            relief_checkpoint_v2(checkpoint)?;
            if second == first + 1 || (first == 0 && second + 1 == polygon.len()) {
                continue;
            }
            resources::charge_v2(
                &mut meter.resources.convexity_segment_tests,
                1,
                input.limits.max_convexity_segment_tests,
            )?;
            let a = &polygon[first];
            let b = &polygon[(first + 1) % polygon.len()];
            let c = &polygon[second];
            let d = &polygon[(second + 1) % polygon.len()];
            let ab_c = orient_v2(a, b, c, meter)?;
            let ab_d = orient_v2(a, b, d, meter)?;
            let cd_a = orient_v2(c, d, a, meter)?;
            let cd_b = orient_v2(c, d, b, meter)?;
            let same_side = |left: &BigRational, right: &BigRational| {
                (!left.is_zero() && !right.is_zero())
                    && ((left > &BigRational::zero()) == (right > &BigRational::zero()))
            };
            if !same_side(&ab_c, &ab_d) && !same_side(&cd_a, &cd_b) {
                return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn clip_half_plane_v2(
    polygon: &[ExactPointV2],
    origin: &ExactPointV2,
    normal: &ExactPointV2,
    threshold: &BigRational,
    meter: &mut ExactMeterV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<Vec<ExactPointV2>, ReliefAggregateErrorV2> {
    let capacity = polygon
        .len()
        .checked_add(1)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
    if output.capacity() > capacity {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    let mut previous = polygon.last().unwrap();
    let mut previous_value = shifted_dot_v2(previous, origin, normal, meter)?;
    let mut previous_inside = previous_value >= *threshold;
    for current in polygon {
        relief_checkpoint_v2(checkpoint)?;
        let current_value = shifted_dot_v2(current, origin, normal, meter)?;
        let current_inside = current_value >= *threshold;
        if previous_inside != current_inside {
            let numerator = meter.sub(threshold, &previous_value)?;
            let denominator = meter.sub(&current_value, &previous_value)?;
            let t = meter.div(&numerator, &denominator)?;
            if t < BigRational::zero() || t > BigRational::from_integer(1.into()) {
                return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
            }
            let dx = meter.sub(&current[0], &previous[0])?;
            let dz = meter.sub(&current[1], &previous[1])?;
            let x_step = meter.mul(&t, &dx)?;
            let z_step = meter.mul(&t, &dz)?;
            output.push([
                meter.add(&previous[0], &x_step)?,
                meter.add(&previous[1], &z_step)?,
            ]);
        }
        if current_inside {
            output.push(current.clone());
        }
        previous = current;
        previous_value = current_value;
        previous_inside = current_inside;
    }
    if output.len() < 3 {
        return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
    }
    let mut twice_area = BigRational::zero();
    for index in 0..output.len() {
        relief_checkpoint_v2(checkpoint)?;
        let left = meter.mul(&output[index][0], &output[(index + 1) % output.len()][1])?;
        let right = meter.mul(&output[index][1], &output[(index + 1) % output.len()][0])?;
        let cross = meter.sub(&left, &right)?;
        twice_area = meter.add(&twice_area, &cross)?;
    }
    if twice_area.is_zero() {
        return Err(ReliefAggregateErrorV2::UnprovenSharedRelief);
    }
    Ok(output)
}

pub(super) fn squared_length_v2(
    vector: &ExactPointV2,
    meter: &mut ExactMeterV2<'_>,
) -> Result<BigRational, ReliefAggregateErrorV2> {
    let x = meter.mul(&vector[0], &vector[0])?;
    let z = meter.mul(&vector[1], &vector[1])?;
    let value = meter.add(&x, &z)?;
    if value <= BigRational::zero() {
        return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
    }
    Ok(value)
}

pub(super) fn shifted_dot_v2(
    point: &ExactPointV2,
    origin: &ExactPointV2,
    normal: &ExactPointV2,
    meter: &mut ExactMeterV2<'_>,
) -> Result<BigRational, ReliefAggregateErrorV2> {
    let shifted = [
        meter.sub(&point[0], &origin[0])?,
        meter.sub(&point[1], &origin[1])?,
    ];
    dot_v2(normal, &shifted, meter)
}

pub(super) fn dot_v2(
    left: &ExactPointV2,
    right: &ExactPointV2,
    meter: &mut ExactMeterV2<'_>,
) -> Result<BigRational, ReliefAggregateErrorV2> {
    let x = meter.mul(&left[0], &right[0])?;
    let z = meter.mul(&left[1], &right[1])?;
    meter.add(&x, &z)
}

fn orient_v2(
    a: &ExactPointV2,
    b: &ExactPointV2,
    c: &ExactPointV2,
    meter: &mut ExactMeterV2<'_>,
) -> Result<BigRational, ReliefAggregateErrorV2> {
    let ab = [meter.sub(&b[0], &a[0])?, meter.sub(&b[1], &a[1])?];
    let ac = [meter.sub(&c[0], &a[0])?, meter.sub(&c[1], &a[1])?];
    let left = meter.mul(&ab[0], &ac[1])?;
    let right = meter.mul(&ab[1], &ac[0])?;
    meter.sub(&left, &right)
}
