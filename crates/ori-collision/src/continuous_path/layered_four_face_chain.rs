//! A deliberately narrow layered continuous certificate for one four-face
//! material-tree chain. It does not widen the ordinary continuous
//! certificates.

use std::sync::Arc;

use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1, StaticCollisionError,
    StaticCollisionLimits, diagnose_static_collision_geometry_with_control_v1,
};
use ori_domain::{EdgeId, FaceId};
#[cfg(test)]
use ori_kinematics::OutwardIntervalV1;
use ori_kinematics::{
    CanonicalHingeAngles, MaterialTreeDyadicFaceIntervalRegistryV1,
    MaterialTreeDyadicIntervalErrorV1, MaterialTreeDyadicIntervalLimitsV1,
    MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

#[cfg(test)]
use super::layered_chain_common::{
    LayeredChainNonadjacentIntervalPairV1 as NonadjacentIntervalPairV1, pair_key_v1,
    verify_layered_chain_nonadjacent_gaps_with_control_v1,
};
use super::{
    NativeStackedFoldInitialSampleLayerAdmissionV1, StackedFoldInitialLayerOrderSourceV1,
    initial_sample_layer_admission::{
        StationaryFlatStackTransportBindingV1, stationary_flat_stack_transport_bindings_v1,
    },
    initial_sample_layer_admission_has_issuer_v1,
    initial_sample_layer_admission_matches_snapshot_v1,
    layered_chain_common::{
        LayeredChainIntervalErrorV1, bounded_face_boundaries_v1, canonical_pair_v1,
        layered_continuous_resource_limits_within_hard_caps_v1,
        layered_leaf_count_v1 as leaf_count_v1, validate_linear_chain_hinges_v1,
        validate_single_moving_flat_chain_schedule_v1,
        verify_layered_chain_nonadjacent_registry_gaps_with_control_v1,
    },
    layered_three_face::direct_hinge_boundary_only_open_interval_theorem_v1,
    retain_initial_sample_layer_admission_issuer_v1,
};

pub const LAYERED_FOUR_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "layered_four_face_chain_dyadic_continuous_certificate_v1";

const MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_FACES_V1: usize = 4;
const MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1: usize = 3;
const MAX_LAYERED_FOUR_FACE_CHAIN_NONADJACENT_PAIRS_V1: usize = 3;
const MAX_LAYERED_FOUR_FACE_CHAIN_ALL_PAIRS_V1: usize = 6;

/// Bounded work limits for the exact four-face chain theorem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayeredFourFaceChainContinuousLimitsV1 {
    /// The complete path is partitioned into `2^dyadic_depth` closed leaves,
    /// additionally capped by [`super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1`].
    pub dyadic_depth: u8,
    pub max_leaves: usize,
    /// Must cover the complete set of three nonadjacent face pairs.
    pub max_nonadjacent_pairs: usize,
    /// Caller-selectable budgets bounded by the layered V1 hard ceilings.
    pub interval_limits: MaterialTreeDyadicIntervalLimitsV1,
    /// Caller-selectable budgets bounded by the layered V1 static ceilings.
    pub static_limits: StaticCollisionLimits,
}

impl Default for LayeredFourFaceChainContinuousLimitsV1 {
    fn default() -> Self {
        let interval_limits = MaterialTreeDyadicIntervalLimitsV1 {
            max_faces: MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_FACES_V1,
            max_hinges: MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1,
            ..MaterialTreeDyadicIntervalLimitsV1::default()
        };
        Self {
            dyadic_depth: 0,
            max_leaves: 1,
            max_nonadjacent_pairs: MAX_LAYERED_FOUR_FACE_CHAIN_NONADJACENT_PAIRS_V1,
            interval_limits,
            static_limits: StaticCollisionLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LayeredFourFaceChainContinuousErrorV1 {
    #[error("the layered certificate only supports exactly four faces and three chain hinges")]
    UnsupportedTree,
    #[error(
        "the path must have one 0-to-(0,180) moving hinge and two stationary 180-degree hinges"
    )]
    InvalidAngleSchedule,
    #[error("the initial sample-layer admission could not be exactly revalidated")]
    InitialLayerAdmissionUnavailable,
    #[error(
        "the moving direct-hinge pair was not proven boundary-only on its complete open interval"
    )]
    MovingBoundaryOnlyUnavailable,
    #[error(
        "a stationary flat-stack order could not be transported by one constant direct-hinge transform"
    )]
    StationaryLayerTransportUnavailable,
    #[error("a nonadjacent pair touches or its outward intervals overlap")]
    NonadjacentIntervalOverlap,
    #[error("the exact six-pair partition is inconsistent")]
    PairPartitionUnavailable,
    #[error("the material-pose issuer drifted")]
    IssuerMismatch,
    #[error("the bounded layered proof exhausted a resource limit")]
    ResourceLimit,
    #[error("the bounded layered proof was cancelled")]
    Cancelled,
    #[error("the bounded layered proof deadline elapsed")]
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FourFaceChainPairPartitionV1 {
    pub(super) direct_pairs: [[FaceId; 2]; 3],
    pub(super) nonadjacent_pairs: [[FaceId; 2]; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FourFaceChainDirectHingeV1 {
    pub(super) edge: EdgeId,
    pub(super) pair: [FaceId; 2],
    pub(super) moving: bool,
}

/// Native direct-hinge recurrence evidence for one source-authenticated
/// stationary flat-stack order.
///
/// In a material tree the child-face transform is the parent transform
/// composed with the direct hinge's material-local rotation. If that hinge's
/// source and target are both bit-exact 180 degrees, linear interpolation is
/// the singleton 180-degree angle on every dyadic leaf. The local rotation is
/// therefore source-identical everywhere, so both incident faces differ from
/// their source transforms by one common world rigid transform. Applying one
/// bijective rigid transform to both faces preserves the authenticated
/// lower/upper order without accepting a sampled-order witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StationaryFlatPairTransportV1 {
    binding: StationaryFlatStackTransportBindingV1,
    source_angle_bits: u64,
    target_angle_bits: u64,
    dyadic_depth: u8,
    leaf_count: usize,
}

/// Opaque proof for all six pairs of one exact four-face chain.
///
/// The type deliberately implements neither `Clone` nor serialization. Its
/// retained interval registries and private admission issuer bind it to the
/// exact model, source pose, target schedule, initial-layer authority and
/// work limits used at issuance. Private stationary-pair evidence additionally
/// binds each directed source order to the native constant-relative-transform
/// theorem on every retained leaf.
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_collision::LayeredFourFaceChainContinuousCertificateV1>();
/// ```
#[derive(Debug)]
pub struct LayeredFourFaceChainContinuousCertificateV1 {
    source_pose: MaterialTreePose,
    target_angles: CanonicalHingeAngles,
    direct_hinges: [FourFaceChainDirectHingeV1; 3],
    pair_partition: FourFaceChainPairPartitionV1,
    stationary_transports: [StationaryFlatPairTransportV1; 2],
    admission_issuer: Arc<()>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
    leaves: Vec<MaterialTreeDyadicFaceIntervalRegistryV1>,
}

impl LayeredFourFaceChainContinuousCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        LAYERED_FOUR_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
    }

    #[must_use]
    pub fn moving_hinge(&self) -> EdgeId {
        self.direct_hinges
            .iter()
            .find(|hinge| hinge.moving)
            .expect("issued certificate has exactly one moving hinge")
            .edge
    }

    /// Canonical unordered direct pairs followed by canonical unordered
    /// nonadjacent pairs. Each group is sorted by face identity.
    #[must_use]
    pub const fn pair_partition(&self) -> [[FaceId; 2]; 6] {
        [
            self.pair_partition.direct_pairs[0],
            self.pair_partition.direct_pairs[1],
            self.pair_partition.direct_pairs[2],
            self.pair_partition.nonadjacent_pairs[0],
            self.pair_partition.nonadjacent_pairs[1],
            self.pair_partition.nonadjacent_pairs[2],
        ]
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub fn is_for<T>(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_angles: &CanonicalHingeAngles,
        admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
        limits: LayeredFourFaceChainContinuousLimitsV1,
    ) -> bool
    where
        T: StackedFoldInitialLayerOrderSourceV1,
    {
        self.is_for_with_control_v1(
            model,
            source_pose,
            target_angles,
            admission,
            limits,
            &CooperativeOperationControlV1::unbounded(),
        )
        .unwrap_or(false)
    }

    /// Revalidates every binding and theorem premise. A stopped or partial
    /// operation can never return a truthy result.
    pub fn is_for_with_control_v1<T>(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_angles: &CanonicalHingeAngles,
        admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
        limits: LayeredFourFaceChainContinuousLimitsV1,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<bool, LayeredFourFaceChainContinuousErrorV1>
    where
        T: StackedFoldInitialLayerOrderSourceV1,
    {
        certificate_checkpoint_v1(control)?;
        let Some(leaf_count) = leaf_count_v1(limits.dyadic_depth, limits.max_leaves) else {
            return Ok(false);
        };
        if self.limits != limits
            || limits.max_nonadjacent_pairs != MAX_LAYERED_FOUR_FACE_CHAIN_NONADJACENT_PAIRS_V1
            || !layered_continuous_resource_limits_within_hard_caps_v1(
                limits.interval_limits,
                MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_FACES_V1,
                MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1,
                limits.static_limits,
            )
            || !self.source_pose.same_instance(source_pose)
            || self.target_angles != *target_angles
            || !model.owns_pose(source_pose)
            || !initial_sample_layer_admission_has_issuer_v1(&self.admission_issuer, admission)
            || !bounded_face_boundaries_v1(model, limits.interval_limits.max_vertices)
            || self.leaves.len() != leaf_count
        {
            return Ok(false);
        }
        let Some((partition, direct_hinges)) =
            matches_four_face_chain_schedule_v1(model, source_pose, target_angles)
        else {
            return Ok(false);
        };
        if partition != self.pair_partition || direct_hinges != self.direct_hinges {
            return Ok(false);
        }

        certificate_checkpoint_v1(control)?;
        let snapshot = diagnose_static_collision_geometry_with_control_v1(
            model,
            source_pose,
            0.0,
            limits.static_limits,
            control,
        )
        .map_err(map_static_error_v1)?;
        if !initial_sample_layer_admission_matches_snapshot_v1(
            admission,
            model,
            source_pose,
            0.0,
            &snapshot,
        ) {
            return Ok(false);
        }
        let Some(stationary_transports) =
            bind_stationary_flat_pair_transports_v1(StationaryFlatPairTransportInputV1 {
                model,
                source_pose,
                target_angles,
                admission,
                direct_hinges: &direct_hinges,
                dyadic_depth: limits.dyadic_depth,
                leaf_count,
                control,
            })?
        else {
            return Ok(false);
        };
        if stationary_transports != self.stationary_transports {
            return Ok(false);
        }

        certificate_checkpoint_v1(control)?;
        let target_pose = match model.solve(source_pose.fixed_face(), target_angles) {
            Ok(pose) => pose,
            Err(_) => return Ok(false),
        };
        let Some(moving_hinge) = self.direct_hinges.iter().find(|hinge| hinge.moving) else {
            return Ok(false);
        };
        certificate_checkpoint_v1(control)?;
        if !direct_hinge_boundary_only_open_interval_theorem_v1(
            model,
            source_pose,
            &target_pose,
            moving_hinge.edge,
            moving_hinge.pair,
            control,
        )
        .map_err(map_boundary_error_v1)?
        {
            return Ok(false);
        }

        for (index, registry) in self.leaves.iter().enumerate() {
            certificate_checkpoint_v1(control)?;
            if !stationary_transports_cover_leaf_v1(
                &self.stationary_transports,
                limits.dyadic_depth,
                index,
            ) {
                return Ok(false);
            }
            if !registry.is_for(
                model,
                source_pose,
                target_angles,
                limits.dyadic_depth,
                index as u64,
            ) {
                return Ok(false);
            }
            verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
                registry,
                &self.pair_partition.nonadjacent_pairs,
                limits.max_nonadjacent_pairs,
                control,
            )
            .map_err(map_four_face_interval_error_v1)?;
        }
        certificate_checkpoint_v1(control)?;
        Ok(true)
    }
}

/// Certifies all six face pairs of the complete closed linear path.
///
/// The exact initial admission supplies the directed source order for the two
/// stationary, bit-exact 180-degree flat-stack pairs. A native direct-hinge
/// recurrence theorem transports each order because its relative material
/// transform is constant on every dyadic leaf. The moving pair is
/// boundary-only at its 0-degree endpoint and must independently satisfy the
/// authenticated direct-hinge theorem on the complete open interval. Every
/// nonadjacent pair must have a strict outward interval gap on every retained
/// dyadic leaf; touching is rejected.
pub fn certify_layered_four_face_chain_continuous_path_with_control_v1<T>(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<LayeredFourFaceChainContinuousCertificateV1, LayeredFourFaceChainContinuousErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    certificate_checkpoint_v1(control)?;
    if !model.owns_pose(source_pose) {
        return Err(LayeredFourFaceChainContinuousErrorV1::IssuerMismatch);
    }
    if model.face_ids().len() != MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_FACES_V1
        || model.hinges().len() != MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1
    {
        return Err(LayeredFourFaceChainContinuousErrorV1::UnsupportedTree);
    }
    let leaf_count = leaf_count_v1(limits.dyadic_depth, limits.max_leaves)
        .ok_or(LayeredFourFaceChainContinuousErrorV1::ResourceLimit)?;
    if limits.max_nonadjacent_pairs != MAX_LAYERED_FOUR_FACE_CHAIN_NONADJACENT_PAIRS_V1
        || !layered_continuous_resource_limits_within_hard_caps_v1(
            limits.interval_limits,
            MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_FACES_V1,
            MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1,
            limits.static_limits,
        )
        || !bounded_face_boundaries_v1(model, limits.interval_limits.max_vertices)
    {
        return Err(LayeredFourFaceChainContinuousErrorV1::ResourceLimit);
    }
    let (pair_partition, direct_hinges) =
        matches_four_face_chain_schedule_v1(model, source_pose, target_angles)
            .ok_or(LayeredFourFaceChainContinuousErrorV1::InvalidAngleSchedule)?;

    certificate_checkpoint_v1(control)?;
    let snapshot = diagnose_static_collision_geometry_with_control_v1(
        model,
        source_pose,
        0.0,
        limits.static_limits,
        control,
    )
    .map_err(map_static_error_v1)?;
    if !initial_sample_layer_admission_matches_snapshot_v1(
        admission,
        model,
        source_pose,
        0.0,
        &snapshot,
    ) {
        return Err(LayeredFourFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable);
    }
    let stationary_transports =
        bind_stationary_flat_pair_transports_v1(StationaryFlatPairTransportInputV1 {
            model,
            source_pose,
            target_angles,
            admission,
            direct_hinges: &direct_hinges,
            dyadic_depth: limits.dyadic_depth,
            leaf_count,
            control,
        })?
        .ok_or(LayeredFourFaceChainContinuousErrorV1::StationaryLayerTransportUnavailable)?;

    certificate_checkpoint_v1(control)?;
    let target_pose = model
        .solve(source_pose.fixed_face(), target_angles)
        .map_err(|_| LayeredFourFaceChainContinuousErrorV1::IssuerMismatch)?;
    let moving_hinge = direct_hinges
        .iter()
        .find(|hinge| hinge.moving)
        .ok_or(LayeredFourFaceChainContinuousErrorV1::InvalidAngleSchedule)?;
    certificate_checkpoint_v1(control)?;
    if !direct_hinge_boundary_only_open_interval_theorem_v1(
        model,
        source_pose,
        &target_pose,
        moving_hinge.edge,
        moving_hinge.pair,
        control,
    )
    .map_err(map_boundary_error_v1)?
    {
        return Err(LayeredFourFaceChainContinuousErrorV1::MovingBoundaryOnlyUnavailable);
    }

    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(leaf_count)
        .map_err(|_| LayeredFourFaceChainContinuousErrorV1::ResourceLimit)?;
    for index in 0..leaf_count {
        certificate_checkpoint_v1(control)?;
        if !stationary_transports_cover_leaf_v1(&stationary_transports, limits.dyadic_depth, index)
        {
            return Err(LayeredFourFaceChainContinuousErrorV1::StationaryLayerTransportUnavailable);
        }
        let registry = model
            .prepare_dyadic_face_vertex_intervals_with_checkpoint_v1(
                source_pose,
                target_angles,
                limits.dyadic_depth,
                index as u64,
                limits.interval_limits,
                || control.checkpoint().is_ok(),
            )
            .map_err(|error| map_dyadic_interval_error_v1(error, control))?;
        verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &pair_partition.nonadjacent_pairs,
            limits.max_nonadjacent_pairs,
            control,
        )
        .map_err(map_four_face_interval_error_v1)?;
        leaves.push(registry);
    }
    certificate_checkpoint_v1(control)?;
    Ok(LayeredFourFaceChainContinuousCertificateV1 {
        source_pose: source_pose.clone(),
        target_angles: target_angles.clone(),
        direct_hinges,
        pair_partition,
        stationary_transports,
        admission_issuer: retain_initial_sample_layer_admission_issuer_v1(admission),
        limits,
        leaves,
    })
}

pub fn certify_layered_four_face_chain_continuous_path_v1<T>(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
) -> Result<LayeredFourFaceChainContinuousCertificateV1, LayeredFourFaceChainContinuousErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    certify_layered_four_face_chain_continuous_path_with_control_v1(
        model,
        source_pose,
        target_angles,
        admission,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FourFaceChainIntervalErrorV1 {
    ResourceLimit,
    Cancelled,
    DeadlineExceeded,
    IntervalUnavailable,
    IntervalOverlap,
}

pub(super) fn verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
    registry: &MaterialTreeDyadicFaceIntervalRegistryV1,
    pairs: &[[FaceId; 2]; 3],
    maximum_pairs: usize,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), FourFaceChainIntervalErrorV1> {
    verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
        registry,
        pairs,
        MAX_LAYERED_FOUR_FACE_CHAIN_NONADJACENT_PAIRS_V1,
        maximum_pairs,
        control,
    )
    .map_err(map_layered_chain_interval_error_v1)
}

#[cfg(test)]
fn verify_four_face_chain_nonadjacent_gaps_with_control_v1(
    pairs: &[NonadjacentIntervalPairV1<'_>],
    maximum_pairs: usize,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), FourFaceChainIntervalErrorV1> {
    verify_layered_chain_nonadjacent_gaps_with_control_v1(
        pairs,
        MAX_LAYERED_FOUR_FACE_CHAIN_NONADJACENT_PAIRS_V1,
        maximum_pairs,
        control,
    )
    .map_err(map_layered_chain_interval_error_v1)
}

fn map_layered_chain_interval_error_v1(
    error: LayeredChainIntervalErrorV1,
) -> FourFaceChainIntervalErrorV1 {
    match error {
        LayeredChainIntervalErrorV1::ResourceLimit => FourFaceChainIntervalErrorV1::ResourceLimit,
        LayeredChainIntervalErrorV1::Cancelled => FourFaceChainIntervalErrorV1::Cancelled,
        LayeredChainIntervalErrorV1::DeadlineExceeded => {
            FourFaceChainIntervalErrorV1::DeadlineExceeded
        }
        LayeredChainIntervalErrorV1::IntervalUnavailable => {
            FourFaceChainIntervalErrorV1::IntervalUnavailable
        }
        LayeredChainIntervalErrorV1::IntervalOverlap => {
            FourFaceChainIntervalErrorV1::IntervalOverlap
        }
    }
}

pub(super) fn validate_four_face_chain_schedule_angles_v1(
    direct_hinges: &[(EdgeId, [FaceId; 2])],
    source: &[(EdgeId, f64)],
    target: &[(EdgeId, f64)],
) -> Option<[FourFaceChainDirectHingeV1; 3]> {
    let output = validate_single_moving_flat_chain_schedule_v1(
        direct_hinges,
        source,
        target,
        MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1,
        MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_HINGES_V1,
    )
    .ok()??;
    let output: [_; 3] = output.try_into().ok()?;
    Some(output.map(|hinge| FourFaceChainDirectHingeV1 {
        edge: hinge.edge,
        pair: hinge.pair,
        moving: hinge.moving,
    }))
}

pub(super) fn validate_four_face_chain_hinges_v1(
    faces: &[FaceId],
    hinges: &[(EdgeId, [FaceId; 2])],
) -> Option<FourFaceChainPairPartitionV1> {
    let partition = validate_linear_chain_hinges_v1(
        faces,
        hinges,
        MAX_LAYERED_FOUR_FACE_CHAIN_INTERVAL_FACES_V1,
        MAX_LAYERED_FOUR_FACE_CHAIN_ALL_PAIRS_V1,
    )
    .ok()??;
    Some(FourFaceChainPairPartitionV1 {
        direct_pairs: partition.direct_pairs.try_into().ok()?,
        nonadjacent_pairs: partition.nonadjacent_pairs.try_into().ok()?,
    })
}

fn matches_four_face_chain_schedule_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target: &CanonicalHingeAngles,
) -> Option<(
    FourFaceChainPairPartitionV1,
    [FourFaceChainDirectHingeV1; 3],
)> {
    if model.face_ids().len() != 4
        || model.hinges().len() != 3
        || source_pose.hinge_angles().len() != 3
        || target.as_slice().len() != 3
    {
        return None;
    }
    let direct_hinges = model
        .hinges()
        .iter()
        .map(|hinge| {
            (
                hinge.edge(),
                canonical_pair_v1(hinge.left_face(), hinge.right_face()),
            )
        })
        .collect::<Vec<_>>();
    let partition = validate_four_face_chain_hinges_v1(model.face_ids(), &direct_hinges)?;
    let source = source_pose
        .hinge_angles()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let target = target
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let schedule = validate_four_face_chain_schedule_angles_v1(&direct_hinges, &source, &target)?;
    if schedule.map(|hinge| hinge.pair) != partition.direct_pairs {
        return None;
    }
    Some((partition, schedule))
}

struct StationaryFlatPairTransportInputV1<'a, 'control, T> {
    model: &'a MaterialTreeKinematicsModel,
    source_pose: &'a MaterialTreePose,
    target_angles: &'a CanonicalHingeAngles,
    admission: &'a NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    direct_hinges: &'a [FourFaceChainDirectHingeV1; 3],
    dyadic_depth: u8,
    leaf_count: usize,
    control: &'a CooperativeOperationControlV1<'control>,
}

fn bind_stationary_flat_pair_transports_v1<T>(
    input: StationaryFlatPairTransportInputV1<'_, '_, T>,
) -> Result<Option<[StationaryFlatPairTransportV1; 2]>, LayeredFourFaceChainContinuousErrorV1> {
    let StationaryFlatPairTransportInputV1 {
        model,
        source_pose,
        target_angles,
        admission,
        direct_hinges,
        dyadic_depth,
        leaf_count,
        control,
    } = input;
    certificate_checkpoint_v1(control)?;
    if !model.owns_pose(source_pose) || leaf_count_v1(dyadic_depth, leaf_count) != Some(leaf_count)
    {
        return Ok(None);
    }
    let mut stationary = direct_hinges.iter().filter(|hinge| !hinge.moving);
    let (Some(first), Some(second)) = (stationary.next(), stationary.next()) else {
        return Ok(None);
    };
    if stationary.next().is_some() {
        return Ok(None);
    }
    let expected = [(first.edge, first.pair), (second.edge, second.pair)];
    let Some(bindings) =
        stationary_flat_stack_transport_bindings_v1(admission, model, source_pose, &expected)
    else {
        return Ok(None);
    };
    let bind = |hinge: FourFaceChainDirectHingeV1,
                binding: StationaryFlatStackTransportBindingV1|
     -> Option<StationaryFlatPairTransportV1> {
        let mut tree_hinges = model
            .hinges()
            .iter()
            .filter(|candidate| candidate.edge() == hinge.edge);
        let tree_hinge = tree_hinges.next()?;
        if tree_hinges.next().is_some()
            || canonical_pair_v1(tree_hinge.left_face(), tree_hinge.right_face()) != hinge.pair
            || binding.hinge != hinge.edge
            || binding.pair != hinge.pair
            || !binding.pair.contains(&binding.lower_face)
            || !binding.pair.contains(&binding.upper_face)
            || binding.lower_face == binding.upper_face
        {
            return None;
        }
        let source_angle_bits = angle_bits_v1(source_pose.hinge_angles(), hinge.edge)?;
        let target_angle_bits = angle_bits_v1(target_angles.as_slice(), hinge.edge)?;
        if source_angle_bits != 180.0_f64.to_bits() || target_angle_bits != 180.0_f64.to_bits() {
            return None;
        }
        Some(StationaryFlatPairTransportV1 {
            binding,
            source_angle_bits,
            target_angle_bits,
            dyadic_depth,
            leaf_count,
        })
    };
    certificate_checkpoint_v1(control)?;
    let Some(first_transport) = bind(*first, bindings[0]) else {
        return Ok(None);
    };
    certificate_checkpoint_v1(control)?;
    let Some(second_transport) = bind(*second, bindings[1]) else {
        return Ok(None);
    };
    certificate_checkpoint_v1(control)?;
    Ok(Some([first_transport, second_transport]))
}

fn angle_bits_v1(angles: &[ori_kinematics::HingeAngle], edge: EdgeId) -> Option<u64> {
    let mut matches = angles.iter().filter(|angle| angle.edge() == edge);
    let angle = matches.next()?.angle_degrees().to_bits();
    matches.next().is_none().then_some(angle)
}

fn stationary_transports_cover_leaf_v1(
    transports: &[StationaryFlatPairTransportV1; 2],
    dyadic_depth: u8,
    index: usize,
) -> bool {
    transports.iter().all(|transport| {
        transport.source_angle_bits == 180.0_f64.to_bits()
            && transport.target_angle_bits == 180.0_f64.to_bits()
            && transport.dyadic_depth == dyadic_depth
            && index < transport.leaf_count
    })
}

fn certificate_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), LayeredFourFaceChainContinuousErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => LayeredFourFaceChainContinuousErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
        }
    })
}

fn map_dyadic_interval_error_v1(
    error: MaterialTreeDyadicIntervalErrorV1,
    control: &CooperativeOperationControlV1<'_>,
) -> LayeredFourFaceChainContinuousErrorV1 {
    match error {
        MaterialTreeDyadicIntervalErrorV1::Cancelled => certificate_checkpoint_v1(control)
            .err()
            .unwrap_or(LayeredFourFaceChainContinuousErrorV1::Cancelled),
        MaterialTreeDyadicIntervalErrorV1::ResourceLimit => {
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        }
        MaterialTreeDyadicIntervalErrorV1::SourceIssuerMismatch => {
            LayeredFourFaceChainContinuousErrorV1::IssuerMismatch
        }
        MaterialTreeDyadicIntervalErrorV1::InvalidLeaf
        | MaterialTreeDyadicIntervalErrorV1::SourceAnglesMismatch
        | MaterialTreeDyadicIntervalErrorV1::TargetAnglesMismatch
        | MaterialTreeDyadicIntervalErrorV1::IntervalUnavailable => {
            LayeredFourFaceChainContinuousErrorV1::PairPartitionUnavailable
        }
    }
}

fn map_four_face_interval_error_v1(
    error: FourFaceChainIntervalErrorV1,
) -> LayeredFourFaceChainContinuousErrorV1 {
    match error {
        FourFaceChainIntervalErrorV1::ResourceLimit => {
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        }
        FourFaceChainIntervalErrorV1::Cancelled => LayeredFourFaceChainContinuousErrorV1::Cancelled,
        FourFaceChainIntervalErrorV1::DeadlineExceeded => {
            LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
        }
        FourFaceChainIntervalErrorV1::IntervalUnavailable => {
            LayeredFourFaceChainContinuousErrorV1::PairPartitionUnavailable
        }
        FourFaceChainIntervalErrorV1::IntervalOverlap => {
            LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap
        }
    }
}

fn map_static_error_v1(error: StaticCollisionError) -> LayeredFourFaceChainContinuousErrorV1 {
    match error {
        StaticCollisionError::Cancelled => LayeredFourFaceChainContinuousErrorV1::Cancelled,
        StaticCollisionError::DeadlineExceeded => {
            LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
        }
        StaticCollisionError::ResourceLimitExceeded => {
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        }
        _ => LayeredFourFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable,
    }
}

fn map_boundary_error_v1(
    error: crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1,
) -> LayeredFourFaceChainContinuousErrorV1 {
    match error {
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded => {
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::Cancelled => {
            LayeredFourFaceChainContinuousErrorV1::Cancelled
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::DeadlineExceeded => {
            LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose => {
            LayeredFourFaceChainContinuousErrorV1::MovingBoundaryOnlyUnavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn interval(x: (f64, f64)) -> (ori_domain::VertexId, [OutwardIntervalV1; 3]) {
        (
            ori_domain::VertexId::new(),
            [
                OutwardIntervalV1::new(x.0, x.1).unwrap(),
                OutwardIntervalV1::new(0.0, 0.0).unwrap(),
                OutwardIntervalV1::new(0.0, 0.0).unwrap(),
            ],
        )
    }

    fn chain_v1() -> (Vec<FaceId>, Vec<(EdgeId, [FaceId; 2])>) {
        let faces = (0..4).map(|_| FaceId::new()).collect::<Vec<_>>();
        let hinges = (0..3)
            .map(|index| {
                (
                    EdgeId::new(),
                    canonical_pair_v1(faces[index], faces[index + 1]),
                )
            })
            .collect();
        (faces, hinges)
    }

    #[test]
    fn four_face_chain_partitions_all_six_pairs() {
        let (faces, hinges) = chain_v1();
        let partition = validate_four_face_chain_hinges_v1(&faces, &hinges).unwrap();
        let mut all = partition
            .direct_pairs
            .into_iter()
            .chain(partition.nonadjacent_pairs)
            .collect::<Vec<_>>();
        all.sort_unstable_by_key(pair_key_v1);
        assert_eq!(all.len(), 6);
        assert!(all.windows(2).all(|pairs| pairs[0] != pairs[1]));
    }

    #[test]
    fn four_face_chain_rejects_wrong_count_redundant_hinge_and_non_path_degrees() {
        let (faces, hinges) = chain_v1();
        assert!(validate_four_face_chain_hinges_v1(&faces[..3], &hinges).is_none());
        let mut duplicate = hinges.clone();
        duplicate[2].0 = duplicate[1].0;
        assert!(validate_four_face_chain_hinges_v1(&faces, &duplicate).is_none());
        let redundant_pair = vec![hinges[0], hinges[1], (EdgeId::new(), hinges[1].1)];
        assert!(validate_four_face_chain_hinges_v1(&faces, &redundant_pair).is_none());
        let star = vec![
            (EdgeId::new(), canonical_pair_v1(faces[0], faces[1])),
            (EdgeId::new(), canonical_pair_v1(faces[0], faces[2])),
            (EdgeId::new(), canonical_pair_v1(faces[0], faces[3])),
        ];
        assert!(validate_four_face_chain_hinges_v1(&faces, &star).is_none());
    }

    #[test]
    fn four_face_chain_binds_one_moving_and_two_stationary_hinges() {
        let (_faces, hinges) = chain_v1();
        let source = [
            (hinges[0].0, 0.0),
            (hinges[1].0, 180.0),
            (hinges[2].0, 180.0),
        ];
        let target = [
            (hinges[0].0, 45.0),
            (hinges[1].0, 180.0),
            (hinges[2].0, 180.0),
        ];
        let schedule =
            validate_four_face_chain_schedule_angles_v1(&hinges, &source, &target).unwrap();
        assert_eq!(schedule.iter().filter(|hinge| hinge.moving).count(), 1);
        assert!(
            schedule
                .windows(2)
                .all(|items| pair_key_v1(&items[0].pair) < pair_key_v1(&items[1].pair))
        );
    }

    #[test]
    fn four_face_chain_rejects_malformed_angle_bindings() {
        let (_, hinges) = chain_v1();
        let source = [
            (hinges[0].0, 0.0),
            (hinges[1].0, 180.0),
            (hinges[2].0, 180.0),
        ];
        let mut target = [
            (hinges[0].0, 45.0),
            (hinges[1].0, 180.0),
            (hinges[2].0, 180.0),
        ];
        target[0].1 = 180.0;
        assert!(validate_four_face_chain_schedule_angles_v1(&hinges, &source, &target).is_none());
        target[0].1 = f64::NAN;
        assert!(validate_four_face_chain_schedule_angles_v1(&hinges, &source, &target).is_none());
        let missing = [
            (hinges[0].0, 45.0),
            (hinges[1].0, 180.0),
            (EdgeId::new(), 180.0),
        ];
        assert!(validate_four_face_chain_schedule_angles_v1(&hinges, &source, &missing).is_none());
        let duplicate = [
            (hinges[0].0, 45.0),
            (hinges[1].0, 180.0),
            (hinges[1].0, 180.0),
        ];
        assert!(
            validate_four_face_chain_schedule_angles_v1(&hinges, &source, &duplicate).is_none()
        );
    }

    #[test]
    fn four_face_chain_leaf_count_cannot_exceed_the_global_dyadic_hard_cap() {
        let maximum = super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1;
        assert!(maximum.is_power_of_two());
        let maximum_depth =
            u8::try_from(maximum.ilog2()).expect("the global leaf cap has a bounded depth");
        assert_eq!(leaf_count_v1(maximum_depth, maximum), Some(maximum));
        assert_eq!(
            leaf_count_v1(
                maximum_depth,
                maximum
                    .checked_add(1)
                    .expect("the hard cap has a successor"),
            ),
            None
        );
        assert_eq!(
            leaf_count_v1(
                maximum_depth
                    .checked_add(1)
                    .expect("the bounded depth has a successor"),
                maximum,
            ),
            None
        );
    }

    #[test]
    fn three_nonadjacent_interval_gaps_fail_closed_for_overlap_limits_and_stops() {
        let first = [interval((0.0, 1.0))];
        let second = [interval((2.0, 3.0))];
        let pairs = [
            (&first[..], &second[..]),
            (&first[..], &second[..]),
            (&first[..], &second[..]),
        ];
        assert_eq!(
            verify_four_face_chain_nonadjacent_gaps_with_control_v1(
                &pairs,
                3,
                &CooperativeOperationControlV1::unbounded()
            ),
            Ok(())
        );
        assert_eq!(
            verify_four_face_chain_nonadjacent_gaps_with_control_v1(
                &pairs,
                2,
                &CooperativeOperationControlV1::unbounded()
            ),
            Err(FourFaceChainIntervalErrorV1::ResourceLimit)
        );
        let touching = [interval((1.0, 2.0))];
        let blocked = [
            (&first[..], &touching[..]),
            (&first[..], &second[..]),
            (&first[..], &second[..]),
        ];
        assert_eq!(
            verify_four_face_chain_nonadjacent_gaps_with_control_v1(
                &blocked,
                3,
                &CooperativeOperationControlV1::unbounded()
            ),
            Err(FourFaceChainIntervalErrorV1::IntervalOverlap)
        );
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        assert_eq!(
            verify_four_face_chain_nonadjacent_gaps_with_control_v1(
                &pairs,
                3,
                &CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + std::time::Duration::from_secs(1)
                )
            ),
            Err(FourFaceChainIntervalErrorV1::Cancelled)
        );
        assert_eq!(
            verify_four_face_chain_nonadjacent_gaps_with_control_v1(
                &pairs,
                3,
                &CooperativeOperationControlV1::new(None, Instant::now())
            ),
            Err(FourFaceChainIntervalErrorV1::DeadlineExceeded)
        );
    }
}

#[cfg(test)]
#[path = "layered_four_face_chain/production_tests.rs"]
mod production_tests;
