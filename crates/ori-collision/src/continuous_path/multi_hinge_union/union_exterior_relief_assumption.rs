//! Bounded recognition of one split-hinge relief union and its finite rest
//! geometry envelope.
//!
//! This evidence is deliberately an assumption record, not continuous
//! collision clearance. It reconnects no admission, simulation, persistence,
//! apply, or mutation path.

use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleLimitsV1, EffectiveGeneratorSignV1,
    ExactCommonLinearCycleProfileLimitsV1, ExactCommonLinearCycleProfileV1,
    ExactCommonSplitPairEffectiveGeneratorSignLimitsV1,
    ExactCommonSplitPairEffectiveGeneratorSignV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry,
};
use thiserror::Error;

use super::{
    MAX_MULTI_HINGE_UNION_GEOMETRY_HINGES_V2, MAX_MULTI_HINGE_UNION_HINGES_V2,
    MAX_MULTI_HINGE_UNION_PAIRS_V2, MAX_MULTI_HINGE_UNION_STORAGE_BYTES_V2,
    MAX_MULTI_HINGE_UNION_WORK_V2, MAX_MULTI_HINGES_PER_FACE_PAIR_V2,
    MultiHingeReliefUnionCertificateV2, MultiHingeReliefUnionGapReportV2,
    MultiHingeReliefUnionLimitsV2, compound_corridor::CompoundCorridorCompositionBindingV2,
    revalidate_multi_hinge_relief_union_certificate_v2,
};
use crate::{
    HingeReliefLinearAngleScheduleV1, HingeReliefPolicyLimitsV1, HingeReliefPolicyRecordV1,
    NativeHingeReliefLocalIntervalCertificateV1, NativeHingeReliefPrerequisiteV1,
    revalidate_hinge_relief_local_intervals_v1,
};

mod content_hash;
mod exact_geometry;

use content_hash::content_hash_v1;
use exact_geometry::{
    ExactCorridorGeometryV1, MeterV1, exact_from_f64, exact_mul,
    validate_exact_corridor_geometry_v1,
};

pub const SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_MODEL_ID_V1: &str =
    "split_hinge_union_exterior_relief_assumption_v1";

const MIN_EDGES_V1: usize = 2;
const MAX_EDGES_V1: usize = 3;
const REQUIRED_FACES_V1: usize = 2;
const ID_BYTES_V1: usize = 16;
const HASH_BYTES_V1: usize = 32;
const WORD_BYTES_V1: usize = 8;
const SIGN_BYTES_V1: usize = 1;
const INSTANCE_ANCHOR_BYTES_V1: usize = 16;
// Ten geometry/angle/dimension bit words, every retained caller limit
// (including the deliberately duplicated nested profile envelope), and the
// five published resource observations.
const RETAINED_WORDS_V1: usize = 10 + (1 + 6 + 4 + 4 + 9 + 9) + 5;
const EXACT_COMPOSITION_SCRATCH_BYTES_V1: usize = 64 * 1024;
const MAX_BOUNDARY_VERTICES_PER_FACE_V1: usize = 4_096;
const MAX_TOTAL_BOUNDARY_VERTICES_V1: usize = 8_192;
const MAX_EXACT_BITS_PER_RATIONAL_V1: u64 = 16_384;
const MAX_TOTAL_EXACT_BITS_V1: u64 = 512 * 1024 * 1024;
const MAX_WORK_V1: usize = 16 * 1024 * 1024;
const MAX_RETAINED_BYTES_V1: usize = 64 * 1024;
const MAX_PEAK_BYTES_V1: usize = 512 * 1024;
const MAX_SCHEDULE_WORK_V1: usize = 1_048_576;
const MAX_SCHEDULE_DEGREE_V1: usize = 64;
const MAX_SCHEDULE_COEFFICIENT_BITS_V1: u32 = 4_096;

/// Complete caller-selected resource envelope.
///
/// The three nested limits independently bound fresh schedule/profile/sign
/// validation. Policy and union limits are explicit producer arguments because
/// those values are already part of the upstream certificates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitHingeUnionExteriorReliefAssumptionLimitsV1 {
    pub schedule_limits: CycleScheduleLimitsV1,
    pub profile_limits: ExactCommonLinearCycleProfileLimitsV1,
    pub split_sign_limits: ExactCommonSplitPairEffectiveGeneratorSignLimitsV1,
    pub max_edges: usize,
    pub max_faces: usize,
    pub max_boundary_vertices_per_face: usize,
    pub max_total_boundary_vertices: usize,
    pub max_exact_bits_per_rational: u64,
    pub max_total_exact_bits: u64,
    pub max_work: usize,
    pub max_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

impl Default for SplitHingeUnionExteriorReliefAssumptionLimitsV1 {
    fn default() -> Self {
        Self {
            schedule_limits: CycleScheduleLimitsV1::default(),
            profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
            split_sign_limits: ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
            max_edges: MAX_EDGES_V1,
            max_faces: REQUIRED_FACES_V1,
            max_boundary_vertices_per_face: MAX_BOUNDARY_VERTICES_PER_FACE_V1,
            max_total_boundary_vertices: MAX_TOTAL_BOUNDARY_VERTICES_V1,
            max_exact_bits_per_rational: MAX_EXACT_BITS_PER_RATIONAL_V1,
            max_total_exact_bits: MAX_TOTAL_EXACT_BITS_V1,
            max_work: 2 * 1024 * 1024,
            max_retained_bytes: 4 * 1024,
            max_peak_bytes: MAX_PEAK_BYTES_V1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SplitHingeUnionExteriorReliefAssumptionErrorV1 {
    #[error("split-hinge union-exterior limits are invalid")]
    InvalidLimits,
    #[error("split-hinge union-exterior composition binding is invalid or stale")]
    InvalidBinding,
    #[error("split-hinge union-exterior proof exceeded a hard resource limit")]
    ResourceLimit,
    #[error("the common linear profile is foreign or stale")]
    ForeignCommonProfile,
    #[error("the split-pair effective sign is foreign or stale")]
    ForeignSplitPairSign,
    #[error("the local hinge-relief evidence is foreign or stale")]
    ForeignRelief,
    #[error("the multi-hinge relief union is foreign, incomplete, or stale")]
    ForeignUnion,
    #[error("the exact split pair has no unique compound corridor")]
    MissingCompoundCorridor,
    #[error("the root outward angle box is outside the supported (0, 90] domain")]
    AngleDomain,
    #[error("the exact relief-radius/thickness inequality is not satisfied")]
    ReliefInequality,
    #[error("the bound face boundary registry is incomplete")]
    BoundaryRegistry,
    #[error("a face boundary leaves the finite compound-corridor envelope")]
    CorridorEnvelope,
    #[error("the two face boundaries do not occupy strict opposite carrier sides")]
    SideTopology,
    #[error("the two face boundaries do not attain both exact axial caps")]
    AxialCaps,
    #[error("the evidence was not issued by these exact geometry inputs")]
    IssuerMismatch,
}

/// Opaque, non-persistent recognition evidence for the Phase-B assumption.
///
/// It authenticates a finite rest-geometry envelope, a conservative root
/// angle domain, and exact relief inequalities after fresh upstream proof
/// revalidation. It does not prove separation during motion.
///
/// ```compile_fail
/// use ori_collision::SplitHingeUnionExteriorReliefAssumptionV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<SplitHingeUnionExteriorReliefAssumptionV1>();
/// ```
#[derive(Debug, Clone)]
pub struct SplitHingeUnionExteriorReliefAssumptionV1 {
    // The nested proof retains the kinematics crate's opaque Arc identity
    // anchor. This avoids cloning an entire geometry while preserving strict
    // same-preparation-instance revalidation.
    issuer: ExactCommonSplitPairEffectiveGeneratorSignV1,
    pair: [FaceId; 2],
    fixed_face: FaceId,
    canonical_edges: Vec<EdgeId>,
    common_effective_sign: EffectiveGeneratorSignV1,
    lower_bits: [u64; 3],
    upper_bits: [u64; 3],
    angle_lower_bits: u64,
    angle_upper_bits: u64,
    radial_depth_bits: u64,
    thickness_bits: u64,
    graph_hash: [u8; HASH_BYTES_V1],
    schedule_hash: [u8; HASH_BYTES_V1],
    gap_hash: [u8; HASH_BYTES_V1],
    union_hash: [u8; HASH_BYTES_V1],
    compound_hash: [u8; HASH_BYTES_V1],
    boundary_hash: [u8; HASH_BYTES_V1],
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
    work_used: usize,
    maximum_exact_bits: u64,
    total_exact_bits: u64,
    retained_storage_bytes: usize,
    peak_storage_bytes: usize,
    content_hash: [u8; HASH_BYTES_V1],
}

impl SplitHingeUnionExteriorReliefAssumptionV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_MODEL_ID_V1
    }

    #[must_use]
    pub const fn face_pair(&self) -> [FaceId; 2] {
        self.pair
    }

    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub fn moving_face(&self) -> FaceId {
        if self.pair[0] == self.fixed_face {
            self.pair[1]
        } else {
            self.pair[0]
        }
    }

    #[must_use]
    pub fn edge_ids(&self) -> &[EdgeId] {
        &self.canonical_edges
    }

    #[must_use]
    pub const fn common_effective_sign(&self) -> EffectiveGeneratorSignV1 {
        self.common_effective_sign
    }

    #[must_use]
    pub const fn content_hash_v1(&self) -> [u8; 32] {
        self.content_hash
    }

    /// Composition-local deterministic work. Fresh upstream proofs enforce
    /// their own independently hashed resource envelopes.
    #[must_use]
    pub const fn work_used(&self) -> usize {
        self.work_used
    }

    #[must_use]
    pub const fn total_exact_bits(&self) -> u64 {
        self.total_exact_bits
    }

    #[must_use]
    pub const fn maximum_exact_bits(&self) -> u64 {
        self.maximum_exact_bits
    }

    #[must_use]
    pub const fn retained_storage_bytes(&self) -> usize {
        self.retained_storage_bytes
    }

    #[must_use]
    pub const fn peak_storage_bytes(&self) -> usize {
        self.peak_storage_bytes
    }

    #[must_use]
    pub const fn recognizes_union_exterior_relief_assumption(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn authorizes_union_exterior_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_whole_path(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_free_classification(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_shared_hinge_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    fn same_evidence(&self, other: &Self) -> bool {
        self.issuer == other.issuer
            && self.pair == other.pair
            && self.fixed_face == other.fixed_face
            && self.canonical_edges == other.canonical_edges
            && self.common_effective_sign == other.common_effective_sign
            && self.lower_bits == other.lower_bits
            && self.upper_bits == other.upper_bits
            && self.angle_lower_bits == other.angle_lower_bits
            && self.angle_upper_bits == other.angle_upper_bits
            && self.radial_depth_bits == other.radial_depth_bits
            && self.thickness_bits == other.thickness_bits
            && self.graph_hash == other.graph_hash
            && self.schedule_hash == other.schedule_hash
            && self.gap_hash == other.gap_hash
            && self.union_hash == other.union_hash
            && self.compound_hash == other.compound_hash
            && self.boundary_hash == other.boundary_hash
            && self.policy_limits == other.policy_limits
            && self.union_limits == other.union_limits
            && self.limits == other.limits
            && self.work_used == other.work_used
            && self.maximum_exact_bits == other.maximum_exact_bits
            && self.total_exact_bits == other.total_exact_bits
            && self.retained_storage_bytes == other.retained_storage_bytes
            && self.peak_storage_bytes == other.peak_storage_bytes
            && self.content_hash == other.content_hash
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prove_split_hinge_union_exterior_relief_assumption_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    split_sign: &ExactCommonSplitPairEffectiveGeneratorSignV1,
    gaps: &MultiHingeReliefUnionGapReportV2,
    union: &MultiHingeReliefUnionCertificateV2,
    thickness_mm: f64,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    local: &NativeHingeReliefLocalIntervalCertificateV1,
    policies: &[HingeReliefPolicyRecordV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
) -> Result<SplitHingeUnionExteriorReliefAssumptionV1, SplitHingeUnionExteriorReliefAssumptionErrorV1>
{
    validate_limits(policy_limits, union_limits, limits)?;
    let mut meter = MeterV1::new(limits);
    meter.charge_work(1)?;

    common_profile
        .revalidate_issuer_schedule_v1(schedule, limits.profile_limits)
        .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignCommonProfile)?;
    split_sign
        .revalidate_issuers_v1(
            geometry,
            audit,
            fixed_face,
            schedule,
            common_profile,
            limits.split_sign_limits,
        )
        .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignSplitPairSign)?;
    revalidate_hinge_relief_local_intervals_v1(
        local,
        prerequisite,
        geometry,
        thickness_mm,
        policies,
        local_schedules,
        policy_limits,
    )
    .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignRelief)?;
    revalidate_multi_hinge_relief_union_certificate_v2(
        union,
        gaps,
        geometry,
        audit,
        fixed_face,
        schedule,
        thickness_mm,
        prerequisite,
        local,
        policies,
        local_schedules,
        policy_limits,
        union_limits,
    )
    .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignUnion)?;

    let binding = validate_composition_binding(
        geometry,
        fixed_face,
        schedule,
        common_profile,
        split_sign,
        gaps,
        union,
        thickness_mm,
        policy_limits,
        union_limits,
        limits,
        &mut meter,
    )?;
    meter.retain(retained_bytes_v1(binding.hinges.len())?)?;
    let mut canonical_edges = Vec::new();
    canonical_edges
        .try_reserve_exact(binding.hinges.len())
        .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)?;
    canonical_edges.extend_from_slice(split_sign.edge_ids());

    let angle = validate_angle_domain(schedule, &canonical_edges, limits, &mut meter)?;
    meter.begin_temporary(EXACT_COMPOSITION_SCRATCH_BYTES_V1)?;
    let exact_result = validate_relief_and_geometry(
        geometry,
        fixed_face,
        thickness_mm,
        binding,
        angle,
        limits,
        &mut meter,
    );
    let (exact_geometry, exact_values) = match exact_result {
        Ok(value) => value,
        Err(error) => {
            meter.end_temporary(EXACT_COMPOSITION_SCRATCH_BYTES_V1);
            return Err(error);
        }
    };
    let hash_result = content_hash_v1(
        fixed_face,
        split_sign.common_effective_sign(),
        &canonical_edges,
        binding,
        angle,
        &exact_geometry,
        &exact_values,
        gaps,
        union,
        policies,
        local_schedules,
        policy_limits,
        union_limits,
        limits,
        &mut meter,
    );
    meter.end_temporary(EXACT_COMPOSITION_SCRATCH_BYTES_V1);
    let content_hash = hash_result?;

    Ok(SplitHingeUnionExteriorReliefAssumptionV1 {
        issuer: split_sign.clone(),
        pair: binding.pair,
        fixed_face,
        canonical_edges,
        common_effective_sign: split_sign.common_effective_sign(),
        lower_bits: binding.lower_bits,
        upper_bits: binding.upper_bits,
        angle_lower_bits: angle.0,
        angle_upper_bits: angle.1,
        radial_depth_bits: binding.binding.radial_depth_bits,
        thickness_bits: thickness_mm.to_bits(),
        graph_hash: schedule.graph_binding_fingerprint_v1(),
        schedule_hash: schedule.certificate_binding_fingerprint_v2(),
        gap_hash: gaps.content_hash,
        union_hash: union.content_hash,
        compound_hash: binding.content_hash,
        boundary_hash: exact_geometry.boundary_hash,
        policy_limits,
        union_limits,
        limits,
        work_used: meter.work_used(),
        maximum_exact_bits: meter.maximum_exact_bits(),
        total_exact_bits: meter.total_exact_bits(),
        retained_storage_bytes: meter.retained_storage_bytes(),
        peak_storage_bytes: meter.peak_storage_bytes(),
        content_hash,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn revalidate_split_hinge_union_exterior_relief_assumption_v1(
    evidence: &SplitHingeUnionExteriorReliefAssumptionV1,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    split_sign: &ExactCommonSplitPairEffectiveGeneratorSignV1,
    gaps: &MultiHingeReliefUnionGapReportV2,
    union: &MultiHingeReliefUnionCertificateV2,
    thickness_mm: f64,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    local: &NativeHingeReliefLocalIntervalCertificateV1,
    policies: &[HingeReliefPolicyRecordV1],
    local_schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
) -> Result<(), SplitHingeUnionExteriorReliefAssumptionErrorV1> {
    validate_limits(policy_limits, union_limits, limits)?;
    evidence
        .issuer
        .revalidate_issuers_v1(
            geometry,
            audit,
            fixed_face,
            schedule,
            common_profile,
            limits.split_sign_limits,
        )
        .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::IssuerMismatch)?;
    let fresh = prove_split_hinge_union_exterior_relief_assumption_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        common_profile,
        split_sign,
        gaps,
        union,
        thickness_mm,
        prerequisite,
        local,
        policies,
        local_schedules,
        policy_limits,
        union_limits,
        limits,
    )?;
    if evidence.same_evidence(&fresh) {
        Ok(())
    } else {
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::IssuerMismatch)
    }
}

fn validate_limits(
    policy: HingeReliefPolicyLimitsV1,
    union: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
) -> Result<(), SplitHingeUnionExteriorReliefAssumptionErrorV1> {
    let schedule = limits.schedule_limits;
    let profile = limits.profile_limits;
    let sign = limits.split_sign_limits;
    if !(MIN_EDGES_V1..=MAX_EDGES_V1).contains(&limits.max_edges)
        || limits.max_faces != REQUIRED_FACES_V1
        || !(3..=MAX_BOUNDARY_VERTICES_PER_FACE_V1).contains(&limits.max_boundary_vertices_per_face)
        || !(6..=MAX_TOTAL_BOUNDARY_VERTICES_V1).contains(&limits.max_total_boundary_vertices)
        || limits.max_exact_bits_per_rational == 0
        || limits.max_exact_bits_per_rational > MAX_EXACT_BITS_PER_RATIONAL_V1
        || limits.max_total_exact_bits == 0
        || limits.max_total_exact_bits > MAX_TOTAL_EXACT_BITS_V1
        || limits.max_work == 0
        || limits.max_work > MAX_WORK_V1
        || limits.max_retained_bytes == 0
        || limits.max_retained_bytes > MAX_RETAINED_BYTES_V1
        || limits.max_peak_bytes < limits.max_retained_bytes
        || limits.max_peak_bytes > MAX_PEAK_BYTES_V1
        || schedule.max_hinges < MIN_EDGES_V1
        || schedule.max_hinges > 128
        || schedule.max_degree == 0
        || schedule.max_degree > MAX_SCHEDULE_DEGREE_V1
        || schedule.max_coefficient_bits == 0
        || schedule.max_coefficient_bits > MAX_SCHEDULE_COEFFICIENT_BITS_V1
        || schedule.max_work == 0
        || schedule.max_work > MAX_SCHEDULE_WORK_V1
        || profile.max_edges < MIN_EDGES_V1
        || profile.max_edges > MAX_EDGES_V1
        || profile.max_work == 0
        || profile.max_work > 4_096
        || profile.max_retained_bytes == 0
        || profile.max_retained_bytes > 4_096
        || profile.max_peak_bytes < profile.max_retained_bytes
        || profile.max_peak_bytes > 16 * 1024
        || sign.profile_limits != profile
        || sign.max_edges < MIN_EDGES_V1
        || sign.max_edges > MAX_EDGES_V1
        || sign.max_faces != REQUIRED_FACES_V1
        || sign.max_work == 0
        || sign.max_work > 96 * 1024
        || sign.max_retained_bytes == 0
        || sign.max_retained_bytes > 4_096
        || sign.max_peak_bytes < sign.max_retained_bytes
        || sign.max_peak_bytes > 128 * 1024
        || policy.max_records == 0
        || policy.max_records > crate::MAX_HINGE_RELIEF_RECORDS_V1
        || union.max_pairs == 0
        || union.max_pairs > MAX_MULTI_HINGE_UNION_PAIRS_V2
        || union.max_hinges_per_pair < MIN_EDGES_V1
        || union.max_hinges_per_pair > MAX_MULTI_HINGES_PER_FACE_PAIR_V2
        || union.max_total_union_hinges < MIN_EDGES_V1
        || union.max_total_union_hinges > MAX_MULTI_HINGE_UNION_HINGES_V2
        || union.max_geometry_hinges < MIN_EDGES_V1
        || union.max_geometry_hinges > MAX_MULTI_HINGE_UNION_GEOMETRY_HINGES_V2
        || union.max_work == 0
        || union.max_work > MAX_MULTI_HINGE_UNION_WORK_V2
        || union.max_storage_bytes == 0
        || union.max_storage_bytes > MAX_MULTI_HINGE_UNION_STORAGE_BYTES_V2
    {
        return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::InvalidLimits);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_composition_binding<'a>(
    geometry: &MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    split_sign: &ExactCommonSplitPairEffectiveGeneratorSignV1,
    gaps: &MultiHingeReliefUnionGapReportV2,
    union: &'a MultiHingeReliefUnionCertificateV2,
    thickness_mm: f64,
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
    meter: &mut MeterV1,
) -> Result<CompoundCorridorCompositionBindingV2<'a>, SplitHingeUnionExteriorReliefAssumptionErrorV1>
{
    meter.charge_work(32)?;
    if !thickness_mm.is_finite()
        || thickness_mm <= 0.0
        || !gaps.issuer.same_instance(geometry)
        || !union.issuer.same_instance(geometry)
        || gaps.fixed_face != fixed_face
        || union.fixed_face != fixed_face
        || gaps.geometry_hash != schedule.graph_binding_fingerprint_v1()
        || union.geometry_hash != gaps.geometry_hash
        || gaps.schedule_hash != schedule.certificate_binding_fingerprint_v2()
        || union.schedule_hash != gaps.schedule_hash
        || gaps.thickness_bits != thickness_mm.to_bits()
        || union.thickness_bits != gaps.thickness_bits
        || gaps.limits != union_limits
        || union.limits != union_limits
        || union.policy_limits != policy_limits
        || union.gap_hash != gaps.content_hash
        || gaps.gaps.len() != 1
        || union.covered.len() != 1
        || union.compound_corridors.len() != 1
    {
        return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::InvalidBinding);
    }
    let gap = &gaps.gaps[0];
    let covered = &union.covered[0];
    let compound = union.compound_corridors[0].composition_binding_v2();
    let edges = split_sign.edge_ids();
    if edges.len() > limits.max_edges {
        return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit);
    }
    if !(MIN_EDGES_V1..=MAX_EDGES_V1).contains(&edges.len())
        || split_sign.fixed_face() != fixed_face
        || split_sign.face_pair() != gap.pair
        || covered.pair != gap.pair
        || compound.pair != gap.pair
        || common_profile.edge_ids().len() != edges.len()
        || !same_edge_set(edges, common_profile.edge_ids())
        || !same_mapped_edge_set(edges, &gap.hinges, |item| item.hinge)
        || !same_edge_set(edges, &covered.hinges)
        || !same_edge_set(edges, compound.hinges)
        || compound.schedule_hash != schedule.certificate_binding_fingerprint_v2()
        || compound.binding.thickness_bits != thickness_mm.to_bits()
    {
        return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::MissingCompoundCorridor);
    }
    Ok(compound)
}

fn validate_angle_domain(
    schedule: &CanonicalCycleScheduleV1,
    edges: &[EdgeId],
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
    meter: &mut MeterV1,
) -> Result<(u64, u64), SplitHingeUnionExteriorReliefAssumptionErrorV1> {
    let temporary = edges
        .len()
        .checked_mul(ID_BYTES_V1 + 3 * WORD_BYTES_V1)
        .ok_or(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)?;
    meter.begin_temporary(temporary)?;
    let result = (|| {
        let boxes = schedule
            .evaluate_angle_box_dyadic(0, 0, limits.schedule_limits)
            .map_err(|_| SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain)?;
        meter.charge_work(boxes.len())?;
        if boxes.len() != edges.len() || !same_mapped_edge_set(edges, &boxes, |(edge, _)| *edge) {
            return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain);
        }
        let Some((_, first)) = boxes.first() else {
            return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain);
        };
        let common = (first.lower().to_bits(), first.upper().to_bits());
        if boxes
            .iter()
            .any(|(_, interval)| (interval.lower().to_bits(), interval.upper().to_bits()) != common)
            || !first.lower().is_finite()
            || !first.upper().is_finite()
            || first.lower() <= 0.0
            || first.upper() > 90.0
        {
            return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain);
        }
        Ok(common)
    })();
    meter.end_temporary(temporary);
    result
}

struct ExactReliefValuesV1 {
    alpha_lower: BigRational,
    alpha_upper: BigRational,
    radius: BigRational,
    thickness: BigRational,
    relief_left: BigRational,
    relief_right: BigRational,
}

fn validate_relief_and_geometry(
    geometry: &MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    thickness_mm: f64,
    binding: CompoundCorridorCompositionBindingV2<'_>,
    angle: (u64, u64),
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
    meter: &mut MeterV1,
) -> Result<
    (ExactCorridorGeometryV1, ExactReliefValuesV1),
    SplitHingeUnionExteriorReliefAssumptionErrorV1,
> {
    let alpha_lower = exact_from_f64(f64::from_bits(angle.0), meter)?;
    let alpha_upper = exact_from_f64(f64::from_bits(angle.1), meter)?;
    let radius = exact_from_f64(f64::from_bits(binding.binding.radial_depth_bits), meter)?;
    let thickness = exact_from_f64(thickness_mm, meter)?;
    if thickness <= BigRational::zero() || radius < thickness {
        return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ReliefInequality);
    }
    let sixty = exact_from_f64(60.0, meter)?;
    let relief_left = exact_mul(&radius, &alpha_lower, meter)?;
    let relief_right = exact_mul(&sixty, &thickness, meter)?;
    if relief_left < relief_right {
        return Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ReliefInequality);
    }
    let geometry_evidence = validate_exact_corridor_geometry_v1(
        geometry,
        binding.pair,
        fixed_face,
        binding.lower_bits,
        binding.upper_bits,
        binding.binding.radial_depth_bits,
        limits,
        meter,
    )?;
    Ok((
        geometry_evidence,
        ExactReliefValuesV1 {
            alpha_lower,
            alpha_upper,
            radius,
            thickness,
            relief_left,
            relief_right,
        },
    ))
}

fn same_edge_set(left: &[EdgeId], right: &[EdgeId]) -> bool {
    same_mapped_edge_set(left, right, |edge| *edge)
}

fn same_mapped_edge_set<T>(left: &[EdgeId], right: &[T], edge_of: impl Fn(&T) -> EdgeId) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|edge| right.iter().any(|item| edge_of(item) == *edge))
        && left
            .iter()
            .enumerate()
            .all(|(index, edge)| !left[..index].contains(edge))
        && right.iter().enumerate().all(|(index, item)| {
            !right[..index]
                .iter()
                .any(|previous| edge_of(previous) == edge_of(item))
        })
}

fn retained_bytes_v1(
    edge_count: usize,
) -> Result<usize, SplitHingeUnionExteriorReliefAssumptionErrorV1> {
    edge_count
        .checked_mul(ID_BYTES_V1 * 2)
        .and_then(|edges| {
            ((REQUIRED_FACES_V1 + 1) * 2)
                .checked_mul(ID_BYTES_V1)
                .and_then(|faces| edges.checked_add(faces))
        })
        .and_then(|bytes| bytes.checked_add(SIGN_BYTES_V1 * 2))
        .and_then(|bytes| bytes.checked_add(INSTANCE_ANCHOR_BYTES_V1))
        .and_then(|bytes| {
            HASH_BYTES_V1
                .checked_mul(11)
                .and_then(|hashes| bytes.checked_add(hashes))
        })
        .and_then(|bytes| {
            WORD_BYTES_V1
                .checked_mul(RETAINED_WORDS_V1)
                .and_then(|words| bytes.checked_add(words))
        })
        .ok_or(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
}

#[cfg(test)]
mod internal_tests {
    use super::*;

    #[test]
    fn retained_size_helper_fails_closed_on_overflow() {
        assert_eq!(retained_bytes_v1(2), Ok(914));
        assert_eq!(retained_bytes_v1(3), Ok(946));
        assert_eq!(
            retained_bytes_v1(usize::MAX),
            Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
        );
    }
}
