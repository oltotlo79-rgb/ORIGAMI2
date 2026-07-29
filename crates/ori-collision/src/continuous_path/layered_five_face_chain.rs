//! A deliberately narrow layered continuous certificate for one five-face
//! material-tree chain. It does not widen the ordinary continuous
//! certificates.

use std::sync::Arc;

use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CanonicalHingeAngles, MaterialTreeDyadicFaceIntervalRegistryV1,
    MaterialTreeDyadicIntervalErrorV1, MaterialTreeDyadicIntervalLimitsV1,
    MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1, StaticCollisionError,
    StaticCollisionLimits, diagnose_static_collision_geometry_with_control_v1,
};

use super::{
    NativeStackedFoldInitialSampleLayerAdmissionV1, StackedFoldInitialLayerOrderSourceV1,
    initial_sample_layer_admission::{
        StationaryFlatStackTransportBindingErrorV1, StationaryFlatStackTransportBindingV1,
        stationary_flat_stack_transport_bindings_bounded_v1,
    },
    initial_sample_layer_admission_has_issuer_v1,
    initial_sample_layer_admission_matches_snapshot_v1,
    layered_chain_common::{
        LayeredChainDirectHingeV1, LayeredChainIntervalErrorV1, LayeredChainResourceErrorV1,
        bounded_face_boundaries_v1, canonical_pair_v1,
        layered_continuous_resource_limits_within_hard_caps_v1, layered_leaf_count_v1,
        validate_linear_chain_hinges_v1, validate_single_moving_flat_chain_schedule_v1,
        verify_layered_chain_nonadjacent_registry_gaps_with_control_v1,
    },
    layered_three_face::direct_hinge_boundary_only_open_interval_theorem_v1,
    retain_initial_sample_layer_admission_issuer_v1,
};

pub const LAYERED_FIVE_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "layered_five_face_chain_dyadic_continuous_certificate_v1";

const FIVE_FACE_COUNT_V1: usize = 5;
const FIVE_HINGE_COUNT_V1: usize = 4;
const FIVE_STATIONARY_HINGE_COUNT_V1: usize = 3;
const FIVE_NONADJACENT_PAIR_COUNT_V1: usize = 6;
const FIVE_ALL_PAIR_COUNT_V1: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayeredFiveFaceChainContinuousLimitsV1 {
    pub dyadic_depth: u8,
    pub max_leaves: usize,
    /// Must equal the complete six-pair nonadjacent partition.
    pub max_nonadjacent_pairs: usize,
    pub interval_limits: MaterialTreeDyadicIntervalLimitsV1,
    pub static_limits: StaticCollisionLimits,
}

impl Default for LayeredFiveFaceChainContinuousLimitsV1 {
    fn default() -> Self {
        Self {
            dyadic_depth: 0,
            max_leaves: 1,
            max_nonadjacent_pairs: FIVE_NONADJACENT_PAIR_COUNT_V1,
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_faces: FIVE_FACE_COUNT_V1,
                max_hinges: FIVE_HINGE_COUNT_V1,
                ..MaterialTreeDyadicIntervalLimitsV1::default()
            },
            static_limits: StaticCollisionLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LayeredFiveFaceChainContinuousErrorV1 {
    #[error("the layered certificate only supports exactly five faces and four chain hinges")]
    UnsupportedTree,
    #[error(
        "the path must have one 0-to-(0,180) moving hinge and three stationary 180-degree hinges"
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
    #[error("the exact ten-pair partition is inconsistent")]
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
struct FiveFaceChainPairPartitionV1 {
    direct_pairs: [[FaceId; 2]; FIVE_HINGE_COUNT_V1],
    nonadjacent_pairs: [[FaceId; 2]; FIVE_NONADJACENT_PAIR_COUNT_V1],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StationaryFlatPairTransportV1 {
    binding: StationaryFlatStackTransportBindingV1,
    source_angle_bits: u64,
    target_angle_bits: u64,
    dyadic_depth: u8,
    leaf_count: usize,
}

/// Opaque proof for all ten pairs of one exact five-face linear chain.
///
/// It intentionally implements neither `Clone` nor serialization and never
/// authorizes mutation. Revalidation binds the original pose instance,
/// schedule, admission issuer, exact limits, three directed stationary
/// transports, and every retained outward-interval registry.
#[derive(Debug)]
pub struct LayeredFiveFaceChainContinuousCertificateV1 {
    source_pose: MaterialTreePose,
    target_angles: CanonicalHingeAngles,
    direct_hinges: [LayeredChainDirectHingeV1; FIVE_HINGE_COUNT_V1],
    pair_partition: FiveFaceChainPairPartitionV1,
    stationary_transports: [StationaryFlatPairTransportV1; FIVE_STATIONARY_HINGE_COUNT_V1],
    admission_issuer: Arc<()>,
    limits: LayeredFiveFaceChainContinuousLimitsV1,
    leaves: Vec<MaterialTreeDyadicFaceIntervalRegistryV1>,
}

impl LayeredFiveFaceChainContinuousCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        LAYERED_FIVE_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
    }

    #[must_use]
    pub fn moving_hinge(&self) -> EdgeId {
        self.direct_hinges
            .iter()
            .find(|hinge| hinge.moving)
            .expect("issued certificate has exactly one moving hinge")
            .edge
    }

    #[must_use]
    pub const fn pair_partition(&self) -> [[FaceId; 2]; FIVE_ALL_PAIR_COUNT_V1] {
        [
            self.pair_partition.direct_pairs[0],
            self.pair_partition.direct_pairs[1],
            self.pair_partition.direct_pairs[2],
            self.pair_partition.direct_pairs[3],
            self.pair_partition.nonadjacent_pairs[0],
            self.pair_partition.nonadjacent_pairs[1],
            self.pair_partition.nonadjacent_pairs[2],
            self.pair_partition.nonadjacent_pairs[3],
            self.pair_partition.nonadjacent_pairs[4],
            self.pair_partition.nonadjacent_pairs[5],
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
        limits: LayeredFiveFaceChainContinuousLimitsV1,
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

    pub fn is_for_with_control_v1<T>(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_angles: &CanonicalHingeAngles,
        admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
        limits: LayeredFiveFaceChainContinuousLimitsV1,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<bool, LayeredFiveFaceChainContinuousErrorV1>
    where
        T: StackedFoldInitialLayerOrderSourceV1,
    {
        certificate_checkpoint_v1(control)?;
        let Some(leaf_count) = layered_leaf_count_v1(limits.dyadic_depth, limits.max_leaves) else {
            return Ok(false);
        };
        if self.limits != limits
            || !valid_limits_v1(model, limits)
            || !self.source_pose.same_instance(source_pose)
            || self.target_angles != *target_angles
            || !model.owns_pose(source_pose)
            || !initial_sample_layer_admission_has_issuer_v1(&self.admission_issuer, admission)
            || self.leaves.len() != leaf_count
        {
            return Ok(false);
        }
        let Some((partition, direct_hinges)) =
            matches_five_face_chain_schedule_v1(model, source_pose, target_angles)
                .map_err(map_resource_error_v1)?
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
        let Some(moving_hinge) = direct_hinges.iter().find(|hinge| hinge.moving) else {
            return Ok(false);
        };
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
            ) || !registry.is_for(
                model,
                source_pose,
                target_angles,
                limits.dyadic_depth,
                index as u64,
            ) {
                return Ok(false);
            }
            verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
                registry,
                &self.pair_partition.nonadjacent_pairs,
                FIVE_NONADJACENT_PAIR_COUNT_V1,
                limits.max_nonadjacent_pairs,
                control,
            )
            .map_err(map_interval_error_v1)?;
        }
        certificate_checkpoint_v1(control)?;
        Ok(true)
    }
}

pub fn certify_layered_five_face_chain_continuous_path_with_control_v1<T>(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    limits: LayeredFiveFaceChainContinuousLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<LayeredFiveFaceChainContinuousCertificateV1, LayeredFiveFaceChainContinuousErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    certificate_checkpoint_v1(control)?;
    if !model.owns_pose(source_pose) {
        return Err(LayeredFiveFaceChainContinuousErrorV1::IssuerMismatch);
    }
    if model.face_ids().len() != FIVE_FACE_COUNT_V1 || model.hinges().len() != FIVE_HINGE_COUNT_V1 {
        return Err(LayeredFiveFaceChainContinuousErrorV1::UnsupportedTree);
    }
    let leaf_count = layered_leaf_count_v1(limits.dyadic_depth, limits.max_leaves)
        .ok_or(LayeredFiveFaceChainContinuousErrorV1::ResourceLimit)?;
    if !valid_limits_v1(model, limits) {
        return Err(LayeredFiveFaceChainContinuousErrorV1::ResourceLimit);
    }
    let (pair_partition, direct_hinges) =
        matches_five_face_chain_schedule_v1(model, source_pose, target_angles)
            .map_err(map_resource_error_v1)?
            .ok_or(LayeredFiveFaceChainContinuousErrorV1::InvalidAngleSchedule)?;

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
        return Err(LayeredFiveFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable);
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
        .ok_or(LayeredFiveFaceChainContinuousErrorV1::StationaryLayerTransportUnavailable)?;

    certificate_checkpoint_v1(control)?;
    let target_pose = model
        .solve(source_pose.fixed_face(), target_angles)
        .map_err(|_| LayeredFiveFaceChainContinuousErrorV1::IssuerMismatch)?;
    let moving_hinge = direct_hinges
        .iter()
        .find(|hinge| hinge.moving)
        .ok_or(LayeredFiveFaceChainContinuousErrorV1::InvalidAngleSchedule)?;
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
        return Err(LayeredFiveFaceChainContinuousErrorV1::MovingBoundaryOnlyUnavailable);
    }

    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(leaf_count)
        .map_err(|_| LayeredFiveFaceChainContinuousErrorV1::ResourceLimit)?;
    for index in 0..leaf_count {
        certificate_checkpoint_v1(control)?;
        if !stationary_transports_cover_leaf_v1(&stationary_transports, limits.dyadic_depth, index)
        {
            return Err(LayeredFiveFaceChainContinuousErrorV1::StationaryLayerTransportUnavailable);
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
        verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &pair_partition.nonadjacent_pairs,
            FIVE_NONADJACENT_PAIR_COUNT_V1,
            limits.max_nonadjacent_pairs,
            control,
        )
        .map_err(map_interval_error_v1)?;
        leaves.push(registry);
    }
    certificate_checkpoint_v1(control)?;
    Ok(LayeredFiveFaceChainContinuousCertificateV1 {
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

pub fn certify_layered_five_face_chain_continuous_path_v1<T>(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    limits: LayeredFiveFaceChainContinuousLimitsV1,
) -> Result<LayeredFiveFaceChainContinuousCertificateV1, LayeredFiveFaceChainContinuousErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    certify_layered_five_face_chain_continuous_path_with_control_v1(
        model,
        source_pose,
        target_angles,
        admission,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
}

fn valid_limits_v1(
    model: &MaterialTreeKinematicsModel,
    limits: LayeredFiveFaceChainContinuousLimitsV1,
) -> bool {
    limits.max_nonadjacent_pairs == FIVE_NONADJACENT_PAIR_COUNT_V1
        && layered_continuous_resource_limits_within_hard_caps_v1(
            limits.interval_limits,
            FIVE_FACE_COUNT_V1,
            FIVE_HINGE_COUNT_V1,
            limits.static_limits,
        )
        && bounded_face_boundaries_v1(model, limits.interval_limits.max_vertices)
}

fn matches_five_face_chain_schedule_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target: &CanonicalHingeAngles,
) -> Result<
    Option<(
        FiveFaceChainPairPartitionV1,
        [LayeredChainDirectHingeV1; FIVE_HINGE_COUNT_V1],
    )>,
    LayeredChainResourceErrorV1,
> {
    if model.face_ids().len() != FIVE_FACE_COUNT_V1
        || model.hinges().len() != FIVE_HINGE_COUNT_V1
        || source_pose.hinge_angles().len() != FIVE_HINGE_COUNT_V1
        || target.as_slice().len() != FIVE_HINGE_COUNT_V1
    {
        return Ok(None);
    }
    let direct_hinges: [(EdgeId, [FaceId; 2]); FIVE_HINGE_COUNT_V1] =
        std::array::from_fn(|index| {
            let hinge = &model.hinges()[index];
            (
                hinge.edge(),
                canonical_pair_v1(hinge.left_face(), hinge.right_face()),
            )
        });
    let Some(partition) = validate_linear_chain_hinges_v1(
        model.face_ids(),
        &direct_hinges,
        FIVE_FACE_COUNT_V1,
        FIVE_ALL_PAIR_COUNT_V1,
    )?
    else {
        return Ok(None);
    };
    let source: [(EdgeId, f64); FIVE_HINGE_COUNT_V1] = std::array::from_fn(|index| {
        let angle = source_pose.hinge_angles()[index];
        (angle.edge(), angle.angle_degrees())
    });
    let target: [(EdgeId, f64); FIVE_HINGE_COUNT_V1] = std::array::from_fn(|index| {
        let angle = target.as_slice()[index];
        (angle.edge(), angle.angle_degrees())
    });
    let Some(schedule) = validate_single_moving_flat_chain_schedule_v1(
        &direct_hinges,
        &source,
        &target,
        FIVE_HINGE_COUNT_V1,
        FIVE_HINGE_COUNT_V1,
    )?
    else {
        return Ok(None);
    };
    let direct_pairs: [[FaceId; 2]; FIVE_HINGE_COUNT_V1] = match partition.direct_pairs.try_into() {
        Ok(pairs) => pairs,
        Err(_) => return Ok(None),
    };
    let nonadjacent_pairs: [[FaceId; 2]; FIVE_NONADJACENT_PAIR_COUNT_V1] =
        match partition.nonadjacent_pairs.try_into() {
            Ok(pairs) => pairs,
            Err(_) => return Ok(None),
        };
    let schedule: [LayeredChainDirectHingeV1; FIVE_HINGE_COUNT_V1] = match schedule.try_into() {
        Ok(schedule) => schedule,
        Err(_) => return Ok(None),
    };
    if schedule.map(|hinge| hinge.pair) != direct_pairs {
        return Ok(None);
    }
    Ok(Some((
        FiveFaceChainPairPartitionV1 {
            direct_pairs,
            nonadjacent_pairs,
        },
        schedule,
    )))
}

struct StationaryFlatPairTransportInputV1<'a, 'control, T> {
    model: &'a MaterialTreeKinematicsModel,
    source_pose: &'a MaterialTreePose,
    target_angles: &'a CanonicalHingeAngles,
    admission: &'a NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    direct_hinges: &'a [LayeredChainDirectHingeV1; FIVE_HINGE_COUNT_V1],
    dyadic_depth: u8,
    leaf_count: usize,
    control: &'a CooperativeOperationControlV1<'control>,
}

fn bind_stationary_flat_pair_transports_v1<T>(
    input: StationaryFlatPairTransportInputV1<'_, '_, T>,
) -> Result<
    Option<[StationaryFlatPairTransportV1; FIVE_STATIONARY_HINGE_COUNT_V1]>,
    LayeredFiveFaceChainContinuousErrorV1,
> {
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
    if !model.owns_pose(source_pose)
        || layered_leaf_count_v1(dyadic_depth, leaf_count) != Some(leaf_count)
    {
        return Ok(None);
    }
    let mut stationary = direct_hinges.iter().filter(|hinge| !hinge.moving);
    let (Some(first), Some(second), Some(third)) =
        (stationary.next(), stationary.next(), stationary.next())
    else {
        return Ok(None);
    };
    if stationary.next().is_some() {
        return Ok(None);
    }
    let hinges = [*first, *second, *third];
    let expected = hinges.map(|hinge| (hinge.edge, hinge.pair));
    let Some(bindings) = stationary_flat_stack_transport_bindings_bounded_v1(
        admission,
        model,
        source_pose,
        &expected,
        FIVE_STATIONARY_HINGE_COUNT_V1,
        control,
    )
    .map_err(map_transport_binding_error_v1)?
    else {
        return Ok(None);
    };
    let Ok(bindings): Result<
        [StationaryFlatStackTransportBindingV1; FIVE_STATIONARY_HINGE_COUNT_V1],
        _,
    > = bindings.try_into() else {
        return Ok(None);
    };

    let bind = |hinge: LayeredChainDirectHingeV1,
                binding: StationaryFlatStackTransportBindingV1|
     -> Option<StationaryFlatPairTransportV1> {
        let mut matches = model
            .hinges()
            .iter()
            .filter(|candidate| candidate.edge() == hinge.edge);
        let tree_hinge = matches.next()?;
        if matches.next().is_some()
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
    let (Some(first), Some(second), Some(third)) = (
        bind(hinges[0], bindings[0]),
        bind(hinges[1], bindings[1]),
        bind(hinges[2], bindings[2]),
    ) else {
        return Ok(None);
    };
    certificate_checkpoint_v1(control)?;
    Ok(Some([first, second, third]))
}

fn angle_bits_v1(angles: &[ori_kinematics::HingeAngle], edge: EdgeId) -> Option<u64> {
    let mut matches = angles.iter().filter(|angle| angle.edge() == edge);
    let bits = matches.next()?.angle_degrees().to_bits();
    matches.next().is_none().then_some(bits)
}

fn stationary_transports_cover_leaf_v1(
    transports: &[StationaryFlatPairTransportV1; FIVE_STATIONARY_HINGE_COUNT_V1],
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
) -> Result<(), LayeredFiveFaceChainContinuousErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => LayeredFiveFaceChainContinuousErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded
        }
    })
}

fn map_resource_error_v1(_: LayeredChainResourceErrorV1) -> LayeredFiveFaceChainContinuousErrorV1 {
    LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
}

fn map_transport_binding_error_v1(
    error: StationaryFlatStackTransportBindingErrorV1,
) -> LayeredFiveFaceChainContinuousErrorV1 {
    match error {
        StationaryFlatStackTransportBindingErrorV1::ResourceLimit => {
            LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
        }
        StationaryFlatStackTransportBindingErrorV1::Cancelled => {
            LayeredFiveFaceChainContinuousErrorV1::Cancelled
        }
        StationaryFlatStackTransportBindingErrorV1::DeadlineExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded
        }
    }
}

fn map_interval_error_v1(
    error: LayeredChainIntervalErrorV1,
) -> LayeredFiveFaceChainContinuousErrorV1 {
    match error {
        LayeredChainIntervalErrorV1::ResourceLimit => {
            LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
        }
        LayeredChainIntervalErrorV1::Cancelled => LayeredFiveFaceChainContinuousErrorV1::Cancelled,
        LayeredChainIntervalErrorV1::DeadlineExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded
        }
        LayeredChainIntervalErrorV1::IntervalUnavailable => {
            LayeredFiveFaceChainContinuousErrorV1::PairPartitionUnavailable
        }
        LayeredChainIntervalErrorV1::IntervalOverlap => {
            LayeredFiveFaceChainContinuousErrorV1::NonadjacentIntervalOverlap
        }
    }
}

fn map_dyadic_interval_error_v1(
    error: MaterialTreeDyadicIntervalErrorV1,
    control: &CooperativeOperationControlV1<'_>,
) -> LayeredFiveFaceChainContinuousErrorV1 {
    match error {
        MaterialTreeDyadicIntervalErrorV1::Cancelled => certificate_checkpoint_v1(control)
            .err()
            .unwrap_or(LayeredFiveFaceChainContinuousErrorV1::Cancelled),
        MaterialTreeDyadicIntervalErrorV1::ResourceLimit => {
            LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
        }
        MaterialTreeDyadicIntervalErrorV1::SourceIssuerMismatch => {
            LayeredFiveFaceChainContinuousErrorV1::IssuerMismatch
        }
        MaterialTreeDyadicIntervalErrorV1::InvalidLeaf
        | MaterialTreeDyadicIntervalErrorV1::SourceAnglesMismatch
        | MaterialTreeDyadicIntervalErrorV1::TargetAnglesMismatch
        | MaterialTreeDyadicIntervalErrorV1::IntervalUnavailable => {
            LayeredFiveFaceChainContinuousErrorV1::PairPartitionUnavailable
        }
    }
}

fn map_static_error_v1(error: StaticCollisionError) -> LayeredFiveFaceChainContinuousErrorV1 {
    match error {
        StaticCollisionError::Cancelled => LayeredFiveFaceChainContinuousErrorV1::Cancelled,
        StaticCollisionError::DeadlineExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded
        }
        StaticCollisionError::ResourceLimitExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
        }
        _ => LayeredFiveFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable,
    }
}

fn map_boundary_error_v1(
    error: crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1,
) -> LayeredFiveFaceChainContinuousErrorV1 {
    match error {
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::Cancelled => {
            LayeredFiveFaceChainContinuousErrorV1::Cancelled
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::DeadlineExceeded => {
            LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose => {
            LayeredFiveFaceChainContinuousErrorV1::MovingBoundaryOnlyUnavailable
        }
    }
}

#[cfg(test)]
#[path = "layered_five_face_chain/production_tests.rs"]
mod production_tests;
