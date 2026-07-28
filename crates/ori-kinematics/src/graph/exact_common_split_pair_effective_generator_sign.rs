use num_bigint::Sign;
use num_rational::BigRational;
use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    MaterialHingeGraphAudit,
    exact_common_effective_generator_sign::EffectiveGeneratorSignV1,
    exact_generator_word::{CanonicalInfiniteLineV1, exact_generator_line_v1},
};
use crate::{
    CanonicalCycleScheduleV1, ExactCommonLinearCycleProfileErrorV1,
    ExactCommonLinearCycleProfileLimitsV1, ExactCommonLinearCycleProfileV1,
    MaterialHingeGraphGeometry,
    schedule::{
        EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1, ExactCommonLinearCompositionBindingV1,
    },
    tree::MaterialHingeGraphInstanceV1,
};

/// Frozen model identifier for the logical split-pair orientation proof.
pub const EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1: &str =
    "exact_common_split_pair_effective_generator_sign_v1";

const MIN_EDGES_V1: usize = 2;
const MAX_EDGES_V1: usize = 3;
const REQUIRED_FACES_V1: usize = 2;
const EDGE_BYTES_V1: usize = 16;
const FACE_BYTES_V1: usize = 16;
const FINGERPRINT_BYTES_V1: usize = 32;
const SIGN_BYTES_V1: usize = 1;
const INSTANCE_ANCHOR_BYTES_V1: usize = 16;
const SHA256_SCRATCH_BYTES_V1: usize = 104;
// Logical cross-runtime envelopes. These include canonical audit-partition
// records, sorted geometry indexes, and per-edge orientation witnesses.
const TEMPORARY_BYTES_PER_FACE_V1: usize = 32;
const TEMPORARY_BYTES_PER_EDGE_V1: usize = 192;
// Every exact rational comes from finite binary64 hinge geometry. This fixed
// envelope conservatively covers all exact Pluecker intermediates and hashes.
const EXACT_LINE_SCRATCH_BYTES_V1: usize = 64 * 1024;
const EXACT_LINE_WORK_PER_EDGE_V1: usize = 8 * 1024;
const STRUCTURE_WORK_PER_EDGE_V1: usize = 48;
const STRUCTURE_WORK_PER_FACE_V1: usize = 16;

/// Explicit resource envelope for the complete logical split-pair proof.
///
/// The nested profile limits independently bound fresh upstream proof
/// revalidation. Storage limits count canonical logical payload bytes rather
/// than target-dependent Rust collection metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
    pub profile_limits: ExactCommonLinearCycleProfileLimitsV1,
    pub max_edges: usize,
    pub max_faces: usize,
    pub max_work: usize,
    pub max_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

impl Default for ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
    fn default() -> Self {
        Self {
            profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
            max_edges: MAX_EDGES_V1,
            max_faces: REQUIRED_FACES_V1,
            max_work: 96 * 1024,
            max_retained_bytes: retained_bytes_v1(MAX_EDGES_V1).unwrap_or(usize::MAX),
            max_peak_bytes: 128 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ExactCommonSplitPairEffectiveGeneratorSignErrorV1 {
    #[error("the split-pair effective-generator-sign input is malformed")]
    InvalidInput,
    #[error("the common linear profile was not issued by this schedule")]
    ProfileIssuerMismatch,
    #[error("the schedule is not bound to this graph and fixed face")]
    GraphBindingMismatch,
    #[error("the common profile does not exactly cover the audited split hinges")]
    CarrierSetMismatch,
    #[error("the carrier is not one strict two-face split pair")]
    UnsupportedSplitPair,
    #[error("the split hinges do not use one exact infinite line")]
    NonCollinearCarrier,
    #[error("the fixed-side effective generator signs are not identical")]
    EffectiveSignMismatch,
    #[error("the split-pair proof exceeds its explicit resource limits")]
    ResourceLimit,
    #[error("the proof was not issued by these exact inputs")]
    IssuerMismatch,
}

/// Opaque recognition evidence for one logical quotient edge represented by
/// two or three physical hinges between the same two material faces.
///
/// Exactly one audited hinge must be spanning and every other hinge must be a
/// closure edge. All hinges must carry one freshly revalidated common linear
/// profile, use one exact infinite carrier, and agree on the fixed-side
/// effective generator sign.
///
/// This is nonauthority evidence. It grants no closure, continuous-motion,
/// collision, simulation, persistence, apply, or project-mutation authority.
///
/// ```compile_fail
/// use ori_kinematics::ExactCommonSplitPairEffectiveGeneratorSignV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ExactCommonSplitPairEffectiveGeneratorSignV1>();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactCommonSplitPairEffectiveGeneratorSignV1 {
    canonical_edges: Vec<EdgeId>,
    canonical_face_pair: [FaceId; REQUIRED_FACES_V1],
    fixed_face: FaceId,
    common_effective_sign: EffectiveGeneratorSignV1,
    issuer_geometry_instance_v1: MaterialHingeGraphInstanceV1,
    issuer_schedule_fingerprint_v2: [u8; FINGERPRINT_BYTES_V1],
    issuer_graph_binding_fingerprint_v1: [u8; FINGERPRINT_BYTES_V1],
    issuer_common_profile_fingerprint_v1: [u8; FINGERPRINT_BYTES_V1],
    proof_fingerprint_v1: [u8; FINGERPRINT_BYTES_V1],
}

impl ExactCommonSplitPairEffectiveGeneratorSignV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1
    }

    #[must_use]
    pub fn edge_ids(&self) -> &[EdgeId] {
        &self.canonical_edges
    }

    #[must_use]
    pub const fn face_pair(&self) -> [FaceId; 2] {
        self.canonical_face_pair
    }

    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub fn moving_face(&self) -> FaceId {
        if self.canonical_face_pair[0] == self.fixed_face {
            self.canonical_face_pair[1]
        } else {
            self.canonical_face_pair[0]
        }
    }

    #[must_use]
    pub const fn common_effective_sign(&self) -> EffectiveGeneratorSignV1 {
        self.common_effective_sign
    }

    /// Recomputes the entire bounded proof and additionally requires the
    /// original prepared material-graph instance. A separately prepared,
    /// deeply equal graph is not the same issuer. Revalidation bills the
    /// identity check and complete proof comparison in addition to producer
    /// work.
    pub fn revalidate_issuers_v1(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        common_profile: &ExactCommonLinearCycleProfileV1,
        limits: ExactCommonSplitPairEffectiveGeneratorSignLimitsV1,
    ) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
        let mut meter = MeterV1::new(limits);
        meter.charge_work(1)?;
        if !self.issuer_geometry_instance_v1.matches(geometry) {
            return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::IssuerMismatch);
        }
        let candidate = prove_with_meter_v1(
            geometry,
            audit,
            fixed_face,
            schedule,
            common_profile,
            &mut meter,
        )?;
        meter.charge_work(
            retained_bytes_v1(self.canonical_edges.len())?
                .checked_add(1)
                .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?,
        )?;
        if &candidate == self {
            Ok(())
        } else {
            Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::IssuerMismatch)
        }
    }

    #[must_use]
    pub const fn authorizes_closure(&self) -> bool {
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
}

#[derive(Debug)]
struct MeterV1 {
    limits: ExactCommonSplitPairEffectiveGeneratorSignLimitsV1,
    work: usize,
    retained_bytes: usize,
    temporary_bytes: usize,
    peak_bytes: usize,
}

impl MeterV1 {
    const fn new(limits: ExactCommonSplitPairEffectiveGeneratorSignLimitsV1) -> Self {
        Self {
            limits,
            work: 0,
            retained_bytes: 0,
            temporary_bytes: 0,
            peak_bytes: 0,
        }
    }

    fn charge_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        if self.work > self.limits.max_work {
            return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit);
        }
        Ok(())
    }

    fn retain(
        &mut self,
        amount: usize,
    ) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(amount)
            .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        if self.retained_bytes > self.limits.max_retained_bytes {
            return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit);
        }
        self.update_peak()
    }

    fn begin_temporary(
        &mut self,
        amount: usize,
    ) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_add(amount)
            .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        self.update_peak()
    }

    fn end_temporary(&mut self, amount: usize) {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_sub(amount)
            .expect("internal split-pair temporary storage must balance");
    }

    fn update_peak(&mut self) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
        let current = self
            .retained_bytes
            .checked_add(self.temporary_bytes)
            .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        self.peak_bytes = self.peak_bytes.max(current);
        if self.peak_bytes > self.limits.max_peak_bytes {
            return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SplitWitnessV1 {
    edge: EdgeId,
    left_face: FaceId,
    right_face: FaceId,
    is_spanning: bool,
    fixed_side_sign: i8,
    line_generator_sign: i8,
    effective_sign: i8,
}

fn retained_bytes_v1(
    edge_count: usize,
) -> Result<usize, ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
    edge_count
        .checked_mul(EDGE_BYTES_V1)
        .and_then(|edges| {
            FACE_BYTES_V1
                .checked_mul(REQUIRED_FACES_V1 + 1)
                .and_then(|faces| edges.checked_add(faces))
        })
        .and_then(|bytes| bytes.checked_add(SIGN_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(INSTANCE_ANCHOR_BYTES_V1))
        .and_then(|bytes| {
            FINGERPRINT_BYTES_V1
                .checked_mul(4)
                .and_then(|fingerprints| bytes.checked_add(fingerprints))
        })
        .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
}

fn temporary_bytes_v1(
    face_count: usize,
    edge_count: usize,
) -> Result<usize, ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
    let faces = face_count
        .checked_mul(TEMPORARY_BYTES_PER_FACE_V1)
        .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    let edges = edge_count
        .checked_mul(TEMPORARY_BYTES_PER_EDGE_V1)
        .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    faces
        .checked_add(edges)
        .and_then(|bytes| bytes.checked_add(EXACT_LINE_SCRATCH_BYTES_V1))
        .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
}

fn map_profile_error_v1(
    error: ExactCommonLinearCycleProfileErrorV1,
) -> ExactCommonSplitPairEffectiveGeneratorSignErrorV1 {
    if error == ExactCommonLinearCycleProfileErrorV1::ResourceLimit {
        ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit
    } else {
        ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ProfileIssuerMismatch
    }
}

fn sign_tag_v1(value: i8) -> Option<u8> {
    match value {
        -1 => Some(0),
        1 => Some(1),
        _ => None,
    }
}

const fn effective_sign_from_i8_v1(value: i8) -> Option<EffectiveGeneratorSignV1> {
    match value {
        -1 => Some(EffectiveGeneratorSignV1::Negative),
        1 => Some(EffectiveGeneratorSignV1::Positive),
        _ => None,
    }
}

const fn effective_sign_tag_v1(value: EffectiveGeneratorSignV1) -> u8 {
    match value {
        EffectiveGeneratorSignV1::Negative => 0,
        EffectiveGeneratorSignV1::Positive => 1,
    }
}

fn canonical_pair_v1(first: FaceId, second: FaceId) -> Option<[FaceId; REQUIRED_FACES_V1]> {
    if first == second {
        return None;
    }
    if first.canonical_bytes() < second.canonical_bytes() {
        Some([first, second])
    } else {
        Some([second, first])
    }
}

fn hash_frame_v1(
    hash: &mut Sha256,
    value: &[u8],
    meter: &mut MeterV1,
) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
    let length = u64::try_from(value.len())
        .map_err(|_| ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    meter.charge_work(8)?;
    meter.charge_work(value.len())?;
    hash.update(length.to_be_bytes());
    hash.update(value);
    Ok(())
}

fn hash_rational_v1(
    hash: &mut Sha256,
    value: &BigRational,
    meter: &mut MeterV1,
) -> Result<(), ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
    let (numerator_sign, numerator) = value.numer().to_bytes_be();
    let (_, denominator) = value.denom().to_bytes_be();
    let sign = match numerator_sign {
        Sign::Minus => 0,
        Sign::NoSign => 1,
        Sign::Plus => 2,
    };
    hash_frame_v1(hash, &[sign], meter)?;
    hash_frame_v1(hash, &numerator, meter)?;
    hash_frame_v1(hash, &denominator, meter)
}

struct ProofFingerprintInputV1<'a> {
    edges: &'a [EdgeId],
    canonical_face_pair: [FaceId; REQUIRED_FACES_V1],
    fixed_face: FaceId,
    common_sign: EffectiveGeneratorSignV1,
    line: &'a CanonicalInfiniteLineV1,
    witnesses: &'a [SplitWitnessV1],
    composition: ExactCommonLinearCompositionBindingV1,
}

fn proof_fingerprint_v1(
    input: ProofFingerprintInputV1<'_>,
    meter: &mut MeterV1,
) -> Result<[u8; FINGERPRINT_BYTES_V1], ExactCommonSplitPairEffectiveGeneratorSignErrorV1> {
    const DOMAIN_SEPARATOR: &[u8] =
        b"ORIGAMI2_EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_PROOF_V1";
    meter.begin_temporary(SHA256_SCRATCH_BYTES_V1)?;
    let result = (|| {
        let mut hash = Sha256::new();
        hash_frame_v1(&mut hash, DOMAIN_SEPARATOR, meter)?;
        hash_frame_v1(
            &mut hash,
            EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        hash_frame_v1(
            &mut hash,
            EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        hash_frame_v1(&mut hash, &input.composition.schedule_fingerprint_v2, meter)?;
        hash_frame_v1(
            &mut hash,
            &input.composition.graph_binding_fingerprint_v1,
            meter,
        )?;
        hash_frame_v1(&mut hash, &input.composition.proof_fingerprint_v1, meter)?;
        for face in input.canonical_face_pair {
            hash_frame_v1(&mut hash, &face.canonical_bytes(), meter)?;
        }
        hash_frame_v1(&mut hash, &input.fixed_face.canonical_bytes(), meter)?;
        hash_frame_v1(
            &mut hash,
            &u64::try_from(input.edges.len())
                .map_err(|_| ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for edge in input.edges {
            hash_frame_v1(&mut hash, &edge.canonical_bytes(), meter)?;
        }
        for bits in input.line.direction_bits() {
            hash_frame_v1(&mut hash, &bits.to_be_bytes(), meter)?;
        }
        for moment in input.line.exact_moment() {
            hash_rational_v1(&mut hash, moment, meter)?;
        }
        hash_frame_v1(
            &mut hash,
            &[effective_sign_tag_v1(input.common_sign)],
            meter,
        )?;
        hash_frame_v1(
            &mut hash,
            &[sign_tag_v1(input.composition.slope_sign)
                .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?],
            meter,
        )?;
        hash_frame_v1(
            &mut hash,
            &u64::try_from(input.witnesses.len())
                .map_err(|_| ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for witness in input.witnesses {
            hash_frame_v1(&mut hash, &witness.edge.canonical_bytes(), meter)?;
            hash_frame_v1(&mut hash, &witness.left_face.canonical_bytes(), meter)?;
            hash_frame_v1(&mut hash, &witness.right_face.canonical_bytes(), meter)?;
            hash_frame_v1(&mut hash, &[u8::from(witness.is_spanning)], meter)?;
            for sign in [
                witness.fixed_side_sign,
                witness.line_generator_sign,
                witness.effective_sign,
            ] {
                hash_frame_v1(
                    &mut hash,
                    &[sign_tag_v1(sign)
                        .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?],
                    meter,
                )?;
            }
        }
        Ok(hash.finalize().into())
    })();
    meter.end_temporary(SHA256_SCRATCH_BYTES_V1);
    result
}

fn prove_with_meter_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    meter: &mut MeterV1,
) -> Result<
    ExactCommonSplitPairEffectiveGeneratorSignV1,
    ExactCommonSplitPairEffectiveGeneratorSignErrorV1,
> {
    let edge_count = common_profile.edge_ids().len();
    let face_count = geometry.face_ids().len();
    meter.charge_work(12)?;
    if !(MIN_EDGES_V1..=MAX_EDGES_V1).contains(&edge_count) {
        return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput);
    }
    if edge_count > meter.limits.max_edges || face_count > meter.limits.max_faces {
        return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit);
    }
    if face_count != REQUIRED_FACES_V1
        || audit.faces().len() != REQUIRED_FACES_V1
        || geometry.face_ids() != audit.faces()
        || audit.faces()[0].canonical_bytes() >= audit.faces()[1].canonical_bytes()
        || !audit.faces().contains(&fixed_face)
        || geometry.hinges().len() != edge_count
        || audit.spanning_hinges().len() != 1
        || audit.closure_hinges().len() != edge_count - 1
    {
        return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::UnsupportedSplitPair);
    }
    if !schedule.matches_binding(geometry, audit, fixed_face) {
        return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::GraphBindingMismatch);
    }
    let composition = common_profile
        .revalidate_composition_binding_v1(schedule, meter.limits.profile_limits)
        .map_err(map_profile_error_v1)?;
    if common_profile
        .edge_ids()
        .windows(2)
        .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
    {
        return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
    }
    if audit.spanning_hinges() != &common_profile.edge_ids()[..1]
        || audit.closure_hinges() != &common_profile.edge_ids()[1..]
    {
        return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
    }

    meter.retain(retained_bytes_v1(edge_count)?)?;
    let temporary_bytes = temporary_bytes_v1(face_count, edge_count)?;
    meter.begin_temporary(temporary_bytes)?;
    let result = (|| {
        meter.charge_work(
            edge_count
                .checked_mul(STRUCTURE_WORK_PER_EDGE_V1)
                .and_then(|edge_work| {
                    face_count
                        .checked_mul(STRUCTURE_WORK_PER_FACE_V1)
                        .and_then(|face_work| edge_work.checked_add(face_work))
                })
                .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?,
        )?;

        let mut geometry_order = Vec::new();
        geometry_order
            .try_reserve_exact(edge_count)
            .map_err(|_| ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        geometry_order.extend(0..edge_count);
        for unsorted in (1..edge_count).rev() {
            for left in 0..unsorted {
                meter.charge_work(1)?;
                if geometry.hinges()[geometry_order[left]]
                    .edge()
                    .canonical_bytes()
                    > geometry.hinges()[geometry_order[left + 1]]
                        .edge()
                        .canonical_bytes()
                {
                    geometry_order.swap(left, left + 1);
                }
            }
        }
        if geometry_order
            .iter()
            .map(|index| geometry.hinges()[*index].edge())
            .ne(common_profile.edge_ids().iter().copied())
        {
            return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
        }

        let canonical_face_pair = [audit.faces()[0], audit.faces()[1]];
        let mut reference_line = None;
        let mut common_sign = None;
        let mut witnesses = Vec::new();
        witnesses
            .try_reserve_exact(edge_count)
            .map_err(|_| ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        for geometry_index in geometry_order {
            meter.charge_work(EXACT_LINE_WORK_PER_EDGE_V1)?;
            let hinge = &geometry.hinges()[geometry_index];
            if canonical_pair_v1(hinge.left_face(), hinge.right_face()) != Some(canonical_face_pair)
            {
                return Err(
                    ExactCommonSplitPairEffectiveGeneratorSignErrorV1::UnsupportedSplitPair,
                );
            }
            let is_spanning = audit.spanning_hinges()[0] == hinge.edge();
            if !is_spanning && !audit.closure_hinges().contains(&hinge.edge()) {
                return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
            }
            let fixed_side_sign = if hinge.left_face() == fixed_face {
                1_i8
            } else if hinge.right_face() == fixed_face {
                -1_i8
            } else {
                return Err(
                    ExactCommonSplitPairEffectiveGeneratorSignErrorV1::UnsupportedSplitPair,
                );
            };
            let (line, line_generator_sign) = exact_generator_line_v1(hinge)
                .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?;
            if let Some(reference) = reference_line.as_ref() {
                if reference != &line {
                    return Err(
                        ExactCommonSplitPairEffectiveGeneratorSignErrorV1::NonCollinearCarrier,
                    );
                }
            } else {
                reference_line = Some(line);
            }
            let effective_sign = fixed_side_sign
                .checked_mul(line_generator_sign)
                .filter(|sign| matches!(*sign, -1 | 1))
                .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?;
            if common_sign.is_some_and(|expected| expected != effective_sign) {
                return Err(
                    ExactCommonSplitPairEffectiveGeneratorSignErrorV1::EffectiveSignMismatch,
                );
            }
            common_sign = Some(effective_sign);
            witnesses.push(SplitWitnessV1 {
                edge: hinge.edge(),
                left_face: hinge.left_face(),
                right_face: hinge.right_face(),
                is_spanning,
                fixed_side_sign,
                line_generator_sign,
                effective_sign,
            });
        }
        if witnesses
            .iter()
            .filter(|witness| witness.is_spanning)
            .count()
            != 1
        {
            return Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
        }

        let reference_line = reference_line
            .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?;
        let common_effective_sign = effective_sign_from_i8_v1(
            common_sign.ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?,
        )
        .ok_or(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::InvalidInput)?;
        let proof_fingerprint_v1 = proof_fingerprint_v1(
            ProofFingerprintInputV1 {
                edges: common_profile.edge_ids(),
                canonical_face_pair,
                fixed_face,
                common_sign: common_effective_sign,
                line: &reference_line,
                witnesses: &witnesses,
                composition,
            },
            meter,
        )?;

        let mut canonical_edges = Vec::new();
        canonical_edges
            .try_reserve_exact(edge_count)
            .map_err(|_| ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        canonical_edges.extend_from_slice(common_profile.edge_ids());
        Ok(ExactCommonSplitPairEffectiveGeneratorSignV1 {
            canonical_edges,
            canonical_face_pair,
            fixed_face,
            common_effective_sign,
            issuer_geometry_instance_v1: geometry.instance_anchor_v1(),
            issuer_schedule_fingerprint_v2: composition.schedule_fingerprint_v2,
            issuer_graph_binding_fingerprint_v1: composition.graph_binding_fingerprint_v1,
            issuer_common_profile_fingerprint_v1: composition.proof_fingerprint_v1,
            proof_fingerprint_v1,
        })
    })();
    meter.end_temporary(temporary_bytes);
    result
}

/// Proves a strict two- or three-hinge logical split pair without granting any
/// downstream closure, collision, persistence, apply, or mutation authority.
pub fn prove_exact_common_split_pair_effective_generator_sign_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    limits: ExactCommonSplitPairEffectiveGeneratorSignLimitsV1,
) -> Result<
    ExactCommonSplitPairEffectiveGeneratorSignV1,
    ExactCommonSplitPairEffectiveGeneratorSignErrorV1,
> {
    let mut meter = MeterV1::new(limits);
    prove_with_meter_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        common_profile,
        &mut meter,
    )
}

#[cfg(test)]
#[path = "exact_common_split_pair_effective_generator_sign_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "exact_common_split_pair_effective_generator_sign_production_tests.rs"]
mod production_tests;
