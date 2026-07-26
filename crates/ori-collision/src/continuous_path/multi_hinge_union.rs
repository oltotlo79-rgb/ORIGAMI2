//! Bounded evidence for unions of multiple shared-hinge relief corridors.
//!
//! The V1 corridor contract accepts exactly one shared hinge per face pair.
//! This V2 slice keeps that contract intact and proves a narrower fact for a
//! pair sharing two or three hinges: every canonical hinge-local relief
//! neighbourhood is present in one complete union. It is not a whole-path CCD
//! proof and grants no collision-free or mutation authority.

use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CanonicalCycleScheduleV1, MaterialHingeGraphAudit, MaterialHingeGraphGeometry, TreeHinge,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    HingeReliefLinearAngleScheduleV1, HingeReliefPolicyLimitsV1, HingeReliefPolicyRecordV1,
    NativeHingeReliefLocalIntervalCertificateV1, NativeHingeReliefPrerequisiteV1,
    revalidate_hinge_relief_local_intervals_v1,
};

pub const MULTI_HINGE_RELIEF_UNION_GAP_MODEL_ID_V2: &str = "multi_hinge_relief_union_gap_v2";
pub const MULTI_HINGE_RELIEF_UNION_CERTIFICATE_MODEL_ID_V2: &str =
    "multi_hinge_relief_union_certificate_v2";
pub const MAX_MULTI_HINGE_UNION_PAIRS_V2: usize = 64;
pub const MAX_MULTI_HINGES_PER_FACE_PAIR_V2: usize = 3;
pub const MAX_MULTI_HINGE_UNION_HINGES_V2: usize = 192;
pub const MAX_MULTI_HINGE_UNION_GEOMETRY_HINGES_V2: usize = 256;
pub const MAX_MULTI_HINGE_UNION_WORK_V2: usize = 1_000_000;
pub const MAX_MULTI_HINGE_UNION_STORAGE_BYTES_V2: usize = 1_048_576;

const ID_BYTES: usize = 16;
const WORD_BYTES: usize = 8;
const MEMBERSHIP_BYTES: usize = ID_BYTES * 3 + WORD_BYTES;
const ANGLE_BYTES: usize = ID_BYTES + WORD_BYTES;
const GAP_BASE_BYTES: usize = 32 * 3 + WORD_BYTES * 7;
const GAP_PAIR_BYTES: usize = ID_BYTES * 2 + WORD_BYTES;
const GAP_HINGE_BYTES: usize = ID_BYTES + WORD_BYTES * 3;
const CERTIFICATE_BASE_BYTES: usize = 32 * 4 + WORD_BYTES * 7;
const COVERED_PAIR_BYTES: usize = ID_BYTES * 2 + WORD_BYTES;
const EXPECTED_HINGE_BYTES: usize = ID_BYTES * 3 + WORD_BYTES * 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiHingeReliefUnionLimitsV2 {
    pub max_pairs: usize,
    pub max_hinges_per_pair: usize,
    pub max_total_union_hinges: usize,
    pub max_geometry_hinges: usize,
    /// Conservative deterministic operation budget, including sorting.
    pub max_work: usize,
    /// Canonical retained-plus-scratch byte budget. It is intentionally
    /// independent of allocator layout and therefore stable across targets.
    pub max_storage_bytes: usize,
}

impl Default for MultiHingeReliefUnionLimitsV2 {
    fn default() -> Self {
        Self {
            max_pairs: MAX_MULTI_HINGE_UNION_PAIRS_V2,
            max_hinges_per_pair: MAX_MULTI_HINGES_PER_FACE_PAIR_V2,
            max_total_union_hinges: MAX_MULTI_HINGE_UNION_HINGES_V2,
            max_geometry_hinges: MAX_MULTI_HINGE_UNION_GEOMETRY_HINGES_V2,
            max_work: MAX_MULTI_HINGE_UNION_WORK_V2,
            max_storage_bytes: MAX_MULTI_HINGE_UNION_STORAGE_BYTES_V2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MultiHingeReliefUnionErrorV2 {
    #[error("multi-hinge relief-union limits are invalid")]
    InvalidLimits,
    #[error("multi-hinge relief-union binding is invalid or stale")]
    InvalidBinding,
    #[error("multi-hinge relief-union work or storage exceeded its bound")]
    ResourceLimit,
    #[error("multi-hinge relief-union analysis was cancelled")]
    Cancelled,
    #[error("multi-hinge relief-union local relief evidence is foreign")]
    ForeignRelief,
    #[error("multi-hinge relief-union coverage is missing, duplicated, or noncanonical")]
    IncompleteCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiHingeReliefUnionHingeGapV2 {
    hinge: EdgeId,
    source_angle_bits: u64,
    target_angle_bits: u64,
    derivative_bound_bits: u64,
}

impl MultiHingeReliefUnionHingeGapV2 {
    #[must_use]
    pub const fn hinge(&self) -> EdgeId {
        self.hinge
    }
    #[must_use]
    pub const fn source_angle_bits(&self) -> u64 {
        self.source_angle_bits
    }
    #[must_use]
    pub const fn target_angle_bits(&self) -> u64 {
        self.target_angle_bits
    }
    #[must_use]
    pub const fn derivative_bound_bits(&self) -> u64 {
        self.derivative_bound_bits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiHingeReliefUnionGapV2 {
    pair: [FaceId; 2],
    hinges: Vec<MultiHingeReliefUnionHingeGapV2>,
}

impl MultiHingeReliefUnionGapV2 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub fn hinges(&self) -> &[MultiHingeReliefUnionHingeGapV2] {
        &self.hinges
    }
}

#[derive(Debug, Clone)]
pub struct MultiHingeReliefUnionGapReportV2 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    geometry_hash: [u8; 32],
    schedule_hash: [u8; 32],
    thickness_bits: u64,
    limits: MultiHingeReliefUnionLimitsV2,
    work_used: usize,
    retained_storage_bytes: usize,
    peak_storage_bytes: usize,
    gaps: Vec<MultiHingeReliefUnionGapV2>,
    content_hash: [u8; 32],
}

impl MultiHingeReliefUnionGapReportV2 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        MULTI_HINGE_RELIEF_UNION_GAP_MODEL_ID_V2
    }
    #[must_use]
    pub const fn geometry_hash_v2(&self) -> [u8; 32] {
        self.geometry_hash
    }
    #[must_use]
    pub const fn schedule_hash_v2(&self) -> [u8; 32] {
        self.schedule_hash
    }
    #[must_use]
    pub const fn content_hash_v2(&self) -> [u8; 32] {
        self.content_hash
    }
    #[must_use]
    pub const fn work_used(&self) -> usize {
        self.work_used
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
    pub fn gaps(&self) -> &[MultiHingeReliefUnionGapV2] {
        &self.gaps
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
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
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        thickness_mm: f64,
        limits: MultiHingeReliefUnionLimitsV2,
    ) -> bool {
        diagnose_multi_hinge_relief_union_gaps_v2(
            geometry,
            audit,
            fixed_face,
            schedule,
            thickness_mm,
            limits,
        )
        .is_ok_and(|fresh| self.same_evidence(&fresh))
    }
    fn same_evidence(&self, other: &Self) -> bool {
        self.issuer.same_instance(&other.issuer)
            && self.fixed_face == other.fixed_face
            && self.geometry_hash == other.geometry_hash
            && self.schedule_hash == other.schedule_hash
            && self.thickness_bits == other.thickness_bits
            && self.limits == other.limits
            && self.work_used == other.work_used
            && self.retained_storage_bytes == other.retained_storage_bytes
            && self.peak_storage_bytes == other.peak_storage_bytes
            && self.gaps == other.gaps
            && self.content_hash == other.content_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiHingeReliefUnionCoveredPairV2 {
    pair: [FaceId; 2],
    hinges: Vec<EdgeId>,
}

impl MultiHingeReliefUnionCoveredPairV2 {
    #[must_use]
    pub const fn pair(&self) -> [FaceId; 2] {
        self.pair
    }
    #[must_use]
    pub fn hinges(&self) -> &[EdgeId] {
        &self.hinges
    }
}

/// Opaque, non-authorizing proof that the exact local-relief records cover the
/// union of every reported shared-hinge neighbourhood.
///
/// Elevating this prerequisite to continuous clearance requires a separate
/// bounded primitive that proves interval separation of both thick face
/// prisms outside this union on every certified path leaf. No such
/// union-exterior separation API exists in V2, so this value must remain
/// ineligible for collision admission.
#[derive(Debug, Clone)]
pub struct MultiHingeReliefUnionCertificateV2 {
    issuer: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    geometry_hash: [u8; 32],
    schedule_hash: [u8; 32],
    thickness_bits: u64,
    gap_hash: [u8; 32],
    policy_limits: HingeReliefPolicyLimitsV1,
    limits: MultiHingeReliefUnionLimitsV2,
    work_used: usize,
    retained_storage_bytes: usize,
    peak_storage_bytes: usize,
    covered: Vec<MultiHingeReliefUnionCoveredPairV2>,
    content_hash: [u8; 32],
}

impl MultiHingeReliefUnionCertificateV2 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        MULTI_HINGE_RELIEF_UNION_CERTIFICATE_MODEL_ID_V2
    }
    #[must_use]
    pub const fn content_hash_v2(&self) -> [u8; 32] {
        self.content_hash
    }
    #[must_use]
    pub const fn work_used(&self) -> usize {
        self.work_used
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
    pub fn covered(&self) -> &[MultiHingeReliefUnionCoveredPairV2] {
        &self.covered
    }
    /// This proves set-completeness of local relief evidence, not pair
    /// separation over an open motion interval.
    #[must_use]
    pub const fn covers_every_reported_hinge_neighbourhood(&self) -> bool {
        true
    }
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
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
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_persistence(&self) -> bool {
        false
    }
    fn same_evidence(&self, other: &Self) -> bool {
        self.issuer.same_instance(&other.issuer)
            && self.fixed_face == other.fixed_face
            && self.geometry_hash == other.geometry_hash
            && self.schedule_hash == other.schedule_hash
            && self.thickness_bits == other.thickness_bits
            && self.gap_hash == other.gap_hash
            && self.policy_limits == other.policy_limits
            && self.limits == other.limits
            && self.work_used == other.work_used
            && self.retained_storage_bytes == other.retained_storage_bytes
            && self.peak_storage_bytes == other.peak_storage_bytes
            && self.covered == other.covered
            && self.content_hash == other.content_hash
    }
}

#[derive(Debug, Clone, Copy)]
struct Membership {
    pair: [FaceId; 2],
    hinge_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct ExpectedHinge {
    gap: MultiHingeReliefUnionHingeGapV2,
}

struct Meter {
    limits: MultiHingeReliefUnionLimitsV2,
    work: usize,
    storage: usize,
    peak: usize,
}

impl Meter {
    fn new(limits: MultiHingeReliefUnionLimitsV2) -> Result<Self, MultiHingeReliefUnionErrorV2> {
        validate_limits(limits)?;
        Ok(Self {
            limits,
            work: 0,
            storage: 0,
            peak: 0,
        })
    }
    fn work(&mut self, amount: usize) -> Result<(), MultiHingeReliefUnionErrorV2> {
        self.work = self
            .work
            .checked_add(amount)
            .filter(|value| *value <= self.limits.max_work)
            .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
        Ok(())
    }
    fn retain(&mut self, amount: usize) -> Result<(), MultiHingeReliefUnionErrorV2> {
        self.storage = self
            .storage
            .checked_add(amount)
            .filter(|value| *value <= self.limits.max_storage_bytes)
            .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
        self.peak = self.peak.max(self.storage);
        Ok(())
    }
    fn release(&mut self, amount: usize) -> Result<(), MultiHingeReliefUnionErrorV2> {
        self.storage = self
            .storage
            .checked_sub(amount)
            .ok_or(MultiHingeReliefUnionErrorV2::InvalidBinding)?;
        Ok(())
    }
}

fn validate_limits(
    limits: MultiHingeReliefUnionLimitsV2,
) -> Result<(), MultiHingeReliefUnionErrorV2> {
    if limits.max_pairs == 0
        || limits.max_pairs > MAX_MULTI_HINGE_UNION_PAIRS_V2
        || !(2..=MAX_MULTI_HINGES_PER_FACE_PAIR_V2).contains(&limits.max_hinges_per_pair)
        || limits.max_total_union_hinges == 0
        || limits.max_total_union_hinges > MAX_MULTI_HINGE_UNION_HINGES_V2
        || limits.max_geometry_hinges == 0
        || limits.max_geometry_hinges > MAX_MULTI_HINGE_UNION_GEOMETRY_HINGES_V2
        || limits.max_work == 0
        || limits.max_work > MAX_MULTI_HINGE_UNION_WORK_V2
        || limits.max_storage_bytes == 0
        || limits.max_storage_bytes > MAX_MULTI_HINGE_UNION_STORAGE_BYTES_V2
    {
        Err(MultiHingeReliefUnionErrorV2::InvalidLimits)
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn diagnose_multi_hinge_relief_union_gaps_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    thickness_mm: f64,
    limits: MultiHingeReliefUnionLimitsV2,
) -> Result<MultiHingeReliefUnionGapReportV2, MultiHingeReliefUnionErrorV2> {
    diagnose_multi_hinge_relief_union_gaps_with_cancel_v2(
        geometry,
        audit,
        fixed_face,
        schedule,
        thickness_mm,
        limits,
        || false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn diagnose_multi_hinge_relief_union_gaps_with_cancel_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    thickness_mm: f64,
    limits: MultiHingeReliefUnionLimitsV2,
    mut cancelled: impl FnMut() -> bool,
) -> Result<MultiHingeReliefUnionGapReportV2, MultiHingeReliefUnionErrorV2> {
    let mut meter = Meter::new(limits)?;
    if cancelled() {
        return Err(MultiHingeReliefUnionErrorV2::Cancelled);
    }
    if !thickness_mm.is_finite()
        || thickness_mm <= 0.0
        || !schedule.matches_binding(geometry, audit, fixed_face)
    {
        return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
    }
    if geometry.hinges().len() > limits.max_geometry_hinges {
        return Err(MultiHingeReliefUnionErrorV2::ResourceLimit);
    }
    meter.retain(GAP_BASE_BYTES)?;
    let membership_storage = geometry
        .hinges()
        .len()
        .checked_mul(MEMBERSHIP_BYTES)
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    let angle_storage = geometry
        .hinges()
        .len()
        .checked_mul(ANGLE_BYTES * 2)
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    meter.retain(membership_storage)?;
    meter.retain(angle_storage)?;
    let source = schedule
        .evaluate(0.0)
        .ok_or(MultiHingeReliefUnionErrorV2::InvalidBinding)?;
    let target = schedule
        .evaluate(1.0)
        .ok_or(MultiHingeReliefUnionErrorV2::InvalidBinding)?;
    if source.as_slice().len() != geometry.hinges().len()
        || target.as_slice().len() != geometry.hinges().len()
    {
        return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
    }
    meter.work(
        geometry
            .hinges()
            .len()
            .checked_mul(3)
            .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
    )?;

    let mut memberships = Vec::new();
    memberships
        .try_reserve_exact(geometry.hinges().len())
        .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    for (hinge_index, hinge) in geometry.hinges().iter().enumerate() {
        if cancelled() {
            return Err(MultiHingeReliefUnionErrorV2::Cancelled);
        }
        meter.work(1)?;
        memberships.push(Membership {
            pair: canonical_pair(hinge.left_face(), hinge.right_face())?,
            hinge_index,
        });
    }
    meter.work(sort_work(memberships.len())?)?;
    memberships.sort_unstable_by(|left, right| {
        compare_pair(left.pair, right.pair).then_with(|| {
            geometry.hinges()[left.hinge_index]
                .edge()
                .canonical_bytes()
                .cmp(
                    &geometry.hinges()[right.hinge_index]
                        .edge()
                        .canonical_bytes(),
                )
        })
    });

    let mut pair_count = 0_usize;
    let mut preflight_total = 0_usize;
    let mut preflight_start = 0_usize;
    while preflight_start < memberships.len() {
        if cancelled() {
            return Err(MultiHingeReliefUnionErrorV2::Cancelled);
        }
        let pair = memberships[preflight_start].pair;
        let mut preflight_end = preflight_start + 1;
        while memberships
            .get(preflight_end)
            .is_some_and(|membership| membership.pair == pair)
        {
            preflight_end += 1;
        }
        let count = preflight_end - preflight_start;
        meter.work(count)?;
        if count > 1 {
            if count > limits.max_hinges_per_pair || pair_count >= limits.max_pairs {
                return Err(MultiHingeReliefUnionErrorV2::ResourceLimit);
            }
            pair_count += 1;
            preflight_total = preflight_total
                .checked_add(count)
                .filter(|value| *value <= limits.max_total_union_hinges)
                .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
        }
        preflight_start = preflight_end;
    }
    let mut gaps = Vec::new();
    gaps.try_reserve_exact(pair_count)
        .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    let mut total = 0_usize;
    let mut start = 0_usize;
    while start < memberships.len() {
        if cancelled() {
            return Err(MultiHingeReliefUnionErrorV2::Cancelled);
        }
        let pair = memberships[start].pair;
        let mut end = start + 1;
        while memberships
            .get(end)
            .is_some_and(|membership| membership.pair == pair)
        {
            end += 1;
        }
        let count = end - start;
        meter.work(count)?;
        if count > 1 {
            if count > limits.max_hinges_per_pair || gaps.len() >= limits.max_pairs {
                return Err(MultiHingeReliefUnionErrorV2::ResourceLimit);
            }
            total = total
                .checked_add(count)
                .filter(|value| *value <= limits.max_total_union_hinges)
                .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
            meter.retain(
                GAP_PAIR_BYTES
                    .checked_add(
                        count
                            .checked_mul(GAP_HINGE_BYTES)
                            .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
                    )
                    .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
            )?;
            let mut hinges = Vec::new();
            hinges
                .try_reserve_exact(count)
                .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
            for membership in &memberships[start..end] {
                let hinge = &geometry.hinges()[membership.hinge_index];
                let derivative = schedule
                    .derivative_bound(hinge.edge())
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .ok_or(MultiHingeReliefUnionErrorV2::InvalidBinding)?;
                hinges.push(MultiHingeReliefUnionHingeGapV2 {
                    hinge: hinge.edge(),
                    source_angle_bits: find_angle(source.as_slice(), hinge.edge(), &mut meter)?,
                    target_angle_bits: find_angle(target.as_slice(), hinge.edge(), &mut meter)?,
                    derivative_bound_bits: derivative.to_bits(),
                });
            }
            if hinges
                .windows(2)
                .any(|pair| pair[0].hinge.canonical_bytes() >= pair[1].hinge.canonical_bytes())
            {
                return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
            }
            gaps.push(MultiHingeReliefUnionGapV2 { pair, hinges });
        }
        start = end;
    }
    if gaps.len() != pair_count || total != preflight_total {
        return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
    }
    let geometry_hash = schedule.graph_binding_fingerprint_v1();
    let schedule_hash = schedule.certificate_binding_fingerprint_v1();
    let content_hash = gap_hash(
        geometry,
        fixed_face,
        geometry_hash,
        schedule_hash,
        thickness_mm.to_bits(),
        limits,
        &gaps,
        &mut meter,
    )?;
    meter.release(membership_storage)?;
    meter.release(angle_storage)?;
    if cancelled() {
        return Err(MultiHingeReliefUnionErrorV2::Cancelled);
    }
    Ok(MultiHingeReliefUnionGapReportV2 {
        issuer: geometry.clone(),
        fixed_face,
        geometry_hash,
        schedule_hash,
        thickness_bits: thickness_mm.to_bits(),
        limits,
        work_used: meter.work,
        retained_storage_bytes: meter.storage,
        peak_storage_bytes: meter.peak,
        gaps,
        content_hash,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn certify_multi_hinge_relief_union_v2(
    gaps: &MultiHingeReliefUnionGapReportV2,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    thickness_mm: f64,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    local: &NativeHingeReliefLocalIntervalCertificateV1,
    policies: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    limits: MultiHingeReliefUnionLimitsV2,
) -> Result<MultiHingeReliefUnionCertificateV2, MultiHingeReliefUnionErrorV2> {
    certify_multi_hinge_relief_union_with_cancel_v2(
        gaps,
        geometry,
        audit,
        fixed_face,
        schedule,
        thickness_mm,
        prerequisite,
        local,
        policies,
        schedules,
        policy_limits,
        limits,
        || false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn certify_multi_hinge_relief_union_with_cancel_v2(
    gaps: &MultiHingeReliefUnionGapReportV2,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    thickness_mm: f64,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    local: &NativeHingeReliefLocalIntervalCertificateV1,
    policies: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    limits: MultiHingeReliefUnionLimitsV2,
    mut cancelled: impl FnMut() -> bool,
) -> Result<MultiHingeReliefUnionCertificateV2, MultiHingeReliefUnionErrorV2> {
    let mut meter = Meter::new(limits)?;
    if cancelled() {
        return Err(MultiHingeReliefUnionErrorV2::Cancelled);
    }
    let fresh = diagnose_multi_hinge_relief_union_gaps_with_cancel_v2(
        geometry,
        audit,
        fixed_face,
        schedule,
        thickness_mm,
        gaps.limits,
        &mut cancelled,
    )?;
    if !gaps.same_evidence(&fresh) {
        return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
    }
    revalidate_hinge_relief_local_intervals_v1(
        local,
        prerequisite,
        geometry,
        thickness_mm,
        policies,
        schedules,
        policy_limits,
    )
    .map_err(|_| MultiHingeReliefUnionErrorV2::ForeignRelief)?;

    meter.retain(CERTIFICATE_BASE_BYTES)?;
    let count = gaps
        .gaps
        .iter()
        .try_fold(0_usize, |sum, gap| sum.checked_add(gap.hinges.len()))
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    if count == 0
        || count > limits.max_total_union_hinges
        || count != policies.len()
        || count != schedules.len()
    {
        return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
    }
    let expected_storage = count
        .checked_mul(EXPECTED_HINGE_BYTES)
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    meter.retain(expected_storage)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(count)
        .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    for gap in &gaps.gaps {
        expected.extend(gap.hinges.iter().copied().map(|gap| ExpectedHinge { gap }));
    }
    meter.work(count)?;
    meter.work(sort_work(count)?)?;
    expected.sort_unstable_by_key(|entry| entry.gap.hinge.canonical_bytes());
    if expected
        .windows(2)
        .any(|pair| pair[0].gap.hinge == pair[1].gap.hinge)
    {
        return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
    }
    for ((expected, policy), local) in expected.iter().zip(policies).zip(schedules) {
        if cancelled() {
            return Err(MultiHingeReliefUnionErrorV2::Cancelled);
        }
        meter.work(1)?;
        let derivative = (local.target_angle_degrees - local.source_angle_degrees).abs();
        let constant =
            derivative == 0.0 && schedule.is_exact_constant_profile_v1(expected.gap.hinge);
        if policy.edge != expected.gap.hinge
            || local.edge != expected.gap.hinge
            || local.source_angle_degrees.to_bits() != expected.gap.source_angle_bits
            || local.target_angle_degrees.to_bits() != expected.gap.target_angle_bits
            || (!constant && derivative.to_bits() != expected.gap.derivative_bound_bits)
        {
            return Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage);
        }
    }

    let mut covered = Vec::new();
    covered
        .try_reserve_exact(gaps.gaps.len())
        .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
    for gap in &gaps.gaps {
        meter.retain(
            COVERED_PAIR_BYTES
                .checked_add(
                    gap.hinges
                        .len()
                        .checked_mul(ID_BYTES)
                        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
                )
                .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)?,
        )?;
        let mut hinges = Vec::new();
        hinges
            .try_reserve_exact(gap.hinges.len())
            .map_err(|_| MultiHingeReliefUnionErrorV2::ResourceLimit)?;
        hinges.extend(gap.hinges.iter().map(|entry| entry.hinge));
        covered.push(MultiHingeReliefUnionCoveredPairV2 {
            pair: gap.pair,
            hinges,
        });
    }
    let content_hash = certificate_hash(
        gaps,
        policies,
        schedules,
        policy_limits,
        limits,
        &covered,
        &mut meter,
    )?;
    meter.release(expected_storage)?;
    if cancelled() {
        return Err(MultiHingeReliefUnionErrorV2::Cancelled);
    }
    Ok(MultiHingeReliefUnionCertificateV2 {
        issuer: geometry.clone(),
        fixed_face,
        geometry_hash: gaps.geometry_hash,
        schedule_hash: gaps.schedule_hash,
        thickness_bits: gaps.thickness_bits,
        gap_hash: gaps.content_hash,
        policy_limits,
        limits,
        work_used: meter.work,
        retained_storage_bytes: meter.storage,
        peak_storage_bytes: meter.peak,
        covered,
        content_hash,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn revalidate_multi_hinge_relief_union_certificate_v2(
    certificate: &MultiHingeReliefUnionCertificateV2,
    gaps: &MultiHingeReliefUnionGapReportV2,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    thickness_mm: f64,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    local: &NativeHingeReliefLocalIntervalCertificateV1,
    policies: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    limits: MultiHingeReliefUnionLimitsV2,
) -> Result<(), MultiHingeReliefUnionErrorV2> {
    let fresh = certify_multi_hinge_relief_union_v2(
        gaps,
        geometry,
        audit,
        fixed_face,
        schedule,
        thickness_mm,
        prerequisite,
        local,
        policies,
        schedules,
        policy_limits,
        limits,
    )?;
    if certificate.same_evidence(&fresh) {
        Ok(())
    } else {
        Err(MultiHingeReliefUnionErrorV2::InvalidBinding)
    }
}

fn canonical_pair(
    first: FaceId,
    second: FaceId,
) -> Result<[FaceId; 2], MultiHingeReliefUnionErrorV2> {
    match first.canonical_bytes().cmp(&second.canonical_bytes()) {
        std::cmp::Ordering::Less => Ok([first, second]),
        std::cmp::Ordering::Greater => Ok([second, first]),
        std::cmp::Ordering::Equal => Err(MultiHingeReliefUnionErrorV2::InvalidBinding),
    }
}

fn compare_pair(left: [FaceId; 2], right: [FaceId; 2]) -> std::cmp::Ordering {
    left[0]
        .canonical_bytes()
        .cmp(&right[0].canonical_bytes())
        .then_with(|| left[1].canonical_bytes().cmp(&right[1].canonical_bytes()))
}

fn sort_work(count: usize) -> Result<usize, MultiHingeReliefUnionErrorV2> {
    if count < 2 {
        return Ok(count);
    }
    count
        .checked_mul((usize::BITS - (count - 1).leading_zeros()) as usize)
        .ok_or(MultiHingeReliefUnionErrorV2::ResourceLimit)
}

fn find_angle(
    angles: &[ori_kinematics::HingeAngle],
    edge: EdgeId,
    meter: &mut Meter,
) -> Result<u64, MultiHingeReliefUnionErrorV2> {
    meter.work(angles.len())?;
    let mut found = None;
    for angle in angles.iter().filter(|angle| angle.edge() == edge) {
        if found.replace(angle.angle_degrees().to_bits()).is_some() {
            return Err(MultiHingeReliefUnionErrorV2::InvalidBinding);
        }
    }
    found.ok_or(MultiHingeReliefUnionErrorV2::InvalidBinding)
}

#[allow(clippy::too_many_arguments)]
fn gap_hash(
    geometry: &MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    geometry_hash: [u8; 32],
    schedule_hash: [u8; 32],
    thickness_bits: u64,
    limits: MultiHingeReliefUnionLimitsV2,
    gaps: &[MultiHingeReliefUnionGapV2],
    meter: &mut Meter,
) -> Result<[u8; 32], MultiHingeReliefUnionErrorV2> {
    let mut hash = Sha256::new();
    hash.update(MULTI_HINGE_RELIEF_UNION_GAP_MODEL_ID_V2.as_bytes());
    hash.update(fixed_face.canonical_bytes());
    hash.update(geometry_hash);
    hash.update(schedule_hash);
    hash.update(thickness_bits.to_be_bytes());
    hash_limits(&mut hash, limits);
    hash.update((gaps.len() as u64).to_be_bytes());
    for gap in gaps {
        meter.work(gap.hinges.len())?;
        hash.update(gap.pair[0].canonical_bytes());
        hash.update(gap.pair[1].canonical_bytes());
        hash.update((gap.hinges.len() as u64).to_be_bytes());
        for item in &gap.hinges {
            let hinge = geometry
                .hinges()
                .iter()
                .find(|hinge| hinge.edge() == item.hinge)
                .ok_or(MultiHingeReliefUnionErrorV2::InvalidBinding)?;
            hash_hinge(&mut hash, hinge);
            hash.update(item.source_angle_bits.to_be_bytes());
            hash.update(item.target_angle_bits.to_be_bytes());
            hash.update(item.derivative_bound_bits.to_be_bytes());
        }
    }
    Ok(hash.finalize().into())
}

fn hash_hinge(hash: &mut Sha256, hinge: &TreeHinge) {
    hash.update(hinge.edge().canonical_bytes());
    hash.update(hinge.left_face().canonical_bytes());
    hash.update(hinge.right_face().canonical_bytes());
    hash.update([match hinge.assignment() {
        ori_topology::FoldAssignment::Mountain => 0x4d,
        ori_topology::FoldAssignment::Valley => 0x56,
    }]);
    for point in [hinge.start(), hinge.end(), hinge.axis()] {
        hash.update(point.x().to_bits().to_be_bytes());
        hash.update(point.y().to_bits().to_be_bytes());
        hash.update(point.z().to_bits().to_be_bytes());
    }
}

fn certificate_hash(
    gaps: &MultiHingeReliefUnionGapReportV2,
    policies: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
    policy_limits: HingeReliefPolicyLimitsV1,
    limits: MultiHingeReliefUnionLimitsV2,
    covered: &[MultiHingeReliefUnionCoveredPairV2],
    meter: &mut Meter,
) -> Result<[u8; 32], MultiHingeReliefUnionErrorV2> {
    let mut hash = Sha256::new();
    hash.update(MULTI_HINGE_RELIEF_UNION_CERTIFICATE_MODEL_ID_V2.as_bytes());
    hash.update(gaps.geometry_hash);
    hash.update(gaps.schedule_hash);
    hash.update(gaps.thickness_bits.to_be_bytes());
    hash.update(gaps.content_hash);
    hash.update((policy_limits.max_records as u64).to_be_bytes());
    hash_limits(&mut hash, limits);
    for (policy, schedule) in policies.iter().zip(schedules) {
        meter.work(1)?;
        hash.update(policy.edge.canonical_bytes());
        hash.update(policy.cutout_width_mm.to_bits().to_be_bytes());
        hash.update(policy.bevel_angle_degrees.to_bits().to_be_bytes());
        hash.update(policy.material_thickness_mm.to_bits().to_be_bytes());
        hash.update(schedule.source_angle_degrees.to_bits().to_be_bytes());
        hash.update(schedule.target_angle_degrees.to_bits().to_be_bytes());
    }
    for pair in covered {
        meter.work(pair.hinges.len())?;
        hash.update(pair.pair[0].canonical_bytes());
        hash.update(pair.pair[1].canonical_bytes());
        hash.update((pair.hinges.len() as u64).to_be_bytes());
        for hinge in &pair.hinges {
            hash.update(hinge.canonical_bytes());
        }
    }
    Ok(hash.finalize().into())
}

fn hash_limits(hash: &mut Sha256, limits: MultiHingeReliefUnionLimitsV2) {
    for value in [
        limits.max_pairs,
        limits.max_hinges_per_pair,
        limits.max_total_union_hinges,
        limits.max_geometry_hinges,
        limits.max_work,
        limits.max_storage_bytes,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
}

#[cfg(test)]
#[path = "multi_hinge_union_tests.rs"]
mod tests;
