//! Exact, source-bound authority for one flat initial path sample and its
//! unchanged, directly hinged flat-stack pairs at later sampled poses.
//!
//! This module deliberately keeps the retained proof private. The parent path
//! diagnostic can only ask whether an exact static snapshot has the same
//! issuer, pose, thickness, and pair-scoped evidence as the prepared
//! admission.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    marker::PhantomData,
    sync::Arc,
};

use ori_domain::{EdgeId, FaceId};
use ori_foldability::{ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign};
use ori_kinematics::{MaterialTreeKinematicsModel, MaterialTreePose};

use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1, IntersectionEvidenceV2,
    NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1, NonFlatCellTransportErrorV1,
    NonFlatFacePairOrderStructuralV1, NonFlatFoldedFaceStructuralRefV1,
    NonFlatLayerOrderStructuralSourceV1, NonFlatOverlapCellStructuralRefV1,
    StaticCollisionDiagnosticSnapshot, StaticCollisionError, StaticCollisionLimits,
    StaticCollisionPairDisposition, TopologyRelation,
    diagnose_static_collision_geometry_with_control_v1,
    validate_non_flat_layer_order_structure_with_control_v1,
};

use super::StackedFoldPathDiagnosticErrorV1;

const MAX_STACKED_FOLD_INITIAL_LAYER_BOUNDARY_POINTS_PER_CELL_V1: usize = 4_096;
const MAX_STACKED_FOLD_INITIAL_LAYER_TOTAL_BOUNDARY_POINTS_V1: usize = 50_000;
const BITS_PER_BYTE_V1: usize = u8::BITS as usize;

/// Cheap encoded-payload preflight performed before any exact value is
/// converted to `BigInt`, `BigRational`, or binary64.
///
/// The hard caps reuse the foldability certificate policy because this source
/// is retained proof input from that exact pipeline. The caller-selected
/// static limits may only narrow those caps. Total payload charges count the
/// canonical numerator-magnitude and denominator byte slices themselves; the
/// already bounded face/point counts independently bound per-value structure.
struct InitialLayerExactPayloadPreflightV1 {
    total_bytes: usize,
    total_byte_limit: usize,
    max_integer_bits: usize,
    max_integer_bytes: usize,
}

impl InitialLayerExactPayloadPreflightV1 {
    fn new(limits: StaticCollisionLimits) -> Self {
        let max_integer_bits = limits
            .max_rational_input_bits
            .min(ori_foldability::DEFAULT_MAX_EXACT_INTEGER_BITS);
        let max_integer_bytes = max_integer_bits.div_ceil(BITS_PER_BYTE_V1);
        Self {
            total_bytes: 0,
            total_byte_limit: (limits.max_total_rational_input_storage_bits / BITS_PER_BYTE_V1)
                .min(ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES),
            max_integer_bits,
            max_integer_bytes,
        }
    }

    fn charge_rational(
        &mut self,
        value: &ExactRationalValue,
    ) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
        if !initial_layer_exact_rational_has_canonical_slices_v1(value) {
            return initial_layer_unavailable_v1();
        }
        if value.numerator_magnitude_be.len() > self.max_integer_bytes
            || value.denominator_be.len() > self.max_integer_bytes
            || initial_layer_exact_integer_bits_v1(&value.numerator_magnitude_be)?
                > self.max_integer_bits
            || initial_layer_exact_integer_bits_v1(&value.denominator_be)? > self.max_integer_bits
        {
            return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
        }
        let bytes = value
            .numerator_magnitude_be
            .len()
            .checked_add(value.denominator_be.len())
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
        let total_bytes = self
            .total_bytes
            .checked_add(bytes)
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
        if total_bytes > self.total_byte_limit {
            return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
        }
        self.total_bytes = total_bytes;
        Ok(())
    }

    fn charge_transform_with_control(
        &mut self,
        transform: &ExactAffineTransform,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
        for value in [
            &transform.m00,
            &transform.m01,
            &transform.m10,
            &transform.m11,
            &transform.tx,
            &transform.ty,
        ] {
            initial_layer_checkpoint_v1(control)?;
            self.charge_rational(value)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn charge_transform(
        &mut self,
        transform: &ExactAffineTransform,
    ) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
        self.charge_transform_with_control(transform, &CooperativeOperationControlV1::unbounded())
    }

    fn charge_boundary_with_control(
        &mut self,
        boundary: &[ExactPointValue],
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
        for point in boundary {
            initial_layer_checkpoint_v1(control)?;
            self.charge_rational(&point.x)?;
            initial_layer_checkpoint_v1(control)?;
            self.charge_rational(&point.y)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn charge_boundary(
        &mut self,
        boundary: &[ExactPointValue],
    ) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
        self.charge_boundary_with_control(boundary, &CooperativeOperationControlV1::unbounded())
    }
}

fn initial_layer_exact_rational_has_canonical_slices_v1(value: &ExactRationalValue) -> bool {
    let numerator = value.numerator_magnitude_be.as_slice();
    let denominator = value.denominator_be.as_slice();
    if denominator.is_empty() || denominator.first() == Some(&0) {
        return false;
    }
    match value.sign {
        ExactSign::Zero => matches!(numerator, [] | [0]) && denominator == [1_u8],
        ExactSign::Negative | ExactSign::Positive => {
            !numerator.is_empty() && numerator.first() != Some(&0)
        }
    }
}

fn initial_layer_exact_integer_bits_v1(
    magnitude_be: &[u8],
) -> Result<usize, StackedFoldPathDiagnosticErrorV1> {
    let Some(first) = magnitude_be.first().copied() else {
        return Ok(0);
    };
    let first_bits = (u8::BITS - first.leading_zeros()) as usize;
    magnitude_be
        .len()
        .saturating_sub(1)
        .checked_mul(BITS_PER_BYTE_V1)
        .and_then(|bits| bits.checked_add(first_bits))
        .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
}

/// Read-only source needed to authenticate a flat initial layer order.
///
/// Structural overlap geometry is inherited from
/// [`NonFlatLayerOrderStructuralSourceV1`]. The additional observations bind
/// that geometry to one complete target-pair scan and to the exact flat pose
/// for which the order was prepared. This trait grants no collision or
/// mutation authority by itself.
pub trait StackedFoldInitialLayerOrderSourceV1: NonFlatLayerOrderStructuralSourceV1 {
    fn tested_face_pairs_v1(&self) -> usize;
    fn fixed_face_v1(&self) -> Option<FaceId>;
    fn hinge_angle_count_v1(&self) -> usize;
    fn hinge_angle_v1(&self, index: usize) -> Option<(EdgeId, u64)>;
    fn paper_thickness_bits_v1(&self) -> u64;
}

#[derive(Debug)]
struct InitialSampleLayerAdmissionProofV1 {
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
    paper_thickness_bits: u64,
    initial_static_snapshot: StaticCollisionDiagnosticSnapshot,
    initial_flat_pairs: Vec<InitialFlatPairAdmissionV1>,
    persistent_flat_hinges: Vec<PersistentFlatHingeAdmissionV1>,
    directed_orders: Vec<NonFlatFacePairOrderStructuralV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InitialFlatPairAdmissionV1 {
    first_face: FaceId,
    second_face: FaceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PersistentFlatHingeAdmissionV1 {
    first_face: FaceId,
    second_face: FaceId,
    hinge: EdgeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StationaryFlatStackTransportBindingV1 {
    pub(super) hinge: EdgeId,
    pub(super) pair: [FaceId; 2],
    pub(super) lower_face: FaceId,
    pub(super) upper_face: FaceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InitialLayerPairAdmissionKindV1 {
    UnorderedNonblocking,
    InitialOnlyFlatStack,
    PersistentFlatStack { hinge: EdgeId },
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InitialLayerPairAdmissionDecisionV1 {
    pub(super) pair: (FaceId, FaceId),
    pub(super) kind: InitialLayerPairAdmissionKindV1,
}

pub(super) fn classify_initial_layer_pair_admission_v1(
    pair: (FaceId, FaceId),
    layer_ordered: bool,
    disposition: StaticCollisionPairDisposition,
    evidence: IntersectionEvidenceV2,
    direct_hinge: Option<EdgeId>,
    initial_hinge_angle_bits: Option<u64>,
) -> InitialLayerPairAdmissionDecisionV1 {
    let pair = initial_layer_canonical_pair_v1(pair.0, pair.1);
    let kind = if disposition == StaticCollisionPairDisposition::Indeterminate {
        if evidence != IntersectionEvidenceV2::SharedFeatureFlatStack || !layer_ordered {
            InitialLayerPairAdmissionKindV1::Rejected
        } else if let Some(hinge) = direct_hinge
            && initial_hinge_angle_bits == Some(180.0_f64.to_bits())
        {
            InitialLayerPairAdmissionKindV1::PersistentFlatStack { hinge }
        } else {
            InitialLayerPairAdmissionKindV1::InitialOnlyFlatStack
        }
    } else if layer_ordered {
        InitialLayerPairAdmissionKindV1::Rejected
    } else {
        InitialLayerPairAdmissionKindV1::UnorderedNonblocking
    };
    InitialLayerPairAdmissionDecisionV1 { pair, kind }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PersistentFlatStackSampleRejectionReasonV1 {
    IncompletePairScan,
    PenetrationPresent,
    AuthorityPairMissing,
    MissingDirectSharedHinge,
    HingeMoves,
    InitialHingeNotFlat,
    CurrentHingeNotFlat,
    TopologyMismatch,
    EvidenceMismatch,
    DispositionMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PersistentFlatStackSampleRejectionV1 {
    pub(super) pair: (FaceId, FaceId),
    pub(super) reason: PersistentFlatStackSampleRejectionReasonV1,
}

#[derive(Debug, Clone, Copy)]
struct PersistentFlatStackSampleObservationV1 {
    pair: (FaceId, FaceId),
    complete_pair_scan: bool,
    penetration_free: bool,
    authority_pair_authenticated: bool,
    direct_shared_hinge_authenticated: bool,
    hinge_is_stationary: bool,
    initial_hinge_angle_bits: Option<u64>,
    current_hinge_angle_bits: Option<u64>,
    topology: TopologyRelation,
    evidence: IntersectionEvidenceV2,
    disposition: StaticCollisionPairDisposition,
}

#[cfg(test)]
fn persistent_flat_stack_sample_observation_is_admissible_v1(
    observation: PersistentFlatStackSampleObservationV1,
) -> bool {
    diagnose_persistent_flat_stack_sample_observation_v1(observation).is_ok()
}

fn diagnose_persistent_flat_stack_sample_observation_v1(
    observation: PersistentFlatStackSampleObservationV1,
) -> Result<(), PersistentFlatStackSampleRejectionV1> {
    let reason = if !observation.complete_pair_scan {
        Some(PersistentFlatStackSampleRejectionReasonV1::IncompletePairScan)
    } else if !observation.penetration_free {
        Some(PersistentFlatStackSampleRejectionReasonV1::PenetrationPresent)
    } else if !observation.authority_pair_authenticated {
        Some(PersistentFlatStackSampleRejectionReasonV1::AuthorityPairMissing)
    } else if !observation.direct_shared_hinge_authenticated {
        Some(PersistentFlatStackSampleRejectionReasonV1::MissingDirectSharedHinge)
    } else if !observation.hinge_is_stationary {
        Some(PersistentFlatStackSampleRejectionReasonV1::HingeMoves)
    } else if observation.initial_hinge_angle_bits != Some(180.0_f64.to_bits()) {
        Some(PersistentFlatStackSampleRejectionReasonV1::InitialHingeNotFlat)
    } else if observation.current_hinge_angle_bits != Some(180.0_f64.to_bits()) {
        Some(PersistentFlatStackSampleRejectionReasonV1::CurrentHingeNotFlat)
    } else if observation.topology != TopologyRelation::SharedHingeEdge {
        Some(PersistentFlatStackSampleRejectionReasonV1::TopologyMismatch)
    } else if observation.evidence != IntersectionEvidenceV2::SharedFeatureFlatStack {
        Some(PersistentFlatStackSampleRejectionReasonV1::EvidenceMismatch)
    } else if observation.disposition != StaticCollisionPairDisposition::Indeterminate {
        Some(PersistentFlatStackSampleRejectionReasonV1::DispositionMismatch)
    } else {
        None
    };
    match reason {
        Some(reason) => Err(PersistentFlatStackSampleRejectionV1 {
            pair: initial_layer_canonical_pair_v1(observation.pair.0, observation.pair.1),
            reason,
        }),
        None => Ok(()),
    }
}

/// Defensive test seam for an initial-only pair class that current valid Tree
/// geometry cannot emit: `SharedFeatureFlatStack` requires shared-hinge
/// topology, and a valid Tree gives that pair one direct hinge. Keeping this
/// decision explicit prevents a future broader static classifier from
/// accidentally turning source order into positive-sample authority.
#[cfg(test)]
pub(super) fn diagnose_nondirect_positive_flat_stack_for_test_v1(
    pair: (FaceId, FaceId),
) -> Result<(), PersistentFlatStackSampleRejectionV1> {
    diagnose_persistent_flat_stack_sample_observation_v1(PersistentFlatStackSampleObservationV1 {
        pair,
        complete_pair_scan: true,
        penetration_free: true,
        authority_pair_authenticated: true,
        direct_shared_hinge_authenticated: false,
        hinge_is_stationary: true,
        initial_hinge_angle_bits: Some(180.0_f64.to_bits()),
        current_hinge_angle_bits: Some(180.0_f64.to_bits()),
        topology: TopologyRelation::SharedHingeEdge,
        evidence: IntersectionEvidenceV2::SharedFeatureFlatStack,
        disposition: StaticCollisionPairDisposition::Indeterminate,
    })
}

/// Opaque, source-type-bound admission for the authenticated initial sample
/// and stationary, bit-exact-flat direct hinges of one bounded diagnosis.
///
/// The admission never certifies an open path interval and cannot authorize a
/// project mutation. At positive samples it permits only canonical pairs from
/// the retained initial authority whose direct hinge is outside the moving set
/// and remains bit-exact 180 degrees.
pub struct NativeStackedFoldInitialSampleLayerAdmissionV1<T> {
    proof: Arc<InitialSampleLayerAdmissionProofV1>,
    issuer: Arc<()>,
    source_type: PhantomData<fn() -> T>,
}

impl<T> Clone for NativeStackedFoldInitialSampleLayerAdmissionV1<T> {
    fn clone(&self) -> Self {
        Self {
            proof: Arc::clone(&self.proof),
            issuer: Arc::clone(&self.issuer),
            source_type: PhantomData,
        }
    }
}

pub(super) fn retain_initial_sample_layer_admission_issuer_v1<T>(
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
) -> Arc<()> {
    Arc::clone(&admission.issuer)
}

pub(super) fn initial_sample_layer_admission_has_issuer_v1<T>(
    issuer: &Arc<()>,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
) -> bool {
    Arc::ptr_eq(issuer, &admission.issuer)
}

/// Binds the directed source order of exactly two authenticated stationary
/// flat-stack hinges without exposing the admission's retained payload.
///
/// The returned order is source-authenticated data, not a continuous proof.
/// A caller must independently prove that each direct pair has one constant
/// relative material transform over its complete path.
pub(super) fn stationary_flat_stack_transport_bindings_v1<T>(
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    expected: &[(EdgeId, [FaceId; 2]); 2],
) -> Option<[StationaryFlatStackTransportBindingV1; 2]> {
    if admission.proof.model != *model
        || !model.owns_pose(source_pose)
        || !admission.proof.pose.same_instance(source_pose)
        || admission.proof.persistent_flat_hinges.len() != 2
        || admission.proof.directed_orders.len() != 2
    {
        return None;
    }
    let bind = |(hinge, pair): (EdgeId, [FaceId; 2])| {
        let (first_face, second_face) = initial_layer_canonical_pair_v1(pair[0], pair[1]);
        let authenticated_hinge = admission
            .proof
            .persistent_flat_hinges
            .iter()
            .find(|entry| {
                entry.first_face == first_face
                    && entry.second_face == second_face
                    && entry.hinge == hinge
            })?;
        let directed = admission.proof.directed_orders.iter().find(|order| {
            initial_layer_canonical_pair_v1(order.lower_face, order.upper_face)
                == (
                    authenticated_hinge.first_face,
                    authenticated_hinge.second_face,
                )
        })?;
        (directed.lower_face != directed.upper_face).then_some(
            StationaryFlatStackTransportBindingV1 {
                hinge,
                pair: [first_face, second_face],
                lower_face: directed.lower_face,
                upper_face: directed.upper_face,
            },
        )
    };
    let bindings = [bind(expected[0])?, bind(expected[1])?];
    (bindings[0].pair != bindings[1].pair
        && bindings[0].hinge != bindings[1].hinge
        && admission.proof.persistent_flat_hinges.iter().all(|entry| {
            bindings.iter().any(|binding| {
                binding.hinge == entry.hinge
                    && binding.pair == [entry.first_face, entry.second_face]
            })
        }))
    .then_some(bindings)
}

impl<T> NativeStackedFoldInitialSampleLayerAdmissionV1<T> {
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

/// This is the only proof observation exposed to the parent path diagnostic.
/// No retained field or snapshot can be extracted from the opaque admission.
pub(super) fn initial_sample_layer_admission_matches_snapshot_v1<T>(
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    paper_thickness_mm: f64,
    snapshot: &StaticCollisionDiagnosticSnapshot,
) -> bool {
    admission.proof.model == *model
        && admission.proof.pose.same_instance(initial_pose)
        && admission.proof.paper_thickness_bits == paper_thickness_mm.to_bits()
        && admission.proof.initial_static_snapshot == *snapshot
}

/// Revalidates the exact initial snapshot, then admits at later sampled poses
/// only the same source-authenticated flat-stack pairs whose direct shared
/// hinge remains bit-exact flat and is not one of the moving hinges.
///
/// This is sampled diagnostic admission only. It does not prove any open path
/// interval and cannot issue a continuous-motion or mutation certificate.
pub(super) struct SampledLayerAdmissionSnapshotV1<'a> {
    pub(super) model: &'a MaterialTreeKinematicsModel,
    pub(super) initial_pose: &'a MaterialTreePose,
    pub(super) moving_hinges: &'a [EdgeId],
    pub(super) sample_index: usize,
    pub(super) sample_pose: &'a MaterialTreePose,
    pub(super) paper_thickness_mm: f64,
    pub(super) snapshot: &'a StaticCollisionDiagnosticSnapshot,
}

pub(super) fn sampled_layer_admission_matches_snapshot_v1<T>(
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    input: SampledLayerAdmissionSnapshotV1<'_>,
) -> bool {
    let SampledLayerAdmissionSnapshotV1 {
        model,
        initial_pose,
        moving_hinges,
        sample_index,
        sample_pose,
        paper_thickness_mm,
        snapshot,
    } = input;
    if admission.proof.model != *model
        || !admission.proof.pose.same_instance(initial_pose)
        || admission.proof.paper_thickness_bits != paper_thickness_mm.to_bits()
        || paper_thickness_mm.to_bits() != 0.0_f64.to_bits()
        || !model.owns_pose(initial_pose)
        || !model.owns_pose(sample_pose)
        || sample_pose.fixed_face() != initial_pose.fixed_face()
    {
        return false;
    }
    if sample_index == 0 {
        return initial_layer_zero_sample_pose_matches_v1(sample_pose, initial_pose)
            && initial_sample_layer_admission_matches_snapshot_v1(
                admission,
                model,
                initial_pose,
                paper_thickness_mm,
                snapshot,
            );
    }

    let Some(expected_pairs) = model
        .face_ids()
        .len()
        .checked_sub(1)
        .and_then(|prior| model.face_ids().len().checked_mul(prior))
        .and_then(|ordered| ordered.checked_div(2))
    else {
        return false;
    };
    let complete_pair_scan = snapshot.face_count() == model.face_ids().len()
        && snapshot.expected_unordered_face_pairs() == expected_pairs
        && snapshot.pairs().len() == expected_pairs
        && snapshot.candidate_excluded_pairs() == 0;
    let penetration_free = snapshot.penetrating_pairs() == 0;
    if !complete_pair_scan || !penetration_free {
        return false;
    }

    snapshot.pairs().iter().all(|pair| {
        if pair.disposition() != StaticCollisionPairDisposition::Indeterminate {
            return pair.disposition() != StaticCollisionPairDisposition::Penetrating;
        }
        let (first_face, second_face) =
            initial_layer_canonical_pair_v1(pair.first_face(), pair.second_face());
        let key = (first_face.canonical_bytes(), second_face.canonical_bytes());
        let authenticated = admission
            .proof
            .initial_flat_pairs
            .binary_search_by_key(&key, |entry| {
                (
                    entry.first_face.canonical_bytes(),
                    entry.second_face.canonical_bytes(),
                )
            })
            .ok()
            .and_then(|index| admission.proof.initial_flat_pairs.get(index));
        let authenticated_hinge = admission
            .proof
            .persistent_flat_hinges
            .binary_search_by_key(&key, |entry| {
                (
                    entry.first_face.canonical_bytes(),
                    entry.second_face.canonical_bytes(),
                )
            })
            .ok()
            .and_then(|index| admission.proof.persistent_flat_hinges.get(index))
            .map(|entry| entry.hinge);
        let direct_hinge = direct_shared_hinge_v1(model, first_face, second_face);
        let observation = PersistentFlatStackSampleObservationV1 {
            pair: (first_face, second_face),
            complete_pair_scan,
            penetration_free,
            authority_pair_authenticated: authenticated.is_some(),
            direct_shared_hinge_authenticated: authenticated_hinge
                .zip(direct_hinge)
                .is_some_and(|(authenticated, direct)| authenticated == direct),
            hinge_is_stationary: authenticated_hinge
                .is_some_and(|hinge| !moving_hinges.contains(&hinge)),
            initial_hinge_angle_bits: authenticated_hinge
                .and_then(|hinge| hinge_angle_bits_v1(initial_pose, hinge)),
            current_hinge_angle_bits: authenticated_hinge
                .and_then(|hinge| hinge_angle_bits_v1(sample_pose, hinge)),
            topology: pair.topology(),
            evidence: pair.evidence(),
            disposition: pair.disposition(),
        };
        let decision = diagnose_persistent_flat_stack_sample_observation_v1(observation);
        decision.is_ok()
    })
}

fn initial_layer_zero_sample_pose_matches_v1(
    sample_pose: &MaterialTreePose,
    initial_pose: &MaterialTreePose,
) -> bool {
    sample_pose.same_instance(initial_pose)
        && sample_pose.hinge_angles() == initial_pose.hinge_angles()
}

fn direct_shared_hinge_v1(
    model: &MaterialTreeKinematicsModel,
    first_face: FaceId,
    second_face: FaceId,
) -> Option<EdgeId> {
    let mut matches = model.hinges().iter().filter(|hinge| {
        (hinge.left_face() == first_face && hinge.right_face() == second_face)
            || (hinge.left_face() == second_face && hinge.right_face() == first_face)
    });
    let hinge = matches.next()?.edge();
    matches.next().is_none().then_some(hinge)
}

fn hinge_angle_bits_v1(pose: &MaterialTreePose, hinge: EdgeId) -> Option<u64> {
    pose.hinge_angles()
        .iter()
        .find(|angle| angle.edge() == hinge)
        .map(|angle| angle.angle_degrees().to_bits())
}

#[derive(Debug, Clone, Copy)]
struct InitialLayerAdmissionCountsV1 {
    model_faces: usize,
    material_faces: usize,
    folded_faces: usize,
    overlap_cells: usize,
    directed_orders: usize,
    tested_pairs: usize,
    pose_hinges: usize,
    source_hinges: usize,
}

/// One read-once borrowed snapshot of the public source trait.
///
/// A trait implementation may use interior mutability, so validation must not
/// assume two calls return the same count, face, cell, or order. Every source
/// observation is captured once after count preflight, then all structural and
/// admission checks consume only this stable view.
struct CapturedInitialLayerSourceV1<'a> {
    material_faces: Vec<FaceId>,
    folded_faces: Vec<NonFlatFoldedFaceStructuralRefV1<'a>>,
    overlap_cells: Vec<NonFlatOverlapCellStructuralRefV1<'a>>,
    directed_orders: Vec<NonFlatFacePairOrderStructuralV1>,
    tested_pairs: usize,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<(EdgeId, u64)>,
    paper_thickness_bits: u64,
}

impl NonFlatLayerOrderStructuralSourceV1 for CapturedInitialLayerSourceV1<'_> {
    fn material_face_count(&self) -> usize {
        self.material_faces.len()
    }

    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        self.material_faces.get(index).copied()
    }

    fn folded_face_count(&self) -> usize {
        self.folded_faces.len()
    }

    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        self.folded_faces
            .get(index)
            .map(|folded| NonFlatFoldedFaceStructuralRefV1 {
                face_id: folded.face_id,
                dropped_world_axis: folded.dropped_world_axis,
                source_to_plane: folded.source_to_plane,
            })
    }

    fn overlap_cell_count(&self) -> usize {
        self.overlap_cells.len()
    }

    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        self.overlap_cells
            .get(index)
            .map(|cell| NonFlatOverlapCellStructuralRefV1 {
                boundary: cell.boundary,
                exact_boundary: cell.exact_boundary,
                lower_face: cell.lower_face,
                upper_face: cell.upper_face,
            })
    }

    fn face_pair_order_count(&self) -> usize {
        self.directed_orders.len()
    }

    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        self.directed_orders.get(index).copied()
    }
}

fn initial_layer_resource_limit_v1<T>(
    result: Result<(), T>,
) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
    result.map_err(|_| StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
}

fn initial_layer_unavailable_v1<T>() -> Result<T, StackedFoldPathDiagnosticErrorV1> {
    Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)
}

fn preflight_initial_layer_admission_counts_v1(
    counts: InitialLayerAdmissionCountsV1,
    limits: StaticCollisionLimits,
) -> Result<usize, StackedFoldPathDiagnosticErrorV1> {
    if counts.model_faces == 0 {
        return initial_layer_unavailable_v1();
    }
    if counts.model_faces > limits.max_faces
        || counts.material_faces > limits.max_faces
        || counts.folded_faces > limits.max_faces
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
    }
    let expected_pairs = counts
        .model_faces
        .checked_mul(counts.model_faces.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
    let pair_limit = limits
        .max_unordered_face_pairs
        .min(NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1);
    if expected_pairs > pair_limit
        || counts.overlap_cells > pair_limit
        || counts.directed_orders > pair_limit
        || counts.tested_pairs > pair_limit
        || counts.overlap_cells > expected_pairs
        || counts.directed_orders > expected_pairs
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
    }
    let hinge_limit = counts.model_faces.saturating_sub(1);
    if counts.pose_hinges > hinge_limit || counts.source_hinges > hinge_limit {
        return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
    }
    if counts.material_faces != counts.model_faces
        || counts.folded_faces != counts.model_faces
        || counts.overlap_cells != counts.directed_orders
        || counts.tested_pairs != expected_pairs
        || counts.pose_hinges != counts.source_hinges
    {
        return initial_layer_unavailable_v1();
    }
    Ok(expected_pairs)
}

fn capture_initial_layer_source_v1<'a, T>(
    source: &'a T,
    model_faces: usize,
    pose_hinges: usize,
    limits: StaticCollisionLimits,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(CapturedInitialLayerSourceV1<'a>, usize), StackedFoldPathDiagnosticErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    initial_layer_checkpoint_v1(control)?;
    let counts = InitialLayerAdmissionCountsV1 {
        model_faces,
        material_faces: source.material_face_count(),
        folded_faces: source.folded_face_count(),
        overlap_cells: source.overlap_cell_count(),
        directed_orders: source.face_pair_order_count(),
        tested_pairs: source.tested_face_pairs_v1(),
        pose_hinges,
        source_hinges: source.hinge_angle_count_v1(),
    };
    initial_layer_checkpoint_v1(control)?;
    let expected_pairs = preflight_initial_layer_admission_counts_v1(counts, limits)?;
    let mut exact_payload = InitialLayerExactPayloadPreflightV1::new(limits);

    let mut material_faces = Vec::new();
    initial_layer_resource_limit_v1(material_faces.try_reserve_exact(counts.material_faces))?;
    let mut folded_faces = Vec::new();
    initial_layer_resource_limit_v1(folded_faces.try_reserve_exact(counts.folded_faces))?;
    let mut overlap_cells = Vec::new();
    initial_layer_resource_limit_v1(overlap_cells.try_reserve_exact(counts.overlap_cells))?;
    let mut directed_orders = Vec::new();
    initial_layer_resource_limit_v1(directed_orders.try_reserve_exact(counts.directed_orders))?;
    let mut hinge_angles = Vec::new();
    initial_layer_resource_limit_v1(hinge_angles.try_reserve_exact(counts.source_hinges))?;

    for index in 0..counts.material_faces {
        initial_layer_checkpoint_v1(control)?;
        material_faces.push(
            source
                .material_face_id(index)
                .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?,
        );
    }
    for index in 0..counts.folded_faces {
        initial_layer_checkpoint_v1(control)?;
        let folded = source
            .folded_face(index)
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?;
        exact_payload.charge_transform_with_control(folded.source_to_plane, control)?;
        folded_faces.push(folded);
    }

    let maximum_per_cell = limits
        .max_boundary_vertices_per_face
        .min(MAX_STACKED_FOLD_INITIAL_LAYER_BOUNDARY_POINTS_PER_CELL_V1);
    let maximum_total = limits
        .max_total_boundary_vertices
        .min(MAX_STACKED_FOLD_INITIAL_LAYER_TOTAL_BOUNDARY_POINTS_V1);
    let mut total = 0_usize;
    for index in 0..counts.overlap_cells {
        initial_layer_checkpoint_v1(control)?;
        let cell = source
            .overlap_cell(index)
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?;
        if cell.boundary.len() != cell.exact_boundary.len() {
            return initial_layer_unavailable_v1();
        }
        if cell.boundary.len() > maximum_per_cell {
            return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
        }
        total = total
            .checked_add(cell.boundary.len())
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
        if total > maximum_total {
            return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
        }
        exact_payload.charge_boundary_with_control(cell.exact_boundary, control)?;
        overlap_cells.push(cell);
    }
    for index in 0..counts.directed_orders {
        initial_layer_checkpoint_v1(control)?;
        directed_orders.push(
            source
                .face_pair_order(index)
                .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?,
        );
    }
    for index in 0..counts.source_hinges {
        initial_layer_checkpoint_v1(control)?;
        hinge_angles.push(
            source
                .hinge_angle_v1(index)
                .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?,
        );
    }

    initial_layer_checkpoint_v1(control)?;
    let fixed_face = source.fixed_face_v1();
    initial_layer_checkpoint_v1(control)?;
    let paper_thickness_bits = source.paper_thickness_bits_v1();
    initial_layer_checkpoint_v1(control)?;
    Ok((
        CapturedInitialLayerSourceV1 {
            material_faces,
            folded_faces,
            overlap_cells,
            directed_orders,
            tested_pairs: counts.tested_pairs,
            fixed_face,
            hinge_angles,
            paper_thickness_bits,
        },
        expected_pairs,
    ))
}

fn initial_layer_canonical_pair_v1(first: FaceId, second: FaceId) -> (FaceId, FaceId) {
    if first.canonical_bytes() < second.canonical_bytes() {
        (first, second)
    } else {
        (second, first)
    }
}

fn validate_initial_layer_order_dag_v1(
    face_count: usize,
    directed_edges: &[(usize, usize)],
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
    if directed_edges.len() > NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1 {
        return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
    }

    let mut indegree = Vec::<usize>::new();
    initial_layer_resource_limit_v1(indegree.try_reserve_exact(face_count))?;
    indegree.resize(face_count, 0);
    let mut outdegree = Vec::<usize>::new();
    initial_layer_resource_limit_v1(outdegree.try_reserve_exact(face_count))?;
    outdegree.resize(face_count, 0);
    let maximum_outdegree = face_count.saturating_sub(1);
    for &(lower, upper) in directed_edges {
        initial_layer_checkpoint_v1(control)?;
        if lower >= face_count || upper >= face_count || lower == upper {
            return initial_layer_unavailable_v1();
        }
        indegree[upper] = indegree[upper]
            .checked_add(1)
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
        let next_outdegree = outdegree[lower]
            .checked_add(1)
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
        if next_outdegree > maximum_outdegree {
            return Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit);
        }
        outdegree[lower] = next_outdegree;
    }

    let mut outgoing = Vec::<Vec<usize>>::new();
    initial_layer_resource_limit_v1(outgoing.try_reserve_exact(face_count))?;
    for degree in &outdegree {
        initial_layer_checkpoint_v1(control)?;
        let mut next_faces = Vec::new();
        initial_layer_resource_limit_v1(next_faces.try_reserve_exact(*degree))?;
        outgoing.push(next_faces);
    }
    for &(lower, upper) in directed_edges {
        initial_layer_checkpoint_v1(control)?;
        outgoing[lower].push(upper);
    }

    let mut queue = VecDeque::new();
    initial_layer_resource_limit_v1(queue.try_reserve(face_count))?;
    for (face, degree) in indegree.iter().copied().enumerate() {
        initial_layer_checkpoint_v1(control)?;
        if degree == 0 {
            queue.push_back(face);
        }
    }
    let mut visited = 0_usize;
    while let Some(face) = queue.pop_front() {
        initial_layer_checkpoint_v1(control)?;
        visited = visited
            .checked_add(1)
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)?;
        for &next_face in &outgoing[face] {
            initial_layer_checkpoint_v1(control)?;
            let next_degree = indegree[next_face]
                .checked_sub(1)
                .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?;
            indegree[next_face] = next_degree;
            if next_degree == 0 {
                queue.push_back(next_face);
            }
        }
    }
    if visited != face_count {
        return initial_layer_unavailable_v1();
    }
    Ok(())
}

fn map_structural_validation_error_v1(
    error: NonFlatCellTransportErrorV1,
) -> StackedFoldPathDiagnosticErrorV1 {
    match error {
        NonFlatCellTransportErrorV1::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
        NonFlatCellTransportErrorV1::DeadlineExceeded => {
            StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
        }
        NonFlatCellTransportErrorV1::ResourceLimit => {
            StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit
        }
        NonFlatCellTransportErrorV1::BindingMismatch
        | NonFlatCellTransportErrorV1::IncompleteCoverage
        | NonFlatCellTransportErrorV1::Crossing => {
            StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable
        }
    }
}

fn initial_layer_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
        }
    })
}

fn map_initial_layer_static_error_v1(
    error: StaticCollisionError,
) -> StackedFoldPathDiagnosticErrorV1 {
    match error {
        StaticCollisionError::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
        StaticCollisionError::DeadlineExceeded => {
            StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
        }
        _ => StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable,
    }
}

/// Prepares a fail-closed admission for a flat initial sample whose unresolved
/// exact static pairs are all covered by one acyclic, source-derived layer
/// order.
///
/// The general static diagnosis remains authoritative: only pairs it reports
/// as `Indeterminate` with exact `SharedFeatureFlatStack` evidence are
/// admissible. Every such canonical pair must have exactly one directed order,
/// and no order may be present for another pair.
pub fn prepare_stacked_fold_initial_sample_layer_admission_v1<T>(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    paper_thickness_mm: f64,
    static_limits: StaticCollisionLimits,
    source: &T,
) -> Result<NativeStackedFoldInitialSampleLayerAdmissionV1<T>, StackedFoldPathDiagnosticErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        model,
        initial_pose,
        paper_thickness_mm,
        static_limits,
        source,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Controlled form of [`prepare_stacked_fold_initial_sample_layer_admission_v1`].
/// A cancellation or deadline is returned before the opaque admission is
/// constructed, so no partial source capture or proof can be published.
pub fn prepare_stacked_fold_initial_sample_layer_admission_with_control_v1<T>(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    paper_thickness_mm: f64,
    static_limits: StaticCollisionLimits,
    source: &T,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<NativeStackedFoldInitialSampleLayerAdmissionV1<T>, StackedFoldPathDiagnosticErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    initial_layer_checkpoint_v1(control)?;
    let zero_bits = 0.0_f64.to_bits();
    if paper_thickness_mm.to_bits() != zero_bits {
        return initial_layer_unavailable_v1();
    }
    model
        .bind_pose(initial_pose)
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch)?;
    initial_layer_checkpoint_v1(control)?;

    let (captured, expected_pairs) = capture_initial_layer_source_v1(
        source,
        model.face_ids().len(),
        initial_pose.hinge_angles().len(),
        static_limits,
        control,
    )?;
    initial_layer_checkpoint_v1(control)?;
    if captured.paper_thickness_bits != zero_bits
        || captured.paper_thickness_bits != paper_thickness_mm.to_bits()
    {
        return initial_layer_unavailable_v1();
    }
    validate_non_flat_layer_order_structure_with_control_v1(&captured, control)
        .map_err(map_structural_validation_error_v1)?;
    initial_layer_checkpoint_v1(control)?;
    if captured.tested_pairs != expected_pairs {
        return initial_layer_unavailable_v1();
    }

    let mut face_index = HashMap::<FaceId, usize>::new();
    initial_layer_resource_limit_v1(face_index.try_reserve(model.face_ids().len()))?;
    for (index, face) in model.face_ids().iter().copied().enumerate() {
        initial_layer_checkpoint_v1(control)?;
        if face_index.insert(face, index).is_some() {
            return initial_layer_unavailable_v1();
        }
    }
    let mut source_faces_seen = Vec::<bool>::new();
    initial_layer_resource_limit_v1(
        source_faces_seen.try_reserve_exact(captured.material_faces.len()),
    )?;
    source_faces_seen.resize(captured.material_faces.len(), false);
    for face in &captured.material_faces {
        initial_layer_checkpoint_v1(control)?;
        let source_index = face_index
            .get(face)
            .copied()
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?;
        if source_faces_seen[source_index] {
            return initial_layer_unavailable_v1();
        }
        source_faces_seen[source_index] = true;
    }
    if source_faces_seen.iter().any(|seen| !seen) {
        return initial_layer_unavailable_v1();
    }
    if captured.fixed_face != initial_pose.fixed_face() {
        return initial_layer_unavailable_v1();
    }
    for (angle, captured_angle) in initial_pose
        .hinge_angles()
        .iter()
        .zip(&captured.hinge_angles)
    {
        initial_layer_checkpoint_v1(control)?;
        let bits = angle.angle_degrees().to_bits();
        if !matches!(bits, value if value == zero_bits || value == 180.0_f64.to_bits())
            || *captured_angle != (angle.edge(), bits)
        {
            return initial_layer_unavailable_v1();
        }
    }

    let mut ordered_pairs = HashSet::<(FaceId, FaceId)>::new();
    initial_layer_resource_limit_v1(ordered_pairs.try_reserve(captured.directed_orders.len()))?;
    let mut directed_edges = Vec::<(usize, usize)>::new();
    initial_layer_resource_limit_v1(
        directed_edges.try_reserve_exact(captured.directed_orders.len()),
    )?;
    for order in captured.directed_orders.iter().copied() {
        initial_layer_checkpoint_v1(control)?;
        if order.lower_face == order.upper_face {
            return initial_layer_unavailable_v1();
        }
        let lower = face_index
            .get(&order.lower_face)
            .copied()
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?;
        let upper = face_index
            .get(&order.upper_face)
            .copied()
            .ok_or(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)?;
        let canonical = initial_layer_canonical_pair_v1(order.lower_face, order.upper_face);
        if !ordered_pairs.insert(canonical) {
            return initial_layer_unavailable_v1();
        }
        directed_edges.push((lower, upper));
    }
    validate_initial_layer_order_dag_v1(model.face_ids().len(), &directed_edges, control)?;

    initial_layer_checkpoint_v1(control)?;
    let initial_static_snapshot = diagnose_static_collision_geometry_with_control_v1(
        model,
        initial_pose,
        paper_thickness_mm,
        static_limits,
        control,
    )
    .map_err(map_initial_layer_static_error_v1)?;
    if initial_static_snapshot.expected_unordered_face_pairs() != expected_pairs
        || initial_static_snapshot.pairs().len() != expected_pairs
        || initial_static_snapshot.penetrating_pairs() != 0
        || initial_static_snapshot.indeterminate_pairs() == 0
        || initial_static_snapshot.candidate_excluded_pairs() != 0
    {
        return initial_layer_unavailable_v1();
    }
    let mut diagnosed_pairs = HashSet::<(FaceId, FaceId)>::new();
    initial_layer_resource_limit_v1(diagnosed_pairs.try_reserve(expected_pairs))?;
    let mut initial_flat_pairs = Vec::new();
    initial_layer_resource_limit_v1(initial_flat_pairs.try_reserve_exact(ordered_pairs.len()))?;
    let mut persistent_flat_hinges = Vec::new();
    initial_layer_resource_limit_v1(persistent_flat_hinges.try_reserve_exact(ordered_pairs.len()))?;
    for pair in initial_static_snapshot.pairs() {
        initial_layer_checkpoint_v1(control)?;
        if pair.first_face() == pair.second_face()
            || !face_index.contains_key(&pair.first_face())
            || !face_index.contains_key(&pair.second_face())
        {
            return initial_layer_unavailable_v1();
        }
        let canonical = initial_layer_canonical_pair_v1(pair.first_face(), pair.second_face());
        if !diagnosed_pairs.insert(canonical) {
            return initial_layer_unavailable_v1();
        }
        let layer_ordered = ordered_pairs.contains(&canonical);
        let direct_hinge = direct_shared_hinge_v1(model, canonical.0, canonical.1);
        let decision = classify_initial_layer_pair_admission_v1(
            canonical,
            layer_ordered,
            pair.disposition(),
            pair.evidence(),
            direct_hinge,
            direct_hinge.and_then(|hinge| hinge_angle_bits_v1(initial_pose, hinge)),
        );
        match decision.kind {
            InitialLayerPairAdmissionKindV1::PersistentFlatStack { hinge } => {
                initial_flat_pairs.push(InitialFlatPairAdmissionV1 {
                    first_face: decision.pair.0,
                    second_face: decision.pair.1,
                });
                persistent_flat_hinges.push(PersistentFlatHingeAdmissionV1 {
                    first_face: decision.pair.0,
                    second_face: decision.pair.1,
                    hinge,
                });
            }
            InitialLayerPairAdmissionKindV1::InitialOnlyFlatStack => {
                initial_flat_pairs.push(InitialFlatPairAdmissionV1 {
                    first_face: decision.pair.0,
                    second_face: decision.pair.1,
                });
            }
            InitialLayerPairAdmissionKindV1::Rejected => {
                return initial_layer_unavailable_v1();
            }
            InitialLayerPairAdmissionKindV1::UnorderedNonblocking => {}
        }
    }
    if diagnosed_pairs.len() != expected_pairs
        || ordered_pairs.len() != initial_static_snapshot.indeterminate_pairs()
        || initial_flat_pairs.len() != ordered_pairs.len()
    {
        return initial_layer_unavailable_v1();
    }
    initial_layer_checkpoint_v1(control)?;
    initial_flat_pairs.sort_unstable_by_key(|entry| {
        (
            entry.first_face.canonical_bytes(),
            entry.second_face.canonical_bytes(),
        )
    });
    for entries in initial_flat_pairs.windows(2) {
        initial_layer_checkpoint_v1(control)?;
        if entries[0].first_face == entries[1].first_face
            && entries[0].second_face == entries[1].second_face
        {
            return initial_layer_unavailable_v1();
        }
    }
    initial_layer_checkpoint_v1(control)?;
    persistent_flat_hinges.sort_unstable_by_key(|entry| {
        (
            entry.first_face.canonical_bytes(),
            entry.second_face.canonical_bytes(),
        )
    });
    for entries in persistent_flat_hinges.windows(2) {
        initial_layer_checkpoint_v1(control)?;
        if entries[0].first_face == entries[1].first_face
            && entries[0].second_face == entries[1].second_face
        {
            return initial_layer_unavailable_v1();
        }
    }
    let mut directed_orders = captured.directed_orders;
    directed_orders.sort_unstable_by_key(|order| {
        let pair = initial_layer_canonical_pair_v1(order.lower_face, order.upper_face);
        (pair.0.canonical_bytes(), pair.1.canonical_bytes())
    });

    initial_layer_checkpoint_v1(control)?;
    Ok(NativeStackedFoldInitialSampleLayerAdmissionV1 {
        proof: Arc::new(InitialSampleLayerAdmissionProofV1 {
            model: model.clone(),
            pose: initial_pose.clone(),
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            initial_static_snapshot,
            initial_flat_pairs,
            persistent_flat_hinges,
            directed_orders,
        }),
        issuer: Arc::new(()),
        source_type: PhantomData,
    })
}

#[cfg(test)]
mod tests;
