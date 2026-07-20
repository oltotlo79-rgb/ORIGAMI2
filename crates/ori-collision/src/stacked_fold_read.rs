//! Authenticated, observation-only preparation for one linear stacked-fold
//! request.
//!
//! This is deliberately below the SIM-010 mutation boundary. It joins one
//! project/revision/generation binding, one exact native material-pose
//! instance, one exact current `LayerOrderSnapshot` object, and one normalized
//! world-space line. The resulting proposal only enumerates flat overlap
//! cells and their complete bottom-to-top face records. It does not prove a
//! collision-free path, invert the line into material faces, assign
//! mountain/valley creases, or authorize `ApplyStackedFold`.

use std::sync::Arc;

use num_rational::BigRational;
use num_traits::{Signed, Zero};
use ori_domain::{FaceId, ProjectId};
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{MaterialTreeKinematicsModel, MaterialTreePose, Point3};
use thiserror::Error;

use crate::flat_endpoint_layer_order::{
    CellGeometry, FlatEndpointCellKeyV1, FlatEndpointLayerOrderAnchorErrorV1,
    FlatEndpointLayerOrderInputV1, FlatEndpointLayerOrderLimitsV1, FlatEndpointLayerOrderWorkV1,
    NativeFlatEndpointLayerOrderAnchorV1, RationalPoint, anchor_flat_endpoint_layer_order_v1,
    revalidate_flat_endpoint_layer_order_anchor_v1,
};

pub const STACKED_FOLD_READ_GUARD_MODEL_ID_V1: &str = "native_flat_stacked_fold_read_guard_v1";
pub const STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1: &str =
    "native_linear_stacked_fold_read_proposal_v1";

/// The native project-side identities that a future fixed-lock-order desktop
/// capture must read while both the pose and layer-order capabilities are
/// current.
///
/// Values alone are not mutation authority. A guard additionally seals the
/// exact pose, snapshot, and capture identity, and a future desktop boundary
/// must still compare these values with its live private slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackedFoldReadBindingV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: u64,
    pose_generation: u64,
    layer_order_generation: u64,
}

impl StackedFoldReadBindingV1 {
    #[must_use]
    pub const fn new(
        project_instance_id: ProjectId,
        project_id: ProjectId,
        source_revision: u64,
        pose_generation: u64,
        layer_order_generation: u64,
    ) -> Self {
        Self {
            project_instance_id,
            project_id,
            source_revision,
            pose_generation,
            layer_order_generation,
        }
    }

    #[must_use]
    pub const fn project_instance_id(self) -> ProjectId {
        self.project_instance_id
    }

    #[must_use]
    pub const fn project_id(self) -> ProjectId {
        self.project_id
    }

    #[must_use]
    pub const fn source_revision(self) -> u64 {
        self.source_revision
    }

    #[must_use]
    pub const fn pose_generation(self) -> u64 {
        self.pose_generation
    }

    #[must_use]
    pub const fn layer_order_generation(self) -> u64 {
        self.layer_order_generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldReadSupportV1 {
    NoHingeSingleFace,
    BitExactFlatEndpointTree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldFixedSideV1 {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldRotationDirectionV1 {
    Positive,
    Negative,
}

/// A normalized request for one oriented infinite line in the current world
/// plane.
///
/// The two points define line orientation only; intersected finite segments
/// are derived from every certified cell. `requested_angle_degrees` is merely
/// retained for later stages and is not executed or certified here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedFoldLinearCandidateV1 {
    first: Point3,
    second: Point3,
    fixed_side: StackedFoldFixedSideV1,
    rotation_direction: StackedFoldRotationDirectionV1,
    requested_angle_degrees: f64,
}

impl StackedFoldLinearCandidateV1 {
    pub fn new(
        first: Point3,
        second: Point3,
        fixed_side: StackedFoldFixedSideV1,
        rotation_direction: StackedFoldRotationDirectionV1,
        requested_angle_degrees: f64,
    ) -> Result<Self, StackedFoldReadErrorV1> {
        if first.y().to_bits() != 0.0_f64.to_bits()
            || second.y().to_bits() != 0.0_f64.to_bits()
            || (first.x().to_bits() == second.x().to_bits()
                && first.z().to_bits() == second.z().to_bits())
            || !requested_angle_degrees.is_finite()
            || requested_angle_degrees <= 0.0
            || requested_angle_degrees > 180.0
            || requested_angle_degrees.to_bits() == (-0.0_f64).to_bits()
        {
            return Err(StackedFoldReadErrorV1::InvalidLinearCandidate);
        }
        Ok(Self {
            first,
            second,
            fixed_side,
            rotation_direction,
            requested_angle_degrees,
        })
    }

    #[must_use]
    pub const fn first(self) -> Point3 {
        self.first
    }

    #[must_use]
    pub const fn second(self) -> Point3 {
        self.second
    }

    #[must_use]
    pub const fn fixed_side(self) -> StackedFoldFixedSideV1 {
        self.fixed_side
    }

    #[must_use]
    pub const fn rotation_direction(self) -> StackedFoldRotationDirectionV1 {
        self.rotation_direction
    }

    #[must_use]
    pub const fn requested_angle_degrees(self) -> f64 {
        self.requested_angle_degrees
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackedFoldReadLimitsV1 {
    pub flat_endpoint: FlatEndpointLayerOrderLimitsV1,
    pub max_scanned_cells: usize,
    pub max_total_boundary_vertices: usize,
    pub max_total_layer_records: usize,
    pub max_orientation_tests: usize,
    pub max_exact_arithmetic_operations: usize,
    pub max_exact_integer_bits: usize,
    pub max_total_exact_integer_bits: usize,
    pub max_retained_cells: usize,
    pub max_retained_target_faces: usize,
}

impl Default for StackedFoldReadLimitsV1 {
    fn default() -> Self {
        Self {
            flat_endpoint: FlatEndpointLayerOrderLimitsV1::default(),
            max_scanned_cells: 100_000,
            max_total_boundary_vertices: 500_000,
            max_total_layer_records: 1_000_000,
            max_orientation_tests: 500_000,
            max_exact_arithmetic_operations: 3_500_000,
            max_exact_integer_bits: 262_144,
            max_total_exact_integer_bits: 512 * 1024 * 1024,
            max_retained_cells: 100_000,
            max_retained_target_faces: 10_001,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldReadResourceV1 {
    ScannedCells,
    TotalBoundaryVertices,
    TotalLayerRecords,
    OrientationTests,
    ExactArithmeticOperations,
    ExactIntegerBits,
    TotalExactIntegerBits,
    RetainedCells,
    RetainedTargetFaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StackedFoldReadWorkV1 {
    pub scanned_cells: usize,
    pub total_boundary_vertices: usize,
    pub total_layer_records: usize,
    pub orientation_tests: usize,
    pub exact_arithmetic_operations: usize,
    pub maximum_exact_integer_bits: usize,
    pub total_exact_integer_bits: usize,
    pub retained_cells: usize,
    pub retained_target_faces: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackedFoldReadFailureClassV1 {
    Unsupported,
    Indeterminate,
}

/// Every failure is blocking. No variant means that the line is safe to fold.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StackedFoldReadErrorV1 {
    #[error("the project or authority-generation binding is invalid")]
    InvalidProjectBinding,
    #[error("the project, pose, layer-order, or capture identity is stale or foreign")]
    AuthorityBindingMismatch,
    #[error("the current pose class is unsupported ({faces} faces, {hinges} hinges)")]
    UnsupportedPoseClass { faces: usize, hinges: usize },
    #[error("the current pose is not at the bit-exact flat endpoint")]
    UnsupportedNonFlatEndpoint,
    #[error("the current flat layer order is indeterminate: {0}")]
    LayerOrderIndeterminate(FlatEndpointLayerOrderAnchorErrorV1),
    #[error("the linear fold request is invalid or is outside the certified world plane")]
    InvalidLinearCandidate,
    #[error("the line is tangent to or coincident with a certified cell boundary")]
    AmbiguousCellBoundary,
    #[error("the line does not strictly cross any certified layer cell")]
    NoCrossedLayerCell,
    #[error("the sealed guard/proposal data failed an internal consistency check")]
    InternalIndeterminate,
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimitExceeded {
        resource: StackedFoldReadResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("stacked-fold read-only resource counting overflowed")]
    ResourceCountOverflow,
    #[error("stacked-fold read-only output allocation failed")]
    AllocationFailed,
    #[error("immutable stacked-fold proposal revalidation failed")]
    ProposalReverificationFailed,
}

impl StackedFoldReadErrorV1 {
    #[must_use]
    pub const fn failure_class(&self) -> StackedFoldReadFailureClassV1 {
        match self {
            Self::UnsupportedPoseClass { .. } | Self::UnsupportedNonFlatEndpoint => {
                StackedFoldReadFailureClassV1::Unsupported
            }
            _ => StackedFoldReadFailureClassV1::Indeterminate,
        }
    }
}

#[derive(Debug)]
struct StackedFoldReadGuardProofV1 {
    binding: StackedFoldReadBindingV1,
    support: StackedFoldReadSupportV1,
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
}

/// Opaque in-process binding of one project generation, native pose, and exact
/// flat layer-order snapshot.
///
/// Cloning preserves guard identity. Re-capturing equal values creates a new
/// identity and cannot revalidate a proposal from the earlier capture.
///
/// This type does not implement serialization.
///
/// ```compile_fail
/// use ori_collision::NativeStackedFoldReadGuardV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<NativeStackedFoldReadGuardV1<'static>>();
/// ```
#[derive(Debug, Clone)]
pub struct NativeStackedFoldReadGuardV1<'snapshot> {
    proof: Arc<StackedFoldReadGuardProofV1>,
    anchor: NativeFlatEndpointLayerOrderAnchorV1<'snapshot>,
}

impl NativeStackedFoldReadGuardV1<'_> {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        STACKED_FOLD_READ_GUARD_MODEL_ID_V1
    }

    #[must_use]
    pub fn binding(&self) -> StackedFoldReadBindingV1 {
        self.proof.binding
    }

    #[must_use]
    pub fn support(&self) -> StackedFoldReadSupportV1 {
        self.proof.support
    }

    #[must_use]
    pub fn layer_order_work(&self) -> FlatEndpointLayerOrderWorkV1 {
        self.anchor.work()
    }

    #[must_use]
    pub fn same_guard(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.proof, &other.proof) && self.anchor.same_anchor(&other.anchor)
    }

    #[must_use]
    pub fn is_for_authorities(
        &self,
        binding: StackedFoldReadBindingV1,
        model: &MaterialTreeKinematicsModel,
        pose: &MaterialTreePose,
        snapshot: &LayerOrderSnapshot,
    ) -> bool {
        self.proof.binding == binding
            && self.proof.model == *model
            && self.proof.pose.same_instance(pose)
            && self.anchor.is_for_authorities(model, pose, snapshot)
    }

    /// This guard is observation-only and never carries project-mutation
    /// authority.
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// This guard cannot authorize `ApplyStackedFold`.
    #[must_use]
    pub const fn authorizes_apply_stacked_fold(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackedFoldReadCellV1 {
    cell_key: FlatEndpointCellKeyV1,
    bottom_to_top_faces: Vec<FaceId>,
}

impl StackedFoldReadCellV1 {
    #[must_use]
    pub const fn cell_key(&self) -> FlatEndpointCellKeyV1 {
        self.cell_key
    }

    #[must_use]
    pub fn bottom_to_top_faces(&self) -> &[FaceId] {
        &self.bottom_to_top_faces
    }
}

#[derive(Debug)]
struct StackedFoldReadProposalProofV1<'snapshot> {
    guard: Arc<StackedFoldReadGuardProofV1>,
    anchor: NativeFlatEndpointLayerOrderAnchorV1<'snapshot>,
    candidate: StackedFoldLinearCandidateV1,
    crossed_cells: Vec<StackedFoldReadCellV1>,
    target_faces: Vec<FaceId>,
    work: StackedFoldReadWorkV1,
}

/// Opaque read-only proposal for the cells and layers crossed by one line.
///
/// The proposal does not implement serialization and cannot be reconstructed
/// from its observation getters.
///
/// ```compile_fail
/// use ori_collision::NativeStackedFoldReadProposalV1;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<NativeStackedFoldReadProposalV1<'static>>();
/// ```
#[derive(Debug, Clone)]
pub struct NativeStackedFoldReadProposalV1<'snapshot> {
    proof: Arc<StackedFoldReadProposalProofV1<'snapshot>>,
}

impl NativeStackedFoldReadProposalV1<'_> {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1
    }

    #[must_use]
    pub fn binding(&self) -> StackedFoldReadBindingV1 {
        self.proof.guard.binding
    }

    #[must_use]
    pub fn support(&self) -> StackedFoldReadSupportV1 {
        self.proof.guard.support
    }

    #[must_use]
    pub fn candidate(&self) -> StackedFoldLinearCandidateV1 {
        self.proof.candidate
    }

    #[must_use]
    pub fn crossed_cells(&self) -> &[StackedFoldReadCellV1] {
        &self.proof.crossed_cells
    }

    #[must_use]
    pub fn target_faces(&self) -> &[FaceId] {
        &self.proof.target_faces
    }

    #[must_use]
    pub fn work(&self) -> StackedFoldReadWorkV1 {
        self.proof.work
    }

    #[must_use]
    pub fn same_proposal(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.proof, &other.proof)
    }

    #[must_use]
    pub fn is_for_guard(&self, guard: &NativeStackedFoldReadGuardV1<'_>) -> bool {
        Arc::ptr_eq(&self.proof.guard, &guard.proof) && self.proof.anchor.same_anchor(&guard.anchor)
    }

    /// A read proposal is not a collision or continuous-motion certificate.
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// No `ApplyStackedFold` one-shot authority is issued by this module.
    #[must_use]
    pub const fn authorizes_apply_stacked_fold(&self) -> bool {
        false
    }
}

struct ProposalAnalysis {
    crossed_cells: Vec<StackedFoldReadCellV1>,
    target_faces: Vec<FaceId>,
    work: StackedFoldReadWorkV1,
}

pub fn capture_stacked_fold_read_guard_v1<'snapshot>(
    binding: StackedFoldReadBindingV1,
    input: FlatEndpointLayerOrderInputV1<'_, 'snapshot>,
    limits: StackedFoldReadLimitsV1,
) -> Result<NativeStackedFoldReadGuardV1<'snapshot>, StackedFoldReadErrorV1> {
    validate_project_binding(binding, input)?;
    let anchor = anchor_flat_endpoint_layer_order_v1(input, limits.flat_endpoint)
        .map_err(map_anchor_error)?;
    if anchor.identity_namespace() != binding.project_id
        || anchor.source_revision() != binding.source_revision
        || !anchor.is_for_authorities(input.model, input.pose, input.layer_order)
    {
        return Err(StackedFoldReadErrorV1::AuthorityBindingMismatch);
    }
    let support = support_for_pose(input.pose)?;
    Ok(NativeStackedFoldReadGuardV1 {
        proof: Arc::new(StackedFoldReadGuardProofV1 {
            binding,
            support,
            model: input.model.clone(),
            pose: input.pose.clone(),
        }),
        anchor,
    })
}

pub fn revalidate_stacked_fold_read_guard_v1(
    guard: &NativeStackedFoldReadGuardV1<'_>,
    binding: StackedFoldReadBindingV1,
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    limits: StackedFoldReadLimitsV1,
) -> Result<(), StackedFoldReadErrorV1> {
    validate_project_binding(binding, input)?;
    if !guard.is_for_authorities(binding, input.model, input.pose, input.layer_order)
        || guard.anchor.identity_namespace() != binding.project_id
        || guard.anchor.source_revision() != binding.source_revision
        || guard.anchor.snapshot().provenance.source.identity_namespace != Some(binding.project_id)
        || support_for_pose(input.pose)? != guard.support()
    {
        return Err(StackedFoldReadErrorV1::AuthorityBindingMismatch);
    }
    revalidate_flat_endpoint_layer_order_anchor_v1(&guard.anchor, input, limits.flat_endpoint)
        .map_err(map_anchor_error)
}

pub fn propose_linear_stacked_fold_read_v1<'snapshot>(
    guard: &NativeStackedFoldReadGuardV1<'snapshot>,
    binding: StackedFoldReadBindingV1,
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    candidate: StackedFoldLinearCandidateV1,
    limits: StackedFoldReadLimitsV1,
) -> Result<NativeStackedFoldReadProposalV1<'snapshot>, StackedFoldReadErrorV1> {
    revalidate_stacked_fold_read_guard_v1(guard, binding, input, limits)?;
    validate_candidate(candidate)?;
    let analysis = analyze_candidate(&guard.anchor, candidate, limits)?;
    Ok(NativeStackedFoldReadProposalV1 {
        proof: Arc::new(StackedFoldReadProposalProofV1 {
            guard: Arc::clone(&guard.proof),
            anchor: guard.anchor.clone(),
            candidate,
            crossed_cells: analysis.crossed_cells,
            target_faces: analysis.target_faces,
            work: analysis.work,
        }),
    })
}

pub fn revalidate_linear_stacked_fold_read_proposal_v1(
    proposal: &NativeStackedFoldReadProposalV1<'_>,
    guard: &NativeStackedFoldReadGuardV1<'_>,
    binding: StackedFoldReadBindingV1,
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
    candidate: StackedFoldLinearCandidateV1,
    limits: StackedFoldReadLimitsV1,
) -> Result<(), StackedFoldReadErrorV1> {
    revalidate_stacked_fold_read_guard_v1(guard, binding, input, limits)?;
    validate_candidate(candidate)?;
    if !proposal.is_for_guard(guard)
        || proposal.binding() != binding
        || !candidate_bits_equal(proposal.candidate(), candidate)
    {
        return Err(StackedFoldReadErrorV1::AuthorityBindingMismatch);
    }
    let analysis = analyze_candidate(&guard.anchor, candidate, limits)?;
    if proposal.proof.crossed_cells != analysis.crossed_cells
        || proposal.proof.target_faces != analysis.target_faces
        || proposal.proof.work != analysis.work
    {
        return Err(StackedFoldReadErrorV1::ProposalReverificationFailed);
    }
    Ok(())
}

fn validate_project_binding(
    binding: StackedFoldReadBindingV1,
    input: FlatEndpointLayerOrderInputV1<'_, '_>,
) -> Result<(), StackedFoldReadErrorV1> {
    if binding.project_instance_id.canonical_bytes() == [0; 16]
        || binding.project_id.canonical_bytes() == [0; 16]
        || binding.pose_generation == 0
        || binding.layer_order_generation == 0
    {
        return Err(StackedFoldReadErrorV1::InvalidProjectBinding);
    }
    if binding.project_id != input.identity_namespace
        || binding.source_revision != input.source_revision
        || input.layer_order.provenance.source.identity_namespace != Some(binding.project_id)
        || input.layer_order.provenance.source.source_revision != binding.source_revision
    {
        return Err(StackedFoldReadErrorV1::AuthorityBindingMismatch);
    }
    Ok(())
}

fn support_for_pose(
    pose: &MaterialTreePose,
) -> Result<StackedFoldReadSupportV1, StackedFoldReadErrorV1> {
    let faces = pose.face_ids().len();
    let hinges = pose.hinges().len();
    if faces == 1 && hinges == 0 && pose.fixed_face().is_none() {
        return Ok(StackedFoldReadSupportV1::NoHingeSingleFace);
    }
    if faces >= 2
        && hinges == faces.saturating_sub(1)
        && pose.fixed_face().is_some()
        && pose.hinge_angles().len() == hinges
    {
        if pose
            .hinge_angles()
            .iter()
            .any(|angle| angle.angle_degrees().to_bits() != 180.0_f64.to_bits())
        {
            return Err(StackedFoldReadErrorV1::UnsupportedNonFlatEndpoint);
        }
        return Ok(StackedFoldReadSupportV1::BitExactFlatEndpointTree);
    }
    Err(StackedFoldReadErrorV1::UnsupportedPoseClass { faces, hinges })
}

fn validate_candidate(
    candidate: StackedFoldLinearCandidateV1,
) -> Result<(), StackedFoldReadErrorV1> {
    StackedFoldLinearCandidateV1::new(
        candidate.first,
        candidate.second,
        candidate.fixed_side,
        candidate.rotation_direction,
        candidate.requested_angle_degrees,
    )
    .map(|_| ())
}

fn analyze_candidate(
    anchor: &NativeFlatEndpointLayerOrderAnchorV1<'_>,
    candidate: StackedFoldLinearCandidateV1,
    limits: StackedFoldReadLimitsV1,
) -> Result<ProposalAnalysis, StackedFoldReadErrorV1> {
    let cells = anchor.cells();
    let exact_cells = anchor.exact_cells();
    if cells.is_empty() || cells.len() != exact_cells.len() {
        return Err(StackedFoldReadErrorV1::InternalIndeterminate);
    }
    check_limit(
        StackedFoldReadResourceV1::ScannedCells,
        cells.len(),
        limits.max_scanned_cells,
    )?;
    let line_first = world_to_flat_exact(candidate.first)?;
    let line_second = world_to_flat_exact(candidate.second)?;
    if line_first == line_second {
        return Err(StackedFoldReadErrorV1::InvalidLinearCandidate);
    }

    let mut work = StackedFoldReadWorkV1 {
        scanned_cells: cells.len(),
        ..StackedFoldReadWorkV1::default()
    };
    let mut crossed_cells = Vec::new();
    reserve_output(
        &mut crossed_cells,
        cells.len().min(limits.max_retained_cells),
    )?;
    let mut target_faces = Vec::<FaceId>::new();
    reserve_output(
        &mut target_faces,
        anchor
            .material_faces()
            .len()
            .min(limits.max_retained_target_faces),
    )?;

    for (cell, exact) in cells.iter().zip(exact_cells) {
        validate_cell_alignment(anchor, cell, exact)?;
        work.total_boundary_vertices =
            checked_add(work.total_boundary_vertices, exact.polygon.len())?;
        check_limit(
            StackedFoldReadResourceV1::TotalBoundaryVertices,
            work.total_boundary_vertices,
            limits.max_total_boundary_vertices,
        )?;
        work.total_layer_records =
            checked_add(work.total_layer_records, cell.bottom_to_top_faces().len())?;
        check_limit(
            StackedFoldReadResourceV1::TotalLayerRecords,
            work.total_layer_records,
            limits.max_total_layer_records,
        )?;
        work.orientation_tests = checked_add(work.orientation_tests, exact.polygon.len())?;
        check_limit(
            StackedFoldReadResourceV1::OrientationTests,
            work.orientation_tests,
            limits.max_orientation_tests,
        )?;

        let relation =
            classify_line_cell(&line_first, &line_second, &exact.polygon, &mut work, limits)?;
        match relation {
            LineCellRelation::Separated => {}
            LineCellRelation::AmbiguousBoundary => {
                return Err(StackedFoldReadErrorV1::AmbiguousCellBoundary);
            }
            LineCellRelation::StrictlyCrossed => {
                let next = crossed_cells
                    .len()
                    .checked_add(1)
                    .ok_or(StackedFoldReadErrorV1::ResourceCountOverflow)?;
                check_limit(
                    StackedFoldReadResourceV1::RetainedCells,
                    next,
                    limits.max_retained_cells,
                )?;
                let mut order = Vec::new();
                reserve_output(&mut order, cell.bottom_to_top_faces().len())?;
                order.extend_from_slice(cell.bottom_to_top_faces());
                for face in &order {
                    let key = face.canonical_bytes();
                    if let Err(index) = target_faces
                        .binary_search_by_key(&key, |existing| existing.canonical_bytes())
                    {
                        let next = checked_add(target_faces.len(), 1)?;
                        check_limit(
                            StackedFoldReadResourceV1::RetainedTargetFaces,
                            next,
                            limits.max_retained_target_faces,
                        )?;
                        // Capacity for every possible successful insertion was
                        // reserved before scanning. The limit check above
                        // therefore prevents `insert` from reallocating.
                        target_faces.insert(index, *face);
                    }
                }
                crossed_cells.push(StackedFoldReadCellV1 {
                    cell_key: cell.cell_key(),
                    bottom_to_top_faces: order,
                });
            }
        }
    }
    if crossed_cells.is_empty() {
        return Err(StackedFoldReadErrorV1::NoCrossedLayerCell);
    }
    check_limit(
        StackedFoldReadResourceV1::RetainedTargetFaces,
        target_faces.len(),
        limits.max_retained_target_faces,
    )?;
    work.retained_cells = crossed_cells.len();
    work.retained_target_faces = target_faces.len();
    Ok(ProposalAnalysis {
        crossed_cells,
        target_faces,
        work,
    })
}

fn validate_cell_alignment(
    anchor: &NativeFlatEndpointLayerOrderAnchorV1<'_>,
    cell: &crate::FlatEndpointLayerCellV1,
    exact: &CellGeometry,
) -> Result<(), StackedFoldReadErrorV1> {
    if cell.cell_key().canonical_bytes() != exact.key.0
        || exact.polygon.len() != cell.world_boundary().len()
        || exact.covering_indices.len() != cell.covering_faces().len()
        || exact.bottom_indices.len() != cell.bottom_to_top_faces().len()
        || exact.exact_area <= BigRational::zero()
    {
        return Err(StackedFoldReadErrorV1::InternalIndeterminate);
    }
    let material_faces = anchor.material_faces();
    if exact
        .covering_indices
        .iter()
        .zip(cell.covering_faces())
        .any(|(index, face)| {
            material_faces
                .get(*index)
                .is_none_or(|layer| layer.face_id != *face)
        })
        || exact
            .bottom_indices
            .iter()
            .zip(cell.bottom_to_top_faces())
            .any(|(index, face)| {
                material_faces
                    .get(*index)
                    .is_none_or(|layer| layer.face_id != *face)
            })
    {
        return Err(StackedFoldReadErrorV1::InternalIndeterminate);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineCellRelation {
    Separated,
    StrictlyCrossed,
    AmbiguousBoundary,
}

fn classify_line_cell(
    first: &RationalPoint,
    second: &RationalPoint,
    polygon: &[RationalPoint],
    work: &mut StackedFoldReadWorkV1,
    limits: StackedFoldReadLimitsV1,
) -> Result<LineCellRelation, StackedFoldReadErrorV1> {
    let mut positive = false;
    let mut negative = false;
    let mut zero = false;
    for point in polygon {
        let sign = metered_orientation(first, second, point, work, limits)?;
        positive |= sign.is_positive();
        negative |= sign.is_negative();
        zero |= sign.is_zero();
    }
    Ok(if positive && negative {
        LineCellRelation::StrictlyCrossed
    } else if zero {
        LineCellRelation::AmbiguousBoundary
    } else {
        LineCellRelation::Separated
    })
}

fn world_to_flat_exact(point: Point3) -> Result<RationalPoint, StackedFoldReadErrorV1> {
    if point.y().to_bits() != 0.0_f64.to_bits() {
        return Err(StackedFoldReadErrorV1::InvalidLinearCandidate);
    }
    Ok(RationalPoint {
        x: BigRational::from_float(point.x())
            .ok_or(StackedFoldReadErrorV1::InvalidLinearCandidate)?,
        y: BigRational::from_float(canonical_zero(-point.z()))
            .ok_or(StackedFoldReadErrorV1::InvalidLinearCandidate)?,
    })
}

fn orientation(
    first: &RationalPoint,
    second: &RationalPoint,
    third: &RationalPoint,
) -> BigRational {
    (&second.x - &first.x) * (&third.y - &first.y) - (&second.y - &first.y) * (&third.x - &first.x)
}

#[derive(Debug, Clone, Copy)]
struct RationalBitBound {
    numerator: usize,
    denominator: usize,
}

impl RationalBitBound {
    fn maximum(self) -> usize {
        self.numerator.max(self.denominator)
    }
}

fn metered_orientation(
    first: &RationalPoint,
    second: &RationalPoint,
    third: &RationalPoint,
    work: &mut StackedFoldReadWorkV1,
    limits: StackedFoldReadLimitsV1,
) -> Result<BigRational, StackedFoldReadErrorV1> {
    const OPERATIONS_PER_ORIENTATION: usize = 7;
    let dx_line = subtract_bit_bound(
        rational_bit_bound(&second.x)?,
        rational_bit_bound(&first.x)?,
    )?;
    let dy_line = subtract_bit_bound(
        rational_bit_bound(&second.y)?,
        rational_bit_bound(&first.y)?,
    )?;
    let dx_point =
        subtract_bit_bound(rational_bit_bound(&third.x)?, rational_bit_bound(&first.x)?)?;
    let dy_point =
        subtract_bit_bound(rational_bit_bound(&third.y)?, rational_bit_bound(&first.y)?)?;
    let first_product = multiply_bit_bound(dx_line, dy_point)?;
    let second_product = multiply_bit_bound(dy_line, dx_point)?;
    let result = subtract_bit_bound(first_product, second_product)?;
    let maximum = [
        dx_line.maximum(),
        dy_line.maximum(),
        dx_point.maximum(),
        dy_point.maximum(),
        first_product.maximum(),
        second_product.maximum(),
        result.maximum(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    check_limit(
        StackedFoldReadResourceV1::ExactIntegerBits,
        maximum,
        limits.max_exact_integer_bits,
    )?;
    work.maximum_exact_integer_bits = work.maximum_exact_integer_bits.max(maximum);
    work.total_exact_integer_bits = checked_add(work.total_exact_integer_bits, maximum)?;
    check_limit(
        StackedFoldReadResourceV1::TotalExactIntegerBits,
        work.total_exact_integer_bits,
        limits.max_total_exact_integer_bits,
    )?;
    work.exact_arithmetic_operations =
        checked_add(work.exact_arithmetic_operations, OPERATIONS_PER_ORIENTATION)?;
    check_limit(
        StackedFoldReadResourceV1::ExactArithmeticOperations,
        work.exact_arithmetic_operations,
        limits.max_exact_arithmetic_operations,
    )?;
    let orientation = orientation(first, second, third);
    if rational_bit_bound(&orientation)?.maximum() > maximum {
        return Err(StackedFoldReadErrorV1::InternalIndeterminate);
    }
    Ok(orientation)
}

fn rational_bit_bound(value: &BigRational) -> Result<RationalBitBound, StackedFoldReadErrorV1> {
    Ok(RationalBitBound {
        numerator: usize::try_from(value.numer().bits())
            .map_err(|_| StackedFoldReadErrorV1::ResourceCountOverflow)?
            .max(1),
        denominator: usize::try_from(value.denom().bits())
            .map_err(|_| StackedFoldReadErrorV1::ResourceCountOverflow)?
            .max(1),
    })
}

fn subtract_bit_bound(
    first: RationalBitBound,
    second: RationalBitBound,
) -> Result<RationalBitBound, StackedFoldReadErrorV1> {
    let first_term = checked_add(first.numerator, second.denominator)?;
    let second_term = checked_add(second.numerator, first.denominator)?;
    Ok(RationalBitBound {
        numerator: checked_add(first_term.max(second_term), 1)?,
        denominator: checked_add(first.denominator, second.denominator)?,
    })
}

fn multiply_bit_bound(
    first: RationalBitBound,
    second: RationalBitBound,
) -> Result<RationalBitBound, StackedFoldReadErrorV1> {
    Ok(RationalBitBound {
        numerator: checked_add(first.numerator, second.numerator)?,
        denominator: checked_add(first.denominator, second.denominator)?,
    })
}

fn candidate_bits_equal(
    first: StackedFoldLinearCandidateV1,
    second: StackedFoldLinearCandidateV1,
) -> bool {
    point_bits(first.first) == point_bits(second.first)
        && point_bits(first.second) == point_bits(second.second)
        && first.fixed_side == second.fixed_side
        && first.rotation_direction == second.rotation_direction
        && first.requested_angle_degrees.to_bits() == second.requested_angle_degrees.to_bits()
}

fn point_bits(point: Point3) -> [u64; 3] {
    [
        point.x().to_bits(),
        point.y().to_bits(),
        point.z().to_bits(),
    ]
}

fn checked_add(first: usize, second: usize) -> Result<usize, StackedFoldReadErrorV1> {
    first
        .checked_add(second)
        .ok_or(StackedFoldReadErrorV1::ResourceCountOverflow)
}

fn reserve_output<T>(output: &mut Vec<T>, additional: usize) -> Result<(), StackedFoldReadErrorV1> {
    output
        .try_reserve_exact(additional)
        .map_err(|_| StackedFoldReadErrorV1::AllocationFailed)
}

fn check_limit(
    resource: StackedFoldReadResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), StackedFoldReadErrorV1> {
    if actual > maximum {
        Err(StackedFoldReadErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn map_anchor_error(error: FlatEndpointLayerOrderAnchorErrorV1) -> StackedFoldReadErrorV1 {
    match error {
        FlatEndpointLayerOrderAnchorErrorV1::UnsupportedPoseClass { faces, hinges } => {
            StackedFoldReadErrorV1::UnsupportedPoseClass { faces, hinges }
        }
        FlatEndpointLayerOrderAnchorErrorV1::NotBitExactFlatEndpoint { .. } => {
            StackedFoldReadErrorV1::UnsupportedNonFlatEndpoint
        }
        FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch => {
            StackedFoldReadErrorV1::AuthorityBindingMismatch
        }
        other => StackedFoldReadErrorV1::LayerOrderIndeterminate(other),
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::{
        RationalBitBound, StackedFoldReadErrorV1, checked_add, multiply_bit_bound, reserve_output,
        subtract_bit_bound,
    };

    #[test]
    fn counter_and_exact_bound_overflow_fail_closed() {
        assert_eq!(
            checked_add(usize::MAX, 1),
            Err(StackedFoldReadErrorV1::ResourceCountOverflow)
        );
        assert!(matches!(
            subtract_bit_bound(
                RationalBitBound {
                    numerator: usize::MAX,
                    denominator: 1,
                },
                RationalBitBound {
                    numerator: 1,
                    denominator: 1,
                },
            ),
            Err(StackedFoldReadErrorV1::ResourceCountOverflow)
        ));
        assert!(matches!(
            multiply_bit_bound(
                RationalBitBound {
                    numerator: usize::MAX,
                    denominator: 1,
                },
                RationalBitBound {
                    numerator: 1,
                    denominator: 1,
                },
            ),
            Err(StackedFoldReadErrorV1::ResourceCountOverflow)
        ));

        let mut output = Vec::<u8>::new();
        assert_eq!(
            reserve_output(&mut output, usize::MAX),
            Err(StackedFoldReadErrorV1::AllocationFailed)
        );
    }
}
