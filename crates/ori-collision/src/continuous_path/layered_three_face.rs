//! A deliberately narrow layered continuous certificate for one three-face
//! material tree.  It does not widen the ordinary continuous certificates.

use std::sync::Arc;

#[cfg(test)]
use std::{
    cell::{Cell, RefCell},
    sync::atomic::{AtomicBool, Ordering},
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

use crate::{
    CooperativeOperationControlV1, CooperativeOperationStopV1, StaticCollisionError,
    StaticCollisionLimits, diagnose_static_collision_geometry_with_control_v1,
};

use super::{
    NativeStackedFoldInitialSampleLayerAdmissionV1, StackedFoldInitialLayerOrderSourceV1,
    initial_sample_layer_admission_has_issuer_v1,
    initial_sample_layer_admission_matches_snapshot_v1,
    layered_chain_common::{
        LayeredChainIntervalCheckpointPhaseV1, LayeredChainIntervalErrorV1,
        bounded_face_boundaries_v1, canonical_pair_v1,
        layered_continuous_resource_limits_within_hard_caps_v1,
        layered_leaf_count_v1 as leaf_count_v1,
        verify_layered_chain_nonadjacent_registry_gaps_with_checkpoint_v1,
    },
    retain_initial_sample_layer_admission_issuer_v1,
};

#[cfg(test)]
pub(super) use super::layered_chain_common::{
    LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1, MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1,
    MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1, MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1,
};

pub const LAYERED_THREE_FACE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "layered_three_face_dyadic_continuous_certificate_v1";

/// Non-expandable V1 ceilings shared by the narrow layered theorems. Caller
/// limits may only narrow these values.
const MAX_LAYERED_THREE_FACE_INTERVAL_FACES_V1: usize = 3;
const MAX_LAYERED_THREE_FACE_INTERVAL_HINGES_V1: usize = 2;

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum LayeredThreeFaceTestCheckpointPhaseV1 {
    IssueLeaf,
    RevalidationLeaf,
    NonadjacentAxis,
    NonadjacentVertex,
    FinalRevalidation,
}

#[cfg(test)]
enum LayeredThreeFaceTestCheckpointHookV1 {
    Cancel {
        phase: LayeredThreeFaceTestCheckpointPhaseV1,
        signal: Arc<AtomicBool>,
    },
    Deadline {
        phase: LayeredThreeFaceTestCheckpointPhaseV1,
        entered: Arc<AtomicBool>,
    },
}

#[cfg(test)]
thread_local! {
    static LAYERED_THREE_FACE_TEST_CHECKPOINT_HOOK_V1:
        RefCell<Option<LayeredThreeFaceTestCheckpointHookV1>> = const { RefCell::new(None) };
    static LAYERED_THREE_FACE_TEST_FORCED_DEADLINE_V1: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn set_layered_three_face_test_checkpoint_hook_v1(hook: LayeredThreeFaceTestCheckpointHookV1) {
    LAYERED_THREE_FACE_TEST_CHECKPOINT_HOOK_V1.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "a layered checkpoint hook is already armed"
        );
        *slot.borrow_mut() = Some(hook);
    });
}

#[cfg(test)]
fn clear_layered_three_face_test_checkpoint_hook_v1() {
    LAYERED_THREE_FACE_TEST_CHECKPOINT_HOOK_V1.with(|slot| {
        *slot.borrow_mut() = None;
    });
    LAYERED_THREE_FACE_TEST_FORCED_DEADLINE_V1.with(|forced| forced.set(false));
}

#[cfg(test)]
fn run_layered_three_face_test_checkpoint_hook_v1(phase: LayeredThreeFaceTestCheckpointPhaseV1) {
    LAYERED_THREE_FACE_TEST_CHECKPOINT_HOOK_V1.with(|slot| {
        let hook = { slot.borrow_mut().take() };
        let Some(hook) = hook else {
            return;
        };
        match hook {
            LayeredThreeFaceTestCheckpointHookV1::Cancel {
                phase: expected,
                signal,
            } if expected == phase => signal.store(true, Ordering::Release),
            LayeredThreeFaceTestCheckpointHookV1::Deadline {
                phase: expected,
                entered,
            } if expected == phase => {
                entered.store(true, Ordering::Release);
                LAYERED_THREE_FACE_TEST_FORCED_DEADLINE_V1.with(|forced| forced.set(true));
            }
            hook => *slot.borrow_mut() = Some(hook),
        }
    });
}

/// Bounded work limits for the deliberately narrow layered path proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayeredThreeFaceContinuousLimitsV1 {
    /// The complete path is partitioned into `2^dyadic_depth` closed leaves,
    /// additionally capped by [`super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1`].
    pub dyadic_depth: u8,
    pub max_leaves: usize,
    /// Caller-selectable budgets bounded by the layered V1 hard ceilings.
    pub interval_limits: MaterialTreeDyadicIntervalLimitsV1,
    /// Caller-selectable budgets bounded by fixed layered V1 static ceilings.
    pub static_limits: StaticCollisionLimits,
}

impl Default for LayeredThreeFaceContinuousLimitsV1 {
    fn default() -> Self {
        Self {
            dyadic_depth: 0,
            max_leaves: 1,
            interval_limits: MaterialTreeDyadicIntervalLimitsV1::default(),
            static_limits: StaticCollisionLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LayeredThreeFaceContinuousErrorV1 {
    #[error("the layered certificate only supports exactly three faces and two hinges")]
    UnsupportedTree,
    #[error("the path must have one 0-to-(0,180) moving hinge and one stationary 180-degree hinge")]
    InvalidAngleSchedule,
    #[error("the initial sample-layer admission could not be exactly revalidated")]
    InitialLayerAdmissionUnavailable,
    #[error("the moving shared-hinge pair was not proven boundary-only")]
    MovingBoundaryOnlyUnavailable,
    #[error("a nonadjacent pair touches or its outward intervals overlap")]
    NonadjacentIntervalOverlap,
    #[error("the exact three-pair partition is inconsistent")]
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

/// Opaque proof for the exact three-pair partition of one layered path.
///
/// This type deliberately implements neither `Clone` nor serialization.  The
/// retained dyadic registries bind it to the exact issuing model and source
/// pose; callers can only consume it through [`Self::is_for`].
#[derive(Debug)]
pub struct LayeredThreeFaceContinuousCertificateV1 {
    source_pose: MaterialTreePose,
    target_angles: CanonicalHingeAngles,
    moving_hinge: EdgeId,
    stationary_hinge: EdgeId,
    stationary_pair: [FaceId; 2],
    moving_pair: [FaceId; 2],
    nonadjacent_pair: [FaceId; 2],
    admission_issuer: Arc<()>,
    limits: LayeredThreeFaceContinuousLimitsV1,
    leaves: Vec<MaterialTreeDyadicFaceIntervalRegistryV1>,
}

impl LayeredThreeFaceContinuousCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        LAYERED_THREE_FACE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
    }

    #[must_use]
    pub const fn moving_hinge(&self) -> EdgeId {
        self.moving_hinge
    }

    #[must_use]
    pub const fn stationary_hinge(&self) -> EdgeId {
        self.stationary_hinge
    }

    /// Canonical unordered pairs, in theorem order: stationary flat,
    /// moving direct hinge, then nonadjacent interval-separated pair.
    #[must_use]
    pub const fn pair_partition(&self) -> [[FaceId; 2]; 3] {
        [
            self.stationary_pair,
            self.moving_pair,
            self.nonadjacent_pair,
        ]
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// Rechecks issuer identity, the exact initial admission at `t = 0`, the
    /// direct-hinge theorem on `(0, target]`, and every retained dyadic leaf.
    #[must_use]
    pub fn is_for<T>(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_angles: &CanonicalHingeAngles,
        admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
        limits: LayeredThreeFaceContinuousLimitsV1,
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

    /// Controlled revalidation; cancellation and deadline failures never
    /// return a truthy partial result.
    pub fn is_for_with_control_v1<T>(
        &self,
        model: &MaterialTreeKinematicsModel,
        source_pose: &MaterialTreePose,
        target_angles: &CanonicalHingeAngles,
        admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
        limits: LayeredThreeFaceContinuousLimitsV1,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<bool, LayeredThreeFaceContinuousErrorV1>
    where
        T: StackedFoldInitialLayerOrderSourceV1,
    {
        checkpoint_v1(control)?;
        let Some(leaf_count) = leaf_count_v1(limits.dyadic_depth, limits.max_leaves) else {
            return Ok(false);
        };
        if self.limits != limits
            || !layered_continuous_resource_limits_within_hard_caps_v1(
                limits.interval_limits,
                MAX_LAYERED_THREE_FACE_INTERVAL_FACES_V1,
                MAX_LAYERED_THREE_FACE_INTERVAL_HINGES_V1,
                limits.static_limits,
            )
            || !self.source_pose.same_instance(source_pose)
            || self.target_angles != *target_angles
            || !model.owns_pose(source_pose)
            || !initial_sample_layer_admission_has_issuer_v1(&self.admission_issuer, admission)
            || !bounded_face_boundaries_v1(model, limits.interval_limits.max_vertices)
            || self.leaves.len() != leaf_count
            || !matches_three_face_schedule_v1(model, source_pose, target_angles)
                .is_some_and(|partition| partition == self.partition())
        {
            return Ok(false);
        }
        checkpoint_v1(control)?;
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
        checkpoint_v1(control)?;
        let target_pose = match model.solve(source_pose.fixed_face(), target_angles) {
            Ok(pose) => pose,
            Err(_) => return Ok(false),
        };
        if !direct_hinge_boundary_only_open_interval_theorem_v1(
            model,
            source_pose,
            &target_pose,
            self.moving_hinge,
            self.moving_pair,
            control,
        )
        .map_err(map_boundary_error_v1)?
        {
            return Ok(false);
        }
        for (index, registry) in self.leaves.iter().enumerate() {
            checkpoint_v1(control)?;
            #[cfg(test)]
            run_layered_three_face_test_checkpoint_hook_v1(
                LayeredThreeFaceTestCheckpointPhaseV1::RevalidationLeaf,
            );
            checkpoint_v1(control)?;
            if !registry.is_for(
                model,
                source_pose,
                target_angles,
                limits.dyadic_depth,
                index as u64,
            ) {
                return Ok(false);
            }
            if !strictly_separated_registry_pair_with_control_v1(
                registry,
                self.nonadjacent_pair,
                control,
            )? {
                return Ok(false);
            }
        }
        checkpoint_v1(control)?;
        #[cfg(test)]
        run_layered_three_face_test_checkpoint_hook_v1(
            LayeredThreeFaceTestCheckpointPhaseV1::FinalRevalidation,
        );
        checkpoint_v1(control)?;
        Ok(true)
    }

    const fn partition(&self) -> ThreeFacePairPartitionV1 {
        ThreeFacePairPartitionV1 {
            moving_hinge: self.moving_hinge,
            stationary_hinge: self.stationary_hinge,
            stationary_pair: self.stationary_pair,
            moving_pair: self.moving_pair,
            nonadjacent_pair: self.nonadjacent_pair,
        }
    }
}

/// Certifies the complete closed linear path. The exact initial layer
/// admission is the only authority for the moving hinge's `t = 0` flat
/// endpoint. For every `t` in the open interval the two incident material
/// faces lie in distinct non-coplanar planes whose only common line is their
/// authenticated direct hinge; the native exact target check binds that
/// direct-hinge boundary-only theorem to this issued material tree. The
/// unique nonadjacent pair must additionally have a strict outward interval
/// gap on every closed leaf. Touching is never accepted as a gap.
pub fn certify_layered_three_face_continuous_path_with_control_v1<T>(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    limits: LayeredThreeFaceContinuousLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<LayeredThreeFaceContinuousCertificateV1, LayeredThreeFaceContinuousErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    checkpoint_v1(control)?;
    if !model.owns_pose(source_pose) {
        return Err(LayeredThreeFaceContinuousErrorV1::IssuerMismatch);
    }
    if model.face_ids().len() != MAX_LAYERED_THREE_FACE_INTERVAL_FACES_V1
        || model.hinges().len() != MAX_LAYERED_THREE_FACE_INTERVAL_HINGES_V1
    {
        return Err(LayeredThreeFaceContinuousErrorV1::UnsupportedTree);
    }
    let leaf_count = leaf_count_v1(limits.dyadic_depth, limits.max_leaves)
        .ok_or(LayeredThreeFaceContinuousErrorV1::ResourceLimit)?;
    if !layered_continuous_resource_limits_within_hard_caps_v1(
        limits.interval_limits,
        MAX_LAYERED_THREE_FACE_INTERVAL_FACES_V1,
        MAX_LAYERED_THREE_FACE_INTERVAL_HINGES_V1,
        limits.static_limits,
    ) || !bounded_face_boundaries_v1(model, limits.interval_limits.max_vertices)
    {
        return Err(LayeredThreeFaceContinuousErrorV1::ResourceLimit);
    }
    let partition = matches_three_face_schedule_v1(model, source_pose, target_angles)
        .ok_or(LayeredThreeFaceContinuousErrorV1::InvalidAngleSchedule)?;

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
        return Err(LayeredThreeFaceContinuousErrorV1::InitialLayerAdmissionUnavailable);
    }

    let target_pose = model
        .solve(source_pose.fixed_face(), target_angles)
        .map_err(|_| LayeredThreeFaceContinuousErrorV1::IssuerMismatch)?;
    if !direct_hinge_boundary_only_open_interval_theorem_v1(
        model,
        source_pose,
        &target_pose,
        partition.moving_hinge,
        partition.moving_pair,
        control,
    )
    .map_err(map_boundary_error_v1)?
    {
        return Err(LayeredThreeFaceContinuousErrorV1::MovingBoundaryOnlyUnavailable);
    }

    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(leaf_count)
        .map_err(|_| LayeredThreeFaceContinuousErrorV1::ResourceLimit)?;
    for index in 0..leaf_count {
        checkpoint_v1(control)?;
        #[cfg(test)]
        run_layered_three_face_test_checkpoint_hook_v1(
            LayeredThreeFaceTestCheckpointPhaseV1::IssueLeaf,
        );
        checkpoint_v1(control)?;
        let registry = model
            .prepare_dyadic_face_vertex_intervals_with_checkpoint_v1(
                source_pose,
                target_angles,
                limits.dyadic_depth,
                index as u64,
                limits.interval_limits,
                || control.checkpoint().is_ok(),
            )
            .map_err(|error| map_interval_error_v1(error, control))?;
        if !strictly_separated_registry_pair_with_control_v1(
            &registry,
            partition.nonadjacent_pair,
            control,
        )? {
            return Err(LayeredThreeFaceContinuousErrorV1::NonadjacentIntervalOverlap);
        }
        leaves.push(registry);
    }
    checkpoint_v1(control)?;
    Ok(LayeredThreeFaceContinuousCertificateV1 {
        source_pose: source_pose.clone(),
        target_angles: target_angles.clone(),
        moving_hinge: partition.moving_hinge,
        stationary_hinge: partition.stationary_hinge,
        stationary_pair: partition.stationary_pair,
        moving_pair: partition.moving_pair,
        nonadjacent_pair: partition.nonadjacent_pair,
        admission_issuer: retain_initial_sample_layer_admission_issuer_v1(admission),
        limits,
        leaves,
    })
}

pub fn certify_layered_three_face_continuous_path_v1<T>(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<T>,
    limits: LayeredThreeFaceContinuousLimitsV1,
) -> Result<LayeredThreeFaceContinuousCertificateV1, LayeredThreeFaceContinuousErrorV1>
where
    T: StackedFoldInitialLayerOrderSourceV1,
{
    certify_layered_three_face_continuous_path_with_control_v1(
        model,
        source_pose,
        target_angles,
        admission,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThreeFacePairPartitionV1 {
    moving_hinge: EdgeId,
    stationary_hinge: EdgeId,
    stationary_pair: [FaceId; 2],
    moving_pair: [FaceId; 2],
    nonadjacent_pair: [FaceId; 2],
}

fn matches_three_face_schedule_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target: &CanonicalHingeAngles,
) -> Option<ThreeFacePairPartitionV1> {
    if model.face_ids().len() != 3
        || model.hinges().len() != 2
        || source_pose.hinge_angles().len() != 2
        || target.as_slice().len() != 2
    {
        return None;
    }
    let mut moving = None;
    let mut stationary = None;
    for (source, target) in source_pose.hinge_angles().iter().zip(target.as_slice()) {
        if source.edge() != target.edge() {
            return None;
        }
        let source_bits = source.angle_degrees().to_bits();
        let target_bits = target.angle_degrees().to_bits();
        if source_bits == 0.0_f64.to_bits()
            && target.angle_degrees().is_finite()
            && target.angle_degrees() > 0.0
            && target.angle_degrees() < 180.0
        {
            moving = Some(source.edge());
        } else if source_bits == 180.0_f64.to_bits() && target_bits == 180.0_f64.to_bits() {
            stationary = Some(source.edge());
        } else {
            return None;
        }
    }
    let (moving_hinge, stationary_hinge) = moving.zip(stationary)?;
    let moving_tree_hinge = model
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == moving_hinge)?;
    let stationary_tree_hinge = model
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == stationary_hinge)?;
    let moving_pair = canonical_pair_v1(
        moving_tree_hinge.left_face(),
        moving_tree_hinge.right_face(),
    );
    let stationary_pair = canonical_pair_v1(
        stationary_tree_hinge.left_face(),
        stationary_tree_hinge.right_face(),
    );
    if moving_pair == stationary_pair {
        return None;
    }
    let all_faces = model.face_ids();
    let shared = moving_pair
        .into_iter()
        .find(|face| stationary_pair.contains(face))?;
    let moving_outer = moving_pair.into_iter().find(|face| *face != shared)?;
    let stationary_outer = stationary_pair.into_iter().find(|face| *face != shared)?;
    if !all_faces.contains(&shared)
        || !all_faces.contains(&moving_outer)
        || !all_faces.contains(&stationary_outer)
        || moving_outer == stationary_outer
    {
        return None;
    }
    let nonadjacent_pair = canonical_pair_v1(moving_outer, stationary_outer);
    let mut actual = [stationary_pair, moving_pair, nonadjacent_pair];
    actual.sort_unstable_by_key(|pair| (pair[0].canonical_bytes(), pair[1].canonical_bytes()));
    let expected = [
        canonical_pair_v1(all_faces[0], all_faces[1]),
        canonical_pair_v1(all_faces[0], all_faces[2]),
        canonical_pair_v1(all_faces[1], all_faces[2]),
    ];
    if actual != expected {
        return None;
    }
    Some(ThreeFacePairPartitionV1 {
        moving_hinge,
        stationary_hinge,
        stationary_pair,
        moving_pair,
        nonadjacent_pair,
    })
}

/// Native binding point for the direct-hinge open-interval theorem.  The
/// zero-angle endpoint is intentionally excluded: it is covered above only by
/// `NativeStackedFoldInitialSampleLayerAdmissionV1`.  A nonzero target exact
/// boundary classification rejects issuer drift, a different pair, area
/// overlap, and every geometry for which the direct-hinge theorem is not
/// available; the topological direct-hinge premise then applies throughout
/// the connected interval `(0, target]`.
pub(super) fn direct_hinge_boundary_only_open_interval_theorem_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_pose: &MaterialTreePose,
    moving_hinge: EdgeId,
    pair: [FaceId; 2],
    control: &CooperativeOperationControlV1<'_>,
) -> Result<bool, crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1> {
    if !direct_hinge_open_interval_premises_v1(model, source_pose, moving_hinge, pair, control)? {
        return Ok(false);
    }
    let bound = model.bind_pose(target_pose).map_err(|_| {
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose
    })?;
    crate::cayley::diagnose_bound_zero_thickness_shared_hinge_boundaries_with_control_v1(
        bound,
        &[(pair[0], pair[1])],
        control,
    )
    .map(|summary| summary.proves_boundary_contact_pair(pair[0], pair[1]))
}

/// The strict-positive dihedral theorem needs more than a pair label: both
/// faces must contain the direct edge exactly once, share exactly its two
/// endpoint identities, and have no other material vertex on that axis.  In
/// that case their non-coplanar supporting planes meet only in the direct
/// hinge line, while each material face meets that line only in the common
/// segment.  The prepared material topology is therefore boundary-only for
/// every strictly positive angle, not merely for a sampled target pose.
fn direct_hinge_open_interval_premises_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    edge: EdgeId,
    pair: [FaceId; 2],
    control: &CooperativeOperationControlV1<'_>,
) -> Result<bool, crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1> {
    boundary_checkpoint_v1(control)?;
    let mut matches = model.hinges().iter().filter(|hinge| hinge.edge() == edge);
    let Some(hinge) = matches.next() else {
        return Ok(false);
    };
    if matches.next().is_some() || canonical_pair_v1(hinge.left_face(), hinge.right_face()) != pair
    {
        return Ok(false);
    }
    let (Some(left), Some(right)) = (
        model.face_boundary(hinge.left_face()),
        model.face_boundary(hinge.right_face()),
    ) else {
        return Ok(false);
    };
    let endpoints = [hinge.start(), hinge.end()];
    let Some(left_index) = left.edges().iter().position(|candidate| *candidate == edge) else {
        return Ok(false);
    };
    let Some(right_index) = right
        .edges()
        .iter()
        .position(|candidate| *candidate == edge)
    else {
        return Ok(false);
    };
    if left
        .edges()
        .iter()
        .filter(|candidate| **candidate == edge)
        .count()
        != 1
        || right
            .edges()
            .iter()
            .filter(|candidate| **candidate == edge)
            .count()
            != 1
    {
        return Ok(false);
    }
    let left_endpoints = [
        left.vertices()[left_index],
        left.vertices()[(left_index + 1) % left.vertices().len()],
    ];
    let right_endpoints = [
        right.vertices()[right_index],
        right.vertices()[(right_index + 1) % right.vertices().len()],
    ];
    let mut left_endpoints = left_endpoints;
    let mut right_endpoints = right_endpoints;
    left_endpoints.sort_unstable_by_key(|vertex| vertex.canonical_bytes());
    right_endpoints.sort_unstable_by_key(|vertex| vertex.canonical_bytes());
    if left_endpoints != right_endpoints {
        return Ok(false);
    }
    let shared_count = left
        .vertices()
        .iter()
        .filter(|vertex| right.vertices().contains(vertex))
        .count();
    if shared_count != 2
        || !left_endpoints
            .iter()
            .all(|vertex| right.vertices().contains(vertex))
    {
        return Ok(false);
    }
    for vertex in left
        .vertices()
        .iter()
        .chain(right.vertices())
        .filter(|vertex| !left_endpoints.contains(vertex))
    {
        boundary_checkpoint_v1(control)?;
        if !source_pose
            .vertex_position(*vertex)
            .is_some_and(|point| point_is_not_on_axis_v1(point, endpoints))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn boundary_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::DeadlineExceeded
        }
    })
}

fn point_is_not_on_axis_v1(
    point: ori_kinematics::Point3,
    endpoints: [ori_kinematics::Point3; 2],
) -> bool {
    let direction = [
        endpoints[1].x() - endpoints[0].x(),
        endpoints[1].y() - endpoints[0].y(),
        endpoints[1].z() - endpoints[0].z(),
    ];
    let offset = [
        point.x() - endpoints[0].x(),
        point.y() - endpoints[0].y(),
        point.z() - endpoints[0].z(),
    ];
    let cross = [
        direction[1] * offset[2] - direction[2] * offset[1],
        direction[2] * offset[0] - direction[0] * offset[2],
        direction[0] * offset[1] - direction[1] * offset[0],
    ];
    cross
        .into_iter()
        .any(|coordinate| coordinate != 0.0 && coordinate.is_finite())
}

#[cfg(test)]
fn strictly_separated_registry_pair_v1(
    registry: &MaterialTreeDyadicFaceIntervalRegistryV1,
    pair: [FaceId; 2],
) -> bool {
    let (Some(first), Some(second)) = (
        registry.face_vertices(pair[0]),
        registry.face_vertices(pair[1]),
    ) else {
        return false;
    };
    (0..3).any(|axis| strict_axis_gap_v1(first, second, axis))
}

fn strictly_separated_registry_pair_with_control_v1(
    registry: &MaterialTreeDyadicFaceIntervalRegistryV1,
    pair: [FaceId; 2],
    control: &CooperativeOperationControlV1<'_>,
) -> Result<bool, LayeredThreeFaceContinuousErrorV1> {
    match verify_layered_chain_nonadjacent_registry_gaps_with_checkpoint_v1(
        registry,
        &[pair],
        1,
        1,
        |phase| {
            checkpoint_v1(control).map_err(map_three_face_checkpoint_to_common_v1)?;
            match phase {
                LayeredChainIntervalCheckpointPhaseV1::Axis => {
                    #[cfg(test)]
                    run_layered_three_face_test_checkpoint_hook_v1(
                        LayeredThreeFaceTestCheckpointPhaseV1::NonadjacentAxis,
                    );
                }
                LayeredChainIntervalCheckpointPhaseV1::Vertex => {
                    #[cfg(test)]
                    run_layered_three_face_test_checkpoint_hook_v1(
                        LayeredThreeFaceTestCheckpointPhaseV1::NonadjacentVertex,
                    );
                }
                LayeredChainIntervalCheckpointPhaseV1::Pair
                | LayeredChainIntervalCheckpointPhaseV1::Final => {}
            }
            checkpoint_v1(control).map_err(map_three_face_checkpoint_to_common_v1)
        },
    ) {
        Ok(()) => Ok(true),
        Err(
            LayeredChainIntervalErrorV1::IntervalUnavailable
            | LayeredChainIntervalErrorV1::IntervalOverlap,
        ) => Ok(false),
        Err(error) => Err(map_layered_chain_interval_error_v1(error)),
    }
}

fn map_three_face_checkpoint_to_common_v1(
    error: LayeredThreeFaceContinuousErrorV1,
) -> LayeredChainIntervalErrorV1 {
    match error {
        LayeredThreeFaceContinuousErrorV1::Cancelled => LayeredChainIntervalErrorV1::Cancelled,
        LayeredThreeFaceContinuousErrorV1::DeadlineExceeded => {
            LayeredChainIntervalErrorV1::DeadlineExceeded
        }
        _ => LayeredChainIntervalErrorV1::ResourceLimit,
    }
}

fn map_layered_chain_interval_error_v1(
    error: LayeredChainIntervalErrorV1,
) -> LayeredThreeFaceContinuousErrorV1 {
    match error {
        LayeredChainIntervalErrorV1::ResourceLimit => {
            LayeredThreeFaceContinuousErrorV1::ResourceLimit
        }
        LayeredChainIntervalErrorV1::Cancelled => LayeredThreeFaceContinuousErrorV1::Cancelled,
        LayeredChainIntervalErrorV1::DeadlineExceeded => {
            LayeredThreeFaceContinuousErrorV1::DeadlineExceeded
        }
        LayeredChainIntervalErrorV1::IntervalUnavailable => {
            LayeredThreeFaceContinuousErrorV1::PairPartitionUnavailable
        }
        LayeredChainIntervalErrorV1::IntervalOverlap => {
            LayeredThreeFaceContinuousErrorV1::NonadjacentIntervalOverlap
        }
    }
}

#[cfg(test)]
fn strict_axis_gap_v1(
    first: &[(ori_domain::VertexId, [OutwardIntervalV1; 3])],
    second: &[(ori_domain::VertexId, [OutwardIntervalV1; 3])],
    axis: usize,
) -> bool {
    let Some(first_lower) = first
        .iter()
        .map(|(_, point)| point[axis].lower())
        .reduce(f64::min)
    else {
        return false;
    };
    let Some(first_upper) = first
        .iter()
        .map(|(_, point)| point[axis].upper())
        .reduce(f64::max)
    else {
        return false;
    };
    let Some(second_lower) = second
        .iter()
        .map(|(_, point)| point[axis].lower())
        .reduce(f64::min)
    else {
        return false;
    };
    let Some(second_upper) = second
        .iter()
        .map(|(_, point)| point[axis].upper())
        .reduce(f64::max)
    else {
        return false;
    };
    first_upper < second_lower || second_upper < first_lower
}

fn checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), LayeredThreeFaceContinuousErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => LayeredThreeFaceContinuousErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            LayeredThreeFaceContinuousErrorV1::DeadlineExceeded
        }
    })?;
    #[cfg(test)]
    if LAYERED_THREE_FACE_TEST_FORCED_DEADLINE_V1.with(|forced| forced.replace(false)) {
        return Err(LayeredThreeFaceContinuousErrorV1::DeadlineExceeded);
    }
    Ok(())
}

fn map_interval_error_v1(
    error: MaterialTreeDyadicIntervalErrorV1,
    control: &CooperativeOperationControlV1<'_>,
) -> LayeredThreeFaceContinuousErrorV1 {
    match error {
        MaterialTreeDyadicIntervalErrorV1::Cancelled => checkpoint_v1(control)
            .err()
            .unwrap_or(LayeredThreeFaceContinuousErrorV1::Cancelled),
        MaterialTreeDyadicIntervalErrorV1::ResourceLimit => {
            LayeredThreeFaceContinuousErrorV1::ResourceLimit
        }
        MaterialTreeDyadicIntervalErrorV1::SourceIssuerMismatch => {
            LayeredThreeFaceContinuousErrorV1::IssuerMismatch
        }
        MaterialTreeDyadicIntervalErrorV1::InvalidLeaf
        | MaterialTreeDyadicIntervalErrorV1::SourceAnglesMismatch
        | MaterialTreeDyadicIntervalErrorV1::TargetAnglesMismatch
        | MaterialTreeDyadicIntervalErrorV1::IntervalUnavailable => {
            LayeredThreeFaceContinuousErrorV1::PairPartitionUnavailable
        }
    }
}

fn map_static_error_v1(error: StaticCollisionError) -> LayeredThreeFaceContinuousErrorV1 {
    match error {
        StaticCollisionError::Cancelled => LayeredThreeFaceContinuousErrorV1::Cancelled,
        StaticCollisionError::DeadlineExceeded => {
            LayeredThreeFaceContinuousErrorV1::DeadlineExceeded
        }
        StaticCollisionError::ResourceLimitExceeded => {
            LayeredThreeFaceContinuousErrorV1::ResourceLimit
        }
        _ => LayeredThreeFaceContinuousErrorV1::InitialLayerAdmissionUnavailable,
    }
}

fn map_boundary_error_v1(
    error: crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1,
) -> LayeredThreeFaceContinuousErrorV1 {
    match error {
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded => {
            LayeredThreeFaceContinuousErrorV1::ResourceLimit
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::Cancelled => {
            LayeredThreeFaceContinuousErrorV1::Cancelled
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::DeadlineExceeded => {
            LayeredThreeFaceContinuousErrorV1::DeadlineExceeded
        }
        crate::cayley::ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose => {
            LayeredThreeFaceContinuousErrorV1::MovingBoundaryOnlyUnavailable
        }
    }
}

#[cfg(test)]
mod tests;
