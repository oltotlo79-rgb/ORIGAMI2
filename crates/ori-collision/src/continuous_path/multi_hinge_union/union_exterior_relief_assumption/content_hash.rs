use num_bigint::Sign;
use num_rational::BigRational;
use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1,
    EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1, EffectiveGeneratorSignV1,
};
use sha2::{Digest, Sha256};

use super::exact_geometry::{ExactCorridorGeometryV1, MeterV1};
use super::{
    ExactReliefValuesV1, SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_MODEL_ID_V1,
    SplitHingeUnionExteriorReliefAssumptionErrorV1 as ErrorV1,
    SplitHingeUnionExteriorReliefAssumptionLimitsV1 as LimitsV1,
};
use crate::{
    HINGE_RELIEF_LOCAL_INTERVAL_MODEL_ID_V1, HINGE_RELIEF_POLICY_MODEL_ID_V1,
    HingeReliefLinearAngleScheduleV1, HingeReliefPolicyLimitsV1, HingeReliefPolicyRecordV1,
};

use super::super::{
    MULTI_HINGE_RELIEF_UNION_CERTIFICATE_MODEL_ID_V2, MULTI_HINGE_RELIEF_UNION_GAP_MODEL_ID_V2,
    MultiHingeReliefUnionCertificateV2, MultiHingeReliefUnionGapReportV2,
    MultiHingeReliefUnionLimitsV2,
    compound_corridor::{
        COMPOUND_LOGICAL_CORRIDOR_MODEL_ID_V2, CompoundCorridorCompositionBindingV2,
    },
};

const SHA256_SCRATCH_BYTES_V1: usize = 104;
const WORD_BYTES_V1: usize = 8;

#[allow(clippy::too_many_arguments)]
pub(super) fn content_hash_v1(
    fixed_face: FaceId,
    sign: EffectiveGeneratorSignV1,
    edges: &[EdgeId],
    binding: CompoundCorridorCompositionBindingV2<'_>,
    angle: (u64, u64),
    exact_geometry: &ExactCorridorGeometryV1,
    exact: &ExactReliefValuesV1,
    gaps: &MultiHingeReliefUnionGapReportV2,
    union: &MultiHingeReliefUnionCertificateV2,
    policies: &[HingeReliefPolicyRecordV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: LimitsV1,
    meter: &mut MeterV1,
) -> Result<[u8; 32], ErrorV1> {
    meter.begin_temporary(SHA256_SCRATCH_BYTES_V1)?;
    let result = hash_content(
        fixed_face,
        sign,
        edges,
        binding,
        angle,
        exact_geometry,
        exact,
        gaps,
        union,
        policies,
        local_schedules,
        policy_limits,
        union_limits,
        limits,
        meter,
    );
    meter.end_temporary(SHA256_SCRATCH_BYTES_V1);
    result
}

#[allow(clippy::too_many_arguments)]
fn hash_content(
    fixed_face: FaceId,
    sign: EffectiveGeneratorSignV1,
    edges: &[EdgeId],
    binding: CompoundCorridorCompositionBindingV2<'_>,
    angle: (u64, u64),
    exact_geometry: &ExactCorridorGeometryV1,
    exact: &ExactReliefValuesV1,
    gaps: &MultiHingeReliefUnionGapReportV2,
    union: &MultiHingeReliefUnionCertificateV2,
    policies: &[HingeReliefPolicyRecordV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: LimitsV1,
    meter: &mut MeterV1,
) -> Result<[u8; 32], ErrorV1> {
    let mut hash = Sha256::new();
    hash_field(
        &mut hash,
        b"ORIGAMI2_SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_CONTENT_V1",
        meter,
    )?;
    for model in [
        SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_MODEL_ID_V1,
        MULTI_HINGE_RELIEF_UNION_GAP_MODEL_ID_V2,
        MULTI_HINGE_RELIEF_UNION_CERTIFICATE_MODEL_ID_V2,
        COMPOUND_LOGICAL_CORRIDOR_MODEL_ID_V2,
        HINGE_RELIEF_POLICY_MODEL_ID_V1,
        HINGE_RELIEF_LOCAL_INTERVAL_MODEL_ID_V1,
        EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1,
        EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1,
    ] {
        hash_field(&mut hash, model.as_bytes(), meter)?;
    }
    for digest in [
        gaps.geometry_hash,
        gaps.schedule_hash,
        gaps.content_hash,
        union.content_hash,
        binding.schedule_hash,
        binding.content_hash,
        exact_geometry.boundary_hash,
    ] {
        hash_field(&mut hash, &digest, meter)?;
    }
    hash_field(&mut hash, &fixed_face.canonical_bytes(), meter)?;
    for face in binding.pair {
        hash_field(&mut hash, &face.canonical_bytes(), meter)?;
    }
    hash_field(
        &mut hash,
        &[match sign {
            EffectiveGeneratorSignV1::Negative => 0,
            EffectiveGeneratorSignV1::Positive => 1,
        }],
        meter,
    )?;
    hash_usize(&mut hash, edges.len(), meter)?;
    for edge in edges {
        hash_field(&mut hash, &edge.canonical_bytes(), meter)?;
    }
    for bits in binding.lower_bits.into_iter().chain(binding.upper_bits) {
        hash_field(&mut hash, &bits.to_be_bytes(), meter)?;
    }
    for bits in [
        angle.0,
        angle.1,
        binding.binding.radial_depth_bits,
        binding.binding.thickness_bits,
        binding.binding.bevel_angle_bits,
        binding.binding.source_angle_bits,
        binding.binding.target_angle_bits,
        binding.binding.derivative_bound_bits,
    ] {
        hash_field(&mut hash, &bits.to_be_bytes(), meter)?;
    }
    hash_field(&mut hash, &[binding.binding.assignment_tag], meter)?;
    for rational in [
        &exact.alpha_lower,
        &exact.alpha_upper,
        &exact.radius,
        &exact.thickness,
        &exact.relief_left,
        &exact.relief_right,
        &exact_geometry.line_length_squared,
        &exact_geometry.radial_squared_line_length_squared,
    ] {
        hash_rational(&mut hash, rational, meter)?;
    }
    hash_usize(&mut hash, policies.len(), meter)?;
    for policy in policies {
        hash_field(&mut hash, &policy.edge.canonical_bytes(), meter)?;
        for bits in [
            policy.cutout_width_mm.to_bits(),
            policy.bevel_angle_degrees.to_bits(),
            policy.material_thickness_mm.to_bits(),
        ] {
            hash_field(&mut hash, &bits.to_be_bytes(), meter)?;
        }
    }
    hash_usize(&mut hash, local_schedules.len(), meter)?;
    for schedule in local_schedules {
        hash_field(&mut hash, &schedule.edge.canonical_bytes(), meter)?;
        hash_field(
            &mut hash,
            &schedule.source_angle_degrees.to_bits().to_be_bytes(),
            meter,
        )?;
        hash_field(
            &mut hash,
            &schedule.target_angle_degrees.to_bits().to_be_bytes(),
            meter,
        )?;
    }
    hash_all_limits(&mut hash, policy_limits, union_limits, limits, meter)?;
    Ok(hash.finalize().into())
}

fn hash_all_limits(
    hash: &mut Sha256,
    policy: HingeReliefPolicyLimitsV1,
    union: MultiHingeReliefUnionLimitsV2,
    limits: LimitsV1,
    meter: &mut MeterV1,
) -> Result<(), ErrorV1> {
    let schedule = limits.schedule_limits;
    let profile = limits.profile_limits;
    let sign = limits.split_sign_limits;
    for value in [
        policy.max_records,
        union.max_pairs,
        union.max_hinges_per_pair,
        union.max_total_union_hinges,
        union.max_geometry_hinges,
        union.max_work,
        union.max_storage_bytes,
        schedule.max_hinges,
        schedule.max_degree,
        usize::try_from(schedule.max_coefficient_bits).map_err(|_| ErrorV1::ResourceLimit)?,
        schedule.max_work,
        profile.max_edges,
        profile.max_work,
        profile.max_retained_bytes,
        profile.max_peak_bytes,
        sign.max_edges,
        sign.max_faces,
        sign.max_work,
        sign.max_retained_bytes,
        sign.max_peak_bytes,
        limits.max_edges,
        limits.max_faces,
        limits.max_boundary_vertices_per_face,
        limits.max_total_boundary_vertices,
        limits.max_work,
        limits.max_retained_bytes,
        limits.max_peak_bytes,
    ] {
        hash_usize(hash, value, meter)?;
    }
    hash_field(
        hash,
        &limits.max_exact_bits_per_rational.to_be_bytes(),
        meter,
    )?;
    hash_field(hash, &limits.max_total_exact_bits.to_be_bytes(), meter)
}

fn hash_rational(
    hash: &mut Sha256,
    value: &BigRational,
    meter: &mut MeterV1,
) -> Result<(), ErrorV1> {
    let numerator_bytes =
        usize::try_from(value.numer().bits().div_ceil(8)).map_err(|_| ErrorV1::ResourceLimit)?;
    let denominator_bytes =
        usize::try_from(value.denom().bits().div_ceil(8)).map_err(|_| ErrorV1::ResourceLimit)?;
    let temporary = numerator_bytes
        .checked_add(denominator_bytes)
        .ok_or(ErrorV1::ResourceLimit)?;
    meter.begin_temporary(temporary)?;
    let (sign, numerator) = value.numer().to_bytes_be();
    let (_, denominator) = value.denom().to_bytes_be();
    let result = (|| {
        if numerator.len() > numerator_bytes || denominator.len() > denominator_bytes {
            return Err(ErrorV1::ResourceLimit);
        }
        hash_field(
            hash,
            &[match sign {
                Sign::Minus => 0,
                Sign::NoSign => 1,
                Sign::Plus => 2,
            }],
            meter,
        )?;
        hash_field(hash, &numerator, meter)?;
        hash_field(hash, &denominator, meter)
    })();
    meter.end_temporary(temporary);
    result
}

fn hash_usize(hash: &mut Sha256, value: usize, meter: &mut MeterV1) -> Result<(), ErrorV1> {
    hash_field(
        hash,
        &u64::try_from(value)
            .map_err(|_| ErrorV1::ResourceLimit)?
            .to_be_bytes(),
        meter,
    )
}

fn hash_field(hash: &mut Sha256, bytes: &[u8], meter: &mut MeterV1) -> Result<(), ErrorV1> {
    meter.charge_work(
        bytes
            .len()
            .checked_add(WORD_BYTES_V1)
            .ok_or(ErrorV1::ResourceLimit)?,
    )?;
    let length = u64::try_from(bytes.len()).map_err(|_| ErrorV1::ResourceLimit)?;
    hash.update(length.to_be_bytes());
    hash.update(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitHingeUnionExteriorReliefAssumptionLimitsV1;

    fn framed_digest(fields: &[&[u8]]) -> [u8; 32] {
        let mut meter = MeterV1::new(SplitHingeUnionExteriorReliefAssumptionLimitsV1::default());
        let mut hash = Sha256::new();
        for field in fields {
            hash_field(&mut hash, field, &mut meter).unwrap();
        }
        hash.finalize().into()
    }

    #[test]
    fn every_hash_field_is_length_framed() {
        assert_ne!(framed_digest(&[b"ab", b"c"]), framed_digest(&[b"a", b"bc"]));
        assert_ne!(framed_digest(&[b""]), framed_digest(&[]));
    }
}
