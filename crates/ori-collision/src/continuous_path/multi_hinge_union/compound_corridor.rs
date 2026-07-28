//! Private Phase-A preflight for one compound logical hinge corridor.
//!
//! This module only canonicalizes already certified local-relief evidence. It
//! does not prove the union exterior clear, classify a collision pair, or
//! authorize any mutation. A caller must still supply a separate continuous
//! separation proof before this internal certificate can participate in an
//! admission decision.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{CanonicalCycleScheduleV1, MaterialHingeGraphGeometry, Point3};
use sha2::{Digest, Sha256};

use super::{
    ID_BYTES, Meter, MultiHingeReliefUnionErrorV2, MultiHingeReliefUnionGapV2,
    MultiHingeReliefUnionLimitsV2, WORD_BYTES, canonical_pair, sort_work,
};
use crate::{HingeReliefLinearAngleScheduleV1, HingeReliefPolicyRecordV1};

pub(super) const COMPOUND_LOGICAL_CORRIDOR_MODEL_ID_V2: &str =
    "compound_logical_hinge_corridor_preflight_v2";
const POINT_BYTES: usize = WORD_BYTES * 3;
const COMPOUND_SET_BYTES: usize = WORD_BYTES * 3;
const COMPOUND_CERTIFICATE_BYTES: usize =
    ID_BYTES * 2 + POINT_BYTES * 2 + WORD_BYTES * 7 + 1 + 32 * 2;
const COMPOUND_SEGMENT_BYTES: usize = ID_BYTES * 3 + POINT_BYTES * 2 + WORD_BYTES * 7;
const CANONICAL_SEGMENT_BYTES: usize = ID_BYTES * 3 + POINT_BYTES * 2 + WORD_BYTES * 7;
// One exact predicate converts nine binary64 dyadics, then reduces them to two
// three-component difference vectors with fixed-size cross products. Their
// canonical representations are bounded by the binary64 exponent range.
// The byte charge is deliberately allocator-independent and conservative.
const EXACT_PREDICATE_SCRATCH_BYTES: usize = 8_192;
const EXACT_COLLINEAR_WORK: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CompoundCorridorBindingV2 {
    pub(super) radial_depth_bits: u64,
    pub(super) thickness_bits: u64,
    pub(super) bevel_angle_bits: u64,
    pub(super) source_angle_bits: u64,
    pub(super) target_angle_bits: u64,
    pub(super) derivative_bound_bits: u64,
    pub(super) assignment_tag: u8,
}

/// The issuer's unit axis is a rounded normalization of `start -> end`, so
/// exact line geometry uses only the endpoint chord. The enclosing
/// authenticated gap hash independently binds the stored axis bits.
#[derive(Debug, Clone, Copy)]
pub(super) struct CompoundCorridorSegmentInputV2 {
    pub(super) pair: [FaceId; 2],
    pub(super) hinge: EdgeId,
    pub(super) start: Point3,
    pub(super) end: Point3,
    pub(super) binding: CompoundCorridorBindingV2,
}

#[derive(Debug, Clone, Copy)]
struct CanonicalSegmentV2 {
    hinge: EdgeId,
    lower: Point3,
    upper: Point3,
}

/// Opaque internal evidence that two or three split hinge records describe one
/// contiguous logical corridor with one exact relief binding and one shared
/// schedule snapshot. The endpoint and derivative summary retained here does
/// not prove that distinct hinge profiles are identical over the open domain.
#[derive(Debug, Clone)]
pub(super) struct CompoundLogicalCorridorCertificateV2 {
    pair: [FaceId; 2],
    hinges: Vec<EdgeId>,
    lower_bits: [u64; 3],
    upper_bits: [u64; 3],
    binding: CompoundCorridorBindingV2,
    schedule_hash: [u8; 32],
    content_hash: [u8; 32],
}

/// Narrow borrowed composition view for the union-exterior assumption
/// recognizer. It exposes only the immutable canonical values already bound
/// by the compound certificate and cannot construct or mutate that evidence.
#[derive(Debug, Clone, Copy)]
pub(super) struct CompoundCorridorCompositionBindingV2<'a> {
    pub(super) pair: [FaceId; 2],
    pub(super) hinges: &'a [EdgeId],
    pub(super) lower_bits: [u64; 3],
    pub(super) upper_bits: [u64; 3],
    pub(super) binding: CompoundCorridorBindingV2,
    pub(super) schedule_hash: [u8; 32],
    pub(super) content_hash: [u8; 32],
}

impl CompoundLogicalCorridorCertificateV2 {
    pub(super) fn composition_binding_v2(&self) -> CompoundCorridorCompositionBindingV2<'_> {
        CompoundCorridorCompositionBindingV2 {
            pair: self.pair,
            hinges: &self.hinges,
            lower_bits: self.lower_bits,
            upper_bits: self.upper_bits,
            binding: self.binding,
            schedule_hash: self.schedule_hash,
            content_hash: self.content_hash,
        }
    }

    #[cfg(test)]
    pub(super) const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }

    #[cfg(test)]
    pub(super) fn hinges(&self) -> &[EdgeId] {
        &self.hinges
    }

    pub(super) const fn content_hash_v2(&self) -> [u8; 32] {
        self.content_hash
    }

    #[cfg(test)]
    pub(super) const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(super) const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(super) const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    fn same_evidence(&self, other: &Self) -> bool {
        self.pair == other.pair
            && self.hinges == other.hinges
            && self.lower_bits == other.lower_bits
            && self.upper_bits == other.upper_bits
            && self.binding == other.binding
            && self.schedule_hash == other.schedule_hash
            && self.content_hash == other.content_hash
    }
}

impl PartialEq for CompoundLogicalCorridorCertificateV2 {
    fn eq(&self, other: &Self) -> bool {
        self.same_evidence(other)
    }
}

impl Eq for CompoundLogicalCorridorCertificateV2 {}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_compound_logical_corridors_v2(
    geometry: &MaterialHingeGraphGeometry,
    schedule: &CanonicalCycleScheduleV1,
    gaps: &[MultiHingeReliefUnionGapV2],
    policies: &[HingeReliefPolicyRecordV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    thickness_bits: u64,
    schedule_hash: [u8; 32],
    limits: MultiHingeReliefUnionLimitsV2,
    meter: &mut Meter,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<Vec<CompoundLogicalCorridorCertificateV2>, MultiHingeReliefUnionErrorV2> {
    let thickness = f64::from_bits(thickness_bits);
    if meter.limits != limits
        || schedule.certificate_binding_fingerprint_v2() != schedule_hash
        || !thickness.is_finite()
        || thickness <= 0.0
        || policies.len() != local_schedules.len()
    {
        return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
    }
    if gaps.is_empty()
        || gaps.len() > limits.max_pairs
        || geometry.hinges().len() > limits.max_geometry_hinges
    {
        return Err(MultiHingeReliefUnionErrorV2::ResourceLimit);
    }
    meter.retain(COMPOUND_SET_BYTES)?;
    let mut certificates = Vec::new();
    certificates
        .try_reserve_exact(gaps.len())
        .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;

    for gap in gaps {
        if cancelled() {
            return Err(MultiHingeReliefUnionErrorV2::Cancelled);
        }
        let count = gap.hinges.len();
        if !(2..=3).contains(&count) || count > limits.max_hinges_per_pair {
            return Err(MultiHingeReliefUnionErrorV2::ResourceLimit);
        }
        let segment_storage = count
            .checked_mul(COMPOUND_SEGMENT_BYTES)
            .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
        meter.retain(segment_storage)?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(count)
            .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
        for expected in &gap.hinges {
            if cancelled() {
                return Err(MultiHingeReliefUnionErrorV2::Cancelled);
            }
            meter.work(geometry.hinges().len())?;
            let hinge = geometry
                .hinges()
                .iter()
                .find(|hinge| hinge.edge() == expected.hinge)
                .ok_or(MultiHingeReliefUnionErrorV2::IncompleteCoverage)?;
            if canonical_pair(hinge.left_face(), hinge.right_face())? != gap.pair {
                return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
            }

            meter.work(policies.len())?;
            let policy = policies
                .iter()
                .find(|policy| policy.edge == expected.hinge)
                .ok_or(MultiHingeReliefUnionErrorV2::IncompleteCoverage)?;
            meter.work(local_schedules.len())?;
            let local_schedule = local_schedules
                .iter()
                .find(|item| item.edge == expected.hinge)
                .ok_or(MultiHingeReliefUnionErrorV2::IncompleteCoverage)?;
            let radial_depth = policy.cutout_width_mm;
            if !radial_depth.is_finite()
                || radial_depth <= 0.0
                || policy.material_thickness_mm.to_bits() != thickness_bits
                || local_schedule.source_angle_degrees.to_bits() != expected.source_angle_bits
                || local_schedule.target_angle_degrees.to_bits() != expected.target_angle_bits
            {
                return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
            }
            segments.push(CompoundCorridorSegmentInputV2 {
                pair: gap.pair,
                hinge: expected.hinge,
                start: hinge.start(),
                end: hinge.end(),
                binding: CompoundCorridorBindingV2 {
                    radial_depth_bits: radial_depth.to_bits(),
                    thickness_bits,
                    bevel_angle_bits: policy.bevel_angle_degrees.to_bits(),
                    source_angle_bits: expected.source_angle_bits,
                    target_angle_bits: expected.target_angle_bits,
                    derivative_bound_bits: expected.derivative_bound_bits,
                    assignment_tag: match hinge.assignment() {
                        ori_topology::FoldAssignment::Mountain => 0x4d,
                        ori_topology::FoldAssignment::Valley => 0x56,
                    },
                },
            });
        }
        let certificate =
            normalize_compound_corridor_pair_v2(&segments, schedule_hash, limits, meter)?;
        meter.release(segment_storage)?;
        certificates.push(certificate);
    }
    if cancelled() {
        return Err(MultiHingeReliefUnionErrorV2::Cancelled);
    }
    Ok(certificates)
}

/// The caller has already revalidated every
/// [`HingeReliefLinearAngleScheduleV1`]. Its source and target bits completely
/// determine that local linear profile; the exact derivative-bound bits and
/// the enclosing canonical schedule hash are retained as additional binding.
pub(super) fn normalize_compound_corridor_pair_v2(
    inputs: &[CompoundCorridorSegmentInputV2],
    schedule_hash: [u8; 32],
    limits: MultiHingeReliefUnionLimitsV2,
    meter: &mut Meter,
) -> Result<CompoundLogicalCorridorCertificateV2, MultiHingeReliefUnionErrorV2> {
    if meter.limits != limits {
        return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
    }
    if !(2..=3).contains(&inputs.len())
        || inputs.len() > limits.max_hinges_per_pair
        || inputs.len() > limits.max_total_union_hinges
    {
        return Err(MultiHingeReliefUnionErrorV2::ResourceLimit);
    }

    meter.work(inputs.len())?;
    let binding = inputs[0].binding;
    let pair = inputs[0].pair;
    if canonical_pair(pair[0], pair[1])? != pair
        || !valid_binding(binding)
        || inputs
            .iter()
            .any(|segment| segment.pair != pair || segment.binding != binding)
    {
        return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
    }

    let duplicate_checks = inputs
        .len()
        .checked_mul(inputs.len().saturating_sub(1))
        .map(|value| value / 2)
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    meter.work(duplicate_checks)?;
    for (index, segment) in inputs.iter().enumerate() {
        if inputs[index + 1..]
            .iter()
            .any(|other| segment.hinge == other.hinge)
        {
            return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
        }
    }

    meter.work(inputs.len())?;
    let canonical_storage = inputs
        .len()
        .checked_mul(CANONICAL_SEGMENT_BYTES)
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    meter.retain(canonical_storage)?;
    let mut segments = Vec::new();
    segments
        .try_reserve_exact(inputs.len())
        .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    for input in inputs {
        let order = point_cmp(input.start, input.end);
        if order == Ordering::Equal {
            return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
        }
        let (lower, upper) = if order == Ordering::Less {
            (input.start, input.end)
        } else {
            (input.end, input.start)
        };
        segments.push(CanonicalSegmentV2 {
            hinge: input.hinge,
            lower,
            upper,
        });
    }
    meter.work(sort_work(segments.len())?)?;
    segments.sort_unstable_by(|left, right| {
        point_cmp(left.lower, right.lower)
            .then_with(|| point_cmp(left.upper, right.upper))
            .then_with(|| {
                left.hinge
                    .canonical_bytes()
                    .cmp(&right.hinge.canonical_bytes())
            })
    });
    meter.work(segments.len().saturating_sub(1))?;
    if segments
        .windows(2)
        .any(|pair| !point_bits_equal(pair[0].upper, pair[1].lower))
    {
        return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
    }

    let lower = segments[0].lower;
    let upper = segments[segments.len() - 1].upper;
    if point_cmp(lower, upper) != Ordering::Less {
        return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
    }
    meter.retain(EXACT_PREDICATE_SCRATCH_BYTES)?;
    for segment in &segments {
        meter.work(
            EXACT_COLLINEAR_WORK
                .checked_mul(2)
                .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
        )?;
        if !exact_point_on_line(lower, upper, segment.lower)
            || !exact_point_on_line(lower, upper, segment.upper)
        {
            return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
        }
    }
    meter.release(EXACT_PREDICATE_SCRATCH_BYTES)?;

    let retained = COMPOUND_CERTIFICATE_BYTES
        .checked_add(
            segments
                .len()
                .checked_mul(ID_BYTES)
                .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
        )
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    meter.retain(retained)?;
    let hinges = segments
        .iter()
        .map(|segment| segment.hinge)
        .collect::<Vec<EdgeId>>();
    let lower_bits = point_bits(lower);
    let upper_bits = point_bits(upper);
    meter.work(segments.len())?;
    let content_hash = compound_hash(
        pair,
        &hinges,
        lower_bits,
        upper_bits,
        binding,
        schedule_hash,
    );
    meter.release(canonical_storage)?;
    Ok(CompoundLogicalCorridorCertificateV2 {
        pair,
        hinges,
        lower_bits,
        upper_bits,
        binding,
        schedule_hash,
        content_hash,
    })
}

fn valid_binding(binding: CompoundCorridorBindingV2) -> bool {
    let radial_depth = f64::from_bits(binding.radial_depth_bits);
    let thickness = f64::from_bits(binding.thickness_bits);
    let bevel = f64::from_bits(binding.bevel_angle_bits);
    let source = f64::from_bits(binding.source_angle_bits);
    let target = f64::from_bits(binding.target_angle_bits);
    let derivative = f64::from_bits(binding.derivative_bound_bits);
    radial_depth.is_finite()
        && radial_depth > 0.0
        && thickness.is_finite()
        && thickness > 0.0
        && bevel.is_finite()
        && bevel > 0.0
        && bevel < 180.0
        && source.is_finite()
        && source > 0.0
        && source <= 180.0
        && target.is_finite()
        && target > 0.0
        && target <= 180.0
        && derivative.is_finite()
        && derivative >= 0.0
        && matches!(binding.assignment_tag, 0x4d | 0x56)
}

fn point_cmp(left: Point3, right: Point3) -> Ordering {
    left.x()
        .total_cmp(&right.x())
        .then_with(|| left.y().total_cmp(&right.y()))
        .then_with(|| left.z().total_cmp(&right.z()))
}

fn point_bits_equal(left: Point3, right: Point3) -> bool {
    point_bits(left) == point_bits(right)
}

fn point_bits(point: Point3) -> [u64; 3] {
    [
        point.x().to_bits(),
        point.y().to_bits(),
        point.z().to_bits(),
    ]
}

fn exact_point_on_line(reference_start: Point3, reference_end: Point3, point: Point3) -> bool {
    let Some(origin) = exact_point(reference_start) else {
        return false;
    };
    let Some(end) = exact_point(reference_end) else {
        return false;
    };
    let Some(point) = exact_point(point) else {
        return false;
    };
    let reference = subtract(&end, &origin);
    let offset = subtract(&point, &origin);
    cross_is_zero(&offset, &reference)
}

fn exact_point(point: Point3) -> Option<[BigRational; 3]> {
    Some([
        BigRational::from_float(point.x())?,
        BigRational::from_float(point.y())?,
        BigRational::from_float(point.z())?,
    ])
}

fn subtract(left: &[BigRational; 3], right: &[BigRational; 3]) -> [BigRational; 3] {
    [
        &left[0] - &right[0],
        &left[1] - &right[1],
        &left[2] - &right[2],
    ]
}

fn cross_is_zero(left: &[BigRational; 3], right: &[BigRational; 3]) -> bool {
    [
        &left[1] * &right[2] - &left[2] * &right[1],
        &left[2] * &right[0] - &left[0] * &right[2],
        &left[0] * &right[1] - &left[1] * &right[0],
    ]
    .iter()
    .all(Zero::is_zero)
}

fn compound_hash(
    pair: [FaceId; 2],
    hinges: &[EdgeId],
    lower_bits: [u64; 3],
    upper_bits: [u64; 3],
    binding: CompoundCorridorBindingV2,
    schedule_hash: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPOUND_LOGICAL_CORRIDOR_MODEL_ID_V2.as_bytes());
    hash.update(pair[0].canonical_bytes());
    hash.update(pair[1].canonical_bytes());
    hash.update((hinges.len() as u64).to_be_bytes());
    for hinge in hinges {
        hash.update(hinge.canonical_bytes());
    }
    for bits in lower_bits.into_iter().chain(upper_bits) {
        hash.update(bits.to_be_bytes());
    }
    for bits in [
        binding.radial_depth_bits,
        binding.thickness_bits,
        binding.bevel_angle_bits,
        binding.source_angle_bits,
        binding.target_angle_bits,
        binding.derivative_bound_bits,
    ] {
        hash.update(bits.to_be_bytes());
    }
    hash.update([binding.assignment_tag]);
    hash.update(schedule_hash);
    hash.finalize().into()
}
