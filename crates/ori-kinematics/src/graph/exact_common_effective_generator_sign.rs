use std::collections::VecDeque;

use num_bigint::Sign;
use num_rational::BigRational;
use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    MaterialHingeGraphAudit,
    exact_generator_word::{
        AuthenticatedGraphV1, CanonicalInfiniteLineV1, authenticate_graph_structure_v1,
        exact_generator_line_v1,
    },
};
use crate::{
    CanonicalCycleScheduleV1, ExactCommonLinearCycleProfileErrorV1,
    ExactCommonLinearCycleProfileLimitsV1, ExactCommonLinearCycleProfileV1,
    MaterialHingeGraphGeometry,
    schedule::{
        EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1, ExactCommonLinearCompositionBindingV1,
    },
};

/// Frozen model identifier for the rooted effective-generator-sign proof.
pub const EXACT_COMMON_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1: &str =
    "exact_common_effective_generator_sign_v1";

const MIN_EDGES_V1: usize = 2;
const MAX_EDGES_V1: usize = 3;
const MAX_FACES_V1: usize = MAX_EDGES_V1 + 1;
const EDGE_BYTES_V1: usize = 16;
const FACE_BYTES_V1: usize = 16;
const FINGERPRINT_BYTES_V1: usize = 32;
const SIGN_BYTES_V1: usize = 1;
const SHA256_SCRATCH_BYTES_V1: usize = 104;
// Cross-runtime logical envelopes for the authenticated face registry, rooted
// traversal state, and all canonical edge/set/witness records. Vec/HashMap
// implementation metadata is deliberately not modeled.
const TEMPORARY_BYTES_PER_FACE_V1: usize = 64;
const TEMPORARY_BYTES_PER_EDGE_V1: usize = 192;
// Two exact Pluecker lines, including conservative BigRational intermediate
// storage for all finite IEEE-754 binary64 inputs.
const EXACT_LINE_SCRATCH_BYTES_V1: usize = 64 * 1024;
const EXACT_LINE_WORK_PER_EDGE_V1: usize = 8 * 1024;
const STRUCTURE_WORK_PER_EDGE_V1: usize = 32;
const STRUCTURE_WORK_PER_FACE_V1: usize = 16;

/// Sign of a root-outward local hinge generator relative to the canonical
/// direction of its exact infinite carrier.
///
/// For one bridge, `q` is positive when the endpoint on the fixed-face side is
/// the stored left face and negative otherwise. If `o` is the exact
/// left-to-right generator sign returned by the native line authenticator, this
/// value is `q * o`.
///
/// This sign is relative to the public hinge angle. The common schedule slope
/// is separately bound into the proof hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectiveGeneratorSignV1 {
    Negative,
    Positive,
}

impl EffectiveGeneratorSignV1 {
    const fn from_i8(value: i8) -> Option<Self> {
        match value {
            -1 => Some(Self::Negative),
            1 => Some(Self::Positive),
            _ => None,
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Negative => 0,
            Self::Positive => 1,
        }
    }
}

/// Explicit resource envelope for the complete rooted carrier proof.
///
/// Storage limits count canonical payload bytes rather than target-dependent
/// Rust object layout. The nested profile limits independently bound upstream
/// proof revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactCommonEffectiveGeneratorSignLimitsV1 {
    pub profile_limits: ExactCommonLinearCycleProfileLimitsV1,
    pub max_edges: usize,
    pub max_faces: usize,
    pub max_work: usize,
    pub max_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

impl Default for ExactCommonEffectiveGeneratorSignLimitsV1 {
    fn default() -> Self {
        Self {
            profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
            max_edges: MAX_EDGES_V1,
            max_faces: MAX_FACES_V1,
            max_work: 64 * 1024,
            max_retained_bytes: retained_bytes_v1(MAX_EDGES_V1).unwrap_or(usize::MAX),
            max_peak_bytes: 128 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ExactCommonEffectiveGeneratorSignErrorV1 {
    #[error("the effective-generator-sign input is malformed")]
    InvalidInput,
    #[error("the common linear profile was not issued by this schedule")]
    ProfileIssuerMismatch,
    #[error("the schedule is not bound to this graph and fixed face")]
    GraphBindingMismatch,
    #[error("the common profile does not exactly cover every graph hinge")]
    CarrierSetMismatch,
    #[error("the complete carrier is not one connected rooted tree")]
    UnsupportedRootedCarrier,
    #[error("the complete carrier does not use one exact infinite line")]
    NonCollinearCarrier,
    #[error("the rooted effective generator signs are not identical")]
    EffectiveSignMismatch,
    #[error("the effective-generator-sign proof exceeds its explicit resource limits")]
    ResourceLimit,
    #[error("the proof was not issued by these exact inputs")]
    IssuerMismatch,
}

/// Opaque evidence that every hinge in one complete two- or three-edge rooted
/// tree has one exact carrier and one root-outward effective generator sign.
///
/// This is a recognition proof only. It deliberately has no persistence
/// traits and grants no closure, collision, or mutation authority.
///
/// ```compile_fail
/// use ori_kinematics::ExactCommonEffectiveGeneratorSignV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ExactCommonEffectiveGeneratorSignV1>();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactCommonEffectiveGeneratorSignV1 {
    canonical_edges: Vec<EdgeId>,
    fixed_face: FaceId,
    common_effective_sign: EffectiveGeneratorSignV1,
    issuer_schedule_fingerprint_v2: [u8; FINGERPRINT_BYTES_V1],
    issuer_graph_binding_fingerprint_v1: [u8; FINGERPRINT_BYTES_V1],
    issuer_common_profile_fingerprint_v1: [u8; FINGERPRINT_BYTES_V1],
    proof_fingerprint_v1: [u8; FINGERPRINT_BYTES_V1],
}

impl ExactCommonEffectiveGeneratorSignV1 {
    #[must_use]
    pub fn edge_ids(&self) -> &[EdgeId] {
        &self.canonical_edges
    }

    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub const fn common_effective_sign(&self) -> EffectiveGeneratorSignV1 {
        self.common_effective_sign
    }

    pub fn revalidate_issuers_v1(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        common_profile: &ExactCommonLinearCycleProfileV1,
        limits: ExactCommonEffectiveGeneratorSignLimitsV1,
    ) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
        let mut meter = MeterV1::new(limits);
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
                .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?,
        )?;
        if &candidate == self {
            Ok(())
        } else {
            Err(ExactCommonEffectiveGeneratorSignErrorV1::IssuerMismatch)
        }
    }

    #[must_use]
    pub const fn authorizes_closure(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct MeterV1 {
    limits: ExactCommonEffectiveGeneratorSignLimitsV1,
    work: usize,
    retained_bytes: usize,
    temporary_bytes: usize,
    peak_bytes: usize,
}

impl MeterV1 {
    const fn new(limits: ExactCommonEffectiveGeneratorSignLimitsV1) -> Self {
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
    ) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        if self.work > self.limits.max_work {
            return Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit);
        }
        Ok(())
    }

    fn retain(&mut self, amount: usize) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(amount)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        if self.retained_bytes > self.limits.max_retained_bytes {
            return Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit);
        }
        self.update_peak()
    }

    fn begin_temporary(
        &mut self,
        amount: usize,
    ) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_add(amount)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        self.update_peak()
    }

    fn end_temporary(&mut self, amount: usize) {
        self.temporary_bytes = self
            .temporary_bytes
            .checked_sub(amount)
            .expect("internal effective-generator temporary storage must balance");
    }

    fn update_peak(&mut self) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
        let current = self
            .retained_bytes
            .checked_add(self.temporary_bytes)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        self.peak_bytes = self.peak_bytes.max(current);
        if self.peak_bytes > self.limits.max_peak_bytes {
            return Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RootedWitnessV1 {
    edge: EdgeId,
    fixed_side: FaceId,
    moving_side: FaceId,
    traversal_sign: i8,
    line_generator_sign: i8,
    effective_sign: i8,
}

fn retained_bytes_v1(edge_count: usize) -> Result<usize, ExactCommonEffectiveGeneratorSignErrorV1> {
    edge_count
        .checked_mul(EDGE_BYTES_V1)
        .and_then(|edges| edges.checked_add(FACE_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(SIGN_BYTES_V1))
        .and_then(|bytes| {
            FINGERPRINT_BYTES_V1
                .checked_mul(4)
                .and_then(|fingerprints| bytes.checked_add(fingerprints))
        })
        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
}

fn temporary_bytes_v1(
    face_count: usize,
    edge_count: usize,
) -> Result<usize, ExactCommonEffectiveGeneratorSignErrorV1> {
    let faces = face_count
        .checked_mul(TEMPORARY_BYTES_PER_FACE_V1)
        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    let edges = edge_count
        .checked_mul(TEMPORARY_BYTES_PER_EDGE_V1)
        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    faces
        .checked_add(edges)
        .and_then(|bytes| bytes.checked_add(EXACT_LINE_SCRATCH_BYTES_V1))
        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
}

fn map_profile_error_v1(
    error: ExactCommonLinearCycleProfileErrorV1,
) -> ExactCommonEffectiveGeneratorSignErrorV1 {
    if error == ExactCommonLinearCycleProfileErrorV1::ResourceLimit {
        ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit
    } else {
        ExactCommonEffectiveGeneratorSignErrorV1::ProfileIssuerMismatch
    }
}

fn sign_tag_v1(value: i8) -> Option<u8> {
    match value {
        -1 => Some(0),
        1 => Some(1),
        _ => None,
    }
}

fn hash_frame_v1(
    hash: &mut Sha256,
    value: &[u8],
    meter: &mut MeterV1,
) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
    let length = u64::try_from(value.len())
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
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
) -> Result<(), ExactCommonEffectiveGeneratorSignErrorV1> {
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

fn proof_fingerprint_v1(
    edges: &[EdgeId],
    fixed_face: FaceId,
    common_sign: EffectiveGeneratorSignV1,
    line: &CanonicalInfiniteLineV1,
    witnesses: &[RootedWitnessV1],
    composition: ExactCommonLinearCompositionBindingV1,
    meter: &mut MeterV1,
) -> Result<[u8; FINGERPRINT_BYTES_V1], ExactCommonEffectiveGeneratorSignErrorV1> {
    const DOMAIN_SEPARATOR: &[u8] = b"ORIGAMI2_EXACT_COMMON_EFFECTIVE_GENERATOR_SIGN_PROOF_V1";
    meter.begin_temporary(SHA256_SCRATCH_BYTES_V1)?;
    let result = (|| {
        let mut hash = Sha256::new();
        hash_frame_v1(&mut hash, DOMAIN_SEPARATOR, meter)?;
        hash_frame_v1(
            &mut hash,
            EXACT_COMMON_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        hash_frame_v1(
            &mut hash,
            EXACT_COMMON_LINEAR_CYCLE_PROFILE_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        hash_frame_v1(&mut hash, &composition.schedule_fingerprint_v2, meter)?;
        hash_frame_v1(&mut hash, &composition.graph_binding_fingerprint_v1, meter)?;
        hash_frame_v1(&mut hash, &composition.proof_fingerprint_v1, meter)?;
        hash_frame_v1(&mut hash, &fixed_face.canonical_bytes(), meter)?;
        hash_frame_v1(
            &mut hash,
            &u64::try_from(edges.len())
                .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for edge in edges {
            hash_frame_v1(&mut hash, &edge.canonical_bytes(), meter)?;
        }
        for bits in line.direction_bits() {
            hash_frame_v1(&mut hash, &bits.to_be_bytes(), meter)?;
        }
        for moment in line.exact_moment() {
            hash_rational_v1(&mut hash, moment, meter)?;
        }
        hash_frame_v1(&mut hash, &[common_sign.tag()], meter)?;
        hash_frame_v1(
            &mut hash,
            &[sign_tag_v1(composition.slope_sign)
                .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?],
            meter,
        )?;
        hash_frame_v1(
            &mut hash,
            &u64::try_from(witnesses.len())
                .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?
                .to_be_bytes(),
            meter,
        )?;
        for witness in witnesses {
            hash_frame_v1(&mut hash, &witness.edge.canonical_bytes(), meter)?;
            hash_frame_v1(&mut hash, &witness.fixed_side.canonical_bytes(), meter)?;
            hash_frame_v1(&mut hash, &witness.moving_side.canonical_bytes(), meter)?;
            for sign in [
                witness.traversal_sign,
                witness.line_generator_sign,
                witness.effective_sign,
            ] {
                hash_frame_v1(
                    &mut hash,
                    &[sign_tag_v1(sign)
                        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?],
                    meter,
                )?;
            }
        }
        Ok(hash.finalize().into())
    })();
    meter.end_temporary(SHA256_SCRATCH_BYTES_V1);
    result
}

fn rooted_traversal_signs_v1(
    graph: &AuthenticatedGraphV1,
    fixed_face: FaceId,
    meter: &mut MeterV1,
) -> Result<Vec<i8>, ExactCommonEffectiveGeneratorSignErrorV1> {
    let root = graph
        .faces()
        .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier)?;
    let mut degrees = Vec::new();
    degrees
        .try_reserve_exact(graph.faces().len())
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    degrees.resize(graph.faces().len(), 0usize);
    for record in graph.hinges() {
        meter.charge_work(2)?;
        degrees[record.left()] = degrees[record.left()]
            .checked_add(1)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        degrees[record.right()] = degrees[record.right()]
            .checked_add(1)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    }
    let mut adjacency = Vec::new();
    adjacency
        .try_reserve_exact(graph.faces().len())
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    for degree in degrees {
        let mut neighbors = Vec::<(usize, usize, i8)>::new();
        neighbors
            .try_reserve_exact(degree)
            .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
        adjacency.push(neighbors);
    }
    for (record_index, record) in graph.hinges().iter().enumerate() {
        meter.charge_work(2)?;
        adjacency[record.left()].push((record.right(), record_index, 1));
        adjacency[record.right()].push((record.left(), record_index, -1));
    }

    let mut visited = Vec::new();
    visited
        .try_reserve_exact(graph.faces().len())
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    visited.resize(graph.faces().len(), false);
    let mut traversal_signs = Vec::new();
    traversal_signs
        .try_reserve_exact(graph.hinges().len())
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    traversal_signs.resize(graph.hinges().len(), 0_i8);
    let mut queue = VecDeque::new();
    queue
        .try_reserve(graph.faces().len())
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    visited[root] = true;
    queue.push_back(root);
    let mut discovered_edges = 0usize;
    while let Some(parent) = queue.pop_front() {
        for &(child, record_index, sign) in &adjacency[parent] {
            meter.charge_work(1)?;
            if visited[child] {
                continue;
            }
            if traversal_signs[record_index] != 0 {
                return Err(ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier);
            }
            traversal_signs[record_index] = sign;
            discovered_edges = discovered_edges
                .checked_add(1)
                .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
            visited[child] = true;
            queue.push_back(child);
        }
    }
    if visited.iter().any(|value| !value)
        || discovered_edges != graph.hinges().len()
        || traversal_signs.iter().any(|sign| !matches!(*sign, -1 | 1))
    {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier);
    }
    Ok(traversal_signs)
}

fn prove_with_meter_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    meter: &mut MeterV1,
) -> Result<ExactCommonEffectiveGeneratorSignV1, ExactCommonEffectiveGeneratorSignErrorV1> {
    let edge_count = common_profile.edge_ids().len();
    let face_count = geometry.face_ids().len();
    meter.charge_work(8)?;
    if !(MIN_EDGES_V1..=MAX_EDGES_V1).contains(&edge_count) {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput);
    }
    if edge_count > meter.limits.max_edges || face_count > meter.limits.max_faces {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit);
    }
    let expected_face_count = edge_count
        .checked_add(1)
        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    if face_count != expected_face_count
        || face_count > MAX_FACES_V1
        || geometry.hinges().len() != edge_count
        || audit.faces().len() != face_count
        || audit.spanning_hinges().len() != edge_count
        || !audit.closure_hinges().is_empty()
        || !audit.faces().contains(&fixed_face)
    {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier);
    }
    if !schedule.matches_binding(geometry, audit, fixed_face) {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::GraphBindingMismatch);
    }
    let composition = common_profile
        .revalidate_composition_binding_v1(schedule, meter.limits.profile_limits)
        .map_err(map_profile_error_v1)?;

    meter.retain(retained_bytes_v1(edge_count)?)?;
    let temporary_bytes = temporary_bytes_v1(face_count, edge_count)?;
    meter.begin_temporary(temporary_bytes)?;

    meter.charge_work(
        edge_count
            .checked_mul(STRUCTURE_WORK_PER_EDGE_V1)
            .and_then(|edge_work| {
                face_count
                    .checked_mul(STRUCTURE_WORK_PER_FACE_V1)
                    .and_then(|face_work| edge_work.checked_add(face_work))
            })
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?,
    )?;
    if common_profile
        .edge_ids()
        .windows(2)
        .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
    {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
    }
    let mut spanning = Vec::new();
    spanning
        .try_reserve_exact(edge_count)
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    spanning.extend_from_slice(audit.spanning_hinges());
    for unsorted in (1..edge_count).rev() {
        for left in 0..unsorted {
            meter.charge_work(1)?;
            if spanning[left].canonical_bytes() > spanning[left + 1].canonical_bytes() {
                spanning.swap(left, left + 1);
            }
        }
    }
    if spanning.as_slice() != common_profile.edge_ids() {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
    }

    let graph = authenticate_graph_structure_v1(geometry, audit)
        .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier)?;
    if graph.faces().len() != face_count
        || graph.hinges().len() != edge_count
        || graph
            .hinges()
            .iter()
            .map(|record| geometry.hinges()[record.geometry_index()].edge())
            .ne(common_profile.edge_ids().iter().copied())
    {
        return Err(ExactCommonEffectiveGeneratorSignErrorV1::CarrierSetMismatch);
    }

    let traversal_signs = rooted_traversal_signs_v1(&graph, fixed_face, meter)?;
    let mut reference_line = None;
    let mut common_sign = None;
    let mut witnesses = Vec::new();
    witnesses
        .try_reserve_exact(edge_count)
        .map_err(|_| ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)?;
    for (record_index, record) in graph.hinges().iter().enumerate() {
        meter.charge_work(EXACT_LINE_WORK_PER_EDGE_V1)?;
        let hinge = &geometry.hinges()[record.geometry_index()];
        let (line, line_generator_sign) = exact_generator_line_v1(hinge)
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?;
        if reference_line
            .as_ref()
            .is_some_and(|reference| reference != &line)
        {
            return Err(ExactCommonEffectiveGeneratorSignErrorV1::NonCollinearCarrier);
        }
        if reference_line.is_none() {
            reference_line = Some(line.clone());
        }
        let traversal_sign = traversal_signs[record_index];
        let effective_sign = traversal_sign
            .checked_mul(line_generator_sign)
            .filter(|sign| matches!(*sign, -1 | 1))
            .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?;
        if common_sign.is_some_and(|expected| expected != effective_sign) {
            return Err(ExactCommonEffectiveGeneratorSignErrorV1::EffectiveSignMismatch);
        }
        common_sign = Some(effective_sign);
        let (fixed_side, moving_side) = if traversal_sign == 1 {
            (hinge.left_face(), hinge.right_face())
        } else {
            (hinge.right_face(), hinge.left_face())
        };
        witnesses.push(RootedWitnessV1 {
            edge: hinge.edge(),
            fixed_side,
            moving_side,
            traversal_sign,
            line_generator_sign,
            effective_sign,
        });
    }
    let reference_line =
        reference_line.ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?;
    let common_effective_sign = EffectiveGeneratorSignV1::from_i8(
        common_sign.ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?,
    )
    .ok_or(ExactCommonEffectiveGeneratorSignErrorV1::InvalidInput)?;
    let proof_fingerprint_v1 = proof_fingerprint_v1(
        common_profile.edge_ids(),
        fixed_face,
        common_effective_sign,
        &reference_line,
        &witnesses,
        composition,
        meter,
    )?;
    meter.end_temporary(temporary_bytes);

    Ok(ExactCommonEffectiveGeneratorSignV1 {
        canonical_edges: common_profile.edge_ids().to_vec(),
        fixed_face,
        common_effective_sign,
        issuer_schedule_fingerprint_v2: composition.schedule_fingerprint_v2,
        issuer_graph_binding_fingerprint_v1: composition.graph_binding_fingerprint_v1,
        issuer_common_profile_fingerprint_v1: composition.proof_fingerprint_v1,
        proof_fingerprint_v1,
    })
}

/// Proves exact common rooted generator orientation for one complete two- or
/// three-hinge tree carrier.
///
/// Closure edges and cyclic carriers are rejected. Every edge must belong to
/// the upstream exact common linear profile, use one exact infinite line, and
/// have the same effective sign when traversed away from `fixed_face`.
pub fn prove_exact_common_effective_generator_sign_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    common_profile: &ExactCommonLinearCycleProfileV1,
    limits: ExactCommonEffectiveGeneratorSignLimitsV1,
) -> Result<ExactCommonEffectiveGeneratorSignV1, ExactCommonEffectiveGeneratorSignErrorV1> {
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
#[path = "exact_common_effective_generator_sign_tests.rs"]
mod tests;
