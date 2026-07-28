use num_rational::BigRational;
use num_traits::{FromPrimitive, Zero};
use ori_domain::{FaceId, VertexId};
use ori_kinematics::{MaterialHingeGraphGeometry, Point3};
use sha2::{Digest, Sha256};

use super::{
    SplitHingeUnionExteriorReliefAssumptionErrorV1 as ErrorV1,
    SplitHingeUnionExteriorReliefAssumptionLimitsV1 as LimitsV1,
};

const SHA256_SCRATCH_BYTES_V1: usize = 104;
const EXACT_SCRATCH_BYTES_V1: usize = 256 * 1024;

pub(super) struct ExactCorridorGeometryV1 {
    pub(super) boundary_hash: [u8; 32],
    pub(super) line_length_squared: BigRational,
    pub(super) radial_squared_line_length_squared: BigRational,
}

#[derive(Debug)]
pub(super) struct MeterV1 {
    limits: LimitsV1,
    work: usize,
    retained_bytes: usize,
    temporary_bytes: usize,
    peak_bytes: usize,
    maximum_exact_bits: u64,
    total_exact_bits: u64,
}

impl MeterV1 {
    pub(super) const fn new(limits: LimitsV1) -> Self {
        Self {
            limits,
            work: 0,
            retained_bytes: 0,
            temporary_bytes: 0,
            peak_bytes: 0,
            maximum_exact_bits: 0,
            total_exact_bits: 0,
        }
    }

    pub(super) fn charge_work(&mut self, amount: usize) -> Result<(), ErrorV1> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(ErrorV1::ResourceLimit)?;
        if self.work > self.limits.max_work {
            return Err(ErrorV1::ResourceLimit);
        }
        Ok(())
    }

    pub(super) fn retain(&mut self, amount: usize) -> Result<(), ErrorV1> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(amount)
            .ok_or(ErrorV1::ResourceLimit)?;
        if self.retained_bytes > self.limits.max_retained_bytes {
            return Err(ErrorV1::ResourceLimit);
        }
        self.update_peak()
    }

    pub(super) fn begin_temporary(&mut self, amount: usize) -> Result<(), ErrorV1> {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_add(amount)
            .ok_or(ErrorV1::ResourceLimit)?;
        self.update_peak()
    }

    pub(super) fn end_temporary(&mut self, amount: usize) {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_sub(amount)
            .expect("union-exterior temporary accounting must balance");
    }

    pub(super) fn observe_exact(&mut self, value: &BigRational) -> Result<(), ErrorV1> {
        let numerator_bits = value.numer().bits();
        let denominator_bits = value.denom().bits();
        if numerator_bits > self.limits.max_exact_bits_per_rational
            || denominator_bits > self.limits.max_exact_bits_per_rational
        {
            return Err(ErrorV1::ResourceLimit);
        }
        self.maximum_exact_bits = self
            .maximum_exact_bits
            .max(numerator_bits)
            .max(denominator_bits);
        let bits = numerator_bits
            .checked_add(denominator_bits)
            .ok_or(ErrorV1::ResourceLimit)?;
        self.total_exact_bits = self
            .total_exact_bits
            .checked_add(bits)
            .ok_or(ErrorV1::ResourceLimit)?;
        if self.total_exact_bits > self.limits.max_total_exact_bits {
            return Err(ErrorV1::ResourceLimit);
        }
        self.charge_work(usize::try_from(bits).map_err(|_| ErrorV1::ResourceLimit)?)
    }

    pub(super) const fn work_used(&self) -> usize {
        self.work
    }

    pub(super) const fn retained_storage_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub(super) const fn peak_storage_bytes(&self) -> usize {
        self.peak_bytes
    }

    pub(super) const fn total_exact_bits(&self) -> u64 {
        self.total_exact_bits
    }

    pub(super) const fn maximum_exact_bits(&self) -> u64 {
        self.maximum_exact_bits
    }

    fn update_peak(&mut self) -> Result<(), ErrorV1> {
        let current = self
            .retained_bytes
            .checked_add(self.temporary_bytes)
            .ok_or(ErrorV1::ResourceLimit)?;
        self.peak_bytes = self.peak_bytes.max(current);
        if self.peak_bytes > self.limits.max_peak_bytes {
            return Err(ErrorV1::ResourceLimit);
        }
        Ok(())
    }
}

pub(super) fn exact_from_f64(value: f64, meter: &mut MeterV1) -> Result<BigRational, ErrorV1> {
    let exact = BigRational::from_f64(value).ok_or(ErrorV1::InvalidBinding)?;
    meter.observe_exact(&exact)?;
    Ok(exact)
}

pub(super) fn exact_add(
    left: &BigRational,
    right: &BigRational,
    meter: &mut MeterV1,
) -> Result<BigRational, ErrorV1> {
    meter.charge_work(1)?;
    let value = left + right;
    meter.observe_exact(&value)?;
    Ok(value)
}

pub(super) fn exact_sub(
    left: &BigRational,
    right: &BigRational,
    meter: &mut MeterV1,
) -> Result<BigRational, ErrorV1> {
    meter.charge_work(1)?;
    let value = left - right;
    meter.observe_exact(&value)?;
    Ok(value)
}

pub(super) fn exact_mul(
    left: &BigRational,
    right: &BigRational,
    meter: &mut MeterV1,
) -> Result<BigRational, ErrorV1> {
    meter.charge_work(1)?;
    let value = left * right;
    meter.observe_exact(&value)?;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_exact_corridor_geometry_v1(
    geometry: &MaterialHingeGraphGeometry,
    pair: [FaceId; 2],
    fixed_face: FaceId,
    lower_bits: [u64; 3],
    upper_bits: [u64; 3],
    radial_depth_bits: u64,
    limits: LimitsV1,
    meter: &mut MeterV1,
) -> Result<ExactCorridorGeometryV1, ErrorV1> {
    if pair[0] == pair[1] || !pair.contains(&fixed_face) {
        return Err(ErrorV1::InvalidBinding);
    }
    let boundaries = [
        geometry
            .face_boundary_vertices(pair[0])
            .ok_or(ErrorV1::BoundaryRegistry)?,
        geometry
            .face_boundary_vertices(pair[1])
            .ok_or(ErrorV1::BoundaryRegistry)?,
    ];
    let total_vertices = boundaries.iter().try_fold(0_usize, |total, boundary| {
        if boundary.len() < 3 {
            return Err(ErrorV1::BoundaryRegistry);
        }
        if boundary.len() > limits.max_boundary_vertices_per_face {
            return Err(ErrorV1::ResourceLimit);
        }
        total
            .checked_add(boundary.len())
            .ok_or(ErrorV1::ResourceLimit)
    })?;
    if total_vertices > limits.max_total_boundary_vertices {
        return Err(ErrorV1::ResourceLimit);
    }

    meter.begin_temporary(EXACT_SCRATCH_BYTES_V1)?;
    let result = scan_exact_corridor_geometry_v1(
        geometry,
        pair,
        lower_bits,
        upper_bits,
        radial_depth_bits,
        boundaries,
        total_vertices,
        meter,
    );
    meter.end_temporary(EXACT_SCRATCH_BYTES_V1);
    result
}

#[allow(clippy::too_many_arguments)]
fn scan_exact_corridor_geometry_v1(
    geometry: &MaterialHingeGraphGeometry,
    pair: [FaceId; 2],
    lower_bits: [u64; 3],
    upper_bits: [u64; 3],
    radial_depth_bits: u64,
    boundaries: [&[VertexId]; 2],
    total_vertices: usize,
    meter: &mut MeterV1,
) -> Result<ExactCorridorGeometryV1, ErrorV1> {
    let lower_point = point_from_bits(lower_bits)?;
    let upper_point = point_from_bits(upper_bits)?;
    let lower = lift_point(lower_point, meter)?;
    let upper = lift_point(upper_point, meter)?;
    let direction = exact_vector_sub(&upper, &lower, meter)?;
    if !direction[1].is_zero() {
        return Err(ErrorV1::SideTopology);
    }
    let line_length_squared = exact_dot(&direction, &direction, meter)?;
    if line_length_squared <= BigRational::zero() {
        return Err(ErrorV1::CorridorEnvelope);
    }
    let radius = exact_from_f64(f64::from_bits(radial_depth_bits), meter)?;
    if radius <= BigRational::zero() {
        return Err(ErrorV1::ReliefInequality);
    }
    let radius_squared = exact_mul(&radius, &radius, meter)?;
    let radial_squared_line_length_squared =
        exact_mul(&radius_squared, &line_length_squared, meter)?;

    meter.begin_temporary(SHA256_SCRATCH_BYTES_V1)?;
    let mut hash = Sha256::new();
    hash_field(
        &mut hash,
        b"split_hinge_union_exterior_relief_boundary_v1",
        meter,
    )?;
    hash_field(&mut hash, &2_u64.to_be_bytes(), meter)?;
    hash_field(
        &mut hash,
        &u64::try_from(total_vertices)
            .map_err(|_| ErrorV1::ResourceLimit)?
            .to_be_bytes(),
        meter,
    )?;

    let mut registry_ok = true;
    let mut corridor_ok = true;
    let mut planar_ok = true;
    let mut saw_lower = [false; 2];
    let mut saw_upper = [false; 2];
    let mut saw_positive = [false; 2];
    let mut saw_negative = [false; 2];

    for (face_index, (face, boundary)) in pair.into_iter().zip(boundaries).enumerate() {
        hash_field(&mut hash, &face.canonical_bytes(), meter)?;
        hash_field(
            &mut hash,
            &u64::try_from(boundary.len())
                .map_err(|_| ErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for vertex in boundary {
            meter.charge_work(1)?;
            hash_field(&mut hash, &vertex.canonical_bytes(), meter)?;
            let Some(point) = geometry.vertex_position(*vertex) else {
                registry_ok = false;
                hash_field(&mut hash, &[0], meter)?;
                continue;
            };
            hash_field(&mut hash, &[1], meter)?;
            let bits = point_bits(point);
            for coordinate in bits {
                hash_field(&mut hash, &coordinate.to_be_bytes(), meter)?;
            }
            saw_lower[face_index] |= bits == lower_bits;
            saw_upper[face_index] |= bits == upper_bits;

            let exact = lift_point(point, meter)?;
            let offset = exact_vector_sub(&exact, &lower, meter)?;
            planar_ok &= offset[1].is_zero();
            let axial = exact_dot(&offset, &direction, meter)?;
            let offset_squared = exact_dot(&offset, &offset, meter)?;
            let axial_squared = exact_mul(&axial, &axial, meter)?;
            let scaled_offset = exact_mul(&line_length_squared, &offset_squared, meter)?;
            let radial_numerator = exact_sub(&scaled_offset, &axial_squared, meter)?;
            meter.charge_work(4)?;
            corridor_ok &= axial >= BigRational::zero()
                && axial <= line_length_squared
                && radial_numerator >= BigRational::zero()
                && radial_numerator <= radial_squared_line_length_squared;

            let z_x = exact_mul(&direction[2], &offset[0], meter)?;
            let x_z = exact_mul(&direction[0], &offset[2], meter)?;
            let side = exact_sub(&z_x, &x_z, meter)?;
            meter.charge_work(2)?;
            saw_positive[face_index] |= side > BigRational::zero();
            saw_negative[face_index] |= side < BigRational::zero();
        }
    }
    let boundary_hash: [u8; 32] = hash.finalize().into();
    meter.end_temporary(SHA256_SCRATCH_BYTES_V1);

    if !registry_ok {
        return Err(ErrorV1::BoundaryRegistry);
    }
    if !planar_ok {
        return Err(ErrorV1::SideTopology);
    }
    if !corridor_ok {
        return Err(ErrorV1::CorridorEnvelope);
    }
    let first_nonpositive = !saw_positive[0] && saw_negative[0];
    let first_nonnegative = !saw_negative[0] && saw_positive[0];
    let second_nonpositive = !saw_positive[1] && saw_negative[1];
    let second_nonnegative = !saw_negative[1] && saw_positive[1];
    if !((first_nonpositive && second_nonnegative) || (first_nonnegative && second_nonpositive)) {
        return Err(ErrorV1::SideTopology);
    }
    if !saw_lower.into_iter().all(|value| value) || !saw_upper.into_iter().all(|value| value) {
        return Err(ErrorV1::AxialCaps);
    }
    meter.charge_work(8)?;
    Ok(ExactCorridorGeometryV1 {
        boundary_hash,
        line_length_squared,
        radial_squared_line_length_squared,
    })
}

fn point_from_bits(bits: [u64; 3]) -> Result<Point3, ErrorV1> {
    Point3::new(
        f64::from_bits(bits[0]),
        f64::from_bits(bits[1]),
        f64::from_bits(bits[2]),
    )
    .map_err(|_| ErrorV1::InvalidBinding)
}

fn point_bits(point: Point3) -> [u64; 3] {
    [
        point.x().to_bits(),
        point.y().to_bits(),
        point.z().to_bits(),
    ]
}

fn lift_point(point: Point3, meter: &mut MeterV1) -> Result<[BigRational; 3], ErrorV1> {
    Ok([
        exact_from_f64(point.x(), meter)?,
        exact_from_f64(point.y(), meter)?,
        exact_from_f64(point.z(), meter)?,
    ])
}

fn exact_vector_sub(
    left: &[BigRational; 3],
    right: &[BigRational; 3],
    meter: &mut MeterV1,
) -> Result<[BigRational; 3], ErrorV1> {
    Ok([
        exact_sub(&left[0], &right[0], meter)?,
        exact_sub(&left[1], &right[1], meter)?,
        exact_sub(&left[2], &right[2], meter)?,
    ])
}

fn exact_dot(
    left: &[BigRational; 3],
    right: &[BigRational; 3],
    meter: &mut MeterV1,
) -> Result<BigRational, ErrorV1> {
    let x = exact_mul(&left[0], &right[0], meter)?;
    let y = exact_mul(&left[1], &right[1], meter)?;
    let z = exact_mul(&left[2], &right[2], meter)?;
    let xy = exact_add(&x, &y, meter)?;
    exact_add(&xy, &z, meter)
}

fn hash_field(hash: &mut Sha256, bytes: &[u8], meter: &mut MeterV1) -> Result<(), ErrorV1> {
    let length = u64::try_from(bytes.len()).map_err(|_| ErrorV1::ResourceLimit)?;
    meter.charge_work(bytes.len().checked_add(8).ok_or(ErrorV1::ResourceLimit)?)?;
    hash.update(length.to_be_bytes());
    hash.update(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitHingeUnionExteriorReliefAssumptionLimitsV1;

    #[test]
    fn meter_checked_arithmetic_fails_closed() {
        let limits = SplitHingeUnionExteriorReliefAssumptionLimitsV1::default();

        let mut meter = MeterV1::new(limits);
        meter.work = usize::MAX;
        assert_eq!(meter.charge_work(1), Err(ErrorV1::ResourceLimit));

        let mut meter = MeterV1::new(limits);
        meter.retained_bytes = usize::MAX;
        assert_eq!(meter.retain(1), Err(ErrorV1::ResourceLimit));

        let mut meter = MeterV1::new(limits);
        meter.temporary_bytes = usize::MAX;
        assert_eq!(meter.begin_temporary(1), Err(ErrorV1::ResourceLimit));

        let mut meter = MeterV1::new(limits);
        meter.retained_bytes = usize::MAX;
        assert_eq!(meter.begin_temporary(1), Err(ErrorV1::ResourceLimit));

        let mut meter = MeterV1::new(limits);
        meter.total_exact_bits = u64::MAX;
        assert_eq!(
            meter.observe_exact(&BigRational::zero()),
            Err(ErrorV1::ResourceLimit)
        );
    }
}
